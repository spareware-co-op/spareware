//! Attacks on what piece G299's reconstruction claims (19.7), on random soups from a printed seed
//! (`SPAREKIT_SEED` replaces it): overlapping and touching boxes, in any face order and any turn,
//! give exactly the volume of their union; a hole cut from a flat face of a box is closed in its
//! plane; the result is closed, solid, Approximate by a deviation of at least the rounding's one
//! step, and is the same mesh whatever order the faces and corners came in; and no cell is both
//! kept and left out as undecided.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::assert_is_empty,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    reason = "test generators and checks; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeSet;
use std::num::NonZeroU64;

use geom_mesh::exact::build::Source;
use geom_mesh::exact::mass::Volume;
use geom_mesh::exact::raw::RawMesh;
use geom_mesh::exact::{ExactMesh, MeshKind, Role};
use geom_mesh::repair::{Reconstruct, Reconstructed, reconstruct};
use geom_num::Exactness;
use geom_num::grid::P3;
use geom_num::wide::I256;
use geom_op::NameId;
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

/// One cell of the lattice the boxes sit on, in steps.
const CELL: i64 = 40;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 30))
}

fn run(mesh: &RawMesh) -> Reconstructed {
    reconstruct(
        mesh,
        Reconstruct::default(),
        Source {
            name: NameId::from_bytes([6; 16]),
            role: Role::FaceSet,
            flags: 0,
        },
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap()
}

fn six_volume(m: &ExactMesh) -> I256 {
    match m.volume(&mut meter()).unwrap() {
        Volume::Exact(six) => six,
        Volume::Certified(_) => panic!("every corner here is on the grid"),
    }
}

fn below(rng: &mut SplitMix64, n: u64) -> i64 {
    rng.below(NonZeroU64::new(n).unwrap()) as i64
}

const SIDES: [[usize; 4]; 6] = [
    [0, 4, 6, 2],
    [1, 3, 7, 5],
    [0, 1, 5, 4],
    [2, 6, 7, 3],
    [0, 2, 3, 1],
    [4, 5, 7, 6],
];

type Box3 = ([i64; 3], [i64; 3]);

fn box_corners(low: [i64; 3], high: [i64; 3]) -> [[i64; 3]; 8] {
    std::array::from_fn(|i| [0, 1, 2].map(|a| if i >> a & 1 == 1 { high[a] } else { low[a] }))
}

/// Boxes on lattice cells, 1 to 3 cells a side, in a 4 by 4 by 4 block of cells: they overlap,
/// touch along faces and edges, and share planes.
fn boxes(rng: &mut SplitMix64, count: usize) -> Vec<Box3> {
    (0..count)
        .map(|_| {
            let low = [0; 3].map(|_| below(rng, 3));
            let size = [0; 3].map(|_| 1 + below(rng, 3));
            (
                low.map(|v| v * CELL),
                [0, 1, 2].map(|a| (low[a] + size[a]) * CELL),
            )
        })
        .collect()
}

/// The boxes as a soup, each box its own corners, its faces turned the way `flip` says.
fn soup(boxes: &[Box3], flip: &[bool]) -> (Vec<[i64; 3]>, Vec<Vec<u32>>) {
    let (mut points, mut faces) = (Vec::new(), Vec::new());
    for (&(low, high), &turn) in boxes.iter().zip(flip) {
        let base = points.len() as u32;
        points.extend(box_corners(low, high));
        for side in SIDES {
            let mut face: Vec<u32> = side.iter().map(|&v| base + v as u32).collect();
            if turn {
                face.reverse();
            }
            faces.push(face);
        }
    }
    (points, faces)
}

fn raw(points: &[[i64; 3]], faces: &[Vec<u32>]) -> RawMesh {
    let vertices = points
        .iter()
        .map(|&[x, y, z]| P3::new(x, y, z).unwrap())
        .collect();
    let mut corners = Vec::new();
    let mut start = vec![0];
    for f in faces {
        corners.extend_from_slice(f);
        start.push(corners.len() as u32);
    }
    RawMesh::new(vertices, corners, start).unwrap()
}

/// The volume of the union of `boxes`, counted over the lattice's unit cells.
fn union_volume(boxes: &[Box3]) -> i128 {
    let mut cells = BTreeSet::new();
    for &(low, high) in boxes {
        for x in low[0] / CELL..high[0] / CELL {
            for y in low[1] / CELL..high[1] / CELL {
                for z in low[2] / CELL..high[2] / CELL {
                    cells.insert([x, y, z]);
                }
            }
        }
    }
    cells.len() as i128 * i128::from(CELL).pow(3)
}

fn closed_and_approximate(r: &Reconstructed) {
    assert!(r.mesh.is_solid());
    assert_ne!(r.mesh.kind(), MeshKind::Open);
    match r.mesh.parts().class {
        Exactness::Approximate { deviation, .. } => assert!(deviation.get() >= 1),
        other => panic!("not approximate: {other:?}"),
    }
    // No cell is counted twice.
    let c = r.log.cells;
    assert_eq!(r.undecided.len() as u32, c.undecided);
}

#[test]
fn boxes_that_overlap_touch_and_share_planes_give_exactly_their_union() {
    let seed = resolve("geom-mesh reconstruct union", Seed(0x299)).unwrap();
    let mut rng = SplitMix64::new(seed);
    for _ in 0..24 {
        let count = 1 + below(&mut rng, 3) as usize;
        let shapes = boxes(&mut rng, count);
        // Every box faces outward: a soup of closed boxes is the case healing's boolean resolves.
        let (points, faces) = soup(&shapes, &vec![false; count]);
        let r = run(&raw(&points, &faces));
        closed_and_approximate(&r);
        assert_eq!(
            six_volume(&r.mesh).to_i128().unwrap(),
            6 * union_volume(&shapes),
            "{shapes:?}"
        );
        // A lone box comes back as itself: its deviation is the rounding's step alone. Where boxes
        // overlap, the faces inside the other box are as far from the union as they are deep.
        match r.mesh.parts().class {
            Exactness::Approximate { deviation, .. } if count == 1 => {
                assert_eq!(deviation.get(), 1);
            }
            Exactness::Approximate { deviation, .. } => {
                assert!(deviation.get() <= 3 * 3 * 4 * CELL as u64, "{deviation:?}");
            }
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn a_box_turned_inside_out_is_turned_back_before_the_cells_are_counted() {
    let seed = resolve("geom-mesh reconstruct turns", Seed(0x2991)).unwrap();
    let mut rng = SplitMix64::new(seed);
    for _ in 0..16 {
        let shapes = boxes(&mut rng, 1);
        let (points, faces) = soup(&shapes, &[true]);
        let r = run(&raw(&points, &faces));
        assert_eq!(
            six_volume(&r.mesh).to_i128().unwrap(),
            6 * union_volume(&shapes)
        );
    }
}

#[test]
fn the_order_of_the_faces_and_where_each_starts_do_not_show() {
    let seed = resolve("geom-mesh reconstruct order", Seed(0x2992)).unwrap();
    let mut rng = SplitMix64::new(seed);
    for _ in 0..12 {
        let count = 1 + below(&mut rng, 3) as usize;
        let shapes = boxes(&mut rng, count);
        let (points, faces) = soup(&shapes, &vec![false; count]);
        let a = run(&raw(&points, &faces));
        let mut shuffled = faces.clone();
        // A deterministic shuffle from the seed.
        for i in (1..shuffled.len()).rev() {
            let j = below(&mut rng, i as u64 + 1) as usize;
            shuffled.swap(i, j);
        }
        for f in &mut shuffled {
            let k = below(&mut rng, f.len() as u64) as usize;
            f.rotate_left(k);
        }
        let b = run(&raw(&points, &shuffled));
        assert_eq!(a.mesh, b.mesh, "{shapes:?}");
        assert_eq!(a.log, b.log);
        assert_eq!(a.undecided, b.undecided);
    }
}

/// A box whose bottom is a frame round a square hole, the hole's corners at `from` to `to`.
fn box_with_a_hole(side: i64, from: i64, to: i64) -> RawMesh {
    let mut points = box_corners([0; 3], [side; 3]).to_vec();
    for (x, y) in [(from, from), (to, from), (to, to), (from, to)] {
        points.push([x, y, 0]);
    }
    let mut faces: Vec<Vec<u32>> = SIDES
        .iter()
        .map(|s| s.iter().map(|&v| v as u32).collect())
        .collect();
    faces.retain(|f| !f.iter().all(|&v| points[v as usize][2] == 0));
    let outer = [0_u32, 2, 3, 1];
    let inner = [8_u32, 11, 10, 9];
    for k in 0..4 {
        faces.push(vec![
            outer[k],
            outer[(k + 1) % 4],
            inner[(k + 1) % 4],
            inner[k],
        ]);
    }
    raw(&points, &faces)
}

#[test]
fn a_hole_of_any_size_in_a_flat_face_is_closed_in_its_plane() {
    let seed = resolve("geom-mesh reconstruct holes", Seed(0x2993)).unwrap();
    let mut rng = SplitMix64::new(seed);
    for _ in 0..16 {
        let side = 100 + below(&mut rng, 400);
        let from = 1 + below(&mut rng, (side / 2 - 2) as u64);
        let to = side - 1 - below(&mut rng, (side / 2 - 2) as u64);
        let r = run(&box_with_a_hole(side, from, to));
        closed_and_approximate(&r);
        assert_eq!(
            six_volume(&r.mesh).to_i128().unwrap(),
            6 * i128::from(side).pow(3),
            "side {side}, hole {from} to {to}"
        );
        assert_eq!(r.undecided, Vec::new());
    }
}
