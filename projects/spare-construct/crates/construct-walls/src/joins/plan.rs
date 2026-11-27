//! The plan arithmetic of the joins: exact integers on the kernel's grid steps (a `Length`'s
//! steps), every step checked. A point is `[x, y]` in steps of the model's plan; a wall along an
//! axis has a [`Frame`]: its direction `u` and the unit `r` to its right, each a unit vector
//! along x or y, so going into a wall's frame and back is a choice of coordinate and a sign, and
//! nothing is rounded.
//!
//! Bounds: every coordinate is within the kernel's grid, `2^40 - 1` steps of the origin
//! ([`LIMIT`]), so a difference is under `2^41`, a dot or cross product of two differences under
//! `2^83` and a squared distance times the tolerance's square under `2^96`: all within an `i128`.

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

/// `a - b`.
pub(super) fn sub(a: i64, b: i64) -> Option<i64> {
    a.checked_sub(b)
}

/// `a * s` for a sign `s` of 1 or -1 (or 0).
pub(super) fn signed(a: i64, s: i64) -> Option<i64> {
    a.checked_mul(s)
}

/// `a - b`, as vectors.
pub(super) fn less(a: P, b: P) -> Option<P> {
    let ([ax, ay], [bx, by]) = (a, b);
    Some([sub(ax, bx)?, sub(ay, by)?])
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

/// `a . u` for a unit vector `u` along an axis: a coordinate of `a`, with its sign.
pub(super) fn along(a: P, u: P) -> Option<i64> {
    let ([ax, ay], [ux, uy]) = (a, u);
    add(ax.checked_mul(ux)?, ay.checked_mul(uy)?)
}

/// The point `a * u + c * r` of a frame's two unit vectors.
pub(super) fn point(a: i64, u: P, c: i64, r: P) -> Option<P> {
    let ([ux, uy], [rx, ry]) = (u, r);
    Some([
        add(a.checked_mul(ux)?, c.checked_mul(rx)?)?,
        add(a.checked_mul(uy)?, c.checked_mul(ry)?)?,
    ])
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

/// A wall's frame: its direction and the unit to its right, when it runs along an axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Frame {
    /// Along the wall, from its start to its end.
    pub(super) u: P,
    /// To its right: its exterior side.
    pub(super) r: P,
}

impl Frame {
    /// The frame of the line from `start` to `end`, when it runs along x or y.
    pub(super) fn of(start: P, end: P) -> Option<Frame> {
        let [dx, dy] = less(end, start)?;
        let u = match (dx.signum(), dy.signum()) {
            (s, 0) if s != 0 => [s, 0],
            (0, s) if s != 0 => [0, s],
            _ => return None,
        };
        let [ux, uy] = u;
        Some(Frame {
            u,
            r: [uy, ux.checked_neg()?],
        })
    }
}
