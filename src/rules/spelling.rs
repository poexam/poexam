// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the spelling rules: check spelling:
//! - `spelling-ctxt`: in the context (`msgctxt`)
//! - `spelling-id`: in the source (`msgid`)
//! - `spelling-str`: in the translation (`msgstr`)
//! - `spelling-translators`: in the indications for translators (extracted comments `#.`)

use std::collections::HashSet;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::dict::CachedDict;
use crate::po::entry::{CommentKinds, Entry};
use crate::po::format::iter::FormatWordPos;
use crate::po::format::language::Language;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct SpellingCtxtRule;

impl RuleChecker for SpellingCtxtRule {
    fn name(&self) -> &'static str {
        "spelling-ctxt"
    }

    fn description(&self) -> &'static str {
        "Check spelling in context string."
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check spelling in the context string (English).
    ///
    /// This rule is not enabled by default.
    ///
    /// Wrong entry:
    /// ```text
    /// msgctxt "month of the yearr"
    /// msgid "May"
    /// msgstr "Mai"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgctxt "month of the year"
    /// msgid "May"
    /// msgstr "Mai"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`info`](Severity::Info): `misspelled words in context: …`
    fn check_ctxt(&self, checker: &Checker, entry: &Entry, msgctxt: &Message) -> Vec<Diagnostic> {
        if let Some(dict) = &checker.dict_id {
            let (misspelled_words, pos_words) =
                check_words(&msgctxt.value, entry.format_language, dict);
            if !misspelled_words.is_empty() {
                return self
                    .new_diag(checker, Severity::Info, "misspelled words in context")
                    .map(|d| {
                        d.with_msg_hl(msgctxt, pos_words)
                            .with_misspelled_words(misspelled_words)
                    })
                    .into_iter()
                    .collect();
            }
        }
        vec![]
    }
}

pub struct SpellingIdRule;

impl RuleChecker for SpellingIdRule {
    fn name(&self) -> &'static str {
        "spelling-id"
    }

    fn description(&self) -> &'static str {
        "Check spelling in source string."
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check spelling in the source string (English).
    ///
    /// This rule is not enabled by default.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "this is a tyypo"
    /// msgstr "ceci est une faute"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "this is a typo"
    /// msgstr "ceci est une faute"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`info`](Severity::Info): `misspelled words in source: …`
    fn check_msg(
        &self,
        checker: &Checker,
        entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        if let Some(dict) = &checker.dict_id {
            let (misspelled_words, pos_words) =
                check_words(&msgid.value, entry.format_language, dict);
            if !misspelled_words.is_empty() {
                return self
                    .new_diag(checker, Severity::Info, "misspelled words in source")
                    .map(|d| {
                        d.with_msgs_hl(msgid, pos_words, msgstr, [])
                            .with_misspelled_words(misspelled_words)
                    })
                    .into_iter()
                    .collect();
            }
        }
        vec![]
    }
}

pub struct SpellingStrRule;

impl RuleChecker for SpellingStrRule {
    fn name(&self) -> &'static str {
        "spelling-str"
    }

    fn description(&self) -> &'static str {
        "Check spelling in translation string."
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check spelling in the translated string (using language detected in PO file).
    ///
    /// This rule is not enabled by default.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "this is a typo"
    /// msgstr "ceci est une fôte"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "this is a typo"
    /// msgstr "ceci est une faute"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`info`](Severity::Info): `misspelled words in translation: …`
    fn check_msg(
        &self,
        checker: &Checker,
        entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        if let Some(dict) = &checker.dict_str {
            let (misspelled_words, pos_words) =
                check_words(&msgstr.value, entry.format_language, dict);
            if !misspelled_words.is_empty() {
                return self
                    .new_diag(checker, Severity::Info, "misspelled words in translation")
                    .map(|d| {
                        d.with_msgs_hl(msgid, [], msgstr, pos_words)
                            .with_misspelled_words(misspelled_words)
                    })
                    .into_iter()
                    .collect();
            }
        }
        vec![]
    }
}

pub struct SpellingTranslatorsRule;

impl RuleChecker for SpellingTranslatorsRule {
    fn name(&self) -> &'static str {
        "spelling-translators"
    }

    fn description(&self) -> &'static str {
        "Check spelling in indications for translators."
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    fn uses_comments(&self) -> CommentKinds {
        CommentKinds {
            extracted: true,
            ..CommentKinds::NONE
        }
    }

    /// Check spelling in the indications for translators (English): the extracted
    /// comments (`#. comment`), written by the developer in the source code.
    ///
    /// This rule is not enabled by default.
    ///
    /// Wrong entry:
    /// ```text
    /// #. TRANSLATORS: this is a tyypo
    /// msgid "this is a typo"
    /// msgstr "ceci est une faute"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// #. TRANSLATORS: this is a typo
    /// msgid "this is a typo"
    /// msgstr "ceci est une faute"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`info`](Severity::Info): `misspelled words in indications for translators: …`
    fn check_entry(&self, checker: &Checker, entry: &Entry) -> Vec<Diagnostic> {
        let Some(dict) = &checker.dict_id else {
            return vec![];
        };
        let mut misspelled_words: HashSet<&str> = HashSet::new();
        let mut lines = Vec::new();
        for comment in &entry.extracted_comments {
            // Comments generated by po4a (e.g. "type: Content of: <para>"): not prose.
            if comment.value.starts_with("type: ") {
                continue;
            }
            let mut pos_words = Vec::new();
            for (offset, token) in get_prose_tokens(&comment.value) {
                let (words, pos) = check_words(token, Language::Null, dict);
                misspelled_words.extend(words);
                pos_words.extend(pos.into_iter().map(|(s, e)| (s + offset, e + offset)));
            }
            if !pos_words.is_empty() {
                lines.push((comment, pos_words));
            }
        }
        if misspelled_words.is_empty() {
            return vec![];
        }
        self.new_diag(
            checker,
            Severity::Info,
            "misspelled words in indications for translators",
        )
        .map(|mut d| {
            for (comment, pos_words) in lines {
                // Display the comment as in the PO file, with the "#. " prefix.
                d.add_line(
                    comment.line_number,
                    format!("#. {}", comment.value),
                    pos_words
                        .into_iter()
                        .map(|(start, end)| (start + 3, end + 3)),
                );
            }
            d.with_entry(entry).with_misspelled_words(misspelled_words)
        })
        .into_iter()
        .collect()
    }
}

/// Return the tokens of an indication for translators that are prose, with their
/// byte offset in `s`.
///
/// Tokens are separated by whitespace; skipped tokens are:
/// - text between double quotes, usually a literal string or a keyword
///   (e.g. `please do not translate "chghost"`),
/// - technical tokens like paths, URLs, file names, identifiers or markup
///   (e.g. `./data/gui.xml`, `https://example.com`, `msgstr[0]`, `<name>`).
fn get_prose_tokens(s: &str) -> Vec<(usize, &str)> {
    let mut tokens = Vec::new();
    let mut in_quotes = false;
    let mut start = None;
    for (pos, c) in s.char_indices().chain(std::iter::once((s.len(), ' '))) {
        if c.is_whitespace() {
            if let Some(start) = start.take() {
                let token = &s[start..pos];
                let quotes = token.matches(['"', '“', '”']).count();
                if !in_quotes && quotes == 0 && !is_technical_token(token) {
                    tokens.push((start, token));
                }
                if quotes % 2 == 1 {
                    in_quotes = !in_quotes;
                }
            }
        } else if start.is_none() {
            start = Some(pos);
        }
    }
    tokens
}

/// Return `true` if the token looks technical (path, URL, file name, identifier, markup…)
/// rather than prose.
fn is_technical_token(token: &str) -> bool {
    let bytes = token.as_bytes();
    bytes.iter().any(|b| {
        matches!(
            b,
            b'/' | b'\\' | b'@' | b'_' | b'=' | b'<' | b'>' | b'{' | b'}' | b'[' | b']' | b'$'
        )
    }) || bytes.windows(3).any(|w| {
        matches!(w[1], b'.' | b':') && w[0].is_ascii_alphanumeric() && w[2].is_ascii_alphanumeric()
    })
}

/// Check words in a string: context (msgctxt), source (msgid) or translation (msgstr).
///
/// Each distinct word is looked up in the dictionary only once per file: `dict`
/// memoizes the result of every check (see [`CachedDict`]).
///
/// Return list of misspelled words (can be empty) and their positions in the string (start, end).
fn check_words<'s>(
    s: &'s str,
    format_language: Language,
    dict: &CachedDict,
) -> (HashSet<&'s str>, Vec<(usize, usize)>) {
    let mut misspelled_words: HashSet<&str> = HashSet::new();
    let mut pos_words = Vec::new();
    for word in FormatWordPos::new(s, format_language) {
        // Ignore word if it contains at least one digit.
        if word.s.chars().any(|c| c.is_ascii_digit()) {
            continue;
        }
        // Ignore with at least two chars and only uppercase chars (e.g. "HTTP").
        if word.s.len() >= 2 && word.s.chars().all(|c| c.is_ascii_uppercase()) {
            continue;
        }
        if !dict.check(word.s) {
            misspelled_words.insert(word.s);
            pos_words.push((word.start, word.end));
        }
    }
    (misspelled_words, pos_words)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::{config::Config, diagnostic::Diagnostic, rules::rule::Rules};

    fn check_spelling(content: &str) -> Vec<Diagnostic> {
        let mut test_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        test_dir.push("resources");
        test_dir.push("test");
        let mut config = Config::default();
        config.check.path_dicts = test_dir;
        let mut checker = Checker::new(content.as_bytes()).with_config(config);
        let rules = Rules::new(vec![
            Box::new(SpellingCtxtRule {}),
            Box::new(SpellingIdRule {}),
            Box::new(SpellingStrRule {}),
            Box::new(SpellingTranslatorsRule {}),
        ]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_spelling_ok() {
        let diags = check_spelling(
            r#"
msgid ""
msgstr "Language: fr\n"

msgctxt "some context"
msgid "tested: HTTP v3"
msgstr "testé : HTTP v3"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_spelling_same_words_in_multiple_entries() {
        // Words checked in an entry are memoized for the whole file: the same
        // typo must still be reported in every entry where it appears, and the
        // same correct word must stay correct.
        let diags = check_spelling(
            r#"
msgid ""
msgstr "Language: fr\n"

msgid "this is a tyypo"
msgstr "ceci est une faute"

msgid "a tyypo is this"
msgstr "ceci est une faute"
"#,
        );
        assert_eq!(diags.len(), 2);
        for diag in &diags {
            assert_eq!(diag.rule, "spelling-id");
            assert_eq!(diag.misspelled_words, HashSet::from(["tyypo".to_string()]));
        }
    }

    #[test]
    fn test_spelling_error_noqa() {
        let diags = check_spelling(
            r#"
msgid ""
msgstr "Language: fr\n"

#, noqa:spelling-ctxt;spelling-id;spelling-str
msgctxt "some contxet, some contxet"
msgid "this is a tyypo, this is a tyypo"
msgstr "ceci est unz fôte, ceci est unz fôte"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_spelling_error() {
        let diags = check_spelling(
            r#"
msgid ""
msgstr "Language: fr\n"

msgctxt "some contxet, some contxet"
msgid "this is a tyypo, this is a tyypo"
msgstr "ceci est unz fôte, ceci est unz fôte"
"#,
        );
        assert_eq!(diags.len(), 3);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.build_message(), "misspelled words in context: contxet");
        assert_eq!(
            diag.misspelled_words,
            HashSet::from(["contxet".to_string()])
        );
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.build_message(), "misspelled words in source: tyypo");
        assert_eq!(diag.misspelled_words, HashSet::from(["tyypo".to_string()]));
        let diag = &diags[2];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(
            diag.build_message(),
            "misspelled words in translation: fôte, unz"
        );
        assert_eq!(
            diag.misspelled_words,
            HashSet::from(["fôte".to_string(), "unz".to_string()])
        );
    }

    #[test]
    fn test_spelling_translators_ok() {
        let diags = check_spelling(
            r#"
msgid ""
msgstr "Language: fr\n"

#. TRANSLATORS: this is a typo
#. HTTP v3
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_spelling_translators_noqa() {
        let diags = check_spelling(
            r#"
msgid ""
msgstr "Language: fr\n"

#, noqa:spelling-translators
#. TRANSLATORS: this is a tyypo
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_spelling_translators_error() {
        let diags = check_spelling(
            r#"
msgid ""
msgstr "Language: fr\n"

# this is a tyypo in a translator comment (not checked)
#. TRANSLATORS: this is a tyypo
#. a tyypo is a contxet
#: src/tyypo.c:42
msgid "tested"
msgstr "testé"
"#,
        );
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.rule, "spelling-translators");
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(
            diag.build_message(),
            "misspelled words in indications for translators: contxet, tyypo"
        );
        assert_eq!(
            diag.misspelled_words,
            HashSet::from(["contxet".to_string(), "tyypo".to_string()])
        );
        assert_eq!(diag.lines.len(), 4);
        assert_eq!(diag.lines[0].line_number, 6);
        assert_eq!(diag.lines[0].message, "#. TRANSLATORS: this is a tyypo");
        assert_eq!(diag.lines[0].highlights, vec![(26, 31)]);
        assert_eq!(diag.lines[1].line_number, 7);
        assert_eq!(diag.lines[1].highlights, vec![(5, 10), (16, 23)]);
    }

    #[test]
    fn test_get_prose_tokens() {
        assert!(get_prose_tokens("").is_empty());
        assert_eq!(
            get_prose_tokens("this is  a typo"),
            vec![(0, "this"), (5, "is"), (9, "a"), (11, "typo")]
        );
        assert_eq!(
            get_prose_tokens(r#"do not translate "foo bar" or "baz", ok"#),
            vec![
                (0, "do"),
                (3, "not"),
                (7, "translate"),
                (27, "or"),
                (37, "ok")
            ]
        );
        assert_eq!(
            get_prose_tokens("see ./data/gui.xml, https://example.com or msgstr[0] (e.g. <name>)"),
            vec![(0, "see"), (40, "or")]
        );
        assert_eq!(get_prose_tokens("color lightred:blue"), vec![(0, "color")]);
        assert_eq!(
            get_prose_tokens("note: typo."),
            vec![(0, "note:"), (6, "typo.")]
        );
    }

    #[test]
    fn test_spelling_translators_skipped() {
        let diags = check_spelling(
            r#"
msgid ""
msgstr "Language: fr\n"

#. type: Content of: <tyypo>
#. TRANSLATORS: this is a "tyypo" ./tyypo.c tyypo_func tyypo.c
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }
}
