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
        MatchStrPos,
        format_pos::{FormatPos, strip_formats},
        language::Language,
    };

    #[test]
    fn test_sort_index() {
        assert_eq!(fmt_sort_index(""), usize::MAX);
        assert_eq!(fmt_sort_index("test"), usize::MAX);
        assert_eq!(fmt_sort_index("%d"), usize::MAX);
        assert_eq!(fmt_sort_index("%$d"), usize::MAX);
        assert_eq!(fmt_sort_index("%a$d"), usize::MAX);
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
        assert!(
            FormatPos::new("Hello, world!", &Language::C)
                .next()
                .is_none()
        );
        assert_eq!(
            strip_formats("Hello, world!", &Language::C),
            "Hello, world!"
        );
    }

    #[test]
    fn test_invalid_format() {
        assert!(FormatPos::new("%", &Language::C).next().is_none());
        assert_eq!(strip_formats("%", &Language::C), "%");
        assert_eq!(
            FormatPos::new("%é", &Language::C).collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "%",
                start: 0,
                end: 1,
            }]
        );
        assert_eq!(strip_formats("%é", &Language::C), "é");
    }

    #[test]
    fn test_single_format() {
        assert_eq!(
            FormatPos::new("Hello, %s world!", &Language::C).collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "%s",
                start: 7,
                end: 9,
            }]
        );
        assert_eq!(
            strip_formats("Hello, %s world!", &Language::C),
            "Hello,  world!"
        );
    }

    #[test]
    fn test_multiple_formats() {
        assert_eq!(
            FormatPos::new("%d%s%f", &Language::C).collect::<Vec<_>>(),
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
        assert!(strip_formats("%d%s%f", &Language::C).is_empty());
    }

    #[test]
    fn test_multiple_formats_with_reordering() {
        assert_eq!(
            FormatPos::new("Hello, %3$d %2$s %1$f world!", &Language::C).collect::<Vec<_>>(),
            vec![
                MatchStrPos {
                    s: "%3$d",
                    start: 7,
                    end: 11,
                },
                MatchStrPos {
                    s: "%2$s",
                    start: 12,
                    end: 16,
                },
                MatchStrPos {
                    s: "%1$f",
                    start: 17,
                    end: 21,
                },
            ]
        );
        assert_eq!(
            strip_formats("Hello, %3$d %2$s %1$f world!", &Language::C),
            "Hello,    world!"
        );
    }

    #[test]
    fn test_escaped_percent() {
        assert_eq!(
            FormatPos::new("Hello, %% %s world!", &Language::C).collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "%s",
                start: 10,
                end: 12,
            }]
        );
        assert_eq!(
            strip_formats("Hello, %% %s world!", &Language::C),
            "Hello, %  world!"
        );
    }

    #[test]
    fn test_flags_width_precision() {
        assert_eq!(
            FormatPos::new("Hello, %05.2f world!", &Language::C).collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "%05.2f",
                start: 7,
                end: 13,
            }]
        );
        assert_eq!(
            strip_formats("Hello, %05.2f world!", &Language::C),
            "Hello,  world!"
        );
    }

    #[test]
    fn test_flags_width_length() {
        assert_eq!(
            FormatPos::new("Hello, %ld %9llu world!", &Language::C).collect::<Vec<_>>(),
            vec![
                MatchStrPos {
                    s: "%ld",
                    start: 7,
                    end: 10,
                },
                MatchStrPos {
                    s: "%9llu",
                    start: 11,
                    end: 16,
                },
            ]
        );
        assert_eq!(
            strip_formats("Hello, %ld %9llu world!", &Language::C),
            "Hello,   world!"
        );
    }

    #[test]
    fn test_unicode() {
        assert_eq!(
            FormatPos::new("héllo, мир! %lld 你好", &Language::C).collect::<Vec<_>>(),
            vec![MatchStrPos {
                s: "%lld",
                start: 16,
                end: 20,
            }]
        );
        assert_eq!(
            strip_formats("héllo, мир! %lld 你好", &Language::C),
            "héllo, мир!  你好"
        );
    }
}
