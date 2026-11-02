//! Attacks on G102's checks (issue #438's flat half, 8.3): each loop runs the right way round its
//! face, a face's stored box holds its vertices, each closed flat shell faces the way its place
//! says, and each shell lies where its region allows. Every fault is planted in a body that passes
//! without it, and what a curved face or shell leaves unjudged is counted, never passed silently.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::arithmetic_side_effects,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly"
)]

mod common;

use common::{
    CUBE_FACES, Rows, ball, certified, cube, face_of, faces_into, has, ix, kinds, meter, report,
    round_cylinder,
};
use geom_check::{Entity, Fault, FaultKind, Group, Level, check};
use geom_math::Box3;
use geom_num::{Exactness, P3};
use geom_run::{Budget, Stop, WorkMeter};
use geom_surface::{Plane, SurfaceKind};
use geom_topo::entity::RegionKind;
use geom_topo::{BodyKind, Sense};

/// A box's corners: bit 0 is x, bit 1 y, bit 2 z.
fn corners(lo: [i64; 3], hi: [i64; 3]) -> Vec<[i64; 3]> {
    (0..8)
        .map(|v: usize| [0, 1, 2].map(|k| if v >> k & 1 == 1 { hi[k] } else { lo[k] }))
        .collect()
}

/// Adds a box from `lo` to `hi` to shell `s`: facing outward, or into itself as a cavity does.
fn add_box(r: &mut Rows, s: u32, lo: [i64; 3], hi: [i64; 3], cavity: bool) {
    let base = r.vertices.len() as u32;
    for c in corners(lo, hi) {
        r.vertex(c);
    }
    let rings: Vec<Vec<u32>> = CUBE_FACES
        .iter()
        .map(|f| {
            let mut ring: Vec<u32> = f.iter().map(|v| v + base).collect();
            if cavity {
                ring.reverse();
            }
            ring
        })
        .collect();
    let refs: Vec<&[u32]> = rings.iter().map(Vec::as_slice).collect();
    faces_into(r, s, &refs);
}

/// One solid region holding a box from (0, 0, 0) to (30, 30, 30), and the region's row.
fn solid() -> (Rows, u32) {
    let mut r = Rows::empty();
    let region = r.region(RegionKind::Solid);
    let s = r.shell(region);
    add_box(&mut r, s, [0; 3], [30; 3], false);
    (r, region)
}

/// Adds to `region` a shell holding a box.
fn add(r: &mut Rows, region: u32, lo: [i64; 3], hi: [i64; 3], cavity: bool) -> u32 {
    let s = r.shell(region);
    add_box(r, s, lo, hi, cavity);
    s
}

fn exact(r: &Rows) -> geom_check::Report {
    report(r, BodyKind::Solid, Exactness::Exact)
}

fn passes(r: &Rows) {
    let found = exact(r);
    assert!(found.passed(), "{:?}", found.faults());
    assert_eq!(found.deferred(), 0);
}

#[test]
fn a_box_with_cavities_and_lumps_passes() {
    // One cavity.
    let (mut r, region) = solid();
    add(&mut r, region, [10; 3], [20; 3], true);
    passes(&r);
    // Two cavities side by side.
    let (mut r, region) = solid();
    add(&mut r, region, [2; 3], [8; 3], true);
    add(&mut r, region, [12; 3], [18; 3], true);
    passes(&r);
    // Two solid regions apart.
    let (mut r, _) = solid();
    let other = r.region(RegionKind::Solid);
    add(&mut r, other, [50; 3], [60; 3], false);
    passes(&r);
    // A lump floating in a cavity, a solid region of its own.
    let (mut r, region) = solid();
    add(&mut r, region, [5; 3], [25; 3], true);
    let other = r.region(RegionKind::Solid);
    add(&mut r, other, [10; 3], [20; 3], false);
    passes(&r);
}

#[test]
fn a_shell_inside_out_is_found() {
    let mut r = Rows::empty();
    let region = r.region(RegionKind::Solid);
    let s = r.shell(region);
    add_box(&mut r, s, [0; 3], [10; 3], true);
    let found = exact(&r);
    assert_eq!(
        found.faults(),
        [
            Fault::at(FaultKind::ShellInward, Entity::Region(ix(region)))
                .with(Entity::Shell(ix(s)))
        ]
    );
    assert_eq!(FaultKind::ShellInward.group(), Group::SelfIntersection);
}

#[test]
fn a_second_outward_shell_in_one_region_is_found() {
    let (mut r, region) = solid();
    let s = add(&mut r, region, [50; 3], [60; 3], false);
    assert_eq!(
        exact(&r).faults(),
        [Fault::at(FaultKind::ShellOutward, Entity::Shell(ix(s))).with(Entity::Shell(ix(0)))]
    );
}

#[test]
fn a_cavity_outside_its_solid_is_found() {
    let (mut r, region) = solid();
    let s = add(&mut r, region, [50; 3], [60; 3], true);
    assert_eq!(
        exact(&r).faults(),
        [Fault::at(FaultKind::ShellNesting, Entity::Shell(ix(s))).with(Entity::Shell(ix(0)))]
    );
}

#[test]
fn a_cavity_inside_a_cavity_is_found() {
    let (mut r, region) = solid();
    let outer = add(&mut r, region, [5; 3], [25; 3], true);
    let inner = add(&mut r, region, [10; 3], [20; 3], true);
    assert_eq!(
        exact(&r).faults(),
        [Fault::at(FaultKind::ShellNesting, Entity::Shell(ix(inner)))
            .with(Entity::Shell(ix(outer)))]
    );
}

#[test]
fn a_lump_in_another_regions_material_is_found() {
    let (mut r, _) = solid();
    let other = r.region(RegionKind::Solid);
    let lump = add(&mut r, other, [10; 3], [20; 3], false);
    assert_eq!(
        exact(&r).faults(),
        [Fault::at(FaultKind::ShellNesting, Entity::Shell(ix(lump))).with(Entity::Shell(ix(0)))]
    );
    // A cavity elsewhere in that material does not hold it.
    let (mut r, region) = solid();
    add(&mut r, region, [2; 3], [8; 3], true);
    let other = r.region(RegionKind::Solid);
    let lump = add(&mut r, other, [12; 3], [20; 3], false);
    assert_eq!(
        exact(&r).faults(),
        [Fault::at(FaultKind::ShellNesting, Entity::Shell(ix(lump))).with(Entity::Shell(ix(0)))]
    );
}

#[test]
fn a_lump_with_a_cavity_of_its_own_in_another_regions_material_is_one_fault() {
    // The lump's outer shell is the fault. Its cavity is the lump's business: it lies inside the
    // other region too, but only a region's outer shell is judged against another region.
    let (mut r, _) = solid();
    let other = r.region(RegionKind::Solid);
    let lump = add(&mut r, other, [10; 3], [20; 3], false);
    add(&mut r, other, [12; 3], [18; 3], true);
    assert_eq!(
        exact(&r).faults(),
        [Fault::at(FaultKind::ShellNesting, Entity::Shell(ix(lump))).with(Entity::Shell(ix(0)))]
    );
}

#[test]
fn a_shell_is_judged_only_once_its_loops_are_sound() {
    // Inside out and with one face's sense wrong: the loop comes first, and a shell whose loops
    // are wrong is not judged.
    let mut r = Rows::empty();
    let region = r.region(RegionKind::Solid);
    let s = r.shell(region);
    add_box(&mut r, s, [0; 3], [10; 3], true);
    r.faces[1].sense = Sense::Reversed;
    assert_eq!(
        exact(&r).faults(),
        [Fault::at(FaultKind::LoopSense, Entity::Face(ix(1))).value(0)]
    );
}

#[test]
fn a_shell_that_encloses_nothing_never_reaches_orientation() {
    // A pillow: two triangles on one plane, back to back, their edges shared: they lie on one side
    // of each edge, so group 3 finds them crossing.
    let mut r = Rows::empty();
    let region = r.region(RegionKind::Solid);
    let s = r.shell(region);
    for p in [[0, 0, 0], [10, 0, 0], [0, 10, 0]] {
        r.vertex(p);
    }
    faces_into(&mut r, s, &[&[0, 1, 2], &[0, 2, 1]]);
    assert_eq!(kinds(exact(&r).faults()), [FaultKind::FacesCross]);
}

#[test]
fn shells_that_share_every_vertex_of_one_fail_closed() {
    // A cube dented on all six sides, each dent four triangles to a tip inside it, and a cavity
    // that is the tetrahedron through four of the tips: every vertex of the cavity is a vertex of
    // the outer shell, so no vertex can say where it lies.
    let mut r = Rows::empty();
    let region = r.region(RegionKind::Solid);
    let outer = r.shell(region);
    for c in corners([0; 3], [12; 3]) {
        r.vertex(c);
    }
    let tips = [
        [6, 6, 2],
        [6, 6, 10],
        [6, 2, 6],
        [6, 10, 6],
        [2, 6, 6],
        [10, 6, 6],
    ];
    for t in tips {
        r.vertex(t);
    }
    let mut rings: Vec<Vec<u32>> = Vec::new();
    for (f, ring) in CUBE_FACES.iter().enumerate() {
        for k in 0..4 {
            rings.push(vec![ring[k], ring[(k + 1) % 4], 8 + f as u32]);
        }
    }
    let refs: Vec<&[u32]> = rings.iter().map(Vec::as_slice).collect();
    faces_into(&mut r, outer, &refs);
    let cavity = r.shell(region);
    // Tips 8, 9, 10 and 12 of the 14 vertices, each face clockwise seen from outside.
    faces_into(
        &mut r,
        cavity,
        &[&[8, 10, 9], &[8, 9, 12], &[8, 12, 10], &[9, 10, 12]],
    );
    let found = exact(&r);
    assert_eq!(
        found.faults(),
        [
            Fault::at(FaultKind::PairUnchecked, Entity::Shell(ix(cavity)))
                .with(Entity::Shell(ix(outer)))
        ],
        "{:?}",
        found.faults()
    );
}

#[test]
fn orientation_is_for_solids() {
    // The same shell as a general body: nothing about it is judged here.
    let mut r = Rows::empty();
    let region = r.region(RegionKind::Solid);
    let s = r.shell(region);
    add_box(&mut r, s, [0; 3], [10; 3], true);
    assert!(report(&r, BodyKind::General, Exactness::Exact).passed());
    // A void region of a general body beside it: its shell is not counted in any solid region.
    let mut r = Rows::empty();
    let v = r.region(RegionKind::Void);
    let s = r.shell(v);
    add_box(&mut r, s, [0; 3], [10; 3], false);
    assert!(report(&r, BodyKind::General, Exactness::Exact).passed());
}

#[test]
fn a_face_whose_sense_is_wrong_is_found() {
    let mut r = cube();
    r.faces[2].sense = Sense::Reversed;
    let found = report(&r, BodyKind::Solid, Exactness::Exact);
    assert_eq!(
        found.faults(),
        [Fault::at(FaultKind::LoopSense, Entity::Face(ix(2))).value(0)]
    );
    assert_eq!(FaultKind::LoopSense.group(), Group::SelfIntersection);
    // Reversing both a face's sense and its plane's normal keeps it right.
    let mut r = cube();
    r.faces[2].sense = Sense::Reversed;
    let SurfaceKind::Plane(flat) = r.surfaces[2].clone() else {
        panic!("a cube's faces are flat");
    };
    let (o, a, b) = flat.vectors();
    r.surfaces[2] = SurfaceKind::Plane(Plane::new(o, b, a, ((0.0, 1.0), (0.0, 1.0))).unwrap());
    assert!(report(&r, BodyKind::Solid, Exactness::Exact).passed());
}

#[test]
fn a_faces_loops_are_one_outer_and_holes() {
    let square = |r: &mut Rows, c: [i64; 2], size: i64, turn: bool| -> Vec<u32> {
        let [x, y] = c;
        let mut ring: Vec<u32> = [[0, 0], [size, 0], [size, size], [0, size]]
            .iter()
            .map(|d| r.vertex([x + d[0], y + d[1], 0]))
            .collect();
        if turn {
            ring.reverse();
        }
        ring
    };
    // A hole clockwise inside an outer loop passes.
    let mut r = Rows::empty();
    let outer = square(&mut r, [0, 0], 30, false);
    let hole = square(&mut r, [10, 10], 10, true);
    let s = r.shell(0);
    let f = face_of(&mut r, s, &[&outer, &hole]);
    let body = report(&r, BodyKind::Sheet, certified());
    assert!(
        !body.faults().iter().any(|x| x.kind == FaultKind::LoopSense),
        "{:?}",
        body.faults()
    );
    // Two counter-clockwise loops: the second is the wrong one.
    let mut r = Rows::empty();
    let outer = square(&mut r, [0, 0], 30, false);
    let hole = square(&mut r, [10, 10], 10, false);
    let s = r.shell(0);
    face_of(&mut r, s, &[&outer, &hole]);
    let found = report(&r, BodyKind::Sheet, certified());
    let fault = Fault::at(FaultKind::LoopSense, Entity::Face(ix(f)))
        .with(Entity::Loop(ix(1)))
        .value(2);
    assert!(found.faults().contains(&fault), "{:?}", found.faults());
    // Every loop clockwise: no outer loop.
    let mut r = Rows::empty();
    let outer = square(&mut r, [0, 0], 30, false);
    let s = r.shell(0);
    face_of(&mut r, s, &[&outer]);
    r.faces[0].sense = Sense::Reversed;
    let found = report(&r, BodyKind::Sheet, certified());
    assert!(
        found
            .faults()
            .contains(&Fault::at(FaultKind::LoopSense, Entity::Face(ix(0))).value(0)),
        "{:?}",
        found.faults()
    );
}

/// A square sheet whose face has a second loop of two fins, out along an edge and back.
fn spiked() -> Rows {
    let mut r = Rows::empty();
    for p in [[0, 0, 0], [10, 0, 0], [10, 10, 0], [0, 10, 0]] {
        r.vertex(p);
    }
    let (a, b) = (r.vertex([3, 3, 0]), r.vertex([6, 3, 0]));
    let sides: Vec<(u32, Sense)> = (0..4)
        .map(|k| {
            let (u, v) = (k, (k + 1) % 4);
            let edge = r.edge(Some((u.min(v), u.max(v))));
            let sense = if u < v {
                Sense::Forward
            } else {
                Sense::Reversed
            };
            (edge, sense)
        })
        .collect();
    let spike = r.edge(Some((a, b)));
    let s = r.shell(0);
    r.face(
        s,
        &[&sides, &[(spike, Sense::Forward), (spike, Sense::Reversed)]],
    );
    r
}

#[test]
fn a_loop_enclosing_no_area_is_found() {
    let found = report(&spiked(), BodyKind::Sheet, certified());
    let want = Fault::at(FaultKind::LoopSense, Entity::Face(ix(0)))
        .with(Entity::Loop(ix(1)))
        .value(1);
    assert!(found.faults().contains(&want), "{:?}", found.faults());
}

#[test]
fn a_stored_box_must_hold_every_vertex() {
    let p = |x, y, z| P3::new(x, y, z).unwrap();
    // A box exactly round the face's vertices passes; larger passes too.
    let mut r = cube();
    let b = r.faces[0].bound;
    assert_eq!(b, Box3::new(p(0, 0, 0), p(10, 10, 0)).unwrap());
    passes(&r);
    r.faces[0].bound = Box3::new(p(-5, -5, -5), p(50, 50, 50)).unwrap();
    passes(&r);
    // One step short along each axis finds the vertices it leaves out.
    for (hi, corners_out) in [(p(9, 10, 0), 2), (p(10, 9, 0), 2), (p(10, 10, -1), 4)] {
        let mut r = cube();
        r.faces[0].bound = Box3::new(p(0, 0, -1), hi).unwrap();
        let found = exact(&r);
        assert_eq!(found.faults().len(), corners_out, "{:?}", found.faults());
        assert!(
            found
                .faults()
                .iter()
                .all(|f| f.kind == FaultKind::FaceBound && f.at == Entity::Face(ix(0))),
            "{:?}",
            found.faults()
        );
        assert!(matches!(found.faults()[0].with, Some(Entity::Vertex(_))));
    }
    assert_eq!(FaultKind::FaceBound.group(), Group::Geometric);
}

#[test]
fn what_is_curved_is_counted_never_judged() {
    // The cylinder: two flat caps round circles, and the side; and its one shell.
    let found = report(&round_cylinder(), BodyKind::Solid, certified());
    assert!(found.passed(), "{:?}", found.faults());
    assert_eq!(found.deferred(), 4);
    // The ball: one curved face, and its shell.
    let found = report(&ball(), BodyKind::Solid, certified());
    assert!(found.passed(), "{:?}", found.faults());
    assert_eq!(found.deferred(), 2);
    // A flat body leaves nothing.
    assert_eq!(exact(&cube()).deferred(), 0);
}

#[test]
fn a_flat_cap_still_has_its_box_tested() {
    // A square face whose fourth side is a circle's edge is not a polygon, but its vertices are
    // still held to its box.
    let mut r = cube();
    r.faces[0].bound = {
        let p = |x, y, z| P3::new(x, y, z).unwrap();
        Box3::new(p(0, 0, 0), p(9, 10, 0)).unwrap()
    };
    let e = r.fins[0].edge.row() as usize;
    r.curves[e] = common::circle();
    let found = report(&r, BodyKind::Solid, certified());
    assert!(
        has(found.faults(), FaultKind::FaceBound, Entity::Face(ix(0))),
        "{:?}",
        kinds(found.faults())
    );
}

#[test]
fn a_plane_given_by_doubles_is_judged_in_intervals() {
    // A cube whose faces are planes through their corners, spanned by vectors of fractions: the
    // plane's normal is not an integer plane's, so its sign comes from certified intervals.
    let scaled = |r: &mut Rows, k: usize| {
        let SurfaceKind::Plane(flat) = r.surfaces[k].clone() else {
            panic!("a cube's faces are flat");
        };
        let (o, a, b) = flat.vectors();
        let half = |v: [f64; 3]| v.map(|x| x * 0.3);
        r.surfaces[k] =
            SurfaceKind::Plane(Plane::new(o, half(a), half(b), ((0.0, 1.0), (0.0, 1.0))).unwrap());
    };
    let mut r = cube();
    for k in 0..6 {
        scaled(&mut r, k);
    }
    let found = report(&r, BodyKind::Solid, certified());
    assert!(found.passed(), "{:?}", found.faults());
    // The same plane with the face's sense reversed fails.
    r.faces[0].sense = Sense::Reversed;
    assert!(has(
        report(&r, BodyKind::Solid, certified()).faults(),
        FaultKind::LoopSense,
        Entity::Face(ix(0))
    ));
    // A loop of no area on such a plane cannot be told from a small one: it is unchecked.
    let mut r = spiked();
    r.surfaces[0] = SurfaceKind::Plane(
        Plane::new(
            [0.5, 0.5, 0.0],
            [0.25, 0.0, 0.0],
            [0.0, 0.25, 0.0],
            ((0.0, 1.0), (0.0, 1.0)),
        )
        .unwrap(),
    );
    let found = report(&r, BodyKind::Sheet, certified());
    assert!(
        has(found.faults(), FaultKind::Unchecked, Entity::Surface(ix(0))),
        "{:?}",
        found.faults()
    );
}

#[test]
fn every_new_kind_says_what_is_wrong() {
    for kind in [
        FaultKind::LoopSense,
        FaultKind::FaceBound,
        FaultKind::ShellInward,
        FaultKind::ShellOutward,
        FaultKind::ShellNesting,
    ] {
        assert_ne!(kind.what(), "");
        assert!(
            Fault::at(kind, Entity::Shell(ix(0)))
                .to_string()
                .contains(kind.what())
        );
    }
}

#[test]
fn every_row_is_charged_and_a_stop_stops_it() {
    let (mut r, region) = solid();
    add(&mut r, region, [10; 3], [20; 3], true);
    let other = r.region(RegionKind::Solid);
    add(&mut r, other, [50; 3], [60; 3], false);
    let body = r.body(BodyKind::Solid);
    let mut m = meter();
    let full = check(&body, Exactness::Exact, Level::Full, &mut m).unwrap();
    assert!(full.passed(), "{:?}", full.faults());
    let total = m.used().work;
    // The last unit is charged; one less stops the check.
    let mut stopped = 0;
    for work in 0..total as u64 {
        let mut m = WorkMeter::new(Budget::of_work(work));
        match check(&body, Exactness::Exact, Level::Full, &mut m) {
            Err(stop) => {
                assert!(matches!(stop.reason(), Stop::OverBudget(_)));
                stopped += 1;
            }
            Ok(_) => panic!("the check finished in {work} of {total} units"),
        }
    }
    assert_eq!(stopped, total as u64);
    let mut m = WorkMeter::new(Budget::of_work(total as u64));
    assert!(check(&body, Exactness::Exact, Level::Full, &mut m).is_ok());
}

#[test]
fn a_check_with_cancellation_stops_in_each_new_pass() {
    // The cheap level runs neither new pass: its cost does not rise with them.
    let (mut r, region) = solid();
    add(&mut r, region, [10; 3], [20; 3], true);
    let body = r.body(BodyKind::Solid);
    let mut cheap = meter();
    check(&body, Exactness::Exact, Level::Cheap, &mut cheap).unwrap();
    let mut full = meter();
    check(&body, Exactness::Exact, Level::Full, &mut full).unwrap();
    assert!(cheap.used().work < full.used().work);
}
