//! Group 2 of chapter 8.3 on every curve and surface kind no closed form covers: a vertex on any
//! surface, a vertex at any curve's end, and a pcurve against its edge's 3D curve, certified with
//! bounds, never sampled (G177).
//!
//! What it does: [`vertex_on_surface`] reads the surface's nearest points to the vertex,
//! [`vertex_at`] encloses the curve at its end, and [`pcurve_on`] bounds f(t) = S(p(t)) − C(t)
//! between a fin's pcurve p, mapped through its face's surface S, and its edge's curve C, piece by
//! piece (`geom_intersect::gap`). Each returns a [`Verdict`]: holds, fails, or cannot be decided
//! here, which the caller reports as unchecked.
//!
//! Guarantees:
//!
//! - **A vertex on a surface** is the nearest-point search of 12.4
//!   (`geom_intersect::project_surface`): it holds when a proven nearest point lies within the
//!   bound, fails when every nearest point is proven farther, and is undecided otherwise.
//! - **A vertex at a curve's end** is the curve's enclosure at that parameter: it holds when the
//!   enclosure's farthest point is within the bound, and fails when its nearest is beyond it.
//! - **A pcurve on its edge** holds when every piece of the edge's span is within the bound by
//!   the mean-value form, halving a piece up to [`PIECES`] times; it fails when one piece's middle
//!   is proven beyond the bound. When it holds, every point of C lies within the bound of the point
//!   S(p(t)) of the surface, so the edge lies on the face too.
//! - **Fails closed.** A kind's error, a parameter outside a domain, or a pcurve on another span
//!   than its edge leave the check undecided, never passed; only the meter's stop ends it.
//!
//! Method: chapter 8.3's group 2 (`projects/engines/geometry/architecture/08-topology.md`), with the
//! nearest points of chapter 12.4 and the bounds of `geom_intersect::gap`.
//!
//! Cost: one nearest-point search per vertex and face; per pcurve up to 2^[`PIECES`] pieces, each
//! a few reads of the kinds. Recursion: none.

use alloc::vec::Vec;

use geom_curve::ParamCurve;
use geom_curve::tight::larger;
use geom_intersect::{Projection, gap, project_surface};
use geom_num::interval::Interval;
use geom_run::{Stopped, WorkMeter};
use geom_topo::{CurveKind, SurfaceKind};

/// The depth to which a pcurve's span is halved: at most 2^`PIECES` pieces.
pub const PIECES: usize = 6;

/// What a check found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// Within the bound.
    Holds,
    /// Proven beyond it.
    Fails,
    /// Not decided here: reported unchecked.
    Unknown,
}

/// What an error of `geom-intersect` or a kind leaves: the meter's stop, if it stopped, and
/// otherwise an undecided check.
fn undecided(meter: &WorkMeter<'_>) -> Result<Verdict, Stopped> {
    meter.check()?;
    Ok(Verdict::Unknown)
}

/// The middle of an interval.
fn mid(x: Interval) -> f64 {
    x.lo() / 2.0 + x.hi() / 2.0
}

/// The verdict on a distance enclosed by `d`, against the bound `b`.
fn against(d: Interval, b: f64) -> Verdict {
    if d.hi() <= b {
        Verdict::Holds
    } else if d.lo() > b {
        Verdict::Fails
    } else {
        Verdict::Unknown
    }
}

/// Whether the point `p` (exact) lies within `b` of `surface` (module documentation).
pub(crate) fn vertex_on_surface(
    surface: &SurfaceKind,
    p: &[Interval; 3],
    b: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Verdict, Stopped> {
    // A floor below the bound, so the nearest points come back fine enough to decide it, and
    // never below an eighth of a step, finer than the grid's points can be told apart.
    let floor = larger(b, 1.0) / 8.0;
    match project_surface(surface, &p.map(mid), floor, meter) {
        Ok(Projection::Points(feet)) => {
            if feet.is_empty() {
                return Ok(Verdict::Unknown);
            }
            let mut verdict = Verdict::Fails;
            for foot in &feet {
                match against(foot.distance, b) {
                    Verdict::Holds => return Ok(Verdict::Holds),
                    Verdict::Unknown => verdict = Verdict::Unknown,
                    Verdict::Fails => {}
                }
            }
            Ok(verdict)
        }
        Ok(Projection::Set { distance, .. }) => Ok(against(distance, b)),
        Ok(Projection::NonIsolated { .. }) => Ok(Verdict::Unknown),
        Err(_) => undecided(meter),
    }
}

/// Whether the point `p` (exact) lies within `b` of `curve` at `t`.
pub(crate) fn vertex_at(
    curve: &CurveKind<3>,
    t: f64,
    p: &[Interval; 3],
    b: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Verdict, Stopped> {
    match gap::at_point(curve, t, p, meter) {
        Ok(d) => Ok(against(d, b)),
        Err(_) => undecided(meter),
    }
}

/// Whether `pcurve`, mapped through `surface`, lies within `b` of `curve` over their common span
/// (module documentation).
pub(crate) fn pcurve_on(
    surface: &SurfaceKind,
    pcurve: &CurveKind<2>,
    curve: &CurveKind<3>,
    b: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Verdict, Stopped> {
    let span = curve.domain();
    if pcurve.domain() != span {
        return Ok(Verdict::Unknown);
    }
    let mut work: Vec<((f64, f64), usize)> = alloc::vec![(span, 0)];
    let mut verdict = Verdict::Holds;
    while let Some(((a, c), depth)) = work.pop() {
        meter.charge(1)?;
        let Ok(read) = gap::mapped(surface, (pcurve, curve), (a, c), meter) else {
            return undecided(meter);
        };
        let Some((middle, upper)) = read else {
            verdict = Verdict::Unknown;
            continue;
        };
        if middle.lo() > b {
            return Ok(Verdict::Fails);
        }
        if upper <= b {
            continue;
        }
        let m = a / 2.0 + c / 2.0;
        if depth < PIECES && a < m && m < c {
            let deeper = depth.saturating_add(1);
            work.push(((m, c), deeper));
            work.push(((a, m), deeper));
        } else {
            verdict = Verdict::Unknown;
        }
    }
    Ok(verdict)
}
