//! Attacks on what piece G298's healing claims (19.7), on random soups from a printed seed
//! (`SPAREKIT_SEED` replaces it): no vertex moves more than ε, and only onto an input point; the
//! class says the largest move; the input's order does not show; a hole wider than ε is never
//! closed, and one within it closes at its own width; faces the wrong way round are found and
//! turned, whatever the flood starts from.

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

use std::num::NonZeroU64;

use geom_mesh::exact::build::Source;
use geom_mesh::exact::mass::Volume;
use geom_mesh::exact::raw::RawMesh;
use geom_mesh::exact::{ExactMesh, Role};
use geom_mesh::repair::{Heal, HealError, Healed, heal};
use geom_num::Exactness;
use geom_num::grid::{GridStep, P3, Steps};
use geom_num::wide::I256;
use geom_op::{NameId, OpId, OpPath};
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

const MM: i64 = 6400;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 26))
}

fn heal_at(mesh: &RawMesh, epsilon: u64) -> Result<Healed, HealError> {
    heal(
        mesh,
        Heal {
            epsilon: Steps(epsilon),
            step: GridStep::Q0,
        },
        Source {
            name: NameId::from_bytes([5; 16]),
            role: Role::FaceSet,
            flags: 0,
        },
        &OpPath::new(OpId::from_bytes([2; 16])),
        &Exec::serial(),
        &mut meter(),
    )
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

fn box_corners(low: [i64; 3], high: [i64; 3]) -> [[i64; 3]; 8] {
    std::array::from_fn(|i| [0, 1, 2].map(|a| if i >> a & 1 == 1 { high[a] } else { low[a] }))
}

/// Boxes on a lattice of 2 mm cells, a cell apart at least, each 1 mm on a side or more.
fn boxes(rng: &mut SplitMix64, count: usize) -> Vec<([i64; 3], [i64; 3])> {
    let mut cells: Vec<[i64; 3]> = Vec::new();
    while cells.len() < count {
        let c = [below(rng, 4), below(rng, 4), below(rng, 4)];
        if !cells.contains(&c) {
            cells.push(c);
        }
    }
    cells
        .into_iter()
        .map(|c| {
            let low = c.map(|v| 3 * MM * v);
            let size = [0; 3].map(|_| MM + below(rng, 2 * MM as u64 / 2));
            (low, [low[0] + size[0], low[1] + size[1], low[2] + size[2]])
        })
        .collect()
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

/// The boxes as a soup: every face has its own copies of its corners, each moved by `jitter`.
fn soup(
    boxes: &[([i64; 3], [i64; 3])],
    mut jitter: impl FnMut() -> [i64; 3],
) -> (Vec<[i64; 3]>, Vec<Vec<u32>>) {
    let (mut points, mut faces) = (Vec::new(), Vec::new());
    for &(low, high) in boxes {
        let corners = box_corners(low, high);
        for side in SIDES {
            let mut face = Vec::new();
            for v in side {
                let d = jitter();
                face.push(points.len() as u32);
                points.push([0, 1, 2].map(|a| corners[v][a] + d[a]));
            }
            faces.push(face);
        }
    }
    (points, faces)
}

fn volumes(boxes: &[([i64; 3], [i64; 3])]) -> i128 {
    boxes.iter().fold(0, |total, (l, h)| {
        total + (0..3).fold(1, |v, a| v * i128::from(h[a] - l[a]))
    })
}

fn distance_squared(a: P3, b: P3) -> i128 {
    let d = [
        a.x.get() - b.x.get(),
        a.y.get() - b.y.get(),
        a.z.get() - b.z.get(),
    ];
    d.iter().fold(0, |total, &v| total + i128::from(v).pow(2))
}

#[test]
fn no_vertex_moves_more_than_epsilon_or_off_the_input() {
    let seed = resolve("geom-mesh heal moves", Seed(0x298)).unwrap();
    let mut rng = SplitMix64::new(seed);
    for _ in 0..24 {
        let count = 1 + below(&mut rng, 3) as usize;
        let shapes = boxes(&mut rng, count);
        // ε stays under half a side, so no two corners of a box weld.
        let reach = below(&mut rng, 150) as u64;
        let epsilon = 2 * reach + below(&mut rng, 3000 - 2 * reach + 1) as u64;
        let (points, faces) = soup(&shapes, || {
            [0; 3].map(|_| below(&mut rng, reach + 1) - (reach as i64) / 2)
        });
        // Copies of one corner lie within √3 · reach of each other; ε is at least twice reach.
        let h = heal_at(&raw(&points, &faces), epsilon).unwrap();
        let input: Vec<P3> = points
            .iter()
            .map(|&[x, y, z]| P3::new(x, y, z).unwrap())
            .collect();
        let mut worst = 0;
        for j in &h.log.weld.joined {
            let d = distance_squared(j.from, j.to);
            assert!(d <= i128::from(epsilon).pow(2), "{j:?} at ε {epsilon}");
            assert!(input.contains(&j.from) && input.contains(&j.to));
            worst = worst.max(d);
        }
        let bound = h.log.weld.movement.get();
        assert!(i128::from(bound).pow(2) >= worst);
        assert!(bound == 0 || i128::from(bound - 1).pow(2) < worst);
        let class = h.mesh.parts().class;
        if bound == 0 {
            assert_eq!(class, Exactness::Exact);
        } else {
            assert_eq!(
                class,
                Exactness::Certified {
                    bound: Steps(bound),
                    step: GridStep::Q0
                }
            );
        }
        // Each box keeps its volume, give or take its faces' moves.
        let six = six_volume(&h.mesh).to_i128().unwrap();
        let exact = 6 * volumes(&shapes);
        let slack = 6 * 6 * 3 * (4 * MM as i128).pow(2) * i128::from(bound);
        assert!((six - exact).abs() <= slack, "{six} against {exact}");
    }
}

#[test]
fn the_order_of_the_input_does_not_show() {
    let seed = resolve("geom-mesh heal order", Seed(0x2981)).unwrap();
    let mut rng = SplitMix64::new(seed);
    for _ in 0..16 {
        let count = 1 + below(&mut rng, 3) as usize;
        let shapes = boxes(&mut rng, count);
        let (points, faces) = soup(&shapes, || [0; 3].map(|_| below(&mut rng, 60)));
        // The vertices renumbered by a random permutation, the faces shuffled, each loop turned
        // to start at a random corner.
        let mut order: Vec<usize> = (0..points.len()).collect();
        for i in (1..order.len()).rev() {
            order.swap(i, below(&mut rng, i as u64 + 1) as usize);
        }
        let mut place = vec![0_u32; points.len()];
        for (new, &old) in order.iter().enumerate() {
            place[old] = new as u32;
        }
        let moved_points: Vec<[i64; 3]> = order.iter().map(|&old| points[old]).collect();
        let mut moved_faces: Vec<Vec<u32>> = faces
            .iter()
            .map(|f| {
                let mut g: Vec<u32> = f.iter().map(|&v| place[v as usize]).collect();
                let at = below(&mut rng, g.len() as u64) as usize;
                g.rotate_left(at);
                g
            })
            .collect();
        for i in (1..moved_faces.len()).rev() {
            moved_faces.swap(i, below(&mut rng, i as u64 + 1) as usize);
        }
        let a = heal_at(&raw(&points, &faces), 640).unwrap();
        let b = heal_at(&raw(&moved_points, &moved_faces), 640).unwrap();
        assert_eq!(a.mesh, b.mesh);
        assert_eq!(a.log.weld.joined, b.log.weld.joined);
        assert_eq!(a.log.weld.movement, b.log.weld.movement);
    }
}

#[test]
fn a_hole_wider_than_epsilon_is_never_closed() {
    let seed = resolve("geom-mesh heal holes", Seed(0x2982)).unwrap();
    let mut rng = SplitMix64::new(seed);
    let side = 4 * MM;
    for _ in 0..40 {
        let leg = 1 + below(&mut rng, 6000);
        let epsilon = below(&mut rng, 6401) as u64;
        // A cube whose top lacks the triangle of legs `leg` at its corner (0, 0, side).
        let mut points: Vec<[i64; 3]> = box_corners([0; 3], [side; 3]).to_vec();
        points.extend([[leg, 0, side], [0, leg, side]]);
        let mut faces: Vec<Vec<u32>> = SIDES
            .iter()
            .map(|s| s.iter().map(|&v| v as u32).collect())
            .collect();
        faces[5] = vec![8, 5, 7, 6, 9];
        let got = heal_at(&raw(&points, &faces), epsilon);
        if leg as u64 <= epsilon {
            // Its two corners are within ε of the cube's corner, and join it.
            let h = got.unwrap();
            assert_eq!(h.log.weld.movement, Steps(leg as u64));
            assert_eq!(
                six_volume(&h.mesh),
                I256::from_i128(6 * i128::from(side).pow(3))
            );
        } else {
            // √2 · leg, rounded up.
            let n = 2 * leg * leg;
            let mut width = n.isqrt();
            if width * width < n {
                width += 1;
            }
            let e = got.unwrap_err();
            let boundaries = match &e {
                HealError::NotClosed { boundaries } => {
                    assert!(width <= 6400);
                    boundaries
                }
                HealError::ToleranceExceeded { boundaries } => {
                    assert!(width > 6400);
                    boundaries
                }
                other => panic!("{other:?}"),
            };
            assert_eq!(boundaries.len(), 1);
            assert_eq!(boundaries[0].width, Steps(width as u64));
            assert_eq!(boundaries[0].corners, 3);
        }
    }
}

#[test]
fn every_face_the_wrong_way_round_is_found_and_turned() {
    let seed = resolve("geom-mesh heal turns", Seed(0x2983)).unwrap();
    let mut rng = SplitMix64::new(seed);
    for _ in 0..24 {
        let count = 1 + below(&mut rng, 3) as usize;
        let shapes = boxes(&mut rng, count);
        let (points, mut faces) = soup(&shapes, || [0; 3]);
        let mut wrong = Vec::new();
        for (f, face) in faces.iter_mut().enumerate() {
            if below(&mut rng, 2) == 1 {
                face.reverse();
                wrong.push(f as u32);
            }
        }
        let h = heal_at(&raw(&points, &faces), 640).unwrap();
        assert_eq!(h.log.orient.turned, wrong);
        assert_eq!(h.log.orient.patches as usize, shapes.len());
        assert_eq!(h.mesh.parts().class, Exactness::Exact);
        assert_eq!(six_volume(&h.mesh), I256::from_i128(6 * volumes(&shapes)));
    }
}
