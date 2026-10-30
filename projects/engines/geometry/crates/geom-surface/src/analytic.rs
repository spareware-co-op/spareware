//! The analytic surface kinds: plane, cylinder, cone, sphere and torus, each stored as itself.
//!
//! What it does: each kind writes its surface in the shared form c + Σ f(u)·g(v)·W (`crate::terms`),
//! with angles in half-turns on an exact [`Frame`] (x̂, ŷ, n̂), and a sign s = ±1 on its axial term
//! so that reversing either parameter is exact:
//!
//! | Kind | S(u, v) | Normal direction (a positive multiple of `S_u` × `S_v`) |
//! | --- | --- | --- |
//! | [`Plane`] | c + u·a + v·b | a × b |
//! | [`Cylinder`] | c + r·e(u) + s·v·n̂ | s·e(u) |
//! | [`Cone`] | c + (r₀ + k·v)·e(u) + s·v·n̂ | sign(r₀ + k·v)·(s·e(u) − k·n̂) |
//! | [`Sphere`] | c + r·cos πv·e(u) + s·r·sin πv·n̂ | s·cos πv·e(u) + sin πv·n̂ |
//! | [`Torus`] | c + (R + r·cos πv)·e(u) + s·r·sin πv·n̂ | sign(R + r·cos πv)·(s·cos πv·e(u) + sin πv·n̂) |
//!
//! with e(u) = cos πu·x̂ + sin πu·ŷ. A torus whose minor radius exceeds its major one is a spindle:
//! its domain in v picks the apple (R + r·cos πv ≥ 0) or the lemon (≤ 0), and [`Torus::form`] says
//! which. A plane's parameter ends may be infinite.
//!
//! Guarantees:
//!
//! - **Stored as themselves.** Centres, radii, slopes and frames are the stored doubles, exactly.
//! - **Certified, or refused.** Enclosures and bounds are the terms' closed forms in interval
//!   arithmetic; the normal bound is the closed-form direction, which survives the poles of a
//!   sphere and the apex of a cone (where its sign may be either: then both).
//! - **Exact NURBS forms.** Each converts to a tensor-product rational surface (a circle in each
//!   angular direction, a line in each linear one) with a parameter map per direction, its
//!   deviation 0 where the doubles hold it exactly (a whole cylinder on an axis-aligned frame).
//! - **Transforms change kinds explicitly.** A similarity keeps the kind (lengths times its scale,
//!   the frame carried, a mirror flipping the axial sign); any other transform gives the moved
//!   NURBS form. A plane takes every affine map.
//! - **Exact split and reversal.**
//!
//! Method: the elementary surfaces of ISO 10303-42 (`plane`, `cylindrical_surface`,
//! `conical_surface`, `spherical_surface`, `toroidal_surface`), parametrised in half-turns, with the
//! kinds and the lemon and apple forms of chapter 7
//! (`projects/engines/geometry/architecture/07-geometry-types.md`); their normals by the cross
//! product of the closed-form partials.
//!
//! Cost: constant per evaluation; a bound O(1) per piece (at most 64 pieces); a conversion O(m·n)
//! for its control net.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': c the centre, r and k radii and slope, s the axial sign, u and v the parameters, a and b the plane's vectors"
)]

use alloc::vec::Vec;

use geom_curve::moved::Motion;
use geom_curve::tight::{self, point};
use geom_curve::{CurveError, DerivativeBound, Frame};
use geom_math::TypedTransform;
use geom_num::eft::two_sum;
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Changed, Direction, Limit};
use geom_run::WorkMeter;

use crate::extent::{self, Factored};
use crate::param::{ParamBox, ParamSurface, SurfaceForm, SurfaceKind, partial_index};
use crate::terms::{Fun, Term, Terms};

/// A plane, c + u·a + v·b (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Plane {
    origin: [f64; 3],
    a: [f64; 3],
    b: [f64; 3],
    domain: ParamBox,
    terms: Terms,
}

/// A frame-based kind's stored data: centre, frame, axial sign, lengths and domain.
#[derive(Clone, Debug, PartialEq)]
struct Round {
    center: [f64; 3],
    frame: Frame<3>,
    sign: f64,
    lengths: [f64; 2],
    domain: ParamBox,
}

/// A cylinder, c + r·e(u) + s·v·n̂ (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Cylinder {
    round: Round,
    terms: Terms,
}

/// A cone, c + (r₀ + k·v)·e(u) + s·v·n̂ (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Cone {
    round: Round,
    terms: Terms,
}

/// A sphere, c + r·cos πv·e(u) + s·r·sin πv·n̂ (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Sphere {
    round: Round,
    terms: Terms,
}

/// A torus, c + (R + r·cos πv)·e(u) + s·r·sin πv·n̂ (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Torus {
    round: Round,
    terms: Terms,
}

/// Which part of a torus a domain holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TorusForm {
    /// A ring torus: minor radius below the major one.
    Ring,
    /// A horn torus: the radii equal.
    Horn,
    /// The outer part of a spindle torus, where R + r·cos πv ≥ 0.
    Apple,
    /// The inner part of a spindle torus, where R + r·cos πv ≤ 0.
    Lemon,
    /// A domain reaching both parts of a spindle torus.
    Spindle,
}

/// Which kind a frame-based surface is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Which {
    Cylinder,
    Cone,
    Sphere,
    Torus,
}

/// An interval at most one turn long (2 half-turns), finite and ordered, decided exactly.
pub(crate) fn check_turn((a, b): (f64, f64)) -> Result<(), CurveError> {
    // b − a = s + e exactly; s is infinite or NaN when an end is not finite, and 0 or below unless
    // a < b.
    let (s, e) = two_sum(b, -a);
    if 0.0 < s && (s < 2.0 || (s == 2.0 && e <= 0.0)) {
        Ok(())
    } else {
        Err(CurveError::SpanOutside)
    }
}

/// A finite, ordered interval.
pub(crate) fn check_finite((a, b): (f64, f64)) -> Result<(), CurveError> {
    if a.is_finite() && b.is_finite() && a < b {
        Ok(())
    } else {
        Err(CurveError::SpanOutside)
    }
}

fn finite(v: &[f64]) -> Result<(), CurveError> {
    if v.iter().all(|x| x.is_finite()) {
        Ok(())
    } else {
        Err(CurveError::NotFinite)
    }
}

/// The points of a box inside the domain, ordered and finite.
fn check_box(((u0, u1), (v0, v1)): ParamBox, ((a, b), (c, d)): ParamBox) -> Result<(), CurveError> {
    let ok = u0.is_finite()
        && u1.is_finite()
        && v0.is_finite()
        && v1.is_finite()
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

fn check_point((u, v): (f64, f64), ((a, b), (c, d)): ParamBox) -> Result<(), CurveError> {
    if u.is_finite() && v.is_finite() && a <= u && u <= b && c <= v && v <= d {
        Ok(())
    } else {
        Err(CurveError::ParameterOutside)
    }
}

/// The terms of a frame-based kind.
fn round_terms(which: Which, r: &Round) -> Result<Terms, CurveError> {
    let (d, e) = r.frame.unit_vectors()?;
    let [xd, yd, nd] = d;
    let [xe, ye, ne] = e;
    let [l0, l1] = r.lengths;
    let s = r.sign;
    let mut terms = Vec::new();
    match which {
        Which::Cylinder => {
            terms.push(Term::scaled(Fun::Cos, Fun::One, l0, xd, xe)?);
            terms.push(Term::scaled(Fun::Sin, Fun::One, l0, yd, ye)?);
            terms.push(Term::scaled(Fun::One, Fun::Lin, s, nd, ne)?);
        }
        Which::Cone => {
            terms.push(Term::scaled(Fun::Cos, Fun::One, l0, xd, xe)?);
            terms.push(Term::scaled(Fun::Cos, Fun::Lin, l1, xd, xe)?);
            terms.push(Term::scaled(Fun::Sin, Fun::One, l0, yd, ye)?);
            terms.push(Term::scaled(Fun::Sin, Fun::Lin, l1, yd, ye)?);
            terms.push(Term::scaled(Fun::One, Fun::Lin, s, nd, ne)?);
        }
        Which::Sphere => {
            terms.push(Term::scaled(Fun::Cos, Fun::Cos, l0, xd, xe)?);
            terms.push(Term::scaled(Fun::Sin, Fun::Cos, l0, yd, ye)?);
            terms.push(Term::scaled(Fun::One, Fun::Sin, s * l0, nd, ne)?);
        }
        Which::Torus => {
            terms.push(Term::scaled(Fun::Cos, Fun::One, l0, xd, xe)?);
            terms.push(Term::scaled(Fun::Cos, Fun::Cos, l1, xd, xe)?);
            terms.push(Term::scaled(Fun::Sin, Fun::One, l0, yd, ye)?);
            terms.push(Term::scaled(Fun::Sin, Fun::Cos, l1, yd, ye)?);
            terms.push(Term::scaled(Fun::One, Fun::Sin, s * l1, nd, ne)?);
        }
    }
    let [c0, c1, c2] = r.center;
    Ok(Terms {
        c: r.center,
        ce: [point(c0)?, point(c1)?, point(c2)?],
        terms,
    })
}

/// The normal direction's terms of a frame-based kind, before the sign of its radius.
fn normal_terms(which: Which, r: &Round) -> Result<Terms, CurveError> {
    let (d, e) = r.frame.unit_vectors()?;
    let [xd, yd, nd] = d;
    let [xe, ye, ne] = e;
    let s = r.sign;
    let [_, k] = r.lengths;
    let mut terms = Vec::new();
    match which {
        Which::Cylinder => {
            terms.push(Term::scaled(Fun::Cos, Fun::One, s, xd, xe)?);
            terms.push(Term::scaled(Fun::Sin, Fun::One, s, yd, ye)?);
        }
        Which::Cone => {
            terms.push(Term::scaled(Fun::Cos, Fun::One, s, xd, xe)?);
            terms.push(Term::scaled(Fun::Sin, Fun::One, s, yd, ye)?);
            terms.push(Term::scaled(Fun::One, Fun::One, -k, nd, ne)?);
        }
        Which::Sphere | Which::Torus => {
            terms.push(Term::scaled(Fun::Cos, Fun::Cos, s, xd, xe)?);
            terms.push(Term::scaled(Fun::Sin, Fun::Cos, s, yd, ye)?);
            terms.push(Term::scaled(Fun::One, Fun::Sin, 1.0, nd, ne)?);
        }
    }
    Ok(Terms {
        c: [0.0; 3],
        ce: [point(0.0)?; 3],
        terms,
    })
}

/// The pieces a direction of a box is cut into: eighths of a turn for an angle, else one.
pub(crate) fn cuts((lo, hi): (f64, f64), angular: bool) -> Result<Vec<(f64, f64)>, CurveError> {
    let n: u32 = if angular {
        let x = geom_num::math::ceil((hi - lo) * 4.0)?.get();
        if x >= 8.0 {
            8
        } else if x >= 1.0 {
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "x is a whole number in [1, 8)"
            )]
            let k = x as u32;
            k
        } else {
            1
        }
    } else {
        1
    };
    let mut out = Vec::with_capacity(8);
    let mut start = lo;
    for i in 1..=n {
        let end = if i == n {
            hi
        } else {
            lo + (hi - lo) * (f64::from(i) / f64::from(n))
        };
        // Rounding may step past either end; the piece stays in [start, hi].
        let end = tight::larger(tight::smaller(end, hi), start);
        out.push((start, end));
        start = end;
    }
    Ok(out)
}

/// The hull of `f` over the box, cut into pieces.
fn hull(
    region: ParamBox,
    angular: (bool, bool),
    meter: &mut WorkMeter<'_>,
    f: impl Fn(Interval, Interval) -> Result<[Interval; 3], CurveError>,
) -> Result<Bounds<3>, CurveError> {
    let mut out = Bounds {
        lo: [f64::INFINITY; 3],
        hi: [f64::NEG_INFINITY; 3],
    };
    for (u0, u1) in cuts(region.0, angular.0)? {
        for (v0, v1) in cuts(region.1, angular.1)? {
            meter.charge(1)?;
            let e = f(Interval::new(u0, u1)?, Interval::new(v0, v1)?)?;
            for ((l, h), x) in out.lo.iter_mut().zip(out.hi.iter_mut()).zip(&e) {
                *l = tight::smaller(*l, x.lo());
                *h = tight::larger(*h, x.hi());
            }
        }
    }
    Ok(out)
}

/// Every partial to `order` over a region, each from `partial` (its closed-form extent), listed by
/// `partial_index`.
fn partials_upto(
    order: usize,
    meter: &mut WorkMeter<'_>,
    mut partial: impl FnMut((usize, usize), &mut WorkMeter<'_>) -> Result<[Interval; 3], CurveError>,
) -> Result<Vec<[Interval; 3]>, CurveError> {
    let mut out = Vec::with_capacity(partial_index(order.saturating_add(1), 0)?);
    for n in 0..=order {
        for j in 0..=n {
            let i = n.checked_sub(j).ok_or(CurveError::Defect)?;
            out.push(partial((i, j), meter)?);
        }
    }
    Ok(out)
}

/// The box of three enclosures.
fn boxed([x, y, z]: [Interval; 3]) -> Bounds<3> {
    Bounds {
        lo: [x.lo(), y.lo(), z.lo()],
        hi: [x.hi(), y.hi(), z.hi()],
    }
}

/// Whether a kind's directions are angular.
fn angular(which: Which) -> (bool, bool) {
    match which {
        Which::Cylinder | Which::Cone => (true, false),
        Which::Sphere | Which::Torus => (true, true),
    }
}

impl Round {
    fn check(&self, which: Which) -> Result<(), CurveError> {
        finite(&self.center)?;
        finite(&self.lengths)?;
        let ((u0, u1), (v0, v1)) = self.domain;
        check_turn((u0, u1))?;
        let [l0, l1] = self.lengths;
        match which {
            Which::Cylinder | Which::Sphere => {
                if l0 <= 0.0 {
                    return Err(CurveError::Degenerate);
                }
            }
            Which::Cone => {
                if l0 == 0.0 && l1 == 0.0 {
                    return Err(CurveError::Degenerate);
                }
            }
            Which::Torus => {
                if l0 <= 0.0 || l1 <= 0.0 {
                    return Err(CurveError::Degenerate);
                }
            }
        }
        match which {
            Which::Cylinder | Which::Cone => check_finite((v0, v1)),
            Which::Sphere => {
                if -0.5 <= v0 && v0 < v1 && v1 <= 0.5 {
                    Ok(())
                } else {
                    Err(CurveError::SpanOutside)
                }
            }
            Which::Torus => check_turn((v0, v1)),
        }
    }

    /// The sign of the radius r₀ + k·v (cone) or R + r·cos πv (torus) over v: Some(true) if
    /// certainly positive, Some(false) if certainly negative, None if it may be either.
    fn radius_sign(&self, which: Which, v: Interval) -> Result<Option<bool>, CurveError> {
        let [l0, l1] = self.lengths;
        let rho = match which {
            Which::Cylinder | Which::Sphere => return Ok(Some(true)),
            Which::Cone => tight::add(point(l0)?, tight::mul(point(l1)?, v)?)?,
            Which::Torus => tight::add(point(l0)?, tight::mul(point(l1)?, v.cospi()?)?)?,
        };
        Ok(if rho.lo() > 0.0 {
            Some(true)
        } else if rho.hi() < 0.0 {
            Some(false)
        } else {
            None
        })
    }

    /// ∂^{i+j}S/∂u^i∂v^j over the region in closed form (`crate::extent`): a torus by its
    /// factored form c + (R + r·cos πv)·(cos πu·x̂ + sin πu·ŷ) + s·r·sin πv·n̂, every other kind
    /// by its terms.
    fn closed(
        &self,
        which: Which,
        terms: &Terms,
        region: ParamBox,
        order: (usize, usize),
        meter: &mut WorkMeter<'_>,
    ) -> Result<[Interval; 3], CurveError> {
        let ((u0, u1), (v0, v1)) = region;
        if which != Which::Torus || (u0 == u1 && v0 == v1) {
            return extent::terms(terms, region, order, meter);
        }
        let (_, [xe, ye, ne]) = self.frame.unit_vectors()?;
        let [l0, l1] = self.lengths;
        let zero = point(0.0)?;
        let sr = point(self.sign * l1)?;
        let mut g = [(zero, zero); 3];
        let mut q = [[zero; 4]; 3];
        for ((gk, qk), ((&x, &y), &n)) in g.iter_mut().zip(&mut q).zip(xe.iter().zip(&ye).zip(&ne))
        {
            *gk = (x, y);
            *qk = [zero, zero, zero, tight::mul(sr, n)?];
        }
        let f = Factored {
            center: terms.ce,
            g,
            p: [point(l0)?, zero, point(l1)?, zero],
            q,
        };
        extent::factored(&f, region, order, meter)
    }

    /// This round's data under a similarity: centre and frame carried, lengths scaled, the axial
    /// sign flipped by a mirror.
    fn moved(&self, m: &Motion) -> Result<Round, CurveError> {
        Ok(Round {
            center: m.point(self.center)?,
            frame: Frame::new(m.vector(self.frame.x())?, m.normal(self.frame.normal())?)?,
            sign: if m.mirror() { -self.sign } else { self.sign },
            lengths: self.lengths.map(|x| m.scale * x),
            domain: self.domain,
        })
    }

    /// Reversed in `dir`: in u, the frame's normal and the axial sign flip (ŷ negated, n̂·s kept);
    /// in v, the axial sign flips (and a cone's slope).
    fn reversed(&self, which: Which, dir: Direction) -> Round {
        let ((u0, u1), (v0, v1)) = self.domain;
        match dir {
            Direction::U => Round {
                frame: self.frame.reversed(),
                sign: -self.sign,
                domain: ((-u1, -u0), (v0, v1)),
                ..self.clone()
            },
            Direction::V => {
                let [l0, l1] = self.lengths;
                Round {
                    sign: -self.sign,
                    lengths: if which == Which::Cone {
                        [l0, -l1]
                    } else {
                        [l0, l1]
                    },
                    domain: ((u0, u1), (-v1, -v0)),
                    ..self.clone()
                }
            }
        }
    }
}

/// The exact image of a kind's terms under a transform, enclosed (c as a point, each W as a
/// vector), the functions kept.
fn image_terms(t: &Terms, m: &Motion) -> Result<Terms, CurveError> {
    let mut terms = Vec::with_capacity(t.terms.len());
    for x in &t.terms {
        terms.push(Term {
            u: x.u,
            v: x.v,
            w: x.w,
            we: m.enclosed(&x.we, false)?,
        });
    }
    Ok(Terms {
        c: t.c,
        ce: m.enclosed(&t.ce, true)?,
        terms,
    })
}

/// The shared behaviour of a frame-based kind.
fn round_point(t: &Terms, d: ParamBox, u: f64, v: f64) -> Result<[f64; 3], CurveError> {
    check_point((u, v), d)?;
    t.value(u, v, (0, 0))
}

fn round_partials(t: &Terms, d: ParamBox, u: f64, v: f64) -> Result<[[f64; 3]; 3], CurveError> {
    check_point((u, v), d)?;
    Ok([
        t.value(u, v, (0, 0))?,
        t.value(u, v, (1, 0))?,
        t.value(u, v, (0, 1))?,
    ])
}

fn round_enclose(
    t: &Terms,
    d: ParamBox,
    u: f64,
    v: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<[[Interval; 3]; 3], CurveError> {
    check_point((u, v), d)?;
    meter.charge(3)?;
    let (pu, pv) = (point(u)?, point(v)?);
    Ok([
        t.enclose(pu, pv, (0, 0))?,
        t.enclose(pu, pv, (1, 0))?,
        t.enclose(pu, pv, (0, 1))?,
    ])
}

/// A frame-based kind's accessors and builder, written once.
macro_rules! round_kind {
    ($kind:ident, $which:expr) => {
        impl $kind {
            /// The stored centre.
            #[must_use]
            pub fn center(&self) -> [f64; 3] {
                self.round.center
            }

            /// The frame: e(u) = cos πu·x̂ + sin πu·ŷ, the axis n̂.
            #[must_use]
            pub fn frame(&self) -> Frame<3> {
                self.round.frame
            }

            /// The axial sign s: +1 or −1.
            #[must_use]
            pub fn axial_sign(&self) -> f64 {
                self.round.sign
            }

            fn build(round: Round) -> Result<$kind, CurveError> {
                round.check($which)?;
                let terms = round_terms($which, &round)?;
                Ok($kind { round, terms })
            }
        }
    };
}

round_kind!(Cylinder, Which::Cylinder);
round_kind!(Cone, Which::Cone);
round_kind!(Sphere, Which::Sphere);
round_kind!(Torus, Which::Torus);

impl Cylinder {
    /// The cylinder about `center` with `radius` on `frame` (axis n̂), u in half-turns (at most
    /// one turn) and v the height along n̂.
    ///
    /// # Errors
    ///
    /// [`CurveError::NotFinite`]; [`CurveError::Degenerate`] unless the radius is positive;
    /// [`CurveError::SpanOutside`] for a domain more than a turn wide in u or not finite.
    pub fn new(
        center: [f64; 3],
        radius: f64,
        frame: Frame<3>,
        domain: ParamBox,
    ) -> Result<Cylinder, CurveError> {
        Cylinder::build(Round {
            center,
            frame,
            sign: 1.0,
            lengths: [radius, 0.0],
            domain,
        })
    }

    /// The radius.
    #[must_use]
    pub fn radius(&self) -> f64 {
        self.round.lengths[0]
    }
}

impl Cone {
    /// The cone about `center` whose radius at height v along n̂ is `radius` + `slope`·v.
    ///
    /// # Errors
    ///
    /// As [`Cylinder::new`]; [`CurveError::Degenerate`] if both radius and slope are zero.
    pub fn new(
        center: [f64; 3],
        radius: f64,
        slope: f64,
        frame: Frame<3>,
        domain: ParamBox,
    ) -> Result<Cone, CurveError> {
        Cone::build(Round {
            center,
            frame,
            sign: 1.0,
            lengths: [radius, slope],
            domain,
        })
    }

    /// The radius at v = 0 and the slope.
    #[must_use]
    pub fn radius_and_slope(&self) -> (f64, f64) {
        let [a, b] = self.round.lengths;
        (a, b)
    }
}

impl Sphere {
    /// The sphere about `center` of `radius` on `frame`: u the longitude (at most one turn), v the
    /// latitude in [−1/2, 1/2] half-turns.
    ///
    /// # Errors
    ///
    /// As [`Cylinder::new`], with v inside [−1/2, 1/2].
    pub fn new(
        center: [f64; 3],
        radius: f64,
        frame: Frame<3>,
        domain: ParamBox,
    ) -> Result<Sphere, CurveError> {
        Sphere::build(Round {
            center,
            frame,
            sign: 1.0,
            lengths: [radius, 0.0],
            domain,
        })
    }

    /// The radius.
    #[must_use]
    pub fn radius(&self) -> f64 {
        self.round.lengths[0]
    }
}

impl Torus {
    /// The torus about `center` with radii `major` and `minor` on `frame`: u about the axis, v
    /// about the tube, each at most one turn.
    ///
    /// # Errors
    ///
    /// As [`Cylinder::new`], with both radii positive.
    pub fn new(
        center: [f64; 3],
        major: f64,
        minor: f64,
        frame: Frame<3>,
        domain: ParamBox,
    ) -> Result<Torus, CurveError> {
        Torus::build(Round {
            center,
            frame,
            sign: 1.0,
            lengths: [major, minor],
            domain,
        })
    }

    /// The major and minor radii.
    #[must_use]
    pub fn radii(&self) -> (f64, f64) {
        let [a, b] = self.round.lengths;
        (a, b)
    }

    /// Which part of the torus the domain holds.
    ///
    /// # Errors
    ///
    /// [`CurveError::NotFinite`] (it cannot happen for a torus [`Torus::new`] accepted).
    pub fn form(&self) -> Result<TorusForm, CurveError> {
        let [big, small] = self.round.lengths;
        if small < big {
            return Ok(TorusForm::Ring);
        }
        if small == big {
            return Ok(TorusForm::Horn);
        }
        let (_, (v0, v1)) = self.round.domain;
        Ok(
            match self
                .round
                .radius_sign(Which::Torus, Interval::new(v0, v1)?)?
            {
                Some(true) => TorusForm::Apple,
                Some(false) => TorusForm::Lemon,
                None => TorusForm::Spindle,
            },
        )
    }
}

/// The `ParamSurface` implementation of a frame-based kind, written once for all four.
macro_rules! round_surface {
    ($kind:ident, $which:expr, $variant:ident) => {
        impl ParamSurface for $kind {
            fn domain(&self) -> ParamBox {
                self.round.domain
            }

            fn point(
                &self,
                u: f64,
                v: f64,
                _sides: (Limit, Limit),
            ) -> Result<[f64; 3], CurveError> {
                round_point(&self.terms, self.round.domain, u, v)
            }

            fn partials(
                &self,
                u: f64,
                v: f64,
                _sides: (Limit, Limit),
            ) -> Result<[[f64; 3]; 3], CurveError> {
                round_partials(&self.terms, self.round.domain, u, v)
            }

            fn enclose(
                &self,
                u: f64,
                v: f64,
                _sides: (Limit, Limit),
                meter: &mut WorkMeter<'_>,
            ) -> Result<[[Interval; 3]; 3], CurveError> {
                round_enclose(&self.terms, self.round.domain, u, v, meter)
            }

            fn bound(
                &self,
                region: ParamBox,
                meter: &mut WorkMeter<'_>,
            ) -> Result<Bounds<3>, CurveError> {
                check_box(region, self.round.domain)?;
                Ok(boxed(self.round.closed(
                    $which,
                    &self.terms,
                    region,
                    (0, 0),
                    meter,
                )?))
            }

            fn derivative_bound(
                &self,
                region: ParamBox,
                dir: Direction,
                meter: &mut WorkMeter<'_>,
            ) -> Result<DerivativeBound<3>, CurveError> {
                check_box(region, self.round.domain)?;
                let order = match dir {
                    Direction::U => (1, 0),
                    Direction::V => (0, 1),
                };
                Ok(DerivativeBound::Bounded(boxed(self.round.closed(
                    $which,
                    &self.terms,
                    region,
                    order,
                    meter,
                )?)))
            }

            fn partials_over(
                &self,
                region: ParamBox,
                _sides: (Limit, Limit),
                order: usize,
                meter: &mut WorkMeter<'_>,
            ) -> Result<Vec<[Interval; 3]>, CurveError> {
                check_box(region, self.round.domain)?;
                partials_upto(order, meter, |o, m| {
                    self.round.closed($which, &self.terms, region, o, m)
                })
            }

            fn breaks(&self) -> (Vec<f64>, Vec<f64>) {
                (Vec::new(), Vec::new())
            }

            fn normal_bound(
                &self,
                region: ParamBox,
                meter: &mut WorkMeter<'_>,
            ) -> Result<Bounds<3>, CurveError> {
                check_box(region, self.round.domain)?;
                let n = normal_terms($which, &self.round)?;
                hull(region, angular($which), meter, |u, v| {
                    let e = n.enclose(u, v, (0, 0))?;
                    match self.round.radius_sign($which, v)? {
                        Some(true) => Ok(e),
                        Some(false) => Ok(e.map(Interval::neg)),
                        None => {
                            let mut out = e;
                            for o in &mut out {
                                let m = tight::magnitude(*o);
                                *o = Interval::new(-m, m)?;
                            }
                            Ok(out)
                        }
                    }
                })
            }

            fn to_nurbs(
                &self,
                meter: &mut WorkMeter<'_>,
            ) -> Result<Changed<SurfaceForm>, CurveError> {
                self.terms.to_nurbs(self.round.domain, meter)
            }

            fn split(
                &self,
                dir: Direction,
                t: f64,
                _meter: &mut WorkMeter<'_>,
            ) -> Result<(Changed<SurfaceKind>, Changed<SurfaceKind>), CurveError> {
                let ((u0, u1), (v0, v1)) = self.round.domain;
                let (lo, hi) = match dir {
                    Direction::U => (u0, u1),
                    Direction::V => (v0, v1),
                };
                if !(lo < t && t < hi) {
                    return Err(CurveError::ParameterOutside);
                }
                let (left, right) = match dir {
                    Direction::U => (((u0, t), (v0, v1)), ((t, u1), (v0, v1))),
                    Direction::V => (((u0, u1), (v0, t)), ((u0, u1), (t, v1))),
                };
                let part = |domain| -> Result<Changed<SurfaceKind>, CurveError> {
                    Ok(Changed {
                        value: SurfaceKind::$variant($kind::build(Round {
                            domain,
                            ..self.round.clone()
                        })?),
                        deviation: 0.0,
                    })
                };
                Ok((part(left)?, part(right)?))
            }

            fn reversed(
                &self,
                dir: Direction,
                _meter: &mut WorkMeter<'_>,
            ) -> Result<Changed<SurfaceKind>, CurveError> {
                Ok(Changed {
                    value: SurfaceKind::$variant($kind::build(self.round.reversed($which, dir))?),
                    deviation: 0.0,
                })
            }

            fn transformed(
                &self,
                transform: &TypedTransform,
                meter: &mut WorkMeter<'_>,
            ) -> Result<Changed<SurfaceKind>, CurveError> {
                let m = Motion::of::<3>(transform)?;
                meter.charge(1)?;
                if !m.similar() {
                    return affine_image(&self.terms, self.round.domain, angular($which), &m);
                }
                let k = $kind::build(self.round.moved(&m)?)?;
                let image = image_terms(&self.terms, &m)?;
                let deviation = k.terms.distance(&image, self.round.domain)?;
                Ok(Changed {
                    value: SurfaceKind::$variant(k),
                    deviation,
                })
            }
        }
    };
}

round_surface!(Cylinder, Which::Cylinder, Cylinder);
round_surface!(Cone, Which::Cone, Cone);
round_surface!(Sphere, Which::Sphere, Sphere);
round_surface!(Torus, Which::Torus, Torus);

impl Plane {
    /// The plane c + u·a + v·b over `domain`, whose ends may be infinite.
    ///
    /// # Errors
    ///
    /// [`CurveError::NotFinite`]; [`CurveError::Degenerate`] for parallel a and b;
    /// [`CurveError::SpanOutside`] for a side that is not ordered or is NaN.
    pub fn new(
        origin: [f64; 3],
        a: [f64; 3],
        b: [f64; 3],
        domain: ParamBox,
    ) -> Result<Plane, CurveError> {
        finite(&origin)?;
        finite(&a)?;
        finite(&b)?;
        if !geom_curve::frame::not_parallel(a, b) {
            return Err(CurveError::Degenerate);
        }
        for (lo, hi) in [domain.0, domain.1] {
            if !(lo < hi && lo < f64::INFINITY && hi > f64::NEG_INFINITY) {
                return Err(CurveError::SpanOutside);
            }
        }
        let [c0, c1, c2] = origin;
        let terms = Terms {
            c: origin,
            ce: [point(c0)?, point(c1)?, point(c2)?],
            terms: alloc::vec![
                Term::exact(Fun::Lin, Fun::One, a)?,
                Term::exact(Fun::One, Fun::Lin, b)?
            ],
        };
        Ok(Plane {
            origin,
            a,
            b,
            domain,
            terms,
        })
    }

    /// The origin c and the vectors a and b.
    #[must_use]
    pub fn vectors(&self) -> ([f64; 3], [f64; 3], [f64; 3]) {
        (self.origin, self.a, self.b)
    }

    fn bounded(&self) -> bool {
        let ((a, b), (c, d)) = self.domain;
        a.is_finite() && b.is_finite() && c.is_finite() && d.is_finite()
    }
}

impl ParamSurface for Plane {
    fn domain(&self) -> ParamBox {
        self.domain
    }

    fn point(&self, u: f64, v: f64, _sides: (Limit, Limit)) -> Result<[f64; 3], CurveError> {
        round_point(&self.terms, self.domain, u, v)
    }

    fn partials(
        &self,
        u: f64,
        v: f64,
        _sides: (Limit, Limit),
    ) -> Result<[[f64; 3]; 3], CurveError> {
        round_partials(&self.terms, self.domain, u, v)
    }

    fn enclose(
        &self,
        u: f64,
        v: f64,
        _sides: (Limit, Limit),
        meter: &mut WorkMeter<'_>,
    ) -> Result<[[Interval; 3]; 3], CurveError> {
        round_enclose(&self.terms, self.domain, u, v, meter)
    }

    fn bound(&self, region: ParamBox, meter: &mut WorkMeter<'_>) -> Result<Bounds<3>, CurveError> {
        check_box(region, self.domain)?;
        Ok(boxed(extent::terms(&self.terms, region, (0, 0), meter)?))
    }

    fn derivative_bound(
        &self,
        region: ParamBox,
        dir: Direction,
        meter: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<3>, CurveError> {
        check_box(region, self.domain)?;
        let order = match dir {
            Direction::U => (1, 0),
            Direction::V => (0, 1),
        };
        Ok(DerivativeBound::Bounded(boxed(extent::terms(
            &self.terms,
            region,
            order,
            meter,
        )?)))
    }

    fn partials_over(
        &self,
        region: ParamBox,
        _sides: (Limit, Limit),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; 3]>, CurveError> {
        check_box(region, self.domain)?;
        partials_upto(order, meter, |o, m| {
            extent::terms(&self.terms, region, o, m)
        })
    }

    fn breaks(&self) -> (Vec<f64>, Vec<f64>) {
        (Vec::new(), Vec::new())
    }

    fn normal_bound(
        &self,
        region: ParamBox,
        _meter: &mut WorkMeter<'_>,
    ) -> Result<Bounds<3>, CurveError> {
        check_box(region, self.domain)?;
        let [a0, a1, a2] = self.a.map(point);
        let [b0, b1, b2] = self.b.map(point);
        let (a0, a1, a2, b0, b1, b2) = (a0?, a1?, a2?, b0?, b1?, b2?);
        let n = [
            tight::sub(tight::mul(a1, b2)?, tight::mul(a2, b1)?)?,
            tight::sub(tight::mul(a2, b0)?, tight::mul(a0, b2)?)?,
            tight::sub(tight::mul(a0, b1)?, tight::mul(a1, b0)?)?,
        ];
        Ok(Bounds {
            lo: n.map(Interval::lo),
            hi: n.map(Interval::hi),
        })
    }

    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<SurfaceForm>, CurveError> {
        if !self.bounded() {
            return Err(CurveError::Unbounded);
        }
        self.terms.to_nurbs(self.domain, meter)
    }

    fn split(
        &self,
        dir: Direction,
        t: f64,
        _meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<SurfaceKind>, Changed<SurfaceKind>), CurveError> {
        let ((u0, u1), (v0, v1)) = self.domain;
        let (lo, hi) = match dir {
            Direction::U => (u0, u1),
            Direction::V => (v0, v1),
        };
        if !(lo < t && t < hi) {
            return Err(CurveError::ParameterOutside);
        }
        let (left, right) = match dir {
            Direction::U => (((u0, t), (v0, v1)), ((t, u1), (v0, v1))),
            Direction::V => (((u0, u1), (v0, t)), ((u0, u1), (t, v1))),
        };
        let part = |domain| -> Result<Changed<SurfaceKind>, CurveError> {
            Ok(Changed {
                value: SurfaceKind::Plane(Plane::new(self.origin, self.a, self.b, domain)?),
                deviation: 0.0,
            })
        };
        Ok((part(left)?, part(right)?))
    }

    fn reversed(
        &self,
        dir: Direction,
        _meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        let ((u0, u1), (v0, v1)) = self.domain;
        let p = match dir {
            Direction::U => Plane::new(
                self.origin,
                self.a.map(|x| -x),
                self.b,
                ((-u1, -u0), (v0, v1)),
            )?,
            Direction::V => Plane::new(
                self.origin,
                self.a,
                self.b.map(|x| -x),
                ((u0, u1), (-v1, -v0)),
            )?,
        };
        Ok(Changed {
            value: SurfaceKind::Plane(p),
            deviation: 0.0,
        })
    }

    fn transformed(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        let m = Motion::of::<3>(transform)?;
        meter.charge(1)?;
        let p = Plane::new(
            m.point(self.origin)?,
            m.vector(self.a)?,
            m.vector(self.b)?,
            self.domain,
        )?;
        let image = image_terms(&self.terms, &m)?;
        let deviation = if self.bounded() {
            p.terms.distance(&image, self.domain)?
        } else {
            // Unbounded: only an exact image of a and b keeps the plane within a finite distance.
            for (x, y) in p.terms.terms.iter().zip(&image.terms) {
                if tight::distance(&x.we, &y.we)? != 0.0 {
                    return Err(CurveError::Unbounded);
                }
            }
            tight::distance(&p.terms.ce, &image.ce)?
        };
        Ok(Changed {
            value: SurfaceKind::Plane(p),
            deviation,
        })
    }
}

/// The image of an analytic kind under an affine map that is not a similarity: an elliptic
/// cylinder or cone, an ellipsoid, an affine torus. It keeps the kind's terms, each vector mapped,
/// so it keeps the kind's parameters exactly (a transform may change a kind, never its
/// parameters); its normals are bounded by the cross product of its enclosed partials.
#[derive(Clone, Debug, PartialEq)]
pub struct AffineSurface {
    terms: Terms,
    domain: ParamBox,
    angular: (bool, bool),
}

/// The terms traversed the other way in `dir`: the vectors of the terms odd in that parameter
/// (t and sin πt) negated. Exact.
fn reversed_terms(t: &Terms, dir: Direction) -> Terms {
    let mut out = t.clone();
    for x in &mut out.terms {
        let f = match dir {
            Direction::U => x.u,
            Direction::V => x.v,
        };
        if matches!(f, Fun::Lin | Fun::Sin) {
            x.w = x.w.map(|y| -y);
            x.we = x.we.map(Interval::neg);
        }
    }
    out
}

/// The affine image of `terms` under `m` on the same parameters, with its certified deviation.
fn affine_image(
    terms: &Terms,
    domain: ParamBox,
    angular: (bool, bool),
    m: &Motion,
) -> Result<Changed<SurfaceKind>, CurveError> {
    let c = m.point(terms.c)?;
    let [c0, c1, c2] = c;
    let mut list = Vec::with_capacity(terms.terms.len());
    for x in &terms.terms {
        list.push(Term::exact(x.u, x.v, m.vector(x.w)?)?);
    }
    let moved = Terms {
        c,
        ce: [point(c0)?, point(c1)?, point(c2)?],
        terms: list,
    };
    let image = image_terms(terms, m)?;
    let deviation = moved.distance(&image, domain)?;
    Ok(Changed {
        value: SurfaceKind::Affine(AffineSurface {
            terms: moved,
            domain,
            angular,
        }),
        deviation,
    })
}

impl ParamSurface for AffineSurface {
    fn domain(&self) -> ParamBox {
        self.domain
    }

    fn point(&self, u: f64, v: f64, _sides: (Limit, Limit)) -> Result<[f64; 3], CurveError> {
        round_point(&self.terms, self.domain, u, v)
    }

    fn partials(
        &self,
        u: f64,
        v: f64,
        _sides: (Limit, Limit),
    ) -> Result<[[f64; 3]; 3], CurveError> {
        round_partials(&self.terms, self.domain, u, v)
    }

    fn enclose(
        &self,
        u: f64,
        v: f64,
        _sides: (Limit, Limit),
        meter: &mut WorkMeter<'_>,
    ) -> Result<[[Interval; 3]; 3], CurveError> {
        round_enclose(&self.terms, self.domain, u, v, meter)
    }

    fn bound(&self, region: ParamBox, meter: &mut WorkMeter<'_>) -> Result<Bounds<3>, CurveError> {
        check_box(region, self.domain)?;
        Ok(boxed(extent::terms(&self.terms, region, (0, 0), meter)?))
    }

    fn derivative_bound(
        &self,
        region: ParamBox,
        dir: Direction,
        meter: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<3>, CurveError> {
        check_box(region, self.domain)?;
        let order = match dir {
            Direction::U => (1, 0),
            Direction::V => (0, 1),
        };
        Ok(DerivativeBound::Bounded(boxed(extent::terms(
            &self.terms,
            region,
            order,
            meter,
        )?)))
    }

    fn partials_over(
        &self,
        region: ParamBox,
        _sides: (Limit, Limit),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; 3]>, CurveError> {
        check_box(region, self.domain)?;
        partials_upto(order, meter, |o, m| {
            extent::terms(&self.terms, region, o, m)
        })
    }

    fn breaks(&self) -> (Vec<f64>, Vec<f64>) {
        (Vec::new(), Vec::new())
    }

    fn normal_bound(
        &self,
        region: ParamBox,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Bounds<3>, CurveError> {
        check_box(region, self.domain)?;
        hull(region, self.angular, meter, |u, v| {
            let a = self.terms.enclose(u, v, (1, 0))?;
            let b = self.terms.enclose(u, v, (0, 1))?;
            let [a0, a1, a2] = a;
            let [b0, b1, b2] = b;
            Ok([
                tight::sub(tight::mul(a1, b2)?, tight::mul(a2, b1)?)?,
                tight::sub(tight::mul(a2, b0)?, tight::mul(a0, b2)?)?,
                tight::sub(tight::mul(a0, b1)?, tight::mul(a1, b0)?)?,
            ])
        })
    }

    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<SurfaceForm>, CurveError> {
        self.terms.to_nurbs(self.domain, meter)
    }

    fn split(
        &self,
        dir: Direction,
        t: f64,
        _meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<SurfaceKind>, Changed<SurfaceKind>), CurveError> {
        let ((u0, u1), (v0, v1)) = self.domain;
        let (lo, hi) = match dir {
            Direction::U => (u0, u1),
            Direction::V => (v0, v1),
        };
        if !(lo < t && t < hi) {
            return Err(CurveError::ParameterOutside);
        }
        let (left, right) = match dir {
            Direction::U => (((u0, t), (v0, v1)), ((t, u1), (v0, v1))),
            Direction::V => (((u0, u1), (v0, t)), ((u0, u1), (t, v1))),
        };
        let part = |domain| Changed {
            value: SurfaceKind::Affine(AffineSurface {
                domain,
                ..self.clone()
            }),
            deviation: 0.0,
        };
        Ok((part(left), part(right)))
    }

    fn reversed(
        &self,
        dir: Direction,
        _meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        let ((u0, u1), (v0, v1)) = self.domain;
        let domain = match dir {
            Direction::U => ((-u1, -u0), (v0, v1)),
            Direction::V => ((u0, u1), (-v1, -v0)),
        };
        Ok(Changed {
            value: SurfaceKind::Affine(AffineSurface {
                terms: reversed_terms(&self.terms, dir),
                domain,
                angular: self.angular,
            }),
            deviation: 0.0,
        })
    }

    fn transformed(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        let m = Motion::of::<3>(transform)?;
        meter.charge(1)?;
        affine_image(&self.terms, self.domain, self.angular, &m)
    }
}
