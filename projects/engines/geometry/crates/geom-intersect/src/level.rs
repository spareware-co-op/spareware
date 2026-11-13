//! Level curves and the implicit-parametric tracer: the zero set of a function on a surface's
//! parameter plane, traced with proofs (12.8, 12.13).
//!
//! What it does: [`trace`] finds every branch of g(u, v) = 0 on a surface. g is a [`Level`]: an
//! implicit surface F substituted into the parametric one ([`OnImplicit`]), or a function of the
//! surface's own normal, for silhouettes ([`Silhouette`], [`Perspective`]), isoclines
//! ([`Isocline`]) and envelopes ([`Envelope`]). The domain is cut at the surface's breaks, then
//! each box is excluded, settled or split, and the settled boxes' crossings are joined across their
//! shared sides into branches.
//!
//! Guarantees:
//!
//! - **Nothing is missed.** A box is dropped only where g is proven nonzero (Snyder's exclusion),
//!   or where g is monotone in one parameter and no side holds a root: a monotone box holds no
//!   closed loop, so any curve in it would cross a side.
//! - **No branch is jumped.** In a settled box the curve is a graph over one parameter, so its arcs
//!   lie over disjoint spans of that parameter, and sorting the crossings along it pairs them.
//! - **Everything is accounted for.** Each crossing is met by one crossing across its side, or ends
//!   at the domain's boundary, at a break, or at a flagged box. Any other crossing is
//!   [`IntersectError::Internal`], never a branch quietly ended.
//! - **What cannot be decided is flagged.** A box at the floor that holds a possible singular
//!   point, a crease, or a side whose roots cannot be told apart is a [`FlaggedBox`] (for 12.9).
//! - **Each branch has a bound.** [`Branch::deviation`] bounds, in space, how far the exact branch
//!   is from the surface's image of the straight parameter chords through its vertices.
//! - **Deterministic.** A fixed cut position, first part first, and branches in the order of their
//!   first vertices.
//!
//! Method: J. M. Snyder, Interval analysis for computer graphics, SIGGRAPH (1992),
//! <https://doi.org/10.1145/142920.134024>, for the exclusion and parameterisability tests, as
//! 12.8 sets them out. S. Plantinga and G. Vegter's small-normal-variation test (2004,
//! <https://doi.org/10.1145/1057432.1057465>) needs no code of its own: if both partials' enclosures
//! span zero, the enclosure of the gradient's inner product with itself spans zero as well, so a box
//! it accepts is one Snyder's test accepts already. Values take the mean-value form over a box
//! (R. E. Moore, R. B. Kearfott and M. J. Cloud, *Introduction to Interval Analysis*, SIAM (2009),
//! <https://doi.org/10.1137/1.9780898717716>, section 6.4), so exclusion tightens with the square
//! of a box's width. An arc over a span w whose slopes lie in an interval of width m leaves its
//! chord by at most m·w/4, which with the surface's speed across the arc is [`Branch::deviation`].
//!
//! A cut falls 7/16 of the way across a box: the new coordinate has more binary digits than any
//! before it, with an odd last one, so it never lands on a dyadic middle, which a symmetric curve
//! runs through. A corner on the curve could not be decided, so a cut whose ends g cannot sign
//! moves to another share. Every corner is the end of some cut, so only the domain's own corners
//! can sit on the curve, and those are flagged.
//!
//! Across a seam (G173): where the domain's two ends in a parameter are one curve
//! ([`crate::seam`]), their two lines are stitched as one, so a branch goes on across; a crossing
//! of a closing end or of a knot line not proven tangent-continuous is kept once and named in
//! [`Branch::passes`]. A branch whose crossing has no partner across an inner break still ends as
//! [`CurveEnd::Seam`].
//!
//! What this piece leaves: flagged boxes go to 12.9's analysis (G171). The
//! implicit surface is taken whole: an [`OnImplicit`] on a trimmed kind finds the kind's untrimmed
//! equation, and whoever pairs the two operands trims the answer to the other domain.
//!
//! Cost: each box or span takes two reads of the surface's partials and the function, charged to
//! the meter. Boxes along a branch of length L number about L·√(κ/ceiling) for curvature κ, with
//! their neighbours; a side's crossing takes log₂(side/floor) reads more. Recursion: none.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are chapter 12's: g the function, S the surface, N its normal, u and v the parameters"
)]

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use geom_curve::tight::{larger, smaller};
use geom_num::NumError;
use geom_num::interval::Interval;
use geom_nurbs::Limit;
use geom_run::WorkMeter;
use geom_surface::{ImplicitBound, ParamBox as Region, ParamSurface, SurfaceKind};

use crate::IntersectError;
use crate::project::{dot, exact, halfway, negative, positive};
use crate::result::{CurveEnd, Degeneracy, Test, Unresolved};
use crate::seam::{Charts, Pass, Passing, charts};

/// What a read costs the meter, besides the surface's own charges.
const STEP: u64 = 1;

/// Where a box or span is cut, as shares of its width, in the order they are tried (module
/// documentation).
const CUTS: [f64; 3] = [0.4375, 0.3125, 0.5625];

/// g and its partial derivatives, enclosed over a box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    /// Every value of g.
    pub value: Interval,
    /// Every value of `g_u`.
    pub du: Interval,
    /// Every value of `g_v`.
    pub dv: Interval,
    /// Whether g is continuously differentiable throughout the box.
    pub smooth: bool,
}

/// A function on a surface's parameter plane, whose zero set [`trace`] finds.
pub trait Level {
    /// The order of the surface's partials [`Level::sample`] reads: at least 1.
    fn order(&self) -> usize;

    /// g and its partials, from enclosures of the surface's partials to [`Level::order`], listed by
    /// [`geom_surface::partial_index`].
    ///
    /// # Errors
    ///
    /// [`IntersectError::Internal`] for too few partials; an enclosure's own errors.
    fn sample(&self, partials: &[[Interval; 3]]) -> Result<Sample, IntersectError>;
}

/// `F(S(u, v))`, an implicit surface substituted into the parametric one (12.8).
#[derive(Clone, Copy)]
pub struct OnImplicit<'a>(pub &'a dyn ImplicitBound);

/// `N·d`: a silhouette in parallel view, or a parting line for pull direction d (12.13). It is also
/// the envelope of a surface translating along d.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Silhouette {
    /// The view or pull direction.
    pub direction: [f64; 3],
}

/// `N·(S − E)`: a silhouette in perspective from the eye E (12.13).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Perspective {
    /// The eye.
    pub eye: [f64; 3],
}

/// `(N·d)² − cos²θ·(N·N)(d·d)`: where the normal meets d at angle θ, or at π − θ, since the
/// condition is squared to stay polynomial (12.13).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Isocline {
    /// The direction.
    pub direction: [f64; 3],
    /// cos θ.
    pub cosine: f64,
}

/// `N·v(S)`: the envelope of a moving tool, where the normal is perpendicular to the surface's
/// velocity v(x) = w + ω × (x − c) (12.13).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Envelope {
    /// The linear velocity w.
    pub linear: [f64; 3],
    /// The angular velocity ω.
    pub angular: [f64; 3],
    /// The point c the rotation is about.
    pub about: [f64; 3],
}

/// `a × b`.
fn cross(a: &[Interval; 3], b: &[Interval; 3]) -> Result<[Interval; 3], NumError> {
    let [a0, a1, a2] = *a;
    let [b0, b1, b2] = *b;
    Ok([
        a1.mul(b2)?.sub(a2.mul(b1)?)?,
        a2.mul(b0)?.sub(a0.mul(b2)?)?,
        a0.mul(b1)?.sub(a1.mul(b0)?)?,
    ])
}

/// `a + b`.
fn plus(a: &[Interval; 3], b: &[Interval; 3]) -> Result<[Interval; 3], NumError> {
    let [a0, a1, a2] = *a;
    let [b0, b1, b2] = *b;
    Ok([a0.add(b0)?, a1.add(b1)?, a2.add(b2)?])
}

/// `a − p`.
fn less(a: &[Interval; 3], p: &[f64; 3]) -> Result<[Interval; 3], NumError> {
    let [a0, a1, a2] = *a;
    let [b0, b1, b2] = exact(p)?;
    Ok([a0.sub(b0)?, a1.sub(b1)?, a2.sub(b2)?])
}

/// The first partials of S: `S`, `S_u` and `S_v`.
fn first(partials: &[[Interval; 3]]) -> Result<[&[Interval; 3]; 3], IntersectError> {
    match partials {
        [s, su, sv, ..] => Ok([s, su, sv]),
        _ => Err(IntersectError::Internal),
    }
}

/// The normal `N = S_u × S_v` and its partials, by the product rule.
fn normal(partials: &[[Interval; 3]]) -> Result<[[Interval; 3]; 3], IntersectError> {
    let [_, su, sv, suu, suv, svv, ..] = partials else {
        return Err(IntersectError::Internal);
    };
    Ok([
        cross(su, sv)?,
        plus(&cross(suu, sv)?, &cross(su, suv)?)?,
        plus(&cross(suv, sv)?, &cross(su, svv)?)?,
    ])
}

impl Level for OnImplicit<'_> {
    fn order(&self) -> usize {
        1
    }

    fn sample(&self, partials: &[[Interval; 3]]) -> Result<Sample, IntersectError> {
        let [s, su, sv] = first(partials)?;
        let e = self.0.enclose_box(*s)?;
        Ok(Sample {
            value: e.value,
            du: dot(&e.gradient, su)?,
            dv: dot(&e.gradient, sv)?,
            smooth: e.smooth,
        })
    }
}

impl Level for Silhouette {
    fn order(&self) -> usize {
        2
    }

    fn sample(&self, partials: &[[Interval; 3]]) -> Result<Sample, IntersectError> {
        let [n, nu, nv] = normal(partials)?;
        let d = exact(&self.direction)?;
        Ok(Sample {
            value: dot(&n, &d)?,
            du: dot(&nu, &d)?,
            dv: dot(&nv, &d)?,
            smooth: true,
        })
    }
}

impl Level for Perspective {
    fn order(&self) -> usize {
        2
    }

    fn sample(&self, partials: &[[Interval; 3]]) -> Result<Sample, IntersectError> {
        let [s, ..] = first(partials)?;
        let [n, nu, nv] = normal(partials)?;
        let gap = less(s, &self.eye)?;
        // N is perpendicular to S_u and S_v, so the product rule's second term vanishes.
        Ok(Sample {
            value: dot(&n, &gap)?,
            du: dot(&nu, &gap)?,
            dv: dot(&nv, &gap)?,
            smooth: true,
        })
    }
}

impl Level for Isocline {
    fn order(&self) -> usize {
        2
    }

    fn sample(&self, partials: &[[Interval; 3]]) -> Result<Sample, IntersectError> {
        let [n, nu, nv] = normal(partials)?;
        let d = exact(&self.direction)?;
        let c = Interval::point(self.cosine)?.square()?.mul(dot(&d, &d)?)?;
        let along = dot(&n, &d)?;
        let rate = |nd: &[Interval; 3]| -> Result<Interval, NumError> {
            let two = Interval::point(2.0)?;
            two.mul(along.mul(dot(nd, &d)?)?.sub(c.mul(dot(&n, nd)?)?)?)
        };
        Ok(Sample {
            value: along.square()?.sub(c.mul(dot(&n, &n)?)?)?,
            du: rate(&nu)?,
            dv: rate(&nv)?,
            smooth: true,
        })
    }
}

impl Level for Envelope {
    fn order(&self) -> usize {
        2
    }

    fn sample(&self, partials: &[[Interval; 3]]) -> Result<Sample, IntersectError> {
        let [s, su, sv] = first(partials)?;
        let [n, nu, nv] = normal(partials)?;
        let omega = exact(&self.angular)?;
        let v = plus(
            &exact(&self.linear)?,
            &cross(&omega, &less(s, &self.about)?)?,
        )?;
        Ok(Sample {
            value: dot(&n, &v)?,
            du: dot(&nu, &v)?.add(dot(&n, &cross(&omega, su)?)?)?,
            dv: dot(&nv, &v)?.add(dot(&n, &cross(&omega, sv)?)?)?,
            smooth: true,
        })
    }
}

/// One branch of a level curve.
#[derive(Clone, Debug, PartialEq)]
pub struct Branch {
    /// Enclosures of points on the branch, in order along it, each holding one exact point. A
    /// closed branch does not repeat its first.
    pub vertices: Vec<[Interval; 2]>,
    /// Why it ends, at its first vertex and at its last.
    pub ends: [CurveEnd; 2],
    /// A bound, in space, on how far the exact branch is from the surface's image of the straight
    /// parameter chords through the vertices.
    pub deviation: f64,
    /// Where it passes a closing seam or a knot line, each point recorded once (12.10), in order
    /// along it.
    pub passes: Vec<Pass>,
}

/// A box of parameters where the trace could not decide.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlaggedBox {
    /// The box.
    pub at: [Interval; 2],
    /// Why.
    pub why: Unresolved,
}

/// The zero set of a function on a surface.
#[derive(Clone, Debug, PartialEq)]
pub struct LevelCurves {
    /// Its branches, in the order of their first vertices.
    pub branches: Vec<Branch>,
    /// What could not be decided.
    pub flagged: Vec<FlaggedBox>,
}

/// One read of the function over a box.
#[derive(Clone, Copy, Debug)]
struct Read {
    /// The function.
    sample: Sample,
    /// Upper bounds on |`S_u`| and |`S_v`|.
    speed: [f64; 2],
}

/// Reads the function, on one cell between breaks.
struct Reader<'a> {
    /// The surface.
    surface: &'a SurfaceKind,
    /// The function.
    level: &'a dyn Level,
    /// The cell.
    cell: Region,
}

/// The side a break at `at` is read from, inside `cell`.
fn from(at: f64, (_, hi): (f64, f64)) -> Limit {
    if at < hi {
        Limit::FromRight
    } else {
        Limit::FromLeft
    }
}

/// A parameter's span as an interval.
fn span((a, b): (f64, f64)) -> Result<Interval, NumError> {
    Interval::new(a, b)
}

/// `a` and `b` together: both hold one exact value, so their intersection does too (if rounding
/// ever made them miss, the first is kept).
fn meet(a: Interval, b: Interval) -> Result<Interval, NumError> {
    let (lo, hi) = (larger(a.lo(), b.lo()), smaller(a.hi(), b.hi()));
    if lo <= hi {
        Interval::new(lo, hi)
    } else {
        Ok(a)
    }
}

/// Whether an interval excludes zero.
fn signed(x: Interval) -> bool {
    positive(x) || negative(x)
}

/// The width of an interval.
fn width(x: Interval) -> f64 {
    x.hi() - x.lo()
}

/// Where to cut `(a, b)`: at the first share where `clear` holds, so no corner lands on the curve,
/// or failing that at the first share with room, for the floor to decide. `None` if the span has
/// no room.
fn place(
    (a, b): (f64, f64),
    mut clear: impl FnMut(f64) -> Result<bool, IntersectError>,
) -> Result<Option<f64>, IntersectError> {
    let mut first = None;
    for share in CUTS {
        let m = a + (b - a) * share;
        if a < m && m < b {
            if clear(m)? {
                return Ok(Some(m));
            }
            first = first.or(Some(m));
        }
    }
    Ok(first)
}

impl Reader<'_> {
    /// The function over `region`.
    fn read(&self, region: Region, meter: &mut WorkMeter<'_>) -> Result<Read, IntersectError> {
        meter.charge(STEP)?;
        let sides = (from(region.0.0, self.cell.0), from(region.1.0, self.cell.1));
        let order = self.level.order();
        let partials = self.surface.partials_over(region, sides, order, meter)?;
        let [_, su, sv] = first(&partials)?;
        Ok(Read {
            sample: self.level.sample(&partials)?,
            speed: [Interval::norm(su)?.hi(), Interval::norm(sv)?.hi()],
        })
    }

    /// g over `region`, tightened by the mean-value form around its middle.
    fn value(
        &self,
        region: Region,
        whole: &Read,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Interval, IntersectError> {
        let ((a, b), (c, d)) = region;
        let (m, n) = (a / 2.0 + b / 2.0, c / 2.0 + d / 2.0);
        let middle = self.read(((m, m), (n, n)), meter)?.sample.value;
        let u = span((a, b))?.sub(Interval::point(m)?)?;
        let v = span((c, d))?.sub(Interval::point(n)?)?;
        let s = whole.sample;
        let centred = middle.add(s.du.mul(u)?)?.add(s.dv.mul(v)?)?;
        Ok(meet(s.value, centred)?)
    }

    /// The sign of g at a point: `true` above zero, `false` below, `None` undecided.
    fn sign(
        &self,
        u: f64,
        v: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<bool>, IntersectError> {
        let g = self.read(((u, u), (v, v)), meter)?.sample.value;
        Ok(if positive(g) {
            Some(true)
        } else if negative(g) {
            Some(false)
        } else {
            None
        })
    }
}

/// A side of a box: parameter `axis` held at `at`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Line {
    /// 0 where u is held, 1 where v is.
    axis: usize,
    /// The value it is held at.
    at: f64,
}

impl Line {
    /// The box of `t` along the line.
    const fn region(self, t: (f64, f64)) -> Region {
        if self.axis == 0 {
            ((self.at, self.at), t)
        } else {
            (t, (self.at, self.at))
        }
    }

    /// The point `t` along the line.
    const fn point(self, t: f64) -> (f64, f64) {
        if self.axis == 0 {
            (self.at, t)
        } else {
            (t, self.at)
        }
    }

    /// How the line sorts, a double's bits being one exact key for one exact value.
    fn key(self) -> (usize, u64) {
        (self.axis, self.at.to_bits())
    }
}

/// g's slope along `line`, and the surface's speed along it.
const fn along(line: Line, read: &Read) -> (Interval, f64) {
    if line.axis == 0 {
        (read.sample.dv, read.speed[1])
    } else {
        (read.sample.du, read.speed[0])
    }
}

/// `line`'s stretch `t`, known to hold one simple root, narrowed to `floor` in space.
fn narrow(
    reader: &Reader<'_>,
    line: Line,
    (mut a, mut b): (f64, f64),
    low: bool,
    (floor, speed): (f64, f64),
    meter: &mut WorkMeter<'_>,
) -> Result<Interval, IntersectError> {
    loop {
        if (b - a) * speed <= floor {
            break;
        }
        let Some(m) = halfway((a, b)) else {
            break;
        };
        let at = |t: f64| line.point(t);
        let (u, v) = at(m);
        match reader.sign(u, v, meter)? {
            // g at the low end has sign `low`, and the root lies where it changes.
            Some(sign) if sign == low => a = m,
            Some(_) => b = m,
            None => {
                // The root is at the middle, to rounding: close in from both sides.
                let before = (a, b);
                if let Some(x) = halfway((a, m)) {
                    let (u, v) = at(x);
                    if reader.sign(u, v, meter)? == Some(low) {
                        a = x;
                    }
                }
                if let Some(x) = halfway((m, b)) {
                    let (u, v) = at(x);
                    if reader.sign(u, v, meter)? == Some(!low) {
                        b = x;
                    }
                }
                if (a, b) == before {
                    break;
                }
            }
        }
    }
    Ok(Interval::new(a, b)?)
}

/// The roots of g along `line` over `t`, in order, each simple; `None` if they cannot be told
/// apart above the floor.
fn roots(
    reader: &Reader<'_>,
    line: Line,
    t: (f64, f64),
    floor: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Interval>>, IntersectError> {
    let mut out = Vec::new();
    let mut work = alloc::vec![t];
    while let Some((a, b)) = work.pop() {
        let region = line.region((a, b));
        let whole = reader.read(region, meter)?;
        let (slope, speed) = along(line, &whole);
        if whole.sample.smooth {
            if signed(reader.value(region, &whole, meter)?) {
                continue;
            }
            if signed(slope) {
                let (x, y) = line.point(a);
                let (p, q) = line.point(b);
                match (reader.sign(x, y, meter)?, reader.sign(p, q, meter)?) {
                    (Some(low), Some(high)) if low == high => continue,
                    (Some(low), Some(_)) => {
                        out.push(narrow(reader, line, (a, b), low, (floor, speed), meter)?);
                        continue;
                    }
                    _ => {}
                }
            }
        } else if signed(whole.sample.value) {
            continue;
        }
        if (b - a) * speed <= floor {
            return Ok(None);
        }
        let cut = place((a, b), |m| {
            let (x, y) = line.point(m);
            Ok(reader.sign(x, y, meter)?.is_some())
        })?;
        let Some(m) = cut else {
            return Ok(None);
        };
        work.push((m, b));
        work.push((a, m));
    }
    out.sort_by(|x, y| x.lo().total_cmp(&y.lo()));
    Ok(Some(out))
}

/// Where a curve crosses a box's side.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Crossing {
    /// The side's line.
    line: Line,
    /// Whether the box lies above the line.
    above: bool,
    /// The point.
    at: [Interval; 2],
}

/// The crossings on `region`'s four sides, in order along `axis`, in pairs that join within the
/// box. `None` if they cannot be told apart or do not pair.
fn crossings(
    reader: &Reader<'_>,
    region: Region,
    axis: usize,
    floor: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Crossing>>, IntersectError> {
    let ((a, b), (c, d)) = region;
    let sides = [
        (Line { axis: 0, at: a }, true, (c, d)),
        (Line { axis: 0, at: b }, false, (c, d)),
        (Line { axis: 1, at: c }, true, (a, b)),
        (Line { axis: 1, at: d }, false, (a, b)),
    ];
    let mut out = Vec::new();
    for (line, above, t) in sides {
        let Some(found) = roots(reader, line, t, floor, meter)? else {
            return Ok(None);
        };
        for root in found {
            let held = Interval::point(line.at)?;
            let at = if line.axis == 0 {
                [held, root]
            } else {
                [root, held]
            };
            out.push(Crossing { line, above, at });
        }
    }
    let low = |c: &Crossing| c.at.get(axis).map_or(0.0, |i| i.lo());
    out.sort_by(|x, y| low(x).total_cmp(&low(y)));
    // Arcs lie over disjoint spans, so each pair must end before the next begins. A side's roots
    // are simple and the corners signed, so arcs come in pairs: an odd count stops the stitching.
    let apart = out.chunks(2).zip(out.chunks(2).skip(1)).all(|(x, y)| {
        let high = x
            .last()
            .and_then(|c| c.at.get(axis))
            .map_or(0.0, |i| i.hi());
        y.first().is_some_and(|c| {
            let touching = low(c) <= high;
            !touching
        })
    });
    Ok(apart.then_some(out))
}

/// A settled box.
#[derive(Clone, Debug)]
struct Leaf {
    /// Its crossings, in pairs.
    crossings: Vec<Crossing>,
    /// How far its arcs leave their chords, in space.
    sag: f64,
}

/// What became of a box.
enum Verdict {
    /// No curve in it.
    Empty,
    /// Its arcs are known.
    Settled(Leaf),
    /// Too wide to say.
    Split,
    /// Undecided at the floor.
    Flag(Unresolved),
}

/// The tests of 12.8 on one box.
fn decide(
    reader: &Reader<'_>,
    region: Region,
    (floor, ceiling): (f64, f64),
    meter: &mut WorkMeter<'_>,
) -> Result<(Verdict, Read), IntersectError> {
    let whole = reader.read(region, meter)?;
    let s = whole.sample;
    let ((a, b), (c, d)) = region;
    let [su, sv] = whole.speed;
    let small = larger((b - a) * su, (d - c) * sv) <= floor;
    let fall = |why| {
        if small {
            Verdict::Flag(why)
        } else {
            Verdict::Split
        }
    };
    if !s.smooth {
        let verdict = if signed(s.value) {
            Verdict::Empty
        } else {
            fall(Unresolved::NonSmooth {
                at: Degeneracy::Crease,
            })
        };
        return Ok((verdict, whole));
    }
    if signed(reader.value(region, &whole, meter)?) {
        return Ok((Verdict::Empty, whole));
    }
    let singular = Unresolved::UndecidedAtFloor {
        test: Test::Uniqueness,
    };
    if !(signed(s.du) || signed(s.dv)) {
        return Ok((fall(singular), whole));
    }
    // The curve is a graph over the parameter whose partial may vanish; of two, take the flatter.
    let steep = |over: Interval, by: Interval| over.neg().div(by).map_or(f64::INFINITY, width);
    let (over_u, over_v) = (steep(s.du, s.dv), steep(s.dv, s.du));
    let graph_over_u = over_u <= over_v;
    let (axis, sag) = if graph_over_u {
        (0, over_u * (b - a) / 4.0 * sv)
    } else {
        (1, over_v * (d - c) / 4.0 * su)
    };
    let verdict = if sag <= ceiling || small {
        match crossings(reader, region, axis, floor, meter)? {
            None => fall(singular),
            Some(crossings) => Verdict::Settled(Leaf { crossings, sag }),
        }
    } else {
        Verdict::Split
    };
    Ok((verdict, whole))
}

/// The cells of `domain` between the surface's breaks, in order.
fn cells(domain: Region, (us, vs): (&[f64], &[f64])) -> Result<Vec<Region>, IntersectError> {
    // `ParamSurface::breaks` lists only what is strictly inside the domain.
    let cuts = |(a, b): (f64, f64), inner: &[f64]| {
        let mut out = alloc::vec![a];
        out.extend(inner.iter().copied());
        out.push(b);
        out
    };
    let (u, v) = (cuts(domain.0, us), cuts(domain.1, vs));
    let mut out = Vec::new();
    for x in u.windows(2) {
        for y in v.windows(2) {
            let ([a, b], [c, d]) = (x, y) else {
                return Err(IntersectError::Internal);
            };
            out.push(((*a, *b), (*c, *d)));
        }
    }
    Ok(out)
}

/// One crossing, as the stitching reads it.
#[derive(Clone, Copy, Debug)]
struct Entry {
    /// The crossing.
    crossing: Crossing,
    /// The other end of its arc in its box.
    mate: usize,
    /// How far that arc leaves its chord.
    sag: f64,
    /// The crossing across its side, or why there is none.
    across: Across,
    /// Whether a branch has taken it.
    taken: bool,
}

/// What lies across a crossing's side.
#[derive(Clone, Copy, Debug)]
enum Across {
    /// Not worked out yet.
    Unknown,
    /// The crossing of the box across.
    Met(usize),
    /// The branch ends here.
    End(CurveEnd),
}

/// Whether two enclosures of a line's parameter overlap.
fn overlap(x: Interval, y: Interval) -> bool {
    x.lo() <= y.hi() && y.lo() <= x.hi()
}

/// Why a crossing with nothing across it ends, or `None` if it has no reason to.
fn ending(
    crossing: &Crossing,
    domain: Region,
    (us, vs): (&[f64], &[f64]),
    flagged: &[FlaggedBox],
) -> Option<CurveEnd> {
    let [u, v] = crossing.at;
    let touches = |b: &FlaggedBox| {
        let [x, y] = b.at;
        overlap(x, u) && overlap(y, v)
    };
    let at = crossing.line.at;
    let (ends, inner) = if crossing.line.axis == 0 {
        (domain.0, us)
    } else {
        (domain.1, vs)
    };
    if flagged.iter().any(touches) {
        Some(CurveEnd::Flagged)
    } else if at <= ends.0 || at >= ends.1 {
        Some(CurveEnd::Boundary)
    } else if inner.contains(&at) {
        Some(CurveEnd::Seam)
    } else {
        None
    }
}

/// The enclosure a crossing and the one across it give together.
fn vertex(entries: &[Entry], k: usize) -> Result<[Interval; 2], IntersectError> {
    let entry = entries.get(k).ok_or(IntersectError::Internal)?;
    let [u, v] = entry.crossing.at;
    if let Across::Met(j) = entry.across {
        let other = entries.get(j).ok_or(IntersectError::Internal)?;
        let [x, y] = other.crossing.at;
        return Ok([meet(u, x)?, meet(v, y)?]);
    }
    Ok([u, v])
}

/// Follows a branch from entry `start`, crossing box after box, until it ends or closes.
fn follow(
    entries: &mut [Entry],
    start: usize,
    lines: &Lines<'_>,
) -> Result<Branch, IntersectError> {
    let first_end = match entries.get(start).ok_or(IntersectError::Internal)?.across {
        Across::End(end) => end,
        Across::Met(_) | Across::Unknown => CurveEnd::Closed,
    };
    let mut vertices = alloc::vec![vertex(entries, start)?];
    let mut passes = Vec::new();
    let mut deviation = 0.0;
    let mut k = start;
    // Each round takes an entry for good, so a branch that runs out of them has lost its way.
    for _ in 0..entries.len() {
        let entry = entries.get_mut(k).ok_or(IntersectError::Internal)?;
        entry.taken = true;
        let (mate, sag) = (entry.mate, entry.sag);
        deviation = larger(deviation, sag);
        let other = entries.get_mut(mate).ok_or(IntersectError::Internal)?;
        other.taken = true;
        let (across, line) = (other.across, other.crossing.line);
        match across {
            Across::End(end) => {
                vertices.push(vertex(entries, mate)?);
                return Ok(Branch {
                    vertices,
                    ends: [first_end, end],
                    deviation,
                    passes,
                });
            }
            Across::Met(next) if next == start => {
                // The loop closes on its first vertex, which stands for this crossing too.
                if let Some(passing) = lines.passing(line) {
                    passes.insert(0, Pass { at: 0, passing });
                }
                return Ok(Branch {
                    vertices,
                    ends: [CurveEnd::Closed, CurveEnd::Closed],
                    deviation,
                    passes,
                });
            }
            Across::Met(next) => {
                vertices.push(vertex(entries, mate)?);
                if let Some(passing) = lines.passing(line) {
                    passes.push(Pass {
                        at: vertices.len().saturating_sub(1),
                        passing,
                    });
                }
                k = next;
            }
            Across::Unknown => return Err(IntersectError::Internal),
        }
    }
    Err(IntersectError::Internal)
}

/// A domain's lines as the stitching reads them (12.10).
struct Lines<'a> {
    domain: Region,
    seams: &'a Charts,
}

impl Lines<'_> {
    /// The key a line is stitched under: the two ends of a closing parameter are one line, so
    /// the far end is keyed as the near.
    fn key(&self, line: Line) -> (usize, u64) {
        let ((u0, u1), (v0, v1)) = self.domain;
        let [cu, cv] = self.seams.closes;
        match line.axis {
            0 if cu && line.at == u1 => Line { axis: 0, at: u0 }.key(),
            1 if cv && line.at == v1 => Line { axis: 1, at: v0 }.key(),
            _ => line.key(),
        }
    }

    /// What a branch passes where it crosses `line` into another box: a closing end, a knot line
    /// not proven tangent-continuous, or nothing to name.
    fn passing(&self, line: Line) -> Option<Passing> {
        let ((u0, u1), (v0, v1)) = self.domain;
        let [cu, cv] = self.seams.closes;
        let (closes, (lo, hi), creases) = if line.axis == 0 {
            (cu, (u0, u1), &self.seams.creases.0)
        } else {
            (cv, (v0, v1), &self.seams.creases.1)
        };
        if closes && (line.at == lo || line.at == hi) {
            Some(Passing::Seam)
        } else if creases.contains(&line.at) {
            Some(Passing::KnotLine)
        } else {
            None
        }
    }
}

/// Joins settled boxes across their shared sides into branches.
fn stitch(
    leaves: &[Leaf],
    flagged: &[FlaggedBox],
    domain: Region,
    breaks: (&[f64], &[f64]),
    seams: &Charts,
) -> Result<Vec<Branch>, IntersectError> {
    let read = Lines { domain, seams };
    let mut entries: Vec<Entry> = Vec::new();
    for leaf in leaves {
        for pair in leaf.crossings.chunks(2) {
            let [x, y] = pair else {
                return Err(IntersectError::Internal);
            };
            let k = entries.len();
            for (crossing, mate) in [(x, k.saturating_add(1)), (y, k)] {
                entries.push(Entry {
                    crossing: *crossing,
                    mate,
                    sag: leaf.sag,
                    across: Across::Unknown,
                    taken: false,
                });
            }
        }
    }
    // Each line's crossings, from the boxes below it and above it.
    let mut lines: BTreeMap<(usize, u64), [Vec<usize>; 2]> = BTreeMap::new();
    for (k, entry) in entries.iter().enumerate() {
        let [below, above] = lines.entry(read.key(entry.crossing.line)).or_default();
        if entry.crossing.above {
            above.push(k);
        } else {
            below.push(k);
        }
    }
    let along_line = |entries: &[Entry], k: usize| -> Result<Interval, IntersectError> {
        let c = entries.get(k).ok_or(IntersectError::Internal)?.crossing;
        c.at.get(usize::from(c.line.axis == 0))
            .copied()
            .ok_or(IntersectError::Internal)
    };
    for [below, above] in lines.values() {
        for (own, others) in [(below, above), (above, below)] {
            for &k in own {
                let x = along_line(&entries, k)?;
                let mut met = Vec::new();
                for &j in others {
                    if overlap(x, along_line(&entries, j)?) {
                        met.push(j);
                    }
                }
                let entry = entries.get(k).ok_or(IntersectError::Internal)?;
                let across = match met.as_slice() {
                    [j] => Across::Met(*j),
                    [] => Across::End(
                        ending(&entry.crossing, domain, breaks, flagged)
                            .ok_or(IntersectError::Internal)?,
                    ),
                    // Two roots a side could not part: 12.8's step 8 wants them accounted for.
                    _ => return Err(IntersectError::Internal),
                };
                entries.get_mut(k).ok_or(IntersectError::Internal)?.across = across;
            }
        }
    }
    let low = |e: &Entry| e.crossing.at.map(Interval::lo);
    let mut order: Vec<usize> = (0..entries.len()).collect();
    order.sort_by(|&x, &y| {
        let key = |k: usize| entries.get(k).map(low);
        key(x)
            .partial_cmp(&key(y))
            .unwrap_or(core::cmp::Ordering::Equal)
    });
    let mut branches = Vec::new();
    for ended in [true, false] {
        for &k in &order {
            let entry = entries.get(k).ok_or(IntersectError::Internal)?;
            if !entry.taken && matches!(entry.across, Across::End(_)) == ended {
                branches.push(follow(&mut entries, k, &read)?);
            }
        }
    }
    branches.sort_by(|x, y| {
        let key = |b: &Branch| b.vertices.first().map(|v| v.map(Interval::lo));
        key(x)
            .partial_cmp(&key(y))
            .unwrap_or(core::cmp::Ordering::Equal)
    });
    Ok(branches)
}

/// The zero set of `level` on `surface`, to a floor of `floor` in space and with every branch
/// within `ceiling` of its chords.
///
/// # Errors
///
/// [`IntersectError::InvalidInput`] for a floor or ceiling that is not positive;
/// [`IntersectError::BudgetExceeded`] and [`IntersectError::Cancelled`];
/// [`IntersectError::UnprovenMaths`] where a kind's bound needs one; [`IntersectError::Internal`]
/// if a crossing cannot be accounted for.
pub fn trace(
    surface: &SurfaceKind,
    level: &dyn Level,
    floor: f64,
    ceiling: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<LevelCurves, IntersectError> {
    if floor.is_nan() || floor <= 0.0 || ceiling.is_nan() || ceiling <= 0.0 {
        return Err(IntersectError::InvalidInput);
    }
    let domain = surface.domain();
    let (us, vs) = surface.breaks();
    let breaks = (us.as_slice(), vs.as_slice());
    let mut leaves = Vec::new();
    let mut flagged = Vec::new();
    for cell in cells(domain, breaks)? {
        let reader = Reader {
            surface,
            level,
            cell,
        };
        let mut work = alloc::vec![cell];
        while let Some(region) = work.pop() {
            let (verdict, read) = decide(&reader, region, (floor, ceiling), meter)?;
            match verdict {
                Verdict::Empty => {}
                Verdict::Settled(leaf) => leaves.push(leaf),
                Verdict::Flag(why) => flagged.push(FlaggedBox {
                    at: [span(region.0)?, span(region.1)?],
                    why,
                }),
                Verdict::Split => {
                    let ((a, b), (c, d)) = region;
                    let [su, sv] = read.speed;
                    let wide_u = (b - a) * su >= (d - c) * sv;
                    let cuts = if wide_u {
                        let clear = |m| {
                            let low = reader.sign(m, c, meter)?;
                            Ok(low.is_some() && reader.sign(m, d, meter)?.is_some())
                        };
                        place((a, b), clear)?.map(|m| (((m, b), (c, d)), ((a, m), (c, d))))
                    } else {
                        let clear = |m| {
                            let low = reader.sign(a, m, meter)?;
                            Ok(low.is_some() && reader.sign(b, m, meter)?.is_some())
                        };
                        place((c, d), clear)?.map(|m| (((a, b), (m, d)), ((a, b), (c, m))))
                    };
                    if let Some((later, sooner)) = cuts {
                        work.push(later);
                        work.push(sooner);
                    } else {
                        // No double between its sides, so nothing here can be told apart.
                        flagged.push(FlaggedBox {
                            at: [span(region.0)?, span(region.1)?],
                            why: Unresolved::UndecidedAtFloor {
                                test: Test::Uniqueness,
                            },
                        });
                    }
                }
            }
        }
    }
    let branches = stitch(&leaves, &flagged, domain, breaks, &charts(surface))?;
    Ok(LevelCurves { branches, flagged })
}
