//! The display shape of a grid line and a reference plane: what the plan and the 3D view draw of a
//! datum, which has no solid.
//!
//! What it promises:
//! - [`build`] answers for the four operations construct-grids owns (`grid.line`, `grid.arc`,
//!   `refplane.vertical`, `refplane.horizontal`) and for no other (`None`), so each builder
//!   hands a recipe here first and goes on if it is not a datum's. Both builders do, the
//!   stand-in ([`crate::stand_in`]) and the kernel's ([`crate::kernel`]): a datum is the same
//!   whichever builds the rest.
//! - The body is the recipe's own operation and arguments as canonical JSON: the geometry exactly,
//!   in stored text, with no float in it. So its content ID is the same on the server and in the
//!   browser, which is what the checkpoints compare.
//! - The display is in whole millimetres, as every display is: a straight piece is its two
//!   points; an arc is a chain of chords no more than 1 mm from the arc (construct-grids'
//!   `Shape::polyline`); each point is the nearest millimetre, so a display strays from the
//!   stored geometry by at most 1 mm for a line and 2 mm for an arc (`deviation` says so). The
//!   mesh is a ribbon standing on the outline, [`RIBBON_MM`] high, with no thickness. A vertical
//!   plane is the same. A horizontal plane has no place in plan and no extent yet: its display
//!   is empty, and its bounds are the point at its elevation.
//! - It has no faces to name: a datum has no solid.
//!
//! Interim: the ribbon's height. A grid is vertical and has no height of its own; how far it is
//! drawn up in 3D is set per level and per view, which are not elements yet (jobs 3.1 for levels,
//! 5.x for views). Until then this one number answers, in this file only.

use construct_grids::{Datum, PlaneShape, Point, Shape, datum_of_recipe};
use construct_model::geometry::{Bounds, Built, Display, GeometryError, Mesh, Recipe};
use construct_params::Length;
use serde::Serialize;
use std::collections::BTreeMap;

/// How high a datum's ribbon stands in the 3D view, in millimetres. Interim: see the header.
pub const RIBBON_MM: i64 = 3000;

/// The most an arc's chords may stray from it, in steps: 1 mm.
const CHORD: Length = Length::from_mm(1);

/// A datum's body: its operation and arguments, in this order and sorted, whatever else the JSON
/// library is set to.
#[derive(Serialize)]
struct Body<'a> {
    args: &'a BTreeMap<String, serde_json::Value>,
    datum: &'a str,
}

fn fault(code: &str, message: impl Into<String>) -> GeometryError {
    GeometryError {
        code: code.to_owned(),
        message: message.into(),
    }
}

/// A length in whole millimetres, the nearest, a half rounding up.
fn nearest_mm(length: Length) -> Result<i64, GeometryError> {
    length
        .steps()
        .checked_mul(2)
        .and_then(|doubled| doubled.checked_add(Length::STEPS_PER_MM))
        .and_then(|up| up.checked_div_euclid(Length::STEPS_PER_MM.checked_mul(2)?))
        .ok_or_else(|| {
            fault(
                "OutOfRange",
                "a datum is further out than the model reaches",
            )
        })
}

fn outline_mm(points: &[Point]) -> Result<Vec<[i64; 2]>, GeometryError> {
    points
        .iter()
        .map(|p| Ok([nearest_mm(p.x)?, nearest_mm(p.y)?]))
        .collect()
}

/// A ribbon standing on a chain of points: two vertices for each, and two triangles for each piece.
fn ribbon(outline: &[[i64; 2]]) -> Mesh {
    let mut positions = Vec::new();
    for [x, y] in outline {
        positions.extend([*x, *y, 0, *x, *y, RIBBON_MM]);
    }
    let mut indices = Vec::new();
    let mut base: u32 = 0;
    for _ in outline.iter().skip(1) {
        // The piece's two bottom and two top corners: (a, b) on this point, (c, d) on the next.
        let (a, b, c, d) = (
            base,
            base.saturating_add(1),
            base.saturating_add(2),
            base.saturating_add(3),
        );
        indices.extend([a, c, d, a, d, b]);
        base = base.saturating_add(2);
    }
    Mesh { positions, indices }
}

fn standing(points: &[Point], deviation: i64) -> Result<(Vec<Display>, Bounds), GeometryError> {
    let outline = outline_mm(points)?;
    let mut min = [i64::MAX, i64::MAX];
    let mut max = [i64::MIN, i64::MIN];
    for [x, y] in &outline {
        min = [min[0].min(*x), min[1].min(*y)];
        max = [max[0].max(*x), max[1].max(*y)];
    }
    let displays = (0..2)
        .map(|lod| Display {
            lod,
            deviation,
            mesh: ribbon(&outline),
            outline: outline.clone(),
        })
        .collect();
    let bounds = Bounds {
        min: [min[0], min[1], 0],
        max: [max[0], max[1], RIBBON_MM],
    };
    Ok((displays, bounds))
}

fn level_at(elevation: Length) -> Result<(Vec<Display>, Bounds), GeometryError> {
    let z = nearest_mm(elevation)?;
    let displays = (0..2)
        .map(|lod| Display {
            lod,
            deviation: 0,
            mesh: Mesh {
                positions: Vec::new(),
                indices: Vec::new(),
            },
            outline: Vec::new(),
        })
        .collect();
    Ok((
        displays,
        Bounds {
            min: [0, 0, z],
            max: [0, 0, z],
        },
    ))
}

/// Builds a datum's display shape, if the recipe is a datum's.
#[must_use]
pub fn build(recipe: &Recipe) -> Option<Result<Built, GeometryError>> {
    let datum = datum_of_recipe(recipe)?;
    Some(
        datum
            .map_err(|e| fault("BadDatum", e.to_string()))
            .and_then(|datum| {
                let (displays, bounds) = match datum {
                    Datum::Grid(shape) => {
                        let deviation = if matches!(shape, Shape::Line { .. }) {
                            1
                        } else {
                            2
                        };
                        standing(&shape.polyline(CHORD), deviation)?
                    }
                    Datum::Plane(PlaneShape::Vertical { from, to }) => standing(&[from, to], 1)?,
                    Datum::Plane(PlaneShape::Horizontal { elevation }) => level_at(elevation)?,
                };
                let body = Body {
                    args: &recipe.args,
                    datum: &recipe.op,
                };
                Ok(Built {
                    body: serde_json::to_vec(&body).unwrap_or_default(),
                    bounds,
                    names: Vec::new(),
                    displays,
                })
            }),
    )
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    reason = "tests; the rules are for the crate's own code"
)]
mod tests {
    use super::*;
    use construct_grids::{Grid, ReferencePlane};
    use construct_params::Angle;
    use kit_core::Id;

    fn id(text: &str) -> Id {
        Id::parse(text).unwrap()
    }

    fn built(recipe: &Recipe) -> Built {
        build(recipe).unwrap().unwrap()
    }

    #[test]
    fn a_straight_grid_is_its_two_points_standing_up() {
        let shape = Shape::line(Point::mm(0, 0), Point::mm(6000, 0)).unwrap();
        let grid = Grid::new(id("G-A"), "A", shape).unwrap();
        let shape = built(&grid.to_record(&id("DOC-1")).recipe);
        assert_eq!(shape.displays[0].outline, vec![[0, 0], [6000, 0]]);
        assert_eq!(shape.bounds.min, [0, 0, 0]);
        assert_eq!(shape.bounds.max, [6000, 0, RIBBON_MM]);
        assert_eq!(shape.displays[0].mesh.indices, vec![0, 2, 3, 0, 3, 1]);
        assert_eq!(shape.names.len(), 0);
    }

    #[test]
    fn an_arc_is_a_chain_of_chords() {
        let arc = Shape::arc(
            Point::mm(0, 0),
            Length::from_mm(1000),
            Angle::from_units(0),
            Angle::from_units(90 * Angle::UNITS_PER_DEGREE),
        )
        .unwrap();
        let grid = Grid::new(id("G-B"), "B", arc).unwrap();
        let shape = built(&grid.to_record(&id("DOC-1")).recipe);
        let outline = &shape.displays[0].outline;
        assert_eq!(outline.len(), 19);
        assert_eq!(outline.first(), Some(&[1000, 0]));
        assert_eq!(outline.last(), Some(&[0, 1000]));
        assert_eq!(shape.displays[0].deviation, 2);
        assert_eq!(shape.displays[0].mesh.positions.len(), 19 * 6);
    }

    #[test]
    fn a_horizontal_plane_is_a_point_at_its_elevation_with_nothing_to_draw() {
        let level = PlaneShape::Horizontal {
            elevation: Length::from_mm(-300),
        };
        let plane = ReferencePlane::new(id("P-1"), None, level).unwrap();
        let shape = built(&plane.to_record(&id("DOC-1")).recipe);
        assert_eq!(shape.bounds.min, [0, 0, -300]);
        assert_eq!(shape.bounds.max, [0, 0, -300]);
        assert!(shape.displays[0].outline.is_empty() && shape.displays[0].mesh.indices.is_empty());
    }

    #[test]
    fn the_body_is_the_exact_recipe_and_the_same_every_time() {
        let shape = Shape::line(Point::mm(0, 0), Point::mm(6000, 0)).unwrap();
        let recipe = Grid::new(id("G-A"), "A", shape)
            .unwrap()
            .to_record(&id("DOC-1"))
            .recipe;
        let (one, two) = (built(&recipe), built(&recipe));
        assert_eq!(one.body_id(), two.body_id());
        assert_eq!(
            String::from_utf8(one.body).unwrap(),
            concat!(
                r#"{"args":{"x0":"length:q0:0","x1":"length:q0:38400000","#,
                r#""y0":"length:q0:0","y1":"length:q0:0"},"datum":"grid.line"}"#
            )
        );
    }

    #[test]
    fn another_recipe_is_not_a_datums_and_a_bad_one_is_a_failure() {
        assert!(build(&Recipe::construct("wall.straight", BTreeMap::new())).is_none());
        let mut bad = Recipe::construct("grid.line", BTreeMap::new());
        bad.args
            .insert("x0".to_owned(), serde_json::json!("length:q0:0"));
        assert_eq!(build(&bad).unwrap().unwrap_err().code, "BadDatum");
    }
}
