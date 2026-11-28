//! The plan arithmetic of the joins: exact integers on the kernel's grid steps (a `Length`'s
//! steps), every step checked. A point is `[x, y]` in steps of the model's plan; a wall's line
//! runs along its direction `d` (its end less its start, whole steps at any angle), and a point's
//! place along a wall is [`along`], its dot product with `d`: the true distance times `|d|`, so it
//! orders points exactly and needs no root. Where a corner is not a grid point it is worked in
//! [`super::exact`] and rounded once.
//!
//! Bounds: every coordinate is within the kernel's grid, `2^40 - 1` steps of the origin
//! ([`LIMIT`]), so a difference is under `2^41`, a dot or cross product of two differences under
//! `2^83` and a squared distance times the tolerance's square under `2^96`: all within an `i128`.

use num_bigint::BigInt;

/// The farthest a coordinate may be from the origin, in grid steps: the kernel's grid.
pub(super) const LIMIT: i64 = 1_099_511_627_775;

/// `2^63`: a cross product at least this large is far beyond any tolerance times a length.
const FAR: u128 = 9_223_372_036_854_775_808;

/// A point or a vector of the plan, in steps.
pub(super) type P = [i64; 2];

/// `a + b`.
pub(super) fn add(a: i64, b: i64) -> Option<i64> {
    a.checked_add(b)
}

/// `a * s` for a sign `s` of 1 or -1 (or 0).
pub(super) fn signed(a: i64, s: i64) -> Option<i64> {
    a.checked_mul(s)
}

/// `a - b`, as vectors.
pub(super) fn less(a: P, b: P) -> Option<P> {
    let ([ax, ay], [bx, by]) = (a, b);
    Some([ax.checked_sub(bx)?, ay.checked_sub(by)?])
}

/// `a + b`, as vectors.
pub(super) fn plus(a: P, b: P) -> Option<P> {
    let ([ax, ay], [bx, by]) = (a, b);
    Some([ax.checked_add(bx)?, ay.checked_add(by)?])
}

/// `-a`.
pub(super) fn negated([x, y]: P) -> Option<P> {
    Some([x.checked_neg()?, y.checked_neg()?])
}

/// `a . b`, wide.
pub(super) fn dot(a: P, b: P) -> i128 {
    let ([ax, ay], [bx, by]) = (a, b);
    // Each product is of two numbers under 2^42, so under 2^84; their sum is an i128.
    i128::from(ax)
        .wrapping_mul(i128::from(bx))
        .wrapping_add(i128::from(ay).wrapping_mul(i128::from(by)))
}

/// `a x b`, wide.
pub(super) fn cross(a: P, b: P) -> i128 {
    let ([ax, ay], [bx, by]) = (a, b);
    i128::from(ax)
        .wrapping_mul(i128::from(by))
        .wrapping_sub(i128::from(ay).wrapping_mul(i128::from(bx)))
}

/// The vector to the right of `d`, as long as it: `(dy, -dx)`.
pub(super) fn right([dx, dy]: P) -> Option<P> {
    Some([dy, dx.checked_neg()?])
}

/// The sign of `a . b`: 1, 0 or -1.
pub(super) fn side(a: P, b: P) -> i64 {
    match dot(a, b).signum() {
        1 => 1,
        -1 => -1,
        _ => 0,
    }
}

/// Where `p` lies along the line from `start` along `d`: `(p - start) . d`, the true distance
/// times `|d|`.
pub(super) fn along(p: P, start: P, d: P) -> Option<i128> {
    Some(dot(less(p, start)?, d))
}

/// Whether `gap`, a distance along a line along `d` as [`along`] gives it, is more than `tol`
/// steps: `gap > tol * |d|`, asked of the squares.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "arbitrary-precision whole numbers, which never overflow"
)]
pub(super) fn farther(gap: i128, tol: i64, d: P) -> bool {
    if gap <= 0 {
        return false;
    }
    let g = BigInt::from(gap);
    let t = BigInt::from(tol);
    let dd = BigInt::from(dot(d, d));
    &g * &g > &t * &t * dd
}

/// `|d|` rounded up to a whole number of steps.
pub(super) fn length_up(d: P) -> Option<i128> {
    let dd = u128::try_from(dot(d, d)).ok()?;
    let root = dd.isqrt();
    let up = if root.checked_mul(root)? == dd {
        root
    } else {
        root.checked_add(1)?
    };
    i128::try_from(up).ok()
}

/// Whether the square of `w`'s length is at most `tol * tol`.
fn short(w: P, tol: i64) -> bool {
    let t = i128::from(tol);
    dot(w, w) <= t.wrapping_mul(t)
}

/// Whether `p` is within `tol` of the segment from `from` to `to`, exactly.
pub(super) fn near(p: P, from: P, to: P, tol: i64) -> bool {
    let (Some(run), Some(rel), Some(past)) = (less(to, from), less(p, from), less(p, to)) else {
        return false;
    };
    let (square, onto) = (dot(run, run), dot(rel, run));
    if onto <= 0 {
        return short(rel, tol);
    }
    if onto >= square {
        return short(past, tol);
    }
    // The distance from the line is |run x rel| / |run|; compare squares. Over 2^63 it is far
    // beyond `tol * |run|`, which is under 2^54.
    let off = cross(run, rel).unsigned_abs();
    if off >= FAR {
        return false;
    }
    let tt = u128::from(tol.unsigned_abs()).wrapping_mul(u128::from(tol.unsigned_abs()));
    off.wrapping_mul(off) <= tt.wrapping_mul(square.unsigned_abs())
}

/// Whether the segments `a0 a1` and `b0 b1` cross at a point inside both.
pub(super) fn crosses(a0: P, a1: P, b0: P, b1: P) -> bool {
    let side = |o: P, e: P, p: P| match (less(e, o), less(p, o)) {
        (Some(d), Some(w)) => cross(d, w).signum(),
        _ => 0,
    };
    let (s1, s2) = (side(a0, a1, b0), side(a0, a1, b1));
    let (s3, s4) = (side(b0, b1, a0), side(b0, b1, a1));
    s1.wrapping_mul(s2) < 0 && s3.wrapping_mul(s4) < 0
}

/// Twice the signed area of a polygon, positive when it runs counter-clockwise seen from above.
pub(super) fn twice_area(corners: &[P]) -> Option<i128> {
    let mut sum = 0_i128;
    for (at, &a) in corners.iter().enumerate() {
        let b = *corners.get(at.checked_add(1)?.checked_rem(corners.len())?)?;
        sum = sum.checked_add(cross(a, b))?;
    }
    Some(sum)
}

/// Whether a polygon turns left at every corner: strictly convex and counter-clockwise.
pub(super) fn convex(corners: &[P]) -> bool {
    let n = corners.len();
    n >= 3
        && (0..n).all(|at| {
            let pick = |k: usize| {
                at.checked_add(k)
                    .and_then(|i| i.checked_rem(n))
                    .and_then(|i| corners.get(i))
                    .copied()
            };
            match (pick(0), pick(1), pick(2)) {
                (Some(a), Some(b), Some(c)) => match (less(b, a), less(c, b)) {
                    (Some(ab), Some(bc)) => cross(ab, bc) > 0,
                    _ => false,
                },
                _ => false,
            }
        })
}
