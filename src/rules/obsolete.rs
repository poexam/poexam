// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `obsolete` rule: report obsolete entries.

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Fix, FixTarget};
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct ObsoleteRule;

impl RuleChecker for ObsoleteRule {
    fn name(&self) -> &'static str {
        "obsolete"
    }

    fn description(&self) -> &'static str {
        "Report obsolete entries."
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        false
    }

    /// Report entry if obsolete.
    ///
    /// Obsolete is not strictly speaking an error, but this check helps to identify
    /// obsolete entries in a PO file.
    ///
    /// This rule is not enabled by default.
    ///
    /// Reported:
    /// ```text
    /// #~ msgid "this is a test"
    /// #~ msgstr "ceci est un test"
    /// ```
    ///
    /// Not reported:
    /// ```text
    /// msgid "this is a test"
    /// msgstr "ceci est un test"
    /// ```
    ///
    /// Diagnostics reported (auto-fixable — the fix deletes the entire entry
    /// from the file, including any leading comments and the trailing
    /// blank-line separator):
    /// - [`info`](Severity::Info): `obsolete entry` (auto-fixable)
    fn check_entry(&self, checker: &Checker, entry: &Entry) -> Vec<Diagnostic> {
        if entry.obsolete {
            let fix = Fix {
                target: FixTarget::Entry {
                    file_byte_range: entry.byte_range.clone(),
                },
                edits: Vec::new(),
                safe: true,
            };
            self.new_diag(checker, Severity::Info, "obsolete entry")
                .map(|d| d.with_entry(entry).with_fix(fix))
                .into_iter()
                .collect()
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_obsolete(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(ObsoleteRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_not_obsolete() {
        let diags = check_obsolete(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_obsolete_error_noqa() {
        let diags = check_obsolete(
            r#"
#, noqa:obsolete
#~ msgid "tested"
#~ msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_obsolete_error() {
        let diags = check_obsolete(
            r#"
#~ msgid "tested"
#~ msgstr "testé"
"#,
        );
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "obsolete entry");
    }

    #[test]
    fn test_obsolete_fix_targets_entire_entry() {
        let content = "\n#~ msgid \"tested\"\n#~ msgstr \"testé\"\n";
        let diags = check_obsolete(content);
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix attached");
        let FixTarget::Entry { file_byte_range } = &fix.target else {
            panic!("expected FixTarget::Entry, got {:?}", fix.target);
        };
        // Edits are unused for entry deletion.
        assert!(fix.edits.is_empty());
        // The byte range must cover both `#~` lines (and the parser includes
        // the trailing newline / blank-line separator when present).
        let slice = &content[file_byte_range.clone()];
        assert!(slice.starts_with("#~ msgid"));
        assert!(slice.contains("#~ msgstr"));
    }

    #[test]
    fn test_obsolete_fix_covers_leading_comments() {
        // The entry's comments must be part of the byte range so the fix
        // deletes them too — leaving comments alone would leave dangling
        // metadata in the file.
        let content =
            "\n# translator note\n# another comment\n#~ msgid \"tested\"\n#~ msgstr \"testé\"\n";
        let diags = check_obsolete(content);
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix attached");
        let FixTarget::Entry { file_byte_range } = &fix.target else {
            panic!("expected FixTarget::Entry");
        };
        let slice = &content[file_byte_range.clone()];
        assert!(slice.contains("# translator note"));
        assert!(slice.contains("# another comment"));
        assert!(slice.contains("#~ msgid"));
    }
}
