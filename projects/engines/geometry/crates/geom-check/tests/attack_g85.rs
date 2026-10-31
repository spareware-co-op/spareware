//! Attacks on piece G85's claims for group 2 (geometric consistency): a vertex moved off its
//! face's plane, cylinder, sphere or torus, or off its edge's end, is caught exactly at the body's
//! bound and never a step past it; an edge off its face is caught; a carrier with no certified
//! check is reported, never passed; values out of range fail closed; group 2 runs only on a body
//! group 1 passed; and it charges one unit per face, fin and edge.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "test code: a failure should stop loudly"
)]

mod common;

use common::{
    Rows, cone, cube, has, hemisphere, ix, line, meter, report, ring_torus, round, round_cylinder,
    seamed_cylinder,
};
use geom_check::{Entity, Fault, FaultKind, Group, Level, check};
use geom_curve::{Axes, Basis, CurveKind, Ellipse, Frame, Line, Trimmed};
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_run::BudgetKind;
use geom_surface::Cone;
use geom_surface::{Extrusion, Plane, SurfaceKind, Torus};
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

/// Group 2's faults in a full check of the solid `rows`, recorded certified to `bound`. (A
/// vertex moved off its planes leaves faces that are not flat, which group 3 then reports
/// unchecked: `attack_g85_group3.rs`.)
fn at(rows: &Rows, bound: u64) -> Vec<Fault> {
    report(rows, BodyKind::Solid, certified(bound))
        .faults()
        .iter()
        .filter(|f| f.kind.group() == Group::Geometric)
        .copied()
        .collect()
}

/// Whether `faults` say vertex `v` is off face `f`'s surface.
fn off(faults: &[Fault], v: u32, f: u32) -> bool {
    faults.contains(
        &Fault::at(FaultKind::VertexOffSurface, Entity::Vertex(ix(v))).with(Entity::Face(ix(f))),
    )
}

/// Whether `faults` say edge `e` is off face `f`'s surface.
fn edge_off(faults: &[Fault], e: u32, f: u32) -> bool {
    faults.contains(
        &Fault::at(FaultKind::EdgeOffSurface, Entity::Edge(ix(e))).with(Entity::Face(ix(f))),
    )
}

#[test]
fn a_vertex_off_its_plane_and_its_line_ends_is_caught_exactly_at_the_bound() {
    for d in [1_i64, 3, 7] {
        let r = moved([0, 0, -d]);
        // The corner is d steps below the bottom face's plane and d steps from the ends of its three
        // edges; on the two side planes through it, it has not moved.
        let faults = at(&r, d as u64 - 1);
        assert!(
            has(&faults, FaultKind::VertexOffSurface, Entity::Vertex(ix(0))),
            "{d}: {faults:?}"
        );
        assert!(
            has(&faults, FaultKind::VertexOffCurve, Entity::Vertex(ix(0))),
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
        assert_eq!(at(&r, d as u64), [], "{d}");
        assert!(
            !report(&r, BodyKind::Solid, Exactness::Exact).passed(),
            "{d}"
        );
        // An approximate record's deviation is the bound too.
        let approximate = Exactness::Approximate {
            deviation: Steps(d as u64),
            step: GridStep::Q0,
        };
        assert!(
            report(&r, BodyKind::Solid, approximate)
                .faults()
                .iter()
                .all(|f| f.kind.group() != Group::Geometric),
            "{d}"
        );
    }
}

#[test]
fn a_diagonal_move_is_judged_by_its_true_distance() {
    // Moved by (3, 4, 0): 5 steps from the ends of the edges along z, 3 and 4 from the planes
    // x = 0 and y = 0. Bound 4 passes the planes but not the 5-step line ends; bound 5 passes all.
    let r = moved([3, 4, 0]);
    let four = at(&r, 4);
    assert!(!has(
        &four,
        FaultKind::VertexOffSurface,
        Entity::Vertex(ix(0))
    ));
    assert!(has(&four, FaultKind::VertexOffCurve, Entity::Vertex(ix(0))));
    assert_eq!(at(&r, 5), []);
    assert!(has(
        &at(&r, 3),
        FaultKind::VertexOffSurface,
        Entity::Vertex(ix(0))
    ));
}

#[test]
fn a_vertex_off_a_cylinder_sphere_or_torus_is_caught_exactly_at_the_bound() {
    // Each vertex moved 3 steps straight out from its curved face, along the plane it also lies
    // on: off the curved face by exactly 3.
    let mut cylinder = seamed_cylinder();
    cylinder.points[0] = P3::new(8, 0, 0).unwrap();
    let mut dome = hemisphere();
    dome.points[0] = P3::new(8, 0, 0).unwrap();
    let mut torus = ring_torus();
    torus.points[0] = P3::new(16, 0, 0).unwrap();
    // Off the bottom circle's start, where the vertex was, by the same 3 steps.
    let mut lifted = seamed_cylinder();
    lifted.points[0] = P3::new(8, 0, 0).unwrap();
    let two = at(&lifted, 2);
    assert!(
        two.contains(
            &Fault::at(FaultKind::VertexOffCurve, Entity::Vertex(ix(0))).with(Entity::Edge(ix(0)))
        ),
        "{two:?}"
    );
    for (name, rows, face) in [
        ("cylinder", cylinder, 2),
        ("hemisphere", dome, 1),
        ("torus", torus, 0),
    ] {
        let two = at(&rows, 2);
        assert!(off(&two, 0, face), "{name}: {two:?}");
        assert!(!off(&at(&rows, 3), 0, face), "{name}");
        // The planes it lies on find nothing.
        assert!(
            two.iter()
                .filter(|f| f.kind == FaultKind::VertexOffSurface)
                .all(|f| f.with == Some(Entity::Face(ix(face)))),
            "{name}: {two:?}"
        );
    }
}

#[test]
fn a_vertex_off_a_cone_is_judged_by_its_perpendicular_distance() {
    // The base vertex moved out 3 steps along the base plane: 3 / √(1 + 1/4) ≈ 2.68 steps from the
    // cone, whose slope is 1/2.
    let mut r = cone();
    r.points[0] = P3::new(9, 0, 0).unwrap();
    assert!(off(&at(&r, 2), 0, 1));
    assert!(!off(&at(&r, 3), 0, 1));
    // The apex moved past the cone's end, where its radius would be negative: not certified at
    // any bound.
    let mut r = cone();
    r.points[1] = P3::new(0, 0, 14).unwrap();
    assert!(off(&at(&r, 64), 1, 1));
}

#[test]
fn an_edge_off_its_face_is_caught() {
    // The top circle of radius 7 on a cylinder of radius 5: 2 steps off it everywhere.
    let mut r = seamed_cylinder();
    r.curves[1] = round([0.0, 0.0, 10.0], 7.0, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    assert!(edge_off(&at(&r, 1), 1, 2));
    assert!(!edge_off(&at(&r, 2), 1, 2));
    // The seam replaced by a chord across the side: its ends are on the cylinder, its middle is
    // not.
    let mut r = seamed_cylinder();
    r.points[1] = P3::new(0, 5, 10).unwrap();
    r.curves[1] = round([0.0, 0.0, 10.0], 5.0, [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    r.curves[2] = line([5, 0, 0], [0, 5, 10]);
    let faults = at(&r, 1);
    assert!(edge_off(&faults, 2, 2), "{faults:?}");
    assert!(!off(&faults, 1, 2), "{faults:?}");
    // The cylinder's bottom circle lifted 3 steps off its cap.
    let mut r = round_cylinder();
    r.curves[1] = round([0.0, 0.0, 3.0], 5.0, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    assert!(edge_off(&at(&r, 2), 1, 1));
    assert!(!edge_off(&at(&r, 3), 1, 1));
    // A circle of radius 30 about a wide cone's axis, tilted so that its normal is (4, 0, 3) / 5:
    // its lowest point, 24 steps down and 18 from the axis, is 54 steps inside the cone's radius
    // there, about 48 steps from the cone. The bound must cover the whole height the circle
    // spans, not only its top, where it is about 40 steps from the cone.
    let mut r = common::bare_cone();
    r.surfaces[1] = SurfaceKind::Cone(
        Cone::new(
            [0.0; 3],
            60.0,
            -0.5,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (0.0, 120.0)),
        )
        .unwrap(),
    );
    r.curves[0] = round([0.0; 3], 30.0, [1.0, 0.0, 0.0], [4.0, 0.0, 3.0]);
    assert!(edge_off(&at(&r, 45), 0, 1));
    // A line from a corner on the bottom face's plane to a point 5 steps above it: its start is
    // on the plane, its end is not.
    let mut r = cube();
    r.curves[0] = CurveKind::Line(Line::new([0.0; 3], [0.0, 10.0, 5.0], (0.0, 1.0)).unwrap());
    let faults = at(&r, 1);
    assert!(edge_off(&faults, 0, 0), "{faults:?}");
    // A line can lie on neither a sphere nor a torus.
    let mut r = ring_torus();
    r.curves[1] = line([13, 0, 0], [13, 0, 0]);
    assert!(edge_off(&at(&r, 64), 1, 0));
    // The meridian seam turned into a parallel's plane: far from both families.
    let mut r = ring_torus();
    r.curves[1] = round([10.0, 0.0, 0.0], 3.0, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    assert!(edge_off(&at(&r, 2), 1, 0));
}

#[test]
fn a_plane_given_by_doubles_is_unchecked_in_an_exact_body_and_measured_otherwise() {
    // The plane z = 1/2, its origin half a step off the grid.
    let mut r = cube();
    r.surfaces[0] = SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, 0.5],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((0.0, 1.0), (0.0, 1.0)),
        )
        .unwrap(),
    );
    assert_eq!(
        report(&r, BodyKind::Solid, Exactness::Exact).faults(),
        [Fault::at(FaultKind::Unchecked, Entity::Surface(ix(0))).with(Entity::Face(ix(0)))]
    );
    // Certified, its four corners and four edges are half a step off: caught at bound 0, passed at
    // bound 1.
    let zero = at(&r, 0);
    assert_eq!(zero.len(), 8, "{zero:?}");
    assert!(zero.iter().all(|f| f.with == Some(Entity::Face(ix(0)))
        && matches!(
            f.kind,
            FaultKind::VertexOffSurface | FaultKind::EdgeOffSurface
        )));
    assert_eq!(at(&r, 1), []);
    // A plane past the grid's range, 2^41 steps along itself: z = 0 still, so certified it
    // passes, and recorded Exact it is unchecked.
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
    assert_eq!(at(&r, 0), []);
    assert!(has(
        report(&r, BodyKind::Solid, Exactness::Exact).faults(),
        FaultKind::Unchecked,
        Entity::Surface(ix(0))
    ));
}

#[test]
fn carriers_without_a_certified_check_are_reported_never_passed() {
    let only = |rows: &Rows, fault: Fault| {
        let faults = at(rows, 64);
        assert_eq!(faults, [fault], "{faults:?}");
    };
    // A spindle torus, its minor radius above its major one.
    let mut r = ring_torus();
    r.surfaces[0] = SurfaceKind::Torus(
        Torus::new(
            [0.0; 3],
            2.0,
            3.0,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (0.0, 2.0)),
        )
        .unwrap(),
    );
    only(
        &r,
        Fault::at(FaultKind::Unchecked, Entity::Surface(ix(0))).with(Entity::Face(ix(0))),
    );
    // A horn torus, its two radii equal.
    let mut r = ring_torus();
    r.surfaces[0] = SurfaceKind::Torus(
        Torus::new(
            [0.0; 3],
            3.0,
            3.0,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (0.0, 2.0)),
        )
        .unwrap(),
    );
    only(
        &r,
        Fault::at(FaultKind::Unchecked, Entity::Surface(ix(0))).with(Entity::Face(ix(0))),
    );
    // A swept surface.
    let mut r = cube();
    let profile = Trimmed::new(
        Basis::Line(Line::new([0.0; 3], [1.0, 0.0, 0.0], (0.0, 10.0)).unwrap()),
        (0.0, 10.0),
    )
    .unwrap();
    r.surfaces[0] =
        SurfaceKind::Extrusion(Extrusion::new(profile, [0.0, 1.0, 0.0], (0.0, 10.0)).unwrap());
    only(
        &r,
        Fault::at(FaultKind::Unchecked, Entity::Surface(ix(0))).with(Entity::Face(ix(0))),
    );
    // An ellipse: its ends and its place on its faces are both unchecked, reported once.
    let mut r = cube();
    r.curves[0] = CurveKind::Ellipse(
        Ellipse::new(
            [0.0; 3],
            Axes::Frame {
                frame: Frame::standard().unwrap(),
                a: 2.0,
                b: 1.0,
            },
            (0.0, 2.0),
        )
        .unwrap(),
    );
    only(
        &r,
        Fault::at(FaultKind::Unchecked, Entity::Curve(ix(0))).with(Entity::Edge(ix(0))),
    );
    // Unchecked faults are group 2's and say why in words.
    assert_eq!(FaultKind::Unchecked.group(), Group::Geometric);
    assert_eq!(FaultKind::VertexOffSurface.group(), Group::Geometric);
    assert_eq!(FaultKind::VertexOffCurve.group(), Group::Geometric);
    assert_eq!(FaultKind::EdgeOffSurface.group(), Group::Geometric);
}

#[test]
fn a_line_whose_end_cannot_be_enclosed_fails_closed() {
    // A direction so long that the squared gap overflows a double: no bound, so a fault.
    let mut r = cube();
    r.curves[0] = CurveKind::Line(Line::new([0.0; 3], [1e300, 0.0, 0.0], (0.0, 1.0)).unwrap());
    let faults = at(&r, 64);
    assert!(
        faults
            .iter()
            .any(|f| f.kind == FaultKind::VertexOffCurve && f.with == Some(Entity::Edge(ix(0)))),
        "{faults:?}"
    );
    // A ray, its far end at infinity: neither its end nor its place on its faces can be bounded.
    let mut r = cube();
    r.curves[0] =
        CurveKind::Line(Line::new([0.0; 3], [0.0, 1.0, 0.0], (0.0, f64::INFINITY)).unwrap());
    let faults = at(&r, 64);
    assert!(edge_off(&faults, 0, 0), "{faults:?}");
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
fn groups_2_and_3_charge_one_unit_per_row_and_pair_visited() {
    let work = |rows: &Rows, level| {
        let body = rows.body(BodyKind::Solid);
        let mut m = meter();
        check(&body, certified(1), level, &mut m).unwrap();
        m.used().get(BudgetKind::Work)
    };
    // Group 2, one unit per face, fin and edge; group 3, one per face, per pair of a face's edges,
    // per pair of faces and per edge tested against a face. The cube: 6 faces, 24 fins and 12
    // edges; 6 faces, 6 pairs of edges in each, 15 pairs of faces, and each of the 12 pairs that
    // meet at an edge tests its 8 edges. The cylinder with a seam: 3 faces, 6 fins and 3 edges; 3
    // faces, the 3 pairs of the side's edges, 3 pairs of faces (parallel caps, and caps that share
    // a circle with the side, tested without their edges).
    for (rows, rows_visited) in [
        (cube(), (6 + 24 + 12) + (6 + 6 * 6 + 15 + 12 * 8)),
        (seamed_cylinder(), (3 + 6 + 3) + (3 + 3 + 3)),
    ] {
        assert_eq!(
            work(&rows, Level::Full) - work(&rows, Level::Cheap),
            rows_visited
        );
    }
}
