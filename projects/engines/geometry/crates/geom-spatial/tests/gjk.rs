//! The certified GJK filter through the public interface (piece G294), against what 19.10 asks of
//! it: never clear below the threshold, every certificate exact, and clear wherever a plain gap
//! shows.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    reason = "test checks and generators; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::num::NonZeroU64;

use geom_num::Sign;
use geom_num::distance;
use geom_num::grid::{COORD_MAX, P3};
use geom_run::{Budget, Stop, WorkMeter};
use geom_spatial::gjk::{Certificate, DIRECTION, Filter, GjkError, MAX_REACH, STEPS, filter};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// The eight corners of the box from `lo` to `hi`.
fn cube(lo: [i64; 3], hi: [i64; 3]) -> Vec<P3> {
    let mut out = Vec::new();
    for corner in 0..8 {
        let pick = |axis: usize| {
            if corner >> axis & 1 == 0 {
                lo[axis]
            } else {
                hi[axis]
            }
        };
        out.push(p(pick(0), pick(1), pick(2)));
    }
    out
}

fn clear(a: &[P3], b: &[P3], reach: u64) -> Option<Certificate> {
    match filter(a, b, reach, &mut meter()).unwrap() {
        Filter::Clear(c) => Some(c),
        Filter::Undecided => None,
    }
}

/// The certificate checked here, apart from the crate: its extremes are attained and bound every
/// point, and its gap squared reaches reach² · |n|², in plain wide integers.
fn holds(c: &Certificate, a: &[P3], b: &[P3], reach: u64) -> bool {
    let [x, y, z] = c.direction.map(i128::from);
    let along =
        |q: &P3| x * i128::from(q.x.get()) + y * i128::from(q.y.get()) + z * i128::from(q.z.get());
    let first = a.iter().map(along).max().unwrap();
    let second = b.iter().map(along).min().unwrap();
    let gap = second - first;
    let length = (x * x + y * y + z * z) as u128;
    first == c.first
        && second == c.second
        && gap > 0
        && (gap as u128) * (gap as u128) >= u128::from(reach) * u128::from(reach) * length
}

/// Whether every point of `b` is at least `reach` from every point of `a`, by G151's exact
/// comparison of squares: what a clear verdict must imply.
fn every_pair_apart(a: &[P3], b: &[P3], reach: u64) -> bool {
    a.iter().all(|q| {
        b.iter()
            .all(|r| distance::point_point(*q, *r, reach).unwrap() != Sign::Negative)
    })
}

#[test]
fn boxes_a_gap_apart_are_clear_up_to_the_gap_and_no_further() {
    let a = cube([0, 0, 0], [10, 10, 10]);
    let b = cube([25, 0, 0], [35, 10, 10]);
    for reach in [0, 1, 14, 15] {
        let c = clear(&a, &b, reach).unwrap_or_else(|| panic!("reach {reach}"));
        assert!(c.proves(&a, &b, reach), "reach {reach}");
        assert!(holds(&c, &a, &b, reach), "reach {reach}");
        assert_eq!(c.direction, [DIRECTION, 0, 0]);
    }
    for reach in [16, 100] {
        assert_eq!(clear(&a, &b, reach), None, "reach {reach}");
    }
    // The second set is on the other side: the direction turns round.
    let c = clear(&b, &a, 15).unwrap();
    assert_eq!(c.direction, [-DIRECTION, 0, 0]);
    assert!(holds(&c, &b, &a, 15));
}

#[test]
fn two_points_five_apart_are_clear_at_five_and_not_at_six() {
    let a = [p(0, 0, 0)];
    let b = [p(3, 4, 0)];
    let c = clear(&a, &b, 5).unwrap();
    assert_eq!(c.direction, [786_432, 1_048_576, 0]);
    assert_eq!((c.first, c.second), (0, 6_553_600));
    assert!(c.proves(&a, &b, 5));
    assert_eq!(clear(&a, &b, 6), None);
}

#[test]
fn a_slanted_gap_is_found_and_never_overstated() {
    // The nearest corners are (10, 10, 10) and (100, 100, 100): 90√3 ≈ 155.88 apart.
    let a = cube([0, 0, 0], [10, 10, 10]);
    let b = cube([100, 100, 100], [110, 110, 110]);
    for reach in [100, 150, 155] {
        let c = clear(&a, &b, reach).unwrap_or_else(|| panic!("reach {reach}"));
        assert!(holds(&c, &a, &b, reach));
        assert!(every_pair_apart(&a, &b, reach));
    }
    assert_eq!(clear(&a, &b, 156), None);
}

#[test]
fn overlapping_touching_and_shared_sets_are_never_clear() {
    let a = cube([0, 0, 0], [10, 10, 10]);
    let inside = cube([5, 5, 5], [15, 15, 15]);
    let touching = cube([10, 0, 0], [20, 10, 10]);
    let corner = cube([10, 10, 10], [20, 20, 20]);
    for b in [&inside, &touching, &corner, &a] {
        for reach in [0, 1, 1000] {
            assert_eq!(clear(&a, b, reach), None);
        }
    }
}

#[test]
fn the_far_corners_of_the_frame_stay_in_the_budget() {
    let m = COORD_MAX;
    let a = cube([-m, -m, -m], [-m + 7, -m + 7, -m + 7]);
    let b = cube([m - 7, m - 7, m - 7], [m, m, m]);
    // About 2 · (2^40 − 8) · √3 ≈ 3.8 · 10^12 apart.
    let reach = 1_u64 << 41;
    let c = clear(&a, &b, reach).unwrap();
    assert!(c.proves(&a, &b, reach));
    assert!(holds(&c, &a, &b, reach));
    assert_eq!(clear(&a, &b, MAX_REACH), None);
    // Along one axis only, the gap is exactly 2 · (2^40 − 1) − 2 · 7.
    let left = cube([-m, 0, 0], [-m + 7, 7, 7]);
    let right = cube([m - 7, 0, 0], [m, 7, 7]);
    let gap = (2 * (m - 7)) as u64;
    assert!(
        clear(&left, &right, gap)
            .unwrap()
            .proves(&left, &right, gap)
    );
    assert_eq!(clear(&left, &right, gap + 1), None);
}

#[test]
fn errors_say_what_was_wrong() {
    let a = cube([0, 0, 0], [1, 1, 1]);
    assert_eq!(filter(&[], &a, 0, &mut meter()), Err(GjkError::Empty));
    assert_eq!(filter(&a, &[], 0, &mut meter()), Err(GjkError::Empty));
    let b = cube([5, 0, 0], [6, 1, 1]);
    assert_eq!(
        filter(&a, &b, MAX_REACH + 1, &mut meter()),
        Err(GjkError::ReachTooLarge)
    );
    assert!(filter(&a, &b, MAX_REACH, &mut meter()).is_ok());
    let mut small = WorkMeter::new(Budget::of_work(1));
    assert!(matches!(
        filter(&a, &b, 0, &mut small),
        Err(GjkError::Stopped(Stop::OverBudget(_)))
    ));
    assert_eq!(GjkError::Empty.to_string(), "a set of points was empty");
    assert!(
        GjkError::ReachTooLarge
            .to_string()
            .contains(&MAX_REACH.to_string())
    );
    assert!(
        GjkError::Stopped(Stop::Cancelled)
            .to_string()
            .contains("cancelled")
    );
}

#[test]
fn the_work_charged_is_the_steps_taken_over_both_sets() {
    let a = cube([0, 0, 0], [10, 10, 10]);
    let b = cube([25, 0, 0], [35, 10, 10]);
    let mut counted = meter();
    filter(&a, &b, 0, &mut counted).unwrap();
    let work = counted.used().work;
    // Each step and the certificate charge one unit per point of both sets.
    assert_eq!(work % 16, 0, "{work}");
    assert!((32..=16 * (STEPS as u128 + 1)).contains(&work), "{work}");
    // A budget of exactly that answers; one less stops.
    let exact = u64::try_from(work).unwrap();
    assert!(filter(&a, &b, 0, &mut WorkMeter::new(Budget::of_work(exact))).is_ok());
    assert!(filter(&a, &b, 0, &mut WorkMeter::new(Budget::of_work(exact - 1))).is_err());
}

#[test]
fn a_certificate_proves_only_what_it_says() {
    let a = cube([0, 0, 0], [10, 10, 10]);
    let b = cube([25, 0, 0], [35, 10, 10]);
    let c = clear(&a, &b, 15).unwrap();
    assert!(c.proves(&a, &b, 15));
    assert!(c.proves(&a, &b, 0));
    assert!(!c.proves(&a, &b, 16));
    assert!(!c.proves(&b, &a, 15));
    assert!(!c.proves(&a, &b, MAX_REACH + 1));
    assert!(!c.proves(&[], &b, 0));
    let moved = |f: &dyn Fn(&mut Certificate)| {
        let mut d = c;
        f(&mut d);
        d.proves(&a, &b, 0)
    };
    assert!(!moved(&|d| d.first += 1));
    assert!(!moved(&|d| d.first -= 1));
    assert!(!moved(&|d| d.second += 1));
    assert!(!moved(&|d| d.second -= 1));
    assert!(!moved(&|d| d.direction = [DIRECTION * 2, 0, 0]));
    assert!(!moved(&|d| d.direction = [0, 0, 0]));
    // A smaller direction is a certificate too, if its extremes are the sets' own.
    let mut half = c;
    half.direction = [DIRECTION / 2, 0, 0];
    half.first /= 2;
    half.second /= 2;
    assert!(half.proves(&a, &b, 15));
}

/// Up to `n` points within `side` steps of `at`.
fn cloud(rng: &mut SplitMix64, at: [i64; 3], side: i64, n: u64) -> Vec<P3> {
    let count = 1 + rng.below(NonZeroU64::new(n).unwrap());
    (0..count)
        .map(|_| {
            let mut c = [0; 3];
            for (i, v) in c.iter_mut().enumerate() {
                *v = at[i] + rng.range_i64(-side, side).unwrap();
            }
            p(c[0], c[1], c[2])
        })
        .collect()
}

#[test]
fn random_sets_are_never_clear_below_their_gap() {
    let seed = resolve("geom-spatial gjk random sets", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let (mut clears, mut undecided) = (0, 0);
    for case in 0..20_000 {
        // Small clouds at every scale up to the frame's edge, some apart and some overlapping.
        let unit = [1_i64, 1 << 10, 1 << 30, 1 << 34][case % 4];
        let side = 6 * unit;
        let shift = |rng: &mut SplitMix64| rng.range_i64(-30, 30).unwrap() * unit;
        let offset = [shift(&mut rng), shift(&mut rng), shift(&mut rng)];
        let a = cloud(&mut rng, [0, 0, 0], side, 8);
        let b = cloud(&mut rng, offset, side, 8);
        let reach = rng.below(NonZeroU64::new(60).unwrap()) * unit as u64;
        match filter(&a, &b, reach, &mut meter()).unwrap() {
            Filter::Clear(c) => {
                clears += 1;
                assert!(holds(&c, &a, &b, reach), "case {case}: {c:?}");
                assert!(c.proves(&a, &b, reach), "case {case}");
                assert!(every_pair_apart(&a, &b, reach), "case {case}");
            }
            Filter::Undecided => undecided += 1,
        }
    }
    assert!(clears > 5_000 && undecided > 1_000, "{clears} {undecided}");
}

#[test]
fn boxes_with_an_axis_gap_are_clear_up_to_it() {
    let seed = resolve("geom-spatial gjk axis gaps", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let below = |rng: &mut SplitMix64, n: u64| rng.below(NonZeroU64::new(n).unwrap()) as i64;
    for case in 0..5_000 {
        let axis = case % 3;
        let mut lo = [
            below(&mut rng, 50),
            below(&mut rng, 50),
            below(&mut rng, 50),
        ];
        let mut hi = lo.map(|v| v + 1 + below(&mut rng, 40));
        let a = cube(lo, hi);
        let gap = 1 + below(&mut rng, 100);
        let width = hi[axis] - lo[axis];
        lo[axis] = hi[axis] + gap;
        hi[axis] = lo[axis] + width;
        // Slide the second box across the other two axes, keeping their ranges overlapping.
        for other in (0..3).filter(|o| *o != axis) {
            let slide = below(&mut rng, 3) - 1;
            lo[other] += slide;
            hi[other] += slide;
        }
        let b = cube(lo, hi);
        let reach = gap as u64;
        let c = clear(&a, &b, reach).unwrap_or_else(|| panic!("case {case}"));
        assert!(holds(&c, &a, &b, reach), "case {case}");
        assert_eq!(clear(&a, &b, reach + 1), None, "case {case}");
    }
}
