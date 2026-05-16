// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! PO file entry.

use serde::Serialize;

use std::collections::BTreeMap;
use std::ops::Range;

use crate::{po::escape::EscapePoExt, po::format::language::Language, po::message::Message};

#[derive(Debug, Default, Serialize)]
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
    /// Byte range of the whole entry (including leading comments and the
    /// trailing blank line separator) in the original file bytes. Used by the
    /// auto-fix writer to splice or delete the entry.
    #[serde(skip)]
    pub byte_range: Range<usize>,
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.line_number == other.line_number
            && self.keywords == other.keywords
            && self.fuzzy == other.fuzzy
            && self.obsolete == other.obsolete
            && self.noqa == other.noqa
            && self.noqa_rules == other.noqa_rules
            && self.nowrap == other.nowrap
            && self.format_language == other.format_language
            && self.encoding_error == other.encoding_error
            && self.msgctxt == other.msgctxt
            && self.msgid == other.msgid
            && self.msgid_plural == other.msgid_plural
            && self.msgstr == other.msgstr
    }
}

impl Eq for Entry {}

impl Entry {
    /// Create a new PO entry with the line number and default values.
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
    pub const fn is_header(&self) -> bool {
        match &self.msgid {
            Some(msg) => msg.value.is_empty(),
            None => false,
        }
    }

    /// Return `true` if this entry has a plural form (`msgid_plural` is set).
    pub const fn has_plural_form(&self) -> bool {
        self.msgid_plural.is_some()
    }

    /// Return `true` if this entry has at least one non-empty translation string
    /// (even if the entry is marked as fuzzy).
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

    /// Iterator over plural translations only (`msgstr[n]` with `n > 0`).
    ///
    /// Skips `msgstr[0]` directly via `BTreeMap::range`, avoiding a `filter` pass.
    pub fn iter_plural_strs(&self) -> impl Iterator<Item = (&u32, &Message)> + '_ {
        self.msgstr.range(1..)
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

    /// Convert the keywords of this entry back to PO file lines.
    pub fn keywords_to_po_lines(&self) -> Vec<String> {
        self.keywords
            .iter()
            .map(|keyword| format!("#, {keyword}"))
            .collect()
    }

    /// Convert the messages of this entry back to PO file lines.
    pub fn msg_to_po_lines(&self) -> Vec<(usize, String)> {
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
        msgstr.insert(0, Message::new(4, "fichier\n", 0..0));
        msgstr.insert(1, Message::new(5, "fichiers\n", 0..0));
        Entry {
            msgctxt: Some(Message::new(1, "a file\n", 0..0)),
            msgid: Some(Message::new(2, "file\n", 0..0)),
            msgid_plural: Some(Message::new(3, "files\n", 0..0)),
            msgstr,
            ..Default::default()
        }
    }

    #[test]
    fn test_entry() {
        let mut entry = Entry::new(1);
        assert!(!entry.is_header());
        assert!(!entry.is_translated());
        entry.msgctxt = Some(Message::new(1, "a file", 0..0));
        entry.msgid = Some(Message::new(2, "", 0..0));
        assert!(entry.is_header());
        assert!(!entry.is_translated());
        entry.msgstr.insert(0, Message::new(4, "fichier\\n", 0..0));
        assert!(entry.is_header());
        assert!(entry.is_translated());
        entry.msgid = Some(Message::new(2, "file\\n", 0..0));
        assert!(!entry.is_header());
        assert!(entry.is_translated());
    }

    #[test]
    fn test_entry_append() {
        let mut entry = get_test_entry();
        entry.append_msgctxt("here");
        assert_eq!(entry.msgctxt, Some(Message::new(1, "a file\nhere", 0..0)));
        entry.append_msgid("here");
        assert_eq!(entry.msgid, Some(Message::new(2, "file\nhere", 0..0)));
        entry.append_msgid_plural("here");
        assert_eq!(
            entry.msgid_plural,
            Some(Message::new(3, "files\nhere", 0..0))
        );
        entry.append_msgstr(0, "ici");
        assert_eq!(
            entry.msgstr.get(&0),
            Some(&Message::new(4, "fichier\nici", 0..0))
        );
        entry.append_msgstr(1, "ici");
        assert_eq!(
            entry.msgstr.get(&1),
            Some(&Message::new(5, "fichiers\nici", 0..0))
        );
    }

    #[test]
    fn test_entry_iter() {
        let entry = get_test_entry();
        let mut iter = entry.iter_ids();
        assert_eq!(iter.next(), Some(Message::new(2, "file\n", 0..0)).as_ref());
        assert_eq!(iter.next(), Some(Message::new(3, "files\n", 0..0)).as_ref());
        assert!(iter.next().is_none());
        let mut iter = entry.iter_strs();
        assert_eq!(iter.next(), Some((&0, &Message::new(4, "fichier\n", 0..0))));
        assert_eq!(
            iter.next(),
            Some((&1, &Message::new(5, "fichiers\n", 0..0)))
        );
        assert!(iter.next().is_none());
        // `iter_plural_strs` skips msgstr[0] and yields only msgstr[n] for n > 0.
        let mut iter = entry.iter_plural_strs();
        assert_eq!(
            iter.next(),
            Some((&1, &Message::new(5, "fichiers\n", 0..0)))
        );
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_iter_plural_strs_no_plurals() {
        // Entry with only msgstr[0] yields nothing from `iter_plural_strs`.
        let mut entry = Entry::new(1);
        entry.msgstr.insert(0, Message::new(2, "only-one", 0..0));
        assert!(entry.iter_plural_strs().next().is_none());
    }

    #[test]
    fn test_escape() {
        let mut entry = get_test_entry();
        entry.escape_strings();
        assert_eq!(entry.msgctxt, Some(Message::new(1, "a file\\n", 0..0)));
        assert_eq!(entry.msgid, Some(Message::new(2, "file\\n", 0..0)));
        assert_eq!(entry.msgid_plural, Some(Message::new(3, "files\\n", 0..0)));
        assert_eq!(
            entry.msgstr.get(&0),
            Some(&Message::new(4, "fichier\\n", 0..0))
        );
        assert_eq!(
            entry.msgstr.get(&1),
            Some(&Message::new(5, "fichiers\\n", 0..0))
        );
        entry.unescape_strings();
        assert_eq!(entry.msgctxt, Some(Message::new(1, "a file\n", 0..0)));
        assert_eq!(entry.msgid, Some(Message::new(2, "file\n", 0..0)));
        assert_eq!(entry.msgid_plural, Some(Message::new(3, "files\n", 0..0)));
        assert_eq!(
            entry.msgstr.get(&0),
            Some(&Message::new(4, "fichier\n", 0..0))
        );
        assert_eq!(
            entry.msgstr.get(&1),
            Some(&Message::new(5, "fichiers\n", 0..0))
        );
    }

    #[test]
    fn test_msg_to_po_lines() {
        let entry = get_test_entry();
        let po_lines = entry.msg_to_po_lines();
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

    #[test]
    fn test_keywords_to_po_lines() {
        let mut entry = get_test_entry();
        entry.keywords = vec!["noqa".to_string(), "fuzzy".to_string()];
        let po_lines = entry.keywords_to_po_lines();
        assert_eq!(
            po_lines,
            vec!["#, noqa".to_string(), "#, fuzzy".to_string(),]
        );
    }
}
