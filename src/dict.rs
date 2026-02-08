// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{error::Error, path::Path};

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

// Get the dictionary for a language (eg: `fr` or `pt_BR`).
pub fn get_dict(path: &Path, language: &str) -> Result<Dictionary, Box<dyn Error>> {
    // First look for the dictionary with complete language (eg: `pt_BR`).
    if let Some(dict) = get_dict_name(path, language) {
        return Ok(dict);
    }
    // Then look for the dictionary with language without country (eg: `pt`).
    if let Some(pos) = language.find('_')
        && let Some(dict) = get_dict_name(path, &language[..pos])
    {
        return Ok(dict);
    }
    Err(format!(
        "dictionary not found for language '{language}' (path: {}), spelling rule ignored",
        path.to_string_lossy()
    )
    .into())
}
