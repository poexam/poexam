// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `pipes` rule: check missing/extra pipes.

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct PipesRule;

impl RuleChecker for PipesRule {
    fn name(&self) -> &'static str {
        "pipes"
    }

    fn is_default(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    /// Check for missing or extra pipes in the translation.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "syntax: ./test -f|-h|-v"
    /// msgstr "syntaxe : ./test -f|-h"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "syntax: ./test -f|-h|-v"
    /// msgstr "syntaxe : ./test -f|-h|-v"
    /// ```
    ///
    /// Diagnostics reported with severity [`info`](Severity::Info):
    /// - `missing pipes '|' (# / #)`
    /// - `extra pipes '|' (# / #)`
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        let id_pipes: Vec<_> = msgid
            .match_indices('|')
            .map(|(idx, value)| (idx, idx + value.len()))
            .collect();
        let str_pipes: Vec<_> = msgstr
            .match_indices('|')
            .map(|(idx, value)| (idx, idx + value.len()))
            .collect();
        match id_pipes.len().cmp(&str_pipes.len()) {
            std::cmp::Ordering::Greater => {
                checker.report_msg(
                    entry,
                    format!(
                        "missing pipes '|' ({} / {})",
                        id_pipes.len(),
                        str_pipes.len()
                    ),
                    msgid,
                    &id_pipes,
                    msgstr,
                    &str_pipes,
                );
            }
            std::cmp::Ordering::Less => {
                checker.report_msg(
                    entry,
                    format!("extra pipes '|' ({} / {})", id_pipes.len(), str_pipes.len()),
                    msgid,
                    &id_pipes,
                    msgstr,
                    &str_pipes,
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

    fn check_pipes(content: &str) -> Vec<Diagnostic> {
        let rules = Rules::new(vec![Box::new(PipesRule {})]);
        let mut checker = Checker::new(content.as_bytes(), &rules);
        checker.do_all_checks();
        checker.diagnostics
    }

    #[test]
    fn test_no_pipes() {
        let diags = check_pipes(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_pipes_ok() {
        let diags = check_pipes(
            r#"
msgid "tested|here"
msgstr "testé|ici"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_pipes_error() {
        let diags = check_pipes(
            r#"
msgid "tested|"
msgstr "testé"

msgid "tested"
msgstr "testé|"
"#,
        );
        assert_eq!(diags.len(), 2);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "missing pipes '|' (1 / 0)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "extra pipes '|' (0 / 1)");
    }
}
