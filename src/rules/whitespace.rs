// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the whitespace rules: check inconsistent whitespace:
//! - `whitespace-start`: whitespace at the beginning of the string
//! - `whitespace-end`: whitespace at the end of the string
//! - `whitespace-line-start`: whitespace at the beginning of each interior line
//! - `whitespace-line-end`: whitespace at the end of each interior line

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
                safe: true,
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
                safe: true,
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

pub struct WhitespaceLineStartRule;

impl RuleChecker for WhitespaceLineStartRule {
    fn name(&self) -> &'static str {
        "whitespace-line-start"
    }

    fn description(&self) -> &'static str {
        "Check for inconsistent leading whitespace at the start of each line."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for inconsistent leading whitespace at the start of each *interior*
    /// line (the lines after an embedded newline). The string's own leading
    /// whitespace is handled by `whitespace-start`, so the first line is skipped.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "first line\n  second line"
    /// msgstr "première ligne\nseconde ligne"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "first line\n  second line"
    /// msgstr "première ligne\n  seconde ligne"
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
        check_interior_whitespace(self, checker, msgid, msgstr, LineEdge::Start)
    }
}

pub struct WhitespaceLineEndRule;

impl RuleChecker for WhitespaceLineEndRule {
    fn name(&self) -> &'static str {
        "whitespace-line-end"
    }

    fn description(&self) -> &'static str {
        "Check for inconsistent trailing whitespace at the end of each line."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for inconsistent trailing whitespace at the end of each *interior*
    /// line (the lines before an embedded newline). The string's own trailing
    /// whitespace is handled by `whitespace-end`, so the last line is skipped.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "first line  \nsecond line"
    /// msgstr "première ligne\nseconde ligne"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "first line  \nsecond line"
    /// msgstr "première ligne  \nseconde ligne"
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
        check_interior_whitespace(self, checker, msgid, msgstr, LineEdge::End)
    }
}

/// Which edge of a line the interior per-line whitespace check inspects.
#[derive(Clone, Copy)]
enum LineEdge {
    /// Leading whitespace, at the start of a line (`whitespace-line-start`).
    Start,
    /// Trailing whitespace, at the end of a line (`whitespace-line-end`).
    End,
}

/// Split `value` on `'\n'` into `(byte_offset, line)` pairs, where `byte_offset`
/// is the start offset of the line within `value`.
fn lines_with_offsets(value: &str) -> Vec<(usize, &str)> {
    let mut offset = 0;
    let mut lines = Vec::new();
    for line in value.split('\n') {
        lines.push((offset, line));
        offset += line.len() + 1;
    }
    lines
}

/// Check inconsistent leading/trailing whitespace at the *interior* line
/// boundaries (around embedded newlines) between source and translation.
///
/// The string's outer edges are covered by `whitespace-start` / `whitespace-end`,
/// so the first line's leading run and the last line's trailing run are skipped.
/// The comparison is line-by-line, so it runs only when source and translation
/// have the same number of lines; otherwise the lines can not be aligned and
/// nothing is reported. Each mismatching boundary yields its own diagnostic,
/// each carrying a fix for that one whitespace run.
fn check_interior_whitespace<R: RuleChecker>(
    rule: &R,
    checker: &Checker,
    msgid: &Message,
    msgstr: &Message,
    edge: LineEdge,
) -> Vec<Diagnostic> {
    if msgid.value.trim().is_empty() || msgstr.value.trim().is_empty() {
        return vec![];
    }
    let id_lines = lines_with_offsets(&msgid.value);
    let str_lines = lines_with_offsets(&msgstr.value);
    if id_lines.len() != str_lines.len() || str_lines.len() < 2 {
        return vec![];
    }
    // Leading edge: skip the first line (its leading run is the string start).
    // Trailing edge: skip the last line (its trailing run is the string end).
    let indices = match edge {
        LineEdge::Start => 1..str_lines.len(),
        LineEdge::End => 0..str_lines.len() - 1,
    };
    let position = match edge {
        LineEdge::Start => "leading",
        LineEdge::End => "trailing",
    };
    let mut diagnostics = Vec::new();
    for i in indices {
        let (id_off, id_line) = id_lines[i];
        let (str_off, str_line) = str_lines[i];
        let (id_ws, str_ws, id_hl, str_hl) = match edge {
            LineEdge::Start => {
                let id_ws = get_whitespace_start(id_line);
                let str_ws = get_whitespace_start(str_line);
                (
                    id_ws,
                    str_ws,
                    (id_off, id_off + id_ws.len()),
                    (str_off, str_off + str_ws.len()),
                )
            }
            LineEdge::End => {
                let id_ws = get_whitespace_end(id_line);
                let str_ws = get_whitespace_end(str_line);
                let id_end = id_off + id_line.len();
                let str_end = str_off + str_line.len();
                (
                    id_ws,
                    str_ws,
                    (id_end - id_ws.len(), id_end),
                    (str_end - str_ws.len(), str_end),
                )
            }
        };
        if id_ws == str_ws {
            continue;
        }
        let fix = Fix {
            target: FixTarget::Msgstr {
                file_byte_range: msgstr.byte_range.clone(),
            },
            edits: vec![Edit {
                range: str_hl.0..str_hl.1,
                replacement: id_ws.to_string(),
            }],
            safe: true,
        };
        if let Some(diag) = rule.new_diag(
            checker,
            Severity::Info,
            format!("inconsistent {position} whitespace ('{id_ws}' / '{str_ws}')"),
        ) {
            diagnostics.push(
                diag.with_msgs_hl(msgid, [id_hl], msgstr, [str_hl])
                    .with_fix(fix),
            );
        }
    }
    diagnostics
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

    fn check_whitespace_line_start(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(WhitespaceLineStartRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    fn check_whitespace_line_end(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(WhitespaceLineEndRule {})]);
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
        let FixTarget::Msgstr { file_byte_range } = &fix.target else {
            panic!("expected FixTarget::Msgstr, got {:?}", fix.target);
        };
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
        let FixTarget::Msgstr { file_byte_range } = &fix.target else {
            panic!("expected FixTarget::Msgstr, got {:?}", fix.target);
        };
        // Decoded msgstr value is "testé  " (= 7 bytes: t-e-s-t-é(2)-space-space).
        // The fix replaces the trailing 2-byte whitespace run with " ".
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].range, 6..8);
        assert_eq!(fix.edits[0].replacement, " ");
        assert!(file_byte_range.start < file_byte_range.end);
    }

    #[test]
    fn test_whitespace_line_consistent_ok() {
        // Interior leading and trailing whitespace match line-by-line.
        let content = r#"
msgid "one\n two \nthree"
msgstr "un\n deux \ntrois"
"#;
        assert!(check_whitespace_line_start(content).is_empty());
        assert!(check_whitespace_line_end(content).is_empty());
    }

    #[test]
    fn test_whitespace_line_ignores_outer_edges() {
        // The leading run of the first line and the trailing run of the last
        // line are the string's outer edges (whitespace-start / whitespace-end),
        // so the per-line rules must ignore them.
        let content = r#"
msgid " one\ntwo "
msgstr "un\ndeux"
"#;
        assert!(check_whitespace_line_start(content).is_empty());
        assert!(check_whitespace_line_end(content).is_empty());
    }

    #[test]
    fn test_whitespace_line_different_line_count_skipped() {
        // Source has two lines, translation one: lines can't be aligned.
        let content = r#"
msgid "one\ntwo"
msgstr "un deux"
"#;
        assert!(check_whitespace_line_start(content).is_empty());
        assert!(check_whitespace_line_end(content).is_empty());
    }

    #[test]
    fn test_whitespace_line_single_line_skipped() {
        // No embedded newline means no interior boundary to check.
        let content = r#"
msgid " one "
msgstr "un"
"#;
        assert!(check_whitespace_line_start(content).is_empty());
        assert!(check_whitespace_line_end(content).is_empty());
    }

    #[test]
    fn test_whitespace_line_start_error() {
        let diags = check_whitespace_line_start(
            r#"
msgid "one\n two"
msgstr "un\ndeux"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Info);
        assert_eq!(
            diags[0].message,
            "inconsistent leading whitespace (' ' / '')"
        );
    }

    #[test]
    fn test_whitespace_line_end_error() {
        let diags = check_whitespace_line_end(
            r#"
msgid "one \ntwo"
msgstr "un\ndeux"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Info);
        assert_eq!(
            diags[0].message,
            "inconsistent trailing whitespace (' ' / '')"
        );
    }

    #[test]
    fn test_whitespace_line_error_noqa() {
        let diags = check_whitespace_line_start(
            r#"
#, noqa:whitespace-line-start
msgid "one\n two"
msgstr "un\ndeux"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_whitespace_line_start_multiple_errors() {
        // Two interior lines each miss the source's leading space.
        let diags = check_whitespace_line_start(
            r#"
msgid "x\n a\n b"
msgstr "u\nc\nd"
"#,
        );
        assert_eq!(diags.len(), 2);
    }

    #[test]
    fn test_whitespace_line_start_fix() {
        // Interior line of msgstr is missing the leading space the msgid has.
        let diags = check_whitespace_line_start(
            r#"
msgid "x\n y"
msgstr "a\nb"
"#,
        );
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix should be attached");
        let FixTarget::Msgstr { file_byte_range } = &fix.target else {
            panic!("expected FixTarget::Msgstr, got {:?}", fix.target);
        };
        // Decoded msgstr value is "a\nb"; the second line "b" starts at byte 2.
        // Its (empty) leading run is replaced with " " (insertion at 2..2).
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].range, 2..2);
        assert_eq!(fix.edits[0].replacement, " ");
        assert!(file_byte_range.start < file_byte_range.end);
    }

    #[test]
    fn test_whitespace_line_end_fix() {
        // Interior line of msgstr is missing the trailing space the msgid has.
        let diags = check_whitespace_line_end(
            r#"
msgid "x \ny"
msgstr "a\nb"
"#,
        );
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix should be attached");
        let FixTarget::Msgstr { file_byte_range } = &fix.target else {
            panic!("expected FixTarget::Msgstr, got {:?}", fix.target);
        };
        // Decoded msgstr value is "a\nb"; the first line "a" ends at byte 1
        // (just before the newline). Its (empty) trailing run is replaced with
        // " " (insertion at 1..1).
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].range, 1..1);
        assert_eq!(fix.edits[0].replacement, " ");
        assert!(file_byte_range.start < file_byte_range.end);
    }
}
