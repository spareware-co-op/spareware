//! The kernel's engine seam answered with solidcore behind it: the first engine a program codes
//! against through `engine-geometry`.
//!
//! What it does: [`SolidcoreEngine`] implements `geom_op::engine::Engine` on one grid step. Where
//! the kernel's own exact operations answer, they do; solidcore answers only what they cannot, as
//! decision 32.70 gives it:
//!
//! - `extrude`: a profile with no holes, level and swept straight up, goes to the kernel's exact
//!   `make.prism` (geom-make); anything else, and anything `make.prism` refuses (a profile that is
//!   not strictly convex), to solidcore's linear sweep, through [`Modeller::extrude`] (decision
//!   32.91), its body converted and checked as the kernel's and each flat face's plane written
//!   through its own grid corners ([`on_its_corners`]);
//! - `subtract`: the kernel's exact `geom-bool` (`combine` with one subtract selection), since
//!   flat-faced booleans stay the kernel's (32.70, rule 5); `subtract_pieces` the same, its
//!   selection every cell inside the target and outside the tool, one solid each, ordered by
//!   their corners ([`corners_of`]);
//! - `loft`: for sections whose walls are flat (each wall's four corners exactly on one plane,
//!   decided here on the grid), the solid the walls bound written as the kernel's rows here
//!   ([`lofted`]): each wall and each end one flat face through its own grid corners, however
//!   little a wall turns from its neighbours, Exact. solidcore's skin is not used for it: the
//!   skin joins neighbouring walls that turn by less than half a degree, across sections or
//!   between two sides of one section, into one bent B-spline face (the curve and surface
//!   continuity tests of solidcore at the pin, `CONTINUITY_ANGLE`, a constant no caller sets),
//!   which the kernel has no kind for. A wall that is not flat is unsupported, as the kernel has
//!   no kind for a ruled surface yet. The sections are checked first as [`crate::loft_reversed`]
//!   checks them, and the kernel's checker, which every result passes, refuses a body whose
//!   walls cross: a folded or crossed loft is an error, never a solid;
//! - `unite`: the kernel's exact `geom-bool` (`combine` with the unite selection) when both
//!   solids are flat-faced (32.70, rule 5), and solidcore's boolean union ([`Modeller::union`])
//!   otherwise, on the solidcore bodies the engine keeps of the solids solidcore made; a curved
//!   operand united with one solidcore holds no body of is unsupported;
//! - `volume`: the kernel's exact six-volume of the body's rows (`geom_topo::query_ops::six_volume`);
//! - `mesh`: each flat face's loops filled by solidcore-tess's planar triangulation
//!   (`solidcore_tess::triangulate_planar`), the face's own corners and no point added, so faces
//!   that share an edge share its points bit for bit;
//! - `section`: a flat-faced solid cut by a plane square to a grid axis, exactly, by the kernel's
//!   own convex pieces of its faces (`geom_bool::operand::prepare`), cut by the plane's piece
//!   (`geom_poly::cut::local_cuts`) and met with it (`geom_poly::merge::section`), the meeting
//!   curves joined into loops here ([`crate::cut`]); a curved solid, another plane, and a corner of
//!   the solid on the plane are unsupported;
//! - `edition`: the engine's name, solidcore's pinned commit, the grid step, solidcore's
//!   tolerances as its kernel holds them, and the list above.
//!
//! An [`EngineSolid`] is the kernel's checked body, its exactness and what made it
//! ([`EngineSolid::made_by`]), and, privately, solidcore's own body of it when solidcore made it;
//! no solidcore type is in its interface.
//!
//! Interim: this whole module answers the seam until the kernel's own engine does; the loft's
//! rows until geom-sweep's ruled loft (chapter 13.6) writes them (`tests/interim.json`).
//!
//! Not yet: a loft wall that is not flat is not made, a curved face is not meshed (solidcore-tess's planner works on solidcore's record of a
//! body, and a body `geom-bool` made has none), and `subtract` returns a cut that leaves several
//! pieces as one solid of as many shells (`subtract_pieces` returns each piece).
//!
//! SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;
use std::fmt::Display;
use std::sync::Arc;

use geom_bool::combine::combine;
use geom_bool::operand::{BooleanInput, Operand, OperandKey, OperandKind, Selection, Term};
use geom_check::{Level, check};
use geom_make::{PrismInput, prism};
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_op::engine::{
    Answer, Cut, DisplayMesh, Edition, Engine, EngineError, GridPlane, Profile, Section, Tolerance,
    Volume,
};
use geom_op::{OpContext, OpId, OpPath};
use geom_poly::merge::Merge;
use geom_run::{Budget, Exec, WorkMeter};
use geom_topo::query::{face_area, loop_area, point};
use geom_topo::query_ops::six_volume;
use geom_topo::{
    Body, BodyKind, Box3, Builder, CurveKind, Edge, EdgeIx, Face, FaceIx, Fin, FinIx, FlatSurface,
    Ix, Line, Loop, LoopIx, PointKind, Region, RegionKind, Sense, Shell, SurfaceKind, Vertex,
    VertexIx,
};
use solidcore::KernelOptions;

use crate::{Corner, Hole, Modeller, Solid};

/// The engine's name in its [`Edition`].
pub const ENGINE: &str = "geom-solidcore";

/// solidcore's repository and the commit this crate pins (its Cargo.toml's `rev`).
pub const SOLIDCORE: &str = "solidcore https://github.com/spareware-co-op/solidcore 1d8e14b46dedccbb03572956e7c708782fef1b8f";

/// The operation path the engine's kernel calls run under.
const PATH: [u8; 16] = *b"geom-solidcore-e";

/// A solid of the engine: the kernel's checked body and the exactness it carries, and solidcore's
/// own body of it when solidcore made it, which only solidcore's union reads.
#[derive(Clone, Debug)]
pub struct EngineSolid {
    body: Body,
    exactness: Exactness,
    made_by: &'static str,
    solidcore: Option<Arc<Solid>>,
}

/// Two solids are equal when their kernel bodies, exactness and makers are: solidcore's body of a
/// solid is a copy of the same shape, kept for solidcore's own operations.
impl PartialEq for EngineSolid {
    fn eq(&self, other: &EngineSolid) -> bool {
        (&self.body, self.exactness, self.made_by) == (&other.body, other.exactness, other.made_by)
    }
}

/// The extrusions the kernel's prism takes.
pub const PRISM_CASE: &str =
    "a profile with no holes, level, swept straight up, which make.prism takes (strictly convex)";

/// The extrusions solidcore takes.
pub const SWEEP_CASE: &str = "every other profile: holes, a profile not level or not swept straight up, or one make.prism refuses";

/// The case of an operation one part answers whole.
pub const EVERY_CASE: &str = "every case";

/// What made a solid: the kernel's exact prism.
pub const BY_PRISM: &str = "the kernel's make.prism (geom-make)";

/// What made a solid: solidcore's linear sweep.
pub const BY_SWEEP: &str = "solidcore's linear sweep (decision 32.91)";

/// What made a solid: the kernel's exact boolean.
pub const BY_BOOL: &str = "the kernel's geom-bool (decision 32.70, rule 5)";

/// The lofts the engine makes.
pub const LOFT_CASE: &str = "sections whose walls are flat: each wall's four corners on one plane";

/// What made a solid: the engine's own rows of a loft.
pub const BY_LOFT: &str = "the engine's own rows: each wall and each end one flat face through the sections' grid corners, Exact";

/// The unions the kernel's boolean takes.
pub const FLAT_CASE: &str = "both solids flat-faced";

/// The unions solidcore takes.
pub const CURVED_CASE: &str =
    "a solid with a curved face, united with one solidcore made (it keeps solidcore's body of it)";

/// What made a solid: solidcore's boolean union.
pub const BY_UNION: &str = "solidcore's boolean union";

/// The sections the engine answers.
pub const CUT_CASE: &str = "a flat-faced solid and a plane square to a grid axis";

/// What answers a section: the kernel's own pieces and meeting curves, joined here.
pub const BY_CUT: &str = "the kernel's geom-bool prepare and geom-poly local_cuts and section, joined into loops by geom-solidcore";

impl EngineSolid {
    /// What made it, as the engine's edition names it: [`BY_PRISM`], [`BY_SWEEP`] or
    /// [`BY_BOOL`].
    #[must_use]
    pub fn made_by(&self) -> &'static str {
        self.made_by
    }

    /// The kernel's body, which `geom-check` passed.
    #[must_use]
    pub fn body(&self) -> &Body {
        &self.body
    }

    /// The exactness class the body carries.
    #[must_use]
    pub fn exactness(&self) -> Exactness {
        self.exactness
    }

    /// The same solid, keeping solidcore's body `made` of it.
    fn keeping(self, made: Solid) -> EngineSolid {
        EngineSolid {
            solidcore: Some(Arc::new(made)),
            ..self
        }
    }

    /// Whether every face of the body is flat.
    fn flat(&self) -> bool {
        let a = self.body.arenas();
        (0..a.faces.len()).all(|row| {
            FaceIx::new(row)
                .and_then(|f| a.faces.get(f))
                .and_then(|f| a.surfaces.get(f.surface))
                .is_some_and(|s| matches!(s, SurfaceKind::Plane(_)))
        })
    }
}

/// The engine: a [`Modeller`] for what solidcore answers, on the same grid step as the kernel's
/// own calls.
#[derive(Debug)]
pub struct SolidcoreEngine {
    modeller: Modeller,
}

impl SolidcoreEngine {
    /// An engine on the grid step `step`, with solidcore's default options.
    #[must_use]
    pub fn new(step: GridStep) -> SolidcoreEngine {
        SolidcoreEngine {
            modeller: Modeller::new(step),
        }
    }
}

fn refused(operation: &'static str, reason: impl Display) -> EngineError {
    EngineError::Refused {
        operation,
        reason: reason.to_string(),
    }
}

fn unsupported(operation: &'static str, what: &str) -> EngineError {
    EngineError::Unsupported {
        operation,
        what: what.to_owned(),
    }
}

/// How many edges of `body` more than two faces meet at: the radial ring of fins round each edge
/// counted, a ring that does not come back to its first fin within the body's fins counted as far
/// as it went.
fn crowded_edges(body: &Body) -> u32 {
    let a = body.arenas();
    let mut crowded = 0;
    for row in 0..a.edges.len() {
        let Some(edge) = EdgeIx::new(row).and_then(|e| a.edges.get(e)) else {
            continue;
        };
        let first = edge.first_fin;
        let (mut at, mut fins) = (first, 0_u32);
        while let Some(fin) = at {
            fins += 1;
            if fins > a.fins.len() {
                break;
            }
            at = a
                .fins
                .get(fin)
                .map(|f| f.radial)
                .filter(|&r| Some(r) != first);
        }
        if fins > 2 {
            crowded += 1;
        }
    }
    crowded
}

/// Why `body`, which is not of the kind Solid, is no solid, in plain words: nothing kept, solids
/// that touch along an edge (an edge more than two faces meet at), or at a point (a vertex the
/// faces round which make more than one fan). A union of solids that touch only so has no solid
/// answer: what a body of the kind Solid bounds is manifold everywhere (chapter 8.1).
fn not_a_solid(body: &Body) -> String {
    let edges = crowded_edges(body);
    if body.arenas().faces.is_empty() {
        "the result is not a solid: it keeps nothing".to_owned()
    } else if edges > 0 {
        format!(
            "the result is not a solid: solids in it touch along an edge without sharing a face there ({edges} edges where more than two faces meet), so it is non-manifold, which no solid is"
        )
    } else {
        "the result is not a solid: solids in it touch at a point (a vertex where the faces meet in more than one fan), so it is non-manifold, which no solid is".to_owned()
    }
}

/// `body` as the engine's solid, only if it is a solid the kernel's checker passes at its full
/// level with `exactness`: every result the engine returns comes through here, so a body that is
/// not closed, or fails any other check, is refused and never handed to the program; a body of
/// another kind is refused saying why it is none ([`not_a_solid`]).
fn accepted(
    operation: &'static str,
    (body, exactness): (Body, Exactness),
    made_by: &'static str,
) -> Result<EngineSolid, EngineError> {
    if body.kind() != BodyKind::Solid {
        return Err(refused(operation, not_a_solid(&body)));
    }
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let report = check(&body, exactness, Level::Full, &mut meter)
        .map_err(|_| refused(operation, "the checker stopped on an unlimited meter"))?;
    if !report.passed() {
        let first = report.faults().first().map_or_else(
            || "faults past the cap only".to_owned(),
            ToString::to_string,
        );
        return Err(refused(
            operation,
            format!("the kernel's checker refused the result: {first}"),
        ));
    }
    Ok(EngineSolid {
        body,
        exactness,
        made_by,
        solidcore: None,
    })
}

/// The base of `profile` and the height of `by` as the kernel's `make.prism` takes them, when it
/// can: no holes, every corner at one height, `by` straight up; the base turned counter-clockwise
/// from above, decided exactly from its corners' whole steps.
fn prism_input(profile: &Profile, by: [i64; 3]) -> Option<PrismInput> {
    let [0, 0, h] = by else {
        return None;
    };
    let z = profile.outer.first()?.z;
    if !profile.holes.is_empty() || h <= 0 || profile.outer.iter().any(|p| p.z != z) {
        return None;
    }
    let mut base = profile.outer.clone();
    let turn: i128 = base
        .iter()
        .zip(base.iter().cycle().skip(1))
        .map(|(p, q)| {
            let (px, py) = (i128::from(p.x.get()), i128::from(p.y.get()));
            let (qx, qy) = (i128::from(q.x.get()), i128::from(q.y.get()));
            px * qy - py * qx
        })
        .sum();
    if turn < 0 {
        base.reverse();
    }
    Some(PrismInput {
        base,
        height: Steps(h.unsigned_abs()),
    })
}

/// The context the kernel's own calls run in: no budget limit, the engine's path.
fn context() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes(PATH)), Budget::UNLIMITED)
}

/// The corners of loop `l` of `body`, in ring order: each fin's start, every edge a line between
/// two grid points.
fn ring(body: &Body, l: LoopIx) -> Result<Vec<P3>, EngineError> {
    const OP: &str = "mesh";
    let a = body.arenas();
    let row = a
        .loops
        .get(l)
        .ok_or_else(|| refused(OP, "a loop out of its arena"))?;
    let first: Option<FinIx> = row.first_fin;
    let mut at = first;
    let mut out = Vec::new();
    while let Some(fin) = at {
        if out.len() > a.fins.len() as usize {
            return Err(refused(OP, "a loop's ring does not close"));
        }
        let f = a
            .fins
            .get(fin)
            .ok_or_else(|| refused(OP, "a fin out of its arena"))?;
        let e = a
            .edges
            .get(f.edge)
            .ok_or_else(|| refused(OP, "an edge out of its arena"))?;
        if !matches!(a.curves.get(e.curve), Some(CurveKind::Line(_))) {
            return Err(unsupported(OP, "a curved edge"));
        }
        let v = match f.sense {
            Sense::Forward => e.start,
            Sense::Reversed => e.end,
        }
        .ok_or_else(|| unsupported(OP, "an edge with no vertex"))?;
        out.push(point(body, v).map_err(|e| refused(OP, e))?);
        at = Some(f.next).filter(|&n| Some(n) != first);
    }
    Ok(out)
}

/// The f64 of a grid coordinate: exact, since every coordinate is under 2^40 steps.
#[allow(
    clippy::cast_precision_loss,
    reason = "a coordinate is under 2^40 steps, which an f64 holds exactly"
)]
fn coords(p: P3) -> [f64; 3] {
    [p.x.get() as f64, p.y.get() as f64, p.z.get() as f64]
}

/// `p` without its coordinate along `axis`, the other two in cyclic order, so that a polygon
/// turning positively about `+axis` turns counter-clockwise in the plane.
fn projected(p: P3, axis: usize) -> [f64; 2] {
    let c = coords(p);
    match axis {
        0 => [c[1], c[2]],
        1 => [c[2], c[0]],
        _ => [c[0], c[1]],
    }
}

/// Adds the triangles of the flat face `face` to `out`.
#[allow(
    clippy::cast_precision_loss,
    reason = "the direction of an area vector, which an f64 keeps to rounding"
)]
fn mesh_face(body: &Body, face: FaceIx, out: &mut DisplayMesh) -> Result<(), EngineError> {
    const OP: &str = "mesh";
    let a = body.arenas();
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let f = a
        .faces
        .get(face)
        .ok_or_else(|| refused(OP, "a face out of its arena"))?;
    if !matches!(a.surfaces.get(f.surface), Some(SurfaceKind::Plane(_))) {
        return Err(unsupported(OP, "a curved face"));
    }
    let n = face_area(body, face, &mut meter).map_err(|e| refused(OP, e))?;
    let nf = n.map(|c| c as f64);
    let axis = (0..3)
        .max_by(|&i, &j| nf[i].abs().total_cmp(&nf[j].abs()))
        .unwrap_or(2);
    let length = (nf[0] * nf[0] + nf[1] * nf[1] + nf[2] * nf[2]).sqrt();
    if length == 0.0 {
        return Err(refused(OP, "a face with no area"));
    }
    let normal = nf.map(|c| c / length);
    // The outer loop turns with the face's normal and every hole against it; the loops' area
    // vectors are parallel to the normal, so the sign of each product is the sign of the dot.
    let (mut outer, mut holes) = (Vec::new(), Vec::new());
    let mut at = f.first_loop;
    let mut seen = 0_u32;
    while let Some(l) = at {
        seen += 1;
        if seen > a.loops.len() {
            return Err(refused(OP, "a face's loops do not end"));
        }
        let area = loop_area(body, l, &mut meter).map_err(|e| refused(OP, e))?;
        let dot: f64 = area.iter().zip(nf).map(|(&c, m)| c as f64 * m).sum();
        let corners = ring(body, l)?;
        if dot > 0.0 {
            outer.push(corners);
        } else {
            holes.push(corners);
        }
        at = a.loops.get(l).and_then(|r| r.next);
    }
    if outer.len() != 1 {
        return Err(unsupported(OP, "a face without exactly one outer loop"));
    }
    let corners: Vec<P3> = outer.iter().chain(&holes).flatten().copied().collect();
    let loops: Vec<Vec<[f64; 2]>> = outer
        .iter()
        .chain(&holes)
        .map(|ring| ring.iter().map(|&p| projected(p, axis)).collect())
        .collect();
    let (points, triangles) =
        solidcore_tess::triangulate_planar(&loops).map_err(|e| refused(OP, e))?;
    if points.len() != corners.len() {
        return Err(refused(OP, "the triangulation added a point"));
    }
    let base = u32::try_from(out.positions.len()).map_err(|e| refused(OP, e))?;
    for &p in &corners {
        out.positions.push(coords(p));
        out.normals.push(normal);
    }
    let flip = nf[axis] < 0.0;
    for t in triangles {
        let [i, j, k] = t.map(|i| base + i);
        out.triangles.push(if flip { [i, k, j] } else { [i, j, k] });
    }
    Ok(())
}

/// `a × b`, exactly: each coordinate is under 2^41 in size, so each product is under 2^82 and
/// each difference under 2^83, inside i128.
fn cross(a: [i128; 3], b: [i128; 3]) -> [i128; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The grid point `p` as whole steps.
fn whole(p: P3) -> [i128; 3] {
    [p.x.get(), p.y.get(), p.z.get()].map(i128::from)
}

/// `body` with each flat face's plane written through three of its own grid corners, its normal
/// on the side the plane solidcore wrote faces; the body unchanged where a face is curved or its
/// first loop has an edge that is not a line between grid points.
///
/// solidcore writes a plane in metres, and its axes come back to the grid as doubles that need not
/// be whole numbers of steps, while the kernel's exact `geom-bool` takes a face only on a plane
/// given by grid points. Every corner of the face stays where it is; the result is checked again
/// as every result is ([`accepted`]), so a face whose corners are not exactly on one plane is
/// refused there, never kept.
#[allow(
    clippy::cast_precision_loss,
    reason = "whole steps under 2^41, which an f64 holds exactly, and a direction's sign"
)]
fn on_its_corners(operation: &'static str, body: &Body) -> Result<Body, EngineError> {
    let a = body.arenas();
    let mut b = Builder::new();
    b.arenas = a.clone();
    for row in 0..a.faces.len() {
        let face =
            FaceIx::new(row).ok_or_else(|| refused(operation, "a face row past the index"))?;
        let f = a
            .faces
            .get(face)
            .ok_or_else(|| refused(operation, "a face out of its arena"))?;
        let Some(SurfaceKind::Plane(flat)) = a.surfaces.get(f.surface) else {
            return Ok(body.clone());
        };
        let Some(first) = f.first_loop else {
            return Ok(body.clone());
        };
        let Ok(corners) = ring(body, first) else {
            return Ok(body.clone());
        };
        let Some(&p0) = corners.first() else {
            return Ok(body.clone());
        };
        let o = whole(p0);
        let less = |p: P3| {
            let q = whole(p);
            [q[0] - o[0], q[1] - o[1], q[2] - o[2]]
        };
        let along = corners.iter().map(|&p| less(p)).find(|d| *d != [0; 3]);
        let across = along.and_then(|u| {
            corners
                .iter()
                .map(|&p| less(p))
                .find(|&d| cross(u, d) != [0; 3])
                .map(|v| (u, v))
        });
        let Some((u, v)) = across else {
            return Err(refused(
                operation,
                "a flat face whose corners lie on one line",
            ));
        };
        let (_, fu, fv) = flat.vectors();
        let wrote = [
            fu[1] * fv[2] - fu[2] * fv[1],
            fu[2] * fv[0] - fu[0] * fv[2],
            fu[0] * fv[1] - fu[1] * fv[0],
        ];
        let n = cross(u, v);
        let facing: f64 = n.iter().zip(wrote).map(|(&c, w)| c as f64 * w).sum();
        let (u, v) = if facing < 0.0 { (v, u) } else { (u, v) };
        // The plane's own coordinates of each corner, through the dual basis of (u, v), and a
        // step's margin round them, as the conversion writes a plane's domain.
        let (uf, vf) = (u.map(|c| c as f64), v.map(|c| c as f64));
        let dot = |x: [f64; 3], y: [f64; 3]| x[0] * y[0] + x[1] * y[1] + x[2] * y[2];
        let (uu, uv, vv) = (dot(uf, uf), dot(uf, vf), dot(vf, vf));
        let det = uu * vv - uv * uv;
        let (mut su, mut sv) = (
            (f64::INFINITY, f64::NEG_INFINITY),
            (f64::INFINITY, f64::NEG_INFINITY),
        );
        for &p in &corners {
            let d = less(p).map(|c| c as f64);
            let (x, y) = (
                (vv * dot(d, uf) - uv * dot(d, vf)) / det,
                (uu * dot(d, vf) - uv * dot(d, uf)) / det,
            );
            su = (su.0.min(x - 1.0), su.1.max(x + 1.0));
            sv = (sv.0.min(y - 1.0), sv.1.max(y + 1.0));
        }
        let plane = FlatSurface::new(o.map(|c| c as f64), uf, vf, (su, sv))
            .map_err(|e| refused(operation, format!("{e:?}")))?;
        b.arenas
            .surfaces
            .set(f.surface, SurfaceKind::Plane(plane))
            .map_err(|e| refused(operation, format!("{e:?}")))?;
    }
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    b.publish(body.kind(), body.outside(), &mut meter)
        .map_err(|e| refused(operation, format!("{e:?}")))
}

/// The index of row `row`.
fn ix<K>(operation: &'static str, row: usize) -> Result<Ix<K>, EngineError> {
    u32::try_from(row)
        .ok()
        .and_then(Ix::new)
        .ok_or_else(|| refused(operation, "more rows than an index holds"))
}

/// Twice the vector area of the polygon `ps` (its Newell normal), in squared steps, exactly, taken
/// about its first corner: each difference is under 2^41 and each term of the sum under 2^83, so a
/// sum of fewer than 2^43 terms stays inside i128.
fn newell(ps: &[P3]) -> [i128; 3] {
    let Some(&first) = ps.first() else {
        return [0; 3];
    };
    let o = whole(first);
    let less = |p: P3| {
        let q = whole(p);
        [q[0] - o[0], q[1] - o[1], q[2] - o[2]]
    };
    let mut n = [0_i128; 3];
    for w in ps.windows(2).skip(1) {
        let c = cross(less(w[0]), less(w[1]));
        for (s, t) in n.iter_mut().zip(c) {
            *s += t;
        }
    }
    n
}

/// The plane of the flat polygon `ps`, which runs counter-clockwise seen from the side its normal
/// points to: through its first corner, along two of its sides' grid vectors from there whose
/// cross product points the way its Newell normal does, decided exactly on the axis where that
/// normal is largest (every such cross product is a multiple of the normal, as the corners are on
/// one plane); its domain the corners' own coordinates in the plane and a step round them.
#[allow(
    clippy::cast_precision_loss,
    reason = "whole steps under 2^41, which an f64 holds exactly"
)]
fn plane_of(operation: &'static str, ps: &[P3]) -> Result<FlatSurface, EngineError> {
    let n = newell(ps);
    let axis = (0..3).max_by_key(|&a| n[a].unsigned_abs()).unwrap_or(2);
    let Some(&first) = ps.first() else {
        return Err(refused(operation, "a face with no corner"));
    };
    if n[axis] == 0 {
        return Err(refused(operation, "a face with no area"));
    }
    let o = whole(first);
    let less = |p: P3| {
        let q = whole(p);
        [q[0] - o[0], q[1] - o[1], q[2] - o[2]]
    };
    let along = ps.iter().map(|&p| less(p)).find(|d| *d != [0; 3]);
    let across = along.and_then(|u| {
        ps.iter()
            .map(|&p| less(p))
            .find(|&d| {
                let c = cross(u, d)[axis];
                c != 0 && (c > 0) == (n[axis] > 0)
            })
            .map(|v| (u, v))
    });
    let Some((u, v)) = across else {
        return Err(refused(operation, "a face whose corners lie on one line"));
    };
    let (uf, vf) = (u.map(|c| c as f64), v.map(|c| c as f64));
    let dot = |x: [f64; 3], y: [f64; 3]| x[0] * y[0] + x[1] * y[1] + x[2] * y[2];
    let (uu, uv, vv) = (dot(uf, uf), dot(uf, vf), dot(vf, vf));
    let det = uu * vv - uv * uv;
    let (mut su, mut sv) = (
        (f64::INFINITY, f64::NEG_INFINITY),
        (f64::INFINITY, f64::NEG_INFINITY),
    );
    for &p in ps {
        let d = less(p).map(|c| c as f64);
        let (x, y) = (
            (vv * dot(d, uf) - uv * dot(d, vf)) / det,
            (uu * dot(d, vf) - uv * dot(d, uf)) / det,
        );
        su = (su.0.min(x - 1.0), su.1.max(x + 1.0));
        sv = (sv.0.min(y - 1.0), sv.1.max(y + 1.0));
    }
    FlatSurface::new(o.map(|c| c as f64), uf, vf, (su, sv))
        .map_err(|e| refused(operation, format!("{e:?}")))
}

/// The box of the grid points `ps`.
fn box_of(operation: &'static str, ps: &[P3]) -> Result<Box3, EngineError> {
    let first = ps
        .first()
        .copied()
        .ok_or_else(|| refused(operation, "a face with no corner"))?;
    let pick = |f: fn(i64, i64) -> i64| {
        let [x, y, z] = ps.iter().fold(
            [first.x.get(), first.y.get(), first.z.get()],
            |[x, y, z], p| [f(x, p.x.get()), f(y, p.y.get()), f(z, p.z.get())],
        );
        P3::new(x, y, z).map_err(|e| refused(operation, e))
    };
    Box3::new(pick(i64::min)?, pick(i64::max)?).map_err(|e| refused(operation, e))
}

/// The solid lofted through `sections`, which [`crate::loft_reversed`] passed (`reversed` its
/// answer) and whose walls are flat, as the kernel's rows, written here and published unchecked:
/// one vertex at each corner of each section; the first section a face facing back and the last a
/// face facing ahead; between each section and the next, one flat face for each pair of its
/// corners that a side joins, the wall to the same pair of the next section, however little it
/// turns from its neighbours (a wall in one plane with the next is its own face all the same);
/// one edge for each side of a section and each corner's line to the next section's; every face
/// on the plane through its own grid corners ([`plane_of`]), every edge a line between two grid
/// points. Each section is taken turning positively about the way ahead, every one listed the
/// other way round when the first turns negatively, which keeps corner `i` of each paired with
/// corner `i` of the next. Nothing is rounded: the body is Exact. A face with no area (a crossed
/// wall whose halves cancel) is refused here, and the checker refuses any other body whose faces
/// cross ([`accepted`]).
#[allow(
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    reason = "whole steps under 2^41, which an f64 holds exactly; one pass writes every arena's rows"
)]
fn lofted(sections: &[Section], reversed: bool) -> Result<Body, EngineError> {
    const OP: &str = "loft";
    let broke = |e| refused(OP, format!("{e:?}"));
    let n = sections.first().map_or(0, |s| s.corners.len());
    let m = sections.len();
    let mut points = Vec::with_capacity(m * n);
    for s in sections {
        for i in 0..n {
            let at = if reversed { (n - i) % n } else { i };
            points.push(
                *s.corners
                    .get(at)
                    .ok_or_else(|| refused(OP, "a section short of corners"))?,
            );
        }
    }
    // Each face's ring of vertex rows, counter-clockwise seen from outside.
    let mut rings: Vec<Vec<usize>> = Vec::with_capacity(2 + (m - 1) * n);
    rings.push((0..n).map(|i| (n - i) % n).collect());
    for k in 0..m - 1 {
        for i in 0..n {
            let j = (i + 1) % n;
            rings.push(vec![k * n + i, k * n + j, (k + 1) * n + j, (k + 1) * n + i]);
        }
    }
    rings.push((0..n).map(|i| (m - 1) * n + i).collect());

    let mut edge_of: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    let mut ends: Vec<(usize, usize)> = Vec::new();
    let mut ring_of: Vec<Vec<usize>> = Vec::new();
    let (mut faces, mut surfaces, mut loops, mut fins) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let count = rings.len();
    for (f, ring) in rings.iter().enumerate() {
        let corners: Vec<P3> = ring.iter().map(|&v| points[v]).collect();
        surfaces.push(SurfaceKind::Plane(plane_of(OP, &corners)?));
        let first = fins.len();
        let len = ring.len();
        loops.push(Loop {
            face: ix(OP, f)?,
            next: None,
            first_fin: Some(ix(OP, first)?),
        });
        for (s, &a) in ring.iter().enumerate() {
            let b = ring[(s + 1) % len];
            let key = (a.min(b), a.max(b));
            let e = *edge_of.entry(key).or_insert_with(|| {
                ends.push(key);
                ring_of.push(Vec::new());
                ends.len() - 1
            });
            ring_of[e].push(first + s);
            fins.push(Fin {
                loop_: ix(OP, f)?,
                next: ix(OP, first + (s + 1) % len)?,
                prev: ix(OP, first + (s + len - 1) % len)?,
                edge: ix(OP, e)?,
                sense: if a == key.0 {
                    Sense::Forward
                } else {
                    Sense::Reversed
                },
                radial: ix(OP, first + s)?,
                pcurve: None,
            });
        }
        faces.push(Face {
            shell: ix(OP, 0)?,
            next: if f + 1 < count {
                Some(ix(OP, f + 1)?)
            } else {
                None
            },
            surface: ix(OP, f)?,
            sense: Sense::Forward,
            first_loop: Some(ix(OP, f)?),
            bound: box_of(OP, &corners)?,
        });
    }
    let mut edges = Vec::with_capacity(ends.len());
    let mut curves = Vec::with_capacity(ends.len());
    for (e, &(a, b)) in ends.iter().enumerate() {
        let [x, y] = ring_of[e][..] else {
            return Err(refused(OP, "an edge not shared by exactly two faces"));
        };
        fins[x].radial = ix(OP, y)?;
        fins[y].radial = ix(OP, x)?;
        let (from, to) = (coords(points[a]), coords(points[b]));
        let line = Line::new(
            from,
            [to[0] - from[0], to[1] - from[1], to[2] - from[2]],
            (0.0, 1.0),
        )
        .map_err(|e| refused(OP, format!("{e:?}")))?;
        curves.push(CurveKind::Line(line));
        edges.push(Edge {
            first_fin: Some(ix(OP, x)?),
            start: Some(ix(OP, a)?),
            end: Some(ix(OP, b)?),
            curve: ix(OP, e)?,
        });
    }
    let vertices: Vec<Vertex> = (0..points.len())
        .map(|v| Ok(Vertex { point: ix(OP, v)? }))
        .collect::<Result<_, EngineError>>()?;
    let regions = [
        Region {
            kind: RegionKind::Void,
            first_shell: None,
        },
        Region {
            kind: RegionKind::Solid,
            first_shell: Some(ix(OP, 0)?),
        },
    ];
    let shells = [Shell {
        region: ix(OP, 1)?,
        next: None,
        first_face: Some(ix(OP, 0)?),
    }];
    let mut b = Builder::new();
    let a = &mut b.arenas;
    a.regions.extend(&regions).map_err(broke)?;
    a.shells.extend(&shells).map_err(broke)?;
    a.faces.extend(&faces).map_err(broke)?;
    a.loops.extend(&loops).map_err(broke)?;
    a.fins.extend(&fins).map_err(broke)?;
    a.edges.extend(&edges).map_err(broke)?;
    a.vertices.extend(&vertices).map_err(broke)?;
    a.surfaces.extend(surfaces).map_err(broke)?;
    a.curves.extend(curves).map_err(broke)?;
    a.points
        .extend(points.into_iter().map(PointKind::Grid).collect::<Vec<_>>())
        .map_err(broke)?;
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    b.publish(BodyKind::Solid, ix(OP, 0)?, &mut meter)
        .map_err(broke)
}

/// The grid points of every vertex of `body`, sorted by x, then y, then z: the key the pieces of a
/// cut are ordered by.
fn corners_of(body: &Body) -> Vec<(i64, i64, i64)> {
    let mut out: Vec<(i64, i64, i64)> = (0..body.arenas().vertices.len())
        .filter_map(VertexIx::new)
        .filter_map(|v| point(body, v).ok())
        .map(|p| (p.x.get(), p.y.get(), p.z.get()))
        .collect();
    out.sort_unstable();
    out
}

impl Engine for SolidcoreEngine {
    type Solid = EngineSolid;

    fn edition(&self) -> Edition {
        let t = KernelOptions::default().tolerances;
        let tol = |name: &str, value: f64, unit: &str| Tolerance {
            name: name.to_owned(),
            value,
            unit: unit.to_owned(),
        };
        let answer = |operation: &str, case: &str, by: &str| Answer {
            operation: operation.to_owned(),
            case: case.to_owned(),
            by: by.to_owned(),
        };
        Edition {
            engine: ENGINE.to_owned(),
            version: SOLIDCORE.to_owned(),
            step: self.modeller.step(),
            tolerances: vec![
                tol("zone_tol_3d", t.zone_tol_3d, "m"),
                tol("xsect_tol_3d", t.xsect_tol_3d, "m"),
                tol("approx_tol_3d", t.approx_tol_3d, "m"),
                tol("ang_tol_deg", t.ang_tol_deg, "degree"),
            ],
            answers: vec![
                answer("extrude", PRISM_CASE, BY_PRISM),
                answer("extrude", SWEEP_CASE, BY_SWEEP),
                answer("subtract", EVERY_CASE, BY_BOOL),
                answer("subtract_pieces", EVERY_CASE, BY_BOOL),
                answer("loft", LOFT_CASE, BY_LOFT),
                answer("unite", FLAT_CASE, BY_BOOL),
                answer("unite", CURVED_CASE, BY_UNION),
                answer(
                    "volume",
                    EVERY_CASE,
                    "the kernel's exact six-volume (geom-topo)",
                ),
                answer(
                    "mesh",
                    EVERY_CASE,
                    "solidcore-tess's planar triangulation of each flat face",
                ),
                answer("section", CUT_CASE, BY_CUT),
            ],
        }
    }

    fn extrude(&mut self, profile: &Profile, by: [i64; 3]) -> Result<EngineSolid, EngineError> {
        const OP: &str = "extrude";
        if let Some(input) = prism_input(profile, by) {
            // What the kernel's prism refuses (a base that is not strictly convex) goes to
            // solidcore below; what it makes is the kernel's own exact body.
            if let Ok(made) = prism(&context(), &input) {
                return accepted(OP, (made.result.body, made.exactness), BY_PRISM);
            }
        }
        let corners = |ps: &[P3]| ps.iter().map(|&p| Corner::from(p)).collect::<Vec<_>>();
        let p = crate::Profile {
            outer: corners(&profile.outer),
            holes: profile
                .holes
                .iter()
                .map(|h| Hole::Polygon(corners(h)))
                .collect(),
        };
        let s = self.modeller.extrude(&p, by).map_err(|e| match e {
            crate::Error::Invalid { what, .. } => EngineError::Invalid {
                operation: OP,
                what,
            },
            other => refused(OP, other),
        })?;
        accepted(OP, (on_its_corners(OP, s.body())?, s.exactness()), BY_SWEEP)
            .map(|made| made.keeping(s))
    }

    fn loft(&mut self, sections: &[Section]) -> Result<EngineSolid, EngineError> {
        const OP: &str = "loft";
        for pair in sections.windows(2) {
            let (a, b) = (&pair[0].corners, &pair[1].corners);
            if a.len() != b.len() {
                // The modeller refuses it by name below.
                break;
            }
            for i in 0..a.len() {
                let j = (i + 1) % a.len();
                let o = whole(a[i]);
                let less = |p: P3| {
                    let q = whole(p);
                    [q[0] - o[0], q[1] - o[1], q[2] - o[2]]
                };
                let (u, v, w) = (less(a[j]), less(b[i]), less(b[j]));
                // Each difference is under 2^41, each product of the cross under 2^83 and the dot
                // of three under 2^126: exact in i128.
                let c = cross(v, w);
                if u[0] * c[0] + u[1] * c[1] + u[2] * c[2] != 0 {
                    return Err(unsupported(
                        OP,
                        "a wall whose four corners are not on one plane (the kernel has no kind for a ruled surface yet)",
                    ));
                }
            }
        }
        let given: Vec<Vec<Corner>> = sections
            .iter()
            .map(|s| s.corners.iter().map(|&p| Corner::from(p)).collect())
            .collect();
        let reversed = crate::loft_reversed(&given).map_err(|e| match e {
            crate::Error::Invalid { what, .. } => EngineError::Invalid {
                operation: OP,
                what,
            },
            other => refused(OP, other),
        })?;
        accepted(OP, (lofted(sections, reversed)?, Exactness::Exact), BY_LOFT)
    }

    fn unite(&mut self, a: &EngineSolid, b: &EngineSolid) -> Result<EngineSolid, EngineError> {
        const OP: &str = "unite";
        if a.flat() && b.flat() {
            let (ka, kb) = (OperandKey([1; 16]), OperandKey([2; 16]));
            fn operand(key: OperandKey, s: &EngineSolid) -> Operand<'_> {
                Operand {
                    key,
                    body: &s.body,
                    kind: OperandKind::Solid,
                    exactness: s.exactness,
                }
            }
            let would = a
                .exactness
                .combine(b.exactness)
                .ok_or_else(|| unsupported(OP, "operands whose exactness has no common class"))?;
            let input = BooleanInput {
                operands: vec![operand(ka, a), operand(kb, b)],
                select: vec![Selection::Unite],
                merge: Merge::RemoveRedundant,
                accept: Some(would),
            };
            let out = combine(&context(), &Exec::serial(), &input).map_err(|e| refused(OP, e))?;
            let exactness = out.exactness;
            let mut lists = out.result;
            let mut bodies = lists.pop().unwrap_or_default();
            if bodies.len() != 1 || !lists.is_empty() {
                return Err(unsupported(OP, "a result that is not one solid"));
            }
            let body = bodies
                .pop()
                .map(|c| c.body)
                .ok_or_else(|| refused(OP, "no body"))?;
            return accepted(OP, (body, exactness), BY_BOOL);
        }
        let (Some(x), Some(y)) = (&a.solidcore, &b.solidcore) else {
            return Err(unsupported(
                OP,
                "a solid with a curved face united with one solidcore holds no body of",
            ));
        };
        let s = self.modeller.union(x, y).map_err(|e| refused(OP, e))?;
        accepted(OP, (s.body().clone(), s.exactness()), BY_UNION).map(|made| made.keeping(s))
    }

    fn subtract(
        &mut self,
        target: &EngineSolid,
        tool: &EngineSolid,
    ) -> Result<EngineSolid, EngineError> {
        const OP: &str = "subtract";
        let (t, k) = (OperandKey([1; 16]), OperandKey([2; 16]));
        fn operand(key: OperandKey, s: &EngineSolid) -> Operand<'_> {
            Operand {
                key,
                body: &s.body,
                kind: OperandKind::Solid,
                exactness: s.exactness,
            }
        }
        let would = target
            .exactness
            .combine(tool.exactness)
            .ok_or_else(|| unsupported(OP, "operands whose exactness has no common class"))?;
        let input = BooleanInput {
            operands: vec![operand(t, target), operand(k, tool)],
            select: vec![Selection::Subtract { target: t }],
            merge: Merge::RemoveRedundant,
            accept: Some(would),
        };
        let out = combine(&context(), &Exec::serial(), &input).map_err(|e| refused(OP, e))?;
        let exactness = out.exactness;
        let mut lists = out.result;
        let mut bodies = lists.pop().unwrap_or_default();
        if bodies.len() != 1 || !lists.is_empty() {
            return Err(unsupported(OP, "a result that is not one solid"));
        }
        let body = bodies
            .pop()
            .map(|c| c.body)
            .ok_or_else(|| refused(OP, "no body"))?;
        accepted(OP, (body, exactness), BY_BOOL)
    }

    fn subtract_pieces(
        &mut self,
        target: &EngineSolid,
        tool: &EngineSolid,
    ) -> Result<Vec<EngineSolid>, EngineError> {
        const OP: &str = "subtract_pieces";
        let (t, k) = (OperandKey([1; 16]), OperandKey([2; 16]));
        fn operand(key: OperandKey, s: &EngineSolid) -> Operand<'_> {
            Operand {
                key,
                body: &s.body,
                kind: OperandKind::Solid,
                exactness: s.exactness,
            }
        }
        let would = target
            .exactness
            .combine(tool.exactness)
            .ok_or_else(|| unsupported(OP, "operands whose exactness has no common class"))?;
        // Every cell inside the target and outside the tool, each its own body: the pieces.
        let input = BooleanInput {
            operands: vec![operand(t, target), operand(k, tool)],
            select: vec![Selection::Cells {
                test: vec![Term::Inside(t), Term::Inside(k), Term::Not, Term::And],
                seeds: vec![],
            }],
            merge: Merge::RemoveRedundant,
            accept: Some(would),
        };
        let out = combine(&context(), &Exec::serial(), &input).map_err(|e| refused(OP, e))?;
        let exactness = out.exactness;
        let mut lists = out.result;
        let cells = lists.pop().unwrap_or_default();
        if !lists.is_empty() {
            return Err(refused(OP, "more than one list for one selection"));
        }
        let mut pieces = cells
            .into_iter()
            .map(|c| accepted(OP, (c.body, exactness), BY_BOOL))
            .collect::<Result<Vec<_>, _>>()?;
        pieces.sort_by_cached_key(|p| corners_of(&p.body));
        Ok(pieces)
    }

    fn volume(&mut self, solid: &EngineSolid) -> Result<Volume, EngineError> {
        const OP: &str = "volume";
        let out =
            six_volume(&context(), &solid.body, solid.exactness).map_err(|e| refused(OP, e))?;
        Ok(Volume::SixTimes {
            cubic_steps: out.result,
            exactness: out.exactness,
        })
    }

    fn mesh(&mut self, solid: &EngineSolid) -> Result<DisplayMesh, EngineError> {
        const OP: &str = "mesh";
        let mut out = DisplayMesh::default();
        for row in 0..solid.body.arenas().faces.len() {
            let face = FaceIx::new(row).ok_or_else(|| refused(OP, "a face row past the index"))?;
            mesh_face(&solid.body, face, &mut out)?;
        }
        Ok(out)
    }

    fn section(&mut self, solid: &EngineSolid, plane: &GridPlane) -> Result<Cut, EngineError> {
        crate::cut::section(&solid.body, solid.exactness, plane)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use geom_topo::{Builder, ShellIx};

    /// One millimetre in steps of `Q0`.
    const MM: i64 = 6_400;

    /// A box of 400 by 200 by 300 mm on `Q0` made by the engine.
    fn cuboid(e: &mut SolidcoreEngine) -> EngineSolid {
        let p = |x: i64, y: i64| P3::new(x * MM, y * MM, 0).expect("a grid point");
        let profile = Profile {
            outer: vec![p(0, 0), p(400, 0), p(400, 200), p(0, 200)],
            holes: vec![],
        };
        e.extrude(&profile, [0, 0, 300 * MM]).expect("a box")
    }

    /// A result that is not closed is refused, never returned: the engine's box with its first
    /// face taken out of its shell publishes as rows, and the gate every result passes through
    /// refuses it with the kernel's checker's fault, while the box itself passes.
    #[test]
    fn an_unclosed_result_is_refused_not_returned() {
        let mut e = SolidcoreEngine::new(GridStep::Q0);
        let whole = cuboid(&mut e);
        let again = accepted("test", (whole.body.clone(), whole.exactness), BY_PRISM);
        assert_eq!(again.as_ref().map(EngineSolid::body), Ok(&whole.body));
        let mut b = Builder::new();
        b.arenas = whole.body.arenas().clone();
        let shell = ShellIx::new(0).expect("the first shell");
        let first = b
            .arenas
            .shells
            .first_face
            .get(shell)
            .copied()
            .flatten()
            .expect("a first face");
        let second = b.arenas.faces.next.get(first).copied().flatten();
        b.arenas
            .shells
            .first_face
            .set(shell, second)
            .expect("the shell's first face");
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        let open = b
            .publish(BodyKind::Solid, whole.body.outside(), &mut meter)
            .expect("the rows publish");
        let found = accepted("test", (open, whole.exactness), BY_PRISM);
        assert!(
            matches!(
                &found,
                Err(EngineError::Refused { operation: "test", reason })
                    if reason.starts_with("the kernel's checker refused the result")
            ),
            "{found:?}"
        );
    }

    /// A cylinder of radius 500 and height 1000 mm on the origin, made by solidcore and kept as
    /// the engine keeps every solid solidcore makes.
    fn cylinder(e: &mut SolidcoreEngine) -> EngineSolid {
        let c = e
            .modeller
            .cylinder(
                P3::new(0, 0, 0).expect("the origin"),
                Steps(500 * MM.unsigned_abs()),
                Steps(1000 * MM.unsigned_abs()),
            )
            .expect("a cylinder");
        accepted("test", (c.body().clone(), c.exactness()), BY_SWEEP)
            .expect("the cylinder passes the checker")
            .keeping(c)
    }

    /// A curved solid is united by solidcore's boolean union, on the bodies the engine keeps of
    /// what solidcore made: the cylinder with a box solidcore swept, standing on the box's top
    /// face. What it gives today is recorded as it is: solidcore unites them, and the kernel's
    /// checker refuses the result, having no certified test yet of a pair the circle's arcs make
    /// with the box's faces (the checker's gap CK-3), so the union is an error, never a solid.
    #[test]
    fn a_curved_solid_is_united_by_solidcore() {
        let mut e = SolidcoreEngine::new(GridStep::Q0);
        let c = cylinder(&mut e);
        assert!(!c.flat());
        let p = |x: i64, y: i64, z: i64| P3::new(x * MM, y * MM, z * MM).expect("a grid point");
        let side = Profile {
            outer: vec![
                p(-1000, -1000, -400),
                p(1000, -1000, -400),
                p(1000, -1000, 0),
                p(-1000, -1000, 0),
            ],
            holes: vec![],
        };
        let block = e.extrude(&side, [0, 2000 * MM, 0]).expect("a swept box");
        assert_eq!(block.made_by(), BY_SWEEP);
        let found = e.unite(&c, &block);
        assert!(
            matches!(
                &found,
                Err(EngineError::Refused { operation: "unite", reason })
                    if reason.starts_with("union: the result failed its check: geom-check found")
                        && reason.contains("no certified test of this pair exists yet")
            ),
            "{found:?}"
        );
    }

    /// A curved solid united with one solidcore holds no body of (a box the kernel's prism made)
    /// is unsupported: neither the kernel's boolean nor solidcore's can take that pair.
    #[test]
    fn a_curved_solid_with_a_kernel_made_one_is_unsupported() {
        let mut e = SolidcoreEngine::new(GridStep::Q0);
        let c = cylinder(&mut e);
        let b = cuboid(&mut e);
        assert_eq!(b.made_by(), BY_PRISM);
        let found = e.unite(&c, &b);
        assert!(
            matches!(
                &found,
                Err(EngineError::Unsupported {
                    operation: "unite",
                    ..
                })
            ),
            "{found:?}"
        );
    }
}
