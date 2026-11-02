//! Fuzz target: `geom-intersect`'s subdivision leaves no crossing outside its settled or flagged
//! pairs, its marching stays on both surfaces, and its analysis of the flagged pairs says nothing
//! the surfaces deny, on any bytes.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: one of four surfaces on small
//! lattice sizes, some over a whole turn and some cones with an apex (G173's seams and poles), and
//! a sphere or a plane through it. [`check`] subdivides the pair and holds the
//! answer to the laws below, with a dense grid of the implicit form of the second surface on the
//! first as the oracle: a sign change between neighbours proves the surfaces cross there, and
//! [`crate::general`]'s pairs must cover it. [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes; a unit test builds
//!   the input that reaches it.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case: a crossing
//!   no pair covers, a start point or a branch's vertex off a surface, a settled pair with an odd
//!   count, a chord past its ceiling, a pass named at a vertex off its seam or pole, two branch
//!   ends left apart across a closing seam, a contact whose point is on one surface only, a bound
//!   past its ceiling, a region off its surface, an internal error, or a budget that changes an
//!   answer instead of stopping it.
//!
//! Method: chapter 12.8's subdivision
//! (`projects/engines/geometry/architecture/12-intersections.md`) with chapter 28.5's oracle stack
//! (`projects/engines/geometry/architecture/28-testing.md`). The grid proves a crossing, so the law
//! cannot be broken by a true answer.
//!
//! Cost: one subdivision, one budgeted rerun, one march and one analysis, and [`CELLS`] squared
//! evaluations.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are chapter 12.8's: S the surfaces, u and v their parameters"
)]

use geom_curve::Frame;
use geom_intersect::IntersectError;
use geom_intersect::contact::{Reading, analyse};
use geom_intersect::general::{Subdivided, subdivide};
use geom_intersect::march::{Traced, march};
use geom_intersect::result::{Coincidence, CurveContact, CurveEnd, PointContact};
use geom_intersect::seam::{Charts, Passing, charts};
use geom_num::grid::Steps;
use geom_num::interval::Interval;
use geom_nurbs::Limit;
use geom_run::{Budget, WorkMeter};
use geom_surface::{
    Cone, Cylinder, ImplicitBound, ParamBox, ParamSurface, Plane, Sphere, SurfaceKind, Torus,
};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The most bytes [`decode`] reads from any input: 7, the fields of [`Case`].
pub const MAX_INPUT_BYTES: usize = 7;

/// Cells along each parameter of the grid.
pub const CELLS: u32 = 24;

/// The floor of every subdivision.
const FLOOR: f64 = 1e-2;

/// The ceiling of every march.
const CEILING: f64 = 1e-1;

/// The work a subdivision may take. It is far above what most cases need, and bounds a broken one.
const WORK: u64 = 1 << 21;

/// What rounding may add to a start point's place on a surface: a few floors.
const REACH: f64 = 4.0 * FLOOR;

/// A half-turn is one parameter unit of the round kinds.
const TURN: f64 = 2.0;

/// One input of the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Case {
    /// Plane, cylinder, cone or torus, by its remainder on division by four. Its next bit gives a
    /// round kind one whole turn, so its domain closes on itself, and the bit after puts a cone's
    /// apex on its domain's edge (G173's seams and poles).
    pub surface: u8,
    /// How large the surface is.
    pub size: u8,
    /// A sphere or a plane, by its remainder on division by two.
    pub second: u8,
    /// Where the second surface stands, on a lattice of quarter units around the origin.
    pub at: [u8; 3],
    /// The work budget of the budgeted rerun.
    pub work: u8,
}

impl Case {
    /// Where the second surface stands.
    #[must_use]
    pub fn place(&self) -> [f64; 3] {
        self.at.map(|b| (f64::from(b % 17) - 8.0) / 4.0)
    }
}

/// What one case's answer was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Settled pairs, nothing flagged.
    Settled,
    /// Something flagged.
    Flagged,
    /// Nothing there.
    Empty,
    /// Stopped by its budget.
    Refused,
}

/// The case in `data`, and how many bytes it read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let case = Case {
        surface: src.byte(),
        size: src.byte(),
        second: src.byte(),
        at: [src.byte(), src.byte(), src.byte()],
        work: src.byte(),
    };
    (case, data.len().saturating_sub(src.remaining()))
}

/// `made`, or the finding that a drawn value was not valid.
fn must<T, E: core::fmt::Debug>(made: Result<T, E>, law: &str) -> Result<T, Finding> {
    made.map_err(|e| Finding::new(law, || format!("{e:?}")))
}

const LAW: &str = "the lattice's kinds are valid";

/// The first surface `case` draws.
///
/// # Errors
///
/// A finding if a kind's constructor refuses values the lattice keeps valid.
pub fn surface(case: Case) -> Result<SurfaceKind, Finding> {
    let frame = must(Frame::standard(), LAW)?;
    let size = 1.0 + f64::from(case.size % 8) / 2.0;
    let whole = (case.surface / 4) % 2 == 1;
    let round = if whole { (0.0, TURN) } else { (0.0625, 1.8125) };
    let apex = (case.surface / 8) % 2 == 1;
    Ok(match case.surface % 4 {
        0 => SurfaceKind::Plane(must(
            Plane::new(
                [0.0625, 0.03125, 0.125],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                ((-size, size), (-size, size)),
            ),
            LAW,
        )?),
        1 => SurfaceKind::Cylinder(must(
            Cylinder::new([0.0; 3], size, frame, (round, (-4.0, 4.0))),
            LAW,
        )?),
        // Its slope from the size: steep enough for the plane of `second` to cut its generators
        // at the apex, or shallow enough that it only touches the apex, a lone point.
        2 if apex => SurfaceKind::Cone(must(
            Cone::new([0.0; 3], 0.0, size, frame, (round, (0.0, 4.0))),
            LAW,
        )?),
        2 => SurfaceKind::Cone(must(
            Cone::new([0.0; 3], size, 0.25, frame, (round, (0.0, 4.0))),
            LAW,
        )?),
        _ => SurfaceKind::Torus(must(
            Torus::new([0.0; 3], size + 2.0, 1.0, frame, (round, round)),
            LAW,
        )?),
    })
}

/// The second surface: the same kind as its implicit form, both boxed for the case.
fn second(case: Case) -> Result<(SurfaceKind, Box<dyn ImplicitBound>), Finding> {
    let frame = must(Frame::standard(), LAW)?;
    let at = case.place();
    if case.second % 2 == 0 {
        let domain = ((0.0, TURN), (-0.45, 0.45));
        let sphere = must(Sphere::new(at, 2.5, frame, domain), LAW)?;
        Ok((SurfaceKind::Sphere(sphere.clone()), Box::new(sphere)))
    } else {
        let domain = ((-9.0, 9.0), (-9.0, 9.0));
        let plane = must(
            Plane::new(at, [1.0, 0.25, 0.0], [0.0, 1.0, 0.5], domain),
            LAW,
        )?;
        Ok((SurfaceKind::Plane(plane.clone()), Box::new(plane)))
    }
}

/// `k` of `count` steps across `(lo, hi)`, the last exact.
fn step(k: u32, count: u32, (lo, hi): (f64, f64)) -> f64 {
    if k >= count {
        hi
    } else {
        lo + (hi - lo) * f64::from(k) / f64::from(count)
    }
}

/// `surface` at `(u, v)`, in doubles.
fn on(surface: &SurfaceKind, u: f64, v: f64) -> Result<[f64; 3], Finding> {
    let ((_, b), (_, d)) = surface.domain();
    let side = |t: f64, hi: f64| {
        if t < hi {
            Limit::FromRight
        } else {
            Limit::FromLeft
        }
    };
    must(
        surface.point(u, v, (side(u, b), side(v, d))),
        "a kind evaluates inside its domain",
    )
}

/// The implicit form at `p`, in doubles.
fn value(implicit: &dyn ImplicitBound, p: [f64; 3]) -> Result<f64, Finding> {
    let law = "a kind's equation reads anywhere";
    let [x, y, z] = p.map(Interval::point);
    let found = must(
        implicit.enclose_box([must(x, law)?, must(y, law)?, must(z, law)?]),
        law,
    )?;
    Ok(found.value.lo() / 2.0 + found.value.hi() / 2.0)
}

/// Whether `(u, v)` lies within `room` of `region`.
fn within(region: geom_surface::ParamBox, (u, v): (f64, f64), room: [f64; 2]) -> bool {
    let ((a, b), (c, d)) = region;
    let [du, dv] = room;
    a - du <= u && u <= b + du && c - dv <= v && v <= d + dv
}

/// The laws of an answer.
///
/// # Errors
///
/// The first law `found` breaks.
pub fn judge(
    first: &SurfaceKind,
    second: &SurfaceKind,
    implicit: &dyn ImplicitBound,
    found: &Subdivided,
) -> Result<(), Finding> {
    for pair in &found.settled {
        if pair.starts.len() % 2 != 0 {
            return Err(Finding::new("a settled pair's count is even", || {
                format!("{pair:?}")
            }));
        }
        for [a, b, c, d] in &pair.starts {
            let mid = |x: &Interval| x.lo() / 2.0 + x.hi() / 2.0;
            let p = on(first, mid(a), mid(b))?;
            let q = on(second, mid(c), mid(d))?;
            let gap = p
                .iter()
                .zip(&q)
                .fold(0.0, |sum, (x, y)| sum + (x - y) * (x - y))
                .sqrt();
            if gap > REACH {
                return Err(Finding::new("a start point is on both surfaces", || {
                    format!("{gap}: {pair:?}")
                }));
            }
        }
    }
    let (us, vs) = first.domain();
    let room = [
        2.0 * (us.1 - us.0) / f64::from(CELLS),
        2.0 * (vs.1 - vs.0) / f64::from(CELLS),
    ];
    let rows = usize::try_from(CELLS).unwrap_or(0).saturating_add(1);
    let mut grid = Vec::new();
    for i in 0..=CELLS {
        for j in 0..=CELLS {
            let at = (step(i, CELLS, us), step(j, CELLS, vs));
            grid.push((at, value(implicit, on(first, at.0, at.1)?)?));
        }
    }
    for (k, &(at, g)) in grid.iter().enumerate() {
        let right = grid.get(k.saturating_add(rows));
        let up = grid
            .get(k.saturating_add(1))
            .filter(|_| k.saturating_add(1).checked_rem(rows) != Some(0));
        let changes = [right, up]
            .into_iter()
            .flatten()
            .any(|&(_, h)| (g > 0.0 && h < 0.0) || (g < 0.0 && h > 0.0));
        // The second surface is trimmed and its equation is not, so a crossing counts only where
        // the first surface's point is on the second's own patch.
        if changes && reaches(second, on(first, at.0, at.1)?) {
            let covered = found.settled.iter().any(|p| within(p.first, at, room))
                || found.flagged.iter().any(|p| within(p.first, at, room));
            if !covered {
                return Err(Finding::new(
                    "every crossing lies in a settled or flagged pair",
                    || format!("at {at:?}: {found:?}"),
                ));
            }
        }
    }
    Ok(())
}

/// Whether `x` is exactly `at`.
fn held(x: Interval, at: f64) -> bool {
    x.lo() == at && x.hi() == at
}

/// Each of (u₁, v₁, u₂, v₂): its domain's ends, and whether they close.
fn lines(surfaces: [&SurfaceKind; 2], charts: &[Charts; 2]) -> [((f64, f64), bool); 4] {
    let [one, two] = charts;
    let ([cu, cv], [cs, cr]) = (one.closes, two.closes);
    let [((u0, u1), (v0, v1)), ((s0, s1), (r0, r1))] = surfaces.map(ParamSurface::domain);
    [
        ((u0, u1), cu),
        ((v0, v1), cv),
        ((s0, s1), cs),
        ((r0, r1), cr),
    ]
}

/// Whether two ends of branches are one point across a closing seam: one held at each end of a
/// closing parameter, every other parameter overlapping.
fn across_a_seam(a: &[Interval; 4], b: &[Interval; 4], lines: &[((f64, f64), bool); 4]) -> bool {
    let mut seam = false;
    for ((x, y), ((lo, hi), closes)) in a.iter().zip(b).zip(lines) {
        if x.lo() <= y.hi() && y.lo() <= x.hi() {
            continue;
        }
        if *closes && ((held(*x, *lo) && held(*y, *hi)) || (held(*x, *hi) && held(*y, *lo))) {
            seam = true;
        } else {
            return false;
        }
    }
    seam
}

/// The laws of a march: every vertex is on both surfaces, every branch keeps within its ceiling,
/// every pass names a vertex on the line it passes, and no two ends across a closing seam are left
/// apart (12.10).
///
/// # Errors
///
/// The first law `traced` breaks.
pub fn judge_march(
    first: &SurfaceKind,
    second: &SurfaceKind,
    traced: &Traced,
) -> Result<(), Finding> {
    let read = [charts(first), charts(second)];
    let lines = lines([first, second], &read);
    let mut ends = Vec::new();
    for branch in &traced.branches {
        for pass in &branch.passes {
            let on_line = branch.vertices.get(pass.at).is_some_and(|x| {
                let [u, v, p, q] = *x;
                let [one, two] = &read;
                let on = |t: Interval, poles: &[f64]| poles.iter().any(|&at| held(t, at));
                match pass.passing {
                    Passing::Seam => x
                        .iter()
                        .zip(&lines)
                        .any(|(t, ((lo, hi), closes))| *closes && (held(*t, *lo) || held(*t, *hi))),
                    Passing::Pole => {
                        on(u, &one.poles.0)
                            || on(v, &one.poles.1)
                            || on(p, &two.poles.0)
                            || on(q, &two.poles.1)
                    }
                    Passing::KnotLine => false,
                }
            });
            if !on_line {
                return Err(Finding::new("a pass names a vertex on its line", || {
                    format!("{pass:?}: {branch:?}")
                }));
            }
        }
        if branch.ends != [CurveEnd::Closed, CurveEnd::Closed]
            && let (Some(a), Some(b)) = (branch.vertices.first(), branch.vertices.last())
        {
            ends.push(*a);
            ends.push(*b);
        }
    }
    for (k, a) in ends.iter().enumerate() {
        for b in ends.iter().skip(k.saturating_add(1)) {
            if across_a_seam(a, b, &lines) {
                return Err(Finding::new(
                    "two ends across a closing seam are one branch",
                    || format!("{a:?} and {b:?}: {traced:?}"),
                ));
            }
        }
    }
    for branch in &traced.branches {
        if branch.deviation > CEILING {
            return Err(Finding::new("a branch keeps within its ceiling", || {
                format!("{}: {branch:?}", branch.deviation)
            }));
        }
        for [a, b, c, d] in &branch.vertices {
            let mid = |x: &Interval| x.lo() / 2.0 + x.hi() / 2.0;
            let p = on(first, mid(a), mid(b))?;
            let q = on(second, mid(c), mid(d))?;
            let gap = p
                .iter()
                .zip(&q)
                .fold(0.0, |sum, (x, y)| sum + (x - y) * (x - y))
                .sqrt();
            if gap > REACH {
                return Err(Finding::new("a vertex is on both surfaces", || {
                    format!("{gap}: {branch:?}")
                }));
            }
        }
    }
    Ok(())
}

/// How far apart a parameter box's two surface points are, at the middle of each.
fn apart_at(
    first: &SurfaceKind,
    second: &SurfaceKind,
    [a, b, c, d]: &[Interval; 4],
) -> Result<f64, Finding> {
    let mid = |x: &Interval| x.lo() / 2.0 + x.hi() / 2.0;
    let p = on(first, mid(a), mid(b))?;
    let q = on(second, mid(c), mid(d))?;
    Ok(p.iter()
        .zip(&q)
        .fold(0.0, |sum, (x, y)| sum + (x - y) * (x - y))
        .sqrt())
}

/// Whether `outer` holds `inner`.
fn holds(outer: ParamBox, inner: ParamBox) -> bool {
    let ((p, q), (r, s)) = outer;
    let ((t, w), (x, y)) = inner;
    p <= t && w <= q && r <= x && y <= s
}

/// The laws of an analysis: a contact's point is on both surfaces, a settled pair's starts are, a
/// bound keeps within its ceiling, and every region lies on its surface.
///
/// # Errors
///
/// The first law `readings` breaks.
pub fn judge_contact(
    first: &SurfaceKind,
    second: &SurfaceKind,
    readings: &[Reading],
) -> Result<(), Finding> {
    let lying_on = |a: ParamBox, b: ParamBox| {
        if holds(first.domain(), a) && holds(second.domain(), b) {
            Ok(())
        } else {
            Err(Finding::new("a reading lies on its surfaces", || {
                format!("{a:?} {b:?}")
            }))
        }
    };
    let within = |steps: Steps| {
        if steps <= Steps(exact_ceiling()) {
            Ok(())
        } else {
            Err(Finding::new("a reading keeps within its ceiling", || {
                format!("{steps:?}")
            }))
        }
    };
    for reading in readings {
        match reading {
            Reading::Point { at, gap, contact } => {
                if !gap.contains(0.0) {
                    return Err(Finding::new("a contact's gap may be zero", || {
                        format!("{reading:?}")
                    }));
                }
                // A contact read from a cluster's branches is a box, not a point.
                let tight = matches!(
                    contact,
                    PointContact::IsolatedTangent | PointContact::Singular { branches: 4 }
                );
                if tight && apart_at(first, second, at)? > REACH {
                    return Err(Finding::new("a contact is on both surfaces", || {
                        format!("{reading:?}")
                    }));
                }
            }
            Reading::Crossing(pair) => {
                for start in &pair.starts {
                    if apart_at(first, second, start)? > REACH {
                        return Err(Finding::new("a settled start is on both surfaces", || {
                            format!("{pair:?}")
                        }));
                    }
                }
                lying_on(pair.first, pair.second)?;
            }
            Reading::Curve {
                first: a,
                second: b,
                contact,
            } => {
                if let CurveContact::Tangent { band, .. } = contact {
                    within(*band)?;
                }
                lying_on(*a, *b)?;
            }
            Reading::Overlap {
                first: a,
                second: b,
                coincidence,
            } => {
                if let Coincidence::Within(distance) = coincidence {
                    within(*distance)?;
                }
                lying_on(*a, *b)?;
            }
            Reading::Undecided {
                first: a,
                second: b,
                ..
            } => lying_on(*a, *b)?,
        }
    }
    Ok(())
}

/// [`CEILING`] rounded up to whole steps, as a reading rounds its bounds.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a small positive constant"
)]
fn exact_ceiling() -> u64 {
    CEILING.ceil() as u64
}

/// Whether a point near the second surface's equation stands over its trimmed patch: a sphere
/// band's latitude, or a plane patch's whole reach.
fn reaches(second: &SurfaceKind, p: [f64; 3]) -> bool {
    if let SurfaceKind::Sphere(sphere) = second {
        let [cx, cy, cz] = sphere.center();
        let [x, y, z] = p;
        let (dx, dy, dz) = (x - cx, y - cy, z - cz);
        let length = (dx * dx + dy * dy + dz * dz).sqrt();
        // A margin inside the band: its edge's own crossings are the first surface's business.
        return dz.abs() < 0.9 * length * 0.98;
    }
    true
}

/// Holds one case to the laws, and says what its answer was.
///
/// # Errors
///
/// The first law the case breaks.
pub fn check(case: Case) -> Result<Outcome, Finding> {
    let first = surface(case)?;
    let (other, implicit) = second(case)?;
    let run = |work: u64| {
        subdivide(
            &first,
            &other,
            FLOOR,
            &mut WorkMeter::new(Budget::of_work(work)),
        )
    };
    match run(WORK) {
        Ok(found) => {
            judge(&first, &other, implicit.as_ref(), &found)?;
            let mut meter = WorkMeter::new(Budget::of_work(WORK));
            match march(&first, &other, &found, FLOOR, CEILING, &mut meter) {
                Ok(traced) => judge_march(&first, &other, &traced)?,
                Err(IntersectError::BudgetExceeded | IntersectError::UnprovenMaths) => {}
                Err(e) => {
                    return Err(Finding::new(
                        "a march accounts for every start point",
                        || format!("{e:?}"),
                    ));
                }
            }
            let mut meter = WorkMeter::new(Budget::of_work(WORK));
            match analyse(&first, &other, &found, FLOOR, CEILING, &mut meter) {
                Ok(readings) => judge_contact(&first, &other, &readings)?,
                Err(IntersectError::BudgetExceeded | IntersectError::UnprovenMaths) => {}
                Err(e) => {
                    return Err(Finding::new(
                        "an analysis stops only on its budget or unproven maths",
                        || format!("{e:?}"),
                    ));
                }
            }
            match run(u64::from(case.work) * 256) {
                Ok(again) if again == found => {}
                Err(IntersectError::BudgetExceeded) => {}
                other => {
                    return Err(Finding::new(
                        "a budget stops an answer and never changes it",
                        || format!("{found:?} then {other:?}"),
                    ));
                }
            }
            Ok(if !found.flagged.is_empty() {
                Outcome::Flagged
            } else if found.settled.is_empty() {
                Outcome::Empty
            } else {
                Outcome::Settled
            })
        }
        Err(IntersectError::BudgetExceeded | IntersectError::UnprovenMaths) => Ok(Outcome::Refused),
        Err(e) => Err(Finding::new(
            "a subdivision stops only on its budget or unproven maths",
            || format!("{e:?}"),
        )),
    }
}

/// `decode`, then `check`.
///
/// # Errors
///
/// The first law the case breaks.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(decode(data).0).map(|_| ())
}
