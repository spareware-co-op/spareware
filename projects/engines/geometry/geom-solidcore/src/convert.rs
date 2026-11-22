//! solidcore's body as the kernel's: the record solidcore writes of a body (`solidcore-record`'s
//! plain B-rep, written by `solidcore-convert`), turned into `geom-topo`'s rows on the frame's
//! grid, published, and run through `geom-check` before anything is returned.
//!
//! What it does: [`body_of`] reads the record's regions, shells, faces, loops, edges and vertices
//! and writes one `geom-topo` row for each, with every list linked, each edge's fins in one radial
//! ring in the order the loops use them; each vertex is the grid point nearest solidcore's point;
//! each surface and curve is the `geom-surface` or `geom-curve` kind of the same shape, in grid
//! steps, on the parameters `geom-surface` and `geom-curve` fix (angles in half-turns), a B-spline
//! curve whose control points lie on its chord within solidcore's tolerance of the edge being the
//! line it is (`straight`: how a lofted wall's edges come). A seam,
//! where solidcore cuts a face that runs the whole way round a cylinder, cone or sphere, is left
//! out as the kernel's own bodies have none, and a circle's or ellipse's vertex with it when
//! nothing else meets there (`form`): no point, curve or surface moves. An edge or vertex that
//! solids touch along, which the record writes once for each solid, is one row, by the identity
//! the record's order gives each copy, never by where the copies lie (decision 32.76). The body
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
use std::collections::BTreeMap;

use geom_check::{Level, check};
use geom_curve::{Axes, Circle, CurveKind, Ellipse, Frame, Line};
use geom_math::Box3;
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Plane, Sphere, SurfaceKind};
use geom_topo::{
    Body, BodyKind, Builder, Edge, Face, Fin, Ix, Loop, PointKind, Region, RegionKind, Sense,
    Shell, Vertex,
};
use solidcore_convert::brep::Order;
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

/// How many steps of `step` make a metre: 4e9 quarter-nanometres over the step's count of them,
/// 6.4e6, 1.6e8 or 4e9, each a whole number and exact in an f64.
#[must_use]
pub fn per_metre(step: GridStep) -> f64 {
    4e9 / f64::from(step.quarter_nm())
}

/// The record's geometry in steps, `.0` steps to the metre. A length is multiplied by that exact
/// whole number and so rounded once. Dividing by [`metres`] instead rounds twice, the step itself
/// not being a binary fraction: on Q0 a metre then came to 6399999.999999999 steps, and every
/// cone and cylinder failed the check there for it.
struct Scale(f64);

impl Scale {
    fn len(&self, x: f64) -> f64 {
        x * self.0
    }

    fn p(&self, p: [f64; 3]) -> [f64; 3] {
        p.map(|x| x * self.0)
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
        let pick = |axis: usize| {
            if (k >> axis) & 1 == 0 {
                lo[axis]
            } else {
                hi[axis]
            }
        };
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
        r::Curve::Ellipse { frame: f, radii } => {
            let (c, a, b) = (s.p(f.origin), s.len(radii[0]), s.len(radii[1]));
            // The record's t of a point, its eccentric angle about the frame, in half-turns:
            // the kernel's ellipse takes its parameter, not the polar angle (chapter 7).
            let turn = |p: [f64; 3]| {
                let q = less(p, c);
                (dot(q, f.y_axis) / b).atan2(dot(q, f.x_axis) / a) / PI
            };
            let t0 = turn(from);
            let mut t1 = if closed { t0 + 2.0 } else { turn(to) };
            while t1 <= t0 {
                t1 += 2.0;
            }
            Ok((ellipse(c, frame(f)?, a, b, (t0, t1))?, reach(c, a.max(b))))
        }
        r::Curve::Polyline { .. } => Err(no("a polyline")),
        r::Curve::Spline { .. } | r::Curve::RationalSpline { .. } => Err(no("a spline curve")),
        r::Curve::SurfaceIntersection { .. } => Err(no("a surface intersection curve")),
    }
}

/// The line `k` lies on, as the record's line from its first control point to its last, when `k`
/// is a B-spline curve (not rational) whose control points all lie within `tolerance` (metres,
/// solidcore's own tolerance of the edge) of that chord and run along it without turning back:
/// a B-spline lies in the hull of its control points and turns back no more often than they do,
/// so such a curve is that segment, within the tolerance the body's bound already holds. A
/// lofted wall's edges come so: solidcore's skin writes each edge across its sections as a
/// B-spline of degree 1 through the sections' corners. Anything else is `None`, and stays the
/// spline curve the kernel has no kind for.
fn straight(k: &r::Curve, tolerance: f64) -> Option<r::Curve> {
    let r::Curve::Spline { points, .. } = k else {
        return None;
    };
    let (&start, &end) = (points.first()?, points.last()?);
    let d = less(end, start);
    let length = dot(d, d).sqrt();
    // A chord no longer than the tolerance, or not a number, is no line to read it as.
    if length.partial_cmp(&tolerance) != Some(core::cmp::Ordering::Greater) {
        return None;
    }
    let slack = tolerance / length;
    let mut before = -slack;
    for &p in points {
        let q = less(p, start);
        let t = dot(q, d) / (length * length);
        let off = less(q, d.map(|c| c * t));
        if dot(off, off).sqrt() > tolerance || t < before - slack || t > 1.0 + slack {
            return None;
        }
        before = before.max(t);
    }
    Some(r::Curve::Line { start, end })
}

/// The record's closed curve the whole way round, for a closed edge left with no vertex (see
/// `form`): the kernel's kind on its full turn, and the box that holds it (in steps).
fn full(s: &Scale, k: &r::Curve) -> Result<(CurveKind<3>, [[f64; 3]; 2]), Unconverted> {
    let bad = |_| no("a curve its kind refuses");
    match k {
        r::Curve::Circle { frame: f, radius } => {
            let (c, rr) = (s.p(f.origin), s.len(*radius));
            let circle = Circle::full(c, rr, frame(f)?).map_err(bad)?;
            Ok((
                CurveKind::Circle(circle),
                [c.map(|x| x - rr), c.map(|x| x + rr)],
            ))
        }
        r::Curve::Ellipse { frame: f, radii } => {
            let (c, a, b) = (s.p(f.origin), s.len(radii[0]), s.len(radii[1]));
            Ok((ellipse(c, frame(f)?, a, b, (0.0, 2.0))?, reach(c, a.max(b))))
        }
        _ => Err(no("an edge with one end left out")),
    }
}

/// The kernel's ellipse c + cos πt·(a·x̂) + sin πt·(b·ŷ) on `frame` over `domain`: the record's
/// ellipse, its semi-axes along the frame's x and y, on the record's own parameter.
fn ellipse(
    c: [f64; 3],
    frame: Frame<3>,
    a: f64,
    b: f64,
    domain: (f64, f64),
) -> Result<CurveKind<3>, Unconverted> {
    Ellipse::new(c, Axes::Frame { frame, a, b }, domain)
        .map(CurveKind::Ellipse)
        .map_err(|_| no("a curve its kind refuses"))
}

/// The box of the points within `r` of `c` on every axis.
fn reach(c: [f64; 3], r: f64) -> [[f64; 3]; 2] {
    [c.map(|x| x - r), c.map(|x| x + r)]
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
        [
            a[0][0].min(b[0][0]),
            a[0][1].min(b[0][1]),
            a[0][2].min(b[0][2]),
        ],
        [
            a[1][0].max(b[1][0]),
            a[1][1].max(b[1][1]),
            a[1][2].max(b[1][2]),
        ],
    ]
}

/// Whether the record's surface closes round its own axis, so that a face on it may run the
/// whole way round: a cylinder, cone or sphere.
fn wraps(k: &r::Surface) -> bool {
    matches!(
        k,
        r::Surface::Cylinder { .. } | r::Surface::Cone { .. } | r::Surface::Sphere { .. }
    )
}

/// A face's loops in the kernel's form, and which edges and vertices are left out of it.
struct Form<'a> {
    /// Each face's loops, in the record's order and the record's direction.
    loops: Vec<Vec<Vec<&'a r::OrientedEdge>>>,
    /// Each record edge's row in the kernel's body, or none for a seam; the copies of one
    /// solidcore edge share its row.
    edge: Vec<Option<usize>>,
    /// Each record vertex's row in the kernel's body, or none for a seam's end; the copies of one
    /// solidcore vertex share its row.
    vertex: Vec<Option<usize>>,
    /// Whether each record edge is the first copy of its solidcore edge, the one whose curve and
    /// ends the row is written from.
    first_edge: Vec<bool>,
    /// Whether each record vertex is the first copy of its solidcore vertex.
    first_vertex: Vec<bool>,
}

/// Each index's first index with the same entity in `of`, which must be `n` long: solidcore's
/// record writes an edge or vertex that solids touch along once for each solid, and its order
/// names the one entity of solidcore's body each copy is.
fn firsts<T: Ord + Copy>(of: &[T], n: usize) -> Result<Vec<usize>, Unconverted> {
    if of.len() != n {
        return Err(no("an order that is not the record's"));
    }
    let mut seen = BTreeMap::new();
    Ok(of
        .iter()
        .enumerate()
        .map(|(i, &t)| *seen.entry(t).or_insert(i))
        .collect())
}

/// The kernel's row of each record entity: a new row, in record order, for each first copy that
/// `keep` keeps, and the first copy's row for every later one. A later copy of an entity whose
/// first copy is left out, or one left out whose first copy is kept, is refused: the copies of one
/// entity are kept or left out together, never told apart.
fn rows(first: &[usize], keep: impl Fn(usize) -> bool) -> Result<Vec<Option<usize>>, Unconverted> {
    let mut out: Vec<Option<usize>> = Vec::with_capacity(first.len());
    let mut kept = 0;
    for (i, &f) in first.iter().enumerate() {
        let row = if f == i {
            keep(i).then(|| {
                kept += 1;
                kept - 1
            })
        } else {
            let at = *out
                .get(f)
                .ok_or_else(|| no("an order naming a later copy first"))?;
            if at.is_some() != keep(i) {
                return Err(no("copies of one entity, one kept and one left out"));
            }
            at
        };
        out.push(row);
    }
    Ok(out)
}

/// `l` with every seam taken out: at each seam the loop parts in two, the run between its two
/// uses and the run round the rest, and a part with no edge left is no loop.
fn split<'a>(l: Vec<&'a r::OrientedEdge>, seam: &[bool]) -> Vec<Vec<&'a r::OrientedEdge>> {
    let mut out = Vec::new();
    let mut todo = vec![l];
    while let Some(l) = todo.pop() {
        let at = l
            .iter()
            .position(|oe| seam.get(oe.edge as usize).copied().unwrap_or(false));
        match at {
            None if l.is_empty() => {}
            None => out.push(l),
            Some(i) => {
                let e = l[i].edge;
                // A seam is used twice in one loop (see `form`), so its last use is after `i`.
                let j = l.iter().rposition(|oe| oe.edge == e).unwrap_or(i);
                let inner = l[i + 1..j].to_vec();
                let outer: Vec<_> = l[j + 1..].iter().chain(&l[..i]).copied().collect();
                todo.push(inner);
                todo.push(outer);
            }
        }
    }
    out
}

/// The record's faces, edges and vertices as the kernel writes the same body.
///
/// solidcore cuts a face that runs the whole way round a cylinder, cone or sphere along a seam:
/// an edge the one face uses twice, once each way, in one loop, and used by nothing else, with a
/// vertex at each end on the circles (or ellipses) it joins. The kernel's own bodies have no seam (`geom-make`'s
/// cylinder is two closed circles with no vertex, its side one loop on each; its sphere one face
/// with no edge). So a seam is left out, its loop parted where it ran, and each of its two
/// vertices with it when nothing but one closed circle or ellipse is left at it; that curve is then
/// the full turn with no vertex. No point, curve or surface is moved: the same faces, on the same
/// surfaces, bounded by the same curves.
///
/// Where solids touch along an edge or at a vertex, solidcore's body has one edge or vertex that
/// both use, and its record writes a copy of it for each solid, naming in `order` the one entity
/// each copy is. The kernel's body keeps the one entity, shared, by that identity: the copies are
/// one row, never joined because they lie at the same place.
fn form<'a>(rec: &'a r::Brep, order: &Order) -> Result<Form<'a>, Unconverted> {
    let edge_first = firsts(&order.edges, rec.edges.len())?;
    let vertex_first = firsts(&order.vertices, rec.vertices.len())?;
    let mut uses = vec![0_u32; rec.edges.len()];
    let mut record_loops = Vec::with_capacity(rec.faces.len());
    for face in &rec.faces {
        let mut loops = Vec::with_capacity(face.loops.len());
        for l in &face.loops {
            let r::Loop::Edges { edges } = l else {
                return Err(no("a loop of one vertex"));
            };
            if edges.is_empty() {
                return Err(no("a loop of no edges"));
            }
            for oe in edges {
                *uses
                    .get_mut(row(oe.edge)?)
                    .ok_or_else(|| no("a loop naming an edge the record lacks"))? += 1;
            }
            loops.push(edges.iter().collect::<Vec<_>>());
        }
        record_loops.push(loops);
    }
    let mut seam = vec![false; rec.edges.len()];
    for (face, loops) in rec.faces.iter().zip(&record_loops) {
        if !wraps(&face.surface) {
            continue;
        }
        for l in loops {
            for oe in l {
                let e = row(oe.edge)?;
                let twin = l.iter().filter(|x| x.edge == oe.edge).collect::<Vec<_>>();
                if uses[e] == 2 && twin.len() == 2 && twin[0].forward != twin[1].forward {
                    seam[e] = true;
                }
            }
        }
    }
    let loops: Vec<_> = record_loops
        .into_iter()
        .map(|ls| ls.into_iter().flat_map(|l| split(l, &seam)).collect())
        .collect();
    // A seam's end goes when what is left at it is one closed circle or ellipse.
    let mut at_vertex: Vec<Vec<usize>> = vec![Vec::new(); rec.vertices.len()];
    let mut seam_end = vec![false; rec.vertices.len()];
    for (e, edge) in rec.edges.iter().enumerate() {
        for v in edge.vertices {
            let v = row(v)?;
            if v >= rec.vertices.len() {
                return Err(no("an edge naming a vertex the record lacks"));
            }
            if seam[e] {
                seam_end[v] = true;
            } else if !at_vertex[v].contains(&e) {
                at_vertex[v].push(e);
            }
        }
    }
    let gone = |v: usize| {
        seam_end[v]
            && match at_vertex[v].as_slice() {
                [] => true,
                [e] => {
                    let edge = &rec.edges[*e];
                    edge.vertices[0] == edge.vertices[1]
                        && matches!(
                            edge.curve,
                            r::Curve::Circle { .. } | r::Curve::Ellipse { .. }
                        )
                }
                _ => false,
            }
    };
    // A later copy of an edge runs between the same solidcore vertices, the same way, as its
    // first copy, whose curve and ends its row is written from; one that does not is refused.
    let ends = |e: usize| rec.edges[e].vertices.map(|v| vertex_first[v as usize]);
    if (0..rec.edges.len()).any(|e| ends(e) != ends(edge_first[e])) {
        return Err(no("copies of one edge between different vertices"));
    }
    let vertex = rows(&vertex_first, |v| !gone(v))?;
    let edge = rows(&edge_first, |e| !seam[e])?;
    let first = |of: &[usize]| of.iter().enumerate().map(|(i, &f)| f == i).collect();
    Ok(Form {
        loops,
        edge,
        vertex,
        first_edge: first(&edge_first),
        first_vertex: first(&vertex_first),
    })
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

/// One fin of the kernel's body, as the test of its kind reads it.
#[derive(Clone, Copy, Debug)]
struct Use {
    /// The shell of its face.
    shell: usize,
    /// Its edge's row.
    edge: usize,
    /// Whether it runs along its edge, from the edge's start to its end.
    along: bool,
    /// The row of the fin after it in its loop.
    next: usize,
}

fn root(of: &mut [usize], mut i: usize) -> usize {
    while of[i] != i {
        of[i] = of[of[i]];
        i = of[i];
    }
    i
}

/// Whether the body of these fins is non-manifold, so of the kernel's kind General (chapter 8.1:
/// "mixed and non-manifold"), not Solid: an edge with more than two fins (`ring`, each edge's
/// fins), or a vertex the faces round which do not make one fan. A loop's corner at a vertex is
/// where one fin ends and the next begins; two corners at a vertex are of one fan when a chain of
/// edges at that vertex, each used by both of two corners in the chain, joins them. A vertex with
/// corners in two shells, or in two fans of one shell, is one where solids or parts of one touch.
/// `ends` is each edge's start and end vertex, none for a closed edge without one.
fn non_manifold(uses: &[Use], ends: &[[Option<usize>; 2]], ring: &[Vec<usize>]) -> bool {
    if ring.iter().any(|fins| fins.len() > 2) {
        return true;
    }
    let mut of: Vec<usize> = (0..uses.len()).collect();
    let mut by_edge: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    let mut corners = Vec::new();
    for (i, u) in uses.iter().enumerate() {
        let Some(v) = ends[u.edge][usize::from(u.along)] else {
            continue;
        };
        corners.push((v, i));
        for e in [u.edge, uses[u.next].edge] {
            let first = *by_edge.entry((v, e)).or_insert(i);
            let (a, b) = (root(&mut of, first), root(&mut of, i));
            of[a] = b;
        }
    }
    let mut first: BTreeMap<usize, usize> = BTreeMap::new();
    for (v, i) in corners {
        let f = *first.entry(v).or_insert(i);
        if root(&mut of, f) != root(&mut of, i) || uses[f].shell != uses[i].shell {
            return true;
        }
    }
    false
}

/// The record `rec` as the kernel's solid on `step`, unchecked, and its record of exactness.
/// `order` is the order `solidcore-convert` wrote the record with: the copies it names as one
/// solidcore edge or vertex are one row of the kernel's body.
///
/// # Errors
///
/// [`Unconverted`], naming what the record holds that has no row or kind here.
#[allow(
    clippy::too_many_lines,
    reason = "one pass over the record writes every arena's rows; split, each piece would need the others' counts"
)]
pub fn body_of(
    rec: &r::Brep,
    order: &Order,
    step: GridStep,
) -> Result<(Body, Exactness), Unconverted> {
    let s = Scale(per_metre(step));
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
        let first_face = faces
            .first()
            .map(|f| row(f.face).and_then(ix))
            .transpose()?;
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

    let form = form(rec, order)?;

    // Edges first: their curves and boxes, by the kernel's row.
    let mut curves = Vec::new();
    let mut edge_boxes = Vec::new();
    let mut edges = Vec::new();
    for (k, e) in rec.edges.iter().enumerate() {
        if form.edge[k].is_none() || !form.first_edge[k] {
            continue;
        }
        let end = |v: u32| {
            rec.vertices
                .get(row(v)?)
                .map(|x| x.point)
                .ok_or_else(|| no("an edge naming a vertex the record lacks"))
        };
        let [a, b] = e.vertices;
        let ends = [row(a)?, row(b)?].map(|v| form.vertex[v]);
        let (c, bx) = match ends {
            // A closed edge left with no vertex (see `form`): the full turn.
            [None, None] => full(&s, &e.curve)?,
            [Some(_), Some(_)] => {
                let line = straight(&e.curve, e.tolerance.max(rec.tolerance));
                let k = line.as_ref().unwrap_or(&e.curve);
                curve(&s, k, end(a)?, end(b)?, a == b)?
            }
            _ => return Err(no("an edge with one end left out")),
        };
        curves.push(c);
        edge_boxes.push(bx);
        edges.push(ends);
    }
    let mut points = Vec::new();
    for (k, v) in rec.vertices.iter().enumerate() {
        if form.vertex[k].is_some() && form.first_vertex[k] {
            points.push(PointKind::Grid(s.grid(v.point)?));
        }
    }

    // Faces, loops and fins, each edge's fins kept for its radial ring.
    let mut faces = Vec::new();
    let mut surfaces = Vec::new();
    let mut loops = Vec::new();
    let mut fins: Vec<Fin> = Vec::new();
    let mut uses: Vec<Use> = Vec::new();
    let mut ring: Vec<Vec<usize>> = vec![Vec::new(); edges.len()];
    for (f, (face, face_loops)) in rec.faces.iter().zip(&form.loops).enumerate() {
        let (shell, forward) = face_use[f].ok_or_else(|| no("a face in no shell"))?;
        let mut bx: Option<[[f64; 3]; 2]> = match &face.surface {
            r::Surface::Sphere { frame: fr, radius } => {
                let (c, rr) = (s.p(fr.origin), s.len(*radius));
                Some([c.map(|x| x - rr), c.map(|x| x + rr)])
            }
            _ => None,
        };
        let first_loop = if face_loops.is_empty() {
            None
        } else {
            Some(ix(loops.len())?)
        };
        let n_loops = face_loops.len();
        for (li, l) in face_loops.iter().enumerate() {
            let here = loops.len();
            let next = if li + 1 < n_loops {
                Some(ix(here + 1)?)
            } else {
                None
            };
            let first = fins.len();
            let n = l.len();
            loops.push(Loop {
                face: ix(f)?,
                next,
                first_fin: Some(ix(first)?),
            });
            // The record runs a loop round its surface's natural normal; a face the shell uses
            // reversed runs, in the kernel's rows, the other way round its outward normal.
            let ordered: Vec<&r::OrientedEdge> = if forward {
                l.clone()
            } else {
                l.iter().rev().copied().collect()
            };
            for (k, oe) in ordered.into_iter().enumerate() {
                let at = first + k;
                let e = form
                    .edge
                    .get(row(oe.edge)?)
                    .copied()
                    .flatten()
                    .ok_or_else(|| no("a loop naming an edge the record lacks"))?;
                let eb = *edge_boxes
                    .get(e)
                    .ok_or_else(|| no("a loop naming an edge the record lacks"))?;
                bx = Some(bx.map_or(eb, |b| join(b, eb)));
                ring[e].push(at);
                uses.push(Use {
                    shell,
                    edge: e,
                    along: oe.forward == forward,
                    next: first + (k + 1) % n,
                });
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
    let edges_ends = edges.clone();
    let edges: Vec<Edge> = edges
        .iter()
        .enumerate()
        .map(|(e, [a, b])| {
            Ok(Edge {
                first_fin: ring[e].first().map(|&x| ix(x)).transpose()?,
                start: a.map(ix).transpose()?,
                end: b.map(ix).transpose()?,
                curve: ix(e)?,
            })
        })
        .collect::<Result<_, Unconverted>>()?;
    let vertices: Vec<Vertex> = (0..points.len())
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
    // A body where solids touch along an edge or at a vertex is non-manifold: the kernel's kind
    // for it is General (chapter 8.1).
    let kind = if non_manifold(&uses, &edges_ends, &ring) {
        BodyKind::General
    } else {
        BodyKind::Solid
    };
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let body = b
        .publish(kind, ix(outside)?, &mut meter)
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
    use solidcore::boolean::api_brep::boolean_union;
    use solidcore::primitive::api_primitives::create_box;
    use solidcore::{Kernel, KernelOptions, Point3};

    /// The record of solidcore's unit cube at the origin, and its order.
    fn cube() -> (r::Brep, Order) {
        let mut k = Kernel::new(&KernelOptions::default());
        let mut b = create_box(&mut k, Point3::new(0.0, 0.0, 0.0), 1.0, 1.0, 1.0).expect("a box");
        solidcore_convert::brep::to_record(&mut k, &mut b).expect("its record")
    }

    #[test]
    fn a_cube_passes_on_every_step() {
        let (rec, order) = cube();
        for step in [GridStep::Q0, GridStep::Q1, GridStep::Q2] {
            let (body, exactness) = body_of(&rec, &order, step).expect("the cube's rows");
            assert!(checked(body, exactness).is_ok(), "{step:?}");
        }
    }

    /// The record and order of solidcore's union of the unit cube at the origin and the one whose
    /// least corner is `at`, which touch along an edge or at a vertex only.
    fn touching(at: [f64; 3]) -> (r::Brep, Order) {
        let mut k = Kernel::new(&KernelOptions::default());
        let a = create_box(&mut k, Point3::new(0.0, 0.0, 0.0), 1.0, 1.0, 1.0).expect("a box");
        let b = create_box(&mut k, Point3::new(at[0], at[1], at[2]), 1.0, 1.0, 1.0).expect("a box");
        let mut u = boolean_union(&mut k, a, b).expect("the union");
        solidcore_convert::brep::to_record(&mut k, &mut u).expect("its record")
    }

    /// SC-6's one cause was ours (decision 32.76): two cubes that touch along an edge (bool-05) or
    /// at a vertex (bool-06), united, are one body in solidcore with that edge and its vertices, or
    /// that vertex, shared by its two shells. solidcore's record writes what solids touch along once
    /// for each solid, and its order names the one entity each copy is. The kernel's body keeps
    /// that one entity, shared, by the order's identity, never by where the copies lie, and the
    /// checker passes it. Both bodies are non-manifold, the edge of four fins and the vertex two
    /// shells share alike, so both are of the kernel's kind General (chapter 8.1), as is a body
    /// with a vertex where the faces of one shell make two fans.
    #[test]
    fn what_two_solids_touch_along_stays_one() {
        for (at, record, kernel) in [
            ([1.0, 1.0, 0.0], (24, 16), (23, 14)),
            ([1.0, 1.0, 1.0], (24, 16), (24, 15)),
        ] {
            let (rec, order) = touching(at);
            assert_eq!((rec.edges.len(), rec.vertices.len()), record, "{at:?}");
            let (body, exactness) = body_of(&rec, &order, GridStep::Q1).expect("the rows");
            assert_eq!(body.kind(), BodyKind::General, "{at:?}");
            let a = body.arenas();
            assert_eq!(
                (a.shells.len(), a.edges.len(), a.vertices.len()),
                (2, kernel.0, kernel.1),
                "{at:?}"
            );
            let found = checked(body, exactness);
            assert!(found.is_ok(), "{at:?}: {found:?}");
        }
        // A vertex where the faces of one shell make two fans is non-manifold too, as one two
        // shells share is; one fan is not. Four edges leave vertex 0: corners at it join edges 0
        // and 1 in one fan, and 2 and 3 in another.
        let ends = [
            [Some(0), Some(1)],
            [Some(0), Some(2)],
            [Some(0), Some(3)],
            [Some(0), Some(4)],
        ];
        let fan = |at: usize, a: usize, b: usize, shell: usize| {
            [
                Use {
                    shell,
                    edge: a,
                    along: false,
                    next: at + 1,
                },
                Use {
                    shell,
                    edge: b,
                    along: true,
                    next: at,
                },
                Use {
                    shell,
                    edge: b,
                    along: false,
                    next: at + 3,
                },
                Use {
                    shell,
                    edge: a,
                    along: true,
                    next: at + 2,
                },
            ]
        };
        let one: Vec<Use> = fan(0, 0, 1, 0).to_vec();
        let ring = |uses: &[Use]| {
            let mut r = vec![Vec::new(); ends.len()];
            for (i, u) in uses.iter().enumerate() {
                r[u.edge].push(i);
            }
            r
        };
        assert!(!non_manifold(&one, &ends, &ring(&one)));
        let two: Vec<Use> = fan(0, 0, 1, 0).into_iter().chain(fan(4, 2, 3, 0)).collect();
        assert!(non_manifold(&two, &ends, &ring(&two)));
        let shells: Vec<Use> = fan(0, 0, 1, 0).into_iter().chain(fan(4, 2, 3, 1)).collect();
        assert!(non_manifold(&shells, &ends, &ring(&shells)));
    }

    /// isect-28's cone passes on Q0 as on Q1: a metre is exactly 6.4e6 steps of Q0, so its unit
    /// axes and its caps' come over whole (they came over as 6399999.999999999 when the record was
    /// divided by Q0's step in metres, and the check refused the cone on Q0 alone).
    #[test]
    fn a_cone_passes_on_q0_and_q1() {
        assert_eq!(per_metre(GridStep::Q0), 6.4e6);
        let mut k = Kernel::new(&KernelOptions::default());
        let mut c = solidcore::primitive::api_primitives::create_cone(
            &mut k,
            Point3::new(0.0, 0.0, 0.0),
            1.0,
            0.25,
            2.0,
        )
        .expect("a cone");
        let (rec, order) = solidcore_convert::brep::to_record(&mut k, &mut c).expect("its record");
        for step in [GridStep::Q0, GridStep::Q1] {
            let (body, exactness) = body_of(&rec, &order, step).expect("the cone's rows");
            let found = checked(body, exactness);
            assert!(found.is_ok(), "{step:?}: {found:?}");
        }
    }

    /// A vertex moved off its faces by a millimetre, far past solidcore's tolerance, is refused:
    /// the body that fails the check is never returned.
    #[test]
    fn a_body_that_fails_the_check_is_refused() {
        let (mut rec, order) = cube();
        rec.vertices[0].point[2] += 1e-3;
        let (body, exactness) = body_of(&rec, &order, GridStep::Q1).expect("the rows");
        assert!(matches!(
            checked(body, exactness),
            Err(Refusal::Failed { faults, .. }) if faults > 0
        ));
    }

    /// The bound recorded is solidcore's own tolerance in whole steps, and one more.
    #[test]
    fn the_bound_is_solidcores_tolerance() {
        let (rec, _) = cube();
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

    /// The record of solidcore's cylinder of radius 0.5 and height 1 on the origin, and its order.
    fn can() -> (r::Brep, Order) {
        let mut k = Kernel::new(&KernelOptions::default());
        let mut c = solidcore::primitive::api_primitives::create_cylinder(
            &mut k,
            Point3::new(0.0, 0.0, 0.0),
            0.5,
            1.0,
        )
        .expect("a cylinder");
        solidcore_convert::brep::to_record(&mut k, &mut c).expect("its record")
    }

    /// solidcore's cylinder, with its seam and a vertex on each circle, is written as the kernel
    /// writes its own: three faces, two closed circles with no vertex, the side one loop on each.
    #[test]
    fn a_cylinder_has_no_seam() {
        let (rec, order) = can();
        assert_eq!((rec.edges.len(), rec.vertices.len()), (3, 2));
        let (body, _) = body_of(&rec, &order, GridStep::Q1).expect("the rows");
        let a = body.arenas();
        assert_eq!(
            (
                a.faces.len(),
                a.loops.len(),
                a.edges.len(),
                a.vertices.len()
            ),
            (3, 4, 2, 0)
        );
        for k in 0..a.edges.len() {
            let e = a.edges.get(Ix::new(k).expect("a row")).expect("an edge");
            assert!(e.start.is_none() && e.end.is_none());
        }
    }

    /// SC-5's cause at the cylinder of radius 0.5 and height 1 is gone at the pin: solidcore
    /// writes both caps on planes exactly square to the axis, through z = 0 and z = 1 exactly (the
    /// fork's flat face on loops at one height lies on that height), so the checker's test of a
    /// plane and a cylinder meeting in one circle certifies each cap, and the body passes.
    #[test]
    fn a_cylinder_has_its_caps_exact() {
        let (rec, order) = can();
        let mut heights = Vec::new();
        for f in &rec.faces {
            if let r::Surface::Plane {
                origin,
                u_axis,
                v_axis,
            } = &f.surface
            {
                assert_eq!((u_axis[2], v_axis[2]), (0.0, 0.0), "{:?}", f.surface);
                heights.push(origin[2]);
            }
        }
        heights.sort_by(f64::total_cmp);
        assert_eq!(heights, [0.0, 1.0]);
        let (body, exactness) = body_of(&rec, &order, GridStep::Q1).expect("the rows");
        let found = checked(body, exactness);
        assert!(found.is_ok(), "{found:?}");
    }

    /// WR-3: the record's ellipse, origin + radii[0] cos t x + radii[1] sin t y, is the kernel's
    /// ellipse c + cos πt·(a·x̂) + sin πt·(b·ŷ) on the same frame, in steps, its t the record's
    /// eccentric angle in half-turns: an arc runs between its two ends' parameters, a closed edge
    /// one turn from its vertex, and a closed edge with no vertex is the full turn. No point moves:
    /// each end of the arc is the record's own point there.
    #[test]
    fn an_ellipse_is_the_kernels_ellipse() {
        use geom_curve::geom_nurbs::Limit;
        use geom_curve::{Axes, ParamCurve};
        let s = Scale(per_metre(GridStep::Q1));
        let (x, y) = ([0.6, 0.0, 0.8], [0.0, 1.0, 0.0]);
        let f = r::Frame {
            origin: [1.0, 2.0, 3.0],
            x_axis: x,
            y_axis: y,
        };
        let k = r::Curve::Ellipse {
            frame: f.clone(),
            radii: [0.5, 0.25],
        };
        let at = |t: f64| {
            let (c, si) = ((t * PI).cos(), (t * PI).sin());
            core::array::from_fn(|i| f.origin[i] + 0.5 * c * x[i] + 0.25 * si * y[i])
        };
        let near = |p: [f64; 3], q: [f64; 3]| (0..3).all(|i| (p[i] - q[i]).abs() < 1e-6);
        let (c, bx) = curve(&s, &k, at(0.25), at(1.5), false).expect("the ellipse");
        let CurveKind::Ellipse(e) = &c else {
            panic!("not the kernel's ellipse: {c:?}");
        };
        assert_eq!(e.center(), s.p(f.origin));
        let Axes::Frame { frame: fr, a, b } = e.axes() else {
            panic!("not on a frame: {:?}", e.axes());
        };
        assert_eq!((a, b), (s.len(0.5), s.len(0.25)));
        assert_eq!(fr, frame(&f).expect("the frame"));
        let (t0, t1) = c.domain();
        assert!(
            (t0 - 0.25).abs() < 1e-12 && (t1 - 1.5).abs() < 1e-12,
            "{t0} {t1}"
        );
        let ends = [t0, t1].map(|t| c.point(t, Limit::FromRight).expect("a point"));
        assert!(near(ends[0], s.p(at(0.25))) && near(ends[1], s.p(at(1.5))));
        let reach = s.len(0.5);
        assert_eq!(
            bx,
            [
                s.p(f.origin).map(|v| v - reach),
                s.p(f.origin).map(|v| v + reach)
            ]
        );
        // An arc past the half-turn's wrap runs forward through it.
        let (c, _) = curve(&s, &k, at(1.5), at(0.25), false).expect("the arc");
        let (t0, t1) = c.domain();
        assert!(
            (t0 + 0.5).abs() < 1e-12 && (t1 - 0.25).abs() < 1e-12,
            "{t0} {t1}"
        );
        // A closed edge is one turn from its vertex.
        let (c, _) = curve(&s, &k, at(0.25), at(0.25), true).expect("the closed edge");
        let (t0, t1) = c.domain();
        assert!(
            (t0 - 0.25).abs() < 1e-12 && (t1 - t0 - 2.0).abs() < 1e-12,
            "{t0} {t1}"
        );
        // A closed edge left with no vertex is the full turn.
        let (c, _) = full(&s, &k).expect("the full ellipse");
        assert!(matches!(c, CurveKind::Ellipse(_)));
        assert_eq!(c.domain(), (0.0, 2.0));
    }

    #[test]
    fn a_length_of_whole_steps_is_its_nearest_metres() {
        assert_eq!(to_metres(32_000_000, GridStep::Q1), 0.2);
        assert_eq!(to_metres(16, GridStep::Q1), 1e-7);
        assert_eq!(to_metres(-64, GridStep::Q0), -1e-5);
    }

    #[test]
    fn a_shape_with_no_kind_here_is_refused_by_name() {
        let (mut rec, order) = cube();
        rec.edges[0].curve = r::Curve::Polyline {
            points: vec![[0.0; 3], [1.0, 0.0, 0.0]],
        };
        assert_eq!(
            body_of(&rec, &order, GridStep::Q1).map(|_| ()),
            Err(Unconverted("a polyline".into()))
        );
    }

    /// A B-spline edge whose control points run along their chord is the line it is; one that
    /// turns back along it (a fold), or bends off it by more than the tolerance, is not, and stays
    /// the spline curve the kernel has no kind for.
    #[test]
    fn a_straight_spline_is_its_line_and_a_folded_or_bent_one_is_not() {
        let spline = |points: Vec<[f64; 3]>| r::Curve::Spline {
            knots: vec![0.0, 0.0, 0.5, 1.0, 1.0],
            points,
        };
        let tol = 1e-5;
        assert_eq!(
            straight(
                &spline(vec![[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]),
                tol
            ),
            Some(r::Curve::Line {
                start: [0.0; 3],
                end: [2.0, 0.0, 0.0]
            })
        );
        let near = [1.0, 0.5e-5, 0.0];
        assert!(straight(&spline(vec![[0.0; 3], near, [2.0, 0.0, 0.0]]), tol).is_some());
        let folded = spline(vec![[0.0; 3], [2.0, 0.0, 0.0], [1.0, 0.0, 0.0]]);
        assert_eq!(straight(&folded, tol), None);
        let bent = spline(vec![[0.0; 3], [1.0, 2e-5, 0.0], [2.0, 0.0, 0.0]]);
        assert_eq!(straight(&bent, tol), None);
    }
}
