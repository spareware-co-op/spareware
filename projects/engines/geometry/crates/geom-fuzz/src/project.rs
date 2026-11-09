//! Fuzz target: `geom-intersect`'s nearest points are never beaten by a sample, on any bytes.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: one of eight kinds on small
//! lattice sizes and domains, and a point on a coarse lattice, so points on an axis, ties and feet
//! on an end are common. [`check`] projects the point and holds the answer to the laws below, with
//! a dense sample of the kind as the oracle: the search bounds and prunes, and the oracle only
//! evaluates, so the two share nothing but the kind. [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes; a unit test builds
//!   the input that reaches it.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case: a sample
//!   nearer than the distance reported, a nearest point that is not on the kind where it says or
//!   not in the domain, points out of parameter order, a typed set whose members are not all at its
//!   distance, an internal error, or a budget that changes an answer instead of stopping it.
//!
//! Method: chapter 12.4's projection (`projects/engines/geometry/architecture/12-intersections.md`)
//! with chapter 28.5's oracle stack (`projects/engines/geometry/architecture/28-testing.md`). A
//! sample bounds the minimum from above, which is all a sample can do: the law cannot be broken by
//! a true answer, and a wrong one is caught once a sample lands nearer than it.
//!
//! Cost: one search and one budgeted rerun (chapter 12.4's cost), and at most [`CURVE_SAMPLES`] or
//! [`SURFACE_SAMPLES`] squared evaluations of the kind.

use geom_curve::{Axes, Circle, CurveKind, Ellipse, Frame, Line, ParamCurve};
use geom_intersect::{
    Continuum, IntersectError, Nearest, Projection, project_curve, project_surface,
};
use geom_num::interval::Interval;
use geom_nurbs::Limit;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, ParamSurface, Plane, Sphere, SurfaceKind, Torus};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The most bytes [`decode`] reads from any input: 8, the fields of [`Case`].
pub const MAX_INPUT_BYTES: usize = 8;

/// Samples along a curve, the ends included.
pub const CURVE_SAMPLES: u32 = 256;

/// Samples along each parameter of a surface, the ends included.
pub const SURFACE_SAMPLES: u32 = 32;

/// The floor of every search: a thousandth of a micrometre in a lattice of metres.
const FLOOR: f64 = 1e-6;

/// The work a search may take. It is far above what any case needs, and bounds a broken search.
const WORK: u64 = 1 << 24;

/// What rounding may add to a distance or a point worked out in doubles.
const SLACK: f64 = 1e-9;

/// A half-turn is one parameter unit of the round kinds.
const TURN: f64 = 2.0;

/// One input of the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Case {
    /// Line, circle, ellipse, plane, sphere, cylinder, cone or torus, by its remainder on division
    /// by eight.
    pub kind: u8,
    /// How large the kind is.
    pub size: u8,
    /// How much of the first parameter's domain is kept.
    pub trim: u8,
    /// How much of the second parameter's domain is kept.
    pub width: u8,
    /// The point, on a lattice of half units around the origin.
    pub point: [u8; 3],
    /// The work budget of the budgeted rerun.
    pub work: u8,
}

impl Case {
    /// The point projected: each coordinate a multiple of a half in [−4, 4], so zeros are common
    /// and a point on an axis is too.
    #[must_use]
    pub fn at(&self) -> [f64; 3] {
        self.point.map(|b| (f64::from(b % 17) - 8.0) / 2.0)
    }
}

/// A kind, a curve or a surface.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// A curve in space.
    Curve(Box<CurveKind<3>>),
    /// A surface.
    Surface(Box<SurfaceKind>),
}

/// What one case's answer was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// One nearest point.
    One,
    /// A tie.
    Tie,
    /// A typed set.
    Set,
    /// Undecided at the floor.
    NonIsolated,
    /// Refused or stopped.
    Refused,
}

/// The case in `data`, and how many bytes it read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let case = Case {
        kind: src.byte(),
        size: src.byte(),
        trim: src.byte(),
        width: src.byte(),
        point: [src.byte(), src.byte(), src.byte()],
        work: src.byte(),
    };
    (case, data.len().saturating_sub(src.remaining()))
}

/// `made`, or the finding that a drawn value was not valid.
fn must<T, E: core::fmt::Debug>(made: Result<T, E>, law: &str) -> Result<T, Finding> {
    made.map_err(|e| Finding::new(law, || format!("{e:?}")))
}

/// The kind `case` draws.
///
/// # Errors
///
/// A finding if a kind's constructor refuses values the lattice keeps valid.
pub fn shape(case: &Case) -> Result<Shape, Finding> {
    const LAW: &str = "the lattice's kinds are valid";
    let frame = must(Frame::standard(), LAW)?;
    let size = 1.0 + f64::from(case.size % 16) / 2.0;
    let kept = (f64::from(case.trim % 8) + 1.0) / 8.0;
    let across = (f64::from(case.width % 8) + 1.0) / 8.0;
    let round = (0.0, TURN * kept);
    let long = (-4.0 * across, 4.0 * across);
    Ok(match case.kind % 8 {
        0 => Shape::Curve(Box::new(CurveKind::Line(
            must(
                Line::segment([-size, 1.0, 0.5], [size, -1.0, 0.5 + across]),
                LAW,
            )?
            .value,
        ))),
        1 => Shape::Curve(Box::new(CurveKind::Circle(must(
            Circle::new([0.0; 3], size, frame, round),
            LAW,
        )?))),
        2 => Shape::Curve(Box::new(CurveKind::Ellipse(must(
            Ellipse::new(
                [0.0; 3],
                Axes::Frame {
                    frame,
                    a: size + 1.0,
                    b: size / 2.0,
                },
                round,
            ),
            LAW,
        )?))),
        3 => Shape::Surface(Box::new(SurfaceKind::Plane(must(
            Plane::new(
                [0.0, 0.0, 0.5],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                ((-size, size), long),
            ),
            LAW,
        )?))),
        4 => Shape::Surface(Box::new(SurfaceKind::Sphere(must(
            Sphere::new(
                [0.0; 3],
                size,
                frame,
                (round, (-0.5 * across, 0.5 * across)),
            ),
            LAW,
        )?))),
        5 => Shape::Surface(Box::new(SurfaceKind::Cylinder(must(
            Cylinder::new([0.0; 3], size, frame, (round, long)),
            LAW,
        )?))),
        6 => Shape::Surface(Box::new(SurfaceKind::Cone(must(
            Cone::new([0.0; 3], size, 0.25, frame, (round, long)),
            LAW,
        )?))),
        _ => Shape::Surface(Box::new(SurfaceKind::Torus(must(
            Torus::new(
                [0.0; 3],
                size + 2.0,
                1.0 + across,
                frame,
                (round, (0.0, TURN)),
            ),
            LAW,
        )?))),
    })
}

/// A meter of `work` units.
fn meter(work: u64) -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(work))
}

/// The Euclidean distance between two points.
fn distance(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y) * (x - y))
        .fold(0.0, |total, s| total + s)
        .sqrt()
}

/// `k` of `count` steps across `(lo, hi)`, the ends exact.
fn step(k: u32, count: u32, (lo, hi): (f64, f64)) -> f64 {
    if k.saturating_add(1) == count {
        hi
    } else {
        lo + (hi - lo) * f64::from(k) / (f64::from(count) - 1.0)
    }
}

/// The side a sample at `t` of `(lo, hi)` is read from: the one inside the domain.
fn side(t: f64, (_, hi): (f64, f64)) -> Limit {
    if t < hi {
        Limit::FromRight
    } else {
        Limit::FromLeft
    }
}

/// The middle of an enclosure, clamped to the domain it should lie in.
fn inside(x: Interval, (lo, hi): (f64, f64)) -> f64 {
    (x.lo() / 2.0 + x.hi() / 2.0).clamp(lo, hi)
}

/// `curve` at `t`, in doubles.
fn on_curve(curve: &CurveKind<3>, t: f64) -> Result<[f64; 3], Finding> {
    must(
        curve.point(t, side(t, curve.domain())),
        "a kind evaluates inside its domain",
    )
}

/// `surface` at `(u, v)`, in doubles.
fn on_surface(surface: &SurfaceKind, u: f64, v: f64) -> Result<[f64; 3], Finding> {
    let (us, vs) = surface.domain();
    let [point, ..] = must(
        surface.partials(u, v, (side(u, us), side(v, vs))),
        "a kind evaluates inside its domain",
    )?;
    Ok(point)
}

/// What each law allows for rounding at the reach of `p` and `points`.
fn slack(p: &[f64; 3], points: &[[f64; 3]]) -> f64 {
    let reach = points
        .iter()
        .chain(core::iter::once(p))
        .flatten()
        .fold(0.0f64, |m, x| if x.abs() > m { x.abs() } else { m });
    SLACK * (1.0 + reach)
}

/// No sample is nearer than a distance can be.
fn none_nearer(
    p: &[f64; 3],
    samples: &[[f64; 3]],
    reported: Interval,
    slack: f64,
) -> Result<(), Finding> {
    for s in samples {
        let d = distance(p, s);
        if d < reported.lo() - slack {
            return Err(Finding::new(
                "no sample is nearer than the distance reported",
                || format!("{s:?} at {d} from {p:?}: {reported:?}"),
            ));
        }
    }
    Ok(())
}

/// One nearest point's law: it lies on the kind where it says, at the distance it says.
fn on_the_kind<const N: usize>(
    p: &[f64; 3],
    nearest: &Nearest<N, 3>,
    at: [f64; 3],
    slack: f64,
) -> Result<(), Finding> {
    let d = distance(p, &at);
    let room = slack + 4.0 * FLOOR;
    if d < nearest.distance.lo() - room || nearest.distance.hi() + room < d {
        return Err(Finding::new(
            "a nearest point is at the distance it reports",
            || format!("{at:?} is {d} from {p:?}: {nearest:?}"),
        ));
    }
    let off = nearest
        .point
        .iter()
        .zip(&at)
        .any(|(x, y)| *y < x.lo() - room || x.hi() + room < *y);
    if off {
        return Err(Finding::new(
            "a nearest point lies on the kind at its parameters",
            || format!("{at:?}: {nearest:?}"),
        ));
    }
    Ok(())
}

/// Whether `x` overlaps `(lo, hi)`.
fn overlaps(x: Interval, (lo, hi): (f64, f64)) -> bool {
    x.lo() <= hi && lo <= x.hi()
}

/// The laws of a curve's answer.
///
/// # Errors
///
/// The first law `found` breaks.
pub fn judge_curve(
    curve: &CurveKind<3>,
    p: &[f64; 3],
    found: &Projection<1, 3>,
) -> Result<(), Finding> {
    let domain = curve.domain();
    let samples = (0..CURVE_SAMPLES)
        .map(|k| on_curve(curve, step(k, CURVE_SAMPLES, domain)))
        .collect::<Result<Vec<_>, _>>()?;
    let slack = slack(p, &samples);
    match found {
        Projection::Points(points) => {
            let mut last = f64::NEG_INFINITY;
            for nearest in points {
                let [at] = nearest.at;
                if !overlaps(at, domain) || at.lo() < last {
                    return Err(Finding::new(
                        "nearest points are in the domain and in parameter order",
                        || format!("{found:?}"),
                    ));
                }
                last = at.lo();
                none_nearer(p, &samples, nearest.distance, slack)?;
                on_the_kind(p, nearest, on_curve(curve, inside(at, domain))?, slack)?;
            }
            if points.is_empty() {
                return Err(Finding::new("a kind has a nearest point", || {
                    format!("{found:?}")
                }));
            }
            Ok(())
        }
        Projection::Set { distance, .. } => {
            none_nearer(p, &samples, *distance, slack)?;
            all_at(p, &samples, *distance, slack)
        }
        Projection::NonIsolated { enclosure: [at] } if overlaps(*at, domain) => Ok(()),
        Projection::NonIsolated { .. } => {
            Err(Finding::new("what is undecided is in the domain", || {
                format!("{found:?}")
            }))
        }
    }
}

/// Every one of `points` is at `reported`.
fn all_at(
    p: &[f64; 3],
    points: &[[f64; 3]],
    reported: Interval,
    slack: f64,
) -> Result<(), Finding> {
    for s in points {
        let d = distance(p, s);
        if d < reported.lo() - slack || reported.hi() + slack < d {
            return Err(Finding::new(
                "every member of a typed set is at its distance",
                || format!("{s:?} at {d} from {p:?}: {reported:?}"),
            ));
        }
    }
    Ok(())
}

/// The laws of a surface's answer.
///
/// # Errors
///
/// The first law `found` breaks.
pub fn judge_surface(
    surface: &SurfaceKind,
    p: &[f64; 3],
    found: &Projection<2, 3>,
) -> Result<(), Finding> {
    let (us, vs) = surface.domain();
    let mut samples = Vec::new();
    for i in 0..SURFACE_SAMPLES {
        for j in 0..SURFACE_SAMPLES {
            let (u, v) = (step(i, SURFACE_SAMPLES, us), step(j, SURFACE_SAMPLES, vs));
            samples.push(on_surface(surface, u, v)?);
        }
    }
    let slack = slack(p, &samples);
    match found {
        Projection::Points(points) => {
            let mut last = (f64::NEG_INFINITY, f64::NEG_INFINITY);
            for nearest in points {
                let [u, v] = nearest.at;
                let here = (u.lo(), v.lo());
                let ordered = here.0 > last.0 || (here.0 >= last.0 && here.1 >= last.1);
                if !(overlaps(u, us) && overlaps(v, vs) && ordered) {
                    return Err(Finding::new(
                        "nearest points are in the domain and in parameter order",
                        || format!("{found:?}"),
                    ));
                }
                last = here;
                none_nearer(p, &samples, nearest.distance, slack)?;
                let at = on_surface(surface, inside(u, us), inside(v, vs))?;
                on_the_kind(p, nearest, at, slack)?;
            }
            if points.is_empty() {
                return Err(Finding::new("a kind has a nearest point", || {
                    format!("{found:?}")
                }));
            }
            Ok(())
        }
        Projection::Set { set, distance } => {
            none_nearer(p, &samples, *distance, slack)?;
            match set {
                Continuum::Parallel { v } => {
                    let v = inside(*v, vs);
                    let parallel = (0..SURFACE_SAMPLES)
                        .map(|i| on_surface(surface, step(i, SURFACE_SAMPLES, us), v))
                        .collect::<Result<Vec<_>, _>>()?;
                    all_at(p, &parallel, *distance, slack + 4.0 * FLOOR)
                }
                Continuum::Curve | Continuum::Surface => all_at(p, &samples, *distance, slack),
            }
        }
        Projection::NonIsolated { enclosure: [u, v] } if overlaps(*u, us) && overlaps(*v, vs) => {
            Ok(())
        }
        Projection::NonIsolated { .. } => {
            Err(Finding::new("what is undecided is in the domain", || {
                format!("{found:?}")
            }))
        }
    }
}

/// What an answer was, and the law that a refusal of `error` breaks.
fn refusal(error: IntersectError) -> Result<Outcome, Finding> {
    match error {
        IntersectError::BudgetExceeded | IntersectError::UnprovenMaths => Ok(Outcome::Refused),
        e => Err(Finding::new(
            "a search stops only on its budget or unproven maths",
            || format!("{e:?}"),
        )),
    }
}

/// How `found` is classed.
fn outcome<const N: usize>(found: &Projection<N, 3>) -> Outcome {
    match found {
        Projection::Points(_) if found.tie_broken() => Outcome::Tie,
        Projection::Points(_) => Outcome::One,
        Projection::Set { .. } => Outcome::Set,
        Projection::NonIsolated { .. } => Outcome::NonIsolated,
    }
}

/// A budget changes nothing but whether an answer comes.
fn rerun<const N: usize>(
    full: &Projection<N, 3>,
    budgeted: Result<Projection<N, 3>, IntersectError>,
) -> Result<(), Finding> {
    match budgeted {
        Ok(again) if again == *full => Ok(()),
        Err(IntersectError::BudgetExceeded) => Ok(()),
        other => Err(Finding::new(
            "a budget stops an answer and never changes it",
            || format!("{full:?} then {other:?}"),
        )),
    }
}

/// Holds one case to the laws, and says what its answer was.
///
/// # Errors
///
/// The first law the case breaks.
pub fn check(case: &Case) -> Result<Outcome, Finding> {
    let p = case.at();
    let work = u64::from(case.work) * 256;
    match shape(case)? {
        Shape::Curve(curve) => match project_curve(&curve, &p, FLOOR, &mut meter(WORK)) {
            Ok(found) => {
                judge_curve(&curve, &p, &found)?;
                rerun(&found, project_curve(&curve, &p, FLOOR, &mut meter(work)))?;
                Ok(outcome(&found))
            }
            Err(e) => refusal(e),
        },
        Shape::Surface(surface) => match project_surface(&surface, &p, FLOOR, &mut meter(WORK)) {
            Ok(found) => {
                judge_surface(&surface, &p, &found)?;
                rerun(
                    &found,
                    project_surface(&surface, &p, FLOOR, &mut meter(work)),
                )?;
                Ok(outcome(&found))
            }
            Err(e) => refusal(e),
        },
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
