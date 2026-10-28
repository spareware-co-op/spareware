//! Rational rotations: one exact rotation type for 2D and 3D, held as an integer quaternion, and the
//! closest such rotation, or Pythagorean direction, within a requested angle (decision 32.14).
//!
//! A [`RationalRotation`] is an integer quaternion q = (w, x, y, z), in lowest terms with its first
//! nonzero component positive, standing for the rotation M(q)/|q|², where M(q) is the integer
//! Euler–Rodrigues matrix
//!
//! ```text
//! | w²+x²−y²−z²   2(xy − wz)    2(xz + wy)  |
//! | 2(xy + wz)    w²−x²+y²−z²   2(yz − wx)  |
//! | 2(xz − wy)    2(yz + wx)    w²−x²−y²+z² |
//! ```
//!
//! so it is exact in fractions, and two rotations compose exactly by the quaternion product. A
//! rotation matrix is rational exactly when it comes from an integer quaternion, so this one type
//! holds every rational rotation. Every multiple of q, −q included, gives the same rotation, which
//! is why the form is canonical: `==` is equality of rotations. A rotation in the plane is the
//! case (w, 0, 0, z): it turns the x axis to the Pythagorean direction (w² − z², 2wz)/(w² + z²), a
//! [`Pythagorean`] unit vector.
//!
//! **Budget.** |q|² < [`NORM2_LIMIT`] = 2^72, so every component is below 2^36, every entry of M(q)
//! is at most |q|² in magnitude, and a grid point's image Σ M(q)ᵢⱼpⱼ is below 3·2^112, inside
//! `i128` (32.14). Where |q|² ≤ 2^40 the rotation is also a [`RationalTransform`]
//! ([`RationalRotation::to_rational`]); above that it is its own variant of the typed transform,
//! [`TypedTransform::Rotation`](crate::TypedTransform::Rotation).
//!
//! **Within an angle.** Every search takes the bound δ the caller accepts, from the angular bound 2^−34 rad
//! ([`geom_num::ladder::ANGULAR`]) to [`BOUND_MAX`] = 1/2 rad, and returns a [`WithinAngle`]: the
//! result and a certified upper bound, at most δ, on the angle of the rotation that takes it to the
//! one asked for. Zero means exact.
//!
//! - [`RationalRotation::plane_within`] and [`Pythagorean::within`], in 2D, follow Canny, Donald
//!   and Ressler. The rotation by θ has half-angle tangent t = tan(θ/2), and (m, 0, 0, n) turns by
//!   2·atan(n/m), so the search is for a fraction n/m near t. The target is first turned by
//!   quarter turns (exact on doubles) so that |θ| ≤ π/4. The tangents [tan(θ/2 − a), tan(θ/2 + a)],
//!   a = atan(u) with x = (1 − 2^−10)·δ/2 and u at most x + x³/3, so that a ≤ x < δ/2 (tan x ≥
//!   x + x³/3, its Taylor coefficients being positive), are enclosed from inside, and the fraction
//!   with the smallest denominator among them (the simplest rational, from the continued fractions
//!   of the two ends) is taken and turned back. a falls short of x by less than x⁵/7, so those
//!   tangents span at least (1 − 2^−9)·δ > 2^−35: a fraction with a denominator below 2^35 lies
//!   among them, and every component stays below 2^36. No rotation with a smaller denominator is
//!   within (1 − 2^−9)·δ.
//! - [`RationalRotation::within`], in 3D, rounds s·u to integers, u the unit quaternion of the
//!   target, for s = 1, 2, 4, ..., 2^34 and then 3·2^34, and takes the first that certifies within
//!   δ. Rounding moves s·u by at most 1, so the angle is at most about 2/s: at s = 3·2^34 it is
//!   below (2/3)·2^−34 (with the certificate's slack), inside every allowed δ, and
//!   |q|² ≤ (3·2^34 + 1)² < 2^72.
//! - [`RationalRotation::compose`] is the exact product, re-approximated once by the 3D search when
//!   it outgrows the budget. The angle of the rotation between two rotations is a metric that
//!   composition on either side leaves unchanged, so a composition of approximations is within the
//!   sum of their bounds, plus the re-approximation's, of the composition of what they approximate.
//!
//! **The certificate.** The angle of the rotation between q and a target quaternion t is 2∠, where
//! ∠ ∈ [0, π/2) is their angle as vectors in R⁴ when ⟨q, t⟩ > 0, and sin ∠ = |q ∧ t|/(|q|·|t|),
//! with |q ∧ t|² = Σᵢ<ⱼ (qᵢtⱼ − qⱼtᵢ)² (Lagrange's identity). It is evaluated in interval
//! arithmetic over an enclosure of t, giving S ≥ sin ∠, and 2·asin(S) ≤ 2S + S³/2 for S ≤ 1/2:
//! (asin S − S)/S³ has positive Taylor coefficients, so it grows with S, and at S = 1/2 it is
//! (π/6 − 1/2)·8 < 0.19 < 1/4. A candidate that does not certify (⟨q, t⟩ not certainly positive,
//! or S above 1/2) is not taken.
//!
//! Guarantees: a [`RationalRotation`] is exact and canonical, its image of a grid point is the
//! exact image rounded once, ties to even, with whether it was exact; composition is exact while
//! the product fits the budget and otherwise re-approximated once, saying by how much; every
//! [`WithinAngle`] bound is a certified upper bound on the true angle and at most the bound asked
//! for; and a step the budget proves fits that fails all the same is reported as
//! [`NumError::BudgetExceeded`], never answered.
//!
//! Method: J. Canny, B. Donald and E. K. Ressler, "A rational rotation method for robust geometric
//! algorithms", Proceedings of the 8th Symposium on Computational Geometry (1992), 251–260,
//! <https://doi.org/10.1145/142675.142726>, for the 2D directions; the Euler–Rodrigues
//! parametrisation of rotations by quaternions for the 3D type (decision 32.14); the simplest
//! rational in an interval from the continued fractions of its ends (G. H. Hardy and E. M. Wright,
//! *An Introduction to the Theory of Numbers*, 6th ed., Oxford University Press (2008), chapter 10,
//! <https://global.oup.com/academic/product/9780199219865>); interval arithmetic as in
//! [`geom_num::interval`]. The quaternion of a rational rotation matrix ([`RationalRotation::from_rational`])
//! is read from the identity d·(1 + tr R) = 4w²·d/|q|² and its companions, derived in that
//! function's comment.
//!
//! Cost: constant time, no allocation. Applying is ten `i128` products for M(q) and nine for the
//! image, and three Euclidean divisions; composing is sixteen products and a reduction (four
//! greatest common divisors); a 2D search is about twenty interval operations, at most 200 steps of
//! the simplest-rational loop and one certificate (about 60 interval operations); a 3D search is at
//! most 36 roundings and certificates.

#![allow(
    clippy::many_single_char_names,
    reason = "variable names follow the formulas of the method each function cites"
)]

use geom_num::grid::{Coord, P2, P3};
use geom_num::interval::Interval;
use geom_num::{NumError, Real, ladder, math};

use crate::error::{MathError, budget};
use crate::exact::{exponent, gcd_u128, pow2, rational_to_f64, scale_b};
use crate::float::F2;
use crate::grid_transform::Axis;
use crate::rational::{RationalTransform, round_div};
use crate::rounded::Rounded;
use crate::transform::{Classification, Transform, TransformKind};

/// The bound on |q|²: 2^72. Every component is then below 2^36.
pub const NORM2_LIMIT: u128 = 1 << 72;

/// The largest |q|² of a rotation that is also a [`RationalTransform`]: its denominator bound,
/// 2^40.
const RATIONAL_NORM2: u128 = 1 << 40;

/// The loosest angle bound a search accepts: half a radian, where the certificate's 2·asin bound
/// still holds with room.
pub const BOUND_MAX: f64 = 0.5;

/// The fraction of δ/2 the 2D search aims inside: 1/2 − 2^−11, exact in a double.
const AIM: f64 = 0.5 - 1.0 / 2048.0;

/// The 2D search's ends are rounded inward to multiples of 2^−100.
const ENDS_LOG2: i32 = 100;

/// The largest denominator m a 2D search may return: 2^35 (see the module documentation).
const DENOMINATOR_LIMIT: i128 = 1 << 35;

/// More than enough steps for the simplest-rational loop: each step is one of Euclid's algorithm
/// on numbers below 2^101, which takes fewer than 150.
const SIMPLEST_STEPS: usize = 200;

/// A result and a certified upper bound on the angle, in radians, of the rotation that takes it to
/// the one asked for: zero when it is exact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WithinAngle<T> {
    value: T,
    angle: Real,
}

impl<T: Copy> WithinAngle<T> {
    /// The result.
    #[must_use]
    pub fn value(&self) -> T {
        self.value
    }

    /// The certified upper bound on the angle, in radians; zero when the result is exact.
    #[must_use]
    pub fn angle(&self) -> Real {
        self.angle
    }

    /// Whether the result is exactly what was asked for.
    #[must_use]
    pub fn is_exact(&self) -> bool {
        self.angle.get() == 0.0
    }
}

/// A rational unit vector (a/c, b/c) in the plane: a² + b² = c², c > 0, in lowest terms.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Pythagorean {
    a: i128,
    b: i128,
    c: i128,
}

/// A rotation exact in fractions, held as an integer quaternion (see the module documentation).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RationalRotation {
    q: [i64; 4],
}

/// An interval operation whose operands the caller bounds, so that it cannot fail: its value, or
/// [`NumError::BudgetExceeded`], a defect.
fn iv(step: Result<Interval, NumError>) -> Result<Interval, NumError> {
    step.map_err(|_| NumError::BudgetExceeded)
}

/// The interval holding exactly the integer `v`, |v| < 2^53, so the double is exact.
fn exact_int(v: i64) -> Result<Interval, NumError> {
    #[allow(
        clippy::cast_precision_loss,
        reason = "every caller's |v| is below 2^37 < 2^53, so the conversion is exact"
    )]
    let f = v as f64;
    iv(Interval::point(f))
}

/// The interval holding the integer `v` exactly: the nearest double when that is `v`, else the
/// two doubles either side of it.
fn int_interval(v: i128) -> Result<(Interval, f64), NumError> {
    let r = budget(rational_to_f64(v, 1))?;
    let f = r.value();
    let i = if r.is_exact() {
        Interval::point(f)
    } else {
        Interval::new(f.next_down(), f.next_up())
    };
    Ok((iv(i)?, f))
}

/// The requested bound, checked: from the angular bound 2^−34 to [`BOUND_MAX`].
fn checked_bound(bound: Real) -> Result<f64, NumError> {
    let b = bound.get();
    if b < ladder::ANGULAR.get() || b > BOUND_MAX {
        return Err(NumError::ToleranceExceeded);
    }
    Ok(b)
}

/// The doubles of a target scaled by one power of two so that the largest magnitude lies in
/// [1, 2), and enclosures of their exact scaled values: exact, except for a component whose scaled
/// value fell below 2^−1022, where scaling may round and the enclosure is one double either side.
/// `Degenerate` if every component is zero, `NotFinite` if one is not finite.
fn scaled<const N: usize>(t: [f64; N]) -> Result<([f64; N], [Interval; N]), NumError> {
    if t.iter().any(|v| !v.is_finite()) {
        return Err(NumError::NotFinite);
    }
    let e = t
        .iter()
        .filter_map(|&v| exponent(v))
        .max()
        .ok_or(NumError::Degenerate)?;
    // −1074 ≤ e ≤ 1023, so −e fits.
    let k = budget(e.checked_neg())?;
    let mut values = [0.0; N];
    let mut boxes = [Interval::point(0.0)?; N];
    for ((out, b), &v) in values.iter_mut().zip(boxes.iter_mut()).zip(t.iter()) {
        let s = budget(scale_b(v, k))?;
        *out = s;
        *b = if v != 0.0 && s.abs() < f64::MIN_POSITIVE {
            iv(Interval::new(s.next_down(), s.next_up()))?
        } else {
            iv(Interval::point(s))?
        };
    }
    Ok((values, boxes))
}

/// The certified upper bound on the angle of the rotation between the integer quaternion `q`
/// (every |qᵢ| < 2^37) and the target enclosed by `t` (largest component's magnitude at least 1/2
/// and below 2^75), or `None` when it does not certify (module documentation).
fn certify(q: [i64; 4], t: &[Interval; 4]) -> Result<Option<f64>, NumError> {
    let [q0, q1, q2, q3] = q;
    let qi = [
        exact_int(q0)?,
        exact_int(q1)?,
        exact_int(q2)?,
        exact_int(q3)?,
    ];
    let mut dot = Interval::point(0.0)?;
    for (a, b) in qi.iter().zip(t.iter()) {
        dot = iv(dot.add(iv(a.mul(*b))?))?;
    }
    if dot.lo() <= 0.0 {
        return Ok(None);
    }
    let [a0, a1, a2, a3] = qi;
    let [t0, t1, t2, t3] = *t;
    let wedge = |a: Interval, tb: Interval, b: Interval, ta: Interval| {
        iv(iv(a.mul(tb))?.sub(iv(b.mul(ta))?))
    };
    let w = [
        wedge(a0, t1, a1, t0)?,
        wedge(a0, t2, a2, t0)?,
        wedge(a0, t3, a3, t0)?,
        wedge(a1, t2, a2, t1)?,
        wedge(a1, t3, a3, t1)?,
        wedge(a2, t3, a3, t2)?,
    ];
    let num = iv(Interval::norm(&w))?;
    let den = iv(iv(Interval::norm(&qi))?.mul(iv(Interval::norm(t))?))?;
    let s = iv(num.div(den))?.hi();
    if s > 0.5 {
        return Ok(None);
    }
    // 2·asin(s) ≤ 2s + s³/2 for 0 ≤ s ≤ 1/2.
    let si = iv(Interval::point(s))?;
    let cube = iv(iv(si.square())?.mul(si))?;
    let half_cube = iv(cube.mul(iv(Interval::point(0.5))?))?;
    Ok(Some(iv(iv(si.add(si))?.add(half_cube))?.hi()))
}

/// The canonical rotation of an integer quaternion: in lowest terms, first nonzero component
/// positive. `Degenerate` for zero, `OutOfRange` when |q|² reaches [`NORM2_LIMIT`] (with the
/// component, or |q|², that does not fit).
fn canonical(q: [i128; 4]) -> Result<RationalRotation, NumError> {
    let mut g = 0u128;
    for v in q {
        if v == i128::MIN {
            return Err(NumError::OutOfRange { value: v });
        }
        g = gcd_u128(g, v.unsigned_abs());
    }
    if g == 0 {
        return Err(NumError::Degenerate);
    }
    // 1 ≤ g < 2^127 (no component is i128::MIN), so g fits i128 and every quotient is exact.
    let g = budget(i128::try_from(g).ok())?;
    let lead = q.iter().copied().find(|&v| v != 0).unwrap_or(1);
    let g = if lead < 0 {
        budget(g.checked_neg())?
    } else {
        g
    };
    let mut out = [0i64; 4];
    let mut norm2 = 0u128;
    for (o, v) in out.iter_mut().zip(q) {
        let r = budget(v.checked_div(g))?;
        // A component of 2^36 or more makes |q|² at least 2^72.
        if r.unsigned_abs() >= 1 << 36 {
            return Err(NumError::OutOfRange { value: r });
        }
        let sq = budget(r.unsigned_abs().checked_mul(r.unsigned_abs()))?;
        norm2 = budget(norm2.checked_add(sq))?;
        *o = budget(i64::try_from(r).ok())?;
    }
    if norm2 >= NORM2_LIMIT {
        return Err(NumError::OutOfRange {
            value: budget(i128::try_from(norm2).ok())?,
        });
    }
    Ok(RationalRotation { q: out })
}

/// The Hamilton product p·q, exactly: components below 2^72 for |p|, |q| < 2^36.
fn product(p: [i64; 4], q: [i64; 4]) -> Option<[i128; 4]> {
    let [a1, b1, c1, d1] = p.map(i128::from);
    let [a2, b2, c2, d2] = q.map(i128::from);
    let sum4 = |s: [(i128, i128, bool); 4]| -> Option<i128> {
        let mut acc = 0i128;
        for (x, y, plus) in s {
            let t = x.checked_mul(y)?;
            acc = if plus {
                acc.checked_add(t)?
            } else {
                acc.checked_sub(t)?
            };
        }
        Some(acc)
    };
    Some([
        sum4([
            (a1, a2, true),
            (b1, b2, false),
            (c1, c2, false),
            (d1, d2, false),
        ])?,
        sum4([
            (a1, b2, true),
            (b1, a2, true),
            (c1, d2, true),
            (d1, c2, false),
        ])?,
        sum4([
            (a1, c2, true),
            (b1, d2, false),
            (c1, a2, true),
            (d1, b2, true),
        ])?,
        sum4([
            (a1, d2, true),
            (b1, c2, true),
            (c1, b2, false),
            (d1, a2, true),
        ])?,
    ])
}

/// The rotation within `bound` of the target enclosed by `t`, found by rounding s·u for growing s
/// (module documentation), `approx` being doubles near the target with the largest magnitude in
/// [1/2, 2^75).
fn round_within(
    t: &[Interval; 4],
    approx: [f64; 4],
    bound: f64,
) -> Result<WithinAngle<RationalRotation>, NumError> {
    let n2 = approx.iter().fold(0.0, |acc, v| acc + v * v);
    let len = math::sqrt(n2)?.get();
    let u = approx.map(|v| v / len);
    for k in 0..=35 {
        // 2^k for k ≤ 34, then 3·2^34.
        let s = if k == 35 {
            3.0 * budget(pow2(34))?
        } else {
            budget(pow2(k))?
        };
        let mut q = [0i64; 4];
        for (o, v) in q.iter_mut().zip(u) {
            let r = math::round_ties_even(v * s)?.get();
            // |u| ≤ 1 up to rounding, so |r| ≤ 3·2^34 + 1: a whole number that i64 holds exactly.
            #[allow(
                clippy::cast_possible_truncation,
                reason = "r is a whole number of magnitude at most 3·2^34 + 1"
            )]
            let whole = r as i64;
            *o = whole;
        }
        if q == [0; 4] {
            continue;
        }
        if let Some(angle) = certify(q, t)?
            && angle <= bound
        {
            return Ok(WithinAngle {
                value: canonical(q.map(i128::from))?,
                angle: Real::new(angle)?,
            });
        }
    }
    // At s = 3·2^34 the angle is below (2/3)·2^−34 plus the certificate's slack, inside every
    // allowed bound: reaching here is a defect.
    Err(NumError::BudgetExceeded)
}

/// The simplest fraction n/m (smallest m ≥ 1, then smallest |n|) with lo ≤ n/m ≤ hi, both ends
/// rounded inward to multiples of 2^−100; `None` if a step does not fit, which the 2D search's
/// bounds (|lo|, |hi| < 1, hi − lo > 2^−35) rule out.
fn simplest(lo: f64, hi: f64) -> Option<(i128, i128)> {
    if lo <= 0.0 && hi >= 0.0 {
        return Some((0, 1));
    }
    if hi < 0.0 {
        let (n, m) = simplest_positive(-hi, -lo)?;
        return Some((n.checked_neg()?, m));
    }
    simplest_positive(lo, hi)
}

/// [`simplest`] for 0 < lo ≤ hi < 1.
///
/// The ends become ln/D and un/D with D = 2^100 (lo·2^100 and hi·2^100 are exact, being normal
/// doubles scaled by a power of two, and below 2^100). Then, as long as the smallest integer
/// c ≥ x is above y, the interval [x, y] lies inside (a, a + 1) with a = c − 1, a is the next
/// continued-fraction term, and the search goes on in [1/(y − a), 1/(x − a)]; the first c ≤ y is
/// the last term. Every numerator and denominator stays below 2^101 and only falls (each step is
/// one of Euclid's algorithm), and the convergents grow only to the result.
fn simplest_positive(lo: f64, hi: f64) -> Option<(i128, i128)> {
    let to_int = |v: f64| -> Option<i128> {
        // A whole number below 2^100 in magnitude: exact in i128.
        #[allow(
            clippy::cast_possible_truncation,
            reason = "v is a whole number below 2^100 in magnitude"
        )]
        let i = v as i128;
        Some(i)
    };
    let d: i128 = 1 << ENDS_LOG2;
    let mut ln = to_int(math::ceil(scale_b(lo, ENDS_LOG2)?).ok()?.get())?;
    let mut un = to_int(math::floor(scale_b(hi, ENDS_LOG2)?).ok()?.get())?;
    let (mut ld, mut ud) = (d, d);
    if ln > un || ln <= 0 {
        return None;
    }
    // The convergents p_{k−2}/q_{k−2} and p_{k−1}/q_{k−1}, starting from 0/1 and 1/0.
    let (mut p0, mut q0, mut p1, mut q1) = (0i128, 1i128, 1i128, 0i128);
    for _ in 0..SIMPLEST_STEPS {
        let floor = ln.checked_div_euclid(ld)?;
        let c = if ln.checked_rem_euclid(ld)? == 0 {
            floor
        } else {
            floor.checked_add(1)?
        };
        // c ≤ y, as c·ud ≤ un; a product that does not fit is above un.
        if c.checked_mul(ud).is_some_and(|v| v <= un) {
            let n = c.checked_mul(p1)?.checked_add(p0)?;
            let m = c.checked_mul(q1)?.checked_add(q0)?;
            return Some((n, m));
        }
        let a = floor;
        let (np, nq) = (
            a.checked_mul(p1)?.checked_add(p0)?,
            a.checked_mul(q1)?.checked_add(q0)?,
        );
        (p0, q0, p1, q1) = (p1, q1, np, nq);
        let (nln, nld) = (ud, un.checked_sub(a.checked_mul(ud)?)?);
        let (nun, nud) = (ld, ln.checked_sub(a.checked_mul(ld)?)?);
        (ln, ld, un, ud) = (nln, nld, nun, nud);
    }
    None
}

impl RationalRotation {
    /// No rotation: q = (1, 0, 0, 0).
    pub const IDENTITY: RationalRotation = RationalRotation { q: [1, 0, 0, 0] };

    /// The rotation of the integer quaternion q = (w, x, y, z), brought to canonical form.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] for q = 0; [`NumError::OutOfRange`] if |q|² in lowest terms is
    /// [`NORM2_LIMIT`] or more.
    pub fn new(q: [i64; 4]) -> Result<RationalRotation, NumError> {
        canonical(q.map(i128::from))
    }

    /// The quaternion (w, x, y, z), canonical: in lowest terms, first nonzero component positive.
    #[must_use]
    pub const fn quaternion(&self) -> [i64; 4] {
        self.q
    }

    /// |q|², below 2^72: the denominator of M(q)/|q|².
    #[must_use]
    pub fn norm2(&self) -> u128 {
        // Each square is below 2^72 and the sum below 2^72, in u128: saturating never saturates.
        self.q.iter().fold(0u128, |acc, v| {
            let m = u128::from(v.unsigned_abs());
            acc.saturating_add(m.saturating_mul(m))
        })
    }

    /// The integer matrix M(q), every entry at most |q|² < 2^72 in magnitude; the rotation is
    /// M(q)/|q|².
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only if a checked step failed, which the budget rules out.
    pub fn matrix(&self) -> Result<[[i128; 3]; 3], NumError> {
        let [w, x, y, z] = self.q.map(i128::from);
        let m = || -> Option<[[i128; 3]; 3]> {
            let (ww, xx, yy, zz) = (
                w.checked_mul(w)?,
                x.checked_mul(x)?,
                y.checked_mul(y)?,
                z.checked_mul(z)?,
            );
            let twice = |a: i128, b: i128, c: i128, d: i128, plus: bool| -> Option<i128> {
                let (ab, cd) = (a.checked_mul(b)?, c.checked_mul(d)?);
                let s = if plus {
                    ab.checked_add(cd)?
                } else {
                    ab.checked_sub(cd)?
                };
                s.checked_mul(2)
            };
            let diag = |p: i128, q: i128, r: i128, s: i128| -> Option<i128> {
                p.checked_add(q)?.checked_sub(r)?.checked_sub(s)
            };
            Some([
                [
                    diag(ww, xx, yy, zz)?,
                    twice(x, y, w, z, false)?,
                    twice(x, z, w, y, true)?,
                ],
                [
                    twice(x, y, w, z, true)?,
                    diag(ww, yy, xx, zz)?,
                    twice(y, z, w, x, false)?,
                ],
                [
                    twice(x, z, w, y, false)?,
                    twice(y, z, w, x, true)?,
                    diag(ww, zz, xx, yy)?,
                ],
            ])
        };
        budget(m())
    }

    /// The rotation about `axis` through the origin that turns by the angle of `direction`
    /// (counter-clockwise looking from the positive end of the axis, as
    /// [`RationalTransform::rotation`]): the half-angle tangent of (a, b)/c is b/(c + a), so
    /// q = (c + a, b) on the axis's component, and (0, 1) for the half turn a = −c.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if the rotation does not fit the budget (it always does for a
    /// direction this module returned); [`NumError::BudgetExceeded`] only if a checked step failed.
    pub fn about(axis: Axis, direction: &Pythagorean) -> Result<RationalRotation, NumError> {
        let (w, s) = if budget(direction.c.checked_add(direction.a))? == 0 {
            (0, 1)
        } else {
            (budget(direction.c.checked_add(direction.a))?, direction.b)
        };
        let q = match axis {
            Axis::X => [w, s, 0, 0],
            Axis::Y => [w, 0, s, 0],
            Axis::Z => [w, 0, 0, s],
        };
        canonical(q)
    }

    /// The rotation in the plane (about z) within `bound` of the rotation that turns the x axis to
    /// `direction`, with the smallest half-angle denominator the 2D search finds (module
    /// documentation; Canny, Donald and Ressler).
    ///
    /// # Errors
    ///
    /// [`NumError::ToleranceExceeded`] if `bound` is below the angular bound 2^−34 or above
    /// [`BOUND_MAX`]; [`NumError::Degenerate`] for the zero direction; [`NumError::BudgetExceeded`]
    /// only if a checked step failed, which the module's bounds rule out.
    pub fn plane_within(
        direction: F2,
        bound: Real,
    ) -> Result<WithinAngle<RationalRotation>, MathError> {
        let bound = checked_bound(bound)?;
        let ([x, y], [ix, iy]) = scaled(direction.to_array())?;
        // Quarter turns clockwise, k of them, so that the direction (ex, ey) has ex ≥ |ey|, ex > 0.
        let (k, ex, ey) = if x > 0.0 && x >= y.abs() {
            (0, ix, iy)
        } else if y > 0.0 && y >= x.abs() {
            (1, iy, ix.neg())
        } else if x < 0.0 && -x >= y.abs() {
            (2, ix.neg(), iy.neg())
        } else {
            (3, iy.neg(), ix)
        };
        // The half-angle quaternion of (ex, ey) is (|e| + ex, 0, 0, ey), and t = ey/(|e| + ex).
        let len = iv(Interval::norm(&[ex, ey]))?;
        let h = iv(len.add(ex))?;
        let t = iv(ey.div(h))?;
        let one = iv(Interval::point(1.0))?;
        let x = iv(iv(Interval::point(bound))?.mul(iv(Interval::point(AIM))?))?.lo();
        let xi = iv(Interval::point(x))?;
        // u ≤ x + x³/3, so atan(u) ≤ x: tan x ≥ x + x³/3 for 0 ≤ x < π/2.
        let third = iv(iv(iv(xi.square())?.mul(xi))?.div(iv(Interval::point(3.0))?))?;
        let ui = iv(Interval::point(iv(xi.add(third))?.lo()))?;
        let (th, tl) = (iv(Interval::point(t.hi()))?, iv(Interval::point(t.lo()))?);
        // tan(θ/2 − atan u) = (t − u)/(1 + tu), rising with t: its largest value over the
        // enclosure of t, from above, is an inner lower end; likewise the upper end.
        let lower = iv(iv(th.sub(ui))?.div(iv(one.add(iv(th.mul(ui))?))?))?.hi();
        let upper = iv(iv(tl.add(ui))?.div(iv(one.sub(iv(tl.mul(ui))?))?))?.lo();
        let (n, m) = budget(simplest(lower, upper))?;
        if m >= DENOMINATOR_LIMIT || n.unsigned_abs() > m.unsigned_abs() {
            return Err(NumError::BudgetExceeded.into());
        }
        let (mi, ni) = (
            budget(i64::try_from(m).ok())?,
            budget(i64::try_from(n).ok())?,
        );
        let zero = iv(Interval::point(0.0))?;
        let angle = budget(certify([mi, 0, 0, ni], &[h, zero, zero, ey])?)?;
        if angle > bound {
            return Err(NumError::BudgetExceeded.into());
        }
        // Turn back: the rotation (w, z) followed by k quarter turns, (1, 0, 0, 1)^k.
        let (w, z) = (m, n);
        let (w, z) = match k {
            0 => (w, z),
            1 => (budget(w.checked_sub(z))?, budget(w.checked_add(z))?),
            2 => (budget(z.checked_neg())?, w),
            _ => (
                budget(budget(z.checked_neg())?.checked_sub(w))?,
                budget(w.checked_sub(z))?,
            ),
        };
        Ok(WithinAngle {
            value: canonical([w, 0, 0, z])?,
            angle: Real::new(angle)?,
        })
    }

    /// The rotation within `bound` of the rotation of the quaternion `target` (any nonzero finite
    /// doubles, not necessarily of unit length), by rounding (module documentation).
    ///
    /// # Errors
    ///
    /// [`NumError::ToleranceExceeded`] if `bound` is below the angular bound 2^−34 or above
    /// [`BOUND_MAX`]; [`NumError::Degenerate`] for the zero quaternion; [`NumError::NotFinite`]
    /// if a component is not finite; [`NumError::BudgetExceeded`] only if a checked step failed,
    /// which the module's bounds rule out.
    pub fn within(
        target: [f64; 4],
        bound: Real,
    ) -> Result<WithinAngle<RationalRotation>, MathError> {
        let bound = checked_bound(bound)?;
        let (values, boxes) = scaled(target)?;
        Ok(round_within(&boxes, values, bound)?)
    }

    /// The composition `self ∘ first` (apply `first`, then `self`): the quaternion product
    /// self·first, exact while it fits the budget in lowest terms, and otherwise re-approximated
    /// once within `bound` by [`RationalRotation::within`]'s rounding.
    ///
    /// # Errors
    ///
    /// [`NumError::ToleranceExceeded`] if `bound` is outside [2^−34, [`BOUND_MAX`]], checked even
    /// when the product is exact; [`NumError::BudgetExceeded`] only if a checked step failed.
    pub fn compose(
        &self,
        first: &RationalRotation,
        bound: Real,
    ) -> Result<WithinAngle<RationalRotation>, MathError> {
        let bound = checked_bound(bound)?;
        if let Some(exact) = self.compose_exact(first)? {
            return Ok(WithinAngle {
                value: exact,
                angle: Real::ZERO,
            });
        }
        let p = budget(product(self.q, first.q))?;
        let [p0, p1, p2, p3] = p;
        let (i0, f0) = int_interval(p0)?;
        let (i1, f1) = int_interval(p1)?;
        let (i2, f2) = int_interval(p2)?;
        let (i3, f3) = int_interval(p3)?;
        Ok(round_within(&[i0, i1, i2, i3], [f0, f1, f2, f3], bound)?)
    }

    /// The composition `self ∘ first`, exactly, when it fits the budget in lowest terms; `None`
    /// when it does not.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only if a checked step failed, which the budget rules out.
    pub fn compose_exact(
        &self,
        first: &RationalRotation,
    ) -> Result<Option<RationalRotation>, NumError> {
        match canonical(budget(product(self.q, first.q))?) {
            Ok(r) => Ok(Some(r)),
            Err(NumError::OutOfRange { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// The inverse rotation: the conjugate (w, −x, −y, −z), in canonical form.
    #[must_use]
    pub fn inverse(&self) -> RationalRotation {
        let [w, x, y, z] = self.q;
        // Every component is below 2^36 in magnitude, so negating never saturates.
        let c = [
            w,
            x.saturating_neg(),
            y.saturating_neg(),
            z.saturating_neg(),
        ];
        let lead = c.iter().copied().find(|&v| v != 0).unwrap_or(1);
        RationalRotation {
            q: if lead < 0 {
                c.map(i64::saturating_neg)
            } else {
                c
            },
        }
    }

    /// Whether the rotation is about the z axis, so it acts in the plane z = 0 (x = y = 0).
    #[must_use]
    pub fn acts_in_plane(&self) -> bool {
        let [_, x, y, _] = self.q;
        x == 0 && y == 0
    }

    /// The Pythagorean direction the rotation turns the x axis to, for a rotation in the plane:
    /// (w² − z², 2wz)/(w² + z²) in lowest terms; `None` for a rotation that does not act in the
    /// plane.
    #[must_use]
    pub fn direction(&self) -> Option<Pythagorean> {
        if !self.acts_in_plane() {
            return None;
        }
        let [w, _, _, z] = self.q.map(i128::from);
        // |w|, |z| < 2^36: squares and their sum below 2^73, 2wz below 2^73.
        let (ww, zz) = (w.checked_mul(w)?, z.checked_mul(z)?);
        let (a, b, c) = (
            ww.checked_sub(zz)?,
            w.checked_mul(z)?.checked_mul(2)?,
            ww.checked_add(zz)?,
        );
        Pythagorean::reduced(a, b, c)
    }

    /// The image of a grid point: M(q)·p/|q|² per coordinate, by exact integer arithmetic (below
    /// 3·2^112), rounded once to the nearest step, ties to even, with whether it was exact.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if a rounded coordinate lies outside the frame's ±(2^40 − 1)
    /// range. [`NumError::BudgetExceeded`] only if a checked step failed, which the budget rules
    /// out.
    pub fn apply(&self, p: P3) -> Result<Rounded<P3>, NumError> {
        let m = self.matrix()?;
        let n = budget(i128::try_from(self.norm2()).ok())?;
        let v = [p.x.get(), p.y.get(), p.z.get()].map(i128::from);
        let coord = |row: [i128; 3]| -> Result<Rounded<Coord>, NumError> {
            let mut num = 0i128;
            for (a, b) in row.iter().zip(v.iter()) {
                num = budget(a.checked_mul(*b).and_then(|t| num.checked_add(t)))?;
            }
            let r = budget(round_div(num, n))?;
            Ok(Rounded::new(Coord::from_i128(r.value())?, r.is_exact()))
        };
        let [r0, r1, r2] = m;
        let (x, y, z) = (coord(r0)?, coord(r1)?, coord(r2)?);
        Ok(Rounded::new(
            P3 {
                x: x.value(),
                y: y.value(),
                z: z.value(),
            },
            x.is_exact() && y.is_exact() && z.is_exact(),
        ))
    }

    /// The image of a 2D grid point: [`RationalRotation::apply`] at z = 0, whose z stays 0.
    ///
    /// # Errors
    ///
    /// [`MathError::NotInPlane`] if the rotation does not act in the plane; otherwise as
    /// [`RationalRotation::apply`].
    pub fn apply_2d(&self, p: P2) -> Result<Rounded<P2>, MathError> {
        if !self.acts_in_plane() {
            return Err(MathError::NotInPlane);
        }
        let q = self.apply(P3 {
            x: p.x,
            y: p.y,
            z: Coord::ZERO,
        })?;
        let exact = q.is_exact();
        let q = q.value();
        Ok(Rounded::new(P2 { x: q.x, y: q.y }, exact))
    }

    /// The classification: the identity, or a rigid motion that keeps orientation.
    #[must_use]
    pub fn classify(&self) -> Classification {
        let kind = if *self == RationalRotation::IDENTITY {
            TransformKind::Identity
        } else {
            TransformKind::Rigid
        };
        Classification {
            kind,
            mirror: false,
        }
    }

    /// The same rotation as a [`RationalTransform`] M(q)/|q|², when |q|² ≤ 2^40 (the rational
    /// transforms' denominator bound, which then bounds every entry too); `None` otherwise.
    #[must_use]
    pub fn to_rational(&self) -> Option<RationalTransform> {
        let n = self.norm2();
        if n > RATIONAL_NORM2 {
            return None;
        }
        let m = self.matrix().ok()?;
        let entry = |v: i128| i64::try_from(v).ok();
        let row = |r: [i128; 3]| -> Option<[i64; 3]> {
            let [a, b, c] = r;
            Some([entry(a)?, entry(b)?, entry(c)?])
        };
        let [r0, r1, r2] = m;
        RationalTransform::new(
            [row(r0)?, row(r1)?, row(r2)?],
            [0; 3],
            i64::try_from(n).ok()?,
        )
        .ok()
    }

    /// The rotation of a rational transform that is one: no translation, and M/d orthogonal with
    /// determinant one (classified exactly as a rigid motion or the identity, without a mirror).
    /// `None` for any other transform.
    ///
    /// With R = M/d = M(q)/|q|², the integer rows
    ///
    /// - (d + m₀₀ + m₁₁ + m₂₂, m₂₁ − m₁₂, m₀₂ − m₂₀, m₁₀ − m₀₁) = 4w·q·d/|q|²,
    /// - (m₂₁ − m₁₂, d + m₀₀ − m₁₁ − m₂₂, m₀₁ + m₁₀, m₀₂ + m₂₀) = 4x·q·d/|q|²,
    /// - (m₀₂ − m₂₀, m₀₁ + m₁₀, d − m₀₀ + m₁₁ − m₂₂, m₁₂ + m₂₁) = 4y·q·d/|q|²,
    /// - (m₁₀ − m₀₁, m₀₂ + m₂₀, m₁₂ + m₂₁, d − m₀₀ − m₁₁ + m₂₂) = 4z·q·d/|q|²
    ///
    /// (from the entries of M(q) in the module documentation, for instance
    /// 1 + tr R = 4w²/|q|²) are each a multiple of q, and the one with the largest diagonal entry
    /// 4qₖ²·d/|q|² is not zero, because those four sum to 4d. Its canonical form is q.
    #[must_use]
    pub fn from_rational(t: &RationalTransform) -> Option<RationalRotation> {
        if t.translation_part() != [0; 3] {
            return None;
        }
        let c = t.classify().ok()?;
        if c.mirror || !matches!(c.kind, TransformKind::Identity | TransformKind::Rigid) {
            return None;
        }
        let [[m00, m01, m02], [m10, m11, m12], [m20, m21, m22]] =
            t.matrix().map(|r| r.map(i128::from));
        let d = i128::from(t.denominator());
        // Entries and d are at most 2^40: every sum below 2^43.
        let rows = || -> Option<[[i128; 4]; 4]> {
            let (s01, s02, s12) = (
                m01.checked_add(m10)?,
                m02.checked_add(m20)?,
                m12.checked_add(m21)?,
            );
            let (a21, a02, a10) = (
                m21.checked_sub(m12)?,
                m02.checked_sub(m20)?,
                m10.checked_sub(m01)?,
            );
            Some([
                [
                    d.checked_add(m00)?.checked_add(m11)?.checked_add(m22)?,
                    a21,
                    a02,
                    a10,
                ],
                [
                    a21,
                    d.checked_add(m00)?.checked_sub(m11)?.checked_sub(m22)?,
                    s01,
                    s02,
                ],
                [
                    a02,
                    s01,
                    d.checked_sub(m00)?.checked_add(m11)?.checked_sub(m22)?,
                    s12,
                ],
                [
                    a10,
                    s02,
                    s12,
                    d.checked_sub(m00)?.checked_sub(m11)?.checked_add(m22)?,
                ],
            ])
        };
        let rows = rows()?;
        let mut best: Option<([i128; 4], i128)> = None;
        for (k, row) in rows.into_iter().enumerate() {
            let diag = row.iter().copied().nth(k)?;
            if best.is_none_or(|(_, b)| diag > b) {
                best = Some((row, diag));
            }
        }
        canonical(best?.0).ok()
    }

    /// The nearest floating-point transform: every entry of M(q)/|q|² rounded to the nearest
    /// double, ties to even.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only if a checked step failed, which the budget rules out;
    /// every entry is at most one in magnitude, so [`Transform::from_rows`] accepts it.
    pub fn to_transform(&self) -> Result<Transform, NumError> {
        let m = self.matrix()?;
        let n = self.norm2();
        let q = |v: i128| budget(rational_to_f64(v, n)).map(|r| r.value());
        let [r0, r1, r2] = m;
        let row = |r: [i128; 3]| -> Result<[f64; 4], NumError> {
            let [a, b, c] = r;
            Ok([q(a)?, q(b)?, q(c)?, 0.0])
        };
        Transform::from_rows([row(r0)?, row(r1)?, row(r2)?])
    }
}

impl Default for RationalRotation {
    /// The identity.
    fn default() -> Self {
        RationalRotation::IDENTITY
    }
}

impl Pythagorean {
    /// The direction (1, 0).
    pub const X: Pythagorean = Pythagorean { a: 1, b: 0, c: 1 };

    /// (a, b, c) in lowest terms with c > 0, or `None` if they are not a Pythagorean triple with
    /// c ≠ 0 (or a step does not fit).
    fn reduced(a: i128, b: i128, c: i128) -> Option<Pythagorean> {
        let g = gcd_u128(
            gcd_u128(a.unsigned_abs(), b.unsigned_abs()),
            c.unsigned_abs(),
        );
        if g == 0 {
            return None;
        }
        let g = i128::try_from(g).ok()?;
        let g = if c < 0 { g.checked_neg()? } else { g };
        let (a, b, c) = (a.checked_div(g)?, b.checked_div(g)?, c.checked_div(g)?);
        if c <= 0 {
            return None;
        }
        Some(Pythagorean { a, b, c })
    }

    /// The direction (a, b)/c, in lowest terms.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] unless a² + b² = c² with c ≠ 0.
    pub fn new(a: i64, b: i64, c: i64) -> Result<Pythagorean, NumError> {
        let (a, b, c) = (i128::from(a), i128::from(b), i128::from(c));
        let sq = |v: i128| v.checked_mul(v);
        // Squares of i64 values are below 2^126 and the sum of two below 2^127.
        let lhs = budget(sq(a).zip(sq(b)).and_then(|(x, y)| x.checked_add(y)))?;
        if c == 0 || lhs != budget(sq(c))? {
            return Err(NumError::Degenerate);
        }
        budget(Pythagorean::reduced(a, b, c))
    }

    /// The Pythagorean direction within `bound` of `direction`: the image of the x axis under
    /// [`RationalRotation::plane_within`] (Canny, Donald and Ressler's closest rational direction,
    /// with the smallest half-angle denominator the search finds).
    ///
    /// # Errors
    ///
    /// As [`RationalRotation::plane_within`].
    pub fn within(direction: F2, bound: Real) -> Result<WithinAngle<Pythagorean>, MathError> {
        let r = RationalRotation::plane_within(direction, bound)?;
        Ok(WithinAngle {
            value: budget(r.value.direction())?,
            angle: r.angle,
        })
    }

    /// a, the x numerator.
    #[must_use]
    pub const fn a(&self) -> i128 {
        self.a
    }

    /// b, the y numerator.
    #[must_use]
    pub const fn b(&self) -> i128 {
        self.b
    }

    /// c, the common denominator, positive.
    #[must_use]
    pub const fn c(&self) -> i128 {
        self.c
    }
}
