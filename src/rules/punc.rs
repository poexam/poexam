// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::highlight::HighlightExt;
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct PuncStartRule {}

impl RuleChecker for PuncStartRule {
    fn name(&self) -> &'static str {
        "punc-start"
    }

    fn is_default(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    /// Check for inconsistent leading punctuation between source and translation.
    ///
    /// The following characters are considered as punctuation for this check
    /// (half-width and full-width):
    /// - colon: `:`, `：`
    /// - semicolon: `;`, `；`
    /// - full stop (period): `.`, `。`, `…`
    /// - comma: `,`, `，`, `،`
    /// - exclamation mark: `!`, `！`
    /// - question mark: `?`, `？`, `؟`
    ///
    /// Special cases handled:
    /// - Greek: the question mark is `;`.
    /// - Arabic and Persian: the question mark is `؟`.
    /// - Leading dots in the source or translation are ignored, because they are often
    ///   used for hidden or filename extension.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "; this is a test"
    /// msgstr "ceci est un test"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "; this is a test"
    /// msgstr "; ceci est un test"
    /// ```
    ///
    /// Diagnostics reported with severity [`info`](Severity::Info):
    /// - `inconsistent leading punctuation ('…' / '…')`
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        let language = checker.language();
        let id_punc = get_punc_start(msgid);
        let str_punc = get_punc_start(msgstr);
        let id_punc2 = punc_normalize(id_punc.trim(), language);
        let str_punc2 = punc_normalize(str_punc.trim(), language);
        if id_punc2.starts_with('.') || str_punc2.starts_with('.') {
            // Ignore leading dots, often used for hidden or filename extension,
            // and the translation may change the order of words.
            // For example:
            //   msgid ".po file broken"
            //   msgstr "fichier .po cassé"
            return;
        }
        if id_punc2 != str_punc2 {
            checker.report_msg(
                entry,
                format!("inconsistent leading punctuation ('{id_punc2}' / '{str_punc2}')"),
                msgid.highlight_pos(0, id_punc.len()),
                msgstr.highlight_pos(0, str_punc.len()),
            );
        }
    }
}

pub struct PuncEndRule {}

impl RuleChecker for PuncEndRule {
    fn name(&self) -> &'static str {
        "punc-end"
    }

    fn is_default(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    /// Check for inconsistent trailing punctuation between source and translation.
    ///
    /// The following characters are considered as punctuation for this check
    /// (half-width and full-width):
    /// - colon: `:`, `：`
    /// - semicolon: `;`, `；`
    /// - full stop (period): `.`, `。`, `…`
    /// - comma: `,`, `，`, `،`
    /// - exclamation mark: `!`, `！`
    /// - question mark: `?`, `？`, `؟`
    ///
    /// Special cases handled:
    /// - Greek: the question mark is `;`.
    /// - Arabic and Persian: the question mark is `؟`.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "This is a test."
    /// msgstr "Ceci est un test"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "This is a test."
    /// msgstr "Ceci est un test."
    /// ```
    ///
    /// Diagnostics reported with severity [`info`](Severity::Info):
    /// - `inconsistent trailing punctuation ('…' / '…')`
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        let language = checker.language();
        let id_punc = get_punc_end(msgid);
        let str_punc = get_punc_end(msgstr);
        let id_punc2 = punc_normalize(id_punc.trim(), language);
        let str_punc2 = punc_normalize(str_punc.trim(), language);
        if id_punc2 != str_punc2 {
            checker.report_msg(
                entry,
                format!("inconsistent trailing punctuation ('{id_punc2}' / '{str_punc2}')"),
                msgid.highlight_pos(msgid.len() - id_punc.len(), msgid.len()),
                msgstr.highlight_pos(msgstr.len() - str_punc.len(), msgstr.len()),
            );
        }
    }
}

/// Check if a character is considered as punctuation for this rule.
fn is_punc(c: char) -> bool {
    c == ':'
        || c == '：'
        || c == ';'
        || c == '；'
        || c == '؛'
        || c == '.'
        || c == '。'
        || c == '…'
        || c == ','
        || c == '，'
        || c == '،'
        || c == '!'
        || c == '！'
        || c == '?'
        || c == '？'
        || c == '؟'
}

/// Get the leading punctuation of a string (it includes whitespace).
fn get_punc_start(s: &str) -> &str {
    let mut whitespace_ended: bool = false;
    let pos = s
        .chars()
        .take_while(|c| {
            if is_punc(*c) {
                whitespace_ended = true;
                true
            } else if c.is_whitespace() && *c != '\n' {
                !whitespace_ended
            } else {
                false
            }
        })
        .map(char::len_utf8)
        .sum::<usize>();
    &s[..pos]
}

/// Get the trailing punctuation of a string (it includes whitespace).
fn get_punc_end(s: &str) -> &str {
    let mut whitespace_ended: bool = false;
    let pos = s
        .chars()
        .rev()
        .take_while(|c| {
            if is_punc(*c) {
                whitespace_ended = true;
                true
            } else if c.is_whitespace() && *c != '\n' {
                !whitespace_ended
            } else {
                false
            }
        })
        .map(char::len_utf8)
        .sum::<usize>();
    &s[s.len() - pos..]
}

/// Convert punctuation from full-width to normal width and collapse ellipsis.
fn punc_normalize(s: &str, language: &str) -> String {
    s.chars()
        .map(|c| match c {
            // Special case for Greek question mark.
            '?' if language == "el" => ';',
            // General punctuation normalization.
            '：' => ':',
            '；' | '؛' => ';',
            '。' => '.',
            '，' | '،' => ',',
            '！' => '!',
            '？' | '؟' => '?',
            _ => c,
        })
        .collect::<String>()
        .replace("...", "…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_punc_start(content: &str) -> Vec<Diagnostic> {
        let rules = Rules::new(vec![Box::new(PuncStartRule {})]);
        let mut checker = Checker::new(content.as_bytes(), &rules);
        checker.do_all_checks();
        checker.diagnostics
    }

    fn check_punc_end(content: &str) -> Vec<Diagnostic> {
        let rules = Rules::new(vec![Box::new(PuncEndRule {})]);
        let mut checker = Checker::new(content.as_bytes(), &rules);
        checker.do_all_checks();
        checker.diagnostics
    }

    #[test]
    fn test_is_punc() {
        // Characters that should be recognized as punctuation
        let punc_chars = [':', ';', '.', ',', '!', '?'];
        for &c in &punc_chars {
            assert!(is_punc(c), "{c} should be punctuation");
        }
        // Characters that should not be recognized as punctuation
        let non_punc_chars = [
            'a', 'Z', ' ', '-', '\'', '"', '0', 'é', '(', ')', '\r', '\n',
        ];
        for &c in &non_punc_chars {
            assert!(!is_punc(c), "{c} should not be punctuation");
        }
    }

    #[test]
    fn test_get_punc_start() {
        assert_eq!(get_punc_start(""), "");
        assert_eq!(get_punc_start("test"), "");
        assert_eq!(get_punc_start(", test"), ",");
        assert_eq!(get_punc_start("...test"), "...");
        assert_eq!(get_punc_start("…test"), "…");
        assert_eq!(get_punc_start("テスト済み"), "");
        assert_eq!(get_punc_start("。テスト済み"), "。");
        assert_eq!(get_punc_start("。。。テスト済み"), "。。。");
    }

    #[test]
    fn test_get_punc_end() {
        assert_eq!(get_punc_end(""), "");
        assert_eq!(get_punc_end("test"), "");
        assert_eq!(get_punc_end("test, "), ", ");
        assert_eq!(get_punc_end("test..."), "...");
        assert_eq!(get_punc_end("test…"), "…");
        assert_eq!(get_punc_end("テスト済み"), "");
        assert_eq!(get_punc_end("テスト済み。"), "。");
        assert_eq!(get_punc_end("テスト済み。。。"), "。。。");
    }

    #[test]
    fn test_punc_normalize() {
        assert_eq!(punc_normalize("", "fr"), "");
        assert_eq!(punc_normalize("test", "fr"), "test");
        assert_eq!(punc_normalize("。，！？؟：；؛。。。", "zh"), ".,!??:;;…");
        assert_eq!(punc_normalize("?", "fr"), "?");
        // Special case for Greek question mark.
        assert_eq!(punc_normalize("?", "el"), ";");
    }

    #[test]
    fn test_no_punc() {
        let diags = check_punc_start(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
        let diags = check_punc_end(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_punc_ok() {
        let diags = check_punc_end(
            r#"
msgid "tested, ..."
msgstr "testé…"
"#,
        );
        assert!(diags.is_empty());
        let diags = check_punc_start(
            r#"
msgid "tested."
msgstr "テスト済み。"
"#,
        );
        assert!(diags.is_empty());
        let diags = check_punc_end(
            r#"
msgid "tested."
msgstr "テスト済み。"
"#,
        );
        assert!(diags.is_empty());
        let diags = check_punc_end(
            r#"
msgid "tested,"
msgstr "テスト済み，"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_punc_error() {
        let diags = check_punc_start(
            r#"
msgid ":tested!"
msgstr ",testé !!!"
"#,
        );
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "inconsistent leading punctuation (':' / ',')");
        let diags = check_punc_end(
            r#"
msgid ":tested!"
msgstr ",testé !!!"
"#,
        );
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(
            diag.message,
            "inconsistent trailing punctuation ('!' / '!!!')"
        );
    }
}
