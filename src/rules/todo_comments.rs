// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `todo-comments` rule: check markers left in translator comments.

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::po::entry::Entry;
use crate::rules::rule::RuleChecker;

/// Markers of unfinished work, matched as whole words (case sensitive).
const MARKERS: [&str; 3] = ["TODO", "FIXME", "XXX"];

pub struct TodoCommentsRule;

impl RuleChecker for TodoCommentsRule {
    fn name(&self) -> &'static str {
        "todo-comments"
    }

    fn description(&self) -> &'static str {
        "Check for TODO, FIXME and XXX markers left in translator comments."
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    fn uses_comments(&self) -> bool {
        true
    }

    /// Check for markers of unfinished work left in translator comments.
    ///
    /// The markers `TODO`, `FIXME` and `XXX` are searched as whole words, in upper case,
    /// in translator comments only (`# comment`): extracted comments (`#.`) are written
    /// by developers, and are not the translator's responsibility.
    ///
    /// This rule is not enabled by default.
    ///
    /// Wrong entry:
    /// ```text
    /// # TODO: check the translation of "bookmark"
    /// msgid "Add a bookmark"
    /// msgstr "Ajouter un marque-page"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// # "Marque-page" is the term used in Firefox.
    /// msgid "Add a bookmark"
    /// msgstr "Ajouter un marque-page"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`info`](Severity::Info): `translator comment with marker …`
    fn check_entry(&self, checker: &Checker, entry: &Entry) -> Vec<Diagnostic> {
        let mut found: Vec<&str> = Vec::new();
        let mut lines = Vec::new();
        for comment in &entry.translator_comments {
            let hl = get_markers(&comment.value);
            if hl.is_empty() {
                continue;
            }
            for (start, end) in &hl {
                let marker = &comment.value[*start..*end];
                if !found.contains(&marker) {
                    found.push(marker);
                }
            }
            lines.push((comment, hl));
        }
        if found.is_empty() {
            return vec![];
        }
        let message = format!(
            "translator comment with {} {}",
            if found.len() > 1 { "markers" } else { "marker" },
            found.join(", ")
        );
        self.new_diag(checker, Severity::Info, message)
            .map(|mut d| {
                for (comment, hl) in lines {
                    // Display the comment as in the PO file, with the "# " prefix.
                    d.add_line(
                        comment.line_number,
                        format!("# {}", comment.value),
                        hl.into_iter().map(|(start, end)| (start + 2, end + 2)),
                    );
                }
                d.with_entry(entry)
            })
            .into_iter()
            .collect()
    }
}

/// Return the byte ranges of the markers found in `s`, sorted by position.
fn get_markers(s: &str) -> Vec<(usize, usize)> {
    let bytes = s.as_bytes();
    let is_word = |pos: usize| bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'_';
    let mut markers: Vec<_> = MARKERS
        .iter()
        .flat_map(|marker| {
            s.match_indices(marker).filter_map(|(start, m)| {
                let end = start + m.len();
                let word_start = start == 0 || !is_word(start - 1);
                let word_end = end == bytes.len() || !is_word(end);
                (word_start && word_end).then_some((start, end))
            })
        })
        .collect();
    markers.sort_unstable();
    markers
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_todo_comments(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(TodoCommentsRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_get_markers() {
        assert!(get_markers("").is_empty());
        assert!(get_markers("nothing to do").is_empty());
        assert!(get_markers("todo, fixme, xxx").is_empty());
        assert!(get_markers("TODOS XXXX FIXMEE _TODO TODO2 ATODO").is_empty());
        assert_eq!(get_markers("TODO"), vec![(0, 4)]);
        assert_eq!(
            get_markers("TODO: FIXME (XXX), TODO(bob)"),
            vec![(0, 4), (6, 11), (13, 16), (19, 23)]
        );
        assert_eq!(get_markers("été XXX"), vec![(6, 9)]);
    }

    #[test]
    fn test_no_marker() {
        let diags = check_todo_comments(
            r#"
# Translator comment: nothing to do here.
#. TODO: extracted comments are not checked
#: src/todo.c:42
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_marker_noqa() {
        let diags = check_todo_comments(
            r#"
# TODO: check the translation
#, noqa:todo-comments
msgid "tested"
msgstr "testé"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_marker() {
        let diags = check_todo_comments(
            r#"
# TODO: check the translation
msgid "tested"
msgstr "testé"
"#,
        );
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Info);
        assert_eq!(diag.message, "translator comment with marker TODO");
        assert_eq!(diag.lines.len(), 3);
        assert_eq!(diag.lines[0].line_number, 2);
        assert_eq!(diag.lines[0].message, "# TODO: check the translation");
        assert_eq!(diag.lines[0].highlights, vec![(2, 6)]);
        assert_eq!(diag.lines[1].message, "msgid \"tested\"");
        assert_eq!(diag.lines[2].message, "msgstr \"testé\"");
    }

    #[test]
    fn test_multiple_markers() {
        let diags = check_todo_comments(
            r#"
# FIXME: wrong term, TODO: ask the team
# Some context.
# TODO: XXX
msgid "tested"
msgstr "testé"
"#,
        );
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(
            diag.message,
            "translator comment with markers FIXME, TODO, XXX"
        );
        assert_eq!(diag.lines.len(), 4);
        assert_eq!(diag.lines[0].line_number, 2);
        assert_eq!(diag.lines[0].highlights, vec![(2, 7), (21, 25)]);
        assert_eq!(diag.lines[1].line_number, 4);
        assert_eq!(diag.lines[1].message, "# TODO: XXX");
        assert_eq!(diag.lines[1].highlights, vec![(2, 6), (8, 11)]);
    }
}
