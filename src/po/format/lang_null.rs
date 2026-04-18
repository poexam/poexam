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
        iter::{
            FormatEmailPos, FormatHtmlTagPos, FormatPathPos, FormatPos, FormatUrlPos, FormatWordPos,
        },
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

    #[test]
    fn test_path_pos() {
        assert!(FormatPathPos::new("", &Language::Null).next().is_none());
        assert!(
            FormatPathPos::new("Hello, %s world!", &Language::Null)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatPathPos::new("Path: /home/%s/file.txt", &Language::Null)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("/home/%s/file.txt", 6, 23)]
        );
    }

    #[test]
    fn test_html_tags_pos() {
        assert!(FormatHtmlTagPos::new("", &Language::Null).next().is_none());
        assert!(
            FormatHtmlTagPos::new("Hello, %s world!", &Language::Null)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatHtmlTagPos::new(
                r#"Hello <b>%s</b>! 3 < 5 <br/>Click <a href="https://example.com">here</a><span title="a > b"></span><br"#,
                &Language::Null
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![
                ("<b>", 6, 9),
                ("</b>", 11, 15),
                ("<br/>", 23, 28),
                (r#"<a href="https://example.com">"#, 34, 64),
                ("</a>", 68, 72),
                (r#"<span title="a > b">"#, 72, 92),
                ("</span>", 92, 99),
            ]
        );
    }
}
