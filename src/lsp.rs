// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Language Server Protocol (LSP) server.
//!
//! Runs poexam over stdin/stdout so editors can show diagnostics in real time
//! while a PO file is being edited. The buffer the editor holds (which may be
//! unsaved) is checked with [`check_bytes`](crate::checker::check_bytes), and
//! every poexam [`Diagnostic`](crate::diagnostic::Diagnostic) is mapped to an
//! LSP diagnostic.
//!
//! Rules that read the PO file from disk rather than the buffer (see
//! [`DISK_CONTENT_RULES`]) are skipped while the buffer has unsaved changes, so
//! they never report on stale content; they run on open and on save.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

use tower_lsp::jsonrpc::Result as RpcResult;
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, InitializeParams, InitializeResult,
    InitializedParams, MessageType, NumberOrString, Position, Range, ServerCapabilities,
    ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind, Url,
};
use tower_lsp::{Client, LanguageServer, LspService, Server};

use crate::args::LspArgs;
use crate::checker::check_bytes;
use crate::config::{Config, find_config_path};
use crate::diagnostic::{Diagnostic as PoDiagnostic, Severity as PoSeverity};

/// LSP backend: keeps the latest text of every open document so it can be
/// re-checked on save and cleared on close.
struct Backend {
    client: Client,
    documents: RwLock<HashMap<Url, String>>,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            documents: RwLock::new(HashMap::new()),
        }
    }

    /// Store `text` for `uri`, check it and publish the resulting diagnostics.
    ///
    /// `include_disk_rules` is forwarded to [`analyze`]: it is `true` only when
    /// the buffer is known to match the file on disk (open/save).
    async fn upsert_and_publish(&self, uri: Url, text: String, include_disk_rules: bool) {
        let diagnostics = analyze(&uri, &text, include_disk_rules);
        if let Ok(mut documents) = self.documents.write() {
            documents.insert(uri.clone(), text);
        }
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _params: InitializeParams) -> RpcResult<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                // FULL sync: the whole buffer arrives on every change, so the
                // text we check always matches what the editor displays.
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "poexam".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "poexam language server initialized")
            .await;
    }

    async fn shutdown(&self) -> RpcResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        // A freshly opened buffer matches the file on disk, so disk-content
        // rules (e.g. compilation) are accurate.
        self.upsert_and_publish(params.text_document.uri, params.text_document.text, true)
            .await;
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        // With FULL sync the last change carries the entire buffer. The buffer
        // may now differ from disk, so disk-content rules are skipped.
        if let Some(change) = params.content_changes.pop() {
            self.upsert_and_publish(params.text_document.uri, change.text, false)
                .await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = self
            .documents
            .read()
            .ok()
            .and_then(|documents| documents.get(&uri).cloned());
        if let Some(text) = text {
            // The buffer now matches the file on disk: run disk-content rules.
            let diagnostics = analyze(&uri, &text, true);
            self.client
                .publish_diagnostics(uri, diagnostics, None)
                .await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Ok(mut documents) = self.documents.write() {
            documents.remove(&uri);
        }
        // Clear the diagnostics for the closed document.
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }
}

/// Rules that inspect the PO file *on disk* rather than the in-memory buffer,
/// so their result is stale while the buffer has unsaved changes. They are
/// skipped on `did_change` and run only when the buffer matches the file on
/// disk (`did_open` / `did_save`).
///
/// Only `compilation` qualifies: it runs `msgfmt` on the file path. Other rules
/// that touch the filesystem (`spelling-*`, `force-trans`, `no-trans`) read
/// external dictionaries/word-lists and check the in-memory buffer, so they
/// stay accurate while editing.
const DISK_CONTENT_RULES: &[&str] = &["compilation"];

/// Check the buffer `text` of `uri` and return LSP diagnostics.
///
/// The PO configuration is discovered from the file on disk (walking up from
/// the document path); if none is found or it is invalid, defaults are used.
/// When `include_disk_rules` is `false` the [`DISK_CONTENT_RULES`] are skipped.
fn analyze(uri: &Url, text: &str, include_disk_rules: bool) -> Vec<Diagnostic> {
    let path = uri
        .to_file_path()
        .unwrap_or_else(|()| PathBuf::from("untitled.po"));
    let mut config = find_config_path(&path)
        .and_then(|config_path| Config::new(Some(&config_path)).ok())
        .unwrap_or_default();
    apply_buffer_rule_policy(&mut config, include_disk_rules);
    let file_lines: Vec<&str> = text.lines().collect();
    check_bytes(text.as_bytes(), &path, config)
        .iter()
        .map(|diag| to_lsp_diagnostic(diag, &file_lines))
        .collect()
}

/// When the buffer may differ from disk (`include_disk_rules == false`), add the
/// [`DISK_CONTENT_RULES`] to the config's ignore list so they are skipped.
fn apply_buffer_rule_policy(config: &mut Config, include_disk_rules: bool) {
    if !include_disk_rules {
        for rule in DISK_CONTENT_RULES {
            config.check.ignore.push((*rule).to_string());
        }
    }
}

/// Map a poexam [`Diagnostic`](PoDiagnostic) to an LSP [`Diagnostic`].
fn to_lsp_diagnostic(diag: &PoDiagnostic, file_lines: &[&str]) -> Diagnostic {
    Diagnostic {
        range: diagnostic_range(diag, file_lines),
        severity: Some(to_lsp_severity(diag.severity)),
        code: Some(NumberOrString::String(diag.rule.to_string())),
        source: Some("poexam".to_string()),
        message: diag.build_message().into_owned(),
        ..Diagnostic::default()
    }
}

/// Compute the LSP range for a diagnostic.
///
/// poexam locates a diagnostic by line number; the per-line `highlights` index
/// into the *decoded* message value rather than the raw PO source line, so they
/// can't be mapped to file columns reliably. We therefore underline the whole
/// anchor line — the *last* line carrying a real (non-zero) line number. In the
/// usual `msgid` → `msgstr` layout a diagnostic lists the source first and the
/// translation last, and rules almost always flag the translation, so the last
/// numbered line is the one to highlight. Falls back to the top of the file.
/// The end column is the line's length in UTF-16 code units, which is the unit
/// LSP positions use by default.
fn diagnostic_range(diag: &PoDiagnostic, file_lines: &[&str]) -> Range {
    let line_index = diag
        .lines
        .iter()
        .rev()
        .map(|line| line.line_number)
        .find(|&number| number > 0)
        .map_or(0, |number| number - 1);
    let line = u32::try_from(line_index).unwrap_or(u32::MAX);
    let end_column = file_lines
        .get(line_index)
        .map_or(0, |text| line_len_utf16(text));
    Range::new(Position::new(line, 0), Position::new(line, end_column))
}

/// Length of a line in UTF-16 code units (the default LSP position unit).
fn line_len_utf16(line: &str) -> u32 {
    u32::try_from(line.encode_utf16().count()).unwrap_or(u32::MAX)
}

/// Map a poexam [`Severity`](PoSeverity) to an LSP [`DiagnosticSeverity`].
fn to_lsp_severity(severity: PoSeverity) -> DiagnosticSeverity {
    match severity {
        PoSeverity::Error => DiagnosticSeverity::ERROR,
        PoSeverity::Warning => DiagnosticSeverity::WARNING,
        PoSeverity::Info => DiagnosticSeverity::INFORMATION,
    }
}

/// Run the language server over stdin/stdout.
pub fn run_lsp(_args: &LspArgs) -> i32 {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("poexam: failed to start the LSP runtime: {err}");
            return 1;
        }
    };
    runtime.block_on(async {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let (service, socket) = LspService::new(Backend::new);
        Server::new(stdin, stdout, socket).serve(service).await;
    });
    0
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::diagnostic::DiagnosticLine;

    /// Build a default config restricted to the given selected rules.
    fn config_select(select: &[&str]) -> Config {
        let mut config = Config::default();
        config.check.select = select.iter().map(|s| (*s).to_string()).collect();
        config
    }

    /// Build a poexam diagnostic from a list of `(line_number, message)` lines.
    fn po_diag(lines: &[(usize, &str)]) -> PoDiagnostic {
        PoDiagnostic {
            path: PathBuf::from("fr.po"),
            rule: "blank",
            severity: PoSeverity::Warning,
            message: "blank translation".into(),
            lines: lines
                .iter()
                .map(|(number, message)| DiagnosticLine {
                    line_number: *number,
                    message: (*message).to_string(),
                    highlights: vec![],
                })
                .collect(),
            misspelled_words: std::collections::HashSet::new(),
            fix: None,
        }
    }

    #[test]
    fn test_to_lsp_severity_maps_all_levels() {
        assert_eq!(
            to_lsp_severity(PoSeverity::Error),
            DiagnosticSeverity::ERROR
        );
        assert_eq!(
            to_lsp_severity(PoSeverity::Warning),
            DiagnosticSeverity::WARNING
        );
        assert_eq!(
            to_lsp_severity(PoSeverity::Info),
            DiagnosticSeverity::INFORMATION
        );
    }

    #[test]
    fn test_diagnostic_range_anchors_on_last_numbered_line_in_utf16() {
        // The msgid → separator → msgstr layout: the diagnostic must anchor on
        // the msgstr (line 8), not the msgid (line 6).
        let diag = po_diag(&[(6, "source"), (0, ""), (8, "translation")]);
        // Line 8 is index 7; it contains a non-BMP char counted as 2 UTF-16 units:
        // `msgstr "` (8) + 😀 (2) + `"` (1) = 11.
        let file_lines = ["a", "b", "c", "d", "e", "msgid \"x\"", "", "msgstr \"😀\""];
        let range = diagnostic_range(&diag, &file_lines);
        assert_eq!(range.start, Position::new(7, 0));
        assert_eq!(range.end, Position::new(7, 11));
    }

    #[test]
    fn test_diagnostic_range_no_numbered_line_defaults_to_first_line() {
        let diag = po_diag(&[(0, "kw")]);
        let file_lines = ["hello"];
        let range = diagnostic_range(&diag, &file_lines);
        assert_eq!(range.start, Position::new(0, 0));
        assert_eq!(range.end, Position::new(0, 5));
    }

    #[test]
    fn test_diagnostic_range_line_beyond_buffer_yields_zero_width() {
        let diag = po_diag(&[(99, "x")]);
        let file_lines = ["only one line"];
        let range = diagnostic_range(&diag, &file_lines);
        assert_eq!(range.start, Position::new(98, 0));
        assert_eq!(range.end, Position::new(98, 0));
    }

    #[test]
    fn test_to_lsp_diagnostic_sets_rule_source_and_message() {
        let diag = po_diag(&[(2, "x")]);
        let file_lines = ["l1", "msgstr \"\""];
        let lsp = to_lsp_diagnostic(&diag, &file_lines);
        assert_eq!(lsp.source.as_deref(), Some("poexam"));
        assert_eq!(lsp.code, Some(NumberOrString::String("blank".to_string())));
        assert_eq!(lsp.severity, Some(DiagnosticSeverity::WARNING));
        assert_eq!(lsp.message, "blank translation");
        assert_eq!(lsp.range.start, Position::new(1, 0));
    }

    #[test]
    fn test_apply_buffer_rule_policy_skips_disk_rules_when_dirty() {
        let mut config = Config::default();
        apply_buffer_rule_policy(&mut config, false);
        assert!(config.check.ignore.iter().any(|rule| rule == "compilation"));
    }

    #[test]
    fn test_apply_buffer_rule_policy_keeps_disk_rules_when_clean() {
        let mut config = Config::default();
        apply_buffer_rule_policy(&mut config, true);
        assert!(config.check.ignore.is_empty());
    }

    #[test]
    fn test_dirty_buffer_skips_compilation_rule() {
        // Compilation selected, but the buffer is dirty: the rule is removed, so
        // msgfmt is never run and no compilation diagnostic is produced.
        let mut config = config_select(&["compilation"]);
        apply_buffer_rule_policy(&mut config, false);
        let diags = check_bytes(
            b"msgid \"\"\nmsgstr \"\"\n",
            Path::new("/no/such/fr.po"),
            config,
        );
        assert!(diags.iter().all(|d| d.rule != "compilation"));
    }

    #[test]
    fn test_clean_buffer_runs_compilation_rule() {
        // Compilation selected and the buffer is clean: the rule runs against the
        // (missing) path and reports a compilation diagnostic.
        let mut config = config_select(&["compilation"]);
        apply_buffer_rule_policy(&mut config, true);
        let diags = check_bytes(
            b"msgid \"\"\nmsgstr \"\"\n",
            Path::new("/no/such/fr.po"),
            config,
        );
        assert!(diags.iter().any(|d| d.rule == "compilation"));
    }
}
