//! Reading what people type and showing values. All unit arithmetic is in whole numbers, in 128
//! bits, with one rounding rule: half to even, as the geometry kernel rounds onto its grid
//! (geometry architecture chapters 6 and 17). A typed length, area or volume is never rounded: it
//! is a whole number of grid steps (square or cubic steps) exactly, or it is refused naming the
//! nearest two that are. A typed angle rounds to the angle unit. A shown value rounds to its
//! format's places and prints exactly that many.

use crate::definition::{Definition, Kind};
use crate::error::ParamError;
use crate::quantity::{Angle, Area, Length, Slope, Volume, exact_decimal};
use crate::units::{Format, Unit, Units};
use crate::value::Value;
use kit_core::Id;
use num_rational::Ratio;
use rust_decimal::Decimal;
use std::cmp::Ordering;

/// The unit symbols read, as the SI writes them: case-sensitive, so `MM` is not millimetres. A
/// unit for another system (feet, inches) is a row here and a [`Unit`]; stored values never move.
const SYMBOLS: [(&str, Unit); 13] = [
    ("mm", Unit::Millimetre),
    ("m", Unit::Metre),
    ("mm2", Unit::SquareMillimetre),
    ("mm²", Unit::SquareMillimetre),
    ("m2", Unit::SquareMetre),
    ("m²", Unit::SquareMetre),
    ("mm3", Unit::CubicMillimetre),
    ("mm³", Unit::CubicMillimetre),
    ("m3", Unit::CubicMetre),
    ("m³", Unit::CubicMetre),
    ("°", Unit::Degree),
    ("deg", Unit::Degree),
    ("%", Unit::Percent),
];

/// Reads `typed` as a value of `def`'s kind. The caller checks the value against `def`.
pub(crate) fn parse(def: &Definition, typed: &str, units: &Units) -> Result<Value, ParamError> {
    let parameter = || def.name().to_owned();
    let text = typed.trim();
    match def.kind() {
        Kind::Text => Ok(Value::Text(typed.to_owned())),
        Kind::YesNo => {
            if text.eq_ignore_ascii_case("yes") {
                Ok(Value::YesNo(true))
            } else if text.eq_ignore_ascii_case("no") {
                Ok(Value::YesNo(false))
            } else {
                Err(ParamError::NotYesOrNo {
                    parameter: parameter(),
                    text: text.to_owned(),
                })
            }
        }
        Kind::Choice => def
            .choices()
            .iter()
            .find(|choice| choice.label == text)
            .or_else(|| def.choices().iter().find(|choice| choice.key == text))
            .map(|choice| Value::Choice(choice.key.clone()))
            .ok_or_else(|| ParamError::NotAChoice {
                parameter: parameter(),
                text: text.to_owned(),
            }),
        Kind::Element | Kind::Material => {
            let id = Id::parse(text).map_err(|_| ParamError::BadId {
                parameter: parameter(),
                text: text.to_owned(),
            })?;
            Ok(if def.kind() == Kind::Element {
                Value::Element(id)
            } else {
                Value::Material(id)
            })
        }
        Kind::Link => Value::link(text).ok_or_else(|| ParamError::BadLink {
            parameter: parameter(),
            text: text.to_owned(),
        }),
        Kind::Length
        | Kind::Area
        | Kind::Volume
        | Kind::Angle
        | Kind::Slope
        | Kind::Number
        | Kind::Whole => numeric(def, text, units),
    }
}

/// Reads a number, with a unit or a ratio where the kind has one.
fn numeric(def: &Definition, text: &str, units: &Units) -> Result<Value, ParamError> {
    let kind = def.kind();
    let out_of_range = || ParamError::OutOfRange {
        parameter: def.name().to_owned(),
        text: text.to_owned(),
    };
    if let Some((left, right)) = text.split_once(':') {
        if kind != Kind::Slope {
            return Err(ParamError::WrongUnit {
                parameter: def.name().to_owned(),
                expected: kind,
                unit: ":".to_owned(),
                found: Kind::Slope,
            });
        }
        let rise = decimal(def, left.trim(), text)?;
        let run = decimal(def, right.trim(), text)?;
        let (rise, run) = ratio_of(rise, run).ok_or_else(out_of_range)?;
        return slope(rise, run).ok_or_else(out_of_range);
    }
    let (number, symbol) = split(text);
    let number = decimal(def, number, text)?;
    let unit = if symbol.is_empty() {
        None
    } else {
        let unit = SYMBOLS
            .iter()
            .find(|(written, _)| *written == symbol)
            .map(|(_, unit)| *unit)
            .ok_or_else(|| ParamError::UnknownUnit {
                parameter: def.name().to_owned(),
                unit: symbol.to_owned(),
            })?;
        if unit.kind() != kind {
            return Err(ParamError::WrongUnit {
                parameter: def.name().to_owned(),
                expected: kind,
                unit: symbol.to_owned(),
                found: unit.kind(),
            });
        }
        Some(unit)
    };
    if kind == Kind::Number {
        return Ok(Value::number(number));
    }
    if kind == Kind::Whole {
        if !number.fract().is_zero() {
            return Err(ParamError::NotWhole {
                parameter: def.name().to_owned(),
                text: text.to_owned(),
            });
        }
        let whole = to_whole(
            number.mantissa(),
            1,
            ten_to(number.scale()).ok_or_else(out_of_range)?,
        )
        .ok_or_else(out_of_range)?;
        return Ok(Value::Whole(whole));
    }
    let Some(Format { unit: shown, .. }) = units.format(kind) else {
        return Err(out_of_range());
    };
    let unit = unit.unwrap_or(shown);
    let mantissa = number.mantissa();
    let scale = ten_to(number.scale()).ok_or_else(out_of_range)?;
    match unit {
        Unit::Percent => {
            let run = scale.checked_mul(100).ok_or_else(out_of_range)?;
            slope(mantissa, run).ok_or_else(out_of_range)
        }
        // A bare number in ratio form is the run for a rise of one.
        Unit::Ratio => slope(scale, mantissa).ok_or_else(out_of_range),
        _ => {
            let (num, den) = unit.factor().ok_or_else(out_of_range)?;
            let top = mantissa.checked_mul(num).ok_or_else(out_of_range)?;
            let bottom = den.checked_mul(scale).ok_or_else(out_of_range)?;
            if kind == Kind::Angle {
                let units = to_whole(top, 1, bottom).ok_or_else(out_of_range)?;
                return Ok(Value::Angle(Angle::from_units(units)));
            }
            let base = on_grid(def, text, unit, top, bottom)?;
            Ok(match kind {
                Kind::Length => Value::Length(Length::from_steps(
                    i64::try_from(base).map_err(|_| out_of_range())?,
                )),
                Kind::Area => Value::Area(Area::from_square_steps(base)),
                _ => Value::Volume(Volume::from_cubic_steps(base)),
            })
        }
    }
}

/// `top / bottom` stored units (grid steps, square or cubic steps) if it is a whole number of them
/// that fits, typed in `unit`; otherwise [`ParamError::OffGrid`] naming the whole numbers either
/// side, shown exactly in `unit`, or [`ParamError::OutOfRange`].
fn on_grid(
    def: &Definition,
    text: &str,
    unit: Unit,
    top: i128,
    bottom: i128,
) -> Result<i128, ParamError> {
    let out_of_range = || ParamError::OutOfRange {
        parameter: def.name().to_owned(),
        text: text.to_owned(),
    };
    let below = top.checked_div_euclid(bottom).ok_or_else(out_of_range)?;
    let rest = top.checked_rem_euclid(bottom).ok_or_else(out_of_range)?;
    let fits = |v: i128| def.kind() != Kind::Length || i64::try_from(v).is_ok();
    if rest == 0 {
        return if fits(below) {
            Ok(below)
        } else {
            Err(out_of_range())
        };
    }
    let above = below.checked_add(1).ok_or_else(out_of_range)?;
    if !fits(below) || !fits(above) {
        return Err(out_of_range());
    }
    let (num, den) = unit.factor().ok_or_else(out_of_range)?;
    let show = |base: i128| -> Result<String, ParamError> {
        let n = base.checked_mul(den).ok_or_else(out_of_range)?;
        let number = exact_decimal(n, num).ok_or_else(out_of_range)?;
        Ok(format!("{number}{}", unit.suffix()))
    };
    Err(ParamError::OffGrid {
        parameter: def.name().to_owned(),
        text: text.to_owned(),
        below: show(below)?,
        above: show(above)?,
    })
}

/// Splits typed text into its number (digits and points, after an optional sign) and the rest.
fn split(text: &str) -> (&str, &str) {
    let end = text
        .char_indices()
        .find(|&(at, c)| !(c.is_ascii_digit() || c == '.' || (at == 0 && matches!(c, '+' | '-'))))
        .map_or(text.len(), |(at, _)| at);
    text.split_at_checked(end)
        .map_or((text, ""), |(number, rest)| (number, rest.trim()))
}

/// Reads a decimal number exactly; `whole` is the text to name if it is not one.
fn decimal(def: &Definition, number: &str, whole: &str) -> Result<Decimal, ParamError> {
    let number = number.strip_prefix('+').unwrap_or(number);
    let digits = number.strip_prefix('-').unwrap_or(number);
    let not_a_number = || ParamError::NotANumber {
        parameter: def.name().to_owned(),
        text: whole.to_owned(),
    };
    // The decimal type reads more spellings than people type (underscores, an exponent): only
    // digits and one point are taken.
    if !digits.chars().any(|c| c.is_ascii_digit())
        || !digits.chars().all(|c| c.is_ascii_digit() || c == '.')
        || digits.matches('.').count() > 1
    {
        return Err(not_a_number());
    }
    Decimal::from_str_exact(number).map_err(|_| not_a_number())
}

/// 10 to the `power`, if it fits.
fn ten_to(power: u32) -> Option<i128> {
    10_i128.checked_pow(power)
}

/// The rise and run of the ratio `rise:run` of two decimals, as whole numbers.
fn ratio_of(rise: Decimal, run: Decimal) -> Option<(i128, i128)> {
    let rise_whole = rise.mantissa().checked_mul(ten_to(run.scale())?)?;
    let run_whole = run.mantissa().checked_mul(ten_to(rise.scale())?)?;
    Some((rise_whole, run_whole))
}

/// The slope `rise` over `run` in lowest terms, if the run is not zero and the terms fit.
fn slope(rise: i128, run: i128) -> Option<Value> {
    if run == 0 || rise == i128::MIN || run == i128::MIN {
        return None;
    }
    // Neither side is i128::MIN and the run is not zero, so reducing neither divides by zero nor
    // overflows.
    let reduced = Ratio::new(rise, run);
    let rise = i64::try_from(*reduced.numer()).ok()?;
    let run = i64::try_from(*reduced.denom()).ok()?;
    Slope::new(rise, run).map(Value::Slope)
}

/// `n * scale / d` rounded half to even, as an `i64`, if it fits; `d` above zero.
fn to_whole(n: i128, scale: i128, d: i128) -> Option<i64> {
    i64::try_from(round_half_even(n.checked_mul(scale)?, d)?).ok()
}

/// `n / d` rounded to the nearest whole number, a half to the even one; `d` above zero.
fn round_half_even(n: i128, d: i128) -> Option<i128> {
    let quotient = n.checked_div_euclid(d)?;
    let twice_rest = n.checked_rem_euclid(d)?.checked_mul(2)?;
    let up = match twice_rest.cmp(&d) {
        Ordering::Less => false,
        Ordering::Greater => true,
        Ordering::Equal => quotient.checked_rem_euclid(2)? == 1,
    };
    if up {
        quotient.checked_add(1)
    } else {
        Some(quotient)
    }
}

/// `n / d` shown to `places` decimal places, rounded half to even; `d` above zero.
fn shown(n: i128, d: i128, places: u8) -> Option<String> {
    let places = u32::from(places);
    let scaled = round_half_even(n.checked_mul(ten_to(places)?)?, d)?;
    Decimal::try_from_i128_with_scale(scaled, places)
        .ok()
        .map(|decimal| decimal.to_string())
}

/// Shows a value of `def`'s kind. The caller has checked the value against `def`.
pub(crate) fn format(def: &Definition, value: &Value, units: &Units) -> Result<String, ParamError> {
    let out_of_range = || ParamError::OutOfRange {
        parameter: def.name().to_owned(),
        text: value.stored(),
    };
    let measured = |base: i128| -> Result<String, ParamError> {
        let format = units.format(value.kind()).ok_or_else(out_of_range)?;
        let (num, den) = format.unit.factor().ok_or_else(out_of_range)?;
        let top = base.checked_mul(den).ok_or_else(out_of_range)?;
        let number = shown(top, num, format.places).ok_or_else(out_of_range)?;
        Ok(format!("{number}{}", format.unit.suffix()))
    };
    match value {
        Value::Length(v) => measured(i128::from(v.steps())),
        Value::Area(v) => measured(v.square_steps()),
        Value::Volume(v) => measured(v.cubic_steps()),
        Value::Angle(v) => measured(i128::from(v.units())),
        Value::Slope(v) => {
            let format = units.format(Kind::Slope).ok_or_else(out_of_range)?;
            show_slope(*v, format).ok_or_else(out_of_range)
        }
        Value::Number(v) => Ok(v.to_string()),
        Value::Whole(v) => Ok(v.to_string()),
        Value::Text(v) | Value::Link(v) => Ok(v.clone()),
        Value::YesNo(v) => Ok((if *v { "Yes" } else { "No" }).to_owned()),
        Value::Choice(key) => Ok(def
            .choices()
            .iter()
            .find(|choice| &choice.key == key)
            .map_or_else(|| key.clone(), |choice| choice.label.clone())),
        Value::Element(v) | Value::Material(v) => Ok(v.as_str().to_owned()),
    }
}

/// A slope as a percentage, or as a ratio: `1:n` when it runs at least as far as it rises, `n:1`
/// when it rises further, so the side shown as one never rounds to nothing. A steep slope whose
/// `n` rounds to one is shown `1:1`, as the one form that text reads back to.
fn show_slope(slope: Slope, format: Format) -> Option<String> {
    let rise = i128::from(slope.rise());
    let run = i128::from(slope.run());
    if format.unit == Unit::Percent {
        let number = shown(rise.checked_mul(100)?, run, format.places)?;
        return Some(format!("{number}{}", format.unit.suffix()));
    }
    if rise == 0 {
        return Some("0:1".to_owned());
    }
    let sign = if rise < 0 { "-" } else { "" };
    let rise = rise.checked_abs()?;
    if rise <= run {
        return Some(format!("{sign}1:{}", shown(run, rise, format.places)?));
    }
    let steep = shown(rise, run, format.places)?;
    Some(if steep == shown(1, 1, format.places)? {
        format!("{sign}1:{steep}")
    } else {
        format!("{sign}{steep}:1")
    })
}
