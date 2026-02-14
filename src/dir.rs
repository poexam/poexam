// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Directory utilities.

use std::path::PathBuf;
use std::sync::Mutex;
use std::{collections::HashSet, sync::Arc};

use colored::Colorize;
use ignore::WalkBuilder;

/// Recursively find all gettext files (matching the `*.po` pattern) under the given paths.
///
/// The .gitignore rules are respected: ignored files are skipped.
pub fn find_po_files(paths: &[PathBuf]) -> HashSet<PathBuf> {
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

    let files = Arc::new(Mutex::new(HashSet::new()));
    builder.build_parallel().run(|| {
        let files = Arc::clone(&files);
        Box::new(move |entry| {
            match entry {
                Ok(dirent) => {
                    if dirent.file_type().is_some_and(|ft| ft.is_file())
                        && dirent.path().extension().is_some_and(|ext| ext == "po")
                    {
                        let mut files = files.lock().unwrap();
                        files.insert(
                            dirent
                                .path()
                                .strip_prefix("./")
                                .unwrap_or(dirent.path())
                                .to_path_buf(),
                        );
                    }
                }
                Err(err) => {
                    eprintln!("{}: could not read entry: {err}", "Warning".yellow());
                }
            }
            ignore::WalkState::Continue
        })
    });
    files.lock().unwrap().clone()
}
