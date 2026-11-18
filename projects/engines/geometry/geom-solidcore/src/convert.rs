//! solidcore's body as the kernel's: the record solidcore writes of a body (`solidcore-record`'s
//! plain B-rep, written by `solidcore-convert`), turned into `geom-topo`'s rows on the frame's
//! grid, published, and run through `geom-check` before anything is returned.
//!
//! What it does: [`body_of`] reads the record's regions, shells, faces, loops, edges and vertices
//! and writes one `geom-topo` row for each, with every list linked, each edge's fins in one radial
//! ring in the order the loops use them; each vertex is the grid point nearest solidcore's point;
//! each surface and curve is the `geom-surface` or `geom-curve` kind of the same shape, in grid
//! steps, on the parameters `geom-surface` and `geom-curve` fix (angles in half-turns). The body
//! is recorded Certified, with a bound of solidcore's own largest tolerance in whole steps, rounded
//! up, and one step more for the rounding of its vertices onto the grid. [`checked`] runs every
//! group of the checker on it, in every build, and refuses a body the checker does not pass.
//!
//! Guarantees:
//!
//! - **Checked before returned.** No body leaves this module unless `geom_check::check` at
//!   [`Level::Full`] passed it: a fault, or a carrier the checker cannot certify, is an error. The
//!   level is the same in every build, so a shipped build never returns what a test build refuses
//!   (principle 5, decision 32.48).
//! - **Nothing guessed.** A surface or curve kind with no `geom-surface` or `geom-curve` kind of the
//!   same shape here, a loop of one vertex, a wire, or a point past the grid's range is refused by
//!   name ([`Unconverted`]), never approximated.
//! - **No silent tolerance.** The bound recorded is solidcore's own tolerance, never one chosen to
//!   make the check pass.
//!
//! Method: chapter 8's rows (8.1) and the checker's postcondition (8.3), as `geom-make`'s build
//! writes them (`projects/engines/geometry/architecture/08-topology.md`), from the record's own
//! fields (solidcore at the commit `Cargo.toml` pins, Apache-2.0).
//!
//! Cost: linear in the record's rows, plus the checker's own cost. Recursion: none.

use core::f64::consts::PI;

use geom_check::{Level, check};
use geom_curve::{Circle, CurveKind, Frame, Line};
use geom_math::Box3;
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Plane, Sphere, SurfaceKind};
use geom_topo::{
    Body, BodyKind, Builder, Edge, Face, Fin, Ix, Loop, PointKind, Region, RegionKind, Sense,
    Shell, Vertex,
};
use solidcore_record::brep as r;

/// Why a record could not be written as the kernel's body: what it holds that has no row or kind
/// here, by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unconverted(pub String);

/// What [`checked`] refuses: a record it cannot write, or a body the checker does not pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The record holds what has no row or kind here.
    Unconverted(Unconverted),
    /// The checker did not pass the body: its first fault, and how many it found.
    Failed {
        /// The first fault, as the checker writes it.
        first: String,
        /// How many faults it found.
        faults: u64,
    },
}

fn no(what: &str) -> Unconverted {
    Unconverted(what.into())
}

/// One grid step of `step` in metres, the unit solidcore's records are in: an exact count of
/// quarter-nanometres over 4e9.
#[must_use]
pub fn metres(step: GridStep) -> f64 {
    f64::from(step.quarter_nm()) / 4e9
}

/// `steps` grid steps of `step` in metres: the product is exact (both factors are whole and it
/// stays below 2^53) and the one division rounds once, so a length that is a whole number of
/// steps gives the f64 nearest its decimal value in metres.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "a coordinate is at most 2^40 - 1 steps and a step at most 625 quarter-nanometres, so the product is below 2^53 and exact"
)]
pub fn to_metres(steps: i64, step: GridStep) -> f64 {
    (steps * i64::from(step.quarter_nm())) as f64 / 4e9
}

/// The record's geometry in steps of `s` metres.
struct Scale(f64);

impl Scale {
    fn len(&self, x: f64) -> f64 {
        x / self.0
    }

    fn p(&self, p: [f64; 3]) -> [f64; 3] {
        p.map(|x| x / self.0)
    }

    #[allow(
        clippy::cast_possible_truncation,
        reason = "the value is rounded first and checked against the grid's range by P3::new; one past i64 saturates and is refused there"
    )]
    fn grid(&self, p: [f64; 3]) -> Result<P3, Unconverted> {
        let [x, y, z] = self.p(p).map(|c| c.round() as i64);
        P3::new(x, y, z).map_err(|_| no("a vertex past the grid's range"))
    }
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn frame(f: &r::Frame) -> Result<Frame<3>, Unconverted> {
    Frame::new(f.x_axis, cross(f.x_axis, f.y_axis)).map_err(|_| no("a frame that is not one"))
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn less(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// The eight corners of the box `[lo, hi]`.
fn corners([lo, hi]: [[f64; 3]; 2]) -> [[f64; 3]; 8] {
    core::array::from_fn(|k| {
        let pick = |axis: usize| if (k >> axis) & 1 == 0 { lo[axis] } else { hi[axis] };
        [pick(0), pick(1), pick(2)]
    })
}

/// The least and the most of `f` over the corners of `bx`, each a step further out.
fn span(bx: [[f64; 3]; 2], f: impl Fn([f64; 3]) -> f64) -> (f64, f64) {
    corners(bx)
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &c| {
            let x = f(c);
            (lo.min(x - 1.0), hi.max(x + 1.0))
        })
}

/// The record's surface as the kernel's kind, on a domain that holds the face whose box (in
/// steps) is `bx`. The record's own (u, v) are its NURBS form's, not the analytic kinds', so the
/// domain is found from the face's place instead: the whole turn and the face's reach along the
/// axis for a cylinder or cone, the whole sphere, and the face's box in the plane's coordinates.
fn surface(s: &Scale, k: &r::Surface, bx: [[f64; 3]; 2]) -> Result<SurfaceKind, Unconverted> {
    let bad = |_| no("a surface its kind refuses");
    let plane = |o: [f64; 3], a: [f64; 3], b: [f64; 3]| {
        let (o, a, b) = (s.p(o), s.p(a), s.p(b));
        // The plane's own coordinates of a point: the dual basis of (a, b).
        let (aa, ab, bb) = (dot(a, a), dot(a, b), dot(b, b));
        let det = aa * bb - ab * ab;
        let u = |p: [f64; 3]| (bb * dot(less(p, o), a) - ab * dot(less(p, o), b)) / det;
        let v = |p: [f64; 3]| (aa * dot(less(p, o), b) - ab * dot(less(p, o), a)) / det;
        Plane::new(o, a, b, (span(bx, u), span(bx, v)))
            .map(SurfaceKind::Plane)
            .map_err(bad)
    };
    let along = |f: &r::Frame| {
        let (c, n) = (s.p(f.origin), cross(f.x_axis, f.y_axis));
        span(bx, move |p| dot(less(p, c), n))
    };
    match k {
        r::Surface::Plane {
            origin,
            u_axis,
            v_axis,
        } => plane(*origin, *u_axis, *v_axis),
        r::Surface::PlaneThrough {
            origin,
            u_point,
            v_point,
        } => plane(*origin, less(*u_point, *origin), less(*v_point, *origin)),
        r::Surface::Cylinder { frame: f, radius } => Cylinder::new(
            s.p(f.origin),
            s.len(*radius),
            frame(f)?,
            ((0.0, 2.0), along(f)),
        )
        .map(SurfaceKind::Cylinder)
        .map_err(bad),
        r::Surface::Cone {
            frame: f,
            radius_at_origin,
            half_angle,
        } => Cone::new(
            s.p(f.origin),
            s.len(*radius_at_origin),
            half_angle.tan(),
            frame(f)?,
            ((0.0, 2.0), along(f)),
        )
        .map(SurfaceKind::Cone)
        .map_err(bad),
        r::Surface::Sphere { frame: f, radius } => Sphere::new(
            s.p(f.origin),
            s.len(*radius),
            frame(f)?,
            ((0.0, 2.0), (-0.5, 0.5)),
        )
        .map(SurfaceKind::Sphere)
        .map_err(bad),
        r::Surface::Torus { .. } => Err(no("a torus")),
        r::Surface::BallPatch { .. } => Err(no("a ball patch")),
        r::Surface::Spline { .. } | r::Surface::RationalSpline { .. } => {
            Err(no("a spline surface"))
        }
        r::Surface::PlacedRevolution { .. } => Err(no("a surface of revolution")),
        r::Surface::Blend { .. } => Err(no("a blend")),
    }
}

/// The record's curve from the vertex at `from` to the one at `to` (in metres; the same vertex
/// for a closed edge), as the kernel's kind on the parameters of those ends, and the box that
/// holds it (in steps, not yet widened by the bound). The record's own interval is its NURBS
/// form's, not the analytic kinds', so the ends are found from the vertices instead.
fn curve(
    s: &Scale,
    k: &r::Curve,
    from: [f64; 3],
    to: [f64; 3],
    closed: bool,
) -> Result<(CurveKind<3>, [[f64; 3]; 2]), Unconverted> {
    let bad = |_| no("a curve its kind refuses");
    let (from, to) = (s.p(from), s.p(to));
    match k {
        r::Curve::Line { start, end } => {
            let o = s.p(*start);
            let d = less(s.p(*end), o);
            let t = |p: [f64; 3]| dot(less(p, o), d) / dot(d, d);
            let (t0, t1) = (t(from), t(to));
            if t0.partial_cmp(&t1) != Some(core::cmp::Ordering::Less) {
                return Err(no("an edge running against its line"));
            }
            let line = Line::new(o, d, (t0, t1)).map_err(bad)?;
            let at = |t: f64| [o[0] + t * d[0], o[1] + t * d[1], o[2] + t * d[2]];
            let (a, b) = (at(t0), at(t1));
            let lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
            let hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
            Ok((CurveKind::Line(line), [lo, hi]))
        }
        r::Curve::Circle { frame: f, radius } => {
            let (c, rr) = (s.p(f.origin), s.len(*radius));
            // The angle of a point about the frame, in half-turns.
            let turn = |p: [f64; 3]| {
                let q = less(p, c);
                dot(q, f.y_axis).atan2(dot(q, f.x_axis)) / PI
            };
            let a0 = turn(from);
            let mut a1 = if closed { a0 + 2.0 } else { turn(to) };
            while a1 <= a0 {
                a1 += 2.0;
            }
            let circle = Circle::new(c, rr, frame(f)?, (a0, a1)).map_err(bad)?;
            Ok((
                CurveKind::Circle(circle),
                [c.map(|x| x - rr), c.map(|x| x + rr)],
            ))
        }
        r::Curve::Ellipse { .. } => Err(no("an ellipse")),
        r::Curve::Polyline { .. } => Err(no("a polyline")),
        r::Curve::Spline { .. } | r::Curve::RationalSpline { .. } => Err(no("a spline curve")),
        r::Curve::SurfaceIntersection { .. } => Err(no("a surface intersection curve")),
    }
}

fn ix<K>(row: usize) -> Result<Ix<K>, Unconverted> {
    u32::try_from(row)
        .ok()
        .and_then(Ix::new)
        .ok_or_else(|| no("more rows than an index holds"))
}

fn row(i: u32) -> Result<usize, Unconverted> {
    usize::try_from(i).map_err(|_| no("an index past this machine's"))
}

fn sense(forward: bool) -> Sense {
    if forward {
        Sense::Forward
    } else {
        Sense::Reversed
    }
}

/// The box from `lo` to `hi` widened by `by` steps, on the grid.
#[allow(
    clippy::cast_possible_truncation,
    reason = "floor and ceil first; P3::new refuses what is past the grid's range"
)]
fn widened([lo, hi]: [[f64; 3]; 2], by: f64) -> Result<Box3, Unconverted> {
    let far = || no("a face past the grid's range");
    let [a, b, c] = lo.map(|x| (x - by).floor() as i64);
    let [d, e, f] = hi.map(|x| (x + by).ceil() as i64);
    let (min, max) = (P3::new(a, b, c), P3::new(d, e, f));
    Box3::new(min.map_err(|_| far())?, max.map_err(|_| far())?).map_err(|_| far())
}

fn join(a: [[f64; 3]; 2], b: [[f64; 3]; 2]) -> [[f64; 3]; 2] {
    [
        [a[0][0].min(b[0][0]), a[0][1].min(b[0][1]), a[0][2].min(b[0][2])],
        [a[1][0].max(b[1][0]), a[1][1].max(b[1][1]), a[1][2].max(b[1][2])],
    ]
}

/// The certified record of the body of `rec` on `step`: solidcore's largest tolerance in whole
/// steps, rounded up, and one step for the rounding of each vertex onto the grid (at most half a
/// step on each axis, so less than one step away).
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the count is a ceiling of a positive, finite tolerance over the step, checked below u64's range first"
)]
fn exactness(rec: &r::Brep, step: GridStep) -> Result<Exactness, Unconverted> {
    let worst = rec
        .edges
        .iter()
        .map(|e| e.tolerance)
        .chain(rec.vertices.iter().map(|v| v.tolerance))
        .fold(rec.tolerance, f64::max);
    let steps = (worst / metres(step)).ceil();
    if !(steps.is_finite() && (0.0..9.0e15).contains(&steps)) {
        return Err(no("a tolerance that is not a length"));
    }
    Ok(Exactness::Certified {
        bound: Steps(steps as u64 + 1),
        step,
    })
}

/// The empty solid: the void outside, and nothing else.
pub fn empty() -> Result<Body, Unconverted> {
    let mut b = Builder::new();
    let a = &mut b.arenas;
    let broke = |_| no("the empty body's one row");
    a.regions
        .extend(&[Region {
            kind: RegionKind::Void,
            first_shell: None,
        }])
        .map_err(broke)?;
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    b.publish(BodyKind::Solid, ix(0)?, &mut meter)
        .map_err(|_| no("the empty body"))
}

/// The record `rec` as the kernel's solid on `step`, unchecked, and its record of exactness.
///
/// # Errors
///
/// [`Unconverted`], naming what the record holds that has no row or kind here.
#[allow(
    clippy::too_many_lines,
    reason = "one pass over the record writes every arena's rows; split, each piece would need the others' counts"
)]
pub fn body_of(rec: &r::Brep, step: GridStep) -> Result<(Body, Exactness), Unconverted> {
    let s = Scale(metres(step));
    let exactness = exactness(rec, step)?;
    let Exactness::Certified { bound, .. } = exactness else {
        return Err(no("an exactness that is not certified"));
    };
    #[allow(
        clippy::cast_precision_loss,
        reason = "a bound is far below 2^53 steps (checked in exactness)"
    )]
    let by = bound.0 as f64;

    // Regions, in the record's order. The record's first region is the infinite one, empty
    // space though not marked a void; each closed shell is listed by both regions it separates,
    // and here belongs to the material one, as the kernel's solids keep it, its faces' normals
    // pointing out of that material (the record's rule for a closed shell).
    let outside = 0;
    let material = |g: usize| g != outside && rec.regions.get(g).is_some_and(|x| !x.void);
    if rec.regions.is_empty() {
        return Err(no("a body with no region"));
    }
    let mut regions: Vec<Region> = (0..rec.regions.len())
        .map(|g| Region {
            kind: if material(g) {
                RegionKind::Solid
            } else {
                RegionKind::Void
            },
            first_shell: None,
        })
        .collect();
    let mut shell_region = vec![None; rec.shells.len()];
    let mut owned: Vec<Vec<usize>> = vec![Vec::new(); rec.regions.len()];
    for (g, region) in rec.regions.iter().enumerate() {
        if !material(g) {
            continue;
        }
        for &sh in &region.shells {
            let k = row(sh)?;
            let slot = shell_region
                .get_mut(k)
                .ok_or_else(|| no("a region naming a shell the record lacks"))?;
            if slot.replace(g).is_some() {
                return Err(no("a shell between two material regions"));
            }
            owned[g].push(k);
        }
        if let Some(&first) = owned[g].first() {
            regions[g].first_shell = Some(ix(first)?);
        }
    }
    let mut shells = Vec::new();
    // Which shell each face is in, and whether it is used forward.
    let mut face_use: Vec<Option<(usize, bool)>> = vec![None; rec.faces.len()];
    for (k, sh) in rec.shells.iter().enumerate() {
        let r::Shell::Faces { faces, wires } = sh else {
            return Err(no("a wire or point shell"));
        };
        if !wires.is_empty() {
            return Err(no("a wire in a shell"));
        }
        let g = shell_region[k].ok_or_else(|| no("a shell bounding no material"))?;
        let next = owned[g]
            .iter()
            .position(|&x| x == k)
            .and_then(|at| owned[g].get(at + 1))
            .map(|&x| ix(x))
            .transpose()?;
        let first_face = faces.first().map(|f| row(f.face).and_then(ix)).transpose()?;
        shells.push(Shell {
            region: ix(g)?,
            next,
            first_face,
        });
        for f in faces {
            let slot = face_use
                .get_mut(row(f.face)?)
                .ok_or_else(|| no("a shell naming a face the record lacks"))?;
            if slot.replace((k, f.forward)).is_some() {
                return Err(no("a face used twice"));
            }
        }
    }
    let mut next_face_in_shell: Vec<Option<usize>> = vec![None; rec.faces.len()];
    for sh in &rec.shells {
        if let r::Shell::Faces { faces, .. } = sh {
            for w in faces.windows(2) {
                next_face_in_shell[row(w[0].face)?] = Some(row(w[1].face)?);
            }
        }
    }

    // Edges first: their curves and boxes.
    let mut curves = Vec::new();
    let mut edge_boxes = Vec::new();
    for e in &rec.edges {
        let end = |v: u32| {
            rec.vertices
                .get(row(v)?)
                .map(|x| x.point)
                .ok_or_else(|| no("an edge naming a vertex the record lacks"))
        };
        let [a, b] = e.vertices;
        let (c, bx) = curve(&s, &e.curve, end(a)?, end(b)?, a == b)?;
        curves.push(c);
        edge_boxes.push(bx);
    }
    let mut points = Vec::new();
    for v in &rec.vertices {
        points.push(PointKind::Grid(s.grid(v.point)?));
    }

    // Faces, loops and fins, each edge's fins kept for its radial ring.
    let mut faces = Vec::new();
    let mut surfaces = Vec::new();
    let mut loops = Vec::new();
    let mut fins: Vec<Fin> = Vec::new();
    let mut ring: Vec<Vec<usize>> = vec![Vec::new(); rec.edges.len()];
    for (f, face) in rec.faces.iter().enumerate() {
        let (shell, forward) = face_use[f].ok_or_else(|| no("a face in no shell"))?;
        let mut bx: Option<[[f64; 3]; 2]> = match &face.surface {
            r::Surface::Sphere { frame: fr, radius } => {
                let (c, rr) = (s.p(fr.origin), s.len(*radius));
                Some([c.map(|x| x - rr), c.map(|x| x + rr)])
            }
            _ => None,
        };
        let first_loop = if face.loops.is_empty() {
            None
        } else {
            Some(ix(loops.len())?)
        };
        let n_loops = face.loops.len();
        for (li, l) in face.loops.iter().enumerate() {
            let r::Loop::Edges { edges } = l else {
                return Err(no("a loop of one vertex"));
            };
            let here = loops.len();
            let next = if li + 1 < n_loops {
                Some(ix(here + 1)?)
            } else {
                None
            };
            let first = fins.len();
            let n = edges.len();
            if n == 0 {
                return Err(no("a loop of no edges"));
            }
            loops.push(Loop {
                face: ix(f)?,
                next,
                first_fin: Some(ix(first)?),
            });
            // The record runs a loop round its surface's natural normal; a face the shell uses
            // reversed runs, in the kernel's rows, the other way round its outward normal.
            let ordered: Vec<&r::OrientedEdge> = if forward {
                edges.iter().collect()
            } else {
                edges.iter().rev().collect()
            };
            for (k, oe) in ordered.into_iter().enumerate() {
                let at = first + k;
                let e = row(oe.edge)?;
                let eb = *edge_boxes
                    .get(e)
                    .ok_or_else(|| no("a loop naming an edge the record lacks"))?;
                bx = Some(bx.map_or(eb, |b| join(b, eb)));
                ring[e].push(at);
                fins.push(Fin {
                    loop_: ix(here)?,
                    next: ix(first + (k + 1) % n)?,
                    prev: ix(first + (k + n - 1) % n)?,
                    edge: ix(e)?,
                    sense: sense(oe.forward == forward),
                    radial: ix(at)?,
                    pcurve: None,
                });
            }
        }
        let bx = bx.ok_or_else(|| no("a face with no edge that is not a sphere"))?;
        surfaces.push(surface(&s, &face.surface, bx)?);
        faces.push(Face {
            shell: ix(shell)?,
            next: next_face_in_shell[f].map(ix).transpose()?,
            surface: ix(f)?,
            sense: sense(forward),
            first_loop,
            bound: widened(bx, by)?,
        });
    }
    for members in &ring {
        for (k, &fin) in members.iter().enumerate() {
            fins[fin].radial = ix(members[(k + 1) % members.len()])?;
        }
    }
    let edges: Vec<Edge> = rec
        .edges
        .iter()
        .enumerate()
        .map(|(e, edge)| {
            let [a, b] = edge.vertices;
            Ok(Edge {
                first_fin: ring[e].first().map(|&x| ix(x)).transpose()?,
                start: Some(ix(row(a)?)?),
                end: Some(ix(row(b)?)?),
                curve: ix(e)?,
            })
        })
        .collect::<Result<_, Unconverted>>()?;
    let vertices: Vec<Vertex> = (0..rec.vertices.len())
        .map(|v| Ok(Vertex { point: ix(v)? }))
        .collect::<Result<_, Unconverted>>()?;

    let mut b = Builder::new();
    let a = &mut b.arenas;
    let broke = |_| no("a row the arenas refused");
    a.regions.extend(&regions).map_err(broke)?;
    a.shells.extend(&shells).map_err(broke)?;
    a.faces.extend(&faces).map_err(broke)?;
    a.loops.extend(&loops).map_err(broke)?;
    a.fins.extend(&fins).map_err(broke)?;
    a.edges.extend(&edges).map_err(broke)?;
    a.vertices.extend(&vertices).map_err(broke)?;
    a.surfaces.extend(surfaces).map_err(broke)?;
    a.curves.extend(curves).map_err(broke)?;
    a.points.extend(points).map_err(broke)?;
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let body = b
        .publish(BodyKind::Solid, ix(outside)?, &mut meter)
        .map_err(|e| Unconverted(format!("the rows did not publish: {e:?}")))?;
    Ok((body, exactness))
}

/// `body`, if the checker passes it at [`Level::Full`] with the record `exactness`.
///
/// # Errors
///
/// [`Refusal::Failed`] with the checker's first fault and its count of faults.
pub fn checked(body: Body, exactness: Exactness) -> Result<Body, Refusal> {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let report = check(&body, exactness, Level::Full, &mut meter).map_err(|_| Refusal::Failed {
        first: "the checker stopped on an unlimited meter".into(),
        faults: 0,
    })?;
    if report.passed() {
        return Ok(body);
    }
    let kept = u64::try_from(report.faults().len()).unwrap_or(u64::MAX);
    Err(Refusal::Failed {
        first: report
            .faults()
            .first()
            .map_or_else(|| "faults past the cap only".into(), ToString::to_string),
        faults: kept.saturating_add(report.more()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use solidcore::primitive::api_primitives::create_box;
    use solidcore::{Kernel, KernelOptions, Point3};

    /// The record of solidcore's unit cube at the origin.
    fn cube() -> r::Brep {
        let mut k = Kernel::new(&KernelOptions::default());
        let mut b = create_box(&mut k, Point3::new(0.0, 0.0, 0.0), 1.0, 1.0, 1.0).expect("a box");
        solidcore_convert::brep::to_record(&mut k, &mut b)
            .expect("its record")
            .0
    }

    #[test]
    fn a_cube_passes_on_every_step() {
        for step in [GridStep::Q0, GridStep::Q1, GridStep::Q2] {
            let (body, exactness) = body_of(&cube(), step).expect("the cube's rows");
            assert!(checked(body, exactness).is_ok(), "{step:?}");
        }
    }

    /// A vertex moved off its faces by a millimetre, far past solidcore's tolerance, is refused:
    /// the body that fails the check is never returned.
    #[test]
    fn a_body_that_fails_the_check_is_refused() {
        let mut rec = cube();
        rec.vertices[0].point[2] += 1e-3;
        let (body, exactness) = body_of(&rec, GridStep::Q1).expect("the rows");
        assert!(matches!(
            checked(body, exactness),
            Err(Refusal::Failed { faults, .. }) if faults > 0
        ));
    }

    /// The bound recorded is solidcore's own tolerance in whole steps, and one more.
    #[test]
    fn the_bound_is_solidcores_tolerance() {
        let rec = cube();
        let tol = rec.tolerance;
        let Exactness::Certified { bound, step } = exactness(&rec, GridStep::Q0).expect("a record")
        else {
            panic!("not certified");
        };
        assert_eq!(step, GridStep::Q0);
        #[allow(clippy::cast_precision_loss, reason = "a small count")]
        let b = bound.0 as f64;
        assert!(b - 1.0 >= tol / metres(GridStep::Q0));
        assert!(b - 2.0 < tol / metres(GridStep::Q0));
    }

    #[test]
    fn a_length_of_whole_steps_is_its_nearest_metres() {
        assert_eq!(to_metres(32_000_000, GridStep::Q1), 0.2);
        assert_eq!(to_metres(16, GridStep::Q1), 1e-7);
        assert_eq!(to_metres(-64, GridStep::Q0), -1e-5);
    }

    #[test]
    fn a_shape_with_no_kind_here_is_refused_by_name() {
        let mut rec = cube();
        rec.edges[0].curve = r::Curve::Polyline {
            points: vec![[0.0; 3], [1.0, 0.0, 0.0]],
        };
        assert_eq!(
            body_of(&rec, GridStep::Q1).map(|_| ()),
            Err(Unconverted("a polyline".into()))
        );
    }
}
