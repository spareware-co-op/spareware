//! Group 3 of chapter 8.3, self-intersection, for primitive bodies: no face crosses itself, and no
//! two faces cross except along shared edges.
//!
//! What it does: [`check`] runs on a body groups 1 and 2 passed, so every face lies on a plane,
//! cylinder, cone, sphere or ring torus and every edge is a line or a circle on its faces.
//!
//! - **A face's surface is embedded by its kind.** Planes, cylinders, spheres and ring tori never
//!   cross themselves; a cone crosses itself only at its apex, and group 2 has certified every
//!   vertex and circle of each face on the side of the apex where the radius is not negative.
//! - **A face's edges meet only at shared vertices.** Every pair of distinct edges of a face, from
//!   all its loops, is tested. Two segments (a line edge, from vertex to vertex) are tested
//!   exactly: segments through a shared vertex meet again only if they run along each other
//!   (their directions' cross product is 0 and their dot product positive); segments that share
//!   no vertex meet only if they are coplanar (`orient3d` is zero) and their projections meet in
//!   each of the three coordinate planes (some projection is one to one on their plane, and none
//!   misses a meeting point). A segment and a circle, and two circles, are separated by certified
//!   closed forms (`crate::curved`); a pair none of these separates is reported unchecked.
//! - **Two faces meet only along shared edges and at shared vertices.** Every pair of faces is
//!   tested. Two faces exactly on integer planes, or on planes their grid corners fix, are tested
//!   exactly by `crate::planar`, whatever their shape. Two integer planes are compared exactly, in
//!   320-bit integers, and other planes by certified intervals. Parallel planes certainly apart
//!   cannot meet. A plane perpendicular to a cylinder's or cone's axis, or a plane and a sphere,
//!   meet in one circle at most, so faces on them that share a full circle edge meet only along
//!   it. Any other pair is reported unchecked.
//! - **Edges with a vertex where three planes meet** are tested by `crate::planar` too.
//!
//! Guarantees:
//!
//! - **Exact on segments and polygons.** Their tests are integer predicates on the vertices
//!   (`geom_num::predicate::orient3d`, 2D orientations in `i128`, checked, and `crate::planar`'s
//!   signs of planes).
//! - **Certified elsewhere, and fails closed.** A pair no exact or certified test separates is a
//!   [`FaultKind::PairUnchecked`] fault, never a pass (8.3: "certified with bounds, not sampled").
//! - **A segment is its vertices.** A line edge is tested as the segment between its vertices,
//!   which group 2 has certified within the body's bound of the line's ends.
//! - **One outline per flat face.** Every list is walked again where it is needed, but for the
//!   flat faces' outlines, read once and reserved on the meter.
//!
//! Method: chapter 8.3's group 3 (`projects/engines/geometry/architecture/08-topology.md`), on
//! J. R. Shewchuk's exact orientation predicates ("Adaptive precision floating-point arithmetic and
//! fast robust geometric predicates", Discrete & Computational Geometry 18 (1997),
//! <https://doi.org/10.1007/PL00009321>) and the segment tests built on them (J. O'Rourke,
//! *Computational Geometry in C*, 2nd ed., Cambridge University Press 1998,
//! <https://doi.org/10.1017/CBO9780511804120>).
//!
//! Cost: one meter unit per face, per pair of a face's edges and per pair of faces, and
//! `crate::planar`'s; quadratic in a face's edges and in the faces. Recursion: none.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the predicates': p, q, r and s segment ends, v a shared vertex, a to d 2D points"
)]

use geom_curve::{Circle, ParamCurve};
use geom_num::predicate::orient3d;
use geom_num::{Coord, Exactness, P3, Sign};
use geom_run::Stopped;
use geom_topo::entity::{Edge, Face, Vertex};
use geom_topo::ix::Arena;
use geom_topo::mass::Corner;
use geom_topo::{Arenas, Body, Ix};

use crate::combinatorial::{edge_fins, face_fins, fin_edge, fin_face, rows};
use crate::curved::{self, Pair};
use crate::geometric::{Ends, corner, surface};
use crate::planar::{self, Outline};
use crate::report::{Entity, Fault, FaultKind};
use crate::sink::Sink;

/// Group 3 on `body`, recorded `exactness`.
pub(crate) fn check(
    body: &Body,
    exactness: Exactness,
    sink: &mut Sink<'_, '_>,
) -> Result<(), Stopped> {
    let a = body.arenas();
    let exact = matches!(exactness, Exactness::Exact);
    let outlines = planar::outlines(a, exact, sink)?;
    let pieces = |f: Ix<Face>| -> &[Outline] {
        outlines
            .get(f.to_usize())
            .map_or(&[], |slot| slot.0.as_slice())
    };
    for f in rows::<Face>(a.rows(Arena::Face)) {
        sink.charge()?;
        let implicit = pieces(f).first().filter(|o| o.implicit);
        for (i, first) in edges(a, f).enumerate() {
            for second in edges(a, f).skip(i).skip(1) {
                sink.charge()?;
                let found = match implicit {
                    Some(o) => planar::edges_meet(o, first, second, sink)?,
                    None => edges_meet(a, first, second),
                };
                report(sink, found, Entity::Edge(first), Entity::Edge(second));
            }
        }
    }
    for (i, f) in rows::<Face>(a.rows(Arena::Face)).enumerate() {
        for g in rows::<Face>(a.rows(Arena::Face)).skip(i).skip(1) {
            sink.charge()?;
            let found = if pieces(f).is_empty() || pieces(g).is_empty() {
                other_faces_meet(a, f, g, exact)
            } else {
                planar::slots_meet(a, pieces(f), pieces(g), sink)?
            };
            report(sink, found, Entity::Face(f), Entity::Face(g));
        }
    }
    Ok(())
}

/// How two faces meet that are not both outlines, as far as their carriers say.
fn other_faces_meet(a: &Arenas, f: Ix<Face>, g: Ix<Face>, exact: bool) -> Meet {
    let lie = surface(a, f, exact)
        .zip(surface(a, g, exact))
        .map(|(p, q)| p.lies_with(&q));
    match lie {
        Some(Pair::Apart) => Meet::Apart,
        Some(Pair::Circle) if shares_full_circle(a, f, g) => Meet::Apart,
        _ => Meet::Unknown,
    }
}

/// What a test of two edges or two faces found.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Meet {
    /// They meet only where they share an edge or a vertex.
    Apart,
    /// They meet elsewhere.
    Cross,
    /// No test here separates them.
    Unknown,
}

/// Reports what a test of `at` and `with` found: nothing if they are apart.
fn report(sink: &mut Sink<'_, '_>, found: Meet, at: Entity, with: Entity) {
    let kind = match (found, at) {
        (Meet::Apart, _) => return,
        (Meet::Cross, Entity::Edge(_)) => FaultKind::EdgesCross,
        (Meet::Cross, _) => FaultKind::FacesCross,
        (Meet::Unknown, _) => FaultKind::PairUnchecked,
    };
    sink.push(Fault::at(kind, at).with(with));
}

/// The distinct edges of face `f`, in the order its loops first use them.
fn edges(a: &Arenas, f: Ix<Face>) -> impl Iterator<Item = Ix<Edge>> + '_ {
    face_fins(a, f)
        .enumerate()
        .filter(move |&(i, n)| {
            !face_fins(a, f)
                .take(i)
                .any(|m| fin_edge(a, m) == fin_edge(a, n))
        })
        .filter_map(move |(_, n)| fin_edge(a, n))
}

/// A vertex and its point.
#[derive(Clone, Copy)]
struct End {
    vertex: Ix<Vertex>,
    at: P3,
}

/// An edge as group 3 tests it.
enum Shape<'c> {
    /// A line edge: the segment between its two vertices, at distinct points.
    Segment(End, End),
    /// A circle edge, and its vertex if it has one.
    Circle(&'c Circle<3>, Option<Ix<Vertex>>),
}

/// Edge `e` as a shape: `None` for a line edge without two grid vertices at distinct points.
fn shape(a: &Arenas, e: Ix<Edge>) -> Option<Shape<'_>> {
    let start = a.edges.start.get(e).copied().flatten();
    let end = a.edges.end.get(e).copied().flatten();
    let end_at = |v: Ix<Vertex>| match corner(a, v)? {
        Corner::Grid(at) => Some(End { vertex: v, at }),
        _ => None,
    };
    match Ends::of(a.edges.curve.get(e).and_then(|c| a.curves.get(*c)))? {
        Ends::Line(_) => {
            let (s, t) = start.zip(end)?;
            let (s, t) = (end_at(s)?, end_at(t)?);
            (s.at != t.at).then_some(Shape::Segment(s, t))
        }
        Ends::Circle(circle) => Some(Shape::Circle(circle, start)),
    }
}

/// How two distinct edges of one face meet.
fn edges_meet(a: &Arenas, first: Ix<Edge>, second: Ix<Edge>) -> Meet {
    match (shape(a, first), shape(a, second)) {
        (Some(Shape::Segment(p, q)), Some(Shape::Segment(r, s))) => segments_meet([p, q], [r, s]),
        (Some(Shape::Segment(p, q)), Some(Shape::Circle(c, v)))
        | (Some(Shape::Circle(c, v)), Some(Shape::Segment(p, q))) => {
            // The shared end, if any, first.
            let (from, to) = if Some(q.vertex) == v { (q, p) } else { (p, q) };
            apart(curved::segment_apart(
                from.at,
                to.at,
                Some(from.vertex) == v,
                c,
            ))
        }
        (Some(Shape::Circle(c, v)), Some(Shape::Circle(d, w))) => {
            apart(curved::circles_apart(c, d, v.is_some() && v == w))
        }
        _ => Meet::Unknown,
    }
}

/// `Apart` if a certified test separated a pair, `Unknown` if not.
fn apart(separated: bool) -> Meet {
    if separated {
        Meet::Apart
    } else {
        Meet::Unknown
    }
}

/// How two segments of one face meet, exactly.
fn segments_meet(first: [End; 2], second: [End; 2]) -> Meet {
    let [p, q] = first;
    let [r, s] = second;
    let shared = [(p, q, r, s), (p, q, s, r), (q, p, r, s), (q, p, s, r)]
        .into_iter()
        .find(|(v, _, w, _)| v.vertex == w.vertex);
    // Two edges between the same two vertices have their other ends at one point too, so they
    // run along each other: `fold_back` says so.
    match shared {
        Some((v, x, _, y)) => fold_back(v.at, x.at, y.at),
        None => cross_3d([p.at, q.at], [r.at, s.at]),
    }
}

/// Whether segments from `v` to `x` and from `v` to `y` run along each other: `Cross` if their
/// directions are parallel and point the same way. (Both are segments of positive length, so
/// parallel directions have a dot product that is not zero.)
fn fold_back(v: P3, x: P3, y: P3) -> Meet {
    let found = || {
        let (u, w) = (diff(x, v)?, diff(y, v)?);
        let turn = cross(u, w)?;
        let along = dot(u, w)?;
        Some(turn == [0; 3] && along.is_positive())
    };
    found().map_or(Meet::Unknown, cross_if)
}

/// `Cross` if a pair meets, `Apart` if not.
fn cross_if(meets: bool) -> Meet {
    if meets { Meet::Cross } else { Meet::Apart }
}

/// How two segments that share no vertex meet: not at all unless coplanar, and then exactly when
/// their projections meet in all three coordinate planes (module documentation).
fn cross_3d(first: [P3; 2], second: [P3; 2]) -> Meet {
    let [p, q] = first;
    let [r, s] = second;
    let coplanar = |side: Sign| {
        [0, 1, 2]
            .into_iter()
            .try_fold(side == Sign::Zero, |all, k| {
                Some(all && meet_2d(drop(p, k), drop(q, k), drop(r, k), drop(s, k))?)
            })
    };
    orient3d(p, q, r, s)
        .ok()
        .and_then(coplanar)
        .map_or(Meet::Unknown, cross_if)
}

/// Whether 2D segments a–b and c–d meet (touching counts); `None` if a step overflows.
fn meet_2d(a: [i128; 2], b: [i128; 2], c: [i128; 2], d: [i128; 2]) -> Option<bool> {
    let sides = [
        orient(c, d, a)?,
        orient(c, d, b)?,
        orient(a, b, c)?,
        orient(a, b, d)?,
    ];
    let [ca, cb, ac, ad] = sides;
    // Each pair's ends strictly on opposite sides of the other's line.
    let proper = opposite(ca, cb) && opposite(ac, ad);
    // An end on the other's line, within its box.
    let touching = [(c, d, a, ca), (c, d, b, cb), (a, b, c, ac), (a, b, d, ad)]
        .into_iter()
        .try_fold(false, |any, (x, y, z, side)| {
            Some(any || side == 0 && within(x, y, z)?)
        })?;
    Some(proper || touching)
}

/// Whether `x` and `y` are certainly of opposite signs, neither zero.
fn opposite(x: i128, y: i128) -> bool {
    x.signum().checked_mul(y.signum()) == Some(-1)
}

/// Whether `z`, on the line through `x` and `y`, lies between them: in their box.
fn within(x: [i128; 2], y: [i128; 2], z: [i128; 2]) -> Option<bool> {
    let [x0, x1] = x;
    let [y0, y1] = y;
    let [z0, z1] = z;
    let first = z0.checked_sub(x0)?.checked_mul(z0.checked_sub(y0)?)?;
    let second = z1.checked_sub(x1)?.checked_mul(z1.checked_sub(y1)?)?;
    Some(first.max(second) <= 0)
}

/// The orientation of `a`, `b`, `c`: (b − a) × (c − a), exactly; `None` if a step overflows.
fn orient(a: [i128; 2], b: [i128; 2], c: [i128; 2]) -> Option<i128> {
    let [a0, a1] = a;
    let [b0, b1] = b;
    let [c0, c1] = c;
    b0.checked_sub(a0)?
        .checked_mul(c1.checked_sub(a1)?)?
        .checked_sub(b1.checked_sub(a1)?.checked_mul(c0.checked_sub(a0)?)?)
}

/// `p` with coordinate `k` dropped.
fn drop(p: P3, k: usize) -> [i128; 2] {
    let [x, y, z] = [p.x, p.y, p.z].map(|c| i128::from(c.get()));
    match k {
        0 => [y, z],
        1 => [x, z],
        _ => [x, y],
    }
}

/// `p − q`, exactly.
fn diff(p: P3, q: P3) -> Option<[i128; 3]> {
    let d = |a: Coord, b: Coord| i128::from(a.get()).checked_sub(i128::from(b.get()));
    Some([d(p.x, q.x)?, d(p.y, q.y)?, d(p.z, q.z)?])
}

/// `u × w`, exactly.
fn cross(u: [i128; 3], w: [i128; 3]) -> Option<[i128; 3]> {
    let [u0, u1, u2] = u;
    let [w0, w1, w2] = w;
    let term =
        |a: i128, b: i128, c: i128, d: i128| a.checked_mul(b)?.checked_sub(c.checked_mul(d)?);
    Some([
        term(u1, w2, u2, w1)?,
        term(u2, w0, u0, w2)?,
        term(u0, w1, u1, w0)?,
    ])
}

/// `u · w`, exactly.
fn dot(u: [i128; 3], w: [i128; 3]) -> Option<i128> {
    let [u0, u1, u2] = u;
    let [w0, w1, w2] = w;
    u0.checked_mul(w0)?
        .checked_add(u1.checked_mul(w1)?)?
        .checked_add(u2.checked_mul(w2)?)
}

/// Whether faces `f` and `g` share an edge that is a full circle.
fn shares_full_circle(a: &Arenas, f: Ix<Face>, g: Ix<Face>) -> bool {
    edges(a, f).any(|e| {
        let full = matches!(
            shape(a, e),
            Some(Shape::Circle(circle, _)) if {
                let (lo, hi) = circle.domain();
                hi - lo == 2.0
            }
        );
        full && edge_fins(a, e).any(|n| fin_face(a, n) == Some(g))
    })
}
