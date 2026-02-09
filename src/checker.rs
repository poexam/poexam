// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::HashSet,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

use colored::Colorize;
use rayon::prelude::*;
use spellbook::Dictionary;

use crate::{
    args,
    diagnostic::{Diagnostic, Severity},
    dict::get_dict,
    dir::find_po_files,
    po::{entry::Entry, parser::Parser},
    rules::rule::{Rule, Rules, get_selected_rules},
};

type CheckResult = (PathBuf, Vec<Diagnostic>, HashSet<String>);

#[derive(Default)]
pub struct Checker<'d, 'r, 't> {
    pub path: PathBuf,
    pub dict_id: Option<&'t Dictionary>,
    pub dict_str: Option<Dictionary>,
    pub diagnostics: Vec<Diagnostic>,
    parser: Parser<'d>,
    rules: &'r Rules,
    check_fuzzy: bool,
    check_noqa: bool,
    check_obsolete: bool,
    path_dicts: PathBuf,
    path_words: Option<PathBuf>,
    misspelled_words: HashSet<String>,
    current_rule: &'static str,
    current_severity: Severity,
    current_line_ctxt: usize,
    current_line_id: usize,
    current_line_str: usize,
}

impl<'d, 'r, 't> Checker<'d, 'r, 't> {
    /// Create a new `Checker` for the given data and rules.
    pub fn new(data: &'d [u8], rules: &'r Rules) -> Self {
        Checker {
            parser: Parser::new(data),
            rules,
            ..Default::default()
        }
    }

    /// Set the path of the file being checked.
    pub fn with_path(mut self, path: &Path) -> Self {
        self.path = PathBuf::from(path);
        self
    }

    /// Set the dictionary for English language.
    pub fn with_dict_id(mut self, dict_id: Option<&'t Dictionary>) -> Self {
        self.dict_id = dict_id;
        self
    }

    /// Set the flag indicating the fuzzy entries are checked.
    pub fn with_check_fuzzy(mut self, check_fuzzy: bool) -> Self {
        self.check_fuzzy = check_fuzzy;
        self
    }

    /// Set the flag indicating the "noqa" entries are checked.
    pub fn with_check_noqa(mut self, check_noqa: bool) -> Self {
        self.check_noqa = check_noqa;
        self
    }

    /// Set the flag indicating the obsolete entries are checked.
    pub fn with_check_obsolete(mut self, check_obsolete: bool) -> Self {
        self.check_obsolete = check_obsolete;
        self
    }

    /// Set the path to the hunspell dictionaries.
    pub fn with_path_dicts(mut self, path_dicts: &Path) -> Self {
        self.path_dicts = PathBuf::from(path_dicts);
        self
    }

    /// Set the path to a directory containing files with list of words to add per language.
    pub fn with_path_words(mut self, path_words: Option<&PathBuf>) -> Self {
        self.path_words = path_words.cloned();
        self
    }

    pub fn add_misspelled_word(&mut self, word: &str) {
        self.misspelled_words.insert(word.to_string());
    }

    /// Get the language of the file being checked (e.g. `pt_BR`).
    pub fn language(&self) -> &str {
        &self.parser.language
    }

    /// Get the language code of the file being checked (e.g. `pt`).
    pub fn language_code(&self) -> &str {
        &self.parser.language_code
    }

    /// Get the country of the file being checked (e.g. `BR`).
    pub fn country(&self) -> &str {
        &self.parser.country
    }

    /// Return the encoding name.
    pub fn encoding_name(&self) -> &'static str {
        self.parser.encoding_name()
    }

    /// Return the number of plurals for the file being parsed.
    pub fn nplurals(&self) -> u32 {
        self.parser.nplurals()
    }

    /// Report a diagnostic for the given PO file.
    pub fn report_file(&mut self, rule: &'static str, severity: Severity, message: String) {
        self.diagnostics.push(Diagnostic::new(
            self.path.as_path(),
            rule,
            severity,
            message,
        ));
    }

    /// Report a diagnostic for the given PO entry.
    pub fn report_entry(&mut self, message: String, entry: &Entry) {
        let mut diagnostic = Diagnostic::new(
            self.path.as_path(),
            self.current_rule,
            self.current_severity,
            message,
        );
        for (line_no, line) in entry.to_po_lines() {
            diagnostic.add_message(line_no, &line, &[]);
        }
        self.diagnostics.push(diagnostic);
    }

    /// Report a diagnostic for a given context of a PO entry (msgctxt).
    pub fn report_ctxt(
        &mut self,
        _entry: &Entry,
        message: String,
        msgctxt: &str,
        hl_ctxt: &[(usize, usize)],
    ) {
        let mut diagnostic = Diagnostic::new(
            self.path.as_path(),
            self.current_rule,
            self.current_severity,
            message,
        );
        diagnostic.add_message(self.current_line_id, msgctxt, hl_ctxt);
        self.diagnostics.push(diagnostic);
    }

    /// Report a diagnostic for a given message of a PO entry (couple source/translated).
    pub fn report_msg(
        &mut self,
        _entry: &Entry,
        message: String,
        msgid: &str,
        hl_id: &[(usize, usize)],
        msgstr: &str,
        hl_str: &[(usize, usize)],
    ) {
        let mut diagnostic = Diagnostic::new(
            self.path.as_path(),
            self.current_rule,
            self.current_severity,
            message,
        );
        diagnostic.add_message(self.current_line_id, msgid, hl_id);
        diagnostic.add_message(0, "", &[]);
        diagnostic.add_message(self.current_line_str, msgstr, hl_str);
        self.diagnostics.push(diagnostic);
    }

    /// Check the PO entry using the given rule.
    ///
    /// This function calls the following functions defined in the rule that implements
    /// the trait [`RuleChecker`](crate::rules::rule::RuleChecker):
    /// - [`check_entry`](crate::rules::rule::RuleChecker::check_entry): check the global entry
    /// - [`check_msg`](crate::rules::rule::RuleChecker::check_msg): check the strings:
    ///   - `msgid` / `msgstr[0]`
    ///   - `msgid_plural` / `msgstr[n]` (for each n > 0)
    pub fn check_entry(&mut self, entry: &Entry, rule: &Rule) {
        self.current_rule = rule.name();
        self.current_severity = rule.severity();
        let rule_is_untranslated = self.current_rule == "untranslated";
        rule.check_entry(self, entry);
        if let Some(msgctxt) = &entry.msgctxt {
            self.current_line_ctxt = msgctxt.line_number;
            rule.check_ctxt(self, entry, &msgctxt.value);
        }
        if let (Some(msgid), Some(msgstr_0)) = (&entry.msgid, entry.msgstr.get(&0))
            && (!msgstr_0.value.is_empty()
                || (self.rules.untranslated_rule && rule_is_untranslated))
        {
            self.current_line_id = msgid.line_number;
            self.current_line_str = msgstr_0.line_number;
            rule.check_msg(self, entry, &msgid.value, &msgstr_0.value);
        }
        if let Some(msgid_plural) = &entry.msgid_plural {
            for (_, msgstr_n) in entry.iter_strs().filter(|(k, _)| **k > 0) {
                if !msgstr_n.value.is_empty()
                    || (self.rules.untranslated_rule && rule_is_untranslated)
                {
                    self.current_line_id = msgid_plural.line_number;
                    self.current_line_str = msgstr_n.line_number;
                    rule.check_msg(self, entry, &msgid_plural.value, &msgstr_n.value);
                }
            }
        }
    }

    /// Perform all checks on every entry of the PO file.
    pub fn do_all_checks(&mut self) {
        let mut error_dict_str = false;
        while let Some(entry) = self.parser.next() {
            if entry.is_header() {
                if self.rules.spelling_str_rule && self.dict_str.is_none() {
                    self.dict_str = match get_dict(
                        self.path_dicts.as_path(),
                        self.path_words.as_ref(),
                        &self.parser.language,
                    ) {
                        Ok(dict) => Some(dict),
                        Err(err) => {
                            if !error_dict_str {
                                self.report_file(
                                    "spelling-str",
                                    Severity::Warning,
                                    err.to_string(),
                                );
                            }
                            error_dict_str = true;
                            None
                        }
                    };
                }
                continue;
            }
            if (!entry.is_translated() && !self.rules.untranslated_rule)
                || (entry.fuzzy && !self.check_fuzzy && !self.rules.fuzzy_rule)
                || (entry.noqa && !self.check_noqa)
                || (entry.obsolete && !self.check_obsolete && !self.rules.obsolete_rule)
            {
                continue;
            }
            for rule in &self.rules.enabled {
                if !entry.noqa_rules.is_empty()
                    && entry.noqa_rules.contains(&rule.name().to_string())
                {
                    continue;
                }
                self.check_entry(&entry, rule);
            }
        }
    }
}

/// Check a single PO file and return the list of diagnostics found.
pub fn check_file(
    path: &PathBuf,
    args: &args::CheckArgs,
    rules: &Rules,
    dict_id: Option<&Dictionary>,
) -> CheckResult {
    let Ok(mut file) = File::open(path) else {
        return (
            PathBuf::from(path.as_path()),
            vec![Diagnostic::new(
                path.as_path(),
                "read-error",
                Severity::Error,
                "could not open file".to_string(),
            )],
            HashSet::new(),
        );
    };
    let mut buf = Vec::new();
    let Ok(_) = file.read_to_end(&mut buf) else {
        return (
            PathBuf::from(path.as_path()),
            vec![Diagnostic::new(
                path.as_path(),
                "read-error",
                Severity::Error,
                "could not read file".to_string(),
            )],
            HashSet::new(),
        );
    };
    let mut checker = Checker::new(&buf, rules)
        .with_path(path)
        .with_dict_id(dict_id)
        .with_check_fuzzy(args.fuzzy)
        .with_check_noqa(args.noqa)
        .with_check_obsolete(args.obsolete)
        .with_path_dicts(&args.path_dicts)
        .with_path_words(args.path_words.as_ref());
    checker.do_all_checks();
    (
        PathBuf::from(path.as_path()),
        checker.diagnostics,
        checker.misspelled_words,
    )
}

/// Display the settings used to check files.
fn display_settings(args: &args::CheckArgs, rules: &Rules) {
    if args.quiet || !args.show_settings {
        return;
    }
    println!("Configuration:");
    let rules_names = rules
        .enabled
        .iter()
        .map(|r| r.name())
        .collect::<Vec<&str>>()
        .join(", ");
    println!(
        "  Rules enabled: {}",
        if rules_names.is_empty() {
            "<none>".to_string()
        } else {
            rules_names
        }
    );
    println!(
        "  Check fuzzy entries: {}",
        if rules.fuzzy_rule || args.fuzzy {
            "yes"
        } else {
            "no"
        }
    );
    println!(
        "  Check noqa entries: {}",
        if args.noqa { "yes" } else { "no" }
    );
    println!(
        "  Check obsolete entries: {}",
        if rules.obsolete_rule || args.obsolete {
            "yes"
        } else {
            "no"
        }
    );
    println!("  Output format: {}", args.output);
}

/// Display diagnostics in human format.
fn display_diagnostics_human(result: &[CheckResult], args: &args::CheckArgs) {
    let mut diags: Vec<&Diagnostic> = result.iter().flat_map(|x| &x.1).collect();
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

/// Display diagnostics in JSON format.
fn display_diagnostics_json(result: &[CheckResult], _args: &args::CheckArgs) {
    let diags: Vec<&Diagnostic> = result.iter().flat_map(|x| &x.1).collect();
    println!("{}", serde_json::to_string(&diags).unwrap_or_default());
}

/// Display misspelled words.
fn display_misspelled_words(result: &[CheckResult], _args: &args::CheckArgs) {
    let hash_misspelled_words: HashSet<_> =
        result.iter().flat_map(|x| &x.2).collect::<HashSet<_>>();
    let mut misspelled_words = hash_misspelled_words.iter().copied().collect::<Vec<_>>();
    misspelled_words.sort_unstable();
    for word in misspelled_words {
        println!("{word}");
    }
}

/// Display the result of the checks and return the appropriate exit code.
fn display_result(result: &[CheckResult], args: &args::CheckArgs, elapsed: &Duration) -> i32 {
    let mut files_checked = 0;
    let mut files_with_errors = 0;
    let mut count_info = 0;
    let mut count_warnings = 0;
    let mut count_errors = 0;
    let mut file_errors: Vec<(PathBuf, usize, usize, usize)> = Vec::new();
    for (filename, errors, _) in result {
        let mut count_file_info = 0;
        let mut count_file_warnings = 0;
        let mut count_file_errors = 0;
        files_checked += 1;
        if !errors.is_empty() {
            files_with_errors += 1;
            for error in errors {
                match error.severity {
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
        if args.file_status {
            file_errors.push((
                filename.clone(),
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
                if args.file_status {
                    file_errors.sort();
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
        1
    }
}

/// Check and display result for all PO files.
pub fn run_check(args: &args::CheckArgs) -> i32 {
    let start = std::time::Instant::now();
    let rules = match get_selected_rules(args) {
        Ok(selected_rules) => selected_rules,
        Err(err) => {
            eprintln!("{}: {err}", "Error".bright_red().bold());
            return 1;
        }
    };
    display_settings(args, &rules);
    let po_files = find_po_files(&args.files);
    let dict_id = if rules.spelling_ctxt_rule || rules.spelling_id_rule {
        match get_dict(
            args.path_dicts.as_path(),
            args.path_words.as_ref(),
            &args.lang_id,
        ) {
            Ok(dict) => Some(dict),
            Err(err) => {
                eprintln!("{}: {err}", "Warning".yellow());
                None
            }
        }
    } else {
        None
    };
    let result: Vec<CheckResult> = po_files
        .par_iter()
        .map(|f| check_file(f, args, &rules, dict_id.as_ref()))
        .collect();
    let elapsed = start.elapsed();
    display_result(&result, args, &elapsed)
}
