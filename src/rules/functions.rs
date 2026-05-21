// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `functions` rule: check missing/extra/different function names.

use std::collections::HashSet;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Edit, Fix, FixTarget};
use crate::po::entry::Entry;
use crate::po::format::iter::FormatFunctionPos;
use crate::po::message::Message;
use crate::rules::double_quotes::trim_quotes;
use crate::rules::rule::RuleChecker;

pub struct FunctionsRule;

impl RuleChecker for FunctionsRule {
    fn name(&self) -> &'static str {
        "functions"
    }

    fn description(&self) -> &'static str {
        "Check for missing, extra or different function names in translation."
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for missing, extra or different function names in the translation.
    ///
    /// A function name is a sequence of word characters and dots (optionally
    /// joined by `::` or `->` separators) ending with `()`, for example:
    /// `foo()`, `module.function()`, `Class::method()` or `ptr->method()`.
    ///
    /// This rule is not enabled by default.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "Call foo() to start"
    /// msgstr "Appelez démarrer() pour commencer"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "Call foo() to start"
    /// msgstr "Appelez foo() pour commencer"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`warning`](Severity::Warning): `missing functions (# / #)`
    /// - [`warning`](Severity::Warning): `extra functions (# / #)`
    /// - [`warning`](Severity::Warning): `different functions` (auto-fixable)
    ///
    /// Only the `different functions` diagnostic carries an auto-fix: each
    /// translation function name is replaced in place with the function name
    /// at the same position in the source. The `missing` and `extra` cases
    /// are left unfixed because inserting a missing function at the right
    /// position in the prose or choosing which extra to drop both require
    /// translator judgement.
    fn check_msg(
        &self,
        checker: &Checker,
        entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let id_funcs: Vec<_> =
            FormatFunctionPos::new(&msgid.value, entry.format_language).collect();
        let str_funcs: Vec<_> =
            FormatFunctionPos::new(&msgstr.value, entry.format_language).collect();
        match id_funcs.len().cmp(&str_funcs.len()) {
            std::cmp::Ordering::Greater => self
                .new_diag(
                    checker,
                    Severity::Warning,
                    format!(
                        "missing functions ({} / {})",
                        id_funcs.len(),
                        str_funcs.len()
                    ),
                )
                .map(|d| {
                    d.with_msgs_hl(
                        msgid,
                        id_funcs.iter().map(|m| (m.start, m.end)),
                        msgstr,
                        str_funcs.iter().map(|m| (m.start, m.end)),
                    )
                })
                .into_iter()
                .collect(),
            std::cmp::Ordering::Less => self
                .new_diag(
                    checker,
                    Severity::Warning,
                    format!("extra functions ({} / {})", id_funcs.len(), str_funcs.len()),
                )
                .map(|d| {
                    d.with_msgs_hl(
                        msgid,
                        id_funcs.iter().map(|m| (m.start, m.end)),
                        msgstr,
                        str_funcs.iter().map(|m| (m.start, m.end)),
                    )
                })
                .into_iter()
                .collect(),
            std::cmp::Ordering::Equal => {
                // Check that functions are the same, in any order.
                // A single pair of quotes is skipped from both sides of the name.
                let id_funcs_hash: HashSet<_> = id_funcs.iter().map(|m| trim_quotes(m.s)).collect();
                let str_funcs_hash: HashSet<_> =
                    str_funcs.iter().map(|m| trim_quotes(m.s)).collect();
                if id_funcs_hash == str_funcs_hash {
                    vec![]
                } else {
                    let edits: Vec<Edit> = id_funcs
                        .iter()
                        .zip(str_funcs.iter())
                        .filter(|(id, str)| trim_quotes(id.s) != trim_quotes(str.s))
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
                    });
                    self.new_diag(checker, Severity::Warning, "different functions")
                        .map(|d| {
                            d.with_msgs_hl(
                                msgid,
                                id_funcs
                                    .iter()
                                    .filter(|m| !str_funcs_hash.contains(trim_quotes(m.s)))
                                    .map(|m| (m.start, m.end)),
                                msgstr,
                                str_funcs
                                    .iter()
                                    .filter(|m| !id_funcs_hash.contains(trim_quotes(m.s)))
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

    fn check_functions(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(FunctionsRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_no_functions() {
        let diags = check_functions(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_functions_ok() {
        let diags = check_functions(
            // Order of functions is not checked.
            r#"
msgid "Call foo() and Class::method() to start"
msgstr "Appelez Class::method() et foo() pour démarrer"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_functions_error() {
        let diags = check_functions(
            r#"
msgid "missing functions: foo() bar()"
msgstr "fonctions manquantes : foo()"

msgid "extra functions: foo()"
msgstr "fonctions extra : foo() bar()"

msgid "different functions: foo() bar()"
msgstr "fonctions différentes : foo() baz()"
"#,
        );
        assert_eq!(diags.len(), 3);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "missing functions (2 / 1)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "extra functions (1 / 2)");
        let diag = &diags[2];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "different functions");
    }

    #[test]
    fn test_different_functions_fix_replaces_each_in_place() {
        let diags = check_functions(
            r#"
msgid "Call foo() or bar()"
msgstr "Appelez baz() ou qux()"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "different functions");
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 2);
        assert_eq!(fix.edits[0].replacement, "foo()");
        assert_eq!(fix.edits[1].replacement, "bar()");
    }

    #[test]
    fn test_different_functions_fix_skips_positions_already_equal() {
        let diags = check_functions(
            r#"
msgid "Call foo() or bar()"
msgstr "Appelez foo() ou qux()"
"#,
        );
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].replacement, "bar()");
    }

    #[test]
    fn test_missing_and_extra_functions_have_no_fix() {
        let diags = check_functions(
            r#"
msgid "Call foo() or bar()"
msgstr "Appelez foo()"

msgid "Call foo()"
msgstr "Appelez foo() ou bar()"
"#,
        );
        assert_eq!(diags.len(), 2);
        assert!(
            diags[0].fix.is_none(),
            "missing functions diagnostic must not carry a fix"
        );
        assert!(
            diags[1].fix.is_none(),
            "extra functions diagnostic must not carry a fix"
        );
    }

    #[test]
    fn test_functions_qualified_names_ok() {
        let diags = check_functions(
            r#"
msgid "Use module.foo() and Class::method() and ptr->m()"
msgstr "Utiliser module.foo() et Class::method() et ptr->m()"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_different_functions_highlights_only_differing() {
        // `foo()` is present in both messages, so it must NOT be highlighted.
        // Only the differing names (`bar()` in msgid, `baz()` in msgstr) are
        // highlighted, on each side respectively.
        let diags = check_functions(
            r#"
msgid "Use foo() and bar() now"
msgstr "Use foo() and baz() now"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "different functions");
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
        assert_eq!(id_hl, vec!["bar()"]);
        assert_eq!(str_hl, vec!["baz()"]);
    }

    #[test]
    fn test_functions_translated_name_is_flagged() {
        // The translation replaces the function name itself, which is exactly
        // what this rule is meant to catch.
        let diags = check_functions(
            r#"
msgid "Call init() to start"
msgstr "Appelez démarrer() pour commencer"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "different functions");
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].replacement, "init()");
    }
}
