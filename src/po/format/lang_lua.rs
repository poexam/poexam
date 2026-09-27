// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format strings: Lua language.
//!
//! Handle format specifiers of `string.format`: `%[flags][width][.precision]type`,
//! like `%s`, `%d`, `%5.2f`, `%-10s`, `%q`; `%%` is an escaped percent.
//!
//! Unlike C, there are no length modifiers and no reordering of arguments.
//!
//! See: <https://www.lua.org/manual/5.4/manual.html#pdf-string.format>.

use crate::po::format::FormatParser;

pub struct FormatLua;

impl FormatParser for FormatLua {
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

        // Skip flags / width / precision.
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

        // Parse conversion specifier (e.g. s, d, f, q, etc.).
        if pos_end < len && bytes[pos_end].is_ascii_alphabetic() {
            pos_end += 1;
        }

        pos_end
    }
}

#[cfg(test)]
mod tests {
    use crate::po::format::{
        iter::{FormatPos, FormatWordPos},
        language::Language,
        strip_formats,
    };

    #[test]
    fn test_strip_formats() {
        assert_eq!(strip_formats("", Language::Lua), "");
        assert_eq!(
            strip_formats("Hello, world!", Language::Lua),
            "Hello, world!"
        );
        assert_eq!(
            strip_formats(
                "Hello/你好, %s %d %5.2f %-10s %+ #05i %q %X %% %é world! %",
                Language::Lua
            ),
            "Hello/你好,        % é world! %"
        );
    }

    #[test]
    fn test_format_pos() {
        assert!(FormatPos::new("", Language::Lua).next().is_none());
        assert!(
            FormatPos::new("Hello, world!", Language::Lua)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatPos::new(
                "Hello/你好, %s %d %5.2f %-10s %+ #05i %q %X %% %é world! %",
                Language::Lua
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![
                ("%s", 14, 16),
                ("%d", 17, 19),
                ("%5.2f", 20, 25),
                ("%-10s", 26, 31),
                ("%+ #05i", 32, 39),
                ("%q", 40, 42),
                ("%X", 43, 45),
                ("%", 49, 50),
            ]
        );
    }

    #[test]
    fn test_word_pos() {
        assert_eq!(
            FormatWordPos::new("Hello %s, %5.2f files", Language::Lua)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("Hello", 0, 5), ("files", 16, 21)]
        );
    }
}
