// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Implementation of all rules.

pub mod blank;
pub mod brackets;
pub mod c_formats;
pub mod changed;
pub mod double_quotes;
pub mod double_spaces;
pub mod encoding;
pub mod escapes;
pub mod fuzzy;
pub mod newlines;
pub mod obsolete;
pub mod pipes;
pub mod plurals;
pub mod punc;
pub mod rule;
pub mod spelling;
pub mod tabs;
pub mod unchanged;
pub mod untranslated;
pub mod whitespace;
