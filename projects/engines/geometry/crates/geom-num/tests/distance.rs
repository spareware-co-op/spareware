//! Exact distance comparisons against known answers (piece G151): each is a right triangle's or an
//! axis's distance, tried with the clearance just under, at and just over it, and each refusal.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    reason = "test checks; the rules are for kernel code"
)]

use geom_num::distance::{
    MAX_CLEARANCE, dot_from, line_line, point_line, point_plane, point_point, product_difference,
};
use geom_num::grid::{COORD_MAX, P3};
use geom_num::{NumError, Sign};

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// The signs of a comparison at clearances `gap − 1`, `gap` and `gap + 1`.
fn around(gap: u64, compare: impl Fn(u64) -> Result<Sign, NumError>) {
    assert_eq!(compare(gap - 1), Ok(Sign::Positive));
    assert_eq!(compare(gap), Ok(Sign::Zero));
    assert_eq!(compare(gap + 1), Ok(Sign::Negative));
}

#[test]
fn the_clearance_is_compared_with_each_distance() {
    around(5, |c| point_point(p(1, 2, 0), p(4, 6, 0), c));
    around(5, |c| point_line(p(0, 1, 1), p(2, 1, 1), p(1, 4, 5), c));
    around(3, |c| {
        point_plane(p(0, 0, 0), p(1, 0, 0), p(0, 1, 0), p(5, 7, 3), c)
    });
    around(3, |c| {
        line_line(p(0, 0, 0), p(1, 0, 0), p(4, 0, 3), p(4, 1, 3), c)
    });
}

#[test]
fn a_clearance_over_the_limit_is_refused() {
    let (a, b) = (p(0, 0, 0), p(1, 0, 0));
    assert_eq!(MAX_CLEARANCE, 1 << 42);
    assert_eq!(point_point(a, b, MAX_CLEARANCE), Ok(Sign::Negative));
    assert_eq!(
        point_point(a, b, MAX_CLEARANCE + 1),
        Err(NumError::OutOfRange {
            value: i128::from(MAX_CLEARANCE + 1)
        })
    );
}

#[test]
fn no_line_or_plane_means_no_distance() {
    let (a, b, c) = (p(1, 1, 1), p(2, 2, 2), p(3, 3, 3));
    assert_eq!(point_line(a, a, b, 1), Err(NumError::Degenerate));
    assert_eq!(
        point_plane(a, b, c, p(0, 1, 2), 1),
        Err(NumError::Degenerate)
    );
    assert_eq!(
        line_line(a, b, p(0, 1, 2), p(1, 2, 3), 1),
        Err(NumError::Degenerate)
    );
}

#[test]
fn the_frame_is_within_the_budgets() {
    let m = COORD_MAX;
    let (low, high) = (p(-m, -m, -m), p(m, m, m));
    assert_eq!(point_point(low, high, 1 << 40), Ok(Sign::Positive));
    assert_eq!(point_point(low, high, MAX_CLEARANCE), Ok(Sign::Negative));
    assert_eq!(point_point(low, low, 0), Ok(Sign::Zero));
    assert_eq!(
        point_line(low, high, p(m, -m, -m), 1 << 40),
        Ok(Sign::Positive)
    );
    assert_eq!(
        point_plane(low, p(m, -m, -m), p(-m, m, -m), high, 0),
        Ok(Sign::Positive)
    );
    assert_eq!(
        line_line(low, p(m, -m, -m), high, p(m, -m, m), 0),
        Ok(Sign::Positive)
    );
}

#[test]
fn products_and_dots_are_exact() {
    assert_eq!(product_difference(3, 4, 2, 6), Ok(Sign::Zero));
    assert_eq!(product_difference(3, 5, 2, 6), Ok(Sign::Positive));
    assert_eq!(product_difference(3, 3, 2, 6), Ok(Sign::Negative));
    let big = 1 << 83;
    assert_eq!(
        product_difference(big, big, big, big - 1),
        Ok(Sign::Positive)
    );
    assert_eq!(dot_from(p(1, 1, 1), p(2, 3, 4), p(0, 5, 2)), 10);
}
