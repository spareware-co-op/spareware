//! A floor's boundary: one closed outer loop and any number of holes, held as the solver's
//! sketch; every refusal names the loop and the place; arcs are refused for now; the profile the
//! builder takes runs the outer loop anticlockwise and each hole clockwise.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{at, bare, id, l_corners, l_hole, l_with_hole, rectangle};
use construct_slabs::{Boundary, Edge, Ring, Segment, SlabError};
use construct_solver::{Constraint, Point, Sketch, mm};

/// Corners become a sketch of points and lines, one line per edge in order, and the loops name
/// each line by its edge's ID. Edges with no ID get `o<n>` on the outer loop and `h<k>-<n>` on
/// hole `k`.
#[test]
fn corners_make_a_sketch_and_loops() {
    let floor = l_with_hole();
    assert_eq!(floor.sketch().points().len(), 10);
    assert_eq!(floor.sketch().lines().len(), 10);
    assert_eq!(floor.sketch().constraints().len(), 0);
    let ids: Vec<String> = floor.outer().iter().map(|s| s.id.to_string()).collect();
    assert_eq!(ids, ["e0", "e1", "e2", "e3", "e4", "e5"]);
    assert_eq!(floor.holes().len(), 1);
    assert_eq!(
        floor.outer()[0].edge,
        Edge::Line(construct_solver::LineId(0))
    );
    let corners = floor.corners(Ring::Outer).unwrap();
    assert_eq!(corners[1], (id("e1"), Point::mm(8000, 0)));
    assert_eq!(floor.corners(Ring::Hole(1)), None);

    let plain = Boundary::from_corners(
        &[bare(0, 0), bare(1000, 0), bare(1000, 1000)],
        &[vec![bare(300, 100), bare(400, 100), bare(400, 200)]],
    );
    // A hole that touches nothing, inside a triangle.
    let plain = plain.unwrap();
    let ids: Vec<String> = plain
        .outer()
        .iter()
        .chain(&plain.holes()[0])
        .map(|s| s.id.to_string())
        .collect();
    assert_eq!(ids, ["o0", "o1", "o2", "h0-0", "h0-1", "h0-2"]);
}

/// The builder's profile: the outer loop anticlockwise and each hole clockwise, each corner with
/// the ID of the edge that starts there, whichever way the loops were drawn.
#[test]
fn the_profile_runs_the_outer_loop_anticlockwise_and_holes_clockwise() {
    let floor = l_with_hole();
    let profile = floor.profile();
    assert_eq!(profile.len(), 2);
    assert_eq!(profile[0], floor.corners(Ring::Outer).unwrap());
    // The hole was drawn anticlockwise, so it is run backwards: each edge keeps its own ID.
    let hole: Vec<(String, Point)> = profile[1]
        .iter()
        .map(|(edge, p)| (edge.to_string(), *p))
        .collect();
    assert_eq!(
        hole,
        [
            ("h3".to_owned(), Point::mm(1000, 1000)),
            ("h2".to_owned(), Point::mm(1000, 3000)),
            ("h1".to_owned(), Point::mm(3000, 3000)),
            ("h0".to_owned(), Point::mm(3000, 1000)),
        ]
    );
    let mut clockwise = l_corners();
    clockwise.reverse();
    let drawn_the_other_way = Boundary::from_corners(&clockwise, &[]).unwrap();
    let outer: Vec<String> = drawn_the_other_way.profile()[0]
        .iter()
        .map(|(edge, _)| edge.to_string())
        .collect();
    // Drawn clockwise from (0, 8000) as e5, e4, ... e0 (each edge from its corner to the next), it
    // is run backwards from the same corner: e0 first, down the left side.
    assert_eq!(outer, ["e0", "e1", "e2", "e3", "e4", "e5"]);
    assert_eq!(
        drawn_the_other_way.profile()[0][0],
        (id("e0"), Point::mm(0, 8000))
    );
}

/// Fewer than three sides, an edge of no length, and two edges with one ID are refused.
#[test]
fn sides_lengths_and_ids() {
    assert_eq!(
        Boundary::from_corners(&[at("a", 0, 0), at("b", 1000, 0)], &[]),
        Err(SlabError::TooFewSides {
            ring: Ring::Outer,
            sides: 2
        })
    );
    assert_eq!(
        Boundary::from_corners(
            &[
                at("a", 0, 0),
                at("b", 1000, 0),
                at("c", 1000, 0),
                at("d", 0, 1000)
            ],
            &[]
        ),
        Err(SlabError::ZeroLength {
            ring: Ring::Outer,
            segment: id("b")
        })
    );
    assert_eq!(
        Boundary::from_corners(&[at("a", 0, 0), at("b", 1000, 0), at("a", 0, 1000)], &[]),
        Err(SlabError::SameId { segment: id("a") })
    );
    let hole_repeats =
        Boundary::from_corners(&l_corners(), &[rectangle("e", 1000, 1000, 500, 500)]);
    assert_eq!(hole_repeats, Err(SlabError::SameId { segment: id("e0") }));
    // Three corners on one line: the last edge runs back over the first.
    assert_eq!(
        Boundary::from_corners(&[at("a", 0, 0), at("b", 1000, 0), at("c", 2000, 0)], &[]),
        Err(SlabError::Crossing {
            ring: Ring::Outer,
            first: id("a"),
            second: id("c")
        })
    );
}

/// A loop that crosses or touches itself is refused, naming the two edges.
#[test]
fn a_loop_may_not_cross_or_touch_itself() {
    // A bow tie: its first and third edges cross.
    let bow = [
        at("a", 0, 0),
        at("b", 1000, 1000),
        at("c", 1000, 0),
        at("d", 0, 1000),
    ];
    assert_eq!(
        Boundary::from_corners(&bow, &[]),
        Err(SlabError::Crossing {
            ring: Ring::Outer,
            first: id("a"),
            second: id("c")
        })
    );
    // A corner that lands on a far edge: d's end at (500, 0) touches a.
    let touch = [
        at("a", 0, 0),
        at("b", 1000, 0),
        at("c", 1000, 1000),
        at("d", 500, 0),
        at("e", 0, 1000),
    ];
    assert!(matches!(
        Boundary::from_corners(&touch, &[]),
        Err(SlabError::Crossing {
            ring: Ring::Outer,
            ..
        })
    ));
    // An edge that doubles back along the one before it: b runs back over a.
    let spike = [
        at("a", 0, 0),
        at("b", 2000, 0),
        at("c", 1000, 0),
        at("d", 1000, 1000),
    ];
    assert_eq!(
        Boundary::from_corners(&spike, &[]),
        Err(SlabError::Crossing {
            ring: Ring::Outer,
            first: id("a"),
            second: id("b")
        })
    );
    let text = SlabError::Crossing {
        ring: Ring::Hole(2),
        first: id("a"),
        second: id("c"),
    }
    .to_string();
    assert_eq!(
        text,
        "hole 2 crosses or touches itself where edges a and c meet"
    );
}

/// A hole sits inside the outer loop, touching nothing, and holes do not overlap or touch.
#[test]
fn holes_are_inside_and_apart() {
    let outside = rectangle("h", 9000, 0, 500, 500);
    assert_eq!(
        Boundary::from_corners(&l_corners(), &[outside]),
        Err(SlabError::HoleOutside { hole: 0 })
    );
    // In the notch of the L: inside its box, outside the floor.
    let in_the_notch = rectangle("h", 5000, 5000, 500, 500);
    assert_eq!(
        Boundary::from_corners(&l_corners(), &[in_the_notch]),
        Err(SlabError::HoleOutside { hole: 0 })
    );
    let across = rectangle("h", 7500, 1000, 1000, 500);
    assert_eq!(
        Boundary::from_corners(&l_corners(), &[across]),
        Err(SlabError::HoleCrossesOuter {
            hole: 0,
            segment: id("h0"),
            outer: id("e1")
        })
    );
    let touching = rectangle("h", 0, 1000, 500, 500);
    assert!(matches!(
        Boundary::from_corners(&l_corners(), &[touching]),
        Err(SlabError::HoleCrossesOuter { hole: 0, .. })
    ));
    let two = [
        rectangle("a", 1000, 1000, 1000, 1000),
        rectangle("b", 1500, 1500, 1000, 1000),
    ];
    assert!(matches!(
        Boundary::from_corners(&l_corners(), &two),
        Err(SlabError::HolesOverlap {
            first: 0,
            second: 1,
            ..
        })
    ));
    let nested = [
        rectangle("a", 1000, 1000, 2000, 2000),
        rectangle("b", 1500, 1500, 500, 500),
    ];
    assert_eq!(
        Boundary::from_corners(&l_corners(), &nested),
        Err(SlabError::HolesOverlap {
            first: 0,
            second: 1,
            segment: None
        })
    );
    let apart = [
        rectangle("a", 1000, 1000, 1000, 1000),
        rectangle("b", 2500, 1000, 1000, 1000),
    ];
    assert!(Boundary::from_corners(&l_corners(), &apart).is_ok());
    assert_eq!(
        Boundary::from_corners(&l_corners(), &[l_hole(), l_hole()]),
        Err(SlabError::SameId { segment: id("h0") })
    );
}

/// A sketch with constraints is refused until floors are sketched with the solver (job 5.2); a
/// sketch with a line no loop uses, or a loop whose edges do not join end to start, is refused.
#[test]
fn the_sketch_is_only_the_loops() {
    let floor = l_with_hole();
    let mut constrained = floor.sketch().clone();
    constrained
        .add_constraint(Constraint::Horizontal(construct_solver::LineId(0)))
        .unwrap();
    assert_eq!(
        Boundary::new(constrained, floor.outer().to_vec(), floor.holes().to_vec()),
        Err(SlabError::Constrained)
    );
    let mut spare = floor.sketch().clone();
    let p = spare.add_point(mm(100), mm(100)).unwrap();
    let q = spare.add_point(mm(200), mm(100)).unwrap();
    spare.add_line(p, q).unwrap();
    assert_eq!(
        Boundary::new(spare, floor.outer().to_vec(), floor.holes().to_vec()),
        Err(SlabError::Unused {
            what: "line 10".to_owned()
        })
    );
    // The outer loop with its edges out of order: e0 then e2.
    let mut shuffled = floor.outer().to_vec();
    shuffled.swap(1, 2);
    assert_eq!(
        Boundary::new(floor.sketch().clone(), shuffled, floor.holes().to_vec()),
        Err(SlabError::NotClosed {
            ring: Ring::Outer,
            segment: id("e0")
        })
    );
    let twice = vec![floor.outer()[0].clone(), {
        let mut again = floor.outer()[0].clone();
        again.id = id("x");
        again
    }];
    assert!(matches!(
        Boundary::new(floor.sketch().clone(), twice, Vec::new()),
        Err(SlabError::UsedTwice { .. } | SlabError::TooFewSides { .. })
    ));
}

/// An arc edge is held in the data model and in the stored form, and refused for now: the
/// engine's extrusion takes a polygon only, so an arc could not be built exactly.
#[test]
fn an_arc_is_refused_for_now() {
    let mut sketch = Sketch::new();
    let a = sketch.add_point(mm(0), mm(0)).unwrap();
    let b = sketch.add_point(mm(2000), mm(0)).unwrap();
    let centre = sketch.add_point(mm(1000), mm(0)).unwrap();
    let line = sketch.add_line(b, a).unwrap();
    // Anticlockwise from a to b about the centre: the half below the x axis... drawn over it.
    let curve = sketch.add_arc(centre, a, b).unwrap();
    let outer = vec![
        Segment {
            id: id("arc"),
            edge: Edge::Arc {
                curve,
                reversed: false,
            },
        },
        Segment {
            id: id("chord"),
            edge: Edge::Line(line),
        },
    ];
    let refused = Boundary::new(sketch, outer, Vec::new()).unwrap_err();
    assert_eq!(
        refused,
        SlabError::Arc {
            ring: Ring::Outer,
            segment: id("arc")
        }
    );
    assert_eq!(
        refused.to_string(),
        "edge arc of the outer loop is an arc, and a floor with an arc edge cannot be built \
         exactly yet: draw it with straight edges"
    );
}

/// Corners in a command's plain form: `{"outer":[{"id":..,"at":[x,y]}],"holes":[[..]]}` in grid
/// steps, the IDs optional; anything else is refused, never repaired.
#[test]
fn the_plain_form_a_command_takes() {
    let steps = |v: i32| i64::from(v) * 6400;
    let value = serde_json::json!({
        "outer": [
            { "id": "e0", "at": [0, 0] },
            { "id": "e1", "at": [steps(8000), 0] },
            { "id": "e2", "at": [steps(8000), steps(4000)] },
            { "id": "e3", "at": [steps(4000), steps(4000)] },
            { "id": "e4", "at": [steps(4000), steps(8000)] },
            { "id": "e5", "at": [0, steps(8000)] },
        ],
        "holes": [[
            { "id": "h0", "at": [steps(1000), steps(1000)] },
            { "id": "h1", "at": [steps(3000), steps(1000)] },
            { "id": "h2", "at": [steps(3000), steps(3000)] },
            { "id": "h3", "at": [steps(1000), steps(3000)] },
        ]],
    });
    assert_eq!(Boundary::from_plain(&value).unwrap(), l_with_hole());
    let no_holes = serde_json::json!({ "outer": [{ "at": [0, 0] }, { "at": [6400, 0] }, { "at": [0, 6400] }] });
    assert_eq!(Boundary::from_plain(&no_holes).unwrap().outer().len(), 3);
    for bad in [
        serde_json::json!({ "outer": [] , "extra": 1 }),
        serde_json::json!({ "outer": [{ "at": [0.5, 0] }] }),
        serde_json::json!({ "outer": [{ "id": "not an id", "at": [0, 0] }] }),
        serde_json::json!([]),
    ] {
        assert!(matches!(
            Boundary::from_plain(&bad),
            Err(SlabError::Malformed(_))
        ));
    }
    let far = serde_json::json!({ "outer": [{ "at": [1_i64 << 41, 0] }, { "at": [0, 0] }, { "at": [0, 1] }] });
    assert_eq!(Boundary::from_plain(&far), Err(SlabError::OutOfRange));
}
