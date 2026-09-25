// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format strings: Perl language.
//!
//! Handle `sprintf` patterns like `%s`, `%05.2f`, `%1$s`, `%-*d`, `%vd`, `%lld`
//! (`perl-format`), and brace patterns like `{name}` (`perl-brace-format`).
//!
//! See: <https://perldoc.perl.org/functions/sprintf> and
//! <https://metacpan.org/pod/Locale::TextDomain>.

use crate::po::format::FormatParser;

pub struct FormatPerl;

/// Return the position after an explicit argument number (`digits` followed by `$`)
/// starting at `pos`, or `pos` itself if there is none.
fn skip_arg_number(bytes: &[u8], pos: usize) -> usize {
    let mut end = pos;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end > pos && end < bytes.len() && bytes[end] == b'$' {
        end + 1
    } else {
        pos
    }
}

/// Return the position after a width or precision value starting at `pos`: digits, or
/// `*` optionally followed by an argument number (e.g. `*2$`).
fn skip_number_or_star(bytes: &[u8], pos: usize) -> usize {
    if pos < bytes.len() && bytes[pos] == b'*' {
        return skip_arg_number(bytes, pos + 1);
    }
    let mut end = pos;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    end
}

impl FormatParser for FormatPerl {
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
        let bytes = &s.as_bytes()[..len];

        // Skip argument number (e.g. "1$").
        let mut pos_end = skip_arg_number(bytes, pos);

        // Skip flags.
        while pos_end < len && matches!(bytes[pos_end], b' ' | b'+' | b'-' | b'0' | b'#') {
            pos_end += 1;
        }

        // Skip vector flag: "v", "*v" or "*2$v" (the star gives the join string).
        if pos_end < len && bytes[pos_end] == b'v' {
            pos_end += 1;
        } else if pos_end < len && bytes[pos_end] == b'*' {
            let end = skip_arg_number(bytes, pos_end + 1);
            if end < len && bytes[end] == b'v' {
                pos_end = end + 1;
            }
        }

        // Skip width.
        pos_end = skip_number_or_star(bytes, pos_end);

        // Skip precision.
        if pos_end < len && bytes[pos_end] == b'.' {
            pos_end = skip_number_or_star(bytes, pos_end + 1);
        }

        // Skip size (h, l, ll, q, L, V).
        if pos_end < len {
            match bytes[pos_end] {
                b'l' => {
                    pos_end += 1;
                    if pos_end < len && bytes[pos_end] == b'l' {
                        pos_end += 1;
                    }
                }
                b'h' | b'q' | b'L' | b'V' => pos_end += 1,
                _ => {}
            }
        }

        // Parse conversion specifier.
        if pos_end < len
            && matches!(
                bytes[pos_end],
                b'b' | b'B'
                    | b'c'
                    | b'd'
                    | b'D'
                    | b'e'
                    | b'E'
                    | b'f'
                    | b'F'
                    | b'g'
                    | b'G'
                    | b'i'
                    | b'n'
                    | b'o'
                    | b'O'
                    | b'p'
                    | b's'
                    | b'u'
                    | b'U'
                    | b'x'
                    | b'X'
            )
        {
            pos_end += 1;
        }

        pos_end
    }
}

pub struct FormatPerlBrace;

/// Return the position of the `}` closing the brace format string whose name starts at
/// `pos` (just after `{`), or `None` if there is no valid name followed by `}` there.
///
/// The name is an identifier: an ASCII letter or `_`, then ASCII alphanumeric characters
/// or `_`.
fn brace_name_end(bytes: &[u8], pos: usize) -> Option<usize> {
    if pos >= bytes.len() || !(bytes[pos].is_ascii_alphabetic() || bytes[pos] == b'_') {
        return None;
    }
    let mut end = pos + 1;
    while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
        end += 1;
    }
    (end < bytes.len() && bytes[end] == b'}').then_some(end)
}

impl FormatParser for FormatPerlBrace {
    #[inline]
    fn next_char(&self, s: &str, pos: usize) -> Option<(char, usize, bool)> {
        match s[pos..].chars().next() {
            // "{name}" is a format string, any other '{' is a literal.
            Some('{') => Some((
                '{',
                pos + 1,
                brace_name_end(s.as_bytes(), pos + 1).is_some(),
            )),
            // Other character: not a format string.
            Some(c) => Some((c, pos + c.len_utf8(), false)),
            // End of string: no more character.
            None => None,
        }
    }

    #[inline]
    fn find_end_format(&self, s: &str, pos: usize, len: usize) -> usize {
        // Skip the name and the closing '}' (checked by `next_char`).
        brace_name_end(&s.as_bytes()[..len], pos).map_or(pos, |end| end + 1)
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
        assert_eq!(strip_formats("", Language::Perl), "");
        assert_eq!(
            strip_formats("Hello, world!", Language::Perl),
            "Hello, world!"
        );
        assert_eq!(
            strip_formats(
                "Hello/你好, %2$s %1$d %05.2f %-*d %.*2$s %vd %*v02x %lld %#X %% world! %",
                Language::Perl
            ),
            "Hello/你好,          % world! %"
        );
    }

    #[test]
    fn test_format_pos() {
        assert!(FormatPos::new("", Language::Perl).next().is_none());
        assert!(
            FormatPos::new("Hello, world!", Language::Perl)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatPos::new(
                "Hello/你好, %2$s %1$d %05.2f %-*d %.*2$s %vd %*v02x %lld %% %é world! %",
                Language::Perl
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![
                ("%2$s", 14, 18),
                ("%1$d", 19, 23),
                ("%05.2f", 24, 30),
                ("%-*d", 31, 35),
                ("%.*2$s", 36, 42),
                ("%vd", 43, 46),
                ("%*v02x", 47, 53),
                ("%lld", 54, 58),
                ("%", 62, 63),
            ]
        );
    }

    #[test]
    fn test_strip_formats_brace() {
        assert_eq!(strip_formats("", Language::PerlBrace), "");
        assert_eq!(
            strip_formats(
                "Hello/你好, {name} {_n1}s {} {1} {a b} {a-b} {{x}} {é} {unclosed %s world! {",
                Language::PerlBrace
            ),
            "Hello/你好,  s {} {1} {a b} {a-b} {} {é} {unclosed %s world! {"
        );
    }

    #[test]
    fn test_format_pos_brace() {
        assert!(FormatPos::new("", Language::PerlBrace).next().is_none());
        assert_eq!(
            FormatPos::new(
                "Hello/你好, {name} {_n1}s {} {1} {{x}} %s {",
                Language::PerlBrace
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![("{name}", 14, 20), ("{_n1}", 21, 26), ("{x}", 36, 39)]
        );
    }

    #[test]
    fn test_word_pos() {
        assert_eq!(
            FormatWordPos::new("Hello %s, %1$d files", Language::Perl)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("Hello", 0, 5), ("files", 15, 20)]
        );
    }

    #[test]
    fn test_url_pos() {
        assert_eq!(
            FormatUrlPos::new(
                "Test https://%s.example.com https://example2.com %",
                Language::Perl
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![
                ("https://%s.example.com", 5, 27),
                ("https://example2.com", 28, 48),
            ]
        );
    }
}
