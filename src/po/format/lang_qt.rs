// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format strings: Qt language.
//!
//! Handle the placeholders replaced by `QString::arg`: `%` optionally followed by `L`
//! (locale-aware formatting), then one or two digits, e.g. `%1`, `%L2`, `%12`.
//! `%0n` is the same placeholder as `%n`.
//!
//! Qt has no escape for `%`: a `%` not followed by a placeholder is a literal.
//!
//! See: <https://doc.qt.io/qt-6/qstring.html#arg>.

use crate::po::format::FormatParser;

pub struct FormatQt;

/// Return the position after the placeholder whose content starts at `pos` (just after
/// `%`), or `None` if there is no placeholder there.
fn placeholder_end(bytes: &[u8], pos: usize) -> Option<usize> {
    let mut end = pos;
    if end < bytes.len() && bytes[end] == b'L' {
        end += 1;
    }
    let digits_start = end;
    while end < bytes.len() && end - digits_start < 2 && bytes[end].is_ascii_digit() {
        end += 1;
    }
    (end > digits_start).then_some(end)
}

impl FormatParser for FormatQt {
    #[inline]
    fn next_char(&self, s: &str, pos: usize) -> Option<(char, usize, bool)> {
        match s[pos..].chars().next() {
            Some('%') => Some((
                '%',
                pos + 1,
                placeholder_end(s.as_bytes(), pos + 1).is_some(),
            )),
            // Other character: not a format string.
            Some(c) => Some((c, pos + c.len_utf8(), false)),
            // End of string: no more character.
            None => None,
        }
    }

    #[inline]
    fn find_end_format(&self, s: &str, pos: usize, len: usize) -> usize {
        placeholder_end(&s.as_bytes()[..len], pos).unwrap_or(pos)
    }
}

/// Return the locale flag and the argument number of a Qt format string.
///
/// For example, for `"%L2"`, this function returns `(true, 2)`, and for `"%01"` and
/// `"%1"`, it returns `(false, 1)`.
pub fn fmt_qt_arg(fmt: &str) -> (bool, u8) {
    let spec = fmt.strip_prefix('%').unwrap_or(fmt);
    let (locale, digits) = spec
        .strip_prefix('L')
        .map_or((false, spec), |digits| (true, digits));
    (locale, digits.parse().unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::po::format::{
        iter::{FormatPos, FormatUrlPos, FormatWordPos},
        language::Language,
        strip_formats,
    };

    #[test]
    fn test_fmt_qt_arg() {
        assert_eq!(fmt_qt_arg("%1"), (false, 1));
        assert_eq!(fmt_qt_arg("%01"), (false, 1));
        assert_eq!(fmt_qt_arg("%L2"), (true, 2));
        assert_eq!(fmt_qt_arg("%99"), (false, 99));
    }

    #[test]
    fn test_strip_formats() {
        assert_eq!(strip_formats("", Language::Qt), "");
        assert_eq!(
            strip_formats("Hello, world!", Language::Qt),
            "Hello, world!"
        );
        assert_eq!(
            strip_formats(
                "Hello/你好, %1 %L2 %01 %123 100% %% %%3 %L %s world! %",
                Language::Qt
            ),
            "Hello/你好,    3 100% %% % %L %s world! %"
        );
    }

    #[test]
    fn test_format_pos() {
        assert!(FormatPos::new("", Language::Qt).next().is_none());
        assert!(
            FormatPos::new("Hello, world!", Language::Qt)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatPos::new(
                "Hello/你好, %1 %L2 %01 %123 100% %% %%3 %L %s world! %",
                Language::Qt
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![
                ("%1", 14, 16),
                ("%L2", 17, 20),
                ("%01", 21, 24),
                ("%12", 25, 28),
                ("%3", 39, 41),
            ]
        );
    }

    #[test]
    fn test_word_pos() {
        assert_eq!(
            FormatWordPos::new("Hello %1, %L2 files", Language::Qt)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("Hello", 0, 5), ("files", 14, 19)]
        );
    }

    #[test]
    fn test_url_pos() {
        assert_eq!(
            FormatUrlPos::new(
                "Test https://%1.example.com https://example2.com %",
                Language::Qt
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![
                ("https://%1.example.com", 5, 27),
                ("https://example2.com", 28, 48),
            ]
        );
    }
}
