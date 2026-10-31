//! Certified mass properties of flat polygons whose corners may be implicit points (10.2, 19.2;
//! piece G102).
//!
//! What it does: a [`Corner`] is a grid point or an implicit point: three planes (`Tpi`) or a line
//! and a plane (`Lpi`), kept exactly (6.3). [`loop_area`] gives twice a loop's area vector and
//! [`shell_volume`] six times the signed volume of a closed shell given as its loops, each as a
//! certified interval. Each term of a sum is an exact fraction; adding fractions with different
//! denominators needs integers that grow without bound (19.2), so each term is rounded outward and
//! the intervals are added. Bodies hold grid vertices only until G134 brings implicit ones, which
//! `crate::query`'s exact sums cannot take; these functions take the corners as a list so that
//! piece has them ready.
//!
//! Guarantees:
//!
//! - **Contains the true value.** Every term's numerator and denominator are computed exactly, each
//!   enclosed by the doubles either side of its nearest double, and divided in outward-rounded
//!   interval arithmetic; the sum adds in the same arithmetic. So the interval holds the exact
//!   rational value, within a few units in the last place per term.
//! - **Exact to the budget.** A corner's homogeneous coordinates are below 2^294 and its weight
//!   below 2^252 (`geom_num::plane`), so a volume term's numerator, six products of three
//!   coordinates, is below 2^885, and its denominator, three weights, below 2^756; every partial
//!   result fits the 1024 bits used here, and every step is checked. [`QueryError::Overflow`]
//!   reports what cannot happen, never a wrong value.
//! - **Charged, nothing allocated.** One unit of work per corner visited.
//!
//! Method: chapter 19.2's mass properties for implicit vertices
//! (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`): the sums of
//! `crate::query` (Goldman, Graphics Gems II, doi:10.1016/B978-0-08-050754-5.50043-8) on
//! homogeneous coordinates, with `geom_num::ExactSum`'s correctly rounded sum (Shewchuk, "Adaptive
//! Precision Floating-Point Arithmetic", doi:10.1007/PL00009321) giving each integer's nearest
//! double, and `geom_num::interval`'s outward rounding.
//!
//! Cost: linear in the corners; 16 limbs per product, about a thousand limb multiplications per
//! term. Nothing allocated. Recursion: none.

use geom_num::interval::Interval;
use geom_num::plane::{Lpi, Tpi};
use geom_num::{ExactSum, P3, Wide};
use geom_run::WorkMeter;

use crate::query::QueryError;

/// Integers wide enough for every term here: 1024 bits (module header).
type Big = Wide<16>;

/// 2^32, the step between the 32-bit parts [`enclose`] adds.
const PART: f64 = 4_294_967_296.0;

/// One corner of a flat polygon.
#[derive(Clone, Debug)]
pub enum Corner {
    /// A grid point.
    Grid(P3),
    /// The meeting of three planes.
    Tpi(Tpi),
    /// The meeting of a line through two grid points and a plane.
    Lpi(Lpi),
}

/// A point's homogeneous coordinates x, y, z and its weight w: the point is (x/w, y/w, z/w).
struct Homogeneous {
    x: [Big; 3],
    w: Big,
}

/// `v` in [`Big`].
fn widen<const L: usize>(v: &Wide<L>) -> Result<Big, QueryError> {
    v.resize().ok_or(QueryError::Overflow)
}

impl Corner {
    /// Its homogeneous coordinates.
    fn homogeneous(&self) -> Result<Homogeneous, QueryError> {
        match self {
            Corner::Grid(p) => Ok(Homogeneous {
                x: [p.x, p.y, p.z].map(|c| Big::from_i64(c.get())),
                w: Big::one(),
            }),
            Corner::Tpi(t) => {
                let ([x, y, z], w) = t.homogeneous()?;
                Ok(Homogeneous {
                    x: [widen(&x)?, widen(&y)?, widen(&z)?],
                    w: widen(&w)?,
                })
            }
            Corner::Lpi(l) => {
                let ([x, y, z], w) = l.homogeneous()?;
                Ok(Homogeneous {
                    x: [widen(&x)?, widen(&y)?, widen(&z)?],
                    w: Big::from_i128(w),
                })
            }
        }
    }
}

/// `a × b`, or the overflow nothing here can reach.
fn mul(a: &Big, b: &Big) -> Result<Big, QueryError> {
    a.checked_mul(b).ok_or(QueryError::Overflow)
}

/// `a + b`, or the overflow nothing here can reach.
fn add(a: &Big, b: &Big) -> Result<Big, QueryError> {
    a.checked_add(b).ok_or(QueryError::Overflow)
}

/// `a b − c d`, or the overflow nothing here can reach.
fn minor(a: &Big, b: &Big, c: &Big, d: &Big) -> Result<Big, QueryError> {
    mul(a, b)?
        .checked_sub(&mul(c, d)?)
        .ok_or(QueryError::Overflow)
}

/// An interval around `v`: exactly 0, or the doubles either side of its nearest double.
fn enclose(v: &Big) -> Result<Interval, QueryError> {
    if v.is_zero() {
        return Ok(Interval::point(0.0)?);
    }
    let size = if v.is_negative() {
        v.checked_neg().ok_or(QueryError::Overflow)?
    } else {
        *v
    };
    let mut sum = ExactSum::new();
    let mut scale = 1.0_f64;
    // Each 32-bit part times a power of two is a double exactly, and the 1024-bit budget ends two
    // parts below the doubles' range.
    for limb in size.to_le_limbs() {
        for part in [limb & 0xFFFF_FFFF, limb >> 32] {
            #[allow(
                clippy::cast_precision_loss,
                reason = "a 32-bit part is exact in a double"
            )]
            sum.add(part as f64 * scale);
            scale *= PART;
        }
    }
    let nearest = sum.round()?.get();
    let around = Interval::new(nearest.next_down(), nearest.next_up())?;
    Ok(if v.is_negative() {
        around.neg()
    } else {
        around
    })
}

/// `n / d`, each exact, rounded outward.
fn fraction(n: &Big, d: &Big) -> Result<Interval, QueryError> {
    Ok(enclose(n)?.div(enclose(d)?)?)
}

/// `a + b`, rounded outward.
fn plus(a: Interval, b: Interval) -> Result<Interval, QueryError> {
    Ok(a.add(b)?)
}

/// `p × q` as three exact fractions, each rounded outward.
fn cross(p: &Homogeneous, q: &Homogeneous) -> Result<[Interval; 3], QueryError> {
    let [px, py, pz] = &p.x;
    let [qx, qy, qz] = &q.x;
    let w = mul(&p.w, &q.w)?;
    Ok([
        fraction(&minor(py, qz, pz, qy)?, &w)?,
        fraction(&minor(pz, qx, px, qz)?, &w)?,
        fraction(&minor(px, qy, py, qx)?, &w)?,
    ])
}

/// `p · (q × r)` as one exact fraction, rounded outward.
fn triple(p: &Homogeneous, q: &Homogeneous, r: &Homogeneous) -> Result<Interval, QueryError> {
    let [px, py, pz] = &p.x;
    let [qx, qy, qz] = &q.x;
    let [rx, ry, rz] = &r.x;
    let terms = [
        mul(px, &minor(qy, rz, qz, ry)?)?,
        mul(py, &minor(qz, rx, qx, rz)?)?,
        mul(pz, &minor(qx, ry, qy, rx)?)?,
    ];
    let [first, second, third] = &terms;
    let n = add(&add(first, second)?, third)?;
    fraction(&n, &mul(&mul(&p.w, &q.w)?, &r.w)?)
}

/// Twice the area vector of the closed polygon through `corners`: Σ pᵢ × pᵢ₊₁, each component a
/// certified interval.
///
/// # Errors
///
/// A corner the numbers layer cannot put in homogeneous form, as [`QueryError::Overflow`], and the
/// meter's stop.
pub fn loop_area(
    corners: &[Corner],
    meter: &mut WorkMeter<'_>,
) -> Result<[Interval; 3], QueryError> {
    let mut sum = [Interval::point(0.0)?; 3];
    for (p, q) in corners.iter().zip(corners.iter().cycle().skip(1)) {
        meter.charge(1)?;
        let [sx, sy, sz] = sum;
        let [cx, cy, cz] = cross(&p.homogeneous()?, &q.homogeneous()?)?;
        sum = [plus(sx, cx)?, plus(sy, cy)?, plus(sz, cz)?];
    }
    Ok(sum)
}

/// Six times the signed volume a closed shell encloses, given its loops: Σ over each loop's fan of
/// triangles from its first corner of p₀ · (pᵢ × pᵢ₊₁), a certified interval.
///
/// # Errors
///
/// A corner the numbers layer cannot put in homogeneous form, as [`QueryError::Overflow`], and the
/// meter's stop.
pub fn shell_volume(
    loops: &[&[Corner]],
    meter: &mut WorkMeter<'_>,
) -> Result<Interval, QueryError> {
    let mut sum = Interval::point(0.0)?;
    for corners in loops {
        let Some((first, rest)) = corners.split_first() else {
            continue;
        };
        let apex = first.homogeneous()?;
        for (q, r) in rest.iter().zip(rest.iter().skip(1)) {
            meter.charge(1)?;
            sum = plus(sum, triple(&apex, &q.homogeneous()?, &r.homogeneous()?)?)?;
        }
    }
    Ok(sum)
}
