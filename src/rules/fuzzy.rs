// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct FuzzyRule {}

impl RuleChecker for FuzzyRule {
    fn name(&self) -> &'static str {
        "fuzzy"
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

    /// Check for fuzzy entry.
    ///
    /// Fuzzy is not strictly speaking an error, but this check helps to identify fuzzy
    /// entries in a PO file.
    ///
    /// This check is not enabled by default.
    ///
    /// Reported:
    /// ```text
    /// #, fuzzy
    /// msgid "this is a test"
    /// msgstr "mauvaise traduction"
    /// ```
    ///
    /// Not reported:
    /// ```text
    /// msgid "this is a test"
    /// msgstr "ceci est un test"
    /// ```
    ///
    /// Diagnostics reportedwith severity [`info`](Severity::Info):
    /// - `fuzzy entry`
    fn check_entry(&self, checker: &mut Checker, entry: &Entry) {
        if entry.fuzzy {
            checker.report_entry("fuzzy entry".to_string(), entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_fuzzy(content: &str) -> Vec<Diagnostic> {
        let rules = Rules::new(vec![Box::new(FuzzyRule {})]);
        let mut checker = Checker::new(content.as_bytes(), &rules);
        checker.do_all_checks();
        checker.diagnostics
    }

    #[test]
    fn test_not_fuzzy() {
        let diags = check_fuzzy(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_fuzzy_error() {
        let diags = check_fuzzy(
            r#"
#, fuzzy
msgid "tested"
msgstr "mauvaise traduction"
"#,
        );
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "fuzzy entry");
    }
}
