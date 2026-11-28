//! What a curtain wall reads back, worked out each time from its layout and never stored: its
//! length, height and area, how many panels it has and their area, its glazed area, and how many
//! mullions it has and their length; also as read-only parameters a page shows and a schedule
//! lists ([`read_only`]).
//!
//! Every figure is the layout's ([`crate::Grid`]), exactly: a panel's area is its own, between its
//! mullions' faces (a door panel's and an empty panel's count too), the glazed area only the
//! glazed panels', and a mullion's length its run along its line after the crossings. The
//! read-only parameters have fixed IDs: [`LENGTH`], [`HEIGHT`], [`AREA`], [`PANELS`],
//! [`PANEL_AREA`], [`GLAZED_AREA`], [`MULLIONS`] and [`MULLION_LENGTH`].

use crate::error::CurtainError;
use crate::layout::Grid;
use crate::length;
use crate::spec::PanelKind;
use construct_params::{Area, Decimal, Definition, Holder, Kind, Length, Value};
use kit_core::Id;

/// The read-only parameter of a curtain wall's length along its line.
pub const LENGTH: &str = "curtain.length";
/// The read-only parameter of a curtain wall's height.
pub const HEIGHT: &str = "curtain.height";
/// The read-only parameter of a curtain wall's area: its length by its height.
pub const AREA: &str = "curtain.area";
/// The read-only parameter of how many panels a curtain wall has.
pub const PANELS: &str = "curtain.panels";
/// The read-only parameter of its panels' area.
pub const PANEL_AREA: &str = "curtain.panel-area";
/// The read-only parameter of its glazed panels' area.
pub const GLAZED_AREA: &str = "curtain.glazed-area";
/// The read-only parameter of how many mullions it has.
pub const MULLIONS: &str = "curtain.mullions";
/// The read-only parameter of its mullions' length, all together.
pub const MULLION_LENGTH: &str = "curtain.mullion-length";

/// What a curtain wall reads back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quantities {
    /// Its length along its line.
    pub length: Length,
    /// Its height.
    pub height: Length,
    /// Its length by its height.
    pub area: Area,
    /// How many panels.
    pub panels: u64,
    /// Their area, each its own.
    pub panel_area: Area,
    /// The glazed panels' area.
    pub glazed_area: Area,
    /// How many mullions.
    pub mullions: u64,
    /// Their length, all together.
    pub mullion_length: Length,
}

/// What the curtain wall laid out as `grid` reads back.
///
/// # Errors
/// [`CurtainError::OutOfRange`] when a sum does not fit.
pub fn quantities(grid: &Grid) -> Result<Quantities, CurtainError> {
    let mut panel_area = Area::from_square_steps(0);
    let mut glazed_area = Area::from_square_steps(0);
    for panel in &grid.panels {
        let area = length::area(panel.width(), panel.height()).ok_or(CurtainError::OutOfRange)?;
        panel_area = panel_area
            .checked_add(area)
            .ok_or(CurtainError::OutOfRange)?;
        if panel.panel.kind() == PanelKind::Glazed {
            glazed_area = glazed_area
                .checked_add(area)
                .ok_or(CurtainError::OutOfRange)?;
        }
    }
    let mut mullion_length = Length::from_steps(0);
    for mullion in &grid.mullions {
        mullion_length =
            length::sum(mullion_length, mullion.length()).ok_or(CurtainError::OutOfRange)?;
    }
    Ok(Quantities {
        length: grid.length,
        height: grid.height,
        area: length::area(grid.length, grid.height).ok_or(CurtainError::OutOfRange)?,
        panels: u64::try_from(grid.panels.len()).map_err(|_| CurtainError::OutOfRange)?,
        panel_area,
        glazed_area,
        mullions: u64::try_from(grid.mullions.len()).map_err(|_| CurtainError::OutOfRange)?,
        mullion_length,
    })
}

/// A read-only parameter's definition with one of this crate's fixed IDs.
fn define(id: &str, name: &str, kind: Kind) -> Result<Definition, CurtainError> {
    let id = Id::parse(id).map_err(|_| CurtainError::Malformed(format!("{id:?} is not an ID")))?;
    Definition::new(id, name, kind, Holder::Instance)
        .map_err(|e| CurtainError::Malformed(e.to_string()))
}

/// A count as a number.
fn count(n: u64) -> Result<Value, CurtainError> {
    let n = i64::try_from(n).map_err(|_| CurtainError::OutOfRange)?;
    Ok(Value::number(Decimal::new(n, 0)))
}

/// A curtain wall's read-only parameters, each with its value, in the order of the module header.
///
/// # Errors
/// [`CurtainError::OutOfRange`] for a count beyond a decimal; [`CurtainError::Malformed`] never,
/// for these fixed definitions.
pub fn read_only(q: &Quantities) -> Result<Vec<(Definition, Option<Value>)>, CurtainError> {
    Ok(vec![
        (
            define(LENGTH, "Length", Kind::Length)?,
            Some(Value::Length(q.length)),
        ),
        (
            define(HEIGHT, "Height", Kind::Length)?,
            Some(Value::Length(q.height)),
        ),
        (define(AREA, "Area", Kind::Area)?, Some(Value::Area(q.area))),
        (
            define(PANELS, "Panels", Kind::Number)?,
            Some(count(q.panels)?),
        ),
        (
            define(PANEL_AREA, "Panel area", Kind::Area)?,
            Some(Value::Area(q.panel_area)),
        ),
        (
            define(GLAZED_AREA, "Glazed area", Kind::Area)?,
            Some(Value::Area(q.glazed_area)),
        ),
        (
            define(MULLIONS, "Mullions", Kind::Number)?,
            Some(count(q.mullions)?),
        ),
        (
            define(MULLION_LENGTH, "Mullion length", Kind::Length)?,
            Some(Value::Length(q.mullion_length)),
        ),
    ])
}
