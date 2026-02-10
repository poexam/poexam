// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `encoding` rule: check incorrect encoding.

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct EncodingRule {}

impl RuleChecker for EncodingRule {
    fn name(&self) -> &'static str {
        "encoding"
    }

    fn is_default(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    /// Check for translation with incorrect encoding.
    ///
    /// The encoding used to check is the one declared in the PO file, with a fallback
    /// to UTF-8 if not specified, example:
    /// ```text
    /// "Content-Type: text/plain; charset=UTF-8\n"
    /// ```
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "tested"
    /// msgstr "test�"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "tested"
    /// msgstr "testé"
    /// ```
    ///
    /// Diagnostics reported with severity [`info`](Severity::Info):
    /// - `invalid characters for encoding xxx`
    fn check_entry(&self, checker: &mut Checker, entry: &Entry) {
        if entry.encoding_error {
            checker.report_entry(
                format!(
                    "invalid characters for encoding {}",
                    checker.encoding_name()
                ),
                entry,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_encoding(content: &str) -> Vec<Diagnostic> {
        let rules = Rules::new(vec![Box::new(EncodingRule {})]);
        let mut checker = Checker::new(content.as_bytes(), &rules);
        checker.do_all_checks();
        checker.diagnostics
    }

    #[test]
    fn test_encoding_ok() {
        let diags = check_encoding(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_encoding_error() {
        let rules = Rules::new(vec![Box::new(EncodingRule {})]);
        let mut checker = Checker::new(b"msgid \"tested\"\nmsgstr \"test\xe9\"\n", &rules);
        checker.do_all_checks();
        let diags = checker.diagnostics;
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "invalid characters for encoding UTF-8");
    }
}
