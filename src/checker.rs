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
        self.parser.language()
    }

    /// Get the language code of the file being checked (e.g. `pt`).
    pub fn language_code(&self) -> &str {
        self.parser.language_code()
    }

    /// Get the country of the file being checked (e.g. `BR`).
    pub fn country(&self) -> &str {
        self.parser.country()
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
    fn check_entry(&self, entry: &Entry, rule: &Rule, untranslated_rule: bool) -> Vec<Diagnostic> {
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
            for (_, msgstr_n) in entry.iter_plural_strs() {
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
    pub(crate) fn do_all_checks(&mut self, rules: &Rules) {
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
                let language = self.parser.language();
                if (rules.spelling_str_rule && self.dict_str.is_none())
                    && (self.config.check.langs.is_empty()
                        || self.config.check.langs.iter().any(|s| s == language))
                {
                    self.dict_str = match dict::get_dict(
                        self.config.check.path_dicts.as_path(),
                        self.config.check.path_words.as_ref(),
                        language,
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
                if let Some(msgstr_0) = entry.msgstr.get(&0) {
                    for rule in &rules.enabled {
                        if rule.name() != "noqa"
                            && (entry.noqa || entry.noqa_rules.iter().any(|r| r == rule.name()))
                        {
                            continue;
                        }
                        self.diagnostics
                            .extend(rule.check_header(self, &entry, msgstr_0));
                    }
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
fn check_file(path: &PathBuf, args: &args::CheckArgs) -> CheckFileResult {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(label: &str) -> tempfile::TempDir {
        tempfile::TempDir::with_prefix(format!("poexam-checker-{label}-")).expect("create temp dir")
    }

    fn default_check_args() -> args::CheckArgs {
        args::CheckArgs {
            files: vec![],
            show_settings: false,
            config: None,
            no_config: false,
            fuzzy: false,
            noqa: false,
            obsolete: false,
            select: None,
            ignore: None,
            path_msgfmt: None,
            path_dicts: None,
            path_words: None,
            lang_id: None,
            langs: None,
            short_factor: None,
            long_factor: None,
            severity: vec![],
            punc_ignore_ellipsis: false,
            no_errors: false,
            sort: args::CheckSort::default(),
            rule_stats: false,
            file_stats: false,
            output: args::CheckOutputFormat::default(),
            quiet: true,
        }
    }

    /// Minimal valid PO content with a `pt_BR` header and one translated entry.
    const PO_PT_BR: &str = "msgid \"\"
msgstr \"\"
\"Language: pt_BR\\n\"
\"Content-Type: text/plain; charset=UTF-8\\n\"

msgid \"hello\"
msgstr \"olá\"
";

    fn write_po(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, content).expect("write po file");
        path
    }

    #[test]
    fn test_new_returns_default_path_and_no_diagnostics() {
        let checker = Checker::new(b"");
        assert_eq!(checker.path, PathBuf::new());
        assert!(checker.diagnostics.is_empty());
        assert!(checker.dict_id.is_none());
        assert!(checker.dict_str.is_none());
    }

    #[test]
    fn test_with_path_sets_path() {
        let checker = Checker::new(b"").with_path(Path::new("foo/bar.po"));
        assert_eq!(checker.path, PathBuf::from("foo/bar.po"));
    }

    #[test]
    fn test_with_config_sets_config() {
        let mut config = Config::default();
        config.check.lang_id = "fr".to_string();
        let checker = Checker::new(b"").with_config(config);
        assert_eq!(checker.config.check.lang_id, "fr");
    }

    #[test]
    fn test_unparsed_state_has_default_metadata() {
        let checker = Checker::new(b"");
        assert_eq!(checker.language(), "");
        assert_eq!(checker.language_code(), "");
        assert_eq!(checker.country(), "");
        // No `Content-Type` header parsed yet → encoding defaults to UTF-8.
        assert_eq!(checker.encoding_name(), "UTF-8");
        assert_eq!(checker.nplurals(), 0);
    }

    #[test]
    fn test_language_extracted_from_header_after_parsing() {
        let mut checker = Checker::new(PO_PT_BR.as_bytes());
        // Empty rule set: parser walks all entries, populates header metadata,
        // and produces no diagnostics.
        checker.do_all_checks(&Rules::default());
        assert_eq!(checker.language(), "pt_BR");
        assert_eq!(checker.language_code(), "pt");
        assert_eq!(checker.country(), "BR");
        assert_eq!(checker.encoding_name(), "UTF-8");
        assert!(checker.diagnostics.is_empty());
    }

    #[test]
    fn test_do_all_checks_on_empty_input_does_nothing() {
        let mut checker = Checker::new(b"");
        checker.do_all_checks(&Rules::default());
        assert!(checker.diagnostics.is_empty());
    }

    #[test]
    fn test_check_file_missing_path_returns_read_error() {
        let missing = PathBuf::from("/this/path/should/not/exist/file.po");
        let mut args = default_check_args();
        args.no_config = true;
        let result = check_file(&missing, &args);
        assert_eq!(result.path, missing);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].rule, "read-error");
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
    }

    #[test]
    fn test_check_file_invalid_config_returns_config_error() {
        let tmp = tmp_dir("bad-config");
        let cfg_path = tmp.path().join("poexam.toml");
        std::fs::write(&cfg_path, "this = is = not toml").expect("write bad config");
        let po_path = write_po(tmp.path(), "fr.po", PO_PT_BR);

        let mut args = default_check_args();
        args.config = Some(cfg_path);
        let result = check_file(&po_path, &args);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].rule, "config-error");
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
    }

    #[test]
    fn test_check_file_invalid_rule_selector_returns_rules_error() {
        let tmp = tmp_dir("bad-rule");
        let po_path = write_po(tmp.path(), "fr.po", PO_PT_BR);

        let mut args = default_check_args();
        args.no_config = true;
        args.select = Some("does-not-exist-rule".to_string());
        let result = check_file(&po_path, &args);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].rule, "rules-error");
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
    }

    #[test]
    fn test_check_file_no_config_runs_with_default_rules() {
        let tmp = tmp_dir("no-config");
        let po_path = write_po(tmp.path(), "fr.po", PO_PT_BR);

        let mut args = default_check_args();
        args.no_config = true;
        // Pick a non-default rule that won't fire on a non-fuzzy, non-obsolete entry.
        args.select = Some("fuzzy".to_string());
        let result = check_file(&po_path, &args);
        assert_eq!(result.path, po_path);
        assert!(
            result.diagnostics.is_empty(),
            "expected no diagnostics, got {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn test_check_file_uses_args_config_when_provided() {
        // A `--config` path that doesn't exist must surface as a config error
        // (canonicalize fails → raw path passed to Config::new → read fails).
        let tmp = tmp_dir("missing-config");
        let po_path = write_po(tmp.path(), "fr.po", PO_PT_BR);
        let mut args = default_check_args();
        args.config = Some(PathBuf::from("/no/such/poexam.toml"));
        let result = check_file(&po_path, &args);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].rule, "config-error");
    }

    #[test]
    fn test_run_check_clean_file_returns_zero() {
        let tmp = tmp_dir("run-clean");
        let po_path = write_po(tmp.path(), "fr.po", PO_PT_BR);

        let mut args = default_check_args();
        args.no_config = true;
        args.select = Some("fuzzy".to_string());
        args.files = vec![po_path];
        let code = run_check(&args);
        assert_eq!(code, 0);
    }

    #[test]
    fn test_run_check_invalid_rule_returns_one() {
        let tmp = tmp_dir("run-bad-rule");
        let po_path = write_po(tmp.path(), "fr.po", PO_PT_BR);

        let mut args = default_check_args();
        args.no_config = true;
        args.select = Some("does-not-exist-rule".to_string());
        args.files = vec![po_path];
        let code = run_check(&args);
        assert_eq!(code, 1);
    }
}
