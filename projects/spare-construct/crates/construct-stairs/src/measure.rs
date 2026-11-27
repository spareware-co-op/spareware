//! What a stair reads back, worked out each time from its layout and never stored, as read-only
//! parameters a page shows, a schedule lists and a tag reads.
//!
//! With fixed IDs ([`read_only`]): [`RISE`] (base to top), [`RISERS`] (how many), [`RISER_HEIGHT`]
//! (the tallest riser: a rise that does not divide evenly leaves some one grid step lower, which
//! no display shows), [`TREADS`], [`GOING`], [`RUN_LENGTH`] (the total run: the walking line in
//! plan, treads and landings), [`PITCH`] (the tallest riser over the going, a slope) and
//! [`WIDTH`].

use crate::error::StairError;
use crate::layout::Layout;
use construct_params::{Definition, Holder, Kind, Value};
use kit_core::Id;

/// The read-only parameter of a stair's rise from its base to its top.
pub const RISE: &str = "stair.rise";
/// The read-only parameter of how many risers a stair has.
pub const RISERS: &str = "stair.risers";
/// The read-only parameter of a stair's tallest riser.
pub const RISER_HEIGHT: &str = "stair.riser-height";
/// The read-only parameter of how many treads a stair has.
pub const TREADS: &str = "stair.treads";
/// The read-only parameter of a stair's going.
pub const GOING: &str = "stair.going";
/// The read-only parameter of a stair's total run.
pub const RUN_LENGTH: &str = "stair.run-length";
/// The read-only parameter of a stair's pitch.
pub const PITCH: &str = "stair.pitch";
/// The read-only parameter of a stair's width.
pub const WIDTH: &str = "stair.width";

/// A read-only parameter's definition with one of this crate's fixed IDs.
fn define(id: &str, name: &str, kind: Kind) -> Result<Definition, StairError> {
    let id = Id::parse(id).map_err(|_| StairError::Malformed(format!("{id:?} is not an ID")))?;
    Definition::new(id, name, kind, Holder::Instance)
        .map_err(|e| StairError::Malformed(e.to_string()))
}

/// A count as a whole number.
fn whole(n: usize) -> Result<Value, StairError> {
    i64::try_from(n)
        .map(Value::Whole)
        .map_err(|_| StairError::OutOfRange {
            what: "a count".to_owned(),
        })
}

/// A stair's read-only parameters with their values, in the module header's order.
///
/// # Errors
/// [`StairError::OutOfRange`] for a sum that does not fit, which a stair's limits rule out;
/// [`StairError::Malformed`] never, for these fixed definitions.
pub fn read_only(layout: &Layout) -> Result<Vec<(Definition, Option<Value>)>, StairError> {
    Ok(vec![
        (
            define(RISE, "Rise", Kind::Length)?,
            Some(Value::Length(layout.rise)),
        ),
        (
            define(RISERS, "Risers", Kind::Whole)?,
            Some(whole(layout.riser_count())?),
        ),
        (
            define(RISER_HEIGHT, "Riser height", Kind::Length)?,
            Some(Value::Length(layout.tallest())),
        ),
        (
            define(TREADS, "Treads", Kind::Whole)?,
            Some(whole(layout.tread_count())?),
        ),
        (
            define(GOING, "Going", Kind::Length)?,
            Some(Value::Length(layout.going)),
        ),
        (
            define(RUN_LENGTH, "Total run", Kind::Length)?,
            Some(Value::Length(layout.total_run()?)),
        ),
        (
            define(PITCH, "Pitch", Kind::Slope)?,
            layout.pitch().map(Value::Slope),
        ),
        (
            define(WIDTH, "Width", Kind::Length)?,
            Some(Value::Length(layout.width)),
        ),
    ])
}
