// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::highlight::HighlightExt;
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

const DOUBLE_QUOTES: [char; 3] = ['"', '„', '”'];
const DOUBLE_QUOTES_STR: [&str; 3] = ["\"", "„", "”"];

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
        let id_quotes = msgid.matches(DOUBLE_QUOTES).count();
        let str_quotes = msgstr.matches(&DOUBLE_QUOTES).count();
        match id_quotes.cmp(&str_quotes) {
            std::cmp::Ordering::Greater => {
                checker.report_msg(
                    entry,
                    format!("missing double quotes ({id_quotes} / {str_quotes})"),
                    msgid.highlight_list_str(&DOUBLE_QUOTES_STR),
                    msgstr.highlight_list_str(&DOUBLE_QUOTES_STR),
                );
            }
            std::cmp::Ordering::Less => {
                checker.report_msg(
                    entry,
                    format!("extra double quotes ({id_quotes} / {str_quotes})"),
                    msgid.highlight_list_str(&DOUBLE_QUOTES_STR),
                    msgstr.highlight_list_str(&DOUBLE_QUOTES_STR),
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
