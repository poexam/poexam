// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format strings: no language.

use crate::po::format::FormatParser;

pub struct FormatNull;

impl FormatParser for FormatNull {
    #[inline]
    fn next_char(&self, _s: &str, pos: usize, _len: usize) -> (usize, bool) {
        (pos, false)
    }

    #[inline]
    fn find_end_format(&self, _s: &str, _pos: usize, len: usize) -> usize {
        len
    }
}

#[cfg(test)]
mod tests {
    use crate::po::format::{
        MatchStrPos, char_pos::CharPos, format_pos::FormatPos, language::Language, url_pos::UrlPos,
        word_pos::WordPos,
    };

    #[test]
    fn test_no_format() {
        assert!(
            FormatPos::new("Hello, %s world!", &Language::Null)
                .next()
                .is_none()
        );
        assert_eq!(
            WordPos::new("Hello, %s world!", &Language::Null).collect::<Vec<_>>(),
            vec![
                MatchStrPos {
                    s: "Hello",
                    start: 0,
                    end: 5,
                },
                MatchStrPos {
                    s: "s",
                    start: 8,
                    end: 9,
                },
                MatchStrPos {
                    s: "world",
                    start: 10,
                    end: 15,
                },
            ]
        );
        assert_eq!(
            CharPos::new("Hé, %s w!", &Language::Null).collect::<Vec<_>>(),
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
                    s: "s",
                    start: 6,
                    end: 7,
                },
                MatchStrPos {
                    s: "w",
                    start: 8,
                    end: 9,
                },
            ]
        );
        assert_eq!(
            UrlPos::new("Hello, %s world! https://example.com", &Language::Null)
                .collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "https://example.com",
                start: 17,
                end: 36,
            }]
        );
    }
}
