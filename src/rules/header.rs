// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `header` rule: check that the PO file header
//! contains all the required fields (`Project-Id-Version`, `Language`,
//! `Content-Type`, …) and that their values are well-formed.

use std::collections::HashSet;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Edit, Fix, FixTarget};
use crate::po::entry::Entry;
use crate::po::format::iter::{FormatEmailPos, FormatUrlPos};
use crate::po::format::language::Language;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

/// Fields that must be present in the PO file header, with the severity to
/// emit when they're missing. `Language` is mandatory for any gettext consumer
/// to pick the right translation (error); `Content-Type` and
/// `Content-Transfer-Encoding` are encoding-critical (warning); the rest are
/// informational metadata.
///
/// Order is the canonical display order; diagnostics are emitted in this
/// order for stable output.
const REQUIRED_FIELDS: &[(&str, Severity)] = &[
    ("Project-Id-Version", Severity::Info),
    ("Report-Msgid-Bugs-To", Severity::Info),
    ("POT-Creation-Date", Severity::Info),
    ("PO-Revision-Date", Severity::Info),
    ("Last-Translator", Severity::Info),
    ("Language", Severity::Error),
    ("Language-Team", Severity::Info),
    ("Content-Type", Severity::Warning),
    ("Content-Transfer-Encoding", Severity::Warning),
];

/// Default value to use when auto-fixing a missing header field. Only fields
/// whose value is universally safe (any gettext consumer accepts them and the
/// value matches the parser's implicit default) appear here; the other
/// required fields (language, contact info, dates, version) cannot be
/// inferred and must be filled in by the translator.
fn default_value_for_fix(field: &str) -> Option<&'static str> {
    match field {
        "Content-Type" => Some("text/plain; charset=UTF-8"),
        "Content-Transfer-Encoding" => Some("8bit"),
        _ => None,
    }
}

/// Build a `FixTarget::Msgstr` fix that appends `"<field>: <value>\n"` to
/// the end of the header `msgstr` value. A leading `\n` is added when the
/// existing value is non-empty and does not already end with a newline,
/// so two fields never collide on the same line.
fn append_header_field_fix(msgstr: &Message, field: &str, value: &str) -> Fix {
    let end = msgstr.value.len();
    let needs_separator = !msgstr.value.is_empty() && !msgstr.value.ends_with('\n');
    let replacement = if needs_separator {
        format!("\n{field}: {value}\n")
    } else {
        format!("{field}: {value}\n")
    };
    Fix {
        target: FixTarget::Msgstr {
            file_byte_range: msgstr.byte_range.clone(),
        },
        edits: vec![Edit {
            range: end..end,
            replacement,
        }],
    }
}

pub struct HeaderRule;

impl RuleChecker for HeaderRule {
    fn name(&self) -> &'static str {
        "header"
    }

    fn description(&self) -> &'static str {
        "Missing required fields or invalid field values in PO file header."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check the PO file header for invalid or missing required fields.
    ///
    /// Field matching is case-insensitive (per RFC 822, which the gettext
    /// header format follows) and tolerates surrounding whitespace.
    ///
    /// Wrong header (empty):
    /// ```text
    /// msgid ""
    /// msgstr ""
    /// ```
    ///
    /// Correct header:
    /// ```text
    /// msgid ""
    /// msgstr ""
    /// "Project-Id-Version: poexam\n"
    /// "Report-Msgid-Bugs-To: flashcode@flashtux.org\n"
    /// "POT-Creation-Date: 2026-02-01 18:12:08+0100\n"
    /// "PO-Revision-Date: 2026-02-01 18:12:08+0100\n"
    /// "Last-Translator: Sébastien Helleu <flashcode@flashtux.org>\n"
    /// "Language-Team: Sébastien Helleu <flashcode@flashtux.org>\n"
    /// "Language: fr\n"
    /// "MIME-Version: 1.0\n"
    /// "Content-Type: text/plain; charset=UTF-8\n"
    /// "Content-Transfer-Encoding: 8bit\n"
    /// "Plural-Forms: nplurals=2; plural=(n > 1);\n"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`error`](Severity::Error): `missing field 'Language' in header`
    /// - [`error`](Severity::Error): `invalid value '…' for field 'Content-Type' in header`
    /// - [`error`](Severity::Error): `invalid value '…' for field 'Plural-Forms' in header`
    /// - [`error`](Severity::Error): `invalid value '…' for field 'Language' in header`
    /// - [`warning`](Severity::Warning): `missing field 'Content-Type' in header` (auto-fixable)
    /// - [`warning`](Severity::Warning): `missing field 'Content-Transfer-Encoding' in header` (auto-fixable)
    /// - [`info`](Severity::Info): `missing field '…' in header` (for any other required field)
    /// - [`info`](Severity::Info): `invalid value '…' for field 'Report-Msgid-Bugs-To' in header`
    /// - [`info`](Severity::Info): `invalid value '…' for field 'Last-Translator' in header`
    /// - [`info`](Severity::Info): `invalid value '…' for field 'Language-Team' in header`
    ///
    /// Only the two missing-field diagnostics for `Content-Type` and
    /// `Content-Transfer-Encoding` are auto-fixable: the fix appends the
    /// canonical default value (`text/plain; charset=UTF-8` and `8bit`
    /// respectively). Every other diagnostic either depends on translator
    /// knowledge (language, contacts, dates, project version) or on the
    /// actual file encoding, so no safe default exists.
    #[allow(clippy::too_many_lines)]
    fn check_header(&self, checker: &Checker, _entry: &Entry, msgstr: &Message) -> Vec<Diagnostic> {
        let fields: Vec<(String, &str)> = msgstr
            .value
            .split('\n')
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim()))
            .collect();
        let present: HashSet<&str> = fields.iter().map(|(name, _)| name.as_str()).collect();

        let mut diagnostics: Vec<Diagnostic> = REQUIRED_FIELDS
            .iter()
            .filter(|(field, _)| !present.contains(field.to_ascii_lowercase().as_str()))
            .filter_map(|(field, severity)| {
                let fix = default_value_for_fix(field)
                    .map(|value| append_header_field_fix(msgstr, field, value));
                self.new_diag(
                    checker,
                    *severity,
                    format!("missing field '{field}' in header"),
                )
                .map(|d| {
                    let d = d.with_msg(msgstr);
                    if let Some(fix) = fix {
                        d.with_fix(fix)
                    } else {
                        d
                    }
                })
            })
            .collect();

        if let Some((_, value)) = fields.iter().find(|(name, _)| name == "content-type")
            && !is_valid_content_type(value)
        {
            diagnostics.extend(
                self.new_diag(
                    checker,
                    Severity::Error,
                    format!("invalid value '{value}' for field 'Content-Type' in header"),
                )
                .map(|d| d.with_msg(msgstr)),
            );
        }

        if let Some((_, value)) = fields.iter().find(|(name, _)| name == "plural-forms")
            && !is_valid_plural_forms(value)
        {
            diagnostics.extend(
                self.new_diag(
                    checker,
                    Severity::Error,
                    format!("invalid value '{value}' for field 'Plural-Forms' in header"),
                )
                .map(|d| d.with_msg(msgstr)),
            );
        }

        if let Some((_, value)) = fields.iter().find(|(name, _)| name == "language")
            && !is_valid_language(value)
        {
            diagnostics.extend(
                self.new_diag(
                    checker,
                    Severity::Error,
                    format!("invalid value '{value}' for field 'Language' in header"),
                )
                .map(|d| d.with_msg(msgstr)),
            );
        }

        if let Some((_, value)) = fields
            .iter()
            .find(|(name, _)| name == "report-msgid-bugs-to")
            && !is_valid_report_msgid_bugs_to(value)
        {
            diagnostics.extend(
                self.new_diag(
                    checker,
                    Severity::Info,
                    format!("invalid value '{value}' for field 'Report-Msgid-Bugs-To' in header"),
                )
                .map(|d| d.with_msg(msgstr)),
            );
        }

        if let Some((_, value)) = fields.iter().find(|(name, _)| name == "last-translator")
            && !is_valid_last_translator(value)
        {
            diagnostics.extend(
                self.new_diag(
                    checker,
                    Severity::Info,
                    format!("invalid value '{value}' for field 'Last-Translator' in header"),
                )
                .map(|d| d.with_msg(msgstr)),
            );
        }

        if let Some((_, value)) = fields.iter().find(|(name, _)| name == "language-team")
            && !is_valid_language_team(value)
        {
            diagnostics.extend(
                self.new_diag(
                    checker,
                    Severity::Info,
                    format!("invalid value '{value}' for field 'Language-Team' in header"),
                )
                .map(|d| d.with_msg(msgstr)),
            );
        }

        diagnostics
    }
}

/// Validate a `Language` header value against the gettext spec, which accepts
/// three forms:
/// - `ll` — ISO 639 two- or three-letter lowercase language code
/// - `ll_CC` — language code, `_`, ISO 3166 two-letter uppercase country code
/// - `ll_CC@variant` — `ll_CC`, `@`, lowercase variant designator
///
/// Only structural validation is performed (case and length); the actual ISO
/// code lists are not consulted.
fn is_valid_language(value: &str) -> bool {
    let (lang_country, variant) = match value.split_once('@') {
        Some((lc, v)) => (lc, Some(v)),
        None => (value, None),
    };

    if let Some(v) = variant
        && (v.is_empty() || !v.chars().all(|c| c.is_ascii_lowercase()))
    {
        return false;
    }

    let (lang, country) = match lang_country.split_once('_') {
        Some((l, c)) => (l, Some(c)),
        None => (lang_country, None),
    };

    if variant.is_some() && country.is_none() {
        return false;
    }

    if !matches!(lang.len(), 2 | 3) || !lang.chars().all(|c| c.is_ascii_lowercase()) {
        return false;
    }

    if let Some(c) = country
        && (c.len() != 2 || !c.chars().all(|ch| ch.is_ascii_uppercase()))
    {
        return false;
    }

    true
}

/// Validate a `Report-Msgid-Bugs-To` header value: it must contain exactly one email address.
fn is_valid_report_msgid_bugs_to(value: &str) -> bool {
    count_emails(value) == 1
}

/// Validate a `Last-Translator` header value: it must contain exactly one email address.
fn is_valid_last_translator(value: &str) -> bool {
    count_emails(value) == 1
}

/// Validate a `Language-Team` header value: it must contain exactly one
/// contact, either an email address **or** an HTTP(S) URL. Having both is
/// rejected, as is having neither or duplicates of either.
fn is_valid_language_team(value: &str) -> bool {
    matches!(
        (
            count_emails(value),
            FormatUrlPos::new(value, Language::Null).count()
        ),
        (1, 0) | (0, 1),
    )
}

/// Count the email addresses in a header field value.
///
/// The value is first normalized to take care of obfuscated email address like
/// "user AT domain DOT com".
fn count_emails(value: &str) -> usize {
    let normalized = value
        .replace(" at ", "@")
        .replace(" AT ", "@")
        .replace(" dot ", ".")
        .replace(" DOT ", ".");
    FormatEmailPos::new(&normalized, Language::Null).count()
}

/// Validate a `Content-Type` header value. The value must be of the form
/// `text/plain; charset=<name>`, where `<name>` is a charset that
/// [`encoding_rs`] recognises. The MIME type is matched case-insensitively per
/// RFC 2045; whitespace around `;` and `=` is tolerated.
fn is_valid_content_type(value: &str) -> bool {
    let Some((mime_type, params)) = value.split_once(';') else {
        return false;
    };
    if !mime_type.trim().eq_ignore_ascii_case("text/plain") {
        return false;
    }
    let charset = params.split(';').find_map(|param| {
        let (key, val) = param.split_once('=')?;
        key.trim()
            .eq_ignore_ascii_case("charset")
            .then(|| val.trim())
    });
    charset.is_some_and(|c| encoding_rs::Encoding::for_label(c.as_bytes()).is_some())
}

/// Validate the `nplurals=N` part of a `Plural-Forms` header value: `N` must
/// be a positive integer. The `plural=EXPRESSION` part is not validated.
fn is_valid_plural_forms(value: &str) -> bool {
    let Some(n) = value.split(';').find_map(|param| {
        let (key, val) = param.split_once('=')?;
        key.trim()
            .eq_ignore_ascii_case("nplurals")
            .then(|| val.trim())
    }) else {
        return false;
    };
    matches!(n.parse::<u32>(), Ok(n) if n >= 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    /// A header containing every required field.
    const COMPLETE_HEADER: &str = "msgid \"\"
msgstr \"\"
\"Project-Id-Version: poexam\\n\"
\"Report-Msgid-Bugs-To: flashcode@flashtux.org\\n\"
\"POT-Creation-Date: 2026-02-01 18:12:08+0100\\n\"
\"PO-Revision-Date: 2026-02-01 18:12:08+0100\\n\"
\"Last-Translator: Sébastien Helleu <flashcode@flashtux.org>\\n\"
\"Language-Team: French <translators-fr@example.com>\\n\"
\"Language: fr\\n\"
\"MIME-Version: 1.0\\n\"
\"Content-Type: text/plain; charset=UTF-8\\n\"
\"Content-Transfer-Encoding: 8bit\\n\"
\"Plural-Forms: nplurals=2; plural=(n > 1);\\n\"
";

    fn check(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(HeaderRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_complete_header_is_silent() {
        let diags = check(COMPLETE_HEADER);
        assert!(diags.is_empty(), "got unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn test_empty_header_reports_every_required_field() {
        let diags = check("msgid \"\"\nmsgstr \"\"\n");
        assert_eq!(diags.len(), REQUIRED_FIELDS.len());
        for (d, (_, expected_severity)) in diags.iter().zip(REQUIRED_FIELDS.iter()) {
            assert_eq!(d.severity, *expected_severity);
            assert!(d.message.starts_with("missing field '"));
            assert!(d.message.ends_with("' in header"));
        }
    }

    #[test]
    fn test_diagnostics_emitted_in_canonical_order() {
        let diags = check("msgid \"\"\nmsgstr \"\"\n");
        let messages: Vec<&str> = diags.iter().map(|d| d.message.as_ref()).collect();
        for (idx, (field, _)) in REQUIRED_FIELDS.iter().enumerate() {
            assert!(
                messages[idx].contains(&format!("'{field}'")),
                "expected diag #{idx} to mention '{field}', got: {}",
                messages[idx]
            );
        }
    }

    #[test]
    fn test_single_missing_field_is_reported_alone() {
        // Language is mandatory for gettext to pick a translation → error.
        let header = COMPLETE_HEADER.replace("\"Language: fr\\n\"\n", "");
        let diags = check(&header);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "missing field 'Language' in header");
        assert_eq!(diags[0].severity, Severity::Error);
    }

    #[test]
    fn test_missing_content_type_is_warning() {
        // Content-Type is encoding-critical → warning.
        let header =
            COMPLETE_HEADER.replace("\"Content-Type: text/plain; charset=UTF-8\\n\"\n", "");
        let diags = check(&header);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "missing field 'Content-Type' in header");
        assert_eq!(diags[0].severity, Severity::Warning);
    }

    #[test]
    fn test_missing_content_transfer_encoding_is_warning() {
        let header = COMPLETE_HEADER.replace("\"Content-Transfer-Encoding: 8bit\\n\"\n", "");
        let diags = check(&header);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "missing field 'Content-Transfer-Encoding' in header"
        );
        assert_eq!(diags[0].severity, Severity::Warning);
    }

    #[test]
    fn test_two_missing_fields() {
        let header = COMPLETE_HEADER
            .replace("\"Language: fr\\n\"\n", "")
            .replace(
                "\"Language-Team: French <translators-fr@example.com>\\n\"\n",
                "",
            );
        let diags = check(&header);
        assert_eq!(diags.len(), 2);
        // Order in REQUIRED_FIELDS: Language then Language-Team.
        assert!(diags[0].message.contains("'Language'"));
        assert!(diags[1].message.contains("'Language-Team'"));
    }

    #[test]
    fn test_field_match_is_case_insensitive() {
        let header = COMPLETE_HEADER.replace("\"Language:", "\"language:");
        let diags = check(&header);
        assert!(
            !diags.iter().any(|d| d.message.contains("'Language'")),
            "lowercase 'language:' should still match the Language field"
        );
        let header = COMPLETE_HEADER.replace("\"Content-Type:", "\"CONTENT-TYPE:");
        let diags = check(&header);
        assert!(
            !diags.iter().any(|d| d.message.contains("'Content-Type'")),
            "ALL CAPS 'CONTENT-TYPE:' should still match"
        );
    }

    #[test]
    fn test_whitespace_around_field_is_tolerated() {
        let header = COMPLETE_HEADER.replace("\"Language: fr", "\"  Language  : fr");
        let diags = check(&header);
        assert!(
            !diags.iter().any(|d| d.message.contains("'Language'")),
            "whitespace-padded 'Language' should still match"
        );
    }

    #[test]
    fn test_diagnostic_includes_header_msgstr_as_context() {
        let diags = check("msgid \"\"\nmsgstr \"\"\n\"Language: fr\\n\"\n");
        let first = &diags[0];
        assert!(
            !first.lines.is_empty(),
            "diagnostic should include the header msgstr as context"
        );
    }

    #[test]
    fn test_noqa_per_rule_suppresses_diagnostic() {
        let diags = check("#, noqa:header\nmsgid \"\"\nmsgstr \"\"\n");
        assert!(
            diags.is_empty(),
            "`noqa:header` on the header entry should suppress all diagnostics, got: {diags:?}"
        );
    }

    #[test]
    fn test_global_noqa_suppresses_diagnostic() {
        let diags = check("#, noqa\nmsgid \"\"\nmsgstr \"\"\n");
        assert!(
            diags.is_empty(),
            "global `noqa` on the header entry should suppress all diagnostics"
        );
    }

    fn check_language(value: &str) -> Vec<Diagnostic> {
        let header =
            COMPLETE_HEADER.replace("\"Language: fr\\n\"", &format!("\"Language: {value}\\n\""));
        check(&header)
    }

    #[test]
    fn test_language_two_letter_code_is_valid() {
        assert!(check_language("fr").is_empty());
        assert!(check_language("en").is_empty());
        assert!(check_language("de").is_empty());
    }

    #[test]
    fn test_language_three_letter_code_is_valid() {
        assert!(check_language("haw").is_empty());
        assert!(check_language("ast").is_empty());
    }

    #[test]
    fn test_language_with_country_is_valid() {
        assert!(check_language("pt_BR").is_empty());
        assert!(check_language("de_AT").is_empty());
        assert!(check_language("en_US").is_empty());
    }

    #[test]
    fn test_language_with_variant_is_valid() {
        assert!(check_language("sr_RS@latin").is_empty());
        assert!(check_language("ca_ES@valencia").is_empty());
    }

    #[test]
    fn test_language_uppercase_is_invalid() {
        let diags = check_language("FR");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "invalid value 'FR' for field 'Language' in header"
        );
        assert_eq!(diags[0].severity, Severity::Error);
    }

    #[test]
    fn test_language_too_long_is_invalid() {
        let diags = check_language("fren");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("'fren'"));
    }

    #[test]
    fn test_language_single_letter_is_invalid() {
        let diags = check_language("f");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("'f'"));
    }

    #[test]
    fn test_language_country_lowercase_is_invalid() {
        let diags = check_language("fr_fr");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("'fr_fr'"));
    }

    #[test]
    fn test_language_country_three_letter_is_invalid() {
        let diags = check_language("en_USA");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("'en_USA'"));
    }

    #[test]
    fn test_language_variant_without_country_is_invalid() {
        let diags = check_language("sr@latin");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("'sr@latin'"));
    }

    #[test]
    fn test_language_uppercase_variant_is_invalid() {
        let diags = check_language("sr_RS@LATIN");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("'sr_RS@LATIN'"));
    }

    #[test]
    fn test_language_empty_variant_is_invalid() {
        let diags = check_language("sr_RS@");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("'sr_RS@'"));
    }

    #[test]
    fn test_language_empty_value_is_invalid() {
        let diags = check_language("");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "invalid value '' for field 'Language' in header"
        );
    }

    #[test]
    fn test_language_with_digits_is_invalid() {
        let diags = check_language("fr2");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("'fr2'"));
    }

    fn check_content_type(value: &str) -> Vec<Diagnostic> {
        let header = COMPLETE_HEADER.replace(
            "\"Content-Type: text/plain; charset=UTF-8\\n\"",
            &format!("\"Content-Type: {value}\\n\""),
        );
        check(&header)
    }

    #[test]
    fn test_content_type_utf8_is_valid() {
        assert!(check_content_type("text/plain; charset=UTF-8").is_empty());
    }

    #[test]
    fn test_content_type_iso_8859_15_is_valid() {
        assert!(check_content_type("text/plain; charset=ISO-8859-15").is_empty());
    }

    #[test]
    fn test_content_type_charset_is_case_insensitive() {
        // Encoding labels are case-insensitive per the WHATWG encoding standard.
        assert!(check_content_type("text/plain; charset=utf-8").is_empty());
    }

    #[test]
    fn test_content_type_extra_whitespace_is_tolerated() {
        assert!(check_content_type("text/plain ;  charset = UTF-8").is_empty());
    }

    #[test]
    fn test_content_type_mime_case_insensitive() {
        assert!(check_content_type("Text/Plain; charset=UTF-8").is_empty());
    }

    #[test]
    fn test_content_type_wrong_mime_type_is_invalid() {
        let diags = check_content_type("text/html; charset=UTF-8");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "invalid value 'text/html; charset=UTF-8' for field 'Content-Type' in header"
        );
        assert_eq!(diags[0].severity, Severity::Error);
    }

    #[test]
    fn test_content_type_missing_charset_is_invalid() {
        let diags = check_content_type("text/plain");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("'text/plain'"));
    }

    #[test]
    fn test_content_type_missing_charset_value_is_invalid() {
        let diags = check_content_type("text/plain; charset=");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("'text/plain; charset='"));
    }

    #[test]
    fn test_content_type_unknown_charset_is_invalid() {
        let diags = check_content_type("text/plain; charset=does-not-exist");
        assert_eq!(diags.len(), 1);
        assert!(
            diags[0]
                .message
                .contains("'text/plain; charset=does-not-exist'")
        );
    }

    #[test]
    fn test_content_type_empty_value_is_invalid() {
        let diags = check_content_type("");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "invalid value '' for field 'Content-Type' in header"
        );
    }

    #[test]
    fn test_content_type_other_param_before_charset_is_valid() {
        // RFC 2045 allows multiple parameters in any order.
        assert!(check_content_type("text/plain; format=flowed; charset=UTF-8").is_empty());
    }

    fn check_report_msgid_bugs_to(value: &str) -> Vec<Diagnostic> {
        let header = COMPLETE_HEADER.replace(
            "\"Report-Msgid-Bugs-To: flashcode@flashtux.org\\n\"",
            &format!("\"Report-Msgid-Bugs-To: {value}\\n\""),
        );
        check(&header)
    }

    #[test]
    fn test_report_msgid_bugs_to_bare_email_is_valid() {
        assert!(check_report_msgid_bugs_to("bugs@example.org").is_empty());
    }

    #[test]
    fn test_report_msgid_bugs_to_canonical_form_is_valid() {
        assert!(check_report_msgid_bugs_to("Project Bugs <bugs@example.org>").is_empty());
    }

    #[test]
    fn test_report_msgid_bugs_to_no_email_is_invalid() {
        let diags = check_report_msgid_bugs_to("Project Bugs");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "invalid value 'Project Bugs' for field 'Report-Msgid-Bugs-To' in header"
        );
        assert_eq!(diags[0].severity, Severity::Info);
    }

    #[test]
    fn test_report_msgid_bugs_to_empty_value_is_invalid() {
        let diags = check_report_msgid_bugs_to("");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "invalid value '' for field 'Report-Msgid-Bugs-To' in header"
        );
    }

    #[test]
    fn test_report_msgid_bugs_to_two_emails_is_invalid() {
        let diags = check_report_msgid_bugs_to("a@example.org b@example.org");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("'Report-Msgid-Bugs-To'"));
    }

    #[test]
    fn test_report_msgid_bugs_to_url_only_is_invalid() {
        // The user spec says "exactly one email" — a bug-tracker URL alone is not enough.
        let diags = check_report_msgid_bugs_to("https://bugs.example.org/");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("'Report-Msgid-Bugs-To'"));
    }

    fn check_last_translator(value: &str) -> Vec<Diagnostic> {
        let header = COMPLETE_HEADER.replace(
            "\"Last-Translator: Sébastien Helleu <flashcode@flashtux.org>\\n\"",
            &format!("\"Last-Translator: {value}\\n\""),
        );
        check(&header)
    }

    #[test]
    fn test_last_translator_canonical_form_is_valid() {
        assert!(check_last_translator("Sébastien Helleu <flashcode@flashtux.org>").is_empty());
    }

    #[test]
    fn test_last_translator_bare_email_is_valid() {
        assert!(check_last_translator("flashcode@flashtux.org").is_empty());
    }

    #[test]
    fn test_last_translator_no_email_is_invalid() {
        let diags = check_last_translator("Sébastien Helleu");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "invalid value 'Sébastien Helleu' for field 'Last-Translator' in header"
        );
        assert_eq!(diags[0].severity, Severity::Info);
    }

    #[test]
    fn test_last_translator_empty_value_is_invalid() {
        let diags = check_last_translator("");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "invalid value '' for field 'Last-Translator' in header"
        );
    }

    #[test]
    fn test_last_translator_two_emails_is_invalid() {
        let diags = check_last_translator("Foo <foo@example.com> and Bar <bar@example.com>");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("'Last-Translator'"));
    }

    #[test]
    fn test_last_translator_url_only_is_invalid() {
        // A URL is not an email — Last-Translator requires an email specifically.
        let diags = check_last_translator("Sébastien Helleu <https://flashtux.org>");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("'Last-Translator'"));
    }

    fn check_language_team(value: &str) -> Vec<Diagnostic> {
        let header = COMPLETE_HEADER.replace(
            "\"Language-Team: French <translators-fr@example.com>\\n\"",
            &format!("\"Language-Team: {value}\\n\""),
        );
        check(&header)
    }

    #[test]
    fn test_language_team_with_email_is_valid() {
        assert!(check_language_team("French <translators-fr@example.com>").is_empty());
    }

    #[test]
    fn test_language_team_bare_email_is_valid() {
        assert!(check_language_team("translators-fr@example.com").is_empty());
    }

    #[test]
    fn test_language_team_with_url_is_valid() {
        assert!(check_language_team("French <https://example.com/i18n/>").is_empty());
    }

    #[test]
    fn test_language_team_bare_url_is_valid() {
        assert!(check_language_team("https://example.com/i18n/").is_empty());
    }

    #[test]
    fn test_language_team_no_contact_is_invalid() {
        let diags = check_language_team("French");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "invalid value 'French' for field 'Language-Team' in header"
        );
        assert_eq!(diags[0].severity, Severity::Info);
    }

    #[test]
    fn test_language_team_empty_value_is_invalid() {
        let diags = check_language_team("");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "invalid value '' for field 'Language-Team' in header"
        );
    }

    #[test]
    fn test_language_team_email_and_url_is_invalid() {
        // "Either … or …" — the rule rejects having both forms.
        let diags =
            check_language_team("French <translators-fr@example.com> <https://example.com/i18n/>");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("'Language-Team'"));
    }

    #[test]
    fn test_language_team_two_emails_is_invalid() {
        let diags = check_language_team("French <a@example.com> <b@example.com>");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("'Language-Team'"));
    }

    #[test]
    fn test_language_team_two_urls_is_invalid() {
        let diags =
            check_language_team("French <https://example.com/> <https://other.example.org/>");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("'Language-Team'"));
    }

    fn check_plural_forms(value: &str) -> Vec<Diagnostic> {
        let header = COMPLETE_HEADER.replace(
            "\"Plural-Forms: nplurals=2; plural=(n > 1);\\n\"",
            &format!("\"Plural-Forms: {value}\\n\""),
        );
        check(&header)
    }

    #[test]
    fn test_plural_forms_is_optional() {
        // Removing Plural-Forms entirely must not produce a diagnostic.
        let header =
            COMPLETE_HEADER.replace("\"Plural-Forms: nplurals=2; plural=(n > 1);\\n\"\n", "");
        assert!(check(&header).is_empty());
    }

    #[test]
    fn test_plural_forms_simple_is_valid() {
        assert!(check_plural_forms("nplurals=2; plural=(n != 1);").is_empty());
    }

    #[test]
    fn test_plural_forms_single_is_valid() {
        assert!(check_plural_forms("nplurals=1; plural=0;").is_empty());
    }

    #[test]
    fn test_plural_forms_arabic_is_valid() {
        assert!(check_plural_forms("nplurals=6; plural=(n==0 ? 0 : n==1 ? 1 : 2);").is_empty());
    }

    #[test]
    fn test_plural_forms_extra_whitespace_is_tolerated() {
        assert!(check_plural_forms(" nplurals = 3 ; plural=(n != 1);").is_empty());
    }

    #[test]
    fn test_plural_forms_zero_is_invalid() {
        let diags = check_plural_forms("nplurals=0; plural=0;");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "invalid value 'nplurals=0; plural=0;' for field 'Plural-Forms' in header"
        );
        assert_eq!(diags[0].severity, Severity::Error);
    }

    #[test]
    fn test_plural_forms_negative_is_invalid() {
        let diags = check_plural_forms("nplurals=-1; plural=0;");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("'nplurals=-1; plural=0;'"));
    }

    #[test]
    fn test_plural_forms_non_integer_is_invalid() {
        let diags = check_plural_forms("nplurals=abc; plural=0;");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("'nplurals=abc; plural=0;'"));
    }

    #[test]
    fn test_plural_forms_missing_nplurals_is_invalid() {
        let diags = check_plural_forms("plural=(n != 1);");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("'plural=(n != 1);'"));
    }

    #[test]
    fn test_plural_forms_empty_nplurals_is_invalid() {
        let diags = check_plural_forms("nplurals=; plural=0;");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("'nplurals=; plural=0;'"));
    }

    #[test]
    fn test_plural_forms_empty_value_is_invalid() {
        let diags = check_plural_forms("");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "invalid value '' for field 'Plural-Forms' in header"
        );
    }

    fn diag_with_message<'a>(diags: &'a [Diagnostic], message: &str) -> &'a Diagnostic {
        diags
            .iter()
            .find(|d| d.message == message)
            .unwrap_or_else(|| panic!("no diagnostic with message {message:?} in {diags:#?}"))
    }

    #[test]
    fn test_missing_content_type_fix_appends_default() {
        let header =
            COMPLETE_HEADER.replace("\"Content-Type: text/plain; charset=UTF-8\\n\"\n", "");
        let diags = check(&header);
        let d = diag_with_message(&diags, "missing field 'Content-Type' in header");
        let fix = d.fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 1);
        // Inserted at end of the existing msgstr value (the header is non-empty
        // and already ends with `\n`, so no extra separator is needed).
        let edit = &fix.edits[0];
        assert_eq!(edit.range.start, edit.range.end);
        assert_eq!(
            edit.replacement,
            "Content-Type: text/plain; charset=UTF-8\n"
        );
    }

    #[test]
    fn test_missing_content_transfer_encoding_fix_appends_default() {
        let header = COMPLETE_HEADER.replace("\"Content-Transfer-Encoding: 8bit\\n\"\n", "");
        let diags = check(&header);
        let d = diag_with_message(
            &diags,
            "missing field 'Content-Transfer-Encoding' in header",
        );
        let fix = d.fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(
            fix.edits[0].replacement,
            "Content-Transfer-Encoding: 8bit\n"
        );
    }

    #[test]
    fn test_other_missing_field_diagnostics_have_no_fix() {
        // None of the other required fields have a safe default to insert.
        let diags = check("msgid \"\"\nmsgstr \"\"\n");
        for d in &diags {
            if !d.message.contains("'Content-Type'")
                && !d.message.contains("'Content-Transfer-Encoding'")
            {
                assert!(
                    d.fix.is_none(),
                    "expected no fix on diagnostic {:?}",
                    d.message
                );
            }
        }
    }

    #[test]
    fn test_invalid_value_diagnostics_have_no_fix() {
        // The correct replacement depends on per-file context (language,
        // encoding, contact info) so no auto-fix is offered.
        let diags = check_language("FR");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].fix.is_none());

        let diags = check_content_type("text/html; charset=UTF-8");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].fix.is_none());

        let diags = check_plural_forms("nplurals=0; plural=0;");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].fix.is_none());
    }

    #[test]
    fn test_missing_content_type_fix_on_empty_header_works() {
        // Empty header → msgstr value is "" → fix replacement has no
        // leading `\n` and the result is well-formed.
        let diags = check("msgid \"\"\nmsgstr \"\"\n");
        let d = diag_with_message(&diags, "missing field 'Content-Type' in header");
        let fix = d.fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits[0].range, 0..0);
        assert_eq!(
            fix.edits[0].replacement,
            "Content-Type: text/plain; charset=UTF-8\n"
        );
    }
}
