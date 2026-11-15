//! Common blocks by the contact rule across kinds (G243, chapter 11.4 step 3): straight edges of
//! different operands between the same two vertices, one along the x axis and the others along
//! lines off the grid a little above it, so the exact test is not asked. Each block is cut into
//! 2^k pieces; each piece's ball (its box's grid-aligned centre and the radius holding the box) is
//! measured from the other line, and at the finest refinement (k = 8) a piece is apart when the
//! line lies further than its radius, and touching when it lies within the contact radius of
//! every point of the ball.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_bool::carrier::PointGeom;
use geom_bool::common::Proof;
use geom_bool::interfere::{Arrangement, EdgeIn, Interfered, VertexIn, interfere};
use geom_bool::merge::default_remaining;
use geom_curve::{CurveKind, Line};
use geom_num::P3;
use geom_run::{Budget, Exec, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 30))
}

/// The edge from (0, 0, 0) to (`length`, 0, 0) of operand 0, and for each of `heights` an edge of
/// its own operand between the same two points running along y = height: all their vertices on
/// the axis, their lines above it.
fn above_the_axis(length: i64, heights: &[f64]) -> Interfered {
    let mut arr = Arrangement::default();
    #[allow(
        clippy::cast_precision_loss,
        reason = "the lengths are a few thousand steps"
    )]
    let reach = length as f64;
    for (operand, y) in core::iter::once(0.0)
        .chain(heights.iter().copied())
        .enumerate()
    {
        let operand = u32::try_from(operand).unwrap();
        let mut end = |x: i64| {
            arr.vertices.push(VertexIn {
                point: PointGeom::Grid(P3::new(x, 0, 0).unwrap()),
                operand,
            });
            arr.vertices.len() - 1
        };
        let ends = [end(0), end(length)];
        let line = Line::new([0.0, y, 0.0], [reach, 0.0, 0.0], (0.0, 1.0)).unwrap();
        let curve = arr.carriers.add_curve(CurveKind::Line(line)).unwrap();
        arr.edges.push(EdgeIn {
            curve,
            ends: Some(ends),
            operand,
        });
    }
    interfere(&arr, default_remaining(), &Exec::serial(), &mut meter()).unwrap()
}

/// The widest enclosure a common block's proof holds.
fn proof_of(out: &Interfered) -> (usize, f64) {
    assert_eq!(out.common.len(), 1, "{:?}", out.common);
    let Proof::Contact(d) = out.common[0].proof else {
        panic!("{:?}", out.common[0].proof);
    };
    (out.common[0].members.len(), d.hi())
}

/// A line 20.5 steps above an edge 10,496 steps long: at the finest refinement each piece is 41
/// steps long, its ball's centre the grid point a half step to one side of its middle, so the
/// ball reaches 21 steps one way and 20 the other, and its radius is the larger. The line lies
/// within 21 steps of every piece, so none is proven apart; and within 41.5 steps of every point
/// of every ball, below the contact radius of 64, so the two are one block, its proof reaching
/// 41.5. A radius taken from the nearer side, or one that loses the side the centre stands from,
/// proves some piece apart; an enclosure whose upper end is the distance times the radius, or the
/// distance less it, holds no zero within the contact radius and is cut on until a piece is
/// apart.
#[test]
fn a_line_within_each_piece_s_ball_is_one_block_by_the_contact_rule() {
    let out = above_the_axis(41 * 256, &[20.5]);
    let (members, widest) = proof_of(&out);
    assert_eq!(members, 2);
    assert!(41.4 < widest && widest < 41.6, "{widest}");
}

/// A line 2.5 steps above an edge 100 steps long: within the contact radius of the edge's ball
/// while the pieces are long, but proven apart once they are short, before the finest refinement.
/// No piece is taken as touching before that refinement, so there is no block.
#[test]
fn a_line_proven_apart_at_a_finer_cut_is_no_block() {
    let out = above_the_axis(100, &[2.5]);
    assert!(out.common.is_empty(), "{:?}", out.common);
}

/// A line a quarter step above an edge 1,000 steps long: pieces of 3.90625 steps, whose balls
/// reach from 1.95 to 2.44 steps along the axis by where the grid point nearest each middle
/// falls. Seen from the edge, the farthest point is a quarter step more than the widest ball,
/// about 2.69 steps; seen from the line, whose balls' centres round down to the axis, the farthest
/// is the widest ball itself, about 2.45. The block's proof is the widest enclosure of all,
/// 2.69: not the first piece's or the last's (2.25), the narrowest (2.22), nor the narrower
/// side's (2.45).
#[test]
fn a_block_s_proof_is_its_widest_enclosure_both_ways() {
    let out = above_the_axis(1000, &[0.25]);
    let (members, widest) = proof_of(&out);
    assert_eq!(members, 2);
    assert!(2.6 < widest && widest < 2.8, "{widest}");
}

/// Three operands' edges between the same two points, on the axis, a quarter step above it and a
/// half step above it: one block of three, whose proof is the weaker of its pairs', the half
/// step's, about 2.94 (the quarter step's is about 2.69).
#[test]
fn a_block_of_three_holds_the_weakest_proof_of_its_pairs() {
    let out = above_the_axis(1000, &[0.25, 0.5]);
    let (members, widest) = proof_of(&out);
    assert_eq!(members, 3);
    assert!(2.85 < widest && widest < 3.0, "{widest}");
}
