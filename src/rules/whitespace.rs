// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the whitespace rules: check inconsistent whitespace:
//! - `whitespace-start`: whitespace at the beginning of the string
//! - `whitespace-end`: whitespace at the end of the string

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct WhitespaceStartRule;

impl RuleChecker for WhitespaceStartRule {
    fn name(&self) -> &'static str {
        "whitespace-start"
    }

    fn is_default(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    /// Check for inconsistent leading whitespace between source and translation.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid " this is a test"
    /// msgstr "ceci est un test"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid " this is a test"
    /// msgstr " ceci est un test"
    /// ```
    ///
    /// Diagnostics reported with severity [`info`](Severity::Info):
    /// - `inconsistent leading whitespace ('…' / '…')`
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        if msgid.trim().is_empty() || msgstr.trim().is_empty() {
            return;
        }
        let id_ws = get_whitespace_start(msgid);
        let str_ws = get_whitespace_start(msgstr);
        if id_ws != str_ws {
            checker.report_msg(
                entry,
                format!("inconsistent leading whitespace ('{id_ws}' / '{str_ws}')"),
                msgid,
                &[(0, id_ws.len())],
                msgstr,
                &[(0, str_ws.len())],
            );
        }
    }
}

pub struct WhitespaceEndRule;

impl RuleChecker for WhitespaceEndRule {
    fn name(&self) -> &'static str {
        "whitespace-end"
    }

    fn is_default(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    /// Check for inconsistent trailing whitespace between source and translation.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "this is a test "
    /// msgstr "ceci est un test"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "this is a test "
    /// msgstr "ceci est un test "
    /// ```
    ///
    /// Diagnostics reported with severity [`info`](Severity::Info):
    /// - `inconsistent trailing whitespace ('…' / '…')`
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        if msgid.trim().is_empty() || msgstr.trim().is_empty() {
            return;
        }
        let id_ws = get_whitespace_end(msgid);
        let str_ws = get_whitespace_end(msgstr);
        if id_ws != str_ws {
            checker.report_msg(
                entry,
                format!("inconsistent trailing whitespace ('{id_ws}' / '{str_ws}')"),
                msgid,
                &[(msgid.len() - id_ws.len(), msgid.len())],
                msgstr,
                &[(msgstr.len() - str_ws.len(), msgstr.len())],
            );
        }
    }
}

fn get_whitespace_start(value: &str) -> &str {
    let pos = value
        .chars()
        .take_while(|c| c.is_whitespace() && *c != '\n')
        .map(char::len_utf8)
        .sum::<usize>();
    &value[..pos]
}

fn get_whitespace_end(value: &str) -> &str {
    let pos = value
        .chars()
        .rev()
        .take_while(|c| c.is_whitespace() && *c != '\n')
        .map(char::len_utf8)
        .sum::<usize>();
    &value[value.len() - pos..]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_whitespace_start(content: &str) -> Vec<Diagnostic> {
        let rules = Rules::new(vec![Box::new(WhitespaceStartRule {})]);
        let mut checker = Checker::new(content.as_bytes(), &rules);
        checker.do_all_checks();
        checker.diagnostics
    }

    fn check_whitespace_end(content: &str) -> Vec<Diagnostic> {
        let rules = Rules::new(vec![Box::new(WhitespaceEndRule {})]);
        let mut checker = Checker::new(content.as_bytes(), &rules);
        checker.do_all_checks();
        checker.diagnostics
    }

    #[test]
    fn test_get_whitespace_start() {
        assert_eq!(get_whitespace_start(""), "");
        assert_eq!(get_whitespace_start("test"), "");
        assert_eq!(get_whitespace_start("  test"), "  ");
        assert_eq!(get_whitespace_start("\ttest"), "\t");
        assert_eq!(get_whitespace_start(" \ttest"), " \t");
        assert_eq!(get_whitespace_start("\n test"), "");
    }

    #[test]
    fn test_get_whitespace_end() {
        assert_eq!(get_whitespace_end(""), "");
        assert_eq!(get_whitespace_end("test"), "");
        assert_eq!(get_whitespace_end("test  "), "  ");
        assert_eq!(get_whitespace_end("test\t"), "\t");
        assert_eq!(get_whitespace_end("test\t "), "\t ");
        assert_eq!(get_whitespace_end("test \n"), "");
    }

    #[test]
    fn test_no_whitespace() {
        let diags = check_whitespace_start(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
        let diags = check_whitespace_end(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_whitespace_ok() {
        let diags = check_whitespace_start(
            r#"
msgid "  tested  "
msgstr "  testé  "
"#,
        );
        assert!(diags.is_empty());
        let diags = check_whitespace_end(
            r#"
msgid "  tested  "
msgstr "  testé  "
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_whitespace_error() {
        let diags = check_whitespace_start(
            r#"
msgid " tested "
msgstr "testé  "
"#,
        );
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "inconsistent leading whitespace (' ' / '')");
        let diags = check_whitespace_end(
            r#"
msgid " tested "
msgstr "testé  "
"#,
        );
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(
            diag.message,
            "inconsistent trailing whitespace (' ' / '  ')"
        );
    }
}
