// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format strings: PHP language.
//!
//! Handle `sprintf` patterns like `%s`, `%05.2f`, `%'*10s`, `%1$s`, `%-10d`.
//!
//! See: <https://www.php.net/manual/en/function.sprintf.php>.

use crate::po::format::FormatParser;

pub struct FormatPhp;

impl FormatParser for FormatPhp {
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

        // Skip argument number (e.g. "1$").
        let mut pos_digits = pos_end;
        while pos_digits < len && bytes[pos_digits].is_ascii_digit() {
            pos_digits += 1;
        }
        if pos_digits > pos_end && pos_digits < len && bytes[pos_digits] == b'$' {
            pos_end = pos_digits + 1;
        }

        // Skip flags: '-', '+', ' ', '0' and a custom padding char introduced by '\''.
        while pos_end < len {
            match bytes[pos_end] {
                b'-' | b'+' | b' ' | b'0' => pos_end += 1,
                b'\'' => {
                    pos_end += 1;
                    // The padding char can be any char (UTF-8 included).
                    if let Some(c) = s[pos_end..].chars().next() {
                        pos_end += c.len_utf8();
                    }
                }
                _ => break,
            }
        }

        // Skip width.
        while pos_end < len && bytes[pos_end].is_ascii_digit() {
            pos_end += 1;
        }

        // Skip precision.
        if pos_end < len && bytes[pos_end] == b'.' {
            pos_end += 1;
            while pos_end < len && bytes[pos_end].is_ascii_digit() {
                pos_end += 1;
            }
        }

        // Skip length modifier (accepted and ignored by PHP).
        if pos_end < len && bytes[pos_end] == b'l' {
            pos_end += 1;
        }

        // Parse conversion specifier.
        if pos_end < len
            && matches!(
                bytes[pos_end],
                b'b' | b'c'
                    | b'd'
                    | b'e'
                    | b'E'
                    | b'f'
                    | b'F'
                    | b'g'
                    | b'G'
                    | b'h'
                    | b'H'
                    | b'o'
                    | b's'
                    | b'u'
                    | b'x'
                    | b'X'
            )
        {
            pos_end += 1;
        }

        pos_end
    }
}

#[cfg(test)]
mod tests {
    use crate::po::format::{
        iter::{FormatPos, FormatUrlPos, FormatWordPos},
        language::Language,
        strip_formats,
    };

    #[test]
    fn test_strip_formats() {
        assert_eq!(strip_formats("", Language::Php), "");
        assert_eq!(
            strip_formats("Hello, world!", Language::Php),
            "Hello, world!"
        );
        assert_eq!(
            strip_formats(
                "Hello/你好, %2$s %1$d %05.2f %'*10s %'é5d %-10s %+d %ld %b %X %% world! %",
                Language::Php
            ),
            "Hello/你好,           % world! %"
        );
    }

    #[test]
    fn test_format_pos() {
        assert!(FormatPos::new("", Language::Php).next().is_none());
        assert!(
            FormatPos::new("Hello, world!", Language::Php)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatPos::new(
                "Hello/你好, %2$s %1$d %05.2f %'*10s %'é5d %-10s %+d %ld %% %é world! %",
                Language::Php
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![
                ("%2$s", 14, 18),
                ("%1$d", 19, 23),
                ("%05.2f", 24, 30),
                ("%'*10s", 31, 37),
                ("%'é5d", 38, 44),
                ("%-10s", 45, 50),
                ("%+d", 51, 54),
                ("%ld", 55, 58),
                ("%", 62, 63),
            ]
        );
    }

    #[test]
    fn test_word_pos() {
        assert_eq!(
            FormatWordPos::new("Hello %s, %1$d files", Language::Php)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("Hello", 0, 5), ("files", 15, 20)]
        );
    }

    #[test]
    fn test_url_pos() {
        assert_eq!(
            FormatUrlPos::new(
                "Test https://%s.example.com https://example2.com %",
                Language::Php
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![
                ("https://%s.example.com", 5, 27),
                ("https://example2.com", 28, 48),
            ]
        );
    }
}
