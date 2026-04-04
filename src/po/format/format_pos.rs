// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format iterator: return format strings.

use std::borrow::Cow;

use crate::po::format::{FormatParser, MatchStrPos, language::Language};

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

impl<'a> Iterator for FormatPos<'a> {
    type Item = MatchStrPos<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((_, new_pos, is_format)) = self.fmt.next_char(self.s, self.pos) {
            if is_format {
                let start = self.pos;
                self.pos = self.fmt.find_end_format(self.s, new_pos, self.len);
                return Some(MatchStrPos {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_format() {
        assert!(
            FormatPos::new("Hello, world!", &Language::Null)
                .next()
                .is_none()
        );
        assert_eq!(
            strip_formats("Hello, world!", &Language::Null),
            "Hello, world!"
        );
    }
}
