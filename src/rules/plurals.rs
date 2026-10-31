// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the rules `plurals` (check incorrect number of plurals),
//! `plurals-empty` (check empty plural form among translated ones) and
//! `plurals-same` (check identical plural forms).

use std::fmt::Write as _;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct PluralsRule;
pub struct PluralsEmptyRule;
pub struct PluralsSameRule;

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

impl RuleChecker for PluralsSameRule {
    fn name(&self) -> &'static str {
        "plurals-same"
    }

    fn description(&self) -> &'static str {
        "Check for identical plural forms in translation when source forms differ."
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for identical plural forms in translation while `msgid` and `msgid_plural` differ.
    ///
    /// This often means the singular form was copied into the other forms and not adapted.
    ///
    /// Entries with a single translated form (e.g. `nplurals=1`), with an empty form (reported
    /// by the `plurals-empty` and `untranslated` rules) or with identical source forms are not
    /// reported.
    ///
    /// The rule is not enabled by default because identical forms are legit in some cases:
    /// invariable words (e.g. French "%d fois") or languages that don't mark the plural after
    /// a number (e.g. Turkish, Hungarian).
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "%d file"
    /// msgid_plural "%d files"
    /// msgstr[0] "%d fichier"
    /// msgstr[1] "%d fichier"
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
    /// Diagnostics reported:
    /// - [`warning`](Severity::Warning): `identical plural forms in translation (#)`
    fn check_entry(&self, checker: &Checker, entry: &Entry) -> Vec<Diagnostic> {
        let (Some(msgid), Some(msgid_plural)) = (&entry.msgid, &entry.msgid_plural) else {
            return vec![];
        };
        if entry.msgstr.len() < 2 || msgid.value == msgid_plural.value {
            return vec![];
        }
        let mut strs = entry.msgstr.values();
        let Some(first) = strs.next() else {
            return vec![];
        };
        if first.value.is_empty() || !strs.all(|msgstr| msgstr.value == first.value) {
            return vec![];
        }
        self.new_diag(
            checker,
            Severity::Warning,
            format!(
                "identical plural forms in translation ({})",
                entry.msgstr.len()
            ),
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

    fn check_plurals_same(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(PluralsSameRule {})]);
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

    #[test]
    fn test_plurals_same_ok() {
        // Different plural forms.
        let diags = check_plurals_same(
            r#"
msgid "%d file"
msgid_plural "%d files"
msgstr[0] "%d fichier"
msgstr[1] "%d fichiers"
"#,
        );
        assert!(diags.is_empty());
        // Entry without plural form.
        let diags = check_plurals_same(
            r#"
msgid "tested"
msgstr "tested"
"#,
        );
        assert!(diags.is_empty());
        // Only some forms identical (3 forms).
        let diags = check_plurals_same(
            r#"
msgid "%d file"
msgid_plural "%d files"
msgstr[0] "%d файл"
msgstr[1] "%d файла"
msgstr[2] "%d файла"
"#,
        );
        assert!(diags.is_empty());
        // Single form (nplurals=1).
        let diags = check_plurals_same(
            r#"
msgid "%d file"
msgid_plural "%d files"
msgstr[0] "%d 個のファイル"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_plurals_same_identical_source() {
        // Identical source forms: identical translations are expected.
        let diags = check_plurals_same(
            r#"
msgid "%d sheep"
msgid_plural "%d sheep"
msgstr[0] "%d mouton"
msgstr[1] "%d mouton"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_plurals_same_empty() {
        // Untranslated or partially translated entries are reported by other rules.
        let diags = check_plurals_same(
            r#"
msgid "%d file"
msgid_plural "%d files"
msgstr[0] ""
msgstr[1] ""
"#,
        );
        assert!(diags.is_empty());
        let diags = check_plurals_same(
            r#"
msgid "%d file"
msgid_plural "%d files"
msgstr[0] "%d fichier"
msgstr[1] ""
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_plurals_same_error_noqa() {
        let diags = check_plurals_same(
            r#"
#, noqa:plurals-same
msgid "%d time"
msgid_plural "%d times"
msgstr[0] "%d fois"
msgstr[1] "%d fois"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_plurals_same_error() {
        let diags = check_plurals_same(
            r#"
msgid "%d file"
msgid_plural "%d files"
msgstr[0] "%d fichier"
msgstr[1] "%d fichier"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Warning);
        assert_eq!(
            diags[0].message,
            "identical plural forms in translation (2)"
        );
        let diags = check_plurals_same(
            r#"
msgid "%d file"
msgid_plural "%d files"
msgstr[0] "%d файл"
msgstr[1] "%d файл"
msgstr[2] "%d файл"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "identical plural forms in translation (3)"
        );
    }
}
