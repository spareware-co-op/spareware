//! Exact contact and closest features of grid triangles (19.10).
//!
//! What it does: [`triangles`] says whether two triangles are apart, touch or cross, by the test of
//! Devillers and Guigue, which asks only orientation questions. [`closest_feature`] says which
//! vertex, edge or the face of a triangle is nearest a point, and [`point_triangle`] compares that
//! distance with a clearance.
//!
//! Guarantees:
//!
//! - **Exact.** Every decision is an exact predicate on grid points; nothing is rounded.
//! - **`Cross` is proven.** It is returned only when every orientation asked was strict, so the
//!   triangles' interiors truly cross (19.10, "Overlap, proven"). A contact with a zero sign is
//!   `Touch`, which the arrangement settles; `Apart` is proven by a strict separation.
//! - **Symmetric.** Swapping the triangles, or turning either's vertices, gives the same answer.
//! - **Refused, never guessed.** A triangle with no area is [`NumError::Degenerate`].
//!
//! Method: Devillers and Guigue, "Faster Triangle-Triangle Intersection Tests", INRIA RR-4488,
//! 2002 (<https://hal.inria.fr/inria-00072100>), from the paper. If each triangle straddles the
//! other's plane, both cut the planes' common line in an interval, and they meet exactly when the
//! intervals do. With each triangle turned so that its first vertex lies alone on the positive
//! side, each end of an interval comes from one pair of edges, and comparing two ends is one
//! orientation of four points. Coplanar triangles are compared in the plane of their normal's
//! largest axis. Closest features are Ericson's Voronoi regions (Real-Time Collision Detection,
//! 2005, section 5.1.5), in products of dots, as 19.10 sets out
//! (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`).
//!
//! Cost: constant: at most 8 orientations and, for coplanar triangles, 15 more in the plane. No
//! allocation.

#![allow(
    clippy::many_single_char_names,
    reason = "p, q, r and s are points and a, b, c and d signs, as in the paper"
)]

use geom_num::distance::{self, dot_from, product_difference};
use geom_num::grid::{P2, P3};
use geom_num::predicate::{orient2d, orient3d};
use geom_num::{NumError, Sign};

/// How two triangles stand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Contact {
    /// They share no point.
    Apart,
    /// They share a point, and some orientation is zero, so they may only touch.
    Touch,
    /// Every orientation was strict: their interiors cross.
    Cross,
}

/// The nearest part of a triangle to a point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Closest {
    /// Its vertex with this index.
    Vertex(usize),
    /// Its edge from the vertex with this index to the next.
    Edge(usize),
    /// Its face.
    Face,
}

/// `t` turned so that its first vertex is alone on its side of the other plane, from the signs of
/// its vertices: the turned vertices and signs, and whether the first is on the negative side.
fn lone(t: [P3; 3], s: [Sign; 3]) -> Option<([P3; 3], [Sign; 3], bool)> {
    let [p, q, r] = t;
    let [a, b, c] = s;
    let turns = [
        ([p, q, r], [a, b, c]),
        ([q, r, p], [b, c, a]),
        ([r, p, q], [c, a, b]),
    ];
    for (turned, [x, y, z]) in turns {
        let off = |want: Sign| x == want && y != want && z != want;
        if off(Sign::Positive) {
            return Some((turned, [x, y, z], false));
        }
        if off(Sign::Negative) {
            return Some((turned, [x, y, z], true));
        }
    }
    // Two on the plane and one off is caught above; one on it and two to a side stays here.
    for (turned, [x, y, z]) in turns {
        if x == Sign::Zero && y == z {
            return Some((turned, [x, y, z], y == Sign::Positive));
        }
    }
    None
}

/// How two coplanar triangles stand: touching if they share any point.
fn coplanar(a: [P3; 3], b: [P3; 3], normal_axis: usize) -> Result<Contact, NumError> {
    let flat = |p: P3| -> Result<P2, NumError> {
        let (x, y, z) = (p.x.get(), p.y.get(), p.z.get());
        match normal_axis {
            0 => P2::new(y, z),
            1 => P2::new(z, x),
            _ => P2::new(x, y),
        }
    };
    let (ta, tb) = (a.map(flat), b.map(flat));
    let [a0, a1, a2] = ta;
    let [b0, b1, b2] = tb;
    let (ta, tb) = ([a0?, a1?, a2?], [b0?, b1?, b2?]);
    let met = meets(ta, tb)? || inside(ta, tb)? || inside(tb, ta)?;
    Ok(if met { Contact::Touch } else { Contact::Apart })
}

/// Whether any edge of `a` meets any edge of `b`.
fn meets(a: [P2; 3], b: [P2; 3]) -> Result<bool, NumError> {
    let edges = |t: [P2; 3]| {
        let [p, q, r] = t;
        [(p, q), (q, r), (r, p)]
    };
    for (p, q) in edges(a) {
        for (r, s) in edges(b) {
            if segments_meet(p, q, r, s)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Whether `q`, on the line through `a` and `b`, lies between them or on an end.
fn on_segment(a: P2, b: P2, q: P2) -> bool {
    a.min(b) <= q && q <= a.max(b)
}

/// Whether two closed segments share a point.
fn segments_meet(p: P2, q: P2, r: P2, s: P2) -> Result<bool, NumError> {
    let (a, b) = (orient2d(p, q, r)?, orient2d(p, q, s)?);
    let (c, d) = (orient2d(r, s, p)?, orient2d(r, s, q)?);
    if a != b && c != d && a.mul(b) != Sign::Positive && c.mul(d) != Sign::Positive {
        return Ok(true);
    }
    Ok((a == Sign::Zero && on_segment(p, q, r))
        || (b == Sign::Zero && on_segment(p, q, s))
        || (c == Sign::Zero && on_segment(r, s, p))
        || (d == Sign::Zero && on_segment(r, s, q)))
}

/// Whether the first vertex of `a` is inside or on `b`.
fn inside(a: [P2; 3], b: [P2; 3]) -> Result<bool, NumError> {
    let [p, ..] = a;
    let [x, y, z] = b;
    let turn = orient2d(x, y, z)?;
    Ok(orient2d(x, y, p)? != turn.negate()
        && orient2d(y, z, p)? != turn.negate()
        && orient2d(z, x, p)? != turn.negate())
}

/// The largest axis of the normal of `t`.
fn largest_axis(t: [P3; 3]) -> usize {
    let [p, q, r] = t;
    let d = |a: i64, b: i64| i128::from(b).wrapping_sub(i128::from(a));
    let u = [
        d(p.x.get(), q.x.get()),
        d(p.y.get(), q.y.get()),
        d(p.z.get(), q.z.get()),
    ];
    let v = [
        d(p.x.get(), r.x.get()),
        d(p.y.get(), r.y.get()),
        d(p.z.get(), r.z.get()),
    ];
    let [a, b, c] = u;
    let [x, y, z] = v;
    let term = |m: i128, n: i128, o: i128, s: i128| {
        m.wrapping_mul(n)
            .wrapping_sub(o.wrapping_mul(s))
            .unsigned_abs()
    };
    let (nx, ny, nz) = (term(b, z, c, y), term(c, x, a, z), term(a, y, b, x));
    if nx >= ny && nx >= nz {
        0
    } else if ny >= nz {
        1
    } else {
        2
    }
}

/// Whether three points are on one line.
fn no_area(t: [P3; 3]) -> bool {
    let [p, q, r] = t;
    let flat = |x: i64, y: i64| P2::new(x, y);
    let turn = |a: Result<P2, NumError>, b: Result<P2, NumError>, c: Result<P2, NumError>| matches!((a, b, c), (Ok(a), Ok(b), Ok(c)) if orient2d(a, b, c) == Ok(Sign::Zero));
    let view = |f: &dyn Fn(P3) -> Result<P2, NumError>| turn(f(p), f(q), f(r));
    view(&|c: P3| flat(c.x.get(), c.y.get()))
        && view(&|c: P3| flat(c.y.get(), c.z.get()))
        && view(&|c: P3| flat(c.z.get(), c.x.get()))
}

/// How the triangles `a` and `b` stand.
///
/// # Errors
///
/// [`NumError::Degenerate`] for a triangle with no area.
pub fn triangles(a: [P3; 3], b: [P3; 3]) -> Result<Contact, NumError> {
    if no_area(a) || no_area(b) {
        return Err(NumError::Degenerate);
    }
    let side = |t: [P3; 3], p: P3| {
        let [x, y, z] = t;
        orient3d(x, y, z, p)
    };
    let [a0, a1, a2] = a;
    let of_a = [side(b, a0)?, side(b, a1)?, side(b, a2)?];
    let same = |[x, y, z]: [Sign; 3]| x == y && y == z;
    if same(of_a) && of_a[0] != Sign::Zero {
        return Ok(Contact::Apart);
    }
    if of_a == [Sign::Zero; 3] {
        return coplanar(a, b, largest_axis(b));
    }
    let [b0, b1, b2] = b;
    let of_b = [side(a, b0)?, side(a, b1)?, side(a, b2)?];
    if same(of_b) && of_b[0] != Sign::Zero {
        return Ok(Contact::Apart);
    }
    let strict = of_a.iter().chain(&of_b).all(|s| *s != Sign::Zero);
    let (Some((a_turned, _, a_low)), Some((b_turned, _, b_low))) = (lone(a, of_a), lone(b, of_b))
    else {
        return Err(NumError::Degenerate);
    };
    let [p1, q1, r1] = if b_low {
        let [p, q, r] = a_turned;
        [p, r, q]
    } else {
        a_turned
    };
    let [p2, q2, r2] = if a_low {
        let [p, q, r] = b_turned;
        [p, r, q]
    } else {
        b_turned
    };
    let first = orient3d(p1, q1, p2, q2)?;
    let second = orient3d(p1, r1, r2, p2)?;
    if first == Sign::Positive || second == Sign::Positive {
        return Ok(Contact::Apart);
    }
    Ok(
        if strict && first == Sign::Negative && second == Sign::Negative {
            Contact::Cross
        } else {
            Contact::Touch
        },
    )
}

/// Which part of triangle `t` is nearest `p`.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`], which grid points cannot reach.
pub fn closest_feature(p: P3, t: [P3; 3]) -> Result<Closest, NumError> {
    let [a, b, c] = t;
    // Dots from each corner: ab·ap and so on, each below 2^84.
    let (d1, d2) = (dot_from(a, b, p), dot_from(a, c, p));
    if d1 <= 0 && d2 <= 0 {
        return Ok(Closest::Vertex(0));
    }
    let (d3, d4) = (dot_from_edges(a, b, b, p), dot_from_edges(a, c, b, p));
    if d3 >= 0 && d4 <= d3 {
        return Ok(Closest::Vertex(1));
    }
    let vc = product_difference(d1, d4, d3, d2)?;
    if vc != Sign::Positive && d1 >= 0 && d3 <= 0 {
        return Ok(Closest::Edge(0));
    }
    let (d5, d6) = (dot_from_edges(a, b, c, p), dot_from_edges(a, c, c, p));
    if d6 >= 0 && d5 <= d6 {
        return Ok(Closest::Vertex(2));
    }
    let vb = product_difference(d5, d2, d1, d6)?;
    if vb != Sign::Positive && d2 >= 0 && d6 <= 0 {
        return Ok(Closest::Edge(2));
    }
    let va = product_difference(d3, d6, d5, d4)?;
    let from_b = d4.saturating_sub(d3);
    let from_c = d5.saturating_sub(d6);
    if va != Sign::Positive && from_b >= 0 && from_c >= 0 {
        return Ok(Closest::Edge(1));
    }
    Ok(Closest::Face)
}

/// (b − a) · (q − p).
fn dot_from_edges(a: P3, b: P3, p: P3, q: P3) -> i128 {
    let d = |x: i64, y: i64| i128::from(y).wrapping_sub(i128::from(x));
    d(a.x.get(), b.x.get())
        .wrapping_mul(d(p.x.get(), q.x.get()))
        .wrapping_add(d(a.y.get(), b.y.get()).wrapping_mul(d(p.y.get(), q.y.get())))
        .wrapping_add(d(a.z.get(), b.z.get()).wrapping_mul(d(p.z.get(), q.z.get())))
}

/// The sign of the distance from `p` to triangle `t`, less `clearance`, by its closest feature.
///
/// # Errors
///
/// [`NumError::OutOfRange`] for a clearance over [`distance::MAX_CLEARANCE`], and
/// [`NumError::Degenerate`] for a triangle with no area.
pub fn point_triangle(p: P3, t: [P3; 3], clearance: u64) -> Result<Sign, NumError> {
    let [a, b, c] = t;
    if no_area(t) {
        return Err(NumError::Degenerate);
    }
    match closest_feature(p, t)? {
        Closest::Vertex(0) => distance::point_point(p, a, clearance),
        Closest::Vertex(1) => distance::point_point(p, b, clearance),
        Closest::Vertex(_) => distance::point_point(p, c, clearance),
        Closest::Edge(0) => distance::point_line(a, b, p, clearance),
        Closest::Edge(1) => distance::point_line(b, c, p, clearance),
        Closest::Edge(_) => distance::point_line(c, a, p, clearance),
        Closest::Face => distance::point_plane(a, b, c, p, clearance),
    }
}
