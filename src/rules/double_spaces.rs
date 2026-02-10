// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `double-spaces` rule: check missing/extra double spaces.

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct DoubleSpacesRule {}

impl RuleChecker for DoubleSpacesRule {
    fn name(&self) -> &'static str {
        "double-spaces"
    }

    fn is_default(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    /// Check for missing or extra double spaces in the translation.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "the test:  \"xyz\""
    /// msgstr "le test : \"xyz\""
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "the test:  \"xyz\""
    /// msgstr "le test :  \"xyz\""
    /// ```
    ///
    /// Diagnostics reported with severity [`info`](Severity::Info):
    /// - `missing double spaces '  ' (# / #)`
    /// - `extra double spaces '  ' (# / #)`
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        let id_quotes: Vec<_> = msgid
            .match_indices("  ")
            .map(|(idx, value)| (idx, idx + value.len()))
            .collect();
        let str_quotes: Vec<_> = msgstr
            .match_indices("  ")
            .map(|(idx, value)| (idx, idx + value.len()))
            .collect();
        match id_quotes.len().cmp(&str_quotes.len()) {
            std::cmp::Ordering::Greater => {
                checker.report_msg(
                    entry,
                    format!(
                        "missing double spaces '  ' ({} / {})",
                        id_quotes.len(),
                        str_quotes.len()
                    ),
                    msgid,
                    &id_quotes,
                    msgstr,
                    &str_quotes,
                );
            }
            std::cmp::Ordering::Less => {
                checker.report_msg(
                    entry,
                    format!(
                        "extra double spaces '  ' ({} / {})",
                        id_quotes.len(),
                        str_quotes.len()
                    ),
                    msgid,
                    &id_quotes,
                    msgstr,
                    &str_quotes,
                );
            }
            std::cmp::Ordering::Equal => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_double_spaces(content: &str) -> Vec<Diagnostic> {
        let rules = Rules::new(vec![Box::new(DoubleSpacesRule {})]);
        let mut checker = Checker::new(content.as_bytes(), &rules);
        checker.do_all_checks();
        checker.diagnostics
    }

    #[test]
    fn test_no_double_spaces() {
        let diags = check_double_spaces(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_double_spaces_ok() {
        let diags = check_double_spaces(
            r#"
msgid "this  is  a  test"
msgstr "ceci  est  un  test"
"#,
        );
        // Note: leading and trailing and double spaces are ignored here
        // (such errors are reported in the "whitespace" checks).
        assert!(diags.is_empty());
    }

    #[test]
    fn test_double_spaces_error() {
        let diags = check_double_spaces(
            r#"
msgid "this is a  test"
msgstr "ceci est un test"

msgid "this is a test"
msgstr "ceci est un  test"
"#,
        );
        assert_eq!(diags.len(), 2);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "missing double spaces '  ' (1 / 0)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "extra double spaces '  ' (0 / 1)");
    }
}
