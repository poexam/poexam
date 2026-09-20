<!--
SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>

SPDX-License-Identifier: GPL-3.0-or-later
-->

<p align="center">
  <img src="poexam.png" alt="Poexam logo">
</p>
<p align="center">
  <a href="https://crates.io/crates/poexam">
    <img src="https://img.shields.io/crates/v/poexam.svg" alt="Crates.io">
  </a>
  <a href="https://github.com/poexam/poexam/actions?query=workflow%3A%22CI%22">
    <img src="https://github.com/poexam/poexam/workflows/CI/badge.svg" alt="Build status">
  </a>
  <a href="https://api.reuse.software/info/github.com/poexam/poexam">
    <img src="https://api.reuse.software/badge/github.com/poexam/poexam" alt="REUSE status">
  </a>
</p>

# Poexam

**Poexam** is a blazingly fast PO file linter with a comprehensive diagnostic report and auto-fix feature.

It reports very few false positives and can be used in CI jobs and pre-commit hooks.

## Overview

- ⚡️ **Blazingly fast**: large directories and files are checked in parallel, in a few milliseconds.
- 🔎 **Rules**: a lot of checks performed with very few false positives.
- 🎯 **Clear results**: tricky errors in strings are highlighted with colors.
- 🔧 **Auto-fix**: rewrite files in place to clear auto-fixable diagnostics.
- 📝 **Editor integration**: a built-in language server (`poexam lsp`) reports diagnostics as you type.
- 📊 **Statistics**: detailed statistics including progress, count of messages/words/characters.
- 💻 **Multi-platform**: available wherever the Rust compiler is available.
- 🎁 **Free software**: released under [GPLv3](https://gnu.org/licenses/gpl-3.0.html).

## Installation

With cargo:

```shell
cargo install poexam
```

### Static Linux binaries

Each release ships a static binary for Linux `x86_64` and `aarch64`, built against musl libc: it does not depend on the system libc and runs on any distribution, including Alpine and `scratch` containers.

Download it, check it against its `sha256` sidecar and install it (replace `x86_64` with `aarch64` on ARM):

```shell
url=https://github.com/poexam/poexam/releases/latest/download
curl -L --remote-name-all ${url}/poexam-linux-x86_64.tar.gz ${url}/poexam-linux-x86_64.sha256
sha256sum --check poexam-linux-x86_64.sha256
tar --extract --gzip --file poexam-linux-x86_64.tar.gz
install -D -m 0755 poexam-linux-x86_64/poexam ~/.local/bin/poexam
```

The same binary can be built from a clone, on a Linux host of the target architecture:

```shell
tools/build-musl.sh --package
```

## Pre-commit

Add this to your `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: https://github.com/poexam/poexam
    rev: <git-tag-or-commit-sha>  # Use a specific tag vX.Y.Z or commit SHA
    hooks:
      - id: poexam
```

## Features

Poexam can check entire directories and a lot of PO files in just a few milliseconds.

### Configuration file

Poexam can use a different configuration file for each directory scanned, using the TOML format.

The configuration file used is the closest file found by order, where `<path>` is the path of each PO file, and all ancestors of this directory are used as well:

- `<path>/.poexam/poexam.toml`
- `<path>/poexam.toml`
- `<path>/.poexam.toml`

The following options are available in the `check` section (each option can be overridden by the command line parameter having the same name):

| Option               | Type             | Description                                                       |
|----------------------|------------------|-------------------------------------------------------------------|
| fuzzy                | Boolean          | Check fuzzy entries.                                              |
| noqa                 | Boolean          | Check entries marked as "noqa".                                   |
| obsolete             | Boolean          | Check obsolete entries.                                           |
| exclude              | Array of strings | Exclude files matching these glob patterns (path or file name).   |
| langs                | Array of strings | Check only files in these languages (other files are ignored).    |
| select               | Array of strings | Selected rules.                                                   |
| ignore               | Array of strings | Ignored rules.                                                    |
| path_msgfmt          | String (path)    | Path to `msgfmt` for PO file compilation.                         |
| path_dicts           | String (path)    | Path to the Hunspell dictionaries.                                |
| path_words           | String (path)    | Path with custom words (absolute or relative to the config file). |
| force_trans_file     | String (path)    | Path to a word list for the `force-trans` rule.                   |
| no_trans_file        | String (path)    | Path to a word list for the `no-trans` rule.                      |
| spelling_lang_id     | String           | Language used to check source strings.                            |
| spelling_langs       | Array of strings | Check spelling only for these languages.                          |
| short_factor         | Integer          | Min ratio source/translation length to flag "too short" (min: 2). |
| long_factor          | Integer          | Min ratio translation/source length to flag "too long" (min: 2).  |
| severity             | Array of strings | Show diagnostics with these severities (info/warning/error).      |
| punc_ignore_ellipsis | Boolean          | Ignore ellipsis differences (`...` vs `…`) in punc rules.         |
| accelerator          | String (char)    | Marker for keyboard accelerators (default: `&`).                  |
| variable_styles      | Array of strings | Variable syntaxes checked by the rule `variables`.                |
| width                | Integer          | Output page width for `--fix` (default: 79); 0 disables wrapping. |
| unsafe_fixes         | Boolean          | Also apply unsafe auto-fixes with `--fix` (see auto-fix section). |

See configuration file example: [poexam.toml](examples/poexam.toml).

### Rules

It can perform a lot of checks via the default rules:

| Rule name             | Diagnostic reported                                 |
|-----------------------|-----------------------------------------------------|
| accelerators          | Missing/extra keyboard accelerators.                |
| blank                 | Blank translation (only whitespace).                |
| brackets              | Missing/extra brackets.                             |
| double-quotes         | Missing/extra double quotes.                        |
| double-spaces         | Missing/extra double spaces.                        |
| emails                | Missing/extra/different emails.                     |
| encoding              | Incorrect encoding (charset).                       |
| escapes               | Missing/extra escape characters.                    |
| formats               | Inconsistent format strings.                        |
| header                | Invalid/missing/unfilled fields in PO file header.  |
| long                  | Translation too long.                               |
| newlines              | Missing/extra newlines.                             |
| pipes                 | Missing/extra pipes.                                |
| plurals               | Incorrect number of plurals.                        |
| plurals-empty         | Empty plural form among translated ones.            |
| punc-start            | Inconsistent leading punctuation.                   |
| punc-end              | Inconsistent trailing punctuation.                  |
| punc-space-id         | Incorrect spaces around punctuation (source).       |
| punc-space-str        | Incorrect spaces around punctuation (translation).  |
| short                 | Translation too short.                              |
| tabs                  | Missing/extra tabs.                                 |
| unicode-ctrl          | Stray Unicode control chars in translation.         |
| variables             | Missing/extra/different variables (placeholders).   |
| whitespace-end        | Missing/extra whitespace at the end.                |
| whitespace-line-end   | Missing/extra whitespace at the end of each line.   |
| whitespace-line-start | Missing/extra whitespace at the start of each line. |
| whitespace-start      | Missing/extra whitespace at the start.              |

For the rule `formats`, the following languages are supported:

- C (`c-format`): printf format (e.g. `%s %12lld`)
- Java (`java-format`): Java `MessageFormat` language (e.g. `{0}`, `{1,date,short}`)
- Python (`python-format`): Python % format strings (e.g. `%s %(age)d`)
- Python brace (`python-brace-format`): Python brace format strings (e.g. `{0!r:20} {1}`).

The rule `variables` covers the placeholders of the templating syntaxes gettext knows nothing about: such an entry carries no `*-format` flag, so `formats` returns early and the placeholder is otherwise unchecked. The syntaxes to look for are selected with the `variable_styles` option:

| Style          | Syntax    | Where it appears                                   | Enabled by default |
|----------------|-----------|----------------------------------------------------|--------------------|
| `dollar`       | `$VAR`    | shell, systemd units, config templates             | no                 |
| `dollar-brace` | `${VAR}`  | shell, systemd units, config templates             | yes                |
| `percent`      | `%VAR%`   | Windows-style, Qt, some web frameworks             | yes                |
| `at`           | `@VAR@`   | autotools, Transifex                               | yes                |
| `double-brace` | `{{var}}` | Mustache, Handlebars, Jinja, JavaScript frameworks | yes                |
| `brace`        | `{var}`   | .NET and JavaScript template literals              | no                 |

The two styles off by default are the ones whose delimiters also occur in ordinary prose, so a project using them opts in, for example:

```toml
[check]
variable_styles = ["dollar", "dollar-brace"]
```

A variable name is made of ASCII letters, digits and underscores; `{{…}}` accepts a whole expression, e.g. `{{ user.name }}`. A bare `$VAR` must start with a letter or an underscore (`$5` is an amount of money), and `%VAR%` and `@VAR@` are recognized only at a word boundary (`user@example.com` is an address). Variables inside format strings are skipped, so a `python-brace-format` entry is left to the `formats` rule.

Some extra rules are not used by default because they are not really "checks",
report too many false positives or can slow down the process.

You can enable them on-demand:

| Rule name      | Diagnostic reported                              |
|----------------|--------------------------------------------------|
| acronyms       | Acronyms from the source missing in translation. |
| changed        | Translation is different from the source string. |
| compilation    | Compilation with `msgfmt`.                       |
| double-words   | Translation has consecutive repeated words.      |
| force-trans    | Words that must be translated.                   |
| functions      | Missing/extra/different function names.          |
| fuzzy          | Fuzzy entry.                                     |
| html-tags      | Missing/extra/different HTML tags.               |
| no-trans       | Words that must not be translated.               |
| noqa           | Entry has `noqa` comment.                        |
| numbers        | Missing/extra/different numbers.                 |
| obsolete       | Obsolete entry.                                  |
| options        | Missing/extra/different command-line options.    |
| paths          | Missing/extra/different paths.                   |
| spelling-ctxt  | Spelling error in the context.                   |
| spelling-id    | Spelling error in the source.                    |
| spelling-str   | Spelling error in the translation.               |
| unchanged      | Translation is the same as the source string.    |
| untranslated   | Untranslated entry.                              |
| urls           | Missing/extra/different URLs.                    |

The result is very clear, almost all errors are highlighted in the strings so you can immediately see where the issue is.

You can check by yourself with the following command executed in the root directory of the project (output is truncated here):

```shell
poexam check examples/fr.po
```

Example of diagnostic reported:

```text
examples/fr.po:42: [info:brackets] missing opening and closing square brackets '[' (1 / 0) and ']' (1 / 0)
        |
     43 | Test [brackets]
        |
     44 | Test crochets
        |
```

### PO file header

The default rule `header` checks the fields of the PO file header: that the required ones are present, that their values are well-formed, and that none of them is still the placeholder `xgettext` and `msginit` write into a fresh template.

A header still carrying any of these values was never filled in:

| Field                       | Template value                |
|-----------------------------|-------------------------------|
| `Project-Id-Version`        | `PACKAGE VERSION`             |
| `PO-Revision-Date`          | `YEAR-MO-DA HO:MI+ZONE`       |
| `Last-Translator`           | `FULL NAME <EMAIL@ADDRESS>`   |
| `Language-Team`             | `LANGUAGE <LL@li.org>`        |
| `Content-Type`              | `text/plain; charset=CHARSET` |
| `Content-Transfer-Encoding` | `ENCODING`                    |

These are reported as warnings, except `Content-Type`: `charset=CHARSET` leaves the file with no usable encoding declaration, so it is an error. A field reported this way is not reported again as an `invalid value`, so each one yields a single diagnostic. An empty `Language` needs no entry in the table, since `invalid value '' for field 'Language'` already reports it.

### Spell checking

You can check all words in a file by using one of these rules:

- `spelling-ctxt`: check all words in context strings (`msgctxt`) with English `en_US` dictionary.
- `spelling-id`: check all words in source strings (`msgid`) with English `en_US` dictionary.
- `spelling-str`: check all words in translated strings (`msgstr`) with the language found in PO file header.

The special rule `spelling` can be used to select these 3 rules at once.

For rules `spelling-ctxt` and `spelling-id`, the default dictionary used is `en_US` and can be changed with the option `--spelling-lang-id`.

The dictionaries are read from the hunspell directory (option `--path-dicts` to override it), in the following way:

- Search the dictionary with the language name, e.g. files `en_US.aff` and `en_US.dic`
- Search the dictionary with the language code and no country, e.g. files `en.aff` and `en.dic`.

Personal words can be used, so that they are ignored by the spell checker (always considered good).\
With the option `--path-words` you can specify a directory containing personal words files, one per language.

For example this file `en_US.dic` can be used in such directory to ignore some words in English:

```text
charset
hostname
stdout
uptime
```

The output `misspelled` displays all misspelled words and can be used to build such dictionary.

For example, to build a dictionary for English (the English hunspell dictionary must be installed):

```shell
poexam check --select spelling-id --output misspelled fr.po > en_US.dic
```

And for the translated words in French (the French hunspell dictionary must be installed):

```shell
poexam check --select spelling-str --output misspelled fr.po > fr.dic
```

### Acronyms

The non-default rule `acronyms` checks that every acronym found in the source (`msgid`) is also present verbatim in the translation (`msgstr`). An acronym is an alphanumeric word of length ≥ 2 for which Python's `str.isupper()` returns true: at least one cased character is uppercase and no character is lowercase. So `URL`, `HTTP`, `API`, `JSON`, `MP3` and `B2B` are acronyms; `Url`, `URLs`, `Json` and pure-digit words like `123` are not. Format strings are skipped (e.g. `%s`, `{0}`).

For example, for an English-to-French translation:

```text
msgid "Use the HTTP API"
msgstr "Utiliser l'API HTTP"  # ok, both acronyms are preserved
msgstr "Utiliser l'interface"  # flagged: HTTP and API are missing
```

When the `force-trans` rule is also enabled and a `force-trans-file` is configured, any acronym whose lowercase form is listed in that file is ignored by the `acronyms` rule, because the force-trans rule requires it to be translated (the two rules would otherwise contradict each other).

### Command-line options

The non-default rule `options` checks that every command-line option found in the source is also present in the translation: `--verbose` rendered as `--bavard` is a help string telling the user to type something that does not work.

An option is a `-` or `--` prefix at a word boundary followed by an ASCII letter, then any number of ASCII alphanumeric characters, `-` and `_`. Only the name is compared, so the value of `--opt=value` stays translatable:

```text
msgid "Use --log-level=debug"
msgstr "Utilisez --log-level=débogage"  # ok, only the name is compared
msgstr "Utilisez --niveau=débogage"     # flagged: the option was translated
```

The word boundary and the leading letter are what keep prose out: the hyphen of `well-known`, `café-restaurant`, `UTF-8` and `5-10` follows an alphanumeric character, while `-5` and a dash surrounded by spaces are not followed by a letter. Format strings are skipped (e.g. `%s`, `{0}`).

### Force / forbid translation of specific words

Two non-default rules consult external word lists to enforce, for a given vocabulary, whether words found in the source must (or must not) be translated:

- `force-trans` (option `--force-trans-file` or `force_trans_file`): every word from the list that appears in the source (`msgid`) must NOT appear in the translation (`msgstr`) with the **same case as used in the source**. A translation that reuses the source word with a different case is considered a deliberate variant and is not flagged.
- `no-trans` (option `--no-trans-file` or `no_trans_file`): every word from the list that appears in the source must also appear in the translation, the same number of times — and with the **same case as used in the source**, regardless of the case in which the word is stored in the list file. The position of the word in the translation does not matter.

Both word-list files share the same format: one word per line, blank lines and lines starting with `#` are ignored. Matching of source words against the list is case-insensitive (so `LINUX`, `Linux` and `linux` in the list are equivalent).

For example, with `linux` in a no-trans file:

```text
msgid "Linux is great"
msgstr "Linux est génial"  # ok, source case is preserved
msgstr "linux est génial"  # flagged: source uses "Linux", not "linux"
msgstr "ceci est génial"   # flagged: missing "Linux"
```

And with `forbidden` in a force-trans file:

```text
msgid "this is forbidden"
msgstr "ceci est interdit"   # ok, "forbidden" was translated
msgstr "ceci est forbidden"  # flagged: source case-form "forbidden" reused verbatim
msgstr "ceci est Forbidden"  # ok, different case from the source — counts as a variant
```

### Auto-fix

With the option `--fix`, poexam rewrites each PO file in place, applying every diagnostic that carries a **safe** auto-fix. The file is then re-checked, so the reported diagnostics reflect the post-fix state; any remaining diagnostic is annotated with `Note: no fix available.` (or `Note: unsafe fix available, use --unsafe-fixes to apply it.` when a fix exists but is unsafe).

Each fix is either **safe** or **unsafe** (see the per-rule list below). Safe fixes preserve the translation's meaning (whitespace and punctuation normalization, header defaults, obsolete-entry deletion, …) and are always applied by `--fix`. Unsafe fixes rely on positional heuristics that a reordered translation can defeat (e.g. replacing emails, URLs, paths, numbers, variables, options, function names or HTML tags by position), so `--fix` skips them unless `--unsafe-fixes` is also given:

```shell
poexam check --fix po/                 # safe fixes only
poexam check --fix --unsafe-fixes po/  # safe and unsafe fixes
```

`--unsafe-fixes` (or the `unsafe_fixes` config key) has no effect on its own; it must be combined with `--fix`.

The rewriter wraps each replaced `msgstr` block the same way GNU `msgcat` does (Unicode Line Breaking + display width, default page width 79), so running `msgcat` on a fixed file is a no-op. The page width is configurable with `--width N` (or `check.width` in the config file); `--width 0` disables wrapping entirely (matches `msgcat --width=0` / `msgcat --no-wrap`).

Rules that currently produce auto-fixes (`Safe: no` fixes require `--unsafe-fixes`):

#### double-spaces

- **Fix**: When the source has no double spaces, collapse each interior run of two or more spaces
  in the translation to a single space. Only interior runs are touched; leading, trailing and
  per-line edge runs are left to the whitespace rules. The "extra" case where the source itself
  has double spaces (which run is surplus is ambiguous) and the "missing" case (no determinable
  insertion point) are not auto-fixable.
- **Safe**: yes.

#### double-words

- **Fix**: Remove the second occurrence of any consecutive repeated word from the translation,
  along with the whitespace separating it from the first occurrence.
- **Safe**: no.
- **Caveats**: a few constructions legitimately repeat a word — English "had had", "that that";
  French "que que" in subjunctive clauses; some proper names.

#### emails

- **Fix**: When the translation has the same number of emails as the source but at least one
  differs, replace each translation email in place with the email at the same position in the
  source. The "missing" and "extra" diagnostics (count mismatch) are not auto-fixable.
- **Safe**: no.
- **Caveats**: the translator may have intentionally used a localized contact address (e.g. a
  language-specific support inbox); the fix overwrites that choice with the source's email.

#### functions

- **Fix**: When the translation has the same number of function names as the source but at least
  one differs, replace each translation function name in place with the function name at the same
  position in the source. The "missing" and "extra" diagnostics (count mismatch) are not
  auto-fixable.
- **Safe**: no.
- **Caveats**: the translator may have reordered function references in the prose; positional
  pairing then maps a translation name to a different source name and the fix renames it
  incorrectly.

#### header

- **Fix**: Append a default value to the header for `Content-Type` (`text/plain; charset=UTF-8`)
  and `Content-Transfer-Encoding` (`8bit`) when those fields are missing. Other missing fields
  and all "invalid value" diagnostics are not auto-fixable because the correct value depends
  on per-file context (language, encoding, contacts, dates).
- **Safe**: yes.

#### html-tags

- **Fix**: When the translation has the same number of HTML tags as the source but at least one
  differs, replace each translation tag in place with the tag at the same position in the source.
  The "missing" and "extra" diagnostics (count mismatch) are not auto-fixable.
- **Safe**: no.
- **Caveats**: the translator may have reordered tags in the prose; positional pairing then maps
  a translation tag to a different source tag and the fix replaces it incorrectly.

#### newlines

- **Fix**: Mirror the source's leading and trailing `\r`/`\n` runs in the translation. The
  "count" diagnostics (mid-string newline mismatches) are not auto-fixable.
- **Safe**: yes.

#### numbers

- **Fix**: When the translation has as many numbers as the source but at least one differs,
  replace each translation number in place with the number at the same position in the source.
  The "missing" and "extra" diagnostics (count mismatch) are not auto-fixable.
- **Safe**: no.
- **Caveats**: the source number is copied verbatim, so the fix can bring the source's
  digit-group and decimal separators into a language using others (e.g. `1,000` written in a
  French translation instead of `1 000`).

#### obsolete

- **Fix**: Delete the entire obsolete entry from the file, including any leading comments and
  the trailing blank-line separator.
- **Safe**: yes.

#### options

- **Fix**: When the translation has the same number of command-line options as the source but
  at least one differs, replace each translation option in place with the option at the same
  position in the source. The edit spans the option name only, so the value of `--opt=value` is
  kept. The "missing" and "extra" diagnostics (count mismatch) are not auto-fixable.
- **Safe**: no.
- **Caveats**: the translator may have reordered options in the prose; positional pairing then
  maps a translation option to a different source option and the fix renames it incorrectly.

#### paths

- **Fix**: When the translation has the same number of paths as the source but at least one
  differs, replace each translation path in place with the path at the same position in the
  source. The "missing" and "extra" diagnostics (count mismatch) are not auto-fixable.
- **Safe**: no.
- **Caveats**: the translator may have intentionally used a localized path (e.g. a directory
  name that is part of an example string rather than a real filesystem reference); the fix
  overwrites that choice with the source's path.

#### punc-start

- **Fix**: Replace the leading punctuation run in the translation with the source's run.
- **Safe**: yes.

#### punc-end

- **Fix**: Replace the trailing punctuation run in the translation with the source's run.
- **Safe**: yes.

#### punc-space-str

- **Fix**: Insert or replace spaces around punctuation in the translation to match the
  target-language convention (NBSP before `:`, `;`, `!`, `?`, around `«` `»`, and between a
  digit and `%` in French; regular space in Finnish).
- **Safe**: yes.

#### unicode-ctrl

- **Fix**: Remove every stray Unicode control or format character from the translation (NULL,
  soft hyphen, zero-width spaces, bidi overrides, BOM, …) that is not present in the source.
- **Safe**: yes.

#### urls

- **Fix**: When the translation has the same number of URLs as the source but at least one
  differs, replace each translation URL in place with the URL at the same position in the
  source. The "missing" and "extra" diagnostics (count mismatch) are not auto-fixable.
- **Safe**: no.
- **Caveats**: the translator may have intentionally used a localized URL (e.g. a page with a
  language prefix such as `/fr/about` instead of `/about`); the fix overwrites that choice
  with the source's URL.

#### variables

- **Fix**: When the translation has the same number of variables as the source but at least one
  differs, replace each translation variable in place with the variable at the same position in
  the source. The "missing" and "extra" diagnostics (count mismatch) are not auto-fixable.
- **Safe**: no.
- **Caveats**: the translator may have reordered variables in the prose; positional pairing then
  maps a translation variable to a different source variable and the fix renames it incorrectly.

#### whitespace-start

- **Fix**: Replace the leading whitespace run in the translation with the source's run.
- **Safe**: yes.

#### whitespace-end

- **Fix**: Replace the trailing whitespace run in the translation with the source's run.
- **Safe**: yes.

#### whitespace-line-start

- **Fix**: Replace the leading whitespace run of each interior line in the translation with the
  source's run. Only the lines after an embedded newline are touched (the string start is handled
  by `whitespace-start`), and only when source and translation have the same number of lines.
- **Safe**: yes.

#### whitespace-line-end

- **Fix**: Replace the trailing whitespace run of each interior line in the translation with the
  source's run. Only the lines before an embedded newline are touched (the string end is handled
  by `whitespace-end`), and only when source and translation have the same number of lines.
- **Safe**: yes.

### Output

The environment variable `CLICOLOR_FORCE` can be set to `1` to force output with colors even when you pipe the command to another program.

For example pipe with less and keep colors:

```shell
CLICOLOR_FORCE=1 poexam check | less -R
```

### Statistics

Poexam can also give statistics about the translation progress and number of lines/words/characters, see: `poexam help stats`.

Example:

```text
$ poexam stats --sort status
po/de.po    [████████████████████] 3853 = 3853 (100%) + 0 (0%) + 0 (0%) + 0 (0%)
po/fr.po    [████████████████████] 3853 = 3853 (100%) + 0 (0%) + 0 (0%) + 0 (0%)
po/pl.po    [████████████████████] 3853 = 3853 (100%) + 0 (0%) + 0 (0%) + 0 (0%)
po/sr.po    [████████████████████] 3853 = 3853 (100%) + 0 (0%) + 0 (0%) + 0 (0%)
po/tr.po    [█████████████▒▒     ] 3853 = 2519 (65%) + 480 (12%) + 854 (22%) + 0 (0%)
po/ja.po    [█████████▒▒▒▒       ] 3853 = 1846 (47%) + 836 (21%) + 1171 (30%) + 0 (0%)
po/pt.po    [████████▒▒▒▒▒       ] 3853 = 1586 (41%) + 1002 (26%) + 1265 (32%) + 0 (0%)
po/es.po    [██████▒▒▒▒▒         ] 3853 = 1259 (32%) + 1113 (28%) + 1481 (38%) + 0 (0%)
po/it.po    [██████▒▒▒▒▒▒        ] 3853 = 1226 (31%) + 1183 (30%) + 1444 (37%) + 0 (0%)
po/cs.po    [██████▒▒▒▒▒▒        ] 3853 = 1178 (30%) + 1240 (32%) + 1435 (37%) + 0 (0%)
po/pt_BR.po [████▒▒▒▒▒▒          ] 3853 = 853 (22%) + 1163 (30%) + 1837 (47%) + 0 (0%)
po/ru.po    [▒▒▒▒▒▒▒▒▒           ] 3853 = 96 (2%) + 1782 (46%) + 1975 (51%) + 0 (0%)
po/hu.po    [▒▒▒▒▒▒▒▒▒           ] 3853 = 76 (1%) + 1766 (45%) + 2011 (52%) + 0 (0%)
Total (13)  [██████████▒▒▒▒      ] 50089 = 26051 (52%) + 10565 (21%) + 13473 (26%) + 0 (0%)
```

Detailed statistics on words and characters:

```text
$ poexam stats --words fr.po ja.po
fr.po:
                    Entries          Words (src / translated)     Chars (src / translated)
Translated           3853 (100%)      40634 (100%)      48504     184916 (100%)     226692
Fuzzy                   0 (  0%)          0 (  0%)          0          0 (  0%)          0
Untranslated            0 (  0%)          0 (  0%)          0          0 (  0%)          0
Obsolete                0 (  0%)          0 (  0%)          0          0 (  0%)          0
Total                3853             40634             48504     184916            226692

ja.po:
                    Entries          Words (src / translated)     Chars (src / translated)
Translated           1846 ( 47%)      16879 ( 41%)       7671      74349 ( 40%)      50055
Fuzzy                 836 ( 21%)       9763 ( 24%)       4787      45188 ( 24%)      29197
Untranslated         1171 ( 30%)      13992 ( 34%)          0      65379 ( 35%)          0
Obsolete                0 (  0%)          0 (  0%)          0          0 (  0%)          0
Total                3853             40634              7671     184916             50055

Total (2):
                    Entries          Words (src / translated)     Chars (src / translated)
Translated           5699 ( 73%)      57513 ( 70%)      56175     259265 ( 70%)     276747
Fuzzy                 836 ( 10%)       9763 ( 12%)       4787      45188 ( 12%)      29197
Untranslated         1171 ( 15%)      13992 ( 17%)          0      65379 ( 17%)          0
Obsolete                0 (  0%)          0 (  0%)          0          0 (  0%)          0
Total                7706             81268             56175     369832            276747
```

### Editor integration

Poexam ships a language server (LSP) so editors can show diagnostics in real time while you edit
PO files:

```shell
poexam lsp
```

The server speaks LSP over stdin/stdout. It checks the editor's in-memory buffer (even before it is
saved), discovers the closest `poexam.toml` from the file path, and publishes info/warning/error
diagnostics with the rule name as the diagnostic code. Point your editor's language-server client at
`poexam lsp` and associate it with `*.po` files.

Rules that inspect the file on disk rather than the buffer (only `compilation`, which runs `msgfmt`)
run when the file is opened and saved, and are skipped while there are unsaved changes so they never
report on stale content.

#### Emacs

Using gettext's `po-mode` together with
[Eglot](https://www.gnu.org/software/emacs/manual/html_mono/eglot.html), the LSP client built into
Emacs 29+, add to your `init.el`:

```elisp
;; Run poexam as the language server for PO files
(with-eval-after-load 'eglot
  (add-to-list 'eglot-server-programs '(po-mode . ("poexam" "lsp"))))
(add-hook 'po-mode-hook #'eglot-ensure)
```

`poexam` must be on the `PATH` that Emacs sees (use an absolute path otherwise). Diagnostics appear
as Flymake overlays; navigate them with `flymake-goto-next-error` / `flymake-goto-prev-error` (not
`next-error` / `M-g n`, which is a separate framework), or list them with
`flymake-show-buffer-diagnostics`. The navigation commands are unbound by default; bind them, e.g.:

```elisp
(with-eval-after-load 'flymake
  (define-key flymake-mode-map (kbd "M-n") #'flymake-goto-next-error)
  (define-key flymake-mode-map (kbd "M-p") #'flymake-goto-prev-error))
```

#### Zed

A [Zed](https://zed.dev) extension built on this language server lives in
[`editors/zed`](editors/zed); it adds PO syntax highlighting and real-time diagnostics. See its
[README](editors/zed/README.md) for installation.

## Copyright

<!-- REUSE-IgnoreStart -->
Copyright © 2026 [Sébastien Helleu](https://github.com/flashcode)

This file is part of Poexam, the blazingly fast PO file linter.

Poexam is free software; you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation; either version 3 of the License, or
(at your option) any later version.

Poexam is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with Poexam.  If not, see <https://gnu.org/licenses/>.
<!-- REUSE-IgnoreEnd -->
