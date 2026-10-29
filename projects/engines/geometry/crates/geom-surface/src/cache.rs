//! A NURBS cache of a surface kind on its own parameters, by Piegl and Tiller's sample,
//! interpolate and remove-knots method, certified by interval bounds.
//!
//! What it does: [`same_parameter`] cuts the kind's domain at its breaks into smooth cells,
//! samples the kind on an n × n grid of equally spaced parameters in each cell, and interpolates
//! the samples with a bicubic B-spline surface on those parameters (A9.4, through `geom-nurbs`'s
//! `Curve::interpolate_at` along u and then along v), each cell's knots those averaging (9.8)
//! gives its equally spaced parameters, joined C⁰ at the breaks, so the surface's spans never
//! straddle a break. It then certifies the
//! surface against the kind span by span, and samples twice as densely while the certified error
//! is above half the target and still falls by at least a quarter with each doubling (the error
//! itself falls about sixteen times for a smooth kind; where the certificate stops falling,
//! rounding or a jump of the kind is what is left). Last, it removes knots (A5.8 on every row of the net at once,
//! `geom-nurbs`'s certified `Curve::remove_knot`) while the removals' certified changes fit in
//! what the target leaves, and returns the surface with its deviation: the certified error plus
//! the removals' changes.
//!
//! The certificate on a span box B with centre c is Taylor's theorem with the Lagrange remainder
//! for E = O − N, the kind minus the surface, in tensor-product form: Taylor in u to degree p = 3
//! with its remainder, then each coefficient Taylor in v to degree p with its remainder. So E(c + h)
//! is the sum of Σ_{i, j ≤ p} `E_ij`(c)·`h_u^i`·`h_v^j`/(i!·j!), of
//! Σ_{i ≤ p} `E_{i,p+1}`(`c_u`, η)·`h_u^i`·`h_v^{p+1}`/(i!·(p+1)!) and of
//! `E_{p+1,0}`(ξ, v)·`h_u^{p+1}`/(p+1)!. The surface is of degree p in each direction, so its
//! partials of order p + 1 in u or in v vanish and the remainders are the kind's partials alone,
//! with no difference of two enclosures to lose the cancellation. The kind's partials at c, along
//! the line u = `c_u` and over B are its certified enclosures ([`ParamSurface::partials_over`]),
//! the surface's at c are its Bernstein patch's partial forms (piece G59), and the sum is
//! evaluated over h ∈ B − c in interval arithmetic; the largest norm over the boxes is the
//! certified error.
//! The boxes are the spans cut at the sample parameters, one sampling step wide each (averaged
//! knots make the end spans two steps wide, and the remainders grow with the fourth power of a
//! box's width).
//!
//! Guarantees:
//!
//! - **Certified.** |O(u, v) − N(u, v)| is at most the deviation returned at every (u, v) of the
//!   domain: on each span the remainder form holds because both are smooth inside it (no span
//!   meets a break of the kind inside), every term is an outward-rounded enclosure, and knot
//!   removal adds no more than its own certified change (the triangle inequality; a row's change
//!   bounds the net's, since the other direction's basis functions are a partition of unity).
//! - **Within the tolerance, or refused.** Asked for a tolerance, the result is within it or the
//!   call fails with [`NurbsError::OutsideTolerance`]; without one, the target is the fit bound
//!   `ε_fit` (`geom_num::ladder::FIT`, 64 grid steps: chapter 13 certifies a procedural kind's cache
//!   within it), and the deviation returned is what the densest sampling certified when the
//!   target is not reached: honest, never assumed.
//! - **Same parameter.** The surface at (u, v) is within the deviation of the kind at (u, v), so
//!   both maps are `ParamMap::Same`.
//! - **Deterministic.** Fixed sampling, fixed orders.
//!
//! Method: L. Piegl and W. Tiller, "Computing offsets of NURBS curves and surfaces",
//! Computer-Aided Design 31(2) (1999), 147–156,
//! <https://doi.org/10.1016/S0010-4485(98)00066-9> (sample, interpolate, remove knots), with The
//! NURBS Book's A9.4 and A5.8 (2nd ed., <https://doi.org/10.1007/978-3-642-59223-2>) as
//! `geom-nurbs` implements them; the error certified instead of estimated from samples, by
//! Taylor's theorem in several variables with the Lagrange remainder in interval arithmetic
//! (R. E. Moore, R. B. Kearfott and M. J. Cloud, *Introduction to Interval Analysis*, SIAM (2009),
//! <https://doi.org/10.1137/1.9780898717716>, on Taylor forms).
//!
//! Cost: per try, (cells·n + 1)² samples and as many interpolation solves' worth of O(n) work,
//! plus one certificate per span (the kind's partials to order 6 at a point, 7 along a line and 4
//! over a box, and O(p²) interval operations to sum them); at most four tries (n = 4 to 32); knot
//! removal O(knots · rows · n).

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the method's: n the samples per cell, u and v the parameters, c a centre, h the step from it, i and j orders, e the error"
)]

use alloc::vec::Vec;

use geom_curve::tight::{self, point};
use geom_curve::{CurveError, ParamMap};
use geom_num::interval::Interval;
use geom_num::ladder;
use geom_nurbs::{
    Basis, BernsteinPatch, Changed, Curve, Direction, FitVersion, Limit, NurbsError, Surface,
};
use geom_run::WorkMeter;

use crate::nurbs::{Where, patch_partials};
use crate::param::{ParamBox, ParamSurface, SurfaceForm, partial_index};

/// The cache's degree in each direction: bicubic.
const DEGREE: usize = 3;

/// Samples per cell and direction on the first try.
const FIRST: usize = 4;

/// Samples per cell and direction on the last try.
const MOST: usize = 32;

/// The sides every sample and certificate evaluates from: the spans never straddle a break, so a
/// side matters only at the domain's end, where it is the only one.
const SIDES: (Limit, Limit) = (Limit::FromRight, Limit::FromRight);

/// The kind's NURBS form on the same parameters (module documentation): within `tolerance` or
/// refused when one is given, else to the default target.
///
/// # Errors
///
/// [`NurbsError::Tolerance`] for a tolerance that is not positive and finite;
/// [`NurbsError::OutsideTolerance`] if a tolerance given cannot be certified; [`CurveError::Unbounded`]
/// for an unbounded domain, or a partial that cannot be bounded even at the densest sampling (a
/// point where the kind's normal vanishes); as the kind's evaluations and `geom-nurbs`'s
/// interpolation and knot removal.
pub(crate) fn same_parameter<K: ParamSurface>(
    kind: &K,
    tolerance: Option<f64>,
    meter: &mut WorkMeter<'_>,
) -> Result<Changed<SurfaceForm>, CurveError> {
    if let Some(t) = tolerance
        && !(t.is_finite() && t > 0.0)
    {
        return Err(NurbsError::Tolerance.into());
    }
    let domain = kind.domain();
    let ((a, b), (c, d)) = domain;
    if ![a, b, c, d].iter().all(|x| x.is_finite()) {
        return Err(CurveError::Unbounded);
    }
    // Without a tolerance, the fit bound: chapter 13 certifies a procedural kind's cache within
    // ε_fit, and coordinates are in grid steps.
    let target = if let Some(t) = tolerance {
        t
    } else {
        f64::from(u32::try_from(ladder::FIT.0).map_err(|_| CurveError::Defect)?)
    };
    let (bu, bv) = kind.breaks();
    let mut n = FIRST;
    let mut last = f64::INFINITY;
    let (surface, error) = loop {
        let (u, basis_u) = grid((a, b), &bu, n)?;
        let (v, basis_v) = grid((c, d), &bv, n)?;
        let s = interpolate(kind, (&u, &v), (basis_u, basis_v), meter)?;
        let e = certify(kind, &s, (&u, &v), meter)?;
        // Twice the samples cut a smooth kind's bicubic error about sixteen times, and its
        // certificate at least a few times (on coarse grids the enclosures' own widths, which
        // fall with the boxes', dominate it); where the certificate does not fall by a quarter,
        // it has met rounding or a jump of the kind, and more samples would not help.
        if e <= target * 0.5 || n >= MOST || e > last * 0.75 {
            break (s, e);
        }
        last = e;
        n = n.checked_mul(2).ok_or(CurveError::Defect)?;
    };
    if !error.is_finite() {
        return Err(CurveError::Unbounded);
    }
    let form = |surface: Surface<3>, deviation: f64| Changed {
        value: SurfaceForm {
            surface,
            u: ParamMap::Same,
            v: ParamMap::Same,
        },
        deviation,
    };
    if error > target {
        // The densest sampling certified no better: refused when asked for, else reported.
        return match tolerance {
            Some(_) => Err(NurbsError::OutsideTolerance.into()),
            None => Ok(form(surface, error)),
        };
    }
    let budget = tight::sub(point(target)?, point(error)?)?.lo();
    let (surface, removed) = remove_knots(surface, budget, meter)?;
    // error + removed ≤ error + (target − error) = target, and rounding the sum up to a double
    // cannot pass the double target: within it.
    Ok(form(
        surface,
        tight::add(point(error)?, point(removed)?)?.hi(),
    ))
}

/// The parameters of one direction (each cell between breaks cut into `n` ≥ 4 equal steps, the
/// cells' shared ends once) and its basis: in each cell the knots that averaging (The NURBS Book,
/// equation 9.8) gives equally spaced parameters, the p-th to the (n − p)-th parameter
/// themselves, so no span is a rounding's width; the cells joined with each inner break repeated
/// `DEGREE` times.
fn grid((lo, hi): (f64, f64), breaks: &[f64], n: usize) -> Result<(Vec<f64>, Basis), CurveError> {
    // A kind's breaks lie strictly inside its domain (`ParamSurface::breaks`).
    let mut ends = alloc::vec![lo];
    ends.extend_from_slice(breaks);
    ends.push(hi);
    let count = u32::try_from(n).map_err(|_| CurveError::Defect)?;
    // Averaging p = 3 equally spaced parameters gives the middle one: knot j + p is t_(j+1).
    let first = DEGREE.checked_sub(1).ok_or(CurveError::Defect)?;
    let last = n.checked_sub(first).ok_or(CurveError::Defect)?;
    let mut params = alloc::vec![lo];
    let mut knots = alloc::vec![lo; DEGREE.checked_add(1).ok_or(CurveError::Defect)?];
    for w in ends.windows(2) {
        let (Some(&x0), Some(&x1)) = (w.first(), w.get(1)) else {
            return Err(CurveError::Defect);
        };
        let mut cell = Vec::with_capacity(n.saturating_add(1));
        for i in 0..=count {
            cell.push(if i == count {
                x1
            } else {
                x0 + (x1 - x0) * (f64::from(i) / f64::from(count))
            });
        }
        knots.extend_from_slice(cell.get(first..=last).ok_or(CurveError::Defect)?);
        knots.extend(core::iter::repeat_n(x1, DEGREE));
        params.extend(cell.iter().skip(1));
    }
    knots.push(hi);
    Ok((params, Basis::new(DEGREE, knots)?))
}

/// The bicubic surface through the kind's points at the grid (A9.4): along u through each
/// column, then along v through each row of the result.
fn interpolate<K: ParamSurface>(
    kind: &K,
    (u, v): (&[f64], &[f64]),
    (basis_u, basis_v): (Basis, Basis),
    meter: &mut WorkMeter<'_>,
) -> Result<Surface<3>, CurveError> {
    let mut columns: Vec<Vec<[f64; 3]>> = Vec::with_capacity(v.len());
    for &y in v {
        let mut q = Vec::with_capacity(u.len());
        for &x in u {
            meter.charge(1)?;
            q.push(kind.point(x, y, SIDES)?);
        }
        let fit = Curve::<3>::interpolate_at(&q, u, basis_u.clone(), FitVersion::V1, meter)?;
        columns.push(fit.curve.homogeneous().to_vec());
    }
    let rows = basis_u.count();
    let mut net = Vec::with_capacity(rows.saturating_mul(basis_v.count()));
    for i in 0..rows {
        let mut q = Vec::with_capacity(v.len());
        for col in &columns {
            q.push(*col.get(i).ok_or(CurveError::Defect)?);
        }
        let fit = Curve::<3>::interpolate_at(&q, v, basis_v.clone(), FitVersion::V1, meter)?;
        net.extend_from_slice(fit.curve.homogeneous());
    }
    Ok(Surface::from_homogeneous(basis_u, basis_v, net, None)?)
}

/// [lo, hi] cut at the parameters strictly inside it.
fn cut_at((lo, hi): (f64, f64), params: &[f64]) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let mut start = lo;
    for &t in params.iter().filter(|&&t| lo < t && t < hi) {
        out.push((start, t));
        start = t;
    }
    out.push((start, hi));
    out
}

/// The certified largest distance between the kind and the surface: the largest of the spans'
/// certificates, each span cut at the sample parameters inside it (the averaged knots make the
/// end spans two steps wide), so every box certified is one step wide; infinite if some box is
/// too wide to bound the kind's partials (where the kind's normal may vanish over it).
fn certify<K: ParamSurface>(
    kind: &K,
    surface: &Surface<3>,
    (u, v): (&[f64], &[f64]),
    meter: &mut WorkMeter<'_>,
) -> Result<f64, CurveError> {
    let mut worst = 0.0_f64;
    for p in surface.bernstein(meter)? {
        let (pu, pv) = p.domain();
        for su in cut_at(pu, u) {
            for sv in cut_at(pv, v) {
                let piece = if (su, sv) == (pu, pv) {
                    p.clone()
                } else {
                    p.restrict(su, sv)?
                };
                // A box too wide to bound the kind's partials certifies nothing at this sampling.
                let e = match certify_span(kind, &piece, meter) {
                    Ok(e) => e,
                    Err(CurveError::Unbounded) => return Ok(f64::INFINITY),
                    Err(e) => return Err(e),
                };
                worst = tight::larger(worst, e);
            }
        }
    }
    Ok(worst)
}

/// The interval h ∈ [lo − c, hi − c], rounded outward.
fn step((lo, hi): (f64, f64), c: f64) -> Result<Interval, CurveError> {
    let l = tight::sub(point(lo)?, point(c)?)?.lo();
    let h = tight::sub(point(hi)?, point(c)?)?.hi();
    Ok(Interval::new(l, h)?)
}

/// 1, x, x², …, x^k.
fn powers(x: Interval, k: usize) -> Result<Vec<Interval>, CurveError> {
    let mut out = Vec::with_capacity(k.saturating_add(1));
    let mut p = point(1.0)?;
    for _ in 0..=k {
        out.push(p);
        p = tight::mul(p, x)?;
    }
    Ok(out)
}

/// i!·j!, exact (orders up to the degree and one above).
fn factorials(i: usize, j: usize) -> Result<Interval, CurveError> {
    let f = |n: usize| -> Result<f64, CurveError> {
        let mut x = 1.0;
        for k in 2..=n {
            x *= f64::from(u32::try_from(k).map_err(|_| CurveError::Defect)?);
        }
        Ok(x)
    };
    tight::mul(point(f(i)?)?, point(f(j)?)?)
}

/// The certified largest distance between the kind and the patch over the patch's domain
/// (module documentation): with p the degree in both directions, E(c + h) is
/// Σ_{i, j ≤ p} `E_ij`(c)·`h_u^i`·`h_v^j`/(i!·j!), plus Σ_{i ≤ p} `O_{i,p+1}`(`c_u`, η)·`h_u^i`·`h_v^{p+1}`/(i!·(p+1)!)
/// over the line u = `c_u`, plus `O_{p+1,0}`(ξ, v)·`h_u^{p+1}`/(p+1)! over the box: the surface's
/// partials of order p + 1 in either direction vanish, so the remainders are the kind's alone.
fn certify_span<K: ParamSurface>(
    kind: &K,
    patch: &BernsteinPatch<3>,
    meter: &mut WorkMeter<'_>,
) -> Result<f64, CurveError> {
    meter.charge(1)?;
    let region: ParamBox = patch.domain();
    let ((u0, u1), (v0, v1)) = region;
    let (cu, cv) = (f64::midpoint(u0, u1), f64::midpoint(v0, v1));
    let p = DEGREE;
    let next = p.checked_add(1).ok_or(CurveError::Defect)?;
    let both = p.checked_mul(2).ok_or(CurveError::Defect)?;
    let at = ((cu, cu), (cv, cv));
    let oc = kind.partials_over(at, SIDES, both, meter)?;
    let nc = patch_partials(patch, Where::Point(cu, cv), both, meter)?;
    let line = kind.partials_over(((cu, cu), (v0, v1)), SIDES, both.saturating_add(1), meter)?;
    let whole = kind.partials_over(region, SIDES, next, meter)?;
    let hu = powers(step((u0, u1), cu)?, next)?;
    let hv = powers(step((v0, v1), cv)?, next)?;
    let get = |x: &[[Interval; 3]], i: usize, j: usize| -> Result<[Interval; 3], CurveError> {
        x.get(partial_index(i, j)?)
            .copied()
            .ok_or(CurveError::Defect)
    };
    let pow = |h: &[Interval], i: usize| h.get(i).copied().ok_or(CurveError::Defect);
    let mut e = [point(0.0)?; 3];
    let mut add = |c: [Interval; 3], i: usize, j: usize| -> Result<(), CurveError> {
        let f = tight::div(tight::mul(pow(&hu, i)?, pow(&hv, j)?)?, factorials(i, j)?)?;
        for (acc, x) in e.iter_mut().zip(c) {
            *acc = tight::add(*acc, tight::mul(x, f)?)?;
        }
        Ok(())
    };
    for i in 0..=p {
        for j in 0..=p {
            let (o, n) = (get(&oc, i, j)?, get(&nc, i, j)?);
            let [o0, o1, o2] = o;
            let [n0, n1, n2] = n;
            add(
                [
                    tight::sub(o0, n0)?,
                    tight::sub(o1, n1)?,
                    tight::sub(o2, n2)?,
                ],
                i,
                j,
            )?;
        }
        add(get(&line, i, next)?, i, next)?;
    }
    add(get(&whole, next, 0)?, next, 0)?;
    Ok(tight::norm(&e)?.hi())
}

/// The surface with knots removed while the removals' certified changes fit in `budget`, and
/// the total change: each distinct interior knot in u, then in v, in increasing order.
fn remove_knots(
    surface: Surface<3>,
    budget: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<(Surface<3>, f64), CurveError> {
    let mut s = surface;
    let mut used = point(0.0)?;
    for dir in [Direction::U, Direction::V] {
        let basis = match dir {
            Direction::U => s.basis_u(),
            Direction::V => s.basis_v(),
        };
        let (lo, hi) = basis.domain();
        let mut knots: Vec<f64> = basis
            .knots()
            .iter()
            .copied()
            .filter(|&x| lo < x && x < hi)
            .collect();
        knots.dedup();
        for x in knots {
            let left = tight::sub(point(budget)?, used)?.lo();
            if left <= 0.0 {
                break;
            }
            if let Some((next, change)) = remove_in(&s, dir, x, left, meter)? {
                s = next;
                used = tight::add(used, point(change)?)?;
            }
        }
    }
    Ok((s, used.hi()))
}

/// The net's lines along `dir` as curves on that direction's basis: the columns of control
/// points for u, the rows for v.
fn lines(s: &Surface<3>, dir: Direction) -> Result<Vec<Curve<3>>, CurveError> {
    let (bu, bv) = (s.basis_u(), s.basis_v());
    let (rows, cols) = (bu.count(), bv.count());
    let net = s.homogeneous();
    let (basis, count, len) = match dir {
        Direction::U => (bu, cols, rows),
        Direction::V => (bv, rows, cols),
    };
    let mut out = Vec::with_capacity(count);
    for l in 0..count {
        let mut pts = Vec::with_capacity(len);
        for i in 0..len {
            let k = match dir {
                Direction::U => i.checked_mul(cols).and_then(|x| x.checked_add(l)),
                Direction::V => l.checked_mul(cols).and_then(|x| x.checked_add(i)),
            }
            .ok_or(CurveError::Defect)?;
            pts.push(*net.get(k).ok_or(CurveError::Defect)?);
        }
        out.push(Curve::from_homogeneous(basis.clone(), pts, None)?);
    }
    Ok(out)
}

/// The surface with the knot x removed in `dir` as many times as every line allows within
/// `tol`, and the largest line's certified change; `None` if some line allows none.
fn remove_in(
    s: &Surface<3>,
    dir: Direction,
    x: f64,
    tol: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<(Surface<3>, f64)>, CurveError> {
    let all = lines(s, dir)?;
    let basis = match dir {
        Direction::U => s.basis_u(),
        Direction::V => s.basis_v(),
    };
    let mut least = basis.multiplicity(x);
    for c in &all {
        let (_, t) = c.remove_knot(x, least, tol, meter)?;
        least = least.min(t);
        if least == 0 {
            return Ok(None);
        }
    }
    let mut removed = Vec::with_capacity(all.len());
    let mut change = 0.0_f64;
    for c in &all {
        let (r, t) = c.remove_knot(x, least, tol, meter)?;
        if t != least {
            return Ok(None);
        }
        change = tight::larger(change, r.deviation);
        removed.push(r.value);
    }
    let new_basis = removed
        .first()
        .map(|c| c.basis().clone())
        .ok_or(CurveError::Defect)?;
    if removed.iter().any(|c| *c.basis() != new_basis) {
        return Ok(None);
    }
    let len = new_basis.count();
    let (bu, bv) = (s.basis_u(), s.basis_v());
    let (rows, cols) = match dir {
        Direction::U => (len, bv.count()),
        Direction::V => (bu.count(), len),
    };
    let mut net = Vec::with_capacity(rows.saturating_mul(cols));
    for i in 0..rows {
        for j in 0..cols {
            let (l, k) = match dir {
                Direction::U => (j, i),
                Direction::V => (i, j),
            };
            let line = removed.get(l).ok_or(CurveError::Defect)?;
            net.push(*line.homogeneous().get(k).ok_or(CurveError::Defect)?);
        }
    }
    let surface = match dir {
        Direction::U => Surface::from_homogeneous(new_basis, bv.clone(), net, None)?,
        Direction::V => Surface::from_homogeneous(bu.clone(), new_basis, net, None)?,
    };
    Ok(Some((surface, change)))
}
