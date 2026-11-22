//! One parameter value, its stored text, and the arithmetic a schedule needs.
//!
//! A stored value is `<kind>:<payload>` (FORMAT.md beside this crate, version 2):
//! `length:q0:15360000` (2,400 mm in grid steps), `slope:1/40`, `link:https://example.org/`. Each
//! value has exactly one spelling, so the same values always give the same bytes, and reading
//! checks that writing again gives the same text. A length, area or volume written by version 1,
//! in whole millimetres with no `q0:` (`length:2400`), is still read, exactly, as those
//! millimetres; writing it again gives version 2's spelling.

use crate::definition::Kind;
use crate::error::{Op, ParamError};
use crate::quantity::{Angle, Area, Length, Slope, Volume};
use kit_core::Id;
use rust_decimal::Decimal;
use std::str::FromStr;

/// A parameter value. Every number in it is exact: whole stored units, a ratio of whole numbers,
/// or a decimal. There is no float.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    /// A length, in whole grid steps of the geometry kernel (1/6,400 mm).
    Length(Length),
    /// An area, in whole square grid steps.
    Area(Area),
    /// A volume, in whole cubic grid steps.
    Volume(Volume),
    /// An angle, in whole angle units.
    Angle(Angle),
    /// A slope, as a ratio.
    Slope(Slope),
    /// A number with no unit, exact. Make one with [`Value::number`], which writes it in its
    /// one spelling.
    Number(Decimal),
    /// A whole number with no unit.
    Whole(i64),
    /// Text, kept exactly as given.
    Text(String),
    /// Yes or no.
    YesNo(bool),
    /// A choice, by the key its definition lists.
    Choice(String),
    /// A link to another element.
    Element(Id),
    /// A material.
    Material(Id),
    /// An http or https web address, in the one spelling the WHATWG URL standard gives it. Make
    /// one with [`Value::link`].
    Link(String),
}

impl Value {
    /// An exact number, without trailing zeros and with zero unsigned, so `2.50` and `2.5` are
    /// one value.
    #[must_use]
    pub fn number(number: Decimal) -> Self {
        let number = number.normalize();
        Self::Number(if number.is_zero() {
            Decimal::ZERO
        } else {
            number
        })
    }

    /// A web link from text: an http or https address, written as the URL standard writes it.
    /// `None` for anything else, or for the rare address whose written form reads as another.
    #[must_use]
    pub fn link(text: &str) -> Option<Self> {
        let url = url::Url::parse(text).ok()?;
        if !matches!(url.scheme(), "http" | "https") {
            return None;
        }
        let again = url::Url::parse(url.as_str()).ok()?;
        (again.as_str() == url.as_str()).then(|| Self::Link(url.as_str().to_owned()))
    }

    /// The value's kind.
    #[must_use]
    pub const fn kind(&self) -> Kind {
        match self {
            Self::Length(_) => Kind::Length,
            Self::Area(_) => Kind::Area,
            Self::Volume(_) => Kind::Volume,
            Self::Angle(_) => Kind::Angle,
            Self::Slope(_) => Kind::Slope,
            Self::Number(_) => Kind::Number,
            Self::Whole(_) => Kind::Whole,
            Self::Text(_) => Kind::Text,
            Self::YesNo(_) => Kind::YesNo,
            Self::Choice(_) => Kind::Choice,
            Self::Element(_) => Kind::Element,
            Self::Material(_) => Kind::Material,
            Self::Link(_) => Kind::Link,
        }
    }

    /// The stored text: the kind's tag, a colon, and the payload in its one spelling.
    #[must_use]
    pub fn stored(&self) -> String {
        let payload = match self {
            Self::Length(v) => format!("{GRID}{}", v.steps()),
            Self::Area(v) => format!("{GRID}{}", v.square_steps()),
            Self::Volume(v) => format!("{GRID}{}", v.cubic_steps()),
            Self::Angle(v) => v.units().to_string(),
            Self::Slope(v) => format!("{}/{}", v.rise(), v.run()),
            Self::Number(v) => v.to_string(),
            Self::Whole(v) => v.to_string(),
            Self::Text(v) | Self::Choice(v) | Self::Link(v) => v.clone(),
            Self::YesNo(v) => (if *v { "yes" } else { "no" }).to_owned(),
            Self::Element(v) | Self::Material(v) => v.as_str().to_owned(),
        };
        format!("{}:{payload}", self.kind().tag())
    }

    /// Reads stored text. `None` unless it is a value in its one spelling: writing the value read
    /// gives exactly `text` back. The one exception is a length, area or volume in version 1's
    /// spelling, whole millimetres with no `q0:` (`length:2400`, `area:5760000`, `volume:1`),
    /// itself in its one spelling: it reads as exactly those millimetres, and writing it gives
    /// version 2's text. A version 1 value too large for grid steps is `None`.
    #[must_use]
    pub fn from_stored(text: &str) -> Option<Self> {
        let (tag, payload) = text.split_once(':')?;
        let kind = Kind::from_tag(tag)?;
        if matches!(kind, Kind::Length | Kind::Area | Kind::Volume) && !payload.starts_with(GRID) {
            return from_version_1(kind, payload);
        }
        let value = match kind {
            Kind::Length => Self::Length(Length::from_steps(on_grid(payload)?.parse().ok()?)),
            Kind::Area => Self::Area(Area::from_square_steps(on_grid(payload)?.parse().ok()?)),
            Kind::Volume => Self::Volume(Volume::from_cubic_steps(on_grid(payload)?.parse().ok()?)),
            Kind::Angle => Self::Angle(Angle::from_units(payload.parse().ok()?)),
            Kind::Slope => {
                let (rise, run) = payload.split_once('/')?;
                Self::Slope(Slope::new(rise.parse().ok()?, run.parse().ok()?)?)
            }
            Kind::Number => Self::number(Decimal::from_str_exact(payload).ok()?),
            Kind::Whole => Self::Whole(payload.parse().ok()?),
            Kind::Text => Self::Text(payload.to_owned()),
            Kind::YesNo => match payload {
                "yes" => Self::YesNo(true),
                "no" => Self::YesNo(false),
                _ => return None,
            },
            Kind::Choice => {
                if payload.is_empty() {
                    return None;
                }
                Self::Choice(payload.to_owned())
            }
            Kind::Element => Self::Element(Id::from_str(payload).ok()?),
            Kind::Material => Self::Material(Id::from_str(payload).ok()?),
            Kind::Link => Self::link(payload)?,
        };
        (value.stored() == text).then_some(value)
    }

    /// The sum of two values of one measured kind, or of two numbers or whole numbers. Exact.
    ///
    /// # Errors
    /// [`ParamError::Mismatch`] naming both kinds if they differ or do not add;
    /// [`ParamError::Overflow`] if the sum does not fit.
    pub fn add(&self, other: &Self) -> Result<Self, ParamError> {
        let op = Op::Add;
        let answer = match (self, other) {
            (Self::Length(a), Self::Length(b)) => a.checked_add(*b).map(Self::Length),
            (Self::Area(a), Self::Area(b)) => a.checked_add(*b).map(Self::Area),
            (Self::Volume(a), Self::Volume(b)) => a.checked_add(*b).map(Self::Volume),
            (Self::Angle(a), Self::Angle(b)) => a.checked_add(*b).map(Self::Angle),
            (Self::Number(a), Self::Number(b)) => exact_sum(*a, *b, false).map(Self::number),
            (Self::Whole(a), Self::Whole(b)) => a.checked_add(*b).map(Self::Whole),
            _ => return Err(self.mismatch(op, other)),
        };
        answer.ok_or(ParamError::Overflow {
            op,
            kind: self.kind(),
        })
    }

    /// The difference of two values of one kind, under the same rules as [`Value::add`].
    ///
    /// # Errors
    /// As [`Value::add`].
    pub fn subtract(&self, other: &Self) -> Result<Self, ParamError> {
        let op = Op::Subtract;
        let answer = match (self, other) {
            (Self::Length(a), Self::Length(b)) => a.checked_sub(*b).map(Self::Length),
            (Self::Area(a), Self::Area(b)) => a.checked_sub(*b).map(Self::Area),
            (Self::Volume(a), Self::Volume(b)) => a.checked_sub(*b).map(Self::Volume),
            (Self::Angle(a), Self::Angle(b)) => a.checked_sub(*b).map(Self::Angle),
            (Self::Number(a), Self::Number(b)) => exact_sum(*a, *b, true).map(Self::number),
            (Self::Whole(a), Self::Whole(b)) => a.checked_sub(*b).map(Self::Whole),
            _ => return Err(self.mismatch(op, other)),
        };
        answer.ok_or(ParamError::Overflow {
            op,
            kind: self.kind(),
        })
    }

    /// The products a schedule needs, exact: a length times a length is an area, and an area
    /// times a length (either way round) is a volume. Formulas are their own crate.
    ///
    /// # Errors
    /// [`ParamError::Mismatch`] naming both kinds for any other pair;
    /// [`ParamError::Overflow`] if the product does not fit.
    pub fn multiply(&self, other: &Self) -> Result<Self, ParamError> {
        let op = Op::Multiply;
        let (answer, kind) = match (self, other) {
            (Self::Length(a), Self::Length(b)) => (a.checked_mul(*b).map(Self::Area), Kind::Area),
            (Self::Area(a), Self::Length(b)) | (Self::Length(b), Self::Area(a)) => {
                (a.checked_mul(*b).map(Self::Volume), Kind::Volume)
            }
            _ => return Err(self.mismatch(op, other)),
        };
        answer.ok_or(ParamError::Overflow { op, kind })
    }

    fn mismatch(&self, op: Op, other: &Self) -> ParamError {
        ParamError::Mismatch {
            op,
            left: self.kind(),
            right: other.kind(),
        }
    }
}

/// How a version 2 length, area or volume payload starts: the kernel's grid step its whole number
/// counts in (`Q0`, 1/6,400 mm). Version 1 payloads, in whole millimetres, never start with it.
const GRID: &str = "q0:";

/// The whole number after [`GRID`].
fn on_grid(payload: &str) -> Option<&str> {
    payload.strip_prefix(GRID)
}

/// A version 1 length, area or volume: whole millimetres, square millimetres or cubic
/// millimetres in their one spelling, read exactly into grid steps.
fn from_version_1(kind: Kind, payload: &str) -> Option<Value> {
    let mm: i64 = payload.parse().ok()?;
    if mm.to_string() != payload {
        return None;
    }
    match kind {
        Kind::Length => Length::checked_from_mm(mm).map(Value::Length),
        Kind::Area => Some(Value::Area(Area::from_mm2(mm))),
        Kind::Volume => Some(Value::Volume(Volume::from_mm3(mm))),
        _ => None,
    }
}

/// `a + b`, or `a - b` when `less`, exactly: both brought to the finer scale as whole numbers and
/// added in 128 bits. `None` if the answer does not fit a decimal, where the decimal type's own
/// sum would round to 28 digits instead.
fn exact_sum(a: Decimal, b: Decimal, less: bool) -> Option<Decimal> {
    let scale = a.scale().max(b.scale());
    let whole = |d: Decimal| {
        let up = 10_i128.checked_pow(scale.checked_sub(d.scale())?)?;
        d.mantissa().checked_mul(up)
    };
    let b = if less {
        whole(b)?.checked_neg()?
    } else {
        whole(b)?
    };
    Decimal::try_from_i128_with_scale(whole(a)?.checked_add(b)?, scale).ok()
}
