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
//!   flat-faced booleans stay the kernel's (32.70, rule 5);
//! - `loft`: solidcore's skin through the sections ([`Modeller::loft`]), for sections whose walls
//!   are flat (each wall's four corners exactly on one plane, decided here on the grid before
//!   solidcore is reached), its body converted and checked as the kernel's and each flat face's
//!   plane written through its own grid corners as the extrusion's is; a wall that is not flat is
//!   unsupported, as the kernel has no kind for a ruled surface yet. solidcore has no check that a
//!   skin does not fold back or cross itself, and the kernel's checker, which every result
//!   passes, refuses such a body: a folded or crossed loft is an error, never a solid;
//! - `unite`: the kernel's exact `geom-bool` (`combine` with the unite selection) when both
//!   solids are flat-faced (32.70, rule 5), and solidcore's boolean union ([`Modeller::union`])
//!   otherwise, on the solidcore bodies the engine keeps of the solids solidcore made; a curved
//!   operand united with one solidcore holds no body of is unsupported;
//! - `volume`: the kernel's exact six-volume of the body's rows (`geom_topo::query_ops::six_volume`);
//! - `mesh`: each flat face's loops filled by solidcore-tess's planar triangulation
//!   (`solidcore_tess::triangulate_planar`), the face's own corners and no point added, so faces
//!   that share an edge share its points bit for bit;
//! - `edition`: the engine's name, solidcore's pinned commit, the grid step, solidcore's
//!   tolerances as its kernel holds them, and the list above.
//!
//! An [`EngineSolid`] is the kernel's checked body, its exactness and what made it
//! ([`EngineSolid::made_by`]), and, privately, solidcore's own body of it when solidcore made it;
//! no solidcore type is in its interface.
//!
//! Not yet: a loft wall that is not flat is not made, a curved face is not meshed (solidcore-tess's planner works on solidcore's record of a
//! body, and a body `geom-bool` made has none), and a subtraction whose result is more than one
//! solid is refused as unsupported.
//!
//! SPDX-License-Identifier: Apache-2.0

use std::fmt::Display;
use std::sync::Arc;

use geom_bool::combine::combine;
use geom_bool::operand::{BooleanInput, Operand, OperandKey, OperandKind, Selection};
use geom_check::{Level, check};
use geom_make::{PrismInput, prism};
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_op::engine::{
    Answer, DisplayMesh, Edition, Engine, EngineError, Profile, Section, Tolerance, Volume,
};
use geom_op::{OpContext, OpId, OpPath};
use geom_poly::merge::Merge;
use geom_run::{Budget, Exec, WorkMeter};
use geom_topo::query::{face_area, loop_area, point};
use geom_topo::query_ops::six_volume;
use geom_topo::{
    Body, BodyKind, Builder, CurveKind, FaceIx, FinIx, FlatSurface, LoopIx, Sense, SurfaceKind,
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

/// What made a solid: solidcore's skin.
pub const BY_LOFT: &str =
    "solidcore's skin of degree 1 through the sections (create_skin_primitive)";

/// The unions the kernel's boolean takes.
pub const FLAT_CASE: &str = "both solids flat-faced";

/// The unions solidcore takes.
pub const CURVED_CASE: &str =
    "a solid with a curved face, united with one solidcore made (it keeps solidcore's body of it)";

/// What made a solid: solidcore's boolean union.
pub const BY_UNION: &str = "solidcore's boolean union";

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

/// `body` as the engine's solid, only if it is a solid the kernel's checker passes at its full
/// level with `exactness`: every result the engine returns comes through here, so a body that is
/// not closed, or fails any other check, is refused and never handed to the program.
fn accepted(
    operation: &'static str,
    (body, exactness): (Body, Exactness),
    made_by: &'static str,
) -> Result<EngineSolid, EngineError> {
    if body.kind() != BodyKind::Solid {
        return Err(refused(operation, "the result is not a solid"));
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
        let s = self.modeller.loft(&given).map_err(|e| match e {
            crate::Error::Invalid { what, .. } => EngineError::Invalid {
                operation: OP,
                what,
            },
            other => refused(OP, other),
        })?;
        accepted(OP, (on_its_corners(OP, s.body())?, s.exactness()), BY_LOFT)
            .map(|made| made.keeping(s))
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
