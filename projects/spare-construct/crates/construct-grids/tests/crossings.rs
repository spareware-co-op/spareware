//! Where grid lines cross, and the nearest point of one to another.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{at_steps, degrees, mm, pt, steps};
use construct_grids::{Crossing, Shape, crossings, nearest_on};

/// There is no crossing.
fn nothing(found: &[Crossing]) {
    assert_eq!(found.len(), 0, "{found:?}");
}

fn line(a: (i32, i32), b: (i32, i32)) -> Shape {
    Shape::line(pt(a.0, a.1), pt(b.0, b.1)).unwrap()
}

fn circle(cx: i32, cy: i32, radius: i32, from: i64, to: i64) -> Shape {
    Shape::arc(pt(cx, cy), mm(radius), degrees(from), degrees(to)).unwrap()
}

#[test]
fn two_straight_lines_cross_exactly_where_they_cross() {
    let found = crossings(&line((0, 0), (1000, 1000)), &line((0, 1000), (1000, 0)));
    assert_eq!(
        found,
        vec![Crossing {
            point: pt(500, 500),
            exact: true,
            bound: steps(0),
        }]
    );
    // The same answer whichever is asked first.
    let again = crossings(&line((0, 1000), (1000, 0)), &line((0, 0), (1000, 1000)));
    assert_eq!(again, found);
}

#[test]
fn a_crossing_between_steps_is_the_nearest_step_with_a_bound() {
    // A runs along x from 0 to 10 steps; B has the direction (2, 3) from (3, -1): they cross at
    // x = 3 and 2/3 steps, y = 0.
    let a = Shape::line(at_steps(0, 0), at_steps(10, 0)).unwrap();
    let b = Shape::line(at_steps(3, -1), at_steps(5, 2)).unwrap();
    let found = crossings(&a, &b);
    assert_eq!(
        found,
        vec![Crossing {
            point: at_steps(4, 0),
            exact: false,
            bound: steps(1),
        }]
    );
}

#[test]
fn a_crossing_at_a_segments_end_counts() {
    let found = crossings(&line((0, 0), (1000, 0)), &line((1000, -500), (1000, 500)));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].point, pt(1000, 0));
    assert!(found[0].exact);
}

#[test]
fn parallel_lines_and_lines_apart_do_not_cross() {
    nothing(&crossings(
        &line((0, 0), (1000, 0)),
        &line((0, 100), (1000, 100)),
    ));
    // Lying along each other, overlapping or not.
    nothing(&crossings(
        &line((0, 0), (1000, 0)),
        &line((500, 0), (1500, 0)),
    ));
    // Their lines would cross; the segments do not reach.
    nothing(&crossings(
        &line((0, 0), (1000, 0)),
        &line((2000, -500), (2000, 500)),
    ));
    nothing(&crossings(
        &line((0, 0), (1000, 0)),
        &line((500, 100), (500, 900)),
    ));
}

#[test]
fn a_line_crosses_an_arc_where_it_meets_it() {
    // A line at y = 500 mm through a circle of 1,000 mm: x = +-866.0254 mm, which is
    // +-5,542,562.58 steps.
    let across = line((-2000, 500), (2000, 500));
    let upper = circle(0, 0, 1000, 0, 180);
    let found = crossings(&across, &upper);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].point, at_steps(-5_542_563, 3_200_000));
    assert_eq!(found[1].point, at_steps(5_542_563, 3_200_000));
    for crossing in &found {
        assert!(!crossing.exact);
        // The curves meet at 60 degrees (the sine is 0.866): the bound is 1 + ceil(0.01 / 0.866).
        assert_eq!(crossing.bound, steps(2));
    }
    assert_eq!(crossings(&upper, &across), found, "either way round");
    // Only the part of the circle that is on the arc counts.
    let right = circle(0, 0, 1000, 0, 90);
    let one = crossings(&across, &right);
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].point, at_steps(5_542_563, 3_200_000));
}

#[test]
fn a_line_that_misses_an_arc_or_only_touches_its_circle() {
    let upper = circle(0, 0, 1000, 0, 180);
    nothing(&crossings(&line((-2000, 1500), (2000, 1500)), &upper)); // above it
    nothing(&crossings(
        &line((-500, 500), (500, 500)),
        &circle(0, 0, 1000, 0, 180),
    )); // inside it
    // A crossing on the circle but off the arc.
    nothing(&crossings(&line((-2000, -500), (2000, -500)), &upper));
    // Touching: one point, and a bound that says the point cannot be trusted along the line.
    let touch = crossings(&line((-2000, 1000), (2000, 1000)), &upper);
    assert_eq!(touch.len(), 1);
    assert_eq!(touch[0].point, pt(0, 1000));
    assert!(touch[0].bound.steps() > 1000);
}

#[test]
fn two_arcs_cross_where_their_circles_do() {
    // Two circles of 1,000 mm with centres 1,000 mm apart meet at x = 500 mm, y = +-866.0254 mm.
    let first = circle(0, 0, 1000, 0, 350);
    let second = circle(1000, 0, 1000, 0, 350);
    let found = crossings(&first, &second);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].point, at_steps(3_200_000, -5_542_563));
    assert_eq!(found[1].point, at_steps(3_200_000, 5_542_563));
    assert_eq!(found[0].bound, steps(2));
    assert_eq!(crossings(&second, &first), found);
    // Short arcs that do not reach the crossings.
    let near = circle(0, 0, 1000, 0, 30);
    nothing(&crossings(&near, &second));
}

#[test]
fn arcs_that_are_apart_inside_each_other_or_concentric_do_not_cross() {
    let first = circle(0, 0, 1000, 0, 350);
    nothing(&crossings(&first, &circle(3000, 0, 1000, 0, 350))); // apart
    nothing(&crossings(&first, &circle(100, 0, 300, 0, 350))); // inside
    nothing(&crossings(&first, &circle(0, 0, 500, 0, 350))); // concentric
    nothing(&crossings(&first, &circle(0, 0, 1000, 0, 350))); // the same circle
}

#[test]
fn arcs_that_touch_meet_in_one_point() {
    let first = circle(0, 0, 1000, 0, 350);
    let found = crossings(&first, &circle(2000, 0, 1000, 0, 350));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].point, pt(1000, 0));
}

#[test]
fn the_nearest_point_of_a_line_is_the_foot_or_the_nearer_end() {
    let along = line((0, 0), (10_000, 0));
    let foot = nearest_on(&along, pt(3000, 700)).unwrap();
    assert_eq!(foot.point, pt(3000, 0));
    assert_eq!(foot.distance, mm(700));
    // Past the end: the end, 3-4-5 away.
    let beyond = nearest_on(&along, pt(13_000, 4000)).unwrap();
    assert_eq!(beyond.point, pt(10_000, 0));
    assert_eq!(beyond.distance, mm(5000));
    // On the line: no distance.
    assert_eq!(nearest_on(&along, pt(42, 0)).unwrap().distance, steps(0));
    // A foot between steps is the nearest step, a half rounding up: the line (0,0)-(10,10) in
    // steps and the point (5, 0), whose foot is (2.5, 2.5).
    let slope = Shape::line(at_steps(0, 0), at_steps(10, 10)).unwrap();
    let rounded = nearest_on(&slope, at_steps(5, 0)).unwrap();
    assert_eq!(rounded.point, at_steps(3, 3));
}

#[test]
fn the_nearest_point_of_an_arc_is_on_its_circle_or_at_an_end() {
    let quadrant = circle(0, 0, 1000, 0, 90);
    let near = nearest_on(&quadrant, pt(2000, 2000)).unwrap();
    // 707.1068 mm each way: 4,525,483.4 steps.
    assert_eq!(near.point, at_steps(4_525_483, 4_525_483));
    let expect = (2_828.427_124_746_19 - 1000.0) * 6400.0;
    assert!((near.distance.steps() as f64 - expect).abs() <= 2.0);
    // At 225 degrees, off the arc: both ends are as near; the start is taken.
    let off = nearest_on(&quadrant, pt(-1000, -1000)).unwrap();
    assert_eq!(off.point, pt(1000, 0));
    // At the centre every point is as near: the start.
    assert_eq!(nearest_on(&quadrant, pt(0, 0)).unwrap().point, pt(1000, 0));
}
