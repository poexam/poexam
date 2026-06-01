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
    async fn upsert_and_publish(&self, uri: Url, text: String) {
        let diagnostics = analyze(&uri, &text);
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
        self.upsert_and_publish(params.text_document.uri, params.text_document.text)
            .await;
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        // With FULL sync the last change carries the entire buffer.
        if let Some(change) = params.content_changes.pop() {
            self.upsert_and_publish(params.text_document.uri, change.text)
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
            let diagnostics = analyze(&uri, &text);
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

/// Check the buffer `text` of `uri` and return LSP diagnostics.
///
/// The PO configuration is discovered from the file on disk (walking up from
/// the document path); if none is found or it is invalid, defaults are used.
fn analyze(uri: &Url, text: &str) -> Vec<Diagnostic> {
    let path = uri
        .to_file_path()
        .unwrap_or_else(|()| PathBuf::from("untitled.po"));
    let config = find_config_path(&path)
        .and_then(|config_path| Config::new(Some(&config_path)).ok())
        .unwrap_or_default();
    let file_lines: Vec<&str> = text.lines().collect();
    check_bytes(text.as_bytes(), &path, config)
        .iter()
        .map(|diag| to_lsp_diagnostic(diag, &file_lines))
        .collect()
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
/// anchor line — the first line carrying a real (non-zero) line number, falling
/// back to the top of the file. The end column is the line's length in UTF-16
/// code units, which is the unit LSP positions use by default.
fn diagnostic_range(diag: &PoDiagnostic, file_lines: &[&str]) -> Range {
    let line_index = diag
        .lines
        .iter()
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
    use std::path::PathBuf;

    use super::*;
    use crate::diagnostic::DiagnosticLine;

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
    fn test_diagnostic_range_anchors_on_first_numbered_line_in_utf16() {
        // A synthetic separator (line 0) precedes the real line 6.
        let diag = po_diag(&[(0, ""), (6, "value")]);
        // Line 6 is index 5; it contains a non-BMP char counted as 2 UTF-16 units:
        // `msgstr "` (8) + 😀 (2) + `"` (1) = 11.
        let file_lines = ["a", "b", "c", "d", "e", "msgstr \"😀\""];
        let range = diagnostic_range(&diag, &file_lines);
        assert_eq!(range.start, Position::new(5, 0));
        assert_eq!(range.end, Position::new(5, 11));
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
}
