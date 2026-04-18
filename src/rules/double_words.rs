// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `double-words` rule: check for consecutive repeated words.

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::po::format::iter::FormatWordPos;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct DoubleWordsRule;

impl RuleChecker for DoubleWordsRule {
    fn name(&self) -> &'static str {
        "double-words"
    }

    fn description(&self) -> &'static str {
        "Check for consecutive repeated words in translation."
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    /// Check for double consecutive words in the translation.
    ///
    /// This rule is not enabled by default.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "This is a test"
    /// msgstr "Ceci est un un test"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "This is a test"
    /// msgstr "Ceci est un test"
    /// ```
    ///
    /// Diagnostics reported with severity [`info`](Severity::Info):
    /// - `word 'xxx' is repeated`
    fn check_msg(
        &self,
        checker: &Checker,
        entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let mut diags = vec![];
        let mut words_iter = FormatWordPos::new(&msgstr.value, &entry.format_language).peekable();
        while let Some(word) = words_iter.next()
            && let Some(next_word) = words_iter.peek()
        {
            // If the current word is the same as the next word, and that there is only
            // whitespace between them, then report a double word.
            if word.s == next_word.s
                && msgstr.value[word.end..next_word.start]
                    .chars()
                    .all(char::is_whitespace)
            {
                diags.push(
                    self.new_diag(checker, format!("word '{}' is repeated", word.s))
                        .with_msgs_hl(msgid, &[], msgstr, &[(word.start, next_word.end)]),
                );
            }
        }
        diags
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_double_words(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(DoubleWordsRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_no_double_words() {
        let diags = check_double_words(
            r#"
msgid "this is a test"
msgstr "ceci est un test"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_double_words_error_noqa() {
        let diags = check_double_words(
            r#"
#, noqa:double-words
msgid "this is a test"
msgstr "ceci est un un test"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_double_words_error() {
        let diags = check_double_words(
            r#"
msgid "this is a test"
msgstr "ceci est un un test"
"#,
        );
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "word 'un' is repeated");
    }
}
