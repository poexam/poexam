// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `newlines` rule: check missing/extra newlines.

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct NewlinesRule;

impl NewlinesRule {
    /// Check the number of CR ('\r') and LF ('\n') characters.
    fn check_cr_lf_count(checker: &Checker, msgid: &Message, msgstr: &Message) -> Vec<Diagnostic> {
        let mut diags = vec![];
        // Check the number of CR ('\r').
        let id_count_cr = msgid.value.matches('\r').count();
        let str_count_cr = msgstr.value.matches('\r').count();
        match id_count_cr.cmp(&str_count_cr) {
            std::cmp::Ordering::Greater => {
                diags.push(
                    checker
                        .new_diag(format!(
                            "missing carriage returns '\\r' ({id_count_cr} / {str_count_cr})"
                        ))
                        .with_msgs(msgid, msgstr),
                );
            }
            std::cmp::Ordering::Less => {
                diags.push(
                    checker
                        .new_diag(format!(
                            "extra carriage returns '\\r' ({id_count_cr} / {str_count_cr})"
                        ))
                        .with_msgs(msgid, msgstr),
                );
            }
            std::cmp::Ordering::Equal => {}
        }
        // Check the number of LF ('\n').
        let id_count_lf = msgid.value.matches('\n').count();
        let str_count_lf = msgstr.value.matches('\n').count();
        match id_count_lf.cmp(&str_count_lf) {
            std::cmp::Ordering::Greater => {
                diags.push(
                    checker
                        .new_diag(format!(
                            "missing line feeds '\\n' ({id_count_lf} / {str_count_lf})"
                        ))
                        .with_msgs(msgid, msgstr),
                );
            }
            std::cmp::Ordering::Less => {
                diags.push(
                    checker
                        .new_diag(format!(
                            "extra line feeds '\\n' ({id_count_lf} / {str_count_lf})"
                        ))
                        .with_msgs(msgid, msgstr),
                );
            }
            std::cmp::Ordering::Equal => {}
        }
        diags
    }

    /// Check for CR ('\r') and LF ('\n') at the beginning of the strings.
    fn check_cr_lf_beginning(
        checker: &Checker,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let mut diags = vec![];
        // Check CR ('\r') at beginning.
        let id_starts_with_cr = msgid.value.starts_with('\r');
        let str_starts_with_cr = msgstr.value.starts_with('\r');
        match id_starts_with_cr.cmp(&str_starts_with_cr) {
            std::cmp::Ordering::Greater => {
                diags.push(
                    checker
                        .new_diag("missing carriage return '\\r' at the beginning")
                        .with_msgs(msgid, msgstr),
                );
            }
            std::cmp::Ordering::Less => {
                diags.push(
                    checker
                        .new_diag("extra carriage return '\\r' at the beginning")
                        .with_msgs(msgid, msgstr),
                );
            }
            std::cmp::Ordering::Equal => {}
        }
        // Check LF ('\n') at beginning.
        let id_starts_with_lf = msgid.value.starts_with('\n');
        let str_starts_with_lf = msgstr.value.starts_with('\n');
        match id_starts_with_lf.cmp(&str_starts_with_lf) {
            std::cmp::Ordering::Greater => {
                diags.push(
                    checker
                        .new_diag("missing line feed '\\n' at the beginning")
                        .with_msgs(msgid, msgstr),
                );
            }
            std::cmp::Ordering::Less => {
                diags.push(
                    checker
                        .new_diag("extra line feed '\\n' at the beginning")
                        .with_msgs(msgid, msgstr),
                );
            }
            std::cmp::Ordering::Equal => {}
        }
        diags
    }

    /// Check for CR ('\r') and LF ('\n') at the end of the strings.
    fn check_cr_lf_end(checker: &Checker, msgid: &Message, msgstr: &Message) -> Vec<Diagnostic> {
        let mut diags = vec![];
        // Check CR ('\r') at end.
        let id_ends_with_cr = msgid.value.ends_with('\r');
        let str_ends_with_cr = msgstr.value.ends_with('\r');
        match id_ends_with_cr.cmp(&str_ends_with_cr) {
            std::cmp::Ordering::Greater => {
                diags.push(
                    checker
                        .new_diag("missing carriage return '\\r' at the end")
                        .with_msgs(msgid, msgstr),
                );
            }
            std::cmp::Ordering::Less => {
                diags.push(
                    checker
                        .new_diag("extra carriage return '\\r' at the end")
                        .with_msgs(msgid, msgstr),
                );
            }
            std::cmp::Ordering::Equal => {}
        }
        // Check LF ('\n') at end.
        let id_ends_with_lf = msgid.value.ends_with('\n');
        let str_ends_with_lf = msgstr.value.ends_with('\n');
        match id_ends_with_lf.cmp(&str_ends_with_lf) {
            std::cmp::Ordering::Greater => {
                diags.push(
                    checker
                        .new_diag("missing line feed '\\n' at the end")
                        .with_msgs(msgid, msgstr),
                );
            }
            std::cmp::Ordering::Less => {
                diags.push(
                    checker
                        .new_diag("extra line feed '\\n' at the end")
                        .with_msgs(msgid, msgstr),
                );
            }
            std::cmp::Ordering::Equal => {}
        }
        diags
    }
}

impl RuleChecker for NewlinesRule {
    fn name(&self) -> &'static str {
        "newlines"
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
    fn check_msg(
        &self,
        checker: &Checker,
        _entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let mut diags = vec![];
        diags.extend(Self::check_cr_lf_count(checker, msgid, msgstr));
        diags.extend(Self::check_cr_lf_beginning(checker, msgid, msgstr));
        diags.extend(Self::check_cr_lf_end(checker, msgid, msgstr));
        diags
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_newlines(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(NewlinesRule {})]);
        checker.do_all_checks(&rules);
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
    fn test_newlines_error_noqa() {
        let diags = check_newlines(
            r#"
#, noqa:newlines
msgid "\rtested"
msgstr "testé\rligne 2"
"#,
        );
        assert!(diags.is_empty());
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
