// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! PO file parser.

use std::borrow::Cow;

use memchr::memmem;

use crate::{po::entry::Entry, po::message::Message};
use encoding_rs::Encoding;

#[derive(Default)]
enum Field {
    #[default]
    Comment,
    Ctxt,
    Id,
    IdPlural,
    Str(u32),
}

#[derive(Default)]
pub struct Parser<'a> {
    // data and some general info parsed in the header
    pub data: &'a [u8],
    pub data_len: usize,
    pub language: String,
    pub language_code: String,
    pub country: String,
    pub encoding: Option<&'static Encoding>,
    pub nplurals: u32,
    // internal state of the parser
    iter_lines: Option<memchr::memmem::FindIter<'a, 'static>>,
    offset: usize,
    line_number: usize,
    next_line_number: usize,
    field: Field,
    encoding_error: bool,
}

impl<'d> Parser<'d> {
    /// Create a new `Parser` from the given byte slice.
    #[must_use]
    pub fn new(data: &'d [u8]) -> Self {
        Self {
            data,
            data_len: data.len(),
            line_number: 1,
            next_line_number: 1,
            ..Default::default()
        }
    }

    /// Return the encoding name.
    #[must_use]
    pub fn encoding_name(&self) -> &'static str {
        if let Some(enc) = self.encoding {
            enc.name()
        } else {
            encoding_rs::UTF_8.name()
        }
    }

    /// Return the number of plurals defined in the header.
    #[must_use]
    pub fn nplurals(&self) -> u32 {
        self.nplurals
    }

    /// Return the next line from the input data, updating the parser's location.
    fn next_line(&mut self) -> Option<&'d [u8]> {
        if self.offset >= self.data_len {
            return None;
        }
        if self.iter_lines.is_none() {
            self.iter_lines = Some(memmem::find_iter(self.data, "\n"));
        }
        match &mut self.iter_lines {
            Some(iter) => {
                let start = self.offset;
                let end = iter.next().unwrap_or(self.data_len);
                self.offset = end + 1;
                self.next_line_number += 1;
                Some(&self.data[start..end])
            }
            None => None,
        }
    }

    /// Parse the header of a PO entry to extract encoding information if present.
    fn parse_header(&mut self, entry: &mut Entry) {
        let Some(id) = entry.msgid.as_ref() else {
            return;
        };
        if !id.value.is_empty() {
            return;
        }
        let Some(msg) = entry.msgstr.get(&0) else {
            return;
        };
        if msg.value.is_empty() {
            return;
        }
        for line in msg.value.split('\n') {
            let (keyword, value) = line.split_once(':').unwrap_or(("", ""));
            let kw_lower = keyword.trim().to_lowercase();
            if kw_lower == "language" {
                self.language = value.trim().to_string();
                if let Some(pos) = value.find('_') {
                    self.language_code = value[..pos].trim().to_string();
                    self.country = value[pos + 1..].trim().to_string();
                } else {
                    self.language_code = self.language.clone();
                }
            } else if kw_lower == "content-type"
                && let Some(pos) = value.find("charset=")
            {
                let value_charset = &value[pos + 8..];
                let end = value_charset
                    .find(|c: char| c.is_whitespace() || c == ';')
                    .unwrap_or(value_charset.len());
                let charset = &value_charset[..end];
                let encoding = Encoding::for_label(charset.as_bytes());
                // Optimization: if charset is UTF-8, we don't need to decode strings
                // and we can use String::from_utf8_lossy() directly.
                if encoding.is_some_and(|e| e != encoding_rs::UTF_8) {
                    self.encoding = encoding;
                }
            } else if kw_lower == "plural-forms"
                && let Some(pos) = value.find("nplurals=")
            {
                let value_nplurals = &value[pos + 9..];
                let end = value_nplurals
                    .find(|c: char| !c.is_ascii_digit())
                    .unwrap_or(value_nplurals.len());
                if let Ok(nplurals) = value_nplurals[..end].parse::<u32>() {
                    self.nplurals = nplurals;
                }
            }
        }
    }

    /// Parse and add keywords from a comment line, updating flags and format as needed.
    fn parse_keywords(line: &[u8], entry: &mut Entry) {
        entry.keywords.extend(
            line.split(|&b| b == b',')
                .map(|kw| {
                    let kw = String::from_utf8_lossy(kw).trim().to_string();
                    if kw == "fuzzy" {
                        entry.fuzzy = true;
                    } else if kw == "noqa" {
                        entry.noqa = true;
                    } else if let Some(stripped) = kw.strip_prefix("noqa:") {
                        entry.noqa_rules = stripped
                            .split(';')
                            .map(str::trim)
                            .map(String::from)
                            .collect();
                    } else if kw == "no-wrap" {
                        entry.nowrap = true;
                    } else if let Some(stripped) = kw.strip_suffix("-format") {
                        entry.format = stripped.to_string();
                    }
                    kw
                })
                .collect::<Vec<String>>(),
        );
    }

    /// Extract a string value from a line, and decode if necessary (not UTF-8).
    fn extract_string(&mut self, line: &'d [u8]) -> Cow<'d, str> {
        if let Some(start) = line.iter().position(|&b| b == b'"')
            && let Some(end) = line.iter().rposition(|&b| b == b'"')
            && start != end
        {
            if let Some(encoding) = self.encoding {
                let (cow, _, errors) = encoding.decode(&line[start + 1..end]);
                if errors {
                    self.encoding_error = true;
                }
                cow
            } else if let Ok(s) = str::from_utf8(&line[start + 1..end]) {
                Cow::Borrowed(s)
            } else {
                self.encoding_error = true;
                String::from_utf8_lossy(&line[start + 1..end])
            }
        } else {
            Cow::Borrowed("")
        }
    }

    /// Parse a message line and update the corresponding field in the `Entry`.
    ///
    /// The line can be a `msgctxt`, `msgid`, `msgid_plural`, `msgstr`, or a continued string.
    fn parse_message(&mut self, line: &'d [u8], entry: &mut Entry) {
        if line.starts_with(b"msgctxt") {
            self.field = Field::Ctxt;
            entry.msgctxt = Some(Message::new(self.line_number, self.extract_string(line)));
        } else if line.starts_with(b"msgid_plural") {
            self.field = Field::IdPlural;
            entry.msgid_plural = Some(Message::new(self.line_number, self.extract_string(line)));
        } else if line.starts_with(b"msgid") {
            self.field = Field::Id;
            entry.msgid = Some(Message::new(self.line_number, self.extract_string(line)));
        } else if line.starts_with(b"msgstr[") {
            if let Some(idx_end) = line.iter().position(|&b| b == b']')
                && let Ok(str_idx) = str::from_utf8(&line[7..idx_end])
                && let Some(idx) = str_idx.parse::<u32>().ok()
            {
                self.field = Field::Str(idx);
                entry.msgstr.insert(
                    idx,
                    Message::new(self.line_number, self.extract_string(line)),
                );
            }
        } else if line.starts_with(b"msgstr") {
            self.field = Field::Str(0);
            entry
                .msgstr
                .insert(0, Message::new(self.line_number, self.extract_string(line)));
        } else if line.starts_with(b"\"") {
            match self.field {
                Field::Comment => {}
                Field::Ctxt => entry.append_msgctxt(self.extract_string(line)),
                Field::Id => entry.append_msgid(self.extract_string(line)),
                Field::IdPlural => entry.append_msgid_plural(self.extract_string(line)),
                Field::Str(idx) => entry.append_msgstr(idx, self.extract_string(line)),
            }
        }
    }
}

/// Implement the `Iterator` trait for `Parser`, yielding `Entry` items.
impl Iterator for Parser<'_> {
    type Item = Entry;

    /// Return the next `Entry` from the parser, or `None` if finished.
    fn next(&mut self) -> Option<Self::Item> {
        let mut entry = Entry::new(self.next_line_number);
        self.line_number = self.next_line_number;
        self.field = Field::Comment;
        self.encoding_error = false;
        let mut started = false;
        while let Some(line) = self.next_line() {
            if line.is_empty() {
                if started {
                    entry.encoding_error = self.encoding_error;
                    entry.unescape_strings();
                    self.parse_header(&mut entry);
                    return Some(entry);
                }
                entry.line_number = self.next_line_number;
                self.line_number = self.next_line_number;
                continue;
            }
            started = true;
            if let Some(keywords) = line.strip_prefix(b"#,") {
                // Workflow & stiky flags.
                Parser::parse_keywords(keywords, &mut entry);
            } else if let Some(keywords) = line.strip_prefix(b"#=") {
                // Workflow & stiky flags.
                Parser::parse_keywords(keywords, &mut entry);
            } else if let Some(msg) = line.strip_prefix(b"#~ ") {
                // Obsolete entry with a message (start or continued).
                entry.obsolete = true;
                self.parse_message(msg, &mut entry);
            } else if line.starts_with(b"msg") || line.starts_with(b"\"") {
                // Message line (start or continued).
                self.parse_message(line, &mut entry);
            }
            self.line_number = self.next_line_number;
        }
        if started {
            // Send the last entry if we reached the end of data.
            entry.encoding_error = self.encoding_error;
            entry.unescape_strings();
            self.parse_header(&mut entry);
            Some(entry)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_string() {
        let parser = Parser::new(b"");
        let entries: Vec<Entry> = parser.collect();
        assert!(entries.is_empty());
    }

    #[test]
    fn parse_header() {
        let content = r#"# Main comment
msgid ""
msgstr "test\n"
"Project-Id-Version: my_project\n"
"Report-Msgid-Bugs-To: someone@example.com\n"
"Language: fr\n"
"Plural-Forms: nplurals=2; plural=(n > 1);\n"
"#;
        let mut parser = Parser::new(content.as_bytes());
        let entries = parser.by_ref().collect::<Vec<Entry>>();
        assert_eq!(entries[0].line_number, 1);
        assert!(entries[0].keywords.is_empty());
        assert!(!entries[0].fuzzy);
        assert!(!entries[0].noqa);
        assert!(!entries[0].nowrap);
        assert!(entries[0].format.is_empty());
        assert!(!entries[0].encoding_error);
        assert_eq!(parser.nplurals, 2);
        assert!(entries[0].msgctxt.is_none());
        assert_eq!(entries[0].msgid, Some(Message::new(2, "")));
        assert!(entries[0].msgid_plural.is_none());
        assert_eq!(
            entries[0].msgstr.get(&0),
            Some(Message::new(
                3,
                "test\n\
                Project-Id-Version: my_project\n\
                Report-Msgid-Bugs-To: someone@example.com\n\
                Language: fr\n\
                Plural-Forms: nplurals=2; plural=(n > 1);\n"
            ))
            .as_ref()
        );
        assert_eq!(parser.language, "fr");
        assert_eq!(parser.language_code, "fr");
        assert_eq!(parser.country, "");
        assert!(parser.encoding.is_none());

        let content = r#"# Main comment
msgid ""
msgstr "Language: pt_BR\n"
"#;
        let mut parser = Parser::new(content.as_bytes());
        let _ = parser.by_ref().collect::<Vec<Entry>>();
        assert_eq!(parser.language, "pt_BR");
        assert_eq!(parser.language_code, "pt");
        assert_eq!(parser.country, "BR");
    }

    #[test]
    fn parse_simple_entry() {
        let content = r#"
msgid "hello"
msgstr "bonjour"
"#;
        let mut parser = Parser::new(content.as_bytes());
        let entries = parser.by_ref().collect::<Vec<Entry>>();
        assert_eq!(entries[0].line_number, 2);
        assert!(entries[0].keywords.is_empty());
        assert!(!entries[0].fuzzy);
        assert!(!entries[0].noqa);
        assert!(!entries[0].nowrap);
        assert!(entries[0].format.is_empty());
        assert!(!entries[0].encoding_error);
        assert!(entries[0].msgctxt.is_none());
        assert_eq!(entries[0].msgid, Some(Message::new(2, "hello")));
        assert!(entries[0].msgid_plural.is_none());
        assert_eq!(
            entries[0].msgstr.get(&0),
            Some(Message::new(3, "bonjour")).as_ref()
        );
    }

    #[test]
    fn parse_simple_entry_iso8859() {
        let content = r#"
msgid ""
msgstr "Content-Type: text/plain; charset=ISO-8859-15\n"

msgid "tested"
msgstr "testé"
"#;
        let content_iso = encoding_rs::ISO_8859_15.encode(content).0;
        let mut parser = Parser::new(content_iso.as_ref());
        let entries = parser.by_ref().collect::<Vec<Entry>>();
        assert_eq!(parser.encoding, Some(encoding_rs::ISO_8859_15));
        assert_eq!(entries[0].line_number, 2);
        assert!(entries[0].keywords.is_empty());
        assert!(!entries[0].fuzzy);
        assert!(!entries[0].noqa);
        assert!(!entries[0].nowrap);
        assert!(entries[0].format.is_empty());
        assert!(!entries[0].encoding_error);
        assert!(entries[0].msgctxt.is_none());
        assert_eq!(entries[0].msgid, Some(Message::new(2, "")));
        assert!(entries[0].msgid_plural.is_none());
        assert_eq!(
            entries[0].msgstr.get(&0),
            Some(Message::new(
                3,
                "Content-Type: text/plain; charset=ISO-8859-15\n"
            ))
            .as_ref()
        );
        assert!(entries[1].keywords.is_empty());
        assert!(!entries[1].fuzzy);
        assert!(!entries[1].noqa);
        assert!(!entries[1].nowrap);
        assert!(entries[1].format.is_empty());
        assert!(!entries[1].encoding_error);
        assert!(entries[1].msgctxt.is_none());
        assert_eq!(entries[1].msgid, Some(Message::new(5, "tested")));
        assert!(entries[1].msgid_plural.is_none());
        assert_eq!(
            entries[1].msgstr.get(&0),
            Some(Message::new(6, "testé")).as_ref()
        );
    }

    #[test]
    fn parse_simple_entry_encoding_error() {
        let content = r#"
msgid ""
msgstr "Content-Type: text/plain; charset=UTF-8\n"

msgid "tested"
msgstr "testé"
"#;
        let content_iso = encoding_rs::ISO_8859_15.encode(content).0;
        let mut parser = Parser::new(content_iso.as_ref());
        let entries = parser.by_ref().collect::<Vec<Entry>>();
        assert!(parser.encoding.is_none());
        assert_eq!(entries[0].line_number, 2);
        assert!(entries[0].keywords.is_empty());
        assert!(!entries[0].noqa);
        assert!(!entries[0].nowrap);
        assert!(entries[0].format.is_empty());
        assert!(!entries[0].encoding_error);
        assert!(entries[0].msgctxt.is_none());
        assert_eq!(entries[0].msgid, Some(Message::new(2, "")));
        assert!(entries[0].msgid_plural.is_none());
        assert_eq!(
            entries[0].msgstr.get(&0),
            Some(Message::new(3, "Content-Type: text/plain; charset=UTF-8\n",)).as_ref()
        );
        assert!(entries[1].keywords.is_empty());
        assert!(!entries[1].fuzzy);
        assert!(!entries[1].noqa);
        assert!(!entries[1].nowrap);
        assert!(entries[1].format.is_empty());
        assert!(entries[1].encoding_error);
        assert!(entries[1].msgctxt.is_none());
        assert_eq!(entries[1].msgid, Some(Message::new(5, "tested")));
        assert!(entries[1].msgid_plural.is_none());
        assert_eq!(
            entries[1].msgstr.get(&0),
            Some(Message::new(6, "test�")).as_ref()
        );
    }

    #[test]
    fn parse_entry_with_context() {
        let content = r#"
msgctxt "month of the year"
msgid "may"
msgstr "mai"
"#;
        let mut parser = Parser::new(content.as_bytes());
        let entries = parser.by_ref().collect::<Vec<Entry>>();
        assert_eq!(entries[0].line_number, 2);
        assert!(entries[0].keywords.is_empty());
        assert!(!entries[0].fuzzy);
        assert!(!entries[0].noqa);
        assert!(!entries[0].nowrap);
        assert!(entries[0].format.is_empty());
        assert!(!entries[0].encoding_error);
        assert_eq!(
            entries[0].msgctxt,
            Some(Message::new(2, "month of the year"))
        );
        assert_eq!(entries[0].msgid, Some(Message::new(3, "may")));
        assert!(entries[0].msgid_plural.is_none());
        assert_eq!(
            entries[0].msgstr.get(&0),
            Some(Message::new(4, "mai")).as_ref()
        );
    }

    #[test]
    fn parse_two_entries() {
        let content = r#"
msgid "hello"
msgstr "bonjour"

msgid "hello 2"
msgstr ""
"#;
        let mut parser = Parser::new(content.as_bytes());
        let entries = parser.by_ref().collect::<Vec<Entry>>();
        assert_eq!(entries[0].line_number, 2);
        assert!(entries[0].keywords.is_empty());
        assert!(!entries[0].fuzzy);
        assert!(!entries[0].noqa);
        assert!(!entries[0].nowrap);
        assert!(entries[0].format.is_empty());
        assert!(!entries[0].encoding_error);
        assert!(entries[0].msgctxt.is_none());
        assert_eq!(entries[0].msgid, Some(Message::new(2, "hello")));
        assert!(entries[0].msgid_plural.is_none());
        assert_eq!(
            entries[0].msgstr.get(&0),
            Some(Message::new(3, "bonjour")).as_ref()
        );
        assert_eq!(entries[1].line_number, 5);
        assert!(entries[1].keywords.is_empty());
        assert!(!entries[1].fuzzy);
        assert!(!entries[1].noqa);
        assert!(!entries[1].nowrap);
        assert!(entries[1].format.is_empty());
        assert!(!entries[1].encoding_error);
        assert!(entries[1].msgctxt.is_none());
        assert_eq!(entries[1].msgid, Some(Message::new(5, "hello 2")));
        assert!(entries[1].msgid_plural.is_none());
        assert_eq!(
            entries[1].msgstr.get(&0),
            Some(Message::new(6, "")).as_ref()
        );
    }

    #[test]
    fn parse_plural_entry() {
        let content = r#"
msgid "file"
msgid_plural "files"
msgstr[0] "fichier"
msgstr[1] "fichiers"
"#;
        let mut parser = Parser::new(content.as_bytes());
        let entries = parser.by_ref().collect::<Vec<Entry>>();
        assert_eq!(entries[0].line_number, 2);
        assert!(entries[0].keywords.is_empty());
        assert!(!entries[0].fuzzy);
        assert!(!entries[0].noqa);
        assert!(!entries[0].nowrap);
        assert!(entries[0].format.is_empty());
        assert!(!entries[0].encoding_error);
        assert!(entries[0].msgctxt.is_none());
        assert_eq!(entries[0].msgid, Some(Message::new(2, "file")));
        assert_eq!(entries[0].msgid_plural, Some(Message::new(3, "files")));
        assert_eq!(
            entries[0].msgstr.get(&0),
            Some(Message::new(4, "fichier")).as_ref()
        );
        assert_eq!(
            entries[0].msgstr.get(&1),
            Some(Message::new(5, "fichiers")).as_ref()
        );
    }

    #[test]
    fn parse_comments() {
        let content = r#"
# Translator comment
#, fuzzy, python-format,   noqa, noqa:blank;pipes, no-wrap
#= keyword
#: src/main.rs:42
msgid "hello"
msgstr "bonjour"
"#;
        let mut parser = Parser::new(content.as_bytes());
        let entries = parser.by_ref().collect::<Vec<Entry>>();
        assert_eq!(entries[0].line_number, 2);
        assert_eq!(
            entries[0].keywords,
            vec![
                "fuzzy".to_string(),
                "python-format".to_string(),
                "noqa".to_string(),
                "noqa:blank;pipes".to_string(),
                "no-wrap".to_string(),
                "keyword".to_string(),
            ]
        );
        assert!(entries[0].fuzzy);
        assert!(entries[0].noqa);
        assert!(entries[0].nowrap);
        assert_eq!(entries[0].noqa_rules, vec!["blank", "pipes"]);
        assert_eq!(entries[0].format, "python");
        assert!(!entries[0].encoding_error);
        assert!(entries[0].msgctxt.is_none());
        assert_eq!(entries[0].msgid, Some(Message::new(6, "hello")));
        assert!(entries[0].msgid_plural.is_none());
        assert_eq!(
            entries[0].msgstr.get(&0),
            Some(Message::new(7, "bonjour")).as_ref()
        );
    }

    #[test]
    fn parse_multiline_strings() {
        let content = r#"
msgid ""
"hello "
"world"
msgstr ""
"bonjour "
"le monde"
"#;
        let mut parser = Parser::new(content.as_bytes());
        let entries = parser.by_ref().collect::<Vec<Entry>>();
        assert_eq!(entries[0].line_number, 2);
        assert!(entries[0].keywords.is_empty());
        assert!(!entries[0].fuzzy);
        assert!(!entries[0].noqa);
        assert!(!entries[0].nowrap);
        assert!(entries[0].format.is_empty());
        assert!(!entries[0].encoding_error);
        assert!(entries[0].msgctxt.is_none());
        assert_eq!(entries[0].msgid, Some(Message::new(2, "hello world")));
        assert!(entries[0].msgid_plural.is_none());
        assert_eq!(
            entries[0].msgstr.get(&0),
            Some(Message::new(5, "bonjour le monde")).as_ref()
        );
    }
}
