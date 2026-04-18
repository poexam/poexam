// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `html-tags` rule: check missing/extra/different HTML tags.

use std::collections::HashSet;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::po::format::iter::FormatHtmlTagPos;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct HtmlTagsRule;

impl RuleChecker for HtmlTagsRule {
    fn name(&self) -> &'static str {
        "html-tags"
    }

    fn description(&self) -> &'static str {
        "Check for missing, extra or different HTML tags in translation."
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

    /// Check for missing, extra or different HTML tags in the translation.
    ///
    /// This rule is not enabled by default.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "Hello <b>world</b>"
    /// msgstr "Bonjour <b>monde"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "Hello <b>world</b>"
    /// msgstr "Bonjour <b>monde</b>"
    /// ```
    ///
    /// Diagnostics reported with severity [`info`](Severity::Info):
    /// - `missing HTML tags (# / #)`
    /// - `extra HTML tags (# / #)`
    /// - `different HTML tags`
    fn check_msg(
        &self,
        checker: &Checker,
        entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let id_tags: Vec<_> = FormatHtmlTagPos::new(&msgid.value, &entry.format_language).collect();
        let str_tags: Vec<_> =
            FormatHtmlTagPos::new(&msgstr.value, &entry.format_language).collect();
        match id_tags.len().cmp(&str_tags.len()) {
            std::cmp::Ordering::Greater => {
                vec![
                    self.new_diag(
                        checker,
                        format!("missing HTML tags ({} / {})", id_tags.len(), str_tags.len()),
                    )
                    .with_msgs_hl(
                        msgid,
                        &id_tags.iter().map(|m| (m.start, m.end)).collect::<Vec<_>>(),
                        msgstr,
                        &str_tags
                            .iter()
                            .map(|m| (m.start, m.end))
                            .collect::<Vec<_>>(),
                    ),
                ]
            }
            std::cmp::Ordering::Less => {
                vec![
                    self.new_diag(
                        checker,
                        format!("extra HTML tags ({} / {})", id_tags.len(), str_tags.len()),
                    )
                    .with_msgs_hl(
                        msgid,
                        &id_tags.iter().map(|m| (m.start, m.end)).collect::<Vec<_>>(),
                        msgstr,
                        &str_tags
                            .iter()
                            .map(|m| (m.start, m.end))
                            .collect::<Vec<_>>(),
                    ),
                ]
            }
            std::cmp::Ordering::Equal => {
                // Check that HTML tags are the same, in any order.
                let id_tags_hash: HashSet<_> = id_tags.iter().map(|m| m.s).collect();
                let str_tags_hash: HashSet<_> = str_tags.iter().map(|m| m.s).collect();
                if id_tags_hash == str_tags_hash {
                    vec![]
                } else {
                    vec![
                        self.new_diag(checker, "different HTML tags".to_string())
                            .with_msgs_hl(
                                msgid,
                                &id_tags.iter().map(|m| (m.start, m.end)).collect::<Vec<_>>(),
                                msgstr,
                                &str_tags
                                    .iter()
                                    .map(|m| (m.start, m.end))
                                    .collect::<Vec<_>>(),
                            ),
                    ]
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_html_tags(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(HtmlTagsRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_no_html_tags() {
        let diags = check_html_tags(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_html_tags_ok() {
        let diags = check_html_tags(
            r#"
msgid "Hello <b>world</b>"
msgstr "Bonjour <b>monde</b>"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_html_tags_ok_different_order() {
        // Order of HTML tags is not checked.
        let diags = check_html_tags(
            r#"
msgid "<b>Hello</b> <i>world</i>"
msgstr "<i>monde</i> <b>Bonjour</b>"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_html_tags_ok_with_attributes() {
        let diags = check_html_tags(
            r#"
msgid "Click <a href="https://example.com">here</a>"
msgstr "Cliquez <a href="https://example.com">ici</a>"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_html_tags_ok_self_closing() {
        let diags = check_html_tags(
            r#"
msgid "Line 1<br/>Line 2"
msgstr "Ligne 1<br/>Ligne 2"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_html_tags_errors() {
        let diags = check_html_tags(
            r#"
msgid "Hello <b>world</b>"
msgstr "Bonjour <b>monde"

msgid "Hello <b>world</b>"
msgstr "Bonjour <b>monde</b><br/>"

msgid "Hello <b>world</b>"
msgstr "Bonjour <i>monde</i>"
"#,
        );
        assert_eq!(diags.len(), 3);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "missing HTML tags (2 / 1)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "extra HTML tags (2 / 3)");
        let diag = &diags[2];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "different HTML tags");
    }
}
