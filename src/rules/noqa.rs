// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `noqa` rule: report entries with `noqa` comments.

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct NoqaRule;

impl RuleChecker for NoqaRule {
    fn name(&self) -> &'static str {
        "noqa"
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

    /// Report entry if it has a `noqa` comment.
    ///
    /// This rule is not enabled by default.
    ///
    /// Reported:
    /// ```text
    /// #, noqa:url
    /// msgid "this is a translated URL: https://example.com/about"
    /// msgstr "ceci est une URL traduite : https://example.com/à_propos"
    /// ```
    ///
    /// Not reported:
    /// ```text
    /// msgid "this is a translated URL: https://example.com/about"
    /// msgstr "ceci est une URL traduite : https://example.com/à_propos"
    /// ```
    ///
    /// Diagnostics reported with severity [`info`](Severity::Info):
    /// - `entry with noqa`
    fn check_entry(&self, checker: &mut Checker, entry: &Entry) {
        if entry.noqa || !entry.noqa_rules.is_empty() {
            checker.report_entry("entry with noqa".to_string(), entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_noqa(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(NoqaRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_no_noqa() {
        let diags = check_noqa(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_noqa() {
        let diags = check_noqa(
            r#"
#, noqa
msgid "tested"
msgstr "testé  "

#, noqa:whiteqpace
msgid "tested"
msgstr "testé  "
"#,
        );
        assert_eq!(diags.len(), 2);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "entry with noqa");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "entry with noqa");
    }
}
