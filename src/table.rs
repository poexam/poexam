// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Minimal psql-style table renderer with ANSI-aware column widths.

/// Visible width of `s`, ignoring ANSI CSI escape sequences (`ESC [ ... letter`).
pub fn visible_width(s: &str) -> usize {
    let mut width = 0;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.next() == Some('[') {
                for next in chars.by_ref() {
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            width += 1;
        }
    }
    width
}

/// Render table: ` h | h | h `, dashes joined by `+`, then rows.
pub fn render_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|h| visible_width(h)).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(visible_width(cell));
        }
    }
    let mut out = String::new();
    let push_row = |out: &mut String, cells: &[&str]| {
        for (i, cell) in cells.iter().enumerate() {
            if i > 0 {
                out.push('|');
            }
            out.push(' ');
            out.push_str(cell);
            for _ in 0..widths[i].saturating_sub(visible_width(cell)) {
                out.push(' ');
            }
            out.push(' ');
        }
        out.push('\n');
    };
    push_row(&mut out, headers);
    for (i, w) in widths.iter().enumerate() {
        if i > 0 {
            out.push('+');
        }
        for _ in 0..w + 2 {
            out.push('-');
        }
    }
    out.push('\n');
    for row in rows {
        let cells: Vec<&str> = row.iter().map(String::as_str).collect();
        push_row(&mut out, &cells);
    }
    out.pop();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visible_width_empty() {
        assert_eq!(visible_width(""), 0);
    }

    #[test]
    fn test_visible_width_plain_ascii() {
        assert_eq!(visible_width("hello"), 5);
    }

    #[test]
    fn test_visible_width_skips_single_ansi_sequence() {
        assert_eq!(visible_width("\x1b[33mwarning\x1b[0m"), 7);
    }

    #[test]
    fn test_visible_width_skips_multiple_ansi_sequences() {
        // Bold + bright red + content + reset.
        assert_eq!(visible_width("\x1b[1m\x1b[91merror\x1b[0m"), 5);
    }

    #[test]
    fn test_render_table_basic() {
        let table = render_table(
            &["Name", "Count"],
            &[
                vec!["foo".to_string(), "1".to_string()],
                vec!["barbaz".to_string(), "10".to_string()],
            ],
        );
        let expected = " Name   | Count \n\
                        --------+-------\n\
                        \x20foo    | 1     \n\
                        \x20barbaz | 10    ";
        assert_eq!(table, expected);
    }

    #[test]
    fn test_render_table_column_width_uses_header_when_widest() {
        let table = render_table(&["Description"], &[vec!["x".to_string()]]);
        let expected = " Description \n\
                        -------------\n\
                        \x20x           ";
        assert_eq!(table, expected);
    }

    #[test]
    fn test_render_table_aligns_around_ansi_codes() {
        // Column width should be driven by the visible width (3), not the byte length.
        let table = render_table(
            &["Sev"],
            &[
                vec!["\x1b[33minfo\x1b[0m".to_string()],
                vec!["err".to_string()],
            ],
        );
        let expected = " Sev  \n\
                        ------\n\
                        \x20\x1b[33minfo\x1b[0m \n\
                        \x20err  ";
        assert_eq!(table, expected);
    }

    #[test]
    fn test_render_table_no_rows_returns_header_and_separator() {
        let table = render_table(&["A", "B"], &[]);
        assert_eq!(table, " A | B \n---+---");
    }

    #[test]
    fn test_render_table_single_column() {
        let table = render_table(&["X"], &[vec!["yy".to_string()]]);
        assert_eq!(table, " X  \n----\n yy ");
    }
}
