//! Attacks on chapter 8.3's groups 2 and 3 for plane-based bodies (G134): vertices where three
//! planes meet, faces of any shape, on planes that cross, on one plane, and shells that nest.
//!
//! The polyhedra here have three faces at every vertex, so each can be tilted: every face's plane
//! moved a step or so off its grid points, and every vertex the meeting of its three planes, at a
//! place no grid point holds. Moving a plane so little changes no shape, so every tilted polyhedron
//! must pass, and each fault planted in one must be found, by name.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "test fixtures; the rules are for kernel code"
)]

mod common;

use common::{Rows, ix, line, meter};
use geom_check::{Entity, Fault, FaultKind, Level, Report, check};
use geom_curve::CurveKind;
use geom_math::Box3;
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_run::{Budget, WorkMeter};
use geom_topo::{Body, BodyKind, RegionKind, Sense};

/// A polyhedron: its corners, and for each solid region its shells, each a list of faces, each a
/// list of loops of corner numbers, counter-clockwise seen from outside and holes clockwise.
#[derive(Clone, Copy)]
struct Spec<'a> {
    points: &'a [[i64; 3]],
    regions: &'a [&'a [&'a [&'a [&'a [u32]]]]],
}

/// Side 100, corners by bits: bit 0 x, bit 1 y, bit 2 z.
const fn corners(o: [i64; 3], s: [i64; 3]) -> [[i64; 3]; 8] {
    let [x, y, z] = o;
    let [a, b, c] = s;
    [
        [x, y, z],
        [x + a, y, z],
        [x, y + b, z],
        [x + a, y + b, z],
        [x, y, z + c],
        [x + a, y, z + c],
        [x, y + b, z + c],
        [x + a, y + b, z + c],
    ]
}

const BOX_POINTS: [[i64; 3]; 8] = corners([0, 0, 0], [100, 100, 100]);

/// The faces of a box from corner `n`, counter-clockwise from outside.
macro_rules! box_faces {
    ($n:expr) => {
        [
            &[&[$n, $n + 2, $n + 3, $n + 1][..]][..],
            &[&[$n + 4, $n + 5, $n + 7, $n + 6][..]][..],
            &[&[$n, $n + 1, $n + 5, $n + 4][..]][..],
            &[&[$n + 2, $n + 6, $n + 7, $n + 3][..]][..],
            &[&[$n, $n + 4, $n + 6, $n + 2][..]][..],
            &[&[$n + 1, $n + 3, $n + 7, $n + 5][..]][..],
        ]
    };
}

/// The same box inside out, a cavity's shell.
macro_rules! cavity_faces {
    ($n:expr) => {
        [
            &[&[$n, $n + 1, $n + 3, $n + 2][..]][..],
            &[&[$n + 4, $n + 6, $n + 7, $n + 5][..]][..],
            &[&[$n, $n + 4, $n + 5, $n + 1][..]][..],
            &[&[$n + 2, $n + 3, $n + 7, $n + 6][..]][..],
            &[&[$n, $n + 2, $n + 6, $n + 4][..]][..],
            &[&[$n + 1, $n + 5, $n + 7, $n + 3][..]][..],
        ]
    };
}

const BOX_SHELL: [&[&[u32]]; 6] = box_faces!(0);

const BOX: Spec<'_> = Spec {
    points: &BOX_POINTS,
    regions: &[&[&BOX_SHELL]],
};

/// An L, extruded along z from 0 to 100: not convex, on either cap.
const ELL_POINTS: [[i64; 3]; 12] = [
    [0, 0, 0],
    [200, 0, 0],
    [200, 100, 0],
    [100, 100, 0],
    [100, 200, 0],
    [0, 200, 0],
    [0, 0, 100],
    [200, 0, 100],
    [200, 100, 100],
    [100, 100, 100],
    [100, 200, 100],
    [0, 200, 100],
];

const ELL_SHELL: [&[&[u32]]; 8] = [
    &[&[0, 5, 4, 3, 2, 1]],
    &[&[6, 7, 8, 9, 10, 11]],
    &[&[0, 1, 7, 6]],
    &[&[1, 2, 8, 7]],
    &[&[2, 3, 9, 8]],
    &[&[3, 4, 10, 9]],
    &[&[4, 5, 11, 10]],
    &[&[5, 0, 6, 11]],
];

const ELL: Spec<'_> = Spec {
    points: &ELL_POINTS,
    regions: &[&[&ELL_SHELL]],
};

/// A wall with a window through it: two faces with holes.
const WALL_POINTS: [[i64; 3]; 16] = [
    [0, 0, 0],
    [400, 0, 0],
    [0, 0, 300],
    [400, 0, 300],
    [0, 50, 0],
    [400, 50, 0],
    [0, 50, 300],
    [400, 50, 300],
    [100, 0, 100],
    [300, 0, 100],
    [100, 0, 200],
    [300, 0, 200],
    [100, 50, 100],
    [300, 50, 100],
    [100, 50, 200],
    [300, 50, 200],
];

const WALL_SHELL: [&[&[u32]]; 10] = [
    &[&[0, 1, 3, 2], &[8, 10, 11, 9]],
    &[&[4, 6, 7, 5], &[12, 13, 15, 14]],
    &[&[0, 4, 5, 1]],
    &[&[2, 3, 7, 6]],
    &[&[0, 2, 6, 4]],
    &[&[1, 5, 7, 3]],
    &[&[8, 9, 13, 12]],
    &[&[10, 14, 15, 11]],
    &[&[8, 12, 14, 10]],
    &[&[9, 11, 15, 13]],
];

const WALL: Spec<'_> = Spec {
    points: &WALL_POINTS,
    regions: &[&[&WALL_SHELL]],
};

/// A box and a second, apart.
const TWO_POINTS: [[i64; 3]; 16] = {
    let (a, b) = (
        corners([0, 0, 0], [100, 100, 100]),
        corners([300, 0, 0], [100, 100, 100]),
    );
    [
        a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], b[0], b[1], b[2], b[3], b[4], b[5], b[6],
        b[7],
    ]
};

const SECOND_SHELL: [&[&[u32]]; 6] = box_faces!(8);

const TWO: Spec<'_> = Spec {
    points: &TWO_POINTS,
    regions: &[&[&BOX_SHELL], &[&SECOND_SHELL]],
};

/// A box with a box-shaped cavity, and a third box floating in the cavity.
const NEST_POINTS: [[i64; 3]; 24] = {
    let (a, b, c) = (
        corners([0, 0, 0], [900, 900, 900]),
        corners([100, 100, 100], [700, 700, 700]),
        corners([300, 300, 300], [300, 300, 300]),
    );
    [
        a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], b[0], b[1], b[2], b[3], b[4], b[5], b[6],
        b[7], c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7],
    ]
};

const CAVITY_SHELL: [&[&[u32]]; 6] = cavity_faces!(8);
const FLOATING_SHELL: [&[&[u32]]; 6] = box_faces!(16);

const NEST: Spec<'_> = Spec {
    points: &NEST_POINTS,
    regions: &[&[&BOX_SHELL, &CAVITY_SHELL], &[&FLOATING_SHELL]],
};

const CORPUS: [Spec<'_>; 5] = [BOX, ELL, WALL, TWO, NEST];

/// Three corners of `ring` that turn the way it runs, by the ring's area vector.
fn anchors(points: &[[i64; 3]], ring: &[u32]) -> [[i64; 3]; 3] {
    let at = |v: u32| points[v as usize].map(i128::from);
    let cross = |p: [i128; 3], q: [i128; 3]| {
        [
            p[1] * q[2] - p[2] * q[1],
            p[2] * q[0] - p[0] * q[2],
            p[0] * q[1] - p[1] * q[0],
        ]
    };
    let minus = |p: [i128; 3], q: [i128; 3]| [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
    let n = ring.len();
    let mut area = [0_i128; 3];
    for k in 0..n {
        let c = cross(at(ring[k]), at(ring[(k + 1) % n]));
        area = [0, 1, 2].map(|i| area[i] + c[i]);
    }
    for k in 0..n {
        let (a, b, c) = (ring[k], ring[(k + 1) % n], ring[(k + 2) % n]);
        let turn = cross(minus(at(b), at(a)), minus(at(c), at(a)));
        if (0..3).map(|i| turn[i] * area[i]).sum::<i128>() > 0 {
            return [a, b, c].map(|v| points[v as usize]);
        }
    }
    panic!("a ring with no convex corner")
}

/// `spec`'s body, every vertex the meeting of its three faces' planes, each plane through its
/// face's anchors moved by `jog(face, anchor)`.
fn tilted(spec: Spec<'_>, jog: &dyn Fn(usize, usize) -> [i64; 3]) -> Rows {
    let mut r = Rows::empty();
    let faces: Vec<&[&[u32]]> = spec
        .regions
        .iter()
        .flat_map(|shells| shells.iter())
        .flat_map(|faces| faces.iter().copied())
        .collect();
    let mut on: Vec<[[i64; 3]; 3]> = Vec::new();
    for (k, loops) in faces.iter().enumerate() {
        let moved = anchors(spec.points, loops[0]);
        let mut a = 0;
        let moved = moved.map(|p| {
            let by = jog(k, a);
            a += 1;
            [p[0] + by[0], p[1] + by[1], p[2] + by[2]]
        });
        r.plane_through(moved);
        on.push(moved);
    }
    for v in 0..spec.points.len() as u32 {
        let mut planes = faces
            .iter()
            .enumerate()
            .filter(|(_, loops)| loops.iter().any(|ring| ring.contains(&v)))
            .map(|(k, _)| k as u32);
        let triple = [
            planes.next().unwrap(),
            planes.next().unwrap(),
            planes.next().unwrap(),
        ];
        assert_eq!(planes.next(), None, "three faces at a vertex");
        r.meet(triple);
    }
    let mut face = 0;
    for shells in spec.regions {
        let solid = r.region(RegionKind::Solid);
        for shell in *shells {
            let s = r.shell(solid);
            for loops in *shell {
                let rings: Vec<Vec<(u32, Sense)>> =
                    loops.iter().map(|ring| fins(&mut r, ring)).collect();
                let rings: Vec<&[(u32, Sense)]> = rings.iter().map(Vec::as_slice).collect();
                r.face_on(s, &rings, on[face]);
                face += 1;
            }
        }
    }
    r
}

/// The fins of `ring`, each edge made the first time its two vertices meet.
fn fins(r: &mut Rows, ring: &[u32]) -> Vec<(u32, Sense)> {
    let n = ring.len();
    (0..n)
        .map(|k| {
            let (a, b) = (ring[k], ring[(k + 1) % n]);
            let key = (a.min(b), a.max(b));
            let found = (0..r.edges.len()).find(|&e| {
                r.edges[e].start.map(|v| v.row()) == Some(key.0)
                    && r.edges[e].end.map(|v| v.row()) == Some(key.1)
            });
            let e = found.map_or_else(|| r.edge(Some(key)), |e| e as u32);
            (
                e,
                if a < b {
                    Sense::Forward
                } else {
                    Sense::Reversed
                },
            )
        })
        .collect()
}

/// No move at all: the planes through the grid points themselves.
fn still(_: usize, _: usize) -> [i64; 3] {
    [0; 3]
}

/// A step or so each way, different at every anchor of every face, from `seed`.
fn jog(seed: u64) -> impl Fn(usize, usize) -> [i64; 3] {
    move |face, anchor| {
        let mix = |k: u64| {
            let x = seed
                .wrapping_add((face as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15))
                .wrapping_add((anchor as u64 + 1).wrapping_mul(0xBF58_476D_1CE4_E5B9))
                .wrapping_add(k.wrapping_mul(0x94D0_49BB_1331_11EB));
            let x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            ((x ^ (x >> 27)) % 3) as i64 - 1
        };
        [mix(0), mix(1), mix(2)]
    }
}

/// The seed of the random corpus: fixed, printed, and replaced by `SPAREKIT_SEED` in hex.
fn seed() -> u64 {
    let seed = std::env::var("SPAREKIT_SEED").map_or(0x6134_6134, |s| {
        u64::from_str_radix(&s, 16).expect("SPAREKIT_SEED in hex")
    });
    println!("seed: {seed:x}");
    seed
}

/// The faults of `body`, at the full level.
fn full(body: &Body, exactness: Exactness) -> Report {
    check(body, exactness, Level::Full, &mut meter()).unwrap()
}

/// The faults of `rows`, a solid recorded Exact.
fn faults(rows: &Rows) -> Vec<Fault> {
    full(&rows.body(BodyKind::Solid), Exactness::Exact)
        .faults()
        .to_vec()
}

/// The kinds of `found`, sorted away from their order.
fn kinds(found: &[Fault]) -> Vec<FaultKind> {
    found.iter().map(|f| f.kind).collect()
}

#[test]
fn the_planes_through_the_grid_points_pass_as_they_are() {
    for spec in CORPUS {
        let found = faults(&tilted(spec, &still));
        assert!(found.is_empty(), "{found:?}");
    }
}

#[test]
fn every_tilted_polyhedron_passes() {
    let seed = seed();
    for k in 0..40 {
        for spec in CORPUS {
            let rows = tilted(spec, &jog(seed.wrapping_add(k)));
            let found = faults(&rows);
            assert!(found.is_empty(), "seed {seed:x} + {k}: {found:?}");
        }
    }
}

#[test]
fn a_tilted_body_has_vertices_no_grid_point_holds() {
    let rows = tilted(BOX, &jog(7));
    let body = rows.body(BodyKind::Solid);
    let implicit = (0..8).filter(|&v| {
        matches!(
            geom_topo::query::point(&body, ix(v)),
            Err(geom_topo::query::QueryError::Implicit(_))
        )
    });
    assert_eq!(implicit.count(), 8);
}

#[test]
fn a_cut_corner_mixes_grid_vertices_and_meetings() {
    // A cube of side 30 with its far corner cut by 3x + 4y + 7z = 370, which meets the three edges
    // there at z = 160/7, y = 35/2 and x = 40/3.
    let mut r = Rows::empty();
    let solid = r.region(RegionKind::Solid);
    let corner = [
        [0, 0, 0],
        [30, 0, 0],
        [0, 30, 0],
        [30, 30, 0],
        [0, 0, 30],
        [30, 0, 30],
        [0, 30, 30],
    ];
    for p in corner {
        r.vertex(p);
    }
    let high_x = [[30, 0, 0], [30, 30, 0], [30, 0, 30]];
    let high_y = [[0, 30, 0], [0, 30, 30], [30, 30, 0]];
    let high_z = [[0, 0, 30], [30, 0, 30], [0, 30, 30]];
    let cut = [[20, 25, 30], [24, 22, 30], [26, 24, 28]];
    let x = r.plane_through(high_x);
    let y = r.plane_through(high_y);
    let z = r.plane_through(high_z);
    let c = r.plane_through(cut);
    let a = r.meet([x, y, c]);
    let b = r.meet([x, z, c]);
    let d = r.meet([y, z, c]);
    let s = r.shell(solid);
    let low_z = [[0, 0, 0], [0, 30, 0], [30, 0, 0]];
    let low_y = [[0, 0, 0], [30, 0, 0], [0, 0, 30]];
    let low_x = [[0, 0, 0], [0, 0, 30], [0, 30, 0]];
    let faces: [(&[u32], [[i64; 3]; 3]); 7] = [
        (&[0, 2, 3, 1], low_z),
        (&[0, 1, 5, 4], low_y),
        (&[0, 4, 6, 2], low_x),
        (&[1, 3, a, b, 5], high_x),
        (&[2, 6, d, a, 3], high_y),
        (&[4, 5, b, d, 6], high_z),
        (&[a, d, b], cut),
    ];
    for (ring, on) in faces {
        let ring = fins(&mut r, ring);
        r.face_on(s, &[&ring], on);
    }
    assert_eq!(faults(&r), []);
}

#[test]
fn three_planes_that_do_not_meet_are_a_fault() {
    let mut r = tilted(BOX, &still);
    // Faces 0 and 1 are the box's two parallel faces.
    r.meets[0].1 = [0, 1, 2];
    let found = faults(&r);
    assert_eq!(
        found.first(),
        Some(&Fault::at(FaultKind::MeetSingular, Entity::Vertex(ix(0))))
    );
}

#[test]
fn a_meeting_off_a_face_plane_is_a_fault_in_every_class() {
    let mut r = tilted(BOX, &jog(3));
    // Vertex 0 is given vertex 7's planes, which are the far three.
    r.meets[0].1 = r.meets[7].1;
    let body = r.body(BodyKind::Solid);
    let certified = Exactness::Certified {
        bound: Steps::new(1_000).unwrap(),
        step: GridStep::Q0,
    };
    for exactness in [Exactness::Exact, certified] {
        let report = full(&body, exactness);
        assert!(
            kinds(report.faults()).contains(&FaultKind::VertexOffSurface),
            "{:?}",
            report.faults()
        );
    }
}

#[test]
fn a_shadow_line_a_long_way_from_its_vertex_is_a_fault() {
    let mut r = tilted(BOX, &jog(3));
    let p = r.points[0];
    let q = r.points[1];
    let at = |p: P3| [p.x.get(), p.y.get(), p.z.get()];
    let e = (0..r.edges.len())
        .find(|&e| r.edges[e].start.map(|v| v.row()) == Some(0))
        .unwrap();
    let curve = r.edges[e].curve.row() as usize;
    let mut far = at(p);
    far[2] += 5;
    r.curves[curve] = line(far, at(q));
    assert!(kinds(&faults(&r)).contains(&FaultKind::VertexOffCurve));
}

#[test]
fn a_shadow_line_a_step_off_passes() {
    let mut r = tilted(BOX, &still);
    let at = |p: P3| [p.x.get(), p.y.get(), p.z.get()];
    let e = (0..r.edges.len())
        .find(|&e| r.edges[e].start.map(|v| v.row()) == Some(0))
        .unwrap();
    let curve = r.edges[e].curve.row() as usize;
    let end = r.edges[e].end.unwrap().row() as usize;
    let (start, end) = (at(r.points[0]), at(r.points[end]));
    r.curves[curve] = line([start[0] + 1, start[1], start[2]], [end[0], end[1], end[2]]);
    assert_eq!(faults(&r), []);
}

#[test]
fn a_box_is_held_to_a_meetings_exact_place() {
    // The vertex at (40/3, 30, 30): a box to x = 13 misses it, a box to x = 14 holds it.
    let mut r = Rows::empty();
    let solid = r.region(RegionKind::Solid);
    let s = r.shell(solid);
    let y = r.plane_through([[0, 30, 0], [0, 30, 30], [30, 30, 0]]);
    let z = r.plane_through([[0, 0, 30], [30, 0, 30], [0, 30, 30]]);
    let c = r.plane_through([[20, 25, 30], [24, 22, 30], [26, 24, 28]]);
    let d = r.meet([y, z, c]);
    let others = [r.vertex([0, 30, 30]), r.vertex([30, 0, 30])];
    let ring = fins(&mut r, &[d, others[0], others[1]]);
    let f = r.face_on(s, &[&ring], [[0, 0, 30], [30, 0, 30], [0, 30, 30]]);
    let far = |x: i64| Box3::new(P3::new(0, 0, 29).unwrap(), P3::new(x, 31, 31).unwrap()).unwrap();
    let box_fault = |r: &Rows| {
        full(&r.body(BodyKind::Sheet), Exactness::Exact)
            .faults()
            .iter()
            .any(|x| x.kind == FaultKind::FaceBound)
    };
    r.faces[f as usize].bound =
        Box3::new(P3::new(0, 0, 29).unwrap(), P3::new(30, 31, 31).unwrap()).unwrap();
    assert!(!box_fault(&r));
    // The other corners reach x = 30, so the box is cut from below instead: 14 to 30 misses the
    // grid corner at 0 as well; the meeting alone is tested with a box from 13 to 14 in x.
    r.points[others[0] as usize] = P3::new(14, 30, 30).unwrap();
    r.points[others[1] as usize] = P3::new(14, 25, 30).unwrap();
    let curves: Vec<_> = (0..r.edges.len()).collect();
    assert_eq!(curves.len(), 3);
    let narrow = |low: i64, high: i64| {
        Box3::new(P3::new(low, 0, 29).unwrap(), P3::new(high, 31, 31).unwrap()).unwrap()
    };
    r.faces[f as usize].bound = narrow(13, 14);
    let held = full(&r.body(BodyKind::Sheet), Exactness::Exact);
    assert!(!held.faults().iter().any(|x| x.kind == FaultKind::FaceBound));
    r.faces[f as usize].bound = narrow(14, 15);
    assert!(box_fault(&r), "40/3 is below 14");
    let _ = far;
}

#[test]
fn two_edges_of_a_face_crossing_are_found() {
    // A bow tie: the corners of a face taken in the wrong order, all meetings of planes.
    let mut r = tilted(BOX, &jog(5));
    let s = r.shells.len() as u32 - 1;
    let ring = fins(&mut r, &[0, 3, 1, 2]);
    r.face_on(s, &[&ring], [[0, 0, 0], [0, 100, 0], [100, 0, 0]]);
    assert!(
        kinds(&faults(&r)).contains(&FaultKind::EdgesCross),
        "{:?}",
        faults(&r)
    );
}

#[test]
fn a_lump_through_another_is_found() {
    // Two tilted boxes, the second moved until it runs through the first.
    let points: Vec<[i64; 3]> = TWO_POINTS
        .iter()
        .enumerate()
        .map(|(k, p)| {
            if k < 8 {
                *p
            } else {
                [p[0] - 250, p[1] + 25, p[2] + 25]
            }
        })
        .collect();
    let spec = Spec {
        points: &points,
        regions: TWO.regions,
    };
    let found = faults(&tilted(spec, &jog(11)));
    assert!(
        kinds(&found).contains(&FaultKind::FacesCross),
        "{found:?}"
    );
}

#[test]
fn a_tooth_through_a_window_and_a_tooth_into_the_wall_differ() {
    // A box through the wall's window, not touching it, passes as far as faces go.
    let mut points = WALL_POINTS.to_vec();
    points.extend(corners([150, -50, 125], [100, 150, 50]));
    let tooth: [&[&[u32]]; 6] = box_faces!(16);
    let shell = [&WALL_SHELL[..], &tooth[..]];
    let _ = shell;
    let regions: [&[&[&[&[u32]]]]; 2] = [&[&WALL_SHELL], &[&tooth]];
    let spec = Spec {
        points: &points,
        regions: &regions,
    };
    let through = faults(&tilted(spec, &jog(13)));
    assert!(
        !kinds(&through).contains(&FaultKind::FacesCross),
        "{through:?}"
    );
    // Moved up until it runs through the window's head, it crosses.
    let mut points = WALL_POINTS.to_vec();
    points.extend(corners([150, -50, 175], [100, 150, 50]));
    let spec = Spec {
        points: &points,
        regions: &regions,
    };
    let into = faults(&tilted(spec, &jog(13)));
    assert!(kinds(&into).contains(&FaultKind::FacesCross), "{into:?}");
}

#[test]
fn a_shell_turned_inside_out_is_found_by_its_certified_volume() {
    let spec_faces: [&[&[u32]]; 6] = cavity_faces!(0);
    let spec = Spec {
        points: &BOX_POINTS,
        regions: &[&[&spec_faces]],
    };
    let found = faults(&tilted(spec, &jog(17)));
    assert!(
        kinds(&found).contains(&FaultKind::ShellInward),
        "{found:?}"
    );
}

#[test]
fn a_cavity_outside_its_shell_is_found_by_the_exact_ray() {
    let points: Vec<[i64; 3]> = NEST_POINTS[..16]
        .iter()
        .enumerate()
        .map(|(k, p)| if k < 8 { *p } else { [p[0] + 2000, p[1], p[2]] })
        .collect();
    let spec = Spec {
        points: &points,
        regions: &[&[&BOX_SHELL, &CAVITY_SHELL]],
    };
    let found = faults(&tilted(spec, &jog(19)));
    assert_eq!(kinds(&found), [FaultKind::ShellNesting], "{found:?}");
}

#[test]
fn a_lump_inside_another_is_found_by_the_exact_ray() {
    let points: Vec<[i64; 3]> = TWO_POINTS
        .iter()
        .enumerate()
        .map(|(k, p)| {
            if k < 8 {
                [p[0] * 9, p[1] * 9, p[2] * 9]
            } else {
                [p[0] - 200, p[1] + 300, p[2] + 300]
            }
        })
        .collect();
    let spec = Spec {
        points: &points,
        regions: TWO.regions,
    };
    let found = faults(&tilted(spec, &jog(23)));
    assert!(
        kinds(&found).contains(&FaultKind::ShellNesting),
        "{found:?}"
    );
}

#[test]
fn a_loop_run_backwards_is_found_by_its_certified_area() {
    let mut r = tilted(BOX, &jog(29));
    let turned: [usize; 1] = [0];
    for f in turned {
        r.faces[f].sense = Sense::Reversed;
    }
    assert!(kinds(&faults(&r)).contains(&FaultKind::LoopSense));
}

#[test]
fn a_small_tilted_box_far_from_the_origin_keeps_its_sign() {
    let far = 1_099_511_627_000_i64;
    let points = corners([far, far, far], [100, 100, 100]);
    let spec = Spec {
        points: &points,
        regions: &[&[&BOX_SHELL]],
    };
    for k in 0..5 {
        let found = faults(&tilted(spec, &jog(31 + k)));
        assert!(found.is_empty(), "{found:?}");
    }
}

/// Six times `seam`'s two coplanar faces on z = 0: a box's top split along x = 50, and its other
/// faces, with the seam's edge shared.
fn seamed(second_up: bool) -> Rows {
    let mut r = Rows::empty();
    let solid = r.region(RegionKind::Solid);
    let s = r.shell(solid);
    let p = |x: i64, y: i64, z: i64| [x, y, z];
    for q in [
        p(0, 0, 0),
        p(100, 0, 0),
        p(100, 100, 0),
        p(0, 100, 0),
        p(0, 0, 100),
        p(50, 0, 100),
        p(100, 0, 100),
        p(100, 100, 100),
        p(50, 100, 100),
        p(0, 100, 100),
    ] {
        r.vertex(q);
    }
    let faces: [&[u32]; 7] = [
        &[0, 3, 2, 1],
        &[4, 5, 8, 9],
        if second_up {
            &[5, 6, 7, 8]
        } else {
            &[5, 8, 7, 6]
        },
        &[0, 1, 6, 5, 4],
        &[2, 3, 9, 8, 7],
        &[0, 4, 9, 3],
        &[1, 2, 7, 6],
    ];
    for ring in faces {
        let ring = fins(&mut r, ring);
        r.face(s, &[&ring]);
    }
    r
}

#[test]
fn a_seam_between_faces_on_one_plane_passes() {
    assert_eq!(faults(&seamed(true)), []);
}

#[test]
fn a_face_folded_back_on_its_neighbour_is_found() {
    // The second half of the top faces down: two faces of one plane, facing opposite ways, on one
    // side of... its fins run the same way as the seam's other, so group 1 speaks first.
    assert!(!faults(&seamed(false)).is_empty());
}

#[test]
fn coplanar_faces_apart_pass_and_overlapping_ones_are_found() {
    // Two lumps whose tops lie on one plane: apart, then overlapping, then one inside the other's
    // box in plan, each on a body whose faces are not all convex.
    let lump = |x: i64, side: i64| corners([x, 0, 0], [side, side, 100]);
    let apart = [lump(0, 100), lump(200, 100)].concat();
    let spec = Spec {
        points: &apart,
        regions: TWO.regions,
    };
    assert_eq!(faults(&tilted(spec, &still)), []);
    let overlapping = [lump(0, 100), lump(50, 100)].concat();
    let spec = Spec {
        points: &overlapping,
        regions: TWO.regions,
    };
    let found = faults(&tilted(spec, &still));
    assert!(kinds(&found).contains(&FaultKind::FacesCross), "{found:?}");
    let within = [lump(0, 300), lump(100, 100)].concat();
    let spec = Spec {
        points: &within,
        regions: TWO.regions,
    };
    let found = faults(&tilted(spec, &still));
    assert!(kinds(&found).contains(&FaultKind::FacesCross), "{found:?}");
}

#[test]
fn a_lump_touching_another_at_a_corner_is_found() {
    // The second box's corner rests on the middle of the first's top: no shared vertex.
    let points: Vec<[i64; 3]> = TWO_POINTS
        .iter()
        .enumerate()
        .map(|(k, p)| {
            if k < 8 {
                *p
            } else {
                [p[0] - 250, p[1] + 50, p[2] + 100]
            }
        })
        .collect();
    let spec = Spec {
        points: &points,
        regions: TWO.regions,
    };
    let found = faults(&tilted(spec, &still));
    assert!(kinds(&found).contains(&FaultKind::FacesCross), "{found:?}");
}

#[test]
fn every_face_pair_is_charged_and_a_stop_is_a_stop() {
    let body = tilted(WALL, &jog(37)).body(BodyKind::Solid);
    let mut last = None;
    for work in [1, 10, 100, 400, 1_000, 4_000, 10_000] {
        let budget = Budget {
            max_work: work,
            ..Budget::UNLIMITED
        };
        let mut meter = WorkMeter::new(budget);
        last = Some(check(&body, Exactness::Exact, Level::Full, &mut meter).is_err());
    }
    assert_eq!(last, Some(false), "the largest budget finishes");
}

#[test]
fn the_same_body_gives_the_same_report() {
    let body = tilted(WALL, &jog(41)).body(BodyKind::Solid);
    assert_eq!(
        full(&body, Exactness::Exact),
        full(&body, Exactness::Exact)
    );
}

#[test]
fn a_curve_kind_on_a_plane_based_face_has_no_outline() {
    let mut r = tilted(BOX, &jog(43));
    let e = r.edges[0].curve.row() as usize;
    r.curves[e] = CurveKind::Circle(
        geom_curve::Circle::new([0.0; 3], 1.0, geom_curve::Frame::standard().unwrap(), (0.0, 1.0))
            .unwrap(),
    );
    assert!(!faults(&r).is_empty());
}
