//! Circles, arcs and their constraints, judged through the solver's own types only, so the same
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
    Budget, Constraint, ConstraintId, CurveId, FreePoint, Point, PointId, Side, Sketch,
    SketchError, SolveError, Touch, Verdict, solve,
};

fn p(x_mm: i64, y_mm: i64) -> Point {
    Point { x_mm, y_mm }
}

fn add(sketch: &mut Sketch, constraint: Constraint) -> ConstraintId {
    sketch.add_constraint(constraint).unwrap()
}

/// A slot: two straight sides and two half-circle ends, drawn roughly. The arcs share their ends
/// with the lines, so each tangency is at a shared point.
/// Points: left centre 0, right centre 1, top-left 2, top-right 3, bottom-right 4, bottom-left 5.
fn slot() -> Sketch {
    let mut sketch = Sketch::new();
    let left_centre = sketch.add_point(0, 0).unwrap();
    let right_centre = sketch.add_point(2_950, 30).unwrap();
    let top_left = sketch.add_point(10, 510).unwrap();
    let top_right = sketch.add_point(2_990, 495).unwrap();
    let bottom_right = sketch.add_point(3_010, -490).unwrap();
    let bottom_left = sketch.add_point(-5, -505).unwrap();
    let top = sketch.add_line(top_left, top_right).unwrap();
    let bottom = sketch.add_line(bottom_right, bottom_left).unwrap();
    // Anticlockwise: the left end from top to bottom round the left, the right end from bottom
    // to top round the right.
    let left = sketch.add_arc(left_centre, top_left, bottom_left).unwrap();
    let right = sketch
        .add_arc(right_centre, bottom_right, top_right)
        .unwrap();
    add(&mut sketch, Constraint::Fixed(left_centre));
    add(&mut sketch, Constraint::Horizontal(top));
    for line in [top, bottom] {
        for (curve, centre) in [(left, left_centre), (right, right_centre)] {
            let side = sketch.side_of(centre, line).unwrap();
            add(&mut sketch, Constraint::TangentLine { line, curve, side });
        }
    }
    add(&mut sketch, Constraint::EqualRadius(left, right));
    add(
        &mut sketch,
        Constraint::Radius {
            curve: left,
            mm: 500,
        },
    );
    add(
        &mut sketch,
        Constraint::Distance {
            a: left_centre,
            b: right_centre,
            mm: 3_000,
        },
    );
    sketch
}

#[test]
fn a_slot_of_two_lines_and_two_tangent_arcs_solves() {
    let solved = solve(&slot(), Budget::DEFAULT).unwrap();
    assert_eq!(
        solved.points,
        vec![
            p(0, 0),
            p(3_000, 0),
            p(0, 500),
            p(3_000, 500),
            p(3_000, -500),
            p(0, -500)
        ]
    );
    assert_eq!(solved.radii_mm, vec![500, 500]);
    assert_eq!(solved.verdict, Verdict::FullyConstrained);
    assert_eq!(solved.redundant, Vec::new());
    assert_eq!(solved.free_radii, Vec::new());
}

#[test]
fn the_same_slot_twice_gives_the_same_bytes() {
    let once = serde_json::to_vec(&solve(&slot(), Budget::DEFAULT).unwrap()).unwrap();
    let twice = serde_json::to_vec(&solve(&slot().clone(), Budget::DEFAULT).unwrap()).unwrap();
    assert_eq!(once, twice);
    // Pinned, so a machine that solves it differently, even by one step, fails here.
    assert_eq!(String::from_utf8(once).unwrap(), GOLDEN_SLOT);
}

const GOLDEN_SLOT: &str = concat!(
    r#"{"points":[{"x_mm":0,"y_mm":0},{"x_mm":3000,"y_mm":0},{"x_mm":0,"y_mm":500},"#,
    r#"{"x_mm":3000,"y_mm":500},{"x_mm":3000,"y_mm":-500},{"x_mm":0,"y_mm":-500}],"#,
    r#""verdict":"FullyConstrained","redundant":[],"steps":8,"radii_mm":[500,500]}"#,
);

/// Two fixed lines meeting at a right angle at the origin, along the plane's two axes.
fn corner(sketch: &mut Sketch) -> (PointId, [construct_solver::LineId; 2]) {
    let origin = sketch.add_point(0, 0).unwrap();
    let along_x = sketch.add_point(2_000, 0).unwrap();
    let along_y = sketch.add_point(0, 2_000).unwrap();
    for point in [origin, along_x, along_y] {
        add(sketch, Constraint::Fixed(point));
    }
    let x_axis = sketch.add_line(origin, along_x).unwrap();
    let y_axis = sketch.add_line(origin, along_y).unwrap();
    (origin, [x_axis, y_axis])
}

/// A circle drawn roughly in the corner, tangent to both lines, with its radius given.
fn circle_in_corner() -> (Sketch, PointId, PointId, CurveId) {
    let mut sketch = Sketch::new();
    let (origin, lines) = corner(&mut sketch);
    let centre = sketch.add_point(280, 330).unwrap();
    let circle = sketch.add_circle(centre, 310).unwrap();
    for line in lines {
        let side = sketch.side_of(centre, line).unwrap();
        add(
            &mut sketch,
            Constraint::TangentLine {
                line,
                curve: circle,
                side,
            },
        );
    }
    add(
        &mut sketch,
        Constraint::Radius {
            curve: circle,
            mm: 300,
        },
    );
    (sketch, origin, centre, circle)
}

#[test]
fn a_circle_tangent_to_two_perpendicular_lines_with_a_radius() {
    let (sketch, _, centre, _) = circle_in_corner();
    assert_eq!(
        sketch.side_of(centre, construct_solver::LineId(0)),
        Some(Side::Left)
    );
    let solved = solve(&sketch, Budget::DEFAULT).unwrap();
    assert_eq!(solved.points[centre.0 as usize], p(300, 300));
    assert_eq!(solved.radii_mm, vec![300]);
    assert_eq!(solved.verdict, Verdict::FullyConstrained);
}

#[test]
fn concentric_circles_with_a_radius_and_a_diameter() {
    let mut sketch = Sketch::new();
    let a_centre = sketch.add_point(1_000, 1_000).unwrap();
    let b_centre = sketch.add_point(1_020, 990).unwrap();
    let a = sketch.add_circle(a_centre, 480).unwrap();
    let b = sketch.add_circle(b_centre, 790).unwrap();
    add(&mut sketch, Constraint::Fixed(a_centre));
    add(&mut sketch, Constraint::Concentric(a, b));
    add(&mut sketch, Constraint::Radius { curve: a, mm: 500 });
    add(
        &mut sketch,
        Constraint::Diameter {
            curve: b,
            mm: 1_600,
        },
    );
    let solved = solve(&sketch, Budget::DEFAULT).unwrap();
    assert_eq!(solved.points, vec![p(1_000, 1_000), p(1_000, 1_000)]);
    assert_eq!(solved.radii_mm, vec![500, 800]);
    assert_eq!(solved.verdict, Verdict::FullyConstrained);

    // An odd diameter needs a radius of 800.5 mm, which is not a whole millimetre.
    let mut odd = Sketch::new();
    let c = odd.add_point(0, 0).unwrap();
    let circle = odd.add_circle(c, 790).unwrap();
    add(&mut odd, Constraint::Fixed(c));
    let id = add(
        &mut odd,
        Constraint::Diameter {
            curve: circle,
            mm: 1_601,
        },
    );
    assert_eq!(id, ConstraintId(1));
    let error = solve(&odd, Budget::DEFAULT).unwrap_err();
    assert_eq!(
        error,
        SolveError::RoundingRefused {
            constraint: id,
            residual_mm: 1.0
        }
    );
}

#[test]
fn curves_tangent_outside_and_inside() {
    for (touch, drawn, radius, centre_at) in [
        (Touch::Outside, (1_250, 20), 750, 1_250),
        (Touch::FirstInside, (690, 10), 300, 700),
    ] {
        let mut sketch = Sketch::new();
        let fixed_centre = sketch.add_point(0, 0).unwrap();
        let moving_centre = sketch.add_point(drawn.0, drawn.1).unwrap();
        let fixed_radius = if touch == Touch::Outside { 500 } else { 1_000 };
        let fixed_curve = sketch.add_circle(fixed_centre, fixed_radius).unwrap();
        let moving = sketch.add_circle(moving_centre, radius + 20).unwrap();
        let between = sketch.add_line(fixed_centre, moving_centre).unwrap();
        add(&mut sketch, Constraint::Fixed(fixed_centre));
        add(
            &mut sketch,
            Constraint::Radius {
                curve: fixed_curve,
                mm: fixed_radius,
            },
        );
        add(
            &mut sketch,
            Constraint::Radius {
                curve: moving,
                mm: radius,
            },
        );
        add(&mut sketch, Constraint::Horizontal(between));
        // The drawn shape decides the way they touch: the moving circle's centre is inside the
        // fixed one's circle in the second case, and its radius the smaller.
        let first = if touch == Touch::Outside {
            fixed_curve
        } else {
            moving
        };
        let second = if first == moving { fixed_curve } else { moving };
        assert_eq!(sketch.touch_of(first, second), Some(touch));
        add(
            &mut sketch,
            Constraint::TangentCurves {
                a: first,
                b: second,
                touch,
            },
        );
        let solved = solve(&sketch, Budget::DEFAULT).unwrap();
        assert_eq!(solved.points[1], p(centre_at, 0), "{touch:?}");
        assert_eq!(solved.verdict, Verdict::FullyConstrained, "{touch:?}");
    }
}

#[test]
fn a_point_on_a_circle_and_on_a_line() {
    let mut sketch = Sketch::new();
    let centre = sketch.add_point(0, 0).unwrap();
    let left = sketch.add_point(-2_000, 800).unwrap();
    let right = sketch.add_point(2_000, 800).unwrap();
    let held = sketch.add_point(590, 790).unwrap();
    let circle = sketch.add_circle(centre, 1_000).unwrap();
    let line = sketch.add_line(left, right).unwrap();
    for point in [centre, left, right] {
        add(&mut sketch, Constraint::Fixed(point));
    }
    add(
        &mut sketch,
        Constraint::Radius {
            curve: circle,
            mm: 1_000,
        },
    );
    add(&mut sketch, Constraint::PointOnCurve(held, circle));
    add(&mut sketch, Constraint::PointOnLine(held, line));
    let solved = solve(&sketch, Budget::DEFAULT).unwrap();
    assert_eq!(solved.points[3], p(600, 800));
    assert_eq!(solved.verdict, Verdict::FullyConstrained);
}

/// A quarter arc about a fixed centre, drawn with its ends exactly on its circle.
fn quarter_arc() -> (Sketch, PointId, PointId, CurveId) {
    let mut sketch = Sketch::new();
    let centre = sketch.add_point(0, 0).unwrap();
    let start = sketch.add_point(1_000, 0).unwrap();
    let end = sketch.add_point(0, 1_000).unwrap();
    let arc = sketch.add_arc(centre, start, end).unwrap();
    add(&mut sketch, Constraint::Fixed(centre));
    (sketch, start, end, arc)
}

#[test]
fn an_arc_with_only_its_centre_held_is_under_constrained() {
    let (mut sketch, start, end, arc) = quarter_arc();
    // Each end slides round the circle and the radius can change: three freedoms, and each end
    // moves both ways, since the radius moves it outwards.
    let solved = solve(&sketch, Budget::DEFAULT).unwrap();
    assert_eq!(
        solved.verdict,
        Verdict::UnderConstrained {
            dof: 3,
            free: vec![
                FreePoint {
                    point: start,
                    dof: 2
                },
                FreePoint { point: end, dof: 2 }
            ],
        }
    );
    assert_eq!(solved.free_radii, vec![arc]);
    assert_eq!(solved.radii_mm, vec![1_000]);

    // With its radius given, only the two slides are left, and the radius is no longer free.
    add(
        &mut sketch,
        Constraint::Radius {
            curve: arc,
            mm: 1_000,
        },
    );
    let solved = solve(&sketch, Budget::DEFAULT).unwrap();
    assert_eq!(
        solved.verdict,
        Verdict::UnderConstrained {
            dof: 2,
            free: vec![
                FreePoint {
                    point: start,
                    dof: 1
                },
                FreePoint { point: end, dof: 1 }
            ],
        }
    );
    assert_eq!(solved.free_radii, Vec::new());
}

#[test]
fn a_circle_in_a_corner_with_a_contradicting_distance_is_over_constrained() {
    let (mut sketch, origin, centre, _) = circle_in_corner();
    // The centre of a 300 mm circle in the corner is 424.26 mm from it, not 500.
    let extra = add(
        &mut sketch,
        Constraint::Distance {
            a: origin,
            b: centre,
            mm: 500,
        },
    );
    let error = solve(&sketch, Budget::DEFAULT).unwrap_err();
    let SolveError::OverConstrained { conflicting } = error else {
        panic!("expected over-constrained, got {error:?}");
    };
    let radius = ConstraintId(extra.0 - 1);
    assert!(conflicting.contains(&extra), "{conflicting:?}");
    assert!(conflicting.contains(&radius), "{conflicting:?}");
}

#[test]
fn an_arc_whose_end_cannot_fall_on_whole_millimetres_is_refused() {
    // The end is held on the diagonal: at (707.107, 707.107) it is on the circle, and at
    // (707, 707) it is 0.15 mm inside it, more than the model tolerance.
    let (mut sketch, start, end, arc) = quarter_arc();
    let far = sketch.add_point(2_000, 2_000).unwrap();
    let diagonal = sketch.add_line(PointId(0), far).unwrap();
    add(&mut sketch, Constraint::Fixed(far));
    add(&mut sketch, Constraint::Fixed(start));
    add(
        &mut sketch,
        Constraint::Radius {
            curve: arc,
            mm: 1_000,
        },
    );
    add(&mut sketch, Constraint::PointOnLine(end, diagonal));
    let error = solve(&sketch, Budget::DEFAULT).unwrap_err();
    let SolveError::ArcEndRefused { curve, residual_mm } = error else {
        panic!("expected a refusal, got {error:?}");
    };
    assert_eq!(curve, arc);
    assert!(residual_mm > 0.14 && residual_mm < 0.16, "{residual_mm}");
}

#[test]
fn an_arc_end_drawn_at_its_centre_is_degenerate() {
    let mut sketch = Sketch::new();
    let centre = sketch.add_point(0, 0).unwrap();
    let start = sketch.add_point(500, 0).unwrap();
    let end = sketch.add_point(0, 0).unwrap();
    let arc = sketch.add_arc(centre, start, end).unwrap();
    assert_eq!(
        solve(&sketch, Budget::DEFAULT),
        Err(SolveError::DegenerateCurve { curve: arc })
    );
}

#[test]
fn bad_curves_are_refused_when_added() {
    let mut sketch = Sketch::new();
    let a = sketch.add_point(0, 0).unwrap();
    let b = sketch.add_point(100, 0).unwrap();
    assert_eq!(sketch.add_circle(a, 0), Err(SketchError::BadDistance));
    assert_eq!(sketch.add_arc(a, b, b), Err(SketchError::SameTwice));
    assert_eq!(sketch.add_arc(a, a, b), Err(SketchError::SameTwice));
    let circle = sketch.add_circle(a, 100).unwrap();
    assert_eq!(
        sketch.add_constraint(Constraint::PointOnCurve(a, circle)),
        Err(SketchError::SameTwice)
    );
    assert_eq!(
        sketch.add_constraint(Constraint::EqualRadius(circle, circle)),
        Err(SketchError::SameTwice)
    );
    assert_eq!(
        sketch.add_constraint(Constraint::Radius {
            curve: CurveId(7),
            mm: 10
        }),
        Err(SketchError::Unknown)
    );
    assert_eq!(
        sketch.with_positions(&[p(0, 0)], &[100]),
        Err(SketchError::WrongCount)
    );
    // An arc's radius as drawn is its start's distance from the centre, to the nearest whole
    // millimetre: 100.4987... here.
    let c = sketch.add_point(0, 10).unwrap();
    let arc = sketch.add_arc(c, b, a).unwrap();
    assert_eq!(sketch.curve(arc).unwrap().radius_mm(), 100);
}
