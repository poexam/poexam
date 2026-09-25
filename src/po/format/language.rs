// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Supported languages for format strings.

use serde::Serialize;

use crate::po::format::{
    FormatParser,
    lang_c::FormatC,
    lang_java::FormatJava,
    lang_javascript::FormatJavaScript,
    lang_null::FormatNull,
    lang_php::FormatPhp,
    lang_python::{FormatPython, FormatPythonBrace},
    lang_sh::FormatSh,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum Language {
    #[default]
    Null,
    C,
    Java,
    JavaScript,
    Php,
    Python,
    PythonBrace,
    Sh,
}

impl From<&str> for Language {
    fn from(language: &str) -> Self {
        match language {
            "c" => Self::C,
            "java" => Self::Java,
            "javascript" => Self::JavaScript,
            "php" => Self::Php,
            "python" => Self::Python,
            "python-brace" => Self::PythonBrace,
            "sh" => Self::Sh,
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
            Self::JavaScript => write!(f, "JavaScript"),
            Self::Php => write!(f, "PHP"),
            Self::Python => write!(f, "Python"),
            Self::PythonBrace => write!(f, "Python brace"),
            Self::Sh => write!(f, "Shell"),
        }
    }
}

impl FormatParser for Language {
    #[inline]
    fn next_char(&self, s: &str, pos: usize) -> Option<(char, usize, bool)> {
        match self {
            Self::C => FormatC.next_char(s, pos),
            Self::Java => FormatJava.next_char(s, pos),
            Self::JavaScript => FormatJavaScript.next_char(s, pos),
            Self::Php => FormatPhp.next_char(s, pos),
            Self::Python => FormatPython.next_char(s, pos),
            Self::PythonBrace => FormatPythonBrace.next_char(s, pos),
            Self::Sh => FormatSh.next_char(s, pos),
            Self::Null => FormatNull.next_char(s, pos),
        }
    }

    #[inline]
    fn find_end_format(&self, s: &str, pos: usize, len: usize) -> usize {
        match self {
            Self::C => FormatC.find_end_format(s, pos, len),
            Self::Java => FormatJava.find_end_format(s, pos, len),
            Self::JavaScript => FormatJavaScript.find_end_format(s, pos, len),
            Self::Php => FormatPhp.find_end_format(s, pos, len),
            Self::Python => FormatPython.find_end_format(s, pos, len),
            Self::PythonBrace => FormatPythonBrace.find_end_format(s, pos, len),
            Self::Sh => FormatSh.find_end_format(s, pos, len),
            Self::Null => FormatNull.find_end_format(s, pos, len),
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
        assert_eq!(Language::from("javascript"), Language::JavaScript);
        assert_eq!(Language::from("php"), Language::Php);
        assert_eq!(Language::from("python"), Language::Python);
        assert_eq!(Language::from("python-brace"), Language::PythonBrace);
        assert_eq!(Language::from("sh"), Language::Sh);
        assert_eq!(Language::from(""), Language::Null);
        assert_eq!(Language::from("unknown"), Language::Null);
    }
}
