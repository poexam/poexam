// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the whitespace rules: check inconsistent whitespace:
//! - `whitespace-start`: whitespace at the beginning of the string
//! - `whitespace-end`: whitespace at the end of the string

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Edit, Fix, FixTarget};
use crate::po::entry::Entry;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct WhitespaceStartRule;

impl RuleChecker for WhitespaceStartRule {
    fn name(&self) -> &'static str {
        "whitespace-start"
    }

    fn description(&self) -> &'static str {
        "Check for inconsistent leading whitespace between source and translation."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
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
    /// Diagnostics reported:
    /// - [`info`](Severity::Info): `inconsistent leading whitespace ('…' / '…')` (auto-fixable)
    fn check_msg(
        &self,
        checker: &Checker,
        _entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        if msgid.value.trim().is_empty() || msgstr.value.trim().is_empty() {
            return vec![];
        }
        let id_ws = get_whitespace_start(&msgid.value);
        let str_ws = get_whitespace_start(&msgstr.value);
        if id_ws == str_ws {
            vec![]
        } else {
            let fix = Fix {
                target: FixTarget::Msgstr {
                    file_byte_range: msgstr.byte_range.clone(),
                },
                edits: vec![Edit {
                    range: 0..str_ws.len(),
                    replacement: id_ws.to_string(),
                }],
            };
            self.new_diag(
                checker,
                Severity::Info,
                format!("inconsistent leading whitespace ('{id_ws}' / '{str_ws}')"),
            )
            .map(|d| {
                d.with_msgs_hl(msgid, [(0, id_ws.len())], msgstr, [(0, str_ws.len())])
                    .with_fix(fix)
            })
            .into_iter()
            .collect()
        }
    }
}

pub struct WhitespaceEndRule;

impl RuleChecker for WhitespaceEndRule {
    fn name(&self) -> &'static str {
        "whitespace-end"
    }

    fn description(&self) -> &'static str {
        "Check for inconsistent trailing whitespace between source and translation."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
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
    /// Diagnostics reported:
    /// - [`info`](Severity::Info): `inconsistent trailing whitespace ('…' / '…')` (auto-fixable)
    fn check_msg(
        &self,
        checker: &Checker,
        _entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        if msgid.value.trim().is_empty() || msgstr.value.trim().is_empty() {
            return vec![];
        }
        let id_ws = get_whitespace_end(&msgid.value);
        let str_ws = get_whitespace_end(&msgstr.value);
        if id_ws == str_ws {
            vec![]
        } else {
            let str_ws_start = msgstr.value.len() - str_ws.len();
            let fix = Fix {
                target: FixTarget::Msgstr {
                    file_byte_range: msgstr.byte_range.clone(),
                },
                edits: vec![Edit {
                    range: str_ws_start..msgstr.value.len(),
                    replacement: id_ws.to_string(),
                }],
            };
            self.new_diag(
                checker,
                Severity::Info,
                format!("inconsistent trailing whitespace ('{id_ws}' / '{str_ws}')"),
            )
            .map(|d| {
                d.with_msgs_hl(
                    msgid,
                    [(msgid.value.len() - id_ws.len(), msgid.value.len())],
                    msgstr,
                    [(str_ws_start, msgstr.value.len())],
                )
                .with_fix(fix)
            })
            .into_iter()
            .collect()
        }
    }
}

/// Get the leading whitespace of a string (up to the first non-whitespace character or newline).
fn get_whitespace_start(value: &str) -> &str {
    let pos = value
        .chars()
        .take_while(|c| c.is_whitespace() && *c != '\n')
        .map(char::len_utf8)
        .sum::<usize>();
    &value[..pos]
}

/// Get the trailing whitespace of a string (up to the last non-whitespace character or newline).
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
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(WhitespaceStartRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    fn check_whitespace_end(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(WhitespaceEndRule {})]);
        checker.do_all_checks(&rules);
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
    fn test_whitespace_error_noqa() {
        let diags = check_whitespace_start(
            r#"
#, noqa:whitespace-start
msgid " tested "
msgstr "testé  "
"#,
        );
        assert!(diags.is_empty());
        let diags = check_whitespace_end(
            r#"
#, noqa:whitespace-end
msgid " tested "
msgstr "testé  "
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

    #[test]
    fn test_whitespace_start_fix() {
        // msgstr is missing the leading space the msgid has.
        let diags = check_whitespace_start(
            r#"
msgid " tested"
msgstr "testé"
"#,
        );
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix should be attached");
        let FixTarget::Msgstr { file_byte_range } = &fix.target;
        // The fix replaces the leading whitespace run (currently empty, 0..0)
        // of msgstr with " ".
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].range, 0..0);
        assert_eq!(fix.edits[0].replacement, " ");
        // The target byte range must point at the msgstr block in the file.
        assert!(file_byte_range.start < file_byte_range.end);
    }

    #[test]
    fn test_whitespace_end_fix() {
        // msgstr has two trailing spaces; msgid has one.
        let diags = check_whitespace_end(
            r#"
msgid "tested "
msgstr "testé  "
"#,
        );
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix should be attached");
        let FixTarget::Msgstr { file_byte_range } = &fix.target;
        // Decoded msgstr value is "testé  " (= 7 bytes: t-e-s-t-é(2)-space-space).
        // The fix replaces the trailing 2-byte whitespace run with " ".
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].range, 6..8);
        assert_eq!(fix.edits[0].replacement, " ");
        assert!(file_byte_range.start < file_byte_range.end);
    }
}
