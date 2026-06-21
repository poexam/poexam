// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Configuration options.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::error::Error;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

use crate::args;
use crate::diagnostic::Severity;
use crate::dict;
use crate::po::wrap::DEFAULT_PAGE_WIDTH;

pub const DEFAULT_PATH_MSGFMT: &str = "/usr/bin/msgfmt";

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(skip)]
    pub path: Option<PathBuf>,

    #[serde(default)]
    pub check: CheckConfig,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct CheckConfig {
    #[serde(default)]
    pub fuzzy: bool,

    #[serde(default)]
    pub noqa: bool,

    #[serde(default)]
    pub obsolete: bool,

    #[serde(default = "default_check_select")]
    pub select: Vec<String>,

    #[serde(default)]
    pub ignore: Vec<String>,

    #[serde(default = "default_check_path_msgfmt")]
    pub path_msgfmt: PathBuf,

    #[serde(default = "default_check_path_dicts")]
    pub path_dicts: PathBuf,

    #[serde(default)]
    pub path_words: Option<PathBuf>,

    #[serde(default)]
    pub force_trans_file: Option<PathBuf>,

    #[serde(default)]
    pub no_trans_file: Option<PathBuf>,

    #[serde(default = "default_check_lang_id")]
    pub lang_id: String,

    #[serde(default)]
    pub langs: Vec<String>,

    #[serde(default = "default_check_short_factor")]
    pub short_factor: u16,

    #[serde(default = "default_check_long_factor")]
    pub long_factor: u16,

    #[serde(default)]
    pub severity: Vec<Severity>,

    #[serde(default)]
    pub punc_ignore_ellipsis: bool,

    #[serde(default = "default_check_accelerator")]
    pub accelerator: char,

    #[serde(default = "default_check_width")]
    pub width: usize,
}

/// Default value for `check.select`.
fn default_check_select() -> Vec<String> {
    vec![String::from("default")]
}

/// Default value for `check.path_msgfmt`.
fn default_check_path_msgfmt() -> PathBuf {
    PathBuf::from(DEFAULT_PATH_MSGFMT)
}

/// Default value for `check.path_dicts`.
fn default_check_path_dicts() -> PathBuf {
    PathBuf::from(dict::DEFAULT_PATH_DICTS)
}

/// Default value for `check.lang_id`.
fn default_check_lang_id() -> String {
    String::from(dict::DEFAULT_LANG_ID)
}

/// Default value for `check.short_factor`.
fn default_check_short_factor() -> u16 {
    8
}

/// Default value for `check.long_factor`.
fn default_check_long_factor() -> u16 {
    8
}

/// Default value for `check.accelerator`.
const fn default_check_accelerator() -> char {
    '&'
}

/// Default value for `check.width`.
const fn default_check_width() -> usize {
    DEFAULT_PAGE_WIDTH
}

impl Default for CheckConfig {
    fn default() -> Self {
        Self {
            fuzzy: false,
            noqa: false,
            obsolete: false,
            select: default_check_select(),
            ignore: vec![],
            path_msgfmt: default_check_path_msgfmt(),
            path_dicts: default_check_path_dicts(),
            path_words: None,
            force_trans_file: None,
            no_trans_file: None,
            lang_id: default_check_lang_id(),
            langs: vec![],
            short_factor: default_check_short_factor(),
            long_factor: default_check_long_factor(),
            severity: vec![],
            punc_ignore_ellipsis: false,
            accelerator: default_check_accelerator(),
            width: default_check_width(),
        }
    }
}

impl Config {
    /// Create a configuration by reading a configuration file.
    pub fn new(path: Option<&PathBuf>) -> Result<Self, Box<dyn Error>> {
        let content = match path {
            Some(cfg_path) => match read_to_string(cfg_path) {
                Ok(content) => content,
                Err(err) => return Err(format!("could not read config: {err}").into()),
            },
            None => String::new(),
        };
        let mut config: Self = toml::from_str(&content)?;
        if config.check.short_factor < 2 {
            return Err(format!(
                "invalid `check.short_factor`: {} (min: 2)",
                config.check.short_factor,
            )
            .into());
        }
        if config.check.long_factor < 2 {
            return Err(format!(
                "invalid `check.long_factor`: {} (min: 2)",
                config.check.long_factor,
            )
            .into());
        }
        if let Some(path) = path {
            config.path = Some(PathBuf::from(path));
        }
        Ok(config)
    }

    /// Directory of the loaded config file, if any.
    fn config_dir(&self) -> Option<PathBuf> {
        self.path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
    }

    /// Resolve relative `path_words` / `force_trans_file` / `no_trans_file`
    /// values against the directory of the loaded config file, canonicalizing
    /// when possible. A no-op for absolute paths or when no config file path is
    /// set.
    ///
    /// The CLI does this inside [`with_args_check`](Self::with_args_check); the
    /// language server, which loads the config without command-line args, calls
    /// this directly so a config-relative word-list path resolves the same way
    /// in both.
    pub fn resolve_relative_paths(&mut self) {
        let config_dir = self.config_dir();
        let config_dir = config_dir.as_deref();
        resolve_config_relative(&mut self.check.path_words, config_dir);
        resolve_config_relative(&mut self.check.force_trans_file, config_dir);
        resolve_config_relative(&mut self.check.no_trans_file, config_dir);
    }

    /// Update the configuration with command-line arguments.
    pub fn with_args_check(mut self, args: &args::CheckArgs) -> Self {
        if args.fuzzy {
            self.check.fuzzy = true;
        }
        if args.noqa {
            self.check.noqa = true;
        }
        if args.obsolete {
            self.check.obsolete = true;
        }
        if let Some(select) = &args.select {
            self.check.select = select.split(',').map(|s| s.trim().to_string()).collect();
        }
        if let Some(ignore) = &args.ignore {
            self.check.ignore = ignore.split(',').map(|s| s.trim().to_string()).collect();
        }
        if let Some(path_msgfmt) = &args.path_msgfmt {
            self.check.path_msgfmt = PathBuf::from(path_msgfmt);
        }
        if let Some(path_dicts) = &args.path_dicts {
            self.check.path_dicts = PathBuf::from(path_dicts);
        }
        if let Some(path_words) = &args.path_words {
            self.check.path_words = Some(PathBuf::from(path_words));
        } else {
            let config_dir = self.config_dir();
            resolve_config_relative(&mut self.check.path_words, config_dir.as_deref());
        }
        if let Some(force_trans_file) = &args.force_trans_file {
            self.check.force_trans_file = Some(PathBuf::from(force_trans_file));
        } else {
            let config_dir = self.config_dir();
            resolve_config_relative(&mut self.check.force_trans_file, config_dir.as_deref());
        }
        if let Some(no_trans_file) = &args.no_trans_file {
            self.check.no_trans_file = Some(PathBuf::from(no_trans_file));
        } else {
            let config_dir = self.config_dir();
            resolve_config_relative(&mut self.check.no_trans_file, config_dir.as_deref());
        }
        if let Some(lang_id) = &args.lang_id {
            self.check.lang_id = String::from(lang_id);
        }
        if let Some(langs) = &args.langs {
            self.check.langs = langs.split(',').map(|s| s.trim().to_string()).collect();
        }
        if let Some(short_factor) = args.short_factor {
            self.check.short_factor = short_factor;
        }
        if let Some(long_factor) = args.long_factor {
            self.check.long_factor = long_factor;
        }
        if !args.severity.is_empty() {
            self.check.severity.clone_from(&args.severity);
        }
        if args.punc_ignore_ellipsis {
            self.check.punc_ignore_ellipsis = true;
        }
        if let Some(accelerator) = args.accelerator {
            self.check.accelerator = accelerator;
        }
        if let Some(width) = args.width {
            self.check.width = width;
        }
        self
    }
}

/// Resolve a relative path stored in the config against the config file's
/// directory (`config_dir`), canonicalizing when possible and falling back to
/// the joined path otherwise. A no-op for `None`, absolute paths, or an unknown
/// config directory.
fn resolve_config_relative(value: &mut Option<PathBuf>, config_dir: Option<&Path>) {
    if let Some(path) = value.as_ref()
        && path.is_relative()
        && let Some(dir) = config_dir
    {
        let joined = dir.join(path);
        *value = Some(joined.canonicalize().unwrap_or(joined));
    }
}

/// Load a word list from a file: one word per line, lines starting with `#`
/// are comments, blank lines are ignored, and words are lowercased so that
/// callers can match case-insensitively. Used by the `force-trans` and
/// `no-trans` rules.
pub fn load_word_list(path: &Path) -> Result<HashSet<String>, std::io::Error> {
    let content = read_to_string(path)?;
    Ok(content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_lowercase)
        .collect())
}

/// Find the configuration file for a PO file.
///
/// Look for paths in this order (``{path}`` being the path to the PO file):
/// - ``{path}/.poexam/poexam.toml``
/// - ``{path}/poexam.toml``
/// - ``{path}/.poexam.toml``
///
/// If no configuration file is found, search in the parent directory, and so on until
/// the root directory is reached.
pub fn find_config_path(po_path: &Path) -> Option<PathBuf> {
    let Ok(abs_path) = po_path.canonicalize() else {
        return None;
    };
    for path in abs_path.ancestors() {
        let p = path.join(".poexam/poexam.toml");
        if p.exists() {
            return Some(p);
        }
        let p = path.join("poexam.toml");
        if p.exists() {
            return Some(p);
        }
        let p = path.join(".poexam.toml");
        if p.exists() {
            return Some(p);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a temp directory and return its handle plus the canonical path.
    /// `find_config_path` canonicalizes its input, so tests that compare against
    /// its output must use the canonical form (matters on macOS where the
    /// system temp dir is symlinked).
    fn tmp_dir(label: &str) -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::TempDir::with_prefix(format!("poexam-cfg-{label}-"))
            .expect("create temp dir");
        let canonical = tmp.path().canonicalize().expect("canonicalize temp dir");
        (tmp, canonical)
    }

    fn default_check_args() -> args::CheckArgs {
        args::CheckArgs {
            files: vec![],
            show_settings: false,
            config: None,
            no_config: false,
            fuzzy: false,
            noqa: false,
            obsolete: false,
            select: None,
            ignore: None,
            path_msgfmt: None,
            path_dicts: None,
            path_words: None,
            force_trans_file: None,
            no_trans_file: None,
            lang_id: None,
            langs: None,
            short_factor: None,
            long_factor: None,
            severity: vec![],
            punc_ignore_ellipsis: false,
            accelerator: None,
            no_errors: false,
            sort: args::CheckSort::default(),
            rule_stats: false,
            file_stats: false,
            output: args::CheckOutputFormat::default(),
            quiet: false,
            fix: false,
            width: None,
        }
    }

    #[test]
    fn test_default_helpers() {
        assert_eq!(default_check_select(), vec!["default".to_string()]);
        assert_eq!(
            default_check_path_msgfmt(),
            PathBuf::from(DEFAULT_PATH_MSGFMT)
        );
        assert_eq!(
            default_check_path_dicts(),
            PathBuf::from(dict::DEFAULT_PATH_DICTS),
        );
        assert_eq!(default_check_lang_id(), dict::DEFAULT_LANG_ID);
    }

    #[test]
    fn test_check_config_default() {
        let c = CheckConfig::default();
        assert!(!c.fuzzy);
        assert!(!c.noqa);
        assert!(!c.obsolete);
        assert_eq!(c.select, vec!["default".to_string()]);
        assert!(c.ignore.is_empty());
        assert_eq!(c.path_msgfmt, PathBuf::from(DEFAULT_PATH_MSGFMT));
        assert_eq!(c.path_dicts, PathBuf::from(dict::DEFAULT_PATH_DICTS));
        assert!(c.path_words.is_none());
        assert_eq!(c.lang_id, dict::DEFAULT_LANG_ID);
        assert!(c.langs.is_empty());
        assert!(c.severity.is_empty());
        assert!(!c.punc_ignore_ellipsis);
        assert_eq!(c.accelerator, '&');
    }

    #[test]
    fn test_config_new_no_path_yields_defaults() {
        let c = Config::new(None).expect("config builds without a path");
        assert!(c.path.is_none());
        assert_eq!(c.check.select, vec!["default".to_string()]);
        assert_eq!(c.check.lang_id, dict::DEFAULT_LANG_ID);
        assert!(!c.check.fuzzy);
    }

    #[test]
    fn test_config_new_reads_toml_and_keeps_path() {
        let (_tmp, root) = tmp_dir("cfg-read");
        let cfg_path = root.join("poexam.toml");
        std::fs::write(
            &cfg_path,
            r#"
[check]
fuzzy = true
select = ["spelling", "html-tags"]
ignore = ["urls"]
lang_id = "fr"
punc_ignore_ellipsis = true
"#,
        )
        .expect("write config file");
        let c = Config::new(Some(&cfg_path)).expect("parse config");
        assert_eq!(c.path.as_deref(), Some(cfg_path.as_path()));
        assert!(c.check.fuzzy);
        assert_eq!(
            c.check.select,
            vec!["spelling".to_string(), "html-tags".to_string()],
        );
        assert_eq!(c.check.ignore, vec!["urls".to_string()]);
        assert_eq!(c.check.lang_id, "fr");
        assert!(c.check.punc_ignore_ellipsis);
        // Unspecified fields fall back to defaults.
        assert!(!c.check.noqa);
        assert_eq!(c.check.path_msgfmt, PathBuf::from(DEFAULT_PATH_MSGFMT));
    }

    #[test]
    fn test_config_new_reads_accelerator() {
        let (_tmp, root) = tmp_dir("cfg-accel");
        let cfg_path = root.join("poexam.toml");
        std::fs::write(&cfg_path, "[check]\naccelerator = \"_\"\n").expect("write config");
        let c = Config::new(Some(&cfg_path)).expect("parse config");
        assert_eq!(c.check.accelerator, '_');
    }

    #[test]
    fn test_with_args_check_accelerator_overrides() {
        let mut args = default_check_args();
        args.accelerator = Some('_');
        let cfg = Config::default().with_args_check(&args);
        assert_eq!(cfg.check.accelerator, '_');
    }

    #[test]
    fn test_config_new_missing_file_returns_err() {
        let missing = PathBuf::from("/this/path/should/not/exist/poexam.toml");
        let err = Config::new(Some(&missing)).expect_err("missing file is an error");
        assert!(err.to_string().contains("could not read config"));
    }

    #[test]
    fn test_config_new_invalid_toml_returns_err() {
        let (_tmp, root) = tmp_dir("cfg-bad");
        let cfg_path = root.join("poexam.toml");
        std::fs::write(&cfg_path, "not = valid = toml").expect("write file");
        assert!(Config::new(Some(&cfg_path)).is_err());
    }

    #[test]
    fn test_config_new_rejects_factor_below_min() {
        let (_tmp, root) = tmp_dir("cfg-factor");
        let cfg_path = root.join("poexam.toml");

        std::fs::write(&cfg_path, "[check]\nshort_factor = 1\n").expect("write config");
        let err = Config::new(Some(&cfg_path)).expect_err("short_factor below min is an error");
        let msg = err.to_string();
        assert!(msg.contains("check.short_factor"));
        assert!(msg.contains("min: 2"));

        std::fs::write(&cfg_path, "[check]\nlong_factor = 0\n").expect("rewrite config");
        let err = Config::new(Some(&cfg_path)).expect_err("long_factor below min is an error");
        let msg = err.to_string();
        assert!(msg.contains("check.long_factor"));
        assert!(msg.contains("min: 2"));
    }

    #[test]
    fn test_with_args_check_no_overrides_keeps_defaults() {
        let cfg = Config::default().with_args_check(&default_check_args());
        assert!(!cfg.check.fuzzy);
        assert_eq!(cfg.check.select, vec!["default".to_string()]);
        assert!(cfg.check.ignore.is_empty());
        assert_eq!(cfg.check.path_msgfmt, PathBuf::from(DEFAULT_PATH_MSGFMT));
        assert!(cfg.check.path_words.is_none());
        assert_eq!(cfg.check.lang_id, dict::DEFAULT_LANG_ID);
        assert!(cfg.check.severity.is_empty());
    }

    #[test]
    fn test_with_args_check_booleans() {
        let mut args = default_check_args();
        args.fuzzy = true;
        args.noqa = true;
        args.obsolete = true;
        args.punc_ignore_ellipsis = true;
        let cfg = Config::default().with_args_check(&args);
        assert!(cfg.check.fuzzy);
        assert!(cfg.check.noqa);
        assert!(cfg.check.obsolete);
        assert!(cfg.check.punc_ignore_ellipsis);
    }

    #[test]
    fn test_with_args_check_booleans_do_not_unset_existing() {
        // Args are additive: a `false` flag must not turn off a config-set `true`.
        let cfg = Config {
            check: CheckConfig {
                fuzzy: true,
                noqa: true,
                ..CheckConfig::default()
            },
            ..Config::default()
        };
        let cfg = cfg.with_args_check(&default_check_args());
        assert!(cfg.check.fuzzy);
        assert!(cfg.check.noqa);
    }

    #[test]
    fn test_with_args_check_comma_lists_split_and_trim() {
        let mut args = default_check_args();
        args.select = Some(" spelling , html-tags ".to_string());
        args.ignore = Some("urls,paths".to_string());
        args.langs = Some("en_US, fr ,de".to_string());
        let cfg = Config::default().with_args_check(&args);
        assert_eq!(
            cfg.check.select,
            vec!["spelling".to_string(), "html-tags".to_string()],
        );
        assert_eq!(
            cfg.check.ignore,
            vec!["urls".to_string(), "paths".to_string()],
        );
        assert_eq!(
            cfg.check.langs,
            vec!["en_US".to_string(), "fr".to_string(), "de".to_string()],
        );
    }

    #[test]
    fn test_with_args_check_paths_and_lang_id() {
        let mut args = default_check_args();
        args.path_msgfmt = Some(PathBuf::from("/opt/bin/msgfmt"));
        args.path_dicts = Some(PathBuf::from("/opt/share/hunspell"));
        args.path_words = Some(PathBuf::from("/opt/words"));
        args.lang_id = Some("de".to_string());
        let cfg = Config::default().with_args_check(&args);
        assert_eq!(cfg.check.path_msgfmt, PathBuf::from("/opt/bin/msgfmt"));
        assert_eq!(cfg.check.path_dicts, PathBuf::from("/opt/share/hunspell"));
        assert_eq!(cfg.check.path_words, Some(PathBuf::from("/opt/words")));
        assert_eq!(cfg.check.lang_id, "de");
    }

    #[test]
    fn test_with_args_check_severity_replaces_when_non_empty() {
        let mut args = default_check_args();
        args.severity = vec![Severity::Warning, Severity::Error];
        let cfg = Config::default().with_args_check(&args);
        assert_eq!(cfg.check.severity, vec![Severity::Warning, Severity::Error]);
    }

    #[test]
    fn test_with_args_check_resolves_relative_path_words_against_config_dir() {
        // When args.path_words is None and config has a relative path_words plus a known
        // config file path, the path should be resolved against the config's directory.
        let (_tmp, root) = tmp_dir("path-words");
        let words_dir = root.join("words");
        std::fs::create_dir_all(&words_dir).expect("create words dir");
        let cfg_path = root.join("poexam.toml");
        std::fs::write(&cfg_path, "").expect("write empty config");

        let cfg = Config {
            path: Some(cfg_path),
            check: CheckConfig {
                path_words: Some(PathBuf::from("words")),
                ..CheckConfig::default()
            },
        };
        let cfg = cfg.with_args_check(&default_check_args());

        let resolved = cfg.check.path_words.expect("path_words resolved");
        let expected = words_dir.canonicalize().expect("canonicalize words dir");
        assert_eq!(resolved, expected);
    }

    #[test]
    fn test_find_config_path_priority_dot_poexam_dir_first() {
        // All three candidate paths exist in the same directory; the `.poexam/poexam.toml`
        // form must win the priority race.
        let (_tmp, root) = tmp_dir("cfg-priority");
        let hidden_dir = root.join(".poexam");
        std::fs::create_dir_all(&hidden_dir).expect("create .poexam dir");
        let winner = hidden_dir.join("poexam.toml");
        std::fs::write(&winner, "").expect("write winner");
        std::fs::write(root.join("poexam.toml"), "").expect("write poexam.toml");
        std::fs::write(root.join(".poexam.toml"), "").expect("write .poexam.toml");
        let po = root.join("fr.po");
        std::fs::write(&po, "").expect("write po file");

        let found = find_config_path(&po).expect("config found");
        assert_eq!(found, winner);
    }

    #[test]
    fn test_find_config_path_finds_poexam_toml_when_only_one() {
        let (_tmp, root) = tmp_dir("cfg-plain");
        let cfg_path = root.join("poexam.toml");
        std::fs::write(&cfg_path, "").expect("write config");
        let po = root.join("fr.po");
        std::fs::write(&po, "").expect("write po file");

        let found = find_config_path(&po).expect("config found");
        assert_eq!(found, cfg_path);
    }

    #[test]
    fn test_find_config_path_finds_dot_poexam_toml_when_only_one() {
        let (_tmp, root) = tmp_dir("cfg-dot");
        let cfg_path = root.join(".poexam.toml");
        std::fs::write(&cfg_path, "").expect("write config");
        let po = root.join("fr.po");
        std::fs::write(&po, "").expect("write po file");

        let found = find_config_path(&po).expect("config found");
        assert_eq!(found, cfg_path);
    }

    #[test]
    fn test_find_config_path_walks_ancestors() {
        // Config in a parent directory, PO file two levels deeper.
        let (_tmp, root) = tmp_dir("cfg-ancestors");
        let cfg_path = root.join("poexam.toml");
        std::fs::write(&cfg_path, "").expect("write config");
        let sub = root.join("a/b");
        std::fs::create_dir_all(&sub).expect("create nested dirs");
        let po = sub.join("fr.po");
        std::fs::write(&po, "").expect("write po file");

        let found = find_config_path(&po).expect("config found by walking up");
        assert_eq!(found, cfg_path);
    }

    #[test]
    fn test_find_config_path_returns_none_for_nonexistent_input() {
        let missing = PathBuf::from("/this/path/should/not/exist/file.po");
        assert!(find_config_path(&missing).is_none());
    }

    /// Write a temporary word-list file with the given content and return its path.
    fn write_word_list(label: &str, content: &str) -> (tempfile::TempDir, PathBuf) {
        let (tmp, root) = tmp_dir(label);
        let path = root.join("words.txt");
        std::fs::write(&path, content).expect("write word-list file");
        (tmp, path)
    }

    #[test]
    fn test_load_word_list_basic_one_per_line() {
        let (_tmp, path) = write_word_list("words-basic", "alpha\nbeta\ngamma\n");
        let words = load_word_list(&path).expect("load");
        assert_eq!(words.len(), 3);
        assert!(words.contains("alpha"));
        assert!(words.contains("beta"));
        assert!(words.contains("gamma"));
    }

    #[test]
    fn test_load_word_list_lowercases_entries() {
        // Mixed-case input is normalized so callers can match case-insensitively.
        let (_tmp, path) = write_word_list("words-case", "Linux\nFOO\nBaR\n");
        let words = load_word_list(&path).expect("load");
        assert_eq!(words.len(), 3);
        assert!(words.contains("linux"));
        assert!(words.contains("foo"));
        assert!(words.contains("bar"));
        // Original case is not retained.
        assert!(!words.contains("Linux"));
        assert!(!words.contains("FOO"));
    }

    #[test]
    fn test_load_word_list_skips_blank_lines_and_comments() {
        let (_tmp, path) = write_word_list(
            "words-comments",
            "# leading comment\n\nalpha\n\n   # indented comment\nbeta\n\n",
        );
        let words = load_word_list(&path).expect("load");
        assert_eq!(words.len(), 2);
        assert!(words.contains("alpha"));
        assert!(words.contains("beta"));
    }

    #[test]
    fn test_load_word_list_trims_surrounding_whitespace() {
        let (_tmp, path) = write_word_list("words-trim", "  alpha  \n\t beta\t\n");
        let words = load_word_list(&path).expect("load");
        assert_eq!(words.len(), 2);
        assert!(words.contains("alpha"));
        assert!(words.contains("beta"));
    }

    #[test]
    fn test_load_word_list_deduplicates() {
        // The same word repeated (and across cases) collapses into one entry.
        let (_tmp, path) = write_word_list("words-dup", "alpha\nALPHA\nalpha\nbeta\n");
        let words = load_word_list(&path).expect("load");
        assert_eq!(words.len(), 2);
        assert!(words.contains("alpha"));
        assert!(words.contains("beta"));
    }

    #[test]
    fn test_load_word_list_empty_file_yields_empty_set() {
        let (_tmp, path) = write_word_list("words-empty", "");
        let words = load_word_list(&path).expect("load");
        assert!(words.is_empty());
    }

    #[test]
    fn test_load_word_list_only_comments_and_blanks_yields_empty_set() {
        let (_tmp, path) =
            write_word_list("words-only-comments", "# header\n\n\n# trailing\n   \n");
        let words = load_word_list(&path).expect("load");
        assert!(words.is_empty());
    }

    #[test]
    fn test_load_word_list_missing_file_returns_io_error() {
        let missing = PathBuf::from("/this/path/should/not/exist/words.txt");
        let err = load_word_list(&missing).expect_err("missing file is an error");
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }
}
