//! The sketch solver's behaviour, judged through its own types only, so the same tests judge the
//! kernel's solver on the day it answers behind them.

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
    Budget, COORD_LIMIT_MM, Constraint, ConstraintId, FreePoint, LineId, Point, PointId, Side,
    Sketch, SketchError, SolveError, Solved, Verdict, solve,
};

fn solved_points(sketch: &Sketch) -> Vec<Point> {
    solve(sketch, Budget::DEFAULT).unwrap().points
}

fn p(x_mm: i64, y_mm: i64) -> Point {
    Point { x_mm, y_mm }
}

/// A rectangle drawn roughly as four lines, each with its own two ends, tied corner to corner.
/// Lines: bottom (points 0, 1), right (2, 3), top (4, 5), left (6, 7).
struct Rectangle {
    sketch: Sketch,
    right: LineId,
    left: LineId,
    ends: Vec<PointId>,
}

fn rough_rectangle() -> Rectangle {
    let mut sketch = Sketch::new();
    let drawn = [
        (0, 0),
        (3_950, 40),
        (3_950, 40),
        (4_020, 3_100),
        (4_020, 3_100),
        (-60, 2_940),
        (-60, 2_940),
        (0, 0),
    ];
    let ends: Vec<PointId> = drawn
        .iter()
        .map(|(x, y)| sketch.add_point(*x, *y).unwrap())
        .collect();
    let mut line = |i: usize| sketch.add_line(ends[i], ends[i + 1]).unwrap();
    let bottom = line(0);
    let right = line(2);
    let top = line(4);
    let left = line(6);
    for (a, b) in [(1, 2), (3, 4), (5, 6), (7, 0)] {
        sketch
            .add_constraint(Constraint::Coincident(ends[a], ends[b]))
            .unwrap();
    }
    sketch.add_constraint(Constraint::Fixed(ends[0])).unwrap();
    sketch
        .add_constraint(Constraint::Horizontal(bottom))
        .unwrap();
    for (a, b) in [(bottom, right), (right, top), (top, left)] {
        sketch
            .add_constraint(Constraint::Perpendicular(a, b))
            .unwrap();
    }
    Rectangle {
        sketch,
        right,
        left,
        ends,
    }
}

fn distance(sketch: &mut Sketch, a: PointId, b: PointId, mm: i64) -> ConstraintId {
    sketch
        .add_constraint(Constraint::Distance { a, b, mm })
        .unwrap()
}

/// The rectangle with its width and height dimensioned.
fn rectangle() -> Rectangle {
    let mut r = rough_rectangle();
    distance(&mut r.sketch, r.ends[0], r.ends[1], 4_000);
    distance(&mut r.sketch, r.ends[6], r.ends[7], 3_000);
    r
}

#[test]
fn a_rectangle_solves_to_the_exact_rectangle() {
    let r = rectangle();
    let solved = solve(&r.sketch, Budget::DEFAULT).unwrap();
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
    let expected: Vec<Point> = corners.iter().map(|(x, y)| p(*x, *y)).collect();
    assert_eq!(solved.points, expected);
    assert_eq!(solved.verdict, Verdict::FullyConstrained);
    assert_eq!(solved.redundant, Vec::new());
}

#[test]
fn a_triangle_from_three_lengths() {
    let mut sketch = Sketch::new();
    let a = sketch.add_point(0, 0).unwrap();
    let b = sketch.add_point(5_800, 150).unwrap();
    let c = sketch.add_point(3_200, 3_700).unwrap();
    let base = sketch.add_line(a, b).unwrap();
    sketch.add_line(b, c).unwrap();
    sketch.add_line(c, a).unwrap();
    sketch.add_constraint(Constraint::Fixed(a)).unwrap();
    sketch.add_constraint(Constraint::Horizontal(base)).unwrap();
    distance(&mut sketch, a, b, 6_000);
    distance(&mut sketch, b, c, 5_000);
    distance(&mut sketch, c, a, 5_000);
    let solved = solve(&sketch, Budget::DEFAULT).unwrap();
    assert_eq!(solved.points, vec![p(0, 0), p(6_000, 0), p(3_000, 4_000)]);
    assert_eq!(solved.verdict, Verdict::FullyConstrained);
}

#[test]
fn an_undimensioned_height_is_one_freedom_on_the_top_corners() {
    let mut r = rough_rectangle();
    distance(&mut r.sketch, r.ends[0], r.ends[1], 4_000);
    let solved = solve(&r.sketch, Budget::DEFAULT).unwrap();
    let free: Vec<FreePoint> = [3, 4, 5, 6]
        .iter()
        .map(|i| FreePoint {
            point: r.ends[*i],
            dof: 1,
        })
        .collect();
    assert_eq!(solved.verdict, Verdict::UnderConstrained { dof: 1, free });
    // Least change: the width is met and some height near the drawn one is kept.
    assert_eq!(solved.points[1], p(4_000, 0));
    assert_eq!(solved.points[3].x_mm, 4_000);
    assert_eq!(solved.points[3].y_mm, solved.points[6].y_mm);
}

#[test]
fn a_sketch_with_nothing_fixed_keeps_its_placement_freedoms() {
    let mut sketch = Sketch::new();
    let a = sketch.add_point(10, 20).unwrap();
    let b = sketch.add_point(990, 20).unwrap();
    distance(&mut sketch, a, b, 1_000);
    let solved = solve(&sketch, Budget::DEFAULT);
    // Two free points joined by one length: four coordinates, one equation. Least change
    // stretches the line evenly at both ends.
    let Ok(Solved {
        verdict: Verdict::UnderConstrained { dof, free },
        ..
    }) = solved
    else {
        panic!("expected under-constrained, got {solved:?}");
    };
    assert_eq!(dof, 3);
    assert_eq!(solved_points(&sketch), vec![p(0, 20), p(1_000, 20)]);
    assert_eq!(
        free,
        vec![
            FreePoint { point: a, dof: 2 },
            FreePoint { point: b, dof: 2 }
        ]
    );
}

#[test]
fn a_contradicting_dimension_is_over_constrained() {
    let mut r = rectangle();
    let extra = distance(&mut r.sketch, r.ends[2], r.ends[3], 3_500);
    let error = solve(&r.sketch, Budget::DEFAULT).unwrap_err();
    let SolveError::OverConstrained { conflicting } = error else {
        panic!("expected over-constrained, got {error:?}");
    };
    // The two heights contradict each other through the right angles.
    let height = ConstraintId(extra.0 - 1);
    assert!(conflicting.contains(&height), "{conflicting:?}");
    assert!(conflicting.contains(&extra), "{conflicting:?}");
}

#[test]
fn a_dimension_that_repeats_the_others_is_redundant() {
    let mut r = rectangle();
    let repeat = r
        .sketch
        .add_constraint(Constraint::EqualLength(r.left, r.right))
        .unwrap();
    let again = r
        .sketch
        .add_constraint(Constraint::Fixed(r.ends[0]))
        .unwrap();
    let solved = solve(&r.sketch, Budget::DEFAULT).unwrap();
    assert_eq!(solved.verdict, Verdict::FullyConstrained);
    assert!(solved.redundant.contains(&repeat), "{:?}", solved.redundant);
    assert!(solved.redundant.contains(&again), "{:?}", solved.redundant);
}

#[test]
fn parallel_equal_and_point_to_line_distance() {
    // A fixed base line, a point held 1,200 mm to its left on a vertical guide, and a second line
    // from that point, parallel to the base and as long as it.
    let mut sketch = Sketch::new();
    let a = sketch.add_point(0, 0).unwrap();
    let b = sketch.add_point(5_000, 0).unwrap();
    let foot = sketch.add_point(2_000, -500).unwrap();
    let top = sketch.add_point(2_100, 900).unwrap();
    let far = sketch.add_point(6_500, 1_300).unwrap();
    let base = sketch.add_line(a, b).unwrap();
    let guide = sketch.add_line(foot, top).unwrap();
    let copy = sketch.add_line(top, far).unwrap();
    for fixed in [a, b, foot] {
        sketch.add_constraint(Constraint::Fixed(fixed)).unwrap();
    }
    sketch.add_constraint(Constraint::Vertical(guide)).unwrap();
    assert_eq!(sketch.side_of(top, base), Some(Side::Left));
    sketch
        .add_constraint(Constraint::PointLineDistance {
            point: top,
            line: base,
            mm: 1_200,
            side: Side::Left,
        })
        .unwrap();
    sketch
        .add_constraint(Constraint::Parallel(copy, base))
        .unwrap();
    sketch
        .add_constraint(Constraint::EqualLength(copy, base))
        .unwrap();
    let solved = solve(&sketch, Budget::DEFAULT).unwrap();
    assert_eq!(solved.points[3], p(2_000, 1_200));
    assert_eq!(solved.points[4], p(7_000, 1_200));
    assert_eq!(solved.verdict, Verdict::FullyConstrained);
}

#[test]
fn the_same_sketch_gives_the_same_bytes() {
    let r = rectangle();
    let once = serde_json::to_vec(&solve(&r.sketch, Budget::DEFAULT).unwrap()).unwrap();
    let twice = serde_json::to_vec(&solve(&r.sketch.clone(), Budget::DEFAULT).unwrap()).unwrap();
    assert_eq!(once, twice);
    // Pinned, so a machine that solves it differently, even by one step, fails here.
    assert_eq!(String::from_utf8(once).unwrap(), GOLDEN_RECTANGLE);
}

const GOLDEN_RECTANGLE: &str = concat!(
    r#"{"points":[{"x_mm":0,"y_mm":0},{"x_mm":4000,"y_mm":0},{"x_mm":4000,"y_mm":0},"#,
    r#"{"x_mm":4000,"y_mm":3000},{"x_mm":4000,"y_mm":3000},{"x_mm":0,"y_mm":3000},"#,
    r#"{"x_mm":0,"y_mm":3000},{"x_mm":0,"y_mm":0}],"verdict":"FullyConstrained","#,
    r#""redundant":[],"steps":8}"#,
);

#[test]
fn running_out_of_steps_is_an_error() {
    let r = rectangle();
    assert_eq!(
        solve(&r.sketch, Budget::steps(2)),
        Err(SolveError::WorkLimit { steps: 2 })
    );
}

#[test]
fn a_shape_that_does_not_fall_on_whole_millimetres_is_refused() {
    // An equilateral triangle with 1 m sides: its apex is 866.025... mm high, and at 866 mm its
    // two slopes miss 1,000 mm by about 0.02 mm, more than the model tolerance.
    let mut sketch = Sketch::new();
    let a = sketch.add_point(0, 0).unwrap();
    let b = sketch.add_point(1_010, 0).unwrap();
    let c = sketch.add_point(480, 850).unwrap();
    let base = sketch.add_line(a, b).unwrap();
    sketch.add_constraint(Constraint::Fixed(a)).unwrap();
    sketch.add_constraint(Constraint::Horizontal(base)).unwrap();
    distance(&mut sketch, a, b, 1_000);
    distance(&mut sketch, b, c, 1_000);
    distance(&mut sketch, c, a, 1_000);
    let error = solve(&sketch, Budget::DEFAULT).unwrap_err();
    let SolveError::RoundingRefused { residual_mm, .. } = error else {
        panic!("expected a refusal, got {error:?}");
    };
    assert!(residual_mm > 0.01 && residual_mm < 0.05, "{residual_mm}");
}

#[test]
fn a_distance_between_points_drawn_at_one_place_is_degenerate() {
    let mut sketch = Sketch::new();
    let a = sketch.add_point(100, 100).unwrap();
    let b = sketch.add_point(100, 100).unwrap();
    let d = distance(&mut sketch, a, b, 500);
    assert_eq!(
        solve(&sketch, Budget::DEFAULT),
        Err(SolveError::Degenerate { constraint: d })
    );
}

#[test]
fn bad_input_is_refused_when_added() {
    let mut sketch = Sketch::new();
    let a = sketch.add_point(0, 0).unwrap();
    let b = sketch.add_point(1, 0).unwrap();
    assert_eq!(sketch.add_line(a, a), Err(SketchError::SameTwice));
    assert_eq!(sketch.add_line(a, PointId(9)), Err(SketchError::Unknown));
    assert_eq!(
        sketch.add_constraint(Constraint::Distance { a, b, mm: 0 }),
        Err(SketchError::BadDistance)
    );
    assert_eq!(
        sketch.add_point(COORD_LIMIT_MM + 1, 0),
        Err(SketchError::OutOfRange)
    );
    let solved = solve(&Sketch::new(), Budget::DEFAULT).unwrap();
    assert_eq!(solved.points, Vec::new());
    assert_eq!(solved.verdict, Verdict::FullyConstrained);
}
