//! The stored forms: a grid line and a reference plane as element records (FORMAT.md beside this
//! crate). One spelling for each, so the same data gives the same bytes; reading refuses what
//! writing would not give.
//!
//! The geometry is in the record's recipe, as stored text of [`Value`] (`length:q0:...`,
//! `angle:...`), exact to the step: the recipe is how the shape is built, and a datum's shape is
//! its geometry. Other keys in a record's `props` than `@category` (a parameter value, a type)
//! belong to other crates and are kept as they are by [`Grid::update_record`] and
//! [`ReferencePlane::update_record`].

use crate::GridError;
use crate::datum::{Grid, PlaneShape, ReferencePlane};
use crate::shape::{Point, Shape};
use construct_categories::{GRIDS, REFERENCE_PLANES};
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_params::{Angle, Length, Value};
use kit_core::Id;
use serde_json::Value as Json;
use std::collections::BTreeMap;

/// The kind of a grid line's record.
pub const GRID_KIND: &str = "grid";
/// The kind of a reference plane's record.
pub const PLANE_KIND: &str = "refplane";
/// The `props` key that holds a record's category ID (construct-types' key, the same on every
/// record).
pub const CATEGORY_KEY: &str = "@category";

/// The recipe operation of a straight grid line.
pub const OP_GRID_LINE: &str = "grid.line";
/// The recipe operation of an arc grid line.
pub const OP_GRID_ARC: &str = "grid.arc";
/// The recipe operation of a vertical reference plane.
pub const OP_PLANE_VERTICAL: &str = "refplane.vertical";
/// The recipe operation of a horizontal reference plane.
pub const OP_PLANE_HORIZONTAL: &str = "refplane.horizontal";

const NO_PLACEMENT: Placement = Placement { x: 0, y: 0, z: 0 };

/// Whether an element kind is a grid line's or a reference plane's.
#[must_use]
pub fn is_datum_kind(kind: &str) -> bool {
    kind == GRID_KIND || kind == PLANE_KIND
}

/// The geometry a datum recipe holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Datum {
    /// A grid line's.
    Grid(Shape),
    /// A reference plane's.
    Plane(PlaneShape),
}

fn malformed(why: &str) -> GridError {
    GridError::Malformed(why.to_owned())
}

fn text_of(value: &Value) -> Json {
    Json::String(value.stored())
}

fn length_arg(length: Length) -> Json {
    text_of(&Value::Length(length))
}

fn angle_arg(angle: Angle) -> Json {
    text_of(&Value::Angle(angle))
}

fn stored(args: &BTreeMap<String, Json>, key: &str) -> Result<Value, GridError> {
    let text = args
        .get(key)
        .and_then(Json::as_str)
        .ok_or_else(|| GridError::Malformed(format!("the recipe has no {key}")))?;
    match Value::from_stored(text) {
        // One spelling: the text written again is the text read.
        Some(value) if value.stored() == text => Ok(value),
        _ => Err(GridError::Malformed(format!(
            "{key} is not in its one spelling"
        ))),
    }
}

fn length_of(args: &BTreeMap<String, Json>, key: &str) -> Result<Length, GridError> {
    match stored(args, key)? {
        Value::Length(length) => Ok(length),
        _ => Err(GridError::Malformed(format!("{key} is not a length"))),
    }
}

fn angle_of(args: &BTreeMap<String, Json>, key: &str) -> Result<Angle, GridError> {
    match stored(args, key)? {
        Value::Angle(angle) => Ok(angle),
        _ => Err(GridError::Malformed(format!("{key} is not an angle"))),
    }
}

fn point_of(args: &BTreeMap<String, Json>, x: &str, y: &str) -> Result<Point, GridError> {
    Ok(Point::new(length_of(args, x)?, length_of(args, y)?))
}

fn args(pairs: Vec<(&str, Json)>) -> BTreeMap<String, Json> {
    pairs
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect()
}

/// A grid line's recipe.
#[must_use]
pub fn shape_recipe(shape: &Shape) -> Recipe {
    match *shape {
        Shape::Line { from, to } => Recipe::construct(
            OP_GRID_LINE,
            args(vec![
                ("x0", length_arg(from.x)),
                ("x1", length_arg(to.x)),
                ("y0", length_arg(from.y)),
                ("y1", length_arg(to.y)),
            ]),
        ),
        Shape::Arc {
            centre,
            radius,
            from,
            to,
        } => Recipe::construct(
            OP_GRID_ARC,
            args(vec![
                ("cx", length_arg(centre.x)),
                ("cy", length_arg(centre.y)),
                ("from", angle_arg(from)),
                ("radius", length_arg(radius)),
                ("to", angle_arg(to)),
            ]),
        ),
    }
}

/// A reference plane's recipe.
#[must_use]
pub fn plane_recipe(shape: &PlaneShape) -> Recipe {
    match *shape {
        PlaneShape::Vertical { from, to } => Recipe::construct(
            OP_PLANE_VERTICAL,
            args(vec![
                ("x0", length_arg(from.x)),
                ("x1", length_arg(to.x)),
                ("y0", length_arg(from.y)),
                ("y1", length_arg(to.y)),
            ]),
        ),
        PlaneShape::Horizontal { elevation } => Recipe::construct(
            OP_PLANE_HORIZONTAL,
            args(vec![("z", length_arg(elevation))]),
        ),
    }
}

/// The geometry a recipe holds, if it is one of this crate's four operations: `None` for any other
/// operation (not an error: it is another crate's), and for these four the reading, strict.
#[must_use]
pub fn datum_of_recipe(recipe: &Recipe) -> Option<Result<Datum, GridError>> {
    let keys = |wanted: &[&str]| {
        let found: Vec<&str> = recipe.args.keys().map(String::as_str).collect();
        if found == wanted {
            Ok(())
        } else {
            Err(malformed(
                "the recipe's arguments are not the ones this operation has",
            ))
        }
    };
    let read = || -> Result<Datum, GridError> {
        let a = &recipe.args;
        match recipe.op.as_str() {
            OP_GRID_LINE => {
                keys(&["x0", "x1", "y0", "y1"])?;
                let line = Shape::line(point_of(a, "x0", "y0")?, point_of(a, "x1", "y1")?)?;
                Ok(Datum::Grid(line))
            }
            OP_GRID_ARC => {
                keys(&["cx", "cy", "from", "radius", "to"])?;
                let centre = point_of(a, "cx", "cy")?;
                let arc = Shape::arc(
                    centre,
                    length_of(a, "radius")?,
                    angle_of(a, "from")?,
                    angle_of(a, "to")?,
                )?;
                // The angles are kept in [0, 360): a stored angle outside it is not what is written.
                if shape_recipe(&arc) != *recipe {
                    return Err(malformed("an arc's angles are from 0 up to 360 degrees"));
                }
                Ok(Datum::Grid(arc))
            }
            OP_PLANE_VERTICAL => {
                keys(&["x0", "x1", "y0", "y1"])?;
                let from = point_of(a, "x0", "y0")?;
                let to = point_of(a, "x1", "y1")?;
                Shape::line(from, to)?;
                Ok(Datum::Plane(PlaneShape::Vertical { from, to }))
            }
            _ => {
                keys(&["z"])?;
                let shape = PlaneShape::Horizontal {
                    elevation: length_of(a, "z")?,
                };
                shape.check()?;
                Ok(Datum::Plane(shape))
            }
        }
    };
    let ours = [
        OP_GRID_LINE,
        OP_GRID_ARC,
        OP_PLANE_VERTICAL,
        OP_PLANE_HORIZONTAL,
    ];
    (recipe.owner == "construct" && ours.contains(&recipe.op.as_str())).then(|| {
        if recipe.schema == 1 && recipe.op_version == 1 {
            read()
        } else {
            Err(malformed("the recipe is schema 1, operation version 1"))
        }
    })
}

/// The record's fields that every datum record has the same way; why not, if they are not.
fn plain(record: &Element, kind: &str) -> Result<(), GridError> {
    if record.kind != kind {
        return Err(GridError::Malformed(format!(
            "the record's kind is not {kind:?}"
        )));
    }
    if record.level != 0 || record.placement != NO_PLACEMENT || !record.references.is_empty() {
        return Err(malformed("a datum has no level, placement or references"));
    }
    Ok(())
}

fn category_is(record: &Element, id: u32) -> Result<(), GridError> {
    if record.props.get(CATEGORY_KEY).map(String::as_str) == Some(id.to_string().as_str()) {
        Ok(())
    } else {
        Err(malformed(
            "the record's category is not the one its kind has",
        ))
    }
}

impl Grid {
    /// Its element record in `document`: kind `grid`, its name, the recipe that holds its
    /// geometry, and its category in `props`.
    #[must_use]
    pub fn to_record(&self, document: &Id) -> Element {
        let mut props = BTreeMap::new();
        props.insert(CATEGORY_KEY.to_owned(), GRIDS.0.to_string());
        Element {
            id: self.id().clone(),
            document: document.clone(),
            kind: GRID_KIND.to_owned(),
            name: self.name().to_owned(),
            level: 0,
            placement: NO_PLACEMENT,
            recipe: shape_recipe(self.shape()),
            props,
            references: Vec::new(),
        }
    }

    /// The record `record` becomes with this grid's name and geometry, every other field and every
    /// other `props` key as it was.
    #[must_use]
    pub fn update_record(&self, record: &Element) -> Element {
        Element {
            name: self.name().to_owned(),
            recipe: shape_recipe(self.shape()),
            ..record.clone()
        }
    }

    /// Reads a grid line from its element record. Anything [`Grid::to_record`] would not write
    /// is refused, except `props` keys other than `@category`, which are not read.
    ///
    /// # Errors
    ///
    /// [`GridError::Malformed`] for a record that is not a grid as written, or the refusal of its
    /// name or its shape.
    pub fn from_record(record: &Element) -> Result<Self, GridError> {
        plain(record, GRID_KIND)?;
        category_is(record, GRIDS.0)?;
        let Some(datum) = datum_of_recipe(&record.recipe) else {
            return Err(malformed("the recipe is not a grid line's"));
        };
        let Datum::Grid(shape) = datum? else {
            return Err(malformed("the recipe is not a grid line's"));
        };
        let grid = Self::new(record.id.clone(), &record.name, shape)?;
        if grid.name() != record.name {
            return Err(malformed("a name is trimmed"));
        }
        Ok(grid)
    }
}

impl ReferencePlane {
    /// Its element record in `document`: kind `refplane`, its name (empty if it has none), the
    /// recipe that holds its geometry, and its category in `props`.
    #[must_use]
    pub fn to_record(&self, document: &Id) -> Element {
        let mut props = BTreeMap::new();
        props.insert(CATEGORY_KEY.to_owned(), REFERENCE_PLANES.0.to_string());
        Element {
            id: self.id().clone(),
            document: document.clone(),
            kind: PLANE_KIND.to_owned(),
            name: self.name().unwrap_or_default().to_owned(),
            level: 0,
            placement: NO_PLACEMENT,
            recipe: plane_recipe(self.shape()),
            props,
            references: Vec::new(),
        }
    }

    /// The record `record` becomes with this plane's name and geometry, the rest as it was.
    #[must_use]
    pub fn update_record(&self, record: &Element) -> Element {
        Element {
            name: self.name().unwrap_or_default().to_owned(),
            recipe: plane_recipe(self.shape()),
            ..record.clone()
        }
    }

    /// Reads a reference plane from its element record, as [`Grid::from_record`] does a grid.
    ///
    /// # Errors
    ///
    /// [`GridError::Malformed`] for a record that is not a reference plane as written, or the
    /// refusal of its name or its shape.
    pub fn from_record(record: &Element) -> Result<Self, GridError> {
        plain(record, PLANE_KIND)?;
        category_is(record, REFERENCE_PLANES.0)?;
        let Some(datum) = datum_of_recipe(&record.recipe) else {
            return Err(malformed("the recipe is not a reference plane's"));
        };
        let Datum::Plane(shape) = datum? else {
            return Err(malformed("the recipe is not a reference plane's"));
        };
        let name = (!record.name.is_empty()).then_some(record.name.as_str());
        let plane = Self::new(record.id.clone(), name, shape)?;
        if plane.name().unwrap_or_default() != record.name {
            return Err(malformed("a name is trimmed"));
        }
        Ok(plane)
    }
}
