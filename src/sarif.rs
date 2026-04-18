// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! SARIF (Static Analysis Results Interchange Format) v2.1.0 output.

use std::borrow::Cow;
use std::collections::{BTreeMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};

use serde::Serialize;

use crate::checker::CheckFileResult;
use crate::diagnostic::Severity;

const SARIF_SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";
const SARIF_VERSION: &str = "2.1.0";

/// Top-level SARIF log object.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifLog<'a> {
    #[serde(rename = "$schema")]
    pub schema: &'static str,
    pub version: &'static str,
    pub runs: Vec<SarifRun<'a>>,
}

/// A single analysis run.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifRun<'a> {
    pub tool: SarifTool<'a>,
    pub results: Vec<SarifResult<'a>>,
    pub column_kind: &'static str,
}

/// Tool information.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifTool<'a> {
    pub driver: SarifToolComponent<'a>,
}

/// Tool component (driver) with rules.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifToolComponent<'a> {
    pub name: &'static str,
    pub semantic_version: &'static str,
    pub rules: Vec<SarifReportingDescriptor<'a>>,
}

/// A rule descriptor.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifReportingDescriptor<'a> {
    pub id: &'static str,
    pub short_description: SarifMessage<'a>,
    pub full_description: SarifMessage<'a>,
    pub default_configuration: SarifConfiguration,
    pub help: SarifHelp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<SarifRuleProperties>,
}

/// Default configuration for a rule.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifConfiguration {
    pub level: &'static str,
}

/// Help text for a rule.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifHelp {
    pub text: &'static str,
}

/// Optional rule properties.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifRuleProperties {
    pub precision: &'static str,
}

/// A message with text content.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifMessage<'a> {
    pub text: Cow<'a, str>,
}

/// A single result (finding).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifResult<'a> {
    pub rule_id: &'static str,
    pub rule_index: usize,
    pub level: &'static str,
    pub message: SarifMessage<'a>,
    pub locations: Vec<SarifLocation<'a>>,
    pub partial_fingerprints: SarifFingerprints,
}

/// Fingerprints for deduplication.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifFingerprints {
    pub primary_location_line_hash: String,
}

/// A location in the source code.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifLocation<'a> {
    pub physical_location: SarifPhysicalLocation<'a>,
}

/// Physical location with artifact and region.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifPhysicalLocation<'a> {
    pub artifact_location: SarifArtifactLocation<'a>,
    pub region: SarifRegion,
}

/// Artifact location (file path).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifArtifactLocation<'a> {
    pub uri: Cow<'a, str>,
}

/// Region in a file.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifRegion {
    pub start_line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_column: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_column: Option<usize>,
}

/// Map poexam severity to SARIF level string.
fn sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "note",
        Severity::Warning => "warning",
        Severity::Error => "error",
    }
}

/// Compute a fingerprint hash for deduplication.
fn compute_fingerprint(rule: &str, path: &str, line: usize, message: &str) -> String {
    let mut hasher = DefaultHasher::new();
    rule.hash(&mut hasher);
    path.hash(&mut hasher);
    line.hash(&mut hasher);
    message.hash(&mut hasher);
    format!("{:016x}:1", hasher.finish())
}

/// Build a SARIF log from check results.
pub fn build_sarif(result: &[CheckFileResult]) -> SarifLog<'_> {
    // Collect all unique rules across all checked files, preserving order by name.
    let mut rules_map: BTreeMap<&str, (&str, Severity)> = BTreeMap::new();
    for file in result {
        for rule in &file.rules.enabled {
            rules_map
                .entry(rule.name())
                .or_insert((rule.description(), rule.severity()));
        }
    }

    // Build rule descriptors and index lookup.
    let mut rule_index_map: BTreeMap<&str, usize> = BTreeMap::new();
    let mut sarif_rules: Vec<SarifReportingDescriptor> = Vec::new();
    for (idx, (name, (description, severity))) in rules_map.iter().enumerate() {
        rule_index_map.insert(name, idx);
        sarif_rules.push(SarifReportingDescriptor {
            id: name,
            short_description: SarifMessage {
                text: Cow::Borrowed(description),
            },
            full_description: SarifMessage {
                text: Cow::Borrowed(description),
            },
            default_configuration: SarifConfiguration {
                level: sarif_level(*severity),
            },
            help: SarifHelp { text: description },
            properties: Some(SarifRuleProperties {
                precision: "very-high",
            }),
        });
    }

    // Build results from diagnostics.
    let mut sarif_results: Vec<SarifResult> = Vec::new();
    for file in result {
        for diag in &file.diagnostics {
            let path_str = diag.path.to_string_lossy();

            // Find the first line with a non-zero line number.
            let first_line = diag
                .lines
                .iter()
                .find(|l| l.line_number > 0)
                .or(diag.lines.first());

            let start_line =
                first_line.map_or(1, |l| if l.line_number > 0 { l.line_number } else { 1 });

            // Convert byte-offset highlights to character-position columns (1-based for SARIF).
            let (start_column, end_column) = first_line
                .and_then(|l| {
                    l.highlights.first().map(|(s, e)| {
                        let sc = l.message[..*s].chars().count() + 1;
                        let ec = l.message[..*e].chars().count() + 1;
                        (Some(sc), Some(ec))
                    })
                })
                .unwrap_or((None, None));

            let message_text = diag.build_message();
            let fingerprint = compute_fingerprint(diag.rule, &path_str, start_line, &message_text);

            let rule_index = rule_index_map.get(diag.rule).copied().unwrap_or(0);

            sarif_results.push(SarifResult {
                rule_id: diag.rule,
                rule_index,
                level: sarif_level(diag.severity),
                message: SarifMessage { text: message_text },
                locations: vec![SarifLocation {
                    physical_location: SarifPhysicalLocation {
                        artifact_location: SarifArtifactLocation { uri: path_str },
                        region: SarifRegion {
                            start_line,
                            start_column,
                            end_column,
                        },
                    },
                }],
                partial_fingerprints: SarifFingerprints {
                    primary_location_line_hash: fingerprint,
                },
            });
        }
    }

    SarifLog {
        schema: SARIF_SCHEMA,
        version: SARIF_VERSION,
        runs: vec![SarifRun {
            tool: SarifTool {
                driver: SarifToolComponent {
                    name: env!("CARGO_PKG_NAME"),
                    semantic_version: env!("CARGO_PKG_VERSION"),
                    rules: sarif_rules,
                },
            },
            results: sarif_results,
            column_kind: "unicodeCodePoints",
        }],
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::PathBuf;

    use super::*;
    use crate::checker::CheckFileResult;
    use crate::config::Config;
    use crate::diagnostic::{Diagnostic, DiagnosticLine, Severity};
    use crate::rules::rule::{RuleChecker, Rules};

    struct MockRule {
        name: &'static str,
        description: &'static str,
        severity: Severity,
    }

    impl RuleChecker for MockRule {
        fn name(&self) -> &'static str {
            self.name
        }

        fn description(&self) -> &'static str {
            self.description
        }

        fn is_default(&self) -> bool {
            true
        }

        fn is_check(&self) -> bool {
            true
        }

        fn severity(&self) -> Severity {
            self.severity
        }
    }

    fn mock_rule(
        name: &'static str,
        description: &'static str,
        severity: Severity,
    ) -> Box<dyn RuleChecker + Send + Sync> {
        Box::new(MockRule {
            name,
            description,
            severity,
        })
    }

    fn mock_diagnostic(
        path: &str,
        rule: &'static str,
        severity: Severity,
        message: &str,
        line_number: usize,
        line_message: &str,
        highlights: Vec<(usize, usize)>,
    ) -> Diagnostic {
        Diagnostic {
            path: PathBuf::from(path),
            rule,
            severity,
            message: message.to_string(),
            lines: vec![DiagnosticLine {
                line_number,
                message: line_message.to_string(),
                highlights,
            }],
            misspelled_words: HashSet::new(),
        }
    }

    #[test]
    fn test_sarif_level() {
        assert_eq!(sarif_level(Severity::Info), "note");
        assert_eq!(sarif_level(Severity::Warning), "warning");
        assert_eq!(sarif_level(Severity::Error), "error");
    }

    #[test]
    fn test_compute_fingerprint_deterministic() {
        let fp1 = compute_fingerprint("blank", "test.po", 10, "blank translation");
        let fp2 = compute_fingerprint("blank", "test.po", 10, "blank translation");
        assert_eq!(fp1, fp2);
        assert!(fp1.ends_with(":1"));
        // 16 hex chars + ":" + "1" = 18 chars.
        assert_eq!(fp1.len(), 18);
    }

    #[test]
    fn test_compute_fingerprint_differs() {
        let fp1 = compute_fingerprint("blank", "test.po", 10, "blank translation");
        let fp2 = compute_fingerprint("blank", "test.po", 11, "blank translation");
        let fp3 = compute_fingerprint("tabs", "test.po", 10, "blank translation");
        let fp4 = compute_fingerprint("blank", "other.po", 10, "blank translation");
        assert_ne!(fp1, fp2);
        assert_ne!(fp1, fp3);
        assert_ne!(fp1, fp4);
    }

    #[test]
    fn test_build_sarif_empty() {
        let result: Vec<CheckFileResult> = vec![];
        let sarif = build_sarif(&result);
        assert_eq!(sarif.schema, SARIF_SCHEMA);
        assert_eq!(sarif.version, SARIF_VERSION);
        assert_eq!(sarif.runs.len(), 1);
        assert_eq!(sarif.runs[0].tool.driver.name, "poexam");
        assert_eq!(sarif.runs[0].column_kind, "unicodeCodePoints");
        assert!(sarif.runs[0].tool.driver.rules.is_empty());
        assert!(sarif.runs[0].results.is_empty());
    }

    #[test]
    fn test_build_sarif_no_diagnostics() {
        let result = vec![CheckFileResult {
            path: PathBuf::from("test.po"),
            config: Config::default(),
            rules: Rules::new(vec![mock_rule(
                "blank",
                "Checks blank translations.",
                Severity::Warning,
            )]),
            diagnostics: vec![],
        }];
        let sarif = build_sarif(&result);
        assert_eq!(sarif.runs[0].tool.driver.rules.len(), 1);
        assert_eq!(sarif.runs[0].tool.driver.rules[0].id, "blank");
        assert_eq!(
            sarif.runs[0].tool.driver.rules[0]
                .default_configuration
                .level,
            "warning"
        );
        assert!(sarif.runs[0].results.is_empty());
    }

    #[test]
    fn test_build_sarif_with_diagnostics() {
        let result = vec![CheckFileResult {
            path: PathBuf::from("fr.po"),
            config: Config::default(),
            rules: Rules::new(vec![
                mock_rule("blank", "Checks blank translations.", Severity::Warning),
                mock_rule("escapes", "Checks escape characters.", Severity::Error),
            ]),
            diagnostics: vec![
                mock_diagnostic(
                    "fr.po",
                    "blank",
                    Severity::Warning,
                    "blank translation",
                    25,
                    "msgstr \" \"",
                    vec![(8, 9)],
                ),
                mock_diagnostic(
                    "fr.po",
                    "escapes",
                    Severity::Error,
                    "missing escape \\n",
                    42,
                    "msgstr \"test\"",
                    vec![],
                ),
            ],
        }];
        let sarif = build_sarif(&result);
        let run = &sarif.runs[0];

        // Rules are sorted by name (BTreeMap).
        assert_eq!(run.tool.driver.rules.len(), 2);
        assert_eq!(run.tool.driver.rules[0].id, "blank");
        assert_eq!(run.tool.driver.rules[1].id, "escapes");

        // Results.
        assert_eq!(run.results.len(), 2);

        let r0 = &run.results[0];
        assert_eq!(r0.rule_id, "blank");
        assert_eq!(r0.rule_index, 0);
        assert_eq!(r0.level, "warning");
        assert_eq!(r0.message.text, "blank translation");
        assert_eq!(r0.locations.len(), 1);
        assert_eq!(
            r0.locations[0].physical_location.artifact_location.uri,
            "fr.po"
        );
        assert_eq!(r0.locations[0].physical_location.region.start_line, 25);
        // Highlight (8, 9) → char columns (9, 10) (1-based).
        assert_eq!(
            r0.locations[0].physical_location.region.start_column,
            Some(9)
        );
        assert_eq!(
            r0.locations[0].physical_location.region.end_column,
            Some(10)
        );

        let r1 = &run.results[1];
        assert_eq!(r1.rule_id, "escapes");
        assert_eq!(r1.rule_index, 1);
        assert_eq!(r1.level, "error");
        assert_eq!(r1.locations[0].physical_location.region.start_line, 42);
        // No highlights → no columns.
        assert!(
            r1.locations[0]
                .physical_location
                .region
                .start_column
                .is_none()
        );
        assert!(
            r1.locations[0]
                .physical_location
                .region
                .end_column
                .is_none()
        );
    }

    #[test]
    fn test_build_sarif_line_fallback() {
        // Diagnostic with no lines at all → defaults to line 1.
        let result = vec![CheckFileResult {
            path: PathBuf::from("test.po"),
            config: Config::default(),
            rules: Rules::new(vec![mock_rule(
                "encoding",
                "Checks encoding.",
                Severity::Info,
            )]),
            diagnostics: vec![Diagnostic {
                path: PathBuf::from("test.po"),
                rule: "encoding",
                severity: Severity::Info,
                message: "invalid encoding".to_string(),
                lines: vec![],
                misspelled_words: HashSet::new(),
            }],
        }];
        let sarif = build_sarif(&result);
        assert_eq!(
            sarif.runs[0].results[0].locations[0]
                .physical_location
                .region
                .start_line,
            1
        );
    }

    #[test]
    fn test_build_sarif_zero_line_fallback() {
        // Diagnostic with line_number=0 → falls back to line 1.
        let result = vec![CheckFileResult {
            path: PathBuf::from("test.po"),
            config: Config::default(),
            rules: Rules::new(vec![mock_rule(
                "compilation",
                "Checks compilation.",
                Severity::Error,
            )]),
            diagnostics: vec![mock_diagnostic(
                "test.po",
                "compilation",
                Severity::Error,
                "msgfmt error",
                0,
                "some output",
                vec![],
            )],
        }];
        let sarif = build_sarif(&result);
        assert_eq!(
            sarif.runs[0].results[0].locations[0]
                .physical_location
                .region
                .start_line,
            1
        );
    }

    #[test]
    fn test_build_sarif_rules_deduplicated() {
        // Two files with the same rules → rules appear only once.
        let result = vec![
            CheckFileResult {
                path: PathBuf::from("a.po"),
                config: Config::default(),
                rules: Rules::new(vec![mock_rule("blank", "Checks blank.", Severity::Warning)]),
                diagnostics: vec![],
            },
            CheckFileResult {
                path: PathBuf::from("b.po"),
                config: Config::default(),
                rules: Rules::new(vec![mock_rule("blank", "Checks blank.", Severity::Warning)]),
                diagnostics: vec![],
            },
        ];
        let sarif = build_sarif(&result);
        assert_eq!(sarif.runs[0].tool.driver.rules.len(), 1);
    }

    #[test]
    fn test_build_sarif_multiple_files() {
        let result = vec![
            CheckFileResult {
                path: PathBuf::from("fr.po"),
                config: Config::default(),
                rules: Rules::new(vec![mock_rule("blank", "Checks blank.", Severity::Warning)]),
                diagnostics: vec![mock_diagnostic(
                    "fr.po",
                    "blank",
                    Severity::Warning,
                    "blank translation",
                    10,
                    "msgstr \"\"",
                    vec![],
                )],
            },
            CheckFileResult {
                path: PathBuf::from("de.po"),
                config: Config::default(),
                rules: Rules::new(vec![mock_rule("blank", "Checks blank.", Severity::Warning)]),
                diagnostics: vec![mock_diagnostic(
                    "de.po",
                    "blank",
                    Severity::Warning,
                    "blank translation",
                    20,
                    "msgstr \"\"",
                    vec![],
                )],
            },
        ];
        let sarif = build_sarif(&result);
        assert_eq!(sarif.runs[0].results.len(), 2);
        assert_eq!(
            sarif.runs[0].results[0].locations[0]
                .physical_location
                .artifact_location
                .uri,
            "fr.po"
        );
        assert_eq!(
            sarif.runs[0].results[1].locations[0]
                .physical_location
                .artifact_location
                .uri,
            "de.po"
        );
    }

    #[test]
    fn test_build_sarif_serializes_to_valid_json() {
        let result = vec![CheckFileResult {
            path: PathBuf::from("test.po"),
            config: Config::default(),
            rules: Rules::new(vec![mock_rule("blank", "Checks blank.", Severity::Warning)]),
            diagnostics: vec![mock_diagnostic(
                "test.po",
                "blank",
                Severity::Warning,
                "blank translation",
                5,
                "msgstr \" \"",
                vec![(8, 9)],
            )],
        }];
        let sarif = build_sarif(&result);
        let json_str = serde_json::to_string(&sarif).expect("SARIF should serialize to JSON");
        let parsed: serde_json::Value =
            serde_json::from_str(&json_str).expect("SARIF JSON should be valid");
        assert_eq!(parsed["$schema"], SARIF_SCHEMA);
        assert_eq!(parsed["version"], SARIF_VERSION);
        assert!(parsed["runs"][0]["results"][0]["ruleId"].is_string());
        assert!(parsed["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["region"]["startLine"].is_number());
    }
}
