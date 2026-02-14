// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Word iterator: return words of a string, skipping format strings.

use crate::po::format::{FormatParser, MatchStrPos, language::Language};

pub struct WordPos<'a> {
    s: &'a str,
    len: usize,
    pos: usize,
    fmt: Box<dyn FormatParser>,
}

impl<'a> WordPos<'a> {
    pub fn new(s: &'a str, language: &Language) -> Self {
        Self {
            s,
            len: s.len(),
            pos: 0,
            fmt: language.format_parser(),
        }
    }
}

impl<'a> Iterator for WordPos<'a> {
    type Item = MatchStrPos<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut idx_start = None;
        let mut idx_end = None;
        while self.pos < self.len {
            if idx_start.is_none() {
                let (new_pos, is_format) = self.fmt.next_char(self.s, self.pos, self.len);
                self.pos = new_pos;
                if self.pos >= self.len {
                    return None;
                }
                if is_format {
                    self.pos = self.fmt.find_end_format(self.s, self.pos, self.len);
                    continue;
                }
            }
            match self.s[self.pos..].chars().next() {
                Some(c) => {
                    let len_c = c.len_utf8();
                    if c.is_alphanumeric() || (idx_start.is_some() && c == '-') {
                        if idx_start.is_none() {
                            idx_start = Some(self.pos);
                        }
                        idx_end = Some(self.pos + len_c);
                    } else if idx_start.is_some() {
                        break;
                    }
                    self.pos += len_c;
                }
                None => return None,
            }
        }
        match (idx_start, idx_end) {
            (Some(start), Some(end)) => Some(MatchStrPos {
                s: &self.s[start..end],
                start,
                end,
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::po::format::MatchStrPos;

    #[test]
    fn test_no_words() {
        assert!(WordPos::new("", &Language::Null).next().is_none());
        assert!(WordPos::new(" ,.!? ", &Language::Null).next().is_none());
    }

    #[test]
    fn test_words() {
        let mut word_pos = WordPos::new("Hello, world! %llu test-word 42.", &Language::Null);
        assert_eq!(
            word_pos.next(),
            Some(MatchStrPos {
                s: "Hello",
                start: 0,
                end: 5,
            })
        );
        assert_eq!(
            word_pos.next(),
            Some(MatchStrPos {
                s: "world",
                start: 7,
                end: 12,
            })
        );
        assert_eq!(
            word_pos.next(),
            Some(MatchStrPos {
                s: "llu",
                start: 15,
                end: 18,
            })
        );
        assert_eq!(
            word_pos.next(),
            Some(MatchStrPos {
                s: "test-word",
                start: 19,
                end: 28,
            })
        );
        assert_eq!(
            word_pos.next(),
            Some(MatchStrPos {
                s: "42",
                start: 29,
                end: 31,
            })
        );
        assert!(word_pos.next().is_none());

        let mut word_pos = WordPos::new("héllo, мир! %lld 你好", &Language::Null);
        assert_eq!(
            word_pos.next(),
            Some(MatchStrPos {
                s: "héllo",
                start: 0,
                end: 6,
            })
        );
        assert_eq!(
            word_pos.next(),
            Some(MatchStrPos {
                s: "мир",
                start: 8,
                end: 14,
            })
        );
        assert_eq!(
            word_pos.next(),
            Some(MatchStrPos {
                s: "lld",
                start: 17,
                end: 20,
            })
        );
        assert_eq!(
            word_pos.next(),
            Some(MatchStrPos {
                s: "你好",
                start: 21,
                end: 27,
            })
        );
        assert!(word_pos.next().is_none());
    }
}
