//! Attacks on what piece G196a adds to `geom_num::one_root`: `Crossing2::of_extreme`, the ends of a
//! circle's horizontal diameter, where the 2D arrangement cuts an arc so that each piece runs one
//! way in x (chapter 14.4 of the kernel architecture).
//!
//! - **Exact points**: on a circle about a grid point of whole radius the ends are the grid points
//!   (cx ∓ r, cy), compared exactly.
//! - **On the circle, level with the centre**: for random circles about rational centres, each end
//!   is exactly on the circle and at the centre's height, the first left of the centre and the
//!   second right of it.
//! - **Extreme**: every point where a random carrier crosses the circle lies between the two in x;
//!   and the widest circle the budget allows still gives both, exactly on it.
//! - **Ellipses** (piece G196b, `Crossing2::of_ellipse_extreme`): a circle written as an ellipse has
//!   the circle's ends; a rotated ellipse's ends are on it, either side of its centre, beyond every
//!   crossing of a random carrier in x, and the widest ellipse the budget allows still has both.
//!
//! Seeds: the random test prints `seed: <hex>` and runs from a fixed default; `SPAREKIT_SEED=<hex>`
//! replays another (chapter 28.17). Case counts are a twentieth in debug builds.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
#![allow(
    clippy::many_single_char_names,
    reason = "x, y and w are a point's, l a carrier, r a root and z a crossing"
)]

use geom_num::arc::{Circle2, Contact, Disc, Round};
use geom_num::carrier::{Carrier, Rational2};
use geom_num::conic::Ellipse2;
use geom_num::grid::{COORD_MAX, P2};
use geom_num::one_root::{Crossing2, Root};
use geom_num::{I256, Sign};

/// The default seed of the random test here (G196a).
const DEFAULT_SEED: u64 = 0x6196_a000_5eed_196a;

/// `SplitMix64` (Steele, Lea and Flood, OOPSLA 2014), written out as the other tests do.
struct Rng(u64);

impl Rng {
    fn new() -> Rng {
        let seed = std::env::var("SPAREKIT_SEED")
            .ok()
            .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or(DEFAULT_SEED);
        println!("seed: {seed:x}");
        Rng(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A signed integer of at most `bits` bits, its size drawn too, so every scale is reached.
    fn signed(&mut self, bits: u32) -> i128 {
        let size = 1 + (self.next() % u64::from(bits)) as u32;
        let raw = (u128::from(self.next()) << 64) | u128::from(self.next());
        let magnitude = (raw >> (128 - size)) as i128;
        if self.next() & 1 == 0 {
            magnitude
        } else {
            -magnitude
        }
    }

    fn grid(&mut self) -> P2 {
        let mut c = || (self.signed(40) as i64).clamp(-COORD_MAX, COORD_MAX);
        P2::new(c(), c()).unwrap()
    }
}

fn point(x: i128, y: i128, w: i128) -> Crossing2 {
    Crossing2::of_point(&Rational2::new(x, y, w).unwrap()).unwrap()
}

fn ends(disc: &Disc) -> (Crossing2, Crossing2) {
    (
        Crossing2::of_extreme(disc, Root::First).unwrap(),
        Crossing2::of_extreme(disc, Root::Second).unwrap(),
    )
}

#[test]
fn a_whole_circle_about_a_grid_point_ends_at_grid_points() {
    for (cx, cy, r) in [(0, 0, 5), (-7, 3, 1), (100, -40, 13), (2, 2, 2)] {
        let c = Circle2::new(P2::new(cx, cy).unwrap(), i128::from(r)).unwrap();
        let (left, right) = ends(&Disc::of_circle(&c).unwrap());
        let (cx, cy, r) = (i128::from(cx), i128::from(cy), i128::from(r));
        assert!(left.same(&point(cx - r, cy, 1)).unwrap());
        assert!(right.same(&point(cx + r, cy, 1)).unwrap());
        assert!(!left.same(&right).unwrap());
    }
}

#[test]
fn each_end_is_on_the_circle_level_with_the_centre_and_beyond_every_crossing() {
    let mut rng = Rng::new();
    let cases = if cfg!(debug_assertions) { 100 } else { 2_000 };
    for case in 0..cases {
        // A circle through a grid point about a rational centre on a chord's bisector.
        let (on, other) = (rng.grid(), rng.grid());
        if on == other {
            continue;
        }
        let Ok(bisector) = geom_num::arc::Arc2::bisector(on, other) else {
            continue;
        };
        let Ok(across) = Carrier::through(on, rng.grid()) else {
            continue;
        };
        let Ok(centre) = bisector.meet(&across) else {
            continue;
        };
        let round = Round::new(on, centre);
        let Ok(disc) = Disc::of_round(&round) else {
            continue;
        };
        let (left, right) = ends(&disc);
        let ([x, y], w) = centre.coordinates();
        let middle = point(x, y, w);
        for (end, side) in [(&left, Sign::Negative), (&right, Sign::Positive)] {
            assert_eq!(
                end.against_disc(&disc).unwrap(),
                Sign::Zero,
                "case {case}: on it"
            );
            assert_eq!(
                end.against_round(&round).unwrap(),
                Sign::Zero,
                "case {case}"
            );
            assert_eq!(
                end.cmp_y(&middle).unwrap(),
                Sign::Zero,
                "case {case}: level"
            );
            assert_eq!(end.cmp_x(&middle).unwrap(), side, "case {case}: which side");
        }
        // Every crossing of a carrier with the circle lies between the two in x, the grid point
        // the circle was made through too.
        let made = point(i128::from(on.x.get()), i128::from(on.y.get()), 1);
        assert_ne!(made.cmp_x(&left).unwrap(), Sign::Negative, "case {case}");
        assert_ne!(made.cmp_x(&right).unwrap(), Sign::Positive, "case {case}");
        let Ok(l) = Carrier::through(rng.grid(), on) else {
            continue;
        };
        if round.meets(&l).unwrap() == Contact::Apart {
            continue;
        }
        for r in [Root::First, Root::Second] {
            let z = Crossing2::of_round(&l, &round, r).unwrap();
            assert_ne!(z.cmp_x(&left).unwrap(), Sign::Negative, "case {case}");
            assert_ne!(z.cmp_x(&right).unwrap(), Sign::Positive, "case {case}");
        }
    }
}

#[test]
fn the_widest_circle_in_the_budget_still_has_both_ends_exactly() {
    // A centre at the rational budget's corner and a squared radius just below 2^252.
    let x: i128 = (1 << 124) - 1;
    let w: i128 = (1 << 83) - 1;
    let centre = Rational2::new(-x, x, w).unwrap();
    let rr = I256::from_i128(i128::MAX)
        .checked_mul(&I256::from_i128(1 << 120))
        .unwrap();
    let disc = Disc::new(centre, rr).unwrap();
    let (left, right) = ends(&disc);
    for end in [&left, &right] {
        assert_eq!(end.against_disc(&disc).unwrap(), Sign::Zero);
    }
    assert_eq!(left.cmp_x(&right).unwrap(), Sign::Negative);
    assert_eq!(left.cmp_y(&right).unwrap(), Sign::Zero);
}

fn ellipse_ends(e: &Ellipse2) -> (Crossing2, Crossing2) {
    (
        Crossing2::of_ellipse_extreme(e, Root::First).unwrap(),
        Crossing2::of_ellipse_extreme(e, Root::Second).unwrap(),
    )
}

#[test]
fn a_circle_written_as_an_ellipse_has_the_circle_s_ends() {
    for (cx, cy, r) in [(0, 0, 5), (-7, 3, 1), (100, -40, 13)] {
        let c = Circle2::new(P2::new(cx, cy).unwrap(), i128::from(r)).unwrap();
        let (left, right) = ellipse_ends(&Ellipse2::of_circle(&c).unwrap());
        let (want_left, want_right) = ends(&Disc::of_circle(&c).unwrap());
        assert!(left.same(&want_left).unwrap() && right.same(&want_right).unwrap());
    }
}

#[test]
fn an_ellipse_s_ends_are_on_it_either_side_of_its_centre_and_beyond_every_crossing() {
    let mut rng = Rng::new();
    let cases = if cfg!(debug_assertions) { 100 } else { 2_000 };
    let mut checked = 0;
    for case in 0..cases {
        let centre = rng.grid();
        let major = [
            (rng.signed(41) as i64).clamp(-(1 << 40), 1 << 40),
            (rng.signed(41) as i64).clamp(-(1 << 40), 1 << 40),
        ];
        let q = 1 + (rng.next() % (1 << 21)) as i64;
        let p = 1 + (rng.next() % q as u64) as i64;
        let Ok(e) = Ellipse2::new(centre, major, p, q) else {
            continue;
        };
        let (left, right) = ellipse_ends(&e);
        let middle = point(i128::from(centre.x.get()), i128::from(centre.y.get()), 1);
        for (end, side) in [(&left, Sign::Negative), (&right, Sign::Positive)] {
            assert_eq!(
                end.against_ellipse(&e).unwrap(),
                Sign::Zero,
                "case {case}: on it"
            );
            assert_eq!(end.cmp_x(&middle).unwrap(), side, "case {case}: which side");
        }
        let Ok(l) = Carrier::through(rng.grid(), centre) else {
            continue;
        };
        if e.meets(&l).unwrap() == Contact::Apart {
            continue;
        }
        for r in [Root::First, Root::Second] {
            let z = Crossing2::of_ellipse(&l, &e, r).unwrap();
            assert_ne!(z.cmp_x(&left).unwrap(), Sign::Negative, "case {case}");
            assert_ne!(z.cmp_x(&right).unwrap(), Sign::Positive, "case {case}");
        }
        checked += 1;
    }
    assert!(checked > cases / 4, "{checked} of {cases} cases checked");
    // One worked by hand: M = (3, 4), p / q = 1 / 2, so the half-width is the square root of 13.
    let e = Ellipse2::new(P2::new(0, 0).unwrap(), [3, 4], 1, 2).unwrap();
    let (left, right) = ellipse_ends(&e);
    let [x, _] = right.enclosure();
    // The square root of 13 is 3.605551275...
    assert!(x.lo() <= 3.605_551_3 && 3.605_551_2 <= x.hi());
    assert_eq!(
        left.cmp_y(&right).unwrap(),
        Sign::Negative,
        "the tilt puts the left end lower"
    );
}

#[test]
fn the_widest_ellipse_in_the_budget_still_has_both_ends_exactly() {
    let m = (1_i64 << 41) - 1;
    let corner = P2::new(-COORD_MAX, COORD_MAX).unwrap();
    for (major, p, q) in [
        ([m, m], 1, 1 << 21),
        ([m, -m], (1 << 21) - 1, 1 << 21),
        ([m, 0], 1, 3),
    ] {
        let e = Ellipse2::new(corner, major, p, q).unwrap();
        let (left, right) = ellipse_ends(&e);
        for end in [&left, &right] {
            assert_eq!(end.against_ellipse(&e).unwrap(), Sign::Zero);
        }
        assert_eq!(left.cmp_x(&right).unwrap(), Sign::Negative);
    }
}
