// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format strings: no language.

use crate::po::format::FormatParser;

pub struct FormatNull;

impl FormatParser for FormatNull {
    #[inline]
    fn next_char(&self, s: &str, pos: usize) -> Option<(char, usize, bool)> {
        s[pos..]
            .chars()
            .next()
            .map(|c| (c, pos + c.len_utf8(), false))
    }

    #[inline]
    fn find_end_format(&self, _s: &str, _pos: usize, len: usize) -> usize {
        len
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
    fn test_empty_string() {
        let s = "";
        assert!(FormatPos::new(s, &Language::Null).next().is_none());
        assert!(FormatWordPos::new(s, &Language::Null).next().is_none());
        assert!(FormatUrlPos::new(s, &Language::Null).next().is_none());
        assert!(strip_formats(s, &Language::Null).is_empty());
    }

    #[test]
    fn test_no_format() {
        let s = "Hello, %s world! 'test' https://example.com invalid@domain user@domain.com";
        assert!(FormatPos::new(s, &Language::Null).next().is_none());
        assert_eq!(
            FormatWordPos::new(s, &Language::Null)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![
                ("Hello", 0, 5),
                ("s", 8, 9),
                ("world", 10, 15),
                ("test", 18, 22),
                ("https", 24, 29),
                ("example", 32, 39),
                ("com", 40, 43),
                ("invalid", 44, 51),
                ("domain", 52, 58),
                ("user", 59, 63),
                ("domain", 64, 70),
                ("com", 71, 74),
            ]
        );
        assert_eq!(
            FormatUrlPos::new(s, &Language::Null)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://example.com", 24, 43)]
        );
        assert_eq!(
            FormatEmailPos::new(s, &Language::Null)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("user@domain.com", 59, 74)]
        );
        assert_eq!(strip_formats(s, &Language::Null), s);
    }
}
