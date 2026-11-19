//! Attacks on piece G85's claims for group 3 (self-intersection): edges of a face that cross or
//! touch, run back along each other or join the same two vertices are caught; faces that cross
//! or touch away from shared edges and vertices are caught; faces whose planes cross but which do
//! not meet pass; two faces on spheres pass where the spheres are certainly apart, nested or each
//! outside the other; every pair no exact or certified test separates is reported, never passed;
//! and group 3 runs only on a body groups 1 and 2 passed.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    reason = "test code: a failure should stop loudly"
)]

mod common;

use common::{
    CUBE, CUBE_FACES, Rows, annulus_sheet, certified, cube, cylinder, disc_sheet, face_of,
    faces_into, holed_sheet, ix, level, meter, parallel, polyhedron, report, round, round_cylinder,
    torus, torus_surface,
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
fn large_flat_bodies_are_compared_exactly() {
    // Found by the make fuzz target: the normals of a long bar's planes and of a wide prism's have
    // products no double holds, so a test in intervals could not tell parallel from crossing.
    let w = 768_024_272_902;
    let bar: Vec<[i64; 3]> = common::CUBE
        .iter()
        .map(|&[x, y, z]| [x / 10 * w, y / 10 * 487_500, z / 10])
        .collect();
    let r = common::polyhedron(&bar, &common::CUBE_FACES);
    assert_eq!(at(&r, BodyKind::Solid, 0), []);
    // A parallelepiped of grid corners: opposite faces parallel, every other pair crossing.
    let sheared: Vec<[i64; 3]> = common::CUBE
        .iter()
        .map(|&[x, y, z]| {
            let (along, across, up) = (x / 10, y / 10, z / 10);
            [
                along * 300_000_000_001 + across * 7,
                across * 300_000_000_001 - along * 11,
                up * 3 + along,
            ]
        })
        .collect();
    let r = common::polyhedron(&sheared, &common::CUBE_FACES);
    assert_eq!(at(&r, BodyKind::Solid, 0), []);
}

#[test]
fn two_faces_on_one_plane_are_decided() {
    // Two sheets on the plane z = 0, side by side: the same plane, and apart on it.
    let r = sheets(&[
        &[[0, 0, 0], [4, 0, 0], [4, 4, 0], [0, 4, 0]],
        &[[9, 0, 0], [13, 0, 0], [13, 4, 0], [9, 4, 0]],
    ]);
    assert_eq!(group_3(&at(&r, BodyKind::Sheet, 0)), []);
    // The same on the plane z = 1, scaled: an integer multiple of a plane is the same plane.
    let r = sheets(&[
        &[[0, 0, 1], [4, 0, 1], [4, 4, 1], [0, 4, 1]],
        &[[9, 0, 1], [18, 0, 1], [18, 9, 1], [9, 9, 1]],
    ]);
    assert_eq!(group_3(&at(&r, BodyKind::Sheet, 0)), []);
    // Two faces on one plane that is not given by integers: their grid corners fix it.
    let mut r = sheets(&[
        &[[0, 0, 0], [4, 0, 0], [4, 4, 0], [0, 4, 0]],
        &[[9, 0, 0], [13, 0, 0], [13, 4, 0], [9, 4, 0]],
    ]);
    for k in 0..2 {
        r.surfaces[k] = SurfaceKind::Plane(
            Plane::new(
                [0.25, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                ((0.0, 1.0), (0.0, 1.0)),
            )
            .unwrap(),
        );
    }
    assert_eq!(group_3(&at(&r, BodyKind::Sheet, 1)), []);
    // Parallel planes a step apart pass.
    let r = sheets(&[
        &[[0, 0, 1], [4, 0, 1], [4, 4, 1], [0, 4, 1]],
        &[[0, 0, 2], [8, 0, 2], [8, 8, 2], [0, 8, 2]],
    ]);
    assert_eq!(group_3(&at(&r, BodyKind::Sheet, 0)), []);
}

/// A plane through `origin` spanned by `u` and `v`, with an origin off the grid, so the check
/// compares it by certified intervals rather than as an integer plane.
fn plane_off_grid(origin: [f64; 3], u: [f64; 3], v: [f64; 3]) -> SurfaceKind {
    SurfaceKind::Plane(Plane::new(origin, u, v, ((0.0, 1.0), (0.0, 1.0))).unwrap())
}

#[test]
fn planes_off_the_grid_are_certified_apart_or_crossing() {
    // Parallel planes a step apart, neither given by integers: certainly apart, so they pass.
    let mut r = sheets(&[
        &[[0, 0, 1], [4, 0, 1], [4, 4, 1], [0, 4, 1]],
        &[[0, 0, 2], [8, 0, 2], [8, 8, 2], [0, 8, 2]],
    ]);
    for (k, z) in [(0, 1.0), (1, 2.0)] {
        r.surfaces[k] = plane_off_grid([0.25, 0.0, z], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    }
    assert_eq!(group_3(&at(&r, BodyKind::Sheet, 1)), []);
    // A square in z = 0 and one in x = 5 beyond it in y, neither plane given by integers: the
    // planes certainly cross, and the faces are then tested exactly, and pass.
    let mut r = sheets(&[
        &[[0, 0, 0], [10, 0, 0], [10, 10, 0], [0, 10, 0]],
        &[[5, 20, -5], [5, 30, -5], [5, 30, 5], [5, 20, 5]],
    ]);
    r.surfaces[0] = plane_off_grid([0.25, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    r.surfaces[1] = plane_off_grid([5.0, 0.25, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    assert_eq!(group_3(&at(&r, BodyKind::General, 1)), []);
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
    // Against a crossing plane, a face of any shape is decided: not convex, with a first corner
    // that is straight, and bent, which stands for its star of triangles. A bent face no star
    // can stand for is not: this one's notch is cut back past the mean of its corners, (7, 3),
    // which sees the notch's two edges from outside.
    let crossing = [[20, 0, -5], [20, 10, -5], [20, 10, 5], [20, 0, 5]];
    let bent = [[0, 0, 0], [10, 0, 0], [10, 10, 1], [8, 2, 0]];
    let mut r = sheets(&[&bent[..], &crossing[..]]);
    r.surfaces[0] = level(0.0);
    assert_eq!(
        at(&r, BodyKind::General, 1),
        [fault(
            FaultKind::PairUnchecked,
            Entity::Face(ix(0)),
            Entity::Face(ix(1))
        )]
    );
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
        assert_eq!(at(&r, BodyKind::General, 1), [], "{name}");
    }
    // A face holding a circle against a crossing plane: apart where the plane leaves its corners
    // and its circle on one side (decision 32.96), and unchecked where it crosses the face.
    let unchecked_pair = [fault(
        FaultKind::PairUnchecked,
        Entity::Face(ix(0)),
        Entity::Face(ix(1)),
    )];
    for (x, faults) in [(20, &[][..]), (5, &unchecked_pair[..])] {
        let mut r = holed_sheet();
        let s = r.shell(0);
        for [_, y, z] in crossing {
            r.vertex([x, y, z]);
        }
        face_of(&mut r, s, &[&[4, 5, 6, 7]]);
        assert_eq!(at(&r, BodyKind::General, 1), faults, "{x}");
    }
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
    assert_eq!(at(&r, BodyKind::General, 1), []);
    // Two squares on parallel planes z = 0 and z = 5 given by doubles whose normal is not one: their
    // grid corners fix the planes.
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
    assert_eq!(at(&r, BodyKind::General, 1), []);
    // A hole touching the square's sides: a line at exactly its radius is not certified apart.
    let mut r = holed_sheet();
    r.curves[4] = round([5.0, 5.0, 0.0], 5.0, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    unchecked(&r, BodyKind::Sheet, 1);
    // A side of one square lying along the other's, but not an edge of both: they meet other than
    // at anything they share.
    let r = sheets(&[
        &[[0, 0, 0], [10, 0, 0], [10, 10, 0], [0, 10, 0]],
        &[[10, 0, 0], [10, 10, 0], [10, 10, 10], [10, 0, 10]],
    ]);
    assert!(only(&at(&r, BodyKind::General, 0), FaultKind::FacesCross));
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

#[test]
fn a_segment_in_a_circles_plane_is_apart_only_on_the_lower_end_of_its_reach() {
    // A triangle of corners (0, 5), (5, 0) and (5, 5) round a circle at the origin, on one plane:
    // the side from (0, 5) to (5, 0) lies 5/√2 from the centre, which no double holds. The double
    // nearest it, 3.5355339059327378, lies above it, so a circle of that radius crosses the side
    // by less than a rounding: refused, as the side's reach from the centre is read on its lower
    // end. A radius of 3.5 is certainly short of every side.
    let disc = |radius: f64| {
        let mut r = sheet(&[[0, 5, 0], [5, 0, 0], [5, 5, 0]], &[&[0, 1, 2]]);
        let rim = r.edge(None);
        r.curves[rim as usize] = parallel(0.0, radius);
        add_loop(&mut r, 0, rim);
        r.surfaces[0] = level(0.0);
        r
    };
    let faults = at(&disc(3.535_533_905_932_737_8), BodyKind::Sheet, 1);
    assert!(only(&faults, FaultKind::PairUnchecked), "{faults:?}");
    assert_eq!(at(&disc(3.5), BodyKind::Sheet, 1), []);
}

/// Adds closed edge `e` as a loop of its own to face `f`, after its other loops.
fn add_loop(r: &mut Rows, f: u32, e: u32) {
    add_loop_as(r, f, e, Sense::Reversed);
}

/// Adds closed edge `e`, which has no fins yet, as a loop of its own to face `f`, after its other
/// loops, its fin running `sense`.
fn add_loop_as(r: &mut Rows, f: u32, e: u32, sense: Sense) {
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
        sense,
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
fn a_circle_crossing_another_by_less_than_a_rounding_is_not_passed_as_inside() {
    // A rim of radius 4 and a hole of radius 1 + 2^-52 whose centre lies 3 from the rim's, on one
    // plane: 4 − (1 + 2^-52) = 3 − 2^-52 is not a double, so its enclosure reaches the step above
    // 3, yet the hole reaches 2^-52 past the rim and the two circles cross (issue #1033). Read on
    // that upper end of |r − s|, the hole was certified inside. Each loop listed first in turn.
    let hole = 1.0 + f64::EPSILON;
    let ring = |centre: [f64; 3], rim_first: bool| {
        let mut r = Rows::empty();
        let (rim, inner) = (r.edge(None), r.edge(None));
        let s = r.shell(0);
        let (outer, hollow): (&[_], &[_]) = (&[(rim, Sense::Forward)], &[(inner, Sense::Reversed)]);
        if rim_first {
            r.face(s, &[outer, hollow]);
        } else {
            r.face(s, &[hollow, outer]);
        }
        r.curves[rim as usize] = parallel(0.0, 4.0);
        r.curves[inner as usize] = round(centre, hole, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        r.surfaces[0] = level(0.0);
        r
    };
    for rim_first in [true, false] {
        let faults = at(&ring([3.0, 0.0, 0.0], rim_first), BodyKind::Sheet, 1);
        assert!(only(&faults, FaultKind::PairUnchecked), "{faults:?}");
        // The same hole at the rim's centre lies certainly inside it, though 4 − s is not a
        // double either.
        assert_eq!(at(&ring([0.0; 3], rim_first), BodyKind::Sheet, 1), []);
    }
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
fn a_seam_from_a_rim_vertex_off_the_rims_plane_is_not_passed() {
    // A cone whose radius at height v is 6 − v, its apex at (0, 0, 6) and its rim the circle of
    // radius 6 on z = 0. The rim's vertex (7, 0, −1) lies on the cone, a step below the rim's
    // plane and √2 from the rim, within a bound of 2. The seam from it to the apex runs along the
    // cone and crosses the rim's plane a second time, at (6, 0, 0), on the rim (issue #1036). Read
    // only as leaving the plane from the shared vertex, it was passed.
    let mut r = Rows::empty();
    let solid = r.region(RegionKind::Solid);
    let (base, apex) = (r.vertex([7, 0, -1]), r.vertex([0, 0, 6]));
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
            -1.0,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (-1.0, 6.0)),
        )
        .unwrap(),
    );
    r.surfaces[1] = level(0.0);
    let faults = at(&r, BodyKind::Solid, 2);
    assert!(only(&faults, FaultKind::PairUnchecked), "{faults:?}");
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

/// The cylinder of radius 4 about the vertical line through (`x`, `y`), for heights 0 to 10.
fn shaft(x: f64, y: f64) -> SurfaceKind {
    SurfaceKind::Cylinder(
        Cylinder::new(
            [x, y, 0.0],
            4.0,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (0.0, 10.0)),
        )
        .unwrap(),
    )
}

/// The box round the shaft about (`x`, `y`).
fn shaft_box(x: i64, y: i64) -> Box3 {
    Box3::new(
        P3::new(x - 4, y - 4, 0).unwrap(),
        P3::new(x + 4, y + 4, 10).unwrap(),
    )
    .unwrap()
}

/// A wall 40 by 30 by 10 with a round opening of radius 4 through it about the vertical line
/// through (`x`, `y`), as bldg-25 and bldg-26 are made: the box's six faces in `CUBE_FACES`' order,
/// the bottom (face 0) and the top (face 1) each holding the opening's circle as a hole, and the
/// opening's cylindrical face (face 6) between the two circles, facing its axis.
fn wall_with_opening(x: i64, y: i64) -> Rows {
    wall_of([40, 30, 10], ([x, y], 4), 10.0)
}

/// A wall of `size` with a round opening through it about the vertical line through `centre`,
/// of `radius`, as [`wall_with_opening`] makes it, its top circle at the height `top`.
fn wall_of(size: [i64; 3], opening: ([i64; 2], i64), top: f64) -> Rows {
    slab_of(([0; 2], size), opening, (top, 1.0))
}

/// A slab of `size` from the corner (`from`, 0) with a round opening through it about the
/// vertical line through `centre`, of `radius`, as [`wall_with_opening`] makes it: its top circle
/// at the height `top`, and both circles' normals `up` along z (1 or −1).
fn slab_of(
    (from, size): ([i64; 2], [i64; 3]),
    (centre, radius): ([i64; 2], i64),
    (top, up): (f64, f64),
) -> Rows {
    let corners = CUBE.map(|[a, b, c]| {
        [
            from[0] + a / 10 * size[0],
            from[1] + b / 10 * size[1],
            c / 10 * size[2],
        ]
    });
    let mut r = polyhedron(&corners, &CUBE_FACES);
    let (low, high) = (r.edge(None), r.edge(None));
    let [x, y] = centre;
    let (cx, cy, rr) = (x as f64, y as f64, radius as f64);
    r.curves[low as usize] = round([cx, cy, 0.0], rr, [1.0, 0.0, 0.0], [0.0, 0.0, up]);
    r.curves[high as usize] = round([cx, cy, top], rr, [1.0, 0.0, 0.0], [0.0, 0.0, up]);
    // Seen from outside, the bottom from below and the top from above, each runs round its hole
    // clockwise.
    let (way, back) = if up > 0.0 {
        (Sense::Forward, Sense::Reversed)
    } else {
        (Sense::Reversed, Sense::Forward)
    };
    add_loop_as(&mut r, 0, low, way);
    add_loop_as(&mut r, 1, high, back);
    let shaft = SurfaceKind::Cylinder(
        Cylinder::new(
            [cx, cy, 0.0],
            rr,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (0.0, size[2] as f64)),
        )
        .unwrap(),
    );
    let around = Box3::new(
        P3::new(x - radius, y - radius, 0).unwrap(),
        P3::new(x + radius, y + radius, size[2]).unwrap(),
    )
    .unwrap();
    let bore = r.face_with(0, &[&[(low, back)], &[(high, way)]], shaft, around);
    r.faces[bore as usize].sense = Sense::Reversed;
    r
}

/// The same wall with its opening through its ends instead, about the line y = 15, z = 5 along x:
/// the faces holding the circles (4, x = 0, and 5, x = 40) come after the four they meet along
/// the box's edges, and the cylinder (face 6) runs along all four of those faces' planes.
fn wall_with_opening_along_x() -> Rows {
    let corners = CUBE.map(|[a, b, c]| [a * 4, b * 3, c]);
    let mut r = polyhedron(&corners, &CUBE_FACES);
    let (near, far) = (r.edge(None), r.edge(None));
    let (across, along) = ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0]);
    r.curves[near as usize] = round([0.0, 15.0, 5.0], 4.0, across, along);
    r.curves[far as usize] = round([40.0, 15.0, 5.0], 4.0, across, along);
    add_loop_as(&mut r, 4, near, Sense::Forward);
    add_loop_as(&mut r, 5, far, Sense::Reversed);
    let surface = SurfaceKind::Cylinder(
        Cylinder::new(
            [0.0, 15.0, 5.0],
            4.0,
            Frame::new(across, along).unwrap(),
            ((0.0, 2.0), (0.0, 40.0)),
        )
        .unwrap(),
    );
    let bound = Box3::new(P3::new(0, 11, 1).unwrap(), P3::new(40, 19, 9).unwrap()).unwrap();
    let side = r.face_with(
        0,
        &[&[(near, Sense::Reversed)], &[(far, Sense::Forward)]],
        surface,
        bound,
    );
    r.faces[side as usize].sense = Sense::Reversed;
    r
}

#[test]
fn a_wall_with_a_round_opening_passes() {
    // bldg-25's and bldg-26's shape: each face holding the circle lies on its plane exactly, and
    // each side face crosses that plane only along the edge they share; the opening's cylinder is
    // farther from each side's plane, which runs along its axis, than its radius.
    assert_eq!(at(&wall_with_opening(20, 15), BodyKind::Solid, 1), []);
    assert_eq!(at(&wall_with_opening(20, 15), BodyKind::Solid, 0), []);
    // Two steps clear of the side x = 0, and of y = 0 and y = 30 in turn.
    assert_eq!(at(&wall_with_opening(6, 15), BodyKind::Solid, 1), []);
    assert_eq!(at(&wall_with_opening(20, 6), BodyKind::Solid, 1), []);
    assert_eq!(at(&wall_with_opening(34, 24), BodyKind::Solid, 1), []);
    // Through its ends: each face holding a circle listed after the sides it meets, and the
    // cylinder 5 from the top and bottom and 15 from the front and back.
    assert_eq!(at(&wall_with_opening_along_x(), BodyKind::Solid, 1), []);
    // bldg-25's own numbers on its grid (1.6e8 steps to the metre): a wall 4 by 3 by 0.2 with an
    // opening of radius 0.4, and its top circle's centre 7.5e-9 steps above the top's plane, as
    // the conversion writes solidcore's 0.20000000000000004, within the body's bound of 3202
    // steps. The top is not on its plane exactly, and each side, which it reaches only along
    // their shared edge, is still apart from it.
    let bldg_25 = wall_of(
        [640_000_000, 480_000_000, 32_000_000],
        ([320_000_000, 240_000_000], 64_000_000),
        32_000_000.000_000_007,
    );
    assert_eq!(at(&bldg_25, BodyKind::Solid, 3202), []);
    // bldg-47's solid made exact, as lane sc8-fork set every number of solidcore's record to its
    // quarter-metre: the square of side 2 about the z axis with a round hole of radius 0.5 at its
    // centre, extruded 3, on the same grid, the hole's circles on the frame x (1, 0, 0),
    // y (0, -1, 0) the modeller writes, so their normals point down. Its caps and its hole's
    // cylinder were refused against each of the four sides, 12 pairs, and pass.
    let bldg_47 = slab_of(
        ([-160_000_000; 2], [320_000_000, 320_000_000, 480_000_000]),
        ([0, 0], 80_000_000),
        (480_000_000.0, -1.0),
    );
    assert_eq!(at(&bldg_47, BodyKind::Solid, 3202), []);
    assert_eq!(at(&bldg_47, BodyKind::Solid, 1), []);
    // The opening touching the side x = 0: its cylinder and that side are not certified apart,
    // nor the circles and the sides' edges they touch.
    let faults = at(&wall_with_opening(4, 15), BodyKind::Solid, 1);
    assert!(only(&faults, FaultKind::PairUnchecked), "{faults:?}");
    assert!(
        faults.contains(&fault(
            FaultKind::PairUnchecked,
            Entity::Face(ix(4)),
            Entity::Face(ix(6))
        )),
        "{faults:?}"
    );
    assert!(
        !faults.iter().any(|f| f.at == Entity::Face(ix(0))),
        "the bottom holds the circle on its plane and is apart from every side: {faults:?}"
    );
}

/// The square of side 10 with a round hole of radius 2 at its centre, in z = 0, and a second
/// sheet: a square in the plane x = 5 across it, from height `low` to `low` + 10.
fn holed_beside(low: i64) -> Rows {
    let mut r = holed_sheet();
    for p in [
        [5, 0, low],
        [5, 10, low],
        [5, 10, low + 10],
        [5, 0, low + 10],
    ] {
        r.vertex(p);
    }
    let s = r.shell(0);
    face_of(&mut r, s, &[&[4, 5, 6, 7]]);
    r
}

#[test]
fn a_flat_face_holding_a_circle_is_apart_from_a_face_that_keeps_off_its_plane() {
    let only_pair = [fault(
        FaultKind::PairUnchecked,
        Entity::Face(ix(0)),
        Entity::Face(ix(1)),
    )];
    // A step above the holed square's plane, and a step below it: apart, exactly.
    assert_eq!(at(&holed_beside(1), BodyKind::General, 1), []);
    assert_eq!(at(&holed_beside(-11), BodyKind::General, 1), []);
    assert_eq!(at(&holed_beside(1), BodyKind::General, 0), []);
    // Standing on the plane, along a line the holed square holds but has no edge on, and through
    // it: they meet, and nothing certifies them apart.
    assert_eq!(at(&holed_beside(0), BodyKind::General, 1), only_pair);
    assert_eq!(at(&holed_beside(-5), BodyKind::General, 1), only_pair);
    // The hole lifted a step, within the bound of its plane but off it: the holed face is no
    // longer on its plane exactly, and its circle reaches the square above, which it meets.
    let mut r = holed_beside(1);
    r.curves[4] = round([5.0, 5.0, 1.0], 2.0, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    assert_eq!(at(&r, BodyKind::General, 1), only_pair);
    // Tilted by a step at its rim instead: off its plane again.
    let mut r = holed_beside(1);
    r.curves[4] = round([5.0, 5.0, 0.0], 2.0, [1.0, 0.0, 0.0], [0.0, 0.5, 1.0]);
    assert_eq!(at(&r, BodyKind::General, 1), only_pair);
}

/// The holed square ([`holed_sheet`]) and a second sheet, a quadrilateral whose corners are the
/// holed square's vertices numbered in `shared` and new vertices at `points`, in that order.
fn holed_with(shared: &[u32], points: &[[i64; 3]]) -> Rows {
    let mut r = holed_sheet();
    let mut ring = shared.to_vec();
    for &p in points {
        ring.push(r.vertex(p));
    }
    let s = r.shell(0);
    face_of(&mut r, s, &[&ring]);
    r
}

#[test]
fn a_flat_face_holding_a_circle_keeps_off_a_plane_only_where_certified() {
    let only_pair = [fault(
        FaultKind::PairUnchecked,
        Entity::Face(ix(0)),
        Entity::Face(ix(1)),
    )];
    // Beyond the holed square, a square on the plane x + y = 20 whose side passes through its
    // corner (10, 10, 0), which it does not share: they touch there.
    let touching = holed_with(&[], &[[10, 10, -5], [20, 0, -5], [20, 0, 5], [10, 10, 5]]);
    assert_eq!(at(&touching, BodyKind::General, 1), only_pair);
    // A step further out, x + y = 21: the holed square's corners and its circle all lie below.
    let clear = holed_with(&[], &[[11, 10, -5], [21, 0, -5], [21, 0, 5], [11, 10, 5]]);
    assert_eq!(at(&clear, BodyKind::General, 1), []);
    // A square standing on the holed square's far side, x = 10, sharing its two vertices but not
    // the edge between them: they meet along it, and neither test excuses that.
    let standing = holed_with(&[1, 2], &[[10, 10, 10], [10, 0, 10]]);
    assert_eq!(at(&standing, BodyKind::General, 1), only_pair);
    // A notched face holding a circle, and a square on x + y = 10 sharing its corners (10, 0, 0)
    // and (0, 10, 0), the side between them the square's alone, outside the notch: what the face
    // has on the plane is two vertices and no side, so the segment between them is among the
    // mixes of its boundary there, which the square need not share. Not settled.
    let mut r = Rows::empty();
    for p in [[0, 0, 0], [10, 0, 0], [5, 3, 0], [0, 10, 0]] {
        r.vertex(p);
    }
    let sides: Vec<(u32, Sense)> = (0..4)
        .map(|k| (r.edge(Some((k, (k + 1) % 4))), Sense::Forward))
        .collect();
    let hole = r.edge(None);
    r.curves[hole as usize] = round([2.0, 2.0, 0.0], 1.0, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    let s = r.shell(0);
    r.face(s, &[&sides, &[(hole, Sense::Reversed)]]);
    let (over_x, over_y) = (r.vertex([10, 0, 10]), r.vertex([0, 10, 10]));
    let s = r.shell(0);
    face_of(&mut r, s, &[&[1, 3, over_y, over_x]]);
    assert_eq!(at(&r, BodyKind::General, 1), only_pair);
    // A disc of radius 5 about the z axis, no corner at all, and a square across its plane on
    // x = `x`: at its rim on either side it touches the circle, and a step beyond it is apart.
    for (x, faults) in [
        (5, &only_pair[..]),
        (-5, &only_pair[..]),
        (6, &[][..]),
        (-6, &[][..]),
    ] {
        let mut r = disc_sheet();
        r.curves[0] = parallel(0.0, 5.0);
        r.surfaces[0] = level(0.0);
        for p in [[x, -5, -5], [x, 5, -5], [x, 5, 5], [x, -5, 5]] {
            r.vertex(p);
        }
        let s = r.shell(0);
        face_of(&mut r, s, &[&[0, 1, 2, 3]]);
        assert_eq!(at(&r, BodyKind::General, 1), faults, "{x}");
    }
    // The square beyond it on x + y = 21 again, the hole now a line edge without vertices: an edge
    // of another kind, which the test does not settle, so the pair is unchecked.
    let mut r = holed_with(&[], &[[11, 10, -5], [21, 0, -5], [21, 0, 5], [11, 10, 5]]);
    r.curves[4] = common::line([5, 5, 0], [6, 5, 0]);
    let faults = at(&r, BodyKind::General, 1);
    assert!(only(&faults, FaultKind::PairUnchecked), "{faults:?}");
    assert!(faults.contains(&only_pair[0]), "{faults:?}");
    // A holed face with a third corner in its side x = 10, and a square standing on that side
    // sharing its two edges there. On its plane exactly, the holed face rests on the square's
    // plane only along what they share, and passes; its hole lifted half a step, within the bound
    // but off the plane, it no longer rests, and three corners on the square's plane are more than
    // one side's ends, so it is not settled.
    assert_eq!(at(&split_side_standing(0.0), BodyKind::General, 1), []);
    assert_eq!(
        at(&split_side_standing(0.5), BodyKind::General, 1),
        only_pair
    );
}

/// A square of side 10 in z = 0 with a third corner (10, 5, 0) on its side x = 10 and a round hole
/// of radius 2 about (5, 5, `lift`), and a square standing on that side, on x = 10 up to z = 10,
/// through the three corners on it and along the holed face's two edges between them.
fn split_side_standing(lift: f64) -> Rows {
    let mut r = Rows::empty();
    for p in [[0, 0, 0], [10, 0, 0], [10, 5, 0], [10, 10, 0], [0, 10, 0]] {
        r.vertex(p);
    }
    let sides: Vec<(u32, Sense)> = (0..5)
        .map(|k| (r.edge(Some((k, (k + 1) % 5))), Sense::Forward))
        .collect();
    let hole = r.edge(None);
    r.curves[hole as usize] = round([5.0, 5.0, lift], 2.0, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    let s = r.shell(0);
    r.face(s, &[&sides, &[(hole, Sense::Reversed)]]);
    r.surfaces[0] = level(0.0);
    let (near, far) = (r.vertex([10, 0, 10]), r.vertex([10, 10, 10]));
    let up = r.edge(Some((1, near)));
    let over = r.edge(Some((near, far)));
    let down = r.edge(Some((far, 3)));
    let s = r.shell(0);
    r.face(
        s,
        &[&[
            (sides[2].0, Sense::Reversed),
            (sides[1].0, Sense::Reversed),
            (up, Sense::Forward),
            (over, Sense::Forward),
            (down, Sense::Forward),
        ]],
    );
    r
}

/// A tube on `surface`, from its circle `low` to its circle `high`, and a square sheet with
/// corners at `square` on the plane its corners fix: one general body of two sheets.
fn tube_beside(
    surface: SurfaceKind,
    (low, high): (CurveKind<3>, CurveKind<3>),
    square: [[i64; 3]; 4],
) -> Rows {
    let mut r = Rows::empty();
    let (bottom, top) = (r.edge(None), r.edge(None));
    r.curves[bottom as usize] = low;
    r.curves[top as usize] = high;
    let s = r.shell(0);
    r.face_with(
        s,
        &[&[(bottom, Sense::Forward)], &[(top, Sense::Reversed)]],
        surface,
        shaft_box(0, 0),
    );
    for p in square {
        r.vertex(p);
    }
    let s = r.shell(0);
    face_of(&mut r, s, &[&[0, 1, 2, 3]]);
    r
}

/// The shaft of radius 4 about the z axis beside the square in the plane x = `x`.
fn shaft_beside(x: i64) -> Rows {
    tube_beside(
        shaft(0.0, 0.0),
        (parallel(0.0, 4.0), parallel(10.0, 4.0)),
        [[x, -10, 0], [x, 10, 0], [x, 10, 10], [x, -10, 10]],
    )
}

#[test]
fn a_cylinder_and_a_plane_along_its_axis_are_apart_only_beyond_its_radius() {
    let only_pair = [fault(
        FaultKind::PairUnchecked,
        Entity::Face(ix(0)),
        Entity::Face(ix(1)),
    )];
    // A step beyond the radius on either side, exactly; and across y.
    assert_eq!(at(&shaft_beside(5), BodyKind::General, 1), []);
    assert_eq!(at(&shaft_beside(-5), BodyKind::General, 1), []);
    assert_eq!(at(&shaft_beside(5), BodyKind::General, 0), []);
    let across = tube_beside(
        shaft(0.0, 0.0),
        (parallel(0.0, 4.0), parallel(10.0, 4.0)),
        [[-10, 5, 0], [-10, 5, 10], [10, 5, 10], [10, 5, 0]],
    );
    assert_eq!(at(&across, BodyKind::General, 1), []);
    // At the radius, touching along a line, and inside it, across the tube: not apart.
    assert_eq!(at(&shaft_beside(4), BodyKind::General, 1), only_pair);
    assert_eq!(at(&shaft_beside(3), BodyKind::General, 1), only_pair);
    // A plane leaning a step over the height of 1000, which the tube's own height does not reach:
    // not along the axis exactly, so it meets the cylinder somewhere, and nothing is certified.
    let leaning = tube_beside(
        shaft(0.0, 0.0),
        (parallel(0.0, 4.0), parallel(10.0, 4.0)),
        [[5, -10, 0], [5, 10, 0], [6, 10, 1000], [6, -10, 1000]],
    );
    assert_eq!(at(&leaning, BodyKind::General, 1), only_pair);
    // A cone narrowing from radius 6 to 4 over heights 0 to 8, and a plane along its axis beyond
    // both: the cone widens below and meets it.
    let cone = SurfaceKind::Cone(
        Cone::new(
            [0.0; 3],
            6.0,
            -0.25,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (0.0, 8.0)),
        )
        .unwrap(),
    );
    let frustum = tube_beside(
        cone,
        (parallel(0.0, 6.0), parallel(8.0, 4.0)),
        [[7, -10, 0], [7, 10, 0], [7, 10, 8], [7, -10, 8]],
    );
    assert_eq!(at(&frustum, BodyKind::General, 1), only_pair);
    // A square written on x = 6 with a corner a step off it, within the bound of 1: it may stray
    // a step, which still clears the radius; written on x = 5, it may reach the cylinder.
    for (x, faults) in [(6, &[][..]), (5, &only_pair[..])] {
        let mut r = tube_beside(
            shaft(0.0, 0.0),
            (parallel(0.0, 4.0), parallel(10.0, 4.0)),
            [[x, -10, 0], [x, 10, 0], [x + 1, 10, 10], [x, -10, 10]],
        );
        r.surfaces[1] = plane_off_grid([x as f64, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
        assert_eq!(at(&r, BodyKind::General, 1), faults, "{x}");
    }
}


/// The whole sphere about `center` of `radius`.
fn globe(center: [f64; 3], radius: f64) -> SurfaceKind {
    SurfaceKind::Sphere(
        Sphere::new(
            center,
            radius,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (-0.5, 0.5)),
        )
        .unwrap(),
    )
}

/// The box of the sphere about `center` of `radius`, rounded out to the grid.
fn globe_box(center: [f64; 3], radius: f64) -> Box3 {
    let corner = |by: f64, out: fn(f64) -> f64| {
        let [x, y, z] = center.map(|c| out(c + by) as i64);
        P3::new(x, y, z).unwrap()
    };
    Box3::new(corner(-radius, f64::floor), corner(radius, f64::ceil)).unwrap()
}

/// A solid of whole spheres as the kernel writes a ball, one face with no loops: a shell for each
/// (centre, radius, sense), its face run the way the sense says (a cavity's reversed).
fn globes(each: &[([f64; 3], f64, Sense)]) -> Rows {
    let mut r = Rows::empty();
    let solid = r.region(RegionKind::Solid);
    for &(center, radius, sense) in each {
        let s = r.shell(solid);
        let f = r.face_with(s, &[], globe(center, radius), globe_box(center, radius));
        r.faces[f as usize].sense = sense;
    }
    r
}

/// A ball about `outer`'s centre of its radius with a cavity, the sphere `inner`, the cavity's
/// shell listed first when `cavity_first`.
fn hollow(outer: ([f64; 3], f64), inner: ([f64; 3], f64), cavity_first: bool) -> Rows {
    let (out, cavity) = (
        (outer.0, outer.1, Sense::Forward),
        (inner.0, inner.1, Sense::Reversed),
    );
    if cavity_first {
        globes(&[cavity, out])
    } else {
        globes(&[out, cavity])
    }
}

#[test]
fn a_hollow_ball_passes() {
    // bool-26 on its grid (radii 1 and 0.5 of 1.6e8 steps, about one centre) at the converted
    // body's bound, and small ones, a step thick, at bounds of 0 and 1.
    for (outer, inner, bound) in [
        (160_000_000.0, 80_000_000.0, 3202),
        (10.0, 5.0, 1),
        (10.0, 9.0, 0),
        (10.0, 9.0, 1),
    ] {
        for cavity_first in [false, true] {
            let r = hollow(([0.0; 3], outer), ([0.0; 3], inner), cavity_first);
            assert_eq!(
                at(&r, BodyKind::Solid, bound),
                [],
                "{outer} {inner} {bound}"
            );
        }
    }
    // Off the centre, nearer the wall than the radii's difference: by 5 (3, 4, 0) inside a wall
    // 6 thick, and by √0.3125 off the grid inside one 1 thick.
    for inner in [([3.0, 4.0, 0.0], 4.0), ([0.5, 0.25, 0.0], 9.0)] {
        for cavity_first in [false, true] {
            let r = hollow(([0.0; 3], 10.0), inner, cavity_first);
            assert_eq!(at(&r, BodyKind::Solid, 1), [], "{inner:?}");
        }
    }
}

#[test]
fn two_balls_each_outside_the_other_pass() {
    // A step apart along an axis, and a step apart along (3, 4, 0).
    for (center, radius) in [([11.0, 0.0, 0.0], 5.0), ([6.0, 8.0, 0.0], 4.0)] {
        let r = globes(&[
            ([0.0; 3], 5.0, Sense::Forward),
            (center, radius, Sense::Forward),
        ]);
        assert_eq!(at(&r, BodyKind::Solid, 1), [], "{center:?}");
    }
}

#[test]
fn two_spheres_that_touch_cross_or_coincide_stay_unchecked() {
    let unchecked = |r: &Rows, what: &str| {
        let faults = at(r, BodyKind::General, 1);
        assert_eq!(
            faults,
            [fault(
                FaultKind::PairUnchecked,
                Entity::Face(ix(0)),
                Entity::Face(ix(1))
            )],
            "{what}"
        );
    };
    let pair = |first: ([f64; 3], f64), second: ([f64; 3], f64)| {
        globes(&[
            (first.0, first.1, Sense::Forward),
            (second.0, second.1, Sense::Forward),
        ])
    };
    // Touching from outside, along an axis and along (3, 4, 0): the distance is the radii's sum.
    unchecked(&pair(([0.0; 3], 5.0), ([10.0, 0.0, 0.0], 5.0)), "touching");
    unchecked(
        &pair(([0.0; 3], 5.0), ([6.0, 8.0, 0.0], 5.0)),
        "touching, slant",
    );
    // Touching from inside: the distance is the radii's difference.
    unchecked(&pair(([0.0; 3], 10.0), ([5.0, 0.0, 0.0], 5.0)), "inside");
    unchecked(
        &pair(([0.0; 3], 10.0), ([3.0, 4.0, 0.0], 5.0)),
        "inside, slant",
    );
    // Crossing, from outside and from inside.
    unchecked(&pair(([0.0; 3], 5.0), ([6.0, 0.0, 0.0], 5.0)), "crossing");
    unchecked(
        &pair(([0.0; 3], 10.0), ([6.0, 0.0, 0.0], 5.0)),
        "crossing inside",
    );
    // One sphere twice, about the origin and about a point off it.
    unchecked(&pair(([0.0; 3], 5.0), ([0.0; 3], 5.0)), "equal");
    unchecked(
        &pair(([1.0, 2.0, 3.0], 7.0), ([1.0, 2.0, 3.0], 7.0)),
        "equal, off",
    );
}

/// A cap of the sphere of radius 10 about the origin, above a circle at z = 6 of radius `rim` (8
/// is on the sphere, 8.5 lies about 0.4 off it), and the whole sphere `other`: two sheets of a
/// general body.
fn cap_and_globe(rim: f64, other: ([f64; 3], f64)) -> Rows {
    let mut r = Rows::empty();
    let edge = r.edge(None);
    r.curves[edge as usize] = parallel(6.0, rim);
    let s = r.shell(0);
    let top = Box3::new(P3::new(-10, -10, 6).unwrap(), P3::new(10, 10, 10).unwrap()).unwrap();
    r.face_with(s, &[&[(edge, Sense::Forward)]], globe([0.0; 3], 10.0), top);
    let t = r.shell(0);
    r.face_with(t, &[], globe(other.0, other.1), globe_box(other.0, other.1));
    r
}

#[test]
fn spheres_are_apart_only_beyond_what_their_faces_may_stray() {
    let unchecked = |r: &Rows, what: &str| {
        let faults = at(r, BodyKind::General, 1);
        assert_eq!(
            faults,
            [fault(
                FaultKind::PairUnchecked,
                Entity::Face(ix(0)),
                Entity::Face(ix(1))
            )],
            "{what}"
        );
    };
    // The cap's rim on its sphere: it strays by nothing, so a sphere half a step inside, or a step
    // outside, is apart.
    assert_eq!(
        at(&cap_and_globe(8.0, ([0.0; 3], 9.5)), BodyKind::General, 1),
        []
    );
    assert_eq!(
        at(
            &cap_and_globe(8.0, ([21.0, 0.0, 0.0], 10.0)),
            BodyKind::General,
            1
        ),
        []
    );
    unchecked(&cap_and_globe(8.0, ([20.0, 0.0, 0.0], 10.0)), "touching");
    // The rim 0.4 off it, within the bound of 1: the cap may lie a step off its sphere, so a
    // sphere within a step of it, inside or outside, is not apart.
    unchecked(&cap_and_globe(8.5, ([0.0; 3], 9.5)), "half a step inside");
    unchecked(&cap_and_globe(8.5, ([0.0; 3], 9.0)), "a step inside");
    unchecked(
        &cap_and_globe(8.5, ([21.0, 0.0, 0.0], 10.0)),
        "a step outside",
    );
    // Beyond the step, inside and outside, and beyond two steps.
    for other in [
        ([0.0; 3], 8.75),
        ([0.0; 3], 7.5),
        ([21.5, 0.0, 0.0], 10.0),
        ([23.0, 0.0, 0.0], 10.0),
    ] {
        assert_eq!(
            at(&cap_and_globe(8.5, other), BodyKind::General, 1),
            [],
            "{other:?}"
        );
    }
}
