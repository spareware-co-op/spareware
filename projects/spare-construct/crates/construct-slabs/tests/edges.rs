//! Slab edges (job 5.2): a downturn or a thickened edge run along chosen edges of a floor, each a
//! piece with its plan at the underside and at its foot, mitred where two meet, with an exact
//! length and volume; its stored form; and every refusal by name.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{at, id, mm, rectangle, slab};
use construct_params::{Length, Volume};
use construct_slabs::{
    Boundary, Corner, EDGES_ARG, EdgeProfile, SHAPED_VERSION, Shape, SlabEdge, SlabError, filled,
    shape_of, shape_quantities, shape_read_only, shaped_recipe,
};
use construct_solver::Point;
use serde_json::json;

fn rect(w: i32, d: i32) -> Boundary {
    Boundary::from_corners(&rectangle("e", 0, 0, w, d), &[]).unwrap()
}

fn downturn(edge: &str, width: i64, depth: i64) -> SlabEdge {
    SlabEdge {
        edge: id(edge),
        profile: EdgeProfile::Downturn {
            width: mm(width),
            depth: mm(depth),
        },
    }
}

fn thickened(edge: &str, width: i64, foot: i64, depth: i64) -> SlabEdge {
    SlabEdge {
        edge: id(edge),
        profile: EdgeProfile::Thickened {
            width: mm(width),
            foot: mm(foot),
            depth: mm(depth),
        },
    }
}

/// A 300 mm by 400 mm downturn along one 4 m side: square at both ends (both corners are right
/// angles), 4 m long and 4000 x 300 x 400 mm3; the floor's lowest point is its foot.
#[test]
fn a_downturn_along_one_side() {
    let shape = Shape::new(rect(4000, 3000), None, vec![downturn("e0", 300, 400)]).unwrap();
    let piece = &shape.pieces()[0];
    assert_eq!(
        piece.top,
        [
            Point::mm(0, 0),
            Point::mm(4000, 0),
            Point::mm(4000, 300),
            Point::mm(0, 300)
        ]
    );
    assert_eq!(piece.foot, piece.top);
    assert_eq!(piece.length, Length::from_mm(4000));
    assert_eq!(piece.volume, Volume::from_mm3(4000 * 300 * 400));
    let q = shape_quantities(&shape, &slab()).unwrap();
    assert_eq!(q.lowest, Length::from_mm(-600));
    assert_eq!(q.highest, Length::from_mm(0));
    let rows = shape_read_only(&q).unwrap();
    let ids: Vec<String> = rows.iter().map(|(d, _)| d.id().to_string()).collect();
    assert_eq!(ids, ["floor.edge-e0.length", "floor.edge-e0.volume"]);
}

/// A thickened edge all round, 400 mm wide at the underside tapering to 200 mm at its foot 300 mm
/// down: each corner mitred on its bisector; the long sides 332,000,000 mm3 each and the short
/// 242,000,000 (the prismatoid rule), 1.148 m3 in all, which is the section's 90,000 mm2 swept
/// round the line through its centroid (Pappus).
#[test]
fn a_thickened_edge_all_round_is_mitred() {
    let edges = ["e0", "e1", "e2", "e3"].map(|e| thickened(e, 400, 200, 300));
    let shape = Shape::new(rect(4000, 3000), None, edges.to_vec()).unwrap();
    let pieces = shape.pieces();
    assert_eq!(
        pieces[0].top,
        [
            Point::mm(0, 0),
            Point::mm(4000, 0),
            Point::mm(3600, 400),
            Point::mm(400, 400)
        ]
    );
    assert_eq!(
        pieces[0].foot,
        [
            Point::mm(0, 0),
            Point::mm(4000, 0),
            Point::mm(3800, 200),
            Point::mm(200, 200)
        ]
    );
    let volumes: Vec<Volume> = pieces.iter().map(|p| p.volume).collect();
    assert_eq!(
        volumes,
        [332_000_000, 242_000_000, 332_000_000, 242_000_000].map(Volume::from_mm3)
    );
    let lengths: Vec<Length> = pieces.iter().map(|p| p.length).collect();
    assert_eq!(lengths, [4000, 3000, 4000, 3000].map(Length::from_mm));
    let total: i128 = volumes.iter().map(|v| v.cubic_steps()).sum();
    assert_eq!(
        Volume::from_cubic_steps(total),
        Volume::from_mm3(1_148_000_000)
    );
}

/// An L-shaped floor with a downturn on both sides of its inside corner: the two pieces mitre on
/// the reflex corner, reaching past it to join; a hole's edge carries one too, into the floor.
#[test]
fn a_reflex_corner_and_a_hole_edge() {
    let floor = common::l_with_hole();
    let edges = vec![
        downturn("e2", 200, 300),
        downturn("e3", 200, 300),
        downturn("h0", 100, 300),
    ];
    let shape = Shape::new(floor, None, edges).unwrap();
    let pieces = shape.pieces();
    // e2 runs from (8, 4) m to (4, 4) m, the floor below it; e3 from (4, 4) m up to (4, 8) m.
    assert_eq!(pieces[0].top[2], Point::mm(3800, 3800));
    assert_eq!(pieces[1].top[3], Point::mm(3800, 3800));
    // A trapezoid in plan, 4 m along the edge and 4.2 m along its inner face, 200 mm across.
    assert_eq!(pieces[0].volume, Volume::from_mm3(4100 * 200 * 300));
    assert_eq!(pieces[2].length, Length::from_mm(2000));
    assert_eq!(pieces[2].volume, Volume::from_mm3(2000 * 100 * 300));
}

/// The stored form: version 2 with the edges in order, reading back the same; the builder's
/// recipe holds each piece's plan at the underside and at its foot in grid steps; a new boundary
/// keeps the edges, and one without an edge's ID is refused.
#[test]
fn slab_edges_are_stored_and_filled() {
    let shape = Shape::new(
        rect(4000, 3000),
        None,
        vec![thickened("e0", 400, 200, 300), downturn("e2", 300, 400)],
    )
    .unwrap();
    let recipe = shaped_recipe(&shape);
    assert_eq!(recipe.op_version, SHAPED_VERSION);
    assert_eq!(
        recipe.args["edges"][0],
        json!({
            "depth": "length:q0:1920000",
            "edge": "e0",
            "foot": "length:q0:1280000",
            "kind": "thickened",
            "width": "length:q0:2560000",
        })
    );
    assert!(recipe.args["edges"][1].get("foot").is_none());
    assert_eq!(shape_of(&recipe).unwrap(), shape);
    let built = filled(&recipe, &slab()).unwrap();
    let under = Length::from_mm(-200).steps();
    let foot = Length::from_mm(-500).steps();
    let s = |v: i32| Length::from_mm(v).steps();
    assert_eq!(
        built.args[EDGES_ARG][0],
        json!({
            "id": "e0",
            "kind": "thickened",
            "top": [[0, 0, under], [s(4000), 0, under], [s(4000), s(400), under], [0, s(400), under]],
            "foot": [[0, 0, foot], [s(4000), 0, foot], [s(4000), s(200), foot], [0, s(200), foot]],
        })
    );
    let mut empty = recipe.clone();
    empty.args.insert("edges".to_owned(), json!([]));
    assert!(matches!(shape_of(&empty), Err(SlabError::Malformed(_))));

    let record = construct_slabs::floor_record(
        &id("F-1"),
        &id("DOC-1"),
        "Deck",
        &id("floor-slab"),
        &construct_levels::Tie {
            level: id("L-1"),
            offset: Length::from_mm(0),
        },
        &rect(4000, 3000),
    );
    let edged = construct_slabs::with_edges(record, shape.edges().to_vec()).unwrap();
    let deeper = construct_slabs::with_boundary(edged.clone(), &rect(4000, 5000)).unwrap();
    assert_eq!(shape_of(&deeper.recipe).unwrap().edges(), shape.edges());
    let renamed = Boundary::from_corners(&rectangle("f", 0, 0, 4000, 3000), &[]).unwrap();
    assert_eq!(
        construct_slabs::with_boundary(edged, &renamed),
        Err(SlabError::EdgeUnknown { edge: id("e0") })
    );
}

/// The refusals of a slab edge as given, by name: an edge the floor lacks; two on one edge; a
/// profile of no width, a foot wider than its width; an edge whose length is a root; two
/// different profiles meeting; a square end at an acute corner.
#[test]
fn what_a_slab_edge_cannot_be_is_refused() {
    let r = || rect(4000, 3000);
    let refused = |b: Boundary, e: Vec<SlabEdge>| Shape::new(b, None, e).unwrap_err();
    assert_eq!(
        refused(r(), vec![downturn("zz", 300, 400)]),
        SlabError::EdgeUnknown { edge: id("zz") }
    );
    assert_eq!(
        refused(
            r(),
            vec![downturn("e0", 300, 400), downturn("e0", 300, 400)]
        ),
        SlabError::EdgeTwice { edge: id("e0") }
    );
    assert!(matches!(
        refused(r(), vec![downturn("e0", 0, 400)]),
        SlabError::EdgeProfile { .. }
    ));
    assert!(matches!(
        refused(r(), vec![thickened("e0", 200, 300, 400)]),
        SlabError::EdgeProfile { .. }
    ));
    // A triangle whose long side is the root of 17 m long.
    let leaning =
        Boundary::from_corners(&[at("a", 0, 0), at("b", 4000, 0), at("c", 0, 1000)], &[]).unwrap();
    assert_eq!(
        refused(leaning, vec![downturn("b", 100, 100)]),
        SlabError::EdgeOffGrid { edge: id("b") }
    );
    assert_eq!(
        refused(
            r(),
            vec![downturn("e0", 300, 400), thickened("e1", 300, 200, 400)]
        ),
        SlabError::EdgesMeet {
            first: id("e0"),
            second: id("e1")
        }
    );
    // A 3-4-5 triangle: the corner at (4 m, 0) is sharper than a right angle.
    let sharp =
        Boundary::from_corners(&[at("a", 0, 0), at("b", 4000, 0), at("c", 0, 3000)], &[]).unwrap();
    assert_eq!(
        refused(sharp.clone(), vec![downturn("a", 100, 100)]),
        SlabError::EdgeEndOutside { edge: id("a") }
    );
    // Its long side, 5 m, takes a width the length divides: 300 mm in is (-180, -240) mm.
    let along = Shape::new(sharp, None, vec![downturn("b", 300, 100)]);
    assert_eq!(
        along.unwrap_err(),
        SlabError::EdgeEndOutside { edge: id("b") }
    );
}

/// The refusals of a piece that does not fit its floor, by name: wider than its floor, or
/// reaching the far side; two pieces across a narrow floor overlapping; a volume between cubic
/// steps.
#[test]
fn a_slab_edge_that_does_not_fit_is_refused() {
    let refused = |b: Boundary, e: Vec<SlabEdge>| Shape::new(b, None, e).unwrap_err();
    let narrow = || rect(4000, 300);
    assert_eq!(
        refused(narrow(), vec![downturn("e0", 400, 100)]),
        SlabError::EdgeTooWide { edge: id("e0") }
    );
    assert_eq!(
        refused(narrow(), vec![downturn("e0", 300, 100)]),
        SlabError::EdgeTooWide { edge: id("e0") }
    );
    assert_eq!(
        refused(
            rect(4000, 600),
            vec![downturn("e0", 400, 100), downturn("e2", 400, 100)]
        ),
        SlabError::EdgesOverlap {
            first: id("e0"),
            second: id("e2")
        }
    );
    // In steps: an edge 6,401 steps long, a thickened edge 2 steps wide, 1 at its foot and 1
    // deep: 1.5 square steps of section along an odd length.
    let odd = Boundary::from_corners(
        &[
            Corner {
                id: Some(id("a")),
                at: Point { x: 0, y: 0 },
            },
            Corner {
                id: Some(id("b")),
                at: Point { x: 6401, y: 0 },
            },
            Corner {
                id: Some(id("c")),
                at: Point { x: 6401, y: 6400 },
            },
            Corner {
                id: Some(id("d")),
                at: Point { x: 0, y: 6400 },
            },
        ],
        &[],
    )
    .unwrap();
    let fine = SlabEdge {
        edge: id("a"),
        profile: EdgeProfile::Thickened {
            width: Length::from_steps(2),
            foot: Length::from_steps(1),
            depth: Length::from_steps(1),
        },
    };
    assert_eq!(
        refused(odd, vec![fine]),
        SlabError::EdgeVolumeNotExact { edge: id("a") }
    );
}
