// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `header` rule: check that the PO file header
//! contains all the required fields (`Project-Id-Version`, `Language`,
//! `Content-Type`, …).

use std::collections::HashSet;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

/// Fields that must be present in the PO file header. Order is the canonical
/// display order; diagnostics are emitted in this order for stable output.
const REQUIRED_FIELDS: &[&str] = &[
    "Project-Id-Version",
    "Report-Msgid-Bugs-To",
    "POT-Creation-Date",
    "PO-Revision-Date",
    "Last-Translator",
    "Language",
    "Language-Team",
    "Content-Type",
    "Content-Transfer-Encoding",
];

pub struct HeaderRule;

impl RuleChecker for HeaderRule {
    fn name(&self) -> &'static str {
        "header"
    }

    fn description(&self) -> &'static str {
        "Missing required fields in PO file header."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    /// Check the PO file header for missing required fields.
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
    /// Diagnostics reported with severity [`error`](Severity::Error):
    /// - `missing field 'xxx' in header`
    fn check_header(&self, checker: &Checker, _entry: &Entry, msgstr: &Message) -> Vec<Diagnostic> {
        let present: HashSet<String> = msgstr
            .value
            .split('\n')
            .filter_map(|line| line.split_once(':'))
            .map(|(field, _)| field.trim().to_ascii_lowercase())
            .collect();

        REQUIRED_FIELDS
            .iter()
            .filter(|field| !present.contains(&field.to_ascii_lowercase()))
            .map(|field| {
                self.new_diag(checker, format!("missing field '{field}' in header"))
                    .with_msg(msgstr)
            })
            .collect()
    }
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
        for d in &diags {
            assert_eq!(d.severity, Severity::Error);
            assert!(d.message.starts_with("missing field '"));
            assert!(d.message.ends_with("' in header"));
        }
    }

    #[test]
    fn test_diagnostics_emitted_in_canonical_order() {
        let diags = check("msgid \"\"\nmsgstr \"\"\n");
        let messages: Vec<&str> = diags.iter().map(|d| d.message.as_ref()).collect();
        for (idx, field) in REQUIRED_FIELDS.iter().enumerate() {
            assert!(
                messages[idx].contains(&format!("'{field}'")),
                "expected diag #{idx} to mention '{field}', got: {}",
                messages[idx]
            );
        }
    }

    #[test]
    fn test_single_missing_field_is_reported_alone() {
        let header = COMPLETE_HEADER.replace("\"Language: fr\\n\"\n", "");
        let diags = check(&header);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "missing field 'Language' in header");
        assert_eq!(diags[0].severity, Severity::Error);
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
}
