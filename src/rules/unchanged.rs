// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `unchanged` rule: check unchanged translations.

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct UnchangedRule;

impl RuleChecker for UnchangedRule {
    fn name(&self) -> &'static str {
        "unchanged"
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

    /// Check for unchanged translation: the same as the source string.
    ///
    /// If the source message contains only upper case characters, it is ignored.
    ///
    /// This rule is not enabled by default.
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
    fn check_msg(
        &self,
        checker: &Checker,
        _entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        if !msgid.value.trim().is_empty()
            && !msgstr.value.trim().is_empty()
            && msgid.value == msgstr.value
        {
            let all_upper = msgid
                .value
                .chars()
                .filter(|c| c.is_alphabetic())
                .all(char::is_uppercase);
            if !all_upper && msgid.value.to_uppercase() != msgid.value {
                return vec![
                    self.new_diag(checker, "unchanged translation".to_string())
                        .with_msgs(msgid, msgstr),
                ];
            }
        }
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_unchanged(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(UnchangedRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_not_translated() {
        let diags = check_unchanged(
            r#"
msgid "tested"
msgstr ""
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_changed() {
        let diags = check_unchanged(
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
        let diags = check_unchanged(
            r#"
msgid "ACRONYM"
msgstr "ACRONYM"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_unchanged_error_noqa() {
        let diags = check_unchanged(
            r#"
#, noqa:unchanged
msgid "this is a test"
msgstr "this is a test"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_unchanged_error() {
        let diags = check_unchanged(
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
