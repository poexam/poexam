// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Utilities for working with words and characters in PO file entries.

use crate::c_format::get_index_end_c_format;

pub struct WordPos<'a> {
    s: &'a str,
    bytes: &'a [u8],
    len: usize,
    skip_c_format: bool,
    pos: usize,
}

pub struct CharPos<'a> {
    s: &'a str,
    bytes: &'a [u8],
    len: usize,
    skip_c_format: bool,
    pos: usize,
}

/// Count words in a string with optional skip of C format strings.
impl<'a> WordPos<'a> {
    /// Create a new `WordPos` iterator.
    ///
    /// Argument `format` can be `c` or an empty string.
    pub fn new(s: &'a str, format: &str) -> Self {
        let bytes = s.as_bytes();
        let len = bytes.len();
        Self {
            s,
            bytes,
            len,
            skip_c_format: format == "c",
            pos: 0,
        }
    }
}

/// Count characters in a string with optional skip of C format strings.
impl<'a> CharPos<'a> {
    /// Create a new `CharPos` iterator.
    ///
    /// Argument `format` can be `c` or an empty string.
    pub fn new(s: &'a str, format: &str) -> Self {
        let bytes = s.as_bytes();
        let len = bytes.len();
        Self {
            s,
            bytes,
            len,
            skip_c_format: format == "c",
            pos: 0,
        }
    }
}

impl Iterator for WordPos<'_> {
    type Item = (usize, usize);

    fn next(&mut self) -> Option<Self::Item> {
        let mut idx_start = None;
        let mut idx_end = None;
        while self.pos < self.len {
            // Skip C format strings.
            if self.skip_c_format && idx_start.is_none() && self.bytes[self.pos] == b'%' {
                self.pos += 1;
                if self.pos < self.len && self.bytes[self.pos] == b'%' {
                    self.pos += 1;
                } else {
                    self.pos = get_index_end_c_format(self.bytes, self.pos, self.len);
                }
                if self.pos >= self.len {
                    return None;
                }
            } else {
                match self.s[self.pos..].chars().next() {
                    Some(c) => {
                        let len_c = c.len_utf8();
                        if c.is_alphanumeric() || (idx_start.is_some() && c == '-') {
                            if idx_start.is_none() {
                                idx_start = Some(self.pos);
                            }
                            idx_end = Some(self.pos + len_c);
                        } else if idx_start.is_some() {
                            break;
                        }
                        self.pos += len_c;
                    }
                    None => return None,
                }
            }
        }
        match (idx_start, idx_end) {
            (Some(start), Some(end)) => Some((start, end)),
            _ => None,
        }
    }
}

impl Iterator for CharPos<'_> {
    type Item = (usize, usize);

    fn next(&mut self) -> Option<Self::Item> {
        while self.pos < self.len {
            // Skip C format strings.
            if self.skip_c_format && self.bytes[self.pos] == b'%' {
                self.pos += 1;
                if self.pos < self.len && self.bytes[self.pos] == b'%' {
                    self.pos += 1;
                } else {
                    self.pos = get_index_end_c_format(self.bytes, self.pos, self.len);
                }
                if self.pos >= self.len {
                    return None;
                }
            } else {
                match self.s[self.pos..].chars().next() {
                    Some(c) => {
                        let len_c = c.len_utf8();
                        if c.is_alphanumeric() || c == '-' {
                            let result = (self.pos, self.pos + len_c);
                            self.pos += len_c;
                            return Some(result);
                        }
                        self.pos += len_c;
                    }
                    None => return None,
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        let s = "";
        let pos: Vec<_> = WordPos::new(s, "").collect();
        assert!(pos.is_empty());
        let s = "";
        let pos: Vec<_> = CharPos::new(s, "").collect();
        assert!(pos.is_empty());
    }

    #[test]
    fn test_punct() {
        let s = " ,.!? ";
        let pos: Vec<_> = WordPos::new(s, "").collect();
        assert!(pos.is_empty());
        let s = " ,.!? ";
        let pos: Vec<_> = CharPos::new(s, "").collect();
        assert!(pos.is_empty());
    }

    #[test]
    fn test_c_format() {
        let s = "%05d";
        // Do not skip any format.
        let pos: Vec<_> = WordPos::new(s, "").collect();
        assert_eq!(pos, vec![(1, 4)]);
        assert_eq!(&s[pos[0].0..pos[0].1], "05d");
        let pos: Vec<_> = CharPos::new(s, "").collect();
        assert_eq!(pos, vec![(1, 2), (2, 3), (3, 4)]);
        assert_eq!(&s[pos[0].0..pos[0].1], "0");
        assert_eq!(&s[pos[1].0..pos[1].1], "5");
        assert_eq!(&s[pos[2].0..pos[2].1], "d");
        // Skip C format strings.
        let pos: Vec<_> = WordPos::new(s, "c").collect();
        assert!(pos.is_empty());
        let pos: Vec<_> = CharPos::new(s, "c").collect();
        assert!(pos.is_empty());
    }

    #[test]
    fn test_basic_ascii() {
        let s = "Hello, world! %llu test-word 42.";
        // Do not skip any format.
        let pos: Vec<_> = WordPos::new(s, "").collect();
        assert_eq!(pos, vec![(0, 5), (7, 12), (15, 18), (19, 28), (29, 31)]);
        assert_eq!(&s[pos[0].0..pos[0].1], "Hello");
        assert_eq!(&s[pos[1].0..pos[1].1], "world");
        assert_eq!(&s[pos[2].0..pos[2].1], "llu");
        assert_eq!(&s[pos[3].0..pos[3].1], "test-word");
        assert_eq!(&s[pos[4].0..pos[4].1], "42");
        let pos: Vec<_> = CharPos::new(s, "").collect();
        assert_eq!(pos.len(), 24);
        // Skip C format strings.
        let pos: Vec<_> = WordPos::new(s, "c").collect();
        assert_eq!(pos, vec![(0, 5), (7, 12), (19, 28), (29, 31)]);
        assert_eq!(&s[pos[0].0..pos[0].1], "Hello");
        assert_eq!(&s[pos[1].0..pos[1].1], "world");
        assert_eq!(&s[pos[2].0..pos[2].1], "test-word");
        assert_eq!(&s[pos[3].0..pos[3].1], "42");
        let pos: Vec<_> = CharPos::new(s, "c").collect();
        assert_eq!(pos.len(), 21);
    }

    #[test]
    fn test_unicode() {
        let s = "héllo, мир! %lld 你好";
        // Do not skip any format.
        let pos: Vec<_> = WordPos::new(s, "").collect();
        assert_eq!(pos, vec![(0, 6), (8, 14), (17, 20), (21, 27)]);
        assert_eq!(&s[pos[0].0..pos[0].1], "héllo");
        assert_eq!(&s[pos[1].0..pos[1].1], "мир");
        assert_eq!(&s[pos[2].0..pos[2].1], "lld");
        assert_eq!(&s[pos[3].0..pos[3].1], "你好");
        let pos: Vec<_> = CharPos::new(s, "").collect();
        assert_eq!(pos.len(), 13);
        // Skip C format strings.
        let pos: Vec<_> = WordPos::new(s, "c").collect();
        assert_eq!(pos, vec![(0, 6), (8, 14), (21, 27)]);
        assert_eq!(&s[pos[0].0..pos[0].1], "héllo");
        assert_eq!(&s[pos[1].0..pos[1].1], "мир");
        assert_eq!(&s[pos[2].0..pos[2].1], "你好");
        let pos: Vec<_> = CharPos::new(s, "c").collect();
        assert_eq!(pos.len(), 10);
    }
}
