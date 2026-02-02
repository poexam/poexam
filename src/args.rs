// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::diagnostic::Severity;

#[derive(Debug, Parser)]
#[command(
    author,
    name = "poexam",
    about = "Blazingly fast PO linter.",
    after_help = "For help with a specific command, see: `poexam help <command>`."
)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Check files
    Check(CheckArgs),

    /// Display rules used to check files
    Rules(RulesArgs),

    /// Display statistics about files
    Stats(StatsArgs),
}

/// Arguments for the `check` command.
#[derive(Debug, Args)]
#[allow(clippy::struct_excessive_bools)]
pub struct CheckArgs {
    /// List of files or directories
    #[clap(help = "List of files or directories [default: .]")]
    pub files: Vec<PathBuf>,

    /// Display settings used to check files
    #[arg(long)]
    pub show_settings: bool,

    /// Check fuzzy entries (not checked by default)
    #[arg(long)]
    pub fuzzy: bool,

    /// Check entries marked as "noqa" (not checked by default)
    #[arg(long)]
    pub noqa: bool,

    /// Check obsolete entries (not checked by default)
    #[arg(long)]
    pub obsolete: bool,

    /// Select rules to apply (comma-separated list), the special rule "all" can be used to select all rules
    #[arg(short, long)]
    pub select: Option<String>,

    /// Ignore rules (comma-separated list)
    #[arg(short, long)]
    pub ignore: Option<String>,

    /// Perform only checks with this severity (can be given multiple times); by default all checks are performed
    #[arg(short = 'e', long, value_enum)]
    pub severity: Vec<Severity>,

    /// Do not display errors found
    #[arg(short, long)]
    pub no_errors: bool,

    /// Sort of errors displayed
    #[arg(long, value_enum, default_value_t)]
    pub sort: CheckSort,

    /// Display status for each file checked (used only with `human` output format)
    #[arg(short, long)]
    pub file_status: bool,

    /// Output format
    #[arg(short, long, value_enum, default_value_t)]
    pub output: OutputFormat,

    /// Quiet mode: do not report any error, only set the exit code
    #[arg(short, long)]
    pub quiet: bool,
}

/// Sort of errors.
#[derive(Clone, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum CheckSort {
    #[default]
    /// Sort by path, line number
    Line,

    /// Sort by message id, path, line number
    Message,

    /// Sort by error type (rule), path, line number
    Rule,
}

/// Arguments for the `rules` command.
#[derive(Debug, Args)]
pub struct RulesArgs {}

/// Arguments for the `stats` command.
#[derive(Debug, Args)]
pub struct StatsArgs {
    /// List of files or directories (default: .)
    pub files: Vec<PathBuf>,

    /// Output format
    #[arg(short, long, value_enum, default_value_t)]
    pub output: OutputFormat,

    /// Sort files displayed
    #[arg(short, long, value_enum, default_value_t)]
    pub sort: StatsSort,

    /// Display extra statistics on words and characters
    #[arg(short, long)]
    pub words: bool,
}

/// Output format.
#[derive(Clone, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    #[default]
    /// Human readable text format
    Human,

    /// JSON
    Json,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            OutputFormat::Human => write!(f, "human"),
            OutputFormat::Json => write!(f, "json"),
        }
    }
}

/// Sort in stats output.
#[derive(Clone, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum StatsSort {
    #[default]
    /// Sort by path
    Path,

    /// Sort by status (high % translated first), then by path
    Status,
}
