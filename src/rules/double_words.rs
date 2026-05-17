// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `double-words` rule: check for consecutive repeated words.

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Edit, Fix, FixTarget};
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
    /// Diagnostics reported (auto-fixable — the fix deletes the whitespace
    /// run that separates the two occurrences and the second occurrence
    /// itself, leaving the first one in place):
    /// - [`info`](Severity::Info): `word '…' is repeated` (auto-fixable)
    fn check_msg(
        &self,
        checker: &Checker,
        entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let mut diags = vec![];
        let mut words_iter = FormatWordPos::new(&msgstr.value, entry.format_language).peekable();
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
                // Delete the separating whitespace and the second occurrence,
                // keeping the first word untouched.
                let fix = Fix {
                    target: FixTarget::Msgstr {
                        file_byte_range: msgstr.byte_range.clone(),
                    },
                    edits: vec![Edit {
                        range: word.end..next_word.end,
                        replacement: String::new(),
                    }],
                };
                diags.extend(
                    self.new_diag(
                        checker,
                        Severity::Info,
                        format!("word '{}' is repeated", word.s),
                    )
                    .map(|d| {
                        d.with_msgs_hl(msgid, [], msgstr, [(word.start, next_word.end)])
                            .with_fix(fix)
                    }),
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

    #[test]
    fn test_double_words_fix_deletes_second_occurrence() {
        // msgstr = "ceci est un un test"
        // First "un" ends at byte 11, second "un" ends at byte 14.
        // Fix deletes 11..14 (the separating space + the second "un").
        let diags = check_double_words(
            r#"
msgid "this is a test"
msgstr "ceci est un un test"
"#,
        );
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].range, 11..14);
        assert_eq!(fix.edits[0].replacement, "");
    }

    #[test]
    fn test_double_words_multiple_pairs_each_have_their_fix() {
        // msgstr = "un un et et"; indices:
        //   "un"=0..2, " "=2, "un"=3..5, " "=5, "et"=6..8, " "=8, "et"=9..11.
        let diags = check_double_words(
            r#"
msgid "test"
msgstr "un un et et"
"#,
        );
        assert_eq!(diags.len(), 2);
        let fix1 = diags[0].fix.as_ref().expect("fix on first diag");
        assert_eq!(fix1.edits[0].range, 2..5);
        let fix2 = diags[1].fix.as_ref().expect("fix on second diag");
        assert_eq!(fix2.edits[0].range, 8..11);
    }

    #[test]
    fn test_double_words_triple_repeat_collapses_to_one() {
        // msgstr = "the the the test"
        // First pair (the[0..3], the[4..7]): fix deletes 3..7.
        // Second pair (the[4..7], the[8..11]): fix deletes 7..11.
        // Adjacent edits → both apply → "the test".
        let diags = check_double_words(
            r#"
msgid "test"
msgstr "the the the test"
"#,
        );
        assert_eq!(diags.len(), 2);
        let fix1 = diags[0].fix.as_ref().expect("fix on first diag");
        assert_eq!(fix1.edits[0].range, 3..7);
        let fix2 = diags[1].fix.as_ref().expect("fix on second diag");
        assert_eq!(fix2.edits[0].range, 7..11);
    }
}
