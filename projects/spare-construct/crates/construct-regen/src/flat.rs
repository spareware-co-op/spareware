//! The display shape of a room separation line, an area boundary line and an area (jobs 6.3 and
//! 14.6, CON-A4-F7-P1 and CON-A4-F7-P2): flat things in plan, with no solid, which a plan draws and
//! the rooms and areas on their level read.
//!
//! What it promises:
//! - [`build`] answers for construct-rooms' three flat operations (`room.line`, `area.line`,
//!   `area`) and for no other (`None`), so each builder hands a recipe here first, as it does a
//!   datum's ([`crate::datum`]) and a void's ([`crate::voids`]): a flat thing is the same whichever
//!   builds the rest.
//! - The body is the recipe's own operation and arguments as canonical JSON: a line's two ends, an
//!   area's loops, exactly in stored text or grid steps, with no float, so its content ID is the
//!   same on the server and in the browser.
//! - A line's display is its two ends and an area's the outer loop of its filled recipe, in whole
//!   millimetres, each the nearest, a display that rounded anything saying it strays by 1 mm; no
//!   mesh (a flat thing has no surface to light) and no thickness; the bounds are rounded outward.
//! - It has no faces to name: there is no solid.
//! - A recipe that does not read is refused with construct-rooms' code and words; an area not yet
//!   filled with its loop is `Unfilled`, as a room is.
//!
//! Depends on construct-rooms (the recipes) and the rounding of [`crate::wall_plan`]. Nothing here
//! is interim.

use crate::wall_plan::nearest_mm;
use construct_model::geometry::{Bounds, Built, Display, GeometryError, Mesh, Recipe};
use construct_rooms::{LOOPS_ARG, ends_of, is_area_recipe, is_line_recipe};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// A flat thing's body: its operation and arguments, sorted.
#[derive(Serialize)]
struct Body<'a> {
    args: &'a BTreeMap<String, Value>,
    flat: &'a str,
}

fn fault(code: &str, message: impl Into<String>) -> GeometryError {
    GeometryError {
        code: code.to_owned(),
        message: message.into(),
    }
}

/// Whether a recipe is a flat thing's: a line of either kind, or an area.
#[must_use]
pub fn is_flat(recipe: &Recipe) -> bool {
    is_line_recipe(recipe) || is_area_recipe(recipe)
}

/// The outer loop of an area's filled recipe, in grid steps.
fn outer_loop(recipe: &Recipe) -> Result<Vec<[i64; 2]>, GeometryError> {
    let loops = recipe
        .args
        .get(LOOPS_ARG)
        .and_then(Value::as_array)
        .ok_or_else(|| {
            fault(
                "Unfilled",
                "its loop is found from the area boundary lines round it, and was not found before building",
            )
        })?;
    let bad = || {
        fault(
            "MissingArgument",
            "its loop must be a list of corners {\"at\":[x,y],\"wall\"} in grid steps",
        )
    };
    let outer = loops.first().and_then(Value::as_array).ok_or_else(bad)?;
    if outer.len() < 3 {
        return Err(fault(
            "EmptyShape",
            "an area has an outer loop of three corners or more",
        ));
    }
    outer
        .iter()
        .map(|corner| {
            let at = corner.get("at").and_then(Value::as_array).ok_or_else(bad)?;
            match (
                at.first().and_then(Value::as_i64),
                at.get(1).and_then(Value::as_i64),
            ) {
                (Some(x), Some(y)) => Ok([x, y]),
                _ => Err(bad()),
            }
        })
        .collect()
}

/// The display shape of a flat recipe, or `None` for any other recipe.
#[must_use]
pub fn build(recipe: &Recipe) -> Option<Result<Built, GeometryError>> {
    is_flat(recipe).then(|| made(recipe))
}

fn made(recipe: &Recipe) -> Result<Built, GeometryError> {
    let points: Vec<[i64; 2]> = if is_line_recipe(recipe) {
        ends_of(recipe)
            .map_err(|e| fault(e.code(), e.to_string()))?
            .to_vec()
    } else {
        outer_loop(recipe)?
    };
    let mut rounded = false;
    let mut mm = |steps: i64| {
        let (value, exact) = nearest_mm(steps);
        rounded |= !exact;
        value
    };
    let outline: Vec<[i64; 2]> = points.iter().map(|&[x, y]| [mm(x), mm(y)]).collect();
    let mut min = [i64::MAX, i64::MAX, 0];
    let mut max = [i64::MIN, i64::MIN, 0];
    for &[x, y] in &outline {
        min = [min[0].min(x), min[1].min(y), 0];
        max = [max[0].max(x), max[1].max(y), 0];
    }
    let displays = (0..2)
        .map(|lod| Display {
            lod,
            deviation: i64::from(rounded),
            mesh: Mesh {
                positions: Vec::new(),
                indices: Vec::new(),
            },
            outline: outline.clone(),
            parts: Vec::new(),
        })
        .collect();
    let body = serde_json::to_vec(&Body {
        args: &recipe.args,
        flat: &recipe.op,
    })
    .unwrap_or_default();
    Ok(Built {
        body,
        bounds: Bounds { min, max },
        names: Vec::new(),
        displays,
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]
mod tests {
    use super::*;
    use construct_params::Length;
    use construct_rooms::{LineKind, area_recipe, line_recipe};
    use serde_json::json;

    fn mm(v: i64) -> Length {
        Length::checked_from_mm(v).unwrap()
    }

    /// A line is drawn as its two ends; an unfilled area is refused; a filled one is drawn as its
    /// outer loop. None has a mesh or a face, and the same recipe gives the same bytes.
    #[test]
    fn a_line_and_an_area_are_drawn_as_their_outlines() {
        let line = line_recipe(LineKind::Room, [mm(0), mm(0)], [mm(4000), mm(1500)]).unwrap();
        let built = build(&line).unwrap().unwrap();
        assert_eq!(built.displays[0].outline, [[0, 0], [4000, 1500]]);
        assert_eq!(built.displays[0].deviation, 0);
        assert!(built.names.is_empty() && built.displays[0].mesh.positions.is_empty());
        assert_eq!(built.bounds.max, [4000, 1500, 0]);
        assert_eq!(build(&line).unwrap().unwrap().body, built.body);

        let mut area = area_recipe([mm(100), mm(100)]);
        assert_eq!(build(&area).unwrap().unwrap_err().code, "Unfilled");
        let step = Length::STEPS_PER_MM;
        area.args.clear();
        area.args.insert(
            "loops".to_owned(),
            json!([[
                {"at": [0, 0], "wall": "a"},
                {"at": [5000 * step, 0], "wall": "b"},
                {"at": [5000 * step, 3000 * step + 1], "wall": "c"},
                {"at": [0, 3000 * step], "wall": "d"},
            ]]),
        );
        let drawn = build(&area).unwrap().unwrap();
        assert_eq!(drawn.displays[1].outline.len(), 4);
        assert_eq!(drawn.bounds.max, [5000, 3000, 0]);
        // One step over a whole millimetre is rounded, and the display says so.
        assert_eq!(drawn.displays[0].deviation, 1);
        assert!(build(&Recipe::construct("block", BTreeMap::new())).is_none());
    }
}
