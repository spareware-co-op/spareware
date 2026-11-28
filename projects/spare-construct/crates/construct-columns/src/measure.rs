//! What a column reads back, worked out each time from its section, its height and its type's
//! material, and never stored: height, area, volume and mass, and the same as read-only
//! parameters a page shows and a schedule lists.
//!
//! - **Height**: from the base to the top, construct-levels' one function
//!   ([`crate::height_between`]) or the unconnected height.
//! - **Area**: the section's, exact in whole square steps ([`crate::Section::area`]); a round
//!   section's is its polygon's, the interim of [`crate::section`].
//! - **Volume**: the area times the height ([`Area::checked_mul`]), exact in cubic steps.
//! - **Mass**: construct-materials' one function ([`Materials::mass_grams`]) on the volume of
//!   the type's material; known only when the library has the material.
//! - **Read-only parameters** ([`read_only`]): the same four as parameter definitions with fixed
//!   IDs ([`HEIGHT`], [`AREA`], [`VOLUME`], [`MASS`]), held by the instance and never stored.
//!   Mass is a number of kilograms (three places, so exact to the gram), as a floor's is.

use crate::column::check_height;
use crate::error::ColumnError;
use crate::stored::ColumnSpec;
use construct_materials::Materials;
use construct_params::{Area, Decimal, Definition, Holder, Kind, Length, Value, Volume};
use kit_core::Id;

/// The read-only parameter that holds a column's height.
pub const HEIGHT: &str = "column.height";
/// The read-only parameter that holds a column's section area.
pub const AREA: &str = "column.area";
/// The read-only parameter that holds a column's volume.
pub const VOLUME: &str = "column.volume";
/// The read-only parameter that holds a column's mass, in kilograms.
pub const MASS: &str = "column.mass";

/// What a column measures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quantities {
    /// From its base to its top.
    pub height: Length,
    /// Its section's area.
    pub area: Area,
    /// Its volume.
    pub volume: Volume,
    /// Its mass in grams, when its material is in the library.
    pub mass_grams: Option<u64>,
}

/// A column's quantities, from its type's `spec`, its `height` and the material `library`.
///
/// # Errors
/// [`ColumnError::NoHeight`], [`ColumnError::TopBelowBase`] or [`ColumnError::BadSize`] for a
/// height that is not above zero or is past the tallest; the section's refusals;
/// [`ColumnError::OutOfRange`] for a product that does not fit; the library's refusal of a mass.
pub fn quantities(
    spec: &ColumnSpec,
    height: Length,
    library: &Materials,
) -> Result<Quantities, ColumnError> {
    let height = check_height(height)?;
    let area = spec.section.area()?;
    let volume = area.checked_mul(height).ok_or(ColumnError::OutOfRange)?;
    let mass_grams = match library.get(&spec.material) {
        Some(_) => Some(library.mass_grams(&spec.material, volume)?),
        None => None,
    };
    Ok(Quantities {
        height,
        area,
        volume,
        mass_grams,
    })
}

fn fixed(text: &str) -> Result<Id, ColumnError> {
    Id::parse(text).map_err(|_| ColumnError::Malformed(format!("{text:?} is not an ID")))
}

/// The four read-only parameters of a column: its height, section area, volume and mass (a number
/// of kilograms; none when the material is not in the library).
///
/// # Errors
/// [`ColumnError::Malformed`] only if a fixed ID is refused, which it is not.
pub fn read_only(q: &Quantities) -> Result<Vec<(Definition, Option<Value>)>, ColumnError> {
    let define = |id: &str, name: &str, kind: Kind| {
        Definition::new(fixed(id)?, name, kind, Holder::Instance)
            .map_err(|e| ColumnError::Malformed(e.to_string()))
    };
    let kilograms = match q.mass_grams {
        Some(grams) => Some(Value::number(Decimal::new(
            i64::try_from(grams).map_err(|_| ColumnError::OutOfRange)?,
            3,
        ))),
        None => None,
    };
    Ok(vec![
        (
            define(HEIGHT, "Height", Kind::Length)?,
            Some(Value::Length(q.height)),
        ),
        (
            define(AREA, "Section area", Kind::Area)?,
            Some(Value::Area(q.area)),
        ),
        (
            define(VOLUME, "Volume", Kind::Volume)?,
            Some(Value::Volume(q.volume)),
        ),
        (define(MASS, "Mass (kg)", Kind::Number)?, kilograms),
    ])
}
