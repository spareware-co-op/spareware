//! The swept surface kinds: surface of revolution, linear extrusion and ruled surface, each built
//! on `geom-curve`'s trimmed curves and stored as itself.
//!
//! What it does: each kind is a placement c + L·X(u, v) of an unplaced shape X, with L a stored
//! 3 × 3 matrix (the identity when built), so every affine transform keeps the kind on the same
//! parameters (c′ = M·c + t, L′ = M·L):
//!
//! | Kind | X(u, v) | Parameters |
//! | --- | --- | --- |
//! | [`Revolution`] | A(v) + cos πu·B(v) + sin πu·C(v), with w = P(v) − a, A = (w·n̂)·n̂, B = w − A, C = n̂ × w | u the angle about the axis (a, n̂), at most one turn; v the profile's |
//! | [`Extrusion`] | P(u) + v·d | u the profile's; v along d |
//! | [`Ruled`] | P₁(u) + σ·v·(P₂(u) − P₁(u)) | u the two profiles' (one interval); v from P₁ (0) towards P₂ (1), σ = ±1 |
//!
//! Points and partials come from the profiles' certified enclosures and bounds, mapped linearly;
//! the normal bound is the cross product of the enclosed partials (a positive multiple of the
//! normal, itself). The NURBS form is exact where the profiles' are: a revolution is its profile's
//! form times a unit circle (The NURBS Book, A8.1), an extrusion its profile's form times a line,
//! a ruled surface its two profiles' forms made compatible (common degree and knots, A5.9 and
//! A5.4) and joined linearly; profiles whose forms are rational with different weights, or run on
//! different parameter maps, are first converted on the same parameter (`geom_curve::taylor`).
//!
//! Guarantees:
//!
//! - **Certified.** Every enclosure and bound holds the exact value of the surface the stored
//!   profiles and placement define.
//! - **Transforms keep the kind, on the same parameters**, with the rounding of c′ and L′ as the
//!   deviation: |Δc| + ‖ΔL‖·sup|X|.
//! - **Exact split and reversal**: a split trims a profile or cuts the linear direction; a reversal
//!   flips the axis (revolution, in u), reverses the profiles, or negates d or σ (in v).
//!
//! Method: the surfaces of ISO 10303-42 (`surface_of_revolution`, `surface_of_linear_extrusion`)
//! and the ruled surface; their NURBS construction from L. Piegl and W. Tiller, *The NURBS Book*,
//! 2nd ed., sections 8.3 to 8.5 (<https://doi.org/10.1007/978-3-642-59223-2>).
//!
//! Cost: an evaluation is the profiles' plus O(1); a bound O(1) profile bounds per piece; a
//! conversion O(m·n) for its control net, plus the profiles' conversions.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': c and L the placement, a the axis point, n the axis, w, A, B and C the revolution's vectors, d the direction, u and v the parameters"
)]

use alloc::vec::Vec;

use geom_curve::moved::Motion;
use geom_curve::tight::{self, point};
use geom_curve::{CurveError, CurveKind, DerivativeBound, Frame, ParamCurve, ParamMap, Trimmed};
use geom_math::TypedTransform;

use geom_num::interval::Interval;
use geom_nurbs::{Basis, Bounds, Changed, Curve, Direction, Limit, Surface};
use geom_run::WorkMeter;

use crate::analytic::{check_finite, check_turn, cuts};
use crate::param::{ParamBox, ParamSurface, SurfaceForm, SurfaceKind, partial_index};

/// π enclosed.
fn pi() -> Result<Interval, CurveError> {
    Ok(Interval::new(
        f64::from_bits(0x4009_21FB_5444_2D18),
        f64::from_bits(0x4009_21FB_5444_2D19),
    )?)
}

/// A vector of three intervals.
type V3 = [Interval; 3];

/// The placement c + L·x.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Placement {
    c: [f64; 3],
    l: [[f64; 3]; 3],
}

const IDENTITY: Placement = Placement {
    c: [0.0; 3],
    l: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
};

fn pts(v: [f64; 3]) -> Result<V3, CurveError> {
    let [a, b, c] = v;
    Ok([point(a)?, point(b)?, point(c)?])
}

fn add3(a: &V3, b: &V3) -> Result<V3, CurveError> {
    let [a0, a1, a2] = *a;
    let [b0, b1, b2] = *b;
    Ok([
        tight::add(a0, b0)?,
        tight::add(a1, b1)?,
        tight::add(a2, b2)?,
    ])
}

fn sub3(a: &V3, b: &V3) -> Result<V3, CurveError> {
    let [a0, a1, a2] = *a;
    let [b0, b1, b2] = *b;
    Ok([
        tight::sub(a0, b0)?,
        tight::sub(a1, b1)?,
        tight::sub(a2, b2)?,
    ])
}

fn scale3(s: Interval, a: &V3) -> Result<V3, CurveError> {
    let [a0, a1, a2] = *a;
    Ok([tight::mul(s, a0)?, tight::mul(s, a1)?, tight::mul(s, a2)?])
}

fn dot3(a: &V3, b: &V3) -> Result<Interval, CurveError> {
    let [a0, a1, a2] = *a;
    let [b0, b1, b2] = *b;
    let s = tight::add(tight::mul(a0, b0)?, tight::mul(a1, b1)?)?;
    tight::add(s, tight::mul(a2, b2)?)
}

fn cross3(a: &V3, b: &V3) -> Result<V3, CurveError> {
    let [a0, a1, a2] = *a;
    let [b0, b1, b2] = *b;
    Ok([
        tight::sub(tight::mul(a1, b2)?, tight::mul(a2, b1)?)?,
        tight::sub(tight::mul(a2, b0)?, tight::mul(a0, b2)?)?,
        tight::sub(tight::mul(a0, b1)?, tight::mul(a1, b0)?)?,
    ])
}

impl Placement {
    /// L·x (+ c for a point), enclosed, L and c taken as exact.
    fn apply(&self, x: &V3, is_point: bool) -> Result<V3, CurveError> {
        let zero = point(0.0)?;
        let mut out = if is_point { pts(self.c)? } else { [zero; 3] };
        for (o, row) in out.iter_mut().zip(&self.l) {
            let r = pts(*row)?;
            *o = tight::add(*o, dot3(&r, x)?)?;
        }
        Ok(out)
    }

    /// The placement under `m`, and an upper bound on the distance from the exact image over
    /// unplaced points of size at most `reach`.
    fn moved(&self, m: &Motion, reach: f64) -> Result<(Placement, f64), CurveError> {
        let c = m.point(self.c)?;
        let ce = m.enclosed(&pts(self.c)?, true)?;
        let mut l = [[0.0; 3]; 3];
        let mut dl = point(0.0)?;
        let [[l00, l01, l02], [l10, l11, l12], [l20, l21, l22]] = self.l;
        let cols: [[f64; 3]; 3] = [[l00, l10, l20], [l01, l11, l21], [l02, l12, l22]];
        for (j, col) in cols.iter().enumerate() {
            let col: [f64; 3] = *col;
            let img = m.vector(col)?;
            let exact = m.enclosed(&pts(col)?, false)?;
            for (row, (&x, e)) in l.iter_mut().zip(img.iter().zip(&exact)) {
                if let Some(slot) = row.get_mut(j) {
                    *slot = x;
                }
                let d = tight::sub(point(x)?, *e)?;
                dl = tight::add(dl, tight::square(d)?)?;
            }
        }
        let norm = tight::sqrt(Interval::new(tight::larger(dl.lo(), 0.0), dl.hi())?)?;
        let dev = tight::add(
            point(tight::distance_to(&c, &ce)?)?,
            tight::mul(norm, point(reach)?)?,
        )?;
        Ok((Placement { c, l }, dev.hi()))
    }

    /// An upper bound on ‖L‖ (its Frobenius norm).
    fn norm(&self) -> Result<f64, CurveError> {
        let mut v = Vec::with_capacity(9);
        for row in &self.l {
            for &x in row {
                v.push(point(x)?);
            }
        }
        Ok(tight::norm(&v)?.hi())
    }
}

/// Where a profile is evaluated: one parameter, or every parameter of a span.
#[derive(Clone, Copy)]
enum At {
    Point(f64, Limit),
    Span(f64, f64),
}

/// A trimmed profile's derivative of `order` at `at`, enclosed.
fn profile(
    p: &Trimmed<3>,
    at: At,
    order: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<V3, CurveError> {
    match at {
        At::Point(t, side) => p
            .enclose(t, side, order, meter)?
            .get(order)
            .copied()
            .ok_or(CurveError::Defect),
        At::Span(lo, hi) => {
            let b = if order == 0 {
                p.bound((lo, hi), meter)?
            } else {
                match p.derivative_bound((lo, hi), order, meter)? {
                    DerivativeBound::Bounded(b) => b,
                    DerivativeBound::Unbounded => return Err(CurveError::Unbounded),
                }
            };
            let [l0, l1, l2] = b.lo;
            let [h0, h1, h2] = b.hi;
            Ok([
                Interval::new(l0, h0)?,
                Interval::new(l1, h1)?,
                Interval::new(l2, h2)?,
            ])
        }
    }
}

/// The unplaced shape (module documentation).
#[derive(Clone, Debug, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "a ruled surface holds two profiles; the shapes are few and stored once per surface"
)]
enum Shape {
    Revolution {
        profile: Trimmed<3>,
        axis: [f64; 3],
        frame: Frame<3>,
    },
    Extrusion {
        profile: Trimmed<3>,
        direction: [f64; 3],
    },
    Ruled {
        first: Trimmed<3>,
        second: Trimmed<3>,
        sigma: f64,
    },
}

/// A swept surface: its shape, placement and domain.
#[derive(Clone, Debug, PartialEq)]
struct Swept {
    shape: Shape,
    place: Placement,
    domain: ParamBox,
}

/// The trimmed curve inside a kind that a trimmed curve's operation returned.
fn trimmed(k: CurveKind<3>) -> Result<Trimmed<3>, CurveError> {
    match k {
        CurveKind::Trimmed(t) => Ok(t),
        _ => Err(CurveError::Defect),
    }
}

fn check_point((u, v): (f64, f64), ((a, b), (c, d)): ParamBox) -> Result<(), CurveError> {
    if u.is_finite() && v.is_finite() && a <= u && u <= b && c <= v && v <= d {
        Ok(())
    } else {
        Err(CurveError::ParameterOutside)
    }
}

fn check_box(((u0, u1), (v0, v1)): ParamBox, ((a, b), (c, d)): ParamBox) -> Result<(), CurveError> {
    let ok = [u0, u1, v0, v1].iter().all(|x| x.is_finite())
        && a <= u0
        && u0 <= u1
        && u1 <= b
        && c <= v0
        && v0 <= v1
        && v1 <= d;
    if ok {
        Ok(())
    } else {
        Err(CurveError::SpanOutside)
    }
}

impl Swept {
    fn check(&self) -> Result<(), CurveError> {
        let ((u0, u1), (v0, v1)) = self.domain;
        check_finite((u0, u1))?;
        check_finite((v0, v1))?;
        match &self.shape {
            Shape::Revolution { profile, axis, .. } => {
                if !axis.iter().all(|x| x.is_finite()) {
                    return Err(CurveError::NotFinite);
                }
                check_turn((u0, u1))?;
                if profile.interval() != (v0, v1) {
                    return Err(CurveError::SpanOutside);
                }
            }
            Shape::Extrusion { profile, direction } => {
                if !direction.iter().all(|x| x.is_finite()) {
                    return Err(CurveError::NotFinite);
                }
                if direction.iter().all(|&x| x == 0.0) {
                    return Err(CurveError::Degenerate);
                }
                if profile.interval() != (u0, u1) {
                    return Err(CurveError::SpanOutside);
                }
            }
            Shape::Ruled { first, second, .. } => {
                if (first.interval(), second.interval()) != ((u0, u1), (u0, u1)) {
                    return Err(CurveError::SpanOutside);
                }
            }
        }
        Ok(())
    }

    /// The unplaced partial X^(i, j), of any orders, at u (a point or a span) and at the profile
    /// parameter `at` (or v over a span), enclosed: the formulas of the module table differentiated
    /// term by term (a revolution's cos πu and sin πu shift a quarter turn and gain π per order in
    /// u; X is linear in v for an extrusion and a ruled surface).
    fn x(
        &self,
        u: Interval,
        v: Interval,
        (i, j): (usize, usize),
        sides: (Limit, Limit),
        meter: &mut WorkMeter<'_>,
    ) -> Result<V3, CurveError> {
        let at = |t: Interval, side: Limit| {
            if t.lo() == t.hi() {
                At::Point(t.lo(), side)
            } else {
                At::Span(t.lo(), t.hi())
            }
        };
        let zero = point(0.0)?;
        match &self.shape {
            Shape::Revolution {
                profile: p,
                axis,
                frame,
            } => {
                let (_, e) = frame.unit_vectors()?;
                let [_, _, n] = e;
                let mut w = profile(p, at(v, sides.1), j, meter)?;
                if j == 0 {
                    w = sub3(&w, &pts(*axis)?)?;
                }
                let a = scale3(dot3(&w, &n)?, &n)?;
                let b = sub3(&w, &a)?;
                let c = cross3(&n, &w)?;
                // The i-th derivatives of cos πu and sin πu: π^i times the pair shifted i quarter
                // turns.
                let (cos, sin) = (u.cospi()?, u.sinpi()?);
                let (mut fc, mut fs) = match i & 3 {
                    0 => (cos, sin),
                    1 => (sin.neg(), cos),
                    2 => (cos.neg(), sin.neg()),
                    _ => (sin, cos.neg()),
                };
                let p = pi()?;
                for _ in 0..i {
                    fc = tight::mul(p, fc)?;
                    fs = tight::mul(p, fs)?;
                }
                let mut out = add3(&scale3(fc, &b)?, &scale3(fs, &c)?)?;
                if i == 0 {
                    out = add3(&out, &a)?;
                }
                Ok(out)
            }
            Shape::Extrusion {
                profile: p,
                direction,
            } => {
                let d = pts(*direction)?;
                match (i, j) {
                    (0, 0) => add3(&profile(p, at(u, sides.0), 0, meter)?, &scale3(v, &d)?),
                    (_, 0) => profile(p, at(u, sides.0), i, meter),
                    (0, 1) => Ok(d),
                    _ => Ok([zero; 3]),
                }
            }
            Shape::Ruled {
                first,
                second,
                sigma,
            } => {
                if j > 1 {
                    return Ok([zero; 3]);
                }
                let p1 = profile(first, at(u, sides.0), i, meter)?;
                let p2 = profile(second, at(u, sides.0), i, meter)?;
                let diff = scale3(point(*sigma)?, &sub3(&p2, &p1)?)?;
                if j == 1 {
                    return Ok(diff);
                }
                add3(&p1, &scale3(v, &diff)?)
            }
        }
    }

    /// The placed S^(i, j), enclosed.
    fn s(
        &self,
        u: Interval,
        v: Interval,
        order: (usize, usize),
        sides: (Limit, Limit),
        meter: &mut WorkMeter<'_>,
    ) -> Result<V3, CurveError> {
        let x = self.x(u, v, order, sides, meter)?;
        self.place.apply(&x, order == (0, 0))
    }

    /// The pieces of a box: eighths of a turn in a revolution's u, else whole.
    fn pieces(&self, region: ParamBox) -> Result<Vec<ParamBox>, CurveError> {
        let (u, v) = region;
        let revolution = matches!(self.shape, Shape::Revolution { .. });
        Ok(cuts(u, revolution)?.into_iter().map(|s| (s, v)).collect())
    }

    fn hull(
        &self,
        region: ParamBox,
        meter: &mut WorkMeter<'_>,
        f: impl Fn(&Swept, Interval, Interval, &mut WorkMeter<'_>) -> Result<V3, CurveError>,
    ) -> Result<Bounds<3>, CurveError> {
        check_box(region, self.domain)?;
        let mut out = Bounds {
            lo: [f64::INFINITY; 3],
            hi: [f64::NEG_INFINITY; 3],
        };
        for ((u0, u1), (v0, v1)) in self.pieces(region)? {
            meter.charge(1)?;
            let e = f(self, Interval::new(u0, u1)?, Interval::new(v0, v1)?, meter)?;
            for ((l, h), x) in out.lo.iter_mut().zip(out.hi.iter_mut()).zip(&e) {
                *l = tight::smaller(*l, x.lo());
                *h = tight::larger(*h, x.hi());
            }
        }
        Ok(out)
    }

    /// An upper bound on |X| over the domain.
    fn reach(&self, meter: &mut WorkMeter<'_>) -> Result<f64, CurveError> {
        let ((u0, u1), (v0, v1)) = self.domain;
        let sides = (Limit::FromRight, Limit::FromRight);
        let mut r: f64 = 0.0;
        for ((a, b), (c, d)) in self.pieces(((u0, u1), (v0, v1)))? {
            let x = self.x(
                Interval::new(a, b)?,
                Interval::new(c, d)?,
                (0, 0),
                sides,
                meter,
            )?;
            let m = tight::norm(&x)?.hi();
            r = tight::larger(r, m);
        }
        Ok(r)
    }

    /// Every placed partial to `order` over `region`, each the hull of its pieces, listed by
    /// `partial_index`.
    fn partials_over(
        &self,
        region: ParamBox,
        sides: (Limit, Limit),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<V3>, CurveError> {
        let mut out = Vec::with_capacity(partial_index(order.saturating_add(1), 0)?);
        for n in 0..=order {
            for j in 0..=n {
                let i = n.checked_sub(j).ok_or(CurveError::Defect)?;
                let b = self.hull(region, meter, |s, u, v, m| s.s(u, v, (i, j), sides, m))?;
                let [l0, l1, l2] = b.lo;
                let [h0, h1, h2] = b.hi;
                out.push([
                    Interval::new(l0, h0)?,
                    Interval::new(l1, h1)?,
                    Interval::new(l2, h2)?,
                ]);
            }
        }
        Ok(out)
    }

    /// The profiles' breaks strictly inside the domain, in the direction each runs.
    fn breaks(&self) -> (Vec<f64>, Vec<f64>) {
        let ((u0, u1), (v0, v1)) = self.domain;
        let inside = |mut b: Vec<f64>, lo: f64, hi: f64| {
            b.retain(|&x| lo < x && x < hi);
            b.sort_by(f64::total_cmp);
            b.dedup();
            b
        };
        match &self.shape {
            Shape::Revolution { profile, .. } => (Vec::new(), inside(profile.breaks(), v0, v1)),
            Shape::Extrusion { profile, .. } => (inside(profile.breaks(), u0, u1), Vec::new()),
            Shape::Ruled { first, second, .. } => {
                let mut b = first.breaks();
                b.extend(second.breaks());
                (inside(b, u0, u1), Vec::new())
            }
        }
    }

    fn with_shape(&self, shape: Shape, domain: ParamBox) -> Swept {
        Swept {
            shape,
            place: self.place,
            domain,
        }
    }

    fn split(
        &self,
        dir: Direction,
        t: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Swept, Swept), CurveError> {
        let ((u0, u1), (v0, v1)) = self.domain;
        let (lo, hi) = match dir {
            Direction::U => (u0, u1),
            Direction::V => (v0, v1),
        };
        if !(lo < t && t < hi) {
            return Err(CurveError::ParameterOutside);
        }
        let (dl, dr) = match dir {
            Direction::U => (((u0, t), (v0, v1)), ((t, u1), (v0, v1))),
            Direction::V => (((u0, u1), (v0, t)), ((u0, u1), (t, v1))),
        };
        let halves = |p: &Trimmed<3>,
                      meter: &mut WorkMeter<'_>|
         -> Result<(Trimmed<3>, Trimmed<3>), CurveError> {
            let (l, r) = p.split(t, meter)?;
            Ok((trimmed(l.value)?, trimmed(r.value)?))
        };
        let (left, right) = match (&self.shape, dir) {
            (
                Shape::Revolution {
                    profile,
                    axis,
                    frame,
                },
                Direction::V,
            ) => {
                let (pl, pr) = halves(profile, meter)?;
                (
                    Shape::Revolution {
                        profile: pl,
                        axis: *axis,
                        frame: *frame,
                    },
                    Shape::Revolution {
                        profile: pr,
                        axis: *axis,
                        frame: *frame,
                    },
                )
            }
            (Shape::Extrusion { profile, direction }, Direction::U) => {
                let (pl, pr) = halves(profile, meter)?;
                (
                    Shape::Extrusion {
                        profile: pl,
                        direction: *direction,
                    },
                    Shape::Extrusion {
                        profile: pr,
                        direction: *direction,
                    },
                )
            }
            (
                Shape::Ruled {
                    first,
                    second,
                    sigma,
                },
                Direction::U,
            ) => {
                let (fl, fr) = halves(first, meter)?;
                let (sl, sr) = halves(second, meter)?;
                (
                    Shape::Ruled {
                        first: fl,
                        second: sl,
                        sigma: *sigma,
                    },
                    Shape::Ruled {
                        first: fr,
                        second: sr,
                        sigma: *sigma,
                    },
                )
            }
            (s, _) => (s.clone(), s.clone()),
        };
        Ok((self.with_shape(left, dl), self.with_shape(right, dr)))
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one case per kind and direction, each a few lines"
    )]
    fn reversed(
        &self,
        dir: Direction,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Swept, f64), CurveError> {
        let ((u0, u1), (v0, v1)) = self.domain;
        let domain = match dir {
            Direction::U => ((-u1, -u0), (v0, v1)),
            Direction::V => ((u0, u1), (-v1, -v0)),
        };
        let rev =
            |p: &Trimmed<3>, meter: &mut WorkMeter<'_>| -> Result<(Trimmed<3>, f64), CurveError> {
                let r = p.reversed(meter)?;
                Ok((trimmed(r.value)?, r.deviation))
            };
        let (shape, dev) = match (&self.shape, dir) {
            (
                Shape::Revolution {
                    profile,
                    axis,
                    frame,
                },
                Direction::U,
            ) => (
                Shape::Revolution {
                    profile: profile.clone(),
                    axis: *axis,
                    frame: frame.reversed(),
                },
                0.0,
            ),
            (
                Shape::Revolution {
                    profile,
                    axis,
                    frame,
                },
                Direction::V,
            ) => {
                let (p, d) = rev(profile, meter)?;
                (
                    Shape::Revolution {
                        profile: p,
                        axis: *axis,
                        frame: *frame,
                    },
                    d,
                )
            }
            (Shape::Extrusion { profile, direction }, Direction::U) => {
                let (p, d) = rev(profile, meter)?;
                (
                    Shape::Extrusion {
                        profile: p,
                        direction: *direction,
                    },
                    d,
                )
            }
            (Shape::Extrusion { profile, direction }, Direction::V) => (
                Shape::Extrusion {
                    profile: profile.clone(),
                    direction: direction.map(|x| -x),
                },
                0.0,
            ),
            (
                Shape::Ruled {
                    first,
                    second,
                    sigma,
                },
                Direction::U,
            ) => {
                let (f, d1) = rev(first, meter)?;
                let (s, d2) = rev(second, meter)?;
                (
                    Shape::Ruled {
                        first: f,
                        second: s,
                        sigma: *sigma,
                    },
                    tight::larger(d1, d2),
                )
            }
            (
                Shape::Ruled {
                    first,
                    second,
                    sigma,
                },
                Direction::V,
            ) => (
                Shape::Ruled {
                    first: first.clone(),
                    second: second.clone(),
                    sigma: -sigma,
                },
                0.0,
            ),
        };
        // A profile's reversal moves the surface by at most ‖L‖ times its own deviation (0 for an
        // exact one).
        let dev = tight::mul(point(dev)?, point(self.place.norm()?)?)?.hi();
        Ok((self.with_shape(shape, domain), dev))
    }
}

/// A profile's NURBS form as Euclidean control points (enclosed), weights, basis and map.
struct Form1 {
    basis: Basis,
    points: Vec<V3>,
    weights: Vec<f64>,
    map: ParamMap,
    deviation: f64,
}

fn form1(c: &Curve<3>, map: ParamMap, deviation: f64) -> Result<Form1, CurveError> {
    let w: Vec<f64> = c
        .weights()
        .map_or_else(|| alloc::vec![1.0; c.homogeneous().len()], <[f64]>::to_vec);
    let mut points = Vec::with_capacity(w.len());
    for (h, &wi) in c.homogeneous().iter().zip(&w) {
        let [x, y, z] = *h;
        let wv = point(wi)?;
        points.push([
            tight::div(point(x)?, wv)?,
            tight::div(point(y)?, wv)?,
            tight::div(point(z)?, wv)?,
        ]);
    }
    Ok(Form1 {
        basis: c.basis().clone(),
        points,
        weights: w,
        map,
        deviation,
    })
}

/// A net of exact (enclosed) Euclidean points and weights, stored as doubles, with the certified
/// rounding bound of `geom-curve`'s conics: max |Pᵢ − Qᵢ| + 2ε/(1 − ε)·max |Qᵢ − Q₀|.
fn net(
    u: Basis,
    v: Basis,
    points: &[V3],
    weights: &[Interval],
) -> Result<(Surface<3>, f64), CurveError> {
    let one = point(1.0)?;
    let (mut moved, mut eps, mut reach) = (0.0, 0.0, 0.0);
    let origin = *points.first().ok_or(CurveError::Defect)?;
    let mut homs = Vec::with_capacity(points.len());
    let mut ws = Vec::with_capacity(points.len());
    for (p, &w) in points.iter().zip(weights) {
        let pd = p.map(tight::middle);
        let wd = tight::middle(w);
        let d = tight::distance_to(&pd, p)?;
        moved = tight::larger(moved, d);
        let r = tight::magnitude(tight::sub(tight::div(point(wd)?, w)?, one)?);
        eps = tight::larger(eps, r);
        let rr = tight::distance(p, &origin)?;
        reach = tight::larger(reach, rr);
        homs.push(pd.map(|x| x * wd));
        ws.push(wd);
    }
    let mut dev = point(moved)?;
    // The weights' term is exactly 0 when every stored weight is exact.
    if eps.is_nan() || eps >= 0.5 {
        return Err(CurveError::NotFinite);
    }
    let e = point(eps)?;
    let f = tight::div(tight::mul(point(2.0)?, e)?, tight::sub(one, e)?)?;
    dev = tight::add(dev, tight::mul(f, point(reach)?)?)?;
    Ok((Surface::from_homogeneous(u, v, homs, Some(ws))?, dev.hi()))
}

/// The unit circle's form over [a, b] half-turns: (cos, sin) control points, weights, basis, map.
fn circle(a: f64, b: f64, meter: &mut WorkMeter<'_>) -> Result<(Form1, f64), CurveError> {
    let c = geom_curve::Circle::<2>::new([0.0, 0.0], 1.0, Frame::standard()?, (a, b))?;
    let n = c.to_nurbs(meter)?;
    let cv = &n.value.curve;
    let w: Vec<f64> = cv
        .weights()
        .map_or_else(|| alloc::vec![1.0; cv.homogeneous().len()], <[f64]>::to_vec);
    let mut points = Vec::with_capacity(w.len());
    for (h, &wi) in cv.homogeneous().iter().zip(&w) {
        let [x, y] = *h;
        let wv = point(wi)?;
        points.push([
            tight::div(point(x)?, wv)?,
            tight::div(point(y)?, wv)?,
            point(0.0)?,
        ]);
    }
    Ok((
        Form1 {
            basis: cv.basis().clone(),
            points,
            weights: w,
            map: n.value.map,
            deviation: n.deviation,
        },
        n.deviation,
    ))
}

impl Swept {
    #[allow(
        clippy::too_many_lines,
        reason = "one construction per kind: the profile forms, the net, the deviation"
    )]
    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<SurfaceForm>, CurveError> {
        let ((u0, u1), (v0, v1)) = self.domain;
        let norm = self.place.norm()?;
        match &self.shape {
            Shape::Revolution {
                profile,
                axis,
                frame,
            } => {
                let pf = profile.to_nurbs(meter)?;
                let pf = form1(&pf.value.curve, pf.value.map, pf.deviation)?;
                let (cf, cdev) = circle(u0, u1, meter)?;
                let (_, e) = frame.unit_vectors()?;
                let [_, _, n] = e;
                let a = pts(*axis)?;
                let mut points = Vec::new();
                let mut weights = Vec::new();
                let mut radial: f64 = 0.0;
                for (cp, &cw) in cf.points.iter().zip(&cf.weights) {
                    let [ca, sa, _] = *cp;
                    for (pp, &pw) in pf.points.iter().zip(&pf.weights) {
                        let w = sub3(pp, &a)?;
                        let av = scale3(dot3(&w, &n)?, &n)?;
                        let bv = sub3(&w, &av)?;
                        let cvv = cross3(&n, &w)?;
                        let r = tight::norm(&bv)?.hi();
                        radial = tight::larger(radial, r);
                        let x = add3(&add3(&av, &scale3(ca, &bv)?)?, &scale3(sa, &cvv)?)?;
                        points.push(self.place.apply(&x, true)?);
                        weights.push(tight::mul(point(cw)?, point(pw)?)?);
                    }
                }
                let (surface, round) = net(cf.basis.clone(), pf.basis.clone(), &points, &weights)?;
                // The profile's deviation passes through the rotation unchanged, the circle's
                // scales with the radial distance (|B| = |C|), both then by ‖L‖.
                let inner = tight::add(
                    point(pf.deviation)?,
                    tight::mul(point(2.0 * cdev)?, point(radial)?)?,
                )?;
                let dev = tight::add(point(round)?, tight::mul(inner, point(norm)?)?)?;
                Ok(Changed {
                    value: SurfaceForm {
                        surface,
                        u: cf.map,
                        v: pf.map,
                    },
                    deviation: dev.hi(),
                })
            }
            Shape::Extrusion { profile, direction } => {
                let pf = profile.to_nurbs(meter)?;
                let pf = form1(&pf.value.curve, pf.value.map, pf.deviation)?;
                let d = pts(*direction)?;
                let mut points = Vec::new();
                let mut weights = Vec::new();
                for (pp, &pw) in pf.points.iter().zip(&pf.weights) {
                    for t in [v0, v1] {
                        let x = add3(pp, &scale3(point(t)?, &d)?)?;
                        points.push(self.place.apply(&x, true)?);
                        weights.push(point(pw)?);
                    }
                }
                let line = Basis::new(1, alloc::vec![v0, v0, v1, v1])?;
                let (surface, round) = net(pf.basis.clone(), line, &points, &weights)?;
                let dev = tight::add(
                    point(round)?,
                    tight::mul(point(pf.deviation)?, point(norm)?)?,
                )?;
                Ok(Changed {
                    value: SurfaceForm {
                        surface,
                        u: pf.map,
                        v: ParamMap::Same,
                    },
                    deviation: dev.hi(),
                })
            }
            Shape::Ruled {
                first,
                second,
                sigma,
            } => {
                let (f1, f2) = (first.to_nurbs(meter)?, second.to_nurbs(meter)?);
                // The forms as they are when they share a map and are polynomial and clamped (their
                // control points are then the ruling's); else Taylor forms on the same parameter.
                let compatible = f1.value.map == f2.value.map
                    && !f1.value.curve.is_rational()
                    && !f2.value.curve.is_rational()
                    && f1.value.curve.basis().is_clamped()
                    && f2.value.curve.basis().is_clamped();
                let (c1, c2, map, d1, d2) = if compatible {
                    (
                        f1.value.curve,
                        f2.value.curve,
                        f1.value.map,
                        f1.deviation,
                        f2.deviation,
                    )
                } else {
                    let g1 = geom_curve::taylor::same_parameter(first, 9, 0.125, meter)?;
                    let g2 = geom_curve::taylor::same_parameter(second, 9, 0.125, meter)?;
                    (
                        g1.value.curve,
                        g2.value.curve,
                        ParamMap::Same,
                        g1.deviation,
                        g2.deviation,
                    )
                };
                let (c1, c2, extra) = compatible_pair(c1, c2, meter)?;
                let a = form1(&c1, map.clone(), 0.0)?;
                let b = form1(&c2, map.clone(), 0.0)?;
                let s = point(*sigma)?;
                let mut points = Vec::new();
                let mut weights = Vec::new();
                for (p1, p2) in a.points.iter().zip(&b.points) {
                    let diff = scale3(s, &sub3(p2, p1)?)?;
                    for t in [v0, v1] {
                        let x = add3(p1, &scale3(point(t)?, &diff)?)?;
                        points.push(self.place.apply(&x, true)?);
                        weights.push(point(1.0)?);
                    }
                }
                let line = Basis::new(1, alloc::vec![v0, v0, v1, v1])?;
                let (surface, round) = net(a.basis.clone(), line, &points, &weights)?;
                // Each profile's deviation reaches the surface weighted by at most
                // max(|1 − σv|, |σv|) ≤ 1 + max|v|.
                let vmax = tight::larger(v0.abs(), v1.abs());
                let reach = tight::add(point(1.0)?, point(vmax)?)?;
                let inner = tight::mul(
                    tight::add(tight::add(point(d1)?, point(d2)?)?, point(extra)?)?,
                    reach,
                )?;
                let dev = tight::add(point(round)?, tight::mul(inner, point(norm)?)?)?;
                Ok(Changed {
                    value: SurfaceForm {
                        surface,
                        u: map,
                        v: ParamMap::Same,
                    },
                    deviation: dev.hi(),
                })
            }
        }
    }
}

/// Two non-rational curves on one domain raised to one degree and refined to one knot vector,
/// with the sum of the operations' deviations.
fn compatible_pair(
    a: Curve<3>,
    b: Curve<3>,
    meter: &mut WorkMeter<'_>,
) -> Result<(Curve<3>, Curve<3>, f64), CurveError> {
    let p = a.degree().max(b.degree());
    let raise = |c: Curve<3>, meter: &mut WorkMeter<'_>| -> Result<(Curve<3>, f64), CurveError> {
        if c.degree() < p {
            let e = c.elevate(p.saturating_sub(c.degree()), meter)?;
            Ok((e.value, e.deviation))
        } else {
            Ok((c, 0.0))
        }
    };
    let (a, da) = raise(a, meter)?;
    let (b, db) = raise(b, meter)?;
    let mut parts = alloc::vec![da, db];
    // The knots each lacks, interior only, with multiplicity.
    let missing = |x: &Curve<3>, y: &Curve<3>| -> Vec<f64> {
        let (kx, ky) = (x.basis().knots(), y.basis().knots());
        let (lo, hi) = x.basis().domain();
        let mut out: Vec<f64> = Vec::new();
        for &t in ky {
            if t > lo && t < hi {
                let have = kx.iter().filter(|&&k| k == t).count();
                let want = ky.iter().filter(|&&k| k == t).count();
                let already = out.iter().filter(|&&k| k == t).count();
                if have.saturating_add(already) < want {
                    out.push(t);
                }
            }
        }
        out
    };
    let (ma, mb) = (missing(&a, &b), missing(&b, &a));
    let a = if ma.is_empty() {
        a
    } else {
        let r = a.refine(&ma, meter)?;
        parts.push(r.deviation);
        r.value
    };
    let b = if mb.is_empty() {
        b
    } else {
        let r = b.refine(&mb, meter)?;
        parts.push(r.deviation);
        r.value
    };
    if a.basis().knots() != b.basis().knots() {
        return Err(CurveError::Defect);
    }
    // Their sum, rounded up.
    let mut dev = point(0.0)?;
    for d in parts {
        dev = tight::add(dev, point(d)?)?;
    }
    Ok((a, b, dev.hi()))
}

/// The `ParamSurface` implementation of a swept kind, written once for the three.
macro_rules! swept_surface {
    ($kind:ident, $variant:ident) => {
        impl $kind {
            /// The placement c + L·x the shape is moved by, (c, L) with L by rows: the identity
            /// until a transform moves the surface.
            #[must_use]
            pub fn placement(&self) -> ([f64; 3], [[f64; 3]; 3]) {
                (self.inner.place.c, self.inner.place.l)
            }

            /// The same shape placed by c + L·x instead (L by rows), as a transform leaves it:
            /// what a stored surface is rebuilt from.
            ///
            /// # Errors
            ///
            /// [`CurveError::NotFinite`] for a value that is not finite.
            pub fn placed(&self, c: [f64; 3], l: [[f64; 3]; 3]) -> Result<$kind, CurveError> {
                if !c.iter().chain(l.iter().flatten()).all(|x| x.is_finite()) {
                    return Err(CurveError::NotFinite);
                }
                Ok($kind {
                    inner: Swept {
                        place: Placement { c, l },
                        ..self.inner.clone()
                    },
                })
            }
        }

        impl ParamSurface for $kind {
            fn domain(&self) -> ParamBox {
                self.inner.domain
            }

            fn point(&self, u: f64, v: f64, sides: (Limit, Limit)) -> Result<[f64; 3], CurveError> {
                let [p, _, _] = self.partials(u, v, sides)?;
                Ok(p)
            }

            fn partials(
                &self,
                u: f64,
                v: f64,
                sides: (Limit, Limit),
            ) -> Result<[[f64; 3]; 3], CurveError> {
                let m = &mut WorkMeter::new(geom_run::Budget::UNLIMITED);
                let e = self.enclose(u, v, sides, m)?;
                Ok(e.map(|x| x.map(tight::middle)))
            }

            fn enclose(
                &self,
                u: f64,
                v: f64,
                sides: (Limit, Limit),
                meter: &mut WorkMeter<'_>,
            ) -> Result<[[Interval; 3]; 3], CurveError> {
                check_point((u, v), self.inner.domain)?;
                let (pu, pv) = (point(u)?, point(v)?);
                Ok([
                    self.inner.s(pu, pv, (0, 0), sides, meter)?,
                    self.inner.s(pu, pv, (1, 0), sides, meter)?,
                    self.inner.s(pu, pv, (0, 1), sides, meter)?,
                ])
            }

            fn bound(
                &self,
                region: ParamBox,
                meter: &mut WorkMeter<'_>,
            ) -> Result<Bounds<3>, CurveError> {
                let sides = (Limit::FromRight, Limit::FromRight);
                self.inner
                    .hull(region, meter, |s, u, v, m| s.s(u, v, (0, 0), sides, m))
            }

            fn derivative_bound(
                &self,
                region: ParamBox,
                dir: Direction,
                meter: &mut WorkMeter<'_>,
            ) -> Result<DerivativeBound<3>, CurveError> {
                let sides = (Limit::FromRight, Limit::FromRight);
                let order = match dir {
                    Direction::U => (1, 0),
                    Direction::V => (0, 1),
                };
                match self
                    .inner
                    .hull(region, meter, |s, u, v, m| s.s(u, v, order, sides, m))
                {
                    Ok(b) => Ok(DerivativeBound::Bounded(b)),
                    Err(CurveError::Unbounded) => Ok(DerivativeBound::Unbounded),
                    Err(e) => Err(e),
                }
            }

            fn partials_over(
                &self,
                region: ParamBox,
                sides: (Limit, Limit),
                order: usize,
                meter: &mut WorkMeter<'_>,
            ) -> Result<Vec<[Interval; 3]>, CurveError> {
                self.inner.partials_over(region, sides, order, meter)
            }

            fn breaks(&self) -> (Vec<f64>, Vec<f64>) {
                self.inner.breaks()
            }

            fn normal_bound(
                &self,
                region: ParamBox,
                meter: &mut WorkMeter<'_>,
            ) -> Result<Bounds<3>, CurveError> {
                let sides = (Limit::FromRight, Limit::FromRight);
                self.inner.hull(region, meter, |s, u, v, m| {
                    let a = s.s(u, v, (1, 0), sides, m)?;
                    let b = s.s(u, v, (0, 1), sides, m)?;
                    cross3(&a, &b)
                })
            }

            fn to_nurbs(
                &self,
                meter: &mut WorkMeter<'_>,
            ) -> Result<Changed<SurfaceForm>, CurveError> {
                self.inner.to_nurbs(meter)
            }

            fn split(
                &self,
                dir: Direction,
                t: f64,
                meter: &mut WorkMeter<'_>,
            ) -> Result<(Changed<SurfaceKind>, Changed<SurfaceKind>), CurveError> {
                let (l, r) = self.inner.split(dir, t, meter)?;
                Ok((
                    Changed {
                        value: SurfaceKind::$variant($kind { inner: l }),
                        deviation: 0.0,
                    },
                    Changed {
                        value: SurfaceKind::$variant($kind { inner: r }),
                        deviation: 0.0,
                    },
                ))
            }

            fn reversed(
                &self,
                dir: Direction,
                meter: &mut WorkMeter<'_>,
            ) -> Result<Changed<SurfaceKind>, CurveError> {
                let (s, deviation) = self.inner.reversed(dir, meter)?;
                Ok(Changed {
                    value: SurfaceKind::$variant($kind { inner: s }),
                    deviation,
                })
            }

            fn transformed(
                &self,
                transform: &TypedTransform,
                meter: &mut WorkMeter<'_>,
            ) -> Result<Changed<SurfaceKind>, CurveError> {
                let m = Motion::of::<3>(transform)?;
                let reach = self.inner.reach(meter)?;
                let (place, deviation) = self.inner.place.moved(&m, reach)?;
                Ok(Changed {
                    value: SurfaceKind::$variant($kind {
                        inner: Swept {
                            place,
                            ..self.inner.clone()
                        },
                    }),
                    deviation,
                })
            }
        }
    };
}

/// A surface of revolution (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Revolution {
    inner: Swept,
}

/// A linear extrusion (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Extrusion {
    inner: Swept,
}

/// A ruled surface (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Ruled {
    inner: Swept,
}

swept_surface!(Revolution, Revolution);
swept_surface!(Extrusion, Extrusion);
swept_surface!(Ruled, Ruled);

impl Revolution {
    /// The profile, the axis direction and the frame, as [`Revolution::new`] took them.
    ///
    /// # Errors
    ///
    /// [`CurveError::Defect`] never: a revolution holds a revolution's shape.
    pub fn shape(&self) -> Result<(&Trimmed<3>, [f64; 3], Frame<3>), CurveError> {
        match &self.inner.shape {
            Shape::Revolution {
                profile,
                axis,
                frame,
            } => Ok((profile, *axis, *frame)),
            _ => Err(CurveError::Defect),
        }
    }

    /// The surface swept by `profile` turning about the axis through `axis` along the frame's
    /// normal, u in half-turns over `angles` (at most one turn), v the profile's parameter.
    ///
    /// # Errors
    ///
    /// [`CurveError::NotFinite`]; [`CurveError::SpanOutside`] for more than a turn or angles that
    /// are not finite and ordered.
    pub fn new(
        profile: Trimmed<3>,
        axis: [f64; 3],
        frame: Frame<3>,
        angles: (f64, f64),
    ) -> Result<Revolution, CurveError> {
        let domain = (angles, profile.interval());
        let inner = Swept {
            shape: Shape::Revolution {
                profile,
                axis,
                frame,
            },
            place: IDENTITY,
            domain,
        };
        inner.check()?;
        Ok(Revolution { inner })
    }
}

impl Extrusion {
    /// The profile and the direction, as [`Extrusion::new`] took them.
    ///
    /// # Errors
    ///
    /// [`CurveError::Defect`] never: an extrusion holds an extrusion's shape.
    pub fn shape(&self) -> Result<(&Trimmed<3>, [f64; 3]), CurveError> {
        match &self.inner.shape {
            Shape::Extrusion { profile, direction } => Ok((profile, *direction)),
            _ => Err(CurveError::Defect),
        }
    }

    /// The surface swept by `profile` moving along `direction`, u the profile's parameter and v
    /// over `along`.
    ///
    /// # Errors
    ///
    /// [`CurveError::NotFinite`]; [`CurveError::Degenerate`] for a zero direction;
    /// [`CurveError::SpanOutside`] for a span that is not finite and ordered.
    pub fn new(
        profile: Trimmed<3>,
        direction: [f64; 3],
        along: (f64, f64),
    ) -> Result<Extrusion, CurveError> {
        let domain = (profile.interval(), along);
        let inner = Swept {
            shape: Shape::Extrusion { profile, direction },
            place: IDENTITY,
            domain,
        };
        inner.check()?;
        Ok(Extrusion { inner })
    }
}

impl Ruled {
    /// The two profiles and σ, the sign of v: +1 from [`Ruled::new`], −1 once reversed in v.
    ///
    /// # Errors
    ///
    /// [`CurveError::Defect`] never: a ruled surface holds a ruled surface's shape.
    pub fn shape(&self) -> Result<(&Trimmed<3>, &Trimmed<3>, f64), CurveError> {
        match &self.inner.shape {
            Shape::Ruled {
                first,
                second,
                sigma,
            } => Ok((first, second, *sigma)),
            _ => Err(CurveError::Defect),
        }
    }

    /// The same surface with σ = `sigma`, as reversing in v leaves it: what a stored ruled surface
    /// is rebuilt from.
    ///
    /// # Errors
    ///
    /// [`CurveError::Degenerate`] unless σ is +1 or −1.
    pub fn with_sigma(&self, sigma: f64) -> Result<Ruled, CurveError> {
        if sigma != 1.0 && sigma != -1.0 {
            return Err(CurveError::Degenerate);
        }
        let mut inner = self.inner.clone();
        if let Shape::Ruled { sigma: s, .. } = &mut inner.shape {
            *s = sigma;
        }
        Ok(Ruled { inner })
    }

    /// The ruled surface from `first` (v = 0) to `second` (v = 1), on their one interval; v over
    /// `along` (usually [0, 1]).
    ///
    /// # Errors
    ///
    /// [`CurveError::SpanOutside`] unless the curves share their interval and `along` is finite
    /// and ordered.
    pub fn new(
        first: Trimmed<3>,
        second: Trimmed<3>,
        along: (f64, f64),
    ) -> Result<Ruled, CurveError> {
        let domain = (first.interval(), along);
        let inner = Swept {
            shape: Shape::Ruled {
                first,
                second,
                sigma: 1.0,
            },
            place: IDENTITY,
            domain,
        };
        inner.check()?;
        Ok(Ruled { inner })
    }
}
