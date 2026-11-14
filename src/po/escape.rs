// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Utilities for escaping and unescaping strings for PO file format.

use encoding_rs::Encoding;

/// Return the escape char of `c` (e.g. `n` for a newline), as written by
/// GNU gettext, or `None` if `c` is not escaped.
pub const fn escape_char(c: char) -> Option<char> {
    match c {
        '\u{07}' => Some('a'),
        '\u{08}' => Some('b'),
        '\u{0c}' => Some('f'),
        '\n' => Some('n'),
        '\r' => Some('r'),
        '\t' => Some('t'),
        '\u{0b}' => Some('v'),
        '\\' | '"' => Some(c),
        _ => None,
    }
}

/// Result of the unescaping of a string from a PO file.
pub struct Unescaped {
    /// The unescaped string.
    pub value: String,
    /// `true` if all escape sequences were decoded: the escaped string can be
    /// written back from the value. Otherwise, the sequences not decoded are
    /// kept as-is in the value: bytes not valid in the encoding of the file
    /// (e.g. `\377` in UTF-8).
    pub exact: bool,
}

pub trait EscapePoExt {
    /// Escape special characters in a string for PO file format.
    fn escape_po(&self) -> String;

    /// Unescape special character sequences in a string from a PO file, the
    /// bytes given in octal or hexadecimal being decoded with `encoding`: see
    /// [`Unescaped`].
    fn unescape_po_with(&self, encoding: &'static Encoding) -> Unescaped;
}

impl EscapePoExt for str {
    /// Escape special characters in a string for PO file format, like GNU
    /// gettext: `\a`, `\b`, `\f`, `\n`, `\r`, `\t`, `\v`, `\"` and `\\`.
    fn escape_po(&self) -> String {
        let mut out = String::with_capacity(self.len() * 2);
        for ch in self.chars() {
            if let Some(escaped) = escape_char(ch) {
                out.push('\\');
                out.push(escaped);
            } else {
                out.push(ch);
            }
        }
        out
    }

    /// Unescape special character sequences in a string from a PO file, like
    /// GNU gettext: `\a`, `\b`, `\f`, `\n`, `\r`, `\t`, `\v`, `\"`, `\\`,
    /// octal (`\ooo`, 1 to 3 digits) and hexadecimal (`\xhh…`) byte values
    /// (other sequences are kept as-is).
    ///
    /// The string is already decoded from the encoding of the file, but not
    /// the bytes given in octal or hexadecimal: each run of these bytes is
    /// decoded with `encoding`, and kept as-is if invalid in this encoding.
    fn unescape_po_with(&self, encoding: &'static Encoding) -> Unescaped {
        let bytes = self.as_bytes();
        let mut out = String::with_capacity(bytes.len());
        let mut exact = true;
        // Run of bytes given in octal or hexadecimal, and its start in `self`.
        let mut run: Vec<u8> = vec![];
        let mut run_start = 0;
        let mut i = 0;
        while i < bytes.len() {
            let Some(pos) = memchr::memchr(b'\\', &bytes[i..]) else {
                break;
            };
            let start = i + pos;
            if !run.is_empty() && start > i {
                exact &= flush_run(&mut out, &mut run, &self[run_start..i], encoding);
            }
            if run.is_empty() {
                out.push_str(&self[i..start]);
            }
            i = start + 1;
            let Some(&b) = bytes.get(i) else {
                if !run.is_empty() {
                    exact &= flush_run(&mut out, &mut run, &self[run_start..start], encoding);
                }
                out.push('\\');
                break;
            };
            i += 1;
            let value = match b {
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
                    Some(value.to_le_bytes()[0])
                }
                b'x' if bytes.get(i).is_some_and(u8::is_ascii_hexdigit) => {
                    // Any number of hexadecimal digits, the value is
                    // truncated to a byte.
                    let mut value: u8 = 0;
                    while let Some(d) = bytes.get(i).and_then(|&d| char::from(d).to_digit(16)) {
                        value = (value << 4) | u8::try_from(d).unwrap_or(0);
                        i += 1;
                    }
                    Some(value)
                }
                _ => None,
            };
            if let Some(value) = value {
                if run.is_empty() {
                    run_start = start;
                }
                run.push(value);
                continue;
            }
            if !run.is_empty() {
                exact &= flush_run(&mut out, &mut run, &self[run_start..start], encoding);
            }
            match b {
                b'a' => out.push('\u{07}'),
                b'b' => out.push('\u{08}'),
                b'f' => out.push('\u{0c}'),
                b'n' => out.push('\n'),
                b'r' => out.push('\r'),
                b't' => out.push('\t'),
                b'v' => out.push('\u{0b}'),
                b'"' | b'\\' => out.push(char::from(b)),
                _ => {
                    // Unknown sequence: kept as-is.
                    out.push('\\');
                    let c = self[i - 1..].chars().next().unwrap_or_default();
                    out.push(c);
                    i += c.len_utf8() - 1;
                }
            }
        }
        if !run.is_empty() {
            exact &= flush_run(&mut out, &mut run, &self[run_start..i], encoding);
        }
        if i < bytes.len() {
            out.push_str(&self[i..]);
        }
        Unescaped { value: out, exact }
    }
}

/// Decode a run of bytes given in octal or hexadecimal with `encoding` and add
/// it to `out`, or add `source` (the escape sequences of the run) if the bytes
/// are invalid in this encoding. Return `true` if the bytes were decoded.
fn flush_run(
    out: &mut String,
    run: &mut Vec<u8>,
    source: &str,
    encoding: &'static Encoding,
) -> bool {
    let decoded = encoding.decode_without_bom_handling_and_without_replacement(run);
    let ok = decoded.is_some();
    out.push_str(decoded.as_deref().unwrap_or(source));
    run.clear();
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unescape a string from a PO file encoded in UTF-8.
    trait UnescapeUtf8 {
        fn unescape_po(&self) -> String;
    }

    impl UnescapeUtf8 for str {
        fn unescape_po(&self) -> String {
            self.unescape_po_with(encoding_rs::UTF_8).value
        }
    }

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
        // Invalid UTF-8: kept as-is.
        assert_eq!("\\303".unescape_po(), "\\303");
        assert_eq!("a\\303\\x41b".unescape_po(), "a\\303\\x41b");
        assert_eq!("\\377 \\101".unescape_po(), "\\377 A");
        assert_eq!("\\101\\".unescape_po(), "A\\");
        // An escaped backslash followed by digits is not an octal value.
        assert_eq!("\\\\033".unescape_po(), "\\033");
    }

    #[test]
    fn unescape_with_encoding() {
        let unescaped = "caf\\351 \\x41".unescape_po_with(encoding_rs::WINDOWS_1252);
        assert_eq!(unescaped.value, "café A");
        assert!(unescaped.exact);
        let unescaped = "caf\\303\\251".unescape_po_with(encoding_rs::UTF_8);
        assert_eq!(unescaped.value, "café");
        assert!(unescaped.exact);
        // Multibyte encoding: the run of bytes is decoded at once.
        let unescaped = "\\xa4\\xa2 \\244".unescape_po_with(encoding_rs::EUC_JP);
        assert_eq!(unescaped.value, "あ \\244");
        assert!(!unescaped.exact);
        let unescaped = "Bon\\377jour".unescape_po_with(encoding_rs::UTF_8);
        assert_eq!(unescaped.value, "Bon\\377jour");
        assert!(!unescaped.exact);
        assert!("a\\n\\t\\\\b".unescape_po_with(encoding_rs::UTF_8).exact);
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
