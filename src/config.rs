// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Configuration options.

use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

use crate::args;
use crate::diagnostic::Severity;
use crate::dict;

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
    #[serde(default = "default_check_fuzzy")]
    pub fuzzy: bool,

    #[serde(default = "default_check_noqa")]
    pub noqa: bool,

    #[serde(default = "default_check_obsolete")]
    pub obsolete: bool,

    #[serde(default = "default_check_select")]
    pub select: Vec<String>,

    #[serde(default = "default_check_ignore")]
    pub ignore: Vec<String>,

    #[serde(default = "default_check_path_msgfmt")]
    pub path_msgfmt: PathBuf,

    #[serde(default = "default_check_path_dicts")]
    pub path_dicts: PathBuf,

    #[serde(default = "default_check_path_words")]
    pub path_words: Option<PathBuf>,

    #[serde(default = "default_check_lang_id")]
    pub lang_id: String,

    #[serde(default = "default_check_langs")]
    pub langs: Vec<String>,

    #[serde(default = "default_check_severity")]
    pub severity: Vec<Severity>,

    #[serde(default = "default_check_punc_ignore_ellipsis")]
    pub punc_ignore_ellipsis: bool,
}

/// Default value for `check.fuzzy`.
fn default_check_fuzzy() -> bool {
    false
}

/// Default value for `check.noqa`.
fn default_check_noqa() -> bool {
    false
}

/// Default value for `check.obsolete`.
fn default_check_obsolete() -> bool {
    false
}

/// Default value for `check.select`.
fn default_check_select() -> Vec<String> {
    vec![String::from("default")]
}

/// Default value for `check.ignore`.
fn default_check_ignore() -> Vec<String> {
    vec![]
}

/// Default value for `check.path_msgfmt`.
fn default_check_path_msgfmt() -> PathBuf {
    PathBuf::from(DEFAULT_PATH_MSGFMT)
}

/// Default value for `check.path_dicts`.
fn default_check_path_dicts() -> PathBuf {
    PathBuf::from(dict::DEFAULT_PATH_DICTS)
}

/// Default value for `check.path_words`.
fn default_check_path_words() -> Option<PathBuf> {
    None
}

/// Default value for `check.lang_id`.
fn default_check_lang_id() -> String {
    String::from(dict::DEFAULT_LANG_ID)
}

/// Default value for `check.langs`.
fn default_check_langs() -> Vec<String> {
    vec![]
}

/// Default value for `check.severity`.
fn default_check_severity() -> Vec<Severity> {
    vec![]
}

/// Default value for `check.punc_ignore_ellipsis`.
fn default_check_punc_ignore_ellipsis() -> bool {
    false
}

impl Default for CheckConfig {
    fn default() -> Self {
        Self {
            fuzzy: default_check_fuzzy(),
            noqa: default_check_noqa(),
            obsolete: default_check_obsolete(),
            select: default_check_select(),
            ignore: default_check_ignore(),
            path_msgfmt: default_check_path_msgfmt(),
            path_dicts: default_check_path_dicts(),
            path_words: default_check_path_words(),
            lang_id: default_check_lang_id(),
            langs: default_check_langs(),
            severity: default_check_severity(),
            punc_ignore_ellipsis: default_check_punc_ignore_ellipsis(),
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
        if let Some(path) = path {
            config.path = Some(PathBuf::from(path));
        }
        Ok(config)
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
        } else if let Some(path_words) = &self.check.path_words
            && path_words.is_relative()
            && let Some(config_path) = &self.path
            && let Some(config_dir) = config_path.parent()
        {
            let path = PathBuf::from(config_dir).join(path_words);
            self.check.path_words = match path.canonicalize() {
                Ok(abs_path) => Some(abs_path),
                Err(_) => Some(path),
            };
        }
        if let Some(lang_id) = &args.lang_id {
            self.check.lang_id = String::from(lang_id);
        }
        if let Some(langs) = &args.langs {
            self.check.langs = langs.split(',').map(|s| s.trim().to_string()).collect();
        }
        if !args.severity.is_empty() {
            self.check.severity.clone_from(&args.severity);
        }
        if args.punc_ignore_ellipsis {
            self.check.punc_ignore_ellipsis = true;
        }
        self
    }
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
