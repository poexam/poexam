// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `tabs` rule: check inconsistent tabs.

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct TabsRule;

impl RuleChecker for TabsRule {
    fn name(&self) -> &'static str {
        "tabs"
    }

    fn description(&self) -> &'static str {
        "Check for missing or extra tab characters in translation."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    /// Check for missing or extra tabs (`\t`) in the translation.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "test \t (tab)"
    /// msgstr "test   (tab)"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "test \t (tab)"
    /// msgstr "test \t (tab)"
    /// ```
    ///
    /// Diagnostics reported with severity [`error`](Severity::Error):
    /// - `missing tabs '\t' (# / #)`
    /// - `extra tabs '\t' (# / #)`
    fn check_msg(
        &self,
        checker: &Checker,
        _entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let id_count = msgid.value.matches('\t').count();
        let str_count = msgstr.value.matches('\t').count();
        let msg = match id_count.cmp(&str_count) {
            std::cmp::Ordering::Equal => return vec![],
            std::cmp::Ordering::Greater => {
                format!("missing tabs '\\t' ({id_count} / {str_count})")
            }
            std::cmp::Ordering::Less => {
                format!("extra tabs '\\t' ({id_count} / {str_count})")
            }
        };
        vec![
            self.new_diag(checker, msg).with_msgs_hl(
                msgid,
                msgid
                    .value
                    .match_indices('\t')
                    .map(|(idx, value)| (idx, idx + value.len())),
                msgstr,
                msgstr
                    .value
                    .match_indices('\t')
                    .map(|(idx, value)| (idx, idx + value.len())),
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_tabs(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(TabsRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_no_tabs() {
        let diags = check_tabs(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_tabs_ok() {
        let diags = check_tabs(
            r#"
msgid "\ttested\tline 2\t"
msgstr "\ttesté\tligne 2\t"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_tabs_error_noqa() {
        let diags = check_tabs(
            r#"
msgid "tested\tline 2"
msgstr "testé ligne 2"

msgid "tested line 2"
msgstr "testé\tligne 2"
"#,
        );
        assert_eq!(diags.len(), 2);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "missing tabs '\\t' (1 / 0)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "extra tabs '\\t' (0 / 1)");
    }

    #[test]
    fn test_tabs_error() {
        let diags = check_tabs(
            r#"
msgid "tested\tline 2"
msgstr "testé ligne 2"

msgid "tested line 2"
msgstr "testé\tligne 2"
"#,
        );
        assert_eq!(diags.len(), 2);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "missing tabs '\\t' (1 / 0)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "extra tabs '\\t' (0 / 1)");
    }
}
