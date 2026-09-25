<!--
SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>

SPDX-License-Identifier: GPL-3.0-or-later
-->

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/2.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Add non-default rule "markdown" to check for missing, extra or different Markdown syntax in the translation (code spans, link destinations and emphasis markers).
- Add default rule "compendium" to check for unresolved `msgcat` conflict markers (`#-#-#-#-#`) left in the translation or the header.
- Add default rule "lang-path" to check that the language in the PO file header matches the language announced by the file path.
- Report the unchanged POT template values of the PO file header (`PACKAGE VERSION`, `YEAR-MO-DA HO:MI+ZONE`, `FULL NAME <EMAIL@ADDRESS>`, `LANGUAGE <LL@li.org>`, `charset=CHARSET`, `ENCODING`) with rule "header".
- Add non-default rule "options" to check for missing, extra or different command-line options in the translation.
- Add option `--variable-styles` and config key `variable_styles` to select the variable syntaxes checked by the rule "variables".
- Add default rule "variables" to check for missing, extra or different variables in the translation (placeholders of templating syntaxes that carry no `*-format` flag, e.g. `${VAR}`, `%VAR%`, `@VAR@` or `{{var}}`).
- Add non-default rule "numbers" to check for missing, extra or different numbers in the translation.
- Add default rule "plurals-empty" to check for an empty plural form in an entry where another plural form is translated.

### Changed

- Load each spelling dictionary only once per run instead of once per file, and check each distinct word only once per file: the spelling rules are up to 4 times faster and use less memory.

## [0.1.1] - 2026-09-01

### Added

- Publish static Linux binaries for `x86_64` and `aarch64`, built against musl libc, on each release.

## [0.1.0] - 2026-08-09

### Added

- Add option `--exclude` and config key `exclude` to exclude files matching glob patterns.
- Add option `--langs` and config key `langs` to check only files in these languages (other files are ignored).
- Add auto-fix for rule "double-spaces" when the source has no double spaces.
- Add auto-fix for rule "html-tags" when the translation has different tags at the same positions.
- Add option `--unsafe-fixes` and config key `unsafe_fixes` to also apply unsafe auto-fixes with `--fix`.

### Changed

- **Breaking:** rename option `--lang-id` to `--spelling-lang-id` and config key `lang_id` to `spelling_lang_id`.
- **Breaking:** rename option `--langs` to `--spelling-langs` and config key `langs` to `spelling_langs`.
- Apply only safe auto-fixes with `--fix` by default; unsafe fixes now require `--unsafe-fixes`.

### Fixed

- Do not report inconsistent leading whitespace in rules "whitespace-start" and "whitespace-line-start" when a French translation starts with the space required before `:`, `;`, `!`, `?` or `»`.

## [0.0.12] - 2026-06-28

### Added

- Add command "lsp" to run a language server (LSP) over stdin/stdout, reporting diagnostics in real time for editor integration.
- Add a Zed editor extension providing PO syntax highlighting and real-time diagnostics via `poexam lsp`.
- Add default rule "whitespace-line-start" to check for inconsistent leading whitespace at the start of each line.
- Add default rule "whitespace-line-end" to check for inconsistent trailing whitespace at the end of each line.

### Fixed

- Do not treat surrounding quotes as part of an email in rule "emails".

## [0.0.11] - 2026-06-07

### Added

- Add option `--fix` to rewrite files in place, applying auto-fixable diagnostics.
- Add option `--width` to set the page width used by `--fix` when wrapping msgstr blocks (default: 79, 0 disables wrapping).
- Add default rule "accelerators" to check that keyboard accelerators are preserved in the translation, with option `--accelerator` and config key `accelerator` to set the marker character (default: `&`).
- Add non-default rule "acronyms" to check that source acronyms are preserved in the translation.
- Add non-default rule "force-trans" with option `--force-trans-file` and config key `force_trans_file`.
- Add non-default rule "no-trans" with option `--no-trans-file` and config key `no_trans_file`.
- Add non-default rule "functions" to check for missing, extra or different function names.

### Changed

- Treat surrounding whitespace as part of the leading/trailing punctuation run in rules "punc-start" and "punc-end".
- Ignore whitespace when comparing leading/trailing punctuation in rules "punc-start" and "punc-end".
- Recognize script-specific punctuation in rules "punc-start" and "punc-end": Japanese/Chinese ideographic comma `、`, Devanagari danda and double danda, Khmer "khan", Myanmar section, Armenian full stop.

## [0.0.10] - 2026-05-13

### Added

- Add support for format string "java-format".
- Add default rule "header".
- Add default rule "unicode-ctrl".
- Add non-default rule "html-tags".
- Add SARIF v2.1.0 output with `--output sarif`.
- Add description of rules in output of `poexam rules`.
- Add options `short_factor` and `long_factor` to configure the length-ratio factor in rules "short" and "long" (default: 8, min: 2).

### Changed

- Display `poexam rules` output as tables.
- Report severity per diagnostic instead of per rule: remove Severity column from `poexam rules` output, filter diagnostics with `--severity`.
- Use severity "error" instead of "info" in rule "encoding".
- Use severity "warning" instead of "info" in rule "emails".
- Use severity "warning" instead of "info" in rule "paths".
- Use severity "warning" instead of "info" in rule "urls".
- Remove special-case condition for single-character strings in rules "short" and "long".

### Fixed

- Allow `+` in local part of an email.
- Allow emails and URLs inside angle brackets.
- Use only URLs with at least one dot inside in rule "urls".

## [0.0.9] - 2026-04-15

### Added

- Add default rules "emails", "punc-space-id" and "punc-space-str".
- Add non-default rules "compilation", "double-words", "noqa", "paths" and "urls".
- Add option `--punc-ignore-ellipsis` for rules "punc-start" and "punc-end".
- Add option `--path-msgfmt` for rule "compilation".

### Changed

- Report ellipsis differences by default in rules "punc-start" and "punc-end".
- Do not display empty translation in diagnostics produced by the "untranslated" rule.

### Fixed

- Add double quotes U+201C (left double quotation mark), U+201F (double high-reversed-9 quotation mark) and U+FF02 (fullwidth quotation mark) in rule "double-quotes".

## [0.0.8] - 2026-03-25

### Added

- Add option `--langs` to check spelling only for these languages.

### Fixed

- Display a specific message with `--rule-stats` when no errors are found.

## [0.0.7] - 2026-03-12

### Added

- Add support for configuration file "poexam.toml", add options `--config` and `--no-config`.
- Add support for format strings "python-format" and "python-brace-format".
- Add rules "changed", "long" and "short".

### Changed

- Rename rule "c-formats" to "formats".
- Ignore words containing digits and all-uppercase words of at least two characters in spelling rules.
- Add French "guillemets" in rule "double-quotes".

### Fixed

- Always exit with code 0 with `--output misspelled`.
- Consider apostrophe as part of word if found inside a word.
- Parse "noqa" in simple comments (lines starting with "# noqa").
- Fix error on unknown rules when using `--severity`.
- Remove leading "./" from file paths.

## [0.0.6] - 2026-02-09

### Added

- Add option `--rule-stats` to display rule statistics.
- Add pre-commit hook.
- Add special rule "default" to allow adding extra rules.
- Add output `misspelled` in `check` command to display only all misspelled words.
- Add option `--path-words` to specify a path to a directory containing files with list of words to add per language.

### Changed

- Rename parameter `--file-status` to `--file-stats`.

### Fixed

- Fix selection of rules when special rules are used.
- Use severity "Warning" when a dictionary is not found for a language.

## [0.0.5] - 2026-02-08

### Fixed

- Fix highlight of misspelled words.

## [0.0.4] - 2026-02-08

### Fixed

- Add special rule "spelling" in output of `poexam rules`.

## [0.0.3] - 2026-02-08

### Added

- Add rules "spelling-ctxt", "spelling-id" and "spelling-str".
- Add special rule "spelling" to select all spelling rules.
- Add special rule "checks" to select all rules except fuzzy, obsolete and untranslated.

### Changed

- Use multiple threads to search PO files in directories.

### Fixed

- Fix detection of C formats in strings.
- Fix panic in case of invalid format.
- Skip C format strings in count of words and characters.
- Fix message when no files are checked.
- Sort file status by path (option `--file-status`).

## [0.0.2] - 2026-02-04

### Changed

- Change line number color to cyan in diagnostic output.

### Fixed

- Remove color from JSON output.
- Add field "highlights" (list of (start, end) positions in string).
- Add support for full-width, Arabic, Greek and Persian punctuation.
- Sort errors by filename to have a predictable order.

## [0.0.1] - 2026-02-02

### Added

- Publish initial release. 🎉
- Check PO files with 19 built-in rules.
- Display statistics: progress, count of messages, words, characters.

[Unreleased]: https://github.com/poexam/poexam/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/poexam/poexam/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/poexam/poexam/compare/v0.0.12...v0.1.0
[0.0.12]: https://github.com/poexam/poexam/compare/v0.0.11...v0.0.12
[0.0.11]: https://github.com/poexam/poexam/compare/v0.0.10...v0.0.11
[0.0.10]: https://github.com/poexam/poexam/compare/v0.0.9...v0.0.10
[0.0.9]: https://github.com/poexam/poexam/compare/v0.0.8...v0.0.9
[0.0.8]: https://github.com/poexam/poexam/compare/v0.0.7...v0.0.8
[0.0.7]: https://github.com/poexam/poexam/compare/v0.0.6...v0.0.7
[0.0.6]: https://github.com/poexam/poexam/compare/v0.0.5...v0.0.6
[0.0.5]: https://github.com/poexam/poexam/compare/v0.0.4...v0.0.5
[0.0.4]: https://github.com/poexam/poexam/compare/v0.0.3...v0.0.4
[0.0.3]: https://github.com/poexam/poexam/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/poexam/poexam/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/poexam/poexam/releases/tag/v0.0.1
