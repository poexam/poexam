// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `emails` rule: check missing/extra/different emails.

use std::collections::HashSet;

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::po::entry::Entry;
use crate::po::format::iter::FormatEmailPos;
use crate::rules::double_quotes::DOUBLE_QUOTES;
use crate::rules::rule::RuleChecker;

pub struct EmailsRule;

impl RuleChecker for EmailsRule {
    fn name(&self) -> &'static str {
        "emails"
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
    /// Diagnostics reported with severity [`info`](Severity::Info):
    /// - `missing emails (# / #)`
    /// - `extra emails (# / #)`
    /// - `different emails`
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        let id_emails: Vec<_> = FormatEmailPos::new(msgid, &entry.format_language).collect();
        let str_emails: Vec<_> = FormatEmailPos::new(msgstr, &entry.format_language).collect();
        match id_emails.len().cmp(&str_emails.len()) {
            std::cmp::Ordering::Greater => {
                checker.report_id_str(
                    entry,
                    format!(
                        "missing emails ({} / {})",
                        id_emails.len(),
                        str_emails.len()
                    ),
                    msgid,
                    &id_emails
                        .iter()
                        .map(|m| (m.start, m.end))
                        .collect::<Vec<_>>(),
                    msgstr,
                    &str_emails
                        .iter()
                        .map(|m| (m.start, m.end))
                        .collect::<Vec<_>>(),
                );
            }
            std::cmp::Ordering::Less => {
                checker.report_id_str(
                    entry,
                    format!("extra emails ({} / {})", id_emails.len(), str_emails.len()),
                    msgid,
                    &id_emails
                        .iter()
                        .map(|m| (m.start, m.end))
                        .collect::<Vec<_>>(),
                    msgstr,
                    &str_emails
                        .iter()
                        .map(|m| (m.start, m.end))
                        .collect::<Vec<_>>(),
                );
            }
            std::cmp::Ordering::Equal => {
                // Check that emails are the same, in any order.
                // A single pair of quotes is skipped from both sides of the email.
                let id_emails_hash: HashSet<_> =
                    id_emails.iter().map(|m| trim_quotes(m.s)).collect();
                let str_emails_hash: HashSet<_> =
                    str_emails.iter().map(|m| trim_quotes(m.s)).collect();
                if id_emails_hash != str_emails_hash {
                    checker.report_id_str(
                        entry,
                        "different emails".to_string(),
                        msgid,
                        &id_emails
                            .iter()
                            .map(|m| (m.start, m.end))
                            .collect::<Vec<_>>(),
                        msgstr,
                        &str_emails
                            .iter()
                            .map(|m| (m.start, m.end))
                            .collect::<Vec<_>>(),
                    );
                }
            }
        }
    }
}

/// Trim one pair of quotes from both sides of the email, if any.
///
/// The quote skipped at the beginning may be different from the quote at the end.
fn trim_quotes(s: &str) -> &str {
    if s.starts_with(DOUBLE_QUOTES) && s.ends_with(DOUBLE_QUOTES) {
        // Return the string without the first and last UTF-8 char.
        let start = s.chars().next().unwrap().len_utf8();
        let end = s.char_indices().next_back().unwrap().0;
        return &s[start..end];
    }
    s
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
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "missing emails (2 / 1)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "extra emails (1 / 2)");
        let diag = &diags[2];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "different emails");
    }
}
