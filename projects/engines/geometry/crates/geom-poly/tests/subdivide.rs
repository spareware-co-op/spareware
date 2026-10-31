//! A whole arrangement, box by box, against an oracle that counts blocks, on any number of threads
//! (G128, 11.3).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests: native arithmetic and short names, and a failure stops the test"
)]

use geom_num::NumError;
use geom_num::plane::{Plane, Tpi};
use geom_poly::convex::Piece;
use geom_poly::subdivide::{LEAF_POLYGONS, around, arrange};
use geom_poly::trace::{Cell, Classified, Polygon, TraceError};
use geom_poly::winding::{Kept, Winding, WindingTest, kept};
#[cfg(not(target_family = "wasm"))]
use geom_run::Pool;
use geom_run::{Budget, Exec, RunError, WorkMeter};
use proptest::prelude::*;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn up(axis: usize, at: i64) -> Plane {
    let mut n = [0i128; 3];
    n[axis] = 1;
    Plane::new(n, -i128::from(at)).unwrap()
}

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

fn polygons(blocks: &[Block]) -> Vec<Polygon> {
    let mut out = Vec::new();
    for (entry, block) in blocks.iter().enumerate() {
        for axis in 0..3 {
            for (at, upward) in [(block.high[axis], true), (block.low[axis], false)] {
                out.push(Polygon {
                    piece: side(block, axis, at, upward),
                    entry: Some(entry as u32),
                });
            }
        }
    }
    out
}

/// Whether `block` holds a point given in eighths of a step.
fn holds(block: &Block, p: [i64; 3]) -> bool {
    (0..3).all(|i| block.low[i] * 8 < p[i] && p[i] < block.high[i] * 8)
}

fn oracle(blocks: &[Block], p: [i64; 3]) -> Winding {
    let mut w = Winding::new();
    for (entry, block) in blocks.iter().enumerate() {
        if holds(block, p) {
            w.add(entry as u32, 1).unwrap();
        }
    }
    w
}

/// A piece's corners in half steps, where boxes put them.
fn corners(piece: &Piece) -> Vec<[i64; 3]> {
    let k = piece.bounds.len();
    (0..k)
        .map(|i| {
            let t = Tpi::new(piece.face, piece.bounds[i], piece.bounds[(i + 1) % k]).unwrap();
            let (x, w) = t.homogeneous().unwrap();
            let w = w.to_i128().unwrap();
            x.map(|v| {
                let doubled = v.to_i128().unwrap() * 2;
                assert_eq!(doubled % w, 0, "a corner off the half steps");
                (doubled / w) as i64
            })
        })
        .collect()
}

fn normal(piece: &Piece) -> (usize, bool) {
    let n = piece.face.n();
    let axis = n.iter().position(|c| *c != 0).unwrap();
    (axis, n[axis] > 0)
}

/// Points an eighth of a step in front of and behind a piece's middle, and an eighth along it.
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

/// A piece's area in square half steps.
fn area(piece: &Piece) -> i64 {
    let c = corners(piece);
    let (axis, _) = normal(piece);
    let span =
        |i: usize| c.iter().map(|p| p[i]).max().unwrap() - c.iter().map(|p| p[i]).min().unwrap();
    span((axis + 1) % 3) * span((axis + 2) % 3)
}

fn agrees(blocks: &[Block], classified: &[Classified]) {
    for c in classified {
        let (front, back) = samples(&c.piece);
        assert_eq!(c.front, oracle(blocks, front), "front of {:?}", c.piece);
        assert_eq!(c.back, oracle(blocks, back), "back of {:?}", c.piece);
    }
}

/// What `test` keeps, in cubic half steps, by the divergence theorem on x.
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

/// The same, one unit box at a time.
fn counted(test: &WindingTest, blocks: &[Block], span: std::ops::Range<i64>) -> i64 {
    let mut count = 0;
    for x in span.clone() {
        for y in span.clone() {
            for z in span.clone() {
                let w = oracle(blocks, [8 * x + 4, 8 * y + 4, 8 * z + 4]);
                count += i64::from(test.holds(&w));
            }
        }
    }
    count * 8
}

fn run(blocks: &[Block], leaf: usize, tests: &[WindingTest]) -> Vec<Classified> {
    let list = polygons(blocks);
    let cell = around(&list).unwrap();
    arrange(
        &Exec::serial(),
        cell,
        Winding::new(),
        &list,
        leaf,
        tests,
        &mut meter(),
    )
    .unwrap()
}

/// A pool that runs each task on its own thread, last first.
#[cfg(not(target_family = "wasm"))]
struct Threads;

#[cfg(not(target_family = "wasm"))]
impl Pool for Threads {
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
        std::thread::scope(|scope| {
            for task in tasks.iter_mut().rev() {
                scope.spawn(task);
            }
        });
    }
}

const A: Block = Block {
    low: [0, 0, 0],
    high: [4, 4, 4],
};
const B: Block = Block {
    low: [2, 2, 2],
    high: [6, 6, 6],
};

#[test]
fn the_box_around_polygons_is_clear_of_them_on_odd_half_steps() {
    let cell = around(&polygons(&[A, B])).unwrap();
    assert_eq!(cell.low, [-3; 3]);
    assert_eq!(cell.high, [15; 3]);
    assert_eq!(
        around(&[]).unwrap(),
        Cell {
            low: [-1; 3],
            high: [1; 3]
        }
    );
    let broken = Polygon {
        piece: Piece {
            face: up(0, 0),
            bounds: vec![up(0, 1), up(0, 2)],
        },
        entry: None,
    };
    assert_eq!(around(&[broken]), Err(NumError::Degenerate));
}

#[test]
fn a_small_leaf_gives_the_vectors_a_large_one_does() {
    let blocks = [A, B];
    for leaf in [1, 2, 4, 8, LEAF_POLYGONS] {
        let classified = run(&blocks, leaf, &[]);
        agrees(&blocks, &classified);
        assert_eq!(volume(&WindingTest::unite(), &classified), 120 * 8);
        assert_eq!(volume(&WindingTest::intersect(2), &classified), 8 * 8);
    }
}

#[test]
fn a_polygon_names_its_source_in_every_box() {
    let list = polygons(&[A, B]);
    let cell = around(&list).unwrap();
    let classified = arrange(
        &Exec::serial(),
        cell,
        Winding::new(),
        &list,
        1,
        &[],
        &mut meter(),
    )
    .unwrap();
    assert!(classified.len() > list.len());
    for c in &classified {
        let source = &list[c.polygon];
        assert_eq!(c.piece.face, source.piece.face);
    }
}

#[test]
fn a_test_that_is_settled_drops_its_box_and_nothing_it_keeps() {
    let blocks = [A, B];
    let test = WindingTest::intersect(2);
    let whole = run(&blocks, 1, &[]);
    let dropped = run(&blocks, 1, std::slice::from_ref(&test));
    assert!(dropped.len() < whole.len(), "{}", dropped.len());
    assert_eq!(volume(&test, &dropped), volume(&test, &whole));
    assert!(dropped.iter().any(|c| kept(&test, &c.front, &c.back) != Kept::No));
}

#[test]
fn a_box_with_a_cutter_in_it_stays() {
    let blocks = [A];
    let mut list = polygons(&blocks);
    list.push(Polygon {
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
    });
    let cell = around(&list).unwrap();
    let go = |tests: &[WindingTest]| {
        arrange(
            &Exec::serial(),
            cell,
            Winding::new(),
            &list,
            1,
            tests,
            &mut meter(),
        )
        .unwrap()
    };
    let of_sheet = |got: &[Classified]| got.iter().filter(|c| c.polygon == 6).count();
    let (whole, dropped) = (go(&[]), go(&[WindingTest::intersect(5)]));
    assert!(dropped.len() < whole.len());
    assert!(of_sheet(&dropped) > 0);
    assert_eq!(of_sheet(&dropped), of_sheet(&whole));
}

#[test]
fn a_split_passes_over_a_plane_that_holds_a_polygon() {
    // A sheet on x = 3.5, the first split of the box.
    let sheet = Polygon {
        piece: Piece {
            face: Plane::new([2, 0, 0], -7).unwrap(),
            bounds: vec![up(1, 1), down(2, 6), down(1, 6), up(2, 1)],
        },
        entry: None,
    };
    let mut list = polygons(&[Block {
        low: [1, 1, 1],
        high: [6, 6, 6],
    }]);
    list.push(sheet);
    let cell = Cell {
        low: [-1; 3],
        high: [15; 3],
    };
    let got = arrange(
        &Exec::serial(),
        cell,
        Winding::new(),
        &list,
        1,
        &[],
        &mut meter(),
    )
    .unwrap();
    assert!(got.iter().any(|c| c.polygon == 6));
}

#[test]
fn twenty_cubes_on_one_top_and_bottom_stop_at_their_shared_planes() {
    // Each overlaps the next, all between z = 0 and z = 4 (EMBER's figure 25).
    let blocks: Vec<Block> = (0..20)
        .map(|i| Block {
            low: [i, 0, 0],
            high: [i + 4, 4, 4],
        })
        .collect();
    let classified = run(&blocks, 8, &[]);
    agrees(&blocks, &classified);
    assert_eq!(volume(&WindingTest::unite(), &classified), 23 * 16 * 8);
}

#[test]
fn ten_by_ten_crossing_beams_are_counted_at_every_crossing() {
    let mut blocks = Vec::new();
    for i in 0..10 {
        blocks.push(Block {
            low: [i * 3, 0, 0],
            high: [i * 3 + 1, 30, 1],
        });
        blocks.push(Block {
            low: [0, i * 3, 1],
            high: [30, i * 3 + 1, 2],
        });
        blocks.push(Block {
            low: [i * 3, i * 3, 0],
            high: [i * 3 + 1, i * 3 + 1, 2],
        });
    }
    let test = WindingTest::unite();
    let classified = run(&blocks, LEAF_POLYGONS, &[]);
    agrees(&blocks, &classified);
    assert_eq!(
        volume(&test, &classified),
        counted(&test, &blocks, -1..31)
    );
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn the_same_pieces_come_out_on_any_threads() {
    let blocks = [
        A,
        B,
        Block {
            low: [1, -2, 1],
            high: [3, 9, 3],
        },
    ];
    let list = polygons(&blocks);
    let cell = around(&list).unwrap();
    let go = |exec: &Exec<'_>| {
        arrange(
            exec,
            cell,
            Winding::new(),
            &list,
            2,
            &[],
            &mut meter(),
        )
        .unwrap()
    };
    assert_eq!(go(&Exec::serial()), go(&Exec::with_pool(&Threads)));
}

#[test]
fn a_small_budget_stops_the_arrangement() {
    let list = polygons(&[A, B]);
    let cell = around(&list).unwrap();
    let mut small = WorkMeter::new(Budget::new(20, u64::MAX));
    let got = arrange(
        &Exec::serial(),
        cell,
        Winding::new(),
        &list,
        2,
        &[],
        &mut small,
    );
    assert!(
        matches!(
            got,
            Err(RunError::BudgetExceeded(_)
                | RunError::Failed(TraceError::Stopped(_)))
        ),
        "{got:?}"
    );
}

fn any_block() -> impl Strategy<Value = Block> {
    (
        [-6i64..6, -6i64..6, -6i64..6],
        [1i64..6, 1i64..6, 1i64..6],
    )
        .prop_map(|(low, size)| Block {
            low,
            high: [low[0] + size[0], low[1] + size[1], low[2] + size[2]],
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn every_arrangement_agrees_with_the_blocks(
        blocks in prop::collection::vec(any_block(), 1..5),
        leaf in 1usize..12,
    ) {
        let classified = run(&blocks, leaf, &[]);
        agrees(&blocks, &classified);
        for test in [
            WindingTest::unite(),
            WindingTest::intersect(blocks.len() as u32),
            WindingTest::subtract(0),
        ] {
            let dropped = run(&blocks, leaf, std::slice::from_ref(&test));
            let want = counted(&test, &blocks, -7..12);
            prop_assert_eq!(volume(&test, &classified), want);
            prop_assert_eq!(volume(&test, &dropped), want);
        }
    }
}
