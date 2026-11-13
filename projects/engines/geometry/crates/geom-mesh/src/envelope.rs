//! The polyhedral envelope: a body that proves "within ε of these triangles" exactly (19.8).
//!
//! What it does: [`TriangleEnvelope`] is a convex polyhedron around one triangle, its corners on
//! the grid, inscribed in the triangle's ε-offset. [`Envelope`] holds one for each triangle of a
//! surface, behind a box tree. A point inside any of them is within ε of the surface.
//!
//! Guarantees:
//!
//! - **Inside proves within ε.** Each polyhedron is the triangle swept by a cube of half side h, the
//!   largest whole number with 3h² ≤ ε². Its corners are a triangle's vertices moved by (±h, ±h, ±h),
//!   each within ε of the triangle, and the ε-offset of a triangle is convex, so the whole polyhedron
//!   is inside it. A point farther than ε is never inside.
//! - **Within h proves inside.** A point within h of the triangle is inside, since a ball of radius
//!   h fits in the cube. So the envelope is never more than a factor √3 narrower than ε.
//! - **Exact, within 384 bits.** Every test is the sign of a plane at a point: a grid point in `i128`,
//!   a line-plane or three-plane point in [`Plane::side_of_lpi`] and [`Plane::side_of_tpi`], chapter
//!   6.2's deepest tests. The width is absolute, in grid steps, never a fraction of a model's size.
//! - **Refused, never rounded.** A plane over 6.2's budget is an error. That takes an envelope
//!   reaching past the frame on a triangle whose normal is near its largest.
//! - **A whole triangle, proven.** [`covers`] and [`Envelope::covers`] answer whether a triangle
//!   lies inside a union of these bodies, and [`covers_within`] whether the part of it inside one
//!   more body does, never by sampling. `true` is proven by the argument on [`covers_within`]; `false` may also be a triangle that only touches the union's boundary from inside,
//!   so an operation that waits for `true` is never wrong, only sometimes cautious.
//!
//! Method: the polyhedral envelope of Wang et al. (2020), <https://doi.org/10.1145/3386569.3392426>,
//! as 19.8 asks, its corners on the grid
//! (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`). The paper bounds a triangle
//! by a prism with caps; here the same inscribed body is a Minkowski sum, the triangle and a cube,
//! whose facets have closed-form normals: the cube's, the triangle's, and each edge crossed with
//! each axis, on the side away from the third corner (a facet of a Minkowski sum is a sum of
//! faces, here an edge and a cube edge, so on the other side there is none). A body with normal m reaches m · v + h·|m|₁ at its furthest, so each
//! facet's plane is known without a hull. The triangle test asks the paper's points: the triangle's
//! corners, where its edges cross the bodies' facets, and where two facets cross it, each a grid
//! point, a line-plane point or a three-plane point, as 19.8 lists.
//!
//! Cost: 17 planes a triangle at most (both ways along the axes and the triangle's normal, the one
//! way that bounds a face for each edge crossed with an axis), each one or two greatest common
//! divisors. A containment test is a tree query and up to 17 signs for each triangle whose box
//! holds the point. A triangle test against k bodies that meet it, l of whose facets cross it,
//! makes at most 3 + 3l + l²/2 points and up to 17k signs for each, most decided by intervals.

#![allow(
    clippy::many_single_char_names,
    reason = "the names of the geometry: a triangle's corners, a normal's parts, a point"
)]

use alloc::vec::Vec;

use geom_bvh::{Aabb, BoxTree, BvhError, Entry};
use geom_math::gcd_u128;
use geom_num::grid::P3;
use geom_num::interval::Interval;
use geom_num::plane::{Lpi, Plane, Tpi};
use geom_num::wide::{I256, Wide};
use geom_num::{NumError, Sign};
use geom_run::{Exec, Stopped, WorkMeter};

/// Why an envelope could not be made or asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvelopeError {
    /// A plane is over the budget of 6.2, or the numbers layer refused.
    Num(NumError),
    /// The box tree refused.
    Bvh(BvhError),
    /// The work budget ran out, or the run was cancelled.
    Stopped(Stopped),
    /// More triangles than a tree indexes.
    Full,
}

impl core::fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EnvelopeError::Num(e) => write!(f, "{e}"),
            EnvelopeError::Bvh(e) => write!(f, "{e}"),
            EnvelopeError::Stopped(e) => write!(f, "{e}"),
            EnvelopeError::Full => f.write_str("more triangles than a tree indexes"),
        }
    }
}

impl core::error::Error for EnvelopeError {}

impl From<NumError> for EnvelopeError {
    fn from(e: NumError) -> Self {
        EnvelopeError::Num(e)
    }
}

impl From<BvhError> for EnvelopeError {
    fn from(e: BvhError) -> Self {
        EnvelopeError::Bvh(e)
    }
}

impl From<Stopped> for EnvelopeError {
    fn from(e: Stopped) -> Self {
        EnvelopeError::Stopped(e)
    }
}

/// The half side of the cube that fits an envelope of width `epsilon`: the largest h with
/// 3h² ≤ ε², so that (±h, ±h, ±h) is within ε of the origin.
#[must_use]
pub fn half_side(epsilon: u64) -> u64 {
    let e = u128::from(epsilon);
    let h = e.saturating_mul(e).checked_div(3).unwrap_or(0).isqrt();
    u64::try_from(h).unwrap_or(0)
}

/// Coordinate `k` of a point.
fn coordinate(p: P3, k: usize) -> i128 {
    i128::from(match k {
        0 => p.x.get(),
        1 => p.y.get(),
        _ => p.z.get(),
    })
}

/// `b − a`.
fn run(a: P3, b: P3) -> [i128; 3] {
    let part = |k: usize| coordinate(b, k).wrapping_sub(coordinate(a, k));
    [part(0), part(1), part(2)]
}

/// u × v of differences: each component below 2^83, so wrapping steps are exact.
fn cross(u: [i128; 3], v: [i128; 3]) -> [i128; 3] {
    let [a, b, c] = u;
    let [x, y, z] = v;
    let term =
        |p: i128, q: i128, r: i128, s: i128| p.wrapping_mul(q).wrapping_sub(r.wrapping_mul(s));
    [term(b, z, c, y), term(c, x, a, z), term(a, y, b, x)]
}

/// `m` over its components' common divisor, or `None` for a zero vector.
fn primitive(m: [i128; 3]) -> Option<[i128; 3]> {
    let [x, y, z] = m.map(i128::unsigned_abs);
    let g = gcd_u128(gcd_u128(x, y), z);
    if g == 0 {
        return None;
    }
    let by = i128::try_from(g).ok()?;
    Some(m.map(|v| v.checked_div(by).unwrap_or(0)))
}

/// The plane of the facet with outward normal `m`: n · x + d = 0 with d = −(max m·v + h·|m|₁).
/// A point is on or under it when its side is not `Positive`.
fn facet(m: [i128; 3], t: [P3; 3], h: u64) -> Result<Plane, NumError> {
    let reach = |v: P3| -> Option<i128> {
        let [a, b, c] = m;
        let dot = a
            .checked_mul(coordinate(v, 0))?
            .checked_add(b.checked_mul(coordinate(v, 1))?)?
            .checked_add(c.checked_mul(coordinate(v, 2))?)?;
        Some(dot)
    };
    let [p, q, r] = t;
    let furthest = reach(p)
        .zip(reach(q))
        .zip(reach(r))
        .map(|((x, y), z)| x.max(y).max(z))
        .ok_or(NumError::BudgetExceeded)?;
    let [a, b, c] = m.map(i128::unsigned_abs);
    let width = a
        .checked_add(b)
        .and_then(|s| s.checked_add(c))
        .and_then(|s| i128::try_from(s).ok())
        .and_then(|s| s.checked_mul(i128::from(h)))
        .ok_or(NumError::BudgetExceeded)?;
    let offset = furthest
        .checked_add(width)
        .and_then(i128::checked_neg)
        .ok_or(NumError::BudgetExceeded)?;
    Plane::new(m, offset)
}

/// The low and high corners of the box around `t`, `pad` clear of it on every side.
fn reach(t: [P3; 3], pad: i64) -> ([i64; 3], [i64; 3]) {
    let [a, b, c] = t;
    let axis = |x: i64, y: i64, z: i64| {
        (
            x.min(y).min(z).saturating_sub(pad),
            x.max(y).max(z).saturating_add(pad),
        )
    };
    let (low_x, high_x) = axis(a.x.get(), b.x.get(), c.x.get());
    let (low_y, high_y) = axis(a.y.get(), b.y.get(), c.y.get());
    let (low_z, high_z) = axis(a.z.get(), b.z.get(), c.z.get());
    ([low_x, low_y, low_z], [high_x, high_y, high_z])
}

/// A convex polyhedron with grid corners around one triangle, inscribed in its ε-offset.
#[derive(Clone, Debug)]
pub struct TriangleEnvelope {
    planes: Vec<Plane>,
    /// Each plane's coefficients as intervals, for the triangle test's filter.
    approx: Vec<Approx>,
    /// The triangle it was made round.
    t: [P3; 3],
    /// The cube's half side.
    h: u64,
}

/// Bodies are equal when made round the same triangle with the same cube: the planes and their
/// intervals follow from those.
impl PartialEq for TriangleEnvelope {
    fn eq(&self, other: &Self) -> bool {
        self.t == other.t && self.h == other.h
    }
}

impl Eq for TriangleEnvelope {}

/// A plane's coefficients n and d as intervals.
type Approx = [Interval; 4];

/// The interval round a wide integer, built from its 32-bit digits, each exact in a double.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] for the one value with no negation, and as [`Interval`]'s steps
/// report.
pub(crate) fn enclose<const L: usize>(v: &Wide<L>) -> Result<Interval, NumError> {
    let negative = v.is_negative();
    let magnitude = if negative {
        v.checked_neg().ok_or(NumError::BudgetExceeded)?
    } else {
        *v
    };
    let radix = Interval::point(4_294_967_296.0)?;
    let mut sum = Interval::point(0.0)?;
    for limb in magnitude.to_le_limbs().iter().rev() {
        let (high, low) = (limb >> 32, limb & 0xffff_ffff);
        #[allow(
            clippy::cast_precision_loss,
            reason = "each is below 2^32, exact in a double"
        )]
        for digit in [high as f64, low as f64] {
            sum = sum.mul(radix)?.add(Interval::point(digit)?)?;
        }
    }
    Ok(if negative { sum.neg() } else { sum })
}

/// The interval round an `i128`.
fn enclose_i128(v: i128) -> Result<Interval, NumError> {
    enclose(&I256::from_i128(v).resize::<2>().ok_or(NumError::BudgetExceeded)?)
}

/// A plane's coefficients as intervals.
fn approximate(plane: &Plane) -> Result<Approx, NumError> {
    let [a, b, c] = plane.n();
    Ok([
        enclose_i128(a)?,
        enclose_i128(b)?,
        enclose_i128(c)?,
        enclose_i128(plane.d())?,
    ])
}

impl TriangleEnvelope {
    /// The triangle `t` swept by the cube of half side `h`. A triangle with no area, or a repeated
    /// corner, gives the same body with the facets of what remains.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] for a plane over 6.2's budget.
    pub fn new(t: [P3; 3], h: u64) -> Result<Self, NumError> {
        let [a, b, c] = t;
        let axes: [[i128; 3]; 3] = [[1, 0, 0], [0, 1, 0], [0, 0, 1]];
        // Each normal with the directions to keep: both for the axes and the triangle's normal.
        let mut normals: Vec<([i128; 3], bool, bool)> = Vec::with_capacity(13);
        normals.extend(axes.map(|n| (n, true, true)));
        normals.push((cross(run(a, b), run(a, c)), true, true));
        // An edge crossed with an axis bounds a face only on the side away from the third corner:
        // on the other side the triangle touches the plane at that corner alone, so the plane
        // meets the body in a segment and bounds nothing the other planes do not.
        for (p, q, r) in [(a, b, c), (b, c, a), (c, a, b)] {
            for axis in axes {
                let n = cross(run(p, q), axis);
                // Each part of n is below 2^42 and of r − p below 2^41: the dot fits.
                let [x, y, z] = run(p, r);
                let behind = n[0]
                    .wrapping_mul(x)
                    .wrapping_add(n[1].wrapping_mul(y))
                    .wrapping_add(n[2].wrapping_mul(z));
                normals.push((n, behind <= 0, behind >= 0));
            }
        }
        let mut planes = Vec::with_capacity(26);
        for (n, forward, backward) in normals {
            if let Some(m) = primitive(n) {
                if forward {
                    planes.push(facet(m, t, h)?);
                }
                if backward {
                    planes.push(facet(m.map(i128::wrapping_neg), t, h)?);
                }
            }
        }
        // A normal met twice (an edge crossed with an axis along the triangle's normal, say)
        // gives the same plane twice.
        planes.sort_unstable_by_key(|plane| (plane.n(), plane.d()));
        planes.dedup();
        let approx = planes.iter().map(approximate).collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            planes,
            approx,
            t,
            h,
        })
    }

    /// The facets' planes, each looking outward.
    #[must_use]
    pub fn planes(&self) -> &[Plane] {
        &self.planes
    }

    /// Whether grid point `p` is inside or on the polyhedron.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`], which planes and points within budget cannot reach.
    pub fn contains(&self, p: P3) -> Result<bool, NumError> {
        for plane in &self.planes {
            if plane.side(p)? == Sign::Positive {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Whether a three-plane point is inside or on the polyhedron.
    ///
    /// # Errors
    ///
    /// As for [`Plane::side_of_tpi`].
    pub fn contains_tpi(&self, p: &Tpi) -> Result<bool, NumError> {
        for plane in &self.planes {
            if plane.side_of_tpi(p)? == Sign::Positive {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Whether a line-plane point is inside or on the polyhedron.
    ///
    /// # Errors
    ///
    /// As for [`Plane::side_of_lpi`].
    pub fn contains_lpi(&self, p: &Lpi) -> Result<bool, NumError> {
        for plane in &self.planes {
            if plane.side_of_lpi(p)? == Sign::Positive {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// The low and high corners of the polyhedron's box.
    pub(crate) fn bounds(&self) -> ([i64; 3], [i64; 3]) {
        reach(self.t, i64::try_from(self.h).unwrap_or(i64::MAX))
    }

    /// Whether the two polyhedra's boxes meet: `false` proves they do not.
    fn reaches_body(&self, other: &TriangleEnvelope) -> bool {
        let (low, high) = other.bounds();
        self.reaches(low, high)
    }

    /// Whether the polyhedron's box meets the box from `low` to `high`: `false` proves the
    /// polyhedron is apart from everything in that box.
    #[must_use]
    pub fn reaches(&self, low: [i64; 3], high: [i64; 3]) -> bool {
        let (l, h) = self.bounds();
        l.iter().zip(&high).all(|(a, b)| a <= b) && low.iter().zip(&h).all(|(a, b)| a <= b)
    }

    /// Whether the polyhedron lies strictly on one side of `plane`, so no point of `plane` is in
    /// it: over its corners, the triangle's moved by (±h, ±h, ±h), n · x + d ranges over
    /// [min − h·|n|₁, max + h·|n|₁], with min and max over the triangle's corners. `false` when a
    /// step does not fit, which only costs the caller a point it could have skipped.
    fn misses(&self, plane: &Plane) -> bool {
        let n = plane.n();
        let at = |p: P3| -> Option<i128> {
            n[0].checked_mul(coordinate(p, 0))?
                .checked_add(n[1].checked_mul(coordinate(p, 1))?)?
                .checked_add(n[2].checked_mul(coordinate(p, 2))?)?
                .checked_add(plane.d())
        };
        let width = n
            .iter()
            .try_fold(0_i128, |s, v| s.checked_add(i128::try_from(v.unsigned_abs()).ok()?))
            .and_then(|s| s.checked_mul(i128::from(self.h)));
        let [a, b, c] = self.t;
        let (Some(x), Some(y), Some(z), Some(w)) = (at(a), at(b), at(c), width) else {
            return false;
        };
        let (low, high) = (x.min(y).min(z), x.max(y).max(z));
        low.checked_sub(w).is_some_and(|l| l > 0) || high.checked_add(w).is_some_and(|h| h < 0)
    }

    /// Whether triangle `t` may meet the polyhedron: `false` proves it does not, since a facet
    /// then has every corner of `t` outside it.
    ///
    /// # Errors
    ///
    /// As for [`Plane::side`].
    pub fn may_meet(&self, t: [P3; 3]) -> Result<bool, NumError> {
        Ok(self.against(t)? != Against::Apart)
    }

    /// Where triangle `t` lies against the polyhedron: wholly inside (its corners are, and the body
    /// is convex), apart (one facet has every corner outside), or neither.
    fn against(&self, t: [P3; 3]) -> Result<Against, NumError> {
        let mut whole = true;
        for plane in &self.planes {
            let [a, b, c] = t;
            let sides = [plane.side(a)?, plane.side(b)?, plane.side(c)?];
            if sides.iter().all(|s| *s == Sign::Positive) {
                return Ok(Against::Apart);
            }
            if sides.contains(&Sign::Positive) {
                whole = false;
            }
        }
        Ok(if whole { Against::Whole } else { Against::Partly })
    }

    /// Whether `p` is inside or on the polyhedron, or, `strictly`, inside and on no facet.
    fn holds(&self, p: &Asked, strictly: bool) -> Result<bool, NumError> {
        if p.outside(self.bounds()) {
            return Ok(false);
        }
        for (plane, approx) in self.planes.iter().zip(&self.approx) {
            match p.side(plane, approx)? {
                Sign::Positive => return Ok(false),
                Sign::Zero if strictly => return Ok(false),
                _ => {}
            }
        }
        Ok(true)
    }
}

/// Where a triangle lies against one body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Against {
    /// Every point of the triangle is in the body.
    Whole,
    /// No point of the triangle is in the body.
    Apart,
    /// Undecided by the corners alone.
    Partly,
}

/// A point the triangle test asks about: a grid point, a line-plane point or a three-plane point.
#[derive(Clone, Copy, Debug)]
enum Point {
    /// A corner of the triangle.
    Grid(P3),
    /// Where an edge of the triangle crosses a facet.
    Line(Lpi),
    /// Where two facets cross the triangle's plane.
    Planes(Tpi),
}

impl Point {
    /// Which side of `plane` the point is on, exactly.
    fn side(&self, plane: &Plane) -> Result<Sign, NumError> {
        match self {
            Point::Grid(p) => plane.side(*p),
            Point::Line(l) => plane.side_of_lpi(l),
            Point::Planes(t) => plane.side_of_tpi(t),
        }
    }
}

/// Intervals round the point where three planes meet, by Cramer's rule on the planes'
/// coefficients as intervals (the formula on [`Tpi::homogeneous`]): `None` where the weight's
/// interval holds zero or a step fails, and the exact coordinates are needed.
fn cramer(planes: [&Approx; 3]) -> Option<[Interval; 3]> {
    let [[a1, b1, c1, d1], [a2, b2, c2, d2], [a3, b3, c3, d3]] = planes.map(|p| *p);
    let cross = |[x1, y1, z1]: [Interval; 3], [x2, y2, z2]: [Interval; 3]| -> Option<[Interval; 3]> {
        Some([
            y1.mul(z2).ok()?.sub(z1.mul(y2).ok()?).ok()?,
            z1.mul(x2).ok()?.sub(x1.mul(z2).ok()?).ok()?,
            x1.mul(y2).ok()?.sub(y1.mul(x2).ok()?).ok()?,
        ])
    };
    let (n1, n2, n3) = ([a1, b1, c1], [a2, b2, c2], [a3, b3, c3]);
    let (u, v, w) = (cross(n2, n3)?, cross(n3, n1)?, cross(n1, n2)?);
    let dot = |[x1, y1, z1]: [Interval; 3], [x2, y2, z2]: [Interval; 3]| -> Option<Interval> {
        x1.mul(x2).ok()?.add(y1.mul(y2).ok()?).ok()?.add(z1.mul(z2).ok()?).ok()
    };
    let weight = dot(n1, u)?;
    weight.certain_sign().filter(|s| *s != Sign::Zero)?;
    let part = |k: usize| -> Option<Interval> {
        let sum = d1
            .mul(*u.get(k)?)
            .ok()?
            .add(d2.mul(*v.get(k)?).ok()?)
            .ok()?
            .add(d3.mul(*w.get(k)?).ok()?)
            .ok()?;
        sum.neg().div(weight).ok()
    };
    Some([part(0)?, part(1)?, part(2)?])
}

/// A point asked about, with intervals round its coordinates where it is not a grid point: a
/// sign the intervals decide is the exact sign, since they hold the exact values; a sign they
/// leave open is decided exactly.
#[derive(Clone, Copy, Debug)]
struct Asked {
    point: Point,
    at: Option<[Interval; 3]>,
}

impl Asked {
    /// `point`, with its coordinates' intervals: for a three-plane point by Cramer's rule on
    /// `approx`, its planes' intervals, when given and decisive; otherwise X / w from its exact
    /// homogeneous coordinates.
    fn new(point: Point, approx: Option<[&Approx; 3]>) -> Result<Asked, NumError> {
        let divide = |x: Interval, w: Interval| x.div(w).ok();
        let at = match point {
            Point::Grid(_) => None,
            Point::Line(l) => {
                let ([x, y, z], w) = l.homogeneous()?;
                let w = enclose_i128(w)?;
                divide(enclose(&x)?, w)
                    .zip(divide(enclose(&y)?, w))
                    .zip(divide(enclose(&z)?, w))
                    .map(|((x, y), z)| [x, y, z])
            }
            Point::Planes(t) => {
                if let Some(at) = approx.and_then(cramer) {
                    Some(at)
                } else {
                    let ([x, y, z], w) = t.homogeneous()?;
                    let w = enclose(&w)?;
                    divide(enclose(&x)?, w)
                        .zip(divide(enclose(&y)?, w))
                        .zip(divide(enclose(&z)?, w))
                        .map(|((x, y), z)| [x, y, z])
                }
            }
        };
        Ok(Asked { point, at })
    }

    /// Whether the point is certainly outside the box from `low` to `high`.
    fn outside(&self, (low, high): ([i64; 3], [i64; 3])) -> bool {
        #[allow(
            clippy::cast_precision_loss,
            reason = "a box corner below 2^41 in magnitude, a double exactly"
        )]
        let past = |v: Interval, l: i64, h: i64| v.hi() < l as f64 || v.lo() > h as f64;
        match (self.point, self.at) {
            (Point::Grid(p), _) => [p.x.get(), p.y.get(), p.z.get()]
                .iter()
                .zip(low.iter().zip(&high))
                .any(|(v, (l, h))| v < l || v > h),
            (_, Some(at)) => at
                .iter()
                .zip(low.iter().zip(&high))
                .any(|(v, (l, h))| past(*v, *l, *h)),
            _ => false,
        }
    }

    /// Which side of `plane` the point is on: by the intervals when they decide a strict sign,
    /// exactly otherwise.
    fn side(&self, plane: &Plane, approx: &Approx) -> Result<Sign, NumError> {
        if let Some([x, y, z]) = self.at {
            let [a, b, c, d] = *approx;
            let value = a
                .mul(x)
                .and_then(|ax| ax.add(b.mul(y)?))
                .and_then(|s| s.add(c.mul(z)?))
                .and_then(|s| s.add(d));
            if let Ok(Some(sign)) = value.map(Interval::certain_sign)
                && sign != Sign::Zero
            {
                return Ok(sign);
            }
        }
        self.point.side(plane)
    }
}

/// Whether some body holds `p` strictly inside, so that a neighbourhood of `p` is covered. The
/// bodies numbered in `on`, whose facets `p` lies on, cannot, and are not asked.
fn held(
    bodies: &[&TriangleEnvelope],
    p: &Asked,
    on: [usize; 2],
    meter: &mut WorkMeter<'_>,
) -> Result<bool, EnvelopeError> {
    for (k, body) in bodies.iter().enumerate() {
        if on.contains(&k) {
            continue;
        }
        meter.charge(1)?;
        if body.holds(p, true)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// m · p + d = 0 through grid point `p` with normal `m`, `m` below 2^42 in each part: each product
/// is below 2^82, so every step fits `i128`, and every step is checked all the same.
fn through(m: [i128; 3], p: P3) -> Result<Plane, NumError> {
    let [x, y, z] = m;
    let d = x
        .checked_mul(coordinate(p, 0))
        .zip(y.checked_mul(coordinate(p, 1)))
        .zip(z.checked_mul(coordinate(p, 2)))
        .and_then(|((a, b), c)| a.checked_add(b)?.checked_add(c)?.checked_neg())
        .ok_or(NumError::BudgetExceeded)?;
    Plane::new(m, d)
}

/// The three planes through the edges of triangle `t`, square to the coordinate plane its normal
/// `n` is not parallel to, each looking away from the triangle: a point of the triangle's plane is
/// in the triangle exactly when it is on no plane's positive side, because the projection along
/// that axis maps the triangle's plane one to one onto the coordinate plane.
fn rim(t: [P3; 3], n: [i128; 3]) -> Result<[Plane; 3], NumError> {
    let axis: [i128; 3] = match n {
        [x, _, _] if x != 0 => [1, 0, 0],
        [_, y, _] if y != 0 => [0, 1, 0],
        _ => [0, 0, 1],
    };
    let [a, b, c] = t;
    let side = |p: P3, q: P3, r: P3| -> Result<Plane, NumError> {
        // (q − p) × axis: the edge is not parallel to the axis, or the projection would be flat.
        let m = cross(run(p, q), axis);
        let plane = through(m, p)?;
        match plane.side(r)? {
            Sign::Negative => Ok(plane),
            Sign::Positive => through(m.map(i128::wrapping_neg), p),
            Sign::Zero => Err(NumError::Degenerate),
        }
    };
    Ok([side(a, b, c)?, side(b, c, a)?, side(c, a, b)?])
}

/// Whether triangle `t` lies inside the union of `bodies`: every point of it in at least one.
/// [`covers_within`] with no region.
///
/// # Errors
///
/// As for [`covers_within`].
pub fn covers(
    bodies: &[&TriangleEnvelope],
    t: [P3; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<bool, EnvelopeError> {
    covers_within(bodies, t, None, meter)
}

/// Whether the part of triangle `t` inside body `within` (all of `t` when `None`) lies inside the
/// union of `bodies`: every point of it in at least one.
///
/// `true` is proven, `false` is cautious: it is also the answer for some parts that lie in the
/// union but touch its boundary at one of the points below.
///
/// Proof. Let C be the part, a closed convex polygon in the triangle's plane bounded by lines of
/// `t`'s edges and of `within`'s facets; let U be the union and `Q_k` each body's cut by that
/// plane, a convex polygon bounded by lines where the body's facets cross it. Suppose every point
/// the test asks about lies strictly inside some body: each crossing, inside C, of two of C's
/// boundary lines; each crossing, inside C, of one of them with a facet line of a body i, inside
/// i; and each crossing, inside C, of facet lines of bodies i and j, inside i and j. Suppose too a
/// point of C is outside U. The points of C outside U form a set open in C, so its closure W has
/// an extreme point v of its convex hull, with v in W. The cone of W at v holds no line through v,
/// so it has two boundary rays on two different lines. Next to such a ray, on its outer side, lie
/// points of C outside W, or no points of C. In the second case the ray runs along a boundary line
/// of C. In the first those points are in U, so in some `Q_i`, while the points just inside the
/// ray are in no `Q_i`; a cell of the lines' arrangement lies wholly inside or outside each
/// `Q_i`, so the ray runs along a facet line of body i, and, `Q_i` being closed and finitely many,
/// v is in `Q_i` for one such i. So v is one of the points the test asks about. But every
/// neighbourhood of v meets points outside U, so no body holds v strictly inside. That contradicts
/// the test passing, so C lies in U.
///
/// # Errors
///
/// [`EnvelopeError::Num`] with [`NumError::Degenerate`] for a triangle with no area that no one
/// body holds, and as the predicates report; [`EnvelopeError::Stopped`] when the budget runs out.
#[allow(
    clippy::similar_names,
    clippy::too_many_lines,
    reason = "the names of the proof: bodies i and j, facets f and g; the proof's three kinds of \
              point, asked in turn"
)]
pub fn covers_within(
    bodies: &[&TriangleEnvelope],
    t: [P3; 3],
    within: Option<&TriangleEnvelope>,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, EnvelopeError> {
    if let Some(region) = within
        && region.against(t)? == Against::Apart
    {
        return Ok(true);
    }
    let mut near: Vec<&TriangleEnvelope> = Vec::with_capacity(bodies.len());
    for &body in bodies {
        meter.charge(1)?;
        if within.is_some_and(|region| !region.reaches_body(body)) {
            continue;
        }
        match body.against(t)? {
            Against::Whole => return Ok(true),
            Against::Apart => {}
            Against::Partly => near.push(body),
        }
    }
    if near.is_empty() {
        return Ok(false);
    }
    let [a, b, c] = t;
    let base = Plane::through(a, b, c)?;
    let rims = rim(t, base.n())?;
    let rims = [
        (rims[0], approximate(&rims[0])?),
        (rims[1], approximate(&rims[1])?),
        (rims[2], approximate(&rims[2])?),
    ];
    // Whether a point of the triangle's plane is in C.
    let in_part = |p: &Asked, on_rim: bool| -> Result<bool, NumError> {
        if !on_rim {
            for (edge, approx) in &rims {
                if p.side(edge, approx)? == Sign::Positive {
                    return Ok(false);
                }
            }
        }
        within.map_or(Ok(true), |region| region.holds(p, false))
    };
    // A facet crosses the triangle when its corners are not all on one side and the triangle is
    // not in its plane.
    let crosses = |plane: &Plane| -> Result<bool, NumError> {
        let sides = [plane.side(a)?, plane.side(b)?, plane.side(c)?];
        Ok(
            sides.contains(&Sign::Positive) && sides.contains(&Sign::Negative)
                || sides.contains(&Sign::Zero) && sides.iter().any(|s| *s != Sign::Zero),
        )
    };
    let mut cut: Vec<(Plane, Approx)> = Vec::new();
    if let Some(region) = within {
        for (plane, approx) in region.planes.iter().zip(&region.approx) {
            if crosses(plane)? {
                cut.push((*plane, *approx));
            }
        }
    }
    let mut lines: Vec<(usize, Plane, Approx)> = Vec::new();
    for (i, body) in near.iter().enumerate() {
        for (plane, approx) in body.planes.iter().zip(&body.approx) {
            if crosses(plane)? && within.is_none_or(|region| !region.misses(plane)) {
                lines.push((i, *plane, *approx));
            }
        }
    }
    let edges = [(a, b), (b, c), (c, a)];
    // An edge's crossing with a plane, strictly inside the edge: a crossing at a corner is the
    // corner, asked on its own.
    let crossing = |p: P3, q: P3, plane: &Plane| -> Result<Option<Asked>, NumError> {
        let (sp, sq) = (plane.side(p)?, plane.side(q)?);
        if sp == Sign::Zero || sq == Sign::Zero || sp == sq {
            return Ok(None);
        }
        Ok(Some(Asked::new(Point::Line(Lpi::new(p, q, *plane)?), None)?))
    };
    let base_approx = approximate(&base)?;
    let meeting = |(f, fa): (&Plane, &Approx),
                   (g, ga): (&Plane, &Approx)|
     -> Result<Option<Asked>, NumError> {
        match Tpi::meeting(base, *f, *g) {
            Ok(tpi) => Ok(Some(Asked::new(
                Point::Planes(tpi),
                Some([&base_approx, fa, ga]),
            )?)),
            // Parallel lines, or one line twice: no point.
            Err(NumError::Degenerate) => Ok(None),
            Err(e) => Err(e),
        }
    };
    let none = [usize::MAX; 2];
    // C's corners: the triangle's, its edges' crossings with the region's facets, and two of the
    // region's facet lines' crossings.
    for p in t {
        let point = Asked::new(Point::Grid(p), None)?;
        if in_part(&point, true)? && !held(&near, &point, none, meter)? {
            return Ok(false);
        }
    }
    for (f, _) in &cut {
        for (p, q) in edges {
            meter.charge(1)?;
            if let Some(point) = crossing(p, q, f)?
                && in_part(&point, true)?
                && !held(&near, &point, none, meter)?
            {
                return Ok(false);
            }
        }
    }
    for (k, (f, fa)) in cut.iter().enumerate() {
        for (g, ga) in cut.get(k.saturating_add(1)..).unwrap_or(&[]) {
            meter.charge(1)?;
            if let Some(point) = meeting((f, fa), (g, ga))?
                && in_part(&point, false)?
                && !held(&near, &point, none, meter)?
            {
                return Ok(false);
            }
        }
    }
    // C's sides crossing the bodies' facet lines.
    for (i, plane, plane_approx) in &lines {
        let (i, plane) = (*i, *plane);
        let Some(body) = near.get(i) else {
            continue;
        };
        for (p, q) in edges {
            meter.charge(1)?;
            if let Some(point) = crossing(p, q, &plane)?
                && body.holds(&point, false)?
                && in_part(&point, true)?
                && !held(&near, &point, [i, i], meter)?
            {
                return Ok(false);
            }
        }
        for (f, fa) in &cut {
            meter.charge(1)?;
            if let Some(point) = meeting((f, fa), (&plane, plane_approx))?
                && body.holds(&point, false)?
                && in_part(&point, false)?
                && !held(&near, &point, [i, i], meter)?
            {
                return Ok(false);
            }
        }
    }
    // Two bodies' facet lines crossing in C. They can meet at a point inside both bodies only if
    // each facet's plane meets the other's body, and the two bodies' boxes meet.
    for (i, body_i) in near.iter().enumerate() {
        for (j, body_j) in near.iter().enumerate().skip(i) {
            if !body_i.reaches_body(body_j) {
                continue;
            }
            let of = |k: usize, other: &TriangleEnvelope| -> Vec<(Plane, Approx)> {
                lines
                    .iter()
                    .filter(|(b, plane, _)| *b == k && (i == j || !other.misses(plane)))
                    .map(|(_, plane, approx)| (*plane, *approx))
                    .collect()
            };
            let (fs, gs) = (of(i, body_j), of(j, body_i));
            for (k, (f, fa)) in fs.iter().enumerate() {
                let start = if i == j { k.saturating_add(1) } else { 0 };
                for (g, ga) in gs.get(start..).unwrap_or(&[]) {
                    meter.charge(1)?;
                    if let Some(point) = meeting((f, fa), (g, ga))?
                        && body_i.holds(&point, false)?
                        && body_j.holds(&point, false)?
                        && in_part(&point, false)?
                        && !held(&near, &point, [i, j], meter)?
                    {
                        return Ok(false);
                    }
                }
            }
        }
    }
    Ok(true)
}

/// The envelope of a surface: one polyhedron to a triangle, and a tree of their boxes.
#[derive(Debug)]
pub struct Envelope {
    triangles: Vec<TriangleEnvelope>,
    tree: BoxTree<3>,
    /// How far a box reaches past its triangle.
    h: u64,
}

impl Envelope {
    /// The envelope of width `epsilon`, in grid steps, around `triangles`.
    ///
    /// # Errors
    ///
    /// [`EnvelopeError::Num`] for a plane over budget, [`EnvelopeError::Full`] and
    /// [`EnvelopeError::Stopped`] as they say, and [`EnvelopeError::Bvh`] for a box the tree
    /// refuses.
    pub fn new(
        triangles: &[[P3; 3]],
        epsilon: u64,
        exec: &Exec<'_>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Self, EnvelopeError> {
        let h = half_side(epsilon);
        let pad = i64::try_from(h).map_err(|_| NumError::BudgetExceeded)?;
        let mut bodies = Vec::with_capacity(triangles.len());
        let mut entries = Vec::with_capacity(triangles.len());
        for (t, key) in triangles.iter().zip(0_u64..) {
            meter.charge(1)?;
            bodies.push(TriangleEnvelope::new(*t, h)?);
            let (low, high) = reach(*t, pad);
            entries.push(Entry {
                key,
                bbox: Aabb::new(low, high)?,
            });
        }
        let tree = BoxTree::build(&entries, exec, meter)?;
        Ok(Self {
            triangles: bodies,
            tree,
            h,
        })
    }

    /// The half side of the envelope's cube: every point this close to the surface is inside.
    #[must_use]
    pub const fn half_side(&self) -> u64 {
        self.h
    }

    /// The keys (positions in the list it was made from) of the triangles whose bodies' boxes meet
    /// the box from `low` to `high`: every triangle whose body meets the box is among them.
    ///
    /// # Errors
    ///
    /// As for the tree's query.
    pub fn near_keys(
        &self,
        low: [i64; 3],
        high: [i64; 3],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<u64>, EnvelopeError> {
        Ok(self.tree.overlapping(&Aabb::new(low, high)?, meter)?)
    }

    /// Whether triangle `t` is inside the envelope, so its every point within ε of the surface:
    /// [`covers`] over the bodies whose boxes meet the triangle's.
    ///
    /// # Errors
    ///
    /// As for [`covers`] and the tree's query.
    pub fn covers(&self, t: [P3; 3], meter: &mut WorkMeter<'_>) -> Result<bool, EnvelopeError> {
        let (low, high) = reach(t, 0);
        let bodies = self.near(low, high, meter)?;
        covers(&bodies, t, meter)
    }

    /// The bodies whose boxes hold the box from `low` to `high`.
    fn near(
        &self,
        low: [i64; 3],
        high: [i64; 3],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<&TriangleEnvelope>, EnvelopeError> {
        let keys = self.tree.overlapping(&Aabb::new(low, high)?, meter)?;
        let mut out = Vec::with_capacity(keys.len());
        for key in keys {
            let i = usize::try_from(key).map_err(|_| EnvelopeError::Full)?;
            if let Some(body) = self.triangles.get(i) {
                out.push(body);
            }
        }
        Ok(out)
    }

    /// Whether grid point `p` is inside the envelope, so within ε of the surface.
    ///
    /// # Errors
    ///
    /// As for [`TriangleEnvelope::contains`] and the tree's query.
    pub fn contains(&self, p: P3, meter: &mut WorkMeter<'_>) -> Result<bool, EnvelopeError> {
        let at = [p.x.get(), p.y.get(), p.z.get()];
        for body in self.near(at, at, meter)? {
            meter.charge(1)?;
            if body.contains(p)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Whether a three-plane point is inside the envelope.
    ///
    /// # Errors
    ///
    /// As for [`TriangleEnvelope::contains_tpi`] and the tree's query.
    pub fn contains_tpi(&self, p: &Tpi, meter: &mut WorkMeter<'_>) -> Result<bool, EnvelopeError> {
        // Rounding moves a coordinate at most half a step, so one step's padding holds the point.
        let c = p.to_grid()?.value();
        let at = [c.x.get(), c.y.get(), c.z.get()];
        let (low, high) = (
            at.map(|v| v.saturating_sub(1)),
            at.map(|v| v.saturating_add(1)),
        );
        for body in self.near(low, high, meter)? {
            meter.charge(1)?;
            if body.contains_tpi(p)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
