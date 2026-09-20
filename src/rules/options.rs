// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `options` rule: check missing/extra/different command-line options.

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Edit, Fix, FixTarget};
use crate::po::entry::Entry;
use crate::po::format::iter::FormatOptionPos;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct OptionsRule;

impl RuleChecker for OptionsRule {
    fn name(&self) -> &'static str {
        "options"
    }

    fn description(&self) -> &'static str {
        "Check for missing, extra or different command-line options in translation."
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for missing, extra or different command-line options in the translation.
    ///
    /// An option translated is a help string telling the user to type something that
    /// does not work. An option is a `-` or `--` prefix at a word boundary followed by
    /// an ASCII letter, then ASCII alphanumeric characters, `-` and `_`. Only the name
    /// is compared, so the value of `--opt=value` stays translatable.
    ///
    /// This rule is not enabled by default.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "Use --verbose to get more output"
    /// msgstr "Utilisez --bavard pour plus de détails"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "Use --verbose to get more output"
    /// msgstr "Utilisez --verbose pour plus de détails"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`warning`](Severity::Warning): `missing options (# / #)`
    /// - [`warning`](Severity::Warning): `extra options (# / #)`
    /// - [`warning`](Severity::Warning): `different options` (auto-fixable)
    ///
    /// Only the `different options` diagnostic carries an auto-fix: each translation
    /// option is replaced in place with the option at the same position in the source.
    /// The `missing` and `extra` cases are left unfixed because inserting a missing
    /// option at the right position in the prose or choosing which extra to drop both
    /// require translator judgement.
    fn check_msg(
        &self,
        checker: &Checker,
        entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let id_opts: Vec<_> = FormatOptionPos::new(&msgid.value, entry.format_language).collect();
        let str_opts: Vec<_> = FormatOptionPos::new(&msgstr.value, entry.format_language).collect();
        match id_opts.len().cmp(&str_opts.len()) {
            std::cmp::Ordering::Greater => self
                .new_diag(
                    checker,
                    Severity::Warning,
                    format!("missing options ({} / {})", id_opts.len(), str_opts.len()),
                )
                .map(|d| {
                    d.with_msgs_hl(
                        msgid,
                        id_opts.iter().map(|m| (m.start, m.end)),
                        msgstr,
                        str_opts.iter().map(|m| (m.start, m.end)),
                    )
                })
                .into_iter()
                .collect(),
            std::cmp::Ordering::Less => self
                .new_diag(
                    checker,
                    Severity::Warning,
                    format!("extra options ({} / {})", id_opts.len(), str_opts.len()),
                )
                .map(|d| {
                    d.with_msgs_hl(
                        msgid,
                        id_opts.iter().map(|m| (m.start, m.end)),
                        msgstr,
                        str_opts.iter().map(|m| (m.start, m.end)),
                    )
                })
                .into_iter()
                .collect(),
            std::cmp::Ordering::Equal => {
                // Check that options are the same, in any order: word order changes
                // between languages, so "--input before --output" can legitimately be
                // translated with the two options swapped.
                let mut id_sorted: Vec<_> = id_opts.iter().map(|m| m.s).collect();
                let mut str_sorted: Vec<_> = str_opts.iter().map(|m| m.s).collect();
                id_sorted.sort_unstable();
                str_sorted.sort_unstable();
                if id_sorted == str_sorted {
                    return vec![];
                }
                let edits: Vec<Edit> = id_opts
                    .iter()
                    .zip(str_opts.iter())
                    .filter(|(id, str)| id.s != str.s)
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
                    safe: false,
                });
                self.new_diag(checker, Severity::Warning, "different options")
                    .map(|d| {
                        d.with_msgs_hl(
                            msgid,
                            id_opts.iter().map(|m| (m.start, m.end)),
                            msgstr,
                            str_opts.iter().map(|m| (m.start, m.end)),
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
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_options(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(OptionsRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_no_options() {
        let diags = check_options(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_options_ok() {
        let diags = check_options(
            r#"
msgid "Use --verbose to get more output"
msgstr "Utilisez --verbose pour plus de détails"

msgid "Options: -v, -q and --no-color"
msgstr "Options : -v, -q et --no-color"

msgid "Pass \"--jobs\" or (-j)"
msgstr "Passez \"--jobs\" ou (-j)"
"#,
        );
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn test_option_value_stays_translatable() {
        // Only the name of "--opt=value" is compared, the value is left to the translator.
        let diags = check_options(
            r#"
msgid "Use --log-level=debug"
msgstr "Utilisez --log-level=débogage"
"#,
        );
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn test_options_swapped_are_ok() {
        // Options are compared as a multiset: a reordered translation is correct.
        let diags = check_options(
            r#"
msgid "Use --input before --output"
msgstr "Avant --output, utilisez --input"
"#,
        );
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn test_prose_dashes_are_not_options() {
        let diags = check_options(
            r#"
msgid "A well-known UTF-8 café-restaurant, 5-10 entries, -5 degrees - really"
msgstr "Un café-restaurant UTF-8 bien connu, de 5 à 10 entrées, -5 degrés - vraiment"

msgid "Everything after -- is a file name"
msgstr "Tout ce qui suit -- est un nom de fichier"
"#,
        );
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn test_options_error() {
        let diags = check_options(
            r#"
msgid "missing: --verbose and --quiet"
msgstr "manquant : --verbose"

msgid "extra: --verbose"
msgstr "en trop : --verbose et --quiet"

msgid "different: --verbose"
msgstr "différent : --bavard"
"#,
        );
        assert_eq!(diags.len(), 3);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "missing options (2 / 1)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "extra options (1 / 2)");
        let diag = &diags[2];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "different options");
    }

    #[test]
    fn test_short_option_case_matters() {
        let diags = check_options(
            r#"
msgid "Use -v for the version"
msgstr "Utilisez -V pour la version"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "different options");
    }

    #[test]
    fn test_options_in_format_strings_are_ignored() {
        let diags = check_options(
            r#"
#, c-format
msgid "Unknown option %s, try --help"
msgstr "Option inconnue %s, essayez --help"
"#,
        );
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn test_different_options_fix_replaces_each_in_place() {
        let diags = check_options(
            r#"
msgid "Use --verbose or --quiet"
msgstr "Utilisez --bavard ou --silencieux"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "different options");
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 2);
        assert_eq!(fix.edits[0].replacement, "--verbose");
        assert_eq!(fix.edits[1].replacement, "--quiet");
        assert!(!fix.safe);
    }

    #[test]
    fn test_different_options_fix_skips_positions_already_equal() {
        let diags = check_options(
            r#"
msgid "Use --verbose or --quiet"
msgstr "Utilisez --verbose ou --silencieux"
"#,
        );
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].replacement, "--quiet");
    }

    #[test]
    fn test_different_options_fix_keeps_the_value() {
        // The edit spans the option name only, so "--opt=value" keeps its value.
        let diags = check_options(
            r#"
msgid "Use --log-level=debug"
msgstr "Utilisez --niveau=débogage"
"#,
        );
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].replacement, "--log-level");
    }

    #[test]
    fn test_missing_and_extra_options_have_no_fix() {
        let diags = check_options(
            r#"
msgid "Use --verbose or --quiet"
msgstr "Utilisez --verbose"

msgid "Use --verbose"
msgstr "Utilisez --verbose ou --quiet"
"#,
        );
        assert_eq!(diags.len(), 2);
        assert!(
            diags[0].fix.is_none(),
            "missing options diagnostic must not carry a fix"
        );
        assert!(
            diags[1].fix.is_none(),
            "extra options diagnostic must not carry a fix"
        );
    }
}
