// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Checker for PO files.

use std::{
    collections::HashSet,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use rayon::prelude::*;
use spellbook::Dictionary;

use crate::{
    args,
    config::{Config, find_config_path},
    diagnostic::{Diagnostic, Severity},
    dict,
    dir::find_po_files,
    po::{entry::Entry, parser::Parser},
    result::display_result,
    rules::rule::{Rule, Rules, get_selected_rules},
};

#[derive(Default)]
pub struct CheckFileResult {
    pub path: PathBuf,
    pub config: Config,
    pub rules: Rules,
    pub diagnostics: Vec<Diagnostic>,
    pub misspelled_words: HashSet<String>,
}

#[derive(Default)]
pub struct Checker<'d> {
    pub path: PathBuf,
    pub config: Config,
    pub dict_id: Option<Dictionary>,
    pub dict_str: Option<Dictionary>,
    pub diagnostics: Vec<Diagnostic>,
    parser: Parser<'d>,
    misspelled_words: HashSet<String>,
    current_rule: &'static str,
    current_severity: Severity,
    current_line_ctxt: usize,
    current_line_id: usize,
    current_line_str: usize,
}

impl<'d> Checker<'d> {
    /// Create a new `Checker` for the given data and rules.
    pub fn new(data: &'d [u8]) -> Self {
        Checker {
            parser: Parser::new(data),
            ..Default::default()
        }
    }

    /// Set the path of the file being checked.
    pub fn with_path(mut self, path: &Path) -> Self {
        self.path = PathBuf::from(path);
        self
    }

    /// Set the path of the file being checked.
    pub fn with_config(mut self, config: Config) -> Self {
        self.config = config;
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
    pub fn report_file(
        &mut self,
        rule: &'static str,
        severity: Severity,
        message: String,
        detail: Option<String>,
    ) {
        let mut diagnostic = Diagnostic::new(self.path.as_path(), rule, severity, message);
        if let Some(content) = detail {
            // Split lines in detail and add them to the diagnostic with no line number (0).
            for line in content.lines() {
                diagnostic.add_message(0, line, &[]);
            }
        }
        self.diagnostics.push(diagnostic);
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
    pub fn report_id_str(
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
    pub fn check_entry(&mut self, entry: &Entry, rule: &Rule, untranslated_rule: bool) {
        self.current_rule = rule.name();
        self.current_severity = rule.severity();
        let rule_is_untranslated = self.current_rule == "untranslated";
        rule.check_entry(self, entry);
        if let Some(msgctxt) = &entry.msgctxt {
            self.current_line_ctxt = msgctxt.line_number;
            rule.check_ctxt(self, entry, &msgctxt.value);
        }
        if let (Some(msgid), Some(msgstr_0)) = (&entry.msgid, entry.msgstr.get(&0))
            && (!msgstr_0.value.is_empty() || (untranslated_rule && rule_is_untranslated))
        {
            self.current_line_id = msgid.line_number;
            self.current_line_str = msgstr_0.line_number;
            rule.check_msg(self, entry, &msgid.value, &msgstr_0.value);
        }
        if let Some(msgid_plural) = &entry.msgid_plural {
            for (_, msgstr_n) in entry.iter_strs().filter(|(k, _)| **k > 0) {
                if !msgstr_n.value.is_empty() || (untranslated_rule && rule_is_untranslated) {
                    self.current_line_id = msgid_plural.line_number;
                    self.current_line_str = msgstr_n.line_number;
                    rule.check_msg(self, entry, &msgid_plural.value, &msgstr_n.value);
                }
            }
        }
    }

    /// Perform all checks on every entry of the PO file.
    pub fn do_all_checks(&mut self, rules: &Rules) {
        // Run rules for the entire file (e.g. check compilation of the file with msgfmt command).
        for rule in &rules.enabled {
            rule.check_file(self);
        }
        let mut error_dict_id = false;
        let mut error_dict_str = false;
        while let Some(entry) = self.parser.next() {
            if entry.is_header() {
                if (rules.spelling_ctxt_rule || rules.spelling_id_rule)
                    && (self.config.check.langs.is_empty()
                        || self.config.check.langs.contains(&self.config.check.lang_id))
                {
                    self.dict_id = match dict::get_dict(
                        self.config.check.path_dicts.as_path(),
                        self.config.check.path_words.as_ref(),
                        &self.config.check.lang_id,
                    ) {
                        Ok(dict) => Some(dict),
                        Err(err) => {
                            if !error_dict_id {
                                self.report_file(
                                    "spelling-id-ctxt",
                                    Severity::Warning,
                                    err.to_string(),
                                    None,
                                );
                            }
                            error_dict_id = true;
                            None
                        }
                    }
                }
                if (rules.spelling_str_rule && self.dict_str.is_none())
                    && (self.config.check.langs.is_empty()
                        || self.config.check.langs.contains(&self.parser.language))
                {
                    self.dict_str = match dict::get_dict(
                        self.config.check.path_dicts.as_path(),
                        self.config.check.path_words.as_ref(),
                        &self.parser.language,
                    ) {
                        Ok(dict) => Some(dict),
                        Err(err) => {
                            if !error_dict_str {
                                self.report_file(
                                    "spelling-str",
                                    Severity::Warning,
                                    err.to_string(),
                                    None,
                                );
                            }
                            error_dict_str = true;
                            None
                        }
                    };
                }
                continue;
            }
            if (!entry.is_translated() && !rules.untranslated_rule)
                || (entry.fuzzy && !self.config.check.fuzzy && !rules.fuzzy_rule)
                || (entry.noqa && !self.config.check.noqa && !rules.noqa_rule)
                || (entry.obsolete && !self.config.check.obsolete && !rules.obsolete_rule)
            {
                continue;
            }
            for rule in &rules.enabled {
                if rule.name() != "noqa"
                    && (entry.noqa || entry.noqa_rules.iter().any(|r| r == rule.name()))
                {
                    continue;
                }
                self.check_entry(&entry, rule, rules.untranslated_rule);
            }
        }
    }
}

/// Check a single PO file and return the list of diagnostics found.
pub fn check_file(path: &PathBuf, args: &args::CheckArgs) -> CheckFileResult {
    let path_config = if args.no_config {
        None
    } else {
        match args.config.as_ref() {
            Some(path) => match path.canonicalize() {
                Ok(abs_path) => Some(abs_path),
                Err(_) => Some(PathBuf::from(path)),
            },
            None => find_config_path(path),
        }
    };
    let config = match Config::new(path_config.as_ref()) {
        Ok(cfg) => cfg.with_args_check(args),
        Err(err) => {
            return CheckFileResult {
                path: path.clone(),
                diagnostics: vec![Diagnostic::new(
                    path.as_path(),
                    "config-error",
                    Severity::Error,
                    format!(
                        "invalid config file (path: {}): {err}",
                        path_config.unwrap_or_default().display()
                    ),
                )],
                ..Default::default()
            };
        }
    };
    let rules = match get_selected_rules(&config) {
        Ok(selected_rules) => selected_rules,
        Err(err) => {
            return CheckFileResult {
                path: path.clone(),
                diagnostics: vec![Diagnostic::new(
                    path.as_path(),
                    "rules-error",
                    Severity::Error,
                    err.to_string(),
                )],
                ..Default::default()
            };
        }
    };
    let mut data: Vec<u8> = Vec::new();
    match File::open(path) {
        Ok(mut file) => {
            if let Err(err) = file.read_to_end(&mut data) {
                return CheckFileResult {
                    path: path.clone(),
                    diagnostics: vec![Diagnostic::new(
                        path.as_path(),
                        "read-error",
                        Severity::Error,
                        err.to_string(),
                    )],
                    ..Default::default()
                };
            }
        }
        Err(err) => {
            return CheckFileResult {
                path: path.clone(),
                diagnostics: vec![Diagnostic::new(
                    path.as_path(),
                    "read-error",
                    Severity::Error,
                    err.to_string(),
                )],
                ..Default::default()
            };
        }
    }
    let mut checker = Checker::new(&data).with_path(path).with_config(config);
    checker.do_all_checks(&rules);
    CheckFileResult {
        path: path.clone(),
        config: checker.config,
        rules,
        diagnostics: checker.diagnostics,
        misspelled_words: checker.misspelled_words,
    }
}

/// Check and display result for all PO files.
pub fn run_check(args: &args::CheckArgs) -> i32 {
    let start = std::time::Instant::now();
    let result: Vec<CheckFileResult> = find_po_files(&args.files)
        .par_iter()
        .map(|path| check_file(path, args))
        .collect();
    let elapsed = start.elapsed();
    display_result(&result, args, &elapsed)
}
