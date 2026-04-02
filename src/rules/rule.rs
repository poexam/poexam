// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Load rules and execute the `rules` command.

use std::collections::HashSet;

use crate::{
    args,
    checker::Checker,
    config::Config,
    po::entry::Entry,
    rules::{
        blank, brackets, changed, compilation, double_quotes, double_spaces, encoding, escapes,
        formats, fuzzy, long, newlines, obsolete, pipes, plurals, punc, short, spelling, tabs,
        unchanged, untranslated, urls, whitespace,
    },
};

pub type Rule = Box<dyn RuleChecker + Send + Sync>;

const SPECIAL_RULES: [&str; 4] = ["all", "checks", "default", "spelling"];

#[derive(Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct Rules {
    pub enabled: Vec<Rule>,
    pub fuzzy_rule: bool,
    pub obsolete_rule: bool,
    pub untranslated_rule: bool,
    pub spelling_ctxt_rule: bool,
    pub spelling_id_rule: bool,
    pub spelling_str_rule: bool,
}

impl<'a> Default for &'a Rules {
    fn default() -> &'a Rules {
        static RULES: Rules = Rules {
            enabled: vec![],
            fuzzy_rule: false,
            obsolete_rule: false,
            untranslated_rule: false,
            spelling_ctxt_rule: false,
            spelling_id_rule: false,
            spelling_str_rule: false,
        };
        &RULES
    }
}

impl std::fmt::Display for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} [{}]", self.name(), self.severity())
    }
}

impl Rules {
    pub fn new(rules: Vec<Rule>) -> Self {
        let fuzzy_rule = rules.iter().any(|r| r.name() == "fuzzy");
        let obsolete_rule = rules.iter().any(|r| r.name() == "obsolete");
        let untranslated_rule = rules.iter().any(|r| r.name() == "untranslated");
        let spelling_ctxt_rule = rules.iter().any(|r| r.name() == "spelling-ctxt");
        let spelling_id_rule = rules.iter().any(|r| r.name() == "spelling-id");
        let spelling_str_rule = rules.iter().any(|r| r.name() == "spelling-str");
        Self {
            enabled: rules,
            fuzzy_rule,
            obsolete_rule,
            untranslated_rule,
            spelling_ctxt_rule,
            spelling_id_rule,
            spelling_str_rule,
        }
    }
}

pub trait RuleChecker {
    fn name(&self) -> &'static str;
    fn is_default(&self) -> bool;
    fn is_check(&self) -> bool;
    fn severity(&self) -> crate::diagnostic::Severity;
    fn check_file(&self, _checker: &mut Checker) {}
    fn check_entry(&self, _checker: &mut Checker, _entry: &Entry) {}
    fn check_ctxt(&self, _checker: &mut Checker, _entry: &Entry, _ctxt: &str) {}
    fn check_msg(&self, _checker: &mut Checker, _entry: &Entry, _msgid: &str, _msgstr: &str) {}
}

pub fn get_all_rules() -> Vec<Rule> {
    vec![
        Box::new(blank::BlankRule {}),
        Box::new(brackets::BracketsRule {}),
        Box::new(changed::ChangedRule {}),
        Box::new(compilation::CompilationRule {}),
        Box::new(double_quotes::DoubleQuotesRule {}),
        Box::new(double_spaces::DoubleSpacesRule {}),
        Box::new(encoding::EncodingRule {}),
        Box::new(escapes::EscapesRule {}),
        Box::new(formats::FormatsRule {}),
        Box::new(fuzzy::FuzzyRule {}),
        Box::new(long::LongRule {}),
        Box::new(newlines::NewlinesRule {}),
        Box::new(obsolete::ObsoleteRule {}),
        Box::new(pipes::PipesRule {}),
        Box::new(plurals::PluralsRule {}),
        Box::new(punc::PuncEndRule {}),
        Box::new(punc::PuncStartRule {}),
        Box::new(short::ShortRule {}),
        Box::new(spelling::SpellingCtxtRule {}),
        Box::new(spelling::SpellingIdRule {}),
        Box::new(spelling::SpellingStrRule {}),
        Box::new(tabs::TabsRule {}),
        Box::new(unchanged::UnchangedRule {}),
        Box::new(untranslated::UntranslatedRule {}),
        Box::new(urls::UrlsRule {}),
        Box::new(whitespace::WhitespaceEndRule {}),
        Box::new(whitespace::WhitespaceStartRule {}),
    ]
}

/// Get unknown rule names from a list of names compared to all available rules.
pub fn get_unknown_rules<'a>(
    names: &'a [String],
    all_rules_names: &HashSet<&'static str>,
) -> Vec<&'a str> {
    let selected_rules_names = names
        .iter()
        .map(std::convert::AsRef::as_ref)
        .collect::<HashSet<_>>();
    let mut unknown_rules_names: HashSet<&str> = selected_rules_names
        .difference(all_rules_names)
        .copied()
        .collect();
    // Some special rules like "all" and "checks" are always known, we just ignore them.
    for name in SPECIAL_RULES {
        unknown_rules_names.remove(name);
    }
    if unknown_rules_names.is_empty() {
        return vec![];
    }
    let mut unknown = unknown_rules_names.iter().copied().collect::<Vec<_>>();
    unknown.sort_unstable();
    unknown
}

/// Get the selected rules based on command line parameters `--select` and `--ignore`.
///
/// If `--select` is provided, only the specified rules are included.
/// If `--select` is not provided, all default rules are included.
/// Then, any rules specified in `--ignore` are removed from the selection.
pub fn get_selected_rules(config: &Config) -> Result<Rules, Box<dyn std::error::Error>> {
    let mut all_rules: Vec<Rule> = get_all_rules();
    let all_rules_names: HashSet<&'static str> = all_rules.iter().map(|r| r.name()).collect();
    let mut selected_rules: Vec<Rule> = Vec::new();

    let unknown_rules_names = get_unknown_rules(&config.check.select, &all_rules_names);
    if !unknown_rules_names.is_empty() {
        return Err(format!("unknown selected rules: {}", unknown_rules_names.join(", ")).into());
    }
    for name in &config.check.select {
        if name == "all" {
            selected_rules.extend(all_rules.extract_if(.., |_| true));
        } else if name == "checks" {
            selected_rules.extend(all_rules.extract_if(.., |rule| rule.is_check()));
        } else if name == "default" {
            selected_rules.extend(all_rules.extract_if(.., |rule| rule.is_default()));
        } else if name == "spelling" {
            selected_rules
                .extend(all_rules.extract_if(.., |rule| rule.name().starts_with("spelling-")));
        } else {
            selected_rules.extend(all_rules.extract_if(.., |rule| rule.name() == name));
        }
    }

    // Remove the ignored rules.
    let unknown_rules_names = get_unknown_rules(&config.check.ignore, &all_rules_names);
    if !unknown_rules_names.is_empty() {
        return Err(format!(
            "unknown rules to ignore: {}",
            unknown_rules_names.join(", ")
        )
        .into());
    }
    selected_rules.retain(|rule| !config.check.ignore.iter().any(|r| r == rule.name()));

    // Retain only rules with the specified severities.
    let all_severities = config.check.severity.is_empty();
    selected_rules
        .retain(|rule| all_severities || config.check.severity.contains(&rule.severity()));

    // Sort rules by name.
    selected_rules.sort_by(|a, b| a.name().cmp(b.name()));

    Ok(Rules::new(selected_rules))
}

/// Display rules used to check PO files.
pub fn run_rules(_args: &args::RulesArgs) -> i32 {
    let rules = get_all_rules();
    let default_rules: Vec<&Rule> = rules.iter().filter(|r| r.is_default()).collect();
    let other_rules: Vec<&Rule> = rules.iter().filter(|r| !r.is_default()).collect();
    let non_check_rules: Vec<&Rule> = rules.iter().filter(|r| !r.is_check()).collect();
    if default_rules.is_empty() {
        println!("No default rules.");
    } else {
        println!("{} default rules:", default_rules.len());
        for rule in &default_rules {
            println!("  {rule}");
        }
    }
    if other_rules.is_empty() {
        println!("No other rules.");
    } else {
        println!("{} other rules:", other_rules.len());
        for rule in &other_rules {
            println!("  {rule}");
        }
    }
    println!("Total: {} rules", default_rules.len() + other_rules.len());
    println!();
    println!("Special rules to enable multiple rules at once:");
    println!("  all: all available rules");
    println!(
        "  checks: all rules that actually check (all rules except: {})",
        non_check_rules
            .iter()
            .map(|rule| rule.name())
            .collect::<Vec<_>>()
            .join(", "),
    );
    println!(
        "  default: default rules (can be used to add extra rules, e.g. \"default,spelling,fuzzy\")"
    );
    println!("  spelling: all spelling rules");
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::Severity;

    fn rule_names(rules: &Rules) -> Vec<&str> {
        rules.enabled.iter().map(|r| r.name()).collect()
    }

    fn make_config(select: Vec<&str>, ignore: Vec<&str>, severity: Vec<Severity>) -> Config {
        let mut config = Config::default();
        config.check.select = select.into_iter().map(String::from).collect();
        config.check.ignore = ignore.into_iter().map(String::from).collect();
        config.check.severity = severity;
        config
    }

    fn all_rules_name_set() -> HashSet<&'static str> {
        get_all_rules().iter().map(|r| r.name()).collect()
    }

    #[test]
    fn test_get_all_rules() {
        let rules = get_all_rules();
        assert!(!rules.is_empty());
        let names: HashSet<&str> = rules.iter().map(|r| r.name()).collect();
        assert_eq!(names.len(), rules.len(), "rule names must be unique");
        assert!(
            rules.iter().any(|r| r.is_default()),
            "should have at least one default rule"
        );
        assert!(
            rules.iter().any(|r| !r.is_default()),
            "should have at least one non-default rule"
        );
        assert!(
            rules.iter().any(|r| r.is_check()),
            "should have at least one check rule"
        );
        assert!(
            rules.iter().any(|r| !r.is_check()),
            "should have at least one non-check rule"
        );
    }

    #[test]
    fn test_rules_new_empty() {
        let rules = Rules::new(vec![]);
        assert!(rules.enabled.is_empty());
        assert!(!rules.fuzzy_rule);
        assert!(!rules.obsolete_rule);
        assert!(!rules.untranslated_rule);
        assert!(!rules.spelling_ctxt_rule);
        assert!(!rules.spelling_id_rule);
        assert!(!rules.spelling_str_rule);
    }

    #[test]
    fn test_rules_new_fuzzy_flag() {
        let rules = Rules::new(vec![Box::new(fuzzy::FuzzyRule {})]);
        assert!(rules.fuzzy_rule);
        assert!(!rules.obsolete_rule);
        assert!(!rules.untranslated_rule);
    }

    #[test]
    fn test_rules_new_obsolete_flag() {
        let rules = Rules::new(vec![Box::new(obsolete::ObsoleteRule {})]);
        assert!(!rules.fuzzy_rule);
        assert!(rules.obsolete_rule);
        assert!(!rules.untranslated_rule);
    }

    #[test]
    fn test_rules_new_untranslated_flag() {
        let rules = Rules::new(vec![Box::new(untranslated::UntranslatedRule {})]);
        assert!(!rules.fuzzy_rule);
        assert!(!rules.obsolete_rule);
        assert!(rules.untranslated_rule);
    }

    #[test]
    fn test_rules_new_spelling_flags() {
        let rules = Rules::new(vec![
            Box::new(spelling::SpellingCtxtRule {}),
            Box::new(spelling::SpellingIdRule {}),
            Box::new(spelling::SpellingStrRule {}),
        ]);
        assert!(rules.spelling_ctxt_rule);
        assert!(rules.spelling_id_rule);
        assert!(rules.spelling_str_rule);
        assert!(!rules.fuzzy_rule);
    }

    #[test]
    fn test_rules_new_all_flags() {
        let rules = Rules::new(vec![
            Box::new(fuzzy::FuzzyRule {}),
            Box::new(obsolete::ObsoleteRule {}),
            Box::new(untranslated::UntranslatedRule {}),
            Box::new(spelling::SpellingCtxtRule {}),
            Box::new(spelling::SpellingIdRule {}),
            Box::new(spelling::SpellingStrRule {}),
        ]);
        assert!(rules.fuzzy_rule);
        assert!(rules.obsolete_rule);
        assert!(rules.untranslated_rule);
        assert!(rules.spelling_ctxt_rule);
        assert!(rules.spelling_id_rule);
        assert!(rules.spelling_str_rule);
        assert_eq!(rules.enabled.len(), 6);
    }

    #[test]
    fn test_rules_new_non_special_rule() {
        let rules = Rules::new(vec![Box::new(blank::BlankRule {})]);
        assert_eq!(rules.enabled.len(), 1);
        assert!(!rules.fuzzy_rule);
        assert!(!rules.obsolete_rule);
        assert!(!rules.untranslated_rule);
        assert!(!rules.spelling_ctxt_rule);
        assert!(!rules.spelling_id_rule);
        assert!(!rules.spelling_str_rule);
    }

    #[test]
    fn test_rules_default_ref() {
        let rules: &Rules = Default::default();
        assert!(rules.enabled.is_empty());
        assert!(!rules.fuzzy_rule);
        assert!(!rules.obsolete_rule);
        assert!(!rules.untranslated_rule);
        assert!(!rules.spelling_ctxt_rule);
        assert!(!rules.spelling_id_rule);
        assert!(!rules.spelling_str_rule);
    }

    #[test]
    fn test_rule_display() {
        let rule: Rule = Box::new(blank::BlankRule {});
        let display = format!("{rule}");
        assert!(display.contains("blank"));
        assert!(display.contains('['));
        assert!(display.contains(']'));
    }

    #[test]
    fn test_get_unknown_rules_empty_names() {
        let names: Vec<String> = vec![];
        let all_names = all_rules_name_set();
        let unknown = get_unknown_rules(&names, &all_names);
        assert!(unknown.is_empty());
    }

    #[test]
    fn test_get_unknown_rules_all_known() {
        let names = vec![String::from("blank"), String::from("fuzzy")];
        let all_names = all_rules_name_set();
        let unknown = get_unknown_rules(&names, &all_names);
        assert!(unknown.is_empty());
    }

    #[test]
    fn test_get_unknown_rules_one_unknown() {
        let names = vec![String::from("blank"), String::from("nonexistent")];
        let all_names = all_rules_name_set();
        let unknown = get_unknown_rules(&names, &all_names);
        assert_eq!(unknown, vec!["nonexistent"]);
    }

    #[test]
    fn test_get_unknown_rules_multiple_unknown() {
        let names = vec![
            String::from("blank"),
            String::from("zzz-unknown"),
            String::from("aaa-unknown"),
        ];
        let all_names = all_rules_name_set();
        let unknown = get_unknown_rules(&names, &all_names);
        // Results should be sorted.
        assert_eq!(unknown, vec!["aaa-unknown", "zzz-unknown"]);
    }

    #[test]
    fn test_get_unknown_rules_special_rules_ignored() {
        // Rules "all", "checks", "default", "spelling" are special and should NOT be reported as unknown.
        let names = vec![
            String::from("all"),
            String::from("checks"),
            String::from("default"),
            String::from("spelling"),
        ];
        let all_names = all_rules_name_set();
        let unknown = get_unknown_rules(&names, &all_names);
        assert!(unknown.is_empty());
    }

    #[test]
    fn test_get_unknown_rules_special_mixed_with_unknown() {
        let names = vec![String::from("all"), String::from("does-not-exist")];
        let all_names = all_rules_name_set();
        let unknown = get_unknown_rules(&names, &all_names);
        assert_eq!(unknown, vec!["does-not-exist"]);
    }

    #[test]
    fn test_get_selected_rules_default() {
        let config = make_config(vec!["default"], vec![], vec![]);
        let rules = get_selected_rules(&config).unwrap();
        let names = rule_names(&rules);
        let all = get_all_rules();
        // All default rules should be present.
        let expected_defaults: Vec<&str> = all
            .iter()
            .filter(|r| r.is_default())
            .map(|r| r.name())
            .collect();
        for name in &expected_defaults {
            assert!(names.contains(name), "missing default rule: {name}");
        }
        // Non-default rules should be absent.
        let non_defaults: Vec<&str> = all
            .iter()
            .filter(|r| !r.is_default())
            .map(|r| r.name())
            .collect();
        for name in &non_defaults {
            assert!(!names.contains(name), "unexpected non-default rule: {name}");
        }
    }

    #[test]
    fn test_get_selected_rules_all() {
        let config = make_config(vec!["all"], vec![], vec![]);
        let rules = get_selected_rules(&config).unwrap();
        let all = get_all_rules();
        assert_eq!(rules.enabled.len(), all.len());
    }

    #[test]
    fn test_get_selected_rules_checks() {
        let config = make_config(vec!["checks"], vec![], vec![]);
        let rules = get_selected_rules(&config).unwrap();
        let names = rule_names(&rules);
        let all = get_all_rules();
        // All check rules should be present.
        let expected_checks: Vec<&str> = all
            .iter()
            .filter(|r| r.is_check())
            .map(|r| r.name())
            .collect();
        for name in &expected_checks {
            assert!(names.contains(name), "missing check rule: {name}");
        }
        // Non-check rules should be absent.
        let non_checks: Vec<&str> = all
            .iter()
            .filter(|r| !r.is_check())
            .map(|r| r.name())
            .collect();
        for name in &non_checks {
            assert!(!names.contains(name), "unexpected non-check rule: {name}");
        }
    }

    #[test]
    fn test_get_selected_rules_spelling() {
        let config = make_config(vec!["spelling"], vec![], vec![]);
        let rules = get_selected_rules(&config).unwrap();
        let names = rule_names(&rules);
        assert!(names.contains(&"spelling-ctxt"));
        assert!(names.contains(&"spelling-id"));
        assert!(names.contains(&"spelling-str"));
        assert_eq!(rules.enabled.len(), 3);
    }

    #[test]
    fn test_get_selected_rules_single_rule() {
        let config = make_config(vec!["blank"], vec![], vec![]);
        let rules = get_selected_rules(&config).unwrap();
        let names = rule_names(&rules);
        assert_eq!(names, vec!["blank"]);
    }

    #[test]
    fn test_get_selected_rules_multiple_explicit() {
        let config = make_config(vec!["blank", "fuzzy", "tabs"], vec![], vec![]);
        let rules = get_selected_rules(&config).unwrap();
        let names = rule_names(&rules);
        assert_eq!(names, vec!["blank", "fuzzy", "tabs"]);
    }

    #[test]
    fn test_get_selected_rules_default_plus_spelling() {
        let config = make_config(vec!["default", "spelling"], vec![], vec![]);
        let rules = get_selected_rules(&config).unwrap();
        let names = rule_names(&rules);
        assert!(names.contains(&"spelling-ctxt"));
        assert!(names.contains(&"spelling-id"));
        assert!(names.contains(&"spelling-str"));
        // Default rules should also be present.
        assert!(names.contains(&"blank"));
    }

    #[test]
    fn test_get_selected_rules_sorted_by_name() {
        let config = make_config(vec!["all"], vec![], vec![]);
        let rules = get_selected_rules(&config).unwrap();
        let names = rule_names(&rules);
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted, "rules should be sorted by name");
    }

    #[test]
    fn test_get_selected_rules_ignore() {
        let config = make_config(vec!["default"], vec!["blank", "tabs"], vec![]);
        let rules = get_selected_rules(&config).unwrap();
        let names = rule_names(&rules);
        assert!(!names.contains(&"blank"));
        assert!(!names.contains(&"tabs"));
    }

    #[test]
    fn test_get_selected_rules_ignore_all_selected() {
        let config = make_config(vec!["blank"], vec!["blank"], vec![]);
        let rules = get_selected_rules(&config).unwrap();
        assert!(rules.enabled.is_empty());
    }

    #[test]
    fn test_get_selected_rules_severity_filter() {
        let config = make_config(
            vec!["all"],
            vec!["punc-start", "punc-end"],
            vec![Severity::Error],
        );
        let rules = get_selected_rules(&config).unwrap();
        for rule in &rules.enabled {
            assert_eq!(
                rule.severity(),
                Severity::Error,
                "rule '{}' should have Error severity",
                rule.name()
            );
        }
    }

    #[test]
    fn test_get_selected_rules_severity_filter_warning() {
        let config = make_config(vec!["all"], vec![], vec![Severity::Warning]);
        let rules = get_selected_rules(&config).unwrap();
        for rule in &rules.enabled {
            assert_eq!(
                rule.severity(),
                Severity::Warning,
                "rule '{}' should have Warning severity",
                rule.name()
            );
        }
    }

    #[test]
    fn test_get_selected_rules_severity_filter_multiple() {
        let config = make_config(
            vec!["all"],
            vec![],
            vec![Severity::Warning, Severity::Error],
        );
        let rules = get_selected_rules(&config).unwrap();
        for rule in &rules.enabled {
            assert!(
                rule.severity() == Severity::Warning || rule.severity() == Severity::Error,
                "rule '{}' has unexpected severity",
                rule.name()
            );
        }
    }

    #[test]
    fn test_get_selected_rules_empty_severity_means_all() {
        let config = make_config(vec!["all"], vec![], vec![]);
        let rules = get_selected_rules(&config).unwrap();
        let all = get_all_rules();
        assert_eq!(rules.enabled.len(), all.len());
    }

    #[test]
    fn test_get_selected_rules_unknown_select_error() {
        let config = make_config(vec!["nonexistent-rule"], vec![], vec![]);
        let result = get_selected_rules(&config);
        match result {
            Err(err) => {
                let err = err.to_string();
                assert!(
                    err.contains("unknown selected rules"),
                    "error should mention unknown selected rules, got: {err}"
                );
                assert!(err.contains("nonexistent-rule"));
            }
            Ok(_) => panic!("expected error for unknown selected rule"),
        }
    }

    #[test]
    fn test_get_selected_rules_unknown_ignore_error() {
        let config = make_config(vec!["default"], vec!["nonexistent-rule"], vec![]);
        let result = get_selected_rules(&config);
        match result {
            Err(err) => {
                let err = err.to_string();
                assert!(
                    err.contains("unknown rules to ignore"),
                    "error should mention unknown rules to ignore, got: {err}"
                );
                assert!(err.contains("nonexistent-rule"));
            }
            Ok(_) => panic!("expected error for unknown ignored rule"),
        }
    }

    #[test]
    fn test_get_selected_rules_flags_set_correctly() {
        let config = make_config(vec!["all"], vec![], vec![]);
        let rules = get_selected_rules(&config).unwrap();
        assert!(rules.fuzzy_rule);
        assert!(rules.obsolete_rule);
        assert!(rules.untranslated_rule);
        assert!(rules.spelling_ctxt_rule);
        assert!(rules.spelling_id_rule);
        assert!(rules.spelling_str_rule);
    }

    #[test]
    fn test_get_selected_rules_default_flags() {
        let config = make_config(vec!["default"], vec![], vec![]);
        let rules = get_selected_rules(&config).unwrap();
        // Rules "fuzzy", "obsolete", "untranslated" are not default rules.
        assert!(!rules.fuzzy_rule);
        assert!(!rules.obsolete_rule);
        assert!(!rules.untranslated_rule);
        // Spelling rules are not default either.
        assert!(!rules.spelling_ctxt_rule);
        assert!(!rules.spelling_id_rule);
        assert!(!rules.spelling_str_rule);
    }

    #[test]
    fn test_run_rules_returns_zero() {
        let args = args::RulesArgs;
        let exit_code = run_rules(&args);
        assert_eq!(exit_code, 0);
    }
}
