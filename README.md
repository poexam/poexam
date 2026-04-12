<!--
SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>

SPDX-License-Identifier: GPL-3.0-or-later
-->

# Poexam

[![Crates.io](https://img.shields.io/crates/v/poexam.svg)](https://crates.io/crates/poexam)
[![Build status](https://github.com/poexam/poexam/workflows/CI/badge.svg)](https://github.com/poexam/poexam/actions?query=workflow%3A%22CI%22)
[![REUSE status](https://api.reuse.software/badge/github.com/poexam/poexam)](https://api.reuse.software/info/github.com/poexam/poexam)

**Poexam** is a blazingly fast PO file linter with a comprehensive diagnostic report.

It reports very few false positives and can be used in CI jobs and pre-commit hooks.

> [!NOTE]
> Poexam is in active development and may not be fully stable yet.\
> The command-line interface and features may change at any time.

## Overview

- ⚡️ **Blazingly fast**: large directories and files are checked in parallel, in a few milliseconds.
- 🔎 **Rules**: a lot of checks performed with very few false positives.
- 🎯 **Clear results**: tricky errors in strings are highlighted with colors.
- 📊 **Statistics**: detailed statistics including progress, count of messages/words/characters.
- 💻 **Multi-platform**: available wherever the Rust compiler is available.
- 🎁 **Free software**: released under [GPLv3](https://gnu.org/licenses/gpl-3.0.html).

## Installation

With cargo:

```shell
cargo install poexam
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
| select               | Array of strings | Selected rules.                                                   |
| ignore               | Array of strings | Ignored rules.                                                    |
| path_msgfmt          | String (path)    | Path to `msgfmt` for PO file compilation.                         |
| path_dicts           | String (path)    | Path to the Hunspell dictionaries.                                |
| path_words           | String (path)    | Path with custom words (absolute or relative to the config file). |
| lang_id              | String           | Language used to check source strings.                            |
| langs                | Array of strings | Check spelling only for these languages.                          |
| severity             | Array of strings | Run only checks with these severities (`info`/`warning`/`error`). |
| punc_ignore_ellipsis | Boolean          | Ignore ellipsis differences (`...` vs `…`) in punc rules.         |

See configuration file example: [poexam.toml](examples/poexam.toml).

### Rules

It can perform a lot of checks via the default rules:

| Rule name        | Severity | Diagnostic reported                                |
|------------------|----------|----------------------------------------------------|
| blank            | warning  | Blank translation (only whitespace).               |
| brackets         | info     | Missing/extra brackets.                            |
| double-quotes    | info     | Missing/extra double quotes.                       |
| double-spaces    | info     | Missing/extra double spaces.                       |
| emails           | info     | Missing/extra/different emails.                    |
| encoding         | info     | Incorrect encoding (charset).                      |
| escapes          | error    | Missing/extra escape characters.                   |
| formats          | error    | Inconsistent format strings.                       |
| long             | warning  | Translation too long.                              |
| newlines         | error    | Missing/extra newlines.                            |
| pipes            | info     | Missing/extra pipes.                               |
| plurals          | error    | Incorrect number of plurals.                       |
| punc-start       | info     | Inconsistent leading punctuation.                  |
| punc-end         | info     | Inconsistent trailing punctuation.                 |
| punc-space-id    | info     | Incorrect spaces around punctuation (source).      |
| punc-space-str   | info     | Incorrect spaces around punctuation (translation). |
| short            | warning  | Translation too short.                             |
| tabs             | error    | Missing/extra tabs.                                |
| urls             | info     | Missing/extra/different URLs.                      |
| whitespace-end   | info     | Missing/extra whitespace at the end.               |
| whitespace-start | info     | Missing/extra whitespace at the start.             |

For the rule `formats`, the following languages are supported:

- C (`c-format`): printf format (e.g. `%s %12lld`)
- Python (`python-format`): Python % format strings (e.g. `%s %(age)d`)
- Python brace (`python-brace-format`): Python brace format strings (e.g. `{0!r:20} {1}`).

 Some extra rules are not used by default because they are not really "checks",
 report too many false positives or can slow down the process.

 You can enable them on-demand:

| Rule name      | Severity | Diagnostic reported                              |
|----------------|----------|--------------------------------------------------|
| changed        | info     | Translation is different from the source string. |
| compilation    | error    | Compilation with `msgfmt`.                       |
| double-words   | info     | Translation has consecutive repeated words.      |
| fuzzy          | info     | Fuzzy entry.                                     |
| noqa           | info     | Entry has `noqa` comment.                        |
| obsolete       | info     | Obsolete entry.                                  |
| paths          | info     | Missing/extra/different paths.                   |
| spelling-ctxt  | info     | Spelling error (context).                        |
| spelling-id    | info     | Spelling error (source).                         |
| spelling-str   | info     | Spelling error (translation).                    |
| unchanged      | info     | Translation is the same as the source string.    |
| untranslated   | info     | Untranslated entry.                              |

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

### Output

The environment variable `CLICOLOR_FORCE` can be set to `1` to force output with colors even when you pipe the command to another program.

For example pipe with less and keep colors:

```shell
CLICOLOR_FORCE=1 poexam check | less -R
```

### Spell checking

You can check all words in a file by using one of these rules:

- `spelling-ctxt`: check all words in context strings (`msgctxt`) with English `en_US` dictionary.
- `spelling-id`: check all words in source strings (`msgid`) with English `en_US` dictionary.
- `spelling-str`: check all words in translated strings (`msgstr`) with the language found in PO file header.

The special rule `spelling` can be used to select these 3 rules at once.

For rules `spelling-ctxt` and `spelling-id`, the default dictionary used is `en_US` and can be changed with the option `--lang-id`.

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

## Roadmap

- [ ] Add new rules.
- [ ] Add support for custom rules and checks.
- [ ] Add a server mode to integrate with IDEs and text editors.

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
