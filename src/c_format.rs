// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Utilities for C-format strings.

pub struct CFormat<'a> {
    s: &'a str,
    bytes: &'a [u8],
    len: usize,
    pos: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchCFormat<'a> {
    pub format: &'a str,
    pub start: usize,
    pub end: usize,
}

impl<'a> CFormat<'a> {
    pub fn new(s: &'a str) -> Self {
        let bytes = s.as_bytes();
        let len = bytes.len();
        Self {
            s,
            bytes,
            len,
            pos: 0,
        }
    }
}

impl MatchCFormat<'_> {
    /// Get the reordering index if present, otherwise return `usize::MAX`.
    ///
    /// For example, for format `"%3$d"`, this function returns `3`.
    pub fn sort_index(&self) -> usize {
        let bytes = self.format.as_bytes();
        if bytes.is_empty() || bytes[0] != b'%' {
            return usize::MAX;
        }
        let mut pos = 1;
        while pos < bytes.len() && bytes[pos].is_ascii_digit() {
            pos += 1;
        }
        if pos == 1 || pos >= bytes.len() || bytes[pos] != b'$' {
            return usize::MAX;
        }
        match &self.format[1..pos].parse::<usize>() {
            Ok(index) => *index,
            Err(_) => usize::MAX,
        }
    }

    /// Return the format string without reordering part.
    ///
    /// For example, for format `"%3$d"`, this function returns `"%d"`.
    pub fn remove_reordering(&self) -> String {
        let bytes = self.format.as_bytes();
        if bytes.is_empty() || bytes[0] != b'%' {
            return self.format.to_string();
        }
        let mut pos = 1;
        while pos < bytes.len() && bytes[pos].is_ascii_digit() {
            pos += 1;
        }
        if pos == 1 || pos >= bytes.len() || bytes[pos] != b'$' {
            return self.format.to_string();
        }
        let mut result = String::from("%");
        result.push_str(&self.format[pos + 1..]);
        result
    }
}

impl Ord for MatchCFormat<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Sort matching formats by reordering index first (e.g. "%1$s" before "%2$d"),
        // then by start position, then by end position.
        self.sort_index()
            .cmp(&other.sort_index())
            .then(self.start.cmp(&other.start))
            .then(self.end.cmp(&other.end))
    }
}

impl PartialOrd for MatchCFormat<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Get the index of the end of a C format string.
///
/// `start` is the index of the first character of the format string (after `%`).
/// `len` is the length of `bytes`.
pub fn get_index_end_c_format(bytes: &[u8], start: usize, len: usize) -> usize {
    let mut pos = start;

    // Skip flags / width / precision / reordering.
    while pos < len {
        if matches!(
            bytes[pos],
            b'-' | b'+' | b' ' | b'#' | b'.' | b'$' | b'0'..=b'9'
        ) {
            pos += 1;
        } else {
            break;
        }
    }

    // Parse length modifiers (h, hh, l, ll, q, L, j, z, Z, t).
    if pos < len {
        match bytes[pos] {
            b'h' => {
                pos += 1;
                if pos < len && bytes[pos] == b'h' {
                    pos += 1;
                }
            }
            b'l' => {
                pos += 1;
                if pos < len && bytes[pos] == b'l' {
                    pos += 1;
                }
            }
            b'q' | b'L' | b'j' | b'z' | b'Z' | b't' => {
                pos += 1;
            }
            _ => {}
        }
    }

    // Parse conversion specifier (e.g. s, d, f, etc.).
    if pos < len && bytes[pos].is_ascii_alphabetic() {
        pos += 1;
    }

    pos
}

impl<'a> Iterator for CFormat<'a> {
    type Item = MatchCFormat<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.pos < self.len {
            if self.bytes[self.pos] != b'%' {
                self.pos += 1;
                continue;
            }
            let start = self.pos;
            self.pos += 1;

            // Handle escaped "%%".
            if self.pos < self.len && self.bytes[self.pos] == b'%' {
                self.pos += 1;
                continue;
            }

            self.pos = get_index_end_c_format(self.bytes, self.pos, self.len);

            return Some(MatchCFormat {
                format: &self.s[start..self.pos],
                start,
                end: self.pos,
            });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_sort_index() {
        let mf = MatchCFormat {
            format: "%3$d",
            start: 0,
            end: 4,
        };
        assert_eq!(mf.sort_index(), 3);
        let mf_no_reorder = MatchCFormat {
            format: "%d",
            start: 0,
            end: 2,
        };
        assert_eq!(mf_no_reorder.sort_index(), usize::MAX);
        let mf_invalid = MatchCFormat {
            format: "%$d",
            start: 0,
            end: 3,
        };
        assert_eq!(mf_invalid.sort_index(), usize::MAX);
    }

    #[test]
    fn test_match_remove_reordering() {
        let mf = MatchCFormat {
            format: "%3$d",
            start: 0,
            end: 4,
        };
        assert_eq!(mf.remove_reordering(), "%d");
        let mf_no_reorder = MatchCFormat {
            format: "%d",
            start: 0,
            end: 2,
        };
        assert_eq!(mf_no_reorder.remove_reordering(), "%d");
        let mf_invalid = MatchCFormat {
            format: "%$d",
            start: 0,
            end: 3,
        };
        assert_eq!(mf_invalid.remove_reordering(), "%$d");
    }

    #[test]
    fn test_no_format() {
        let s = "Hello, world!";
        let mut cf = CFormat::new(s);
        assert!(cf.next().is_none());
    }

    #[test]
    fn test_invalid() {
        let s = "%";
        let mut cf = CFormat::new(s);
        assert_eq!(
            cf.next(),
            Some(MatchCFormat {
                format: "%",
                start: 0,
                end: 1
            })
        );
        assert!(cf.next().is_none());
        let s = "%é";
        let mut cf = CFormat::new(s);
        assert_eq!(
            cf.next(),
            Some(MatchCFormat {
                format: "%",
                start: 0,
                end: 1
            })
        );
        assert!(cf.next().is_none());
    }

    #[test]
    fn test_single_format() {
        let s = "hello, %s world!";
        let mut cf = CFormat::new(s);
        assert_eq!(
            cf.next(),
            Some(MatchCFormat {
                format: "%s",
                start: 7,
                end: 9
            })
        );
        assert!(cf.next().is_none());
    }

    #[test]
    fn test_multiple_formats() {
        let s = "%d %s %f";
        let mut cf = CFormat::new(s);
        assert_eq!(
            cf.next(),
            Some(MatchCFormat {
                format: "%d",
                start: 0,
                end: 2
            })
        );
        assert_eq!(
            cf.next(),
            Some(MatchCFormat {
                format: "%s",
                start: 3,
                end: 5
            })
        );
        assert_eq!(
            cf.next(),
            Some(MatchCFormat {
                format: "%f",
                start: 6,
                end: 8
            })
        );
        assert!(cf.next().is_none());
    }

    #[test]
    fn test_multiple_formats_with_reordering() {
        let s = "Hello, %3$d %2$s %1$f world!";
        let mut cf = CFormat::new(s);
        assert_eq!(
            cf.next(),
            Some(MatchCFormat {
                format: "%3$d",
                start: 7,
                end: 11,
            })
        );
        assert_eq!(
            cf.next(),
            Some(MatchCFormat {
                format: "%2$s",
                start: 12,
                end: 16,
            })
        );
        assert_eq!(
            cf.next(),
            Some(MatchCFormat {
                format: "%1$f",
                start: 17,
                end: 21,
            })
        );
        assert!(cf.next().is_none());
    }

    #[test]
    fn test_escaped_percent() {
        let s = "Hello, %% %s world!";
        let mut cf = CFormat::new(s);
        assert_eq!(
            cf.next(),
            Some(MatchCFormat {
                format: "%s",
                start: 10,
                end: 12,
            })
        );
        assert!(cf.next().is_none());
    }

    #[test]
    fn test_flags_width_precision() {
        let s = "Hello, %05.2f world!";
        let mut cf = CFormat::new(s);
        assert_eq!(
            cf.next(),
            Some(MatchCFormat {
                format: "%05.2f",
                start: 7,
                end: 13,
            })
        );
        assert!(cf.next().is_none());
    }

    #[test]
    fn test_flags_width_length() {
        let s = "Hello, %ld %9llu world!";
        let mut cf = CFormat::new(s);
        assert_eq!(
            cf.next(),
            Some(MatchCFormat {
                format: "%ld",
                start: 7,
                end: 10,
            })
        );
        assert_eq!(
            cf.next(),
            Some(MatchCFormat {
                format: "%9llu",
                start: 11,
                end: 16,
            })
        );
        assert!(cf.next().is_none());
    }
}
