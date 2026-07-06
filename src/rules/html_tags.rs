// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `html-tags` rule: check missing/extra/different HTML tags.

use std::collections::HashSet;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Edit, Fix, FixTarget};
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
    /// Diagnostics reported:
    /// - [`warning`](Severity::Warning): `missing HTML tags (# / #)`
    /// - [`warning`](Severity::Warning): `extra HTML tags (# / #)`
    /// - [`warning`](Severity::Warning): `different HTML tags` (auto-fixable)
    ///
    /// Only the `different HTML tags` diagnostic carries an auto-fix: each
    /// translation tag is replaced in place with the tag at the same position
    /// in the source. The `missing` and `extra` cases are left unfixed because
    /// inserting a missing tag at the right place in the prose or choosing which
    /// extra to drop both require translator judgement.
    fn check_msg(
        &self,
        checker: &Checker,
        entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let id_tags: Vec<_> = FormatHtmlTagPos::new(&msgid.value, entry.format_language).collect();
        let str_tags: Vec<_> =
            FormatHtmlTagPos::new(&msgstr.value, entry.format_language).collect();
        match id_tags.len().cmp(&str_tags.len()) {
            std::cmp::Ordering::Greater => self
                .new_diag(
                    checker,
                    Severity::Warning,
                    format!("missing HTML tags ({} / {})", id_tags.len(), str_tags.len()),
                )
                .map(|d| {
                    d.with_msgs_hl(
                        msgid,
                        id_tags.iter().map(|m| (m.start, m.end)),
                        msgstr,
                        str_tags.iter().map(|m| (m.start, m.end)),
                    )
                })
                .into_iter()
                .collect(),
            std::cmp::Ordering::Less => self
                .new_diag(
                    checker,
                    Severity::Warning,
                    format!("extra HTML tags ({} / {})", id_tags.len(), str_tags.len()),
                )
                .map(|d| {
                    d.with_msgs_hl(
                        msgid,
                        id_tags.iter().map(|m| (m.start, m.end)),
                        msgstr,
                        str_tags.iter().map(|m| (m.start, m.end)),
                    )
                })
                .into_iter()
                .collect(),
            std::cmp::Ordering::Equal => {
                // Check that HTML tags are the same, in any order.
                let id_tags_hash: HashSet<_> = id_tags.iter().map(|m| m.s).collect();
                let str_tags_hash: HashSet<_> = str_tags.iter().map(|m| m.s).collect();
                if id_tags_hash == str_tags_hash {
                    vec![]
                } else {
                    // Auto-fix: replace each translation tag with the source tag
                    // at the same position, for the positions where they differ.
                    let edits: Vec<Edit> = id_tags
                        .iter()
                        .zip(str_tags.iter())
                        .filter(|(id, str)| id.s != str.s)
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
                    self.new_diag(checker, Severity::Warning, "different HTML tags")
                        .map(|d| {
                            d.with_msgs_hl(
                                msgid,
                                id_tags
                                    .iter()
                                    .filter(|m| !str_tags_hash.contains(m.s))
                                    .map(|m| (m.start, m.end)),
                                msgstr,
                                str_tags
                                    .iter()
                                    .filter(|m| !id_tags_hash.contains(m.s))
                                    .map(|m| (m.start, m.end)),
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
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "missing HTML tags (2 / 1)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "extra HTML tags (2 / 3)");
        let diag = &diags[2];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "different HTML tags");
    }

    #[test]
    fn test_different_html_tags_fix_replaces_each_in_place() {
        let diags = check_html_tags(
            r#"
msgid "Hello <b>world</b>"
msgstr "Bonjour <i>monde</i>"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "different HTML tags");
        let fix = diags[0].fix.as_ref().expect("fix attached");
        // Both tags differ: <i> -> <b> and </i> -> </b>.
        assert_eq!(fix.edits.len(), 2);
        assert_eq!(fix.edits[0].replacement, "<b>");
        assert_eq!(fix.edits[1].replacement, "</b>");
    }

    #[test]
    fn test_different_html_tags_fix_skips_positions_already_equal() {
        // The opening <b> matches; only the mismatched closing tag is fixed.
        let diags = check_html_tags(
            r#"
msgid "<b>Hello</b>"
msgstr "<b>Bonjour</i>"
"#,
        );
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].replacement, "</b>");
    }

    #[test]
    fn test_missing_and_extra_html_tags_have_no_fix() {
        let diags = check_html_tags(
            r#"
msgid "Hello <b>world</b>"
msgstr "Bonjour <b>monde"

msgid "Hello <b>world</b>"
msgstr "Bonjour <b>monde</b><br/>"
"#,
        );
        assert_eq!(diags.len(), 2);
        assert!(
            diags[0].fix.is_none(),
            "missing HTML tags diagnostic must not carry a fix"
        );
        assert!(
            diags[1].fix.is_none(),
            "extra HTML tags diagnostic must not carry a fix"
        );
    }

    #[test]
    fn test_different_html_tags_highlights_only_differing() {
        // <b>/</b> appear in both messages, so they must NOT be highlighted;
        // only the differing tags (<i>/</i> in the translation) are.
        let diags = check_html_tags(
            r#"
msgid "<b>Hi</b> and <i>bye</i>"
msgstr "<b>Salut</b> et <u>adieu</u>"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "different HTML tags");
        // `with_msgs_hl` adds 3 lines: msgid, an empty separator, then msgstr.
        assert_eq!(diags[0].lines.len(), 3);
        let id_hl: Vec<_> = diags[0].lines[0]
            .highlights
            .iter()
            .map(|(s, e)| &diags[0].lines[0].message[*s..*e])
            .collect();
        let str_hl: Vec<_> = diags[0].lines[2]
            .highlights
            .iter()
            .map(|(s, e)| &diags[0].lines[2].message[*s..*e])
            .collect();
        assert_eq!(id_hl, vec!["<i>", "</i>"]);
        assert_eq!(str_hl, vec!["<u>", "</u>"]);
    }
}
