// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `blank` rule: check blank translation.

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct BlankRule;

impl RuleChecker for BlankRule {
    fn name(&self) -> &'static str {
        "blank"
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
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
    fn check_msg(
        &self,
        checker: &Checker,
        _entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        if !msgid.value.trim().is_empty()
            && !msgstr.value.is_empty()
            && msgstr.value.trim().is_empty()
        {
            vec![
                self.new_diag(checker, "blank translation".to_string())
                    .with_msgs_hl(msgid, &[], msgstr, &[(0, msgstr.value.len())]),
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

    fn check_blank(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(BlankRule {})]);
        checker.do_all_checks(&rules);
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
    fn test_blank_error_noqa() {
        let diags = check_blank(
            r#"
#, noqa:blank
msgid "tested"
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
