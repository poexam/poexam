// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Zed extension for poexam.
//!
//! Registers the `PO` language and starts the poexam language server
//! (`poexam lsp`) for it, giving real-time PO (gettext) diagnostics in Zed.
//! The `poexam` binary is expected on the user's `$PATH`.

use zed_extension_api::{self as zed, Command, LanguageServerId, Result, Worktree};

/// Name of the poexam binary, looked up on the user's `$PATH`.
const POEXAM_BINARY: &str = "poexam";

struct PoexamExtension;

impl zed::Extension for PoexamExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Command> {
        let command = worktree.which(POEXAM_BINARY).ok_or_else(|| {
            format!(
                "{POEXAM_BINARY} was not found on $PATH; \
                 install it (for example with `cargo install poexam`)"
            )
        })?;
        Ok(Command {
            command,
            args: vec!["lsp".to_string()],
            env: worktree.shell_env(),
        })
    }
}

zed::register_extension!(PoexamExtension);
