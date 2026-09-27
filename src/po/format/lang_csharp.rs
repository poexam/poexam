// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format strings: C# language.
//!
//! Handle composite format items of `String.Format` like `{0}`, `{1,10}`, `{2:N2}`,
//! `{0,-8:yyyy-MM-dd}`; `{{` and `}}` are escaped braces.
//!
//! See: <https://learn.microsoft.com/en-us/dotnet/standard/base-types/composite-formatting>.

use crate::po::format::FormatParser;

pub struct FormatCSharp;

impl FormatParser for FormatCSharp {
    #[inline]
    fn next_char(&self, s: &str, pos: usize) -> Option<(char, usize, bool)> {
        match s[pos..].chars().next() {
            Some('{') => match s[pos + 1..].chars().next() {
                // Escaped brace: "{{" is a literal "{".
                Some('{') => Some(('{', pos + 2, false)),
                // A digit after '{' means the start of a format item.
                Some(c) if c.is_ascii_digit() => Some(('{', pos + 1, true)),
                // '{' not followed by a digit is a literal character.
                _ => Some(('{', pos + 1, false)),
            },
            // Escaped brace: "}}" is a literal "}".
            Some('}') if s[pos + 1..].starts_with('}') => Some(('}', pos + 2, false)),
            // Other character: not a format string.
            Some(c) => Some((c, pos + c.len_utf8(), false)),
            // End of string: no more character.
            None => None,
        }
    }

    #[inline]
    fn find_end_format(&self, s: &str, pos: usize, len: usize) -> usize {
        // The format item ends at the first '}': index, alignment and format string
        // can not contain any.
        memchr::memchr(b'}', &s.as_bytes()[pos..len]).map_or(len, |end| pos + end + 1)
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
        assert_eq!(strip_formats("", Language::CSharp), "");
        assert_eq!(
            strip_formats("Hello, world!", Language::CSharp),
            "Hello, world!"
        );
        assert_eq!(
            strip_formats(
                "Hello/你好, {0} has {1,10} items, {2:N2} {0,-8:yyyy-MM-dd} {{0}} {name} {",
                Language::CSharp
            ),
            "Hello/你好,  has  items,   {0} {name} {"
        );
    }

    #[test]
    fn test_format_pos() {
        assert!(FormatPos::new("", Language::CSharp).next().is_none());
        assert!(
            FormatPos::new("Hello, world!", Language::CSharp)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatPos::new(
                "Hello/你好, {0} has {1,10} items, {2:N2} {0,-8:yyyy-MM-dd} {{0}} {{{1}}} {name} {3",
                Language::CSharp
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![
                ("{0}", 14, 17),
                ("{1,10}", 22, 28),
                ("{2:N2}", 36, 42),
                ("{0,-8:yyyy-MM-dd}", 43, 60),
                ("{1}", 69, 72),
                ("{3", 82, 84),
            ]
        );
    }

    #[test]
    fn test_word_pos() {
        assert_eq!(
            FormatWordPos::new("Hello {0}, {1:N2} files", Language::CSharp)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("Hello", 0, 5), ("files", 18, 23)]
        );
    }

    #[test]
    fn test_url_pos() {
        assert_eq!(
            FormatUrlPos::new(
                "Test https://{0}.example.com https://example2.com",
                Language::CSharp
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![
                ("https://{0}.example.com", 5, 28),
                ("https://example2.com", 29, 49),
            ]
        );
    }
}
