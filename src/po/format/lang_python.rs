// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format strings: Python language.

use crate::po::format::FormatParser;

pub struct FormatPython;

impl FormatParser for FormatPython {
    #[inline]
    fn next_char(&self, s: &str, pos: usize) -> Option<(char, usize, bool)> {
        match s[pos..].chars().next() {
            Some('%') => match s[pos + 1..].chars().next() {
                // Escaped percent: "%%" is not a format string.
                Some('%') => Some(('%', pos + 2, false)),
                // Start of a format string.
                Some(_) => Some(('%', pos + 1, true)),
                // Invalid format string: '%' at the end of the string.
                None => Some(('%', pos + 1, false)),
            },
            // Other character: not a format string.
            Some(c) => Some((c, pos + c.len_utf8(), false)),
            // End of string: no more character.
            None => None,
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
    fn next_char(&self, s: &str, pos: usize) -> Option<(char, usize, bool)> {
        match s[pos..].chars().next() {
            Some('{') => match s[pos + 1..].chars().next() {
                // Escaped brace: "{{" is not a format string.
                Some('{') => Some(('{', pos + 2, false)),
                // Start of a format string.
                Some(_) => Some(('{', pos + 1, true)),
                // Invalid format string: '{' at the end of the string.
                None => Some(('{', pos + 1, false)),
            },
            // Other character: not a format string.
            Some(c) => Some((c, pos + c.len_utf8(), false)),
            // End of string: no more character.
            None => None,
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
        MatchFmtPos,
        iterators::{FormatPos, FormatUrlPos, FormatWordPos, strip_formats},
        language::Language,
    };

    #[test]
    fn test_no_format_percent() {
        let s = "Hello, world! https://example.com";
        assert!(FormatPos::new(s, &Language::Python).next().is_none());
        assert_eq!(
            FormatWordPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "Hello",
                    start: 0,
                    end: 5,
                },
                MatchFmtPos {
                    s: "world",
                    start: 7,
                    end: 12,
                },
                MatchFmtPos {
                    s: "https",
                    start: 14,
                    end: 19,
                },
                MatchFmtPos {
                    s: "example",
                    start: 22,
                    end: 29,
                },
                MatchFmtPos {
                    s: "com",
                    start: 30,
                    end: 33,
                },
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "https://example.com",
                start: 14,
                end: 33,
            }]
        );
        assert_eq!(strip_formats(s, &Language::Python), s);
    }

    #[test]
    fn test_no_format_brace() {
        let s = "Hello, world! https://example.com";
        assert!(FormatPos::new(s, &Language::PythonBrace).next().is_none());
        assert_eq!(
            FormatWordPos::new(s, &Language::PythonBrace).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "Hello",
                    start: 0,
                    end: 5,
                },
                MatchFmtPos {
                    s: "world",
                    start: 7,
                    end: 12,
                },
                MatchFmtPos {
                    s: "https",
                    start: 14,
                    end: 19,
                },
                MatchFmtPos {
                    s: "example",
                    start: 22,
                    end: 29,
                },
                MatchFmtPos {
                    s: "com",
                    start: 30,
                    end: 33,
                },
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::PythonBrace).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "https://example.com",
                start: 14,
                end: 33,
            }]
        );
        assert_eq!(strip_formats(s, &Language::PythonBrace), s);
    }

    #[test]
    fn test_invalid_format_percent() {
        let s = "%";
        assert!(FormatPos::new(s, &Language::Python).next().is_none());
        assert!(FormatWordPos::new(s, &Language::Python).next().is_none());
        assert!(FormatUrlPos::new(s, &Language::Python).next().is_none());
        assert_eq!(strip_formats(s, &Language::Python), s);

        let s = "%é";
        assert_eq!(
            FormatPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "%",
                start: 0,
                end: 1,
            }]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "é",
                start: 1,
                end: 3,
            }]
        );
        assert!(FormatUrlPos::new(s, &Language::Python).next().is_none());
        assert_eq!(strip_formats(s, &Language::Python), "é");

        let s = "%(test";
        assert_eq!(
            FormatPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "%(test",
                start: 0,
                end: 6,
            }]
        );
        assert!(FormatWordPos::new(s, &Language::Python).next().is_none());
        assert!(FormatUrlPos::new(s, &Language::Python).next().is_none());
        assert!(strip_formats(s, &Language::Python).is_empty());
    }

    #[test]
    fn test_invalid_format_brace() {
        let s = "{";
        assert!(FormatPos::new(s, &Language::PythonBrace).next().is_none());
        assert!(
            FormatWordPos::new(s, &Language::PythonBrace)
                .next()
                .is_none()
        );
        assert!(
            FormatUrlPos::new(s, &Language::PythonBrace)
                .next()
                .is_none()
        );
        assert_eq!(strip_formats(s, &Language::PythonBrace), s);

        let s = "{é";
        assert_eq!(
            FormatPos::new(s, &Language::PythonBrace).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "{é",
                start: 0,
                end: 3,
            }]
        );
        assert!(
            FormatWordPos::new(s, &Language::PythonBrace)
                .next()
                .is_none()
        );
        assert!(
            FormatUrlPos::new(s, &Language::PythonBrace)
                .next()
                .is_none()
        );
        assert!(strip_formats(s, &Language::PythonBrace).is_empty());
    }

    #[test]
    fn test_single_format_percent() {
        let s = "Hello, %s world! https://example.com";
        assert_eq!(
            FormatPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "%s",
                start: 7,
                end: 9,
            }]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "Hello",
                    start: 0,
                    end: 5,
                },
                MatchFmtPos {
                    s: "world",
                    start: 10,
                    end: 15,
                },
                MatchFmtPos {
                    s: "https",
                    start: 17,
                    end: 22,
                },
                MatchFmtPos {
                    s: "example",
                    start: 25,
                    end: 32,
                },
                MatchFmtPos {
                    s: "com",
                    start: 33,
                    end: 36,
                },
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "https://example.com",
                start: 17,
                end: 36,
            }]
        );
        assert_eq!(
            strip_formats(s, &Language::Python),
            "Hello,  world! https://example.com"
        );
    }

    #[test]
    fn test_single_format_brace() {
        let s = "Hello, {0:{1}} world! https://example.com";
        assert_eq!(
            FormatPos::new(s, &Language::PythonBrace).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "{0:{1}}",
                start: 7,
                end: 14,
            }]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::PythonBrace).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "Hello",
                    start: 0,
                    end: 5,
                },
                MatchFmtPos {
                    s: "world",
                    start: 15,
                    end: 20,
                },
                MatchFmtPos {
                    s: "https",
                    start: 22,
                    end: 27,
                },
                MatchFmtPos {
                    s: "example",
                    start: 30,
                    end: 37,
                },
                MatchFmtPos {
                    s: "com",
                    start: 38,
                    end: 41,
                },
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::PythonBrace).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "https://example.com",
                start: 22,
                end: 41,
            }]
        );
        assert_eq!(
            strip_formats(s, &Language::PythonBrace),
            "Hello,  world! https://example.com"
        );
    }

    #[test]
    fn test_single_format_percent_keyword() {
        let s = "Hello, %(name)s world! https://example.com";
        assert_eq!(
            FormatPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "%(name)s",
                start: 7,
                end: 15,
            }]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "Hello",
                    start: 0,
                    end: 5,
                },
                MatchFmtPos {
                    s: "world",
                    start: 16,
                    end: 21,
                },
                MatchFmtPos {
                    s: "https",
                    start: 23,
                    end: 28,
                },
                MatchFmtPos {
                    s: "example",
                    start: 31,
                    end: 38,
                },
                MatchFmtPos {
                    s: "com",
                    start: 39,
                    end: 42,
                },
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "https://example.com",
                start: 23,
                end: 42,
            }]
        );
        assert_eq!(
            strip_formats(s, &Language::Python),
            "Hello,  world! https://example.com"
        );
    }

    #[test]
    fn test_multiple_formats_percent() {
        let s = "Hello, %d%s%f world! https://%s.example.com";
        assert_eq!(
            FormatPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "%d",
                    start: 7,
                    end: 9,
                },
                MatchFmtPos {
                    s: "%s",
                    start: 9,
                    end: 11,
                },
                MatchFmtPos {
                    s: "%f",
                    start: 11,
                    end: 13,
                },
                MatchFmtPos {
                    s: "%s",
                    start: 29,
                    end: 31,
                },
            ]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "Hello",
                    start: 0,
                    end: 5,
                },
                MatchFmtPos {
                    s: "world",
                    start: 14,
                    end: 19,
                },
                MatchFmtPos {
                    s: "https",
                    start: 21,
                    end: 26,
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
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "https://%s.example.com",
                start: 21,
                end: 43,
            }]
        );
        assert_eq!(
            strip_formats(s, &Language::Python),
            "Hello,  world! https://.example.com"
        );
    }

    #[test]
    fn test_multiple_formats_brace() {
        let s = "Hello, {0!r:20}{1}{2} world! https://{3}.example.com";
        assert_eq!(
            FormatPos::new(s, &Language::PythonBrace).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "{0!r:20}",
                    start: 7,
                    end: 15,
                },
                MatchFmtPos {
                    s: "{1}",
                    start: 15,
                    end: 18,
                },
                MatchFmtPos {
                    s: "{2}",
                    start: 18,
                    end: 21,
                },
                MatchFmtPos {
                    s: "{3}",
                    start: 37,
                    end: 40,
                },
            ]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::PythonBrace).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "Hello",
                    start: 0,
                    end: 5,
                },
                MatchFmtPos {
                    s: "world",
                    start: 22,
                    end: 27,
                },
                MatchFmtPos {
                    s: "https",
                    start: 29,
                    end: 34,
                },
                MatchFmtPos {
                    s: "example",
                    start: 41,
                    end: 48,
                },
                MatchFmtPos {
                    s: "com",
                    start: 49,
                    end: 52,
                },
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::PythonBrace).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "https://{3}.example.com",
                start: 29,
                end: 52,
            }]
        );
        assert_eq!(
            strip_formats(s, &Language::PythonBrace),
            "Hello,  world! https://.example.com"
        );
    }

    #[test]
    fn test_escaped_percent() {
        let s = "Hello, %% %s world! https://%s.example.com";
        assert_eq!(
            FormatPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "%s",
                    start: 10,
                    end: 12,
                },
                MatchFmtPos {
                    s: "%s",
                    start: 28,
                    end: 30,
                },
            ]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "Hello",
                    start: 0,
                    end: 5,
                },
                MatchFmtPos {
                    s: "world",
                    start: 13,
                    end: 18,
                },
                MatchFmtPos {
                    s: "https",
                    start: 20,
                    end: 25,
                },
                MatchFmtPos {
                    s: "example",
                    start: 31,
                    end: 38,
                },
                MatchFmtPos {
                    s: "com",
                    start: 39,
                    end: 42,
                },
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "https://%s.example.com",
                start: 20,
                end: 42,
            }]
        );
        assert_eq!(
            strip_formats(s, &Language::Python),
            "Hello, %  world! https://.example.com"
        );
    }

    #[test]
    fn test_escaped_brace() {
        let s = "Hello, {{ {0} world! https://{1}.example.com";
        assert_eq!(
            FormatPos::new(s, &Language::PythonBrace).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "{0}",
                    start: 10,
                    end: 13,
                },
                MatchFmtPos {
                    s: "{1}",
                    start: 29,
                    end: 32,
                },
            ]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::PythonBrace).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "Hello",
                    start: 0,
                    end: 5,
                },
                MatchFmtPos {
                    s: "world",
                    start: 14,
                    end: 19,
                },
                MatchFmtPos {
                    s: "https",
                    start: 21,
                    end: 26,
                },
                MatchFmtPos {
                    s: "example",
                    start: 33,
                    end: 40,
                },
                MatchFmtPos {
                    s: "com",
                    start: 41,
                    end: 44,
                },
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::PythonBrace).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "https://{1}.example.com",
                start: 21,
                end: 44,
            }]
        );
        assert_eq!(
            strip_formats(s, &Language::PythonBrace),
            "Hello, {  world! https://.example.com"
        );
    }

    #[test]
    fn test_flags_width_precision() {
        let s = "Hello, %05.2f world! https://%s.example.com";
        assert_eq!(
            FormatPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "%05.2f",
                    start: 7,
                    end: 13,
                },
                MatchFmtPos {
                    s: "%s",
                    start: 29,
                    end: 31,
                },
            ]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "Hello",
                    start: 0,
                    end: 5,
                },
                MatchFmtPos {
                    s: "world",
                    start: 14,
                    end: 19,
                },
                MatchFmtPos {
                    s: "https",
                    start: 21,
                    end: 26,
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
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "https://%s.example.com",
                start: 21,
                end: 43,
            }]
        );
        assert_eq!(
            strip_formats(s, &Language::Python),
            "Hello,  world! https://.example.com"
        );
    }

    #[test]
    fn test_flags_width_length() {
        let s = "Hello, %ld %9lu world! https://%s.example.com";
        assert_eq!(
            FormatPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "%ld",
                    start: 7,
                    end: 10,
                },
                MatchFmtPos {
                    s: "%9lu",
                    start: 11,
                    end: 15,
                },
                MatchFmtPos {
                    s: "%s",
                    start: 31,
                    end: 33,
                },
            ]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "Hello",
                    start: 0,
                    end: 5,
                },
                MatchFmtPos {
                    s: "world",
                    start: 16,
                    end: 21,
                },
                MatchFmtPos {
                    s: "https",
                    start: 23,
                    end: 28,
                },
                MatchFmtPos {
                    s: "example",
                    start: 34,
                    end: 41,
                },
                MatchFmtPos {
                    s: "com",
                    start: 42,
                    end: 45,
                },
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "https://%s.example.com",
                start: 23,
                end: 45,
            }]
        );
        assert_eq!(
            strip_formats(s, &Language::Python),
            "Hello,   world! https://.example.com"
        );
    }

    #[test]
    fn test_unicode_percent() {
        let s = "héllo, мир! %ld 你好 https://%s.example.com";
        assert_eq!(
            FormatPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "%ld",
                    start: 16,
                    end: 19,
                },
                MatchFmtPos {
                    s: "%s",
                    start: 35,
                    end: 37,
                },
            ]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "héllo",
                    start: 0,
                    end: 6,
                },
                MatchFmtPos {
                    s: "мир",
                    start: 8,
                    end: 14,
                },
                MatchFmtPos {
                    s: "你好",
                    start: 20,
                    end: 26,
                },
                MatchFmtPos {
                    s: "https",
                    start: 27,
                    end: 32,
                },
                MatchFmtPos {
                    s: "example",
                    start: 38,
                    end: 45,
                },
                MatchFmtPos {
                    s: "com",
                    start: 46,
                    end: 49,
                },
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::Python).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "https://%s.example.com",
                start: 27,
                end: 49,
            }]
        );
        assert_eq!(
            strip_formats(s, &Language::Python),
            "héllo, мир!  你好 https://.example.com"
        );
    }

    #[test]
    fn test_unicode_brace() {
        let s = "héllo, мир! {0} 你好 https://{1}.example.com";
        assert_eq!(
            FormatPos::new(s, &Language::PythonBrace).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "{0}",
                    start: 16,
                    end: 19,
                },
                MatchFmtPos {
                    s: "{1}",
                    start: 35,
                    end: 38,
                },
            ]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::PythonBrace).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "héllo",
                    start: 0,
                    end: 6,
                },
                MatchFmtPos {
                    s: "мир",
                    start: 8,
                    end: 14,
                },
                MatchFmtPos {
                    s: "你好",
                    start: 20,
                    end: 26,
                },
                MatchFmtPos {
                    s: "https",
                    start: 27,
                    end: 32,
                },
                MatchFmtPos {
                    s: "example",
                    start: 39,
                    end: 46,
                },
                MatchFmtPos {
                    s: "com",
                    start: 47,
                    end: 50,
                },
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::PythonBrace).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "https://{1}.example.com",
                start: 27,
                end: 50,
            }]
        );
        assert_eq!(
            strip_formats(s, &Language::PythonBrace),
            "héllo, мир!  你好 https://.example.com"
        );
    }
}
