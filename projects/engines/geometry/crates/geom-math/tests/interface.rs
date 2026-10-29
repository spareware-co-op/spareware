//! Tests of promises `geom_math`'s public interface makes that the other files leave unchecked,
//! each through the interface alone (the mutation gate judges the crate by these files only).
//!
//! What this file proves:
//!
//! - Every error prints its own sentence, with the coordinate for `FrameRangeExceeded` and the
//!   numbers layer's sentence for `Num`.
//! - `F2`'s negation negates both components and its squared length is the dot product with
//!   itself; `F3`'s components read back as given.
//! - `DD::exact_product` answers only inside its precondition: never for a factor that is not
//!   finite, a zero factor included, and never for a factor of 2^996 or more.
//! - Frame to map, irrational directions, where the origin cancels most of the rotated offset: the
//!   nearest double, on points whose double-double value lies on the near side of a midpoint while
//!   the exact value lies just past it (found by search, so the certificate must refuse them), and
//!   on a point whose value, about 2^−90 m, is far below the double-double value's own error.
//! - A shift between frames takes a vertical offset of exactly 2^62 steps and refuses one more.
//! - A grid image rounds to the nearest step, ties to even, with the exact flag right, on rows
//!   whose plain double sum, or whose double-double sum, lands within its error bound of a whole
//!   step or a midpoint on the far side from the exact value: each filter must refuse them; and a
//!   sum of tiny terms that cancel is exactly zero.
//! - A transform whose determinant is small but certain is classified by its sign, and the
//!   identity reverses no orientation.
//! - The exact stage's wide path breaks a tie to the even step.
//! - A length of exactly 2^1023 is finite.
//! - A rotation's target scaled by a power of two, down to the subnormals, gives the same
//!   rotation within the same certified angle.

#![allow(
    clippy::float_cmp,
    clippy::arithmetic_side_effects,
    reason = "exact values are compared exactly, and test cases are built with native arithmetic; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::similar_names,
    reason = "test cases built from bounded values, named as in the formulas; the rules are for kernel code"
)]

mod common;

use std::cmp::Ordering;

use common::{
    exact, int, is_integer, round_half_even, sign_minus_sqrt, step_down, step_up, to_i128,
};
use geom_math::frame::steps_per_metre;
use geom_math::{
    CrsId, DD, F2, F3, FrameMismatch, FrameRecord, MathError, RationalRotation, Rotation, Scale,
    Transform, TransformKind,
};
use geom_num::grid::{GridStep, P3};
use geom_num::{NumError, Real};
use num_bigint::BigInt;

/// 2^k for a normal exponent k, from its bits.
fn pow2(k: i32) -> f64 {
    f64::from_bits(u64::try_from(k + 1023).unwrap() << 52)
}

#[test]
fn every_error_prints_its_own_sentence() {
    let mismatch = [
        (
            FrameMismatch::Crs,
            "the frames use different coordinate reference systems",
        ),
        (
            FrameMismatch::VerticalCrs,
            "the frames' heights use different vertical coordinate reference systems",
        ),
        (
            FrameMismatch::Rotation,
            "the frames are rotated differently",
        ),
        (
            FrameMismatch::Scale,
            "the frames have different scale factors",
        ),
        (
            FrameMismatch::NotOnLattice,
            "the frames' origins do not differ by whole grid steps along the target's axes",
        ),
    ];
    for (m, text) in mismatch {
        assert_eq!(m.to_string(), text);
        assert_eq!(
            MathError::FrameMismatch(m).to_string(),
            format!("no exact shift between the frames: {text}")
        );
    }
    assert_eq!(
        MathError::Singular.to_string(),
        "the transform is singular and has no inverse"
    );
    assert_eq!(
        MathError::NotInPlane.to_string(),
        "the transform changes z, so it does not act in the plane"
    );
    assert_eq!(
        MathError::FrameRangeExceeded { value: -12_345 }.to_string(),
        "coordinate -12345 grid steps does not fit the frame's range of ±(2^40 − 1) steps"
    );
    for e in [
        NumError::Degenerate,
        NumError::NotFinite,
        NumError::BudgetExceeded,
    ] {
        assert_eq!(MathError::Num(e).to_string(), e.to_string());
    }
}

#[test]
fn float_vectors_negate_and_measure_componentwise() {
    let v = F2::new(3.0, -4.5).unwrap();
    let n = -v;
    assert_eq!((n.x(), n.y()), (-3.0, 4.5));
    assert_eq!(v.norm2(), Ok(3.0 * 3.0 + 4.5 * 4.5));
    assert_eq!(F2::new(0.5, 0.25).unwrap().norm2(), Ok(0.3125));
    let p = F3::new(2.5, -7.0, 11.0).unwrap();
    assert_eq!((p.x(), p.y(), p.z()), (2.5, -7.0, 11.0));
    assert_eq!(F3::new(-0.75, 0.0, 0.0).unwrap().x(), -0.75);
}

#[test]
fn exact_products_answer_only_inside_their_precondition() {
    // A factor that is not finite: no answer, even with a zero factor.
    for bad in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        for other in [0.0, -0.0, 2.0] {
            assert!(DD::exact_product(bad, other).is_none(), "{bad} · {other}");
            assert!(DD::exact_product(other, bad).is_none(), "{other} · {bad}");
        }
    }
    // A zero factor with a finite one: exactly zero.
    let zero = DD::exact_product(0.0, 1.5).unwrap();
    assert_eq!((zero.hi(), zero.lo()), (0.0, 0.0));
    // Both factors below 2^996, with a product well inside the range: the exact product.
    let below = 1.75 * pow2(995);
    let p = DD::exact_product(3.0, below).unwrap();
    assert_eq!((p.hi(), p.lo()), (3.0 * below, 0.0));
    let p = DD::exact_product(below, 3.0).unwrap();
    assert_eq!((p.hi(), p.lo()), (3.0 * below, 0.0));
    // A factor of 2^996 or more is outside the precondition, however small the other.
    for big in [pow2(996), 1.75 * pow2(996), f64::MAX] {
        for small in [1.0, 3.0, pow2(-40)] {
            assert!(DD::exact_product(small, big).is_none(), "{small} · {big}");
            assert!(DD::exact_product(big, small).is_none(), "{big} · {small}");
        }
    }
}

const NZTM: CrsId = CrsId::Epsg(2193);

/// Whether `d` is the double nearest the map coordinate V = (o + `k_n`·a/(`k_d`·√n))/S, by exact signs
/// at the midpoints either side of `d` (as `tests/frames.rs` checks irrational directions).
fn nearest_irrational(d: f64, o: i128, a: i128, scale: Scale, n2: u64, s: i128) -> bool {
    let (kn, kd) = (int(scale.num().into()), int(scale.den().into()));
    let at = |w: f64| ((exact(w) + exact(d)) / int(2) * int(s) - int(o)) * &kd / &kn;
    let a = int(a);
    sign_minus_sqrt(&a, &at(step_down(d)), n2) == Ordering::Greater
        && sign_minus_sqrt(&a, &at(step_up(d)), n2) == Ordering::Less
}

/// Converts the point (x, y, 0) of a frame at q0 with origin (o, 0, 0) and checks that both
/// horizontal map coordinates are the nearest doubles; returns the easting.
fn check_irrational(dir: (i32, i32), scale: (u32, u32), o: i64, x: i64, y: i64) -> f64 {
    let rot = Rotation::direction(dir.0, dir.1).unwrap();
    assert!(
        rot.exact_norm().is_none(),
        "{dir:?} is not an irrational direction"
    );
    let scale = Scale::new(scale.0, scale.1).unwrap();
    let f = FrameRecord::new(NZTM, GridStep::Q0, [o, 0, 0], rot, scale).unwrap();
    let r = f.to_map(P3::new(x, y, 0).unwrap()).unwrap();
    assert!(!r.is_exact());
    let s = i128::from(steps_per_metre(GridStep::Q0));
    let (c, sn) = (i128::from(rot.cos()), i128::from(rot.sin()));
    let (x, y) = (i128::from(x), i128::from(y));
    let m = r.value();
    for (d, o, a) in [
        (m.easting, i128::from(o), c * x - sn * y),
        (m.northing, 0, sn * x + c * y),
    ] {
        assert!(
            nearest_irrational(d, o, a, scale, rot.norm2(), s),
            "{d:e} is not the nearest double: direction {dir:?}, origin {o}, a {a}"
        );
    }
    m.easting
}

/// A direction, a scale (num, den), an easting origin and a point (x, y).
type Case = ((i32, i32), (u32, u32), i64, i64, i64);

#[test]
fn frame_to_map_rounds_values_just_past_a_midpoint() {
    // (direction, scale, easting origin, x, y). In each, the origin cancels all but about 2^−46 of
    // the rotated offset, so the double-double value's error is a sizeable fraction of an ulp, and
    // the exact value lies just past a midpoint between doubles from the double-double value:
    // above it in the first six, below it in the last six, for values of either sign.
    let cases: [Case; 12] = [
        (
            (-1_216_370_114, -1_821_055_087),
            (813_335, 2),
            -5_178_572_053_757_938,
            1_178_824_741,
            16_101_015_179,
        ),
        (
            (170_587_119, -1_266_409_819),
            (328_094, 3),
            -25_023_877_591_230_718,
            947_615_182,
            230_750_207_110,
        ),
        (
            (257_333_526, -537_000_743),
            (653_405, 2),
            32_858_502_835_154_013,
            143_744_412,
            -111_596_873_760,
        ),
        (
            (144_517_546, -148_939_419),
            (385_233, 1),
            7_875_247_015_191_804,
            60_571_462,
            -28_543_349_750,
        ),
        (
            (-489_765_664, -320_582_573),
            (114_857, 1),
            -21_420_152_708_261_694,
            58_221_372,
            340_611_978_235,
        ),
        (
            (1_288_358_327, -956_144_139),
            (275_227, 1),
            24_825_679_048_911_772,
            511_518_579,
            -152_044_553_008,
        ),
        (
            (179_984_194, -427_560_291),
            (1_040_186, 1),
            -27_890_404_434_823_348,
            132_217_547,
            29_036_082_951,
        ),
        (
            (-28_001_869, -1_758_797_741),
            (220_308, 1),
            -6_457_727_743_328_061,
            1_679_694_905,
            29_342_728_005,
        ),
        (
            (669_173_273, 1_910_629_623),
            (543_235, 1),
            22_198_574_921_041_434,
            1_510_646_568,
            43_826_565_802,
        ),
        (
            (273_198_119, 1_210_046_508),
            (586_137, 2),
            21_342_972_886_196_458,
            399_035_853,
            74_749_032_276,
        ),
        (
            (350_992_428, -97_902_107),
            (413_807, 1),
            -13_687_914_363_436_820,
            76_710_172,
            122_841_004_065,
        ),
        (
            (1_934_417_011, 930_039_830),
            (873_405, 4),
            18_913_491_567_954_785,
            839_376_176,
            201_649_579_247,
        ),
    ];
    let mut signs = [0; 2];
    for (dir, scale, o, x, y) in cases {
        let e = check_irrational(dir, scale, o, x, y);
        signs[usize::from(e > 0.0)] += 1;
        // Cancellation: a value of at most a thousandth of a metre, from an origin of 2^50 steps
        // or more.
        assert!(e.abs() < 1e-3, "{e:e}");
    }
    assert_eq!(signs, [6, 6]);
}

/// (u, v) with c·u − s·v = 1, for coprime c and s, by the extended Euclidean algorithm.
fn bezout(c: i128, s: i128) -> (i128, i128) {
    let (mut r0, mut r1, mut u0, mut u1, mut v0, mut v1) = (c, -s, 1i128, 0i128, 0i128, 1i128);
    while r1 != 0 {
        let q = r0 / r1;
        (r0, r1) = (r1, r0 - q * r1);
        (u0, u1) = (u1, u0 - q * u1);
        (v0, v1) = (v1, v0 - q * v1);
    }
    // c·u0 + (−s)·v0 = r0 = ±1.
    (u0 * r0, v0 * r0)
}

/// (q, p): the last convergent p/q of α = `k_n`/(`k_d`·√n) with q ≤ 2^limit, so that q·α − p is
/// about 1/q from zero, the closest any multiplier of that size can bring α to a whole number.
fn convergent(n2: u64, scale: Scale, limit: u32) -> (i128, i128) {
    // α to about 200 bits: `k_n`·2^200 / (`k_d`·⌊√(n·2^400)⌋).
    let root = (BigInt::from(n2) << 400u32).sqrt();
    let mut num = BigInt::from(scale.num()) << 200u32;
    let mut den = BigInt::from(scale.den()) * root;
    let (mut p0, mut q0) = (BigInt::from(0), BigInt::from(1));
    let (mut p1, mut q1) = (BigInt::from(1), BigInt::from(0));
    let cap = BigInt::from(1) << limit;
    while den != BigInt::from(0) {
        let (part, rest) = (&num / &den, &num % &den);
        let (p2, q2) = (&part * &p1 + &p0, &part * &q1 + &q0);
        if q2 > cap {
            break;
        }
        (p0, q0, p1, q1) = (p1, q1, p2, q2);
        (num, den) = (den, rest);
    }
    (i128::try_from(q1).unwrap(), i128::try_from(p1).unwrap())
}

#[test]
fn frame_to_map_rounds_a_value_far_below_its_own_error_bound() {
    let (dir, scale) = ((1_234_567_891, -987_654_323), (524_287, 1));
    let rot = Rotation::direction(dir.0, dir.1).unwrap();
    let (c, sn) = (i128::from(rot.cos()), i128::from(rot.sin()));
    // The rotated offset a = q and the origin −p of a convergent: o + `k_n`·a/(`k_d`·√n) is about
    // 2^−68 steps.
    let (a, p) = convergent(rot.norm2(), Scale::new(scale.0, scale.1).unwrap(), 68);
    // c·x − s·y = a with 0 ≤ x < |s|.
    let (u, _) = bezout(c, sn);
    let x = (a * u).rem_euclid(sn.abs());
    let y = (c * x - a) / sn;
    assert_eq!(c * x - sn * y, a);
    let o = i64::try_from(-p).unwrap();
    let e = check_irrational(dir, scale, o, x as i64, y as i64);
    // About 2^−90 m: far below the double-double value's error, which is about 2^−98 of the
    // rotated offset, 2^−34 m here.
    assert!(e != 0.0 && e.abs() < 1e-24, "{e:e}");
}

#[test]
fn a_shift_takes_an_offset_of_2_62_steps_and_refuses_one_more() {
    let id = Rotation::direction(1, 0).unwrap();
    let one = Scale::new(1, 1).unwrap();
    // 625·7378697629483821 − 2^62 = 221: a q0 origin height and a q2 one exactly 2^62 q2 steps
    // apart; a q2 height of 220 makes them one step farther apart.
    let base = 7_378_697_629_483_821i64;
    let source = FrameRecord::new(NZTM, GridStep::Q0, [0, 0, base], id, one).unwrap();
    for (height, ok) in [(221, true), (222, true), (220, false)] {
        let target = FrameRecord::new(NZTM, GridStep::Q2, [0, 0, height], id, one).unwrap();
        let size = 625 * i128::from(base) - i128::from(height);
        match source.shift_to(&target) {
            Ok(shift) => {
                assert!(ok, "{size}");
                assert_eq!(i128::from(shift.offset()[2]).abs(), size);
            }
            Err(e) => {
                assert!(!ok, "{size}: {e:?}");
                assert_eq!(e, MathError::Num(NumError::OutOfRange { value: size }));
            }
        }
    }
}

/// Applies a transform whose first row is `row` (its other rows copy x and z) to the grid point
/// `v`, and checks the first coordinate is the exact value rounded to the nearest step, ties to
/// even, with the exact flag right.
fn check_row(row: [f64; 4], v: [i64; 3]) {
    let t = Transform::from_rows([row, [1.0, 0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]]).unwrap();
    let got = t.apply_grid(P3::new(v[0], v[1], v[2]).unwrap()).unwrap();
    let value = exact(row[0]) * int(v[0].into())
        + exact(row[1]) * int(v[1].into())
        + exact(row[2]) * int(v[2].into())
        + exact(row[3]);
    assert_eq!(
        i128::from(got.value().x.get()),
        to_i128(&round_half_even(&value)),
        "{row:?} {v:?}"
    );
    assert_eq!(got.is_exact(), is_integer(&value), "{row:?} {v:?}");
}

#[test]
fn grid_images_round_rows_whose_sums_mislead() {
    // Rows (as bits) and points whose plain double sum lands within its error bound of a whole step
    // or a midpoint, on the far side from the exact value (found by search): the filter must refuse
    // them, whichever terms of its error bound are large and whichever small.
    let floats: [([u64; 4], [i64; 3]); 7] = [
        (
            [
                13_800_088_646_199_046_416,
                4_636_542_898_653_080_414,
                9_223_372_036_854_775_808,
                4_605_528_787_574_200_429,
            ],
            [3, 3, 302_382_538_118],
        ),
        (
            [
                13_720_222_882_212_012_787,
                13_934_628_128_717_704_012,
                13_723_034_304_949_366_581,
                4_598_186_683_659_505_602,
            ],
            [197_570_708_500, 0, -3],
        ),
        (
            [
                4_557_835_963_210_325_804,
                4_557_885_557_627_847_924,
                0,
                4_602_677_552_694_165_520,
            ],
            [-421_033_947_376, 416_634_566_336, 416_435_347_467],
        ),
        (
            [
                4_614_321_898_245_484_095,
                4_610_737_876_854_328_519,
                4_735_605_145_369_861_786,
                4_603_844_851_218_494_924,
            ],
            [-158_011_625_086, 279_963_438_408, 0],
        ),
        (
            [
                13_773_948_829_054_700_974,
                9_223_372_036_854_775_808,
                4_564_342_554_621_623_720,
                4_590_073_817_154_733_875,
            ],
            [130_995_460_234, -366_509_382_420, 233_450_774_702],
        ),
        (
            [
                13_875_014_458_290_828_434,
                4_652_942_106_136_125_965,
                13_705_976_657_154_790_998,
                13_671_448_807_979_258_426,
            ],
            [327_432_043_241, 264_060_968_293, 0],
        ),
        (
            [
                4_688_340_221_748_307_508,
                13_916_703_332_751_642_226,
                13_734_082_746_051_126_462,
                4_602_011_288_366_872_593,
            ],
            [415_082_916_508, 187_641_870_268, -161_819_145_631],
        ),
    ];
    for (bits, v) in floats {
        check_row(bits.map(f64::from_bits), v);
    }
    // Rows whose double-double sum loses a term of 3·2^−80: l₀x is about 2^39, so the double-double
    // sum of it and l₁y keeps nothing below about 2^−66, and l₂z then cancels l₀x exactly. The
    // exact value is then 0 exactly (an exact step the double-double sum puts 3·2^−80 away), and
    // 1/2 + 3·2^−80 (just above a midpoint the double-double sum lands on).
    let l = 1.0 + pow2(-52);
    let x = (1i64 << 39) - 1;
    for t in [-3.0 * pow2(-80), 0.5] {
        check_row([l, pow2(-80), -l, t], [x, 3, x]);
    }
}

#[test]
fn a_grid_image_of_tiny_terms_that_cancel_is_exactly_zero() {
    // Every term is 2^−140 in magnitude, far below a step, and they cancel: both filters see a
    // zero they cannot certify, and the exact stage finds the value exactly zero.
    check_row([pow2(-140), -pow2(-140), 0.0, 0.0], [1, 1, 0]);
    check_row([pow2(-140), pow2(-141), -pow2(-139), pow2(-141)], [3, 2, 1]);
}

#[test]
fn a_small_but_certain_determinant_decides_the_orientation() {
    // det = 2^−49 exactly (the rows need no scaling): small, but the enclosure of the determinant
    // of the exact entries, about ±2^−50.2 wide, certifies its sign. The enclosure of entries each
    // a step wider would not.
    let row_a = [1.0, 1.0, 0.0, 0.0];
    let row_b = [1.0, 1.0 + pow2(-49), 0.0, 0.0];
    let z = [0.0, 0.0, 1.0, 0.0];
    for (rows, mirror) in [([row_a, row_b, z], false), ([row_b, row_a, z], true)] {
        let c = Transform::from_rows(rows).unwrap().classify().unwrap();
        assert_eq!(c.kind, TransformKind::NonUniform, "{rows:?}");
        assert_eq!(c.mirror, mirror, "{rows:?}");
        assert_eq!(c.reverses_orientation(), mirror, "{rows:?}");
    }
    let identity = Transform::IDENTITY.classify().unwrap();
    assert!(!identity.reverses_orientation());
}

#[test]
fn a_wide_exact_rounding_breaks_a_tie_to_even() {
    // Terms 34 binary places apart (0.5 = 2^−1 and 2^−35 times a coordinate) take the exact stage's
    // wide path. ±(0.5 + 2) = ±2.5 is a tie, which goes to the even ±2, not away from zero; and
    // 0.5 + 3 = 3.5 goes to 4.
    check_row([0.5, pow2(-35), 0.0, 0.0], [1, 1 << 36, 0]);
    check_row([-0.5, pow2(-35), 0.0, 0.0], [1, -(1 << 36), 0]);
    check_row([0.5, pow2(-35), 0.0, 0.0], [1, 3 << 35, 0]);
}

#[test]
fn a_length_of_2_1023_is_finite() {
    assert_eq!(F2::new(pow2(1023), 0.0).unwrap().length(), Ok(pow2(1023)));
    assert_eq!(F2::new(0.0, -pow2(1023)).unwrap().length(), Ok(pow2(1023)));
    assert_eq!(
        F3::new(0.0, pow2(1023), 0.0).unwrap().length(),
        Ok(pow2(1023))
    );
}

#[test]
fn a_rotation_target_scaled_by_a_power_of_two_gives_the_same_rotation() {
    let bound = Real::new(pow2(-20)).unwrap();
    let tiny = f64::from_bits(1); // 2^−1074, the smallest subnormal
    let plane = RationalRotation::plane_within(F2::new(3.0, 4.0).unwrap(), bound).unwrap();
    for scale in [tiny, tiny * 1024.0, pow2(-1022), pow2(-500), pow2(500)] {
        let got = RationalRotation::plane_within(F2::new(3.0 * scale, 4.0 * scale).unwrap(), bound)
            .unwrap();
        assert_eq!(
            got.value().quaternion(),
            plane.value().quaternion(),
            "{scale:e}"
        );
        assert_eq!(got.angle(), plane.angle(), "{scale:e}");
    }
    let target = [1.0, 2.0, -3.0, 5.0];
    let space = RationalRotation::within(target, bound).unwrap();
    for scale in [tiny, tiny * 1024.0, pow2(-1022), pow2(-500), pow2(500)] {
        let got = RationalRotation::within(target.map(|v| v * scale), bound).unwrap();
        assert_eq!(
            got.value().quaternion(),
            space.value().quaternion(),
            "{scale:e}"
        );
        assert_eq!(got.angle(), space.angle(), "{scale:e}");
    }
}
