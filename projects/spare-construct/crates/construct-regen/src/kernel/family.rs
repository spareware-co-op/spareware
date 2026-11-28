//! A placed family on the engine (job 6.4, CON-A7-F3-P2): `family.instance`, its solids built by
//! the recipe evaluator over the builder's engine and drawn with their materials.
//!
//! What it builds: the family recipe its record names, found in the builder's family source,
//! evaluated for its type (`construct_families::evaluate_turned`; the parent keeps what each type
//! built to by the evaluator's key, so a type is built once however many are placed and however
//! often its wall moves), in the family's own frame turned by its turn: the family's origin is the
//! element's placement, which the regenerator reads from its wall (construct-regen's `hosting`).
//! In a wall at an angle it is evaluated seated instead (`construct_families::evaluate_seated`,
//! piece A83): every corner put through its seat and rounded once, its origin where the seat puts
//! it from the element's placement, and kept by the evaluator's key for that seat.
//! Each solid is meshed by the engine and drawn as a part of both displays with its material and
//! its step's key as its function, so a view colours the frame, the leaf and the glass apart.
//!
//! - Displays: both levels the solids, every coordinate the nearest millimetre to the engine's
//!   grid point; a level that rounded anything strays by 1 mm. The coarse level does not yet drop
//!   to the family's symbols (the views' job): a family's detail levels are its recipe's, and a
//!   view reads them there.
//! - Bounds: the meshes' reach, rounded outward. Outline: the box of that reach in plan, and each
//!   part the box of its own.
//! - Body: what was asked, the family by its content id, the type, the turn (or the seat) and each
//!   solid's step and material, so the same family in the same place gives the same bytes.
//! - Names: none yet. A placed family hosts nothing, so none of its faces is named for a hosted
//!   element; naming a family's faces for dimensions and tags is the views' and annotations'.
//!
//! Depends on the parent module's units and errors, the layered wall's mesh helpers, and
//! [`crate::families`]. Interim: as the parent module's, and where the recipe comes from
//! ([`crate::families`], job 7.6).

use super::layered::{Extent, append, empty, millimetres};
use super::{Units, fault, from_engine};
use crate::families::{self, Instance, Resolved};
use construct_families::{Limits, evaluate_seated, evaluate_turned};
use construct_model::geometry::{Bounds, Built, Display, GeometryError, Part};
use engine_geometry::value::engine::Engine;
use serde::Serialize;

/// A placed family's body bytes: what was asked.
#[derive(Serialize)]
struct Asked<'a> {
    family: &'a str,
    #[serde(rename = "type")]
    ty: &'a str,
    turn: u8,
    /// Where it stands in a wall at an angle: its seat's way, base and origin, in steps.
    #[serde(skip_serializing_if = "Option::is_none")]
    seat: Option<[[i64; 2]; 3]>,
    solids: Vec<AskedSolid>,
}

/// One solid as it was asked: its step and its material.
#[derive(Serialize)]
struct AskedSolid {
    step: String,
    material: String,
}

/// The four corners of the plan box from `lo` to `hi` (grid steps), each the nearest millimetre.
fn plan_box(units: Units, (lo, hi): ([i64; 3], [i64; 3])) -> Vec<[i64; 2]> {
    let [x0, y0, x1, y1] = [lo[0], lo[1], hi[0], hi[1]].map(|s| units.nearest_mm(s).0);
    vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
}

/// Builds the placed family `instance`, its recipe and type's values `resolved`, on `engine`.
///
/// # Errors
///
/// `FamilyRefused` with the evaluation's words (a sketch that does not solve at the type's values,
/// an engine on another grid, the engine's refusal of a step); `EmptyShape` for a family with no
/// solid; and the engine's own refusals of a mesh.
pub(super) fn build<E: Engine>(
    engine: &mut E,
    instance: &Instance,
    resolved: &Resolved,
    units: Units,
) -> Result<Built, GeometryError> {
    let evaluated = match &instance.seat {
        Some(seat) => evaluate_seated(
            engine,
            &resolved.recipe,
            &resolved.values,
            Limits::DEFAULT,
            seat,
        ),
        None => evaluate_turned(
            engine,
            &resolved.recipe,
            &resolved.values,
            Limits::DEFAULT,
            instance.turn,
        ),
    }
    .map_err(|e| families::evaluation_refused(&resolved.recipe, &e))?;
    let mut mesh = empty();
    let mut parts = Vec::with_capacity(evaluated.solids.len());
    let mut asked = Asked {
        family: instance.family.as_str(),
        ty: instance.ty.as_str(),
        turn: instance.turn.quarters(),
        seat: instance
            .seat
            .as_ref()
            .map(|seat| [seat.way(), seat.base(), seat.at()]),
        solids: Vec::with_capacity(evaluated.solids.len()),
    };
    let (mut reach, mut rounded): (Extent, bool) = (None, false);
    for solid in &evaluated.solids {
        let drawn = engine.mesh(&solid.solid).map_err(from_engine)?;
        let mut own: Extent = None;
        let (part_mesh, r) = millimetres(&drawn, units, &mut own)?;
        rounded |= r;
        let Some(own) = own else {
            continue;
        };
        reach = Some(match reach {
            None => own,
            Some((mut lo, mut hi)) => {
                for (l, o) in lo.iter_mut().zip(own.0) {
                    *l = (*l).min(o);
                }
                for (h, o) in hi.iter_mut().zip(own.1) {
                    *h = (*h).max(o);
                }
                (lo, hi)
            }
        });
        let triangles = append(&mut mesh, &part_mesh)?;
        parts.push(Part {
            material: solid.material.as_str().to_owned(),
            function: solid.step.as_str().to_owned(),
            triangles,
            outline: plan_box(units, own),
        });
        asked.solids.push(AskedSolid {
            step: solid.step.as_str().to_owned(),
            material: solid.material.as_str().to_owned(),
        });
    }
    let reach = reach.ok_or_else(|| {
        fault(
            "EmptyShape",
            format!("family {}: it has no solid to draw", resolved.recipe.name),
        )
    })?;
    let (lo, hi) = reach;
    let bounds = Bounds {
        min: lo.map(|s| units.floor_mm(s)),
        max: hi.map(|s| units.ceil_mm(s)),
    };
    let outline = plan_box(units, reach);
    let displays = (0..2)
        .map(|lod| Display {
            lod,
            deviation: i64::from(rounded),
            mesh: mesh.clone(),
            outline: outline.clone(),
            parts: parts.clone(),
        })
        .collect();
    Ok(Built {
        body: serde_json::to_vec(&asked).unwrap_or_default(),
        bounds,
        names: Vec::new(),
        displays,
    })
}
