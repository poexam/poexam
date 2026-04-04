// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Word iterator: return words of a string.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchWordPos<'a> {
    pub s: &'a str,
    pub start: usize,
    pub end: usize,
}

pub struct WordPos<'a> {
    s: &'a str,
    pos: usize,
}

impl<'a> WordPos<'a> {
    pub fn new(s: &'a str) -> Self {
        Self { s, pos: 0 }
    }
}

impl<'a> Iterator for WordPos<'a> {
    type Item = MatchWordPos<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut idx_start = None;
        let mut idx_end = None;
        let mut start_apostrophe = false;
        for c in self.s[self.pos..].chars() {
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
            } else if idx_start.is_some() {
                self.pos += len_c;
                break;
            }
            self.pos += len_c;
        }
        match (idx_start, idx_end) {
            (Some(start), Some(end)) => {
                let s = &self.s[start..end];
                if start_apostrophe && let Some(s2) = s.strip_suffix('\'') {
                    Some(MatchWordPos {
                        s: s2,
                        start,
                        end: end - 1,
                    })
                } else {
                    Some(MatchWordPos { s, start, end })
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_words() {
        assert!(WordPos::new("").next().is_none());
        assert!(WordPos::new(" ,.!? ").next().is_none());
    }

    #[test]
    fn test_words() {
        assert_eq!(
            WordPos::new("Hello, world! %llu test-word didn't didn’t didnʼt 'test' 42.")
                .collect::<Vec<_>>(),
            vec![
                MatchWordPos {
                    s: "Hello",
                    start: 0,
                    end: 5,
                },
                MatchWordPos {
                    s: "world",
                    start: 7,
                    end: 12,
                },
                MatchWordPos {
                    s: "llu",
                    start: 15,
                    end: 18,
                },
                MatchWordPos {
                    s: "test-word",
                    start: 19,
                    end: 28,
                },
                MatchWordPos {
                    s: "didn't", // U+0027: APOSTROPHE
                    start: 29,
                    end: 35,
                },
                MatchWordPos {
                    s: "didn’t", // U+2019: RIGHT SINGLE QUOTATION MARK
                    start: 36,
                    end: 44,
                },
                MatchWordPos {
                    s: "didnʼt", // U+02BC: MODIFIER LETTER APOSTROPHE
                    start: 45,
                    end: 52,
                },
                MatchWordPos {
                    s: "test",
                    start: 54,
                    end: 58,
                },
                MatchWordPos {
                    s: "42",
                    start: 60,
                    end: 62,
                },
            ]
        );
        assert_eq!(
            WordPos::new("héllo, мир! %lld 你好").collect::<Vec<_>>(),
            vec![
                MatchWordPos {
                    s: "héllo",
                    start: 0,
                    end: 6,
                },
                MatchWordPos {
                    s: "мир",
                    start: 8,
                    end: 14,
                },
                MatchWordPos {
                    s: "lld",
                    start: 17,
                    end: 20,
                },
                MatchWordPos {
                    s: "你好",
                    start: 21,
                    end: 27,
                },
            ]
        );
    }
}
