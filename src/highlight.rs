// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use colored::Colorize;

const HL_TEXT: &str = "bright yellow";
const HL_BG: &str = "red";

pub trait HighlightExt {
    fn highlight_pos(&self, start: usize, end: usize) -> String;
    fn highlight_list_pos(&self, list_pos: &[(usize, usize)]) -> String;
    fn highlight_str(&self, hl: &str) -> String;
    fn highlight_list_str(&self, list_hl: &[&str]) -> String;
}

impl HighlightExt for str {
    /// Highlight a substring from `start` to `end` with the given text and background colors.
    fn highlight_pos(&self, start: usize, end: usize) -> String {
        format!(
            "{}{}{}",
            &self[..start],
            &self[start..end].color(HL_TEXT).bold().on_color(HL_BG),
            &self[end..],
        )
    }

    /// Highlight multiple substrings from `start` to `end` with the given text and background colors.
    fn highlight_list_pos(&self, list_pos: &[(usize, usize)]) -> String {
        let mut result = String::new();
        let mut pos = 0;
        for (start, end) in list_pos {
            if *start < pos {
                continue;
            }
            result.push_str(&self[pos..*start]);
            result.push_str(
                &self[*start..*end]
                    .color(HL_TEXT)
                    .bold()
                    .on_color(HL_BG)
                    .to_string(),
            );
            pos = *end;
        }
        result.push_str(&self[pos..]);
        result
    }

    /// Highlight all occurrences of `hl` with the given text and background colors.
    fn highlight_str(&self, hl: &str) -> String {
        self.replace(
            hl,
            hl.color(HL_TEXT)
                .on_color(HL_BG)
                .bold()
                .to_string()
                .as_str(),
        )
    }

    /// Highlight all occurrences of each substring in `list_hl` with the given text and background colors.
    fn highlight_list_str(&self, list_hl: &[&str]) -> String {
        let mut result = self.to_string();
        for hl in list_hl {
            result = result.highlight_str(hl);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_pos_basic() {
        assert_eq!(
            "abcdef".highlight_pos(2, 4),
            "ab".to_string() + &"cd".color(HL_TEXT).bold().on_color(HL_BG).to_string() + "ef"
        );
        assert_eq!(
            "abcdef".highlight_pos(0, 6),
            "abcdef".color(HL_TEXT).bold().on_color(HL_BG).to_string()
        );
    }

    #[test]
    fn test_highlight_list_pos() {
        assert_eq!(
            "abcdefg".highlight_list_pos(&[(1, 3), (4, 6)]),
            "a".to_string()
                + &"bc".color(HL_TEXT).bold().on_color(HL_BG).to_string()
                + "d"
                + &"ef".color(HL_TEXT).bold().on_color(HL_BG).to_string()
                + "g"
        );
    }

    #[test]
    fn test_highlight_str() {
        assert_eq!(
            "this is a test and another test".highlight_str("test"),
            "this is a ".to_string()
                + &"test".color(HL_TEXT).bold().on_color(HL_BG).to_string()
                + " and another "
                + &"test".color(HL_TEXT).bold().on_color(HL_BG).to_string()
        );
    }

    #[test]
    fn test_highlight_list_str() {
        assert_eq!(
            "abc def ghi abc def".highlight_list_str(&["abc", "def"]),
            "abc".color(HL_TEXT).bold().on_color(HL_BG).to_string()
                + " "
                + &"def".color(HL_TEXT).bold().on_color(HL_BG).to_string()
                + " ghi "
                + &"abc".color(HL_TEXT).bold().on_color(HL_BG).to_string()
                + " "
                + &"def".color(HL_TEXT).bold().on_color(HL_BG).to_string()
        );
    }
}
