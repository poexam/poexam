// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the punctuation space rules: check for spaces around punctuation:
//! - `punc-space-id`: in the source (`msgid`)
//! - `punc-space-str`: in the translation (`msgstr`)

use std::ops::Range;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Edit, Fix, FixTarget};
use crate::po::entry::Entry;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

/// Canonical non-breaking space used by the auto-fix when the rule wants
/// the translator to use a non-breaking space. The rule accepts both
/// U+00A0 (NO-BREAK SPACE) and U+202F (NARROW NO-BREAK SPACE) as valid;
/// the fix always emits U+00A0 because it is the most universally
/// supported. Translators who prefer the narrow form can change it
/// manually after the fix.
const NBSP: &str = "\u{00A0}";

/// Build a `FixTarget::Msgstr` fix that performs a single edit on `msgstr`.
fn msgstr_fix(msgstr: &Message, range: Range<usize>, replacement: &str) -> Fix {
    Fix {
        target: FixTarget::Msgstr {
            file_byte_range: msgstr.byte_range.clone(),
        },
        edits: vec![Edit {
            range,
            replacement: replacement.to_string(),
        }],
        safe: true,
    }
}

pub struct PuncSpaceIdRule;

impl RuleChecker for PuncSpaceIdRule {
    fn name(&self) -> &'static str {
        "punc-space-id"
    }

    fn description(&self) -> &'static str {
        "Check for incorrect spaces around punctuation in source string."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for spaces around punctuation in the source string (English).
    ///
    /// In English there must be no space before punctuation.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "this is a test !"
    /// msgstr "ceci est un test !"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "this is a test!"
    /// msgstr "ceci est un test !"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`info`](Severity::Info): `extra space before 'x'` in source
    fn check_msg(
        &self,
        checker: &Checker,
        _entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let mut diags = vec![];
        let mut other_char = false;
        let mut chars_iter = msgid.value.char_indices().peekable();
        while let Some((idx, c)) = chars_iter.next()
            && let Some((next_idx, next_c)) = chars_iter.peek()
        {
            if !matches!(
                c,
                ' ' | '\u{00A0}' | '\u{202F}' | ':' | ';' | '!' | '?' | '%' | '«' | '»'
            ) {
                other_char = true;
            }
            if other_char
                && matches!(c, ' ' | '\u{00A0}' | '\u{202F}')
                && matches!(*next_c, ':' | ';' | '!' | '?')
            {
                diags.extend(
                    self.new_diag(
                        checker,
                        Severity::Info,
                        format!("extra space before '{next_c}' in source"),
                    )
                    .map(|d| {
                        d.with_msgs_hl(msgid, [(idx, *next_idx + next_c.len_utf8())], msgstr, [])
                    }),
                );
            }
        }
        diags
    }
}

pub struct PuncSpaceStrRule;

impl RuleChecker for PuncSpaceStrRule {
    fn name(&self) -> &'static str {
        "punc-space-str"
    }

    fn description(&self) -> &'static str {
        "Check for incorrect spaces around punctuation in translation."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for spaces around punctuation in the translated string.
    ///
    /// Only French and Finnish are supported.
    ///
    /// In French:
    /// - There must be a non-breaking space before `:`, `;`, `!`, `?` and `»`.
    /// - There must be a non-breaking space after `«`.
    /// - There must be a non-breaking space between a digit and `%`.
    ///
    /// In Finnish:
    /// - There must be a space between a digit and `%`.
    ///
    /// Wrong entry (French):
    /// ```text
    /// msgid "completion: 42%, this is a test!"
    /// msgstr "achèvement: 42%, ceci est un test!"
    /// ```
    ///
    /// Correct entry (French):
    /// ```text
    /// msgid "completion: 42%, this is a test!"
    /// msgstr "achèvement : 42 %, ceci est un test !"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`info`](Severity::Info): `missing space before 'x' in translation` (auto-fixable)
    /// - [`info`](Severity::Info): `missing non-breaking space before/after 'x' in translation` (auto-fixable)
    /// - [`info`](Severity::Info): `space must be a non-breaking space before/after 'x' in translation` (auto-fixable)
    #[allow(clippy::too_many_lines)]
    fn check_msg(
        &self,
        checker: &Checker,
        _entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let lang_code = checker.language_code();
        let is_french = lang_code == "fr";
        let is_finnish = lang_code == "fi";
        if !is_french && !is_finnish {
            // For now, only check for French and Finnish.
            return vec![];
        }
        let mut diags = vec![];
        let mut other_char = false;
        let mut chars_iter = msgstr.value.char_indices().peekable();
        while let Some((idx, c)) = chars_iter.next()
            && let Some((next_idx, next_c)) = chars_iter.peek()
        {
            if !matches!(
                c,
                ' ' | '\u{00A0}' | '\u{202F}' | ':' | ';' | '!' | '?' | '%' | '«' | '»'
            ) {
                other_char = true;
            }
            if is_french && c == '«' {
                if *next_c == ' ' {
                    // Replace the regular space after `«` with a NBSP.
                    let fix = msgstr_fix(msgstr, *next_idx..(*next_idx + next_c.len_utf8()), NBSP);
                    diags.extend(
                        self.new_diag(
                            checker,
                            Severity::Info,
                            format!(
                                "space must be a non-breaking space after '{c}' in translation"
                            ),
                        )
                        .map(|d| {
                            d.with_msgs_hl(
                                msgid,
                                [],
                                msgstr,
                                [(idx, *next_idx + next_c.len_utf8())],
                            )
                            .with_fix(fix)
                        }),
                    );
                } else if !matches!(*next_c, '\u{00A0}' | '\u{202F}') {
                    // Insert a NBSP between `«` and the next char.
                    let fix = msgstr_fix(msgstr, *next_idx..*next_idx, NBSP);
                    diags.extend(
                        self.new_diag(
                            checker,
                            Severity::Info,
                            format!("missing non-breaking space after '{c}' in translation"),
                        )
                        .map(|d| {
                            d.with_msgs_hl(
                                msgid,
                                [],
                                msgstr,
                                [(idx, *next_idx + next_c.len_utf8())],
                            )
                            .with_fix(fix)
                        }),
                    );
                }
            } else if is_french && *next_c == '»' {
                if c == ' ' {
                    // Replace the regular space before `»` with a NBSP.
                    let fix = msgstr_fix(msgstr, idx..(idx + c.len_utf8()), NBSP);
                    diags.extend(
                        self.new_diag(
                            checker,
                            Severity::Info,
                            format!(
                                "space must be a non-breaking space before '{next_c}' in translation"
                            ),
                        )
                        .map(|d| {
                            d.with_msgs_hl(
                                msgid,
                                [],
                                msgstr,
                                [(idx, *next_idx + next_c.len_utf8())],
                            )
                            .with_fix(fix)
                        }),
                    );
                } else if !matches!(c, '\u{00A0}' | '\u{202F}') {
                    // Insert a NBSP between the previous char and `»`.
                    let fix = msgstr_fix(msgstr, *next_idx..*next_idx, NBSP);
                    diags.extend(
                        self.new_diag(
                            checker,
                            Severity::Info,
                            format!("missing non-breaking space before '{next_c}' in translation"),
                        )
                        .map(|d| {
                            d.with_msgs_hl(
                                msgid,
                                [],
                                msgstr,
                                [(idx, *next_idx + next_c.len_utf8())],
                            )
                            .with_fix(fix)
                        }),
                    );
                }
            } else if c.is_ascii_digit() && *next_c == '%' {
                if is_french {
                    // Insert a NBSP between the digit and `%`.
                    let fix = msgstr_fix(msgstr, *next_idx..*next_idx, NBSP);
                    diags.extend(
                        self.new_diag(
                            checker,
                            Severity::Info,
                            format!("missing non-breaking space before '{next_c}' in translation"),
                        )
                        .map(|d| {
                            d.with_msgs_hl(
                                msgid,
                                [],
                                msgstr,
                                [(idx, *next_idx + next_c.len_utf8())],
                            )
                            .with_fix(fix)
                        }),
                    );
                } else if is_finnish {
                    // Insert a regular space between the digit and `%`.
                    let fix = msgstr_fix(msgstr, *next_idx..*next_idx, " ");
                    diags.extend(
                        self.new_diag(
                            checker,
                            Severity::Info,
                            format!("missing space before '{next_c}' in translation"),
                        )
                        .map(|d| {
                            d.with_msgs_hl(
                                msgid,
                                [],
                                msgstr,
                                [(idx, *next_idx + next_c.len_utf8())],
                            )
                            .with_fix(fix)
                        }),
                    );
                }
            } else if is_french
                && other_char
                && c == ' '
                && matches!(*next_c, ':' | ';' | '!' | '?')
            {
                // Replace the regular space before `: ; ! ?` with a NBSP.
                let fix = msgstr_fix(msgstr, idx..(idx + c.len_utf8()), NBSP);
                diags.extend(
                    self.new_diag(
                        checker,
                        Severity::Info,
                        format!(
                            "space must be a non-breaking space before '{next_c}' in translation"
                        ),
                    )
                    .map(|d| {
                        d.with_msgs_hl(msgid, [], msgstr, [(idx, *next_idx + next_c.len_utf8())])
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

    fn check_punc_space_id(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(PuncSpaceIdRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    fn check_punc_space_str(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(PuncSpaceStrRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_no_punc() {
        let diags = check_punc_space_id(
            r#"
msgid ""
msgstr "Language: fr\n"

msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
        let diags = check_punc_space_str(
            r#"
msgid ""
msgstr "Language: fr\n"

msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_punc_ok() {
        let diags = check_punc_space_id(
            r#"
msgid ""
msgstr "Language: fr\n"

msgid "completion: 42%, this is a test!"
msgstr "achèvement : 42 %, ceci est un test !"
"#,
        );
        assert!(diags.is_empty());
        let diags = check_punc_space_str(
            r#"
msgid ""
msgstr "Language: fr\n"

msgid "completion: 42%, this is a test!"
msgstr "achèvement : 42 %, ceci est un test !"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_punc_space_error_noqa() {
        let diags = check_punc_space_id(
            r#"
msgid ""
msgstr "Language: fr\n"

#, noqa:punc-space-id
msgid "completion : 42%, this is a test !"
msgstr "achèvement: 42%, ceci est un test!"
"#,
        );
        assert!(diags.is_empty());
        let diags = check_punc_space_str(
            r#"
msgid ""
msgstr "Language: fr\n"

#, noqa:punc-space-str
msgid "completion : 42%, this is a test !"
msgstr "achèvement: 42%, ceci est un test!"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_punc_space_error() {
        let diags = check_punc_space_id(
            r#"
msgid ""
msgstr "Language: fr\n"

msgid "completion : 42%, this is a test !"
msgstr "achèvement: 42%, ceci est un test!"
"#,
        );
        assert_eq!(diags.len(), 2);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "extra space before ':' in source");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "extra space before '!' in source");
        let diags = check_punc_space_str(
            r#"
msgid ""
msgstr "Language: fr\n"

msgid "completion : 42%, this is a test !"
msgstr "achèvement: 42%, ceci est un test !"
"#,
        );
        assert_eq!(diags.len(), 2);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(
            diag.message,
            "missing non-breaking space before '%' in translation"
        );
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(
            diag.message,
            "space must be a non-breaking space before '!' in translation"
        );
        let diags = check_punc_space_str(
            r#"
msgid ""
msgstr "Language: fr\n"

msgid "French quotes: test"
msgstr "Guillemets français : « test »"
"#,
        );
        assert_eq!(diags.len(), 2);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(
            diag.message,
            "space must be a non-breaking space after '«' in translation"
        );
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(
            diag.message,
            "space must be a non-breaking space before '»' in translation"
        );
        let diags = check_punc_space_str(
            r#"
msgid ""
msgstr "Language: fr\n"

msgid "French quotes: test"
msgstr "Guillemets français : «test»"
"#,
        );
        assert_eq!(diags.len(), 2);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(
            diag.message,
            "missing non-breaking space after '«' in translation"
        );
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(
            diag.message,
            "missing non-breaking space before '»' in translation"
        );
        let diags = check_punc_space_str(
            r#"
msgid ""
msgstr "Language: fi\n"

msgid "Finnish: 42%"
msgstr "Finlancais : 42%"
"#,
        );
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "missing space before '%' in translation");
    }

    /// Find the single fix attached to the diagnostic carrying `message`.
    /// Returns `(range, replacement)` of the (always single) edit.
    fn fix_for<'a>(diags: &'a [Diagnostic], message: &str) -> (std::ops::Range<usize>, &'a str) {
        let diag = diags
            .iter()
            .find(|d| d.message == message)
            .unwrap_or_else(|| panic!("no diagnostic with message {message:?} in {diags:#?}"));
        let fix = diag.fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 1);
        (
            fix.edits[0].range.clone(),
            fix.edits[0].replacement.as_str(),
        )
    }

    #[test]
    fn test_punc_space_str_french_fixes() {
        // Replace a regular space before `!` with a NBSP.
        // msgstr = "test !"; ' ' is at byte 4 (1 byte), '!' at byte 5.
        let diags = check_punc_space_str(
            "msgid \"\"\nmsgstr \"Language: fr\\n\"\n\nmsgid \"test!\"\nmsgstr \"test !\"\n",
        );
        let (range, replacement) = fix_for(
            &diags,
            "space must be a non-breaking space before '!' in translation",
        );
        assert_eq!(range, 4..5);
        assert_eq!(replacement, "\u{00A0}");

        // Insert a NBSP between digit and `%`.
        // msgstr = "42%"; '4'=0, '2'=1, '%'=2. Fix inserts at 2.
        let diags = check_punc_space_str(
            "msgid \"\"\nmsgstr \"Language: fr\\n\"\n\nmsgid \"42%\"\nmsgstr \"42%\"\n",
        );
        let (range, replacement) = fix_for(
            &diags,
            "missing non-breaking space before '%' in translation",
        );
        assert_eq!(range, 2..2);
        assert_eq!(replacement, "\u{00A0}");

        // Replace a regular space after `«` with a NBSP.
        // msgstr = "« x »"; '«'=0..2 (2 bytes), ' '=2, 'x'=3, ' '=4, '»'=5..7.
        let diags = check_punc_space_str(
            "msgid \"\"\nmsgstr \"Language: fr\\n\"\n\nmsgid \"x\"\nmsgstr \"« x »\"\n",
        );
        let (range, replacement) = fix_for(
            &diags,
            "space must be a non-breaking space after '«' in translation",
        );
        assert_eq!(range, 2..3);
        assert_eq!(replacement, "\u{00A0}");
        let (range, replacement) = fix_for(
            &diags,
            "space must be a non-breaking space before '»' in translation",
        );
        assert_eq!(range, 4..5);
        assert_eq!(replacement, "\u{00A0}");

        // Insert a NBSP after `«` and before `»` when there's no space at all.
        // msgstr = "«x»"; '«'=0..2, 'x'=2, '»'=3..5.
        let diags = check_punc_space_str(
            "msgid \"\"\nmsgstr \"Language: fr\\n\"\n\nmsgid \"x\"\nmsgstr \"«x»\"\n",
        );
        let (range, replacement) = fix_for(
            &diags,
            "missing non-breaking space after '«' in translation",
        );
        assert_eq!(range, 2..2);
        assert_eq!(replacement, "\u{00A0}");
        let (range, replacement) = fix_for(
            &diags,
            "missing non-breaking space before '»' in translation",
        );
        assert_eq!(range, 3..3);
        assert_eq!(replacement, "\u{00A0}");
    }

    #[test]
    fn test_punc_space_str_finnish_fix_uses_regular_space() {
        // Finnish: insert a regular space (not NBSP) between digit and `%`.
        let diags = check_punc_space_str(
            "msgid \"\"\nmsgstr \"Language: fi\\n\"\n\nmsgid \"42%\"\nmsgstr \"42%\"\n",
        );
        let (range, replacement) = fix_for(&diags, "missing space before '%' in translation");
        assert_eq!(range, 2..2);
        assert_eq!(replacement, " ");
    }

    #[test]
    fn test_punc_space_id_has_no_fix() {
        // Source-string diagnostics should never carry a fix.
        let diags = check_punc_space_id(
            "msgid \"\"\nmsgstr \"Language: fr\\n\"\n\nmsgid \"test !\"\nmsgstr \"testé !\"\n",
        );
        assert_eq!(diags.len(), 1);
        assert!(diags[0].fix.is_none());
    }
}
