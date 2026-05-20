// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `force-trans` rule: words from the source that must
//! be translated (must not appear verbatim in the translation).

use std::collections::HashSet;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::po::format::iter::FormatWordPos;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct ForceTransRule;

impl RuleChecker for ForceTransRule {
    fn name(&self) -> &'static str {
        "force-trans"
    }

    fn description(&self) -> &'static str {
        "Check that words listed in `force-trans-file` are translated (not present in translation)."
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check that every word listed in `check.force_trans_file` that appears
    /// in the source string has been translated, i.e. does NOT also appear
    /// verbatim in the translation. Matching against the word list is
    /// case-insensitive, but the comparison between source and translation
    /// is case-sensitive: a translation that reuses the source word with a
    /// **different case** is considered a deliberate variant and is not
    /// flagged.
    ///
    /// This rule is not enabled by default and is silently skipped when the
    /// word list could not be loaded.
    ///
    /// Wrong entry (with `forbidden` in the force-trans file):
    /// ```text
    /// msgid "this is forbidden"
    /// msgstr "ceci est forbidden"
    /// ```
    ///
    /// Correct entries:
    /// ```text
    /// msgid "this is forbidden"
    /// msgstr "ceci est interdit"
    ///
    /// msgid "this is Forbidden"
    /// msgstr "ceci est forbidden"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`warning`](Severity::Warning): `word '…' must be translated`
    fn check_msg(
        &self,
        checker: &Checker,
        entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let Some(force_words) = checker.force_trans_words.as_ref() else {
            return vec![];
        };
        // Collect the exact case-forms used in the source for each word that
        // matches the force-trans list (case-insensitively against the list).
        let mut id_forms: HashSet<String> = HashSet::new();
        for word in FormatWordPos::new(&msgid.value, entry.format_language) {
            if force_words.contains(&word.s.to_lowercase()) {
                id_forms.insert(word.s.to_string());
            }
        }
        if id_forms.is_empty() {
            return vec![];
        }
        // Flag translation words whose case-form matches one of the source's
        // exact case-forms. A different case in the translation is considered
        // an acceptable variant. One diagnostic per offending case-form, with
        // all occurrences highlighted in both msgid and msgstr.
        let mut diags = vec![];
        let mut reported: HashSet<String> = HashSet::new();
        for str_word in FormatWordPos::new(&msgstr.value, entry.format_language) {
            if !id_forms.contains(str_word.s) || reported.contains(str_word.s) {
                continue;
            }
            reported.insert(str_word.s.to_string());
            let id_hl: Vec<(usize, usize)> =
                FormatWordPos::new(&msgid.value, entry.format_language)
                    .filter(|w| w.s == str_word.s)
                    .map(|w| (w.start, w.end))
                    .collect();
            let str_hl: Vec<(usize, usize)> =
                FormatWordPos::new(&msgstr.value, entry.format_language)
                    .filter(|w| w.s == str_word.s)
                    .map(|w| (w.start, w.end))
                    .collect();
            diags.extend(
                self.new_diag(
                    checker,
                    Severity::Warning,
                    format!("word '{}' must be translated", str_word.s),
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

    /// Write a temporary force-trans file with the given content and return
    /// the path along with the owning `TempDir`.
    fn write_force_file(content: &str) -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::TempDir::with_prefix("poexam-force-trans-")
            .expect("create force-trans temp dir");
        let path = tmp.path().join("force.txt");
        std::fs::write(&path, content).expect("write force-trans file");
        (tmp, path)
    }

    fn check_force_trans(force_file: &Path, content: &str) -> Vec<Diagnostic> {
        let mut config = Config::default();
        config.check.force_trans_file = Some(force_file.to_path_buf());
        let mut checker = Checker::new(content.as_bytes()).with_config(config);
        let rules = Rules::new(vec![Box::new(ForceTransRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_no_match_in_source_is_ok() {
        // Source has none of the force-trans words → nothing to report.
        let (_tmp, force) = write_force_file("forbidden\nstop\n");
        let diags = check_force_trans(
            &force,
            r#"
msgid "hello world"
msgstr "bonjour le monde"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_translated_word_is_ok() {
        // Source has a force-trans word but the translation does not.
        let (_tmp, force) = write_force_file("forbidden\n");
        let diags = check_force_trans(
            &force,
            r#"
msgid "this is forbidden"
msgstr "ceci est interdit"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_word_present_in_translation_is_flagged() {
        let (_tmp, force) = write_force_file("forbidden\n");
        let diags = check_force_trans(
            &force,
            r#"
msgid "this is forbidden"
msgstr "ceci est forbidden"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Warning);
        assert_eq!(diags[0].message, "word 'forbidden' must be translated");
    }

    #[test]
    fn test_case_insensitive_word_list_and_match() {
        // Force-trans file uses lowercase "stop"; source has "STOP", translation
        // also has "STOP" — should be flagged regardless of case.
        let (_tmp, force) = write_force_file("stop\n");
        let diags = check_force_trans(
            &force,
            r#"
msgid "STOP now"
msgstr "STOP maintenant"
"#,
        );
        assert_eq!(diags.len(), 1);
        // The diagnostic message reports the offending translation word as-is.
        assert_eq!(diags[0].message, "word 'STOP' must be translated");
    }

    #[test]
    fn test_translation_with_different_case_is_ok() {
        // Source "Forbidden", translation "forbidden" — the translator chose a
        // different case-form, which counts as a translation variant; not
        // flagged.
        let (_tmp, force) = write_force_file("forbidden\n");
        let diags = check_force_trans(
            &force,
            r#"
msgid "this is Forbidden"
msgstr "ceci est forbidden"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_each_source_case_form_flagged_independently() {
        // Source has both "Forbidden" and "forbidden". Translation reuses both
        // exact forms verbatim → one diagnostic per case-form.
        let (_tmp, force) = write_force_file("forbidden\n");
        let diags = check_force_trans(
            &force,
            r#"
msgid "Forbidden, then forbidden"
msgstr "Forbidden, puis forbidden"
"#,
        );
        assert_eq!(diags.len(), 2);
        let mut msgs: Vec<&str> = diags.iter().map(|d| d.message.as_ref()).collect();
        msgs.sort_unstable();
        assert_eq!(
            msgs,
            vec![
                "word 'Forbidden' must be translated",
                "word 'forbidden' must be translated",
            ]
        );
    }

    #[test]
    fn test_one_diagnostic_per_word_even_with_multiple_occurrences() {
        // Translation contains the forbidden word twice — only one diagnostic
        // for the word, but both occurrences are highlighted.
        let (_tmp, force) = write_force_file("forbidden\n");
        let diags = check_force_trans(
            &force,
            r#"
msgid "this is forbidden"
msgstr "ceci est forbidden, forbidden"
"#,
        );
        assert_eq!(diags.len(), 1);
        // Two msgstr highlights for the two occurrences.
        let str_line = diags[0].lines.last().expect("msgstr line");
        assert_eq!(str_line.highlights.len(), 2);
    }

    #[test]
    fn test_multiple_distinct_force_words_each_reported_once() {
        let (_tmp, force) = write_force_file("forbidden\nstop\n");
        let diags = check_force_trans(
            &force,
            r#"
msgid "stop, this is forbidden"
msgstr "stop, ceci est forbidden"
"#,
        );
        assert_eq!(diags.len(), 2);
        let mut msgs: Vec<&str> = diags.iter().map(|d| d.message.as_ref()).collect();
        msgs.sort_unstable();
        assert_eq!(
            msgs,
            vec![
                "word 'forbidden' must be translated",
                "word 'stop' must be translated",
            ]
        );
    }

    #[test]
    fn test_comments_and_blank_lines_in_force_file_are_ignored() {
        let (_tmp, force) = write_force_file("# this is a comment\n\nforbidden\n  # comment\n");
        let diags = check_force_trans(
            &force,
            r#"
msgid "this is forbidden"
msgstr "ceci est forbidden"
"#,
        );
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_noqa_suppresses_force_trans() {
        let (_tmp, force) = write_force_file("forbidden\n");
        let diags = check_force_trans(
            &force,
            r#"
#, noqa:force-trans
msgid "this is forbidden"
msgstr "ceci est forbidden"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_missing_force_file_emits_warning_and_skips_checks() {
        // Path that doesn't exist on disk.
        let missing = PathBuf::from("/this/path/should/not/exist/force.txt");
        let diags = check_force_trans(
            &missing,
            r#"
msgid "this is forbidden"
msgstr "ceci est forbidden"
"#,
        );
        // Exactly one diagnostic: the file-load warning.
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "force-trans");
        assert_eq!(diags[0].severity, Severity::Warning);
        assert!(diags[0].message.contains("words file not found"));
        assert!(diags[0].message.contains("force-trans rule ignored"));
        assert!(diags[0].message.contains("force.txt"));
    }
}
