// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! A message of a PO entry.

use serde::Serialize;

use crate::po::escape::EscapePoExt;

#[derive(Debug, Default, PartialEq, Serialize)]
pub struct Message {
    pub line_number: usize,
    pub value: String,
}

impl Message {
    /// Create a new `Message` with the given line and value.
    pub fn new<S: AsRef<str>>(line_number: usize, value: S) -> Self {
        Message {
            line_number,
            value: value.as_ref().to_string(),
        }
    }

    /// Escape special characters in the value (to be written in a PO file).
    pub fn escape(&mut self) {
        self.value = self.value.escape_po();
    }

    /// Unescape special character sequences in a the value read from PO file.
    pub fn unescape(&mut self) {
        self.value = self.value.unescape_po();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_po_string() {
        let mut msgid = Message::new(8, "test\nline 2");
        assert_eq!(
            format!("{msgid:?}"),
            "Message { line_number: 8, value: \"test\\nline 2\" }"
        );
        msgid.escape();
        assert_eq!(msgid.value, "test\\nline 2");
        msgid.unescape();
        assert_eq!(msgid.value, "test\nline 2");
    }
}
