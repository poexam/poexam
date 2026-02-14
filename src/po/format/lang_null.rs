// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format strings: no language.

use crate::po::format::FormatParser;

pub struct FormatNull;

impl FormatParser for FormatNull {
    #[inline]
    fn next_char(&self, _s: &str, pos: usize, _len: usize) -> (usize, bool) {
        (pos, false)
    }

    #[inline]
    fn find_end_format(&self, _s: &str, _pos: usize, len: usize) -> usize {
        len
    }
}

#[cfg(test)]
mod tests {
    use crate::po::format::{
        MatchStrPos, char_pos::CharPos, format_pos::FormatPos, language::Language,
        word_pos::WordPos,
    };

    #[test]
    fn test_no_format() {
        let s = "Hello, %s world!";
        // Formats: none.
        assert!(FormatPos::new(s, &Language::Null).next().is_none());
        // Words: "Hello", "s", "world".
        let mut word_pos = WordPos::new(s, &Language::Null);
        assert_eq!(
            word_pos.next(),
            Some(MatchStrPos {
                s: "Hello",
                start: 0,
                end: 5,
            })
        );
        assert_eq!(
            word_pos.next(),
            Some(MatchStrPos {
                s: "s",
                start: 8,
                end: 9,
            })
        );
        assert_eq!(
            word_pos.next(),
            Some(MatchStrPos {
                s: "world",
                start: 10,
                end: 15,
            })
        );
        assert!(word_pos.next().is_none());
        // Chars: 'H', 'é', 's', 'w'.
        let mut char_pos = CharPos::new("Hé, %s w!", &Language::Null);
        assert_eq!(
            char_pos.next(),
            Some(MatchStrPos {
                s: "H",
                start: 0,
                end: 1,
            })
        );
        assert_eq!(
            char_pos.next(),
            Some(MatchStrPos {
                s: "é",
                start: 1,
                end: 3,
            })
        );
        assert_eq!(
            char_pos.next(),
            Some(MatchStrPos {
                s: "s",
                start: 6,
                end: 7,
            })
        );
        assert_eq!(
            char_pos.next(),
            Some(MatchStrPos {
                s: "w",
                start: 8,
                end: 9,
            })
        );
        assert!(char_pos.next().is_none());
    }
}
