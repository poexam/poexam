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
        iter::{FormatEmailPos, FormatPathPos, FormatPos, FormatUrlPos, FormatWordPos},
        language::Language,
        strip_formats,
    };

    #[test]
    fn test_strip_formats() {
        assert_eq!(strip_formats("", &Language::Python), "");
        assert_eq!(strip_formats("", &Language::PythonBrace), "");
        assert_eq!(
            strip_formats("Hello, world!", &Language::Python),
            "Hello, world!"
        );
        assert_eq!(
            strip_formats("Hello, world!", &Language::PythonBrace),
            "Hello, world!"
        );
        assert_eq!(
            strip_formats("Hello, %s {0} world!", &Language::Python),
            "Hello,  {0} world!"
        );
        assert_eq!(
            strip_formats("Hello, %s {0} world!", &Language::PythonBrace),
            "Hello, %s  world!"
        );
    }

    #[test]
    fn test_format_pos() {
        assert!(FormatPos::new("", &Language::Python).next().is_none());
        assert!(FormatPos::new("", &Language::PythonBrace).next().is_none());
        assert!(
            FormatPos::new("Hello, world!", &Language::Python)
                .next()
                .is_none()
        );
        assert!(
            FormatPos::new("Hello, world!", &Language::PythonBrace)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatPos::new(
                "Hello/你好, %05.2f %(name)s %ld %hd %% %é world! %",
                &Language::Python
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![
                ("%05.2f", 14, 20),
                ("%(name)s", 21, 29),
                ("%ld", 30, 33),
                ("%hd", 34, 37),
                ("%", 41, 42),
            ]
        );
        assert_eq!(
            FormatPos::new("{é", &Language::PythonBrace)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("{é", 0, 3)]
        );
        assert_eq!(
            FormatPos::new(
                "Hello/你好, {0!r:20}{1}{2} {{ }} world! {",
                &Language::PythonBrace
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![("{0!r:20}", 14, 22), ("{1}", 22, 25), ("{2}", 25, 28)]
        );
    }

    #[test]
    fn test_word_pos() {
        assert!(FormatWordPos::new("", &Language::Python).next().is_none());
        assert!(
            FormatWordPos::new("", &Language::PythonBrace)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatWordPos::new("Hello, world!", &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("Hello", 0, 5), ("world", 7, 12)]
        );
        assert_eq!(
            FormatWordPos::new("Hello, world!", &Language::PythonBrace)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("Hello", 0, 5), ("world", 7, 12)]
        );
        assert_eq!(
            FormatWordPos::new(
                "Hello/你好, %05.2f %(name)s %ld %hd %% %é world! %",
                &Language::Python
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            [
                ("Hello", 0, 5),
                ("你好", 6, 12),
                ("é", 42, 44),
                ("world", 45, 50),
            ]
        );
        assert_eq!(
            FormatWordPos::new(
                "Hello/你好, {0!r:20}{1}{2} {{ }} world! {",
                &Language::PythonBrace
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            [("Hello", 0, 5), ("你好", 6, 12), ("world", 35, 40)]
        );
    }

    #[test]
    fn test_url_pos() {
        assert!(FormatUrlPos::new("", &Language::Python).next().is_none());
        assert!(
            FormatUrlPos::new("", &Language::PythonBrace)
                .next()
                .is_none()
        );
        assert!(
            FormatUrlPos::new("Hello, world!", &Language::Python)
                .next()
                .is_none()
        );
        assert!(
            FormatUrlPos::new("Hello, world!", &Language::PythonBrace)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatUrlPos::new(
                "Invalid URL: https://example, valid URL: https://example.com",
                &Language::Python
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![("https://example.com", 41, 60)]
        );
        assert_eq!(
            FormatUrlPos::new(
                "Invalid URL: https://example, valid URL: https://example.com",
                &Language::PythonBrace
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![("https://example.com", 41, 60)]
        );
        assert_eq!(
            FormatUrlPos::new(
                "Test https://%s.example.com https://example2.com",
                &Language::Python
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![
                ("https://%s.example.com", 5, 27),
                ("https://example2.com", 28, 48),
            ]
        );
        assert_eq!(
            FormatUrlPos::new(
                "Test https://{0}.example.com https://example2.com",
                &Language::PythonBrace
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![
                ("https://{0}.example.com", 5, 28),
                ("https://example2.com", 29, 49),
            ]
        );
    }

    #[test]
    fn test_email_pos() {
        assert!(FormatEmailPos::new("", &Language::Python).next().is_none());
        assert!(
            FormatEmailPos::new("", &Language::PythonBrace)
                .next()
                .is_none()
        );
        assert!(
            FormatEmailPos::new("Hello, world!", &Language::Python)
                .next()
                .is_none()
        );
        assert!(
            FormatEmailPos::new("Hello, world!", &Language::PythonBrace)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatEmailPos::new(
                "Contact us at user@example.com for more info.",
                &Language::Python
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![("user@example.com", 14, 30)]
        );
        assert_eq!(
            FormatEmailPos::new(
                "Contact us at user@%s.example.com for more info.",
                &Language::PythonBrace
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![("user@%s.example.com", 14, 33)]
        );
        assert_eq!(
            FormatEmailPos::new(
                "Invalid email: user@domain, valid email: user@%s.domain.com",
                &Language::Python
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![("user@%s.domain.com", 41, 59)]
        );
        assert_eq!(
            FormatEmailPos::new(
                "Invalid email: user@domain, valid email: user@{0}.domain.com",
                &Language::PythonBrace
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![("user@{0}.domain.com", 41, 60)]
        );
    }

    #[test]
    fn test_path_pos() {
        assert!(FormatPathPos::new("", &Language::Python).next().is_none());
        assert!(
            FormatPathPos::new("", &Language::PythonBrace)
                .next()
                .is_none()
        );
        assert!(
            FormatPathPos::new("Hello, world!", &Language::Python)
                .next()
                .is_none()
        );
        assert!(
            FormatPathPos::new("Hello, world!", &Language::PythonBrace)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatPathPos::new("Path: /home/%s/file.txt", &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("/home/%s/file.txt", 6, 23)]
        );
        assert_eq!(
            FormatPathPos::new("Path: /home/{0}/file.txt", &Language::PythonBrace)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("/home/{0}/file.txt", 6, 24)]
        );
    }
}
