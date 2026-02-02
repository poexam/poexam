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
> The command-line interface and features may change at any time.\
> As it doesn't write anything on disk, there should be no risk of data loss, and bug reports are welcome.

## Overview

- ⚡️ **Blazingly fast**: large directories and files are checked in parallel, in a few milliseconds.
- 🔎 **Rules**: a lot of checks performed with very few false positives.
- 🎯 **Clear results**: tricky errors in strings are highlighted with colors.
- 📊 **Statistics**: detailed statistics including progress, count of messages/words/characters.
- 💻 **Multi-platform**: available wherever the Rust compiler is available.
- 🎁 **Free software**: released under [GPLv3](https://gnu.org/licenses/gpl-3.0.html).

## Installation

With cargo:

```bash
cargo install poexam
```

## Features

Poexam can check entire directories and a lot of PO files in just a few milliseconds.

It can perform a lot of checks via the default rules:

| Rule name        | Severity | Diagnostic reported                    |
|------------------|----------|----------------------------------------|
| blank            | warning  | Blank translation (only whitespace).   |
| brackets         | info     | Missing/extra brackets.                |
| c-formats        | error    | Inconsistent C format strings.         |
| double-quotes    | info     | Missing/extra double quotes.           |
| double-spaces    | info     | Missing or extra double spaces.        |
| encoding         | info     | Incorrect encoding (charset).          |
| escapes          | error    | Missing/extra escape characters.       |
| newlines         | error    | Missing/extra newlines.                |
| pipes            | info     | Missing/extra pipes.                   |
| plurals          | error    | Incorrect number of plurals.           |
| punc-end         | info     | Inconsistent trailing punctuation.     |
| punc-start       | info     | Inconsistent leading punctuation.      |
| tabs             | error    | Missing/extra tabs.                    |
| whitespace-end   | info     | Missing/extra whitespace at the end.   |
| whitespace-start | info     | Missing/extra whitespace at the start. |

 Some extra rules are not used by default because they are not really "checks" or report too many false positives.

 You can enable them on-demand:

| Rule name    | Severity | Diagnostic reported                           |
|--------------|----------|-----------------------------------------------|
| fuzzy        | info     | Fuzzy entry.                                  |
| obsolete     | info     | Obsolete entry.                               |
| unchanged    | info     | Translation is the same as the source string. |
| untranslated | info     | Untranslated entry.                           |

The result is very clear, almost all errors are highlighted in the strings so you can immediately see where the issue is.

You can check by yourself with the following command executed in the root directory of the project (output is truncated here):

```text
$ poexam check --select all examples/fr.po
examples/fr.po:25: [warning:blank] blank translation
        |
     25 | Test: blank translation
        |
     26 |
        |

examples/fr.po:29: [info:brackets] missing opening and closing square brackets '[' (1 / 0) and ']' (1 / 0)
        |
     29 | Test [brackets]
        |
     30 | Test brackets
        |

examples/fr.po:34: [error:c-formats] inconsistent C format strings
        |
     34 | Name: %s, age: %d
        |
     35 | Âge : %2$d, nom : %1$f
        |

examples/fr.po:38: [info:double-quotes] missing double quotes (2 / 0)
        |
     38 | Test "double quotes"
        |
     39 | Test guillemets doubles
        |

(...)

1 files checked: 19 problems in 1 files (5 errors, 1 warnings, 13 info) [848.884µs]
```

This file [examples/fr.po](examples/fr.po) contains one error per rule.

Poexam can also give statistics about the translation progress and number of lines/words/charecters, see: `poexam help stats`.

Example:

```text
$ poexam stats
cs.po      [██████▒▒▒▒▒▒        ] 3853 = 1178 (30%) + 1240 (32%) + 1435 (37%) + 0 (0%)
de.po      [████████████████████] 3853 = 3853 (100%) + 0 (0%) + 0 (0%) + 0 (0%)
es.po      [██████▒▒▒▒▒         ] 3853 = 1259 (32%) + 1113 (28%) + 1481 (38%) + 0 (0%)
fr.po      [████████████████████] 3853 = 3853 (100%) + 0 (0%) + 0 (0%) + 0 (0%)
hu.po      [▒▒▒▒▒▒▒▒▒           ] 3853 = 76 (1%) + 1766 (45%) + 2011 (52%) + 0 (0%)
it.po      [██████▒▒▒▒▒▒        ] 3853 = 1226 (31%) + 1183 (30%) + 1444 (37%) + 0 (0%)
ja.po      [█████████▒▒▒▒       ] 3853 = 1846 (47%) + 836 (21%) + 1171 (30%) + 0 (0%)
pl.po      [████████████████████] 3853 = 3853 (100%) + 0 (0%) + 0 (0%) + 0 (0%)
pt.po      [████████▒▒▒▒▒       ] 3853 = 1586 (41%) + 1002 (26%) + 1265 (32%) + 0 (0%)
pt_BR.po   [████▒▒▒▒▒▒          ] 3853 = 853 (22%) + 1163 (30%) + 1837 (47%) + 0 (0%)
ru.po      [▒▒▒▒▒▒▒▒▒           ] 3853 = 96 (2%) + 1782 (46%) + 1975 (51%) + 0 (0%)
sr.po      [████████████████████] 3853 = 3853 (100%) + 0 (0%) + 0 (0%) + 0 (0%)
tr.po      [█████████████▒▒     ] 3853 = 2519 (65%) + 480 (12%) + 854 (22%) + 0 (0%)
Total (13) [██████████▒▒▒▒      ] 50089 = 26051 (52%) + 10565 (21%) + 13473 (26%) + 0 (0%)
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

- [ ] Add spell checking for source and translated strings.
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
