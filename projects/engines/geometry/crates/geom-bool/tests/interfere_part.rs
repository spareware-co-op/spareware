//! The interference benchmark of chapter 11.17, item 17 (G243): a part of about 1,000 faces against
//! one tool, the vertices and edges of both met lowest dimension first, timed and reported against
//! chapter 23.
//!
//! The part is a row of 167 boxes (1,002 faces), 100 steps on a side at a pitch of 200; the tool is
//! one long box standing on the same floor and against the same front and back, so each box's front
//! and back bottom edges are collinear with the tool's bottom edges and every bottom corner lies on
//! one of them. The test checks what comes out (every corner a pave of the tool, every vertical edge
//! met at its own corner, two exact common blocks per box) and prints the time; run it in release:
//! `cargo test --release -p geom-bool --test interfere_part -- --ignored --nocapture`.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::print_stdout,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    reason = "test code: a failure should stop loudly, and the timing is printed"
)]

use geom_bool::carrier::PointGeom;
use geom_bool::common::Proof;
use geom_bool::interfere::{Arrangement, EdgeIn, Interference, VertexIn, interfere};
use geom_bool::merge::default_remaining;
use geom_curve::{CurveKind, Line};
use geom_num::P3;
use geom_run::{Budget, Exec, WorkMeter};

/// Boxes in the part.
const BOXES: i64 = 167;
/// A box's side, in steps.
const SIDE: i64 = 100;
/// The distance from one box to the next, in steps.
const PITCH: i64 = 200;

/// Adds a box from `lo` to `hi` to `arr` as operand `operand`: its 8 corners and 12 edges.
fn add_box(arr: &mut Arrangement, lo: [i64; 3], hi: [i64; 3], operand: u32) {
    let first = arr.vertices.len();
    for k in 0..8_usize {
        let pick = |axis: usize| {
            if k >> axis & 1 == 1 {
                hi[axis]
            } else {
                lo[axis]
            }
        };
        arr.vertices.push(VertexIn {
            point: PointGeom::Grid(P3::new(pick(0), pick(1), pick(2)).unwrap()),
            operand,
        });
    }
    // Each edge joins two corners that differ in one axis bit.
    for k in 0..8_usize {
        for axis in 0..3 {
            if k >> axis & 1 == 1 {
                continue;
            }
            let other = k | 1 << axis;
            let point = |i: usize| match arr.vertices[first + i].point {
                PointGeom::Grid(p) => [p.x.get() as f64, p.y.get() as f64, p.z.get() as f64],
                _ => panic!("a box corner is a grid point"),
            };
            let (a, b) = (point(k), point(other));
            let line = Line::new(a, [b[0] - a[0], b[1] - a[1], b[2] - a[2]], (0.0, 1.0)).unwrap();
            let curve = arr.carriers.add_curve(CurveKind::Line(line)).unwrap();
            arr.edges.push(EdgeIn {
                curve,
                ends: Some([first + k, first + other]),
                operand,
            });
        }
    }
}

#[test]
#[ignore = "a timing run of a 1,000-face part: run it in release, as the module documentation says"]
fn a_thousand_face_part_against_one_tool() {
    let mut arr = Arrangement::default();
    for i in 0..BOXES {
        let x = i * PITCH;
        add_box(&mut arr, [x, 0, 0], [x + SIDE, SIDE, SIDE], 0);
    }
    let end = (BOXES - 1) * PITCH + SIDE + 50;
    add_box(&mut arr, [-50, 0, 0], [end, SIDE, 150], 1);
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let start = std::time::Instant::now();
    let out = interfere(&arr, default_remaining(), &Exec::serial(), &mut meter).unwrap();
    let took = start.elapsed();
    // No vertex merges and none is made: every meeting is at a corner of the part.
    assert_eq!(out.vertices.len(), arr.vertices.len());
    let on_tool = out
        .interferences
        .iter()
        .filter(|i| matches!(i, Interference::VertexEdge { .. }))
        .count();
    assert_eq!(
        on_tool,
        4 * BOXES as usize,
        "every bottom front and back corner"
    );
    let reused = out
        .interferences
        .iter()
        .filter(|i| matches!(i, Interference::EdgeEdge { reused: true, .. }))
        .count();
    let made = out
        .interferences
        .iter()
        .filter(|i| matches!(i, Interference::EdgeEdge { reused: false, .. }))
        .count();
    assert_eq!(made, 0);
    assert!(reused >= 8 * BOXES as usize, "{reused}");
    assert_eq!(out.common.len(), 2 * BOXES as usize);
    assert!(out.common.iter().all(|c| c.proof == Proof::Exact));
    println!(
        "{} faces, {} edges, {} vertices: {} common blocks, {} meetings at existing corners, in {:?}",
        6 * (BOXES + 1),
        arr.edges.len(),
        arr.vertices.len(),
        out.common.len(),
        reused,
        took
    );
}
