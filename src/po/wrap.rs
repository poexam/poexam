// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Wrapping of PO message blocks, matching GNU `msgcat`'s output so a
//! subsequent `msgcat` pass is a no-op.
//!
//! This follows the function `wrap` of GNU gettext (`write-po.c`): the value
//! is split after each newline, then each portion is escaped and wrapped
//! with the line breaking of GNU libunistring (see
//! [`linebreak`](crate::po::linebreak)), with these overrides: no break inside
//! an escape sequence (e.g. `\n`) or a format string directive (e.g. `%s`),
//! and no break immediately before a trailing `\n`.

use crate::po::format::{FormatParser, language::Language};
use crate::po::linebreak::{
    Break, apply_overrides, char_width, insert_linebreaks, possible_linebreaks,
};

/// Default page width used by GNU `msgcat` (and other gettext tools).
pub const DEFAULT_PAGE_WIDTH: usize = 79;

/// Return the escape char of `c` (e.g. `n` for a newline), as written by
/// GNU gettext, or `None` if `c` is not escaped.
const fn escape_char(c: char) -> Option<char> {
    match c {
        '\u{07}' => Some('a'),
        '\u{08}' => Some('b'),
        '\u{0c}' => Some('f'),
        '\n' => Some('n'),
        '\r' => Some('r'),
        '\t' => Some('t'),
        '\u{0b}' => Some('v'),
        '\\' | '"' => Some(c),
        _ => None,
    }
}

/// Return for each byte of `value` whether it is inside a format string
/// directive, except its first byte: no line break is allowed there. An
/// escaped percent (`%%`) is a directive too.
fn format_directives(value: &str, format: Language) -> Vec<bool> {
    let mut inside = vec![false; value.len()];
    if format == Language::Null {
        return inside;
    }
    let mut pos = 0;
    while let Some((_, next, is_format)) = format.next_char(value, pos) {
        let end = if is_format {
            format.find_end_format(value, next, value.len())
        } else if value[pos..].starts_with("%%") && next == pos + 2 {
            next
        } else {
            pos = next;
            continue;
        };
        for flag in &mut inside[pos + 1..end] {
            *flag = true;
        }
        pos = end;
    }
    inside
}

/// Escape a portion of value, returning the escaped string and the line
/// break overrides for each of its bytes.
fn escape_portion(portion: &str, directives: &[bool]) -> (String, Vec<Break>) {
    let mut escaped = String::with_capacity(portion.len() + 8);
    let mut overrides = Vec::with_capacity(portion.len() + 8);
    for (pos, c) in portion.char_indices() {
        // Don't break inside format directives.
        let brk = if directives[pos] {
            Break::Prohibited
        } else {
            Break::Undefined
        };
        if let Some(e) = escape_char(c) {
            escaped.push('\\');
            escaped.push(e);
            overrides.extend([brk, Break::Prohibited]);
        } else {
            escaped.push(c);
            overrides.push(brk);
            overrides.extend(std::iter::repeat_n(Break::Prohibited, c.len_utf8() - 1));
        }
    }
    // Don't break immediately before the "\n" at the end.
    if portion.ends_with('\n') {
        let len = overrides.len();
        overrides[len - 2] = Break::Prohibited;
    }
    (escaped, overrides)
}

/// Return `true` if `original_block` is a single line with the value
/// `value` already formatted like GNU `msgcat` would do: no newline except at
/// the end of the value, canonical escapes and a line which fits in
/// `page_width` columns (0 for no wrap).
///
/// This is a fast check without allocation, for the most common case (short
/// messages): when it returns `false`, the block may still be well formatted,
/// see [`format_message_block`].
pub fn is_single_line_block(original_block: &[u8], value: &str, page_width: usize) -> bool {
    let Some(body) = original_block.strip_suffix(b"\"\n") else {
        return false;
    };
    let Some(quote_pos) = body.iter().position(|&b| b == b'"') else {
        return false;
    };
    let head = &body[..quote_pos];
    let mut raw = &body[quote_pos + 1..];
    // The keyword must be followed by exactly one space, with no prefix (an
    // obsolete entry has the prefix "#~ ").
    if head.len() < 2 || head[head.len() - 1] != b' ' || head[head.len() - 2] == b' ' {
        return false;
    }
    if head.starts_with(b"#~") && !head.starts_with(b"#~ ") {
        return false;
    }
    let value_trimmed = value.strip_suffix('\n').unwrap_or(value);
    if value_trimmed.contains('\n') {
        return false;
    }
    let mut width = head.len() + 2;
    let mut buf = [0_u8; 4];
    for c in value.chars() {
        let expected: &[u8] = if let Some(e) = escape_char(c) {
            width += 2;
            buf[0] = b'\\';
            buf[1] = u8::try_from(e).unwrap_or(b'?');
            &buf[..2]
        } else {
            width += char_width(c);
            c.encode_utf8(&mut buf).as_bytes()
        };
        let Some(rest) = raw.strip_prefix(expected) else {
            return false;
        };
        raw = rest;
    }
    raw.is_empty() && (page_width == 0 || width <= page_width)
}

/// Build the bytes of one message block (`msgctxt`, `msgid`, `msgid_plural`,
/// `msgstr` or `msgstr[N]`), like GNU `msgcat` would write it.
///
/// The value is wrapped to fit `page_width` columns; passing `page_width = 0`
/// disables wrapping (matching `msgcat --width=0`, `msgcat --no-wrap` and the
/// `no-wrap` flag): the value is then split only after its newlines. The
/// format language of the entry (from its flag, e.g. `c-format`) prevents line
/// breaks inside format string directives.
///
/// The keyword form is copied from `original_block` so plural and
/// obsolete-prefix variants (`msgstr`, `msgstr[N]`, `#~ msgstr`, …) are
/// preserved; all the lines of an obsolete entry are prefixed with `#~ `.
pub fn format_message_block(
    original_block: &[u8],
    value: &str,
    page_width: usize,
    format: Language,
) -> Vec<u8> {
    let quote_pos = original_block
        .iter()
        .position(|&b| b == b'"')
        .unwrap_or(original_block.len());
    let head = original_block[..quote_pos].trim_ascii();
    let (prefix, name): (&[u8], &[u8]) = match head.strip_prefix(b"#~") {
        Some(name) => (b"#~ ", name.trim_ascii_start()),
        None => (b"", head),
    };
    let directives = format_directives(value, format);

    // Columns are signed, like in gettext: the width can be negative for a
    // tiny page width.
    let col = |n: usize| isize::try_from(n).unwrap_or(isize::MAX);
    // Subsequent lines after a break start after the prefix and the quote.
    let startcol_after_break = col(prefix.len()) + 1;
    // The line width, with room for the closing quote character.
    let width = if page_width == 0 {
        isize::MAX / 2
    } else {
        col(page_width)
    } - 1
        - startcol_after_break;

    let mut out = Vec::with_capacity(original_block.len() + 16);
    let mut first_line = true;
    let mut offset = 0;
    loop {
        let rest = &value[offset..];
        let end = rest.find('\n').map_or(rest.len(), |pos| pos + 1);
        let portion = &rest[..end];
        let more = end < rest.len();
        let (escaped, overrides) = escape_portion(portion, &directives[offset..offset + end]);

        // The first line starts after the prefix, the keyword, a space and the
        // opening quote.
        let mut startcol = if first_line {
            col(prefix.len() + name.len()) + 1
        } else {
            col(prefix.len())
        } + 1
            - startcol_after_break;
        // Fast path: no line break is inserted if the whole portion fits.
        let portion_width = col(escaped.chars().map(char_width).sum());
        // The possible line breaks are computed once, for both start columns.
        let mut possible: Option<Vec<Break>> = None;
        let mut linebreaks = |startcol: isize| {
            if startcol + portion_width <= width {
                return vec![false; escaped.len()];
            }
            let possible = possible.get_or_insert_with(|| {
                let mut possible = possible_linebreaks(&escaped);
                apply_overrides(&mut possible, &overrides);
                possible
            });
            insert_linebreaks(&escaped, possible, width, startcol)
        };
        let mut breaks = linebreaks(startcol);

        // If this is the first line and the line would wrap, then use an
        // empty first line and restart.
        if first_line && !escaped.is_empty() && (more || startcol > width || breaks.contains(&true))
        {
            out.extend_from_slice(prefix);
            out.extend_from_slice(name);
            out.extend_from_slice(b" \"\"\n");
            first_line = false;
            startcol = col(prefix.len()) + 1 - startcol_after_break;
            breaks = linebreaks(startcol);
        }

        out.extend_from_slice(prefix);
        if first_line {
            out.extend_from_slice(name);
            out.push(b' ');
            first_line = false;
        }
        out.push(b'"');
        for (pos, byte) in escaped.bytes().enumerate() {
            if breaks[pos] {
                out.extend_from_slice(b"\"\n");
                out.extend_from_slice(prefix);
                out.push(b'"');
            }
            out.push(byte);
        }
        out.extend_from_slice(b"\"\n");

        offset += end;
        if offset >= value.len() {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(value: &str) -> String {
        let block = b"msgstr \"original\"\n";
        let bytes = format_message_block(block, value, DEFAULT_PAGE_WIDTH, Language::Null);
        String::from_utf8(bytes).expect("utf-8 output")
    }

    #[test]
    fn no_wrap_when_page_width_is_zero() {
        let block = b"msgstr \"original\"\n";
        // Long value with break opportunities is emitted on a single line.
        let value = "the quick brown fox jumps over the lazy dog the quick brown fox jumps over the lazy dog";
        let bytes = format_message_block(block, value, 0, Language::Null);
        let expected = format!("msgstr \"{value}\"\n");
        assert_eq!(String::from_utf8(bytes).unwrap(), expected);
    }

    #[test]
    fn no_wrap_splits_only_after_newlines() {
        let block = b"msgstr \"original\"\n";
        let bytes = format_message_block(block, "first\nsecond", 0, Language::Null);
        assert_eq!(bytes, b"msgstr \"\"\n\"first\\n\"\n\"second\"\n");
        let long = "word ".repeat(30);
        let bytes = format_message_block(block, &long, 0, Language::Null);
        assert_eq!(bytes, format!("msgstr \"{long}\"\n").as_bytes());
    }

    #[test]
    fn wrap_obsolete_continuation_lines() {
        let block = b"#~ msgid \"original\"\n";
        let value = "This is an obsolete very long line that goes far beyond the seventy-nine \
                     columns limit";
        let bytes = format_message_block(block, value, 79, Language::Null);
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            "#~ msgid \"\"\n\
             #~ \"This is an obsolete very long line that goes far beyond the seventy-nine \"\n\
             #~ \"columns limit\"\n"
        );
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
        let bytes = format_message_block(block, "short", DEFAULT_PAGE_WIDTH, Language::Null);
        assert_eq!(bytes, b"msgstr[1] \"short\"\n");
    }

    #[test]
    fn wrap_preserves_obsolete_prefix() {
        let block = b"#~ msgstr \"original\"\n";
        let bytes = format_message_block(block, "short", DEFAULT_PAGE_WIDTH, Language::Null);
        assert_eq!(bytes, b"#~ msgstr \"short\"\n");
    }

    #[test]
    fn wrap_respects_custom_page_width() {
        // With a tight page width (40), even a moderate value forces multi-line.
        let value = "the quick brown fox jumps over the lazy dog";
        let bytes = format_message_block(b"msgstr \"x\"\n", value, 40, Language::Null);
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
            let our_bytes = format_message_block(block, value, DEFAULT_PAGE_WIDTH, Language::Null);
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
