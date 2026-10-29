//! Exact integer paths for Bernstein forms on the grid: the Bernstein coefficients of a plane's
//! equation over a grid Bezier piece, and the normal patch of a grid Bezier patch, with every sign
//! (zeros included) exact, each within a written bit budget.
//!
//! What it does: a Bezier piece whose control points are grid points (chapter 6.1) has integer
//! Bernstein coefficients. [`plane_coefficients`] gives the coefficients of n·x + d over it, for a
//! plane of `geom-num`'s exact [`Plane`] type: they are n·`P_k` + d, by affine invariance, so the
//! signs of [`Signs`] decide exactly whether the piece can meet the plane (chapter 12.3's exact
//! path: "all of its signs, zeros included, are then exact"). [`normal_patch`] gives the normal
//! patch of a nonrational grid Bezier patch in scaled form, its coefficient (a, b) being
//! C(2p − 1, a)·C(2q − 1, b)·`n_ab` times a positive constant, so each has the direction and sign
//! of the Bernstein coefficient `n_ab` of `S_u` × `S_v`; a boundary edge whose control points are
//! one grid point is divided out exactly.
//!
//! Guarantees, with M = 2^40 − 1 the largest grid coordinate:
//!
//! - **Plane coefficients within 2^126.** [`Plane`] holds |`n_i`| < 2^83 and |d| < 2^125 (checked at
//!   construction). Each |`n_i`·`x_i`| < 2^83·2^40 = 2^123, the sum of three below 3·2^123 < 2^125,
//!   and with d below 2^126: in `i128` (limit 2^127), every step checked.
//! - **Normal coefficients within 2^193.** In scaled form the partial derivatives' coefficients
//!   are C(p − 1, i)·C(q, j)·(`P_(i+1,j)` − `P_ij`) and C(p, i)·C(q − 1, j)·(`P_(i,j+1)` − `P_ij`),
//!   differences of at most 2M < 2^41. Products of scaled forms are plain convolutions (the
//!   binomials are already inside), and a cross product's component is a difference of two
//!   products, so coefficient (a, b) is at most 2·(2M)²·Σ C(p − 1, i1)·C(p, i2)·C(q, j1)·C(q − 1, j2)
//!   = 2·(2M)²·C(2p − 1, a)·C(2q − 1, b) < 2^83·2^(2p−1)·2^(2q−1) = 2^(81+2p+2q) ≤ 2^193 for
//!   p, q ≤ 28 ([`crate::MAX_DEGREE`]), and every partial sum below the same bound: in
//!   `geom-num`'s [`I256`] (limit 2^255), every step checked. A check that fails would be a defect
//!   in this derivation, reported as [`NurbsError::Defect`], never a wrong sign.
//! - **Collapsed edges, exactly.** An edge counts only when its grid points are equal; then the
//!   normal patch's row along it is zero (checked), and in scaled form dividing by the factor
//!   that vanishes there is dropping that row: f = Σ `c_k`·t^k·(1 − t)^(n−k) with `c_0` = 0 is
//!   t·Σ `c_(k+1)`·t^k·(1 − t)^(n−1−k), and with `c_n` = 0 it is (1 − t)·Σ `c_k`·t^k·(1 − t)^(n−1−k).
//! - **Deterministic.** Integer arithmetic only.
//!
//! Method: exact integer evaluation with bit budgets derived before the code, as chapter 6.2
//! requires, on the Bernstein form's affine invariance and products in the scaled basis
//! t^k·(1 − t)^(n−k) (R. T. Farouki and V. T. Rajan, *Computer Aided Geometric Design* 5(1) (1988),
//! 1–26; the normal patch as in `crate::product`, after Sederberg and Meyers 1988,
//! <https://doi.org/10.1016/0167-8396(88)90029-5>).
//!
//! Cost: linear in the control points for a plane; O(p²·q²) wide multiplications for a normal
//! patch; one allocation per result, charged first.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the book's: p and q for degrees, a and b for coefficient indices, n and d for a plane"
)]

use alloc::vec::Vec;

use geom_num::grid::P3;
use geom_num::plane::Plane;
use geom_num::wide::I256;
use geom_run::WorkMeter;

use crate::NurbsError;
use crate::basis::MAX_DEGREE;
use crate::budget::vec_for;
use crate::index::{add, get, mul, sub};
use crate::product::Edge;

/// How many of a polynomial's Bernstein coefficients are positive, negative and zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Signs {
    /// Coefficients above zero.
    pub positive: usize,
    /// Coefficients below zero.
    pub negative: usize,
    /// Coefficients exactly zero.
    pub zero: usize,
}

impl Signs {
    /// Whether every coefficient has one strict sign, which proves the polynomial has no zero on
    /// its domain (the Bernstein basis is positive inside it and sums to 1).
    #[must_use]
    pub fn excludes_zero(&self) -> bool {
        self.zero == 0 && (self.positive == 0 || self.negative == 0)
    }

    fn count(&mut self, s: core::cmp::Ordering) -> Result<(), NurbsError> {
        let slot = match s {
            core::cmp::Ordering::Greater => &mut self.positive,
            core::cmp::Ordering::Less => &mut self.negative,
            core::cmp::Ordering::Equal => &mut self.zero,
        };
        *slot = add(*slot, 1)?;
        Ok(())
    }
}

/// The signs of integer coefficients.
#[must_use]
pub fn signs(coefficients: &[i128]) -> Signs {
    let mut s = Signs::default();
    for c in coefficients {
        // At most usize::MAX coefficients exist, so the count cannot overflow.
        let _ = s.count(c.cmp(&0));
    }
    s
}

/// The Bernstein coefficients of n·x + d over the Bezier piece (curve or patch, nonrational or with
/// positive weights) whose control points are `points`: n·`P_k` + d each, exactly. A rational piece
/// has the weighted coefficients `w_k`·(n·`P_k` + d), whose signs these are.
///
/// # Errors
///
/// [`NurbsError::Defect`] if a step exceeded its bit budget (it cannot; see the module header);
/// [`NurbsError::Stopped`].
pub fn plane_coefficients(
    points: &[P3],
    plane: &Plane,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<i128>, NurbsError> {
    let n = plane.n();
    let mut out = vec_for(meter, points.len())?;
    for p in points {
        let x = [p.x.get(), p.y.get(), p.z.get()];
        let mut s = plane.d();
        for (&ni, &xi) in n.iter().zip(&x) {
            let term = ni.checked_mul(i128::from(xi)).ok_or(NurbsError::Defect)?;
            s = s.checked_add(term).ok_or(NurbsError::Defect)?;
        }
        out.push(s);
    }
    Ok(out)
}

/// The scaled normal patch of a grid Bezier patch (see [`normal_patch`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactNormal {
    degrees: (usize, usize),
    coefficients: Vec<[I256; 3]>,
    divided: Vec<Edge>,
}

impl ExactNormal {
    /// The degrees in u and v, after any division.
    #[must_use]
    pub fn degrees(&self) -> (usize, usize) {
        self.degrees
    }

    /// The scaled coefficients, row by row: each a positive multiple of the Bernstein coefficient
    /// of the (reduced) normal patch.
    #[must_use]
    pub fn coefficients(&self) -> &[[I256; 3]] {
        &self.coefficients
    }

    /// The collapsed edges divided out.
    #[must_use]
    pub fn divided(&self) -> &[Edge] {
        &self.divided
    }

    /// The signs of component `k` (0, 1 or 2) of the coefficients.
    ///
    /// # Errors
    ///
    /// [`NurbsError::Index`] for k above 2.
    pub fn signs(&self, k: usize) -> Result<Signs, NurbsError> {
        if k > 2 {
            return Err(NurbsError::Index { index: k, count: 3 });
        }
        let mut s = Signs::default();
        for c in &self.coefficients {
            let v = get(c, k)?;
            s.count(if v.is_zero() {
                core::cmp::Ordering::Equal
            } else if v.is_negative() {
                core::cmp::Ordering::Less
            } else {
                core::cmp::Ordering::Greater
            })?;
        }
        Ok(s)
    }
}

/// C(n, k) for n ≤ 2·[`MAX_DEGREE`], exactly (C(56, 28) < 2^53).
fn binomial(n: usize, k: usize) -> Result<i128, NurbsError> {
    if k > n {
        return Ok(0);
    }
    let mut c: i128 = 1;
    for i in 0..k {
        let top = i128::try_from(sub(n, i)?).map_err(|_| NurbsError::Defect)?;
        let bottom = i128::try_from(add(i, 1)?).map_err(|_| NurbsError::Defect)?;
        // c·(n − i) / (i + 1) is C(n, i + 1), a whole number: exact division.
        c = c
            .checked_mul(top)
            .and_then(|x| x.checked_div(bottom))
            .ok_or(NurbsError::Defect)?;
    }
    Ok(c)
}

/// The grid point (i, j) of a net of `cols` columns.
fn at(points: &[P3], cols: usize, i: usize, j: usize) -> Result<[i128; 3], NurbsError> {
    let p = get(points, add(mul(i, cols)?, j)?)?;
    Ok([p.x.get(), p.y.get(), p.z.get()].map(i128::from))
}

/// The wide value of `x`.
fn wide(x: i128) -> I256 {
    I256::from_i128(x)
}

/// The normal patch of the nonrational Bezier patch of degrees (p, q) ≥ 1 whose control points are
/// the grid points `points`, row by row ((p + 1)·(q + 1) of them), in scaled form (module
/// header), with each boundary edge whose grid points are one point divided out.
///
/// # Errors
///
/// [`NurbsError::Degree`] unless 1 ≤ p, q ≤ [`MAX_DEGREE`]; [`NurbsError::PointCount`] unless
/// there are (p + 1)·(q + 1) points; [`NurbsError::Degenerate`] if every edge is collapsed;
/// [`NurbsError::Defect`] if a step exceeded its budget (it cannot); [`NurbsError::Stopped`].
pub fn normal_patch(
    points: &[P3],
    (p, q): (usize, usize),
    meter: &mut WorkMeter<'_>,
) -> Result<ExactNormal, NurbsError> {
    for degree in [p, q] {
        if degree == 0 || degree > MAX_DEGREE {
            return Err(NurbsError::Degree { degree });
        }
    }
    let cols = add(q, 1)?;
    let expected = mul(add(p, 1)?, cols)?;
    if points.len() != expected {
        return Err(NurbsError::PointCount {
            points: points.len(),
            expected,
        });
    }
    let scaled = |i: usize, j: usize, dir_u: bool| -> Result<[i128; 3], NurbsError> {
        let (a, b) = if dir_u {
            (at(points, cols, i, j)?, at(points, cols, add(i, 1)?, j)?)
        } else {
            (at(points, cols, i, j)?, at(points, cols, i, add(j, 1)?)?)
        };
        let c = if dir_u {
            binomial(sub(p, 1)?, i)?.checked_mul(binomial(q, j)?)
        } else {
            binomial(p, i)?.checked_mul(binomial(sub(q, 1)?, j)?)
        }
        .ok_or(NurbsError::Defect)?;
        let mut out = [0_i128; 3];
        for (o, (&bx, &ax)) in out.iter_mut().zip(b.iter().zip(&a)) {
            *o = bx
                .checked_sub(ax)
                .and_then(|d| d.checked_mul(c))
                .ok_or(NurbsError::Defect)?;
        }
        Ok(out)
    };
    let (dp, dq) = (sub(mul(p, 2)?, 1)?, sub(mul(q, 2)?, 1)?);
    let width = add(dq, 1)?;
    let count = mul(add(dp, 1)?, width)?;
    let terms = mul(mul(p, add(p, 1)?)?, mul(q, add(q, 1)?)?)?;
    meter.charge(u64::try_from(terms).map_err(|_| NurbsError::Defect)?)?;
    let zero = wide(0);
    let mut c = vec_for(meter, count)?;
    c.resize(count, [zero; 3]);
    for i1 in 0..p {
        for j1 in 0..=q {
            let a = scaled(i1, j1, true)?.map(wide);
            for i2 in 0..=p {
                for j2 in 0..q {
                    let b = scaled(i2, j2, false)?.map(wide);
                    let slot = c
                        .get_mut(add(mul(add(i1, i2)?, width)?, add(j1, j2)?)?)
                        .ok_or(NurbsError::Defect)?;
                    let x = cross(&a, &b)?;
                    for (s, t) in slot.iter_mut().zip(&x) {
                        *s = s.checked_add(t).ok_or(NurbsError::Defect)?;
                    }
                }
            }
        }
    }
    let mut out = ExactNormal {
        degrees: (dp, dq),
        coefficients: c,
        divided: Vec::new(),
    };
    let edges = collapsed(points, (p, q))?;
    if edges.len() == 4 {
        return Err(NurbsError::Degenerate);
    }
    for e in edges {
        out = divide(meter, out, e)?;
    }
    Ok(out)
}

/// a × b, checked.
fn cross(a: &[I256; 3], b: &[I256; 3]) -> Result<[I256; 3], NurbsError> {
    let [a0, a1, a2] = a;
    let [b0, b1, b2] = b;
    let m = |x: &I256, y: &I256, z: &I256, w: &I256| -> Result<I256, NurbsError> {
        let l = x.checked_mul(y).ok_or(NurbsError::Defect)?;
        let r = z.checked_mul(w).ok_or(NurbsError::Defect)?;
        l.checked_sub(&r).ok_or(NurbsError::Defect)
    };
    Ok([m(a1, b2, a2, b1)?, m(a2, b0, a0, b2)?, m(a0, b1, a1, b0)?])
}

/// The boundary edges of a grid net whose points are all one point.
fn collapsed(points: &[P3], (p, q): (usize, usize)) -> Result<Vec<Edge>, NurbsError> {
    let cols = add(q, 1)?;
    let mut out = Vec::new();
    for edge in [Edge::U0, Edge::U1, Edge::V0, Edge::V1] {
        let len = match edge {
            Edge::U0 | Edge::U1 => q,
            Edge::V0 | Edge::V1 => p,
        };
        let cell = |k: usize| match edge {
            Edge::U0 => (0, k),
            Edge::U1 => (p, k),
            Edge::V0 => (k, 0),
            Edge::V1 => (k, q),
        };
        let (i0, j0) = cell(0);
        let first = at(points, cols, i0, j0)?;
        let mut one = true;
        for k in 1..=len {
            let (i, j) = cell(k);
            if at(points, cols, i, j)? != first {
                one = false;
                break;
            }
        }
        if one {
            out.push(edge);
        }
    }
    Ok(out)
}

/// The scaled normal patch divided by the factor vanishing along `edge`: the zero row or column
/// (checked) dropped.
fn divide(
    meter: &mut WorkMeter<'_>,
    n: ExactNormal,
    edge: Edge,
) -> Result<ExactNormal, NurbsError> {
    let (dp, dq) = n.degrees;
    let along_u = matches!(edge, Edge::U0 | Edge::U1);
    let deg = if along_u { dp } else { dq };
    if deg == 0 {
        return Err(NurbsError::Degenerate);
    }
    let zero_line = match edge {
        Edge::U0 | Edge::V0 => 0,
        Edge::U1 | Edge::V1 => deg,
    };
    let width = add(dq, 1)?;
    let index = |i: usize, j: usize| add(mul(i, width)?, j);
    let line = if along_u { dq } else { dp };
    for k in 0..=line {
        let c = if along_u {
            get(&n.coefficients, index(zero_line, k)?)?
        } else {
            get(&n.coefficients, index(k, zero_line)?)?
        };
        if !c.iter().all(I256::is_zero) {
            return Err(NurbsError::Defect);
        }
    }
    let degrees = if along_u {
        (sub(dp, 1)?, dq)
    } else {
        (dp, sub(dq, 1)?)
    };
    let shift = usize::from(zero_line == 0);
    let mut out = vec_for(meter, mul(add(degrees.0, 1)?, add(degrees.1, 1)?)?)?;
    for i in 0..=degrees.0 {
        for j in 0..=degrees.1 {
            let src = if along_u {
                index(add(i, shift)?, j)?
            } else {
                index(i, add(j, shift)?)?
            };
            out.push(get(&n.coefficients, src)?);
        }
    }
    let mut divided = n.divided;
    divided.push(edge);
    Ok(ExactNormal {
        degrees,
        coefficients: out,
        divided,
    })
}
