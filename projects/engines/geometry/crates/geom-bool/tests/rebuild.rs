//! Kept pieces made into bodies, against the checker and closed forms, and the same bytes however
//! the boxes fell (G129, 11.3).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests: native arithmetic and short names, and a failure stops the test"
)]

use geom_bool::rebuild::{Carrier, RebuildError, rebuild};
use geom_check::{Level, check};
use geom_num::plane::Plane;
use geom_num::{Exactness, NumError, P3, Sign};
use geom_poly::convex::{Piece, convex_pieces};
use geom_poly::merge::{Merge, MergeError, Outward, select};
use geom_poly::subdivide::{LEAF_POLYGONS, around, arrange};
use geom_poly::trace::{Classified, Polygon};
use geom_poly::winding::{Winding, WindingTest};
use geom_run::{Budget, Exec, WorkMeter};
use geom_surface::Plane as Flat;
use geom_topo::{Body, BodyKind, Ix, PointKind, Sense, SurfaceKind};
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

fn block(low: [i64; 3], high: [i64; 3]) -> Block {
    Block { low, high }
}

fn side(b: &Block, axis: usize, at: i64, upward: bool) -> Piece {
    let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
    let (ul, uh) = (b.low[u], b.high[u]);
    let (vl, vh) = (b.low[v], b.high[v]);
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

/// The flat surface of a block's side, its normal the side's.
fn flat(axis: usize, at: i64, upward: bool) -> SurfaceKind {
    let mut origin = [0.0; 3];
    origin[axis] = at as f64;
    let (mut along, mut across) = ([0.0; 3], [0.0; 3]);
    along[(axis + 1) % 3] = 1.0;
    across[(axis + 2) % 3] = if upward { 1.0 } else { -1.0 };
    let wide = (-1e9, 1e9);
    SurfaceKind::Plane(Flat::new(origin, along, across, (wide, wide)).unwrap())
}

fn scene(blocks: &[Block]) -> (Vec<Polygon>, Vec<Carrier>) {
    let mut polygons = Vec::new();
    let mut carriers = Vec::new();
    for (entry, b) in blocks.iter().enumerate() {
        for axis in 0..3 {
            for (at, upward) in [(b.high[axis], true), (b.low[axis], false)] {
                polygons.push(Polygon {
                    piece: side(b, axis, at, upward),
                    entry: Some(entry as u32),
                });
                carriers.push(Carrier {
                    surface: flat(axis, at, upward),
                    sense: Sense::Forward,
                });
            }
        }
    }
    (polygons, carriers)
}

fn classified(polygons: &[Polygon], leaf: usize) -> Vec<Classified> {
    let cell = around(polygons).unwrap();
    arrange(
        &Exec::serial(),
        cell,
        Winding::new(),
        polygons,
        leaf,
        &[],
        &mut meter(),
    )
    .unwrap()
}

fn body(blocks: &[Block], test: &WindingTest, leaf: usize) -> Body {
    let (polygons, carriers) = scene(blocks);
    let kept = select(test, &classified(&polygons, leaf)).unwrap();
    rebuild(&kept, &carriers, Merge::RemoveRedundant, &mut meter()).unwrap()
}

/// `body`, held to every group of the checker.
fn checked(body: &Body) {
    let report = check(body, Exactness::Exact, Level::Full, &mut meter()).unwrap();
    assert!(report.passed(), "{report:?}");
}

fn counts(b: &Body) -> [u32; 5] {
    let a = b.arenas();
    [
        a.shells.len(),
        a.faces.len(),
        a.loops.len(),
        a.edges.len(),
        a.vertices.len(),
    ]
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
fn one_block_is_a_block_again() {
    let b = body(&[A], &WindingTest::unite(), LEAF_POLYGONS);
    assert_eq!(counts(&b), [1, 6, 6, 12, 8]);
    assert_eq!(b.kind(), BodyKind::Solid);
    checked(&b);
}

#[test]
fn overlapping_blocks_unite_into_one_shell() {
    let b = body(&[A, B], &WindingTest::unite(), LEAF_POLYGONS);
    assert_eq!(counts(&b)[0], 1);
    // Three faces each lost a corner, three more do, and three reentrant corners stand between.
    assert_eq!(counts(&b)[1], 12);
    checked(&b);
}

#[test]
fn their_intersection_is_a_cube_and_their_difference_a_notched_one() {
    let both = body(&[A, B], &WindingTest::intersect(2), LEAF_POLYGONS);
    assert_eq!(counts(&both), [1, 6, 6, 12, 8]);
    checked(&both);
    let less = body(&[A, B], &WindingTest::subtract(0), LEAF_POLYGONS);
    assert_eq!(counts(&less), [1, 9, 9, 21, 14]);
    checked(&less);
}

#[test]
fn a_hole_right_through_makes_a_face_with_two_loops() {
    let tool = block([1, 1, -1], [3, 3, 5]);
    let b = body(&[A, tool], &WindingTest::subtract(0), LEAF_POLYGONS);
    assert_eq!(counts(&b), [1, 10, 12, 24, 16]);
    checked(&b);
}

#[test]
fn a_block_inside_a_block_leaves_a_cavity() {
    let tool = block([1, 1, 1], [3, 3, 3]);
    let b = body(&[A, tool], &WindingTest::subtract(0), LEAF_POLYGONS);
    assert_eq!(counts(&b), [2, 12, 12, 24, 16]);
    assert_eq!(b.arenas().regions.len(), 2);
    checked(&b);
}

#[test]
fn blocks_apart_make_two_solids_and_one_in_a_cavity_makes_three() {
    let far = block([9, 0, 0], [12, 4, 4]);
    let b = body(&[A, far], &WindingTest::unite(), LEAF_POLYGONS);
    assert_eq!(b.arenas().regions.len(), 3);
    checked(&b);
    // A shell, a void in it, and a lump in the void.
    let outer = block([0, 0, 0], [9, 9, 9]);
    let void = block([2, 2, 2], [7, 7, 7]);
    let lump = block([4, 4, 4], [5, 5, 5]);
    let (mut polygons, mut carriers) = scene(&[outer, void, lump]);
    let test = WindingTest::new(vec![
        geom_poly::winding::Term::Inside(0),
        geom_poly::winding::Term::Inside(1),
        geom_poly::winding::Term::Not,
        geom_poly::winding::Term::And,
        geom_poly::winding::Term::Inside(2),
        geom_poly::winding::Term::Or,
    ])
    .unwrap();
    let kept = select(&test, &classified(&polygons, LEAF_POLYGONS)).unwrap();
    polygons.clear();
    carriers.shrink_to_fit();
    let b = rebuild(&kept, &carriers, Merge::RemoveRedundant, &mut meter()).unwrap();
    assert_eq!(b.arenas().shells.len(), 3);
    assert_eq!(b.arenas().regions.len(), 3);
    checked(&b);
}

#[test]
fn a_shared_face_disappears_and_flush_faces_merge() {
    let right = block([4, 0, 0], [8, 4, 4]);
    let b = body(&[A, right], &WindingTest::unite(), LEAF_POLYGONS);
    assert_eq!(counts(&b), [1, 6, 6, 12, 8]);
    checked(&b);
}

#[test]
fn seams_stay_when_asked() {
    let right = block([4, 0, 0], [8, 4, 4]);
    let (polygons, carriers) = scene(&[A, right]);
    let kept = select(&WindingTest::unite(), &classified(&polygons, LEAF_POLYGONS)).unwrap();
    let b = rebuild(&kept, &carriers, Merge::KeepSeams, &mut meter()).unwrap();
    assert_eq!(counts(&b), [1, 10, 10, 20, 12]);
    checked(&b);
}

#[test]
fn blocks_that_share_only_an_edge_stay_two_shells_of_a_general_body() {
    let other = block([4, 4, 0], [8, 8, 4]);
    let b = body(&[A, other], &WindingTest::unite(), LEAF_POLYGONS);
    assert_eq!(b.kind(), BodyKind::General);
    assert_eq!(counts(&b)[0], 2);
}

#[test]
fn nothing_kept_is_an_empty_body() {
    let far = block([9, 0, 0], [12, 4, 4]);
    let b = body(&[A, far], &WindingTest::intersect(2), LEAF_POLYGONS);
    assert_eq!(counts(&b), [0, 0, 0, 0, 0]);
    assert_eq!(b.arenas().regions.len(), 1);
}

#[test]
fn the_same_body_comes_out_however_the_boxes_fall() {
    let blocks = [A, B, block([1, -2, 1], [3, 9, 3])];
    let test = WindingTest::unite();
    let whole = body(&blocks, &test, 1000);
    for leaf in [1, 2, 4, 8, LEAF_POLYGONS] {
        assert_eq!(body(&blocks, &test, leaf), whole, "leaf {leaf}");
    }
}

#[test]
fn the_same_body_comes_out_whatever_order_the_pieces_came_in() {
    let (polygons, carriers) = scene(&[A, B]);
    let mut kept = select(&WindingTest::unite(), &classified(&polygons, 4)).unwrap();
    let once = rebuild(&kept, &carriers, Merge::RemoveRedundant, &mut meter()).unwrap();
    kept.reverse();
    let again = rebuild(&kept, &carriers, Merge::RemoveRedundant, &mut meter()).unwrap();
    assert_eq!(once, again);
}

#[test]
fn pieces_that_do_not_close_are_refused() {
    let (polygons, carriers) = scene(&[A]);
    let mut kept = select(&WindingTest::unite(), &classified(&polygons, LEAF_POLYGONS)).unwrap();
    kept.pop();
    assert_eq!(
        rebuild(&kept, &carriers, Merge::RemoveRedundant, &mut meter()),
        Err(RebuildError::Merge(MergeError::Open))
    );
}

#[test]
fn pieces_that_overlap_are_refused() {
    let (polygons, carriers) = scene(&[A]);
    let mut kept = select(&WindingTest::unite(), &classified(&polygons, LEAF_POLYGONS)).unwrap();
    let copy: Outward = kept[0].clone();
    kept.push(copy);
    assert_eq!(
        rebuild(&kept, &carriers, Merge::RemoveRedundant, &mut meter()),
        Err(RebuildError::Merge(MergeError::Overlap))
    );
}

#[test]
fn a_shell_turned_inside_out_is_refused() {
    let (polygons, carriers) = scene(&[A]);
    let test = WindingTest::new(vec![
        geom_poly::winding::Term::Inside(0),
        geom_poly::winding::Term::Not,
    ])
    .unwrap();
    let kept = select(&test, &classified(&polygons, LEAF_POLYGONS)).unwrap();
    assert_eq!(
        rebuild(&kept, &carriers, Merge::RemoveRedundant, &mut meter()),
        Err(RebuildError::Merge(MergeError::Inside))
    );
}

#[test]
fn errors_say_what_was_wrong() {
    assert_eq!(
        RebuildError::from(MergeError::Open).to_string(),
        "kept pieces do not close"
    );
    assert!(
        RebuildError::from(NumError::Degenerate)
            .to_string()
            .starts_with("pieces that cannot be rebuilt")
    );
    assert_eq!(RebuildError::Broken.to_string(), "a rebuild lost a row");
}

#[test]
fn a_small_budget_stops_the_rebuild() {
    let (polygons, carriers) = scene(&[A]);
    let kept = select(&WindingTest::unite(), &classified(&polygons, LEAF_POLYGONS)).unwrap();
    let mut small = WorkMeter::new(Budget::new(3, u64::MAX));
    let got = rebuild(&kept, &carriers, Merge::RemoveRedundant, &mut small);
    assert!(matches!(
        got,
        Err(RebuildError::Merge(MergeError::Stopped(_)))
    ));
}

/// A tetrahedron's faces, each the plane through three corners, outward.
fn tetrahedron(
    corner: [[i64; 3]; 4],
    entry: u32,
    polygons: &mut Vec<Polygon>,
    carriers: &mut Vec<Carrier>,
) -> Option<()> {
    let at = |p: [i64; 3]| P3::new(p[0], p[1], p[2]).unwrap();
    let mut planes = Vec::new();
    let mut flats = Vec::new();
    for i in 0..4 {
        let [mut a, b, mut c] = [(i + 1) % 4, (i + 2) % 4, (i + 3) % 4].map(|k| corner[k]);
        let mut plane = Plane::through(at(a), at(b), at(c)).ok()?;
        match plane.side(at(corner[i])).unwrap() {
            Sign::Zero => return None,
            Sign::Negative => {}
            Sign::Positive => {
                std::mem::swap(&mut a, &mut c);
                plane = Plane::through(at(a), at(b), at(c)).ok()?;
            }
        }
        let to = |p: [i64; 3]| [0, 1, 2].map(|k| (p[k] - a[k]) as f64);
        let surface = Flat::new(
            a.map(|v| v as f64),
            to(b),
            to(c),
            ((-1e9, 1e9), (-1e9, 1e9)),
        );
        flats.push(SurfaceKind::Plane(surface.ok()?));
        planes.push(plane);
    }
    let turned = |p: &Plane| Plane::new(p.n().map(|v| -v), -p.d()).unwrap();
    for i in 0..4 {
        let sides: Vec<Plane> = (1..4).map(|k| turned(&planes[(i + k) % 4])).collect();
        let mut pieces = convex_pieces(&planes[i], &[sides], &mut meter()).ok()?;
        polygons.push(Polygon {
            piece: pieces.pop()?,
            entry: Some(entry),
        });
        carriers.push(Carrier {
            surface: flats[i].clone(),
            sense: Sense::Forward,
        });
    }
    Some(())
}

fn tilted(solids: &[[[i64; 3]; 4]], test: &WindingTest, leaf: usize) -> Option<Body> {
    let (mut polygons, mut carriers) = (Vec::new(), Vec::new());
    for (entry, corners) in solids.iter().enumerate() {
        tetrahedron(*corners, entry as u32, &mut polygons, &mut carriers)?;
    }
    let kept = select(test, &classified(&polygons, leaf)).unwrap();
    Some(rebuild(&kept, &carriers, Merge::RemoveRedundant, &mut meter()).unwrap())
}

#[test]
fn tilted_solids_meet_where_three_planes_do() {
    let a = [[0, 0, 0], [90, 10, 0], [10, 80, 10], [20, 10, 70]];
    let b = [[10, 10, 10], [80, 30, 20], [30, 90, 40], [40, 20, 90]];
    let test = WindingTest::unite();
    let whole = tilted(&[a, b], &test, 1000).unwrap();
    assert!(
        (0..whole.arenas().points.len()).any(|at| matches!(
            whole.arenas().points.get(Ix::new(at).unwrap()),
            Some(PointKind::Meet(_))
        )),
        "no vertex off the grid"
    );
    checked(&whole);
    for leaf in [1, 2, 4, 16] {
        assert_eq!(tilted(&[a, b], &test, leaf).unwrap(), whole, "leaf {leaf}");
    }
    for test in [WindingTest::intersect(2), WindingTest::subtract(0)] {
        checked(&tilted(&[a, b], &test, 4).unwrap());
    }
}

fn any_block() -> impl Strategy<Value = Block> {
    ([-6i64..6, -6i64..6, -6i64..6], [1i64..6, 1i64..6, 1i64..6]).prop_map(|(low, size)| Block {
        low,
        high: [low[0] + size[0], low[1] + size[1], low[2] + size[2]],
    })
}

fn any_corners() -> impl Strategy<Value = [[i64; 3]; 4]> {
    let corner = || [-40i64..=40, -40i64..=40, -40i64..=40];
    [corner(), corner(), corner(), corner()]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn every_tilted_union_is_a_valid_body_and_the_boxes_do_not_show(
        solids in prop::collection::vec(any_corners(), 1..4),
        leaf in 1usize..10,
    ) {
        let test = WindingTest::unite();
        let once = tilted(&solids, &test, leaf);
        prop_assume!(once.is_some());
        let once = once.unwrap();
        prop_assert_eq!(&once, &tilted(&solids, &test, 1000).unwrap());
        checked(&once);
    }

    #[test]
    fn every_union_is_a_valid_body_and_the_boxes_do_not_show(
        blocks in prop::collection::vec(any_block(), 1..4),
        leaf in 1usize..10,
    ) {
        let test = WindingTest::unite();
        let once = body(&blocks, &test, leaf);
        prop_assert_eq!(&once, &body(&blocks, &test, 1000));
        checked(&once);
    }
}
