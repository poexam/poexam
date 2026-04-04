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
        MatchStrPos,
        format_pos::{FormatPos, strip_formats},
        language::Language,
    };

    #[test]
    fn test_no_format_percent() {
        assert!(
            FormatPos::new("Hello, world!", &Language::Python)
                .next()
                .is_none()
        );
        assert_eq!(
            strip_formats("Hello, world!", &Language::Python),
            "Hello, world!"
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
            strip_formats("Hello, world!", &Language::PythonBrace),
            "Hello, world!"
        );
    }

    #[test]
    fn test_invalid_format_percent() {
        assert!(FormatPos::new("%", &Language::Python).next().is_none());
        assert_eq!(strip_formats("%", &Language::Python), "%");
        assert_eq!(
            FormatPos::new("%é", &Language::Python).collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "%",
                start: 0,
                end: 1,
            }]
        );
        assert_eq!(strip_formats("%é", &Language::Python), "é");
        assert_eq!(
            FormatPos::new("%(test", &Language::Python).collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "%(test",
                start: 0,
                end: 6,
            }]
        );
        assert!(strip_formats("%(test", &Language::Python).is_empty());
    }

    #[test]
    fn test_invalid_format_brace() {
        assert!(FormatPos::new("{", &Language::PythonBrace).next().is_none());
        assert_eq!(strip_formats("{", &Language::PythonBrace), "{");
        assert_eq!(
            FormatPos::new("{é", &Language::PythonBrace).collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "{é",
                start: 0,
                end: 3,
            }]
        );
        assert!(strip_formats("{é", &Language::PythonBrace).is_empty());
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
            strip_formats("Hello, %s world!", &Language::Python),
            "Hello,  world!"
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
            strip_formats("Hello, {0:{1}} world!", &Language::PythonBrace),
            "Hello,  world!"
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
            strip_formats("Hello, %(name)s world!", &Language::Python),
            "Hello,  world!"
        );
    }

    #[test]
    fn test_multiple_formats_percent() {
        assert_eq!(
            FormatPos::new("%d%s%f", &Language::Python).collect::<Vec<_>>(),
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
        assert!(strip_formats("%d%s%f", &Language::Python).is_empty());
    }

    #[test]
    fn test_multiple_formats_brace() {
        assert_eq!(
            FormatPos::new("{0!r:20}{1}{2}", &Language::PythonBrace).collect::<Vec<_>>(),
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
        assert!(strip_formats("{0!r:20}{1}{2}", &Language::PythonBrace).is_empty());
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
            strip_formats("Hello, %% %s world!", &Language::Python),
            "Hello, %  world!"
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
            strip_formats("Hello, {{ {0} world!", &Language::PythonBrace),
            "Hello, {  world!"
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
            strip_formats("Hello, %05.2f world!", &Language::Python),
            "Hello,  world!"
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
            strip_formats("Hello, %ld %9lu world!", &Language::Python),
            "Hello,   world!"
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
            strip_formats("héllo, мир! %ld 你好", &Language::Python),
            "héllo, мир!  你好"
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
            strip_formats("héllo, мир! {0} 你好", &Language::PythonBrace),
            "héllo, мир!  你好"
        );
    }
}
