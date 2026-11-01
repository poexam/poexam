// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `reused-translation` rule: check for translations
//! used for different source strings.

use std::borrow::Cow;
use std::collections::hash_map::Entry as MapEntry;
use std::collections::{HashMap, HashSet};

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

pub struct ReusedTranslationRule;

/// Minimum number of words of the sources to compare: short sources are
/// often synonyms with the same translation (e.g. `Delete` and `Remove`).
const MIN_WORDS: usize = 3;

/// Return the words of a source: lowercase alphanumeric sequences, ignoring
/// the keyboard accelerators `_` and `&` (e.g. `Sa_ve as` gives `save`, `as`).
fn words(s: &str) -> HashSet<String> {
    s.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '&')
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect::<String>()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// Return `true` if the sources (as sets of words) are different: both have
/// at least [`MIN_WORDS`] words and they share less than a third of their
/// words, so that rephrased sources with the same meaning (e.g. `Passwords do
/// not match` and `Passwords don't match`) are not considered different.
fn different_sources(words1: &HashSet<String>, words2: &HashSet<String>) -> bool {
    if words1.len() < MIN_WORDS || words2.len() < MIN_WORDS {
        return false;
    }
    let common = words1.intersection(words2).count();
    let union = words1.len() + words2.len() - common;
    common * 3 < union
}

/// Return the translation of an entry (all plural forms joined with a NUL
/// char), or `None` if the entry is not fully translated or the translation
/// has no letter (e.g. `%s: %s`).
fn translation(entry: &Entry) -> Option<Cow<'_, str>> {
    let mut strs = entry.msgstr.values().map(|m| m.value.as_str());
    let first = strs.next()?;
    if first.is_empty() || !first.chars().any(char::is_alphabetic) {
        return None;
    }
    if entry.msgstr.len() == 1 {
        return Some(Cow::Borrowed(first));
    }
    let mut result = first.to_string();
    for value in strs {
        if value.is_empty() {
            return None;
        }
        result.push('\0');
        result.push_str(value);
    }
    Some(Cow::Owned(result))
}

/// Entries using the same translation.
struct Group<'a> {
    /// First entry with this translation.
    first: &'a Entry,
    /// Words of the sources of the entries with this translation, computed
    /// only when the translation is used a second time.
    sources: Vec<HashSet<String>>,
}

impl RuleChecker for ReusedTranslationRule {
    fn name(&self) -> &'static str {
        "reused-translation"
    }

    fn description(&self) -> &'static str {
        "Check for translations used for different source strings."
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

    /// Check for translations used for different source strings, which may
    /// be a copy-paste error, or a translation not updated after a change of
    /// the source.
    ///
    /// Only sources with at least 3 words sharing less than a third of their
    /// words are considered different: short sources are often synonyms
    /// (e.g. `Delete` and `Remove`), and close sources are rephrased strings
    /// with the same meaning (e.g. `Passwords do not match` and `Passwords
    /// don't match`). Keyboard accelerators (`_` and `&`) are ignored.
    /// Translations without letters (e.g. `%s: %s`) are not checked.
    ///
    /// An entry is reported if its source is different from the sources of
    /// all the previous entries with the same translation, with the source of
    /// the first one.
    ///
    /// Wrong entries:
    /// ```text
    /// msgid "Open the selected file"
    /// msgstr "Ouvrir le fichier sélectionné"
    ///
    /// msgid "Close the current window"
    /// msgstr "Ouvrir le fichier sélectionné"
    /// ```
    ///
    /// Correct entries:
    /// ```text
    /// msgid "Open the selected file"
    /// msgstr "Ouvrir le fichier sélectionné"
    ///
    /// msgid "Close the current window"
    /// msgstr "Fermer la fenêtre courante"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`info`](Severity::Info): `same translation as for source '…' (line #)`
    fn check_entries(&self, checker: &Checker, entries: &[&Entry]) -> Vec<Diagnostic> {
        let mut diags = vec![];
        let mut groups: HashMap<Cow<str>, Group> = HashMap::new();
        for entry in entries {
            let (Some(msgid), Some(msgstr), Some(str_key)) =
                (&entry.msgid, entry.msgstr.get(&0), translation(entry))
            else {
                continue;
            };
            let group = match groups.entry(str_key) {
                MapEntry::Vacant(vacant) => {
                    vacant.insert(Group {
                        first: entry,
                        sources: vec![],
                    });
                    continue;
                }
                MapEntry::Occupied(occupied) => occupied.into_mut(),
            };
            let Some(first_msgid) = &group.first.msgid else {
                continue;
            };
            if group.sources.is_empty() {
                group.sources.push(words(&first_msgid.value));
            }
            let source = words(&msgid.value);
            let different = group.sources.iter().all(|s| different_sources(s, &source));
            group.sources.push(source);
            if !different {
                continue;
            }
            if let Some(diag) = self.new_diag(
                checker,
                Severity::Info,
                format!(
                    "same translation as for source '{}' (line {})",
                    first_msgid.value, first_msgid.line_number
                ),
            ) {
                diags.push(diag.with_msgs(msgid, msgstr));
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

    fn check(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(ReusedTranslationRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    /// Check a file with the given entries (msgid, msgstr).
    fn check_entries(entries: &[(&str, &str)]) -> Vec<Diagnostic> {
        let mut content = String::from("msgid \"\"\nmsgstr \"Language: fr\\n\"\n");
        for (msgid, msgstr) in entries {
            write!(content, "\nmsgid \"{msgid}\"\nmsgstr \"{msgstr}\"\n").unwrap();
        }
        check(&content)
    }

    #[test]
    fn test_words() {
        let w = |s| {
            let mut v: Vec<String> = words(s).into_iter().collect();
            v.sort();
            v
        };
        assert!(w("").is_empty());
        assert_eq!(w("Sa_ve as..."), ["as", "save"]);
        assert_eq!(w("Save & Exit"), ["exit", "save"]);
        assert_eq!(w("&Open the FILE, open it"), ["file", "it", "open", "the"]);
    }

    #[test]
    fn test_different_sources() {
        let diff = |a, b| different_sources(&words(a), &words(b));
        // Short sources.
        assert!(!diff("Delete", "Remove"));
        assert!(!diff("Delete file", "Remove item"));
        // Close sources.
        assert!(!diff("Passwords do not match", "Passwords don't match"));
        assert!(!diff(
            "Delete selected accounts",
            "Remove selected accounts"
        ));
        // Different sources.
        assert!(diff("Open the selected file", "Close the current window"));
        assert!(diff("This contact is a bot", "List of all contacts"));
    }

    #[test]
    fn test_no_error() {
        assert!(
            check_entries(&[
                ("Open the selected file", "Ouvrir le fichier sélectionné"),
                ("Close the current window", "Fermer la fenêtre courante"),
                ("Delete", "Supprimer"),
                ("Remove", "Supprimer"),
                ("%s: %s (%d)", "%s : %s (%d)"),
                ("%s: %s, %d", "%s : %s (%d)"),
                (
                    "Passwords do not match",
                    "Les mots de passe sont différents"
                ),
                ("Passwords don't match", "Les mots de passe sont différents"),
            ])
            .is_empty()
        );
    }

    #[test]
    fn test_error() {
        let diags = check_entries(&[
            ("Open the selected file", "Ouvrir le fichier sélectionné"),
            ("Close the current window", "Ouvrir le fichier sélectionné"),
            (
                "Open the selected file now",
                "Ouvrir le fichier sélectionné",
            ),
            ("Quit the application", "Ouvrir le fichier sélectionné"),
        ]);
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].severity, Severity::Info);
        assert_eq!(
            diags[0].message,
            "same translation as for source 'Open the selected file' (line 4)"
        );
        assert_eq!(diags[0].lines[0].line_number, 7);
        assert_eq!(diags[1].lines[0].line_number, 13);
    }

    #[test]
    fn test_plurals() {
        let diags = check(
            "msgid \"\"\nmsgstr \"Language: fr\\n\"\n\n\
             msgid \"%d file was deleted\"\nmsgid_plural \"%d files were deleted\"\n\
             msgstr[0] \"%d fichier supprimé\"\nmsgstr[1] \"%d fichiers supprimés\"\n\n\
             msgid \"%d window is open now\"\nmsgid_plural \"%d windows are open now\"\n\
             msgstr[0] \"%d fichier supprimé\"\nmsgstr[1] \"%d fichiers supprimés\"\n\n\
             msgid \"%d window is still open\"\nmsgid_plural \"%d windows are still open\"\n\
             msgstr[0] \"%d fichier supprimé\"\nmsgstr[1] \"%d fichiers\"\n",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].lines[0].line_number, 9);
    }

    #[test]
    fn test_noqa_fuzzy() {
        let content = "msgid \"\"\nmsgstr \"Language: fr\\n\"\n\n\
                       msgid \"Open the selected file\"\nmsgstr \"Ouvrir le fichier\"\n\n\
                       #, noqa:reused-translation\n\
                       msgid \"Close the current window\"\nmsgstr \"Ouvrir le fichier\"\n\n\
                       #, fuzzy\n\
                       msgid \"Quit the application\"\nmsgstr \"Ouvrir le fichier\"\n";
        assert!(check(content).is_empty());
    }
}
