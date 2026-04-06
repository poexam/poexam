// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the spelling rules: check spelling:
//! - `spelling-ctxt`: in the context (`msgctxt`)
//! - `spelling-id`: in the source (`msgid`)
//! - `spelling-str`: in the translation (`msgstr`)

use std::collections::HashSet;

use spellbook::Dictionary;

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::po::entry::Entry;
use crate::po::format::iterators::FormatWordPos;
use crate::po::format::language::Language;
use crate::rules::rule::RuleChecker;

pub struct SpellingCtxtRule;

impl RuleChecker for SpellingCtxtRule {
    fn name(&self) -> &'static str {
        "spelling-ctxt"
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    /// Check spelling in the context string (English).
    ///
    /// This rule is not enabled by default.
    ///
    /// Wrong entry:
    /// ```text
    /// msgctxt "month of the yearr"
    /// msgid "May"
    /// msgstr "Mai"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgctxt "month of the year"
    /// msgid "May"
    /// msgstr "Mai"
    /// ```
    ///
    /// Diagnostics reported with severity [`info`](Severity::Info):
    /// - `misspelled words in context: xxx`
    fn check_ctxt(&self, checker: &mut Checker, entry: &Entry, msgctxt: &str) {
        if let Some(dict) = &checker.dict_id {
            let (misspelled_words, pos_words) = check_words(msgctxt, &entry.format_language, dict);
            if !misspelled_words.is_empty() {
                checker.report_line(
                    entry,
                    format!(
                        "misspelled words in context: {}",
                        misspelled_words.join(", ")
                    ),
                    msgctxt,
                    &pos_words,
                );
                for word in misspelled_words {
                    checker.add_misspelled_word(word);
                }
            }
        }
    }
}

pub struct SpellingIdRule;

impl RuleChecker for SpellingIdRule {
    fn name(&self) -> &'static str {
        "spelling-id"
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    /// Check spelling in the source string (English).
    ///
    /// This rule is not enabled by default.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "this is a tyypo"
    /// msgstr "ceci est une faute"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "this is a typo"
    /// msgstr "ceci est une faute"
    /// ```
    ///
    /// Diagnostics reported with severity [`info`](Severity::Info):
    /// - `misspelled words in source: xxx`
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        if let Some(dict) = &checker.dict_id {
            let (misspelled_words, pos_words) = check_words(msgid, &entry.format_language, dict);
            if !misspelled_words.is_empty() {
                checker.report_id_str(
                    entry,
                    format!(
                        "misspelled words in source: {}",
                        misspelled_words.join(", ")
                    ),
                    msgid,
                    &pos_words,
                    msgstr,
                    &[],
                );
                for word in misspelled_words {
                    checker.add_misspelled_word(word);
                }
            }
        }
    }
}

pub struct SpellingStrRule;

impl RuleChecker for SpellingStrRule {
    fn name(&self) -> &'static str {
        "spelling-str"
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    /// Check spelling in the translated string (using language detected in PO file).
    ///
    /// This rule is not enabled by default.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "this is a typo"
    /// msgstr "ceci est une fôte"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "this is a typo"
    /// msgstr "ceci est une faute"
    /// ```
    ///
    /// Diagnostics reported with severity [`info`](Severity::Info):
    /// - `misspelled words in translation: xxx`
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        if let Some(dict) = &checker.dict_str {
            let (misspelled_words, pos_words) = check_words(msgstr, &entry.format_language, dict);
            if !misspelled_words.is_empty() {
                checker.report_id_str(
                    entry,
                    format!(
                        "misspelled words in translation: {}",
                        misspelled_words.join(", ")
                    ),
                    msgid,
                    &[],
                    msgstr,
                    &pos_words,
                );
                for word in misspelled_words {
                    checker.add_misspelled_word(word);
                }
            }
        }
    }
}

/// Check words in a string: context (msgctxt), source (msgid) or translation (msgstr).
///
/// Return list of misspelled words (can be empty) and their positions in the string (start, end).
fn check_words<'s>(
    s: &'s str,
    format_language: &Language,
    dict: &Dictionary,
) -> (Vec<&'s str>, Vec<(usize, usize)>) {
    let mut misspelled_words: HashSet<&str> = HashSet::new();
    let mut hash_words: HashSet<&str> = HashSet::new();
    let mut pos_words = Vec::new();
    for word in FormatWordPos::new(s, format_language) {
        // Ignore word if it contains at least one digit.
        if word.s.chars().any(|c| c.is_ascii_digit()) {
            continue;
        }
        // Ignore with at least two chars and only uppercase chars (e.g. "HTTP").
        if word.s.len() >= 2 && word.s.chars().all(|c| c.is_ascii_uppercase()) {
            continue;
        }
        if hash_words.contains(word.s) {
            if misspelled_words.contains(word.s) {
                pos_words.push((word.start, word.end));
            }
        } else {
            hash_words.insert(word.s);
            if !dict.check(word.s) {
                misspelled_words.insert(word.s);
                pos_words.push((word.start, word.end));
            }
        }
    }
    let mut list_words = misspelled_words.iter().copied().collect::<Vec<_>>();
    list_words.sort_unstable();
    (list_words, pos_words)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::{config::Config, diagnostic::Diagnostic, rules::rule::Rules};

    fn check_spelling(content: &str) -> Vec<Diagnostic> {
        let mut test_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        test_dir.push("resources");
        test_dir.push("test");
        let mut config = Config::default();
        config.check.path_dicts = test_dir;
        let mut checker = Checker::new(content.as_bytes()).with_config(config);
        let rules = Rules::new(vec![
            Box::new(SpellingCtxtRule {}),
            Box::new(SpellingIdRule {}),
            Box::new(SpellingStrRule {}),
        ]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_spelling_ok() {
        let diags = check_spelling(
            r#"
msgid ""
msgstr "Language: fr\n"

msgctxt "some context"
msgid "tested: HTTP v3"
msgstr "testé : HTTP v3"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_spelling_error_noqa() {
        let diags = check_spelling(
            r#"
msgid ""
msgstr "Language: fr\n"

#, noqa:spelling-ctxt;spelling-id;spelling-str
msgctxt "some contxet, some contxet"
msgid "this is a tyypo, this is a tyypo"
msgstr "ceci est unz fôte, ceci est unz fôte"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_spelling_error() {
        let diags = check_spelling(
            r#"
msgid ""
msgstr "Language: fr\n"

msgctxt "some contxet, some contxet"
msgid "this is a tyypo, this is a tyypo"
msgstr "ceci est unz fôte, ceci est unz fôte"
"#,
        );
        assert_eq!(diags.len(), 3);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "misspelled words in context: contxet");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "misspelled words in source: tyypo");
        let diag = &diags[2];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "misspelled words in translation: fôte, unz");
    }
}
