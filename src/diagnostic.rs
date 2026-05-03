// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Diagnostic for PO files.

use std::{
    borrow::Cow,
    collections::HashSet,
    path::{Path, PathBuf},
};

use clap::ValueEnum;
use colored::Colorize;
use serde::{
    Deserialize, Serialize,
    ser::{SerializeStruct, Serializer},
};

use crate::po::{entry::Entry, message::Message};

const HIGHLIGHT_COLOR: &str = "bright yellow";
const HIGHLIGHT_ON_COLOR: &str = "red";

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Ord,
    PartialOrd,
    Hash,
    Serialize,
    Deserialize,
    ValueEnum,
)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    #[default]
    Info,
    Warning,
    Error,
}

#[derive(Debug, Default)]
pub struct DiagnosticLine {
    pub line_number: usize,
    pub message: String,
    pub highlights: Vec<(usize, usize)>,
}

#[derive(Debug, Default, Serialize)]
pub struct Diagnostic {
    pub path: PathBuf,
    pub rule: &'static str,
    pub severity: Severity,
    pub message: Cow<'static, str>,
    pub lines: Vec<DiagnosticLine>,
    pub misspelled_words: HashSet<String>,
}

impl std::fmt::Display for Severity {
    /// Format the `Severity` as a colored string for display.
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = match self {
            Self::Info => "info".cyan(),
            Self::Warning => "warning".yellow(),
            Self::Error => "error".bright_red().bold(),
        };
        write!(f, "{s}")
    }
}

impl Serialize for DiagnosticLine {
    /// Custom serialization for `DiagnosticLine` to convert highlights from byte positions to character positions.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("DiagnosticLine", 3)?;
        state.serialize_field("line_number", &self.line_number)?;
        state.serialize_field("message", &self.message)?;
        // Convert highlights from byte positions to character positions for serialization.
        let hl: Vec<_> = self
            .highlights
            .iter()
            .map(|(s, e)| {
                (
                    self.message[..*s].chars().count(),
                    self.message[..*e].chars().count(),
                )
            })
            .collect();
        state.serialize_field("highlights", &hl)?;
        state.end()
    }
}

impl DiagnosticLine {
    /// Highlight multiple substrings from `start` to `end` with the given text and background colors.
    fn highlight_list_pos(s: &str, list_pos: &[(usize, usize)]) -> String {
        let mut result = String::new();
        let mut pos = 0;
        for (start, end) in list_pos {
            if *start < pos {
                continue;
            }
            result.push_str(&s[pos..*start]);
            result.push_str(
                &s[*start..*end]
                    .color(HIGHLIGHT_COLOR)
                    .bold()
                    .on_color(HIGHLIGHT_ON_COLOR)
                    .to_string(),
            );
            pos = *end;
        }
        result.push_str(&s[pos..]);
        result
    }

    /// Get the message with highlights applied.
    fn message_hl_color(&self) -> Cow<'_, str> {
        if self.highlights.is_empty() {
            Cow::Borrowed(&self.message)
        } else {
            Cow::Owned(Self::highlight_list_pos(&self.message, &self.highlights))
        }
    }
}

impl Diagnostic {
    /// Create a new `Diagnostic` with the given path, severity, and message.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        path: &Path,
        rule: &'static str,
        severity: Severity,
        message: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self {
            path: PathBuf::from(path),
            rule,
            severity,
            message: message.into(),
            ..Default::default()
        }
    }

    /// Add keywords of a PO entry to the diagnostic.
    pub fn with_keywords(mut self, entry: &Entry) -> Self {
        for line in entry.keywords_to_po_lines() {
            self.add_line(0, &line, []);
        }
        self
    }

    /// Add messages of a PO entry to the diagnostic.
    pub fn with_entry(mut self, entry: &Entry) -> Self {
        for (line_no, line) in entry.msg_to_po_lines() {
            self.add_line(line_no, &line, []);
        }
        self
    }

    /// Add one message to the diagnostic.
    pub fn with_msg(mut self, msg: &Message) -> Self {
        self.add_line(msg.line_number, &msg.value, []);
        self
    }

    /// Add one message to the diagnostic with the given highlights.
    pub fn with_msg_hl<I>(mut self, msg: &Message, hl: I) -> Self
    where
        I: IntoIterator<Item = (usize, usize)>,
    {
        self.add_line(msg.line_number, &msg.value, hl);
        self
    }

    /// Add two messages (typically msgid and msgstr) to the diagnostic.
    pub fn with_msgs(mut self, msgid: &Message, msgstr: &Message) -> Self {
        self.add_line(msgid.line_number, &msgid.value, []);
        self.add_line(0, "", []);
        self.add_line(msgstr.line_number, &msgstr.value, []);
        self
    }

    /// Add two messages (typically msgid and msgstr) to the diagnostic with the given highlights.
    pub fn with_msgs_hl<A, B>(
        mut self,
        msgid: &Message,
        hl_id: A,
        msgstr: &Message,
        hl_str: B,
    ) -> Self
    where
        A: IntoIterator<Item = (usize, usize)>,
        B: IntoIterator<Item = (usize, usize)>,
    {
        self.add_line(msgid.line_number, &msgid.value, hl_id);
        self.add_line(0, "", []);
        self.add_line(msgstr.line_number, &msgstr.value, hl_str);
        self
    }

    /// Add multiple lines to the diagnostic with the given multiline string.
    pub fn with_multiline(mut self, lines: &str) -> Self {
        if !lines.trim().is_empty() {
            for line in lines.lines() {
                self.add_line(0, line, []);
            }
        }
        self
    }

    /// Add misspelled words to the diagnostic.
    pub fn with_misspelled_words(mut self, misspelled_words: HashSet<&str>) -> Self {
        self.misspelled_words = misspelled_words.into_iter().map(String::from).collect();
        self
    }

    /// Add a line message to the diagnostic with the given line number and highlights.
    pub fn add_line<I>(&mut self, line: usize, message: impl Into<String>, highlights: I)
    where
        I: IntoIterator<Item = (usize, usize)>,
    {
        self.lines.push(DiagnosticLine {
            line_number: line,
            message: message.into(),
            highlights: highlights.into_iter().collect(),
        });
    }

    /// Build the diagnostic message (append misspelled words if any).
    pub(crate) fn build_message(&self) -> Cow<'_, str> {
        if self.misspelled_words.is_empty() {
            Cow::Borrowed(&self.message)
        } else {
            // Sort misspelled words for predictable output.
            let mut list_words = self
                .misspelled_words
                .iter()
                .map(String::as_str)
                .collect::<Vec<&str>>();
            list_words.sort_unstable();
            Cow::Owned(format!("{}: {}", self.message, list_words.join(", ")))
        }
    }

    /// Append the formatted diagnostic line (number + message) with colors to `out`.
    ///
    /// `prefix_lf_empty` is the line-continuation prefix; the caller computes it
    /// once per `format_lines` call and passes it down here.
    fn format_line_into(out: &mut String, line: &DiagnosticLine, prefix_lf_empty: &str) {
        let prefix_line: Cow<'_, str> = if line.line_number > 0 {
            Cow::Owned(format!("{:7} | ", line.line_number).cyan().to_string())
        } else {
            Cow::Borrowed(prefix_lf_empty)
        };
        if line.message.is_empty() {
            out.push_str(&prefix_line);
            return;
        }
        for (idx, l) in line.message_hl_color().lines().enumerate() {
            if idx == 0 {
                out.push_str(&prefix_line);
            } else {
                out.push('\n');
                out.push_str(prefix_lf_empty);
            }
            out.push_str(l);
        }
    }

    /// Format the diagnostic lines with colors for display.
    fn format_lines(&self) -> String {
        if self.lines.is_empty() {
            return "\n".to_string();
        }
        let bar = "        |".cyan().to_string();
        let prefix_lf_empty = "        | ".cyan().to_string();
        let mut out = String::new();
        out.push('\n');
        out.push_str(&bar);
        for line in &self.lines {
            out.push('\n');
            Self::format_line_into(&mut out, line, &prefix_lf_empty);
        }
        out.push('\n');
        out.push_str(&bar);
        out.push('\n');
        out
    }
}

impl std::fmt::Display for Diagnostic {
    /// Format the `Diagnostic` for display, including file, severity, message, and context.
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let str_first_line = self
            .lines
            .iter()
            .find(|line| line.line_number > 0)
            .map_or_else(String::new, |line| format!(":{}", line.line_number));
        write!(
            f,
            "{}{str_first_line}: [{}:{}] {}{}",
            self.path.display().to_string().white().bold(),
            self.severity,
            self.rule,
            self.build_message(),
            self.format_lines(),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn entry_with_msg(line: usize, msgid: &str, msgstr: &str) -> Entry {
        let mut entry = Entry::new(line);
        entry.msgid = Some(Message::new(line + 1, msgid));
        let mut msgstr_map = BTreeMap::new();
        msgstr_map.insert(0_u32, Message::new(line + 2, msgstr));
        entry.msgstr = msgstr_map;
        entry
    }

    #[test]
    fn test_severity_default() {
        assert_eq!(Severity::default(), Severity::Info);
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
    }

    #[test]
    fn test_severity_display() {
        colored::control::set_override(false);
        assert_eq!(Severity::Info.to_string(), "info");
        assert_eq!(Severity::Warning.to_string(), "warning");
        assert_eq!(Severity::Error.to_string(), "error");
    }

    #[test]
    fn test_diagnostic_new() {
        let diag = Diagnostic::new(
            Path::new("test.po"),
            "blank",
            Severity::Warning,
            "blank translation".to_string(),
        );
        assert_eq!(diag.path, PathBuf::from("test.po"));
        assert_eq!(diag.rule, "blank");
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "blank translation");
        assert!(diag.lines.is_empty());
        assert!(diag.misspelled_words.is_empty());
    }

    #[test]
    fn test_add_line() {
        let mut diag = Diagnostic::new(
            Path::new("test.po"),
            "blank",
            Severity::Info,
            String::new(),
        );
        diag.add_line(42, "msgstr \"\"", [(8, 9)]);
        assert_eq!(diag.lines.len(), 1);
        assert_eq!(diag.lines[0].line_number, 42);
        assert_eq!(diag.lines[0].message, "msgstr \"\"");
        assert_eq!(diag.lines[0].highlights, vec![(8, 9)]);
    }

    #[test]
    fn test_with_msg() {
        let msg = Message::new(10, "hello");
        let diag = Diagnostic::new(Path::new("a.po"), "r", Severity::Info, String::new())
            .with_msg(&msg);
        assert_eq!(diag.lines.len(), 1);
        assert_eq!(diag.lines[0].line_number, 10);
        assert_eq!(diag.lines[0].message, "hello");
        assert!(diag.lines[0].highlights.is_empty());
    }

    #[test]
    fn test_with_msg_hl() {
        let msg = Message::new(10, "hello");
        let diag = Diagnostic::new(Path::new("a.po"), "r", Severity::Info, String::new())
            .with_msg_hl(&msg, [(0, 5)]);
        assert_eq!(diag.lines[0].highlights, vec![(0, 5)]);
    }

    #[test]
    fn test_with_msgs_inserts_separator() {
        let msgid = Message::new(10, "hello");
        let msgstr = Message::new(11, "bonjour");
        let diag = Diagnostic::new(Path::new("a.po"), "r", Severity::Info, String::new())
            .with_msgs(&msgid, &msgstr);
        assert_eq!(diag.lines.len(), 3);
        assert_eq!(diag.lines[0].line_number, 10);
        assert_eq!(diag.lines[0].message, "hello");
        assert_eq!(diag.lines[1].line_number, 0);
        assert_eq!(diag.lines[1].message, "");
        assert_eq!(diag.lines[2].line_number, 11);
        assert_eq!(diag.lines[2].message, "bonjour");
    }

    #[test]
    fn test_with_msgs_hl() {
        let msgid = Message::new(10, "hello");
        let msgstr = Message::new(11, "bonjour");
        let diag = Diagnostic::new(Path::new("a.po"), "r", Severity::Info, String::new())
            .with_msgs_hl(&msgid, [(0, 1)], &msgstr, [(2, 4)]);
        assert_eq!(diag.lines[0].highlights, vec![(0, 1)]);
        assert!(diag.lines[1].highlights.is_empty());
        assert_eq!(diag.lines[2].highlights, vec![(2, 4)]);
    }

    #[test]
    fn test_with_keywords_and_entry() {
        let mut entry = entry_with_msg(5, "hello", "bonjour");
        entry.keywords = vec!["fuzzy".to_string(), "c-format".to_string()];
        let diag = Diagnostic::new(Path::new("a.po"), "r", Severity::Info, String::new())
            .with_keywords(&entry)
            .with_entry(&entry);
        // 2 keyword lines + 2 entry lines (msgid, msgstr).
        assert_eq!(diag.lines.len(), 4);
        assert_eq!(diag.lines[0].line_number, 0);
        assert_eq!(diag.lines[0].message, "#, fuzzy");
        assert_eq!(diag.lines[1].message, "#, c-format");
        assert_eq!(diag.lines[2].message, "msgid \"hello\"");
        assert_eq!(diag.lines[2].line_number, 6);
        assert_eq!(diag.lines[3].message, "msgstr \"bonjour\"");
        assert_eq!(diag.lines[3].line_number, 7);
    }

    #[test]
    fn test_with_multiline() {
        let diag = Diagnostic::new(Path::new("a.po"), "r", Severity::Info, String::new())
            .with_multiline("a\nb\nc");
        assert_eq!(diag.lines.len(), 3);
        assert_eq!(diag.lines[0].message, "a");
        assert_eq!(diag.lines[1].message, "b");
        assert_eq!(diag.lines[2].message, "c");
        for line in &diag.lines {
            assert_eq!(line.line_number, 0);
        }
    }

    #[test]
    fn test_with_multiline_empty_or_blank_is_skipped() {
        let d_empty = Diagnostic::new(Path::new("a.po"), "r", Severity::Info, String::new())
            .with_multiline("");
        assert!(d_empty.lines.is_empty());
        let d_blank = Diagnostic::new(Path::new("a.po"), "r", Severity::Info, String::new())
            .with_multiline("   \n\t\n");
        assert!(d_blank.lines.is_empty());
    }

    #[test]
    fn test_with_misspelled_words() {
        let mut set = HashSet::new();
        set.insert("xxa");
        set.insert("xxb");
        let diag = Diagnostic::new(Path::new("a.po"), "r", Severity::Info, "msg".to_string())
            .with_misspelled_words(set);
        assert_eq!(diag.misspelled_words.len(), 2);
        assert!(diag.misspelled_words.contains("xxa"));
        assert!(diag.misspelled_words.contains("xxb"));
    }

    #[test]
    fn test_build_message_no_misspelled() {
        let diag = Diagnostic::new(Path::new("a.po"), "r", Severity::Info, "msg".to_string());
        assert_eq!(diag.build_message(), "msg");
    }

    #[test]
    fn test_build_message_misspelled_sorted_and_joined() {
        let mut set = HashSet::new();
        set.insert("xxc");
        set.insert("xxb");
        set.insert("xxa");
        let diag = Diagnostic::new(
            Path::new("a.po"),
            "spelling-str",
            Severity::Info,
            "misspelled words".to_string(),
        )
        .with_misspelled_words(set);
        assert_eq!(diag.build_message(), "misspelled words: xxa, xxb, xxc");
    }

    #[test]
    fn test_diagnostic_line_serialize_byte_to_char_positions() {
        // "café" = 'c'(1B) 'a'(1B) 'f'(1B) 'é'(2B): 5 bytes, 4 chars.
        // Highlight bytes (2, 5) cover "fé"; chars (2, 4) — "ca"=2 and "café"=4.
        let line = DiagnosticLine {
            line_number: 7,
            message: "café".to_string(),
            highlights: vec![(2, 5)],
        };
        let v = serde_json::to_value(&line).expect("DiagnosticLine should serialize");
        assert_eq!(v["line_number"], 7);
        assert_eq!(v["message"], "café");
        assert_eq!(v["highlights"], serde_json::json!([[2, 4]]));
    }

    #[test]
    fn test_diagnostic_line_serialize_no_highlights() {
        let line = DiagnosticLine {
            line_number: 3,
            message: "hello".to_string(),
            highlights: vec![],
        };
        let v = serde_json::to_value(&line).expect("DiagnosticLine should serialize");
        assert_eq!(v["highlights"], serde_json::json!([]));
    }

    #[test]
    fn test_diagnostic_display_with_lines() {
        colored::control::set_override(false);
        let msgid = Message::new(10, "hello");
        let msgstr = Message::new(11, "");
        let diag = Diagnostic::new(
            Path::new("fr.po"),
            "blank",
            Severity::Warning,
            "blank translation".to_string(),
        )
        .with_msgs(&msgid, &msgstr);
        let s = diag.to_string();
        assert!(s.starts_with("fr.po:10: [warning:blank] blank translation"));
        assert!(s.contains("     10 | hello"));
        assert!(s.contains("     11 | "));
    }

    #[test]
    fn test_diagnostic_display_no_lines() {
        colored::control::set_override(false);
        let diag = Diagnostic::new(
            Path::new("a.po"),
            "encoding",
            Severity::Info,
            "bad encoding".to_string(),
        );
        let s = diag.to_string();
        // No lines → no ":line" suffix on the path.
        assert!(s.starts_with("a.po: [info:encoding] bad encoding"));
    }
}
