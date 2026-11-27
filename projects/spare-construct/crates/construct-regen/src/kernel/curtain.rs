//! A curtain wall on the engine (job 14.1, CON-A4-F6-P1, piece A84): one solid per glazed or solid
//! panel and per mullion of a filled `wall.curtain` recipe, each its quadrilateral in plan
//! ([`crate::curtain_plan`]) extruded straight up between its two heights through
//! [`Engine::extrude`], and each door panel its family's solids, made by the recipe evaluator
//! through the same engine.
//!
//! What it builds:
//!
//! - A solid for each panel with a slab and each mullion: the parallelogram the plan gives it,
//!   corners on the grid by the plan's one rounding rule, extruded from its foot by its height.
//!   No two overlap and every meeting is face to face (construct-curtain's layout), so the solids
//!   are kept apart, never united. Each knows its part (`panel.glazed`, `panel.solid`, `mullion`)
//!   and its material: the body's bytes list them, and each display lists the triangles of each as
//!   a part ([`Part`]), so a page colours glass, panels and mullions by material.
//! - A door panel: its family recipe from the builder's [`FamilySource`], its type's values with
//!   its `width` and `height` set to its panel's, evaluated through
//!   `construct_families::evaluate_turned` turned as the plan says, each solid meshed by the engine
//!   and moved to the door's place by whole grid steps before it is drawn, each a part named
//!   `door.<step>` with its material.
//! - Names (chapter 20.2): the wall's six roles, `base`, `top`, `exterior`, `interior`, `end0` and
//!   `end1`, under this recipe's operation, as a wall's are. A curtain wall hosts nothing.
//! - Displays: both levels the same solids, every coordinate the nearest millimetre to the
//!   engine's grid point; a level that rounded anything strays by 1 mm. The bounds are rounded
//!   outward, the outline is the plan's box round the wall.
//!
//! Depends on the parent module's units, naming and errors, the layered wall's mesh helpers,
//! [`crate::curtain_plan`] and [`crate::families`]. Interim: as the parent module's (the engine
//! behind the seam, the body's bytes being the engine's request, names written from the recipe);
//! and a door panel's solids are built at the family's origin and moved only in their meshes,
//! since the engine seam has no move (the body's bytes list where each door stands).

use super::layered::{Extent, append, empty};
use super::{Units, WALL_ROLES, as_steps, fault, from_engine, name, op_id};
use crate::curtain_plan::{CurtainPlan, DOOR_HEIGHT, DOOR_WIDTH, PlanDoor};
use crate::families::{self, FamilySource, Instance};
use construct_families::{Key, Limits, evaluate_turned};
use construct_model::geometry::{
    Bounds, Built, Display, FaceName, GeometryError, Mesh, Part, Recipe,
};
use construct_params::Value;
use engine_geometry::value::engine::{DisplayMesh, Engine, Profile};
use engine_geometry::value::grid::P3;
use serde::Serialize;
use std::collections::BTreeMap;

/// A curtain wall's body bytes: what the engine was asked for each solid, and each door panel.
#[derive(Serialize)]
struct Asked {
    solids: Vec<AskedSolid>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    doors: Vec<AskedDoor>,
}

/// One solid as the engine was asked for it.
#[derive(Serialize)]
struct AskedSolid {
    key: String,
    function: String,
    material: String,
    extrude: Vec<[i64; 3]>,
    by: [i64; 3],
}

/// One door panel as it was asked for: its family, type and size, where it stands and its turn.
#[derive(Serialize)]
struct AskedDoor {
    key: String,
    family: String,
    #[serde(rename = "type")]
    ty: String,
    width: String,
    height: String,
    origin: [i64; 3],
    turn: u8,
}

/// The persistent names of a curtain wall's faces: the wall's six roles under its operation,
/// sorted by name as a built shape lists them.
pub(super) fn names(recipe: &Recipe) -> Result<Vec<FaceName>, GeometryError> {
    let op = op_id(recipe)?;
    let mut names = WALL_ROLES
        .iter()
        .map(|role| name(op, role))
        .collect::<Result<Vec<_>, _>>()?;
    names.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(names)
}

/// The engine's mesh moved by `by` whole steps, in whole millimetres, each coordinate the nearest
/// millimetre, and whether any was rounded; the lowest and highest steps it reaches widen `extent`.
fn moved_mm(
    mesh: &DisplayMesh,
    by: [i64; 3],
    units: Units,
    extent: &mut Extent,
) -> Result<(Mesh, bool), GeometryError> {
    let mut positions = Vec::with_capacity(mesh.positions.len().saturating_mul(3));
    let mut rounded = false;
    for point in &mesh.positions {
        let mut steps = [0_i64; 3];
        for ((slot, &c), &d) in steps.iter_mut().zip(point).zip(&by) {
            *slot = Units::whole_steps(c)?
                .checked_add(d)
                .ok_or_else(super::too_big)?;
        }
        for &s in &steps {
            let (mm, exact) = units.nearest_mm(s);
            rounded |= !exact;
            positions.push(mm);
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
    let mut indices = Vec::with_capacity(mesh.triangles.len().saturating_mul(3));
    for &index in mesh.triangles.iter().flatten() {
        if !usize::try_from(index).is_ok_and(|i| i < count) {
            return Err(fault("EngineRefused", "mesh: a triangle past its points"));
        }
        indices.push(index);
    }
    Ok((Mesh { positions, indices }, rounded))
}

/// Plan corners in the building grid's steps, in whole millimetres, each the nearest.
fn outline(corners: &[[i64; 2]; 4], units: Units) -> Result<Vec<[i64; 2]>, GeometryError> {
    let mut out = Vec::with_capacity(4);
    for &[x, y] in corners {
        out.push([
            units.nearest_mm(units.from_q0(x)?).0,
            units.nearest_mm(units.from_q0(y)?).0,
        ]);
    }
    Ok(out)
}

/// The family `door` stands in, its type's values with its size set to its panel's.
fn door_recipe(
    families: &dyn FamilySource,
    door: &PlanDoor,
) -> Result<families::Resolved, GeometryError> {
    let instance = Instance {
        family: door.door.family().clone(),
        ty: door.door.ty().clone(),
        turn: door.turn,
        values: BTreeMap::new(),
    };
    let mut resolved = families::resolve(families, &instance)
        .map_err(|e| fault(&e.code, format!("door panel {}: {}", door.key, e.message)))?;
    for (key, length) in [(DOOR_WIDTH, door.width), (DOOR_HEIGHT, door.height)] {
        let key = Key::parse(key).ok_or_else(|| fault("BadInstance", "a parameter's key"))?;
        resolved.values.set(key, Value::Length(length).stored());
    }
    Ok(resolved)
}

/// Builds a curtain wall on `engine`, each door panel's family from `families`.
///
/// # Errors
///
/// `EmptyShape` for a curtain wall with nothing to build (every panel empty and no mullion); a
/// door panel's family's refusals, naming it; and the engine's own.
#[allow(
    clippy::too_many_lines,
    reason = "one curtain wall's solids, doors, meshes and names, in the order the body lists them"
)]
pub(super) fn build<E: Engine>(
    engine: &mut E,
    recipe: &Recipe,
    plan: &CurtainPlan,
    units: Units,
    families: &dyn FamilySource,
) -> Result<Built, GeometryError> {
    let mut asked = Asked {
        solids: Vec::with_capacity(plan.solids.len()),
        doors: Vec::with_capacity(plan.doors.len()),
    };
    let mut mesh = empty();
    let mut parts = Vec::new();
    let (mut reach, mut rounded): (Extent, bool) = (None, false);
    for solid in &plan.solids {
        let [z0, z1] = [units.from_q0(solid.z[0])?, units.from_q0(solid.z[1])?];
        let mut corners = Vec::with_capacity(4);
        for &[x, y] in &solid.corners {
            corners.push(
                P3::new(units.from_q0(x)?, units.from_q0(y)?, z0).map_err(|_| super::too_big())?,
            );
        }
        let by = [0, 0, z1.checked_sub(z0).ok_or_else(super::too_big)?];
        asked.solids.push(AskedSolid {
            key: solid.key.clone(),
            function: solid.function.to_owned(),
            material: solid.material.clone(),
            extrude: as_steps(&corners),
            by,
        });
        let profile = Profile {
            outer: corners,
            holes: Vec::new(),
        };
        let named = |e| {
            let e = from_engine(e);
            fault(
                &e.code,
                format!("{} {}: {}", solid.function, solid.key, e.message),
            )
        };
        let made = engine.extrude(&profile, by).map_err(named)?;
        let drawn = engine.mesh(&made).map_err(named)?;
        let (part, r) = moved_mm(&drawn, [0, 0, 0], units, &mut reach)?;
        rounded |= r;
        let triangles = append(&mut mesh, &part)?;
        parts.push(Part {
            material: solid.material.clone(),
            function: solid.function.to_owned(),
            triangles,
            outline: outline(&solid.corners, units)?,
        });
    }
    for door in &plan.doors {
        let resolved = door_recipe(families, door)?;
        let by = [
            units.from_q0(door.origin[0])?,
            units.from_q0(door.origin[1])?,
            units.from_q0(door.origin[2])?,
        ];
        asked.doors.push(AskedDoor {
            key: door.key.clone(),
            family: door.door.family().as_str().to_owned(),
            ty: door.door.ty().as_str().to_owned(),
            width: Value::Length(door.width).stored(),
            height: Value::Length(door.height).stored(),
            origin: by,
            turn: door.turn.quarters(),
        });
        let evaluated = evaluate_turned(
            engine,
            &resolved.recipe,
            &resolved.values,
            Limits::DEFAULT,
            door.turn,
        )
        .map_err(|e| {
            let e = families::evaluation_refused(&resolved.recipe, &e);
            fault(&e.code, format!("door panel {}: {}", door.key, e.message))
        })?;
        for made in &evaluated.solids {
            let drawn = engine.mesh(&made.solid).map_err(from_engine)?;
            let mut own: Extent = None;
            let (part, r) = moved_mm(&drawn, by, units, &mut own)?;
            rounded |= r;
            let Some((lo, hi)) = own else {
                continue;
            };
            reach = Some(match reach {
                None => (lo, hi),
                Some((mut min, mut max)) => {
                    for (m, l) in min.iter_mut().zip(lo) {
                        *m = (*m).min(l);
                    }
                    for (m, h) in max.iter_mut().zip(hi) {
                        *m = (*m).max(h);
                    }
                    (min, max)
                }
            });
            let triangles = append(&mut mesh, &part)?;
            let [x0, y0, x1, y1] = [lo[0], lo[1], hi[0], hi[1]].map(|s| units.nearest_mm(s).0);
            parts.push(Part {
                material: made.material.as_str().to_owned(),
                function: format!("door.{}", made.step.as_str()),
                triangles,
                outline: vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]],
            });
        }
    }
    let (lo, hi) = reach.ok_or_else(|| {
        fault(
            "EmptyShape",
            "a curtain wall with every panel empty and no mullion has nothing to build",
        )
    })?;
    let bounds = Bounds {
        min: lo.map(|s| units.floor_mm(s)),
        max: hi.map(|s| units.ceil_mm(s)),
    };
    let wall_outline = outline(&plan.outline, units)?;
    let displays = (0..2)
        .map(|lod| Display {
            lod,
            deviation: i64::from(rounded),
            mesh: mesh.clone(),
            outline: wall_outline.clone(),
            parts: parts.clone(),
        })
        .collect();
    Ok(Built {
        body: serde_json::to_vec(&asked).unwrap_or_default(),
        bounds,
        names: names(recipe)?,
        displays,
    })
}
