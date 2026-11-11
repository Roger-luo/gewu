//! The arithmetic a cost model's series are written in. Numbers, declared
//! parameters, `+ - * / ^`, parentheses, and `log2 ceil floor min max
//! next_pow2`: enough to say "entries times bytes per entry", and nothing
//! that can loop, call out, or read anything else. `apps/desktop` evaluates
//! the same grammar in `formula.ts`.

use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Number(f64),
    /// Index into the parameter values.
    Param(usize),
    Neg(Box<Expr>),
    Binary(char, Box<Expr>, Box<Expr>),
    Call(Function, Vec<Expr>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Function {
    Log2,
    Ceil,
    Floor,
    Min,
    Max,
    NextPow2,
}

impl Function {
    fn named(name: &str) -> Option<(Self, usize)> {
        Some(match name {
            "log2" => (Self::Log2, 1),
            "ceil" => (Self::Ceil, 1),
            "floor" => (Self::Floor, 1),
            "min" => (Self::Min, 2),
            "max" => (Self::Max, 2),
            "next_pow2" => (Self::NextPow2, 1),
            _ => return None,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct FormulaError(String);

impl fmt::Display for FormulaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Parameter ids and names: a lowercase letter or `_`, then those or digits.
#[must_use]
pub fn is_name(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_lowercase() || first == '_')
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

impl Expr {
    /// Parses `text`, where `params` are the only names it may use.
    pub fn parse(text: &str, params: &[&str]) -> Result<Self, FormulaError> {
        let mut parser = Parser {
            chars: text.chars().filter(|c| !c.is_whitespace()).collect(),
            at: 0,
            params,
        };
        if parser.chars.is_empty() {
            return Err(FormulaError("the formula is empty".into()));
        }
        let expr = parser.sum()?;
        match parser.peek() {
            None => Ok(expr),
            Some(c) => Err(FormulaError(format!("unexpected `{c}`"))),
        }
    }

    #[must_use]
    pub fn eval(&self, values: &[f64]) -> f64 {
        match self {
            Self::Number(n) => *n,
            Self::Param(index) => values.get(*index).copied().unwrap_or(f64::NAN),
            Self::Neg(inner) => -inner.eval(values),
            Self::Binary(op, left, right) => {
                let (a, b) = (left.eval(values), right.eval(values));
                match op {
                    '+' => a + b,
                    '-' => a - b,
                    '*' => a * b,
                    '/' => a / b,
                    _ => a.powf(b),
                }
            }
            Self::Call(function, args) => {
                let a = args.first().map_or(f64::NAN, |arg| arg.eval(values));
                let b = args.get(1).map_or(f64::NAN, |arg| arg.eval(values));
                match function {
                    Function::Log2 => a.log2(),
                    Function::Ceil => a.ceil(),
                    Function::Floor => a.floor(),
                    Function::Min => a.min(b),
                    Function::Max => a.max(b),
                    Function::NextPow2 => 2f64.powf(a.log2().ceil().max(0.0)),
                }
            }
        }
    }
}

struct Parser<'a> {
    chars: Vec<char>,
    at: usize,
    params: &'a [&'a str],
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn eat(&mut self, c: char) -> bool {
        let found = self.peek() == Some(c);
        if found {
            self.at += 1;
        }
        found
    }

    fn sum(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.product()?;
        while let Some(op @ ('+' | '-')) = self.peek() {
            self.at += 1;
            left = Expr::Binary(op, Box::new(left), Box::new(self.product()?));
        }
        Ok(left)
    }

    fn product(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.unary()?;
        while let Some(op @ ('*' | '/')) = self.peek() {
            self.at += 1;
            left = Expr::Binary(op, Box::new(left), Box::new(self.unary()?));
        }
        Ok(left)
    }

    /// `-2^2` is `-(2^2)`: a power binds tighter than a minus sign.
    fn unary(&mut self) -> Result<Expr, FormulaError> {
        if self.eat('-') {
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        self.power()
    }

    /// Right-associative: `2^3^2` is `2^(3^2)`, and `2^-1` is a half.
    fn power(&mut self) -> Result<Expr, FormulaError> {
        let base = self.primary()?;
        if self.eat('^') {
            return Ok(Expr::Binary('^', Box::new(base), Box::new(self.unary()?)));
        }
        Ok(base)
    }

    fn primary(&mut self) -> Result<Expr, FormulaError> {
        match self.peek() {
            Some('(') => {
                self.at += 1;
                let inner = self.sum()?;
                if !self.eat(')') {
                    return Err(FormulaError("a `(` is never closed".into()));
                }
                Ok(inner)
            }
            Some(c) if c.is_ascii_digit() || c == '.' => self.number(),
            Some(c) if c.is_ascii_alphabetic() || c == '_' => self.name(),
            Some(c) => Err(FormulaError(format!("unexpected `{c}`"))),
            None => Err(FormulaError("the formula ends too early".into())),
        }
    }

    fn number(&mut self) -> Result<Expr, FormulaError> {
        let start = self.at;
        while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '.') {
            self.at += 1;
        }
        let text: String = self.chars[start..self.at].iter().collect();
        text.parse()
            .map(Expr::Number)
            .map_err(|_| FormulaError(format!("`{text}` is not a number")))
    }

    fn name(&mut self) -> Result<Expr, FormulaError> {
        let start = self.at;
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            self.at += 1;
        }
        let name: String = self.chars[start..self.at].iter().collect();
        if self.eat('(') {
            let Some((function, arity)) = Function::named(&name) else {
                return Err(FormulaError(format!(
                    "unknown function `{name}`; use log2, ceil, floor, min, max, or next_pow2"
                )));
            };
            let mut args = Vec::new();
            if !self.eat(')') {
                loop {
                    args.push(self.sum()?);
                    if self.eat(')') {
                        break;
                    }
                    if !self.eat(',') {
                        return Err(FormulaError(format!("`{name}(` is never closed")));
                    }
                }
            }
            if args.len() != arity {
                return Err(FormulaError(format!(
                    "`{name}` takes {arity} argument(s), got {}",
                    args.len()
                )));
            }
            return Ok(Expr::Call(function, args));
        }
        self.params
            .iter()
            .position(|param| *param == name)
            .map(Expr::Param)
            .ok_or_else(|| FormulaError(format!("`{name}` is not a declared parameter")))
    }
}
