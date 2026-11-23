//! What a roof reads back, worked out each time from its footprint, its edges and its type and
//! never stored: its plan area, its area as laid on its slopes, its thickness, its pitch, a
//! gable's ridge height, its volume and its mass, and each layer's volume and mass, also as
//! read-only parameters a page shows and a schedule lists.
//!
//! Every figure is the shape's ([`crate::shape`]): the areas and volumes exactly those of the
//! solids built, the slope area the nearest square step to the plan area times `sqrt(1 + s^2)`.
//! Mass is construct-materials' one function ([`Materials::mass_grams`]) on each layer's volume,
//! the roof's known only when every layer's material is in the library. Mass is a number of
//! kilograms, three places, so exact to the gram: construct-params has no mass kind yet.
//!
//! The read-only parameters ([`read_only`]), with fixed IDs: [`AREA`], [`SLOPE_AREA`],
//! [`THICKNESS`], [`VOLUME`], [`MASS`], and for a sloped roof [`PITCH`] (the pitch as given, a
//! slope or an angle; a gable's first slope's) and for a gable, or a solved roof rising from more
//! than one edge, [`RIDGE_HEIGHT`] (a solved roof's: its top face's highest point); per layer ([`layer_read_only`]), layer
//! `k` from 1 at the top, `roof.layer-<k>.volume` and `roof.layer-<k>.mass`, named with the
//! layer's function and material, as a floor's are.

use crate::error::RoofError;
use crate::roof::Roof;
use crate::shape::{Shape, shape};
use construct_materials::Materials;
use construct_params::{Area, Decimal, Definition, Holder, Kind, Length, Value, Volume};
use construct_slabs::Layers;
use kit_core::Id;

/// The read-only parameter of the area a roof covers in plan, its overhangs included.
pub const AREA: &str = "roof.area";
/// The read-only parameter of a roof's top face as laid on its slopes.
pub const SLOPE_AREA: &str = "roof.slope-area";
/// The read-only parameter of a roof's total thickness, square to its slopes: its type's.
pub const THICKNESS: &str = "roof.thickness";
/// The read-only parameter of a roof's volume.
pub const VOLUME: &str = "roof.volume";
/// The read-only parameter of a roof's mass, in kilograms.
pub const MASS: &str = "roof.mass";
/// The read-only parameter of a sloped roof's pitch, as given.
pub const PITCH: &str = "roof.pitch";
/// The read-only parameter of a gable's ridge height: its top face along the ridge above its base.
pub const RIDGE_HEIGHT: &str = "roof.ridge-height";

/// One layer's share of a roof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerQuantity {
    /// Its volume: its solid's, or none for a layer of no thickness.
    pub volume: Volume,
    /// Its mass in whole grams, when its material is in the library.
    pub mass_grams: Option<u64>,
}

/// What a roof reads back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quantities {
    /// The area it covers in plan, its overhangs included.
    pub area: Area,
    /// Its top face's area as laid on its slopes.
    pub slope_area: Area,
    /// Its total thickness, square to its slopes.
    pub thickness: Length,
    /// Its pitch as given, a sloped roof's.
    pub pitch: Option<Value>,
    /// A gable's ridge height above its base.
    pub ridge_height: Option<Length>,
    /// Its volume.
    pub volume: Volume,
    /// Each layer's share, top first.
    pub layers: Vec<LayerQuantity>,
    /// Its mass in whole grams, when every layer's material is in the library.
    pub mass_grams: Option<u64>,
}

/// What a roof of these layers reads back, its mass from `library`.
///
/// # Errors
/// Any refusal of [`shape`]; [`RoofError::OutOfRange`] when a sum does not fit;
/// [`RoofError::Material`] for a material the library holds without a usable density.
pub fn quantities(
    roof: &Roof,
    layers: &Layers,
    library: &Materials,
) -> Result<Quantities, RoofError> {
    let built: Shape = shape(roof, layers)?;
    let mut volume = Volume::from_cubic_steps(0);
    let mut mass = Some(0_u64);
    let mut each = Vec::with_capacity(layers.layers().len());
    for (at, layer) in layers.layers().iter().enumerate() {
        let share = built
            .solids
            .iter()
            .find(|solid| solid.layer == at)
            .map_or(Volume::from_cubic_steps(0), |solid| solid.volume);
        volume = volume.checked_add(share).ok_or(RoofError::OutOfRange)?;
        let grams = match library.get(&layer.material) {
            Some(_) => Some(library.mass_grams(&layer.material, share)?),
            None => None,
        };
        mass = match (mass, grams) {
            (Some(sum), Some(grams)) => Some(sum.checked_add(grams).ok_or(RoofError::OutOfRange)?),
            _ => None,
        };
        each.push(LayerQuantity {
            volume: share,
            mass_grams: grams,
        });
    }
    let pitch = roof
        .form()
        .eaves()
        .first()
        .and_then(|eaves| roof.edge(eaves))
        .and_then(|spec| spec.pitch)
        .map(crate::Pitch::value);
    // A gable's ridge, or a solved roof's highest point where two or more slopes meet.
    let ridge_height = match roof.form() {
        crate::Form::Solved { eaves } if eaves.len() > 1 => Some(built.height),
        _ => built.ridge.map(|ridge| ridge.height),
    };
    Ok(Quantities {
        area: built.plan_area,
        slope_area: built.slope_area,
        thickness: layers.total(),
        pitch,
        ridge_height,
        volume,
        layers: each,
        mass_grams: mass,
    })
}

/// Grams as a number of kilograms, three places: exact to the gram.
fn kilograms(grams: Option<u64>) -> Result<Option<Value>, RoofError> {
    Ok(match grams {
        Some(grams) => Some(Value::number(Decimal::new(
            i64::try_from(grams).map_err(|_| RoofError::OutOfRange)?,
            3,
        ))),
        None => None,
    })
}

/// A read-only parameter's definition with one of this crate's fixed IDs.
fn define(id: &str, name: &str, kind: Kind) -> Result<Definition, RoofError> {
    let id = Id::parse(id).map_err(|_| RoofError::Malformed(format!("{id:?} is not an ID")))?;
    Definition::new(id, name, kind, Holder::Instance)
        .map_err(|e| RoofError::Malformed(e.to_string()))
}

/// A roof's read-only parameters, each with its value: area, slope area, thickness, volume and
/// mass, then a sloped roof's pitch and a gable's ridge height, in that order. Mass has no value
/// when it is not known.
///
/// # Errors
/// [`RoofError::OutOfRange`] for a mass beyond a decimal; [`RoofError::Malformed`] never, for
/// these fixed definitions.
pub fn read_only(q: &Quantities) -> Result<Vec<(Definition, Option<Value>)>, RoofError> {
    let mut rows = vec![
        (define(AREA, "Area", Kind::Area)?, Some(Value::Area(q.area))),
        (
            define(SLOPE_AREA, "Slope area", Kind::Area)?,
            Some(Value::Area(q.slope_area)),
        ),
        (
            define(THICKNESS, "Thickness", Kind::Length)?,
            Some(Value::Length(q.thickness)),
        ),
        (
            define(VOLUME, "Volume", Kind::Volume)?,
            Some(Value::Volume(q.volume)),
        ),
        (
            define(MASS, "Mass (kg)", Kind::Number)?,
            kilograms(q.mass_grams)?,
        ),
    ];
    if let Some(pitch) = &q.pitch {
        rows.push((define(PITCH, "Pitch", pitch.kind())?, Some(pitch.clone())));
    }
    if let Some(height) = q.ridge_height {
        rows.push((
            define(RIDGE_HEIGHT, "Ridge height", Kind::Length)?,
            Some(Value::Length(height)),
        ));
    }
    Ok(rows)
}

/// Each layer's read-only parameters, top first: its volume and its mass, layer `k` (from 1 at
/// the top) as `roof.layer-<k>.volume` and `roof.layer-<k>.mass`, named
/// `Layer <k> (<function>, <material>) volume` and `... mass (kg)`. A mass has no value when the
/// library lacks the layer's material. `q` is [`quantities`] of these `layers`.
///
/// # Errors
/// [`RoofError::OutOfRange`] for a mass beyond a decimal; [`RoofError::Malformed`] when `q` is not
/// of these layers (it holds another count of them).
pub fn layer_read_only(
    layers: &Layers,
    q: &Quantities,
) -> Result<Vec<(Definition, Option<Value>)>, RoofError> {
    if layers.layers().len() != q.layers.len() {
        return Err(RoofError::Malformed(
            "the quantities are of other layers".to_owned(),
        ));
    }
    let mut rows = Vec::with_capacity(q.layers.len().saturating_mul(2));
    for (at, (layer, share)) in layers.layers().iter().zip(&q.layers).enumerate() {
        let k = at.saturating_add(1);
        let function = serde_json::to_value(layer.function)
            .ok()
            .and_then(|f| f.as_str().map(str::to_owned))
            .unwrap_or_default();
        let what = format!("Layer {k} ({function}, {})", layer.material.as_str());
        rows.push((
            define(
                &format!("roof.layer-{k}.volume"),
                &format!("{what} volume"),
                Kind::Volume,
            )?,
            Some(Value::Volume(share.volume)),
        ));
        rows.push((
            define(
                &format!("roof.layer-{k}.mass"),
                &format!("{what} mass (kg)"),
                Kind::Number,
            )?,
            kilograms(share.mass_grams)?,
        ));
    }
    Ok(rows)
}
