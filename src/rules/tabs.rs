// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `tabs` rule: check inconsistent tabs.

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct TabsRule {}

impl RuleChecker for TabsRule {
    fn name(&self) -> &'static str {
        "tabs"
    }

    fn is_default(&self) -> bool {
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
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        let id_tabs: Vec<_> = msgid
            .match_indices('\t')
            .map(|(idx, value)| (idx, idx + value.len()))
            .collect();
        let str_tabs: Vec<_> = msgstr
            .match_indices('\t')
            .map(|(idx, value)| (idx, idx + value.len()))
            .collect();
        match id_tabs.len().cmp(&str_tabs.len()) {
            std::cmp::Ordering::Greater => {
                checker.report_msg(
                    entry,
                    format!(
                        "missing tabs '\\t' ({} / {})",
                        id_tabs.len(),
                        str_tabs.len()
                    ),
                    msgid,
                    &id_tabs,
                    msgstr,
                    &str_tabs,
                );
            }
            std::cmp::Ordering::Less => {
                checker.report_msg(
                    entry,
                    format!("extra tabs '\\t' ({} / {})", id_tabs.len(), str_tabs.len()),
                    msgid,
                    &id_tabs,
                    msgstr,
                    &str_tabs,
                );
            }
            std::cmp::Ordering::Equal => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_tabs(content: &str) -> Vec<Diagnostic> {
        let rules = Rules::new(vec![Box::new(TabsRule {})]);
        let mut checker = Checker::new(content.as_bytes(), &rules);
        checker.do_all_checks();
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
