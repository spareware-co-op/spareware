//! The recursion statements of `geom-poly`, run at their bounds (the recursion check, piece G13).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests: native arithmetic and indexing, and a failure stops the test"
)]

use geom_num::grid::COORD_MAX;
use geom_num::plane::Plane;
use geom_poly::convex::Piece;
use geom_poly::subdivide::{around, arrange_reusing};
use geom_poly::trace::Polygon;
use geom_poly::winding::Winding;
use geom_run::{Budget, Exec, WorkMeter};

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

/// The six faces of the block from `low` to `high`, each facing out, as operand `entry`.
fn block(low: [i64; 3], high: [i64; 3], entry: u32, out: &mut Vec<Polygon>) {
    for axis in 0..3 {
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        let rim = |flip: bool| {
            let mut bounds = vec![
                up(v, low[v]),
                down(u, high[u]),
                down(v, high[v]),
                up(u, low[u]),
            ];
            if flip {
                bounds = vec![
                    up(u, low[u]),
                    down(v, high[v]),
                    down(u, high[u]),
                    up(v, low[v]),
                ];
            }
            bounds
        };
        out.push(Polygon {
            piece: Piece {
                face: up(axis, high[axis]),
                bounds: rim(false),
            },
            entry: Some(entry),
        });
        out.push(Polygon {
            piece: Piece {
                face: down(axis, low[axis]),
                bounds: rim(true),
            },
            entry: Some(entry),
        });
    }
}

#[test]
fn the_kept_tree_clones_compares_prints_and_drops_a_hundred_levels_deep() {
    // Two one-step cubes at opposite corners of the frame: the box around them spans the whole
    // frame, and with one polygon to a leaf each corner's box splits until no odd half step lies
    // inside it, along all three axes, about 42 times each, so the tree nests over a hundred
    // levels deep on each side, near the statement's bound of 127.
    let m = COORD_MAX;
    let mut list = Vec::new();
    block([m - 1; 3], [m; 3], 0, &mut list);
    block([-m; 3], [1 - m; 3], 1, &mut list);
    let cell = around(&list).unwrap();
    let meter = &mut WorkMeter::new(Budget::UNLIMITED);
    let exec = Exec::serial();
    let (_, solved) =
        arrange_reusing(&exec, cell, Winding::new(), &list, 1, &[], None, meter).unwrap();
    let copy = solved.clone();
    assert_eq!(copy, solved);
    // A second arrangement of the same polygons builds a tree of its own, box for box the same,
    // so comparing the two walks every level of both.
    let (_, again) =
        arrange_reusing(&exec, cell, Winding::new(), &list, 1, &[], None, meter).unwrap();
    assert_eq!(again, solved);
    assert!(format!("{copy:?}").contains("Split"));
    // Every division of the copy is the original's, so this counts the tree's boxes: two for each
    // split along the paths to the two corners, and the root.
    let boxes = solved.divisions_shared_with(&copy);
    assert!(boxes > 4 * 120, "{boxes} boxes");
    drop(copy);
}
