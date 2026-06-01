<!--
SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>

SPDX-License-Identifier: GPL-3.0-or-later
-->

# poexam Zed extension

A [Zed](https://zed.dev) extension that adds support for PO (gettext) files:

- **Real-time diagnostics** (info / warning / error) from the poexam language server.
- **Syntax highlighting** via the [tree-sitter-po](https://github.com/tree-sitter-grammars/tree-sitter-po) grammar.

The extension itself is a thin WASM shim: it registers the `PO` language and launches
`poexam lsp` as its language server. All checking is done by the `poexam` binary.

## Requirements

The `poexam` binary must be available on your `$PATH`:

```shell
cargo install poexam
```

(or `cargo install --path .` from the repository root for a development build).

## Install as a dev extension

1. Build prerequisite (once): `rustup target add wasm32-wasip2`.
2. In Zed, open the command palette and run **zed: install dev extension**.
3. Select this directory (`editors/zed`).

Zed compiles the extension and the grammar, then associates `*.po` and `*.pot` files with the
`PO` language. Open a PO file: it is syntax-highlighted, and poexam diagnostics appear inline as
you type. Use **zed: open language server logs** to inspect the `poexam` server if needed.

## Configuration

The language server discovers the closest `poexam.toml` by walking up from the file path, exactly
like the `poexam check` command. See the [main README](../../README.md) for configuration details.
