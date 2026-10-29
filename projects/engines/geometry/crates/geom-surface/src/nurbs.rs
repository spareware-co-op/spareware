//! The NURBS surface kind: `geom-nurbs`'s [`Surface`] under the kind contract.
//!
//! What it does: implements [`ParamSurface`] for [`Surface`]. Points and partials are A3.5, A3.6
//! and A4.4 (`geom-nurbs`). Enclosures and bounds come from the surface's Bernstein patches in
//! interval arithmetic (piece G59): the patch holding (u, v), or each patch meeting the box cut
//! down to it, its box from the convex hull; partials from the patch's partial forms, and for a
//! rational surface `S_d` = (`A_d`·W − A·`W_d`) / W². The normal bound is the hull of the normal
//! patches (`Surface::normal_patches`, collapsed edges divided out) cut down to the box. Conversion
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
use geom_curve::{CurveError, DerivativeBound};
use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_num::real::Real;
use geom_nurbs::{Basis, BernsteinPatch, Bounds, Changed, Direction, Limit, Surface};
use geom_run::WorkMeter;

use crate::param::{ParamBox, ParamSurface, SurfaceKind};

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

/// Control points and their weights, if any.
type Net = (Vec<[f64; 3]>, Option<Vec<f64>>);

/// The union of two boxes, the first absent before any.
fn union(a: Option<Bounds<3>>, b: Bounds<3>) -> Result<Bounds<3>, CurveError> {
    match a {
        None => Ok(b),
        Some(a) => Ok(a.union(&b)?),
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

    fn to_nurbs(&self, _meter: &mut WorkMeter<'_>) -> Result<Changed<Surface<3>>, CurveError> {
        Ok(Changed {
            value: self.clone(),
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
