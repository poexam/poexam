// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `compendium` rule: check for unresolved `msgcat`
//! conflict markers left in the translation.

use std::sync::LazyLock;

use memchr::memmem;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

/// Marker `msgcat` writes around each alternative when the catalogs it merges
/// disagree on the translation of one `msgid`.
const CONFLICT_MARKER: &str = "#-#-#-#-#";

/// Prebuilt searcher for [`CONFLICT_MARKER`]: the rule runs on every message of
/// every file, so `memmem` pays for its preprocessing once instead of per call.
static MARKER_FINDER: LazyLock<memmem::Finder<'static>> =
    LazyLock::new(|| memmem::Finder::new(CONFLICT_MARKER.as_bytes()));

/// Return the byte range of every conflict marker in `value`.
fn conflict_markers(value: &str) -> Vec<(usize, usize)> {
    MARKER_FINDER
        .find_iter(value.as_bytes())
        .map(|start| (start, start + CONFLICT_MARKER.len()))
        .collect()
}

pub struct CompendiumRule;

impl CompendiumRule {
    /// Report the conflict markers of one message, `location` naming where the
    /// message sits for the diagnostic.
    fn check_value(&self, checker: &Checker, msg: &Message, location: &str) -> Vec<Diagnostic> {
        let markers = conflict_markers(&msg.value);
        if markers.is_empty() {
            return vec![];
        }
        self.new_diag(
            checker,
            Severity::Error,
            format!("unresolved msgcat conflict '{CONFLICT_MARKER}' in {location}"),
        )
        .map(|d| d.with_msg_hl(msg, markers))
        .into_iter()
        .collect()
    }
}

impl RuleChecker for CompendiumRule {
    fn name(&self) -> &'static str {
        "compendium"
    }

    fn description(&self) -> &'static str {
        "Unresolved msgcat conflict markers in translation."
    }

    fn is_default(&self) -> bool {
        true
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check the PO file header for unresolved `msgcat` conflict markers.
    ///
    /// Merging catalogs whose headers differ puts the markers there too, and
    /// the `header` rule does not notice: it reads the header line by line and
    /// a marker line, holding no `:`, is simply not a field.
    ///
    /// Diagnostics reported:
    /// - [`error`](Severity::Error): `unresolved msgcat conflict '#-#-#-#-#' in header`
    fn check_header(&self, checker: &Checker, _entry: &Entry, msgstr: &Message) -> Vec<Diagnostic> {
        self.check_value(checker, msgstr, "header")
    }

    /// Check for unresolved `msgcat` conflict markers in the translation.
    ///
    /// When `msgcat` merges catalogs that hold conflicting translations for the
    /// same `msgid`, it does not pick one: it writes every alternative into the
    /// `msgstr`, each introduced by a `#-#-#-#-#` line naming where it came
    /// from. The conflict is meant to be resolved by hand, and a catalog still
    /// carrying the markers ships them verbatim to users.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "Save"
    /// msgstr ""
    /// "#-#-#-#-#  fr.po (app)  #-#-#-#-#\n"
    /// "Enregistrer\n"
    /// "#-#-#-#-#  fr.po (lib)  #-#-#-#-#\n"
    /// "Sauvegarder"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "Save"
    /// msgstr "Enregistrer"
    /// ```
    ///
    /// `msgcat` marks every entry it could not merge as fuzzy, and fuzzy
    /// entries are skipped unless `--fuzzy` is given, so a freshly merged
    /// catalog reports its conflicts through the header. This diagnostic is
    /// what catches the state that actually ships: the fuzzy flag cleared
    /// while the markers are still there.
    ///
    /// Diagnostics reported:
    /// - [`error`](Severity::Error): `unresolved msgcat conflict '#-#-#-#-#' in translation`
    ///
    /// No auto-fix: the whole point of the markers is that `msgcat` could not
    /// choose, and neither can poexam. Keeping one alternative and dropping the
    /// others is the translator's call.
    fn check_msg(
        &self,
        checker: &Checker,
        _entry: &Entry,
        _msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        self.check_value(checker, msgstr, "translation")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_compendium(content: &str) -> Vec<Diagnostic> {
        check_compendium_with(content, false)
    }

    /// Check a PO content, optionally checking the fuzzy entries too.
    fn check_compendium_with(content: &str, fuzzy: bool) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        checker.config.check.fuzzy = fuzzy;
        let rules = Rules::new(vec![Box::new(CompendiumRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_conflict_markers() {
        assert!(conflict_markers("Enregistrer").is_empty());
        assert!(conflict_markers("#-#-#-#").is_empty());
        assert_eq!(conflict_markers("#-#-#-#-#"), vec![(0, 9)]);
        assert_eq!(
            conflict_markers("#-#-#-#-#  a.po  #-#-#-#-#\nx"),
            vec![(0, 9), (17, 26)],
        );
    }

    #[test]
    fn test_no_conflict_is_silent() {
        let diags = check_compendium(
            r#"
msgid "Save"
msgstr "Enregistrer"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_hash_in_translation_is_not_a_conflict() {
        // A lone '#', a comment-looking string or a shorter run must not match.
        let diags = check_compendium(
            r#"
msgid "Issue #42 and #-#-#-#"
msgstr "Ticket #42 et #-#-#-#"
"#,
        );
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn test_conflict_in_translation_is_reported() {
        let diags = check_compendium(
            "\nmsgid \"Save\"\nmsgstr \"\"\n\
             \"#-#-#-#-#  fr.po (app)  #-#-#-#-#\\n\"\n\
             \"Enregistrer\\n\"\n\
             \"#-#-#-#-#  fr.po (lib)  #-#-#-#-#\\n\"\n\
             \"Sauvegarder\"\n",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Error);
        assert_eq!(
            diags[0].message,
            "unresolved msgcat conflict '#-#-#-#-#' in translation"
        );
    }

    #[test]
    fn test_every_marker_is_highlighted() {
        let diags = check_compendium(
            "\nmsgid \"Save\"\nmsgstr \"\"\n\
             \"#-#-#-#-#  a.po  #-#-#-#-#\\n\"\n\
             \"Un\\n\"\n\
             \"#-#-#-#-#  b.po  #-#-#-#-#\\n\"\n\
             \"Deux\"\n",
        );
        assert_eq!(diags.len(), 1);
        let highlights: usize = diags[0].lines.iter().map(|l| l.highlights.len()).sum();
        assert_eq!(highlights, 4);
    }

    #[test]
    fn test_conflict_in_header_is_reported() {
        // Merging catalogs with differing headers puts the markers there too.
        let diags = check_compendium(
            "msgid \"\"\nmsgstr \"\"\n\
             \"#-#-#-#-#  a.po  #-#-#-#-#\\n\"\n\
             \"Project-Id-Version: a\\n\"\n\
             \"#-#-#-#-#  b.po  #-#-#-#-#\\n\"\n\
             \"Project-Id-Version: b\\n\"\n",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Error);
        assert_eq!(
            diags[0].message,
            "unresolved msgcat conflict '#-#-#-#-#' in header"
        );
    }

    #[test]
    fn test_each_plural_form_is_checked() {
        let diags = check_compendium(
            "\nmsgid \"%d file\"\nmsgid_plural \"%d files\"\n\
             msgstr[0] \"#-#-#-#-# a #-#-#-#-# %d fichier\"\n\
             msgstr[1] \"#-#-#-#-# a #-#-#-#-# %d fichiers\"\n",
        );
        assert_eq!(diags.len(), 2);
        assert!(diags.iter().all(|d| d.message.ends_with("in translation")));
    }

    #[test]
    fn test_fuzzy_entry_needs_the_fuzzy_option() {
        // msgcat marks every entry it could not merge as fuzzy, so a freshly
        // merged catalog reports its conflicts through the header; this
        // diagnostic catches the state that ships, once the flag is cleared.
        let content = "\n#, fuzzy\nmsgid \"Save\"\n\
                       msgstr \"#-#-#-#-# a #-#-#-#-# Enregistrer\"\n";
        assert!(check_compendium(content).is_empty());
        assert_eq!(check_compendium_with(content, true).len(), 1);
    }

    #[test]
    fn test_no_fix_is_offered() {
        let diags =
            check_compendium("\nmsgid \"Save\"\nmsgstr \"#-#-#-#-# a #-#-#-#-# Enregistrer\"\n");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].fix.is_none());
    }
}
