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

    /// Unescape special character sequences in a string from a PO file, like
    /// GNU gettext: `\a`, `\b`, `\f`, `\n`, `\r`, `\t`, `\v`, `\"`, `\\`,
    /// octal (`\ooo`, 1 to 3 digits) and hexadecimal (`\xhh…`) byte values
    /// (other sequences are kept as-is).
    ///
    /// Bytes given in octal or hexadecimal which do not form valid UTF-8 are
    /// replaced with U+FFFD.
    fn unescape_po(&self) -> String {
        let bytes = self.as_bytes();
        let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            let Some(pos) = memchr::memchr(b'\\', &bytes[i..]) else {
                out.extend_from_slice(&bytes[i..]);
                break;
            };
            out.extend_from_slice(&bytes[i..i + pos]);
            i += pos + 1;
            let Some(&b) = bytes.get(i) else {
                out.push(b'\\');
                break;
            };
            i += 1;
            match b {
                b'a' => out.push(0x07),
                b'b' => out.push(0x08),
                b'f' => out.push(0x0c),
                b'n' => out.push(b'\n'),
                b'r' => out.push(b'\r'),
                b't' => out.push(b'\t'),
                b'v' => out.push(0x0b),
                b'"' | b'\\' => out.push(b),
                b'0'..=b'7' => {
                    // Up to 3 octal digits, the value is truncated to a byte.
                    let mut value = u32::from(b - b'0');
                    for _ in 0..2 {
                        match bytes.get(i) {
                            Some(&d @ b'0'..=b'7') => {
                                value = value * 8 + u32::from(d - b'0');
                                i += 1;
                            }
                            _ => break,
                        }
                    }
                    out.push(value.to_le_bytes()[0]);
                }
                b'x' if bytes.get(i).is_some_and(u8::is_ascii_hexdigit) => {
                    // Any number of hexadecimal digits, the value is
                    // truncated to a byte.
                    let mut value: u8 = 0;
                    while let Some(d) = bytes.get(i).and_then(|&d| char::from(d).to_digit(16)) {
                        value = (value << 4) | u8::try_from(d).unwrap_or(0);
                        i += 1;
                    }
                    out.push(value);
                }
                _ => out.extend_from_slice(&[b'\\', b]),
            }
        }
        String::from_utf8(out)
            .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
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
    fn unescape_octal_hex() {
        assert_eq!("Esc \\033[1m".unescape_po(), "Esc \u{1b}[1m");
        assert_eq!("\\x1b[0m".unescape_po(), "\u{1b}[0m");
        assert_eq!("\\x1B".unescape_po(), "\u{1b}");
        assert_eq!("\\0".unescape_po(), "\0");
        assert_eq!("\\1234".unescape_po(), "S4");
        assert_eq!("\\x41z".unescape_po(), "Az");
        assert_eq!("\\x4142".unescape_po(), "B");
        assert_eq!("\\7x".unescape_po(), "\u{07}x");
        assert_eq!("caf\\303\\251".unescape_po(), "café");
        assert_eq!("caf\\xc3\\xa9".unescape_po(), "café");
        assert_eq!("\\303".unescape_po(), "\u{fffd}");
        // An escaped backslash followed by digits is not an octal value.
        assert_eq!("\\\\033".unescape_po(), "\\033");
    }

    #[test]
    fn unescape_unknown_sequence_is_kept() {
        assert_eq!("\\x".unescape_po(), "\\x");
        assert_eq!("\\xg".unescape_po(), "\\xg");
        assert_eq!("\\8".unescape_po(), "\\8");
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
