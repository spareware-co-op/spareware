//! Working a formula out, exactly, and landing its answer on a `construct-params` value.
//!
//! Inside, every number is an exact rational (`dashu-ratio`) in `construct-params`' stored units:
//! a length in grid steps of the geometry kernel (6,400 to the millimetre), an area in square
//! steps, a volume in cubic steps, an angle in the kernel's angle units (9,000,000 to the degree),
//! anything else plain. A length times a length is then square steps, an area's own unit, so no
//! step converts anything. The rationals never leave: a parameter's value comes in as a
//! [`Value`] and the answer goes out as one, landed by [`land`] with the rule its kind has for
//! typed text.

use crate::parse::{Expr, Op, parse, unit_of};
use crate::types::{self, Dim, Ty, bare};
use crate::{FormulaError, MAX_BITS, MAX_DIGITS};
use construct_params::{Angle, Area, Decimal, Kind, Length, Slope, Unit, Value, Volume};
use dashu_int::ops::{BitTest, DivRem, UnsignedAbs};
use dashu_int::{IBig, UBig};
use dashu_ratio::RBig;

/// A value inside a formula: an exact number of stored units and its dimension, or a yes or no.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Val {
    Q(RBig, Dim),
    Truth(bool),
}

fn whole(n: i128) -> RBig {
    RBig::from(IBig::from(n))
}

/// Refuses a number grown too large to be worth working with.
fn checked(value: RBig, dim: Dim) -> Result<Val, FormulaError> {
    let bits = value.numerator().unsigned_abs().bit_len() + value.denominator().bit_len();
    if bits > MAX_BITS {
        return Err(FormulaError::new(
            "TooLarge",
            "That works out to a number too large to keep exactly.",
        ));
    }
    Ok(Val::Q(value, dim))
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

/// Stored units in one of `unit`, exactly, from `construct-params`' table.
fn factor(unit: Unit) -> Result<RBig, FormulaError> {
    let (num, den) = unit
        .factor()
        .ok_or_else(|| FormulaError::new("Syntax", format!("{} isn't a unit.", unit.symbol())))?;
    Ok(whole(num) / whole(den))
}

/// A parameter's value inside a formula.
pub(crate) fn from_value(name: &str, value: &Value) -> Result<Val, FormulaError> {
    let q = |n: i128, dim| Ok(Val::Q(whole(n), dim));
    match value {
        Value::Length(l) => q(i128::from(l.steps()), Dim::LENGTH),
        Value::Area(a) => q(a.square_steps(), Dim::AREA),
        Value::Volume(v) => q(v.cubic_steps(), Dim::VOLUME),
        Value::Angle(a) => q(i128::from(a.units()), Dim::ANGLE),
        Value::Whole(n) => q(i128::from(*n), Dim::NUMBER),
        Value::Number(d) => {
            let places = usize::try_from(d.scale()).unwrap_or(usize::MAX);
            Ok(Val::Q(
                RBig::from_parts(IBig::from(d.mantissa()), UBig::from(10u8).pow(places)),
                Dim::NUMBER,
            ))
        }
        Value::Slope(s) => Ok(Val::Q(
            whole(i128::from(s.rise())) / whole(i128::from(s.run())),
            Dim::NUMBER,
        )),
        Value::YesNo(t) => Ok(Val::Truth(*t)),
        other => Err(types::unusable(name, other.kind())),
    }
}

fn ty(val: &Val) -> Ty {
    match val {
        Val::Q(_, d) => Ty::Q(*d),
        Val::Truth(_) => Ty::Truth,
    }
}

/// The number in a value the type check found to be one.
fn number(val: Val) -> RBig {
    match val {
        Val::Q(v, _) => v,
        Val::Truth(_) => RBig::ZERO,
    }
}

/// What a function call gives; `types::call` has checked it.
fn call(name: &str, args: Vec<Val>) -> Result<Val, FormulaError> {
    let types: Vec<Ty> = args.iter().map(ty).collect();
    let Ty::Q(dim) = types::call(name, &types)? else {
        return Err(FormulaError::new("Type", format!("{name} gives a number.")));
    };
    let mut numbers = args.into_iter().map(number);
    let first = numbers
        .next()
        .ok_or_else(|| FormulaError::new("Syntax", format!("{name} needs a value.")))?;
    match name {
        "min" | "max" => {
            let mut best = first;
            for n in numbers {
                let better = if name == "min" { n < best } else { n > best };
                if better {
                    best = n;
                }
            }
            checked(best, dim)
        }
        "abs" => checked(if first < RBig::ZERO { -first } else { first }, dim),
        _ => {
            let step = match numbers.next() {
                Some(step) => {
                    if step <= RBig::ZERO {
                        return Err(types::units(format!(
                            "{name}'s step must be more than zero."
                        )));
                    }
                    step
                }
                None => RBig::ONE,
            };
            let units_of_step = first / step.clone();
            let count = match name {
                "round" => units_of_step.round(),
                "floor" => units_of_step.floor(),
                _ => units_of_step.ceil(),
            };
            checked(RBig::from(count) * step, dim)
        }
    }
}

/// Works out an expression. `lookup` gives a name's value, or why it has none. Every step checks
/// its types again, so a value is never worked out under the wrong ones.
pub(crate) fn eval(
    e: &Expr,
    lookup: &mut dyn FnMut(&str) -> Result<Value, FormulaError>,
) -> Result<Val, FormulaError> {
    match e {
        Expr::Number(text) => checked(decimal(text)?, Dim::NUMBER),
        Expr::Measure(parts) => {
            let mut total: Option<(RBig, Dim)> = None;
            for (text, symbol) in parts {
                let (unit, dim) = unit_of(symbol).ok_or_else(|| {
                    FormulaError::new("Syntax", format!("{symbol} isn't a unit."))
                })?;
                let part = decimal(text)? * factor(unit)?;
                total = Some(match total {
                    None => (part, dim),
                    Some((t, d)) => {
                        types::same(d, dim, "add")?;
                        (t + part, dim)
                    }
                });
            }
            let (value, dim) =
                total.ok_or_else(|| FormulaError::new("Syntax", "A measure is empty."))?;
            checked(value, dim)
        }
        Expr::Name(name) => from_value(name, &lookup(name)?),
        Expr::Neg(inner) => {
            let inner = eval(inner, lookup)?;
            let dim = types::number(ty(&inner), "A minus sign")?;
            checked(-number(inner), dim)
        }
        Expr::Not(inner) => match eval(inner, lookup)? {
            Val::Truth(t) => Ok(Val::Truth(!t)),
            other => {
                types::truth(ty(&other), "!")?;
                Ok(other)
            }
        },
        Expr::If(test, yes, no) => {
            let test = eval(test, lookup)?;
            types::truth(ty(&test), "A choice (?:)")?;
            // Both sides are worked out, so a formula's units never depend on its values.
            let (yes, no) = (eval(yes, lookup)?, eval(no, lookup)?);
            types::choice(ty(&yes), ty(&no))?;
            Ok(if test == Val::Truth(true) { yes } else { no })
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
    let out = types::binary(op, ty(&a), ty(&b))?;
    match (a, b) {
        (Val::Truth(a), Val::Truth(b)) => Ok(Val::Truth(match op {
            Op::And => a && b,
            Op::Or => a || b,
            Op::Eq => a == b,
            _ => a != b,
        })),
        (Val::Q(a, _), Val::Q(b, _)) => match (op, out) {
            (Op::Add, Ty::Q(d)) => checked(a + b, d),
            (Op::Sub, Ty::Q(d)) => checked(a - b, d),
            (Op::Mul, Ty::Q(d)) => checked(a * b, d),
            (Op::Div, Ty::Q(d)) => {
                if b == RBig::ZERO {
                    return Err(FormulaError::new("DivideByZero", "That divides by zero."));
                }
                checked(a / b, d)
            }
            _ => Ok(Val::Truth(match op {
                Op::Lt => a < b,
                Op::Le => a <= b,
                Op::Gt => a > b,
                Op::Ge => a >= b,
                Op::Eq => a == b,
                _ => a != b,
            })),
        },
        _ => Err(FormulaError::new(
            "Type",
            "A yes or no and a number don't go together.",
        )),
    }
}

/// The unit a plain number standing alone is in, for a value of `kind`: millimetres, square and
/// cubic millimetres, degrees; `None` for a plain number. Fixed, never the display setting, so a
/// stored formula never changes its meaning.
fn bare_unit(kind: Kind) -> Option<Unit> {
    match kind {
        Kind::Length => Some(Unit::Millimetre),
        Kind::Area => Some(Unit::SquareMillimetre),
        Kind::Volume => Some(Unit::CubicMillimetre),
        Kind::Angle => Some(Unit::Degree),
        _ => None,
    }
}

/// Works out a read and type-checked formula as a value of `kind`.
pub(crate) fn eval_as(
    e: &Expr,
    kind: Kind,
    lookup: &mut dyn FnMut(&str) -> Result<Value, FormulaError>,
) -> Result<Value, FormulaError> {
    if let Some((text, negative)) = bare(e)
        && kind != Kind::YesNo
    {
        let value = decimal(text)?;
        let value = if negative { -value } else { value };
        let value = match bare_unit(kind) {
            Some(unit) => value * factor(unit)?,
            None => value,
        };
        let dim = match types::of_kind(kind) {
            Some(Ty::Q(d)) => d,
            _ => Dim::NUMBER,
        };
        return land(kind, checked(value, dim)?);
    }
    let got = eval(e, lookup)?;
    types::fits(ty(&got), kind)?;
    land(kind, got)
}

fn too_large(kind: Kind) -> FormulaError {
    FormulaError::new(
        "TooLarge",
        format!("That works out to {kind} too large to store."),
    )
}

/// `value` as a whole number, if it is one; else the whole numbers either side.
fn whole_or_near(value: &RBig) -> Result<IBig, (IBig, IBig)> {
    if value.denominator().is_one() {
        Ok(value.numerator().clone())
    } else {
        Err((value.floor(), value.ceil()))
    }
}

fn to_i64(n: &IBig, kind: Kind) -> Result<i64, FormulaError> {
    i64::try_from(n).map_err(|_| too_large(kind))
}

fn to_i128(n: &IBig, kind: Kind) -> Result<i128, FormulaError> {
    i128::try_from(n).map_err(|_| too_large(kind))
}

/// The refusal of a length, area or volume between grid steps, naming the two either side, as
/// `construct-params` names them for typed text.
fn off_grid(kind: Kind, below: Value, above: Value) -> FormulaError {
    let show = |v: &Value| match v {
        Value::Length(l) => l.to_string(),
        Value::Area(a) => a.to_string(),
        Value::Volume(v) => v.to_string(),
        other => other.stored(),
    };
    let (a, b) = (show(&below), show(&above));
    let mut e = FormulaError::new(
        "OffGrid",
        format!(
            "This works out to {kind} that is not a whole number of grid steps (1/6400 mm); the \
             nearest are {a} and {b}. Round it, such as round(x, 0.01 mm)."
        ),
    );
    e.nearest = Some((below, above));
    e
}

/// `value` rounded to the nearest whole number, a half to the even one: the rule
/// `construct-params` rounds a typed angle by.
fn round_half_even(value: &RBig) -> IBig {
    let floor = value.floor();
    let rest = value.clone() - RBig::from(floor.clone());
    let half = RBig::from_parts(IBig::ONE, UBig::from(2u8));
    if rest > half || (rest == half && floor.bit(0)) {
        floor + IBig::ONE
    } else {
        floor
    }
}

/// An exact decimal, if `value` is one with at most 28 places that fits.
fn exact_decimal(value: &RBig) -> Option<Decimal> {
    let mut den = value.denominator().clone();
    let twos = den.trailing_zeros().unwrap_or(0);
    den >>= twos;
    let five = UBig::from(5u8);
    let mut fives = 0usize;
    // The denominator is under 2^MAX_BITS, so this ends within MAX_BITS steps.
    while !den.is_one() {
        let (q, r) = (&den).div_rem(&five);
        if r != UBig::ZERO {
            return None;
        }
        den = q;
        fives += 1;
    }
    let scale = twos.max(fives);
    let places = u32::try_from(scale).ok().filter(|p| *p <= 28)?;
    let scaled = value.clone() * RBig::from(UBig::from(10u8).pow(scale));
    let mantissa = i128::try_from(scaled.numerator()).ok()?;
    Decimal::try_from_i128_with_scale(mantissa, places).ok()
}

/// Lands a worked-out value on `kind`, by the rule each kind has for typed text: a length, area
/// or volume is a whole number of grid steps or is refused naming the nearest values below and
/// above; an angle rounds half to even to the kernel's angle unit; a number is an exact decimal;
/// a whole number is whole; a slope is a ratio of two 64-bit whole numbers.
pub(crate) fn land(kind: Kind, val: Val) -> Result<Value, FormulaError> {
    let value = match val {
        Val::Truth(t) if kind == Kind::YesNo => return Ok(Value::YesNo(t)),
        Val::Q(v, _) if kind != Kind::YesNo => v,
        _ => {
            return Err(FormulaError::new(
                "Type",
                format!("This doesn't work out to {kind}."),
            ));
        }
    };
    match kind {
        Kind::Length => match whole_or_near(&value) {
            Ok(n) => Ok(Value::Length(Length::from_steps(to_i64(&n, kind)?))),
            Err((lo, hi)) => Err(off_grid(
                kind,
                Value::Length(Length::from_steps(to_i64(&lo, kind)?)),
                Value::Length(Length::from_steps(to_i64(&hi, kind)?)),
            )),
        },
        Kind::Area => match whole_or_near(&value) {
            Ok(n) => Ok(Value::Area(Area::from_square_steps(to_i128(&n, kind)?))),
            Err((lo, hi)) => Err(off_grid(
                kind,
                Value::Area(Area::from_square_steps(to_i128(&lo, kind)?)),
                Value::Area(Area::from_square_steps(to_i128(&hi, kind)?)),
            )),
        },
        Kind::Volume => match whole_or_near(&value) {
            Ok(n) => Ok(Value::Volume(Volume::from_cubic_steps(to_i128(&n, kind)?))),
            Err((lo, hi)) => Err(off_grid(
                kind,
                Value::Volume(Volume::from_cubic_steps(to_i128(&lo, kind)?)),
                Value::Volume(Volume::from_cubic_steps(to_i128(&hi, kind)?)),
            )),
        },
        Kind::Angle => Ok(Value::Angle(Angle::from_units(to_i64(
            &round_half_even(&value),
            kind,
        )?))),
        Kind::Whole => match whole_or_near(&value) {
            Ok(n) => Ok(Value::Whole(to_i64(&n, kind)?)),
            Err(_) => Err(FormulaError::new(
                "NotWhole",
                format!(
                    "This works out to {value}, and it should be a whole number. Round it, such \
                     as round(x)."
                ),
            )),
        },
        Kind::Number => exact_decimal(&value).map(Value::number).ok_or_else(|| {
            FormulaError::new(
                "NotExact",
                format!(
                    "This works out to {value}, which has no exact decimal of at most 28 places. \
                     Round it, such as round(x, 0.001)."
                ),
            )
        }),
        Kind::Slope => {
            let rise = to_i64(value.numerator(), kind)?;
            let run = i64::try_from(value.denominator()).map_err(|_| too_large(kind))?;
            Slope::new(rise, run)
                .map(Value::Slope)
                .ok_or_else(|| too_large(kind))
        }
        _ => Err(FormulaError::new(
            "Type",
            format!("No formula gives {kind}: a formula works with numbers and yes or no."),
        )),
    }
}

/// One value of `kind`, from text: a number in the kind's own unit (millimetres for a length,
/// degrees for an angle), a parameter's name, or a formula. `lookup` gives each name's value. The
/// formula is type-checked first, from the kinds of the values `lookup` gives, then worked out
/// exactly and landed on `kind`.
///
/// # Errors
///
/// Why it can't be read, type-checked, worked out or landed, as a [`FormulaError`].
pub fn value_of(
    text: &str,
    kind: Kind,
    lookup: &mut dyn FnMut(&str) -> Result<Value, FormulaError>,
) -> Result<Value, FormulaError> {
    let e = parse(text)?;
    types::check_as(&e, kind, &mut |name| lookup(name).map(|v| v.kind()))?;
    eval_as(&e, kind, lookup)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(name: &str) -> Result<Value, FormulaError> {
        Err(FormulaError::new("UnknownName", name.to_owned()))
    }

    fn steps(text: &str) -> i64 {
        match value_of(text, Kind::Length, &mut none).unwrap() {
            Value::Length(l) => l.steps(),
            other => panic!("{other:?}"),
        }
    }

    /// Old answers in millimetres, times 6,400 steps: exact.
    #[test]
    fn units_convert_exactly() {
        assert_eq!(steps("2 ft 8 in"), 8128 * 640, "812.8 mm");
        assert_eq!(steps("1 m - 1 mm"), 999 * 6400);
        assert_eq!(steps("0.1 mm + 0.2 mm"), 3 * 640, "0.3 mm exactly");
        assert_eq!(steps("1/3 * 3 mm"), 6400);
        assert_eq!(steps("120"), 120 * 6400);
        assert_eq!(steps("-8.5"), -85 * 640);
        assert_eq!(steps("2.5 cm"), 25 * 6400);
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
            value_of("10 mm / 2 mm", Kind::Number, &mut none),
            Ok(Value::number(Decimal::from(5)))
        );
        assert_eq!(
            value_of("30 deg + 5 mm", Kind::Angle, &mut none)
                .unwrap_err()
                .code,
            "Units"
        );
        assert_eq!(
            value_of("5 mm * 2 mm", Kind::Area, &mut none),
            Ok(Value::Area(Area::from_mm2(10)))
        );
        assert_eq!(
            value_of("2 m2 * 3 mm", Kind::Volume, &mut none),
            Ok(Value::Volume(Volume::from_mm3(6_000_000)))
        );
    }

    #[test]
    fn choices_functions_and_rounding() {
        assert_eq!(steps("3 > 2 ? 4 mm : 5 mm"), 4 * 6400);
        assert_eq!(steps("max(2 mm, 9 mm, 3 mm)"), 9 * 6400);
        assert_eq!(steps("round(12.5 mm, 1 mm)"), 13 * 6400);
        assert_eq!(steps("round(-12.5 mm, 1 mm)"), -13 * 6400);
        assert_eq!(steps("floor(12.9 mm, 0.5 mm)"), 125 * 640);
        assert_eq!(steps("abs(-3 mm)"), 3 * 6400);
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
        assert_eq!(
            value_of("2 > 1 && !(1 > 2)", Kind::YesNo, &mut none),
            Ok(Value::YesNo(true))
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

    #[test]
    fn a_length_off_the_grid_is_refused_naming_the_nearest() {
        let e = value_of("1000 mm / 3", Kind::Length, &mut none).unwrap_err();
        assert_eq!(e.code, "OffGrid");
        assert_eq!(
            e.nearest,
            Some((
                Value::Length(Length::from_steps(2_133_333)),
                Value::Length(Length::from_steps(2_133_334)),
            ))
        );
        assert!(
            e.message
                .contains("the nearest are 333.33328125 mm and 333.3334375 mm"),
            "{}",
            e.message
        );
        assert_eq!(steps("round(1000 mm / 3, 0.01 mm)"), 2_133_312);
        let e = value_of("1 mm * 1 mm / 3", Kind::Area, &mut none).unwrap_err();
        assert_eq!(e.code, "OffGrid");
    }

    #[test]
    fn other_kinds_land_by_their_own_rule() {
        // An angle rounds half to even to the kernel's unit, as typed text does.
        assert_eq!(
            value_of("1 deg / 18000000", Kind::Angle, &mut none),
            Ok(Value::Angle(Angle::from_units(0)))
        );
        assert_eq!(
            value_of("3 deg / 18000000", Kind::Angle, &mut none),
            Ok(Value::Angle(Angle::from_units(2)))
        );
        assert_eq!(
            value_of("90", Kind::Angle, &mut none),
            Ok(Value::Angle(Angle::from_units(810_000_000)))
        );
        assert_eq!(
            value_of("7 / 4", Kind::Number, &mut none),
            Ok(Value::number(Decimal::new(175, 2)))
        );
        assert_eq!(
            value_of("10 / 3", Kind::Number, &mut none)
                .unwrap_err()
                .code,
            "NotExact"
        );
        assert_eq!(
            value_of("7 / 2", Kind::Whole, &mut none).unwrap_err().code,
            "NotWhole"
        );
        assert_eq!(
            value_of("5 mm / 200 mm", Kind::Slope, &mut none),
            Ok(Value::Slope(Slope::new(1, 40).unwrap()))
        );
    }
}
