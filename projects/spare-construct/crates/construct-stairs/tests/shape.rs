//! Where every piece of a stair is: each run's side profile lofted across its width and each
//! landing extruded up, worked by hand for a straight stair; the L and U stairs laid end to end in
//! plan; and the headroom where one piece passes over another.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{MM, mm, runs, settings};
use construct_params::Length;
use construct_stairs::{
    Layout, PieceKind, RuleSet, Shape, Solid, Stair, StairError, Turn, layout, layout_with, shape,
};

fn shaped(stair: &Stair, rise: i64) -> (Layout, Shape) {
    let l = layout(stair, mm(rise)).unwrap();
    let s = shape(stair, &l).unwrap();
    (l, s)
}

/// The plan outline of a piece, in whole millimetres.
fn plan_mm(s: &Shape, k: usize) -> Vec<[i64; 2]> {
    s.pieces[k]
        .plan
        .iter()
        .map(|p| [p[0] / MM, p[1] / MM])
        .collect()
}

/// A main private stairway rising 2700 mm on a 300 mm going, 1000 mm wide from the origin
/// towards +x, with a 150 mm waist: 15 risers of exactly 180 mm (1,152,000 steps), 14 treads of
/// 300 mm (1,920,000 steps), one run 4200 mm long (26,880,000 steps). By hand: from its first
/// nosing to its last it rises 14 x 180 = 2520 mm over 4200 mm, a slope of 3 in 5, so its
/// underside stands below the line of its inner corners by 150 x sqrt(1 + 0.36) = 174.928... mm
/// upright: 960,000 x 1.16619037896906 = 1,119,542.76 steps, so 1,119,543. Its top end is at
/// its last inner corner (4200 mm, 2520 mm), down to 2520 mm less that: 15,008,457 steps. The
/// underside meets the floor 15,008,457 x 5 / 3 = 25,014,095 steps back from there:
/// 26,880,000 - 25,014,095 = 1,865,905 steps (291.55 mm) from the first riser.
#[test]
fn a_straight_run_is_its_profile_lofted_across_its_width() {
    let mut s = settings("main-private");
    s.going = Some(mm(300));
    let stair = Stair::new(s).unwrap();
    let (l, built) = shaped(&stair, 2700);
    assert_eq!(l.riser_count(), 15);
    assert_eq!(built.pieces.len(), 1);
    let piece = &built.pieces[0];
    assert_eq!(piece.id, "run.1");
    assert_eq!(piece.kind, PieceKind::Run { treads: 14 });
    assert_eq!(
        plan_mm(&built, 0),
        vec![[0, 0], [4200, 0], [4200, 1000], [0, 1000]]
    );
    let Solid::Loft {
        sections: [right, left],
    } = &piece.solid
    else {
        panic!("a run is a loft");
    };
    let (riser, going) = (180 * MM, 300 * MM);
    let mut expected = vec![[0, 0, 0]];
    for i in 0..14 {
        expected.push([i * going, 0, (i + 1) * riser]);
        expected.push([(i + 1) * going, 0, (i + 1) * riser]);
    }
    expected.push([26_880_000, 0, 15_008_457]);
    expected.push([1_865_905, 0, 0]);
    assert_eq!(*right, expected);
    let moved: Vec<[i64; 3]> = expected.iter().map(|p| [p[0], 1000 * MM, p[2]]).collect();
    assert_eq!(*left, moved);
}

/// An L: a common stairway rising 2700 mm in two runs of 8 and 7 risers turning left on a 1000
/// mm landing. The first run is 7 x 280 = 1960 mm long; the landing is from there to 2960 mm and
/// 1000 mm across; the second run leaves the landing's left side at its far end, going +y, 6 x
/// 280 = 1680 mm long, lying to its left (towards -x). The first run reaches on under the
/// landing until its underside meets the landing's (1440 - 150 = 1290 mm up): by hand, its
/// underside is 150 x sqrt(1 + (9/14)^2) = 178.32 mm below its inner corners (1,141,256 steps),
/// so it meets 1290 mm at 1960 + (1290 - 1260 + 178.32) x 14 / 9 mm: 12,544,000 + 1,333,256 x
/// 14 / 9 = 12,544,000 + 2,073,953.8, so 14,617,954 steps.
#[test]
fn an_l_stair_turns_on_its_landing() {
    let stair = runs("common", &[None, None], &[(1000, Turn::Left)]);
    let (_, built) = shaped(&stair, 2700);
    let ids: Vec<&str> = built.pieces.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, vec!["run.1", "landing.1", "run.2"]);
    assert_eq!(built.pieces[0].plan[1], [14_617_954, 0]);
    assert_eq!(
        plan_mm(&built, 1),
        vec![[1960, 0], [2960, 0], [2960, 1000], [1960, 1000]]
    );
    assert_eq!(
        plan_mm(&built, 2),
        vec![[2960, 1000], [2960, 2680], [1960, 2680], [1960, 1000]]
    );
    let Solid::Extrude { profile, by } = &built.pieces[1].solid else {
        panic!("a landing is extruded");
    };
    assert_eq!(*by, [0, 0, 150 * MM]);
    assert!(profile.iter().all(|p| p[2] == 1290 * MM));
    // The second run stands on the landing's underside, its first riser rising from the landing.
    let Solid::Loft { sections, .. } = &built.pieces[2].solid else {
        panic!("a run is a loft");
    };
    assert_eq!(sections[0][0], [2960 * MM, 1000 * MM, 1290 * MM]);
    assert_eq!(sections[0][1], [2960 * MM, 1000 * MM, 1620 * MM]);
}

/// The two half turns bring the next run back beside the last, on its left or its right, the
/// landing as wide as both; a right quarter turn leaves the landing's right side.
#[test]
fn a_u_stair_comes_back_beside_itself() {
    let left = runs("accessible", &[None, None], &[(1200, Turn::BackLeft)]);
    let (_, built) = shaped(&left, 3000);
    assert_eq!(
        plan_mm(&built, 1),
        vec![[2480, 0], [3680, 0], [3680, 2000], [2480, 2000]]
    );
    assert_eq!(
        plan_mm(&built, 2),
        vec![[2480, 2000], [310, 2000], [310, 1000], [2480, 1000]]
    );
    let right = runs("accessible", &[None, None], &[(1200, Turn::BackRight)]);
    let (_, built) = shaped(&right, 3000);
    assert_eq!(
        plan_mm(&built, 1),
        vec![[2480, -1000], [3680, -1000], [3680, 1000], [2480, 1000]]
    );
    assert_eq!(
        plan_mm(&built, 2),
        vec![[2480, 0], [310, 0], [310, -1000], [2480, -1000]]
    );
    let quarter = runs("accessible", &[None, None], &[(1200, Turn::Right)]);
    let (_, built) = shaped(&quarter, 3000);
    // From the landing's far end less the width, going -y, lying to its left (towards +x).
    assert_eq!(
        plan_mm(&built, 2),
        vec![[2680, 0], [2680, -2170], [3680, -2170], [3680, 0]]
    );
}

/// A stair that turns a quarter and then a half comes back over its own first run. The third run
/// passes about 362 mm above the first run's pitch line, far short of the 2000 mm D1/AS1 asks
/// above a stair (1.4.1, Table 1 and Figure 3), and is refused naming both; a rule set asking for
/// only 100 mm takes it.
#[test]
fn a_run_over_another_needs_its_headroom() {
    let stair = runs(
        "main-private",
        &[Some(3), Some(2), None],
        &[(1000, Turn::Left), (1000, Turn::BackLeft)],
    );
    let l = layout(&stair, mm(2700)).unwrap();
    let err = shape(&stair, &l).unwrap_err();
    let StairError::Headroom {
        lower,
        upper,
        clearance,
        least,
    } = &err
    else {
        panic!("{err:?}");
    };
    assert_eq!((lower.as_str(), upper.as_str()), ("run 1", "run 3"));
    assert_eq!(least.value, mm(2000));
    assert!(
        clearance.steps() > 300 * MM && clearance.steps() < 400 * MM,
        "{clearance}"
    );
    assert!(err.to_string().starts_with("Run 3 passes 3"), "{err}");
    assert!(
        err.to_string()
            .ends_with("above run 1's pitch line: the least headroom above it is 2000 mm (D1/AS1 1.4.1, Table 1 and Figure 3)."),
        "{err}"
    );

    let low = RuleSet::from_json(
        &include_str!("../rules/nz-d1-as1.json")
            .replace("\"2000 mm\"", "\"100 mm\"")
            .replace("\"nz-d1-as1\"", "\"low\""),
    )
    .unwrap();
    let mut s = stair.settings();
    s.rules = "low".to_owned();
    let stair = Stair::new(s).unwrap();
    let l = layout_with(&low, &stair, mm(2700)).unwrap();
    assert_eq!(shape(&stair, &l).unwrap().pieces.len(), 5);
}

/// Whether every inner corner of a run's profile (the right-hand section of its loft) stands
/// strictly above the underside, worked here apart from the crate: the underside is the profile's
/// last two corners' line where it is above the base.
fn passes_below(section: &[[i64; 3]], going: i64) -> bool {
    let n = section.len();
    let ([a0, _, z0], [a1, _, z1]) = (section[n - 1], section[n - 2]);
    section[1..n - 2]
        .iter()
        .filter(|p| p[0] % going == 0 && p[0] > a0 && p[0] <= a1 && a1 > a0)
        .filter(|p| section.iter().any(|q| q[0] + going == p[0] && q[2] == p[2]))
        .all(|p| {
            i128::from(p[2] - z0) * i128::from(a1 - a0)
                > i128::from(z1 - z0) * i128::from(p[0] - a0)
        })
}

/// The underside passes below every step even with a waist of one grid step, over rises a step
/// apart on the steepest stairway, where the line's rounding matters most: each shape either
/// passes this check, worked here apart from the crate, or is refused as too thin (the crate's
/// own unit test makes one that is). A thick waist makes a run a solid block down to its base.
#[test]
fn the_underside_passes_below_every_step() {
    let mut s = settings("service");
    s.waist = Length::from_steps(1);
    let thin = Stair::new(s).unwrap();
    for k in 0..4000 {
        let l = layout(&thin, Length::from_steps(2000 * MM + k)).unwrap();
        match shape(&thin, &l) {
            Ok(built) => {
                let Solid::Loft { sections, .. } = &built.pieces[0].solid else {
                    panic!("a run is a loft");
                };
                assert!(passes_below(&sections[0], l.going.steps()), "{k}");
            }
            Err(e) => {
                assert_eq!(
                    e,
                    StairError::WaistTooThin {
                        run: 1,
                        waist: Length::from_steps(1)
                    }
                );
            }
        }
    }
    let mut s = settings("main-private");
    s.waist = mm(5000);
    let thick = Stair::new(s).unwrap();
    let l = layout(&thick, mm(2500)).unwrap();
    let built = shape(&thick, &l).unwrap();
    let Solid::Loft { sections, .. } = &built.pieces[0].solid else {
        panic!("a run is a loft");
    };
    // Its last corner is straight below its top end, on the floor.
    let last = sections[0].last().unwrap();
    assert_eq!(last[2], 0);
    assert_eq!(last[0], l.runs[0].length.steps());
}

/// Every direction gives the same stair turned: the first run along its axis, the stair to its
/// left.
#[test]
fn every_direction_turns_the_same_stair() {
    use construct_stairs::Direction;
    for (direction, far, side) in [
        (Direction::PlusX, [3640, 0], [0, 1000]),
        (Direction::PlusY, [0, 3640], [-1000, 0]),
        (Direction::MinusX, [-3640, 0], [0, -1000]),
        (Direction::MinusY, [0, -3640], [1000, 0]),
    ] {
        let mut s = settings("common");
        s.direction = direction;
        let (_, built) = shaped(&Stair::new(s).unwrap(), 2500);
        let plan = plan_mm(&built, 0);
        assert_eq!(plan[1], far, "{direction:?}");
        assert_eq!(plan[3], side, "{direction:?}");
    }
}
