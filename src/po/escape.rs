// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Utilities for escaping and unescaping strings for PO file format.

pub trait EscapePoExt {
    /// Escape special characters in a string for PO file format.
    fn escape_po(&self) -> String;

    /// Unescape special character sequences in a string from a PO file.
    fn unescape_po(&self) -> String;
}

impl EscapePoExt for str {
    /// Escape special characters in a string for PO file format, like GNU
    /// gettext: `\a`, `\b`, `\f`, `\n`, `\r`, `\t`, `\v`, `\"` and `\\`.
    fn escape_po(&self) -> String {
        let mut out = String::with_capacity(self.len() * 2);
        for ch in self.chars() {
            let escaped = match ch {
                '\u{07}' => 'a',
                '\u{08}' => 'b',
                '\u{0c}' => 'f',
                '\n' => 'n',
                '\r' => 'r',
                '\t' => 't',
                '\u{0b}' => 'v',
                '"' | '\\' => ch,
                _ => {
                    out.push(ch);
                    continue;
                }
            };
            out.push('\\');
            out.push(escaped);
        }
        out
    }

    /// Unescape special character sequences in a string from a PO file:
    /// `\a`, `\b`, `\f`, `\n`, `\r`, `\t`, `\v`, `\"` and `\\` (other sequences
    /// are kept as-is).
    fn unescape_po(&self) -> String {
        let mut out = String::with_capacity(self.len());
        let mut it = self.chars();
        while let Some(ch) = it.next() {
            if ch != '\\' {
                out.push(ch);
                continue;
            }
            match it.next() {
                Some('a') => out.push('\u{07}'),
                Some('b') => out.push('\u{08}'),
                Some('f') => out.push('\u{0c}'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('v') => out.push('\u{0b}'),
                Some(other @ ('"' | '\\')) => out.push(other),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_basic() {
        assert_eq!("".escape_po(), "");
        assert_eq!("abc".escape_po(), "abc");
    }

    #[test]
    fn escape_specials() {
        assert_eq!("\n".escape_po(), "\\n");
        assert_eq!("\r".escape_po(), "\\r");
        assert_eq!("\t".escape_po(), "\\t");
        assert_eq!("\"".escape_po(), "\\\"");
        assert_eq!("\\".escape_po(), "\\\\");
        assert_eq!("\u{07}\u{08}\u{0c}\u{0b}".escape_po(), "\\a\\b\\f\\v");
    }

    #[test]
    fn escape_mixed() {
        let s = "a\\b\nc\td\"e";
        let expected = "a\\\\b\\nc\\td\\\"e";
        assert_eq!(s.escape_po(), expected);
    }

    #[test]
    fn unescape_basic() {
        assert_eq!("".unescape_po(), "");
        assert_eq!("abc".unescape_po(), "abc");
    }

    #[test]
    fn unescape_specials() {
        assert_eq!("\\n".unescape_po(), "\n");
        assert_eq!("\\r".unescape_po(), "\r");
        assert_eq!("\\t".unescape_po(), "\t");
        assert_eq!("\\\"".unescape_po(), "\"");
        assert_eq!("\\".unescape_po(), "\\");
        assert_eq!("\\\\".unescape_po(), "\\");
        assert_eq!("\\a\\b\\f\\v".unescape_po(), "\u{07}\u{08}\u{0c}\u{0b}");
        // An escaped backslash followed by "v" is not a vertical tab.
        assert_eq!("\\\\v".unescape_po(), "\\v");
    }

    #[test]
    fn unescape_unknown_sequence_is_kept() {
        assert_eq!("\\x".unescape_po(), "\\x");
        assert_eq!("test\\qval".unescape_po(), "test\\qval");
    }

    #[test]
    fn roundtrip() {
        let samples = [
            "",
            "plain text",
            "line1\nline2",
            "tab\there",
            "quote:\"",
            "backslash \\",
            "mix\r\n\t\"end",
            "controls: \u{07}\u{08}\u{0c}\u{0b}",
            "utf8: café – 測試",
        ];
        for &s in &samples {
            let escaped = s.escape_po();
            let unescaped = escaped.unescape_po();
            assert_eq!(unescaped, s, "failed roundtrip for: {s:?} -> {escaped:?}");
        }
    }
}
