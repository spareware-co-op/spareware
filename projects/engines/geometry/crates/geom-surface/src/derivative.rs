//! Certified bounds on a surface's second partials and on its unit normal's partials over a
//! parameter box, for every kind, from the kind's partials there (piece G68).
//!
//! What it does: [`second_partials`] reads `S_uu`, `S_uv` and `S_vv` over a box from a kind's
//! partials to order 2 ([`crate::ParamSurface::partials_over`]) as boxes, and
//! [`SecondPartials::chord`] gives chapter 19.3's M = max(`M_uu`, `M_vv`) + `M_uv`, each M an upper
//! bound on its box's Euclidean norm: M bounds ‖`d_u`²·`S_uu` + 2·`d_u`·`d_v`·`S_uv` + `d_v`²·`S_vv`‖
//! for every unit direction (`d_u`, `d_v`), because 2·|`d_u`·`d_v`| ≤ 1, and so the chord bound
//! ½·M·r² of tessellation. [`normal_partials`] bounds the partials `N_u` and `N_v` of the unit
//! normal N = n/|n|, n = `S_u` × `S_v`: from the partials to order 2, the jets of `S_u` and `S_v` (order
//! 1), their cross product's jet, its length's by the root recurrence and N's by the quotient
//! recurrence, N's value cut to [−1, 1] in each coordinate since N is a unit vector. Each of
//! `N_u` and `N_v` is then cut to the box ±|`n_u`|/|n| (or ±|`n_v`|/|n|): `N_u` = (`n_u` − (N·`n_u`)·N)/|n|,
//! the part of `n_u` across N divided by |n|, is no longer than |`n_u`|/|n|.
//!
//! Guarantees:
//!
//! - **Certified.** Each box holds the exact partial at every point of the box, from both sides
//!   of a break inside it, as the kind's partials do: every step is interval arithmetic rounded
//!   outward on enclosures of exact values (inclusion isotonicity), and every cut intersects two
//!   enclosures of one value.
//! - **Unbounded, never a guess.** Where n may vanish over the box (a pole, an apex, a degenerate
//!   point), N has no derivative to bound there, and the result is `Unbounded`; so it is for a
//!   kind whose partials cannot be bounded over the box (an offset where its base normal
//!   vanishes, a swept kind on an unbounded profile).
//! - **Deterministic.** A fixed sequence of operations.
//!
//! Method: the second-derivative bound and the chord bound of chapter 19.3
//! (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`); the derivative of a
//! normalised vector, (n/|n|)′ = (n′ − (N·n′)·N)/|n| (M. P. do Carmo, *Differential Geometry of
//! Curves and Surfaces*, 2nd ed., Dover (2016), section 3-2, the differential of the Gauss map),
//! in two-variable interval jets (A. Griewank and A. Walther, *Evaluating Derivatives*, 2nd ed.,
//! SIAM (2008), chapter 13, <https://doi.org/10.1137/1.9780898717761>; R. E. Moore, R. B. Kearfott
//! and M. J. Cloud, *Introduction to Interval Analysis*, SIAM (2009),
//! <https://doi.org/10.1137/1.9780898717716>).
//!
//! Cost: one call of the kind's partials to order 2 over the box, then a fixed number of interval
//! operations: O(1).

#![allow(
    clippy::many_single_char_names,
    reason = "the formulas' names: p the partials, s the surface's jets, n the normal, q a quotient, x a coordinate"
)]

use alloc::vec::Vec;

use geom_curve::tight::{self, point};
use geom_curve::{CurveError, DerivativeBound};
use geom_num::interval::Interval;
use geom_nurbs::Bounds;

use crate::jet::Jet;
use crate::param::partial_index;

/// Boxes holding a surface's second partials over a box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SecondPartials {
    /// ∂²S/∂u².
    pub uu: Bounds<3>,
    /// ∂²S/∂u∂v.
    pub uv: Bounds<3>,
    /// ∂²S/∂v².
    pub vv: Bounds<3>,
}

/// A bound on a surface's second partials over a box, or none where they cannot be bounded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SecondDerivativeBound {
    /// Every value of `S_uu`, `S_uv` and `S_vv` over the box lies in these boxes.
    Bounded(SecondPartials),
    /// Not bounded over the box (a kind whose partials cannot be bounded there).
    Unbounded,
}

impl SecondPartials {
    /// Chapter 19.3's M = max(`M_uu`, `M_vv`) + `M_uv`, each an upper bound on the Euclidean norm of
    /// its box, rounded up: no smaller than ‖`d_u`²·`S_uu` + 2·`d_u`·`d_v`·`S_uv` + `d_v`²·`S_vv`‖ for any
    /// unit (`d_u`, `d_v`) at any point of the box.
    ///
    /// # Errors
    ///
    /// [`CurveError::NotFinite`] if the bound overflows.
    pub fn chord(&self) -> Result<f64, CurveError> {
        let (uu, uv, vv) = (norm(&self.uu)?, norm(&self.uv)?, norm(&self.vv)?);
        Ok(tight::add(point(tight::larger(uu, vv))?, point(uv)?)?.hi())
    }
}

/// An upper bound on the Euclidean norm of every point of a box.
fn norm(b: &Bounds<3>) -> Result<f64, CurveError> {
    let mut c = [point(0.0)?; 3];
    for (o, (&lo, &hi)) in c.iter_mut().zip(b.lo.iter().zip(&b.hi)) {
        *o = Interval::new(lo, hi)?;
    }
    Ok(tight::norm(&c)?.hi())
}

/// The box of three intervals.
fn hull(x: [Interval; 3]) -> Bounds<3> {
    Bounds {
        lo: x.map(Interval::lo),
        hi: x.map(Interval::hi),
    }
}

/// The partial (i, j) of a list of partials, listed by `partial_index`.
fn at(p: &[[Interval; 3]], i: usize, j: usize) -> Result<[Interval; 3], CurveError> {
    p.get(partial_index(i, j)?)
        .copied()
        .ok_or(CurveError::Defect)
}

/// `S_uu`, `S_uv` and `S_vv` from a kind's partials to order 2 over a box (module documentation).
///
/// # Errors
///
/// [`CurveError::Defect`] if the list is shorter.
pub(crate) fn second_partials(p: &[[Interval; 3]]) -> Result<SecondPartials, CurveError> {
    Ok(SecondPartials {
        uu: hull(at(p, 2, 0)?),
        uv: hull(at(p, 1, 1)?),
        vv: hull(at(p, 0, 2)?),
    })
}

/// The jet of order `k` of coordinate `d` of a list of partials (at least to order `k`).
fn coordinate(p: &[[Interval; 3]], d: usize, k: usize) -> Result<Jet, CurveError> {
    let col: Vec<Interval> = p
        .iter()
        .map(|x| x.get(d).copied().ok_or(CurveError::Defect))
        .collect::<Result<_, _>>()?;
    Jet::from_partials(&col, k)
}

/// The cross product of two jets of vectors.
fn cross(a: &[Jet; 3], b: &[Jet; 3]) -> Result<[Jet; 3], CurveError> {
    let [a0, a1, a2] = a;
    let [b0, b1, b2] = b;
    Ok([
        a1.mul(b2)?.sub(&a2.mul(b1)?)?,
        a2.mul(b0)?.sub(&a0.mul(b2)?)?,
        a0.mul(b1)?.sub(&a1.mul(b0)?)?,
    ])
}

/// The partials `N_u` and `N_v` of the unit normal over a box, from a kind's partials to order 2
/// there (module documentation): `Unbounded` where the normal may vanish.
///
/// # Errors
///
/// [`CurveError::NotFinite`] if a value overflows; [`CurveError::Defect`] if the list is shorter.
pub(crate) fn normal_partials(p: &[[Interval; 3]]) -> Result<[DerivativeBound<3>; 2], CurveError> {
    match unit_normal_partials(p) {
        Ok((nu, nv)) => Ok([
            DerivativeBound::Bounded(hull(nu)),
            DerivativeBound::Bounded(hull(nv)),
        ]),
        // |n| may be zero somewhere in the box: the root or the quotient refused.
        Err(CurveError::Unbounded) => Ok([DerivativeBound::Unbounded; 2]),
        Err(e) => Err(e),
    }
}

/// `N_u` and `N_v` (module documentation); [`CurveError::Unbounded`] where |n| may be zero.
fn unit_normal_partials(p: &[[Interval; 3]]) -> Result<([Interval; 3], [Interval; 3]), CurveError> {
    let s = [
        coordinate(p, 0, 2)?,
        coordinate(p, 1, 2)?,
        coordinate(p, 2, 2)?,
    ];
    let su = [s[0].du()?, s[1].du()?, s[2].du()?];
    let sv = [s[0].dv()?, s[1].dv()?, s[2].dv()?];
    let n = cross(&su, &sv)?;
    let length = n[0]
        .mul(&n[0])?
        .add(&n[1].mul(&n[1])?)?
        .add(&n[2].mul(&n[2])?)?
        .sqrt()?;
    let unit = Interval::new(-1.0, 1.0)?;
    let mut nu = [point(0.0)?; 3];
    let mut nv = [point(0.0)?; 3];
    for (d, x) in n.iter().enumerate() {
        let q = x.div_valued(&length, unit)?;
        let (Some(a), Some(b)) = (nu.get_mut(d), nv.get_mut(d)) else {
            return Err(CurveError::Defect);
        };
        // The coefficients of order 1 are the partials themselves (1!·0! = 1).
        *a = q.get(1, 0)?;
        *b = q.get(0, 1)?;
    }
    // |N_u| ≤ |n_u|/|n|, and likewise in v.
    let least = length.get(0, 0)?.lo();
    let cap = |dn: [Interval; 3], out: &mut [Interval; 3]| -> Result<(), CurveError> {
        let r = tight::div(point(tight::norm(&dn)?.hi())?, point(least)?)?.hi();
        let ball = Interval::new(-r, r)?;
        for o in out.iter_mut() {
            *o = tight::meet(*o, ball)?;
        }
        Ok(())
    };
    let n1 = |along_u: bool| -> Result<[Interval; 3], CurveError> {
        let mut o = [point(0.0)?; 3];
        for (slot, x) in o.iter_mut().zip(&n) {
            *slot = if along_u { x.get(1, 0)? } else { x.get(0, 1)? };
        }
        Ok(o)
    };
    cap(n1(true)?, &mut nu)?;
    cap(n1(false)?, &mut nv)?;
    Ok((nu, nv))
}
