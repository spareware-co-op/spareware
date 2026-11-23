//! A room on the engine (job 5.4, CON-A4-F7-P1): its volume, one solid, the extrusion of its
//! outer loop with every hole in it from its base at `z = 0` straight up to its height
//! ([`crate::room_plan`]), through [`Engine::extrude`]. The solid is the engine's, so the kernel's
//! checker has passed it, as the seam's contract says.
//!
//! - Names (chapter 20.2), under the recipe's operation: `bottom`, `top`, and `side.<k>` for each
//!   edge of each loop in loop order ([`RoomPlan::roles`]).
//! - The body's bytes: what the engine was asked (the outer loop, the holes and the vector, in the
//!   engine's steps).
//! - Displays: levels 0 and 1 the same mesh, in whole millimetres, each coordinate the nearest
//!   millimetre to the engine's grid point, as a floor's; a level that rounded anything strays by
//!   1 mm. No parts: a room is space, of no material. The outline is the outer loop.
//!
//! Depends on the parent module's units, naming and errors, its child `floor`'s mesh helpers, and
//! [`crate::room_plan`]. Interim: as the parent module's (the engine behind the seam, the body's
//! bytes being the engine's request, names written from the recipe, not read from the body).

use super::floor::{Extent, append, ring};
use super::{Kernel, as_steps, fault, from_engine, name, op_id};
use crate::floor_plan;
use crate::room_plan::RoomPlan;
use construct_model::geometry::{Bounds, Built, Display, FaceName, GeometryError, Mesh, Recipe};
use engine_geometry::value::engine::{Engine, Profile};
use engine_geometry::value::op::OpId;
use serde::Serialize;

/// A room's body bytes: what the engine was asked. A room without holes writes no `holes`.
#[derive(Serialize)]
struct Asked {
    extrude: Vec<[i64; 3]>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    holes: Vec<Vec<[i64; 3]>>,
    by: [i64; 3],
}

/// The names of a room's faces, sorted by name.
pub(super) fn names(op: OpId, plan: &RoomPlan) -> Result<Vec<FaceName>, GeometryError> {
    let mut names = plan
        .roles()
        .iter()
        .map(|role| name(op, role))
        .collect::<Result<Vec<_>, _>>()?;
    names.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(names)
}

impl<E: Engine> Kernel<E> {
    /// Builds a filled room recipe, as the module header says.
    ///
    /// # Errors
    ///
    /// [`RoomPlan::of`]'s refusals; `NotAHost` when anything is hosted on it; `NotOnTheGrid` for a
    /// length that is not whole steps of the engine; and the engine's own.
    pub(super) fn build_room(
        &self,
        recipe: &Recipe,
        hosted: usize,
    ) -> Result<Built, GeometryError> {
        let plan = RoomPlan::of(recipe)?;
        if hosted > 0 {
            return Err(fault("NotAHost", "nothing is hosted on a room"));
        }
        let units = self.units;
        let op = op_id(recipe)?;
        let outer = ring(plan.outer(), units, 0)?;
        let holes = plan
            .holes()
            .iter()
            .map(|hole| ring(hole, units, 0))
            .collect::<Result<Vec<_>, _>>()?;
        let by = [0, 0, units.from_q0(plan.height)?];
        let asked = Asked {
            extrude: as_steps(&outer),
            holes: holes.iter().map(|h| as_steps(h)).collect(),
            by,
        };
        let mut mesh = Mesh {
            positions: Vec::new(),
            indices: Vec::new(),
        };
        let mut extent: Extent = None;
        let rounded = {
            let mut engine = self
                .engine
                .lock()
                .map_err(|_| fault("EngineUnavailable", "an earlier build stopped part way"))?;
            let solid = engine
                .extrude(&Profile { outer, holes }, by)
                .map_err(from_engine)?;
            let shown = engine.mesh(&solid).map_err(from_engine)?;
            append(&mut mesh, &shown, units, &mut extent)?.rounded
        };
        let (lo, hi) = extent.ok_or_else(|| fault("EmptyShape", "a room with no volume"))?;
        let bounds = Bounds {
            min: lo.map(|s| units.floor_mm(s)),
            max: hi.map(|s| units.ceil_mm(s)),
        };
        let outline: Vec<[i64; 2]> = plan
            .outer()
            .iter()
            .map(|corner| {
                let [x, y] = corner.at;
                [floor_plan::nearest_mm(x).0, floor_plan::nearest_mm(y).0]
            })
            .collect();
        let deviation = i64::from(rounded);
        let displays = (0..2)
            .map(|lod| Display {
                lod,
                deviation,
                mesh: mesh.clone(),
                outline: outline.clone(),
                parts: Vec::new(),
            })
            .collect();
        Ok(Built {
            body: serde_json::to_vec(&asked).unwrap_or_default(),
            bounds,
            names: names(op, &plan)?,
            displays,
        })
    }
}
