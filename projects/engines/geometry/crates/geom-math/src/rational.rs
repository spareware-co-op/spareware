//! Rational affine transforms: `p ↦ (M·p + t)/d` with an integer matrix M, an integer vector t and
//! a positive integer d.
//!
//! These are the transforms between the exact grid transforms and the certified floating-point
//! ones (chapter 7): a rotation by a Pythagorean direction such as (3, 4)/5, an exact rational
//! scale such as 0.99987654, a translation by a fraction of a step, and their compositions. A
//! [`RationalTransform`] is exact: it is applied to a grid point by exact integer arithmetic and
//! the image is rounded once, half to even, with whether it was already on the grid (chapter 7:
//! "the result says so"); it composes and inverts exactly; its classification is decided exactly,
//! with no tolerance at all.
//!
//! Guarantees: a [`RationalTransform`] is exact. Its image of a grid point is computed in exact
//! integer arithmetic and rounded once, half to even, with whether it was already on the grid; it
//! composes and inverts exactly; its classification is decided exactly, with no tolerance; equal
//! transforms have equal fields; and a step its budget proves fits that fails all the same is
//! reported as [`NumError::BudgetExceeded`], never answered.
//!
//! **Canonical form.** d > 0 and gcd(all entries of M, all of t, d) = 1, so equal transforms have
//! equal fields and `==` is equality of transforms.
//!
//! **Bit budgets.** Every transform keeps |mᵢⱼ| ≤ [`RATIONAL_ENTRY_MAX`] = 2^40,
//! 1 ≤ d ≤ [`RATIONAL_DEN_MAX`] = 2^40 and |tᵢ| ≤ [`RATIONAL_TRANSLATION_MAX`] = 2^84 (so a
//! translation of up to 2^44 steps fits any denominator). Then:
//!
//! - applying to a grid point (|pⱼ| ≤ 2^40 − 1): |Σ mᵢⱼpⱼ + tᵢ| < 3·2^80 + 2^84 < 2^86, in `i128`;
//! - composing: entries of M₁M₂ below 3·2^80, of M₁t₂ + d₂t₁ below 3·2^124 + 2^124 = 2^126, and
//!   d₁d₂ ≤ 2^80, all in `i128`; the result is reduced and refused with
//!   [`NumError::OutOfRange`] if it is still outside the bounds;
//! - inverting: det M below 6·2^120 < 2^123, the adjugate below 2^81, d·adj(M) below 2^121, all in
//!   `i128`; adj(M)·t below 3·2^165 < 2^167, in a 256-bit integer, reduced by the common divisor
//!   with exact long division, and refused with `OutOfRange` if still outside the bounds;
//! - classifying: the Gram matrix `MᵀM` below 3·2^80 and d² ≤ 2^80, in `i128`.
//!
//! Every integer step is a checked method, never a native operator, so nothing depends on overflow
//! checks being on. A step the budgets above prove fits can fail only through a defect in this
//! crate, and is then reported as [`NumError::BudgetExceeded`], never answered.
//!
//! Method: exact rational arithmetic on a common denominator, reduced by Euclid's algorithm
//! (D. E. Knuth, *The Art of Computer Programming*, vol. 2, section 4.5.2,
//! <https://www-cs-faculty.stanford.edu/~knuth/taocp.html>), the inverse by the adjugate (Cramer's
//! rule), and the classification by the Gram matrix as in [`crate::transform`] (R. Hartley and
//! A. Zisserman, *Multiple View Geometry in Computer Vision*, 2nd ed. (2004), section 2.4,
//! <https://doi.org/10.1017/CBO9780511811685>), here exactly.
//!
//! Cost: constant time, no allocation. [`RationalTransform::apply`] is nine `i128` products and
//! three Euclidean divisions. [`RationalTransform::compose`] is 40 `i128` products and a
//! reduction. [`RationalTransform::inverse`] is 36 `i128` products (determinant, adjugate and
//! scaling), nine 256-bit products, twelve greatest common divisors and six long divisions of a
//! 256-bit integer by their result (256 shift-and-subtract steps each), and a reduction.
//! [`RationalTransform::classify`] is at most 28 `i128` products. A reduction folds 13 values into
//! their greatest common divisor, then divides each by it. Every greatest common divisor is
//! Euclid's algorithm on `u128`, at most 257 remainder steps.

#![allow(
    clippy::many_single_char_names,
    reason = "variable names follow the formulas of the method each function cites"
)]

use geom_num::NumError;
use geom_num::grid::{Coord, P2, P3};
use geom_num::sign::Sign;
use geom_num::wide::Wide;

use crate::error::{MathError, budget};
use crate::exact::{divrem_u128, gcd_u128, rational_to_f64};
use crate::frame::{Rotation, Scale};
use crate::grid_transform::{Axis, GridTransform, TRANSLATION_MAX};
use crate::rounded::Rounded;
use crate::transform::{Classification, Transform, TransformKind};
use crate::vector::{V2, V3};

/// The largest magnitude of an entry of M: 2^40.
pub const RATIONAL_ENTRY_MAX: i64 = 1 << 40;

/// The largest denominator d: 2^40.
pub const RATIONAL_DEN_MAX: i64 = 1 << 40;

/// The largest magnitude of a component of t: 2^84.
pub const RATIONAL_TRANSLATION_MAX: i128 = 1 << 84;

/// An exact rational affine transform `p ↦ (M·p + t)/d`, in canonical form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RationalTransform {
    m: [[i64; 3]; 3],
    t: [i128; 3],
    d: i64,
}

/// `num / den` rounded to the nearest integer, ties to even, with whether it was exact (den > 0).
///
/// The callers' budgets keep |num| below 2^86 and 1 ≤ den ≤ 2^40, so the quotient q is below 2^86
/// in magnitude, 0 ≤ r < den makes 2r at most 2^41, and q + 1 fits: no step fails, and `None`
/// would report a defect.
fn round_div(num: i128, den: i128) -> Option<Rounded<i128>> {
    let q = num.checked_div_euclid(den)?;
    let r = num.checked_rem_euclid(den)?;
    let value = match r.checked_mul(2)?.cmp(&den) {
        core::cmp::Ordering::Less => q,
        core::cmp::Ordering::Greater => q.checked_add(1)?,
        core::cmp::Ordering::Equal => {
            if q.rem_euclid(2) == 0 {
                q
            } else {
                q.checked_add(1)?
            }
        }
    };
    Some(Rounded::new(value, r == 0))
}

/// The sum m₀x₀ + m₁x₁ + m₂x₂ of three products, exactly, or `None` if a step does not fit; each
/// caller bounds it.
fn dot3(m: [i128; 3], x: [i128; 3]) -> Option<i128> {
    let ([m0, m1, m2], [x0, x1, x2]) = (m, x);
    m0.checked_mul(x0)?
        .checked_add(m1.checked_mul(x1)?)?
        .checked_add(m2.checked_mul(x2)?)
}

/// a·d − b·c, exactly, or `None` if a step does not fit; each caller bounds it.
fn det2(a: i128, b: i128, c: i128, d: i128) -> Option<i128> {
    a.checked_mul(d)?.checked_sub(b.checked_mul(c)?)
}

/// The transform from unreduced parts, all of magnitude below 2^127: reduced by the greatest common
/// divisor, with d made positive; `OutOfRange` if it is still outside the bit budget, `Degenerate`
/// if d is zero.
fn reduce(m: [[i128; 3]; 3], t: [i128; 3], d: i128) -> Result<RationalTransform, NumError> {
    if d == 0 {
        return Err(NumError::Degenerate);
    }
    let all = || {
        m.iter()
            .flatten()
            .chain(t.iter())
            .chain(core::iter::once(&d))
    };
    if all().any(|&v| v == i128::MIN) {
        return Err(NumError::OutOfRange { value: i128::MIN });
    }
    let mut g = 0u128;
    for v in all() {
        g = gcd_u128(g, v.unsigned_abs());
    }
    // 1 ≤ g ≤ |d| < 2^127 because d ≠ 0, so g fits i128; dividing by it is exact, and the negation
    // below cannot overflow because every magnitude is below 2^127.
    let g = budget(i128::try_from(g).ok())?;
    let negate = d < 0;
    let reduced = |v: i128| -> Result<i128, NumError> {
        let q = budget(v.checked_div(g))?;
        if negate {
            budget(q.checked_neg())
        } else {
            Ok(q)
        }
    };
    let mut m_out = [[0i128; 3]; 3];
    for (out, row) in m_out.iter_mut().zip(m) {
        for (o, v) in out.iter_mut().zip(row) {
            *o = reduced(v)?;
        }
    }
    let mut t_out = [0i128; 3];
    for (o, v) in t_out.iter_mut().zip(t) {
        *o = reduced(v)?;
    }
    let (m, t, d) = (m_out, t_out, reduced(d)?);
    let entry = |v: i128| -> Result<i64, NumError> {
        if v.unsigned_abs() > RATIONAL_ENTRY_MAX.unsigned_abs().into() {
            return Err(NumError::OutOfRange { value: v });
        }
        i64::try_from(v).map_err(|_| NumError::OutOfRange { value: v })
    };
    if d > i128::from(RATIONAL_DEN_MAX) {
        return Err(NumError::OutOfRange { value: d });
    }
    let d = i64::try_from(d).map_err(|_| NumError::OutOfRange { value: d })?;
    if let Some(&v) = t
        .iter()
        .find(|v| v.unsigned_abs() > RATIONAL_TRANSLATION_MAX.unsigned_abs())
    {
        return Err(NumError::OutOfRange { value: v });
    }
    let [r0, r1, r2] = m;
    let row = |r: [i128; 3]| -> Result<[i64; 3], NumError> {
        let [a, b, c] = r;
        Ok([entry(a)?, entry(b)?, entry(c)?])
    };
    Ok(RationalTransform {
        m: [row(r0)?, row(r1)?, row(r2)?],
        t,
        d,
    })
}

impl RationalTransform {
    /// The identity.
    pub const IDENTITY: RationalTransform = RationalTransform {
        m: [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
        t: [0; 3],
        d: 1,
    };

    /// The transform `p ↦ (M·p + t)/d`, brought to canonical form.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] if d is zero; [`NumError::OutOfRange`] if, after reduction, an
    /// entry, the denominator or a translation component exceeds its bound.
    pub fn new(m: [[i64; 3]; 3], t: [i128; 3], d: i64) -> Result<RationalTransform, NumError> {
        if let Some(&v) = t.iter().find(|v| v.unsigned_abs() >= 1u128 << 126) {
            return Err(NumError::OutOfRange { value: v });
        }
        reduce(m.map(|r| r.map(i128::from)), t, i128::from(d))
    }

    /// A grid transform, exactly (denominator one).
    #[must_use]
    pub fn from_grid(g: &GridTransform) -> RationalTransform {
        let m = g.matrix().map(|r| r.map(i64::from));
        let t = g.translation_part().map(i128::from);
        // Entries are 0 and ±1 and translations at most 2^41 − 2: already canonical and in range.
        RationalTransform { m, t, d: 1 }
    }

    /// The rotation about `axis` through the origin whose cosine and sine are c/r and s/r for a
    /// rotation with an integer length r (a quarter turn or a Pythagorean direction such as
    /// (3, 4)), counter-clockwise looking from the positive end of the axis.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] if the rotation's length √(c² + s²) is irrational (such a rotation
    /// is not rational; use a certified [`Transform`]); [`NumError::OutOfRange`] if r exceeds
    /// [`RATIONAL_DEN_MAX`].
    pub fn rotation(axis: Axis, rotation: Rotation) -> Result<RationalTransform, NumError> {
        let r = i64::from(rotation.exact_norm().ok_or(NumError::Degenerate)?);
        let (c, s) = (i64::from(rotation.cos()), i64::from(rotation.sin()));
        // s is an i32 widened to i64, so −s fits.
        let ns = budget(s.checked_neg())?;
        let m = match axis {
            Axis::X => [[r, 0, 0], [0, c, ns], [0, s, c]],
            Axis::Y => [[c, 0, s], [0, r, 0], [ns, 0, c]],
            Axis::Z => [[c, ns, 0], [s, c, 0], [0, 0, r]],
        };
        RationalTransform::new(m, [0; 3], r)
    }

    /// The uniform scale by the exact ratio k = `k_n/k_d` about the origin.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if `k_n` or `k_d` exceeds 2^40 after reduction (a [`Scale`] is
    /// reduced, so only ratios with very large parts are refused).
    pub fn uniform_scale(scale: Scale) -> Result<RationalTransform, NumError> {
        let (n, d) = (i64::from(scale.num()), i64::from(scale.den()));
        RationalTransform::new([[n, 0, 0], [0, n, 0], [0, 0, n]], [0; 3], d)
    }

    /// The translation by t/d steps.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] if d is zero; [`NumError::OutOfRange`] if a part exceeds its bound.
    pub fn translation(t: [i128; 3], d: i64) -> Result<RationalTransform, NumError> {
        RationalTransform::new([[d, 0, 0], [0, d, 0], [0, 0, d]], t, d)
    }

    /// The integer matrix M.
    #[must_use]
    pub const fn matrix(&self) -> [[i64; 3]; 3] {
        self.m
    }

    /// The integer translation numerator t.
    #[must_use]
    pub const fn translation_part(&self) -> [i128; 3] {
        self.t
    }

    /// The denominator d, at least one.
    #[must_use]
    pub const fn denominator(&self) -> i64 {
        self.d
    }

    /// The same transform as a grid transform, when it is one: denominator one, M a signed
    /// permutation and t within [`TRANSLATION_MAX`].
    #[must_use]
    pub fn to_grid(&self) -> Option<GridTransform> {
        if self.d != 1 {
            return None;
        }
        // A row of a signed permutation: exactly one nonzero entry, and that entry ±1.
        let row = |r: [i64; 3]| -> Option<(Axis, bool)> {
            let [a, b, c] = r;
            let (axis, v) = match (a != 0, b != 0, c != 0) {
                (true, false, false) => (Axis::X, a),
                (false, true, false) => (Axis::Y, b),
                (false, false, true) => (Axis::Z, c),
                _ => return None,
            };
            match v {
                1 => Some((axis, false)),
                -1 => Some((axis, true)),
                _ => None,
            }
        };
        let [r0, r1, r2] = self.m;
        let ((s0, n0), (s1, n1), (s2, n2)) = (row(r0)?, row(r1)?, row(r2)?);
        let tr = |v: i128| -> Option<i64> {
            if v.unsigned_abs() > u128::from(TRANSLATION_MAX.unsigned_abs()) {
                return None;
            }
            i64::try_from(v).ok()
        };
        let [t0, t1, t2] = self.t;
        GridTransform::from_parts([s0, s1, s2], [n0, n1, n2], [tr(t0)?, tr(t1)?, tr(t2)?]).ok()
    }

    /// The nearest floating-point transform: every entry mᵢⱼ/d and tᵢ/d rounded to the nearest
    /// double, ties to even. Exact when d is a power of two and the numerators fit 53 bits;
    /// otherwise an approximation, which is why converting to a [`Transform`] is a change of
    /// variant that the [`TypedTransform`](crate::TypedTransform) records.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only if a quotient was refused, which cannot happen: d ≥ 1 and
    /// d ≤ 2^40 < 2^127 is every quotient's precondition. Every quotient is below 2^84 in
    /// magnitude, so finite, and [`Transform::from_rows`] accepts them; its error would be
    /// returned if it did not. For every rational transform the result is `Ok`.
    pub fn to_transform(&self) -> Result<Transform, NumError> {
        let d = u128::from(self.d.unsigned_abs());
        let q = |v: i128| budget(rational_to_f64(v, d)).map(|r| r.value());
        let [r0, r1, r2] = self.m;
        let [t0, t1, t2] = self.t;
        let row = |r: [i64; 3], t: i128| -> Result<[f64; 4], NumError> {
            let [a, b, c] = r;
            Ok([q(a.into())?, q(b.into())?, q(c.into())?, q(t)?])
        };
        Transform::from_rows([row(r0, t0)?, row(r1, t1)?, row(r2, t2)?])
    }

    /// The image of a grid point: (M·p + t)/d per coordinate, by exact integer arithmetic (below
    /// 2^86, see the bit budgets), rounded once to the nearest step, ties to even, with whether it
    /// was already on the grid.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if a rounded coordinate lies outside the frame's ±(2^40 − 1)
    /// range. [`NumError::BudgetExceeded`] only if a checked step failed, which the bit budgets
    /// rule out.
    pub fn apply(&self, p: P3) -> Result<Rounded<P3>, NumError> {
        let v = [p.x.get(), p.y.get(), p.z.get()].map(i128::from);
        let d = i128::from(self.d);
        let coord = |r: [i64; 3], t: i128| -> Result<Rounded<Coord>, NumError> {
            let num = budget(dot3(r.map(i128::from), v).and_then(|s| s.checked_add(t)))?;
            let rounded = budget(round_div(num, d))?;
            Ok(Rounded::new(
                Coord::from_i128(rounded.value())?,
                rounded.is_exact(),
            ))
        };
        let [r0, r1, r2] = self.m;
        let [t0, t1, t2] = self.t;
        let (x, y, z) = (coord(r0, t0)?, coord(r1, t1)?, coord(r2, t2)?);
        Ok(Rounded::new(
            P3 {
                x: x.value(),
                y: y.value(),
                z: z.value(),
            },
            x.is_exact() && y.is_exact() && z.is_exact(),
        ))
    }

    /// Whether the transform acts in the plane: it leaves every z unchanged (its z row is
    /// (0, 0, d)/d with no translation), so it maps the plane z = 0 of a 2D frame onto itself; its
    /// determinant is then that of its 2 × 2 part, so its orientation in the plane is its
    /// orientation in 3D.
    #[must_use]
    pub fn acts_in_plane(&self) -> bool {
        let ([_, _, z], [_, _, tz]) = (self.m, self.t);
        z == [0, 0, self.d] && tz == 0
    }

    /// The image of a 2D grid point: [`RationalTransform::apply`] at z = 0, whose z stays 0 exactly,
    /// so the result is exact exactly when x and y are.
    ///
    /// # Errors
    ///
    /// [`MathError::NotInPlane`] if the transform does not act in the plane; otherwise as
    /// [`RationalTransform::apply`].
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

    /// The image of a 2D grid vector: [`RationalTransform::apply_vector`] at z = 0.
    ///
    /// # Errors
    ///
    /// [`MathError::NotInPlane`] if the transform does not act in the plane; otherwise as
    /// [`RationalTransform::apply_vector`].
    pub fn apply_vector_2d(&self, v: V2) -> Result<Rounded<V2>, MathError> {
        if !self.acts_in_plane() {
            return Err(MathError::NotInPlane);
        }
        let w = self.apply_vector(V3::new(v.x(), v.y(), 0)?)?;
        let exact = w.is_exact();
        let w = w.value();
        Ok(Rounded::new(V2::new(w.x(), w.y())?, exact))
    }

    /// The image of a grid vector (the linear part only): M·v/d per component, rounded once to the
    /// nearest step, ties to even, with whether it was exact.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if a rounded component exceeds the vector range.
    /// [`NumError::BudgetExceeded`] only if a checked step failed, which the bit budgets rule out.
    pub fn apply_vector(&self, v: V3) -> Result<Rounded<V3>, NumError> {
        let a = v.to_array().map(i128::from);
        let d = i128::from(self.d);
        // |entries| ≤ 2^40 and |components| < 2^41: each product below 2^81, the sum below 2^83.
        let comp = |r: [i64; 3]| -> Result<Rounded<i128>, NumError> {
            budget(dot3(r.map(i128::from), a).and_then(|s| round_div(s, d)))
        };
        let [r0, r1, r2] = self.m;
        let (x, y, z) = (comp(r0)?, comp(r1)?, comp(r2)?);
        let out = |v: i128| i64::try_from(v).map_err(|_| NumError::OutOfRange { value: v });
        Ok(Rounded::new(
            V3::new(out(x.value())?, out(y.value())?, out(z.value())?)?,
            x.is_exact() && y.is_exact() && z.is_exact(),
        ))
    }

    /// The composition `self ∘ first`: the transform that applies `first`, then `self`:
    /// (M₁M₂·p + M₁t₂ + d₂t₁)/(d₁d₂), reduced (bit budget in the module documentation).
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if the reduced result exceeds the bit budget.
    /// [`NumError::BudgetExceeded`] only if a checked step failed, which the module's bit budgets
    /// rule out.
    pub fn compose(&self, first: &RationalTransform) -> Result<RationalTransform, NumError> {
        let a = self.m.map(|r| r.map(i128::from));
        let [b0, b1, b2] = first.m.map(|r| r.map(i128::from));
        let col = |j: Axis| -> [i128; 3] { [j.pick(b0), j.pick(b1), j.pick(b2)] };
        let (c0, c1, c2) = (col(Axis::X), col(Axis::Y), col(Axis::Z));
        let mut m = [[0i128; 3]; 3];
        for (out, r) in m.iter_mut().zip(a) {
            *out = [
                budget(dot3(r, c0))?,
                budget(dot3(r, c1))?,
                budget(dot3(r, c2))?,
            ];
        }
        let d1 = i128::from(self.d);
        let d2 = i128::from(first.d);
        let mut t = [0i128; 3];
        for ((out, r), t1) in t.iter_mut().zip(a).zip(self.t) {
            let moved = dot3(r, first.t);
            *out = budget(
                moved
                    .zip(d2.checked_mul(t1))
                    .and_then(|(p, q)| p.checked_add(q)),
            )?;
        }
        reduce(m, t, budget(d1.checked_mul(d2))?)
    }

    /// The determinant of M, exactly (below 6·2^120 in magnitude); `None` only if a checked step
    /// failed, which that bound rules out.
    fn det(&self) -> Option<i128> {
        let [[a, b, c], [d, e, f], [g, h, k]] = self.m.map(|r| r.map(i128::from));
        let ta = a.checked_mul(det2(e, f, h, k)?)?;
        let tb = b.checked_mul(det2(d, f, g, k)?)?;
        let tc = c.checked_mul(det2(d, e, g, h)?)?;
        ta.checked_sub(tb)?.checked_add(tc)
    }

    /// The inverse transform, exactly: p ↦ (d·adj(M)·p − adj(M)·t)/det(M), reduced.
    ///
    /// # Errors
    ///
    /// [`MathError::Singular`] if det M = 0; [`MathError::Num`] with [`NumError::OutOfRange`] if
    /// the reduced inverse exceeds the bit budget (its value is the determinant), or with
    /// [`NumError::BudgetExceeded`] only if a checked step failed, which the module's bit budgets
    /// rule out.
    pub fn inverse(&self) -> Result<RationalTransform, MathError> {
        let det = budget(self.det())?;
        if det == 0 {
            return Err(MathError::Singular);
        }
        let [[a, b, c], [d, e, f], [g, h, k]] = self.m.map(|r| r.map(i128::from));
        // Each cofactor is a 2 × 2 determinant of entries at most 2^40: below 2^81.
        let adj = [
            [
                budget(det2(e, f, h, k))?,
                budget(det2(c, b, k, h))?,
                budget(det2(b, c, e, f))?,
            ],
            [
                budget(det2(f, d, k, g))?,
                budget(det2(a, c, g, k))?,
                budget(det2(c, a, f, d))?,
            ],
            [
                budget(det2(d, e, g, h))?,
                budget(det2(b, a, h, g))?,
                budget(det2(a, b, d, e))?,
            ],
        ];
        let dd = i128::from(self.d);
        let mut m = [[0i128; 3]; 3];
        for (out, row) in m.iter_mut().zip(adj) {
            for (o, v) in out.iter_mut().zip(row) {
                *o = budget(dd.checked_mul(v))?;
            }
        }
        // −adj·t in 256 bits: each product below 2^165, the sum below 2^167.
        let wide = |v: i128| Wide::<4>::from_i128(v);
        let tt = self.t.map(wide);
        let neg_dot = |r: [i128; 3]| -> Result<Wide<4>, NumError> {
            let mut acc = Wide::<4>::ZERO;
            for (x, y) in r.iter().zip(tt.iter()) {
                acc = budget(wide(*x).checked_mul(y).and_then(|p| acc.checked_sub(&p)))?;
            }
            Ok(acc)
        };
        let overflow = MathError::Num(NumError::OutOfRange { value: det });
        let [a0, a1, a2] = adj;
        let (w0, w1, w2) = (neg_dot(a0)?, neg_dot(a1)?, neg_dot(a2)?);
        // The common divisor of everything: first of M and det (in i128), then folding in each
        // component of t through its remainder modulo the divisor so far (exact long division).
        // 1 ≤ g ≤ |det| < 2^123, inside the division's range.
        let mut g = det.unsigned_abs();
        for v in m.iter().flatten() {
            g = gcd_u128(g, v.unsigned_abs());
        }
        for w in [w0, w1, w2] {
            let (_, r) = budget(divrem_u128(&w, g))?;
            g = gcd_u128(g, r);
        }
        let mut t_red = [0i128; 3];
        for (slot, w) in t_red.iter_mut().zip([w0, w1, w2]) {
            let (q, _) = budget(divrem_u128(&w, g))?;
            // A reduced component can still exceed i128: then the inverse is out of the budget.
            *slot = q.to_i128().ok_or(overflow)?;
        }
        let gi = budget(i128::try_from(g).ok())?;
        let mut m_red = [[0i128; 3]; 3];
        for (out, row) in m_red.iter_mut().zip(m) {
            for (o, v) in out.iter_mut().zip(row) {
                *o = budget(v.checked_div(gi))?;
            }
        }
        Ok(reduce(m_red, t_red, budget(det.checked_div(gi))?)?)
    }

    /// The exact classification (no tolerance): `Identity` and `Translation` when M = d·I;
    /// `Degenerate` when det M = 0; otherwise `Rigid` when `MᵀM` = d²·I, `UniformScale` when
    /// `MᵀM` = λ·I with λ ≠ d², else `NonUniform`; a mirror when det M < 0.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only if a checked step failed, which the module's bit budgets
    /// rule out: for every rational transform the result is `Ok`.
    pub fn classify(&self) -> Result<Classification, NumError> {
        let d = i128::from(self.d);
        let m = self.m.map(|r| r.map(i128::from));
        let identity = m == [[d, 0, 0], [0, d, 0], [0, 0, d]];
        if identity {
            let kind = if self.t == [0; 3] {
                TransformKind::Identity
            } else {
                TransformKind::Translation
            };
            return Ok(Classification {
                kind,
                mirror: false,
            });
        }
        let mirror = match Sign::of_i128(budget(self.det())?) {
            Sign::Zero => {
                return Ok(Classification {
                    kind: TransformKind::Degenerate,
                    mirror: false,
                });
            }
            Sign::Negative => true,
            Sign::Positive => false,
        };
        let [[a, b, c], [e, f, g], [h, k, l]] = m;
        let (c0, c1, c2) = ([a, e, h], [b, f, k], [c, g, l]);
        let dot = |u: [i128; 3], v: [i128; 3]| budget(dot3(u, v));
        let (g00, g11, g22) = (dot(c0, c0)?, dot(c1, c1)?, dot(c2, c2)?);
        let off = [dot(c0, c1)?, dot(c0, c2)?, dot(c1, c2)?];
        let scaled_identity = g00 == g11 && g11 == g22 && off == [0; 3];
        let kind = if scaled_identity && g00 == budget(d.checked_mul(d))? {
            TransformKind::Rigid
        } else if scaled_identity {
            TransformKind::UniformScale
        } else {
            TransformKind::NonUniform
        };
        Ok(Classification { kind, mirror })
    }
}

impl Default for RationalTransform {
    /// The identity.
    fn default() -> Self {
        RationalTransform::IDENTITY
    }
}
