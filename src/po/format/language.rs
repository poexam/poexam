// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Supported languages for format strings.

use serde::Serialize;

use crate::po::format::{FormatParser, lang_c::FormatC, lang_null::FormatNull};

#[derive(Debug, Default, PartialEq, Serialize)]
pub enum Language {
    #[default]
    Null,
    C,
}

impl From<&str> for Language {
    fn from(language: &str) -> Self {
        match language {
            "c" => Self::C,
            _ => Self::Null,
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Language::Null => write!(f, "none"),
            Language::C => write!(f, "C"),
        }
    }
}

impl Language {
    pub fn format_parser(&self) -> Box<dyn FormatParser> {
        match self {
            Language::C => Box::new(FormatC),
            Language::Null => Box::new(FormatNull),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language() {
        assert_eq!(Language::from("c"), Language::C);
        assert_eq!(Language::from(""), Language::Null);
        assert_eq!(Language::from("unknown"), Language::Null);
    }
}
