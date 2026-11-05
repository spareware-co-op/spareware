//! Attack tests from SMLib's confirmed defects, projection and inversion (G162): `geom-intersect`.
//!
//! Each test feeds the kernel the input of one defect in solidcore's log of upstream SMLib
//! defects (tangency-labs/solidcore, Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70,
//! `plan/upstream-issues/`, one file per defect, cited by file name at each test). The case is
//! rebuilt from the defect's description in our own API; no fixture is copied. Each test asserts
//! what the kernel owes for that input: a nearest point whose enclosures hold the exact answer,
//! or a defined refusal, and never a panic, a hang, a NaN or a wrong value. A hang is caught with
//! a wall-clock limit around the call, on top of the meter every search already carries.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::disallowed_methods,
    clippy::doc_markdown,
    clippy::manual_midpoint,
    clippy::type_complexity,
    clippy::redundant_closure_for_method_calls,
    clippy::assert_is_empty,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::needless_range_loop,
    clippy::range_minus_one,
    reason = "tests: native arithmetic, casts and indexing, platform maths on an independent oracle (the formula as it is written), prose that names defect files, and a failure stops the test"
)]

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use core::f64::consts::PI;

use geom_curve::conic::Circle;
use geom_curve::line::Line;
use geom_curve::{CurveKind, Frame};
use geom_intersect::{
    Continuum, IntersectError, Place, Projection, invert_curve, invert_surface, project_curve,
    project_surface,
};
use geom_nurbs::{Basis, Curve, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, Sphere, SurfaceKind};

/// A meter above what any case here takes, so a search that loses its way fails, not hangs.
fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 22))
}

/// A meter for the surface with four stacked runs, whose search is longer.
fn long_meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 26))
}

const FLOOR: f64 = 1e-6;

/// Runs `work` on its own thread and fails the test if it panics or has not finished in
/// `seconds`: a hang is a failure, not a stuck test run.
fn within<T: Send + 'static>(seconds: u64, work: impl FnOnce() -> T + Send + 'static) -> T {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(work());
    });
    match receiver.recv_timeout(Duration::from_secs(seconds)) {
        Ok(done) => done,
        Err(mpsc::RecvTimeoutError::Timeout) => panic!("no answer in {seconds} s: a hang"),
        Err(mpsc::RecvTimeoutError::Disconnected) => panic!("the call panicked"),
    }
}

/// Whether a refusal is one the kernel documents: a limit, a range or a precondition, and not a
/// failed proof (`Internal`, which is a bug) or a cancel nobody asked for.
fn documented(e: IntersectError) -> bool {
    matches!(
        e,
        IntersectError::FrameRange
            | IntersectError::UnprovenMaths
            | IntersectError::InvalidInput
            | IntersectError::BudgetExceeded
    )
}

/// What a projection owes when the nearest distance is `want`: a defined refusal, an undecided
/// box, or nearest points every one of whose distance enclosures lies within `slack` of `want`.
fn right_or_refused<const N: usize, const D: usize>(
    found: &Result<Projection<N, D>, IntersectError>,
    want: f64,
    slack: f64,
    what: &str,
) {
    let near = |d: geom_num::interval::Interval| d.lo() - slack <= want && want <= d.hi() + slack;
    match found {
        Err(e) => assert!(documented(*e), "{what}: {e:?}"),
        Ok(Projection::Points(points)) => {
            assert!(!points.is_empty(), "{what}: no nearest point");
            for n in points {
                assert!(near(n.distance), "{what}: {n:?}");
            }
        }
        Ok(Projection::Set { distance, .. }) => assert!(near(*distance), "{what}: {distance:?}"),
        Ok(Projection::NonIsolated { .. }) => {}
    }
}

fn arc(centre: [f64; 2], radius: f64, domain: (f64, f64)) -> CurveKind<2> {
    CurveKind::Circle(Circle::new(centre, radius, Frame::standard().unwrap(), domain).unwrap())
}

/// The point `rho` from the centre (1, 2), at angle `a` half-turns.
fn near_centre(rho: f64, a: f64) -> [f64; 2] {
    let angle = a * PI;
    [1.0 + rho * angle.cos(), 2.0 + rho * angle.sin()]
}

/// solidcore plan/upstream-issues/WR-740--point-curve-newton-stalls-near-a-centre-of-curvature.md
/// (b4de2149): SMLib's closest-point Newton step on an arc was damped so hard near its centre of
/// curvature that no answer came from within about a hundredth of the radius of the centre,
/// though the answer is unique, the arc's point at the test point's angle. The inputs are the
/// defect's: the R5 quarter arc, and test points 0.1, 0.05, 0.01, 0.001, 1e-4 and 1e-6 from its
/// centre. The foot must be at the test point's angle, `5 - rho` away, a foot and not an end;
/// no point returned may be farther than that; and a point beyond the arc's angle has the
/// nearer end, not a made-up foot. A point exactly at the centre is the typed set.
#[test]
fn wr_740_a_point_near_an_arcs_centre_has_its_foot_at_the_points_angle() {
    let quarter = arc([1.0, 2.0], 5.0, (0.0, 0.5));
    for rho in [0.1, 0.05, 1e-2, 1e-3, 1e-4, 1e-6] {
        for a in [0.05, 0.25, 0.45] {
            let p = near_centre(rho, a);
            let found = project_curve(&quarter, &p, 1e-9, &mut meter()).unwrap();
            let Projection::Points(points) = &found else {
                panic!("rho {rho} a {a}: {found:?}");
            };
            let want = 5.0 - rho;
            let foot = points
                .iter()
                .find(|n| n.at[0].lo() - 1e-7 <= a && a <= n.at[0].hi() + 1e-7)
                .unwrap_or_else(|| panic!("rho {rho} a {a}: no foot at the angle: {points:?}"));
            assert_eq!(foot.place, Place::Foot, "rho {rho} a {a}: {foot:?}");
            assert!(
                foot.distance.lo() - 1e-8 <= want && want <= foot.distance.hi() + 1e-8,
                "rho {rho} a {a}: {foot:?}"
            );
            for n in points {
                assert!(
                    n.distance.lo() <= want + 1e-8,
                    "rho {rho} a {a}: a farther point {n:?}"
                );
            }
        }
    }
    // A test point whose angle is past the arc's start: the start is nearest, on the boundary.
    for rho in [1e-2, 1e-3, 1e-6] {
        let p = near_centre(rho, -0.5);
        let found = project_curve(&quarter, &p, 1e-9, &mut meter()).unwrap();
        let end = found
            .first()
            .unwrap_or_else(|| panic!("rho {rho}: {found:?}"));
        assert_eq!(end.place, Place::Boundary, "rho {rho}: {end:?}");
        assert!(end.at[0].contains(0.0), "rho {rho}: {end:?}");
        let want = (25.0_f64 + rho * rho).sqrt();
        assert!(
            end.distance.lo() - 1e-8 <= want && want <= end.distance.hi() + 1e-8,
            "rho {rho}: {end:?}"
        );
    }
    // The centre itself: every point of the arc is nearest, and the answer says so.
    let found = project_curve(&quarter, &[1.0, 2.0], 1e-9, &mut meter()).unwrap();
    let Projection::Set { set, distance } = found else {
        panic!("{found:?}");
    };
    assert_eq!(set, Continuum::Curve);
    assert!(distance.contains(5.0), "{distance:?}");
}

/// The same defect on a B-spline, where it was found (the rounded box's bottom loop was one): the
/// rational quarter circle of radius 0.5 about the origin, and test points 0.1 to 1e-5 from its
/// centre at angles whose feet are not on a knot. The foot's point must be the arc's point at the
/// test point's angle. Very near the centre the bounds of a spline may not separate the feet
/// before the floor; the search then owes the undecided box, never a wrong foot.
#[test]
fn wr_740_the_same_on_a_rational_spline_arc() {
    let basis = Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let points = [[0.5, 0.0], [0.5, 0.5], [0.0, 0.5]];
    let weights = [1.0, core::f64::consts::FRAC_1_SQRT_2, 1.0];
    let spline = CurveKind::Nurbs(Curve::new_rational(basis, &points, &weights).unwrap());
    for rho in [0.1, 1e-2, 1e-3, 1e-5] {
        for a in [PI / 8.0, PI / 4.0, 3.0 * PI / 8.0] {
            let p = [rho * a.cos(), rho * a.sin()];
            let foot_at = [0.5 * a.cos(), 0.5 * a.sin()];
            let want = 0.5 - rho;
            let found = project_curve(&spline, &p, 1e-9, &mut meter()).unwrap();
            match &found {
                Projection::Points(found_points) => {
                    let foot = found_points
                        .iter()
                        .find(|n| {
                            (0..2).all(|d| {
                                n.point[d].lo() - 1e-6 <= foot_at[d]
                                    && foot_at[d] <= n.point[d].hi() + 1e-6
                            })
                        })
                        .unwrap_or_else(|| panic!("rho {rho} a {a}: no foot: {found_points:?}"));
                    assert!(
                        foot.distance.lo() - 1e-6 <= want && want <= foot.distance.hi() + 1e-6,
                        "rho {rho} a {a}: {foot:?}"
                    );
                    for n in found_points {
                        assert!(
                            n.distance.lo() <= want + 1e-6,
                            "rho {rho} a {a}: a farther point {n:?}"
                        );
                    }
                }
                Projection::NonIsolated { .. } => {
                    assert!(rho < 1e-2, "rho {rho} a {a}: undecided too far out");
                }
                Projection::Set { .. } => panic!("rho {rho} a {a}: {found:?}"),
            }
        }
    }
}

/// A rational strip of four runs stacked 5 apart, the runs 40 long and joined by risers: the
/// control net of a coil's wall, whose control points are far from the next turn.
fn stacked_runs() -> SurfaceKind {
    let u = Basis::new(1, vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 7.0]).unwrap();
    let v = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    // (x, z) of the net's rows: run 1 out along x at z 0, a riser up at x 40, run 2 back at
    // z 5, a riser at x 0, and so on to z 15.
    let rows = [
        (0.0, 0.0),
        (40.0, 0.0),
        (40.0, 5.0),
        (0.0, 5.0),
        (0.0, 10.0),
        (40.0, 10.0),
        (40.0, 15.0),
        (0.0, 15.0),
    ];
    let mut net = Vec::new();
    let mut weights = Vec::new();
    for (k, (x, z)) in rows.into_iter().enumerate() {
        let w = if k % 2 == 0 { 1.0 } else { 3.0 };
        for y in [0.0, 10.0] {
            net.push([x, y, z]);
            weights.push(w);
        }
    }
    SurfaceKind::Nurbs(Surface::new_rational(u, v, &net, &weights).unwrap())
}

/// solidcore plan/upstream-issues/WR-444--a-point-solve-on-a-rational-surface-drops-a-good-control-net.md
/// (b4de2149): on a rational surface SMLib's point solve took the control point nearest the test
/// point as its start, and when the control net was coarser than the gap between two parts of the
/// surface the nearest control point was on the wrong part and the solve found nothing (a coil's
/// wall, turns 5 apart, control points about 16 apart along a turn). The input is the defect's
/// shape: a rational strip of four runs stacked 5 apart with control points 40 apart along a run.
/// A point on a run must invert to that run, and a point 1.5 above a run has its foot on that
/// run, not on the next one 3.5 above it.
#[test]
fn wr_444_a_point_on_a_stacked_rational_run_is_found_on_that_run() {
    let surface = stacked_runs();
    for k in 0..4 {
        let z = 5.0 * f64::from(k);
        for x in [5.0, 35.0] {
            for y in [2.0, 8.0] {
                let on = [x, y, z];
                let found = invert_surface(&surface, &on, 1e-5, FLOOR, &mut long_meter()).unwrap();
                let foot = found.unwrap_or_else(|| panic!("run {k} at {on:?}: not found"));
                for d in 0..3 {
                    assert!(
                        foot.point[d].lo() - 1e-5 <= on[d] && on[d] <= foot.point[d].hi() + 1e-5,
                        "run {k} at {on:?}: {foot:?}"
                    );
                }
                let above = [x, y, z + 1.5];
                let found = project_surface(&surface, &above, FLOOR, &mut long_meter()).unwrap();
                let Projection::Points(points) = &found else {
                    panic!("run {k} above {above:?}: {found:?}");
                };
                let foot = points
                    .iter()
                    .find(|n| {
                        (0..3).all(|d| {
                            n.point[d].lo() - 1e-5 <= on[d] && on[d] <= n.point[d].hi() + 1e-5
                        })
                    })
                    .unwrap_or_else(|| panic!("run {k} above {above:?}: no foot: {points:?}"));
                assert!(
                    foot.distance.lo() - 1e-5 <= 1.5 && 1.5 <= foot.distance.hi() + 1e-5,
                    "run {k} above {above:?}: {foot:?}"
                );
                for n in points {
                    assert!(
                        n.distance.lo() <= 1.5 + 1e-5,
                        "run {k} above {above:?}: a farther point {n:?}"
                    );
                }
            }
        }
    }
}

/// The 2 by 2 bilinear net on the unit square at height `z` over its fourth corner, with the
/// weights `w` row by row, its coordinates scaled by `scale`; `None` if the surface is refused.
fn square(scale: f64, z: f64, w: [f64; 4]) -> Option<SurfaceKind> {
    let basis = || Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let net = [
        [0.0, 0.0, 0.0],
        [0.0, scale, 0.0],
        [scale, 0.0, 0.0],
        [scale, scale, z],
    ];
    Surface::new_rational(basis(), basis(), &net, &w)
        .ok()
        .map(SurfaceKind::Nurbs)
}

/// solidcore plan/upstream-issues/HANG-50--smsurfacepointsolve-smsurfacecache-subdivision-does-not.md
/// (b4de2149): SMLib's surface point solve did not return within 20 s on a B-spline surface whose
/// net held weights near 1e308 and control points near 1e-6. The inputs are the defect's: a
/// bilinear net of points 1e-6 apart, with every weight 1e308 (the same surface as with unit
/// weights), with 1e308 on two corners and 1 on the others, and with `f64::MAX`, and a unit-weight
/// control. The net lies in a plane, so for any positive weights the surface is the whole square
/// and the point 2e-7 above its middle is 2e-7 from it. Each call must finish within the time
/// limit with that distance or a defined refusal.
#[test]
fn hang_50_huge_weights_on_a_tiny_net_end_with_the_right_distance_or_a_refusal() {
    let sets = [
        [1.0; 4],
        [1e308; 4],
        [1e308, 1.0, 1.0, 1e308],
        [1.0, 1e308, 1e308, 1.0],
        [f64::MAX; 4],
        [f64::MAX, 1.0, 1.0, 1.0],
    ];
    for w in sets {
        let found = within(120, move || {
            let surface = square(1e-6, 0.0, w)?;
            Some(project_surface(
                &surface,
                &[5e-7, 5e-7, 2e-7],
                1e-12,
                &mut meter(),
            ))
        });
        // A net refused at construction is a defined refusal.
        if let Some(found) = found {
            right_or_refused(&found, 2e-7, 1e-13, &format!("weights {w:?}"));
        }
    }
}

/// solidcore plan/upstream-issues/HANG-20--the-global-solvers-never-finish-when-a-surface-coordinate-s.md
/// (b4de2149): SMLib's solvers never finished once a surface coordinate was in the top decade of
/// the double range (it returned at -7e307 and never at -8e307, -1.7e308 or the largest double).
/// The inputs are the defect's coordinates: the unit square's fourth corner raised to each, as a
/// bilinear surface, as a two-point NURBS curve, as a plane through a vector with that
/// component, and as a line segment. The point is (0.2, 0.3, 0) and (0.5, 0, 0): the nearest
/// point of each is at the start of the surface or curve, 0.2 and 0.5 away, since a step along
/// the huge direction costs more than it gains. Each call must finish in the time limit with that
/// distance or a defined refusal.
#[test]
fn hang_20_a_coordinate_in_the_top_decade_ends_with_the_right_distance_or_a_refusal() {
    let zs = [
        1e300,
        -1e300,
        7e307,
        -8e307,
        1.7e308,
        -1.7e308,
        f64::MAX,
        f64::MIN,
    ];
    for z in zs {
        let found = within(120, move || {
            square(1.0, z, [1.0; 4])
                .map(|s| project_surface(&s, &[0.2, 0.3, 0.0], FLOOR, &mut meter()))
        });
        if let Some(found) = found {
            right_or_refused(&found, 0.2, 1e-9, &format!("surface {z}"));
        }
        let found = within(120, move || {
            let basis = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
            let curve = Curve::new(basis, &[[0.0, 0.0, 0.0], [1.0, 0.0, z]]).ok()?;
            Some(project_curve(
                &CurveKind::Nurbs(curve),
                &[0.5, 0.0, 0.0],
                FLOOR,
                &mut meter(),
            ))
        });
        if let Some(found) = found {
            right_or_refused(&found, 0.5, 1e-9, &format!("curve {z}"));
        }
        let found = within(120, move || {
            let plane = Plane::new(
                [0.0; 3],
                [1.0, 0.0, z],
                [0.0, 1.0, 0.0],
                ((0.0, 1.0), (0.0, 1.0)),
            )
            .ok()?;
            Some(project_surface(
                &SurfaceKind::Plane(plane),
                &[0.2, 0.3, 0.0],
                FLOOR,
                &mut meter(),
            ))
        });
        if let Some(found) = found {
            right_or_refused(&found, 0.2, 1e-9, &format!("plane {z}"));
        }
        let found = within(120, move || {
            let segment = Line::segment([0.0, 0.0, 0.0], [1.0, 0.0, z]).ok()?;
            Some(project_curve(
                &CurveKind::Line(segment.value),
                &[0.5, 0.0, 0.0],
                FLOOR,
                &mut meter(),
            ))
        });
        if let Some(found) = found {
            right_or_refused(&found, 0.5, 1e-9, &format!("line {z}"));
        }
    }
}

/// solidcore plan/upstream-issues/HANG-32--smbrep-createparallelprojectioncurves-spins-on-a-subnormal.md
/// (b4de2149), reported from a C++ run and not re-run in the log: a projection with an
/// approximation tolerance of 5e-324, the smallest subnormal, ran for more than 15 minutes while
/// 1e-100 returned at once. The floor of a search is our tolerance. The inputs are the defect's
/// and its neighbours: every floor from 5e-324 and the smallest normal double up to the largest
/// double and infinity, on an arc, a line segment and a sphere patch, each finishing in the time
/// limit with the true distance or a defined refusal; for the arc the foot must also hold the
/// point's angle. A floor that is not positive, `-0.0`, a negative one, negative infinity or NaN
/// is refused as `InvalidInput`, by projection and by inversion, on a curve and on a surface.
#[test]
fn hang_32_a_subnormal_or_extreme_floor_ends_and_a_bad_one_is_refused() {
    let floors = [
        5e-324,
        1e-320,
        f64::MIN_POSITIVE,
        1e-300,
        1e-100,
        1e100,
        f64::MAX,
        f64::INFINITY,
    ];
    let angle = 8.0_f64.atan2(6.0) / PI;
    for floor in floors {
        let found = within(120, move || {
            project_curve(
                &arc([1.0, 2.0], 5.0, (0.0, 1.5)),
                &[7.0, 10.0],
                floor,
                &mut meter(),
            )
        });
        right_or_refused(&found, 5.0, 1e-6, &format!("arc, floor {floor:e}"));
        if let Ok(Projection::Points(points)) = &found {
            assert!(
                points
                    .iter()
                    .any(|n| n.at[0].lo() - 1e-6 <= angle && angle <= n.at[0].hi() + 1e-6),
                "arc, floor {floor:e}: no foot at the angle: {points:?}"
            );
        }
        let found = within(120, move || {
            let segment = Line::segment([0.0, 0.0], [10.0, 0.0]).unwrap();
            project_curve(
                &CurveKind::Line(segment.value),
                &[3.0, 4.0],
                floor,
                &mut meter(),
            )
        });
        right_or_refused(&found, 4.0, 1e-6, &format!("line, floor {floor:e}"));
        let found = within(120, move || {
            let frame = Frame::standard().unwrap();
            let sphere = Sphere::new([0.0; 3], 4.0, frame, ((0.1, 1.9), (-0.3, 0.3))).unwrap();
            project_surface(
                &SurfaceKind::Sphere(sphere),
                &[1.0, 2.0, 0.5],
                floor,
                &mut meter(),
            )
        });
        right_or_refused(
            &found,
            4.0 - 5.25_f64.sqrt(),
            1e-6,
            &format!("sphere, floor {floor:e}"),
        );
    }
    let curve = arc([1.0, 2.0], 5.0, (0.0, 1.5));
    let frame = Frame::standard().unwrap();
    let sphere =
        SurfaceKind::Sphere(Sphere::new([0.0; 3], 4.0, frame, ((0.1, 1.9), (-0.3, 0.3))).unwrap());
    for floor in [0.0, -0.0, -1.0, -5e-324, f64::NEG_INFINITY, f64::NAN] {
        assert_eq!(
            project_curve(&curve, &[7.0, 10.0], floor, &mut meter()).unwrap_err(),
            IntersectError::InvalidInput,
            "{floor}"
        );
        assert_eq!(
            invert_curve(&curve, &[7.0, 10.0], 1e-5, floor, &mut meter()).unwrap_err(),
            IntersectError::InvalidInput,
            "{floor}"
        );
        assert_eq!(
            project_surface(&sphere, &[1.0, 2.0, 0.5], floor, &mut meter()).unwrap_err(),
            IntersectError::InvalidInput,
            "{floor}"
        );
        assert_eq!(
            invert_surface(&sphere, &[1.0, 2.0, 0.5], 1e-5, floor, &mut meter()).unwrap_err(),
            IntersectError::InvalidInput,
            "{floor}"
        );
    }
}

/// solidcore plan/upstream-issues/UB-169--smhcr-crviewvector-builds-the-view-transform-from.md
/// with its addendum UB-169--addendum-2026-11-01-sm-api-projection-overflowing-normal.md
/// (b4de2149): a vector whose squared length overflows (a component of 1e200 or 1e300) was
/// unitised to zero and the code after it read uninitialised memory. Here the vector is the gap
/// between the point and the kind, and the input is the defect's: a point with a coordinate of
/// 1e150, 1e155 (only its square overflows), 1e300 and the largest double, against a circle, a
/// segment, a plane and a sphere. The nearest distance is the point's distance from the kind,
/// within a relative millionth; each call must give it or a defined refusal, and an inversion of
/// the far point must not claim the point is on the kind.
#[test]
fn ub_169_a_gap_whose_square_overflows_gives_the_distance_or_a_refusal() {
    for scale in [1e150, 1e155, 1e300, f64::MAX] {
        let slack = scale * 1e-6;
        let found = project_curve(
            &arc([0.0, 0.0], 5.0, (0.0, 2.0)),
            &[scale, 0.0],
            FLOOR,
            &mut meter(),
        );
        right_or_refused(&found, scale, slack, &format!("circle, {scale:e}"));
        let inverted = invert_curve(
            &arc([0.0, 0.0], 5.0, (0.0, 2.0)),
            &[scale, 0.0],
            1e-5,
            FLOOR,
            &mut meter(),
        );
        assert!(
            !matches!(inverted, Ok(Some(_))),
            "circle, {scale:e}: {inverted:?}"
        );
        let segment = CurveKind::Line(Line::segment([0.0, 0.0], [10.0, 0.0]).unwrap().value);
        let found = project_curve(&segment, &[5.0, scale], FLOOR, &mut meter());
        right_or_refused(&found, scale, slack, &format!("segment, {scale:e}"));
        let plane = SurfaceKind::Plane(
            Plane::new(
                [0.0; 3],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                ((-1.0, 1.0), (-1.0, 1.0)),
            )
            .unwrap(),
        );
        let found = project_surface(&plane, &[0.0, 0.0, scale], FLOOR, &mut meter());
        right_or_refused(&found, scale, slack, &format!("plane, {scale:e}"));
        let inverted = invert_surface(&plane, &[0.0, 0.0, scale], 1e-5, FLOOR, &mut meter());
        assert!(
            !matches!(inverted, Ok(Some(_))),
            "plane, {scale:e}: {inverted:?}"
        );
        let frame = Frame::standard().unwrap();
        let sphere = SurfaceKind::Sphere(
            Sphere::new([0.0; 3], 4.0, frame, ((0.0, 2.0), (-0.5, 0.5))).unwrap(),
        );
        let found = project_surface(&sphere, &[0.0, scale, 0.0], FLOOR, &mut meter());
        right_or_refused(&found, scale, slack, &format!("sphere, {scale:e}"));
    }
}

/// solidcore plan/upstream-issues/HANG-12--droppoint-never-returns-on-a-curve-reparameterised-to-a-zero.md
/// (b4de2149): SMLib's point drop never returned on a line reparameterised to a zero-length
/// interval, a curve whose every evaluation is one point. Our line refuses a zero-length domain
/// and a zero-length segment at construction. The collapsed curves we can build are the
/// ones the defect describes: a NURBS curve and a NURBS surface whose control points are all one
/// point, and a circle over a zero-length domain if the circle accepts one. Each must finish in the
/// time limit with the distance to that one point or a defined refusal, and a curve or surface
/// that is a point must not be reported as having a foot anywhere else.
#[test]
fn hang_12_a_curve_collapsed_to_a_point_ends_with_that_point_or_a_refusal() {
    assert!(Line::segment([1.0, 2.0], [1.0, 2.0]).is_err());
    assert!(Line::new([1.0, 2.0], [1.0, 0.0], (0.5, 0.5)).is_err());
    assert!(Line::new([1.0, 2.0], [0.0, 0.0], (0.0, 1.0)).is_err());

    let found = within(120, || {
        let basis = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
        let curve = Curve::new(basis, &[[2.0, 3.0], [2.0, 3.0]]).unwrap();
        project_curve(&CurveKind::Nurbs(curve), &[5.0, 7.0], FLOOR, &mut meter())
    });
    right_or_refused(&found, 5.0, 1e-5, "a collapsed curve");
    // It is one point, so the search owes a nearest point, not an undecided box.
    let Ok(Projection::Points(points)) = &found else {
        panic!("a collapsed curve: {found:?}");
    };
    for n in points {
        assert!(
            n.point[0].contains(2.0) || (n.point[0].lo() - 2.0).abs() < 1e-5,
            "{n:?}"
        );
        assert!(
            n.point[1].contains(3.0) || (n.point[1].lo() - 3.0).abs() < 1e-5,
            "{n:?}"
        );
    }

    let found = within(120, || {
        let basis = || Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
        let net = [[2.0, 3.0, 1.0]; 4];
        let surface = Surface::new(basis(), basis(), &net).unwrap();
        project_surface(
            &SurfaceKind::Nurbs(surface),
            &[5.0, 7.0, 1.0],
            FLOOR,
            &mut meter(),
        )
    });
    right_or_refused(&found, 5.0, 1e-5, "a collapsed surface");
    let Ok(Projection::Points(points)) = &found else {
        panic!("a collapsed surface: {found:?}");
    };
    for n in points {
        assert!(
            (n.point[0].lo() - 2.0).abs() < 1e-5 || n.point[0].contains(2.0),
            "{n:?}"
        );
        assert!(
            (n.point[1].lo() - 3.0).abs() < 1e-5 || n.point[1].contains(3.0),
            "{n:?}"
        );
    }

    let found = within(120, || {
        let arc = Circle::new([0.0, 0.0], 5.0, Frame::standard().unwrap(), (0.3, 0.3)).ok()?;
        Some(project_curve(
            &CurveKind::Circle(arc),
            &[10.0, 0.0],
            FLOOR,
            &mut meter(),
        ))
    });
    if let Some(found) = found {
        let q = [5.0 * (0.3 * PI).cos(), 5.0 * (0.3 * PI).sin()];
        let want = (10.0 - q[0]).hypot(q[1]);
        right_or_refused(&found, want, 1e-5, "a circle of no length");
    }
}
