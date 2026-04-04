// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format strings: no language.

use crate::po::format::FormatParser;

pub struct FormatNull;

impl FormatParser for FormatNull {
    #[inline]
    fn next_char(&self, s: &str, pos: usize) -> Option<(char, usize, bool)> {
        s[pos..]
            .chars()
            .next()
            .map(|c| (c, pos + c.len_utf8(), false))
    }

    #[inline]
    fn find_end_format(&self, _s: &str, _pos: usize, len: usize) -> usize {
        len
    }
}

#[cfg(test)]
mod tests {
    use crate::po::format::{
        format_pos::{FormatPos, strip_formats},
        language::Language,
    };

    #[test]
    fn test_no_format() {
        assert!(
            FormatPos::new("Hello, %s world!", &Language::Null)
                .next()
                .is_none()
        );
        assert_eq!(
            strip_formats("Hello, %s world!", &Language::Null),
            "Hello, %s world!"
        );
    }
}
