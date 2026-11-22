//! Krawczyk's interior test on its own: a box of two unknowns lies strictly inside another only
//! clear of all four of its ends (chapter 12.3; the edge proof of chapter 12.8 step 5 decides each
//! of its pieces by it).
//!
//! An image `K` strictly inside its box `X` proves that `X` holds one zero and only one; an end of
//! `K` on an end of `X` proves a zero but not that it is the only one, and a `K` past an end of `X`
//! proves nothing. Each case moves one end at a time, so each of the test's four comparisons and
//! each of the three conjunctions between them is the one that decides somewhere.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::arithmetic_side_effects,
    reason = "test code: a failure should stop loudly"
)]

use geom_intersect::solve::strictly_inside;
use geom_num::interval::Interval;

fn i(a: f64, b: f64) -> Interval {
    Interval::new(a, b).unwrap()
}

/// The box X: `[0, 4]` in s and `[10, 14]` in r.
fn x() -> [Interval; 2] {
    [i(0.0, 4.0), i(10.0, 14.0)]
}

/// The next double above `v`, for a positive `v`.
fn above(v: f64) -> f64 {
    f64::from_bits(v.to_bits() + 1)
}

#[test]
fn an_image_clear_of_all_four_ends_is_inside() {
    assert!(strictly_inside([i(1.0, 3.0), i(11.0, 13.0)], x()));
    // A point in each coordinate, clear of the ends.
    assert!(strictly_inside([i(2.0, 2.0), i(12.0, 12.0)], x()));
}

#[test]
fn an_image_on_any_one_end_is_within_but_not_inside() {
    // Each end of K on the same end of X, one at a time, the other three clear.
    let on_end = [
        [i(0.0, 3.0), i(11.0, 13.0)],
        [i(1.0, 4.0), i(11.0, 13.0)],
        [i(1.0, 3.0), i(10.0, 13.0)],
        [i(1.0, 3.0), i(11.0, 14.0)],
    ];
    for k in on_end {
        assert!(!strictly_inside(k, x()), "{k:?} on an end of {:?}", x());
    }
}

#[test]
fn an_image_past_any_one_end_is_not_inside() {
    // Each end of K past the same end of X, one at a time, the other three clear: K leaves X on
    // one side of one coordinate only.
    let past_end = [
        [i(-1.0, 3.0), i(11.0, 13.0)],
        [i(1.0, 5.0), i(11.0, 13.0)],
        [i(1.0, 3.0), i(9.0, 13.0)],
        [i(1.0, 3.0), i(11.0, 15.0)],
    ];
    for k in past_end {
        assert!(!strictly_inside(k, x()), "{k:?} past an end of {:?}", x());
    }
}

#[test]
fn an_image_outside_in_one_coordinate_is_not_inside_whatever_the_other() {
    // Wholly past X in s and inside it in r, and the other way round.
    assert!(!strictly_inside([i(5.0, 6.0), i(11.0, 13.0)], x()));
    assert!(!strictly_inside([i(1.0, 3.0), i(15.0, 16.0)], x()));
    // Past both ends of s, inside in r.
    assert!(!strictly_inside([i(-1.0, 5.0), i(11.0, 13.0)], x()));
}

#[test]
fn the_test_reads_the_last_bit() {
    // X's upper ends a double's spacing above K's: inside where every end is a spacing clear, not
    // where one lower end meets X's.
    let tight = [i(1.0, above(3.0)), i(11.0, above(13.0))];
    assert!(strictly_inside(
        [i(above(1.0), 3.0), i(above(11.0), 13.0)],
        tight
    ));
    assert!(!strictly_inside([i(1.0, 3.0), i(above(11.0), 13.0)], tight));
    assert!(!strictly_inside([i(above(1.0), 3.0), i(11.0, 13.0)], tight));
}
