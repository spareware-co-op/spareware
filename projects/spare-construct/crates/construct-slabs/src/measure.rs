//! What a floor reads back, worked out each time from its boundary and its type and never stored:
//! area, perimeter, thickness, volume and mass, and the same as read-only parameters.
//!
//! - **Area**: the shoelace sum over the corners in grid steps, exact in 128 bits: the outer
//!   loop's area less each hole's. Twice the area is always a whole number of square steps; an
//!   area that is an odd number of half square steps has no exact value in construct-params'
//!   [`Area`] and is refused ([`SlabError::AreaNotExact`]), never rounded. Corners on whole
//!   millimetres (or any even number of steps) never meet it.
//! - **Perimeter**: every edge's length, the holes' edges included, each the nearest whole grid
//!   step to its true length (an integer square root, exact in whole numbers): exact for an edge
//!   along an axis or of a whole number of steps (a 3-4-5 triangle), and otherwise within half a
//!   step (78 nm) per edge.
//! - **Volume**: per layer, the area times its thickness ([`Area::checked_mul`]); the floor's is
//!   their sum.
//! - **Mass**: per layer, construct-materials' one function ([`Materials::mass_grams`]) on its
//!   volume of its material; the floor's is their sum, and is known only when every layer's
//!   material is in the library.
//! - **Read-only parameters** ([`read_only`]): the same five as parameter definitions with fixed
//!   IDs ([`AREA`], [`PERIMETER`], [`THICKNESS`], [`VOLUME`], [`MASS`]), held by the instance and
//!   never stored, for a page to show and a schedule to list. Mass is a number of kilograms
//!   (three places, so exact to the gram): construct-params has no mass kind yet.

use crate::boundary::{Boundary, twice_signed_area};
use crate::error::SlabError;
use crate::layers::Layers;
use construct_materials::Materials;
use construct_params::{Area, Decimal, Definition, Holder, Kind, Length, Value, Volume};
use construct_solver::Point;
use kit_core::Id;

/// The read-only parameter of a floor's area.
pub const AREA: &str = "floor.area";
/// The read-only parameter of a floor's perimeter, its holes' edges included.
pub const PERIMETER: &str = "floor.perimeter";
/// The read-only parameter of a floor's total thickness, its type's.
pub const THICKNESS: &str = "floor.thickness";
/// The read-only parameter of a floor's volume.
pub const VOLUME: &str = "floor.volume";
/// The read-only parameter of a floor's mass, in kilograms.
pub const MASS: &str = "floor.mass";

/// Every loop's corners, outer first.
fn loops(boundary: &Boundary) -> Vec<Vec<Point>> {
    boundary
        .profile()
        .into_iter()
        .map(|ring| ring.into_iter().map(|(_, p)| p).collect())
        .collect()
}

/// The floor's area: the outer loop's less every hole's, exact.
///
/// # Errors
/// [`SlabError::AreaNotExact`] when it is not a whole number of square grid steps.
pub fn area(boundary: &Boundary) -> Result<Area, SlabError> {
    let mut twice = 0_i128;
    for (at, corners) in loops(boundary).iter().enumerate() {
        let each = twice_signed_area(corners).saturating_abs();
        twice = if at == 0 {
            each
        } else {
            twice.checked_sub(each).ok_or(SlabError::OutOfRange)?
        };
    }
    if twice.checked_rem(2) != Some(0) {
        return Err(SlabError::AreaNotExact);
    }
    twice
        .checked_div(2)
        .map(Area::from_square_steps)
        .ok_or(SlabError::OutOfRange)
}

/// The nearest whole number to the square root of `n` (never a tie, since `n` is whole).
fn nearest_root(n: u128) -> u128 {
    let root = n.isqrt();
    // (root + 1/2)^2 = root^2 + root + 1/4, so n rounds up when n > root^2 + root.
    let edge = root.saturating_mul(root).saturating_add(root);
    if n > edge {
        root.saturating_add(1)
    } else {
        root
    }
}

/// The floor's perimeter: every edge's length to the nearest grid step, the holes' included.
///
/// # Errors
/// [`SlabError::OutOfRange`] when the sum does not fit a length.
pub fn perimeter(boundary: &Boundary) -> Result<Length, SlabError> {
    let mut total = 0_i64;
    for corners in loops(boundary) {
        for (at, a) in corners.iter().enumerate() {
            let b = corners
                .get(at.saturating_add(1))
                .or_else(|| corners.first())
                .copied()
                .unwrap_or(*a);
            // Coordinates are within 2^40 steps, so each difference squared is under 2^84.
            let dx = i128::from(b.x)
                .saturating_sub(i128::from(a.x))
                .unsigned_abs();
            let dy = i128::from(b.y)
                .saturating_sub(i128::from(a.y))
                .unsigned_abs();
            let length = nearest_root(dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy)));
            let length = i64::try_from(length).map_err(|_| SlabError::OutOfRange)?;
            total = total.checked_add(length).ok_or(SlabError::OutOfRange)?;
        }
    }
    Ok(Length::from_steps(total))
}

/// One layer's share of a floor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerQuantity {
    /// Its volume: the floor's area times its thickness.
    pub volume: Volume,
    /// Its mass in whole grams, when its material is in the library.
    pub mass_grams: Option<u64>,
}

/// What a floor reads back from its boundary and its type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quantities {
    /// Its area.
    pub area: Area,
    /// Its perimeter, the holes' edges included.
    pub perimeter: Length,
    /// Its total thickness.
    pub thickness: Length,
    /// Its volume.
    pub volume: Volume,
    /// Each layer's share, top first.
    pub layers: Vec<LayerQuantity>,
    /// Its mass in whole grams, when every layer's material is in the library.
    pub mass_grams: Option<u64>,
}

/// What a floor of these layers with this boundary reads back, its mass from `library`.
///
/// # Errors
/// [`SlabError::AreaNotExact`]; [`SlabError::OutOfRange`] when a sum does not fit;
/// [`SlabError::Material`] for a material the library holds without a usable density.
pub fn quantities(
    boundary: &Boundary,
    layers: &Layers,
    library: &Materials,
) -> Result<Quantities, SlabError> {
    let area = area(boundary)?;
    let mut volume = Volume::from_cubic_steps(0);
    let mut mass = Some(0_u64);
    let mut each = Vec::with_capacity(layers.layers().len());
    for layer in layers.layers() {
        let share = area
            .checked_mul(layer.thickness)
            .ok_or(SlabError::OutOfRange)?;
        volume = volume.checked_add(share).ok_or(SlabError::OutOfRange)?;
        let grams = match library.get(&layer.material) {
            Some(_) => Some(library.mass_grams(&layer.material, share)?),
            None => None,
        };
        mass = match (mass, grams) {
            (Some(sum), Some(grams)) => Some(sum.checked_add(grams).ok_or(SlabError::OutOfRange)?),
            _ => None,
        };
        each.push(LayerQuantity {
            volume: share,
            mass_grams: grams,
        });
    }
    Ok(Quantities {
        area,
        perimeter: perimeter(boundary)?,
        thickness: layers.total(),
        volume,
        layers: each,
        mass_grams: mass,
    })
}

/// One of this crate's fixed parameter IDs.
fn fixed(text: &str) -> Result<Id, SlabError> {
    Id::parse(text).map_err(|_| SlabError::Malformed(format!("{text:?} is not an ID")))
}

/// A floor's read-only parameters, each with its value: area, perimeter, thickness, volume and
/// mass, in that order. Mass has no value when it is not known.
///
/// # Errors
/// [`SlabError::OutOfRange`] for a mass beyond a decimal; [`SlabError::Malformed`] never, for
/// these fixed definitions.
pub fn read_only(q: &Quantities) -> Result<Vec<(Definition, Option<Value>)>, SlabError> {
    let define = |id: &str, name: &str, kind: Kind| {
        Definition::new(fixed(id)?, name, kind, Holder::Instance)
            .map_err(|e| SlabError::Malformed(e.to_string()))
    };
    let kilograms = match q.mass_grams {
        Some(grams) => Some(Value::number(Decimal::new(
            i64::try_from(grams).map_err(|_| SlabError::OutOfRange)?,
            3,
        ))),
        None => None,
    };
    Ok(vec![
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
    ])
}
