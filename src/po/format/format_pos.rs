// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format iterator: return format strings.

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
        let mut start;
        while self.pos < self.len {
            start = self.pos;
            let (new_pos, is_format) = self.fmt.next_char(self.s, self.pos, self.len);
            self.pos = new_pos;
            if self.pos >= self.len {
                return None;
            }
            if is_format {
                self.pos = self.fmt.find_end_format(self.s, self.pos, self.len);
                return Some(MatchStrPos {
                    s: &self.s[start..self.pos],
                    start,
                    end: self.pos,
                });
            }
            // Move to the next character.
            match self.s[self.pos..].chars().next() {
                Some(c) => self.pos += c.len_utf8(),
                None => return None,
            }
        }
        None
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
    }
}
