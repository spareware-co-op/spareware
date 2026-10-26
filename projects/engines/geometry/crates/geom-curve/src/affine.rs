//! Typed transforms applied to control points, with a certified bound on the rounding: the
//! entries of each `geom-math` transform variant as doubles and as interval enclosures of their
//! exact values, and the image of a homogeneous point in both.
//!
//! What it does: [`Entries::of`] reads a [`TypedTransform`]: an exact grid transform and a rational
//! one through their exact integer entries (m / d), a certified one through its own doubles.
//! [`Entries::image`] maps a homogeneous point w·P (weight w) to M·(w·P) + w·t, once in doubles in a
//! fixed order and once in intervals, and returns the doubles with the distance between them, an
//! upper bound on how far the double image lies from the exact image under the exact transform.
//!
//! Guarantees:
//!
//! - **Exact entries enclosed.** An integer entry whose double is not exact (beyond 2^53) is
//!   widened by its rounding; each quotient m / d is an outward-rounded interval division; a
//!   certified transform is exact in its own doubles (`geom-math`'s `transform` header).
//! - **The image's error bounded.** The interval image holds the exact image of the exact point,
//!   and the double image is computed from the doubles of the same entries, so the returned
//!   distance bounds their difference.
//! - **Exact images report 0.** When every entry is exactly its double (grid and certified
//!   transforms), each coordinate is also summed exactly, as exact products in `geom-num`'s
//!   superaccumulator, and an image equal to its double reports an error of 0: interval arithmetic
//!   widens every result and could never say so.
//!
//! Method: affine invariance of B-splines and NURBS under affine maps of their control points (The
//! NURBS Book, 2nd ed., properties P3.10 and P4.10, <https://doi.org/10.1007/978-3-642-59223-2>),
//! with the weights unchanged, and interval arithmetic for the bound.
//!
//! Cost: twelve products and sums per point in doubles and in intervals; nothing allocates.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': h and w for a homogeneous point and its weight, x, y and z its coordinates, d the denominator, m the matrix, t the translation"
)]

use geom_math::{RationalTransform, Transform, TypedTransform};
use geom_num::eft::two_prod;
use geom_num::exact_sum::ExactSum;
use geom_num::interval::Interval;

use crate::error::CurveError;

/// The interval holding the integer `v` exactly: its double, widened by the double's rounding when
/// that is not exact.
///
/// # Errors
///
/// [`CurveError::NotFinite`] (it cannot happen for an `i128`).
pub fn int_interval(v: i128) -> Result<Interval, CurveError> {
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        reason = "the double is checked against the integer, and widened when it is not exact"
    )]
    let (f, back) = {
        let f = v as f64;
        (f, f as i128)
    };
    let p = Interval::point(f)?;
    if back == v {
        return Ok(p);
    }
    // A double's rounding is at most |f|·2^−53, below |f|·ε = |f|·2^−52.
    let u = f.abs() * f64::EPSILON;
    Ok(p.add(Interval::new(-u, u)?)?)
}

/// A transform's entries, rows (m0, m1, m2, t) for x, y and z, as doubles and as enclosures of the
/// exact values.
#[derive(Clone, Copy, Debug)]
pub struct Entries {
    doubles: [[f64; 4]; 3],
    exact: [[Interval; 4]; 3],
    planar: bool,
    /// Whether every entry's double is its exact value, so an image can be checked exactly.
    doubles_exact: bool,
}

impl Entries {
    /// The entries of `t`.
    ///
    /// # Errors
    ///
    /// [`CurveError::NotFinite`] if an entry overflows (it cannot for `geom-math`'s transforms).
    pub fn of(t: &TypedTransform) -> Result<Entries, CurveError> {
        let rational = |r: &RationalTransform| -> Result<[[Interval; 4]; 3], CurveError> {
            let den = r.denominator();
            let d = int_interval(i128::from(den))?;
            // m / d, with no division (which would widen an exact value) when d is 1.
            let q = |v: i128| -> Result<Interval, CurveError> {
                let n = int_interval(v)?;
                Ok(if den == 1 { n } else { n.div(d)? })
            };
            let m = r.matrix();
            let tr = r.translation_part();
            let mut out = [[Interval::point(0.0)?; 4]; 3];
            for ((row, mrow), &ti) in out.iter_mut().zip(&m).zip(&tr) {
                for (slot, &mij) in row.iter_mut().zip(mrow) {
                    *slot = q(i128::from(mij))?;
                }
                if let Some(last) = row.last_mut() {
                    *last = q(ti)?;
                }
            }
            Ok(out)
        };
        let exact = match t {
            TypedTransform::Exact(g) => rational(&RationalTransform::from_grid(g))?,
            TypedTransform::Rational(r) => rational(r)?,
            TypedTransform::Certified(c) => {
                let mut out = [[Interval::point(0.0)?; 4]; 3];
                for (row, r) in out.iter_mut().zip(c.rows()) {
                    for (slot, v) in row.iter_mut().zip(r) {
                        *slot = Interval::point(v)?;
                    }
                }
                out
            }
        };
        let doubles: Transform = t.to_transform()?;
        let rows = doubles.rows();
        let doubles_exact = rows
            .iter()
            .flatten()
            .zip(exact.iter().flatten())
            .all(|(&d, e)| e.lo() == d && e.hi() == d);
        Ok(Entries {
            doubles: rows,
            exact,
            planar: t.acts_in_plane(),
            doubles_exact,
        })
    }

    /// The rows (m0, m1, m2, t) as doubles.
    pub(crate) fn rows(&self) -> &[[f64; 4]; 3] {
        &self.doubles
    }

    /// The rows (m0, m1, m2, t) as enclosures of the exact entries.
    pub(crate) fn exact_rows(&self) -> &[[Interval; 4]; 3] {
        &self.exact
    }

    /// Whether the transform leaves z alone, so it applies to planar points.
    #[must_use]
    pub fn acts_in_plane(&self) -> bool {
        self.planar
    }

    /// The image M·x + t of an enclosed Euclidean point under the exact entries, enclosed.
    ///
    /// # Errors
    ///
    /// [`CurveError::NotFinite`] if the image overflows.
    pub fn image_enclosed(&self, x: [Interval; 3]) -> Result<[Interval; 3], CurveError> {
        let mut out = [Interval::point(0.0)?; 3];
        for (o, row) in out.iter_mut().zip(&self.exact) {
            let [m0, m1, m2, t] = *row;
            let [x0, x1, x2] = x;
            *o = m0.mul(x0)?.add(m1.mul(x1)?)?.add(m2.mul(x2)?)?.add(t)?;
        }
        Ok(out)
    }

    /// The image M·h + w·t of the homogeneous point `h` with weight `w` (a planar point has z = 0),
    /// in doubles, and an upper bound on |double image − exact image| / w: the Euclidean point's
    /// error.
    ///
    /// # Errors
    ///
    /// [`CurveError::NotFinite`] if the image overflows.
    pub fn image(&self, h: [f64; 3], w: f64) -> Result<([f64; 3], f64), CurveError> {
        let [x, y, z] = h;
        let mut out = [0.0; 3];
        let mut diff = [Interval::point(0.0)?; 3];
        let hw = [x, y, z, w];
        for (k, (row, erow)) in self.doubles.iter().zip(&self.exact).enumerate() {
            let mut s = 0.0;
            let mut e = Interval::point(0.0)?;
            for (&m, (&em, &v)) in row.iter().zip(erow.iter().zip(&hw)) {
                s += m * v;
                e = e.add(em.mul(Interval::point(v)?)?)?;
            }
            if !s.is_finite() {
                return Err(CurveError::NotFinite);
            }
            if let (Some(o), Some(d)) = (out.get_mut(k), diff.get_mut(k)) {
                *o = s;
                *d = Interval::point(s)?.sub(e)?;
            }
        }
        if self.doubles_exact && self.images_exactly(&hw, &out) {
            return Ok((out, 0.0));
        }
        let err = Interval::norm(&diff)?.div(Interval::point(w)?)?.hi();
        Ok((out, err))
    }

    /// Whether each double of `out` is exactly Σ m·v over its row, by exact products (`two_prod`,
    /// exact while each product is a whole multiple of 2^−1074 with a representable error, which
    /// holds from 2^−968 up) summed in a superaccumulator.
    fn images_exactly(&self, hw: &[f64; 4], out: &[f64; 3]) -> bool {
        let tiny = f64::MIN_POSITIVE * 4.0 / f64::EPSILON;
        self.doubles.iter().zip(out).all(|(row, &s)| {
            let mut sum = ExactSum::new();
            for (&m, &v) in row.iter().zip(hw) {
                if m == 0.0 || v == 0.0 {
                    continue;
                }
                let (p, e) = two_prod(m, v);
                if !(p.is_finite() && p.abs() >= tiny) {
                    return false;
                }
                sum.add(p);
                sum.add(e);
            }
            sum.add(-s);
            sum.round_ieee() == 0.0
        })
    }
}
