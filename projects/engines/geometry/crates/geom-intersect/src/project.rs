//! Projection and inversion: the nearest points of a curve or surface to a point, proven (12.4).
//!
//! What it does: [`project_curve`] and [`project_surface`] find every nearest point of a kind to a
//! point. A line's foot and a plane's are closed forms. Everything else takes Ma and Hewitt's
//! pruning, made certified: a work list of parameter spans, each dropped once a box around it lies
//! further than the best distance so far, or once its distance's derivative cannot vanish, and the
//! ends of a curve and the edges of a surface added as candidates, since a minimum on the boundary
//! need not be a foot. [`invert_curve`] and [`invert_surface`] find the parameters of a point known
//! to lie on a kind. A point on a round kind's axis has a continuum of nearest points, returned as
//! a typed [`Continuum`] rather than found by search.
//!
//! Guarantees:
//!
//! - **The minimum is proven.** Every span dropped has a lower bound above the reported distance,
//!   or a derivative that cannot vanish, by certified bounds on the kind. Nothing is sampled.
//! - **Ties are kept.** Candidates whose distance enclosures overlap are all returned, in parameter
//!   order; [`Projection::tie_broken`] says so, and the first is the lowest parameter.
//! - **Nothing is guessed at the floor.** Spans at the floor that the test cannot decide join into
//!   runs. A run whose image is within twice the floor is one nearest point, since nothing in it can
//!   be told apart on the grid (a sphere's pole, a foot on a split); a longer one is
//!   [`Projection::NonIsolated`], never a foot.
//! - **Enclosures, never numbers.** Each parameter, point and distance is an interval holding the
//!   exact value.
//! - **Deterministic.** Midpoint splits, and a fixed work order, first half first.
//!
//! Method: X. Ma and W. Hewitt, Point inversion and projection for NURBS curve and surface, Computer
//! Aided Geometric Design 20 (2003), <https://doi.org/10.1016/S0167-8396(03)00021-9>, for the
//! pruning, with convex-hull distance replaced by each kind's certified bound; chapter 12.4
//! (`projects/engines/geometry/architecture/12-intersections.md`). A span's convexity test is the
//! sign of the second derivative of half the squared distance, an interval of the kind's certified
//! derivative bounds, so a span it passes holds at most one foot, by Rolle's theorem. Circles,
//! spheres, cylinders, cones and tori go through the same search: their certified bounds are their
//! own closed forms, and the search adds what a closed form alone lacks, a trimmed domain.
//!
//! Cost: spans are halved, so a curve takes O(log(length/floor)) spans per foot, plus those near
//! the distance's other turning points, each a bound on the point and two derivatives. A surface
//! takes four such curves for its edges, then boxes in the same way, each a set of partials to the
//! second order. All of it is charged to the meter. Recursion: none.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are chapter 12.4's and the paper's: p the point, u and v the parameters, a to d a domain's ends"
)]

use alloc::vec::Vec;

use geom_curve::tight::{larger, smaller};
use geom_curve::{CurveError, CurveKind, DerivativeBound, ParamCurve};
use geom_num::NumError;
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Limit};
use geom_run::WorkMeter;
use geom_surface::{ParamSurface, SurfaceKind, partial_index};

use crate::IntersectError;

/// What a span or box of the search costs the meter, besides the kind's own charges.
const STEP: u64 = 1;

/// Where a nearest point lies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// Inside the domain: a foot of the perpendicular.
    Foot,
    /// On the domain's boundary.
    Boundary,
}

/// One nearest point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Nearest<const N: usize, const D: usize> {
    /// Its parameters.
    pub at: [Interval; N],
    /// The point.
    pub point: [Interval; D],
    /// Its distance from the point projected.
    pub distance: Interval,
    /// Where it lies.
    pub place: Place,
}

/// A continuum of nearest points, on a round kind seen from its axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Continuum {
    /// Every point of the curve: a circle from its axis.
    Curve,
    /// Every point of the surface: a sphere from its centre.
    Surface,
    /// Every point at this `v`: a parallel of a surface of revolution from its axis.
    Parallel {
        /// The parallel.
        v: Interval,
    },
}

/// The nearest points of a kind.
#[derive(Clone, Debug, PartialEq)]
pub enum Projection<const N: usize, const D: usize> {
    /// The nearest points, in parameter order.
    Points(Vec<Nearest<N, D>>),
    /// A continuum of them, at this distance.
    Set {
        /// Which.
        set: Continuum,
        /// The distance.
        distance: Interval,
    },
    /// The search reached the floor undecided in this box: the nearest points may form a continuum
    /// on a general kind, such as a point on the axis of a revolved NURBS.
    NonIsolated {
        /// A box of parameters holding what was left undecided.
        enclosure: [Interval; N],
    },
}

impl<const N: usize, const D: usize> Projection<N, D> {
    /// The nearest point at the lowest parameter.
    #[must_use]
    pub fn first(&self) -> Option<&Nearest<N, D>> {
        match self {
            Projection::Points(points) => points.first(),
            Projection::Set { .. } | Projection::NonIsolated { .. } => None,
        }
    }

    /// Whether [`Projection::first`] broke a tie.
    #[must_use]
    pub fn tie_broken(&self) -> bool {
        matches!(self, Projection::Points(points) if points.len() > 1)
    }
}

/// A box from a kind's bounds.
fn boxed<const D: usize>(bounds: &Bounds<D>) -> Result<[Interval; D], IntersectError> {
    let mut out = [Interval::point(0.0)?; D];
    for ((o, lo), hi) in out.iter_mut().zip(bounds.lo).zip(bounds.hi) {
        *o = Interval::new(lo, hi)?;
    }
    Ok(out)
}

/// `x − p`, coordinate by coordinate.
fn minus<const D: usize>(x: &[Interval; D], p: &[f64; D]) -> Result<[Interval; D], IntersectError> {
    let mut out = *x;
    for (o, c) in out.iter_mut().zip(p) {
        *o = o.sub(Interval::point(*c)?)?;
    }
    Ok(out)
}

/// `a·b`.
fn dot<const D: usize>(a: &[Interval; D], b: &[Interval; D]) -> Result<Interval, NumError> {
    a.iter()
        .zip(b)
        .try_fold(Interval::point(0.0)?, |sum, (x, y)| sum.add(x.mul(*y)?))
}

/// `a·a`, tighter than [`dot`] where a coordinate spans zero.
fn square<const D: usize>(a: &[Interval; D]) -> Result<Interval, NumError> {
    a.iter()
        .try_fold(Interval::point(0.0)?, |sum, x| sum.add(x.square()?))
}

/// The widest side of a box.
fn image<const D: usize>(x: &[Interval; D]) -> f64 {
    x.iter().map(|i| i.hi() - i.lo()).fold(0.0, larger)
}

/// Whether an interval holds nothing but positive numbers.
fn positive(x: Interval) -> bool {
    x.lo() > 0.0
}

/// Whether an interval holds nothing but negative numbers.
fn negative(x: Interval) -> bool {
    x.hi() < 0.0
}

/// A kind of one parameter, as the search reads it.
trait Path<const D: usize> {
    /// The parameter domain.
    fn span(&self) -> (f64, f64);
    /// A box around the path over `span`.
    fn around(
        &self,
        span: (f64, f64),
        meter: &mut WorkMeter<'_>,
    ) -> Result<[Interval; D], IntersectError>;
    /// A box around the derivative of `order` over `span`; `None` where it has no bound.
    fn derivative(
        &self,
        span: (f64, f64),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[Interval; D]>, IntersectError>;
    /// Enclosures of the point and its first derivative at `t`, the limit from `side`.
    fn at(
        &self,
        t: f64,
        side: Limit,
        meter: &mut WorkMeter<'_>,
    ) -> Result<([Interval; D], [Interval; D]), IntersectError>;
}

impl<const D: usize> Path<D> for CurveKind<D> {
    fn span(&self) -> (f64, f64) {
        self.domain()
    }

    fn around(
        &self,
        span: (f64, f64),
        meter: &mut WorkMeter<'_>,
    ) -> Result<[Interval; D], IntersectError> {
        boxed(&self.bound(span, meter)?)
    }

    fn derivative(
        &self,
        span: (f64, f64),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[Interval; D]>, IntersectError> {
        match self.derivative_bound(span, order, meter)? {
            DerivativeBound::Bounded(bounds) => Ok(Some(boxed(&bounds)?)),
            DerivativeBound::Unbounded => Ok(None),
        }
    }

    fn at(
        &self,
        t: f64,
        side: Limit,
        meter: &mut WorkMeter<'_>,
    ) -> Result<([Interval; D], [Interval; D]), IntersectError> {
        let found = self.enclose(t, side, 1, meter)?;
        match found.as_slice() {
            [point, first, ..] => Ok((*point, *first)),
            _ => Err(IntersectError::Internal),
        }
    }
}

/// A surface along one parameter line.
struct Line<'a> {
    surface: &'a SurfaceKind,
    /// The fixed parameter's value.
    at: f64,
    /// Whether u is the one that is fixed.
    fixed_u: bool,
}

impl Line<'_> {
    /// The partials of the surface to the second order over `span` of the line.
    fn partials(
        &self,
        span: (f64, f64),
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<Vec<[Interval; 3]>>, IntersectError> {
        let region = if self.fixed_u {
            ((self.at, self.at), span)
        } else {
            (span, (self.at, self.at))
        };
        let sides = (Limit::FromRight, Limit::FromRight);
        match self.surface.partials_over(region, sides, 2, meter) {
            Ok(partials) => Ok(Some(partials)),
            Err(CurveError::Unbounded) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Where the partial of `order` along the line stands in a list of partials.
    fn index(&self, order: usize) -> Result<usize, IntersectError> {
        let (i, j) = if self.fixed_u { (0, order) } else { (order, 0) };
        Ok(partial_index(i, j)?)
    }
}

impl Path<3> for Line<'_> {
    fn span(&self) -> (f64, f64) {
        let (u, v) = self.surface.domain();
        if self.fixed_u { v } else { u }
    }

    fn around(
        &self,
        span: (f64, f64),
        meter: &mut WorkMeter<'_>,
    ) -> Result<[Interval; 3], IntersectError> {
        let region = if self.fixed_u {
            ((self.at, self.at), span)
        } else {
            (span, (self.at, self.at))
        };
        boxed(&self.surface.bound(region, meter)?)
    }

    fn derivative(
        &self,
        span: (f64, f64),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[Interval; 3]>, IntersectError> {
        let index = self.index(order)?;
        Ok(self
            .partials(span, meter)?
            .and_then(|p| p.get(index).copied()))
    }

    fn at(
        &self,
        t: f64,
        side: Limit,
        meter: &mut WorkMeter<'_>,
    ) -> Result<([Interval; 3], [Interval; 3]), IntersectError> {
        let (u, v, sides) = if self.fixed_u {
            (self.at, t, (Limit::FromRight, side))
        } else {
            (t, self.at, (side, Limit::FromRight))
        };
        let [point, along_u, along_v] = self.surface.enclose(u, v, sides, meter)?;
        Ok((point, if self.fixed_u { along_v } else { along_u }))
    }
}

/// What a search of one path found.
struct Found<const D: usize> {
    /// The nearest points so far, each with its parameter span.
    points: Vec<Nearest<1, D>>,
    /// Spans left undecided at the floor, each with its lower bound on the distance.
    open: Vec<((f64, f64), f64)>,
    /// The smallest upper bound on the distance.
    upper: f64,
}

/// `C′(t)·(C(t) − p)` at `t`, half the derivative of the squared distance.
fn slope<const D: usize>(
    path: &dyn Path<D>,
    p: &[f64; D],
    t: f64,
    side: Limit,
    meter: &mut WorkMeter<'_>,
) -> Result<Interval, IntersectError> {
    let (point, first) = path.at(t, side, meter)?;
    Ok(dot(&first, &minus(&point, p)?)?)
}

/// The point at `span`'s enclosure.
fn nearest<const D: usize>(
    path: &dyn Path<D>,
    p: &[f64; D],
    span: (f64, f64),
    place: Place,
    meter: &mut WorkMeter<'_>,
) -> Result<Nearest<1, D>, IntersectError> {
    let point = path.around(span, meter)?;
    let distance = Interval::norm(&minus(&point, p)?)?;
    Ok(Nearest {
        at: [Interval::new(span.0, span.1)?],
        point,
        distance,
        place,
    })
}

/// Halves `span`, on which the slope rises from negative to positive, until its image is below
/// `floor` or the slope's sign can no longer be told on either side.
fn refine<const D: usize>(
    path: &dyn Path<D>,
    p: &[f64; D],
    mut span: (f64, f64),
    floor: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<(f64, f64), IntersectError> {
    while image(&path.around(span, meter)?) > floor {
        meter.charge(STEP)?;
        let middle = span.0 / 2.0 + span.1 / 2.0;
        if middle <= span.0 || middle >= span.1 {
            break;
        }
        let f = slope(path, p, middle, Limit::FromRight, meter)?;
        if negative(f) {
            span.0 = middle;
        } else if positive(f) {
            span.1 = middle;
        } else {
            // The foot is at the middle, to rounding: close in from both sides.
            let low = span.0 / 2.0 + middle / 2.0;
            let high = middle / 2.0 + span.1 / 2.0;
            let below = negative(slope(path, p, low, Limit::FromRight, meter)?);
            let above = positive(slope(path, p, high, Limit::FromRight, meter)?);
            if below {
                span.0 = low;
            }
            if above {
                span.1 = high;
            }
            if !(below || above) {
                break;
            }
        }
    }
    Ok(span)
}

/// The search of one path: 12.4's steps 2 to 6 in one parameter.
fn search<const D: usize>(
    path: &dyn Path<D>,
    p: &[f64; D],
    floor: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Found<D>, IntersectError> {
    let (a, b) = path.span();
    if !(a.is_finite() && b.is_finite()) {
        return Err(IntersectError::InvalidInput);
    }
    let mut found = Found {
        points: Vec::new(),
        open: Vec::new(),
        upper: f64::INFINITY,
    };
    for (t, side) in [(a, Limit::FromRight), (b, Limit::FromLeft)] {
        let (point, _) = path.at(t, side, meter)?;
        let distance = Interval::norm(&minus(&point, p)?)?;
        found.upper = smaller(found.upper, distance.hi());
        found.points.push(Nearest {
            at: [Interval::point(t)?],
            point,
            distance,
            place: Place::Boundary,
        });
    }
    let mut work = alloc::vec![(a, b)];
    while let Some(span) = work.pop() {
        meter.charge(STEP)?;
        let around = path.around(span, meter)?;
        let gap = minus(&around, p)?;
        if Interval::norm(&gap)?.lo() > found.upper {
            continue;
        }
        let mut convex = false;
        let first = path.derivative(span, 1, meter)?;
        if let Some(first) = &first {
            let f = dot(first, &gap)?;
            if positive(f) || negative(f) {
                continue;
            }
            if let Some(second) = path.derivative(span, 2, meter)? {
                let curve = square(first)?.add(dot(&second, &gap)?)?;
                // Concave: a stationary point inside would be a maximum.
                if negative(curve) {
                    continue;
                }
                if positive(curve) {
                    convex = true;
                    let left = slope(path, p, span.0, Limit::FromRight, meter)?;
                    let right = slope(path, p, span.1, Limit::FromLeft, meter)?;
                    if positive(left) || negative(right) {
                        continue;
                    }
                    if negative(left) && positive(right) {
                        let root = refine(path, p, span, floor, meter)?;
                        let point = nearest(path, p, root, Place::Foot, meter)?;
                        found.upper = smaller(found.upper, point.distance.hi());
                        found.points.push(point);
                        continue;
                    }
                }
            }
        }
        let middle = span.0 / 2.0 + span.1 / 2.0;
        if image(&around) <= floor || middle <= span.0 || middle >= span.1 {
            // Convex here means at most one stationary point, so the span's nearest is one point.
            if convex {
                let point = nearest(path, p, span, Place::Foot, meter)?;
                found.upper = smaller(found.upper, point.distance.hi());
                found.points.push(point);
            } else {
                found.open.push((span, Interval::norm(&gap)?.lo()));
            }
            continue;
        }
        let (point, _) = path.at(middle, Limit::FromRight, meter)?;
        found.upper = smaller(found.upper, Interval::norm(&minus(&point, p)?)?.hi());
        work.push((middle, span.1));
        work.push((span.0, middle));
    }
    Ok(found)
}

/// The smallest interval holding `x` and `y`.
fn hull(x: Interval, y: Interval) -> Result<Interval, NumError> {
    Interval::new(smaller(x.lo(), y.lo()), larger(x.hi(), y.hi()))
}

/// Two touching enclosures of one nearest point as one: the exact point is in one of them, so the
/// answer holds both, and is a foot if either is.
fn joined<const N: usize, const D: usize>(
    a: &Nearest<N, D>,
    b: &Nearest<N, D>,
) -> Result<Nearest<N, D>, NumError> {
    let mut out = *a;
    for (x, y) in out.at.iter_mut().zip(&b.at) {
        *x = hull(*x, *y)?;
    }
    for (x, y) in out.point.iter_mut().zip(&b.point) {
        *x = hull(*x, *y)?;
    }
    out.distance = hull(a.distance, b.distance)?;
    if b.place == Place::Foot {
        out.place = Place::Foot;
    }
    Ok(out)
}

/// Keeps what can still be nearest, in parameter order, joining enclosures that touch: one foot
/// reached from both sides of a split, or from several boxes that touch only through each other,
/// or a foot at an end.
fn settle<const N: usize, const D: usize>(
    mut points: Vec<Nearest<N, D>>,
    upper: f64,
) -> Result<Vec<Nearest<N, D>>, NumError> {
    points.retain(|n| n.distance.lo() <= upper);
    let mut kept: Vec<Nearest<N, D>> = Vec::with_capacity(points.len());
    for mut point in points {
        // Each join can reach another group, so this runs until nothing kept touches the point.
        while let Some(k) = kept.iter().position(|n| {
            n.at.iter()
                .zip(&point.at)
                .all(|(x, y)| x.hi() >= y.lo() && y.hi() >= x.lo())
        }) {
            point = joined(&kept.swap_remove(k), &point)?;
        }
        kept.push(point);
    }
    kept.sort_by(|a, b| {
        let (a, b) = (a.at.map(Interval::lo), b.at.map(Interval::lo));
        a.partial_cmp(&b).unwrap_or(core::cmp::Ordering::Equal)
    });
    Ok(kept)
}

/// Joins the open spans still in reach of `found.upper` into runs. A run within twice the floor
/// becomes a nearest point; the first longer one is returned, as a parameter span.
fn close<const D: usize>(
    path: &dyn Path<D>,
    p: &[f64; D],
    found: &mut Found<D>,
    floor: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<(f64, f64)>, IntersectError> {
    let upper = found.upper;
    found.open.retain(|(_, low)| *low <= upper);
    found
        .open
        .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(core::cmp::Ordering::Equal));
    let mut runs: Vec<(f64, f64)> = Vec::new();
    for (span, _) in found.open.drain(..) {
        match runs.last_mut() {
            Some(run) if run.1 >= span.0 => run.1 = larger(run.1, span.1),
            _ => runs.push(span),
        }
    }
    for run in runs {
        if image(&path.around(run, meter)?) > 2.0 * floor {
            return Ok(Some(run));
        }
        let point = nearest(path, p, run, Place::Foot, meter)?;
        found.upper = smaller(found.upper, point.distance.hi());
        found.points.push(point);
    }
    Ok(None)
}

/// Exact coordinates as intervals.
fn exact<const D: usize>(x: &[f64; D]) -> Result<[Interval; D], NumError> {
    let mut out = [Interval::point(0.0)?; D];
    for (o, c) in out.iter_mut().zip(x) {
        *o = Interval::point(*c)?;
    }
    Ok(out)
}

/// A line's foot, which is rational: (p − o)·d / d·d, for the search's ends to clamp.
fn line_foot<const D: usize>(
    origin: &[f64; D],
    direction: &[f64; D],
    p: &[f64; D],
) -> Result<Interval, IntersectError> {
    let d = exact(direction)?;
    let gap = minus(&exact(origin)?, p)?;
    Ok(dot(&gap, &d)?.neg().div(square(&d)?)?)
}

/// The nearest points of `curve` to `p`, to a floor of `floor` in space.
///
/// # Errors
///
/// [`IntersectError::InvalidInput`] for a domain that is not finite on a kind other than a line, or
/// a floor that is not positive; [`IntersectError::BudgetExceeded`] and
/// [`IntersectError::Cancelled`]; [`IntersectError::UnprovenMaths`] where a kind's bound needs one.
pub fn project_curve<const D: usize>(
    curve: &CurveKind<D>,
    p: &[f64; D],
    floor: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Projection<1, D>, IntersectError> {
    if floor <= 0.0 || floor.is_nan() {
        return Err(IntersectError::InvalidInput);
    }
    if let CurveKind::Line(line) = curve {
        let t = line_foot(&line.origin(), &line.direction(), p)?;
        let (a, b) = curve.domain();
        if t.lo() >= a && t.hi() <= b {
            let (low, _) = Path::at(curve, t.lo(), Limit::FromRight, meter)?;
            let (high, _) = Path::at(curve, t.hi(), Limit::FromLeft, meter)?;
            let mut point = low;
            for (o, other) in point.iter_mut().zip(high) {
                *o = Interval::new(smaller(o.lo(), other.lo()), larger(o.hi(), other.hi()))?;
            }
            let distance = Interval::norm(&minus(&point, p)?)?;
            return Ok(Projection::Points(alloc::vec![Nearest {
                at: [t],
                point,
                distance,
                place: Place::Foot,
            }]));
        }
    }
    if let CurveKind::Circle(circle) = curve {
        let (_, unit) = circle.frame().unit_vectors()?;
        let gap = padded(&minus(&exact(&circle.center())?, p)?)?;
        let [x, y, n] = unit;
        if dot(&gap, &x)?.contains(0.0) && dot(&gap, &y)?.contains(0.0) {
            let height = dot(&gap, &n)?;
            let radius = Interval::point(circle.radius())?;
            return Ok(Projection::Set {
                set: Continuum::Curve,
                distance: radius.square()?.add(height.square()?)?.sqrt()?,
            });
        }
    }
    settled(curve, p, floor, meter)
}

/// One path's search, and its answer.
fn settled<const D: usize>(
    path: &dyn Path<D>,
    p: &[f64; D],
    floor: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Projection<1, D>, IntersectError> {
    let mut found = search(path, p, floor, meter)?;
    if let Some(run) = close(path, p, &mut found, floor, meter)? {
        return Ok(Projection::NonIsolated {
            enclosure: [Interval::new(run.0, run.1)?],
        });
    }
    Ok(Projection::Points(settle(found.points, found.upper)?))
}

/// A point of 2D or 3D as 3D, its missing coordinates zero.
fn padded<const D: usize>(x: &[Interval; D]) -> Result<[Interval; 3], NumError> {
    let mut out = [Interval::point(0.0)?; 3];
    for (o, c) in out.iter_mut().zip(x) {
        *o = *c;
    }
    Ok(out)
}

/// What the interior search left: candidates and undecided boxes.
struct Inside {
    points: Vec<Nearest<2, 3>>,
    open: Vec<([Interval; 2], f64)>,
}

/// The sign of the Hessian of half the squared distance over a box.
#[derive(PartialEq)]
enum Shape {
    /// Positive definite: at most one stationary point, a minimum.
    Bowl,
    /// Negative definite or a saddle: no minimum inside.
    NoMinimum,
    /// Not known.
    Unknown,
}

/// The shape over a box, from its partials and the gap to the point.
fn shape(partials: &[[Interval; 3]], gap: &[Interval; 3]) -> Result<Shape, IntersectError> {
    let [_, su, sv, suu, suv, svv, ..] = partials else {
        return Err(IntersectError::Internal);
    };
    let a = square(su)?.add(dot(suu, gap)?)?;
    let b = dot(su, sv)?.add(dot(suv, gap)?)?;
    let c = square(sv)?.add(dot(svv, gap)?)?;
    let det = a.mul(c)?.sub(b.square()?)?;
    Ok(if positive(a) && positive(det) {
        Shape::Bowl
    } else if negative(a) || negative(c) || negative(det) {
        Shape::NoMinimum
    } else {
        Shape::Unknown
    })
}

/// The search of the inside of a surface: 12.4's steps 2 to 6 in two parameters.
fn search_inside(
    surface: &SurfaceKind,
    p: &[f64; 3],
    floor: f64,
    upper: &mut f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Inside, IntersectError> {
    let mut inside = Inside {
        points: Vec::new(),
        open: Vec::new(),
    };
    let sides = (Limit::FromRight, Limit::FromRight);
    let mut work = alloc::vec![surface.domain()];
    while let Some(region) = work.pop() {
        meter.charge(STEP)?;
        let ((a, b), (c, d)) = region;
        let around = boxed(&surface.bound(region, meter)?)?;
        let gap = minus(&around, p)?;
        if Interval::norm(&gap)?.lo() > *upper {
            continue;
        }
        let partials = match surface.partials_over(region, sides, 2, meter) {
            Ok(partials) => Some(partials),
            Err(CurveError::Unbounded) => None,
            Err(e) => return Err(e.into()),
        };
        let mut bowl = false;
        if let Some(partials) = &partials {
            let [_, su, sv, ..] = partials.as_slice() else {
                return Err(IntersectError::Internal);
            };
            let (fu, fv) = (dot(su, &gap)?, dot(sv, &gap)?);
            if positive(fu) || negative(fu) || positive(fv) || negative(fv) {
                continue;
            }
            match shape(partials, &gap)? {
                Shape::NoMinimum => continue,
                Shape::Bowl => bowl = true,
                Shape::Unknown => {}
            }
        }
        let (u, v) = (a / 2.0 + b / 2.0, c / 2.0 + d / 2.0);
        let stuck = (u <= a || u >= b) && (v <= c || v >= d);
        if image(&around) <= floor || stuck {
            let at = [Interval::new(a, b)?, Interval::new(c, d)?];
            if bowl {
                let distance = Interval::norm(&gap)?;
                *upper = smaller(*upper, distance.hi());
                inside.points.push(Nearest {
                    at,
                    point: around,
                    distance,
                    place: Place::Foot,
                });
            } else {
                inside.open.push((at, Interval::norm(&gap)?.lo()));
            }
            continue;
        }
        let [point, _, _] = surface.enclose(u, v, sides, meter)?;
        *upper = smaller(*upper, Interval::norm(&minus(&point, p)?)?.hi());
        // The wider side in parameter is split, so a box of a long thin kind stays square in use.
        if (b - a >= d - c && u > a && u < b) || v <= c || v >= d {
            work.push(((u, b), (c, d)));
            work.push(((a, u), (c, d)));
        } else {
            work.push(((a, b), (v, d)));
            work.push(((a, b), (c, v)));
        }
    }
    Ok(inside)
}

/// The axis of a round kind: its centre, frame vectors and whether a parallel of it is a continuum
/// from the axis.
fn on_axis(
    center: [f64; 3],
    frame: &geom_curve::Frame<3>,
    p: &[f64; 3],
) -> Result<Option<Interval>, IntersectError> {
    let (_, [x, y, n]) = frame.unit_vectors()?;
    let gap = minus(&exact(&center)?, p)?;
    if dot(&gap, &x)?.contains(0.0) && dot(&gap, &y)?.contains(0.0) {
        return Ok(Some(dot(&gap, &n)?));
    }
    Ok(None)
}

/// The nearest points of `surface` to `p`, to a floor of `floor` in space.
///
/// # Errors
///
/// As [`project_curve`].
pub fn project_surface(
    surface: &SurfaceKind,
    p: &[f64; 3],
    floor: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Projection<2, 3>, IntersectError> {
    if floor <= 0.0 || floor.is_nan() {
        return Err(IntersectError::InvalidInput);
    }
    // A sphere from its centre, or a round kind from its axis, along one meridian: every meridian
    // gives the same.
    let round = match surface {
        SurfaceKind::Sphere(s) => Some((s.center(), s.frame(), true)),
        SurfaceKind::Cylinder(s) => Some((s.center(), s.frame(), false)),
        SurfaceKind::Cone(s) => Some((s.center(), s.frame(), false)),
        SurfaceKind::Torus(s) => Some((s.center(), s.frame(), false)),
        _ => None,
    };
    if let Some((center, frame, ball)) = round
        && let Some(height) = on_axis(center, &frame, p)?
    {
        let (u, _) = surface.domain();
        let meridian = Line {
            surface,
            at: u.0,
            fixed_u: true,
        };
        if ball && height.contains(0.0) {
            let (point, _) = meridian.at(0.0, Limit::FromRight, meter)?;
            return Ok(Projection::Set {
                set: Continuum::Surface,
                distance: Interval::norm(&minus(&point, p)?)?,
            });
        }
        let found = settled(&meridian, p, floor, meter)?;
        if let Some(best) = found.first() {
            let [v] = best.at;
            return Ok(Projection::Set {
                set: Continuum::Parallel { v },
                distance: best.distance,
            });
        }
    }
    let ((a, b), (c, d)) = surface.domain();
    let mut points: Vec<Nearest<2, 3>> = Vec::new();
    let mut open: Vec<([Interval; 2], f64)> = Vec::new();
    let mut upper = f64::INFINITY;
    for (at, fixed_u) in [(c, false), (d, false), (a, true), (b, true)] {
        let edge = Line {
            surface,
            at,
            fixed_u,
        };
        let found = search(&edge, p, floor, meter)?;
        upper = smaller(upper, found.upper);
        let fixed = Interval::point(at)?;
        for point in found.points {
            let [t] = point.at;
            points.push(Nearest {
                at: if fixed_u { [fixed, t] } else { [t, fixed] },
                point: point.point,
                distance: point.distance,
                place: Place::Boundary,
            });
        }
        for (span, low) in found.open {
            let t = Interval::new(span.0, span.1)?;
            open.push((if fixed_u { [fixed, t] } else { [t, fixed] }, low));
        }
    }
    let inside = search_inside(surface, p, floor, &mut upper, meter)?;
    points.extend(inside.points);
    open.extend(inside.open);
    open.retain(|(_, low)| *low <= upper);
    let side = |k: usize| open.iter().filter_map(move |(b, _)| b.get(k).copied());
    let low = |k: usize| side(k).map(Interval::lo).reduce(smaller);
    let high = |k: usize| side(k).map(Interval::hi).reduce(larger);
    if let (Some(a), Some(b), Some(c), Some(d)) = (low(0), high(0), low(1), high(1)) {
        let at = [Interval::new(a, b)?, Interval::new(c, d)?];
        let point = boxed(&surface.bound(((a, b), (c, d)), meter)?)?;
        if image(&point) > 2.0 * floor {
            return Ok(Projection::NonIsolated { enclosure: at });
        }
        let distance = Interval::norm(&minus(&point, p)?)?;
        upper = smaller(upper, distance.hi());
        points.push(Nearest {
            at,
            point,
            distance,
            place: Place::Foot,
        });
    }
    Ok(Projection::Points(settle(points, upper)?))
}

/// The parameters of `p` on `curve`, if it lies within `bound` of it (12.4's inversion).
///
/// # Errors
///
/// As [`project_curve`].
pub fn invert_curve<const D: usize>(
    curve: &CurveKind<D>,
    p: &[f64; D],
    bound: f64,
    floor: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Nearest<1, D>>, IntersectError> {
    let found = project_curve(curve, p, floor, meter)?;
    Ok(found.first().filter(|n| n.distance.lo() <= bound).copied())
}

/// The parameters of `p` on `surface`, if it lies within `bound` of it (12.4's inversion).
///
/// # Errors
///
/// As [`project_curve`].
pub fn invert_surface(
    surface: &SurfaceKind,
    p: &[f64; 3],
    bound: f64,
    floor: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Nearest<2, 3>>, IntersectError> {
    let found = project_surface(surface, p, floor, meter)?;
    Ok(found.first().filter(|n| n.distance.lo() <= bound).copied())
}
