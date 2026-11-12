// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `previous-msgid` rule: check for previous strings
//! (`#| msgid "…"`) on entries not fuzzy.

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Fix, FixTarget};
use crate::po::entry::{CommentKinds, Entry};
use crate::rules::rule::RuleChecker;

pub struct PreviousMsgidRule;

impl RuleChecker for PreviousMsgidRule {
    fn name(&self) -> &'static str {
        "previous-msgid"
    }

    fn description(&self) -> &'static str {
        "Check for previous strings (#| msgid) on entries not fuzzy."
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    fn checks_all_entries(&self) -> bool {
        true
    }

    fn uses_comments(&self) -> CommentKinds {
        CommentKinds::PREVIOUS
    }

    /// Check for previous strings (`#| msgctxt`, `#| msgid` and
    /// `#| msgid_plural`) on entries not fuzzy.
    ///
    /// `msgmerge` (option `--previous`) adds the previous source strings to
    /// the fuzzy entries, to show the translator what changed in the source.
    /// Once the translation is reviewed, the fuzzy flag must be removed with
    /// the previous strings: if only the flag is removed (e.g. by hand), the
    /// previous strings are stale.
    ///
    /// Untranslated entries are checked too.
    ///
    /// Wrong entry:
    /// ```text
    /// #| msgid "Open file"
    /// msgid "Open the file"
    /// msgstr "Ouvrir le fichier"
    /// ```
    ///
    /// Correct entries:
    /// ```text
    /// msgid "Open the file"
    /// msgstr "Ouvrir le fichier"
    ///
    /// #, fuzzy
    /// #| msgid "Open file"
    /// msgid "Open the file"
    /// msgstr "Ouvrir un fichier"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`info`](Severity::Info): `previous strings on an entry not fuzzy`
    ///   (auto-fixable)
    fn check_entry(&self, checker: &Checker, entry: &Entry) -> Vec<Diagnostic> {
        if entry.fuzzy || entry.obsolete || entry.previous.is_empty() {
            return vec![];
        }
        let Some(mut diag) = self.new_diag(
            checker,
            Severity::Info,
            "previous strings on an entry not fuzzy",
        ) else {
            return vec![];
        };
        for comment in &entry.previous {
            diag.add_line(comment.line_number, format!("#| {}", comment.value), []);
        }
        if let Some(msgid) = &entry.msgid {
            diag = diag.with_msg(msgid);
        }
        // The lines are deleted only if they are contiguous.
        let range = entry.previous_byte_range.clone();
        let lines = checker
            .data()
            .get(range.clone())
            .map_or(0, |bytes| memchr::memchr_iter(b'\n', bytes).count());
        let fix = (lines == entry.previous.len()).then_some(Fix {
            target: FixTarget::Block {
                file_byte_range: range,
                bytes: vec![],
            },
            edits: vec![],
            safe: true,
        });
        vec![diag.with_optional_fix(fix)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::rule::Rules;

    const HEADER: &str = "msgid \"\"\nmsgstr \"Content-Type: text/plain; charset=UTF-8\\n\"\n\n";

    fn check(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(PreviousMsgidRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    /// Return the byte range deleted by the fix of a diagnostic.
    fn fix_range(diag: &Diagnostic) -> Option<std::ops::Range<usize>> {
        match &diag.fix.as_ref()?.target {
            FixTarget::Block {
                file_byte_range,
                bytes,
            } => {
                assert!(bytes.is_empty());
                Some(file_byte_range.clone())
            }
            _ => None,
        }
    }

    #[test]
    fn test_no_error() {
        let content = format!(
            "{HEADER}msgid \"Open the file\"\nmsgstr \"Ouvrir le fichier\"\n\n\
             #, fuzzy\n#| msgid \"Open file\"\nmsgid \"Open\"\nmsgstr \"Ouvrir\"\n\n\
             #, fuzzy\n#~| msgid \"Old\"\n#~ msgid \"Obsolete\"\n#~ msgstr \"Obsolète\"\n"
        );
        assert!(check(&content).is_empty());
    }

    #[test]
    fn test_error() {
        let content = format!(
            "{HEADER}#: src/main.c:42\n#| msgctxt \"menu\"\n#| msgid \"Open file\"\n\
             msgctxt \"menu\"\nmsgid \"Open the file\"\nmsgstr \"Ouvrir le fichier\"\n\n\
             #, c-format\n#| msgid \"%d file\"\n#| msgid_plural \"%d files\"\n\
             msgid \"%d item\"\nmsgid_plural \"%d items\"\nmsgstr[0] \"\"\nmsgstr[1] \"\"\n"
        );
        let diags = check(&content);
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].severity, Severity::Info);
        assert_eq!(diags[0].message, "previous strings on an entry not fuzzy");
        assert_eq!(diags[0].lines.len(), 3);
        assert_eq!(diags[0].lines[0].line_number, 5);
        assert_eq!(diags[0].lines[0].message, "#| msgctxt \"menu\"");
        let start = content.find("#| msgctxt").unwrap();
        let end = content.find("msgctxt \"menu\"\nmsgid").unwrap();
        assert_eq!(fix_range(&diags[0]), Some(start..end));
        // Untranslated entry.
        let start = content.find("#| msgid \"%d").unwrap();
        let end = content.find("msgid \"%d item").unwrap();
        assert_eq!(fix_range(&diags[1]), Some(start..end));
    }

    #[test]
    fn test_not_contiguous() {
        let content = format!(
            "{HEADER}#| msgid \"Open file\"\n# comment\n#| msgid_plural \"Open files\"\n\
             msgid \"Open the file\"\nmsgstr \"Ouvrir le fichier\"\n"
        );
        let diags = check(&content);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].fix.is_none());
    }

    #[test]
    fn test_noqa() {
        let content = format!(
            "{HEADER}#, noqa:previous-msgid\n#| msgid \"Open file\"\n\
             msgid \"Open the file\"\nmsgstr \"Ouvrir le fichier\"\n"
        );
        assert!(check(&content).is_empty());
    }
}
