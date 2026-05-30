// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Checker for PO files.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs::File,
    io::Read,
    ops::Range,
    path::{Path, PathBuf},
};

use rayon::prelude::*;
use spellbook::Dictionary;

use crate::{
    args,
    config::{self, Config, find_config_path},
    diagnostic::{Diagnostic, Severity},
    dict,
    dir::find_po_files,
    fix::{Edit, FixTarget, apply_msgstr_fixes},
    po::{
        entry::Entry, parser::Parser, wrap::format_msgstr_block, writer::write_with_replacements,
    },
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
    /// Lowercase words loaded from `check.force_trans_file` (one per line).
    /// Used by the `force-trans` rule.
    pub force_trans_words: Option<HashSet<String>>,
    /// Lowercase words loaded from `check.no_trans_file` (one per line).
    /// Used by the `no-trans` rule.
    pub no_trans_words: Option<HashSet<String>>,
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

    /// Load the word list for a `force-trans` / `no-trans` rule via
    /// [`config::load_word_list`], or emit a warning diagnostic when the file
    /// can not be read (mirrors the behavior of the spelling rules when a
    /// dictionary is missing).
    fn load_rule_word_list(
        &mut self,
        rule_name: &'static str,
        path: Option<PathBuf>,
    ) -> Option<HashSet<String>> {
        let path = path?;
        match config::load_word_list(&path) {
            Ok(words) => Some(words),
            Err(err) => {
                self.diagnostics.push(Diagnostic::new(
                    &self.path,
                    rule_name,
                    Severity::Warning,
                    format!(
                        "words file not found for rule '{rule_name}' (path: {}): {err}, {rule_name} rule ignored",
                        path.display()
                    ),
                ));
                None
            }
        }
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
        // Load word lists for `force-trans` / `no-trans` rules if enabled. These
        // lists are independent of the PO file's header, so we load them up
        // front and surface any file-read error as a single diagnostic.
        if rules.force_trans_rule {
            self.force_trans_words =
                self.load_rule_word_list("force-trans", self.config.check.force_trans_file.clone());
        }
        if rules.no_trans_rule {
            self.no_trans_words =
                self.load_rule_word_list("no-trans", self.config.check.no_trans_file.clone());
        }
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

/// Apply every fixable diagnostic to `data` and return the rewritten bytes
/// together with the count of distinct fixes that were actually applied
/// (msgstrs rewritten + entries deleted), or `None` if there is nothing to
/// rewrite (no fixes, or every fix is in conflict and skipped).
fn apply_fixes_to_data(
    data: &[u8],
    diagnostics: &[Diagnostic],
    page_width: usize,
) -> Option<(Vec<u8>, usize)> {
    // Bucket fixes by target kind.
    let mut edits_by_range: BTreeMap<(usize, usize), Vec<Edit>> = BTreeMap::new();
    let mut entry_deletions: BTreeSet<(usize, usize)> = BTreeSet::new();
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
            FixTarget::Entry { file_byte_range } => {
                entry_deletions.insert((file_byte_range.start, file_byte_range.end));
            }
        }
    }
    if edits_by_range.is_empty() && entry_deletions.is_empty() {
        return None;
    }
    let mut replacements: Vec<(Range<usize>, Vec<u8>)> = Vec::new();
    // Entry deletions: splice the whole range out.
    for (start, end) in &entry_deletions {
        replacements.push((*start..*end, Vec::new()));
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
    for (key, edits) in edits_by_range {
        // Skip msgstr fixes whose target lives inside an entry that's being
        // deleted: the msgstr edit would conflict with the parent deletion,
        // and the change is moot since the whole entry is going away.
        if entry_deletions
            .iter()
            .any(|(es, ee)| *es <= key.0 && key.1 <= *ee)
        {
            continue;
        }
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
        let bytes = format_msgstr_block(original_block, &new_value, page_width);
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
        if let Some((new_data, fixes_applied)) =
            apply_fixes_to_data(&data, &checker.diagnostics, checker.config.check.width)
        {
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
            force_trans_file: None,
            no_trans_file: None,
            lang_id: None,
            langs: None,
            short_factor: None,
            long_factor: None,
            severity: vec![],
            punc_ignore_ellipsis: false,
            accelerator: None,
            no_errors: false,
            sort: args::CheckSort::default(),
            rule_stats: false,
            file_stats: false,
            output: args::CheckOutputFormat::default(),
            quiet: true,
            fix: false,
            width: None,
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

    /// PO content with stray Unicode control characters in two translations:
    /// a ZERO WIDTH SPACE inside "Save" and a SOFT HYPHEN inside "installation".
    const PO_UNICODE_CTRL_ISSUES: &str = "msgid \"\"
msgstr \"\"
\"Language: fr\\n\"
\"Content-Type: text/plain; charset=UTF-8\\n\"

msgid \"Save\"
msgstr \"Sa\u{200B}ve\"

msgid \"installation\"
msgstr \"instal\u{00AD}lation\"
";

    /// PO content with inconsistent leading and trailing punctuation between
    /// source and translation: msgid uses `;` and `.`, msgstr uses `,` and `!!!`.
    const PO_PUNC_ISSUES: &str = "msgid \"\"
msgstr \"\"
\"Language: fr\\n\"
\"Content-Type: text/plain; charset=UTF-8\\n\"

msgid \";hello\"
msgstr \",bonjour\"

msgid \"tested.\"
msgstr \"testé!!!\"
";

    /// PO content with one live entry and one obsolete entry that has a
    /// translator comment on top.
    const PO_OBSOLETE_ISSUES: &str = "msgid \"\"
msgstr \"\"
\"Language: fr\\n\"
\"Content-Type: text/plain; charset=UTF-8\\n\"

msgid \"hello\"
msgstr \"bonjour\"

# old translator note
#~ msgid \"goodbye\"
#~ msgstr \"au revoir\"
";

    #[test]
    fn test_fix_deletes_obsolete_entries_including_comments() {
        let tmp = tmp_dir("fix-obsolete");
        let po_path = write_po(tmp.path(), "fr.po", PO_OBSOLETE_ISSUES);

        let mut args = default_check_args();
        args.no_config = true;
        args.select = Some("obsolete".to_string());
        args.obsolete = true;
        args.fix = true;
        let result = check_file(&po_path, &args);

        let remaining = result
            .diagnostics
            .iter()
            .filter(|d| d.rule == "obsolete")
            .count();
        assert_eq!(
            remaining, 0,
            "expected no obsolete diagnostics after --fix, got {:?}",
            result.diagnostics
        );

        let fixed = std::fs::read_to_string(&po_path).expect("read fixed file");
        // Live entry preserved.
        assert!(fixed.contains("msgid \"hello\""));
        assert!(fixed.contains("msgstr \"bonjour\""));
        // Obsolete entry and its preceding comment removed.
        assert!(!fixed.contains("goodbye"));
        assert!(!fixed.contains("au revoir"));
        assert!(!fixed.contains("old translator note"));
        assert!(!fixed.contains("#~"));
    }

    /// PO content with consecutive repeated words ("un un" and "et et") in
    /// the same translation.
    const PO_DOUBLE_WORDS_ISSUES: &str = "msgid \"\"
msgstr \"\"
\"Language: fr\\n\"
\"Content-Type: text/plain; charset=UTF-8\\n\"

msgid \"test\"
msgstr \"ceci est un un test et et\"
";

    #[test]
    fn test_fix_removes_consecutive_repeated_words() {
        let tmp = tmp_dir("fix-double-words");
        let po_path = write_po(tmp.path(), "fr.po", PO_DOUBLE_WORDS_ISSUES);

        let mut args = default_check_args();
        args.no_config = true;
        args.select = Some("double-words".to_string());
        args.fix = true;
        let result = check_file(&po_path, &args);

        let remaining = result
            .diagnostics
            .iter()
            .filter(|d| d.rule == "double-words")
            .count();
        assert_eq!(
            remaining, 0,
            "expected no double-words diagnostics after --fix, got {:?}",
            result.diagnostics
        );

        let fixed = std::fs::read_to_string(&po_path).expect("read fixed file");
        assert!(fixed.contains("\"ceci est un test et\""));
        assert!(!fixed.contains("un un"));
        assert!(!fixed.contains("et et"));
    }

    /// PO content with a header missing both `Content-Type` and
    /// `Content-Transfer-Encoding`. Other required fields are present so the
    /// fix exercises only the auto-fixable subset.
    const PO_HEADER_MISSING_CONTENT_TYPE: &str = "msgid \"\"
msgstr \"\"
\"Project-Id-Version: poexam\\n\"
\"Last-Translator: Sébastien Helleu <flashcode@flashtux.org>\\n\"
\"Language-Team: French <translators-fr@example.com>\\n\"
\"Language: fr\\n\"
\"Report-Msgid-Bugs-To: flashcode@flashtux.org\\n\"
\"POT-Creation-Date: 2026-02-01 18:12:08+0100\\n\"
\"PO-Revision-Date: 2026-02-01 18:12:08+0100\\n\"

msgid \"hello\"
msgstr \"bonjour\"
";

    #[test]
    fn test_fix_appends_default_content_type_and_transfer_encoding_to_header() {
        let tmp = tmp_dir("fix-header");
        let po_path = write_po(tmp.path(), "fr.po", PO_HEADER_MISSING_CONTENT_TYPE);

        let mut args = default_check_args();
        args.no_config = true;
        args.select = Some("header".to_string());
        args.fix = true;
        let result = check_file(&po_path, &args);

        // Both fixable header diagnostics should be gone after --fix.
        let remaining = result
            .diagnostics
            .iter()
            .filter(|d| {
                d.message.contains("'Content-Type'")
                    || d.message.contains("'Content-Transfer-Encoding'")
            })
            .count();
        assert_eq!(
            remaining, 0,
            "expected no Content-Type/Content-Transfer-Encoding diagnostics after --fix, \
             got {:?}",
            result.diagnostics
        );

        // The rewritten file must include both new header fields.
        let fixed = std::fs::read_to_string(&po_path).expect("read fixed file");
        assert!(fixed.contains("Content-Type: text/plain; charset=UTF-8"));
        assert!(fixed.contains("Content-Transfer-Encoding: 8bit"));
    }

    /// PO content with French-typography violations the punc-space-str rule
    /// flags: regular space before `:` and `!`, missing space before `%`,
    /// regular spaces around `« »`.
    const PO_PUNC_SPACE_ISSUES: &str = "msgid \"\"
msgstr \"\"
\"Language: fr\\n\"
\"Content-Type: text/plain; charset=UTF-8\\n\"

msgid \"completion: 42%, this is a test!\"
msgstr \"achèvement : 42%, ceci est un test !\"

msgid \"French quotes: test\"
msgstr \"Guillemets : « test »\"
";

    #[test]
    fn test_fix_inserts_non_breaking_spaces_in_french_translation() {
        let tmp = tmp_dir("fix-punc-space");
        let po_path = write_po(tmp.path(), "fr.po", PO_PUNC_SPACE_ISSUES);

        let mut args = default_check_args();
        args.no_config = true;
        args.select = Some("punc-space-str".to_string());
        args.fix = true;
        let result = check_file(&po_path, &args);

        let remaining = result
            .diagnostics
            .iter()
            .filter(|d| d.rule == "punc-space-str")
            .count();
        assert_eq!(
            remaining, 0,
            "expected no punc-space-str diagnostics after --fix, got {:?}",
            result.diagnostics
        );

        let fixed = std::fs::read_to_string(&po_path).expect("read fixed file");
        // Regular space before `:` and `!` replaced with NBSP.
        assert!(fixed.contains("achèvement\u{00A0}:"));
        assert!(fixed.contains("test\u{00A0}!"));
        // NBSP inserted between digit and `%`.
        assert!(fixed.contains("42\u{00A0}%"));
        // Regular spaces around `« »` replaced with NBSPs.
        assert!(fixed.contains("«\u{00A0}test\u{00A0}»"));
    }

    #[test]
    fn test_fix_replaces_inconsistent_punctuation() {
        let tmp = tmp_dir("fix-punc");
        let po_path = write_po(tmp.path(), "fr.po", PO_PUNC_ISSUES);

        let mut args = default_check_args();
        args.no_config = true;
        args.select = Some("punc-start,punc-end".to_string());
        args.fix = true;
        let result = check_file(&po_path, &args);

        // Re-checking the rewritten file must report zero punc diagnostics.
        let remaining = result
            .diagnostics
            .iter()
            .filter(|d| d.rule == "punc-start" || d.rule == "punc-end")
            .count();
        assert_eq!(
            remaining, 0,
            "expected no punc diagnostics after --fix, got {:?}",
            result.diagnostics
        );

        // The file on disk should now contain the mirrored punctuation.
        let fixed = std::fs::read_to_string(&po_path).expect("read fixed file");
        assert!(fixed.contains("msgstr \";bonjour\""));
        assert!(fixed.contains("msgstr \"testé.\""));
    }

    #[test]
    fn test_fix_removes_stray_unicode_control_chars() {
        let tmp = tmp_dir("fix-unicode-ctrl");
        let po_path = write_po(tmp.path(), "fr.po", PO_UNICODE_CTRL_ISSUES);

        let mut args = default_check_args();
        args.no_config = true;
        args.select = Some("unicode-ctrl".to_string());
        args.fix = true;
        let result = check_file(&po_path, &args);

        // Both stray chars were fixable, so re-check reports no unicode-ctrl diagnostics.
        let remaining = result
            .diagnostics
            .iter()
            .filter(|d| d.rule == "unicode-ctrl")
            .count();
        assert_eq!(
            remaining, 0,
            "expected no unicode-ctrl diagnostics after --fix, got {:?}",
            result.diagnostics
        );

        // The rewritten file must contain the cleaned msgstrs.
        let fixed = std::fs::read_to_string(&po_path).expect("read fixed file");
        assert!(fixed.contains("msgstr \"Save\""));
        assert!(fixed.contains("msgstr \"installation\""));
        // And must not contain the stray characters anywhere.
        assert!(!fixed.contains('\u{200B}'));
        assert!(!fixed.contains('\u{00AD}'));
    }
}
