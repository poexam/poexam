// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct UnchangedRule {}

impl RuleChecker for UnchangedRule {
    fn name(&self) -> &'static str {
        "unchanged"
    }

    fn is_default(&self) -> bool {
        false
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    /// Check for unchanged translation: the same as the source string.
    ///
    /// If the source message contains only upper case characters, it is ignored.
    ///
    /// This check is not enabled by default.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "this is a test"
    /// msgstr "this is a test"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "this is a test"
    /// msgstr "ceci est un test"
    /// ```
    ///
    /// Diagnostics reported with severity [`info`](Severity::Info):
    /// - `unchanged translation`
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        if !msgid.trim().is_empty() && !msgstr.trim().is_empty() && msgstr == msgid {
            let all_upper = msgid
                .chars()
                .filter(|c| c.is_alphabetic())
                .all(char::is_uppercase);
            if !all_upper && msgid.to_uppercase() != msgid {
                checker.report_msg(
                    entry,
                    "unchanged translation".to_string(),
                    msgid.to_string(),
                    msgstr.to_string(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_blank(content: &str) -> Vec<Diagnostic> {
        let rules = Rules::new(vec![Box::new(UnchangedRule {})]);
        let mut checker = Checker::new(content.as_bytes(), &rules);
        checker.do_all_checks();
        checker.diagnostics
    }

    #[test]
    fn test_not_translated() {
        let diags = check_blank(
            r#"
msgid "tested"
msgstr ""
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_changed() {
        let diags = check_blank(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_unchanged_but_ok() {
        // Unchanged but considered ok (only one word).
        let diags = check_blank(
            r#"
msgid "ACRONYM"
msgstr "ACRONYM"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_unchanged_error() {
        let diags = check_blank(
            r#"
msgid "this is a test"
msgstr "this is a test"
"#,
        );
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "unchanged translation");
    }
}
