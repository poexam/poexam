// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Support of format strings in different languages.

use std::borrow::Cow;

use crate::po::format::language::Language;

pub mod iter;
pub mod lang_c;
pub mod lang_null;
pub mod lang_python;
pub mod language;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchFmtPos<'a> {
    pub s: &'a str,
    pub start: usize,
    pub end: usize,
}

pub trait FormatParser {
    /// Return next char, new position after the char, and a boolean which is true if
    /// the char is the start of a format string.
    fn next_char(&self, _s: &str, _pos: usize) -> Option<(char, usize, bool)>;

    /// Find the position of the end of the format string starting at `pos`, which is
    /// after the format char found.
    ///
    /// Return the index of the character after the end of the format string.
    fn find_end_format(&self, _s: &str, _pos: usize, len: usize) -> usize;
}

/// Strip format strings from a string, according to the given language.
pub fn strip_formats<'a>(s: &'a str, language: &Language) -> Cow<'a, str> {
    if language == &Language::Null {
        // No format strings: return the original string.
        Cow::Borrowed(s)
    } else {
        let len_s = s.len();
        let mut result = String::with_capacity(len_s);
        let mut pos = 0;
        let fmt = language.format_parser();
        while let Some((c, new_pos, is_format)) = fmt.next_char(s, pos) {
            if is_format {
                pos = fmt.find_end_format(s, new_pos, len_s);
            } else {
                result.push(c);
                pos = new_pos;
            }
        }
        Cow::Owned(result)
    }
}
