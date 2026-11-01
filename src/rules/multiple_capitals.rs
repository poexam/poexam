// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `multiple-capitals` rule: check for words starting
//! with misplaced capital letters in the translation (e.g. `HEllo`).

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Edit, Fix, FixTarget};
use crate::po::entry::Entry;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct MultipleCapitalsRule;

/// Word with misplaced capital letters.
#[derive(Debug, PartialEq, Eq)]
struct Misplaced<'a> {
    /// Byte position of the word in the string.
    pos: usize,
    word: &'a str,
    /// Byte position of the second (misplaced) capital letter in the word.
    second: usize,
}

/// Languages where the stem of a noun following its class prefix is
/// capitalized (Nguni languages, e.g. Xhosa `IMaphu`).
const PREFIX_CAPS_LANGUAGES: [&str; 4] = ["nr", "ss", "xh", "zu"];

/// Return the words of `s` starting with exactly two capital letters followed
/// by a lowercase letter (e.g. `HEllo`): the Shift key released too late.
///
/// Are not returned:
/// - words with other capitals (e.g. `YCbCr`, `JChemPaint`)
/// - words with digits (e.g. `IPv6`)
/// - plural acronyms (e.g. `IDs`)
/// - words following `%` or `\` (format strings, escapes)
/// - terms of the source `source`, as-is (e.g. `GLib`) or inflected
///   (e.g. `IDer` for `IDs`, `GFlarelle` for `GFlare`), see [`is_source_term`]
/// - in Dutch, words starting with the digraph `IJ` (e.g. `IJzer`).
fn misplaced_capitals<'a>(s: &'a str, source: &str, language: &str) -> Vec<Misplaced<'a>> {
    let mut words = vec![];
    if PREFIX_CAPS_LANGUAGES.contains(&language) {
        return words;
    }
    for (pos, word) in split_words(s) {
        if matches!(s[..pos].chars().next_back(), Some('%' | '\\')) {
            continue;
        }
        let mut chars = word.char_indices();
        let (Some((_, c1)), Some((second, c2)), Some((_, c3))) =
            (chars.next(), chars.next(), chars.next())
        else {
            continue;
        };
        if !c1.is_uppercase()
            || !c2.is_uppercase()
            || !c3.is_lowercase()
            || &word[second + c2.len_utf8()..] == "s"
            || word.chars().any(char::is_numeric)
            || word[second + c2.len_utf8()..]
                .chars()
                .any(char::is_uppercase)
            || (language == "nl" && c1 == 'I' && c2 == 'J')
            || is_source_term(source, word, &word[..second + c2.len_utf8()])
        {
            continue;
        }
        words.push(Misplaced { pos, word, second });
    }
    words
}

/// Split a string into words (sequences of alphanumeric chars), returning
/// each word with its byte position.
fn split_words(s: &str) -> impl Iterator<Item = (usize, &str)> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(move |w| (w.as_ptr() as usize - s.as_ptr() as usize, w))
}

/// Return `true` if the word `word` of the translation, starting with the
/// capitals `caps`, is a term of the source `source`: a word of the source is
/// the same, or starts with the same capitals (inflected term, e.g. `IDer`
/// for `IDs`).
///
/// A word of the source differing only by case is a term too (e.g. `GLib` for
/// `glib`, `GFig` for `Gfig`), unless it is an acronym (e.g. `URl` for `URL`):
/// this is a typo, so `false` is returned.
fn is_source_term(source: &str, word: &str, caps: &str) -> bool {
    let lower = |s: &str| s.chars().flat_map(char::to_lowercase).collect::<String>();
    let mut term = false;
    for (_, w) in split_words(source) {
        if w == word {
            return true;
        }
        if w.len() == word.len() && lower(w) == lower(word) {
            return !w.chars().all(char::is_uppercase);
        }
        term |= w.starts_with(caps);
    }
    term
}

/// Return the lowercase of the capital letter `c`, or `None` if it can not
/// be converted to a single letter.
///
/// In Turkish and Azerbaijani, `I` is the uppercase of the dotless `ı` and
/// `İ` the one of `i`.
fn to_lowercase(c: char, language: &str) -> Option<char> {
    if matches!(language, "tr" | "az") {
        match c {
            'I' => return Some('ı'),
            'İ' => return Some('i'),
            _ => {}
        }
    }
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(l), None) => Some(l),
        _ => None,
    }
}

impl RuleChecker for MultipleCapitalsRule {
    fn name(&self) -> &'static str {
        "multiple-capitals"
    }

    fn description(&self) -> &'static str {
        "Check for words starting with misplaced capital letters in the translation."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for words starting with misplaced capital letters in the
    /// translation, like `HEllo`: two capital letters followed by a lowercase
    /// letter, the Shift key being released too late.
    ///
    /// Are not reported: acronyms (e.g. `PDF`), even inflected (e.g. Polish
    /// `IRCu`), words with other capitals (e.g. `YCbCr`) or digits (e.g.
    /// `IPv6`), plural acronyms (e.g. `IDs`), format strings, escapes, terms of
    /// the source, as-is or inflected (e.g. `GLib`, `IDer` for `IDs`), Dutch
    /// words starting with `IJ` (e.g. `IJzer`), Nguni languages (e.g. Xhosa
    /// `IMaphu`) and the translator credits. A term of the source with
    /// another case is reported only if it is an acronym (e.g. `URl` for
    /// `URL`).
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "Click on the lock to make changes."
    /// msgstr "CLiquez sur le cadenas pour faire des changements."
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "Click on the lock to make changes."
    /// msgstr "Cliquez sur le cadenas pour faire des changements."
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`info`](Severity::Info): `misplaced capital letter in word '…'`
    ///   (auto-fixable, unsafe: the word may be a name, like `GFig`)
    fn check_msg(
        &self,
        checker: &Checker,
        _entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        if msgid.value == "translator-credits" {
            return vec![];
        }
        let language = checker.language_code();
        misplaced_capitals(&msgstr.value, &msgid.value, language)
            .into_iter()
            .filter_map(|word| {
                let pos = word.pos + word.second;
                let c = word.word[word.second..].chars().next()?;
                let fix = to_lowercase(c, language).map(|lower| Fix {
                    target: FixTarget::Msgstr {
                        file_byte_range: msgstr.byte_range.clone(),
                    },
                    edits: vec![Edit {
                        range: pos..pos + c.len_utf8(),
                        replacement: lower.to_string(),
                    }],
                    safe: false,
                });
                self.new_diag(
                    checker,
                    Severity::Info,
                    format!("misplaced capital letter in word '{}'", word.word),
                )
                .map(|d| {
                    d.with_msgs_hl(msgid, [], msgstr, [(pos, pos + c.len_utf8())])
                        .with_optional_fix(fix)
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{fix::apply_msgstr_fixes, rules::rule::Rules};

    fn check(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(MultipleCapitalsRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    /// Check one entry in a file with the given language.
    fn check_entry(language: &str, msgid: &str, msgstr: &str) -> Vec<Diagnostic> {
        check(&format!(
            "msgid \"\"\nmsgstr \"Language: {language}\\n\"\n\nmsgid \"{msgid}\"\nmsgstr \"{msgstr}\"\n"
        ))
    }

    /// Return the words with misplaced capitals.
    fn words<'a>(s: &'a str, source: &str, language: &str) -> Vec<&'a str> {
        misplaced_capitals(s, source, language)
            .iter()
            .map(|w| w.word)
            .collect()
    }

    #[test]
    fn test_misplaced_capitals() {
        assert!(words("", "", "fr").is_empty());
        assert!(words("Hello world", "", "fr").is_empty());
        assert_eq!(words("HEllo wOrld", "", "fr"), ["HEllo"]);
        assert_eq!(words("CLick, then EStado", "", "es"), ["CLick", "EStado"]);
        assert_eq!(words("ПДф файл", "", "sr"), ["ПДф"]);
        let w = misplaced_capitals("Le FIchier", "", "fr");
        assert_eq!(
            w,
            [Misplaced {
                pos: 3,
                word: "FIchier",
                second: 1
            }]
        );
        // Acronyms and words with other capitals or digits.
        assert!(words("PDF, IRCu, ФАЙЛа, YCbCr, JChemPaint, IPv6", "", "pl").is_empty());
        // Plural acronym.
        assert!(words("Les IDs", "", "fr").is_empty());
        // Format strings and escapes.
        assert!(words("%LSc \\NAme", "", "fr").is_empty());
        // Terms of the source, as-is, inflected or with another case.
        assert!(words("Versione GLib", "GLib version", "it").is_empty());
        assert!(words("lag-IDer", "layer IDs", "nb").is_empty());
        assert!(words("GFlarelle", "the new GFlare", "fi").is_empty());
        assert!(words("CMake", "with cmake", "de").is_empty());
        assert!(words("GFig", "First Gfig", "de").is_empty());
        assert_eq!(words("URl kopieren", "Copy URL", "de"), ["URl"]);
        // Dutch digraph IJ.
        assert!(words("IJzer", "", "nl").is_empty());
        assert_eq!(words("IJzer", "", "fr"), ["IJzer"]);
        // Nguni languages.
        assert!(words("IMaphu", "", "xh").is_empty());
    }

    #[test]
    fn test_no_error() {
        assert!(check_entry("fr", "Click here", "Cliquez ici").is_empty());
        assert!(check_entry("fr", "translator-credits", "JOker, LSc").is_empty());
        let diags = check("#, noqa:multiple-capitals\nmsgid \"Click\"\nmsgstr \"CLiquez\"\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn test_error() {
        let diags = check_entry("fr", "Click here, then here", "CLiquez ici, puis ICi");
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].severity, Severity::Info);
        assert_eq!(
            diags[0].message,
            "misplaced capital letter in word 'CLiquez'"
        );
        assert_eq!(diags[1].message, "misplaced capital letter in word 'ICi'");
    }

    #[test]
    fn test_fix() {
        let diags = check_entry("de", "The key", "Der SChlüssel");
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().unwrap();
        assert!(!fix.safe);
        assert_eq!(
            apply_msgstr_fixes("Der SChlüssel", &fix.edits).unwrap(),
            "Der Schlüssel"
        );
        let diags = check_entry("tr", "File", "DIosya");
        let fix = diags[0].fix.as_ref().unwrap();
        assert_eq!(apply_msgstr_fixes("DIosya", &fix.edits).unwrap(), "Dıosya");
    }
}
