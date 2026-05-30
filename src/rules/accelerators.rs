// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `accelerators` rule: check missing/extra keyboard accelerators.

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::po::format::iter::FormatAcceleratorPos;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct AcceleratorsRule;

impl RuleChecker for AcceleratorsRule {
    fn name(&self) -> &'static str {
        "accelerators"
    }

    fn description(&self) -> &'static str {
        "Check for missing or extra keyboard accelerators in translation."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for missing or extra keyboard accelerators in the translation.
    ///
    /// An accelerator is the marker character (`&` by default, configurable with
    /// the `accelerator` option) immediately followed by an alphanumeric
    /// character; the doubled marker `&&` is an escaped literal ampersand. The
    /// rule only compares the *number* of accelerators, not the letter they
    /// target, because the accelerated letter legitimately differs per language
    /// (e.g. `&File` → `&Fichier`).
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "&File"
    /// msgstr "Fichier"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "&File"
    /// msgstr "&Fichier"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`warning`](Severity::Warning): `missing accelerators '&' (# / #)`
    /// - [`warning`](Severity::Warning): `extra accelerators '&' (# / #)`
    fn check_msg(
        &self,
        checker: &Checker,
        entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let marker = checker.config.check.accelerator;
        let id_accel: Vec<_> =
            FormatAcceleratorPos::new(&msgid.value, entry.format_language, marker).collect();
        let str_accel: Vec<_> =
            FormatAcceleratorPos::new(&msgstr.value, entry.format_language, marker).collect();
        let id_count = id_accel.len();
        let str_count = str_accel.len();
        let msg = match id_count.cmp(&str_count) {
            std::cmp::Ordering::Equal => return vec![],
            std::cmp::Ordering::Greater => {
                format!("missing accelerators '{marker}' ({id_count} / {str_count})")
            }
            std::cmp::Ordering::Less => {
                format!("extra accelerators '{marker}' ({id_count} / {str_count})")
            }
        };
        self.new_diag(checker, Severity::Warning, msg)
            .map(|d| {
                d.with_msgs_hl(
                    msgid,
                    id_accel.iter().map(|m| (m.start, m.end)),
                    msgstr,
                    str_accel.iter().map(|m| (m.start, m.end)),
                )
            })
            .into_iter()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_accelerators(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(AcceleratorsRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    /// Run the rule with a custom marker character configured.
    fn check_accelerators_marker(content: &str, marker: char) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        checker.config.check.accelerator = marker;
        let rules = Rules::new(vec![Box::new(AcceleratorsRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_no_accelerators() {
        let diags = check_accelerators(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_accelerators_ok() {
        let diags = check_accelerators(
            r#"
msgid "&File"
msgstr "&Fichier"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_accelerators_ok_different_letter() {
        // Only the count matters, not the accelerated letter.
        let diags = check_accelerators(
            r#"
msgid "E&xit"
msgstr "&Quitter"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_accelerators_escaped_ampersand_ok() {
        let diags = check_accelerators(
            r#"
msgid "Fish && Chips"
msgstr "Poisson && frites"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_accelerators_error_noqa() {
        let diags = check_accelerators(
            r#"
#, noqa:accelerators
msgid "&File"
msgstr "Fichier"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_accelerators_error() {
        let diags = check_accelerators(
            r#"
msgid "&File"
msgstr "Fichier"

msgid "Save"
msgstr "&Enregistrer"
"#,
        );
        assert_eq!(diags.len(), 2);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "missing accelerators '&' (1 / 0)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "extra accelerators '&' (0 / 1)");
    }

    #[test]
    fn test_accelerators_custom_marker() {
        // With marker '_' (GTK style), '&' is just a literal and '_' is the accelerator.
        let diags = check_accelerators_marker(
            r#"
msgid "_File"
msgstr "Fichier"
"#,
            '_',
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "missing accelerators '_' (1 / 0)");
    }

    #[test]
    fn test_accelerators_custom_marker_ignores_default() {
        // With marker '_', an entry that differs only in '&' count is not flagged.
        let diags = check_accelerators_marker(
            r#"
msgid "&File"
msgstr "Fichier"
"#,
            '_',
        );
        assert!(diags.is_empty());
    }
}
