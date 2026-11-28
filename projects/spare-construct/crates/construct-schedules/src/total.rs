//! Totals: what a footer and the grand total work out of a column.
//!
//! A schedule names, for each column it wants totalled, which of four totals it shows: `sum`,
//! `count`, `min` and `max`. A sum is of lengths, areas, volumes, angles, numbers and whole
//! numbers, added exactly by construct-params (a length in grid steps, an area in square steps:
//! never a float, never rounded). A minimum and a maximum are of anything with a numeric order
//! (those and slopes). A count is of any field: the number of elements that have a value there.
//! A total of nothing (no element has a value) is empty, not zero: nothing can be said of what is
//! not there.

use crate::error::ScheduleError;
use crate::field::Field;
use crate::filter::order;
use construct_params::{Kind, Value};
use std::cmp::Ordering;

/// One kind of total.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TotalKind {
    /// The values added.
    Sum,
    /// How many elements have a value.
    Count,
    /// The least value.
    Min,
    /// The greatest value.
    Max,
}

impl TotalKind {
    /// Every kind of total, in the order they are shown and stored.
    pub const ALL: [Self; 4] = [Self::Sum, Self::Count, Self::Min, Self::Max];

    /// The total's one stored spelling.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Sum => "sum",
            Self::Count => "count",
            Self::Min => "min",
            Self::Max => "max",
        }
    }

    /// The total a key spells.
    ///
    /// # Errors
    /// [`ScheduleError::UnknownTotal`].
    pub fn parse(key: &str) -> Result<Self, ScheduleError> {
        Self::ALL
            .into_iter()
            .find(|total| total.key() == key)
            .ok_or_else(|| ScheduleError::UnknownTotal(key.to_owned()))
    }

    /// Whether a field holding this kind can be totalled this way.
    #[must_use]
    pub const fn fits(self, kind: Kind) -> bool {
        match self {
            Self::Count => true,
            Self::Sum => matches!(
                kind,
                Kind::Length | Kind::Area | Kind::Volume | Kind::Angle | Kind::Number | Kind::Whole
            ),
            Self::Min | Self::Max => matches!(
                kind,
                Kind::Length
                    | Kind::Area
                    | Kind::Volume
                    | Kind::Angle
                    | Kind::Slope
                    | Kind::Number
                    | Kind::Whole
            ),
        }
    }
}

/// The totals one column shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Totals {
    /// The column's field.
    pub field: Field,
    /// Which totals, in the order of [`TotalKind::ALL`] when stored; none repeated.
    pub kinds: Vec<TotalKind>,
}

/// The total of some values, exact: `None` when there is nothing to total (a count of nothing is
/// still `0`).
///
/// # Errors
/// [`ScheduleError::Malformed`] when the sum does not fit the stored number (a length of more than
/// the kernel's grid holds), in words; and when a value is of a kind the total does not take
/// (the caller has checked the field, so this is a record that lies).
pub fn aggregate<'a>(
    total: TotalKind,
    values: impl IntoIterator<Item = &'a Value>,
) -> Result<Option<Value>, ScheduleError> {
    let mut found: Option<Value> = None;
    let mut counted: i64 = 0;
    for value in values {
        counted = counted.saturating_add(1);
        found = match (total, found) {
            (TotalKind::Count, _) => None,
            (_, None) => Some(value.clone()),
            (TotalKind::Sum, Some(sum)) => Some(sum.add(value).map_err(|e| {
                ScheduleError::Malformed(format!("A sum does not fit the numbers held: {e}."))
            })?),
            (TotalKind::Min, Some(least)) => {
                Some(if order(value, &least) == Some(Ordering::Less) {
                    value.clone()
                } else {
                    least
                })
            }
            (TotalKind::Max, Some(most)) => {
                Some(if order(value, &most) == Some(Ordering::Greater) {
                    value.clone()
                } else {
                    most
                })
            }
        };
    }
    Ok(match total {
        TotalKind::Count => Some(Value::Whole(counted)),
        _ => found,
    })
}
