// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format iterator: return format strings.

use std::borrow::Cow;

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
            let len_c = c.len_utf8();
            if idx_start.is_none() && c == '\'' {
                start_apostrophe = true;
            }
            if c.is_alphanumeric()
                || (idx_start.is_some() && (c == '-' || c == '\'' || c == '’') || (c == 'ʼ'))
            {
                if idx_start.is_none() {
                    idx_start = Some(self.pos);
                }
                idx_end = Some(self.pos + len_c);
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
                let len_c = c.len_utf8();
                if !c.is_whitespace() {
                    if idx_start.is_none() {
                        idx_start = Some(self.pos);
                    }
                    idx_end = Some(self.pos + len_c);
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
