//! The narrowest passage between two seeds, walked over the blocks of a room whose corners each
//! test chooses (G137, 11.7).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "tests: native arithmetic, and a failure stops the test"
)]

use geom_bool::passage::{Passage, narrowest};
use geom_num::P3;
use geom_num::plane::Plane;
use geom_poly::convex::Piece;
use geom_run::{Budget, WorkMeter};

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
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

/// The six faces of the room from `low` to `high`, each facing out.
fn room(low: [i64; 3], high: [i64; 3]) -> Vec<Piece> {
    let mut out = Vec::new();
    for axis in 0..3 {
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        let (ul, uh, vl, vh) = (low[u], high[u], low[v], high[v]);
        out.push(Piece {
            face: up(axis, high[axis]),
            bounds: vec![up(v, vl), down(u, uh), down(v, vh), up(u, ul)],
        });
        out.push(Piece {
            face: down(axis, low[axis]),
            bounds: vec![up(u, ul), down(v, vh), down(u, uh), up(v, vl)],
        });
    }
    out
}

#[test]
fn a_block_reached_first_through_a_sliver_takes_the_wider_way_that_follows() {
    // Corners at y 51 and 53, as a column's would be, make the second seed's block a sliver one
    // step high. The walk reaches it first from the sliver beside it, through a portal one step
    // wide, and only later from the block above it, by a way 15 steps wide all along (the seeds'
    // height halves the room): the wider way replaces the first, its narrowest point its first
    // portal.
    let walls = room([0, 0, 0], [100, 100, 30]);
    let corners = [p(0, 0, 0), p(100, 100, 30), p(2, 51, 0), p(4, 53, 0)];
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let got = narrowest(&walls, &corners, p(80, 20, 15), p(60, 50, 15), &mut meter).unwrap();
    assert_eq!(
        got,
        Some(Passage {
            width: 15,
            at: [80, 50, 15],
        })
    );
}
