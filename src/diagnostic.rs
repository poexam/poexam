// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

use clap::ValueEnum;
use colored::Colorize;
use serde::{Deserialize, Serialize};

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
pub enum Severity {
    #[default]
    Info,
    Warning,
    Error,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DiagnosticLine {
    pub line_number: usize,
    pub message: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Diagnostic {
    pub path: PathBuf,
    pub rule: &'static str,
    pub severity: Severity,
    pub message: String,
    pub msgid_raw: String,
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

impl Diagnostic {
    /// Create a new `Diagnostic` with the given path, severity, and message.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        path: &Path,
        rule: &'static str,
        severity: Severity,
        message: String,
        msgid_raw: String,
    ) -> Self {
        Self {
            path: PathBuf::from(path),
            rule,
            severity,
            message,
            msgid_raw,
            ..Default::default()
        }
    }

    pub fn add_message(&mut self, line: usize, message: String) {
        self.lines.push(DiagnosticLine {
            line_number: line,
            message,
        });
    }

    /// Format the diagnostic line (number + message) with colors for display.
    fn format_line(line: &DiagnosticLine) -> String {
        let prefix_lf_empty = "        | ".bright_blue().bold().to_string();
        let prefix_line = if line.line_number > 0 {
            format!("{:7} | ", line.line_number)
                .bright_blue()
                .bold()
                .to_string()
        } else {
            prefix_lf_empty.clone()
        };
        if line.message.is_empty() {
            return prefix_line;
        }
        let mut out = String::new();
        for (idx, line) in line.message.lines().enumerate() {
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
}

impl std::fmt::Display for Diagnostic {
    /// Format the `Diagnostic` for display, including file, severity, message, and context.
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let str_first_line = match self.lines.first() {
            Some(line) => format!(":{}", line.line_number),
            None => String::new(),
        };
        let mut list_lines = Vec::with_capacity(self.lines.len());
        for line in &self.lines {
            list_lines.push(Diagnostic::format_line(line));
        }
        write!(
            f,
            "{}{str_first_line}: [{}:{}] {}\n{}\n{}\n{}\n",
            self.path.display().to_string().white().bold(),
            self.severity,
            self.rule,
            self.message,
            "        |".bright_blue().bold(),
            list_lines.join("\n"),
            "        |".bright_blue().bold(),
        )
    }
}
