// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Diagnostic for PO files.

use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use clap::ValueEnum;
use colored::Colorize;
use serde::{
    Serialize,
    ser::{SerializeStruct, Serializer},
};

const HIGHLIGHT_COLOR: &str = "bright yellow";
const HIGHLIGHT_ON_COLOR: &str = "red";

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash, Serialize, ValueEnum,
)]
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
    pub fn new(path: &Path, rule: &'static str, severity: Severity, message: String) -> Self {
        Self {
            path: PathBuf::from(path),
            rule,
            severity,
            message,
            ..Default::default()
        }
    }

    pub fn add_message(&mut self, line: usize, message: &str, highlights: &[(usize, usize)]) {
        self.lines.push(DiagnosticLine {
            line_number: line,
            message: message.to_string(),
            highlights: highlights.to_vec(),
        });
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
        let str_first_line = match self.lines.first() {
            Some(line) => format!(":{}", line.line_number),
            None => String::new(),
        };
        write!(
            f,
            "{}{str_first_line}: [{}:{}] {}{}",
            self.path.display().to_string().white().bold(),
            self.severity,
            self.rule,
            self.message,
            self.format_lines(),
        )
    }
}
