// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the punctuation rules: check inconsistent punctuation:
//! - `punc-start`: punctuation at the beginning of the string
//! - `punc-end`: punctuation at the end of the string
//! - `punc-repeated`: repeated punctuation in the translation

use std::borrow::Cow;
use std::ops::Range;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Edit, Fix, FixTarget};
use crate::po::entry::Entry;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct PuncStartRule;

impl RuleChecker for PuncStartRule {
    fn name(&self) -> &'static str {
        "punc-start"
    }

    fn description(&self) -> &'static str {
        "Check for inconsistent leading punctuation between source and translation."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for inconsistent leading punctuation between source and translation.
    ///
    /// The following characters are considered as punctuation for this check
    /// (Latin half-width and full-width, plus several script-specific marks):
    /// - colon: `:`, `：`
    /// - semicolon: `;`, `；`, U+061B (Arabic semicolon)
    /// - full stop (period): `.`, `。`, `…`, U+0964 (Devanagari danda),
    ///   U+0965 (Devanagari double danda), U+17D4 (Khmer khan),
    ///   U+104B (Myanmar section), `։` (Armenian full stop)
    /// - comma: `,`, `，`, `،`, `、` (Japanese/Chinese ideographic comma)
    /// - exclamation mark: `!`, `！`
    /// - question mark: `?`, `？`, U+061F (Arabic question mark)
    ///
    /// Special cases handled:
    /// - Greek: the question mark is `;`.
    /// - Leading dots in the source or translation are ignored, because they
    ///   are often used for hidden or filename extension.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "; this is a test"
    /// msgstr "ceci est un test"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "; this is a test"
    /// msgstr "; ceci est un test"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`info`](Severity::Info): `inconsistent leading punctuation ('…' / '…')` (auto-fixable)
    fn check_msg(
        &self,
        checker: &Checker,
        _entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let language = checker.language_code();
        let ignore_ellipsis = checker.config.check.punc_ignore_ellipsis;
        let id_punc = get_punc_start(&msgid.value);
        let str_punc = get_punc_start(&msgstr.value);
        let id_punc2 = punc_normalize(id_punc, language, ignore_ellipsis);
        let str_punc2 = punc_normalize(str_punc, language, ignore_ellipsis);
        if id_punc2.starts_with('.') || str_punc2.starts_with('.') {
            // Ignore leading dots, often used for hidden or filename extension,
            // and the translation may change the order of words.
            // For example:
            //   msgid ".po file broken"
            //   msgstr "fichier .po cassé"
            return vec![];
        }
        if id_punc2 == str_punc2 {
            vec![]
        } else {
            let fix = Fix {
                target: FixTarget::Msgstr {
                    file_byte_range: msgstr.byte_range.clone(),
                },
                edits: vec![Edit {
                    range: 0..str_punc.len(),
                    replacement: id_punc.to_string(),
                }],
                safe: true,
            };
            self.new_diag(
                checker,
                Severity::Info,
                format!("inconsistent leading punctuation ('{id_punc2}' / '{str_punc2}')"),
            )
            .map(|d| {
                d.with_msgs_hl(msgid, [(0, id_punc.len())], msgstr, [(0, str_punc.len())])
                    .with_fix(fix)
            })
            .into_iter()
            .collect()
        }
    }
}

pub struct PuncEndRule;

impl RuleChecker for PuncEndRule {
    fn name(&self) -> &'static str {
        "punc-end"
    }

    fn description(&self) -> &'static str {
        "Check for inconsistent trailing punctuation between source and translation."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for inconsistent trailing punctuation between source and translation.
    ///
    /// The following characters are considered as punctuation for this check
    /// (Latin half-width and full-width, plus several script-specific marks):
    /// - colon: `:`, `：`
    /// - semicolon: `;`, `；`, U+061B (Arabic semicolon)
    /// - full stop (period): `.`, `。`, `…`, U+0964 (Devanagari danda),
    ///   U+0965 (Devanagari double danda), U+17D4 (Khmer khan),
    ///   U+104B (Myanmar section), `։` (Armenian full stop)
    /// - comma: `,`, `，`, `،`, `、` (Japanese/Chinese ideographic comma)
    /// - exclamation mark: `!`, `！`
    /// - question mark: `?`, `？`, U+061F (Arabic question mark)
    ///
    /// Special cases handled:
    /// - Greek: the question mark is `;`.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "This is a test."
    /// msgstr "Ceci est un test"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "This is a test."
    /// msgstr "Ceci est un test."
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`info`](Severity::Info): `inconsistent trailing punctuation ('…' / '…')` (auto-fixable)
    fn check_msg(
        &self,
        checker: &Checker,
        _entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let language = checker.language_code();
        let ignore_ellipsis = checker.config.check.punc_ignore_ellipsis;
        let id_punc = get_punc_end(&msgid.value);
        let str_punc = get_punc_end(&msgstr.value);
        let id_punc2 = punc_normalize(id_punc, language, ignore_ellipsis);
        let str_punc2 = punc_normalize(str_punc, language, ignore_ellipsis);
        if id_punc2 == str_punc2 {
            vec![]
        } else {
            let str_punc_start = msgstr.value.len() - str_punc.len();
            let fix = Fix {
                target: FixTarget::Msgstr {
                    file_byte_range: msgstr.byte_range.clone(),
                },
                edits: vec![Edit {
                    range: str_punc_start..msgstr.value.len(),
                    replacement: id_punc.to_string(),
                }],
                safe: true,
            };
            self.new_diag(
                checker,
                Severity::Info,
                format!("inconsistent trailing punctuation ('{id_punc2}' / '{str_punc2}')"),
            )
            .map(|d| {
                d.with_msgs_hl(
                    msgid,
                    [(msgid.value.len() - id_punc.len(), msgid.value.len())],
                    msgstr,
                    [(str_punc_start, msgstr.value.len())],
                )
                .with_fix(fix)
            })
            .into_iter()
            .collect()
        }
    }
}

pub struct PuncRepeatedRule;

impl RuleChecker for PuncRepeatedRule {
    fn name(&self) -> &'static str {
        "punc-repeated"
    }

    fn description(&self) -> &'static str {
        "Check for repeated punctuation in translation that is not in source."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for repeated punctuation in the translation that is not in the source.
    ///
    /// A repeated punctuation is a run of two or more of the same character among
    /// (half-width and full-width are the same character):
    /// - colon: `:`, `：`
    /// - semicolon: `;`, `；`, U+061B (Arabic semicolon)
    /// - comma: `,`, `，`, `،`, `、` (Japanese/Chinese ideographic comma)
    /// - exclamation mark: `!`, `！`
    /// - question mark: `?`, `？`, U+061F (Arabic question mark)
    ///
    /// Full stops are not checked, since `..` and `...` are common (ellipsis, paths).
    ///
    /// Only the interior of the translation is checked: the leading and trailing
    /// punctuation are checked by the rules `punc-start` and `punc-end`.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "Warning! The file will be deleted."
    /// msgstr "Attention !! Le fichier sera supprimé."
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "Warning! The file will be deleted."
    /// msgstr "Attention ! Le fichier sera supprimé."
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`info`](Severity::Info): `extra repeated punctuation '…' (# / #)` (auto-fixable
    ///   only when the source has *no* repeated punctuation of this character)
    fn check_msg(
        &self,
        checker: &Checker,
        _entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let str_start = get_punc_start(&msgstr.value).len();
        let str_end = msgstr.value.len() - get_punc_end(&msgstr.value).len();
        if str_start >= str_end {
            return vec![];
        }
        let str_runs = get_punc_repeated(&msgstr.value[..str_end], str_start);
        if str_runs.is_empty() {
            return vec![];
        }
        let id_runs = get_punc_repeated(&msgid.value, 0);
        let mut diags = vec![];
        let mut seen: Vec<char> = vec![];
        for (punc, range) in &str_runs {
            if seen.contains(punc) {
                continue;
            }
            seen.push(*punc);
            let id_count = id_runs.iter().filter(|(c, _)| c == punc).count();
            let str_count = str_runs.iter().filter(|(c, _)| c == punc).count();
            if str_count <= id_count {
                continue;
            }
            // Auto-fix only the unambiguous case: the source has *no* repeated
            // punctuation of this character, so every run in the translation collapses
            // to its first character.
            let fix = (id_count == 0).then(|| Fix {
                target: FixTarget::Msgstr {
                    file_byte_range: msgstr.byte_range.clone(),
                },
                edits: str_runs
                    .iter()
                    .filter(|(c, _)| c == punc)
                    .filter_map(|(_, r)| {
                        let first = msgstr.value[r.start..].chars().next()?;
                        Some(Edit {
                            range: r.clone(),
                            replacement: first.to_string(),
                        })
                    })
                    .collect(),
                safe: true,
            });
            diags.extend(
                self.new_diag(
                    checker,
                    Severity::Info,
                    format!(
                        "extra repeated punctuation '{}' ({id_count} / {str_count})",
                        &msgstr.value[range.clone()]
                    ),
                )
                .map(|d| {
                    d.with_msgs_hl(
                        msgid,
                        id_runs
                            .iter()
                            .filter(|(c, _)| c == punc)
                            .map(|(_, r)| (r.start, r.end)),
                        msgstr,
                        str_runs
                            .iter()
                            .filter(|(c, _)| c == punc)
                            .map(|(_, r)| (r.start, r.end)),
                    )
                    .with_optional_fix(fix)
                }),
            );
        }
        diags
    }
}

/// Normalize a punctuation character checked by the rule `punc-repeated` to its
/// half-width symbol, or return `None` if the character is not checked.
const fn punc_repeated_normalize(c: char) -> Option<char> {
    match c {
        ':' | '：' => Some(':'),
        ';' | '；' | '\u{061B}' => Some(';'),
        ',' | '，' | '،' | '、' => Some(','),
        '!' | '！' => Some('!'),
        '?' | '？' | '\u{061F}' => Some('?'),
        _ => None,
    }
}

/// Get the runs of two or more of the same punctuation character (see
/// [`punc_repeated_normalize`]) in `s`, starting at byte index `start`.
///
/// Each run is returned with its normalized punctuation character and its byte range.
fn get_punc_repeated(s: &str, start: usize) -> Vec<(char, Range<usize>)> {
    let mut runs = vec![];
    let mut run: Option<(char, usize, usize, usize)> = None;
    for (idx, c) in s[start..].char_indices() {
        let idx = start + idx;
        let punc = punc_repeated_normalize(c);
        match (run, punc) {
            (Some((run_punc, run_start, _, count)), Some(p)) if p == run_punc => {
                run = Some((run_punc, run_start, idx + c.len_utf8(), count + 1));
            }
            _ => {
                if let Some((run_punc, run_start, run_end, count)) = run
                    && count >= 2
                {
                    runs.push((run_punc, run_start..run_end));
                }
                run = punc.map(|p| (p, idx, idx + c.len_utf8(), 1));
            }
        }
    }
    if let Some((run_punc, run_start, run_end, count)) = run
        && count >= 2
    {
        runs.push((run_punc, run_start..run_end));
    }
    runs
}

/// Check if a character is considered as punctuation for this rule.
///
/// Covers Latin (ASCII and full-width), CJK ideographic, Arabic, and several
/// other scripts whose punctuation regularly appears at sentence boundaries
/// in PO translations.
const fn is_punc(c: char) -> bool {
    c == ':'
        || c == '：'
        || c == ';'
        || c == '；'
        // Arabic semicolon.
        || c == '\u{061B}'
        || c == '.'
        || c == '。'
        || c == '…'
        // Devanagari danda (period for Hindi, Bengali, Marathi, Nepali, …).
        || c == '\u{0964}'
        // Devanagari double danda (verse / section end).
        || c == '\u{0965}'
        // Khmer "khan" (period).
        || c == '\u{17D4}'
        // Myanmar (Burmese) section (period).
        || c == '\u{104B}'
        // Armenian full stop.
        || c == '։'
        || c == ','
        || c == '，'
        || c == '،'
        // Japanese / Chinese ideographic comma.
        || c == '、'
        || c == '!'
        || c == '！'
        || c == '?'
        || c == '？'
        // Arabic question mark.
        || c == '\u{061F}'
}

/// Get the leading punctuation of a string. The returned slice includes any
/// whitespace surrounding the punctuation run on both sides (any whitespace
/// except `\n`), stopping at the first non-punctuation non-whitespace
/// character.
///
/// Returns an empty slice if no punctuation character is present in the
/// leading region — pure leading whitespace is not considered "punctuation".
fn get_punc_start(s: &str) -> &str {
    let mut saw_punc = false;
    let mut idx = 0;
    for c in s.chars() {
        if is_punc(c) {
            saw_punc = true;
            idx += c.len_utf8();
        } else if c.is_whitespace() && c != '\n' {
            idx += c.len_utf8();
        } else {
            break;
        }
    }
    if saw_punc { &s[..idx] } else { "" }
}

/// Get the trailing punctuation of a string. The returned slice includes any
/// whitespace surrounding the punctuation run on both sides (any whitespace
/// except `\n`), stopping at the first non-punctuation non-whitespace
/// character.
///
/// Returns an empty slice if no punctuation character is present in the
/// trailing region.
fn get_punc_end(s: &str) -> &str {
    let mut saw_punc = false;
    let mut pos = 0;
    for c in s.chars().rev() {
        if is_punc(c) {
            saw_punc = true;
            pos += c.len_utf8();
        } else if c.is_whitespace() && c != '\n' {
            pos += c.len_utf8();
        } else {
            break;
        }
    }
    if saw_punc { &s[s.len() - pos..] } else { "" }
}

/// Normalize punctuation to English symbols: full-width to half-width and take care
/// about specific cases in some languages. Also strips every whitespace
/// character from the input — spacing around punctuation is a per-script
/// convention checked by the `punc-space-*` rules, so the start/end rules
/// only care about which punctuation symbols appear.
///
/// Returns `Cow::Borrowed` when the input is already normalized and contains
/// no whitespace (the common case for ASCII English-style punctuation),
/// avoiding an allocation.
fn punc_normalize<'a>(s: &'a str, language: &str, ignore_ellipsis: bool) -> Cow<'a, str> {
    let needs_substitution = s.chars().any(|c| {
        matches!(
            c,
            '：' | '；'
                | '\u{061B}'
                | '。'
                | '，'
                | '،'
                | '！'
                | '？'
                | '\u{061F}'
                | '、'
                | '\u{0964}'
                | '\u{0965}'
                | '\u{17D4}'
                | '\u{104B}'
                | '։'
        ) || (c == '?' && language == "el")
            || c.is_whitespace()
    });
    let needs_ellipsis = ignore_ellipsis && s.contains("...");
    if !needs_substitution && !needs_ellipsis {
        return Cow::Borrowed(s);
    }
    let value: String = s
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            // Special case for Greek question mark.
            '?' if language == "el" => ';',
            // General punctuation normalization.
            '：' => ':',
            '；' | '\u{061B}' => ';',
            '。' | '\u{0964}' | '\u{0965}' | '\u{17D4}' | '\u{104B}' | '։' => '.',
            '，' | '،' | '、' => ',',
            '！' => '!',
            '？' | '\u{061F}' => '?',
            _ => c,
        })
        .collect();
    if ignore_ellipsis {
        Cow::Owned(value.replace("...", "…"))
    } else {
        Cow::Owned(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Config, diagnostic::Diagnostic, rules::rule::Rules};

    fn check_punc_start(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(PuncStartRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    fn check_punc_end(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(PuncEndRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    fn check_punc_repeated(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(PuncRepeatedRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    fn check_punc_end_ignore_ellipsis(content: &str) -> Vec<Diagnostic> {
        let mut config = Config::default();
        config.check.punc_ignore_ellipsis = true;
        let mut checker = Checker::new(content.as_bytes()).with_config(config);
        let rules = Rules::new(vec![Box::new(PuncEndRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_is_punc() {
        // Characters that should be recognized as punctuation: ASCII,
        // full-width / CJK, ellipsis, Arabic, Devanagari / Indic, Khmer,
        // Myanmar, Armenian.
        let punc_chars = [
            ':', ';', '.', ',', '!', '?', '：', '；', '。', '，', '！', '？', '、', '…',
            '\u{061B}', '\u{061F}', '،', '\u{0964}', '\u{0965}', '\u{17D4}', '\u{104B}', '։',
        ];
        for &c in &punc_chars {
            assert!(is_punc(c), "{c} should be punctuation");
        }
        // Characters that should not be recognized as punctuation.
        let non_punc_chars = [
            'a', 'Z', ' ', '-', '\'', '"', '0', 'é', '(', ')', '\r', '\n',
        ];
        for &c in &non_punc_chars {
            assert!(!is_punc(c), "{c} should not be punctuation");
        }
    }

    #[test]
    fn test_get_punc_start() {
        assert_eq!(get_punc_start(""), "");
        assert_eq!(get_punc_start("test"), "");
        // Leading whitespace without any punctuation returns empty.
        assert_eq!(get_punc_start(" test"), "");
        assert_eq!(get_punc_start("  test"), "");
        // Punctuation pulls in whitespace on both sides.
        assert_eq!(get_punc_start(", test"), ", ");
        assert_eq!(get_punc_start(" , test"), " , ");
        assert_eq!(get_punc_start(" :test"), " :");
        assert_eq!(get_punc_start("  :  test"), "  :  ");
        assert_eq!(get_punc_start("...test"), "...");
        assert_eq!(get_punc_start("…test"), "…");
        assert_eq!(get_punc_start("テスト済み"), "");
        assert_eq!(get_punc_start("。テスト済み"), "。");
        assert_eq!(get_punc_start("。。。テスト済み"), "。。。");
    }

    #[test]
    fn test_get_punc_end() {
        assert_eq!(get_punc_end(""), "");
        assert_eq!(get_punc_end("test"), "");
        // Trailing whitespace without any punctuation returns empty.
        assert_eq!(get_punc_end("test "), "");
        assert_eq!(get_punc_end("test  "), "");
        // Punctuation pulls in whitespace on both sides.
        assert_eq!(get_punc_end("test, "), ", ");
        assert_eq!(get_punc_end("test , "), " , ");
        assert_eq!(get_punc_end("test :"), " :");
        assert_eq!(get_punc_end("test  :  "), "  :  ");
        assert_eq!(get_punc_end("test..."), "...");
        assert_eq!(get_punc_end("test…"), "…");
        assert_eq!(get_punc_end("テスト済み"), "");
        assert_eq!(get_punc_end("テスト済み。"), "。");
        assert_eq!(get_punc_end("テスト済み。。。"), "。。。");
    }

    #[test]
    fn test_punc_normalize() {
        assert_eq!(punc_normalize("", "fr", false), "");
        assert_eq!(punc_normalize("test", "fr", false), "test");
        assert_eq!(
            punc_normalize("。，！？\u{061F}：；\u{061B}。。。", "zh", false),
            ".,!??:;;..."
        );
        assert_eq!(punc_normalize("?", "fr", false), "?");
        // Special case for Greek question mark.
        assert_eq!(punc_normalize("?", "el", false), ";");
        // Test ellipsis normalization.
        assert_eq!(punc_normalize("...test...", "fr", false), "...test...");
        assert_eq!(punc_normalize("...test...", "fr", true), "…test…");
        // Script-specific punctuation normalizes to its Latin equivalent.
        assert_eq!(punc_normalize("、", "ja", false), ",");
        assert_eq!(punc_normalize("\u{0964}", "hi", false), ".");
        assert_eq!(punc_normalize("\u{0965}", "hi", false), ".");
        assert_eq!(punc_normalize("\u{17D4}", "km", false), ".");
        assert_eq!(punc_normalize("\u{104B}", "my", false), ".");
        assert_eq!(punc_normalize("։", "hy", false), ".");
        // Every whitespace character is stripped from the result.
        assert_eq!(punc_normalize(" , ", "fr", false), ",");
        assert_eq!(punc_normalize(", ...", "fr", false), ",...");
        assert_eq!(punc_normalize("\t!\n", "fr", false), "!");
        assert_eq!(punc_normalize("   ", "fr", false), "");
        // CJK comma without a space compares equal to Latin comma with a space.
        assert_eq!(
            punc_normalize(", ...", "fr", false),
            punc_normalize("、...", "ja", false),
        );
    }

    #[test]
    fn test_no_punc() {
        let diags = check_punc_start(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
        let diags = check_punc_end(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_punc_ok() {
        let diags = check_punc_end(
            r#"
msgid "tested..."
msgstr "testé..."
"#,
        );
        assert!(diags.is_empty());
        let diags = check_punc_end_ignore_ellipsis(
            r#"
msgid "tested..."
msgstr "testé…"
"#,
        );
        assert!(diags.is_empty());
        let diags = check_punc_start(
            r#"
msgid "tested."
msgstr "テスト済み。"
"#,
        );
        assert!(diags.is_empty());
        let diags = check_punc_end(
            r#"
msgid "tested."
msgstr "テスト済み。"
"#,
        );
        assert!(diags.is_empty());
        let diags = check_punc_end(
            r#"
msgid "tested,"
msgstr "テスト済み，"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_punc_error_noqa() {
        let diags = check_punc_start(
            r#"
#, noqa:punc-start
msgid ":tested!"
msgstr ",testé !!!"
"#,
        );
        assert!(diags.is_empty());
        let diags = check_punc_end(
            r#"
#, noqa:punc-end
msgid ":tested!"
msgstr ",testé !!!"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_punc_error() {
        let diags = check_punc_start(
            r#"
msgid ":tested!"
msgstr ",testé !!!"
"#,
        );
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "inconsistent leading punctuation (':' / ',')");
        let diags = check_punc_end(
            r#"
msgid ":tested!"
msgstr ",testé !!!"
"#,
        );
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(
            diag.message,
            "inconsistent trailing punctuation ('!' / '!!!')"
        );
    }

    #[test]
    fn test_punc_start_fix_replaces_leading_punc() {
        // msgstr leads with ',', msgid with ':'. Fix swaps ',' → ':'.
        let diags = check_punc_start(
            r#"
msgid ":tested"
msgstr ",testé"
"#,
        );
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].range, 0..1);
        assert_eq!(fix.edits[0].replacement, ":");
    }

    #[test]
    fn test_punc_start_fix_inserts_when_missing() {
        // msgstr has no leading punctuation; fix inserts msgid's run at offset 0.
        let diags = check_punc_start(
            r#"
msgid ";tested"
msgstr "testé"
"#,
        );
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].range, 0..0);
        assert_eq!(fix.edits[0].replacement, ";");
    }

    #[test]
    fn test_punc_end_fix_replaces_trailing_run() {
        // msgstr trails with '!!!'; msgid with '!'. Fix collapses to a single '!'.
        let diags = check_punc_end(
            r#"
msgid "tested!"
msgstr "testé!!!"
"#,
        );
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 1);
        // "testé" is 6 bytes; trailing run "!!!" is at 6..9.
        assert_eq!(fix.edits[0].range, 6..9);
        assert_eq!(fix.edits[0].replacement, "!");
    }

    #[test]
    fn test_punc_end_fix_appends_when_missing() {
        // msgstr has no trailing punctuation; fix appends msgid's run.
        let diags = check_punc_end(
            r#"
msgid "tested."
msgstr "testé"
"#,
        );
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 1);
        // "testé" is 6 bytes; insertion at the end (6..6).
        assert_eq!(fix.edits[0].range, 6..6);
        assert_eq!(fix.edits[0].replacement, ".");
    }

    #[test]
    fn test_get_punc_repeated() {
        assert!(get_punc_repeated("", 0).is_empty());
        assert!(get_punc_repeated("a, b; c: d! e? f... g", 0).is_empty());
        assert_eq!(
            // "!？" is not a run, "!！" is a run (same character, mixed widths).
            get_punc_repeated("a!! b??? c,, d!？ e!！ f:: g", 0),
            vec![
                ('!', 1..3),
                ('?', 5..8),
                (',', 10..12),
                ('!', 20..24),
                (':', 26..28)
            ]
        );
        assert_eq!(
            get_punc_repeated("a!? b!! c!!", 0),
            vec![('!', 5..7), ('!', 9..11)]
        );
        assert_eq!(get_punc_repeated("a!! b!!", 4), vec![('!', 5..7)]);
    }

    #[test]
    fn test_punc_repeated_ok() {
        let diags = check_punc_repeated(
            r#"
msgid "Warning! Really?! Wait... done."
msgstr "Attention ! Vraiment ?! Attendez... terminé."

msgid "Warning!! The file will be deleted."
msgstr "Attention !! Le fichier sera supprimé."

msgid "Use Class::method, then quit."
msgstr "Utiliser Class::method, puis quitter."

msgid "Oh!"
msgstr "Oh !!!"

msgid "?? unknown"
msgstr "?? inconnu"
"#,
        );
        // Leading and trailing punctuation is checked by rules "punc-start" and
        // "punc-end".
        assert!(diags.is_empty());
    }

    #[test]
    fn test_punc_repeated_error_noqa() {
        let diags = check_punc_repeated(
            r#"
#, noqa:punc-repeated
msgid "Warning! The file will be deleted."
msgstr "Attention !! Le fichier sera supprimé."
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_punc_repeated_error() {
        let diags = check_punc_repeated(
            r#"
msgid "Warning! The file will be deleted."
msgstr "Attention !! Le fichier sera supprimé."

msgid "Really? Yes, sure."
msgstr "Vraiment ??? Oui,, bien sûr."

msgid "Warning!! Really! Sure."
msgstr "Attention !! Vraiment !! Sûr."

msgid "警告！文件将被删除。"
msgstr "警告！！文件将被删除。"
"#,
        );
        assert_eq!(diags.len(), 5);
        assert!(diags.iter().all(|d| d.severity == Severity::Info));
        assert_eq!(diags[0].message, "extra repeated punctuation '!!' (0 / 1)");
        assert_eq!(diags[1].message, "extra repeated punctuation '???' (0 / 1)");
        assert_eq!(diags[2].message, "extra repeated punctuation ',,' (0 / 1)");
        assert_eq!(diags[3].message, "extra repeated punctuation '!!' (1 / 2)");
        assert_eq!(
            diags[4].message,
            "extra repeated punctuation '！！' (0 / 1)"
        );
    }

    #[test]
    fn test_punc_repeated_fix_collapses_runs() {
        let diags = check_punc_repeated(
            r#"
msgid "Warning! Stop! Now."
msgstr "Attention !! Stop ！！！ Maintenant."
"#,
        );
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert!(fix.safe);
        // "Attention !! Stop ！！！": runs at bytes 10..12 and 18..27.
        assert_eq!(fix.edits.len(), 2);
        assert_eq!(fix.edits[0].range, 10..12);
        assert_eq!(fix.edits[0].replacement, "!");
        assert_eq!(fix.edits[1].range, 18..27);
        assert_eq!(fix.edits[1].replacement, "！");
    }

    #[test]
    fn test_punc_repeated_no_fix_when_source_has_runs() {
        // Source has a repeated "!", so which run of the translation is surplus is
        // ambiguous: report but do not fix.
        let diags = check_punc_repeated(
            r#"
msgid "Warning!! Really! Sure."
msgstr "Attention !! Vraiment !! Sûr."
"#,
        );
        assert_eq!(diags.len(), 1);
        assert!(diags[0].fix.is_none());
    }
}
