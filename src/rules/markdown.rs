// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `markdown` rule: check missing/extra/different Markdown syntax.

use crate::checker::Checker;
use crate::diagnostic::{Diagnostic, Severity};
use crate::fix::{Edit, Fix, FixTarget};
use crate::po::entry::Entry;
use crate::po::format::iter::{FormatMarkdownPos, MarkdownKind, MatchMarkdownPos};
use crate::po::message::Message;
use crate::rules::rule::RuleChecker;

pub struct MarkdownRule;

/// Names used in the diagnostics for a kind of Markdown construct: the plural name of the
/// construct, and what differs when the counts are equal, or `None` when only the counts
/// are compared.
///
/// The content of code spans is not compared: it often holds placeholders that are
/// translated, e.g. `` `copy <file>` `` or `` `user@host` ``.
fn kind_names(kind: MarkdownKind) -> (&'static str, Option<&'static str>) {
    match kind {
        MarkdownKind::CodeSpan => ("code spans", None),
        MarkdownKind::Link => ("links", Some("link targets")),
        MarkdownKind::Emphasis => ("emphasis markers", Some("emphasis markers")),
    }
}

/// Return the key used to compare a Markdown construct between source and translation.
///
/// Emphasis markers are compared by length only: `*text*` and `_text_` render the same.
fn compare_key<'a>(m: &MatchMarkdownPos<'a>) -> &'a str {
    match m.kind {
        MarkdownKind::Emphasis => &"***"[..m.s.len()],
        MarkdownKind::CodeSpan | MarkdownKind::Link => m.s,
    }
}

impl MarkdownRule {
    /// Compare the Markdown constructs of one kind in the source and the translation, and
    /// return a diagnostic if they differ.
    fn check_kind(
        &self,
        checker: &Checker,
        kind: MarkdownKind,
        msgid: &Message,
        id_items: &[&MatchMarkdownPos],
        msgstr: &Message,
        str_items: &[&MatchMarkdownPos],
    ) -> Option<Diagnostic> {
        let (name, different) = kind_names(kind);
        let (message, fix) = match id_items.len().cmp(&str_items.len()) {
            std::cmp::Ordering::Greater => (
                format!(
                    "missing Markdown {name} ({} / {})",
                    id_items.len(),
                    str_items.len()
                ),
                None,
            ),
            std::cmp::Ordering::Less => (
                format!(
                    "extra Markdown {name} ({} / {})",
                    id_items.len(),
                    str_items.len()
                ),
                None,
            ),
            std::cmp::Ordering::Equal => {
                let different = different?;
                // Check that the constructs are the same, in any order.
                let mut id_keys: Vec<_> = id_items.iter().map(|m| compare_key(m)).collect();
                let mut str_keys: Vec<_> = str_items.iter().map(|m| compare_key(m)).collect();
                id_keys.sort_unstable();
                str_keys.sort_unstable();
                if id_keys == str_keys {
                    return None;
                }
                // Emphasis markers are not fixed: the right marker for a translated word
                // is the translator's call.
                let edits: Vec<Edit> = if kind == MarkdownKind::Link {
                    id_items
                        .iter()
                        .zip(str_items.iter())
                        .filter(|(id, str)| id.s != str.s)
                        .map(|(id, str)| Edit {
                            range: str.start..str.end,
                            replacement: id.s.to_string(),
                        })
                        .collect()
                } else {
                    vec![]
                };
                let fix = (!edits.is_empty()).then(|| Fix {
                    target: FixTarget::Msgstr {
                        file_byte_range: msgstr.byte_range.clone(),
                    },
                    edits,
                    safe: false,
                });
                (format!("different Markdown {different}"), fix)
            }
        };
        self.new_diag(checker, Severity::Warning, message).map(|d| {
            d.with_msgs_hl(
                msgid,
                id_items.iter().map(|m| (m.start, m.end)),
                msgstr,
                str_items.iter().map(|m| (m.start, m.end)),
            )
            .with_optional_fix(fix)
        })
    }
}

impl RuleChecker for MarkdownRule {
    fn name(&self) -> &'static str {
        "markdown"
    }

    fn description(&self) -> &'static str {
        "Check for missing, extra or different Markdown syntax in translation."
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    /// Check for missing, extra or different Markdown syntax in the translation: code
    /// spans, inline link destinations and emphasis markers.
    ///
    /// Code spans are only counted: their content may hold translated placeholders.
    ///
    /// This rule is not enabled by default.
    ///
    /// Wrong entry:
    /// ```text
    /// msgid "Read [the **manual**](https://example.com/manual) or run `make help`"
    /// msgstr "Lisez [le manuel] (https://example.com/manuel) ou lancez make help"
    /// ```
    ///
    /// Correct entry:
    /// ```text
    /// msgid "Read [the **manual**](https://example.com/manual) or run `make help`"
    /// msgstr "Lisez [le **manuel**](https://example.com/manual) ou lancez `make help`"
    /// ```
    ///
    /// Diagnostics reported:
    /// - [`warning`](Severity::Warning): `missing Markdown code spans (# / #)`
    /// - [`warning`](Severity::Warning): `extra Markdown code spans (# / #)`
    /// - [`warning`](Severity::Warning): `missing Markdown links (# / #)`
    /// - [`warning`](Severity::Warning): `extra Markdown links (# / #)`
    /// - [`warning`](Severity::Warning): `different Markdown link targets` (auto-fixable)
    /// - [`warning`](Severity::Warning): `missing Markdown emphasis markers (# / #)`
    /// - [`warning`](Severity::Warning): `extra Markdown emphasis markers (# / #)`
    /// - [`warning`](Severity::Warning): `different Markdown emphasis markers`
    ///
    /// Only the `different Markdown link targets` diagnostic carries an auto-fix: each
    /// translation link destination is replaced in place with the one at the same position
    /// in the source.
    fn check_msg(
        &self,
        checker: &Checker,
        entry: &Entry,
        msgid: &Message,
        msgstr: &Message,
    ) -> Vec<Diagnostic> {
        let id_items: Vec<_> =
            FormatMarkdownPos::new(&msgid.value, entry.format_language).collect();
        if id_items.is_empty() && !msgstr.value.contains(['`', '[', '*', '_']) {
            // Fast path: no Markdown syntax on either side.
            return vec![];
        }
        let str_items: Vec<_> =
            FormatMarkdownPos::new(&msgstr.value, entry.format_language).collect();
        [
            MarkdownKind::CodeSpan,
            MarkdownKind::Link,
            MarkdownKind::Emphasis,
        ]
        .into_iter()
        .filter_map(|kind| {
            let id_kind: Vec<_> = id_items.iter().filter(|m| m.kind == kind).collect();
            let str_kind: Vec<_> = str_items.iter().filter(|m| m.kind == kind).collect();
            self.check_kind(checker, kind, msgid, &id_kind, msgstr, &str_kind)
        })
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::po::format::language::Language;
    use crate::{diagnostic::Diagnostic, rules::rule::Rules};

    fn check_markdown(content: &str) -> Vec<Diagnostic> {
        let mut checker = Checker::new(content.as_bytes());
        let rules = Rules::new(vec![Box::new(MarkdownRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    fn items(s: &str, language: Language) -> Vec<(MarkdownKind, &str)> {
        FormatMarkdownPos::new(s, language)
            .map(|m| (m.kind, m.s))
            .collect()
    }

    #[test]
    fn test_iter_code_spans() {
        assert_eq!(
            items("run `make` or ``a ` b``", Language::Null),
            vec![
                (MarkdownKind::CodeSpan, "`make`"),
                (MarkdownKind::CodeSpan, "``a ` b``"),
            ]
        );
        // Emphasis markers inside a code span are literal.
        assert_eq!(
            items("`a*b*c` *d*", Language::Null),
            vec![
                (MarkdownKind::CodeSpan, "`a*b*c`"),
                (MarkdownKind::Emphasis, "*"),
                (MarkdownKind::Emphasis, "*"),
            ]
        );
        // Unclosed backtick and legacy GNU quoting: no code span.
        assert!(items("a lone ` backtick", Language::Null).is_empty());
        assert!(items("use `foo' and `bar'", Language::Null).is_empty());
    }

    #[test]
    fn test_iter_links() {
        assert_eq!(
            items(
                "[a](https://a.com) ![img](b.png \"title\") [c](<d e.md>) [f](a_(g).md)",
                Language::Null
            ),
            vec![
                (MarkdownKind::Link, "https://a.com"),
                (MarkdownKind::Link, "b.png"),
                (MarkdownKind::Link, "d e.md"),
                (MarkdownKind::Link, "a_(g).md"),
            ]
        );
        // Not inline links: space before the destination, no destination, reference.
        assert!(items("[a] (https://a.com) [b] [c][ref]", Language::Null).is_empty());
        // Destination not looking like a URL or a path: label and accelerator.
        assert!(items("前景顏色[透明度](_R)", Language::Null).is_empty());
        assert_eq!(
            items("[a](%s)", Language::C),
            vec![(MarkdownKind::Link, "%s")]
        );
        // Unclosed destination.
        assert!(items("[a](https://a.com", Language::Null).is_empty());
    }

    #[test]
    fn test_iter_link_text_with_markup() {
        assert_eq!(
            items("**[the `docs`](https://x.com/a_b*c)**", Language::Null),
            vec![
                (MarkdownKind::Emphasis, "**"),
                (MarkdownKind::Link, "https://x.com/a_b*c"),
                (MarkdownKind::CodeSpan, "`docs`"),
                (MarkdownKind::Emphasis, "**"),
            ]
        );
        // An emphasis opened in the link text does not close in the destination.
        assert!(
            items("[*a](https://x.com/b*)", Language::Null)
                .iter()
                .all(|(kind, _)| *kind == MarkdownKind::Link)
        );
    }

    #[test]
    fn test_iter_emphasis() {
        assert_eq!(
            items("*a* **b** ***c*** _d_ __e__", Language::Null)
                .iter()
                .map(|(_, s)| *s)
                .collect::<Vec<_>>(),
            vec!["*", "*", "**", "**", "***", "***", "_", "_", "__", "__"]
        );
        // Nested emphasis of the same marker.
        assert_eq!(
            items("*a *b* c*", Language::Null).len(),
            4,
            "both pairs are found"
        );
        // Prose: no emphasis.
        assert!(items("5 * 3 * 2", Language::Null).is_empty());
        assert!(items("snake_case_name and __init__.py", Language::Null).len() == 2);
        assert!(items("*.txt and *.po files", Language::Null).is_empty());
        assert!(items("Log files (*.*) or (*.icc,*.icm)", Language::Null).is_empty());
        assert!(items(r#"choice *%s in "*%s *%s""#, Language::C).is_empty());
        assert_eq!(items("**Note:** read", Language::Null).len(), 2);
        assert_eq!(items("*[link](https://a.com)*", Language::Null).len(), 3);
        assert!(items("* item", Language::Null).is_empty());
        assert!(items(r#""*" means all, "*" or "*,!b""#, Language::Null).is_empty());
        assert!(items(r#""log.*" and "tmp.*""#, Language::Null).is_empty());
        assert!(items(r#""a *", "*,!b""#, Language::Null).is_empty());
        assert_eq!(items("**[link](https://a.com)**", Language::Null).len(), 3);
        assert!(items("**unclosed", Language::Null).is_empty());
        assert!(items(r"\*not emphasis\*", Language::Null).is_empty());
    }

    #[test]
    fn test_iter_formats_skipped() {
        // A format string is a word next to an emphasis marker.
        assert_eq!(items("*%s* and _%d_", Language::C).len(), 4);
        assert!(items("%s_a_", Language::C).is_empty());
        // A code span may hold a format string.
        assert_eq!(
            items("run `%s`", Language::C),
            vec![(MarkdownKind::CodeSpan, "`%s`")]
        );
    }

    #[test]
    fn test_no_markdown() {
        let diags = check_markdown(
            r#"
msgid "tested"
msgstr "testé"

msgid "use `foo' or 5 * 3"
msgstr "utilisez « foo » ou 5 * 3"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_markdown_ok() {
        let diags = check_markdown(
            r#"
msgid "Read [the **manual**](https://example.com/manual) or run `make help`"
msgstr "Lancez `make help` ou lisez [le __manuel__](https://example.com/manual)"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_markdown_ok_plain_text() {
        let diags = check_markdown(
            r#"
msgid "run `copy <file>` on `user@host`"
msgstr "lancez `copy <fichier>` sur `utilisateur@hôte`"

msgid "\"*\" means all, see \"log.*\" and \"tmp.*\""
msgstr "„*” signifie tout, voir „log.*” et „tmp.*”"
"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_markdown_error() {
        let diags = check_markdown(
            r#"
msgid "Read [the **manual**](https://example.com/manual) or run `make help`"
msgstr "Lisez [le manuel] (https://example.com/manuel) ou lancez make help"

msgid "run `make` or `make all`"
msgstr "lancez `make` ou make all"

msgid "see [a](https://a.com)"
msgstr "voir [a](https://a.com) et [b](https://b.com)"

msgid "this is **important**"
msgstr "ceci est *important*"
"#,
        );
        let messages: Vec<_> = diags.iter().map(|d| d.message.as_ref()).collect();
        assert_eq!(
            messages,
            vec![
                "missing Markdown code spans (1 / 0)",
                "missing Markdown links (1 / 0)",
                "missing Markdown emphasis markers (2 / 0)",
                "missing Markdown code spans (2 / 1)",
                "extra Markdown links (1 / 2)",
                "different Markdown emphasis markers",
            ]
        );
        assert!(diags.iter().all(|d| d.severity == Severity::Warning));
    }

    #[test]
    fn test_different_fix_replaces_each_in_place() {
        let diags = check_markdown(
            r#"
msgid "see [a](https://a.com) and [b](https://b.com), run `make`"
msgstr "voir [a](https://a.fr) et [b](https://b.com), lancez `make`"
"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "different Markdown link targets");
        let fix = diags[0].fix.as_ref().expect("fix attached");
        assert!(!fix.safe);
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].replacement, "https://a.com");
    }

    #[test]
    fn test_no_fix_for_counts_and_emphasis() {
        let diags = check_markdown(
            r#"
msgid "run `make`"
msgstr "lancez make"

msgid "this is **important**"
msgstr "ceci est *important*"
"#,
        );
        assert_eq!(diags.len(), 2);
        assert!(diags.iter().all(|d| d.fix.is_none()));
    }
}
