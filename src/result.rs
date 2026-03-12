// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Display check result.

use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
    time::Duration,
};

use crate::diagnostic::{Diagnostic, Severity};
use crate::{args, rules::rule::Rules};
use crate::{checker::CheckFileResult, config::Config};

/// Display the settings used to check a file.
pub fn display_settings(path: &Path, config: &Config, rules: &Rules) {
    println!("Settings for file: {}", path.display());
    println!("  {config:?}");
    let rules_names = rules
        .enabled
        .iter()
        .map(|r| r.name())
        .collect::<Vec<&str>>()
        .join(", ");
    println!(
        "  Rules enabled: {}",
        if rules_names.is_empty() {
            "<none>"
        } else {
            &rules_names
        }
    );
}

/// Display diagnostics in human format.
fn display_diagnostics_human(result: &[CheckFileResult], args: &args::CheckArgs) {
    let mut diags: Vec<&Diagnostic> = result.iter().flat_map(|x| &x.diagnostics).collect();
    match args.sort {
        args::CheckSort::Line => {
            diags.sort_by_key(|diag| {
                (
                    diag.path.as_path(),
                    diag.lines
                        .iter()
                        .map(|l| l.line_number)
                        .collect::<Vec<usize>>(),
                )
            });
        }
        args::CheckSort::Message => {
            diags.sort_by_key(|diag| {
                (
                    diag.lines.first().map_or("", |line| &line.message),
                    diag.path.as_path(),
                    diag.lines
                        .iter()
                        .map(|l| l.line_number)
                        .collect::<Vec<usize>>(),
                )
            });
        }
        args::CheckSort::Rule => {
            diags.sort_by_key(|diag| {
                (
                    diag.rule,
                    diag.path.as_path(),
                    diag.lines
                        .iter()
                        .map(|l| l.line_number)
                        .collect::<Vec<usize>>(),
                )
            });
        }
    }
    for diag in diags {
        println!("{diag}");
    }
}

/// Display rule statistics.
fn display_rule_stats(result: &[CheckFileResult]) {
    let mut count_rule_errors = BTreeMap::<&str, usize>::new();
    for rule in result.iter().flat_map(|x| &x.diagnostics).map(|r| r.rule) {
        *count_rule_errors.entry(rule).or_insert(0) += 1;
    }
    let mut items: Vec<_> = count_rule_errors.iter().collect();
    items.sort_by(|a, b| b.1.cmp(a.1));
    println!("Errors by rule:");
    for (rule, count) in items {
        println!("  {rule}: {count}");
    }
}

/// Display file statistics.
fn display_file_stats(file_errors: &[(PathBuf, usize, usize, usize)]) {
    for (filename, info, warnings, errors) in file_errors {
        if errors + warnings + info == 0 {
            println!("{}: all OK!", filename.display());
        } else {
            println!(
                "{}: {} problems ({} errors, {} warnings, {} info)",
                filename.display(),
                errors + warnings + info,
                errors,
                warnings,
                info,
            );
        }
    }
}

/// Display diagnostics in JSON format.
fn display_diagnostics_json(result: &[CheckFileResult], _args: &args::CheckArgs) {
    let diags: Vec<&Diagnostic> = result.iter().flat_map(|x| &x.diagnostics).collect();
    println!("{}", serde_json::to_string(&diags).unwrap_or_default());
}

/// Display misspelled words.
fn display_misspelled_words(result: &[CheckFileResult], _args: &args::CheckArgs) {
    let hash_misspelled_words: HashSet<_> = result
        .iter()
        .flat_map(|x| &x.misspelled_words)
        .collect::<HashSet<_>>();
    let mut misspelled_words = hash_misspelled_words.iter().copied().collect::<Vec<_>>();
    misspelled_words.sort_unstable();
    for word in misspelled_words {
        println!("{word}");
    }
}

/// Display the result of the checks and return the appropriate exit code.
pub fn display_result(
    result: &[CheckFileResult],
    args: &args::CheckArgs,
    elapsed: &Duration,
) -> i32 {
    let mut files_checked = 0;
    let mut files_with_errors = 0;
    let mut count_info = 0;
    let mut count_warnings = 0;
    let mut count_errors = 0;
    let mut file_errors: Vec<(PathBuf, usize, usize, usize)> = Vec::new();
    for file in result {
        if args.show_settings && !args.quiet {
            display_settings(file.path.as_path(), &file.config, &file.rules);
        }
        let mut count_file_info = 0;
        let mut count_file_warnings = 0;
        let mut count_file_errors = 0;
        files_checked += 1;
        if !file.diagnostics.is_empty() {
            files_with_errors += 1;
            for diag in &file.diagnostics {
                match diag.severity {
                    Severity::Info => {
                        count_info += 1;
                        count_file_info += 1;
                    }
                    Severity::Warning => {
                        count_warnings += 1;
                        count_file_warnings += 1;
                    }
                    Severity::Error => {
                        count_errors += 1;
                        count_file_errors += 1;
                    }
                }
            }
        }
        if args.file_stats {
            file_errors.push((
                file.path.clone(),
                count_file_info,
                count_file_warnings,
                count_file_errors,
            ));
        }
    }
    if !args.quiet {
        match args.output {
            args::CheckOutputFormat::Human => {
                if !args.no_errors {
                    display_diagnostics_human(result, args);
                }
                if args.rule_stats {
                    display_rule_stats(result);
                }
                if args.file_stats {
                    file_errors.sort();
                    display_file_stats(&file_errors);
                }
            }
            args::CheckOutputFormat::Json => {
                if !args.no_errors {
                    display_diagnostics_json(result, args);
                }
            }
            args::CheckOutputFormat::Misspelled => {
                if !args.no_errors {
                    display_misspelled_words(result, args);
                }
            }
        }
    }
    if files_with_errors == 0 {
        if !args.quiet && args.output == args::CheckOutputFormat::Human {
            if files_checked > 0 {
                println!("{files_checked} files checked: all OK! [{elapsed:?}]");
            } else {
                println!("No files checked [{elapsed:?}]");
            }
        }
        0
    } else {
        if !args.quiet && args.output == args::CheckOutputFormat::Human {
            println!(
                "{files_checked} files checked: \
                {} problems \
                in {files_with_errors} files \
                ({count_errors} errors, \
                {count_warnings} warnings, \
                {count_info} info) \
                [{elapsed:?}]",
                count_errors + count_warnings + count_info
            );
        }
        if args.output == args::CheckOutputFormat::Misspelled {
            return 0;
        }
        1
    }
}
