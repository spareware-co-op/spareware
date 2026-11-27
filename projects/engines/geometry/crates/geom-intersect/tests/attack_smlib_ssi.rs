//! Attack tests from SMLib's confirmed defects, batch 3, second line: the general search, marching,
//! tangent analysis and seams (`geom-intersect`, pieces G168 to G174).
//!
//! Each test feeds the kernel the input of one defect in solidcore's log of upstream SMLib defects
//! (tangency-labs/solidcore, Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70,
//! `plan/upstream-issues/`, one file per defect, cited by file name at each test). The case is
//! rebuilt from the defect's description in our own API; no fixture is copied. Each test asserts
//! what the kernel owes for that input: every branch found, on both surfaces, traced once and in
//! order, or a defined refusal, and never a panic, a hang, a NaN or a silent wrong answer.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::disallowed_methods,
    clippy::doc_markdown,
    clippy::type_complexity,
    clippy::needless_range_loop,
    clippy::similar_names,
    reason = "tests: native arithmetic and indexing, platform maths on an independent oracle (the surfaces' own equations), prose that names defect files, and a failure stops the test"
)]

use geom_curve::Frame;
use geom_intersect::IntersectError;
use geom_intersect::contact::{Reading, analyse};
use geom_intersect::general::subdivide;
use geom_intersect::march::{Branch, Traced, march};
use geom_intersect::result::{CurveContact, CurveEnd};
use geom_intersect::seam::Passing;
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cylinder, ParamSurface, Plane, Sphere, SurfaceKind};

const FLOOR: f64 = 1e-3;
const CEILING: f64 = 1e-2;
const TURN: f64 = 2.0;

fn meter_of(work: u64) -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(work))
}

fn mid(x: Interval) -> f64 {
    x.lo() / 2.0 + x.hi() / 2.0
}

fn frame() -> Frame<3> {
    Frame::standard().unwrap()
}

/// A plane over a wide patch, its origin off-centre so that no cut falls on a contact.
fn patch(scale: f64) -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0625 * scale, 0.03125 * scale, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-9.0 * scale, 9.0 * scale), (-9.0 * scale, 9.0 * scale)),
        )
        .unwrap(),
    )
}

/// A biquadratic bowl over [-r, r]^2, exact in Bezier form.
fn bowl(r: f64, h: f64, c: f64) -> SurfaceKind {
    let basis = || Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for (i, x) in [-r, 0.0, r].into_iter().enumerate() {
        for (j, y) in [-r, 0.0, r].into_iter().enumerate() {
            let w = |k: usize, t: f64| if k == 1 { -t * t } else { t * t };
            net.push([x, y, h + c * (w(i, r) + w(j, r))]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(basis(), basis(), &net).unwrap())
}

/// Every vertex of a branch on both surfaces, within four floors.
fn on_both(first: &SurfaceKind, second: &SurfaceKind, branch: &Branch, floor: f64) {
    let sides = (Limit::FromRight, Limit::FromRight);
    for [a, b, c, d] in &branch.vertices {
        let p = first.point(mid(*a), mid(*b), sides);
        let q = second.point(mid(*c), mid(*d), sides);
        if let (Ok(p), Ok(q)) = (p, q) {
            let gap = (p[0] - q[0]).hypot(p[1] - q[1]).hypot(p[2] - q[2]);
            assert!(gap < 4.0 * floor, "{gap}");
        }
    }
}

fn trace(first: &SurfaceKind, second: &SurfaceKind, floor: f64, ceiling: f64, work: u64) -> Traced {
    let found = subdivide(first, second, floor, &mut meter_of(work)).unwrap();
    march(first, second, &found, floor, ceiling, &mut meter_of(work)).unwrap()
}

/// solidcore plan/upstream-issues/HANG-25--intersectinsertrelate-does-not-return-for-a-zero.md
/// (b4de2149, confirmed by a C++ run): the topology intersection did not finish in 180 s at an
/// approximation tolerance of +0.0 or -0.0, while negative values and tiny positive ones down to
/// the smallest subnormal returned. The input is the defect's, as each search here takes its
/// tolerance, its floor: 0, -0, -1e-5, -1e300, 1e-12, 1e-200 and 5e-324, and NaN, for the
/// subdivision of two bowls and for the march and the tangent analysis over what it found. A floor
/// that is not positive is refused; a positive one returns or runs out of its budget, and never
/// hangs, panics or answers with an error that is not one of those two.
#[test]
fn hang_25_a_floor_of_zero_or_nothing_is_refused_and_a_tiny_one_stops_at_its_budget() {
    let low = bowl(2.0, -1.0, 1.0);
    let high = bowl(2.0, 1.0, -1.0);
    for floor in [0.0, -0.0, -1e-5, -1e300, f64::NAN] {
        assert_eq!(
            subdivide(&low, &high, floor, &mut meter_of(1 << 16)).unwrap_err(),
            IntersectError::InvalidInput,
            "{floor}"
        );
    }
    let found = subdivide(&low, &high, FLOOR, &mut meter_of(1 << 21)).unwrap();
    for floor in [0.0, -0.0, -1e-5, -1e300, f64::NAN] {
        assert_eq!(
            march(&low, &high, &found, floor, CEILING, &mut meter_of(1 << 16)).unwrap_err(),
            IntersectError::InvalidInput,
            "{floor}"
        );
        assert_eq!(
            analyse(&low, &high, &found, floor, CEILING, &mut meter_of(1 << 16)).unwrap_err(),
            IntersectError::InvalidInput,
            "{floor}"
        );
    }
    for floor in [1e-12, 1e-200, 1e-300, 5e-324, f64::MIN_POSITIVE] {
        match subdivide(&low, &high, floor, &mut meter_of(1 << 16)) {
            Ok(_) | Err(IntersectError::BudgetExceeded) => {}
            Err(e) => panic!("subdivide at {floor}: {e:?}"),
        }
        match march(&low, &high, &found, floor, CEILING, &mut meter_of(1 << 16)) {
            Ok(_) | Err(IntersectError::BudgetExceeded) => {}
            Err(e) => panic!("march at {floor}: {e:?}"),
        }
        match analyse(&low, &high, &found, floor, CEILING, &mut meter_of(1 << 16)) {
            Ok(_) | Err(IntersectError::BudgetExceeded) => {}
            Err(e) => panic!("analyse at {floor}: {e:?}"),
        }
    }
}

/// A piecewise linear ribbon whose height alternates between -1 and 1 at u = 0, 1, ..., n - 1, over
/// v in [0, 1]: it crosses z = 0 along n - 1 straight lines, at u = 1/2, 3/2, and so on.
fn ribbon(n: usize) -> SurfaceKind {
    let mut knots = vec![0.0, 0.0];
    for i in 1..n - 1 {
        knots.push(i as f64);
    }
    knots.extend([(n - 1) as f64; 2]);
    let across = Basis::new(1, knots).unwrap();
    let along = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for i in 0..n {
        for y in [0.0, 1.0] {
            net.push([i as f64, y, if i % 2 == 0 { -1.0 } else { 1.0 }]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(across, along, &net).unwrap())
}

/// solidcore plan/upstream-issues/UB-74--pointbasedsurfaceintersect-writes-past-its-20-point-array.md
/// (b4de2149, not confirmed by a C++ run) and
/// WR-70--smadvsurfaceintersector-solvebranch-never-tests-gauss-map.md (b4de2149, confirmed by a
/// C++ run): the start points of a surface intersection went into a stack array of 20, written past
/// at 20 points, and every intersecting leaf pair of the search gave its own candidate start where
/// the separable pairs should have shared one. The input is the defects' made large: a ribbon that
/// crosses a plane along 24 separate lines, so the search holds at least 48 start points, far past
/// 20. Each line is one branch, found once, from edge to edge, in order, on both surfaces, with no
/// start left over and nothing flagged.
#[test]
fn ub_74_wr_70_a_surface_crossing_a_plane_in_twenty_four_lines_has_twenty_four_branches() {
    let n = 25;
    let sheet = ribbon(n);
    // A plane over all of the ribbon (x from 0 to 24, y from 0 to 1), its origin off-centre.
    let flat = SurfaceKind::Plane(
        Plane::new(
            [0.0625, 0.03125, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-2.0, 26.0), (-2.0, 3.0)),
        )
        .unwrap(),
    );
    let found = subdivide(&sheet, &flat, FLOOR, &mut meter_of(1 << 26)).unwrap();
    let starts: usize = found.settled.iter().map(|p| p.starts.len()).sum();
    assert!(starts >= 48, "{starts}");
    assert!(found.flagged.is_empty(), "{:?}", found.flagged.len());
    let traced = march(
        &sheet,
        &flat,
        &found,
        FLOOR,
        CEILING,
        &mut meter_of(1 << 26),
    )
    .unwrap();
    assert!(traced.unproven.is_empty(), "{}", traced.unproven.len());
    assert_eq!(traced.branches.len(), n - 1, "{}", traced.branches.len());
    let mut across: Vec<f64> = Vec::new();
    for branch in &traced.branches {
        assert_eq!(branch.ends, [CurveEnd::Boundary, CurveEnd::Boundary]);
        on_both(&sheet, &flat, branch, FLOOR);
        let at: Vec<f64> = branch.vertices.iter().map(|v| mid(v[0])).collect();
        let (lo, hi) = at
            .iter()
            .fold((f64::MAX, f64::MIN), |(l, h), x| (l.min(*x), h.max(*x)));
        assert!(
            hi - lo < 4.0 * FLOOR,
            "a branch drifts across u: {lo} to {hi}"
        );
        across.push(f64::midpoint(lo, hi));
    }
    across.sort_by(f64::total_cmp);
    for (i, u) in across.iter().enumerate() {
        assert!((u - (i as f64 + 0.5)).abs() < 4.0 * FLOOR, "{i}: {u}");
    }
}

/// An angle difference wrapped to (-pi, pi].
fn wrap(d: f64) -> f64 {
    d - core::f64::consts::TAU * (d / core::f64::consts::TAU).round()
}

/// solidcore plan/upstream-issues/WR-503--filletintersector-computenextpoint-accepts-a-point-far-short-of-the-step.md
/// (b4de2149, confirmed in the Rust port only): the trace accepted a next point far short of its
/// step, on another root a few millimetres on, and the rail doubled back; the case was a knife
/// edge, since the same plate scaled by 0.5 or 2 gave the right answer. The input is the defect's
/// shape in our API: a trace that runs to the edge of a domain, a circle cut from a sphere by a
/// plane with the sphere's domain short of a whole turn, at the scales 1/2, 1 and 2 with the floor
/// and the ceiling scaled with it. Each scale has one branch from edge to edge that goes round the
/// circle one way without stepping back, on both surfaces, and nothing unproven.
#[test]
fn wr_503_a_trace_to_the_edge_goes_one_way_at_any_scale() {
    for scale in [0.5, 1.0, 2.0] {
        let plane = patch(scale);
        let sphere = SurfaceKind::Sphere(
            Sphere::new(
                [scale, 2.0 * scale, 3.0 * scale],
                5.0 * scale,
                frame(),
                ((0.0625, TURN), (-0.49, 0.49)),
            )
            .unwrap(),
        );
        let (floor, ceiling) = (FLOOR * scale, CEILING * scale);
        let traced = trace(&plane, &sphere, floor, ceiling, 1 << 22);
        assert!(
            traced.unproven.is_empty(),
            "{scale}: {}",
            traced.unproven.len()
        );
        assert_eq!(
            traced.branches.len(),
            1,
            "{scale}: {}",
            traced.branches.len()
        );
        let branch = &traced.branches[0];
        assert_eq!(
            branch.ends,
            [CurveEnd::Boundary, CurveEnd::Boundary],
            "{scale}"
        );
        on_both(&plane, &sphere, branch, floor);
        // The circle is centred at (scale, 2 scale) with radius 4 scale, in the plane's parameters
        // shifted by the plane's origin.
        let angles: Vec<f64> = branch
            .vertices
            .iter()
            .map(|[u, v, ..]| {
                let x = mid(*u) + 0.0625 * scale - scale;
                let y = mid(*v) + 0.03125 * scale - 2.0 * scale;
                assert!(
                    (x.hypot(y) - 4.0 * scale).abs() < 4.0 * floor,
                    "{scale}: {x} {y}"
                );
                y.atan2(x)
            })
            .collect();
        let steps: Vec<f64> = angles.windows(2).map(|w| wrap(w[1] - w[0])).collect();
        assert!(
            steps.iter().all(|d| *d > 0.0) || steps.iter().all(|d| *d < 0.0),
            "{scale}: the branch turns back: {steps:?}"
        );
    }
}

/// A cylinder of radius 5 units along y, resting on z = 0 (as the tangent analysis's own tests).
const UNIT: f64 = 1024.0;

fn resting() -> SurfaceKind {
    SurfaceKind::Cylinder(
        Cylinder::new(
            [0.0, 0.0, 5.0 * UNIT],
            5.0 * UNIT,
            Frame::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap(),
            ((0.1, 0.9), (-UNIT, UNIT)),
        )
        .unwrap(),
    )
}

fn level(z: f64) -> SurfaceKind {
    let r = 6.0 * UNIT;
    SurfaceKind::Plane(
        Plane::new(
            [64.0, 32.0, z],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-2.0 * r, 2.0 * r), (-2.0 * r, 2.0 * r)),
        )
        .unwrap(),
    )
}

/// solidcore plan/upstream-issues/WR-510--cone-intersectwithplane-tangent-within-a-faces-zone-tolerance.md
/// (b4de2149, not confirmed by a C++ run): a plane a hair inside a cylinder's wall was read as two
/// crossing lines 1.5e-5 apart, not as a touch along one line, and the union failed or left a
/// sliver. The input is the defect's, on the general path: a cylinder of radius 5120 resting on a
/// plane, with the plane moved 1.7e-8 into the cylinder, 1.7e-8 away, 1e-300 into it, and on it
/// (depths whose chord is 1e-4 of the floor, far below it). Where the two lines cannot be told
/// apart at the floor the reading is one tangent curve in a band, or a reading that says it is
/// undecided: never nothing for a plane that touches or dips in, never a point, never an overlap.
/// A plane 1.7e-8 clear of the wall is apart, which the search may prove, and then nothing is
/// right.
#[test]
fn wr_510_a_plane_a_hair_inside_a_wall_reads_as_a_touch_not_as_nothing() {
    let cylinder = resting();
    for z in [0.0, 1.7e-8, -1.7e-8, 1e-300] {
        let plane = level(z);
        let found = subdivide(&cylinder, &plane, 8.0, &mut meter_of(1 << 23)).unwrap();
        let readings =
            analyse(&cylinder, &plane, &found, 8.0, UNIT, &mut meter_of(1 << 23)).unwrap();
        if z < 0.0 {
            // A plane 1.7e-8 clear of the wall is apart, and the search may prove it: nothing is
            // then the right answer. If it does not, the reading must still be a tangent or undecided.
            assert!(readings.len() <= 1, "{z}: {readings:?}");
        } else {
            assert_eq!(readings.len(), 1, "{z}: {readings:?}");
        }
        for reading in &readings {
            assert!(
                matches!(
                    reading,
                    Reading::Curve {
                        contact: CurveContact::Tangent { .. },
                        ..
                    } | Reading::Undecided { .. }
                ),
                "{z}: {readings:?}"
            );
        }
    }
}

/// solidcore plan/upstream-issues/WR-491--createorientedtrimboundary-fills-a-seam-crossing-uv-gap-with-a-line-across-the-domain.md
/// (b4de2149, confirmed by a C++ run) and
/// WR-537--point-on-a-closed-face-seam-classifies-unknown.md (b4de2149, not confirmed by a C++
/// run): a trim boundary that crosses a closed face's seam was closed with a straight line across
/// the whole parameter domain, and a point on the seam was classified unknown. The input is the
/// defects': a plane tilted across a whole-turn cylinder, so the cut is one ellipse that crosses
/// the seam once, at a point of the seam itself. It is one closed branch that records the seam
/// once, whose neighbouring vertices, the last to the first included, are a short step apart in
/// space and never a diameter, on both surfaces.
#[test]
fn wr_491_537_a_curve_through_a_closed_face_s_seam_is_one_loop_with_the_seam_recorded_once() {
    let cylinder = SurfaceKind::Cylinder(
        Cylinder::new(
            [0.0; 3],
            2.0,
            Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap(),
            ((0.0, TURN), (-4.0, 4.0)),
        )
        .unwrap(),
    );
    let tilted = SurfaceKind::Plane(
        Plane::new(
            [0.0625, 0.03125, 0.0],
            [1.0, 0.0, 0.25],
            [0.0, 1.0, 0.0],
            ((-9.0, 9.0), (-9.0, 9.0)),
        )
        .unwrap(),
    );
    let traced = trace(&cylinder, &tilted, FLOOR, CEILING, 1 << 24);
    assert!(traced.unproven.is_empty(), "{}", traced.unproven.len());
    assert_eq!(traced.branches.len(), 1, "{}", traced.branches.len());
    let branch = &traced.branches[0];
    assert_eq!(branch.ends, [CurveEnd::Closed, CurveEnd::Closed]);
    on_both(&cylinder, &tilted, branch, FLOOR);
    let seams: Vec<_> = branch
        .passes
        .iter()
        .filter(|p| p.passing == Passing::Seam)
        .collect();
    assert_eq!(seams.len(), 1, "{:?}", branch.passes);
    let sides = (Limit::FromRight, Limit::FromRight);
    let points: Vec<[f64; 3]> = branch
        .vertices
        .iter()
        .map(|[u, v, ..]| cylinder.point(mid(*u), mid(*v), sides).unwrap())
        .collect();
    assert!(points.len() >= 8, "{}", points.len());
    for k in 0..points.len() {
        let (p, q) = (points[k], points[(k + 1) % points.len()]);
        let step = (p[0] - q[0]).hypot(p[1] - q[1]).hypot(p[2] - q[2]);
        assert!(step > 1e-9 && step < 1.0, "{k}: {step}");
    }
}
