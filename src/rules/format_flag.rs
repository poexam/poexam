// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `format-flag` rule: check for entries with format
//! strings but without a format flag.

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::po::format::iter::FormatPos;
use crate::po::format::language::Language;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct FormatFlagRule;

/// Return the length of the identifier (ASCII letters, digits and `_`, not
/// starting with a digit) at the start of `s`, 0 if there is none.
fn identifier_len(s: &str) -> usize {
    if !s.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
        return 0;
    }
    s.find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .unwrap_or(s.len())
}

/// Return the length of the number at the start of `s`, 0 if there is none.
fn number_len(s: &str) -> usize {
    s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len())
}

/// Return `true` if the brace format string `s` (e.g. `{0}` or `{name}`) is
/// valid for the language: a number or a name, followed by the end or by
/// the separator of the format specification.
fn is_brace_format_string(s: &str, language: Language) -> bool {
    let Some(content) = s.strip_prefix('{').and_then(|s| s.strip_suffix('}')) else {
        return false;
    };
    let (len, separators) = match language {
        Language::CSharp | Language::Java => (number_len(content), ",:"),
        Language::PerlBrace => (identifier_len(content), ""),
        _ => (identifier_len(content).max(number_len(content)), ".[!:"),
    };
    len > 0
        && content[len..]
            .chars()
            .next()
            .is_none_or(|c| separators.contains(c))
}

/// Return `true` if the format string at `start..end` of `value`, returned by
/// the lenient format parser of the language, is a valid format string, like
/// `xgettext` would consider it.
///
/// Format strings with a space flag (e.g. `% d` in `100% done`) and brace
/// format strings preceded by `$` (template variables, e.g. `${name}`) are
/// considered invalid.
fn is_format_string(value: &str, start: usize, end: usize, language: Language) -> bool {
    let s = &value[start..end];
    let conversions = match language {
        Language::C => "diouxXeEfFgGaAcspnm",
        Language::JavaScript => "bcdiofjsxXeEgG",
        Language::Lua => "aAcdeEfgGioqsuxX",
        Language::ObjectPascal => "dDeEfFgGmMnNpPsSuUxX",
        Language::Perl => "bBcdeEfFgGinopsuxX",
        Language::Php => "bcdeEfFgGosuxX",
        Language::Python => "acdeEfFgGiorsuxX",
        Language::Ruby => "aAbBcdeEfgGiopsuxX}",
        Language::CSharp | Language::Java | Language::PerlBrace | Language::PythonBrace => {
            return !value[..start].ends_with('$') && is_brace_format_string(s, language);
        }
        Language::Qt => return true,
        Language::Sh | Language::Null => return false,
    };
    s.len() > 1
        && !s.contains(' ')
        && s.chars()
            .next_back()
            .is_some_and(|c| conversions.contains(c))
}

/// Return the positions of the format strings of the message, or an empty
/// vector if any of them is invalid: then the message is not a format string
/// of this language (e.g. `%Y-%m-%d` is a date format, not a C format
/// string).
fn format_strings(msg: &Message, language: Language) -> Vec<(usize, usize)> {
    let mut positions = vec![];
    for m in FormatPos::new(&msg.value, language) {
        if !is_format_string(&msg.value, m.start, m.end, language) {
            return vec![];
        }
        positions.push((m.start, m.end));
    }
    positions
}

/// Return `true` if the entry has a format flag, e.g. `c-format` or
/// `no-c-format`.
fn has_format_flag(entry: &Entry) -> bool {
    entry.keywords.iter().any(|k| k.ends_with("-format"))
}

impl RuleChecker for FormatFlagRule {
    fn name(&self) -> &'static str {
        "format-flag"
    }

    fn description(&self) -> &'static str {
        "Check for entries with format strings but without a format flag."
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    fn uses_entries(&self) -> bool {
        true
    }

    /// Check for entries with format strings but without a format flag: the
    /// format strings of these entries are not checked by the rule `formats`
    /// (nor by `msgfmt --check-format`), so a wrong format string in the
    /// translation is not detected, and may crash the program at runtime.
    ///
    /// The format languages looked for are the ones of the flags used in the
    /// file (e.g. `c-format`): an entry without any format flag (neither
    /// `c-format` nor `no-c-format`) is reported if its source (`msgid` or
    /// `msgid_plural`) contains format strings of one of these languages.
    ///
    /// Like `xgettext` does, a source with an invalid format string is not
    /// considered as a format string of this language, e.g. `%d/%m/%Y` (date
    /// format) is not a C format string. To prevent false positives, are also
    /// considered invalid: format strings with a space flag (e.g. `% d` in
    /// `100% done`) and brace format strings preceded by `$` (template
    /// variables, e.g. `${name}`).
    ///
    /// Shell variables (`sh-format`) are not looked for: a variable in a text
    /// is often a literal, e.g. in a command (`git switch -c $branch`) or an
    /// environment variable (`$HOME not set`).
    ///
    /// The flag must be added in the source code, or in the template (POT
    /// file) with the `xgettext` option `--flag`: a flag added only in the
    /// translated PO file is lost at the next update with `msgmerge`.
    ///
    /// Wrong entries:
    /// ```text
    /// #, c-format
    /// msgid "%d files"
    /// msgstr "%d fichiers"
    ///
    /// msgid "%d directories"
    /// msgstr "%d répertoires"
    /// ```
    ///
    /// Correct entries:
    /// ```text
    /// #, c-format
    /// msgid "%d files"
    /// msgstr "%d fichiers"
    ///
    /// #, c-format
    /// msgid "%d directories"
    /// msgstr "%d répertoires"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`info`](Severity::Info): `missing format flag (…)`
    fn check_entries(&self, checker: &Checker, entries: &[&Entry]) -> Vec<Diagnostic> {
        let mut languages: Vec<Language> = vec![];
        for entry in entries {
            if !matches!(entry.format_language, Language::Null | Language::Sh)
                && !languages.contains(&entry.format_language)
            {
                languages.push(entry.format_language);
            }
        }
        if languages.is_empty() {
            return vec![];
        }
        let mut diags = vec![];
        for entry in entries {
            if has_format_flag(entry) {
                continue;
            }
            let Some(msgid) = &entry.msgid else {
                continue;
            };
            let sources = std::iter::once(msgid).chain(entry.msgid_plural.iter());
            for msg in sources {
                let mut flags = vec![];
                let mut highlights = vec![];
                for language in &languages {
                    let hl = format_strings(msg, *language);
                    if !hl.is_empty() {
                        flags.push(language.flag());
                        highlights.extend(hl);
                    }
                }
                if flags.is_empty() {
                    continue;
                }
                highlights.sort_unstable();
                highlights.dedup();
                if let Some(diag) = self.new_diag(
                    checker,
                    Severity::Info,
                    format!("missing format flag ({})", flags.join(" or ")),
                ) {
                    diags.push(diag.with_msg_hl(msg, highlights));
                }
                // Report the entry only once.
                break;
            }
        }
        diags
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::rule::Rules;

    const HEADER: &str = "msgid \"\"\nmsgstr \"Content-Type: text/plain; charset=UTF-8\\n\"\n\n";

    fn check(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(FormatFlagRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_no_flag_in_file() {
        let content = format!("{HEADER}msgid \"%d files\"\nmsgstr \"%d fichiers\"\n");
        assert!(check(&content).is_empty());
    }

    #[test]
    fn test_no_error() {
        let content = format!(
            "{HEADER}#, c-format\nmsgid \"%d files\"\nmsgstr \"%d fichiers\"\n\n\
             #, no-c-format\nmsgid \"%d directories\"\nmsgstr \"%d répertoires\"\n\n\
             #, python-format\nmsgid \"%(n)d links\"\nmsgstr \"%(n)d liens\"\n\n\
             msgid \"100% done\"\nmsgstr \"100 % terminé\"\n\n\
             msgid \"50% of {{name}}\"\nmsgstr \"50 % de {{name}}\"\n\n\
             msgid \"100%\"\nmsgstr \"100 %\"\n\n\
             msgid \"100%%\"\nmsgstr \"100 %%\"\n\n\
             msgid \"Date: %d/%m/%Y\"\nmsgstr \"Date : %d/%m/%Y\"\n\n\
             msgid \"%s: done at 100%.\"\nmsgstr \"%s : fait à 100 %.\"\n\n\
             msgid \"Hello\"\nmsgstr \"Bonjour\"\n"
        );
        assert!(check(&content).is_empty(), "{:?}", check(&content));
    }

    #[test]
    fn test_error() {
        let content = format!(
            "{HEADER}#, c-format\nmsgid \"%d files\"\nmsgstr \"%d fichiers\"\n\n\
             msgid \"%d directories in %s\"\nmsgstr \"%d répertoires dans %s\"\n\n\
             msgid \"One link\"\nmsgid_plural \"%d links\"\n\
             msgstr[0] \"Un lien\"\nmsgstr[1] \"%d liens\"\n"
        );
        let diags = check(&content);
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].severity, Severity::Info);
        assert_eq!(diags[0].message, "missing format flag (c-format)");
        assert_eq!(diags[0].lines[0].line_number, 8);
        assert_eq!(diags[1].message, "missing format flag (c-format)");
        assert_eq!(diags[1].lines[0].line_number, 12);
    }

    #[test]
    fn test_several_languages() {
        let content = format!(
            "{HEADER}#, c-format\nmsgid \"%d files\"\nmsgstr \"%d fichiers\"\n\n\
             #, python-brace-format\nmsgid \"{{n}} files\"\nmsgstr \"{{n}} fichiers\"\n\n\
             msgid \"%s in {{dir}}\"\nmsgstr \"%s dans {{dir}}\"\n\n\
             msgid \"File {{name}}\"\nmsgstr \"Fichier {{name}}\"\n"
        );
        let diags = check(&content);
        assert_eq!(diags.len(), 2);
        assert_eq!(
            diags[0].message,
            "missing format flag (c-format or python-brace-format)"
        );
        assert_eq!(
            diags[1].message,
            "missing format flag (python-brace-format)"
        );
    }

    #[test]
    fn test_brace_and_shell() {
        let content = format!(
            "{HEADER}#, python-brace-format\nmsgid \"{{n}} files\"\nmsgstr \"{{n}} fichiers\"\n\n\
             #, sh-format\nmsgid \"File $file\"\nmsgstr \"Fichier $file\"\n\n\
             msgid \"Use ${{NAME}}\"\nmsgstr \"Utiliser ${{NAME}}\"\n\n\
             msgid \"Keys alt-{{home|end}}\"\nmsgstr \"Touches alt-{{home|end}}\"\n\n\
             msgid \"$HOME not set\"\nmsgstr \"$HOME non défini\"\n\n\
             msgid \"Dir {{0}}: {{dir.name!r:>10}}\"\nmsgstr \"Rép {{0}} : {{dir.name!r:>10}}\"\n\n\
             msgid \"Directory $dir\"\nmsgstr \"Répertoire $dir\"\n"
        );
        let diags = check(&content);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "missing format flag (python-brace-format)"
        );
    }

    #[test]
    fn test_noqa() {
        let content = format!(
            "{HEADER}#, c-format\nmsgid \"%d files\"\nmsgstr \"%d fichiers\"\n\n\
             #, noqa:format-flag\nmsgid \"%d directories\"\nmsgstr \"%d répertoires\"\n"
        );
        assert!(check(&content).is_empty());
    }
}
