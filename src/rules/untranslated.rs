// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct UntranslatedRule {}

impl RuleChecker for UntranslatedRule {
    fn name(&self) -> &'static str {
        "untranslated"
    }

    fn is_default(&self) -> bool {
        false
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    /// Check for untranslated entry.
    ///
    /// Untranslated is not stricly speaking an error, but this check helps to identify
    /// untranslated entries in a PO file.
    ///
    /// This check is not enabled by default.
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
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        if msgstr.is_empty() {
            checker.report_msg(
                entry,
                "untranslated message".to_string(),
                msgid.to_string(),
                msgstr.to_string(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_untranslated(content: &str) -> Vec<Diagnostic> {
        let rules = Rules::new(vec![Box::new(UntranslatedRule {})]);
        let mut checker = Checker::new(content.as_bytes(), &rules);
        checker.do_all_checks();
        checker.diagnostics
    }

    #[test]
    fn test_not_fuzzy() {
        let diags = check_untranslated(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_fuzzy_error() {
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
