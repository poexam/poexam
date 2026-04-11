// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the punctuation space rules: check for spaces around punctuation:
//! - `punc-space-id`: in the source (`msgid`)
//! - `punc-space-str`: in the translation (`msgstr`)

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct PuncSpaceIdRule;

impl RuleChecker for PuncSpaceIdRule {
    fn name(&self) -> &'static str {
        "punc-space-id"
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Info
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
    /// Diagnostics reported with severity [`info`](Severity::Info):
    /// - `extra space before 'x'` in source
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
                diags.push(
                    checker
                        .new_diag(format!("extra space before '{next_c}' in source"))
                        .with_msgs_hl(msgid, &[(idx, *next_idx + next_c.len_utf8())], msgstr, &[]),
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

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    /// Check for spaces around punctuation in the translated string.
    ///
    /// Only French and Finnish are supported.
    ///
    /// In French:
    /// - there must be a non-breaking space before `:`, `;`, `!`, `?` and `»`
    /// - there must be a non-breaking space after `«`
    /// - there must be a non-breaking space between a digit and `%`
    ///
    /// In Finnish:
    /// - there must be a space between a digit and `%`
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
    /// Diagnostics reported with severity [`info`](Severity::Info):
    /// - `missing space before 'x' in translation`
    /// - `missing non-breaking space before/after 'x' in translation`
    /// - `space must be a non-breaking space before/after 'x' in translation`
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
                    diags.push(
                        checker
                            .new_diag(format!(
                                "space must be a non-breaking space after '{c}' in translation"
                            ))
                            .with_msgs_hl(
                                msgid,
                                &[],
                                msgstr,
                                &[(idx, *next_idx + next_c.len_utf8())],
                            ),
                    );
                } else if !matches!(*next_c, '\u{00A0}' | '\u{202F}') {
                    diags.push(
                        checker
                            .new_diag(format!(
                                "missing non-breaking space after '{c}' in translation"
                            ))
                            .with_msgs_hl(
                                msgid,
                                &[],
                                msgstr,
                                &[(idx, *next_idx + next_c.len_utf8())],
                            ),
                    );
                }
            } else if is_french && *next_c == '»' {
                if c == ' ' {
                    diags.push(
                        checker.new_diag(
                            format!(
                                "space must be a non-breaking space before '{next_c}' in translation"
                            )
                        ).with_msgs_hl(
                            msgid,
                            &[],
                            msgstr,
                            &[(idx, *next_idx + next_c.len_utf8())]));
                } else if !matches!(c, '\u{00A0}' | '\u{202F}') {
                    diags.push(
                        checker
                            .new_diag(format!(
                                "missing non-breaking space before '{next_c}' in translation"
                            ))
                            .with_msgs_hl(
                                msgid,
                                &[],
                                msgstr,
                                &[(idx, *next_idx + next_c.len_utf8())],
                            ),
                    );
                }
            } else if c.is_ascii_digit() && *next_c == '%' {
                if is_french {
                    diags.push(
                        checker
                            .new_diag(format!(
                                "missing non-breaking space before '{next_c}' in translation"
                            ))
                            .with_msgs_hl(
                                msgid,
                                &[],
                                msgstr,
                                &[(idx, *next_idx + next_c.len_utf8())],
                            ),
                    );
                } else if is_finnish {
                    diags.push(
                        checker
                            .new_diag(format!("missing space before '{next_c}' in translation"))
                            .with_msgs_hl(
                                msgid,
                                &[],
                                msgstr,
                                &[(idx, *next_idx + next_c.len_utf8())],
                            ),
                    );
                }
            } else if is_french
                && other_char
                && c == ' '
                && matches!(*next_c, ':' | ';' | '!' | '?')
            {
                diags.push(
                    checker
                        .new_diag(format!(
                            "space must be a non-breaking space before '{next_c}' in translation"
                        ))
                        .with_msgs_hl(msgid, &[], msgstr, &[(idx, *next_idx + next_c.len_utf8())]),
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
}
