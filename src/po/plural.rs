// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Parser and evaluator of the plural expression of the `Plural-Forms` header,
//! e.g. `nplurals=2; plural=(n > 1);`.
//!
//! The syntax is the C-like one of GNU gettext: the variable `n`, unsigned
//! integers, parentheses, and the operators `!`, `*`, `/`, `%`, `+`, `-`, `<`,
//! `>`, `<=`, `>=`, `==`, `!=`, `&&`, `||` and `?:`, with the C precedence.

/// Maximum length of a plural expression: real ones are below 200 bytes, and
/// the limit bounds the depth of the expression tree (recursive evaluation).
const MAX_EXPR_LEN: usize = 1024;

/// Maximum nesting depth of parentheses, `!` and `?:` while parsing.
const MAX_DEPTH: usize = 64;

/// Binary arithmetic or comparison operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Mul,
    Div,
    Mod,
    Add,
    Sub,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
}

/// Plural expression, evaluated for a number `n` to the index of the plural form.
#[derive(Debug, PartialEq, Eq)]
pub enum PluralExpr {
    N,
    Num(u64),
    Not(Box<PluralExpr>),
    Binary(Op, Box<PluralExpr>, Box<PluralExpr>),
    And(Box<PluralExpr>, Box<PluralExpr>),
    Or(Box<PluralExpr>, Box<PluralExpr>),
    Cond(Box<PluralExpr>, Box<PluralExpr>, Box<PluralExpr>),
}

impl PluralExpr {
    /// Parse a plural expression, return `None` if it is invalid.
    pub fn parse(s: &str) -> Option<Self> {
        if s.len() > MAX_EXPR_LEN {
            return None;
        }
        let mut parser = ExprParser {
            s: s.as_bytes(),
            pos: 0,
            depth: 0,
        };
        let expr = parser.ternary()?;
        parser.skip_ws();
        (parser.pos == parser.s.len()).then_some(expr)
    }

    /// Evaluate the expression for `n`, return `None` on a division by zero.
    ///
    /// Like gettext, arithmetic is done on unsigned integers (wrapping), and
    /// `&&`, `||` and `?:` evaluate only the operands they need.
    pub fn eval(&self, n: u64) -> Option<u64> {
        match self {
            Self::N => Some(n),
            Self::Num(value) => Some(*value),
            Self::Not(expr) => Some(u64::from(expr.eval(n)? == 0)),
            Self::And(left, right) => Some(u64::from(left.eval(n)? != 0 && right.eval(n)? != 0)),
            Self::Or(left, right) => Some(u64::from(left.eval(n)? != 0 || right.eval(n)? != 0)),
            Self::Cond(cond, if_true, if_false) => {
                if cond.eval(n)? == 0 {
                    if_false.eval(n)
                } else {
                    if_true.eval(n)
                }
            }
            Self::Binary(op, left, right) => {
                let (l, r) = (left.eval(n)?, right.eval(n)?);
                Some(match op {
                    Op::Mul => l.wrapping_mul(r),
                    Op::Div => l.checked_div(r)?,
                    Op::Mod => l.checked_rem(r)?,
                    Op::Add => l.wrapping_add(r),
                    Op::Sub => l.wrapping_sub(r),
                    Op::Lt => u64::from(l < r),
                    Op::Gt => u64::from(l > r),
                    Op::Le => u64::from(l <= r),
                    Op::Ge => u64::from(l >= r),
                    Op::Eq => u64::from(l == r),
                    Op::Ne => u64::from(l != r),
                })
            }
        }
    }
}

/// Recursive descent parser, one method per precedence level.
struct ExprParser<'a> {
    s: &'a [u8],
    pos: usize,
    depth: usize,
}

impl ExprParser<'_> {
    fn skip_ws(&mut self) {
        while self.pos < self.s.len() && self.s[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    /// Consume `token` (after optional whitespace) if it comes next.
    fn eat(&mut self, token: &str) -> bool {
        self.skip_ws();
        if self.s[self.pos..].starts_with(token.as_bytes()) {
            self.pos += token.len();
            true
        } else {
            false
        }
    }

    /// Enter a nested level, return `None` if the expression is too deep.
    fn enter(&mut self) -> Option<()> {
        self.depth += 1;
        (self.depth <= MAX_DEPTH).then_some(())
    }

    fn ternary(&mut self) -> Option<PluralExpr> {
        self.enter()?;
        let cond = self.or()?;
        let expr = if self.eat("?") {
            let if_true = self.ternary()?;
            if !self.eat(":") {
                return None;
            }
            let if_false = self.ternary()?;
            PluralExpr::Cond(Box::new(cond), Box::new(if_true), Box::new(if_false))
        } else {
            cond
        };
        self.depth -= 1;
        Some(expr)
    }

    fn or(&mut self) -> Option<PluralExpr> {
        let mut left = self.and()?;
        while self.eat("||") {
            left = PluralExpr::Or(Box::new(left), Box::new(self.and()?));
        }
        Some(left)
    }

    fn and(&mut self) -> Option<PluralExpr> {
        let mut left = self.binary(0)?;
        while self.eat("&&") {
            left = PluralExpr::And(Box::new(left), Box::new(self.binary(0)?));
        }
        Some(left)
    }

    /// Left-associative binary operators, by increasing precedence level
    /// (operators sharing a prefix are listed longest first).
    fn binary(&mut self, level: usize) -> Option<PluralExpr> {
        const LEVELS: [&[(&str, Op)]; 4] = [
            &[("==", Op::Eq), ("!=", Op::Ne)],
            &[("<=", Op::Le), (">=", Op::Ge), ("<", Op::Lt), (">", Op::Gt)],
            &[("+", Op::Add), ("-", Op::Sub)],
            &[("*", Op::Mul), ("/", Op::Div), ("%", Op::Mod)],
        ];
        let Some(ops) = LEVELS.get(level) else {
            return self.unary();
        };
        let mut left = self.binary(level + 1)?;
        while let Some(&(_, op)) = ops.iter().find(|(token, _)| self.eat(token)) {
            let right = self.binary(level + 1)?;
            left = PluralExpr::Binary(op, Box::new(left), Box::new(right));
        }
        Some(left)
    }

    fn unary(&mut self) -> Option<PluralExpr> {
        if self.eat("!") {
            self.enter()?;
            let expr = PluralExpr::Not(Box::new(self.unary()?));
            self.depth -= 1;
            return Some(expr);
        }
        self.primary()
    }

    fn primary(&mut self) -> Option<PluralExpr> {
        if self.eat("(") {
            let expr = self.ternary()?;
            return self.eat(")").then_some(expr);
        }
        if self.eat("n") {
            return Some(PluralExpr::N);
        }
        let start = self.pos;
        while self.pos < self.s.len() && self.s[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        std::str::from_utf8(&self.s[start..self.pos])
            .ok()?
            .parse()
            .ok()
            .map(PluralExpr::Num)
    }
}

/// Parse the value of the `Plural-Forms` header field, e.g.
/// `nplurals=2; plural=(n > 1);`, into the number of plural forms and the
/// plural expression (not parsed).
///
/// Return `None` if `nplurals` or `plural` is missing, or if `nplurals` is not
/// an integer.
pub fn parse_plural_forms(value: &str) -> Option<(u32, &str)> {
    let mut nplurals = None;
    let mut plural = None;
    for param in value.split(';') {
        let Some((key, val)) = param.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.eq_ignore_ascii_case("nplurals") {
            nplurals = val.trim().parse::<u32>().ok();
        } else if key.eq_ignore_ascii_case("plural") {
            plural = Some(val.trim());
        }
    }
    Some((nplurals?, plural?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(expr: &str, n: u64) -> Option<u64> {
        PluralExpr::parse(expr).unwrap().eval(n)
    }

    #[test]
    fn test_parse_plural_forms() {
        assert_eq!(
            parse_plural_forms("nplurals=2; plural=(n > 1);"),
            Some((2, "(n > 1)"))
        );
        assert_eq!(
            parse_plural_forms(" NPLURALS = 3 ; plural = n==1 ? 0 : 1 "),
            Some((3, "n==1 ? 0 : 1"))
        );
        assert_eq!(parse_plural_forms("nplurals=1; plural=0"), Some((1, "0")));
        assert_eq!(parse_plural_forms("nplurals=2;"), None);
        assert_eq!(parse_plural_forms("plural=(n > 1);"), None);
        assert_eq!(parse_plural_forms("nplurals=x; plural=0;"), None);
        assert_eq!(parse_plural_forms(""), None);
    }

    #[test]
    fn test_eval_simple() {
        assert_eq!(eval("0", 5), Some(0));
        assert_eq!(eval("n", 5), Some(5));
        assert_eq!(eval("n != 1", 0), Some(1));
        assert_eq!(eval("n != 1", 1), Some(0));
        assert_eq!(eval("(n > 1)", 0), Some(0));
        assert_eq!(eval("(n > 1)", 2), Some(1));
        assert_eq!(eval("!n", 0), Some(1));
        assert_eq!(eval("!!n", 7), Some(1));
        assert_eq!(eval("  ( ( n ) )  ", 3), Some(3));
    }

    #[test]
    fn test_eval_precedence() {
        assert_eq!(eval("1 + 2 * 3", 0), Some(7));
        assert_eq!(eval("(1 + 2) * 3", 0), Some(9));
        assert_eq!(eval("10 - 2 - 3", 0), Some(5));
        assert_eq!(eval("n % 10 == 1 && n % 100 != 11", 21), Some(1));
        assert_eq!(eval("n % 10 == 1 && n % 100 != 11", 11), Some(0));
        assert_eq!(eval("0 || 1 && 0", 0), Some(0));
        assert_eq!(eval("1 || 0 && 0", 0), Some(1));
        assert_eq!(eval("n <= 4 == 1", 3), Some(1));
        // Nested ternary operators are right-associative.
        let arabic =
            "n==0 ? 0 : n==1 ? 1 : n==2 ? 2 : n%100>=3 && n%100<=10 ? 3 : n%100>=11 ? 4 : 5";
        let values: Vec<u64> = [0, 1, 2, 3, 11, 100, 102]
            .iter()
            .map(|n| eval(arabic, *n).unwrap())
            .collect();
        assert_eq!(values, vec![0, 1, 2, 3, 4, 5, 5]);
    }

    #[test]
    fn test_eval_russian() {
        let russian =
            "(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2)";
        let values: Vec<u64> = [1, 2, 5, 11, 12, 21, 22, 25, 111, 112]
            .iter()
            .map(|n| eval(russian, *n).unwrap())
            .collect();
        assert_eq!(values, vec![0, 1, 2, 2, 2, 0, 1, 2, 2, 2]);
    }

    #[test]
    fn test_eval_division_by_zero() {
        assert_eq!(eval("1 / n", 0), None);
        assert_eq!(eval("1 % n", 0), None);
        assert_eq!(eval("1 / n", 1), Some(1));
        // Operands not needed are not evaluated.
        assert_eq!(eval("n != 0 && 10 / n", 0), Some(0));
        assert_eq!(eval("n == 0 || 10 / n", 0), Some(1));
        assert_eq!(eval("n == 0 ? 0 : 10 / n", 0), Some(0));
    }

    #[test]
    fn test_eval_wrapping() {
        assert_eq!(eval("n - 1", 0), Some(u64::MAX));
    }

    #[test]
    fn test_parse_invalid() {
        for expr in [
            "",
            "  ",
            "x",
            "n >",
            "(n",
            "n)",
            "n ? 1",
            "n ? 1 :",
            "n = 1",
            "n & 1",
            "n | 1",
            "-n",
            "n 1",
            "99999999999999999999999",
        ] {
            assert!(
                PluralExpr::parse(expr).is_none(),
                "{expr:?} should be invalid"
            );
        }
    }

    #[test]
    fn test_parse_limits() {
        assert!(PluralExpr::parse(&format!("{}n{}", "(".repeat(50), ")".repeat(50))).is_some());
        assert!(PluralExpr::parse(&format!("{}n{}", "(".repeat(100), ")".repeat(100))).is_none());
        assert!(PluralExpr::parse(&"!".repeat(100)).is_none());
        assert!(PluralExpr::parse(&"n+".repeat(600)).is_none());
    }
}
