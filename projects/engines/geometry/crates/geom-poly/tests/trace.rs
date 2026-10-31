//! Every piece's two winding vectors against an oracle that counts boxes, and what a selection
//! keeps against the volume it should leave (G127, 11.2 and 11.3).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests: native arithmetic and short names, and a failure stops the test"
)]

use geom_num::plane::{Plane, Tpi};
use geom_num::{NumError, P3, Sign};
use geom_poly::convex::Piece;
use geom_poly::cut::{Clipped, clip, local_cuts};
use geom_poly::trace::{Cell, Classified, Polygon, TraceError, classify};
use geom_poly::winding::{Kept, Winding, WindingError, WindingTest, kept};
use geom_run::{Budget, WorkMeter};
use proptest::prelude::*;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The plane where coordinate `axis` is `at`, facing up that axis.
fn up(axis: usize, at: i64) -> Plane {
    let mut n = [0i128; 3];
    n[axis] = 1;
    Plane::new(n, -i128::from(at)).unwrap()
}

/// The same plane, facing down.
fn down(axis: usize, at: i64) -> Plane {
    let mut n = [0i128; 3];
    n[axis] = -1;
    Plane::new(n, i128::from(at)).unwrap()
}

#[derive(Clone, Copy, Debug)]
struct Block {
    low: [i64; 3],
    high: [i64; 3],
}

/// The side of `block` at coordinate `at` of `axis`, facing up that axis or down it.
fn side(block: &Block, axis: usize, at: i64, upward: bool) -> Piece {
    let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
    let (ul, uh) = (block.low[u], block.high[u]);
    let (vl, vh) = (block.low[v], block.high[v]);
    if upward {
        Piece {
            face: up(axis, at),
            bounds: vec![up(v, vl), down(u, uh), down(v, vh), up(u, ul)],
        }
    } else {
        Piece {
            face: down(axis, at),
            bounds: vec![up(u, ul), down(v, vh), down(u, uh), up(v, vl)],
        }
    }
}

/// `block`'s six faces, outward, as polygons of `entry`.
fn faces(block: &Block, entry: Option<u32>) -> Vec<Polygon> {
    let mut out = Vec::new();
    for axis in 0..3 {
        for (at, upward) in [(block.high[axis], true), (block.low[axis], false)] {
            out.push(Polygon {
                piece: side(block, axis, at, upward),
                entry,
            });
        }
    }
    out
}

/// Whether `block` holds a point given in quarter steps.
fn holds(block: &Block, p: [i64; 3]) -> bool {
    (0..3).all(|i| block.low[i] * 4 < p[i] && p[i] < block.high[i] * 4)
}

/// The vector at a point in quarter steps, off every plane, by looking at each block.
fn oracle(blocks: &[Block], p: [i64; 3]) -> Winding {
    let mut w = Winding::new();
    for (entry, block) in blocks.iter().enumerate() {
        if holds(block, p) {
            w.add(entry as u32, 1).unwrap();
        }
    }
    w
}

/// A piece's corners on the grid.
fn corners(piece: &Piece) -> Vec<[i64; 3]> {
    let k = piece.bounds.len();
    (0..k)
        .map(|i| {
            let t = Tpi::new(piece.face, piece.bounds[i], piece.bounds[(i + 1) % k]).unwrap();
            let r = t.to_grid().unwrap().value();
            [r.x.get(), r.y.get(), r.z.get()]
        })
        .collect()
}

/// The axis a piece's plane is square to, and whether it faces up it.
fn normal(piece: &Piece) -> (usize, bool) {
    let n = piece.face.n();
    let axis = n.iter().position(|c| *c != 0).unwrap();
    (axis, n[axis] > 0)
}

/// A point just in front of a piece and one just behind, in quarter steps, a quarter from its
/// middle: off every plane of a grid of whole steps, and over no other piece.
fn samples(piece: &Piece) -> ([i64; 3], [i64; 3]) {
    let c = corners(piece);
    let (axis, upward) = normal(piece);
    let mut middle = [0i64; 3];
    for i in 0..3 {
        let (low, high) = (
            c.iter().map(|p| p[i]).min().unwrap(),
            c.iter().map(|p| p[i]).max().unwrap(),
        );
        middle[i] = 2 * (low + high) + 1;
    }
    let (mut front, mut back) = (middle, middle);
    let plane = c[0][axis] * 4;
    front[axis] = plane + if upward { 1 } else { -1 };
    back[axis] = plane - if upward { 1 } else { -1 };
    (front, back)
}

/// Twice a piece's area.
fn area(piece: &Piece) -> i64 {
    let c = corners(piece);
    let (axis, _) = normal(piece);
    let span =
        |i: usize| c.iter().map(|p| p[i]).max().unwrap() - c.iter().map(|p| p[i]).min().unwrap();
    span((axis + 1) % 3) * span((axis + 2) % 3)
}

fn pieces_of(polygons: &[Polygon]) -> Vec<Piece> {
    polygons.iter().map(|p| p.piece.clone()).collect()
}

/// The whole scene in a box around it, from a reference point outside everything.
fn scene(blocks: &[Block]) -> Vec<Classified> {
    let polygons: Vec<Polygon> = blocks
        .iter()
        .enumerate()
        .flat_map(|(i, b)| faces(b, Some(i as u32)))
        .collect();
    let cuts = local_cuts(&pieces_of(&polygons), &mut meter()).unwrap();
    let cell = Cell {
        low: [-100; 3],
        high: [100; 3],
    };
    classify(&cell, &Winding::new(), &polygons, &cuts, &mut meter()).unwrap()
}

fn agrees(blocks: &[Block], classified: &[Classified]) {
    for c in classified {
        let (front, back) = samples(&c.piece);
        assert_eq!(c.front, oracle(blocks, front), "front of {:?}", c.piece);
        assert_eq!(c.back, oracle(blocks, back), "back of {:?}", c.piece);
    }
}

/// The volume of what `test` keeps of `classified`, from the divergence theorem on x alone.
fn volume(test: &WindingTest, classified: &[Classified]) -> i64 {
    classified
        .iter()
        .filter(|c| c.first)
        .map(|c| {
            let (axis, upward) = normal(&c.piece);
            if axis != 0 {
                return 0;
            }
            let along = i64::from(upward) * 2 - 1;
            let at = corners(&c.piece)[0][0];
            match kept(test, &c.front, &c.back) {
                Kept::No => 0,
                Kept::AsIs => along * at * area(&c.piece),
                Kept::Flipped => -along * at * area(&c.piece),
            }
        })
        .sum()
}

/// The same volume, one unit box at a time.
fn counted(test: &WindingTest, blocks: &[Block]) -> i64 {
    let mut count = 0;
    for x in -7..12 {
        for y in -7..12 {
            for z in -7..12 {
                let w = oracle(blocks, [4 * x + 2, 4 * y + 2, 4 * z + 2]);
                count += i64::from(test.holds(&w));
            }
        }
    }
    count
}

const A: Block = Block {
    low: [0, 0, 0],
    high: [4, 4, 4],
};

#[test]
fn one_block_is_inside_behind_and_outside_in_front() {
    let classified = scene(&[A]);
    assert_eq!(classified.len(), 6);
    let mut inside = Winding::new();
    inside.add(0, 1).unwrap();
    for c in &classified {
        assert_eq!(c.front, Winding::new());
        assert_eq!(c.back, inside);
        assert!(c.first);
    }
}

#[test]
fn two_overlapping_blocks_count_each_piece() {
    let blocks = [
        A,
        Block {
            low: [2, 2, 2],
            high: [6, 6, 6],
        },
    ];
    let classified = scene(&blocks);
    agrees(&blocks, &classified);
    assert_eq!(volume(&WindingTest::unite(), &classified), 120);
    assert_eq!(volume(&WindingTest::intersect(2), &classified), 8);
    assert_eq!(volume(&WindingTest::subtract(0), &classified), 56);
}

#[test]
fn a_block_inside_a_block_winds_twice() {
    let blocks = [
        A,
        Block {
            low: [1, 1, 1],
            high: [3, 3, 3],
        },
    ];
    let classified = scene(&blocks);
    agrees(&blocks, &classified);
    assert_eq!(volume(&WindingTest::subtract(0), &classified), 56);
}

#[test]
fn a_shared_face_disappears_in_a_union_and_is_counted_on_both_sides() {
    let blocks = [
        A,
        Block {
            low: [4, 0, 0],
            high: [8, 4, 4],
        },
    ];
    let classified = scene(&blocks);
    agrees(&blocks, &classified);
    let unite = WindingTest::unite();
    assert_eq!(volume(&unite, &classified), 128);
    let shared: Vec<&Classified> = classified
        .iter()
        .filter(|c| normal(&c.piece).0 == 0 && corners(&c.piece)[0][0] == 4)
        .collect();
    assert_eq!(shared.len(), 2);
    assert_eq!(shared.iter().filter(|c| c.first).count(), 1);
    for c in shared {
        assert_eq!(kept(&unite, &c.front, &c.back), Kept::No);
    }
    assert_eq!(volume(&WindingTest::intersect(2), &classified), 0);
}

#[test]
fn faces_that_overlap_on_one_plane_are_emitted_once() {
    // Flush on x = 4, the second reaching past the first.
    let blocks = [
        A,
        Block {
            low: [2, 2, 0],
            high: [4, 8, 4],
        },
    ];
    let classified = scene(&blocks);
    agrees(&blocks, &classified);
    assert_eq!(volume(&WindingTest::unite(), &classified), 64 + 48 - 16);
    let on_plane: Vec<&Classified> = classified
        .iter()
        .filter(|c| normal(&c.piece) == (0, true) && corners(&c.piece)[0][0] == 4)
        .collect();
    let emitted: i64 = on_plane
        .iter()
        .filter(|c| c.first)
        .map(|c| area(&c.piece))
        .sum();
    assert_eq!(emitted, 8 * 4);
}

#[test]
fn a_cutter_cuts_and_counts_nothing() {
    let sheet = Polygon {
        piece: side(
            &Block {
                low: [-2, -2, 0],
                high: [9, 9, 0],
            },
            2,
            2,
            true,
        ),
        entry: None,
    };
    let mut polygons = faces(&A, Some(0));
    polygons.push(sheet);
    let cuts = local_cuts(&pieces_of(&polygons), &mut meter()).unwrap();
    let cell = Cell {
        low: [-100; 3],
        high: [100; 3],
    };
    let classified = classify(&cell, &Winding::new(), &polygons, &cuts, &mut meter()).unwrap();
    assert_eq!(classified.len(), 4 * 2 + 2 + 5);
    agrees(&[A], &classified);
}

/// `polygons` clipped to `cell`, those on its faces kept.
fn clipped(polygons: &[Polygon], cell: &Cell) -> Vec<Polygon> {
    let mut walls = Vec::new();
    for axis in 0..3 {
        walls.push(up(axis, cell.low[axis]));
        walls.push(down(axis, cell.high[axis]));
    }
    let mut out = Vec::new();
    for polygon in polygons {
        let mut piece = Some(polygon.piece.clone());
        for wall in &walls {
            piece = piece.and_then(|p| match clip(&p, wall).unwrap() {
                Clipped::Front | Clipped::On => Some(p),
                Clipped::Split(front, _) => Some(front),
                Clipped::Back => None,
            });
        }
        if let Some(piece) = piece {
            out.push(Polygon {
                piece,
                entry: polygon.entry,
            });
        }
    }
    out
}

/// One box of a subdivision: its polygons clipped to it, its reference vector from the oracle.
fn in_cell(blocks: &[Block], cell: &Cell) -> Vec<Classified> {
    let all: Vec<Polygon> = blocks
        .iter()
        .enumerate()
        .flat_map(|(i, b)| faces(b, Some(i as u32)))
        .collect();
    let polygons = clipped(&all, cell);
    let cuts = local_cuts(&pieces_of(&polygons), &mut meter()).unwrap();
    let doubled = Cell {
        low: cell.low.map(|v| v * 2),
        high: cell.high.map(|v| v * 2),
    };
    let start = cell.low.map(|v| v * 4 + 1);
    classify(
        &doubled,
        &oracle(blocks, start),
        &polygons,
        &cuts,
        &mut meter(),
    )
    .unwrap()
}

#[test]
fn a_box_of_a_subdivision_needs_only_its_own_polygons() {
    let blocks = [
        A,
        Block {
            low: [2, 2, 2],
            high: [6, 6, 6],
        },
    ];
    for (low, high) in [
        ([1, 1, 1], [3, 3, 3]),
        ([3, 3, 3], [5, 5, 5]),
        ([0, 0, 0], [2, 2, 2]),
        // A face of the box lies on a face of each block.
        ([4, 0, 0], [6, 6, 6]),
        ([0, 0, 2], [8, 8, 4]),
    ] {
        agrees(&blocks, &in_cell(&blocks, &Cell { low, high }));
    }
}

#[test]
fn the_same_polygons_give_the_same_vectors() {
    let blocks = [
        A,
        Block {
            low: [2, -2, 1],
            high: [7, 3, 3],
        },
    ];
    assert_eq!(scene(&blocks), scene(&blocks));
}

#[test]
fn a_small_budget_stops_the_trace() {
    let polygons = faces(&A, Some(0));
    let cuts = local_cuts(&pieces_of(&polygons), &mut meter()).unwrap();
    let cell = Cell {
        low: [-100; 3],
        high: [100; 3],
    };
    let mut small = WorkMeter::new(Budget::new(3, u64::MAX));
    let got = classify(&cell, &Winding::new(), &polygons, &cuts, &mut small);
    assert!(matches!(got, Err(TraceError::Stopped(_))), "{got:?}");
    assert_eq!(got.unwrap_err().to_string(), "the meter stopped the trace");
}

#[test]
fn a_count_past_its_range_is_refused() {
    let polygons = faces(&A, Some(0));
    let cuts = local_cuts(&pieces_of(&polygons), &mut meter()).unwrap();
    let cell = Cell {
        low: [-100; 3],
        high: [100; 3],
    };
    let mut full = Winding::new();
    full.add(0, i32::MAX).unwrap();
    let got = classify(&cell, &full, &polygons, &cuts, &mut meter()).unwrap_err();
    assert_eq!(got, TraceError::Winding(WindingError::Overflow));
    assert_eq!(got.to_string(), "a winding number left its range");
}

#[test]
fn a_piece_that_is_no_polygon_is_refused() {
    let polygons = faces(&A, Some(0));
    let thin = Piece {
        face: polygons[0].piece.face,
        bounds: vec![],
    };
    let cell = Cell {
        low: [-100; 3],
        high: [100; 3],
    };
    let got = classify(
        &cell,
        &Winding::new(),
        &polygons,
        &[vec![thin]],
        &mut meter(),
    )
    .unwrap_err();
    assert_eq!(got, TraceError::Num(NumError::Degenerate));
    assert!(got.to_string().starts_with("a box that cannot be traced"));
}

/// A tetrahedron's outward planes: each is the face opposite that corner.
fn outward(corner: [[i64; 3]; 4]) -> Option<[Plane; 4]> {
    let at = |p: [i64; 3]| P3::new(p[0], p[1], p[2]).unwrap();
    let mut planes = Vec::new();
    for i in 0..4 {
        let [a, b, c] = [(i + 1) % 4, (i + 2) % 4, (i + 3) % 4].map(|k| at(corner[k]));
        let plane = Plane::through(a, b, c).ok()?;
        planes.push(match plane.side(at(corner[i])).unwrap() {
            Sign::Zero => return None,
            Sign::Negative => plane,
            Sign::Positive => turned(&plane),
        });
    }
    planes.try_into().ok()
}

fn turned(plane: &Plane) -> Plane {
    Plane::new(plane.n().map(|v| -v), -plane.d()).unwrap()
}

/// A tetrahedron's four faces, outward.
fn wedge(planes: &[Plane; 4], entry: u32) -> Vec<Polygon> {
    (0..4)
        .map(|i| Polygon {
            piece: Piece {
                face: planes[i],
                bounds: (1..4).map(|k| turned(&planes[(i + k) % 4])).collect(),
            },
            entry: Some(entry),
        })
        .collect()
}

fn inside(planes: &[Plane; 4], p: [f64; 3]) -> bool {
    planes.iter().all(|q| {
        let n = q.n();
        (0..3).map(|i| n[i] as f64 * p[i]).sum::<f64>() + (q.d() as f64) < 0.0
    })
}

/// The vector at a point, by asking each tetrahedron.
fn wedges_at(solids: &[[Plane; 4]], p: [f64; 3]) -> Winding {
    let mut w = Winding::new();
    for (entry, solid) in solids.iter().enumerate() {
        if inside(solid, p) {
            w.add(entry as u32, 1).unwrap();
        }
    }
    w
}

/// A point just off the middle of a piece, on the side `by` of it.
fn off_middle(piece: &Piece, by: f64) -> [f64; 3] {
    let k = piece.bounds.len();
    let mut middle = [0.0f64; 3];
    for i in 0..k {
        let t = Tpi::new(piece.face, piece.bounds[i], piece.bounds[(i + 1) % k]).unwrap();
        let (x, w) = t.homogeneous().unwrap();
        let w = w.to_i128().unwrap() as f64;
        for axis in 0..3 {
            middle[axis] += x[axis].to_i128().unwrap() as f64 / w / k as f64;
        }
    }
    let n = piece.face.n().map(|v| v as f64);
    let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    [0, 1, 2].map(|axis| middle[axis] + by * n[axis] / length)
}

fn wedges_agree(solids: &[[Plane; 4]]) {
    let polygons: Vec<Polygon> = solids
        .iter()
        .enumerate()
        .flat_map(|(i, s)| wedge(s, i as u32))
        .collect();
    let cuts = local_cuts(&pieces_of(&polygons), &mut meter()).unwrap();
    let cell = Cell {
        low: [-100; 3],
        high: [100; 3],
    };
    for c in classify(&cell, &Winding::new(), &polygons, &cuts, &mut meter()).unwrap() {
        let (front, back) = (off_middle(&c.piece, 1e-7), off_middle(&c.piece, -1e-7));
        assert_eq!(c.front, wedges_at(solids, front), "front of {:?}", c.piece);
        assert_eq!(c.back, wedges_at(solids, back), "back of {:?}", c.piece);
    }
}

#[test]
fn tilted_faces_are_crossed_where_a_path_leaves_them_by_either_of_two_walls() {
    let a = outward([[0, 0, 0], [9, 1, 0], [1, 8, 1], [2, 1, 7]]).unwrap();
    let b = outward([[1, 1, 1], [8, 3, 2], [3, 9, 4], [4, 2, 9]]).unwrap();
    wedges_agree(&[a]);
    wedges_agree(&[a, b]);
    wedges_agree(&[b, a]);
}

fn any_wedge() -> impl Strategy<Value = [Plane; 4]> {
    let corner = || [-8i64..=8, -8i64..=8, -8i64..=8];
    [corner(), corner(), corner(), corner()].prop_filter_map("flat", outward)
}

fn any_block() -> impl Strategy<Value = Block> {
    ([-6i64..6, -6i64..6, -6i64..6], [1i64..6, 1i64..6, 1i64..6]).prop_map(|(low, size)| Block {
        low,
        high: [low[0] + size[0], low[1] + size[1], low[2] + size[2]],
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(160))]

    #[test]
    fn every_tilted_piece_agrees_with_the_solids(
        solids in prop::collection::vec(any_wedge(), 1..4)
    ) {
        wedges_agree(&solids);
    }

    #[test]
    fn every_piece_agrees_with_the_blocks(blocks in prop::collection::vec(any_block(), 1..5)) {
        let classified = scene(&blocks);
        agrees(&blocks, &classified);
        for test in [
            WindingTest::unite(),
            WindingTest::intersect(blocks.len() as u32),
            WindingTest::subtract(0),
        ] {
            prop_assert_eq!(volume(&test, &classified), counted(&test, &blocks));
        }
    }

    #[test]
    fn every_box_agrees_with_the_blocks(
        blocks in prop::collection::vec(any_block(), 1..4),
        low in [-6i64..6, -6i64..6, -6i64..6],
        size in [1i64..7, 1i64..7, 1i64..7],
    ) {
        let cell = Cell {
            low,
            high: [low[0] + size[0], low[1] + size[1], low[2] + size[2]],
        };
        agrees(&blocks, &in_cell(&blocks, &cell));
    }
}
