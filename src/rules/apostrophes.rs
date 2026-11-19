// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `apostrophes` rule: check for apostrophes not
//! consistent with the rest of the file (`'` and `’`).

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Edit, Fix, FixTarget};
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct ApostrophesRule;

/// Straight apostrophe (U+0027 APOSTROPHE).
const STRAIGHT: char = '\'';

/// Typographic apostrophe (U+2019 RIGHT SINGLE QUOTATION MARK).
const TYPOGRAPHIC: char = '’';

/// Return `true` if the character is a letter which can be next to an
/// apostrophe: the CJK scripts (Chinese, Japanese and Korean) have no
/// apostrophes, the `'` next to their characters are quotes.
fn is_letter(c: char) -> bool {
    c.is_alphabetic()
        && !matches!(c,
            '\u{2E80}'..='\u{9FFF}' | '\u{AC00}'..='\u{D7AF}' | '\u{F900}'..='\u{FAFF}')
}

/// Return the letters of `value` around the apostrophe at `start..end`, with
/// the apostrophe (e.g. `d'abord` in `d'abord,` or `d'O` in `d'O'Reilly`).
fn letters_around(value: &str, start: usize, end: usize) -> &str {
    let first = value[..start]
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_alphabetic())
        .last()
        .map_or(start, |(pos, _)| pos);
    let last = value[end..]
        .find(|c: char| !c.is_alphabetic())
        .map_or(value.len(), |pos| end + pos);
    &value[first..last]
}

/// Return the positions of the apostrophes of `value` (`'` or `’`) used as
/// apostrophes, between two letters (e.g. `l'option`, `don't`): the quotes
/// around a word (e.g. `'name'`) are ignored, as well as the letters copied
/// from the sources around the apostrophe (e.g. `Yu'v'`, a notation, or
/// `c't`, a name).
fn apostrophes<'a>(
    value: &'a str,
    sources: &'a [&'a str],
) -> impl Iterator<Item = (usize, char)> + 'a {
    value
        .match_indices([STRAIGHT, TYPOGRAPHIC])
        .filter_map(move |(idx, s)| {
            let c = s.chars().next()?;
            let before = value[..idx].chars().next_back()?;
            let after = value[idx + s.len()..].chars().next()?;
            (is_letter(before)
                && is_letter(after)
                && !sources
                    .iter()
                    .any(|src| src.contains(letters_around(value, idx, idx + s.len()))))
            .then_some((idx, c))
        })
}

/// Return the sources of an entry (`msgid` and `msgid_plural`).
fn sources(entry: &Entry) -> Vec<&str> {
    entry
        .msgid
        .iter()
        .chain(entry.msgid_plural.iter())
        .map(|msg| msg.value.as_str())
        .collect()
}

/// Return the number of straight and typographic apostrophes of `value`.
fn count_apostrophes(value: &str, sources: &[&str]) -> (usize, usize) {
    apostrophes(value, sources).fold((0, 0), |(straight, typo), (_, c)| {
        if c == STRAIGHT {
            (straight + 1, typo)
        } else {
            (straight, typo + 1)
        }
    })
}

impl RuleChecker for ApostrophesRule {
    fn name(&self) -> &'static str {
        "apostrophes"
    }

    fn description(&self) -> &'static str {
        "Check for apostrophes not consistent with the rest of the file."
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    fn uses_entries(&self) -> bool {
        true
    }

    /// Check for apostrophes not consistent with the rest of the file: the
    /// translations must use either the straight apostrophe `'` (U+0027) or
    /// the typographic apostrophe `’` (U+2019), not both.
    ///
    /// The apostrophe used the most in the translations of the file is the
    /// expected one, the typographic apostrophe in case of tie. Only the
    /// apostrophes between two letters are checked (e.g. `l'option`,
    /// `don't`): quotes around a word (e.g. `'name'`) are ignored, as well as
    /// the words copied from the source (e.g. the notation `Yu'v'` or the name
    /// `O'Reilly`).
    ///
    /// Wrong entries:
    /// ```text
    /// msgid "Open the file"
    /// msgstr "Ouvrir l’archive"
    ///
    /// msgid "Save the file"
    /// msgstr "Enregistrer l’archive"
    ///
    /// msgid "Close the file"
    /// msgstr "Fermer l'archive"
    /// ```
    ///
    /// Correct entries:
    /// ```text
    /// msgid "Open the file"
    /// msgstr "Ouvrir l’archive"
    ///
    /// msgid "Save the file"
    /// msgstr "Enregistrer l’archive"
    ///
    /// msgid "Close the file"
    /// msgstr "Fermer l’archive"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`info`](Severity::Info): `inconsistent apostrophe '…' (file: # ’ / # ')`
    ///   (auto-fixable)
    fn check_entries(&self, checker: &Checker, entries: &[&Entry]) -> Vec<Diagnostic> {
        let (straight, typo) = entries
            .iter()
            .flat_map(|entry| {
                let sources = sources(entry);
                entry
                    .msgstr
                    .values()
                    .map(|msg| count_apostrophes(&msg.value, &sources))
                    .collect::<Vec<_>>()
            })
            .fold((0, 0), |(s1, t1), (s2, t2)| (s1 + s2, t1 + t2));
        if straight == 0 || typo == 0 {
            return vec![];
        }
        let (wrong, expected) = if straight > typo {
            (TYPOGRAPHIC, STRAIGHT)
        } else {
            (STRAIGHT, TYPOGRAPHIC)
        };
        let mut diags = vec![];
        for entry in entries {
            let Some(msgid) = &entry.msgid else {
                continue;
            };
            let sources = sources(entry);
            for msgstr in entry.msgstr.values() {
                let positions: Vec<usize> = apostrophes(&msgstr.value, &sources)
                    .filter(|(_, c)| *c == wrong)
                    .map(|(idx, _)| idx)
                    .collect();
                if positions.is_empty() {
                    continue;
                }
                let Some(diag) = self.new_diag(
                    checker,
                    Severity::Info,
                    format!("inconsistent apostrophe '{wrong}' (file: {typo} ’ / {straight} ')"),
                ) else {
                    continue;
                };
                let len = wrong.len_utf8();
                let fix = Fix {
                    target: FixTarget::Msgstr {
                        file_byte_range: msgstr.byte_range.clone(),
                    },
                    edits: positions
                        .iter()
                        .map(|idx| Edit {
                            range: *idx..*idx + len,
                            replacement: expected.to_string(),
                        })
                        .collect(),
                    safe: true,
                };
                diags.push(
                    diag.with_msgs_hl(
                        msgid,
                        [],
                        msgstr,
                        positions.iter().map(|idx| (*idx, *idx + len)),
                    )
                    .with_fix(fix),
                );
            }
        }
        diags
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use super::*;
    use crate::rules::rule::Rules;

    const HEADER: &str = "msgid \"\"\nmsgstr \"Content-Type: text/plain; charset=UTF-8\\n\"\n\n";

    fn check(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(ApostrophesRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    /// Build a file with the given translations.
    fn file(translations: &[&str]) -> String {
        let mut content = HEADER.to_string();
        for (i, msgstr) in translations.iter().enumerate() {
            let _ = write!(content, "msgid \"Source {i}\"\nmsgstr \"{msgstr}\"\n\n");
        }
        content
    }

    /// Return the edits of the fix of a diagnostic.
    fn edits(diag: &Diagnostic) -> Vec<(std::ops::Range<usize>, String)> {
        diag.fix
            .as_ref()
            .expect("fix")
            .edits
            .iter()
            .map(|e| (e.range.clone(), e.replacement.clone()))
            .collect()
    }

    #[test]
    fn test_apostrophes() {
        let found: Vec<_> =
            apostrophes("l'option « d’abord » 'name' it's 1'2 a' 'b", &[]).collect();
        assert_eq!(found, vec![(1, '\''), (13, '’'), (34, '\'')]);
        // Words copied from the source.
        let found: Vec<_> = apostrophes(
            "CIE Yu'v' : l'espace d'O'Reilly",
            &["CIE Yu'v' by O'Reilly"],
        )
        .collect();
        assert_eq!(found, vec![(13, '\''), (22, '\'')]);
        assert_eq!(letters_around("(l'option)", 2, 3), "l'option");
        // Quotes in CJK scripts.
        assert_eq!(apostrophes("檔案'%s'出現", &[]).count(), 0);
        assert_eq!(letters_around("d'O'Reilly", 1, 2), "d'O");
        assert_eq!(letters_around("à l’été", 4, 7), "l’été");
    }

    #[test]
    fn test_no_error() {
        assert!(check(&file(&["l'option", "d'abord", "'name'"])).is_empty());
        assert!(check(&file(&["l’option", "d’abord", "'name'"])).is_empty());
        assert!(check(&file(&["option", "'name' ‘x’"])).is_empty());
    }

    #[test]
    fn test_straight_expected() {
        let diags = check(&file(&[
            "l'option",
            "d'abord",
            "s'il",
            "l’archive et l’image",
        ]));
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Info);
        assert_eq!(
            diags[0].message,
            "inconsistent apostrophe '’' (file: 2 ’ / 3 ')"
        );
        // Tie: the typographic apostrophe is expected.
        let diags = check(&file(&["l'option", "d'abord", "l’archive", "l’image"]));
        assert_eq!(diags.len(), 2);
        assert_eq!(
            diags[0].message,
            "inconsistent apostrophe ''' (file: 2 ’ / 2 ')"
        );
        let diags = check(&file(&["l'option", "d'abord", "l'archive", "l’image"]));
        assert_eq!(diags.len(), 1);
        assert_eq!(edits(&diags[0]), vec![(1..4, "'".to_string())]);
    }

    #[test]
    fn test_typographic_expected() {
        let diags = check(&file(&["l’option", "d’abord", "l'archive et 'x' l'image"]));
        assert_eq!(diags.len(), 1);
        assert_eq!(
            edits(&diags[0]),
            vec![(1..2, "’".to_string()), (18..19, "’".to_string())]
        );
    }

    #[test]
    fn test_plurals_and_noqa() {
        let content = format!(
            "{HEADER}msgid \"a\"\nmsgstr \"l’option\"\n\n\
             msgid \"b\"\nmsgstr \"d’abord\"\n\n\
             msgid \"%d file\"\nmsgid_plural \"%d files\"\n\
             msgstr[0] \"%d fichier d'archive\"\nmsgstr[1] \"%d fichiers d'archive\"\n\n\
             #, noqa:apostrophes\nmsgid \"c\"\nmsgstr \"l'image\"\n"
        );
        let diags = check(&content);
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].lines[2].line_number, 12);
        assert_eq!(diags[1].lines[2].line_number, 13);
    }
}
