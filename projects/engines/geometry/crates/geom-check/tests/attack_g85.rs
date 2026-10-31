//! Attacks on piece G85's claims for group 2 (geometric consistency): a vertex moved off its
//! face's plane or its edge's end is caught exactly at the body's bound and never a step past it;
//! a carrier with no certified check is reported, never passed; values out of range fail closed;
//! group 2 runs only on a body group 1 passed; and it charges one unit per face, fin and edge.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "test code: a failure should stop loudly"
)]

mod common;

use common::{Rows, cube, has, ix, meter, report};
use geom_check::{Entity, FaultKind, Group, Level, check};
use geom_curve::{CurveKind, Line};
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_run::BudgetKind;
use geom_surface::{Plane, SurfaceKind};
use geom_topo::BodyKind;

fn certified(bound: u64) -> Exactness {
    Exactness::Certified {
        bound: Steps(bound),
        step: GridStep::Q0,
    }
}

/// The cube with its first corner, (0, 0, 0), moved by `by`; its faces' planes and its edges'
/// lines stay where the corner was.
fn moved(by: [i64; 3]) -> Rows {
    let mut r = cube();
    r.points[0] = P3::new(by[0], by[1], by[2]).unwrap();
    r
}

#[test]
fn a_vertex_off_its_plane_and_its_line_ends_is_caught_exactly_at_the_bound() {
    for d in [1_i64, 3, 7] {
        let r = moved([0, 0, -d]);
        // The corner is d steps below the bottom face's plane and d steps from the ends of its three
        // edges; on the two side planes through it, it has not moved.
        let at = report(&r, BodyKind::Solid, certified(d as u64 - 1));
        let faults = at.faults();
        assert!(
            has(faults, FaultKind::VertexOffSurface, Entity::Vertex(ix(0))),
            "{d}: {faults:?}"
        );
        assert!(
            has(faults, FaultKind::VertexOffCurve, Entity::Vertex(ix(0))),
            "{d}: {faults:?}"
        );
        assert!(
            faults.iter().all(|f| f.at == Entity::Vertex(ix(0))
                && matches!(
                    f.kind,
                    FaultKind::VertexOffSurface | FaultKind::VertexOffCurve
                )),
            "{d}: {faults:?}"
        );
        // Exactly at the bound it passes; recorded Exact, it fails.
        assert!(
            report(&r, BodyKind::Solid, certified(d as u64)).passed(),
            "{d}"
        );
        assert!(
            !report(&r, BodyKind::Solid, Exactness::Exact).passed(),
            "{d}"
        );
        // An approximate record's deviation is the bound too.
        let approximate = Exactness::Approximate {
            deviation: Steps(d as u64),
            step: GridStep::Q0,
        };
        assert!(report(&r, BodyKind::Solid, approximate).passed(), "{d}");
    }
}

#[test]
fn a_diagonal_move_is_judged_by_its_true_distance() {
    // Moved by (3, 4, 0): 5 steps from the ends of the edges along z, 3 and 4 from the planes
    // x = 0 and y = 0. Bound 4 passes the planes but not the 5-step line ends; bound 5 passes all.
    let r = moved([3, 4, 0]);
    let four = report(&r, BodyKind::Solid, certified(4));
    assert!(!has(
        four.faults(),
        FaultKind::VertexOffSurface,
        Entity::Vertex(ix(0))
    ));
    assert!(has(
        four.faults(),
        FaultKind::VertexOffCurve,
        Entity::Vertex(ix(0))
    ));
    assert!(report(&r, BodyKind::Solid, certified(5)).passed());
    let three = report(&r, BodyKind::Solid, certified(3));
    assert!(has(
        three.faults(),
        FaultKind::VertexOffSurface,
        Entity::Vertex(ix(0))
    ));
}

#[test]
fn carriers_without_a_certified_check_are_reported_never_passed() {
    // A plane whose origin is half a step off the grid: not given by integers.
    let mut r = cube();
    r.surfaces[0] = SurfaceKind::Plane(
        Plane::new(
            [0.5, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((0.0, 1.0), (0.0, 1.0)),
        )
        .unwrap(),
    );
    let faults = report(&r, BodyKind::Solid, certified(64)).faults().to_vec();
    assert!(
        has(&faults, FaultKind::Unchecked, Entity::Surface(ix(0))),
        "{faults:?}"
    );
    // A plane past the grid's range, 2^41 steps out.
    let mut r = cube();
    let far = 2_199_023_255_552.0; // 2^41
    r.surfaces[0] = SurfaceKind::Plane(
        Plane::new(
            [far, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((0.0, 1.0), (0.0, 1.0)),
        )
        .unwrap(),
    );
    let faults = report(&r, BodyKind::Solid, certified(64)).faults().to_vec();
    assert!(
        has(&faults, FaultKind::Unchecked, Entity::Surface(ix(0))),
        "{faults:?}"
    );
    // Unchecked faults are group 2's and say why in words.
    assert_eq!(FaultKind::Unchecked.group(), Group::Geometric);
    assert_eq!(FaultKind::VertexOffSurface.group(), Group::Geometric);
    assert_eq!(FaultKind::VertexOffCurve.group(), Group::Geometric);
}

#[test]
fn a_line_whose_end_cannot_be_enclosed_fails_closed() {
    // A direction so long that the squared gap overflows a double: no bound, so a fault.
    let mut r = cube();
    r.curves[0] = CurveKind::Line(Line::new([0.0; 3], [1e300, 0.0, 0.0], (0.0, 1.0)).unwrap());
    let faults = report(&r, BodyKind::Solid, certified(64)).faults().to_vec();
    assert!(
        faults
            .iter()
            .any(|f| f.kind == FaultKind::VertexOffCurve && f.with == Some(Entity::Edge(ix(0)))),
        "{faults:?}"
    );
}

#[test]
fn group_2_runs_only_on_a_body_group_1_passed() {
    // An edge with one end breaks group 1; group 2 is then neither run nor missing.
    let mut r = moved([0, 0, -5]);
    r.edges[0].end = None;
    let body = r.body(BodyKind::Solid);
    let full = check(&body, Exactness::Exact, Level::Full, &mut meter()).unwrap();
    assert!(!full.ran().any(|g| g == Group::Geometric));
    assert!(!full.missing().any(|g| g == Group::Geometric));
    assert!(
        full.faults()
            .iter()
            .all(|f| f.kind.group() == Group::Combinatorial)
    );
}

#[test]
fn group_2_charges_one_unit_per_face_fin_and_edge() {
    let body = cube().body(BodyKind::Solid);
    let work = |level| {
        let mut m = meter();
        check(&body, Exactness::Exact, level, &mut m).unwrap();
        m.used().get(BudgetKind::Work)
    };
    // The cube: 6 faces, 24 fins, 12 edges.
    assert_eq!(work(Level::Full) - work(Level::Cheap), 6 + 24 + 12);
}
