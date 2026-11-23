//! A grid line's geometry: what is refused, and the pure functions on it.

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
use construct_grids::{End, FULL_TURN, GridError, LIMIT, Point, Shape};
use construct_params::{Angle, Length};

fn quadrant() -> Shape {
    Shape::arc(pt(0, 0), mm(1000), degrees(0), degrees(90)).unwrap()
}

#[test]
fn a_line_of_no_length_is_refused() {
    assert_eq!(Shape::line(pt(5, 5), pt(5, 5)), Err(GridError::ZeroLength));
    assert!(
        Shape::line(pt(5, 5), at_steps(32_001, 32_000)).is_ok(),
        "one step is a length"
    );
}

#[test]
fn an_arc_needs_a_radius_and_a_sweep() {
    let arc = |radius: Length, from: Angle, to: Angle| Shape::arc(pt(0, 0), radius, from, to);
    assert_eq!(
        arc(mm(0), degrees(0), degrees(90)),
        Err(GridError::ZeroRadius)
    );
    assert_eq!(
        arc(mm(-5), degrees(0), degrees(90)),
        Err(GridError::ZeroRadius)
    );
    assert_eq!(
        arc(mm(5), degrees(40), degrees(40)),
        Err(GridError::NoSweep)
    );
    assert_eq!(
        arc(mm(5), degrees(0), degrees(360)),
        Err(GridError::NoSweep),
        "a full turn is the same angle as none, and is refused"
    );
}

#[test]
fn an_arcs_angles_are_taken_into_zero_to_three_sixty() {
    let arc = Shape::arc(pt(0, 0), mm(5), degrees(-90), degrees(450)).unwrap();
    let Shape::Arc { from, to, .. } = arc else {
        panic!("an arc")
    };
    assert_eq!(from, degrees(270));
    assert_eq!(to, degrees(90));
    assert_eq!(FULL_TURN, 360 * Angle::UNITS_PER_DEGREE);
}

#[test]
fn the_limit_is_kept() {
    let far = Point::new(LIMIT, steps(0));
    assert!(Shape::line(pt(0, 0), far).is_ok());
    let past = Point::new(steps(LIMIT.steps() + 1), steps(0));
    assert_eq!(Shape::line(pt(0, 0), past), Err(GridError::OutOfRange));
    assert_eq!(
        Shape::arc(Point::new(LIMIT, steps(0)), mm(1), degrees(0), degrees(90)),
        Err(GridError::OutOfRange),
        "an arc's reach is inside the limit too"
    );
    let moved = Shape::line(pt(0, 0), far)
        .unwrap()
        .moved(steps(1), steps(0));
    assert_eq!(moved, Err(GridError::OutOfRange));
}

#[test]
fn a_lines_length_is_whole_number_arithmetic() {
    assert_eq!(
        Shape::line(pt(0, 0), pt(3000, 4000)).unwrap().length(),
        mm(5000)
    );
    // The nearest step: root 2 is 1, root 5 is 2, root 8 is 3.
    for (dx, dy, length) in [(1, 1, 1), (2, 1, 2), (2, 2, 3), (0, 7, 7)] {
        let line = Shape::line(pt(0, 0), at_steps(dx, dy)).unwrap();
        assert_eq!(line.length(), steps(length), "({dx}, {dy})");
    }
}

#[test]
fn an_arcs_length_is_its_radius_times_its_sweep() {
    // A quarter of a circle of 1,000 mm: 1,570.796... mm, which is 10,053,096.49 steps.
    assert_eq!(quadrant().length(), steps(10_053_096));
}

#[test]
fn an_arcs_ends_are_on_its_circle() {
    let arc = quadrant();
    assert_eq!(arc.end_point(End::Start), Some(pt(1000, 0)));
    assert_eq!(arc.end_point(End::Finish), Some(pt(0, 1000)));
    let line = Shape::line(pt(1, 2), pt(3, 4)).unwrap();
    assert_eq!(line.end_point(End::Start), Some(pt(1, 2)));
    assert_eq!(line.end_point(End::Finish), Some(pt(3, 4)));
}

#[test]
fn moving_keeps_the_shape_and_the_length() {
    let line = Shape::line(pt(0, 0), pt(3000, 4000)).unwrap();
    let moved = line.moved(mm(100), mm(-50)).unwrap();
    assert_eq!(moved, Shape::line(pt(100, -50), pt(3100, 3950)).unwrap());
    assert_eq!(moved.length(), line.length());
    let arc = quadrant().moved(mm(10), mm(20)).unwrap();
    assert_eq!(
        arc,
        Shape::arc(pt(10, 20), mm(1000), degrees(0), degrees(90)).unwrap()
    );
}

#[test]
fn an_end_is_put_where_it_is_dragged() {
    let line = Shape::line(pt(0, 0), pt(3000, 0)).unwrap();
    let dragged = line.with_end(End::Finish, pt(3000, 500)).unwrap();
    assert_eq!(dragged, Shape::line(pt(0, 0), pt(3000, 500)).unwrap());
    let other = line.with_end(End::Start, pt(-100, 0)).unwrap();
    assert_eq!(other, Shape::line(pt(-100, 0), pt(3000, 0)).unwrap());
    assert_eq!(
        line.with_end(End::Finish, pt(0, 0)),
        Err(GridError::ZeroLength)
    );
    assert!(matches!(
        quadrant().with_end(End::Start, pt(1, 1)),
        Err(GridError::WrongShape(_))
    ));
}

#[test]
fn an_arcs_end_is_put_at_an_angle() {
    let arc = quadrant().with_angle(End::Finish, degrees(180)).unwrap();
    assert_eq!(
        arc,
        Shape::arc(pt(0, 0), mm(1000), degrees(0), degrees(180)).unwrap()
    );
    assert_eq!(
        quadrant().with_angle(End::Finish, degrees(0)),
        Err(GridError::NoSweep)
    );
    let line = Shape::line(pt(0, 0), pt(1, 1)).unwrap();
    assert!(matches!(
        line.with_angle(End::Start, degrees(1)),
        Err(GridError::WrongShape(_))
    ));
}

#[test]
fn a_line_is_extended_along_itself() {
    let line = Shape::line(pt(0, 0), pt(3000, 0)).unwrap();
    assert_eq!(
        line.extended(End::Finish, mm(500)).unwrap(),
        Shape::line(pt(0, 0), pt(3500, 0)).unwrap(),
        "along x it is exact"
    );
    assert_eq!(
        line.extended(End::Start, mm(500)).unwrap(),
        Shape::line(pt(-500, 0), pt(3000, 0)).unwrap()
    );
    assert_eq!(
        line.extended(End::Finish, mm(-1000)).unwrap(),
        Shape::line(pt(0, 0), pt(2000, 0)).unwrap(),
        "a negative length shortens"
    );
    let slope = Shape::line(pt(0, 0), pt(3000, 4000)).unwrap();
    assert_eq!(
        slope.extended(End::Finish, mm(5000)).unwrap(),
        Shape::line(pt(0, 0), pt(6000, 8000)).unwrap(),
        "a 3-4-5 line extended by its own length"
    );
}

#[test]
fn a_line_cannot_be_shortened_to_nothing_or_turned_round() {
    let line = Shape::line(pt(0, 0), pt(3000, 0)).unwrap();
    assert_eq!(
        line.extended(End::Finish, mm(-3000)),
        Err(GridError::ZeroLength)
    );
    assert_eq!(
        line.extended(End::Finish, mm(-4000)),
        Err(GridError::ZeroLength)
    );
    assert!(matches!(
        quadrant().extended(End::Finish, mm(1)),
        Err(GridError::WrongShape(_))
    ));
}

#[test]
fn an_arc_is_swept_further_or_less() {
    let longer = quadrant().swept(End::Finish, degrees(90)).unwrap();
    assert_eq!(
        longer,
        Shape::arc(pt(0, 0), mm(1000), degrees(0), degrees(180)).unwrap()
    );
    let back = quadrant().swept(End::Start, degrees(30)).unwrap();
    assert_eq!(
        back,
        Shape::arc(pt(0, 0), mm(1000), degrees(330), degrees(90)).unwrap()
    );
    let shorter = quadrant().swept(End::Finish, degrees(-45)).unwrap();
    assert_eq!(
        shorter,
        Shape::arc(pt(0, 0), mm(1000), degrees(0), degrees(45)).unwrap()
    );
    assert_eq!(
        quadrant().swept(End::Finish, degrees(-90)),
        Err(GridError::NoSweep)
    );
    assert_eq!(
        quadrant().swept(End::Finish, degrees(270)),
        Err(GridError::FullTurn)
    );
    let line = Shape::line(pt(0, 0), pt(1, 1)).unwrap();
    assert!(matches!(
        line.swept(End::Finish, degrees(1)),
        Err(GridError::WrongShape(_))
    ));
}

#[test]
fn an_outline_stays_within_the_deviation_asked() {
    let line = Shape::line(pt(0, 0), pt(3000, 4000)).unwrap();
    assert_eq!(line.polyline(mm(1)), vec![pt(0, 0), pt(3000, 4000)]);
    let arc = quadrant();
    let outline = arc.polyline(mm(1));
    assert_eq!(outline.first(), Some(&pt(1000, 0)));
    assert_eq!(outline.last(), Some(&pt(0, 1000)));
    // A chord of an arc of 1,000 mm strays 1 mm at 0.0894 rad: 18 chords for a quarter circle.
    assert_eq!(outline.len(), 19);
    let radius = mm(1000).steps() as f64;
    for point in &outline {
        let at = ((point.x.steps() as f64).powi(2) + (point.y.steps() as f64).powi(2)).sqrt();
        assert!(
            (at - radius).abs() < 1.0,
            "{point:?} is on the circle to a step"
        );
    }
    // Every chord's middle is within the deviation of the circle (and 1 mm is 6,400 steps).
    for pair in outline.windows(2) {
        let mid_x = (pair[0].x.steps() + pair[1].x.steps()) as f64 / 2.0;
        let mid_y = (pair[0].y.steps() + pair[1].y.steps()) as f64 / 2.0;
        let strays = radius - (mid_x * mid_x + mid_y * mid_y).sqrt();
        assert!(
            (0.0..=6401.0).contains(&strays),
            "a chord strays {strays} steps"
        );
    }
}

#[test]
fn a_finer_outline_has_more_chords_and_never_more_than_the_most() {
    let arc = quadrant();
    assert!(arc.polyline(mm(1)).len() < arc.polyline(steps(64)).len());
    let wide = Shape::arc(pt(0, 0), mm(100_000), degrees(0), degrees(359)).unwrap();
    assert!(wide.polyline(steps(1)).len() <= 1025);
}
