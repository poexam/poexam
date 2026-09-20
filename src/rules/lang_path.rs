// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `lang-path` rule: check that the `Language` field of
//! the PO file header agrees with the language announced by the file path.

use std::ffi::OsStr;
use std::path::Path;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::po::message::Message;
use crate::rules::header::is_valid_language;
use crate::rules::rule::RuleChecker;

/// Directory holding the catalogs of one locale in the gettext runtime layout,
/// e.g. `po/de/LC_MESSAGES/app.po`, where the file is named after the domain
/// and the locale is the parent directory.
const LC_MESSAGES: &str = "LC_MESSAGES";

pub struct LangPathRule;

/// Return the language code of a language tag: the part before the country and
/// the variant, so `pt`, `pt_BR`, `pt@x` and `pt_BR@x` all yield `pt`.
fn language_code(tag: &str) -> &str {
    &tag[..tag.find(['_', '@']).unwrap_or(tag.len())]
}

/// Whether a language tag read off a *file name* is unambiguous enough to
/// compare against the header.
///
/// A bare three-letter code is not: `app.po`, `cli.po`, `doc.po` and `gui.po`
/// are domain names with the exact shape of `ast.po` or `fil.po`, and on a
/// default-on rule, guessing wrong costs more than staying silent on the much
/// rarer catalogs named after a three-letter code. A two-letter code and any
/// tag carrying a country or a variant are kept: no domain is called `pt_BR`.
///
/// The ambiguity is specific to file names. A directory next to `LC_MESSAGES`
/// is the locale by construction, whatever the length of its name.
fn is_unambiguous_file_language(tag: &str) -> bool {
    is_valid_language(tag) && (tag.len() == 2 || tag.contains(['_', '@']))
}

/// Return the language a file path announces, or `None` when it announces none.
///
/// Two layouts are recognized:
/// - `po/de/LC_MESSAGES/app.po`: the parent of `LC_MESSAGES`. The file is named
///   after the domain there, so it is never consulted in this layout.
/// - `po/pt_BR.po`: the file stem, when it is
///   [unambiguously a language](is_unambiguous_file_language).
fn path_language(path: &Path) -> Option<&str> {
    if let Some(parent) = path.parent()
        && parent
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case(LC_MESSAGES))
    {
        return parent
            .parent()
            .and_then(Path::file_name)
            .and_then(OsStr::to_str)
            .filter(|locale| is_valid_language(locale));
    }
    path.file_stem()
        .and_then(OsStr::to_str)
        .filter(|stem| is_unambiguous_file_language(stem))
}

impl RuleChecker for LangPathRule {
    fn name(&self) -> &'static str {
        "lang-path"
    }

    fn description(&self) -> &'static str {
        "Language in PO file header different from the language in the file path."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check that the `Language` header field agrees with the language the file
    /// path announces.
    ///
    /// Starting a translation by copying an existing catalog and forgetting to
    /// update the header is a common mistake, and the consequences go beyond
    /// tidiness: gettext picks the plural rules of the declared language, and
    /// `spelling-str` loads its dictionary, turning the whole file into noise.
    ///
    /// The language is taken from the file stem (`po/fr.po`), or from the
    /// parent of `LC_MESSAGES` when the file is named after the domain
    /// (`po/de/LC_MESSAGES/app.po`). A path announcing no language, such as
    /// `messages.po`, is skipped, and so is a header whose `Language` is
    /// missing or malformed, which the `header` rule reports on its own. A file
    /// name that is a bare three-letter code is skipped as well, being
    /// indistinguishable from a domain name (see
    /// [`is_unambiguous_file_language`]).
    ///
    /// Only the language code is compared, so `pt_BR.po` is happy with
    /// `Language: pt_BR` and with `Language: pt`.
    ///
    /// Wrong entry (in `fr.po`):
    /// ```text
    /// msgid ""
    /// msgstr ""
    /// "Language: de\n"
    /// ```
    ///
    /// Correct entry (in `fr.po`):
    /// ```text
    /// msgid ""
    /// msgstr ""
    /// "Language: fr\n"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`warning`](Severity::Warning): `language '…' in header does not match '…' in file path`
    ///
    /// No auto-fix: which of the two is wrong depends on what the file actually
    /// contains, and rewriting the header of a catalog translated into another
    /// language would only hide the mistake.
    fn check_header(&self, checker: &Checker, _entry: &Entry, msgstr: &Message) -> Vec<Diagnostic> {
        let header_language = checker.language();
        // A missing or malformed `Language` is the `header` rule's business;
        // comparing it with the path would report the same problem again, in
        // vaguer words.
        if !is_valid_language(header_language) {
            return vec![];
        }
        let Some(path_language) = path_language(&checker.path) else {
            return vec![];
        };
        if language_code(path_language).eq_ignore_ascii_case(language_code(header_language)) {
            return vec![];
        }
        self.new_diag(
            checker,
            Severity::Warning,
            format!(
                "language '{header_language}' in header does not match '{path_language}' in file path"
            ),
        )
        .map(|d| d.with_msg(msgstr))
        .into_iter()
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    /// Check a header declaring `language` in a file at `path`.
    fn check(path: &str, language: &str) -> Vec<Diagnostic> {
        let content = format!(
            "msgid \"\"\nmsgstr \"\"\n\"Language: {language}\\n\"\n\
             \"Content-Type: text/plain; charset=UTF-8\\n\"\n"
        );
        let mut checker = Checker::new(content.as_bytes()).with_path(Path::new(path));
        let rules = Rules::new(vec![Box::new(LangPathRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    /// Assert that the single diagnostic reported names both languages.
    fn assert_mismatch(diags: &[Diagnostic], header: &str, path: &str) {
        assert_eq!(diags.len(), 1, "expected one diagnostic: {diags:?}");
        assert_eq!(diags[0].severity, Severity::Warning);
        assert_eq!(
            diags[0].message,
            format!("language '{header}' in header does not match '{path}' in file path")
        );
    }

    #[test]
    fn test_language_code() {
        assert_eq!(language_code("pt"), "pt");
        assert_eq!(language_code("pt_BR"), "pt");
        assert_eq!(language_code("ca@valencia"), "ca");
        assert_eq!(language_code("sr_RS@latin"), "sr");
    }

    #[test]
    fn test_matching_language_is_silent() {
        assert!(check("po/fr.po", "fr").is_empty());
        assert!(check("fr.po", "fr").is_empty());
        assert!(check("/abs/path/to/de.po", "de").is_empty());
    }

    #[test]
    fn test_mismatching_language_is_reported() {
        let diags = check("po/fr.po", "de");
        assert_mismatch(&diags, "de", "fr");
    }

    #[test]
    fn test_only_the_language_code_is_compared() {
        // A country or a variant on either side is not a mismatch.
        assert!(check("po/pt_BR.po", "pt_BR").is_empty());
        assert!(check("po/pt_BR.po", "pt").is_empty());
        assert!(check("po/pt.po", "pt_BR").is_empty());
        assert!(check("po/ca@valencia.po", "ca").is_empty());
        assert!(check("po/sr_RS@latin.po", "sr_RS").is_empty());
    }

    #[test]
    fn test_different_country_of_another_language_is_reported() {
        let diags = check("po/pt_BR.po", "de_DE");
        assert_mismatch(&diags, "de_DE", "pt_BR");
    }

    #[test]
    fn test_lc_messages_layout_uses_the_locale_directory() {
        assert!(check("po/de/LC_MESSAGES/app.po", "de").is_empty());
        assert!(check("locale/pt_BR/LC_MESSAGES/django.po", "pt_BR").is_empty());
        let diags = check("po/de/LC_MESSAGES/app.po", "fr");
        assert_mismatch(&diags, "fr", "de");
    }

    #[test]
    fn test_lc_messages_file_name_is_a_domain_not_a_language() {
        // In this layout the file is named after the domain, so it must never
        // be read as a language, whatever it looks like.
        assert!(check("build/LC_MESSAGES/app.po", "fr").is_empty());
        assert!(check("build/LC_MESSAGES/de.po", "fr").is_empty());
    }

    #[test]
    fn test_lc_messages_locale_may_be_a_three_letter_code() {
        // A directory next to LC_MESSAGES is the locale by construction, so the
        // ambiguity of a bare three-letter file name does not apply.
        assert!(check("po/ast/LC_MESSAGES/app.po", "ast").is_empty());
        let diags = check("po/ast/LC_MESSAGES/app.po", "fr");
        assert_mismatch(&diags, "fr", "ast");
    }

    #[test]
    fn test_path_announcing_no_language_is_skipped() {
        assert!(check("messages.po", "fr").is_empty());
        assert!(check("po/django.po", "fr").is_empty());
        // Uppercase is not a well-formed language tag.
        assert!(check("po/FR.po", "de").is_empty());
    }

    #[test]
    fn test_bare_three_letter_file_name_is_skipped() {
        // A domain name of three letters is shaped exactly like a language
        // code, so neither is read off the file name.
        assert!(check("po/app.po", "fr").is_empty());
        assert!(check("po/cli.po", "fr").is_empty());
        assert!(check("po/ast.po", "fr").is_empty());
        // Two letters, or a country or variant, stay unambiguous.
        assert_mismatch(&check("po/it.po", "fr"), "fr", "it");
        assert_mismatch(&check("po/ast_ES.po", "fr"), "fr", "ast_ES");
    }

    #[test]
    fn test_missing_or_malformed_header_language_is_skipped() {
        // Reported by the `header` rule, at the right severity.
        assert!(check("po/fr.po", "").is_empty());
        assert!(check("po/fr.po", "français").is_empty());
        assert!(check("po/fr.po", "DE").is_empty());
    }

    #[test]
    fn test_no_fix_is_offered() {
        let diags = check("po/fr.po", "de");
        assert!(diags[0].fix.is_none());
    }
}
