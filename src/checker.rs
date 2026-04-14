// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Checker for PO files.

use std::{
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
}

#[derive(Default)]
pub struct Checker<'d> {
    pub path: PathBuf,
    pub config: Config,
    pub dict_id: Option<Dictionary>,
    pub dict_str: Option<Dictionary>,
    pub diagnostics: Vec<Diagnostic>,
    parser: Parser<'d>,
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
    pub const fn nplurals(&self) -> u32 {
        self.parser.nplurals()
    }

    /// Check the PO entry using the given rule.
    ///
    /// This function calls the following functions defined in the rule that implements
    /// the trait [`RuleChecker`](crate::rules::rule::RuleChecker):
    /// - [`check_entry`](crate::rules::rule::RuleChecker::check_entry): check the global entry
    /// - [`check_ctxt`](crate::rules::rule::RuleChecker::check_ctxt): check the context string (msgctxt)
    /// - [`check_msg`](crate::rules::rule::RuleChecker::check_msg): check the strings:
    ///   - `msgid` / `msgstr[0]`
    ///   - `msgid_plural` / `msgstr[n]` (for each n > 0)
    pub fn check_entry(
        &self,
        entry: &Entry,
        rule: &Rule,
        untranslated_rule: bool,
    ) -> Vec<Diagnostic> {
        let mut diags = vec![];
        let rule_is_untranslated = rule.name() == "untranslated";
        diags.extend(rule.check_entry(self, entry));
        if let Some(msgctxt) = &entry.msgctxt {
            diags.extend(rule.check_ctxt(self, entry, msgctxt));
        }
        if let (Some(msgid), Some(msgstr_0)) = (&entry.msgid, entry.msgstr.get(&0))
            && (!msgstr_0.value.is_empty() || (untranslated_rule && rule_is_untranslated))
        {
            diags.extend(rule.check_msg(self, entry, msgid, msgstr_0));
        }
        if let Some(msgid_plural) = &entry.msgid_plural {
            for (_, msgstr_n) in entry.iter_strs().filter(|(k, _)| **k > 0) {
                if !msgstr_n.value.is_empty() || (untranslated_rule && rule_is_untranslated) {
                    diags.extend(rule.check_msg(self, entry, msgid_plural, msgstr_n));
                }
            }
        }
        diags
    }

    /// Perform all checks on every entry of the PO file.
    ///
    /// This function calls the following function defined in the rule that implements
    /// the trait [`RuleChecker`](crate::rules::rule::RuleChecker):
    /// - [`check_file`](crate::rules::rule::RuleChecker::check_file): check the entire file
    ///
    /// Then, for each entry, it calls the function [`check_entry`](crate::checker::Checker::check_entry)
    /// to check the entry with the given rule.
    pub fn do_all_checks(&mut self, rules: &Rules) {
        // Run rules for the entire file (e.g. check compilation of the file with msgfmt command).
        for rule in &rules.enabled {
            self.diagnostics.extend(rule.check_file(self));
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
                                self.diagnostics.push(Diagnostic::new(
                                    &self.path,
                                    "spelling-ctxt-id",
                                    Severity::Warning,
                                    err.to_string(),
                                ));
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
                                self.diagnostics.push(Diagnostic::new(
                                    &self.path,
                                    "spelling-str",
                                    Severity::Warning,
                                    err.to_string(),
                                ));
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
                self.diagnostics
                    .extend(self.check_entry(&entry, rule, rules.untranslated_rule));
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
