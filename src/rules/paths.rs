// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `paths` rule: check missing/extra/different paths.

use std::collections::HashSet;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Edit, Fix, FixTarget};
use crate::po::entry::Entry;
use crate::po::format::iter::FormatPathPos;
use crate::po::message::Message;
use crate::rules::double_quotes::trim_quotes;
use crate::rules::rule::RuleChecker;

pub struct PathsRule;

impl RuleChecker for PathsRule {
    fn name(&self) -> &'static str {
        "paths"
    }

    fn description(&self) -> &'static str {
        "Check for missing, extra or different paths in translation."
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for missing, extra or different paths in the translation.
    ///
    /// This rule is not enabled by default.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "Path: /tmp/output.txt"
    /// msgstr "Chemin : /tmp/sortie.txt"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "Path: /tmp/output.txt"
    /// msgstr "Chemin : /tmp/output.txt"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`warning`](Severity::Warning): `missing paths (# / #)`
    /// - [`warning`](Severity::Warning): `extra paths (# / #)`
    /// - [`warning`](Severity::Warning): `different paths` (auto-fixable)
    ///
    /// Only the `different paths` diagnostic carries an auto-fix: each
    /// translation path is replaced in place with the path at the same
    /// position in the source. The `missing` and `extra` cases are left
    /// unfixed because inserting a missing path at the right position in
    /// the prose or choosing which extra to drop both require translator
    /// judgement.
    fn check_msg(
        &self,
        checker: &Checker,
        entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let id_paths: Vec<_> = FormatPathPos::new(&msgid.value, entry.format_language).collect();
        let str_paths: Vec<_> = FormatPathPos::new(&msgstr.value, entry.format_language).collect();
        match id_paths.len().cmp(&str_paths.len()) {
            std::cmp::Ordering::Greater => self
                .new_diag(
                    checker,
                    Severity::Warning,
                    format!("missing paths ({} / {})", id_paths.len(), str_paths.len()),
                )
                .map(|d| {
                    d.with_msgs_hl(
                        msgid,
                        id_paths.iter().map(|m| (m.start, m.end)),
                        msgstr,
                        str_paths.iter().map(|m| (m.start, m.end)),
                    )
                })
                .into_iter()
                .collect(),
            std::cmp::Ordering::Less => self
                .new_diag(
                    checker,
                    Severity::Warning,
                    format!("extra paths ({} / {})", id_paths.len(), str_paths.len()),
                )
                .map(|d| {
                    d.with_msgs_hl(
                        msgid,
                        id_paths.iter().map(|m| (m.start, m.end)),
                        msgstr,
                        str_paths.iter().map(|m| (m.start, m.end)),
                    )
                })
                .into_iter()
                .collect(),
            std::cmp::Ordering::Equal => {
                // Check that paths are the same, in any order.
                // A single pair of quotes is skipped from both sides of the path.
                let id_paths_hash: HashSet<_> = id_paths.iter().map(|m| trim_quotes(m.s)).collect();
                let str_paths_hash: HashSet<_> =
                    str_paths.iter().map(|m| trim_quotes(m.s)).collect();
                if id_paths_hash == str_paths_hash {
                    vec![]
                } else {
                    let edits: Vec<Edit> = id_paths
                        .iter()
                        .zip(str_paths.iter())
                        .filter(|(id, str)| trim_quotes(id.s) != trim_quotes(str.s))
                        .map(|(id, str)| Edit {
                            range: str.start..str.end,
                            replacement: id.s.to_string(),
                        })
                        .collect();
                    let fix = (!edits.is_empty()).then(|| Fix {
                        target: FixTarget::Msgstr {
                            file_byte_range: msgstr.byte_range.clone(),
                        },
                        edits,
                        safe: false,
                    });
                    self.new_diag(checker, Severity::Warning, "different paths")
                        .map(|d| {
                            d.with_msgs_hl(
                                msgid,
                                id_paths.iter().map(|m| (m.start, m.end)),
                                msgstr,
                                str_paths.iter().map(|m| (m.start, m.end)),
                            )
                            .with_optional_fix(fix)
                        })
                        .into_iter()
                        .collect()
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_paths(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(PathsRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_no_paths() {
        let diags = check_paths(
            r#"
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_paths_ok() {
        let diags = check_paths(
            // Order of paths is not checked.
            r#"
msgid "/tmp/output.txt -- ./relative/path"
msgstr "./relative/path -- /tmp/output.txt"
"#,
        );
        println!("{diags:#?}");
        assert!(diags.is_empty());
    }

    #[test]
    fn test_paths_error() {
        let diags = check_paths(
            r#"
msgid "missing path: /tmp/output.txt -- ./relative/path"
msgstr "chemin manquant : /tmp/output.txt"

msgid "extra path: /tmp/output.txt"
msgstr "chemin extra : /tmp/output.txt -- ./relative/path"

msgid "different paths: /tmp/test/output.txt -- ./relative/path"
msgstr "chemins différents : /tmp/output.txt -- ./relative/path"
"#,
        );
        assert_eq!(diags.len(), 3);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "missing paths (2 / 1)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "extra paths (1 / 2)");
        let diag = &diags[2];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "different paths");
    }

    #[test]
    fn test_different_paths_fix_replaces_each_in_place() {
        let diags = check_paths(
            r#"
msgid "Files at /tmp/a and /tmp/b"
msgstr "Fichiers à /var/a et /var/b"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "different paths");
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 2);
        assert_eq!(fix.edits[0].replacement, "/tmp/a");
        assert_eq!(fix.edits[1].replacement, "/tmp/b");
    }

    #[test]
    fn test_different_paths_fix_skips_positions_already_equal() {
        let diags = check_paths(
            r#"
msgid "Files at /tmp/a and /tmp/b"
msgstr "Fichiers à /tmp/a et /var/b"
"#,
        );
        assert_eq!(diags.len(), 1);
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].replacement, "/tmp/b");
    }

    #[test]
    fn test_missing_and_extra_paths_have_no_fix() {
        let diags = check_paths(
            r#"
msgid "Files at /tmp/a and /tmp/b"
msgstr "Fichiers à /tmp/a"

msgid "Files at /tmp/a"
msgstr "Fichiers à /tmp/a and /tmp/b"
"#,
        );
        assert_eq!(diags.len(), 2);
        assert!(
            diags[0].fix.is_none(),
            "missing paths diagnostic must not carry a fix"
        );
        assert!(
            diags[1].fix.is_none(),
            "extra paths diagnostic must not carry a fix"
        );
    }
}
