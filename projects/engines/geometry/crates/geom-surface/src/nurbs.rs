//! The NURBS surface kind: `geom-nurbs`'s [`Surface`] under the kind contract.
//!
//! What it does: implements [`ParamSurface`] for [`Surface`]. Points and partials are A3.5, A3.6
//! and A4.4 (`geom-nurbs`). Enclosures and bounds come from the surface's Bernstein patches in
//! interval arithmetic (piece G59): the patch holding (u, v), or each patch meeting the box cut
//! down to it, its box from the convex hull; partials from the patch's partial forms, and for a
//! rational surface `S_d` = (`A_d`·W − A·`W_d`) / W². The normal bound is the hull of the normal
//! patches (`Surface::normal_patches`, collapsed edges divided out) cut down to the box. Partials
//! of any order are the patch's repeated partial forms, at the point or bounded by their hulls,
//! and for a rational surface the jet quotient A/W (`crate::jet`). Conversion
//! to NURBS is the surface itself. A split inserts the parameter until its multiplicity is the
//! degree in that direction and parts the net; a reversal negates and reverses one knot vector;
//! a transform maps the control points with the weights kept.
//!
//! Guarantees:
//!
//! - **Certified.** Every enclosure and bound holds the exact value; a split's deviation is the
//!   certified rounding of the knot insertion; a transform's the largest certified error of a
//!   control point's image (a surface point is a weighted average of its control points).
//! - **Exact where the operation is.** Conversion and reversal change no bit; a grid transform of
//!   control points whose images are exact reports 0.
//! - **Refused outside the domain.** Never clamped.
//!
//! Method: The NURBS Book, 2nd ed. (<https://doi.org/10.1007/978-3-642-59223-2>): A3.5, A3.6,
//! A4.4, A5.3, de Casteljau subdivision and affine invariance, on `geom-nurbs` (G56, G59) and the
//! normal patch of Sederberg and Meyers (<https://doi.org/10.1016/0167-8396(88)90029-5>).
//!
//! Cost: a bound or enclosure runs Bezier extraction in intervals, O(`s_u`·`s_v`·(p + q)·p·q) for
//! s spans, plus O(p·q·(p + q)) per patch cut; the normal bound three products per patch; a split
//! O(n·m); a reversal or transform O(n·m).

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the book's: u, v and t for parameters, p for a degree, w for weights, a and b for a domain"
)]

use alloc::vec::Vec;

use geom_curve::affine::Entries;
use geom_curve::{CurveError, DerivativeBound, ParamMap};
use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_num::real::Real;
use geom_nurbs::{Basis, BernsteinPatch, Bounds, Changed, Direction, Limit, Surface};
use geom_run::WorkMeter;

use crate::jet::Jet;
use crate::param::{
    ParamBox, ParamSurface, SurfaceForm, SurfaceKind, centred, offset_from, partial_index,
};

/// A box inside the domain, each side ordered and finite.
fn check_box(((u0, u1), (v0, v1)): ParamBox, ((a, b), (c, d)): ParamBox) -> Result<(), CurveError> {
    let ok = |lo: f64, hi: f64, x: f64, y: f64| {
        lo.is_finite() && hi.is_finite() && x <= lo && lo <= hi && hi <= y
    };
    if ok(u0, u1, a, b) && ok(v0, v1, c, d) {
        Ok(())
    } else {
        Err(CurveError::SpanOutside)
    }
}

/// Whether [lo, hi] meets [a, b] in more than a point (or, for lo = hi, at all).
fn meets((lo, hi): (f64, f64), (a, b): (f64, f64)) -> bool {
    if lo == hi {
        a <= lo && lo <= b
    } else {
        a < hi && lo < b
    }
}

/// The part of a region inside a patch's domain, a direction of one value kept as it is.
fn clipped(((u0, u1), (v0, v1)): ParamBox, ((a, b), (c, d)): ParamBox) -> ParamBox {
    let cut = |lo: f64, hi: f64, x: f64, y: f64| {
        if lo == hi {
            (lo, hi)
        } else {
            (lo.clamp(x, y), hi.clamp(x, y))
        }
    };
    (cut(u0, u1, a, b), cut(v0, v1, c, d))
}

/// The part of [lo, hi] inside [a, b], widened to the whole of [a, b] where it is one value.
fn clip((lo, hi): (f64, f64), (a, b): (f64, f64)) -> (f64, f64) {
    if lo == hi {
        return (a, b);
    }
    (lo.clamp(a, b), hi.clamp(a, b))
}

/// The patch cut down to the part of `region` it covers.
fn restrict<const D: usize>(
    patch: &BernsteinPatch<D>,
    (ru, rv): ParamBox,
) -> Result<BernsteinPatch<D>, CurveError> {
    let (du, dv) = patch.domain();
    let (u, v) = (clip(ru, du), clip(rv, dv));
    if (u, v) == (du, dv) {
        return Ok(patch.clone());
    }
    Ok(patch.restrict(u, v)?)
}

/// The patch along its iso-line at t in `dir` (t inside the patch's domain): the coefficients of
/// its edge after a cut at t, as a patch of degree 0 in `dir` on the same domain.
fn iso<const D: usize>(
    patch: &BernsteinPatch<D>,
    dir: Direction,
    t: f64,
) -> Result<BernsteinPatch<D>, CurveError> {
    let (du, dv) = patch.domain();
    let (lo, _) = match dir {
        Direction::U => du,
        Direction::V => dv,
    };
    let cut = if t > lo {
        match dir {
            Direction::U => patch.restrict((lo, t), dv)?,
            Direction::V => patch.restrict(du, (lo, t))?,
        }
    } else {
        patch.clone()
    };
    let (p, q) = cut.degrees();
    let mut points = Vec::new();
    let mut weights = Vec::new();
    for i in 0..=p {
        for j in 0..=q {
            // The edge at t: the last row (or column) of the cut, or the first where t is the start.
            let on = match dir {
                Direction::U => i == if t > lo { p } else { 0 },
                Direction::V => j == if t > lo { q } else { 0 },
            };
            if !on {
                continue;
            }
            let k = i
                .checked_mul(q.checked_add(1).ok_or(CurveError::Defect)?)
                .and_then(|x| x.checked_add(j))
                .ok_or(CurveError::Defect)?;
            points.push(*cut.points().get(k).ok_or(CurveError::Defect)?);
            if let Some(w) = cut.weights() {
                weights.push(*w.get(k).ok_or(CurveError::Defect)?);
            }
        }
    }
    let degrees = match dir {
        Direction::U => (0, q),
        Direction::V => (p, 0),
    };
    let w = cut.is_rational().then_some(weights);
    Ok(BernsteinPatch::new((du, dv), degrees, points, w)?)
}

/// The patch cut down to the part of `region` it covers, a range of one value cut to the iso-line
/// there (`iso`).
fn restrict_to<const D: usize>(
    patch: &BernsteinPatch<D>,
    region: ParamBox,
) -> Result<BernsteinPatch<D>, CurveError> {
    let (du, dv) = patch.domain();
    let mut p = patch.clone();
    let mut wide = region;
    if region.0.0 == region.0.1 {
        p = iso(&p, Direction::U, region.0.0)?;
        wide.0 = du;
    }
    if region.1.0 == region.1.1 {
        p = iso(&p, Direction::V, region.1.0)?;
        wide.1 = dv;
    }
    restrict(&p, wide)
}

/// Control points and their weights, if any.
type Net = (Vec<[f64; 3]>, Option<Vec<f64>>);

/// The union of two boxes, the first absent before any.
fn union(a: Option<Bounds<3>>, b: Bounds<3>) -> Result<Bounds<3>, CurveError> {
    match a {
        None => Ok(b),
        Some(a) => Ok(a.union(&b)?),
    }
}

/// The smallest interval holding a and b.
fn join(a: Interval, b: Interval) -> Result<Interval, CurveError> {
    let lo = if b.lo() < a.lo() { b.lo() } else { a.lo() };
    let hi = if b.hi() > a.hi() { b.hi() } else { a.hi() };
    Ok(Interval::new(lo, hi)?)
}

/// Each partial's interval joined with the other list's.
pub(crate) fn join_partials(
    a: &[[Interval; 3]],
    b: &[[Interval; 3]],
) -> Result<Vec<[Interval; 3]>, CurveError> {
    let mut out = Vec::with_capacity(a.len());
    for (x, y) in a.iter().zip(b) {
        let [x0, x1, x2] = *x;
        let [y0, y1, y2] = *y;
        out.push([join(x0, y0)?, join(x1, y1)?, join(x2, y2)?]);
    }
    Ok(out)
}

/// The partial forms ∂^{i+j}P/∂u^i∂v^j of a polynomial patch for i + j ≤ `order`, listed by
/// `partial_index`; `None` for a form above the patch's degree in a direction, which is zero.
fn partial_forms<const D: usize>(
    patch: &BernsteinPatch<D>,
    order: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Option<BernsteinPatch<D>>>, CurveError> {
    let (p, q) = patch.degrees();
    let mut table: Vec<Vec<BernsteinPatch<D>>> = Vec::with_capacity(p.saturating_add(1));
    let mut along_u = patch.clone();
    for i in 0..=p.min(order) {
        if i > 0 {
            along_u = along_u.partial(Direction::U, meter)?;
        }
        let top = q.min(order.saturating_sub(i));
        let mut row = Vec::with_capacity(top.saturating_add(1));
        let mut f = along_u.clone();
        for j in 0..=top {
            if j > 0 {
                f = f.partial(Direction::V, meter)?;
            }
            row.push(f.clone());
        }
        table.push(row);
    }
    let mut out = Vec::with_capacity(partial_index(order.saturating_add(1), 0)?);
    for n in 0..=order {
        for j in 0..=n {
            let i = n.checked_sub(j).ok_or(CurveError::Defect)?;
            out.push(table.get(i).and_then(|r| r.get(j)).cloned());
        }
    }
    Ok(out)
}

/// Where a patch's partials are wanted: at a point, or at every point of a box inside its domain
/// (a range of one value in a direction is the iso-line there).
#[derive(Clone, Copy)]
pub(crate) enum Where {
    /// At (u, v).
    Point(f64, f64),
    /// Over the box.
    Over(ParamBox),
}

/// A polynomial patch's value at the point, or its hull over the box (`restrict_to`).
fn value<const D: usize>(
    patch: &BernsteinPatch<D>,
    at: Where,
    meter: &mut WorkMeter<'_>,
) -> Result<[Interval; D], CurveError> {
    let b = match at {
        Where::Point(u, v) => {
            let (x, _) = patch.value_at(u, v)?;
            return Ok(x);
        }
        Where::Over(region) => restrict_to(patch, region)?.bound(meter)?,
    };
    let mut out = [Interval::point(0.0)?; D];
    for (o, (&l, &h)) in out.iter_mut().zip(b.lo.iter().zip(&b.hi)) {
        *o = Interval::new(l, h)?;
    }
    Ok(out)
}

/// The partials of the patch's surface to `order`, listed by `partial_index`, at the point or
/// over the box (`Where`): a polynomial patch's are its partial forms' values; a rational
/// patch's are those of A/W, the quotient of its numerator's and weight's jets.
///
/// # Errors
///
/// [`CurveError::Unbounded`] if the weight may be zero (never, for positive weights); as the
/// patch operations.
pub(crate) fn patch_partials(
    patch: &BernsteinPatch<3>,
    at: Where,
    order: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<[Interval; 3]>, CurveError> {
    let zero = Interval::point(0.0)?;
    let a = partial_forms(&patch.numerator(), order, meter)?;
    let mut av = Vec::with_capacity(a.len());
    for f in &a {
        av.push(match f {
            Some(f) => value(f, at, meter)?,
            None => [zero; 3],
        });
    }
    let Some(w) = patch.weight_function() else {
        return Ok(av);
    };
    let mut wv = Vec::with_capacity(a.len());
    for f in &partial_forms(&w, order, meter)? {
        wv.push(match f {
            Some(f) => {
                let [x] = value(f, at, meter)?;
                x
            }
            None => zero,
        });
    }
    let wj = Jet::from_partials(&wv, order)?;
    let mut out = alloc::vec![[Interval::point(0.0)?; 3]; av.len()];
    for d in 0..3 {
        let col: Vec<Interval> = av
            .iter()
            .map(|x| x.get(d).copied().ok_or(CurveError::Defect))
            .collect::<Result<_, _>>()?;
        let q = Jet::from_partials(&col, order)?.div(&wj)?.partials()?;
        for (o, x) in out.iter_mut().zip(q) {
            if let Some(slot) = o.get_mut(d) {
                *slot = x;
            }
        }
    }
    Ok(out)
}

/// Whether a parameter range takes the patch range `d`: a point by `holds` from `side`, a span by
/// `meets`.
fn takes(r: (f64, f64), d: (f64, f64), side: Limit, whole: (f64, f64)) -> bool {
    if r.0 == r.1 {
        holds(r.0, d, side, d.0 == whole.0, d.1 == whole.1)
    } else {
        meets(r, d)
    }
}

/// Whether t lies in [a, b] from `side`, the ends included as the only side there.
fn holds(t: f64, (a, b): (f64, f64), side: Limit, first: bool, last: bool) -> bool {
    match side {
        Limit::FromRight => {
            let at_end = last && t == b;
            a <= t && (t < b || at_end)
        }
        Limit::FromLeft => {
            let at_start = first && t == a;
            (a < t || at_start) && t <= b
        }
    }
}

impl ParamSurface for Surface<3> {
    fn domain(&self) -> ParamBox {
        (self.basis_u().domain(), self.basis_v().domain())
    }

    fn point(&self, u: f64, v: f64, sides: (Limit, Limit)) -> Result<[f64; 3], CurveError> {
        Ok(Surface::point(self, u, v, sides)?)
    }

    fn partials(&self, u: f64, v: f64, sides: (Limit, Limit)) -> Result<[[f64; 3]; 3], CurveError> {
        let mut out = [[0.0; 3]; 4];
        self.derivs(u, v, sides, (1, 1), &mut out)?;
        let [s, sv, su, _] = out;
        Ok([s, su, sv])
    }

    fn enclose(
        &self,
        u: f64,
        v: f64,
        sides: (Limit, Limit),
        meter: &mut WorkMeter<'_>,
    ) -> Result<[[Interval; 3]; 3], CurveError> {
        let (du, dv) = ParamSurface::domain(self);
        if !(du.0 <= u && u <= du.1 && dv.0 <= v && v <= dv.1) {
            return Err(CurveError::ParameterOutside);
        }
        let patches = self.bernstein(meter)?;
        let patch = patches
            .iter()
            .find(|p| {
                let (pu, pv) = p.domain();
                holds(u, pu, sides.0, pu.0 == du.0, pu.1 == du.1)
                    && holds(v, pv, sides.1, pv.0 == dv.0, pv.1 == dv.1)
            })
            .ok_or(CurveError::ParameterOutside)?;
        let a = patch.numerator();
        let au = a.partial(Direction::U, meter)?;
        let av = a.partial(Direction::V, meter)?;
        let (s, _) = a.value_at(u, v)?;
        let (su, _) = au.value_at(u, v)?;
        let (sv, _) = av.value_at(u, v)?;
        let Some(w) = patch.weight_function() else {
            return Ok([s, su, sv]);
        };
        let wu = w.partial(Direction::U, meter)?;
        let wv = w.partial(Direction::V, meter)?;
        let ([w0], _): ([Interval; 1], Interval) = w.value_at(u, v)?;
        let ([w1u], _): ([Interval; 1], Interval) = wu.value_at(u, v)?;
        let ([w1v], _): ([Interval; 1], Interval) = wv.value_at(u, v)?;
        let w2: Interval = w0.mul(w0)?;
        let mut out = [[Interval::point(0.0)?; 3]; 3];
        for d in 0..3 {
            let sd: Interval = s.get(d).copied().ok_or(CurveError::Defect)?;
            let sud: Interval = su.get(d).copied().ok_or(CurveError::Defect)?;
            let svd: Interval = sv.get(d).copied().ok_or(CurveError::Defect)?;
            let p: Interval = sd.div(w0)?;
            let pu: Interval = sud.mul(w0)?.sub(sd.mul(w1u)?)?.div(w2)?;
            let pv: Interval = svd.mul(w0)?.sub(sd.mul(w1v)?)?.div(w2)?;
            for (row, val) in out.iter_mut().zip([p, pu, pv]) {
                if let Some(slot) = row.get_mut(d) {
                    *slot = val;
                }
            }
        }
        Ok(out)
    }

    fn bound(&self, region: ParamBox, meter: &mut WorkMeter<'_>) -> Result<Bounds<3>, CurveError> {
        check_box(region, ParamSurface::domain(self))?;
        let mut out = None;
        for p in self.bernstein(meter)? {
            let (pu, pv) = p.domain();
            if meets(region.0, pu) && meets(region.1, pv) {
                out = Some(union(out, restrict(&p, region)?.bound(meter)?)?);
            }
        }
        out.ok_or(CurveError::Defect)
    }

    fn derivative_bound(
        &self,
        region: ParamBox,
        dir: Direction,
        meter: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<3>, CurveError> {
        check_box(region, ParamSurface::domain(self))?;
        let mut out = None;
        for p in self.bernstein(meter)? {
            let (pu, pv) = p.domain();
            if meets(region.0, pu) && meets(region.1, pv) {
                let part = restrict(&p, region)?;
                out = Some(union(out, part.derivative_bound(dir, meter)?)?);
            }
        }
        out.map(DerivativeBound::Bounded).ok_or(CurveError::Defect)
    }

    fn partials_over(
        &self,
        region: ParamBox,
        sides: (Limit, Limit),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; 3]>, CurveError> {
        let (du, dv) = ParamSurface::domain(self);
        check_box(region, (du, dv))?;
        let ((u0, u1), (v0, v1)) = region;
        let point = u0 == u1 && v0 == v1;
        let line = u0 == u1 || v0 == v1;
        let mut out: Option<Vec<[Interval; 3]>> = None;
        for p in self.bernstein(meter)? {
            let (pu, pv) = p.domain();
            if !(takes(region.0, pu, sides.0, du) && takes(region.1, pv, sides.1, dv)) {
                continue;
            }
            // A box: the patch cut down to it, then differentiated; a line: each partial form cut
            // to the line, since the iso-line alone has no partials across it.
            let e = if point {
                patch_partials(&p, Where::Point(u0, v0), order, meter)?
            } else {
                let part = clipped(region, p.domain());
                let direct = |k: usize, m: &mut WorkMeter<'_>| {
                    if line {
                        patch_partials(&p, Where::Over(part), k, m)
                    } else {
                        let cut = restrict(&p, part)?;
                        patch_partials(&cut, Where::Over(cut.domain()), k, m)
                    }
                };
                if p.is_rational() {
                    // The quotient's jets widen over a box: the centred form keeps them close.
                    let ((u0, u1), (v0, v1)) = part;
                    let (cu, cv) = (f64::midpoint(u0, u1), f64::midpoint(v0, v1));
                    let up = order.checked_add(1).ok_or(CurveError::Defect)?;
                    let wide = direct(up, meter)?;
                    let at = patch_partials(&p, Where::Point(cu, cv), order, meter)?;
                    let steps = (offset_from((u0, u1), cu)?, offset_from((v0, v1), cv)?);
                    centred(&at, &wide, steps, order)?
                } else {
                    direct(order, meter)?
                }
            };
            out = Some(match out {
                None => e,
                Some(o) => join_partials(&o, &e)?,
            });
        }
        out.ok_or(CurveError::Defect)
    }

    fn breaks(&self) -> (Vec<f64>, Vec<f64>) {
        let inner = |b: &Basis| {
            let (lo, hi) = b.domain();
            let mut k: Vec<f64> = b
                .knots()
                .iter()
                .copied()
                .filter(|&x| lo < x && x < hi)
                .collect();
            k.dedup();
            k
        };
        (inner(self.basis_u()), inner(self.basis_v()))
    }

    fn normal_bound(
        &self,
        region: ParamBox,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Bounds<3>, CurveError> {
        check_box(region, ParamSurface::domain(self))?;
        let mut out = None;
        for n in self.normal_patches(meter)? {
            let (pu, pv) = n.domain();
            if meets(region.0, pu) && meets(region.1, pv) {
                out = Some(union(out, restrict(&n, region)?.bound(meter)?)?);
            }
        }
        out.ok_or(CurveError::Defect)
    }

    fn to_nurbs(&self, _meter: &mut WorkMeter<'_>) -> Result<Changed<SurfaceForm>, CurveError> {
        Ok(Changed {
            value: SurfaceForm {
                surface: self.clone(),
                u: ParamMap::Same,
                v: ParamMap::Same,
            },
            deviation: 0.0,
        })
    }

    fn split(
        &self,
        dir: Direction,
        t: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<SurfaceKind>, Changed<SurfaceKind>), CurveError> {
        let basis = match dir {
            Direction::U => self.basis_u(),
            Direction::V => self.basis_v(),
        };
        let (a, b) = basis.domain();
        if !(a < t && t < b) {
            return Err(CurveError::ParameterOutside);
        }
        let p = basis.degree();
        let s = basis.multiplicity(t);
        let inserted = self.insert_knot(dir, t, p.saturating_sub(s), meter)?;
        let c = &inserted.value;
        let (bu, bv) = (c.basis_u(), c.basis_v());
        let knots = match dir {
            Direction::U => bu.knots(),
            Direction::V => bv.knots(),
        };
        let k = knots
            .iter()
            .rposition(|&x| x == t)
            .ok_or(CurveError::Defect)?;
        let cut = k.checked_sub(p).ok_or(CurveError::Defect)?;
        let mut left_knots: Vec<f64> = knots.get(..=k).ok_or(CurveError::Defect)?.to_vec();
        left_knots.push(t);
        let mut right_knots = Vec::with_capacity(knots.len());
        right_knots.push(t);
        right_knots.extend_from_slice(
            knots
                .get(cut.checked_add(1).ok_or(CurveError::Defect)?..)
                .ok_or(CurveError::Defect)?,
        );
        let (rows, cols) = (bu.count(), bv.count());
        let pts = c.homogeneous();
        let w = c.weights();
        let side = |lo: usize, hi: usize| -> Result<Net, CurveError> {
            let mut out = Vec::new();
            let mut ws = Vec::new();
            for i in 0..rows {
                for j in 0..cols {
                    let idx = match dir {
                        Direction::U => i,
                        Direction::V => j,
                    };
                    if idx < lo || idx > hi {
                        continue;
                    }
                    let n = i
                        .checked_mul(cols)
                        .and_then(|x| x.checked_add(j))
                        .ok_or(CurveError::Defect)?;
                    out.push(*pts.get(n).ok_or(CurveError::Defect)?);
                    if let Some(w) = w {
                        ws.push(*w.get(n).ok_or(CurveError::Defect)?);
                    }
                }
            }
            Ok((out, w.map(|_| ws)))
        };
        let last = match dir {
            Direction::U => rows,
            Direction::V => cols,
        }
        .checked_sub(1)
        .ok_or(CurveError::Defect)?;
        let (lp, lw) = side(0, cut)?;
        let (rp, rw) = side(cut, last)?;
        let make = |kn: Vec<f64>,
                    pts: Vec<[f64; 3]>,
                    ws: Option<Vec<f64>>|
         -> Result<Surface<3>, CurveError> {
            let nb = Basis::new(p, kn)?;
            Ok(match dir {
                Direction::U => Surface::from_homogeneous(nb, bv.clone(), pts, ws)?,
                Direction::V => Surface::from_homogeneous(bu.clone(), nb, pts, ws)?,
            })
        };
        let d = inserted.deviation;
        Ok((
            Changed {
                value: SurfaceKind::Nurbs(make(left_knots, lp, lw)?),
                deviation: d,
            },
            Changed {
                value: SurfaceKind::Nurbs(make(right_knots, rp, rw)?),
                deviation: d,
            },
        ))
    }

    fn reversed(
        &self,
        dir: Direction,
        _meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        let (bu, bv) = (self.basis_u(), self.basis_v());
        let flip = |b: &Basis| -> Result<Basis, CurveError> {
            Ok(Basis::new(
                b.degree(),
                b.knots().iter().rev().map(|k| -k).collect(),
            )?)
        };
        let (rows, cols) = (bu.count(), bv.count());
        let src = |i: usize, j: usize| match dir {
            Direction::U => (rows.saturating_sub(1).saturating_sub(i), j),
            Direction::V => (i, cols.saturating_sub(1).saturating_sub(j)),
        };
        let mut pts = Vec::with_capacity(self.homogeneous().len());
        let mut ws = Vec::new();
        for i in 0..rows {
            for j in 0..cols {
                let (si, sj) = src(i, j);
                let n = si
                    .checked_mul(cols)
                    .and_then(|x| x.checked_add(sj))
                    .ok_or(CurveError::Defect)?;
                pts.push(*self.homogeneous().get(n).ok_or(CurveError::Defect)?);
                if let Some(w) = self.weights() {
                    ws.push(*w.get(n).ok_or(CurveError::Defect)?);
                }
            }
        }
        let (nu, nv) = match dir {
            Direction::U => (flip(bu)?, bv.clone()),
            Direction::V => (bu.clone(), flip(bv)?),
        };
        let s = Surface::from_homogeneous(nu, nv, pts, self.weights().map(|_| ws))?;
        Ok(Changed {
            value: SurfaceKind::Nurbs(s),
            deviation: 0.0,
        })
    }

    fn transformed(
        &self,
        transform: &TypedTransform,
        _meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        let e = Entries::of(transform)?;
        let mut pts = Vec::with_capacity(self.homogeneous().len());
        let mut deviation = 0.0_f64;
        for (i, h) in self.homogeneous().iter().enumerate() {
            let w = match self.weights() {
                Some(ws) => *ws.get(i).ok_or(CurveError::Defect)?,
                None => 1.0,
            };
            let (img, err) = e.image(*h, w)?;
            pts.push(img);
            deviation = Real::new(deviation)?.total_max(Real::new(err)?).get();
        }
        let s = Surface::from_homogeneous(
            self.basis_u().clone(),
            self.basis_v().clone(),
            pts,
            self.weights().map(<[f64]>::to_vec),
        )?;
        Ok(Changed {
            value: SurfaceKind::Nurbs(s),
            deviation,
        })
    }
}
