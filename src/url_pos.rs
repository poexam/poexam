// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! URL iterator: return URLs of a string.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchUrlPos<'a> {
    pub s: &'a str,
    pub start: usize,
    pub end: usize,
}

pub struct UrlPos<'a> {
    s: &'a str,
    pos: usize,
}

impl<'a> UrlPos<'a> {
    pub fn new(s: &'a str) -> Self {
        Self { s, pos: 0 }
    }
}

impl<'a> Iterator for UrlPos<'a> {
    type Item = MatchUrlPos<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut idx_start = None;
        let mut idx_end = None;
        for c in self.s[self.pos..].chars() {
            let len_c = c.len_utf8();
            if !c.is_whitespace() {
                if idx_start.is_none() {
                    idx_start = Some(self.pos);
                }
                idx_end = Some(self.pos + len_c);
            } else if idx_start.is_some() {
                self.pos += len_c;
                match (idx_start, idx_end) {
                    (Some(start), Some(end)) => {
                        let url = &self.s[start..end];
                        if url.contains("://") {
                            return Some(MatchUrlPos { s: url, start, end });
                        }
                        idx_start = None;
                        idx_end = None;
                    }
                    _ => return None,
                }
                continue;
            }
            self.pos += len_c;
        }
        match (idx_start, idx_end) {
            (Some(start), Some(end)) => {
                let url = &self.s[start..end];
                if url.contains("://") {
                    Some(MatchUrlPos { s: url, start, end })
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_urls() {
        assert!(UrlPos::new("").next().is_none());
        assert!(UrlPos::new(" ,.!? ").next().is_none());
    }

    #[test]
    fn test_urls() {
        assert_eq!(
            UrlPos::new(
                "héllo, мир! https://google.com/  https://test.com  http:/fake ftp://example.com"
            )
            .collect::<Vec<_>>(),
            vec![
                MatchUrlPos {
                    s: "https://google.com/",
                    start: 16,
                    end: 35,
                },
                MatchUrlPos {
                    s: "https://test.com",
                    start: 37,
                    end: 53,
                },
                MatchUrlPos {
                    s: "ftp://example.com",
                    start: 66,
                    end: 83,
                },
            ]
        );
    }
}
