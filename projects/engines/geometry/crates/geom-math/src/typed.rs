//! The typed transform: one enum for every transform the kernel applies to geometry, so that
//! whether a transform is exact is part of its type (chapter 7: "Transforms are typed so that this
//! is explicit").
//!
//! | Variant | Holds | A grid point's image |
//! | --- | --- | --- |
//! | `Exact` | [`GridTransform`]: whole steps, quarter turns, mirrors | exact |
//! | `Rational` | [`RationalTransform`]: Pythagorean turns, rational scales | rounded once |
//! | `Rotation` | [`RationalRotation`]: a rational rotation with \|q\|² above 2^40 | rounded once |
//! | `Certified` | [`Transform`] of doubles: any other affine map | rounded once |
//!
//! "Rounded once" means the exact image (under the rational entries, or under the exact values of
//! the doubles) rounded to the nearest step, ties to even, with whether it was already exact.
//!
//! Every [`TypedTransform::apply`] returns a [`Rounded`] point: chapter 7's "other rotations round
//! to the grid and the result says so". Composition keeps the most exact variant both operands
//! allow: exact with exact stays exact, exact or rational with rational stays rational (and is
//! simplified back to exact when it is a grid transform, and refused when it outgrows the rational
//! budget), rotations compose as rotations, exactly, while the product fits the rotation budget
//! (decision 32.14), two rational rotations included where their product outgrows the rational
//! budget, and anything else, a certified transform or a rotation that outgrows its budget or
//! meets a translation, scale or mirror, becomes certified, rounding the exact entries to doubles:
//! a change of variant the result records. A rotation re-approximated within an angle instead is [`RationalRotation::compose`]. Classification is exact for the exact and rational variants and certified for the
//! certified one (see [`crate::transform`]).
//!
//! Guarantees: every [`TypedTransform::apply`] returns a [`Rounded`] point, the exact image rounded
//! once to the nearest step, ties to even, with whether it was already exact; composition keeps
//! the most exact variant both operands allow and records a change of variant; classification is
//! exact for the exact and rational variants and certified for the certified one.
//!
//! Method: the hierarchy of transformation groups (isometries, similarities, affinities) of
//! R. Hartley and A. Zisserman, *Multiple View Geometry in Computer Vision*, 2nd ed., Cambridge
//! University Press (2004), section 2.4, <https://doi.org/10.1017/CBO9780511811685>, with each
//! group represented in the most exact number type that holds it.
//!
//! Cost: constant time, no allocation: that of the variant's own operation (see
//! [`crate::grid_transform`], [`crate::rational`] and [`crate::transform`]), plus, when a result is
//! simplified, a comparison of each of the twelve entries (with 0 and ±1, and the translations
//! with their range), and for a certified transform three roundings to integers and a reduction to
//! canonical form. [`TypedTransform::rotation`] for an irrational direction adds one double-double
//! square root and two double-double divisions.

#![allow(
    clippy::many_single_char_names,
    reason = "variable names follow the formulas of the method each function cites"
)]

use geom_num::grid::{P2, P3};
use geom_num::{NumError, math};

use crate::dd::DD;
use crate::error::{MathError, budget};
use crate::exact::const_pow2;
use crate::frame::Rotation;
use crate::grid_transform::{Axis, GridTransform};
use crate::rational::RationalTransform;
use crate::rotation::RationalRotation;
use crate::rounded::Rounded;
use crate::transform::{Classification, Transform, TransformKind};

/// 2^84, the largest translation a certified transform may have to be read as whole steps (the
/// rational transforms' bound).
const TWO_POW_84: f64 = const_pow2!(84);

/// A transform whose type says how exact it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TypedTransform {
    /// An exact grid transform: every grid point maps to a grid point.
    Exact(GridTransform),
    /// An exact rational transform: images are rounded once and say whether they were exact.
    Rational(RationalTransform),
    /// An exact rational rotation too large for a [`RationalTransform`] (|q|² above 2^40, decision
    /// 32.14): images are rounded once and say whether they were exact.
    Rotation(RationalRotation),
    /// A floating-point transform, exact in its own `f64` entries, which approximate the intended
    /// one: images are rounded once and say whether they were exact.
    Certified(Transform),
}

impl From<GridTransform> for TypedTransform {
    fn from(g: GridTransform) -> Self {
        TypedTransform::Exact(g)
    }
}

impl From<RationalTransform> for TypedTransform {
    fn from(r: RationalTransform) -> Self {
        TypedTransform::Rational(r).simplified()
    }
}

impl From<RationalRotation> for TypedTransform {
    fn from(r: RationalRotation) -> Self {
        TypedTransform::Rotation(r).simplified()
    }
}

impl From<Transform> for TypedTransform {
    fn from(t: Transform) -> Self {
        TypedTransform::Certified(t).simplified()
    }
}

/// The certified transform's exact grid form, when its entries are 0 and ±1 forming a signed
/// permutation and its translation is whole steps within the grid transforms' range.
///
/// The entries are finite (the type's invariant), so `math::round_ties_even` refuses none; were it
/// to, the transform is not read as a grid transform and stays certified, which is the same
/// transform, exact in its own entries.
fn certified_as_grid(t: &Transform) -> Option<GridTransform> {
    let rows = t.rows();
    let mut m = [[0i64; 3]; 3];
    let mut tr = [0i128; 3];
    for ((out, slot), row) in m.iter_mut().zip(tr.iter_mut()).zip(rows) {
        let [a, b, c, s] = row;
        for (o, v) in out.iter_mut().zip([a, b, c]) {
            *o = if v == 0.0 {
                0
            } else if v == 1.0 {
                1
            } else if v == -1.0 {
                -1
            } else {
                return None;
            };
        }
        if s != math::round_ties_even(s).ok()?.get() || s > TWO_POW_84 || s < -TWO_POW_84 {
            return None;
        }
        #[allow(
            clippy::cast_possible_truncation,
            reason = "s is a whole number of at most 2^84 in magnitude: exact in i128"
        )]
        let whole = s as i128;
        *slot = whole;
    }
    RationalTransform::new(m, tr, 1).ok()?.to_grid()
}

impl TypedTransform {
    /// The identity.
    pub const IDENTITY: TypedTransform = TypedTransform::Exact(GridTransform::IDENTITY);

    /// The same transform in the most exact variant that holds it: a rotation with |q|² ≤ 2^40
    /// becomes rational, a rational transform that is a grid transform becomes exact, and so does
    /// a certified transform whose entries are exactly 0 and ±1 in a signed permutation with a
    /// whole-step translation.
    #[must_use]
    pub fn simplified(self) -> TypedTransform {
        match self {
            // A rotation's rational form, then that form's own simplification (below), in place:
            // no recursive call.
            TypedTransform::Rotation(r) => r.to_rational().map_or(self, |t| {
                t.to_grid()
                    .map_or(TypedTransform::Rational(t), TypedTransform::Exact)
            }),
            TypedTransform::Rational(r) => r.to_grid().map_or(self, TypedTransform::Exact),
            TypedTransform::Certified(t) => {
                certified_as_grid(&t).map_or(self, TypedTransform::Exact)
            }
            TypedTransform::Exact(_) => self,
        }
    }

    /// The rotation about `axis` through the origin along the integer direction `rotation`
    /// (cosine and sine c/r and s/r, r = √(c² + s²)), in the most exact variant: a quarter turn is
    /// exact, a Pythagorean direction (integer r) is rational, any other direction is certified,
    /// its entries c/r and s/r computed in double-double (within 2^−100, relatively) and rounded
    /// to the nearest double.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] only if a Pythagorean length exceeds the rational transforms'
    /// denominator bound, which a reduced 32-bit direction cannot do. [`NumError::BudgetExceeded`]
    /// only if a checked step failed, which the rotation's bounds rule out.
    pub fn rotation(axis: Axis, rotation: Rotation) -> Result<TypedTransform, NumError> {
        if let Some(turns) = rotation.quarter_turn_count() {
            return Ok(TypedTransform::Exact(GridTransform::quarter_turns(
                axis,
                i32::from(turns),
            )));
        }
        if rotation.exact_norm().is_some() {
            return Ok(TypedTransform::Rational(RationalTransform::rotation(
                axis, rotation,
            )?));
        }
        // c² + s² < 2^63 for every rotation (`Rotation::norm2`), so it fits i64; a failure is a
        // defect, reported, never passed off as a property of the input.
        let n2 = budget(i64::try_from(rotation.norm2()).ok())?;
        let r = DD::from_i64(n2).sqrt()?;
        let c = DD::from_i64(i64::from(rotation.cos())).div(r)?.to_f64();
        let s = DD::from_i64(i64::from(rotation.sin())).div(r)?.to_f64();
        Ok(TypedTransform::Certified(Transform::rotation_from(
            axis, c, s,
        )?))
    }

    /// Whether this is the exact variant.
    #[must_use]
    pub const fn is_exact(&self) -> bool {
        matches!(self, TypedTransform::Exact(_))
    }

    /// The floating-point form, for evaluating curves: exact for the exact variant, the nearest
    /// doubles for the rational and rotation ones, the transform itself for the certified one.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only as [`RationalTransform::to_transform`] and
    /// [`RationalRotation::to_transform`] report it, which cannot happen: for every typed
    /// transform the result is `Ok`.
    pub fn to_transform(&self) -> Result<Transform, NumError> {
        match self {
            TypedTransform::Exact(g) => Ok(Transform::from_grid(g)),
            TypedTransform::Rational(r) => r.to_transform(),
            TypedTransform::Rotation(r) => r.to_transform(),
            TypedTransform::Certified(t) => Ok(*t),
        }
    }

    /// The image of a grid point, rounded once to the grid, ties to even, with whether it was
    /// exact (always exact for the exact variant).
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if a coordinate of the image leaves the frame's ±(2^40 − 1) range.
    pub fn apply(&self, p: P3) -> Result<Rounded<P3>, NumError> {
        match self {
            TypedTransform::Exact(g) => Ok(Rounded::new(g.apply(p)?, true)),
            TypedTransform::Rational(r) => r.apply(p),
            TypedTransform::Rotation(r) => r.apply(p),
            TypedTransform::Certified(t) => t.apply_grid(p),
        }
    }

    /// Whether the transform acts in the plane, leaving every z unchanged, so it applies to a 2D
    /// frame's points ([`GridTransform::acts_in_plane`] and its rational and certified
    /// counterparts).
    #[must_use]
    pub fn acts_in_plane(&self) -> bool {
        match self {
            TypedTransform::Exact(g) => g.acts_in_plane(),
            TypedTransform::Rational(r) => r.acts_in_plane(),
            TypedTransform::Rotation(r) => r.acts_in_plane(),
            TypedTransform::Certified(t) => t.acts_in_plane(),
        }
    }

    /// The image of a 2D grid point: [`TypedTransform::apply`] at z = 0, rounded once to the grid,
    /// ties to even, with whether it was exact (always exact for the exact variant).
    ///
    /// # Errors
    ///
    /// [`MathError::NotInPlane`] if the transform does not act in the plane; otherwise as
    /// [`TypedTransform::apply`].
    pub fn apply_2d(&self, p: P2) -> Result<Rounded<P2>, MathError> {
        match self {
            TypedTransform::Exact(g) => Ok(Rounded::new(g.apply_2d(p)?, true)),
            TypedTransform::Rational(r) => r.apply_2d(p),
            TypedTransform::Rotation(r) => r.apply_2d(p),
            TypedTransform::Certified(t) => t.apply_grid_2d(p),
        }
    }

    /// The classification: exact for the exact, rational and rotation variants, certified for the
    /// certified one.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only as [`RationalTransform::classify`] and
    /// [`Transform::classify`] report it, which their bounds rule out: for every typed transform
    /// the result is `Ok`.
    pub fn classify(&self) -> Result<Classification, NumError> {
        match self {
            TypedTransform::Exact(g) => {
                let mirror = g.reverses_orientation();
                let linear_identity = g.source() == Axis::ALL && g.negate() == [false; 3];
                let kind = match (linear_identity, g.translation_part() == [0; 3]) {
                    (true, true) => TransformKind::Identity,
                    (true, false) => TransformKind::Translation,
                    (false, _) => TransformKind::Rigid,
                };
                Ok(Classification { kind, mirror })
            }
            TypedTransform::Rational(r) => r.classify(),
            TypedTransform::Rotation(r) => Ok(r.classify()),
            TypedTransform::Certified(t) => t.classify(),
        }
    }

    /// The composition `self ∘ first`, in the most exact variant both allow (see the module
    /// documentation).
    ///
    /// # Errors
    ///
    /// [`MathError::Num`] with [`NumError::OutOfRange`] if an exact or rational result exceeds its
    /// bounds, or [`NumError::NotFinite`] if a certified entry overflows.
    pub fn compose(&self, first: &TypedTransform) -> Result<TypedTransform, MathError> {
        use TypedTransform::{Certified, Exact, Rational, Rotation};
        let rational = |t: &TypedTransform| match t {
            Exact(g) => Some(RationalTransform::from_grid(g)),
            Rational(r) => Some(*r),
            Rotation(_) | Certified(_) => None,
        };
        let rotation = |t: &TypedTransform| match t {
            Rotation(r) => Some(*r),
            _ => rational(t).and_then(|r| RationalRotation::from_rational(&r)),
        };
        let certified = || -> Result<TypedTransform, MathError> {
            Ok(TypedTransform::from(
                self.to_transform()?.compose(&first.to_transform()?)?,
            ))
        };
        match (self, first) {
            (Exact(a), Exact(b)) => Ok(Exact(a.compose(b)?)),
            (Certified(_), _) | (_, Certified(_)) => certified(),
            (Rotation(_), _) | (_, Rotation(_)) => match (rotation(self), rotation(first)) {
                (Some(a), Some(b)) => match a.compose_exact(&b)? {
                    Some(r) => Ok(TypedTransform::from(r)),
                    None => certified(),
                },
                _ => certified(),
            },
            _ => match (rational(self), rational(first)) {
                (Some(a), Some(b)) => match a.compose(&b) {
                    Ok(r) => Ok(TypedTransform::from(r)),
                    // Two rotations past the rational budget compose as rotations (32.14);
                    // anything else past it is refused, as before.
                    Err(e @ NumError::OutOfRange { .. }) => {
                        match (rotation(self), rotation(first)) {
                            (Some(a), Some(b)) => match a.compose_exact(&b)? {
                                Some(r) => Ok(TypedTransform::from(r)),
                                None => certified(),
                            },
                            _ => Err(e.into()),
                        }
                    }
                    Err(e) => Err(e.into()),
                },
                // Unreachable: both are exact or rational here.
                _ => certified(),
            },
        }
    }

    /// The inverse, in the same variant, or in the exact one when the inverse of a rational or
    /// certified transform is a grid transform (as [`TypedTransform::simplified`]).
    ///
    /// # Errors
    ///
    /// [`MathError::Singular`] if the transform has no inverse (or, for the certified variant, its
    /// orientation cannot be certified); [`MathError::Num`] if the inverse exceeds its bounds.
    pub fn inverse(&self) -> Result<TypedTransform, MathError> {
        match self {
            TypedTransform::Exact(g) => Ok(TypedTransform::Exact(g.inverse()?)),
            TypedTransform::Rational(r) => Ok(TypedTransform::from(r.inverse()?)),
            TypedTransform::Rotation(r) => Ok(TypedTransform::from(r.inverse())),
            TypedTransform::Certified(t) => Ok(TypedTransform::from(t.inverse()?)),
        }
    }
}

impl Default for TypedTransform {
    /// The identity.
    fn default() -> Self {
        TypedTransform::IDENTITY
    }
}
