//! Filters, and how two values are ordered.
//!
//! A filter is a field, a comparison and a value. A row is listed only when every filter holds.
//! A row that has no value for the field (a parameter nobody set, a room that is not enclosed)
//! fails every filter on it, whatever the comparison: nothing can be said of what is not there.
//!
//! Numbers (lengths, areas, volumes, angles, slopes, decimals, whole numbers) are compared by
//! their exact value, never through a float. Text is compared by one stated rule, [`text_order`]:
//! lower-cased with Unicode's own mapping, then, to keep two spellings apart, by the exact text.
//! `equals` and `not_equals` on text are exact; `contains` and `begins_with` ignore case.

use crate::error::ScheduleError;
use crate::field::Field;
use construct_params::{Definition, Kind, Value};
use std::cmp::Ordering;

/// How a filter compares a row's value (left) with the filter's value (right).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comparison {
    /// The same value.
    Equals,
    /// Not the same value.
    NotEquals,
    /// Below.
    Less,
    /// Below or the same.
    LessOrEqual,
    /// Above.
    Greater,
    /// Above or the same.
    GreaterOrEqual,
    /// Text that has the filter's text inside it, case aside.
    Contains,
    /// Text that begins with the filter's text, case aside.
    BeginsWith,
}

impl Comparison {
    /// Every comparison, in the order they are listed to a person.
    pub const ALL: [Self; 8] = [
        Self::Equals,
        Self::NotEquals,
        Self::Less,
        Self::LessOrEqual,
        Self::Greater,
        Self::GreaterOrEqual,
        Self::Contains,
        Self::BeginsWith,
    ];

    /// The comparison's one stored spelling.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Equals => "equals",
            Self::NotEquals => "not_equals",
            Self::Less => "less",
            Self::LessOrEqual => "less_or_equal",
            Self::Greater => "greater",
            Self::GreaterOrEqual => "greater_or_equal",
            Self::Contains => "contains",
            Self::BeginsWith => "begins_with",
        }
    }

    /// The comparison a key spells.
    ///
    /// # Errors
    /// [`ScheduleError::UnknownComparison`].
    pub fn parse(key: &str) -> Result<Self, ScheduleError> {
        Self::ALL
            .into_iter()
            .find(|c| c.key() == key)
            .ok_or_else(|| ScheduleError::UnknownComparison(key.to_owned()))
    }

    /// Whether a field holding values of this kind may be compared this way. Anything with an
    /// order (numbers, text) takes the six; text and links take the two text tests; a choice, a
    /// yes or no, an element link and a material only equals and not equals.
    #[must_use]
    pub fn fits(self, kind: Kind) -> bool {
        let equality = matches!(self, Self::Equals | Self::NotEquals);
        match kind {
            Kind::Length
            | Kind::Area
            | Kind::Volume
            | Kind::Angle
            | Kind::Slope
            | Kind::Number
            | Kind::Whole => !matches!(self, Self::Contains | Self::BeginsWith),
            Kind::Text => true,
            Kind::Link => equality || matches!(self, Self::Contains | Self::BeginsWith),
            Kind::YesNo | Kind::Choice | Kind::Element | Kind::Material => equality,
        }
    }
}

/// A condition on one field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    /// The field it tests.
    pub field: Field,
    /// How it compares.
    pub comparison: Comparison,
    /// What it compares with, of the field's kind.
    pub value: Value,
}

impl Filter {
    /// Checks the filter against the definition of its field: the value is of the field's kind
    /// and one its definition takes, and the comparison suits the kind.
    ///
    /// # Errors
    /// [`ScheduleError::FilterKind`], [`ScheduleError::ComparisonNotFor`] or
    /// [`ScheduleError::BadFilterValue`].
    pub fn check(&self, definition: &Definition) -> Result<(), ScheduleError> {
        let holds = definition.kind();
        let given = self.value.kind();
        if holds != given {
            return Err(ScheduleError::FilterKind {
                field: definition.name().to_owned(),
                holds,
                given,
            });
        }
        if !self.comparison.fits(holds) {
            return Err(ScheduleError::ComparisonNotFor {
                field: definition.name().to_owned(),
                comparison: self.comparison.key().to_owned(),
                holds,
            });
        }
        // Name, type and quantities have no choices to check; a parameter's definition does.
        definition
            .check(&self.value)
            .map_err(|e| ScheduleError::BadFilterValue {
                field: definition.name().to_owned(),
                why: e.to_string(),
            })
    }

    /// Whether a value passes this filter.
    #[must_use]
    pub fn holds(&self, value: &Value) -> bool {
        match self.comparison {
            Comparison::Contains | Comparison::BeginsWith => {
                let (Some(have), Some(want)) = (words(value), words(&self.value)) else {
                    return false;
                };
                let (have, want) = (have.to_lowercase(), want.to_lowercase());
                if self.comparison == Comparison::Contains {
                    have.contains(&want)
                } else {
                    have.starts_with(&want)
                }
            }
            Comparison::Equals => value == &self.value,
            Comparison::NotEquals => value != &self.value,
            Comparison::Less => order(value, &self.value) == Some(Ordering::Less),
            Comparison::LessOrEqual => matches!(
                order(value, &self.value),
                Some(Ordering::Less | Ordering::Equal)
            ),
            Comparison::Greater => order(value, &self.value) == Some(Ordering::Greater),
            Comparison::GreaterOrEqual => matches!(
                order(value, &self.value),
                Some(Ordering::Greater | Ordering::Equal)
            ),
        }
    }
}

/// The text of a text or link value.
fn words(value: &Value) -> Option<&str> {
    match value {
        Value::Text(text) | Value::Link(text) => Some(text),
        _ => None,
    }
}

/// The order of two pieces of text: lower-cased with Unicode's mapping, then the exact text. A
/// stated rule and no locale: the same on every machine and run.
#[must_use]
pub fn text_order(a: &str, b: &str) -> Ordering {
    a.to_lowercase()
        .cmp(&b.to_lowercase())
        .then_with(|| a.cmp(b))
}

/// The order of two values of one kind, by exact value; `None` for values of different kinds and
/// for kinds with no order of their own (a choice, an element link and a material are ordered by
/// their stored key, a yes or no with no before yes).
#[must_use]
pub fn order(a: &Value, b: &Value) -> Option<Ordering> {
    match (a, b) {
        (Value::Length(a), Value::Length(b)) => Some(a.cmp(b)),
        (Value::Area(a), Value::Area(b)) => Some(a.cmp(b)),
        (Value::Volume(a), Value::Volume(b)) => Some(a.cmp(b)),
        (Value::Angle(a), Value::Angle(b)) => Some(a.cmp(b)),
        (Value::Slope(a), Value::Slope(b)) => {
            // rise/run against rise/run, cross-multiplied in 128 bits: exact, no division.
            let left = i128::from(a.rise()).checked_mul(i128::from(b.run()))?;
            let right = i128::from(b.rise()).checked_mul(i128::from(a.run()))?;
            Some(left.cmp(&right))
        }
        (Value::Number(a), Value::Number(b)) => Some(a.cmp(b)),
        (Value::Whole(a), Value::Whole(b)) => Some(a.cmp(b)),
        (Value::Text(a), Value::Text(b)) => Some(text_order(a, b)),
        (Value::Link(a), Value::Link(b)) | (Value::Choice(a), Value::Choice(b)) => Some(a.cmp(b)),
        (Value::YesNo(a), Value::YesNo(b)) => Some(a.cmp(b)),
        (Value::Element(a), Value::Element(b)) | (Value::Material(a), Value::Material(b)) => {
            Some(a.cmp(b))
        }
        _ => None,
    }
}
