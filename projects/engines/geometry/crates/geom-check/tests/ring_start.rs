//! The law that validity is a property of the geometry, on faces their rounded corners have bent
//! (G134): the checker finds the same of a body whichever corner each ring starts at, however its
//! vertices are numbered, and of its image under every exact transform, mirrors included.
//!
//! A ring's start is set by names (`geom_topo::Builder::publish_named` makes each loop's first fin
//! its least), so a mirror, which turns every fin round, and a pattern's copy, which renames every
//! entity, start their rings anywhere. A checker that read a bent face as the fan of triangles
//! from its first corner found a body valid and its mirror crossing itself; this test failed on
//! its first body, with its rings restarted and nothing else changed. The star from the mean of a
//! ring's corners has no first corner to depend on (`geom_check`'s `planar` module).
//!
//! The bodies are few and small, so that the whole law runs in seconds: prisms over three corners
//! of a parabola, every corner then moved a step or two, as an inexact transform's rounding leaves
//! one. Rings of five corners are restarted and turned round in `attack_g134`.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::print_stdout,
    reason = "test fixtures; the rules are for kernel code"
)]

mod common;

use common::{Rows, polyhedron, report};
use geom_check::{Entity, FaultKind};
use geom_num::{Exactness, GridStep, Steps};
use geom_surface::{Plane, SurfaceKind};
use geom_topo::BodyKind;

/// `SplitMix64` (Steele, Lea and Flood 2014, doi:10.1145/2660193.2660195), as the other fixtures
/// here mix their seeds.
struct Mix(u64);

impl Mix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let x = (self.0 ^ (self.0 >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        let x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        x ^ (x >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// A polyhedron: its corners, and its faces as rings of corner numbers, counter-clockwise seen
/// from outside.
#[derive(Clone)]
struct Solid {
    points: Vec<[i64; 3]>,
    rings: Vec<Vec<u32>>,
}

/// A prism over three corners on a parabola, each corner then moved up to `most` steps each way,
/// as an inexact transform's rounding leaves one: its three sides are bent.
fn bent_prism(rng: &mut Mix, most: u64) -> Solid {
    let n = 3_i64;
    let (dx, k, height) = (
        1 + rng.below(4) as i64,
        1 + rng.below(3) as i64,
        1 + rng.below(8) as i64,
    );
    let mut jog = || rng.below(2 * most + 1) as i64 - most as i64;
    let mut points = Vec::new();
    for z in [0, height] {
        for i in 0..n {
            points.push([i * dx + jog(), k * i * i + jog(), z + jog()]);
        }
    }
    let n = n as u32;
    let mut rings: Vec<Vec<u32>> = vec![(0..n).rev().collect(), (n..2 * n).collect()];
    for i in 0..n {
        let j = (i + 1) % n;
        rings.push(vec![i, j, n + j, n + i]);
    }
    Solid { points, rings }
}

/// An exact transform: axes permuted and turned, a whole scale, a move.
#[derive(Clone, Copy)]
struct Map {
    axes: [usize; 3],
    signs: [i64; 3],
    scale: i64,
    by: [i64; 3],
}

impl Map {
    const SAME: Map = Map {
        axes: [0, 1, 2],
        signs: [1, 1, 1],
        scale: 1,
        by: [0, 0, 0],
    };

    fn apply(&self, p: [i64; 3]) -> [i64; 3] {
        [0, 1, 2].map(|i| self.signs[i] * self.scale * p[self.axes[i]] + self.by[i])
    }

    /// Whether it turns a body inside out, so that every ring runs the other way.
    fn mirrors(&self) -> bool {
        let odd = matches!(self.axes, [0, 2, 1] | [1, 0, 2] | [2, 1, 0]);
        let [x, y, z] = self.signs;
        (x * y * z < 0) != odd
    }
}

/// `p` × `q`.
fn cross(p: [i128; 3], q: [i128; 3]) -> [i128; 3] {
    [
        p[1] * q[2] - p[2] * q[1],
        p[2] * q[0] - p[0] * q[2],
        p[0] * q[1] - p[1] * q[0],
    ]
}

/// `p` − `q`.
fn minus(p: [i128; 3], q: [i128; 3]) -> [i128; 3] {
    [0, 1, 2].map(|i| p[i] - q[i])
}

/// The plane through three of `ring`'s corners, turned the way the ring runs.
fn carrier(points: &[[i64; 3]], ring: &[u32], three: [u32; 3]) -> SurfaceKind {
    let at = |v: u32| points[v as usize].map(i128::from);
    let mut area = [0_i128; 3];
    for (k, &v) in ring.iter().enumerate() {
        let c = cross(at(v), at(ring[(k + 1) % ring.len()]));
        area = [0, 1, 2].map(|i| area[i] + c[i]);
    }
    let [o, a, b] = three.map(at);
    let (u, v) = (minus(a, o), minus(b, o));
    let n = cross(u, v);
    let turn = n[0] * area[0] + n[1] * area[1] + n[2] * area[2];
    let (u, v) = if turn < 0 { (v, u) } else { (u, v) };
    let f = |w: [i128; 3]| w.map(|x| x as f64);
    SurfaceKind::Plane(Plane::new(f(o), f(u), f(v), ((0.0, 1.0), (0.0, 1.0))).unwrap())
}

/// The first three corners of `ring` that are not on one line.
fn three_of(points: &[[i64; 3]], ring: &[u32]) -> Option<[u32; 3]> {
    let at = |v: u32| points[v as usize].map(i128::from);
    let (o, a) = (at(ring[0]), at(ring[1]));
    ring.iter()
        .skip(2)
        .find(|&&v| cross(minus(a, o), minus(at(v), o)) != [0; 3])
        .map(|&v| [ring[0], ring[1], v])
}

/// `solid` under `map`, each ring started `shifts()` corners on, and its vertices numbered through
/// `order`: the rows, each face on the image of the plane the solid's own listing gives it, so
/// that only the starts, the numbers and the map differ between two copies.
fn rows(solid: &Solid, map: &Map, shifts: &mut dyn FnMut() -> usize, order: &[u32]) -> Rows {
    let mut points = vec![[0; 3]; solid.points.len()];
    for (old, p) in solid.points.iter().enumerate() {
        points[order[old] as usize] = map.apply(*p);
    }
    let rings: Vec<Vec<u32>> = solid
        .rings
        .iter()
        .map(|ring| {
            let mut ring: Vec<u32> = ring.iter().map(|&v| order[v as usize]).collect();
            if map.mirrors() {
                ring.reverse();
            }
            let by = shifts() % ring.len();
            ring.rotate_left(by);
            ring
        })
        .collect();
    let listed: Vec<&[u32]> = rings.iter().map(Vec::as_slice).collect();
    let mut r = polyhedron(&points, &listed);
    for (f, ring) in solid.rings.iter().enumerate() {
        let three = three_of(&solid.points, ring).unwrap();
        r.surfaces[f] = carrier(&points, &rings[f], three.map(|v| order[v as usize]));
    }
    r
}

/// What the checker finds of `rows`: each fault's kind and the faces it names (a face's row is
/// the same in every copy; an edge's or a vertex's follows the numbering, and is left out).
fn verdict(rows: &Rows, bound: u64) -> Vec<(FaultKind, Option<u32>, Option<u32>)> {
    let record = Exactness::Certified {
        bound: Steps(bound),
        step: GridStep::Q0,
    };
    let face = |e: Entity| match e {
        Entity::Face(f) => Some(f.row()),
        _ => None,
    };
    let mut found: Vec<_> = report(rows, BodyKind::Solid, record)
        .faults()
        .iter()
        .map(|f| (f.kind, face(f.at), f.with.and_then(face)))
        .collect();
    found.sort_by_key(|f| format!("{f:?}"));
    found
}

/// The exact transforms each body is copied through, by name.
const MAPS: [(&str, Map); 6] = [
    ("the rings restarted", Map::SAME),
    (
        "a mirror",
        Map {
            signs: [-1, 1, 1],
            ..Map::SAME
        },
    ),
    (
        "a move",
        Map {
            by: [5, -7, 11],
            ..Map::SAME
        },
    ),
    (
        "a quarter turn",
        Map {
            axes: [1, 0, 2],
            signs: [-1, 1, 1],
            ..Map::SAME
        },
    ),
    (
        "a scale",
        Map {
            scale: 3,
            ..Map::SAME
        },
    ),
    (
        "a mirror, moved and turned",
        Map {
            axes: [2, 0, 1],
            signs: [1, -1, 1],
            scale: 2,
            by: [3, 1, -4],
        },
    ),
];

/// How many bodies are drawn: enough for the seed to reach bodies that pass and bodies that
/// cross, few enough to run in seconds.
const CASES: u32 = 12;

#[test]
fn the_maps_are_what_their_names_say() {
    let mirrors: Vec<bool> = MAPS.iter().map(|(_, map)| map.mirrors()).collect();
    assert_eq!(mirrors, [false, true, false, false, false, true]);
    assert_eq!(MAPS[3].1.apply([1, 2, 3]), [-2, 1, 3]);
    assert_eq!(MAPS[5].1.apply([1, 2, 3]), [9, -1, 0]);
}

#[test]
fn a_bent_body_and_every_exact_copy_of_it_get_one_verdict() {
    let mut rng = Mix(0x6134_6134);
    let (mut passed, mut crossed, mut waited) = (0_u32, 0_u32, 0_u32);
    for case in 0..CASES {
        let most = 1 + rng.below(2);
        let solid = bent_prism(&mut rng, most);
        // A ring its moved corners left on one line has no plane to give its face.
        let flat = |ring: &Vec<u32>| three_of(&solid.points, ring).is_none();
        if solid.rings.iter().any(flat) {
            continue;
        }
        let same: Vec<u32> = (0..solid.points.len() as u32).collect();
        let bound = 4 * most;
        let first = verdict(&rows(&solid, &Map::SAME, &mut || 0, &same), bound);
        // Group 1's and 2's faults are not this law's: a corner moved onto another, a loop turned.
        let ours = |k: FaultKind| matches!(k, FaultKind::FacesCross | FaultKind::PairUnchecked);
        if !first.iter().all(|f| ours(f.0)) {
            continue;
        }
        if first.is_empty() {
            passed += 1;
        } else if first.iter().any(|f| f.0 == FaultKind::FacesCross) {
            crossed += 1;
        } else {
            waited += 1;
        }
        for (what, map) in MAPS {
            // A pattern's copy: every entity renamed, so every ring starts anywhere.
            let mut order = same.clone();
            for k in (1..order.len()).rev() {
                order.swap(k, rng.below(k as u64 + 1) as usize);
            }
            let copy = rows(&solid, &map, &mut || rng.below(8) as usize, &order);
            assert_eq!(
                verdict(&copy, bound * map.scale as u64),
                first,
                "case {case}, {what}: {:?} {:?}",
                solid.points,
                solid.rings
            );
        }
    }
    println!("{passed} passed, {crossed} crossed, {waited} waited");
    // The law is held on both sides: bodies the checker passes, and bodies it finds crossing.
    assert!(passed >= 3 && crossed >= 3, "{passed} {crossed} {waited}");
}
