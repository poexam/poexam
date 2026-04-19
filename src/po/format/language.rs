// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Supported languages for format strings.

use serde::Serialize;

use crate::po::format::{
    FormatParser,
    lang_c::FormatC,
    lang_java::FormatJava,
    lang_null::FormatNull,
    lang_python::{FormatPython, FormatPythonBrace},
};

#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub enum Language {
    #[default]
    Null,
    C,
    Java,
    Python,
    PythonBrace,
}

impl From<&str> for Language {
    fn from(language: &str) -> Self {
        match language {
            "c" => Self::C,
            "java" => Self::Java,
            "python" => Self::Python,
            "python-brace" => Self::PythonBrace,
            _ => Self::Null,
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Null => write!(f, "none"),
            Self::C => write!(f, "C"),
            Self::Java => write!(f, "Java"),
            Self::Python => write!(f, "Python"),
            Self::PythonBrace => write!(f, "Python brace"),
        }
    }
}

impl Language {
    pub(crate) fn format_parser(&self) -> Box<dyn FormatParser> {
        match self {
            Self::C => Box::new(FormatC),
            Self::Java => Box::new(FormatJava),
            Self::Python => Box::new(FormatPython),
            Self::PythonBrace => Box::new(FormatPythonBrace),
            Self::Null => Box::new(FormatNull),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language() {
        assert_eq!(Language::from("c"), Language::C);
        assert_eq!(Language::from("java"), Language::Java);
        assert_eq!(Language::from("python"), Language::Python);
        assert_eq!(Language::from("python-brace"), Language::PythonBrace);
        assert_eq!(Language::from(""), Language::Null);
        assert_eq!(Language::from("unknown"), Language::Null);
    }
}
