//! What the tests share: lengths, points, angles and IDs.

#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::must_use_candidate,
    missing_docs,
    reason = "test code; each test file uses its own part"
)]

use construct_grids::Point;
use construct_params::{Angle, Length};
use kit_core::Id;

pub fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

pub fn mm(value: i32) -> Length {
    Length::from_mm(value)
}

pub fn steps(value: i64) -> Length {
    Length::from_steps(value)
}

pub fn pt(x: i32, y: i32) -> Point {
    Point::mm(x, y)
}

pub fn at_steps(x: i64, y: i64) -> Point {
    Point::new(steps(x), steps(y))
}

pub fn degrees(value: i64) -> Angle {
    Angle::from_units(value * Angle::UNITS_PER_DEGREE)
}
