// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `untranslated` rule: report untranslated entries.

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct UntranslatedRule;

impl RuleChecker for UntranslatedRule {
    fn name(&self) -> &'static str {
        "untranslated"
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

    /// Report entry if untranslated.
    ///
    /// Untranslated is not strictly speaking an error, but this check helps to identify
    /// untranslated entries in a PO file.
    ///
    /// This rule is not enabled by default.
    ///
    /// Reported:
    /// ```text
    /// msgid "this is a test"
    /// msgstr ""
    /// ```
    ///
    /// Not reported:
    /// ```text
    /// msgid "this is a test"
    /// msgstr "ceci est un test"
    /// ```
    ///
    /// Diagnostics reported with severity [`info`](Severity::Info):
    /// - `untranslated message`
    fn check_msg(
        &self,
        checker: &Checker,
        _entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        if msgstr.value.is_empty() {
            vec![checker.new_diag("untranslated message").with_msg(msgid)]
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_untranslated(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(UntranslatedRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_translated() {
        let diags = check_untranslated(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_untranslated_error_noqa() {
        let diags = check_untranslated(
            r#"
#, noqa:untranslated
msgid "tested"
msgstr ""
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_untranslated_error() {
        let diags = check_untranslated(
            r#"
msgid "tested"
msgstr ""
"#,
        );
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "untranslated message");
    }
}
