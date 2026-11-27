//! A railing on the engine (job 14.2, CON-A4-F5-P3): one solid per piece of a filled `railing`
//! recipe ([`crate::railing_plan`]), each the sweep construct-railings worked out for it, asked of
//! the engine through its trait alone.
//!
//! What it builds:
//!
//! - A solid for each post and baluster: its flat profile in plan extruded straight up
//!   ([`Engine::extrude`]).
//! - A solid for each piece of top rail or handrail: its upright section lofted to the same section
//!   moved along its piece ([`Engine::loft`] of two sections: every wall of such a loft is flat,
//!   each joining two corners of one section to the same two of the other, as a stair's run is
//!   built), since an extrusion along a vector that is not upright is not every engine's: the
//!   server's answers one with a surface the kernel has no kind for.
//! - Each solid is the engine's, so the kernel's checker has passed it, as the seam's contract
//!   says. The pieces touch or overlap where a rail meets a post or a baluster and are not united.
//! - Names (chapter 20.2), under the recipe's operation as every shape's are: one role for each
//!   piece, its ID ([`RailingPlan::roles`]).
//! - The body's bytes: what the engine was asked for each piece (its ID, its profile and its
//!   vector, in the engine's steps).
//! - Displays: levels 0 and 1 the same mesh, the pieces' meshes one after another, in whole
//!   millimetres, each coordinate the nearest millimetre to the engine's grid point; a level that
//!   rounded anything strays by 1 mm. Each piece is a part ([`Part`]): the railing's material,
//!   function its part (`post`, `top-rail`, `handrail` or `baluster`), its triangles and its
//!   outline in plan. The bounds are rounded outward.
//!
//! Depends on the parent module's units, naming and errors, its `floor` child's mesh helpers, and
//! [`crate::railing_plan`]. Interim: as the parent module's (the engine behind the seam, the body's
//! bytes being the engine's request, names written from the recipe, not read from the body); the
//! interim register's row for a railing says which engine answers each.

use super::floor::{Extent, append};
use super::{Kernel, as_steps, fault, from_engine, name, op_id, too_big};
use crate::floor_plan;
use crate::railing_plan::RailingPlan;
use construct_model::geometry::{
    Bounds, Built, Display, FaceName, GeometryError, Mesh, Part, Recipe,
};
use engine_geometry::value::engine::{Engine, Profile, Section};
use engine_geometry::value::grid::P3;
use engine_geometry::value::op::OpId;
use serde::Serialize;

/// A railing's body bytes: what the engine was asked for each piece.
#[derive(Serialize)]
struct Asked {
    pieces: Vec<AskedPiece>,
}

/// One piece as the engine was asked for it.
#[derive(Serialize)]
struct AskedPiece {
    id: String,
    extrude: Vec<[i64; 3]>,
    by: [i64; 3],
    /// Lofted to the same profile moved by `by`, not extruded.
    lofted: bool,
}

/// The names of a railing's faces, sorted by name.
pub(super) fn names(op: OpId, plan: &RailingPlan) -> Result<Vec<FaceName>, GeometryError> {
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
    /// Builds a filled railing recipe, as the module header says.
    ///
    /// # Errors
    ///
    /// [`RailingPlan::of`]'s refusals; `NotAHost` when anything is hosted on it; `NotOnTheGrid` for
    /// a point that is not a whole number of the engine's steps; and the engine's own.
    pub(super) fn build_railing(
        &self,
        recipe: &Recipe,
        hosted: usize,
    ) -> Result<Built, GeometryError> {
        let plan = RailingPlan::of(recipe)?;
        if hosted > 0 {
            return Err(fault("NotAHost", "nothing is hosted on a railing"));
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
            .map_or((Vec::new(), false), |p| outline_mm(&p.plan()));
        {
            let mut engine = self
                .engine
                .lock()
                .map_err(|_| fault("EngineUnavailable", "an earlier build stopped part way"))?;
            for piece in &plan.pieces {
                let outer = on_engine(&piece.profile, units)?;
                let by = [
                    units.from_q0(piece.by[0])?,
                    units.from_q0(piece.by[1])?,
                    units.from_q0(piece.by[2])?,
                ];
                let lofted = piece.by[0] != 0 || piece.by[1] != 0;
                asked.pieces.push(AskedPiece {
                    id: piece.id.clone(),
                    extrude: as_steps(&outer),
                    by,
                    lofted,
                });
                let made = if lofted {
                    let swept: Vec<[i64; 3]> = piece
                        .profile
                        .iter()
                        .map(|p| {
                            [
                                p[0].saturating_add(piece.by[0]),
                                p[1].saturating_add(piece.by[1]),
                                p[2].saturating_add(piece.by[2]),
                            ]
                        })
                        .collect();
                    let sections = [
                        Section { corners: outer },
                        Section {
                            corners: on_engine(&swept, units)?,
                        },
                    ];
                    engine.loft(&sections).map_err(from_engine)?
                } else {
                    let profile = Profile {
                        outer,
                        holes: Vec::new(),
                    };
                    engine.extrude(&profile, by).map_err(from_engine)?
                };
                let shown = engine.mesh(&made).map_err(from_engine)?;
                let appended = append(&mut mesh, &shown, units, &mut extent)?;
                rounded |= appended.rounded;
                let (piece_outline, outline_rounded) = outline_mm(&piece.plan());
                rounded |= outline_rounded;
                parts.push(Part {
                    material: plan.material.clone(),
                    function: piece.part.clone(),
                    triangles: appended.triangles,
                    outline: piece_outline,
                });
            }
        }
        let (lo, hi) = extent.ok_or_else(|| fault("EmptyShape", "a railing with no piece"))?;
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
