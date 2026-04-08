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
        iter::{FormatEmailPos, FormatPos, FormatUrlPos, FormatWordPos},
        language::Language,
        strip_formats,
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
        let s = "Hello, world! https://example.com user@domain.com";
        assert!(FormatPos::new(s, &Language::C).next().is_none());
        assert_eq!(
            FormatWordPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![
                ("Hello", 0, 5),
                ("world", 7, 12),
                ("https", 14, 19),
                ("example", 22, 29),
                ("com", 30, 33),
                ("user", 34, 38),
                ("domain", 39, 45),
                ("com", 46, 49),
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://example.com", 14, 33),]
        );
        assert_eq!(
            FormatEmailPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("user@domain.com", 34, 49),]
        );
        assert_eq!(strip_formats(s, &Language::C), s);
    }

    #[test]
    fn test_invalid_format() {
        let s = "%";
        assert!(FormatPos::new(s, &Language::C).next().is_none());
        assert!(FormatWordPos::new(s, &Language::C).next().is_none());
        assert!(FormatUrlPos::new(s, &Language::C).next().is_none());
        assert!(FormatEmailPos::new(s, &Language::C).next().is_none());
        assert_eq!(strip_formats(s, &Language::C), "%");

        let s = "%é";
        assert_eq!(
            FormatPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("%", 0, 1),]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("é", 1, 3),]
        );
        assert!(FormatUrlPos::new(s, &Language::C).next().is_none());
        assert!(FormatEmailPos::new(s, &Language::C).next().is_none());
        assert_eq!(strip_formats(s, &Language::C), "é");
    }

    #[test]
    fn test_single_format() {
        let s = "Hello, %s world! https://example.com user@domain.com";
        assert_eq!(
            FormatPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("%s", 7, 9)]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![
                ("Hello", 0, 5),
                ("world", 10, 15),
                ("https", 17, 22),
                ("example", 25, 32),
                ("com", 33, 36),
                ("user", 37, 41),
                ("domain", 42, 48),
                ("com", 49, 52),
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://example.com", 17, 36),]
        );
        assert_eq!(
            FormatEmailPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("user@domain.com", 37, 52),]
        );
        assert_eq!(
            strip_formats(s, &Language::C),
            "Hello,  world! https://example.com user@domain.com"
        );
    }

    #[test]
    fn test_multiple_formats() {
        let s = "Hello, %d%s%f world! https://%s.example.com user@%s.domain.com";
        assert_eq!(
            FormatPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![
                ("%d", 7, 9),
                ("%s", 9, 11),
                ("%f", 11, 13),
                ("%s", 29, 31),
                ("%s", 49, 51),
            ]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![
                ("Hello", 0, 5),
                ("world", 14, 19),
                ("https", 21, 26),
                ("example", 32, 39),
                ("com", 40, 43),
                ("user", 44, 48),
                ("domain", 52, 58),
                ("com", 59, 62),
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://%s.example.com", 21, 43),]
        );
        assert_eq!(
            FormatEmailPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("user@%s.domain.com", 44, 62),]
        );
        assert_eq!(
            strip_formats(s, &Language::C),
            "Hello,  world! https://.example.com user@.domain.com"
        );
    }

    #[test]
    fn test_multiple_formats_with_reordering() {
        let s = "Hello, %3$d %2$s %1$f world! https://%s.example.com user@%s.domain.com";
        assert_eq!(
            FormatPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![
                ("%3$d", 7, 11),
                ("%2$s", 12, 16),
                ("%1$f", 17, 21),
                ("%s", 37, 39),
                ("%s", 57, 59),
            ]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![
                ("Hello", 0, 5),
                ("world", 22, 27),
                ("https", 29, 34),
                ("example", 40, 47),
                ("com", 48, 51),
                ("user", 52, 56),
                ("domain", 60, 66),
                ("com", 67, 70),
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://%s.example.com", 29, 51),]
        );
        assert_eq!(
            strip_formats(s, &Language::C),
            "Hello,    world! https://.example.com user@.domain.com"
        );
    }

    #[test]
    fn test_escaped_percent() {
        let s = "Hello, %% %s world! https://%s.example.com user@%s.domain.com";
        assert_eq!(
            FormatPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("%s", 10, 12), ("%s", 28, 30), ("%s", 48, 50),]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![
                ("Hello", 0, 5),
                ("world", 13, 18),
                ("https", 20, 25),
                ("example", 31, 38),
                ("com", 39, 42),
                ("user", 43, 47),
                ("domain", 51, 57),
                ("com", 58, 61),
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://%s.example.com", 20, 42),]
        );
        assert_eq!(
            FormatEmailPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("user@%s.domain.com", 43, 61),]
        );
        assert_eq!(
            strip_formats(s, &Language::C),
            "Hello, %  world! https://.example.com user@.domain.com"
        );
    }

    #[test]
    fn test_flags_width_precision() {
        let s = "Hello, %05.2f world! https://%s.example.com user@%s.domain.com";
        assert_eq!(
            FormatPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("%05.2f", 7, 13), ("%s", 29, 31), ("%s", 49, 51),]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![
                ("Hello", 0, 5),
                ("world", 14, 19),
                ("https", 21, 26),
                ("example", 32, 39),
                ("com", 40, 43),
                ("user", 44, 48),
                ("domain", 52, 58),
                ("com", 59, 62),
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://%s.example.com", 21, 43),]
        );
        assert_eq!(
            FormatEmailPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("user@%s.domain.com", 44, 62),]
        );
        assert_eq!(
            strip_formats(s, &Language::C),
            "Hello,  world! https://.example.com user@.domain.com"
        );
    }

    #[test]
    fn test_flags_width_length() {
        let s = "Hello, %ld %9llu %hhd %zd world! https://%s.example.com user@%s.domain.com";
        assert_eq!(
            FormatPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![
                ("%ld", 7, 10),
                ("%9llu", 11, 16),
                ("%hhd", 17, 21),
                ("%zd", 22, 25),
                ("%s", 41, 43),
                ("%s", 61, 63),
            ]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![
                ("Hello", 0, 5),
                ("world", 26, 31),
                ("https", 33, 38),
                ("example", 44, 51),
                ("com", 52, 55),
                ("user", 56, 60),
                ("domain", 64, 70),
                ("com", 71, 74),
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://%s.example.com", 33, 55),]
        );
        assert_eq!(
            FormatEmailPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("user@%s.domain.com", 56, 74),]
        );
        assert_eq!(
            strip_formats(s, &Language::C),
            "Hello,     world! https://.example.com user@.domain.com"
        );
    }

    #[test]
    fn test_unicode() {
        let s = "héllo, мир! %lld 你好 https://%s.example.com user@%s.domain.com";
        assert_eq!(
            FormatPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("%lld", 16, 20), ("%s", 36, 38), ("%s", 56, 58),]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![
                ("héllo", 0, 6),
                ("мир", 8, 14),
                ("你好", 21, 27),
                ("https", 28, 33),
                ("example", 39, 46),
                ("com", 47, 50),
                ("user", 51, 55),
                ("domain", 59, 65),
                ("com", 66, 69),
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://%s.example.com", 28, 50),]
        );
        assert_eq!(
            FormatEmailPos::new(s, &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("user@%s.domain.com", 51, 69),]
        );
        assert_eq!(
            strip_formats(s, &Language::C),
            "héllo, мир!  你好 https://.example.com user@.domain.com"
        );
    }
}
