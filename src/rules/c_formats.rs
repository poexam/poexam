// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `c-formats` rule: check inconsistent C format strings.

use crate::c_format::{CFormat, MatchCFormat};
use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct CFormatsRule;

impl RuleChecker for CFormatsRule {
    fn name(&self) -> &'static str {
        "c-formats"
    }

    fn is_default(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    /// Check for inconsistent C format strings.
    ///
    /// Only the entries marked with `c-format` are checked.
    ///
    /// The reordering of format specifiers is supported: `%3$d %1$s %2$f` is considered
    /// equivalent to `%s %f %d`.
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
    /// - `inconsistent C format strings`
    fn check_msg(&self, checker: &mut Checker, entry: &Entry, msgid: &str, msgstr: &str) {
        if entry.format != "c" {
            return;
        }
        let id_fmt: Vec<MatchCFormat> = CFormat::new(msgid).collect();
        let str_fmt: Vec<MatchCFormat> = CFormat::new(msgstr).collect();
        let mut id_fmt_sorted = id_fmt.clone();
        let mut str_fmt_sorted = str_fmt.clone();
        id_fmt_sorted.sort();
        str_fmt_sorted.sort();
        let id_fmt2 = id_fmt_sorted
            .iter()
            .map(MatchCFormat::remove_reordering)
            .collect::<Vec<String>>();
        let str_fmt2 = str_fmt_sorted
            .iter()
            .map(MatchCFormat::remove_reordering)
            .collect::<Vec<String>>();
        if id_fmt2 != str_fmt2 {
            let pos_id = id_fmt
                .iter()
                .map(|m| (m.start, m.end))
                .collect::<Vec<(usize, usize)>>();
            let pos_str = str_fmt
                .iter()
                .map(|m| (m.start, m.end))
                .collect::<Vec<(usize, usize)>>();
            checker.report_msg(
                entry,
                "inconsistent C format strings".to_string(),
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
        let rules = Rules::new(vec![Box::new(CFormatsRule {})]);
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
        assert_eq!(diag.message, "inconsistent C format strings");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "inconsistent C format strings");
    }
}
