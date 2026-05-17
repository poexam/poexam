// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `newlines` rule: check missing/extra newlines.

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Edit, Fix, FixTarget};
use crate::po::entry::Entry;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct NewlinesRule;

impl RuleChecker for NewlinesRule {
    fn name(&self) -> &'static str {
        "newlines"
    }

    fn description(&self) -> &'static str {
        "Check for missing or extra newlines in translation."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
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
    /// Diagnostics reported:
    /// - [`error`](Severity::Error): `missing carriage returns '\r' (# / #)`
    /// - [`error`](Severity::Error): `extra carriage returns '\r' (# / #)`
    /// - [`error`](Severity::Error): `missing line feeds '\n' (# / #)`
    /// - [`error`](Severity::Error): `extra line feeds '\n' (# / #)`
    /// - [`error`](Severity::Error): `missing carriage return '\r' at the beginning` (auto-fixable)
    /// - [`error`](Severity::Error): `extra carriage return '\r' at the beginning` (auto-fixable)
    /// - [`error`](Severity::Error): `missing line feed '\n' at the beginning` (auto-fixable)
    /// - [`error`](Severity::Error): `extra line feed '\n' at the beginning` (auto-fixable)
    /// - [`error`](Severity::Error): `missing carriage return '\r' at the end` (auto-fixable)
    /// - [`error`](Severity::Error): `extra carriage return '\r' at the end` (auto-fixable)
    /// - [`error`](Severity::Error): `missing line feed '\n' at the end` (auto-fixable)
    /// - [`error`](Severity::Error): `extra line feed '\n' at the end` (auto-fixable)
    fn check_msg(
        &self,
        checker: &Checker,
        _entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let mut diags = vec![];
        diags.extend(self.check_cr_lf_count(checker, msgid, msgstr));
        diags.extend(self.check_cr_lf_beginning(checker, msgid, msgstr));
        diags.extend(self.check_cr_lf_end(checker, msgid, msgstr));
        diags
    }
}

impl NewlinesRule {
    /// Check the number of CR ('\r') and LF ('\n') characters.
    fn check_cr_lf_count(
        &self,
        checker: &Checker,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let mut diags = vec![];
        // Check the number of CR ('\r').
        let id_count_cr = msgid.value.matches('\r').count();
        let str_count_cr = msgstr.value.matches('\r').count();
        match id_count_cr.cmp(&str_count_cr) {
            std::cmp::Ordering::Greater => {
                diags.extend(
                    self.new_diag(
                        checker,
                        Severity::Error,
                        format!("missing carriage returns '\\r' ({id_count_cr} / {str_count_cr})"),
                    )
                    .map(|d| d.with_msgs(msgid, msgstr)),
                );
            }
            std::cmp::Ordering::Less => {
                diags.extend(
                    self.new_diag(
                        checker,
                        Severity::Error,
                        format!("extra carriage returns '\\r' ({id_count_cr} / {str_count_cr})"),
                    )
                    .map(|d| d.with_msgs(msgid, msgstr)),
                );
            }
            std::cmp::Ordering::Equal => {}
        }
        // Check the number of LF ('\n').
        let id_count_lf = msgid.value.matches('\n').count();
        let str_count_lf = msgstr.value.matches('\n').count();
        match id_count_lf.cmp(&str_count_lf) {
            std::cmp::Ordering::Greater => {
                diags.extend(
                    self.new_diag(
                        checker,
                        Severity::Error,
                        format!("missing line feeds '\\n' ({id_count_lf} / {str_count_lf})"),
                    )
                    .map(|d| d.with_msgs(msgid, msgstr)),
                );
            }
            std::cmp::Ordering::Less => {
                diags.extend(
                    self.new_diag(
                        checker,
                        Severity::Error,
                        format!("extra line feeds '\\n' ({id_count_lf} / {str_count_lf})"),
                    )
                    .map(|d| d.with_msgs(msgid, msgstr)),
                );
            }
            std::cmp::Ordering::Equal => {}
        }
        diags
    }

    /// Check for CR ('\r') and LF ('\n') at the beginning of the strings.
    ///
    /// When the leading CR/LF run of `msgstr` differs from `msgid`'s, the rule
    /// attaches the same byte-range fix to every diagnostic it emits for this
    /// boundary. `apply_msgstr_fixes` dedups identical edits, so attaching the
    /// fix to both the CR and LF diagnostics is safe.
    fn check_cr_lf_beginning(
        &self,
        checker: &Checker,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let mut diags = vec![];
        let id_run = get_newline_start(&msgid.value);
        let str_run = get_newline_start(&msgstr.value);
        let fix = (id_run != str_run).then(|| Fix {
            target: FixTarget::Msgstr {
                file_byte_range: msgstr.byte_range.clone(),
            },
            edits: vec![Edit {
                range: 0..str_run.len(),
                replacement: id_run.to_string(),
            }],
        });
        // Check CR ('\r') at beginning.
        let id_starts_with_cr = msgid.value.starts_with('\r');
        let str_starts_with_cr = msgstr.value.starts_with('\r');
        match id_starts_with_cr.cmp(&str_starts_with_cr) {
            std::cmp::Ordering::Greater => {
                diags.extend(
                    self.new_diag(
                        checker,
                        Severity::Error,
                        "missing carriage return '\\r' at the beginning".to_string(),
                    )
                    .map(|d| d.with_msgs(msgid, msgstr).with_optional_fix(fix.clone())),
                );
            }
            std::cmp::Ordering::Less => {
                diags.extend(
                    self.new_diag(
                        checker,
                        Severity::Error,
                        "extra carriage return '\\r' at the beginning".to_string(),
                    )
                    .map(|d| d.with_msgs(msgid, msgstr).with_optional_fix(fix.clone())),
                );
            }
            std::cmp::Ordering::Equal => {}
        }
        // Check LF ('\n') at beginning.
        let id_starts_with_lf = msgid.value.starts_with('\n');
        let str_starts_with_lf = msgstr.value.starts_with('\n');
        match id_starts_with_lf.cmp(&str_starts_with_lf) {
            std::cmp::Ordering::Greater => {
                diags.extend(
                    self.new_diag(
                        checker,
                        Severity::Error,
                        "missing line feed '\\n' at the beginning".to_string(),
                    )
                    .map(|d| d.with_msgs(msgid, msgstr).with_optional_fix(fix.clone())),
                );
            }
            std::cmp::Ordering::Less => {
                diags.extend(
                    self.new_diag(
                        checker,
                        Severity::Error,
                        "extra line feed '\\n' at the beginning".to_string(),
                    )
                    .map(|d| d.with_msgs(msgid, msgstr).with_optional_fix(fix.clone())),
                );
            }
            std::cmp::Ordering::Equal => {}
        }
        diags
    }

    /// Check for CR ('\r') and LF ('\n') at the end of the strings.
    ///
    /// See [`check_cr_lf_beginning`](Self::check_cr_lf_beginning) for the
    /// fix-attachment strategy; the same applies here mirrored to the end of
    /// the string.
    fn check_cr_lf_end(
        &self,
        checker: &Checker,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let mut diags = vec![];
        let id_run = get_newline_end(&msgid.value);
        let str_run = get_newline_end(&msgstr.value);
        let str_run_start = msgstr.value.len() - str_run.len();
        let fix = (id_run != str_run).then(|| Fix {
            target: FixTarget::Msgstr {
                file_byte_range: msgstr.byte_range.clone(),
            },
            edits: vec![Edit {
                range: str_run_start..msgstr.value.len(),
                replacement: id_run.to_string(),
            }],
        });
        // Check CR ('\r') at end.
        let id_ends_with_cr = msgid.value.ends_with('\r');
        let str_ends_with_cr = msgstr.value.ends_with('\r');
        match id_ends_with_cr.cmp(&str_ends_with_cr) {
            std::cmp::Ordering::Greater => {
                diags.extend(
                    self.new_diag(
                        checker,
                        Severity::Error,
                        "missing carriage return '\\r' at the end".to_string(),
                    )
                    .map(|d| d.with_msgs(msgid, msgstr).with_optional_fix(fix.clone())),
                );
            }
            std::cmp::Ordering::Less => {
                diags.extend(
                    self.new_diag(
                        checker,
                        Severity::Error,
                        "extra carriage return '\\r' at the end".to_string(),
                    )
                    .map(|d| d.with_msgs(msgid, msgstr).with_optional_fix(fix.clone())),
                );
            }
            std::cmp::Ordering::Equal => {}
        }
        // Check LF ('\n') at end.
        let id_ends_with_lf = msgid.value.ends_with('\n');
        let str_ends_with_lf = msgstr.value.ends_with('\n');
        match id_ends_with_lf.cmp(&str_ends_with_lf) {
            std::cmp::Ordering::Greater => {
                diags.extend(
                    self.new_diag(
                        checker,
                        Severity::Error,
                        "missing line feed '\\n' at the end",
                    )
                    .map(|d| d.with_msgs(msgid, msgstr).with_optional_fix(fix.clone())),
                );
            }
            std::cmp::Ordering::Less => {
                diags.extend(
                    self.new_diag(checker, Severity::Error, "extra line feed '\\n' at the end")
                        .map(|d| d.with_msgs(msgid, msgstr).with_optional_fix(fix.clone())),
                );
            }
            std::cmp::Ordering::Equal => {}
        }
        diags
    }
}

/// Get the leading run of CR/LF characters in `value`.
fn get_newline_start(value: &str) -> &str {
    let pos = value
        .chars()
        .take_while(|c| matches!(c, '\r' | '\n'))
        .map(char::len_utf8)
        .sum::<usize>();
    &value[..pos]
}

/// Get the trailing run of CR/LF characters in `value`.
fn get_newline_end(value: &str) -> &str {
    let pos = value
        .chars()
        .rev()
        .take_while(|c| matches!(c, '\r' | '\n'))
        .map(char::len_utf8)
        .sum::<usize>();
    &value[value.len() - pos..]
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

    #[test]
    fn test_get_newline_start_and_end() {
        assert_eq!(get_newline_start(""), "");
        assert_eq!(get_newline_start("hello"), "");
        assert_eq!(get_newline_start("\nhello"), "\n");
        assert_eq!(get_newline_start("\r\nhello"), "\r\n");
        assert_eq!(get_newline_start("\n\rhello"), "\n\r");
        assert_eq!(get_newline_end(""), "");
        assert_eq!(get_newline_end("hello"), "");
        assert_eq!(get_newline_end("hello\n"), "\n");
        assert_eq!(get_newline_end("hello\r\n"), "\r\n");
    }

    #[test]
    fn test_newlines_count_diagnostic_has_no_fix() {
        // Count mismatches cannot be auto-fixed (we don't know where the
        // missing newline belongs).
        let diags = check_newlines(
            r#"
msgid "first\nsecond"
msgstr "premier second"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "missing line feeds '\\n' (1 / 0)");
        assert!(diags[0].fix.is_none());
    }

    fn diag_with_message<'a>(diags: &'a [Diagnostic], message: &str) -> &'a Diagnostic {
        diags
            .iter()
            .find(|d| d.message == message)
            .unwrap_or_else(|| panic!("no diagnostic with message {message:?} in {diags:#?}"))
    }

    #[test]
    fn test_newlines_beginning_fix_attached() {
        // msgid has a leading LF, msgstr is missing it. The "missing LF at the
        // beginning" diagnostic carries a fix that prepends "\n"; the count
        // diagnostic that also fires has no fix.
        let diags = check_newlines(
            r#"
msgid "\ntested"
msgstr "testé"
"#,
        );
        let count = diag_with_message(&diags, "missing line feeds '\\n' (1 / 0)");
        assert!(
            count.fix.is_none(),
            "count diagnostics are not auto-fixable"
        );
        let begin = diag_with_message(&diags, "missing line feed '\\n' at the beginning");
        let fix = begin.fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].range, 0..0);
        assert_eq!(fix.edits[0].replacement, "\n");
    }

    #[test]
    fn test_newlines_beginning_fix_with_cr_and_lf() {
        // msgid leading = "\n", msgstr leading = "\r\n". Both CR and LF
        // begin-diagnostics fire; both must carry the same fix so dedup
        // composes them into a single edit replacing 0..2 with "\n".
        let diags = check_newlines(
            r#"
msgid "\ntested"
msgstr "\r\ntesté"
"#,
        );
        let cr_begin = diag_with_message(&diags, "extra carriage return '\\r' at the beginning");
        let lf_begin = diag_with_message(&diags, "missing line feed '\\n' at the beginning");
        for diag in [cr_begin, lf_begin] {
            let fix = diag.fix.as_ref().expect("fix on every begin diag");
            assert_eq!(fix.edits.len(), 1);
            assert_eq!(fix.edits[0].range, 0..2);
            assert_eq!(fix.edits[0].replacement, "\n");
        }
    }

    #[test]
    fn test_newlines_end_fix_attached() {
        // msgid trails with "\n", msgstr doesn't.
        let diags = check_newlines(
            r#"
msgid "tested\n"
msgstr "testé"
"#,
        );
        let end = diag_with_message(&diags, "missing line feed '\\n' at the end");
        let fix = end.fix.as_ref().expect("fix attached");
        // msgstr value "testé" is 6 bytes (t-e-s-t-é(2)). Edit inserts "\n" at the end.
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].range, 6..6);
        assert_eq!(fix.edits[0].replacement, "\n");
    }

    #[test]
    fn test_newlines_end_fix_removes_extra() {
        // msgstr has trailing "\n" that msgid doesn't.
        let diags = check_newlines(
            r#"
msgid "tested"
msgstr "testé\n"
"#,
        );
        let end = diag_with_message(&diags, "extra line feed '\\n' at the end");
        let fix = end.fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].range, 6..7);
        assert_eq!(fix.edits[0].replacement, "");
    }
}
