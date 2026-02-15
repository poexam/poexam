// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Char iterator: return chars of a string, skipping format strings.

use crate::po::format::{FormatParser, MatchStrPos, language::Language};

pub struct CharPos<'a> {
    s: &'a str,
    len: usize,
    pos: usize,
    fmt: Box<dyn FormatParser>,
}

impl<'a> CharPos<'a> {
    pub fn new(s: &'a str, language: &Language) -> Self {
        Self {
            s,
            len: s.len(),
            pos: 0,
            fmt: language.format_parser(),
        }
    }
}

impl<'a> Iterator for CharPos<'a> {
    type Item = MatchStrPos<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.pos < self.len {
            let (new_pos, is_format) = self.fmt.next_char(self.s, self.pos, self.len);
            self.pos = new_pos;
            if self.pos >= self.len {
                return None;
            }
            if is_format {
                self.pos = self.fmt.find_end_format(self.s, self.pos, self.len);
                continue;
            }
            match self.s[self.pos..].chars().next() {
                Some(c) => {
                    let len_c = c.len_utf8();
                    if c.is_alphanumeric() || c == '-' {
                        let result = MatchStrPos {
                            s: &self.s[self.pos..self.pos + len_c],
                            start: self.pos,
                            end: self.pos + len_c,
                        };
                        self.pos += len_c;
                        return Some(result);
                    }
                    self.pos += len_c;
                }
                None => return None,
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::po::format::MatchStrPos;

    #[test]
    fn test_no_chars() {
        assert!(CharPos::new("", &Language::Null).next().is_none());
        assert!(CharPos::new(" ,.!? ", &Language::Null).next().is_none());
    }

    #[test]
    fn test_chars() {
        assert_eq!(
            CharPos::new("Hé, w!", &Language::Null).collect::<Vec<_>>(),
            vec![
                MatchStrPos {
                    s: "H",
                    start: 0,
                    end: 1,
                },
                MatchStrPos {
                    s: "é",
                    start: 1,
                    end: 3,
                },
                MatchStrPos {
                    s: "w",
                    start: 5,
                    end: 6,
                },
            ]
        );
    }
}
