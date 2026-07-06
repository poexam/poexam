// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `unicode-ctrl` rule: check for stray Unicode control
//! and format characters (zero-width, bidi overrides, C0/C1 controls, BOM, …)
//! introduced by the translation but absent from the source string.

use std::collections::{BTreeMap, HashSet};

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Edit, Fix, FixTarget};
use crate::po::entry::Entry;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct UnicodeCtrlRule;

impl RuleChecker for UnicodeCtrlRule {
    fn name(&self) -> &'static str {
        "unicode-ctrl"
    }

    fn description(&self) -> &'static str {
        "Check for stray Unicode control or format characters in translation."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for Unicode control / format characters that appear in the translation
    /// but not in the source string. These are usually invisible (zero-width spaces,
    /// bidi overrides, soft hyphens, BOM, C0/C1 controls, …) and are a typical
    /// copy-paste accident from word processors or rich-text editors.
    ///
    /// The rule is asymmetric: a control character that is *also* present in `msgid`
    /// is assumed to be intentional (e.g. a `ZWJ` inside an emoji sequence) and is
    /// not reported. Visible space characters such as `NBSP` (U+00A0) and `NNBSP`
    /// (U+202F) are not flagged here — they are handled by `punc-space-str`.
    ///
    /// Per-locale exemptions: a few format characters are required by certain
    /// scripts and are silently ignored when the PO file's declared language is
    /// known to use them legitimately. See [`is_legitimate_for_locale`] for the
    /// full table — for example `ZWSP` is exempt for Khmer/Burmese/Lao (used as
    /// a word-break marker in scripts without inherent spaces), `ZWNJ` for
    /// Persian/Urdu/Indic scripts, and `LRM` / `RLM` for RTL languages. The
    /// exemptions are deliberately narrow: bidi *overrides* (`LRO`, `RLO`) are
    /// always flagged, even in RTL languages, because they are rarely needed
    /// when the script's inherent direction already does the right thing.
    ///
    /// Wrong entry (translation contains a stray `ZERO WIDTH SPACE` between letters):
    /// ```text
    /// msgid "Save"
    /// msgstr "Sa\u{200B}ve"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "Save"
    /// msgstr "Save"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`error`](Severity::Error): `extra control character U+0000 (NULL)` (auto-fixable)
    /// - [`warning`](Severity::Warning): `extra control character U+XXXX (NAME)` (auto-fixable)
    fn check_msg(
        &self,
        checker: &Checker,
        _entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let id_set: HashSet<char> = msgid.value.chars().filter(|c| is_ctrl_char(*c)).collect();
        let lang_code = checker.language_code();
        let mut strays: BTreeMap<char, Vec<(usize, usize)>> = BTreeMap::new();
        for (idx, c) in msgstr.value.char_indices() {
            if is_ctrl_char(c) && !id_set.contains(&c) && !is_legitimate_for_locale(c, lang_code) {
                strays.entry(c).or_default().push((idx, idx + c.len_utf8()));
            }
        }
        strays
            .into_iter()
            .filter_map(|(c, positions)| {
                let msg = format!(
                    "extra control character U+{:04X} ({})",
                    c as u32,
                    ctrl_char_name(c),
                );
                let edits: Vec<Edit> = positions
                    .iter()
                    .map(|&(start, end)| Edit {
                        range: start..end,
                        replacement: String::new(),
                    })
                    .collect();
                let fix = Fix {
                    target: FixTarget::Msgstr {
                        file_byte_range: msgstr.byte_range.clone(),
                    },
                    edits,
                    safe: true,
                };
                self.new_diag(checker, ctrl_char_severity(c), msg)
                    .map(|d| d.with_msgs_hl(msgid, [], msgstr, positions).with_fix(fix))
            })
            .collect()
    }
}

/// Severity for a stray control character. NULL bytes truncate C strings and
/// silently lose the tail of the translation — that's a correctness bug. All
/// the other control / format characters are visually disruptive but don't
/// alter the meaning of valid prefixes.
fn ctrl_char_severity(c: char) -> Severity {
    match c as u32 {
        0x00 => Severity::Error,
        _ => Severity::Warning,
    }
}

/// Whether a flagged code point is a legitimate, expected use in the given language.
///
/// Lookup is by language code (the part before `_` in the PO `Language:` header,
/// e.g. `fa` for `fa_IR`). Only the format characters that have well-known
/// per-script use cases are exempted; the unconditionally-bad characters (C0/C1
/// controls, BOM, replacement, soft hyphen, bidi overrides `LRO`/`RLO`, …) are
/// never exempt.
fn is_legitimate_for_locale(c: char, language_code: &str) -> bool {
    match c as u32 {
        // ZERO WIDTH SPACE — word-break marker in scripts without inherent spaces
        // (Khmer, Burmese, Lao, Thai, Tibetan, Javanese, Shan) and a soft
        // line-break opportunity in long Indic compound words.
        0x200B => matches!(
            language_code,
            "km" | "my"
                | "lo"
                | "th"
                | "bo"
                | "jv"
                | "shn"
                | "hi"
                | "mr"
                | "ne"
                | "sa"
                | "kok"
                | "bn"
                | "as"
                | "ta"
                | "te"
                | "kn"
                | "ml"
                | "si"
                | "gu"
                | "or"
                | "pa"
        ),
        // ZERO WIDTH NON-JOINER — Arabic-script languages and Indic scripts where
        // it controls sub-word joining and is required for correct rendering.
        0x200C => matches!(
            language_code,
            "ar" | "fa"
                | "ur"
                | "ps"
                | "ckb"
                | "yi"
                | "hi"
                | "mr"
                | "ne"
                | "sa"
                | "kok"
                | "bn"
                | "as"
                | "ta"
                | "te"
                | "kn"
                | "ml"
                | "si"
                | "gu"
                | "or"
                | "pa"
        ),
        // ZERO WIDTH JOINER — Arabic-script joining and Indic ligature formation.
        // Emoji ZWJ sequences are already handled by the asymmetric msgid/msgstr
        // check (ZWJ in source means it carries through to the translation).
        0x200D => matches!(
            language_code,
            "ar" | "fa"
                | "ur"
                | "ps"
                | "ckb"
                | "hi"
                | "mr"
                | "ne"
                | "sa"
                | "kok"
                | "bn"
                | "as"
                | "ta"
                | "te"
                | "kn"
                | "ml"
                | "si"
                | "gu"
                | "or"
                | "pa"
        ),
        // Bidi marks, embeddings, isolates (but NOT overrides 0x202D/0x202E):
        // commonly needed in RTL translations of mixed-direction source strings.
        0x200E | 0x200F | 0x202A..=0x202C | 0x2066..=0x2069 => matches!(
            language_code,
            "ar" | "he" | "fa" | "ur" | "yi" | "ckb" | "ps" | "ug" | "dv"
        ),
        _ => false,
    }
}

/// Whether the character is a Unicode control or format character that should be
/// reported when it appears in the translation but not in the source.
///
/// Excludes the legitimate whitespace controls (`\t`, `\n`, `\r`), every space
/// separator (`Zs`, including NBSP and NNBSP), and variation selectors (`FE00..FE0F`,
/// `E0100..E01EF`) which are required by emoji and CJK sequences.
fn is_ctrl_char(c: char) -> bool {
    let cp = c as u32;
    matches!(cp,
        // C0 controls except `\t` (0x09), `\n` (0x0A), `\r` (0x0D).
        0x00..=0x08 | 0x0B | 0x0C | 0x0E..=0x1F
        // DEL + C1 controls.
        | 0x7F..=0x9F
        // Soft hyphen.
        | 0x00AD
        // Zero-width and bidi marks.
        | 0x200B..=0x200F
        // Bidi embeddings and overrides.
        | 0x202A..=0x202E
        // Line and paragraph separators.
        | 0x2028 | 0x2029
        // Word joiner and invisible math operators.
        | 0x2060..=0x2064
        // Bidi isolates.
        | 0x2066..=0x2069
        // Interlinear annotation anchors.
        | 0xFFF9..=0xFFFB
        // Replacement character (encoding-failure marker).
        | 0xFFFD
        // Zero-width no-break space / BOM (only legitimate at the start of a file).
        | 0xFEFF
    )
}

/// Short human-readable name for a control / format character. Falls back to a
/// generic label for the rarely-seen C0/C1 controls.
fn ctrl_char_name(c: char) -> &'static str {
    match c as u32 {
        0x00 => "NULL",
        0x07 => "BELL",
        0x08 => "BACKSPACE",
        0x0B => "VERTICAL TAB",
        0x0C => "FORM FEED",
        0x1B => "ESCAPE",
        0x7F => "DELETE",
        0x00AD => "SOFT HYPHEN",
        0x200B => "ZERO WIDTH SPACE",
        0x200C => "ZERO WIDTH NON-JOINER",
        0x200D => "ZERO WIDTH JOINER",
        0x200E => "LEFT-TO-RIGHT MARK",
        0x200F => "RIGHT-TO-LEFT MARK",
        0x202A => "LEFT-TO-RIGHT EMBEDDING",
        0x202B => "RIGHT-TO-LEFT EMBEDDING",
        0x202C => "POP DIRECTIONAL FORMATTING",
        0x202D => "LEFT-TO-RIGHT OVERRIDE",
        0x202E => "RIGHT-TO-LEFT OVERRIDE",
        0x2028 => "LINE SEPARATOR",
        0x2029 => "PARAGRAPH SEPARATOR",
        0x2060 => "WORD JOINER",
        0x2061 => "FUNCTION APPLICATION",
        0x2062 => "INVISIBLE TIMES",
        0x2063 => "INVISIBLE SEPARATOR",
        0x2064 => "INVISIBLE PLUS",
        0x2066 => "LEFT-TO-RIGHT ISOLATE",
        0x2067 => "RIGHT-TO-LEFT ISOLATE",
        0x2068 => "FIRST STRONG ISOLATE",
        0x2069 => "POP DIRECTIONAL ISOLATE",
        0xFEFF => "ZERO WIDTH NO-BREAK SPACE",
        0xFFF9 => "INTERLINEAR ANNOTATION ANCHOR",
        0xFFFA => "INTERLINEAR ANNOTATION SEPARATOR",
        0xFFFB => "INTERLINEAR ANNOTATION TERMINATOR",
        0xFFFD => "REPLACEMENT CHARACTER",
        0x01..=0x1F => "C0 CONTROL",
        0x80..=0x9F => "C1 CONTROL",
        _ => "FORMAT CHARACTER",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(UnicodeCtrlRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    /// Run the rule on a PO file with the given `Language:` header value.
    fn check_with_lang(lang: &str, body: &str) -> Vec<Diagnostic> {
        let content = format!(
            "msgid \"\"\n\
             msgstr \"\"\n\
             \"Language: {lang}\\n\"\n\
             \"Content-Type: text/plain; charset=UTF-8\\n\"\n\
             \n\
             {body}"
        );
        check(&content)
    }

    #[test]
    fn test_clean_translation_is_silent() {
        let diags = check(
            r#"
msgid "Save"
msgstr "Save"

msgid "Hello, world"
msgstr "Bonjour, le monde"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_visible_spaces_are_not_flagged() {
        // NBSP, NNBSP, and other Zs spaces are deliberately allowed — they are
        // legitimate in many locales (French uses NBSP/NNBSP before `:`, `;`,
        // `?`, `!`) and are handled by the `punc-space-*` rules, not here.
        let diags = check(
            "msgid \"price: 5\"\n\
             msgstr \"prix\u{00A0}: 5\"\n\
             \n\
             msgid \"width: 5\"\n\
             msgstr \"largeur\u{202F}: 5\"\n",
        );
        assert!(diags.is_empty(), "got unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn test_zero_width_space_in_translation_only_is_flagged() {
        let diags = check("msgid \"Save\"\nmsgstr \"Sa\u{200B}ve\"\n");
        assert_eq!(diags.len(), 1);
        let d = &diags[0];
        assert_eq!(d.severity, Severity::Warning);
        assert_eq!(
            d.message,
            "extra control character U+200B (ZERO WIDTH SPACE)"
        );
    }

    #[test]
    fn test_zero_width_joiner_in_both_is_not_flagged() {
        // Common emoji ZWJ sequence: family emoji. The translation preserves the
        // joiner from the source, which is the intended behavior.
        let diags = check(
            "msgid \"Family: \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\"\n\
             msgstr \"Famille : \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\"\n",
        );
        assert!(
            diags.is_empty(),
            "ZWJ present in both sides should not fire"
        );
    }

    #[test]
    fn test_zwj_only_in_translation_is_flagged() {
        let diags = check("msgid \"hello world\"\nmsgstr \"bonjour\u{200D}monde\"\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "extra control character U+200D (ZERO WIDTH JOINER)"
        );
    }

    #[test]
    fn test_bidi_override_is_flagged() {
        // Trojan Source style: visually reads as "Save" but with U+202E reverses
        // subsequent characters logically.
        let diags = check("msgid \"Click Save\"\nmsgstr \"Click \u{202E}evaS\"\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "extra control character U+202E (RIGHT-TO-LEFT OVERRIDE)"
        );
    }

    #[test]
    fn test_soft_hyphen_is_flagged() {
        let diags = check("msgid \"installation\"\nmsgstr \"instal\u{00AD}lation\"\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "extra control character U+00AD (SOFT HYPHEN)"
        );
    }

    #[test]
    fn test_bom_in_translation_is_flagged() {
        let diags = check("msgid \"x\"\nmsgstr \"\u{FEFF}x\"\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "extra control character U+FEFF (ZERO WIDTH NO-BREAK SPACE)"
        );
    }

    #[test]
    fn test_replacement_character_is_flagged() {
        let diags = check("msgid \"hello\"\nmsgstr \"hell\u{FFFD}\"\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "extra control character U+FFFD (REPLACEMENT CHARACTER)"
        );
    }

    #[test]
    fn test_c0_escape_is_flagged() {
        // ANSI-colored terminal output pasted into a translation.
        let diags = check("msgid \"GET\"\nmsgstr \"\u{001B}[0;32mGET\u{001B}[0m\"\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "extra control character U+001B (ESCAPE)");
    }

    #[test]
    fn test_tab_lf_cr_are_not_flagged() {
        // The standard whitespace controls are exempt — `tabs`, `newlines` and
        // `whitespace-*` rules cover those.
        let diags = check("msgid \"a\"\nmsgstr \"a\\t\\n\\r\"\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn test_variation_selectors_are_not_flagged() {
        // VS-16 turns the heart into the red heart emoji. Adding it in the
        // translation but not the source is a legitimate cosmetic upgrade,
        // not a control-char accident.
        let diags = check("msgid \"\u{2764}\"\nmsgstr \"\u{2764}\u{FE0F}\"\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn test_multiple_distinct_strays_yield_one_diagnostic_each() {
        let diags = check("msgid \"a\"\nmsgstr \"a\u{200B}\u{202E}\u{00AD}\"\n");
        assert_eq!(diags.len(), 3);
        // Diagnostics are emitted in code-point order (BTreeMap).
        assert!(diags[0].message.contains("U+00AD"));
        assert!(diags[1].message.contains("U+200B"));
        assert!(diags[2].message.contains("U+202E"));
    }

    #[test]
    fn test_repeated_same_stray_is_one_diagnostic_with_multiple_highlights() {
        let diags = check("msgid \"abc\"\nmsgstr \"a\u{200B}b\u{200B}c\"\n");
        assert_eq!(diags.len(), 1);
        let d = &diags[0];
        assert_eq!(
            d.message,
            "extra control character U+200B (ZERO WIDTH SPACE)"
        );
        // The diagnostic has three lines of context (msgid, blank separator,
        // msgstr) with both stray-char positions highlighted on the msgstr line.
        assert_eq!(d.lines.len(), 3);
        assert!(d.lines[0].highlights.is_empty());
        assert_eq!(d.lines[2].highlights.len(), 2);
    }

    #[test]
    fn test_noqa_suppresses_diagnostic() {
        let diags = check("#, noqa:unicode-ctrl\nmsgid \"Save\"\nmsgstr \"Sa\u{200B}ve\"\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn test_zwsp_ignored_for_khmer() {
        // Khmer has no inherent word spaces; ZWSP is the standard word-break marker.
        let diags = check_with_lang("km", "msgid \"hello\"\nmsgstr \"hel\u{200B}lo\"\n");
        assert!(diags.is_empty(), "ZWSP should be exempt for km");
    }

    #[test]
    fn test_zwsp_flagged_for_german() {
        // ZWSP has no legitimate use in German — almost always a paste artifact.
        let diags = check_with_lang("de", "msgid \"hello\"\nmsgstr \"hel\u{200B}lo\"\n");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("U+200B"));
    }

    #[test]
    fn test_zwsp_flagged_without_language_header() {
        // No header → no declared language → no exemption applies.
        let diags = check("msgid \"hi\"\nmsgstr \"h\u{200B}i\"\n");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("U+200B"));
    }

    #[test]
    fn test_zwsp_ignored_for_burmese_and_lao() {
        for lang in &["my", "lo", "th", "bo"] {
            let diags = check_with_lang(lang, "msgid \"hi\"\nmsgstr \"h\u{200B}i\"\n");
            assert!(diags.is_empty(), "ZWSP should be exempt for {lang}");
        }
    }

    #[test]
    fn test_zwnj_ignored_for_persian_and_indic() {
        for lang in &["fa", "ur", "hi", "bn", "ta", "te", "kn", "ml", "si"] {
            let diags = check_with_lang(lang, "msgid \"hi\"\nmsgstr \"h\u{200C}i\"\n");
            assert!(diags.is_empty(), "ZWNJ should be exempt for {lang}");
        }
    }

    #[test]
    fn test_zwnj_flagged_for_german() {
        let diags = check_with_lang("de", "msgid \"hi\"\nmsgstr \"h\u{200C}i\"\n");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("U+200C"));
    }

    #[test]
    fn test_zwj_ignored_for_indic() {
        let diags = check_with_lang("hi", "msgid \"a\"\nmsgstr \"a\u{200D}b\"\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn test_zwj_flagged_for_german() {
        let diags = check_with_lang("de", "msgid \"a\"\nmsgstr \"a\u{200D}b\"\n");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("U+200D"));
    }

    #[test]
    fn test_lrm_rlm_ignored_for_rtl_languages() {
        for lang in &["ar", "he", "fa", "ur", "ckb"] {
            let diags = check_with_lang(lang, "msgid \"x\"\nmsgstr \"x\u{200E}\u{200F}\"\n");
            assert!(diags.is_empty(), "LRM/RLM should be exempt for {lang}");
        }
    }

    #[test]
    fn test_lrm_flagged_for_german() {
        let diags = check_with_lang("de", "msgid \"x\"\nmsgstr \"x\u{200E}\"\n");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("U+200E"));
    }

    #[test]
    fn test_bidi_embeddings_ignored_for_rtl() {
        // LRE/RLE/PDF can appear in RTL translations to control mixed-direction text.
        let diags = check_with_lang("ar", "msgid \"x\"\nmsgstr \"x\u{202A}\u{202B}\u{202C}\"\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn test_rlo_lro_always_flagged_even_in_rtl() {
        // Right-to-left override is rarely needed even in Arabic/Hebrew — those
        // languages have inherent direction. Stays flagged regardless of locale.
        let diags = check_with_lang("ar", "msgid \"x\"\nmsgstr \"x\u{202E}\"\n");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("U+202E"));
        let diags = check_with_lang("he", "msgid \"x\"\nmsgstr \"x\u{202D}\"\n");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("U+202D"));
    }

    #[test]
    fn test_country_variant_is_stripped_for_lookup() {
        // Persian written as `fa_IR` should still match the `fa` exemption.
        let diags = check_with_lang("fa_IR", "msgid \"hi\"\nmsgstr \"h\u{200C}i\"\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn test_soft_hyphen_not_exempt_anywhere() {
        // German translators sometimes intentionally use soft hyphens, but the
        // rule still flags them — they're far more often paste residue.
        let diags = check_with_lang(
            "de",
            "msgid \"installation\"\nmsgstr \"instal\u{00AD}lation\"\n",
        );
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("U+00AD"));
    }

    #[test]
    fn test_is_legitimate_for_locale() {
        // Positive cases.
        assert!(is_legitimate_for_locale('\u{200B}', "km"));
        assert!(is_legitimate_for_locale('\u{200C}', "fa"));
        assert!(is_legitimate_for_locale('\u{200C}', "hi"));
        assert!(is_legitimate_for_locale('\u{200D}', "ta"));
        assert!(is_legitimate_for_locale('\u{200E}', "ar"));
        assert!(is_legitimate_for_locale('\u{200F}', "he"));
        assert!(is_legitimate_for_locale('\u{202B}', "fa"));
        // Same chars in unrelated languages.
        assert!(!is_legitimate_for_locale('\u{200B}', "de"));
        assert!(!is_legitimate_for_locale('\u{200C}', "fr"));
        assert!(!is_legitimate_for_locale('\u{200E}', "en"));
        // Bidi *overrides* are never legitimate, even in RTL.
        assert!(!is_legitimate_for_locale('\u{202D}', "ar"));
        assert!(!is_legitimate_for_locale('\u{202E}', "he"));
        // Non-format chars are never "legitimate" (they wouldn't reach this
        // function in the rule, but the predicate must still return false).
        assert!(!is_legitimate_for_locale('\u{0000}', "km"));
        assert!(!is_legitimate_for_locale('\u{FEFF}', "ar"));
    }

    #[test]
    fn test_ctrl_char_name_returns_documented_names() {
        let cases: &[(char, &str)] = &[
            ('\u{0000}', "NULL"),
            ('\u{0007}', "BELL"),
            ('\u{0008}', "BACKSPACE"),
            ('\u{000B}', "VERTICAL TAB"),
            ('\u{000C}', "FORM FEED"),
            ('\u{001B}', "ESCAPE"),
            ('\u{007F}', "DELETE"),
            ('\u{00AD}', "SOFT HYPHEN"),
            ('\u{200B}', "ZERO WIDTH SPACE"),
            ('\u{200C}', "ZERO WIDTH NON-JOINER"),
            ('\u{200D}', "ZERO WIDTH JOINER"),
            ('\u{200E}', "LEFT-TO-RIGHT MARK"),
            ('\u{200F}', "RIGHT-TO-LEFT MARK"),
            ('\u{202A}', "LEFT-TO-RIGHT EMBEDDING"),
            ('\u{202B}', "RIGHT-TO-LEFT EMBEDDING"),
            ('\u{202C}', "POP DIRECTIONAL FORMATTING"),
            ('\u{202D}', "LEFT-TO-RIGHT OVERRIDE"),
            ('\u{202E}', "RIGHT-TO-LEFT OVERRIDE"),
            ('\u{2028}', "LINE SEPARATOR"),
            ('\u{2029}', "PARAGRAPH SEPARATOR"),
            ('\u{2060}', "WORD JOINER"),
            ('\u{2061}', "FUNCTION APPLICATION"),
            ('\u{2062}', "INVISIBLE TIMES"),
            ('\u{2063}', "INVISIBLE SEPARATOR"),
            ('\u{2064}', "INVISIBLE PLUS"),
            ('\u{2066}', "LEFT-TO-RIGHT ISOLATE"),
            ('\u{2067}', "RIGHT-TO-LEFT ISOLATE"),
            ('\u{2068}', "FIRST STRONG ISOLATE"),
            ('\u{2069}', "POP DIRECTIONAL ISOLATE"),
            ('\u{FEFF}', "ZERO WIDTH NO-BREAK SPACE"),
            ('\u{FFF9}', "INTERLINEAR ANNOTATION ANCHOR"),
            ('\u{FFFA}', "INTERLINEAR ANNOTATION SEPARATOR"),
            ('\u{FFFB}', "INTERLINEAR ANNOTATION TERMINATOR"),
            ('\u{FFFD}', "REPLACEMENT CHARACTER"),
        ];
        for (c, expected) in cases {
            assert_eq!(
                ctrl_char_name(*c),
                *expected,
                "wrong name for U+{:04X}",
                *c as u32,
            );
        }
    }

    #[test]
    fn test_ctrl_char_name_c0_fallback_for_unnamed_controls() {
        for cp in [0x01_u32, 0x05, 0x10, 0x14, 0x1F] {
            let c = char::from_u32(cp).expect("valid char");
            assert_eq!(ctrl_char_name(c), "C0 CONTROL", "U+{cp:04X}");
        }
    }

    #[test]
    fn test_ctrl_char_name_c1_fallback() {
        for cp in [0x80_u32, 0x90, 0x9F] {
            let c = char::from_u32(cp).expect("valid char");
            assert_eq!(ctrl_char_name(c), "C1 CONTROL", "U+{cp:04X}");
        }
    }

    #[test]
    fn test_ctrl_char_name_default_fallback_for_out_of_range_input() {
        // The `_ =>` arm is defensive — only reachable if a caller bypasses
        // `is_ctrl_char`. The label still has to be sensible.
        assert_eq!(ctrl_char_name('a'), "FORMAT CHARACTER");
        // 0x2065 sits between two named ranges in `is_ctrl_char` and should
        // therefore not be flagged, but the name function still returns the
        // generic fallback if asked.
        assert_eq!(ctrl_char_name('\u{2065}'), "FORMAT CHARACTER");
    }

    #[test]
    fn test_lre_rle_pdf_flagged_in_ltr_locale() {
        let diags = check_with_lang(
            "de",
            "msgid \"x\"\nmsgstr \"x\u{202A}y\u{202B}z\u{202C}\"\n",
        );
        assert_eq!(diags.len(), 3);
        assert!(
            diags[0]
                .message
                .contains("U+202A (LEFT-TO-RIGHT EMBEDDING)")
        );
        assert!(
            diags[1]
                .message
                .contains("U+202B (RIGHT-TO-LEFT EMBEDDING)")
        );
        assert!(
            diags[2]
                .message
                .contains("U+202C (POP DIRECTIONAL FORMATTING)")
        );
    }

    #[test]
    fn test_lro_flagged_in_ltr_locale() {
        // U+202D LRO is always flagged regardless of locale; cover the LTR path
        // explicitly so its name arm is exercised end-to-end.
        let diags = check_with_lang("en", "msgid \"x\"\nmsgstr \"x\u{202D}y\"\n");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("U+202D (LEFT-TO-RIGHT OVERRIDE)"));
    }

    #[test]
    fn test_rlm_flagged_in_ltr_locale() {
        let diags = check_with_lang("en", "msgid \"x\"\nmsgstr \"x\u{200F}y\"\n");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("U+200F (RIGHT-TO-LEFT MARK)"));
    }

    #[test]
    fn test_line_and_paragraph_separators_flagged() {
        let diags = check_with_lang("de", "msgid \"x\"\nmsgstr \"x\u{2028}y\u{2029}z\"\n");
        assert_eq!(diags.len(), 2);
        assert!(diags[0].message.contains("U+2028 (LINE SEPARATOR)"));
        assert!(diags[1].message.contains("U+2029 (PARAGRAPH SEPARATOR)"));
    }

    #[test]
    fn test_word_joiner_and_invisible_operators_flagged() {
        let diags = check_with_lang(
            "de",
            "msgid \"x\"\nmsgstr \"x\u{2060}\u{2061}\u{2062}\u{2063}\u{2064}\"\n",
        );
        assert_eq!(diags.len(), 5);
        let messages: Vec<&str> = diags.iter().map(|d| d.message.as_ref()).collect();
        assert!(messages.iter().any(|m| m.contains("U+2060 (WORD JOINER)")));
        assert!(
            messages
                .iter()
                .any(|m| m.contains("U+2061 (FUNCTION APPLICATION)"))
        );
        assert!(
            messages
                .iter()
                .any(|m| m.contains("U+2062 (INVISIBLE TIMES)"))
        );
        assert!(
            messages
                .iter()
                .any(|m| m.contains("U+2063 (INVISIBLE SEPARATOR)"))
        );
        assert!(
            messages
                .iter()
                .any(|m| m.contains("U+2064 (INVISIBLE PLUS)"))
        );
    }

    #[test]
    fn test_bidi_isolates_flagged_in_ltr_locale() {
        let diags = check_with_lang(
            "de",
            "msgid \"x\"\nmsgstr \"x\u{2066}\u{2067}\u{2068}\u{2069}\"\n",
        );
        assert_eq!(diags.len(), 4);
        assert!(diags[0].message.contains("U+2066 (LEFT-TO-RIGHT ISOLATE)"));
        assert!(diags[1].message.contains("U+2067 (RIGHT-TO-LEFT ISOLATE)"));
        assert!(diags[2].message.contains("U+2068 (FIRST STRONG ISOLATE)"));
        assert!(
            diags[3]
                .message
                .contains("U+2069 (POP DIRECTIONAL ISOLATE)")
        );
    }

    #[test]
    fn test_bidi_isolates_ignored_for_rtl() {
        let diags = check_with_lang(
            "ar",
            "msgid \"x\"\nmsgstr \"x\u{2066}\u{2067}\u{2068}\u{2069}\"\n",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_interlinear_annotations_flagged() {
        let diags = check_with_lang(
            "de",
            "msgid \"x\"\nmsgstr \"x\u{FFF9}a\u{FFFA}b\u{FFFB}\"\n",
        );
        assert_eq!(diags.len(), 3);
        assert!(
            diags[0]
                .message
                .contains("U+FFF9 (INTERLINEAR ANNOTATION ANCHOR)")
        );
        assert!(
            diags[1]
                .message
                .contains("U+FFFA (INTERLINEAR ANNOTATION SEPARATOR)")
        );
        assert!(
            diags[2]
                .message
                .contains("U+FFFB (INTERLINEAR ANNOTATION TERMINATOR)")
        );
    }

    #[test]
    fn test_null_byte_in_translation_is_flagged() {
        // NULL truncates C strings — gettext consumers may silently lose the
        // tail of the translation. Real correctness bug, must be surfaced.
        let diags = check_with_lang("de", "msgid \"x\"\nmsgstr \"a\u{0000}b\"\n");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("U+0000 (NULL)"));
        assert_eq!(diags[0].severity, Severity::Error);
    }

    #[test]
    fn test_ctrl_char_severity() {
        // NULL byte is the only code point promoted to error.
        assert_eq!(ctrl_char_severity('\u{0000}'), Severity::Error);
        assert_eq!(ctrl_char_severity('\u{0007}'), Severity::Warning);
        assert_eq!(ctrl_char_severity('\u{200B}'), Severity::Warning);
        assert_eq!(ctrl_char_severity('\u{202E}'), Severity::Warning);
        assert_eq!(ctrl_char_severity('\u{FEFF}'), Severity::Warning);
        assert_eq!(ctrl_char_severity('\u{FFFD}'), Severity::Warning);
    }

    #[test]
    fn test_named_c0_controls_flagged() {
        // BELL, BACKSPACE, VT, FF — each has an explicit name arm.
        let diags = check_with_lang(
            "de",
            "msgid \"x\"\nmsgstr \"x\u{0007}y\u{0008}z\u{000B}w\u{000C}\"\n",
        );
        assert_eq!(diags.len(), 4);
        let messages: Vec<&str> = diags.iter().map(|d| d.message.as_ref()).collect();
        assert!(messages.iter().any(|m| m.contains("U+0007 (BELL)")));
        assert!(messages.iter().any(|m| m.contains("U+0008 (BACKSPACE)")));
        assert!(messages.iter().any(|m| m.contains("U+000B (VERTICAL TAB)")));
        assert!(messages.iter().any(|m| m.contains("U+000C (FORM FEED)")));
    }

    #[test]
    fn test_unnamed_c0_control_falls_through_to_generic_label() {
        // 0x01 (SOH) is flagged but has no individual name in this rule.
        let diags = check_with_lang("de", "msgid \"x\"\nmsgstr \"x\u{0001}y\"\n");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("U+0001 (C0 CONTROL)"));
    }

    #[test]
    fn test_del_and_c1_controls_flagged() {
        let diags = check_with_lang(
            "de",
            "msgid \"x\"\nmsgstr \"x\u{007F}y\u{0080}z\u{009F}\"\n",
        );
        assert_eq!(diags.len(), 3);
        let messages: Vec<&str> = diags.iter().map(|d| d.message.as_ref()).collect();
        assert!(messages.iter().any(|m| m.contains("U+007F (DELETE)")));
        assert!(messages.iter().any(|m| m.contains("U+0080 (C1 CONTROL)")));
        assert!(messages.iter().any(|m| m.contains("U+009F (C1 CONTROL)")));
    }

    #[test]
    fn test_fix_attached_for_single_stray_char() {
        // msgstr is "Sa\u{200B}ve". ZWSP is at byte offset 2 and is 3 bytes (UTF-8).
        let diags = check("msgid \"Save\"\nmsgstr \"Sa\u{200B}ve\"\n");
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].range, 2..5);
        assert_eq!(fix.edits[0].replacement, "");
    }

    #[test]
    fn test_fix_has_one_edit_per_occurrence() {
        // Two ZWSPs in the translation: one diagnostic, two deletion edits.
        let diags = check("msgid \"abc\"\nmsgstr \"a\u{200B}b\u{200B}c\"\n");
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 2);
        // Edits are listed in document order; byte ranges cover each ZWSP.
        assert_eq!(fix.edits[0].range, 1..4);
        assert_eq!(fix.edits[0].replacement, "");
        assert_eq!(fix.edits[1].range, 5..8);
        assert_eq!(fix.edits[1].replacement, "");
    }

    #[test]
    fn test_fix_distinct_chars_each_have_own_diagnostic() {
        // Three different stray chars → three diagnostics, each with its own fix.
        let diags = check("msgid \"a\"\nmsgstr \"a\u{200B}\u{202E}\u{00AD}\"\n");
        assert_eq!(diags.len(), 3);
        for d in &diags {
            let fix = d.fix.as_ref().expect("fix on every diag");
            assert_eq!(fix.edits.len(), 1);
            assert_eq!(fix.edits[0].replacement, "");
        }
    }

    #[test]
    fn test_is_ctrl_char_predicate() {
        // Flagged.
        assert!(is_ctrl_char('\u{0000}'));
        assert!(is_ctrl_char('\u{0007}'));
        assert!(is_ctrl_char('\u{001B}'));
        assert!(is_ctrl_char('\u{007F}'));
        assert!(is_ctrl_char('\u{00AD}'));
        assert!(is_ctrl_char('\u{200B}'));
        assert!(is_ctrl_char('\u{200E}'));
        assert!(is_ctrl_char('\u{202E}'));
        assert!(is_ctrl_char('\u{2028}'));
        assert!(is_ctrl_char('\u{2060}'));
        assert!(is_ctrl_char('\u{FEFF}'));
        assert!(is_ctrl_char('\u{FFFD}'));
        // Not flagged: visible whitespace and Zs spaces.
        assert!(!is_ctrl_char('\t'));
        assert!(!is_ctrl_char('\n'));
        assert!(!is_ctrl_char('\r'));
        assert!(!is_ctrl_char(' '));
        assert!(!is_ctrl_char('\u{00A0}')); // NBSP
        assert!(!is_ctrl_char('\u{202F}')); // NNBSP
        assert!(!is_ctrl_char('\u{2009}')); // THIN SPACE
        // Not flagged: variation selectors.
        assert!(!is_ctrl_char('\u{FE0F}'));
        // Not flagged: ordinary letters.
        assert!(!is_ctrl_char('a'));
        assert!(!is_ctrl_char('é'));
        assert!(!is_ctrl_char('\u{D55C}')); // Hangul Syllable Han
    }
}
