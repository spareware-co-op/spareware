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
//! The law covers group 3's orientation too. Which way a bent shell faces is the sign of a
//! volume, and where it lies against another is a ray's crossings with it; read from each loop's
//! first corner, both moved with the start on a body thin or close enough. They are read from the
//! least and most each face's corners can give and through the stars (`geom_check`'s
//! `orientation` module), so a shell inside out, one too thin to say, a cavity and a lump inside
//! another are each found the same in every copy.
//!
//! The bodies are few and small, so that the whole law runs in seconds: prisms over three corners
//! of a parabola, every corner then moved a step or two, as an inexact transform's rounding leaves
//! one, some turned inside out; and boxes with a small box inside or beside them, as a cavity or
//! as a second lump, their corners moved a step. Rings of five corners are restarted and turned
//! round in `attack_g134`.

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

use common::{CUBE_FACES, Rows, faces_into, report};
use geom_check::{Entity, FaultKind};
use geom_num::{Exactness, GridStep, Steps};
use geom_surface::{Plane, SurfaceKind};
use geom_topo::{BodyKind, RegionKind};

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

/// A polyhedron: its corners, its faces as rings of corner numbers, counter-clockwise seen from
/// outside the material, and its shells: for each, the solid region it bounds (the first is 1)
/// and how many of the rings are its faces, in order.
#[derive(Clone)]
struct Solid {
    points: Vec<[i64; 3]>,
    rings: Vec<Vec<u32>>,
    shells: Vec<(u32, usize)>,
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
    Solid {
        points,
        rings,
        shells: vec![(1, 5)],
    }
}

/// A box of side 16 and a box of side 3, every corner of both moved up to a step each way: the
/// small one inside the big one or beside it, as a cavity of its region or as a lump of its own.
/// A cavity inside and a lump beside are where they may be; the other two are not.
fn two_boxes(rng: &mut Mix, inside: bool, cavity: bool) -> Solid {
    let mut jog = |p: [i64; 3]| p.map(|c| c + rng.below(3) as i64 - 1);
    let big = box_corners([0, 0, 0], [16; 3]).map(&mut jog);
    let from = if inside { [5, 7, 6] } else { [22, 7, 6] };
    let small = box_corners(from, [3; 3]).map(&mut jog);
    boxes(big, Some((small, cavity)))
}

/// The corners of the box from `from` of size `size`, numbered by bits: bit 0 is x, bit 1 y and
/// bit 2 z, as [`CUBE_FACES`] lists them.
fn box_corners(from: [i64; 3], size: [i64; 3]) -> [[i64; 3]; 8] {
    [0, 1, 2, 3, 4, 5, 6, 7].map(|k: i64| [0, 1, 2].map(|i| from[i] + size[i] * ((k >> i) & 1)))
}

/// A box with corners `big`, and with it, if given, a second with corners `small`: a cavity of the
/// first's region if the flag says so, a lump of its own otherwise.
fn boxes(big: [[i64; 3]; 8], second: Option<([[i64; 3]; 8], bool)>) -> Solid {
    let mut points = big.to_vec();
    let mut rings: Vec<Vec<u32>> = CUBE_FACES.iter().map(|f| f.to_vec()).collect();
    let mut shells = vec![(1, 6)];
    if let Some((small, cavity)) = second {
        points.extend(small);
        for f in CUBE_FACES {
            let mut ring: Vec<u32> = f.iter().map(|v| v + 8).collect();
            if cavity {
                ring.reverse();
            }
            rings.push(ring);
        }
        shells.push((if cavity { 1 } else { 2 }, 6));
    }
    Solid {
        points,
        rings,
        shells,
    }
}

/// A slab over a square of side 20, `thick` steps thick, its floor and roof one saddle: the
/// corners at (20, 0) and (0, 20) are `rise` steps higher than the other two. Six times its
/// volume lies between 2400 `thick` - 800 `rise` and 2400 `thick` + 800 `rise`, by the least and
/// the most its faces' corners give.
fn saddle_slab(thick: i64, rise: i64) -> Solid {
    let mut points = box_corners([0, 0, 0], [20, 20, thick]);
    for k in [1, 2, 5, 6] {
        points[k][2] += rise;
    }
    boxes(points, None)
}

/// A box 20 wide and 40 high whose floor is bent, its corner at (20, 20) lifted 8 steps, and a
/// box of side 2 standing on z = 3 over the floor's middle, as a cavity or as a lump. The floor's
/// star reaches 2.6 under the small box, which is clear of it and inside; a fan from the floor's
/// corner at the origin, or from the lifted one, folds 3.6 high under the small box's lowest
/// corners and leaves them outside.
fn box_over_a_bent_floor(cavity: bool) -> Solid {
    let mut big = box_corners([0, 0, 0], [20, 20, 40]);
    big[3][2] = 8;
    boxes(big, Some((box_corners([9, 9, 3], [2; 3]), cavity)))
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
    let mut r = Rows::empty();
    for &p in &points {
        r.vertex(p);
    }
    let regions = solid.shells.iter().map(|s| s.0).max().unwrap();
    for _ in 0..regions {
        r.region(RegionKind::Solid);
    }
    let mut at = 0;
    for &(region, faces) in &solid.shells {
        let s = r.shell(region);
        faces_into(&mut r, s, &listed[at..at + faces]);
        at += faces;
    }
    for (f, ring) in solid.rings.iter().enumerate() {
        let three = three_of(&solid.points, ring).unwrap();
        r.surfaces[f] = carrier(&points, &rings[f], three.map(|v| order[v as usize]));
    }
    r
}

/// What the checker found of a body: each fault's kind and the faces, shells and regions it names.
type Verdict = Vec<(FaultKind, Option<(char, u32)>, Option<(char, u32)>)>;

/// What the checker finds of `rows`: each fault's kind and the faces, shells and regions it names
/// (their rows are the same in every copy; an edge's or a vertex's follows the numbering, and is
/// left out).
fn verdict(rows: &Rows, bound: u64) -> Verdict {
    let record = Exactness::Certified {
        bound: Steps(bound),
        step: GridStep::Q0,
    };
    let named = |e: Entity| match e {
        Entity::Face(f) => Some(('f', f.row())),
        Entity::Shell(s) => Some(('s', s.row())),
        Entity::Region(r) => Some(('r', r.row())),
        _ => None,
    };
    let mut found: Verdict = report(rows, BodyKind::Solid, record)
        .faults()
        .iter()
        .map(|f| (f.kind, named(f.at), f.with.and_then(named)))
        .collect();
    found.sort_by_key(|f| format!("{f:?}"));
    found
}

/// Whether `kind` is one of group 3's findings of a body's faces and shells, which the law is
/// about. Group 1's and 2's faults are not: a corner moved onto another, a corner off its plane.
fn ours(kind: FaultKind) -> bool {
    matches!(
        kind,
        FaultKind::FacesCross
            | FaultKind::PairUnchecked
            | FaultKind::Unchecked
            | FaultKind::ShellInward
            | FaultKind::ShellOutward
            | FaultKind::ShellNesting
    )
}

/// The verdict on `solid` as it is listed, within `bound`, if the law is about it; and then every
/// copy of it through [`MAPS`], each ring started anywhere and every vertex renumbered, as a
/// pattern's copy renames every entity, must get the same.
fn one_verdict(solid: &Solid, bound: u64, rng: &mut Mix, case: &str) -> Option<Verdict> {
    // A ring its moved corners left on one line has no plane to give its face.
    if solid
        .rings
        .iter()
        .any(|ring| three_of(&solid.points, ring).is_none())
    {
        return None;
    }
    let same: Vec<u32> = (0..solid.points.len() as u32).collect();
    let first = verdict(&rows(solid, &Map::SAME, &mut || 0, &same), bound);
    if !first.iter().all(|f| ours(f.0)) {
        return None;
    }
    for (what, map) in MAPS {
        let mut order = same.clone();
        for k in (1..order.len()).rev() {
            order.swap(k, rng.below(k as u64 + 1) as usize);
        }
        let copy = rows(solid, &map, &mut || rng.below(8) as usize, &order);
        assert_eq!(
            verdict(&copy, bound * map.scale as u64),
            first,
            "{case}, {what}: {:?} {:?}",
            solid.points,
            solid.rings
        );
    }
    Some(first)
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

#[test]
fn the_maps_are_what_their_names_say() {
    let mirrors: Vec<bool> = MAPS.iter().map(|(_, map)| map.mirrors()).collect();
    assert_eq!(mirrors, [false, true, false, false, false, true]);
    assert_eq!(MAPS[3].1.apply([1, 2, 3]), [-2, 1, 3]);
    assert_eq!(MAPS[5].1.apply([1, 2, 3]), [9, -1, 0]);
}

/// How many prisms are drawn: enough for the seed to reach bodies that pass, bodies that cross
/// and bodies inside out, few enough to run in seconds.
const CASES: u32 = 16;

#[test]
fn a_bent_body_and_every_exact_copy_of_it_get_one_verdict() {
    let mut rng = Mix(0x6134_6134);
    let (mut passed, mut crossed, mut inward, mut other) = (0_u32, 0_u32, 0_u32, 0_u32);
    for case in 0..CASES {
        let most = 1 + rng.below(2);
        let mut solid = bent_prism(&mut rng, most);
        // One in four inside out: every ring run the other way.
        if rng.below(4) == 0 {
            solid.rings.iter_mut().for_each(|ring| ring.reverse());
        }
        let Some(first) = one_verdict(&solid, 4 * most, &mut rng, &format!("case {case}")) else {
            continue;
        };
        let has = |kind: FaultKind| first.iter().any(|f| f.0 == kind);
        if first.is_empty() {
            passed += 1;
        } else if has(FaultKind::FacesCross) {
            crossed += 1;
        } else if has(FaultKind::ShellInward) {
            inward += 1;
        } else {
            other += 1;
        }
    }
    println!("{passed} passed, {crossed} crossed, {inward} inside out, {other} other");
    // The law is held on every side: bodies the checker passes, bodies it finds crossing, and
    // bodies it finds facing inward.
    assert!(
        passed >= 3 && crossed >= 3 && inward >= 1,
        "{passed} {crossed} {inward} {other}"
    );
}

#[test]
fn a_bent_shell_beside_another_and_every_exact_copy_get_one_verdict() {
    let mut rng = Mix(0x6134_0003);
    let mut tally = [[0_u32; 2]; 4];
    for case in 0..8_usize {
        let (inside, cavity) = (case & 1 == 0, case & 2 == 0);
        let solid = two_boxes(&mut rng, inside, cavity);
        let Some(first) = one_verdict(&solid, 4, &mut rng, &format!("case {case}")) else {
            continue;
        };
        let nested = first.iter().any(|f| f.0 == FaultKind::ShellNesting);
        assert!(first.is_empty() || nested, "case {case}: {first:?}");
        tally[case & 3][usize::from(nested)] += 1;
    }
    println!("{tally:?}");
    // A cavity inside its shell and a lump beside another pass; a lump inside another's material
    // and a cavity outside its shell are found. Each of the four is reached, and no body of one
    // kind got the other kind's verdict.
    let [cavity_in, cavity_out, lump_in, lump_out] = tally;
    assert!(cavity_in[0] >= 1 && cavity_in[1] == 0, "{tally:?}");
    assert!(cavity_out[1] >= 1 && cavity_out[0] == 0, "{tally:?}");
    assert!(lump_in[1] >= 1 && lump_in[0] == 0, "{tally:?}");
    assert!(lump_out[0] >= 1 && lump_out[1] == 0, "{tally:?}");
}

#[test]
fn shells_thin_or_close_enough_to_turn_on_a_rings_start_get_one_verdict() {
    // Bodies made so that reading a loop from its first corner gives one answer from one start
    // and another from the next: slabs thinner than their saddle is high, and a small box nearer
    // a bent floor than the floor's fan is to its star. Read from where the corners lie, each has
    // one verdict, in every copy: this test failed on its first body while group 3's orientation
    // read first corners.
    let mut rng = Mix(0x6134_0004);
    let shell = |row| Some(('s', row));
    let inside_out = |solid: &Solid| {
        let mut turned = solid.clone();
        turned.rings.iter_mut().for_each(|ring| ring.reverse());
        turned
    };
    let cases: [(&str, Solid, Verdict); 7] = [
        // From -800 to 5600: no sign, whichever way it faces.
        (
            "a slab one thick under a saddle four high",
            saddle_slab(1, 4),
            vec![(FaultKind::Unchecked, shell(0), None)],
        ),
        (
            "the same slab inside out",
            inside_out(&saddle_slab(1, 4)),
            vec![(FaultKind::Unchecked, shell(0), None)],
        ),
        // From 0 to 4800: the least is nothing, so no sign.
        (
            "a slab one thick under a saddle three high",
            saddle_slab(1, 3),
            vec![(FaultKind::Unchecked, shell(0), None)],
        ),
        // From 2400 to 7200, and from 11200 to 12800.
        (
            "a slab two thick under a saddle three high",
            saddle_slab(2, 3),
            vec![],
        ),
        (
            "a thick slab inside out",
            inside_out(&saddle_slab(5, 1)),
            vec![(FaultKind::ShellInward, Some(('r', 1)), shell(0))],
        ),
        (
            "a cavity over a bent floor",
            box_over_a_bent_floor(true),
            vec![],
        ),
        (
            "a lump over a bent floor",
            box_over_a_bent_floor(false),
            vec![(FaultKind::ShellNesting, shell(1), shell(0))],
        ),
    ];
    for (what, solid, want) in cases {
        assert_eq!(one_verdict(&solid, 8, &mut rng, what), Some(want), "{what}");
    }
}
