//! A column on the engine (job 14.7, CON-A5-F1-P1): one solid, the extrusion of its ring from its
//! base at `z = 0` straight up to its height ([`crate::column_plan`]), through
//! [`Engine::extrude`]. The solid is the engine's, so the kernel's checker has passed it, as the
//! seam's contract says.
//!
//! - Names (chapter 20.2), under the recipe's operation: `bottom`, `top`, and `side.<k>` for each
//!   edge of the ring in order ([`ColumnPlan::roles`]).
//! - The body's bytes: what the engine was asked (the ring and the vector, in the engine's steps).
//! - Displays: levels 0 and 1 the same mesh, in whole millimetres, each coordinate the nearest
//!   millimetre to the engine's grid point; a level that rounded anything strays by 1 mm. One part,
//!   of the column's material and the function `structure`, so a page colours it as a wall's layer.
//!   The outline is the ring.
//!
//! Depends on the parent module's units, naming and errors, its child `floor`'s mesh helpers, and
//! [`crate::column_plan`]. Interim: as the parent module's (the engine behind the seam, the body's
//! bytes being the engine's request, names written from the recipe, not read from the body); a
//! round section is a polygon (construct-columns, the interim register's row) until the kernel's
//! exact sweep takes arcs.

use super::floor::{Extent, append};
use super::{Kernel, as_steps, fault, from_engine, name, op_id, too_big};
use crate::column_plan::ColumnPlan;
use crate::floor_plan;
use construct_model::geometry::{
    Bounds, Built, Display, FaceName, GeometryError, Mesh, Part, Recipe,
};
use engine_geometry::value::engine::{Engine, Profile};
use engine_geometry::value::grid::P3;
use engine_geometry::value::op::OpId;
use serde::Serialize;

/// A column's body bytes: what the engine was asked.
#[derive(Serialize)]
struct Asked {
    extrude: Vec<[i64; 3]>,
    by: [i64; 3],
}

/// The names of a column's faces, sorted by name.
pub(super) fn names(op: OpId, plan: &ColumnPlan) -> Result<Vec<FaceName>, GeometryError> {
    let mut names = plan
        .roles()
        .iter()
        .map(|role| name(op, role))
        .collect::<Result<Vec<_>, _>>()?;
    names.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(names)
}

impl<E: Engine> Kernel<E> {
    /// Builds a filled column recipe, as the module header says.
    ///
    /// # Errors
    ///
    /// [`ColumnPlan::of`]'s refusals; `NotAHost` when anything is hosted on it; `NotOnTheGrid` for
    /// a length that is not whole steps of the engine; and the engine's own.
    pub(super) fn build_column(
        &self,
        recipe: &Recipe,
        hosted: usize,
    ) -> Result<Built, GeometryError> {
        let plan = ColumnPlan::of(recipe)?;
        if hosted > 0 {
            return Err(fault("NotAHost", "nothing is hosted on a column"));
        }
        let units = self.units;
        let op = op_id(recipe)?;
        let outer = plan
            .ring
            .iter()
            .map(|&[x, y]| P3::new(units.from_q0(x)?, units.from_q0(y)?, 0).map_err(|_| too_big()))
            .collect::<Result<Vec<_>, _>>()?;
        let by = [0, 0, units.from_q0(plan.height)?];
        let asked = Asked {
            extrude: as_steps(&outer),
            by,
        };
        let mut mesh = Mesh {
            positions: Vec::new(),
            indices: Vec::new(),
        };
        let mut extent: Extent = None;
        let (rounded, triangles) = {
            let mut engine = self
                .engine
                .lock()
                .map_err(|_| fault("EngineUnavailable", "an earlier build stopped part way"))?;
            let solid = engine
                .extrude(
                    &Profile {
                        outer,
                        holes: Vec::new(),
                    },
                    by,
                )
                .map_err(from_engine)?;
            let shown = engine.mesh(&solid).map_err(from_engine)?;
            let appended = append(&mut mesh, &shown, units, &mut extent)?;
            (appended.rounded, appended.triangles)
        };
        let (lo, hi) = extent.ok_or_else(|| fault("EmptyShape", "a column with no volume"))?;
        let bounds = Bounds {
            min: lo.map(|s| units.floor_mm(s)),
            max: hi.map(|s| units.ceil_mm(s)),
        };
        let outline: Vec<[i64; 2]> = plan
            .ring
            .iter()
            .map(|&[x, y]| [floor_plan::nearest_mm(x).0, floor_plan::nearest_mm(y).0])
            .collect();
        let deviation = i64::from(rounded);
        let part = Part {
            material: plan.material.clone(),
            function: "structure".to_owned(),
            triangles,
            outline: outline.clone(),
        };
        let displays = (0..2)
            .map(|lod| Display {
                lod,
                deviation,
                mesh: mesh.clone(),
                outline: outline.clone(),
                parts: vec![part.clone()],
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
