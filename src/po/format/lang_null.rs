// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format strings: no language.

use crate::po::format::FormatParser;

pub struct FormatNull;

impl FormatParser for FormatNull {
    #[inline]
    fn next_char(&self, s: &str, pos: usize) -> Option<(char, usize, bool)> {
        s[pos..]
            .chars()
            .next()
            .map(|c| (c, pos + c.len_utf8(), false))
    }

    #[inline]
    fn find_end_format(&self, _s: &str, _pos: usize, len: usize) -> usize {
        len
    }
}

#[cfg(test)]
mod tests {
    use crate::po::format::{
        MatchFmtPos,
        iterators::{FormatEmailPos, FormatPos, FormatUrlPos, FormatWordPos, strip_formats},
        language::Language,
    };

    #[test]
    fn test_empty_string() {
        let s = "";
        assert!(FormatPos::new(s, &Language::Null).next().is_none());
        assert!(FormatWordPos::new(s, &Language::Null).next().is_none());
        assert!(FormatUrlPos::new(s, &Language::Null).next().is_none());
        assert!(strip_formats(s, &Language::Null).is_empty());
    }

    #[test]
    fn test_no_format() {
        let s = "Hello, %s world! 'test' https://example.com invalid@domain user@domain.com";
        assert!(FormatPos::new(s, &Language::Null).next().is_none());
        assert_eq!(
            FormatWordPos::new(s, &Language::Null).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "Hello",
                    start: 0,
                    end: 5,
                },
                MatchFmtPos {
                    s: "s",
                    start: 8,
                    end: 9,
                },
                MatchFmtPos {
                    s: "world",
                    start: 10,
                    end: 15,
                },
                MatchFmtPos {
                    s: "test",
                    start: 18,
                    end: 22,
                },
                MatchFmtPos {
                    s: "https",
                    start: 24,
                    end: 29,
                },
                MatchFmtPos {
                    s: "example",
                    start: 32,
                    end: 39,
                },
                MatchFmtPos {
                    s: "com",
                    start: 40,
                    end: 43,
                },
                MatchFmtPos {
                    s: "invalid",
                    start: 44,
                    end: 51,
                },
                MatchFmtPos {
                    s: "domain",
                    start: 52,
                    end: 58,
                },
                MatchFmtPos {
                    s: "user",
                    start: 59,
                    end: 63,
                },
                MatchFmtPos {
                    s: "domain",
                    start: 64,
                    end: 70,
                },
                MatchFmtPos {
                    s: "com",
                    start: 71,
                    end: 74,
                },
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::Null).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "https://example.com",
                start: 24,
                end: 43,
            }]
        );
        assert_eq!(
            FormatEmailPos::new(s, &Language::Null).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "user@domain.com",
                start: 59,
                end: 74,
            }]
        );
        assert_eq!(strip_formats(s, &Language::Null), s);
    }
}
