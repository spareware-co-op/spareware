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
    SketchError, SolveError, Touch, Verdict, mm, solve,
};

fn p(x_mm: i32, y_mm: i32) -> Point {
    Point::mm(x_mm, y_mm)
}

fn add(sketch: &mut Sketch, constraint: Constraint) -> ConstraintId {
    sketch.add_constraint(constraint).unwrap()
}

/// A slot: two straight sides and two half-circle ends, drawn roughly. The arcs share their ends
/// with the lines, so each tangency is at a shared point.
/// Points: left centre 0, right centre 1, top-left 2, top-right 3, bottom-right 4, bottom-left 5.
fn slot() -> Sketch {
    let mut sketch = Sketch::new();
    let left_centre = sketch.add_point(mm(0), mm(0)).unwrap();
    let right_centre = sketch.add_point(mm(2_950), mm(30)).unwrap();
    let top_left = sketch.add_point(mm(10), mm(510)).unwrap();
    let top_right = sketch.add_point(mm(2_990), mm(495)).unwrap();
    let bottom_right = sketch.add_point(mm(3_010), mm(-490)).unwrap();
    let bottom_left = sketch.add_point(mm(-5), mm(-505)).unwrap();
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
            steps: mm(500),
        },
    );
    add(
        &mut sketch,
        Constraint::Distance {
            a: left_centre,
            b: right_centre,
            steps: mm(3_000),
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
    assert_eq!(solved.radii, vec![mm(500), mm(500)]);
    assert_eq!(solved.verdict, Verdict::FullyConstrained);
    assert_eq!(solved.redundant, Vec::new());
    assert_eq!(solved.free_radii, Vec::new());
}

#[test]
fn the_same_slot_twice_gives_the_same_bytes() {
    let once = serde_json::to_vec(&solve(&slot(), Budget::DEFAULT).unwrap()).unwrap();
    let twice = serde_json::to_vec(&solve(&slot().clone(), Budget::DEFAULT).unwrap()).unwrap();
    assert_eq!(once, twice);
    // Pinned, so a machine that solves it differently, even by one step, fails here. Pinned
    // again when lengths moved to grid steps (piece A39): the same shape, its numbers 6,400
    // times larger, in the same 8 steps.
    assert_eq!(String::from_utf8(once).unwrap(), GOLDEN_SLOT);
}

const GOLDEN_SLOT: &str = concat!(
    r#"{"points":[{"x":0,"y":0},{"x":19200000,"y":0},{"x":0,"y":3200000},"#,
    r#"{"x":19200000,"y":3200000},{"x":19200000,"y":-3200000},{"x":0,"y":-3200000}],"#,
    r#""verdict":"FullyConstrained","redundant":[],"steps":8,"radii":[3200000,3200000]}"#,
);

/// Two fixed lines meeting at a right angle at the origin, along the plane's two axes.
fn corner(sketch: &mut Sketch) -> (PointId, [construct_solver::LineId; 2]) {
    let origin = sketch.add_point(mm(0), mm(0)).unwrap();
    let along_x = sketch.add_point(mm(2_000), mm(0)).unwrap();
    let along_y = sketch.add_point(mm(0), mm(2_000)).unwrap();
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
    let centre = sketch.add_point(mm(280), mm(330)).unwrap();
    let circle = sketch.add_circle(centre, mm(310)).unwrap();
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
            steps: mm(300),
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
    assert_eq!(solved.radii, vec![mm(300)]);
    assert_eq!(solved.verdict, Verdict::FullyConstrained);
}

#[test]
fn concentric_circles_with_a_radius_and_a_diameter() {
    let mut sketch = Sketch::new();
    let a_centre = sketch.add_point(mm(1_000), mm(1_000)).unwrap();
    let b_centre = sketch.add_point(mm(1_020), mm(990)).unwrap();
    let a = sketch.add_circle(a_centre, mm(480)).unwrap();
    let b = sketch.add_circle(b_centre, mm(790)).unwrap();
    add(&mut sketch, Constraint::Fixed(a_centre));
    add(&mut sketch, Constraint::Concentric(a, b));
    add(
        &mut sketch,
        Constraint::Radius {
            curve: a,
            steps: mm(500),
        },
    );
    add(
        &mut sketch,
        Constraint::Diameter {
            curve: b,
            steps: mm(1_600),
        },
    );
    let solved = solve(&sketch, Budget::DEFAULT).unwrap();
    assert_eq!(solved.points, vec![p(1_000, 1_000), p(1_000, 1_000)]);
    assert_eq!(solved.radii, vec![mm(500), mm(800)]);
    assert_eq!(solved.verdict, Verdict::FullyConstrained);

    // An odd diameter in millimetres is a whole radius of grid steps: 1,601 mm is 800.5 mm.
    let odd = |diameter: i64| {
        let mut odd = Sketch::new();
        let c = odd.add_point(mm(0), mm(0)).unwrap();
        let circle = odd.add_circle(c, mm(790)).unwrap();
        add(&mut odd, Constraint::Fixed(c));
        add(
            &mut odd,
            Constraint::Diameter {
                curve: circle,
                steps: diameter,
            },
        );
        solve(&odd, Budget::DEFAULT).unwrap()
    };
    let solved = odd(mm(1_601));
    assert_eq!(solved.radii, vec![5_123_200]);
    assert_eq!(solved.verdict, Verdict::FullyConstrained);
    // One step more is half a step of radius off the grid, 78 nm: within the tolerance.
    let solved = odd(mm(1_601) + 1);
    assert!(
        solved.radii == vec![5_123_200] || solved.radii == vec![5_123_201],
        "{:?}",
        solved.radii
    );
}

#[test]
fn curves_tangent_outside_and_inside() {
    for (touch, drawn, radius, centre_at) in [
        (Touch::Outside, (1_250, 20), 750, 1_250),
        (Touch::FirstInside, (690, 10), 300, 700),
    ] {
        let mut sketch = Sketch::new();
        let fixed_centre = sketch.add_point(mm(0), mm(0)).unwrap();
        let moving_centre = sketch.add_point(mm(drawn.0), mm(drawn.1)).unwrap();
        let fixed_radius = if touch == Touch::Outside { 500 } else { 1_000 };
        let fixed_curve = sketch.add_circle(fixed_centre, mm(fixed_radius)).unwrap();
        let moving = sketch.add_circle(moving_centre, mm(radius + 20)).unwrap();
        let between = sketch.add_line(fixed_centre, moving_centre).unwrap();
        add(&mut sketch, Constraint::Fixed(fixed_centre));
        add(
            &mut sketch,
            Constraint::Radius {
                curve: fixed_curve,
                steps: mm(fixed_radius),
            },
        );
        add(
            &mut sketch,
            Constraint::Radius {
                curve: moving,
                steps: mm(radius),
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
    let centre = sketch.add_point(mm(0), mm(0)).unwrap();
    let left = sketch.add_point(mm(-2_000), mm(800)).unwrap();
    let right = sketch.add_point(mm(2_000), mm(800)).unwrap();
    let held = sketch.add_point(mm(590), mm(790)).unwrap();
    let circle = sketch.add_circle(centre, mm(1_000)).unwrap();
    let line = sketch.add_line(left, right).unwrap();
    for point in [centre, left, right] {
        add(&mut sketch, Constraint::Fixed(point));
    }
    add(
        &mut sketch,
        Constraint::Radius {
            curve: circle,
            steps: mm(1_000),
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
    let centre = sketch.add_point(mm(0), mm(0)).unwrap();
    let start = sketch.add_point(mm(1_000), mm(0)).unwrap();
    let end = sketch.add_point(mm(0), mm(1_000)).unwrap();
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
    assert_eq!(solved.radii, vec![mm(1_000)]);

    // With its radius given, only the two slides are left, and the radius is no longer free.
    add(
        &mut sketch,
        Constraint::Radius {
            curve: arc,
            steps: mm(1_000),
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
            steps: mm(500),
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
fn an_arc_ended_at_45_degrees_is_solved_on_the_grid() {
    // The end is held on the diagonal at (707.1068, 707.1068) mm. On whole millimetres it fell
    // 0.15 mm inside the circle and was refused; on the grid it is at most half a step off.
    let (mut sketch, start, end, arc) = quarter_arc();
    let far = sketch.add_point(mm(2_000), mm(2_000)).unwrap();
    let diagonal = sketch.add_line(PointId(0), far).unwrap();
    add(&mut sketch, Constraint::Fixed(far));
    add(&mut sketch, Constraint::Fixed(start));
    add(
        &mut sketch,
        Constraint::Radius {
            curve: arc,
            steps: mm(1_000),
        },
    );
    add(&mut sketch, Constraint::PointOnLine(end, diagonal));
    let solved = solve(&sketch, Budget::DEFAULT).unwrap();
    // 1,000 / sqrt(2) mm is 4,525,483.40 steps.
    assert_eq!(
        solved.points[end.0 as usize],
        Point {
            x: 4_525_483,
            y: 4_525_483
        }
    );
    assert_eq!(solved.radii, vec![mm(1_000)]);
    assert_eq!(solved.verdict, Verdict::FullyConstrained);
}

#[test]
fn a_circle_tangent_to_a_sloped_line_is_solved_on_the_grid() {
    // A 500 mm circle tangent to a line rising 1 in 2, its centre on an upright through the
    // line's start: the centre is 500 x sqrt(5) / 2 = 559.017 mm up, between whole millimetres.
    let mut sketch = Sketch::new();
    let origin = sketch.add_point(mm(0), mm(0)).unwrap();
    let up_slope = sketch.add_point(mm(4_000), mm(2_000)).unwrap();
    let up = sketch.add_point(mm(0), mm(3_000)).unwrap();
    for point in [origin, up_slope, up] {
        add(&mut sketch, Constraint::Fixed(point));
    }
    let slope = sketch.add_line(origin, up_slope).unwrap();
    let upright = sketch.add_line(origin, up).unwrap();
    let centre = sketch.add_point(mm(50), mm(600)).unwrap();
    let circle = sketch.add_circle(centre, mm(480)).unwrap();
    add(&mut sketch, Constraint::PointOnLine(centre, upright));
    let side = sketch.side_of(centre, slope).unwrap();
    assert_eq!(side, Side::Left);
    add(
        &mut sketch,
        Constraint::TangentLine {
            line: slope,
            curve: circle,
            side,
        },
    );
    add(
        &mut sketch,
        Constraint::Radius {
            curve: circle,
            steps: mm(500),
        },
    );
    let solved = solve(&sketch, Budget::DEFAULT).unwrap();
    // 559.0169943... mm is 3,577,708.76 steps.
    assert_eq!(
        solved.points[centre.0 as usize],
        Point { x: 0, y: 3_577_709 }
    );
    assert_eq!(solved.radii, vec![mm(500)]);
    assert_eq!(solved.verdict, Verdict::FullyConstrained);
}

#[test]
fn an_arc_end_drawn_at_its_centre_is_degenerate() {
    let mut sketch = Sketch::new();
    let centre = sketch.add_point(mm(0), mm(0)).unwrap();
    let start = sketch.add_point(mm(500), mm(0)).unwrap();
    let end = sketch.add_point(mm(0), mm(0)).unwrap();
    let arc = sketch.add_arc(centre, start, end).unwrap();
    assert_eq!(
        solve(&sketch, Budget::DEFAULT),
        Err(SolveError::DegenerateCurve { curve: arc })
    );
}

#[test]
fn bad_curves_are_refused_when_added() {
    let mut sketch = Sketch::new();
    let a = sketch.add_point(mm(0), mm(0)).unwrap();
    let b = sketch.add_point(mm(100), mm(0)).unwrap();
    assert_eq!(sketch.add_circle(a, mm(0)), Err(SketchError::BadDistance));
    assert_eq!(sketch.add_arc(a, b, b), Err(SketchError::SameTwice));
    assert_eq!(sketch.add_arc(a, a, b), Err(SketchError::SameTwice));
    let circle = sketch.add_circle(a, mm(100)).unwrap();
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
            steps: mm(10)
        }),
        Err(SketchError::Unknown)
    );
    assert_eq!(
        sketch.with_positions(&[p(0, 0)], &[100]),
        Err(SketchError::WrongCount)
    );
    // An arc's radius as drawn is its start's distance from the centre, to the nearest grid
    // step: 100.4987... mm, 643,192.04 steps, here.
    let c = sketch.add_point(mm(0), mm(10)).unwrap();
    let arc = sketch.add_arc(c, b, a).unwrap();
    assert_eq!(sketch.curve(arc).unwrap().radius(), 643_192);
}
