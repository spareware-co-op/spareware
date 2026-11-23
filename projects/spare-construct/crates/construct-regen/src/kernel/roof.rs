//! A roof on the engine (job 5.3, CON-A4-F4-P3): one solid per layer of a filled `roof` recipe
//! ([`crate::roof_plan`]), each the solid construct-roofs worked out for it, asked of the engine
//! through its trait alone.
//!
//! What it builds:
//!
//! - A solid for each layer with a thickness, top first: a flat roof's layer is its eaves outline
//!   extruded straight up from its lower face to its upper face ([`Engine::extrude`]); a sloped
//!   roof's layer is its section across the roof at one verge lofted to the same section at the
//!   other ([`Engine::loft`]: every wall of such a loft is flat, each joining two corners of one
//!   section to the same two of the other); a solved roof's layer (job 6.2) is each of its flat
//!   pieces extruded straight up ([`Engine::extrude`]) and united in the order the recipe gives
//!   ([`Engine::unite`]), each piece sharing a side with one before it, so the union stays one
//!   solid. A layer of no thickness (a membrane) is a face between
//!   its neighbours and has no solid. Each solid is the engine's, so the kernel's checker has
//!   passed it, as the seam's contract says.
//! - Names (chapter 20.2), under the recipe's operation as every shape's are, so a rebuilt or
//!   reshaped roof keeps them: the roles [`crate::roof_plan::RoofPlan::roles`] lists, each slope
//!   named by the footprint edge it rises from and each side by its edge's permanent ID.
//! - The body's bytes: what the engine was asked for each layer (its function, its material, and
//!   its profile and vector or its two sections, in the engine's steps).
//! - Displays: levels 0 and 1 the same mesh (a roof has no openings yet), the layers' meshes one
//!   after another, in whole millimetres, each coordinate the nearest millimetre to the engine's
//!   grid point; a level that rounded anything strays by 1 mm. Each layer with a thickness is a
//!   part ([`Part`]): its material, its function, its triangles in the mesh and, seen from above,
//!   the eaves outline, as a floor's layers are, so a page colours each layer by its material.
//!   The bounds are rounded outward.
//!
//! Depends on the parent module's units, naming and errors, its `floor` child's mesh helpers, and
//! [`crate::roof_plan`]. Interim: as the parent module's (the engine behind the seam, the body's
//! bytes being the engine's request, names written from the recipe, not read from the body); the
//! interim register's row for a roof's layers says which engine answers each.

use super::floor::{Extent, append};
use super::{Kernel, as_steps, fault, from_engine, name, op_id, too_big};
use crate::floor_plan;
use crate::roof_plan::{PlanSolid, RoofPlan};
use construct_model::geometry::{
    Bounds, Built, Display, FaceName, GeometryError, Mesh, Part, Recipe,
};
use engine_geometry::value::engine::{Engine, Profile, Section};
use engine_geometry::value::grid::P3;
use engine_geometry::value::op::OpId;
use serde::Serialize;

/// A roof's body bytes: what the engine was asked for each layer.
#[derive(Serialize)]
struct Asked {
    layers: Vec<AskedLayer>,
}

/// One layer as the engine was asked for it: an extrusion (`extrude` and `by`) or a loft.
#[derive(Serialize)]
struct AskedLayer {
    function: String,
    material: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    extrude: Vec<[i64; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    by: Option<[i64; 3]>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    loft: Vec<Vec<[i64; 3]>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    unite: Vec<Vec<[i64; 3]>>,
}

/// The names of a roof's faces, sorted by name.
pub(super) fn names(op: OpId, plan: &RoofPlan) -> Result<Vec<FaceName>, GeometryError> {
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

impl<E: Engine> Kernel<E> {
    /// Builds a filled roof recipe, as the module header says.
    ///
    /// # Errors
    ///
    /// [`RoofPlan::of`]'s refusals; `NotAHost` when anything is hosted on it (nothing is, yet);
    /// `NotOnTheGrid` for a point that is not a whole number of the engine's steps; and the
    /// engine's own.
    #[allow(
        clippy::too_many_lines,
        reason = "each kind of layer asked of the engine in one place, as the module header lists them"
    )]
    pub(super) fn build_roof(
        &self,
        recipe: &Recipe,
        hosted: usize,
    ) -> Result<Built, GeometryError> {
        let plan = RoofPlan::of(recipe)?;
        if hosted > 0 {
            return Err(fault("NotAHost", "nothing is hosted on a roof yet"));
        }
        let units = self.units;
        let op = op_id(recipe)?;
        let mut asked = Asked { layers: Vec::new() };
        let mut mesh = Mesh {
            positions: Vec::new(),
            indices: Vec::new(),
        };
        let mut extent: Extent = None;
        let mut rounded = false;
        let outline: Vec<[i64; 2]> = plan
            .outline
            .iter()
            .map(|corner| {
                let [x, y] = corner.at;
                let ((x, a), (y, b)) = (floor_plan::nearest_mm(x), floor_plan::nearest_mm(y));
                rounded |= !(a && b);
                [x, y]
            })
            .collect();
        let mut parts = Vec::new();
        {
            let mut engine = self
                .engine
                .lock()
                .map_err(|_| fault("EngineUnavailable", "an earlier build stopped part way"))?;
            for layer in &plan.layers {
                let Some(solid) = &layer.solid else {
                    continue;
                };
                let mut entry = AskedLayer {
                    function: layer.function.clone(),
                    material: layer.material.clone(),
                    extrude: Vec::new(),
                    by: None,
                    loft: Vec::new(),
                    unite: Vec::new(),
                };
                let made = match solid {
                    PlanSolid::Extrude { profile, by } => {
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
                    PlanSolid::Loft { sections } => {
                        let sections = sections
                            .iter()
                            .map(|s| on_engine(s, units).map(|corners| Section { corners }))
                            .collect::<Result<Vec<_>, _>>()?;
                        entry.loft = sections.iter().map(|s| as_steps(&s.corners)).collect();
                        engine.loft(&sections).map_err(from_engine)?
                    }
                    PlanSolid::Unite { profiles, by } => {
                        let by = [
                            units.from_q0(by[0])?,
                            units.from_q0(by[1])?,
                            units.from_q0(by[2])?,
                        ];
                        entry.by = Some(by);
                        let mut united = None;
                        for profile in profiles {
                            let outer = on_engine(profile, units)?;
                            entry.unite.push(as_steps(&outer));
                            let piece = engine
                                .extrude(
                                    &Profile {
                                        outer,
                                        holes: Vec::new(),
                                    },
                                    by,
                                )
                                .map_err(from_engine)?;
                            united = Some(match united {
                                None => piece,
                                Some(so_far) => {
                                    engine.unite(&so_far, &piece).map_err(from_engine)?
                                }
                            });
                        }
                        united.ok_or_else(|| fault("EmptyShape", "a solved layer has no piece"))?
                    }
                };
                asked.layers.push(entry);
                let shown = engine.mesh(&made).map_err(from_engine)?;
                let appended = append(&mut mesh, &shown, units, &mut extent)?;
                rounded |= appended.rounded;
                parts.push(Part {
                    material: layer.material.clone(),
                    function: layer.function.clone(),
                    triangles: appended.triangles,
                    outline: outline.clone(),
                });
            }
        }
        let (lo, hi) =
            extent.ok_or_else(|| fault("EmptyShape", "a roof with no layer of any thickness"))?;
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
