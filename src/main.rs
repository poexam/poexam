// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Poexam is a blazingly fast PO file linter with a comprehensive diagnostic report.
//!
//! It reports very few false positives and can be used in CI jobs or pre-commit hooks.
//!
//! The following sub-commands are available:
//!
//! - [`check`](#check-files): check and fix files
//! - [`rules`](#rules): display rules used to check files
//! - [`stats`](#stats): display statistics about files
//! - [`lsp`](#lsp): run the language server for editor integration
//!
//! # Check files
//!
//! The `check` command checks all gettext files (*.po) given on command-line or found
//! in the provided directories.
//!
//! The .gitignore rules are respected: ignored files are skipped.
//!
//! The `check` command can also fix some issues in-place, and it can generate a SARIF report for CI jobs.
//!
//! # Rules
//!
//! The `rules` command displays the rules used to check files.
//!
//! Many rules are enabled by default, and some extra rules can be enabled on-demand.
//!
//! # Stats
//!
//! The `stats` command displays statistics about gettext files (*.po) and can compute
//! detailed statistics with the number of entries, words and characters.
//!
//! # LSP
//!
//! The `lsp` command runs a Language Server Protocol server over stdin/stdout, so editors
//! can show poexam diagnostics in real time while editing PO files.

mod args;
mod checker;
mod config;
mod diagnostic;
mod dict;
mod dir;
mod fix;
mod lsp;
mod po;
mod result;
mod rules;
mod sarif;
mod stats;
mod table;

use clap::Parser;

use crate::args::{Cli, Command};
use crate::checker::run_check;
use crate::lsp::run_lsp;
use crate::rules::rule::run_rules;
use crate::stats::run_stats;

fn main() {
    let args = Cli::parse();
    let rc = match &args.command {
        Command::Check(args) => run_check(args),
        Command::Rules(args) => run_rules(args),
        Command::Stats(args) => run_stats(args),
        Command::Lsp(args) => run_lsp(args),
    };
    std::process::exit(rc);
}
