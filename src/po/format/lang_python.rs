// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format strings: Python language.

use crate::po::format::FormatParser;

pub struct FormatPython;

impl FormatParser for FormatPython {
    #[inline]
    fn next_char(&self, s: &str, pos: usize, len: usize) -> (usize, bool) {
        let bytes = s.as_bytes();
        if pos + 1 >= len || bytes[pos] != b'%' {
            (pos, false)
        } else {
            (pos + 1, bytes[pos + 1] != b'%')
        }
    }

    #[inline]
    fn find_end_format(&self, s: &str, pos: usize, len: usize) -> usize {
        let bytes = s.as_bytes();
        let mut pos_end = pos;

        // See: https://docs.python.org/3.15/library/stdtypes.html#printf-style-string-formatting

        if pos_end < len && bytes[pos_end] == b'(' {
            if let Some(pos_end_key) = bytes[pos_end..].iter().position(|&b| b == b')') {
                pos_end += pos_end_key + 1;
            } else {
                return len;
            }
        }

        // Skip conversion flags.
        while pos_end < len {
            if matches!(
                bytes[pos_end],
                b'-' | b'+' | b' ' | b'#' | b'.' | b'0'..=b'9'
            ) {
                pos_end += 1;
            } else {
                break;
            }
        }

        // Parse length modifiers (h, l, L).
        if pos_end < len && matches!(bytes[pos_end], b'h' | b'l' | b'L') {
            pos_end += 1;
        }

        // Parse conversion type (e.g. s, d, f, etc.).
        if pos_end < len && bytes[pos_end].is_ascii_alphabetic() {
            pos_end += 1;
        }

        pos_end
    }
}

pub struct FormatPythonBrace;

impl FormatParser for FormatPythonBrace {
    #[inline]
    fn next_char(&self, s: &str, pos: usize, len: usize) -> (usize, bool) {
        let bytes = s.as_bytes();
        if pos + 1 >= len || bytes[pos] != b'{' {
            (pos, false)
        } else {
            (pos + 1, bytes[pos + 1] != b'{')
        }
    }

    #[inline]
    fn find_end_format(&self, s: &str, pos: usize, len: usize) -> usize {
        let bytes = s.as_bytes();
        let mut pos_end = pos;

        // See: https://peps.python.org/pep-3101/

        // Find the closing curly bracket, skipping any nested curly brackets.
        let mut level = 1;
        while pos_end < len {
            if bytes[pos_end] == b'{' {
                level += 1;
            } else if bytes[pos_end] == b'}' {
                level -= 1;
                if level <= 0 {
                    pos_end += 1;
                    break;
                }
            }
            pos_end += 1;
        }

        pos_end
    }
}

#[cfg(test)]
mod tests {
    use crate::po::format::{
        MatchStrPos, char_pos::CharPos, format_pos::FormatPos, language::Language, url_pos::UrlPos,
        word_pos::WordPos,
    };

    #[test]
    fn test_no_format_percent() {
        assert!(
            FormatPos::new("Hello, world!", &Language::Python)
                .next()
                .is_none()
        );
        assert_eq!(
            WordPos::new("Hello, world!", &Language::Python).collect::<Vec<_>>(),
            vec![
                MatchStrPos {
                    s: "Hello",
                    start: 0,
                    end: 5
                },
                MatchStrPos {
                    s: "world",
                    start: 7,
                    end: 12
                },
            ]
        );
        assert_eq!(
            CharPos::new("Hé, w!", &Language::Python).collect::<Vec<_>>(),
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
        assert_eq!(
            UrlPos::new("Hello, world! https://example.com", &Language::Python).collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "https://example.com",
                start: 14,
                end: 33,
            }]
        );
    }

    #[test]
    fn test_no_format_brace() {
        assert!(
            FormatPos::new("Hello, world!", &Language::PythonBrace)
                .next()
                .is_none()
        );
        assert_eq!(
            WordPos::new("Hello, world!", &Language::PythonBrace).collect::<Vec<_>>(),
            vec![
                MatchStrPos {
                    s: "Hello",
                    start: 0,
                    end: 5,
                },
                MatchStrPos {
                    s: "world",
                    start: 7,
                    end: 12,
                },
            ]
        );
        assert_eq!(
            CharPos::new("Hé, w!", &Language::PythonBrace).collect::<Vec<_>>(),
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
        assert_eq!(
            UrlPos::new("Hello, world! https://example.com", &Language::PythonBrace)
                .collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "https://example.com",
                start: 14,
                end: 33,
            }]
        );
    }

    #[test]
    fn test_invalid_format_percent() {
        let s = "%";
        assert!(FormatPos::new(s, &Language::Python).next().is_none());
        assert!(WordPos::new(s, &Language::Python).next().is_none());
        assert!(CharPos::new(s, &Language::Python).next().is_none());

        assert_eq!(
            FormatPos::new("%é", &Language::Python).collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "%",
                start: 0,
                end: 1,
            }]
        );
        assert_eq!(
            WordPos::new("%é", &Language::Python).collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "é",
                start: 1,
                end: 3,
            }]
        );
        assert_eq!(
            CharPos::new("%é", &Language::Python).collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "é",
                start: 1,
                end: 3,
            }]
        );

        let s = "%(test";
        assert_eq!(
            FormatPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "%(test",
                start: 0,
                end: 6,
            }]
        );
        assert!(WordPos::new(s, &Language::Python).next().is_none());
        assert!(CharPos::new(s, &Language::Python).next().is_none());
        assert!(UrlPos::new(s, &Language::Python).next().is_none());
    }

    #[test]
    fn test_invalid_format_brace() {
        let s = "{";
        assert!(FormatPos::new(s, &Language::PythonBrace).next().is_none());
        assert!(WordPos::new(s, &Language::PythonBrace).next().is_none());
        assert!(CharPos::new(s, &Language::PythonBrace).next().is_none());
        assert!(UrlPos::new(s, &Language::PythonBrace).next().is_none());

        let s = "{é";
        assert_eq!(
            FormatPos::new(s, &Language::PythonBrace).collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "{é",
                start: 0,
                end: 3,
            }]
        );
        assert!(WordPos::new(s, &Language::PythonBrace).next().is_none());
        assert!(CharPos::new(s, &Language::PythonBrace).next().is_none());
        assert!(UrlPos::new(s, &Language::PythonBrace).next().is_none());
    }

    #[test]
    fn test_single_format_percent() {
        assert_eq!(
            FormatPos::new("Hello, %s world!", &Language::Python).collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "%s",
                start: 7,
                end: 9,
            }]
        );
        assert_eq!(
            WordPos::new("Hello, %s world!", &Language::Python).collect::<Vec<_>>(),
            vec![
                MatchStrPos {
                    s: "Hello",
                    start: 0,
                    end: 5,
                },
                MatchStrPos {
                    s: "world",
                    start: 10,
                    end: 15,
                },
            ]
        );
        assert_eq!(
            CharPos::new("Hé, %s w!", &Language::Python).collect::<Vec<_>>(),
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
                    start: 8,
                    end: 9,
                },
            ]
        );
        assert_eq!(
            UrlPos::new("Hello, world! -> %shttps://example.com", &Language::Python)
                .collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "https://example.com",
                start: 19,
                end: 38,
            }]
        );
    }

    #[test]
    fn test_single_format_brace() {
        assert_eq!(
            FormatPos::new("Hello, {0:{1}} world!", &Language::PythonBrace).collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "{0:{1}}",
                start: 7,
                end: 14,
            }]
        );
        assert_eq!(
            WordPos::new("Hello, {0:{1}} world!", &Language::PythonBrace).collect::<Vec<_>>(),
            vec![
                MatchStrPos {
                    s: "Hello",
                    start: 0,
                    end: 5,
                },
                MatchStrPos {
                    s: "world",
                    start: 15,
                    end: 20,
                },
            ]
        );
        assert_eq!(
            CharPos::new("Hé, {0:{1}} w!", &Language::PythonBrace).collect::<Vec<_>>(),
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
                    start: 13,
                    end: 14,
                },
            ]
        );
        assert_eq!(
            UrlPos::new(
                "Hello, world! -> {0:{1}}https://example.com",
                &Language::PythonBrace
            )
            .collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "https://example.com",
                start: 24,
                end: 43,
            }]
        );
    }

    #[test]
    fn test_single_format_percent_keyword() {
        assert_eq!(
            FormatPos::new("Hello, %(name)s world!", &Language::Python).collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "%(name)s",
                start: 7,
                end: 15,
            }]
        );
        assert_eq!(
            WordPos::new("Hello, %(name)s world!", &Language::Python).collect::<Vec<_>>(),
            vec![
                MatchStrPos {
                    s: "Hello",
                    start: 0,
                    end: 5,
                },
                MatchStrPos {
                    s: "world",
                    start: 16,
                    end: 21,
                },
            ]
        );
        assert_eq!(
            CharPos::new("Hé, %(name)s w!", &Language::Python).collect::<Vec<_>>(),
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
                    start: 14,
                    end: 15,
                },
            ]
        );
        assert_eq!(
            UrlPos::new(
                "Hello, world! -> %(name)shttps://example.com",
                &Language::Python
            )
            .collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "https://example.com",
                start: 25,
                end: 44,
            }]
        );
    }

    #[test]
    fn test_multiple_formats_percent() {
        let s = "%d%s%f";
        assert_eq!(
            FormatPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![
                MatchStrPos {
                    s: "%d",
                    start: 0,
                    end: 2,
                },
                MatchStrPos {
                    s: "%s",
                    start: 2,
                    end: 4,
                },
                MatchStrPos {
                    s: "%f",
                    start: 4,
                    end: 6,
                },
            ]
        );
        assert!(WordPos::new(s, &Language::Python).next().is_none());
        assert!(CharPos::new(s, &Language::Python).next().is_none());
        assert!(UrlPos::new(s, &Language::Python).next().is_none());
    }

    #[test]
    fn test_multiple_formats_brace() {
        let s = "{0!r:20}{1}{2}";
        assert_eq!(
            FormatPos::new(s, &Language::PythonBrace).collect::<Vec<_>>(),
            vec![
                MatchStrPos {
                    s: "{0!r:20}",
                    start: 0,
                    end: 8,
                },
                MatchStrPos {
                    s: "{1}",
                    start: 8,
                    end: 11,
                },
                MatchStrPos {
                    s: "{2}",
                    start: 11,
                    end: 14,
                },
            ]
        );
        assert!(WordPos::new(s, &Language::PythonBrace).next().is_none());
        assert!(CharPos::new(s, &Language::PythonBrace).next().is_none());
        assert!(UrlPos::new(s, &Language::PythonBrace).next().is_none());
    }

    #[test]
    fn test_escaped_percent() {
        assert_eq!(
            FormatPos::new("Hello, %% %s world!", &Language::Python).collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "%s",
                start: 10,
                end: 12,
            }]
        );
        assert_eq!(
            WordPos::new("Hello, %% %s world!", &Language::Python).collect::<Vec<_>>(),
            vec![
                MatchStrPos {
                    s: "Hello",
                    start: 0,
                    end: 5,
                },
                MatchStrPos {
                    s: "world",
                    start: 13,
                    end: 18,
                },
            ]
        );
        assert_eq!(
            CharPos::new("Hé, %% %s w!", &Language::Python).collect::<Vec<_>>(),
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
                    start: 11,
                    end: 12,
                },
            ]
        );
        assert_eq!(
            UrlPos::new("Hello, world! -> %%https://example.com", &Language::Python)
                .collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "%https://example.com",
                start: 18,
                end: 38,
            }]
        );
    }

    #[test]
    fn test_escaped_brace() {
        assert_eq!(
            FormatPos::new("Hello, {{ {0} world!", &Language::PythonBrace).collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "{0}",
                start: 10,
                end: 13,
            }]
        );
        assert_eq!(
            WordPos::new("Hello, {{ {0} world!", &Language::PythonBrace).collect::<Vec<_>>(),
            vec![
                MatchStrPos {
                    s: "Hello",
                    start: 0,
                    end: 5,
                },
                MatchStrPos {
                    s: "world",
                    start: 14,
                    end: 19,
                },
            ]
        );
        assert_eq!(
            CharPos::new("Hé, {{ {0} w!", &Language::PythonBrace).collect::<Vec<_>>(),
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
                    start: 12,
                    end: 13,
                },
            ]
        );
        assert_eq!(
            UrlPos::new(
                "Hello, world! -> {{https://example.com",
                &Language::PythonBrace
            )
            .collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "{https://example.com",
                start: 18,
                end: 38,
            }]
        );
    }

    #[test]
    fn test_flags_width_precision() {
        assert_eq!(
            FormatPos::new("Hello, %05.2f world!", &Language::Python).collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "%05.2f",
                start: 7,
                end: 13,
            }]
        );
        assert_eq!(
            WordPos::new("Hello, %05.2f world!", &Language::Python).collect::<Vec<_>>(),
            vec![
                MatchStrPos {
                    s: "Hello",
                    start: 0,
                    end: 5,
                },
                MatchStrPos {
                    s: "world",
                    start: 14,
                    end: 19,
                },
            ]
        );
        assert_eq!(
            CharPos::new("Hé, %05.2f w!", &Language::Python).collect::<Vec<_>>(),
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
                    start: 12,
                    end: 13,
                },
            ]
        );
        assert_eq!(
            UrlPos::new("Hello, world! %05.2fhttps://example.com", &Language::Python)
                .collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "https://example.com",
                start: 20,
                end: 39,
            }]
        );
    }

    #[test]
    fn test_flags_width_length() {
        assert_eq!(
            FormatPos::new("Hello, %ld %9lu world!", &Language::Python).collect::<Vec<_>>(),
            vec![
                MatchStrPos {
                    s: "%ld",
                    start: 7,
                    end: 10,
                },
                MatchStrPos {
                    s: "%9lu",
                    start: 11,
                    end: 15,
                },
            ]
        );
        assert_eq!(
            WordPos::new("Hello, %ld %9lu world!", &Language::Python).collect::<Vec<_>>(),
            vec![
                MatchStrPos {
                    s: "Hello",
                    start: 0,
                    end: 5,
                },
                MatchStrPos {
                    s: "world",
                    start: 16,
                    end: 21,
                },
            ]
        );
        assert_eq!(
            CharPos::new("Hé, %ld %9lu w!", &Language::Python).collect::<Vec<_>>(),
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
                    start: 14,
                    end: 15,
                },
            ]
        );
        assert_eq!(
            UrlPos::new("Hello, world! %9luhttps://example.com", &Language::Python)
                .collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "https://example.com",
                start: 18,
                end: 37,
            }]
        );
    }

    #[test]
    fn test_unicode_percent() {
        assert_eq!(
            FormatPos::new("héllo, мир! %ld 你好", &Language::Python).collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "%ld",
                start: 16,
                end: 19,
            }]
        );
        assert_eq!(
            WordPos::new("héllo, мир! %ld 你好", &Language::Python).collect::<Vec<_>>(),
            vec![
                MatchStrPos {
                    s: "héllo",
                    start: 0,
                    end: 6,
                },
                MatchStrPos {
                    s: "мир",
                    start: 8,
                    end: 14,
                },
                MatchStrPos {
                    s: "你好",
                    start: 20,
                    end: 26,
                },
            ]
        );
        assert_eq!(
            UrlPos::new("héllo, мир! %ld 你好https://example.com", &Language::Python)
                .collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "你好https://example.com",
                start: 20,
                end: 45,
            }]
        );
    }

    #[test]
    fn test_unicode_brace() {
        assert_eq!(
            FormatPos::new("héllo, мир! {0} 你好", &Language::PythonBrace).collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "{0}",
                start: 16,
                end: 19,
            }]
        );
        assert_eq!(
            WordPos::new("héllo, мир! {0} 你好", &Language::PythonBrace).collect::<Vec<_>>(),
            vec![
                MatchStrPos {
                    s: "héllo",
                    start: 0,
                    end: 6,
                },
                MatchStrPos {
                    s: "мир",
                    start: 8,
                    end: 14,
                },
                MatchStrPos {
                    s: "你好",
                    start: 20,
                    end: 26,
                },
            ]
        );
        assert_eq!(
            UrlPos::new(
                "héllo, мир! {0} 你好https://example.com",
                &Language::PythonBrace
            )
            .collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "你好https://example.com",
                start: 20,
                end: 45,
            }]
        );
    }
}
