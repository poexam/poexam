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
                    if s.contains("://") {
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
}

/// Simple check for email validity: check that it contains exactly one '@' and that
/// local and domain parts are not empty and contain only allowed characters, with
/// relaxed rules (e.g. allow language formats like `%s` or `{0}`).
fn is_valid_email(email: &str) -> bool {
    if let Some(pos_arobase) = email.find('@') {
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
    } else {
        false
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
                    if is_valid_email(s) {
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
