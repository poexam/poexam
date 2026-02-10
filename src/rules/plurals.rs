// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `plurals` rule: check incorrect number of plurals.

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct PluralsRule {}

impl RuleChecker for PluralsRule {
    fn name(&self) -> &'static str {
        "plurals"
    }

    fn is_default(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Error
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
    /// Diagnostics reported with severity [`error`](Severity::Error):
    /// - `missing translated plural form (found: #, expected: #)`
    /// - `extra translated plural form (found: #, expected: #)`
    fn check_entry(&self, checker: &mut Checker, entry: &Entry) {
        let nplurals_expected = checker.nplurals() as usize;
        if nplurals_expected == 0 || !entry.has_plural_form() {
            // We check only entries with plural form and when nplurals is defined.
            return;
        }
        let nplurals_found = entry.msgstr.len();
        if nplurals_found < nplurals_expected {
            checker.report_entry(
                format!(
                    "missing translated plural form (found: {nplurals_found}, expected: {nplurals_expected})",
                ),
                entry,
            );
        } else if nplurals_found > nplurals_expected {
            checker.report_entry(
                format!(
                    "extra translated plural form (found: {nplurals_found}, expected: {nplurals_expected})",
                ),
                entry,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_plurals(content: &str) -> Vec<Diagnostic> {
        let rules = Rules::new(vec![Box::new(PluralsRule {})]);
        let mut checker = Checker::new(content.as_bytes(), &rules);
        checker.do_all_checks();
        checker.diagnostics
    }

    #[test]
    fn test_no_plural() {
        let diags = check_plurals(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_plural_ok() {
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
        println!("diags: {diags:?}");
        assert!(diags.is_empty());
    }

    #[test]
    fn test_plural_error() {
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
}
