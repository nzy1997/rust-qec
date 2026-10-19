//! Bounded expression evaluation. Exact rational multiples of pi are never inferred
//! from floating-point proximity.
use super::{ErrorCode, QasmError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Angle {
    Rational {
        numerator: i64,
        denominator: u64,
        pi: bool,
    },
    Radians {
        value: f64,
    },
}
impl Angle {
    pub fn pi_ratio(numerator: i64, denominator: u64) -> Result<Self, QasmError> {
        Self::ratio(numerator as i128, denominator as i128, true)
    }
    pub fn radians(value: f64) -> Result<Self, QasmError> {
        if !value.is_finite() {
            return Err(err("non-finite expression result"));
        }
        Ok(Self::Radians { value })
    }
    fn ratio(n: i128, d: i128, pi: bool) -> Result<Self, QasmError> {
        if d == 0 {
            return Err(err("division by zero"));
        }
        let (n, d) = if d < 0 { (-n, -d) } else { (n, d) };
        let mut a = n.unsigned_abs();
        let mut b = d as u128;
        while b != 0 {
            let r = a % b;
            a = b;
            b = r;
        }
        let g = a.max(1) as i128;
        let (n, d) = (n / g, d / g);
        if n < i64::MIN as i128 || n > i64::MAX as i128 || d > u64::MAX as i128 {
            return Err(QasmError::new(
                ErrorCode::Limit,
                "exact expression arithmetic limit exceeded",
            ));
        }
        Ok(Self::Rational {
            numerator: n as i64,
            denominator: d as u64,
            pi: pi && n != 0,
        })
    }
    pub fn value(&self) -> f64 {
        match self {
            Self::Rational {
                numerator,
                denominator,
                pi,
            } => {
                *numerator as f64 / *denominator as f64
                    * if *pi { std::f64::consts::PI } else { 1.0 }
            }
            Self::Radians { value } => *value,
        }
    }
    pub fn half_turns(&self) -> f64 {
        match self {
            Self::Rational {
                numerator,
                denominator,
                pi: true,
            } => *numerator as f64 / *denominator as f64,
            _ => self.value() / std::f64::consts::PI,
        }
    }
    /// Reduce an executable rotation before conversion, retaining exact pi periods.
    /// Raw `half_turns` remains available for inspecting the unwrapped parameter.
    pub(super) fn execution_half_turns(&self) -> f64 {
        match self {
            Self::Rational {
                numerator,
                denominator,
                pi: true,
            } => {
                // SU(2) rotations have period four half-turns. Converting the
                // numerator first loses low bits above 2^53 and can erase a gate.
                let period = 4i128 * *denominator as i128;
                (*numerator as i128).rem_euclid(period) as f64 / *denominator as f64
            }
            _ => {
                // libm performs argument reduction for large finite radians.
                // Dividing by an approximate pi first can erase their remainder.
                // The discarded 2*pi phase is global to this expanded rotation.
                let (sin, cos) = self.value().sin_cos();
                sin.atan2(cos) / std::f64::consts::PI
            }
        }
    }
    pub fn clifford_quarters(&self) -> Option<i64> {
        match self {
            Self::Rational {
                numerator,
                denominator,
                pi,
            } if *pi || *numerator == 0 => {
                let n = *numerator as i128 * 2;
                let d = *denominator as i128;
                if d != 0 && n % d == 0 {
                    Some((n / d).rem_euclid(4) as i64)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
    pub fn validate(&self) -> Result<(), QasmError> {
        match self {
            Self::Rational { denominator: 0, .. } => Err(err("zero rational denominator")),
            _ if !self.value().is_finite() => Err(err("non-finite angle")),
            _ => Ok(()),
        }
    }
    pub fn expression(&self) -> String {
        match self {
            Self::Rational {
                numerator,
                denominator,
                pi,
            } => {
                let n = if *pi {
                    format!("{numerator}*pi")
                } else {
                    numerator.to_string()
                };
                if *denominator == 1 {
                    n
                } else {
                    format!("({n}/{denominator})")
                }
            }
            Self::Radians { value } => format!("{value:.17e}"),
        }
    }
    pub(super) fn neg(&self) -> Result<Self, QasmError> {
        match self {
            Self::Rational {
                numerator,
                denominator,
                pi,
            } => Self::ratio(-(*numerator as i128), *denominator as i128, *pi),
            _ => Self::radians(-self.value()),
        }
    }
    fn binary(&self, op: char, rhs: &Self) -> Result<Self, QasmError> {
        if let (
            Self::Rational {
                numerator: a,
                denominator: b,
                pi: p,
            },
            Self::Rational {
                numerator: c,
                denominator: d,
                pi: q,
            },
        ) = (self, rhs)
        {
            let (a, b, c, d) = (*a as i128, *b as i128, *c as i128, *d as i128);
            let mul = |x: i128, y: i128| {
                x.checked_mul(y)
                    .ok_or_else(|| QasmError::new(ErrorCode::Limit, "exact arithmetic overflow"))
            };
            match op {
                '+' | '-' if p == q || a == 0 || c == 0 => {
                    let x = mul(a, d)?;
                    let y = mul(c, b)?;
                    let n = if op == '+' {
                        x.checked_add(y)
                    } else {
                        x.checked_sub(y)
                    }
                    .ok_or_else(|| QasmError::new(ErrorCode::Limit, "exact arithmetic overflow"))?;
                    return Self::ratio(n, mul(b, d)?, if a == 0 { *q } else { *p });
                }
                '*' if !(*p && *q) => return Self::ratio(mul(a, c)?, mul(b, d)?, *p || *q),
                '/' if !*q || *p => return Self::ratio(mul(a, d)?, mul(b, c)?, *p && !*q),
                '^' if !*q && d == 1 && c.unsigned_abs() <= 32 && (!*p || c == 0 || c == 1) => {
                    if c == 0 {
                        return Self::ratio(1, 1, false);
                    }
                    let power = c.unsigned_abs() as u32;
                    let n = a.checked_pow(power).ok_or_else(|| {
                        QasmError::new(ErrorCode::Limit, "exact arithmetic overflow")
                    })?;
                    let den = b.checked_pow(power).ok_or_else(|| {
                        QasmError::new(ErrorCode::Limit, "exact arithmetic overflow")
                    })?;
                    return if c < 0 {
                        Self::ratio(den, n, *p)
                    } else {
                        Self::ratio(n, den, *p)
                    };
                }
                _ => {}
            }
        }
        let (a, b) = (self.value(), rhs.value());
        if op == '/' && b == 0.0 {
            return Err(err("division by zero"));
        }
        Self::radians(match op {
            '+' => a + b,
            '-' => a - b,
            '*' => a * b,
            '/' => a / b,
            '^' => a.powf(b),
            _ => unreachable!(),
        })
    }
    pub(super) fn literal(text: &str) -> Result<Self, QasmError> {
        // Decimal exponents remain exact when they fit the bounded rational domain.
        let (base, exp) = match text.find(['e', 'E']) {
            Some(i) => (
                &text[..i],
                text[i + 1..]
                    .parse::<i32>()
                    .map_err(|_| err("invalid exponent"))?,
            ),
            None => (text, 0),
        };
        let digits: String = base.chars().filter(|c| *c != '.').collect();
        let frac = base.find('.').map_or(0, |i| base.len() - i - 1) as i32;
        if let Ok(n) = digits.parse::<i128>() {
            let power = exp
                .checked_sub(frac)
                .ok_or_else(|| err("invalid exponent"))?;
            if power.unsigned_abs() <= 18 {
                let scale = 10i128.pow(power.unsigned_abs());
                if let Some(num) = if power >= 0 {
                    n.checked_mul(scale)
                } else {
                    Some(n)
                } {
                    if let Ok(v) = Self::ratio(num, if power < 0 { scale } else { 1 }, false) {
                        return Ok(v);
                    }
                }
            }
        }
        Self::radians(text.parse::<f64>().map_err(|_| err("invalid number"))?)
    }
}
fn err(message: impl Into<String>) -> QasmError {
    QasmError::new(ErrorCode::Type, message)
}
#[derive(Clone, Debug)]
pub(super) enum Expr {
    Number(String),
    Pi,
    Name(String),
    Neg(Box<Expr>),
    Binary(char, Box<Expr>, Box<Expr>),
    Function(String, Box<Expr>),
}
impl Expr {
    pub(super) fn ensure_depth(&self, max: usize) -> Result<(), QasmError> {
        let mut pending = vec![(self, 1)];
        while let Some((expr, depth)) = pending.pop() {
            if depth > max {
                return Err(QasmError::new(
                    ErrorCode::Limit,
                    "expression depth limit exceeded",
                ));
            }
            match expr {
                Self::Neg(a) | Self::Function(_, a) => pending.push((a, depth + 1)),
                Self::Binary(_, a, b) => {
                    pending.push((a, depth + 1));
                    pending.push((b, depth + 1));
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn has_names(&self) -> bool {
        match self {
            Self::Name(_) => true,
            Self::Neg(a) | Self::Function(_, a) => a.has_names(),
            Self::Binary(_, a, b) => a.has_names() || b.has_names(),
            _ => false,
        }
    }
    pub(super) fn check(&self, names: &[String]) -> Result<(), QasmError> {
        match self {
            Self::Name(n) if !names.contains(n) => {
                return Err(err(format!("unbound parameter {n}")));
            }
            Self::Neg(a) | Self::Function(_, a) => a.check(names)?,
            Self::Binary(op, a, b) => {
                a.check(names)?;
                b.check(names)?;
                if *op == '/' && !b.has_names() && b.eval(&HashMap::new())?.value() == 0.0 {
                    return Err(err("division by zero"));
                }
            }
            _ => {}
        }
        if !self.has_names() {
            self.eval(&HashMap::new())?;
        }
        Ok(())
    }
    pub(super) fn eval(&self, env: &HashMap<String, Angle>) -> Result<Angle, QasmError> {
        match self {
            Self::Number(n) => Angle::literal(n),
            Self::Pi => Angle::pi_ratio(1, 1),
            Self::Name(n) => env
                .get(n)
                .cloned()
                .ok_or_else(|| err(format!("unbound parameter {n}"))),
            Self::Neg(a) => a.eval(env)?.neg(),
            Self::Binary(op, a, b) => a.eval(env)?.binary(*op, &b.eval(env)?),
            Self::Function(f, a) => {
                let angle = a.eval(env)?;
                let x = angle.value();
                if f == "tan" && angle.clifford_quarters().is_some_and(|q| q % 2 == 1) {
                    return Err(err("tan is undefined at an odd multiple of pi/2"));
                }
                Angle::radians(match f.as_str() {
                    "sin" => x.sin(),
                    "cos" => x.cos(),
                    "tan" => x.tan(),
                    "exp" => x.exp(),
                    "ln" => x.ln(),
                    "sqrt" => x.sqrt(),
                    _ => return Err(err("unknown expression function")),
                })
            }
        }
    }
}
