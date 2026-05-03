// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Directory utilities.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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
                                .unwrap_or_else(|_| dirent.path())
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn tmp_dir(label: &str) -> tempfile::TempDir {
        tempfile::TempDir::with_prefix(format!("poexam-dir-{label}-")).expect("create temp dir")
    }

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(path, "").expect("write file");
    }

    #[test]
    fn test_empty_dir_returns_empty_set() {
        let tmp = tmp_dir("empty");
        let found = find_po_files(&[tmp.path().to_path_buf()]);
        assert!(found.is_empty());
    }

    #[test]
    fn test_finds_single_po_file() {
        let tmp = tmp_dir("single");
        let po = tmp.path().join("fr.po");
        touch(&po);
        let found = find_po_files(&[tmp.path().to_path_buf()]);
        assert_eq!(found.len(), 1);
        assert!(found.contains(&po));
    }

    #[test]
    fn test_only_po_extension_returned() {
        let tmp = tmp_dir("ext-filter");
        let po = tmp.path().join("a.po");
        touch(&po);
        touch(&tmp.path().join("a.pot"));
        touch(&tmp.path().join("a.txt"));
        touch(&tmp.path().join("notes.md"));
        let found = find_po_files(&[tmp.path().to_path_buf()]);
        assert_eq!(found, std::iter::once(po).collect::<HashSet<_>>());
    }

    #[test]
    fn test_recursive_search() {
        let tmp = tmp_dir("recursive");
        let a = tmp.path().join("a.po");
        let nested = tmp.path().join("sub/deep/nested.po");
        touch(&a);
        touch(&nested);
        let found = find_po_files(&[tmp.path().to_path_buf()]);
        assert!(found.contains(&a));
        assert!(found.contains(&nested));
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn test_multiple_root_paths_are_combined() {
        let tmp_a = tmp_dir("multi-a");
        let tmp_b = tmp_dir("multi-b");
        let a = tmp_a.path().join("a.po");
        let b = tmp_b.path().join("b.po");
        touch(&a);
        touch(&b);
        let found = find_po_files(&[tmp_a.path().to_path_buf(), tmp_b.path().to_path_buf()]);
        assert!(found.contains(&a));
        assert!(found.contains(&b));
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn test_gitignore_skips_listed_files() {
        let tmp = tmp_dir("gitignore");
        // The `ignore` crate only honors `.gitignore` inside a git repo by default,
        // so mark the temp dir as one (a `.git` directory is enough).
        std::fs::create_dir_all(tmp.path().join(".git")).expect("create .git marker");
        // Excluded subtree.
        let ignored = tmp.path().join("ignored/skip.po");
        touch(&ignored);
        // Visible file.
        let visible = tmp.path().join("keep.po");
        touch(&visible);
        // .gitignore in the walk root excludes the subtree.
        std::fs::write(tmp.path().join(".gitignore"), "ignored/\n").expect("write .gitignore");

        let found = find_po_files(&[tmp.path().to_path_buf()]);
        assert!(found.contains(&visible));
        assert!(!found.contains(&ignored));
    }
}
