// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format strings: Object Pascal language.
//!
//! Handle format specifiers of the `Format` function (Free Pascal, Delphi):
//! `%[index:][-][width][.precision]type`, like `%s`, `%d`, `%1:s`, `%-10.2f`, `%*d`;
//! `%%` is an escaped percent.
//!
//! See: <https://www.freepascal.org/docs-html/rtl/sysutils/format.html>.

use std::borrow::Cow;

use crate::po::format::{FormatParser, MatchFmtPos};

pub struct FormatObjectPascal;

impl FormatParser for FormatObjectPascal {
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

        // Skip index ("1:"), flag ('-'), width and precision (digits or '*').
        while pos_end < len {
            if matches!(bytes[pos_end], b':' | b'-' | b'.' | b'*' | b'0'..=b'9') {
                pos_end += 1;
            } else {
                break;
            }
        }

        // Parse conversion specifier (e.g. s, d, f, etc.).
        if pos_end < len && bytes[pos_end].is_ascii_alphabetic() {
            pos_end += 1;
        }

        pos_end
    }
}

/// Split a format specifier into its index (if present) and the specifier without
/// the leading '%' and the index.
///
/// For example, for format `"%1:-5d"`, this function returns `(Some(1), "-5d")`.
fn fmt_split_index(fmt: &str) -> (Option<usize>, &str) {
    let rest = fmt.strip_prefix('%').unwrap_or(fmt);
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0 && rest.as_bytes().get(digits) == Some(&b':') {
        (
            Some(rest[..digits].parse().unwrap_or(usize::MAX)),
            &rest[digits + 1..],
        )
    } else {
        (None, rest)
    }
}

/// Return the argument index used by each format specifier, with the specifier
/// normalized (index stripped, type lower case), sorted by argument index.
///
/// Without index, a specifier uses the argument following the last one used; an
/// index ("%1:s") sets the argument, and each '*' in width or precision consumes
/// one argument before the value.
///
/// For example, for formats `["%1:s", "%s", "%0:D"]`, this function returns
/// `[(0, "%d"), (1, "%s"), (2, "%s")]`.
pub fn fmt_object_pascal_args<'a>(fmts: &[MatchFmtPos<'a>]) -> Vec<(usize, Cow<'a, str>)> {
    let mut next_arg: usize = 0;
    let mut args: Vec<_> = fmts
        .iter()
        .map(|m| {
            let (index, rest) = fmt_split_index(m.s);
            if let Some(index) = index {
                next_arg = index;
            }
            let arg = next_arg;
            let stars = rest.bytes().filter(|&b| b == b'*').count();
            next_arg = next_arg.saturating_add(stars + 1);
            let fmt = if index.is_none() && !rest.bytes().any(|b| b.is_ascii_uppercase()) {
                Cow::Borrowed(m.s)
            } else {
                Cow::Owned(format!("%{}", rest.to_ascii_lowercase()))
            };
            (arg, fmt)
        })
        .collect();
    args.sort();
    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::po::format::{
        iter::{FormatPos, FormatWordPos},
        language::Language,
        strip_formats,
    };

    fn args(s: &str) -> Vec<(usize, String)> {
        let fmts: Vec<_> = FormatPos::new(s, Language::ObjectPascal).collect();
        fmt_object_pascal_args(&fmts)
            .into_iter()
            .map(|(arg, fmt)| (arg, fmt.into_owned()))
            .collect()
    }

    #[test]
    fn test_split_index() {
        assert_eq!(fmt_split_index("%s"), (None, "s"));
        assert_eq!(fmt_split_index("%10d"), (None, "10d"));
        assert_eq!(fmt_split_index("%1:s"), (Some(1), "s"));
        assert_eq!(fmt_split_index("%12:-5.2f"), (Some(12), "-5.2f"));
        assert_eq!(fmt_split_index("%:s"), (None, ":s"));
    }

    #[test]
    fn test_object_pascal_args() {
        assert!(args("").is_empty());
        assert_eq!(
            args("%s %d"),
            vec![(0, "%s".to_string()), (1, "%d".to_string())]
        );
        assert_eq!(
            args("%1:d %0:S"),
            vec![(0, "%s".to_string()), (1, "%d".to_string())]
        );
        assert_eq!(
            args("%1:s %s %0:D"),
            vec![
                (0, "%d".to_string()),
                (1, "%s".to_string()),
                (2, "%s".to_string()),
            ]
        );
        assert_eq!(
            args("%*.*f %s"),
            vec![(0, "%*.*f".to_string()), (3, "%s".to_string())]
        );
    }

    #[test]
    fn test_strip_formats() {
        assert_eq!(strip_formats("", Language::ObjectPascal), "");
        assert_eq!(
            strip_formats("Hello, world!", Language::ObjectPascal),
            "Hello, world!"
        );
        assert_eq!(
            strip_formats(
                "Hello/你好, %s %d %1:s %-10.2f %*d %0:*.*F %x %% %é world! %",
                Language::ObjectPascal
            ),
            "Hello/你好,        % é world! %"
        );
    }

    #[test]
    fn test_format_pos() {
        assert!(FormatPos::new("", Language::ObjectPascal).next().is_none());
        assert!(
            FormatPos::new("Hello, world!", Language::ObjectPascal)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatPos::new(
                "Hello/你好, %s %d %1:s %-10.2f %*d %0:*.*F %x %% %é world! %",
                Language::ObjectPascal
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![
                ("%s", 14, 16),
                ("%d", 17, 19),
                ("%1:s", 20, 24),
                ("%-10.2f", 25, 32),
                ("%*d", 33, 36),
                ("%0:*.*F", 37, 44),
                ("%x", 45, 47),
                ("%", 51, 52),
            ]
        );
    }

    #[test]
    fn test_word_pos() {
        assert_eq!(
            FormatWordPos::new("Hello %s, %1:d files", Language::ObjectPascal)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("Hello", 0, 5), ("files", 15, 20)]
        );
    }
}
