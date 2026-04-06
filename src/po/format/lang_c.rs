// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format strings: C language.

use crate::po::format::FormatParser;

pub struct FormatC;

impl FormatParser for FormatC {
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

        // Skip flags / width / precision / reordering.
        while pos_end < len {
            if matches!(
                bytes[pos_end],
                b'-' | b'+' | b' ' | b'#' | b'.' | b'$' | b'0'..=b'9'
            ) {
                pos_end += 1;
            } else {
                break;
            }
        }

        // Parse length modifiers (h, hh, l, ll, q, L, j, z, Z, t).
        if pos_end < len {
            match bytes[pos_end] {
                b'h' => {
                    pos_end += 1;
                    if pos_end < len && bytes[pos_end] == b'h' {
                        pos_end += 1;
                    }
                }
                b'l' => {
                    pos_end += 1;
                    if pos_end < len && bytes[pos_end] == b'l' {
                        pos_end += 1;
                    }
                }
                b'q' | b'L' | b'j' | b'z' | b'Z' | b't' => {
                    pos_end += 1;
                }
                _ => {}
            }
        }

        // Parse conversion specifier (e.g. s, d, f, etc.).
        if pos_end < len && bytes[pos_end].is_ascii_alphabetic() {
            pos_end += 1;
        }

        pos_end
    }
}

/// Get the reordering index if present, otherwise return `usize::MAX`.
///
/// For example, for format `"%3$d"`, this function returns `3`.
pub fn fmt_sort_index(fmt: &str) -> usize {
    let bytes = fmt.as_bytes();
    if bytes.is_empty() || bytes[0] != b'%' {
        return usize::MAX;
    }
    let mut pos = 1;
    while pos < bytes.len() && bytes[pos].is_ascii_digit() {
        pos += 1;
    }
    if pos == 1 || pos >= bytes.len() || bytes[pos] != b'$' {
        return usize::MAX;
    }
    match &fmt[1..pos].parse::<usize>() {
        Ok(index) => *index,
        Err(_) => usize::MAX,
    }
}

/// Return the format string without index (reordering part).
///
/// For example, for format `"%3$d"`, this function returns `"%d"`.
pub fn fmt_strip_index(fmt: &str) -> String {
    let bytes = fmt.as_bytes();
    if bytes.is_empty() || bytes[0] != b'%' {
        return fmt.to_string();
    }
    let mut pos = 1;
    while pos < bytes.len() && bytes[pos].is_ascii_digit() {
        pos += 1;
    }
    if pos == 1 || pos >= bytes.len() || bytes[pos] != b'$' {
        return fmt.to_string();
    }
    let mut result = String::from("%");
    result.push_str(&fmt[pos + 1..]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::po::format::{
        MatchFmtPos,
        iterators::{FormatPos, FormatUrlPos, FormatWordPos, strip_formats},
        language::Language,
    };

    #[test]
    fn test_sort_index() {
        assert_eq!(fmt_sort_index(""), usize::MAX);
        assert_eq!(fmt_sort_index("test"), usize::MAX);
        assert_eq!(fmt_sort_index("%d"), usize::MAX);
        assert_eq!(fmt_sort_index("%$d"), usize::MAX);
        assert_eq!(fmt_sort_index("%a$d"), usize::MAX);
        assert_eq!(
            fmt_sort_index("%99999999999999999999999999999999999999999999999999999999999999$d"),
            usize::MAX
        );
        assert_eq!(fmt_sort_index("%3$d"), 3);
        assert_eq!(fmt_sort_index("%42$05s"), 42);
    }

    #[test]
    fn test_remove_reordering() {
        assert_eq!(fmt_strip_index(""), "");
        assert_eq!(fmt_strip_index("test"), "test");
        assert_eq!(fmt_strip_index("%d"), "%d");
        assert_eq!(fmt_strip_index("%$d"), "%$d");
        assert_eq!(fmt_strip_index("%a$d"), "%a$d");
        assert_eq!(fmt_strip_index("%3$d"), "%d");
        assert_eq!(fmt_strip_index("%42$05s"), "%05s");
    }

    #[test]
    fn test_no_format() {
        let s = "Hello, world! https://example.com";
        assert!(FormatPos::new(s, &Language::C).next().is_none());
        assert_eq!(
            FormatWordPos::new(s, &Language::C).collect::<Vec<_>>(),
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
            FormatUrlPos::new(s, &Language::C).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "https://example.com",
                start: 14,
                end: 33,
            }]
        );
        assert_eq!(strip_formats(s, &Language::C), s);
    }

    #[test]
    fn test_invalid_format() {
        let s = "%";
        assert!(FormatPos::new(s, &Language::C).next().is_none());
        assert!(FormatWordPos::new(s, &Language::C).next().is_none());
        assert!(FormatUrlPos::new(s, &Language::C).next().is_none());
        assert_eq!(strip_formats(s, &Language::C), "%");

        let s = "%é";
        assert_eq!(
            FormatPos::new(s, &Language::C).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "%",
                start: 0,
                end: 1,
            }]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::C).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "é",
                start: 1,
                end: 3,
            }]
        );
        assert!(FormatUrlPos::new(s, &Language::C).next().is_none());
        assert_eq!(strip_formats(s, &Language::C), "é");
    }

    #[test]
    fn test_single_format() {
        let s = "Hello, %s world! https://example.com";
        assert_eq!(
            FormatPos::new(s, &Language::C).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "%s",
                start: 7,
                end: 9,
            }]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::C).collect::<Vec<_>>(),
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
            FormatUrlPos::new(s, &Language::C).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "https://example.com",
                start: 17,
                end: 36,
            }]
        );
        assert_eq!(
            strip_formats(s, &Language::C),
            "Hello,  world! https://example.com"
        );
    }

    #[test]
    fn test_multiple_formats() {
        let s = "Hello, %d%s%f world! https://%s.example.com";
        assert_eq!(
            FormatPos::new(s, &Language::C).collect::<Vec<_>>(),
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
            FormatWordPos::new(s, &Language::C).collect::<Vec<_>>(),
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
            FormatUrlPos::new(s, &Language::C).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "https://%s.example.com",
                start: 21,
                end: 43,
            }]
        );
        assert_eq!(
            strip_formats(s, &Language::C),
            "Hello,  world! https://.example.com"
        );
    }

    #[test]
    fn test_multiple_formats_with_reordering() {
        let s = "Hello, %3$d %2$s %1$f world! https://%s.example.com";
        assert_eq!(
            FormatPos::new(s, &Language::C).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "%3$d",
                    start: 7,
                    end: 11,
                },
                MatchFmtPos {
                    s: "%2$s",
                    start: 12,
                    end: 16,
                },
                MatchFmtPos {
                    s: "%1$f",
                    start: 17,
                    end: 21,
                },
                MatchFmtPos {
                    s: "%s",
                    start: 37,
                    end: 39,
                },
            ]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::C).collect::<Vec<_>>(),
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
                    start: 40,
                    end: 47,
                },
                MatchFmtPos {
                    s: "com",
                    start: 48,
                    end: 51,
                },
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::C).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "https://%s.example.com",
                start: 29,
                end: 51,
            }]
        );
        assert_eq!(
            strip_formats(s, &Language::C),
            "Hello,    world! https://.example.com"
        );
    }

    #[test]
    fn test_escaped_percent() {
        let s = "Hello, %% %s world! https://%s.example.com";
        assert_eq!(
            FormatPos::new(s, &Language::C).collect::<Vec<_>>(),
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
            FormatWordPos::new(s, &Language::C).collect::<Vec<_>>(),
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
            FormatUrlPos::new(s, &Language::C).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "https://%s.example.com",
                start: 20,
                end: 42,
            }]
        );
        assert_eq!(
            strip_formats(s, &Language::C),
            "Hello, %  world! https://.example.com"
        );
    }

    #[test]
    fn test_flags_width_precision() {
        let s = "Hello, %05.2f world! https://%s.example.com";
        assert_eq!(
            FormatPos::new(s, &Language::C).collect::<Vec<_>>(),
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
            FormatWordPos::new(s, &Language::C).collect::<Vec<_>>(),
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
            FormatUrlPos::new(s, &Language::C).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "https://%s.example.com",
                start: 21,
                end: 43,
            }]
        );
        assert_eq!(
            strip_formats(s, &Language::C),
            "Hello,  world! https://.example.com"
        );
    }

    #[test]
    fn test_flags_width_length() {
        let s = "Hello, %ld %9llu %hhd %zd world! https://%s.example.com";
        assert_eq!(
            FormatPos::new(s, &Language::C).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "%ld",
                    start: 7,
                    end: 10,
                },
                MatchFmtPos {
                    s: "%9llu",
                    start: 11,
                    end: 16,
                },
                MatchFmtPos {
                    s: "%hhd",
                    start: 17,
                    end: 21,
                },
                MatchFmtPos {
                    s: "%zd",
                    start: 22,
                    end: 25,
                },
                MatchFmtPos {
                    s: "%s",
                    start: 41,
                    end: 43,
                },
            ]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::C).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "Hello",
                    start: 0,
                    end: 5,
                },
                MatchFmtPos {
                    s: "world",
                    start: 26,
                    end: 31,
                },
                MatchFmtPos {
                    s: "https",
                    start: 33,
                    end: 38,
                },
                MatchFmtPos {
                    s: "example",
                    start: 44,
                    end: 51,
                },
                MatchFmtPos {
                    s: "com",
                    start: 52,
                    end: 55,
                },
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::C).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "https://%s.example.com",
                start: 33,
                end: 55,
            }]
        );
        assert_eq!(
            strip_formats(s, &Language::C),
            "Hello,     world! https://.example.com"
        );
    }

    #[test]
    fn test_unicode() {
        let s = "héllo, мир! %lld 你好 https://%s.example.com";
        assert_eq!(
            FormatPos::new(s, &Language::C).collect::<Vec<_>>(),
            vec![
                MatchFmtPos {
                    s: "%lld",
                    start: 16,
                    end: 20,
                },
                MatchFmtPos {
                    s: "%s",
                    start: 36,
                    end: 38,
                },
            ]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::C).collect::<Vec<_>>(),
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
                    start: 21,
                    end: 27,
                },
                MatchFmtPos {
                    s: "https",
                    start: 28,
                    end: 33,
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
            FormatUrlPos::new(s, &Language::C).collect::<Vec<_>>(),
            vec![MatchFmtPos {
                s: "https://%s.example.com",
                start: 28,
                end: 50,
            }]
        );
        assert_eq!(
            strip_formats(s, &Language::C),
            "héllo, мир!  你好 https://.example.com"
        );
    }
}
