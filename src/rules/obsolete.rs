// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct ObsoleteRule {}

impl RuleChecker for ObsoleteRule {
    fn name(&self) -> &'static str {
        "obsolete"
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

    /// Check for obsolete entry.
    ///
    /// Obsolete is not strictly speaking an error, but this check helps to identify
    /// obsolete entries in a PO file.
    ///
    /// This check is not enabled by default.
    ///
    /// Reported:
    /// ```text
    /// #~ msgid "this is a test"
    /// #~ msgstr "ceci est un test"
    /// ```
    ///
    /// Not reported:
    /// ```text
    /// msgid "this is a test"
    /// msgstr "ceci est un test"
    /// ```
    ///
    /// Diagnostics reported with severity [`info`](Severity::Info):
    /// - `obsolete entry`
    fn check_entry(&self, checker: &mut Checker, entry: &Entry) {
        if entry.obsolete {
            checker.report_entry("obsolete entry".to_string(), entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_obsolete(content: &str) -> Vec<Diagnostic> {
        let rules = Rules::new(vec![Box::new(ObsoleteRule {})]);
        let mut checker = Checker::new(content.as_bytes(), &rules);
        checker.do_all_checks();
        checker.diagnostics
    }

    #[test]
    fn test_not_obsolete() {
        let diags = check_obsolete(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_obsolete_error() {
        let diags = check_obsolete(
            r#"
#~ msgid "tested"
#~ msgstr "testé"
"#,
        );
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "obsolete entry");
    }
}
