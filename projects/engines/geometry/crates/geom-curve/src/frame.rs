//! Orthonormal frames defined exactly by two stored vectors: where a kind starts and which way it
//! turns.
//!
//! What it does: a [`Frame`] stores a reference vector x and a normal n, any two finite vectors
//! that are not parallel. They define, exactly, the orthonormal frame
//! x̂ = ((n × x) × n) / |(n × x) × n|, ŷ = (n × x) / |n × x|, n̂ = n / |n|: x̂ is the unit vector
//! along the part of x across n, and ŷ = n̂ × x̂ completes a right-handed frame. Because the stored
//! doubles define the frame, rather than approximate one, a circle on it is exactly a circle of
//! exactly its radius, whatever rounding made x and n. A 2D frame has x in the plane and
//! n = (0, 0, k) with k ≠ 0: ŷ is x̂ turned a quarter turn anticlockwise when k > 0 and clockwise
//! when k < 0. [`Frame::reversed`] negates n, which negates ŷ and n̂ exactly and keeps x̂, as
//! reversing a kind needs.
//!
//! Guarantees:
//!
//! - **An exact definition, certified enclosures.** The unit vectors come in doubles and as
//!   enclosures of their exact values, exact points wherever the arithmetic is exact (an
//!   axis-aligned frame, or one whose lengths are whole, such as 3-4-5).
//! - **Refused when degenerate.** Parallel x and n (decided exactly), a 2D normal with x or y
//!   components, and entries that are not finite are errors.
//!
//! Method: the derived axes of an axis placement, as ISO 10303-42 builds them for
//! `axis2_placement_3d` (its `build_axes` function,
//! <https://downloads.steptools.com/docs/stp_aim/html/t_build_axes.html>: the axis normalised, the
//! reference direction's projection across it, then their cross product), with the projection taken
//! exactly as (n × x) × n so that no stored vector needs to be of unit length or perpendicular.
//!
//! Cost: two cross products and three norms; constant, nothing allocates.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': x and n the stored vectors, p and q their cross products, a and b factors"
)]

use geom_num::eft::two_prod;
use geom_num::exact_sum::ExactSum;
use geom_num::interval::Interval;
use geom_num::math;

use crate::error::CurveError;
use crate::tight::{self, point};

/// A right-handed orthonormal frame, stored exactly as a reference vector `x` and a `normal`
/// (module documentation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame<const D: usize> {
    x: [f64; D],
    normal: [f64; 3],
}

/// A frame's unit vectors x̂, ŷ and n̂: in doubles, and enclosures of the exact ones.
pub type UnitVectors = ([[f64; 3]; 3], [[Interval; 3]; 3]);

/// A frame's unit vectors x̂, ŷ and n̂, in doubles and as enclosures of the exact ones.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Unit {
    /// x̂, ŷ, n̂ in doubles.
    pub(crate) doubles: [[f64; 3]; 3],
    /// Enclosures of the exact x̂, ŷ, n̂.
    pub(crate) exact: [[Interval; 3]; 3],
}

/// `v` in 3D, zeros after its own coordinates.
pub(crate) fn lift<const D: usize>(v: &[f64; D]) -> [f64; 3] {
    let mut out = [0.0; 3];
    for (o, &x) in out.iter_mut().zip(v) {
        *o = x;
    }
    out
}

/// The first D coordinates of `v` (`zero` beyond 3, which no kind reaches).
pub(crate) fn lower<const D: usize, T: Copy>(v: &[T; 3], zero: T) -> [T; D] {
    let mut out = [zero; D];
    for (o, &x) in out.iter_mut().zip(v) {
        *o = x;
    }
    out
}

/// Point intervals at the coordinates of `v`.
pub(crate) fn points(v: [f64; 3]) -> Result<[Interval; 3], CurveError> {
    let [a, b, c] = v;
    Ok([point(a)?, point(b)?, point(c)?])
}

/// a × b in doubles.
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    let [a0, a1, a2] = a;
    let [b0, b1, b2] = b;
    [a1 * b2 - a2 * b1, a2 * b0 - a0 * b2, a0 * b1 - a1 * b0]
}

/// a × b enclosed.
fn cross_enclosed(a: [Interval; 3], b: [Interval; 3]) -> Result<[Interval; 3], CurveError> {
    let [a0, a1, a2] = a;
    let [b0, b1, b2] = b;
    Ok([
        tight::sub(tight::mul(a1, b2)?, tight::mul(a2, b1)?)?,
        tight::sub(tight::mul(a2, b0)?, tight::mul(a0, b2)?)?,
        tight::sub(tight::mul(a0, b1)?, tight::mul(a1, b0)?)?,
    ])
}

/// `v` / |v| in doubles.
fn unit(v: [f64; 3]) -> Result<[f64; 3], CurveError> {
    let [x, y, z] = v;
    let n = math::sqrt(x * x + y * y + z * z)?.get();
    if n == 0.0 {
        return Err(CurveError::Degenerate);
    }
    Ok([x / n, y / n, z / n])
}

/// `v` / |v| enclosed.
fn unit_enclosed(v: [Interval; 3]) -> Result<[Interval; 3], CurveError> {
    let n = tight::norm(&v)?;
    let [x, y, z] = v;
    Ok([tight::div(x, n)?, tight::div(y, n)?, tight::div(z, n)?])
}

/// Whether a × b is certainly not zero, decided exactly: each coordinate's two products summed
/// without rounding. A product whose error is not a double (below 2^−969) leaves it undecided,
/// which counts as zero.
#[must_use]
pub fn not_parallel(a: [f64; 3], b: [f64; 3]) -> bool {
    let [a0, a1, a2] = a;
    let [b0, b1, b2] = b;
    [(a1, b2, a2, b1), (a2, b0, a0, b2), (a0, b1, a1, b0)]
        .iter()
        .any(|&(p, q, r, s)| {
            let (x, ex) = two_prod(p, q);
            let (y, ey) = two_prod(r, s);
            if !(ex.is_finite() && ey.is_finite()) {
                return false;
            }
            let mut sum = ExactSum::new();
            sum.add_slice(&[x, ex, -y, -ey]);
            sum.round_ieee() != 0.0
        })
}

impl<const D: usize> Frame<D> {
    /// The frame with reference vector `x` and normal `normal` (module documentation).
    ///
    /// # Errors
    ///
    /// [`CurveError::Dimension`] unless D is 2 or 3; [`CurveError::NotFinite`]; in 2D,
    /// [`CurveError::NotInPlane`] if the normal has an x or y component; [`CurveError::Degenerate`]
    /// if `x` and `normal` are parallel (or either is zero).
    pub fn new(x: [f64; D], normal: [f64; 3]) -> Result<Frame<D>, CurveError> {
        if D != 2 && D != 3 {
            return Err(CurveError::Dimension { dimension: D });
        }
        if !x.iter().chain(&normal).all(|v| v.is_finite()) {
            return Err(CurveError::NotFinite);
        }
        let [n0, n1, _] = normal;
        if D == 2 && (n0 != 0.0 || n1 != 0.0) {
            return Err(CurveError::NotInPlane);
        }
        if !not_parallel(lift(&x), normal) {
            return Err(CurveError::Degenerate);
        }
        let f = Frame { x, normal };
        // The unit vectors must be finite in doubles too (a vector near the largest double).
        f.unit()?;
        Ok(f)
    }

    /// The standard frame: x along the first axis, the normal along the third.
    ///
    /// # Errors
    ///
    /// [`CurveError::Dimension`] unless D is 2 or 3.
    pub fn standard() -> Result<Frame<D>, CurveError> {
        let mut x = [0.0; D];
        if let Some(first) = x.first_mut() {
            *first = 1.0;
        }
        Frame::new(x, [0.0, 0.0, 1.0])
    }

    /// The stored reference vector.
    #[must_use]
    pub fn x(&self) -> [f64; D] {
        self.x
    }

    /// The stored normal.
    #[must_use]
    pub fn normal(&self) -> [f64; 3] {
        self.normal
    }

    /// The frame with its normal negated: x̂ kept, ŷ and n̂ negated, exactly.
    #[must_use]
    pub fn reversed(&self) -> Frame<D> {
        Frame {
            x: self.x,
            normal: self.normal.map(|v| -v),
        }
    }

    /// The unit vectors x̂, ŷ and n̂: in doubles, and enclosures of the exact ones.
    ///
    /// # Errors
    ///
    /// [`CurveError::NotFinite`] if a unit vector overflows (it cannot for a frame [`Frame::new`]
    /// accepted).
    pub fn unit_vectors(&self) -> Result<UnitVectors, CurveError> {
        let u = self.unit()?;
        Ok((u.doubles, u.exact))
    }

    /// The unit vectors, in doubles and enclosed.
    pub(crate) fn unit(&self) -> Result<Unit, CurveError> {
        let x = lift(&self.x);
        let n = self.normal;
        let p = cross(n, x);
        let q = cross(p, n);
        let doubles = [unit(q)?, unit(p)?, unit(n)?];
        if !doubles.iter().flatten().all(|v| v.is_finite()) {
            return Err(CurveError::NotFinite);
        }
        let ni = points(n)?;
        let pi = cross_enclosed(ni, points(x)?)?;
        let qi = cross_enclosed(pi, ni)?;
        let exact = [unit_enclosed(qi)?, unit_enclosed(pi)?, unit_enclosed(ni)?];
        Ok(Unit { doubles, exact })
    }
}
