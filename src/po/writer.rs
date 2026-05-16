// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Splice byte-range replacements into a PO file.
//!
//! The writer is intentionally PO-syntax-unaware: it only knows how to
//! substitute byte ranges of the original file with replacement bytes
//! supplied by the caller. The fix runner is responsible for producing
//! valid PO bytes (escaping, wrapping, encoding) before calling here.

use std::ops::Range;

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum WriteError {
    /// A replacement range extends past `original.len()`.
    OutOfBounds { range: Range<usize>, len: usize },
    /// Two replacement ranges overlap.
    Overlap {
        first: Range<usize>,
        second: Range<usize>,
    },
}

impl std::fmt::Display for WriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfBounds { range, len } => write!(
                f,
                "replacement range {}..{} is out of bounds (file length {len})",
                range.start, range.end,
            ),
            Self::Overlap { first, second } => write!(
                f,
                "replacement ranges {}..{} and {}..{} overlap",
                first.start, first.end, second.start, second.end,
            ),
        }
    }
}

impl std::error::Error for WriteError {}

/// Rewrite `original` by replacing each `(range, replacement)` pair.
///
/// Ranges refer to byte offsets in `original`. They may be passed in any
/// order — the function sorts internally and validates non-overlap and
/// in-bounds before splicing. The output is a fresh `Vec<u8>`.
///
/// With an empty replacement list, the result is byte-identical to the input.
#[allow(dead_code)]
pub fn write_with_replacements(
    original: &[u8],
    mut replacements: Vec<(Range<usize>, Vec<u8>)>,
) -> Result<Vec<u8>, WriteError> {
    if replacements.is_empty() {
        return Ok(original.to_vec());
    }
    let len = original.len();
    for (range, _) in &replacements {
        if range.start > range.end || range.end > len {
            return Err(WriteError::OutOfBounds {
                range: range.clone(),
                len,
            });
        }
    }
    replacements.sort_by_key(|(r, _)| r.start);
    for w in replacements.windows(2) {
        if w[0].0.end > w[1].0.start {
            return Err(WriteError::Overlap {
                first: w[0].0.clone(),
                second: w[1].0.clone(),
            });
        }
    }
    let extra: usize = replacements.iter().map(|(_, b)| b.len()).sum();
    let mut out = Vec::with_capacity(len + extra);
    let mut cursor = 0;
    for (range, bytes) in &replacements {
        out.extend_from_slice(&original[cursor..range.start]);
        out.extend_from_slice(bytes);
        cursor = range.end;
    }
    out.extend_from_slice(&original[cursor..]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_replacements_returns_input() {
        let input = b"hello world";
        let out = write_with_replacements(input, vec![]).unwrap();
        assert_eq!(out, input);
    }

    #[test]
    fn one_replacement_in_the_middle() {
        let input = b"hello world";
        let out = write_with_replacements(input, vec![(6..11, b"earth".to_vec())]).unwrap();
        assert_eq!(out, b"hello earth");
    }

    #[test]
    fn replacement_at_start() {
        let out = write_with_replacements(b"abcdef", vec![(0..2, b"XY".to_vec())]).unwrap();
        assert_eq!(out, b"XYcdef");
    }

    #[test]
    fn replacement_at_end() {
        let out = write_with_replacements(b"abcdef", vec![(4..6, b"EF".to_vec())]).unwrap();
        assert_eq!(out, b"abcdEF");
    }

    #[test]
    fn replacement_with_growing_length() {
        let out = write_with_replacements(b"abcdef", vec![(2..4, b"CCCC".to_vec())]).unwrap();
        assert_eq!(out, b"abCCCCef");
    }

    #[test]
    fn replacement_with_shrinking_length() {
        let out = write_with_replacements(b"abcdef", vec![(1..5, b"-".to_vec())]).unwrap();
        assert_eq!(out, b"a-f");
    }

    #[test]
    fn empty_replacement_deletes_range() {
        let out = write_with_replacements(b"abcdef", vec![(2..4, vec![])]).unwrap();
        assert_eq!(out, b"abef");
    }

    #[test]
    fn multiple_non_overlapping_replacements_unsorted() {
        let out = write_with_replacements(
            b"one two three four",
            vec![
                (14..18, b"FOUR".to_vec()),
                (0..3, b"ONE".to_vec()),
                (8..13, b"THREE".to_vec()),
            ],
        )
        .unwrap();
        assert_eq!(out, b"ONE two THREE FOUR");
    }

    #[test]
    fn adjacent_non_overlapping_replacements_allowed() {
        // [2..4] and [4..6] touch at offset 4 but do not overlap.
        let out = write_with_replacements(
            b"abcdef",
            vec![(2..4, b"CD".to_vec()), (4..6, b"EF".to_vec())],
        )
        .unwrap();
        assert_eq!(out, b"abCDEF");
    }

    #[test]
    fn overlapping_replacements_rejected() {
        let err = write_with_replacements(
            b"abcdef",
            vec![(1..4, b"X".to_vec()), (3..5, b"Y".to_vec())],
        )
        .unwrap_err();
        assert!(matches!(err, WriteError::Overlap { .. }));
    }

    #[test]
    fn out_of_bounds_replacement_rejected() {
        let err = write_with_replacements(b"abcdef", vec![(4..10, b"Z".to_vec())]).unwrap_err();
        assert!(matches!(err, WriteError::OutOfBounds { .. }));
    }

    #[test]
    fn inverted_range_rejected() {
        // Build the inverted range without a literal `start..end` (clippy denies that).
        let inverted = Range { start: 4, end: 2 };
        let err = write_with_replacements(b"abcdef", vec![(inverted, b"Z".to_vec())]).unwrap_err();
        assert!(matches!(err, WriteError::OutOfBounds { .. }));
    }

    #[test]
    fn empty_range_at_position_is_insertion() {
        let out = write_with_replacements(b"abcdef", vec![(3..3, b"XY".to_vec())]).unwrap();
        assert_eq!(out, b"abcXYdef");
    }
}
