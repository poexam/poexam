// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format iterator: return format strings.

use crate::po::format::{FormatParser, MatchFmtPos, language::Language};

pub struct FormatPos<'a> {
    s: &'a str,
    len: usize,
    pos: usize,
    fmt: Box<dyn FormatParser>,
}

impl<'a> FormatPos<'a> {
    pub fn new(s: &'a str, language: &Language) -> Self {
        Self {
            s,
            len: s.len(),
            pos: 0,
            fmt: language.format_parser(),
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
    fmt: Box<dyn FormatParser>,
}

impl<'a> FormatWordPos<'a> {
    pub fn new(s: &'a str, language: &Language) -> Self {
        Self {
            s,
            len: s.len(),
            pos: 0,
            fmt: language.format_parser(),
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

pub struct FormatUrlPos<'a> {
    s: &'a str,
    len: usize,
    pos: usize,
    fmt: Box<dyn FormatParser>,
}

impl<'a> FormatUrlPos<'a> {
    pub fn new(s: &'a str, language: &Language) -> Self {
        Self {
            s,
            len: s.len(),
            pos: 0,
            fmt: language.format_parser(),
        }
    }
}

/// Iterator returning URLs of a string, according to the given language, skipping
/// format strings.
///
/// For example in C language, with the string `Hello, %d %s world! https://example.com`,
/// it will return `https://example.com` with its position in the string.
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
                (Some(start), Some(end)) => {
                    let s = &self.s[start..end];
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
    fmt: Box<dyn FormatParser>,
}

impl<'a> FormatEmailPos<'a> {
    pub fn new(s: &'a str, language: &Language) -> Self {
        Self {
            s,
            len: s.len(),
            pos: 0,
            fmt: language.format_parser(),
        }
    }

    /// Simple check for email validity: check that it contains exactly one '@' and that
    /// local and domain parts are not empty and contain only allowed characters, with
    /// relaxed rules (e.g. allow language formats like `%s` or `{0}`).
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
                        || c == '%'
                        || c == '{'
                        || c == '}'
                        || c == '$'
                        || c == '"'
                        || c == '„'
                        || c == '”'
                        || c == '«'
                        || c == '»'
                })
                && domain.chars().all(|c| {
                    c.is_alphanumeric()
                        || c == '.'
                        || c == '-'
                        || c == '%'
                        || c == '{'
                        || c == '}'
                        || c == '$'
                        || c == '"'
                        || c == '„'
                        || c == '”'
                        || c == '«'
                        || c == '»'
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
                (Some(start), Some(end)) => {
                    let s = &self.s[start..end];
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
    fmt: Box<dyn FormatParser>,
}

impl<'a> FormatPathPos<'a> {
    pub fn new(s: &'a str, language: &Language) -> Self {
        Self {
            s,
            len: s.len(),
            pos: 0,
            fmt: language.format_parser(),
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
    fmt: Box<dyn FormatParser>,
}

impl<'a> FormatHtmlTagPos<'a> {
    pub fn new(s: &'a str, language: &Language) -> Self {
        Self {
            s,
            len: s.len(),
            pos: 0,
            fmt: language.format_parser(),
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
