//! The exact motions: which turns and mirrors land on the length grid, the refusals of those that
//! do not, and the algebra the modify tools and groups build on (composition, a mirror twice, a
//! turn and its way back), each on our own types.

// The crate denies these outside tests (Cargo.toml); a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_modify::exact::{Miss, QUARTER};
use construct_modify::{ModifyError, Motion, Square, Turn};
use construct_params::{Angle, Length};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};

/// Steps in a millimetre.
const MM: i64 = Length::STEPS_PER_MM;

fn degrees(d: i64) -> Angle {
    Angle::from_units(d * Angle::UNITS_PER_DEGREE)
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

#[test]
fn quarter_turns_land_every_point_and_four_come_round() {
    let centre = [1_234 * MM, -567 * MM];
    let quarter = Motion::turn_about(centre, Turn::of_angle(degrees(90)).unwrap()).unwrap();
    assert_eq!(
        quarter.apply([centre[0] + 3, centre[1]]),
        Ok([centre[0], centre[1] + 3])
    );
    assert_eq!(quarter.apply(centre), Ok(centre));
    let round = quarter.times_over(4).unwrap();
    assert!(round.is_identity());
    for k in -5..=5_i64 {
        let turn = Turn::of_angle(degrees(90 * k)).unwrap();
        assert_eq!(turn.quarter_turns(), Some(k.rem_euclid(4) as u8), "{k}");
    }
}

#[test]
fn an_angle_off_the_quarters_is_refused_naming_the_quarters_either_side() {
    let refused = Turn::of_angle(degrees(30)).unwrap_err();
    assert_eq!(
        refused,
        ModifyError::AngleOffGrid {
            angle: degrees(30),
            below: degrees(0),
            above: degrees(90),
        }
    );
    assert_eq!(
        refused.to_string(),
        "A turn of 30° does not land on the length grid: only a whole number of quarter turns \
         does, and the nearest are 0° and 90°. To turn by another angle, turn from one direction \
         to another whose lengths are whole (3 to 4 to 5, say)."
    );
    let refused = Turn::of_angle(degrees(-100)).unwrap_err();
    assert_eq!(
        refused,
        ModifyError::AngleOffGrid {
            angle: degrees(-100),
            below: degrees(-180),
            above: degrees(-90),
        }
    );
    // One angle unit off a quarter turn is refused too: nothing is rounded to the nearest.
    let nearly = Angle::from_units(QUARTER + 1);
    assert!(matches!(
        Turn::of_angle(nearly),
        Err(ModifyError::AngleOffGrid { .. })
    ));
}

/// Every angle a person can type that is not a whole number of quarter turns is refused: an angle
/// is a whole number of angle units, a rational number of degrees, and by Niven's theorem none of
/// those but the quarter turns has a rational cosine and sine, which landing a point needs.
#[test]
fn every_angle_off_the_quarters_is_refused() {
    runner()
        .run(&(-4 * QUARTER..4 * QUARTER), |units| {
            let result = Turn::of_angle(Angle::from_units(units));
            prop_assert_eq!(result.is_ok(), units % QUARTER == 0);
            Ok(())
        })
        .unwrap();
}

#[test]
fn a_turn_from_one_direction_to_another_lands_when_its_lengths_are_whole() {
    // A wall 3 m by 4 m off the x axis, turned onto it about its start: 5 m along x.
    let turn = Turn::between([3, 4], [1, 0]).unwrap();
    assert_eq!(turn.parts(), (3, -4, 5));
    let onto = Motion::turn_about([0, 0], turn).unwrap();
    assert_eq!(onto.apply([3_000 * MM, 4_000 * MM]), Ok([5_000 * MM, 0]));
    // A point whose steps the hypotenuse does not divide lies between steps.
    assert_eq!(onto.apply([1, 0]), Err(Miss::OffGrid));
    // The lengths need not be whole one by one: the square roots of 2 multiply to 2.
    assert_eq!(
        Turn::between([1, 1], [-1, 1]).unwrap().quarter_turns(),
        Some(1)
    );
    // Lengths whose product is not whole: refused, in words.
    let refused = Turn::between([1, 2], [1, 0]).unwrap_err();
    assert_eq!(refused, ModifyError::TurnNotWhole);
    assert_eq!(
        refused.to_string(),
        "That turn does not land on the length grid: the two directions' lengths do not multiply \
         to a whole number (3 to 4 to 5 does)."
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

#[test]
fn a_turn_and_its_way_back_are_no_turn() {
    runner()
        .run(
            &((-50_i64..50, -50_i64..50), (-50_i64..50, -50_i64..50)),
            |((ux, uy), (vx, vy))| {
                prop_assume!((ux, uy) != (0, 0) && (vx, vy) != (0, 0));
                let (Ok(there), Ok(back)) = (
                    Turn::between([ux, uy], [vx, vy]),
                    Turn::between([vx, vy], [ux, uy]),
                ) else {
                    return Ok(());
                };
                prop_assert_eq!(there.then(back).unwrap(), Turn::NONE);
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn a_mirror_lands_on_lines_along_the_axes_and_at_45_degrees() {
    let along_x = Motion::mirror([0, 10], [7, 10]).unwrap();
    assert_eq!(along_x.apply([3, 13]), Ok([3, 7]));
    assert_eq!(
        along_x.square(),
        Some(Square {
            quarters: 0,
            mirrored: true
        })
    );
    let along_y = Motion::mirror([10, 0], [10, -3]).unwrap();
    assert_eq!(along_y.apply([13, 3]), Ok([7, 3]));
    let diagonal = Motion::mirror([1, 1], [2, 2]).unwrap();
    assert_eq!(diagonal.apply([5, 2]), Ok([2, 5]));
    let other = Motion::mirror([0, 0], [-1, 1]).unwrap();
    assert_eq!(other.apply([5, 2]), Ok([-2, -5]));
    // Any other line has a rational matrix: (1, 2) gives (-3, 4; 4, 3) / 5.
    let steep = Motion::mirror([0, 0], [1, 2]).unwrap();
    assert_eq!(steep.apply([5, 0]), Ok([-3, 4]));
    assert_eq!(steep.apply([1, 0]), Err(Miss::OffGrid));
    assert!(steep.is_mirror() && steep.square().is_none());
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
                prop_assert!(mirror.then(&mirror).unwrap().is_identity());
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
                let turn = Motion::turn_about([cx, cy], Turn::quarters(k)).unwrap();
                let mirror = Motion::mirror([cx, cy], [cx + 1, cy + 1]).unwrap();
                let shift = Motion::by([dx, dy]);
                let chain = turn.then(&mirror).unwrap().then(&shift).unwrap();
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
        let turn = Motion::turn_about([0, 0], Turn::quarters(i64::from(k))).unwrap();
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
    }
    // About the line at 45 degrees, 10 degrees goes to 80.
    let diagonal = Motion::mirror([0, 0], [1, 1]).unwrap().square().unwrap();
    assert_eq!(diagonal.angle(arc_from), degrees(80));
    // About the x axis, 10 degrees goes to 350.
    let along_x = Motion::mirror([0, 0], [1, 0]).unwrap().square().unwrap();
    assert_eq!(along_x.angle(arc_from), degrees(350));
}

#[test]
fn a_move_is_a_translation_and_a_turn_is_not() {
    assert_eq!(Motion::by([5, -6]).translation(), Some([5, -6]));
    let turn = Motion::turn_about([0, 0], Turn::quarters(2)).unwrap();
    assert_eq!(turn.translation(), None);
    // A turn and the turn back about another point is a move.
    let back = Motion::turn_about([10, 0], Turn::quarters(2)).unwrap();
    assert_eq!(turn.then(&back).unwrap().translation(), Some([20, 0]));
}

#[test]
fn a_motion_past_the_numbers_held_is_refused_not_wrapped() {
    let far = Motion::by([i64::MAX, 0]);
    assert_eq!(far.apply([1, 0]), Err(Miss::TooLarge));
    let turn = Turn::between([3, 4], [1, 0]).unwrap();
    let mut chained = Ok(Turn::NONE);
    for _ in 0..200 {
        chained = chained.and_then(|t| t.then(turn));
    }
    assert_eq!(chained.unwrap_err(), ModifyError::TooLarge);
}
