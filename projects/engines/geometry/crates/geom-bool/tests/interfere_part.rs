//! The interference benchmark of chapter 11.17, item 17 (G243 and G243a): a part of about 1,000
//! faces against one tool, the vertices, edges and faces of both met lowest dimension first, timed
//! and reported against chapter 23.
//!
//! The part is a row of 167 boxes (1,002 faces), 100 steps on a side at a pitch of 200; the tool is
//! one long box standing on the same floor and against the same front and back, so each box's front
//! and back bottom edges are collinear with the tool's bottom edges and every bottom corner lies on
//! one of them. The test checks what comes out (every corner a pave of the tool, every vertical edge
//! met at its own corner, two exact common blocks per box; every top corner inside the tool's front
//! or back face, each box's front, back and bottom on the tool's own planes, and no section edge,
//! since every face of the part meets the tool only along its own boundary) and prints the time.
//! Then every flat face is split along what it met (11.4 step 5, G244): the tool's front, back and
//! bottom are each cut round the 167 boxes standing on them, and each box's faces stay whole. The
//! splitting is timed apart, as face splits a second. Run it in release:
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
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "test code: a failure should stop loudly, and the timing is printed"
)]

use geom_bool::carrier::{CarrierIx, PointGeom};
use geom_bool::common::Proof;
use geom_bool::interfere::{Arrangement, EdgeIn, FaceIn, Interference, VertexIn, interfere};
use geom_bool::merge::default_remaining;
use geom_bool::split::split;
use geom_curve::{CurveKind, Line};
use geom_num::P3;
use geom_num::plane::Plane;
use geom_run::{Budget, Exec, WorkMeter};

/// Boxes in the part.
const BOXES: i64 = 167;
/// A box's side, in steps.
const SIDE: i64 = 100;
/// The distance from one box to the next, in steps.
const PITCH: i64 = 200;

/// Adds a box from `lo` to `hi` to `arr` as operand `operand`: its 8 corners, 12 edges and 6 faces,
/// the faces in the order x low, x high, y low, y high, z low, z high; returns the first face's
/// index.
fn add_box(arr: &mut Arrangement, lo: [i64; 3], hi: [i64; 3], operand: u32) -> usize {
    let first_edge = arr.edges.len();
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
    // The edges in the order made, by their two corners.
    let mut made = Vec::new();
    for k in 0..8_usize {
        for axis in 0..3 {
            if k >> axis & 1 == 0 {
                made.push((k, k | 1 << axis));
            }
        }
    }
    let edge = |a: usize, b: usize| {
        first_edge
            + made
                .iter()
                .position(|&(u, v)| (u, v) == (a.min(b), a.max(b)))
                .unwrap()
    };
    let first_face = arr.faces.len();
    for axis in 0..3 {
        let (b, c) = ((axis + 1) % 3, (axis + 2) % 3);
        for side in 0..2_usize {
            let ring: Vec<usize> = [(0, 0), (1, 0), (1, 1), (0, 1)]
                .iter()
                .map(|&(sb, sc)| side << axis | sb << b | sc << c)
                .collect();
            let loop_edges = (0..4).map(|k| edge(ring[k], ring[(k + 1) % 4])).collect();
            let mut n = [0_i128; 3];
            n[axis] = 1;
            let at = if side == 1 { hi[axis] } else { lo[axis] };
            let plane = arr
                .carriers
                .add_plane(&Plane::new(n, -i128::from(at)).unwrap())
                .unwrap();
            arr.faces.push(FaceIn {
                surface: CarrierIx::Plane(plane),
                loops: vec![loop_edges],
                operand,
            });
        }
    }
    first_face
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
    let tool_faces = add_box(&mut arr, [-50, 0, 0], [end, SIDE, 150], 1);
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
    // The faces: each top corner of the part inside the tool's front or back face (the bottom
    // corners lie on the tool's bottom edges, so on those faces' boundaries); each box's front, back
    // and bottom on the tool's own planes; and no section edge.
    for (face, name) in [(tool_faces + 2, "front"), (tool_faces + 3, "back")] {
        assert_eq!(
            out.faces[face].in_vertices.len(),
            2 * BOXES as usize,
            "the top corners in the tool's {name}"
        );
        assert_eq!(out.faces[face].same_carrier.len(), BOXES as usize);
    }
    assert_eq!(out.faces[tool_faces + 4].same_carrier.len(), BOXES as usize);
    assert_eq!(out.sections.len(), 0, "{:?}", out.sections.first());
    let inside = out
        .interferences
        .iter()
        .filter(|i| matches!(i, Interference::VertexFace { .. }))
        .count();
    println!(
        "{} faces, {} edges, {} vertices: {} common blocks, {} meetings at existing corners, {} vertices inside faces, in {:?}",
        6 * (BOXES + 1),
        arr.edges.len(),
        arr.vertices.len(),
        out.common.len(),
        reused,
        inside,
        took
    );
    // Face splitting: every face is flat and bounded by lines, so none waits.
    let start = std::time::Instant::now();
    let splitting = split(&arr, &out, &mut meter).unwrap();
    let split_took = start.elapsed();
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
    let pieces_of = |f: usize| splitting.faces.iter().filter(|s| s.face == f).count();
    // The front and back: each box's face and the rest round them. The bottom: each box's bottom
    // spans the tool's width, so the boxes' bottoms and the gaps between and beside them.
    let boxes = BOXES as usize;
    for (face, name, parts) in [
        (tool_faces + 2, "front", boxes + 1),
        (tool_faces + 3, "back", boxes + 1),
        (tool_faces + 4, "bottom", 2 * boxes + 1),
    ] {
        assert_eq!(pieces_of(face), parts, "the tool's {name}");
    }
    // Every other face stays whole.
    let faces = 6 * (BOXES + 1) as usize;
    assert_eq!(splitting.faces.len(), faces - 3 + 4 * boxes + 3);
    println!(
        "{faces} faces split into {} in {split_took:?}: {:.0} face splits a second",
        splitting.faces.len(),
        faces as f64 / split_took.as_secs_f64()
    );
}
