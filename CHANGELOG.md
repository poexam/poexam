<!--
SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>

SPDX-License-Identifier: GPL-3.0-or-later
-->

# Poexam ChangeLog

## Version 0.0.6 (2026-02-09)

### Changed

- Rename parameter `--file-status` to `--file-stats`

### Added

- Add option `--rule-stats` to display rule statistics
- Add pre-commit hook
- Add special rule "default" to allow add of extra rules
- Add output `misspelled` in `check` command to display only all misspelled words
- Add option `--path-words` to specify a path to a directory containing files with list of words to add per language

### Fixed

- Fix selection of rules when special rules are used
- Use severity "Warning" when a dictionary is not found for a language

## Version 0.0.5 (2026-02-08)

### Fixed

- Fix highlight of misspelled words

## Version 0.0.4 (2026-02-08)

### Fixed

- Add special rule "spelling" in output of `poexam rules`

## Version 0.0.3 (2026-02-08)

### Changed

- Use multiple threads to search PO files in directories

### Added

- Add rules "spelling-ctxt", "spelling-id" and "spelling-str"
- Add special rule "spelling" to select all spelling rules
- Add special rule "checks" to select all rules except fuzzy, obsolete and untranslated

### Fixed

- Fix detection of C formats in strings, fix panic in case of invalid format
- Skip C format strings in count of words and characters
- Fix message when no files are checked
- Sort file status by path (option `--file-status`)

## Version 0.0.2 (2026-02-04)

### Changed

- Change line number color to cyan in diagnostic output

### Fixed

- Remove color from JSON output, add field "highlights" (list of (start, end) positions in string)
- Add support of full-width, Arabian, Greek and Persian punctuation
- Sort errors by filename to have a predictable order

## Version 0.0.1 (2026-02-02)

### Added

- Initial release 🎉
- Check PO files with 19 built-in rules
- Display statistics: progress, count of messages, words, characters
