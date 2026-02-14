// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! PO file entry.

use serde::Serialize;

use std::collections::BTreeMap;

use crate::{po::escape::EscapePoExt, po::format::language::Language, po::message::Message};

#[derive(Debug, Default, PartialEq, Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct Entry {
    pub line_number: usize,
    pub keywords: Vec<String>,
    pub fuzzy: bool,
    pub obsolete: bool,
    pub noqa: bool,
    pub noqa_rules: Vec<String>,
    pub nowrap: bool,
    pub format_language: Language,
    pub encoding_error: bool,
    pub msgctxt: Option<Message>,
    pub msgid: Option<Message>,
    pub msgid_plural: Option<Message>,
    pub msgstr: BTreeMap<u32, Message>,
}

impl Entry {
    /// Create a new PO entry with the line number and default values.
    #[must_use]
    pub fn new(line_number: usize) -> Self {
        Self {
            line_number,
            ..Default::default()
        }
    }

    /// Append additional text to the message context.
    pub fn append_msgctxt<S: AsRef<str>>(&mut self, additional: S) {
        if let Some(ref mut msgctxt) = self.msgctxt {
            msgctxt.value.push_str(additional.as_ref());
        }
    }

    /// Append additional text to the message id.
    pub fn append_msgid<S: AsRef<str>>(&mut self, additional: S) {
        if let Some(ref mut msgid) = self.msgid {
            msgid.value.push_str(additional.as_ref());
        }
    }

    /// Append additional text to the message id (plural).
    pub fn append_msgid_plural<S: AsRef<str>>(&mut self, additional: S) {
        if let Some(ref mut msgid_plural) = self.msgid_plural {
            msgid_plural.value.push_str(additional.as_ref());
        }
    }

    /// Append additional text to a translation using the given index.
    pub fn append_msgstr<S: AsRef<str>>(&mut self, index: u32, additional: S) {
        if let Some(ref mut msgstr) = self.msgstr.get_mut(&index) {
            msgstr.value.push_str(additional.as_ref());
        }
    }

    /// Return `true` if this entry is the header entry (`msgid` is set and is an empty string).
    #[must_use]
    pub fn is_header(&self) -> bool {
        match &self.msgid {
            Some(msg) => msg.value.is_empty(),
            None => false,
        }
    }

    /// Return `true` if this entry has a plural form (`msgid_plural` is set).
    #[must_use]
    pub fn has_plural_form(&self) -> bool {
        self.msgid_plural.is_some()
    }

    /// Return `true` if this entry has at least one non-empty translation string
    /// (even if the entry is marked as fuzzy).
    #[must_use]
    pub fn is_translated(&self) -> bool {
        for msg in self.msgstr.values() {
            if !msg.value.is_empty() {
                return true;
            }
        }
        false
    }

    /// Iterator over all ids in this entry.
    /// Order: `msgid` (if present), `msgid_plural` (if present).
    pub fn iter_ids(&self) -> impl Iterator<Item = &Message> + '_ {
        self.msgid.iter().chain(self.msgid_plural.iter())
    }

    /// Iterator over all translations.
    pub fn iter_strs(&self) -> impl Iterator<Item = (&u32, &Message)> + '_ {
        self.msgstr.iter()
    }

    /// Escapes all string fields in this entry using the provided escape function.
    pub fn escape_strings(&mut self) {
        if let Some(ref mut msg) = self.msgctxt {
            msg.escape();
        }
        if let Some(ref mut msg) = self.msgid {
            msg.escape();
        }
        if let Some(ref mut msg) = self.msgid_plural {
            msg.escape();
        }
        let mut idx: u32 = 0;
        while let Some(msg) = self.msgstr.get_mut(&idx) {
            msg.escape();
            idx += 1;
        }
    }

    /// Unescape all string fields in this entry using the provided unescape function.
    pub fn unescape_strings(&mut self) {
        if let Some(ref mut msg) = self.msgctxt {
            msg.unescape();
        }
        if let Some(ref mut msg) = self.msgid {
            msg.unescape();
        }
        if let Some(ref mut msg) = self.msgid_plural {
            msg.unescape();
        }
        let mut idx: u32 = 0;
        while let Some(msg) = self.msgstr.get_mut(&idx) {
            msg.unescape();
            idx += 1;
        }
    }

    /// Convert this entry back to PO file lines.
    #[must_use]
    pub fn to_po_lines(&self) -> Vec<(usize, String)> {
        let mut lines = Vec::with_capacity(5);
        let prefix = if self.obsolete { "#~ " } else { "" };
        if let Some(msg) = &self.msgctxt {
            lines.push((
                msg.line_number,
                format!("{prefix}msgctxt \"{}\"", msg.value.escape_po()),
            ));
        }
        if let Some(msg) = &self.msgid {
            lines.push((
                msg.line_number,
                format!("{prefix}msgid \"{}\"", msg.value.escape_po()),
            ));
        }
        if let Some(msg) = &self.msgid_plural {
            lines.push((
                msg.line_number,
                format!("{prefix}msgid_plural \"{}\"", msg.value.escape_po()),
            ));
        }
        let mut idx: u32 = 0;
        while let Some(msg) = self.msgstr.get(&idx) {
            if self.has_plural_form() || self.msgstr.len() > 1 {
                lines.push((
                    msg.line_number,
                    format!("{prefix}msgstr[{idx}] \"{}\"", msg.value.escape_po()),
                ));
            } else {
                lines.push((
                    msg.line_number,
                    format!("{prefix}msgstr \"{}\"", msg.value.escape_po()),
                ));
            }
            idx += 1;
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_test_entry() -> Entry {
        let mut msgstr = BTreeMap::new();
        msgstr.insert(0, Message::new(4, "fichier\n"));
        msgstr.insert(1, Message::new(5, "fichiers\n"));
        Entry {
            msgctxt: Some(Message::new(1, "a file\n")),
            msgid: Some(Message::new(2, "file\n")),
            msgid_plural: Some(Message::new(3, "files\n")),
            msgstr,
            ..Default::default()
        }
    }

    #[test]
    fn test_entry() {
        let mut entry = Entry::new(1);
        assert!(!entry.is_header());
        assert!(!entry.is_translated());
        entry.msgctxt = Some(Message::new(1, "a file"));
        entry.msgid = Some(Message::new(2, ""));
        assert!(entry.is_header());
        assert!(!entry.is_translated());
        entry.msgstr.insert(0, Message::new(4, "fichier\\n"));
        assert!(entry.is_header());
        assert!(entry.is_translated());
        entry.msgid = Some(Message::new(2, "file\\n"));
        assert!(!entry.is_header());
        assert!(entry.is_translated());
    }

    #[test]
    fn test_entry_append() {
        let mut entry = get_test_entry();
        entry.append_msgctxt("here");
        assert_eq!(entry.msgctxt, Some(Message::new(1, "a file\nhere")));
        entry.append_msgid("here");
        assert_eq!(entry.msgid, Some(Message::new(2, "file\nhere")));
        entry.append_msgid_plural("here");
        assert_eq!(entry.msgid_plural, Some(Message::new(3, "files\nhere")));
        entry.append_msgstr(0, "ici");
        assert_eq!(entry.msgstr.get(&0), Some(&Message::new(4, "fichier\nici")));
        entry.append_msgstr(1, "ici");
        assert_eq!(
            entry.msgstr.get(&1),
            Some(&Message::new(5, "fichiers\nici"))
        );
    }

    #[test]
    fn test_entry_iter() {
        let entry = get_test_entry();
        let mut iter = entry.iter_ids();
        assert_eq!(iter.next(), Some(Message::new(2, "file\n")).as_ref());
        assert_eq!(iter.next(), Some(Message::new(3, "files\n")).as_ref());
        assert!(iter.next().is_none());
        let mut iter = entry.iter_strs();
        assert_eq!(iter.next(), Some((&0, &Message::new(4, "fichier\n"))));
        assert_eq!(iter.next(), Some((&1, &Message::new(5, "fichiers\n"))));
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_entry_escape() {
        let mut entry = get_test_entry();
        entry.escape_strings();
        assert_eq!(entry.msgctxt, Some(Message::new(1, "a file\\n")));
        assert_eq!(entry.msgid, Some(Message::new(2, "file\\n")));
        assert_eq!(entry.msgid_plural, Some(Message::new(3, "files\\n")));
        assert_eq!(entry.msgstr.get(&0), Some(&Message::new(4, "fichier\\n")));
        assert_eq!(entry.msgstr.get(&1), Some(&Message::new(5, "fichiers\\n")));
        entry.unescape_strings();
        assert_eq!(entry.msgctxt, Some(Message::new(1, "a file\n")));
        assert_eq!(entry.msgid, Some(Message::new(2, "file\n")));
        assert_eq!(entry.msgid_plural, Some(Message::new(3, "files\n")));
        assert_eq!(entry.msgstr.get(&0), Some(&Message::new(4, "fichier\n")));
        assert_eq!(entry.msgstr.get(&1), Some(&Message::new(5, "fichiers\n")));
    }

    #[test]
    fn test_entry_to_po_lines() {
        let entry = get_test_entry();
        let po_lines = entry.to_po_lines();
        assert_eq!(
            po_lines,
            vec![
                (1, "msgctxt \"a file\\n\"".to_string()),
                (2, "msgid \"file\\n\"".to_string()),
                (3, "msgid_plural \"files\\n\"".to_string()),
                (4, "msgstr[0] \"fichier\\n\"".to_string()),
                (5, "msgstr[1] \"fichiers\\n\"".to_string()),
            ]
        );
    }
}
