// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `numbers` rule: check missing/extra/different numbers.

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Edit, Fix, FixTarget};
use crate::po::entry::Entry;
use crate::po::format::iter::{FormatNumberPos, ascii_digit};
use crate::po::format::language::Language;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct NumbersRule;

/// Append a digit group to a normalized number: the first group and any group of
/// exactly three digits (a thousands group) are appended as-is, any other group is
/// a fractional part and is prefixed with the decimal point.
fn push_group(normalized: &mut String, group: &str, first: bool) {
    if !first && group.len() != 3 {
        normalized.push('.');
    }
    normalized.push_str(group);
}

/// Normalize a number so that the same value written with different conventions
/// compares equal: digits are mapped to ASCII, digit-group separators are removed
/// and the decimal separator is folded to `.`.
///
/// A separator followed by a group of exactly three digits is a group separator,
/// any other separator is a decimal separator. So `1,000` (English), `1 000`
/// (French) and `1.000` (German) all normalize to `1000`, while `1.5` and `1,5`
/// both normalize to `1.5`.
///
/// The convention is resolved the same way on both sides of the comparison, so an
/// ambiguous number such as `1,000` cannot produce a false positive: whichever
/// reading is picked, it is picked for the source and the translation alike.
fn normalize(number: &str) -> String {
    let mut normalized = String::with_capacity(number.len());
    let mut group = String::new();
    let mut first = true;
    for c in number.chars() {
        if let Some(digit) = ascii_digit(c) {
            group.push(digit);
        } else {
            push_group(&mut normalized, &group, first);
            group.clear();
            first = false;
        }
    }
    push_group(&mut normalized, &group, first);
    normalized
}

/// Whether both sides hold the same numbers, in any order: a date written `2026-02-01`
/// and translated `01/02/2026` is correct.
fn same_values(left: &[String], right: &[String]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut left: Vec<&str> = left.iter().map(String::as_str).collect();
    let mut right: Vec<&str> = right.iter().map(String::as_str).collect();
    left.sort_unstable();
    right.sort_unstable();
    left == right
}

/// Collect the normalized numbers of a message, reading a letter run directly after the
/// digits as a unit rather than as the rest of an identifier.
fn unit_values(value: &str, language: Language) -> Vec<String> {
    FormatNumberPos::with_units(value, language)
        .map(|m| normalize(m.s))
        .collect()
}

impl RuleChecker for NumbersRule {
    fn name(&self) -> &'static str {
        "numbers"
    }

    fn description(&self) -> &'static str {
        "Check for missing, extra or different numbers in translation."
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for missing, extra or different numbers in the translation.
    ///
    /// Numbers are compared by value, not by spelling: digits of any script are
    /// mapped to ASCII and the digit-group and decimal separators are normalized
    /// (see [`normalize`]), so `1,000` translated as `1 000` is correct. Numbers
    /// glued to a letter (`MP3`, `3D`, `1st`) are identifiers rather than values
    /// and are ignored, as are the numbers inside format strings (e.g. `%3$d`).
    ///
    /// That exclusion is decided per side, so detaching a unit in the translation
    /// would otherwise turn the same number into an identifier on one side and a
    /// value on the other. Before reporting anything, the numbers are compared once
    /// more with the letters read as a unit on both sides, which leaves `8MB`
    /// translated as `8 Mo` alone while still reporting `8MB` translated as `9 Mo`.
    ///
    /// This rule is not enabled by default: a number spelled out on one side
    /// only is reported, as in `3 times` → `trois fois` (missing) or
    /// `first entry` → `1ère entrée` (extra).
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "Press 3 times to confirm"
    /// msgstr "Appuyez 2 fois pour confirmer"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "Press 3 times to confirm"
    /// msgstr "Appuyez 3 fois pour confirmer"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`warning`](Severity::Warning): `missing numbers (# / #)`
    /// - [`warning`](Severity::Warning): `extra numbers (# / #)`
    /// - [`warning`](Severity::Warning): `different numbers` (auto-fixable)
    ///
    /// Only the `different numbers` diagnostic carries an auto-fix: each
    /// translation number is replaced in place with the number at the same
    /// position in the source. The `missing` and `extra` cases are left unfixed
    /// because inserting a missing number at the right position in the prose or
    /// choosing which extra to drop both require translator judgement.
    fn check_msg(
        &self,
        checker: &Checker,
        entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let id_numbers: Vec<_> =
            FormatNumberPos::new(&msgid.value, entry.format_language).collect();
        let str_numbers: Vec<_> =
            FormatNumberPos::new(&msgstr.value, entry.format_language).collect();
        let id_values: Vec<String> = id_numbers.iter().map(|m| normalize(m.s)).collect();
        let str_values: Vec<String> = str_numbers.iter().map(|m| normalize(m.s)).collect();
        if same_values(&id_values, &str_values) {
            return vec![];
        }
        // The digits glued to a letter were skipped as part of an identifier, but a unit is
        // often detached from its value in the translation ("8MB" becomes "8 Mo"), which
        // makes the very same number an identifier on one side and a value on the other.
        // Compare again with the letters read as a unit on both sides: if the numbers then
        // match, only the spacing around the unit changed.
        if same_values(
            &unit_values(&msgid.value, entry.format_language),
            &unit_values(&msgstr.value, entry.format_language),
        ) {
            return vec![];
        }
        match id_numbers.len().cmp(&str_numbers.len()) {
            std::cmp::Ordering::Greater => self
                .new_diag(
                    checker,
                    Severity::Warning,
                    format!(
                        "missing numbers ({} / {})",
                        id_numbers.len(),
                        str_numbers.len()
                    ),
                )
                .map(|d| {
                    d.with_msgs_hl(
                        msgid,
                        id_numbers.iter().map(|m| (m.start, m.end)),
                        msgstr,
                        str_numbers.iter().map(|m| (m.start, m.end)),
                    )
                })
                .into_iter()
                .collect(),
            std::cmp::Ordering::Less => self
                .new_diag(
                    checker,
                    Severity::Warning,
                    format!(
                        "extra numbers ({} / {})",
                        id_numbers.len(),
                        str_numbers.len()
                    ),
                )
                .map(|d| {
                    d.with_msgs_hl(
                        msgid,
                        id_numbers.iter().map(|m| (m.start, m.end)),
                        msgstr,
                        str_numbers.iter().map(|m| (m.start, m.end)),
                    )
                })
                .into_iter()
                .collect(),
            std::cmp::Ordering::Equal => {
                let edits: Vec<Edit> = (0..id_numbers.len())
                    .filter(|&i| id_values[i] != str_values[i])
                    .map(|i| Edit {
                        range: str_numbers[i].start..str_numbers[i].end,
                        replacement: id_numbers[i].s.to_string(),
                    })
                    .collect();
                let fix = (!edits.is_empty()).then(|| Fix {
                    target: FixTarget::Msgstr {
                        file_byte_range: msgstr.byte_range.clone(),
                    },
                    edits,
                    safe: false,
                });
                self.new_diag(checker, Severity::Warning, "different numbers")
                    .map(|d| {
                        d.with_msgs_hl(
                            msgid,
                            id_numbers.iter().map(|m| (m.start, m.end)),
                            msgstr,
                            str_numbers.iter().map(|m| (m.start, m.end)),
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

    fn check_numbers(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(NumbersRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_normalize() {
        assert_eq!(normalize("42"), "42");
        // Digit-group separators are removed, whatever the locale convention.
        assert_eq!(normalize("1,000"), "1000");
        assert_eq!(normalize("1 000"), "1000");
        assert_eq!(normalize("1\u{202F}000"), "1000");
        assert_eq!(normalize("1.000"), "1000");
        assert_eq!(normalize("1'000"), "1000");
        // The decimal separator is folded to '.'.
        assert_eq!(normalize("1.5"), "1.5");
        assert_eq!(normalize("1,5"), "1.5");
        assert_eq!(normalize("1\u{066B}5"), "1.5");
        // Both separators at once.
        assert_eq!(normalize("1,234,567.89"), "1234567.89");
        assert_eq!(normalize("1.234.567,89"), "1234567.89");
        assert_eq!(normalize("1 234 567,89"), "1234567.89");
        // Digits of other scripts are mapped to ASCII.
        assert_eq!(normalize("٣"), "3");
        assert_eq!(normalize("३"), "3");
        assert_eq!(normalize("１２３"), "123");
    }

    #[test]
    fn test_no_numbers() {
        let diags = check_numbers(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_numbers_ok() {
        let diags = check_numbers(
            r#"
msgid "Press 3 times to confirm"
msgstr "Appuyez 3 fois pour confirmer"

msgid "1,000 files and 1.5 GB"
msgstr "1 000 fichiers et 1,5 Go"

msgid "Press 3 times"
msgstr "اضغط ٣ مرات"

msgid "Created on 2026-02-01"
msgstr "Créé le 01/02/2026"
"#,
        );
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn test_numbers_glued_to_letters_are_ignored() {
        let diags = check_numbers(
            r#"
msgid "Play MP3 files in 3D, 1st try, 16px wide"
msgstr "Lire des fichiers MP4 en 4D, 2e essai, 32px de large"
"#,
        );
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn test_numbers_with_detached_unit_are_ok() {
        // A unit glued to its value in the source is routinely detached in the
        // translation, which must not turn the value into an extra number.
        let diags = check_numbers(
            r#"
msgid "Download the 8MB file"
msgstr "Télécharger le fichier de 8 Mo"

msgid "Image is 16px wide"
msgstr "L'image fait 16 px de large"

msgid "Play the 8MB MP3 file"
msgstr "Lire le fichier MP3 de 8 Mo"

msgid "Wait 500ms"
msgstr "Attendre 500 ms"
"#,
        );
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn test_numbers_with_detached_unit_still_report_a_wrong_value() {
        // The unit reading is applied to both sides, so a value that really changed is
        // still reported.
        let diags = check_numbers(
            r#"
msgid "Download the 8MB file"
msgstr "Télécharger le fichier de 9 Mo"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "extra numbers (0 / 1)");
    }

    #[test]
    fn test_numbers_spelled_out_unit_is_still_reported() {
        // The fallback must not swallow a number dropped from the translation.
        let diags = check_numbers(
            r#"
msgid "Wait 3 seconds and press 5 times"
msgstr "Attendez 3 secondes"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "missing numbers (2 / 1)");
    }

    #[test]
    fn test_numbers_in_format_strings_are_ignored() {
        let diags = check_numbers(
            r#"
#, c-format
msgid "Copy %1$s to %2$s"
msgstr "Copier %2$s vers %1$s"
"#,
        );
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn test_numbers_error() {
        let diags = check_numbers(
            r#"
msgid "missing number: 3 of 5"
msgstr "nombre manquant : 3"

msgid "extra number: 3"
msgstr "nombre en trop : 3 sur 5"

msgid "different number: press 3 times"
msgstr "nombre différent : appuyez 2 fois"
"#,
        );
        assert_eq!(diags.len(), 3);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "missing numbers (2 / 1)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "extra numbers (1 / 2)");
        let diag = &diags[2];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "different numbers");
    }

    #[test]
    fn test_numbers_swapped_are_ok() {
        // Numbers are compared as a multiset: a reordered translation is correct.
        let diags = check_numbers(
            r#"
msgid "3 files in 5 directories"
msgstr "dans 5 répertoires : 3 fichiers"
"#,
        );
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn test_repeated_numbers_are_counted() {
        // A set comparison would miss this: both sides use only 3 and 5.
        let diags = check_numbers(
            r#"
msgid "3, 3 and 5"
msgstr "3, 5 et 5"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "different numbers");
    }

    #[test]
    fn test_different_numbers_fix_replaces_each_in_place() {
        let diags = check_numbers(
            r#"
msgid "Press 3 times, wait 5 seconds"
msgstr "Appuyez 2 fois, attendez 4 secondes"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "different numbers");
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 2);
        assert_eq!(fix.edits[0].replacement, "3");
        assert_eq!(fix.edits[1].replacement, "5");
    }

    #[test]
    fn test_different_numbers_fix_skips_positions_already_equal() {
        let diags = check_numbers(
            r#"
msgid "Press 3 times, wait 5 seconds"
msgstr "Appuyez 3 fois, attendez 4 secondes"
"#,
        );
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].replacement, "5");
    }

    #[test]
    fn test_missing_and_extra_numbers_have_no_fix() {
        let diags = check_numbers(
            r#"
msgid "3 files in 5 directories"
msgstr "3 fichiers"

msgid "3 files"
msgstr "3 fichiers dans 5 répertoires"
"#,
        );
        assert_eq!(diags.len(), 2);
        assert!(
            diags[0].fix.is_none(),
            "missing numbers diagnostic must not carry a fix"
        );
        assert!(
            diags[1].fix.is_none(),
            "extra numbers diagnostic must not carry a fix"
        );
    }
}
