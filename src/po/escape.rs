// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

pub trait EscapePoExt {
    fn escape_po(&self) -> String;
    fn unescape_po(&self) -> String;
}

impl EscapePoExt for str {
    /// Escape special characters in a string for PO file format.
    fn escape_po(&self) -> String {
        let mut out = String::with_capacity(self.len() * 2);
        for ch in self.chars() {
            match ch {
                '\n' => {
                    out.push('\\');
                    out.push('n');
                }
                '\r' => {
                    out.push('\\');
                    out.push('r');
                }
                '\t' => {
                    out.push('\\');
                    out.push('t');
                }
                '"' => {
                    out.push('\\');
                    out.push('"');
                }
                '\\' => {
                    out.push('\\');
                    out.push('\\');
                }
                _ => out.push(ch),
            }
        }
        out
    }

    /// Unescape special character sequences in a string from a PO file.
    fn unescape_po(&self) -> String {
        let mut out = String::with_capacity(self.len());
        let mut it = self.chars().peekable();
        while let Some(ch) = it.next() {
            if ch == '\\' {
                match it.peek().copied() {
                    Some('n') => {
                        out.push('\n');
                        it.next();
                    }
                    Some('r') => {
                        out.push('\r');
                        it.next();
                    }
                    Some('t') => {
                        out.push('\t');
                        it.next();
                    }
                    Some('"') => {
                        out.push('"');
                        it.next();
                    }
                    Some('\\') => {
                        out.push('\\');
                        it.next();
                    }
                    Some(other) => {
                        out.push('\\');
                        out.push(other);
                        it.next();
                    }
                    None => out.push('\\'),
                }
            } else {
                out.push(ch);
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
            "utf8: café – 測試",
        ];
        for &s in &samples {
            let escaped = s.escape_po();
            let unescaped = escaped.unescape_po();
            assert_eq!(unescaped, s, "failed roundtrip for: {s:?} -> {escaped:?}");
        }
    }
}
