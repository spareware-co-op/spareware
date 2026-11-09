//! The second checker for flat bodies (chapter 28.4; piece G135): chapter 8.3's rules, written again
//! from their definitions in exact rationals, sharing no code with `geom-check`, to stand behind it.
//!
//! What it does: [`check`] reads a body's rows into rationals: each vertex a point, each face its
//! rings of corners and the plane its surface gives. Then it tests the rules one by one, in the most
//! direct way there is, and lists a [`Finding`] for each one broken.
//!
//! - **Structure.** Each loop's fins run nose to tail and close, and in a solid each edge has two
//!   fins running opposite ways.
//! - **Geometry.** Each corner lies on its face's plane, within the bound the body is recorded with,
//!   and each straight edge's line within a step of its vertices.
//! - **Crossing.** Two faces' planes meet in a line, or are one plane. Every point where a boundary
//!   meets the line is listed, with the midpoint between each two: a point in both faces that is
//!   not on something they share is a crossing. On one plane the list is every corner, every point
//!   where two edges meet and the midpoints along each edge, and a point inside each face, found on
//!   a scan line.
//! - **Loops and shells.** Each face has one loop turning counter-clockwise from outside and the
//!   rest clockwise, and a positive area when they are added. A solid region has one shell of positive volume and the rest negative, and each
//!   shell lies where its region allows, by the parity of a ray.
//!
//! Nothing here calls `geom-check`, its predicates or `geom_num`'s planes: a plane is three rationals
//! and an offset, and a vertex where three planes meet is found by Cramer's rule. What a body stores
//! is read through `geom_topo`'s public rows, since a body is the thing checked.
//!
//! Guarantees:
//!
//! - **Exact.** Every value is a rational and every comparison a comparison of rationals, with no
//!   width to overflow and no tolerance but the bound the body is recorded with.
//! - **Independent.** A different method wherever one exists: points listed and located, where
//!   `geom-check` compares stretches of a line by the signs of planes; Cramer's rule, where it never
//!   leaves the planes; crossing numbers, where it counts by moved lines.
//! - **Says what it cannot judge.** A curved face or edge, or a face whose corners leave one plane,
//!   is [`Finding::Unjudged`], never a pass (flat bodies and meshes are what 28.4 asks of it).
//!
//! Method: chapter 8.3's rules (`projects/engines/geometry/architecture/08-topology.md`); the crossing
//! number for a point in a polygon (Haines, "Point in polygon strategies", Graphics Gems IV, 1994,
//! doi:10.1016/B978-0-12-336156-1.50013-6); Cramer's rule for three planes; the divergence theorem
//! for a polyhedron's volume (Goldman, Graphics Gems II, 1991, doi:10.1016/B978-0-08-050754-5.50043-8).
//!
//! Cost: rationals grow with the depth of the sums, and each pair of faces costs the product of
//! their edges. This checker accepts that, because it runs on tests only. No recursion.

// The lint is for fixed-width arithmetic, which can overflow. Every operator here is on dashu's
// arbitrary-precision rationals, which cannot, and every division is by a value shown nonzero
// beside it.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "exact rational arithmetic, which cannot overflow; divisors are shown nonzero"
)]
#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': a to d points, n a normal, d an offset, s and t parameters"
)]

use core::cmp::Ordering;

use dashu_int::{IBig, UBig};
use dashu_ratio::RBig;
use geom_curve::{CurveKind, ParamCurve};
use geom_num::{Exactness, P3};
use geom_surface::SurfaceKind;
use geom_topo::ix::Arena;
use geom_topo::{Arenas, Body, BodyKind, Face, Ix, PointKind, RegionKind, Sense, Shell, Vertex};

/// A point or a vector.
type V3 = [RBig; 3];
/// A point of a face's plane, in two of its coordinates.
type V2 = [RBig; 2];

/// One rule broken, or one thing this checker cannot judge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finding {
    /// A curved face or edge, or a face whose corners leave one plane.
    Unjudged,
    /// A loop whose fins do not run nose to tail and close, or a solid's edge without two fins
    /// running opposite ways.
    Structure,
    /// A vertex where three planes that do not meet in one point meet.
    Singular(u32),
    /// A corner off its face's plane by more than the body's bound.
    OffPlane(u32),
    /// A vertex off its edge's line.
    OffLine(u32),
    /// A corner outside its face's stored box.
    OutOfBox(u32),
    /// Two edges of a face meeting away from a vertex they share.
    EdgesCross(u32),
    /// Two faces meeting away from anything they share.
    FacesCross(u32, u32),
    /// A face without one loop counter-clockwise from outside and the rest clockwise.
    LoopSense(u32),
    /// A solid region without exactly one shell facing outward.
    ShellSense,
    /// A shell where its region does not allow it.
    Nesting(u32),
}

/// A number as a rational.
fn q(n: i128) -> RBig {
    RBig::from(IBig::from(n))
}

/// A double as a rational, exactly.
fn from_double(x: f64) -> Option<RBig> {
    RBig::try_from(x).ok()
}

/// A grid point as a rational point.
fn point(p: P3) -> V3 {
    [p.x, p.y, p.z].map(|c| q(i128::from(c.get())))
}

/// `f` of each pair of coordinates.
fn zipped(a: &V3, b: &V3, f: impl Fn(&RBig, &RBig) -> RBig) -> V3 {
    let ([ax, ay, az], [bx, by, bz]) = (a, b);
    [f(ax, bx), f(ay, by), f(az, bz)]
}

/// `a − b`.
fn minus(a: &V3, b: &V3) -> V3 {
    zipped(a, b, |x, y| x - y)
}

/// `a + b`.
fn plus(a: &V3, b: &V3) -> V3 {
    zipped(a, b, |x, y| x + y)
}

/// `a × b`.
fn cross(a: &V3, b: &V3) -> V3 {
    let ([ax, ay, az], [bx, by, bz]) = (a, b);
    [ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx]
}

/// `a · b`.
fn dot(a: &V3, b: &V3) -> RBig {
    let ([ax, ay, az], [bx, by, bz]) = (a, b);
    ax * bx + ay * by + az * bz
}

/// Whether a vector is zero.
fn zero(a: &V3) -> bool {
    a.iter().all(|c| *c == RBig::ZERO)
}

/// The point where three planes n · x + d = 0 meet, by Cramer's rule: `None` if they do not meet in
/// one point. The determinant is shown nonzero before it divides.
fn meet(planes: [([i128; 3], i128); 3]) -> Option<V3> {
    let [(a, da), (b, db), (c, dc)] = planes;
    let (a, b, c) = (a.map(q), b.map(q), c.map(q));
    let across = cross(&b, &c);
    let det = dot(&a, &across);
    if det == RBig::ZERO {
        return None;
    }
    let (ca, ab) = (cross(&c, &a), cross(&a, &b));
    let (da, db, dc) = (q(da), q(db), q(dc));
    let sum = plus(
        &across.map(|c| &da * c),
        &plus(&ca.map(|c| &db * c), &ab.map(|c| &dc * c)),
    );
    Some(sum.map(|c| -c / &det))
}

/// A fin, as a ring holds it.
#[derive(Clone, Copy)]
struct Side {
    edge: u32,
    from: u32,
    to: u32,
}

/// A face, read.
struct Plate {
    row: u32,
    shell: u32,
    forward: bool,
    rings: Vec<Vec<Side>>,
    /// The plane its surface gives: n · x + d = 0, n pointing as the surface's normal does.
    n: V3,
    d: RBig,
    low: V3,
    high: V3,
}

/// A body, read.
struct Read {
    points: Vec<Option<V3>>,
    plates: Vec<Plate>,
    findings: Vec<Finding>,
}

/// The rows a list of `rows` rows counts.
fn indices<K>(rows: u32) -> impl Iterator<Item = Ix<K>> {
    (0..rows).filter_map(Ix::new)
}

/// Where each vertex lies, `None` for planes that do not meet.
fn places(a: &Arenas) -> Option<Vec<Option<V3>>> {
    indices::<Vertex>(a.rows(Arena::Vertex))
        .map(|v| {
            let place = a.vertices.point.get(v).and_then(|p| a.points.get(*p))?;
            Some(match place {
                PointKind::Grid(p) => Some(point(*p)),
                PointKind::Meet(rows) => {
                    let [p, q, r] = rows.map(|ix| a.planes.get(ix).map(|x| (x.n(), x.d())));
                    meet([p?, q?, r?])
                }
            })
        })
        .collect()
}

/// A face's ring of sides, from a loop's first fin: `None` if it does not end.
fn ring(a: &Arenas, first: Ix<geom_topo::Fin>) -> Option<Vec<Side>> {
    let mut out = Vec::new();
    let mut at = first;
    for _ in 0..=a.rows(Arena::Fin) {
        let edge = *a.fins.edge.get(at)?;
        let (s, t) = ((*a.edges.start.get(edge)?)?, (*a.edges.end.get(edge)?)?);
        let (from, to) = if *a.fins.sense.get(at)? == Sense::Forward {
            (s, t)
        } else {
            (t, s)
        };
        out.push(Side {
            edge: edge.row(),
            from: from.row(),
            to: to.row(),
        });
        at = *a.fins.next.get(at)?;
        if at == first {
            return Some(out);
        }
    }
    None
}

/// The rational plane of a face's surface, if the surface is a plane.
fn plane_of(surface: &SurfaceKind) -> Option<(V3, RBig)> {
    let SurfaceKind::Plane(flat) = surface else {
        return None;
    };
    let (origin, along, across) = flat.vectors();
    let exact = |w: [f64; 3]| -> Option<V3> {
        let [x, y, z] = w.map(from_double);
        Some([x?, y?, z?])
    };
    let (o, u, v) = (exact(origin)?, exact(along)?, exact(across)?);
    let n = cross(&u, &v);
    let d = -dot(&n, &o);
    Some((n, d))
}

/// Face `f`, read: `None` for anything this checker does not judge.
fn plate(a: &Arenas, f: Ix<Face>) -> Option<Plate> {
    let surface = a.surfaces.get(*a.faces.surface.get(f)?)?;
    let (n, d) = plane_of(surface)?;
    let mut rings = Vec::new();
    let mut at = *a.faces.first_loop.get(f)?;
    for _ in 0..=a.rows(Arena::Loop) {
        let Some(l) = at else { break };
        rings.push(ring(a, (*a.loops.first_fin.get(l)?)?)?);
        at = *a.loops.next.get(l)?;
    }
    let bound = a.faces.bound.get(f)?;
    Some(Plate {
        row: f.row(),
        shell: a.faces.shell.get(f)?.row(),
        forward: *a.faces.sense.get(f)? == Sense::Forward,
        rings,
        n,
        d,
        low: point(bound.min()),
        high: point(bound.max()),
    })
}

/// A body's every straight edge: the rule that its line ends at its vertices within a step.
fn lines(a: &Arenas, points: &[Option<V3>], findings: &mut Vec<Finding>, bound: &RBig) {
    for e in indices::<geom_topo::Edge>(a.rows(Arena::Edge)) {
        let curve = a.edges.curve.get(e).and_then(|c| a.curves.get(*c));
        let Some(CurveKind::Line(line)) = curve else {
            findings.push(Finding::Unjudged);
            continue;
        };
        let (lo, hi) = line.domain();
        let ends = [
            (a.edges.start.get(e).copied().flatten(), lo),
            (a.edges.end.get(e).copied().flatten(), hi),
        ];
        for (vertex, t) in ends {
            let Some(v) = vertex else {
                findings.push(Finding::Structure);
                continue;
            };
            let at = |o: f64, d: f64| {
                from_double(o)
                    .zip(from_double(d))
                    .zip(from_double(t))
                    .map(|((o, d), t)| o + d * t)
            };
            let ([ox, oy, oz], [dx, dy, dz]) = (line.origin(), line.direction());
            let end = [at(ox, dx), at(oy, dy), at(oz, dz)];
            let place = points.get(v.to_usize()).and_then(Option::as_ref);
            // A meeting of planes is shadowed: a step more than the bound, and a half for the
            // rounding of its place to the grid in each coordinate.
            let meeting = a
                .vertices
                .point
                .get(v)
                .and_then(|p| a.points.get(*p))
                .is_some_and(|p| matches!(p, PointKind::Meet(_)));
            let room = if meeting {
                bound + q(3) / q(2)
            } else {
                bound.clone()
            };
            let held = place.is_some_and(|p| {
                p.iter().zip(&end).all(|(c, e)| {
                    e.as_ref().is_some_and(|e| {
                        let gap = c - e;
                        gap.clone() <= room && -gap <= room
                    })
                })
            });
            if !held {
                findings.push(Finding::OffLine(v.row()));
            }
        }
    }
}

/// Reads `body`, recorded with bound `bound`.
fn read(body: &Body, bound: &RBig) -> Read {
    let a = body.arenas();
    let mut findings = Vec::new();
    let points = places(a).unwrap_or_default();
    for (v, p) in points.iter().enumerate() {
        if p.is_none() {
            findings.push(Finding::Singular(u32::try_from(v).unwrap_or(u32::MAX)));
        }
    }
    let mut plates = Vec::new();
    for f in indices::<Face>(a.rows(Arena::Face)) {
        match plate(a, f) {
            Some(one) => plates.push(one),
            None => findings.push(Finding::Unjudged),
        }
    }
    lines(a, &points, &mut findings, bound);
    Read {
        points,
        plates,
        findings,
    }
}

/// The value of `plate`'s plane at `x`.
fn height(plate: &Plate, x: &V3) -> RBig {
    dot(&plate.n, x) + &plate.d
}

/// Each corner on its face's plane, within `bound`; and in its stored box.
fn on_planes(r: &mut Read, bound: &RBig, meetings: &[bool]) {
    let mut found = Vec::new();
    for plate in &r.plates {
        let norm = dot(&plate.n, &plate.n);
        for side in plate.rings.iter().flatten() {
            let row = side.from as usize;
            let Some(Some(x)) = r.points.get(row) else {
                continue;
            };
            let h = height(plate, x);
            // A meeting of planes is on its faces exactly, whatever the class (8.3).
            let meeting = meetings.get(row).copied().unwrap_or(false);
            let held = if meeting {
                h == RBig::ZERO
            } else {
                &h * &h <= bound * bound * &norm
            };
            if !held {
                found.push(Finding::OffPlane(side.from));
            }
            let boxed = |(c, (lo, hi)): (&RBig, (&RBig, &RBig))| lo <= c && c <= hi;
            if !x.iter().zip(plate.low.iter().zip(&plate.high)).all(boxed) {
                found.push(Finding::OutOfBox(plate.row));
            }
        }
    }
    r.findings.append(&mut found);
}

/// Each loop's sides nose to tail, and in a solid each edge with two fins running opposite ways.
fn structure(r: &mut Read, solid: bool) {
    let mut tally: Vec<(u32, u32, u32)> = Vec::new();
    let mut broken = false;
    for side in r.plates.iter().flat_map(|p| p.rings.iter().flatten()) {
        tally.push((side.edge, side.from, side.to));
    }
    for ring in r.plates.iter().flat_map(|p| p.rings.iter()) {
        for (s, t) in ring.iter().zip(ring.iter().cycle().skip(1)) {
            broken |= s.to != t.from;
        }
    }
    if solid {
        tally.sort_unstable();
        for chunk in tally.chunk_by(|p, q| p.0 == q.0) {
            let opposite = matches!(chunk, [p, q] if p.1 == q.2 && p.2 == q.1);
            broken |= !opposite;
        }
    }
    if broken {
        r.findings.push(Finding::Structure);
    }
}

/// The coordinate `n` is largest in, the one a plane drops to map onto two others one to one.
fn largest(n: &V3) -> usize {
    let abs = |x: &RBig| {
        if *x < RBig::ZERO {
            -x.clone()
        } else {
            x.clone()
        }
    };
    let mut best = (0, RBig::ZERO);
    for (k, c) in n.iter().enumerate() {
        if abs(c) > best.1 {
            best = (k, abs(c));
        }
    }
    best.0
}

/// `x` with coordinate `axis` dropped.
fn flat(x: &V3, axis: usize) -> V2 {
    let [a, b, c] = x;
    match axis {
        0 => [b.clone(), c.clone()],
        1 => [a.clone(), c.clone()],
        _ => [a.clone(), b.clone()],
    }
}

/// `(b − a) × (c − a)` in the plane.
fn turn(a: &V2, b: &V2, c: &V2) -> RBig {
    let ([ax, ay], [bx, by], [cx, cy]) = (a, b, c);
    (bx - ax) * (cy - ay) - (by - ay) * (cx - ax)
}

/// Whether `x`, on the line through `a` and `b`, lies between them.
fn between(a: &V2, b: &V2, x: &V2) -> bool {
    a.iter().zip(b).zip(x).all(|((p, q), c)| {
        let (lo, hi) = if p <= q { (p, q) } else { (q, p) };
        lo <= c && c <= hi
    })
}

/// Where a point lies against a polygon.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Place {
    Out,
    Edge,
    In,
}

/// Where `x` lies against the rings of `segments`, by the crossing number of a ray along the first
/// coordinate, taking each edge half open so that a corner on the ray counts once. A divisor
/// `b₁ − a₁` is nonzero: the half-open test holds for no level edge.
fn locate(segments: &[(V2, V2)], x: &V2) -> Place {
    let mut odd = false;
    let [px, py] = x;
    for (a, b) in segments {
        if turn(a, b, x) == RBig::ZERO && between(a, b, x) {
            return Place::Edge;
        }
        let ([ax, ay], [bx, by]) = (a, b);
        if (ay <= py) != (by <= py) {
            let at = ax + (py - ay) * (bx - ax) / (by - ay);
            odd ^= at > *px;
        }
    }
    if odd { Place::In } else { Place::Out }
}

/// A face's sides, each as a segment in its corners' points.
fn sides(r: &Read, plate: &Plate) -> Option<Vec<(V3, V3)>> {
    plate
        .rings
        .iter()
        .flatten()
        .map(|s| {
            Some((
                r.points.get(s.from as usize)?.clone()?,
                r.points.get(s.to as usize)?.clone()?,
            ))
        })
        .collect()
}

/// `sides` projected along `axis`.
fn flats(sides: &[(V3, V3)], axis: usize) -> Vec<(V2, V2)> {
    sides
        .iter()
        .map(|(a, b)| (flat(a, axis), flat(b, axis)))
        .collect()
}

/// Whether `x` lies on the closed segment from `a` to `b`.
fn on_segment(a: &V3, b: &V3, x: &V3) -> bool {
    zero(&cross(&minus(b, a), &minus(x, a))) && {
        let along = dot(&minus(x, a), &minus(b, a));
        along >= RBig::ZERO && along <= dot(&minus(b, a), &minus(b, a))
    }
}

/// Whether `x` is on something `f` and `g` share: a vertex of both, or a point of an edge of both.
fn shared(r: &Read, f: &Plate, g: &Plate, x: &V3) -> bool {
    let of = |p: &Plate| -> Vec<Side> { p.rings.iter().flatten().copied().collect() };
    let (one, other) = (of(f), of(g));
    let place = |v: u32| r.points.get(v as usize).and_then(Option::as_ref);
    one.iter().any(|s| {
        let vertex =
            place(s.from).is_some_and(|p| p == x) && other.iter().any(|t| t.from == s.from);
        let edge = other.iter().any(|t| t.edge == s.edge)
            && place(s.from)
                .zip(place(s.to))
                .is_some_and(|(a, b)| on_segment(a, b, x));
        vertex || edge
    })
}

/// Whether any of `list` lies in both faces away from what they share.
fn any_crossing(r: &Read, f: &Plate, g: &Plate, list: &[V3]) -> Option<bool> {
    let axis = largest(&f.n);
    let (one, other) = (flats(&sides(r, f)?, axis), flats(&sides(r, g)?, axis));
    Some(list.iter().any(|x| {
        let at = flat(x, axis);
        locate(&one, &at) != Place::Out && locate(&other, &at) != Place::Out && !shared(r, f, g, x)
    }))
}

/// `points` sorted along `direction`, one of each place, with the midpoint of each two in turn.
fn with_midpoints(mut points: Vec<V3>, direction: &V3) -> Vec<V3> {
    points.sort_by_key(|p| dot(p, direction));
    points.dedup();
    let mut out = Vec::new();
    for pair in points.windows(2) {
        if let [a, b] = pair {
            out.push(a.clone());
            out.push(plus(a, b).map(|c| c / q(2)));
        }
    }
    out.extend(points.last().cloned());
    out
}

/// Where the sides of `plate` meet the plane of `other`.
fn meets(sides: &[(V3, V3)], other: &Plate) -> Vec<V3> {
    let mut out = Vec::new();
    for (a, b) in sides {
        let (s, t) = (height(other, a), height(other, b));
        if s == RBig::ZERO {
            out.push(a.clone());
        }
        if t == RBig::ZERO {
            out.push(b.clone());
        }
        // s and t of opposite signs, so s − t is nonzero.
        if (s < RBig::ZERO) != (t < RBig::ZERO) && s != RBig::ZERO && t != RBig::ZERO {
            let share = &s / (&s - &t);
            out.push(zipped(a, b, |x, y| x + (y - x) * &share));
        }
    }
    out
}

/// A point inside `plate`, on the scan line halfway between its two lowest levels in `axis`'s plane.
/// A level between corners crosses no corner, so its crossings come in pairs, and the first two
/// bound a stretch of the face.
fn inside(sides: &[(V3, V3)], n: &V3, d: &RBig, axis: usize) -> Option<V3> {
    let segments = flats(sides, axis);
    let mut levels: Vec<RBig> = segments.iter().map(|([_, y], _)| y.clone()).collect();
    levels.sort();
    levels.dedup();
    let level = (levels.first()? + levels.get(1)?) / q(2);
    let mut crossings: Vec<RBig> = segments
        .iter()
        .filter(|([_, ay], [_, by])| (*ay <= level) != (*by <= level))
        .map(|([ax, ay], [bx, by])| ax + (&level - ay) * (bx - ax) / (by - ay))
        .collect();
    crossings.sort();
    let across = (crossings.first()? + crossings.get(1)?) / q(2);
    // The dropped coordinate from the plane; n's largest is nonzero.
    let [nx, ny, nz] = n;
    let (first, second, dropped) = match axis {
        0 => (ny, nz, nx),
        1 => (nx, nz, ny),
        _ => (nx, ny, nz),
    };
    let rest = -(first * &across + second * &level + d) / dropped;
    Some(match axis {
        0 => [rest, across, level],
        1 => [across, rest, level],
        _ => [across, level, rest],
    })
}

/// Whether two faces on one plane meet away from what they share.
fn coplanar(r: &Read, f: &Plate, g: &Plate) -> Option<bool> {
    let (first, second) = (sides(r, f)?, sides(r, g)?);
    let axis = largest(&f.n);
    let mut points: Vec<V3> = Vec::new();
    for (a, b) in first.iter().chain(&second) {
        points.push(a.clone());
        points.push(b.clone());
    }
    let mut along = Vec::new();
    for (a, b) in &first {
        let mut stops = vec![a.clone(), b.clone()];
        for (c, d) in &second {
            let (s, t) = (
                turn(&flat(a, axis), &flat(b, axis), &flat(c, axis)),
                turn(&flat(a, axis), &flat(b, axis), &flat(d, axis)),
            );
            // Not parallel, so s − t is nonzero; the meeting is on the side's line.
            if s != t {
                let share = &s / (&s - &t);
                let at: V3 = zipped(c, d, |x, y| x + (y - x) * &share);
                if on_segment(a, b, &at) {
                    stops.push(at);
                }
            } else if s == RBig::ZERO {
                stops.extend([c, d].into_iter().filter(|x| on_segment(a, b, x)).cloned());
            }
        }
        along.extend(with_midpoints(stops, &minus(b, a)));
    }
    points.append(&mut along);
    points.extend(inside(&first, &f.n, &f.d, axis));
    points.extend(inside(&second, &f.n, &f.d, axis));
    any_crossing(r, f, g, &points)
}

/// Whether two faces meet away from what they share; `None` if a corner has no place.
fn faces_cross(r: &Read, f: &Plate, g: &Plate) -> Option<bool> {
    let below = |p: &V3, q: &V3| p.iter().zip(q).any(|(a, b)| a < b);
    let apart = below(&f.high, &g.low) || below(&g.high, &f.low);
    if apart {
        return Some(false);
    }
    let direction = cross(&f.n, &g.n);
    if zero(&direction) {
        // One plane, or two apart. n and the other's normal are parallel, and each nonzero.
        let k = largest(&f.n);
        let scale = g.n.get(k)? / f.n.get(k)?;
        return if g.d == &f.d * &scale {
            coplanar(r, f, g)
        } else {
            Some(false)
        };
    }
    let mut points = meets(&sides(r, f)?, g);
    points.extend(meets(&sides(r, g)?, f));
    any_crossing(r, f, g, &with_midpoints(points, &direction))
}

/// Whether two sides of a face meet away from a vertex they share.
fn sides_cross(r: &Read, axis: usize, s: Side, t: Side) -> Option<bool> {
    let place = |v: u32| r.points.get(v as usize)?.as_ref().map(|p| flat(p, axis));
    let (a, b) = (place(s.from)?, place(s.to)?);
    let (c, d) = (place(t.from)?, place(t.to)?);
    let (ab_c, ab_d) = (turn(&a, &b, &c), turn(&a, &b, &d));
    let (cd_a, cd_b) = (turn(&c, &d, &a), turn(&c, &d, &b));
    let share = [(s.from, &a, &b), (s.to, &b, &a)]
        .into_iter()
        .find_map(|(v, near, far)| {
            [(t.from, &d), (t.to, &c)]
                .into_iter()
                .find(|(w, _)| *w == v)
                .map(|(_, other)| (near, far, other))
        });
    if let Some((near, far, other)) = share {
        // They run from one vertex: they meet again only along each other, the same way.
        let ([fx, fy], [ox, oy], [nx, ny]) = (far, other, near);
        let along = (fx - nx) * (ox - nx) + (fy - ny) * (oy - ny);
        return Some(turn(near, far, other) == RBig::ZERO && along > RBig::ZERO);
    }
    let opposite = |p: &RBig, q: &RBig| {
        (*p < RBig::ZERO && *q > RBig::ZERO) || (*p > RBig::ZERO && *q < RBig::ZERO)
    };
    let touching = (ab_c == RBig::ZERO && between(&a, &b, &c))
        || (ab_d == RBig::ZERO && between(&a, &b, &d))
        || (cd_a == RBig::ZERO && between(&c, &d, &a))
        || (cd_b == RBig::ZERO && between(&c, &d, &b));
    Some(touching || (opposite(&ab_c, &ab_d) && opposite(&cd_a, &cd_b)))
}

/// Group 3 of chapter 8.3 on the read plates.
fn crossings(r: &mut Read) {
    let mut found = Vec::new();
    for plate in &r.plates {
        let axis = largest(&plate.n);
        let all: Vec<Side> = plate.rings.iter().flatten().copied().collect();
        let mut edges: Vec<Side> = Vec::new();
        for s in all {
            if !edges.iter().any(|t| t.edge == s.edge) {
                edges.push(s);
            }
        }
        for (i, s) in edges.iter().enumerate() {
            for t in edges.iter().skip(i + 1) {
                if sides_cross(r, axis, *s, *t).unwrap_or(false) {
                    found.push(Finding::EdgesCross(plate.row));
                }
            }
        }
    }
    for (i, f) in r.plates.iter().enumerate() {
        for g in r.plates.iter().skip(i + 1) {
            if faces_cross(r, f, g).unwrap_or(false) {
                found.push(Finding::FacesCross(f.row, g.row));
            }
        }
    }
    r.findings.append(&mut found);
}

/// Twice the area vector of a ring.
fn area(r: &Read, ring: &[Side]) -> Option<V3> {
    let mut sum = [RBig::ZERO, RBig::ZERO, RBig::ZERO];
    for s in ring {
        let (a, b) = (
            r.points.get(s.from as usize)?.as_ref()?,
            r.points.get(s.to as usize)?.as_ref()?,
        );
        let c = cross(a, b);
        sum = plus(&sum, &c);
    }
    Some(sum)
}

/// Each face's loops against its outward normal.
fn loops(r: &mut Read) {
    let mut found = Vec::new();
    for plate in &r.plates {
        let mut outer = 0;
        let mut wrong = false;
        let mut whole = RBig::ZERO;
        for ring in &plate.rings {
            let Some(vector) = area(r, ring) else {
                continue;
            };
            let along = dot(&vector, &plate.n);
            let along = if plate.forward { along } else { -along };
            whole += &along;
            match along.partial_cmp(&RBig::ZERO) {
                Some(Ordering::Greater) => outer += 1,
                Some(Ordering::Less) => {}
                _ => wrong = true,
            }
        }
        if outer != 1 || wrong || whole <= RBig::ZERO {
            found.push(Finding::LoopSense(plate.row));
        }
    }
    r.findings.append(&mut found);
}

/// Six times the volume of a shell's faces, each loop a fan from its first corner.
fn volume(r: &Read, shell: u32) -> Option<RBig> {
    let mut sum = RBig::ZERO;
    for plate in r.plates.iter().filter(|p| p.shell == shell) {
        for ring in &plate.rings {
            let first = r.points.get(ring.first()?.from as usize)?.as_ref()?;
            for s in ring {
                let (a, b) = (
                    r.points.get(s.from as usize)?.as_ref()?,
                    r.points.get(s.to as usize)?.as_ref()?,
                );
                sum += dot(first, &cross(a, b));
            }
        }
    }
    Some(sum)
}

/// Directions for a ray, tried in turn until one meets no edge: each a line no two of a fixture's
/// lattice directions share.
const RAYS: [[(i128, i128); 3]; 4] = [
    [(1, 1), (1, 3), (1, 7)],
    [(1, 1), (2, 5), (3, 11)],
    [(-1, 1), (3, 7), (2, 13)],
    [(1, 3), (-5, 7), (1, 1)],
];

/// Whether `x` is inside the shell's faces, by the parity of a ray; `None` if it is on one, or no
/// ray stays clear of the edges.
fn in_shell(r: &Read, shell: u32, x: &V3) -> Option<bool> {
    'rays: for ray in RAYS {
        let direction = ray.map(|(n, d)| q(n) / q(d));
        let mut odd = false;
        for plate in r.plates.iter().filter(|p| p.shell == shell) {
            let axis = largest(&plate.n);
            let segments = flats(&sides(r, plate)?, axis);
            let rate = dot(&plate.n, &direction);
            let h = height(plate, x);
            if rate == RBig::ZERO {
                if h == RBig::ZERO && locate(&segments, &flat(x, axis)) != Place::Out {
                    return None;
                }
                continue;
            }
            // rate is nonzero.
            let t = -&h / &rate;
            if t < RBig::ZERO {
                continue;
            }
            let hit: V3 = zipped(x, &direction, |p, d| p + d * &t);
            match locate(&segments, &flat(&hit, axis)) {
                Place::Out => {}
                _ if t == RBig::ZERO => return None,
                Place::Edge => continue 'rays,
                Place::In => odd = !odd,
            }
        }
        return Some(odd);
    }
    None
}

/// A shell's first corner that is not on `other`, and which side of `other` it lies on.
fn side_of(r: &Read, shell: u32, other: u32) -> Option<bool> {
    r.plates
        .iter()
        .filter(|p| p.shell == shell)
        .flat_map(|p| p.rings.iter().flatten())
        .filter_map(|s| r.points.get(s.from as usize)?.as_ref())
        .find_map(|x| in_shell(r, other, x))
}

/// Solid regions: one shell outward, and each shell where its region allows.
fn shells(body: &Body, r: &mut Read) {
    let a = body.arenas();
    let regions: Vec<Option<u32>> = indices::<Shell>(a.rows(Arena::Shell))
        .map(|s| a.shells.region.get(s).map(|x| x.row()))
        .collect();
    let mut outward: Vec<bool> = Vec::new();
    for s in 0..a.rows(Arena::Shell) {
        outward.push(volume(r, s).is_some_and(|v| v > RBig::ZERO));
    }
    let shells_of = |region: u32| {
        regions
            .iter()
            .zip(0_u32..)
            .filter(move |(q, _)| **q == Some(region))
            .map(|(_, s)| s)
    };
    let mut found = Vec::new();
    for region in indices::<geom_topo::Region>(a.rows(Arena::Region)) {
        if a.regions.kind.get(region) != Some(&RegionKind::Solid) {
            continue;
        }
        let list: Vec<u32> = shells_of(region.row()).collect();
        let out = list
            .iter()
            .filter(|&&s| outward.get(s as usize).copied().unwrap_or(false))
            .count();
        if !list.is_empty() && out != 1 {
            found.push(Finding::ShellSense);
        }
    }
    if found.is_empty() {
        for (s, region) in (0_u32..).zip(&regions) {
            for (t, other) in (0_u32..).zip(&regions) {
                let (s_out, t_out) = (
                    outward.get(s as usize).copied().unwrap_or(false),
                    outward.get(t as usize).copied().unwrap_or(false),
                );
                if s == t {
                    continue;
                }
                let want = match (region == other, s_out, t_out) {
                    (true, false, true) => Some(true),
                    (true, false, false) => Some(false),
                    _ => None,
                };
                let lumps = region != other && s_out && t_out;
                if want.is_none() && !lumps {
                    continue;
                }
                let side = side_of(r, s, t);
                let fine = if lumps {
                    // Inside another's outer shell only within a cavity of that region.
                    side != Some(true)
                        || regions
                            .iter()
                            .zip(0_u32..)
                            .filter(|(q, c)| {
                                *q == other && !outward.get(*c as usize).copied().unwrap_or(false)
                            })
                            .any(|(_, c)| side_of(r, s, c) == Some(true))
                } else {
                    side == want
                };
                if !fine {
                    found.push(Finding::Nesting(s));
                }
            }
        }
    }
    r.findings.append(&mut found);
}

/// The bound `exactness` records, in steps.
fn bound_of(exactness: Exactness) -> RBig {
    let steps = match exactness {
        Exactness::Exact => 0,
        Exactness::Certified { bound, .. } => bound.get(),
        Exactness::Approximate { deviation, .. } => deviation.get(),
    };
    RBig::from(UBig::from(steps))
}

/// Which vertices are meetings of planes.
fn meetings(a: &Arenas) -> Vec<bool> {
    indices::<Vertex>(a.rows(Arena::Vertex))
        .map(|v| {
            a.vertices
                .point
                .get(v)
                .and_then(|p| a.points.get(*p))
                .is_some_and(|p| matches!(p, PointKind::Meet(_)))
        })
        .collect()
}

/// Every rule `body`, recorded with `exactness`, breaks, as far as this checker judges.
///
/// Each stage runs on what the ones before it passed, as chapter 8.3's groups do.
#[must_use]
pub fn check(body: &Body, exactness: Exactness) -> Vec<Finding> {
    let bound = bound_of(exactness);
    let mut r = read(body, &bound);
    structure(&mut r, body.kind() == BodyKind::Solid);
    on_planes(&mut r, &bound, &meetings(body.arenas()));
    if r.findings.is_empty() {
        crossings(&mut r);
    }
    if r.findings.is_empty() {
        loops(&mut r);
    }
    if r.findings.is_empty() && body.kind() == BodyKind::Solid {
        shells(body, &mut r);
    }
    r.findings
}
