// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashSet;
use std::path::PathBuf;

use colored::Colorize;
use ignore::WalkBuilder;

/// Recursively find all gettext files (matching the `*.po` pattern) under the given paths.
///
/// The .gitignore rules are respected: ignored files are skipped.
pub fn find_po_files(paths: &[PathBuf]) -> Vec<PathBuf> {
    let all_paths: Vec<PathBuf> = if paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        paths.to_vec()
    }
    .into_iter()
    .collect();

    let mut builder = WalkBuilder::new(all_paths[0].clone());
    for root in all_paths.iter().skip(1) {
        builder.add(root);
    }

    builder.follow_links(false);

    let mut out = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    for entry in builder.build() {
        match entry {
            Ok(dirent) => {
                let path = dirent.path();
                // Keep only regular files with extension `.po`.
                if dirent.file_type().is_some_and(|ft| ft.is_file())
                    && path.extension().is_some_and(|e| e == "po")
                {
                    let path2 = path.strip_prefix("./").unwrap_or(path).to_path_buf();
                    if seen.insert(path2.clone()) {
                        out.push(path2);
                    }
                }
            }
            Err(err) => {
                eprintln!("{}: could not read entry: {err}", "Warning".yellow());
            }
        }
    }
    out
}
