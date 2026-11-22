//! Dragging a point of a solved sketch, judged through the solver's own types only, so the same
//! tests judge the kernel's solver on the day it answers behind them.

// The crate denies these outside tests (Cargo.toml); clippy.toml's allow-*-in-tests settings don't
// reach helpers outside `#[test]` functions, and a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_solver::{
    Budget, Constraint, ConstraintId, FreePoint, Point, PointId, Reach, Sketch, SolveError,
    Verdict, drag, mm, solve,
};

fn p(x_mm: i32, y_mm: i32) -> Point {
    Point::mm(x_mm, y_mm)
}

fn add(sketch: &mut Sketch, constraint: Constraint) -> ConstraintId {
    sketch.add_constraint(constraint).unwrap()
}

/// The sketch with a solve's answer written back: the solved sketch a drag starts from.
fn solved(sketch: &Sketch) -> Sketch {
    let answer = solve(sketch, Budget::DEFAULT).unwrap();
    sketch
        .with_positions(&answer.points, &answer.radii)
        .unwrap()
}

/// A 4,000 by 3,000 mm rectangle of four lines, each with its own two ends, tied corner to
/// corner, its first corner fixed and its bottom horizontal; with `dimensioned`, its width and
/// height are given too. Lines: bottom (points 0, 1), right (2, 3), top (4, 5), left (6, 7).
fn rectangle(dimensioned: bool) -> Sketch {
    let mut sketch = Sketch::new();
    let corners = [
        (0, 0),
        (4_000, 0),
        (4_000, 0),
        (4_000, 3_000),
        (4_000, 3_000),
        (0, 3_000),
        (0, 3_000),
        (0, 0),
    ];
    let ends: Vec<PointId> = corners
        .iter()
        .map(|(x, y)| sketch.add_point(mm(*x), mm(*y)).unwrap())
        .collect();
    let lines: Vec<_> = (0..4)
        .map(|i| sketch.add_line(ends[2 * i], ends[2 * i + 1]).unwrap())
        .collect();
    for (a, b) in [(1, 2), (3, 4), (5, 6), (7, 0)] {
        add(&mut sketch, Constraint::Coincident(ends[a], ends[b]));
    }
    add(&mut sketch, Constraint::Fixed(ends[0]));
    add(&mut sketch, Constraint::Horizontal(lines[0]));
    for i in 0..3 {
        add(
            &mut sketch,
            Constraint::Perpendicular(lines[i], lines[i + 1]),
        );
    }
    if dimensioned {
        for (a, b, length_mm) in [(0, 1, 4_000), (6, 7, 3_000)] {
            add(
                &mut sketch,
                Constraint::Distance {
                    a: ends[a],
                    b: ends[b],
                    steps: mm(length_mm),
                },
            );
        }
    }
    sketch
}

#[test]
fn a_dragged_rectangle_corner_keeps_it_a_rectangle() {
    let sketch = solved(&rectangle(false));
    let dragged = drag(&sketch, PointId(3), p(5_000, 2_000), Budget::DEFAULT).unwrap();
    assert_eq!(dragged.reach, Reach::Target);
    let corners = [
        (0, 0),
        (5_000, 0),
        (5_000, 0),
        (5_000, 2_000),
        (5_000, 2_000),
        (0, 2_000),
        (0, 2_000),
        (0, 0),
    ];
    let expected: Vec<Point> = corners.iter().map(|(x, y)| p(*x, *y)).collect();
    assert_eq!(dragged.solved.points, expected);
    // Still free in width and height, as before the drag.
    let Verdict::UnderConstrained { dof, .. } = dragged.solved.verdict else {
        panic!(
            "expected under-constrained, got {:?}",
            dragged.solved.verdict
        );
    };
    assert_eq!(dof, 2);
}

#[test]
fn a_fully_constrained_point_will_not_move() {
    let sketch = solved(&rectangle(true));
    let before = sketch.points().to_vec();
    let dragged = drag(&sketch, PointId(3), p(4_500, 3_500), Budget::DEFAULT).unwrap();
    let Reach::Refused { blocking } = &dragged.reach else {
        panic!("expected a refusal, got {:?}", dragged.reach);
    };
    // The width and the height hold it, both moved by the drag.
    assert!(blocking.contains(&ConstraintId(9)), "{blocking:?}");
    assert!(blocking.contains(&ConstraintId(10)), "{blocking:?}");
    assert_eq!(dragged.solved.points, before);
    assert_eq!(dragged.solved.verdict, Verdict::FullyConstrained);

    // A fixed point is refused at once, named by its fixing.
    let dragged = drag(&sketch, PointId(0), p(10, 10), Budget::DEFAULT).unwrap();
    assert_eq!(
        dragged.reach,
        Reach::Refused {
            blocking: vec![ConstraintId(4)]
        }
    );
    assert_eq!(dragged.solved.points, before);
}

/// A fixed-length horizontal line from a fixed point, and an arc of radius 500 tangent to it at
/// its far end, sharing that end as its start. The arc's own end is free to slide round it.
/// Points: line start 0, line end and arc start 1, centre 2, arc end 3.
fn tangent_arc() -> (Sketch, ConstraintId) {
    let mut sketch = Sketch::new();
    let start = sketch.add_point(mm(0), mm(0)).unwrap();
    let join = sketch.add_point(mm(1_000), mm(0)).unwrap();
    let centre = sketch.add_point(mm(1_000), mm(500)).unwrap();
    let end = sketch.add_point(mm(1_500), mm(500)).unwrap();
    let line = sketch.add_line(start, join).unwrap();
    let arc = sketch.add_arc(centre, join, end).unwrap();
    add(&mut sketch, Constraint::Fixed(start));
    add(&mut sketch, Constraint::Horizontal(line));
    add(
        &mut sketch,
        Constraint::Distance {
            a: start,
            b: join,
            steps: mm(1_000),
        },
    );
    let side = sketch.side_of(centre, line).unwrap();
    add(
        &mut sketch,
        Constraint::TangentLine {
            line,
            curve: arc,
            side,
        },
    );
    let radius = add(
        &mut sketch,
        Constraint::Radius {
            curve: arc,
            steps: mm(500),
        },
    );
    (sketch, radius)
}

#[test]
fn a_tangent_arcs_end_slides_round_its_circle() {
    let (sketch, radius) = tangent_arc();
    let sketch = solved(&sketch);
    assert_eq!(sketch.points()[3], p(1_500, 500));

    // To a place on the circle: it gets there, and nothing else moves.
    let dragged = drag(&sketch, PointId(3), p(1_300, 900), Budget::DEFAULT).unwrap();
    assert_eq!(dragged.reach, Reach::Target);
    assert_eq!(
        dragged.solved.points,
        vec![p(0, 0), p(1_000, 0), p(1_000, 500), p(1_300, 900)]
    );
    assert_eq!(dragged.solved.radii, vec![mm(500)]);
    assert_eq!(
        dragged.solved.verdict,
        Verdict::UnderConstrained {
            dof: 1,
            free: vec![FreePoint {
                point: PointId(3),
                dof: 1
            }],
        }
    );

    // Beyond the circle: it stops on the circle, on the way to the target, held by the radius.
    let dragged = drag(&sketch, PointId(3), p(1_600, 1_300), Budget::DEFAULT).unwrap();
    let Reach::Limit { blocking } = &dragged.reach else {
        panic!("expected a limit, got {:?}", dragged.reach);
    };
    assert!(blocking.contains(&radius), "{blocking:?}");
    assert_eq!(dragged.solved.points[3], p(1_300, 900));
    assert_eq!(dragged.solved.points[2], p(1_000, 500));
}

#[test]
fn a_drag_is_the_same_every_time_and_counts_its_work() {
    let (sketch, _) = tangent_arc();
    let sketch = solved(&sketch);
    let once = drag(&sketch, PointId(3), p(1_600, 1_300), Budget::DEFAULT).unwrap();
    let twice = drag(
        &sketch.clone(),
        PointId(3),
        p(1_600, 1_300),
        Budget::DEFAULT,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_vec(&once).unwrap(),
        serde_json::to_vec(&twice).unwrap()
    );
    assert_eq!(
        drag(&sketch, PointId(3), p(1_600, 1_300), Budget::steps(2)),
        Err(SolveError::WorkLimit { steps: 2 })
    );
    assert_eq!(
        drag(&sketch, PointId(9), p(0, 0), Budget::DEFAULT),
        Err(SolveError::UnknownPoint { point: PointId(9) })
    );
}
