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
use std::path::{Path, PathBuf};
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
/// the document path); if none is found, defaults are used. When the discovered
/// config file can not be parsed, a single `config-error` diagnostic is returned
/// instead of silently reverting to defaults. When `include_disk_rules` is
/// `false` the [`DISK_CONTENT_RULES`] are skipped.
fn analyze(uri: &Url, text: &str, include_disk_rules: bool) -> Vec<Diagnostic> {
    let path = uri
        .to_file_path()
        .unwrap_or_else(|()| PathBuf::from("untitled.po"));
    let file_lines: Vec<&str> = text.lines().collect();
    let mut config = match load_config(&path) {
        Ok(config) => config,
        Err(message) => return vec![config_error_diagnostic(&message, &file_lines)],
    };
    apply_buffer_rule_policy(&mut config, include_disk_rules);
    check_bytes(text.as_bytes(), &path, config)
        .iter()
        .map(|diag| to_lsp_diagnostic(diag, &file_lines))
        .collect()
}

/// Discover and load the poexam config for the PO file at `path`.
///
/// Returns the loaded config (with config-relative word-list paths resolved), or
/// defaults when no config file is found. When a config file *is* found but fails
/// to parse, returns an `Err` carrying the error message so the caller can show
/// *why* the config was not applied — mirroring the CLI, which reports the same
/// error rather than running with defaults.
fn load_config(path: &Path) -> Result<Config, String> {
    let Some(config_path) = find_config_path(path) else {
        return Ok(Config::default());
    };
    match Config::new(Some(&config_path)) {
        Ok(mut config) => {
            config.resolve_relative_paths();
            Ok(config)
        }
        Err(err) => Err(format!(
            "invalid config file (path: {}): {err}",
            config_path.display()
        )),
    }
}

/// Build a `config-error` LSP diagnostic for a config that failed to parse.
///
/// It is anchored on the whole first line of the buffer rather than an empty
/// range at `(0, 0)`: Emacs Flymake (and other clients) do not render a
/// zero-width range, so an empty range would be silently invisible.
fn config_error_diagnostic(message: &str, file_lines: &[&str]) -> Diagnostic {
    Diagnostic {
        range: whole_line_range(0, file_lines),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String("config-error".to_string())),
        source: Some("poexam".to_string()),
        message: message.to_string(),
        ..Diagnostic::default()
    }
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
/// When the diagnostic carries a non-empty highlight we underline exactly that
/// span ([`highlighted_range`]); the highlighted line's `message` is always the
/// decoded string value (only `with_msg_hl` / `with_msgs_hl` attach highlights),
/// so the offsets can be mapped onto the raw line when the value appears
/// verbatim in it (no escapes, single line).
///
/// Otherwise we underline the whole *last* line carrying a real (non-zero) line
/// number — the `msgstr` in the usual `msgid` → `msgstr` layout — falling back
/// to the top of the file. Columns are in UTF-16 code units (the default LSP
/// position unit).
fn diagnostic_range(diag: &PoDiagnostic, file_lines: &[&str]) -> Range {
    if let Some(range) = highlighted_range(diag, file_lines) {
        return range;
    }
    let line_index = diag
        .lines
        .iter()
        .rev()
        .map(|line| line.line_number)
        .find(|&number| number > 0)
        .map_or(0, |number| number - 1);
    whole_line_range(line_index, file_lines)
}

/// Map the last numbered line that has a non-empty (width > 0) highlight to a
/// precise LSP range, or `None` when there is no such highlight or the decoded
/// value can not be located verbatim in the raw line (escaped or multi-line
/// string).
fn highlighted_range(diag: &PoDiagnostic, file_lines: &[&str]) -> Option<Range> {
    let line = diag
        .lines
        .iter()
        .rev()
        .find(|line| line.line_number > 0 && line.highlights.iter().any(|(s, e)| e > s))?;
    let line_index = line.line_number - 1;
    let file_line = file_lines.get(line_index)?;
    // Span covering every non-empty highlight on the line.
    let span_start = line
        .highlights
        .iter()
        .filter(|(s, e)| e > s)
        .map(|(s, _)| *s)
        .min()?;
    let span_end = line
        .highlights
        .iter()
        .filter(|(s, e)| e > s)
        .map(|(_, e)| *e)
        .max()?;
    // The decoded value sits right after the opening quote of the string; map
    // the offsets onto the raw line only when it appears there verbatim.
    let content_start = file_line.find('"')? + 1;
    if !file_line
        .get(content_start..)?
        .starts_with(line.message.as_str())
    {
        return None;
    }
    let row = u32::try_from(line_index).unwrap_or(u32::MAX);
    let start = utf16_col(file_line, content_start + span_start);
    let end = utf16_col(file_line, content_start + span_end);
    Some(Range::new(
        Position::new(row, start),
        Position::new(row, end),
    ))
}

/// Range covering the whole line at `line_index`.
fn whole_line_range(line_index: usize, file_lines: &[&str]) -> Range {
    let row = u32::try_from(line_index).unwrap_or(u32::MAX);
    let end_column = file_lines
        .get(line_index)
        .map_or(0, |text| line_len_utf16(text));
    Range::new(Position::new(row, 0), Position::new(row, end_column))
}

/// UTF-16 column of byte offset `byte` within `line`.
fn utf16_col(line: &str, byte: usize) -> u32 {
    let prefix = line.get(..byte).unwrap_or(line);
    u32::try_from(prefix.encode_utf16().count()).unwrap_or(u32::MAX)
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
    fn test_diagnostic_range_uses_highlight_span_on_value() {
        // msgid (line 6) + separator + msgstr (line 8); the trailing space of the
        // msgstr value is highlighted, so the range must cover just that char.
        let mut diag = po_diag(&[(6, "hello"), (0, ""), (8, "bonjour ")]);
        diag.lines[2].highlights = vec![(7, 8)];
        let file_lines = [
            "a",
            "b",
            "c",
            "d",
            "e",
            "msgid \"hello\"",
            "",
            "msgstr \"bonjour \"",
        ];
        // Line 8 is index 7; the value starts at byte 8 (after the opening quote),
        // so highlight (7, 8) maps to file columns (15, 16) — the trailing space.
        let range = diagnostic_range(&diag, &file_lines);
        assert_eq!(range.start, Position::new(7, 15));
        assert_eq!(range.end, Position::new(7, 16));
    }

    #[test]
    fn test_diagnostic_range_falls_back_to_whole_line_for_escaped_value() {
        // The decoded value holds a real newline; the raw line escapes it as `\n`,
        // so the value is not verbatim and the whole line is underlined.
        let mut diag = po_diag(&[(1, "a\nb")]);
        diag.lines[0].highlights = vec![(0, 3)];
        let raw = "msgstr \"a\\nb\"";
        let file_lines = [raw];
        let range = diagnostic_range(&diag, &file_lines);
        assert_eq!(range.start, Position::new(0, 0));
        assert_eq!(range.end, Position::new(0, line_len_utf16(raw)));
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

    /// Create a temp directory with a PO file and an optional config file,
    /// returning the directory handle and the PO file path.
    fn tmp_repo(label: &str, config: Option<&str>) -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::TempDir::with_prefix(format!("poexam-lsp-{label}-"))
            .expect("create temp dir");
        if let Some(content) = config {
            std::fs::write(tmp.path().join("poexam.toml"), content).expect("write config");
        }
        let po = tmp.path().join("fr.po");
        std::fs::write(&po, "msgid \"\"\nmsgstr \"\"\n").expect("write po file");
        (tmp, po)
    }

    #[test]
    fn test_load_config_invalid_config_yields_error_message() {
        // A config that fails validation must surface as an error rather than
        // silently reverting to defaults.
        let (_tmp, po) = tmp_repo("bad-cfg", Some("[check]\nshort_factor = 1\n"));
        let message = load_config(&po).expect_err("invalid config is an error");
        assert!(message.contains("invalid config file"));
        assert!(message.contains("check.short_factor"));
    }

    #[test]
    fn test_config_error_diagnostic_anchors_on_non_empty_first_line() {
        // The diagnostic must span the first line, not an empty range at (0, 0):
        // Emacs Flymake does not render a zero-width range, so an empty range
        // would be invisible.
        let file_lines = ["msgid \"x\"", "msgstr \"y\""];
        let diag = config_error_diagnostic("invalid config file (path: x): boom", &file_lines);
        assert_eq!(
            diag.code,
            Some(NumberOrString::String("config-error".to_string()))
        );
        assert_eq!(diag.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(diag.range.start, Position::new(0, 0));
        assert!(
            diag.range.end.character > 0,
            "config-error range must be non-empty so editors render it, got {:?}",
            diag.range,
        );
    }

    #[test]
    fn test_load_config_no_config_file_yields_defaults() {
        let (_tmp, po) = tmp_repo("no-cfg", None);
        let config = load_config(&po).expect("defaults when no config found");
        assert_eq!(config.check.select, vec!["default".to_string()]);
    }

    #[test]
    fn test_load_config_valid_config_is_applied() {
        let (_tmp, po) = tmp_repo(
            "valid-cfg",
            Some("[check]\nselect = [\"whitespace-end\"]\n"),
        );
        let config = load_config(&po).expect("valid config loads");
        assert_eq!(config.check.select, vec!["whitespace-end".to_string()]);
    }

    #[test]
    fn test_analyze_invalid_config_returns_only_config_error() {
        let (_tmp, po) = tmp_repo("analyze-bad", Some("[check]\nlong_factor = 0\n"));
        let uri = Url::from_file_path(&po).expect("file url");
        let diags = analyze(&uri, "msgid \"x\"\nmsgstr \"y \"\n", true);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("config-error".to_string()))
        );
        assert!(diags[0].range.end.character > 0);
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
