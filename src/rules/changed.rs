// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `changed` rule: check changed translations.

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct ChangedRule;

impl RuleChecker for ChangedRule {
    fn name(&self) -> &'static str {
        "changed"
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        false
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    /// Check for changed translation: the translation is not empty and different from
    /// the source string.
    ///
    /// This rule can be used in rare cases, for example if you use a file `en.po` which
    /// contains English translations (like the sources) with some typos fixed.
    /// So you can then identify which source strings must be fixed in the source code.
    ///
    /// This rule is not enabled by default.
    ///
    /// Reported:
    /// ```text
    /// msgid "this is a test (example
    /// msgstr "this is a test (example)"
    /// ```
    ///
    /// Not reported:
    /// ```text
    /// msgid "this is a test (example)"
    /// msgstr "this is a test (example)"
    /// ```
    ///
    /// Diagnostics reported with severity [`info`](Severity::Info):
    /// - `changed translation`
    fn check_msg(
        &self,
        checker: &Checker,
        _entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        if !msgid.value.trim().is_empty()
            && !msgstr.value.trim().is_empty()
            && msgstr.value != msgid.value
        {
            vec![
                self.new_diag(checker, "changed translation".to_string())
                    .with_msgs(msgid, msgstr),
            ]
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_changed(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(ChangedRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_not_translated() {
        let diags = check_changed(
            r#"
msgid "tested"
msgstr ""
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_unchanged() {
        let diags = check_changed(
            r#"
msgid "tested"
msgstr "tested"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_changed_error_noqa() {
        let diags = check_changed(
            r#"
#, noqa:changed
msgid "this is a test (example"
msgstr "this is a test (example)"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_changed_error() {
        let diags = check_changed(
            r#"
msgid "this is a test (example"
msgstr "this is a test (example)"
"#,
        );
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "changed translation");
    }
}
