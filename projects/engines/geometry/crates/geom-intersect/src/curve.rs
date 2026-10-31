//! Where two curves meet, and where a curve meets itself (chapter 12.5).
//!
//! What it does: [`curves`] answers for any two kinds. Two lines, two circles, and a line with any
//! conic have closed forms, in the plane; two lines also in space. Every other pair runs
//! [`crate::solve`] on A(s) − B(t) in two coordinates, with Bézier clipping narrowing the boxes of
//! NURBS pieces. In 3D the coordinate left out decides: a meeting is reported only where it is proven,
//! and a pair whose gap cannot be told from zero is flagged [`Unresolved::NearMiss`]. Two conics that
//! are not both circles run the engine too: their exact counts are the fixed-integer predicates of
//! chapter 14's piece G190. [`self_crossings`] runs the same engine on a curve and
//! itself, above the diagonal, dropping every span whose tangents stay in one half-plane, since such a
//! span cannot cross itself.
//!
//! Guarantees:
//!
//! - **Every crossing found or flagged.** The engine discards a box only on a proof, so a meeting that
//!   no item reports does not exist; one that could not be isolated is a flagged region.
//! - **Tangency is never a guess.** Two circles, and a line with a circle on whole-number data, decide
//!   it by an exact sign. Elsewhere a tangency is what the operator cannot separate, and 12.9's second
//!   order test reads it: curvatures that differ give [`Unresolved::TangentBand`], with the band's
//!   width, and curvatures that cannot be told apart [`Unresolved::HigherOrderContact`]. Whether a
//!   contact exists there is an equality no interval can prove.
//! - **Overlap by identity.** The same carrier, two pieces of one line, or two arcs of one circle are
//!   an [`Item::Overlap`] with [`Coincidence::Identical`]. A near overlap stays flagged.
//! - **Circles of very different radii.** The count of crossings comes from exact signs of the squared
//!   distances, never from a relative threshold, so a small circle on a large one has its two points.
//!
//! Method: chapter 12.5 (`projects/engines/geometry/architecture/12-intersections.md`): Bézier clipping
//! ([Sederberg and Nishita 1990](https://doi.org/10.1016/0010-4485(90)90039-F)), bisecting when a clip
//! removes too little, finished by Krawczyk's test; exact signs of products by error-free
//! transformations ([Shewchuk 1997](https://doi.org/10.1007/PL00009321)).
//!
//! Cost: a closed form is a few predicates. The engine's boxes are near the logarithm of the curves'
//! reach for a transversal crossing, and each is a few evaluations; a clip is O(p²). Recursion: none.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': s and t the parameters, o, d, p and e two lines, and a, b, c \
              the coefficients of a quadratic"
)]

use alloc::vec::Vec;

use geom_curve::{
    Axes, Circle, CurveError, CurveKind, DerivativeBound, Frame, Line, ParamCurve, tight,
};
use geom_math::Box3;
use geom_num::eft::two_prod;
use geom_num::exact_sum::ExactSum;
use geom_num::grid::{COORD_MAX, GridStep, P3, Steps};
use geom_num::interval::Interval;
use geom_num::isolate::RESOLUTION_FLOOR;
use geom_num::quadric;
use geom_num::{Exactness, Sign, math, predicate};
use geom_nurbs::{BernsteinCurve, Bounds, Limit, NurbsError, Slab};
use geom_run::WorkMeter;

use crate::closed::{self, Meeting};
use crate::error::IntersectError;
use crate::result::{
    Coincidence, Coverage, Diag, Intersection, Item, OverlapRegion, ParamBox, PointContact, Test,
    Unresolved,
};
use crate::solve::{Field, Reason, Solved, grown, runs, solve};

/// The diagnostic of a pair a closed form could not decide, sent on to the engine.
pub const ROUTE: &str = "closed-form-to-engine";

/// The grid's reach in each coordinate, in steps; a double holds it exactly.
#[allow(
    clippy::cast_precision_loss,
    reason = "COORD_MAX is below 2^53, so the cast is exact"
)]
const REACH: f64 = COORD_MAX as f64;

/// The sign of Σ aᵢ·bᵢ, exactly: each product is a double and its error, and their sum is exact.
fn sign_of(terms: &[(f64, f64)]) -> Sign {
    let mut sum = ExactSum::new();
    for &(a, b) in terms {
        let (p, e) = two_prod(a, b);
        sum.add(p);
        sum.add(e);
    }
    let v = sum.round_ieee();
    if v > 0.0 {
        Sign::Positive
    } else if v < 0.0 {
        Sign::Negative
    } else {
        Sign::Zero
    }
}

/// The answer for items that all stand proven.
fn proven(items: Vec<Item>, meter: &mut WorkMeter<'_>) -> Result<Intersection, IntersectError> {
    Intersection::new(items, Coverage::TopologyCertified, Vec::new(), meter)
}

/// A box of grid points around `lo..hi`, whole steps outward.
fn grid_box<const D: usize>(b: &Bounds<D>) -> Result<Box3, IntersectError> {
    let side = |x: &[f64; D], i: usize, up: bool| -> Result<i64, IntersectError> {
        let v = x.get(i).copied().unwrap_or(0.0);
        let r = if up { math::ceil(v)? } else { math::floor(v)? };
        let whole = r.get();
        if whole.abs() > REACH {
            return Err(IntersectError::FrameRange);
        }
        // Inside the grid, so a whole number of at most 2^40.
        Ok(exact_i64(whole))
    };
    let corner = |x: &[f64; D], up: bool| -> Result<P3, IntersectError> {
        Ok(P3::new(side(x, 0, up)?, side(x, 1, up)?, side(x, 2, up)?)?)
    };
    Ok(Box3::new(corner(&b.lo, false)?, corner(&b.hi, true)?)?)
}

/// `v`, a whole number of at most 2^53 in size, as an integer.
#[allow(
    clippy::cast_possible_truncation,
    reason = "the callers hold v to a whole number inside the grid"
)]
const fn exact_i64(v: f64) -> i64 {
    v as i64
}

/// A certified class of one step, for a box the engine refined to the floor.
const fn certified() -> Exactness {
    Exactness::Certified {
        bound: Steps(1),
        step: GridStep::Q0,
    }
}

/// The widest side of `b`.
fn widest<const D: usize>(b: &Bounds<D>) -> f64 {
    let mut out = 0.0;
    for (lo, hi) in b.lo.iter().zip(&b.hi) {
        let w = hi - lo;
        if w > out {
            out = w;
        }
    }
    out
}

/// `x`'s ends.
fn span(x: Interval) -> (f64, f64) {
    (x.lo(), x.hi())
}

/// A box of a kind's `D` coordinates, as two chosen ones.
fn picked<const D: usize>(
    b: &Bounds<D>,
    [i, j]: [usize; 2],
) -> Result<[Interval; 2], IntersectError> {
    let side = |k: usize| -> Result<Interval, IntersectError> {
        match (b.lo.get(k), b.hi.get(k)) {
            (Some(lo), Some(hi)) => Ok(Interval::new(*lo, *hi)?),
            _ => Err(IntersectError::Internal),
        }
    };
    Ok([side(i)?, side(j)?])
}

/// A coordinate of a list of enclosures.
fn coordinate<const D: usize>(
    list: &[[Interval; D]],
    k: usize,
    i: usize,
) -> Result<Interval, IntersectError> {
    list.get(k)
        .and_then(|p| p.get(i))
        .copied()
        .ok_or(IntersectError::Internal)
}

/// A's Bézier pieces, where it has them.
fn pieces<const D: usize>(
    curve: &CurveKind<D>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<BernsteinCurve<D>>, IntersectError> {
    match curve {
        // Clipping only speeds the engine, so a form it cannot take goes without.
        CurveKind::Nurbs(c) => match c.bernstein(meter) {
            Ok(list) => Ok(list),
            Err(NurbsError::NotClamped) => Ok(Vec::new()),
            Err(e) => Err(CurveError::from(e).into()),
        },
        _ => Ok(Vec::new()),
    }
}

/// A parameter's domain, and whether the kind closes on itself over it. A closed kind's join is no
/// boundary: the search runs a little past it, so that a meeting on the join stands inside a box,
/// and evaluates the far side on the near one.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Seam {
    domain: (f64, f64),
    closed: bool,
}

impl Seam {
    /// A domain that ends where it ends.
    pub(crate) const fn open(domain: (f64, f64)) -> Seam {
        Seam {
            domain,
            closed: false,
        }
    }

    /// A domain in half-turns, closed if it is a whole turn.
    pub(crate) fn turns(domain: (f64, f64)) -> Seam {
        Seam {
            domain,
            closed: domain.1 - domain.0 >= 2.0,
        }
    }

    fn period(self) -> f64 {
        self.domain.1 - self.domain.0
    }

    /// The domain.
    pub(crate) fn whole(self) -> Result<Interval, IntersectError> {
        Ok(Interval::new(self.domain.0, self.domain.1)?)
    }

    /// The side to search.
    pub(crate) fn side(self) -> Result<Interval, IntersectError> {
        let (lo, hi) = self.domain;
        let pad = if self.closed {
            self.period() / 1024.0
        } else {
            0.0
        };
        Ok(Interval::new(lo - pad, hi + pad)?)
    }

    /// `t` on the domain.
    pub(crate) fn wrapped(self, t: f64) -> f64 {
        let (lo, hi) = self.domain;
        if self.closed && t < lo {
            t + self.period()
        } else if self.closed && t > hi {
            t - self.period()
        } else {
            t
        }
    }

    /// The stretches of the domain that `x` covers: one, or one each side of the join.
    pub(crate) fn spans(self, x: Interval) -> [Option<(f64, f64)>; 2] {
        if !self.closed {
            return [Some(span(x)), None];
        }
        let (lo, hi) = self.domain;
        let p = self.period();
        let cut = |shift: f64| {
            let (a, b) = (
                tight::larger(x.lo() + shift, lo),
                tight::smaller(x.hi() + shift, hi),
            );
            (a < b).then_some((a, b))
        };
        let mut parts = [cut(0.0), cut(p), cut(-p)].into_iter().flatten();
        // A box of one point on the join has no length to cut.
        let point = self.wrapped(x.lo());
        [parts.next().or(Some((point, point))), parts.next()]
    }

    /// `x` as the domain writes it, a box across the join held to its near side.
    pub(crate) fn onto(self, x: Interval) -> Result<Interval, IntersectError> {
        let (lo, hi) = self.domain;
        let shift = Interval::point(self.period())?;
        let moved = if self.closed && x.hi() > hi {
            x.sub(shift)?
        } else if self.closed && x.hi() < lo {
            x.add(shift)?
        } else {
            x
        };
        held(moved, (lo, hi))
    }
}

/// `curve`'s seam over `domain`.
fn seam_of<const D: usize>(curve: &CurveKind<D>, domain: (f64, f64)) -> Seam {
    match curve {
        CurveKind::Circle(_) | CurveKind::Ellipse(_) => Seam::turns(domain),
        _ => Seam::open(domain),
    }
}

/// A box around `curve` over `x`, across its join if `x` is.
fn bound_over<const D: usize>(
    curve: &CurveKind<D>,
    seam: Seam,
    x: Interval,
    meter: &mut WorkMeter<'_>,
) -> Result<Bounds<D>, IntersectError> {
    let [first, second] = seam.spans(x);
    let first = first.ok_or(IntersectError::Internal)?;
    let mut out = curve.bound(first, meter)?;
    if let Some(part) = second {
        out = out
            .union(&curve.bound(part, meter)?)
            .map_err(CurveError::from)?;
    }
    Ok(out)
}

/// A box around `curve`'s first derivative over `x`, across its join if `x` is; `None` where it has
/// none.
fn slope_over<const D: usize>(
    curve: &CurveKind<D>,
    seam: Seam,
    x: Interval,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Bounds<D>>, IntersectError> {
    let mut out: Option<Bounds<D>> = None;
    for part in seam.spans(x).into_iter().flatten() {
        let DerivativeBound::Bounded(b) = curve.derivative_bound(part, 1, meter)? else {
            return Ok(None);
        };
        out = Some(match out {
            Some(a) => a.union(&b).map_err(CurveError::from)?,
            None => b,
        });
    }
    Ok(out)
}

/// A(s) − B(t) in two coordinates.
struct Gap<'a, const D: usize> {
    a: &'a CurveKind<D>,
    b: &'a CurveKind<D>,
    seams: [Seam; 2],
    axes: [usize; 2],
    coplanar: bool,
    pieces_a: Vec<BernsteinCurve<D>>,
    pieces_b: Vec<BernsteinCurve<D>>,
}

/// The piece that holds `x` whole, cut to it.
fn cut<const D: usize>(pieces: &[BernsteinCurve<D>], x: Interval) -> Option<BernsteinCurve<D>> {
    let piece = pieces.iter().find(|p| {
        let (lo, hi) = p.domain();
        lo <= x.lo() && x.hi() <= hi
    })?;
    piece.restrict(span(x)).ok()
}

/// The thin strip around `piece` in the plane of `axes`, from the chord between its ends.
fn strip<const D: usize>(
    piece: &BernsteinCurve<D>,
    [i, j]: [usize; 2],
    meter: &mut WorkMeter<'_>,
) -> Option<Slab<D>> {
    let (lo, hi) = piece.domain();
    let end = |t: f64| -> Option<[f64; D]> {
        let (n, w) = piece.value_at(t).ok()?;
        let w = w.lo() * 0.5 + w.hi() * 0.5;
        if w == 0.0 {
            return None;
        }
        Some(n.map(|c| (c.lo() * 0.5 + c.hi() * 0.5) / w))
    };
    let (first, last) = (end(lo)?, end(hi)?);
    let (dx, dy) = (last.get(i)? - first.get(i)?, last.get(j)? - first.get(j)?);
    if dx == 0.0 && dy == 0.0 {
        return None;
    }
    let mut normal = [0.0; D];
    *normal.get_mut(i)? = -dy;
    *normal.get_mut(j)? = dx;
    piece.slab(normal, first, meter).ok()
}

impl<const D: usize> Field<2> for Gap<'_, D> {
    fn at(
        &self,
        [s, t]: [f64; 2],
        meter: &mut WorkMeter<'_>,
    ) -> Result<[Interval; 2], IntersectError> {
        let [first, second] = self.seams;
        let x = self
            .a
            .enclose(first.wrapped(s), Limit::FromRight, 0, meter)?;
        let y = self
            .b
            .enclose(second.wrapped(t), Limit::FromRight, 0, meter)?;
        let [i, j] = self.axes;
        Ok([
            coordinate(&x, 0, i)?.sub(coordinate(&y, 0, i)?)?,
            coordinate(&x, 0, j)?.sub(coordinate(&y, 0, j)?)?,
        ])
    }

    fn over(
        &self,
        [s, t]: &[Interval; 2],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[Interval; 2]>, IntersectError> {
        let [first, second] = self.seams;
        let a = picked(&bound_over(self.a, first, *s, meter)?, self.axes)?;
        let b = picked(&bound_over(self.b, second, *t, meter)?, self.axes)?;
        let [a0, a1] = a;
        let [b0, b1] = b;
        Ok(Some([a0.sub(b0)?, a1.sub(b1)?]))
    }

    fn slope(
        &self,
        [s, t]: &[Interval; 2],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[[Interval; 2]; 2]>, IntersectError> {
        let [first, second] = self.seams;
        let (Some(da), Some(db)) = (
            slope_over(self.a, first, *s, meter)?,
            slope_over(self.b, second, *t, meter)?,
        ) else {
            return Ok(None);
        };
        let [ax, ay] = picked(&da, self.axes)?;
        let [bx, by] = picked(&db, self.axes)?;
        Ok(Some([[ax, bx.neg()], [ay, by.neg()]]))
    }

    fn reach(
        &self,
        [s, t]: &[Interval; 2],
        meter: &mut WorkMeter<'_>,
    ) -> Result<f64, IntersectError> {
        let [first, second] = self.seams;
        let a = widest(&bound_over(self.a, first, *s, meter)?);
        let b = widest(&bound_over(self.b, second, *t, meter)?);
        Ok(if a > b { a } else { b })
    }

    fn narrow(
        &self,
        [s, t]: &[Interval; 2],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[Interval; 2]>, IntersectError> {
        let (Some(pa), Some(pb)) = (cut(&self.pieces_a, *s), cut(&self.pieces_b, *t)) else {
            return Ok(Some([*s, *t]));
        };
        let (Some(fat_a), Some(fat_b)) =
            (strip(&pa, self.axes, meter), strip(&pb, self.axes, meter))
        else {
            return Ok(Some([*s, *t]));
        };
        let (Ok(on_a), Ok(on_b)) = (pa.clip(&fat_b, meter), pb.clip(&fat_a, meter)) else {
            return Ok(Some([*s, *t]));
        };
        match (on_a, on_b) {
            (Some(x), Some(y)) => Ok(Some([Interval::new(x.0, x.1)?, Interval::new(y.0, y.1)?])),
            _ => Ok(None),
        }
    }
}

/// The items for what the engine found in `solved`, for a pair in `axes`. In 3D a proven meeting of
/// two coordinates is a meeting only if the third agrees.
fn items_of<const D: usize>(
    gap: &Gap<'_, D>,
    solved: Solved<2>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Item>, IntersectError> {
    let [first, second] = gap.seams;
    // Written on the domains, where two runs either side of a join are one.
    let mut flagged = solved.flagged;
    for f in &mut flagged {
        let [s, t] = f.region;
        f.region = [first.onto(s)?, second.onto(t)?];
    }
    let runs = runs(flagged)?;
    let mut items = Vec::new();
    let mut seen: Vec<[Interval; 2]> = Vec::new();
    let others: Vec<usize> = (0..D)
        .filter(|k| !gap.axes.contains(k) && !gap.coplanar)
        .collect();
    for z in solved.zeros {
        let [s, t] = z.enclosure;
        let enclosure = [first.onto(s)?, second.onto(t)?];
        let [s, t] = enclosure;
        // A zero inside a run is part of what the run could not tell apart, and one on a join is
        // found from both sides of it.
        let meeting = |hull: &[Interval; 2]| {
            hull.iter()
                .zip(&enclosure)
                .all(|(a, b)| a.lo() <= b.hi() && b.lo() <= a.hi())
        };
        if runs.iter().any(|(hull, _)| meeting(hull)) || seen.iter().any(meeting) {
            continue;
        }
        seen.push(enclosure);
        let (a, b) = (gap.a.bound(span(s), meter)?, gap.b.bound(span(t), meter)?);
        let mut apart = false;
        let mut unsure = None;
        for k in &others {
            let [x, _] = picked(&a, [*k, *k])?;
            let [y, _] = picked(&b, [*k, *k])?;
            let difference = x.sub(y)?;
            if difference.lo() > 0.0 || difference.hi() < 0.0 {
                apart = true;
            } else if difference.lo() != 0.0 || difference.hi() != 0.0 {
                unsure = Some(difference);
            }
        }
        if apart {
            continue;
        }
        let at = ParamBox::new(&[s, t])?;
        let xyz = grid_box(&a.union(&b).map_err(CurveError::from)?)?;
        items.push(match unsure {
            Some(g) => {
                let size = |v: f64| Steps(exact_steps(v));
                let gap = crate::result::StepsInterval::new(Steps(0), size(g.hi().abs()))
                    .ok_or(IntersectError::Internal)?;
                Item::Flagged {
                    at,
                    xyz,
                    why: Unresolved::NearMiss { gap },
                }
            }
            None => Item::Point {
                at,
                xyz,
                contact: PointContact::Crossing,
                class: certified(),
            },
        });
    }
    for ([s, t], why) in runs {
        let (a, b) = (gap.a.bound(span(s), meter)?, gap.b.bound(span(t), meter)?);
        let width = Steps(exact_steps(widest(&a)));
        let why = match why {
            Reason::Singular => second_order(gap, [s, t], width, meter)?,
            Reason::Unproven => Unresolved::UndecidedAtFloor {
                test: Test::Uniqueness,
            },
            Reason::NotSmooth => Unresolved::NonSmooth {
                at: crate::result::Degeneracy::Crease,
            },
        };
        items.push(Item::Flagged {
            at: ParamBox::new(&[s, t])?,
            xyz: grid_box(&a.union(&b).map_err(CurveError::from)?)?,
            why,
        });
    }
    Ok(items)
}

/// 12.9's second order test on a run of tangency boxes: the curves' signed curvatures, compared.
/// Where they differ the contact is isolated, its band √(2δ/Δκ) with δ the floor the boxes reached.
/// The band holds a touch, two crossings or nothing, and no interval can say which.
fn second_order<const D: usize>(
    gap: &Gap<'_, D>,
    [s, t]: [Interval; 2],
    run: Steps,
    meter: &mut WorkMeter<'_>,
) -> Result<Unresolved, IntersectError> {
    let not_smooth = Unresolved::NonSmooth {
        at: crate::result::Degeneracy::Crease,
    };
    // Over a run grown past where rounding swamps second derivatives, and on the domains.
    let [first, second] = gap.seams;
    let [s, t] = grown(&[s, t], &[first.whole()?, second.whole()?])?;
    let mut parts = [[Interval::point(0.0)?; 2]; 4];
    let wanted = [(gap.a, s, 1), (gap.a, s, 2), (gap.b, t, 1), (gap.b, t, 2)];
    for (slot, (curve, x, order)) in parts.iter_mut().zip(wanted) {
        let DerivativeBound::Bounded(d) = curve.derivative_bound(span(x), order, meter)? else {
            return Ok(not_smooth);
        };
        *slot = picked(&d, gap.axes)?;
    }
    let [a1, a2, b1, b2] = parts;
    let [ax, ay] = a1;
    let [bx, by] = b1;
    let (speed_a, speed_b) = (
        ax.square()?.add(ay.square()?)?,
        bx.square()?.add(by.square()?)?,
    );
    let along = ax.mul(bx)?.add(ay.mul(by)?)?;
    if speed_a.lo() <= 0.0 || speed_b.lo() <= 0.0 || along.contains(0.0) {
        return Ok(Unresolved::HigherOrderContact);
    }
    let curvature = |[x, y]: [Interval; 2], [u, v]: [Interval; 2], speed: Interval| {
        x.mul(v)?.sub(y.mul(u)?)?.div(speed.mul(speed.sqrt()?)?)
    };
    let (bent_a, bent_b) = (curvature(a1, a2, speed_a)?, curvature(b1, b2, speed_b)?);
    let difference = if along.lo() > 0.0 {
        bent_a.sub(bent_b)?
    } else {
        bent_a.add(bent_b)?
    };
    if difference.contains(0.0) {
        return Ok(Unresolved::HigherOrderContact);
    }
    let least = if difference.lo() > 0.0 {
        difference.lo()
    } else {
        -difference.hi()
    };
    let band = Interval::point(2.0 * RESOLUTION_FLOOR.get())?
        .div(Interval::point(least)?)?
        .sqrt()?;
    let width = Steps(exact_steps(band.hi()).max(run.0));
    Ok(Unresolved::TangentBand { width })
}

/// `v` steps, rounded up, for a band or gap.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a size in a frame's steps, below 2^53; a saturating cast is right for one past that"
)]
fn exact_steps(v: f64) -> u64 {
    math::ceil(v).map_or(u64::MAX, |r| r.get() as u64)
}

/// A line's origin and direction in the two coordinates.
fn in_plane<const D: usize>(
    line: &Line<D>,
    [i, j]: [usize; 2],
) -> Result<([f64; 2], [f64; 2]), IntersectError> {
    let (o, d) = (line.origin(), line.direction());
    let get = |v: &[f64; D], k: usize| v.get(k).copied().ok_or(IntersectError::Internal);
    Ok(([get(&o, i)?, get(&o, j)?], [get(&d, i)?, get(&d, j)?]))
}

/// Two lines in the plane of `axes`. The sign of the determinant decides parallel, and the same
/// expression on the origins decides collinear, both exactly.
fn line_line<const D: usize>(
    a: &Line<D>,
    b: &Line<D>,
    axes: [usize; 2],
    meter: &mut WorkMeter<'_>,
) -> Result<Intersection, IntersectError> {
    let ([ox, oy], [dx, dy]) = in_plane(a, axes)?;
    let ([px, py], [ex, ey]) = in_plane(b, axes)?;
    let (s_lo, s_hi) = ParamCurve::domain(a);
    let det = sign_of(&[(dx, ey), (-dy, ex)]);
    if det == Sign::Zero {
        // Parallel: collinear if the origins' difference is along d too.
        let across = sign_of(&[(px, dy), (-ox, dy), (-py, dx), (oy, dx)]);
        if across != Sign::Zero {
            return proven(Vec::new(), meter);
        }
        // Where b's ends fall on a. Two infinite lines overlap as far as the grid reaches.
        let (s_lo, s_hi) = line_span(a)?;
        let (t_lo, t_hi) = line_span(b)?;
        let along = |t: f64| -> Result<Interval, IntersectError> {
            let (x, y) = (
                Interval::point(px)?.add(Interval::point(t)?.mul(Interval::point(ex)?)?)?,
                Interval::point(py)?.add(Interval::point(t)?.mul(Interval::point(ey)?)?)?,
            );
            let (dx, dy) = (Interval::point(dx)?, Interval::point(dy)?);
            let top = x
                .sub(Interval::point(ox)?)?
                .mul(dx)?
                .add(y.sub(Interval::point(oy)?)?.mul(dy)?)?;
            Ok(top.div(dx.square()?.add(dy.square()?)?)?)
        };
        let (u, v) = (along(t_lo)?, along(t_hi)?);
        let (lo, hi) = if u.lo() <= v.lo() { (u, v) } else { (v, u) };
        let (start, end) = (
            if lo.lo() < s_lo { s_lo } else { lo.lo() },
            if hi.hi() > s_hi { s_hi } else { hi.hi() },
        );
        if start > end {
            return proven(Vec::new(), meter);
        }
        let at = ParamBox::new(&[Interval::new(start, end)?, Interval::new(t_lo, t_hi)?])?;
        let overlap = Item::Overlap {
            at,
            region: OverlapRegion {
                boundary: Vec::new(),
            },
            proof: Coincidence::Identical,
            class: Exactness::Exact,
        };
        return proven(alloc::vec![overlap], meter);
    }
    let i = |v: f64| Interval::point(v);
    let (wx, wy) = (i(px)?.sub(i(ox)?)?, i(py)?.sub(i(oy)?)?);
    let d = i(dx)?.mul(i(ey)?)?.sub(i(dy)?.mul(i(ex)?)?)?;
    let (t_lo, t_hi) = ParamCurve::domain(b);
    let s = wx.mul(i(ey)?)?.sub(wy.mul(i(ex)?)?)?.div(d)?;
    let t = wx.mul(i(dy)?)?.sub(wy.mul(i(dx)?)?)?.div(d)?;
    if s.hi() < s_lo || s.lo() > s_hi || t.hi() < t_lo || t.lo() > t_hi {
        return proven(Vec::new(), meter);
    }
    let boundary = s.contains(s_lo) || s.contains(s_hi) || t.contains(t_lo) || t.contains(t_hi);
    let contact = if boundary {
        PointContact::BoundaryTouch
    } else {
        PointContact::Crossing
    };
    let clip = |x: Interval, lo: f64, hi: f64| {
        Interval::new(
            if x.lo() < lo { lo } else { x.lo() },
            if x.hi() > hi { hi } else { x.hi() },
        )
    };
    let (s, t) = (clip(s, s_lo, s_hi)?, clip(t, t_lo, t_hi)?);
    let point = a.bound(span(s), meter)?;
    let item = Item::Point {
        at: ParamBox::new(&[s, t])?,
        xyz: grid_box(&point)?,
        contact,
        class: certified(),
    };
    proven(alloc::vec![item], meter)
}

/// The turn of `(alpha, beta)`, in half-turns, kept away from atan2's cut.
fn turn_of(alpha: Interval, beta: Interval) -> Result<Interval, IntersectError> {
    if alpha.hi() < 0.0 {
        let opposite = Interval::atan2(beta.neg(), alpha.neg())?.div(Interval::PI)?;
        Ok(opposite.add(Interval::point(1.0)?)?)
    } else {
        Ok(Interval::atan2(beta, alpha)?.div(Interval::PI)?)
    }
}

/// The parameter of a circle's point `(x, y)` from its centre.
fn turn<const D: usize>(
    circle: &Circle<D>,
    x: Interval,
    y: Interval,
    [i, j]: [usize; 2],
) -> Result<Interval, IntersectError> {
    let (_, [ux, uy, _]) = circle.frame().unit_vectors()?;
    let along = |u: [Interval; 3]| -> Result<Interval, IntersectError> {
        let (a, b) = (at(&u, i)?, at(&u, j)?);
        Ok(a.mul(x)?.add(b.mul(y)?)?)
    };
    turn_of(along(ux)?, along(uy)?)
}

/// Entry `k` of `v`.
fn at<T: Copy, const N: usize>(v: &[T; N], k: usize) -> Result<T, IntersectError> {
    v.get(k).copied().ok_or(IntersectError::Internal)
}

/// `t`, a number of half-turns, moved by whole turns onto `domain`, if any of it lands there.
fn onto(t: Interval, (lo, hi): (f64, f64)) -> Result<Option<Interval>, IntersectError> {
    let mut turns = math::floor((lo - t.hi()) / 2.0)?.get();
    for _ in 0..3 {
        let moved = t.add(Interval::point(2.0 * turns)?)?;
        if moved.hi() >= lo && moved.lo() <= hi {
            return Ok(Some(held(moved, (lo, hi))?));
        }
        turns += 1.0;
    }
    Ok(None)
}

/// `x`, cut to `domain`, which it meets.
fn held(x: Interval, (lo, hi): (f64, f64)) -> Result<Interval, IntersectError> {
    let (a, b) = (
        if x.lo() < lo { lo } else { x.lo() },
        if x.hi() > hi { hi } else { x.hi() },
    );
    Ok(Interval::new(a, b)?)
}

/// Two circles in the coordinates `axes`, the count of their crossings by exact signs.
fn circle_circle<const D: usize>(
    a: &Circle<D>,
    b: &Circle<D>,
    axes: [usize; 2],
    meter: &mut WorkMeter<'_>,
) -> Result<Intersection, IntersectError> {
    let (c, e) = (a.center(), b.center());
    let (r, q) = (a.radius(), b.radius());
    let [i, j] = axes;
    let (x0, y0, x1, y1) = (at(&c, i)?, at(&c, j)?, at(&e, i)?, at(&e, j)?);
    // |c − e|² − (r ± q)², written out so that every term is a product of two inputs.
    let compare = |sum: bool| -> Sign {
        let q2 = if sum { -q } else { q };
        sign_of(&[
            (x0, x0),
            (x1, x1),
            (y0, y0),
            (y1, y1),
            (-2.0 * x0, x1),
            (-2.0 * y0, y1),
            (-r, r),
            (-q, q),
            (2.0 * r, q2),
        ])
    };
    let (outer, inner) = (compare(true), compare(false));
    let same_centre = x0 == x1 && y0 == y1;
    if same_centre {
        if r == q {
            return one_circle(a, b, axes, meter);
        }
        return proven(Vec::new(), meter);
    }
    if outer == Sign::Positive || inner == Sign::Negative {
        return proven(Vec::new(), meter);
    }
    let tangent = outer == Sign::Zero || inner == Sign::Zero;
    let small_first = r <= q;
    let c = if small_first {
        Crossing::of([x0, y0], [x1, y1], r, q, tangent)?
    } else {
        Crossing::of([x1, y1], [x0, y0], q, r, tangent)?
    };
    // Two crossings the doubles cannot hold apart are one band around both.
    let merged = !tangent && c.height.lo() <= 0.0;
    let heights = if merged {
        alloc::vec![Interval::new(-c.height.hi(), c.height.hi())?]
    } else if tangent {
        alloc::vec![c.height]
    } else {
        alloc::vec![c.height, c.height.neg()]
    };
    let mut items = Vec::new();
    for h in heights {
        let (from_near, from_far) = c.point(h)?;
        let ([e, f], [g, k]) = if small_first {
            (from_near, from_far)
        } else {
            (from_far, from_near)
        };
        let (s, t) = (turn(a, e, f, axes)?, turn(b, g, k, axes)?);
        let (Some(s), Some(t)) = (
            onto(s, ParamCurve::domain(a))?,
            onto(t, ParamCurve::domain(b))?,
        ) else {
            continue;
        };
        let at = ParamBox::new(&[s, t])?;
        let xyz = grid_box(&a.bound(span(s), meter)?)?;
        items.push(if merged {
            Item::Flagged {
                at,
                xyz,
                why: Unresolved::TangentBand { width: Steps(1) },
            }
        } else {
            Item::Point {
                at,
                xyz,
                contact: if tangent {
                    PointContact::IsolatedTangent
                } else {
                    PointContact::Crossing
                },
                class: certified(),
            }
        });
    }
    let coverage = if merged {
        Coverage::CompleteOutsideFlagged
    } else {
        Coverage::TopologyCertified
    };
    Intersection::new(items, coverage, Vec::new(), meter)
}

/// The length of `(x, y)`; exactly `x`'s or `y`'s size when the other is zero.
fn length(x: Interval, y: Interval) -> Result<Interval, IntersectError> {
    let zero = |v: Interval| v.lo() == 0.0 && v.hi() == 0.0;
    let size = |v: Interval| if v.lo() < 0.0 { v.neg() } else { v };
    if zero(y) {
        Ok(size(x))
    } else if zero(x) {
        Ok(size(y))
    } else {
        Ok(tight::norm(&[x, y])?)
    }
}

/// Where two circles' crossings lie, measured from the smaller circle's centre.
struct Crossing {
    /// From the smaller circle's centre to the larger's.
    run: [Interval; 2],
    /// `run` as a unit vector.
    unit: [Interval; 2],
    /// How far along `unit` the crossings lie.
    foot: Interval,
    /// How far across it.
    height: Interval,
}

impl Crossing {
    /// The crossings of the circles of radii `small` and `large` about `near` and `far`. Measured
    /// from the smaller circle, at whose scale they lie, by Kahan's stable Heron: a small circle on
    /// a large one keeps its points apart, where r² − foot² has lost them. The arithmetic stays exact
    /// where it can, since those factors are differences of nearly equal lengths.
    fn of(
        near: [f64; 2],
        far: [f64; 2],
        small: f64,
        large: f64,
        tangent: bool,
    ) -> Result<Crossing, IntersectError> {
        let [p, q] = near;
        let [u, v] = far;
        let (dx, dy) = (
            tight::sub(tight::point(u)?, tight::point(p)?)?,
            tight::sub(tight::point(v)?, tight::point(q)?)?,
        );
        let d = length(dx, dy)?;
        let (s, l) = (tight::point(small)?, tight::point(large)?);
        let twice = tight::mul(d, tight::point(2.0)?)?;
        let foot = tight::add(
            tight::square(s)?,
            tight::mul(tight::sub(d, l)?, tight::add(d, l)?)?,
        )?;
        let foot = tight::div(foot, twice)?;
        let gap = tight::sub(l, s)?;
        let sum = tight::add(l, s)?;
        let area = tight::mul(
            tight::mul(tight::add(d, sum)?, tight::sub(sum, d)?)?,
            tight::mul(tight::sub(d, gap)?, tight::add(d, gap)?)?,
        )?;
        let height = if tangent {
            tight::point(0.0)?
        } else {
            let area = Interval::new(if area.lo() < 0.0 { 0.0 } else { area.lo() }, area.hi())?;
            tight::div(tight::sqrt(area)?, twice)?
        };
        Ok(Crossing {
            run: [dx, dy],
            unit: [tight::div(dx, d)?, tight::div(dy, d)?],
            foot,
            height,
        })
    }

    /// The crossing at height `h` from each circle's centre: the smaller's, then the larger's.
    fn point(&self, h: Interval) -> Result<([Interval; 2], [Interval; 2]), IntersectError> {
        let [ux, uy] = self.unit;
        let [dx, dy] = self.run;
        let x = self.foot.mul(ux)?.sub(h.mul(uy)?)?;
        let y = self.foot.mul(uy)?.add(h.mul(ux)?)?;
        Ok(([x, y], [x.sub(dx)?, y.sub(dy)?]))
    }
}

/// The whole of two curves that are one, exactly.
fn whole_overlap(
    s: (f64, f64),
    t: (f64, f64),
    meter: &mut WorkMeter<'_>,
) -> Result<Intersection, IntersectError> {
    let at = ParamBox::new(&[Interval::new(s.0, s.1)?, Interval::new(t.0, t.1)?])?;
    let overlap = Item::Overlap {
        at,
        region: OverlapRegion {
            boundary: Vec::new(),
        },
        proof: Coincidence::Identical,
        class: Exactness::Exact,
    };
    proven(alloc::vec![overlap], meter)
}

/// Two arcs of one circle. A point of `b` at t lies on `a` at φ ± t, with φ where `b` starts its
/// turns, so each of `b`'s ranges a whole turn apart can lie on `a`.
fn one_circle<const D: usize>(
    a: &Circle<D>,
    b: &Circle<D>,
    axes: [usize; 2],
    meter: &mut WorkMeter<'_>,
) -> Result<Intersection, IntersectError> {
    let (s, t) = (ParamCurve::domain(a), ParamCurve::domain(b));
    if s.1 - s.0 >= 2.0 && t.1 - t.0 >= 2.0 {
        return whole_overlap(s, t, meter);
    }
    let i = Interval::point;
    let (_, [_, ay, _]) = a.frame().unit_vectors()?;
    let (_, [bx, by, _]) = b.frame().unit_vectors()?;
    let [c, e] = axes;
    let phi = turn(a, at(&bx, c)?, at(&bx, e)?, axes)?;
    let way = at(&ay, c)?
        .mul(at(&by, c)?)?
        .add(at(&ay, e)?.mul(at(&by, e)?)?)?;
    let sign = if way.lo() > 0.0 { 1.0 } else { -1.0 };
    let ends = [
        phi.add(i(sign)?.mul(i(t.0)?)?)?,
        phi.add(i(sign)?.mul(i(t.1)?)?)?,
    ];
    let [low, high] = if sign > 0.0 {
        ends
    } else {
        let [first, second] = ends;
        [second, first]
    };
    let mut turns = math::floor((s.0 - high.hi()) / 2.0)?.get();
    let mut items = Vec::new();
    for _ in 0..3 {
        let shift = i(2.0 * turns)?;
        turns += 1.0;
        let (lo, hi) = (low.add(shift)?, high.add(shift)?);
        let (start, end) = (
            if lo.lo() < s.0 { s.0 } else { lo.lo() },
            if hi.hi() > s.1 { s.1 } else { hi.hi() },
        );
        if start > end {
            continue;
        }
        let part = Interval::new(start, end)?;
        let on_b = i(sign)?.mul(part.sub(phi.add(shift)?)?)?;
        let at = ParamBox::new(&[part, held(on_b, t)?])?;
        let touch = hi.contains(s.0) || lo.contains(s.1);
        let box_of = a.bound(span(part), meter)?;
        items.push(if touch {
            Item::Point {
                at,
                xyz: grid_box(&box_of)?,
                contact: PointContact::BoundaryTouch,
                class: certified(),
            }
        } else {
            Item::Overlap {
                at,
                region: OverlapRegion {
                    boundary: Vec::new(),
                },
                proof: Coincidence::Identical,
                class: certified(),
            }
        });
    }
    proven(items, meter)
}

/// `v` as a grid coordinate, if it is a whole number in reach.
fn whole(v: f64) -> Option<i64> {
    let floor = math::floor(v).ok()?.get();
    (floor == v && v.abs() <= REACH).then(|| exact_i64(v))
}

/// `v`'s coordinates as a grid point, if they are whole.
fn grid_point<const D: usize>(v: &[f64; D]) -> Option<P3> {
    let at = |k: usize| whole(v.get(k).copied().unwrap_or(0.0));
    P3::new(at(0)?, at(1)?, at(2)?).ok()
}

/// The line's two points, `o` and `o + d`, if they are on the grid.
fn grid_line<const D: usize>(line: &Line<D>) -> Option<quadric::Line> {
    let (o, d) = (line.origin(), line.direction());
    let mut far = o;
    for (x, y) in far.iter_mut().zip(&d) {
        *x += y;
    }
    Some(quadric::Line {
        a: grid_point(&o)?,
        b: grid_point(&far)?,
    })
}

/// How a line meets a circle in its plane, exactly, where their data are whole.
fn exactly<const D: usize>(line: &Line<D>, circle: &Circle<D>) -> Option<Meeting> {
    let sphere = quadric::Sphere {
        centre: grid_point(&circle.center())?,
        radius: Steps(u64::try_from(whole(circle.radius())?).ok()?),
    };
    closed::line_sphere(&grid_line(line)?, &sphere).ok()
}

/// How a conic runs in its two vectors: c + αu + βv with (α, β) on a unit shape.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// α² + β² = 1: a circle or an ellipse, α = cos πt and β = sin πt.
    Turn,
    /// α² − β² = 1 with α > 0: a branch of a hyperbola, α = cosh t and β = sinh t.
    Branch,
    /// β² = α: a parabola, α = t² and β = t.
    Arch,
}

/// A conic in the plane, as its form's vectors.
struct Conic {
    centre: [Interval; 2],
    u: [Interval; 2],
    v: [Interval; 2],
    shape: Shape,
    domain: (f64, f64),
}

/// Two coordinates of a point.
fn lifted<const D: usize>(
    p: &[f64; D],
    [i, j]: [usize; 2],
) -> Result<[Interval; 2], IntersectError> {
    Ok([Interval::point(at(p, i)?)?, Interval::point(at(p, j)?)?])
}

/// u = a·x̂ and v = b·ŷ, enclosed.
fn frame_axes<const D: usize>(
    frame: &Frame<D>,
    a: f64,
    b: f64,
    [i, j]: [usize; 2],
) -> Result<([Interval; 2], [Interval; 2]), IntersectError> {
    let (_, [x, y, _]) = frame.unit_vectors()?;
    let (a, b) = (Interval::point(a)?, Interval::point(b)?);
    Ok((
        [at(&x, i)?.mul(a)?, at(&x, j)?.mul(a)?],
        [at(&y, i)?.mul(b)?, at(&y, j)?.mul(b)?],
    ))
}

/// A conic's two vectors.
fn conic_axes<const D: usize>(
    axes: &Axes<D>,
    coordinates: [usize; 2],
) -> Result<([Interval; 2], [Interval; 2]), IntersectError> {
    match axes {
        Axes::Frame { frame, a, b } => frame_axes(frame, *a, *b, coordinates),
        Axes::Vectors { u, v } => Ok((lifted(u, coordinates)?, lifted(v, coordinates)?)),
    }
}

/// `curve`, if it is a conic.
fn conic_of<const D: usize>(
    curve: &CurveKind<D>,
    coordinates: [usize; 2],
) -> Result<Option<Conic>, IntersectError> {
    let (centre, (u, v), shape) = match curve {
        CurveKind::Circle(c) => (
            c.center(),
            frame_axes(&c.frame(), c.radius(), c.radius(), coordinates)?,
            Shape::Turn,
        ),
        CurveKind::Ellipse(c) => (c.center(), conic_axes(&c.axes(), coordinates)?, Shape::Turn),
        CurveKind::Hyperbola(c) => (
            c.center(),
            conic_axes(&c.axes(), coordinates)?,
            Shape::Branch,
        ),
        CurveKind::Parabola(c) => (c.vertex(), conic_axes(&c.axes(), coordinates)?, Shape::Arch),
        _ => return Ok(None),
    };
    Ok(Some(Conic {
        centre: lifted(&centre, coordinates)?,
        u,
        v,
        shape,
        domain: curve.domain(),
    }))
}

/// The roots of a·s² + 2b·s + c, in order, by the count `exact` gives or the discriminant's sign.
/// `None` where an interval cannot tell.
fn quadratic(
    [a, b, c]: [Interval; 3],
    exact: Option<Meeting>,
) -> Result<Option<Vec<Interval>>, IntersectError> {
    let flat = a.lo() == 0.0 && a.hi() == 0.0;
    if a.contains(0.0) && !flat {
        return Ok(None);
    }
    if flat {
        if b.contains(0.0) {
            return Ok(None);
        }
        let root = c.neg().div(b.mul(Interval::point(2.0)?)?)?;
        return Ok(Some(alloc::vec![root]));
    }
    let d = b.square()?.sub(a.mul(c)?)?;
    let meeting = match exact {
        Some(known) => known,
        None if d.hi() < 0.0 => Meeting::Nowhere,
        None if d.lo() > 0.0 => Meeting::Twice,
        None => return Ok(None),
    };
    Ok(Some(match meeting {
        Meeting::Nowhere => Vec::new(),
        Meeting::Twice => {
            // An exact count may rest on a discriminant whose enclosure reaches below zero.
            let root = Interval::new(if d.lo() < 0.0 { 0.0 } else { d.lo() }, d.hi())?.sqrt()?;
            let first = b.neg().sub(root)?.div(a)?;
            let second = b.neg().add(root)?.div(a)?;
            let (low, high) = if first.lo() <= second.lo() {
                (first, second)
            } else {
                (second, first)
            };
            if low.hi() >= high.lo() {
                return Ok(None);
            }
            alloc::vec![low, high]
        }
        _ => alloc::vec![b.neg().div(a)?],
    }))
}

/// A line with a conic in the plane. Written in the conic's own coordinates the line is a quadratic
/// in s, A·s² + 2B·s + C, and B² − AC counts the meetings. `None` where an interval cannot tell:
/// the engine then looks at the pair, and a tangency is flagged there.
fn line_conic<const D: usize>(
    line: &Line<D>,
    conic: &Conic,
    coordinates: [usize; 2],
    exact: Option<Meeting>,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Intersection>, IntersectError> {
    let i = Interval::point;
    let ([ox, oy], [dx, dy]) = in_plane(line, coordinates)?;
    let ([cx, cy], [ux, uy], [vx, vy]) = (conic.centre, conic.u, conic.v);
    let det = ux.mul(vy)?.sub(uy.mul(vx)?)?;
    if det.contains(0.0) {
        return Ok(None);
    }
    let (wx, wy, dx, dy) = (i(ox)?.sub(cx)?, i(oy)?.sub(cy)?, i(dx)?, i(dy)?);
    let a0 = wx.mul(vy)?.sub(wy.mul(vx)?)?.div(det)?;
    let b0 = ux.mul(wy)?.sub(uy.mul(wx)?)?.div(det)?;
    let a1 = dx.mul(vy)?.sub(dy.mul(vx)?)?.div(det)?;
    let b1 = ux.mul(dy)?.sub(uy.mul(dx)?)?.div(det)?;
    let one = i(1.0)?;
    let (a, b, c) = match conic.shape {
        Shape::Turn => (
            a1.square()?.add(b1.square()?)?,
            a0.mul(a1)?.add(b0.mul(b1)?)?,
            a0.square()?.add(b0.square()?)?.sub(one)?,
        ),
        Shape::Branch => (
            a1.square()?.sub(b1.square()?)?,
            a0.mul(a1)?.sub(b0.mul(b1)?)?,
            a0.square()?.sub(b0.square()?)?.sub(one)?,
        ),
        Shape::Arch => (
            b1.square()?,
            b0.mul(b1)?.sub(a1.mul(i(0.5)?)?)?,
            b0.square()?.sub(a0)?,
        ),
    };
    let Some(roots) = quadratic([a, b, c], exact)? else {
        return Ok(None);
    };
    let tangent = exact == Some(Meeting::Tangent);
    let domain = ParamCurve::domain(line);
    let mut items = Vec::new();
    for s in roots {
        if s.hi() < domain.0 || s.lo() > domain.1 {
            continue;
        }
        let (alpha, beta) = (a0.add(s.mul(a1)?)?, b0.add(s.mul(b1)?)?);
        let t = match conic.shape {
            Shape::Turn => onto(turn_of(alpha, beta)?, conic.domain)?,
            Shape::Branch if alpha.hi() < 0.0 => None,
            Shape::Branch => {
                let t = alpha.add(beta)?.ln()?;
                (t.hi() >= conic.domain.0 && t.lo() <= conic.domain.1)
                    .then(|| held(t, conic.domain))
                    .transpose()?
            }
            Shape::Arch => (beta.hi() >= conic.domain.0 && beta.lo() <= conic.domain.1)
                .then(|| held(beta, conic.domain))
                .transpose()?,
        };
        let Some(t) = t else {
            continue;
        };
        let end = |x: Interval, (lo, hi): (f64, f64)| x.contains(lo) || x.contains(hi);
        let contact = if tangent {
            PointContact::IsolatedTangent
        } else if end(s, domain) || end(t, conic.domain) {
            PointContact::BoundaryTouch
        } else {
            PointContact::Crossing
        };
        let s = held(s, domain)?;
        items.push(Item::Point {
            at: ParamBox::new(&[s, t])?,
            xyz: grid_box(&line.bound(span(s), meter)?)?,
            contact,
            class: certified(),
        });
    }
    proven(items, meter).map(Some)
}

/// The determinant of three vectors of a line pair, enclosed.
fn volume(
    d: [Interval; 3],
    e: [Interval; 3],
    w: [Interval; 3],
) -> Result<Interval, IntersectError> {
    let [a, b, c] = d;
    let [x, y, z] = e;
    let [p, q, r] = w;
    Ok(a.mul(y.mul(r)?.sub(z.mul(q)?)?)?
        .sub(b.mul(x.mul(r)?.sub(z.mul(p)?)?)?)?
        .add(c.mul(x.mul(q)?.sub(y.mul(p)?)?)?)?)
}

/// The three coordinates of `v`, enclosed.
fn three<const D: usize>(v: &[f64; D]) -> Result<[Interval; 3], IntersectError> {
    let at = |k: usize| Interval::point(v.get(k).copied().unwrap_or(0.0));
    Ok([at(0)?, at(1)?, at(2)?])
}

/// `u × v` in doubles, for choosing a plane.
fn normal(u: [f64; 3], v: [f64; 3]) -> [f64; 3] {
    let [a, b, c] = u;
    let [x, y, z] = v;
    [b * z - c * y, c * x - a * z, a * y - b * x]
}

/// The two coordinates that keep the most of a plane with normal `n`.
fn across(n: [f64; 3]) -> [usize; 2] {
    let [a, b, c] = n.map(f64::abs);
    if c >= a && c >= b {
        [0, 1]
    } else if b >= a {
        [0, 2]
    } else {
        [1, 2]
    }
}

/// `v`'s three coordinates in doubles.
fn doubles<const D: usize>(v: &[f64; D]) -> [f64; 3] {
    let at = |k: usize| v.get(k).copied().unwrap_or(0.0);
    [at(0), at(1), at(2)]
}

/// Two lines in space. A volume that is proven apart from zero, or an exact sign on whole data,
/// says skew; a coplanar pair is decided in its plane. `None` where neither can tell.
fn lines_in_space<const D: usize>(
    a: &Line<D>,
    b: &Line<D>,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Intersection>, IntersectError> {
    let (o, d, p, e) = (a.origin(), a.direction(), b.origin(), b.direction());
    let mut w = [Interval::point(0.0)?; 3];
    for (slot, (x, y)) in w.iter_mut().zip(three(&p)?.iter().zip(&three(&o)?)) {
        *slot = x.sub(*y)?;
    }
    let v = volume(three(&d)?, three(&e)?, w)?;
    if !v.contains(0.0) {
        return proven(Vec::new(), meter).map(Some);
    }
    let (Some(x), Some(y)) = (grid_line(a), grid_line(b)) else {
        return Ok(None);
    };
    if predicate::orient3d(x.a, x.b, y.a, y.b)? != Sign::Zero {
        return proven(Vec::new(), meter).map(Some);
    }
    let (dir, other) = (doubles(&d), doubles(&e));
    let mut n = normal(dir, other);
    if n == [0.0; 3] {
        let [wx, wy, wz] = w.map(|c| c.lo() * 0.5 + c.hi() * 0.5);
        n = normal(dir, [wx, wy, wz]);
    }
    if n == [0.0; 3] {
        // One line: the two coordinates along which it runs most.
        let [p, q, r] = dir.map(f64::abs);
        n = if r <= p && r <= q {
            [0.0, 0.0, 1.0]
        } else if q <= p {
            [0.0, 1.0, 0.0]
        } else {
            [1.0, 0.0, 0.0]
        };
    }
    line_line(a, b, across(n), meter).map(Some)
}

/// The vector from `p` to `q`.
fn between_points(p: [f64; 3], q: [f64; 3]) -> [f64; 3] {
    let [a, b, c] = p;
    let [x, y, z] = q;
    [x - a, y - b, z - c]
}

/// Points that fix where `curve` lies, as whole coordinates, for the kinds that lie in a plane.
fn fixed_by<const D: usize>(curve: &CurveKind<D>) -> Option<Vec<[f64; 3]>> {
    let flat = |frame: &Frame<D>| -> [[f64; 3]; 2] {
        let [a, b, c] = doubles(&frame.x());
        let n = frame.normal();
        let p = normal(n, [a, b, c]);
        [p, normal(p, n)]
    };
    let from = |centre: [f64; 3], [u, v]: [[f64; 3]; 2]| {
        let [a, b, c] = centre;
        let moved = |[x, y, z]: [f64; 3]| [a + x, b + y, c + z];
        alloc::vec![centre, moved(u), moved(v)]
    };
    let vectors = |axes: &Axes<D>| match axes {
        Axes::Frame { frame, .. } => flat(frame),
        Axes::Vectors { u, v } => [doubles(u), doubles(v)],
    };
    let points = match curve {
        CurveKind::Line(l) => {
            let o = doubles(&l.origin());
            from(o, [doubles(&l.direction()); 2])
        }
        CurveKind::Circle(c) => from(doubles(&c.center()), flat(&c.frame())),
        CurveKind::Ellipse(c) => from(doubles(&c.center()), vectors(&c.axes())),
        CurveKind::Hyperbola(c) => from(doubles(&c.center()), vectors(&c.axes())),
        CurveKind::Parabola(c) => from(doubles(&c.vertex()), vectors(&c.axes())),
        CurveKind::Nurbs(c) if !c.is_rational() => c.homogeneous().iter().map(doubles).collect(),
        _ => return None,
    };
    points
        .iter()
        .all(|p| p.iter().all(|x| whole(*x).is_some()))
        .then_some(points)
}

/// The normal of the plane `a` and `b` both lie in, proven by exact signs on whole-number data: the
/// projection of such a pair on two coordinates loses nothing, so it needs no third coordinate.
fn shared_plane<const D: usize>(a: &CurveKind<D>, b: &CurveKind<D>) -> Option<[f64; 3]> {
    let mut points = fixed_by(a)?;
    points.extend(fixed_by(b)?);
    let first = *points.first()?;
    let (corner, n) = points.iter().enumerate().find_map(|(k, q)| {
        let u = between_points(first, *q);
        points.iter().skip(k).find_map(|r| {
            let v = between_points(first, *r);
            let [p, q, w] = u;
            let [x, y, z] = v;
            let parallel = [
                sign_of(&[(q, z), (-w, y)]),
                sign_of(&[(w, x), (-p, z)]),
                sign_of(&[(p, y), (-q, x)]),
            ];
            parallel.iter().any(|s| *s != Sign::Zero).then_some((
                [*r, [first[0] + p, first[1] + q, first[2] + w]],
                normal(u, v),
            ))
        })
    })?;
    let [r, q] = corner;
    let grid = |p: &[f64; 3]| grid_point(p);
    let (g, h, k) = (grid(&first)?, grid(&q)?, grid(&r)?);
    points
        .iter()
        .all(|p| grid(p).is_some_and(|x| predicate::orient3d(g, h, k, x) == Ok(Sign::Zero)))
        .then_some(n)
}

/// The coordinates of the plane two circles lie in, where it is a coordinate plane: both axes along
/// one coordinate, the centres level in it, by the stored doubles.
fn level<const D: usize>(a: &Circle<D>, b: &Circle<D>) -> Option<[usize; 2]> {
    if D == 2 {
        return Some([0, 1]);
    }
    let (m, n) = (a.frame().normal(), b.frame().normal());
    let along = |v: &[f64; 3]| {
        let mut found = None;
        for (k, x) in v.iter().enumerate() {
            if *x != 0.0 {
                if found.is_some() {
                    return None;
                }
                found = Some(k);
            }
        }
        found
    };
    let k = along(&m)?;
    (along(&n)? == k && a.center().get(k) == b.center().get(k)).then_some(match k {
        0 => [1, 2],
        1 => [0, 2],
        _ => [0, 1],
    })
}

/// The closed form for `a` and `b`, in that order, if there is one that decides.
fn closed_form<const D: usize>(
    a: &CurveKind<D>,
    b: &CurveKind<D>,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Intersection>, IntersectError> {
    let flat = D == 2;
    match (a, b) {
        (CurveKind::Line(x), CurveKind::Line(y)) if flat => {
            line_line(x, y, [0, 1], meter).map(Some)
        }
        (CurveKind::Line(x), CurveKind::Line(y)) => lines_in_space(x, y, meter),
        (CurveKind::Circle(x), CurveKind::Circle(y)) => match level(x, y) {
            Some(axes) => circle_circle(x, y, axes, meter).map(Some),
            None => Ok(None),
        },
        (CurveKind::Line(x), other) => {
            let axes = if flat {
                Some([0, 1])
            } else {
                shared_plane(a, b).map(across)
            };
            let Some(axes) = axes else {
                return Ok(None);
            };
            let Some(conic) = conic_of(other, axes)? else {
                return Ok(None);
            };
            let exact = match other {
                CurveKind::Circle(c) => exactly(x, c),
                _ => None,
            };
            line_conic(x, &conic, axes, exact, meter)
        }
        _ => Ok(None),
    }
}

/// `line`'s domain, an infinite end moved to where it leaves the grid, past which nothing lies.
fn line_span<const D: usize>(line: &Line<D>) -> Result<(f64, f64), IntersectError> {
    let (mut lo, mut hi) = ParamCurve::domain(line);
    if lo.is_finite() && hi.is_finite() {
        return Ok((lo, hi));
    }
    let i = Interval::point;
    for (o, d) in line.origin().iter().zip(&line.direction()) {
        if *d == 0.0 {
            continue;
        }
        let (u, v) = (
            i(-REACH)?.sub(i(*o)?)?.div(i(*d)?)?,
            i(REACH)?.sub(i(*o)?)?.div(i(*d)?)?,
        );
        let (first, last) = if u.lo() < v.lo() { (u, v) } else { (v, u) };
        lo = if first.lo() > lo { first.lo() } else { lo };
        hi = if last.hi() < hi { last.hi() } else { hi };
    }
    if lo <= hi {
        Ok((lo, hi))
    } else {
        Err(IntersectError::FrameRange)
    }
}

/// `curve`'s domain, finite.
fn finite<const D: usize>(curve: &CurveKind<D>) -> Result<(f64, f64), IntersectError> {
    match curve {
        CurveKind::Line(line) => line_span(line),
        _ => Ok(curve.domain()),
    }
}

/// The two coordinates to solve in: in 3D, the two that spread the pair most.
fn plane<const D: usize>(
    a: &CurveKind<D>,
    b: &CurveKind<D>,
    (s, t): ((f64, f64), (f64, f64)),
    meter: &mut WorkMeter<'_>,
) -> Result<[usize; 2], IntersectError> {
    if D < 3 {
        return Ok([0, 1]);
    }
    let (x, y) = (a.bound(s, meter)?, b.bound(t, meter)?);
    let hull = x.union(&y).map_err(CurveError::from)?;
    let mut widths = [0.0; 3];
    for (slot, (lo, hi)) in widths.iter_mut().zip(hull.lo.iter().zip(&hull.hi)) {
        *slot = hi - lo;
    }
    Ok(across(widths.map(|w| 1.0 / (1.0 + w))))
}

/// Where `a` and `b` meet (12.5).
///
/// # Errors
///
/// A kind's errors, and the meter's stop: a pair of kinds is always answered, so
/// [`IntersectError::BudgetExceeded`] means the budget ran out, not that the pair is unsupported.
pub fn curves<const D: usize>(
    a: &CurveKind<D>,
    b: &CurveKind<D>,
    meter: &mut WorkMeter<'_>,
) -> Result<Intersection, IntersectError> {
    meter.poll()?;
    if a == b {
        return whole_overlap(a.domain(), b.domain(), meter);
    }
    if let Some(answer) = closed_form(a, b, meter)? {
        return Ok(answer);
    }
    let turned = matches!(b, CurveKind::Line(_)) && !matches!(a, CurveKind::Line(_));
    let answer = if turned {
        closed_form(b, a, meter)?
    } else {
        None
    };
    if let Some(answer) = answer {
        return Ok(answer.swapped(1));
    }
    let (s, t) = (finite(a)?, finite(b)?);
    let shared = if D == 3 { shared_plane(a, b) } else { None };
    let seams = [seam_of(a, s), seam_of(b, t)];
    let gap = Gap {
        a,
        b,
        seams,
        axes: match shared {
            Some(n) => across(n),
            None => plane(a, b, (s, t), meter)?,
        },
        coplanar: shared.is_some(),
        pieces_a: pieces(a, meter)?,
        pieces_b: pieces(b, meter)?,
    };
    let [first, second] = seams;
    let domain = [first.side()?, second.side()?];
    let solved = solve(&gap, domain, |_, _| Ok(true), meter)?;
    // 12.7: every change of route is on the record. A pair with a closed form that could not decide
    // came here for that reason.
    let line = |c: &CurveKind<D>| matches!(c, CurveKind::Line(_));
    let conic = |c: &CurveKind<D>| {
        matches!(
            c,
            CurveKind::Circle(_)
                | CurveKind::Ellipse(_)
                | CurveKind::Hyperbola(_)
                | CurveKind::Parabola(_)
        )
    };
    let circle = |c: &CurveKind<D>| matches!(c, CurveKind::Circle(_));
    let closed =
        (line(a) && (line(b) || conic(b))) || (conic(a) && line(b)) || (circle(a) && circle(b));
    let route = if closed {
        alloc::vec![Diag { code: ROUTE.into() }]
    } else {
        Vec::new()
    };
    finished(&gap, solved, route, meter)
}

/// The answer from what the engine found.
fn finished<const D: usize>(
    gap: &Gap<'_, D>,
    solved: Solved<2>,
    route: Vec<Diag>,
    meter: &mut WorkMeter<'_>,
) -> Result<Intersection, IntersectError> {
    let items = items_of(gap, solved, meter)?;
    let flagged = items.iter().any(|i| matches!(i, Item::Flagged { .. }));
    let coverage = if flagged {
        Coverage::CompleteOutsideFlagged
    } else {
        Coverage::TopologyCertified
    };
    Intersection::new(items, coverage, route, meter)
}

/// A tangent of `curve` at `t`, in doubles.
fn heading<const D: usize>(
    curve: &CurveKind<D>,
    t: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<[f64; D], IntersectError> {
    let at = curve.enclose(t, Limit::FromRight, 1, meter)?;
    let tangent = at.get(1).ok_or(IntersectError::Internal)?;
    Ok(tangent.map(tight::middle))
}

/// Whether `curve` runs along `direction` everywhere on `x`, so it cannot return there.
fn runs_along<const D: usize>(
    curve: &CurveKind<D>,
    direction: &[f64; D],
    x: (f64, f64),
    meter: &mut WorkMeter<'_>,
) -> Result<bool, IntersectError> {
    let DerivativeBound::Bounded(d) = curve.derivative_bound(x, 1, meter)? else {
        return Ok(false);
    };
    let mut along = Interval::point(0.0)?;
    for ((m, lo), hi) in direction.iter().zip(&d.lo).zip(&d.hi) {
        along = along.add(Interval::point(*m)?.mul(Interval::new(*lo, *hi)?)?)?;
    }
    Ok(along.lo() > 0.0)
}

/// Where `curve` crosses itself. A closed curve's ends are one point and not a crossing.
///
/// # Errors
///
/// As [`curves`].
pub fn self_crossings<const D: usize>(
    curve: &CurveKind<D>,
    meter: &mut WorkMeter<'_>,
) -> Result<Intersection, IntersectError> {
    meter.poll()?;
    let (lo, hi) = finite(curve)?;
    let gap = Gap {
        a: curve,
        b: curve,
        seams: [Seam::open((lo, hi)); 2],
        axes: plane(curve, curve, ((lo, hi), (lo, hi)), meter)?,
        coplanar: false,
        pieces_a: Vec::new(),
        pieces_b: Vec::new(),
    };
    let side = Interval::new(lo, hi)?;
    let closed = curve.point(lo, Limit::FromRight)? == curve.point(hi, Limit::FromLeft)?;
    let keep = |x: &[Interval; 2], m: &mut WorkMeter<'_>| -> Result<bool, IntersectError> {
        let [s, t] = x;
        if s.lo() >= t.hi() {
            return Ok(false);
        }
        let hull = (
            if s.lo() < t.lo() { s.lo() } else { t.lo() },
            if s.hi() > t.hi() { s.hi() } else { t.hi() },
        );
        let along = heading(curve, hull.0 * 0.5 + hull.1 * 0.5, m)?;
        if runs_along(curve, &along, hull, m)? {
            return Ok(false);
        }
        // The short way round a closed curve, through its join.
        if closed {
            let along = heading(curve, lo, m)?;
            let through = runs_along(curve, &along, (t.lo(), hi), m)?
                && runs_along(curve, &along, (lo, s.hi()), m)?;
            return Ok(!through);
        }
        Ok(true)
    };
    let mut solved = solve(&gap, [side, side], keep, meter)?;
    // The join of a closed curve, and any zero of the diagonal itself, are no crossing.
    solved.zeros.retain(|z| {
        let [s, t] = z.enclosure;
        let join = s.contains(lo) && t.contains(hi);
        let diagonal = s.lo() <= t.hi() && t.lo() <= s.hi();
        !(join || diagonal)
    });
    solved.flagged.retain(|f| {
        let [s, t] = f.region;
        !(s.contains(lo) && t.contains(hi))
    });
    finished(&gap, solved, Vec::new(), meter)
}
