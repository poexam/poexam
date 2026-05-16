// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! A message of a PO entry.

use std::ops::Range;

use serde::Serialize;

use crate::po::escape::EscapePoExt;

#[derive(Debug, Default, Serialize)]
pub struct Message {
    pub line_number: usize,
    pub value: String,
    /// Byte range of the whole message block (keyword line + continuation lines)
    /// in the original file bytes. Used by the auto-fix writer to splice a
    /// freshly emitted block back into the file.
    #[serde(skip)]
    pub byte_range: Range<usize>,
}

impl PartialEq for Message {
    fn eq(&self, other: &Self) -> bool {
        self.line_number == other.line_number && self.value == other.value
    }
}

impl Eq for Message {}

impl Message {
    /// Create a new `Message` with the given line, value and byte range.
    pub fn new<S: AsRef<str>>(line_number: usize, value: S, byte_range: Range<usize>) -> Self {
        Self {
            line_number,
            value: value.as_ref().to_string(),
            byte_range,
        }
    }

    /// Escape special characters in the value (to be written in a PO file).
    pub fn escape(&mut self) {
        self.value = self.value.escape_po();
    }

    /// Unescape special character sequences in a the value read from PO file.
    pub fn unescape(&mut self) {
        // Fast path: if there is no backslash in the value, there is nothing
        // to unescape. Skip the allocation and the per-char walk.
        if memchr::memchr(b'\\', self.value.as_bytes()).is_none() {
            return;
        }
        self.value = self.value.unescape_po();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_po_string() {
        let mut msgid = Message::new(8, "test\nline 2", 0..0);
        assert_eq!(
            format!("{msgid:?}"),
            "Message { line_number: 8, value: \"test\\nline 2\", byte_range: 0..0 }"
        );
        msgid.escape();
        assert_eq!(msgid.value, "test\\nline 2");
        msgid.unescape();
        assert_eq!(msgid.value, "test\nline 2");
    }

    #[test]
    fn test_new_with_range() {
        let msg = Message::new(3, "hello", 12..25);
        assert_eq!(msg.line_number, 3);
        assert_eq!(msg.value, "hello");
        assert_eq!(msg.byte_range, 12..25);
    }

    #[test]
    fn test_equality_ignores_byte_range() {
        let a = Message::new(1, "x", 0..5);
        let b = Message::new(1, "x", 100..200);
        assert_eq!(a, b);
    }
}
