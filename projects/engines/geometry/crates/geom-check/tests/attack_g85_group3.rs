//! Attacks on piece G85's claims for group 3 (self-intersection): edges of a face that cross or
//! touch, run back along each other or join the same two vertices are caught; faces that cross
//! or touch away from shared edges and vertices are caught; faces whose planes cross but which do
//! not meet pass; every pair no exact or certified test separates is reported, never passed; and
//! group 3 runs only on a body groups 1 and 2 passed.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    reason = "test code: a failure should stop loudly"
)]

mod common;

use common::{
    CUBE, CUBE_FACES, Rows, annulus_sheet, certified, cube, cylinder, face_of, faces_into,
    holed_sheet, ix, level, meter, parallel, report, round, round_cylinder, torus, torus_surface,
};
use geom_check::{Entity, Fault, FaultKind, Group, Level, check};
use geom_curve::{Circle, CurveKind, Frame};
use geom_math::Box3;
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_surface::{Cone, Cylinder, Plane, Sphere, SurfaceKind};
use geom_topo::{BodyKind, RegionKind, Sense};

/// The faults of a full check of `rows` as a body of `kind`, recorded certified to `bound`.
fn at(rows: &Rows, kind: BodyKind, bound: u64) -> Vec<Fault> {
    let exactness = Exactness::Certified {
        bound: Steps(bound),
        step: GridStep::Q0,
    };
    report(rows, kind, exactness).faults().to_vec()
}

/// A certified record of `bound` steps.
fn certified_to(bound: u64) -> Exactness {
    Exactness::Certified {
        bound: Steps(bound),
        step: GridStep::Q0,
    }
}

/// Group 3's faults alone.
fn group_3(faults: &[Fault]) -> Vec<Fault> {
    faults
        .iter()
        .filter(|f| f.kind.group() == Group::SelfIntersection)
        .copied()
        .collect()
}

/// A sheet of one face whose loops run round `rings` of the vertices at `points`.
fn sheet(points: &[[i64; 3]], rings: &[&[u32]]) -> Rows {
    let mut r = Rows::empty();
    for &p in points {
        r.vertex(p);
    }
    let s = r.shell(0);
    face_of(&mut r, s, rings);
    r
}

/// A general body of sheets in the outside region: one face each, round its vertices at `points`
/// (numbered on from the sheets before).
fn sheets(each: &[&[[i64; 3]]]) -> Rows {
    let mut r = Rows::empty();
    for points in each {
        let first = r.vertices.len() as u32;
        for &p in *points {
            r.vertex(p);
        }
        let ring: Vec<u32> = (first..first + points.len() as u32).collect();
        let s = r.shell(0);
        face_of(&mut r, s, &[&ring]);
    }
    r
}

/// A fault of `kind` at `at` with `with`.
fn fault(kind: FaultKind, at: Entity, with: Entity) -> Fault {
    Fault::at(kind, at).with(with)
}

/// Whether `faults` hold a group 3 fault of `kind` and nothing from another group.
fn only(faults: &[Fault], kind: FaultKind) -> bool {
    !faults.is_empty() && faults.iter().all(|f| f.kind == kind)
}

#[test]
fn edges_of_a_face_that_cross_touch_or_run_back_are_caught() {
    // A bow tie: its first and third sides cross at (5, 5).
    let r = sheet(
        &[[0, 0, 0], [10, 10, 0], [10, 0, 0], [0, 10, 0]],
        &[&[0, 1, 2, 3]],
    );
    assert_eq!(
        at(&r, BodyKind::Sheet, 0),
        [fault(
            FaultKind::EdgesCross,
            Entity::Edge(ix(0)),
            Entity::Edge(ix(2))
        )]
    );
    // A corner on another side: (5, 0) lies on the side from (0, 0) to (10, 0), which also runs
    // on along the next side from (0, 0).
    let r = sheet(
        &[[0, 0, 0], [10, 0, 0], [10, 10, 0], [5, 0, 0]],
        &[&[0, 1, 2, 3]],
    );
    let faults = at(&r, BodyKind::Sheet, 0);
    assert!(only(&faults, FaultKind::EdgesCross), "{faults:?}");
    assert!(faults.contains(&fault(
        FaultKind::EdgesCross,
        Entity::Edge(ix(0)),
        Entity::Edge(ix(2))
    )));
    // Back along itself: the second side turns round and runs back over the first.
    let r = sheet(&[[0, 0, 0], [10, 0, 0], [5, 0, 0]], &[&[0, 1, 2]]);
    let faults = at(&r, BodyKind::Sheet, 0);
    assert!(only(&faults, FaultKind::EdgesCross), "{faults:?}");
    // Two edges between the same two vertices.
    let r = sheet(&[[0, 0, 0], [10, 0, 0]], &[&[0, 1]]);
    assert_eq!(
        at(&r, BodyKind::Sheet, 0),
        [fault(
            FaultKind::EdgesCross,
            Entity::Edge(ix(0)),
            Entity::Edge(ix(1))
        )]
    );
}

#[test]
fn faces_that_cross_or_touch_are_caught() {
    // Two cubes in one solid, the second moved by (5, 5, 5): their faces cross.
    let mut r = Rows::empty();
    let solid = r.region(RegionKind::Solid);
    for shift in [0, 5] {
        for p in CUBE {
            r.vertex(p.map(|c| c + shift));
        }
    }
    let first = r.shell(solid);
    faces_into(&mut r, first, &CUBE_FACES);
    let second = r.shell(solid);
    let moved: Vec<Vec<u32>> = CUBE_FACES
        .iter()
        .map(|f| f.iter().map(|v| v + 8).collect())
        .collect();
    let moved: Vec<&[u32]> = moved.iter().map(Vec::as_slice).collect();
    faces_into(&mut r, second, &moved);
    let faults = at(&r, BodyKind::Solid, 0);
    assert!(only(&faults, FaultKind::FacesCross), "{faults:?}");
    // A triangle standing on a square's middle: its lower corner touches the square inside it.
    let r = sheets(&[
        &[[0, 0, 0], [10, 0, 0], [10, 10, 0], [0, 10, 0]],
        &[[5, 5, 0], [8, 5, 6], [2, 5, 6]],
    ]);
    let faults = at(&r, BodyKind::General, 0);
    assert!(
        faults.contains(&fault(
            FaultKind::FacesCross,
            Entity::Face(ix(0)),
            Entity::Face(ix(1))
        )),
        "{faults:?}"
    );
}

#[test]
fn simple_polygons_pass_in_planes_of_every_slope() {
    // A polygon whose side from (0, 0) to (10, 0) has both ends on either side of the line of the
    // side from (5, 5) to (5, 1), which stops short of it: they do not meet. On the plane
    // z = x + y every coordinate projection is one to one.
    let slanted = sheet(
        &[[0, 0, 0], [10, 0, 10], [10, 10, 20], [5, 5, 10], [5, 1, 6]],
        &[&[0, 1, 2, 3, 4]],
    );
    // A quadrilateral whose diagonal side from (0, 0) to (10, 10) and side from (0, 10) to (4, 6)
    // do not meet, in the planes y = 0 and x = 0, where two coordinate projections are lines on
    // which they overlap.
    let upright = |to: fn([i64; 2]) -> [i64; 3]| {
        let points = [[0, 0], [10, 10], [4, 6], [0, 10]].map(to);
        sheet(&points, &[&[0, 1, 2, 3]])
    };
    let in_y = upright(|[a, b]| [a, 0, b]);
    let in_x = upright(|[a, b]| [0, a, b]);
    for (name, r) in [("slanted", slanted), ("y = 0", in_y), ("x = 0", in_x)] {
        let report = report(&r, BodyKind::Sheet, Exactness::Exact);
        assert!(report.passed(), "{name}: {:?}", report.faults());
    }
}

#[test]
fn faces_whose_planes_cross_but_which_do_not_meet_pass() {
    // A square in z = 0, and one in x = 5 beyond it in y: each crosses the other's plane outside
    // the other.
    let r = sheets(&[
        &[[0, 0, 0], [10, 0, 0], [10, 10, 0], [0, 10, 0]],
        &[[5, 20, -5], [5, 30, -5], [5, 30, 5], [5, 20, 5]],
    ]);
    assert_eq!(at(&r, BodyKind::General, 0), []);
    // A square in x = 15 whose side crosses z = 0 on the line of the first square's far side,
    // beyond its end.
    let r = sheets(&[
        &[[0, 0, 0], [10, 0, 0], [10, 10, 0], [0, 10, 0]],
        &[[15, 10, -5], [15, 20, -5], [15, 20, 5], [15, 10, 5]],
    ]);
    assert_eq!(at(&r, BodyKind::General, 0), []);
    // A triangle standing beside a square, its lower corner on the square's plane outside it.
    let r = sheets(&[
        &[[0, 0, 0], [10, 0, 0], [10, 10, 0], [0, 10, 0]],
        &[[25, 5, 0], [30, 5, 6], [20, 5, 6]],
    ]);
    assert_eq!(at(&r, BodyKind::General, 0), []);
}

#[test]
fn pairs_without_a_certified_test_are_reported_never_passed() {
    let unchecked = |rows: &Rows, kind: BodyKind, bound: u64| {
        let faults = at(rows, kind, bound);
        assert!(only(&faults, FaultKind::PairUnchecked), "{faults:?}");
    };
    // G84's flat stand-ins: a cylinder whose three faces lie on one plane, a torus whose seams are
    // one segment from its vertex back to itself, and an annulus whose hole crosses its side.
    unchecked(&cylinder(), BodyKind::Solid, 1);
    unchecked(&torus(), BodyKind::Solid, 1);
    unchecked(&annulus_sheet(), BodyKind::Sheet, 1);
    // Against a crossing plane: a face that is not convex, one with a hole, one whose first corner
    // is straight, and one that is not flat.
    let crossing = [[20, 0, -5], [20, 10, -5], [20, 10, 5], [20, 0, 5]];
    for (name, face) in [
        (
            "not convex",
            vec![
                [0, 0, 0],
                [10, 0, 0],
                [10, 5, 0],
                [5, 5, 0],
                [5, 10, 0],
                [0, 10, 0],
            ],
        ),
        (
            "straight first corner",
            vec![[0, 0, 0], [5, 0, 0], [10, 0, 0], [10, 10, 0], [0, 10, 0]],
        ),
        (
            "not flat",
            vec![[0, 0, 0], [10, 0, 0], [10, 10, 1], [0, 10, 0]],
        ),
    ] {
        let r = sheets(&[&face[..], &crossing[..]]);
        let faults = at(&r, BodyKind::General, 1);
        assert_eq!(
            faults,
            [fault(
                FaultKind::PairUnchecked,
                Entity::Face(ix(0)),
                Entity::Face(ix(1))
            )],
            "{name}"
        );
    }
    let mut r = holed_sheet();
    let s = r.shell(0);
    for p in crossing {
        r.vertex(p);
    }
    face_of(&mut r, s, &[&[4, 5, 6, 7]]);
    assert_eq!(
        at(&r, BodyKind::General, 1),
        [fault(
            FaultKind::PairUnchecked,
            Entity::Face(ix(0)),
            Entity::Face(ix(1))
        )]
    );
    // A face with a hole against a crossing plane, listed after it.
    let mut r = sheets(&[&crossing[..]]);
    for p in [
        [0, 0, 0],
        [10, 0, 0],
        [10, 10, 0],
        [0, 10, 0],
        [3, 3, 0],
        [3, 7, 0],
        [7, 7, 0],
        [7, 3, 0],
    ] {
        r.vertex(p);
    }
    let s = r.shell(0);
    face_of(&mut r, s, &[&[4, 5, 6, 7], &[8, 9, 10, 11]]);
    assert_eq!(
        at(&r, BodyKind::General, 1),
        [fault(
            FaultKind::PairUnchecked,
            Entity::Face(ix(0)),
            Entity::Face(ix(1))
        )]
    );
    // Two squares on parallel planes z = 0 and z = 5 given by doubles whose normal is not one: the
    // product of their spanning vectors is enclosed, not known, so the planes are not certified
    // parallel and not certified crossing.
    let mut r = sheets(&[
        &[[0, 0, 0], [10, 0, 0], [10, 10, 0], [0, 10, 0]],
        &[[0, 0, 5], [10, 0, 5], [10, 10, 5], [0, 10, 5]],
    ]);
    for (s, z) in [(0, 0.0), (1, 5.0)] {
        r.surfaces[s] = SurfaceKind::Plane(
            Plane::new(
                [0.0, 0.0, z],
                [0.1, 0.0, 0.0],
                [0.0, 0.1, 0.0],
                ((0.0, 1.0), (0.0, 1.0)),
            )
            .unwrap(),
        );
    }
    assert_eq!(
        at(&r, BodyKind::General, 1),
        [fault(
            FaultKind::PairUnchecked,
            Entity::Face(ix(0)),
            Entity::Face(ix(1))
        )]
    );
    // A hole touching the square's sides: a line at exactly its radius is not certified apart.
    let mut r = holed_sheet();
    r.curves[4] = round([5.0, 5.0, 0.0], 5.0, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    unchecked(&r, BodyKind::Sheet, 1);
    // A side of one square lying in the other's plane, on their planes' line, but not an edge of
    // both.
    let r = sheets(&[
        &[[0, 0, 0], [10, 0, 0], [10, 10, 0], [0, 10, 0]],
        &[[10, 0, 0], [10, 10, 0], [10, 10, 10], [10, 0, 10]],
    ]);
    assert!(only(
        &at(&r, BodyKind::General, 0),
        FaultKind::PairUnchecked
    ));
    // A cylinder's top cap tilted: no longer perpendicular to the axis.
    let mut r = round_cylinder();
    r.surfaces[0] = SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, 10.0],
            [10.0, 0.0, 1.0],
            [0.0, 1.0, 0.0],
            ((0.0, 1.0), (0.0, 1.0)),
        )
        .unwrap(),
    );
    unchecked(&r, BodyKind::Solid, 1);
    // A top cap that shares only half a circle with the side.
    let mut r = round_cylinder();
    r.curves[0] = CurveKind::Circle(
        Circle::new(
            [0.0, 0.0, 10.0],
            5.0,
            Frame::standard().unwrap(),
            (0.0, 1.0),
        )
        .unwrap(),
    );
    unchecked(&r, BodyKind::Solid, 1);
    // A capsule: a cylinder's side meets a dome, two curved faces, and the bottom cap and the
    // dome share no edge (the bottom circle has fins on the cap and the side, not the dome).
    unchecked(&capsule(), BodyKind::Solid, 1);
    assert!(at(&capsule(), BodyKind::Solid, 1).contains(&fault(
        FaultKind::PairUnchecked,
        Entity::Face(ix(0)),
        Entity::Face(ix(2))
    )));
    // A torus whose two seams are closed circles without vertices, in crossing planes.
    unchecked(&loose_seams(), BodyKind::Solid, 1);
    // A face holding a circle and a line edge without vertices.
    let mut r = holed_sheet();
    r.curves[4] = common::line([5, 5, 0], [6, 5, 0]);
    unchecked(&r, BodyKind::Sheet, 1);
}

/// A capsule: a bottom cap, a cylindrical side and a dome on top, joined by two circles.
fn capsule() -> Rows {
    let mut r = Rows::empty();
    let solid = r.region(RegionKind::Solid);
    let (bottom, top) = (r.edge(None), r.edge(None));
    let s = r.shell(solid);
    r.face(s, &[&[(bottom, Sense::Forward)]]);
    r.face(
        s,
        &[&[(bottom, Sense::Reversed)], &[(top, Sense::Reversed)]],
    );
    r.face(s, &[&[(top, Sense::Forward)]]);
    r.curves[bottom as usize] = parallel(0.0, 5.0);
    r.curves[top as usize] = parallel(10.0, 5.0);
    r.surfaces[0] = level(0.0);
    r.surfaces[1] = SurfaceKind::Cylinder(
        Cylinder::new(
            [0.0; 3],
            5.0,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (0.0, 10.0)),
        )
        .unwrap(),
    );
    r.surfaces[2] = SurfaceKind::Sphere(
        Sphere::new(
            [0.0, 0.0, 10.0],
            5.0,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (0.0, 0.5)),
        )
        .unwrap(),
    );
    r
}

/// A ring torus whose parallel and meridian seams are closed circles without vertices, each its
/// own loop, used both ways.
fn loose_seams() -> Rows {
    let mut r = Rows::empty();
    let solid = r.region(RegionKind::Solid);
    let (u, w) = (r.edge(None), r.edge(None));
    let s = r.shell(solid);
    r.face(
        s,
        &[
            &[(u, Sense::Forward)],
            &[(u, Sense::Reversed)],
            &[(w, Sense::Forward)],
            &[(w, Sense::Reversed)],
        ],
    );
    r.curves[u as usize] = parallel(0.0, 13.0);
    r.curves[w as usize] = round([10.0, 0.0, 0.0], 3.0, [1.0, 0.0, 0.0], [0.0, -1.0, 0.0]);
    r.surfaces[0] = torus_surface();
    r
}

#[test]
fn segments_and_circles_of_one_face_are_separated_where_certified() {
    // A square with a circle loop lifted 3 steps off it, on one plane within a bound of 5: every
    // side lies strictly below the circle's plane.
    let mut r = sheet(
        &[[0, 0, 0], [10, 0, 0], [10, 10, 0], [0, 10, 0]],
        &[&[0, 1, 2, 3]],
    );
    let lifted = r.edge(None);
    r.curves[lifted as usize] = round([5.0, 5.0, 3.0], 2.0, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    add_loop(&mut r, 0, lifted);
    assert_eq!(at(&r, BodyKind::Sheet, 5), []);
    // A triangle through a disc's plane: each side crosses it once, inside the circle but off it.
    let disc_with = |corners: [[i64; 3]; 3]| {
        let mut r = sheet(&corners, &[&[0, 1, 2]]);
        let rim = r.edge(None);
        r.curves[rim as usize] = parallel(0.0, 5.0);
        add_loop(&mut r, 0, rim);
        r.surfaces[0] = level(0.0);
        r
    };
    assert_eq!(
        at(
            &disc_with([[3, 0, -1], [3, 0, 1], [4, 1, 0]]),
            BodyKind::Sheet,
            1
        ),
        []
    );
    // One that crosses it on the circle: not certified.
    let faults = at(
        &disc_with([[5, 0, -1], [5, 0, 1], [6, 1, 0]]),
        BodyKind::Sheet,
        1,
    );
    assert!(only(&faults, FaultKind::PairUnchecked), "{faults:?}");
}

/// Adds closed edge `e` as a loop of its own to face `f`, after its other loops.
fn add_loop(r: &mut Rows, f: u32, e: u32) {
    let l = r.loops.len() as u32;
    let fin = r.fins.len() as u32;
    r.loops.push(geom_topo::Loop {
        face: ix(f),
        next: None,
        first_fin: Some(ix(fin)),
    });
    let mut last = r.faces[f as usize].first_loop.unwrap().row();
    while let Some(n) = r.loops[last as usize].next {
        last = n.row();
    }
    r.loops[last as usize].next = Some(ix(l));
    r.fins.push(geom_topo::Fin {
        loop_: ix(l),
        next: ix(fin),
        prev: ix(fin),
        edge: ix(e),
        sense: Sense::Reversed,
        radial: ix(fin),
        pcurve: None,
    });
    r.edges[e as usize].first_fin = Some(ix(fin));
}

#[test]
fn circles_of_one_face_are_separated_where_certified() {
    // A ring: a disc of radius 5 with a hole of radius 2 at its centre.
    let ring = |hole: [f64; 3]| {
        let mut r = Rows::empty();
        let (rim, inner) = (r.edge(None), r.edge(None));
        let s = r.shell(0);
        r.face(s, &[&[(rim, Sense::Forward)], &[(inner, Sense::Reversed)]]);
        r.curves[rim as usize] = parallel(0.0, 5.0);
        r.curves[inner as usize] = round(hole, 2.0, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        r.surfaces[0] = level(0.0);
        r
    };
    assert_eq!(at(&ring([0.0; 3]), BodyKind::Sheet, 1), []);
    // The hole moved until it touches the rim from inside: not certified apart.
    let faults = at(&ring([3.0, 0.0, 0.0]), BodyKind::Sheet, 1);
    assert!(only(&faults, FaultKind::PairUnchecked), "{faults:?}");
    // The hole moved across the rim.
    let faults = at(&ring([4.0, 0.0, 0.0]), BodyKind::Sheet, 1);
    assert!(only(&faults, FaultKind::PairUnchecked), "{faults:?}");
}

#[test]
fn a_cone_listed_side_first_passes() {
    // The side first, its seam before its rim: the side's edges are a segment and the full circle
    // it shares with the base.
    let mut r = Rows::empty();
    let solid = r.region(RegionKind::Solid);
    let (base, apex) = (r.vertex([6, 0, 0]), r.vertex([0, 0, 12]));
    let rim = r.edge(Some((base, base)));
    let seam = r.edge(Some((apex, base)));
    let s = r.shell(solid);
    r.face(
        s,
        &[&[
            (seam, Sense::Reversed),
            (seam, Sense::Forward),
            (rim, Sense::Reversed),
        ]],
    );
    r.face(s, &[&[(rim, Sense::Forward)]]);
    r.curves[rim as usize] = parallel(0.0, 6.0);
    r.surfaces[0] = SurfaceKind::Cone(
        Cone::new(
            [0.0; 3],
            6.0,
            -0.5,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (0.0, 12.0)),
        )
        .unwrap(),
    );
    r.surfaces[1] = level(0.0);
    assert_eq!(at(&r, BodyKind::Solid, 1), []);
}

#[test]
fn group_3_runs_only_on_a_body_groups_1_and_2_passed() {
    // A bow tie whose corner is off its plane: group 2 finds it, and group 3 is neither run nor
    // missing.
    let mut r = sheet(
        &[[0, 0, 0], [10, 10, 0], [10, 0, 0], [0, 10, 0]],
        &[&[0, 1, 2, 3]],
    );
    r.points[3] = P3::new(0, 10, 3).unwrap();
    r.faces[0].bound = r.faces[0]
        .bound
        .union(&Box3::new(r.points[3], r.points[3]).unwrap());
    let body = r.body(BodyKind::Sheet);
    let full = check(&body, Exactness::Exact, Level::Full, &mut meter()).unwrap();
    assert!(!full.ran().any(|g| g == Group::SelfIntersection));
    assert!(!full.missing().any(|g| g == Group::SelfIntersection));
    assert_eq!(group_3(full.faults()), []);
    // With a bound that covers the corner, group 3 runs: the sides that crossed are now skew, and
    // the bow tie, as much clockwise as counter-clockwise, encloses no area.
    let lifted = check(&body, certified_to(3), Level::Full, &mut meter()).unwrap();
    assert!(lifted.ran().any(|g| g == Group::SelfIntersection));
    assert_eq!(
        lifted.faults(),
        [Fault::at(FaultKind::LoopSense, Entity::Face(ix(0)))
            .with(Entity::Loop(ix(0)))
            .value(0)]
    );
    // A cube passes all four groups.
    let full = check(
        &cube().body(BodyKind::Solid),
        certified(),
        Level::Full,
        &mut meter(),
    )
    .unwrap();
    assert!(full.passed());
    assert_eq!(full.ran().collect::<Vec<_>>(), Group::ALL);
    assert_eq!(FaultKind::EdgesCross.group(), Group::SelfIntersection);
    assert_eq!(FaultKind::FacesCross.group(), Group::SelfIntersection);
    assert_eq!(FaultKind::PairUnchecked.group(), Group::SelfIntersection);
}
