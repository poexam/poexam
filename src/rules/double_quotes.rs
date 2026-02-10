// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `double-quotes` rule: check missing/extra double quotes.

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

const DOUBLE_QUOTES: [char; 3] = ['"', '„', '”'];

pub struct DoubleQuotesRule {}

impl RuleChecker for DoubleQuotesRule {
    fn name(&self) -> &'static str {
        "double-quotes"
    }

    fn is_default(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    /// Check for missing or extra double quotes (`"`, `„` and `”`) in the translation.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "this is a \"test\""
    /// msgstr "ceci est un test"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "this is a \"test\""
    /// msgstr "ceci est un \"test\""
    /// ```
    ///
    /// Diagnostics reported with severity [`info`](Severity::Info):
    /// - `missing double quotes (# / #)`
    /// - `extra double quotes (# / #)`
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        let id_quotes: Vec<_> = msgid
            .match_indices(DOUBLE_QUOTES)
            .map(|(idx, value)| (idx, idx + value.len()))
            .collect();
        let str_quotes: Vec<_> = msgstr
            .match_indices(DOUBLE_QUOTES)
            .map(|(idx, value)| (idx, idx + value.len()))
            .collect();
        match id_quotes.len().cmp(&str_quotes.len()) {
            std::cmp::Ordering::Greater => {
                checker.report_msg(
                    entry,
                    format!(
                        "missing double quotes ({} / {})",
                        id_quotes.len(),
                        str_quotes.len()
                    ),
                    msgid,
                    &id_quotes,
                    msgstr,
                    &str_quotes,
                );
            }
            std::cmp::Ordering::Less => {
                checker.report_msg(
                    entry,
                    format!(
                        "extra double quotes ({} / {})",
                        id_quotes.len(),
                        str_quotes.len()
                    ),
                    msgid,
                    &id_quotes,
                    msgstr,
                    &str_quotes,
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

    fn check_double_quotes(content: &str) -> Vec<Diagnostic> {
        let rules = Rules::new(vec![Box::new(DoubleQuotesRule {})]);
        let mut checker = Checker::new(content.as_bytes(), &rules);
        checker.do_all_checks();
        checker.diagnostics
    }

    #[test]
    fn test_no_double_quotes() {
        let diags = check_double_quotes(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_double_quotes_ok() {
        let diags = check_double_quotes(
            r#"
msgid "this is a \"test\""
msgstr "ceci est un \"test\""
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_double_quotes_error() {
        let diags = check_double_quotes(
            r#"
msgid "this is a \"test\""
msgstr "ceci est un test"

msgid "this is a test"
msgstr "ceci est un \"test\""
"#,
        );
        assert_eq!(diags.len(), 2);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "missing double quotes (2 / 0)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "extra double quotes (0 / 2)");
    }
}
