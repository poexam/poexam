// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Dictionary for spell checking in PO files.

use std::{
    cell::RefCell,
    collections::HashMap,
    error::Error,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, Mutex, PoisonError},
    time::SystemTime,
};

use spellbook::Dictionary;

pub const DEFAULT_PATH_DICTS: &str = "/usr/share/hunspell";
pub const DEFAULT_LANG_ID: &str = "en_US";

/// A dictionary and the spell check results already computed with it.
///
/// Checking a word with Hunspell affix rules is far more expensive than a hash
/// lookup, and the same words come back over and over in a PO file (in a
/// typical file, only ~20% of the words are distinct), so every answer is
/// memoized: each distinct word is checked against the dictionary only once.
///
/// The cache is per [`Checker`](crate::checker::Checker), hence per file: it is
/// dropped with the file being checked, while the dictionary itself is shared
/// between files (see [`get_dict`]).
pub struct CachedDict {
    dict: Arc<Dictionary>,
    /// Words already checked, with the result.
    cache: RefCell<HashMap<Box<str>, bool>>,
}

impl CachedDict {
    /// Create a cache in front of a shared dictionary.
    pub fn new(dict: Arc<Dictionary>) -> Self {
        Self {
            dict,
            cache: RefCell::new(HashMap::new()),
        }
    }

    /// Return `true` if `word` is in the dictionary, reusing the result of a
    /// previous check of the same word.
    pub fn check(&self, word: &str) -> bool {
        if let Some(known) = self.cache.borrow().get(word) {
            return *known;
        }
        let correct = self.dict.check(word);
        self.cache.borrow_mut().insert(Box::from(word), correct);
        correct
    }
}

/// A dictionary loaded in this process, with the modification time of the file
/// with extra words it was built from (`None` when there is no such file).
struct LoadedDict {
    words_stamp: Option<SystemTime>,
    dict: Arc<Dictionary>,
}

/// Key of a cached dictionary: dictionaries directory, extra words directory
/// and language.
type DictKey = (PathBuf, Option<PathBuf>, String);

/// Slot of a cached dictionary, empty until the dictionary is loaded.
type DictSlot = Arc<Mutex<Option<LoadedDict>>>;

/// Dictionaries already loaded in this process, shared by all files.
///
/// Parsing a Hunspell dictionary takes tens of milliseconds, which used to be
/// paid again for every PO file checked (and, in the language server, on every
/// keystroke). Each entry has its own lock, so two threads loading *different*
/// languages never wait for each other, while two threads asking for the *same*
/// language load it only once.
static DICTS: LazyLock<Mutex<HashMap<DictKey, DictSlot>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Get the dictionary with its name.
fn get_dict_name(path: &Path, name: &str) -> Option<Dictionary> {
    if let Ok(aff) = std::fs::read_to_string(format!("{}/{name}.aff", path.to_string_lossy()))
        && let Ok(dic) = std::fs::read_to_string(format!("{}/{name}.dic", path.to_string_lossy()))
    {
        Dictionary::new(&aff, &dic).ok()
    } else {
        None
    }
}

/// Get the file with extra words for a language (e.g. `fr.dic` or `pt_BR.dic`),
/// falling back to the language without country (e.g. `pt.dic`).
fn get_words_file(path: &Path, language: &str) -> Option<PathBuf> {
    let file = path.join(format!("{language}.dic"));
    if file.is_file() {
        return Some(file);
    }
    let pos = language.find('_')?;
    let file = path.join(format!("{}.dic", &language[..pos]));
    file.is_file().then_some(file)
}

/// Get the modification time of the file with extra words for a language.
///
/// Used to reload the dictionary when the file has changed since it was cached
/// (the language server runs for a long time, and the user can add words to
/// this file while editing).
fn get_words_stamp(path_words: Option<&PathBuf>, language: &str) -> Option<SystemTime> {
    let file = get_words_file(path_words?, language)?;
    std::fs::metadata(file).ok()?.modified().ok()
}

/// Add words to a dictionary.
fn add_words_to_dict(path: &Path, language: &str, dict: &mut Dictionary) {
    if let Some(file) = get_words_file(path, language)
        && let Ok(words) = std::fs::read_to_string(file)
    {
        for word in words.lines() {
            dict.add(word).ok();
        }
    }
}

// Load the dictionary for a language (e.g. `fr` or `pt_BR`).
//
// Words are added to the dictionary if path_words is set and if a file with ignored words exists
// in this directory.
fn load_dict(
    path_dicts: &Path,
    path_words: Option<&PathBuf>,
    language: &str,
) -> Result<Dictionary, Box<dyn Error>> {
    // First look for the dictionary with complete language (e.g. `pt_BR`).
    if let Some(mut dict) = get_dict_name(path_dicts, language) {
        if let Some(path) = path_words {
            add_words_to_dict(path.as_path(), language, &mut dict);
        }
        return Ok(dict);
    }
    // Then look for the dictionary with language without country (e.g. `pt`).
    if let Some(pos) = language.find('_')
        && let Some(mut dict) = get_dict_name(path_dicts, &language[..pos])
    {
        if let Some(path) = path_words {
            add_words_to_dict(path.as_path(), language, &mut dict);
        }
        return Ok(dict);
    }
    Err(format!(
        "dictionary not found for language '{language}' (path: {}), spelling rule ignored",
        path_dicts.to_string_lossy()
    )
    .into())
}

/// Get the dictionary for a language (e.g. `fr` or `pt_BR`), loading it only
/// the first time it is needed in this process.
///
/// The dictionary is rebuilt when the file with extra words (in `path_words`)
/// has been modified since it was cached. A dictionary that can not be loaded
/// is not cached: the error is returned for each file, as before.
pub fn get_dict(
    path_dicts: &Path,
    path_words: Option<&PathBuf>,
    language: &str,
) -> Result<Arc<Dictionary>, Box<dyn Error>> {
    let key = (
        PathBuf::from(path_dicts),
        path_words.cloned(),
        language.to_string(),
    );
    let entry = {
        let mut dicts = DICTS.lock().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(dicts.entry(key).or_default())
    };
    let mut loaded = entry.lock().unwrap_or_else(PoisonError::into_inner);
    let words_stamp = get_words_stamp(path_words, language);
    if let Some(cached) = loaded.as_ref()
        && cached.words_stamp == words_stamp
    {
        return Ok(Arc::clone(&cached.dict));
    }
    let dict = Arc::new(load_dict(path_dicts, path_words, language)?);
    *loaded = Some(LoadedDict {
        words_stamp,
        dict: Arc::clone(&dict),
    });
    Ok(dict)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(label: &str) -> tempfile::TempDir {
        tempfile::TempDir::with_prefix(format!("poexam-dict-{label}-")).expect("create temp dir")
    }

    /// Minimal valid Hunspell `.aff` content.
    const MIN_AFF: &str = "SET UTF-8\n";

    /// Write a minimal `<dir>/<name>.aff` + `<name>.dic` pair containing the given words.
    fn write_dict(dir: &Path, name: &str, words: &[&str]) {
        std::fs::write(dir.join(format!("{name}.aff")), MIN_AFF).expect("write .aff");
        let mut dic = format!("{}\n", words.len());
        for w in words {
            dic.push_str(w);
            dic.push('\n');
        }
        std::fs::write(dir.join(format!("{name}.dic")), dic).expect("write .dic");
    }

    #[test]
    fn test_get_dict_name_loads_when_files_present() {
        let tmp = tmp_dir("name-ok");
        write_dict(tmp.path(), "en_US", &["hello", "world"]);
        let dict = get_dict_name(tmp.path(), "en_US").expect("dictionary loaded");
        assert!(dict.check("hello"));
        assert!(dict.check("world"));
        assert!(!dict.check("doesnotexistword"));
    }

    #[test]
    fn test_get_dict_name_returns_none_when_aff_missing() {
        let tmp = tmp_dir("name-no-aff");
        // Only the .dic file exists.
        std::fs::write(tmp.path().join("en_US.dic"), "1\nhello\n").expect("write .dic");
        assert!(get_dict_name(tmp.path(), "en_US").is_none());
    }

    #[test]
    fn test_get_dict_name_returns_none_when_dic_missing() {
        let tmp = tmp_dir("name-no-dic");
        std::fs::write(tmp.path().join("en_US.aff"), MIN_AFF).expect("write .aff");
        assert!(get_dict_name(tmp.path(), "en_US").is_none());
    }

    #[test]
    fn test_get_dict_name_returns_none_for_unrelated_lang() {
        let tmp = tmp_dir("name-other-lang");
        write_dict(tmp.path(), "en_US", &["hello"]);
        assert!(get_dict_name(tmp.path(), "fr").is_none());
    }

    #[test]
    fn test_add_words_to_dict_uses_full_language_file() {
        let tmp_dicts = tmp_dir("words-full-dict");
        write_dict(tmp_dicts.path(), "en_US", &["base"]);
        let mut dict = get_dict_name(tmp_dicts.path(), "en_US").expect("base dict");
        assert!(!dict.check("zzcustom"));

        let tmp_words = tmp_dir("words-full-words");
        std::fs::write(tmp_words.path().join("en_US.dic"), "zzcustom\n").expect("write words file");
        add_words_to_dict(tmp_words.path(), "en_US", &mut dict);
        assert!(dict.check("zzcustom"));
    }

    #[test]
    fn test_add_words_to_dict_falls_back_to_lang_root() {
        let tmp_dicts = tmp_dir("words-fallback-dict");
        write_dict(tmp_dicts.path(), "pt", &["base"]);
        let mut dict = get_dict_name(tmp_dicts.path(), "pt").expect("base dict");
        assert!(!dict.check("zzbrword"));

        let tmp_words = tmp_dir("words-fallback-words");
        // Only the language-root file exists; `pt_BR.dic` is absent.
        std::fs::write(tmp_words.path().join("pt.dic"), "zzbrword\n").expect("write words file");
        add_words_to_dict(tmp_words.path(), "pt_BR", &mut dict);
        assert!(dict.check("zzbrword"));
    }

    #[test]
    fn test_add_words_to_dict_noop_when_no_file() {
        let tmp_dicts = tmp_dir("words-none-dict");
        write_dict(tmp_dicts.path(), "en_US", &["hello"]);
        let mut dict = get_dict_name(tmp_dicts.path(), "en_US").expect("base dict");

        let tmp_words = tmp_dir("words-none-words");
        // Empty directory: no `en_US.dic` nor `en.dic`.
        add_words_to_dict(tmp_words.path(), "en_US", &mut dict);
        // Original word is still recognized; nothing else got added.
        assert!(dict.check("hello"));
        assert!(!dict.check("zznothingadded"));
    }

    #[test]
    fn test_get_dict_finds_full_language() {
        let tmp = tmp_dir("get-full");
        write_dict(tmp.path(), "pt_BR", &["alpha"]);
        let dict = get_dict(tmp.path(), None, "pt_BR").expect("dictionary");
        assert!(dict.check("alpha"));
    }

    #[test]
    fn test_get_dict_falls_back_to_lang_root() {
        let tmp = tmp_dir("get-fallback");
        // Only `pt.{aff,dic}` is present; requested language is `pt_BR`.
        write_dict(tmp.path(), "pt", &["beta"]);
        let dict = get_dict(tmp.path(), None, "pt_BR").expect("dictionary via fallback");
        assert!(dict.check("beta"));
    }

    #[test]
    fn test_get_dict_full_takes_precedence_over_root() {
        let tmp = tmp_dir("get-precedence");
        write_dict(tmp.path(), "pt", &["root_word"]);
        write_dict(tmp.path(), "pt_BR", &["full_word"]);
        let dict = get_dict(tmp.path(), None, "pt_BR").expect("dictionary");
        assert!(dict.check("full_word"));
        // The root-only word must not be present when the full-language dict exists.
        assert!(!dict.check("root_word"));
    }

    #[test]
    fn test_get_dict_errors_when_not_found() {
        let tmp = tmp_dir("get-missing");
        let err = get_dict(tmp.path(), None, "fr").expect_err("missing dict is an error");
        let msg = err.to_string();
        assert!(msg.contains("dictionary not found"));
        assert!(msg.contains("'fr'"));
        assert!(msg.contains(&tmp.path().to_string_lossy().to_string()));
    }

    #[test]
    fn test_get_dict_no_underscore_does_not_try_fallback() {
        // Language without `_` means there is no root form to fall back to → straight error.
        let tmp = tmp_dir("get-no-underscore");
        write_dict(tmp.path(), "fr", &["bonjour"]);
        // Asking for `de` (no underscore, no `de.{aff,dic}`) must error even though `fr` exists.
        assert!(get_dict(tmp.path(), None, "de").is_err());
    }

    #[test]
    fn test_get_dict_augments_with_path_words_full_lang() {
        let tmp_dicts = tmp_dir("augment-dicts");
        write_dict(tmp_dicts.path(), "en_US", &["seed"]);

        let tmp_words = tmp_dir("augment-words");
        std::fs::write(tmp_words.path().join("en_US.dic"), "zzextra\n").expect("write words file");

        let words_path = PathBuf::from(tmp_words.path());
        let dict = get_dict(tmp_dicts.path(), Some(&words_path), "en_US").expect("dict");
        assert!(dict.check("seed"));
        assert!(dict.check("zzextra"));
    }

    #[test]
    fn test_get_dict_augments_with_path_words_via_fallback() {
        // Dictionary loaded via the root-language fallback should still get augmented.
        let tmp_dicts = tmp_dir("augment-fallback-dicts");
        write_dict(tmp_dicts.path(), "pt", &["seed"]);

        let tmp_words = tmp_dir("augment-fallback-words");
        // Words file uses the *full* language id (matches what `add_words_to_dict` looks up first).
        std::fs::write(tmp_words.path().join("pt_BR.dic"), "zzbrextra\n")
            .expect("write words file");

        let words_path = PathBuf::from(tmp_words.path());
        let dict = get_dict(tmp_dicts.path(), Some(&words_path), "pt_BR").expect("dict");
        assert!(dict.check("seed"));
        assert!(dict.check("zzbrextra"));
    }

    #[test]
    fn test_get_dict_returns_the_same_dictionary_for_the_same_language() {
        let tmp = tmp_dir("cache-hit");
        write_dict(tmp.path(), "en_US", &["hello"]);
        let dict1 = get_dict(tmp.path(), None, "en_US").expect("dictionary");
        let dict2 = get_dict(tmp.path(), None, "en_US").expect("dictionary");
        // Loaded once, then shared: no second parsing of the .aff/.dic files.
        assert!(Arc::ptr_eq(&dict1, &dict2));
        // A different language is a different dictionary.
        write_dict(tmp.path(), "fr", &["bonjour"]);
        let dict_fr = get_dict(tmp.path(), None, "fr").expect("dictionary");
        assert!(!Arc::ptr_eq(&dict1, &dict_fr));
    }

    #[test]
    fn test_get_dict_reloads_when_words_file_changed() {
        let tmp_dicts = tmp_dir("reload-dicts");
        write_dict(tmp_dicts.path(), "en_US", &["seed"]);

        let tmp_words = tmp_dir("reload-words");
        let words_file = tmp_words.path().join("en_US.dic");
        std::fs::write(&words_file, "zzfirst\n").expect("write words file");
        let words_path = PathBuf::from(tmp_words.path());

        let dict1 = get_dict(tmp_dicts.path(), Some(&words_path), "en_US").expect("dictionary");
        assert!(dict1.check("zzfirst"));
        assert!(!dict1.check("zzsecond"));

        // Words file untouched: the cached dictionary is reused.
        let dict2 = get_dict(tmp_dicts.path(), Some(&words_path), "en_US").expect("dictionary");
        assert!(Arc::ptr_eq(&dict1, &dict2));

        // Words file modified: the dictionary is built again with the new words.
        std::fs::write(&words_file, "zzfirst\nzzsecond\n").expect("update words file");
        std::fs::File::options()
            .write(true)
            .open(&words_file)
            .expect("open words file")
            .set_modified(SystemTime::now() + std::time::Duration::from_secs(1))
            .expect("set modification time");
        let dict3 = get_dict(tmp_dicts.path(), Some(&words_path), "en_US").expect("dictionary");
        assert!(!Arc::ptr_eq(&dict1, &dict3));
        assert!(dict3.check("zzsecond"));
    }

    #[test]
    fn test_cached_dict_check_matches_dictionary() {
        let tmp = tmp_dir("cached-check");
        write_dict(tmp.path(), "en_US", &["hello", "world"]);
        let dict = CachedDict::new(get_dict(tmp.path(), None, "en_US").expect("dictionary"));
        // Same answers on the first check and on the memoized ones.
        for _ in 0..2 {
            assert!(dict.check("hello"));
            assert!(dict.check("world"));
            assert!(!dict.check("doesnotexistword"));
        }
    }
}
