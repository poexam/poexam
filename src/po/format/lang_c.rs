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
    fmt[1..pos]
        .parse::<usize>()
        .as_ref()
        .map_or(usize::MAX, |index| *index)
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
        iter::{FormatEmailPos, FormatPathPos, FormatPos, FormatUrlPos, FormatWordPos},
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
    fn test_strip_formats() {
        assert_eq!(strip_formats("", &Language::C), "");
        assert_eq!(
            strip_formats("Hello, world!", &Language::C),
            "Hello, world!"
        );
        assert_eq!(
            strip_formats(
                "Hello/你好, %3$d %2$s %1$f %05.2f %ld %hhd %zd %% %é world! %",
                &Language::C
            ),
            "Hello/你好,        % é world! %"
        );
    }

    #[test]
    fn test_format_pos() {
        assert!(FormatPos::new("", &Language::C).next().is_none());
        assert!(
            FormatPos::new("Hello, world!", &Language::C)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatPos::new(
                "Hello/你好, %3$d %2$s %1$f %05.2f %lld %hhd %zd %% %é world! %",
                &Language::C
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![
                ("%3$d", 14, 18),
                ("%2$s", 19, 23),
                ("%1$f", 24, 28),
                ("%05.2f", 29, 35),
                ("%lld", 36, 40),
                ("%hhd", 41, 45),
                ("%zd", 46, 49),
                ("%", 53, 54),
            ]
        );
    }

    #[test]
    fn test_word_pos() {
        assert!(FormatWordPos::new("", &Language::C).next().is_none());
        assert_eq!(
            FormatWordPos::new("Hello, world!", &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("Hello", 0, 5), ("world", 7, 12)]
        );
        assert_eq!(
            FormatWordPos::new(
                "Hello/你好, %3$d %2$s %1$f %05.2f %lld %hhd %zd %% %é world! %",
                &Language::C
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![
                ("Hello", 0, 5),
                ("你好", 6, 12),
                ("é", 54, 56),
                ("world", 57, 62),
            ]
        );
    }

    #[test]
    fn test_url_pos() {
        assert!(FormatUrlPos::new("", &Language::C).next().is_none());
        assert!(
            FormatUrlPos::new("Hello, world!", &Language::C)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatUrlPos::new("Visit https://example.com for more info.", &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://example.com", 6, 25)]
        );
        assert_eq!(
            FormatUrlPos::new(
                "Invalid URL: https://example, valid URL: https://example.com",
                &Language::C
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![("https://example.com", 41, 60)]
        );
        assert_eq!(
            FormatUrlPos::new(
                "Test https://%s.example.com https://example2.com %",
                &Language::C
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![
                ("https://%s.example.com", 5, 27),
                ("https://example2.com", 28, 48),
            ]
        );
    }

    #[test]
    fn test_email_pos() {
        assert!(FormatEmailPos::new("", &Language::C).next().is_none());
        assert_eq!(
            FormatEmailPos::new(
                "Contact us at user@example.com for more info.",
                &Language::C
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![("user@example.com", 14, 30)]
        );
        assert_eq!(
            FormatEmailPos::new(
                "Invalid email: user@domain, valid email: user@%s.domain.com %",
                &Language::C
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![("user@%s.domain.com", 41, 59)]
        );
    }

    #[test]
    fn test_path_pos() {
        assert!(FormatPathPos::new("", &Language::C).next().is_none());
        assert!(
            FormatPathPos::new("Hello, %s world!", &Language::C)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatPathPos::new("Path: /home/%s/file.txt", &Language::C)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("/home/%s/file.txt", 6, 23)]
        );
    }
}
