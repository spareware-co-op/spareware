//! Angles, arc lengths and points on circles, worked in whole numbers to a stated bound: exact for
//! the angles that can be exact (multiples of 45 degrees), the nearest whole unit or step for the
//! rest. The reference values were worked out by other methods (pi by Gauss-Legendre, arctangents
//! by halving the argument then Taylor, sines and cosines by Taylor, in decimal at 90 digits) and
//! each is given with how far the true value is from the whole number it rounds to, so a test
//! never sits on a half.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_dimensions::trig::{FULL_TURN, HALF_TURN, arc_length, between, point_at};

const STEPS_PER_MM: i64 = 6_400;
const UNITS_PER_DEGREE: i64 = 9_000_000;

#[test]
fn right_angles_and_their_halves_are_exact() {
    // Perpendicular walls along the axes, and a 3:4:5 corner off them: both exactly 90 degrees.
    for (u, v) in [
        ([1, 0], [0, 1]),
        ([3, 4], [-4, 3]),
        ([-5, -12], [12, -5]),
        (
            [3 * STEPS_PER_MM, 4 * STEPS_PER_MM],
            [-4 * STEPS_PER_MM, 3 * STEPS_PER_MM],
        ),
    ] {
        let found = between(u, v).unwrap();
        assert_eq!(found.units, 90 * UNITS_PER_DEGREE);
        assert!(found.exact);
        assert!(found.anticlockwise);
        let back = between(v, u).unwrap();
        assert_eq!(back.units, 90 * UNITS_PER_DEGREE);
        assert!(!back.anticlockwise);
    }
    let exact = |u, v| {
        let found = between(u, v).unwrap();
        assert!(found.exact, "{u:?} {v:?}");
        found.units
    };
    assert_eq!(exact([1, 0], [1, 0]), 0);
    assert_eq!(exact([2, 3], [-4, -6]), HALF_TURN);
    assert_eq!(exact([1, 0], [7, 7]), 45 * UNITS_PER_DEGREE);
    assert_eq!(exact([1, 0], [-7, 7]), 135 * UNITS_PER_DEGREE);
    assert_eq!(exact([5, 1], [2, 3]), 45 * UNITS_PER_DEGREE);
    assert_eq!(between([0, 0], [1, 0]), None);
}

#[test]
fn other_angles_are_the_nearest_unit_and_not_exact() {
    // (u, v, the nearest whole unit to the true angle; the true one is not within 0.0002 of a
    // half, so the bound of 2^-80 cannot move it).
    for (u, v, units) in [
        // The 3:4:5 triangle's two other angles: 53.130102354...° and 36.869897645...°.
        ([1, 0], [3, 4], 478_170_921),
        ([1, 0], [4, 3], 331_829_079),
        // A slope of 2 in 1, and a line a seventh off the axis the other way.
        ([1, 0], [1, 2], 570_914_539),
        ([1, 0], [-7, 1], 1_546_829_079),
        ([1, 0], [-12, -5], 1_416_421_215),
        // The smallest turn the grid holds, and one a hair off 45 degrees.
        ([1, 0], [(1 << 40) - 1, 1], 0),
        ([(1 << 40) - 1, (1 << 40) - 2], [1, 0], 405_000_000),
    ] {
        let found = between(u, v).unwrap();
        assert_eq!(found.units, units, "{u:?} {v:?}");
        assert!(!found.exact, "{u:?} {v:?}");
    }
}

#[test]
fn an_angle_and_its_complement_make_a_right_angle_to_the_unit() {
    // Each is the nearest unit, so the two are within one unit of 90 degrees between them, and
    // with the half turn the same: a deterministic sweep of directions, all on whole steps.
    let mut seen = 0;
    for a in [1_i64, 2, 3, 7, 11, 64, 999, 6_400, 123_457, 1 << 39] {
        for b in [1_i64, 5, 13, 100, 4_097, 77_777, 1 << 38] {
            let one = between([1, 0], [a, b]).unwrap().units;
            let other = between([1, 0], [b, a]).unwrap().units;
            assert!((one + other - 90 * UNITS_PER_DEGREE).abs() <= 1, "{a} {b}");
            let behind = between([1, 0], [-a, b]).unwrap().units;
            assert!((one + behind - HALF_TURN).abs() <= 1, "{a} {b}");
            // Turning both directions by a quarter keeps the angle between them.
            assert_eq!(between([0, 1], [-b, a]).unwrap().units, one, "{a} {b}");
            seen += 1;
        }
    }
    assert_eq!(seen, 70);
}

#[test]
fn an_arcs_length_is_the_nearest_step() {
    let deg = |d: i64| d * UNITS_PER_DEGREE;
    // A quarter of a 6 m circle is 3000 pi mm: 60,318,578.949 steps.
    assert_eq!(arc_length(6_000 * STEPS_PER_MM, deg(90)), Some(60_318_579));
    // 37.5 degrees of a 5 m circle: 20,943,951.024 steps.
    assert_eq!(
        arc_length(5_000 * STEPS_PER_MM, 375 * UNITS_PER_DEGREE / 10),
        Some(20_943_951)
    );
    // Nearly a whole turn of a 12.345 m circle: 496,421,903.371 steps.
    assert_eq!(
        arc_length(12_345 * STEPS_PER_MM, FULL_TURN - 9),
        Some(496_421_903)
    );
    // The smallest: a millionth of a degree of a 1 mm circle is far under a step.
    assert_eq!(arc_length(STEPS_PER_MM, 9), Some(0));
    assert_eq!(arc_length(-1, 9), None);
}

#[test]
fn a_point_on_a_circle_is_the_nearest_step() {
    let r = |mm: i64| mm * STEPS_PER_MM;
    // 30 degrees on a 6 m circle: (5196.152 mm, 3000 mm).
    assert_eq!(
        point_at([0, 0], r(6_000), 30 * UNITS_PER_DEGREE),
        Some([33_255_376, 19_200_000])
    );
    assert_eq!(
        point_at([0, 0], r(6_000), 135 * UNITS_PER_DEGREE),
        Some([-27_152_900, 27_152_900])
    );
    assert_eq!(
        point_at([0, 0], r(2_500), 2_125 * UNITS_PER_DEGREE / 10),
        Some([-13_494_263, -8_596_794])
    );
    // The quarter turns are exact, and a point is about its centre.
    assert_eq!(
        point_at([100, -100], r(1), 90 * UNITS_PER_DEGREE),
        Some([100, -100 + r(1)])
    );
    assert_eq!(point_at([0, 0], r(1), HALF_TURN), Some([-r(1), 0]));
    assert_eq!(
        point_at([0, 0], r(1), -90 * UNITS_PER_DEGREE),
        Some([0, -r(1)])
    );
    // Near 53.13 degrees on a 7 m circle: (4200 mm, 5600 mm), the 3:4:5 point, to the step.
    assert_eq!(
        point_at([0, 0], r(7_000), 478_170_921),
        Some([26_880_000, 35_840_000])
    );
}
