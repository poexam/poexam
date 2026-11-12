// SPDX-FileCopyrightText: 2001-2026 Free Software Foundation, Inc.
// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Line breaking of strings, port of GNU libunistring's `unilbrk` module
//! (functions `u8_possible_linebreaks` and `u8_width_linebreaks`), used by
//! GNU gettext to wrap the messages, so that poexam wraps them exactly like
//! `msgcat`.
//!
//! It implements [Unicode Standard Annex #14](https://www.unicode.org/reports/tr14/)
//! with the properties and pair table of libunistring (see
//! [`linebreak_table`](crate::po::linebreak_table)).

use crate::po::linebreak_table::{
    ASCII_PROPS, D, HANGUL_FIRST, HANGUL_LAST, I, LBP_AI, LBP_AK, LBP_AKLS_VI, LBP_AL1, LBP_AL2,
    LBP_AS, LBP_BK, LBP_CB, LBP_CL, LBP_CM, LBP_CP1, LBP_CP2, LBP_CR, LBP_EX, LBP_GL, LBP_H2,
    LBP_H3, LBP_HH, LBP_HL, LBP_HL_HY, LBP_HY, LBP_ID, LBP_IS, LBP_LF, LBP_NU, LBP_OP1, LBP_OP2,
    LBP_PO, LBP_PR, LBP_QU1, LBP_QU2, LBP_QU3, LBP_RI, LBP_SA1, LBP_SA2, LBP_SP, LBP_SY, LBP_VF,
    LBP_VI, LBP_WJ, LBP_XX, LBP_ZW, LBP_ZWJ, P, PAIR_TABLE, RANGE_PROPS, RANGE_STARTS,
    WIDTH_STARTS, WIDTH_VALUES,
};

/// Line break at a position (before the character starting at this byte).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Break {
    /// No override (used only in overrides).
    Undefined,
    /// No line break allowed.
    Prohibited,
    /// Line break allowed (or inserted, for [`insert_linebreaks`]).
    Possible,
    /// Mandatory line break (after a line break character).
    Mandatory,
}

/// Return the property of a character: `(class << 1) | east_asian`.
fn lookup(uc: char) -> u8 {
    let cp = u32::from(uc);
    if let Some(&prop_ea) = ASCII_PROPS.get(cp as usize) {
        return prop_ea;
    }
    if (HANGUL_FIRST..=HANGUL_LAST).contains(&cp) {
        let class = if (cp - HANGUL_FIRST).is_multiple_of(28) {
            LBP_H2
        } else {
            LBP_H3
        };
        return (class << 1) | 1;
    }
    let index = RANGE_STARTS.partition_point(|&start| start <= cp);
    index
        .checked_sub(1)
        .map_or(LBP_XX << 1, |index| RANGE_PROPS[index])
}

/// Return the width of a character (number of columns: 0, 1 or 2), like the
/// function `uc_width` of GNU libunistring for UTF-8; control characters
/// have a width of 0.
pub fn char_width(uc: char) -> usize {
    let cp = u32::from(uc);
    if cp < 0x80 {
        // ASCII: control characters (C0 and DEL) have a width of 0.
        return usize::from(cp >= 0x20 && cp != 0x7F);
    }
    let index = WIDTH_STARTS.partition_point(|&start| start <= cp);
    index
        .checked_sub(1)
        .map_or(1, |index| usize::from(WIDTH_VALUES[index]))
}

/// Return the line breaking class of a property.
const fn prop(prop_ea: u8) -> u8 {
    prop_ea >> 1
}

/// Return `true` if the property has the East Asian bit.
const fn ea(prop_ea: u8) -> bool {
    prop_ea & 1 != 0
}

/// Resolve the line breaking classes whose behaviour is not fixed.
const fn resolve(prop: u8) -> u8 {
    match prop {
        // This is arbitrary.
        LBP_CB => LBP_ID,
        // Resolve ambiguous (not a CJK encoding). We don't handle complex
        // scripts yet: treat LBP_SA1 like LBP_XX, which is arbitrary.
        LBP_AI | LBP_SA1 | LBP_XX => LBP_AL1,
        _ => prop,
    }
}

/// Return `true` if the class is a quotation mark (ambiguous, initial or
/// final punctuation).
const fn is_quote(prop: u8) -> bool {
    matches!(prop, LBP_QU1 | LBP_QU2 | LBP_QU3)
}

/// State of the line breaking algorithm, updated for each character.
struct State {
    /// Line break property of preceding character.
    preceding_prop: u8,
    /// Line break property of previous character (= last character, ignoring
    /// intervening characters of class CM or ZWJ).
    prev_prop: u8,
    /// East Asian property of the previous character (= last character,
    /// ignoring intervening characters of class CM or ZWJ), and of the
    /// character before the previous character.
    prev_ea: [bool; 2],
    /// The previous character was a word-initial hyphen or unambiguous hyphen.
    prev_initial_hyphen: bool,
    /// Before the previous character, there was a character with line break
    /// property `LBP_NU` and since then only characters with line break property
    /// `LBP_SY` or `LBP_IS`.
    prev_nus: bool,
    /// Line break property of last non-space character (= last character,
    /// ignoring intervening characters of class SP or CM or ZWJ).
    last_prop: u8,
    /// Was a space seen after the last non-space character?
    seen_space: bool,
    /// Number of consecutive regional indicator (RI) characters seen
    /// immediately before the current point.
    ri_count: usize,
}

/// Character being processed, with its lookahead properties.
struct Char {
    uc: char,
    /// Line breaking class (resolved).
    prop: u8,
    /// East Asian property.
    ea: bool,
    /// Line breaking class of the next character.
    next_prop: u8,
    /// East Asian property of the next character.
    next_ea: bool,
    /// Line breaking class of the character after the next one.
    next2_prop: u8,
    /// Ending at the previous character, there was a character with line
    /// break property `LBP_NU` and since then only characters with line break
    /// property `LBP_SY` or `LBP_IS`.
    nus: bool,
}

impl State {
    const fn new() -> Self {
        Self {
            preceding_prop: LBP_BK,
            prev_prop: LBP_BK,
            prev_ea: [false; 2],
            prev_initial_hyphen: false,
            prev_nus: false,
            last_prop: LBP_BK,
            seen_space: false,
            ri_count: 0,
        }
    }

    /// Return the line break before a character with property `prop_ea`, and
    /// update the state.
    /// `lookahead` gives the properties of the next two characters.
    fn next(&mut self, uc: char, prop_ea: u8, lookahead: (u8, u8)) -> Break {
        let nus = self.prev_prop == LBP_NU
            || (self.prev_nus && (self.prev_prop == LBP_SY || self.prev_prop == LBP_IS));
        let mut prop = prop(prop_ea);
        let brk = if prop == LBP_BK || prop == LBP_LF || prop == LBP_CR {
            // (LB4,LB5,LB6) Mandatory break.
            self.last_prop = LBP_BK;
            self.seen_space = false;
            Break::Mandatory
        } else {
            prop = resolve(prop);
            if prop == LBP_SP {
                // (LB7) Don't break just before a space.
                self.seen_space = true;
                Break::Prohibited
            } else if prop == LBP_ZW {
                // (LB7) Don't break just before a zero-width space.
                self.last_prop = LBP_ZW;
                self.seen_space = false;
                Break::Prohibited
            } else if prop == LBP_CM || prop == LBP_SA2 || prop == LBP_ZWJ {
                self.combining_break()
            } else {
                let c = Char {
                    uc,
                    prop,
                    ea: ea(prop_ea),
                    next_prop: self::prop(lookahead.0),
                    next_ea: ea(lookahead.0),
                    next2_prop: self::prop(lookahead.1),
                    nus,
                };
                let brk = self.char_break(&c);
                // For (LB15a): Replace LBP_QU2 with LBP_QU1 if the previous
                // character's line break property was not one of BK, CR, LF,
                // OP, QU, GL, SP, ZW.
                if prop == LBP_QU2
                    && !matches!(
                        self.prev_prop,
                        LBP_BK
                            | LBP_CR
                            | LBP_LF
                            | LBP_OP1
                            | LBP_OP2
                            | LBP_QU1
                            | LBP_QU2
                            | LBP_QU3
                            | LBP_GL
                            | LBP_SP
                            | LBP_ZW
                    )
                {
                    prop = LBP_QU1;
                }
                self.last_prop = prop;
                self.seen_space = false;
                brk
            }
        };
        self.update(prop, ea(prop_ea), nus);
        brk
    }

    /// Return the line break before a combining character or zero-width
    /// joiner.
    fn combining_break(&mut self) -> Break {
        // (LB9) Don't break just before a combining character or zero-width
        // joiner, except immediately after a mandatory break character, space,
        // or zero-width space.
        if self.last_prop == LBP_BK {
            // (LB4,LB5,LB6) Don't break at the beginning of a line.
            // (LB10) Treat CM or ZWJ as AL.
            self.last_prop = LBP_AL1;
            self.seen_space = false;
            Break::Prohibited
        } else if self.last_prop == LBP_ZW
            || (self.seen_space
                // (LB14) has higher priority than (LB18).
                && !(self.last_prop == LBP_OP1 || self.last_prop == LBP_OP2)
                // (LB15a) has higher priority than (LB18).
                && self.last_prop != LBP_QU2)
        {
            // (LB8) Break after zero-width space.
            // (LB18) Break after spaces.
            // (LB10) Treat CM or ZWJ as AL.
            self.last_prop = LBP_AL1;
            self.seen_space = false;
            Break::Possible
        } else {
            // Treat X CM as if it were X.
            Break::Prohibited
        }
    }

    /// Return the line break before a character which is not a mandatory
    /// break, a space or a combining character.
    fn char_break(&self, c: &Char) -> Break {
        if self.last_prop == LBP_BK {
            // (LB4,LB5,LB6) Don't break at the beginning of a line.
            Break::Prohibited
        } else if self.last_prop == LBP_ZW {
            // (LB8) Break after zero-width space.
            Break::Possible
        } else if self.preceding_prop == LBP_ZWJ {
            // (LB8a) Don't break right after a zero-width joiner.
            Break::Prohibited
        } else if c.prop == LBP_IS && self.prev_prop == LBP_SP && c.next_prop == LBP_NU {
            // (LB15c) Break before a decimal mark that follows a space.
            Break::Possible
        } else if (is_quote(c.prop)
            && (!self.prev_ea[0] || !c.next_ea)
            // (LB18) has higher priority than (LB19a).
            && self.prev_prop != LBP_SP)
            || (is_quote(self.prev_prop) && (!self.prev_ea[1] || !c.ea))
        {
            // (LB19a) Don't break on either side of ambiguous quotation marks,
            // except next to an EastAsian character.
            Break::Prohibited
        } else if self.prev_initial_hyphen && matches!(c.prop, LBP_AL1 | LBP_AL2 | LBP_HL) {
            // (LB20a) Don't break after a word-initial hyphen.
            Break::Prohibited
        } else if self.prev_prop == LBP_HL_HY && c.prop != LBP_HL {
            // (LB21a) Don't break after Hebrew + Hyphen/Unambiguous hyphen,
            // before non-Hebrew.
            Break::Prohibited
        } else if (self.prev_nus
            && matches!(self.prev_prop, LBP_CL | LBP_CP1 | LBP_CP2)
            && matches!(c.prop, LBP_PO | LBP_PR))
            || (c.nus && matches!(c.prop, LBP_PO | LBP_PR | LBP_NU))
            || (matches!(self.prev_prop, LBP_PO | LBP_PR)
                && matches!(c.prop, LBP_OP1 | LBP_OP2)
                && (c.next_prop == LBP_NU || (c.next_prop == LBP_IS && c.next2_prop == LBP_NU)))
        {
            // (LB25) Don't break numbers.
            Break::Prohibited
        } else if (self.prev_prop == LBP_AKLS_VI && matches!(c.prop, LBP_AK | LBP_AL2))
            || (c.next_prop == LBP_VF
                && matches!(c.prop, LBP_AK | LBP_AL2 | LBP_AS)
                && matches!(self.prev_prop, LBP_AK | LBP_AL2 | LBP_AS))
        {
            // (LB28a) Don't break inside orthographic syllables of Brahmic
            // scripts, lines 3 and 4.
            Break::Prohibited
        } else if self.last_prop == LBP_IS && c.uc == '<' {
            // Partially disable (LB29) Do not break between numeric punctuation
            // and alphabetics ("e.g."). We find it desirable to break before
            // the HTML tag "</P>" in strings like "<P>Some sentence.</P>".
            Break::Possible
        } else if self.last_prop == LBP_RI && c.prop == LBP_RI {
            // (LB30a) Break between two regional indicator symbols if and only
            // if there are an even number of regional indicators preceding the
            // position of the break.
            if self.seen_space || self.ri_count.is_multiple_of(2) {
                Break::Possible
            } else {
                Break::Prohibited
            }
        } else {
            self.pair_break(c)
        }
    }

    /// Return the line break before a character, from the pair table.
    fn pair_break(&self, c: &Char) -> Break {
        let mut this_prop = c.prop;
        // For (LB15b): Replace LBP_QU3 with LBP_QU1 if the next character's
        // line break property is not one of BK, CR, LF, SP, GL, WJ, CL, QU,
        // CP, EX, IS, SY, ZW.
        if c.prop == LBP_QU3
            && !matches!(
                c.next_prop,
                LBP_BK
                    | LBP_CR
                    | LBP_LF
                    | LBP_SP
                    | LBP_GL
                    | LBP_WJ
                    | LBP_CL
                    | LBP_QU1
                    | LBP_QU2
                    | LBP_QU3
                    | LBP_CP1
                    | LBP_CP2
                    | LBP_EX
                    | LBP_IS
                    | LBP_SY
                    | LBP_ZW
            )
        {
            this_prop = LBP_QU1;
        }
        match PAIR_TABLE[usize::from(self.last_prop)][usize::from(this_prop)] {
            D => Break::Possible,
            I if self.seen_space => Break::Possible,
            I | P => Break::Prohibited,
            _ => unreachable!("invalid value in line break pair table"),
        }
    }

    /// Update the state after a character.
    fn update(&mut self, prop: u8, char_ea: bool, nus: bool) {
        // (LB9) Treat X (CM | ZWJ)* as if it were X, where X is any line break
        // class except BK, CR, LF, NL, SP, or ZW.
        if !((prop == LBP_CM || prop == LBP_ZWJ)
            && !matches!(self.prev_prop, LBP_BK | LBP_LF | LBP_CR | LBP_SP | LBP_ZW))
        {
            self.prev_initial_hyphen = (prop == LBP_HY || prop == LBP_HH)
                && matches!(
                    self.prev_prop,
                    LBP_BK | LBP_CR | LBP_LF | LBP_SP | LBP_ZW | LBP_CB | LBP_GL
                );
            self.prev_prop =
                if prop == LBP_VI && matches!(self.prev_prop, LBP_AK | LBP_AL2 | LBP_AS) {
                    LBP_AKLS_VI
                } else if self.prev_prop == LBP_HL && (prop == LBP_HY || prop == LBP_HH) {
                    LBP_HL_HY
                } else {
                    prop
                };
            self.prev_ea = [char_ea, self.prev_ea[0]];
            self.prev_nus = nus;
        }
        self.preceding_prop = prop;
        if prop == LBP_RI {
            self.ri_count += 1;
        } else {
            self.ri_count = 0;
        }
    }
}

/// Return the possible line breaks of the string, for each byte (before the
/// character starting at this byte; the other bytes of a multibyte character
/// are [`Break::Prohibited`]).
pub fn possible_linebreaks(s: &str) -> Vec<Break> {
    // Don't break inside multibyte characters.
    let mut p = vec![Break::Prohibited; s.len()];
    let chars: Vec<(usize, char, u8)> = s
        .char_indices()
        .map(|(pos, uc)| (pos, uc, lookup(uc)))
        .collect();
    // We need 2 characters of lookahead: 1 character of lookahead for
    // (LB15c,LB19a,LB28a), 2 characters of lookahead for (LB25).
    let lookahead = |i: usize| chars.get(i).map_or(LBP_BK << 1, |c| c.2);
    let mut state = State::new();
    for (i, &(pos, uc, prop_ea)) in chars.iter().enumerate() {
        p[pos] = state.next(uc, prop_ea, (lookahead(i + 1), lookahead(i + 2)));
    }
    p
}

/// Replace the possible line breaks with the overrides which are not
/// [`Break::Undefined`].
pub fn apply_overrides(possible: &mut [Break], overrides: &[Break]) {
    for (brk, &o) in possible.iter_mut().zip(overrides) {
        if o != Break::Undefined {
            *brk = o;
        }
    }
}

/// Return the line breaks to insert in the string so that each line fits in
/// `width` columns, the first line starting at column `start_column`: `true`
/// for each byte where a line break is inserted (before the character
/// starting at this byte).
///
/// `possible` gives the possible line breaks of the string (see
/// [`possible_linebreaks`] and [`apply_overrides`]).
///
/// A text without break opportunity wider than `width` is not split, so
/// lines may be longer than `width`.
pub fn insert_linebreaks(
    s: &str,
    possible: &[Break],
    width: isize,
    start_column: isize,
) -> Vec<bool> {
    let mut breaks = vec![false; s.len()];
    let mut last_p: Option<usize> = None;
    let mut last_column = start_column;
    let mut piece_width: isize = 0;
    for (pos, uc) in s.char_indices() {
        let brk = possible[pos];
        if matches!(brk, Break::Possible | Break::Mandatory) {
            // An atomic piece of text ends here.
            if let Some(last) = last_p
                && last_column + piece_width > width
            {
                // Insert a line break.
                breaks[last] = true;
                last_column = 0;
            }
        }
        if brk == Break::Mandatory {
            // uc is a line break character: start a new piece at column 0.
            last_p = None;
            last_column = 0;
            piece_width = 0;
        } else {
            if brk == Break::Possible {
                // Start a new piece.
                last_p = Some(pos);
                last_column += piece_width;
                piece_width = 0;
            }
            // Control characters in the string have a width of 0.
            piece_width += isize::try_from(char_width(uc)).unwrap_or(0);
        }
    }
    // The last atomic piece of text ends here.
    if let Some(last) = last_p
        && last_column + piece_width > width
    {
        // Insert a line break.
        breaks[last] = true;
    }
    breaks
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Return the string with `|` at each possible line break.
    fn show_breaks(s: &str) -> String {
        let breaks = possible_linebreaks(s);
        let mut out = String::new();
        for (pos, c) in s.char_indices() {
            if breaks[pos] == Break::Possible {
                out.push('|');
            }
            out.push(c);
        }
        out
    }

    #[test]
    fn test_lookup() {
        assert_eq!(prop(lookup('a')), LBP_AL1);
        assert_eq!(prop(lookup(' ')), LBP_SP);
        assert_eq!(prop(lookup('(')), LBP_OP1);
        assert_eq!(prop(lookup('"')), LBP_QU1);
        assert_eq!(prop(lookup('«')), LBP_QU2);
        assert_eq!(prop(lookup('»')), LBP_QU3);
        assert_eq!(prop(lookup('가')), LBP_H2);
        assert_eq!(prop(lookup('각')), LBP_H3);
        assert!(ea(lookup('가')));
        assert_eq!(prop(lookup('中')), LBP_ID);
        assert_eq!(prop(lookup('\u{10_FFFF}')), LBP_XX);
    }

    #[test]
    fn test_char_width() {
        assert_eq!(char_width('a'), 1);
        assert_eq!(char_width('\t'), 0);
        assert_eq!(char_width('\u{0301}'), 0);
        assert_eq!(char_width('中'), 2);
        assert_eq!(char_width('가'), 2);
        // Spacing vowel sign (Malayalam): width 1 in libunistring.
        assert_eq!(char_width('\u{0D3E}'), 1);
    }

    #[test]
    fn test_possible_linebreaks() {
        assert_eq!(show_breaks(""), "");
        assert_eq!(show_breaks("hello world"), "hello |world");
        assert_eq!(show_breaks("a-b c"), "a-|b |c");
        // Breaks after spaces, also before an opening parenthesis after a quote.
        assert_eq!(show_breaks("\"max\" (inclusive)"), "\"max\" |(inclusive)");
        // No break after a word-initial hyphen (LB20a).
        assert_eq!(show_breaks("x -set y"), "x |-set |y");
        // Break before an HTML tag after a full stop (gnulib).
        assert_eq!(show_breaks("weechat.bar.<name>"), "weechat.bar.|<name>");
        // Break between "$" and "{".
        assert_eq!(show_breaks("a ${color}"), "a |$|{color}");
        // No break in numbers.
        assert_eq!(show_breaks("1,000.50 $"), "1,000.50 |$");
        // Ideographs.
        assert_eq!(show_breaks("中文字"), "中|文|字");
    }

    /// Return the line breaks to insert, with the given overrides.
    fn width_linebreaks(s: &str, width: isize, start: isize, overrides: &[Break]) -> Vec<bool> {
        let mut possible = possible_linebreaks(s);
        apply_overrides(&mut possible, overrides);
        insert_linebreaks(s, &possible, width, start)
    }

    #[test]
    fn test_width_linebreaks() {
        let s = "aaa bbb ccc ddd";
        let breaks = width_linebreaks(s, 8, 0, &[]);
        let positions: Vec<usize> = (0..s.len()).filter(|&i| breaks[i]).collect();
        assert_eq!(positions, [8]);
        // Too long word: not split.
        let breaks = width_linebreaks("aaaaaaaaaaaa b", 5, 0, &[]);
        assert_eq!(breaks.iter().filter(|b| **b).count(), 1);
        // Override: prohibit the break.
        let mut overrides = vec![Break::Undefined; s.len()];
        overrides[8] = Break::Prohibited;
        let breaks = width_linebreaks(s, 8, 0, &overrides);
        let positions: Vec<usize> = (0..s.len()).filter(|&i| breaks[i]).collect();
        assert_eq!(positions, [4, 12]);
    }
}
