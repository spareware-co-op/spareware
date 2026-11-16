//! Which shells face out, and which shell holds each cavity, wherever the shells lie: around the
//! origin, and far from it on either side. The merge decides each by a point in front of a shell,
//! traced inside a box that is a step and a half clear of the shell's corners (its grid range).
//! A box that did not hold them, such as one fixed about the origin, can lie inside the shell, and
//! the count then no longer says which way the shell faces: with such a box the block and cavity
//! about the origin are judged wrongly (G129, 11.3).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests: native arithmetic and short indexing, and a failure stops the test"
)]

use geom_num::plane::Plane;
use geom_poly::convex::Piece;
use geom_poly::merge::{Merge, Merged, merge, select};
use geom_poly::subdivide::{LEAF_POLYGONS, around, arrange};
use geom_poly::trace::Polygon;
use geom_poly::winding::{Winding, WindingTest};
use geom_run::{Budget, Exec, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The plane `x_axis = at`, facing up the axis or down it.
fn wall(axis: usize, at: i64, upward: bool) -> Plane {
    let mut n = [0i128; 3];
    n[axis] = if upward { 1 } else { -1 };
    let d = if upward {
        -i128::from(at)
    } else {
        i128::from(at)
    };
    Plane::new(n, d).unwrap()
}

/// A box from `low` to `high`.
#[derive(Clone, Copy)]
struct Block {
    low: [i64; 3],
    high: [i64; 3],
}

/// The side of `b` on `axis` at `at`, facing out of it, bounded by its four neighbours.
fn side(b: &Block, axis: usize, at: i64, upward: bool) -> Piece {
    let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
    let (ul, uh) = (b.low[u], b.high[u]);
    let (vl, vh) = (b.low[v], b.high[v]);
    let bounds = if upward {
        vec![
            wall(v, vl, true),
            wall(u, uh, false),
            wall(v, vh, false),
            wall(u, ul, true),
        ]
    } else {
        vec![
            wall(u, ul, true),
            wall(v, vh, false),
            wall(u, uh, false),
            wall(v, vl, true),
        ]
    };
    Piece {
        face: wall(axis, at, upward),
        bounds,
    }
}

/// The six sides of each block, each block its own entry.
fn scene(blocks: &[Block]) -> Vec<Polygon> {
    let mut polygons = Vec::new();
    for (entry, b) in (0u32..).zip(blocks) {
        for axis in 0..3 {
            for (at, upward) in [(b.high[axis], true), (b.low[axis], false)] {
                polygons.push(Polygon {
                    piece: side(b, axis, at, upward),
                    entry: Some(entry),
                });
            }
        }
    }
    polygons
}

/// The blocks combined by `test` and merged into shells.
fn merged(blocks: &[Block], test: &WindingTest) -> Merged {
    merged_polygons(&scene(blocks), test, LEAF_POLYGONS)
}

/// `polygons` combined by `test`, arranged with at most `leaf` polygons to a leaf, and merged
/// into shells.
fn merged_polygons(polygons: &[Polygon], test: &WindingTest, leaf: usize) -> Merged {
    let cell = around(polygons).unwrap();
    let classified = arrange(
        &Exec::serial(),
        cell,
        Winding::new(),
        polygons,
        leaf,
        &[],
        &mut meter(),
    )
    .unwrap();
    let pieces = select(test, &classified).unwrap();
    merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap()
}

/// A cube of half side `half` about `centre`.
fn cube(centre: [i64; 3], half: i64) -> Block {
    Block {
        low: centre.map(|c| c - half),
        high: centre.map(|c| c + half),
    }
}

/// Each shell's (faces out, holder).
fn nesting(m: &Merged) -> Vec<(bool, Option<usize>)> {
    m.shells.iter().map(|s| (s.outer, s.holder)).collect()
}

/// A block with a cavity in it, about `centre`: one shell faces out, and the other is a cavity
/// held by it.
fn a_cavity_is_held(centre: [i64; 3]) {
    let m = merged(
        &[cube(centre, 8), cube(centre, 4)],
        &WindingTest::subtract(0),
    );
    assert_eq!(m.shells.len(), 2, "{centre:?}");
    let shells = nesting(&m);
    let outer: Vec<usize> = (0..2).filter(|s| shells[*s].0).collect();
    assert_eq!(outer.len(), 1, "{centre:?}: {shells:?}");
    let (out, inner) = (outer[0], 1 - outer[0]);
    assert_eq!(shells[out], (true, None), "{centre:?}");
    assert_eq!(shells[inner], (false, Some(out)), "{centre:?}");
}

#[test]
fn a_cavity_about_the_origin_is_held_by_its_block() {
    a_cavity_is_held([0, 0, 0]);
}

#[test]
fn a_cavity_far_from_the_origin_is_held_by_its_block_on_either_side() {
    for centre in [[100, 100, 100], [-100, -100, -100], [-100, 37, 1000]] {
        a_cavity_is_held(centre);
    }
}
