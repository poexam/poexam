// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `changed` rule: check changed translations.

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct ChangedRule {}

impl RuleChecker for ChangedRule {
    fn name(&self) -> &'static str {
        "changed"
    }

    fn is_default(&self) -> bool {
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
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        if !msgid.trim().is_empty() && !msgstr.trim().is_empty() && msgstr != msgid {
            checker.report_msg(
                entry,
                "changed translation".to_string(),
                msgid,
                &[],
                msgstr,
                &[],
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_changed(content: &str) -> Vec<Diagnostic> {
        let rules = Rules::new(vec![Box::new(ChangedRule {})]);
        let mut checker = Checker::new(content.as_bytes(), &rules);
        checker.do_all_checks();
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
