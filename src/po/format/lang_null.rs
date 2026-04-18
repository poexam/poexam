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
    fn test_strip_formats() {
        assert_eq!(strip_formats("", &Language::Null), "");
        assert_eq!(
            strip_formats("Hello, %s world!", &Language::Null),
            "Hello, %s world!"
        );
    }

    #[test]
    fn test_format_pos() {
        assert!(FormatPos::new("", &Language::Null).next().is_none());
        assert!(
            FormatPos::new("Hello, %s world!", &Language::Null)
                .next()
                .is_none()
        );
    }

    #[test]
    fn test_word_pos() {
        assert!(FormatWordPos::new("", &Language::Null).next().is_none());
        assert_eq!(
            FormatWordPos::new("Hello, %s world!", &Language::Null)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("Hello", 0, 5), ("s", 8, 9), ("world", 10, 15)]
        );
    }

    #[test]
    fn test_url_pos() {
        assert!(FormatUrlPos::new("", &Language::Null).next().is_none());
        assert!(
            FormatUrlPos::new("Hello, %s world!", &Language::Null)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatUrlPos::new("Visit https://example.com for more info.", &Language::Null)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("https://example.com", 6, 25)]
        );
    }

    #[test]
    fn test_email_pos() {
        assert!(FormatEmailPos::new("", &Language::Null).next().is_none());
        assert!(
            FormatEmailPos::new("Hello, %s world!", &Language::Null)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatEmailPos::new(
                "Contact us at user@domain.com for more info. Invalid: user@domain",
                &Language::Null
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![("user@domain.com", 14, 29)]
        );
    }
}
