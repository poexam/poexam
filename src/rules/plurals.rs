// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the rules `plurals` (check incorrect number of plurals)
//! and `plurals-empty` (check empty plural form among translated ones).

use std::fmt::Write as _;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct PluralsRule;
pub struct PluralsEmptyRule;

impl RuleChecker for PluralsRule {
    fn name(&self) -> &'static str {
        "plurals"
    }

    fn description(&self) -> &'static str {
        "Check for incorrect number of plural forms in translation."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for incorrect number of plurals in translation.
    ///
    /// The number of plurals is defined in the PO header like this:
    /// ```text
    /// "Plural-Forms: nplurals=2; plural=(n > 1);\n"
    /// ```
    ///
    /// If the `nplurals` value is not defined, this rule does not report any diagnostic.
    ///
    /// Wrong entry (with nplurals=2):
    /// ```text
    /// msgid "%d file"
    /// msgid_plural "%d files"
    /// msgstr[0] "%d fichier"
    /// ```
    ///
    /// Correct entry (with nplurals=2):
    /// ```text
    /// msgid "%d file"
    /// msgid_plural "%d files"
    /// msgstr[0] "%d fichier"
    /// msgstr[1] "%d fichiers"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`error`](Severity::Error): `missing translated plural form (found: #, expected: #)`
    /// - [`error`](Severity::Error): `extra translated plural form (found: #, expected: #)`
    fn check_entry(&self, checker: &Checker, entry: &Entry) -> Vec<Diagnostic> {
        let expected = checker.nplurals() as usize;
        if expected == 0 || !entry.has_plural_form() {
            // We check only entries with plural form and when nplurals is defined.
            return vec![];
        }
        let found = entry.msgstr.len();
        match found.cmp(&expected) {
            std::cmp::Ordering::Less => self
                .new_diag(
                    checker,
                    Severity::Error,
                    format!(
                        "missing translated plural form (found: {found}, expected: {expected})",
                    ),
                )
                .map(|d| d.with_entry(entry))
                .into_iter()
                .collect(),
            std::cmp::Ordering::Greater => self
                .new_diag(
                    checker,
                    Severity::Error,
                    format!("extra translated plural form (found: {found}, expected: {expected})"),
                )
                .map(|d| d.with_entry(entry))
                .into_iter()
                .collect(),
            std::cmp::Ordering::Equal => vec![],
        }
    }
}

impl RuleChecker for PluralsEmptyRule {
    fn name(&self) -> &'static str {
        "plurals-empty"
    }

    fn description(&self) -> &'static str {
        "Check for empty plural form in a partially translated entry."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for an empty plural form in an entry where another plural form is translated.
    ///
    /// Such an entry is considered as translated, so it is not reported by the `untranslated`
    /// rule, and the number of plural forms is correct, so it is not reported by the `plurals`
    /// rule either: the empty form is silently displayed as an empty string to the user.
    ///
    /// An entry with all plural forms empty is not reported by this rule (it is untranslated).
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "%d file"
    /// msgid_plural "%d files"
    /// msgstr[0] "%d fichier"
    /// msgstr[1] ""
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "%d file"
    /// msgid_plural "%d files"
    /// msgstr[0] "%d fichier"
    /// msgstr[1] "%d fichiers"
    /// ```
    ///
    /// All the empty forms of an entry are reported in a single diagnostic.
    ///
    /// Diagnostics reported:
    /// - [`error`](Severity::Error): `empty plural form msgstr[#]`
    /// - [`error`](Severity::Error): `empty plural forms msgstr[#], msgstr[#]`
    fn check_entry(&self, checker: &Checker, entry: &Entry) -> Vec<Diagnostic> {
        if entry.msgstr.len() < 2 || !entry.is_translated() {
            // We check only entries with multiple forms and at least one translated form.
            return vec![];
        }
        let mut forms = String::new();
        let mut count = 0;
        for (index, msgstr) in entry.iter_strs() {
            if msgstr.value.is_empty() {
                if count > 0 {
                    forms.push_str(", ");
                }
                let _ = write!(forms, "msgstr[{index}]");
                count += 1;
            }
        }
        if count == 0 {
            return vec![];
        }
        let plural = if count > 1 { "s" } else { "" };
        self.new_diag(
            checker,
            Severity::Error,
            format!("empty plural form{plural} {forms}"),
        )
        .map(|d| d.with_entry(entry))
        .into_iter()
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_plurals(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(PluralsRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    fn check_plurals_empty(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(PluralsEmptyRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_no_plurals() {
        let diags = check_plurals(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_plurals_ok() {
        let diags = check_plurals(
            r#"
msgid "%d file"
msgid_plural "%d files"
msgstr[0] "%d fichier"
msgstr[1] "%d fichiers"
"#,
        );
        assert!(diags.is_empty());
        let diags = check_plurals(
            r#"
msgid ""
msgstr ""
"Project-Id-Version: my_project\n"
"Plural-Forms: nplurals=2; plural=(n > 1);\n"

msgid "%d file"
msgid_plural "%d files"
msgstr[0] "%d fichier"
msgstr[1] "%d fichiers"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_plurals_error_noqa() {
        let diags = check_plurals(
            r#"
msgid ""
msgstr ""
"Project-Id-Version: my_project\n"
"Plural-Forms: nplurals=2; plural=(n > 1);\n"

#, noqa:plurals
msgid "%d file"
msgid_plural "%d files"
msgstr[0] "%d fichier"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_plurals_error() {
        let diags = check_plurals(
            r#"
msgid ""
msgstr ""
"Project-Id-Version: my_project\n"
"Plural-Forms: nplurals=2; plural=(n > 1);\n"

msgid "%d file"
msgid_plural "%d files"
msgstr[0] "%d fichier"
"#,
        );
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(
            diag.message,
            "missing translated plural form (found: 1, expected: 2)"
        );
        let diags = check_plurals(
            r#"
msgid ""
msgstr ""
"Project-Id-Version: my_project\n"
"Plural-Forms: nplurals=2; plural=(n > 1);\n"

msgid "%d file"
msgid_plural "%d files"
msgstr[0] "%d fichier"
msgstr[1] "%d fichiers"
msgstr[2] "%d fichiers"
"#,
        );
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(
            diag.message,
            "extra translated plural form (found: 3, expected: 2)"
        );
    }

    #[test]
    fn test_plurals_empty_ok() {
        // All plural forms translated.
        let diags = check_plurals_empty(
            r#"
msgid "%d file"
msgid_plural "%d files"
msgstr[0] "%d fichier"
msgstr[1] "%d fichiers"
"#,
        );
        assert!(diags.is_empty());
        // Entry without plural form.
        let diags = check_plurals_empty(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_plurals_empty_untranslated() {
        // No plural form translated at all: this is an untranslated entry, reported
        // by the `untranslated` rule, not by this one.
        let diags = check_plurals_empty(
            r#"
msgid "%d file"
msgid_plural "%d files"
msgstr[0] ""
msgstr[1] ""
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_plurals_empty_error_noqa() {
        let diags = check_plurals_empty(
            r#"
#, noqa:plurals-empty
msgid "%d file"
msgid_plural "%d files"
msgstr[0] "%d fichier"
msgstr[1] ""
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_plurals_empty_error() {
        let diags = check_plurals_empty(
            r#"
msgid "%d file"
msgid_plural "%d files"
msgstr[0] "%d fichier"
msgstr[1] ""
"#,
        );
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "empty plural form msgstr[1]");
        // The first form can be the empty one as well.
        let diags = check_plurals_empty(
            r#"
msgid "%d file"
msgid_plural "%d files"
msgstr[0] ""
msgstr[1] "%d fichiers"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "empty plural form msgstr[0]");
    }

    #[test]
    fn test_plurals_empty_error_multiple() {
        // A single diagnostic listing all the empty plural forms.
        let diags = check_plurals_empty(
            r#"
msgid ""
msgstr ""
"Project-Id-Version: my_project\n"
"Plural-Forms: nplurals=3; plural=(n == 0) ? 0 : ((n == 1) ? 1 : 2);\n"

msgid "%d file"
msgid_plural "%d files"
msgstr[0] "%d fichier"
msgstr[1] ""
msgstr[2] ""
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Error);
        assert_eq!(diags[0].message, "empty plural forms msgstr[1], msgstr[2]");
    }
}
