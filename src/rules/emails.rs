// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `emails` rule: check missing/extra/different emails.

use std::collections::HashSet;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
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
    /// - [`warning`](Severity::Warning): `different emails`
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
                    self.new_diag(checker, Severity::Warning, "different emails")
                        .map(|d| {
                            d.with_msgs_hl(
                                msgid,
                                id_emails.iter().map(|m| (m.start, m.end)),
                                msgstr,
                                str_emails.iter().map(|m| (m.start, m.end)),
                            )
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
}
