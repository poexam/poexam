// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `formats` rule: check inconsistent format strings.

use std::collections::HashSet;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::po::format::language::Language;
use crate::po::format::{
    iter::FormatPos,
    lang_c::{fmt_sort_index, fmt_strip_index},
    lang_sh::fmt_sh_name,
};
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct FormatsRule;

impl RuleChecker for FormatsRule {
    fn name(&self) -> &'static str {
        "formats"
    }

    fn description(&self) -> &'static str {
        "Check for inconsistent format strings between source and translation."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for inconsistent format strings.
    ///
    /// The following languages are supported:
    /// - C (`c-format`): printf format (e.g. `%s`, `%12lld`)
    /// - Java (`java-format`): Java `MessageFormat` language (e.g. `{0}`, `{1,date,short}`)
    /// - JavaScript (`javascript-format`): `sprintf`-like format (e.g. `%s`, `%1$05.2f`)
    /// - PHP (`php-format`): PHP `sprintf` format (e.g. `%s`, `%'*10.2f`)
    /// - Python (`python-format`): Python % format strings (e.g. `%s`, `%(age)d`)
    /// - Python brace (`python-brace-format`): Python brace format strings (e.g. `{0}`, `{1!r:20}`)
    /// - Shell (`sh-format`): variables expanded by `eval_gettext` (e.g. `$name`, `${name}`)
    ///
    /// For the C, JavaScript and PHP formats, the reordering of format specifiers is supported:
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
    /// Diagnostics reported:
    /// - [`error`](Severity::Error): `inconsistent format strings (…)`
    fn check_msg(
        &self,
        checker: &Checker,
        entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        if entry.format_language == Language::Null {
            return vec![];
        }
        let mut id_fmt: Vec<_> = FormatPos::new(&msgid.value, entry.format_language).collect();
        let mut str_fmt: Vec<_> = FormatPos::new(&msgstr.value, entry.format_language).collect();
        let error = if matches!(
            entry.format_language,
            Language::C | Language::JavaScript | Language::Php
        ) {
            // C, JavaScript and PHP format strings can include reordering position, so we
            // need to sort them and strip index before comparing. The original order is not
            // needed after this branch (highlights below only use positions, which sort
            // independently).
            id_fmt.sort_by_key(|m| (fmt_sort_index(m.s), m.start, m.end));
            str_fmt.sort_by_key(|m| (fmt_sort_index(m.s), m.start, m.end));
            let id_fmt2: Vec<_> = id_fmt.iter().map(|m| fmt_strip_index(m.s)).collect();
            let str_fmt2: Vec<_> = str_fmt.iter().map(|m| fmt_strip_index(m.s)).collect();
            id_fmt2 != str_fmt2
        } else if entry.format_language == Language::Sh {
            // Shell: "$name" and "${name}" are the same variable, compare names only.
            let id_fmt_hash: HashSet<_> = id_fmt.iter().map(|m| fmt_sh_name(m.s)).collect();
            let str_fmt_hash: HashSet<_> = str_fmt.iter().map(|m| fmt_sh_name(m.s)).collect();
            id_fmt_hash != str_fmt_hash
        } else {
            // Other languages: just check that format strings are the same, in any order.
            let id_fmt_hash: HashSet<_> = id_fmt.iter().map(|m| m.s).collect();
            let str_fmt_hash: HashSet<_> = str_fmt.iter().map(|m| m.s).collect();
            id_fmt_hash != str_fmt_hash
        };
        if error {
            self.new_diag(
                checker,
                Severity::Error,
                format!("inconsistent format strings ({})", entry.format_language),
            )
            .map(|d| {
                d.with_msgs_hl(
                    msgid,
                    id_fmt.iter().map(|m| (m.start, m.end)),
                    msgstr,
                    str_fmt.iter().map(|m| (m.start, m.end)),
                )
            })
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

    fn check_formats(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(FormatsRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_no_formats() {
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
    fn test_python_formats_ok() {
        let diags = check_formats(
            r#"
#, python-format
msgid "name: %(name)s, age: %d"
msgstr "age: %d, nom: %(name)s"
"#,
        );
        assert!(diags.is_empty());

        let diags = check_formats(
            r#"
#, python-brace-format
msgid "name: {0}, age: %d"
msgstr "age: %d, nom: {0}"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_c_format_error_noqa() {
        let diags = check_formats(
            r#"
#, c-format, noqa:formats
msgid "name: %s, age: %d"
msgstr "nom : %s, âge : %f"
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

    #[test]
    fn test_php_formats_ok() {
        let diags = check_formats(
            r#"
#, php-format
msgid "%s has %'*10.2f points, %d%% done"
msgstr "%2$'*10.2f points pour %1$s, %3$d %% fait"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_php_format_error() {
        let diags = check_formats(
            r#"
#, php-format
msgid "%s has %'*10.2f points"
msgstr "%s a %10.2f points"

#, php-format
msgid "%d test (%s)"
msgstr "%2$d test (%1$s)"
"#,
        );
        assert_eq!(diags.len(), 2);
        assert!(diags.iter().all(|d| d.severity == Severity::Error));
        assert!(
            diags
                .iter()
                .all(|d| d.message == "inconsistent format strings (PHP)")
        );
    }

    #[test]
    fn test_javascript_formats_ok() {
        let diags = check_formats(
            r#"
#, javascript-format
msgid "%s has %05.2f points, %d%% done"
msgstr "%2$05.2f points pour %1$s, %3$d %% fait"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_javascript_format_error() {
        let diags = check_formats(
            r#"
#, javascript-format
msgid "%s has %j"
msgstr "%s a %s"

#, javascript-format
msgid "%d test (%s)"
msgstr "%2$d test (%1$s)"
"#,
        );
        assert_eq!(diags.len(), 2);
        assert!(diags.iter().all(|d| d.severity == Severity::Error));
        assert!(
            diags
                .iter()
                .all(|d| d.message == "inconsistent format strings (JavaScript)")
        );
    }

    #[test]
    fn test_sh_formats_ok() {
        let diags = check_formats(
            r#"
#, sh-format
msgid "Copying $count files to ${dir}, cost: 5$ ($1)"
msgstr "Copie de ${count} fichiers vers $dir, coût : 5 $ ($2)"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_sh_format_error() {
        let diags = check_formats(
            r#"
#, sh-format
msgid "Copying $count files to $dir"
msgstr "Copie de $nombre fichiers vers $dir"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Error);
        assert_eq!(diags[0].message, "inconsistent format strings (Shell)");
    }

    #[test]
    fn test_python_format_error() {
        let diags = check_formats(
            r#"
#, python-format
msgid "name: %(name)s, age: %d"
msgstr "nom : %s, âge : %f"
"#,
        );
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "inconsistent format strings (Python)");

        let diags = check_formats(
            r#"
#, python-brace-format
msgid "name: {0}, age: {1}"
msgstr "nom : {2}, âge : {1}"
"#,
        );
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "inconsistent format strings (Python brace)");
    }
}
