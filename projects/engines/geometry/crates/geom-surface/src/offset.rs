//! The offset surface kind: a base kind moved along its unit normal, O = S + d·n with
//! n = (`S_u` × `S_v`)/|`S_u` × `S_v`|, with symbolic higher derivatives and a certified NURBS cache.
//!
//! What it does: [`Offset`] stores its base, any [`SurfaceKind`] (an offset too), the part of the
//! base's domain it covers, and the distance d; a positive distance moves along `S_u` × `S_v`. It is
//! procedural (chapter 7): its points and its partials of every order come from the base's
//! partials one order higher through two-variable interval Taylor jets (`crate::jet`), so its
//! second and third derivatives, which Class-A blends need (chapter 13.10), are the formula's
//! exact derivatives, never finite differences. Its NURBS form is a cache on the same parameters,
//! built by Piegl and Tiller's sample, interpolate and remove-knots method and certified box by
//! box in interval arithmetic (`crate::cache`), computed on demand or kept with
//! [`Offset::cached`]; [`Offset::to_nurbs_within`] asks for a stated error.
//!
//! The kinds whose offsets stay in their kind (a plane, cylinder, sphere, torus or cone, chapter
//! 13.11's table) are offset in kind by the operation (piece G270); this kind is the offset of
//! everything else, and of those too when asked.
//!
//! Guarantees:
//!
//! - **Certified, or refused.** Every enclosure holds the exact offset of the stored base. Where
//!   the base's normal may vanish (a pole, a cusp, or a box too wide to separate it from zero) an
//!   enclosure is refused and a derivative bound is `Unbounded`; a bound on the points still holds
//!   there, from the base's bound widened by |d|, and the normal bound is then the whole space,
//!   which decides no sign. Elsewhere the normal bound is the base's, turned over where the offset
//!   folds, wherever the sign of 1 − 2Hd + Kd² is certified (Maekawa's identity below), and the
//!   cross product of the enclosed partials where it is not.
//! - **Offset by d, then by −d, is the base.** Where the offset is regular (d·κ < 1 for both
//!   principal curvatures κ on the side it moves to), the offset's normal is the base's, so the
//!   offset by −d of the offset by d is the base itself, and its enclosures hold the base's points.
//! - **Exact split and reversal.** A split narrows the domain on the same base (deviation 0); a
//!   reversal reverses the base and negates the distance (`S_u` × `S_v` turns over).
//! - **Transforms keep the kind only when exact.** A grid transform (a signed permutation and a
//!   whole-step translation, so lengths are kept) of an offset whose base moves exactly gives an
//!   offset, a mirror negating the distance; any other transform gives the moved NURBS cache, with
//!   both deviations.
//!
//! Method: the offset surface as ISO 10303-42 defines it (`offset_surface`, the base moved along
//! its normal by a distance), its derivatives by automatic differentiation in interval arithmetic
//! (Griewank and Walther, *Evaluating Derivatives*, 2nd ed., SIAM (2008),
//! <https://doi.org/10.1137/1.9780898717761>), which re-derives what `SMLib`'s offset surface gets
//! wrong in its third derivatives (chapter 13.11); the identity `O_u` × `O_v` = (1 − 2Hd + Kd²)·(`S_u` × `S_v`)
//! for the regular offset (T. Maekawa, "An overview of offset curves and surfaces", Computer-Aided
//! Design 31(3) (1999), <https://doi.org/10.1016/S0010-4485(99)00013-5>); the cache by L. Piegl and
//! W. Tiller, "Computing offsets of NURBS curves and surfaces", Computer-Aided Design 31(2)
//! (1999), <https://doi.org/10.1016/S0010-4485(98)00066-9>.
//!
//! Recursion:
//!
//! - Types: `Offset`, `SurfaceKind`.
//!   Bound: 4 (`MAX_NESTING`).
//!   Why: an offset's base may be an offset, and `Offset::new` refuses a base that is already
//!   `MAX_NESTING` offsets deep; split, reversal and transforms keep the depth.
//!   Test: `offsets_clone_compare_and_print_at_their_bound`.
//! - Cycle: `<Offset as ParamSurface>::partials_over`, `<SurfaceKind as ParamSurface>::partials_over`, `Offset::partials_of`, `Offset::direct`, `Offset::jets`.
//!   Bound: 21.
//!   Why: each offset of the chain adds five frames (the enum's dispatch, the offset's
//!   `partials_over`, `partials_of`, `direct` and `jets`) and asks its base, one level down; at
//!   most `MAX_NESTING` = 4 offsets, then the leaf's dispatch.
//!   Test: `offsets_evaluate_at_their_bound`.
//! - Cycle: `<Offset as ParamSurface>::bound`, `<SurfaceKind as ParamSurface>::bound`.
//!   Bound: 9.
//!   Why: two frames per offset of the chain (dispatch and the offset's `bound`, which asks its
//!   base where the normal may vanish), at most `MAX_NESTING` = 4, then the leaf's dispatch.
//!   Test: `offsets_bound_at_their_bound`.
//! - Cycle: `<Offset as ParamSurface>::breaks`, `<SurfaceKind as ParamSurface>::breaks`.
//!   Bound: 9.
//!   Why: two frames per offset of the chain, at most `MAX_NESTING` = 4, then the leaf's dispatch.
//!   Test: `offsets_evaluate_at_their_bound`.
//! - Cycle: `<Offset as ParamSurface>::normal_bound`, `<SurfaceKind as ParamSurface>::normal_bound`.
//!   Bound: 9.
//!   Why: two frames per offset of the chain (dispatch and the offset's `normal_bound`, which takes
//!   its base's where the fold factor's sign is certain), at most `MAX_NESTING` = 4, then the
//!   leaf's dispatch.
//!   Test: `offsets_bound_at_their_bound`.
//! - Cycle: `<Offset as ParamSurface>::reversed`, `<SurfaceKind as ParamSurface>::reversed`.
//!   Bound: 9.
//!   Why: two frames per offset of the chain (each reverses its base), at most `MAX_NESTING` = 4,
//!   then the leaf's dispatch.
//!   Test: `offsets_reverse_and_move_at_their_bound`.
//! - Cycle: `<Offset as ParamSurface>::transformed`, `<SurfaceKind as ParamSurface>::transformed`.
//!   Bound: 9.
//!   Why: two frames per offset of the chain (each moves its base), at most `MAX_NESTING` = 4, then
//!   the leaf's dispatch.
//!   Test: `offsets_reverse_and_move_at_their_bound`.
//!
//! Cost: an evaluation of order k is the base's of order k + 1 plus O(k⁴) interval operations (a
//! box's twice, for its centred form); a bound O(1) evaluations per piece (each smooth part of the
//! box cut into four by four), a normal bound up to 64 more per piece where the fold factor's
//! sign needs smaller boxes (an explicit stack, three halvings deep: no recursion) and two of the
//! base's normal bounds; the cache as `crate::cache` states.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formula's: s the base, o the offset, n the normal, d the distance, u and v the parameters"
)]

use alloc::boxed::Box;
use alloc::vec::Vec;

use geom_curve::tight::{self, point};
use geom_curve::{CurveError, DerivativeBound};
use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Changed, Direction, Limit};
use geom_run::WorkMeter;

use crate::cache;
use crate::jet::Jet;
use crate::param::{
    ParamBox, ParamSurface, SurfaceForm, SurfaceKind, centred, offset_from, partial_index,
};

/// The most offsets stacked on one another: an offset whose base is an offset nested this deep is
/// refused (the module's `Recursion` field).
pub const MAX_NESTING: usize = 4;

/// How many offsets stand on one another at the top of `k` (0 for any other kind): read down the
/// chain in a loop, without recursion.
fn nesting(k: &SurfaceKind) -> usize {
    let mut depth: usize = 0;
    let mut at = k;
    while let SurfaceKind::Offset(o) = at {
        depth = depth.saturating_add(1);
        at = &o.base;
    }
    depth
}

/// How many pieces each smooth part of a box is cut into, per direction, for a bound.
const PIECES: usize = 4;

/// How many times a piece of a normal bound is cut into four while its fold factor's sign is
/// undecided.
const HALVINGS: usize = 3;

/// An offset surface (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Offset {
    base: Box<SurfaceKind>,
    domain: ParamBox,
    distance: f64,
    cache: Option<(SurfaceForm, f64)>,
}

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

/// A parameter pair inside the domain.
fn check_point((u, v): (f64, f64), ((a, b), (c, d)): ParamBox) -> Result<(), CurveError> {
    if u.is_finite() && v.is_finite() && a <= u && u <= b && c <= v && v <= d {
        Ok(())
    } else {
        Err(CurveError::ParameterOutside)
    }
}

/// [lo, hi] cut at the breaks strictly inside it, each part into `n` equal pieces.
fn cuts((lo, hi): (f64, f64), breaks: &[f64], n: usize) -> Result<Vec<(f64, f64)>, CurveError> {
    let mut ends = alloc::vec![lo];
    ends.extend(breaks.iter().copied().filter(|&x| lo < x && x < hi));
    ends.push(hi);
    let count = u32::try_from(n).map_err(|_| CurveError::Defect)?;
    let mut out = Vec::new();
    for w in ends.windows(2) {
        let (Some(&a), Some(&b)) = (w.first(), w.get(1)) else {
            return Err(CurveError::Defect);
        };
        let mut start = a;
        for i in 1..=count {
            let end = if i == count {
                b
            } else {
                a + (b - a) * (f64::from(i) / f64::from(count))
            };
            // Rounding may put a step's end a hair before its start: never a reversed piece.
            let end = tight::larger(end, start);
            out.push((start, end));
            start = end;
        }
    }
    Ok(out)
}

/// The union of two boxes, the first absent before any.
fn union(a: Option<Bounds<3>>, b: &Bounds<3>) -> Result<Bounds<3>, CurveError> {
    match a {
        None => Ok(*b),
        Some(a) => Ok(a.union(b)?),
    }
}

/// The smallest parameter box holding `a` (if any) and `b`.
fn span(a: Option<ParamBox>, b: ParamBox) -> ParamBox {
    let Some(((u0, u1), (v0, v1))) = a else {
        return b;
    };
    let ((x0, x1), (y0, y1)) = b;
    (
        (tight::smaller(u0, x0), tight::larger(u1, x1)),
        (tight::smaller(v0, y0), tight::larger(v1, y1)),
    )
}

/// The box of three intervals.
fn bounds_of(x: &[Interval; 3]) -> Bounds<3> {
    Bounds {
        lo: x.map(Interval::lo),
        hi: x.map(Interval::hi),
    }
}

/// a × b, for jets.
fn cross(a: &[Jet; 3], b: &[Jet; 3]) -> Result<[Jet; 3], CurveError> {
    let [a0, a1, a2] = a;
    let [b0, b1, b2] = b;
    Ok([
        a1.mul(b2)?.sub(&a2.mul(b1)?)?,
        a2.mul(b0)?.sub(&a0.mul(b2)?)?,
        a0.mul(b1)?.sub(&a1.mul(b0)?)?,
    ])
}

/// a × b, for enclosed vectors.
fn cross_enclosed(a: &[Interval; 3], b: &[Interval; 3]) -> Result<[Interval; 3], CurveError> {
    let [a0, a1, a2] = *a;
    let [b0, b1, b2] = *b;
    Ok([
        tight::sub(tight::mul(a1, b2)?, tight::mul(a2, b1)?)?,
        tight::sub(tight::mul(a2, b0)?, tight::mul(a0, b2)?)?,
        tight::sub(tight::mul(a0, b1)?, tight::mul(a1, b0)?)?,
    ])
}

impl Offset {
    /// The offset of `base` by `distance` along its normal `S_u` × `S_v`, over the base's whole
    /// domain.
    ///
    /// # Errors
    ///
    /// [`CurveError::NotFinite`] for a distance that is not finite; [`CurveError::TooDeep`] for a
    /// base that is already [`MAX_NESTING`] offsets deep.
    pub fn new(base: SurfaceKind, distance: f64) -> Result<Offset, CurveError> {
        if !distance.is_finite() {
            return Err(CurveError::NotFinite);
        }
        let depth = nesting(&base);
        if depth >= MAX_NESTING {
            return Err(CurveError::TooDeep {
                depth: depth.saturating_add(1),
            });
        }
        let domain = base.domain();
        Ok(Offset {
            base: Box::new(base),
            domain,
            distance,
            cache: None,
        })
    }

    /// The base.
    #[must_use]
    pub fn base(&self) -> &SurfaceKind {
        &self.base
    }

    /// The distance.
    #[must_use]
    pub fn distance(&self) -> f64 {
        self.distance
    }

    /// The offset with its NURBS cache computed and kept: later conversions return it.
    ///
    /// # Errors
    ///
    /// As [`ParamSurface::to_nurbs`].
    pub fn cached(&self, meter: &mut WorkMeter<'_>) -> Result<Offset, CurveError> {
        let c = self.to_nurbs(meter)?;
        Ok(Offset {
            base: self.base.clone(),
            domain: self.domain,
            distance: self.distance,
            cache: Some((c.value, c.deviation)),
        })
    }

    /// The cache, if kept: the NURBS form and its certified deviation.
    #[must_use]
    pub fn cache(&self) -> Option<(&SurfaceForm, f64)> {
        self.cache.as_ref().map(|(n, d)| (n, *d))
    }

    /// The NURBS form on the same parameters within `tolerance` of the offset everywhere in the
    /// domain, with its certified deviation (at most `tolerance`): Piegl and Tiller's method
    /// (`crate::cache`), sampling until the certified error fits, then removing knots within
    /// what is left.
    ///
    /// # Errors
    ///
    /// [`geom_nurbs::NurbsError::Tolerance`] for a tolerance that is not positive and finite;
    /// [`geom_nurbs::NurbsError::OutsideTolerance`] if the densest sampling cannot be certified
    /// within it; [`CurveError::Unbounded`] for an unbounded domain, or where the base's normal
    /// may vanish; as [`ParamSurface::partials_over`].
    pub fn to_nurbs_within(
        &self,
        tolerance: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceForm>, CurveError> {
        cache::same_parameter(self, Some(tolerance), meter)
    }

    /// The offset's jets per coordinate to `order` over `region` (a point or a box inside the
    /// domain): S and its partials from the base's jets of order + 1, n = N/|N| with
    /// N = `S_u` × `S_v`, and O = S + d·n.
    fn jets(
        &self,
        region: ParamBox,
        sides: (Limit, Limit),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<[Jet; 3], CurveError> {
        let k = order.checked_add(1).ok_or(CurveError::Defect)?;
        let b = self.base.partials_over(region, sides, k, meter)?;
        let column = |d: usize| -> Result<Jet, CurveError> {
            let col: Vec<Interval> = b
                .iter()
                .map(|x| x.get(d).copied().ok_or(CurveError::Defect))
                .collect::<Result<_, _>>()?;
            Jet::from_partials(&col, k)
        };
        let s = [column(0)?, column(1)?, column(2)?];
        let [s0, s1, s2] = &s;
        let su = [s0.du()?, s1.du()?, s2.du()?];
        let sv = [s0.dv()?, s1.dv()?, s2.dv()?];
        let n = cross(&su, &sv)?;
        let [n0, n1, n2] = &n;
        let len = n0.mul(n0)?.add(&n1.mul(n1)?)?.add(&n2.mul(n2)?)?.sqrt()?;
        let d = point(self.distance)?;
        let mut out = Vec::with_capacity(3);
        for (si, ni) in s.iter().zip(&n) {
            out.push(si.truncate(order)?.add(&ni.div(&len)?.scale(d)?)?);
        }
        let [o0, o1, o2]: [Jet; 3] = out.try_into().map_err(|_| CurveError::Defect)?;
        Ok([o0, o1, o2])
    }

    /// The offset's partials to `order` over `region`, listed by `partial_index`, straight from
    /// the jets over the region.
    fn direct(
        &self,
        region: ParamBox,
        sides: (Limit, Limit),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; 3]>, CurveError> {
        let jets = self.jets(region, sides, order, meter)?;
        let mut out = alloc::vec![[point(0.0)?; 3]; partial_index(order.saturating_add(1), 0)?];
        for (d, j) in jets.iter().enumerate() {
            for (o, x) in out.iter_mut().zip(j.partials()?) {
                if let Some(slot) = o.get_mut(d) {
                    *slot = x;
                }
            }
        }
        Ok(out)
    }

    /// The offset's partials to `order` over `region`, listed by `partial_index`. Over a region
    /// wider than a point with no break inside, each is the intersection of its direct enclosure
    /// with its centred form (`crate::param::centred`), so small regions get tight bounds.
    fn partials_of(
        &self,
        region: ParamBox,
        sides: (Limit, Limit),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; 3]>, CurveError> {
        let ((u0, u1), (v0, v1)) = region;
        let (bu, bv) = self.breaks();
        let in_u = bu.iter().any(|&x| u0 < x && x < u1);
        let in_v = bv.iter().any(|&x| v0 < x && x < v1);
        if (u0 == u1 && v0 == v1) || in_u || in_v {
            return self.direct(region, sides, order, meter);
        }
        let up = order.checked_add(1).ok_or(CurveError::Defect)?;
        let wide = self.direct(region, sides, up, meter)?;
        let (cu, cv) = (f64::midpoint(u0, u1), f64::midpoint(v0, v1));
        let at = self.direct(((cu, cu), (cv, cv)), sides, order, meter)?;
        let steps = (offset_from((u0, u1), cu)?, offset_from((v0, v1), cv)?);
        centred(&at, &wide, steps, order)
    }

    /// The sign of 1 − 2Hd + Kd² over `region`, if certain: the factor in
    /// `O_u` × `O_v` = (1 − 2Hd + Kd²)·(`S_u` × `S_v`). It is det(I − d·W) for the shape operator
    /// W = I⁻¹·II, so its sign is that of det(I − d·II) = (E − dL)(G − dN) − (F − dM)², with E, F and
    /// G the first fundamental form's coefficients and L, M and N the second's (the partials
    /// `S_uu`, `S_uv` and `S_vv` against the unit normal ν/|ν|, ν = `S_u` × `S_v`), since
    /// det I = EG − F² > 0; written so, it cancels far less in intervals than the expanded sum.
    fn factor_sign(
        &self,
        region: ParamBox,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<bool>, CurveError> {
        let b =
            match self
                .base
                .partials_over(region, (Limit::FromRight, Limit::FromRight), 2, meter)
            {
                Ok(b) => b,
                Err(CurveError::Unbounded) => return Ok(None),
                Err(e) => return Err(e),
            };
        let [_, su, sv, suu, suv, svv] = b.as_slice() else {
            return Err(CurveError::Defect);
        };
        let dot = |a: &[Interval; 3], b: &[Interval; 3]| -> Result<Interval, CurveError> {
            let [a0, a1, a2] = *a;
            let [b0, b1, b2] = *b;
            tight::add(
                tight::add(tight::mul(a0, b0)?, tight::mul(a1, b1)?)?,
                tight::mul(a2, b2)?,
            )
        };
        let nu = cross_enclosed(su, sv)?;
        let n2 = dot(&nu, &nu)?;
        if n2.lo() <= 0.0 {
            return Ok(None);
        }
        // d/|ν|, the distance over the normal's length: the second form against ν, times it.
        let k = tight::div(point(self.distance)?, tight::sqrt(n2)?)?;
        let first = |x: Interval, s: &[Interval; 3]| -> Result<Interval, CurveError> {
            tight::sub(x, tight::mul(k, dot(s, &nu)?)?)
        };
        let a = first(dot(su, su)?, suu)?;
        let c = first(dot(su, sv)?, suv)?;
        let g = first(dot(sv, sv)?, svv)?;
        let det = tight::sub(tight::mul(a, g)?, tight::mul(c, c)?)?;
        Ok(if det.lo() > 0.0 {
            Some(true)
        } else if det.hi() < 0.0 {
            Some(false)
        } else {
            None
        })
    }

    /// The pieces of a box inside the domain: each smooth part cut into `PIECES` by `PIECES`.
    fn pieces(&self, (ru, rv): ParamBox) -> Result<Vec<ParamBox>, CurveError> {
        let (bu, bv) = self.breaks();
        let mut out = Vec::new();
        for u in cuts(ru, &bu, PIECES)? {
            for v in cuts(rv, &bv, PIECES)? {
                out.push((u, v));
            }
        }
        Ok(out)
    }

    /// The NURBS cache, reversed or moved, as a NURBS kind with both deviations.
    fn moved_cache(
        &self,
        motion: Option<&TypedTransform>,
        dir: Direction,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        let n = self.to_nurbs(meter)?;
        let s = &n.value.surface;
        let moved = match motion {
            Some(t) => ParamSurface::transformed(s, t, meter)?,
            None => ParamSurface::reversed(s, dir, meter)?,
        };
        Ok(Changed {
            value: moved.value,
            deviation: tight::add(point(n.deviation)?, point(moved.deviation)?)?.hi(),
        })
    }
}

impl ParamSurface for Offset {
    fn domain(&self) -> ParamBox {
        self.domain
    }

    fn point(&self, u: f64, v: f64, sides: (Limit, Limit)) -> Result<[f64; 3], CurveError> {
        let [p, _, _] = self.partials(u, v, sides)?;
        Ok(p)
    }

    fn partials(&self, u: f64, v: f64, sides: (Limit, Limit)) -> Result<[[f64; 3]; 3], CurveError> {
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
        check_point((u, v), self.domain)?;
        let p = self.partials_of(((u, u), (v, v)), sides, 1, meter)?;
        match p.as_slice() {
            [s, su, sv] => Ok([*s, *su, *sv]),
            _ => Err(CurveError::Defect),
        }
    }

    fn bound(&self, region: ParamBox, meter: &mut WorkMeter<'_>) -> Result<Bounds<3>, CurveError> {
        check_box(region, self.domain)?;
        let sides = (Limit::FromRight, Limit::FromRight);
        let mut out = None;
        for piece in self.pieces(region)? {
            let b = match self.partials_of(piece, sides, 0, meter) {
                Ok(p) => bounds_of(p.first().ok_or(CurveError::Defect)?),
                Err(CurveError::Unbounded) => {
                    // The normal may vanish: the base's box widened by |d|.
                    let mut b = self.base.bound(piece, meter)?;
                    let r = point(self.distance.abs())?;
                    for (l, h) in b.lo.iter_mut().zip(b.hi.iter_mut()) {
                        *l = tight::sub(point(*l)?, r)?.lo();
                        *h = tight::add(point(*h)?, r)?.hi();
                    }
                    b
                }
                Err(e) => return Err(e),
            };
            out = Some(union(out, &b)?);
        }
        out.ok_or(CurveError::Defect)
    }

    fn derivative_bound(
        &self,
        region: ParamBox,
        dir: Direction,
        meter: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<3>, CurveError> {
        check_box(region, self.domain)?;
        let sides = (Limit::FromRight, Limit::FromRight);
        let k = match dir {
            Direction::U => 1,
            Direction::V => 2,
        };
        let mut out = None;
        for piece in self.pieces(region)? {
            let p = match self.partials_of(piece, sides, 1, meter) {
                Ok(p) => p,
                Err(CurveError::Unbounded) => return Ok(DerivativeBound::Unbounded),
                Err(e) => return Err(e),
            };
            out = Some(union(out, &bounds_of(p.get(k).ok_or(CurveError::Defect)?))?);
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
        check_box(region, self.domain)?;
        self.partials_of(region, sides, order, meter)
    }

    fn breaks(&self) -> (Vec<f64>, Vec<f64>) {
        let ((a, b), (c, d)) = self.domain;
        let (mut bu, mut bv) = self.base.breaks();
        bu.retain(|&x| a < x && x < b);
        bv.retain(|&x| c < x && x < d);
        (bu, bv)
    }

    fn normal_bound(
        &self,
        region: ParamBox,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Bounds<3>, CurveError> {
        check_box(region, self.domain)?;
        let sides = (Limit::FromRight, Limit::FromRight);
        // The boxes spanning the pieces where the factor is certainly positive and certainly
        // negative: the base's normals over each are asked for once, so a chain of offsets costs
        // two of its base's normal bounds per level, not one per piece.
        let (mut kept, mut turned): (Option<ParamBox>, Option<ParamBox>) = (None, None);
        let mut out = None;
        // An explicit stack of (box, halvings left): a box whose factor's sign is undecided is cut
        // into four, at most `HALVINGS` times, before the cross product decides for it.
        let mut stack: Vec<(ParamBox, usize)> = self
            .pieces(region)?
            .into_iter()
            .map(|p| (p, HALVINGS))
            .collect();
        while let Some((piece, left)) = stack.pop() {
            match (self.factor_sign(piece, meter)?, left.checked_sub(1)) {
                (Some(true), _) => kept = Some(span(kept, piece)),
                (Some(false), _) => turned = Some(span(turned, piece)),
                (None, Some(next)) => {
                    let ((u0, u1), (v0, v1)) = piece;
                    let (um, vm) = (f64::midpoint(u0, u1), f64::midpoint(v0, v1));
                    for u in [(u0, um), (um, u1)] {
                        for v in [(v0, vm), (vm, v1)] {
                            stack.push(((u, v), next));
                        }
                    }
                }
                (None, None) => {
                    let n = match self.partials_of(piece, sides, 1, meter) {
                        Ok(p) => match p.as_slice() {
                            [_, su, sv] => bounds_of(&cross_enclosed(su, sv)?),
                            _ => return Err(CurveError::Defect),
                        },
                        // Where the base's normal may vanish, nothing is decided.
                        Err(CurveError::Unbounded) => {
                            return Ok(Bounds {
                                lo: [f64::NEG_INFINITY; 3],
                                hi: [f64::INFINITY; 3],
                            });
                        }
                        Err(e) => return Err(e),
                    };
                    out = Some(union(out, &n)?);
                }
            }
        }
        // Where the factor is positive the offset's normal is a positive multiple of the base's;
        // where negative, of its opposite.
        if let Some(b) = kept {
            out = Some(union(out, &self.base.normal_bound(b, meter)?)?);
        }
        if let Some(b) = turned {
            let n = self.base.normal_bound(b, meter)?;
            let flipped = Bounds {
                lo: n.hi.map(|x| -x),
                hi: n.lo.map(|x| -x),
            };
            out = Some(union(out, &flipped)?);
        }
        out.ok_or(CurveError::Defect)
    }

    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<SurfaceForm>, CurveError> {
        if let Some((n, d)) = &self.cache {
            return Ok(Changed {
                value: n.clone(),
                deviation: *d,
            });
        }
        cache::same_parameter(self, None, meter)
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
        let (dl, dr) = match dir {
            Direction::U => (((u0, t), (v0, v1)), ((t, u1), (v0, v1))),
            Direction::V => (((u0, u1), (v0, t)), ((u0, u1), (t, v1))),
        };
        let part = |domain: ParamBox| Changed {
            value: SurfaceKind::Offset(Offset {
                base: self.base.clone(),
                domain,
                distance: self.distance,
                cache: None,
            }),
            deviation: 0.0,
        };
        Ok((part(dl), part(dr)))
    }

    fn reversed(
        &self,
        dir: Direction,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        let b = self.base.reversed(dir, meter)?;
        if b.deviation != 0.0 {
            return self.moved_cache(None, dir, meter);
        }
        let ((u0, u1), (v0, v1)) = self.domain;
        let domain = match dir {
            Direction::U => ((-u1, -u0), (v0, v1)),
            Direction::V => ((u0, u1), (-v1, -v0)),
        };
        Ok(Changed {
            value: SurfaceKind::Offset(Offset {
                base: Box::new(b.value),
                domain,
                distance: -self.distance,
                cache: None,
            }),
            deviation: 0.0,
        })
    }

    fn transformed(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        if let TypedTransform::Exact(g) = transform {
            let b = self.base.transformed(transform, meter)?;
            if b.deviation == 0.0 {
                let distance = if g.reverses_orientation() {
                    -self.distance
                } else {
                    self.distance
                };
                return Ok(Changed {
                    value: SurfaceKind::Offset(Offset {
                        base: Box::new(b.value),
                        domain: self.domain,
                        distance,
                        cache: None,
                    }),
                    deviation: 0.0,
                });
            }
        }
        self.moved_cache(Some(transform), Direction::U, meter)
    }
}
