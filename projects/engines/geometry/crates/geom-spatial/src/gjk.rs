//! The certified GJK filter: two sets of grid points proven at least a reach apart, or nothing
//! claimed (19.10).
//!
//! What it does: [`filter`] runs GJK on the convex hulls of two sets of grid points, in doubles, to
//! find a direction that separates them. The direction is cut to integers and each set's extreme
//! along it is found exactly. The sets are [`Filter::Clear`] only when those extremes prove a gap
//! of at least the reach; otherwise they are [`Filter::Undecided`], for the exact narrow phase.
//! The reach is the clearance plus each set's own bound (19.10): for the hulls of two certified
//! tessellations, `Clear` proves the true shapes a clearance apart.
//!
//! Guarantees:
//!
//! - **Never clear below the reach.** `Clear` carries a [`Certificate`]: an integer direction n,
//!   the largest n · p over the first set and the smallest n · q over the second, all exact. The
//!   second exceeds the first by g > 0 with g² ≥ r² · |n|², so for every p and q, |q − p| ≥
//!   (q − p) · n / |n| ≥ g / |n| ≥ r (Cauchy and Schwarz): every point of the second hull is at
//!   least r from every point of the first. [`Certificate::proves`] checks one against the points.
//! - **Doubles only steer.** GJK's arithmetic picks the direction. A poor step makes the filter
//!   answer `Undecided` more often, never `Clear` wrongly.
//! - **Deterministic.** Only +, −, × and ÷ on doubles, in a fixed order, with ties going to the
//!   lowest index: the same points in the same order give the same answer on every target.
//!
//! Bit budget: coordinates within ±(2^40 − 1) and direction components within ±[`DIRECTION`] =
//! ±2^20, so n · p is below 3 · 2^60, a gap below 3 · 2^61 and its square below 2^126; the reach
//! is at most [`MAX_REACH`] = 2^43, so r² · |n|² ≤ 2^86 · 3 · 2^40 < 2^128. Every integer step is
//! a checked `i128` or `u128` operation.
//!
//! Method: GJK (Gilbert, Johnson and Keerthi, 1988, <https://doi.org/10.1109/56.2083>) on the
//! Minkowski difference of the two hulls: the point of the simplex nearest the origin, by the
//! regions of its vertices, edges and faces (Ericson, *Real-Time Collision Detection*, 2005,
//! sections 5.1.5 and 5.1.6), then the support point along its negation, until the two bounds
//! meet. The certificate is the separating plane 19.10 asks for, checked in integers.
//!
//! Consulted: parry, Apache-2.0,
//!  <https://github.com/dimforge/parry/blob/b3c3584ce9be9bf6edb821fd7548b817087131ff/src/query/gjk/gjk.rs>,
//!  for GJK's loop: the origin's projection on the simplex, the support point along its negation,
//!  and the stop when the upper and lower bounds meet. No code copied.
//!
//! Cost: at most [`STEPS`] steps, each one support search over both sets, O(n + m) and charged to
//! the meter; then one exact pass for the certificate. No allocation.

use core::fmt;

use geom_num::grid::P3;
use geom_num::math;
use geom_run::{Stop, Stopped, WorkMeter};

/// The largest magnitude of a certificate's direction component, 2^20: enough to steer, and small
/// enough that every product stays inside the bit budget.
pub const DIRECTION: i64 = 1 << 20;

/// The largest reach the filter takes, 2^43 grid steps (twice the frame's width).
pub const MAX_REACH: u64 = 1 << 43;

/// The most GJK steps the filter takes before it tries the direction it has.
pub const STEPS: usize = 64;

/// The relative progress below which a step stops GJK: its two bounds have met.
const SETTLED: f64 = 1.0 / 1_048_576.0;

/// What the filter proves about two sets of points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    /// Proven: every point of the second set's hull is at least the reach from every point of the
    /// first's, by the certificate.
    Clear(Certificate),
    /// Nothing proven: the hulls may be nearer than the reach, or the doubles found no direction
    /// that shows they are not.
    Undecided,
}

/// An exact witness that two sets of points are apart: an integer direction and each set's extreme
/// along it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Certificate {
    /// The direction n, each component within ±[`DIRECTION`], pointing from the first set to the
    /// second.
    pub direction: [i64; 3],
    /// The largest n · p over the first set.
    pub first: i128,
    /// The smallest n · q over the second set.
    pub second: i128,
}

/// Why the filter gave no answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GjkError {
    /// A set had no points.
    Empty,
    /// The reach was over [`MAX_REACH`].
    ReachTooLarge,
    /// The meter stopped the filter.
    Stopped(Stop),
}

impl fmt::Display for GjkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GjkError::Empty => f.write_str("a set of points was empty"),
            GjkError::ReachTooLarge => write!(f, "the reach was over {MAX_REACH} grid steps"),
            GjkError::Stopped(stop) => write!(f, "the work meter stopped: {stop}"),
        }
    }
}

impl core::error::Error for GjkError {}

impl From<Stopped> for GjkError {
    fn from(e: Stopped) -> GjkError {
        GjkError::Stopped(e.reason())
    }
}

impl Certificate {
    /// Whether this proves every point of `second`'s hull at least `reach` from every point of
    /// `first`'s: its extremes are the sets' own along its direction, and they are far enough
    /// apart. Checked exactly, without the meter.
    #[must_use]
    pub fn proves(&self, first: &[P3], second: &[P3], reach: u64) -> bool {
        let within = self
            .direction
            .iter()
            .all(|c| c.unsigned_abs() <= DIRECTION.unsigned_abs());
        within
            && reach <= MAX_REACH
            && extreme(first, self.direction, Side::Largest) == Some(self.first)
            && extreme(second, self.direction, Side::Smallest) == Some(self.second)
            && apart(self.direction, self.first, self.second, reach)
    }
}

/// Whether two sets of grid points are proven at least `reach` grid steps apart: [`Filter::Clear`]
/// with its certificate, or [`Filter::Undecided`].
///
/// # Errors
///
/// [`GjkError::Empty`] for a set with no points, [`GjkError::ReachTooLarge`] for a reach over
/// [`MAX_REACH`], and [`GjkError::Stopped`] if the meter stops it.
pub fn filter(
    first: &[P3],
    second: &[P3],
    reach: u64,
    meter: &mut WorkMeter<'_>,
) -> Result<Filter, GjkError> {
    if reach > MAX_REACH {
        return Err(GjkError::ReachTooLarge);
    }
    let (Some(a), Some(b)) = (first.first(), second.first()) else {
        return Err(GjkError::Empty);
    };
    let both = u64::try_from(first.len().saturating_add(second.len())).unwrap_or(u64::MAX);
    let Some(nearest) =
        nearest_difference(first, second, sub(double(*a), double(*b)), both, meter)?
    else {
        return Ok(Filter::Undecided);
    };
    let Some(direction) = integer_direction(nearest) else {
        return Ok(Filter::Undecided);
    };
    meter.charge(both)?;
    let (Some(low), Some(high)) = (
        extreme(first, direction, Side::Largest),
        extreme(second, direction, Side::Smallest),
    ) else {
        return Ok(Filter::Undecided);
    };
    if !apart(direction, low, high, reach) {
        return Ok(Filter::Undecided);
    }
    Ok(Filter::Clear(Certificate {
        direction,
        first: low,
        second: high,
    }))
}

/// GJK on the Minkowski difference first − second, from the point `start` of it: the point of the
/// difference nearest the origin as far as the doubles find it, or `None` if the origin seems to be
/// in it (the sets may overlap or touch).
fn nearest_difference(
    first: &[P3],
    second: &[P3],
    start: [f64; 3],
    work: u64,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<[f64; 3]>, Stopped> {
    let mut simplex = Simplex {
        points: [start; 4],
        len: 1,
    };
    let mut v = start;
    for _ in 0..STEPS {
        let vv = dot(v, v);
        // Not above zero: the origin is in the simplex, or the doubles failed (NaN).
        if !above_zero(vv) {
            return Ok(None);
        }
        meter.charge(work)?;
        let w = sub(support(first, neg(v)), support(second, v));
        // vv − v · w is how far the support point still is past v: once it is a small part of
        // vv, the upper bound |v| and the lower bound v · w / |v| have met.
        if vv - dot(v, w) <= vv * SETTLED || simplex.holds(w) {
            break;
        }
        simplex.push(w);
        v = simplex.reduce();
        if simplex.len == 4 {
            return Ok(None);
        }
    }
    Ok(Some(v))
}

/// The point of `points` with the largest d · p, the first among equals, as doubles.
fn support(points: &[P3], d: [f64; 3]) -> [f64; 3] {
    let mut best: Option<([f64; 3], f64)> = None;
    for p in points {
        let q = double(*p);
        let along = dot(d, q);
        if best.is_none_or(|(_, top)| along > top) {
            best = Some((q, along));
        }
    }
    best.map_or([0.0; 3], |(q, _)| q)
}

/// The integer direction from the first set to the second, from the nearest point v of first −
/// second: −v scaled so its largest component is ±[`DIRECTION`], each component rounded. `None`
/// for a v the doubles left unusable.
fn integer_direction(v: [f64; 3]) -> Option<[i64; 3]> {
    let mut largest = 0.0_f64;
    for c in v {
        let size = magnitude(c);
        if size > largest {
            largest = size;
        }
    }
    if !above_zero(largest) || !largest.is_finite() {
        return None;
    }
    let scale = whole_to_double(DIRECTION) / largest;
    let mut out = [0_i64; 3];
    for (o, c) in out.iter_mut().zip(v) {
        let rounded = math::round_ties_even(-c * scale).ok()?.get();
        *o = whole(rounded)?;
    }
    Some(out)
}

/// Which extreme of a set [`extreme`] finds.
#[derive(Clone, Copy)]
enum Side {
    Largest,
    Smallest,
}

/// The largest or smallest n · p over `points`, exactly; `None` for no points or a direction
/// outside the budget.
fn extreme(points: &[P3], n: [i64; 3], side: Side) -> Option<i128> {
    let mut out: Option<i128> = None;
    for p in points {
        let along = along(n, *p)?;
        out = Some(match (out, side) {
            (None, _) => along,
            (Some(o), Side::Largest) => o.max(along),
            (Some(o), Side::Smallest) => o.min(along),
        });
    }
    out
}

/// n · p, exactly.
fn along(direction: [i64; 3], point: P3) -> Option<i128> {
    let [dx, dy, dz] = direction.map(i128::from);
    let first = dx.checked_mul(i128::from(point.x.get()))?;
    let second = dy.checked_mul(i128::from(point.y.get()))?;
    let third = dz.checked_mul(i128::from(point.z.get()))?;
    first.checked_add(second)?.checked_add(third)
}

/// Whether `low` and `high`, the two sets' extremes along `n`, prove a gap of at least `reach`:
/// g = high − low > 0 and g² ≥ reach² · |n|².
fn apart(n: [i64; 3], low: i128, high: i128, reach: u64) -> bool {
    let Some(gap) = high.checked_sub(low) else {
        return false;
    };
    let Ok(gap) = u128::try_from(gap) else {
        return false;
    };
    if gap == 0 {
        return false;
    }
    let length = n.iter().try_fold(0_u128, |s, c| {
        let c = u128::from(c.unsigned_abs());
        s.checked_add(c.checked_mul(c)?)
    });
    let reach = u128::from(reach);
    match (
        gap.checked_mul(gap),
        length.and_then(|l| reach.checked_mul(reach)?.checked_mul(l)),
    ) {
        (Some(g), Some(need)) => g >= need,
        // Beyond the budget: nothing is proven.
        _ => false,
    }
}

/// Up to four points of the Minkowski difference, the newest last.
struct Simplex {
    points: [[f64; 3]; 4],
    len: usize,
}

impl Simplex {
    /// Whether `w` is already one of the points.
    fn holds(&self, w: [f64; 3]) -> bool {
        self.points.iter().take(self.len).any(|p| *p == w)
    }

    /// Adds `w`; the caller keeps at most three points between adds.
    fn push(&mut self, w: [f64; 3]) {
        if let Some(slot) = self.points.get_mut(self.len) {
            *slot = w;
            self.len = self.len.saturating_add(1).min(4);
        }
    }

    /// Keeps only the points of the face, edge or vertex nearest the origin, and returns that
    /// nearest point; the origin itself when the four points enclose it.
    fn reduce(&mut self) -> [f64; 3] {
        let [a, b, c, _] = self.points;
        let (nearest, keep) = match self.len {
            2 => segment(a, b),
            3 => triangle(a, b, c),
            4 => tetrahedron(self.points),
            _ => (a, Keep::one(a)),
        };
        self.points = keep.points;
        self.len = keep.len;
        nearest
    }
}

/// The points a simplex keeps.
struct Keep {
    points: [[f64; 3]; 4],
    len: usize,
}

impl Keep {
    fn one(a: [f64; 3]) -> Keep {
        Keep {
            points: [a; 4],
            len: 1,
        }
    }

    fn two(a: [f64; 3], b: [f64; 3]) -> Keep {
        Keep {
            points: [a, b, b, b],
            len: 2,
        }
    }

    fn three(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> Keep {
        Keep {
            points: [a, b, c, c],
            len: 3,
        }
    }
}

/// The point of segment ab nearest the origin, and the vertices it lies on.
fn segment(a: [f64; 3], b: [f64; 3]) -> ([f64; 3], Keep) {
    let ab = sub(b, a);
    let length = dot(ab, ab);
    let t = -dot(a, ab);
    if !above_zero(length) || t <= 0.0 {
        return (a, Keep::one(a));
    }
    if t >= length {
        return (b, Keep::one(b));
    }
    (add(a, scale(ab, t / length)), Keep::two(a, b))
}

/// The point of triangle abc nearest the origin, and the vertices of the feature it lies on: by
/// the regions of its vertices, edges and face (Ericson, section 5.1.5).
fn triangle(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> ([f64; 3], Keep) {
    let ab = sub(b, a);
    let ac = sub(c, a);
    let d1 = -dot(ab, a);
    let d2 = -dot(ac, a);
    if d1 <= 0.0 && d2 <= 0.0 {
        return (a, Keep::one(a));
    }
    let d3 = -dot(ab, b);
    let d4 = -dot(ac, b);
    if d3 >= 0.0 && d4 <= d3 {
        return (b, Keep::one(b));
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return edge(a, b, d1, d1 - d3);
    }
    let d5 = -dot(ab, c);
    let d6 = -dot(ac, c);
    if d6 >= 0.0 && d5 <= d6 {
        return (c, Keep::one(c));
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return edge(a, c, d2, d2 - d6);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        return edge(b, c, d4 - d3, (d4 - d3) + (d5 - d6));
    }
    let sum = va + vb + vc;
    if !above_zero(sum) {
        // A triangle the doubles see as flat: its nearer end of the longest edge serves.
        return segment(a, if dot(ab, ab) >= dot(ac, ac) { b } else { c });
    }
    let point = add(a, add(scale(ab, vb / sum), scale(ac, vc / sum)));
    (point, Keep::three(a, b, c))
}

/// The point at t / length along edge pq, which the caller found to be nearest the origin.
fn edge(p: [f64; 3], q: [f64; 3], t: f64, length: f64) -> ([f64; 3], Keep) {
    if !above_zero(length) {
        return (p, Keep::one(p));
    }
    (add(p, scale(sub(q, p), t / length)), Keep::two(p, q))
}

/// The point of tetrahedron abcd nearest the origin and the vertices of the feature it lies on;
/// the origin itself, keeping all four, when they enclose it (Ericson, section 5.1.6).
fn tetrahedron([a, b, c, d]: [[f64; 3]; 4]) -> ([f64; 3], Keep) {
    let faces = [(a, b, c, d), (a, c, d, b), (a, d, b, c), (b, d, c, a)];
    let mut best: Option<([f64; 3], Keep, f64)> = None;
    for (p, q, r, opposite) in faces {
        if !outside(p, q, r, opposite) {
            continue;
        }
        let (point, keep) = triangle(p, q, r);
        let size = dot(point, point);
        if best.as_ref().is_none_or(|(_, _, top)| size < *top) {
            best = Some((point, keep, size));
        }
    }
    match best {
        Some((point, keep, _)) => (point, keep),
        None => (
            [0.0; 3],
            Keep {
                points: [a, b, c, d],
                len: 4,
            },
        ),
    }
}

/// Whether the origin lies on the other side of the plane pqr from `opposite`, or the four points
/// are flat to the doubles, so every face is a candidate.
fn outside(p: [f64; 3], q: [f64; 3], r: [f64; 3], opposite: [f64; 3]) -> bool {
    let normal = cross(sub(q, p), sub(r, p));
    let origin = -dot(p, normal);
    let other = dot(sub(opposite, p), normal);
    other == 0.0 || origin * other < 0.0
}

/// A grid point as doubles, exactly.
#[allow(
    clippy::cast_precision_loss,
    reason = "a coordinate is at most 2^40 − 1 in magnitude, which a double holds exactly"
)]
fn double(p: P3) -> [f64; 3] {
    [p.x.get() as f64, p.y.get() as f64, p.z.get() as f64]
}

/// A whole number of magnitude at most 2^53 as a double, exactly.
#[allow(
    clippy::cast_precision_loss,
    reason = "the callers pass whole numbers of magnitude at most 2^20, which a double holds exactly"
)]
fn whole_to_double(x: i64) -> f64 {
    x as f64
}

/// A whole double of magnitude at most [`DIRECTION`] as an integer; `None` beyond it.
#[allow(
    clippy::cast_possible_truncation,
    reason = "the range check keeps |x| at most 2^20, and x is whole, so the integer is exact"
)]
fn whole(x: f64) -> Option<i64> {
    if magnitude(x) <= whole_to_double(DIRECTION) {
        Some(x as i64)
    } else {
        None
    }
}

/// Whether x is above zero: false for zero, a negative and a NaN alike.
fn above_zero(x: f64) -> bool {
    x > 0.0
}

/// |x|, as a comparison: a NaN stays NaN.
fn magnitude(x: f64) -> f64 {
    if x < 0.0 { -x } else { x }
}

fn add([ax, ay, az]: [f64; 3], [bx, by, bz]: [f64; 3]) -> [f64; 3] {
    [ax + bx, ay + by, az + bz]
}

fn sub([ax, ay, az]: [f64; 3], [bx, by, bz]: [f64; 3]) -> [f64; 3] {
    [ax - bx, ay - by, az - bz]
}

fn neg([x, y, z]: [f64; 3]) -> [f64; 3] {
    [-x, -y, -z]
}

fn scale([x, y, z]: [f64; 3], s: f64) -> [f64; 3] {
    [x * s, y * s, z * s]
}

fn dot([ax, ay, az]: [f64; 3], [bx, by, bz]: [f64; 3]) -> f64 {
    ax * bx + ay * by + az * bz
}

fn cross([ax, ay, az]: [f64; 3], [bx, by, bz]: [f64; 3]) -> [f64; 3] {
    [ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx]
}
