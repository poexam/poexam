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
    /// The following characters are considered as punctuation for this check:
    /// - colon: `:`
    /// - semicolon: `;`
    /// - full stop (period): `.`, `。` or `…`
    /// - comma: `,`
    /// - exclamation mark: `!`
    /// - question mark: `?`
    ///
    /// Special case: leading dots in the source or translation are ignored, because they
    /// are often used for hidden or filename extension.
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
        let id_punc = get_punc_start(msgid);
        let str_punc = get_punc_start(msgstr);
        let id_punc_stripped = id_punc.trim();
        let str_punc_stripped = str_punc.trim();
        if id_punc_stripped.starts_with('.')
            || id_punc_stripped.starts_with("。")
            || str_punc_stripped.starts_with('.')
            || str_punc_stripped.starts_with("。")
        {
            // Ignore leading dots, often used for hidden or filename extension,
            // and the translation may change the order of words.
            // For example:
            //   msgid ".po file broken"
            //   msgstr "fichier .po cassé"
            return;
        }
        if id_punc_stripped.replace("。", ".") != str_punc_stripped.replace("。", ".") {
            checker.report_msg(
                entry,
                format!("inconsistent leading punctuation ('{id_punc}' / '{str_punc}')"),
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
    /// The following characters are considered as punctuation for this check:
    /// - colon: `:`
    /// - semicolon: `;`
    /// - full stop (period): `.`, `。` or `…`
    /// - comma: `,`
    /// - exclamation mark: `!`
    /// - question mark: `?`
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
        let id_punc = get_punc_end(msgid);
        let str_punc = get_punc_end(msgstr);
        let id_punc_stripped = id_punc.trim();
        let str_punc_stripped = str_punc.trim();
        if id_punc_stripped.replace("。", ".") != str_punc_stripped.replace("。", ".") {
            checker.report_msg(
                entry,
                format!("inconsistent trailing punctuation ('{id_punc}' / '{str_punc}')"),
                msgid.highlight_pos(msgid.len() - id_punc.len(), msgid.len()),
                msgstr.highlight_pos(msgstr.len() - str_punc.len(), msgstr.len()),
            );
        }
    }
}

/// Check if a character is considered as punctuation for this rule.
fn is_punc(c: char) -> bool {
    c == ':' || c == ';' || c == '.' || c == '。' || c == '…' || c == ',' || c == '!' || c == '?'
}

/// Get the leading punctuation of a string (it includes whitespace).
fn get_punc_start(s: &str) -> &str {
    let pos = s
        .chars()
        .take_while(|c| is_punc(*c) || (c.is_whitespace() && *c != '\n'))
        .map(char::len_utf8)
        .sum::<usize>();
    &s[..pos]
}

/// Get the trailing punctuation of a string (it includes whitespace).
fn get_punc_end(s: &str) -> &str {
    let pos = s
        .chars()
        .rev()
        .take_while(|c| is_punc(*c) || (c.is_whitespace() && *c != '\n'))
        .map(char::len_utf8)
        .sum::<usize>();
    &s[s.len() - pos..]
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
        assert_eq!(get_punc_start(", test"), ", ");
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
            "inconsistent trailing punctuation ('!' / ' !!!')"
        );
    }
}
