// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Support of format strings in different languages.

pub mod iterators;
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
