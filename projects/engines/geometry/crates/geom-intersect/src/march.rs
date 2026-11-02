//! General surface with surface, second part: certified marching between the start points of
//! [`crate::general::subdivide`] (12.8, steps 6 to 8).
//!
//! What it does: [`march`] traces each branch through each settled pair, from one start point to
//! another, then joins the pieces across the pairs' shared boundaries into whole branches. In a
//! settled pair one coordinate of space, c, moves one way along every branch, so c parameterises
//! it: the unknowns are the four surface parameters, and the equations are S₁ − S₂ = 0 and
//! S₁[k] − c = 0.
//!
//! Guarantees:
//!
//! - **A step cannot jump to another branch.** Each step is one Krawczyk test over a box and a whole
//!   interval of c at once: passing proves that for every c in it the box holds exactly one point,
//!   so the branch through the step's start is the only one in the box.
//! - **Each branch ends for a reason.** It ends at a start point the box proves it reaches, or, at
//!   a domain's edge, where no box can stand across the point, within the floor of one, and a march
//!   leaves such a point a floor along: nothing there can be told apart on the grid. That point lies on the domain's boundary, or joins the
//!   next pair, or sits beside a flagged pair.
//! - **Everything is accounted for.** Every start point of a traced pair is used exactly once. A
//!   start that reaches no other, in either direction, leaves its whole pair in
//!   [`Traced::unproven`], with none of its pieces kept: never a branch quietly ended.
//! - **Each branch has a bound.** [`Branch::deviation`] bounds, in space, how far the exact branch
//!   is from the chords through its vertices. A step is kept only within the ceiling.
//! - **Deterministic.** Pairs in the order subdivision left them, starts in the order found, and a
//!   step size that follows only from what the tests said.
//! - **Joined across a closing seam (G173).** Where a domain's two ends are one curve
//!   ([`crate::seam`]), a branch that reaches one end goes on from the other: the point is kept
//!   once, and [`Branch::passes`] says where.
//!
//! Method: the predictor along the tangent n₁ × n₂ and the corrector's four-unknown system are
//! Barnhill and Kersey's (<https://doi.org/10.1016/0167-8396(90)90035-P>), with the proof of each
//! step after Kearfott and Xing (<https://doi.org/10.1137/0731048>): Krawczyk's operator, chapter
//! 12.3, with the parameter an interval. An arc over a span w of c whose slopes dx/dc lie in an
//! interval of width m leaves its chord by at most m·w/4. That slope comes from the tangent's own
//! enclosure over the step's box, which is a bound of the same strength as 12.8's h²κ/8 and needs no
//! curvature formula.
//!
//! What this piece leaves: a branch into a flagged pair ends there (G171). A procedural curve from a
//! branch is G175.
//!
//! Cost: each step is two reads of both surfaces and one 4 × 4 inverse. Steps along a branch of
//! length L number about L·√(κ/ceiling), and stitching is quadratic in the start points of a line
//! of pairs. All of it is charged to the meter. Recursion: none.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are chapter 12.8's: S the surfaces, c the coordinate, k its axis, m a middle"
)]

use alloc::vec::Vec;

use geom_curve::tight::{larger, smaller};
use geom_num::NumError;
use geom_num::interval::Interval;
use geom_num::newton::inverse;
use geom_run::WorkMeter;
use geom_surface::{ParamBox as Region, ParamSurface, SurfaceKind};

use crate::IntersectError;
use crate::general::{
    Settled, Subdivided, cross, ends, may_be_parallel, normals, partials, point, speeds,
};
use crate::project::{negative, positive};
use crate::result::CurveEnd;
use crate::seam::{Charts, Meet, Pass, Passing, at_pole, charts, meet};

/// What a step costs the meter, besides the kinds' own charges.
const STEP: u64 = 1;

/// How much a step's box is widened around what the predictor says, as a share of its reach. Any
/// margin past the corrector's own travel would do.
const ROOM: f64 = 0.5;

/// The smallest share of a pair's span of c a step may shrink to before the pair is flagged.
const SMALLEST: f64 = 1.0 / 1_048_576.0;

/// Corrector steps to a point on a branch. Newton's method doubles its digits, so this is past what
/// a double holds from any start a step could give it.
const NEWTON: usize = 8;

/// One branch of an intersection.
#[derive(Clone, Debug, PartialEq)]
pub struct Branch {
    /// Enclosures of points on the branch, (u₁, v₁, u₂, v₂), in order along it. A closed branch
    /// does not repeat its first.
    pub vertices: Vec<[Interval; 4]>,
    /// Why it ends, at its first vertex and at its last.
    pub ends: [CurveEnd; 2],
    /// A bound, in space, on how far the exact branch is from the chords through its vertices.
    pub deviation: f64,
    /// Where it passes a closing seam, each point recorded once (12.10), in order along it.
    pub passes: Vec<Pass>,
}

/// What [`march`] leaves.
#[derive(Clone, Debug, PartialEq)]
pub struct Traced {
    /// The branches, in the order of their first vertices.
    pub branches: Vec<Branch>,
    /// Settled pairs a step could not be proven in.
    pub unproven: Vec<Settled>,
}

/// The middle of an interval.
fn mid(x: Interval) -> f64 {
    x.lo() / 2.0 + x.hi() / 2.0
}

/// Which coordinate of the tangent keeps its sign over a pair, and that sign.
fn axis_of(
    first: &SurfaceKind,
    second: &SurfaceKind,
    pair: &Settled,
    meter: &mut WorkMeter<'_>,
) -> Result<usize, IntersectError> {
    let a = normals(first, pair.first, meter)?;
    let b = normals(second, pair.second, meter)?;
    let tangent = cross(&a, &b)?;
    if may_be_parallel(&a, &b)? {
        return Err(IntersectError::Internal);
    }
    tangent
        .iter()
        .position(|&x| positive(x) || negative(x))
        .ok_or(IntersectError::Internal)
}

/// Whether `region` lies in `surface`'s domain, the only place a kind can be read.
fn within(surface: &SurfaceKind, region: Region) -> bool {
    let ((a, b), (c, d)) = surface.domain();
    let ((p, q), (r, t)) = region;
    a <= p && q <= b && c <= r && t <= d
}

/// The system in one settled pair.
struct System<'a> {
    first: &'a SurfaceKind,
    second: &'a SurfaceKind,
    /// The coordinate of space that moves one way along every branch here.
    axis: usize,
    /// Each parameter's speed, to turn a length in space into a width of that parameter.
    speeds: [f64; 4],
    /// What each surface's data proves of its seams and poles (12.10).
    seams: [&'a Charts; 2],
}

/// The surfaces read over a box of unknowns: each point and its partials.
struct Read {
    s: [Interval; 3],
    su: [Interval; 3],
    sv: [Interval; 3],
    t: [Interval; 3],
    tu: [Interval; 3],
    tv: [Interval; 3],
}

impl System<'_> {
    /// Reads both surfaces over `x`, or `None` where they cannot be bounded.
    fn read(
        &self,
        x: &[Interval; 4],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<Read>, IntersectError> {
        let [u1, v1, u2, v2] = *x;
        let (one, two) = ((ends(u1), ends(v1)), (ends(u2), ends(v2)));
        if !(within(self.first, one) && within(self.second, two)) {
            return Ok(None);
        }
        let a = partials(self.first, one, 1, meter)?;
        let b = partials(self.second, two, 1, meter)?;
        let (Some(a), Some(b)) = (a, b) else {
            return Ok(None);
        };
        let ([s, su, sv, ..], [t, tu, tv, ..]) = (a.as_slice(), b.as_slice()) else {
            return Err(IntersectError::Internal);
        };
        Ok(Some(Read {
            s: *s,
            su: *su,
            sv: *sv,
            t: *t,
            tu: *tu,
            tv: *tv,
        }))
    }

    /// The Jacobian of the system, row by row, over what `r` read.
    fn jacobian(&self, r: &Read) -> Result<[[Interval; 4]; 4], IntersectError> {
        let row = |k: usize| -> Result<[Interval; 4], IntersectError> {
            let at = |x: &[Interval; 3]| x.get(k).copied().ok_or(IntersectError::Internal);
            Ok([at(&r.su)?, at(&r.sv)?, at(&r.tu)?.neg(), at(&r.tv)?.neg()])
        };
        let zero = point(0.0)?;
        let last = {
            let at = |x: &[Interval; 3]| x.get(self.axis).copied().ok_or(IntersectError::Internal);
            [at(&r.su)?, at(&r.sv)?, zero, zero]
        };
        Ok([row(0)?, row(1)?, row(2)?, last])
    }

    /// The residuals over what `r` read, for c in `c`.
    fn residuals(&self, r: &Read, c: Interval) -> Result<[Interval; 4], IntersectError> {
        let [s0, s1, s2] = r.s;
        let [t0, t1, t2] = r.t;
        let height =
            r.s.get(self.axis)
                .copied()
                .ok_or(IntersectError::Internal)?;
        Ok([s0.sub(t0)?, s1.sub(t1)?, s2.sub(t2)?, height.sub(c)?])
    }

    /// The tangent's slopes dx/dc over what `r` read: how far apart they lie, across all three
    /// coordinates of space, or `None` if the tangent's own coordinate can vanish there.
    fn spread(&self, r: &Read) -> Result<Option<f64>, IntersectError> {
        let tangent = cross(&cross(&r.su, &r.sv)?, &cross(&r.tu, &r.tv)?)?;
        let along = *tangent.get(self.axis).ok_or(IntersectError::Internal)?;
        if !(positive(along) || negative(along)) {
            return Ok(None);
        }
        let mut most: f64 = 0.0;
        for x in tangent {
            let slope = x.div(along)?;
            most = larger(most, slope.hi() - slope.lo());
        }
        Ok(Some(most))
    }

    /// A point on the branch at `c` from `x`, by Newton's method, or `None` if it does not settle.
    fn correct(
        &self,
        mut x: [f64; 4],
        c: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[f64; 4]>, IntersectError> {
        let target = point(c)?;
        for _ in 0..NEWTON {
            let at = x.map(point);
            let [a, b, e, d] = at;
            let Some(r) = self.read(&[a?, b?, e?, d?], meter)? else {
                return Ok(None);
            };
            let f = self.residuals(&r, target)?.map(mid);
            let j = self.jacobian(&r)?.map(|row| row.map(mid));
            let Some(y) = inverse(j) else {
                return Ok(None);
            };
            for (out, row) in x.iter_mut().zip(&y) {
                let step = row.iter().zip(&f).fold(0.0, |sum, (m, v)| sum + m * v);
                *out -= step;
            }
        }
        Ok(Some(x))
    }

    /// dx/dc at `x`: the unknowns' rates along the branch.
    fn rates(
        &self,
        x: &[f64; 4],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[f64; 4]>, IntersectError> {
        let [a, b, e, d] = x.map(point);
        let Some(r) = self.read(&[a?, b?, e?, d?], meter)? else {
            return Ok(None);
        };
        let Some(y) = inverse(self.jacobian(&r)?.map(|row| row.map(mid))) else {
            return Ok(None);
        };
        Ok(Some(y.map(|row| row.last().copied().unwrap_or(0.0))))
    }

    /// Krawczyk's test on `b` for every c in `c`: its image if strictly inside `b`, else `None`.
    fn prove(
        &self,
        b: &[Interval; 4],
        c: Interval,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[Interval; 4]>, IntersectError> {
        let m = b.map(mid);
        let [p, q, e, d] = m.map(point);
        let Some(here) = self.read(&[p?, q?, e?, d?], meter)? else {
            return Ok(None);
        };
        let Some(over) = self.read(b, meter)? else {
            return Ok(None);
        };
        let Some(y) = inverse(self.jacobian(&here)?.map(|row| row.map(mid))) else {
            return Ok(None);
        };
        let f = self.residuals(&here, c)?;
        let j = self.jacobian(&over)?;
        let mut out = [point(0.0)?; 4];
        for (i, ((slot, row), mi)) in out.iter_mut().zip(&y).zip(&m).enumerate() {
            let mut image = point(*mi)?;
            for (yk, fk) in row.iter().zip(&f) {
                image = image.sub(point(*yk)?.mul(*fk)?)?;
            }
            for (col, (bk, mk)) in b.iter().zip(&m).enumerate() {
                // (I − Y·J) at (i, col), by the box's distance from its middle.
                let mut entry = point(if i == col { 1.0 } else { 0.0 })?;
                for (yk, jk) in row.iter().zip(&j) {
                    let cell = jk.get(col).copied().ok_or(IntersectError::Internal)?;
                    entry = entry.sub(point(*yk)?.mul(cell)?)?;
                }
                image = image.add(entry.mul(bk.sub(point(*mk)?)?)?)?;
            }
            *slot = image;
        }
        let inside = out
            .iter()
            .zip(b)
            .all(|(k, x)| x.lo() < k.lo() && k.hi() < x.hi());
        Ok(inside.then_some(out))
    }
}

/// A box of unknowns around the path from `x` along `rates` for a step of `h`, with `pad` of space
/// to spare: a length that is a wide box in a surface's parameter and a thin one in a plane's unless
/// each parameter takes it at its own `speeds`.
fn around_step(
    x: &[f64; 4],
    rates: &[f64; 4],
    (h, pad): (f64, f64),
    speeds: &[f64; 4],
) -> Result<[Interval; 4], NumError> {
    let mut out = [point(0.0)?; 4];
    for (((slot, a), r), speed) in out.iter_mut().zip(x).zip(rates).zip(speeds) {
        let b = a + r * h;
        let reach = (b - a).abs() * ROOM + pad / speed;
        *slot = Interval::new(smaller(*a, b) - reach, larger(*a, b) + reach)?;
    }
    Ok(out)
}

/// One piece of a branch: a march through one pair from one start point to another.
struct Piece {
    /// The pair's index and the two starts' indices in it.
    from: (usize, usize),
    /// Where it ends.
    to: usize,
    /// Its vertices, both ends included.
    vertices: Vec<[Interval; 4]>,
    /// Its bound.
    deviation: f64,
}

/// The c of a start point.
fn height(
    system: &System<'_>,
    s: &[Interval; 4],
    meter: &mut WorkMeter<'_>,
) -> Result<Option<f64>, IntersectError> {
    match system.read(s, meter)? {
        Some(r) => {
            let at = r.s.get(system.axis).copied();
            Ok(at.map(mid))
        }
        None => Ok(None),
    }
}

/// A start point not yet used that `here` stands within twice the floor of, in space: a start's
/// enclosure reaches a floor by itself.
fn beside(
    system: &System<'_>,
    here: &[Interval; 4],
    pair: &Settled,
    used: &[bool],
    floor: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<usize>, IntersectError> {
    let Some(at) = system.read(here, meter)? else {
        return Ok(None);
    };
    let place = at.s.map(mid);
    // Of those near enough in space, the nearest in parameters, each at its speed: starts at one
    // pole are one point in space, and differ only in the direction they leave it by (12.10).
    let mut best: Option<(usize, f64)> = None;
    for (k, s) in pair.starts.iter().enumerate() {
        if used.get(k).copied().unwrap_or(true) {
            continue;
        }
        let Some(there) = system.read(s, meter)? else {
            continue;
        };
        let near = place
            .iter()
            .zip(there.s.map(mid))
            .all(|(a, b)| (a - b).abs() <= 2.0 * floor);
        if near {
            let apart = here
                .iter()
                .zip(s)
                .zip(system.speeds)
                .fold(0.0, |most, ((x, y), speed)| {
                    larger(most, (mid(*x) - mid(*y)).abs() * speed)
                });
            if best.is_none_or(|(_, b)| apart < b) {
                best = Some((k, apart));
            }
        }
    }
    Ok(best.map(|(k, _)| k))
}

/// How one march through a pair ended.
enum Walk {
    /// It reached a start point.
    Reached(usize, Vec<[Interval; 4]>, f64),
    /// A step could not be proven above the floor.
    Unproven,
}

/// Where the corrector begins from `start`: its middle, but a floor off any pole line it is held
/// on, into the pair's patch. On the line itself the chart has no tangent plane (a partial
/// vanishes there), so Newton's step could not be taken (12.10).
fn off_poles(system: &System<'_>, pair: &Settled, start: &[Interval; 4], floor: f64) -> [f64; 4] {
    let mut x = start.map(mid);
    let [one, two] = system.seams;
    let spans = [pair.first.0, pair.first.1, pair.second.0, pair.second.1];
    let lines = [&one.poles.0, &one.poles.1, &two.poles.0, &two.poles.1];
    for (((slot, held), (lo, hi)), (lines, speed)) in x
        .iter_mut()
        .zip(start)
        .zip(spans)
        .zip(lines.into_iter().zip(system.speeds))
    {
        if held.lo() == held.hi() && lines.contains(&held.lo()) {
            let step = floor / speed;
            *slot = if held.lo() <= lo {
                smaller(lo + step, hi)
            } else {
                larger(hi - step, lo)
            };
        }
    }
    x
}

/// Marches from start `from` of `pair`, along c in `way`, to another of its start points.
fn walk(
    system: &System<'_>,
    pair: &Settled,
    from: usize,
    way: f64,
    (floor, ceiling): (f64, f64),
    meter: &mut WorkMeter<'_>,
) -> Result<Walk, IntersectError> {
    let first = *pair.starts.get(from).ok_or(IntersectError::Internal)?;
    let Some(c0) = height(system, &first, meter)? else {
        return Ok(Walk::Unproven);
    };
    // A start on a domain's edge has no box around it, so the march begins a floor along.
    let mut c = c0 + way * floor;
    let Some(mut x) = system.correct(off_poles(system, pair, &first, floor), c, meter)? else {
        return Ok(Walk::Unproven);
    };
    let mut heights = Vec::with_capacity(pair.starts.len());
    for s in &pair.starts {
        heights.push(height(system, s, meter)?);
    }
    let span_c = heights
        .iter()
        .flatten()
        .fold(0.0, |most, y: &f64| larger(most, (y - c0).abs()));
    if span_c <= 0.0 {
        return Ok(Walk::Unproven);
    }
    let smallest = span_c * SMALLEST;
    let mut h = span_c / 8.0;
    let mut vertices = alloc::vec![first];
    let mut deviation: f64 = 0.0;
    let mut used = alloc::vec![false; pair.starts.len()];
    if let Some(mark) = used.get_mut(from) {
        *mark = true;
    }
    // The other starts at the pole this one is on are the same point: leaving it, none of them
    // is a place to reach (12.10).
    let domains = [system.first.domain(), system.second.domain()];
    for (mark, s) in used.iter_mut().zip(&pair.starts) {
        if meet(&first, s, domains, system.seams) == Some(Meet::Pole) {
            *mark = true;
        }
    }
    loop {
        meter.charge(STEP)?;
        let Some(rates) = system.rates(&x, meter)? else {
            return Ok(Walk::Unproven);
        };
        let signed = rates.map(|r| r * way);
        let pad = floor / 1024.0 + h * SMALLEST;
        let b = around_step(&x, &signed, (h, pad), &system.speeds)?;
        let over_c = Interval::new(smaller(c, c + way * h), larger(c, c + way * h))?;
        let proven = system.prove(&b, over_c, meter)?;
        let read = system.read(&b, meter)?;
        let sag = match &read {
            Some(r) => system.spread(r)?.map(|spread| spread * h / 4.0),
            None => None,
        };
        let good = proven.is_some() && sag.is_some_and(|s| s <= ceiling);
        if !good {
            h /= 2.0;
            if h < smallest {
                return Ok(Walk::Unproven);
            }
            continue;
        }
        deviation = larger(deviation, sag.unwrap_or(0.0));
        // The box holds one point at each c it covers, so a start at one of them, if it is where
        // the branch stands at that c, is on this branch.
        let mut reached = None;
        for (k, near) in heights.iter().enumerate() {
            let free = !used.get(k).copied().unwrap_or(true);
            if let (true, Some(near)) = (free, near.filter(|y| over_c.contains(*y)))
                && let Some(there) = system.correct(x, near, meter)?
            {
                let [p, q, e, d] = there.map(point);
                if beside(system, &[p?, q?, e?, d?], pair, &used, floor, meter)? == Some(k) {
                    reached = Some(k);
                    break;
                }
            }
        }
        if let Some(k) = reached {
            let s = *pair.starts.get(k).ok_or(IntersectError::Internal)?;
            vertices.push(s);
            return Ok(Walk::Reached(k, vertices, deviation));
        }
        let next = c + way * h;
        let Some(moved) = system.correct(signed_to(&x, &signed, h), next, meter)? else {
            return Ok(Walk::Unproven);
        };
        x = moved;
        c = next;
        let [p, q, e, d] = x.map(point);
        let here = [p?, q?, e?, d?];
        if let Some(k) = beside(system, &here, pair, &used, floor, meter)? {
            let s = *pair.starts.get(k).ok_or(IntersectError::Internal)?;
            vertices.push(s);
            return Ok(Walk::Reached(k, vertices, deviation));
        }
        vertices.push(here);
        // Past every start the pair holds, there is nothing left to reach.
        let beyond = heights
            .iter()
            .flatten()
            .all(|y| (y - c0).abs() + 2.0 * h < (c - c0).abs());
        if beyond {
            return Err(IntersectError::Internal);
        }
        h *= 1.5;
    }
}

/// `x` moved along `rates` for a step of `h`.
fn signed_to(x: &[f64; 4], rates: &[f64; 4], h: f64) -> [f64; 4] {
    let mut out = *x;
    for (slot, r) in out.iter_mut().zip(rates) {
        *slot += r * h;
    }
    out
}

/// A domain as four numbers.
type Flat = (f64, f64, f64, f64);

/// Why a piece's end has no piece beyond it.
fn ending(s: &[Interval; 4], (first, second): (Flat, Flat)) -> CurveEnd {
    let [u1, v1, u2, v2] = *s;
    let on = |x: Interval, a: f64, b: f64| x.contains(a) || x.contains(b);
    if on(u1, first.0, first.1)
        || on(v1, first.2, first.3)
        || on(u2, second.0, second.1)
        || on(v2, second.2, second.3)
    {
        CurveEnd::Boundary
    } else {
        CurveEnd::Flagged
    }
}

/// `region` as four numbers.
const fn flat(((a, b), (c, d)): Region) -> Flat {
    (a, b, c, d)
}

/// What a branch passes where two of its pieces meet so.
const fn passing(how: Meet) -> Option<Passing> {
    match how {
        Meet::Same => None,
        Meet::Seam => Some(Passing::Seam),
        Meet::Pole => Some(Passing::Pole),
    }
}

/// A piece's two ends: the start it leaves from and the one it reaches.
fn ends_of(
    pairs: &[Settled],
    piece: &Piece,
) -> Result<([Interval; 4], [Interval; 4]), IntersectError> {
    let pair = pairs.get(piece.from.0).ok_or(IntersectError::Internal)?;
    let at = |k: usize| pair.starts.get(k).copied().ok_or(IntersectError::Internal);
    Ok((at(piece.from.1)?, at(piece.to)?))
}

/// How two points meet, given the two surfaces' domains and charts.
type Meets<'a> = &'a dyn Fn(&[Interval; 4], &[Interval; 4]) -> Option<Meet>;

/// Where a branch goes on: the next piece's index, its far end, whether it runs forward from the
/// near one, and how the two pieces meet.
type Onward = (usize, [Interval; 4], bool, Meet);

/// The piece other than `k`, not yet taken, with an end that meets `tail`, and how it goes on.
fn next_piece(
    pieces: &[Piece],
    pairs: &[Settled],
    (taken, k): (&[bool], usize),
    tail: &[Interval; 4],
    one: Meets<'_>,
) -> Result<Option<Onward>, IntersectError> {
    for (j, other) in pieces.iter().enumerate() {
        if j == k || taken.get(j).copied().unwrap_or(true) {
            continue;
        }
        let (p, q) = ends_of(pairs, other)?;
        if let Some(how) = one(tail, &p) {
            return Ok(Some((j, q, true, how)));
        }
        if let Some(how) = one(tail, &q) {
            return Ok(Some((j, p, false, how)));
        }
    }
    Ok(None)
}

/// Follows pieces into branches, starting from those with an end, then the closed ones.
fn join(
    pieces: &[Piece],
    pairs: &[Settled],
    domains: (Region, Region),
    seams: [&Charts; 2],
) -> Result<Vec<Branch>, IntersectError> {
    let flats = (flat(domains.0), flat(domains.1));
    let both = [domains.0, domains.1];
    // Ends meeting at a pole: two make one branch through it, any other number a singular point
    // that each of them ends at (12.8, step 7; 12.10).
    let meeting = |x: &[Interval; 4]| -> usize {
        pieces
            .iter()
            .filter_map(|p| ends_of(pairs, p).ok())
            .flat_map(|(a, b)| [a, b])
            .filter(|e| meet(x, e, both, seams).is_some())
            .count()
    };
    let one = |a: &[Interval; 4], b: &[Interval; 4]| match meet(a, b, both, seams) {
        // At a pole, whether the two are held there with the same direction or not: any number
        // of ends but two make it a singular point, which joins none of them.
        Some(_) if at_pole(a, seams) && meeting(a) != 2 => None,
        how => how,
    };
    let mut taken = alloc::vec![false; pieces.len()];
    let none = alloc::vec![false; pieces.len()];
    let mut branches: Vec<Branch> = Vec::new();
    for only_open in [true, false] {
        for (index, piece) in pieces.iter().enumerate() {
            if taken.get(index).copied().unwrap_or(true) {
                continue;
            }
            // Each end is open when no other piece, taken or not, meets it.
            let (a, b) = ends_of(pairs, piece)?;
            let open_a = next_piece(pieces, pairs, (&none, index), &a, &one)?.is_none();
            let open_b = next_piece(pieces, pairs, (&none, index), &b, &one)?.is_none();
            if only_open && !(open_a || open_b) {
                continue;
            }
            // From the open end, or from the first end of a closed one.
            let reversed = !open_a && open_b;
            let mut vertices: Vec<[Interval; 4]> = Vec::new();
            let mut passes: Vec<Pass> = Vec::new();
            let mut deviation: f64 = 0.0;
            let mut k = index;
            let mut tail = if reversed { a } else { b };
            let start = if reversed { b } else { a };
            let mut along = !reversed;
            loop {
                let part = pieces.get(k).ok_or(IntersectError::Internal)?;
                *taken.get_mut(k).ok_or(IntersectError::Internal)? = true;
                deviation = larger(deviation, part.deviation);
                let mut run = part.vertices.clone();
                if !along {
                    run.reverse();
                }
                if !vertices.is_empty() {
                    run.remove(0);
                }
                vertices.extend(run);
                let Some((j, end, forward, how)) =
                    next_piece(pieces, pairs, (&taken, k), &tail, &one)?
                else {
                    break;
                };
                // The point is kept as the part before it reads it; the next part's own
                // enclosure of it is dropped with its first vertex.
                if let Some(passing) = passing(how) {
                    passes.push(Pass {
                        at: vertices.len().saturating_sub(1),
                        passing,
                    });
                }
                k = j;
                tail = end;
                along = forward;
            }
            let closing = one(&tail, &start).filter(|_| vertices.len() > 2);
            if closing.is_some() {
                vertices.pop();
            }
            if let Some(passing) = closing.and_then(passing) {
                passes.insert(0, Pass { at: 0, passing });
            }
            let end = |x: &[Interval; 4]| {
                if at_pole(x, seams) {
                    CurveEnd::Singular
                } else {
                    ending(x, flats)
                }
            };
            branches.push(Branch {
                ends: if closing.is_some() {
                    [CurveEnd::Closed, CurveEnd::Closed]
                } else {
                    [end(&start), end(&tail)]
                },
                vertices,
                deviation,
                passes,
            });
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

/// Traces every branch of `found`, to a floor of `floor` in space and within `ceiling` of its
/// chords.
///
/// # Errors
///
/// [`IntersectError::InvalidInput`] for a floor or ceiling that is not positive;
/// [`IntersectError::BudgetExceeded`] and [`IntersectError::Cancelled`];
/// [`IntersectError::UnprovenMaths`] where a kind's bound needs one.
pub fn march(
    first: &SurfaceKind,
    second: &SurfaceKind,
    found: &Subdivided,
    floor: f64,
    ceiling: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Traced, IntersectError> {
    if floor.is_nan() || floor <= 0.0 || ceiling.is_nan() || ceiling <= 0.0 {
        return Err(IntersectError::InvalidInput);
    }
    let mut pieces: Vec<Piece> = Vec::new();
    let mut unproven = Vec::new();
    let ([a, b], [c, d]) = (
        speeds(first, first.domain(), meter)?,
        speeds(second, second.domain(), meter)?,
    );
    let seams = (charts(first), charts(second));
    for (index, pair) in found.settled.iter().enumerate() {
        let system = System {
            first,
            second,
            axis: axis_of(first, second, pair, meter)?,
            speeds: [a, b, c, d],
            seams: [&seams.0, &seams.1],
        };
        let mut done = alloc::vec![false; pair.starts.len()];
        let mut bad = false;
        for from in 0..pair.starts.len() {
            if done.get(from).copied().unwrap_or(true) {
                continue;
            }
            let mut piece = None;
            for way in [1.0, -1.0] {
                match walk(&system, pair, from, way, (floor, ceiling), meter) {
                    Ok(Walk::Reached(to, vertices, deviation))
                        if !done.get(to).copied().unwrap_or(true) =>
                    {
                        piece = Some(Piece {
                            from: (index, from),
                            to,
                            vertices,
                            deviation,
                        });
                        break;
                    }
                    Ok(_) | Err(IntersectError::Internal) => {}
                    Err(e) => return Err(e),
                }
            }
            let Some(piece) = piece else {
                bad = true;
                break;
            };
            *done.get_mut(from).ok_or(IntersectError::Internal)? = true;
            *done.get_mut(piece.to).ok_or(IntersectError::Internal)? = true;
            pieces.push(piece);
        }
        if bad {
            pieces.retain(|p| p.from.0 != index);
            unproven.push(pair.clone());
        }
    }
    let branches = join(
        &pieces,
        &found.settled,
        (first.domain(), second.domain()),
        [&seams.0, &seams.1],
    )?;
    Ok(Traced { branches, unproven })
}
