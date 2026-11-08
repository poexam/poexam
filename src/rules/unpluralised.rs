// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `unpluralised` rule: check for sources with a count
//! and a plural word, without plural forms.

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::po::format::{iter::FormatPos, language::Language};
use crate::rules::rule::RuleChecker;

pub struct UnpluralisedRule;

/// Words ending with `s` which are not plurals.
const NOT_PLURALS: [&str; 31] = [
    "always",
    "as",
    "afterwards",
    "besides",
    "does",
    "goes",
    "has",
    "hers",
    "his",
    "its",
    "nowadays",
    "ours",
    "perhaps",
    "sometimes",
    "theirs",
    "this",
    "towards",
    "vers",
    "was",
    "whereas",
    "yes",
    "yours",
    // Units.
    "bps",
    "fps",
    "gbps",
    "kbps",
    "mbps",
    "ms",
    "ns",
    "ps",
    "μs",
];

/// Irregular plurals (not ending with `s`).
const IRREGULAR_PLURALS: [&str; 10] = [
    "children",
    "criteria",
    "feet",
    "geese",
    "men",
    "mice",
    "people",
    "phenomena",
    "teeth",
    "women",
];

/// Adjectives allowed between the count and the plural word (e.g. `%d new
/// messages`).
const ADJECTIVES: [&str; 30] = [
    "active",
    "additional",
    "available",
    "changed",
    "deleted",
    "different",
    "duplicate",
    "duplicated",
    "empty",
    "existing",
    "extra",
    "failed",
    "free",
    "hidden",
    "invalid",
    "matching",
    "missing",
    "modified",
    "more",
    "new",
    "old",
    "open",
    "other",
    "pending",
    "remaining",
    "selected",
    "unique",
    "unknown",
    "unread",
    "unused",
];

/// Names of brace placeholders (e.g. `{count}`) holding a count.
const COUNT_NAMES: [&str; 8] = [
    "amount", "count", "n", "nb", "num", "number", "qty", "total",
];

/// Return `true` if the format string (e.g. `%d`, `%(count)d`, `{count}`)
/// is a count: an integer conversion for printf-like formats, a placeholder
/// named like a count for the other ones (e.g. `{count}`, `{num_files}`).
fn is_count(fmt: &str, language: Language) -> bool {
    match language {
        Language::C
        | Language::JavaScript
        | Language::Lua
        | Language::ObjectPascal
        | Language::Perl
        | Language::Php
        | Language::Python
        | Language::Ruby => {
            fmt.ends_with(['d', 'i', 'u'])
                || fmt.contains("PRId")
                || fmt.contains("PRIi")
                || fmt.contains("PRIu")
        }
        Language::CSharp | Language::Java | Language::PerlBrace | Language::PythonBrace => {
            let name = fmt
                .trim_start_matches('{')
                .split(['}', ':', ',', '!', '.', '['])
                .next()
                .unwrap_or_default()
                .to_lowercase();
            name.split('_')
                .any(|part| COUNT_NAMES.contains(&part) || part.starts_with("num"))
                || name.ends_with("count")
        }
        Language::Null | Language::Qt | Language::Sh => false,
    }
}

/// Return `true` if the number starting at `pos` in `s` is part of an
/// identifier: preceded by a letter, a digit, `#`, `-`, `.` or `_` (e.g.
/// `#%d`, `pre-%d`, `v%d`).
fn is_identifier(s: &str, pos: usize) -> bool {
    s[..pos]
        .chars()
        .next_back()
        .is_some_and(|c| c.is_alphanumeric() || matches!(c, '#' | '-' | '.' | '_'))
}

/// Return `true` if the language id (e.g. `en_US`) is English.
fn is_english(lang_id: &str) -> bool {
    lang_id.split(['_', '-', '.', '@']).next() == Some("en")
}

/// Return `true` if the word is a plural (English).
fn is_plural(word: &str) -> bool {
    let lower = word.to_lowercase();
    if IRREGULAR_PLURALS.contains(&lower.as_str()) {
        return true;
    }
    lower.len() >= 3
        && lower.ends_with('s')
        && !lower.ends_with("ss")
        && !lower.ends_with("us")
        && !lower.ends_with("is")
        && !NOT_PLURALS.contains(&lower.as_str())
}

/// Return the word starting at `pos` in `s` (letters only), or `None` if
/// there is no letter at this position.
fn word_at(s: &str, pos: usize) -> Option<&str> {
    let len = s[pos..]
        .find(|c: char| !c.is_alphabetic())
        .unwrap_or(s.len() - pos);
    (len > 0).then(|| &s[pos..pos + len])
}

/// Return the byte range of the plural word following the count ending at
/// `pos` in `s`, separated by spaces, with an optional adjective in between
/// (e.g. `%d files`, `%d new messages`).
fn plural_after(s: &str, pos: usize) -> Option<(usize, usize)> {
    let mut start = pos;
    for i in 0..2 {
        let spaces = s[start..]
            .find(|c: char| c != ' ' && c != '\u{a0}')
            .unwrap_or(s.len() - start);
        if spaces == 0 {
            return None;
        }
        start += spaces;
        let word = word_at(s, start)?;
        if is_plural(word) {
            return Some((start, start + word.len()));
        }
        if i > 0 || !ADJECTIVES.contains(&word.to_lowercase().as_str()) {
            return None;
        }
        start += word.len();
    }
    None
}

/// Return the byte range of a word with a plural suffix in parentheses
/// (e.g. `file(s)`, `match(es)`).
///
/// A suffix followed by `=` or a letter is not a plural (e.g. key binding
/// `alt+enter(s)=set`).
fn parenthesized_plural(s: &str) -> Option<(usize, usize)> {
    for suffix in ["(s)", "(es)"] {
        let mut from = 0;
        while let Some(pos) = s[from..].find(suffix).map(|p| from + p) {
            let start = s[..pos]
                .rfind(|c: char| !c.is_alphabetic())
                .map_or(0, |p| p + 1);
            let end = pos + suffix.len();
            if start < pos && !s[end..].starts_with(|c: char| c == '=' || c.is_alphabetic()) {
                return Some((start, end));
            }
            from = pos + suffix.len();
        }
    }
    None
}

impl RuleChecker for UnpluralisedRule {
    fn name(&self) -> &'static str {
        "unpluralised"
    }

    fn description(&self) -> &'static str {
        "Check for sources with a count and a plural word, without plural forms."
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for sources with a count and a plural word, without plural
    /// forms: the program should use `ngettext`, so that the translation can
    /// vary with the count.
    ///
    /// The count is an integer format string (e.g. `%d`, `%(count)d`) or a
    /// placeholder named like a count (e.g. `{count}`, `{num_files}`), so only
    /// entries with a format flag (e.g. `c-format`) are checked. Qt and shell
    /// formats are not checked: their placeholders have no type or name.
    ///
    /// The plural word is either a word with a plural suffix in parentheses
    /// (e.g. `file(s)`), in any language, or a plural word following the
    /// count, optionally with an adjective in between (e.g. `%d files`, `%d
    /// new messages`), only if the language of the sources (option
    /// `--spelling-lang-id`, `en_US` by default) is English. A count followed
    /// by another char than a space (e.g. `%d%%`, `%dx%d`, `%d-bit`) or
    /// preceded by a word (e.g. `line %d`, `Files: %d`) is not reported.
    ///
    /// This rule checks the source only: the issue must be fixed in the
    /// program.
    ///
    /// Wrong entry:
    /// ```text
    /// #, c-format
    /// msgid "%d files deleted"
    /// msgstr "%d fichiers supprimés"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// #, c-format
    /// msgid "%d file deleted"
    /// msgid_plural "%d files deleted"
    /// msgstr[0] "%d fichier supprimé"
    /// msgstr[1] "%d fichiers supprimés"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`info`](Severity::Info): `count '…' with plural word '…' without plural forms`
    fn check_entry(&self, checker: &Checker, entry: &Entry) -> Vec<Diagnostic> {
        if entry.msgid_plural.is_some() || entry.format_language == Language::Null {
            return vec![];
        }
        let Some(msgid) = &entry.msgid else {
            return vec![];
        };
        let s = &msgid.value;
        let counts = || {
            FormatPos::new(s, entry.format_language).filter(|fmt| {
                is_count(fmt.s, entry.format_language) && !is_identifier(s, fmt.start)
            })
        };
        let Some(first) = counts().next() else {
            return vec![];
        };
        let Some((fmt, (start, end))) =
            parenthesized_plural(s)
                .map(|word| (first, word))
                .or_else(|| {
                    is_english(&checker.config.check.spelling_lang_id)
                        .then(|| {
                            counts()
                                .find_map(|fmt| plural_after(s, fmt.end).map(|word| (fmt, word)))
                        })
                        .flatten()
                })
        else {
            return vec![];
        };
        self.new_diag(
            checker,
            Severity::Info,
            format!(
                "count '{}' with plural word '{}' without plural forms",
                fmt.s,
                &s[start..end]
            ),
        )
        .map(|d| d.with_msg_hl(msgid, [(fmt.start, fmt.end), (start, end)]))
        .into_iter()
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::rule::Rules;

    fn check(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(UnpluralisedRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    /// Return the message of the diagnostic for an entry with the given
    /// format flag and source, or `None` if there is no diagnostic.
    fn check_msgid(flag: &str, msgid: &str) -> Option<String> {
        let diags = check(&format!("#, {flag}\nmsgid \"{msgid}\"\nmsgstr \"x\"\n"));
        assert!(diags.len() <= 1);
        diags.first().map(|d| d.message.to_string())
    }

    #[test]
    fn test_is_count() {
        for fmt in [
            "%d",
            "%i",
            "%u",
            "%ld",
            "%zu",
            "%'d",
            "%1$d",
            "%(count)d",
            "%<PRIu64>",
        ] {
            assert!(
                is_count(fmt, Language::C) || is_count(fmt, Language::Python),
                "{fmt}"
            );
        }
        assert!(!is_count("%s", Language::C));
        assert!(!is_count("%f", Language::C));
        for fmt in ["{count}", "{n}", "{num_files}", "{file_count}", "{total:d}"] {
            assert!(is_count(fmt, Language::PythonBrace), "{fmt}");
        }
        for fmt in ["{}", "{0}", "{name}", "{dir}", "{none}"] {
            assert!(!is_count(fmt, Language::PythonBrace), "{fmt}");
        }
        assert!(!is_count("%1", Language::Qt));
    }

    #[test]
    fn test_is_plural() {
        for word in ["files", "Bytes", "entries", "children", "people"] {
            assert!(is_plural(word), "{word}");
        }
        for word in [
            "file", "is", "was", "this", "class", "status", "axis", "ms", "as",
        ] {
            assert!(!is_plural(word), "{word}");
        }
    }

    #[test]
    fn test_plural_after() {
        assert_eq!(plural_after("%d files", 2), Some((3, 8)));
        assert_eq!(plural_after("%d new messages", 2), Some((7, 15)));
        assert_eq!(plural_after("%d big files", 2), None);
        assert_eq!(plural_after("%d file", 2), None);
        assert_eq!(plural_after("%dpx", 2), None);
        assert_eq!(plural_after("%d", 2), None);
    }

    #[test]
    fn test_parenthesized_plural() {
        assert_eq!(parenthesized_plural("%d file(s)"), Some((3, 10)));
        assert_eq!(parenthesized_plural("%d match(es)"), Some((3, 12)));
        assert_eq!(parenthesized_plural("%d (s)"), None);
        assert_eq!(parenthesized_plural("alt+enter(s)=set"), None);
    }

    #[test]
    fn test_no_error() {
        for (flag, msgid) in [
            ("c-format", "%d file deleted"),
            ("c-format", "%s files deleted"),
            ("c-format", "Line %d"),
            ("c-format", "Files: %d"),
            ("c-format", "%d%% of files"),
            ("c-format", "%dx%d pixels"),
            ("c-format", "%d-bit images"),
            ("c-format", "Area #%d Settings"),
            ("c-format", "pre-%d clients"),
            ("c-format", "%d is too big"),
            ("c-format", "%d ms elapsed"),
            ("python-brace-format", "Copying {name} files"),
            ("qt-format", "%1 files"),
            ("no-c-format", "%d files"),
        ] {
            assert_eq!(check_msgid(flag, msgid), None, "{flag}: {msgid}");
        }
        // Plural forms.
        let diags = check(
            "#, c-format\nmsgid \"%d file\"\nmsgid_plural \"%d files\"\n\
             msgstr[0] \"%d fichier\"\nmsgstr[1] \"%d fichiers\"\n",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_error() {
        for (flag, msgid, message) in [
            (
                "c-format",
                "%d files deleted",
                "count '%d' with plural word 'files'",
            ),
            (
                "c-format",
                "%zu bytes read",
                "count '%zu' with plural word 'bytes'",
            ),
            (
                "c-format",
                "%d new messages",
                "count '%d' with plural word 'messages'",
            ),
            (
                "c-format",
                "%d of %d entries",
                "count '%d' with plural word 'entries'",
            ),
            (
                "c-format",
                "%d/%d files",
                "count '%d' with plural word 'files'",
            ),
            (
                "c-format",
                "Deleted %d file(s)",
                "count '%d' with plural word 'file(s)'",
            ),
            (
                "python-format",
                "%(count)d rows returned",
                "count '%(count)d' with plural word 'rows'",
            ),
            (
                "python-brace-format",
                "Copying {count} files",
                "count '{count}' with plural word 'files'",
            ),
        ] {
            assert_eq!(
                check_msgid(flag, msgid),
                Some(format!("{message} without plural forms")),
                "{flag}: {msgid}"
            );
        }
    }

    #[test]
    fn test_source_language() {
        assert!(is_english("en"));
        assert!(is_english("en_US"));
        assert!(is_english("en-GB"));
        assert!(!is_english("fr"));
        assert!(!is_english("de_DE"));
        let content = "#, c-format\nmsgid \"%d fichiers, %d dossier(s)\"\nmsgstr \"x\"\n";
        let mut checker = Checker::new(content.as_bytes());
        checker.config.check.spelling_lang_id = "fr".to_string();
        let rules = Rules::new(vec![Box::new(UnpluralisedRule {})]);
        checker.do_all_checks(&rules);
        // Only the plural suffix in parentheses is checked in French.
        assert_eq!(checker.diagnostics.len(), 1);
        assert_eq!(
            checker.diagnostics[0].message,
            "count '%d' with plural word 'dossier(s)' without plural forms"
        );
        let content = "#, c-format\nmsgid \"%d fichiers\"\nmsgstr \"x\"\n";
        let mut checker = Checker::new(content.as_bytes());
        checker.config.check.spelling_lang_id = "fr".to_string();
        checker.do_all_checks(&rules);
        assert!(checker.diagnostics.is_empty());
    }

    #[test]
    fn test_noqa() {
        let diags = check("#, c-format, noqa:unpluralised\nmsgid \"%d files\"\nmsgstr \"x\"\n");
        assert!(diags.is_empty());
    }
}
