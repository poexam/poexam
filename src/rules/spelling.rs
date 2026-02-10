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
use crate::rules::rule::RuleChecker;
use crate::words::WordPos;

pub struct SpellingCtxtRule {}

impl RuleChecker for SpellingCtxtRule {
    fn name(&self) -> &'static str {
        "spelling-ctxt"
    }

    fn is_default(&self) -> bool {
        false
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
    /// Diagnostics reported with severity [`warning`](Severity::Info):
    /// - `misspelled words in context: xxx`
    fn check_ctxt(&self, checker: &mut Checker, entry: &Entry, msgctxt: &str) {
        if let Some(dict) = &checker.dict_id {
            let (misspelled_words, pos_words) = check_words(entry, msgctxt, dict);
            if !misspelled_words.is_empty() {
                checker.report_ctxt(
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

pub struct SpellingIdRule {}

impl RuleChecker for SpellingIdRule {
    fn name(&self) -> &'static str {
        "spelling-id"
    }

    fn is_default(&self) -> bool {
        false
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
    /// Diagnostics reported with severity [`warning`](Severity::Info):
    /// - `misspelled words in source: xxx`
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        if let Some(dict) = &checker.dict_id {
            let (misspelled_words, pos_words) = check_words(entry, msgid, dict);
            if !misspelled_words.is_empty() {
                checker.report_msg(
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

pub struct SpellingStrRule {}

impl RuleChecker for SpellingStrRule {
    fn name(&self) -> &'static str {
        "spelling-str"
    }

    fn is_default(&self) -> bool {
        false
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
    /// Diagnostics reported with severity [`warning`](Severity::Info):
    /// - `misspelled words in translation: xxx`
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        if let Some(dict) = &checker.dict_str {
            let (misspelled_words, pos_words) = check_words(entry, msgstr, dict);
            if !misspelled_words.is_empty() {
                checker.report_msg(
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
    entry: &Entry,
    s: &'s str,
    dict: &Dictionary,
) -> (Vec<&'s str>, Vec<(usize, usize)>) {
    let mut misspelled_words: HashSet<&str> = HashSet::new();
    let mut hash_words: HashSet<&str> = HashSet::new();
    let mut pos_words = Vec::new();
    for (start, end) in WordPos::new(s, &entry.format) {
        let word = &s[start..end];
        if hash_words.contains(word) {
            if misspelled_words.contains(word) {
                pos_words.push((start, end));
            }
        } else {
            hash_words.insert(word);
            if !dict.check(word) {
                misspelled_words.insert(word);
                pos_words.push((start, end));
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
    use crate::{
        args::DEFAULT_LANG_ID, diagnostic::Diagnostic, dict::get_dict, rules::rule::Rules,
    };

    fn check_spelling(content: &str) -> Vec<Diagnostic> {
        let rules = Rules::new(vec![
            Box::new(SpellingCtxtRule {}),
            Box::new(SpellingIdRule {}),
            Box::new(SpellingStrRule {}),
        ]);
        let mut test_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        test_dir.push("resources/test");
        let dict_id = get_dict(test_dir.as_path(), None, DEFAULT_LANG_ID).unwrap();
        let mut checker = Checker::new(content.as_bytes(), &rules)
            .with_path_dicts(test_dir.as_path())
            .with_dict_id(Some(&dict_id));
        checker.do_all_checks();
        checker.diagnostics
    }

    #[test]
    fn test_spelling_ok() {
        let diags = check_spelling(
            r#"
msgid ""
msgstr "Language: fr\n"

msgctxt "some context"
msgid "tested"
msgstr "testé"
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
