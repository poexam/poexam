// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `emails` rule: check missing/extra/different emails.

use std::collections::HashSet;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Edit, Fix, FixTarget};
use crate::po::entry::Entry;
use crate::po::format::iter::FormatEmailPos;
use crate::po::message::Message;
use crate::rules::double_quotes::trim_quotes;
use crate::rules::rule::RuleChecker;

pub struct EmailsRule;

impl RuleChecker for EmailsRule {
    fn name(&self) -> &'static str {
        "emails"
    }

    fn description(&self) -> &'static str {
        "Check for missing, extra or different emails in translation."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for missing, extra or different emails in the translation.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "Test email: user@example.com"
    /// msgstr "Email de test : utilisateur@exemple.com"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "Test email: user@example.com"
    /// msgstr "Email de test : user@example.com"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`warning`](Severity::Warning): `missing emails (# / #)`
    /// - [`warning`](Severity::Warning): `extra emails (# / #)`
    /// - [`warning`](Severity::Warning): `different emails` (auto-fixable)
    ///
    /// Only the `different emails` diagnostic carries an auto-fix: each
    /// translation email is replaced in place with the email at the same
    /// position in the source. The `missing` and `extra` cases are left
    /// unfixed because inserting a missing email at the right position in
    /// the prose or choosing which extra to drop both require translator
    /// judgement.
    fn check_msg(
        &self,
        checker: &Checker,
        entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let id_emails: Vec<_> = FormatEmailPos::new(&msgid.value, entry.format_language).collect();
        let str_emails: Vec<_> =
            FormatEmailPos::new(&msgstr.value, entry.format_language).collect();
        match id_emails.len().cmp(&str_emails.len()) {
            std::cmp::Ordering::Greater => self
                .new_diag(
                    checker,
                    Severity::Warning,
                    format!(
                        "missing emails ({} / {})",
                        id_emails.len(),
                        str_emails.len()
                    ),
                )
                .map(|d| {
                    d.with_msgs_hl(
                        msgid,
                        id_emails.iter().map(|m| (m.start, m.end)),
                        msgstr,
                        str_emails.iter().map(|m| (m.start, m.end)),
                    )
                })
                .into_iter()
                .collect(),
            std::cmp::Ordering::Less => self
                .new_diag(
                    checker,
                    Severity::Warning,
                    format!("extra emails ({} / {})", id_emails.len(), str_emails.len()),
                )
                .map(|d| {
                    d.with_msgs_hl(
                        msgid,
                        id_emails.iter().map(|m| (m.start, m.end)),
                        msgstr,
                        str_emails.iter().map(|m| (m.start, m.end)),
                    )
                })
                .into_iter()
                .collect(),
            std::cmp::Ordering::Equal => {
                // Check that emails are the same, in any order.
                // A single pair of quotes is skipped from both sides of the email.
                let id_emails_hash: HashSet<_> =
                    id_emails.iter().map(|m| trim_quotes(m.s)).collect();
                let str_emails_hash: HashSet<_> =
                    str_emails.iter().map(|m| trim_quotes(m.s)).collect();
                if id_emails_hash == str_emails_hash {
                    vec![]
                } else {
                    let edits: Vec<Edit> = id_emails
                        .iter()
                        .zip(str_emails.iter())
                        .filter(|(id, str)| trim_quotes(id.s) != trim_quotes(str.s))
                        .map(|(id, str)| Edit {
                            range: str.start..str.end,
                            replacement: id.s.to_string(),
                        })
                        .collect();
                    let fix = (!edits.is_empty()).then(|| Fix {
                        target: FixTarget::Msgstr {
                            file_byte_range: msgstr.byte_range.clone(),
                        },
                        edits,
                        safe: false,
                    });
                    self.new_diag(checker, Severity::Warning, "different emails")
                        .map(|d| {
                            d.with_msgs_hl(
                                msgid,
                                id_emails.iter().map(|m| (m.start, m.end)),
                                msgstr,
                                str_emails.iter().map(|m| (m.start, m.end)),
                            )
                            .with_optional_fix(fix)
                        })
                        .into_iter()
                        .collect()
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_emails(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(EmailsRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_no_emails() {
        let diags = check_emails(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_emails_ok() {
        let diags = check_emails(
            // Order of emails is not checked.
            r#"
msgid "user@domain.com -- „user2@example.com”"
msgstr "user2@example.com -- „user@domain.com”"
"#,
        );
        println!("{diags:#?}");
        assert!(diags.is_empty());
    }

    #[test]
    fn test_quotes_are_not_emails() {
        // A quoted "@" is not an email: the surrounding quotes must not be
        // treated as email characters (regression for "missing emails (1 / 0)").
        let diags = check_emails(
            r#"
msgid "Not an e-mail: \"@\"."
msgstr "Le \"@\" n'est pas un e-mail."
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_emails_error() {
        let diags = check_emails(
            r#"
msgid "missing email: user@domain.com -- user2@example.com"
msgstr "e-mail manquant : user@domain.com"

msgid "extra email: user@domain.com"
msgstr "e-mail extra : user@domain.com -- user2@example.com"

msgid "different emails: user@test.domain.com -- user2@example.com"
msgstr "e-mails différents : user@domain.com -- user2@example.com"
"#,
        );
        assert_eq!(diags.len(), 3);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "missing emails (2 / 1)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "extra emails (1 / 2)");
        let diag = &diags[2];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "different emails");
    }

    #[test]
    fn test_different_emails_fix_replaces_each_in_place() {
        // Two emails, both differ from the source. The fix should propose two
        // edits, each replacing the translation email with the source email
        // at the same position.
        let diags = check_emails(
            r#"
msgid "Contact a@x.com or b@x.com"
msgstr "Contacter a2@x.com ou b2@x.com"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "different emails");
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 2);
        // Edits are produced in source order; replacements come from msgid.
        assert_eq!(fix.edits[0].replacement, "a@x.com");
        assert_eq!(fix.edits[1].replacement, "b@x.com");
    }

    #[test]
    fn test_different_emails_fix_skips_positions_already_equal() {
        // First email matches the source; only the second differs. One edit.
        let diags = check_emails(
            r#"
msgid "Contact a@x.com or b@x.com"
msgstr "Contacter a@x.com ou b2@x.com"
"#,
        );
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].replacement, "b@x.com");
    }

    #[test]
    fn test_missing_and_extra_emails_have_no_fix() {
        // The two count-mismatch diagnostics carry no fix.
        let diags = check_emails(
            r#"
msgid "Contact a@x.com or b@x.com"
msgstr "Contacter a@x.com"

msgid "Contact a@x.com"
msgstr "Contacter a@x.com ou b@x.com"
"#,
        );
        assert_eq!(diags.len(), 2);
        assert!(
            diags[0].fix.is_none(),
            "missing emails diagnostic must not carry a fix"
        );
        assert!(
            diags[1].fix.is_none(),
            "extra emails diagnostic must not carry a fix"
        );
    }
}
