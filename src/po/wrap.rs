// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Wrapping of PO message blocks, matching GNU `msgcat`'s output so a
//! subsequent `msgcat` pass is a no-op.
//!
//! Uses Unicode Line Breaking (UAX #14) plus display width, with the two
//! overrides `msgcat` applies: no break inside a `\X` escape pair, and no
//! break immediately before a trailing `\n`.

use std::collections::HashSet;

use unicode_linebreak::{BreakOpportunity, linebreaks};
use unicode_width::UnicodeWidthChar;

use crate::po::escape::EscapePoExt;

/// Default page width used by GNU `msgcat` (and other gettext tools).
pub const DEFAULT_PAGE_WIDTH: usize = 79;

/// Build the replacement bytes for one msgstr block. The value is wrapped
/// to fit `page_width` columns the same way GNU `msgcat` would; passing
/// `page_width = 0` disables wrapping (matching `msgcat --width=0` and
/// `msgcat --no-wrap`) and emits the value verbatim on a single line. The
/// keyword form is copied from `original_block` so plural and
/// obsolete-prefix variants (`msgstr`, `msgstr[N]`, `#~ msgstr`, …) are
/// preserved.
pub fn format_msgstr_block(original_block: &[u8], new_value: &str, page_width: usize) -> Vec<u8> {
    let quote_pos = original_block
        .iter()
        .position(|&b| b == b'"')
        .unwrap_or(original_block.len());
    let mut head_end = quote_pos;
    while head_end > 0 && matches!(original_block[head_end - 1], b' ' | b'\t') {
        head_end -= 1;
    }
    let head = &original_block[..head_end];

    let escaped = new_value.escape_po();

    if page_width == 0 {
        let mut out = Vec::with_capacity(head.len() + escaped.len() + 4);
        out.extend_from_slice(head);
        out.push(b' ');
        out.push(b'"');
        out.extend_from_slice(escaped.as_bytes());
        out.push(b'"');
        out.push(b'\n');
        return out;
    }

    let head_str = std::str::from_utf8(head).unwrap_or("");
    let head_width = display_width(head_str);

    let portions = split_value_at_newlines(new_value);
    let has_internal_newline = portions.len() > 1;

    // Single-line budget: `head "value"` ≤ page_width  ⇒  value width ≤ page_width - head_width - 3.
    let first_line_budget = page_width.saturating_sub(head_width).saturating_sub(3);
    let escaped_width = display_width(&escaped);
    let escaped_breaks = compute_break_opportunities(&escaped);
    // msgcat goes multi-line when the value is wider than the first-line budget
    // *and* any break opportunity exists — the line-breaker will take a break
    // (even one that overflows the budget) rather than leave the line whole.
    let needs_first_line_break = escaped_width > first_line_budget && !escaped_breaks.is_empty();

    let use_multi_line = has_internal_newline || needs_first_line_break;

    let mut out = Vec::with_capacity(head.len() + escaped.len() + 8);
    if !use_multi_line {
        out.extend_from_slice(head);
        out.push(b' ');
        out.push(b'"');
        out.extend_from_slice(escaped.as_bytes());
        out.push(b'"');
        out.push(b'\n');
        return out;
    }

    let continuation_budget = page_width.saturating_sub(2);
    out.extend_from_slice(head);
    out.extend_from_slice(b" \"\"\n");
    for portion in &portions {
        let portion_escaped = portion.escape_po();
        let portion_breaks = compute_break_opportunities(&portion_escaped);
        wrap_to_budget(
            &portion_escaped,
            &portion_breaks,
            continuation_budget,
            &mut out,
        );
    }
    out
}

/// Display width (in column cells) of a string, summing per-codepoint widths.
fn display_width(s: &str) -> usize {
    s.chars().map(|c| c.width().unwrap_or(0)).sum()
}

/// Split a string at literal `\n` characters; each returned slice keeps its
/// trailing `\n` (except possibly the last). An empty input yields a single
/// empty slice so multi-line emission still produces one empty `""` line.
fn split_value_at_newlines(value: &str) -> Vec<&str> {
    if value.is_empty() {
        return vec![""];
    }
    let mut portions = Vec::new();
    let mut start = 0;
    for (idx, c) in value.char_indices() {
        if c == '\n' {
            let end = idx + c.len_utf8();
            portions.push(&value[start..end]);
            start = end;
        }
    }
    if start < value.len() {
        portions.push(&value[start..]);
    }
    portions
}

/// UAX #14 break opportunities (byte indices where a new line is allowed
/// to start) in `escaped`, minus the two overrides `msgcat` applies:
/// no break inside a `\X` escape pair, and no break immediately before a
/// trailing `\n`.
fn compute_break_opportunities(escaped: &str) -> Vec<usize> {
    let mut allowed: Vec<usize> = linebreaks(escaped)
        .filter(|(_, op)| matches!(op, BreakOpportunity::Allowed))
        .map(|(p, _)| p)
        .collect();

    let bytes = escaped.as_bytes();
    let mut prohibited: HashSet<usize> = HashSet::new();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'\\' {
            prohibited.insert(i + 1);
            i += 2;
        } else {
            i += 1;
        }
    }
    if bytes.len() >= 2 && bytes[bytes.len() - 2] == b'\\' && bytes[bytes.len() - 1] == b'n' {
        prohibited.insert(bytes.len() - 2);
    }
    allowed.retain(|p| !prohibited.contains(p));
    allowed.sort_unstable();
    allowed.dedup();
    allowed
}

/// Greedy wrap of `escaped` into continuation lines `"chunk"\n`, each whose
/// content width is ≤ `budget`. Breaks at the latest valid opportunity that
/// fits; when nothing fits, emits up to the next opportunity (overflowing)
/// rather than splitting mid-token.
fn wrap_to_budget(escaped: &str, breaks: &[usize], budget: usize, out: &mut Vec<u8>) {
    let break_set: HashSet<usize> = breaks.iter().copied().collect();
    let bytes = escaped.as_bytes();
    let mut start = 0;
    while start < bytes.len() {
        let mut width = 0;
        let mut last_break: Option<usize> = None;
        let mut over_budget = false;
        for (rel_idx, c) in escaped[start..].char_indices() {
            let abs_idx = start + rel_idx;
            let w = c.width().unwrap_or(0);
            if width + w > budget {
                over_budget = true;
                break;
            }
            width += w;
            let char_end = abs_idx + c.len_utf8();
            if break_set.contains(&char_end) {
                last_break = Some(char_end);
            }
        }
        if !over_budget {
            out.push(b'"');
            out.extend_from_slice(&bytes[start..]);
            out.push(b'"');
            out.push(b'\n');
            return;
        }
        let next_start = last_break.or_else(|| breaks.iter().copied().find(|&p| p > start));
        if let Some(b) = next_start {
            out.push(b'"');
            out.extend_from_slice(&bytes[start..b]);
            out.push(b'"');
            out.push(b'\n');
            start = b;
        } else {
            out.push(b'"');
            out.extend_from_slice(&bytes[start..]);
            out.push(b'"');
            out.push(b'\n');
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(value: &str) -> String {
        let block = b"msgstr \"original\"\n";
        let bytes = format_msgstr_block(block, value, DEFAULT_PAGE_WIDTH);
        String::from_utf8(bytes).expect("utf-8 output")
    }

    #[test]
    fn no_wrap_when_page_width_is_zero() {
        let block = b"msgstr \"original\"\n";
        // Long value with break opportunities is emitted on a single line.
        let value = "the quick brown fox jumps over the lazy dog the quick brown fox jumps over the lazy dog";
        let bytes = format_msgstr_block(block, value, 0);
        let expected = format!("msgstr \"{value}\"\n");
        assert_eq!(String::from_utf8(bytes).unwrap(), expected);
    }

    #[test]
    fn no_wrap_preserves_internal_newlines_as_escapes() {
        let block = b"msgstr \"original\"\n";
        let bytes = format_msgstr_block(block, "first\nsecond", 0);
        assert_eq!(bytes, b"msgstr \"first\\nsecond\"\n");
    }

    #[test]
    fn wrap_single_line_when_value_fits() {
        assert_eq!(run("hello"), "msgstr \"hello\"\n");
        assert_eq!(run(""), "msgstr \"\"\n");
    }

    #[test]
    fn wrap_single_line_with_trailing_newline() {
        // `\n` only at the very end stays attached, single line allowed.
        assert_eq!(run("hello\n"), "msgstr \"hello\\n\"\n");
    }

    #[test]
    fn wrap_multi_line_with_internal_newline() {
        assert_eq!(
            run("first line\nsecond line"),
            "msgstr \"\"\n\"first line\\n\"\n\"second line\"\n"
        );
    }

    #[test]
    fn wrap_multi_line_word_wraps_at_spaces() {
        let value = "the quick brown fox jumps over the lazy dog the quick brown fox jumps over the lazy dog the quick brown fox jumps over the lazy dog";
        let out = run(value);
        // Matches `msgcat` output: empty first line then two word-wrapped lines.
        assert_eq!(
            out,
            "msgstr \"\"\n\
             \"the quick brown fox jumps over the lazy dog the quick brown fox jumps over \"\n\
             \"the lazy dog the quick brown fox jumps over the lazy dog\"\n",
        );
    }

    #[test]
    fn wrap_keeps_single_line_when_no_break_opportunity() {
        // 87 a's: line is 96 chars but has no UAX#14 break — msgcat leaves it alone.
        let value = "a".repeat(87);
        let expected = format!("msgstr \"{value}\"\n");
        assert_eq!(run(&value), expected);
    }

    #[test]
    fn wrap_keeps_single_line_with_tab_runs() {
        // Tabs render to `\t` (PR class + AL); UAX#14 gives no break here, so msgcat
        // (and we) keep the line as-is even though it's over 79 columns.
        let value = "aaaaaaaa\tbbbbbbbb\tcccccccc\tdddddddd\teeeeeeee\tffffffff\tgggggggg\thhhhh";
        let expected = "msgstr \"aaaaaaaa\\tbbbbbbbb\\tcccccccc\\tdddddddd\\teeeeeeee\\tffffffff\\tgggggggg\\thhhhh\"\n";
        assert_eq!(run(value), expected);
    }

    #[test]
    fn wrap_multi_line_with_hyphens() {
        // Hyphens are HY class so UAX#14 allows break-after; msgcat wraps even
        // though the value is one "token" to a human.
        let value = "aaaaaaaa-bbbbbbbb-cccccccc-dddddddd-eeeeeeee-ffffffff-gggggggg-hhhhhhhh-iiii";
        assert_eq!(
            run(value),
            "msgstr \"\"\n\
             \"aaaaaaaa-bbbbbbbb-cccccccc-dddddddd-eeeeeeee-ffffffff-gggggggg-hhhhhhhh-iiii\"\n",
        );
    }

    #[test]
    fn wrap_preserves_plural_keyword() {
        let block = b"msgstr[1] \"original\"\n";
        let bytes = format_msgstr_block(block, "short", DEFAULT_PAGE_WIDTH);
        assert_eq!(bytes, b"msgstr[1] \"short\"\n");
    }

    #[test]
    fn wrap_preserves_obsolete_prefix() {
        let block = b"#~ msgstr \"original\"\n";
        let bytes = format_msgstr_block(block, "short", DEFAULT_PAGE_WIDTH);
        assert_eq!(bytes, b"#~ msgstr \"short\"\n");
    }

    #[test]
    fn wrap_respects_custom_page_width() {
        // With a tight page width (40), even a moderate value forces multi-line.
        let value = "the quick brown fox jumps over the lazy dog";
        let bytes = format_msgstr_block(b"msgstr \"x\"\n", value, 40);
        let out = String::from_utf8(bytes).unwrap();
        assert_eq!(
            out,
            "msgstr \"\"\n\
             \"the quick brown fox jumps over the \"\n\
             \"lazy dog\"\n",
        );
    }

    /// End-to-end fidelity check: feed our output through `msgcat` and confirm
    /// it round-trips unchanged. Skipped silently if `msgcat` isn't installed.
    #[test]
    fn wrap_matches_msgcat_byte_for_byte() {
        use std::io::Write;
        use std::process::{Command, Stdio};
        if Command::new("msgcat").arg("--version").output().is_err() {
            eprintln!("skipping: msgcat not available");
            return;
        }
        let cases: Vec<String> = vec![
            "short".to_string(),
            String::new(),
            "trailing newline\n".to_string(),
            "first line\nsecond line".to_string(),
            "first line\nsecond line\n".to_string(),
            "the quick brown fox jumps over the lazy dog the quick brown fox jumps over the lazy dog the quick brown fox jumps over the lazy dog".to_string(),
            "aaaaaaaa-bbbbbbbb-cccccccc-dddddddd-eeeeeeee-ffffffff-gggggggg-hhhhhhhh-iiii".to_string(),
            "aaaaaaaa\tbbbbbbbb\tcccccccc\tdddddddd\teeeeeeee\tffffffff\tgggggggg\thhhhh".to_string(),
            "a".repeat(87),
            "aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo".to_string(),
            "Project-Id-Version: x\nReport-Msgid-Bugs-To: y\nLanguage: fr\n".to_string(),
        ];
        for value in &cases {
            let block = b"msgstr \"original\"\n";
            let our_bytes = format_msgstr_block(block, value, DEFAULT_PAGE_WIDTH);
            // Wrap our msgstr in a minimal valid PO file (header + body), run msgcat,
            // then extract msgcat's reformatting of the same msgstr and compare.
            let header =
                b"msgid \"\"\nmsgstr \"Content-Type: text/plain; charset=UTF-8\\n\"\n\nmsgid \"k\"\n";
            let mut po: Vec<u8> = Vec::new();
            po.extend_from_slice(header);
            po.extend_from_slice(&our_bytes);
            let mut child = Command::new("msgcat")
                .arg("-")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn msgcat");
            child.stdin.as_mut().unwrap().write_all(&po).unwrap();
            let out = child.wait_with_output().expect("msgcat output");
            assert!(out.status.success(), "msgcat failed for value: {value:?}");
            let needle = b"\nmsgid \"k\"\nmsgstr";
            let pos = out
                .stdout
                .windows(needle.len())
                .position(|w| w == needle)
                .expect("find msgstr in msgcat output");
            let msgcat_block = &out.stdout[pos + b"\nmsgid \"k\"\n".len()..];
            let msgcat_block = msgcat_block.strip_suffix(b"\n").unwrap_or(msgcat_block);
            assert_eq!(
                msgcat_block,
                our_bytes.strip_suffix(b"\n").unwrap_or(&our_bytes),
                "msgcat re-wrapped our output for value {value:?}\n  ours:    {:?}\n  msgcat: {:?}",
                String::from_utf8_lossy(&our_bytes),
                String::from_utf8_lossy(msgcat_block),
            );
        }
    }
}
