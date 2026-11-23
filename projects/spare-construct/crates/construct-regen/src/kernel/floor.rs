//! A floor on the engine (job 4.3, CON-A4-F4-P1): one solid per layer of a filled `floor` recipe
//! ([`crate::floor_plan`]), each the extrusion of the floor's boundary, holes and all, from the
//! layer's bottom face straight up to its top.
//!
//! What it builds:
//!
//! - A solid for each layer with a thickness, top first: the profile of the outer loop with every
//!   hole in it, on the plane of the layer's bottom face, extruded up by the layer's thickness
//!   through [`Engine::extrude`]. The floor's top face is at `z = 0` and its layers hang below
//!   (the level rule: its built row holds the height its top face stands at). A layer of no
//!   thickness (a membrane) is a face between its neighbours and has no solid. Each solid is the
//!   engine's, so the kernel's checker has passed it, as the seam's contract says.
//! - Names (chapter 20.2), under the recipe's operation as every shape's are, so a rebuilt or
//!   reshaped floor keeps them: `top` and `bottom`, the floor's outer faces; `face.<k>` between
//!   layer `k - 1` and layer `k`, for `k` from 1, as a layered wall's faces between its layers;
//!   and `side.<edge ID>` for the side each edge of each loop sweeps, named by the edge's own
//!   permanent ID (construct-slabs' `Segment`), so a boundary drawn again with the same edges keeps
//!   its sides' names. Each layer's piece of a side carries the floor's name for it, as a layered
//!   wall's pieces of its ends do.
//! - The body's bytes: what the engine was asked for each layer (its function, its material, its
//!   outer loop and holes and the vector, in the engine's steps).
//! - Displays: levels 0 and 1 the same mesh (a floor has no openings yet), the layers' meshes one
//!   after another, in whole millimetres, each coordinate the nearest millimetre to the engine's
//!   grid point (a corner at 12.5 mm is not on one); a level that rounded anything strays by 1 mm.
//!   The outline is the outer loop, each corner the nearest millimetre; holes are not in it (a
//!   display's outline is one polygon), so a plan draws a floor's holes from its record. The
//!   bounds are rounded outward.
//!
//! Depends on the parent module's units, naming and errors, and [`crate::floor_plan`]. Interim: as
//! the parent module's (the engine behind the seam, the body's bytes being the engine's request,
//! names written from the recipe, not read from the body); an arc edge is refused before it gets
//! here (construct-slabs, the interim register's row) until the kernel's exact sweep takes arcs.

use super::{EXACT_F64, Kernel, Units, as_steps, fault, from_engine, name, op_id, too_big};
use crate::floor_plan::{self, FloorPlan, PlanCorner};
use construct_model::geometry::{Bounds, Built, Display, FaceName, GeometryError, Mesh, Recipe};
use construct_params::Length;
use engine_geometry::value::engine::{DisplayMesh, Engine, Profile};
use engine_geometry::value::grid::P3;
use engine_geometry::value::op::OpId;
use serde::Serialize;

/// A floor's body bytes: what the engine was asked for each layer.
#[derive(Serialize)]
struct Asked {
    layers: Vec<AskedLayer>,
}

/// One layer as the engine was asked for it. A floor without holes writes no `holes`.
#[derive(Serialize)]
struct AskedLayer {
    function: String,
    material: String,
    extrude: Vec<[i64; 3]>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    holes: Vec<Vec<[i64; 3]>>,
    by: [i64; 3],
}

/// The low and high grid points a mesh reaches, in the engine's steps.
type Extent = Option<([i64; 3], [i64; 3])>;

/// The roles of a floor's faces, in order: `top`, `bottom`, the faces between its layers and the
/// side of each edge of each loop.
fn roles(plan: &FloorPlan) -> Vec<String> {
    let mut roles = vec!["top".to_owned(), "bottom".to_owned()];
    for k in 1..plan.layers.len() {
        roles.push(format!("face.{k}"));
    }
    for corner in plan.loops.iter().flatten() {
        roles.push(format!("side.{}", corner.id));
    }
    roles
}

/// The names of a floor's faces, sorted by name.
fn names(op: OpId, plan: &FloorPlan) -> Result<Vec<FaceName>, GeometryError> {
    let mut names = roles(plan)
        .iter()
        .map(|role| name(op, role))
        .collect::<Result<Vec<_>, _>>()?;
    names.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(names)
}

/// A length in construct-params' steps (6,400 to the millimetre) in the engine's steps, exactly:
/// unchanged on `Q0`.
fn from_q0(units: Units, q0: i64) -> Result<i64, GeometryError> {
    let per = i128::from(Length::STEPS_PER_MM);
    let product = i128::from(q0)
        .checked_mul(i128::from(units.per_mm))
        .ok_or_else(too_big)?;
    match (product.checked_rem(per), product.checked_div(per)) {
        (Some(0), Some(steps)) => i64::try_from(steps).map_err(|_| too_big()),
        _ => Err(fault(
            "NotOnTheGrid",
            format!(
                "a length of {q0} building-grid steps is not a whole number of this engine's steps"
            ),
        )),
    }
}

/// `steps` of the engine as whole millimetres rounded down and the steps left over, at or above
/// zero.
fn split(units: Units, steps: i64) -> (i64, i64) {
    // per_mm is above zero (`Units::on`), so neither is `None`.
    (
        steps.checked_div_euclid(units.per_mm).unwrap_or(0),
        steps.checked_rem_euclid(units.per_mm).unwrap_or(0),
    )
}

/// `steps` of the engine to the nearest whole millimetre, a half away from zero, and whether that
/// is exact.
fn nearest_mm(units: Units, steps: i64) -> (i64, bool) {
    let (floor, rest) = split(units, steps);
    // 0 <= rest < per_mm, so twice it is well within an i64.
    let twice = rest.saturating_mul(2);
    let up = twice > units.per_mm || (twice == units.per_mm && floor >= 0);
    (if up { floor.saturating_add(1) } else { floor }, rest == 0)
}

/// `steps` of the engine in whole millimetres, rounded up.
fn ceil_mm(units: Units, steps: i64) -> i64 {
    match split(units, steps) {
        (floor, 0) => floor,
        (floor, _) => floor.saturating_add(1),
    }
}

/// A mesh coordinate as the engine gives it, a whole number of steps held in an f64, as an
/// integer.
fn whole_steps(steps: f64) -> Result<i64, GeometryError> {
    if steps.fract() != 0.0 || steps.abs() >= EXACT_F64 {
        return Err(fault(
            "NotOnTheGrid",
            format!("a mesh coordinate of {steps} is not a whole number of steps"),
        ));
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a whole number under 2^53, checked above, which an i64 holds exactly"
    )]
    let steps = steps as i64;
    Ok(steps)
}

/// The engine's mesh in whole millimetres, each coordinate the nearest millimetre, put after what
/// `into` holds, and whether any was rounded; the lowest and highest steps it reaches widen
/// `extent`.
fn append(
    into: &mut Mesh,
    mesh: &DisplayMesh,
    units: Units,
    extent: &mut Extent,
) -> Result<bool, GeometryError> {
    let many = || fault("TooBig", "a mesh with too many points");
    let base =
        u32::try_from(into.positions.len().checked_div(3).unwrap_or(0)).map_err(|_| many())?;
    let mut rounded = false;
    for point in &mesh.positions {
        let mut steps = [0_i64; 3];
        for (slot, &c) in steps.iter_mut().zip(point) {
            *slot = whole_steps(c)?;
        }
        for &s in &steps {
            let (mm, exact) = nearest_mm(units, s);
            rounded |= !exact;
            into.positions.push(mm);
        }
        *extent = Some(match *extent {
            None => (steps, steps),
            Some((mut lo, mut hi)) => {
                for ((l, h), &s) in lo.iter_mut().zip(hi.iter_mut()).zip(&steps) {
                    *l = (*l).min(s);
                    *h = (*h).max(s);
                }
                (lo, hi)
            }
        });
    }
    let count = mesh.positions.len();
    for &index in mesh.triangles.iter().flatten() {
        if !usize::try_from(index).is_ok_and(|i| i < count) {
            return Err(fault("EngineRefused", "mesh: a triangle past its points"));
        }
        into.indices.push(index.checked_add(base).ok_or_else(many)?);
    }
    Ok(rounded)
}

/// One loop's corners as grid points of the engine at height `z` (engine steps).
fn ring(corners: &[PlanCorner], units: Units, z: i64) -> Result<Vec<P3>, GeometryError> {
    corners
        .iter()
        .map(|corner| {
            let [x, y] = corner.at;
            P3::new(from_q0(units, x)?, from_q0(units, y)?, z).map_err(|_| too_big())
        })
        .collect()
}

impl<E: Engine> Kernel<E> {
    /// Builds a filled floor recipe, as the module header says.
    ///
    /// # Errors
    ///
    /// [`FloorPlan::of`]'s refusals; `NotAHost` when anything is hosted on it (nothing is, yet);
    /// `NotOnTheGrid` for a length that is not whole steps of the engine; and the engine's own.
    pub(super) fn build_floor(
        &self,
        recipe: &Recipe,
        hosted: usize,
    ) -> Result<Built, GeometryError> {
        let plan = FloorPlan::of(recipe)?;
        if hosted > 0 {
            return Err(fault("NotAHost", "nothing is hosted on a floor yet"));
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
        {
            let mut engine = self
                .engine
                .lock()
                .map_err(|_| fault("EngineUnavailable", "an earlier build stopped part way"))?;
            for (_, layer, bottom, top) in plan.slabs() {
                let z = from_q0(units, bottom)?;
                let up = from_q0(units, top.saturating_sub(bottom))?;
                let outer = ring(plan.outer(), units, z)?;
                let holes = plan
                    .holes()
                    .iter()
                    .map(|hole| ring(hole, units, z))
                    .collect::<Result<Vec<_>, _>>()?;
                let by = [0, 0, up];
                asked.layers.push(AskedLayer {
                    function: layer.function.clone(),
                    material: layer.material.clone(),
                    extrude: as_steps(&outer),
                    holes: holes.iter().map(|h| as_steps(h)).collect(),
                    by,
                });
                let profile = Profile { outer, holes };
                let solid = engine.extrude(&profile, by).map_err(from_engine)?;
                let shown = engine.mesh(&solid).map_err(from_engine)?;
                rounded |= append(&mut mesh, &shown, units, &mut extent)?;
            }
        }
        let (lo, hi) =
            extent.ok_or_else(|| fault("EmptyShape", "a floor with no layer of any thickness"))?;
        let bounds = Bounds {
            min: lo.map(|s| split(units, s).0),
            max: hi.map(|s| ceil_mm(units, s)),
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]
mod tests {
    use super::*;
    use engine_geometry::value::grid::GridStep;

    /// A stored length goes to the engine's steps exactly (unchanged on `Q0`, 25 times on `Q1`),
    /// and comes back to the nearest millimetre, a half away from zero, a bound rounding up.
    #[test]
    fn lengths_reach_the_engine_exactly_and_come_back_to_the_nearest_millimetre() {
        let q0 = Units::on(GridStep::Q0).unwrap();
        let q1 = Units::on(GridStep::Q1).unwrap();
        assert_eq!(from_q0(q0, 80_000).unwrap(), 80_000);
        assert_eq!(from_q0(q1, 80_000).unwrap(), 2_000_000);
        assert_eq!(from_q0(q1, i64::MAX).unwrap_err().code, "TooBig");
        assert_eq!(nearest_mm(q0, 368_000), (58, false));
        assert_eq!(nearest_mm(q0, -368_000), (-58, false));
        assert_eq!(nearest_mm(q1, 160_000), (1, true));
        assert_eq!(ceil_mm(q0, -368_000), -57);
        assert_eq!(split(q0, -368_000).0, -58);
        assert_eq!(whole_steps(6400.0).unwrap(), 6400);
        assert_eq!(whole_steps(0.5).unwrap_err().code, "NotOnTheGrid");
    }
}
