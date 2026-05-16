// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Auto-fix primitives.
//!
//! A rule reports a fixable problem by attaching a [`Fix`] to its
//! [`Diagnostic`](crate::diagnostic::Diagnostic). A [`Fix`] is a list of byte-range
//! [`Edit`]s in the **decoded msgstr value** plus a [`FixTarget`] telling the
//! runner *where* in the file the resulting bytes should be spliced.
//!
//! See [`apply_msgstr_fixes`] for how multiple fixes on the same msgstr compose.

use std::ops::Range;

/// One byte-range substitution inside the original `msgstr.value` string.
#[derive(Debug, Clone)]
pub struct Edit {
    pub range: Range<usize>,
    pub replacement: String,
}

/// What the fix is targeting in the original file.
#[derive(Debug, Clone)]
pub enum FixTarget {
    /// Edit a `msgstr` block. The byte range spans the whole block in the
    /// original file (keyword line plus continuation lines, including the
    /// trailing newline). The fix runner reads the original keyword form
    /// (`msgstr` vs. `msgstr[N]`) directly from these bytes, so the rule does
    /// not need to track plural index here.
    Msgstr { file_byte_range: Range<usize> },
}

/// A set of edits to apply to one msgstr value, plus where to splice the result.
#[derive(Debug, Clone)]
pub struct Fix {
    pub target: FixTarget,
    pub edits: Vec<Edit>,
}

/// Returned when two edits on the same msgstr touch overlapping byte ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixConflict {
    pub first: Range<usize>,
    pub second: Range<usize>,
}

impl std::fmt::Display for FixConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "conflicting fix edits: {}..{} overlaps {}..{}",
            self.first.start, self.first.end, self.second.start, self.second.end,
        )
    }
}

impl std::error::Error for FixConflict {}

/// Apply a list of edits to a single msgstr value, returning the new value.
///
/// Edits may be passed in any order. They are sorted by `range.start`; if any two
/// edits cover overlapping byte ranges the function returns [`FixConflict`] and
/// no edit is applied. Adjacent edits that meet at a single offset are allowed.
pub fn apply_msgstr_fixes(value: &str, edits: &[Edit]) -> Result<String, FixConflict> {
    if edits.is_empty() {
        return Ok(value.to_string());
    }
    let mut sorted: Vec<&Edit> = edits.iter().collect();
    sorted.sort_by_key(|e| e.range.start);
    for w in sorted.windows(2) {
        if w[0].range.end > w[1].range.start {
            return Err(FixConflict {
                first: w[0].range.clone(),
                second: w[1].range.clone(),
            });
        }
    }
    let extra: usize = sorted.iter().map(|e| e.replacement.len()).sum();
    let mut out = String::with_capacity(value.len() + extra);
    let mut cursor = 0;
    for e in &sorted {
        out.push_str(&value[cursor..e.range.start]);
        out.push_str(&e.replacement);
        cursor = e.range.end;
    }
    out.push_str(&value[cursor..]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(range: Range<usize>, replacement: &str) -> Edit {
        Edit {
            range,
            replacement: replacement.to_string(),
        }
    }

    #[test]
    fn empty_edits_returns_input() {
        let out = apply_msgstr_fixes("hello", &[]).unwrap();
        assert_eq!(out, "hello");
    }

    #[test]
    fn one_edit_spliced() {
        let out = apply_msgstr_fixes("hello world", &[edit(6..11, "earth")]).unwrap();
        assert_eq!(out, "hello earth");
    }

    #[test]
    fn two_non_overlapping_edits_apply_left_to_right() {
        let out = apply_msgstr_fixes(
            "  hello  ",
            &[edit(7..9, ""), edit(0..2, " ")], // trim trailing, then collapse leading
        )
        .unwrap();
        assert_eq!(out, " hello");
    }

    #[test]
    fn adjacent_edits_allowed() {
        let out = apply_msgstr_fixes("abcdef", &[edit(2..4, "CD"), edit(4..6, "EF")]).unwrap();
        assert_eq!(out, "abCDEF");
    }

    #[test]
    fn overlapping_edits_return_conflict() {
        let err = apply_msgstr_fixes("abcdef", &[edit(1..4, "X"), edit(3..5, "Y")]).unwrap_err();
        assert_eq!(err.first, 1..4);
        assert_eq!(err.second, 3..5);
    }

    #[test]
    fn unsorted_edits_sorted_internally() {
        let out = apply_msgstr_fixes(
            "one two three",
            &[edit(8..13, "THREE"), edit(0..3, "ONE"), edit(4..7, "TWO")],
        )
        .unwrap();
        assert_eq!(out, "ONE TWO THREE");
    }

    #[test]
    fn edit_at_start() {
        let out = apply_msgstr_fixes("  hello", &[edit(0..2, "")]).unwrap();
        assert_eq!(out, "hello");
    }

    #[test]
    fn edit_at_end() {
        let out = apply_msgstr_fixes("hello  ", &[edit(5..7, "")]).unwrap();
        assert_eq!(out, "hello");
    }

    #[test]
    fn insertion_via_empty_range() {
        let out = apply_msgstr_fixes("abef", &[edit(2..2, "cd")]).unwrap();
        assert_eq!(out, "abcdef");
    }
}
