// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `double-spaces` rule: check missing/extra double spaces.

use std::ops::Range;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Edit, Fix, FixTarget};
use crate::po::entry::Entry;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct DoubleSpacesRule;

impl RuleChecker for DoubleSpacesRule {
    fn name(&self) -> &'static str {
        "double-spaces"
    }

    fn description(&self) -> &'static str {
        "Check for missing or extra double spaces in translation."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for missing or extra double spaces in the translation.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "the test:  \"xyz\""
    /// msgstr "le test : \"xyz\""
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "the test:  \"xyz\""
    /// msgstr "le test :  \"xyz\""
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`info`](Severity::Info): `missing double spaces '  ' (# / #)`
    /// - [`info`](Severity::Info): `extra double spaces '  ' (# / #)` (auto-fixable
    ///   only when the source has *no* double spaces)
    fn check_msg(
        &self,
        checker: &Checker,
        _entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let id_count = msgid.value.matches("  ").count();
        let str_count = msgstr.value.matches("  ").count();
        let msg = match id_count.cmp(&str_count) {
            std::cmp::Ordering::Equal => return vec![],
            std::cmp::Ordering::Greater => {
                format!("missing double spaces '  ' ({id_count} / {str_count})")
            }
            std::cmp::Ordering::Less => {
                format!("extra double spaces '  ' ({id_count} / {str_count})")
            }
        };
        // Auto-fix only the unambiguous case: the source has *no* double spaces, so
        // every interior double-space run in the translation is surplus and collapses
        // to a single space. When the source has some double spaces (`id_count > 0`)
        // the position of the surplus run is ambiguous, and the "missing" case has no
        // determinable insertion point, so neither is fixed. Leading, trailing and
        // per-line edge runs are left to the whitespace rules to avoid overlapping
        // (and thus conflicting) fixes; see [`interior_double_space_runs`].
        let fix = (id_count == 0)
            .then(|| interior_double_space_runs(&msgstr.value))
            .filter(|runs| !runs.is_empty())
            .map(|runs| Fix {
                target: FixTarget::Msgstr {
                    file_byte_range: msgstr.byte_range.clone(),
                },
                edits: runs
                    .into_iter()
                    .map(|range| Edit {
                        range,
                        replacement: " ".to_string(),
                    })
                    .collect(),
            });
        self.new_diag(checker, Severity::Info, msg)
            .map(|d| {
                d.with_msgs_hl(
                    msgid,
                    msgid
                        .value
                        .match_indices("  ")
                        .map(|(idx, value)| (idx, idx + value.len())),
                    msgstr,
                    msgstr
                        .value
                        .match_indices("  ")
                        .map(|(idx, value)| (idx, idx + value.len())),
                )
                .with_optional_fix(fix)
            })
            .into_iter()
            .collect()
    }
}

/// Find maximal runs of two-or-more ASCII spaces in `value` that are *interior*:
/// not touching the string start or end, and not adjacent to a `'\n'` on either
/// side. Leading, trailing and per-line edge whitespace are the whitespace rules'
/// responsibility (`whitespace-start`, `whitespace-end`, `whitespace-line-start`,
/// `whitespace-line-end`), so those runs are skipped here to keep the collapse
/// fix from overlapping — and therefore conflicting — with theirs. Each surviving
/// run is returned as a byte range to be replaced by a single space.
fn interior_double_space_runs(value: &str) -> Vec<Range<usize>> {
    let bytes = value.as_bytes();
    let len = bytes.len();
    let mut runs = Vec::new();
    let mut i = 0;
    while i < len {
        if bytes[i] != b' ' {
            i += 1;
            continue;
        }
        let start = i;
        while i < len && bytes[i] == b' ' {
            i += 1;
        }
        let end = i;
        if end - start < 2 {
            continue;
        }
        // A single ASCII byte comparison is safe even next to multi-byte UTF-8
        // characters: `'\n'` (0x0A) never appears as a continuation byte.
        let at_str_edge = start == 0 || end == len;
        let at_line_edge =
            (start > 0 && bytes[start - 1] == b'\n') || (end < len && bytes[end] == b'\n');
        if !at_str_edge && !at_line_edge {
            runs.push(start..end);
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_double_spaces(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(DoubleSpacesRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_no_double_spaces() {
        let diags = check_double_spaces(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_double_spaces_ok() {
        let diags = check_double_spaces(
            r#"
msgid "this  is  a  test"
msgstr "ceci  est  un  test"
"#,
        );
        // Note: leading and trailing and double spaces are ignored here
        // (such errors are reported in the "whitespace" checks).
        assert!(diags.is_empty());
    }

    #[test]
    fn test_double_spaces_error_noqa() {
        let diags = check_double_spaces(
            r#"
#, noqa:double-spaces
msgid "this is a  test"
msgstr "ceci est un test"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_double_spaces_error() {
        let diags = check_double_spaces(
            r#"
msgid "this is a  test"
msgstr "ceci est un test"

msgid "this is a test"
msgstr "ceci est un  test"
"#,
        );
        assert_eq!(diags.len(), 2);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "missing double spaces '  ' (1 / 0)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "extra double spaces '  ' (0 / 1)");
    }

    #[test]
    fn test_double_spaces_fix_collapses_interior_run() {
        // Source has no double spaces; the translation's interior double space is
        // surplus and collapses to a single space.
        let diags = check_double_spaces(
            r#"
msgid "a b c d"
msgstr "a b  c d"
"#,
        );
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix should be attached");
        let FixTarget::Msgstr { file_byte_range } = &fix.target else {
            panic!("expected FixTarget::Msgstr, got {:?}", fix.target);
        };
        // "a b  c d": the double space spans bytes 3..5.
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].range, 3..5);
        assert_eq!(fix.edits[0].replacement, " ");
        assert!(file_byte_range.start < file_byte_range.end);
    }

    #[test]
    fn test_double_spaces_fix_multiple_interior_runs_across_lines() {
        // Two interior runs, one on each line, both collapse.
        let diags = check_double_spaces(
            r#"
msgid "a b\nc d"
msgstr "a  b\nc  d"
"#,
        );
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix should be attached");
        // "a  b\nc  d": runs at bytes 1..3 and 6..8.
        assert_eq!(fix.edits.len(), 2);
        assert_eq!(fix.edits[0].range, 1..3);
        assert_eq!(fix.edits[0].replacement, " ");
        assert_eq!(fix.edits[1].range, 6..8);
        assert_eq!(fix.edits[1].replacement, " ");
    }

    #[test]
    fn test_double_spaces_fix_collapses_longer_run() {
        // A run of three spaces collapses to a single space in one edit.
        let diags = check_double_spaces(
            r#"
msgid "a b"
msgstr "a   b"
"#,
        );
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix should be attached");
        // "a   b": the three-space run spans bytes 1..4.
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].range, 1..4);
        assert_eq!(fix.edits[0].replacement, " ");
    }

    #[test]
    fn test_double_spaces_no_fix_when_source_has_double_spaces() {
        // Source already has a double space, so which of the translation's two is
        // surplus is ambiguous: report but do not fix.
        let diags = check_double_spaces(
            r#"
msgid "a  b"
msgstr "a  b  c"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "extra double spaces '  ' (1 / 2)");
        assert!(diags[0].fix.is_none());
    }

    #[test]
    fn test_double_spaces_no_fix_for_missing() {
        // The "missing" case has no determinable insertion point: report only.
        let diags = check_double_spaces(
            r#"
msgid "a  b  c"
msgstr "a b c"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "missing double spaces '  ' (2 / 0)");
        assert!(diags[0].fix.is_none());
    }

    #[test]
    fn test_double_spaces_skips_trailing_edge_run() {
        // A trailing double-space run is the whitespace-end rule's job: the
        // diagnostic still fires but no double-spaces fix is attached.
        let diags = check_double_spaces(
            r#"
msgid "ab"
msgstr "ab  "
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "extra double spaces '  ' (0 / 1)");
        assert!(diags[0].fix.is_none());
    }

    #[test]
    fn test_double_spaces_skips_newline_adjacent_runs() {
        // Runs adjacent to an embedded newline are per-line edge whitespace,
        // owned by the whitespace-line rules: report but do not fix.
        let diags = check_double_spaces(
            r#"
msgid "a\nb"
msgstr "a  \n  b"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert!(diags[0].fix.is_none());
    }

    #[test]
    fn test_interior_double_space_runs() {
        // Interior run only.
        assert_eq!(interior_double_space_runs("a  b"), vec![1..3]);
        // Leading and trailing edges skipped.
        assert_eq!(interior_double_space_runs("  a  b  "), vec![3..5]);
        // Newline-adjacent runs skipped on both sides; interior kept.
        assert_eq!(interior_double_space_runs("  \na  b\n  "), vec![4..6]);
        // Single spaces are not runs.
        assert!(interior_double_space_runs("a b c").is_empty());
    }
}
