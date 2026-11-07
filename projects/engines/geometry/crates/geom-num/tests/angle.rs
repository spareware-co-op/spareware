//! Angle units, bearings and their sines and cosines (`geom_num::angle`, piece G359, 17.2)
//! against exact arithmetic.
//!
//! - **The units are the ones 17.2 sets.** A turn is the least common multiple of 0.01″ and
//!   0.00001 gon; each unit name is the count it should be.
//! - **Exact arithmetic.** Sums and differences beyond `i64` are `None`; reduction is the exact
//!   remainder at both ends of `i64`; bearings turn, reverse and convert to IFC's angles exactly.
//! - **Sines and cosines enclose.** On the quarter turns they are exactly 0 and ±1; at every
//!   multiple of 30° and 45° the enclosure holds the exact algebraic value, decided in exact
//!   rationals; on random angles across a turn it holds the value of a Taylor series in exact
//!   rationals with π to 60 digits, and is at most a few ulps wide; and an angle and the same
//!   angle a whole number of turns away give the same bits.
//! - **The nearest unit is certified.** An enclosure of a whole number of units gives that unit,
//!   certain, bound 1; one of a half unit is not certain; an exact zero is exact.
//! - **Bearings of vectors.** Exact on the axes and diagonals at every scale; elsewhere the vector
//!   lies strictly between the directions one unit either side of the bearing, decided by the
//!   signs of interval cross products.
//!
//! Seeds: each randomised test prints `seed: <hex>` and runs from a fixed default;
//! `SPAREKIT_SEED=<hex>` replays another (chapter 28.17).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
#![allow(
    clippy::many_single_char_names,
    reason = "sines and cosines are s and c, degrees, minutes and seconds d, m and s"
)]

use geom_num::NumError;
use geom_num::angle::{
    AngleUnits, Bearing, FULL_TURN, HALF_TURN, PER_DEGREE, PER_GON, PER_MINUTE, PER_SECOND,
    QUARTER_TURN,
};
use geom_num::interval::Interval;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};

/// The default seed of every randomised test here.
const DEFAULT_SEED: u64 = 0x0359_a9e1_5eed_0001;

/// `SplitMix64` (Steele, Lea and Flood, OOPSLA 2014), as the crate's other tests write it.
struct Rng(u64);

impl Rng {
    fn for_test(name: &str) -> Rng {
        let seed = std::env::var("SPAREKIT_SEED")
            .ok()
            .and_then(|s| u64::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok())
            .unwrap_or(DEFAULT_SEED);
        println!("{name}: seed: {seed:016x}");
        Rng(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % ((hi - lo) as u64 + 1)) as i64
    }
}

/// A finite double as an exact rational.
fn rational(x: f64) -> BigRational {
    BigRational::from_float(x).unwrap()
}

/// Whether the interval holds `v`, exactly.
fn holds(i: Interval, v: &BigRational) -> bool {
    rational(i.lo()) <= *v && *v <= rational(i.hi())
}

/// Whether the interval holds `s · √(r)`, `s` the sign: `lo ≤ s√r ≤ hi`, decided by squaring.
fn holds_root(i: Interval, sign: i32, r: &BigRational) -> bool {
    let below = |x: &BigRational| {
        // x ≤ s√r
        if sign > 0 {
            !x.is_positive() || x * x <= *r
        } else {
            x.is_negative() && x * x >= *r
        }
    };
    let above = |x: &BigRational| {
        // s√r ≤ x
        if sign > 0 {
            x.is_positive() && x * x >= *r
        } else {
            !x.is_negative() || x * x <= *r
        }
    };
    below(&rational(i.lo())) && above(&rational(i.hi()))
}

/// The fixed-point scale of the oracle: values are integers times 10^−70.
fn scale() -> BigInt {
    BigInt::from(10).pow(70)
}

/// π to 71 significant digits, times 10^70 and truncated: within 10^−70 of π.
fn pi_fixed() -> BigInt {
    "31415926535897932384626433832795028841971693993751058209749445923078164"
        .parse()
        .unwrap()
}

/// sin x and cos x for |x| ≤ π given in fixed point, by their Taylor series in fixed point: 100
/// terms leave less than π^101 / 101! < 10^−100, and each of the 100 truncations loses less than
/// 10^−70, so both are within 10^−67.
fn taylor(x: &BigInt) -> (BigInt, BigInt) {
    let one = scale();
    let mut sin = BigInt::zero();
    let mut cos = BigInt::zero();
    let mut term = one.clone();
    for n in 0..100u32 {
        // term = x^n / n!, in fixed point.
        match n % 4 {
            0 => cos += &term,
            1 => sin += &term,
            2 => cos -= &term,
            _ => sin -= &term,
        }
        term = &term * x / &one / BigInt::from(n + 1);
    }
    (sin, cos)
}

/// The sine and cosine of `units`, within 10^−66, as exact rationals: the angle taken into
/// (−π, π] here, by its own remainder, and its series summed whole, with none of the octant
/// reduction under test.
fn oracle(units: i64) -> (BigRational, BigRational) {
    let mut r = units.rem_euclid(FULL_TURN);
    if r > HALF_TURN {
        r -= FULL_TURN;
    }
    let x = pi_fixed() * BigInt::from(r) / BigInt::from(HALF_TURN);
    let (s, c) = taylor(&x);
    (BigRational::new(s, scale()), BigRational::new(c, scale()))
}

#[test]
fn the_units_are_the_ones_the_survey_chapter_sets() {
    assert_eq!(PER_SECOND, 2_500);
    assert_eq!(PER_MINUTE, 60 * PER_SECOND);
    assert_eq!(PER_DEGREE, 60 * PER_MINUTE);
    assert_eq!(FULL_TURN, 3_240_000_000);
    assert_eq!(FULL_TURN, 360 * PER_DEGREE);
    assert_eq!(FULL_TURN, 400 * PER_GON);
    assert_eq!(QUARTER_TURN * 4, FULL_TURN);
    assert_eq!(HALF_TURN * 2, FULL_TURN);
    // 0.01″, 0.00001 gon and 0.000001° are whole numbers of units.
    assert_eq!(PER_SECOND % 100, 0);
    assert_eq!(PER_GON % 100_000, 0);
    assert_eq!(PER_DEGREE % 1_000_000, 0);
    // And no coarser unit has all three: 25, 81 and 9 units share no factor.
    let gcd = |mut a: i64, mut b: i64| {
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a
    };
    assert_eq!(
        gcd(
            gcd(PER_SECOND / 100, PER_GON / 100_000),
            PER_DEGREE / 1_000_000
        ),
        1
    );
}

#[test]
fn arithmetic_is_exact_and_overflow_is_none() {
    let a = AngleUnits(i64::MAX);
    assert_eq!(a.checked_add(AngleUnits(1)), None);
    assert_eq!(AngleUnits(i64::MIN).checked_sub(AngleUnits(1)), None);
    assert_eq!(AngleUnits(i64::MIN).checked_neg(), None);
    assert_eq!(
        AngleUnits(5).checked_add(AngleUnits(-7)),
        Some(AngleUnits(-2))
    );
    assert_eq!(
        AngleUnits(5).checked_sub(AngleUnits(-7)),
        Some(AngleUnits(12))
    );
    assert_eq!(AngleUnits(5).checked_neg(), Some(AngleUnits(-5)));
    assert_eq!(AngleUnits::ZERO.get(), 0);
    for v in [
        i64::MIN,
        i64::MIN + 1,
        -FULL_TURN - 1,
        -FULL_TURN,
        -1,
        0,
        1,
        FULL_TURN - 1,
        FULL_TURN,
        i64::MAX,
    ] {
        let r = AngleUnits(v).reduced().get();
        assert!((0..FULL_TURN).contains(&r), "{v}");
        assert_eq!(
            (i128::from(v) - i128::from(r)) % i128::from(FULL_TURN),
            0,
            "{v}"
        );
    }
}

#[test]
fn bearings_turn_reverse_and_convert_to_ifc_exactly() {
    assert_eq!(Bearing::NORTH.units(), AngleUnits(0));
    assert_eq!(
        Bearing::new(AngleUnits(-1)).units(),
        AngleUnits(FULL_TURN - 1)
    );
    assert_eq!(
        Bearing::new(AngleUnits(FULL_TURN + 5)).units(),
        AngleUnits(5)
    );
    let b = Bearing::new(AngleUnits(10 * PER_DEGREE));
    assert_eq!(
        b.rotated(AngleUnits(-20 * PER_DEGREE)).units(),
        AngleUnits(350 * PER_DEGREE)
    );
    assert_eq!(
        b.rotated(AngleUnits(i64::MAX)).units(),
        AngleUnits(10 * PER_DEGREE)
            .checked_add(AngleUnits(i64::MAX.rem_euclid(FULL_TURN)))
            .unwrap()
            .reduced()
    );
    assert_eq!(b.reversed().units(), AngleUnits(190 * PER_DEGREE));
    assert_eq!(b.reversed().reversed(), b);
    // IFC's angles run anticlockwise from east: east is bearing 90°, north is IFC 90°.
    assert_eq!(
        Bearing::from_ifc(AngleUnits(0)).units(),
        AngleUnits(QUARTER_TURN)
    );
    assert_eq!(Bearing::from_ifc(AngleUnits(QUARTER_TURN)), Bearing::NORTH);
    assert_eq!(
        Bearing::from_ifc(AngleUnits(-QUARTER_TURN)).units(),
        AngleUnits(HALF_TURN)
    );
    assert_eq!(Bearing::NORTH.to_ifc(), AngleUnits(QUARTER_TURN));
    let mut rng = Rng::for_test("bearings_turn_reverse_and_convert_to_ifc_exactly");
    for _ in 0..10_000 {
        let v = rng.next() as i64;
        let b = Bearing::new(AngleUnits(v));
        assert_eq!(Bearing::from_ifc(b.to_ifc()), b);
        assert_eq!(
            b.rotated(AngleUnits(v))
                .rotated(AngleUnits(-v.rem_euclid(FULL_TURN))),
            b
        );
        assert_eq!(b.east_north().unwrap(), b.units().sin_cos().unwrap());
    }
}

#[test]
fn quarter_turns_are_exact_points() {
    for (k, sin, cos) in [(0, 0.0, 1.0), (1, 1.0, 0.0), (2, 0.0, -1.0), (3, -1.0, 0.0)] {
        for turns in [-3i64, -1, 0, 1, 7] {
            let units = AngleUnits(k * QUARTER_TURN + turns * FULL_TURN);
            let (s, c) = units.sin_cos().unwrap();
            assert_eq!((s.lo(), s.hi()), (sin, sin), "{k} {turns}");
            assert_eq!((c.lo(), c.hi()), (cos, cos), "{k} {turns}");
        }
    }
}

#[test]
fn every_multiple_of_thirty_and_forty_five_degrees_holds_its_exact_value() {
    let half = BigRational::new(BigInt::from(1), BigInt::from(2));
    let three_quarters = BigRational::new(BigInt::from(3), BigInt::from(4));
    for k in -24..=24i64 {
        // 30° steps: sines and cosines in {0, ±½, ±√3/2, ±1}.
        let degrees = 30 * k;
        let (s, c) = AngleUnits(degrees * PER_DEGREE).sin_cos().unwrap();
        let check = |i: Interval, value_deg: i64, is_sin: bool| {
            // sin θ for θ a multiple of 30°: the oracle in exact form.
            let t = value_deg.rem_euclid(360);
            let t = if is_sin { t } else { (90 - t).rem_euclid(360) };
            let sign = if t < 180 { 1 } else { -1 };
            match t % 180 {
                0 => assert!(holds(i, &BigRational::zero()), "{value_deg} {is_sin}"),
                30 | 150 => assert!(
                    holds(i, &(&half * BigRational::from_integer(BigInt::from(sign)))),
                    "{value_deg} {is_sin}"
                ),
                60 | 120 => assert!(holds_root(i, sign, &three_quarters), "{value_deg}"),
                90 => assert!(holds(i, &BigRational::from_integer(BigInt::from(sign)))),
                other => panic!("{other}"),
            }
        };
        check(s, degrees, true);
        check(c, degrees, false);
        // 45° steps: odd multiples have ±√½.
        let degrees = 45 * k;
        let (s, c) = AngleUnits(degrees * PER_DEGREE).sin_cos().unwrap();
        if k % 2 != 0 {
            let t = degrees.rem_euclid(360);
            let sin_sign = if t < 180 { 1 } else { -1 };
            let cos_sign = if (90..=270).contains(&t) { -1 } else { 1 };
            assert!(holds_root(s, sin_sign, &half), "{degrees}");
            assert!(holds_root(c, cos_sign, &half), "{degrees}");
        }
    }
}

#[test]
fn random_angles_hold_the_taylor_series_and_are_a_few_ulps_wide() {
    let mut rng = Rng::for_test("random_angles_hold_the_taylor_series_and_are_a_few_ulps_wide");
    let cases = if cfg!(debug_assertions) { 300 } else { 3_000 };
    for i in 0..cases {
        let units = if i % 3 == 0 {
            rng.next() as i64
        } else {
            rng.range(-2 * FULL_TURN, 2 * FULL_TURN)
        };
        let (s, c) = AngleUnits(units).sin_cos().unwrap();
        let (os, oc) = oracle(units);
        assert!(holds(s, &os), "sin of {units}: [{}, {}]", s.lo(), s.hi());
        assert!(holds(c, &oc), "cos of {units}: [{}, {}]", c.lo(), c.hi());
        for e in [s, c] {
            assert!(e.hi() - e.lo() <= 8.0 * f64::EPSILON, "{units}: width");
        }
        // A whole number of turns away, the same bits.
        if let Some(moved) = units.checked_add(FULL_TURN * (1 + (rng.next() % 3) as i64)) {
            assert_eq!(AngleUnits(moved).sin_cos().unwrap(), (s, c));
        }
    }
}

#[test]
fn the_oracle_is_on_the_unit_circle() {
    for units in [
        0,
        1,
        FULL_TURN / 8,
        QUARTER_TURN - 1,
        HALF_TURN,
        HALF_TURN + 1,
        -1,
    ] {
        let (s, c) = oracle(units);
        let one = &s * &s + &c * &c - BigRational::one();
        assert!(one.abs() < BigRational::new(BigInt::from(1), BigInt::from(10).pow(60)));
    }
    let (s, c) = oracle(FULL_TURN / 12);
    let half = BigRational::new(BigInt::from(1), BigInt::from(2));
    assert!((s - &half).abs() < BigRational::new(BigInt::from(1), BigInt::from(10).pow(60)));
    assert!(c > half);
}

/// An enclosure, in radians, of `units` angle units: `units · π / HALF_TURN`.
fn radians(units: f64) -> Interval {
    Interval::point(units)
        .unwrap()
        .mul(Interval::PI)
        .unwrap()
        .div(Interval::point(HALF_TURN as f64).unwrap())
        .unwrap()
}

#[test]
fn the_nearest_unit_is_certain_away_from_half_units_and_says_so_at_them() {
    let mut rng = Rng::for_test("the_nearest_unit_is_certain_away_from_half_units");
    for _ in 0..20_000 {
        let units = rng.range(-HALF_TURN, HALF_TURN);
        let near = AngleUnits::nearest_radians(radians(units as f64)).unwrap();
        assert_eq!(near.value(), AngleUnits(units));
        assert!(near.is_certain() && !near.is_exact());
        assert_eq!(near.bound(), 1);
        let half = AngleUnits::nearest_radians(radians(units as f64 + 0.5)).unwrap();
        assert!(!half.is_certain(), "{units}.5");
        assert!([units, units + 1].contains(&half.value().get()));
        assert_eq!(half.bound(), 1);
    }
    let zero = AngleUnits::nearest_radians(Interval::point(0.0).unwrap()).unwrap();
    assert!(zero.is_exact() && zero.is_certain());
    assert_eq!(zero.value(), AngleUnits::ZERO);
    // A wide enclosure: the midpoint's unit, with the distance to the farther end.
    let wide = AngleUnits::nearest_radians(Interval::new(0.0, 3.0).unwrap()).unwrap();
    assert!(!wide.is_certain());
    let k = wide.value().get() as f64;
    assert!(wide.bound() as f64 >= k && wide.bound() as f64 <= k + 2.0);
    // Beyond ±2^62 units, refused.
    assert_eq!(
        AngleUnits::nearest_radians(Interval::point(1e10).unwrap()),
        Err(NumError::OutsideDomain)
    );
    assert_eq!(
        AngleUnits::nearest_radians(Interval::point(-1e10).unwrap()),
        Err(NumError::OutsideDomain)
    );
}

#[test]
fn bearings_of_axes_and_diagonals_are_exact_at_every_scale() {
    let eighth = FULL_TURN / 8;
    for scale in [1i64, 7, 1 << 20, (1 << 41) - 3, 1 << 53] {
        for (k, e, n) in [
            (0, 0, 1),
            (1, 1, 1),
            (2, 1, 0),
            (3, 1, -1),
            (4, 0, -1),
            (5, -1, -1),
            (6, -1, 0),
            (7, -1, 1),
        ] {
            let b = Bearing::of_vector(e * scale, n * scale).unwrap();
            assert!(b.is_exact() && b.is_certain());
            assert_eq!(b.bound(), 0);
            assert_eq!(b.value().units(), AngleUnits(k * eighth), "{e} {n} {scale}");
        }
    }
    assert_eq!(Bearing::of_vector(0, 0), Err(NumError::Degenerate));
    let over = (1i64 << 53) + 1;
    assert_eq!(
        Bearing::of_vector(over, 1),
        Err(NumError::OutOfRange {
            value: i128::from(over)
        })
    );
    assert_eq!(
        Bearing::of_vector(1, -over),
        Err(NumError::OutOfRange {
            value: -i128::from(over)
        })
    );
}

/// The sign of `sin(b) · north − cos(b) · east`, the cross product of the direction of bearing `b`
/// with the vector, when certain: negative when the vector is clockwise of the bearing.
fn side_of(b: i64, east: i64, north: i64) -> Option<std::cmp::Ordering> {
    let (s, c) = AngleUnits(b).sin_cos().unwrap();
    let e = Interval::point(east as f64).unwrap();
    let n = Interval::point(north as f64).unwrap();
    let z = s.mul(n).unwrap().sub(c.mul(e).unwrap()).unwrap();
    if z.hi() < 0.0 {
        Some(std::cmp::Ordering::Less)
    } else if z.lo() > 0.0 {
        Some(std::cmp::Ordering::Greater)
    } else {
        None
    }
}

#[test]
fn a_vector_lies_between_the_directions_a_unit_either_side_of_its_bearing() {
    let mut rng = Rng::for_test("a_vector_lies_between_the_directions_a_unit_either_side");
    let cases = if cfg!(debug_assertions) {
        3_000
    } else {
        30_000
    };
    for i in 0..cases {
        let reach = [1 << 10, 1 << 25, 1 << 41][i % 3];
        let (e, n) = (rng.range(-reach, reach), rng.range(-reach, reach));
        if e == 0 || n == 0 || e.abs() == n.abs() {
            continue;
        }
        let b = Bearing::of_vector(e, n).unwrap();
        assert_eq!(b.bound(), 1);
        let u = b.value().units().get();
        assert!((0..FULL_TURN).contains(&u));
        assert_eq!(
            side_of(u - 1, e, n),
            Some(std::cmp::Ordering::Less),
            "{e} {n}"
        );
        assert_eq!(
            side_of(u + 1, e, n),
            Some(std::cmp::Ordering::Greater),
            "{e} {n}"
        );
    }
}
