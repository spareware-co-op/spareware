//! A shape-edited floor's layers on the engine (job 14.15, CON-A4-F4): the layers whose faces
//! follow the top face's triangles ([`FloorPlan::faces`], construct-slabs' `Top`).
//!
//! What it builds, for one layer of the floor's type:
//!
//! - **Follows** ([`Stand::Follows`]: every layer of a type with no variable layer, and the layers
//!   above the variable one): each triangle of the top face let down by the layer's depth, lofted
//!   straight up to the same triangle let down by the depth of its top ([`Engine::loft`]: two
//!   parallel flat sections, every wall upright and flat), so the layer keeps its thickness, plumb,
//!   under every point of the top.
//! - **Fills** ([`Stand::Fills`]: the variable layer): each triangle flat at the layer's plain
//!   bottom lofted up to the triangle let down by the depth of its top, so the layer is thicker
//!   where the top is raised and thinner where it is lowered, and everything below it stays flat.
//!   construct-slabs' check that the variable layer keeps a thickness above zero under every point
//!   is what keeps each loft's top wholly above its bottom, as a loft needs.
//! - **Flat** ([`Stand::Flat`]: the layers below the variable one): left to the floor's own
//!   extrusion, as on a plain floor.
//!
//! The triangles' pieces are united in the order the top face lists them ([`Engine::unite`]), each
//! sharing a whole upright side with one before it, so the union stays one solid, as a solved
//! roof's flat pieces are (the engine answers an extrusion of a profile that is not level with a
//! curved surface, so no layer is asked for as one). Holes in the floor are where no triangle is.
//!
//! Depends on the floor builder's units and errors and [`crate::floor_plan`]. Interim: as the
//! parent module's (the engine behind the seam; the interim register's row for floor layers).

use super::super::{Units, as_steps, from_engine, too_big};
use crate::floor_plan::FloorPlan;
use construct_model::geometry::GeometryError;
use engine_geometry::value::engine::{Engine, Section};
use engine_geometry::value::grid::P3;

/// How one layer of a shape-edited floor stands under its top face.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Stand {
    /// It keeps its thickness and follows the top face.
    Follows,
    /// It is the variable layer: it fills from its flat bottom up to the layers that follow.
    Fills,
    /// It is below the variable layer: flat, where it is on a plain floor.
    Flat,
}

/// How layer `at` (its index from the top) of a shape-edited `plan` stands; every layer of any
/// other floor is [`Stand::Flat`].
pub(super) fn stand(plan: &FloorPlan, at: usize) -> Stand {
    if plan.faces.is_empty() {
        return Stand::Flat;
    }
    match plan.variable {
        None => Stand::Follows,
        Some(v) if at < v => Stand::Follows,
        Some(v) if at == v => Stand::Fills,
        Some(_) => Stand::Flat,
    }
}

/// One triangle's corners as grid points of the engine, each at `z(height)` (grid steps).
fn corners(
    face: &[[i64; 3]; 3],
    units: Units,
    z: impl Fn(i64) -> Option<i64>,
) -> Result<Vec<P3>, GeometryError> {
    face.iter()
        .map(|&[x, y, height]| {
            let up = z(height).ok_or_else(too_big)?;
            P3::new(units.from_q0(x)?, units.from_q0(y)?, units.from_q0(up)?).map_err(|_| too_big())
        })
        .collect()
}

/// A layer that follows or fills, between `bottom` and `top` (its faces' depths below the top
/// face, grid steps, at or below zero), as the module header says; each loft as the engine was
/// asked for it is pushed to `asked`, its two sections in engine steps.
pub(super) fn layer<E: Engine>(
    engine: &mut E,
    plan: &FloorPlan,
    units: Units,
    (bottom, top): (i64, i64),
    stand: Stand,
    asked: &mut Vec<[Vec<[i64; 3]>; 2]>,
) -> Result<E::Solid, GeometryError> {
    let mut solid: Option<E::Solid> = None;
    for face in &plan.faces {
        let lower = match stand {
            Stand::Fills => corners(face, units, |_| Some(bottom))?,
            _ => corners(face, units, |h| h.checked_add(bottom))?,
        };
        let upper = corners(face, units, |h| h.checked_add(top))?;
        asked.push([as_steps(&lower), as_steps(&upper)]);
        let piece = engine
            .loft(&[Section { corners: lower }, Section { corners: upper }])
            .map_err(from_engine)?;
        solid = Some(match solid {
            None => piece,
            Some(so_far) => engine.unite(&so_far, &piece).map_err(from_engine)?,
        });
    }
    solid.ok_or_else(|| super::super::fault("EmptyShape", "a shape-edited floor with no faces"))
}
