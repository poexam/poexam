; SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
;
; SPDX-License-Identifier: GPL-3.0-or-later

; Highlight queries for the tree-sitter-po grammar
; (https://github.com/tree-sitter-grammars/tree-sitter-po).

; Keywords

[
  "msgctxt"
  "msgid"
  "msgid_plural"
  "msgstr"
  "msgstr_plural"
] @keyword

; Punctuation

[ "[" "]" ] @punctuation.bracket

; Literals

(string) @string

(escape_sequence) @string.escape

(number) @number

; Comments

(comment) @comment

(comment (reference (text) @string.special))

(comment (flag (text) @label))
