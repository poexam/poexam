// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Checker for PO files.

use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::Read,
    ops::Range,
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
    fix::{Edit, FixTarget, apply_msgstr_fixes},
    po::{entry::Entry, escape::EscapePoExt, parser::Parser, writer::write_with_replacements},
    result::display_result,
    rules::rule::{Rule, Rules, get_selected_rules},
};

#[derive(Default)]
pub struct CheckFileResult {
    pub path: PathBuf,
    pub config: Config,
    pub rules: Rules,
    pub diagnostics: Vec<Diagnostic>,
    /// How many distinct msgstrs were rewritten when `--fix` ran on this file.
    /// Always 0 when `--fix` was not requested or when nothing needed fixing.
    pub fixes_applied: usize,
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

/// Build the replacement bytes for one msgstr block.
///
/// The original block is rewritten as a single `msgstr "..."` (or
/// `msgstr[N] "..."`) line — the keyword form is copied from the original
/// bytes so plural and obsolete-prefix variants are preserved. The pilot
/// always emits the new value on a single line; existing line wrapping is
/// not re-applied.
fn format_msgstr_block(original_block: &[u8], new_value: &str) -> Vec<u8> {
    let escaped = new_value.escape_po();
    let quote_pos = original_block
        .iter()
        .position(|&b| b == b'"')
        .unwrap_or(original_block.len());
    // Strip trailing spaces/tabs between the keyword and the opening quote.
    let mut head_end = quote_pos;
    while head_end > 0 && matches!(original_block[head_end - 1], b' ' | b'\t') {
        head_end -= 1;
    }
    let head = &original_block[..head_end];
    let mut out = Vec::with_capacity(head.len() + escaped.len() + 4);
    out.extend_from_slice(head);
    out.push(b' ');
    out.push(b'"');
    out.extend_from_slice(escaped.as_bytes());
    out.push(b'"');
    out.push(b'\n');
    out
}

/// Apply every fixable diagnostic to `data` and return the rewritten bytes
/// together with the count of distinct msgstrs that were actually rewritten,
/// or `None` if there is nothing to rewrite (no fixes, or every fix is in
/// conflict and skipped).
fn apply_fixes_to_data(data: &[u8], diagnostics: &[Diagnostic]) -> Option<(Vec<u8>, usize)> {
    // Group all msgstr edits by the file byte range they target.
    let mut edits_by_range: BTreeMap<(usize, usize), Vec<Edit>> = BTreeMap::new();
    for diag in diagnostics {
        let Some(fix) = &diag.fix else { continue };
        match &fix.target {
            FixTarget::Msgstr { file_byte_range } => {
                let key = (file_byte_range.start, file_byte_range.end);
                edits_by_range
                    .entry(key)
                    .or_default()
                    .extend(fix.edits.iter().cloned());
            }
        }
    }
    if edits_by_range.is_empty() {
        return None;
    }
    // Re-parse so we can look up each msgstr's decoded value by its byte range.
    let mut msgstr_values: HashMap<(usize, usize), String> = HashMap::new();
    for entry in Parser::new(data) {
        for msg in entry.msgstr.values() {
            msgstr_values.insert(
                (msg.byte_range.start, msg.byte_range.end),
                msg.value.clone(),
            );
        }
    }
    let mut replacements: Vec<(Range<usize>, Vec<u8>)> = Vec::new();
    for (key, edits) in edits_by_range {
        let Some(value) = msgstr_values.get(&key) else {
            continue;
        };
        let Ok(new_value) = apply_msgstr_fixes(value, &edits) else {
            continue;
        };
        if new_value == *value {
            continue;
        }
        let range = key.0..key.1;
        let original_block = &data[range.clone()];
        let bytes = format_msgstr_block(original_block, &new_value);
        replacements.push((range, bytes));
    }
    if replacements.is_empty() {
        return None;
    }
    let count = replacements.len();
    write_with_replacements(data, replacements)
        .ok()
        .map(|bytes| (bytes, count))
}

/// Rewrite the file on disk with the fixed bytes, then re-run the rules on the
/// new contents so the returned diagnostics reflect the post-fix state.
///
/// Returns a `CheckFileResult` carrying either the re-check result or a single
/// `fix-write-error` diagnostic if writing the file fails.
fn rewrite_and_recheck(
    path: &PathBuf,
    new_data: &[u8],
    fixes_applied: usize,
    config: Config,
    rules: Rules,
    existing_diagnostics: Vec<Diagnostic>,
) -> CheckFileResult {
    if let Err(err) = std::fs::write(path, new_data) {
        let mut diagnostics = existing_diagnostics;
        diagnostics.push(Diagnostic::new(
            path.as_path(),
            "fix-write-error",
            Severity::Error,
            err.to_string(),
        ));
        return CheckFileResult {
            path: path.clone(),
            config,
            rules,
            diagnostics,
            fixes_applied,
        };
    }
    let mut checker = Checker::new(new_data).with_path(path).with_config(config);
    checker.do_all_checks(&rules);
    CheckFileResult {
        path: path.clone(),
        config: checker.config,
        rules,
        diagnostics: checker.diagnostics,
        fixes_applied,
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
    if args.fix {
        if let Some((new_data, fixes_applied)) = apply_fixes_to_data(&data, &checker.diagnostics) {
            let config = std::mem::take(&mut checker.config);
            let diagnostics = std::mem::take(&mut checker.diagnostics);
            drop(checker);
            return rewrite_and_recheck(path, &new_data, fixes_applied, config, rules, diagnostics);
        }
    }
    CheckFileResult {
        path: path.clone(),
        config: checker.config,
        rules,
        diagnostics: checker.diagnostics,
        fixes_applied: 0,
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
            fix: false,
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

    /// PO content with one whitespace-end and one whitespace-start issue.
    const PO_WHITESPACE_ISSUES: &str = "msgid \"\"
msgstr \"\"
\"Language: fr\\n\"
\"Content-Type: text/plain; charset=UTF-8\\n\"

msgid \"hello \"
msgstr \"bonjour\"

msgid \" world\"
msgstr \"monde\"
";

    #[test]
    fn test_fix_rewrites_msgstr_blocks_in_place() {
        let tmp = tmp_dir("fix-rewrite");
        let po_path = write_po(tmp.path(), "fr.po", PO_WHITESPACE_ISSUES);

        let mut args = default_check_args();
        args.no_config = true;
        args.select = Some("whitespace-start,whitespace-end".to_string());
        args.fix = true;
        let result = check_file(&po_path, &args);

        // Re-checking the rewritten file must report zero whitespace diagnostics.
        let whitespace_diags = result
            .diagnostics
            .iter()
            .filter(|d| d.rule == "whitespace-start" || d.rule == "whitespace-end")
            .count();
        assert_eq!(
            whitespace_diags, 0,
            "expected no whitespace diagnostics after --fix, got {:?}",
            result.diagnostics
        );

        // The file on disk should now contain the mirrored whitespace.
        let fixed = std::fs::read_to_string(&po_path).expect("read fixed file");
        assert!(fixed.contains("msgstr \"bonjour \""));
        assert!(fixed.contains("msgstr \" monde\""));
    }

    #[test]
    fn test_fix_is_noop_when_no_fixable_diagnostics() {
        let tmp = tmp_dir("fix-noop");
        let po_path = write_po(tmp.path(), "fr.po", PO_PT_BR);
        let original = std::fs::read(&po_path).expect("read original");

        let mut args = default_check_args();
        args.no_config = true;
        args.select = Some("whitespace-start,whitespace-end".to_string());
        args.fix = true;
        let _ = check_file(&po_path, &args);

        let after = std::fs::read(&po_path).expect("read after");
        assert_eq!(
            after, original,
            "file must be byte-identical when there is nothing to fix"
        );
    }
}
