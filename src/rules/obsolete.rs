// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `obsolete` rule: report obsolete entries.

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct ObsoleteRule;

impl RuleChecker for ObsoleteRule {
    fn name(&self) -> &'static str {
        "obsolete"
    }

    fn description(&self) -> &'static str {
        "Report obsolete entries."
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

    /// Report entry if obsolete.
    ///
    /// Obsolete is not strictly speaking an error, but this check helps to identify
    /// obsolete entries in a PO file.
    ///
    /// This rule is not enabled by default.
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
    fn check_entry(&self, checker: &Checker, entry: &Entry) -> Vec<Diagnostic> {
        if entry.obsolete {
            vec![
                self.new_diag(checker, "obsolete entry".to_string())
                    .with_entry(entry),
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

    fn check_obsolete(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(ObsoleteRule {})]);
        checker.do_all_checks(&rules);
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
    fn test_obsolete_error_noqa() {
        let diags = check_obsolete(
            r#"
#, noqa:obsolete
#~ msgid "tested"
#~ msgstr "testé"
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
