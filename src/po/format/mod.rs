// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Support of format strings in different languages.

pub mod char_pos;
pub mod format_pos;
pub mod lang_c;
pub mod lang_null;
pub mod lang_python;
pub mod language;
pub mod word_pos;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchStrPos<'a> {
    pub s: &'a str,
    pub start: usize,
    pub end: usize,
}

pub trait FormatParser {
    /// Return position of next char to read and a boolean which is true if the start of
    /// a format string has been detected.
    fn next_char(&self, _s: &str, _pos: usize, _len: usize) -> (usize, bool);

    /// Find the position of the end of the format string starting at `pos` (the index
    /// returned is the character after the end of the format string).
    fn find_end_format(&self, _s: &str, _pos: usize, len: usize) -> usize;
}
