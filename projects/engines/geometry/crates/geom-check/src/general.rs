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
use geom_intersect::{Nearest, Projection, gap, project_surface};
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
        Ok(Projection::Points(feet)) => Ok(nearest(&feet, b)),
        Ok(Projection::Set { distance, .. }) => Ok(against(distance, b)),
        Ok(Projection::NonIsolated { .. }) => Ok(Verdict::Unknown),
        Err(_) => undecided(meter),
    }
}

/// The verdict on a point whose nearest points on a surface are `feet`, against the bound `b`: it
/// holds when one is within the bound, fails when every one is proven beyond it, and is undecided
/// otherwise, no foot at all included.
fn nearest(feet: &[Nearest<2, 3>], b: f64) -> Verdict {
    if feet.is_empty() {
        return Verdict::Unknown;
    }
    let mut verdict = Verdict::Fails;
    for foot in feet {
        match against(foot.distance, b) {
            Verdict::Holds => return Verdict::Holds,
            Verdict::Unknown => verdict = Verdict::Unknown,
            Verdict::Fails => {}
        }
    }
    verdict
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

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::indexing_slicing,
        reason = "test code: a failure should stop loudly"
    )]

    use geom_curve::{Frame, Line};
    use geom_intersect::Place;
    use geom_nurbs::{Basis, Surface};
    use geom_run::Budget;
    use geom_surface::Sphere;

    use super::*;

    fn meter() -> WorkMeter<'static> {
        WorkMeter::new(Budget::UNLIMITED)
    }

    fn exact(p: [f64; 3]) -> [Interval; 3] {
        p.map(|x| Interval::point(x).unwrap())
    }

    /// S(u, v) = (u, v, 0) over [0, 10]², a bilinear NURBS patch.
    fn floor() -> SurfaceKind {
        let basis = || Basis::new(1, alloc::vec![0.0, 0.0, 10.0, 10.0]).unwrap();
        let net = [
            [0.0, 0.0, 0.0],
            [0.0, 10.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
        ];
        SurfaceKind::Nurbs(Surface::new(basis(), basis(), &net).unwrap())
    }

    /// A pcurve on [`floor`] from `(x, y)` along `(dx, 0)` over `span`, and its edge's curve, the
    /// same line in space.
    fn on_floor((x, y): (f64, f64), dx: f64, span: (f64, f64)) -> (CurveKind<2>, CurveKind<3>) {
        (
            CurveKind::Line(Line::new([x, y], [dx, 0.0], span).unwrap()),
            CurveKind::Line(Line::new([x, y, 0.0], [dx, 0.0, 0.0], span).unwrap()),
        )
    }

    /// A nearest point at `distance`.
    fn foot(distance: Interval) -> Nearest<2, 3> {
        Nearest {
            at: [Interval::point(0.0).unwrap(); 2],
            point: exact([0.0; 3]),
            distance,
            place: Place::Foot,
        }
    }

    #[test]
    fn no_foot_is_undecided_and_one_within_the_bound_is_enough() {
        let near = foot(Interval::point(0.5).unwrap());
        let far = foot(Interval::point(5.0).unwrap());
        let straddling = foot(Interval::new(0.5, 5.0).unwrap());
        assert_eq!(nearest(&[], 1.0), Verdict::Unknown);
        assert_eq!(nearest(&[far, near], 1.0), Verdict::Holds);
        assert_eq!(nearest(&[far, far], 1.0), Verdict::Fails);
        assert_eq!(nearest(&[far, straddling], 1.0), Verdict::Unknown);
    }

    #[test]
    fn a_vertex_read_past_its_curves_domain_is_undecided_never_passed() {
        let (_, segment) = on_floor((0.0, 0.0), 10.0, (0.0, 1.0));
        let end = exact([10.0, 0.0, 0.0]);
        assert_eq!(
            vertex_at(&segment, 1.0, &end, 1.0, &mut meter()).unwrap(),
            Verdict::Holds
        );
        assert_eq!(
            vertex_at(&segment, 2.0, &end, 1.0, &mut meter()).unwrap(),
            Verdict::Unknown
        );
    }

    #[test]
    fn a_sphere_seen_from_its_centre_is_judged_by_its_radius() {
        let ball = SurfaceKind::Sphere(
            Sphere::new(
                [0.0; 3],
                4.0,
                Frame::standard().unwrap(),
                ((0.0, 2.0), (-0.5, 0.5)),
            )
            .unwrap(),
        );
        let centre = exact([0.0; 3]);
        assert_eq!(
            vertex_on_surface(&ball, &centre, 5.0, &mut meter()).unwrap(),
            Verdict::Holds
        );
        assert_eq!(
            vertex_on_surface(&ball, &centre, 3.0, &mut meter()).unwrap(),
            Verdict::Fails
        );
    }

    /// A rational quarter cylinder of radius 5 is a NURBS, so no closed form sees its axis: from a
    /// point on the axis every point of the arc is nearest, and the search says so instead of
    /// naming a foot.
    #[test]
    fn a_continuum_no_kind_names_is_undecided() {
        let line = Basis::new(1, alloc::vec![0.0, 0.0, 1.0, 1.0]).unwrap();
        let arc = Basis::new(2, alloc::vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
        let mut net = Vec::new();
        let mut weights = Vec::new();
        for z in [-2.5, 2.5] {
            net.extend([[5.0, 0.0, z], [5.0, 5.0, z], [0.0, 5.0, z]]);
            weights.extend([1.0, core::f64::consts::FRAC_1_SQRT_2, 1.0]);
        }
        let quarter = SurfaceKind::Nurbs(Surface::new_rational(line, arc, &net, &weights).unwrap());
        assert!(matches!(
            project_surface(&quarter, &[0.0; 3], 1.0 / 8.0, &mut meter()),
            Ok(Projection::NonIsolated { .. })
        ));
        assert_eq!(
            vertex_on_surface(&quarter, &exact([0.0; 3]), 1.0, &mut meter()).unwrap(),
            Verdict::Unknown
        );
    }

    #[test]
    fn a_bound_that_is_not_a_number_decides_nothing() {
        // The nearest-point search refuses the floor it gives.
        assert_eq!(
            vertex_on_surface(&floor(), &exact([1.0, 1.0, 0.0]), f64::NAN, &mut meter()).unwrap(),
            Verdict::Unknown
        );
        // No piece is within it or beyond it, and a piece whose middle is one of its ends, at
        // either end, cannot be halved: each is left undecided.
        for span in [(1.0, 1.0_f64.next_up()), (1.0_f64.next_down(), 1.0)] {
            let (pcurve, curve) = on_floor((1.0, 1.0), 1.0, span);
            assert_eq!(
                pcurve_on(&floor(), &pcurve, &curve, f64::NAN, &mut meter()).unwrap(),
                Verdict::Unknown,
                "{span:?}"
            );
        }
    }

    #[test]
    fn a_pcurve_that_leaves_its_surfaces_domain_is_undecided() {
        // From (5, 5) to (25, 5): past the patch's u = 10.
        let (pcurve, curve) = on_floor((5.0, 5.0), 20.0, (0.0, 1.0));
        assert_eq!(
            pcurve_on(&floor(), &pcurve, &curve, 1.0, &mut meter()).unwrap(),
            Verdict::Unknown
        );
        // Inside it, the same line holds.
        let (pcurve, curve) = on_floor((1.0, 5.0), 8.0, (0.0, 1.0));
        assert_eq!(
            pcurve_on(&floor(), &pcurve, &curve, 1.0, &mut meter()).unwrap(),
            Verdict::Holds
        );
    }

    #[test]
    fn a_meter_that_stops_inside_a_piece_stops_the_check() {
        let (pcurve, curve) = on_floor((1.0, 5.0), 8.0, (0.0, 1.0));
        let mut short = WorkMeter::new(Budget::new(1, u64::MAX));
        assert!(pcurve_on(&floor(), &pcurve, &curve, 1.0, &mut short).is_err());
    }
}
