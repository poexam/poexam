// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `caps-start` rule: check inconsistent capitalization
//! of the first letter between source and translation.

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Edit, Fix, FixTarget};
use crate::po::entry::Entry;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct CapsStartRule;

/// Characters stopping the search of the first letter: the string starts with
/// a placeholder, a path or code, where the case is not meaningful.
const STOP_CHARS: [char; 8] = ['%', '$', '{', '@', '\\', '/', '`', '~'];

/// Languages where nouns are capitalized: a lowercase source starting with a
/// noun is translated with an uppercase first letter.
const NOUN_CAPS_LANGUAGES: [&str; 2] = ["de", "lb"];

/// Languages written in a script without case: a translation starting with a
/// Latin word (e.g. `IRC サーバ`) keeps the case of the term, whatever the source.
const CASELESS_LANGUAGES: [&str; 26] = [
    "am", "ar", "bn", "fa", "gu", "he", "hi", "ja", "ka", "km", "kn", "ko", "lo", "ml", "mr", "my",
    "ne", "pa", "ps", "si", "ta", "te", "th", "ur", "yi", "zh",
];

/// Case of a letter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Case {
    Upper,
    Lower,
}

/// First letter of a string.
#[derive(Debug, PartialEq, Eq)]
struct FirstLetter<'a> {
    /// Byte position in the string.
    pos: usize,
    ch: char,
    case: Case,
    /// Word starting with the letter.
    word: &'a str,
}

/// Return the first letter of a string, skipping leading whitespace,
/// punctuation and HTML tags.
///
/// Return `None` if the string has no letter, starts with a msgcat conflict
/// marker (reported by the `compendium` rule), a digit, a
/// [stop char](STOP_CHARS), a letter without case (e.g. CJK, Arabic), an
/// acronym (e.g. `URL`) or a word with mixed case (e.g. `iPhone`, `macOS`),
/// whose case is fixed.
fn first_letter(s: &str) -> Option<FirstLetter<'_>> {
    if s.starts_with("#-#-#-#-#") {
        return None;
    }
    let mut pos = 0;
    let ch = loop {
        let c = s[pos..].chars().next()?;
        if c.is_alphanumeric() {
            break c;
        }
        if c == '<' {
            // Skip an HTML tag like `<b>` or `</b>`.
            if !s[pos + 1..].starts_with(|c: char| c.is_ascii_alphabetic() || c == '/') {
                return None;
            }
            pos += memchr::memchr(b'>', &s.as_bytes()[pos..])? + 1;
            continue;
        }
        if STOP_CHARS.contains(&c) {
            return None;
        }
        pos += c.len_utf8();
    };
    let case = if ch.is_uppercase() {
        Case::Upper
    } else if ch.is_lowercase() {
        Case::Lower
    } else {
        return None;
    };
    let word = s[pos..].split(|c: char| !c.is_alphanumeric()).next()?;
    let rest = &word[ch.len_utf8()..];
    if rest.chars().any(char::is_uppercase) {
        // Acronym or mixed case word: the case is part of the word.
        return None;
    }
    Some(FirstLetter {
        pos,
        ch,
        case,
        word,
    })
}

/// Return `true` if `word` is a word of `s` (case-sensitive).
fn contains_word(s: &str, word: &str) -> bool {
    s.split(|c: char| !c.is_alphanumeric()).any(|w| w == word)
}

/// Return the replacement of the first letter of `s` to get the case `case`,
/// with the length of the replaced string, or `None` if the letter can not be
/// converted to a single letter.
///
/// Special cases: Turkish and Azerbaijani dotted and dotless `i`, Dutch `ij`
/// (`IJ` in uppercase) and the Latin digraphs (titlecase `ǅ`, `ǈ`, `ǋ`, `ǲ`).
fn change_case(
    s: &str,
    letter: &FirstLetter,
    case: Case,
    language: &str,
) -> Option<(String, usize)> {
    let ch = letter.ch;
    let len = ch.len_utf8();
    if case == Case::Upper && language == "nl" && s[letter.pos..].starts_with("ij") {
        return Some(("IJ".to_string(), 2));
    }
    let special = match (case, ch, language) {
        (Case::Upper, 'i', "tr" | "az") => Some('İ'),
        (Case::Lower, 'I', "tr" | "az") => Some('ı'),
        (Case::Lower, 'İ', _) => Some('i'),
        (Case::Upper, 'ǆ' | 'Ǆ', _) => Some('ǅ'),
        (Case::Upper, 'ǉ' | 'Ǉ', _) => Some('ǈ'),
        (Case::Upper, 'ǌ' | 'Ǌ', _) => Some('ǋ'),
        (Case::Upper, 'ǳ' | 'Ǳ', _) => Some('ǲ'),
        _ => None,
    };
    if let Some(c) = special {
        return Some((c.to_string(), len));
    }
    let converted: String = match case {
        Case::Upper => ch.to_uppercase().collect(),
        Case::Lower => ch.to_lowercase().collect(),
    };
    (converted.chars().count() == 1).then_some((converted, len))
}

impl RuleChecker for CapsStartRule {
    fn name(&self) -> &'static str {
        "caps-start"
    }

    fn description(&self) -> &'static str {
        "Check for inconsistent capitalization of the first letter between source and translation."
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for inconsistent capitalization of the first letter between
    /// source and translation.
    ///
    /// Leading whitespace, punctuation and HTML tags are skipped. The check is
    /// not done when the source or translation starts with a digit, a
    /// placeholder, a path or code (`%`, `$`, `{`, `@`, `\`, `/`, `` ` ``,
    /// `~`), a letter without case (e.g. CJK, Arabic, Hebrew), an acronym
    /// (e.g. `URL`) or a word with mixed case (e.g. `iPhone`, `macOS`).
    ///
    /// Languages written in a script without case (e.g. Japanese, Arabic) are
    /// not checked: a translation starting with a Latin word keeps the case of
    /// this term.
    ///
    /// A translation starting with a word found as-is (same case) in the source
    /// is a term moved by the word order (e.g. `No xfer` / `xfer yok`): it is
    /// not reported.
    ///
    /// In German and Luxembourgish, where nouns are capitalized, a lowercase
    /// source translated with an uppercase first letter is not reported.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "Delete the file"
    /// msgstr "supprimer le fichier"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "Delete the file"
    /// msgstr "Supprimer le fichier"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`info`](Severity::Info): `inconsistent capitalization of first letter ('…' / '…')`
    ///   (auto-fixable: safe to uppercase, unsafe to lowercase as the translation
    ///   may start with a proper noun)
    fn check_msg(
        &self,
        checker: &Checker,
        _entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let (Some(id_letter), Some(str_letter)) =
            (first_letter(&msgid.value), first_letter(&msgstr.value))
        else {
            return vec![];
        };
        let language = checker.language_code();
        if CASELESS_LANGUAGES.contains(&language) {
            return vec![];
        }
        if id_letter.case == str_letter.case
            || (id_letter.case == Case::Lower && NOUN_CAPS_LANGUAGES.contains(&language))
            || contains_word(
                &msgid.value[id_letter.pos + id_letter.word.len()..],
                str_letter.word,
            )
        {
            // Same case, or the translation starts with a term of the source
            // kept as-is, moved by the word order (e.g. "No xfer" / "xfer yok").
            return vec![];
        }
        let fix = change_case(&msgstr.value, &str_letter, id_letter.case, language).map(
            |(replacement, len)| Fix {
                target: FixTarget::Msgstr {
                    file_byte_range: msgstr.byte_range.clone(),
                },
                edits: vec![Edit {
                    range: str_letter.pos..str_letter.pos + len,
                    replacement,
                }],
                safe: id_letter.case == Case::Upper,
            },
        );
        self.new_diag(
            checker,
            Severity::Info,
            format!(
                "inconsistent capitalization of first letter ('{}' / '{}')",
                id_letter.ch, str_letter.ch
            ),
        )
        .map(|d| {
            d.with_msgs_hl(
                msgid,
                [(id_letter.pos, id_letter.pos + id_letter.ch.len_utf8())],
                msgstr,
                [(str_letter.pos, str_letter.pos + str_letter.ch.len_utf8())],
            )
            .with_optional_fix(fix)
        })
        .into_iter()
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{fix::apply_msgstr_fixes, rules::rule::Rules};

    fn check(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(CapsStartRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    /// Check one entry in a file with the given language.
    fn check_entry(language: &str, msgid: &str, msgstr: &str) -> Vec<Diagnostic> {
        check(&format!(
            "msgid \"\"\nmsgstr \"Language: {language}\\n\"\n\nmsgid \"{msgid}\"\nmsgstr \"{msgstr}\"\n"
        ))
    }

    /// Return the fixed translation and whether the fix is safe.
    fn fixed(language: &str, msgid: &str, msgstr: &str) -> Option<(String, bool)> {
        let diags = check_entry(language, msgid, msgstr);
        assert_eq!(diags.len(), 1, "expected one diagnostic: {diags:?}");
        let fix = diags[0].fix.as_ref()?;
        Some((apply_msgstr_fixes(msgstr, &fix.edits).unwrap(), fix.safe))
    }

    #[test]
    fn test_first_letter() {
        let letter = |s| first_letter(s).map(|l| (l.pos, l.ch, l.case));
        assert_eq!(first_letter("hook_url: %s").map(|l| l.word), Some("hook"));
        assert_eq!(letter("Hello"), Some((0, 'H', Case::Upper)));
        assert_eq!(letter("hello"), Some((0, 'h', Case::Lower)));
        assert_eq!(letter("  ¿Qué?"), Some((4, 'Q', Case::Upper)));
        assert_eq!(letter("« élan »"), Some((3, 'é', Case::Lower)));
        assert_eq!(letter("&Save"), Some((1, 'S', Case::Upper)));
        assert_eq!(letter("_open"), Some((1, 'o', Case::Lower)));
        assert_eq!(letter("<b>Warning</b>"), Some((3, 'W', Case::Upper)));
        assert_eq!(letter("</i><b>warning"), Some((7, 'w', Case::Lower)));
        assert_eq!(letter("...and more"), Some((3, 'a', Case::Lower)));
        assert_eq!(letter("URL to open"), None);
        assert_eq!(letter("A file"), Some((0, 'A', Case::Upper)));
        assert_eq!(letter("l'URL"), Some((0, 'l', Case::Lower)));
        assert_eq!(letter("Ölçek"), Some((0, 'Ö', Case::Upper)));
        assert_eq!(letter(""), None);
        assert_eq!(letter("..."), None);
        assert_eq!(letter("3 files"), None);
        assert_eq!(letter("%s files"), None);
        assert_eq!(letter("{name} logged in"), None);
        assert_eq!(letter("$HOME is"), None);
        assert_eq!(letter("/usr/bin"), None);
        assert_eq!(letter("`code`"), None);
        assert_eq!(letter("< 5"), None);
        assert_eq!(letter("<b unclosed"), None);
        assert_eq!(letter("iPhone"), None);
        assert_eq!(letter("macOS"), None);
        assert_eq!(letter("McDonald"), None);
        assert_eq!(letter("ファイル"), None);
        assert_eq!(letter("ملف"), None);
        assert_eq!(letter("#-#-#-#-#  fr.po (app)  #-#-#-#-#\nPremière"), None);
    }

    #[test]
    fn test_no_error() {
        assert!(check_entry("fr", "Delete the file", "Supprimer le fichier").is_empty());
        assert!(check_entry("fr", "delete the file", "supprimer le fichier").is_empty());
        assert!(check_entry("fr", "<b>Warning</b>", "<b>Attention</b>").is_empty());
        assert!(check_entry("fr", "%s files", "Fichiers : %s").is_empty());
        assert!(check_entry("fr", "Open the iPhone app", "iPhone : ouvrir l'app").is_empty());
        assert!(check_entry("ja", "Delete the file", "ファイルを削除").is_empty());
        assert!(
            check_entry(
                "ja",
                "send a CTCP action",
                "CTCP action をリモートホストに送信"
            )
            .is_empty()
        );
        assert!(check_entry("ja", "Sending hsignal", "hsignal 送信中").is_empty());
        assert!(check_entry("fr", "RGB color", "couleur RGB").is_empty());
        assert!(check_entry("tr", "display info", "KATIL iletilerinde").is_empty());
        // Term moved by the word order.
        assert!(check_entry("tr", "No xfer", "xfer yok").is_empty());
        assert!(
            check_entry("tr", "Debug hook_url: %s", "hook_url hatalarını ayıkla: %s").is_empty()
        );
        assert!(check_entry("sr", "The \\\"irc\\\" protocol", "„irc” протокол").is_empty());
        // Source term moved in the translation: the translation must still
        // start with the same case.
        assert_eq!(check_entry("cs", "Xfer list", "seznam Xfer").len(), 1);
        assert_eq!(check_entry("fr", "lua scripts", "Scripts lua").len(), 1);
    }

    #[test]
    fn test_german_nouns() {
        // Nouns are capitalized in German: lowercase source, uppercase translation is ok.
        assert!(check_entry("de", "file not found", "Datei nicht gefunden").is_empty());
        assert!(check_entry("de_AT", "file not found", "Datei nicht gefunden").is_empty());
        // Uppercase source, lowercase translation is still reported.
        assert_eq!(
            check_entry("de", "Delete the file", "datei löschen").len(),
            1
        );
    }

    #[test]
    fn test_error() {
        let diags = check_entry("fr", "Delete the file", "supprimer le fichier");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Info);
        assert_eq!(
            diags[0].message,
            "inconsistent capitalization of first letter ('D' / 's')"
        );
        let diags = check_entry("fr", "file not found", "Fichier introuvable");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "inconsistent capitalization of first letter ('f' / 'F')"
        );
    }

    #[test]
    fn test_error_noqa() {
        let diags = check(
            "msgid \"\"\nmsgstr \"Language: fr\\n\"\n\n#, noqa:caps-start\n\
             msgid \"Delete the file\"\nmsgstr \"supprimer le fichier\"\n",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_fix() {
        // Uppercase: safe.
        assert_eq!(
            fixed("fr", "Delete the file", "supprimer le fichier"),
            Some(("Supprimer le fichier".to_string(), true))
        );
        assert_eq!(
            fixed("fr", "Warning!", "<b>« attention ! »</b>"),
            Some(("<b>« Attention ! »</b>".to_string(), true))
        );
        assert_eq!(
            fixed("fr", "Edit", "éditer"),
            Some(("Éditer".to_string(), true))
        );
        // Lowercase: unsafe (the translation may start with a proper noun).
        assert_eq!(
            fixed("fr", "file not found", "Fichier introuvable"),
            Some(("fichier introuvable".to_string(), false))
        );
    }

    #[test]
    fn test_fix_special_cases() {
        // Turkish and Azerbaijani dotted and dotless i.
        assert_eq!(
            fixed("tr", "Install", "indir"),
            Some(("İndir".to_string(), true))
        );
        assert_eq!(
            fixed("tr", "install", "Işık"),
            Some(("ışık".to_string(), false))
        );
        assert_eq!(
            fixed("tr", "install", "İndir"),
            Some(("indir".to_string(), false))
        );
        assert_eq!(
            fixed("fr", "Install", "installer"),
            Some(("Installer".to_string(), true))
        );
        // Dutch "ij" is capitalized as "IJ".
        assert_eq!(
            fixed("nl", "Ice cream", "ijsje"),
            Some(("IJsje".to_string(), true))
        );
        // Latin digraphs use the titlecase form.
        assert_eq!(
            fixed("hr", "Bridge", "ǆamija"),
            Some(("ǅamija".to_string(), true))
        );
        // No single uppercase letter for "ß".
        assert_eq!(fixed("de", "Street", "ßtraße"), None);
    }
}
