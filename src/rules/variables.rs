// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `variables` rule: check missing/extra/different variables.

use std::borrow::Cow;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Edit, Fix, FixTarget};
use crate::po::entry::Entry;
use crate::po::format::iter::FormatVariablePos;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct VariablesRule;

/// Normalize a variable so that the same placeholder written with different inner spacing
/// compares equal: `{{ name }}` and `{{name}}` are the same variable for every template
/// engine using that syntax.
fn normalize(variable: &str) -> Cow<'_, str> {
    if variable.chars().any(char::is_whitespace) {
        Cow::Owned(variable.chars().filter(|c| !c.is_whitespace()).collect())
    } else {
        Cow::Borrowed(variable)
    }
}

impl RuleChecker for VariablesRule {
    fn name(&self) -> &'static str {
        "variables"
    }

    fn description(&self) -> &'static str {
        "Check for missing, extra or different variables in translation."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for missing, extra or different variables in the translation.
    ///
    /// A variable is a placeholder substituted at runtime by a templating syntax that is
    /// not one of gettext's `*-format` languages: such an entry carries no format flag, so
    /// the `formats` rule returns early and the placeholder is otherwise unchecked. The
    /// syntaxes recognized are selected with the `variable_styles` option; by default
    /// `${VAR}`, `%VAR%`, `@VAR@` and `{{var}}`, with `$VAR` and `{var}` available on
    /// demand. Variables inside format strings are skipped, so a `python-brace-format`
    /// entry is left to the `formats` rule.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "Welcome, ${USER}"
    /// msgstr "Bienvenue, ${UTILISATEUR}"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "Welcome, ${USER}"
    /// msgstr "Bienvenue, ${USER}"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`error`](Severity::Error): `missing variables (# / #)`
    /// - [`error`](Severity::Error): `extra variables (# / #)`
    /// - [`error`](Severity::Error): `different variables` (auto-fixable)
    ///
    /// Only the `different variables` diagnostic carries an auto-fix: each translation
    /// variable is replaced in place with the variable at the same position in the source.
    /// The `missing` and `extra` cases are left unfixed because inserting a missing
    /// variable at the right position in the prose or choosing which extra to drop both
    /// require translator judgement.
    fn check_msg(
        &self,
        checker: &Checker,
        entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let styles = &checker.config.check.variable_styles;
        if styles.is_empty() {
            return vec![];
        }
        let id_vars: Vec<_> =
            FormatVariablePos::new(&msgid.value, entry.format_language, styles).collect();
        let str_vars: Vec<_> =
            FormatVariablePos::new(&msgstr.value, entry.format_language, styles).collect();
        match id_vars.len().cmp(&str_vars.len()) {
            std::cmp::Ordering::Greater => self
                .new_diag(
                    checker,
                    Severity::Error,
                    format!("missing variables ({} / {})", id_vars.len(), str_vars.len()),
                )
                .map(|d| {
                    d.with_msgs_hl(
                        msgid,
                        id_vars.iter().map(|m| (m.start, m.end)),
                        msgstr,
                        str_vars.iter().map(|m| (m.start, m.end)),
                    )
                })
                .into_iter()
                .collect(),
            std::cmp::Ordering::Less => self
                .new_diag(
                    checker,
                    Severity::Error,
                    format!("extra variables ({} / {})", id_vars.len(), str_vars.len()),
                )
                .map(|d| {
                    d.with_msgs_hl(
                        msgid,
                        id_vars.iter().map(|m| (m.start, m.end)),
                        msgstr,
                        str_vars.iter().map(|m| (m.start, m.end)),
                    )
                })
                .into_iter()
                .collect(),
            std::cmp::Ordering::Equal => {
                // Check that variables are the same, in any order: word order changes
                // between languages, so "${SRC} to ${DEST}" can legitimately be
                // translated with the two variables swapped.
                let id_names: Vec<_> = id_vars.iter().map(|m| normalize(m.s)).collect();
                let str_names: Vec<_> = str_vars.iter().map(|m| normalize(m.s)).collect();
                let mut id_sorted = id_names.clone();
                let mut str_sorted = str_names.clone();
                id_sorted.sort_unstable();
                str_sorted.sort_unstable();
                if id_sorted == str_sorted {
                    return vec![];
                }
                let edits: Vec<Edit> = (0..id_vars.len())
                    .filter(|&i| id_names[i] != str_names[i])
                    .map(|i| Edit {
                        range: str_vars[i].start..str_vars[i].end,
                        replacement: id_vars[i].s.to_string(),
                    })
                    .collect();
                let fix = (!edits.is_empty()).then(|| Fix {
                    target: FixTarget::Msgstr {
                        file_byte_range: msgstr.byte_range.clone(),
                    },
                    edits,
                    safe: false,
                });
                self.new_diag(checker, Severity::Error, "different variables")
                    .map(|d| {
                        d.with_msgs_hl(
                            msgid,
                            id_vars.iter().map(|m| (m.start, m.end)),
                            msgstr,
                            str_vars.iter().map(|m| (m.start, m.end)),
                        )
                        .with_optional_fix(fix)
                    })
                    .into_iter()
                    .collect()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::po::format::iter::VariableStyle;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    /// Check a PO content with the default variable styles.
    fn check_variables(content: &str) -> Vec<Diagnostic> {
        check_variables_with(content, None)
    }

    /// Check a PO content, optionally overriding the enabled variable styles.
    fn check_variables_with(content: &str, styles: Option<Vec<VariableStyle>>) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        if let Some(styles) = styles {
            checker.config.check.variable_styles = styles;
        }
        let rules = Rules::new(vec![Box::new(VariablesRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_normalize() {
        assert_eq!(normalize("${HOME}"), "${HOME}");
        assert_eq!(normalize("{{ user.name }}"), "{{user.name}}");
        assert_eq!(normalize("{{user.name}}"), "{{user.name}}");
    }

    #[test]
    fn test_no_variables() {
        let diags = check_variables(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_variables_ok() {
        let diags = check_variables(
            r#"
msgid "Welcome, ${USER}"
msgstr "Bienvenue, ${USER}"

msgid "Saved in %USERPROFILE%\\Documents"
msgstr "Enregistré dans %USERPROFILE%\\Documents"

msgid "Version @PACKAGE_VERSION@ of @PACKAGE@"
msgstr "Version @PACKAGE_VERSION@ de @PACKAGE@"

msgid "Hello {{ name }}"
msgstr "Bonjour {{name}}"
"#,
        );
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn test_variables_swapped_are_ok() {
        // Variables are compared as a multiset: a reordered translation is correct.
        let diags = check_variables(
            r#"
msgid "Copy ${SRC} to ${DEST}"
msgstr "Vers ${DEST}, copier ${SRC}"
"#,
        );
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn test_repeated_variables_are_counted() {
        // A set comparison would miss this: both sides use only ${A} and ${B}.
        let diags = check_variables(
            r#"
msgid "${A}, ${A} and ${B}"
msgstr "${A}, ${B} et ${B}"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "different variables");
    }

    #[test]
    fn test_variables_error() {
        let diags = check_variables(
            r#"
msgid "missing: ${SRC} to ${DEST}"
msgstr "manquant : ${SRC}"

msgid "extra: ${SRC}"
msgstr "en trop : ${SRC} vers ${DEST}"

msgid "different: ${SRC}"
msgstr "différent : ${SOURCE}"
"#,
        );
        assert_eq!(diags.len(), 3);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "missing variables (2 / 1)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "extra variables (1 / 2)");
        let diag = &diags[2];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "different variables");
    }

    #[test]
    fn test_non_default_styles_are_ignored() {
        // `$VAR` and `{var}` are not part of the default style set.
        let diags = check_variables(
            r#"
msgid "Home is $HOME"
msgstr "Le dossier est $ACCUEIL"

msgid "Hello {name}"
msgstr "Bonjour {nom}"
"#,
        );
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn test_dollar_style_enabled() {
        let diags = check_variables_with(
            r#"
msgid "Home is $HOME"
msgstr "Le dossier est $ACCUEIL"
"#,
            Some(vec![VariableStyle::Dollar]),
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "different variables");
    }

    #[test]
    fn test_dollar_style_ignores_amounts_of_money() {
        // A bare `$` variable must start with a letter or an underscore.
        let diags = check_variables_with(
            r#"
msgid "Only $5 today, $1000 tomorrow"
msgstr "Seulement 5 $ aujourd'hui"
"#,
            Some(vec![VariableStyle::Dollar]),
        );
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn test_brace_style_enabled() {
        let diags = check_variables_with(
            r#"
msgid "Hello {name}, you have {0} messages"
msgstr "Bonjour {name}, vous avez {1} messages"
"#,
            Some(vec![VariableStyle::Brace]),
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "different variables");
    }

    #[test]
    fn test_double_brace_wins_over_brace() {
        // With both styles enabled, `{{name}}` is one variable, not `{name}` plus braces.
        let diags = check_variables_with(
            r#"
msgid "Hello {{name}}"
msgstr "Bonjour {{nom}}"
"#,
            Some(vec![VariableStyle::DoubleBrace, VariableStyle::Brace]),
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "different variables");
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].replacement, "{{name}}");
    }

    #[test]
    fn test_double_brace_is_not_read_as_brace() {
        // With only the `brace` style enabled, `{{…}}` must yield no variable at all,
        // otherwise a Jinja catalog reports the inner `{name}` as extra or missing.
        let diags = check_variables_with(
            r#"
msgid "Hello {{ name }}, welcome"
msgstr "Bonjour {{name}}, bienvenue"
"#,
            Some(vec![VariableStyle::Brace]),
        );
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn test_emails_and_handles_are_not_variables() {
        // `@` opens a variable only at a word boundary, on both sides.
        let diags = check_variables(
            r#"
msgid "Write to user@example.com or a@b@c"
msgstr "Écrivez à contact@exemple.com"

msgid "Follow @user@instance.social"
msgstr "Suivez @someone@autre.social"
"#,
        );
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn test_percentages_are_not_variables() {
        let diags = check_variables(
            r#"
msgid "50% off, up to 70% on Friday"
msgstr "50 % de remise"
"#,
        );
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn test_variables_in_format_strings_are_ignored() {
        // The entry is flagged `python-brace-format`, so `{name}` belongs to `formats`.
        let diags = check_variables_with(
            r#"
#, python-brace-format
msgid "Hello {name}"
msgstr "Bonjour {nom}"
"#,
            Some(vec![VariableStyle::Brace]),
        );
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn test_no_styles_enabled_reports_nothing() {
        let diags = check_variables_with(
            r#"
msgid "Welcome, ${USER}"
msgstr "Bienvenue, ${UTILISATEUR}"
"#,
            Some(vec![]),
        );
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn test_different_variables_fix_replaces_each_in_place() {
        let diags = check_variables(
            r#"
msgid "Copy ${SRC} to %DEST%"
msgstr "Copier ${SOURCE} vers %CIBLE%"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "different variables");
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 2);
        assert_eq!(fix.edits[0].replacement, "${SRC}");
        assert_eq!(fix.edits[1].replacement, "%DEST%");
        assert!(!fix.safe);
    }

    #[test]
    fn test_different_variables_fix_skips_positions_already_equal() {
        let diags = check_variables(
            r#"
msgid "Copy ${SRC} to ${DEST}"
msgstr "Copier ${SRC} vers ${CIBLE}"
"#,
        );
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].replacement, "${DEST}");
    }

    #[test]
    fn test_missing_and_extra_variables_have_no_fix() {
        let diags = check_variables(
            r#"
msgid "Copy ${SRC} to ${DEST}"
msgstr "Copier ${SRC}"

msgid "Copy ${SRC}"
msgstr "Copier ${SRC} vers ${DEST}"
"#,
        );
        assert_eq!(diags.len(), 2);
        assert!(
            diags[0].fix.is_none(),
            "missing variables diagnostic must not carry a fix"
        );
        assert!(
            diags[1].fix.is_none(),
            "extra variables diagnostic must not carry a fix"
        );
    }
}
