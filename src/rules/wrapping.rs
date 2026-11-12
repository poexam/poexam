// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `wrapping` rule: check that messages are wrapped like
//! GNU `msgcat` would do.

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Fix, FixTarget};
use crate::po::entry::Entry;
use crate::po::format::language::Language;
use crate::po::message::Message;
use crate::po::wrap::{format_message_block, is_single_line_block};
use crate::rules::rule::RuleChecker;

pub struct WrappingRule;

impl WrappingRule {
    /// Check that the messages of the entry are wrapped like GNU `msgcat`
    /// would do.
    fn check_messages(&self, checker: &Checker, entry: &Entry) -> Vec<Diagnostic> {
        // The values are decoded to UTF-8: the bytes of other encodings can not
        // be compared.
        if checker.encoding_name() != encoding_rs::UTF_8.name() {
            return vec![];
        }
        // No line break is allowed inside format string directives: an entry
        // with a format not supported can not be checked.
        if entry.format_language == Language::Null
            && entry
                .keywords
                .iter()
                .any(|k| k.ends_with("-format") && !k.starts_with("no-"))
        {
            return vec![];
        }
        let data = checker.data();
        let width = if entry.nowrap {
            0
        } else {
            checker.config.check.width
        };
        let mut diags = vec![];
        let messages = entry
            .msgctxt
            .iter()
            .chain(entry.msgid.iter())
            .chain(entry.msgid_plural.iter())
            .chain(entry.msgstr.values());
        for msg in messages {
            let original = &data[msg.byte_range.clone()];
            // Fast path for short messages, then skip CRLF line endings.
            if is_single_line_block(original, &msg.value, width) || original.contains(&b'\r') {
                continue;
            }
            let bytes = format_message_block(original, &msg.value, width, entry.format_language);
            if bytes == original {
                continue;
            }
            let message = if width == 0 {
                "message not wrapped like msgcat (no wrap)".to_string()
            } else {
                format!("message not wrapped like msgcat (width: {width})")
            };
            if let Some(diag) = self.new_diag(checker, Severity::Info, message) {
                diags.push(diag.with_msg(msg).with_fix(Fix {
                    target: FixTarget::Block {
                        file_byte_range: msg.byte_range.clone(),
                        bytes,
                    },
                    edits: vec![],
                    safe: true,
                }));
            }
        }
        diags
    }
}

impl RuleChecker for WrappingRule {
    fn name(&self) -> &'static str {
        "wrapping"
    }

    fn description(&self) -> &'static str {
        "Check that messages are wrapped like GNU msgcat would do."
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    fn checks_all_entries(&self) -> bool {
        true
    }

    /// Check that the messages of the header are wrapped like GNU `msgcat`
    /// would do.
    fn check_header(&self, checker: &Checker, entry: &Entry, _msgstr: &Message) -> Vec<Diagnostic> {
        self.check_messages(checker, entry)
    }

    /// Check that messages (`msgctxt`, `msgid`, `msgid_plural` and `msgstr`)
    /// are wrapped like GNU `msgcat` would do, with the page width of option
    /// `--width` (79 by default, 0 to not wrap lines): long lines are split
    /// at the page width, and after each newline. The messages of entries
    /// with the flag `no-wrap` are split only after newlines.
    ///
    /// All entries are checked, including the header, untranslated, fuzzy
    /// and obsolete entries, as `msgcat` formats them all.
    ///
    /// Files not encoded in UTF-8, messages with CRLF line endings and entries
    /// with a format not supported by poexam (e.g. `lisp-format`) are not
    /// checked: no line break is allowed inside format string directives.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "Hello"
    /// msgstr ""
    /// "Bonjour"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "Hello"
    /// msgstr "Bonjour"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`info`](Severity::Info): `message not wrapped like msgcat (width: #)`
    ///   (auto-fixable)
    /// - [`info`](Severity::Info): `message not wrapped like msgcat (no wrap)`
    ///   (auto-fixable)
    fn check_entry(&self, checker: &Checker, entry: &Entry) -> Vec<Diagnostic> {
        self.check_messages(checker, entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::rule::Rules;

    const HEADER: &str = "msgid \"\"\nmsgstr \"Content-Type: text/plain; charset=UTF-8\\n\"\n\n";

    fn check_width(content: &str, width: usize) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        checker.config.check.width = width;
        let rules = Rules::new(vec![Box::new(WrappingRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    fn check(content: &str) -> Vec<Diagnostic> {
        check_width(content, 79)
    }

    /// Return the bytes of the fix of a diagnostic.
    fn fix_bytes(diag: &Diagnostic) -> String {
        match &diag.fix.as_ref().expect("fix").target {
            FixTarget::Block { bytes, .. } => String::from_utf8(bytes.clone()).unwrap(),
            _ => panic!("unexpected fix target"),
        }
    }

    #[test]
    fn test_no_error() {
        let content = format!(
            "{HEADER}msgid \"Hello\"\nmsgstr \"Bonjour\"\n\n\
             msgctxt \"menu\"\nmsgid \"File\"\nmsgid_plural \"Files\"\n\
             msgstr[0] \"Fichier\"\nmsgstr[1] \"Fichiers\"\n\n\
             msgid \"\"\n\"First line\\n\"\n\"second line\"\nmsgstr \"\"\n\n\
             msgid \"\"\n\
             \"This is a long line in the source string, which is wrapped at 79 columns by \"\n\
             \"msgcat.\"\nmsgstr \"\"\n\n\
             #~ msgid \"Obsolete\"\n#~ msgstr \"Obsolète\"\n"
        );
        assert!(check(&content).is_empty(), "{:?}", check(&content));
    }

    #[test]
    fn test_error() {
        let content = format!(
            "{HEADER}msgid \"Hello\"\nmsgstr \"\"\n\"Bonjour\"\n\n\
             msgid \"First line\\nsecond line\"\nmsgstr \"\"\n"
        );
        let diags = check(&content);
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].severity, Severity::Info);
        assert_eq!(
            diags[0].message,
            "message not wrapped like msgcat (width: 79)"
        );
        assert_eq!(fix_bytes(&diags[0]), "msgstr \"Bonjour\"\n");
        assert_eq!(
            fix_bytes(&diags[1]),
            "msgid \"\"\n\"First line\\n\"\n\"second line\"\n"
        );
    }

    #[test]
    fn test_width() {
        let content = format!(
            "{HEADER}msgid \"\"\n\"This is a long line in the source string, which is \"\n\
             \"wrapped at 79 columns by msgcat.\"\nmsgstr \"\"\n"
        );
        assert_eq!(check(&content).len(), 1);
        assert!(check_width(&content, 60).is_empty());
        // Width 0: no wrap.
        let diags = check_width(&content, 0);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "message not wrapped like msgcat (no wrap)"
        );
    }

    #[test]
    fn test_no_wrap_flag() {
        let long = "word ".repeat(20);
        let content = format!("{HEADER}#, no-wrap\nmsgid \"{long}\"\nmsgstr \"\"\n");
        assert!(check(&content).is_empty());
        let content = format!("{HEADER}#, no-wrap\nmsgid \"a\\nb\"\nmsgstr \"\"\n");
        assert_eq!(check(&content).len(), 1);
    }

    #[test]
    fn test_obsolete() {
        let content = format!("{HEADER}#~ msgid \"\"\n#~ \"Obsolete\"\n#~ msgstr \"Obsolète\"\n");
        let diags = check(&content);
        assert_eq!(diags.len(), 1);
        assert_eq!(fix_bytes(&diags[0]), "#~ msgid \"Obsolete\"\n");
    }

    #[test]
    fn test_format_directives() {
        // msgcat does not break inside the directive "%%".
        let msgid = "msgid \"\"\n\
                     \"%3d. %s (%s), ficheiro: \\\"%s\\\" (local: \\\"%s\\\"), %s %s, estado: \
                     %s%s%s (%llu \"\n\"%%)\"\nmsgstr \"\"\n";
        assert!(check(&format!("{HEADER}#, c-format\n{msgid}")).is_empty());
        // Without the format flag, "%%" is not a directive: the break is
        // between the two "%".
        let diags = check(&format!("{HEADER}{msgid}"));
        assert_eq!(diags.len(), 1);
        assert!(fix_bytes(&diags[0]).ends_with("(%llu %\"\n\"%)\"\n"));
    }

    #[test]
    fn test_skipped() {
        // Unsupported format.
        let content = format!("{HEADER}#, lisp-format\nmsgid \"\"\n\"Hello\"\nmsgstr \"\"\n");
        assert!(check(&content).is_empty());
        // CRLF line endings.
        let content = "msgid \"Hello\"\r\nmsgstr \"\"\r\n\"Bonjour\"\r\n";
        assert!(check(content).is_empty());
        // Not UTF-8.
        let content = "msgid \"\"\nmsgstr \"Content-Type: text/plain; charset=ISO-8859-1\\n\"\n\n\
                       msgid \"Hello\"\nmsgstr \"\"\n\"Bonjour\"\n";
        assert!(check(content).is_empty());
        // Noqa.
        let content =
            format!("{HEADER}#, noqa:wrapping\nmsgid \"Hello\"\nmsgstr \"\"\n\"Bonjour\"\n");
        assert!(check(&content).is_empty());
    }
}
