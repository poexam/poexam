// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `acronyms` rule: acronyms found in the source string
//! (all-uppercase words of length ≥ 2) must be preserved as-is in the
//! translation (i.e. they must not be translated).

use std::collections::HashSet;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::po::format::iter::FormatAcronymPos;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct AcronymsRule;

impl RuleChecker for AcronymsRule {
    fn name(&self) -> &'static str {
        "acronyms"
    }

    fn description(&self) -> &'static str {
        "Check that acronyms (all-uppercase words of length ≥ 2) from the source appear as-is in the translation."
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check that every acronym (all-uppercase word of length ≥ 2) found in
    /// the source string also appears verbatim in the translation.
    ///
    /// When the `force-trans` rule is enabled and a `force-trans-file` is
    /// configured, source acronyms whose lowercase form is listed in that file
    /// are ignored here, since they MUST be translated and would be flagged by
    /// the `force-trans` rule otherwise.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "Use the HTTP API"
    /// msgstr "Utiliser l'interface"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "Use the HTTP API"
    /// msgstr "Utiliser l'API HTTP"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`warning`](Severity::Warning): `acronym '…' must not be translated`
    fn check_msg(
        &self,
        checker: &Checker,
        entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        // Collect unique acronyms found in the source, skipping any acronym
        // that the `force-trans` rule marks as "must be translated".
        let force_words = checker.force_trans_words.as_ref();
        let mut id_acronyms: HashSet<String> = HashSet::new();
        for word in FormatAcronymPos::new(&msgid.value, entry.format_language) {
            if force_words.is_some_and(|words| words.contains(&word.s.to_lowercase())) {
                continue;
            }
            id_acronyms.insert(word.s.to_string());
        }
        if id_acronyms.is_empty() {
            return vec![];
        }
        // Look for each source acronym among the translation's acronyms. The
        // match is case-sensitive: an acronym in the translation must be the
        // exact same uppercase run as in the source.
        let str_acronyms: HashSet<&str> =
            FormatAcronymPos::new(&msgstr.value, entry.format_language)
                .map(|w| w.s)
                .collect();
        let mut missing: Vec<String> = id_acronyms
            .into_iter()
            .filter(|a| !str_acronyms.contains(a.as_str()))
            .collect();
        missing.sort_unstable();
        let mut diags = vec![];
        for acronym in missing {
            let id_hl: Vec<(usize, usize)> =
                FormatAcronymPos::new(&msgid.value, entry.format_language)
                    .filter(|w| w.s == acronym)
                    .map(|w| (w.start, w.end))
                    .collect();
            diags.extend(
                self.new_diag(
                    checker,
                    Severity::Warning,
                    format!("acronym '{acronym}' must not be translated"),
                )
                .map(|d| d.with_msgs_hl(msgid, id_hl, msgstr, [])),
            );
        }
        diags
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::{config::Config, diagnostic::Diagnostic, rules::rule::Rules};

    fn check_acronyms(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(AcronymsRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    /// Run both the `acronyms` and `force-trans` rules together, using the
    /// given force-trans file content. The `acronyms` rule must then ignore
    /// any source acronym whose lowercase form is listed in that file.
    fn check_acronyms_with_force_trans(force_words: &str, content: &str) -> Vec<Diagnostic> {
        let tmp = tempfile::TempDir::with_prefix("poexam-acronyms-force-")
            .expect("create force-trans temp dir");
        let force_path = tmp.path().join("force.txt");
        std::fs::write(&force_path, force_words).expect("write force-trans file");
        let mut config = Config::default();
        config.check.force_trans_file = Some(force_path);
        let mut checker = Checker::new(content.as_bytes()).with_config(config);
        let rules = Rules::new(vec![
            Box::new(AcronymsRule {}),
            Box::new(crate::rules::force_trans::ForceTransRule {}),
        ]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_no_acronym_in_source_is_ok() {
        let diags = check_acronyms(
            r#"
msgid "hello world"
msgstr "bonjour le monde"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_acronym_present_in_translation_is_ok() {
        let diags = check_acronyms(
            r#"
msgid "Use the HTTP API"
msgstr "Utiliser l'API HTTP"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_acronym_missing_in_translation_is_flagged() {
        let diags = check_acronyms(
            r#"
msgid "Use the HTTP protocol"
msgstr "Utiliser le protocole"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Warning);
        assert_eq!(diags[0].message, "acronym 'HTTP' must not be translated");
    }

    #[test]
    fn test_multiple_missing_acronyms_each_reported_once() {
        let diags = check_acronyms(
            r#"
msgid "Use the HTTP and JSON for the API"
msgstr "Utiliser le protocole et le format pour l'interface"
"#,
        );
        assert_eq!(diags.len(), 3);
        let mut msgs: Vec<&str> = diags.iter().map(|d| d.message.as_ref()).collect();
        msgs.sort_unstable();
        assert_eq!(
            msgs,
            vec![
                "acronym 'API' must not be translated",
                "acronym 'HTTP' must not be translated",
                "acronym 'JSON' must not be translated",
            ]
        );
    }

    #[test]
    fn test_one_diagnostic_per_acronym_even_with_multiple_occurrences() {
        // Source has "HTTP" twice — only one diagnostic, with both highlights.
        let diags = check_acronyms(
            r#"
msgid "HTTP first, HTTP second"
msgstr "premier protocole, second protocole"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "acronym 'HTTP' must not be translated");
        let id_line = diags[0].lines.first().expect("msgid line");
        assert_eq!(id_line.highlights.len(), 2);
    }

    #[test]
    fn test_short_uppercase_word_is_not_an_acronym() {
        // Single-character uppercase words are not acronyms (length < 2).
        let diags = check_acronyms(
            r#"
msgid "Section A is here"
msgstr "La section est ici"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_mixed_case_word_is_not_an_acronym() {
        // "Http" is mixed-case → not flagged.
        let diags = check_acronyms(
            r#"
msgid "Use the Http protocol"
msgstr "Utiliser le protocole"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_word_with_digit_is_an_acronym() {
        // Digits are caseless: "MP3" has no lowercase letter, so it is an
        // acronym and must appear verbatim in the translation.
        let diags = check_acronyms(
            r#"
msgid "Play MP3 files"
msgstr "Lire les fichiers audio"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "acronym 'MP3' must not be translated");
    }

    #[test]
    fn test_word_with_digit_preserved_in_translation_is_ok() {
        let diags = check_acronyms(
            r#"
msgid "Play MP3 files"
msgstr "Lire les fichiers MP3"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_pure_digit_word_is_not_an_acronym() {
        // "123" has no cased characters → not an acronym (Python's
        // `"123".isupper()` returns False).
        let diags = check_acronyms(
            r#"
msgid "Code 123 found"
msgstr "Trouvé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_acronym_case_must_match_exactly() {
        // Source uses "URL", translation uses "url" — different case is flagged.
        let diags = check_acronyms(
            r#"
msgid "Open the URL"
msgstr "Ouvrir l'url"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "acronym 'URL' must not be translated");
    }

    #[test]
    fn test_format_strings_are_skipped() {
        // `%s` must not be treated as a candidate (single-char `s` would not be
        // an acronym anyway), and the surrounding `HTTP` is still required.
        let diags = check_acronyms(
            r#"
#, c-format
msgid "HTTP error: %s"
msgstr "erreur de protocole: %s"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "acronym 'HTTP' must not be translated");
    }

    #[test]
    fn test_force_trans_words_are_ignored_by_acronyms() {
        // "URL" is listed in the force-trans file → translator MUST translate
        // it, so the acronyms rule does not also require it in the msgstr.
        // The force-trans rule itself fires nothing here because "URL" is not
        // reused verbatim in the translation.
        let diags = check_acronyms_with_force_trans(
            "url\n",
            r#"
msgid "Open the URL now"
msgstr "Ouvrir l'adresse maintenant"
"#,
        );
        assert!(diags.is_empty(), "expected no diagnostics, got {diags:?}");
    }

    #[test]
    fn test_force_trans_does_not_affect_other_acronyms() {
        // Only "URL" is in the force-trans list; "HTTP" must still be
        // preserved in the translation.
        let diags = check_acronyms_with_force_trans(
            "url\n",
            r#"
msgid "Open the URL via HTTP"
msgstr "Ouvrir l'adresse via le protocole"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "acronym 'HTTP' must not be translated");
    }

    #[test]
    fn test_force_trans_not_loaded_when_file_missing_still_checks_acronyms() {
        // A missing force-trans file leaves `force_trans_words` as None; the
        // acronyms rule then ignores no acronyms and flags "URL" as usual.
        // (`force-trans` itself emits its own load-error diagnostic.)
        let mut config = Config::default();
        config.check.force_trans_file = Some(PathBuf::from("/no/such/force.txt"));
        let mut checker = Checker::new(
            br#"
msgid "Open the URL"
msgstr "Ouvrir l'adresse"
"#,
        )
        .with_config(config);
        let rules = Rules::new(vec![
            Box::new(AcronymsRule {}),
            Box::new(crate::rules::force_trans::ForceTransRule {}),
        ]);
        checker.do_all_checks(&rules);
        let acronym_diags: Vec<_> = checker
            .diagnostics
            .iter()
            .filter(|d| d.rule == "acronyms")
            .collect();
        assert_eq!(acronym_diags.len(), 1);
        assert_eq!(
            acronym_diags[0].message,
            "acronym 'URL' must not be translated"
        );
    }

    #[test]
    fn test_noqa_suppresses_acronyms() {
        let diags = check_acronyms(
            r#"
#, noqa:acronyms
msgid "Use the HTTP protocol"
msgstr "Utiliser le protocole"
"#,
        );
        assert!(diags.is_empty());
    }
}
