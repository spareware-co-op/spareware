//! Roof solving (job 6.2, CON-A4-F4-P4): where the planes of a roof with a slope per edge meet, in
//! ridges, hips and valleys, and the pieces of its bottom face each layer is built from.
//!
//! **Why it is ours.** Every meeting point is worked as an exact fraction (num-rational's big
//! fractions) and only then rounded to the kernel's grid, once, by one rule, so that the same
//! footprint gives the same bytes on every machine and two planes that meet share their corners bit
//! for bit. The polygon and skeleton crates we may use work in floats, whose answers differ in the
//! last place from one build to another and cannot say whether two planes meet exactly; this is
//! plain plane geometry, short enough to hold here in exact arithmetic.
//!
//! **The method it follows.** A roof whose sloped edges rise at one pitch is the roof model of the
//! straight skeleton of its eaves outline (Aichholzer, Aurenhammer, Alberts and Gaertner, "A novel
//! type of skeleton for polygons", 1995): each sloped edge's plane rises from it at the pitch, and
//! each point of the roof lies on the plane of the edge whose wavefront (the edge moved inward at
//! one speed, its height rising with it) reaches it first. An edge that does not slope (a gable's
//! verge, a mono-pitch roof's high side) is a wavefront that stays where it is, as an edge of
//! weight zero in the weighted straight skeleton (Eppstein and Erickson, "Raising roofs, crashing
//! cycles, and playing pool", 1999). Two footprints are solved, each exactly:
//!
//! - **Convex** (a rectangle, along the axes or turned): the roof is the lower envelope of its
//!   sloped planes, so each plane's face is the eaves outline clipped by the half-planes where it is
//!   lower than every other sloped plane. Any edges may slope, and the others are verges.
//! - **Right-angled with a corner of 270 degrees** (an L, a T, a U), every edge sloping: the
//!   straight skeleton of a right-angled polygon is its L-infinity medial axis, so the wavefront at
//!   depth `t` is the outline eroded by a square of half-side `t`, which is the union of the
//!   outline's maximal rectangles each shrunk by `t` on every side. That union changes only at a
//!   depth where a low side of one rectangle meets a high side of another; between two such depths
//!   every wavefront edge sweeps a trapezoid of its plane, whose two parallel sides lie at the two
//!   depths. The roof is those trapezoids.
//!
//! **The frame.** Every edge of a right-angled footprint runs along `u` or square to it, `u` the
//! first edge's direction as its smallest whole step `(p, q)`. In the frame `U = p x + q y`,
//! `V = -q x + p y` (whole numbers, a turn and a scale by `L = sqrt(p^2 + q^2)`) the footprint lies
//! along the axes, so every meeting point is a fraction; along the plan's own axes `L = 1` and the
//! frame is the plan. A depth `h` in the frame is a true distance `h / L`, so a height is
//! `s h / L` for slope `s`.
//!
//! **Onto the grid.** Each meeting point is rounded once: along the axes to the nearest grid point
//! (a half up, each coordinate), turned to the nearest point whose coordinates are even, so every
//! plane's plan area is a whole number of square steps; each height to the nearest step, a half up
//! (for a turned footprint `s h / sqrt(L^2)` exactly, by whole-number square roots). A point met
//! by several planes is one value, so it rounds once for all of them. Along the axes, on a footprint
//! whose corners are on even steps (every corner on the hundredth of a millimetre is), every meeting
//! point is a grid point already and only heights are rounded; each piece then stays flat, as its
//! corners lie on two level lines. A piece whose rounded corners are not on one plane (which only a
//! turned footprint can give) is cut into triangles from its first corner, each flat; that face is
//! then creased by less than a step.
//!
//! **What it refuses**, each by name ([`RoofError`]): a footprint not right-angled
//! ([`RoofError::NotRightAngled`]); sloped edges at different pitches ([`RoofError::UnevenPitches`]:
//! a layer's thickness, square to two slopes of different pitch, stands each face at a different
//! height, so the hip between them moves from face to face off any line this method holds; an
//! uneven gable on a rectangle along the axes is the one exception, built by
//! [`crate::shape`]); an edge that does not slope on a footprint with a corner of 270 degrees
//! ([`RoofError::VergeOnReflex`]); overhangs that differ there ([`RoofError::OverhangsDiffer`]);
//! an overhang on a footprint off the axes ([`RoofError::OverhangOffAxis`], whose eaves would be
//! irrational); and any piece the rounding folds ([`RoofError::NotExact`]).

#![allow(
    clippy::arithmetic_side_effects,
    reason = "the arithmetic here is on num-bigint's and num-rational's arbitrary-precision numbers, \
              which never overflow, every division is checked for zero first ([`div`]), and every \
              fixed-width step is a checked method"
)]

use crate::error::RoofError;
use crate::exact::on_grid;
use crate::roof::EdgeSpec;
use construct_params::Slope;
use construct_solver::Point;
use kit_core::Id;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive, Zero};
use std::collections::{BTreeMap, BTreeSet};

/// An exact fraction.
type Q = BigRational;
/// A point in the frame.
type P = [Q; 2];
/// A piece on the grid: the index of the footprint edge whose plane it lies in, and its corners.
type Piece = (usize, Vec<[i64; 3]>);
/// A wavefront edge's piece as the next band traces it: its line (normal and `n . p` at depth 0),
/// its footprint edge, and its top side.
type Traced = (([i64; 2], Q), usize, [P; 2]);

/// What a line of a roof is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LineKind {
    /// Where two slopes meet at the top, level: a ridge.
    Ridge,
    /// Where two slopes meet at an outer corner, falling to it: a hip.
    Hip,
    /// Where two slopes meet at an inner corner, falling to it: a valley.
    Valley,
    /// The low edge of a slope, along the eaves outline.
    Eave,
    /// The edge of a slope along an edge the roof does not rise from: a verge (and a mono-pitch
    /// roof's high side).
    Verge,
}

impl LineKind {
    /// Its name: `ridge`, `hip`, `valley`, `eave` or `verge`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Ridge => "ridge",
            Self::Hip => "hip",
            Self::Valley => "valley",
            Self::Eave => "eave",
            Self::Verge => "verge",
        }
    }
}

/// One line of a roof's face: what it is and its two end points, in grid steps of the roof's
/// frame, the lower end (by x, then y, then z) first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// What it is.
    pub kind: LineKind,
    /// One end.
    pub from: [i64; 3],
    /// The other.
    pub to: [i64; 3],
}

/// A solved roof's bottom face.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Solution {
    /// The eaves outline in plan, anticlockwise, each corner with the edge that starts there.
    pub outline: Vec<(Id, [i64; 2])>,
    /// The one slope every sloped edge rises at.
    pub slope: Slope,
    /// The bottom face's flat pieces, in the order the layers unite them: each the index of the
    /// footprint edge whose plane it lies in, and its corners anticlockwise from above.
    pub pieces: Vec<Piece>,
    /// Twice the plan area of every piece, summed: twice the area the roof covers.
    pub twice_area: i128,
    /// Its lines, on the bottom face.
    pub lines: Vec<Line>,
}

/// The value `v` as a fraction.
fn q(v: i128) -> Q {
    Q::from_integer(BigInt::from(v))
}

/// `a / b`, refused when `b` is zero.
fn div(a: &Q, b: &Q) -> Result<Q, RoofError> {
    if b.is_zero() {
        Err(RoofError::OutOfRange)
    } else {
        Ok(a / b)
    }
}

/// The nearest whole number to `v`, a half up, on the kernel's grid.
fn nearest(v: &Q) -> Result<i64, RoofError> {
    let half = Q::new(BigInt::from(1), BigInt::from(2));
    let whole = (v + half).floor().to_integer();
    on_grid(whole.to_i128().ok_or(RoofError::OutOfRange)?)
}

/// The nearest even whole number to `v`, a half up.
fn nearest_even(v: &Q) -> Result<i64, RoofError> {
    let half = nearest(&(v / q(2)))?;
    on_grid(
        i128::from(half)
            .checked_mul(2)
            .ok_or(RoofError::OutOfRange)?,
    )
}

/// The nearest whole number to `v / sqrt(l2)`, a half up, for `v` at or above zero: exact, by
/// whole-number square roots (`floor(sqrt(x)) = isqrt(floor(x))`, then one comparison of squares
/// for the half).
fn nearest_over_root(v: &Q, l2: i128) -> Result<i64, RoofError> {
    if v.is_negative() || l2 <= 0 {
        return Err(RoofError::OutOfRange);
    }
    let (a, b) = (v.numer().clone(), v.denom().clone());
    let l2 = BigInt::from(l2);
    let below = (&a * &a) / (&b * &b * &l2);
    let floor = below.sqrt();
    let odd = &floor * 2 + 1;
    let twice = &a * 2;
    let up = &twice * &twice >= &odd * &odd * &b * &b * &l2;
    let whole = if up { floor + 1 } else { floor };
    on_grid(whole.to_i128().ok_or(RoofError::OutOfRange)?)
}

/// Twice the signed area of a polygon in the frame.
fn twice_area_q(poly: &[P]) -> Q {
    poly.iter()
        .zip(poly.iter().cycle().skip(1))
        .fold(Q::zero(), |sum, ([x0, y0], [x1, y1])| {
            sum + x0 * y1 - x1 * y0
        })
}

/// Twice the signed area of a polygon of grid points, by its plan.
fn twice_area(poly: &[[i64; 3]]) -> Result<i128, RoofError> {
    let mut sum = 0_i128;
    for (a, b) in poly.iter().zip(poly.iter().cycle().skip(1)) {
        let term = i128::from(a[0])
            .checked_mul(i128::from(b[1]))
            .and_then(|t| t.checked_sub(i128::from(b[0]).checked_mul(i128::from(a[1]))?))
            .ok_or(RoofError::OutOfRange)?;
        sum = sum.checked_add(term).ok_or(RoofError::OutOfRange)?;
    }
    Ok(sum)
}

/// `(b - a) x (c - b)` in plan: above zero where the turn at `b` is to the left.
fn turn(a: &P, b: &P, c: &P) -> Q {
    let ([ax, ay], [bx, by], [cx, cy]) = (a, b, c);
    (bx - ax) * (cy - by) - (by - ay) * (cx - bx)
}

/// `poly` with repeated corners and corners on a straight run left out.
fn clean(poly: Vec<P>) -> Vec<P> {
    let mut out = poly;
    loop {
        let len = out.len();
        if len < 3 {
            return out;
        }
        let mut kept = Vec::with_capacity(len);
        for (at, b) in out.iter().enumerate() {
            let a = out.get(at.checked_sub(1).unwrap_or(len.saturating_sub(1)));
            let c = out.get(at.saturating_add(1).checked_rem(len).unwrap_or(0));
            let (Some(a), Some(c)) = (a, c) else {
                continue;
            };
            if a == b {
                continue;
            }
            if b != c && turn(a, b, c).is_zero() {
                continue;
            }
            kept.push(b.clone());
        }
        if kept.len() == len {
            return kept;
        }
        out = kept;
    }
}

/// The part of the convex polygon `poly` where `a . p <= b`.
fn clip(poly: &[P], a: &P, b: &Q) -> Result<Vec<P>, RoofError> {
    let side = |p: &P| &a[0] * &p[0] + &a[1] * &p[1] - b;
    let mut out = Vec::with_capacity(poly.len().saturating_add(1));
    for (p, r) in poly.iter().zip(poly.iter().cycle().skip(1)) {
        let (sp, sr) = (side(p), side(r));
        if !sp.is_positive() {
            out.push(p.clone());
        }
        if (sp.is_negative() && sr.is_positive()) || (sp.is_positive() && sr.is_negative()) {
            let t = div(&sp, &(&sp - &sr))?;
            out.push([&p[0] + (&r[0] - &p[0]) * &t, &p[1] + (&r[1] - &p[1]) * &t]);
        }
    }
    Ok(out)
}

/// The sign of a whole number: -1, 0 or 1.
fn sign(v: &Q) -> i64 {
    if v.is_positive() {
        1
    } else if v.is_negative() {
        -1
    } else {
        0
    }
}

/// The left of an axis direction in the frame.
fn left([u, v]: [i64; 2]) -> [i64; 2] {
    [v.saturating_neg(), u]
}

/// `n . p` for a whole direction `n`.
fn dot(n: [i64; 2], p: &P) -> Q {
    q(i128::from(n[0])) * &p[0] + q(i128::from(n[1])) * &p[1]
}

/// The right-angled frame of a footprint ([the module header](self)).
struct Frame {
    /// The first edge's direction as its smallest whole step.
    u: [i64; 2],
    /// `p^2 + q^2`.
    l2: i128,
}

impl Frame {
    /// The frame point of a plan point.
    fn of(&self, x: i64, y: i64) -> Result<[i128; 2], RoofError> {
        let [p, qq] = self.u.map(i128::from);
        let (x, y) = (i128::from(x), i128::from(y));
        let big = || RoofError::OutOfRange;
        Ok([
            p.checked_mul(x)
                .and_then(|a| a.checked_add(qq.checked_mul(y)?))
                .ok_or_else(big)?,
            p.checked_mul(y)
                .and_then(|a| a.checked_sub(qq.checked_mul(x)?))
                .ok_or_else(big)?,
        ])
    }

    /// The plan point of a frame point, exactly.
    fn plan(&self, [uu, vv]: &P) -> Result<[Q; 2], RoofError> {
        let [p, qq] = self.u.map(|c| q(i128::from(c)));
        let l2 = q(self.l2);
        Ok([
            div(&(&p * uu - &qq * vv), &l2)?,
            div(&(&qq * uu + &p * vv), &l2)?,
        ])
    }

    /// The grid point nearest a frame point, by the module header's rule.
    fn grid(&self, at: &P) -> Result<[i64; 2], RoofError> {
        let [x, y] = self.plan(at)?;
        if self.l2 == 1 {
            Ok([nearest(&x)?, nearest(&y)?])
        } else {
            Ok([nearest_even(&x)?, nearest_even(&y)?])
        }
    }

    /// The step nearest the height of slope `s` at depth `h` in the frame.
    fn height(&self, slope: Slope, h: &Q) -> Result<i64, RoofError> {
        let v = div(
            &(q(i128::from(slope.rise())) * h),
            &q(i128::from(slope.run())),
        )?;
        if self.l2 == 1 {
            nearest(&v)
        } else {
            nearest_over_root(&v, self.l2)
        }
    }
}

/// The smallest whole step along `(dx, dy)`.
fn primitive(dx: i64, dy: i64) -> [i64; 2] {
    let (mut a, mut b) = (dx.unsigned_abs(), dy.unsigned_abs());
    while b != 0 {
        let r = a.checked_rem(b).unwrap_or(0);
        a = b;
        b = r;
    }
    let g = i64::try_from(a).unwrap_or(1).max(1);
    [
        dx.checked_div(g).unwrap_or(0),
        dy.checked_div(g).unwrap_or(0),
    ]
}

/// One edge of the outline in the frame.
#[derive(Clone, Debug)]
struct Edge {
    /// Its inward normal, along an axis of the frame.
    n: [i64; 2],
    /// `n . p` on its footprint edge's line.
    c: Q,
    /// Its slope, if it rises.
    sloped: bool,
}

/// A corner of a piece: where it is in the frame and its depth `h` there (its height is the
/// slope times `h / L`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Node {
    at: P,
    h: Q,
}

/// One piece of a plane, exactly: the footprint edge it rises from and its corners.
#[derive(Clone, Debug)]
struct Exact {
    edge: usize,
    corners: Vec<Node>,
}

/// The roof of `corners` (anticlockwise, each with the edge that starts there and what it does)
/// whose sloped edges all rise at `slope`, as the module header says.
#[allow(
    clippy::too_many_lines,
    reason = "one method in one place: each step reads the one before it, as the module header lists them"
)]
#[allow(
    clippy::many_single_char_names,
    reason = "points and directions of plane geometry, named as the geometry names them"
)]
pub(crate) fn solve(
    corners: &[(Id, Point, EdgeSpec)],
    slope: Slope,
) -> Result<Solution, RoofError> {
    let len = corners.len();
    let ids: Vec<Id> = corners.iter().map(|(id, _, _)| id.clone()).collect();
    let sloping: Vec<Id> = corners
        .iter()
        .filter(|(_, _, spec)| spec.pitch.is_some())
        .map(|(id, _, _)| id.clone())
        .collect();
    let point = |at: usize| {
        corners
            .get(at.checked_rem(len).unwrap_or(0))
            .map(|(_, p, _)| *p)
    };
    // Right-angled: each edge square to the next.
    let mut vectors = Vec::with_capacity(len);
    for at in 0..len {
        let (Some(a), Some(b)) = (point(at), point(at.saturating_add(1))) else {
            return Err(RoofError::OutOfRange);
        };
        vectors.push([b.x.saturating_sub(a.x), b.y.saturating_sub(a.y)]);
    }
    let mut convex = true;
    for (at, [ax, ay]) in vectors.iter().enumerate() {
        let [bx, by] = vectors
            .get(at.saturating_add(1).checked_rem(len).unwrap_or(0))
            .copied()
            .unwrap_or_default();
        let dot = i128::from(*ax)
            .saturating_mul(i128::from(bx))
            .saturating_add(i128::from(*ay).saturating_mul(i128::from(by)));
        if dot != 0 || len < 4 {
            return Err(RoofError::NotRightAngled { edges: sloping });
        }
        let cross = i128::from(*ax)
            .saturating_mul(i128::from(by))
            .saturating_sub(i128::from(*ay).saturating_mul(i128::from(bx)));
        convex &= cross > 0;
    }
    let first = vectors.first().copied().unwrap_or_default();
    let u = if first[0] == 0 || first[1] == 0 {
        [1, 0]
    } else {
        primitive(first[0], first[1])
    };
    let l2 = i128::from(u[0])
        .checked_mul(i128::from(u[0]))
        .and_then(|a| a.checked_add(i128::from(u[1]).checked_mul(i128::from(u[1]))?))
        .ok_or(RoofError::OutOfRange)?;
    let frame = Frame { u, l2 };
    if l2 != 1
        && let Some((id, _, _)) = corners
            .iter()
            .find(|(_, _, spec)| spec.overhang.steps() != 0)
    {
        return Err(RoofError::OverhangOffAxis { edge: id.clone() });
    }
    // The footprint in the frame, and each edge's line.
    let mut at_frame = Vec::with_capacity(len);
    for (_, p, _) in corners {
        let [uu, vv] = frame.of(p.x, p.y)?;
        at_frame.push([q(uu), q(vv)]);
    }
    let mut edges = Vec::with_capacity(len);
    for (at, (_, _, spec)) in corners.iter().enumerate() {
        let (Some(a), Some(b)) = (
            at_frame.get(at),
            at_frame.get(at.saturating_add(1).checked_rem(len).unwrap_or(0)),
        ) else {
            return Err(RoofError::OutOfRange);
        };
        let d = [sign(&(&b[0] - &a[0])), sign(&(&b[1] - &a[1]))];
        let n = left(d);
        edges.push(Edge {
            n,
            c: dot(n, a),
            sloped: spec.pitch.is_some(),
        });
    }
    let overhang = |at: usize| {
        corners
            .get(at)
            .map_or(0, |(_, _, spec)| spec.overhang.steps())
    };
    // The eaves outline: each corner moved out by both its edges' overhangs (an edge along an axis
    // of the frame moves square to itself).
    let mut eaves = Vec::with_capacity(len);
    for at in 0..len {
        let before = at.checked_sub(1).unwrap_or(len.saturating_sub(1));
        let (Some(p), Some(e0), Some(e1)) = (at_frame.get(at), edges.get(before), edges.get(at))
        else {
            return Err(RoofError::OutOfRange);
        };
        let (o0, o1) = (q(i128::from(overhang(before))), q(i128::from(overhang(at))));
        eaves.push([
            &p[0] - &o0 * q(i128::from(e0.n[0])) - &o1 * q(i128::from(e1.n[0])),
            &p[1] - &o0 * q(i128::from(e0.n[1])) - &o1 * q(i128::from(e1.n[1])),
        ]);
    }
    // Each moved edge still runs the way its edge runs: the overhangs do not fold the outline.
    for at in 0..len {
        let (Some(a), Some(b), Some(e)) = (
            eaves.get(at),
            eaves.get(at.saturating_add(1).checked_rem(len).unwrap_or(0)),
            edges.get(at),
        ) else {
            return Err(RoofError::OutOfRange);
        };
        let d = [e.n[1], e.n[0].saturating_neg()];
        if !dot(d, &[&b[0] - &a[0], &b[1] - &a[1]]).is_positive() {
            let id = ids.get(at).cloned().ok_or(RoofError::OutOfRange)?;
            return Err(RoofError::OverhangFolds { edge: id });
        }
    }
    let exact = if convex {
        convex_pieces(&eaves, &edges)?
    } else {
        if let Some(at) = edges.iter().position(|e| !e.sloped) {
            let edge = ids.get(at).cloned().ok_or(RoofError::OutOfRange)?;
            return Err(RoofError::VergeOnReflex { edge });
        }
        let o = overhang(0);
        if let Some(at) = (0..len).find(|&at| overhang(at) != o) {
            let (first, second) = (
                ids.first().cloned().ok_or(RoofError::OutOfRange)?,
                ids.get(at).cloned().ok_or(RoofError::OutOfRange)?,
            );
            return Err(RoofError::OverhangsDiffer { first, second });
        }
        reflex_pieces(&eaves, &edges, &q(i128::from(o)))?
    };
    let lines = lines_of(&exact, &eaves, &edges, &frame, slope)?;
    let mut outline = Vec::with_capacity(len);
    for (id, at) in ids.iter().zip(&eaves) {
        outline.push((id.clone(), frame.grid(at)?));
    }
    let (pieces, twice_area) = on_the_grid(&exact, &frame, slope)?;
    Ok(Solution {
        outline,
        slope,
        pieces,
        twice_area,
        lines,
    })
}

/// A convex footprint's pieces: each sloped plane's face, the eaves outline where that plane is
/// the lowest (the module header's first case).
fn convex_pieces(eaves: &[P], edges: &[Edge]) -> Result<Vec<Exact>, RoofError> {
    let mut out = Vec::new();
    for (i, own) in edges.iter().enumerate() {
        if !own.sloped {
            continue;
        }
        let mut face: Vec<P> = eaves.to_vec();
        for (j, other) in edges.iter().enumerate() {
            if j == i || !other.sloped {
                continue;
            }
            // n_i . p - c_i <= n_j . p - c_j.
            let a = [
                q(i128::from(own.n[0].saturating_sub(other.n[0]))),
                q(i128::from(own.n[1].saturating_sub(other.n[1]))),
            ];
            face = clip(&face, &a, &(&own.c - &other.c))?;
        }
        let face = clean(face);
        if face.len() < 3 || !twice_area_q(&face).is_positive() {
            continue;
        }
        let corners = face
            .into_iter()
            .map(|at| {
                let h = dot(own.n, &at) - &own.c;
                Node { at, h }
            })
            .collect();
        out.push(Exact { edge: i, corners });
    }
    Ok(out)
}

/// A rectangle in the frame: `[u0, u1, v0, v1]`.
type Rect = [Q; 4];

/// Every maximal rectangle inside the right-angled polygon `poly`: the rectangles of its cells
/// that grow no further on any side.
fn maximal_rectangles(poly: &[P]) -> Vec<Rect> {
    let mut us: Vec<Q> = poly.iter().map(|[u, _]| u.clone()).collect();
    let mut vs: Vec<Q> = poly.iter().map(|[_, v]| v.clone()).collect();
    us.sort();
    us.dedup();
    vs.sort();
    vs.dedup();
    let (nu, nv) = (us.len().saturating_sub(1), vs.len().saturating_sub(1));
    let half = Q::new(BigInt::from(1), BigInt::from(2));
    let mut inside = vec![vec![false; nv]; nu];
    for (i, row) in inside.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            let (Some(u0), Some(u1), Some(v0), Some(v1)) = (
                us.get(i),
                us.get(i.saturating_add(1)),
                vs.get(j),
                vs.get(j.saturating_add(1)),
            ) else {
                continue;
            };
            let centre = [(u0 + u1) * &half, (v0 + v1) * &half];
            *cell = contains(poly, &centre);
        }
    }
    let cell = |i: usize, j: usize| {
        inside
            .get(i)
            .and_then(|row| row.get(j))
            .copied()
            .unwrap_or(false)
    };
    let full = |i0: usize, i1: usize, j0: usize, j1: usize| {
        (i0..=i1).all(|i| (j0..=j1).all(|j| cell(i, j)))
    };
    let mut out = Vec::new();
    for i0 in 0..nu {
        for i1 in i0..nu {
            for j0 in 0..nv {
                for j1 in j0..nv {
                    if !full(i0, i1, j0, j1) {
                        continue;
                    }
                    let grows = (i0 > 0
                        && full(i0.saturating_sub(1), i0.saturating_sub(1), j0, j1))
                        || (i1.saturating_add(1) < nu
                            && full(i1.saturating_add(1), i1.saturating_add(1), j0, j1))
                        || (j0 > 0 && full(i0, i1, j0.saturating_sub(1), j0.saturating_sub(1)))
                        || (j1.saturating_add(1) < nv
                            && full(i0, i1, j1.saturating_add(1), j1.saturating_add(1)));
                    if grows {
                        continue;
                    }
                    if let (Some(u0), Some(u1), Some(v0), Some(v1)) = (
                        us.get(i0),
                        us.get(i1.saturating_add(1)),
                        vs.get(j0),
                        vs.get(j1.saturating_add(1)),
                    ) {
                        out.push([u0.clone(), u1.clone(), v0.clone(), v1.clone()]);
                    }
                }
            }
        }
    }
    out
}

/// Whether `p`, on no edge of `poly`, is inside it: a ray to +u crossing its edges across v.
fn contains(poly: &[P], p: &P) -> bool {
    let mut inside = false;
    for (a, b) in poly.iter().zip(poly.iter().cycle().skip(1)) {
        let (lo, hi) = if a[1] < b[1] { (a, b) } else { (b, a) };
        if lo[1] <= p[1] && p[1] < hi[1] && a[0] == b[0] && a[0] > p[0] {
            inside = !inside;
        }
    }
    inside
}

/// A loop of the wavefront: each corner and the way it moves as the depth grows by one.
type Front = Vec<(P, [i64; 2])>;

/// The wavefront at depth `t`: the boundary of the union of `rects` each shrunk by `t`, as loops
/// anticlockwise, each corner with its velocity.
#[allow(
    clippy::too_many_lines,
    reason = "one method in one place: each step reads the one before it, as the module header lists them"
)]
fn front(rects: &[Rect], t: &Q) -> Result<Vec<Front>, RoofError> {
    let shrunk: Vec<Rect> = rects
        .iter()
        .map(|[u0, u1, v0, v1]| [u0 + t, u1 - t, v0 + t, v1 - t])
        .filter(|[u0, u1, v0, v1]| u0 < u1 && v0 < v1)
        .collect();
    let mut us: Vec<Q> = shrunk
        .iter()
        .flat_map(|[u0, u1, _, _]| [u0.clone(), u1.clone()])
        .collect();
    let mut vs: Vec<Q> = shrunk
        .iter()
        .flat_map(|[_, _, v0, v1]| [v0.clone(), v1.clone()])
        .collect();
    us.sort();
    us.dedup();
    vs.sort();
    vs.dedup();
    let (nu, nv) = (us.len().saturating_sub(1), vs.len().saturating_sub(1));
    let covered = |i: usize, j: usize| -> bool {
        let (Some(u0), Some(u1), Some(v0), Some(v1)) = (
            us.get(i),
            us.get(i.saturating_add(1)),
            vs.get(j),
            vs.get(j.saturating_add(1)),
        ) else {
            return false;
        };
        shrunk
            .iter()
            .any(|[a0, a1, b0, b1]| a0 <= u0 && u1 <= a1 && b0 <= v0 && v1 <= b1)
    };
    // Every side of a covered cell beside an uncovered one, interior on its left.
    let mut sides: BTreeMap<P, Vec<P>> = BTreeMap::new();
    for i in 0..nu {
        for j in 0..nv {
            if !covered(i, j) {
                continue;
            }
            let (Some(u0), Some(u1), Some(v0), Some(v1)) = (
                us.get(i),
                us.get(i.saturating_add(1)),
                vs.get(j),
                vs.get(j.saturating_add(1)),
            ) else {
                continue;
            };
            let corner = |u: &Q, v: &Q| [u.clone(), v.clone()];
            let below = j == 0 || !covered(i, j.saturating_sub(1));
            let right = !covered(i.saturating_add(1), j);
            let above = !covered(i, j.saturating_add(1));
            let beside = i == 0 || !covered(i.saturating_sub(1), j);
            for (on, from, to) in [
                (below, corner(u0, v0), corner(u1, v0)),
                (right, corner(u1, v0), corner(u1, v1)),
                (above, corner(u1, v1), corner(u0, v1)),
                (beside, corner(u0, v1), corner(u0, v0)),
            ] {
                if on {
                    sides.entry(from).or_default().push(to);
                }
            }
        }
    }
    let mut loops = Vec::new();
    while let Some((start, _)) = sides.iter().next().map(|(k, v)| (k.clone(), v.clone())) {
        let mut chain = Vec::new();
        let mut at = start.clone();
        loop {
            let nexts = sides.remove(&at).unwrap_or_default();
            let [next] = nexts.as_slice() else {
                return Err(RoofError::NotSolved);
            };
            chain.push(at.clone());
            at.clone_from(next);
            if at == start {
                break;
            }
        }
        let chain = clean(chain);
        let count = chain.len();
        let mut moving = Vec::with_capacity(count);
        for (k, p) in chain.iter().enumerate() {
            let (Some(a), Some(c)) = (
                chain.get(k.checked_sub(1).unwrap_or(count.saturating_sub(1))),
                chain.get(k.saturating_add(1).checked_rem(count).unwrap_or(0)),
            ) else {
                return Err(RoofError::NotSolved);
            };
            let d_in = [sign(&(&p[0] - &a[0])), sign(&(&p[1] - &a[1]))];
            let d_out = [sign(&(&c[0] - &p[0])), sign(&(&c[1] - &p[1]))];
            let (l_in, l_out) = (left(d_in), left(d_out));
            moving.push((
                p.clone(),
                [
                    l_in[0].saturating_add(l_out[0]),
                    l_in[1].saturating_add(l_out[1]),
                ],
            ));
        }
        loops.push(moving);
    }
    Ok(loops)
}

/// `p` moved by `by` times `vel`.
fn moved(p: &P, vel: [i64; 2], by: &Q) -> P {
    [
        &p[0] + q(i128::from(vel[0])) * by,
        &p[1] + q(i128::from(vel[1])) * by,
    ]
}

/// The wavefront `loops`, taken at depth `from`, moved to depth `to`, each loop starting at its
/// least corner and the loops in order: the form two fronts are compared in.
fn normal(loops: &[Front], from: &Q, to: &Q) -> Vec<Vec<P>> {
    let by = to - from;
    let mut out: Vec<Vec<P>> = loops
        .iter()
        .map(|ring| {
            let points: Vec<P> = ring.iter().map(|(p, vel)| moved(p, *vel, &by)).collect();
            let least = points
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.cmp(b.1))
                .map_or(0, |(k, _)| k);
            let mut turned = points.clone();
            turned.rotate_left(least);
            turned
        })
        .collect();
    out.sort();
    out
}

/// A right-angled footprint's pieces, every edge sloping, `o` its one overhang (the module
/// header's second case): the trapezoids each wavefront edge sweeps between two depths where the
/// wavefront changes.
#[allow(
    clippy::too_many_lines,
    reason = "one method in one place: each step reads the one before it, as the module header lists them"
)]
fn reflex_pieces(eaves: &[P], edges: &[Edge], o: &Q) -> Result<Vec<Exact>, RoofError> {
    let rects = maximal_rectangles(eaves);
    let two = q(2);
    let mut depths: BTreeSet<Q> = BTreeSet::new();
    depths.insert(Q::zero());
    let mut deepest = Q::zero();
    for [u0, u1, v0, v1] in &rects {
        let narrow = if u1 - u0 < v1 - v0 { u1 - u0 } else { v1 - v0 };
        let half = div(&narrow, &two)?;
        if half > deepest {
            deepest = half;
        }
    }
    for [a0, _, b0, _] in &rects {
        for [_, a1, _, b1] in &rects {
            for t in [div(&(a1 - a0), &two)?, div(&(b1 - b0), &two)?] {
                if t.is_positive() && t < deepest {
                    depths.insert(t);
                }
            }
        }
    }
    depths.insert(deepest);
    let depths: Vec<Q> = depths.into_iter().collect();
    // Bands between two depths where the wavefront changes, each with its wavefront in between.
    let mut bands: Vec<(Q, Q, Q, Vec<Front>)> = Vec::new();
    for (lo, hi) in depths.iter().zip(depths.iter().skip(1)) {
        let mid = div(&(lo + hi), &two)?;
        let loops = front(&rects, &mid)?;
        if let Some((_, end, at, kept)) = bands.last_mut()
            && normal(kept, at, &mid) == normal(&loops, &mid, &mid)
        {
            *end = hi.clone();
            continue;
        }
        bands.push((lo.clone(), hi.clone(), mid, loops));
    }
    // The line each outline edge lies on: its normal and `n . p`.
    let own: Vec<([i64; 2], Q)> = eaves
        .iter()
        .zip(edges)
        .map(|(p, e)| (e.n, dot(e.n, p)))
        .collect();
    let mut out: Vec<Exact> = Vec::new();
    // The pieces of the band before, each with its line, its edge and its top side.
    let mut before: Vec<Traced> = Vec::new();
    for (lo, hi, at, loops) in &bands {
        let mut this = Vec::new();
        for ring in loops {
            let count = ring.len();
            for (k, (p, vel)) in ring.iter().enumerate() {
                let Some((r, rvel)) = ring.get(k.saturating_add(1).checked_rem(count).unwrap_or(0))
                else {
                    return Err(RoofError::NotSolved);
                };
                let d = [sign(&(&r[0] - &p[0])), sign(&(&r[1] - &p[1]))];
                let n = left(d);
                let line = (n, dot(n, p) - at);
                let (low, high) = (lo - at, hi - at);
                let (a_lo, b_lo) = (moved(p, *vel, &low), moved(r, *rvel, &low));
                let (a_hi, b_hi) = (moved(p, *vel, &high), moved(r, *rvel, &high));
                let overlaps = |[s0, s1]: &[P; 2], [t0, t1]: &[P; 2]| {
                    let along = |x: &P| dot(d, x);
                    let (lo0, hi0) = ordered(along(s0), along(s1));
                    let (lo1, hi1) = ordered(along(t0), along(t1));
                    (if lo0 > lo1 { lo0 } else { lo1 }) <= (if hi0 < hi1 { hi0 } else { hi1 })
                };
                let bottom = [a_lo.clone(), b_lo.clone()];
                let edge = if lo.is_zero() {
                    own.iter().enumerate().find_map(|(at, l)| {
                        let next = eaves
                            .get(at.saturating_add(1).checked_rem(eaves.len()).unwrap_or(0))?;
                        (*l == line && overlaps(&bottom, &[eaves.get(at)?.clone(), next.clone()]))
                            .then_some(at)
                    })
                } else {
                    before
                        .iter()
                        .find(|(l, _, top)| *l == line && overlaps(&bottom, top))
                        .map(|(_, edge, _)| *edge)
                };
                let edge = edge
                    .or_else(|| own.iter().position(|l| *l == line))
                    .ok_or(RoofError::NotSolved)?;
                let corners: Vec<Node> = [
                    (a_lo, lo),
                    (b_lo, lo),
                    (b_hi.clone(), hi),
                    (a_hi.clone(), hi),
                ]
                .into_iter()
                .map(|(at, t)| Node { at, h: t - o })
                .collect();
                let mut kept: Vec<Node> = Vec::with_capacity(4);
                for node in corners {
                    if kept.last().is_none_or(|last| last.at != node.at) {
                        kept.push(node);
                    }
                }
                if kept.len() > 1 && kept.first().map(|n| &n.at) == kept.last().map(|n| &n.at) {
                    kept.pop();
                }
                this.push((line, edge, [b_hi, a_hi]));
                if kept.len() >= 3 {
                    out.push(Exact {
                        edge,
                        corners: kept,
                    });
                }
            }
        }
        before = this;
    }
    Ok(out)
}

/// `(a, b)` least first.
fn ordered(a: Q, b: Q) -> (Q, Q) {
    if a <= b { (a, b) } else { (b, a) }
}

/// The lines of the exact pieces: where two pieces of different planes share a side (a ridge,
/// hip or valley) and where a piece's side lies on the eaves outline (an eave or a verge), each
/// joined with the next of its kind on one straight line, rounded onto the grid.
#[allow(
    clippy::too_many_lines,
    reason = "one method in one place: each step reads the one before it, as the module header lists them"
)]
fn lines_of(
    exact: &[Exact],
    eaves: &[P],
    edges: &[Edge],
    frame: &Frame,
    slope: Slope,
) -> Result<Vec<Line>, RoofError> {
    let mut sides: BTreeMap<(Node, Node), usize> = BTreeMap::new();
    for (at, piece) in exact.iter().enumerate() {
        for (a, b) in piece
            .corners
            .iter()
            .zip(piece.corners.iter().cycle().skip(1))
        {
            sides.insert((a.clone(), b.clone()), at);
        }
    }
    let mut found: Vec<(LineKind, Node, Node)> = Vec::new();
    for ((a, b), at) in &sides {
        let Some(piece) = exact.get(*at) else {
            continue;
        };
        let Some(own) = edges.get(piece.edge) else {
            continue;
        };
        if let Some(other) = sides.get(&(b.clone(), a.clone())) {
            let Some(other_edge) = exact.get(*other).and_then(|p| edges.get(p.edge)) else {
                continue;
            };
            if at > other || own.n == other_edge.n {
                continue;
            }
            // The piece lies to the left of a -> b; on that side its plane is the lower of the two
            // where the fold is convex.
            let ln = [&a.at[1] - &b.at[1], &b.at[0] - &a.at[0]];
            let g = [
                q(i128::from(own.n[0].saturating_sub(other_edge.n[0]))),
                q(i128::from(own.n[1].saturating_sub(other_edge.n[1]))),
            ];
            let convex = (&g[0] * &ln[0] + &g[1] * &ln[1]).is_negative();
            let opposite = own.n
                == [
                    other_edge.n[0].saturating_neg(),
                    other_edge.n[1].saturating_neg(),
                ];
            let kind = match (opposite, convex) {
                (true, true) => LineKind::Ridge,
                (false, true) => LineKind::Hip,
                _ => LineKind::Valley,
            };
            found.push((kind, a.clone(), b.clone()));
            continue;
        }
        // A side on the outline: which edge's line holds it, within the edge.
        let count = eaves.len();
        for (k, e) in edges.iter().enumerate() {
            let (Some(p0), Some(p1)) = (
                eaves.get(k),
                eaves.get(k.saturating_add(1).checked_rem(count).unwrap_or(0)),
            ) else {
                continue;
            };
            let on = |x: &P| turn(p0, x, p1).is_zero() && dot(e.n, x) == dot(e.n, p0);
            if on(&a.at) && on(&b.at) {
                let kind = if e.sloped {
                    LineKind::Eave
                } else {
                    LineKind::Verge
                };
                found.push((kind, a.clone(), b.clone()));
                break;
            }
        }
    }
    // Join each line with the next of its kind that carries it straight on.
    let straight = |a: &Node, b: &Node, c: &Node| {
        let d1 = [&b.at[0] - &a.at[0], &b.at[1] - &a.at[1], &b.h - &a.h];
        let d2 = [&c.at[0] - &b.at[0], &c.at[1] - &b.at[1], &c.h - &b.h];
        (&d1[0] * &d2[1] - &d1[1] * &d2[0]).is_zero()
            && (&d1[1] * &d2[2] - &d1[2] * &d2[1]).is_zero()
            && (&d1[0] * &d2[2] - &d1[2] * &d2[0]).is_zero()
    };
    let mut joined = true;
    while joined {
        joined = false;
        'outer: for i in 0..found.len() {
            for j in 0..found.len() {
                if i == j {
                    continue;
                }
                let (Some((ki, ai, bi)), Some((kj, aj, bj))) = (found.get(i), found.get(j)) else {
                    continue;
                };
                if ki != kj {
                    continue;
                }
                let line = if bi == aj && straight(ai, bi, bj) {
                    Some((*ki, ai.clone(), bj.clone()))
                } else if bi == bj && straight(ai, bi, aj) {
                    Some((*ki, ai.clone(), aj.clone()))
                } else if ai == aj && straight(bi, ai, bj) {
                    Some((*ki, bi.clone(), bj.clone()))
                } else {
                    None
                };
                if let Some(line) = line {
                    let (lo, hi) = (i.min(j), i.max(j));
                    found.remove(hi);
                    found.remove(lo);
                    found.push(line);
                    joined = true;
                    break 'outer;
                }
            }
        }
    }
    let mut out = Vec::with_capacity(found.len());
    for (kind, a, b) in found {
        let end = |n: &Node| -> Result<[i64; 3], RoofError> {
            let [x, y] = frame.grid(&n.at)?;
            Ok([x, y, frame.height(slope, &n.h)?])
        };
        let (from, to) = (end(&a)?, end(&b)?);
        let (from, to) = if from <= to { (from, to) } else { (to, from) };
        out.push(Line { kind, from, to });
    }
    out.sort_by_key(|line| (line.kind, line.from, line.to));
    out.dedup();
    Ok(out)
}

/// Whether the corners of `poly` lie on one plane.
fn flat(poly: &[[i64; 3]]) -> bool {
    let wide = |p: &[i64; 3]| p.map(BigInt::from);
    let (Some(a), Some(b)) = (poly.first().map(wide), poly.get(1).map(wide)) else {
        return true;
    };
    let cross = |x: &[BigInt; 3], y: &[BigInt; 3]| -> [BigInt; 3] {
        [
            &x[1] * &y[2] - &x[2] * &y[1],
            &x[2] * &y[0] - &x[0] * &y[2],
            &x[0] * &y[1] - &x[1] * &y[0],
        ]
    };
    let ab = [&b[0] - &a[0], &b[1] - &a[1], &b[2] - &a[2]];
    let mut normal: Option<[BigInt; 3]> = None;
    for c in poly.iter().skip(2).map(wide) {
        let ac = [&c[0] - &a[0], &c[1] - &a[1], &c[2] - &a[2]];
        let n = cross(&ab, &ac);
        if n.iter().all(Zero::is_zero) {
            continue;
        }
        match &normal {
            None => normal = Some(n),
            Some(m) => {
                if !cross(&n, m).iter().all(Zero::is_zero) {
                    return false;
                }
            }
        }
    }
    true
}

/// The exact pieces rounded onto the grid, each flat or cut into flat triangles, in an order where
/// each shares a side with one before it; and twice their plan area, summed.
fn on_the_grid(
    exact: &[Exact],
    frame: &Frame,
    slope: Slope,
) -> Result<(Vec<Piece>, i128), RoofError> {
    let mut pieces: Vec<Piece> = Vec::new();
    for piece in exact {
        let mut corners: Vec<[i64; 3]> = Vec::with_capacity(piece.corners.len());
        for node in &piece.corners {
            let [x, y] = frame.grid(&node.at)?;
            let corner = [x, y, frame.height(slope, &node.h)?];
            if corners.last() != Some(&corner) {
                corners.push(corner);
            }
        }
        if corners.len() > 1 && corners.first() == corners.last() {
            corners.pop();
        }
        if corners.len() < 3 {
            continue;
        }
        let twice = twice_area(&corners)?;
        if twice == 0 {
            continue;
        }
        if twice < 0 {
            return Err(RoofError::NotExact);
        }
        if flat(&corners) {
            pieces.push((piece.edge, without_straight_runs(&corners)));
            continue;
        }
        let Some((&first, rest)) = corners.split_first() else {
            continue;
        };
        for pair in rest.windows(2) {
            let [b, c] = pair else {
                continue;
            };
            let triangle = vec![first, *b, *c];
            let twice = twice_area(&triangle)?;
            if twice < 0 {
                return Err(RoofError::NotExact);
            }
            if twice > 0 {
                pieces.push((piece.edge, triangle));
            }
        }
    }
    let mut total = 0_i128;
    for (_, corners) in &pieces {
        total = total
            .checked_add(twice_area(corners)?)
            .ok_or(RoofError::OutOfRange)?;
    }
    Ok((in_union_order(pieces)?, total))
}

/// A flat polygon with each corner on a straight run between its neighbours (in plan) left out.
fn without_straight_runs(corners: &[[i64; 3]]) -> Vec<[i64; 3]> {
    let len = corners.len();
    let mut out = Vec::with_capacity(len);
    for (at, b) in corners.iter().enumerate() {
        let (Some(a), Some(c)) = (
            corners.get(at.checked_sub(1).unwrap_or(len.saturating_sub(1))),
            corners.get(at.saturating_add(1).checked_rem(len).unwrap_or(0)),
        ) else {
            continue;
        };
        let cross = (i128::from(b[0]) - i128::from(a[0])) * (i128::from(c[1]) - i128::from(b[1]))
            - (i128::from(b[1]) - i128::from(a[1])) * (i128::from(c[0]) - i128::from(b[0]));
        if cross != 0 {
            out.push(*b);
        }
    }
    out
}

/// Whether two pieces share a stretch of a side in plan: a side of each on one line, running
/// opposite ways, overlapping by more than a point.
#[allow(
    clippy::many_single_char_names,
    reason = "points and directions of plane geometry, named as the geometry names them"
)]
fn beside(a: &[[i64; 3]], b: &[[i64; 3]]) -> bool {
    let w = |p: &[i64; 3]| [i128::from(p[0]), i128::from(p[1])];
    for (p0, p1) in a.iter().zip(a.iter().cycle().skip(1)) {
        let (p0, p1) = (w(p0), w(p1));
        let d = [p1[0] - p0[0], p1[1] - p0[1]];
        for (r0, r1) in b.iter().zip(b.iter().cycle().skip(1)) {
            let (r0, r1) = (w(r0), w(r1));
            let e = [r1[0] - r0[0], r1[1] - r0[1]];
            let on = |x: [i128; 2]| d[0] * (x[1] - p0[1]) - d[1] * (x[0] - p0[0]) == 0;
            if !(on(r0) && on(r1)) || d[0] * e[0] + d[1] * e[1] >= 0 {
                continue;
            }
            let along = |x: [i128; 2]| d[0] * (x[0] - p0[0]) + d[1] * (x[1] - p0[1]);
            let (a0, a1) = (0, along(p1));
            let (b0, b1) = (along(r1), along(r0));
            if a0.max(b0) < a1.min(b1) {
                return true;
            }
        }
    }
    false
}

/// The pieces in an order where each after the first shares a side with one before it, the
/// least-numbered such piece first each time, so a layer's union grows as one solid.
fn in_union_order(pieces: Vec<Piece>) -> Result<Vec<Piece>, RoofError> {
    let count = pieces.len();
    let mut taken = vec![false; count];
    let mut order: Vec<usize> = Vec::with_capacity(count);
    if count > 0 {
        order.push(0);
        if let Some(first) = taken.first_mut() {
            *first = true;
        }
    }
    while order.len() < count {
        let next = (0..count).find(|&k| {
            !taken.get(k).copied().unwrap_or(true)
                && pieces.get(k).is_some_and(|(_, p)| {
                    order
                        .iter()
                        .any(|&o| pieces.get(o).is_some_and(|(_, r)| beside(p, r)))
                })
        });
        let Some(next) = next else {
            return Err(RoofError::NotSolved);
        };
        if let Some(flag) = taken.get_mut(next) {
            *flag = true;
        }
        order.push(next);
    }
    let mut slots: Vec<Option<Piece>> = pieces.into_iter().map(Some).collect();
    Ok(order
        .into_iter()
        .filter_map(|k| slots.get_mut(k).and_then(Option::take))
        .collect())
}

/// The ridge of a gable on a footprint `wide` steps across from its first eaves edge (at `a = 0`)
/// to its far one, rising at `first` from the first and at `second` from the far, with each face
/// stood `up` and `far_up` steps above the bottom face upright on each slope: where the two faces
/// meet across the roof and how high, `(a, z)`, each the nearest step, a half up. With one slope and
/// one height it is midway, `round(wide / 2)`, at `round(s wide / 2) + up`.
pub(crate) fn gable_ridge(
    first: Slope,
    second: Slope,
    wide: i64,
    up: i64,
    far_up: i64,
) -> Result<(i64, i64), RoofError> {
    let s = |slope: Slope| div(&q(i128::from(slope.rise())), &q(i128::from(slope.run())));
    let (s1, s2) = (s(first)?, s(second)?);
    let (w, u1, u2) = (
        q(i128::from(wide)),
        q(i128::from(up)),
        q(i128::from(far_up)),
    );
    // s1 a + u1 = s2 (w - a) + u2.
    let a = div(&(&s2 * &w + &u2 - &u1), &(&s1 + &s2))?;
    let z = &s1 * &a + &u1;
    Ok((nearest(&a)?, nearest(&z)?))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]
mod tests {
    use super::*;

    /// Halves round up, either side of zero; a turned height rounds by square roots exactly.
    #[test]
    fn rounding_is_one_rule() {
        let r = |n: i64, d: i64| Q::new(BigInt::from(n), BigInt::from(d));
        assert_eq!(nearest(&r(1, 2)).unwrap(), 1);
        assert_eq!(nearest(&r(-1, 2)).unwrap(), 0);
        assert_eq!(nearest(&r(-3, 2)).unwrap(), -1);
        assert_eq!(nearest_even(&r(3, 1)).unwrap(), 4);
        assert_eq!(nearest_even(&r(5, 2)).unwrap(), 2);
        // 10 / sqrt(25) = 2; 7 / sqrt(2) = 4.95; 15 / sqrt(4) = 7.5 goes up.
        assert_eq!(nearest_over_root(&r(10, 1), 25).unwrap(), 2);
        assert_eq!(nearest_over_root(&r(7, 1), 2).unwrap(), 5);
        assert_eq!(nearest_over_root(&r(15, 1), 4).unwrap(), 8);
        assert_eq!(nearest_over_root(&r(0, 1), 2).unwrap(), 0);
    }

    /// An L's two maximal rectangles are its two arms run through.
    #[test]
    fn an_l_has_two_maximal_rectangles() {
        let p = |u: i128, v: i128| [q(u), q(v)];
        let l = [p(0, 0), p(8, 0), p(8, 4), p(4, 4), p(4, 8), p(0, 8)];
        let rects = maximal_rectangles(&l);
        assert_eq!(rects.len(), 2);
        assert!(rects.contains(&[q(0), q(8), q(0), q(4)]));
        assert!(rects.contains(&[q(0), q(4), q(0), q(8)]));
        let at_one = front(&rects, &q(1)).unwrap();
        assert_eq!(at_one.len(), 1);
        assert_eq!(at_one[0].len(), 6);
    }
}
