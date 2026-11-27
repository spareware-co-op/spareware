//! A stair on the engine (job 6.7, CON-A4-F5-P1): one solid per piece of a filled `stair` recipe
//! ([`crate::stair_plan`]), each the solid construct-stairs worked out for it, asked of the engine
//! through its trait alone.
//!
//! What it builds:
//!
//! - A solid for each run: its side profile at its right-hand side lofted to the same profile at
//!   its left ([`Engine::loft`]: every wall of such a loft is flat, each joining two corners of
//!   one section to the same two of the other, the way a sloped roof's layer is built).
//! - A solid for each landing: its outline at its underside extruded straight up by its waist
//!   ([`Engine::extrude`]).
//! - Each solid is the engine's, so the kernel's checker has passed it, as the seam's contract
//!   says. The pieces touch where a run meets a landing and are not united.
//! - Names (chapter 20.2), under the recipe's operation as every shape's are, so a stair rebuilt
//!   or reshaped keeps them: the roles [`StairPlan::roles`] lists.
//! - The body's bytes: what the engine was asked for each piece (its ID, and its two sections or
//!   its outline and vector, in the engine's steps).
//! - Displays: levels 0 and 1 the same mesh, the pieces' meshes one after another, in whole
//!   millimetres, each coordinate the nearest millimetre to the engine's grid point; a level that
//!   rounded anything strays by 1 mm. Each piece is a part ([`Part`]): the stair's material,
//!   function `structure`, its triangles and its plan outline, so a page colours it as any
//!   element's material. The outline is the first run's plan. The bounds are rounded outward.
//!
//! Depends on the parent module's units, naming and errors, its `floor` child's mesh helpers, and
//! [`crate::stair_plan`]. Interim: as the parent module's (the engine behind the seam, the body's
//! bytes being the engine's request, names written from the recipe, not read from the body); the
//! interim register's row for a stair says which engine answers each.

use super::floor::{Extent, append};
use super::{Kernel, as_steps, fault, from_engine, name, op_id, too_big};
use crate::floor_plan;
use crate::stair_plan::{PieceSolid, StairPlan};
use construct_model::geometry::{
    Bounds, Built, Display, FaceName, GeometryError, Mesh, Part, Recipe,
};
use engine_geometry::value::engine::{Engine, Profile, Section};
use engine_geometry::value::grid::P3;
use engine_geometry::value::op::OpId;
use serde::Serialize;

/// The function every piece of a stair is drawn as.
const FUNCTION: &str = "structure";

/// A stair's body bytes: what the engine was asked for each piece.
#[derive(Serialize)]
struct Asked {
    pieces: Vec<AskedPiece>,
}

/// One piece as the engine was asked for it: a loft or an extrusion.
#[derive(Serialize)]
struct AskedPiece {
    id: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    loft: Vec<Vec<[i64; 3]>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    extrude: Vec<[i64; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    by: Option<[i64; 3]>,
}

/// The names of a stair's faces, sorted by name.
pub(super) fn names(op: OpId, plan: &StairPlan) -> Result<Vec<FaceName>, GeometryError> {
    let mut names = plan
        .roles()
        .iter()
        .map(|role| name(op, role))
        .collect::<Result<Vec<_>, _>>()?;
    names.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(names)
}

/// Points of the building grid as the engine's grid points.
fn on_engine(points: &[[i64; 3]], units: super::Units) -> Result<Vec<P3>, GeometryError> {
    points
        .iter()
        .map(|&[x, y, z]| {
            P3::new(units.from_q0(x)?, units.from_q0(y)?, units.from_q0(z)?).map_err(|_| too_big())
        })
        .collect()
}

/// A plan outline in whole millimetres, each corner the nearest, and whether any was rounded.
fn outline_mm(plan: &[[i64; 2]]) -> (Vec<[i64; 2]>, bool) {
    let mut rounded = false;
    let outline = plan
        .iter()
        .map(|&[x, y]| {
            let ((x, a), (y, b)) = (floor_plan::nearest_mm(x), floor_plan::nearest_mm(y));
            rounded |= !(a && b);
            [x, y]
        })
        .collect();
    (outline, rounded)
}

impl<E: Engine> Kernel<E> {
    /// Builds a filled stair recipe, as the module header says.
    ///
    /// # Errors
    ///
    /// [`StairPlan::of`]'s refusals; `NotAHost` when anything is hosted on it (nothing is, yet);
    /// `NotOnTheGrid` for a point that is not a whole number of the engine's steps; and the
    /// engine's own.
    pub(super) fn build_stair(
        &self,
        recipe: &Recipe,
        hosted: usize,
    ) -> Result<Built, GeometryError> {
        let plan = StairPlan::of(recipe)?;
        if hosted > 0 {
            return Err(fault("NotAHost", "nothing is hosted on a stair yet"));
        }
        let units = self.units;
        let op = op_id(recipe)?;
        let mut asked = Asked { pieces: Vec::new() };
        let mut mesh = Mesh {
            positions: Vec::new(),
            indices: Vec::new(),
        };
        let mut extent: Extent = None;
        let mut parts = Vec::new();
        let (outline, mut rounded) = plan
            .pieces
            .first()
            .map_or((Vec::new(), false), |p| outline_mm(&p.plan));
        {
            let mut engine = self
                .engine
                .lock()
                .map_err(|_| fault("EngineUnavailable", "an earlier build stopped part way"))?;
            for piece in &plan.pieces {
                let mut entry = AskedPiece {
                    id: piece.id.clone(),
                    loft: Vec::new(),
                    extrude: Vec::new(),
                    by: None,
                };
                let made = match &piece.solid {
                    PieceSolid::Loft { sections } => {
                        let sections = sections
                            .iter()
                            .map(|s| on_engine(s, units).map(|corners| Section { corners }))
                            .collect::<Result<Vec<_>, _>>()?;
                        entry.loft = sections.iter().map(|s| as_steps(&s.corners)).collect();
                        engine.loft(&sections).map_err(from_engine)?
                    }
                    PieceSolid::Extrude { profile, by } => {
                        let outer = on_engine(profile, units)?;
                        let by = [
                            units.from_q0(by[0])?,
                            units.from_q0(by[1])?,
                            units.from_q0(by[2])?,
                        ];
                        entry.extrude = as_steps(&outer);
                        entry.by = Some(by);
                        let profile = Profile {
                            outer,
                            holes: Vec::new(),
                        };
                        engine.extrude(&profile, by).map_err(from_engine)?
                    }
                };
                asked.pieces.push(entry);
                let shown = engine.mesh(&made).map_err(from_engine)?;
                let appended = append(&mut mesh, &shown, units, &mut extent)?;
                rounded |= appended.rounded;
                let (piece_outline, outline_rounded) = outline_mm(&piece.plan);
                rounded |= outline_rounded;
                parts.push(Part {
                    material: plan.material.clone(),
                    function: FUNCTION.to_owned(),
                    triangles: appended.triangles,
                    outline: piece_outline,
                });
            }
        }
        let (lo, hi) = extent.ok_or_else(|| fault("EmptyShape", "a stair with no piece"))?;
        let bounds = Bounds {
            min: lo.map(|s| units.floor_mm(s)),
            max: hi.map(|s| units.ceil_mm(s)),
        };
        let deviation = i64::from(rounded);
        let displays = (0..2)
            .map(|lod| Display {
                lod,
                deviation,
                mesh: mesh.clone(),
                outline: outline.clone(),
                parts: parts.clone(),
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
