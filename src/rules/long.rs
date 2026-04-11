// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `long` rule: check if translation is too long.

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct LongRule;

impl RuleChecker for LongRule {
    fn name(&self) -> &'static str {
        "long"
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    /// Check for too long translation.
    ///
    /// This rule reports the entry if one of both conditions is met (leading and trailing
    /// whitespace in strings are ignored):
    ///
    /// - the translation has at least 10 times more UTF-8 characters than the source
    /// - the source has one UTF-8 character and the translation has more than one character.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid " :"
    /// msgstr " ... :"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "ok"
    /// msgstr "ok, ceci est une traduction trop longue pour test"
    /// ```
    ///
    /// Diagnostics reported with severity [`warning`](Severity::Warning):
    /// - `translation too long (# / #)`
    fn check_msg(
        &self,
        checker: &Checker,
        _entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        // Count the number of UTF-8 chars in both strings, ignoring leading/trailing whitespace.
        let len_msgid = msgid
            .value
            .trim()
            .as_bytes()
            .iter()
            .filter(|&&b| b & 0xC0 != 0x80)
            .count();
        if len_msgid == 0 {
            return vec![];
        }
        let len_msgstr = msgstr
            .value
            .trim()
            .as_bytes()
            .iter()
            .filter(|&&b| b & 0xC0 != 0x80)
            .count();
        if len_msgstr == 0 {
            return vec![];
        }
        if len_msgid * 10 <= len_msgstr || (len_msgid == 1 && len_msgstr > 1) {
            vec![
                checker
                    .new_diag(format!("translation too long ({len_msgid} / {len_msgstr})"))
                    .with_msgs(msgid, msgstr),
            ]
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_long(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(LongRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_no_long() {
        let diags = check_long(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_long_error_noqa() {
        let diags = check_long(
            r#"
#, noqa:long
msgid " :"
msgstr " ... :"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_long_error() {
        let diags = check_long(
            r#"
msgid " :"
msgstr " ... :"

msgid "ok"
msgstr "ok, ceci est une traduction trop longue pour test"
"#,
        );
        assert_eq!(diags.len(), 2);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "translation too long (1 / 5)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "translation too long (2 / 49)");
    }
}
