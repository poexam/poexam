// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `plural-forms` rule: check that the plural
//! expression of the `Plural-Forms` header field is valid and matches the
//! plural rules of the language of the file.

use std::collections::BTreeMap;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Edit, Fix, FixTarget};
use crate::po::entry::Entry;
use crate::po::message::Message;
use crate::po::plural::{PluralExpr, parse_plural_forms};
use crate::rules::rule::RuleChecker;

pub struct PluralFormsRule;

/// Plural forms: number of forms and plural expression.
type PluralForms = (u32, &'static str);

// Plural forms shared by several languages: the first ones are those of the
// GNU gettext table (used by `msginit`), the others are variants found in
// real-world catalogs (CLDR, KDE, Weblate).
const ONE: PluralForms = (1, "0");
const NOT_ONE: PluralForms = (2, "(n != 1)");
const GT_ONE: PluralForms = (2, "(n > 1)");
const EAST_SLAVIC: PluralForms = (
    3,
    "(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2)",
);
const EAST_SLAVIC_KDE: PluralForms = (
    4,
    "n==1 ? 3 : n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2",
);
const CZECH: PluralForms = (3, "(n==1) ? 0 : (n>=2 && n<=4) ? 1 : 2");
const POLISH: PluralForms = (
    3,
    "(n==1 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2)",
);
const LITHUANIAN: PluralForms = (
    3,
    "(n%10==1 && n%100!=11 ? 0 : n%10>=2 && (n%100<10 || n%100>=20) ? 1 : 2)",
);
const LATVIAN: PluralForms = (3, "(n%10==1 && n%100!=11 ? 0 : n != 0 ? 1 : 2)");
const LATVIAN_CLDR: PluralForms = (
    3,
    "(n%10==0 || (n%100>=11 && n%100<=19)) ? 0 : (n%10==1 && n%100!=11) ? 1 : 2",
);
const ROMANIAN: PluralForms = (3, "n==1 ? 0 : (n==0 || (n%100 > 0 && n%100 < 20)) ? 1 : 2");
const SLOVENIAN: PluralForms = (
    4,
    "(n%100==1 ? 0 : n%100==2 ? 1 : n%100==3 || n%100==4 ? 2 : 3)",
);
const ONE_TWO_OTHER: PluralForms = (3, "n==1 ? 0 : n==2 ? 1 : 2");
const IRISH_CLDR: PluralForms = (
    5,
    "n==1 ? 0 : n==2 ? 1 : (n>2 && n<7) ? 2 : (n>6 && n<11) ? 3 : 4",
);
const HEBREW_OLD: PluralForms = (4, "(n==1) ? 0 : (n==2) ? 1 : (n>10 && n%10==0) ? 2 : 3");
const ARABIC: PluralForms = (
    6,
    "n==0 ? 0 : n==1 ? 1 : n==2 ? 2 : n%100>=3 && n%100<=10 ? 3 : n%100>=11 ? 4 : 5",
);
const WELSH: PluralForms = (4, "(n==1) ? 0 : (n==2) ? 1 : (n != 8 && n != 11) ? 2 : 3");
const WELSH_CLDR: PluralForms = (
    6,
    "(n==0) ? 0 : (n==1) ? 1 : (n==2) ? 2 : (n==3) ? 3 : (n==6) ? 4 : 5",
);
/// Irish plural forms used by Django (0 is in the "few" form).
const IRISH_DJANGO: PluralForms = (5, "(n==1 ? 0 : n==2 ? 1 : n<7 ? 2 : n<11 ? 3 : 4)");
const ICELANDIC: PluralForms = (2, "(n%10!=1 || n%100==11)");
const MACEDONIAN_OLD: PluralForms = (2, "(n==1 || n%10==1) ? 0 : 1");
/// CLDR "many" form for large round numbers (e.g. Spanish "1 millón de").
const NOT_ONE_MANY: PluralForms = (3, "n==1 ? 0 : n!=0 && n%1000000==0 ? 1 : 2");
/// CLDR "many" form for large round numbers (e.g. French "1 million de").
const GT_ONE_MANY: PluralForms = (3, "(n==0 || n==1) ? 0 : n!=0 && n%1000000==0 ? 1 : 2");

/// Accepted plural forms by language, sorted by language; the first plural
/// forms of each language are the ones suggested in diagnostics.
///
/// Languages that are not listed are not checked.
const LANGUAGE_PLURAL_FORMS: &[(&str, &[PluralForms])] = &[
    ("af", &[NOT_ONE]),
    ("am", &[GT_ONE]),
    ("ar", &[ARABIC]),
    ("az", &[NOT_ONE, ONE]),
    ("be", &[EAST_SLAVIC, EAST_SLAVIC_KDE]),
    ("bg", &[NOT_ONE]),
    ("bs", &[EAST_SLAVIC, EAST_SLAVIC_KDE]),
    ("ca", &[NOT_ONE, NOT_ONE_MANY]),
    ("cs", &[CZECH]),
    ("cy", &[WELSH, WELSH_CLDR]),
    ("da", &[NOT_ONE]),
    ("de", &[NOT_ONE]),
    ("el", &[NOT_ONE]),
    ("en", &[NOT_ONE]),
    ("eo", &[NOT_ONE]),
    ("es", &[NOT_ONE, NOT_ONE_MANY]),
    ("et", &[NOT_ONE]),
    ("eu", &[NOT_ONE]),
    ("fa", &[GT_ONE, ONE]),
    ("fi", &[NOT_ONE]),
    ("fo", &[NOT_ONE]),
    ("fr", &[GT_ONE, GT_ONE_MANY]),
    ("fy", &[NOT_ONE]),
    ("ga", &[ONE_TWO_OTHER, IRISH_CLDR, IRISH_DJANGO]),
    ("gl", &[NOT_ONE]),
    ("he", &[NOT_ONE, ONE_TWO_OTHER, HEBREW_OLD]),
    ("hr", &[EAST_SLAVIC, EAST_SLAVIC_KDE]),
    ("hu", &[NOT_ONE, ONE]),
    ("hy", &[GT_ONE, NOT_ONE]),
    ("id", &[ONE, NOT_ONE]),
    ("is", &[ICELANDIC, NOT_ONE]),
    ("it", &[NOT_ONE, NOT_ONE_MANY]),
    ("ja", &[ONE]),
    ("ka", &[NOT_ONE, ONE]),
    ("kk", &[NOT_ONE, ONE]),
    ("ko", &[ONE]),
    ("lt", &[LITHUANIAN]),
    ("lv", &[LATVIAN, LATVIAN_CLDR]),
    ("mk", &[ICELANDIC, MACEDONIAN_OLD]),
    ("ms", &[ONE, NOT_ONE]),
    ("nb", &[NOT_ONE]),
    ("nl", &[NOT_ONE]),
    ("nn", &[NOT_ONE]),
    ("no", &[NOT_ONE]),
    ("oc", &[GT_ONE]),
    ("pl", &[POLISH]),
    ("pt", &[NOT_ONE, GT_ONE, NOT_ONE_MANY, GT_ONE_MANY]),
    ("pt_BR", &[GT_ONE, GT_ONE_MANY]),
    ("pt_PT", &[NOT_ONE, NOT_ONE_MANY]),
    ("ro", &[ROMANIAN]),
    ("ru", &[EAST_SLAVIC, EAST_SLAVIC_KDE]),
    ("sk", &[CZECH]),
    ("sl", &[SLOVENIAN]),
    ("sq", &[NOT_ONE]),
    ("sr", &[EAST_SLAVIC, EAST_SLAVIC_KDE]),
    ("sv", &[NOT_ONE]),
    ("th", &[ONE]),
    ("tr", &[NOT_ONE, ONE, GT_ONE]),
    ("uk", &[EAST_SLAVIC, EAST_SLAVIC_KDE]),
    ("vi", &[ONE]),
    ("zh", &[ONE]),
];

/// Values of `n` the plural expressions are evaluated for: all numbers up to
/// 1000 (enough to cover the `n%100` and `n%10` rules) and a few large ones
/// (for the CLDR "many" form of large round numbers).
fn samples() -> impl Iterator<Item = u64> {
    (0..=1000).chain([
        10_000,
        100_000,
        1_000_000,
        1_000_001,
        2_000_000,
        1_000_000_000,
    ])
}

/// Return the accepted plural forms for a language (e.g. `pt_BR`, `sr_RS@latin`):
/// the full language is looked up first, then without variant, then the
/// language code alone.
fn language_plural_forms(language: &str) -> Option<&'static [PluralForms]> {
    let no_variant = language.split('@').next().unwrap_or(language);
    let code = no_variant.split('_').next().unwrap_or(no_variant);
    [language, no_variant, code].into_iter().find_map(|tag| {
        LANGUAGE_PLURAL_FORMS
            .iter()
            .find(|(lang, _)| *lang == tag)
            .map(|(_, forms)| *forms)
    })
}

/// Return `true` if the expression splits the [samples] into the same
/// groups of numbers as the expected one, whatever the index of each group.
///
/// The indices are only labels for the `msgstr[N]` of the file: plural forms
/// listed in another order (e.g. `n==1 ? 1 : 0`), or with an extra form never
/// used for integers (e.g. Transifex forms for fractions), are correct as long
/// as the same numbers share a plural form.
fn same_plural_groups(expr: &PluralExpr, expected: &PluralExpr) -> bool {
    // Pairs (index in `expr`, index in `expected`), must be a one-to-one mapping.
    let mut mapping: Vec<(u64, u64)> = Vec::new();
    samples().all(|n| {
        let (Some(index), Some(expected_index)) = (expr.eval(n), expected.eval(n)) else {
            return false;
        };
        if let Some(&(i, e)) = mapping
            .iter()
            .find(|(i, e)| *i == index || *e == expected_index)
        {
            return i == index && e == expected_index;
        }
        mapping.push((index, expected_index));
        true
    })
}

/// Return `true` if each plural form of the expression has the same meaning as
/// the form with the same index in the expected expression: for each index
/// returned by the expression, most of the [samples] with this index
/// have the same index in the expected expression.
///
/// For example with French, `n != 1` has the same meaning as `n > 1` (form 0
/// is singular, form 1 is plural), and not `n == 1` (form 1 is singular): the
/// translator wrote the `msgstr[N]` for these meanings, so replacing the
/// expression is a fix only if the meanings are kept.
fn same_form_meaning(expr: &PluralExpr, expected: &PluralExpr) -> bool {
    // Counts of (index in `expr`, index in `expected`).
    let mut counts: BTreeMap<(u64, u64), usize> = BTreeMap::new();
    for n in samples() {
        if let (Some(index), Some(expected_index)) = (expr.eval(n), expected.eval(n)) {
            *counts.entry((index, expected_index)).or_default() += 1;
        }
    }
    let mut majority: BTreeMap<u64, (usize, u64)> = BTreeMap::new();
    for (&(index, expected_index), &count) in &counts {
        let best = majority.entry(index).or_insert((0, expected_index));
        if count > best.0 {
            *best = (count, expected_index);
        }
    }
    majority
        .iter()
        .all(|(index, (_, expected_index))| index == expected_index)
}

/// Build the fix replacing the value of the `Plural-Forms` field (at `start..end`
/// in the header) with the given plural forms.
fn plural_forms_fix(
    msgstr: &Message,
    start: usize,
    end: usize,
    (nplurals, expr): PluralForms,
    safe: bool,
) -> Fix {
    Fix {
        target: FixTarget::Msgstr {
            file_byte_range: msgstr.byte_range.clone(),
        },
        edits: vec![Edit {
            range: start..end,
            replacement: format!("nplurals={nplurals}; plural={expr};"),
        }],
        safe,
    }
}

/// Find the `Plural-Forms` field in the header, return its value and the
/// position of the value in the header (start and end offsets).
fn find_plural_forms(header: &str) -> Option<(&str, usize, usize)> {
    let mut offset = 0;
    for line in header.split('\n') {
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("Plural-Forms")
        {
            let trimmed = value.trim_start();
            let start = offset + name.len() + 1 + value.len() - trimmed.len();
            let trimmed = trimmed.trim_end();
            return Some((trimmed, start, start + trimmed.len()));
        }
        offset += line.len() + 1;
    }
    None
}

impl RuleChecker for PluralFormsRule {
    fn name(&self) -> &'static str {
        "plural-forms"
    }

    fn description(&self) -> &'static str {
        "Invalid plural expression or plural forms not matching the language in PO file header."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check the plural expression of the `Plural-Forms` header field.
    ///
    /// The expression must be valid, must not divide by zero and must return
    /// an index lower than `nplurals`: otherwise gettext selects no or a wrong
    /// plural form at runtime, whatever the language.
    ///
    /// Then, when the `Language` field is known, the plural forms must match
    /// the plural rules of the language: the expression must split numbers
    /// into the same groups, each group sharing a plural form. They are
    /// compared by evaluating the expressions for many values of `n`, so
    /// equivalent expressions written differently (e.g. `(n > 1)` and `n>1`)
    /// are accepted, as well as forms listed in another order (the order of
    /// `msgstr[N]` follows the header) or an extra form never used for integers
    /// (e.g. Transifex forms for fractions). Several variants are accepted for
    /// some languages, like the plural forms of GNU gettext, CLDR and Django.
    /// Languages without known plural forms are not checked.
    ///
    /// A missing `Plural-Forms` field or a missing or invalid `nplurals` is
    /// reported by the `header` rule.
    ///
    /// Auto-fix: the value of `Plural-Forms` is replaced with the plural forms
    /// of the language, only if the number of forms is the same (otherwise
    /// forms would have to be added to or removed from all plural entries).
    /// When the plural forms do not match the language, the fix is offered only
    /// if each form keeps its meaning (see [`same_form_meaning`]); it is safe
    /// with 2 forms (singular and plural) and unsafe with more forms. The fix
    /// of an invalid expression is always unsafe.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid ""
    /// msgstr ""
    /// "Language: fr\n"
    /// "Plural-Forms: nplurals=2; plural=(n != 1);\n"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid ""
    /// msgstr ""
    /// "Language: fr\n"
    /// "Plural-Forms: nplurals=2; plural=(n > 1);\n"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`error`](Severity::Error): `invalid plural expression '…'`
    /// - [`error`](Severity::Error): `division by zero in plural expression '…' (n = #)`
    /// - [`error`](Severity::Error): `plural expression '…' returns # for n = #, out of range for nplurals=#`
    /// - [`warning`](Severity::Warning): `plural forms '…' do not match language '…' (expected: '…')`
    fn check_header(&self, checker: &Checker, _entry: &Entry, msgstr: &Message) -> Vec<Diagnostic> {
        let Some((value, start, end)) = find_plural_forms(&msgstr.value) else {
            return vec![];
        };
        let Some((nplurals, expr_str)) = parse_plural_forms(value) else {
            return vec![];
        };
        if nplurals == 0 {
            return vec![];
        }
        let language = checker.language();
        let accepted = language_plural_forms(language);
        // Plural forms of the language replacing the current ones in the fix:
        // only with the same number of forms, so that all `msgstr[N]` are kept.
        let fix_forms = accepted
            .map(|forms| forms[0])
            .filter(|(expected_nplurals, _)| *expected_nplurals == nplurals);
        let diag = |severity, message: String, fix: Option<Fix>| {
            self.new_diag(checker, severity, message)
                .map(|d| d.with_msg_hl(msgstr, [(start, end)]).with_optional_fix(fix))
                .into_iter()
                .collect()
        };
        // Fix of an invalid expression: the meaning of the forms is unknown.
        let unsafe_fix =
            || fix_forms.map(|forms| plural_forms_fix(msgstr, start, end, forms, false));
        let Some(expr) = PluralExpr::parse(expr_str) else {
            return diag(
                Severity::Error,
                format!("invalid plural expression '{expr_str}'"),
                unsafe_fix(),
            );
        };
        for n in samples() {
            match expr.eval(n) {
                None => {
                    return diag(
                        Severity::Error,
                        format!("division by zero in plural expression '{expr_str}' (n = {n})"),
                        unsafe_fix(),
                    );
                }
                Some(index) if index >= u64::from(nplurals) => {
                    return diag(
                        Severity::Error,
                        format!(
                            "plural expression '{expr_str}' returns {index} for n = {n}, \
                             out of range for nplurals={nplurals}"
                        ),
                        unsafe_fix(),
                    );
                }
                Some(_) => {}
            }
        }
        let Some(accepted) = accepted else {
            return vec![];
        };
        if accepted
            .iter()
            .filter_map(|(_, expected)| PluralExpr::parse(expected))
            .any(|expected| same_plural_groups(&expr, &expected))
        {
            return vec![];
        }
        let fix = fix_forms.and_then(|forms| {
            let expected = PluralExpr::parse(forms.1)?;
            same_form_meaning(&expr, &expected)
                .then(|| plural_forms_fix(msgstr, start, end, forms, nplurals <= 2))
        });
        let (expected_nplurals, expected_expr) = accepted[0];
        diag(
            Severity::Warning,
            format!(
                "plural forms '{value}' do not match language '{language}' \
                 (expected: 'nplurals={expected_nplurals}; plural={expected_expr};')"
            ),
            fix,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    /// Check a header with the given `Language` (if not empty) and `Plural-Forms`.
    fn check(language: &str, plural_forms: &str) -> Vec<Diagnostic> {
        let language = if language.is_empty() {
            String::new()
        } else {
            format!("\"Language: {language}\\n\"\n")
        };
        let content = format!(
            "msgid \"\"\nmsgstr \"\"\n{language}\
             \"Content-Type: text/plain; charset=UTF-8\\n\"\n\
             \"Plural-Forms: {plural_forms}\\n\"\n"
        );
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(PluralFormsRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    fn assert_single(diags: &[Diagnostic], severity: Severity, message: &str) {
        assert_eq!(diags.len(), 1, "expected one diagnostic: {diags:?}");
        assert_eq!(diags[0].severity, severity);
        assert_eq!(diags[0].message, message);
    }

    #[test]
    fn test_table() {
        // Languages are sorted and unique.
        for pair in LANGUAGE_PLURAL_FORMS.windows(2) {
            assert!(pair[0].0 < pair[1].0, "{} >= {}", pair[0].0, pair[1].0);
        }
        // All plural forms are valid and return an index lower than nplurals.
        for (lang, forms) in LANGUAGE_PLURAL_FORMS {
            assert!(!forms.is_empty(), "{lang}");
            for (nplurals, expr) in *forms {
                let parsed = PluralExpr::parse(expr).unwrap_or_else(|| panic!("{lang}: {expr}"));
                for n in samples() {
                    let index = parsed.eval(n).unwrap();
                    assert!(index < u64::from(*nplurals), "{lang}: {expr} (n = {n})");
                }
            }
        }
    }

    #[test]
    fn test_language_plural_forms() {
        assert_eq!(
            language_plural_forms("fr"),
            Some(&[GT_ONE, GT_ONE_MANY][..])
        );
        assert_eq!(
            language_plural_forms("fr_CA"),
            Some(&[GT_ONE, GT_ONE_MANY][..])
        );
        assert_eq!(
            language_plural_forms("pt_BR"),
            Some(&[GT_ONE, GT_ONE_MANY][..])
        );
        assert_eq!(
            language_plural_forms("sr_RS@latin"),
            Some(&[EAST_SLAVIC, EAST_SLAVIC_KDE][..])
        );
        assert_eq!(language_plural_forms("xx"), None);
        assert_eq!(language_plural_forms(""), None);
    }

    #[test]
    fn test_same_plural_groups() {
        let same = |a: &str, b: &str| {
            same_plural_groups(
                &PluralExpr::parse(a).unwrap(),
                &PluralExpr::parse(b).unwrap(),
            )
        };
        assert!(same("n != 1", "n != 1"));
        assert!(same("n != 1", "n == 1"));
        assert!(same("n != 1", "n == 1 ? 5 : 3"));
        assert!(!same("n != 1", "n > 1"));
        assert!(!same("n != 1", "0"));
        assert!(!same("0", "n != 1"));
        assert!(!same("n == 1 ? 0 : n == 2 ? 1 : 2", "n == 1 ? 0 : 1"));
        assert!(!same("1 / n", "n != 1"));
    }

    #[test]
    fn test_find_plural_forms() {
        let header = "Language: fr\nPlural-Forms:  nplurals=2; plural=(n > 1); \n";
        let (value, start, end) = find_plural_forms(header).unwrap();
        assert_eq!(value, "nplurals=2; plural=(n > 1);");
        assert_eq!(&header[start..end], value);
        assert!(find_plural_forms("Language: fr\n").is_none());
    }

    #[test]
    fn test_matching_plural_forms() {
        assert!(check("fr", "nplurals=2; plural=(n > 1);").is_empty());
        assert!(check("fr", "nplurals=2; plural=n>1").is_empty());
        assert!(check("fr", "nplurals=2; plural=(n == 0 || n == 1) ? 0 : 1;").is_empty());
        assert!(
            check(
                "fr",
                "nplurals=3; plural=(n == 0 || n == 1) ? 0 : n != 0 && n % 1000000 == 0 ? 1 : 2;"
            )
            .is_empty()
        );
        assert!(check("en_US", "nplurals=2; plural=n != 1;").is_empty());
        assert!(check("de", "nplurals=2; plural=(n==1) ? 0 : 1;").is_empty());
        assert!(check("ja", "nplurals=1; plural=0;").is_empty());
        assert!(check("tr", "nplurals=1; plural=0;").is_empty());
        assert!(check("tr", "nplurals=2; plural=(n != 1);").is_empty());
        assert!(
            check(
                "ru",
                "nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && \
                 (n%100<10 || n%100>=20) ? 1 : 2);"
            )
            .is_empty()
        );
        assert!(
            check(
                "sr@latin",
                "nplurals=4; plural=n==1 ? 3 : n%10==1 && n%100!=11 ? 0 : \
                 n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2;"
            )
            .is_empty()
        );
        assert!(check("pt_PT", "nplurals=2; plural=(n != 1);").is_empty());
        // Forms in another order.
        assert!(
            check(
                "sl",
                "nplurals=4; plural=(n%100==1 ? 1 : n%100==2 ? 2 : n%100==3 || n%100==4 ? 3 : 0);"
            )
            .is_empty()
        );
        // Extra form for fractions, never used for integers (Transifex).
        assert!(
            check(
                "cs",
                "nplurals=4; plural=(n == 1 && n % 1 == 0) ? 0 : (n >= 2 && n <= 4 && n % 1 == 0) \
                 ? 1: (n % 1 != 0 ) ? 2 : 3;"
            )
            .is_empty()
        );
        // Extra unused form.
        assert!(check("de", "nplurals=3; plural=(n != 1);").is_empty());
    }

    #[test]
    fn test_unknown_language() {
        assert!(check("", "nplurals=2; plural=(n != 1);").is_empty());
        assert!(check("xx", "nplurals=2; plural=(n != 1);").is_empty());
    }

    #[test]
    fn test_missing_or_invalid_nplurals() {
        // Reported by the `header` rule.
        assert!(check("fr", "nplurals=0; plural=0;").is_empty());
        assert!(check("fr", "nplurals=x; plural=0;").is_empty());
        assert!(check("fr", "plural=(n > 1);").is_empty());
    }

    #[test]
    fn test_mismatching_plural_forms() {
        assert_single(
            &check("fr", "nplurals=2; plural=(n != 1);"),
            Severity::Warning,
            "plural forms 'nplurals=2; plural=(n != 1);' do not match language 'fr' \
             (expected: 'nplurals=2; plural=(n > 1);')",
        );
        assert_single(
            &check("pt_BR", "nplurals=2; plural=(n != 1);"),
            Severity::Warning,
            "plural forms 'nplurals=2; plural=(n != 1);' do not match language 'pt_BR' \
             (expected: 'nplurals=2; plural=(n > 1);')",
        );
        assert_single(
            &check("ja", "nplurals=2; plural=(n != 1);"),
            Severity::Warning,
            "plural forms 'nplurals=2; plural=(n != 1);' do not match language 'ja' \
             (expected: 'nplurals=1; plural=0;')",
        );
        // Plural forms of another language.
        assert_eq!(
            check(
                "pl",
                "nplurals=3; plural=(n==1) ? 0 : (n>=2 && n<=4) ? 1 : 2;"
            )
            .len(),
            1
        );
    }

    #[test]
    fn test_invalid_expression() {
        assert_single(
            &check("fr", "nplurals=2; plural=(n > 1;"),
            Severity::Error,
            "invalid plural expression '(n > 1'",
        );
        assert_single(
            &check("", "nplurals=2; plural=;"),
            Severity::Error,
            "invalid plural expression ''",
        );
    }

    #[test]
    fn test_division_by_zero() {
        assert_single(
            &check("fr", "nplurals=2; plural=1 / n;"),
            Severity::Error,
            "division by zero in plural expression '1 / n' (n = 0)",
        );
    }

    #[test]
    fn test_index_out_of_range() {
        // Russian expression with only 2 forms, reported whatever the language.
        let diags = check(
            "",
            "nplurals=2; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && \
             (n%100<10 || n%100>=20) ? 1 : 2);",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Error);
        assert!(
            diags[0]
                .message
                .ends_with("returns 2 for n = 0, out of range for nplurals=2"),
            "{}",
            diags[0].message
        );
        assert_single(
            &check("ja", "nplurals=1; plural=n != 1;"),
            Severity::Error,
            "plural expression 'n != 1' returns 1 for n = 0, out of range for nplurals=1",
        );
    }

    /// Return the fix of the single diagnostic: (safe, header after the fix).
    fn check_fix(language: &str, plural_forms: &str) -> Option<(bool, String)> {
        let diags = check(language, plural_forms);
        assert_eq!(diags.len(), 1, "expected one diagnostic: {diags:?}");
        let fix = diags[0].fix.as_ref()?;
        assert!(matches!(fix.target, FixTarget::Msgstr { .. }));
        let header = header_value(language, plural_forms);
        let fixed = crate::fix::apply_msgstr_fixes(&header, &fix.edits).unwrap();
        Some((fix.safe, fixed))
    }

    /// Value of the header built by [`check`].
    fn header_value(language: &str, plural_forms: &str) -> String {
        format!(
            "Language: {language}\nContent-Type: text/plain; charset=UTF-8\n\
             Plural-Forms: {plural_forms}\n"
        )
    }

    #[test]
    fn test_fix_two_forms() {
        // Same meaning of the forms (0: singular, 1: plural): safe fix.
        assert_eq!(
            check_fix("fr", "nplurals=2; plural=(n != 1);"),
            Some((true, header_value("fr", "nplurals=2; plural=(n > 1);")))
        );
        assert_eq!(
            check_fix("it", "nplurals=2; plural=n>1"),
            Some((true, header_value("it", "nplurals=2; plural=(n != 1);")))
        );
        assert_eq!(
            check_fix("pt_BR", "nplurals=2; plural=(n != 1);"),
            Some((true, header_value("pt_BR", "nplurals=2; plural=(n > 1);")))
        );
        // Form 1 is the singular: replacing the expression would swap the forms.
        assert_eq!(check_fix("fr", "nplurals=2; plural=(n == 1);"), None);
    }

    #[test]
    fn test_fix_more_forms() {
        // Polish expression in a Russian file: same meaning of the 3 forms, unsafe fix.
        let (safe, fixed) = check_fix(
            "ru",
            "nplurals=3; plural=(n==1 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);",
        )
        .unwrap();
        assert!(!safe);
        assert_eq!(
            fixed,
            header_value(
                "ru",
                "nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && \
                 (n%100<10 || n%100>=20) ? 1 : 2);"
            )
        );
        // Lithuanian expression in a Russian file: form 1 is mostly "many" in Russian.
        assert_eq!(
            check_fix(
                "ru",
                "nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && (n%100<10 || n%100>=20) ? 1 : 2);"
            ),
            None
        );
    }

    #[test]
    fn test_no_fix_different_nplurals() {
        assert_eq!(check_fix("ru", "nplurals=2; plural=(n != 1);"), None);
        assert_eq!(check_fix("ja", "nplurals=2; plural=(n != 1);"), None);
        assert_eq!(check_fix("it", "nplurals=1; plural=0;"), None);
    }

    #[test]
    fn test_fix_errors() {
        // Invalid expression: unsafe fix if the language is known with the same nplurals.
        assert_eq!(
            check_fix("fr", "nplurals=2; plural=(n > 1;"),
            Some((false, header_value("fr", "nplurals=2; plural=(n > 1);")))
        );
        assert_eq!(
            check_fix("fr", "nplurals=2; plural=1 / n;"),
            Some((false, header_value("fr", "nplurals=2; plural=(n > 1);")))
        );
        assert_eq!(
            check_fix("fr", "nplurals=2; plural=n;"),
            Some((false, header_value("fr", "nplurals=2; plural=(n > 1);")))
        );
        assert_eq!(check_fix("", "nplurals=2; plural=(n > 1;"), None);
        assert_eq!(check_fix("xx", "nplurals=2; plural=(n > 1;"), None);
        assert_eq!(check_fix("ru", "nplurals=2; plural=(n > 1;"), None);
    }

    #[test]
    fn test_noqa() {
        let content = "#, noqa:plural-forms\nmsgid \"\"\nmsgstr \"\"\n\
                       \"Language: fr\\n\"\n\
                       \"Plural-Forms: nplurals=2; plural=(n != 1);\\n\"\n";
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(PluralFormsRule {})]);
        checker.do_all_checks(&rules);
        assert!(checker.diagnostics.is_empty());
    }
}
