// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format string directives as recognized by the format string parsers of GNU
//! gettext (`format-c-parse.h` and `format-python.c`), used to wrap messages
//! like GNU `msgcat`.
//!
//! The parsers of gettext stop at the first invalid directive: `msgcat`
//! prohibits line breaks only inside the directives found before it. The
//! invalid directive and the ones after it are not returned.

use std::ops::Range;

/// Return the byte at `pos` in `s`, or `0` at the end of the string (like a C
/// string).
fn at(s: &[u8], pos: usize) -> u8 {
    s.get(pos).copied().unwrap_or(0)
}

/// Return the position after the ASCII digits starting at `pos`.
fn skip_digits(s: &[u8], mut pos: usize) -> usize {
    while at(s, pos).is_ascii_digit() {
        pos += 1;
    }
    pos
}

/// Parse an argument number (`N$`) at `pos`: return `Ok(Some(pos))` with the
/// position after the `$` if there is a number, `Ok(None)` if there is none
/// and `Err(())` if the number is 0 (invalid).
fn arg_number(s: &[u8], pos: usize) -> Result<Option<usize>, ()> {
    let end = skip_digits(s, pos);
    if end == pos || at(s, end) != b'$' {
        return Ok(None);
    }
    if s[pos..end].iter().all(|&b| b == b'0') {
        return Err(());
    }
    Ok(Some(end + 1))
}

/// Kinds of arguments used by the directives of a C format string: numbered
/// (e.g. `%1$d`) and unnumbered (e.g. `%d`) arguments are exclusive.
#[derive(Default)]
struct CArgs {
    numbered: bool,
    unnumbered: bool,
}

impl CArgs {
    /// Add an argument, return `false` if numbered and unnumbered arguments
    /// are mixed.
    fn add(&mut self, numbered: bool) -> bool {
        if numbered {
            self.numbered = true;
            !self.unnumbered
        } else {
            self.unnumbered = true;
            !self.numbered
        }
    }
}

/// Parse a width or precision given as argument (`*` or `*N$`), with `pos`
/// after the `*`: return the position after it, or `None` if it is invalid.
fn c_star_arg(s: &[u8], pos: usize, args: &mut CArgs) -> Option<usize> {
    let (pos, numbered) = match arg_number(s, pos).ok()? {
        Some(end) => (end, true),
        None => (pos, false),
    };
    args.add(numbered).then_some(pos)
}

/// Parse an ISO C 99 format macro (e.g. `<PRId64>`), with `pos` on the `<`:
/// return the position of the `>`, or `None` if it is invalid.
fn c_macro(s: &[u8], pos: usize) -> Option<usize> {
    let rest = s.get(pos + 1..)?.strip_prefix(b"PRI")?;
    let (conversion, rest) = rest.split_first()?;
    if !matches!(conversion, b'd' | b'i' | b'u' | b'o' | b'x' | b'X') {
        return None;
    }
    let rest = if let Some(r) = rest
        .strip_prefix(b"MAX")
        .or_else(|| rest.strip_prefix(b"PTR"))
    {
        r
    } else {
        let r = rest
            .strip_prefix(b"LEAST")
            .or_else(|| rest.strip_prefix(b"FAST"))
            .unwrap_or(rest);
        [&b"8"[..], b"16", b"32", b"64"]
            .iter()
            .find_map(|n| r.strip_prefix(*n))?
    };
    (rest.first() == Some(&b'>')).then(|| s.len() - rest.len())
}

/// Integer size of a C directive (e.g. `l` in `%ld`).
#[derive(PartialEq)]
enum CIntSize {
    None,
    Long,
    Other,
}

/// Parse the size and the conversion of a C directive at `pos`: return the
/// position of the conversion char and whether the directive takes an
/// argument, or `None` if it is invalid.
fn c_conversion(s: &[u8], mut pos: usize) -> Option<(usize, bool)> {
    let mut int_size = CIntSize::None;
    let mut float_size = false;
    match at(s, pos) {
        b'h' => {
            int_size = CIntSize::Other;
            pos += if at(s, pos + 1) == b'h' { 2 } else { 1 };
        }
        b'l' => {
            if at(s, pos + 1) == b'l' {
                int_size = CIntSize::Other;
                float_size = true;
                pos += 2;
            } else {
                int_size = CIntSize::Long;
                pos += 1;
            }
        }
        b'L' | b'q' => {
            int_size = CIntSize::Other;
            float_size = true;
            pos += 1;
        }
        b'j' | b'z' | b'Z' | b't' => {
            int_size = CIntSize::Other;
            pos += 1;
        }
        b'w' => {
            // ISO C 23: wN and wfN.
            let start = if at(s, pos + 1) == b'f' {
                pos + 2
            } else {
                pos + 1
            };
            let len = match (at(s, start), at(s, start + 1)) {
                (b'8', _) => 1,
                (b'1', b'6') | (b'3', b'2') | (b'6', b'4') => 2,
                _ => 0,
            };
            if len > 0 {
                int_size = CIntSize::Other;
                pos = start + len;
            }
        }
        _ => {}
    }
    let has_arg = match at(s, pos) {
        b'%' | b'm' => false,
        b'c' | b's' if int_size != CIntSize::None && int_size != CIntSize::Long => return None,
        b'e' | b'E' | b'f' | b'F' | b'g' | b'G' | b'a' | b'A'
            if int_size != CIntSize::None && !float_size =>
        {
            return None;
        }
        b'c' | b's' | b'C' | b'S' | b'i' | b'd' | b'u' | b'o' | b'x' | b'X' | b'b' | b'e'
        | b'E' | b'f' | b'F' | b'g' | b'G' | b'a' | b'A' | b'p' | b'n' => true,
        _ => return None,
    };
    Some((pos, has_arg))
}

/// Return the byte ranges of the directives of a C format string (from the
/// `%` to the conversion char, inclusive) found before the first invalid
/// directive, like the function `format_parse_entrails` of GNU gettext
/// (`format-c-parse.h`).
///
/// The flag `I` is allowed only in a translation (`translated` = `true`).
pub fn c_directives(s: &str, translated: bool) -> Vec<Range<usize>> {
    let s = s.as_bytes();
    let mut ranges = vec![];
    let mut args = CArgs::default();
    let mut offset = 0;
    while let Some(start) = memchr::memchr(b'%', &s[offset..]).map(|p| offset + p) {
        let Ok(number) = arg_number(s, start + 1) else {
            return ranges;
        };
        let mut pos = number.unwrap_or(start + 1);
        // Flags.
        while matches!(at(s, pos), b' ' | b'+' | b'-' | b'#' | b'0' | b'\'')
            || (translated && at(s, pos) == b'I')
        {
            pos += 1;
        }
        // Width.
        if at(s, pos) == b'*' {
            let Some(end) = c_star_arg(s, pos + 1, &mut args) else {
                return ranges;
            };
            pos = end;
        } else {
            pos = skip_digits(s, pos);
        }
        // Precision.
        if at(s, pos) == b'.' {
            pos += 1;
            if at(s, pos) == b'*' {
                let Some(end) = c_star_arg(s, pos + 1, &mut args) else {
                    return ranges;
                };
                pos = end;
            } else {
                pos = skip_digits(s, pos);
            }
        }
        let conversion = if at(s, pos) == b'<' {
            c_macro(s, pos).map(|end| (end, true))
        } else {
            c_conversion(s, pos)
        };
        let Some((end, has_arg)) = conversion else {
            return ranges;
        };
        if has_arg && !args.add(number.is_some()) {
            return ranges;
        }
        ranges.push(start..end + 1);
        offset = end + 1;
    }
    ranges
}

/// Return the byte ranges of the directives of a Python format string (from
/// the `%` to the conversion char, inclusive) found before the first invalid
/// directive, like the function `format_parse` of GNU gettext
/// (`format-python.c`).
pub fn python_directives(s: &str) -> Vec<Range<usize>> {
    let s = s.as_bytes();
    let mut ranges = vec![];
    // Named (e.g. `%(count)d`) and unnamed (e.g. `%d`) arguments are
    // exclusive.
    let mut named_args = false;
    let mut unnamed_args = false;
    let mut offset = 0;
    while let Some(start) = memchr::memchr(b'%', &s[offset..]).map(|p| offset + p) {
        let mut pos = start + 1;
        // Name, with balanced parentheses.
        let named = at(s, pos) == b'(';
        if named {
            pos += 1;
            let mut depth = 0_usize;
            loop {
                match at(s, pos) {
                    0 => return ranges,
                    b'(' => depth += 1,
                    b')' if depth == 0 => break,
                    b')' => depth -= 1,
                    _ => {}
                }
                pos += 1;
            }
            pos += 1;
        }
        // Flags.
        while matches!(at(s, pos), b'-' | b'+' | b' ' | b'#' | b'0') {
            pos += 1;
        }
        // Width and precision.
        for i in 0..2 {
            if i == 1 {
                if at(s, pos) != b'.' {
                    break;
                }
                pos += 1;
            }
            if at(s, pos) == b'*' {
                if named_args {
                    return ranges;
                }
                unnamed_args = true;
                pos += 1;
            } else {
                pos = skip_digits(s, pos);
            }
        }
        // Size.
        if matches!(at(s, pos), b'h' | b'l' | b'L') {
            pos += 1;
        }
        let conversion = at(s, pos);
        if !matches!(
            conversion,
            b'%' | b'c'
                | b's'
                | b'r'
                | b'i'
                | b'd'
                | b'u'
                | b'o'
                | b'x'
                | b'X'
                | b'e'
                | b'E'
                | b'f'
                | b'g'
                | b'G'
        ) {
            return ranges;
        }
        if named {
            if unnamed_args {
                return ranges;
            }
            named_args = true;
        } else if conversion != b'%' {
            if named_args {
                return ranges;
            }
            unnamed_args = true;
        }
        ranges.push(start..pos + 1);
        offset = pos + 1;
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(s: &str) -> Vec<&str> {
        c_directives(s, false).into_iter().map(|r| &s[r]).collect()
    }

    fn python(s: &str) -> Vec<&str> {
        python_directives(s).into_iter().map(|r| &s[r]).collect()
    }

    #[test]
    fn test_c_valid() {
        assert!(c("").is_empty());
        assert!(c("no format").is_empty());
        assert_eq!(
            c("%d %s %% %5.2f %-*.*s %ld %lld %hhx %zu %ls %Lf %m %wf32d %w64u %#o"),
            vec![
                "%d", "%s", "%%", "%5.2f", "%-*.*s", "%ld", "%lld", "%hhx", "%zu", "%ls", "%Lf",
                "%m", "%wf32d", "%w64u", "%#o"
            ]
        );
        assert_eq!(
            c("%2$s %1$d %3$*2$.*1$d %%"),
            vec!["%2$s", "%1$d", "%3$*2$.*1$d", "%%"]
        );
        assert_eq!(
            c("%<PRId64> %<PRIuMAX> %<PRIxLEAST16> %<PRIoFAST8> %5%"),
            vec![
                "%<PRId64>",
                "%<PRIuMAX>",
                "%<PRIxLEAST16>",
                "%<PRIoFAST8>",
                "%5%"
            ]
        );
        // A space is a flag.
        assert_eq!(c("100% done"), vec!["% d"]);
        // The flag "I" is allowed only in a translation.
        assert!(c("%Id").is_empty());
        assert_eq!(
            c_directives("%Id", true)
                .into_iter()
                .map(|r| &"%Id"[r])
                .collect::<Vec<_>>(),
            vec!["%Id"]
        );
    }

    #[test]
    fn test_c_invalid() {
        // Parsing stops at the first invalid directive.
        assert_eq!(c("%d 100% … %s"), vec!["%d"]);
        assert_eq!(c("%s %"), vec!["%s"]);
        assert_eq!(c("%s %0$d %d"), vec!["%s"]);
        assert_eq!(c("%1$s %d %s"), vec!["%1$s"]);
        assert_eq!(c("%d %2$s"), vec!["%d"]);
        assert_eq!(c("%1$s %*d"), vec!["%1$s"]);
        assert_eq!(c("%hs %d"), Vec::<&str>::new());
        assert_eq!(c("%lf"), Vec::<&str>::new());
        assert_eq!(c("%w7d"), Vec::<&str>::new());
        assert_eq!(c("%<PRId63>"), Vec::<&str>::new());
        assert_eq!(c("%<PRId64"), Vec::<&str>::new());
        assert_eq!(c("%@"), Vec::<&str>::new());
        // "%%" takes no argument: it can be mixed with numbered arguments.
        assert_eq!(c("%1$d%% %%"), vec!["%1$d", "%%", "%%"]);
    }

    #[test]
    fn test_python() {
        assert!(python("").is_empty());
        assert_eq!(
            python("%s %d %% %5.2f %-*.*s %r %ld"),
            vec!["%s", "%d", "%%", "%5.2f", "%-*.*s", "%r", "%ld"]
        );
        assert_eq!(
            python("%(n)d %(name)s %(a(b))s %(n)%"),
            vec!["%(n)d", "%(name)s", "%(a(b))s", "%(n)%"]
        );
        // Parsing stops at the first invalid directive.
        assert_eq!(python("%d 100% … %s"), vec!["%d"]);
        assert_eq!(python("%(n)d %d"), vec!["%(n)d"]);
        assert_eq!(python("%d %(n)d"), vec!["%d"]);
        assert_eq!(python("%(n)d %*d"), vec!["%(n)d"]);
        assert_eq!(python("%(n)*d"), Vec::<&str>::new());
        assert_eq!(python("%(n"), Vec::<&str>::new());
        assert_eq!(python("%F"), Vec::<&str>::new());
        // "%%" takes no argument: it can be mixed with named arguments.
        assert_eq!(python("%(n)d %%"), vec!["%(n)d", "%%"]);
    }
}
