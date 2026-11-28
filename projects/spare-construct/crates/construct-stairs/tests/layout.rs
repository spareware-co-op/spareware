//! A stair's layout from its rise, worked by hand for several floor-to-floor heights and checked
//! here exactly: the count of risers, each riser's height in grid steps (6,400 to the millimetre),
//! the runs, the landings and the total run; and every limit's refusal, naming the limit, its
//! clause and, where no count of risers works, the nearest heights that do.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{MM, mm, runs, settings, steps, straight};
use construct_params::{Length, Slope};
use construct_stairs::{Layout, RiserCount, RuleSet, Stair, StairError, Turn, layout, layout_with};

/// The risers that are one grid step taller than the rest, numbered from 1 at the bottom.
fn taller(l: &Layout) -> Vec<usize> {
    let low = l.lowest();
    l.risers
        .iter()
        .enumerate()
        .filter(|(_, r)| **r != low)
        .map(|(k, _)| k + 1)
        .collect()
}

/// A common stairway rising 2500 mm, one run, the least going (280 mm). By hand: 2500 / 190 =
/// 13.16, so 14 risers (13 would be 192.3 mm, over the 190 mm of Table 6); 2500 / 14 =
/// 178.571... mm, which is 16,000,000 / 14 = 1,142,857 steps and 2 over: risers 7 and 14 are
/// 1,142,858 steps (floor(7 H / 14) is 8,000,000 exactly), the other twelve 1,142,857. 13
/// treads of 280 mm: a total run of 3640 mm. The one run rises 2500 mm, Table 7's most for a
/// common stairway, which it may.
#[test]
fn a_common_stair_of_2500_mm() {
    let l = layout(&straight("common"), mm(2500)).unwrap();
    assert_eq!(l.riser_count(), 14);
    assert_eq!(l.tallest(), Length::from_steps(1_142_858));
    assert_eq!(l.lowest(), Length::from_steps(1_142_857));
    assert_eq!(taller(&l), vec![7, 14]);
    assert_eq!(l.risers.iter().map(|r| r.steps()).sum::<i64>(), 2500 * MM);
    assert_eq!(l.going, mm(280));
    assert_eq!(l.tread_count(), 13);
    assert_eq!(l.total_run().unwrap(), mm(3640));
    assert_eq!(l.runs.len(), 1);
    assert_eq!(l.runs[0].rise, mm(2500));
    assert_eq!(l.runs[0].length, mm(3640));
    assert_eq!(l.pitch(), Slope::new(1_142_858, 280 * MM));
    assert_eq!(l.category_name, "Common stairway");
}

/// A common stairway rising 2700 mm in two runs with a straight landing of 900 mm. By hand:
/// 2700 / 190 = 14.2, so 15 risers, each exactly 180 mm (2700 / 15). The two runs share them, the
/// first taking the odd one: 8 and 7, rising 1440 mm and 1260 mm; the landing is 1440 mm up. 7 +
/// 6 treads of 280 mm and the landing: 1960 + 900 + 1680 = 4540 mm. In one run it would rise
/// 2700 mm, over Table 7's 2500 mm for a common stairway, and is refused, naming the clause.
#[test]
fn a_common_stair_of_2700_mm_needs_its_landing() {
    let l = layout(
        &runs("common", &[None, None], &[(900, Turn::Straight)]),
        mm(2700),
    )
    .unwrap();
    assert_eq!(l.riser_count(), 15);
    assert!(l.risers.iter().all(|r| *r == mm(180)));
    assert_eq!(
        l.runs
            .iter()
            .map(|r| (r.risers, r.rise))
            .collect::<Vec<_>>(),
        vec![(8, mm(1440)), (7, mm(1260))]
    );
    assert_eq!(l.runs[1].first, 8);
    assert_eq!(l.runs[1].base, mm(1440));
    assert_eq!(l.landings[0].height, mm(1440));
    assert_eq!(l.tread_count(), 13);
    assert_eq!(l.total_run().unwrap(), mm(4540));

    let err = layout(&straight("common"), mm(2700)).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Run 1 rises 2700 mm on a common stairway: the most a stair may rise between landings is 2500 mm (D1/AS1 4.3.2 and Table 7). Add a landing."
    );
    // A main private stairway may rise 4 m between landings.
    assert_eq!(
        layout(&straight("main-private"), mm(2700))
            .unwrap()
            .riser_count(),
        15
    );
}

/// A main private stairway rising 2600 mm on a 300 mm going. By hand: 2600 / 190 = 13.68, so 14
/// risers of 185.714... mm: 16,640,000 / 14 = 1,188,571 steps and 6 over. Riser k is the taller
/// one exactly when 6k mod 14 is below 6: risers 3, 5, 7, 10, 12 and 14. 13 treads of 300 mm: a
/// total run of 3900 mm.
#[test]
fn a_main_private_stair_of_2600_mm_on_a_300_mm_going() {
    let mut s = settings("main-private");
    s.going = Some(mm(300));
    let l = layout(&Stair::new(s).unwrap(), mm(2600)).unwrap();
    assert_eq!(l.riser_count(), 14);
    assert_eq!(l.lowest(), Length::from_steps(1_188_571));
    assert_eq!(l.tallest(), Length::from_steps(1_188_572));
    assert_eq!(taller(&l), vec![3, 5, 7, 10, 12, 14]);
    assert_eq!(l.going, mm(300));
    assert_eq!(l.total_run().unwrap(), mm(3900));
}

/// An accessible stairway rising 3000 mm in two runs with a 1200 mm landing, on its least going of
/// 310 mm. By hand: 3000 / 180 = 16.67, so 17 risers of 176.47 mm. The runs take 9 and 8; the
/// first rises floor(9 x 19,200,000 / 17) = 10,164,705 steps (1588.24 mm), the second the other
/// 9,035,295 (1411.76 mm), each within Table 7's 2500 mm. Treads 8 + 7 of 310 mm and the
/// landing: 2480 + 1200 + 2170 = 5850 mm.
#[test]
fn an_accessible_stair_of_3000_mm() {
    let l = layout(
        &runs("accessible", &[None, None], &[(1200, Turn::BackLeft)]),
        mm(3000),
    )
    .unwrap();
    assert_eq!(l.riser_count(), 17);
    assert_eq!(l.going, mm(310));
    assert_eq!(
        l.runs
            .iter()
            .map(|r| (r.risers, steps(r.rise)))
            .collect::<Vec<_>>(),
        vec![(9, 10_164_705), (8, 9_035_295)]
    );
    assert_eq!(l.total_run().unwrap(), mm(5850));
}

/// A rise between two counts: 400 mm on a common stairway. Two risers would be 200 mm, over
/// Table 6's 190 mm; three would be 133.3 mm, under the 150 mm Figure 11 charts. The nearest
/// heights that work are 2 x 190 = 380 mm and 3 x 150 = 450 mm.
#[test]
fn a_rise_no_count_lays_out_names_the_nearest_heights_that_work() {
    let err = layout(&straight("common"), mm(400)).unwrap_err();
    let StairError::NoRiserCount(no) = &err else {
        panic!("{err:?}");
    };
    let construct_stairs::NoCount {
        below,
        above,
        lowest,
        highest,
        risers,
        ..
    } = no.as_ref();
    assert_eq!((*below, *above), (Some(mm(380)), Some(mm(450))));
    assert_eq!(lowest.value, mm(150));
    assert_eq!(highest.value, mm(190));
    assert_eq!(*risers, RiserCount::AtLeast(2));
    assert_eq!(
        err.to_string(),
        "No count of risers lays out a rise of 400 mm on a common stairway with a going of 280 mm and at least 2 risers, two to each run: the lowest a riser may be is 150 mm (D1/AS1 4.1.1 and Figure 11 (the least riser height it charts)), and the highest a riser may be is 190 mm (D1/AS1 4.1.1 and Table 6); the nearest floor-to-floor heights that work are 380 mm and 450 mm."
    );
    // Below two of the least risers, only a height above works: 300 mm.
    let err = layout(&straight("common"), mm(250)).unwrap_err();
    assert!(
        err.to_string()
            .ends_with("; the nearest floor-to-floor height that works is 300 mm."),
        "{err}"
    );
}

/// On a long going the least pitch sets the lowest riser: on 400 mm, 23 degrees is a riser of
/// ceil(2,560,000 x 0.424474816) = 1,086,656 steps (169.79 mm). A rise of 330 mm is then refused
/// (two risers of 165 mm are too shallow, one of 330 mm too high), naming the pitch and its
/// clause; the nearest height that works is two of those risers.
#[test]
fn the_least_pitch_sets_the_lowest_riser_on_a_long_going() {
    let mut s = settings("common");
    s.going = Some(mm(400));
    let stair = Stair::new(s).unwrap();
    let err = layout(&stair, mm(330)).unwrap_err();
    let StairError::NoRiserCount(no) = &err else {
        panic!("{err:?}");
    };
    let construct_stairs::NoCount {
        lowest,
        below,
        above,
        ..
    } = no.as_ref();
    assert_eq!(lowest.value, Length::from_steps(1_086_656));
    assert_eq!(
        lowest.clause,
        "D1/AS1 1.2.1 and Figure 2, and 4.1.1 Comment 2"
    );
    assert_eq!(*below, None);
    assert_eq!(*above, Some(Length::from_steps(2 * 1_086_656)));
    assert_eq!(layout(&stair, mm(2500)).unwrap().riser_count(), 14);
}

/// Runs that give their counts fix the stair's: 8 and 6 risers make 14, too few for 2700 mm (192.9
/// mm each). The only heights 14 risers lay out are 14 x 150 = 2100 mm to 14 x 190 = 2660 mm.
#[test]
fn runs_that_give_their_counts_fix_the_stairs() {
    let stair = runs("common", &[Some(8), Some(6)], &[(1000, Turn::Straight)]);
    let err = layout(&stair, mm(2700)).unwrap_err();
    let StairError::NoRiserCount(no) = &err else {
        panic!("{err:?}");
    };
    let construct_stairs::NoCount {
        risers,
        below,
        above,
        ..
    } = no.as_ref();
    assert_eq!(*risers, RiserCount::Exactly(14));
    assert_eq!((*below, *above), (Some(mm(2660)), None));
    let l = layout(&stair, mm(2600)).unwrap();
    assert_eq!(
        l.runs.iter().map(|r| r.risers).collect::<Vec<_>>(),
        vec![8, 6]
    );
    // One run gives its count and the other takes the rest.
    let l = layout(
        &runs("common", &[Some(4), None], &[(1000, Turn::Straight)]),
        mm(2500),
    )
    .unwrap();
    assert_eq!(
        l.runs.iter().map(|r| r.risers).collect::<Vec<_>>(),
        vec![4, 10]
    );
}

/// The going, the width and the landings are checked against their limits, each named with its
/// clause.
#[test]
fn each_limit_is_refused_naming_its_clause() {
    let with = |f: &dyn Fn(&mut construct_stairs::Settings)| {
        let mut s = settings("common");
        f(&mut s);
        layout(&Stair::new(s).unwrap(), mm(2500))
            .unwrap_err()
            .to_string()
    };
    assert_eq!(
        with(&|s| s.going = Some(mm(270))),
        "A going of 270 mm is too short for a common stairway: the shortest a going may be is 280 mm (D1/AS1 4.1.4 a) and Table 6)."
    );
    assert_eq!(
        with(&|s| s.going = Some(mm(410))),
        "A going of 410 mm is too long: the longest a going may be is 400 mm (D1/AS1 4.1.1 and Figure 11 (the deepest tread it charts))."
    );
    assert_eq!(
        with(&|s| s.width = mm(850)),
        "A width of 850 mm is too narrow for a common stairway: the narrowest it may be is 900 mm (D1/AS1 4.2.1)."
    );
    let short = runs("common", &[None, None], &[(800, Turn::Straight)]);
    assert_eq!(
        layout(&short, mm(2500)).unwrap_err().to_string(),
        "Landing 1 is 800 mm long: the shortest a landing may be is 900 mm (D1/AS1 4.3.4)."
    );
    // A main private stairway has no least width.
    let mut s = settings("main-private");
    s.width = mm(850);
    assert!(layout(&Stair::new(s).unwrap(), mm(2500)).is_ok());
    assert_eq!(
        layout(&straight("common"), mm(0)).unwrap_err(),
        StairError::NotRising { rise: mm(0) }
    );
    let mut s = settings("grand");
    s.rules = "nz-d1-as1".to_owned();
    assert!(matches!(
        layout(&Stair::new(s).unwrap(), mm(2500)),
        Err(StairError::UnknownCategory { .. })
    ));
}

/// The rule set is data: one that asks for risers exactly alike lays a rise that 14 risers do not
/// divide out in 16 of exactly 156.25 mm, and refuses one no allowed count divides (a rise one
/// step over 2500 mm), naming its uniformity; one with a shallower most pitch sets the highest
/// riser by it.
#[test]
fn the_rule_set_drives_the_layout() {
    let nz = include_str!("../rules/nz-d1-as1.json");
    let exact = RuleSet::from_json(
        &nz.replace("\"length\": \"5 mm\"", "\"length\": \"0 mm\"")
            .replace("\"nz-d1-as1\"", "\"exact\""),
    )
    .unwrap();
    let mut s = settings("common");
    s.rules = "exact".to_owned();
    let stair = Stair::new(s).unwrap();
    let l = layout_with(&exact, &stair, mm(2500)).unwrap();
    assert_eq!(l.riser_count(), 16);
    assert!(l.risers.iter().all(|r| r.steps() == 1_000_000));
    let err = layout_with(&exact, &stair, Length::from_steps(2500 * MM + 1)).unwrap_err();
    assert!(
        matches!(err, StairError::NotUniform { risers: 14, .. }),
        "{err:?}"
    );

    let shallow = RuleSet::from_json(
        &nz.replacen("\"degrees\": 37", "\"degrees\": 30", 1)
            .replace("\"nz-d1-as1\"", "\"shallow\""),
    )
    .unwrap();
    let mut s = settings("common");
    s.rules = "shallow".to_owned();
    let stair = Stair::new(s).unwrap();
    // 280 mm at 30 degrees: floor(1,792,000 x 0.577350269) = 1,034,611 steps (161.66 mm), so 2500
    // mm takes 16 risers (15 would be 166.67 mm), and 400 mm none (two of 200 mm are too high,
    // three of 133.3 mm too low).
    let l = layout_with(&shallow, &stair, mm(2500)).unwrap();
    assert_eq!(l.riser_count(), 16);
    let err = layout_with(&shallow, &stair, mm(400)).unwrap_err();
    let StairError::NoRiserCount(no) = &err else {
        panic!("{err:?}");
    };
    let highest = &no.highest;
    assert_eq!(highest.value, Length::from_steps(1_034_611));
    assert!(
        highest.what.contains("most pitch of 30 degrees"),
        "{}",
        highest.what
    );
    // A stair names its rule set: laid out by another, it is refused.
    assert!(matches!(
        layout_with(&exact, &straight("common"), mm(2500)),
        Err(StairError::UnknownRules(_))
    ));
}

/// The same stair and rise give the same layout every time, and every height from 300 mm to 4 m
/// in steps of 7 mm either lays out with every riser within the limits, the risers summing to
/// the rise and differing by one grid step at most, or is refused.
#[test]
fn every_layout_keeps_every_limit() {
    let stair = runs("main-private", &[None, None], &[(1000, Turn::Left)]);
    let rules = RuleSet::builtin("nz-d1-as1").unwrap();
    let c = rules.category("main-private").unwrap();
    for h in (300..=4000).step_by(7) {
        let rise = mm(h);
        match layout(&stair, rise) {
            Ok(l) => {
                assert_eq!(l, layout(&stair, rise).unwrap());
                assert_eq!(
                    l.risers.iter().map(|r| r.steps()).sum::<i64>(),
                    rise.steps()
                );
                assert!(l.tallest().steps() - l.lowest().steps() <= 1);
                assert!(
                    l.tallest() <= c.riser_most.length && l.lowest() >= rules.riser_least.length
                );
                assert!(l.runs.iter().all(|r| r.risers >= 2));
            }
            Err(e) => assert!(matches!(e, StairError::NoRiserCount(_)), "{h}: {e}"),
        }
    }
}
