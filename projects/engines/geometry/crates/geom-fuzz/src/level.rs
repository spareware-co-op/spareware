//! Fuzz target: `geom-intersect`'s level curves miss nothing a dense grid can see, on any bytes.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: one of five surfaces on small
//! lattice sizes, and one of six functions on it. [`check`] traces the zero set and holds the
//! answer to the laws below, with a dense grid of the function's own value as the oracle: the trace
//! bounds and prunes, and the oracle only evaluates. [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes; a unit test builds
//!   the input that reaches it.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case: a sign
//!   change on the grid with no branch and no flag near it, a vertex off the curve or out of the
//!   domain, a branch with a chord further out than the ceiling lets it, an internal error, or a
//!   budget that changes an answer instead of stopping it.
//!
//! Method: chapter 12.8's implicit tracer and 12.13's level curves
//! (`projects/engines/geometry/architecture/12-intersections.md`) with chapter 28.5's oracle stack
//! (`projects/engines/geometry/architecture/28-testing.md`). A sign change between neighbours
//! proves a zero between them, so the law cannot be broken by a true answer; the grid sees nothing
//! smaller than a cell, which is why [`crate::level`]'s own tests carry small loops.
//!
//! Cost: one trace and one budgeted rerun (chapter 12.8's cost), and [`CELLS`] squared evaluations.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are chapter 12's: g the function, u and v the parameters"
)]

use geom_curve::Frame;
use geom_intersect::{
    Branch, Envelope, IntersectError, Isocline, Level, LevelCurves, OnImplicit, Perspective,
    Silhouette, trace,
};
use geom_num::interval::Interval;
use geom_nurbs::Limit;
use geom_run::{Budget, WorkMeter};
use geom_surface::{
    Cone, Cylinder, ImplicitBound, ParamSurface, Plane, Sphere, SurfaceKind, Torus,
};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The most bytes [`decode`] reads from any input: 8, the fields of [`Case`].
pub const MAX_INPUT_BYTES: usize = 8;

/// Cells along each parameter of the grid.
pub const CELLS: u32 = 32;

/// The floor of every trace.
const FLOOR: f64 = 1e-5;

/// The ceiling of every trace: a thousandth, in a lattice of metres.
const CEILING: f64 = 1e-3;

/// The work a trace may take. It is far above what most cases need, and bounds a broken trace.
const WORK: u64 = 1 << 21;

/// A half-turn is one parameter unit of the round kinds.
const TURN: f64 = 2.0;

/// One input of the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Case {
    /// Plane, sphere, cylinder, cone or torus, by its remainder on division by five.
    pub surface: u8,
    /// How large the surface is.
    pub size: u8,
    /// Which function: on a sphere, on a plane, a silhouette, a perspective, an isocline or an
    /// envelope, by its remainder on division by six.
    pub level: u8,
    /// The function's three numbers, on a lattice of quarter units around the origin.
    pub at: [u8; 3],
    /// How much of the surface's domain is kept.
    pub trim: u8,
    /// The work budget of the budgeted rerun.
    pub work: u8,
}

impl Case {
    /// The function's three numbers: multiples of a quarter in [−4, 4].
    #[must_use]
    pub fn numbers(&self) -> [f64; 3] {
        self.at.map(|b| (f64::from(b % 33) - 16.0) / 4.0)
    }
}

/// What one case's answer was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Its branches, none closed.
    Open,
    /// At least one closed branch.
    Closed,
    /// Nothing there.
    Empty,
    /// Something flagged.
    Flagged,
    /// Refused or stopped.
    Refused,
}

/// The case in `data`, and how many bytes it read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let case = Case {
        surface: src.byte(),
        size: src.byte(),
        level: src.byte(),
        at: [src.byte(), src.byte(), src.byte()],
        trim: src.byte(),
        work: src.byte(),
    };
    (case, data.len().saturating_sub(src.remaining()))
}

/// `made`, or the finding that a drawn value was not valid.
fn must<T, E: core::fmt::Debug>(made: Result<T, E>, law: &str) -> Result<T, Finding> {
    made.map_err(|e| Finding::new(law, || format!("{e:?}")))
}

const LAW: &str = "the lattice's kinds are valid";

/// The surface `case` draws.
///
/// # Errors
///
/// A finding if a kind's constructor refuses values the lattice keeps valid.
pub fn surface(case: &Case) -> Result<SurfaceKind, Finding> {
    let frame = must(Frame::standard(), LAW)?;
    let size = 1.0 + f64::from(case.size % 16) / 2.0;
    let kept = (f64::from(case.trim % 8) + 1.0) / 8.0;
    let (round, long) = ((0.0625, TURN * kept), (-4.0 * kept, 4.0 * kept));
    Ok(match case.surface % 5 {
        0 => SurfaceKind::Plane(must(
            Plane::new(
                [0.0, 0.0, 0.5],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                ((-size, size), long),
            ),
            LAW,
        )?),
        1 => SurfaceKind::Sphere(must(
            Sphere::new([0.0; 3], size, frame, (round, (-0.4 * kept, 0.4 * kept))),
            LAW,
        )?),
        2 => SurfaceKind::Cylinder(must(
            Cylinder::new([0.0; 3], size, frame, (round, long)),
            LAW,
        )?),
        3 => SurfaceKind::Cone(must(
            Cone::new([0.0; 3], size, 0.25, frame, (round, (0.0, 4.0 * kept))),
            LAW,
        )?),
        _ => SurfaceKind::Torus(must(
            Torus::new([0.0; 3], size + 2.0, 1.0, frame, (round, (0.0625, TURN))),
            LAW,
        )?),
    })
}

/// Traces `level` on `surface`, under `work`.
fn run(surface: &SurfaceKind, level: &dyn Level, work: u64) -> Result<LevelCurves, IntersectError> {
    trace(
        surface,
        level,
        FLOOR,
        CEILING,
        &mut WorkMeter::new(Budget::of_work(work)),
    )
}

/// g at `(u, v)`, in doubles.
fn value(surface: &SurfaceKind, level: &dyn Level, (u, v): (f64, f64)) -> Result<f64, Finding> {
    let ((_, b), (_, d)) = surface.domain();
    let side = |t: f64, hi: f64| {
        if t < hi {
            Limit::FromRight
        } else {
            Limit::FromLeft
        }
    };
    let law = "a function reads inside its domain";
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let partials = must(
        surface.partials_over(
            ((u, u), (v, v)),
            (side(u, b), side(v, d)),
            level.order(),
            &mut meter,
        ),
        law,
    )?;
    let sample = must(level.sample(&partials), law)?;
    Ok(sample.value.lo() / 2.0 + sample.value.hi() / 2.0)
}

/// The middle of an enclosure.
fn mid(x: Interval) -> f64 {
    x.lo() / 2.0 + x.hi() / 2.0
}

/// `k` of `count` steps across `(lo, hi)`, the last exact.
fn step(k: u32, count: u32, (lo, hi): (f64, f64)) -> f64 {
    if k >= count {
        hi
    } else {
        lo + (hi - lo) * f64::from(k) / f64::from(count)
    }
}

/// The least distance from `(u, v)` to `branch`'s chords.
fn reach(branch: &Branch, (u, v): (f64, f64)) -> f64 {
    let chord = |a: &[Interval; 2], b: &[Interval; 2]| {
        let ([p, q], [r, s]) = (a, b);
        let (x0, y0, x1, y1) = (mid(*p), mid(*q), mid(*r), mid(*s));
        let (dx, dy) = (x1 - x0, y1 - y0);
        let length = dx * dx + dy * dy;
        let t = if length > 0.0 {
            (((u - x0) * dx + (v - y0) * dy) / length).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (ex, ey) = (u - x0 - t * dx, v - y0 - t * dy);
        ex * ex + ey * ey
    };
    let mut best = f64::INFINITY;
    for pair in branch.vertices.windows(2) {
        if let [a, b] = pair {
            let d = chord(a, b);
            if d < best {
                best = d;
            }
        }
    }
    if let (Some(a), Some(b)) = (branch.vertices.first(), branch.vertices.last()) {
        let d = chord(b, a);
        if d < best {
            best = d;
        }
    }
    best.sqrt()
}

/// The laws of an answer.
///
/// # Errors
///
/// The first law `found` breaks.
pub fn judge(surface: &SurfaceKind, level: &dyn Level, found: &LevelCurves) -> Result<(), Finding> {
    let (us, vs) = surface.domain();
    let mut grid = Vec::new();
    for i in 0..=CELLS {
        for j in 0..=CELLS {
            let at = (step(i, CELLS, us), step(j, CELLS, vs));
            grid.push((at, value(surface, level, at)?));
        }
    }
    let (du, dv) = (
        (us.1 - us.0) / f64::from(CELLS),
        (vs.1 - vs.0) / f64::from(CELLS),
    );
    let room = 2.0 * (du * du + dv * dv).sqrt();
    let rows = usize::try_from(CELLS).unwrap_or(0).saturating_add(1);
    for (k, &(at, g)) in grid.iter().enumerate() {
        let right = grid.get(k.saturating_add(rows));
        let up = grid
            .get(k.saturating_add(1))
            .filter(|_| k.saturating_add(1).checked_rem(rows) != Some(0));
        let changes = [right, up]
            .into_iter()
            .flatten()
            .any(|&(_, h)| (g > 0.0 && h < 0.0) || (g < 0.0 && h > 0.0));
        if changes {
            let near_branch = found.branches.iter().any(|b| reach(b, at) <= room);
            let near_flag = found.flagged.iter().any(|f| {
                let [x, y] = f.at;
                x.lo() - room <= at.0
                    && at.0 <= x.hi() + room
                    && y.lo() - room <= at.1
                    && at.1 <= y.hi() + room
            });
            if !(near_branch || near_flag) {
                return Err(Finding::new(
                    "every sign change on the grid is on a branch or a flag",
                    || format!("at {at:?}: {found:?}"),
                ));
            }
        }
    }
    for branch in &found.branches {
        for [u, v] in &branch.vertices {
            let inside = u.lo() <= us.1 && us.0 <= u.hi() && v.lo() <= vs.1 && vs.0 <= v.hi();
            if !inside {
                return Err(Finding::new("a vertex is in the domain", || {
                    format!("{u:?} {v:?}: {found:?}")
                }));
            }
        }
        if branch.deviation > CEILING {
            return Err(Finding::new("a branch keeps within its ceiling", || {
                format!("{}: {found:?}", branch.deviation)
            }));
        }
    }
    Ok(())
}

/// How an answer is classed.
fn outcome(found: &LevelCurves) -> Outcome {
    use geom_intersect::result::CurveEnd;
    if !found.flagged.is_empty() {
        Outcome::Flagged
    } else if found.branches.is_empty() {
        Outcome::Empty
    } else if found
        .branches
        .iter()
        .any(|b| b.ends.contains(&CurveEnd::Closed))
    {
        Outcome::Closed
    } else {
        Outcome::Open
    }
}

/// Holds `level` on `surface` to the laws, with `work` for the budgeted rerun.
fn hold(surface: &SurfaceKind, level: &dyn Level, work: u64) -> Result<Outcome, Finding> {
    match run(surface, level, WORK) {
        Ok(found) => {
            judge(surface, level, &found)?;
            match run(surface, level, work) {
                Ok(again) if again == found => Ok(outcome(&found)),
                Err(IntersectError::BudgetExceeded) => Ok(outcome(&found)),
                other => Err(Finding::new(
                    "a budget stops an answer and never changes it",
                    || format!("{found:?} then {other:?}"),
                )),
            }
        }
        Err(IntersectError::BudgetExceeded | IntersectError::UnprovenMaths) => Ok(Outcome::Refused),
        Err(e) => Err(Finding::new(
            "a trace stops only on its budget or unproven maths",
            || format!("{e:?}"),
        )),
    }
}

/// Holds one case to the laws, and says what its answer was.
///
/// # Errors
///
/// The first law the case breaks.
pub fn check(case: &Case) -> Result<Outcome, Finding> {
    let shape = surface(case)?;
    let numbers = case.numbers();
    let work = u64::from(case.work) * 64;
    let frame = must(Frame::standard(), LAW)?;
    let domain = ((0.0, TURN), (-0.5, 0.5));
    match case.level % 6 {
        0 => {
            let sphere = must(Sphere::new(numbers, 2.5, frame, domain), LAW)?;
            let implicit: &dyn ImplicitBound = &sphere;
            hold(&shape, &OnImplicit(implicit), work)
        }
        1 => {
            let plane = must(
                Plane::new(numbers, [1.0, 0.25, 0.0], [0.0, 1.0, 0.5], domain),
                LAW,
            )?;
            let implicit: &dyn ImplicitBound = &plane;
            hold(&shape, &OnImplicit(implicit), work)
        }
        2 => hold(&shape, &Silhouette { direction: numbers }, work),
        3 => hold(&shape, &Perspective { eye: numbers }, work),
        4 => {
            let level = Isocline {
                direction: numbers,
                cosine: 0.6,
            };
            hold(&shape, &level, work)
        }
        _ => {
            let level = Envelope {
                linear: numbers,
                angular: [0.0, 0.0, 1.0],
                about: [1.0, 0.0, 0.0],
            };
            hold(&shape, &level, work)
        }
    }
}

/// `decode`, then `check`.
///
/// # Errors
///
/// The first law the case breaks.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0).map(|_| ())
}
