//! Working a formula out, exactly, with its units checked.

use crate::parse::{Expr, Op, parse};
use crate::quantity::{Dim, Kind, Quantity};
use crate::{FormulaError, MAX_DIGITS};
use dashu_int::{IBig, UBig};
use dashu_ratio::RBig;

/// A value inside a formula: a quantity, or the answer to a comparison.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Val {
    Q(Quantity),
    Truth(bool),
}

fn units(message: impl Into<String>) -> FormulaError {
    FormulaError::new("Units", message)
}

fn ratio(numerator: u64, denominator: u64) -> RBig {
    RBig::from_parts(IBig::from(numerator), UBig::from(denominator))
}

/// A number as written, exactly.
pub(crate) fn decimal(text: &str) -> Result<RBig, FormulaError> {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if whole.len() + fraction.len() > MAX_DIGITS {
        return Err(FormulaError::new(
            "TooLarge",
            format!("A number has at most {MAX_DIGITS} digits."),
        ));
    }
    let digits = format!("{whole}{fraction}");
    let numerator = UBig::from_str_radix(if digits.is_empty() { "0" } else { &digits }, 10)
        .map_err(|_| FormulaError::new("Syntax", format!("{text} isn't a number.")))?;
    let denominator = UBig::from(10u8).pow(fraction.len());
    Ok(RBig::from_parts(IBig::from(numerator), denominator))
}

/// How much of the base unit one of a unit is, and the dimension it has.
fn unit(name: &str) -> (RBig, Dim) {
    match name {
        "cm" => (ratio(10, 1), Dim::LENGTH),
        "m" => (ratio(1000, 1), Dim::LENGTH),
        // exactly, by the international yard of 1959
        "in" => (ratio(127, 5), Dim::LENGTH),
        "ft" => (ratio(1524, 5), Dim::LENGTH),
        "deg" => (ratio(1, 1), Dim::ANGLE),
        _ => (ratio(1, 1), Dim::LENGTH),
    }
}

fn quantity(val: Val, what: &str) -> Result<Quantity, FormulaError> {
    match val {
        Val::Q(q) => Ok(q),
        Val::Truth(_) => Err(FormulaError::new(
            "Type",
            format!("{what} needs a number, and this is a yes or no."),
        )),
    }
}

fn truth(val: Val, what: &str) -> Result<bool, FormulaError> {
    match val {
        Val::Truth(t) => Ok(t),
        Val::Q(_) => Err(FormulaError::new(
            "Type",
            format!("{what} needs a yes or no, such as a < b, and this is a number."),
        )),
    }
}

fn same(a: &Quantity, b: &Quantity, doing: &str) -> Result<(), FormulaError> {
    if a.dim == b.dim {
        return Ok(());
    }
    let hint = if a.dim == Dim::NUMBER || b.dim == Dim::NUMBER {
        ": give the number a unit, such as 5 mm"
    } else {
        ""
    };
    Err(units(format!(
        "{} and {} don't {doing}{hint}.",
        capital(&a.dim.to_string()),
        b.dim
    )))
}

fn capital(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |c| {
        c.to_uppercase().collect::<String>() + chars.as_str()
    })
}

fn q(value: RBig, dim: Dim) -> Result<Val, FormulaError> {
    Ok(Val::Q(Quantity { value, dim }.checked()?))
}

/// What a function call gives.
fn call(name: &str, args: Vec<Val>) -> Result<Val, FormulaError> {
    let count = args.len();
    let numbers: Vec<Quantity> = args
        .into_iter()
        .map(|a| quantity(a, name))
        .collect::<Result<_, _>>()?;
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
    match name {
        "min" | "max" => {
            arity("one or more values", count >= 1)?;
            let mut best = numbers[0].clone();
            for n in &numbers[1..] {
                same(&best, n, "compare")?;
                let better = if name == "min" {
                    n.value < best.value
                } else {
                    n.value > best.value
                };
                if better {
                    best = n.clone();
                }
            }
            Ok(Val::Q(best))
        }
        "abs" => {
            arity("one value", count == 1)?;
            let n = &numbers[0];
            let value = if n.value < RBig::ZERO {
                -n.value.clone()
            } else {
                n.value.clone()
            };
            q(value, n.dim)
        }
        "round" | "floor" | "ceil" => {
            arity(
                "a value, and for a length or an angle a step",
                count == 1 || count == 2,
            )?;
            let n = &numbers[0];
            let step = match numbers.get(1) {
                Some(step) => {
                    same(n, step, "round together")?;
                    if step.value <= RBig::ZERO {
                        return Err(units(format!("{name}'s step must be more than zero.")));
                    }
                    step.value.clone()
                }
                None if n.dim == Dim::NUMBER => RBig::ONE,
                None => {
                    return Err(units(format!(
                        "{name} of {} needs a step to round to, such as {name}(x, 1 mm).",
                        n.dim
                    )));
                }
            };
            let units_of_step = n.value.clone() / step.clone();
            let whole = match name {
                "round" => units_of_step.round(),
                "floor" => units_of_step.floor(),
                _ => units_of_step.ceil(),
            };
            q(RBig::from(whole) * step, n.dim)
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

/// Works out an expression. `lookup` gives a name's value, or why it has none.
pub(crate) fn eval(
    e: &Expr,
    lookup: &mut dyn FnMut(&str) -> Result<Quantity, FormulaError>,
) -> Result<Val, FormulaError> {
    match e {
        Expr::Number(text) => q(decimal(text)?, Dim::NUMBER),
        Expr::Measure(parts) => {
            let mut total: Option<Quantity> = None;
            for (text, name) in parts {
                let (scale, dim) = unit(name);
                let part = Quantity {
                    value: decimal(text)? * scale,
                    dim,
                };
                total = Some(match total {
                    None => part,
                    Some(t) => {
                        same(&t, &part, "add")?;
                        Quantity {
                            value: t.value + part.value,
                            dim,
                        }
                    }
                });
            }
            let total = total.ok_or_else(|| FormulaError::new("Syntax", "A measure is empty."))?;
            Ok(Val::Q(total.checked()?))
        }
        Expr::Name(name) => Ok(Val::Q(lookup(name)?)),
        Expr::Neg(inner) => {
            let n = quantity(eval(inner, lookup)?, "A minus sign")?;
            q(-n.value, n.dim)
        }
        Expr::Not(inner) => Ok(Val::Truth(!truth(eval(inner, lookup)?, "!")?)),
        Expr::If(test, yes, no) => {
            let pick = truth(eval(test, lookup)?, "A choice (?:)")?;
            // Both sides are worked out, so a formula's units never depend on its values.
            let (yes, no) = (eval(yes, lookup)?, eval(no, lookup)?);
            match (&yes, &no) {
                (Val::Q(a), Val::Q(b)) => same(a, b, "make one choice")?,
                (Val::Truth(_), Val::Truth(_)) => {}
                _ => {
                    return Err(FormulaError::new(
                        "Type",
                        "Both sides of a choice (?:) must be numbers, or both yes or no.",
                    ));
                }
            }
            Ok(if pick { yes } else { no })
        }
        Expr::Call(name, args) => {
            let values = args
                .iter()
                .map(|a| eval(a, lookup))
                .collect::<Result<Vec<_>, _>>()?;
            call(name, values)
        }
        Expr::Binary(op, left, right) => {
            let (a, b) = (eval(left, lookup)?, eval(right, lookup)?);
            binary(*op, a, b)
        }
    }
}

fn binary(op: Op, a: Val, b: Val) -> Result<Val, FormulaError> {
    match op {
        Op::And | Op::Or => {
            let (a, b) = (truth(a, "&& and ||")?, truth(b, "&& and ||")?);
            Ok(Val::Truth(if op == Op::And { a && b } else { a || b }))
        }
        Op::Eq | Op::Ne if matches!((&a, &b), (Val::Truth(_), Val::Truth(_))) => {
            Ok(Val::Truth((a == b) == (op == Op::Eq)))
        }
        _ => {
            let what = match op {
                Op::Add | Op::Sub => "+ and -",
                Op::Mul | Op::Div => "* and /",
                _ => "A comparison",
            };
            let (a, b) = (quantity(a, what)?, quantity(b, what)?);
            match op {
                Op::Add | Op::Sub => {
                    same(&a, &b, if op == Op::Add { "add" } else { "subtract" })?;
                    let value = if op == Op::Add {
                        a.value + b.value
                    } else {
                        a.value - b.value
                    };
                    q(value, a.dim)
                }
                Op::Mul => q(a.value * b.value, a.dim.times(b.dim, 1)?),
                Op::Div => {
                    if b.value == RBig::ZERO {
                        return Err(FormulaError::new("DivideByZero", "That divides by zero."));
                    }
                    q(a.value / b.value, a.dim.times(b.dim, -1)?)
                }
                _ => {
                    same(&a, &b, "compare")?;
                    let truth = match op {
                        Op::Lt => a.value < b.value,
                        Op::Le => a.value <= b.value,
                        Op::Gt => a.value > b.value,
                        Op::Ge => a.value >= b.value,
                        Op::Eq => a.value == b.value,
                        _ => a.value != b.value,
                    };
                    Ok(Val::Truth(truth))
                }
            }
        }
    }
}

/// A plain number standing alone is in the value's own unit: a length of `120` is 120 mm.
fn bare(e: &Expr) -> Option<(&str, bool)> {
    match e {
        Expr::Number(text) => Some((text, false)),
        Expr::Neg(inner) => match inner.as_ref() {
            Expr::Number(text) => Some((text, true)),
            _ => None,
        },
        _ => None,
    }
}

/// Works out a read formula as a value of `kind`.
pub(crate) fn as_kind(
    e: &Expr,
    kind: Kind,
    lookup: &mut dyn FnMut(&str) -> Result<Quantity, FormulaError>,
) -> Result<Quantity, FormulaError> {
    if let Some((text, negative)) = bare(e) {
        let value = decimal(text)?;
        let value = if negative { -value } else { value };
        return Quantity {
            value,
            dim: kind.dim(),
        }
        .checked();
    }
    let got = quantity(eval(e, lookup)?, "A value")?;
    if got.dim != kind.dim() {
        return Err(units(format!(
            "This works out to {}, and it should be {}.",
            got.dim,
            kind.dim()
        )));
    }
    Ok(got)
}

/// One value of `kind`, from text: a number in the kind's own unit, a parameter's name, or a
/// formula. `lookup` gives each name's value.
///
/// # Errors
///
/// Why it can't be read or worked out, as a [`FormulaError`].
pub fn value_of(
    text: &str,
    kind: Kind,
    lookup: &mut dyn FnMut(&str) -> Result<Quantity, FormulaError>,
) -> Result<Quantity, FormulaError> {
    as_kind(&parse(text)?, kind, lookup)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(name: &str) -> Result<Quantity, FormulaError> {
        Err(FormulaError::new("UnknownName", name.to_owned()))
    }

    fn mm(text: &str) -> f64 {
        value_of(text, Kind::Length, &mut none).unwrap().to_f64()
    }

    #[test]
    fn units_convert_exactly() {
        assert_eq!(mm("2 ft 8 in"), 812.8);
        assert_eq!(mm("1 m - 1 mm"), 999.0);
        assert_eq!(mm("0.1 mm + 0.2 mm"), 0.3, "exact, not 0.30000000000000004");
        assert_eq!(mm("1/3 * 3 mm"), 1.0);
        assert_eq!(mm("120"), 120.0);
        assert_eq!(mm("-8.5"), -8.5);
    }

    #[test]
    fn units_must_agree() {
        let e = value_of("5 mm + 2", Kind::Length, &mut none).unwrap_err();
        assert_eq!(e.code, "Units");
        assert!(
            e.message.contains("give the number a unit"),
            "{}",
            e.message
        );
        let e = value_of("5 mm * 2 mm", Kind::Length, &mut none).unwrap_err();
        assert!(e.message.contains("an area"), "{}", e.message);
        assert_eq!(
            value_of("10 mm / 2 mm", Kind::Count, &mut none)
                .unwrap()
                .to_f64(),
            5.0
        );
        assert_eq!(
            value_of("30 deg + 5 mm", Kind::Angle, &mut none)
                .unwrap_err()
                .code,
            "Units"
        );
    }

    #[test]
    fn choices_functions_and_rounding() {
        assert_eq!(mm("3 > 2 ? 4 mm : 5 mm"), 4.0);
        assert_eq!(mm("max(2 mm, 9 mm, 3 mm)"), 9.0);
        assert_eq!(mm("round(12.5 mm, 1 mm)"), 13.0);
        assert_eq!(mm("round(-12.5 mm, 1 mm)"), -13.0);
        assert_eq!(mm("floor(12.9 mm, 0.5 mm)"), 12.5);
        assert_eq!(mm("abs(-3 mm)"), 3.0);
        let e = value_of("round(12.5 mm)", Kind::Length, &mut none).unwrap_err();
        assert!(e.message.contains("needs a step"), "{}", e.message);
        assert_eq!(
            value_of("sqrt(2) * 1 mm", Kind::Length, &mut none)
                .unwrap_err()
                .code,
            "NotExact"
        );
        assert_eq!(
            value_of("1 mm / 0", Kind::Length, &mut none)
                .unwrap_err()
                .code,
            "DivideByZero"
        );
        assert_eq!(
            value_of("2 > 1", Kind::Length, &mut none).unwrap_err().code,
            "Type"
        );
        let e = value_of("1 > 0 ? 1 mm : 1 deg", Kind::Length, &mut none).unwrap_err();
        assert_eq!(e.code, "Units");
    }

    #[test]
    fn numbers_stay_a_sensible_size() {
        let huge = "999999999999999 * ".repeat(50) + "1 mm";
        assert_eq!(
            value_of(&huge, Kind::Length, &mut none).unwrap_err().code,
            "TooLarge"
        );
        let long = format!("{} mm", "9".repeat(31));
        assert_eq!(
            value_of(&long, Kind::Length, &mut none).unwrap_err().code,
            "TooLarge"
        );
    }
}
