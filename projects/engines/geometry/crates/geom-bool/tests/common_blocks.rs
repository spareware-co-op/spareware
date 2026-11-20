//! Common blocks by the contact rule across kinds (G243, chapter 11.4 step 3): straight edges of
//! different operands between the same two vertices, one along the x axis and the others along
//! lines off the grid a little above it, so the exact test is not asked. Each block is cut into
//! 2^k pieces; each piece's ball (its box's grid-aligned centre and the radius holding the box) is
//! measured from the other line, and at the finest refinement (k = 8) a piece is apart when the
//! line lies further than its radius, and touching when it lies within the contact radius of
//! every point of the ball. Then a closed circle and its NURBS copy, cut into halves that share
//! both their vertices, among them halves that run round the seam.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_bool::carrier::PointGeom;
use geom_bool::common::{DEEPEST, Proof, common_blocks};
use geom_bool::interfere::{Arrangement, BuildError, EdgeIn, Interfered, VertexIn, interfere};
use geom_bool::merge::default_remaining;
use geom_bool::pave::{Pave, PaveBlock};
use geom_bool::rule::{ContactError, NearContact};
use geom_curve::{Circle, CurveKind, Frame, Line, ParamCurve};
use geom_num::P3;
use geom_num::interval::Interval;
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

/// A closed circle of radius 1105 about the origin, starting from (1105, 0), and its NURBS copy,
/// of two operands, each cut by a third operand's vertices at `cut`.
fn closed_circle_and_copy(cut: [[i64; 2]; 2]) -> Interfered {
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let circle = Circle::new([0.0, 0.0, 0.0], 1105.0, frame, (0.0, 2.0)).unwrap();
    let copy = CurveKind::Circle(circle.clone())
        .to_nurbs(&mut meter())
        .unwrap()
        .value
        .curve;
    let mut arr = Arrangement::default();
    for (operand, curve) in [(0, CurveKind::Circle(circle)), (1, CurveKind::Nurbs(copy))] {
        let curve = arr.carriers.add_curve(curve).unwrap();
        arr.edges.push(EdgeIn {
            curve,
            ends: None,
            operand,
        });
    }
    for [x, y] in cut {
        arr.vertices.push(VertexIn {
            point: PointGeom::Grid(P3::new(x, y, 0).unwrap()),
            operand: 2,
        });
    }
    interfere(&arr, default_remaining(), &Exec::serial(), &mut meter()).unwrap()
}

/// The circle and its copy cut at (1105, 0) and (−1105, 0): each is two halves between the same
/// two vertices, the lower half running from (−1105, 0) round to the seam at (1105, 0), where the
/// vertex stands at the start of both domains. Each half is one block with the same half of the
/// other curve, and not with the other half, whose nearest points to it lie on the curve but off
/// that half: two blocks of two, the upper halves (pieces 0 and 2) and the lower (1 and 3).
#[test]
fn a_closed_circle_and_its_copy_cut_across_are_two_blocks() {
    let out = closed_circle_and_copy([[1105, 0], [-1105, 0]]);
    let members: Vec<Vec<usize>> = out.common.iter().map(|c| c.members.clone()).collect();
    assert_eq!(members, vec![vec![0, 2], vec![1, 3]], "{:?}", out.common);
}

/// The same cut at (0, 1105) and (0, −1105): each right half runs round its curve's seam, from
/// the last vertex on to the domain's end and from the start to the first, and is one block with
/// the other right half, as the left halves are.
#[test]
fn halves_that_run_round_the_seam_are_one_block() {
    let out = closed_circle_and_copy([[0, 1105], [0, -1105]]);
    let members: Vec<Vec<usize>> = out.common.iter().map(|c| c.members.clone()).collect();
    assert_eq!(members, vec![vec![0, 2], vec![1, 3]], "{:?}", out.common);
    for piece in [1, 3] {
        assert!(out.splits[piece].block.wraps, "{:?}", out.splits[piece]);
    }
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

/// A closed circle of radius 1105 from (1105, 0) and a NURBS copy of its first quarter, to
/// (0, 1105), of another operand, each one block between the vertices at the quarter's ends. The
/// circle's block runs round the seam from the seam vertex's pave, whose enclosure is written
/// from a little before the end and runs past it (decision 32.104). Inside its paves the block is
/// one stretch, from where that pave ends modulo the turn to the quarter's pave, and the contact
/// rule cuts only that stretch; a seam pave ending exactly on the end leaves nothing before the
/// join, and the stretch from the domain's start. Both are one block with the copy, at the work
/// the stretches take.
#[test]
fn a_block_from_a_seam_pave_past_the_end_is_one_stretch_inside_its_paves() {
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let circle = Circle::new([0.0, 0.0, 0.0], 1105.0, frame, (0.0, 2.0)).unwrap();
    let quarter = Circle::new([0.0, 0.0, 0.0], 1105.0, frame, (0.0, 0.5)).unwrap();
    let copy = CurveKind::Circle(quarter)
        .to_nurbs(&mut meter())
        .unwrap()
        .value
        .curve;
    let (s0, s1) = copy.domain();
    let mut arr = Arrangement::default();
    for (operand, curve) in [(0, CurveKind::Circle(circle)), (1, CurveKind::Nurbs(copy))] {
        let curve = arr.carriers.add_curve(curve).unwrap();
        arr.edges.push(EdgeIn {
            curve,
            ends: None,
            operand,
        });
    }
    for [x, y] in [[1105, 0], [0, 1105]] {
        arr.vertices.push(VertexIn {
            point: PointGeom::Grid(P3::new(x, y, 0).unwrap()),
            operand: 2,
        });
    }
    let iv = |lo: f64, hi: f64| Interval::new(lo, hi).unwrap();
    let pave = |vertex: usize, t: Interval| Some(Pave { vertex, t });
    let common = |seam_end: f64| {
        let blocks = [
            PaveBlock {
                edge: 0,
                from: pave(0, iv(2.0 - 1.0 / 64.0, seam_end)),
                to: pave(1, iv(0.5, 0.5)),
                wraps: true,
            },
            PaveBlock {
                edge: 1,
                from: pave(0, iv(s0, s0)),
                to: pave(1, iv(s1, s1)),
                wraps: false,
            },
        ];
        let mut m = meter();
        let found = common_blocks(&arr, &blocks, &mut m).unwrap();
        let members: Vec<Vec<usize>> = found.iter().map(|c| c.members.clone()).collect();
        (members, m.used().work)
    };
    // Two savings that add: the one stretch from where the seam pave ends modulo the turn
    // (decision 32.104) saves 443,095 where the pave runs past the end, and a circle's foot by its
    // closed form (project_curve_over, issue 1102) saves 66,955 at both.
    assert_eq!(
        [common(2.0 + 1.0 / 64.0), common(2.0)],
        [(vec![vec![0, 1]], 465_618), (vec![vec![0, 1]], 908_713)]
    );
}

/// A line of half a step from (0.25, 0.25) to (−0.25, 0.25), and a NURBS copy of the half circle
/// of 8 steps about the origin from (8, 0) to (−8, 0), of two operands, each one block between the
/// same two vertices (given by hand: the contact rule reads only the blocks' curves and paves).
/// Every piece of the line lies within a quarter step of the origin on each axis, so its ball is
/// centred on the origin, from which every point of the copy is as far: the search on the copy is
/// left undecided (`NonIsolated`) at every cut, and the first piece, at (0.25, 0.25), reaches the
/// deepest cut with no decided distance. It fails with `NearContact`, and its enclosure is finite
/// and true: from zero to the largest distance between the piece's box and the copy's, from the
/// piece to the copy's box's corner at (−8, 8), a little over 11.319 steps (8.25 across, the low
/// side's gap, and 7.75 up, the high side's), holding the piece's distance from the copy, about
/// 7.65 steps. Not `NotFinite`, which names no number that failed.
#[test]
fn a_piece_with_no_decided_distance_at_the_deepest_cut_is_a_finite_near_contact() {
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let half = Circle::new([0.0, 0.0, 0.0], 8.0, frame, (0.0, 1.0)).unwrap();
    let copy = CurveKind::Circle(half)
        .to_nurbs(&mut meter())
        .unwrap()
        .value
        .curve;
    let (s0, s1) = copy.domain();
    let line = Line::new([0.25, 0.25, 0.0], [-0.5, 0.0, 0.0], (0.0, 1.0)).unwrap();
    let mut arr = Arrangement::default();
    for (operand, curve) in [(0, CurveKind::Line(line)), (1, CurveKind::Nurbs(copy))] {
        let curve = arr.carriers.add_curve(curve).unwrap();
        arr.edges.push(EdgeIn {
            curve,
            ends: None,
            operand,
        });
    }
    for x in [8, -8] {
        arr.vertices.push(VertexIn {
            point: PointGeom::Grid(P3::new(x, 0, 0).unwrap()),
            operand: 2,
        });
    }
    let at = |t: f64| Interval::new(t, t).unwrap();
    let block = |edge: usize, (t0, t1): (f64, f64)| PaveBlock {
        edge,
        from: Some(Pave {
            vertex: 0,
            t: at(t0),
        }),
        to: Some(Pave {
            vertex: 1,
            t: at(t1),
        }),
        wraps: false,
    };
    let blocks = [block(0, (0.0, 1.0)), block(1, (s0, s1))];
    let found = common_blocks(&arr, &blocks, &mut meter());
    let Err(BuildError::Contact(ContactError::NearContact(NearContact {
        enclosure,
        refinement,
    }))) = found
    else {
        panic!("{found:?}");
    };
    assert_eq!(refinement, DEEPEST);
    assert!(
        enclosure.lo() <= 7.6 && 7.7 <= enclosure.hi(),
        "{enclosure:?}"
    );
    assert!(
        11.319 < enclosure.hi() && enclosure.hi() < 11.32,
        "{enclosure:?}"
    );
}
