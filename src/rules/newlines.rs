// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `newlines` rule: check missing/extra newlines.

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct NewlinesRule {}

impl NewlinesRule {
    /// Check the number of CR ('\r') and LF ('\n') characters.
    fn check_cr_lf_count(checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        // Check the number of CR ('\r').
        let id_count_cr = msgid.matches('\r').count();
        let str_count_cr = msgstr.matches('\r').count();
        match id_count_cr.cmp(&str_count_cr) {
            std::cmp::Ordering::Greater => {
                checker.report_msg(
                    entry,
                    format!("missing carriage returns '\\r' ({id_count_cr} / {str_count_cr})"),
                    msgid,
                    &[],
                    msgstr,
                    &[],
                );
            }
            std::cmp::Ordering::Less => {
                checker.report_msg(
                    entry,
                    format!("extra carriage returns '\\r' ({id_count_cr} / {str_count_cr})"),
                    msgid,
                    &[],
                    msgstr,
                    &[],
                );
            }
            std::cmp::Ordering::Equal => {}
        }
        // Check the number of LF ('\n').
        let id_count_lf = msgid.matches('\n').count();
        let str_count_lf = msgstr.matches('\n').count();
        match id_count_lf.cmp(&str_count_lf) {
            std::cmp::Ordering::Greater => {
                checker.report_msg(
                    entry,
                    format!("missing line feeds '\\n' ({id_count_lf} / {str_count_lf})"),
                    msgid,
                    &[],
                    msgstr,
                    &[],
                );
            }
            std::cmp::Ordering::Less => {
                checker.report_msg(
                    entry,
                    format!("extra line feeds '\\n' ({id_count_lf} / {str_count_lf})"),
                    msgid,
                    &[],
                    msgstr,
                    &[],
                );
            }
            std::cmp::Ordering::Equal => {}
        }
    }

    /// Check for CR ('\r') and LF ('\n') at the beginning of the strings.
    fn check_cr_lf_beginning(checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        // Check CR ('\r') at beginning.
        let id_starts_with_cr = msgid.starts_with('\r');
        let str_starts_with_cr = msgstr.starts_with('\r');
        match id_starts_with_cr.cmp(&str_starts_with_cr) {
            std::cmp::Ordering::Greater => {
                checker.report_msg(
                    entry,
                    "missing carriage return '\\r' at the beginning".to_string(),
                    msgid,
                    &[],
                    msgstr,
                    &[],
                );
            }
            std::cmp::Ordering::Less => {
                checker.report_msg(
                    entry,
                    "extra carriage return '\\r' at the beginning".to_string(),
                    msgid,
                    &[],
                    msgstr,
                    &[],
                );
            }
            std::cmp::Ordering::Equal => {}
        }
        // Check LF ('\n') at beginning.
        let id_starts_with_lf = msgid.starts_with('\n');
        let str_starts_with_lf = msgstr.starts_with('\n');
        match id_starts_with_lf.cmp(&str_starts_with_lf) {
            std::cmp::Ordering::Greater => {
                checker.report_msg(
                    entry,
                    "missing line feed '\\n' at the beginning".to_string(),
                    msgid,
                    &[],
                    msgstr,
                    &[],
                );
            }
            std::cmp::Ordering::Less => {
                checker.report_msg(
                    entry,
                    "extra line feed '\\n' at the beginning".to_string(),
                    msgid,
                    &[],
                    msgstr,
                    &[],
                );
            }
            std::cmp::Ordering::Equal => {}
        }
    }

    /// Check for CR ('\r') and LF ('\n') at the end of the strings.
    fn check_cr_lf_end(checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        // Check CR ('\r') at end.
        let id_ends_with_cr = msgid.ends_with('\r');
        let str_ends_with_cr = msgstr.ends_with('\r');
        match id_ends_with_cr.cmp(&str_ends_with_cr) {
            std::cmp::Ordering::Greater => {
                checker.report_msg(
                    entry,
                    "missing carriage return '\\r' at the end".to_string(),
                    msgid,
                    &[],
                    msgstr,
                    &[],
                );
            }
            std::cmp::Ordering::Less => {
                checker.report_msg(
                    entry,
                    "extra carriage return '\\r' at the end".to_string(),
                    msgid,
                    &[],
                    msgstr,
                    &[],
                );
            }
            std::cmp::Ordering::Equal => {}
        }
        // Check LF ('\n') at end.
        let id_ends_with_lf = msgid.ends_with('\n');
        let str_ends_with_lf = msgstr.ends_with('\n');
        match id_ends_with_lf.cmp(&str_ends_with_lf) {
            std::cmp::Ordering::Greater => {
                checker.report_msg(
                    entry,
                    "missing line feed '\\n' at the end".to_string(),
                    msgid,
                    &[],
                    msgstr,
                    &[],
                );
            }
            std::cmp::Ordering::Less => {
                checker.report_msg(
                    entry,
                    "extra line feed '\\n' at the end".to_string(),
                    msgid,
                    &[],
                    msgstr,
                    &[],
                );
            }
            std::cmp::Ordering::Equal => {}
        }
    }
}

impl RuleChecker for NewlinesRule {
    fn name(&self) -> &'static str {
        "newlines"
    }

    fn is_default(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    /// Check for missing or extra newlines in the translation: carriage return (`\r`) or line feed (`\n`).
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "this is a test\n"
    /// "second line"
    /// msgstr "ceci est un test"
    /// "seconde ligne"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "this is a test\n"
    /// "second line"
    /// msgstr "ceci est un test\n"
    /// "seconde ligne"
    /// ```
    ///
    /// Diagnostics reported with severity [`error`](Severity::Error):
    /// - `missing carriage returns '\r' (# / #)`
    /// - `extra carriage returns '\r' (# / #)`
    /// - `missing line feeds '\n' (# / #)`
    /// - `extra line feeds '\n' (# / #)`
    /// - `missing carriage return '\r' at the beginning`
    /// - `extra carriage return '\r' at the beginning`
    /// - `missing line feed '\n' at the beginning`
    /// - `extra line feed '\n' at the beginning`
    /// - `missing carriage return '\r' at the end`
    /// - `extra carriage return '\r' at the end`
    /// - `missing line feed '\n' at the end`
    /// - `extra line feed '\n' at the end`
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        NewlinesRule::check_cr_lf_count(checker, entry, msgid, msgstr);
        NewlinesRule::check_cr_lf_beginning(checker, entry, msgid, msgstr);
        NewlinesRule::check_cr_lf_end(checker, entry, msgid, msgstr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_newlines(content: &str) -> Vec<Diagnostic> {
        let rules = Rules::new(vec![Box::new(NewlinesRule {})]);
        let mut checker = Checker::new(content.as_bytes(), &rules);
        checker.do_all_checks();
        checker.diagnostics
    }

    #[test]
    fn test_no_newlines() {
        let diags = check_newlines(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_newlines_ok() {
        let diags = check_newlines(
            r#"
msgid "\ntested\nline 2\n"
msgstr "\ntesté\nligne 2\n"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_newlines_count_error() {
        let diags = check_newlines(
            r#"
msgid "tested\rline 2"
msgstr "testé ligne 2"

msgid "tested line 2"
msgstr "testé\rligne 2"

msgid "tested\nline 2"
msgstr "testé ligne 2"

msgid "testedline 2"
msgstr "testé\nligne 2"
"#,
        );
        assert_eq!(diags.len(), 4);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "missing carriage returns '\\r' (1 / 0)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "extra carriage returns '\\r' (0 / 1)");
        let diag = &diags[2];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "missing line feeds '\\n' (1 / 0)");
        let diag = &diags[3];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "extra line feeds '\\n' (0 / 1)");
    }

    #[test]
    fn test_newlines_beginning_error() {
        let diags = check_newlines(
            r#"
msgid "\rtested"
msgstr "testé\rligne 2"

msgid "\ntested"
msgstr "testé\nligne 2"

msgid "tested\rline 2"
msgstr "\rtesté"

msgid "tested\nline 2"
msgstr "\ntesté"
"#,
        );
        assert_eq!(diags.len(), 4);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(
            diag.message,
            "missing carriage return '\\r' at the beginning"
        );
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "missing line feed '\\n' at the beginning");
        let diag = &diags[2];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "extra carriage return '\\r' at the beginning");
        let diag = &diags[3];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "extra line feed '\\n' at the beginning");
    }

    #[test]
    fn test_newlines_end_error() {
        let diags = check_newlines(
            r#"
msgid "tested\r"
msgstr "testé\rligne 2"

msgid "tested\n"
msgstr "testé\nligne 2"

msgid "tested\rline 2"
msgstr "testé\r"

msgid "tested\nline 2"
msgstr "testé\n"
"#,
        );
        assert_eq!(diags.len(), 4);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "missing carriage return '\\r' at the end");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "missing line feed '\\n' at the end");
        let diag = &diags[2];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "extra carriage return '\\r' at the end");
        let diag = &diags[3];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "extra line feed '\\n' at the end");
    }
}
