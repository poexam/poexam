// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `formats` rule: check inconsistent format strings.

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::po::entry::Entry;
use crate::po::format::language::Language;
use crate::po::format::{
    format_pos::FormatPos,
    lang_c::{fmt_sort_index, fmt_strip_index},
};
use crate::rules::rule::RuleChecker;

pub struct FormatsRule;

impl RuleChecker for FormatsRule {
    fn name(&self) -> &'static str {
        "formats"
    }

    fn is_default(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    /// Check for inconsistent format strings.
    ///
    /// The following languages are supported:
    /// - C (`c-format`): printf format
    ///
    /// For the C format, the reordering of format specifiers is supported:
    /// `%3$d %1$s %2$f` is considered equivalent to `%s %f %d`.
    ///
    /// Wrong entries:
    /// ```text
    /// #, c-format
    /// msgid "name: %s, age: %d"
    /// msgstr "nom : %s, âge : %f"
    ///
    /// #, c-format
    /// msgid "%d test (%s)"
    /// msgstr "%2$d test (%1$s)"
    /// ```
    ///
    /// Correct entries:
    /// ```text
    /// #, c-format
    /// msgid "name: %s, age: %d"
    /// msgstr "nom : %s, âge : %d"
    ///
    /// #, c-format
    /// msgid "%d test (%s)"
    /// msgstr "%2$s test (%1$d)"
    /// ```
    ///
    /// Diagnostics reported with severity [`error`](Severity::Error):
    /// - `inconsistent format strings (xxx)`
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        if entry.format_language == Language::Null {
            return;
        }
        let id_fmt: Vec<_> = FormatPos::new(msgid, &entry.format_language).collect();
        let str_fmt: Vec<_> = FormatPos::new(msgstr, &entry.format_language).collect();
        let mut id_fmt_sorted = id_fmt.clone();
        let mut str_fmt_sorted = str_fmt.clone();
        id_fmt_sorted.sort_by_key(|m| (fmt_sort_index(m.s), m.start, m.end));
        str_fmt_sorted.sort_by_key(|m| (fmt_sort_index(m.s), m.start, m.end));
        let id_fmt2 = id_fmt_sorted
            .iter()
            .map(|m| fmt_strip_index(m.s))
            .collect::<Vec<String>>();
        let str_fmt2 = str_fmt_sorted
            .iter()
            .map(|m| fmt_strip_index(m.s))
            .collect::<Vec<String>>();
        if id_fmt2 != str_fmt2 {
            let pos_id: Vec<_> = id_fmt.iter().map(|m| (m.start, m.end)).collect();
            let pos_str: Vec<_> = str_fmt.iter().map(|m| (m.start, m.end)).collect();
            checker.report_msg(
                entry,
                format!("inconsistent format strings ({})", entry.format_language),
                msgid,
                &pos_id,
                msgstr,
                &pos_str,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_formats(content: &str) -> Vec<Diagnostic> {
        let rules = Rules::new(vec![Box::new(FormatsRule {})]);
        let mut checker = Checker::new(content.as_bytes(), &rules);
        checker.do_all_checks();
        checker.diagnostics
    }

    #[test]
    fn test_no_c_formats() {
        let diags = check_formats(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_c_formats_ok() {
        let diags = check_formats(
            r#"
#, c-format
msgid "name: %s, age: %d"
msgstr "nom : %s, âge : %d"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_c_format_error() {
        let diags = check_formats(
            r#"
#, c-format
msgid "name: %s, age: %d"
msgstr "nom : %s, âge : %f"

#, c-format
msgid "%d test (%s)"
msgstr "%2$d test (%1$s)"
"#,
        );
        assert_eq!(diags.len(), 2);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "inconsistent format strings (C)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "inconsistent format strings (C)");
    }
}
