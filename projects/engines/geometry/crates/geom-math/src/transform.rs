//! General affine transforms in `f64`, classified so callers know when a kind changes.
//!
//! A [`Transform`] is `p ↦ L·p + t` with a 3 × 3 linear part `L` and a translation `t`, stored as
//! three rows `[l₀, l₁, l₂, t]` of finite doubles. Transforms that keep the grid exact are
//! [`GridTransform`]s, and transforms with rational entries are
//! [`RationalTransform`](crate::RationalTransform)s; this type is for everything else, the
//! *certified* member of [`TypedTransform`](crate::TypedTransform) (chapter 7: "other rotations
//! round to the grid and the result says so"). Its meaning is the exact real transform with these
//! exact `f64` entries.
//!
//! **Classification** ([`Transform::classify`]) tells a caller what a transform does to kinds of
//! geometry (chapter 7: "transforms can change a kind"). With ε = ε_ang = 2^−34
//! ([`geom_num::ladder::ANGULAR`]), the ladder's angular bound and the only tolerance used:
//!
//! | Kind | Linear part | Circles | Lengths |
//! | --- | --- | --- | --- |
//! | `Identity` | exactly I, no translation | kept | kept |
//! | `Translation` | exactly I | kept | kept |
//! | `Rigid` | every |L·v| within a relative ε of |v| | kept | kept within a relative ε |
//! | `UniformScale` | every |L·v| within a relative ε of s·|v|, s ≠ 1 | kept | scaled by s |
//! | `NonUniform` | anything else invertible | become ellipses | changed |
//! | `Degenerate` | singular, or orientation not certain | collapse | collapse |
//!
//! and separately whether it is a **mirror** (negative determinant), which reverses arcs, winding
//! and face orientation. A rotation built from `sinpi` and `cospi` is orthonormal only to
//! rounding, about 2^−52, so an exact test would call every rotation non-uniform; the angular bound
//! is what "rigid" means for floating-point transforms.
//!
//! How each test is certified, never guessed:
//!
//! - **Lengths.** |L·v|² = vᵀGv with the Gram matrix G = LᵀL, so every |L·v|² lies within
//!   λ|v|² of |v|², where λ bounds the eigenvalues of G − I. Gershgorin's theorem bounds them by
//!   the largest absolute row sum of G − I. `Rigid` requires that row sum to be certainly at most
//!   2ε − ε², computed in interval arithmetic (`geom_num::interval`) on the exact entries; then
//!   |L·v|/|v| lies in [√(1 − 2ε + ε²), √(1 + 2ε − ε²)] ⊂ [1 − ε, 1 + ε]. `UniformScale` applies
//!   the same test to G/G₀₀ (s² = G₀₀, the squared length of L's first column), computed on L
//!   scaled by a power of two so that G cannot overflow or underflow.
//! - **Orientation.** The determinant is enclosed in interval arithmetic after each row of L is
//!   scaled by the power of two that brings its largest entry into [1, 2) (a positive factor, so
//!   the sign is unchanged, and no product can overflow; an entry that would fall below 2^−1022 is
//!   enclosed between its two neighbouring doubles). The transform is a mirror when the enclosure
//!   is negative, and `Degenerate` when it contains zero: singular, or so close to singular that
//!   the sign is not certain. A transform whose determinant would overflow or underflow, such as a
//!   uniform scale by 10^103 or 10^−110, is classified like any other.
//!
//! Guarantees:
//!
//! - **Finite.** Every entry is finite; operations whose results would overflow return
//!   [`NumError::NotFinite`]. Zero entries are stored as +0, so equal transforms have equal bits.
//! - **Deterministic.** Each entry of a product is evaluated as `(a₀b₀ + a₁b₁) + a₂b₂`, in that
//!   order, with IEEE 754 operations only; no fused multiply-add, no platform maths.
//! - **Never a wrong orientation.** [`Classification::mirror`] comes from a certified determinant
//!   sign. When the sign is not certain the transform is `Degenerate`, and [`Transform::inverse`]
//!   refuses it with [`MathError::Singular`].
//! - **Grid points round once.** [`Transform::apply_grid`] gives the exact image of a grid point
//!   under the exact entries, rounded to the nearest grid point, ties to even, with whether it was
//!   already on the grid. An image outside the frame is refused with its exact rounded coordinate
//!   (saturated at the ends of `i128`, as [`NumError::OutOfRange`] documents), never an estimate.
//! - **Defects reported, never answered.** The integer steps (the exponents of the row scalings
//!   and of the exact rounding) are checked methods, and the maths functions' errors are passed
//!   on. For finite entries none can fail; one that did is reported as
//!   [`NumError::BudgetExceeded`] (or the maths function's own error), never turned into a
//!   classification or a coordinate. Where a stage of [`Transform::apply_grid`] can decline (the
//!   two filters), such a failure only makes it decline, and the exact stage decides.
//!
//! Method: the hierarchy of planar and spatial transformations (isometry, similarity, affinity) of
//! R. Hartley and A. Zisserman, *Multiple View Geometry in Computer Vision*, 2nd ed., Cambridge
//! University Press (2004), section 2.4, <https://doi.org/10.1017/CBO9780511811685>, with each
//! level recognised from the Gram matrix LᵀL; Gershgorin's theorem for the eigenvalue bound
//! (R. A. Horn and C. R. Johnson, *Matrix Analysis*, 2nd ed., Cambridge University Press (2012),
//! section 6.1, <https://doi.org/10.1017/CBO9781139020411>); the inverse by the adjugate (Cramer's
//! rule) on the row-scaled matrix; the certified tests by interval arithmetic (R. E. Moore,
//! R. B. Kearfott and M. J. Cloud, *Introduction to Interval Analysis*, SIAM (2009),
//! <https://doi.org/10.1137/1.9780898717716>); exact rounding of grid images by the
//! filter-then-exact pattern of the kernel's predicates (J. R. Shewchuk, *Discrete & Computational
//! Geometry* 18 (1997), <https://doi.org/10.1007/PL00009321>).
//!
//! Cost: constant time, no allocation. [`Transform::apply_point`] is 18 IEEE operations and
//! [`Transform::compose`] 63. [`Transform::classify`] is fewer than 100 interval operations (the
//! determinant enclosure, then one or two Gram matrices and their row sums), each a few IEEE
//! operations with a one-step widening. [`Transform::inverse`] is a classification, 60 IEEE
//! operations and 18 power-of-two scalings. [`Transform::apply_grid`] decides each coordinate by
//! the cheapest stage that can: about twenty IEEE operations in plain doubles; if those cannot
//! decide, three exact products and three double-double additions; only then the exact sum of
//! four shifted terms in an integer of 128 bits up to 2176 bits (34 limbs, chosen by how far apart
//! the exponents are) and one exact rounding, each linear in the limbs. A coordinate outside the
//! frame always takes that exact stage, which finds the rounded coordinate its error reports.

use geom_num::grid::{Coord, P3};
use geom_num::interval::Interval;
use geom_num::ladder::ANGULAR;
use geom_num::sign::Sign;
use geom_num::wide::Wide;
use geom_num::{NumError, math};

use crate::dd::DD;
use crate::error::{MathError, budget};
use crate::exact::{const_pow2, dyadic, exponent, round_scaled, scale_b, shl_into};
use crate::float::F3;
use crate::grid_transform::{Axis, GridTransform};
use crate::rounded::Rounded;

/// What a transform does to kinds of geometry. See the module documentation for the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransformKind {
    /// The identity: linear part exactly the identity matrix and no translation.
    Identity,
    /// A pure translation: linear part exactly the identity matrix.
    Translation,
    /// A rotation, possibly with a mirror and a translation: every length is kept within a
    /// relative ε_ang.
    Rigid,
    /// A rigid transform followed by a uniform scale s ≠ 1: every length is multiplied by s within
    /// a relative ε_ang.
    UniformScale,
    /// Any other invertible transform: a circle becomes an ellipse, a sphere an ellipsoid.
    NonUniform,
    /// The linear part is singular, or so close to singular that its determinant's sign cannot be
    /// certified: the kernel treats it as collapsing space onto a plane, a line or a point, with no
    /// inverse.
    Degenerate,
}

/// The classification of a transform: its kind and whether it reverses orientation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Classification {
    /// What the transform does to kinds.
    pub kind: TransformKind,
    /// Whether the determinant is certainly negative: the transform includes a mirror, so arcs run
    /// the other way and outward normals flip. Always false for `Degenerate`.
    pub mirror: bool,
}

impl Classification {
    /// Whether circles stay circles (and spheres stay spheres): the kind is `Identity`,
    /// `Translation`, `Rigid` or `UniformScale`.
    #[must_use]
    pub const fn keeps_circles(self) -> bool {
        matches!(
            self.kind,
            TransformKind::Identity
                | TransformKind::Translation
                | TransformKind::Rigid
                | TransformKind::UniformScale
        )
    }

    /// Whether lengths are kept (within a relative ε_ang for `Rigid`).
    #[must_use]
    pub const fn keeps_lengths(self) -> bool {
        matches!(
            self.kind,
            TransformKind::Identity | TransformKind::Translation | TransformKind::Rigid
        )
    }

    /// Whether orientation is reversed: the same as [`Classification::mirror`].
    #[must_use]
    pub const fn reverses_orientation(self) -> bool {
        self.mirror
    }
}

/// An affine transform `p ↦ L·p + t` of finite doubles, rows `[l₀, l₁, l₂, t]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    m: [[f64; 4]; 3],
}

/// The largest certified row sum of |G − I| for a rigid transform: 2ε − ε² = 2^−33 − 2^−68, an
/// exact double, so that √(1 ± (2ε − ε²)) lies within [1 − ε, 1 + ε].
const ROW_SUM_MAX: f64 = 2.0 * ANGULAR.get() - ANGULAR.get() * ANGULAR.get();

/// The relative factor of the double-double filter in [`Transform::apply_grid`]: 2^−98 = 256u²,
/// well above the 15.01u² the three additions can lose (see `round_row`).
const FILTER_FACTOR: f64 = const_pow2!(-98);

/// The relative factor of the floating-point filter, 2^−50 = 8u (see `float_row`).
const FLOAT_FACTOR: f64 = const_pow2!(-50);

/// The floating-point filter's absolute floor, 2^−1070 (a subnormal power of two, the exact
/// product 2^−1022 · 2^−48; see `float_row`).
const FLOAT_FLOOR: f64 = const_pow2!(-1022) * const_pow2!(-48);

/// The double-double filter's slack, 2^−52 (see `filtered_row`).
const FILTERED_SLACK: f64 = const_pow2!(-52);

/// 2^41: a coordinate value at or beyond it is out of every frame's range.
const RANGE_LIMIT: f64 = const_pow2!(41);

// ε² is exact (2^−68) and so is 2ε − ε², a double with 36 significant bits.
const _: () = assert!(ROW_SUM_MAX == 2.0 * const_pow2!(-34) - const_pow2!(-68));

/// A 3 × 3 matrix of enclosures.
type Enclosed = [[Interval; 3]; 3];

/// −0 becomes +0; every other finite value is unchanged.
const fn canon(x: f64) -> f64 {
    x + 0.0
}

/// The larger of two doubles that are not NaN.
const fn max2(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// The enclosure of the exact value x·2^k: the point x·2^k when that is a double (then
/// [`scale_b`] is exact), otherwise the two doubles either side of the correctly rounded value.
/// `Ok(None)` if it overflows, which certifies nothing. k is a row scaling, from −1023 to 1074, so
/// −k fits `i32`; a failed step is reported, never taken as an overflow.
fn scaled_point(x: f64, k: i32) -> Result<Option<Interval>, NumError> {
    let y = budget(scale_b(x, k))?;
    if !y.is_finite() {
        return Ok(None);
    }
    let back = budget(scale_b(y, budget(k.checked_neg())?))?;
    Ok(if back == x {
        Interval::point(y).ok()
    } else {
        Interval::new(y.next_down(), y.next_up()).ok()
    })
}

/// The determinant of an enclosed matrix, expanded along the first row in the same order as the
/// floating-point evaluation in [`Transform::inverse`]:
/// l₀₀(l₁₁l₂₂ − l₁₂l₂₁) − l₀₁(l₁₀l₂₂ − l₁₂l₂₀) + l₀₂(l₁₀l₂₁ − l₁₁l₂₀). `None` if a bound overflows.
fn det_of(m: &Enclosed) -> Option<Interval> {
    let [[a, b, c], [d, e, f], [g, h, k]] = *m;
    let minor = |w: Interval, x: Interval, y: Interval, z: Interval| -> Option<Interval> {
        w.mul(x).ok()?.sub(y.mul(z).ok()?).ok()
    };
    let m1 = minor(e, k, f, h)?;
    let m2 = minor(d, k, f, g)?;
    let m3 = minor(d, h, e, g)?;
    a.mul(m1)
        .ok()?
        .sub(b.mul(m2).ok()?)
        .ok()?
        .add(c.mul(m3).ok()?)
        .ok()
}

/// The Gram matrix LᵀL (dot products of the columns of L) of an enclosed matrix, in the order
/// [G₀₀, G₁₁, G₂₂, G₀₁, G₀₂, G₁₂]. `None` if a bound overflows.
fn gram_of(m: &Enclosed) -> Option<[Interval; 6]> {
    let [[a, b, c], [d, e, f], [g, h, k]] = *m;
    let dot = |u: [Interval; 3], v: [Interval; 3]| -> Option<Interval> {
        let [u0, u1, u2] = u;
        let [v0, v1, v2] = v;
        u0.mul(v0)
            .ok()?
            .add(u1.mul(v1).ok()?)
            .ok()?
            .add(u2.mul(v2).ok()?)
            .ok()
    };
    let (c0, c1, c2) = ([a, d, g], [b, e, h], [c, f, k]);
    Some([
        dot(c0, c0)?,
        dot(c1, c1)?,
        dot(c2, c2)?,
        dot(c0, c1)?,
        dot(c0, c2)?,
        dot(c1, c2)?,
    ])
}

/// Whether the symmetric matrix M with these enclosures (order [M₀₀, M₁₁, M₂₂, M₀₁, M₀₂, M₁₂]) is
/// certainly within [`ROW_SUM_MAX`] of the identity in every absolute row sum of M − I, so that by
/// Gershgorin's theorem every eigenvalue of M − I lies within it. `None` if a bound overflows.
fn near_identity(m: [Interval; 6]) -> Option<bool> {
    let [m00, m11, m22, m01, m02, m12] = m;
    let one = Interval::point(1.0).ok()?;
    // An upper bound of |x − c| over the enclosure: the larger magnitude of its shifted bounds,
    // max(−lo, hi) since lo ≤ hi (as `geom_num::interval` bounds a square). Negation is exact, so
    // this is max(|lo|, |hi|) with no rounding; a zero's sign is made +0 by the sums below.
    let dev = |iv: Interval, shift: bool| -> Option<f64> {
        let d = if shift { iv.sub(one).ok()? } else { iv };
        Some(max2(-d.lo(), d.hi()))
    };
    let (d00, d11, d22) = (dev(m00, true)?, dev(m11, true)?, dev(m22, true)?);
    let (d01, d02, d12) = (dev(m01, false)?, dev(m02, false)?, dev(m12, false)?);
    // Each row sum, as the upper bound of an interval sum, so its rounding cannot hide an excess.
    let row = |a: f64, b: f64, c: f64| -> Option<f64> {
        let p = |x: f64| Interval::point(x).ok();
        Some(p(a)?.add(p(b)?).ok()?.add(p(c)?).ok()?.hi())
    };
    let rows = [
        row(d00, d01, d02)?,
        row(d01, d11, d12)?,
        row(d02, d12, d22)?,
    ];
    Some(rows.iter().all(|&r| r <= ROW_SUM_MAX))
}

impl Transform {
    /// The identity transform.
    pub const IDENTITY: Transform = Transform {
        m: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
    };

    /// The transform with rows `[l₀, l₁, l₂, t]`.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if an entry is NaN or infinite.
    pub fn from_rows(rows: [[f64; 4]; 3]) -> Result<Transform, NumError> {
        if rows.iter().flatten().all(|x| x.is_finite()) {
            Ok(Transform {
                m: rows.map(|r| r.map(canon)),
            })
        } else {
            Err(NumError::NotFinite)
        }
    }

    /// The rows `[l₀, l₁, l₂, t]`.
    #[must_use]
    pub const fn rows(&self) -> [[f64; 4]; 3] {
        self.m
    }

    /// The translation by `t`.
    #[must_use]
    pub fn translation(t: F3) -> Transform {
        let [x, y, z] = t.to_array();
        Transform {
            m: [
                [1.0, 0.0, 0.0, canon(x)],
                [0.0, 1.0, 0.0, canon(y)],
                [0.0, 0.0, 1.0, canon(z)],
            ],
        }
    }

    /// The scale by `sx`, `sy`, `sz` along the axes, about the origin.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if a factor is NaN or infinite.
    pub fn scale(sx: f64, sy: f64, sz: f64) -> Result<Transform, NumError> {
        Transform::from_rows([
            [sx, 0.0, 0.0, 0.0],
            [0.0, sy, 0.0, 0.0],
            [0.0, 0.0, sz, 0.0],
        ])
    }

    /// The uniform scale by `s` about the origin.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if `s` is NaN or infinite.
    pub fn uniform_scale(s: f64) -> Result<Transform, NumError> {
        Transform::scale(s, s, s)
    }

    /// The rotation about `axis` through the origin by `half_turns` half-turns (π·`half_turns`
    /// radians), counter-clockwise looking from the positive end of the axis (the right-hand rule).
    /// Sine and cosine come from [`geom_num::math::sinpi`] and [`geom_num::math::cospi`], so a
    /// multiple of a quarter turn gives entries of exactly 0 and ±1.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if `half_turns` is NaN or infinite (the maths functions refuse
    /// nothing else).
    pub fn rotation(axis: Axis, half_turns: f64) -> Result<Transform, NumError> {
        if !half_turns.is_finite() {
            return Err(NumError::NotFinite);
        }
        let c = math::cospi(half_turns)?.get();
        let s = math::sinpi(half_turns)?.get();
        Transform::rotation_from(axis, c, s)
    }

    /// The rotation about `axis` whose cosine and sine entries are `c` and `s` (not checked to be
    /// a rotation: classification says what the result really is).
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if `c` or `s` is NaN or infinite.
    pub fn rotation_from(axis: Axis, c: f64, s: f64) -> Result<Transform, NumError> {
        let rows = match axis {
            Axis::X => [[1.0, 0.0, 0.0, 0.0], [0.0, c, -s, 0.0], [0.0, s, c, 0.0]],
            Axis::Y => [[c, 0.0, s, 0.0], [0.0, 1.0, 0.0, 0.0], [-s, 0.0, c, 0.0]],
            Axis::Z => [[c, -s, 0.0, 0.0], [s, c, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]],
        };
        Transform::from_rows(rows)
    }

    /// The mirror that negates the coordinate along `axis` (reflection in the plane through the
    /// origin perpendicular to it).
    #[must_use]
    pub fn mirror(axis: Axis) -> Transform {
        let d = |a: Axis| if a == axis { -1.0 } else { 1.0 };
        Transform {
            m: [
                [d(Axis::X), 0.0, 0.0, 0.0],
                [0.0, d(Axis::Y), 0.0, 0.0],
                [0.0, 0.0, d(Axis::Z), 0.0],
            ],
        }
    }

    /// The same transform as a grid transform, exactly: entries 0 and ±1, and a translation below
    /// 2^42 in magnitude, which `f64` holds exactly.
    #[must_use]
    pub fn from_grid(g: &GridTransform) -> Transform {
        let [t0, t1, t2] = g.translation_part();
        let [r0, r1, r2] = g.matrix();
        #[allow(
            clippy::cast_precision_loss,
            reason = "a grid translation is at most 2^41 − 2 < 2^53 in magnitude: exact in f64"
        )]
        let row = |r: [i8; 3], t: i64| {
            let [a, b, c] = r;
            [f64::from(a), f64::from(b), f64::from(c), t as f64]
        };
        Transform {
            m: [row(r0, t0), row(r1, t1), row(r2, t2)],
        }
    }

    /// The image of a point: each coordinate evaluated as `((l₀x + l₁y) + l₂z) + t`.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if a coordinate overflows.
    pub fn apply_point(&self, p: F3) -> Result<F3, NumError> {
        let [x, y, z] = p.to_array();
        let [r0, r1, r2] = self.m.map(|[a, b, c, t]| a * x + b * y + c * z + t);
        F3::new(r0, r1, r2)
    }

    /// The image of a vector: the linear part only, each coordinate evaluated as
    /// `(l₀x + l₁y) + l₂z`.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if a coordinate overflows.
    pub fn apply_vector(&self, v: F3) -> Result<F3, NumError> {
        let [x, y, z] = v.to_array();
        let [r0, r1, r2] = self.m.map(|[a, b, c, _]| a * x + b * y + c * z);
        F3::new(r0, r1, r2)
    }

    /// The image of a grid point under the exact entries, rounded once to the nearest grid point,
    /// ties to even, with whether it was already on the grid (chapter 7: "other rotations round to
    /// the grid and the result says so").
    ///
    /// Each coordinate l₀x + l₁y + l₂z + t is first evaluated as a double-double from the exact
    /// products (`two_prod`, the coordinates being integers below 2^40); three additions lose at
    /// most (2·B_a + B_f)(|l₀x| + |l₁y| + |l₂z| + |t|) < 15.01u²·Σ (bounds from [`crate::dd`]), far
    /// below the filter 2^−98·Σ. A value farther than the filter from every whole step and every
    /// midpoint rounds like the exact value. Otherwise, and whenever a product is outside
    /// `two_prod`'s range, the exact value is formed as an integer: every entry is m·2^e exactly,
    /// so the coordinate is Σ mᵢxᵢ2^(eᵢ) with |mᵢxᵢ| < 2^93, summed in a wide integer after shifting
    /// to the smallest exponent, or to 2^0 if that is smaller (at most 2097 bits of shift, from an
    /// entry 2^1023 down to a subnormal's 2^−1074; each shifted term is at most
    /// |entry|·|xᵢ|·2^1074 < 2^1024·2^40·2^1074 = 2^2138, so the sum of four is below 2^2140 in
    /// `Wide<34>`; `i128`, `Wide<4>` or `Wide<10>` is used when the exponents are close), then
    /// rounded half to even exactly.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if a rounded coordinate lies outside the frame's ±(2^40 − 1)
    /// range, carrying that rounded coordinate exactly, or saturated with its sign when it is
    /// beyond `i128`, as the numbers layer documents the variant. [`NumError::BudgetExceeded`] only
    /// if a checked step of the exact stage failed, which its bounds rule out.
    pub fn apply_grid(&self, p: P3) -> Result<Rounded<P3>, NumError> {
        let v = [p.x.get(), p.y.get(), p.z.get()];
        let [r0, r1, r2] = self.m.map(|row| round_row(row, v));
        let (x, y, z) = (r0?, r1?, r2?);
        Ok(Rounded::new(
            P3 {
                x: x.value(),
                y: y.value(),
                z: z.value(),
            },
            x.is_exact() && y.is_exact() && z.is_exact(),
        ))
    }

    /// The composition `self ∘ first`: the transform that applies `first`, then `self`. Linear part
    /// L_self · L_first, translation L_self · t_first + t_self.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if an entry overflows.
    pub fn compose(&self, first: &Transform) -> Result<Transform, NumError> {
        let [[a, b, c, _], [d, e, f, _], [g, h, k, _]] = first.m;
        let [[_, _, _, tx], [_, _, _, ty], [_, _, _, tz]] = first.m;
        let rows = self.m.map(|[p, q, r, t]| {
            [
                p * a + q * d + r * g,
                p * b + q * e + r * h,
                p * c + q * f + r * k,
                p * tx + q * ty + r * tz + t,
            ]
        });
        Transform::from_rows(rows)
    }

    /// For each row i of L, the scaling 2^uᵢ that brings its largest entry into [1, 2): uᵢ = −kᵢ,
    /// kᵢ the exponent of that entry, from −1074 to 1023, so uᵢ fits `i32`. `Ok(None)` if a row is
    /// zero (then L is certainly singular). The entries are finite (the type's invariant), so
    /// `math::abs` never refuses one; its error, or a failed negation, would be returned.
    fn row_unscalings(&self) -> Result<Option<[i32; 3]>, NumError> {
        let mut u = [0i32; 3];
        for (slot, [a, b, c, _]) in u.iter_mut().zip(self.m) {
            let m = max2(
                max2(math::abs(a)?.get(), math::abs(b)?.get()),
                math::abs(c)?.get(),
            );
            if m == 0.0 {
                return Ok(None);
            }
            *slot = budget(exponent(m).and_then(i32::checked_neg))?;
        }
        Ok(Some(u))
    }

    /// L with each row i scaled by 2^uᵢ, entry by entry enclosed; `Ok(None)` if an entry
    /// overflows, which certifies nothing.
    fn rows_scaled(&self, u: [i32; 3]) -> Result<Option<Enclosed>, NumError> {
        let row = |r: [f64; 4], u: i32| -> Result<Option<[Interval; 3]>, NumError> {
            let [a, b, c, _] = r;
            let (a, b, c) = (
                scaled_point(a, u)?,
                scaled_point(b, u)?,
                scaled_point(c, u)?,
            );
            Ok(match (a, b, c) {
                (Some(a), Some(b), Some(c)) => Some([a, b, c]),
                _ => None,
            })
        };
        let [r0, r1, r2] = self.m;
        let [u0, u1, u2] = u;
        let (x, y, z) = (row(r0, u0)?, row(r1, u1)?, row(r2, u2)?);
        Ok(match (x, y, z) {
            (Some(x), Some(y), Some(z)) => Some([x, y, z]),
            _ => None,
        })
    }

    /// The certain sign of the determinant, from the row-scaled enclosure; `Some(Zero)` for a zero
    /// row, `None` when the enclosure holds zero and other values (or a bound overflows).
    fn det_sign(&self) -> Result<Option<Sign>, NumError> {
        let Some(u) = self.row_unscalings()? else {
            return Ok(Some(Sign::Zero));
        };
        let Some(scaled) = self.rows_scaled(u)? else {
            return Ok(None);
        };
        Ok(det_of(&scaled).and_then(Interval::certain_sign))
    }

    /// Whether every length is certainly kept within a relative ε (the `Rigid` test on G = LᵀL).
    fn is_rigid(&self) -> bool {
        let point = |x: f64| Interval::point(x).ok();
        let row = |r: [f64; 4]| -> Option<[Interval; 3]> {
            let [a, b, c, _] = r;
            Some([point(a)?, point(b)?, point(c)?])
        };
        let [r0, r1, r2] = self.m;
        let enclosed =
            || -> Option<bool> { near_identity(gram_of(&[row(r0)?, row(r1)?, row(r2)?])?) };
        enclosed() == Some(true)
    }

    /// Whether every length is certainly multiplied by s = |first column| within a relative ε (the
    /// `UniformScale` test on G/G₀₀), on L scaled by the power of two that brings its largest
    /// entry into [1, 2): the smallest of the rows' scalings, since uᵢ = −kᵢ.
    fn is_uniform(&self) -> Result<bool, NumError> {
        let Some([u0, u1, u2]) = self.row_unscalings()? else {
            return Ok(false);
        };
        let u = u0.min(u1).min(u2);
        let Some(scaled) = self.rows_scaled([u, u, u])? else {
            return Ok(false);
        };
        let test = || -> Option<bool> {
            let [g00, g11, g22, g01, g02, g12] = gram_of(&scaled)?;
            let ratio = |x: Interval| x.div(g00).ok();
            near_identity([
                ratio(g00)?,
                ratio(g11)?,
                ratio(g22)?,
                ratio(g01)?,
                ratio(g02)?,
                ratio(g12)?,
            ])
        };
        Ok(test() == Some(true))
    }

    /// The certified classification of this transform (see the module documentation).
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only if a checked step on the rows' exponents failed, which
    /// their range rules out (and a maths function's error, which finite entries rule out): for
    /// every transform the result is `Ok`.
    pub fn classify(&self) -> Result<Classification, NumError> {
        let [[a, b, c, tx], [d, e, f, ty], [g, h, k, tz]] = self.m;
        let identity_linear = a == 1.0
            && e == 1.0
            && k == 1.0
            && b == 0.0
            && c == 0.0
            && d == 0.0
            && f == 0.0
            && g == 0.0
            && h == 0.0;
        if identity_linear {
            let kind = if tx == 0.0 && ty == 0.0 && tz == 0.0 {
                TransformKind::Identity
            } else {
                TransformKind::Translation
            };
            return Ok(Classification {
                kind,
                mirror: false,
            });
        }
        let degenerate = Classification {
            kind: TransformKind::Degenerate,
            mirror: false,
        };
        let mirror = match self.det_sign()? {
            Some(Sign::Positive) => false,
            Some(Sign::Negative) => true,
            Some(Sign::Zero) | None => return Ok(degenerate),
        };
        let kind = if self.is_rigid() {
            TransformKind::Rigid
        } else if self.is_uniform()? {
            TransformKind::UniformScale
        } else {
            TransformKind::NonUniform
        };
        Ok(Classification { kind, mirror })
    }

    /// The inverse transform: linear part L⁻¹, translation −L⁻¹·t.
    ///
    /// L is first scaled row by row, L = D·L′ with D = diag(2^kᵢ) as in the classification, so no
    /// product overflows or underflows needlessly; L′⁻¹ = adj(L′)/det(L′), and L⁻¹ = L′⁻¹·D⁻¹
    /// (column i scaled by 2^(−kᵢ), exactly unless the entry falls below 2^−1022). The determinant
    /// of L′ is evaluated in the same order as its certified enclosure in
    /// [`Transform::classify`], from entries inside that enclosure, so it lies inside it and is
    /// nonzero whenever the transform is not `Degenerate`. For matrices whose products stay in the
    /// normal range this gives the same bits as the unscaled adjugate formula, because scaling by
    /// powers of two commutes with every rounding there.
    ///
    /// # Errors
    ///
    /// [`MathError::Singular`] if the transform is `Degenerate`; [`MathError::Num`] with
    /// [`NumError::NotFinite`] if an entry of the inverse overflows, or with
    /// [`NumError::BudgetExceeded`] only if a checked step on the rows' exponents failed, which
    /// their range rules out.
    pub fn inverse(&self) -> Result<Transform, MathError> {
        if self.classify()?.kind == TransformKind::Degenerate {
            return Err(MathError::Singular);
        }
        let [u0, u1, u2] = self.row_unscalings()?.ok_or(MathError::Singular)?;
        let s = |x: f64, u: i32| budget(scale_b(x, u));
        let [[a, b, c, tx], [d, e, f, ty], [g, h, k, tz]] = self.m;
        let (a, b, c) = (s(a, u0)?, s(b, u0)?, s(c, u0)?);
        let (d, e, f) = (s(d, u1)?, s(e, u1)?, s(f, u1)?);
        let (g, h, k) = (s(g, u2)?, s(h, u2)?, s(k, u2)?);
        let m1 = e * k - f * h;
        let m2 = d * k - f * g;
        let m3 = d * h - e * g;
        let det = a * m1 - b * m2 + c * m3;
        if det == 0.0 {
            return Err(MathError::Singular);
        }
        let unscale = |[p, q, r]: [f64; 3]| -> Result<[f64; 3], NumError> {
            Ok([s(p, u0)?, s(q, u1)?, s(r, u2)?])
        };
        let l = [
            unscale([m1 / det, (c * h - b * k) / det, (b * f - c * e) / det])?,
            unscale([-m2 / det, (a * k - c * g) / det, (c * d - a * f) / det])?,
            unscale([m3 / det, (b * g - a * h) / det, (a * e - b * d) / det])?,
        ];
        let rows = l.map(|[p, q, r]| [p, q, r, -(p * tx + q * ty + r * tz)]);
        Ok(Transform::from_rows(rows)?)
    }
}

impl Default for Transform {
    /// The identity.
    fn default() -> Self {
        Transform::IDENTITY
    }
}

/// One coordinate of [`Transform::apply_grid`]: l₀x + l₁y + l₂z + t rounded to the nearest step,
/// ties to even, with whether it was exact. Three stages, each used only when the cheaper one
/// cannot decide: plain doubles, double-doubles, exact integers.
fn round_row(row: [f64; 4], v: [i64; 3]) -> Result<Rounded<Coord>, NumError> {
    if let Some(r) = float_row(row, v) {
        return r;
    }
    match filtered_row(row, v) {
        Some(r) => r,
        None => exact_row(row, v),
    }
}

/// The floating-point filter of [`round_row`]: `None` when it cannot decide.
///
/// v = ((l₀x + l₁y) + l₂z) + t in `f64`. Each product and each sum rounds once, by at most u times
/// its magnitude, or by at most 2^−1075 if it falls below 2^−1022, and every partial sum is at most
/// (1 + u)³·Σ in magnitude, with Σ = |l₀x| + |l₁y| + |l₂z| + |t|. So the computed value is within
/// ((1 + u)⁴ − 1)·Σ + 7·2^−1075 < 4.01u·Σ + 2^−1071 of the exact one. The filter is
/// 2^−50·Σ_f + 2^−1070 (2^−50 = 8u), Σ_f being Σ of the rounded products and |t| summed in `f64`,
/// which is at least (1 − u)⁷·Σ, so the filter exceeds the error. v − rint(v) is exact (see
/// [`DD::round_shifted`]).
///
/// A product that overflows makes the filter decline, as an overflowing sum does. For finite
/// values the maths functions refuse nothing; were one to, the filter declines too, and the exact
/// stage decides: a decline never changes the result. A value at or beyond 2^41 in magnitude is out
/// of range, and the filter declines it too: the error carries the exact rounded coordinate, which
/// the exact stage finds.
fn float_row(row: [f64; 4], v: [i64; 3]) -> Option<Result<Rounded<Coord>, NumError>> {
    let [l0, l1, l2, t] = row;
    #[allow(
        clippy::cast_precision_loss,
        reason = "a grid coordinate is below 2^40 < 2^53 in magnitude: exact in f64"
    )]
    let [x, y, z] = v.map(|c| c as f64);
    let (a, b, c) = (l0 * x, l1 * y, l2 * z);
    if !a.is_finite() || !b.is_finite() || !c.is_finite() {
        return None;
    }
    let s = a + b + c + t;
    let size = |w: f64| math::abs(w).ok().map(geom_num::Real::get);
    let sigma = size(a)? + size(b)? + size(c)? + size(t)?;
    if !s.is_finite() || !sigma.is_finite() {
        return None;
    }
    let err = sigma * FLOAT_FACTOR + FLOAT_FLOOR;
    if s >= RANGE_LIMIT || s <= -RANGE_LIMIT {
        // Out of range (certainly so when the filter is below 1), but the error carries the exact
        // rounded coordinate, which only the exact stage knows: it reports it.
        return None;
    }
    let r = math::round_ties_even(s).ok()?.get();
    let frac = size(s - r)?;
    if frac > err && frac < 0.5 - err {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "r is a whole number below 2^41 in magnitude: exact in i64"
        )]
        let whole = r as i64;
        return Some(Coord::new(whole).map(|value| Rounded::new(value, false)));
    }
    None
}

/// The double-double filter of [`round_row`]: `None` when it cannot decide.
///
/// The products are exact (`exact_product`). The three additions err by at most B_a|p₀ + p₁|,
/// B_a|s₁ + p₂| and B_f|s₂ + t| (B_a < 6u² + 45u³, B_f < 3u² + 10u³), so the double-double value
/// lies within (2B_a + B_f)(1 + 13u²)·Σ < 15.01u²·Σ of the exact value, with Σ = |p₀| + |p₁| +
/// |p₂| + |t|. The filter `err` is 2^−98 times Σ computed from the leading parts, which is at least
/// 255u²·Σ even after the rounding of that sum, and `frac` (the distance of the double-double
/// value from its nearest integer x, one rounding) is compared with a further 2^−52 of slack.
///
/// The parts are finite, so the maths functions and the exact rounding refuse nothing here; were
/// one to, the filter declines and the exact stage decides. A value at or beyond 2^41 in magnitude
/// is declined too, so that the exact stage reports its exact rounded coordinate.
fn filtered_row(row: [f64; 4], v: [i64; 3]) -> Option<Result<Rounded<Coord>, NumError>> {
    let [l0, l1, l2, t] = row;
    #[allow(
        clippy::cast_precision_loss,
        reason = "a grid coordinate is below 2^40 < 2^53 in magnitude: exact in f64"
    )]
    let [x, y, z] = v.map(|c| c as f64);
    let p0 = DD::exact_product(l0, x)?;
    let p1 = DD::exact_product(l1, y)?;
    let p2 = DD::exact_product(l2, z)?;
    let s = p0.add(p1).ok()?.add(p2).ok()?.add_f64(t).ok()?;
    let size = |w: f64| math::abs(w).ok().map(geom_num::Real::get);
    let sigma = size(p0.hi())? + size(p1.hi())? + size(p2.hi())? + size(t)?;
    let err = sigma * FILTER_FACTOR + FILTERED_SLACK;
    if s.hi() >= RANGE_LIMIT || s.hi() <= -RANGE_LIMIT {
        // Out of range (certainly so when the filter is below 1), but the error carries the exact
        // rounded coordinate, which only the exact stage knows: it reports it.
        return None;
    }
    let cand = s.round_shifted(0)?.value();
    // |cand| ≤ 2^41, so it is a double and hi − cand is exact.
    #[allow(
        clippy::cast_precision_loss,
        reason = "|cand| ≤ 2^41 < 2^53: exact in f64"
    )]
    let cand_f = cand as f64;
    let frac = size((s.hi() - cand_f) + s.lo())?;
    if frac > err && frac < 0.5 - err {
        return Some(Coord::from_i128(cand).map(|value| Rounded::new(value, false)));
    }
    None
}

/// The exact rounding of [`round_row`]: the coordinate as Σ mᵢcᵢ·2^(eᵢ), shifted to a common
/// exponent in a wide integer and rounded half to even.
///
/// Every integer step is checked: |mᵢ| < 2^53 and |cᵢ| < 2^40 make each product below 2^93, and
/// the exponents (from −1074 to 971) make every shift from 0 to 2097 and the final division by at
/// most 2^1074. A failed step is reported as [`NumError::BudgetExceeded`]. A rounded coordinate
/// outside the frame is reported as [`NumError::OutOfRange`] with that coordinate, exactly, or
/// saturated with its sign when it is beyond `i128` ([`round_terms`]).
fn exact_row(row: [f64; 4], v: [i64; 3]) -> Result<Rounded<Coord>, NumError> {
    let [l0, l1, l2, t] = row;
    let [x, y, z] = v;
    // The entries are finite (the type's invariant), so each is a dyadic number.
    let term = |l: f64, c: i64| -> Result<(i128, i32), NumError> {
        let (m, e) = budget(dyadic(l))?;
        Ok((budget(i128::from(m).checked_mul(i128::from(c)))?, e))
    };
    let terms = [term(l0, x)?, term(l1, y)?, term(l2, z)?, term(t, 1)?];
    let mut lo_e = i32::MAX;
    let mut hi_e = i32::MIN;
    for &(m, e) in &terms {
        if m != 0 {
            lo_e = lo_e.min(e);
            hi_e = hi_e.max(e);
        }
    }
    if lo_e == i32::MAX {
        return Ok(Rounded::new(Coord::ZERO, true));
    }
    let base = lo_e.min(0);
    let span = budget(hi_e.checked_sub(base))?;
    // Each nonzero term's shift to the common exponent `base`, e − base from 0 to span, and the
    // final division by 2^(−base); a zero term adds nothing and needs no shift.
    let mut shifted = [(0i128, 0u32); 4];
    for (slot, &(m, e)) in shifted.iter_mut().zip(&terms) {
        if m != 0 {
            let shift = budget(e.checked_sub(base).and_then(|s| u32::try_from(s).ok()))?;
            *slot = (m, shift);
        }
    }
    let k = budget(base.checked_neg().and_then(|k| u32::try_from(k).ok()))?;
    // |mᵢcᵢ| < 2^93 and four terms: the sum is below 2^(span + 95), which bounds the narrower
    // widths. For the widest, span can reach 1023 + 1074 = 2097, but each term is also
    // |entry|·|cᵢ|·2^(−base) < 2^1024·2^40·2^1074 = 2^2138, so the sum is below 2^2140, inside
    // `Wide<34>`'s ±2^2175.
    let r = if span <= 31 {
        budget(round_terms_i128(&shifted, k))?
    } else if span <= 160 {
        round_terms::<4>(&shifted, k)?
    } else if span <= 470 {
        round_terms::<10>(&shifted, k)?
    } else {
        round_terms::<34>(&shifted, k)?
    };
    // The rounded coordinate, exactly: `OutOfRange` carries it when it leaves the frame.
    Ok(Rounded::new(Coord::from_i128(r.value())?, r.is_exact()))
}

/// [`round_terms`] in `i128`, for exponents within 31 of each other: each term m shifted left by
/// at most 31 places is below 2^93·2^31 = 2^124 and the sum of four below 2^126, so no step fails.
/// Divided by 2^k: a shift of 127 or more leaves a value below 1/2 in magnitude, which rounds to
/// zero; otherwise 1 ≤ k ≤ 126 and every shift below is under 128. `None` only if a checked step
/// failed, which these bounds rule out; the caller reports it as [`NumError::BudgetExceeded`].
fn round_terms_i128(terms: &[(i128, u32); 4], k: u32) -> Option<Rounded<i128>> {
    let mut sum = 0i128;
    for &(m, shift) in terms {
        if m != 0 {
            sum = sum.checked_add(m.checked_shl(shift)?)?;
        }
    }
    if k == 0 {
        return Some(Rounded::new(sum, true));
    }
    if k >= 127 {
        return Some(Rounded::new(0, sum == 0));
    }
    let q = sum >> k;
    let rem = sum.checked_sub(q << k)?;
    // 2^(k − 1), by shifts alone.
    let half = (1i128 << k) >> 1;
    let value = if rem > half || (rem == half && q & 1 == 1) {
        q.checked_add(1)?
    } else {
        q
    };
    Some(Rounded::new(value, rem == 0))
}

/// Σ m·2^shift, exactly, divided by 2^k and rounded half to even.
///
/// # Errors
///
/// A rounded value beyond `i128` is far outside every frame and is reported as the numbers layer's
/// [`NumError::OutOfRange`] documents a coordinate beyond `i128`: saturated, with its sign
/// (`i128::MAX` for 2^127 − 1 or more, `i128::MIN` for −2^127 or less; [`round_scaled`] answers
/// for every value that fits, and the sum's sign is the rounded value's). A shifted term or a sum
/// that does not fit `Wide<L>` is ruled out by the widths [`exact_row`] chooses, and is reported
/// as [`NumError::BudgetExceeded`].
fn round_terms<const L: usize>(
    terms: &[(i128, u32); 4],
    k: u32,
) -> Result<Rounded<i128>, NumError> {
    let mut sum = Wide::<L>::ZERO;
    for &(m, shift) in terms {
        if m != 0 {
            sum = budget(shl_into::<L>(m, shift).and_then(|term| sum.checked_add(&term)))?;
        }
    }
    let saturated = if sum.is_negative() {
        i128::MIN
    } else {
        i128::MAX
    };
    round_scaled(&sum, k).ok_or(NumError::OutOfRange { value: saturated })
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "tests may use native arithmetic and casts; the rules are for kernel code"
)]
mod tests {
    use super::*;
    use geom_num::grid::COORD_MAX;

    #[test]
    fn exact_rows_round_half_to_even() {
        let t = Transform::from_rows([[0.5, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.25], [0.0; 4]]);
        let t = t.unwrap();
        let r = t.apply_grid(P3::new(3, 7, 0).unwrap()).unwrap();
        // 1.5 → 2 (even), 7.25 → 7, 0 exact.
        let p = r.value();
        assert_eq!((p.x.get(), p.y.get(), p.z.get()), (2, 7, 0));
        assert!(!r.is_exact());
        let r = t.apply_grid(P3::new(5, 7, 0).unwrap()).unwrap();
        assert_eq!(r.value().x.get(), 2);
        let near = Transform::from_rows([[1.0, 0.0, 0.0, 0.0]; 3]).unwrap();
        let edge = near.apply_grid(P3::new(COORD_MAX, 0, 0).unwrap()).unwrap();
        assert!(edge.is_exact() && edge.value().x.get() == COORD_MAX);
    }

    #[test]
    fn huge_cancelling_entries_round_exactly() {
        let big = crate::exact::pow2(900).unwrap();
        let t = Transform::from_rows([[big, -big, 0.5, 0.0], [0.0; 4], [0.0; 4]]).unwrap();
        let r = t.apply_grid(P3::new(12_345, 12_345, 3).unwrap()).unwrap();
        assert_eq!(r.value().x.get(), 2);
        assert!(!r.is_exact());
        let tiny = f64::from_bits(1);
        let t = Transform::from_rows([[tiny, 0.0, 0.0, 2.5], [0.0; 4], [0.0; 4]]).unwrap();
        let r = t.apply_grid(P3::new(1, 0, 0).unwrap()).unwrap();
        // 2.5 + 2^−1074 is just above the midpoint: rounds up to 3.
        assert_eq!(r.value().x.get(), 3);
    }
}
