//! Fuzz target: `geom-2d`'s arrangement of line segments keeps its stated laws on any input.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a count of segments and, for
//! each, four signed bytes, the grid coordinates of its two ends. [`check`] arranges them and
//! tests each rule against an independent reading of it: the vertices are exactly the end points
//! and the proper crossings that the cross-product formula gives, as reduced fractions, each once
//! and in lexicographic order; Euler's formula holds, V − E + F = 1 + C for C connected
//! components; every half-edge lies on exactly one cycle; every cycle is the outer boundary of
//! one face or a hole of exactly one; and arranging the segments in the opposite order gives the
//! same vertices and edges. [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below; a unit
//!   test builds the input that reaches it.
//! - **A panic is a failure; a disagreement is a [`Finding`]** naming the law and the case.
//!
//! Method: differential testing against the cross-product formula for the meeting of two segments
//! in whole numbers (O'Rourke, *Computational Geometry in C*, 2nd ed., Cambridge 1998, section
//! 7.1, <https://doi.org/10.1017/CBO9780511804120>) and Euler's formula for planar graphs, written here from the
//! rules and not from the code.
//!
//! Cost: constant per case, at most [`MAX_SEGMENTS`] segments; at most [`MAX_INPUT_BYTES`] bytes
//! read.

#![allow(
    clippy::many_single_char_names,
    reason = "a, b, e, f, c, v, x, y and w are the formulas' own letters"
)]

use std::collections::BTreeSet;

use geom_2d::arrange::{Arrangement, At, arrange};
use geom_num::grid::P2;
use geom_run::{Budget, WorkMeter};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The most segments of a case.
pub const MAX_SEGMENTS: u8 = 8;

/// The most bytes [`decode`] reads from any input: 33.
///
/// A count byte, and four bytes (two signed coordinates for each end) for each of at most
/// [`MAX_SEGMENTS`] segments: 1 + 4 · 8 = 33. The unit test
/// `the_input_bound_is_reached_and_never_passed` builds the input.
pub const MAX_INPUT_BYTES: usize = 1 + 4 * 8;

const _: () = assert!(MAX_INPUT_BYTES == 33);

/// One case: segments by their end coordinates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// Each segment as (x0, y0, x1, y1), small signed numbers.
    pub segments: Vec<[i8; 4]>,
}

/// Decodes `data` into a case, and returns it with the number of bytes read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let count = usize::from(src.byte() % (MAX_SEGMENTS + 1));
    let segments = (0..count)
        .map(|_| {
            let mut s = [0_i8; 4];
            for v in &mut s {
                // Coordinates fall in a narrow band so that segments meet.
                *v = i8::from_le_bytes([src.byte()]) / 8;
            }
            s
        })
        .collect();
    let read = data.len().saturating_sub(src.remaining());
    (Case { segments }, read)
}

fn fail(case: &Case, law: &str) -> Finding {
    Finding::new(law, || format!("{case:?}"))
}

/// A point as a reduced fraction pair (x numerator, x denominator, y numerator, y denominator).
type Red = (i128, i128, i128, i128);

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.unsigned_abs(), b.unsigned_abs());
    while b != 0 {
        let r = a.checked_rem(b).unwrap_or(0);
        a = b;
        b = r;
    }
    i128::try_from(a).unwrap_or(1).max(1)
}

fn reduced(x: i128, y: i128, w: i128) -> Option<Red> {
    let gx = gcd(x, w);
    let gy = gcd(y, w);
    Some((
        x.checked_div(gx)?,
        w.checked_div(gx)?,
        y.checked_div(gy)?,
        w.checked_div(gy)?,
    ))
}

fn of_rational(v: &At) -> Option<Red> {
    let ([x, y], w) = v.rational()?.coordinates();
    reduced(x, y, w)
}

/// The vertices the cross-product formula gives: every end point and every proper crossing.
fn oracle(segs: &[[i128; 4]]) -> Option<BTreeSet<Red>> {
    let mut out = BTreeSet::new();
    for &[x0, y0, x1, y1] in segs {
        out.insert((x0, 1, y0, 1));
        out.insert((x1, 1, y1, 1));
    }
    for (i, &[ax, ay, bx, by]) in segs.iter().enumerate() {
        for &[cx, cy, dx, dy] in segs.get(i.checked_add(1)?..)? {
            let (rx, ry) = (bx.checked_sub(ax)?, by.checked_sub(ay)?);
            let (sx, sy) = (dx.checked_sub(cx)?, dy.checked_sub(cy)?);
            let den = rx.checked_mul(sy)?.checked_sub(ry.checked_mul(sx)?)?;
            if den == 0 {
                continue;
            }
            let (qx, qy) = (cx.checked_sub(ax)?, cy.checked_sub(ay)?);
            let tn = qx.checked_mul(sy)?.checked_sub(qy.checked_mul(sx)?)?;
            let un = qx.checked_mul(ry)?.checked_sub(qy.checked_mul(rx)?)?;
            let (tn, un, den) = if den < 0 {
                (tn.checked_neg()?, un.checked_neg()?, den.checked_neg()?)
            } else {
                (tn, un, den)
            };
            if tn < 0 || tn > den || un < 0 || un > den {
                continue;
            }
            let x = ax.checked_mul(den)?.checked_add(tn.checked_mul(rx)?)?;
            let y = ay.checked_mul(den)?.checked_add(tn.checked_mul(ry)?)?;
            out.insert(reduced(x, y, den)?);
        }
    }
    Some(out)
}

/// The number of connected components of the edge graph.
fn components(a: &Arrangement) -> usize {
    let n = a.vertices().len();
    let mut parent: Vec<usize> = (0..n).collect();
    let find = |parent: &mut Vec<usize>, mut x: usize| {
        while let Some(&p) = parent.get(x) {
            if p == x {
                break;
            }
            let grand = parent.get(p).copied().unwrap_or(p);
            if let Some(slot) = parent.get_mut(x) {
                *slot = grand;
            }
            x = grand;
        }
        x
    };
    for e in a.edges() {
        let u = find(&mut parent, usize::try_from(e.from).unwrap_or(0));
        let v = find(&mut parent, usize::try_from(e.to).unwrap_or(0));
        if let Some(slot) = parent.get_mut(u) {
            *slot = v;
        }
    }
    let mut roots = BTreeSet::new();
    for v in 0..n {
        roots.insert(find(&mut parent, v));
    }
    roots.len()
}

/// Tests the arrangement's laws on the case.
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn check(case: &Case) -> Result<(), Finding> {
    let grid: Option<Vec<(P2, P2)>> = case
        .segments
        .iter()
        .map(|&[x0, y0, x1, y1]| {
            Some((
                P2::new(i64::from(x0), i64::from(y0)).ok()?,
                P2::new(i64::from(x1), i64::from(y1)).ok()?,
            ))
        })
        .collect();
    let Some(segs) = grid else {
        return Err(fail(case, "small points are on the grid"));
    };
    let Ok(a) = arrange(&segs, &mut WorkMeter::new(Budget::UNLIMITED)) else {
        return Err(fail(case, "small segments are within the exact budget"));
    };
    let wide: Vec<[i128; 4]> = case
        .segments
        .iter()
        .filter(|&&[x0, y0, x1, y1]| x0 != x1 || y0 != y1)
        .map(|s| s.map(i128::from))
        .collect();
    let want = oracle(&wide).ok_or_else(|| fail(case, "the oracle fits"))?;
    let got: Option<Vec<Red>> = a.vertices().iter().map(of_rational).collect();
    let got = got.ok_or_else(|| fail(case, "a vertex reduces"))?;
    let got_set: BTreeSet<Red> = got.iter().copied().collect();
    if got_set != want || got_set.len() != got.len() {
        return Err(fail(
            case,
            "the vertices are exactly the end points and the proper crossings, each once",
        ));
    }
    let (v, e, f, c) = (
        a.vertices().len(),
        a.edges().len(),
        a.faces().len(),
        components(&a),
    );
    if v.checked_add(f) != e.checked_add(c).and_then(|n| n.checked_add(1)) {
        return Err(fail(case, "Euler's formula: V - E + F = 1 + C"));
    }
    let half_edges: usize = a
        .cycles()
        .iter()
        .map(Vec::len)
        .fold(0, usize::saturating_add);
    if Some(half_edges) != e.checked_mul(2) {
        return Err(fail(case, "every half-edge is on one cycle"));
    }
    let mut owners = vec![0_u32; a.cycles().len()];
    for face in a.faces() {
        for c in face.outer.iter().chain(face.holes.iter()) {
            if let Some(n) = usize::try_from(*c).ok().and_then(|i| owners.get_mut(i)) {
                *n = n.saturating_add(1);
            }
        }
    }
    if owners.iter().any(|&n| n != 1) {
        return Err(fail(
            case,
            "every cycle bounds one face or is a hole of exactly one",
        ));
    }
    // The opposite order gives the same vertices and edges.
    let mut reversed = segs.clone();
    reversed.reverse();
    let Ok(b) = arrange(&reversed, &mut WorkMeter::new(Budget::UNLIMITED)) else {
        return Err(fail(case, "the reversed segments are within the budget"));
    };
    let same_vertices = a
        .vertices()
        .iter()
        .map(of_rational)
        .eq(b.vertices().iter().map(of_rational));
    let same_edges = a
        .edges()
        .iter()
        .map(|e| (e.from, e.to))
        .eq(b.edges().iter().map(|e| (e.from, e.to)));
    if !(same_vertices && same_edges) {
        return Err(fail(case, "the input order does not matter"));
    }
    Ok(())
}

/// Decodes `data` and checks the case.
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}
