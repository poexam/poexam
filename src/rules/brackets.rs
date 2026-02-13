// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `brackets` rule: check missing/extra brackets.

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

const BRACKET_PAIRS: &[(char, char)] = &[('(', ')'), ('[', ']'), ('{', '}'), ('<', '>')];
const BRACKET_NAMES: &[&str] = &["round", "square", "curly", "angle"];

pub struct BracketsRule;

impl RuleChecker for BracketsRule {
    fn name(&self) -> &'static str {
        "brackets"
    }

    fn is_default(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    /// Check for missing or extra round/square/curly/angle brackets in the translation.
    ///
    /// Special case: extra parentheses in the translation are ignored, because this is
    /// often used to precise a word in the translated language.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "this is a test (example)"
    /// msgstr "ceci est un test"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "this is a test (example)"
    /// msgstr "ceci est un test (exemple)"
    /// ```
    ///
    /// Diagnostics reported (`xxx` is `round`/`square`/`curly`/`angle`) with severity [`info`](Severity::Info):
    /// - `missing opening and closing xxx brackets '…' (# / #) and '…' (# / #)`
    /// - `extra opening and closing xxx brackets '…' (# / #) and '…' (# / #)`
    /// - `missing opening xxx brackets '…' (# / #)`
    /// - `extra opening xxx brackets '…' (# / #)`
    /// - `missing closing xxx brackets '…' (# / #)`
    /// - `extra closing xxx brackets '…' (# / #)`
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        for (idx, bracket) in BRACKET_PAIRS.iter().enumerate() {
            let mut id_open = get_opening_bracket_pos(msgid, bracket.0);
            let id_count_open = id_open.len();
            let mut str_open = get_opening_bracket_pos(msgstr, bracket.0);
            let str_count_open = str_open.len();
            let id_close = get_closing_bracket_pos(msgid, bracket.1);
            let id_count_close = id_close.len();
            let str_close = get_closing_bracket_pos(msgstr, bracket.1);
            let str_count_close = str_close.len();
            if BRACKET_PAIRS[idx].0 == '('
                && id_count_open < str_count_open
                && id_count_close < str_count_close
            {
                // We ignore translations with extra parentheses, because this is often used
                // to precise a word in the translated language.
                // For example:
                //   msgid "the position: bottom, top, left or right"
                //   msgstr "la position : bottom (bas), top (haut), left (gauche) ou right (droite)"
                continue;
            }
            if (id_count_open > str_count_open && id_count_close > str_count_close)
                || (id_count_open < str_count_open && id_count_close < str_count_close)
            {
                id_open.extend(&id_close);
                id_open.sort_unstable();
                str_open.extend(&str_close);
                str_open.sort_unstable();
                checker.report_msg(
                    entry,
                    format!(
                        "{} opening and closing {} brackets '{}' ({id_count_open} / {str_count_open}) \
                        and '{}' ({id_count_close} / {str_count_close})",
                        if id_count_open > str_count_open {
                            "missing"
                        } else {
                            "extra"
                        },
                        BRACKET_NAMES[idx], bracket.0, bracket.1,
                    ),
                    msgid,
                    &id_open,
                    msgstr,
                    &str_open,
                );
                continue;
            }
            if id_count_open > str_count_open {
                checker.report_msg(
                    entry,
                    format!(
                        "missing opening {} brackets '{}' ({id_count_open} / {str_count_open})",
                        BRACKET_NAMES[idx], bracket.0,
                    ),
                    msgid,
                    &id_open,
                    msgstr,
                    &str_open,
                );
            }
            if id_count_open < str_count_open {
                checker.report_msg(
                    entry,
                    format!(
                        "extra opening {} brackets '{}' ({id_count_open} / {str_count_open})",
                        BRACKET_NAMES[idx], bracket.0,
                    ),
                    msgid,
                    &id_open,
                    msgstr,
                    &str_open,
                );
            }
            if id_count_close > str_count_close {
                checker.report_msg(
                    entry,
                    format!(
                        "missing closing {} brackets '{}' ({id_count_close} / {str_count_close})",
                        BRACKET_NAMES[idx], bracket.1,
                    ),
                    msgid,
                    &id_close,
                    msgstr,
                    &str_close,
                );
            }
            if id_count_close < str_count_close {
                checker.report_msg(
                    entry,
                    format!(
                        "extra closing {} brackets '{}' ({id_count_close} / {str_count_close})",
                        BRACKET_NAMES[idx], bracket.1,
                    ),
                    msgid,
                    &id_close,
                    msgstr,
                    &str_close,
                );
            }
        }
    }
}

/// Get positions of opening brackets in the string, excluding some patterns.
fn get_opening_bracket_pos(s: &str, bracket_char: char) -> Vec<(usize, usize)> {
    s.match_indices(bracket_char)
        .map(|(idx, value)| (idx, idx + value.len()))
        .filter(|(idx, _)| !is_excluded_start(s, *idx, bracket_char))
        .collect()
}

/// Get positions of closing brackets in the string, excluding some patterns.
fn get_closing_bracket_pos(s: &str, bracket_char: char) -> Vec<(usize, usize)> {
    s.match_indices(bracket_char)
        .map(|(idx, value)| (idx, idx + value.len()))
        .filter(|(idx, _)| !is_excluded_end(s, *idx, bracket_char))
        .collect()
}

/// Check if an excluded pattern is found at index of opening bracket.
///
/// Excluded patterns are "(s)" and "(S)" for opening bracket '(', because they are
/// often used to indicate optional plural forms.
fn is_excluded_start(s: &str, index: usize, bracket_char: char) -> bool {
    if bracket_char == '(' {
        // Exclude "(s)" and "(S)" patterns.
        if s[index..].starts_with("(s)") || s[index..].starts_with("(S)") {
            return true;
        }
    }
    false
}

/// Check if an excluded pattern is found until the index of closing bracket.
///
/// Excluded patterns are "(s)" and "(S)" for closing bracket ')', because they are
/// often used to indicate optional plural forms.
fn is_excluded_end(s: &str, index: usize, bracket_char: char) -> bool {
    if bracket_char == ')' {
        // Exclude "(s)" and "(S)" patterns.
        if s[..=index].ends_with("(s)") || s[..=index].ends_with("(S)") {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_brackets(content: &str) -> Vec<Diagnostic> {
        let rules = Rules::new(vec![Box::new(BracketsRule {})]);
        let mut checker = Checker::new(content.as_bytes(), &rules);
        checker.do_all_checks();
        checker.diagnostics
    }

    #[test]
    fn test_no_brackets() {
        let diags = check_brackets(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_brackets_ok() {
        let diags = check_brackets(
            r#"
msgid "[({<tested>})]"
msgstr "[({<testé>})]"
"#,
        );
        assert!(diags.is_empty());
        let diags = check_brackets(
            r#"
msgid "[({<tested>})]"
msgstr "[({<testé>})]"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_brackets_error() {
        let diags = check_brackets(
            r#"
msgid "[(tested"
msgstr "testé>}"
"#,
        );
        assert_eq!(diags.len(), 4);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "missing opening round brackets '(' (1 / 0)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "missing opening square brackets '[' (1 / 0)");
        let diag = &diags[2];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "extra closing curly brackets '}' (0 / 1)");
        let diag = &diags[3];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "extra closing angle brackets '>' (0 / 1)");

        let diags = check_brackets(
            r#"
msgid "[tested]] {tested}"
msgstr "tested] {{ested}}}"
"#,
        );
        assert_eq!(diags.len(), 2);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(
            diag.message,
            "missing opening and closing square brackets '[' (1 / 0) and ']' (2 / 1)"
        );
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(
            diag.message,
            "extra opening and closing curly brackets '{' (1 / 2) and '}' (1 / 3)"
        );
    }
}
