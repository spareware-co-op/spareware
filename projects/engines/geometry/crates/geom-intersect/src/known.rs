//! Intersections known by construction: a curve two surfaces are known to meet along, taken out of
//! the general search only where nothing else is proven to meet near it (12.11).
//!
//! What it does: a fillet's contact curves, a sweep's seams and an extrusion's side edges arrive as
//! [`Known`] curves, each with its certified bound and its contact. [`subdivide_known`] proves a
//! tube around each stretch of one, cell by cell, and runs [`crate::general::subdivide`]'s search
//! with every pair that lies inside a proven cell taken out: there the known curve is all the two
//! surfaces share. A pair that meets the curve but no cell holds is searched further, and flagged
//! [`Unresolved::KnownCurveUnproven`] at the floor; a stretch no cell covers is searched as any
//! other, and what stays undecided there is flagged the same way.
//!
//! Guarantees:
//!
//! - **Only a proven tube is excluded.** A cell is taken out only with its proof:
//!   - A crossing: the system of 12.8 has a regular interval Jacobian over the cell, so for every
//!     value c of the coordinate the tangent moves along, the cell holds at most one common point
//!     (`march::unique_over`). The curve crosses the cell's slab of c, so within it the curve is
//!     every common point.
//!   - A tangent: across the curve, the other surface's equation read on this one, g(s), has
//!     g(0) = 0 and g′(0) = 0 by construction, and its second derivative is proven of one sign over
//!     the cell, so g(s) = ½·g″(ξ)·s² has no other zero there (Taylor's theorem with the
//!     remainder in an interval). This needs one surface's implicit form with its second
//!     derivatives (`geom_surface::ImplicitBound::hessian_box`), so it serves a pair with an
//!     analytic surface.
//! - **The tube is never wider than its proof.** Each cell starts wide and halves until it is
//!   proven or narrower than the floor. A second intersection d away from the curve fails every
//!   cell reaching past it, so it lies outside the tube and the search finds it or flags it.
//! - **Nothing is dropped.** A pair is taken out only inside a proven cell; anything else the
//!   search settles, splits or flags, and a flag near a known curve says
//!   [`Unresolved::KnownCurveUnproven`].
//!
//! Method: chapter 12.11 (`projects/engines/geometry/architecture/12-intersections.md`). Uniqueness
//! from a regular interval matrix: ‖I − Y·A‖ < 1 for every A of the enclosure makes each A
//! nonsingular (A. Neumaier, *Interval Methods for Systems of Equations*, Cambridge University
//! Press (1990), chapter 4, <https://doi.org/10.1017/CBO9780511526473>), and the mean value
//! theorem row by row turns that into one zero. The tangent test is Taylor's theorem with the
//! remainder enclosed (R. E. Moore, R. B. Kearfott and M. J. Cloud, *Introduction to Interval
//! Analysis*, SIAM (2009), <https://doi.org/10.1137/1.9780898717716>).
//!
//! Cost: per stretch of a known curve, at most log₂ of its first width over the floor tries, each
//! one Jacobian or one second-order read; each pair of the search then tests its box against the
//! cells, linear in them. Recursion: none.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are chapter 12.11's and 12.8's: u, v and s, r the surfaces' parameters, c the coordinate a tangent moves along, g the separation"
)]

use alloc::vec::Vec;

use geom_curve::tight::{larger, smaller};
use geom_num::interval::Interval;
use geom_run::WorkMeter;
use geom_surface::{ImplicitBound, ParamBox as Region, ParamSurface, SurfaceKind, partial_index};

use crate::IntersectError;
use crate::general::{
    Leave, Subdivided, around, ends, partials, speeds, subdivide_from, subdivide_with,
};
use crate::march::unique_over;
use crate::project::{dot, negative, positive};
use crate::result::Unresolved;
use crate::seam::{cells, cells_in, charts};

/// How two surfaces meet along a known curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnownContact {
    /// They cross at an angle along it.
    Crossing,
    /// They touch along it: tangent planes agree at every point of it.
    Tangent,
}

/// A curve two surfaces are known to meet along, as whoever made it gives it (12.11).
#[derive(Clone, Debug, PartialEq)]
pub struct Known {
    /// Enclosures of points on it, (u₁, v₁, u₂, v₂), in order along it, as
    /// [`crate::march::Branch`] holds them.
    pub vertices: Vec<[Interval; 4]>,
    /// A bound, in space, on how far the curve is from the chords through its vertices.
    pub bound: f64,
    /// How the surfaces meet along it.
    pub contact: KnownContact,
}

/// One proven cell of a tube.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Cell {
    /// In `space`, (u₁, v₁, u₂, v₂), wherever the first surface's coordinate `axis` lies in
    /// `slab`, the known curve is every common point.
    Crossing {
        space: [Interval; 4],
        axis: usize,
        slab: (f64, f64),
    },
    /// Over `region` of the first surface (the second's if `second`), the other surface's
    /// equation vanishes only on the known curve.
    Tangent { second: bool, region: Region },
}

/// The proven tube of a pair's known curves (12.11), and every stretch of them.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Tube {
    cells: Vec<Cell>,
    /// Each stretch's hull, widened by its curve's bound, and whether a cell covers it.
    slim: Vec<([Interval; 4], bool)>,
}

/// Whether `x` lies within `outer`.
fn within(x: Interval, outer: Interval) -> bool {
    outer.lo() <= x.lo() && x.hi() <= outer.hi()
}

/// Whether two boxes share a point.
fn meets(a: &[Interval; 4], b: &[Interval; 4]) -> bool {
    a.iter()
        .zip(b)
        .all(|(x, y)| x.lo() <= y.hi() && y.lo() <= x.hi())
}

/// The pair `(a, b)` as one box of (u₁, v₁, u₂, v₂).
fn boxed((a, b): (Region, Region)) -> Result<[Interval; 4], IntersectError> {
    let ((p, q), (r, s)) = a;
    let ((t, u), (v, w)) = b;
    Ok([
        Interval::new(p, q)?,
        Interval::new(r, s)?,
        Interval::new(t, u)?,
        Interval::new(v, w)?,
    ])
}

impl Tube {
    /// Whether a proven cell holds the whole pair `(a, b)`, so that every point it shares is on a
    /// known curve.
    pub(crate) fn holds(
        &self,
        first: &SurfaceKind,
        pair: (Region, Region),
        meter: &mut WorkMeter<'_>,
    ) -> Result<bool, IntersectError> {
        let at = boxed(pair)?;
        for cell in &self.cells {
            match *cell {
                Cell::Crossing { space, axis, slab } => {
                    if at.iter().zip(&space).all(|(x, y)| within(*x, *y)) {
                        let image = around(first, pair.0, meter)?;
                        let c = image.get(axis).ok_or(IntersectError::Internal)?;
                        if slab.0 <= c.lo() && c.hi() <= slab.1 {
                            return Ok(true);
                        }
                    }
                }
                Cell::Tangent { second, region } => {
                    let patch = if second { pair.1 } else { pair.0 };
                    let ((a, b), (c, d)) = region;
                    let ((p, q), (r, s)) = patch;
                    if a <= p && q <= b && c <= r && s <= d {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }

    /// Whether the pair meets a stretch of a known curve: `Some(true)` for one a cell covers,
    /// `Some(false)` for one only an unproven stretch reaches, `None` for neither.
    pub(crate) fn near(&self, pair: (Region, Region)) -> Result<Option<bool>, IntersectError> {
        let at = boxed(pair)?;
        let mut found = None;
        for (hull, proven) in &self.slim {
            if meets(&at, hull) {
                if *proven {
                    return Ok(Some(true));
                }
                found = Some(false);
            }
        }
        Ok(found)
    }
}

impl Leave for Tube {
    fn holds(
        &self,
        first: &SurfaceKind,
        pair: (Region, Region),
        meter: &mut WorkMeter<'_>,
    ) -> Result<bool, IntersectError> {
        Tube::holds(self, first, pair, meter)
    }

    fn near(&self, pair: (Region, Region)) -> Result<Option<(bool, Unresolved)>, IntersectError> {
        Ok(Tube::near(self, pair)?.map(|proven| (proven, Unresolved::KnownCurveUnproven)))
    }
}

/// The implicit form of an analytic kind, if it is one.
fn implicit_of(surface: &SurfaceKind) -> Option<&dyn ImplicitBound> {
    match surface {
        SurfaceKind::Plane(s) => Some(s),
        SurfaceKind::Sphere(s) => Some(s),
        SurfaceKind::Cylinder(s) => Some(s),
        SurfaceKind::Cone(s) => Some(s),
        SurfaceKind::Torus(s) => Some(s),
        _ => None,
    }
}

/// The hull of `points`, each coordinate widened by `pad` over its speed, inside `domains`.
fn hull(
    points: &[[Interval; 4]],
    pad: f64,
    speeds: [f64; 4],
    domains: [Region; 2],
) -> Result<Option<[Interval; 4]>, IntersectError> {
    let [((a0, a1), (b0, b1)), ((c0, c1), (d0, d1))] = domains;
    let ends = [(a0, a1), (b0, b1), (c0, c1), (d0, d1)];
    let Some(first) = points.first() else {
        return Ok(None);
    };
    let mut out = *first;
    for p in points {
        for (o, x) in out.iter_mut().zip(p) {
            *o = Interval::new(smaller(o.lo(), x.lo()), larger(o.hi(), x.hi()))?;
        }
    }
    for ((o, speed), (lo, hi)) in out.iter_mut().zip(speeds).zip(ends) {
        let room = pad / speed;
        *o = Interval::new(larger(o.lo() - room, lo), smaller(o.hi() + room, hi))?;
    }
    Ok(Some(out))
}

/// How far, in space, a box of parameters reaches along its widest parameter.
fn reach(b: &[Interval; 4], speeds: [f64; 4]) -> f64 {
    b.iter()
        .zip(speeds)
        .fold(0.0, |most, (x, s)| larger(most, (x.hi() - x.lo()) * s))
}

/// A crossing cell about `stretch`, the widest proven, or `None` if none wider than the floor is.
fn crossing_cell(
    (first, second): (&SurfaceKind, &SurfaceKind),
    (stretch, ends_curve): (&[[Interval; 4]], (bool, bool)),
    (bound, floor): (f64, f64),
    (speeds, domains): ([f64; 4], [Region; 2]),
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Cell>, IntersectError> {
    let (Some(start), Some(end)) = (stretch.first(), stretch.last()) else {
        return Ok(None);
    };
    let Some(slim) = hull(stretch, bound, speeds, domains)? else {
        return Ok(None);
    };
    let mut rho = larger(reach(&slim, speeds), floor) * 2.0;
    while rho >= floor {
        meter.charge(1)?;
        let Some(space) = hull(stretch, bound + rho, speeds, domains)? else {
            return Ok(None);
        };
        if let Some(axis) = unique_over(first, second, &space, meter)? {
            let at = |x: &[Interval; 4],
                      meter: &mut WorkMeter<'_>|
             -> Result<Interval, IntersectError> {
                // A vertex is a point, read by its enclosure, not by a bound over a box, which a
                // kind may give wider than the point.
                let [u, v, ..] = *x;
                let read = partials(first, (ends(u), ends(v)), 0, meter)?;
                let at = read.as_ref().and_then(|r| r.first());
                at.and_then(|p| p.get(axis))
                    .copied()
                    .ok_or(IntersectError::Internal)
            };
            let (c0, c1) = (at(start, meter)?, at(end, meter)?);
            // The curve crosses the cell between its stretch's ends, so the slab is what lies
            // between them, their enclosures left out. A neighbour's slab covers what lies past an
            // end inside the curve; past the curve's own end it reaches a floor further, which is
            // all its end's enclosure can hold and all the floor tells apart (12.3).
            let (past_start, past_end) = ends_curve;
            let reach = |past: bool| if past { floor } else { 0.0 };
            let slab = if c0.hi() < c1.lo() {
                (c0.hi() - reach(past_start), c1.lo() + reach(past_end))
            } else if c1.hi() < c0.lo() {
                (c1.hi() - reach(past_end), c0.lo() + reach(past_start))
            } else {
                return Ok(None);
            };
            return Ok(Some(Cell::Crossing { space, axis, slab }));
        }
        rho /= 2.0;
    }
    Ok(None)
}

/// Whether, over `region` of `surface`, `implicit` read on it has a second derivative of one sign
/// along the parameter `across` (0 for u): g″ = S′ᵀ·∇²F·S′ + ∇F·S″, with S′ and S″ the partials of
/// the surface across.
fn bends_one_way(
    implicit: &dyn ImplicitBound,
    surface: &SurfaceKind,
    region: Region,
    across: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, IntersectError> {
    let Some(found) = partials(surface, region, 2, meter)? else {
        return Ok(false);
    };
    let (once, twice) = if across == 0 {
        (partial_index(1, 0)?, partial_index(2, 0)?)
    } else {
        (partial_index(0, 1)?, partial_index(0, 2)?)
    };
    let (Some(d1), Some(d2)) = (found.get(once), found.get(twice)) else {
        return Err(IntersectError::Internal);
    };
    let box_ = around(surface, region, meter)?;
    let Some(h) = implicit.hessian_box(box_)? else {
        return Ok(false);
    };
    let gradient = implicit.enclose_box(box_)?.gradient;
    let mut quadratic = Interval::point(0.0)?;
    for (row, di) in h.iter().zip(d1) {
        quadratic = quadratic.add(dot(row, d1)?.mul(*di)?)?;
    }
    let second = quadratic.add(dot(&gradient, d2)?)?;
    Ok(positive(second) || negative(second))
}

/// A tangent cell about `stretch` on the surface the band lies on (the second if `side`), the
/// widest proven, or `None`.
fn tangent_cell(
    (surface, implicit, side): (&SurfaceKind, &dyn ImplicitBound, bool),
    stretch: &[[Interval; 4]],
    (bound, floor): (f64, f64),
    speeds: [f64; 2],
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Cell>, IntersectError> {
    let on = |x: &[Interval; 4]| -> [Interval; 2] {
        let [a, b, c, d] = *x;
        if side { [c, d] } else { [a, b] }
    };
    let points: Vec<[Interval; 2]> = stretch.iter().map(on).collect();
    let (Some(start), Some(end)) = (points.first(), points.last()) else {
        return Ok(None);
    };
    let [su, sv] = speeds;
    let [[u0, v0], [u1, v1]] = [*start, *end];
    let (du, dv) = (
        (u1.lo() - u0.lo()).abs() * su,
        (v1.lo() - v0.lo()).abs() * sv,
    );
    let along = usize::from(dv > du);
    let pick = |p: &[Interval; 2], k: usize| {
        let [u, v] = *p;
        if k == 0 { u } else { v }
    };
    // The curve must run one way along the parameter it follows, so that every point of the cell
    // has a point of the curve straight across from it.
    let increasing = pick(start, along).hi() < pick(end, along).lo();
    for pair in points.windows(2) {
        let (Some(a), Some(b)) = (pair.first(), pair.last()) else {
            return Ok(None);
        };
        let (x, y) = (pick(a, along), pick(b, along));
        let ordered = if increasing {
            x.hi() < y.lo()
        } else {
            y.hi() < x.lo()
        };
        if !ordered {
            return Ok(None);
        }
    }
    let (a, b) = (pick(start, along), pick(end, along));
    let length = if increasing {
        (a.hi(), b.lo())
    } else {
        (b.hi(), a.lo())
    };
    let across = usize::from(along == 0);
    let speed = if across == 0 { su } else { sv };
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in &points {
        let x = pick(p, across);
        lo = smaller(lo, x.lo());
        hi = larger(hi, x.hi());
    }
    let ((d0, d1), (e0, e1)) = surface.domain();
    let (dlo, dhi) = if across == 0 { (d0, d1) } else { (e0, e1) };
    let run = (length.1 - length.0) * if along == 0 { su } else { sv };
    let mut rho = larger((hi - lo) * speed + run, floor) * 2.0;
    while rho >= floor {
        meter.charge(1)?;
        let room = (bound + rho) / speed;
        let wide = (larger(lo - room, dlo), smaller(hi + room, dhi));
        let region = if along == 0 {
            (length, wide)
        } else {
            (wide, length)
        };
        if bends_one_way(implicit, surface, region, across, meter)? {
            return Ok(Some(Cell::Tangent {
                second: side,
                region,
            }));
        }
        rho /= 2.0;
    }
    Ok(None)
}

impl Tube {
    /// The tube of `known` on `first` and `second`, each stretch proven as its contact asks.
    ///
    /// # Errors
    ///
    /// The meter's stop; [`IntersectError::UnprovenMaths`] where a kind's bound needs one.
    pub(crate) fn prove(
        first: &SurfaceKind,
        second: &SurfaceKind,
        known: &[Known],
        floor: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Tube, IntersectError> {
        let ([a, b], [c, d]) = (
            speeds(first, first.domain(), meter)?,
            speeds(second, second.domain(), meter)?,
        );
        let speeds4 = [a, b, c, d];
        let domains = [first.domain(), second.domain()];
        // The band lies on the surface whose partner has an implicit form.
        let band = match (implicit_of(second), implicit_of(first)) {
            (Some(f), _) => Some((first, f, false, [a, b])),
            (None, Some(f)) => Some((second, f, true, [c, d])),
            (None, None) => None,
        };
        let mut tube = Tube::default();
        for curve in known {
            let n = curve.vertices.len();
            for k in 0..n.saturating_sub(1) {
                let (Some(seg), Some(stretch)) = (
                    curve.vertices.get(k..k.saturating_add(2)),
                    curve
                        .vertices
                        .get(k.saturating_sub(1)..n.min(k.saturating_add(3))),
                ) else {
                    return Err(IntersectError::Internal);
                };
                let Some(slim) = hull(seg, curve.bound, speeds4, domains)? else {
                    return Err(IntersectError::Internal);
                };
                let cell = match (curve.contact, band) {
                    (KnownContact::Crossing, _) => crossing_cell(
                        (first, second),
                        (stretch, (k == 0, k.saturating_add(2) >= n)),
                        (curve.bound, floor),
                        (speeds4, domains),
                        meter,
                    )?,
                    (KnownContact::Tangent, Some((surface, f, side, two))) => tangent_cell(
                        (surface, f, side),
                        stretch,
                        (curve.bound, floor),
                        two,
                        meter,
                    )?,
                    (KnownContact::Tangent, None) => None,
                };
                tube.slim.push((slim, cell.is_some()));
                if let Some(cell) = cell {
                    tube.cells.push(cell);
                }
            }
        }
        Ok(tube)
    }
}

/// 12.8's subdivision of `first` and `second` with the proven tube about each of `known` taken
/// out (module documentation), to a floor of `floor` in space.
///
/// # Errors
///
/// [`IntersectError::InvalidInput`] for a floor that is not positive;
/// [`IntersectError::BudgetExceeded`] and [`IntersectError::Cancelled`];
/// [`IntersectError::UnprovenMaths`] where a kind's bound needs one.
pub fn subdivide_known(
    first: &SurfaceKind,
    second: &SurfaceKind,
    known: &[Known],
    floor: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Subdivided, IntersectError> {
    if floor.is_nan() || floor <= 0.0 {
        return Err(IntersectError::InvalidInput);
    }
    let tube = Tube::prove(first, second, known, floor, meter)?;
    subdivide_with(first, second, floor, &tube, meter)
}

/// [`subdivide_known`] over `regions` of the two surfaces alone: two faces' parts (8.3, G177).
///
/// # Errors
///
/// As [`subdivide_known`].
pub fn subdivide_known_over(
    (first, second): (&SurfaceKind, &SurfaceKind),
    (a, b): (Region, Region),
    known: &[Known],
    floor: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Subdivided, IntersectError> {
    if floor.is_nan() || floor <= 0.0 {
        return Err(IntersectError::InvalidInput);
    }
    let tube = Tube::prove(first, second, known, floor, meter)?;
    let (one, two) = (
        cells_in(&cells(first, &charts(first)), a),
        cells_in(&cells(second, &charts(second)), b),
    );
    let mut start = Vec::with_capacity(one.len().saturating_mul(two.len()));
    for x in &one {
        for y in &two {
            start.push((*x, *y));
        }
    }
    subdivide_from(first, second, (start, floor), &tube, meter)
}
