//! Piece G413's attack: tries to break every claim of the exchange basics (the unit table,
//! numbers to grid, reals, written decimals, map conversions) against an exact rational model
//! (`num-bigint`, `num-rational`) that never calls the code under test, on hand-picked edges and on
//! inputs from a printed seed (`SPAREKIT_SEED` replays one).

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal,
    clippy::float_cmp,
    clippy::cast_lossless,
    clippy::disallowed_methods,
    clippy::semicolon_if_nothing_returned,
    reason = "test cases and oracles, named as in the formulas they check; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::indexing_slicing,
    reason = "oracles and generators; the rules are for kernel code"
)]

use geom_exchange::ExchangeError;
use geom_exchange::grid::{self, Gridded};
use geom_exchange::map::{MapConversion, place};
use geom_exchange::real;
use geom_exchange::unit::{self, AngleUnit, NamedLength, SiPrefix};
use geom_exchange::write;
use geom_math::frame::{ORIGIN_MAX, steps_per_metre};
use geom_math::{CrsId, Decimal, FrameKind, LinearUnit, Scale};
use geom_num::grid::{COORD_MAX, GridStep};
use geom_test::seed::{self, Seed};
use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};

/// splitmix64: a test generator, outside the kernel.
struct Rng(u64);

impl Rng {
    fn new(label: &str, default: u64) -> Rng {
        Rng(seed::resolve(label, Seed(default)).unwrap().get())
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u64) as usize]
    }
}

const STEPS: [GridStep; 3] = [GridStep::Q0, GridStep::Q1, GridStep::Q2];

fn big(v: i128) -> BigInt {
    BigInt::from(v)
}

fn ratio(n: i128, d: i128) -> BigRational {
    BigRational::new(big(n), big(d))
}

fn of_decimal(d: Decimal) -> BigRational {
    let m = BigRational::from_integer(big(d.significand()));
    let ten = BigRational::from_integer(BigInt::from(10));
    if d.exponent() >= 0 {
        m * num_traits::pow(ten, d.exponent() as usize)
    } else {
        m / num_traits::pow(ten, d.exponent().unsigned_abs() as usize)
    }
}

fn of_unit(u: LinearUnit) -> BigRational {
    ratio(i128::from(u.num()), i128::from(u.den()))
}

fn per_metre(step: GridStep) -> BigRational {
    BigRational::from_integer(BigInt::from(steps_per_metre(step)))
}

/// The value rounded to the nearest integer, ties to even, and whether it was whole.
fn round_half_even(v: &BigRational) -> (BigInt, bool) {
    let floor = v.floor();
    let frac = v - &floor;
    let half = ratio(1, 2);
    let f = floor.to_integer();
    if frac.is_zero() {
        return (f, true);
    }
    let up = frac > half || (frac == half && f.is_odd());
    (if up { f + 1 } else { f }, false)
}

/// The oracle's count for `value` units of `unit` at `step`.
fn expected(value: &BigRational, unit: LinearUnit, step: GridStep) -> (BigInt, bool) {
    round_half_even(&(value * of_unit(unit) * per_metre(step)))
}

fn check_count(got: Result<Gridded, ExchangeError>, want: &(BigInt, bool), limit: i64, what: &str) {
    let (count, exact) = want;
    if count.abs() <= BigInt::from(limit) {
        let g = got.unwrap_or_else(|e| panic!("{what}: {e:?}, expected {count}"));
        assert_eq!(BigInt::from(g.count()), *count, "{what}");
        assert_eq!(g.is_exact(), *exact, "{what}");
    } else {
        match got {
            Err(ExchangeError::OutOfRange { count: c }) => {
                assert_eq!(BigInt::from(c), *count, "{what}")
            }
            other => panic!("{what}: {other:?}, expected out of range at {count}"),
        }
    }
}

fn units(rng: &mut Rng) -> LinearUnit {
    let fixed = [
        LinearUnit::METRE,
        unit::metre(Some(SiPrefix::Milli)).unwrap(),
        NamedLength::Inch.unit().unwrap(),
        NamedLength::Foot.unit().unwrap(),
        NamedLength::UsSurveyFoot.unit().unwrap(),
        LinearUnit::new(1_523_986_327, 5_000_000_000).unwrap(),
    ];
    if rng.below(4) == 0 {
        LinearUnit::new(1 + rng.below(1 << 20), 1 + rng.below(1 << 20)).unwrap()
    } else {
        rng.pick(&fixed)
    }
}

fn decimal(rng: &mut Rng) -> Decimal {
    let digits = 1 + rng.below(17) as u32;
    let m = (rng.next() % 10_u64.pow(digits.min(19))) as i128;
    let m = if rng.below(2) == 0 { -m } else { m };
    Decimal::new(m, rng.range(-14, 4) as i32).unwrap()
}

// ---------------------------------------------------------------------------------------------
// The unit table.

#[test]
fn every_unit_is_its_definition_exactly() {
    // NIST SP 811, Appendix B: 1 in = 0.0254 m, 1 ft = 0.3048 m, 1 yd = 0.9144 m, 1 mi =
    // 1609.344 m, 1 mil = 0.001 in; the US survey foot = 1200/3937 m.
    let want = [
        (NamedLength::Inch, ratio(254, 10_000)),
        (NamedLength::Foot, ratio(3048, 10_000)),
        (NamedLength::Yard, ratio(9144, 10_000)),
        (NamedLength::Mile, ratio(1_609_344, 1_000)),
        (NamedLength::Thou, ratio(254, 10_000_000)),
        (NamedLength::UsSurveyFoot, ratio(1200, 3937)),
    ];
    for (u, r) in want {
        assert_eq!(of_unit(u.unit().unwrap()), r, "{u:?}");
    }
    // The foot is twelve inches, the yard three feet, the mile 5,280 feet, exactly.
    let inch = of_unit(NamedLength::Inch.unit().unwrap());
    assert_eq!(
        of_unit(NamedLength::Foot.unit().unwrap()),
        &inch * BigInt::from(12)
    );
    assert_eq!(
        of_unit(NamedLength::Yard.unit().unwrap()),
        &inch * BigInt::from(36)
    );
    assert_eq!(
        of_unit(NamedLength::Mile.unit().unwrap()),
        &inch * BigInt::from(63_360)
    );
}

#[test]
fn every_prefix_is_its_power_of_ten_or_refused() {
    for p in SiPrefix::ALL {
        let power = p.power();
        let exact = if power >= 0 {
            BigRational::from_integer(num_traits::pow(BigInt::from(10), power as usize))
        } else {
            BigRational::new(
                BigInt::one(),
                num_traits::pow(BigInt::from(10), power.unsigned_abs() as usize),
            )
        };
        // A frame's linear unit lies within 2^-32 to 2^32 metres: giga to nano.
        let inside = (-9..=9).contains(&power);
        match unit::metre(Some(p)) {
            Ok(u) => {
                assert!(inside, "{p:?}");
                assert_eq!(of_unit(u), exact, "{p:?}");
            }
            Err(e) => {
                assert!(!inside, "{p:?}");
                assert_eq!(e, ExchangeError::UnitOutOfRange);
            }
        }
    }
    assert_eq!(unit::metre(None).unwrap(), LinearUnit::METRE);
}

#[test]
fn a_conversion_based_unit_is_the_files_factor_exactly() {
    // STEP's inch: CONVERSION_BASED_UNIT('INCH', MEASURE_WITH_UNIT(LENGTH_MEASURE(25.4), mm)).
    let mm = unit::metre(Some(SiPrefix::Milli)).unwrap();
    let inch = unit::conversion_based(Decimal::parse("25.4").unwrap(), mm).unwrap();
    assert_eq!(inch, NamedLength::Inch.unit().unwrap());
    // A survey foot given as 0.3048006096 m is not the US survey foot (1200/3937): the file's
    // number decides, so it is held as written.
    let given =
        unit::conversion_based(Decimal::parse("0.3048006096").unwrap(), LinearUnit::METRE).unwrap();
    assert_eq!(of_unit(given), ratio(3_048_006_096, 10_000_000_000));
    assert_ne!(given, NamedLength::UsSurveyFoot.unit().unwrap());
    // Zero and negative factors, and products outside the range, are refused.
    assert_eq!(
        unit::conversion_based(Decimal::ZERO, mm),
        Err(ExchangeError::ScaleOutOfRange)
    );
    assert_eq!(
        unit::conversion_based(Decimal::parse("-1").unwrap(), mm),
        Err(ExchangeError::ScaleOutOfRange)
    );
    assert_eq!(
        unit::conversion_based(Decimal::parse("1e-7").unwrap(), mm),
        Err(ExchangeError::UnitOutOfRange)
    );
}

#[test]
fn the_steps_per_unit_are_chapter_24s_table() {
    let per = |u: LinearUnit, s: GridStep| unit::steps_per(u, s).unwrap();
    let mm = unit::metre(Some(SiPrefix::Milli)).unwrap();
    assert_eq!(per(mm, GridStep::Q0).whole(), Some(6_400));
    assert_eq!(per(mm, GridStep::Q1).whole(), Some(160_000));
    assert_eq!(
        per(LinearUnit::METRE, GridStep::Q0).whole(),
        Some(6_400_000)
    );
    let inch = NamedLength::Inch.unit().unwrap();
    assert_eq!(per(inch, GridStep::Q0).whole(), Some(162_560));
    assert_eq!(162_560, (1 << 8) * 5 * 127);
    assert_eq!(per(inch, GridStep::Q1).whole(), Some(4_064_000));
    let foot = NamedLength::Foot.unit().unwrap();
    assert_eq!(per(foot, GridStep::Q0).whole(), Some(1_950_720));
    let survey = per(NamedLength::UsSurveyFoot.unit().unwrap(), GridStep::Q0);
    assert_eq!((survey.num(), survey.den()), (7_680_000_000, 3_937));
    assert_eq!(survey.whole(), None);
    // Every exactness claim of 24.8's table, through numbers to grid: a decimal inch is exact at q0
    // only in whole 1/1,280 inches; a foot to 0.1 ft; millimetres to 0.01 mm; a US survey foot
    // only in multiples of 3,937 feet; at q1, the inch to 0.001 and the foot to 0.001.
    let exact = |text: &str, u: LinearUnit, s: GridStep| {
        grid::length(Decimal::parse(text).unwrap(), u, s)
            .unwrap()
            .is_exact()
    };
    assert!(exact("0.00078125", inch, GridStep::Q0)); // 1/1,280 inch
    assert!(exact("0.05", inch, GridStep::Q0));
    assert!(exact("0.00390625", inch, GridStep::Q0)); // 1/256 inch
    assert!(!exact("0.001", inch, GridStep::Q0));
    assert!(exact("0.001", inch, GridStep::Q1));
    assert!(exact("0.1", foot, GridStep::Q0));
    assert!(!exact("0.01", foot, GridStep::Q0));
    assert!(exact("0.001", foot, GridStep::Q1));
    assert!(exact("1234.56", mm, GridStep::Q0));
    let us = NamedLength::UsSurveyFoot.unit().unwrap();
    assert!(exact("3937", us, GridStep::Q0));
    assert!(!exact("1", us, GridStep::Q0));
    assert!(!exact("3936", us, GridStep::Q1));
}

#[test]
fn angle_units_are_whole_counts_and_the_radian_is_not() {
    assert_eq!(unit::ANGLE_UNITS_PER_TURN, 360 * 3_600 * 2_500);
    for u in AngleUnit::ALL {
        let per_turn = match u {
            AngleUnit::Degree => Some(360),
            AngleUnit::Gon => Some(400),
            AngleUnit::ArcMinute => Some(360 * 60),
            AngleUnit::ArcSecond => Some(360 * 3_600),
            AngleUnit::Radian => None,
        };
        match (u.angle_units(), per_turn) {
            (Some(k), Some(n)) => assert_eq!(k * n, unit::ANGLE_UNITS_PER_TURN, "{u:?}"),
            (None, None) => {}
            other => panic!("{u:?}: {other:?}"),
        }
    }
    // Decimal degrees to six places, gon to five and arc-seconds to 0.01 are exact (17.2).
    let a = |t: &str, u: AngleUnit| grid::angle(Decimal::parse(t).unwrap(), u).unwrap();
    assert_eq!(a("12.345678", AngleUnit::Degree).count(), 111_111_102);
    assert!(a("12.345678", AngleUnit::Degree).is_exact());
    assert!(a("123.45678", AngleUnit::Gon).is_exact());
    assert!(a("56.78", AngleUnit::ArcSecond).is_exact());
    assert!(!a("0.0000001", AngleUnit::Degree).is_exact());
    assert_eq!(a("0.00000005", AngleUnit::Degree).count(), 0); // 0.45 of a unit
    assert_eq!(a("90", AngleUnit::Degree).count(), 810_000_000);
    assert_eq!(
        grid::angle(Decimal::parse("1").unwrap(), AngleUnit::Radian),
        Err(ExchangeError::InexactAngleUnit)
    );
    assert_eq!(
        grid::angle_of_double(0.5, AngleUnit::Radian),
        Err(ExchangeError::InexactAngleUnit)
    );
}

// ---------------------------------------------------------------------------------------------
// Numbers to grid.

#[test]
fn decimal_lengths_round_once_half_to_even_as_the_exact_model_does() {
    let mut rng = Rng::new("g413 decimal lengths", 0x6413_0001);
    for i in 0..4_000 {
        let (u, s, d) = (units(&mut rng), rng.pick(&STEPS), decimal(&mut rng));
        let want = expected(&of_decimal(d), u, s);
        check_count(
            grid::length(d, u, s),
            &want,
            COORD_MAX,
            &format!("{i}: {d} in {u:?} at {s:?}"),
        );
    }
}

#[test]
fn doubles_are_taken_at_their_exact_value_and_rounded_once() {
    let mut rng = Rng::new("g413 double lengths", 0x6413_0002);
    for i in 0..4_000 {
        let (u, s) = (units(&mut rng), rng.pick(&STEPS));
        let x = (rng.next() as i64 as f64) * 2f64.powi(rng.range(-90, -40) as i32);
        let want = expected(&BigRational::from_float(x).unwrap(), u, s);
        check_count(
            grid::length_of_double(x, u, s),
            &want,
            COORD_MAX,
            &format!("{i}: {x:e}"),
        );
    }
    assert_eq!(
        grid::length_of_double(f64::NAN, LinearUnit::METRE, GridStep::Q0),
        Err(ExchangeError::NotFinite)
    );
    assert_eq!(
        grid::length_of_double(f64::INFINITY, LinearUnit::METRE, GridStep::Q0),
        Err(ExchangeError::NotFinite)
    );
}

#[test]
fn ties_go_to_the_even_count_both_ways() {
    let mm = unit::metre(Some(SiPrefix::Milli)).unwrap();
    // 1/12,800 mm is half a q0 step: 0.5 rounds to 0, 1.5 to 2, -1.5 to -2, 2.5 to 2.
    for (text, count) in [
        ("0.000078125", 0),
        ("0.000234375", 2),
        ("-0.000234375", -2),
        ("0.000390625", 2),
    ] {
        let g = grid::length(Decimal::parse(text).unwrap(), mm, GridStep::Q0).unwrap();
        assert_eq!((g.count(), g.is_exact()), (count, false), "{text}");
    }
}

#[test]
fn a_positive_length_that_rounds_to_zero_names_the_finer_step() {
    let mm = unit::metre(Some(SiPrefix::Milli)).unwrap();
    let p = |t: &str, s| grid::positive_length(Decimal::parse(t).unwrap(), mm, s);
    // 0.00005 mm is 0.32 of a q0 step, 8 q1 steps.
    assert_eq!(
        p("0.00005", GridStep::Q0),
        Err(ExchangeError::RoundsToZero {
            finer: Some(GridStep::Q1)
        })
    );
    // 0.000002 mm is 0.0128 q0, 0.32 q1, 8 q2 steps.
    assert_eq!(
        p("0.000002", GridStep::Q0),
        Err(ExchangeError::RoundsToZero {
            finer: Some(GridStep::Q2)
        })
    );
    // 0.0000001 mm is 0.4 of a q2 step: nothing holds it.
    assert_eq!(
        p("0.0000001", GridStep::Q0),
        Err(ExchangeError::RoundsToZero { finer: None })
    );
    assert_eq!(p("0", GridStep::Q0), Err(ExchangeError::NotPositive));
    assert_eq!(p("-1", GridStep::Q0), Err(ExchangeError::NotPositive));
    assert_eq!(p("1", GridStep::Q0).unwrap().count(), 6_400);
    let d = |x: f64| grid::positive_length_of_double(x, mm, GridStep::Q0);
    assert_eq!(d(0.0), Err(ExchangeError::NotPositive));
    assert_eq!(d(-0.0), Err(ExchangeError::NotPositive));
    assert_eq!(d(-2.0), Err(ExchangeError::NotPositive));
    assert_eq!(d(f64::NAN), Err(ExchangeError::NotFinite));
    assert_eq!(
        d(0.00005),
        Err(ExchangeError::RoundsToZero {
            finer: Some(GridStep::Q1)
        })
    );
    assert_eq!(d(0.5).unwrap().count(), 3_200);
}

#[test]
fn map_coordinates_reach_a_frame_origins_range_and_say_when_they_are_exact() {
    let mut rng = Rng::new("g413 map coordinates", 0x6413_0003);
    // NZTM2000 northings near 5,921,000 m: about 2^45 steps, past a coordinate's 2^40.
    for (text, exact) in [
        ("5921000", true),
        ("5921000.1234", true),
        ("5921000.12345678", false),
    ] {
        let d = Decimal::parse(text).unwrap();
        let g = grid::map_coordinate(d, LinearUnit::METRE, GridStep::Q0).unwrap();
        let want = expected(&of_decimal(d), LinearUnit::METRE, GridStep::Q0);
        assert_eq!(BigInt::from(g.count()), want.0, "{text}");
        assert_eq!(g.is_exact(), exact, "{text}");
        assert!(matches!(
            grid::length(d, LinearUnit::METRE, GridStep::Q0),
            Err(ExchangeError::OutOfRange { .. })
        ));
    }
    for i in 0..2_000 {
        let (u, s) = (units(&mut rng), rng.pick(&STEPS));
        let m = rng.next() as i64 as i128;
        let d = Decimal::new(m, rng.range(-12, 0) as i32).unwrap();
        let want = expected(&of_decimal(d), u, s);
        check_count(
            grid::map_coordinate(d, u, s),
            &want,
            ORIGIN_MAX,
            &format!("{i}: {d}"),
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Reals.

/// The text of a shortest decimal, read back through this crate's own parser.
fn round_trip(x: f64) {
    let s = real::shortest(x).unwrap();
    let text = s.to_string();
    let back = real::read(&text).unwrap();
    assert_eq!(back.to_bits(), x.to_bits(), "{x:e} wrote {text}");
    assert_eq!(s.to_f64().unwrap().to_bits(), x.to_bits(), "{x:e}");
    // Shortest: the nearest decimal of one digit fewer does not read back (if any did, the
    // nearest would), as the core library's correctly rounded formatting gives it.
    let digits = s.decimal().significand().unsigned_abs().to_string().len();
    if digits > 1 {
        let fewer = format!("{:.*e}", digits - 2, x);
        match real::read(&fewer) {
            Ok(y) => assert_ne!(y.to_bits(), x.to_bits(), "{x:e}: {fewer}"),
            // Rounded past the largest finite double: it does not read back either.
            Err(ExchangeError::RealOutOfRange) => assert!(x.abs() > 1e308, "{x:e}"),
            Err(e) => panic!("{x:e}: {fewer}: {e:?}"),
        }
    }
}

#[test]
fn every_double_writes_shortest_and_reads_back_bit_for_bit() {
    for x in [
        0.0,
        -0.0,
        f64::MIN_POSITIVE,
        -f64::MIN_POSITIVE,
        f64::from_bits(1),
        -f64::from_bits(1),
        f64::from_bits(0x000F_FFFF_FFFF_FFFF),
        f64::MAX,
        f64::MIN,
        f64::EPSILON,
        0.1,
        0.3,
        1.0 / 3.0,
        123_456_789.012_345_68,
        9_007_199_254_740_993.0,
        5e-324,
        2.225_073_858_507_201e-308,
    ] {
        round_trip(x);
    }
    let mut rng = Rng::new("g413 doubles", 0x6413_0004);
    for _ in 0..200_000 {
        let x = f64::from_bits(rng.next());
        if x.is_finite() {
            round_trip(x);
        }
    }
    // Every subnormal exponent and the edges of every binade near the ends.
    for k in 1..64_u32 {
        for bits in [1_u64 << (k - 1), (1_u64 << k) - 1, u64::MAX >> (64 - k)] {
            let x = f64::from_bits(bits);
            if x.is_finite() {
                round_trip(x);
            }
        }
    }
    assert!(real::read("-0").unwrap().is_sign_negative());
    assert!(real::read("-0.000e12").unwrap().is_sign_negative());
    assert!(!real::read("0").unwrap().is_sign_negative());
    assert_eq!(real::shortest(-0.0).unwrap().to_string(), "-0");
    assert!(real::shortest(-0.0).unwrap().is_negative_zero());
    assert_eq!(real::shortest(f64::NAN), Err(ExchangeError::NotFinite));
    assert_eq!(
        real::shortest(f64::NEG_INFINITY),
        Err(ExchangeError::NotFinite)
    );
    assert_eq!(real::read("1e309"), Err(ExchangeError::RealOutOfRange));
    assert_eq!(real::read("-1.8e308"), Err(ExchangeError::RealOutOfRange));
    assert!(matches!(real::read("1,5"), Err(ExchangeError::Decimal(_))));
}

#[test]
fn a_decimal_of_at_most_fifteen_digits_comes_back_as_written() {
    let mut rng = Rng::new("g413 fifteen digits", 0x6413_0005);
    for _ in 0..50_000 {
        let digits = 1 + rng.below(15) as u32;
        let m = (rng.next() % 10_u64.pow(digits)) as i128;
        let d = Decimal::new(
            if rng.below(2) == 0 { -m } else { m },
            rng.range(-300, 290) as i32,
        )
        .unwrap();
        let x = real::nearest(d).unwrap();
        assert_eq!(real::shortest(x).unwrap().decimal(), d, "{d}");
    }
}

#[test]
fn nearest_is_the_double_nearest_the_exact_decimal() {
    let mut rng = Rng::new("g413 nearest", 0x6413_0006);
    for _ in 0..20_000 {
        let m = (u128::from(rng.next()) << 40 | u128::from(rng.next() >> 24)) as i128;
        let d = Decimal::new(m % 10_i128.pow(30), rng.range(-330, 270) as i32).unwrap();
        let Ok(x) = real::nearest(d) else { continue };
        let exact = of_decimal(d);
        let xr = BigRational::from_float(x).unwrap();
        // No neighbour of x is nearer the exact value; on a tie x is even.
        for n in [x.next_up(), x.next_down()] {
            if n.is_finite() {
                let nr = BigRational::from_float(n).unwrap();
                let (dx, dn) = ((&xr - &exact).abs(), (&nr - &exact).abs());
                assert!(dx < dn || (dx == dn && x.to_bits() & 1 == 0), "{d}: {x:e}");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Written decimals.

/// The ends of the decimals that read back to `n ≥ 0` steps of `unit` at `step`, in units, and
/// whether they belong to it (a tie reads back to the even count).
fn interval(n: u64, unit: LinearUnit, step: GridStep) -> (BigRational, BigRational, bool) {
    let per = of_unit(unit) * per_metre(step);
    let n = BigRational::from_integer(BigInt::from(n));
    let lo = (&n - ratio(1, 2)) / &per;
    let hi = (&n + ratio(1, 2)) / &per;
    (lo, hi, n.to_integer().is_even())
}

fn within(v: &BigRational, lo: &BigRational, hi: &BigRational, closed: bool) -> bool {
    if closed {
        lo <= v && v <= hi
    } else {
        lo < v && v < hi
    }
}

fn power_of_ten(k: i32) -> BigRational {
    let ten = BigRational::from_integer(BigInt::from(10));
    if k >= 0 {
        num_traits::pow(ten, k as usize)
    } else {
        BigRational::one() / num_traits::pow(ten, k.unsigned_abs() as usize)
    }
}

fn check_written(n: i64, unit: LinearUnit, step: GridStep) {
    let w = write::shortest(n, unit, step).unwrap_or_else(|e| panic!("{n}: {e:?}"));
    let d = w.decimal();
    let v = of_decimal(d);
    // It reads back.
    let back = if n.unsigned_abs() <= COORD_MAX as u64 {
        grid::length(d, unit, step)
    } else {
        grid::map_coordinate(d, unit, step)
    }
    .unwrap();
    assert_eq!(back.count(), n, "{n} {unit:?} {step:?} wrote {d}");
    // Its deviation is exact and at most half a step.
    let per = of_unit(unit) * per_metre(step);
    let dev = &v * &per - BigRational::from_integer(BigInt::from(n));
    let got = ratio(w.deviation().num(), w.deviation().den() as i128);
    assert_eq!(got, dev, "{n} wrote {d}");
    assert!(dev.abs() <= ratio(1, 2));
    assert_eq!(w.is_exact(), dev.is_zero());
    if n == 0 {
        assert!(d.is_zero());
        return;
    }
    // Shortest: no multiple of the next coarser place reads back (by symmetry, in magnitudes).
    let (lo, hi, closed) = interval(n.unsigned_abs(), unit, step);
    let coarser = power_of_ten(d.exponent() + 1);
    let first = (&lo / &coarser).ceil();
    for k in 0..2 {
        let c = (&first + BigRational::from_integer(BigInt::from(k))) * &coarser;
        assert!(
            !within(&c, &lo, &hi, closed),
            "{n} {unit:?} {step:?}: {c} is coarser than {d}"
        );
    }
    // Closest at its place: no neighbour at the same place that reads back is nearer.
    let place = power_of_ten(d.exponent());
    let (va, exact) = (
        v.abs(),
        BigRational::from_integer(BigInt::from(n.unsigned_abs())) / &per,
    );
    for neighbour in [&va + &place, &va - &place] {
        if within(&neighbour, &lo, &hi, closed) {
            let (dn, dv) = ((&neighbour - &exact).abs(), (&va - &exact).abs());
            let even = (&neighbour / &place).to_integer().is_even();
            assert!(
                dn > dv || (dn == dv && !even),
                "{n}: {neighbour} nearer than {d}"
            );
        }
    }
}

#[test]
fn shortest_decimals_read_back_and_are_shortest_and_closest() {
    let mut rng = Rng::new("g413 written", 0x6413_0007);
    for &n in &[
        0,
        1,
        -1,
        2,
        7,
        162_559,
        162_560,
        1_950_720,
        COORD_MAX,
        -COORD_MAX,
        ORIGIN_MAX,
        -ORIGIN_MAX,
    ] {
        for s in STEPS {
            for u in [
                LinearUnit::METRE,
                NamedLength::Inch.unit().unwrap(),
                NamedLength::UsSurveyFoot.unit().unwrap(),
            ] {
                check_written(n, u, s);
            }
        }
    }
    for _ in 0..6_000 {
        let (u, s) = (units(&mut rng), rng.pick(&STEPS));
        let n = if rng.below(3) == 0 {
            rng.range(-1_000, 1_000)
        } else if rng.below(2) == 0 {
            rng.range(-COORD_MAX, COORD_MAX)
        } else {
            rng.range(-ORIGIN_MAX, ORIGIN_MAX)
        };
        check_written(n, u, s);
    }
    assert!(matches!(
        write::shortest(ORIGIN_MAX + 1, LinearUnit::METRE, GridStep::Q0),
        Err(ExchangeError::OutOfRange { .. })
    ));
}

#[test]
fn metric_values_are_written_exactly_and_inches_are_not() {
    let mm = unit::metre(Some(SiPrefix::Milli)).unwrap();
    // One q0 step is 0.00015625 mm (chapter 24.10).
    let w = write::length(1, mm, GridStep::Q0).unwrap();
    assert_eq!(w.decimal(), Decimal::parse("0.00015625").unwrap());
    assert!(w.is_exact());
    let mut rng = Rng::new("g413 exact metric", 0x6413_0008);
    for _ in 0..5_000 {
        let n = rng.range(-ORIGIN_MAX, ORIGIN_MAX);
        for u in [LinearUnit::METRE, mm] {
            let s = rng.pick(&STEPS);
            let w = write::length(n, u, s).unwrap();
            assert!(w.is_exact(), "{n}");
            let per = of_unit(u) * per_metre(s);
            assert_eq!(
                of_decimal(w.decimal()) * per,
                BigRational::from_integer(BigInt::from(n))
            );
        }
    }
    // An inch at q0 terminates only on whole multiples of 127 steps; others are written shortest.
    let inch = NamedLength::Inch.unit().unwrap();
    assert!(write::exact(127, inch, GridStep::Q0).unwrap().is_some());
    assert!(write::exact(1, inch, GridStep::Q0).unwrap().is_none());
    let w = write::length(1, inch, GridStep::Q0).unwrap();
    assert!(!w.is_exact());
    assert_eq!(
        grid::length(w.decimal(), inch, GridStep::Q0)
            .unwrap()
            .count(),
        1
    );
    // A US survey foot never terminates except on multiples of 3,937 feet.
    let us = NamedLength::UsSurveyFoot.unit().unwrap();
    assert!(
        write::exact(7_680_000_000, us, GridStep::Q0)
            .unwrap()
            .is_some()
    );
    assert!(
        write::exact(7_680_000_001, us, GridStep::Q0)
            .unwrap()
            .is_none()
    );
}

// ---------------------------------------------------------------------------------------------
// Map conversions.

fn d(t: &str) -> Decimal {
    Decimal::parse(t).unwrap()
}

fn conversion(e: &str, n: &str, h: &str) -> MapConversion {
    MapConversion {
        eastings: d(e),
        northings: d(n),
        orthogonal_height: d(h),
        x_axis: None,
        scale: None,
        factors: None,
    }
}

const NZTM: CrsId = CrsId::Epsg(2193);

#[test]
fn a_map_conversion_becomes_a_frame_with_its_origin_on_the_grid() {
    let c = conversion("1757000.123", "5921000.1234", "12.5");
    let p = place(&c, NZTM, LinearUnit::METRE, LinearUnit::METRE, GridStep::Q0).unwrap();
    let f = p.frame();
    assert_eq!(
        f.origin(),
        [11_244_800_787_200, 37_894_400_789_760, 80_000_000]
    );
    assert_eq!(f.kind(), FrameKind::Map);
    assert_eq!(f.crs(), NZTM);
    assert!(p.is_origin_exact() && p.is_rotation_exact());
    assert_eq!(p.residual(), None);
    // An origin off the grid rounds once and says so.
    let off = conversion("1757000.12345678", "0", "0");
    let p = place(
        &off,
        NZTM,
        LinearUnit::METRE,
        LinearUnit::METRE,
        GridStep::Q0,
    )
    .unwrap();
    assert!(!p.is_origin_exact());
    let want = expected(
        &of_decimal(d("1757000.12345678")),
        LinearUnit::METRE,
        GridStep::Q0,
    )
    .0;
    assert_eq!(BigInt::from(p.frame().origin()[0]), want);
    // Past 2^60 steps it is refused.
    let far = conversion("1e12", "0", "0");
    assert!(matches!(
        place(
            &far,
            NZTM,
            LinearUnit::METRE,
            LinearUnit::METRE,
            GridStep::Q0
        ),
        Err(ExchangeError::OutOfRange { .. })
    ));
}

#[test]
fn a_direction_is_exact_when_its_integers_fit_and_rounded_within_two_to_the_minus_31_otherwise() {
    let mut c = conversion("0", "0", "0");
    let rot = |c: &MapConversion| {
        let p = place(c, NZTM, LinearUnit::METRE, LinearUnit::METRE, GridStep::Q0).unwrap();
        match p.frame().kind() {
            FrameKind::Ground { rotation, .. } => {
                (rotation.cos(), rotation.sin(), p.is_rotation_exact())
            }
            FrameKind::Map => (1, 0, p.is_rotation_exact()),
        }
    };
    c.x_axis = Some([d("0.6"), d("0.8")]);
    assert_eq!(rot(&c), (3, 4, true));
    c.x_axis = Some([d("-0.0"), d("-2")]);
    assert_eq!(rot(&c), (0, -1, true));
    c.x_axis = Some([d("1"), d("0")]);
    assert_eq!(rot(&c), (1, 0, true));
    c.x_axis = Some([d("0"), d("0")]);
    assert_eq!(
        place(&c, NZTM, LinearUnit::METRE, LinearUnit::METRE, GridStep::Q0),
        Err(ExchangeError::ZeroDirection)
    );
    // cos and sin of 30 degrees to 17 digits: no small integer direction; rounded, and within
    // 2^-31 rad of the given one: |tan difference| = |s/c - b/a| ≤ 2^-31 with c = 2^30.
    let mut rng = Rng::new("g413 directions", 0x6413_0009);
    let mut cases = vec![
        (d("0.86602540378443865"), d("0.5")),
        (d("-0.5"), d("0.86602540378443865")),
    ];
    for _ in 0..2_000 {
        let a = Decimal::new(
            rng.range(-(1 << 52), 1 << 52) as i128,
            rng.range(-20, -10) as i32,
        )
        .unwrap();
        let b = Decimal::new(
            rng.range(-(1 << 52), 1 << 52) as i128,
            rng.range(-20, -10) as i32,
        )
        .unwrap();
        if !(a.is_zero() && b.is_zero()) {
            cases.push((a, b));
        }
    }
    cases.push((d("1"), d("1e-60")));
    for (a, b) in cases {
        c.x_axis = Some([a, b]);
        let (cc, ss, exact) = rot(&c);
        let (ar, br) = (of_decimal(a), of_decimal(b));
        let (cr, sr) = (
            BigRational::from_integer(BigInt::from(cc)),
            BigRational::from_integer(BigInt::from(ss)),
        );
        // Same half-plane, and the cross product's ratio to the dot product (the tangent of the
        // angle between them) within 2^-31.
        let cross = &ar * &sr - &br * &cr;
        let dot = &ar * &cr + &br * &sr;
        assert!(dot.is_positive(), "{a}, {b}");
        let tan = (cross / dot).abs();
        if exact {
            assert!(tan.is_zero(), "{a}, {b}");
        } else {
            assert!(tan <= ratio(1, 1 << 31), "{a}, {b}: {cc}, {ss}");
        }
    }
}

#[test]
fn scale_converts_units_and_factors_scale_coordinates() {
    let mm = unit::metre(Some(SiPrefix::Milli)).unwrap();
    let mut c = conversion("100", "200", "3");
    // A project in millimetres on a map in metres, Scale 0.001 as IFC asks: the frame's scale is
    // one.
    c.scale = Some(d("0.001"));
    let p = place(&c, NZTM, LinearUnit::METRE, mm, GridStep::Q0).unwrap();
    assert_eq!(p.frame().kind(), FrameKind::Map);
    assert_eq!(p.residual(), None);
    assert_eq!(p.frame().linear_unit(), LinearUnit::METRE);
    // Scale omitted with the same mismatch: taken as written, 1.0, so the frame scales by 1000
    // horizontally and the heights' factor of 1000 is left to the importer.
    c.scale = None;
    let p = place(&c, NZTM, LinearUnit::METRE, mm, GridStep::Q0).unwrap();
    match p.frame().kind() {
        FrameKind::Ground { scale, .. } => assert_eq!((scale.num(), scale.den()), (1000, 1)),
        FrameKind::Map => panic!("no scale"),
    }
    let one = Scale::ONE;
    assert_eq!(p.residual(), Some([one, one, Scale::new(1000, 1).unwrap()]));
    // IfcMapConversionScaled with FactorX = FactorY and FactorZ = 1: a Ground frame, scaled.
    c.scale = Some(d("0.001"));
    c.factors = Some([d("0.9996"), d("0.9996"), d("1")]);
    let p = place(&c, NZTM, LinearUnit::METRE, mm, GridStep::Q0).unwrap();
    match p.frame().kind() {
        FrameKind::Ground { scale, rotation } => {
            assert_eq!((scale.num(), scale.den()), (2499, 2500));
            assert_eq!((rotation.cos(), rotation.sin()), (1, 0));
        }
        FrameKind::Map => panic!("no scale"),
    }
    assert_eq!(p.residual(), None);
    // Unequal factors stay with the importer, the frame unscaled; so does a height factor.
    c.factors = Some([d("0.9996"), d("0.9995"), d("1.0001")]);
    let p = place(&c, NZTM, LinearUnit::METRE, mm, GridStep::Q0).unwrap();
    assert_eq!(p.frame().kind(), FrameKind::Map);
    assert_eq!(
        p.residual(),
        Some([
            Scale::new(2499, 2500).unwrap(),
            Scale::new(1999, 2000).unwrap(),
            Scale::new(10001, 10000).unwrap()
        ])
    );
    // A factor that is zero or negative is refused.
    c.factors = Some([d("0"), d("1"), d("1")]);
    assert_eq!(
        place(&c, NZTM, LinearUnit::METRE, mm, GridStep::Q0),
        Err(ExchangeError::ScaleOutOfRange)
    );
    c.factors = None;
    c.scale = Some(d("-0.001"));
    assert_eq!(
        place(&c, NZTM, LinearUnit::METRE, mm, GridStep::Q0),
        Err(ExchangeError::ScaleOutOfRange)
    );
}

#[test]
fn a_survey_feet_map_holds_its_unit_and_rounds_its_origin_once() {
    let us = NamedLength::UsSurveyFoot.unit().unwrap();
    let c = conversion("2000000.5", "500000.25", "100");
    let p = place(&c, CrsId::Epsg(2227), us, us, GridStep::Q0).unwrap();
    assert_eq!(p.frame().linear_unit(), us);
    assert!(!p.is_origin_exact());
    for (i, t) in ["2000000.5", "500000.25", "100"].iter().enumerate() {
        let want = expected(&of_decimal(d(t)), us, GridStep::Q0).0;
        assert_eq!(BigInt::from(p.frame().origin()[i]), want, "{t}");
    }
    assert_eq!(p.residual(), None);
}
