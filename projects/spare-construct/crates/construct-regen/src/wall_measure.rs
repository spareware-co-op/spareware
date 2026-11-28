//! What a layered wall reads back: its length, its face area, its volume and each layer's volume,
//! worked out each time from the wall as built and never stored (issue: a wall has no quantity to
//! read or to schedule). The same numbers are read-only parameters ([`read_only`]), for a page to
//! show and a schedule to list.
//!
//! The convention (decision note CON-A4-F2-P3-wall-quantities.md):
//! - **Length**: the wall's reference line, start to end, in grid steps. Exact for a wall along an
//!   axis; for a wall at any other angle the nearest whole grid step to the true length (an
//!   integer square root, within half a step), as a floor's perimeter is.
//! - **Area**: one face, the reference line's: length times height, less each opening's width
//!   times its height (the opening the wall's builder cuts, [`crate::inserts`]). Exact for a wall
//!   along an axis. It is the face on the location line, whatever the wall's thickness.
//! - **Volume**: per layer, the layer's plan area as built times the wall's height, less each
//!   opening's width times the layer's thickness times the opening's height; the wall's is the
//!   sum of its layers'. The layer's plan area is the sum of its joined pieces' areas when the
//!   wall's joints are made ([`crate::wall_plan::Joints`]), else its strip's, so a joint's
//!   trimming is counted. A layer with no thickness has none. An opening is cut through the whole
//!   thickness, so its box in a layer is its width along the wall by the layer's thickness; where
//!   that box does not lie wholly inside one of the layer's pieces (an opening that reaches into a
//!   joint), the cut is not a plain box and the quantities are refused ([`MeasureError`]) rather
//!   than guessed.
//!
//! Every sum is exact whole-number arithmetic through construct-params' `Length`, `Area` and
//! `Volume`: no float, and nothing rounded except a line's own length at an angle. Why this is
//! written here and not in a library: the numbers are the wall plan's own exact grid arithmetic
//! (the strip's area is the size of a cross product of grid vectors), and no library holds a
//! layered wall.
//!
//! Depends on [`crate::wall_plan`] (the wall as built), [`crate::inserts`] (the openings) and
//! construct-params. Nothing here is interim.

use crate::inserts::Insert;
use crate::joins;
use crate::parameters::{At, ParameterView};
use crate::wall_plan::Layered;
use construct_model::element::Element;
use construct_params::{Area, Definition, Holder, Kind, Length, Units, Value, Volume};
use kit_core::Id;
use kit_store::{Reader, StoreError};
use std::fmt;

/// The read-only parameter of a wall's length.
pub const LENGTH: &str = "wall.length";
/// The read-only parameter of a wall's face area.
pub const AREA: &str = "wall.area";
/// The read-only parameter of a wall's volume.
pub const VOLUME: &str = "wall.volume";

/// Why a wall's quantities are not read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MeasureError {
    /// A sum or a product does not fit.
    OutOfRange,
    /// A piece of a layer has an odd number of half square steps, which `Area` cannot hold.
    AreaNotExact,
    /// An opening reaches into a joint: its box is not wholly inside one piece of the layer.
    OpeningInJoint {
        /// What the opening is called.
        who: String,
        /// The layer it reaches into, counting from 1 at the exterior.
        layer: usize,
    },
    /// An opening on a wall that does not run along an axis.
    OpeningNotAlongAxis(String),
    /// A layer is too thin to have an area on the grid at this angle.
    Degenerate,
}

impl fmt::Display for MeasureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange => f.write_str("a quantity is beyond the kernel's grid"),
            Self::AreaNotExact => f.write_str("an area is not a whole number of square grid steps"),
            Self::OpeningInJoint { who, layer } => write!(
                f,
                "{who} reaches into a joint of layer {layer}: its cut is not a plain box"
            ),
            Self::OpeningNotAlongAxis(who) => {
                write!(f, "{who} is in a wall that does not run along an axis")
            }
            Self::Degenerate => f.write_str("a layer is too thin to have an area at this angle"),
        }
    }
}

impl std::error::Error for MeasureError {}

/// One layer's share of a wall.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerQuantity {
    /// What it does, in construct-walls' words.
    pub function: String,
    /// What it is made of, by material ID.
    pub material: String,
    /// How thick it is.
    pub thickness: Length,
    /// Its volume, openings taken out.
    pub volume: Volume,
}

/// What a wall reads back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quantities {
    /// Its reference line's length.
    pub length: Length,
    /// Its one-face area, openings taken out.
    pub area: Area,
    /// Its volume, openings taken out: its layers' sum.
    pub volume: Volume,
    /// Each layer's share, from the exterior.
    pub layers: Vec<LayerQuantity>,
}

/// The nearest whole number to the square root of `n` (never a tie, since `n` is whole).
/// Written here: `u128::isqrt` is the floor, and the rounding rule is the floor perimeter's.
fn nearest_root(n: u128) -> u128 {
    let root = n.isqrt();
    let edge = root.saturating_mul(root).saturating_add(root);
    if n > edge {
        root.saturating_add(1)
    } else {
        root
    }
}

fn length_of(wall: &Layered) -> Result<Length, MeasureError> {
    if let Some(steps) = wall.plan.axis_length() {
        return Ok(Length::from_steps(steps));
    }
    let [dx, dy] = wall.plan.direction();
    let square = |v: i64| u128::from(v.unsigned_abs()).saturating_mul(u128::from(v.unsigned_abs()));
    let root = nearest_root(square(dx).saturating_add(square(dy)));
    i64::try_from(root)
        .map(Length::from_steps)
        .map_err(|_| MeasureError::OutOfRange)
}

/// Twice the area of a counter-clockwise polygon, in square steps.
fn twice_area(corners: &[[i64; 2]]) -> Option<i128> {
    let mut sum = 0_i128;
    for (at, a) in corners.iter().enumerate() {
        let b = corners
            .get(at.checked_add(1)?)
            .or_else(|| corners.first())?;
        let part = i128::from(a[0])
            .checked_mul(i128::from(b[1]))?
            .checked_sub(i128::from(a[1]).checked_mul(i128::from(b[0]))?)?;
        sum = sum.checked_add(part)?;
    }
    Some(sum)
}

fn area_of(corners: &[[i64; 2]]) -> Result<Area, MeasureError> {
    let twice = twice_area(corners).ok_or(MeasureError::OutOfRange)?.abs();
    if twice.checked_rem(2) != Some(0) {
        return Err(MeasureError::AreaNotExact);
    }
    twice
        .checked_div(2)
        .map(Area::from_square_steps)
        .ok_or(MeasureError::OutOfRange)
}

/// Whether `p` lies on or inside the convex counter-clockwise polygon `corners`.
fn inside(corners: &[[i64; 2]], p: [i64; 2]) -> bool {
    corners.iter().enumerate().all(|(at, a)| {
        let b = corners
            .get(at.saturating_add(1))
            .or_else(|| corners.first())
            .unwrap_or(a);
        let side = |v: i64, w: i64| i128::from(v).saturating_sub(i128::from(w));
        let cross = side(b[0], a[0])
            .saturating_mul(side(p[1], a[1]))
            .saturating_sub(side(b[1], a[1]).saturating_mul(side(p[0], a[0])));
        cross >= 0
    })
}

/// One opening's box in a layer: along the wall from `from` to `to`, across the layer between
/// `across`, as the four plan corners (the wall's axis index `axis`).
fn corners_of(axis: usize, [from, to]: [i64; 2], [c0, c1]: [i64; 2]) -> [[i64; 2]; 4] {
    let at = |along: i64, across: i64| {
        let mut p = [0_i64; 2];
        if let Some(slot) = p.get_mut(axis) {
            *slot = along;
        }
        if let Some(slot) = p.get_mut(1_usize.saturating_sub(axis)) {
            *slot = across;
        }
        p
    };
    [at(from, c0), at(to, c0), at(to, c1), at(from, c1)]
}

fn product(a: Length, b: Length) -> Result<Area, MeasureError> {
    a.checked_mul(b).ok_or(MeasureError::OutOfRange)
}

/// The wall's length and its one-face area, openings taken out: what does not depend on how its
/// joints cut its layers.
///
/// # Errors
/// [`MeasureError::OutOfRange`], or [`MeasureError::OpeningNotAlongAxis`] for an opening in a wall
/// that does not run along an axis.
pub fn face(wall: &Layered, inserts: &[Insert]) -> Result<(Length, Area), MeasureError> {
    let length = length_of(wall)?;
    let height = Length::from_steps(wall.height);
    if let (None, Some(first)) = (wall.plan.axis(), inserts.first()) {
        return Err(MeasureError::OpeningNotAlongAxis(first.who.clone()));
    }
    let mut area = product(length, height)?;
    for insert in inserts {
        let width = Length::from_steps(
            insert
                .high
                .checked_sub(insert.low)
                .ok_or(MeasureError::OutOfRange)?,
        );
        let tall = Length::from_steps(
            insert
                .head
                .checked_sub(insert.foot)
                .ok_or(MeasureError::OutOfRange)?,
        );
        area = area
            .checked_sub(product(width, tall)?)
            .ok_or(MeasureError::OutOfRange)?;
    }
    Ok((length, area))
}

/// What the wall `wall` (in its element's frame) reads back, with the openings `inserts` hosted
/// in it (in the model's plan, so `origin`, the wall's placement in grid steps, is taken off).
///
/// # Errors
/// [`MeasureError`], as its variants say.
pub fn quantities(
    wall: &Layered,
    origin: [i64; 2],
    inserts: &[Insert],
) -> Result<Quantities, MeasureError> {
    let (length, area) = face(wall, inserts)?;
    let height = Length::from_steps(wall.height);
    let axis = wall.plan.axis().map(crate::wall_plan::Axis::index);
    let start = wall.plan.start();
    let mut layers = Vec::with_capacity(wall.layers.len());
    let mut volume = Volume::from_cubic_steps(0);
    for (at, layer) in wall.layers.iter().enumerate() {
        let number = at.saturating_add(1);
        let thickness = Length::from_steps(
            i64::try_from(layer.thickness()).map_err(|_| MeasureError::OutOfRange)?,
        );
        let mut share = Volume::from_cubic_steps(0);
        if layer.thickness() != 0 {
            let pieces: Vec<Vec<[i64; 2]>> = if let Some(joined) = &wall.joined {
                joined.layers.get(at).cloned().unwrap_or_default()
            } else {
                let (corners, _) = wall
                    .plan
                    .strip(layer.exterior, layer.interior)
                    .map_err(|_| MeasureError::Degenerate)?;
                vec![corners.to_vec()]
            };
            for piece in &pieces {
                share = share
                    .checked_add(
                        area_of(piece)?
                            .checked_mul(height)
                            .ok_or(MeasureError::OutOfRange)?,
                    )
                    .ok_or(MeasureError::OutOfRange)?;
            }
            if let Some(axis) = axis {
                let across_index = 1_usize.saturating_sub(axis);
                let line = start.get(across_index).copied().unwrap_or_default();
                let face = |offset: i64| {
                    wall.plan
                        .across(offset)
                        .ok()
                        .and_then(|r| r.get(across_index).copied())
                        .and_then(|r| r.checked_add(line))
                };
                let (Some(c0), Some(c1)) = (face(layer.exterior), face(layer.interior)) else {
                    return Err(MeasureError::OutOfRange);
                };
                let band = [c0.min(c1), c0.max(c1)];
                for insert in inserts {
                    let along = |v: i64| v.checked_sub(origin.get(axis).copied().unwrap_or(0));
                    let (Some(from), Some(to)) = (along(insert.low), along(insert.high)) else {
                        return Err(MeasureError::OutOfRange);
                    };
                    let rect = corners_of(axis, [from, to], band);
                    if !pieces
                        .iter()
                        .any(|piece| rect.iter().all(|p| inside(piece, *p)))
                    {
                        return Err(MeasureError::OpeningInJoint {
                            who: insert.who.clone(),
                            layer: number,
                        });
                    }
                    let across = Length::from_steps(
                        band[1]
                            .checked_sub(band[0])
                            .ok_or(MeasureError::OutOfRange)?,
                    );
                    let width =
                        Length::from_steps(to.checked_sub(from).ok_or(MeasureError::OutOfRange)?);
                    let tall = Length::from_steps(
                        insert
                            .head
                            .checked_sub(insert.foot)
                            .ok_or(MeasureError::OutOfRange)?,
                    );
                    let cut = product(width, across)?
                        .checked_mul(tall)
                        .ok_or(MeasureError::OutOfRange)?;
                    share = share.checked_sub(cut).ok_or(MeasureError::OutOfRange)?;
                }
            }
        }
        volume = volume.checked_add(share).ok_or(MeasureError::OutOfRange)?;
        layers.push(LayerQuantity {
            function: layer.function.clone(),
            material: layer.material.clone(),
            thickness,
            volume: share,
        });
    }
    Ok(Quantities {
        length,
        area,
        volume,
        layers,
    })
}

fn define(id: &str, name: &str, kind: Kind) -> Option<Definition> {
    let id = Id::parse(id).ok()?;
    Definition::new(id, name, kind, Holder::Instance).ok()
}

/// A wall's read-only parameters, each with its value: length, area and volume, then each layer's
/// volume (`wall.layer-<k>.volume`, from 1 at the exterior, named with the layer's function and
/// material), in that order.
#[must_use]
pub fn read_only(q: &Quantities) -> Vec<(Definition, Option<Value>)> {
    let mut rows = Vec::with_capacity(q.layers.len().saturating_add(3));
    let fixed_rows = [
        (LENGTH, "Length", Kind::Length, Value::Length(q.length)),
        (AREA, "Area", Kind::Area, Value::Area(q.area)),
        (VOLUME, "Volume", Kind::Volume, Value::Volume(q.volume)),
    ];
    for (id, name, kind, value) in fixed_rows {
        if let Some(def) = define(id, name, kind) {
            rows.push((def, Some(value)));
        }
    }
    for (at, layer) in q.layers.iter().enumerate() {
        let k = at.saturating_add(1);
        let name = format!("Layer {k} ({}, {}) volume", layer.function, layer.material);
        if let Some(def) = define(&format!("wall.layer-{k}.volume"), &name, Kind::Volume) {
            rows.push((def, Some(Value::Volume(layer.volume))));
        }
    }
    rows
}

/// A wall's read-only parameters as a page shows them, worked out of the document as it stands
/// now: its recipe as built (its type's layers and its joints filled), the openings hosted in it
/// ([`crate::inserts`]) and [`quantities`], each row marked `read-only`. A record that is not a
/// layered wall, or one that does not build, has none (its built row says why). When the
/// quantities are refused ([`MeasureError`], an opening reaching into a joint) the length and the
/// face area are still given, and the volume row has no value and says why.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn read_only_view<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    wall: &Element,
    units: &Units,
) -> Result<Vec<ParameterView>, StoreError> {
    if !crate::wall_plan::is_layered(&wall.recipe) {
        return Ok(Vec::new());
    }
    let Ok(recipe) = joins::recipe_at(reader, project, document, wall, At::Now).await? else {
        return Ok(Vec::new());
    };
    let Ok(layered) = Layered::of(&recipe) else {
        return Ok(Vec::new());
    };
    let inserts = crate::inserts::of_wall(reader, project, document, wall).await?;
    let (Some(x), Some(y)) = (
        Length::checked_from_mm(wall.placement.x),
        Length::checked_from_mm(wall.placement.y),
    ) else {
        return Ok(Vec::new());
    };
    let shown = |rows: Vec<(Definition, Option<Value>)>| -> Vec<ParameterView> {
        rows.into_iter()
            .map(|(def, value)| crate::slabs::shown(&def, value, units))
            .collect()
    };
    Ok(
        match quantities(&layered, [x.steps(), y.steps()], &inserts) {
            Ok(q) => shown(read_only(&q)),
            Err(why) => {
                // The face does not depend on the joints: give it, and say why the volume is not.
                let mut rows = Vec::new();
                if let Ok((length, area)) = face(&layered, &inserts) {
                    for (id, name, kind, value) in [
                        (LENGTH, "Length", Kind::Length, Value::Length(length)),
                        (AREA, "Area", Kind::Area, Value::Area(area)),
                    ] {
                        if let Some(def) = define(id, name, kind) {
                            rows.push(crate::slabs::shown(&def, Some(value), units));
                        }
                    }
                }
                if let Some(def) = define(VOLUME, "Volume", Kind::Volume) {
                    let mut row = crate::slabs::shown(&def, None, units);
                    row.reason = Some(why.to_string());
                    rows.push(row);
                }
                rows
            }
        },
    )
}
