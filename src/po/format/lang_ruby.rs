// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format strings: Ruby language.
//!
//! Handle `format` / `sprintf` patterns like `%s`, `%05.2f`, `%1$s`, `%-*d`, and named
//! references like `%<name>d` and `%{name}`.
//!
//! See: <https://docs.ruby-lang.org/en/master/format_specifications_rdoc.html>.

use crate::po::format::FormatParser;

pub struct FormatRuby;

/// Return the position after the digits starting at `pos`.
fn skip_digits(bytes: &[u8], pos: usize) -> usize {
    let mut end = pos;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    end
}

/// Return the position after the `close` delimiter ending the non-empty name that starts
/// at `pos` (just after the opening delimiter), or `None` if there is no such name.
fn name_end(bytes: &[u8], pos: usize, close: u8) -> Option<usize> {
    let end = pos + memchr::memchr(close, &bytes[pos..])?;
    (end > pos).then_some(end + 1)
}

/// Return the position after an argument reference for a width or a precision starting
/// at `pos`: digits, or `*` optionally followed by an argument number (e.g. `*2$`).
fn skip_number_or_star(bytes: &[u8], pos: usize) -> usize {
    if pos < bytes.len() && bytes[pos] == b'*' {
        let end = skip_digits(bytes, pos + 1);
        if end > pos + 1 && end < bytes.len() && bytes[end] == b'$' {
            return end + 1;
        }
        return pos + 1;
    }
    skip_digits(bytes, pos)
}

impl FormatParser for FormatRuby {
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
        let mut pos_end = pos;

        // Flags, argument number, width, precision and name can come in any order, as
        // parsed by Ruby: loop until the conversion specifier.
        while pos_end < len {
            match bytes[pos_end] {
                b' ' | b'#' | b'+' | b'-' | b'0' => pos_end += 1,
                b'1'..=b'9' => {
                    // Argument number ("1$") or width.
                    let end = skip_digits(bytes, pos_end);
                    pos_end = if end < len && bytes[end] == b'$' {
                        end + 1
                    } else {
                        end
                    };
                }
                b'*' => pos_end = skip_number_or_star(bytes, pos_end),
                b'.' => pos_end = skip_number_or_star(bytes, pos_end + 1),
                // Named reference, followed by a conversion: "%<name>d".
                b'<' => match name_end(bytes, pos_end + 1, b'>') {
                    Some(end) => pos_end = end,
                    None => break,
                },
                // Named reference without conversion: "%{name}".
                b'{' => {
                    if let Some(end) = name_end(bytes, pos_end + 1, b'}') {
                        pos_end = end;
                    }
                    break;
                }
                b'a' | b'A' | b'b' | b'B' | b'c' | b'd' | b'e' | b'E' | b'f' | b'g' | b'G'
                | b'i' | b'o' | b'p' | b's' | b'u' | b'x' | b'X' => {
                    pos_end += 1;
                    break;
                }
                _ => break,
            }
        }

        pos_end
    }
}

/// Return the name of a named Ruby format string (`%<name>d` or `%{name}`), or an empty
/// string for an unnamed one.
///
/// For example, for `"%-5<count>d"`, this function returns `"count"`.
pub fn fmt_ruby_name(fmt: &str) -> &str {
    let bytes = fmt.as_bytes();
    for (open, close) in [(b'<', b'>'), (b'{', b'}')] {
        if let Some(start) = memchr::memchr(open, bytes)
            && let Some(end) = memchr::memchr(close, &bytes[start..])
        {
            return &fmt[start + 1..start + end];
        }
    }
    ""
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
    fn test_fmt_ruby_name() {
        assert_eq!(fmt_ruby_name("%s"), "");
        assert_eq!(fmt_ruby_name("%1$s"), "");
        assert_eq!(fmt_ruby_name("%-5<count>d"), "count");
        assert_eq!(fmt_ruby_name("%{name}"), "name");
    }

    #[test]
    fn test_strip_formats() {
        assert_eq!(strip_formats("", Language::Ruby), "");
        assert_eq!(
            strip_formats("Hello, world!", Language::Ruby),
            "Hello, world!"
        );
        assert_eq!(
            strip_formats(
                "Hello/你好, %2$s %1$d %05.2f %-*d %<n>5.1f %{name}s %#x %% world! %",
                Language::Ruby
            ),
            "Hello/你好,      s  % world! %"
        );
    }

    #[test]
    fn test_format_pos() {
        assert!(FormatPos::new("", Language::Ruby).next().is_none());
        assert!(
            FormatPos::new("Hello, world!", Language::Ruby)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatPos::new(
                "Hello/你好, %2$s %1$d %05.2f %-*d %<n>5.1f %{name}s %-1$5d %% %é %<x %",
                Language::Ruby
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![
                ("%2$s", 14, 18),
                ("%1$d", 19, 23),
                ("%05.2f", 24, 30),
                ("%-*d", 31, 35),
                ("%<n>5.1f", 36, 44),
                ("%{name}", 45, 52),
                ("%-1$5d", 54, 60),
                ("%", 64, 65),
                ("%", 68, 69),
            ]
        );
    }

    #[test]
    fn test_word_pos() {
        assert_eq!(
            FormatWordPos::new("Hello %s, %{name} files", Language::Ruby)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("Hello", 0, 5), ("files", 18, 23)]
        );
    }

    #[test]
    fn test_url_pos() {
        assert_eq!(
            FormatUrlPos::new(
                "Test https://%{host}.example.com https://example2.com %",
                Language::Ruby
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![
                ("https://%{host}.example.com", 5, 32),
                ("https://example2.com", 33, 53),
            ]
        );
    }
}
