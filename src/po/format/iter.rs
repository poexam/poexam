// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format iterator: return format strings.

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::po::format::{FormatParser, MatchFmtPos, language::Language};
use crate::rules::double_quotes::DOUBLE_QUOTES;

pub struct FormatPos<'a> {
    s: &'a str,
    len: usize,
    pos: usize,
    fmt: Language,
}

impl<'a> FormatPos<'a> {
    pub fn new(s: &'a str, language: Language) -> Self {
        Self {
            s,
            len: s.len(),
            pos: 0,
            fmt: language,
        }
    }
}

/// Iterator returning format strings of a string, according to the given language.
///
/// For example in C language, with the string `Hello, %d %s world!`, it will return
/// `%d` and `%s` with their positions in the string.
impl<'a> Iterator for FormatPos<'a> {
    type Item = MatchFmtPos<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((_, new_pos, is_format)) = self.fmt.next_char(self.s, self.pos) {
            if is_format {
                let start = self.pos;
                self.pos = self.fmt.find_end_format(self.s, new_pos, self.len);
                return Some(MatchFmtPos {
                    s: &self.s[start..self.pos],
                    start,
                    end: self.pos,
                });
            }
            self.pos = new_pos;
        }
        None
    }
}

pub struct FormatWordPos<'a> {
    s: &'a str,
    len: usize,
    pos: usize,
    fmt: Language,
}

impl<'a> FormatWordPos<'a> {
    pub fn new(s: &'a str, language: Language) -> Self {
        Self {
            s,
            len: s.len(),
            pos: 0,
            fmt: language,
        }
    }
}

/// Iterator returning words of a string, according to the given language, skipping
/// format strings.
///
/// For example in C language, with the string `Hello, %d %s world!`, it will return
/// `Hello` and `world` with their positions in the string.
impl<'a> Iterator for FormatWordPos<'a> {
    type Item = MatchFmtPos<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut idx_start = None;
        let mut idx_end = None;
        let mut start_apostrophe = false;

        while let Some((c, new_pos, is_format)) = self.fmt.next_char(self.s, self.pos) {
            if is_format {
                if idx_start.is_some() {
                    break;
                }
                self.pos = self.fmt.find_end_format(self.s, new_pos, self.len);
                continue;
            }
            if idx_start.is_none() && c == '\'' {
                start_apostrophe = true;
            }
            if c.is_alphanumeric()
                || (idx_start.is_some() && (c == '-' || c == '\'' || c == '’') || (c == 'ʼ'))
            {
                if idx_start.is_none() {
                    idx_start = Some(self.pos);
                }
                idx_end = Some(new_pos);
                self.pos = new_pos;
            } else if idx_start.is_some() {
                break;
            } else {
                self.pos = new_pos;
            }
        }
        match (idx_start, idx_end) {
            (Some(start), Some(end)) => {
                let s = &self.s[start..end];
                if start_apostrophe && let Some(s2) = s.strip_suffix('\'') {
                    Some(MatchFmtPos {
                        s: s2,
                        start,
                        end: end - 1,
                    })
                } else {
                    Some(MatchFmtPos { s, start, end })
                }
            }
            _ => None,
        }
    }
}

pub struct FormatAcronymPos<'a> {
    s: &'a str,
    len: usize,
    pos: usize,
    fmt: Language,
}

impl<'a> FormatAcronymPos<'a> {
    pub fn new(s: &'a str, language: Language) -> Self {
        Self {
            s,
            len: s.len(),
            pos: 0,
            fmt: language,
        }
    }
}

/// Iterator returning acronyms of a string, according to the given language,
/// skipping format strings.
///
/// An acronym is a word of length ≥ 2 chars whose Python-equivalent
/// `str.isupper()` returns true: at least one cased character is present and
/// none is lowercase. Caseless characters (digits, etc.) are allowed.
///
/// Words are alphanumeric runs (boundaries: any non-alphanumeric character
/// including apostrophes, hyphens, spaces and punctuation), so `API` in
/// `l'API` is recognized as well as `MP3` and `B2B`. `URLs` and `Json` are
/// not acronyms (they contain a lowercase letter).
///
/// For example with the string `Use the HTTP API for %s and l'API`, it will
/// return `HTTP`, `API` and `API` with their positions in the string.
impl<'a> Iterator for FormatAcronymPos<'a> {
    type Item = MatchFmtPos<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let mut idx_start = None;
            let mut idx_end = None;
            let mut has_upper = false;
            let mut has_lower = false;
            while let Some((c, new_pos, is_format)) = self.fmt.next_char(self.s, self.pos) {
                if is_format {
                    if idx_start.is_some() {
                        break;
                    }
                    self.pos = self.fmt.find_end_format(self.s, new_pos, self.len);
                    continue;
                }
                if c.is_alphanumeric() {
                    if idx_start.is_none() {
                        idx_start = Some(self.pos);
                        has_upper = false;
                        has_lower = false;
                    }
                    if c.is_uppercase() {
                        has_upper = true;
                    } else if c.is_lowercase() {
                        has_lower = true;
                    }
                    idx_end = Some(new_pos);
                    self.pos = new_pos;
                } else if idx_start.is_some() {
                    break;
                } else {
                    self.pos = new_pos;
                }
            }
            match (idx_start, idx_end) {
                (Some(start), Some(end)) => {
                    let word = &self.s[start..end];
                    if has_upper && !has_lower && word.chars().count() >= 2 {
                        return Some(MatchFmtPos {
                            s: word,
                            start,
                            end,
                        });
                    }
                    // Not an all-uppercase word, or too short — keep scanning.
                }
                _ => return None,
            }
        }
    }
}

pub struct FormatAcceleratorPos<'a> {
    s: &'a str,
    len: usize,
    pos: usize,
    fmt: Language,
    marker: char,
}

impl<'a> FormatAcceleratorPos<'a> {
    pub fn new(s: &'a str, language: Language, marker: char) -> Self {
        Self {
            s,
            len: s.len(),
            pos: 0,
            fmt: language,
            marker,
        }
    }
}

/// Iterator returning keyboard accelerator markers of a string, according to the
/// given language, skipping format strings.
///
/// An accelerator is the `marker` character (e.g. `&`) immediately followed by an
/// alphanumeric character. A doubled marker (e.g. `&&`) is an escaped literal and
/// is not an accelerator: both characters are skipped. A trailing marker, or a
/// marker followed by whitespace or punctuation, is treated as a literal and
/// ignored (this avoids false positives on prose such as "Drag & drop"). Only the
/// marker character is returned, not the accelerated character, so its span is
/// always one character wide.
///
/// For example with marker `&` and the string `&File and E&xit`, it will return
/// the `&` at positions 0 and 11.
impl<'a> Iterator for FormatAcceleratorPos<'a> {
    type Item = MatchFmtPos<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((c, new_pos, is_format)) = self.fmt.next_char(self.s, self.pos) {
            if is_format {
                self.pos = self.fmt.find_end_format(self.s, new_pos, self.len);
                continue;
            }
            if c == self.marker {
                let start = self.pos;
                match self.s[new_pos..].chars().next() {
                    // Doubled marker is an escaped literal: skip both characters.
                    Some(next) if next == self.marker => {
                        self.pos = new_pos + self.marker.len_utf8();
                    }
                    // Marker before an alphanumeric character: an accelerator.
                    Some(next) if next.is_alphanumeric() => {
                        self.pos = new_pos;
                        return Some(MatchFmtPos {
                            s: &self.s[start..new_pos],
                            start,
                            end: new_pos,
                        });
                    }
                    // Trailing marker, or marker before whitespace/punctuation: literal.
                    _ => self.pos = new_pos,
                }
                continue;
            }
            self.pos = new_pos;
        }
        None
    }
}

pub struct FormatUrlPos<'a> {
    s: &'a str,
    len: usize,
    pos: usize,
    fmt: Language,
}

impl<'a> FormatUrlPos<'a> {
    pub fn new(s: &'a str, language: Language) -> Self {
        Self {
            s,
            len: s.len(),
            pos: 0,
            fmt: language,
        }
    }
}

/// Iterator returning URLs of a string, according to the given language, skipping
/// format strings.
///
/// For example in C language, with the string `Hello, %d %s world! https://example.com`,
/// it will return `https://example.com` with its position in the string.
///
/// Angle brackets around URLs are handled, e.g. `Hello, %d %s world! <https://example.com>`
/// (the angle brackets are not included in the returned URL).
impl<'a> Iterator for FormatUrlPos<'a> {
    type Item = MatchFmtPos<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut idx_start = None;
        let mut idx_end = None;
        loop {
            while let Some((c, new_pos, is_format)) = self.fmt.next_char(self.s, self.pos) {
                if is_format {
                    self.pos = self.fmt.find_end_format(self.s, new_pos, self.len);
                    continue;
                }
                if !c.is_whitespace() {
                    if idx_start.is_none() {
                        idx_start = Some(self.pos);
                    }
                    idx_end = Some(new_pos);
                    self.pos = new_pos;
                } else if idx_start.is_some() {
                    break;
                } else {
                    self.pos = new_pos;
                }
            }
            match (idx_start, idx_end) {
                (Some(mut start), Some(mut end)) => {
                    let mut s = &self.s[start..end];
                    if s.starts_with('<') && s.ends_with('>') {
                        s = &s[1..s.len() - 1];
                        start += 1;
                        end -= 1;
                    }
                    if s.contains("://") && s.contains('.') {
                        return Some(MatchFmtPos { s, start, end });
                    }
                    idx_start = None;
                    idx_end = None;
                }
                _ => return None,
            }
        }
    }
}

pub struct FormatEmailPos<'a> {
    s: &'a str,
    len: usize,
    pos: usize,
    fmt: Language,
}

impl<'a> FormatEmailPos<'a> {
    pub fn new(s: &'a str, language: Language) -> Self {
        Self {
            s,
            len: s.len(),
            pos: 0,
            fmt: language,
        }
    }

    /// Simple check for email validity: check that it contains exactly one '@' and that
    /// local and domain parts are not empty and contain only allowed characters, with
    /// relaxed rules (e.g. allow language formats like `%s` or `{0}`).
    ///
    /// Quotes are not allowed here: a single pair of surrounding quotes is stripped by
    /// the iterator before validation (see [`Iterator::next`]), so a token like `"@".`
    /// is correctly rejected instead of being seen as the email `"@".`.
    fn is_valid_email(email: &str) -> bool {
        email.find('@').is_some_and(|pos_arobase| {
            let local = &email[..pos_arobase];
            let domain = &email[pos_arobase + 1..];
            !local.is_empty()
                && !domain.is_empty()
                && local.chars().all(|c| {
                    c.is_alphanumeric()
                        || c == '.'
                        || c == '-'
                        || c == '_'
                        || c == '+'
                        || c == '%'
                        || c == '{'
                        || c == '}'
                        || c == '$'
                })
                && domain.chars().all(|c| {
                    c.is_alphanumeric()
                        || c == '.'
                        || c == '-'
                        || c == '%'
                        || c == '{'
                        || c == '}'
                        || c == '$'
                })
                && domain.contains('.')
        })
    }
}

/// Iterator returning emails of a string, according to the given language, skipping
/// format strings.
///
/// For example in C language, with the string `Please send email to: user@example.com`,
/// it will return `user@example.com` with its position in the string.
///
/// Angle brackets around emails are handled, e.g. `Please send email to: <user@example.com>`
/// (the angle brackets are not included in the returned email).
///
/// A single pair of surrounding quotes is handled the same way, e.g. `"user@example.com"`
/// or `„user@example.com”` (the quotes are not included in the returned email).
impl<'a> Iterator for FormatEmailPos<'a> {
    type Item = MatchFmtPos<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut idx_start = None;
        let mut idx_end = None;
        loop {
            while let Some((c, new_pos, is_format)) = self.fmt.next_char(self.s, self.pos) {
                if is_format {
                    self.pos = self.fmt.find_end_format(self.s, new_pos, self.len);
                    continue;
                }
                if !c.is_whitespace() {
                    if idx_start.is_none() {
                        idx_start = Some(self.pos);
                    }
                    idx_end = Some(new_pos);
                    self.pos = new_pos;
                } else if idx_start.is_some() {
                    break;
                } else {
                    self.pos = new_pos;
                }
            }
            match (idx_start, idx_end) {
                (Some(mut start), Some(mut end)) => {
                    let mut s = &self.s[start..end];
                    // Strip one pair of surrounding angle brackets, e.g. <user@example.com>.
                    if s.starts_with('<') && s.ends_with('>') {
                        s = &s[1..s.len() - 1];
                        start += 1;
                        end -= 1;
                    }
                    // Strip one pair of surrounding quotes, e.g. "user@example.com" or
                    // „user@example.com”, so the quotes are not treated as email characters.
                    let mut chars = s.char_indices();
                    if let (Some((_, first)), Some((last_idx, last))) =
                        (chars.next(), chars.next_back())
                        && DOUBLE_QUOTES.contains(&first)
                        && DOUBLE_QUOTES.contains(&last)
                    {
                        start += first.len_utf8();
                        end -= last.len_utf8();
                        s = &s[first.len_utf8()..last_idx];
                    }
                    if Self::is_valid_email(s) {
                        return Some(MatchFmtPos { s, start, end });
                    }
                    idx_start = None;
                    idx_end = None;
                }
                _ => return None,
            }
        }
    }
}

pub struct FormatPathPos<'a> {
    s: &'a str,
    len: usize,
    pos: usize,
    fmt: Language,
}

impl<'a> FormatPathPos<'a> {
    pub fn new(s: &'a str, language: Language) -> Self {
        Self {
            s,
            len: s.len(),
            pos: 0,
            fmt: language,
        }
    }

    /// Check if a string is a path: it starts with '/' or './' or '../' or '~/'.
    fn is_path(path: &str) -> bool {
        if path.starts_with("./") || path.starts_with("../") || path.starts_with("~/") {
            return true;
        }
        if path.starts_with('/')
            && let Some(pos) = path[1..].find('/')
            && pos > 0
            && !path[pos + 2..].is_empty()
        {
            return true;
        }
        false
    }
}

/// Iterator returning paths of a string, according to the given language, skipping
/// format strings.
///
/// For example in C language, with the string `Hello, %d %s world! /tmp/output.txt`,
/// it will return `/tmp/output.txt` with its position in the string.
impl<'a> Iterator for FormatPathPos<'a> {
    type Item = MatchFmtPos<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut idx_start = None;
        let mut idx_end = None;
        loop {
            while let Some((c, new_pos, is_format)) = self.fmt.next_char(self.s, self.pos) {
                if is_format {
                    self.pos = self.fmt.find_end_format(self.s, new_pos, self.len);
                    continue;
                }
                if !c.is_whitespace() {
                    if idx_start.is_none() {
                        idx_start = Some(self.pos);
                    }
                    idx_end = Some(new_pos);
                    self.pos = new_pos;
                } else if idx_start.is_some() {
                    break;
                } else {
                    self.pos = new_pos;
                }
            }
            match (idx_start, idx_end) {
                (Some(start), Some(end)) => {
                    let s = &self.s[start..end];
                    if Self::is_path(s) {
                        return Some(MatchFmtPos { s, start, end });
                    }
                    idx_start = None;
                    idx_end = None;
                }
                _ => return None,
            }
        }
    }
}

pub struct FormatHtmlTagPos<'a> {
    s: &'a str,
    len: usize,
    pos: usize,
    fmt: Language,
}

impl<'a> FormatHtmlTagPos<'a> {
    pub fn new(s: &'a str, language: Language) -> Self {
        Self {
            s,
            len: s.len(),
            pos: 0,
            fmt: language,
        }
    }
}

/// Iterator returning HTML tags of a string, according to the given language, skipping
/// format strings.
///
/// For example with the string `Hello <b>world</b>`, it will return
/// `<b>` and `</b>` with their positions in the string.
///
/// Tags with attributes are also matched, e.g. `<a href="...">`.
/// Quoted attribute values (double or single quotes) are handled so that
/// a `>` inside quotes does not end the tag prematurely.
impl<'a> Iterator for FormatHtmlTagPos<'a> {
    type Item = MatchFmtPos<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((c, new_pos, is_format)) = self.fmt.next_char(self.s, self.pos) {
            if is_format {
                self.pos = self.fmt.find_end_format(self.s, new_pos, self.len);
                continue;
            }
            if c == '<' {
                // Check the next character is a letter or '/' (tag start).
                let tag_start = self.pos;
                if let Some(next_ch) = self.s[new_pos..].chars().next()
                    && (next_ch.is_ascii_alphabetic() || next_ch == '/')
                    && let Some(tag_end) = self.find_tag_end(new_pos)
                {
                    self.pos = tag_end;
                    return Some(MatchFmtPos {
                        s: &self.s[tag_start..tag_end],
                        start: tag_start,
                        end: tag_end,
                    });
                }
            }
            self.pos = new_pos;
        }
        None
    }
}

impl FormatHtmlTagPos<'_> {
    /// Find the end of an HTML tag starting after `<`, handling quoted attribute values.
    /// Returns the byte position after the closing `>`, or `None` if not found.
    fn find_tag_end(&self, start: usize) -> Option<usize> {
        let mut pos = start;
        while pos < self.len {
            let c = self.s.as_bytes()[pos];
            match c {
                b'>' => return Some(pos + 1),
                b'"' | b'\'' => {
                    // Skip quoted attribute value.
                    pos += 1;
                    while pos < self.len && self.s.as_bytes()[pos] != c {
                        pos += 1;
                    }
                    if pos < self.len {
                        // Skip closing quote.
                        pos += 1;
                    }
                }
                _ => pos += 1,
            }
        }
        None
    }
}

pub struct FormatFunctionPos<'a> {
    s: &'a str,
    len: usize,
    pos: usize,
    fmt: Language,
}

impl<'a> FormatFunctionPos<'a> {
    pub fn new(s: &'a str, language: Language) -> Self {
        Self {
            s,
            len: s.len(),
            pos: 0,
            fmt: language,
        }
    }
}

/// Iterator returning function calls of a string, according to the given language,
/// skipping format strings.
///
/// A function call is a name (ASCII word characters and dots), optionally with
/// `::` or `->` separators followed by more name parts, ending with `()`.
///
/// For example with the string `Use foo() and bar.baz() and Class::method()`,
/// it will return `foo()`, `bar.baz()` and `Class::method()` with their
/// positions in the string.
impl<'a> Iterator for FormatFunctionPos<'a> {
    type Item = MatchFmtPos<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        'outer: loop {
            // Find the start: the first ASCII word character (alphanumeric or `_`).
            // Skip format strings and any other characters.
            let start;
            loop {
                let (c, new_pos, is_format) = self.fmt.next_char(self.s, self.pos)?;
                if is_format {
                    self.pos = self.fmt.find_end_format(self.s, new_pos, self.len);
                    continue;
                }
                if c.is_ascii_alphanumeric() || c == '_' {
                    start = self.pos;
                    self.pos = new_pos;
                    break;
                }
                self.pos = new_pos;
            }

            // Walk forward, accepting name characters (`\w`, `.`), separators (`::`, `->`)
            // and format strings (transparent: included in the match span but not in the
            // syntactic name). Stop at `()` (success) or any other character (failure).
            loop {
                let bytes = self.s.as_bytes();
                // Try separator `::` or `->`.
                if self.pos + 2 <= self.len {
                    let two = &bytes[self.pos..self.pos + 2];
                    if two == b"::" || two == b"->" {
                        self.pos += 2;
                        continue;
                    }
                }
                // Try `()`.
                if self.pos + 2 <= self.len
                    && bytes[self.pos] == b'('
                    && bytes[self.pos + 1] == b')'
                {
                    let end = self.pos + 2;
                    self.pos = end;
                    return Some(MatchFmtPos {
                        s: &self.s[start..end],
                        start,
                        end,
                    });
                }
                // Get the next char (may be the start of a format string).
                let Some((c, new_pos, is_format)) = self.fmt.next_char(self.s, self.pos) else {
                    // End of string reached without finding `()`.
                    return None;
                };
                if is_format {
                    self.pos = self.fmt.find_end_format(self.s, new_pos, self.len);
                    continue;
                }
                if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
                    self.pos = new_pos;
                    continue;
                }
                // Not a valid character in a function name: abandon the current attempt
                // and resume scanning for a new start from the same position.
                continue 'outer;
            }
        }
    }
}

/// Zero code points of the Unicode decimal digit blocks recognized as digits.
///
/// Each block is a contiguous run of ten code points, so a character `c` in the range
/// `zero..zero + 10` is the digit `c - zero`.
const DIGIT_ZEROS: [char; 20] = [
    '\u{0660}', // Arabic-Indic
    '\u{06F0}', // Extended Arabic-Indic (Persian, Urdu)
    '\u{07C0}', // NKo
    '\u{0966}', // Devanagari
    '\u{09E6}', // Bengali
    '\u{0A66}', // Gurmukhi
    '\u{0AE6}', // Gujarati
    '\u{0B66}', // Oriya
    '\u{0BE6}', // Tamil
    '\u{0C66}', // Telugu
    '\u{0CE6}', // Kannada
    '\u{0D66}', // Malayalam
    '\u{0DE6}', // Sinhala
    '\u{0E50}', // Thai
    '\u{0ED0}', // Lao
    '\u{0F20}', // Tibetan
    '\u{1040}', // Myanmar
    '\u{17E0}', // Khmer
    '\u{1810}', // Mongolian
    '\u{FF10}', // Fullwidth
];

/// Separators that can be either a digit-group separator or a decimal separator:
/// they continue a number token before any digit.
const DECIMAL_SEPARATORS: [char; 3] = [
    '.', ',', '\u{066B}', // Arabic decimal separator
];

/// Separators that are only ever digit-group separators: they continue a number token
/// before a group of exactly three digits.
const GROUP_SEPARATORS: [char; 7] = [
    ' ', '\u{00A0}', // no-break space
    '\u{202F}', // narrow no-break space
    '\u{2009}', // thin space
    '\u{2007}', // figure space
    '\u{066C}', // Arabic thousands separator
    '\'',       // Swiss group separator
];

/// Return the ASCII equivalent of a decimal digit character, or `None` if the character
/// is not a decimal digit.
///
/// ASCII digits are returned as-is, digits of the scripts listed in [`DIGIT_ZEROS`] are
/// mapped to their ASCII equivalent: `٣` (Arabic-Indic) and `३` (Devanagari) both return
/// `'3'`.
#[inline]
pub fn ascii_digit(c: char) -> Option<char> {
    if c.is_ascii_digit() {
        return Some(c);
    }
    if !c.is_numeric() {
        return None;
    }
    DIGIT_ZEROS
        .iter()
        .find_map(|zero| char::from_digit(u32::from(c).wrapping_sub(u32::from(*zero)), 10))
}

pub struct FormatNumberPos<'a> {
    s: &'a str,
    len: usize,
    pos: usize,
    fmt: Language,
    prev_alnum: bool,
    /// Read a letter run directly after the digits as a unit (`8MB`) instead of as the
    /// rest of an identifier (`3D`).
    units: bool,
}

impl<'a> FormatNumberPos<'a> {
    /// Iterate over the numbers of `s`, skipping the digits glued to a letter, which are
    /// part of an identifier: `MP3`, `3D` and `1st` yield nothing.
    pub fn new(s: &'a str, language: Language) -> Self {
        Self::with_options(s, language, false)
    }

    /// Iterate over the numbers of `s`, including the ones directly followed by a letter
    /// run, which is read as a unit: `8MB` yields `8`.
    ///
    /// Only the letters *after* the digits are read this way, so `MP3` still yields
    /// nothing. Used by the `numbers` rule to tell a detached unit (`8MB` translated
    /// `8 Mo`) from a lost value.
    pub fn with_units(s: &'a str, language: Language) -> Self {
        Self::with_options(s, language, true)
    }

    fn with_options(s: &'a str, language: Language, units: bool) -> Self {
        Self {
            s,
            len: s.len(),
            pos: 0,
            fmt: language,
            prev_alnum: false,
            units,
        }
    }

    /// Return the position just after the run of decimal digits starting at `pos`,
    /// which is `pos` itself when there is no digit there.
    fn digits_end(&self, pos: usize) -> usize {
        let mut pos = pos;
        while let Some((c, new_pos, is_format)) = self.fmt.next_char(self.s, pos) {
            if is_format || ascii_digit(c).is_none() {
                break;
            }
            pos = new_pos;
        }
        pos
    }

    /// Return the position just after the number token starting at `start`, which must
    /// be the position of a decimal digit.
    ///
    /// A token is a run of digits, continued by a [decimal separator](DECIMAL_SEPARATORS)
    /// before any digit, or by a [group separator](GROUP_SEPARATORS) before a group of
    /// exactly three digits.
    fn number_end(&self, start: usize) -> usize {
        let mut end = self.digits_end(start);
        while let Some((sep, after_sep, is_format)) = self.fmt.next_char(self.s, end) {
            if is_format {
                break;
            }
            let group_end = self.digits_end(after_sep);
            let group_len = self.s[after_sep..group_end].chars().count();
            let is_separator = if DECIMAL_SEPARATORS.contains(&sep) {
                group_len > 0
            } else if GROUP_SEPARATORS.contains(&sep) {
                group_len == 3
            } else {
                false
            };
            if !is_separator {
                break;
            }
            end = group_end;
        }
        end
    }
}

/// Iterator returning numbers of a string, according to the given language, skipping
/// format strings.
///
/// A number is a run of decimal digits, possibly including digit-group and decimal
/// separators, e.g. `1,000.5`, `1 000,5` or `1.000,5` (see [`Self::number_end`]).
/// Digits of any script are recognized, e.g. `٣` (Arabic-Indic) and `३` (Devanagari).
///
/// A number glued to a letter is part of an identifier rather than a value, so it is
/// not returned: `MP3`, `3D` and `1st` yield nothing. A letter run *after* the digits is
/// read as a unit instead when the iterator is built with [`with_units`](Self::with_units),
/// so that `8MB` yields `8`.
///
/// For example in C language, with the string `Press %d of 3 times`, it will return
/// `3` with its position in the string (the `3` of a format string such as `%3$d` is
/// skipped along with the format string).
impl<'a> Iterator for FormatNumberPos<'a> {
    type Item = MatchFmtPos<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((c, new_pos, is_format)) = self.fmt.next_char(self.s, self.pos) {
            if is_format {
                self.pos = self.fmt.find_end_format(self.s, new_pos, self.len);
                self.prev_alnum = false;
                continue;
            }
            if ascii_digit(c).is_none() {
                self.prev_alnum = c.is_alphanumeric();
                self.pos = new_pos;
                continue;
            }
            let start = self.pos;
            let after_alnum = self.prev_alnum;
            let end = self.number_end(start);
            self.pos = end;
            self.prev_alnum = true;
            let before_letter = !self.units
                && self.s[end..]
                    .chars()
                    .next()
                    .is_some_and(char::is_alphabetic);
            if after_alnum || before_letter {
                // Part of an identifier ("MP3", "3D", "1st"), not a value.
                continue;
            }
            return Some(MatchFmtPos {
                s: &self.s[start..end],
                start,
                end,
            });
        }
        None
    }
}

/// Syntax of a variable recognized by [`FormatVariablePos`], and value accepted by the
/// `variable_styles` configuration key of the `variables` rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum VariableStyle {
    /// `$VAR`: shell, systemd units, config templates.
    Dollar,
    /// `${VAR}`: shell, systemd units, config templates.
    DollarBrace,
    /// `%VAR%`: Windows-style, Qt, some web frameworks.
    Percent,
    /// `@VAR@`: autotools, Transifex.
    At,
    /// `{{var}}`: Mustache, Handlebars, Jinja, JavaScript frameworks.
    DoubleBrace,
    /// `{var}`: .NET and JavaScript template literals.
    Brace,
}

/// Return the character at `pos` and the position just after it, or `None` at the end of
/// the string or when a format string starts at `pos`.
///
/// Used by the iterators that look ahead one character at a time while leaving format
/// strings out of the way.
fn plain_char_at(s: &str, fmt: Language, pos: usize) -> Option<(char, usize)> {
    match fmt.next_char(s, pos) {
        Some((c, new_pos, false)) => Some((c, new_pos)),
        _ => None,
    }
}

/// Characters allowed in a variable name: ASCII alphanumeric and underscore.
///
/// Deliberately narrow: allowing `.` or `-` would make `%d.%d` and `50%-60%` look like
/// `%VAR%` variables.
#[inline]
fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

pub struct FormatVariablePos<'a> {
    s: &'a str,
    len: usize,
    pos: usize,
    fmt: Language,
    styles: &'a [VariableStyle],
    prev: Option<char>,
}

impl<'a> FormatVariablePos<'a> {
    pub fn new(s: &'a str, language: Language, styles: &'a [VariableStyle]) -> Self {
        Self {
            s,
            len: s.len(),
            pos: 0,
            fmt: language,
            styles,
            prev: None,
        }
    }

    /// Whether the given variable style is enabled.
    fn enabled(&self, style: VariableStyle) -> bool {
        self.styles.contains(&style)
    }

    /// Return the character at `pos` and the position just after it, or `None` at the end
    /// of the string or when a format string starts at `pos`.
    fn char_at(&self, pos: usize) -> Option<(char, usize)> {
        plain_char_at(self.s, self.fmt, pos)
    }

    /// Return the position just after the run of [name characters](is_name_char) starting
    /// at `pos`, which is `pos` itself when there is no name character there.
    fn name_end(&self, pos: usize) -> usize {
        let mut pos = pos;
        while let Some((c, new_pos)) = self.char_at(pos) {
            if !is_name_char(c) {
                break;
            }
            pos = new_pos;
        }
        pos
    }

    /// Return the position just after a non-empty variable name starting at `pos` and
    /// terminated by `close`, or `None` when the name is empty or not terminated.
    fn closed_name_end(&self, pos: usize, close: char) -> Option<usize> {
        let end = self.name_end(pos);
        if end == pos {
            return None;
        }
        match self.char_at(end) {
            Some((c, new_pos)) if c == close => Some(new_pos),
            _ => None,
        }
    }

    /// Return the position just after a `%NAME%` or `@NAME@` variable whose name starts at
    /// `pos`, or `None` when there is no such variable there.
    ///
    /// The closing delimiter must not be glued to a name character, so that
    /// `@user@instance` is a social handle rather than the variable `@user@`. The caller
    /// applies the same rule to the opening delimiter, which rules out `user@example.com`.
    fn paired_end(&self, pos: usize, delimiter: char) -> Option<usize> {
        let end = self.closed_name_end(pos, delimiter)?;
        match self.char_at(end) {
            Some((c, _)) if is_name_char(c) => None,
            _ => Some(end),
        }
    }

    /// Return the position just after a `$NAME` or `${NAME}` variable opened by the `$`
    /// ending at `pos`, or `None` when no enabled style matches there.
    fn dollar_end(&self, pos: usize) -> Option<usize> {
        let (c, after) = self.char_at(pos)?;
        if c == '{' {
            if self.enabled(VariableStyle::DollarBrace) {
                return self.closed_name_end(after, '}');
            }
            return None;
        }
        // The name of a bare `$` variable must start with a letter or an underscore: `$5`
        // and `$1,000` are amounts of money, not variables.
        if self.enabled(VariableStyle::Dollar) && (c.is_ascii_alphabetic() || c == '_') {
            return Some(self.name_end(pos));
        }
        None
    }

    /// Return the position just after a `{{…}}` or `{NAME}` variable opened by the `{`
    /// ending at `pos`, or `None` when no enabled style matches there.
    fn brace_end(&self, pos: usize) -> Option<usize> {
        if let Some(('{', after)) = self.char_at(pos) {
            // A doubled `{` opens a `{{…}}` variable, never a `{NAME}` one: with only the
            // `brace` style enabled, `{{name}}` must yield nothing rather than `{name}`.
            if self.enabled(VariableStyle::DoubleBrace) {
                return self.double_brace_end(after);
            }
            return None;
        }
        if self.enabled(VariableStyle::Brace) {
            return self.closed_name_end(pos, '}');
        }
        None
    }

    /// Return the position just after a `{{…}}` variable whose content starts at `pos`, or
    /// `None` when the content is blank, holds a brace or a control character, or is not
    /// terminated by `}}`.
    ///
    /// The content is not restricted to a [name](is_name_char): the template engines using
    /// this syntax accept expressions, e.g. `{{ user.name }}` or `{{ count|plural }}`.
    fn double_brace_end(&self, pos: usize) -> Option<usize> {
        let mut end = pos;
        while let Some((c, new_pos)) = self.char_at(end) {
            if c == '{' || c.is_control() {
                return None;
            }
            if c == '}' {
                if self.s[pos..end].trim().is_empty() {
                    return None;
                }
                return match self.char_at(new_pos) {
                    Some(('}', after)) => Some(after),
                    _ => None,
                };
            }
            end = new_pos;
        }
        None
    }
}

/// Iterator returning variables of a string, according to the given language and the
/// enabled [styles](VariableStyle), skipping format strings.
///
/// A variable is a placeholder substituted at runtime by a templating syntax that gettext
/// knows nothing about: it carries no `*-format` flag, so the `formats` rule never sees it.
///
/// For example with the styles `dollar-brace` and `percent` and the string
/// `Copy ${SRC} to %DEST%`, it will return `${SRC}` and `%DEST%` with their positions in
/// the string.
impl<'a> Iterator for FormatVariablePos<'a> {
    type Item = MatchFmtPos<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((c, new_pos, is_format)) = self.fmt.next_char(self.s, self.pos) {
            if is_format {
                self.pos = self.fmt.find_end_format(self.s, new_pos, self.len);
                self.prev = None;
                continue;
            }
            let start = self.pos;
            let prev = self.prev;
            let glued = prev.is_some_and(is_name_char);
            self.pos = new_pos;
            self.prev = Some(c);
            // `%` and `@` are ordinary characters in prose, so they open a variable only at
            // a word boundary, unlike `$` and `{` which are distinctive enough on their own.
            let end = match c {
                '$' => self.dollar_end(new_pos),
                '%' if !glued && self.enabled(VariableStyle::Percent) => {
                    self.paired_end(new_pos, '%')
                }
                '@' if !glued && self.enabled(VariableStyle::At) => self.paired_end(new_pos, '@'),
                // The second `{` of a `{{…}}` opener was already handled by the first one.
                '{' if prev != Some('{') => self.brace_end(new_pos),
                _ => None,
            };
            if let Some(end) = end {
                self.pos = end;
                self.prev = self.s[..end].chars().next_back();
                return Some(MatchFmtPos {
                    s: &self.s[start..end],
                    start,
                    end,
                });
            }
        }
        None
    }
}

/// Characters allowed in a command-line option name: the [name characters](is_name_char)
/// plus the hyphen of `--no-color`.
#[inline]
fn is_option_char(c: char) -> bool {
    is_name_char(c) || c == '-'
}

pub struct FormatOptionPos<'a> {
    s: &'a str,
    len: usize,
    pos: usize,
    fmt: Language,
    prev: Option<char>,
}

impl<'a> FormatOptionPos<'a> {
    pub fn new(s: &'a str, language: Language) -> Self {
        Self {
            s,
            len: s.len(),
            pos: 0,
            fmt: language,
            prev: None,
        }
    }

    /// Return the character at `pos` and the position just after it, or `None` at the end
    /// of the string or when a format string starts at `pos`.
    fn char_at(&self, pos: usize) -> Option<(char, usize)> {
        plain_char_at(self.s, self.fmt, pos)
    }

    /// Return the position just after the option name starting at `pos`, or `None` when
    /// there is no name there.
    ///
    /// The name must start with an ASCII letter, which keeps `-5` a negative number and a
    /// dash surrounded by spaces a dash.
    fn name_end(&self, pos: usize) -> Option<usize> {
        let (c, mut end) = self.char_at(pos)?;
        if !c.is_ascii_alphabetic() {
            return None;
        }
        while let Some((c, new_pos)) = self.char_at(end) {
            if !is_option_char(c) {
                break;
            }
            end = new_pos;
        }
        Some(end)
    }
}

/// Iterator returning command-line options of a string, according to the given language,
/// skipping format strings.
///
/// An option is a `-` or `--` prefix at a word boundary followed by an ASCII letter, then
/// any number of ASCII alphanumeric characters, `-` and `_`: `-v`, `--verbose` and
/// `--no-color`. The value of `--opt=value` is not part of the option, so only `--opt` is
/// returned and the value stays translatable.
///
/// The word boundary and the leading letter are what keep prose out: the hyphen of
/// `well-known` and `café-restaurant` follows an alphanumeric character, while `-5` and a
/// dash surrounded by spaces are not followed by a letter.
///
/// For example with the string `Use -v or --log-level=debug`, it will return `-v` and
/// `--log-level` with their positions in the string.
impl<'a> Iterator for FormatOptionPos<'a> {
    type Item = MatchFmtPos<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((c, new_pos, is_format)) = self.fmt.next_char(self.s, self.pos) {
            if is_format {
                self.pos = self.fmt.find_end_format(self.s, new_pos, self.len);
                self.prev = None;
                continue;
            }
            let start = self.pos;
            // An option starts a word: a dash glued to the end of one is a compound word or
            // a range ("well-known", "café-restaurant", "UTF-8", "5-10").
            let boundary = self
                .prev
                .is_none_or(|p| !p.is_alphanumeric() && p != '_' && p != '-');
            self.pos = new_pos;
            self.prev = Some(c);
            if c != '-' || !boundary {
                continue;
            }
            // A second dash makes it a long option, anything else starts the name.
            let after_dashes = match self.char_at(new_pos) {
                Some(('-', after)) => after,
                _ => new_pos,
            };
            if let Some(end) = self.name_end(after_dashes) {
                self.pos = end;
                self.prev = self.s[..end].chars().next_back();
                return Some(MatchFmtPos {
                    s: &self.s[start..end],
                    start,
                    end,
                });
            }
        }
        None
    }
}

/// Kind of a Markdown construct returned by [`FormatMarkdownPos`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownKind {
    /// Code span, including its backticks: `` `code` ``.
    CodeSpan,
    /// Destination of an inline link or image: the `url` of `[text](url)`.
    Link,
    /// Emphasis delimiter, opening or closing: `*`, `**`, `***`, `_`, `__` or `___`.
    Emphasis,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchMarkdownPos<'a> {
    pub kind: MarkdownKind,
    pub s: &'a str,
    pub start: usize,
    pub end: usize,
}

/// A position the Markdown scan must step over when it reaches it: the closing delimiter
/// of an emphasis already returned (`emit` is true) or the `](url)` tail of a link whose
/// destination was already returned.
struct MarkdownPending {
    start: usize,
    end: usize,
    emit: bool,
}

/// Class of the character next to a delimiter run, as far as flanking is concerned.
///
/// A format string counts as a word character: `*%s*` emphasizes the format string.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MarkdownSide {
    Space,
    /// Punctuation that can start emphasized text: an opening bracket, a backtick or a
    /// backslash.
    ///
    /// Quotes are deliberately left out: a `*` next to a quote is nearly always a literal
    /// wildcard, e.g. `"*"`, `"log.*"` or `"*,!b"`.
    Opening,
    Punct,
    Word,
}

impl MarkdownSide {
    fn of(c: char) -> Self {
        if c.is_whitespace() {
            Self::Space
        } else if c.is_alphanumeric() {
            Self::Word
        } else if matches!(c, '(' | '[' | '{' | '`' | '\\') {
            Self::Opening
        } else {
            Self::Punct
        }
    }

    fn is_punct(self) -> bool {
        matches!(self, Self::Opening | Self::Punct)
    }
}

pub struct FormatMarkdownPos<'a> {
    s: &'a str,
    len: usize,
    pos: usize,
    fmt: Language,
    prev: MarkdownSide,
    pending: Vec<MarkdownPending>,
}

impl<'a> FormatMarkdownPos<'a> {
    pub fn new(s: &'a str, language: Language) -> Self {
        Self {
            s,
            len: s.len(),
            pos: 0,
            fmt: language,
            prev: MarkdownSide::Space,
            pending: Vec::new(),
        }
    }

    /// Return the character at `pos` (`None` for a format string) and the position just
    /// after it (after the whole format string), or `None` at the end of the string.
    fn step(&self, pos: usize) -> Option<(Option<char>, usize)> {
        let (c, new_pos, is_format) = self.fmt.next_char(self.s, pos)?;
        if is_format {
            Some((None, self.fmt.find_end_format(self.s, new_pos, self.len)))
        } else {
            Some((Some(c), new_pos))
        }
    }

    /// Return the class of the character at `pos`, the end of the string counting as a
    /// space.
    fn side_at(&self, pos: usize) -> MarkdownSide {
        match self.step(pos) {
            None => MarkdownSide::Space,
            Some((None, _)) => MarkdownSide::Word,
            Some((Some(c), _)) => MarkdownSide::of(c),
        }
    }

    /// Return the position just after the run of `c` starting at `pos`, which is `pos`
    /// itself when there is no `c` there.
    fn run_end(&self, pos: usize, c: char) -> usize {
        let mut end = pos;
        while let Some((Some(c2), new_pos)) = self.step(end)
            && c2 == c
        {
            end = new_pos;
        }
        end
    }

    /// Return the position just after the backslash escape starting at `pos` (on the
    /// backslash), or `None` when the backslash does not escape an ASCII punctuation
    /// character.
    fn escape_end(&self, pos: usize) -> Option<usize> {
        match self.step(pos) {
            Some((Some('\\'), after)) => match self.step(after) {
                Some((Some(c), end)) if c.is_ascii_punctuation() => Some(end),
                _ => None,
            },
            _ => None,
        }
    }

    /// Return the position just after the code span whose content starts at `pos`, opened
    /// by a run of `count` backticks, or `None` when it is not closed.
    ///
    /// The closing run must have exactly `count` backticks and must not be glued to a word:
    /// the legacy GNU quoting of `` `a' and `b' `` holds two backticks but no code span.
    fn code_span_end(&self, pos: usize, count: usize) -> Option<usize> {
        let mut pos = pos;
        while let Some((c, new_pos)) = self.step(pos) {
            if c == Some('`') {
                let end = self.run_end(pos, '`');
                if end - pos == count && self.side_at(end) != MarkdownSide::Word {
                    return Some(end);
                }
                pos = end;
            } else {
                pos = new_pos;
            }
        }
        None
    }

    /// Return the position just after the code span starting at `pos` (on its first
    /// backtick), or `None` when there is none.
    fn code_span_at(&self, pos: usize) -> Option<usize> {
        let after = self.run_end(pos, '`');
        self.code_span_end(after, after - pos)
    }

    /// Return the position of the `]` closing the link text that starts at `pos` (just
    /// after the `[`), or `None` when the brackets are not balanced.
    fn link_text_end(&self, pos: usize) -> Option<usize> {
        let mut pos = pos;
        let mut depth = 0_usize;
        while let Some((c, new_pos)) = self.step(pos) {
            match c {
                Some('\\') => {
                    pos = self.escape_end(pos).unwrap_or(new_pos);
                    continue;
                }
                Some('[') => depth += 1,
                Some(']') if depth == 0 => return Some(pos),
                Some(']') => depth -= 1,
                _ => {}
            }
            pos = new_pos;
        }
        None
    }

    /// Return the start and end of the link destination and the position just after the
    /// closing `)`, for the link tail whose content starts at `pos` (just after `](`), or
    /// `None` when the tail is not a valid destination with an optional title.
    fn link_tail(&self, pos: usize) -> Option<(usize, usize, usize)> {
        let skip_spaces = |mut pos: usize| {
            while let Some((Some(c), new_pos)) = self.step(pos)
                && c.is_whitespace()
            {
                pos = new_pos;
            }
            pos
        };
        let dest_start = skip_spaces(pos);
        let mut dest_end = dest_start;
        let (start, end) = if let Some((Some('<'), after)) = self.step(dest_start) {
            // Destination between angle brackets: `[text](<url with spaces>)`.
            let mut pos = after;
            loop {
                match self.step(pos)? {
                    (Some('>'), new_pos) => {
                        dest_end = new_pos;
                        break (after, pos);
                    }
                    (Some('<' | '\n'), _) => return None,
                    (_, new_pos) => pos = new_pos,
                }
            }
        } else {
            // Bare destination: no whitespace, parentheses balanced.
            let mut depth = 0_usize;
            while let Some((c, new_pos)) = self.step(dest_end) {
                match c {
                    Some(c) if c.is_whitespace() || c.is_control() => break,
                    Some('(') => depth += 1,
                    Some(')') if depth == 0 => break,
                    Some(')') => depth -= 1,
                    _ => {}
                }
                dest_end = new_pos;
            }
            (dest_start, dest_end)
        };
        let mut pos = skip_spaces(dest_end);
        // Optional title, which is prose: it is skipped, not compared.
        if pos > dest_end
            && let Some((Some(open), after)) = self.step(pos)
            && let Some(close) = match open {
                '"' => Some('"'),
                '\'' => Some('\''),
                '(' => Some(')'),
                _ => None,
            }
        {
            pos = after;
            loop {
                match self.step(pos)? {
                    (Some(c), new_pos) if c == close => {
                        pos = skip_spaces(new_pos);
                        break;
                    }
                    (_, new_pos) => pos = new_pos,
                }
            }
        }
        match self.step(pos)? {
            (Some(')'), after) => Some((start, end, after)),
            _ => None,
        }
    }

    /// Return the start and end of the destination and the positions of the `](url)` tail
    /// of the inline link whose text starts at `pos` (just after the `[`), or `None` when
    /// there is no inline link there.
    fn link_at(&self, pos: usize) -> Option<(usize, usize, MarkdownPending)> {
        let bracket = self.link_text_end(pos)?;
        let (_, after_bracket) = self.step(bracket)?;
        let Some((Some('('), after_paren)) = self.step(after_bracket) else {
            return None;
        };
        let (start, end, tail_end) = self.link_tail(after_paren)?;
        // The destination must look like a URL or a path: `[透明度](_R)` is a label
        // followed by its accelerator.
        let dest = &self.s[start..end];
        if !dest.contains(['/', '.', ':', '#']) && FormatPos::new(dest, self.fmt).next().is_none() {
            return None;
        }
        Some((
            start,
            end,
            MarkdownPending {
                start: bracket,
                end: tail_end,
                emit: false,
            },
        ))
    }

    /// Return whether the run of `marker` ending at `end`, preceded by a character
    /// of class `prev`, can open and can close an emphasis.
    ///
    /// These are the `CommonMark` flanking rules, with two restrictions that keep plain
    /// text out:
    /// - an opening run must be followed by a word, or by an [opening](MarkdownSide::Opening)
    ///   character when it is not itself preceded by punctuation: the `*` of the globs
    ///   `(*.*)` and `*.icc,*.icm` and the quoted `*` of `"*"` and `"a *"` open nothing,
    /// - an `_` run must not be glued to a word on the outer side, so that
    ///   `snake_case_name` holds no emphasis.
    fn flanking(&self, marker: char, prev: MarkdownSide, end: usize) -> (bool, bool) {
        let next = self.side_at(end);
        let left = next != MarkdownSide::Space
            && (!next.is_punct() || prev == MarkdownSide::Space || prev.is_punct());
        let right = prev != MarkdownSide::Space
            && (!prev.is_punct() || next == MarkdownSide::Space || next.is_punct());
        let can_open = left
            && (next == MarkdownSide::Word || (next == MarkdownSide::Opening && !prev.is_punct()));
        if marker == '_' {
            (
                can_open && prev != MarkdownSide::Word,
                right && next != MarkdownSide::Word,
            )
        } else {
            (can_open, right)
        }
    }

    /// Return the start and end of the run closing the emphasis opened by the run of
    /// `count` times `marker` ending at `pos`, or `None` when it is not closed before
    /// `limit`.
    ///
    /// Code spans and escaped characters are stepped over, and nested runs of the same
    /// length are paired first: in `*a *b* c*`, the outer `*` closes after `c`.
    fn emphasis_end(
        &self,
        pos: usize,
        marker: char,
        count: usize,
        limit: usize,
    ) -> Option<(usize, usize)> {
        let mut pos = pos;
        let mut prev = MarkdownSide::Word;
        let mut depth = 0_usize;
        while pos < limit {
            let (c, new_pos) = self.step(pos)?;
            match c {
                Some('\\') => {
                    if let Some(end) = self.escape_end(pos) {
                        pos = end;
                        prev = MarkdownSide::Punct;
                        continue;
                    }
                    prev = MarkdownSide::Punct;
                }
                Some('`') => {
                    let end = self
                        .code_span_at(pos)
                        .unwrap_or_else(|| self.run_end(pos, '`'));
                    pos = end;
                    prev = MarkdownSide::Punct;
                    continue;
                }
                Some(c) if c == marker => {
                    let end = self.run_end(pos, marker);
                    if end - pos == count {
                        let (can_open, can_close) = self.flanking(marker, prev, end);
                        if can_close {
                            if depth == 0 {
                                return Some((pos, end));
                            }
                            depth -= 1;
                        } else if can_open {
                            depth += 1;
                        }
                    }
                    pos = end;
                    prev = MarkdownSide::Punct;
                    continue;
                }
                Some(c) => prev = MarkdownSide::of(c),
                None => prev = MarkdownSide::Word,
            }
            pos = new_pos;
        }
        None
    }

    /// Move the scan to `pos`, just after a construct ending with `last`, and forget the
    /// pending positions jumped over.
    fn jump(&mut self, pos: usize, last: MarkdownSide) {
        self.pos = pos;
        self.prev = last;
        self.pending.retain(|p| p.start >= pos);
    }
}

/// Iterator returning Markdown constructs of a string, according to the given language,
/// skipping format strings.
///
/// Three constructs are returned, each with its [kind](MarkdownKind):
/// - code spans, including their backticks: `` `code` ``,
/// - destinations of inline links and images: the `url` of `[text](url)` and `![alt](url)`
///   (the text and the optional title are prose, and are skipped),
/// - emphasis delimiters, opening and closing: `*`, `**`, `***`, `_`, `__` and `___`.
///
/// Only well-formed constructs are returned: a lone backtick, a `[text] (url)` with a space
/// or an unclosed `**` are plain text, which is precisely what they render as.
///
/// For example with the string ``See **[the docs](https://example.com)** or run `make` ``,
/// it will return `**`, `https://example.com`, `**` and `` `make` `` with their positions
/// in the string.
impl<'a> Iterator for FormatMarkdownPos<'a> {
    type Item = MatchMarkdownPos<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((c, new_pos)) = self.step(self.pos) {
            let start = self.pos;
            if !self.pending.is_empty()
                && let Some(idx) = self.pending.iter().position(|p| p.start == start)
            {
                let p = self.pending.swap_remove(idx);
                self.jump(p.end, MarkdownSide::Punct);
                if p.emit {
                    return Some(MatchMarkdownPos {
                        kind: MarkdownKind::Emphasis,
                        s: &self.s[p.start..p.end],
                        start: p.start,
                        end: p.end,
                    });
                }
                continue;
            }
            let Some(c) = c else {
                // Format string.
                self.jump(new_pos, MarkdownSide::Word);
                continue;
            };
            match c {
                '\\' => {
                    if let Some(end) = self.escape_end(start) {
                        self.jump(end, MarkdownSide::Punct);
                        continue;
                    }
                }
                '`' => {
                    let end = self.run_end(start, '`');
                    if self.prev != MarkdownSide::Word
                        && let Some(span_end) = self.code_span_end(end, end - start)
                    {
                        self.jump(span_end, MarkdownSide::Punct);
                        return Some(MatchMarkdownPos {
                            kind: MarkdownKind::CodeSpan,
                            s: &self.s[start..span_end],
                            start,
                            end: span_end,
                        });
                    }
                    self.jump(end, MarkdownSide::Punct);
                    continue;
                }
                '[' => {
                    if let Some((dest_start, dest_end, tail)) = self.link_at(new_pos) {
                        // The link text is scanned next: it may hold emphasis or code.
                        self.pending.push(tail);
                        self.pos = new_pos;
                        self.prev = MarkdownSide::Punct;
                        return Some(MatchMarkdownPos {
                            kind: MarkdownKind::Link,
                            s: &self.s[dest_start..dest_end],
                            start: dest_start,
                            end: dest_end,
                        });
                    }
                }
                '*' | '_' => {
                    let end = self.run_end(start, c);
                    let count = end - start;
                    let (can_open, _) = self.flanking(c, self.prev, end);
                    if count <= 3 && can_open {
                        // An emphasis opened inside a link text must close inside it.
                        let limit = self
                            .pending
                            .iter()
                            .filter(|p| !p.emit && p.start > start)
                            .map(|p| p.start)
                            .min()
                            .unwrap_or(self.len);
                        if let Some((close_start, close_end)) =
                            self.emphasis_end(end, c, count, limit)
                        {
                            self.pending.push(MarkdownPending {
                                start: close_start,
                                end: close_end,
                                emit: true,
                            });
                            self.pos = end;
                            self.prev = MarkdownSide::Punct;
                            return Some(MatchMarkdownPos {
                                kind: MarkdownKind::Emphasis,
                                s: &self.s[start..end],
                                start,
                                end,
                            });
                        }
                    }
                    self.jump(end, MarkdownSide::Punct);
                    continue;
                }
                _ => {}
            }
            self.pos = new_pos;
            self.prev = MarkdownSide::of(c);
        }
        None
    }
}
