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
        iter::{FormatEmailPos, FormatPos, FormatUrlPos, FormatWordPos},
        language::Language,
        strip_formats,
    };

    #[test]
    fn test_no_format_percent() {
        let s = "Hello, world! https://example.com user@domain.com";
        assert!(FormatPos::new(s, &Language::Python).next().is_none());
        assert_eq!(
            FormatWordPos::new(s, &Language::Python)
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
            FormatUrlPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://example.com", 14, 33),]
        );
        assert_eq!(
            FormatEmailPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("user@domain.com", 34, 49),]
        );
        assert_eq!(strip_formats(s, &Language::Python), s);
    }

    #[test]
    fn test_no_format_brace() {
        let s = "Hello, world! https://example.com user@domain.com";
        assert!(FormatPos::new(s, &Language::PythonBrace).next().is_none());
        assert_eq!(
            FormatWordPos::new(s, &Language::PythonBrace)
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
            FormatUrlPos::new(s, &Language::PythonBrace)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://example.com", 14, 33),]
        );
        assert_eq!(
            FormatEmailPos::new(s, &Language::PythonBrace)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("user@domain.com", 34, 49),]
        );
        assert_eq!(strip_formats(s, &Language::PythonBrace), s);
    }

    #[test]
    fn test_invalid_format_percent() {
        let s = "%";
        assert!(FormatPos::new(s, &Language::Python).next().is_none());
        assert!(FormatWordPos::new(s, &Language::Python).next().is_none());
        assert!(FormatUrlPos::new(s, &Language::Python).next().is_none());
        assert!(FormatEmailPos::new(s, &Language::Python).next().is_none());
        assert_eq!(strip_formats(s, &Language::Python), s);

        let s = "%é";
        assert_eq!(
            FormatPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("%", 0, 1),]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("é", 1, 3),]
        );
        assert!(FormatUrlPos::new(s, &Language::Python).next().is_none());
        assert!(FormatEmailPos::new(s, &Language::Python).next().is_none());
        assert_eq!(strip_formats(s, &Language::Python), "é");

        let s = "%(test";
        assert_eq!(
            FormatPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("%(test", 0, 6),]
        );
        assert!(FormatWordPos::new(s, &Language::Python).next().is_none());
        assert!(FormatUrlPos::new(s, &Language::Python).next().is_none());
        assert!(FormatEmailPos::new(s, &Language::Python).next().is_none());
        assert!(strip_formats(s, &Language::Python).is_empty());
    }

    #[test]
    fn test_invalid_format_brace() {
        let s = "{";
        assert!(FormatPos::new(s, &Language::PythonBrace).next().is_none());
        assert!(
            FormatWordPos::new(s, &Language::PythonBrace)
                .next()
                .is_none()
        );
        assert!(
            FormatUrlPos::new(s, &Language::PythonBrace)
                .next()
                .is_none()
        );
        assert!(
            FormatEmailPos::new(s, &Language::PythonBrace)
                .next()
                .is_none()
        );
        assert_eq!(strip_formats(s, &Language::PythonBrace), s);

        let s = "{é";
        assert_eq!(
            FormatPos::new(s, &Language::PythonBrace)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("{é", 0, 3),]
        );
        assert!(
            FormatWordPos::new(s, &Language::PythonBrace)
                .next()
                .is_none()
        );
        assert!(
            FormatUrlPos::new(s, &Language::PythonBrace)
                .next()
                .is_none()
        );
        assert!(
            FormatEmailPos::new(s, &Language::PythonBrace)
                .next()
                .is_none()
        );
        assert!(strip_formats(s, &Language::PythonBrace).is_empty());
    }

    #[test]
    fn test_single_format_percent() {
        let s = "Hello, %s world! https://example.com user@domain.com";
        assert_eq!(
            FormatPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("%s", 7, 9),]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::Python)
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
            FormatUrlPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://example.com", 17, 36),]
        );
        assert_eq!(
            FormatEmailPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("user@domain.com", 37, 52),]
        );
        assert_eq!(
            strip_formats(s, &Language::Python),
            "Hello,  world! https://example.com user@domain.com"
        );
    }

    #[test]
    fn test_single_format_brace() {
        let s = "Hello, {0:{1}} world! https://example.com user@domain.com";
        assert_eq!(
            FormatPos::new(s, &Language::PythonBrace)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("{0:{1}}", 7, 14),]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::PythonBrace)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![
                ("Hello", 0, 5),
                ("world", 15, 20),
                ("https", 22, 27),
                ("example", 30, 37),
                ("com", 38, 41),
                ("user", 42, 46),
                ("domain", 47, 53),
                ("com", 54, 57),
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::PythonBrace)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://example.com", 22, 41),]
        );
        assert_eq!(
            FormatEmailPos::new(s, &Language::PythonBrace)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("user@domain.com", 42, 57),]
        );
        assert_eq!(
            strip_formats(s, &Language::PythonBrace),
            "Hello,  world! https://example.com user@domain.com"
        );
    }

    #[test]
    fn test_single_format_percent_keyword() {
        let s = "Hello, %(name)s world! https://example.com user@domain.com";
        assert_eq!(
            FormatPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("%(name)s", 7, 15),]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![
                ("Hello", 0, 5),
                ("world", 16, 21),
                ("https", 23, 28),
                ("example", 31, 38),
                ("com", 39, 42),
                ("user", 43, 47),
                ("domain", 48, 54),
                ("com", 55, 58),
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://example.com", 23, 42),]
        );
        assert_eq!(
            FormatEmailPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("user@domain.com", 43, 58),]
        );
        assert_eq!(
            strip_formats(s, &Language::Python),
            "Hello,  world! https://example.com user@domain.com"
        );
    }

    #[test]
    fn test_multiple_formats_percent() {
        let s = "Hello, %d%s%f world! https://%s.example.com user@%s.domain.com";
        assert_eq!(
            FormatPos::new(s, &Language::Python)
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
            FormatWordPos::new(s, &Language::Python)
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
            FormatUrlPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://%s.example.com", 21, 43),]
        );
        assert_eq!(
            FormatEmailPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("user@%s.domain.com", 44, 62),]
        );
        assert_eq!(
            strip_formats(s, &Language::Python),
            "Hello,  world! https://.example.com user@.domain.com"
        );
    }

    #[test]
    fn test_multiple_formats_brace() {
        let s = "Hello, {0!r:20}{1}{2} world! https://{3}.example.com user@{4}.domain.com";
        assert_eq!(
            FormatPos::new(s, &Language::PythonBrace)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![
                ("{0!r:20}", 7, 15),
                ("{1}", 15, 18),
                ("{2}", 18, 21),
                ("{3}", 37, 40),
                ("{4}", 58, 61),
            ]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::PythonBrace)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![
                ("Hello", 0, 5),
                ("world", 22, 27),
                ("https", 29, 34),
                ("example", 41, 48),
                ("com", 49, 52),
                ("user", 53, 57),
                ("domain", 62, 68),
                ("com", 69, 72),
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::PythonBrace)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://{3}.example.com", 29, 52),]
        );
        assert_eq!(
            FormatEmailPos::new(s, &Language::PythonBrace)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("user@{4}.domain.com", 53, 72),]
        );
        assert_eq!(
            strip_formats(s, &Language::PythonBrace),
            "Hello,  world! https://.example.com user@.domain.com"
        );
    }

    #[test]
    fn test_escaped_percent() {
        let s = "Hello, %% %s world! https://%s.example.com user@%s.domain.com";
        assert_eq!(
            FormatPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("%s", 10, 12), ("%s", 28, 30), ("%s", 48, 50),]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::Python)
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
            FormatUrlPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://%s.example.com", 20, 42),]
        );
        assert_eq!(
            FormatEmailPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("user@%s.domain.com", 43, 61),]
        );
        assert_eq!(
            strip_formats(s, &Language::Python),
            "Hello, %  world! https://.example.com user@.domain.com"
        );
    }

    #[test]
    fn test_escaped_brace() {
        let s = "Hello, {{ {0} world! https://{1}.example.com user@{2}.domain.com";
        assert_eq!(
            FormatPos::new(s, &Language::PythonBrace)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("{0}", 10, 13), ("{1}", 29, 32), ("{2}", 50, 53),]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::PythonBrace)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![
                ("Hello", 0, 5),
                ("world", 14, 19),
                ("https", 21, 26),
                ("example", 33, 40),
                ("com", 41, 44),
                ("user", 45, 49),
                ("domain", 54, 60),
                ("com", 61, 64),
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::PythonBrace)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://{1}.example.com", 21, 44),]
        );
        assert_eq!(
            FormatEmailPos::new(s, &Language::PythonBrace)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("user@{2}.domain.com", 45, 64),]
        );
        assert_eq!(
            strip_formats(s, &Language::PythonBrace),
            "Hello, {  world! https://.example.com user@.domain.com"
        );
    }

    #[test]
    fn test_flags_width_precision() {
        let s = "Hello, %05.2f world! https://%s.example.com user@%s.domain.com";
        assert_eq!(
            FormatPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("%05.2f", 7, 13), ("%s", 29, 31), ("%s", 49, 51),]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::Python)
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
            FormatUrlPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://%s.example.com", 21, 43),]
        );
        assert_eq!(
            FormatEmailPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("user@%s.domain.com", 44, 62),]
        );
        assert_eq!(
            strip_formats(s, &Language::Python),
            "Hello,  world! https://.example.com user@.domain.com"
        );
    }

    #[test]
    fn test_flags_width_length() {
        let s = "Hello, %ld %9lu world! https://%s.example.com user@%s.domain.com";
        assert_eq!(
            FormatPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![
                ("%ld", 7, 10),
                ("%9lu", 11, 15),
                ("%s", 31, 33),
                ("%s", 51, 53),
            ]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![
                ("Hello", 0, 5),
                ("world", 16, 21),
                ("https", 23, 28),
                ("example", 34, 41),
                ("com", 42, 45),
                ("user", 46, 50),
                ("domain", 54, 60),
                ("com", 61, 64),
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://%s.example.com", 23, 45),]
        );
        assert_eq!(
            FormatEmailPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("user@%s.domain.com", 46, 64),]
        );
        assert_eq!(
            strip_formats(s, &Language::Python),
            "Hello,   world! https://.example.com user@.domain.com"
        );
    }

    #[test]
    fn test_unicode_percent() {
        let s = "héllo, мир! %ld 你好 https://%s.example.com user@%s.domain.com";
        assert_eq!(
            FormatPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("%ld", 16, 19), ("%s", 35, 37), ("%s", 55, 57),]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![
                ("héllo", 0, 6),
                ("мир", 8, 14),
                ("你好", 20, 26),
                ("https", 27, 32),
                ("example", 38, 45),
                ("com", 46, 49),
                ("user", 50, 54),
                ("domain", 58, 64),
                ("com", 65, 68),
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://%s.example.com", 27, 49),]
        );
        assert_eq!(
            FormatEmailPos::new(s, &Language::Python)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("user@%s.domain.com", 50, 68),]
        );
        assert_eq!(
            strip_formats(s, &Language::Python),
            "héllo, мир!  你好 https://.example.com user@.domain.com"
        );
    }

    #[test]
    fn test_unicode_brace() {
        let s = "héllo, мир! {0} 你好 https://{1}.example.com user@{2}.domain.com";
        assert_eq!(
            FormatPos::new(s, &Language::PythonBrace)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("{0}", 16, 19), ("{1}", 35, 38), ("{2}", 56, 59),]
        );
        assert_eq!(
            FormatWordPos::new(s, &Language::PythonBrace)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![
                ("héllo", 0, 6),
                ("мир", 8, 14),
                ("你好", 20, 26),
                ("https", 27, 32),
                ("example", 39, 46),
                ("com", 47, 50),
                ("user", 51, 55),
                ("domain", 60, 66),
                ("com", 67, 70),
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::PythonBrace)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://{1}.example.com", 27, 50),]
        );
        assert_eq!(
            FormatEmailPos::new(s, &Language::PythonBrace)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("user@{2}.domain.com", 51, 70),]
        );
        assert_eq!(
            strip_formats(s, &Language::PythonBrace),
            "héllo, мир!  你好 https://.example.com user@.domain.com"
        );
    }
}
