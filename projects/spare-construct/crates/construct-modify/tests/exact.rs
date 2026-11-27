//! The motions: which are exact, the turn by any angle and its bound (each point within half a
//! step of the exact turn, rounded once), and the algebra the modify tools and groups build on
//! (composition, a mirror twice, a turn and its way back), each on our own types.
//!
//! The pinned points were worked out apart from this crate, with Python's decimal module at 90
//! digits (pi and the cosine and sine by the recipes in its documentation), each the nearest step
//! to the exact turn, a half away from zero; none is within 0.04 of a step of a tie.

// The crate denies these outside tests (Cargo.toml); a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_modify::exact::{Angles, FULL, LIMIT, QUARTER};
use construct_modify::trig::{PRECISION, half_tangent};
use construct_modify::{ModifyError, Motion, Square, Turn};
use construct_params::{Angle, Length};
use num_bigint::BigInt;
use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};

/// Steps in a millimetre.
const MM: i64 = Length::STEPS_PER_MM;

fn degrees(d: i64) -> Angle {
    Angle::from_units(d * Angle::UNITS_PER_DEGREE)
}

/// An angle of `tenths` tenths of a degree.
fn tenths(tenths: i64) -> Angle {
    Angle::from_units(tenths * Angle::UNITS_PER_DEGREE / 10)
}

/// A runner with a fixed seed, so a run is the same on every machine.
fn runner() -> TestRunner {
    let seed = [7_u8; 32];
    TestRunner::new_with_rng(
        Config {
            cases: 256,
            failure_persistence: None,
            ..Config::default()
        },
        TestRng::from_seed(RngAlgorithm::ChaCha, &seed),
    )
}

fn turned(angle: Angle, centre: [i64; 2], p: [i64; 2]) -> [i64; 2] {
    Motion::turn_about(centre, &Turn::of_angle(angle))
        .apply(p)
        .unwrap()
}

#[test]
fn quarter_turns_land_every_point_exactly_and_four_come_round() {
    let centre = [1_234 * MM, -567 * MM];
    let quarter = Motion::turn_about(centre, &Turn::of_angle(degrees(90)));
    assert_eq!(
        quarter.apply([centre[0] + 3, centre[1]]),
        Some([centre[0], centre[1] + 3])
    );
    assert_eq!(quarter.apply(centre), Some(centre));
    assert!(quarter.times_over(4).is_identity());
    for k in -5..=5_i64 {
        let turn = Turn::of_angle(degrees(90 * k));
        assert_eq!(turn.quarter_turns(), Some(k.rem_euclid(4) as u8), "{k}");
        assert_eq!(turn, Turn::quarters(k), "{k}");
    }
}

/// Every angle a person can type is a turn: a whole number of quarter turns exactly, any other as
/// the true rotation nearest it (its cosine and sine fractions whose squares add to exactly one).
#[test]
fn every_angle_is_a_turn_and_only_the_quarters_are_quarter_turns() {
    runner()
        .run(&(-4 * QUARTER..4 * QUARTER), |units| {
            let turn = Turn::of_angle(Angle::from_units(units));
            prop_assert_eq!(turn.quarter_turns().is_some(), units % QUARTER == 0);
            let (c, s, h) = turn.parts();
            prop_assert_eq!(c * c + s * s, h * h);
            prop_assert_eq!(
                turn.angle(),
                Some(Angle::from_units(units.rem_euclid(FULL)))
            );
            Ok(())
        })
        .unwrap();
}

/// A wall 10 m along x from the origin, turned about its start; and points far out, turned about
/// a point that is not on a millimetre. Each is the nearest step to the exact turn.
#[test]
fn any_angle_turns_each_point_to_the_nearest_step() {
    let ten_m = [10_000 * MM, 0];
    assert_eq!(turned(tenths(375), [0, 0], ten_m), [50_774_614, 38_960_731]);
    assert_eq!(turned(degrees(15), [0, 0], ten_m), [61_819_253, 16_564_419]);
    assert_eq!(
        turned(tenths(-375), [0, 0], ten_m),
        [50_774_614, -38_960_731]
    );
    // One angle unit (1/9,000,000 of a degree) moves the end of a 10 m wall 0.12 of a step: it
    // stays where it is.
    assert_eq!(turned(Angle::from_units(1), [0, 0], ten_m), ten_m);
    assert_eq!(turned(Angle::from_units(FULL - 1), [0, 0], ten_m), ten_m);
    // 123.456789 degrees is 1,111,111,101 angle units.
    assert_eq!(
        turned(
            Angle::from_units(1_111_111_101),
            [1_000 * MM, -2_000 * MM],
            [3_000 * MM, 4_000 * MM]
        ),
        [-32_693_932, -23_291_161]
    );
    // 100 km out, the turn is still well past the grid: each step is the nearest.
    assert_eq!(
        turned(
            tenths(375),
            [12_345 * MM / 10, -2_000 * MM],
            [100_000_000 * MM, 37_000_000 * MM]
        ),
        [363_585_271_925, 577_465_930_767]
    );
}

/// Whether `a sqrt(r) <= b`, in whole numbers.
fn below_root(a: i128, r: i128, b: i128) -> bool {
    match (a <= 0, b >= 0) {
        (true, true) => true,
        (false, false) => false,
        (false, true) => a * a * r <= b * b,
        (true, false) => a * a * r >= b * b,
    }
}

/// Whether `a sqrt(r) >= b`, in whole numbers.
fn above_root(a: i128, r: i128, b: i128) -> bool {
    below_root(-a, r, -b)
}

/// The bound, proved on whole numbers where the exact turn is known in roots: at 45 degrees a
/// turn takes `(dx, dy)` to `((dx - dy) / sqrt 2, (dx + dy) / sqrt 2)`, at 30 degrees to
/// `((dx sqrt 3 - dy) / 2, (dx + dy sqrt 3) / 2)`, at 60 to `((dx - dy sqrt 3) / 2,
/// (dx sqrt 3 + dy) / 2)`. Every coordinate given is within half a step of these, checked by
/// squaring (a tie, exactly half a step, may go either way).
#[test]
fn each_point_is_within_half_a_step_of_the_exact_turn() {
    let near = 1_i64 << 38;
    runner()
        .run(
            &((-near..near, -near..near), (-near..near, -near..near)),
            |((cx, cy), (px, py))| {
                let (dx, dy) = (i128::from(px - cx), i128::from(py - cy));
                let off = |q: [i64; 2]| (i128::from(q[0] - cx), i128::from(q[1] - cy));

                let (kx, ky) = off(turned(degrees(45), [cx, cy], [px, py]));
                // |(dx - dy) / sqrt 2 - kx| <= 1/2: (2kx - 1) sqrt 2 <= 2 (dx - dy) <= (2kx + 1) sqrt 2.
                prop_assert!(below_root(2 * kx - 1, 2, 2 * (dx - dy)));
                prop_assert!(above_root(2 * kx + 1, 2, 2 * (dx - dy)));
                prop_assert!(below_root(2 * ky - 1, 2, 2 * (dx + dy)));
                prop_assert!(above_root(2 * ky + 1, 2, 2 * (dx + dy)));

                let (kx, ky) = off(turned(degrees(30), [cx, cy], [px, py]));
                // |dx sqrt 3 - dy - 2kx| <= 1 and |dx + dy sqrt 3 - 2ky| <= 1.
                prop_assert!(above_root(dx, 3, 2 * kx + dy - 1));
                prop_assert!(below_root(dx, 3, 2 * kx + dy + 1));
                prop_assert!(above_root(dy, 3, 2 * ky - dx - 1));
                prop_assert!(below_root(dy, 3, 2 * ky - dx + 1));

                let (kx, ky) = off(turned(degrees(60), [cx, cy], [px, py]));
                // |dx - dy sqrt 3 - 2kx| <= 1 and |dx sqrt 3 + dy - 2ky| <= 1.
                prop_assert!(above_root(dy, 3, dx - 2 * kx - 1));
                prop_assert!(below_root(dy, 3, dx - 2 * kx + 1));
                prop_assert!(above_root(dx, 3, 2 * ky - dy - 1));
                prop_assert!(below_root(dx, 3, 2 * ky - dy + 1));
                Ok(())
            },
        )
        .unwrap();
}

/// The half-tangent the turn is made from is within half its last bit of the true one:
/// tan(22.5 degrees) is sqrt 2 - 1 and tan(15 degrees) is 2 - sqrt 3, checked by squaring.
#[test]
fn the_half_tangent_is_within_half_its_last_bit() {
    let one = BigInt::from(1) << PRECISION;
    // n = (sqrt 2 - 1) 2^P within 1/2: (2n + 2^(P+1) - 1)^2 <= 8 4^P <= (2n + 2^(P+1) + 1)^2.
    let n = half_tangent(405_000_000, 1);
    let twice = &n * 2 + &one * 2;
    let root_two = &one * &one * 8;
    assert!((&twice - 1) * (&twice - 1) <= root_two);
    assert!(root_two <= (&twice + 1) * (&twice + 1));
    // n = (2 - sqrt 3) 2^P within 1/2: (2^(P+2) - 2n - 1)^2 <= 12 4^P <= (2^(P+2) - 2n + 1)^2.
    let n = half_tangent(270_000_000, 1);
    let twice = &one * 4 - &n * 2;
    let root_three = &one * &one * 12;
    assert!((&twice - 1) * (&twice - 1) <= root_three);
    assert!(root_three <= (&twice + 1) * (&twice + 1));
    assert_eq!(half_tangent(0, 1), BigInt::from(0));
}

/// A turn by an angle and the turn back are exactly no turn: the two are worked out alike. Points
/// rounded between them were rounded, though: back by the same angle, a point may be a step from
/// where it began. The point 7,976 steps along x is one: turned 37.5 degrees it is at (6,328,
/// 4,855), and turned back from there at (7,976, -1).
#[test]
fn a_turn_and_its_way_back_are_no_turn_but_the_points_were_rounded() {
    runner()
        .run(&(-4 * FULL..4 * FULL), |units| {
            let there = Turn::of_angle(Angle::from_units(units));
            let back = Turn::of_angle(Angle::from_units(-units));
            prop_assert_eq!(there.then(&back), Turn::none());
            Ok(())
        })
        .unwrap();
    let out = turned(tenths(375), [0, 0], [7_976, 0]);
    assert_eq!(out, [6_328, 4_855]);
    assert_eq!(turned(tenths(-375), [0, 0], out), [7_976, -1]);
    // Held as one motion, there and back is no motion, so nothing was rounded between.
    let there = Motion::turn_about([0, 0], &Turn::of_angle(tenths(375)));
    let back = Motion::turn_about([0, 0], &Turn::of_angle(tenths(-375)));
    assert!(there.then(&back).is_identity());
}

/// Two turns in turn are the turn by their sum, to within the rounding: a step at most.
#[test]
fn turns_add_within_a_step() {
    let near = 1_i64 << 38;
    runner()
        .run(
            &(
                -FULL..FULL,
                -FULL..FULL,
                (-near..near, -near..near),
                (-near..near, -near..near),
            ),
            |(a, b, (cx, cy), (px, py))| {
                let both = Turn::of_angle(Angle::from_units(a))
                    .then(&Turn::of_angle(Angle::from_units(b)));
                let sum = Turn::of_angle(Angle::from_units(a + b));
                prop_assert_eq!(both.angle(), sum.angle());
                let p = Motion::turn_about([cx, cy], &both).apply([px, py]).unwrap();
                let q = Motion::turn_about([cx, cy], &sum).apply([px, py]).unwrap();
                prop_assert!((p[0] - q[0]).abs() <= 1 && (p[1] - q[1]).abs() <= 1);
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn a_turn_from_one_direction_to_another_is_exact_when_its_lengths_are_whole() {
    // A wall 3 m by 4 m off the x axis, turned onto it about its start: 5 m along x.
    let turn = Turn::between([3, 4], [1, 0]).unwrap();
    let five = |v: i64| BigInt::from(v);
    assert_eq!(turn.parts(), (&five(3), &five(-4), &five(5)));
    assert_eq!(turn.angle(), None, "no whole number of angle units");
    let onto = Motion::turn_about([0, 0], &turn);
    assert_eq!(onto.apply([3_000 * MM, 4_000 * MM]), Some([5_000 * MM, 0]));
    // A point the hypotenuse does not divide is rounded once: (0.6, -0.8) steps is (1, -1).
    assert_eq!(onto.apply([1, 0]), Some([1, -1]));
    // The lengths need not be whole one by one: the square roots of 2 multiply to 2.
    assert_eq!(
        Turn::between([1, 1], [-1, 1]).unwrap().quarter_turns(),
        Some(1)
    );
    assert_eq!(
        Turn::between([0, 0], [1, 0]).unwrap_err(),
        ModifyError::NoDirection
    );
    // Lengths in steps reduce first: 3 m by 4 m is 3 to 4.
    assert_eq!(
        Turn::between([3_000 * MM, 4_000 * MM], [7 * MM, 0]).unwrap(),
        turn
    );
}

/// Directions whose lengths do not multiply to a whole number turn too: (1, 2) onto (1, 0) takes
/// 10 m along x to (10 / sqrt 5, -20 / sqrt 5) m; past a quarter turn either way, 135 degrees.
#[test]
fn a_turn_between_any_two_directions_is_a_turn() {
    let ten_m = [10_000 * MM, 0];
    let apply = |from, to| {
        Motion::turn_about([0, 0], &Turn::between(from, to).unwrap())
            .apply(ten_m)
            .unwrap()
    };
    assert_eq!(apply([1, 2], [1, 0]), [28_621_670, -57_243_340]);
    // 10 m / sqrt 2 is 45,254,833.996 steps.
    assert_eq!(apply([1, 0], [-1, 1]), [-45_254_834, 45_254_834]);
    assert_eq!(apply([1, 0], [-1, -1]), [-45_254_834, -45_254_834]);
    assert_eq!(apply([1, 0], [1, 1]), turned(degrees(45), [0, 0], ten_m));
}

#[test]
fn a_turn_between_two_directions_and_its_way_back_are_no_turn() {
    runner()
        .run(
            &((-50_i64..50, -50_i64..50), (-50_i64..50, -50_i64..50)),
            |((ux, uy), (vx, vy))| {
                prop_assume!((ux, uy) != (0, 0) && (vx, vy) != (0, 0));
                let there = Turn::between([ux, uy], [vx, vy]).unwrap();
                let back = Turn::between([vx, vy], [ux, uy]).unwrap();
                let (c, s, h) = there.parts();
                prop_assert_eq!(c * c + s * s, h * h);
                prop_assert!(there.then(&back).quarter_turns() == Some(0));
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn a_mirror_is_exact_and_rounds_a_point_between_steps_once() {
    let along_x = Motion::mirror([0, 10], [7, 10]).unwrap();
    assert_eq!(along_x.apply([3, 13]), Some([3, 7]));
    assert_eq!(
        along_x.square(),
        Some(Square {
            quarters: 0,
            mirrored: true
        })
    );
    let along_y = Motion::mirror([10, 0], [10, -3]).unwrap();
    assert_eq!(along_y.apply([13, 3]), Some([7, 3]));
    let diagonal = Motion::mirror([1, 1], [2, 2]).unwrap();
    assert_eq!(diagonal.apply([5, 2]), Some([2, 5]));
    let other = Motion::mirror([0, 0], [-1, 1]).unwrap();
    assert_eq!(other.apply([5, 2]), Some([-2, -5]));
    // Any other line has a rational matrix: (1, 2) gives (-3, 4; 4, 3) / 5, so (1, 0) goes to
    // (-0.6, 0.8), the nearest step (-1, 1).
    let steep = Motion::mirror([0, 0], [1, 2]).unwrap();
    assert_eq!(steep.apply([5, 0]), Some([-3, 4]));
    assert_eq!(steep.apply([1, 0]), Some([-1, 1]));
    assert!(steep.is_mirror() && steep.square().is_none());
    assert_eq!(steep.angles(), None);
    assert_eq!(
        Motion::mirror([4, 4], [4, 4]).unwrap_err(),
        ModifyError::LineOnePoint
    );
}

#[test]
fn a_mirror_twice_is_no_motion() {
    runner()
        .run(
            &(
                (-1_000_i64..1_000, -1_000_i64..1_000),
                (-9_i64..9, -9_i64..9),
            ),
            |((ax, ay), (dx, dy))| {
                prop_assume!((dx, dy) != (0, 0));
                let mirror = Motion::mirror([ax, ay], [ax + dx, ay + dy]).unwrap();
                prop_assert!(mirror.then(&mirror).is_identity());
                prop_assert!(mirror.is_mirror());
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn motions_in_turn_are_one_motion() {
    runner()
        .run(
            &(
                (-1_000_i64..1_000, -1_000_i64..1_000),
                0_i64..4,
                (-1_000_i64..1_000, -1_000_i64..1_000),
                (-1_000_i64..1_000, -1_000_i64..1_000),
            ),
            |((cx, cy), k, (dx, dy), (px, py))| {
                let turn = Motion::turn_about([cx, cy], &Turn::quarters(k));
                let mirror = Motion::mirror([cx, cy], [cx + 1, cy + 1]).unwrap();
                let shift = Motion::by([dx, dy]);
                let chain = turn.then(&mirror).then(&shift);
                let step = turn
                    .apply([px, py])
                    .and_then(|p| mirror.apply(p))
                    .and_then(|p| shift.apply(p));
                prop_assert_eq!(chain.apply([px, py]), step);
                prop_assert!(chain.is_mirror());
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn the_eight_motions_of_a_square_change_angles_as_they_turn() {
    let arc_from = degrees(10);
    for k in 0..4_u8 {
        let turn = Motion::turn_about([0, 0], &Turn::quarters(i64::from(k)));
        let square = turn.square().unwrap();
        assert_eq!(
            square,
            Square {
                quarters: k,
                mirrored: false
            }
        );
        assert_eq!(square.angle(arc_from), degrees(10 + 90 * i64::from(k)));
        assert_eq!(square.swaps_axes(), k % 2 == 1);
        assert_eq!(turn.angles(), Some(square.angles()));
    }
    // About the line at 45 degrees, 10 degrees goes to 80.
    let diagonal = Motion::mirror([0, 0], [1, 1]).unwrap().square().unwrap();
    assert_eq!(diagonal.angle(arc_from), degrees(80));
    // About the x axis, 10 degrees goes to 350.
    let along_x = Motion::mirror([0, 0], [1, 0]).unwrap().square().unwrap();
    assert_eq!(along_x.angle(arc_from), degrees(350));
}

/// A turn by an angle changes an angle by exactly that angle, in whole angle units, through any
/// motions in turn; a turn between two directions at another angle changes it by none.
#[test]
fn a_turn_by_an_angle_changes_angles_by_whole_units() {
    let turn = Motion::turn_about([5, 5], &Turn::of_angle(tenths(375)));
    let by = tenths(375).units();
    assert_eq!(
        turn.angles(),
        Some(Angles {
            mirrored: false,
            by
        })
    );
    let along_x = Motion::mirror([0, 0], [1, 0]).unwrap();
    let both = turn.then(&along_x).then(&Motion::by([7, 0]));
    // 10 degrees turned to 47.5, then mirrored about x to 312.5.
    assert_eq!(both.angles().unwrap().angle(degrees(10)), tenths(3125));
    assert!(both.angles().unwrap().mirrored);
    let three_four = Motion::turn_about([0, 0], &Turn::between([3, 4], [1, 0]).unwrap());
    assert_eq!(three_four.angles(), None);
    assert_eq!(turn.then(&three_four).angles(), None);
}

#[test]
fn a_move_is_a_translation_and_a_turn_is_not() {
    assert_eq!(Motion::by([5, -6]).translation(), Some([5, -6]));
    let turn = Motion::turn_about([0, 0], &Turn::quarters(2));
    assert_eq!(turn.translation(), None);
    // A turn and the turn back about another point is a move.
    let back = Motion::turn_about([10, 0], &Turn::quarters(2));
    assert_eq!(turn.then(&back).translation(), Some([20, 0]));
}

/// What cannot be built is refused: a point past the model's limit. Turns in turn never grow past
/// what is held: two hundred 3:4:5 turns and their way back are no turn.
#[test]
fn a_point_past_the_models_limit_is_refused_and_nothing_overflows() {
    assert_eq!(LIMIT, (1 << 40) - 1);
    let far = Motion::by([LIMIT, 0]);
    assert_eq!(far.apply([0, 0]), Some([LIMIT, 0]));
    assert_eq!(far.apply([1, 0]), None);
    let eighth = Motion::turn_about([0, 0], &Turn::of_angle(degrees(45)));
    assert_eq!(eighth.apply([LIMIT, LIMIT]), None);
    assert_eq!(Motion::by([i64::MAX, 0]).apply([1, 0]), None);
    let there = Turn::between([3, 4], [1, 0]).unwrap();
    let back = Turn::between([1, 0], [3, 4]).unwrap();
    let mut chained = Turn::none();
    for _ in 0..200 {
        chained = chained.then(&there);
    }
    for _ in 0..200 {
        chained = chained.then(&back);
    }
    assert_eq!(chained.quarter_turns(), Some(0));
}
