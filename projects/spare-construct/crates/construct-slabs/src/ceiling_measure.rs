//! What a ceiling reads back, worked out each time from its boundary, its type and its grid and
//! never stored: the quantities of a floor of the same layers ([`crate::quantities`]) under a
//! ceiling's own parameter IDs, and its tiles.
//!
//! The read-only parameters ([`ceiling_read_only`]): [`AREA`], [`PERIMETER`], [`THICKNESS`],
//! [`VOLUME`] and [`MASS`] (kilograms, three places), and, for a ceiling with a grid whose tiles
//! can be counted ([`crate::Grid::tiles`]), [`TILES`] (every tile with any part in the ceiling)
//! and [`TILES_CUT`] (those the outline cuts). A ceiling's area is the one a schedule lists.

use crate::error::SlabError;
use crate::measure::Quantities;
use crate::tiles::Tiles;
use construct_params::{Decimal, Definition, Holder, Kind, Value};
use kit_core::Id;

/// The read-only parameter of a ceiling's area.
pub const AREA: &str = "ceiling.area";
/// The read-only parameter of a ceiling's perimeter, its openings' edges included.
pub const PERIMETER: &str = "ceiling.perimeter";
/// The read-only parameter of a ceiling's total thickness, its type's.
pub const THICKNESS: &str = "ceiling.thickness";
/// The read-only parameter of a ceiling's volume.
pub const VOLUME: &str = "ceiling.volume";
/// The read-only parameter of a ceiling's mass, in kilograms.
pub const MASS: &str = "ceiling.mass";
/// The read-only parameter of the number of tiles of a ceiling's grid that lie in it.
pub const TILES: &str = "ceiling.tiles";
/// The read-only parameter of the number of those tiles that are cut.
pub const TILES_CUT: &str = "ceiling.tiles-cut";

fn count(n: u64) -> Result<Value, SlabError> {
    Ok(Value::number(Decimal::new(
        i64::try_from(n).map_err(|_| SlabError::OutOfRange)?,
        0,
    )))
}

/// A ceiling's read-only parameters, each with its value, in the order of the module header.
/// Mass has no value when it is not known; the two tile counts are there only when `tiles` is.
///
/// # Errors
/// [`SlabError::OutOfRange`] for a mass or a count beyond a decimal; [`SlabError::Malformed`]
/// never, for these fixed definitions.
pub fn ceiling_read_only(
    q: &Quantities,
    tiles: Option<Tiles>,
) -> Result<Vec<(Definition, Option<Value>)>, SlabError> {
    let define = |id: &str, name: &str, kind: Kind| {
        let id = Id::parse(id).map_err(|_| SlabError::Malformed(format!("{id:?} is not an ID")))?;
        Definition::new(id, name, kind, Holder::Instance)
            .map_err(|e| SlabError::Malformed(e.to_string()))
    };
    let kilograms = match q.mass_grams {
        Some(grams) => Some(Value::number(Decimal::new(
            i64::try_from(grams).map_err(|_| SlabError::OutOfRange)?,
            3,
        ))),
        None => None,
    };
    let mut rows = vec![
        (define(AREA, "Area", Kind::Area)?, Some(Value::Area(q.area))),
        (
            define(PERIMETER, "Perimeter", Kind::Length)?,
            Some(Value::Length(q.perimeter)),
        ),
        (
            define(THICKNESS, "Thickness", Kind::Length)?,
            Some(Value::Length(q.thickness)),
        ),
        (
            define(VOLUME, "Volume", Kind::Volume)?,
            Some(Value::Volume(q.volume)),
        ),
        (define(MASS, "Mass (kg)", Kind::Number)?, kilograms),
    ];
    if let Some(tiles) = tiles {
        rows.push((
            define(TILES, "Tiles", Kind::Number)?,
            Some(count(tiles.total())?),
        ));
        rows.push((
            define(TILES_CUT, "Tiles cut", Kind::Number)?,
            Some(count(tiles.cut)?),
        ));
    }
    Ok(rows)
}
