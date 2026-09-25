// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format strings: JavaScript language.
//!
//! Handle `sprintf`-like patterns like `%s`, `%05.2f`, `%1$s`, `%-10d`, `%j`, as
//! recognized by GNU gettext for the `javascript-format` flag.
//!
//! See: <https://www.gnu.org/software/gettext/manual/html_node/javascript_002dformat.html>.

use crate::po::format::FormatParser;

pub struct FormatJavaScript;

impl FormatParser for FormatJavaScript {
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

        // Skip flags.
        while pos_end < len && matches!(bytes[pos_end], b'-' | b'+' | b' ' | b'0') {
            pos_end += 1;
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

        // Parse conversion specifier.
        if pos_end < len
            && matches!(
                bytes[pos_end],
                b'b' | b'c' | b'd' | b'f' | b'j' | b'o' | b's' | b'x' | b'X'
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
        assert_eq!(strip_formats("", Language::JavaScript), "");
        assert_eq!(
            strip_formats("Hello, world!", Language::JavaScript),
            "Hello, world!"
        );
        assert_eq!(
            strip_formats(
                "Hello/你好, %2$s %1$d %05.2f %-10s %+d %j %b %X %% world! %",
                Language::JavaScript
            ),
            "Hello/你好,         % world! %"
        );
    }

    #[test]
    fn test_format_pos() {
        assert!(FormatPos::new("", Language::JavaScript).next().is_none());
        assert!(
            FormatPos::new("Hello, world!", Language::JavaScript)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatPos::new(
                "Hello/你好, %2$s %1$d %05.2f %-10s %+d %j %% %é world! %",
                Language::JavaScript
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![
                ("%2$s", 14, 18),
                ("%1$d", 19, 23),
                ("%05.2f", 24, 30),
                ("%-10s", 31, 36),
                ("%+d", 37, 40),
                ("%j", 41, 43),
                ("%", 47, 48),
            ]
        );
    }

    #[test]
    fn test_word_pos() {
        assert_eq!(
            FormatWordPos::new("Hello %s, %1$d files", Language::JavaScript)
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
                Language::JavaScript
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
