//! The type check: what a formula works out to, from the kinds of the names it uses, before any
//! value is worked out. A length plus an area is refused here.
//!
//! Inside a formula a number carries a dimension, the powers of length and angle, so a step on the
//! way may be a length to the fourth power, which no parameter holds. The dimension is this
//! module's own and never leaves the crate: a formula's inputs and answer are `construct-params`'
//! kinds, and [`of_kind`] and [`fits`] are the one map between the two.

use crate::FormulaError;
use crate::parse::{Expr, Op, unit_of};
use construct_params::Kind;
use std::fmt;

/// A dimension: the powers of length and angle. A plain number (a count, a ratio) has both zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) struct Dim {
    pub(crate) length: i8,
    pub(crate) angle: i8,
}

/// The largest power a dimension may reach, either way. Nothing a part needs comes near it.
const MOST_POWER: i8 = 6;

impl Dim {
    pub(crate) const NUMBER: Self = Self {
        length: 0,
        angle: 0,
    };
    pub(crate) const LENGTH: Self = Self {
        length: 1,
        angle: 0,
    };
    pub(crate) const AREA: Self = Self {
        length: 2,
        angle: 0,
    };
    pub(crate) const VOLUME: Self = Self {
        length: 3,
        angle: 0,
    };
    pub(crate) const ANGLE: Self = Self {
        length: 0,
        angle: 1,
    };

    /// `self` times `other` (`sign` 1) or over it (`sign` -1).
    pub(crate) fn times(self, other: Self, sign: i8) -> Result<Self, FormulaError> {
        let power = |a: i8, b: i8| {
            a.checked_add(b.checked_mul(sign)?)
                .filter(|p| p.abs() <= MOST_POWER)
        };
        match (
            power(self.length, other.length),
            power(self.angle, other.angle),
        ) {
            (Some(length), Some(angle)) => Ok(Self { length, angle }),
            _ => Err(FormulaError::new(
                "Units",
                "That makes a unit no part needs (a power past six).",
            )),
        }
    }
}

impl fmt::Display for Dim {
    /// In words, with an article: "a length", "an area", "a plain number".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.length, self.angle) {
            (0, 0) => f.write_str("a plain number"),
            (1, 0) => f.write_str("a length"),
            (2, 0) => f.write_str("an area"),
            (3, 0) => f.write_str("a volume"),
            (0, 1) => f.write_str("an angle"),
            (l, a) => {
                let part = |name: &str, p: i8| match p {
                    0 => String::new(),
                    1 => name.to_owned(),
                    p => format!("{name} to the power {p}"),
                };
                let words: Vec<String> = [part("length", l), part("angle", a)]
                    .into_iter()
                    .filter(|w| !w.is_empty())
                    .collect();
                write!(f, "{}", words.join(" times "))
            }
        }
    }
}

/// What a formula or a part of one is: a number of a dimension, or a yes or no.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ty {
    Q(Dim),
    Truth,
}

/// What a value of `kind` is inside a formula; `None` for a kind no formula uses (text, a choice,
/// a link).
pub(crate) fn of_kind(kind: Kind) -> Option<Ty> {
    Some(match kind {
        Kind::Length => Ty::Q(Dim::LENGTH),
        Kind::Area => Ty::Q(Dim::AREA),
        Kind::Volume => Ty::Q(Dim::VOLUME),
        Kind::Angle => Ty::Q(Dim::ANGLE),
        Kind::Slope | Kind::Number | Kind::Whole => Ty::Q(Dim::NUMBER),
        Kind::YesNo => Ty::Truth,
        Kind::Text | Kind::Choice | Kind::Element | Kind::Material | Kind::Link => return None,
    })
}

/// The refusal of a parameter of a kind no formula works with.
pub(crate) fn unusable(name: &str, kind: Kind) -> FormulaError {
    FormulaError::new(
        "Type",
        format!("{name} is {kind}, and a formula works with numbers and yes or no."),
    )
}

/// What a parameter of `kind` is inside a formula, or the refusal naming it.
pub(crate) fn usable(name: &str, kind: Kind) -> Result<Ty, FormulaError> {
    of_kind(kind).ok_or_else(|| unusable(name, kind))
}

/// Whether a formula of type `got` gives a value of `kind`, and if not, why.
pub(crate) fn fits(got: Ty, kind: Kind) -> Result<(), FormulaError> {
    let want = of_kind(kind).ok_or_else(|| {
        FormulaError::new(
            "Type",
            format!("No formula gives {kind}: a formula works with numbers and yes or no."),
        )
    })?;
    match (got, want) {
        (a, b) if a == b => Ok(()),
        // Named by dimension, as the language always has: a number, a whole number and a slope
        // are each "a plain number".
        (Ty::Q(dim), Ty::Q(want)) => Err(units(format!(
            "This works out to {dim}, and it should be {want}."
        ))),
        (Ty::Q(dim), Ty::Truth) => Err(FormulaError::new(
            "Type",
            format!("This works out to {dim}, and it should be {kind}."),
        )),
        (Ty::Truth, _) => Err(FormulaError::new(
            "Type",
            format!("This works out to a yes or no, and it should be {kind}."),
        )),
    }
}

pub(crate) fn units(message: impl Into<String>) -> FormulaError {
    FormulaError::new("Units", message)
}

fn capital(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |c| {
        c.to_uppercase().collect::<String>() + chars.as_str()
    })
}

/// Two dimensions that must be the same to `doing` (add, compare, ...).
pub(crate) fn same(a: Dim, b: Dim, doing: &str) -> Result<(), FormulaError> {
    if a == b {
        return Ok(());
    }
    let hint = if a == Dim::NUMBER || b == Dim::NUMBER {
        ": give the number a unit, such as 5 mm"
    } else {
        ""
    };
    Err(units(format!(
        "{} and {b} don't {doing}{hint}.",
        capital(&a.to_string())
    )))
}

pub(crate) fn number(ty: Ty, what: &str) -> Result<Dim, FormulaError> {
    match ty {
        Ty::Q(dim) => Ok(dim),
        Ty::Truth => Err(FormulaError::new(
            "Type",
            format!("{what} needs a number, and this is a yes or no."),
        )),
    }
}

pub(crate) fn truth(ty: Ty, what: &str) -> Result<(), FormulaError> {
    match ty {
        Ty::Truth => Ok(()),
        Ty::Q(_) => Err(FormulaError::new(
            "Type",
            format!("{what} needs a yes or no, such as a < b, and this is a number."),
        )),
    }
}

/// The two sides of a choice (`?:`) must be of one type.
pub(crate) fn choice(yes: Ty, no: Ty) -> Result<Ty, FormulaError> {
    match (yes, no) {
        (Ty::Q(a), Ty::Q(b)) => {
            same(a, b, "make one choice")?;
            Ok(yes)
        }
        (Ty::Truth, Ty::Truth) => Ok(Ty::Truth),
        _ => Err(FormulaError::new(
            "Type",
            "Both sides of a choice (?:) must be numbers, or both yes or no.",
        )),
    }
}

/// What a binary step gives.
pub(crate) fn binary(op: Op, a: Ty, b: Ty) -> Result<Ty, FormulaError> {
    match op {
        Op::And | Op::Or => {
            truth(a, "&& and ||")?;
            truth(b, "&& and ||")?;
            Ok(Ty::Truth)
        }
        Op::Eq | Op::Ne if a == Ty::Truth && b == Ty::Truth => Ok(Ty::Truth),
        _ => {
            let what = match op {
                Op::Add | Op::Sub => "+ and -",
                Op::Mul | Op::Div => "* and /",
                _ => "A comparison",
            };
            let (a, b) = (number(a, what)?, number(b, what)?);
            match op {
                Op::Add | Op::Sub => {
                    same(a, b, if op == Op::Add { "add" } else { "subtract" })?;
                    Ok(Ty::Q(a))
                }
                Op::Mul => Ok(Ty::Q(a.times(b, 1)?)),
                Op::Div => Ok(Ty::Q(a.times(b, -1)?)),
                _ => {
                    same(a, b, "compare")?;
                    Ok(Ty::Truth)
                }
            }
        }
    }
}

/// What a call gives.
pub(crate) fn call(name: &str, args: &[Ty]) -> Result<Ty, FormulaError> {
    let count = args.len();
    let arity = |want: &str, ok: bool| {
        if ok {
            Ok(())
        } else {
            Err(FormulaError::new(
                "Syntax",
                format!("{name} takes {want}, not {count}."),
            ))
        }
    };
    let dims =
        || -> Result<Vec<Dim>, FormulaError> { args.iter().map(|a| number(*a, name)).collect() };
    match name {
        "min" | "max" => {
            let dims = dims()?;
            arity("one or more values", count >= 1)?;
            let first = dims.first().copied().unwrap_or_default();
            for d in dims.iter().skip(1) {
                same(first, *d, "compare")?;
            }
            Ok(Ty::Q(first))
        }
        "abs" => {
            let dims = dims()?;
            arity("one value", count == 1)?;
            Ok(Ty::Q(dims.first().copied().unwrap_or_default()))
        }
        "round" | "floor" | "ceil" => {
            let dims = dims()?;
            arity(
                "a value, and for a length or an angle a step",
                count == 1 || count == 2,
            )?;
            let n = dims.first().copied().unwrap_or_default();
            match dims.get(1) {
                Some(step) => same(n, *step, "round together")?,
                None if n == Dim::NUMBER => {}
                None => {
                    return Err(units(format!(
                        "{name} of {n} needs a step to round to, such as {name}(x, 1 mm)."
                    )));
                }
            }
            Ok(Ty::Q(n))
        }
        "sqrt" | "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "atan2" | "exp" | "ln"
        | "log" | "pow" | "pi" => Err(FormulaError::new(
            "NotExact",
            format!(
                "{name} isn't exact, so it waits for the kernel's certified functions; formulas \
                 here are worked out exactly."
            ),
        )),
        _ => Err(FormulaError::new(
            "UnknownName",
            format!(
                "There is no function called {name}. There are min, max, abs, round, floor and \
                 ceil."
            ),
        )),
    }
}

/// A plain number standing alone is in its kind's own unit: a length of `120` is 120 mm.
pub(crate) fn bare(e: &Expr) -> Option<(&str, bool)> {
    match e {
        Expr::Number(text) => Some((text, false)),
        Expr::Neg(inner) => match inner.as_ref() {
            Expr::Number(text) => Some((text, true)),
            _ => None,
        },
        _ => None,
    }
}

/// The type of an expression, given the kind of each name it uses (or why a name has none).
pub(crate) fn check(
    e: &Expr,
    kind_of: &mut dyn FnMut(&str) -> Result<Kind, FormulaError>,
) -> Result<Ty, FormulaError> {
    match e {
        Expr::Number(_) => Ok(Ty::Q(Dim::NUMBER)),
        Expr::Measure(parts) => {
            let mut total: Option<Dim> = None;
            for (_, symbol) in parts {
                let dim = unit_of(symbol).map(|(_, dim)| dim).ok_or_else(|| {
                    FormulaError::new("Syntax", format!("{symbol} isn't a unit."))
                })?;
                if let Some(t) = total {
                    same(t, dim, "add")?;
                }
                total = Some(dim);
            }
            total
                .map(Ty::Q)
                .ok_or_else(|| FormulaError::new("Syntax", "A measure is empty."))
        }
        Expr::Name(name) => usable(name, kind_of(name)?),
        Expr::Neg(inner) => Ok(Ty::Q(number(check(inner, kind_of)?, "A minus sign")?)),
        Expr::Not(inner) => {
            truth(check(inner, kind_of)?, "!")?;
            Ok(Ty::Truth)
        }
        Expr::If(test, yes, no) => {
            truth(check(test, kind_of)?, "A choice (?:)")?;
            let yes = check(yes, kind_of)?;
            let no = check(no, kind_of)?;
            choice(yes, no)
        }
        Expr::Call(name, args) => {
            let types = args
                .iter()
                .map(|a| check(a, kind_of))
                .collect::<Result<Vec<_>, _>>()?;
            call(name, &types)
        }
        Expr::Binary(op, left, right) => {
            let a = check(left, kind_of)?;
            let b = check(right, kind_of)?;
            binary(*op, a, b)
        }
    }
}

/// Checks a read formula gives a value of `kind`, with the bare-number rule.
pub(crate) fn check_as(
    e: &Expr,
    kind: Kind,
    kind_of: &mut dyn FnMut(&str) -> Result<Kind, FormulaError>,
) -> Result<(), FormulaError> {
    if bare(e).is_some() && of_kind(kind).is_some_and(|ty| ty != Ty::Truth) {
        return Ok(());
    }
    fits(check(e, kind_of)?, kind)
}
