// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! URL iterator: return URLs of a string, skipping format strings.

use crate::po::format::{FormatParser, MatchStrPos, language::Language};

pub struct UrlPos<'a> {
    s: &'a str,
    len: usize,
    pos: usize,
    fmt: Box<dyn FormatParser>,
}

impl<'a> UrlPos<'a> {
    pub fn new(s: &'a str, language: &Language) -> Self {
        Self {
            s,
            len: s.len(),
            pos: 0,
            fmt: language.format_parser(),
        }
    }
}

impl<'a> Iterator for UrlPos<'a> {
    type Item = MatchStrPos<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut idx_start = None;
        let mut idx_end = None;
        loop {
            while self.pos < self.len {
                if idx_start.is_none() {
                    let (new_pos, is_format) = self.fmt.next_char(self.s, self.pos, self.len);
                    self.pos = new_pos;
                    if self.pos >= self.len {
                        return None;
                    }
                    if is_format {
                        self.pos = self.fmt.find_end_format(self.s, self.pos, self.len);
                        continue;
                    }
                }
                match self.s[self.pos..].chars().next() {
                    Some(c) => {
                        let len_c = c.len_utf8();
                        if !c.is_whitespace() {
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
            match (idx_start, idx_end) {
                (Some(start), Some(end)) => {
                    let s = &self.s[start..end];
                    if s.contains("://") {
                        return Some(MatchStrPos { s, start, end });
                    }
                    idx_start = None;
                    idx_end = None;
                }
                _ => return None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::po::format::MatchStrPos;

    #[test]
    fn test_no_words() {
        assert!(UrlPos::new("", &Language::Null).next().is_none());
        assert!(UrlPos::new(" ,.!? ", &Language::Null).next().is_none());
    }

    #[test]
    fn test_words() {
        assert_eq!(
            UrlPos::new(
                "héllo, мир! https://google.com/ http:/fake ftp://example.com",
                &Language::Null
            )
            .collect::<Vec<_>>(),
            vec![
                MatchStrPos {
                    s: "https://google.com/",
                    start: 16,
                    end: 35,
                },
                MatchStrPos {
                    s: "ftp://example.com",
                    start: 47,
                    end: 64,
                },
            ]
        );
    }
}
