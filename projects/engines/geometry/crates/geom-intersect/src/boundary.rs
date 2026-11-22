//! An edge of a patch that lies in the other surface: a branch that runs along a domain's end
//! (G173a, chapter 12.8 step 5).
//!
//! What it does: [`lies_in`] takes an edge of one surface, the curve `S₁(t)` with one parameter
//! held at an end of its domain, and the other surface it may lie in, and proves it so over a
//! stretch of `t`, piece by piece. Where it does, the edge is the branch: its system against the
//! other surface has a zero for every `t`, so the interior engine of [`crate::solve`] can never
//! prove one and flags boxes the whole length of it, and the march ([`crate::march`]) walks the
//! edge itself, since a box around a point of it cannot stand in the domain.
//!
//! Guarantees:
//!
//! - **A piece is proven for every `t` in it.** Two of the edge system's three equations are
//!   solved in the other surface's `(s, r)` with `t` an interval: Krawczyk's operator (chapter
//!   12.3) with the parameter in the residual, so one box holds exactly one solution for each `t`
//!   of the piece. The pair of equations is the one whose partials determine `(s, r)` best.
//! - **The third equation holds to within the floor.** Its residual over the proven box is
//!   enclosed, and must lie within an eighth of the floor of zero: the edge lies in the other
//!   surface to within the resolution floor, the standard every contact is held to (12.9).
//! - **A piece stays within the ceiling.** The edge's chord over a piece leaves the edge by at most
//!   the piece's length times the width of the edge's velocity over it, which a piece is halved to
//!   keep under the ceiling.
//! - **Nothing is guessed.** A piece that cannot be proven is halved, down to a 4096th of the
//!   stretch, and then the whole stretch is not proven: the pair stays flagged or unproven.
//!
//! Ownership: the caller takes the first surface's edge before the second's, so where two
//! patches share a border that is an edge of both, the branch along it is traced once.
//!
//! Method: the parametric Krawczyk test is Kearfott and Xing's
//! (<https://doi.org/10.1137/0731048>), the one [`crate::march`] steps with; the least-squares
//! start for the middle of a piece is Gauss-Newton on the three residuals. The box of a piece is
//! one length on the other surface, a little more than the edge's travel over the piece, and each
//! of `(s, r)` takes it in its own unit, by the surface's speed along that parameter, as the
//! march's step box does: a turn and a height, or a plane's two vectors of different lengths, are
//! not one unit.
//!
//! Cost: a piece takes a handful of reads of both surfaces; a straight edge takes one piece where
//! the piece's box stays inside the other surface's domain and the room past its ends, and is
//! halved until it does where it does not. All of it is charged to the meter. Recursion: none.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are chapter 12.8's: t along the edge, s and r the other surface's parameters"
)]

use alloc::vec::Vec;

use geom_curve::tight::{larger, smaller};
use geom_num::interval::Interval;
use geom_num::newton::inverse;
use geom_run::WorkMeter;
use geom_surface::ParamSurface;

use crate::IntersectError;
use crate::general::{Edge, partials, point};
use crate::solve::{Field, strictly_inside};

/// How many times a stretch may be halved: down to a 4096th of it, so at most 4096 pieces.
const DEPTH: u32 = 12;

/// The share of the floor that the third equation's residual may reach.
const SHARE: f64 = 8.0;

/// Gauss-Newton steps for the middle of a piece: past what a double holds from any start.
const NEWTON: usize = 12;

/// Radii tried for the box of one piece, each twice the last.
const TRIES: usize = 10;

/// The rows the edge system could keep, and the row each leaves out to be checked.
const ROWS: [([usize; 2], usize); 3] = [([0, 1], 2), ([0, 2], 1), ([1, 2], 0)];

/// An edge proven to lie in the other surface over a stretch of its parameter.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Run {
    /// Where the pieces end: the stretch's start, the cuts between pieces, its end.
    pub(crate) cuts: Vec<f64>,
    /// The other surface's `(s, r)` over each piece, one box holding the one solution for each `t`.
    pub(crate) pieces: Vec<[Interval; 2]>,
    /// The other surface's `(s, r)` at each cut: the box of the piece before and the one after, met.
    pub(crate) at: Vec<[Interval; 2]>,
    /// A bound, in space, on how far the edge is from the chords through the cuts.
    pub(crate) deviation: f64,
}

impl Run {
    /// Whether the box `[t, s, r]` of the edge system meets the run: some piece covering part of
    /// its `t` has a box that meets its `s` and `r`. A zero of the system in that box is the run's.
    pub(crate) fn holds(&self, region: &[Interval; 3]) -> bool {
        let [t, s, r] = *region;
        let meets = |a: Interval, b: Interval| a.lo() <= b.hi() && b.lo() <= a.hi();
        self.cuts
            .windows(2)
            .zip(&self.pieces)
            .any(|(cut, [ps, pr])| match cut {
                [a, b] => t.lo() <= *b && *a <= t.hi() && meets(s, *ps) && meets(r, *pr),
                _ => false,
            })
    }
}

/// A read that failed for a box outside a kind's domain, which it refuses, as one that cannot be
/// bounded.
fn soft<T>(read: Result<Option<T>, IntersectError>) -> Result<Option<T>, IntersectError> {
    match read {
        Err(IntersectError::InvalidInput) => Ok(None),
        other => other,
    }
}

/// What has been proven of one edge so far: whether the proof was tried, and the run it gave, over
/// the edge and over the edge with the room past its ends.
#[derive(Default)]
pub(crate) struct Lying {
    /// Whether [`lies_in`] has been run for the edge.
    pub(crate) tried: bool,
    /// The run, if it proved the edge lies in the other surface.
    pub(crate) run: Option<Run>,
    /// Whether the run over the room past the edge's ends has been tried.
    pub(crate) widened: bool,
    /// That run, if it proved the edge, continued past its ends, lies in the other surface.
    pub(crate) wide: Option<Run>,
}

/// The middle of an interval.
fn mid(x: Interval) -> f64 {
    x.lo() / 2.0 + x.hi() / 2.0
}

/// The largest absolute value in an interval.
fn most(x: Interval) -> f64 {
    larger(x.lo().abs(), x.hi().abs())
}

/// The two solved rows of three, as indexes, by how well their partials determine `(s, r)`, and the
/// row left over.
fn best_rows(columns: [[f64; 2]; 3]) -> Option<([usize; 2], usize)> {
    let [[a0, b0], [a1, b1], [a2, b2]] = columns;
    let dets = [a0 * b1 - b0 * a1, a0 * b2 - b0 * a2, a1 * b2 - b1 * a2];
    let mut best: Option<(f64, ([usize; 2], usize))> = None;
    for (det, choice) in dets.iter().zip(ROWS) {
        let size = det.abs();
        if size.is_finite() && size > 0.0 && best.is_none_or(|(b, _)| size > b) {
            best = Some((size, choice));
        }
    }
    best.map(|(_, choice)| choice)
}

/// The `(s, r)` columns of the edge system's partials, by the middle of each.
fn columns(rows: &[[Interval; 3]; 3]) -> [[f64; 2]; 3] {
    rows.map(|row| {
        let [_, s, r] = row;
        [mid(s), mid(r)]
    })
}

/// How many of `r`'s units one of `s`'s is worth as a length on the other surface: its speed along
/// `s` over its speed along `r`, by the partials `slope` holds. A cylinder's turn against its
/// height, or a plane whose two vectors differ in length, has parameters in different units, and a
/// length is the same number in both only where this is 1.
fn pace(slope: &[[Interval; 3]; 3]) -> Result<f64, IntersectError> {
    let [[_, s0, r0], [_, s1, r1], [_, s2, r2]] = *slope;
    let along_s = Interval::norm(&[s0, s1, s2])?.hi();
    let along_r = Interval::norm(&[r0, r1, r2])?.hi();
    Ok(along_s / along_r)
}

/// The `(s, r)` at the middle of `t`, by Gauss-Newton on the three residuals, from the middle of
/// the other surface's domain; `None` where it does not settle or is not defined.
fn middle(
    edge: &Edge<'_>,
    tm: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<[f64; 2]>, IntersectError> {
    let ((s0, s1), (r0, r1)) = edge.other.domain();
    let (mut ms, mut mr) = (s0 / 2.0 + s1 / 2.0, r0 / 2.0 + r1 / 2.0);
    for _ in 0..NEWTON {
        let whole = [point(tm)?, point(ms)?, point(mr)?];
        let (Some(f), Some(j)) = (
            soft(edge.over(&whole, meter))?,
            soft(edge.slope(&whole, meter))?,
        ) else {
            return Ok(None);
        };
        let [f0, f1, f2] = f.map(mid);
        let [[a0, b0], [a1, b1], [a2, b2]] = columns(&j);
        let (aa, ab, bb) = (
            a0 * a0 + a1 * a1 + a2 * a2,
            a0 * b0 + a1 * b1 + a2 * b2,
            b0 * b0 + b1 * b1 + b2 * b2,
        );
        let (ga, gb) = (a0 * f0 + a1 * f1 + a2 * f2, b0 * f0 + b1 * f1 + b2 * f2);
        let Some([[y00, y01], [y10, y11]]) = inverse([[aa, ab], [ab, bb]]) else {
            return Ok(None);
        };
        // A step past the doubles settles nowhere: a box of a point is refused for either.
        let (Ok(ns), Ok(nr)) = (
            point(ms - (y00 * ga + y01 * gb)),
            point(mr - (y10 * ga + y11 * gb)),
        ) else {
            return Ok(None);
        };
        (ms, mr) = (ns.lo(), nr.lo());
    }
    Ok(Some([ms, mr]))
}

/// The box of `(s, r)` holding one solution of the edge system for every `t` from `a` to `b`, with
/// the third equation within the floor's share of zero, or `None`.
fn piece(
    edge: &Edge<'_>,
    (a, b): (f64, f64),
    meter: &mut WorkMeter<'_>,
) -> Result<Option<[Interval; 2]>, IntersectError> {
    let t = Interval::new(a, b)?;
    let Some([m0, m1]) = middle(edge, a / 2.0 + b / 2.0, meter)? else {
        return Ok(None);
    };
    let here = [point(a / 2.0 + b / 2.0)?, point(m0)?, point(m1)?];
    let Some(slope) = soft(edge.slope(&here, meter))? else {
        return Ok(None);
    };
    let Some(([i, j], leftover)) = best_rows(columns(&slope)) else {
        return Ok(None);
    };
    let entry = |row: usize, col: usize| -> Option<f64> {
        slope.get(row).and_then(|r| r.get(col)).copied().map(mid)
    };
    let (Some(p00), Some(p01), Some(p10), Some(p11)) =
        (entry(i, 1), entry(i, 2), entry(j, 1), entry(j, 2))
    else {
        return Ok(None);
    };
    let Some([[y00, y01], [y10, y11]]) = inverse([[p00, p01], [p10, p11]]) else {
        return Ok(None);
    };
    let Some(f) = soft(edge.over(&[t, point(m0)?, point(m1)?], meter))? else {
        return Ok(None);
    };
    let (Some(fi), Some(fj)) = (f.get(i).copied(), f.get(j).copied()) else {
        return Ok(None);
    };
    let yf0 = point(y00)?.mul(fi)?.add(point(y01)?.mul(fj)?)?;
    let yf1 = point(y10)?.mul(fi)?.add(point(y11)?.mul(fj)?)?;
    // The box's radius is one length on the other surface, a little more than the larger of the
    // two first corrections Y.F(m; t) over the piece, and each unknown takes it in its own unit:
    // `rho` in s's, `rho * pace` in r's. One number for both is that length only where the two
    // parameters run at one speed. Where they do not, the box of one unknown was as wide, in its
    // unit, as the edge is long in the other's: it left the domain, or held too much of a curved
    // surface for the test below, however short the piece. The march's step box takes its room
    // the same way (`around_step`).
    let pace = pace(&slope)?;
    let mut rho = larger(1.05 * larger(most(yf0), most(yf1) / pace), 1e-12);
    for _ in 0..TRIES {
        let x0 = Interval::new(m0 - rho, m0 + rho)?;
        let wide = rho * pace;
        let x1 = Interval::new(m1 - wide, m1 + wide)?;
        let Some(over) = soft(edge.slope(&[t, x0, x1], meter))? else {
            return Ok(None);
        };
        let cell = |row: usize, col: usize| -> Result<Interval, IntersectError> {
            over.get(row)
                .and_then(|r| r.get(col))
                .copied()
                .ok_or(IntersectError::Internal)
        };
        // x - m, enclosed: the box's own ends less its middle, so Krawczyk's term holds every
        // point of the box however its ends rounded.
        let reaches = [x0.sub(point(m0)?)?, x1.sub(point(m1)?)?];
        // K = m - Y.F(m; t) + (I - Y.J(x)).(x - m), one unknown at a time.
        let image = |y: [f64; 2],
                     diagonal: usize,
                     held: f64,
                     yf: Interval|
         -> Result<Interval, IntersectError> {
            let [ya, yb] = y;
            let mut out = point(held)?.sub(yf)?;
            for ((col, at), reach) in [1_usize, 2].into_iter().enumerate().zip(reaches) {
                let yj = point(ya)?
                    .mul(cell(i, at)?)?
                    .add(point(yb)?.mul(cell(j, at)?)?)?;
                let eye = point(if diagonal == col { 1.0 } else { 0.0 })?;
                out = out.add(eye.sub(yj)?.mul(reach)?)?;
            }
            Ok(out)
        };
        let k0 = image([y00, y01], 0, m0, yf0)?;
        let k1 = image([y10, y11], 1, m1, yf1)?;
        let inside = strictly_inside([k0, k1], [x0, x1]);
        if inside {
            let Some(g) = soft(edge.over(&[t, k0, k1], meter))? else {
                return Ok(None);
            };
            let Some(third) = g.get(leftover).copied() else {
                return Ok(None);
            };
            let tolerance = edge.floor / SHARE;
            let holds = third.lo() >= -tolerance && third.hi() <= tolerance;
            return Ok(holds.then_some([k0, k1]));
        }
        rho *= 2.0;
    }
    Ok(None)
}

/// A bound, in space, on how far the edge is from its chord over `a` to `b`: the piece's length
/// times the width of the edge's velocity over it (the velocity's variation integrated). `None`
/// where the surface cannot be bounded there, or the edge may stand still.
fn bowed(
    edge: &Edge<'_>,
    (a, b): (f64, f64),
    meter: &mut WorkMeter<'_>,
) -> Result<Option<f64>, IntersectError> {
    let along = edge.along(Interval::new(a, b)?);
    let Some(found) = soft(partials(edge.own, along, 1, meter))? else {
        return Ok(None);
    };
    let (Some(du), Some(dv)) = (found.get(1), found.get(2)) else {
        return Err(IntersectError::Internal);
    };
    let velocity: &[Interval; 3] = if edge.fixed_u { dv } else { du };
    // An edge that stands still, as one on a pole line does, is a point and no curve: it lies in
    // whatever contains the point, and is no branch.
    if !velocity
        .iter()
        .any(|x: &Interval| x.lo() > 0.0 || x.hi() < 0.0)
    {
        return Ok(None);
    }
    let wide = velocity
        .iter()
        .fold(0.0, |most: f64, x: &Interval| larger(most, x.hi() - x.lo()));
    Ok(Some((b - a) * wide))
}

/// The meet of two boxes of `(s, r)`, or `None` if they part.
fn meet(a: [Interval; 2], b: [Interval; 2]) -> Option<[Interval; 2]> {
    let one = |x: Interval, y: Interval| {
        Interval::new(larger(x.lo(), y.lo()), smaller(x.hi(), y.hi())).ok()
    };
    let [a0, a1] = a;
    let [b0, b1] = b;
    Some([one(a0, b0)?, one(a1, b1)?])
}

/// Proves that the edge lies in the other surface for every `t` from `lo` to `hi`, piece by piece,
/// each piece within `ceiling` of its chord; `None` where it cannot.
///
/// # Errors
///
/// The meter's stop, or a kind's.
pub(crate) fn lies_in(
    edge: &Edge<'_>,
    (lo, hi): (f64, f64),
    ceiling: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Run>, IntersectError> {
    if lo.is_nan() || hi.is_nan() || hi <= lo {
        return Ok(None);
    }
    // Each piece carries how often it has been halved, which alone bounds the work: a piece at
    // the deepest level that is not proven ends the proof, so at most 2^DEPTH pieces are proven
    // and twice that tried, however the halves round.
    let mut work = alloc::vec![(lo, hi, 0_u32)];
    let mut done: Vec<((f64, f64), [Interval; 2], f64)> = Vec::new();
    while let Some((a, b, depth)) = work.pop() {
        meter.charge(1)?;
        let bow = bowed(edge, (a, b), meter)?;
        let proof = match bow {
            Some(d) if d <= ceiling => piece(edge, (a, b), meter)?,
            _ => None,
        };
        if let (Some(x), Some(d)) = (proof, bow) {
            done.push(((a, b), x, d));
            continue;
        }
        if depth == DEPTH {
            return Ok(None);
        }
        let half = a / 2.0 + b / 2.0;
        let deeper = depth.saturating_add(1);
        work.push((half, b, deeper));
        work.push((a, half, deeper));
    }
    let mut cuts = alloc::vec![lo];
    let mut pieces = Vec::new();
    let mut deviation: f64 = 0.0;
    for ((_, b), x, d) in &done {
        cuts.push(*b);
        pieces.push(*x);
        deviation = larger(deviation, *d);
    }
    let (Some(first), Some(last)) = (pieces.first().copied(), pieces.last().copied()) else {
        return Ok(None);
    };
    let mut at = alloc::vec![first];
    for pair in pieces.windows(2) {
        let [x, y] = pair else {
            return Err(IntersectError::Internal);
        };
        let Some(joined) = meet(*x, *y) else {
            return Ok(None);
        };
        at.push(joined);
    }
    at.push(last);
    Ok(Some(Run {
        cuts,
        pieces,
        at,
        deviation,
    }))
}

/// A point of an edge as a box of the pair's four parameters, `(u1, v1, u2, v2)`.
///
/// `own_first` says whether the edge belongs to the first surface; `fixed_u` whether its fixed
/// parameter is u; `end` is the fixed value; `t` the parameter along the edge and `x` the other
/// surface's `(s, r)` there.
pub(crate) fn four(
    (own_first, fixed_u): (bool, bool),
    end: f64,
    t: f64,
    x: [Interval; 2],
) -> Result<[Interval; 4], IntersectError> {
    let held = point(end)?;
    let along = point(t)?;
    let [s, r] = x;
    let [p, q] = if fixed_u {
        [held, along]
    } else {
        [along, held]
    };
    Ok(if own_first {
        [p, q, s, r]
    } else {
        [s, r, p, q]
    })
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::indexing_slicing,
        clippy::arithmetic_side_effects,
        clippy::panic,
        clippy::float_cmp,
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::disallowed_methods,
        reason = "test code: a failure should stop loudly, and platform maths may judge the kernel's"
    )]

    use super::*;
    use geom_curve::Frame;
    use geom_run::Budget;
    use geom_surface::{Cylinder, Plane, SurfaceKind};

    const UNIT: f64 = 1024.0;

    fn meter() -> WorkMeter<'static> {
        WorkMeter::new(Budget::UNLIMITED)
    }

    /// The other plane's two vectors: whole, in general position, no component zero in the sum
    /// of their products, so every product of the normal equations counts.
    const A: [f64; 3] = [-1.0, 3.0, 3.0];
    const B: [f64; 3] = [3.0, -2.0, 1.0];

    /// `s·A + r·B`, exact for the small whole and binary numbers used here.
    fn on(s: f64, r: f64) -> [f64; 3] {
        [
            s * A[0] + r * B[0],
            s * A[1] + r * B[1],
            s * A[2] + r * B[2],
        ]
    }

    fn plane(
        origin: [f64; 3],
        u: [f64; 3],
        v: [f64; 3],
        domain: ((f64, f64), (f64, f64)),
    ) -> SurfaceKind {
        SurfaceKind::Plane(Plane::new(origin, u, v, domain).unwrap())
    }

    /// The plane through the origin on A and B over `domain`.
    fn other(domain: ((f64, f64), (f64, f64))) -> SurfaceKind {
        plane([0.0; 3], A, B, domain)
    }

    /// A plane in the other plane, its edge u = 0 running along v as A − B from `origin`: the other
    /// plane's (s, r) on the edge are `origin`'s plus (v, −v).
    fn own(origin: (f64, f64), v: (f64, f64)) -> SurfaceKind {
        let (s, r) = origin;
        let sum = on(1.0, 1.0);
        let across = on(1.0, -1.0);
        plane(on(s, r), sum, across, ((0.0, 1.0), v))
    }

    fn edge_of<'a>(own: &'a SurfaceKind, other: &'a SurfaceKind) -> Edge<'a> {
        Edge {
            own,
            other,
            at: 0.0,
            fixed_u: true,
            floor: 8.0,
        }
    }

    /// The circle z = 0 of radius one unit about z, as the edge v = 0 of a cylinder, t along its
    /// turn over `turn`.
    fn ring(turn: (f64, f64)) -> SurfaceKind {
        let frame = Frame::standard().unwrap();
        SurfaceKind::Cylinder(Cylinder::new([0.0; 3], UNIT, frame, (turn, (0.0, UNIT))).unwrap())
    }

    fn circle_edge<'a>(own: &'a SurfaceKind, other: &'a SurfaceKind) -> Edge<'a> {
        Edge {
            own,
            other,
            at: 0.0,
            fixed_u: false,
            floor: 8.0,
        }
    }

    /// The plane z = 0 in units, about the origin.
    fn floor_plane() -> SurfaceKind {
        plane(
            [0.0; 3],
            [UNIT, 0.0, 0.0],
            [0.0, UNIT, 0.0],
            ((-2.0, 2.0), (-2.0, 2.0)),
        )
    }

    fn holds_all(x: Interval, lo: f64, hi: f64) -> bool {
        x.lo() <= lo && hi <= x.hi()
    }

    #[test]
    fn the_rows_kept_have_the_largest_determinant_the_first_of_a_tie() {
        // Every column pair of whole numbers from −3 to 3, against the determinants in integers.
        // Under Miri, those from −2 to 2, with ties and zeros as the wider ones have: each pair is
        // the same check, and Miri takes minutes over all 7^6.
        let (base, reach) = if cfg!(miri) { (5_u32, 2_i64) } else { (7, 3) };
        let digits = |mut code: u32| -> [i64; 6] {
            let mut out = [0_i64; 6];
            for d in &mut out {
                *d = i64::from(code % base) - reach;
                code /= base;
            }
            out
        };
        for code in 0..base.pow(6) {
            let [a0, b0, a1, b1, a2, b2] = digits(code);
            let dets = [a0 * b1 - b0 * a1, a0 * b2 - b0 * a2, a1 * b2 - b1 * a2];
            let mut best: Option<(i64, usize)> = None;
            for (k, d) in dets.iter().enumerate() {
                if d.abs() > 0 && best.is_none_or(|(b, _)| d.abs() > b) {
                    best = Some((d.abs(), k));
                }
            }
            let want = best.map(|(_, k)| ROWS[k]);
            let f = |x: i64| x as f64;
            let got = best_rows([[f(a0), f(b0)], [f(a1), f(b1)], [f(a2), f(b2)]]);
            assert_eq!(got, want, "columns {:?}", [a0, b0, a1, b1, a2, b2]);
        }
    }

    #[test]
    fn a_run_holds_a_box_only_where_a_piece_meets_it_in_every_unknown() {
        let i = |a: f64, b: f64| Interval::new(a, b).unwrap();
        let run = Run {
            cuts: alloc::vec![0.0, 1.0, 2.0],
            pieces: alloc::vec![[i(0.0, 1.0), i(10.0, 11.0)], [i(2.0, 3.0), i(12.0, 13.0)]],
            at: Vec::new(),
            deviation: 0.0,
        };
        // In the first piece's t, s and r.
        assert!(run.holds(&[i(0.25, 0.5), i(0.5, 0.75), i(10.5, 10.75)]));
        // Touching counts: ends are closed.
        assert!(run.holds(&[i(1.0, 1.0), i(3.0, 4.0), i(13.0, 14.0)]));
        // Past every piece's t, though s and r meet the first piece's box.
        assert!(!run.holds(&[i(5.0, 6.0), i(0.5, 0.75), i(10.5, 10.75)]));
        // Before every piece's t.
        assert!(!run.holds(&[i(-2.0, -1.0), i(0.5, 0.75), i(10.5, 10.75)]));
        // In the first piece's t and r, s past it; and in its t and s, r before it.
        assert!(!run.holds(&[i(0.25, 0.5), i(5.0, 6.0), i(10.5, 10.75)]));
        assert!(!run.holds(&[i(0.25, 0.5), i(0.5, 0.75), i(5.0, 6.0)]));
        // s and r from the second piece, t only in the first.
        assert!(!run.holds(&[i(0.25, 0.5), i(2.5, 2.75), i(12.5, 12.75)]));
    }

    #[test]
    fn a_straight_edge_is_its_chord_and_a_circle_bows_by_its_sagitta_or_more() {
        // A straight edge's velocity is one vector: its width is 0, and so is the bound.
        let flat = plane(
            [5.0, 7.0, 9.0],
            [2.0, 1.0, 4.0],
            [1.0, 2.0, 3.0],
            ((0.0, 1.0), (10.0, 12.0)),
        );
        let e = edge_of(&flat, &flat);
        assert_eq!(bowed(&e, (10.0, 11.0), &mut meter()).unwrap(), Some(0.0));
        // A quarter of a half-turn of the unit circle, from 10 half-turns on: it leaves its chord by
        // the sagitta R(1 − cos π/8) at its middle, and the bound is no more than the piece's length
        // times the whole range of the velocity's components, 2πR.
        let round = ring((10.0, 12.0));
        let floor = floor_plane();
        let e = circle_edge(&round, &floor);
        let (a, b) = (10.0, 10.25);
        let got = bowed(&e, (a, b), &mut meter()).unwrap().unwrap();
        let sagitta = UNIT * (1.0 - (core::f64::consts::PI / 8.0).cos());
        let most = (b - a) * 2.0 * core::f64::consts::PI * UNIT;
        assert!(
            sagitta <= got && got <= most,
            "{sagitta} <= {got} <= {most}"
        );
    }

    #[test]
    fn the_middle_is_the_least_squares_point_from_any_start_in_the_domain() {
        // The edge's point at t = 1/2 is 10.75·A + 11.25·B: the start (11, 11), the middle of the
        // other plane's domain, is a quarter off in each, and one Gauss-Newton step on a plane is
        // exact.
        let other = other(((10.0, 12.0), (10.0, 12.0)));
        let own = own((10.25, 11.75), (0.0, 1.0));
        let e = edge_of(&own, &other);
        let [s, r] = middle(&e, 0.5, &mut meter()).unwrap().unwrap();
        assert!(
            (s - 10.75).abs() < 1e-12 && (r - 11.25).abs() < 1e-12,
            "({s}, {r})"
        );
    }

    #[test]
    fn a_step_past_the_doubles_is_no_middle() {
        // The edge stands 10^300 away along x: the first step's gradient overflows.
        let far = plane(
            [1e300, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            ((0.0, 1.0), (0.0, 1.0)),
        );
        let big = plane(
            [0.0; 3],
            [1e10, 0.0, 0.0],
            [0.0, 1e10, 0.0],
            ((0.0, 1.0), (0.0, 1.0)),
        );
        let e = edge_of(&far, &big);
        assert_eq!(middle(&e, 0.5, &mut meter()).unwrap(), None);
    }

    #[test]
    fn a_piece_far_along_its_edge_holds_every_point_of_it() {
        // The edge runs from t = 10 to 12, the other plane's (s, r) on it (t − 11, 33 − t). A
        // sixteenth of it from t = 10 is short enough for the third equation: its residual over
        // the box is enclosed over t and the box apart, and so is as wide as about ten times the
        // piece's length.
        let other = other(((-2.0, 2.0), (20.0, 24.0)));
        let own = own((-11.0, 33.0), (10.0, 12.0));
        let e = edge_of(&own, &other);
        let (a, b) = (10.0, 10.0625);
        let [k0, k1] = piece(&e, (a, b), &mut meter()).unwrap().unwrap();
        let (s, r) = ((a - 11.0, b - 11.0), (33.0 - b, 33.0 - a));
        assert!(
            holds_all(k0, s.0, s.1) && holds_all(k1, r.0, r.1),
            "{k0:?} {k1:?}"
        );
        // And no more than its radius, a little past the travel, beyond it.
        assert!(k0.lo() > s.0 - 0.05 && k0.hi() < s.1 + 0.05, "{k0:?}");
        assert!(k1.lo() > r.0 - 0.05 && k1.hi() < r.1 + 0.05, "{k1:?}");
    }

    #[test]
    fn a_piece_far_from_zero_takes_its_box_a_few_roundings_wide() {
        // The other plane's (s, r) on the edge are (10^7 + t, 10^7 − t), where a double's spacing
        // is about 2^−29, and the piece is 2^−33 long: the first radius rounds to a box no wider
        // than the residual's own rounding, which is doubled until the box holds K. The domain is
        // a unit each way, so no radius jumps out of it.
        let m = 10_000_000.0;
        let other = other(((m - 1.0, m + 1.0), (m - 1.0, m + 1.0)));
        let own = own((m, m), (0.0, 1.0));
        let e = edge_of(&own, &other);
        let (a, b) = (0.5, 0.5 + 2.0_f64.powi(-33));
        let [k0, k1] = piece(&e, (a, b), &mut meter()).unwrap().unwrap();
        assert!(
            holds_all(k0, m + a, m + b) && holds_all(k1, m - b, m - a),
            "{k0:?} {k1:?}"
        );
        assert!(
            k0.hi() - k0.lo() < 1e-6 && k1.hi() - k1.lo() < 1e-6,
            "{k0:?} {k1:?}"
        );
    }

    #[test]
    fn nothing_is_proven_over_no_stretch() {
        let other = other(((10.0, 12.0), (10.0, 12.0)));
        let own = own((10.25, 11.75), (0.0, 1.0));
        let e = edge_of(&own, &other);
        for (lo, hi) in [(f64::NAN, 1.0), (0.0, f64::NAN), (1.0, 1.0), (1.0, 0.5)] {
            assert_eq!(
                lies_in(&e, (lo, hi), 10.24, &mut meter()).unwrap(),
                None,
                "{lo} to {hi}"
            );
        }
        // The same edge over its whole length is proven, from end to end.
        let run = lies_in(&e, (0.0, 1.0), 10.24, &mut meter())
            .unwrap()
            .unwrap();
        assert_eq!(
            (run.cuts.first(), run.cuts.last()),
            (Some(&0.0), Some(&1.0))
        );
        assert!(run.cuts.windows(2).all(|w| w[0] < w[1]), "{:?}", run.cuts);
    }

    /// The bounds `bowed` gives the 4096 pieces of a half-turn of the unit circle.
    fn bows_at_4096ths(e: &Edge<'_>) -> (f64, f64) {
        let mut least = f64::INFINITY;
        let mut most: f64 = 0.0;
        for k in 0..4096 {
            let (a, b) = (f64::from(k) / 4096.0, f64::from(k + 1) / 4096.0);
            let d = bowed(e, (a, b), &mut meter()).unwrap().unwrap();
            least = least.min(d);
            most = most.max(d);
        }
        (least, most)
    }

    #[test]
    fn a_curved_edge_is_halved_down_to_a_4096th_and_no_further() {
        // A half-turn of the unit circle in the plane it lies in. With the ceiling just above the
        // largest bound of a 4096th, every 4096th is within it and every 2048th past it, so the
        // proof takes exactly 4096 pieces, each held to the ceiling, its box holding the circle's
        // point (cos πt, sin πt) at both its ends. Just below the smallest, the pieces would have
        // to be 8192ths, past the deepest halving: nothing is proven.
        let round = ring((0.0, 2.0));
        let floor = floor_plane();
        let e = circle_edge(&round, &floor);
        let (least, most) = bows_at_4096ths(&e);
        assert!(4.0 * least > 1.02 * most, "{least} {most}");
        let ceiling = 1.01 * most;
        let run = lies_in(&e, (0.0, 1.0), ceiling, &mut meter())
            .unwrap()
            .unwrap();
        assert_eq!(run.pieces.len(), 4096);
        assert!(run.deviation <= ceiling, "{} > {ceiling}", run.deviation);
        let near = |x: Interval, y: f64| x.lo() - 1e-12 <= y && y <= x.hi() + 1e-12;
        for (k, [s, r]) in run.pieces.iter().enumerate() {
            for end in [k, k + 1] {
                let t = core::f64::consts::PI * end as f64 / 4096.0;
                assert!(
                    near(*s, t.cos()) && near(*r, t.sin()),
                    "piece {k}: {s:?} {r:?}"
                );
            }
        }
        assert_eq!(
            lies_in(&e, (0.0, 1.0), 0.99 * least, &mut meter()).unwrap(),
            None
        );
    }

    #[test]
    fn a_box_that_holds_its_image_only_past_the_domain_proves_nothing() {
        // s is held at 10^7 along the edge, where a double's spacing is 2^−29, and r runs with t.
        // The other plane's s reaches only a few of those spacings each way, and the box that
        // holds Krawczyk's image strictly in s needs about seven of them: with eight it is proven,
        // with seven no box both stands in the domain and holds its image in both unknowns, though
        // smaller ones hold it in r.
        let m = 10_000_000.0;
        let spacing = 2.0_f64.powi(-29);
        let own = plane(on(m, 0.0), on(1.0, 1.0), B, ((0.0, 1.0), (0.0, 1.0)));
        let (a, b) = (0.5, 0.5 + 1e-9);
        let reach = |k: f64| other(((m - k * spacing, m + k * spacing), (0.0, 1.0)));
        let wide = reach(8.0);
        let e = edge_of(&own, &wide);
        let [k0, k1] = piece(&e, (a, b), &mut meter()).unwrap().unwrap();
        assert!(holds_all(k0, m, m) && holds_all(k1, a, b), "{k0:?} {k1:?}");
        let narrow = reach(7.0);
        let e = edge_of(&own, &narrow);
        assert_eq!(piece(&e, (a, b), &mut meter()).unwrap(), None);
    }
}
