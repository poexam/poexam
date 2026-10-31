// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `max-length` rule: check that the translation does
//! not exceed the maximum length set by the `max-length:N` flag (Weblate flag).

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct MaxLengthRule;

/// Prefix of the flag giving the maximum length of the translation.
const FLAG_MAX_LENGTH: &str = "max-length:";

/// Flag to ignore the maximum length (Weblate flag).
const FLAG_IGNORE: &str = "ignore-max-length";

/// Flag to ignore XML tags when computing the length (Weblate flag).
const FLAG_XML_TEXT: &str = "xml-text";

/// Return the byte position of the end of the XML tag starting at `pos` in
/// `s` (after `>`), or `None` if there is no tag at this position.
///
/// A tag starts with `<` followed by a letter, `/` or `!` (comment).
fn xml_tag_end(s: &str, pos: usize) -> Option<usize> {
    let rest = &s[pos..];
    if !rest.starts_with('<')
        || !rest[1..].starts_with(|c: char| c.is_ascii_alphabetic() || c == '/' || c == '!')
    {
        return None;
    }
    memchr::memchr(b'>', rest.as_bytes()).map(|end| pos + end + 1)
}

/// Return the length of the string (number of Unicode characters, ignoring
/// XML tags if `xml_text` is `true`) and the byte position of the first
/// character beyond the maximum length `max` (if the string is too long).
fn length(s: &str, max: usize, xml_text: bool) -> (usize, Option<usize>) {
    let mut count = 0;
    let mut beyond = None;
    let mut pos = 0;
    while let Some(c) = s[pos..].chars().next() {
        if xml_text && let Some(end) = xml_tag_end(s, pos) {
            pos = end;
            continue;
        }
        if count == max {
            beyond.get_or_insert(pos);
        }
        count += 1;
        pos += c.len_utf8();
    }
    (count, beyond)
}

impl RuleChecker for MaxLengthRule {
    fn name(&self) -> &'static str {
        "max-length"
    }

    fn description(&self) -> &'static str {
        "Check that translation does not exceed the maximum length set by the flag `max-length:N`."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check that the translation does not exceed the maximum length set by the
    /// flag `max-length:N`, compatible with Weblate.
    ///
    /// The length is the number of Unicode characters. With the flag
    /// `xml-text`, the XML tags are not counted. The flag `ignore-max-length`
    /// disables the check for the entry.
    ///
    /// Entries without the flag are not checked. Each plural form is checked.
    ///
    /// Wrong entry:
    /// ```text
    /// #, max-length:10
    /// msgid "Save file"
    /// msgstr "Enregistrer le fichier"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// #, max-length:10
    /// msgid "Save file"
    /// msgstr "Enregistrer"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`warning`](Severity::Warning): `translation too long: # characters (max: #)`
    /// - [`warning`](Severity::Warning): `invalid value '…' for flag 'max-length'`
    fn check_msg(
        &self,
        checker: &Checker,
        entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let mut max_value = None;
        let mut xml_text = false;
        for keyword in &entry.keywords {
            if let Some(value) = keyword.strip_prefix(FLAG_MAX_LENGTH) {
                max_value = Some(value);
            } else if keyword == FLAG_XML_TEXT {
                xml_text = true;
            } else if keyword == FLAG_IGNORE {
                return vec![];
            }
        }
        let Some(max_value) = max_value else {
            return vec![];
        };
        let Some(max) = max_value.parse::<usize>().ok().filter(|max| *max > 0) else {
            return self
                .new_diag(
                    checker,
                    Severity::Warning,
                    format!("invalid value '{max_value}' for flag 'max-length'"),
                )
                .map(|d| d.with_keywords(entry))
                .into_iter()
                .collect();
        };
        let (count, beyond) = length(&msgstr.value, max, xml_text);
        let Some(beyond) = beyond else {
            return vec![];
        };
        self.new_diag(
            checker,
            Severity::Warning,
            format!("translation too long: {count} characters (max: {max})"),
        )
        .map(|d| {
            d.with_keywords(entry)
                .with_msgs_hl(msgid, [], msgstr, [(beyond, msgstr.value.len())])
        })
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
        let rules = Rules::new(vec![Box::new(MaxLengthRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    /// Check an entry with the given flags line and translation.
    fn check_entry(flags: &str, msgstr: &str) -> Vec<Diagnostic> {
        check(&format!(
            "#, {flags}\nmsgid \"Save file\"\nmsgstr \"{msgstr}\"\n"
        ))
    }

    #[test]
    fn test_length() {
        assert_eq!(length("", 3, false), (0, None));
        assert_eq!(length("abc", 3, false), (3, None));
        assert_eq!(length("abcd", 3, false), (4, Some(3)));
        assert_eq!(length("éèàù", 2, false), (4, Some(4)));
        assert_eq!(length("ファイル", 3, false), (4, Some(9)));
        // XML tags.
        assert_eq!(length("<b>abc</b>", 3, false), (10, Some(3)));
        assert_eq!(length("<b>abc</b>", 3, true), (3, None));
        assert_eq!(length("<b>abcd</b>", 3, true), (4, Some(6)));
        assert_eq!(length("a<br/>b<!-- c -->", 2, true), (2, None));
        assert_eq!(length("a < b", 3, true), (5, Some(3)));
        assert_eq!(length("a <b", 3, true), (4, Some(3)));
    }

    #[test]
    fn test_no_flag() {
        let diags = check("msgid \"Save file\"\nmsgstr \"Enregistrer le fichier\"\n");
        assert!(diags.is_empty());
        assert!(check_entry("c-format", "Enregistrer le fichier").is_empty());
    }

    #[test]
    fn test_ok() {
        assert!(check_entry("max-length:11", "Enregistrer").is_empty());
        assert!(check_entry("c-format, max-length:20", "Enregistrer").is_empty());
        assert!(check_entry("max-length:5, xml-text", "<b>Sauve</b>").is_empty());
    }

    #[test]
    fn test_ignore() {
        assert!(check_entry("max-length:5, ignore-max-length", "Enregistrer").is_empty());
        assert!(check_entry("max-length:5, noqa:max-length", "Enregistrer").is_empty());
    }

    #[test]
    fn test_too_long() {
        let diags = check_entry("max-length:10", "Enregistrer le fichier");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Warning);
        assert_eq!(
            diags[0].message,
            "translation too long: 22 characters (max: 10)"
        );
        // Tags are counted without the flag `xml-text`.
        let diags = check_entry("max-length:5", "<b>Sauve</b>");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "translation too long: 12 characters (max: 5)"
        );
    }

    #[test]
    fn test_plurals() {
        let diags = check(
            "#, max-length:10\nmsgid \"%d file\"\nmsgid_plural \"%d files\"\n\
             msgstr[0] \"%d fichier\"\nmsgstr[1] \"%d fichiers\"\n",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "translation too long: 11 characters (max: 10)"
        );
    }

    #[test]
    fn test_invalid_value() {
        for value in ["", "abc", "0", "-5", "1.5"] {
            let diags = check_entry(&format!("max-length:{value}"), "Enregistrer");
            assert_eq!(diags.len(), 1, "value: {value:?}");
            assert_eq!(
                diags[0].message,
                format!("invalid value '{value}' for flag 'max-length'")
            );
        }
    }
}
