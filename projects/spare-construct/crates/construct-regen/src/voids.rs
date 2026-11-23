//! The display shape of an opening and a shaft (job 4.2, CON-A4-F3-P2): what the plan and the 3D
//! view draw of a void, which has no solid of its own. What it cuts from its host is the host's
//! builder's: the kernel cuts a wall opening on the door's path ([`crate::kernel`]'s `opening`)
//! and a floor's openings and shafts as holes through every layer (its `floor`).
//!
//! What it promises:
//! - [`build`] answers for construct-openings' three operations (`opening.wall`,
//!   `opening.floor`, `shaft`) and for no other (`None`), so each builder hands a recipe here
//!   first, as it does a datum's ([`crate::datum`]): a void is drawn the same whichever builds the
//!   rest, the stand-in ([`crate::stand_in`]) or the kernel's.
//! - The body is the recipe's own operation and arguments as canonical JSON: the void exactly, in
//!   stored text, with no float in it, so its content ID is the same on the server and in the
//!   browser.
//! - A wall opening is drawn as the rectangle it opens, standing in its wall's plane at its place
//!   (its width along its wall's axis, its height up), with no thickness: its outline in plan is
//!   the line along its wall. A floor opening is its outline in plan, with no mesh. A shaft is its
//!   outline in plan and a ribbon standing on it from its base up to its top, with no thickness.
//!   Each point is the nearest whole millimetre, and a display that rounded anything says it
//!   strays by 1 mm; the bounds are rounded outward.
//! - It has no faces to name: a void has no solid.
//! - A recipe that does not read is refused with construct-openings' code and words, naming the
//!   element `this opening` or `this shaft`: a builder is handed a recipe, not its record, and
//!   the refusal stands in that element's own built row.
//!
//! Depends on construct-openings (the recipes) and the rounding of [`crate::wall_plan`]. Nothing
//! here is interim but what construct-openings refuses (a curved outline).

use crate::wall_plan::nearest_mm;
use construct_model::geometry::{Bounds, Built, Display, GeometryError, Mesh, Recipe};
use construct_openings::{OpeningError, Shaft, WallOpening, is_void, outline_of};
use construct_params::Length;
use serde::Serialize;
use std::collections::BTreeMap;

/// A void's body: its operation and arguments, sorted.
#[derive(Serialize)]
struct Body<'a> {
    args: &'a BTreeMap<String, serde_json::Value>,
    void: &'a str,
}

/// The key a void is named by in a refusal from its builder, which is handed no record: the
/// refusal then names it `this opening` or `this shaft`, in its own built row.
const UNNAMED: &str = "";

fn refusal(error: &OpeningError) -> GeometryError {
    GeometryError {
        code: error.code().to_owned(),
        message: error.to_string(),
    }
}

/// Corners and heights in grid steps as whole millimetres, each the nearest, and whether any was
/// rounded.
struct Rounder {
    rounded: bool,
}

impl Rounder {
    fn mm(&mut self, steps: i64) -> i64 {
        let (mm, exact) = nearest_mm(steps);
        self.rounded |= !exact;
        mm
    }
}

/// The box around `points` from `low` up to `high`, in whole millimetres, rounded outward.
fn bounds_of(points: &[[i64; 2]], low: i64, high: i64) -> Bounds {
    let mut min = [i64::MAX, i64::MAX, low];
    let mut max = [i64::MIN, i64::MIN, high];
    for &[x, y] in points {
        min = [min[0].min(x), min[1].min(y), low];
        max = [max[0].max(x), max[1].max(y), high];
    }
    if points.is_empty() {
        (min[0], min[1], max[0], max[1]) = (0, 0, 0, 0);
    }
    Bounds { min, max }
}

/// A ribbon standing on a closed loop of points from `0` up to `up` millimetres: two vertices for
/// each point, two triangles for each edge, the last edge back to the first point.
fn ribbon(points: &[[i64; 2]], up: i64) -> Mesh {
    let mut positions = Vec::with_capacity(points.len().saturating_mul(6));
    for &[x, y] in points {
        positions.extend([x, y, 0, x, y, up]);
    }
    let count = u32::try_from(points.len()).unwrap_or(0);
    let mut indices = Vec::new();
    for at in 0..count {
        let next = at.saturating_add(1).checked_rem(count).unwrap_or(0);
        let (a, b) = (at.saturating_mul(2), at.saturating_mul(2).saturating_add(1));
        let (c, d) = (
            next.saturating_mul(2),
            next.saturating_mul(2).saturating_add(1),
        );
        indices.extend([a, c, d, a, d, b]);
    }
    Mesh { positions, indices }
}

/// Both levels of detail of one drawing: a void has no coarser form.
fn both(mesh: &Mesh, outline: &[[i64; 2]], deviation: i64) -> Vec<Display> {
    (0..2)
        .map(|lod| Display {
            lod,
            deviation,
            mesh: mesh.clone(),
            outline: outline.to_vec(),
            parts: Vec::new(),
        })
        .collect()
}

/// The display shape of a void's recipe, or `None` for any other recipe.
#[must_use]
pub fn build(recipe: &Recipe) -> Option<Result<Built, GeometryError>> {
    is_void(recipe).then(|| made(recipe))
}

fn made(recipe: &Recipe) -> Result<Built, GeometryError> {
    let body = serde_json::to_vec(&Body {
        args: &recipe.args,
        void: &recipe.op,
    })
    .unwrap_or_default();
    let mut round = Rounder { rounded: false };
    let (mesh, outline, bounds) = if recipe.op == construct_openings::WALL_OP {
        let opening = WallOpening::of(UNNAMED, recipe).map_err(|e| refusal(&e))?;
        let (w, h) = (
            round.mm(opening.width.steps()),
            round.mm(opening.height.steps()),
        );
        let (end, positions) = if opening.along_y {
            ([0, w], vec![0, 0, 0, 0, w, 0, 0, w, h, 0, 0, h])
        } else {
            ([w, 0], vec![0, 0, 0, w, 0, 0, w, 0, h, 0, 0, h])
        };
        let mesh = Mesh {
            positions,
            indices: vec![0, 1, 2, 0, 2, 3],
        };
        let outline = vec![[0, 0], end];
        let bounds = bounds_of(&outline, 0, h);
        (mesh, outline, bounds)
    } else {
        let (shape, up) = if recipe.op == construct_openings::SHAFT_OP {
            let shaft = Shaft::of(UNNAMED, recipe).map_err(|e| refusal(&e))?;
            (shaft.outline, Some(shaft.height))
        } else {
            (outline_of(UNNAMED, recipe).map_err(|e| refusal(&e))?, None)
        };
        let outline: Vec<[i64; 2]> = shape
            .corners()
            .iter()
            .map(|[x, y]| [round.mm(x.steps()), round.mm(y.steps())])
            .collect();
        let top = up.map_or(0, |height: Length| round.mm(height.steps()));
        let mesh = if up.is_some() {
            ribbon(&outline, top)
        } else {
            Mesh {
                positions: Vec::new(),
                indices: Vec::new(),
            }
        };
        let bounds = bounds_of(&outline, 0, top);
        (mesh, outline, bounds)
    };
    Ok(Built {
        body,
        bounds,
        names: Vec::new(),
        displays: both(&mesh, &outline, i64::from(round.rounded)),
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
    use construct_openings::{Named, Outline, floor_opening_recipe, shaft_recipe};
    use construct_params::Value as Stored;
    use serde_json::json;

    fn mm(v: i64) -> Length {
        Length::checked_from_mm(v).unwrap()
    }

    /// A wall opening is drawn as its rectangle standing on its wall's line; a floor opening as
    /// its outline; a shaft as its outline and a ribbon up to its top. None has a face to name,
    /// and the same recipe gives the same bytes.
    #[test]
    fn a_void_is_drawn_as_its_outline() {
        let wall = WallOpening::new(&Named::opening("O-1"), mm(900), mm(1200), true)
            .unwrap()
            .recipe();
        let built = build(&wall).unwrap().unwrap();
        assert_eq!(built.names.len(), 0);
        assert_eq!(built.displays[0].outline, [[0, 0], [0, 900]]);
        assert_eq!(built.bounds.max, [0, 900, 1200]);
        assert_eq!(built.displays[0].deviation, 0);
        assert_eq!(build(&wall).unwrap().unwrap().body, built.body);

        let square = Outline::from_mm(
            &Named::opening("O-2"),
            &[[0, 0], [1000, 0], [1000, 800], [0, 800]],
        )
        .unwrap();
        let hole = build(&floor_opening_recipe(&square)).unwrap().unwrap();
        assert_eq!(hole.displays[0].mesh.positions.len(), 0);
        assert_eq!(hole.displays[1].outline.len(), 4);
        assert_eq!(hole.bounds.max, [1000, 800, 0]);

        let mut shaft = shaft_recipe(&square);
        let unfilled = build(&shaft).unwrap().unwrap_err();
        assert_eq!(unfilled.code, "BadOpening");
        shaft.args.insert(
            "height".to_owned(),
            json!(Stored::Length(Length::from_steps(6400 * 3000 + 3200)).stored()),
        );
        let lift = build(&shaft).unwrap().unwrap();
        assert_eq!(lift.bounds.max, [1000, 800, 3001]);
        assert_eq!(lift.displays[0].deviation, 1);
        assert_eq!(lift.displays[0].mesh.indices.len(), 4 * 6);
        assert!(build(&Recipe::construct("block", BTreeMap::new())).is_none());
    }
}
