// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format strings: Java `MessageFormat` language.
//!
//! Handle patterns like `{0}`, `{1,number}`, `{2,date,short}`,
//! `{0,choice,0#no files|1#one file|1<{0,number,integer} files}`.
//!
//! See: <https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/text/MessageFormat.html>.

use crate::po::format::FormatParser;

pub struct FormatJava;

impl FormatParser for FormatJava {
    #[inline]
    fn next_char(&self, s: &str, pos: usize) -> Option<(char, usize, bool)> {
        match s[pos..].chars().next() {
            Some('{') => match s[pos + 1..].chars().next() {
                // A digit after '{' means the start of a format element.
                Some(c) if c.is_ascii_digit() => Some(('{', pos + 1, true)),
                // '{' not followed by a digit is a literal character.
                _ => Some(('{', pos + 1, false)),
            },
            Some('\'') => {
                // Quoted literal: skip everything between single quotes.
                // "''" is an escaped single quote.
                let new_pos = pos + 1;
                if new_pos < s.len() && s.as_bytes()[new_pos] == b'\'' {
                    // Escaped single quote: "''" → literal "'"
                    Some(('\'', new_pos + 1, false))
                } else {
                    // Skip until matching closing quote.
                    let mut end = new_pos;
                    while end < s.len() {
                        if s.as_bytes()[end] == b'\'' {
                            return Some(('\'', end + 1, false));
                        }
                        end += 1;
                    }
                    // No closing quote: treat as literal.
                    Some(('\'', new_pos, false))
                }
            }
            Some(c) => Some((c, pos + c.len_utf8(), false)),
            None => None,
        }
    }

    #[inline]
    fn find_end_format(&self, s: &str, pos: usize, len: usize) -> usize {
        let bytes = s.as_bytes();
        let mut pos_end = pos;
        let mut depth = 1;

        // Find the matching closing '}', accounting for nested format elements
        // (e.g. in ChoiceFormat patterns).
        while pos_end < len {
            match bytes[pos_end] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return pos_end + 1;
                    }
                }
                b'\'' => {
                    // Skip quoted literals inside the format element.
                    pos_end += 1;
                    if pos_end < len && bytes[pos_end] == b'\'' {
                        // Escaped quote "''" — skip the second quote.
                        pos_end += 1;
                        continue;
                    }
                    // Skip until closing quote.
                    while pos_end < len && bytes[pos_end] != b'\'' {
                        pos_end += 1;
                    }
                    if pos_end < len {
                        // Skip closing quote.
                        pos_end += 1;
                    }
                    continue;
                }
                _ => {}
            }
            pos_end += 1;
        }
        pos_end
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
        assert_eq!(strip_formats("", &Language::Java), "");
        assert_eq!(
            strip_formats("Hello, world!", &Language::Java),
            "Hello, world!"
        );
        assert_eq!(
            strip_formats("Hello {0}, you have {1,number} items.", &Language::Java),
            "Hello , you have  items."
        );
        assert_eq!(
            strip_formats(
                "{0,choice,0#no files|1#one file|1<{0,number,integer} files}",
                &Language::Java
            ),
            ""
        );
    }

    #[test]
    fn test_format_pos() {
        assert!(FormatPos::new("", &Language::Java).next().is_none());
        assert!(
            FormatPos::new("Hello, world!", &Language::Java)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatPos::new("Name: {0}, age: {1}", &Language::Java)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("{0}", 6, 9), ("{1}", 16, 19)]
        );
        // Formats with type.
        assert_eq!(
            FormatPos::new("{0,number} and {1,date,short}", &Language::Java)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("{0,number}", 0, 10), ("{1,date,short}", 15, 29)]
        );
        // Complex ChoiceFormat pattern.
        assert_eq!(
            FormatPos::new(
                "{0,choice,0#no files|1#one file|1<{0,number,integer} files}",
                &Language::Java
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![(
                "{0,choice,0#no files|1#one file|1<{0,number,integer} files}",
                0,
                59
            )]
        );
        // Quoted section: '{0}' inside quotes is not a format.
        assert_eq!(
            FormatPos::new("literal '{0}' and {1}", &Language::Java)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("{1}", 18, 21)]
        );
        // "''" is an escaped single quote, not a quoting block.
        assert_eq!(
            FormatPos::new("it''s {0} o''clock", &Language::Java)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("{0}", 6, 9)]
        );
        // '{' not followed by a digit is literal.
        assert!(
            FormatPos::new("{name} and {}", &Language::Java)
                .next()
                .is_none()
        );
        // Quoted literal inside a format element.
        assert_eq!(
            FormatPos::new("{0,number,'#'#}", &Language::Java)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("{0,number,'#'#}", 0, 15)]
        );
    }

    #[test]
    fn test_word_pos() {
        assert!(FormatWordPos::new("", &Language::Java).next().is_none());
        assert_eq!(
            FormatWordPos::new("Hello, world!", &Language::Java)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("Hello", 0, 5), ("world", 7, 12)]
        );
        assert_eq!(
            FormatWordPos::new("Name: {0}, age: {1,number}", &Language::Java)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("Name", 0, 4), ("age", 11, 14)]
        );
    }

    #[test]
    fn test_url_pos() {
        assert!(FormatUrlPos::new("", &Language::Java).next().is_none());
        assert!(
            FormatUrlPos::new("Hello, world!", &Language::Java)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatUrlPos::new(
                "Invalid URL: https://example, valid URL: https://example.com",
                &Language::Java
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![("https://example.com", 41, 60)]
        );
        assert_eq!(
            FormatUrlPos::new(
                "Test https://{0}.example.com https://example2.com",
                &Language::Java
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
        assert!(FormatEmailPos::new("", &Language::Java).next().is_none());
        assert!(
            FormatEmailPos::new("Hello, world!", &Language::Java)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatEmailPos::new(
                "Contact us at user@example.com for more info.",
                &Language::Java
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![("user@example.com", 14, 30)]
        );
        assert_eq!(
            FormatEmailPos::new(
                "Invalid email: user@domain, valid email: user@{0}.domain.com",
                &Language::Java
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![("user@{0}.domain.com", 41, 60)]
        );
    }

    #[test]
    fn test_path_pos() {
        assert!(FormatPathPos::new("", &Language::Java).next().is_none());
        assert!(
            FormatPathPos::new("Hello, world!", &Language::Java)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatPathPos::new("File: /home/{0}/data.txt", &Language::Java)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("/home/{0}/data.txt", 6, 24)]
        );
    }

    #[test]
    fn test_html_tags_pos() {
        assert!(FormatHtmlTagPos::new("", &Language::Java).next().is_none());
        assert!(
            FormatHtmlTagPos::new("Hello, world!", &Language::Java)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatHtmlTagPos::new("Hello <b>{0}</b>!", &Language::Java)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("<b>", 6, 9), ("</b>", 12, 16)]
        );
    }
}
