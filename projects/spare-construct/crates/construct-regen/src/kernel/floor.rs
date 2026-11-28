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
//! - A sloped floor (job 5.2): the same, each layer's bottom face the floor's loops lifted onto
//!   the sloped plane (every corner at its top's height, [`FloorPlan::top_at`], let down by the
//!   layer's depth: grid points on one plane, as construct-slabs' `Shape` checked) and lofted to
//!   the same loop moved straight up by its thickness ([`Engine::loft`], every wall upright and
//!   flat), each hole cut through by its own loft ([`Engine::subtract`]); so the layers are plumb
//!   and their side faces upright. The engine answers an extrusion of a profile that is not level
//!   with a curved surface, so a sloped layer is not asked for as one.
//! - Slab edges (job 5.2): each piece of [`FloorPlan::edges`], a downturn as its foot extruded
//!   straight up to the floor's underside ([`Engine::extrude`]) and a thickened edge as its foot
//!   lofted to its top ([`Engine::loft`]: every wall of it flat), united in order with the lowest
//!   layer's solid ([`Engine::unite`], the engine's flat-faced boolean), so the lowest layer's
//!   part holds them in its material. A union the engine does not answer is the engine's refusal,
//!   never a floor built without its edges.
//! - Names (chapter 20.2), under the recipe's operation as every shape's are, so a rebuilt or
//!   reshaped floor keeps them: `top` and `bottom`, the floor's outer faces; `face.<k>` between
//!   layer `k - 1` and layer `k`, for `k` from 1, as a layered wall's faces between its layers;
//!   `side.<edge ID>` for the side each edge of each loop sweeps, named by the edge's own
//!   permanent ID (construct-slabs' `Segment`), so a boundary drawn again with the same edges keeps
//!   its sides' names; and `edge.<edge ID>` for each slab edge. Each layer's piece of a side
//!   carries the floor's name for it, as a layered wall's pieces of its ends do.
//! - Openings and shafts (job 4.2, CON-A4-F3-P2): each floor opening hosted in it and each shaft
//!   the regenerator hands it (one passing wholly through its thickness) is one more hole in every
//!   layer's profile, so it is cut through every layer by the floor's own extrusion, with no
//!   second cutter. Their outlines are fitted first by construct-openings' `fit_in_floor`, which is
//!   the floor's own boundary checks with them as more holes: each refusal names the opening or the
//!   shaft and says why, before the engine is asked, and [`fits`] asks the same without building.
//!   A shaft beside the floor in plan is passed over. Their sides are named as a tool's faces,
//!   `tool(<key>).side.<i>`, under the floor's operation. A sloped floor or one with slab edges
//!   that an opening or a shaft would cut is refused by name instead (job 5.2: the cutters are
//!   level outlines and the slope's layers and the edges' pieces are not, so a cut through them is
//!   not built until it is built exactly; [`SHAPED_FLOOR_CUT`], the interim register's row).
//! - The body's bytes: what the engine was asked for each layer (its function, its material, its
//!   outer loop and holes and the vector, in the engine's steps, and the lowest layer's slab
//!   edges, each its extrusion or its loft), and the keys of what was cut.
//! - Displays: levels 0 and 1 the same mesh, its openings cut, the layers' meshes one
//!   after another, in whole millimetres, each coordinate the nearest millimetre to the engine's
//!   grid point (a corner at 12.5 mm is not on one); a level that rounded anything strays by 1 mm.
//!   Each layer with a thickness is a part ([`Part`]): its material, its function, its triangles
//!   in the mesh and, seen from above, the outer loop, as a layered wall's layers are, so a page
//!   colours each layer by its material. The outline is the outer loop, each corner the nearest
//!   millimetre; holes are not in it nor in a part's outline (a display's outline is one polygon),
//!   so a plan draws a floor's holes from its record. The bounds are rounded outward.
//!
//! - A shape-edited floor (job 14.15): each layer that follows its top face, and its variable
//!   layer, is the top face's triangles lofted and united (the child module `edited`); a layer
//!   below the variable one is extruded flat as on a plain floor. Openings and shafts through such
//!   a floor are refused by name as through a sloped one ([`SHAPED_FLOOR_CUT`]).
//!
//! Depends on the parent module's units, naming and errors, and [`crate::floor_plan`]. Interim: as
//! the parent module's (the engine behind the seam, the body's bytes being the engine's request,
//! names written from the recipe, not read from the body); an arc edge is refused before it gets
//! here (construct-slabs, the interim register's row) until the kernel's exact sweep takes arcs.

mod edited;

use super::opening::{Hosted, tool_names};
use super::{Kernel, Units, as_steps, fault, from_engine, name, op_id, too_big};
use crate::floor_plan::{self, FloorPlan, PlanCorner};
use construct_model::geometry::{
    Bounds, Built, Display, FaceName, GeometryError, Mesh, Part, Recipe,
};
use construct_openings::{Cutter, OpeningError, fit_in_floor, is_floor_opening, is_shaft};
use edited::Stand;
use engine_geometry::value::engine::{DisplayMesh, Engine, Profile, Section};
use engine_geometry::value::grid::P3;
use engine_geometry::value::op::OpId;
use serde::Serialize;

/// A floor's body bytes: what the engine was asked for each layer, and the keys of the openings and
/// shafts cut through it, in order. A floor with none writes no `cut`, so its bytes are as they
/// were before floors had openings.
#[derive(Serialize)]
struct Asked {
    layers: Vec<AskedLayer>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cut: Vec<String>,
}

/// An outline cut through every layer of a floor: the opening's or shaft's key, what a refusal
/// calls it, its operation, and its corners in the engine's steps in the floor's frame, clockwise
/// seen from above as a hole's are.
struct FloorCut {
    key: String,
    who: String,
    op: OpId,
    ring: Vec<[i64; 2]>,
}

/// The code of the refusal of an opening or a shaft that would cut a sloped floor or one with slab
/// edges (the interim register's row).
pub const SHAPED_FLOOR_CUT: &str = "ShapedFloorCut";

/// What the floor's refusals call it: the builder is handed its recipe, not its record.
const THIS_FLOOR: &str = "its floor";

fn refusal(error: &OpeningError) -> GeometryError {
    fault(error.code(), error.to_string())
}

/// A loop's corners in the engine's steps.
fn engine_ring(corners: &[PlanCorner], units: Units) -> Result<Vec<[i64; 2]>, GeometryError> {
    corners
        .iter()
        .map(|c| {
            let [x, y] = c.at;
            Ok([units.from_q0(x)?, units.from_q0(y)?])
        })
        .collect()
}

/// Twice the signed area of a ring of engine steps: above 0 anticlockwise.
fn twice_area(ring: &[[i64; 2]]) -> i128 {
    let mut twice = 0_i128;
    for (at, &[x0, y0]) in ring.iter().enumerate() {
        let [x1, y1] = ring
            .get(at.saturating_add(1))
            .or_else(|| ring.first())
            .copied()
            .unwrap_or([x0, y0]);
        twice = twice.saturating_add(
            i128::from(x0)
                .saturating_mul(i128::from(y1))
                .saturating_sub(i128::from(x1).saturating_mul(i128::from(y0))),
        );
    }
    twice
}

/// The outlines of `hosted` that cut a floor of `plan`, in the order of their keys, each where its
/// place in the floor's frame puts it (a floor opening's is drawn in that frame, at no offset; a
/// shaft's is moved by its place), checked by construct-openings' [`fit_in_floor`], which also
/// passes over a shaft beside the floor in plan.
///
/// # Errors
/// `NotAnOpening` for anything hosted on a floor that is neither; construct-openings' refusals of
/// an outline and of its fit, each naming the element and why.
fn floor_cuts(
    plan: &FloorPlan,
    hosted: &[Hosted<'_>],
    units: Units,
) -> Result<Vec<FloorCut>, GeometryError> {
    let mut cutters = Vec::with_capacity(hosted.len());
    for h in hosted {
        if !is_floor_opening(h.recipe) && !is_shaft(h.recipe) {
            return Err(fault(
                "NotAnOpening",
                format!(
                    "{}: a {} is not cut through a floor; only an opening.floor or a shaft is",
                    super::opening::who(h.recipe, h.key),
                    h.recipe.op
                ),
            ));
        }
        let element = construct_openings::named(h.key, h.recipe);
        let outline = construct_openings::outline_of(h.key, h.recipe).map_err(|e| refusal(&e))?;
        let mut corners = Vec::with_capacity(outline.corners().len());
        for [x, y] in outline.corners() {
            corners.push([
                units
                    .from_q0(x.steps())?
                    .checked_add(h.at.x.get())
                    .ok_or_else(too_big)?,
                units
                    .from_q0(y.steps())?
                    .checked_add(h.at.y.get())
                    .ok_or_else(too_big)?,
            ]);
        }
        cutters.push(Cutter { element, corners });
    }
    if cutters.is_empty() {
        return Ok(Vec::new());
    }
    let outer = engine_ring(plan.outer(), units)?;
    let holes = plan
        .holes()
        .iter()
        .map(|hole| engine_ring(hole, units))
        .collect::<Result<Vec<_>, _>>()?;
    let cutting = fit_in_floor(THIS_FLOOR, &outer, &holes, &cutters).map_err(|e| refusal(&e))?;
    // Job 5.2: a cutter is a level outline cut straight through every layer, and a sloped floor's
    // layers and a slab edge's piece are not level: a cut through either is refused by name, never
    // built wrongly, until it is built exactly.
    if let Some(at) = cutting.first() {
        let shaped = if !plan.tops.is_empty() {
            Some(("a slope", "the slope"))
        } else if !plan.edges.is_empty() {
            Some(("slab edges", "the slab edges"))
        } else if !plan.faces.is_empty() {
            Some(("an edited shape", "the shape edits (reset its shape)"))
        } else {
            None
        };
        if let (Some((shaped, away)), Some(cutter)) = (shaped, cutters.get(*at)) {
            return Err(fault(
                SHAPED_FLOOR_CUT,
                format!(
                    "{}: it would be cut through a floor with {shaped}, and a floor with {shaped} is not cut by openings and shafts yet: take {away} away, or put the opening or shaft elsewhere",
                    cutter.element,
                ),
            ));
        }
    }
    let mut cuts = Vec::with_capacity(cutting.len());
    for at in cutting {
        let (Some(h), Some(cutter)) = (hosted.get(at), cutters.get(at)) else {
            continue;
        };
        let mut ring = cutter.corners.clone();
        if twice_area(&ring) > 0 {
            ring.reverse();
        }
        cuts.push(FloorCut {
            key: h.key.to_owned(),
            who: cutter.element.to_string(),
            op: op_id(h.recipe)?,
            ring,
        });
    }
    Ok(cuts)
}

/// Whether the openings and shafts `hosted` cut a floor of `recipe` by this builder's rules,
/// asked without the engine: what [`Kernel::build_hosting`] would refuse, in its words.
///
/// # Errors
/// [`FloorPlan::of`]'s refusals and those of [`floor_cuts`].
pub(super) fn fits(
    recipe: &Recipe,
    hosted: &[Hosted<'_>],
    units: Units,
) -> Result<(), GeometryError> {
    floor_cuts(&FloorPlan::of(recipe)?, hosted, units).map(|_| ())
}

/// One layer as the engine was asked for it. A floor without holes writes no `holes`.
#[derive(Serialize)]
struct AskedLayer {
    function: String,
    material: String,
    /// None on a layer of a shape-edited floor made of its triangles' lofts.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    extrude: Vec<[i64; 3]>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    holes: Vec<Vec<[i64; 3]>>,
    by: [i64; 3],
    /// A sloped layer's: its outer loop lofted from its bottom face to its top (`extrude` to it
    /// moved by `by`), each hole cut through as its loop lofted from a step under the bottom face
    /// to a step over the top.
    #[serde(skip_serializing_if = "Option::is_none")]
    lofted: Option<bool>,
    /// The slab edges united with the lowest layer; none on any other.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    edges: Vec<AskedEdge>,
    /// A shape-edited floor's layer that follows its top or fills to its flat bottom: each
    /// triangle's loft, its lower and upper sections, united in order (job 14.15).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    faces: Vec<[Vec<[i64; 3]>; 2]>,
}

/// The low and high grid points a mesh reaches, in the engine's steps.
pub(super) type Extent = Option<([i64; 3], [i64; 3])>;

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
    for edge in &plan.edges {
        roles.push(format!("edge.{}", edge.id));
    }
    roles
}

/// The names of a floor's faces, sorted by name.
pub(super) fn names(op: OpId, plan: &FloorPlan) -> Result<Vec<FaceName>, GeometryError> {
    let mut names = roles(plan)
        .iter()
        .map(|role| name(op, role))
        .collect::<Result<Vec<_>, _>>()?;
    names.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(names)
}

/// A count of a mesh's items, of three numbers each, as a display's triangle numbers are.
fn thirds(len: usize) -> Result<u32, GeometryError> {
    u32::try_from(len.checked_div(3).unwrap_or(0))
        .map_err(|_| fault("TooBig", "a mesh with too many points or triangles"))
}

/// What [`append`] put in a mesh: its first triangle and how many, and whether any point was
/// rounded.
pub(super) struct Appended {
    pub(super) triangles: [u32; 2],
    pub(super) rounded: bool,
}

/// The engine's mesh in whole millimetres, each coordinate the nearest millimetre, put after what
/// `into` holds; the lowest and highest steps it reaches widen `extent`. A roof's layers are put
/// together the same way ([`super::roof`]).
pub(super) fn append(
    into: &mut Mesh,
    mesh: &DisplayMesh,
    units: Units,
    extent: &mut Extent,
) -> Result<Appended, GeometryError> {
    let many = || fault("TooBig", "a mesh with too many points");
    let base = thirds(into.positions.len())?;
    let first = thirds(into.indices.len())?;
    let mut rounded = false;
    for point in &mesh.positions {
        let mut steps = [0_i64; 3];
        for (slot, &c) in steps.iter_mut().zip(point) {
            *slot = Units::whole_steps(c)?;
        }
        for &s in &steps {
            let (mm, exact) = units.nearest_mm(s);
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
    let count = thirds(into.indices.len())?.saturating_sub(first);
    Ok(Appended {
        triangles: [first, count],
        rounded,
    })
}

/// One loop's corners as grid points of the engine at height `z` (engine steps).
pub(super) fn ring(corners: &[PlanCorner], units: Units, z: i64) -> Result<Vec<P3>, GeometryError> {
    corners
        .iter()
        .map(|corner| {
            let [x, y] = corner.at;
            P3::new(units.from_q0(x)?, units.from_q0(y)?, z).map_err(|_| too_big())
        })
        .collect()
}

/// Loop `at` of the plan as grid points of the engine, each corner `depth` (grid steps, at or
/// below zero) under the top face there: on a flat floor all at `depth`, on a sloped one on the
/// plane through the corners' tops let down by `depth`, a plane still.
fn lifted_ring(
    plan: &FloorPlan,
    at: usize,
    corners: &[PlanCorner],
    units: Units,
    depth: i64,
) -> Result<Vec<P3>, GeometryError> {
    corners
        .iter()
        .enumerate()
        .map(|(k, corner)| {
            let [x, y] = corner.at;
            let z = plan.top_at(at, k).checked_add(depth).ok_or_else(too_big)?;
            P3::new(units.from_q0(x)?, units.from_q0(y)?, units.from_q0(z)?).map_err(|_| too_big())
        })
        .collect()
}

/// `points` each moved by `by` (engine steps).
fn shifted(points: &[P3], by: [i64; 3]) -> Result<Vec<P3>, GeometryError> {
    points
        .iter()
        .map(|p| {
            let at = |c: i64, d: i64| c.checked_add(d).ok_or_else(too_big);
            P3::new(
                at(p.x.get(), by[0])?,
                at(p.y.get(), by[1])?,
                at(p.z.get(), by[2])?,
            )
            .map_err(|_| too_big())
        })
        .collect()
}

/// A sloped layer: its outer loop on its sloped bottom face lofted to the same loop moved up by
/// `by` ([`Engine::loft`]: every wall upright and flat, both caps on the sloped planes), less each
/// hole lofted the same way from one grid step under the bottom face to one over the top
/// ([`Engine::subtract`]), so no face of a cutter lies on a face of the layer. An extrusion of a
/// profile that is not level is not used: the engine answers it with a curved surface.
fn sloped_layer<E: Engine>(
    engine: &mut E,
    outer: &[P3],
    holes: &[Vec<P3>],
    by: [i64; 3],
) -> Result<E::Solid, GeometryError> {
    let up = |points: &[P3], d: [i64; 3]| shifted(points, d).map(|corners| Section { corners });
    let mut solid = engine
        .loft(&[up(outer, [0, 0, 0])?, up(outer, by)?])
        .map_err(from_engine)?;
    for hole in holes {
        let over = by[2].checked_add(1).ok_or_else(too_big)?;
        let cutter = engine
            .loft(&[up(hole, [0, 0, -1])?, up(hole, [0, 0, over])?])
            .map_err(from_engine)?;
        solid = engine.subtract(&solid, &cutter).map_err(from_engine)?;
    }
    Ok(solid)
}

/// Four corners of a slab edge's plan as grid points of the engine.
fn corners(plan: &[[i64; 3]; 4], units: Units) -> Result<Vec<P3>, GeometryError> {
    plan.iter()
        .map(|&[x, y, z]| {
            P3::new(units.from_q0(x)?, units.from_q0(y)?, units.from_q0(z)?).map_err(|_| too_big())
        })
        .collect()
}

/// A slab edge as the engine was asked for it: a downturn's foot extruded up (`extrude`, `by`),
/// a thickened edge's foot lofted to its top (`loft`).
#[derive(Serialize)]
struct AskedEdge {
    id: String,
    kind: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    extrude: Vec<[i64; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    by: Option<[i64; 3]>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    loft: Vec<Vec<[i64; 3]>>,
}

/// Each slab edge of `plan` as a solid united with `layer` (the lowest layer's solid), in order,
/// each asked as `asked` records.
fn with_edges<E: Engine>(
    engine: &mut E,
    plan: &FloorPlan,
    units: Units,
    mut layer: E::Solid,
    asked: &mut Vec<AskedEdge>,
) -> Result<E::Solid, GeometryError> {
    for edge in &plan.edges {
        let (top, foot) = (corners(&edge.top, units)?, corners(&edge.foot, units)?);
        let mut entry = AskedEdge {
            id: edge.id.clone(),
            kind: edge.kind.clone(),
            extrude: Vec::new(),
            by: None,
            loft: Vec::new(),
        };
        let piece = if edge.top.map(|p| [p[0], p[1]]) == edge.foot.map(|p| [p[0], p[1]]) {
            let rise = edge.top[0][2]
                .checked_sub(edge.foot[0][2])
                .ok_or_else(too_big)?;
            let by = [0, 0, units.from_q0(rise)?];
            entry.extrude = as_steps(&foot);
            entry.by = Some(by);
            let profile = Profile {
                outer: foot,
                holes: Vec::new(),
            };
            engine.extrude(&profile, by).map_err(from_engine)?
        } else {
            let sections = [Section { corners: foot }, Section { corners: top }];
            entry.loft = sections.iter().map(|s| as_steps(&s.corners)).collect();
            engine.loft(&sections).map_err(from_engine)?
        };
        asked.push(entry);
        layer = engine.unite(&layer, &piece).map_err(from_engine)?;
    }
    Ok(layer)
}

impl<E: Engine> Kernel<E> {
    /// Builds a filled floor recipe with the openings and shafts `hosted` cut through every layer,
    /// as the module header says.
    ///
    /// # Errors
    ///
    /// [`FloorPlan::of`]'s refusals; those of [`floor_cuts`] for what is hosted, each naming it;
    /// `NotOnTheGrid` for a length that is not whole steps of the engine; and the engine's own,
    /// naming what was cut through the floor.
    #[allow(
        clippy::too_many_lines,
        reason = "one floor's cuts, layers, meshes and names, in the order the record lists them"
    )]
    pub(super) fn build_floor(
        &self,
        recipe: &Recipe,
        hosted: &[Hosted<'_>],
    ) -> Result<Built, GeometryError> {
        let plan = FloorPlan::of(recipe)?;
        let units = self.units;
        let cuts = floor_cuts(&plan, hosted, units)?;
        let op = op_id(recipe)?;
        let mut asked = Asked {
            layers: Vec::new(),
            cut: cuts.iter().map(|c| c.key.clone()).collect(),
        };
        let named = |error: GeometryError| {
            if cuts.is_empty() {
                return error;
            }
            let through: Vec<&str> = cuts.iter().map(|c| c.who.as_str()).collect();
            fault(
                &error.code,
                format!(
                    "with {} cut through it: {}",
                    through.join(", "),
                    error.message
                ),
            )
        };
        let mut mesh = Mesh {
            positions: Vec::new(),
            indices: Vec::new(),
        };
        let mut extent: Extent = None;
        let mut rounded = false;
        let outline: Vec<[i64; 2]> = plan
            .outer()
            .iter()
            .map(|corner| {
                let [x, y] = corner.at;
                [floor_plan::nearest_mm(x).0, floor_plan::nearest_mm(y).0]
            })
            .collect();
        let mut parts = Vec::new();
        {
            let mut engine = self
                .engine
                .lock()
                .map_err(|_| fault("EngineUnavailable", "an earlier build stopped part way"))?;
            let slabs = plan.slabs();
            let last = slabs.len().saturating_sub(1);
            let sloped = plan.tops.iter().flatten().any(|&z| z != 0);
            for (n, (at, layer, bottom, top)) in slabs.into_iter().enumerate() {
                let up = units.from_q0(top.saturating_sub(bottom))?;
                let z = units.from_q0(bottom)?;
                let stand = edited::stand(&plan, at);
                if stand != Stand::Flat {
                    let mut entry = AskedLayer {
                        function: layer.function.clone(),
                        material: layer.material.clone(),
                        extrude: Vec::new(),
                        holes: Vec::new(),
                        by: [0, 0, up],
                        lofted: Some(true),
                        edges: Vec::new(),
                        faces: Vec::new(),
                    };
                    let solid = edited::layer(
                        &mut *engine,
                        &plan,
                        units,
                        (bottom, top),
                        stand,
                        &mut entry.faces,
                    )?;
                    asked.layers.push(entry);
                    let shown = engine.mesh(&solid).map_err(from_engine)?;
                    let appended = append(&mut mesh, &shown, units, &mut extent)?;
                    rounded |= appended.rounded;
                    parts.push(Part {
                        material: layer.material.clone(),
                        function: layer.function.clone(),
                        triangles: appended.triangles,
                        outline: outline.clone(),
                    });
                    continue;
                }
                let outer = lifted_ring(&plan, 0, plan.outer(), units, bottom)?;
                let mut holes = plan
                    .holes()
                    .iter()
                    .enumerate()
                    .map(|(k, hole)| lifted_ring(&plan, k.saturating_add(1), hole, units, bottom))
                    .collect::<Result<Vec<_>, _>>()?;
                for cut in &cuts {
                    let mut hole = Vec::with_capacity(cut.ring.len());
                    for &[x, y] in &cut.ring {
                        hole.push(P3::new(x, y, z).map_err(|_| too_big())?);
                    }
                    holes.push(hole);
                }
                let by = [0, 0, up];
                let mut entry = AskedLayer {
                    function: layer.function.clone(),
                    material: layer.material.clone(),
                    extrude: as_steps(&outer),
                    holes: holes.iter().map(|h| as_steps(h)).collect(),
                    by,
                    lofted: sloped.then_some(true),
                    edges: Vec::new(),
                    faces: Vec::new(),
                };
                let mut solid = if sloped {
                    sloped_layer(&mut *engine, &outer, &holes, by)?
                } else {
                    let profile = Profile { outer, holes };
                    engine
                        .extrude(&profile, by)
                        .map_err(|e| named(from_engine(e)))?
                };
                if n == last {
                    solid = with_edges(&mut *engine, &plan, units, solid, &mut entry.edges)?;
                }
                asked.layers.push(entry);
                let shown = engine.mesh(&solid).map_err(|e| named(from_engine(e)))?;
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
            extent.ok_or_else(|| fault("EmptyShape", "a floor with no layer of any thickness"))?;
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
        let mut faces = names(op, &plan)?;
        for cut in &cuts {
            let roles = (0..cut.ring.len()).map(|i| format!("side.{i}"));
            faces.extend(tool_names(&cut.who, &cut.key, cut.op, op, roles)?);
        }
        faces.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Built {
            body: serde_json::to_vec(&asked).unwrap_or_default(),
            bounds,
            names: faces,
            displays,
        })
    }
}
