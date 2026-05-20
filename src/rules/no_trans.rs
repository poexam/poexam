// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `no-trans` rule: words from the source that must NOT
//! be translated (must appear in the translation with the same case used in
//! the source).

use std::collections::HashMap;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::po::format::iter::FormatWordPos;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct NoTransRule;

impl RuleChecker for NoTransRule {
    fn name(&self) -> &'static str {
        "no-trans"
    }

    fn description(&self) -> &'static str {
        "Check that words listed in `no-trans-file` appear in translation, with the source case."
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check that every word listed in `check.no_trans_file` that appears in
    /// the source string also appears in the translation, the same number of
    /// times, and with the **exact case used in the source** (which may
    /// differ from the case used in the word list file — the list itself is
    /// case-insensitive).
    ///
    /// This rule is not enabled by default and is silently skipped when the
    /// word list could not be loaded.
    ///
    /// Wrong entries (with `linux` in the no-trans file):
    /// ```text
    /// msgid "Linux is great"
    /// msgstr "linux est génial"
    ///
    /// msgid "Linux Linux"
    /// msgstr "Linux"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "Linux is great"
    /// msgstr "Linux est génial"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`warning`](Severity::Warning): `word '…' must not be translated (… in source, … in translation)`
    fn check_msg(
        &self,
        checker: &Checker,
        entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let Some(no_trans_words) = checker.no_trans_words.as_ref() else {
            return vec![];
        };
        // Tally the exact case-form of each source word matching the no-trans
        // list (case-insensitively). Iteration order of the resulting map is
        // unspecified, so the diagnostics list is sorted at the end for
        // deterministic output.
        let mut id_counts: HashMap<String, usize> = HashMap::new();
        for word in FormatWordPos::new(&msgid.value, entry.format_language) {
            if no_trans_words.contains(&word.s.to_lowercase()) {
                *id_counts.entry(word.s.to_string()).or_insert(0) += 1;
            }
        }
        if id_counts.is_empty() {
            return vec![];
        }
        // Count exact-case occurrences of these words in the translation.
        let mut str_counts: HashMap<String, usize> = HashMap::new();
        let str_words: Vec<_> = FormatWordPos::new(&msgstr.value, entry.format_language).collect();
        for word in &str_words {
            if id_counts.contains_key(word.s) {
                *str_counts.entry(word.s.to_string()).or_insert(0) += 1;
            }
        }
        let mut diffs: Vec<(String, usize, usize)> = id_counts
            .into_iter()
            .filter_map(|(word, id_count)| {
                let str_count = str_counts.get(&word).copied().unwrap_or(0);
                (str_count != id_count).then_some((word, id_count, str_count))
            })
            .collect();
        diffs.sort_by(|a, b| a.0.cmp(&b.0));
        let mut diags = vec![];
        for (word, id_count, str_count) in diffs {
            let id_hl: Vec<(usize, usize)> =
                FormatWordPos::new(&msgid.value, entry.format_language)
                    .filter(|w| w.s == word)
                    .map(|w| (w.start, w.end))
                    .collect();
            let str_hl: Vec<(usize, usize)> = str_words
                .iter()
                .filter(|w| w.s == word)
                .map(|w| (w.start, w.end))
                .collect();
            diags.extend(
                self.new_diag(
                    checker,
                    Severity::Warning,
                    format!(
                        "word '{word}' must not be translated ({id_count} in source, {str_count} in translation)"
                    ),
                )
                .map(|d| d.with_msgs_hl(msgid, id_hl, msgstr, str_hl)),
            );
        }
        diags
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::{config::Config, diagnostic::Diagnostic, rules::rule::Rules};

    /// Write a temporary no-trans file with the given content and return the
    /// path along with the owning `TempDir`.
    fn write_no_trans_file(content: &str) -> (tempfile::TempDir, PathBuf) {
        let tmp =
            tempfile::TempDir::with_prefix("poexam-no-trans-").expect("create no-trans temp dir");
        let path = tmp.path().join("no-trans.txt");
        std::fs::write(&path, content).expect("write no-trans file");
        (tmp, path)
    }

    fn check_no_trans(no_trans_file: &Path, content: &str) -> Vec<Diagnostic> {
        let mut config = Config::default();
        config.check.no_trans_file = Some(no_trans_file.to_path_buf());
        let mut checker = Checker::new(content.as_bytes()).with_config(config);
        let rules = Rules::new(vec![Box::new(NoTransRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_no_match_in_source_is_ok() {
        let (_tmp, no_trans) = write_no_trans_file("linux\n");
        let diags = check_no_trans(
            &no_trans,
            r#"
msgid "hello world"
msgstr "bonjour le monde"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_present_with_matching_case_is_ok() {
        let (_tmp, no_trans) = write_no_trans_file("linux\n");
        let diags = check_no_trans(
            &no_trans,
            r#"
msgid "Linux is great"
msgstr "Linux est génial"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_missing_in_translation_is_flagged() {
        let (_tmp, no_trans) = write_no_trans_file("linux\n");
        let diags = check_no_trans(
            &no_trans,
            r#"
msgid "Linux is great"
msgstr "ceci est génial"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Warning);
        assert_eq!(
            diags[0].message,
            "word 'Linux' must not be translated (1 in source, 0 in translation)"
        );
    }

    #[test]
    fn test_different_case_in_translation_is_flagged() {
        // Source uses "Linux" (capital L), translation uses "linux" (lowercase) —
        // case must match the source form.
        let (_tmp, no_trans) = write_no_trans_file("linux\n");
        let diags = check_no_trans(
            &no_trans,
            r#"
msgid "Linux is great"
msgstr "linux est génial"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "word 'Linux' must not be translated (1 in source, 0 in translation)"
        );
    }

    #[test]
    fn test_word_list_case_does_not_matter() {
        // The no-trans file lists "LINUX" in uppercase; the rule must still
        // detect "Linux" in source and require "Linux" (source case) in the
        // translation.
        let (_tmp, no_trans) = write_no_trans_file("LINUX\n");
        let diags = check_no_trans(
            &no_trans,
            r#"
msgid "Linux is great"
msgstr "Linux est génial"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_count_mismatch_is_flagged() {
        // Source has "Linux" twice, translation has it once — count mismatch.
        let (_tmp, no_trans) = write_no_trans_file("linux\n");
        let diags = check_no_trans(
            &no_trans,
            r#"
msgid "Linux and Linux"
msgstr "Linux"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "word 'Linux' must not be translated (2 in source, 1 in translation)"
        );
    }

    #[test]
    fn test_position_independent_count_match_is_ok() {
        // Counts match, positions differ — should pass.
        let (_tmp, no_trans) = write_no_trans_file("linux\n");
        let diags = check_no_trans(
            &no_trans,
            r#"
msgid "Linux is Linux"
msgstr "Linux Linux est ok"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_different_cases_in_source_each_tracked_separately() {
        // Source has "Linux" once and "LINUX" once → translation must contain
        // each exact case-form the same number of times.
        let (_tmp, no_trans) = write_no_trans_file("linux\n");
        let diags = check_no_trans(
            &no_trans,
            r#"
msgid "Linux and LINUX"
msgstr "Linux and LINUX"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_different_cases_in_source_partial_translation_is_flagged() {
        // Translation only has "Linux" twice — "LINUX" is missing.
        let (_tmp, no_trans) = write_no_trans_file("linux\n");
        let diags = check_no_trans(
            &no_trans,
            r#"
msgid "Linux and LINUX"
msgstr "Linux and Linux"
"#,
        );
        // Two diagnostics: one for "LINUX" missing, one for "Linux" extra count.
        assert_eq!(diags.len(), 2);
        let mut msgs: Vec<&str> = diags.iter().map(|d| d.message.as_ref()).collect();
        msgs.sort_unstable();
        assert_eq!(
            msgs,
            vec![
                "word 'LINUX' must not be translated (1 in source, 0 in translation)",
                "word 'Linux' must not be translated (1 in source, 2 in translation)",
            ]
        );
    }

    #[test]
    fn test_multiple_distinct_no_trans_words() {
        let (_tmp, no_trans) = write_no_trans_file("linux\nposix\n");
        let diags = check_no_trans(
            &no_trans,
            r#"
msgid "Linux and POSIX"
msgstr "ceci est génial"
"#,
        );
        assert_eq!(diags.len(), 2);
        let mut msgs: Vec<&str> = diags.iter().map(|d| d.message.as_ref()).collect();
        msgs.sort_unstable();
        assert_eq!(
            msgs,
            vec![
                "word 'Linux' must not be translated (1 in source, 0 in translation)",
                "word 'POSIX' must not be translated (1 in source, 0 in translation)",
            ]
        );
    }

    #[test]
    fn test_comments_and_blank_lines_in_no_trans_file_are_ignored() {
        let (_tmp, no_trans) = write_no_trans_file("# header\n\nlinux\n  # trailing\n");
        let diags = check_no_trans(
            &no_trans,
            r#"
msgid "Linux is great"
msgstr "Linux est génial"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_noqa_suppresses_no_trans() {
        let (_tmp, no_trans) = write_no_trans_file("linux\n");
        let diags = check_no_trans(
            &no_trans,
            r#"
#, noqa:no-trans
msgid "Linux is great"
msgstr "ceci est génial"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_missing_no_trans_file_emits_warning_and_skips_checks() {
        let missing = PathBuf::from("/this/path/should/not/exist/no-trans.txt");
        let diags = check_no_trans(
            &missing,
            r#"
msgid "Linux is great"
msgstr "ceci est génial"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "no-trans");
        assert_eq!(diags[0].severity, Severity::Warning);
        assert!(diags[0].message.contains("words file not found"));
        assert!(diags[0].message.contains("no-trans rule ignored"));
        assert!(diags[0].message.contains("no-trans.txt"));
    }
}
