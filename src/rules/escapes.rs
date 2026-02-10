// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `escapes` rule: check missing/extra escape characters.

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct EscapesRule {}

impl RuleChecker for EscapesRule {
    fn name(&self) -> &'static str {
        "escapes"
    }

    fn is_default(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    /// Check for missing or extra escape characters (`\\` and `\`) in the translation.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "this is a \"test\""
    /// msgstr "ceci est un \\\"test\\\""
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "this is a \"test\""
    /// msgstr "ceci est un \"test\""
    /// ```
    ///
    /// Diagnostics reported with severity [`error`](Severity::Error):
    /// - `missing escaped escape characters '\\' (# / #)`
    /// - `extra escaped escape characters '\\' (# / #)`
    /// - `missing escaped escape characters '\' (# / #)`
    /// - `extra escaped escape characters '\' (# / #)`
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        let id_esc: Vec<_> = msgid
            .match_indices("\\\\")
            .map(|(idx, value)| (idx, idx + value.len()))
            .collect();
        let str_esc: Vec<_> = msgstr
            .match_indices("\\\\")
            .map(|(idx, value)| (idx, idx + value.len()))
            .collect();
        match id_esc.len().cmp(&str_esc.len()) {
            std::cmp::Ordering::Greater => {
                checker.report_msg(
                    entry,
                    format!(
                        "missing escaped escape characters '\\\\' ({} / {})",
                        id_esc.len(),
                        str_esc.len()
                    ),
                    msgid,
                    &id_esc,
                    msgstr,
                    &str_esc,
                );
            }
            std::cmp::Ordering::Less => {
                checker.report_msg(
                    entry,
                    format!(
                        "extra escaped escape characters '\\\\' ({} / {})",
                        id_esc.len(),
                        str_esc.len()
                    ),
                    msgid,
                    &id_esc,
                    msgstr,
                    &str_esc,
                );
            }
            std::cmp::Ordering::Equal => {
                let id_esc: Vec<_> = msgid
                    .match_indices('\\')
                    .map(|(idx, value)| (idx, idx + value.len()))
                    .collect();
                let str_esc: Vec<_> = msgstr
                    .match_indices('\\')
                    .map(|(idx, value)| (idx, idx + value.len()))
                    .collect();
                match id_esc.len().cmp(&str_esc.len()) {
                    std::cmp::Ordering::Greater => {
                        checker.report_msg(
                            entry,
                            format!(
                                "missing escape characters '\\' ({} / {})",
                                id_esc.len(),
                                str_esc.len()
                            ),
                            msgid,
                            &id_esc,
                            msgstr,
                            &str_esc,
                        );
                    }
                    std::cmp::Ordering::Less => {
                        checker.report_msg(
                            entry,
                            format!(
                                "extra escape characters '\\' ({} / {})",
                                id_esc.len(),
                                str_esc.len()
                            ),
                            msgid,
                            &id_esc,
                            msgstr,
                            &str_esc,
                        );
                    }
                    std::cmp::Ordering::Equal => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_escapes(content: &str) -> Vec<Diagnostic> {
        let rules = Rules::new(vec![Box::new(EscapesRule {})]);
        let mut checker = Checker::new(content.as_bytes(), &rules);
        checker.do_all_checks();
        checker.diagnostics
    }

    #[test]
    fn test_no_escapes() {
        let diags = check_escapes(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_escapes_ok() {
        let diags = check_escapes(
            r#"
msgid "tested: \ \n "
msgstr "testé : \ \n "
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_escaped_escape_char_error() {
        let diags = check_escapes(
            r#"
msgid "tested\\\\"
msgstr "testé"

msgid "tested"
msgstr "testé\\\\"
"#,
        );
        assert_eq!(diags.len(), 2);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(
            diag.message,
            "missing escaped escape characters '\\\\' (1 / 0)"
        );
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(
            diag.message,
            "extra escaped escape characters '\\\\' (0 / 1)"
        );
    }

    #[test]
    fn test_escaped_char_error() {
        let diags = check_escapes(
            r#"
msgid "tested\\"
msgstr "testé"

msgid "tested"
msgstr "testé\\"
"#,
        );
        assert_eq!(diags.len(), 2);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "missing escape characters '\\' (1 / 0)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "extra escape characters '\\' (0 / 1)");
    }
}
