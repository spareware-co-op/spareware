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
//! start for the middle of a piece is Gauss-Newton on the three residuals.
//!
//! Cost: a piece takes a handful of reads of both surfaces; a straight edge takes one piece.
//! All of it is charged to the meter. Recursion: none.

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
use crate::solve::Field;

/// The pieces a stretch may be halved into at most: a 4096th of it.
const SMALLEST: f64 = 4096.0;

/// The most pieces a stretch may take.
const PIECES: usize = 4096;

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

/// What has been proven of one edge so far: whether the proof was tried, and the run it gave.
#[derive(Default)]
pub(crate) struct Lying {
    /// Whether [`lies_in`] has been run for the edge.
    pub(crate) tried: bool,
    /// The run, if it proved the edge lies in the other surface.
    pub(crate) run: Option<Run>,
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
        let (ns, nr) = (ms - (y00 * ga + y01 * gb), mr - (y10 * ga + y11 * gb));
        if !(ns.is_finite() && nr.is_finite()) {
            return Ok(None);
        }
        (ms, mr) = (ns, nr);
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
    let mut rho = larger(1.05 * larger(most(yf0), most(yf1)), 1e-12);
    for _ in 0..TRIES {
        let x0 = Interval::new(m0 - rho, m0 + rho)?;
        let x1 = Interval::new(m1 - rho, m1 + rho)?;
        let Some(over) = soft(edge.slope(&[t, x0, x1], meter))? else {
            return Ok(None);
        };
        let cell = |row: usize, col: usize| -> Result<Interval, IntersectError> {
            over.get(row)
                .and_then(|r| r.get(col))
                .copied()
                .ok_or(IntersectError::Internal)
        };
        let reach = Interval::new(-rho, rho)?;
        // K = m - Y.F(m; t) + (I - Y.J(x)).(x - m), one unknown at a time.
        let image = |y: [f64; 2], diagonal: usize, held: f64, yf: Interval| {
            let [ya, yb] = y;
            let mut out = point(held)?.sub(yf)?;
            for (col, at) in [1_usize, 2].into_iter().enumerate() {
                let yj = point(ya)?
                    .mul(cell(i, at)?)?
                    .add(point(yb)?.mul(cell(j, at)?)?)?;
                let eye = point(if diagonal == col { 1.0 } else { 0.0 })?;
                out = out.add(eye.sub(yj)?.mul(reach)?)?;
            }
            Ok::<Interval, IntersectError>(out)
        };
        let k0 = image([y00, y01], 0, m0, yf0)?;
        let k1 = image([y10, y11], 1, m1, yf1)?;
        let inside =
            x0.lo() < k0.lo() && k0.hi() < x0.hi() && x1.lo() < k1.lo() && k1.hi() < x1.hi();
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
    let velocity = if edge.fixed_u { dv } else { du };
    // An edge that stands still, as one on a pole line does, is a point and no curve: it lies in
    // whatever contains the point, and is no branch.
    if !velocity.iter().any(|x| x.lo() > 0.0 || x.hi() < 0.0) {
        return Ok(None);
    }
    let wide = velocity
        .iter()
        .fold(0.0, |most, x| larger(most, x.hi() - x.lo()));
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
    let smallest = (hi - lo) / SMALLEST;
    let mut work = alloc::vec![(lo, hi)];
    let mut done: Vec<((f64, f64), [Interval; 2], f64)> = Vec::new();
    while let Some((a, b)) = work.pop() {
        meter.charge(1)?;
        if done.len().saturating_add(work.len()) > PIECES {
            return Ok(None);
        }
        let bow = bowed(edge, (a, b), meter)?;
        let proof = match bow {
            Some(d) if d <= ceiling => piece(edge, (a, b), meter)?,
            _ => None,
        };
        if let (Some(x), Some(d)) = (proof, bow) {
            done.push(((a, b), x, d));
            continue;
        }
        let half = a / 2.0 + b / 2.0;
        if b - a <= smallest || !(a < half && half < b) {
            return Ok(None);
        }
        work.push((half, b));
        work.push((a, half));
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
