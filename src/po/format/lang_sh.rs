// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format strings: Shell language.
//!
//! Handle the variable references expanded by `eval_gettext` and `envsubst`: `$name`
//! and `${name}`, where the name is made of ASCII alphanumeric characters and
//! underscores, and does not start with a digit.
//!
//! Anything else after a `$` is a literal: positional and special parameters (`$1`,
//! `$$`), and parameter expansions with operators (`${name:-default}`).
//!
//! See: <https://www.gnu.org/software/gettext/manual/html_node/sh_002dformat.html>.

use crate::po::format::FormatParser;

pub struct FormatSh;

/// Return true if the byte can start a variable name.
#[inline]
fn is_name_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

/// Return true if the byte can continue a variable name.
#[inline]
fn is_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Return the position after the variable name starting at `pos`, which is `pos` itself
/// if there is no valid name there.
fn name_end(bytes: &[u8], pos: usize) -> usize {
    if pos >= bytes.len() || !is_name_start(bytes[pos]) {
        return pos;
    }
    let mut end = pos + 1;
    while end < bytes.len() && is_name_char(bytes[end]) {
        end += 1;
    }
    end
}

impl FormatParser for FormatSh {
    #[inline]
    fn next_char(&self, s: &str, pos: usize) -> Option<(char, usize, bool)> {
        match s[pos..].chars().next() {
            Some('$') => {
                let bytes = s.as_bytes();
                let after = pos + 1;
                let is_format = if after < bytes.len() && bytes[after] == b'{' {
                    // "${name}": the name must be valid and followed by '}'.
                    let end = name_end(bytes, after + 1);
                    end > after + 1 && end < bytes.len() && bytes[end] == b'}'
                } else {
                    name_end(bytes, after) > after
                };
                Some(('$', after, is_format))
            }
            // Other character: not a format string.
            Some(c) => Some((c, pos + c.len_utf8(), false)),
            // End of string: no more character.
            None => None,
        }
    }

    #[inline]
    fn find_end_format(&self, s: &str, pos: usize, len: usize) -> usize {
        let bytes = &s.as_bytes()[..len];
        if pos < len && bytes[pos] == b'{' {
            // Skip the name and the closing '}' (checked by `next_char`).
            (name_end(bytes, pos + 1) + 1).min(len)
        } else {
            name_end(bytes, pos)
        }
    }
}

/// Return the variable name of a shell format string, without `$` and braces.
///
/// For example, for `"${name}"` and `"$name"`, this function returns `"name"`.
pub fn fmt_sh_name(fmt: &str) -> &str {
    let name = fmt.strip_prefix('$').unwrap_or(fmt);
    name.strip_prefix('{')
        .and_then(|n| n.strip_suffix('}'))
        .unwrap_or(name)
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
    fn test_fmt_sh_name() {
        assert_eq!(fmt_sh_name("$name"), "name");
        assert_eq!(fmt_sh_name("${name}"), "name");
        assert_eq!(fmt_sh_name("$_a1"), "_a1");
    }

    #[test]
    fn test_strip_formats() {
        assert_eq!(strip_formats("", Language::Sh), "");
        assert_eq!(
            strip_formats("Hello, world!", Language::Sh),
            "Hello, world!"
        );
        assert_eq!(
            strip_formats(
                "Hello/你好, $name ${count}s $_x1 $1 $$ ${a:-b} ${} $ $é world! $",
                Language::Sh
            ),
            "Hello/你好,  s  $1 $$ ${a:-b} ${} $ $é world! $"
        );
    }

    #[test]
    fn test_format_pos() {
        assert!(FormatPos::new("", Language::Sh).next().is_none());
        assert!(
            FormatPos::new("Hello, world!", Language::Sh)
                .next()
                .is_none()
        );
        assert_eq!(
            FormatPos::new(
                "Hello/你好, $name ${count}s $_x1 $1 $$ ${a:-b} ${} ${unclosed $",
                Language::Sh
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![("$name", 14, 19), ("${count}", 20, 28), ("$_x1", 30, 34)]
        );
    }

    #[test]
    fn test_word_pos() {
        assert_eq!(
            FormatWordPos::new("Hello $name, ${count} files", Language::Sh)
                .map(|m| (m.s, m.start, m.end))
                .collect::<Vec<_>>(),
            vec![("Hello", 0, 5), ("files", 22, 27)]
        );
    }

    #[test]
    fn test_url_pos() {
        assert_eq!(
            FormatUrlPos::new(
                "Test https://$host.example.com https://example2.com $",
                Language::Sh
            )
            .map(|m| (m.s, m.start, m.end))
            .collect::<Vec<_>>(),
            vec![
                ("https://$host.example.com", 5, 30),
                ("https://example2.com", 31, 51),
            ]
        );
    }
}
