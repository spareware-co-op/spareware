//! A floor sketched with the solver (job 5.2): a boundary whose sketch holds constraints is the
//! sketch as solved; a dimension changed moves the outline; a sketch the solver cannot solve, or
//! one that solves to an outline crossing itself, is refused by name; the stored form keeps the
//! constraints and reads back the same.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{at, id};
use construct_params::{Area, Length};
use construct_slabs::{
    Boundary, Corner, Ring, SlabError, area, boundary_of, floor_recipe, perimeter,
};
use construct_solver::{Constraint, ConstraintId, LineId, Point, PointId, Sketch, mm};

/// A boundary drawn from `corners`, then given `constraints` (each made from the point and line
/// numbers of the drawing: corner `k` is point `k`, the edge from it is line `k`).
fn sketched(corners: &[Corner], constraints: &[Constraint]) -> Result<Boundary, SlabError> {
    let drawn = Boundary::from_corners(corners, &[]).unwrap();
    let mut sketch: Sketch = drawn.sketch().clone();
    for c in constraints {
        sketch.add_constraint(*c).unwrap();
    }
    Boundary::new(sketch, drawn.outer().to_vec(), Vec::new())
}

fn p(k: u32) -> PointId {
    PointId(k)
}

fn l(k: u32) -> LineId {
    LineId(k)
}

/// A rough four-sided sketch held square, its first corner fixed, 4 m along x and 3 m up: the
/// rectangle the constraints say, wherever it was drawn.
fn rectangle_constraints() -> Vec<Constraint> {
    vec![
        Constraint::Fixed(p(0)),
        Constraint::Horizontal(l(0)),
        Constraint::Vertical(l(1)),
        Constraint::Horizontal(l(2)),
        Constraint::Vertical(l(3)),
        Constraint::Distance {
            a: p(0),
            b: p(1),
            steps: mm(4000),
        },
        Constraint::Distance {
            a: p(1),
            b: p(2),
            steps: mm(3000),
        },
    ]
}

fn rough() -> Vec<Corner> {
    vec![
        at("e0", 0, 0),
        at("e1", 4100, 37),
        at("e2", 4050, 2950),
        at("e3", -20, 3020),
    ]
}

/// A rough sketch held square and dimensioned solves to its rectangle exactly: its corners, its
/// area of 12 m2 and perimeter of 14 m are the solved ones, and the stored sketch keeps the
/// points as drawn and the constraints, reading back the same.
#[test]
fn a_sketched_floor_is_its_solved_outline() {
    let floor = sketched(&rough(), &rectangle_constraints()).unwrap();
    let corners: Vec<Point> = floor
        .corners(Ring::Outer)
        .unwrap()
        .into_iter()
        .map(|(_, p)| p)
        .collect();
    assert_eq!(
        corners,
        [
            Point::mm(0, 0),
            Point::mm(4000, 0),
            Point::mm(4000, 3000),
            Point::mm(0, 3000)
        ]
    );
    assert_eq!(area(&floor).unwrap(), Area::from_mm2(12_000_000));
    assert_eq!(perimeter(&floor).unwrap(), Length::from_mm(14_000));
    assert_eq!(floor.sketch().points()[1], Point::mm(4100, 37));
    assert_eq!(floor.solved().points()[1], Point::mm(4000, 0));
    let recipe = floor_recipe(&floor);
    assert_eq!(boundary_of(&recipe).unwrap(), floor);
    assert_eq!(
        recipe.args["boundary"]["sketch"]["constraints"]
            .as_array()
            .unwrap()
            .len(),
        7
    );
}

/// A dimension changed moves the outline: 4 m to 5 m makes 15 m2, every edge keeping its ID;
/// a constraint that holds no length is not a dimension.
#[test]
fn a_dimension_change_moves_the_outline() {
    let floor = sketched(&rough(), &rectangle_constraints()).unwrap();
    let wider = floor.with_dimension(ConstraintId(5), mm(5000)).unwrap();
    assert_eq!(area(&wider).unwrap(), Area::from_mm2(15_000_000));
    let corners = wider.corners(Ring::Outer).unwrap();
    assert_eq!(corners[2], (id("e2"), Point::mm(5000, 3000)));
    assert_eq!(
        wider.sketch().constraints()[5],
        Constraint::Distance {
            a: p(0),
            b: p(1),
            steps: mm(5000),
        }
    );
    assert_eq!(
        floor.with_dimension(ConstraintId(1), mm(5000)),
        Err(SlabError::NotADimension { constraint: 1 })
    );
    assert_eq!(
        floor.with_dimension(ConstraintId(70), mm(5000)),
        Err(SlabError::NotADimension { constraint: 70 })
    );
}

/// A sketch the solver cannot solve is refused by name, with the solver's reason: here its four
/// corners are fixed where drawn and one side is held 5 m long besides.
#[test]
fn a_sketch_the_solver_cannot_solve_is_refused() {
    let square = [
        at("e0", 0, 0),
        at("e1", 4000, 0),
        at("e2", 4000, 3000),
        at("e3", 0, 3000),
    ];
    let held = [
        Constraint::Fixed(p(0)),
        Constraint::Fixed(p(1)),
        Constraint::Fixed(p(2)),
        Constraint::Fixed(p(3)),
        Constraint::Distance {
            a: p(0),
            b: p(1),
            steps: mm(5000),
        },
    ];
    let Err(SlabError::NotSolved { why }) = sketched(&square, &held) else {
        panic!("an over-constrained sketch is refused");
    };
    assert!(why.starts_with("over-constrained"), "{why}");
    assert!(
        SlabError::NotSolved { why }
            .to_string()
            .starts_with("the floor's sketch cannot be solved")
    );
}

/// The L-shaped floor with its notch dimensioned: 4 m solves; 9 m pushes the notch past the far
/// side, and the solved outline, crossing itself, is refused as a drawn one would be.
#[test]
fn a_dimension_that_makes_the_outline_cross_itself_is_refused() {
    let corners = [
        at("e0", 0, 0),
        at("e1", 8000, 0),
        at("e2", 8000, 4000),
        at("e3", 4000, 4000),
        at("e4", 4000, 8000),
        at("e5", 0, 8000),
    ];
    let held = [
        Constraint::Fixed(p(0)),
        Constraint::Fixed(p(1)),
        Constraint::Fixed(p(2)),
        Constraint::Fixed(p(5)),
        Constraint::Horizontal(l(2)),
        Constraint::Vertical(l(3)),
        Constraint::Horizontal(l(4)),
        Constraint::Distance {
            a: p(2),
            b: p(3),
            steps: mm(4000),
        },
    ];
    let floor = sketched(&corners, &held).unwrap();
    assert_eq!(area(&floor).unwrap(), Area::from_mm2(48_000_000));
    assert_eq!(
        area(&floor.with_dimension(ConstraintId(7), mm(6000)).unwrap()).unwrap(),
        Area::from_mm2(40_000_000)
    );
    assert_eq!(
        floor.with_dimension(ConstraintId(7), mm(9000)),
        Err(SlabError::Crossing {
            ring: Ring::Outer,
            first: id("e2"),
            second: id("e5"),
        })
    );
}

/// A command's plain form may carry constraints: the same sketch as one made by hand, the same
/// corners, and the same refusals; a point's side of a line, left out, is read from the drawing.
#[test]
fn a_plain_boundary_may_carry_constraints() {
    let steps = |millimetres: i32| mm(millimetres);
    let corner =
        |name: &str, x: i32, y: i32| serde_json::json!({ "id": name, "at": [steps(x), steps(y)] });
    let plain = serde_json::json!({
        "outer": [corner("e0", 0, 0), corner("e1", 4100, 100), corner("e2", 3900, 3100),
                  corner("e3", -50, 2900)],
        "constraints": [
            { "kind": "fixed", "point": 0 },
            { "kind": "horizontal", "line": 0 },
            { "kind": "vertical", "line": 1 },
            { "kind": "horizontal", "line": 2 },
            { "kind": "vertical", "line": 3 },
            { "kind": "distance", "a": 0, "b": 1, "steps": steps(4000) },
            { "kind": "point_line_distance", "point": 2, "line": 0, "steps": steps(3000) },
        ],
    });
    let made = Boundary::from_plain(&plain).unwrap();
    assert_eq!(area(&made).unwrap(), Area::from_mm2(12_000_000));
    // The same, by hand.
    let by_hand = sketched(
        &[
            Corner {
                id: Some(id("e0")),
                at: Point::mm(0, 0),
            },
            Corner {
                id: Some(id("e1")),
                at: Point::mm(4100, 100),
            },
            Corner {
                id: Some(id("e2")),
                at: Point::mm(3900, 3100),
            },
            Corner {
                id: Some(id("e3")),
                at: Point::mm(-50, 2900),
            },
        ],
        &[
            Constraint::Fixed(p(0)),
            Constraint::Horizontal(l(0)),
            Constraint::Vertical(l(1)),
            Constraint::Horizontal(l(2)),
            Constraint::Vertical(l(3)),
            Constraint::Distance {
                a: p(0),
                b: p(1),
                steps: mm(4000),
            },
            Constraint::PointLineDistance {
                point: p(2),
                line: l(0),
                steps: mm(3000),
                side: construct_solver::Side::Left,
            },
        ],
    )
    .unwrap();
    assert_eq!(made, by_hand);
    // Refused: a constraint of no kind, one that names a point the sketch lacks, and one more
    // distance than the shape can hold.
    for bad in [
        serde_json::json!([{ "kind": "wobbly" }]),
        serde_json::json!([{ "kind": "fixed", "point": 9 }]),
        serde_json::json!([{ "kind": "fixed", "point": 0, "extra": 1 }]),
    ] {
        let mut value = plain.clone();
        value["constraints"] = bad;
        assert!(matches!(
            Boundary::from_plain(&value),
            Err(SlabError::Malformed(_))
        ));
    }
}
