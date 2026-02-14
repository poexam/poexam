// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format strings: C language.

use crate::po::format::FormatParser;

pub struct FormatC;

impl FormatParser for FormatC {
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
        MatchStrPos, char_pos::CharPos, format_pos::FormatPos, language::Language,
        word_pos::WordPos,
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
        // Formats: none.
        let s = "Hello, world!";
        assert!(FormatPos::new(s, &Language::C).next().is_none());
        // Words: "Hello", "world".
        let mut word_pos = WordPos::new(s, &Language::C);
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
        assert!(word_pos.next().is_none());
        // Chars: 'H', 'é', 'w'.
        let mut char_pos = CharPos::new("Hé, w!", &Language::C);
        assert_eq!(
            char_pos.next(),
            Some(MatchStrPos {
                s: "H",
                start: 0,
                end: 1,
            })
        );
        assert_eq!(
            char_pos.next(),
            Some(MatchStrPos {
                s: "é",
                start: 1,
                end: 3,
            })
        );
        assert_eq!(
            char_pos.next(),
            Some(MatchStrPos {
                s: "w",
                start: 5,
                end: 6,
            })
        );
        assert!(char_pos.next().is_none());
    }

    #[test]
    fn test_invalid_format() {
        let s = "%";
        // Formats: none.
        assert!(FormatPos::new(s, &Language::C).next().is_none());
        // Words: none.
        assert!(WordPos::new(s, &Language::C).next().is_none());
        // Chars: none.
        assert!(CharPos::new(s, &Language::C).next().is_none());

        let s = "%é";
        // Formats: "%".
        let mut fmt_pos = FormatPos::new(s, &Language::C);
        assert_eq!(
            fmt_pos.next(),
            Some(MatchStrPos {
                s: "%",
                start: 0,
                end: 1,
            })
        );
        assert!(fmt_pos.next().is_none());
        // Words: "é".
        let mut word_pos = WordPos::new(s, &Language::C);
        assert_eq!(
            word_pos.next(),
            Some(MatchStrPos {
                s: "é",
                start: 1,
                end: 3,
            })
        );
        assert!(word_pos.next().is_none());
        // Chars: 'é'.
        let mut char_pos = CharPos::new(s, &Language::C);
        assert_eq!(
            char_pos.next(),
            Some(MatchStrPos {
                s: "é",
                start: 1,
                end: 3,
            })
        );
        assert!(char_pos.next().is_none());
    }

    #[test]
    fn test_single_format() {
        let s = "Hello, %s world!";
        // Formats: "%s".
        let mut fmt_pos = FormatPos::new(s, &Language::C);
        assert_eq!(
            fmt_pos.next(),
            Some(MatchStrPos {
                s: "%s",
                start: 7,
                end: 9,
            })
        );
        assert!(fmt_pos.next().is_none());
        // Words: "Hello", "world".
        let mut word_pos = WordPos::new(s, &Language::C);
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
                start: 10,
                end: 15,
            })
        );
        assert!(word_pos.next().is_none());
        // Chars: : 'H', 'é', 'w'.
        let mut char_pos = CharPos::new("Hé, %s w!", &Language::C);
        assert_eq!(
            char_pos.next(),
            Some(MatchStrPos {
                s: "H",
                start: 0,
                end: 1,
            })
        );
        assert_eq!(
            char_pos.next(),
            Some(MatchStrPos {
                s: "é",
                start: 1,
                end: 3,
            })
        );
        assert_eq!(
            char_pos.next(),
            Some(MatchStrPos {
                s: "w",
                start: 8,
                end: 9,
            })
        );
        assert!(char_pos.next().is_none());
    }

    #[test]
    fn test_multiple_formats() {
        let s = "%d%s%f";
        // Formats: "%d", "%s", "%f".
        let mut fmt_pos = FormatPos::new(s, &Language::C);
        assert_eq!(
            fmt_pos.next(),
            Some(MatchStrPos {
                s: "%d",
                start: 0,
                end: 2,
            })
        );
        assert_eq!(
            fmt_pos.next(),
            Some(MatchStrPos {
                s: "%s",
                start: 2,
                end: 4,
            })
        );
        assert_eq!(
            fmt_pos.next(),
            Some(MatchStrPos {
                s: "%f",
                start: 4,
                end: 6,
            })
        );
        assert!(fmt_pos.next().is_none());
        // Words: none.
        assert!(WordPos::new(s, &Language::C).next().is_none());
        // Chars: none.
        assert!(CharPos::new(s, &Language::C).next().is_none());
    }

    #[test]
    fn test_multiple_formats_with_reordering() {
        let s = "Hello, %3$d %2$s %1$f world!";
        // Formats: "%3$d", "%2$s", "%1$f".
        let mut fmt_pos = FormatPos::new(s, &Language::C);
        assert_eq!(
            fmt_pos.next(),
            Some(MatchStrPos {
                s: "%3$d",
                start: 7,
                end: 11,
            })
        );
        assert_eq!(
            fmt_pos.next(),
            Some(MatchStrPos {
                s: "%2$s",
                start: 12,
                end: 16,
            })
        );
        assert_eq!(
            fmt_pos.next(),
            Some(MatchStrPos {
                s: "%1$f",
                start: 17,
                end: 21,
            })
        );
        assert!(fmt_pos.next().is_none());
        // Words: "Hello", "world".
        let mut word_pos = WordPos::new(s, &Language::C);
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
                start: 22,
                end: 27,
            })
        );
        assert!(word_pos.next().is_none());
        // Chars: 'H', 'é', 'w'.
        let mut char_pos = CharPos::new("Hé, %3$d %2$s %1$f w!", &Language::C);
        assert_eq!(
            char_pos.next(),
            Some(MatchStrPos {
                s: "H",
                start: 0,
                end: 1,
            })
        );
        assert_eq!(
            char_pos.next(),
            Some(MatchStrPos {
                s: "é",
                start: 1,
                end: 3,
            })
        );
        assert_eq!(
            char_pos.next(),
            Some(MatchStrPos {
                s: "w",
                start: 20,
                end: 21,
            })
        );
        assert!(char_pos.next().is_none());
    }

    #[test]
    fn test_escaped_percent() {
        let s = "Hello, %% %s world!";
        // Formats: "%s".
        let mut fmt_pos = FormatPos::new(s, &Language::C);
        assert_eq!(
            fmt_pos.next(),
            Some(MatchStrPos {
                s: "%s",
                start: 10,
                end: 12,
            })
        );
        assert!(fmt_pos.next().is_none());
        // Words: "Hello", "world".
        let mut word_pos = WordPos::new(s, &Language::C);
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
                start: 13,
                end: 18,
            })
        );
        assert!(word_pos.next().is_none());
        // Chars: 'H', 'é', 'w'.
        let mut char_pos = CharPos::new("Hé, %% %s w!", &Language::C);
        assert_eq!(
            char_pos.next(),
            Some(MatchStrPos {
                s: "H",
                start: 0,
                end: 1,
            })
        );
        assert_eq!(
            char_pos.next(),
            Some(MatchStrPos {
                s: "é",
                start: 1,
                end: 3,
            })
        );
        assert_eq!(
            char_pos.next(),
            Some(MatchStrPos {
                s: "w",
                start: 11,
                end: 12,
            })
        );
        assert!(char_pos.next().is_none());
    }

    #[test]
    fn test_flags_width_precision() {
        let s = "Hello, %05.2f world!";
        // Formats: "%05.2f".
        let mut fmt_pos = FormatPos::new(s, &Language::C);
        assert_eq!(
            fmt_pos.next(),
            Some(MatchStrPos {
                s: "%05.2f",
                start: 7,
                end: 13,
            })
        );
        assert!(fmt_pos.next().is_none());
        // Words: "Hello", "world".
        let mut word_pos = WordPos::new(s, &Language::C);
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
                start: 14,
                end: 19,
            })
        );
        assert!(word_pos.next().is_none());
        // Chars: 'H', 'é', 'w'.
        let mut char_pos = CharPos::new("Hé, %05.2f w!", &Language::C);
        assert_eq!(
            char_pos.next(),
            Some(MatchStrPos {
                s: "H",
                start: 0,
                end: 1,
            })
        );
        assert_eq!(
            char_pos.next(),
            Some(MatchStrPos {
                s: "é",
                start: 1,
                end: 3,
            })
        );
        assert_eq!(
            char_pos.next(),
            Some(MatchStrPos {
                s: "w",
                start: 12,
                end: 13,
            })
        );
        assert!(char_pos.next().is_none());
    }

    #[test]
    fn test_flags_width_length() {
        let s = "Hello, %ld %9llu world!";
        // Formats: "%ld", "%9llu".
        let mut fmt_pos = FormatPos::new(s, &Language::C);
        assert_eq!(
            fmt_pos.next(),
            Some(MatchStrPos {
                s: "%ld",
                start: 7,
                end: 10,
            })
        );
        assert_eq!(
            fmt_pos.next(),
            Some(MatchStrPos {
                s: "%9llu",
                start: 11,
                end: 16,
            })
        );
        assert!(fmt_pos.next().is_none());
        // Words: "Hello", "world".
        let mut word_pos = WordPos::new(s, &Language::C);
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
                start: 17,
                end: 22,
            })
        );
        assert!(word_pos.next().is_none());
        // Chars: 'H', 'é', 'w'.
        let mut char_pos = CharPos::new("Hé, %ld %9llu w!", &Language::C);
        assert_eq!(
            char_pos.next(),
            Some(MatchStrPos {
                s: "H",
                start: 0,
                end: 1,
            })
        );
        assert_eq!(
            char_pos.next(),
            Some(MatchStrPos {
                s: "é",
                start: 1,
                end: 3,
            })
        );
        assert_eq!(
            char_pos.next(),
            Some(MatchStrPos {
                s: "w",
                start: 15,
                end: 16,
            })
        );
        assert!(char_pos.next().is_none());
    }

    #[test]
    fn test_unicode() {
        let s = "héllo, мир! %lld 你好";
        // Formats: "%lld".
        let mut fmt_pos = FormatPos::new(s, &Language::C);
        assert_eq!(
            fmt_pos.next(),
            Some(MatchStrPos {
                s: "%lld",
                start: 16,
                end: 20,
            })
        );
        assert!(fmt_pos.next().is_none());
        // Words: "héllo", "мир", "你好".
        let mut word_pos = WordPos::new(s, &Language::C);
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
                s: "你好",
                start: 21,
                end: 27,
            })
        );
        assert!(word_pos.next().is_none());
    }
}
