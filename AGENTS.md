<!--
SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>

SPDX-License-Identifier: GPL-3.0-or-later
-->

# AGENTS.md

Instructions for AI coding agents working on this project.

## Project overview

Poexam is a blazingly fast PO file linter written in Rust. It checks gettext PO files for common errors and reports diagnostics with highlighted context. It also reports statistics on PO files (translation progress, and counts of entries, words and characters per file and aggregated).

## Build and test

```shell
cargo build            # Debug build
cargo build --release  # Release build
cargo test --verbose   # Run all tests
cargo clippy -- -D clippy::pedantic  # Lint (must pass with zero warnings)
cargo fmt --check      # Formatting (must report no diffs)
cargo fmt              # Auto-format all Rust source files
```

All four commands must pass before submitting changes — they mirror `.github/workflows/ci.yml` exactly.

**Formatting is mandatory**: every Rust file must be formatted with stock `cargo fmt` (no `rustfmt.toml` overrides). Always run `cargo fmt` after editing — `cargo fmt --check` must report no diffs, otherwise CI fails.

The project is REUSE-compliant. Run `pre-commit run --all-files` (or rely on the pre-commit hook) to verify SPDX/license headers — `.github/workflows/reuse.yml` runs the same check on every push.

## Project structure

- `src/main.rs` — Entry point, dispatches to subcommands (`check`, `rules`, `stats`, `lsp`).
- `src/args.rs` — CLI argument parsing with `clap` derive.
- `src/checker.rs` — Core checking logic, runs rules against PO entries.
- `src/config.rs` — TOML configuration file handling.
- `src/diagnostic.rs` — Diagnostic types (`Severity`: `Info`, `Warning`, `Error`).
- `src/dict.rs` — Hunspell dictionary and spell checking support.
- `src/dir.rs` — Directory traversal (respects `.gitignore`).
- `src/result.rs` — Display check results (human/JSON/SARIF/misspelled) and compute exit code.
- `src/sarif.rs` — SARIF v2.1.0 output format.
- `src/lsp.rs` — Language server (LSP) over stdin/stdout for editor integration (`poexam lsp`).
- `src/stats.rs` — Statistics command implementation.
- `src/po/` — PO file parser (entry, escape, message, format strings).
- `src/rules/` — All lint rules, one file per rule, or per closely related rule group.
- `src/rules/rule.rs` — `RuleChecker` trait and rule loading.
- `src/rules/mod.rs` — Module declarations for all rules.
- `editors/zed/` — Zed editor extension (standalone WASM crate, excluded from the package): launches `poexam lsp` and registers the `PO` language + tree-sitter grammar. Not part of the main `cargo` build.

## Adding a new rule

1. Create a new file in `src/rules/` (e.g. `src/rules/my_rule.rs`).
2. Implement the `RuleChecker` trait from `src/rules/rule.rs`. Required methods:
   - `name()` — kebab-case rule name (e.g. `"my-rule"`).
   - `description()` — short description of what the rule checks.
   - `is_default()` — whether the rule is enabled by default.
   - `is_check()` — `true` for real checks, `false` for special rules like `fuzzy`/`noqa`.
   - `severity()` — pick by impact:
     - `Severity::Error` — file won't compile or msgid/msgstr structural mismatch that breaks runtime (e.g. `compilation`, `escapes`, `formats`, `newlines`, `plurals`, `tabs`).
     - `Severity::Warning` — translation is likely wrong but file still compiles (e.g. `blank`, `long`, `short`).
     - `Severity::Info` — stylistic or informational (default for most rules).
   - One or more check methods: `check_file()`, `check_header()`, `check_entry()`, `check_ctxt()`, or `check_msg()`.
3. Add `pub mod my_rule;` in `src/rules/mod.rs`.
4. Register the rule in `src/rules/rule.rs`, `get_all_rules()`.
5. Add tests in the same file using `#[cfg(test)]` module.
6. Update `README.md` rules table and `CHANGELOG.md`.

## Adding a new format language

The `formats` rule checks format strings per-language (C, Java, Python…). Each language has its own parser in `src/po/format/`. The PO keyword (e.g. `c-format`, `java-format`) is parsed in `src/po/parser.rs` by stripping the `-format` suffix and passing the remainder to `Language::from()`.

To add support for a new format language (e.g. `ruby-format`):

1. Create `src/po/format/lang_ruby.rs` implementing the `FormatParser` trait (methods `next_char()` and `find_end_format()`). Use `lang_java.rs` or `lang_c.rs` as a reference.
2. Add `pub mod lang_ruby;` in `src/po/format/mod.rs`.
3. Update `src/po/format/language.rs`:
   - Add a variant to the `Language` enum (e.g. `Ruby`).
   - Add a match arm in `From<&str>` for `"ruby"` → `Self::Ruby` (this maps from the PO keyword `ruby-format`).
   - Add a match arm in `Display` (e.g. `Self::Ruby => write!(f, "Ruby")`).
   - Add a match arm in `format_parser()` returning the new parser.
   - Add a test case in `test_language()`.
4. Add tests in `lang_ruby.rs` using a `#[cfg(test)]` module (test `strip_formats`, `FormatPos`, etc.).
5. Update the `formats` rule documentation in `README.md`.
6. Update `CHANGELOG.md`.

The format is automatically picked up by the parser — no changes needed in `src/po/parser.rs` (it already strips `-format` and calls `Language::from()`).

## Code conventions

- **SPDX headers**: Every file must start with the SPDX copyright and license header (update year, author and comment type accordingly on each file change). This is enforced by the REUSE tool via the pre-commit hook and the `reuse.yml` workflow — missing or malformed headers fail CI:

  ```rust
  // SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
  //
  // SPDX-License-Identifier: GPL-3.0-or-later
  ```

- **Clippy pedantic**: Code must pass `cargo clippy -- -D clippy::pedantic` with no warnings.
- **Rust edition**: 2024 (minimum Rust version: 1.87).
- **Tests**: Use inline `#[cfg(test)]` modules within each source file (no separate test files).
- **Rule naming**: Use kebab-case for rule names (e.g. `double-quotes`, `punc-start`).
- **Module naming**: Use snake_case for file/module names (e.g. `double_quotes.rs`, `punc_space.rs`). A single file may host multiple related rule structs (e.g. `punc.rs` defines both `PuncStartRule` and `PuncEndRule`; `spelling.rs` defines `SpellingCtxtRule`/`SpellingIdRule`/`SpellingStrRule`) — group rules by topic rather than mechanically one-per-file.

## Coding guidelines

Performance matters: poexam is meant to lint large PO trees in milliseconds. Hot paths are the parser (`src/po/parser.rs`) and per-entry/per-message rule callbacks (`check_entry`, `check_msg`, `check_ctxt`) — they run once per entry across every PO file under `rayon` parallelism.

- Avoid allocations in hot paths: prefer `&str` over `String`, and `Cow<str>` when a transformation may be a no-op.
- Use `memchr` (already a dependency) for byte-level scanning instead of `str::find` / `chars().position()` over large strings.
- Don't compile regexes inside per-entry callbacks; build them once (e.g. via `OnceLock` / `LazyLock`) and reuse.
- Verify perf-sensitive changes with a release-mode timing run on the PO files under `examples/` (e.g. `cargo run --release -- check examples/`) and compare against `main`.

## Dependencies

Avoid adding new dependencies unless strictly necessary. Current key dependencies:
`clap`, `colored`, `rayon`, `spellbook`, `encoding_rs`, `ignore`, `serde`, `serde_json`, `toml`, `memchr`, `path-absolutize`, `tower-lsp` and `tokio` (used only by the `lsp` command).

## Changelog

Update `CHANGELOG.md` for every user-facing change, following the existing format with `Removed`, `Changed`, `Added`, `Fixed` sections (in this order) under the current development version.
