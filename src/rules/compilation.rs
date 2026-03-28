// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of the `compilation` rule: check compilation of PO file with msgfmt command.

use crate::checker::Checker;
use crate::diagnostic::Severity;
use crate::rules::rule::RuleChecker;

pub struct CompilationRule;

impl RuleChecker for CompilationRule {
    fn name(&self) -> &'static str {
        "compilation"
    }

    fn is_default(&self) -> bool {
        false
    }

    fn is_check(&self) -> bool {
        true
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    /// Check for compilation errors using the `msgfmt` command.
    ///
    /// This rule is not enabled by default.
    ///
    /// Diagnostics reported with severity [`error`](Severity::Error):
    /// - `command '/usr/bin/msgfmt' reported errors`
    /// - `failed to run command '/usr/bin/msgfmt'`
    fn check_file(&self, checker: &mut Checker) {
        match std::process::Command::new(&checker.config.check.path_msgfmt)
            .arg("--check-format")
            .arg("-o")
            .arg("/dev/null")
            .arg(checker.path.as_path())
            .output()
        {
            Ok(output) => {
                if !output.status.success() {
                    checker.report_file(
                        self.name(),
                        self.severity(),
                        format!(
                            "command `{}` reported errors",
                            checker.config.check.path_msgfmt.display()
                        ),
                        Some(String::from_utf8_lossy(&output.stderr).to_string()),
                    );
                }
            }
            Err(err) => {
                checker.report_file(
                    self.name(),
                    self.severity(),
                    format!(
                        "failed to run command `{}`",
                        checker.config.check.path_msgfmt.display()
                    ),
                    Some(err.to_string()),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs::File,
        io::Read,
        path::{Path, PathBuf},
    };

    use super::*;
    use crate::{
        config::Config, config::DEFAULT_PATH_MSGFMT, diagnostic::Diagnostic, rules::rule::Rules,
    };

    fn check_compilation(filename: &str, path_msgfmt: &Path) -> Vec<Diagnostic> {
        let mut po_file = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        po_file.push("resources");
        po_file.push("test");
        po_file.push(filename);
        let mut data: Vec<u8> = Vec::new();
        File::open(&po_file)
            .unwrap()
            .read_to_end(&mut data)
            .unwrap();
        let mut config = Config::default();
        config.check.path_msgfmt = path_msgfmt.to_path_buf();
        let mut checker = Checker::new(&data)
            .with_path(po_file.as_path())
            .with_config(config);
        let rules = Rules::new(vec![Box::new(CompilationRule {})]);
        checker.do_all_checks(&rules);
        checker.diagnostics
    }

    #[test]
    fn test_compilation_ok() {
        let diags = check_compilation("fr_compilation_ok.po", Path::new(DEFAULT_PATH_MSGFMT));
        assert!(diags.is_empty());
    }

    #[test]
    fn test_compilation_error() {
        let diags = check_compilation("fr_compilation_error.po", Path::new(DEFAULT_PATH_MSGFMT));
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(
            diag.message,
            format!("command `{DEFAULT_PATH_MSGFMT}` reported errors")
        );
    }

    #[test]
    fn test_compilation_command_not_fount() {
        let path = "this_path_does_not_exist";
        let diags = check_compilation("fr_compilation_error.po", Path::new(path));
        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, format!("failed to run command `{path}`"));
    }
}
