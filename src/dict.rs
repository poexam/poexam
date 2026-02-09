// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    error::Error,
    path::{Path, PathBuf},
};

use spellbook::Dictionary;

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

/// Add words to a dictionary.
fn add_words_to_dict(path: &Path, language: &str, dict: &mut Dictionary) {
    if let Ok(words) =
        std::fs::read_to_string(format!("{}/{}.dic", path.to_string_lossy(), language))
    {
        for word in words.lines() {
            dict.add(word).ok();
        }
    } else if let Some(pos) = language.find('_')
        && let Ok(words) = std::fs::read_to_string(format!(
            "{}/{}.dic",
            path.to_string_lossy(),
            &language[..pos]
        ))
    {
        for word in words.lines() {
            dict.add(word).ok();
        }
    }
}

// Get the dictionary for a language (e.g. `fr` or `pt_BR`).
//
// Words are added to the dictionary if path_words is set and if a file with ignored words exists
// in this directory.
pub fn get_dict(
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
