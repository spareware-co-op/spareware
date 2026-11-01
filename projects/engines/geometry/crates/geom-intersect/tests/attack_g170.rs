//! Attacks on piece G170's claims: closed and open branches traced against known curves, chords
//! within their reported bound, two branches kept apart, and every start point accounted for.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::Frame;
use geom_intersect::IntersectError;
use geom_intersect::general::subdivide;
use geom_intersect::march::{Branch, Traced, march};
use geom_intersect::result::CurveEnd;
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{ParamSurface, Plane, Sphere, SurfaceKind};

const FLOOR: f64 = 1e-3;
const CEILING: f64 = 1e-2;
const TURN: f64 = 2.0;

/// A meter above what any case here takes, so a march that loses its way fails, not hangs.
fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 21))
}

fn frame() -> Frame<3> {
    Frame::standard().unwrap()
}

fn mid(x: Interval) -> f64 {
    x.lo() / 2.0 + x.hi() / 2.0
}

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

fn patch() -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0625, 0.03125, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-9.0, 9.0), (-9.0, 9.0)),
        )
        .unwrap(),
    )
}

fn trace(first: &SurfaceKind, second: &SurfaceKind) -> Traced {
    let found = subdivide(first, second, FLOOR, &mut meter()).unwrap();
    march(first, second, &found, FLOOR, CEILING, &mut meter()).unwrap()
}

/// Every vertex of a branch on both surfaces.
fn on_both(first: &SurfaceKind, second: &SurfaceKind, branch: &Branch) {
    let sides = (Limit::FromRight, Limit::FromRight);
    for [a, b, c, d] in &branch.vertices {
        let p = first.point(mid(*a), mid(*b), sides);
        let q = second.point(mid(*c), mid(*d), sides);
        if let (Ok(p), Ok(q)) = (p, q) {
            let gap = (p[0] - q[0]).hypot(p[1] - q[1]).hypot(p[2] - q[2]);
            assert!(gap < 4.0 * FLOOR, "{gap}");
        }
    }
}

#[test]
fn a_sphere_through_a_plane_is_one_branch_from_seam_to_seam() {
    let plane = patch();
    let sphere = SurfaceKind::Sphere(
        Sphere::new(
            [1.0, 2.0, 3.0],
            5.0,
            frame(),
            ((0.0625, TURN), (-0.49, 0.49)),
        )
        .unwrap(),
    );
    let traced = trace(&plane, &sphere);
    assert!(traced.unproven.is_empty(), "{:?}", traced.unproven.len());
    assert_eq!(traced.branches.len(), 1, "{:?}", traced.branches.len());
    assert_eq!(
        traced.branches[0].ends,
        [CurveEnd::Boundary, CurveEnd::Boundary]
    );
    for branch in &traced.branches {
        on_both(&plane, &sphere, branch);
        for [u, v, ..] in &branch.vertices {
            let (x, y) = (mid(*u) + 0.0625 - 1.0, mid(*v) + 0.03125 - 2.0);
            assert!((x.hypot(y) - 4.0).abs() < 4.0 * FLOOR, "{x} {y}");
        }
    }
}

#[test]
fn a_lens_is_one_closed_loop() {
    let low = bowl(2.0, -1.0, 1.0);
    let high = bowl(2.0, 1.0, -1.0);
    let traced = trace(&low, &high);
    assert!(traced.unproven.is_empty(), "{:?}", traced.unproven.len());
    assert_eq!(traced.branches.len(), 1, "{:?}", traced.branches.len());
    let branch = &traced.branches[0];
    assert_eq!(branch.ends, [CurveEnd::Closed, CurveEnd::Closed]);
    assert!(branch.vertices.len() >= 8, "{}", branch.vertices.len());
    assert!(branch.deviation <= CEILING, "{}", branch.deviation);
    on_both(&low, &high, branch);
    for [u, v, ..] in &branch.vertices {
        let (x, y) = (4.0 * mid(*u) - 2.0, 4.0 * mid(*v) - 2.0);
        assert!((x.hypot(y) - 1.0).abs() < 4.0 * FLOOR, "{x} {y}");
    }
}

#[test]
fn the_chords_stay_within_the_deviation_reported() {
    let low = bowl(2.0, -1.0, 1.0);
    let high = bowl(2.0, 1.0, -1.0);
    let traced = trace(&low, &high);
    let branch = &traced.branches[0];
    let n = branch.vertices.len();
    for k in 0..n {
        let [a, b, ..] = branch.vertices[k];
        let [c, d, ..] = branch.vertices[(k + 1) % n];
        let (x, y) = (2.0 * (mid(a) + mid(c)) - 2.0, 2.0 * (mid(b) + mid(d)) - 2.0);
        assert!(
            (x.hypot(y) - 1.0).abs() <= branch.deviation + 4.0 * FLOOR,
            "{k}: {}",
            x.hypot(y)
        );
    }
}

/// A plane through a bowl, cut off by the patch's edge: an open branch, ended by the boundary.
#[test]
fn a_branch_past_the_domain_ends_at_its_boundary() {
    let low = bowl(2.0, -1.0, 1.0);
    let flat = SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, 4.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-3.0, 3.0), (-3.0, 3.0)),
        )
        .unwrap(),
    );
    let traced = trace(&low, &flat);
    assert!(!traced.branches.is_empty(), "{traced:?}");
    for branch in &traced.branches {
        assert!(
            branch.ends.contains(&CurveEnd::Boundary),
            "{:?}",
            branch.ends
        );
        on_both(&low, &flat, branch);
    }
}

#[test]
fn a_floor_or_ceiling_that_is_not_positive_is_refused() {
    let low = bowl(2.0, -1.0, 1.0);
    let high = bowl(2.0, 1.0, -1.0);
    let found = subdivide(&low, &high, FLOOR, &mut meter()).unwrap();
    for (floor, ceiling) in [(0.0, 1.0), (f64::NAN, 1.0), (1.0, 0.0), (1.0, f64::NAN)] {
        assert_eq!(
            march(&low, &high, &found, floor, ceiling, &mut meter()),
            Err(IntersectError::InvalidInput)
        );
    }
}

#[test]
fn a_budget_stops_the_march() {
    let low = bowl(2.0, -1.0, 1.0);
    let high = bowl(2.0, 1.0, -1.0);
    let found = subdivide(&low, &high, FLOOR, &mut meter()).unwrap();
    let mut tight = WorkMeter::new(Budget::of_work(10));
    assert_eq!(
        march(&low, &high, &found, FLOOR, CEILING, &mut tight),
        Err(IntersectError::BudgetExceeded)
    );
}

#[test]
fn the_march_is_deterministic() {
    let low = bowl(2.0, -1.0, 1.0);
    let high = bowl(2.0, 1.0, -1.0);
    assert_eq!(trace(&low, &high), trace(&low, &high));
}

/// A march's work and the shape of its answer, to the unit: it is a pure function, so a change here
/// is a change to how steps are taken or how far a chord is said to reach, which where the branch
/// lies cannot show. A pin holds at its own work and fails one unit under it.
#[test]
fn each_march_takes_the_work_it_always_took() {
    let plane = patch();
    let sphere = SurfaceKind::Sphere(
        Sphere::new(
            [1.0, 2.0, 3.0],
            5.0,
            frame(),
            ((0.0625, TURN), (-0.49, 0.49)),
        )
        .unwrap(),
    );
    let low = bowl(2.0, -1.0, 1.0);
    let high = bowl(2.0, 1.0, -1.0);
    let cases: [(&SurfaceKind, &SurfaceKind, u64, usize, u64); 2] = [
        (&plane, &sphere, 64_233, 252, 4_576_821_325_672_623_522),
        (&low, &high, 341_740, 215, 4_576_859_448_148_097_595),
    ];
    for (a, b, work, vertices, reach) in cases {
        let found = subdivide(a, b, FLOOR, &mut meter()).unwrap();
        let run = |budget: u64| {
            let mut m = WorkMeter::new(Budget::of_work(budget));
            march(a, b, &found, FLOOR, CEILING, &mut m)
        };
        assert_eq!(run(work - 1), Err(IntersectError::BudgetExceeded));
        let t = run(work).unwrap();
        let total: usize = t.branches.iter().map(|x| x.vertices.len()).sum();
        let most = t.branches.iter().fold(0.0f64, |m, x| m.max(x.deviation));
        assert_eq!((total, most.to_bits()), (vertices, reach));
    }
}
