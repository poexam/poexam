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
    pub message: String,
    pub lines: Vec<DiagnosticLine>,
    pub misspelled_words: HashSet<String>,
}

impl std::fmt::Display for Severity {
    /// Format the `Severity` as a colored string for display.
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = match self {
            Severity::Info => "info".cyan(),
            Severity::Warning => "warning".yellow(),
            Severity::Error => "error".bright_red().bold(),
        };
        write!(f, "{s}")
    }
}

impl Serialize for DiagnosticLine {
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
            Cow::Owned(DiagnosticLine::highlight_list_pos(
                &self.message,
                &self.highlights,
            ))
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
        message: impl Into<String>,
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
            self.add_line(0, &line, &[]);
        }
        self
    }

    /// Add messages of a PO entry to the diagnostic.
    pub fn with_entry(mut self, entry: &Entry) -> Self {
        for (line_no, line) in entry.msg_to_po_lines() {
            self.add_line(line_no, &line, &[]);
        }
        self
    }

    /// Add one message to the diagnostic.
    pub fn with_msg(mut self, msg: &Message) -> Self {
        self.add_line(msg.line_number, &msg.value, &[]);
        self
    }

    /// Add one message to the diagnostic with the given highlights.
    pub fn with_msg_hl(mut self, msg: &Message, hl: &[(usize, usize)]) -> Self {
        self.add_line(msg.line_number, &msg.value, hl);
        self
    }

    /// Add two messages (typically msgid and msgstr) to the diagnostic.
    pub fn with_msgs(mut self, msgid: &Message, msgstr: &Message) -> Self {
        self.add_line(msgid.line_number, &msgid.value, &[]);
        self.add_line(0, "", &[]);
        self.add_line(msgstr.line_number, &msgstr.value, &[]);
        self
    }

    /// Add two messages (typically msgid and msgstr) to the diagnostic with the given highlights.
    pub fn with_msgs_hl(
        mut self,
        msgid: &Message,
        hl_id: &[(usize, usize)],
        msgstr: &Message,
        hl_str: &[(usize, usize)],
    ) -> Self {
        self.add_line(msgid.line_number, &msgid.value, hl_id);
        self.add_line(0, "", &[]);
        self.add_line(msgstr.line_number, &msgstr.value, hl_str);
        self
    }

    /// Add multiple lines to the diagnostic with the given multiline string.
    pub fn with_multiline(mut self, lines: &str) -> Self {
        if !lines.trim().is_empty() {
            for line in lines.lines() {
                self.add_line(0, line, &[]);
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
    pub fn add_line(
        &mut self,
        line: usize,
        message: impl Into<String>,
        highlights: &[(usize, usize)],
    ) {
        self.lines.push(DiagnosticLine {
            line_number: line,
            message: message.into(),
            highlights: highlights.to_vec(),
        });
    }

    /// Add a message to the diagnostic with the given line number and highlights.
    pub fn add_message(&mut self, line: usize, message: &str, highlights: &[(usize, usize)]) {
        self.lines.push(DiagnosticLine {
            line_number: line,
            message: message.to_string(),
            highlights: highlights.to_vec(),
        });
    }

    /// Build the diagnostic message (append misspelled words if any).
    pub fn build_message(&self) -> Cow<'_, str> {
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

    /// Format the diagnostic line (number + message) with colors for display.
    fn format_line(line: &DiagnosticLine) -> String {
        let prefix_lf_empty = "        | ".cyan().to_string();
        let prefix_line = if line.line_number > 0 {
            format!("{:7} | ", line.line_number).cyan().to_string()
        } else {
            prefix_lf_empty.clone()
        };
        if line.message.is_empty() {
            return prefix_line;
        }
        let mut out = String::new();
        for (idx, line) in line.message_hl_color().lines().enumerate() {
            if idx == 0 {
                out.push_str(&prefix_line);
            } else {
                out.push('\n');
                out.push_str(&prefix_lf_empty);
            }
            out.push_str(line);
        }
        out
    }

    /// Format the diagnostic lines with colors for display.
    fn format_lines(&self) -> String {
        if self.lines.is_empty() {
            "\n".to_string()
        } else {
            let mut list_lines = Vec::with_capacity(self.lines.len() + 2);
            list_lines.push(String::new());
            list_lines.push("        |".cyan().to_string());
            for line in &self.lines {
                list_lines.push(Diagnostic::format_line(line));
            }
            list_lines.push("        |".cyan().to_string());
            list_lines.push(String::new());
            list_lines.join("\n")
        }
    }
}

impl std::fmt::Display for Diagnostic {
    /// Format the `Diagnostic` for display, including file, severity, message, and context.
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let str_first_line = match self.lines.iter().find(|line| line.line_number > 0) {
            Some(line) => format!(":{}", line.line_number),
            _ => String::new(),
        };
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
