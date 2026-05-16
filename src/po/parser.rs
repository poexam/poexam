// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! PO file parser.

use std::borrow::Cow;

use crate::{po::entry::Entry, po::format::language::Language, po::message::Message};
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
    // Data and some general info parsed in the header.
    data: &'a [u8],
    data_len: usize,
    language: String,
    language_code: String,
    country: String,
    encoding: Option<&'static Encoding>,
    nplurals: u32,
    // Internal state of the parser.
    offset: usize,
    line_offset_start: usize,
    line_number: usize,
    next_line_number: usize,
    field: Field,
    encoding_error: bool,
}

impl<'d> Parser<'d> {
    /// Create a new `Parser` from the given byte slice.
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
    pub fn encoding_name(&self) -> &'static str {
        self.encoding
            .map_or_else(|| encoding_rs::UTF_8.name(), |enc| enc.name())
    }

    pub fn language(&self) -> &str {
        &self.language
    }

    pub fn language_code(&self) -> &str {
        &self.language_code
    }
    pub fn country(&self) -> &str {
        &self.country
    }

    /// Return the number of plurals defined in the header.
    pub const fn nplurals(&self) -> u32 {
        self.nplurals
    }

    /// Return the next line from the input data, updating the parser's location.
    fn next_line(&mut self) -> Option<&'d [u8]> {
        if self.offset >= self.data_len {
            return None;
        }
        let start = self.offset;
        self.line_offset_start = start;
        let end =
            memchr::memchr(b'\n', &self.data[start..]).map_or(self.data_len, |pos| start + pos);
        self.offset = end + 1;
        self.next_line_number += 1;
        Some(&self.data[start..end])
    }

    /// End offset of the line that was just read by `next_line` (clamped to `data_len`
    /// so the trailing-no-newline case stays in bounds).
    const fn line_end_offset(&self) -> usize {
        if self.offset > self.data_len {
            self.data_len
        } else {
            self.offset
        }
    }

    /// Parse the header of a PO entry to extract encoding information if present.
    fn parse_header(&mut self, entry: &Entry) {
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
            let keyword = keyword.trim();
            if keyword.eq_ignore_ascii_case("language") {
                self.language = value.trim().to_string();
                if let Some(pos) = value.find('_') {
                    self.language_code = value[..pos].trim().to_string();
                    self.country = value[pos + 1..].trim().to_string();
                } else {
                    self.language_code = self.language.clone();
                }
            } else if keyword.eq_ignore_ascii_case("content-type")
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
            } else if keyword.eq_ignore_ascii_case("plural-forms")
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
        for kw in line.split(|&b| b == b',') {
            let kw = kw.trim_ascii();
            match kw {
                b"fuzzy" => entry.fuzzy = true,
                b"noqa" => entry.noqa = true,
                b"no-wrap" => entry.nowrap = true,
                _ => {
                    if let Some(rules) = kw.strip_prefix(b"noqa:") {
                        entry.noqa_rules = rules
                            .split(|&b| b == b';')
                            .map(|r| String::from_utf8_lossy(r.trim_ascii()).into_owned())
                            .collect();
                    } else if let Some(stripped) = kw.strip_suffix(b"-format")
                        && let Ok(s) = str::from_utf8(stripped)
                    {
                        entry.format_language = Language::from(s);
                    }
                }
            }
            entry
                .keywords
                .push(String::from_utf8_lossy(kw).into_owned());
        }
    }

    /// Extract a string value from a line, and decode if necessary (not UTF-8).
    fn extract_string(&mut self, line: &'d [u8]) -> Cow<'d, str> {
        let Some(start) = memchr::memchr(b'"', line) else {
            return Cow::Borrowed("");
        };
        // Fast path: PO string lines almost always end with the closing quote,
        // so we avoid a second full scan.
        let end = if line.len() > start + 1 && line.last() == Some(&b'"') {
            line.len() - 1
        } else {
            match memchr::memrchr(b'"', line) {
                Some(end) if end != start => end,
                _ => return Cow::Borrowed(""),
            }
        };
        let bytes = &line[start + 1..end];
        if let Some(encoding) = self.encoding {
            let (cow, _, errors) = encoding.decode(bytes);
            if errors {
                self.encoding_error = true;
            }
            cow
        } else if let Ok(s) = str::from_utf8(bytes) {
            Cow::Borrowed(s)
        } else {
            self.encoding_error = true;
            String::from_utf8_lossy(bytes)
        }
    }

    /// Parse a message line and update the corresponding field in the `Entry`.
    ///
    /// The line can be a `msgctxt`, `msgid`, `msgid_plural`, `msgstr`, or a continued string.
    fn parse_message(&mut self, line: &'d [u8], entry: &mut Entry) {
        let line_start = self.line_offset_start;
        let line_end = self.line_end_offset();
        match line {
            [b'"', ..] => {
                let value = self.extract_string(line);
                match self.field {
                    Field::Comment => {}
                    Field::Ctxt => {
                        entry.append_msgctxt(value);
                        if let Some(msg) = entry.msgctxt.as_mut() {
                            msg.byte_range.end = line_end;
                        }
                    }
                    Field::Id => {
                        entry.append_msgid(value);
                        if let Some(msg) = entry.msgid.as_mut() {
                            msg.byte_range.end = line_end;
                        }
                    }
                    Field::IdPlural => {
                        entry.append_msgid_plural(value);
                        if let Some(msg) = entry.msgid_plural.as_mut() {
                            msg.byte_range.end = line_end;
                        }
                    }
                    Field::Str(idx) => {
                        entry.append_msgstr(idx, value);
                        if let Some(msg) = entry.msgstr.get_mut(&idx) {
                            msg.byte_range.end = line_end;
                        }
                    }
                }
            }
            [b'm', b's', b'g', b'c', b't', b'x', b't', ..] => {
                self.field = Field::Ctxt;
                entry.msgctxt = Some(Message::new(
                    self.line_number,
                    self.extract_string(line),
                    line_start..line_end,
                ));
            }
            [
                b'm',
                b's',
                b'g',
                b'i',
                b'd',
                b'_',
                b'p',
                b'l',
                b'u',
                b'r',
                b'a',
                b'l',
                ..,
            ] => {
                self.field = Field::IdPlural;
                entry.msgid_plural = Some(Message::new(
                    self.line_number,
                    self.extract_string(line),
                    line_start..line_end,
                ));
            }
            [b'm', b's', b'g', b'i', b'd', ..] => {
                self.field = Field::Id;
                entry.msgid = Some(Message::new(
                    self.line_number,
                    self.extract_string(line),
                    line_start..line_end,
                ));
            }
            [b'm', b's', b'g', b's', b't', b'r', b'[', ..] => {
                if let Some(idx_end) = memchr::memchr(b']', line)
                    && let Ok(str_idx) = str::from_utf8(&line[7..idx_end])
                    && let Ok(idx) = str_idx.parse::<u32>()
                {
                    self.field = Field::Str(idx);
                    entry.msgstr.insert(
                        idx,
                        Message::new(
                            self.line_number,
                            self.extract_string(line),
                            line_start..line_end,
                        ),
                    );
                }
            }
            [b'm', b's', b'g', b's', b't', b'r', ..] => {
                self.field = Field::Str(0);
                entry.msgstr.insert(
                    0,
                    Message::new(
                        self.line_number,
                        self.extract_string(line),
                        line_start..line_end,
                    ),
                );
            }
            _ => {}
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
                    entry.byte_range.end = self.line_end_offset();
                    entry.encoding_error = self.encoding_error;
                    entry.unescape_strings();
                    self.parse_header(&entry);
                    return Some(entry);
                }
                entry.line_number = self.next_line_number;
                self.line_number = self.next_line_number;
                continue;
            }
            if !started {
                entry.byte_range.start = self.line_offset_start;
                started = true;
            }
            match line {
                // Workflow and sticky flags.
                [b'#', b',' | b'=', keywords @ ..] => {
                    Parser::parse_keywords(keywords, &mut entry);
                }
                // Obsolete entry with a message (start or continued).
                [b'#', b'~', b' ', msg @ ..] => {
                    entry.obsolete = true;
                    self.parse_message(msg, &mut entry);
                }
                // Flag "noqa:xxx" in a comment (with rules).
                [b'#', b' ', b'n', b'o', b'q', b'a', b':', rules @ ..] => {
                    entry.noqa_rules = rules
                        .split(|&b| b == b';')
                        .map(|r| String::from_utf8_lossy(r.trim_ascii()).into_owned())
                        .collect();
                }
                // Flag "noqa" in a comment.
                [b'#', b' ', b'n', b'o', b'q', b'a', ..] => {
                    entry.noqa = true;
                }
                // Message line (start or continued).
                [b'm' | b'"', ..] => {
                    self.parse_message(line, &mut entry);
                }
                _ => {}
            }
            self.line_number = self.next_line_number;
        }
        if started {
            // Send the last entry if we reached the end of data.
            entry.byte_range.end = self.line_end_offset();
            entry.encoding_error = self.encoding_error;
            entry.unescape_strings();
            self.parse_header(&entry);
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
        assert!(Parser::new(b"").next().is_none());
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
        assert_eq!(entries[0].format_language, Language::Null);
        assert!(!entries[0].encoding_error);
        assert_eq!(parser.nplurals, 2);
        assert!(entries[0].msgctxt.is_none());
        assert_eq!(entries[0].msgid, Some(Message::new(2, "", 0..0)));
        assert!(entries[0].msgid_plural.is_none());
        assert_eq!(
            entries[0].msgstr.get(&0),
            Some(Message::new(
                3,
                "test\n\
                Project-Id-Version: my_project\n\
                Report-Msgid-Bugs-To: someone@example.com\n\
                Language: fr\n\
                Plural-Forms: nplurals=2; plural=(n > 1);\n",
                0..0,
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
        assert_eq!(entries[0].format_language, Language::Null);
        assert!(!entries[0].encoding_error);
        assert!(entries[0].msgctxt.is_none());
        assert_eq!(entries[0].msgid, Some(Message::new(2, "hello", 0..0)));
        assert!(entries[0].msgid_plural.is_none());
        assert_eq!(
            entries[0].msgstr.get(&0),
            Some(Message::new(3, "bonjour", 0..0)).as_ref()
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
        assert_eq!(entries[0].format_language, Language::Null);
        assert!(!entries[0].encoding_error);
        assert!(entries[0].msgctxt.is_none());
        assert_eq!(entries[0].msgid, Some(Message::new(2, "", 0..0)));
        assert!(entries[0].msgid_plural.is_none());
        assert_eq!(
            entries[0].msgstr.get(&0),
            Some(Message::new(
                3,
                "Content-Type: text/plain; charset=ISO-8859-15\n",
                0..0,
            ))
            .as_ref()
        );
        assert!(entries[1].keywords.is_empty());
        assert!(!entries[1].fuzzy);
        assert!(!entries[1].noqa);
        assert!(!entries[1].nowrap);
        assert_eq!(entries[1].format_language, Language::Null);
        assert!(!entries[1].encoding_error);
        assert!(entries[1].msgctxt.is_none());
        assert_eq!(entries[1].msgid, Some(Message::new(5, "tested", 0..0)));
        assert!(entries[1].msgid_plural.is_none());
        assert_eq!(
            entries[1].msgstr.get(&0),
            Some(Message::new(6, "testé", 0..0)).as_ref()
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
        assert_eq!(entries[0].format_language, Language::Null);
        assert!(!entries[0].encoding_error);
        assert!(entries[0].msgctxt.is_none());
        assert_eq!(entries[0].msgid, Some(Message::new(2, "", 0..0)));
        assert!(entries[0].msgid_plural.is_none());
        assert_eq!(
            entries[0].msgstr.get(&0),
            Some(Message::new(
                3,
                "Content-Type: text/plain; charset=UTF-8\n",
                0..0
            ))
            .as_ref()
        );
        assert!(entries[1].keywords.is_empty());
        assert!(!entries[1].fuzzy);
        assert!(!entries[1].noqa);
        assert!(!entries[1].nowrap);
        assert_eq!(entries[1].format_language, Language::Null);
        assert!(entries[1].encoding_error);
        assert!(entries[1].msgctxt.is_none());
        assert_eq!(entries[1].msgid, Some(Message::new(5, "tested", 0..0)));
        assert!(entries[1].msgid_plural.is_none());
        assert_eq!(
            entries[1].msgstr.get(&0),
            Some(Message::new(6, "test�", 0..0)).as_ref()
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
        assert_eq!(entries[0].format_language, Language::Null);
        assert!(!entries[0].encoding_error);
        assert_eq!(
            entries[0].msgctxt,
            Some(Message::new(2, "month of the year", 0..0))
        );
        assert_eq!(entries[0].msgid, Some(Message::new(3, "may", 0..0)));
        assert!(entries[0].msgid_plural.is_none());
        assert_eq!(
            entries[0].msgstr.get(&0),
            Some(Message::new(4, "mai", 0..0)).as_ref()
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
        assert_eq!(entries[0].format_language, Language::Null);
        assert!(!entries[0].encoding_error);
        assert!(entries[0].msgctxt.is_none());
        assert_eq!(entries[0].msgid, Some(Message::new(2, "hello", 0..0)));
        assert!(entries[0].msgid_plural.is_none());
        assert_eq!(
            entries[0].msgstr.get(&0),
            Some(Message::new(3, "bonjour", 0..0)).as_ref()
        );
        assert_eq!(entries[1].line_number, 5);
        assert!(entries[1].keywords.is_empty());
        assert!(!entries[1].fuzzy);
        assert!(!entries[1].noqa);
        assert!(!entries[1].nowrap);
        assert_eq!(entries[1].format_language, Language::Null);
        assert!(!entries[1].encoding_error);
        assert!(entries[1].msgctxt.is_none());
        assert_eq!(entries[1].msgid, Some(Message::new(5, "hello 2", 0..0)));
        assert!(entries[1].msgid_plural.is_none());
        assert_eq!(
            entries[1].msgstr.get(&0),
            Some(Message::new(6, "", 0..0)).as_ref()
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
        assert_eq!(entries[0].format_language, Language::Null);
        assert!(!entries[0].encoding_error);
        assert!(entries[0].msgctxt.is_none());
        assert_eq!(entries[0].msgid, Some(Message::new(2, "file", 0..0)));
        assert_eq!(
            entries[0].msgid_plural,
            Some(Message::new(3, "files", 0..0))
        );
        assert_eq!(
            entries[0].msgstr.get(&0),
            Some(Message::new(4, "fichier", 0..0)).as_ref()
        );
        assert_eq!(
            entries[0].msgstr.get(&1),
            Some(Message::new(5, "fichiers", 0..0)).as_ref()
        );
    }

    #[test]
    fn parse_comments() {
        let content = r#"
# Translator comment
#, fuzzy, c-format,   noqa, noqa:blank; pipes, no-wrap
#= keyword
#: src/main.rs:42
msgid "hello, %s"
msgstr "bonjour, %s"
"#;
        let mut parser = Parser::new(content.as_bytes());
        let entries = parser.by_ref().collect::<Vec<Entry>>();
        assert_eq!(entries[0].line_number, 2);
        assert_eq!(
            entries[0].keywords,
            vec![
                "fuzzy".to_string(),
                "c-format".to_string(),
                "noqa".to_string(),
                "noqa:blank; pipes".to_string(),
                "no-wrap".to_string(),
                "keyword".to_string(),
            ]
        );
        assert!(entries[0].fuzzy);
        assert!(entries[0].noqa);
        assert!(entries[0].nowrap);
        assert_eq!(entries[0].noqa_rules, vec!["blank", "pipes"]);
        assert_eq!(entries[0].format_language, Language::C);
        assert!(!entries[0].encoding_error);
        assert!(entries[0].msgctxt.is_none());
        assert_eq!(entries[0].msgid, Some(Message::new(6, "hello, %s", 0..0)));
        assert!(entries[0].msgid_plural.is_none());
        assert_eq!(
            entries[0].msgstr.get(&0),
            Some(Message::new(7, "bonjour, %s", 0..0)).as_ref()
        );
        // Parse "noqa" comment.
        let content = r#"
# noqa
#, c-format
msgid "hello, %s"
msgstr "bonjour, %s"
"#;
        let mut parser = Parser::new(content.as_bytes());
        let entries = parser.by_ref().collect::<Vec<Entry>>();
        assert_eq!(entries[0].line_number, 2);
        assert_eq!(entries[0].keywords, vec!["c-format"]);
        assert!(!entries[0].fuzzy);
        assert!(entries[0].noqa);
        assert!(!entries[0].nowrap);
        assert!(entries[0].noqa_rules.is_empty());
        assert_eq!(entries[0].format_language, Language::C);
        assert!(!entries[0].encoding_error);
        assert!(entries[0].msgctxt.is_none());
        assert_eq!(entries[0].msgid, Some(Message::new(4, "hello, %s", 0..0)));
        assert!(entries[0].msgid_plural.is_none());
        assert_eq!(
            entries[0].msgstr.get(&0),
            Some(Message::new(5, "bonjour, %s", 0..0)).as_ref()
        );
        // Parse "noqa:xxx" comment (with rules).
        let content = r#"
# noqa:blank; pipes
#, c-format
msgid "hello, %s"
msgstr "bonjour, %s"
"#;
        let mut parser = Parser::new(content.as_bytes());
        let entries = parser.by_ref().collect::<Vec<Entry>>();
        assert_eq!(entries[0].line_number, 2);
        assert_eq!(entries[0].keywords, vec!["c-format"]);
        assert!(!entries[0].fuzzy);
        assert!(!entries[0].noqa);
        assert!(!entries[0].nowrap);
        assert_eq!(entries[0].noqa_rules, vec!["blank", "pipes"]);
        assert_eq!(entries[0].format_language, Language::C);
        assert!(!entries[0].encoding_error);
        assert!(entries[0].msgctxt.is_none());
        assert_eq!(entries[0].msgid, Some(Message::new(4, "hello, %s", 0..0)));
        assert!(entries[0].msgid_plural.is_none());
        assert_eq!(
            entries[0].msgstr.get(&0),
            Some(Message::new(5, "bonjour, %s", 0..0)).as_ref()
        );
    }

    #[test]
    fn byte_range_identity_roundtrip() {
        // Parsing then writing with no replacements must yield byte-identical output.
        // Run this on the example fixture so it exercises a realistic file.
        let original: &[u8] =
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/fr.po"));
        // Drain the parser so all entries are produced (and all internal byte ranges are tracked).
        let entries: Vec<Entry> = Parser::new(original).collect();
        assert!(!entries.is_empty());
        let out = crate::po::writer::write_with_replacements(original, vec![]).unwrap();
        assert_eq!(out, original, "no-op rewrite must be byte-identical");
    }

    #[test]
    fn byte_range_self_splice_roundtrip() {
        // For every Message in the fixture, replace its byte_range with the bytes
        // currently at that range. The result must be byte-identical to the input,
        // proving the recorded offsets are exact.
        let original: &[u8] =
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/fr.po"));
        let entries: Vec<Entry> = Parser::new(original).collect();
        let mut replacements: Vec<(std::ops::Range<usize>, Vec<u8>)> = Vec::new();
        for entry in &entries {
            for msg in entry
                .msgctxt
                .iter()
                .chain(entry.msgid.iter())
                .chain(entry.msgid_plural.iter())
                .chain(entry.msgstr.values())
            {
                assert!(
                    msg.byte_range.start < msg.byte_range.end,
                    "every message must have a non-empty byte range"
                );
                assert!(msg.byte_range.end <= original.len());
                let bytes = original[msg.byte_range.clone()].to_vec();
                replacements.push((msg.byte_range.clone(), bytes));
            }
        }
        let out = crate::po::writer::write_with_replacements(original, replacements).unwrap();
        assert_eq!(
            out, original,
            "self-splice with the same bytes must be byte-identical"
        );
    }

    #[test]
    fn byte_range_entry_covers_entry_bytes() {
        let content = b"\nmsgid \"hello\"\nmsgstr \"bonjour\"\n\nmsgid \"hello 2\"\nmsgstr \"\"\n";
        let entries: Vec<Entry> = Parser::new(content).collect();
        assert_eq!(entries.len(), 2);
        // First entry's byte range should start at "msgid" of entry 1.
        let r0 = entries[0].byte_range.clone();
        assert_eq!(&content[r0.start..r0.start + 5], b"msgid");
        // It should end past the blank-line separator (or at end of buffer).
        assert!(r0.end > 0 && r0.end <= content.len());
        // And the second entry must start strictly after entry 0's range start.
        assert!(entries[1].byte_range.start >= r0.end - 1);
    }

    #[test]
    fn byte_range_message_covers_block_with_continuation() {
        let content =
            b"msgid \"\"\n\"hello \"\n\"world\"\nmsgstr \"\"\n\"bonjour \"\n\"le monde\"\n";
        let entries: Vec<Entry> = Parser::new(content).collect();
        let msgid = entries[0].msgid.as_ref().unwrap();
        let msgstr0 = entries[0].msgstr.get(&0).unwrap();
        // msgid block: from b"msgid" up to and including the b"world"\n line.
        let id_slice = &content[msgid.byte_range.clone()];
        assert!(id_slice.starts_with(b"msgid"));
        assert!(id_slice.ends_with(b"\"world\"\n"));
        // msgstr block: from b"msgstr" up to and including the b"le monde"\n line.
        let str_slice = &content[msgstr0.byte_range.clone()];
        assert!(str_slice.starts_with(b"msgstr"));
        assert!(str_slice.ends_with(b"\"le monde\"\n"));
        // Blocks must not overlap.
        assert!(msgid.byte_range.end <= msgstr0.byte_range.start);
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
        assert_eq!(entries[0].format_language, Language::Null);
        assert!(!entries[0].encoding_error);
        assert!(entries[0].msgctxt.is_none());
        assert_eq!(entries[0].msgid, Some(Message::new(2, "hello world", 0..0)));
        assert!(entries[0].msgid_plural.is_none());
        assert_eq!(
            entries[0].msgstr.get(&0),
            Some(Message::new(5, "bonjour le monde", 0..0)).as_ref()
        );
    }
}
