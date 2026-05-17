// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `urls` rule: check missing/extra/different URLs.

use std::collections::HashSet;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Edit, Fix, FixTarget};
use crate::po::entry::Entry;
use crate::po::format::iter::FormatUrlPos;
use crate::po::message::Message;
use crate::rules::double_quotes::trim_quotes;
use crate::rules::rule::RuleChecker;

pub struct UrlsRule;

impl RuleChecker for UrlsRule {
    fn name(&self) -> &'static str {
        "urls"
    }

    fn description(&self) -> &'static str {
        "Check for missing, extra or different URLs in translation."
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for missing, extra or different URLs in the translation.
    ///
    /// This rule is not enabled by default.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "Test URL: https://example.com"
    /// msgstr "URL de test : https://example.com/extra"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "Test URL: https://example.com"
    /// msgstr "URL de test : https://example.com"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`warning`](Severity::Warning): `missing URLs (# / #)`
    /// - [`warning`](Severity::Warning): `extra URLs (# / #)`
    /// - [`warning`](Severity::Warning): `different URLs` (auto-fixable)
    ///
    /// Only the `different URLs` diagnostic carries an auto-fix: each
    /// translation URL is replaced in place with the URL at the same
    /// position in the source. The `missing` and `extra` cases are left
    /// unfixed because inserting a missing URL at the right position in
    /// the prose or choosing which extra to drop both require translator
    /// judgement.
    fn check_msg(
        &self,
        checker: &Checker,
        entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let id_urls: Vec<_> = FormatUrlPos::new(&msgid.value, entry.format_language).collect();
        let str_urls: Vec<_> = FormatUrlPos::new(&msgstr.value, entry.format_language).collect();
        match id_urls.len().cmp(&str_urls.len()) {
            std::cmp::Ordering::Greater => self
                .new_diag(
                    checker,
                    Severity::Warning,
                    format!("missing URLs ({} / {})", id_urls.len(), str_urls.len()),
                )
                .map(|d| {
                    d.with_msgs_hl(
                        msgid,
                        id_urls.iter().map(|m| (m.start, m.end)),
                        msgstr,
                        str_urls.iter().map(|m| (m.start, m.end)),
                    )
                })
                .into_iter()
                .collect(),
            std::cmp::Ordering::Less => self
                .new_diag(
                    checker,
                    Severity::Warning,
                    format!("extra URLs ({} / {})", id_urls.len(), str_urls.len()),
                )
                .map(|d| {
                    d.with_msgs_hl(
                        msgid,
                        id_urls.iter().map(|m| (m.start, m.end)),
                        msgstr,
                        str_urls.iter().map(|m| (m.start, m.end)),
                    )
                })
                .into_iter()
                .collect(),
            std::cmp::Ordering::Equal => {
                // Check that URLs are the same, in any order.
                // A single pair of quotes is skipped from both sides of the URL.
                let id_urls_hash: HashSet<_> = id_urls.iter().map(|m| trim_quotes(m.s)).collect();
                let str_urls_hash: HashSet<_> = str_urls.iter().map(|m| trim_quotes(m.s)).collect();
                if id_urls_hash == str_urls_hash {
                    vec![]
                } else {
                    let edits: Vec<Edit> = id_urls
                        .iter()
                        .zip(str_urls.iter())
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
                    });
                    self.new_diag(checker, Severity::Warning, "different URLs")
                        .map(|d| {
                            d.with_msgs_hl(
                                msgid,
                                id_urls.iter().map(|m| (m.start, m.end)),
                                msgstr,
                                str_urls.iter().map(|m| (m.start, m.end)),
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

    fn check_urls(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(UrlsRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_no_urls() {
        let diags = check_urls(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_urls_ok() {
        let diags = check_urls(
            // Order of URLs is not checked.
            r#"
msgid "https://example.com/ -- „https://google.com”"
msgstr "https://google.com -- „https://example.com/”"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_urls_error() {
        let diags = check_urls(
            r#"
msgid "missing URL: https://example.com -- http://google.com"
msgstr "URL manquante : https://example.com"

msgid "extra URL: https://example.com"
msgstr "URL extra : https://example.com -- http://google.com""

msgid "different URLs: https://example.com -- http://google.com"
msgstr "URLs différentes : https://exampe.com/test -- http://google.com"
"#,
        );
        assert_eq!(diags.len(), 3);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "missing URLs (2 / 1)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "extra URLs (1 / 2)");
        let diag = &diags[2];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "different URLs");
    }

    #[test]
    fn test_different_urls_fix_replaces_each_in_place() {
        let diags = check_urls(
            r#"
msgid "See https://a.com or https://b.com"
msgstr "Voir https://a2.com ou https://b2.com"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "different URLs");
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 2);
        assert_eq!(fix.edits[0].replacement, "https://a.com");
        assert_eq!(fix.edits[1].replacement, "https://b.com");
    }

    #[test]
    fn test_different_urls_fix_skips_positions_already_equal() {
        let diags = check_urls(
            r#"
msgid "See https://a.com or https://b.com"
msgstr "Voir https://a.com ou https://b2.com"
"#,
        );
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].replacement, "https://b.com");
    }

    #[test]
    fn test_missing_and_extra_urls_have_no_fix() {
        let diags = check_urls(
            r#"
msgid "See https://a.com or https://b.com"
msgstr "Voir https://a.com"

msgid "See https://a.com"
msgstr "Voir https://a.com or https://b.com"
"#,
        );
        assert_eq!(diags.len(), 2);
        assert!(
            diags[0].fix.is_none(),
            "missing URLs diagnostic must not carry a fix"
        );
        assert!(
            diags[1].fix.is_none(),
            "extra URLs diagnostic must not carry a fix"
        );
    }
}
