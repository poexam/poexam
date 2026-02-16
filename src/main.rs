// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Poexam is a blazingly fast PO file linter with a comprehensive diagnostic report.
//!
//! It reports very few false positives and can be used in CI jobs or pre-commit hooks.
//!
//! The following sub-commands are available:
//!
//! - [`check`](#check-files): check files
//! - [`rules`](#rules): display rules used to check files
//! - [`stats`](#stats): display statistics about files
//!
//! # Check files
//!
//! The `check` command checks all gettext files (*.po) given on command-line or found
//! in the provided directories.
//!
//! The .gitignore rules are respected: ignored files are skipped.
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

mod args;
mod checker;
mod diagnostic;
mod dict;
mod dir;
mod po;
mod result;
mod rules;
mod stats;

use clap::Parser;

use crate::args::{Cli, Command};
use crate::checker::run_check;
use crate::rules::rule::run_rules;
use crate::stats::run_stats;

fn main() {
    let args = Cli::parse();
    let rc = match &args.command {
        Command::Check(args) => run_check(args),
        Command::Rules(args) => run_rules(args),
        Command::Stats(args) => run_stats(args),
    };
    std::process::exit(rc);
}
