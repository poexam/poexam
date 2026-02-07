// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashSet;

use crate::{
    args,
    checker::Checker,
    po::entry::Entry,
    rules::{
        blank, brackets, c_formats, double_quotes, double_spaces, encoding, escapes, fuzzy,
        newlines, obsolete, pipes, plurals, punc, spelling, tabs, unchanged, untranslated,
        whitespace,
    },
};

pub type Rule = Box<dyn RuleChecker + Sync>;

const SPECIAL_RULES: [&str; 2] = ["all", "checks"];

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
    fn is_check(&self) -> bool {
        true
    }
    fn severity(&self) -> crate::diagnostic::Severity;
    fn check_entry(&self, _checker: &mut Checker, _entry: &Entry) {}
    fn check_ctxt(&self, _checker: &mut Checker, _entry: &Entry, _ctxt: &str) {}
    fn check_msg(&self, _checker: &mut Checker, _entry: &Entry, _msgid: &str, _msgstr: &str) {}
}

pub fn get_all_rules() -> Vec<Rule> {
    vec![
        Box::new(blank::BlankRule {}),
        Box::new(brackets::BracketsRule {}),
        Box::new(c_formats::CFormatsRule {}),
        Box::new(double_quotes::DoubleQuotesRule {}),
        Box::new(double_spaces::DoubleSpacesRule {}),
        Box::new(encoding::EncodingRule {}),
        Box::new(escapes::EscapesRule {}),
        Box::new(fuzzy::FuzzyRule {}),
        Box::new(newlines::NewlinesRule {}),
        Box::new(obsolete::ObsoleteRule {}),
        Box::new(pipes::PipesRule {}),
        Box::new(plurals::PluralsRule {}),
        Box::new(punc::PuncEndRule {}),
        Box::new(punc::PuncStartRule {}),
        Box::new(spelling::SpellingCtxtRule {}),
        Box::new(spelling::SpellingIdRule {}),
        Box::new(spelling::SpellingStrRule {}),
        Box::new(tabs::TabsRule {}),
        Box::new(unchanged::UnchangedRule {}),
        Box::new(untranslated::UntranslatedRule {}),
        Box::new(whitespace::WhitespaceEndRule {}),
        Box::new(whitespace::WhitespaceStartRule {}),
    ]
}

/// Get unknown rule names from a list of names compared to all available rules.
pub fn get_unknown_rules<'a>(
    names: &'a [&str],
    all_rules_names: &HashSet<&'static str>,
) -> Vec<&'a str> {
    let selected_rules_names = names.iter().copied().collect::<HashSet<_>>();
    let mut unknown_rules_names: HashSet<&str> = selected_rules_names
        .difference(all_rules_names)
        .copied()
        .collect();
    // Some special rules like "all" and "check" are always known, we just ignore them.
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
pub fn get_selected_rules(args: &args::CheckArgs) -> Result<Rules, Box<dyn std::error::Error>> {
    let all_severities = args.severity.is_empty();
    let all_rules: Vec<Rule> = get_all_rules()
        .into_iter()
        .filter(|r| all_severities || args.severity.contains(&r.severity()))
        .collect();
    let check_rules: Vec<Rule> = get_all_rules()
        .into_iter()
        .filter(|r| r.is_check() && (all_severities || args.severity.contains(&r.severity())))
        .collect();
    let all_rules_names: HashSet<&'static str> = all_rules.iter().map(|r| r.name()).collect();
    let mut selected_rules: Vec<Rule> = Vec::new();

    if let Some(select_str) = &args.select {
        let names: Vec<&str> = select_str.split(',').map(str::trim).collect();
        let unknown_rules_names = get_unknown_rules(&names, &all_rules_names);
        if !unknown_rules_names.is_empty() {
            return Err(
                format!("unknown selected rules: {}", unknown_rules_names.join(", ")).into(),
            );
        }
        if names.contains(&"all") {
            selected_rules = all_rules;
        } else if names.contains(&"checks") {
            selected_rules = check_rules;
        } else {
            for rule in all_rules {
                if names.contains(&rule.name()) {
                    selected_rules.push(rule);
                }
            }
        }
    } else {
        // If no selection was provided, start with all default rules.
        for rule in all_rules {
            if rule.is_default() {
                selected_rules.push(rule);
            }
        }
    }

    // Remove the ignored rules.
    if let Some(ignore_str) = &args.ignore {
        let names: Vec<&str> = ignore_str.split(',').map(str::trim).collect();
        let unknown_rules_names = get_unknown_rules(&names, &all_rules_names);
        if !unknown_rules_names.is_empty() {
            return Err(format!(
                "unknown rules to ignore: {}",
                unknown_rules_names.join(", ")
            )
            .into());
        }
        selected_rules.retain(|rule| !names.contains(&rule.name()));
    }

    Ok(Rules::new(selected_rules))
}

/// Display rules used to check PO files.
pub fn run_rules(_args: &args::RulesArgs) -> i32 {
    let rules = get_all_rules();
    let default_rules: Vec<&Rule> = rules.iter().filter(|r| r.is_default()).collect();
    let other_rules: Vec<&Rule> = rules.iter().filter(|r| !r.is_default()).collect();
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
    println!("Special rules:");
    println!("  all: select all rules");
    println!(
        "  checks: select rules that actually check (all rules except fuzzy, obsolete and untranslated)"
    );
    0
}
