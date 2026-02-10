// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `blank` rule: check blank translation.

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct BlankRule {}

impl RuleChecker for BlankRule {
    fn name(&self) -> &'static str {
        "blank"
    }

    fn is_default(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    /// Check for blank translation (only whitespace).
    ///
    /// As the translation is not empty, it is used and it does not contain the appropriate
    /// translated text.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "this is a test"
    /// msgstr " "
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "this is a test"
    /// msgstr "ceci est un test"
    /// ```
    ///
    /// Diagnostics reported with severity [`warning`](Severity::Warning):
    /// - `blank translation`
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        if !msgid.trim().is_empty() && !msgstr.is_empty() && msgstr.trim().is_empty() {
            checker.report_msg(
                entry,
                "blank translation".to_string(),
                msgid,
                &[],
                msgstr,
                &[(0, msgstr.len())],
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_blank(content: &str) -> Vec<Diagnostic> {
        let rules = Rules::new(vec![Box::new(BlankRule {})]);
        let mut checker = Checker::new(content.as_bytes(), &rules);
        checker.do_all_checks();
        checker.diagnostics
    }

    #[test]
    fn test_no_blank() {
        let diags = check_blank(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_blank_id_and_str_ok() {
        let diags = check_blank(
            r#"
msgid "  "
msgstr "  "
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_blank_error() {
        let diags = check_blank(
            r#"
msgid "tested"
msgstr "  "
"#,
        );
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "blank translation");
    }
}
