<!--
SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>

SPDX-License-Identifier: GPL-3.0-or-later
-->

# Poexam ChangeLog

## Version 0.0.3 (under dev)

### Fixed

- Fix detection of C formats in strings

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
