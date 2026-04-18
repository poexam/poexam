// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `short` rule: check if translation is too short.

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct ShortRule;

impl RuleChecker for ShortRule {
    fn name(&self) -> &'static str {
        "short"
    }

    fn description(&self) -> &'static str {
        "Check if translation is too short compared to source."
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
        if len_msgstr * 10 <= len_msgid
            || (len_msgstr == 1 && len_msgid > 1 && msgid.value.chars().any(char::is_whitespace))
        {
            vec![
                self.new_diag(
                    checker,
                    format!("translation too short ({len_msgid} / {len_msgstr})"),
                )
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

    fn check_short(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(ShortRule {})]);
        checker.do_all_checks(&rules);
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
    fn test_short_error_noqa() {
        let diags = check_short(
            r#"
#, noqa:short
msgid " ... :"
msgstr " :"
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
