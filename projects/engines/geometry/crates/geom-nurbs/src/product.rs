//! Symbolic products of Bernstein forms, and the normal patch of a surface with its collapsed
//! edges divided out: the Gauss-map bound of chapter 12.6.
//!
//! What it does: the product of two polynomials in Bernstein form is a polynomial of the summed
//! degree in Bernstein form, coefficient k being Σ C(p, i)·C(q, k − i)/C(p + q, k) · `a_i`·`b_(k−i)`
//! (and the same in each direction of a patch). On that rule, [`BernsteinCurve::scaled_by`],
//! [`BernsteinCurve::dot`] and [`BernsteinCurve::cross`] multiply curves exactly, as polynomials,
//! and [`BernsteinPatch::scaled_by`], [`BernsteinPatch::dot`] and [`BernsteinPatch::cross`]
//! patches. [`BernsteinPatch::normal`] builds the normal patch: `S_u` × `S_v` for a nonrational
//! patch, and for a rational one S = A / W the polynomial
//! N = W·(`A_u` × `A_v`) − `W_v`·(`A_u` × A) − `W_u`·(A × `A_v`) = W³·(`S_u` × `S_v`), a positive multiple
//! of the normal. [`Surface::normal_patches`] gives the normal patch of every Bezier patch of a
//! surface, and where a boundary edge of the surface is collapsed to one point (a pole, an apex)
//! divides the factor that vanishes along it out of the patches on that edge, so the bound keeps
//! the normals' directions instead of containing zero.
//!
//! Guarantees:
//!
//! - **Exact products, enclosed.** Each coefficient of a product is computed in interval
//!   arithmetic from enclosures of its factors' exact coefficients and of the binomial ratios, so
//!   it encloses the exact coefficient of the exact product. The binomials come from Pascal's
//!   rule in intervals, exact while they stay below 2^53.
//! - **The normal patch's sign and direction.** For a nonrational patch the normal patch's value at
//!   (u, v) is `S_u` × `S_v`; for a rational one W³ times it, W > 0. Either way the hull of its
//!   coefficients bounds the normals' directions (the Gauss map).
//! - **Collapsed edges divided exactly.** An edge counts as collapsed only when the surface's
//!   stored control points along it are one point exactly: identical doubles, or for a rational
//!   surface identical quotients w·P / w, compared through exact products (`geom-num`'s
//!   `two_prod`). Then S along the edge is a constant, the partial derivative along it vanishes
//!   there, and so does N, exactly: N = v·T (edge v = v0), and T's coefficients are N's with the
//!   vanishing row dropped and row j scaled by q/(j + 1) (q/(q − j) for v = v1; likewise in u).
//!   Chapter 12.9 names this derivation as ours, to be proven in its build piece: the proof is
//!   these lines and `tests/bernstein.rs`.
//! - **Typed refusals.** Products of rational forms are [`NurbsError::Rational`] (multiply their
//!   numerators and weight functions); factors on different domains are [`NurbsError::Domains`].
//! - **Deterministic.** Sums in index order; no fused multiply-add.
//!
//! Method: products of polynomials in Bernstein form (R. T. Farouki and V. T. Rajan, "Algorithms
//! for polynomials in Bernstein form", *Computer Aided Geometric Design* 5(1) (1988), 1–26) as
//! Piegl and Tiller use them for symbolic operators on NURBS (L. Piegl and W. Tiller, "Symbolic
//! operators for NURBS", *Computer-Aided Design* 29(5) (1997), 361–368,
//! <https://doi.org/10.1016/S0010-4485(96)00074-7>) and `NLib` implements them; the normal patch
//! for Gauss-map bounds (T. W. Sederberg and R. J. Meyers, "Loop detection in surface patch
//! intersections", *Computer Aided Geometric Design* 5(2) (1988), 161–171,
//! <https://doi.org/10.1016/0167-8396(88)90029-5>).
//!
//! Source: PORT from NVIDIA USD `BRep` `NLib`, Apache-2.0,
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_Bezier.cpp>
//!   SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
//!   Changed for Spare Kit: translated to safe `no_std` Rust from `N_BezFuncMultiplyBezCrvMatrix`,
//!   `N_BezFuncMultiplyBezFunc`, `N_BezSrfFuncMultiply`, `N_BezCrossProduct`,
//!   `N_BezSrfCrossProduct` and `N_BezSrfDotProduct`, in interval arithmetic so every coefficient
//!   is certified; the binomial ratios from Pascal's rule in intervals instead of `NLib`'s table of
//!   18 rows; the normal patch of rational patches and the division of collapsed edges are ours.
//! SPDX-License-Identifier: MPL-2.0 AND Apache-2.0
//!
//! Cost: a product of degrees (p1, q1) and (p2, q2) takes O((p1 + 1)(p2 + 1)(q1 + 1)(q2 + 1)·D)
//! interval operations; a rational normal patch three products of degree (3p − 1, 3q − 1); one
//! allocation per result, charged first.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the book's: p and q for degrees, a, b and c for coefficients, u and v for directions, w for weights"
)]

use alloc::vec::Vec;

use geom_num::eft::two_prod;
use geom_num::interval::Interval;
use geom_run::WorkMeter;

use crate::NurbsError;
use crate::arith::Num;
use crate::bernstein::{BernsteinCurve, BernsteinPatch, iv};
use crate::budget::vec_for;
use crate::index::{add, get, mul, real, sub};
use crate::ops::Direction;
use crate::surface::Surface;

/// The highest degree a product may have: twice [`crate::MAX_DEGREE`] times two, so a rational
/// normal patch of degree [`crate::MAX_DEGREE`] (degree 3p − 1) fits.
const MAX_PRODUCT_DEGREE: usize = 4 * crate::MAX_DEGREE;

/// Row `n` of Pascal's triangle in intervals: C(n, k) for k = 0 to n, exact while below 2^53.
fn binomials(meter: &mut WorkMeter<'_>, n: usize) -> Result<Vec<Interval>, NurbsError> {
    let mut row = vec_for(meter, add(n, 1)?)?;
    row.push(iv(1.0)?);
    for _ in 0..n {
        let mut next = Vec::new();
        next.try_reserve_exact(add(row.len(), 1)?)
            .map_err(|_| NurbsError::Stopped(meter.out_of_memory()))?;
        next.push(iv(1.0)?);
        for pair in row.windows(2) {
            let [a, b] = pair else {
                return Err(NurbsError::Defect);
            };
            next.push(a.plus(*b)?);
        }
        next.push(iv(1.0)?);
        row = next;
    }
    Ok(row)
}

/// The weights C(p, i)·C(q, k − i)/C(p + q, k), indexed [i][k − i].
fn ratios(meter: &mut WorkMeter<'_>, p: usize, q: usize) -> Result<Vec<Interval>, NurbsError> {
    let (bp, bq, bpq) = (
        binomials(meter, p)?,
        binomials(meter, q)?,
        binomials(meter, add(p, q)?)?,
    );
    let mut out = vec_for(meter, mul(add(p, 1)?, add(q, 1)?)?)?;
    for i in 0..=p {
        for j in 0..=q {
            out.push(
                get(&bp, i)?
                    .times(get(&bq, j)?)?
                    .over(get(&bpq, add(i, j)?)?)?,
            );
        }
    }
    Ok(out)
}

/// Degrees and coefficients, row by row.
type Grid<const E: usize> = ((usize, usize), Vec<[Interval; E]>);

/// The tensor product rule: c[k][l] = Σ `r_u`(i1, i2) `r_v`(j1, j2) f(a[i1][j1], b[i2][j2]) over
/// i1 + i2 = k and j1 + j2 = l, for row-major coefficient grids of degrees `da` and `db`.
fn tensor<X: Copy, Y: Copy, const E: usize>(
    meter: &mut WorkMeter<'_>,
    (a, da): (&[X], (usize, usize)),
    (b, db): (&[Y], (usize, usize)),
    f: impl Fn(X, Y) -> Result<[Interval; E], NurbsError>,
) -> Result<Grid<E>, NurbsError> {
    let degrees = (add(da.0, db.0)?, add(da.1, db.1)?);
    if degrees.0 > MAX_PRODUCT_DEGREE || degrees.1 > MAX_PRODUCT_DEGREE {
        return Err(NurbsError::Degree {
            degree: degrees.0.max(degrees.1),
        });
    }
    let ru = ratios(meter, da.0, db.0)?;
    let rv = ratios(meter, da.1, db.1)?;
    let wide = |k: usize| u64::try_from(k).map_err(|_| NurbsError::Defect);
    let terms = mul(
        mul(add(da.0, 1)?, add(db.0, 1)?)?,
        mul(add(da.1, 1)?, add(db.1, 1)?)?,
    )?;
    meter.charge(wide(terms)?.saturating_mul(wide(E.max(1))?))?;
    let count = mul(add(degrees.0, 1)?, add(degrees.1, 1)?)?;
    let zero = iv(0.0)?;
    let mut out = vec_for(meter, count)?;
    out.resize(count, [zero; E]);
    let (ca, cb) = (add(da.1, 1)?, add(db.1, 1)?);
    let cols = add(degrees.1, 1)?;
    for i1 in 0..=da.0 {
        for i2 in 0..=db.0 {
            let wu = get(&ru, add(mul(i1, add(db.0, 1)?)?, i2)?)?;
            for j1 in 0..=da.1 {
                let x = get(a, add(mul(i1, ca)?, j1)?)?;
                for j2 in 0..=db.1 {
                    let wv = get(&rv, add(mul(j1, add(db.1, 1)?)?, j2)?)?;
                    let y = get(b, add(mul(i2, cb)?, j2)?)?;
                    let term = f(x, y)?;
                    let w = wu.times(wv)?;
                    let slot = out
                        .get_mut(add(mul(add(i1, i2)?, cols)?, add(j1, j2)?)?)
                        .ok_or(NurbsError::Defect)?;
                    for (s, t) in slot.iter_mut().zip(&term) {
                        *s = s.plus(w.times(*t)?)?;
                    }
                }
            }
        }
    }
    Ok((degrees, out))
}

/// x·y for a scalar x and a point y.
fn scale<const D: usize>(x: [Interval; 1], y: [Interval; D]) -> Result<[Interval; D], NurbsError> {
    let [s] = x;
    let mut out = y;
    for c in &mut out {
        *c = s.times(*c)?;
    }
    Ok(out)
}

/// x·y.
fn dot<const D: usize>(x: [Interval; D], y: [Interval; D]) -> Result<[Interval; 1], NurbsError> {
    let mut s = iv(0.0)?;
    for (a, b) in x.iter().zip(&y) {
        s = s.plus(a.times(*b)?)?;
    }
    Ok([s])
}

/// x × y.
fn cross(x: [Interval; 3], y: [Interval; 3]) -> Result<[Interval; 3], NurbsError> {
    let [x0, x1, x2] = x;
    let [y0, y1, y2] = y;
    Ok([
        x1.times(y2)?.minus(x2.times(y1)?)?,
        x2.times(y0)?.minus(x0.times(y2)?)?,
        x0.times(y1)?.minus(x1.times(y0)?)?,
    ])
}

/// Two nonrational curves on one domain, or the reason not.
fn curve_pair<const D: usize, const E: usize>(
    a: &BernsteinCurve<D>,
    b: &BernsteinCurve<E>,
) -> Result<(), NurbsError> {
    if a.is_rational() || b.is_rational() {
        return Err(NurbsError::Rational);
    }
    if a.domain() != b.domain() {
        return Err(NurbsError::Domains);
    }
    Ok(())
}

/// Two nonrational patches on one domain, or the reason not.
fn patch_pair<const D: usize, const E: usize>(
    a: &BernsteinPatch<D>,
    b: &BernsteinPatch<E>,
) -> Result<(), NurbsError> {
    if a.is_rational() || b.is_rational() {
        return Err(NurbsError::Rational);
    }
    if a.domain() != b.domain() {
        return Err(NurbsError::Domains);
    }
    Ok(())
}

impl<const D: usize> BernsteinCurve<D> {
    /// The product f·C of a scalar polynomial f and this curve, of the summed degree.
    ///
    /// # Errors
    ///
    /// [`NurbsError::Rational`] if either is rational; [`NurbsError::Domains`] if their domains
    /// differ; [`NurbsError::Degree`] above degree 4·[`crate::MAX_DEGREE`];
    /// [`NurbsError::NotFinite`]; [`NurbsError::Stopped`].
    pub fn scaled_by(
        &self,
        f: &BernsteinCurve<1>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<BernsteinCurve<D>, NurbsError> {
        curve_pair(self, f)?;
        let (deg, pts) = tensor(
            meter,
            (f.points(), (f.degree(), 0)),
            (self.points(), (self.degree(), 0)),
            scale::<D>,
        )?;
        let _ = deg;
        BernsteinCurve::new(self.domain(), pts, None)
    }

    /// The scalar product C·E of two curves, of the summed degree.
    ///
    /// # Errors
    ///
    /// As [`BernsteinCurve::scaled_by`].
    pub fn dot(
        &self,
        other: &BernsteinCurve<D>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<BernsteinCurve<1>, NurbsError> {
        curve_pair(self, other)?;
        let (_, pts) = tensor(
            meter,
            (self.points(), (self.degree(), 0)),
            (other.points(), (other.degree(), 0)),
            dot::<D>,
        )?;
        BernsteinCurve::new(self.domain(), pts, None)
    }
}

impl BernsteinCurve<3> {
    /// The vector product C × E of two space curves, of the summed degree.
    ///
    /// # Errors
    ///
    /// As [`BernsteinCurve::scaled_by`].
    pub fn cross(
        &self,
        other: &BernsteinCurve<3>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<BernsteinCurve<3>, NurbsError> {
        curve_pair(self, other)?;
        let (_, pts) = tensor(
            meter,
            (self.points(), (self.degree(), 0)),
            (other.points(), (other.degree(), 0)),
            cross,
        )?;
        BernsteinCurve::new(self.domain(), pts, None)
    }
}

impl<const D: usize> BernsteinPatch<D> {
    /// The product f·S of a scalar polynomial patch f and this patch, of the summed degrees.
    ///
    /// # Errors
    ///
    /// As [`BernsteinCurve::scaled_by`].
    pub fn scaled_by(
        &self,
        f: &BernsteinPatch<1>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<BernsteinPatch<D>, NurbsError> {
        patch_pair(self, f)?;
        let (deg, pts) = tensor(
            meter,
            (f.points(), f.degrees()),
            (self.points(), self.degrees()),
            scale::<D>,
        )?;
        Ok(self.with(deg, pts))
    }

    /// The scalar product S·T of two patches, of the summed degrees.
    ///
    /// # Errors
    ///
    /// As [`BernsteinCurve::scaled_by`].
    pub fn dot(
        &self,
        other: &BernsteinPatch<D>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<BernsteinPatch<1>, NurbsError> {
        patch_pair(self, other)?;
        let (deg, pts) = tensor(
            meter,
            (self.points(), self.degrees()),
            (other.points(), other.degrees()),
            dot::<D>,
        )?;
        Ok(self.with(deg, pts))
    }
}

/// An edge of a patch's domain: u = u0, u = u1, v = v0 or v = v1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Edge {
    /// u at the start of its domain.
    U0,
    /// u at the end of its domain.
    U1,
    /// v at the start of its domain.
    V0,
    /// v at the end of its domain.
    V1,
}

impl BernsteinPatch<3> {
    /// The vector product S × T of two patches, of the summed degrees.
    ///
    /// # Errors
    ///
    /// As [`BernsteinCurve::scaled_by`].
    pub fn cross(
        &self,
        other: &BernsteinPatch<3>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<BernsteinPatch<3>, NurbsError> {
        patch_pair(self, other)?;
        let (deg, pts) = tensor(
            meter,
            (self.points(), self.degrees()),
            (other.points(), other.degrees()),
            cross,
        )?;
        Ok(self.with(deg, pts))
    }

    /// The normal patch: `S_u` × `S_v` of degrees (2p − 1, 2q − 1) for a nonrational patch, and for
    /// a rational one N = W·(`A_u` × `A_v`) − `W_v`·(`A_u` × A) − `W_u`·(A × `A_v`) = W³·(`S_u` × `S_v`)
    /// of degrees (3p − 1, 3q − 1).
    ///
    /// # Errors
    ///
    /// [`NurbsError::Degree`] above degree 4·[`crate::MAX_DEGREE`]; [`NurbsError::NotFinite`];
    /// [`NurbsError::Stopped`].
    pub fn normal(&self, meter: &mut WorkMeter<'_>) -> Result<BernsteinPatch<3>, NurbsError> {
        let Some(w) = self.weight_function() else {
            let su = self.partial(Direction::U, meter)?;
            let sv = self.partial(Direction::V, meter)?;
            return su.cross(&sv, meter);
        };
        let a = self.numerator();
        let au = a.partial(Direction::U, meter)?;
        let av = a.partial(Direction::V, meter)?;
        let wu = w.partial(Direction::U, meter)?;
        let wv = w.partial(Direction::V, meter)?;
        let first = au.cross(&av, meter)?.scaled_by(&w, meter)?;
        let second = au.cross(&a, meter)?.scaled_by(&wv, meter)?;
        let third = a.cross(&av, meter)?.scaled_by(&wu, meter)?;
        first.minus(&second, meter)?.minus(&third, meter)
    }

    /// This normal patch divided by the factor that vanishes along `edge` (v − v0, v1 − v, u − u0
    /// or u1 − u, each over its span's length), which the caller has proven to vanish there
    /// exactly: the vanishing row or column is dropped, and the others scaled.
    fn divide(
        &self,
        edge: Edge,
        meter: &mut WorkMeter<'_>,
    ) -> Result<BernsteinPatch<3>, NurbsError> {
        let (p, q) = self.degrees();
        let along_u = matches!(edge, Edge::U0 | Edge::U1);
        let n = if along_u { p } else { q };
        if n == 0 {
            return Err(NurbsError::Defect);
        }
        let degrees = if along_u {
            (sub(p, 1)?, q)
        } else {
            (p, sub(q, 1)?)
        };
        let zero_row = match edge {
            Edge::U0 | Edge::V0 => 0,
            Edge::U1 | Edge::V1 => n,
        };
        // The dropped coefficients must hold zero, as the caller's exact proof says.
        let line = if along_u { q } else { p };
        for k in 0..=line {
            let c = if along_u {
                self.at(zero_row, k)?
            } else {
                self.at(k, zero_row)?
            };
            if !c.iter().all(|x| x.lo() <= 0.0 && x.hi() >= 0.0) {
                return Err(NurbsError::Defect);
            }
        }
        let nr = iv(real(n)?)?;
        let mut out = vec_for(meter, mul(add(degrees.0, 1)?, add(degrees.1, 1)?)?)?;
        for i in 0..=degrees.0 {
            for j in 0..=degrees.1 {
                let (src, k) = match edge {
                    Edge::U0 => ((add(i, 1)?, j), add(i, 1)?),
                    Edge::V0 => ((i, add(j, 1)?), add(j, 1)?),
                    Edge::U1 => ((i, j), sub(n, i)?),
                    Edge::V1 => ((i, j), sub(n, j)?),
                };
                let factor = nr.over(iv(real(k)?)?)?;
                let mut c = self.at(src.0, src.1)?;
                for x in &mut c {
                    *x = factor.times(*x)?;
                }
                out.push(c);
            }
        }
        Ok(self.with(degrees, out))
    }
}

/// Whether homogeneous points (w·P with weight w) all have one Euclidean point exactly: w·P / w
/// equal, compared as exact products a·w' = a'·w through `two_prod`. A product whose rounding
/// error could be lost to underflow does not count as equal.
fn one_point<const D: usize>(points: &[([f64; D], f64)]) -> bool {
    let Some(&(first, w0)) = points.first() else {
        return false;
    };
    // a·b exactly as an unevaluated sum s + e: exact when a factor is zero, or when the product
    // is at least 2^−968, far from underflow (`two_prod`'s header).
    let exact = |a: f64, b: f64| {
        if a == 0.0 || b == 0.0 {
            return Some((0.0, 0.0));
        }
        let (s, e) = two_prod(a, b);
        if s.is_finite() && s.abs() >= f64::MIN_POSITIVE * 4.0 / f64::EPSILON {
            Some((s, e))
        } else {
            None
        }
    };
    points.iter().all(|&(c, w)| {
        c.iter().zip(&first).all(|(&x, &x0)| {
            if w == w0 {
                return x == x0;
            }
            match (exact(x, w0), exact(x0, w)) {
                (Some(l), Some(r)) => l == r,
                _ => false,
            }
        })
    })
}

impl Surface<3> {
    /// The boundary edges of the surface's domain along which its control points are one point
    /// exactly (poles and apexes), in the order u0, u1, v0, v1.
    #[must_use]
    pub fn collapsed_edges(&self) -> Vec<Edge> {
        let (rows, cols) = (self.basis_u().count(), self.basis_v().count());
        let pts = self.homogeneous();
        let w = self.weights();
        let at = |i: usize, j: usize| -> Option<([f64; 3], f64)> {
            let k = i.checked_mul(cols)?.checked_add(j)?;
            let c = *pts.get(k)?;
            let wk = match w {
                Some(ws) => *ws.get(k)?,
                None => 1.0,
            };
            Some((c, wk))
        };
        let line = |edge: Edge| -> Option<Vec<([f64; 3], f64)>> {
            match edge {
                Edge::U0 => (0..cols).map(|j| at(0, j)).collect(),
                Edge::U1 => (0..cols).map(|j| at(rows.checked_sub(1)?, j)).collect(),
                Edge::V0 => (0..rows).map(|i| at(i, 0)).collect(),
                Edge::V1 => (0..rows).map(|i| at(i, cols.checked_sub(1)?)).collect(),
            }
        };
        [Edge::U0, Edge::U1, Edge::V0, Edge::V1]
            .into_iter()
            .filter(|&e| line(e).is_some_and(|l| one_point(&l)))
            .collect()
    }

    /// The normal patch of every Bezier patch ([`BernsteinPatch::normal`]), in the order of
    /// [`Surface::bernstein`], with the vanishing factor of each collapsed boundary edge
    /// ([`Surface::collapsed_edges`]) divided out of the patches on it, so their bound keeps the
    /// directions of the normals near the pole.
    ///
    /// # Errors
    ///
    /// [`NurbsError::NotClamped`]; [`NurbsError::Degree`] for a rational surface above degree
    /// 4·[`crate::MAX_DEGREE`] / 3 in a direction; [`NurbsError::NotFinite`];
    /// [`NurbsError::Stopped`].
    pub fn normal_patches(
        &self,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<BernsteinPatch<3>>, NurbsError> {
        let edges = self.collapsed_edges();
        let (u_dom, v_dom) = (self.basis_u().domain(), self.basis_v().domain());
        let patches = self.bernstein(meter)?;
        let mut out = vec_for(meter, patches.len())?;
        for patch in &patches {
            let mut n = patch.normal(meter)?;
            let (u, v) = patch.domain();
            for &e in &edges {
                let on = match e {
                    Edge::U0 => u.0 == u_dom.0,
                    Edge::U1 => u.1 == u_dom.1,
                    Edge::V0 => v.0 == v_dom.0,
                    Edge::V1 => v.1 == v_dom.1,
                };
                if on {
                    n = n.divide(e, meter)?;
                }
            }
            out.push(n);
        }
        Ok(out)
    }
}
