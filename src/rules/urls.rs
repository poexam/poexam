// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `urls` rule: check missing/extra/different URLs.

use std::collections::HashSet;

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::po::entry::Entry;
use crate::po::format::iter::FormatUrlPos;
use crate::rules::double_quotes::DOUBLE_QUOTES;
use crate::rules::rule::RuleChecker;

pub struct UrlsRule;

impl RuleChecker for UrlsRule {
    fn name(&self) -> &'static str {
        "urls"
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

    /// Check for missing, extra or different URLs in the translation.
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
    /// Diagnostics reported with severity [`warning`](Severity::Warning):
    /// - `missing URLs (# / #)`
    /// - `extra URLs (# / #)`
    /// - `different URLs`
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        let id_urls: Vec<_> = FormatUrlPos::new(msgid, &entry.format_language).collect();
        let str_urls: Vec<_> = FormatUrlPos::new(msgstr, &entry.format_language).collect();
        match id_urls.len().cmp(&str_urls.len()) {
            std::cmp::Ordering::Greater => {
                checker.report_id_str(
                    entry,
                    format!("missing URLs ({} / {})", id_urls.len(), str_urls.len()),
                    msgid,
                    &id_urls.iter().map(|m| (m.start, m.end)).collect::<Vec<_>>(),
                    msgstr,
                    &str_urls
                        .iter()
                        .map(|m| (m.start, m.end))
                        .collect::<Vec<_>>(),
                );
            }
            std::cmp::Ordering::Less => {
                checker.report_id_str(
                    entry,
                    format!("extra URLs ({} / {})", id_urls.len(), str_urls.len()),
                    msgid,
                    &id_urls.iter().map(|m| (m.start, m.end)).collect::<Vec<_>>(),
                    msgstr,
                    &str_urls
                        .iter()
                        .map(|m| (m.start, m.end))
                        .collect::<Vec<_>>(),
                );
            }
            std::cmp::Ordering::Equal => {
                // Check that URLs are the same, in any order.
                // A single pair of quotes is skipped from both sides of the URL.
                let id_urls_hash: HashSet<_> = id_urls.iter().map(|m| trim_quotes(m.s)).collect();
                let str_urls_hash: HashSet<_> = str_urls.iter().map(|m| trim_quotes(m.s)).collect();
                if id_urls_hash != str_urls_hash {
                    checker.report_id_str(
                        entry,
                        "different URLs".to_string(),
                        msgid,
                        &id_urls.iter().map(|m| (m.start, m.end)).collect::<Vec<_>>(),
                        msgstr,
                        &str_urls
                            .iter()
                            .map(|m| (m.start, m.end))
                            .collect::<Vec<_>>(),
                    );
                }
            }
        }
    }
}

/// Trim one pair of quotes from both sides of the URL, if any.
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
}
