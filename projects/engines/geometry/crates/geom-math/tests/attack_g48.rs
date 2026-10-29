//! Piece G48's attack: tries to break every claim of frame records with linear units, epochs and
//! deformation models, exact decimal text, the exact conversion path both ways, exact
//! out-of-range values, shifts that decide the lattice first, and 64-bit scale factors. Every
//! check is against an exact rational model (`num-bigint`, `num-rational`) that never calls the
//! code under test, on hand-picked edges and on inputs from a printed seed (`SPAREKIT_SEED`
//! replays one).

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::disallowed_methods,
    clippy::too_many_lines,
    clippy::single_match_else,
    clippy::unreadable_literal,
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

mod common;

use std::cmp::Ordering;

use common::{
    exact, int, is_nearest_double, pow2, pow2f, round_half_even, sign_minus_sqrt, step_down,
    step_up,
};
use geom_math::frame::steps_per_metre;
use geom_math::{
    Box3, CrsId, Decimal, DecimalError, DecimalPoint, DecimalYear, FrameMismatch, FrameRecord,
    LinearUnit, MapPoint, MathError, ModelRef, Rotation, Scale,
};
use geom_num::NumError;
use geom_num::grid::{COORD_MAX, GridStep, P3};
use geom_test::golden::{self, Sha256, hex};
use geom_test::seed::{self, Seed};
use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};

const NZTM: CrsId = CrsId::Epsg(2193);

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

    /// A value in lo..=hi, from 128 random bits (the span may exceed i128).
    fn range(&mut self, lo: i128, hi: i128) -> i128 {
        let bits = (u128::from(self.next()) << 64) | u128::from(self.next());
        let span = (hi as u128).wrapping_sub(lo as u128).wrapping_add(1);
        lo.wrapping_add((bits % span) as i128)
    }

    fn coin(&mut self) -> bool {
        self.next() & 1 == 1
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u64) as usize]
    }
}

// ---------------------------------------------------------------------------------------------
// The exact model.

fn pow10(e: i32) -> BigRational {
    let p = BigInt::from(10).pow(e.unsigned_abs());
    if e >= 0 {
        BigRational::from_integer(p)
    } else {
        BigRational::new(BigInt::one(), p)
    }
}

fn dec_value(d: Decimal) -> BigRational {
    int(d.significand()) * pow10(d.exponent())
}

fn unit_value(u: LinearUnit) -> BigRational {
    BigRational::new(BigInt::from(u.num()), BigInt::from(u.den()))
}

/// An exact frame coordinate: a rational, or a/√n with n not a square.
enum Exact {
    Rational(BigRational),
    Root(BigRational, u64),
}

/// The nearest integer, ties to even, and whether it is exact.
fn round_exact(x: &Exact) -> (BigInt, bool) {
    match x {
        Exact::Rational(q) => (round_half_even(q), q.is_integer()),
        Exact::Root(a, n) => {
            if a.is_zero() {
                return (BigInt::zero(), true);
            }
            // ⌊|a|/√n⌋ = ⌊√⌊a²/n⌋⌋, then up when 4a² > (2f + 1)²n (never equal: irrational).
            let a2 = a * a;
            let nn = int(i128::from(*n));
            let f = (&a2 / &nn).floor().to_integer().sqrt();
            let f2 = BigInt::from(2) * &f + 1;
            let up = int(4) * &a2 > BigRational::from_integer(&f2 * &f2) * &nn;
            let m = if up { f + 1 } else { f };
            (if a.is_negative() { -m } else { m }, false)
        }
    }
}

/// The exact frame coordinates [x, y, z] of map values given in the frame's unit.
fn exact_frame(f: &FrameRecord, v: &[BigRational; 3]) -> [Exact; 3] {
    let s = int(i128::from(steps_per_metre(f.step())));
    let u = unit_value(f.linear_unit());
    let [oe, on, oh] = f.origin().map(|o| int(o.into()));
    let w: Vec<BigRational> = v.iter().map(|x| x * &u * &s).collect();
    let (ue, un) = (&w[0] - oe, &w[1] - on);
    let (c, sn) = (
        int(f.rotation().cos().into()),
        int(f.rotation().sin().into()),
    );
    let k = BigRational::new(BigInt::from(f.scale().den()), BigInt::from(f.scale().num()));
    let px = (&c * &ue + &sn * &un) * &k;
    let py = (&c * &un - &sn * &ue) * &k;
    let z = Exact::Rational(&w[2] - oh);
    match f.rotation().exact_norm() {
        Some(r) => {
            let r = int(r.into());
            [Exact::Rational(px / &r), Exact::Rational(py / &r), z]
        }
        None => {
            let n = f.rotation().norm2();
            [Exact::Root(px, n), Exact::Root(py, n), z]
        }
    }
}

/// An exact integer as the value `OutOfRange` reports: itself inside `i128`, saturated outside.
fn reported(v: &BigInt) -> i128 {
    v.to_i128().unwrap_or(if v.is_negative() {
        i128::MIN
    } else {
        i128::MAX
    })
}

/// What a conversion to the frame must give: the rounded point and whether all three were exact,
/// or `OutOfRange` with the first coordinate beyond ±(2^40 − 1), in the order z, x, y.
fn expected(f: &FrameRecord, v: &[BigRational; 3]) -> Result<(P3, bool), NumError> {
    let [x, y, z] = exact_frame(f, v);
    let mut out = [0i64; 3];
    let mut all = true;
    for (slot, e) in [(2, &z), (0, &x), (1, &y)] {
        let (r, ex) = round_exact(e);
        if r.abs() > BigInt::from(COORD_MAX) {
            return Err(NumError::OutOfRange {
                value: reported(&r),
            });
        }
        out[slot] = r.to_i64().unwrap();
        all &= ex;
    }
    Ok((P3::new(out[0], out[1], out[2]).unwrap(), all))
}

fn got(r: Result<geom_math::Rounded<P3>, NumError>) -> Result<(P3, bool), NumError> {
    r.map(|p| (p.value(), p.is_exact()))
}

fn values_f64(m: MapPoint) -> [BigRational; 3] {
    [exact(m.easting), exact(m.northing), exact(m.height)]
}

fn values_dec(m: DecimalPoint) -> [BigRational; 3] {
    [
        dec_value(m.easting),
        dec_value(m.northing),
        dec_value(m.height),
    ]
}

/// The sign of V − w for an irrational horizontal map coordinate V = (O + `k_n·a/(k_d·√n))·q/(S·p)`.
fn irrational_minus(
    f: &FrameRecord,
    o: &BigRational,
    a: &BigRational,
    w: &BigRational,
) -> Ordering {
    let s = int(i128::from(steps_per_metre(f.step())));
    let (p, q) = (
        int(f.linear_unit().num().into()),
        int(f.linear_unit().den().into()),
    );
    let (kn, kd) = (int(f.scale().num().into()), int(f.scale().den().into()));
    let big_a = &q * &kn * a;
    let big_b = (w * &s * &p - o * &q) * &kd;
    sign_minus_sqrt(&big_a, &big_b, f.rotation().norm2())
}

/// Checks `to_map` against the exact model: the nearest doubles, ties to even, and whether exact.
fn check_to_map(f: &FrameRecord, p: P3) {
    let m = f.to_map(p).unwrap_or_else(|e| panic!("{f:?} {p:?}: {e:?}"));
    let s = int(i128::from(steps_per_metre(f.step())));
    let u = unit_value(f.linear_unit());
    let [oe, on, oh] = f.origin().map(|o| int(o.into()));
    let (x, y, z) = (
        int(p.x.get().into()),
        int(p.y.get().into()),
        int(p.z.get().into()),
    );
    let (c, sn) = (
        int(f.rotation().cos().into()),
        int(f.rotation().sin().into()),
    );
    let (ae, an) = (&c * &x - &sn * &y, &sn * &x + &c * &y);
    let height = (oh + z) / (&s * &u);
    let got = m.value();
    assert!(is_nearest_double(got.height, &height), "{f:?} {p:?} height");
    let mut exact_all = height == exact(got.height);
    match f.rotation().exact_norm() {
        Some(r) => {
            let k = BigRational::new(BigInt::from(f.scale().num()), BigInt::from(f.scale().den()));
            let r = int(r.into());
            for (d, o, a) in [(got.easting, &oe, &ae), (got.northing, &on, &an)] {
                let v = (o + &k * a / &r) / (&s * &u);
                assert!(is_nearest_double(d, &v), "{f:?} {p:?}: {d} against {v}");
                exact_all &= v == exact(d);
            }
        }
        None => {
            for (d, o, a) in [(got.easting, &oe, &ae), (got.northing, &on, &an)] {
                if a.is_zero() {
                    let v = o / (&s * &u);
                    assert!(is_nearest_double(d, &v), "{f:?} {p:?}: {d}");
                    exact_all &= v == exact(d);
                    continue;
                }
                let two = int(2);
                let below = (exact(step_down(d)) + exact(d)) / &two;
                let above = (exact(step_up(d)) + exact(d)) / &two;
                assert_eq!(
                    irrational_minus(f, o, a, &below),
                    Ordering::Greater,
                    "{f:?} {p:?}"
                );
                assert_eq!(
                    irrational_minus(f, o, a, &above),
                    Ordering::Less,
                    "{f:?} {p:?}"
                );
                exact_all = false;
            }
        }
    }
    assert_eq!(m.is_exact(), exact_all, "{f:?} {p:?}");
}

// ---------------------------------------------------------------------------------------------
// Frames and points to attack with.

/// Clarke's foot, 0.3047972654 m (EPSG:9005).
fn clarkes_foot() -> LinearUnit {
    LinearUnit::new(1_523_986_327, 5_000_000_000).unwrap()
}

const DIRECTIONS: [(i32, i32); 9] = [
    (1, 0),
    (0, 1),
    (-1, 0),
    (0, -1),
    (3, 4),
    (-20, 21),
    (1, 2),
    (2_147_483_647, -2_147_483_646),
    (-7, -3),
];

const SCALES: [(u64, u64); 7] = [
    (1, 1),
    (9_996, 10_000),
    (9_999_876_543, 10_000_000_000),
    (18_446_744_073_709_551_557, 18_446_744_073_709_551_615),
    (1, 3),
    (7, 1),
    (1_000_000_007, 999_999_999_989),
];

fn unit(rng: &mut Rng) -> LinearUnit {
    match rng.below(6) {
        0 => LinearUnit::METRE,
        1 => LinearUnit::INTERNATIONAL_FOOT,
        2 => LinearUnit::US_SURVEY_FOOT,
        3 => clarkes_foot(),
        _ => loop {
            let (n, d) = (1 + rng.below(1 << 40), 1 + rng.below(1 << 40));
            if let Ok(u) = LinearUnit::new(n, d) {
                break u;
            }
        },
    }
}

fn frame(rng: &mut Rng) -> FrameRecord {
    let step = rng.pick(&[GridStep::Q0, GridStep::Q0, GridStep::Q1, GridStep::Q2]);
    let s = i128::from(steps_per_metre(step));
    let site = [1_757_000, 5_921_000, 30].map(|m| (m * s) as i64);
    let origin = if rng.coin() {
        site.map(|o| o + rng.range(-1000, 1000) as i64)
    } else {
        [0, 1, 2].map(|_| rng.range(-(1 << 60), 1 << 60) as i64)
    };
    let (c, sn) = rng.pick(&DIRECTIONS);
    let (kn, kd) = rng.pick(&SCALES);
    FrameRecord::new(
        NZTM,
        step,
        origin,
        Rotation::direction(c, sn).unwrap(),
        Scale::new(kn, kd).unwrap(),
    )
    .unwrap()
    .with_linear_unit(unit(rng))
}

/// A map value near the frame's origin (so usually in range), on a whole step, on a midpoint, or
/// anywhere, in the frame's unit; `spread` steps around the origin.
fn near_origin(rng: &mut Rng, f: &FrameRecord, axis: usize, spread: i128) -> BigRational {
    let s = int(i128::from(steps_per_metre(f.step())));
    let u = unit_value(f.linear_unit());
    let o = int(f.origin()[axis].into());
    let steps = int(rng.range(-spread, spread));
    let half = BigRational::new(BigInt::one(), BigInt::from(2));
    let fraction = match rng.below(3) {
        0 => BigRational::zero(),
        1 => half,
        _ => BigRational::new(BigInt::from(rng.below(1 << 40)), BigInt::from(1u64 << 40)),
    };
    (o + steps + fraction) / (&s * &u)
}

/// The double nearest a rational, by bisection with exact comparisons (an oracle).
fn to_f64(x: &BigRational) -> f64 {
    let mut d = x.to_f64().unwrap();
    for _ in 0..4 {
        if is_nearest_double(d, x) {
            return d;
        }
        d = if exact(d) < *x {
            step_up(d)
        } else {
            step_down(d)
        };
    }
    d
}

/// A decimal near a rational: its first `digits` significant digits, cut (an approximation, then
/// read exactly).
fn decimal_near(x: &BigRational, digits: u32) -> Decimal {
    if x.is_zero() {
        return Decimal::ZERO;
    }
    let mag = x.abs();
    let mut e = 0i32;
    let ten = int(10);
    let mut scaled = mag.clone();
    while scaled >= ten {
        scaled /= &ten;
        e += 1;
    }
    while scaled < BigRational::one() {
        scaled *= &ten;
        e -= 1;
    }
    let shift = digits as i32 - 1;
    let m = (mag * pow10(shift - e)).floor().to_integer();
    let m = m.to_i128().unwrap();
    Decimal::new(if x.is_negative() { -m } else { m }, e - shift).unwrap()
}

// ---------------------------------------------------------------------------------------------
// Decimal text.

/// The oracle's reading of decimal text: the value, its significant digits and written exponent,
/// or `None` for text outside the grammar.
fn oracle_parse(text: &str) -> Option<(BigRational, usize, Option<i64>)> {
    let (neg, body) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    let (mantissa, exponent) = match body.find(['e', 'E']) {
        Some(i) => (&body[..i], Some(&body[i + 1..])),
        None => (body, None),
    };
    let (whole, frac) = match mantissa.find('.') {
        Some(i) => (&mantissa[..i], &mantissa[i + 1..]),
        None => (mantissa, ""),
    };
    let digit = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if !digit(whole) || !digit(frac) || whole.len() + frac.len() == 0 {
        return None;
    }
    let written = match exponent {
        None => Some(0i64),
        Some(t) => {
            let u = t.strip_prefix(['+', '-']).unwrap_or(t);
            if u.is_empty() || !digit(u) {
                return None;
            }
            t.parse::<i32>().ok().map(i64::from)
        }
    };
    let all = format!("{whole}{frac}");
    let n = BigInt::parse_bytes(all.as_bytes(), 10).unwrap();
    let significant = all.trim_start_matches('0').trim_end_matches('0').len();
    let value = BigRational::from_integer(if neg { -n } else { n })
        * pow10(-(frac.len() as i32))
        * pow10(written.unwrap_or(0) as i32);
    Some((value, significant, written))
}

/// What `Decimal::parse` must return for text the grammar accepts.
fn oracle_decimal(text: &str) -> Result<BigRational, DecimalError> {
    let (value, significant, written) = oracle_parse(text).expect("text inside the grammar");
    if significant > 38 {
        return Err(DecimalError::TooManyDigits);
    }
    if written.is_none() {
        return Err(DecimalError::ExponentRange);
    }
    if value.is_zero() {
        return Ok(value);
    }
    // The exponent in lowest terms: value = m·10^e with m not divisible by 10.
    let mut e = 0i64;
    let mut v = value.clone();
    while v.is_integer() && (v.to_integer() % BigInt::from(10)).is_zero() {
        v /= int(10);
        e += 1;
    }
    while !v.is_integer() {
        v *= int(10);
        e -= 1;
    }
    if !(-340..=340).contains(&e) {
        return Err(DecimalError::ExponentRange);
    }
    Ok(value)
}

#[test]
fn decimal_text_is_read_exactly_or_refused() {
    let read = |t: &str| Decimal::parse(t).map(|d| (d.significand(), d.exponent()));
    for (text, want) in [
        ("0", (0, 0)),
        ("-0", (0, 0)),
        ("+0.000", (0, 0)),
        ("0e99", (0, 0)),
        ("12", (12, 0)),
        ("+12.", (12, 0)),
        (".5", (5, -1)),
        ("-0.000125", (-125, -6)),
        ("6.02E23", (602, 21)),
        ("1200", (12, 2)),
        ("1.2300", (123, -2)),
        ("0001.0010e-3", (1001, -6)),
        ("5921000.1234", (59_210_001_234, -4)),
        ("1e340", (1, 340)),
        ("1e-340", (1, -340)),
        ("10e339", (1, 340)),
        (
            "12345678901234567890123456789012345678",
            (12_345_678_901_234_567_890_123_456_789_012_345_678, 0),
        ),
        (
            "1234567890123456789012345678901234567800000",
            (12_345_678_901_234_567_890_123_456_789_012_345_678, 5),
        ),
        ("0.0000000000000000000000000000000000000000012", (12, -43)),
        ("1e+5", (1, 5)),
        ("1E-0", (1, 0)),
    ] {
        assert_eq!(read(text), Ok(want), "{text}");
    }
    for (text, want) in [
        ("", DecimalError::Syntax { at: 0 }),
        ("-", DecimalError::Syntax { at: 1 }),
        (".", DecimalError::Syntax { at: 1 }),
        ("e5", DecimalError::Syntax { at: 0 }),
        ("1e", DecimalError::Syntax { at: 2 }),
        ("1e+", DecimalError::Syntax { at: 3 }),
        ("1e5x", DecimalError::Syntax { at: 3 }),
        ("1e+-5", DecimalError::Syntax { at: 3 }),
        ("1..2", DecimalError::Syntax { at: 2 }),
        ("1.2.3", DecimalError::Syntax { at: 3 }),
        (" 1", DecimalError::Syntax { at: 0 }),
        ("1 ", DecimalError::Syntax { at: 1 }),
        ("1,5", DecimalError::Syntax { at: 1 }),
        ("1_000", DecimalError::Syntax { at: 1 }),
        ("--1", DecimalError::Syntax { at: 1 }),
        ("inf", DecimalError::Syntax { at: 0 }),
        ("NaN", DecimalError::Syntax { at: 0 }),
        ("0x10", DecimalError::Syntax { at: 1 }),
        ("١", DecimalError::Syntax { at: 0 }),
        (
            "123456789012345678901234567890123456789",
            DecimalError::TooManyDigits,
        ),
        (
            "1.00000000000000000000000000000000000001",
            DecimalError::TooManyDigits,
        ),
        ("1e341", DecimalError::ExponentRange),
        ("1e-341", DecimalError::ExponentRange),
        ("100e339", DecimalError::ExponentRange),
        ("1e2147483648", DecimalError::ExponentRange),
        ("0e2147483648", DecimalError::ExponentRange),
    ] {
        assert_eq!(read(text), Err(want), "{text:?}");
    }
    // Random text inside the grammar, against the oracle.
    let mut rng = Rng::new("decimal text", 0x6448_5445_5854_0001);
    let digits = |rng: &mut Rng, n: u64| -> String {
        (0..rng.below(n + 1))
            .map(|_| {
                char::from(
                    b'0' + if rng.below(4) == 0 {
                        0
                    } else {
                        rng.below(10) as u8
                    },
                )
            })
            .collect()
    };
    for _ in 0..10_000 {
        let sign = rng.pick(&["", "-", "+"]);
        let whole = digits(&mut rng, 24);
        let frac = digits(&mut rng, 24);
        let mantissa = if rng.coin() || frac.is_empty() {
            if whole.is_empty() && frac.is_empty() {
                "7".to_string()
            } else if frac.is_empty() {
                whole
            } else {
                format!("{whole}.{frac}")
            }
        } else {
            format!("{whole}.{frac}")
        };
        let mantissa = if mantissa == "." {
            "0.".to_string()
        } else {
            mantissa
        };
        let exponent = match rng.below(4) {
            0 => String::new(),
            1 => format!("e{}", rng.range(-400, 400)),
            2 => format!("E+{}", rng.range(0, 30)),
            _ => format!("e{}", rng.pick(&["-0", "00012", "-00360", "99999999999"])),
        };
        let text = format!("{sign}{mantissa}{exponent}");
        let got = Decimal::parse(&text).map(dec_value);
        assert_eq!(got, oracle_decimal(&text), "{text}");
    }
}

#[test]
fn decimals_have_one_form_and_read_back() {
    let d = |t: &str| Decimal::parse(t).unwrap();
    assert_eq!(d("2023.50"), d("2023.5"));
    assert_eq!(d("-0"), Decimal::ZERO);
    assert_eq!(d("1500"), Decimal::new(15, 2).unwrap());
    assert_eq!(Decimal::new(1500, -2), Ok(d("15")));
    assert_eq!(Decimal::new(0, 77), Ok(Decimal::ZERO));
    assert!(Decimal::ZERO.is_zero() && !d("1e-340").is_zero());
    let ten38: i128 = 100_000_000_000_000_000_000_000_000_000_000_000_000;
    assert_eq!(Decimal::new(ten38, 0), Ok(Decimal::new(1, 38).unwrap()));
    assert_eq!(Decimal::new(ten38 + 1, 0), Err(DecimalError::TooManyDigits));
    assert_eq!(
        Decimal::new(-(ten38 - 1), 0).map(Decimal::significand),
        Ok(-(ten38 - 1))
    );
    assert_eq!(Decimal::new(i128::MIN, 0), Err(DecimalError::TooManyDigits));
    assert_eq!(Decimal::new(1, 341), Err(DecimalError::ExponentRange));
    assert_eq!(Decimal::new(10, 340), Err(DecimalError::ExponentRange));
    assert_eq!(Decimal::new(10, i32::MAX), Err(DecimalError::ExponentRange));
    assert_eq!(Decimal::new(1, -341), Err(DecimalError::ExponentRange));
    assert_eq!(Decimal::new(1000, -342), Ok(Decimal::new(1, -339).unwrap()));
    assert_eq!("12.5".parse::<Decimal>(), Ok(d("12.5")));
    // Text forms: plain within 30 zeros of the digits, an exponent beyond; each reads back.
    for (v, text) in [
        (d("1500"), "1500"),
        (d("-12.5"), "-12.5"),
        (d("0.00125"), "0.00125"),
        (d("15e40"), "15e40"),
        (d("-125e-45"), "-125e-45"),
        (d("1e30"), "1000000000000000000000000000000"),
        (d("1e31"), "1e31"),
        (d("1e-30"), "0.000000000000000000000000000001"),
        (d("1e-32"), "1e-32"),
        (d("12345e-34"), "0.0000000000000000000000000000012345"),
        (d("12345e-36"), "12345e-36"),
        (Decimal::ZERO, "0"),
    ] {
        assert_eq!(v.to_string(), text);
        assert_eq!(Decimal::parse(text), Ok(v), "{text}");
    }
    let mut rng = Rng::new("decimal forms", 0x6448_464f_524d_0002);
    for _ in 0..10_000 {
        let m = match rng.below(3) {
            0 => rng.range(-1000, 1000),
            1 => rng.range(-(ten38 - 1), ten38 - 1),
            _ => rng.range(-(1 << 60), 1 << 60),
        };
        let e = rng.range(-345, 345) as i32;
        let Ok(v) = Decimal::new(m, e) else {
            continue;
        };
        assert_eq!(dec_value(v), int(m) * pow10(e));
        assert_eq!(Decimal::parse(&v.to_string()), Ok(v), "{m}e{e}");
        // The same value written with extra zeros is the same decimal.
        if let Some(m) = v.significand().checked_mul(1000)
            && v.exponent() > -330
        {
            assert_eq!(Decimal::new(m, v.exponent() - 3), Ok(v));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Units, epochs and models.

#[test]
fn linear_units_are_exact_reduced_ratios_in_range() {
    let r = |n: u64, d: u64| LinearUnit::new(n, d);
    let d = |t: &str| LinearUnit::from_decimal(Decimal::parse(t).unwrap());
    assert!(LinearUnit::METRE.is_metre());
    assert!(!LinearUnit::US_SURVEY_FOOT.is_metre() && !r(2, 1).unwrap().is_metre());
    assert!(!r(1, 2).unwrap().is_metre());
    assert_eq!(r(7, 7), Ok(LinearUnit::METRE));
    assert_eq!(r(3048, 10_000), Ok(LinearUnit::INTERNATIONAL_FOOT));
    assert_eq!(r(12_000, 39_370), Ok(LinearUnit::US_SURVEY_FOOT));
    assert_eq!(d("0.3048"), Ok(LinearUnit::INTERNATIONAL_FOOT));
    assert_eq!(d("0.3047972654"), Ok(clarkes_foot()));
    assert_eq!(d("1000"), r(1000, 1));
    assert_eq!(d("1e-9"), r(1, 1_000_000_000));
    let u = LinearUnit::US_SURVEY_FOOT;
    assert_eq!((u.num(), u.den()), (1200, 3937));
    // The range, 2^−32 to 2^32 metres, both ends included.
    let top = 1u64 << 32;
    assert_eq!(r(top, 1).map(LinearUnit::num), Ok(top));
    assert_eq!(r(1, top).map(LinearUnit::den), Ok(top));
    assert_eq!(r(3 * top, 3), r(top, 1));
    assert_eq!(r(top + 1, 1), Err(NumError::OutsideDomain));
    assert_eq!(r(1, top + 1), Err(NumError::OutsideDomain));
    assert_eq!(r(u64::MAX, 1), Err(NumError::OutsideDomain));
    assert_eq!(r(1, u64::MAX), Err(NumError::OutsideDomain));
    assert_eq!(r(u64::MAX, u64::MAX - 1).map(LinearUnit::num), Ok(u64::MAX));
    assert_eq!(r(0, 1), Err(NumError::Degenerate));
    assert_eq!(r(1, 0), Err(NumError::Degenerate));
    assert_eq!(d("4294967296"), r(top, 1));
    assert_eq!(d("4294967297"), Err(NumError::OutsideDomain));
    assert_eq!(d("1e10"), Err(NumError::OutsideDomain));
    assert_eq!(d("1e40"), Err(NumError::OutsideDomain));
    assert_eq!(d("1e-10"), Err(NumError::OutsideDomain));
    assert_eq!(d("0"), Err(NumError::Degenerate));
    assert_eq!(d("-0.3048"), Err(NumError::OutsideDomain));
    // Twenty decimals with no factor of 2 or 5 to cancel need a denominator of 10^20, beyond 64
    // bits.
    assert_eq!(d("0.33333333333333333333"), Err(NumError::OutsideDomain));
    // Exponents below −38: the shared factors of 2 and 5 cancel before the denominator is formed.
    let five40 = BigInt::from(5).pow(40);
    let m = (five40 * BigInt::from(301)).to_i128().unwrap();
    let tiny = Decimal::new(m, -40).unwrap();
    assert_eq!(LinearUnit::from_decimal(tiny), r(301, 1 << 40));
    let two100 = 1i128 << 100;
    assert_eq!(
        LinearUnit::from_decimal(Decimal::new(two100 * 3, -45).unwrap()),
        Err(NumError::OutsideDomain)
    );
    // Random ratios against the model: reduced, and refused exactly outside the range.
    let mut rng = Rng::new("linear units", 0x554e_4954_0000_0003);
    for _ in 0..20_000 {
        let n = rng.next() >> rng.below(64);
        let dd = rng.next() >> rng.below(64);
        let want = BigRational::new(BigInt::from(n), BigInt::from(dd.max(1)));
        match r(n, dd) {
            Ok(u) => {
                assert_eq!(unit_value(u), want);
                assert!(want >= pow2(-32) && want <= pow2(32));
                assert_eq!(
                    BigInt::from(u.num()).gcd(&BigInt::from(u.den())),
                    BigInt::one()
                );
            }
            Err(NumError::Degenerate) => assert!(n == 0 || dd == 0),
            Err(NumError::OutsideDomain) => assert!(want < pow2(-32) || want > pow2(32)),
            Err(e) => panic!("{e:?}"),
        }
    }
}

#[test]
fn epochs_are_decimal_years_and_models_their_digests() {
    let y = |t: &str| DecimalYear::new(Decimal::parse(t).unwrap());
    assert_eq!(y("2023.50"), y("2023.5"));
    assert_ne!(y("2023.5").unwrap(), y("2023.51").unwrap());
    assert_eq!(
        y("2000").map(DecimalYear::year),
        Ok(Decimal::parse("2000.0").unwrap())
    );
    for ok in ["0", "0.0001", "9999.99999999", "9999", "1e3", "2.0005e3"] {
        assert!(y(ok).is_ok(), "{ok}");
    }
    for bad in [
        "10000",
        "1e4",
        "10000.000001",
        "-0.5",
        "-2000",
        "1e300",
        "99999",
    ] {
        assert_eq!(y(bad), Err(NumError::OutsideDomain), "{bad}");
    }
    assert!(DecimalYear::new(Decimal::new(99_999_999_999_999_999_999, -18).unwrap()).is_ok());
    assert!(DecimalYear::new(Decimal::new(1, -340).unwrap()).is_ok());
    let digest = [7u8; 32];
    let model = ModelRef::from_sha256(digest);
    assert_eq!(model.sha256(), digest);
    assert_ne!(model, ModelRef::from_sha256([8; 32]));
}

#[test]
fn records_carry_units_epochs_and_models_and_never_mix_them() {
    let base = FrameRecord::map(NZTM, GridStep::Q0, [0, 0, 0]).unwrap();
    assert_eq!(base.linear_unit(), LinearUnit::METRE);
    assert_eq!((base.epoch(), base.deformation_model()), (None, None));
    let epoch = DecimalYear::new(Decimal::parse("2000.0").unwrap()).unwrap();
    let later = DecimalYear::new(Decimal::parse("2023.5").unwrap()).unwrap();
    let model = ModelRef::from_sha256([1; 32]);
    let other_model = ModelRef::from_sha256([2; 32]);
    let full = base
        .with_linear_unit(LinearUnit::US_SURVEY_FOOT)
        .with_epoch(Some(epoch))
        .with_deformation_model(Some(model));
    assert_eq!(full.linear_unit(), LinearUnit::US_SURVEY_FOOT);
    assert_eq!(full.epoch(), Some(epoch));
    assert_eq!(full.deformation_model(), Some(model));
    assert!(full.shift_to(&full).is_ok());
    let mismatch = |a: &FrameRecord, b: &FrameRecord| match a.shift_to(b) {
        Err(MathError::FrameMismatch(m)) => Some(m),
        _ => None,
    };
    // Each difference alone, both ways round, and in the documented order when several differ.
    let cases = [
        (
            full.with_linear_unit(LinearUnit::METRE),
            FrameMismatch::LinearUnit,
        ),
        (full.with_epoch(Some(later)), FrameMismatch::Epoch),
        (full.with_epoch(None), FrameMismatch::Epoch),
        (
            full.with_deformation_model(Some(other_model)),
            FrameMismatch::DeformationModel,
        ),
        (
            full.with_deformation_model(None),
            FrameMismatch::DeformationModel,
        ),
        (
            full.with_linear_unit(LinearUnit::METRE).with_epoch(None),
            FrameMismatch::LinearUnit,
        ),
        (
            full.with_epoch(None).with_deformation_model(None),
            FrameMismatch::Epoch,
        ),
        (
            full.with_vertical_crs(Some(CrsId::Epsg(7839)))
                .with_linear_unit(LinearUnit::METRE),
            FrameMismatch::VerticalCrs,
        ),
    ];
    for (other, want) in cases {
        assert_eq!(mismatch(&full, &other), Some(want), "{other:?}");
        assert_eq!(mismatch(&other, &full), Some(want), "{other:?}");
    }
    let rotated = FrameRecord::new(
        NZTM,
        GridStep::Q0,
        [0; 3],
        Rotation::direction(3, 4).unwrap(),
        Scale::ONE,
    )
    .unwrap()
    .with_deformation_model(Some(model));
    assert_eq!(
        mismatch(
            &full.with_linear_unit(LinearUnit::METRE).with_epoch(None),
            &rotated
        ),
        Some(FrameMismatch::Rotation)
    );
    assert_eq!(
        mismatch(&rotated, &rotated.with_deformation_model(None)),
        Some(FrameMismatch::DeformationModel)
    );
    // Lattice homes: every operand must agree on all of them, and the home keeps them.
    let cell = |i: i64| {
        FrameRecord::lattice(NZTM, GridStep::Q0, [0; 3], [i, 0])
            .unwrap()
            .with_linear_unit(LinearUnit::US_SURVEY_FOOT)
            .with_epoch(Some(epoch))
            .with_deformation_model(Some(model))
    };
    let b = Box3::from_point(P3::new(10, 10, 0).unwrap());
    let home = FrameRecord::home_of(&[(cell(0), b), (cell(1), b)]).unwrap();
    assert_eq!(home.linear_unit(), LinearUnit::US_SURVEY_FOOT);
    assert_eq!(home.epoch(), Some(epoch));
    assert_eq!(home.deformation_model(), Some(model));
    for (other, want) in [
        (
            cell(1).with_linear_unit(LinearUnit::METRE),
            FrameMismatch::LinearUnit,
        ),
        (cell(1).with_epoch(Some(later)), FrameMismatch::Epoch),
        (
            cell(1).with_deformation_model(None),
            FrameMismatch::DeformationModel,
        ),
        (
            cell(1).with_vertical_crs(Some(CrsId::Epsg(7839))),
            FrameMismatch::VerticalCrs,
        ),
    ] {
        assert_eq!(
            FrameRecord::home_of(&[(cell(0), b), (other, b)]),
            Err(MathError::FrameMismatch(want))
        );
    }
    let (h, shift) = cell(0).home_frame(&b).unwrap();
    assert_eq!(
        (h.linear_unit(), h.epoch()),
        (LinearUnit::US_SURVEY_FOOT, Some(epoch))
    );
    assert_eq!(shift.apply(b.min()), Ok(b.min()));
}

// ---------------------------------------------------------------------------------------------
// Map to frame.

#[test]
fn map_values_in_any_unit_round_once_to_the_exact_model() {
    let mut rng = Rng::new("map to frame in units", 0x4d41_5054_4f46_0004);
    let mut seen = [0usize; 4];
    for _ in 0..3000 {
        let f = frame(&mut rng);
        let spread = rng.pick(&[3, 1 << 20, 1 << 39, (1 << 40) + 8]);
        let v = [0, 1, 2].map(|a| near_origin(&mut rng, &f, a, spread));
        // As doubles: the value's nearest double is the input, read exactly by the model.
        let m = MapPoint {
            easting: to_f64(&v[0]),
            northing: to_f64(&v[1]),
            height: to_f64(&v[2]),
        };
        let want = expected(&f, &values_f64(m));
        assert_eq!(got(f.to_frame(m)), want, "{f:?} {m:?}");
        // As decimals: 25 significant digits of the value, read exactly.
        let dp = DecimalPoint {
            easting: decimal_near(&v[0], 25),
            northing: decimal_near(&v[1], 25),
            height: decimal_near(&v[2], 25),
        };
        let want_d = expected(&f, &values_dec(dp));
        assert_eq!(got(f.to_frame_decimal(dp)), want_d, "{f:?} {dp:?}");
        match want_d {
            Ok((_, true)) => seen[0] += 1,
            Ok((_, false)) => seen[1] += 1,
            Err(NumError::OutOfRange { .. }) => seen[2] += 1,
            Err(_) => seen[3] += 1,
        }
    }
    // Exact cases are the next test's: here the inputs are cut, so mostly inexact.
    assert!(seen[1] > 250 && seen[2] > 150, "{seen:?}");
}

#[test]
fn decimal_text_on_whole_steps_and_midpoints_converts_exactly_and_ties_to_even() {
    // At q0 in metres one step is 1/6,400,000 m = 0.00000015625 m: every whole step and every
    // midpoint is a short decimal, so these are the exact cases, each one exactly.
    let site = [1_757_000_i64 * 6_400_000, 5_921_000 * 6_400_000, 0];
    let frames = [
        FrameRecord::map(NZTM, GridStep::Q0, site).unwrap(),
        FrameRecord::new(
            NZTM,
            GridStep::Q0,
            site,
            Rotation::quarter_turns(3),
            Scale::ONE,
        )
        .unwrap(),
        FrameRecord::new(
            NZTM,
            GridStep::Q0,
            site,
            Rotation::direction(3, 4).unwrap(),
            Scale::ONE,
        )
        .unwrap(),
    ];
    let half_step = Decimal::new(78_125, -12).unwrap();
    for f in frames {
        for k in [-3i128, -2, -1, 0, 1, 2, 3, 1 << 30, (1 << 40) - 2] {
            for half in [false, true] {
                // The map value (O + 2k + half)/S, written exactly: (O + 2k + h)·78125·10^−12.
                let o = |axis: usize| i128::from(site[axis]);
                let text = |axis: usize| {
                    let n = 2 * (o(axis) + k) + i128::from(half);
                    Decimal::new(n * 78_125, -12).unwrap()
                };
                let p = DecimalPoint {
                    easting: text(0),
                    northing: text(1),
                    height: text(2),
                };
                let want = expected(&f, &values_dec(p));
                assert_eq!(got(f.to_frame_decimal(p)), want, "{f:?} k {k} half {half}");
                if f.rotation() == Rotation::IDENTITY {
                    let (q, exact) = want.unwrap();
                    assert_eq!(exact, !half);
                    // A midpoint goes to the even neighbour.
                    let even = |c: i64| {
                        if half {
                            (k + i128::from(k % 2 != 0)) as i64
                        } else {
                            c
                        }
                    };
                    assert_eq!(q.x.get(), even(k as i64));
                }
            }
        }
        assert_eq!(dec_value(half_step) * int(6_400_000) * int(2), int(1));
    }
}

#[test]
fn doubles_and_their_exact_decimals_convert_alike() {
    // A double that is a short decimal converts the same through either door: the fast path and
    // the exact path agree, and so do the two input forms.
    let mut rng = Rng::new("doubles as decimals", 0x4444_4f4f_5253_0005);
    for _ in 0..2000 {
        let mut f = frame(&mut rng);
        if rng.coin() {
            f = f.with_linear_unit(LinearUnit::METRE);
        }
        let mut pair = || {
            let k = rng.below(21) as u32;
            let m = rng.range(-(1 << 52), 1 << 52) >> rng.below(20);
            let x = m as f64 / f64::from(1u32 << k.min(20));
            let d = Decimal::new(m * 5i128.pow(k), -(k as i32)).unwrap();
            assert_eq!(dec_value(d), exact(x));
            (x, d)
        };
        let (e, de) = pair();
        let (n, dn) = pair();
        let (h, dh) = pair();
        let m = MapPoint {
            easting: e,
            northing: n,
            height: h,
        };
        let d = DecimalPoint {
            easting: de,
            northing: dn,
            height: dh,
        };
        assert_eq!(
            got(f.to_frame(m)),
            got(f.to_frame_decimal(d)),
            "{f:?} {m:?}"
        );
    }
}

#[test]
fn out_of_range_coordinates_carry_their_exact_value() {
    let mut rng = Rng::new("out of range", 0x4f55_5452_4e47_0006);
    let mut far = 0;
    for _ in 0..1500 {
        let f = frame(&mut rng);
        let s = int(i128::from(steps_per_metre(f.step())));
        let u = unit_value(f.linear_unit());
        // Map values up to 2^(40 + j) steps from the origin, j up to 100: beyond the frame, often
        // beyond i128, never beyond 2^200 steps.
        let j = rng.below(100) as i32;
        let v = [0, 1, 2].map(|axis| {
            let o = int(f.origin()[axis].into());
            let off = int(rng.range(-(1 << 62), 1 << 62)) * pow2(j - 22);
            (o + off) / (&s * &u)
        });
        let m = MapPoint {
            easting: to_f64(&v[0]),
            northing: to_f64(&v[1]),
            height: to_f64(&v[2]),
        };
        let want = expected(&f, &values_f64(m));
        assert_eq!(got(f.to_frame(m)), want, "{f:?} {m:?}");
        if matches!(want, Err(NumError::OutOfRange { value }) if value == i128::MAX || value == i128::MIN)
        {
            far += 1;
        }
        let dp = DecimalPoint {
            easting: decimal_near(&v[0], 30),
            northing: decimal_near(&v[1], 30),
            height: decimal_near(&v[2], 30),
        };
        assert_eq!(
            got(f.to_frame_decimal(dp)),
            expected(&f, &values_dec(dp)),
            "{f:?} {dp:?}"
        );
    }
    assert!(far > 50, "only {far} saturated values");
}

#[test]
fn values_at_the_ends_of_i128_saturate_exactly_as_documented() {
    // One unit is one step (1/6,400,000 m at q0), so x = k_d·E exactly in a Ground frame with no
    // rotation and k = 1/k_d. 2^128 − 3 = 76,439·4,451,685,225,093,714,772,084,598,273,548,427 puts
    // x on MAX − 1/2 for k_d = 76,439, whose even neighbour MAX − 1 is reported as it is; and
    // 2^128 − 1 = (2^64 − 1)(2^64 + 1) puts x on MIN + 1/2 for k_d = 2^64 − 1, which goes to the
    // even MIN.
    let one_step = LinearUnit::new(1, 6_400_000).unwrap();
    let ground = |kd: u64| {
        FrameRecord::new(
            NZTM,
            GridStep::Q0,
            [0; 3],
            Rotation::IDENTITY,
            Scale::new(1, kd).unwrap(),
        )
        .unwrap()
        .with_linear_unit(one_step)
    };
    let at = |f: &FrameRecord, e: &str| {
        let p = DecimalPoint {
            easting: Decimal::parse(e).unwrap(),
            northing: Decimal::ZERO,
            height: Decimal::ZERO,
        };
        (got(f.to_frame_decimal(p)), expected(f, &values_dec(p)))
    };
    let out = |value| Err(NumError::OutOfRange { value });
    let top = ground(76_439);
    for (e, want) in [
        ("2225842612546857386042299136774213.5", out(i128::MAX - 1)),
        ("2225842612546857386042299136774213.6", out(i128::MAX)),
        (
            "2225842612546857386042299136774213.4",
            out(i128::MAX - 7_644),
        ),
        ("2225842612546857386042299136774214", out(i128::MAX)),
        ("1e37", out(i128::MAX)),
    ] {
        let (g, w) = at(&top, e);
        assert_eq!(g, w, "{e}");
        assert_eq!(g, want, "{e}");
    }
    let bottom = ground(u64::MAX);
    for (e, want) in [
        ("-9223372036854775808.5", out(i128::MIN)),
        (
            "-9223372036854775808.4",
            out(i128::MIN + 1_844_674_407_370_955_162),
        ),
        ("-9223372036854775809", out(i128::MIN)),
        ("1", out(i128::from(u64::MAX))),
    ] {
        let (g, w) = at(&bottom, e);
        assert_eq!(g, w, "{e}");
        assert_eq!(g, want, "{e}");
    }
    // One step from the frame's edge either way, in every unit.
    for unit in [LinearUnit::METRE, LinearUnit::US_SURVEY_FOOT, one_step] {
        let g = FrameRecord::map(NZTM, GridStep::Q0, [5, -7, 11])
            .unwrap()
            .with_linear_unit(unit);
        let s = int(6_400_000) * unit_value(unit);
        for (steps, ok) in [
            (COORD_MAX, true),
            (COORD_MAX + 1, false),
            (-COORD_MAX, true),
            (-COORD_MAX - 1, false),
        ] {
            let v = [0, 1, 2].map(|a| (int(g.origin()[a].into()) + int(steps.into())) / &s);
            let p = DecimalPoint {
                easting: decimal_near(&v[0], 38),
                northing: decimal_near(&v[1], 38),
                height: decimal_near(&v[2], 38),
            };
            let want = expected(&g, &values_dec(p));
            assert_eq!(want.is_ok(), ok, "{unit:?} {steps}");
            assert_eq!(got(g.to_frame_decimal(p)), want);
        }
    }
}

#[test]
fn inputs_beyond_every_budget_report_the_larger_coordinate_saturated() {
    // Images beyond 2^200 steps: the report is the height, or the larger of x and y, saturated
    // with its true sign, which the model computes exactly. Images below that are exact.
    let mut rng = Rng::new("beyond the budgets", 0x4245_594f_4e44_0007);
    let budget = pow2(200);
    let mut beyond = 0;
    for _ in 0..2000 {
        let f = frame(&mut rng);
        let huge = |rng: &mut Rng| -> f64 {
            let v = rng.range(1, 1 << 52) as f64 * pow2f(rng.range(150, 970) as i32);
            if rng.coin() { -v } else { v }
        };
        let small = |rng: &mut Rng| rng.range(-1_000_000, 1_000_000) as f64 / 8.0;
        let (e, n, h) = match rng.below(4) {
            0 => (huge(&mut rng), small(&mut rng), small(&mut rng)),
            1 => (small(&mut rng), huge(&mut rng), small(&mut rng)),
            2 => (huge(&mut rng), huge(&mut rng), small(&mut rng)),
            _ => (small(&mut rng), small(&mut rng), huge(&mut rng)),
        };
        let m = MapPoint {
            easting: e,
            northing: n,
            height: h,
        };
        let r = f.to_frame(m);
        let s = int(i128::from(steps_per_metre(f.step())));
        let image = |v: f64| (exact(v) * &s * unit_value(f.linear_unit())).abs();
        let [x, y, z] = exact_frame(&f, &values_f64(m));
        // The square of a coordinate's value, and its sign.
        let square = |e: &Exact| match e {
            Exact::Rational(q) => q * q,
            Exact::Root(a, n) => a * a / int(i128::from(*n)),
        };
        let negative = |e: &Exact| match e {
            Exact::Rational(q) | Exact::Root(q, _) => q.is_negative(),
        };
        let sat = |e: &Exact| NumError::OutOfRange {
            value: if negative(e) { i128::MIN } else { i128::MAX },
        };
        if image(h) > budget {
            assert_eq!(r, Err(sat(&z)), "{f:?} {m:?}");
            beyond += 1;
        } else if image(e) <= budget && image(n) <= budget {
            assert_eq!(got(r), expected(&f, &values_f64(m)), "{f:?} {m:?}");
        } else if round_exact(&z).0.abs() > BigInt::from(COORD_MAX) {
            assert_eq!(
                r,
                Err(NumError::OutOfRange {
                    value: reported(&round_exact(&z).0)
                })
            );
        } else {
            beyond += 1;
            let (sx, sy) = (square(&x), square(&y));
            assert!(sx.clone().max(sy.clone()) > pow2(260), "{f:?} {m:?}");
            // Either coordinate when the two are this close; otherwise the larger.
            let close = (&sx - &sy).abs() < &sx * pow2(-100);
            let want = if sx >= sy { sat(&x) } else { sat(&y) };
            assert!(
                r == Err(want) || (close && r == Err(sat(&y))),
                "{f:?} {m:?}: {r:?}"
            );
        }
    }
    assert!(beyond > 500, "only {beyond} inputs beyond the budgets");
    // Decimal inputs beyond the budgets too.
    let f = FrameRecord::map(NZTM, GridStep::Q0, [0; 3]).unwrap();
    let p = DecimalPoint {
        easting: Decimal::parse("-1e300").unwrap(),
        northing: Decimal::ZERO,
        height: Decimal::ZERO,
    };
    assert_eq!(
        f.to_frame_decimal(p),
        Err(NumError::OutOfRange { value: i128::MIN })
    );
    let p = DecimalPoint {
        height: Decimal::parse("1e300").unwrap(),
        ..p
    };
    assert_eq!(
        f.to_frame_decimal(p),
        Err(NumError::OutOfRange { value: i128::MAX })
    );
}

// ---------------------------------------------------------------------------------------------
// Frame to map, and round trips.

#[test]
fn grid_points_go_to_the_nearest_doubles_in_any_unit() {
    let mut rng = Rng::new("frame to map in units", 0x464d_4150_554e_0008);
    for _ in 0..2000 {
        let f = frame(&mut rng);
        let c = |rng: &mut Rng| match rng.below(3) {
            0 => rng.range(-3, 3) as i64,
            1 => rng.pick(&[COORD_MAX, -COORD_MAX]),
            _ => rng.range(-i128::from(COORD_MAX), i128::from(COORD_MAX)) as i64,
        };
        let p = P3::new(c(&mut rng), c(&mut rng), c(&mut rng)).unwrap();
        check_to_map(&f, p);
    }
}

#[test]
fn rational_map_values_round_at_every_edge_of_a_double() {
    // One unit is one step and the frame has no rotation, so a Ground frame with scale k_n/k_d
    // maps the grid point (x, 0, 0) to E = O + x·k_n/k_d units exactly, through the wide
    // quotient: exact values, ties to even both ways, a sticky bit, a carry into the exponent,
    // tiny and huge quotients, both signs.
    let one_step = LinearUnit::new(1, 6_400_000).unwrap();
    let two = |k: u32| 1u64 << k;
    for (kn, kd, x, origin, want) in [
        (3, 4, 1, 0, Some((0.75, true))),
        (two(54) - 1, two(54), 1, 0, Some((1.0, false))),
        (two(53) + 1, two(53), 1, 0, Some((1.0, false))),
        (two(53) + 3, two(53), 1, 0, Some((1.0 + pow2f(-51), false))),
        (
            two(60) + two(7) + 1,
            two(60),
            1,
            0,
            Some((1.0 + pow2f(-52), false)),
        ),
        (two(54) - 1, two(54), -1, 0, Some((-1.0, false))),
        (1, two(63), 1, 0, Some((pow2f(-63), true))),
        (1, 3, 1, 0, None),
        (u64::MAX, 1, COORD_MAX, 0, None),
        (u64::MAX, 7, -COORD_MAX, 1 << 60, None),
        (5, 3, 3, -(1 << 60), None),
    ] {
        let f = FrameRecord::new(
            NZTM,
            GridStep::Q0,
            [origin, 0, 0],
            Rotation::IDENTITY,
            Scale::new(kn, kd).unwrap(),
        )
        .unwrap()
        .with_linear_unit(one_step);
        let p = P3::new(x, 0, 0).unwrap();
        check_to_map(&f, p);
        if let Some((e, exact)) = want {
            let m = f.to_map(p).unwrap();
            assert_eq!((m.value().easting, m.is_exact()), (e, exact), "{kn}/{kd}");
        }
    }
}

#[test]
fn irrational_map_values_that_cancel_the_origin_are_still_the_nearest_doubles() {
    // With the direction (1, 1), κ = k_n·a/(k_d·√2); taking k_n·a/k_d = x with x² − 2y² = ±1
    // (Pell's equation) puts κ within 1/(2√2·x) of y, so an origin of −y cancels all but that.
    // The double-double estimate's error grows with κ, so for large x its guess is wrong and the
    // bisection decides; for small x the guess stands. Both against the exact model.
    let (mut x, mut y) = (1i128, 1i128);
    let mut cases = 0;
    while y < 1 << 60 {
        for unit in [LinearUnit::METRE, LinearUnit::US_SURVEY_FOOT] {
            for sign in [1i64, -1] {
                let o = -sign * y as i64;
                let f = FrameRecord::new(
                    NZTM,
                    GridStep::Q0,
                    [o, o, 0],
                    Rotation::direction(1, 1).unwrap(),
                    Scale::new(x as u64, 1).unwrap(),
                )
                .unwrap()
                .with_linear_unit(unit);
                check_to_map(&f, P3::new(sign, 0, 0).unwrap());
                cases += 1;
            }
        }
        (x, y) = (x + 2 * y, x + y);
    }
    assert!(cases > 150, "{cases}");
}

#[test]
fn round_trips_at_nztm_extremes_and_the_frame_range() {
    // NZTM's corners (easting 1,000,000 to 2,200,000 m, northing 4,700,000 to 6,300,000 m), every
    // grid point combination at 0 and ±(2^40 − 1), at q0 and q1, in metres and in feet, in Map
    // and Ground frames: grid → map → grid is the identity (a map value's half ulp is far below a
    // step at these magnitudes), and in metres the map value's exact decimal text comes back
    // exactly.
    let ends = [-COORD_MAX, 0, COORD_MAX];
    for (e, n) in [
        (1_000_000, 4_700_000),
        (1_000_000, 6_300_000),
        (2_200_000, 4_700_000),
        (2_200_000, 6_300_000),
        (1_757_000, 5_921_000),
    ] {
        for step in [GridStep::Q0, GridStep::Q1] {
            let s = i64::try_from(steps_per_metre(step)).unwrap();
            let origin = [e * s, n * s, 25 * s];
            let kinds = [
                (Rotation::IDENTITY, Scale::ONE),
                (Rotation::quarter_turns(1), Scale::ONE),
                (
                    Rotation::direction(3, 4).unwrap(),
                    Scale::new(9_999_876_543, 10_000_000_000).unwrap(),
                ),
                (
                    Rotation::direction(1, 2).unwrap(),
                    Scale::new(9_996, 10_000).unwrap(),
                ),
            ];
            for (rot, k) in kinds {
                for unit in [
                    LinearUnit::METRE,
                    LinearUnit::INTERNATIONAL_FOOT,
                    LinearUnit::US_SURVEY_FOOT,
                ] {
                    let f = FrameRecord::new(NZTM, step, origin, rot, k)
                        .unwrap()
                        .with_linear_unit(unit);
                    for x in ends {
                        for y in ends {
                            for z in ends {
                                let p = P3::new(x, y, z).unwrap();
                                let m = f.to_map(p).unwrap();
                                assert_eq!(
                                    f.to_frame(m.value()).map(|r| r.value()),
                                    Ok(p),
                                    "{f:?} {p:?}"
                                );
                                if unit.is_metre()
                                    && rot == Rotation::IDENTITY
                                    && step == GridStep::Q0
                                {
                                    // (O + c)/6,400,000 m = (O + c)·15625·10^−11 exactly.
                                    let text = |o: i64, c: i64| {
                                        Decimal::new(i128::from(o + c) * 15_625, -11).unwrap()
                                    };
                                    let d = DecimalPoint {
                                        easting: text(origin[0], x),
                                        northing: text(origin[1], y),
                                        height: text(origin[2], z),
                                    };
                                    let back = f.to_frame_decimal(d).unwrap();
                                    assert_eq!((back.value(), back.is_exact()), (p, true));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Shifts.

#[test]
fn shifts_decide_the_lattice_before_the_range_and_report_exact_offsets() {
    let ground_at = |step: GridStep, origin: [i64; 3], dir: (i32, i32), k: (u64, u64)| {
        FrameRecord::new(
            NZTM,
            step,
            origin,
            Rotation::direction(dir.0, dir.1).unwrap(),
            Scale::new(k.0, k.1).unwrap(),
        )
        .unwrap()
    };
    let ground =
        |origin: [i64; 3], dir: (i32, i32), k: (u64, u64)| ground_at(GridStep::Q0, origin, dir, k);
    // Far apart and off the lattice (c·ΔE + s·ΔN = 3·(2^59 + 1) + 4·0, not a multiple of 5): the
    // frames are refused as off the lattice, though the offset would be beyond 2^62.
    let a = ground([0; 3], (3, 4), (1, 1 << 40));
    let b = ground([(1 << 59) + 1, 0, 0], (3, 4), (1, 1 << 40));
    assert_eq!(
        a.shift_to(&b),
        Err(MathError::FrameMismatch(FrameMismatch::NotOnLattice))
    );
    // On the lattice and far apart: the exact offset, beyond 2^62, in the error.
    let c = ground([5 << 40, 0, 0], (3, 4), (1, 1 << 40));
    let want = -(3 * (5i128 << 40) * (1 << 40)) / 5;
    assert_eq!(
        a.shift_to(&c),
        Err(MathError::Num(NumError::OutOfRange { value: want }))
    );
    // Exact within i128: (2^64 − 1)·2^60.
    let d = ground([0; 3], (1, 0), (1, u64::MAX));
    let e = ground([-(1 << 60), 0, 0], (1, 0), (1, u64::MAX));
    let want = i128::from(u64::MAX) << 60;
    assert_eq!(
        d.shift_to(&e),
        Err(MathError::Num(NumError::OutOfRange { value: want }))
    );
    assert_eq!(
        e.shift_to(&d),
        Err(MathError::Num(NumError::OutOfRange { value: -want }))
    );
    // Beyond i128, refining two rungs (625·2^60 + 2^60 steps, times 2^64 − 1): saturated.
    let far = |o: i64, step| ground_at(step, [o, 0, 0], (1, 0), (1, u64::MAX));
    let (up, down) = (1i64 << 60, -(1i64 << 60));
    assert_eq!(
        far(up, GridStep::Q0).shift_to(&far(down, GridStep::Q2)),
        Err(MathError::Num(NumError::OutOfRange { value: i128::MAX }))
    );
    assert_eq!(
        far(down, GridStep::Q0).shift_to(&far(up, GridStep::Q2)),
        Err(MathError::Num(NumError::OutOfRange { value: i128::MIN }))
    );
    // 64-bit scales on a shared lattice: the shift moves points exactly, as the model says.
    let mut rng = Rng::new("shifts", 0x5348_4946_5453_0009);
    for _ in 0..1500 {
        let (c, sn) = rng.pick(&[(3, 4), (1, 0), (0, -1), (-20, 21)]);
        let (kn, kd) = rng.pick(&SCALES);
        let r = i128::from(Rotation::direction(c, sn).unwrap().exact_norm().unwrap());
        let k = Scale::new(kn, kd).unwrap();
        let from = ground(
            [
                rng.range(-1 << 50, 1 << 50) as i64,
                rng.range(-1 << 50, 1 << 50) as i64,
                0,
            ],
            (c, sn),
            (kn, kd),
        );
        // Move the origin by k·R·t whole steps (t chosen so the multiple is whole): on the lattice.
        let t = [rng.range(-1000, 1000), rng.range(-1000, 1000)];
        let scale_num = i128::from(k.num()) * r;
        let lift = |v: i128| v * scale_num / i128::from(k.den()).max(1);
        let (dx, dy) = (
            lift(i128::from(c) * t[0] - i128::from(sn) * t[1]),
            lift(i128::from(sn) * t[0] + i128::from(c) * t[1]),
        );
        let Ok(dx) = i64::try_from(dx) else { continue };
        let Ok(dy) = i64::try_from(dy) else { continue };
        let to_origin = [
            from.origin()[0].saturating_add(dx),
            from.origin()[1].saturating_add(dy),
            3,
        ];
        let Ok(to) = FrameRecord::new(NZTM, GridStep::Q0, to_origin, from.rotation(), k) else {
            continue;
        };
        match from.shift_to(&to) {
            Ok(shift) => {
                // The exact offset: k_d·R⁻¹(ΔO)/k_n in the target's axes, a whole number.
                let de = int(i128::from(from.origin()[0] - to.origin()[0]));
                let dn = int(i128::from(from.origin()[1] - to.origin()[1]));
                let kk = BigRational::new(
                    BigInt::from(k.den()),
                    BigInt::from(k.num()) * BigInt::from(r),
                );
                let ox = (int(c.into()) * &de + int(sn.into()) * &dn) * &kk;
                let oy = (int(c.into()) * &dn - int(sn.into()) * &de) * &kk;
                assert!(ox.is_integer() && oy.is_integer());
                assert_eq!(
                    shift.offset(),
                    [
                        ox.to_integer().to_i64().unwrap(),
                        oy.to_integer().to_i64().unwrap(),
                        -3
                    ]
                );
            }
            Err(MathError::FrameMismatch(FrameMismatch::NotOnLattice)) => {
                let de = int(i128::from(from.origin()[0] - to.origin()[0]));
                let dn = int(i128::from(from.origin()[1] - to.origin()[1]));
                let kk = BigRational::new(
                    BigInt::from(k.den()),
                    BigInt::from(k.num()) * BigInt::from(r),
                );
                let ox = (int(c.into()) * &de + int(sn.into()) * &dn) * &kk;
                let oy = (int(c.into()) * &dn - int(sn.into()) * &de) * &kk;
                assert!(!ox.is_integer() || !oy.is_integer());
            }
            Err(MathError::Num(NumError::OutOfRange { value })) => {
                assert!(value.unsigned_abs() > 1 << 62);
            }
            Err(e) => panic!("{e:?}"),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Bit identity on every target.

struct Digest(Sha256);

impl Digest {
    fn eat(&mut self, v: u64) {
        self.0.write(&v.to_le_bytes());
    }

    fn rounded(&mut self, r: Result<geom_math::Rounded<P3>, NumError>) {
        match r {
            Ok(p) => {
                let v = p.value();
                for c in [v.x.get(), v.y.get(), v.z.get()] {
                    self.eat(c as u64);
                }
                self.eat(u64::from(p.is_exact()));
            }
            Err(NumError::OutOfRange { value }) => {
                self.eat(1);
                self.eat(value as u64);
                self.eat((value >> 64) as u64);
            }
            Err(_) => self.eat(2),
        }
    }
}

/// The workload of piece G48: decimal text, units, conversions both ways in every unit and from
/// both input forms, out-of-range values and shifts with 64-bit scales, hashed in a fixed order.
fn workload(seed: u64) -> String {
    let mut rng = Rng(seed);
    let mut h = Digest(Sha256::new());
    for i in 0..1200 {
        let f = frame(&mut rng);
        let spread = if i % 5 == 0 { 1 << 45 } else { 1 << 20 };
        let v = [0, 1, 2].map(|a| near_origin(&mut rng, &f, a, spread));
        let m = MapPoint {
            easting: to_f64(&v[0]),
            northing: to_f64(&v[1]),
            height: to_f64(&v[2]),
        };
        h.rounded(f.to_frame(m));
        let d = DecimalPoint {
            easting: decimal_near(&v[0], 20 + (i % 19) as u32),
            northing: decimal_near(&v[1], 20),
            height: decimal_near(&v[2], 12),
        };
        h.rounded(f.to_frame_decimal(d));
        let text = d.easting.to_string();
        h.0.write(text.as_bytes());
        if let Ok(p) = f.to_frame(m).map(|r| r.value())
            && let Ok(back) = f.to_map(p)
        {
            h.eat(back.value().easting.to_bits());
            h.eat(back.value().northing.to_bits());
            h.eat(back.value().height.to_bits());
            h.eat(u64::from(back.is_exact()));
        }
        let g = frame(&mut rng);
        match f
            .with_linear_unit(LinearUnit::METRE)
            .shift_to(&g.with_linear_unit(LinearUnit::METRE))
        {
            Ok(s) => {
                for o in s.offset() {
                    h.eat(o as u64);
                }
            }
            Err(MathError::Num(NumError::OutOfRange { value })) => h.eat(value as u64),
            Err(_) => h.eat(3),
        }
        let unit = LinearUnit::from_decimal(decimal_near(&v[0], 1 + (i % 12) as u32));
        h.eat(unit.map_or(0, |u| u.num() ^ u.den().rotate_left(17)));
    }
    hex(h.0.finish())
}

/// The workload's golden in the golden store (`geom-test/golden/store.json`): its SHA-256 digest,
/// the same on every target and thread count (principle 5, chapter 21).
const WORKLOAD: &str = "geom-math/frames";

#[test]
fn frame_results_identical_across_targets() {
    let checked = golden::check(WORKLOAD, || {
        let one = workload(0x6748_5752_4b4c_4430);
        let digest = golden::Digest::from_hex(&one).unwrap();
        (digest.0, one)
    });
    println!("geom-math frame workload digest: {}", checked.value);
    checked
        .verdict()
        .unwrap_or_else(|e| panic!("the frame workload's digest differs: {e}"));
}

#[test]
#[cfg_attr(
    all(target_family = "wasm", not(target_feature = "atomics")),
    ignore = "WebAssembly without the atomics feature (wasm32-wasip1) has no threads: \
              std::thread::scope cannot spawn"
)]
fn frame_results_identical_across_thread_counts() {
    let one = workload(0x6748_5752_4b4c_4430);
    for threads in geom_test::golden::THREAD_COUNTS {
        let digests: Vec<String> = std::thread::scope(|sc| {
            let handles: Vec<_> = (0..threads)
                .map(|_| sc.spawn(|| workload(0x6748_5752_4b4c_4430)))
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert!(
            digests.iter().all(|d| *d == one),
            "{threads} threads: {digests:?} vs {one}"
        );
    }
}
