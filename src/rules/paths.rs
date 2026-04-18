// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `paths` rule: check missing/extra/different paths.

use std::collections::HashSet;

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
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

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Info
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
    /// Diagnostics reported with severity [`info`](Severity::Info):
    /// - `missing paths (# / #)`
    /// - `extra paths (# / #)`
    /// - `different paths`
    fn check_msg(
        &self,
        checker: &Checker,
        entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let id_paths: Vec<_> = FormatPathPos::new(&msgid.value, &entry.format_language).collect();
        let str_paths: Vec<_> = FormatPathPos::new(&msgstr.value, &entry.format_language).collect();
        match id_paths.len().cmp(&str_paths.len()) {
            std::cmp::Ordering::Greater => {
                vec![
                    self.new_diag(
                        checker,
                        format!("missing paths ({} / {})", id_paths.len(), str_paths.len()),
                    )
                    .with_msgs_hl(
                        msgid,
                        &id_paths
                            .iter()
                            .map(|m| (m.start, m.end))
                            .collect::<Vec<_>>(),
                        msgstr,
                        &str_paths
                            .iter()
                            .map(|m| (m.start, m.end))
                            .collect::<Vec<_>>(),
                    ),
                ]
            }
            std::cmp::Ordering::Less => {
                vec![
                    self.new_diag(
                        checker,
                        format!("extra paths ({} / {})", id_paths.len(), str_paths.len()),
                    )
                    .with_msgs_hl(
                        msgid,
                        &id_paths
                            .iter()
                            .map(|m| (m.start, m.end))
                            .collect::<Vec<_>>(),
                        msgstr,
                        &str_paths
                            .iter()
                            .map(|m| (m.start, m.end))
                            .collect::<Vec<_>>(),
                    ),
                ]
            }
            std::cmp::Ordering::Equal => {
                // Check that paths are the same, in any order.
                // A single pair of quotes is skipped from both sides of the path.
                let id_paths_hash: HashSet<_> = id_paths.iter().map(|m| trim_quotes(m.s)).collect();
                let str_paths_hash: HashSet<_> =
                    str_paths.iter().map(|m| trim_quotes(m.s)).collect();
                if id_paths_hash == str_paths_hash {
                    vec![]
                } else {
                    vec![
                        self.new_diag(checker, "different paths".to_string())
                            .with_msgs_hl(
                                msgid,
                                &id_paths
                                    .iter()
                                    .map(|m| (m.start, m.end))
                                    .collect::<Vec<_>>(),
                                msgstr,
                                &str_paths
                                    .iter()
                                    .map(|m| (m.start, m.end))
                                    .collect::<Vec<_>>(),
                            ),
                    ]
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
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "missing paths (2 / 1)");
        let diag = &diags[1];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "extra paths (1 / 2)");
        let diag = &diags[2];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "different paths");
    }
}
