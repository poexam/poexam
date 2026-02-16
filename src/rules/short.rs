// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `short` rule: check if translation is too short.

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct ShortRule;

impl RuleChecker for ShortRule {
    fn name(&self) -> &'static str {
        "short"
    }

    fn is_default(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    /// Check for too short translation.
    ///
    /// This rule reports the entry if one of both conditions is met (leading and trailing
    /// whitespace in strings are ignored):
    ///
    /// - the source has at least 10 times more UTF-8 characters than the translation
    /// - the translation has one UTF-8 character and the source has more than one character.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid " ... :"
    /// msgstr " :"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "ok, this is a very long test message"
    /// msgstr "ok"
    /// ```
    ///
    /// Diagnostics reported with severity [`warning`](Severity::Warning):
    /// - `translation too short (# / #)`
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        // Count the number of UTF-8 chars in both strings, ignoring leading/trailing whitespace.
        let len_msgid = msgid
            .trim()
            .as_bytes()
            .iter()
            .filter(|&&b| b & 0xC0 != 0x80)
            .count();
        if len_msgid == 0 {
            return;
        }
        let len_msgstr = msgstr
            .trim()
            .as_bytes()
            .iter()
            .filter(|&&b| b & 0xC0 != 0x80)
            .count();
        if len_msgstr == 0 {
            return;
        }
        if len_msgstr * 10 <= len_msgid || (len_msgstr == 1 && len_msgid > 1) {
            checker.report_msg(
                entry,
                format!("translation too short ({len_msgid} / {len_msgstr})"),
                msgid,
                &[],
                msgstr,
                &[],
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_short(content: &str) -> Vec<Diagnostic> {
        let rules = Rules::new(vec![Box::new(ShortRule {})]);
        let mut checker = Checker::new(content.as_bytes(), &rules);
        checker.do_all_checks();
        checker.diagnostics
    }

    #[test]
    fn test_no_short() {
        let diags = check_short(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_short_error() {
        let diags = check_short(
            r#"
msgid " ... :"
msgstr " :"

msgid "ok, this is a very long test message"
msgstr "ok"
"#,
        );
        assert_eq!(diags.len(), 2);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "translation too short (5 / 1)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "translation too short (36 / 2)");
    }
}
