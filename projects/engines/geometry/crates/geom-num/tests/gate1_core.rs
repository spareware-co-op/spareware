//! Tests that kill the mutants the first full mutation run (gate 1, chapter 32.43) found surviving
//! in geom-num outside `math/`: the generated filters `lpi_side`, `lpi_weight`, `ssi_weight` and
//! `tpi_weight`, the generated exact paths of `lpi_side`, `ssi_side` and `tpi_side`, and the
//! double-double, through the crate's public interface only.
//!
//! Each test names the mutants it kills, in cargo-mutants' words. They survived because the
//! routed callers (`Plane::side_of_lpi`, `Plane::side_of_tpi`, `Ssi::orient2d`) fall back to their
//! hand-written exact paths, so a filter that gives up, or an exact path that answers `None`, never
//! changes their answer; here the generated functions are called directly. The filters' fixtures
//! were found by a search that replays each filter and each mutant of its scale line in IEEE
//! binary64 (kept outside the repository); every one is checked here against the exact predicate.
//! The survivors no input can tell apart are in the quality ledger, each with its reason.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them to state the values they expect.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "expected values and oracles; the rules are for kernel code"
)]
#![allow(
    clippy::many_single_char_names,
    reason = "points are a to d, p and q, as in the predicates' descriptions"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a failing test should stop loudly"
)]

use std::collections::BTreeSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use geom_num::dd::{
    ADD_BOUND, ADD_F64_BOUND, DIV_BOUND, DIV_F64_BOUND, MUL_BOUND, MUL_F64_BOUND, SQRT_BOUND,
    UNDERFLOW_BOUND,
};
use geom_num::generated::{lpi_side, lpi_weight, ssi_side, ssi_weight, tpi_side, tpi_weight};
use geom_num::{DD, P2, P3, Sign};

fn p2(x: i64, y: i64) -> P2 {
    P2::new(x, y).unwrap()
}

fn p3([x, y, z]: [i64; 3]) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// 2^k for a normal power of two, from its bits.
fn pow2(k: i32) -> f64 {
    assert!((-1022..=1023).contains(&k));
    f64::from_bits(((1023 + k) as u64) << 52)
}

// ---------------------------------------------------------------------------------------------
// The generated predicates, called directly.
// ---------------------------------------------------------------------------------------------

/// One input of `lpi_side`: plane 1 (n1, d1), plane 2 (n2, d2), and the line from p to q.
struct LpiSide {
    n1: [i128; 3],
    d1: i128,
    n2: [i128; 3],
    d2: i128,
    p: [i64; 3],
    q: [i64; 3],
}

impl LpiSide {
    /// The sign of w1 s2p − w2 s1p (the description's formula): each factor in `i128` (at most
    /// 2^83 · 2^41 · 3 + 2^125), their products in `num-bigint`.
    fn exact_sign(&self) -> Sign {
        let qp: [i128; 3] = std::array::from_fn(|i| i128::from(self.q[i] - self.p[i]));
        let p: [i128; 3] = self.p.map(i128::from);
        let dot = |n: [i128; 3], v: [i128; 3]| n[0] * v[0] + n[1] * v[1] + n[2] * v[2];
        let (w1, w2) = (dot(self.n1, qp), dot(self.n2, qp));
        let (s1p, s2p) = (dot(self.n1, p) + self.d1, dot(self.n2, p) + self.d2);
        let big = |v: i128| num_bigint::BigInt::from(v);
        let s = big(w1) * big(s2p) - big(w2) * big(s1p);
        match s.sign() {
            num_bigint::Sign::Plus => Sign::Positive,
            num_bigint::Sign::Minus => Sign::Negative,
            num_bigint::Sign::NoSign => Sign::Zero,
        }
    }

    fn filter(&self) -> Option<Sign> {
        let ([ax, ay, az], [bx, by, bz]) = (self.n1, self.n2);
        lpi_side::filter(
            ax,
            ay,
            az,
            self.d1,
            bx,
            by,
            bz,
            self.d2,
            p3(self.p),
            p3(self.q),
        )
    }

    fn exact(&self) -> Option<Sign> {
        let ([ax, ay, az], [bx, by, bz]) = (self.n1, self.n2);
        lpi_side::exact(
            ax,
            ay,
            az,
            self.d1,
            bx,
            by,
            bz,
            self.d2,
            p3(self.p),
            p3(self.q),
        )
    }
}

/// The inputs of [`lpi_side_filter_and_exact_path_at_their_scale`], each with what the filter
/// gives.
const LPI_SIDE_CASES: [(LpiSide, Option<Sign>); 10] = [
    // Plane 2 is all zeros: scale zero.
    (
        LpiSide {
            n1: [2, 2, -233_358_824_076_487_245],
            d1: 0,
            n2: [0, 0, 0],
            d2: 0,
            p: [3, -2, 752],
            q: [248_080, -1, 52_492],
        },
        Some(Sign::Zero),
    ),
    // Plane 1 is all zeros, the points in the plane z = 0: scale zero.
    (
        LpiSide {
            n1: [0, 0, 0],
            d1: 0,
            n2: [
                -2_689_667_172_233_838_271_317,
                -58_954_760_066_987_107_952,
                5643,
            ],
            d2: 2,
            p: [39_306_663_134, 204, 0],
            q: [42_216_305_481, 166_491, 0],
        },
        Some(Sign::Zero),
    ),
    (
        LpiSide {
            n1: [0, 0, 0],
            d1: 0,
            n2: [-652_694_365, 1_468_077_915_979_003_307_525, 2],
            d2: 1_299_493_247_340_153_136_774_922_002_001,
            p: [-144_097_967, 0, -2],
            q: [0, 0, -12],
        },
        Some(Sign::Zero),
    ),
    // Certified, the points in the plane z = 0.
    (
        LpiSide {
            n1: [-1_055_861_940, 0, -1],
            d1: -1_570_340_023_094,
            n2: [0, 62, -425_903_796],
            d2: -16_101_468_959_502_096_688_931_107_975_088,
            p: [11, -1_750_436_742, 0],
            q: [-1, 21_672_351_529, 0],
        },
        Some(Sign::Negative),
    ),
    // Certified, plane 2's normal zero.
    (
        LpiSide {
            n1: [502, 2, 6164],
            d1: -177_786_309_129_201_510_643_547_416_314_221_774,
            n2: [0, 0, 0],
            d2: -6_995_754_689_131_887_120_663_941_320_872_684,
            p: [234, 2, 0],
            q: [-2, 470_188, -4],
        },
        Some(Sign::Negative),
    ),
    // Certified, the points in the plane y = 0.
    (
        LpiSide {
            n1: [-184_152, 10, -2],
            d1: 1_163_955_065_274_096_146_820_146_576_913_056_283,
            n2: [306_527_896_834_831_526, 0, 521_664_790_050_985],
            d2: 0,
            p: [3, 0, -739_753_728],
            q: [72, 0, 241_501],
        },
        Some(Sign::Negative),
    ),
    // Certified, plane 1's normal zero.
    (
        LpiSide {
            n1: [0, 0, 0],
            d1: 3,
            n2: [0, 0, 956_903_552_190_510],
            d2: 0,
            p: [-1, -241_890, -1],
            q: [0, 0, 58_658_160_066],
        },
        Some(Sign::Negative),
    ),
    // Not certified: points on the x axis (exact value zero), on the z axis (zero), and on the
    // y axis (negative).
    (
        LpiSide {
            n1: [-6_416_039_800, 0, 2086],
            d1: 0,
            n2: [-6_034_406_507_175_050_600, 0, 12_916_506],
            d2: 0,
            p: [-5_604_624, 0, 0],
            q: [1, 0, 0],
        },
        None,
    ),
    (
        LpiSide {
            n1: [-1, 0, -2],
            d1: 0,
            n2: [1, 2, -18_630_418_146_364],
            d2: 0,
            p: [0, 0, 25_392],
            q: [0, 0, -3_017_306],
        },
        None,
    ),
    (
        LpiSide {
            n1: [0, -75_782_325_630_070_485_624_642, 0],
            d1: -222_696_737,
            n2: [-414_947, -4, 0],
            d2: 0,
            p: [0, -5863, 0],
            q: [0, 1_137_554, 0],
        },
        None,
    ),
];

/// `lpi_side`'s filter over inputs where a mutant of its scale line answers differently, each
/// checked against the exact sign: where the filter certifies, it must give that sign (a mutant
/// whose scale is larger, or not a number, gives up); where it gives up, a mutant whose scale is
/// smaller certifies a wrong sign. And its exact path gives the exact sign on every one.
///
/// Kills, in `lpi_side::filter`'s scale lines (`generated/lpi_side.rs:252 to 262`), every
/// `replace * with +` and `replace * with /` that survived (60), and
/// `replace exact -> Option<Sign> with None` (`generated/lpi_side.rs:349`).
///
/// The first three inputs have a scale of zero: plane 2's normal and offset are zero, or plane 1's,
/// so every term of the scale has a zero factor and the filter decides Zero, exactly (the module
/// documentation of `generated`: "`Some(Sign::Zero)` when the scale is zero, which makes the exact
/// value zero"). A `*` turned into `+` makes such a term nonzero, and the filter gives up. A `*`
/// turned into `/` divides by a zero group maximum, and the scale becomes infinite (or not a
/// number), so the filter gives up where it certified; the next four inputs certify with one group
/// zero, a coordinate axis or a plane's normal. The last three have every point on one or two
/// coordinate axes, where a term of the form `m_a · m_a · m_n1 · m_n2` leads the scale: turned into
/// `m_a / m_a · m_n1 · m_n2` it shrinks by `m_a`² and the filter certifies a sign the exact value
/// does not have (two of them are exactly zero).
#[test]
fn lpi_side_filter_and_exact_path_at_their_scale() {
    for (i, (case, filtered)) in LPI_SIDE_CASES.iter().enumerate() {
        let exact = case.exact_sign();
        assert_eq!(case.exact(), Some(exact), "case {i}: the exact path");
        assert_eq!(case.filter(), *filtered, "case {i}: the filter");
        if let Some(s) = filtered {
            assert_eq!(*s, exact, "case {i}: the fixture's own sign");
        }
    }
}

/// `lpi_weight`'s filter decides n · (q − p): Zero when q = p (its scale, |n| times the largest
/// |q − p|, is zero), and the sign of a clear case.
///
/// Kills `replace filter -> Option<Sign> with None` (`generated/lpi_weight.rs:128`), and
/// `replace * with +` and `replace * with /` in its scale, `m_n * larger(...)`
/// (`generated/lpi_weight.rs:143`): |n| + 0 is not zero, and |n| / 0 is infinite.
#[test]
fn lpi_weight_filter_decides_zero_scales_and_clear_cases() {
    let at = p3([10, 20, 30]);
    assert_eq!(lpi_weight::filter(3, -5, 7, at, at), Some(Sign::Zero));
    assert_eq!(lpi_weight::exact(3, -5, 7, at, at), Some(Sign::Zero));
    let (p, q) = (p3([0, 0, 0]), p3([5, 0, 0]));
    assert_eq!(lpi_weight::filter(1, 0, 0, p, q), Some(Sign::Positive));
    assert_eq!(lpi_weight::filter(-1, 0, 0, p, q), Some(Sign::Negative));
}

/// `ssi_weight`'s filter decides (b − a) × (d − c): Zero when every point lies on the x axis (its
/// scale, the largest |x| difference times the largest |y| difference, is zero), and the sign of a
/// clear case.
///
/// Kills `replace filter -> Option<Sign> with None` (`generated/ssi_weight.rs:117`), and
/// `replace * with +` and `replace * with /` in its scale, `m_x * m_y`
/// (`generated/ssi_weight.rs:126`).
#[test]
fn ssi_weight_filter_decides_zero_scales_and_clear_cases() {
    let (a, b, c, d) = (p2(0, 0), p2(5, 0), p2(1, 0), p2(3, 0));
    assert_eq!(ssi_weight::filter(a, b, c, d), Some(Sign::Zero));
    assert_eq!(ssi_weight::exact(a, b, c, d), Some(Sign::Zero));
    let (a, b, c, d) = (p2(0, 0), p2(1, 0), p2(0, 0), p2(0, 1));
    assert_eq!(ssi_weight::filter(a, b, c, d), Some(Sign::Positive));
    assert_eq!(ssi_weight::filter(b, a, c, d), Some(Sign::Negative));
}

/// `tpi_weight`'s filter decides the determinant of three normals: the coordinate axes, either
/// order.
///
/// Kills `replace filter -> Option<Sign> with None` (`generated/tpi_weight.rs:163`).
#[test]
fn tpi_weight_filter_decides_the_axes() {
    assert_eq!(
        tpi_weight::filter(1, 0, 0, 0, 1, 0, 0, 0, 1),
        Some(Sign::Positive)
    );
    assert_eq!(
        tpi_weight::filter(0, 1, 0, 1, 0, 0, 0, 0, 1),
        Some(Sign::Negative)
    );
}

/// `ssi_side`'s exact path: the lines from (0, 0) to (2, 2) and from (0, 2) to (2, 0) cross at
/// (1, 1). Against the line from p to q, `ssi_side` is oa · ob₂ − ob · oa₂ (its description), whose
/// sign times the weight's is the side: (1, 1) lies left of the x axis run left to right.
///
/// Kills `replace exact -> Option<Sign> with None` (`generated/ssi_side.rs:250`).
#[test]
fn ssi_side_exact_path_gives_the_side() {
    let (a, b, c, d) = (p2(0, 0), p2(2, 2), p2(0, 2), p2(2, 0));
    // oa = −4, ob = 4, oa₂ = 0, ob₂ = 2 for p = (0, 0), q = (1, 0): s = −8; the weight is −8.
    let (p, q) = (p2(0, 0), p2(1, 0));
    assert_eq!(ssi_side::exact(a, b, c, d, p, q), Some(Sign::Negative));
    assert_eq!(ssi_side::exact(a, b, c, d, q, p), Some(Sign::Positive));
    assert_eq!(ssi_weight::exact(a, b, c, d), Some(Sign::Negative));
    // The line through (1, 1) itself: zero.
    assert_eq!(
        ssi_side::exact(a, b, c, d, p2(0, 1), p2(1, 1)),
        Some(Sign::Zero)
    );
}

/// `tpi_side`'s exact path: the planes x = 1, y = 2 and z = 3 meet at (1, 2, 3), with weight 1;
/// against x + y + z + d4 = 0 the side is the sign of 6 + d4.
///
/// Kills `replace exact -> Option<Sign> with None` (`generated/tpi_side.rs:494`).
#[test]
fn tpi_side_exact_path_gives_the_side() {
    let side = |n4: [i128; 3], d4: i128| {
        let [sx, sy, sz] = n4;
        tpi_side::exact(1, 0, 0, -1, 0, 1, 0, -2, 0, 0, 1, -3, sx, sy, sz, d4)
    };
    assert_eq!(side([1, 1, 1], 0), Some(Sign::Positive));
    assert_eq!(side([1, 1, 1], -6), Some(Sign::Zero));
    assert_eq!(side([-1, -1, -1], 0), Some(Sign::Negative));
    assert_eq!(
        tpi_weight::exact(1, 0, 0, 0, 1, 0, 0, 0, 1),
        Some(Sign::Positive)
    );
}

// ---------------------------------------------------------------------------------------------
// The double-double.
// ---------------------------------------------------------------------------------------------

/// The published bounds are the values their documentation states, no larger: tests/dd.rs holds
/// every operation to them and checks that each is at least its theorem's, and a looser constant
/// would pass both while weakening every caller that budgets with it.
///
/// Kills `replace * with +` and `replace * with /` in `DIV_BOUND = 9.8 * U2` (`dd.rs:229`).
#[test]
fn the_published_bounds_are_the_documented_values() {
    let u2 = pow2(-106);
    let margin = pow2(-49);
    assert_eq!(ADD_BOUND, (3.0 + margin) * u2);
    assert_eq!(ADD_F64_BOUND, 2.0 * u2);
    assert_eq!(MUL_F64_BOUND, (2.0 + margin) * u2);
    assert_eq!(MUL_BOUND, 4.0 * u2);
    assert_eq!(DIV_F64_BOUND, (3.0 + margin) * u2);
    assert_eq!(DIV_BOUND, 9.8 * u2);
    assert_eq!(SQRT_BOUND, (3.125 + margin) * u2);
    assert_eq!(UNDERFLOW_BOUND, f64::from_bits(1));
}

/// `round_shifted` answers up to |hi| = 2^100 inclusive and refuses the next double, and reports
/// whether the value was already an integer.
///
/// Kills `replace > with >=` and `replace > with ==` in `magnitude(self.hi) > ROUND_LIMIT`
/// (`dd.rs:937`), and `replace || with &&` in
/// `te == 0.0 && (t == 0.0 || t == 1.0 || t == -1.0)` (`dd.rs:963`), which made every result
/// inexact.
#[test]
fn round_shifted_at_its_limit_and_its_exactness() {
    let limit = pow2(100);
    for (x, want) in [(limit, 1_i128 << 100), (-limit, -(1_i128 << 100))] {
        let r = DD::from_f64(x).unwrap().round_shifted(0).unwrap();
        assert_eq!((r.value(), r.is_exact()), (want, true), "{x}");
    }
    assert!(
        DD::from_f64(limit.next_up())
            .unwrap()
            .round_shifted(0)
            .is_none()
    );
    assert!(
        DD::from_f64(-limit.next_up())
            .unwrap()
            .round_shifted(0)
            .is_none()
    );
    // (value, offset, rounded, exact): integers (with a trailing part too) are exact, the rest
    // round half to even.
    let big = DD::from_sum(pow2(60), 1.0).unwrap();
    let cases = [
        (DD::from_f64(3.0).unwrap(), 0, 3, true),
        (DD::from_f64(3.0).unwrap(), 1, 2, true),
        (DD::from_f64(-7.0).unwrap(), -2, -5, true),
        (DD::ZERO, 5, -5, true),
        (big, 0, (1_i128 << 60) + 1, true),
        (DD::from_f64(2.5).unwrap(), 0, 2, false),
        (DD::from_f64(-2.25).unwrap(), 0, -2, false),
    ];
    for (x, offset, value, exact) in cases {
        let r = x.round_shifted(offset).unwrap();
        assert_eq!(
            (r.value(), r.is_exact()),
            (value, exact),
            "{x:?} − {offset}"
        );
    }
}

/// The multiplicative operations scale each operand to [1, 2) and drop a scaled trailing part
/// below 2^−400 (the module documentation, *Range*), and only below it: a trailing part of exactly
/// 2^−400 is kept, one of 2^−401 dropped, so multiplying by one keeps the first value and rounds
/// the second to one.
///
/// Kills `replace < with <=` and `replace < with ==` in
/// `if magnitude(lo) < TRAIL_FLOOR { 0.0 } else { lo }` (`dd.rs:659`).
#[test]
fn a_trailing_part_is_dropped_only_below_two_to_the_minus_400() {
    let kept = DD::from_sum(1.0, pow2(-400)).unwrap();
    assert_eq!(kept.lo(), pow2(-400));
    assert_eq!(kept.mul(DD::ONE).unwrap(), kept);
    assert_eq!(DD::ONE.mul(kept).unwrap(), kept);
    assert_eq!(kept.mul_f64(1.0).unwrap(), kept);
    let dropped = DD::from_sum(1.0, pow2(-401)).unwrap();
    assert_eq!(dropped.lo(), pow2(-401));
    assert_eq!(dropped.mul(DD::ONE).unwrap(), DD::ONE);
    assert_eq!(dropped.mul_f64(1.0).unwrap(), DD::ONE);
}

/// A product far below the smallest subnormal rounds to zero. 3·2^−1074 times 2^−1002 is
/// 3·2^−2076, below half of 2^−1074, so its correct rounding is +0; scaled, it is 1.5 times
/// 2^−2075, and scaling back goes through `scale_pow2`'s loop of 2^−1000 steps (an exponent below
/// −2074), whose every step must multiply.
///
/// Kills `replace *= with +=` in `v *= TWO_POW_NEG_1000` (`dd.rs:354`), which gave 2^−1074.
#[test]
fn a_product_far_below_the_subnormals_rounds_to_zero() {
    let x = DD::from_f64(3.0 * f64::from_bits(1)).unwrap();
    let y = pow2(-1002);
    assert_eq!(x.mul_f64(y).unwrap(), DD::ZERO);
    assert_eq!(x.mul(DD::from_f64(y).unwrap()).unwrap(), DD::ZERO);
    assert_eq!(x.negate().mul_f64(y).unwrap(), DD::ZERO);
    // One step less deep, the product 3·2^−1075 rounds to the nearest subnormal, 2^−1073 (the
    // tie 1.5·2^−1074 goes to the even 2·2^−1074).
    assert_eq!(
        x.mul_f64(0.5).unwrap(),
        DD::from_f64(f64::from_bits(2)).unwrap()
    );
}

/// Operands with a subnormal leading part: products and quotients that are exact doubles come out
/// exactly, and the root of 4·2^−1074 = 2^−1072 is 2^−536.
///
/// Aimed at `replace & with |` and `replace & with ^` in `exponent`'s subnormal branch
/// (`dd.rs:294`), which give such an operand a wrong exponent; these survive (the quality ledger
/// says why), and the test records the behaviour they cannot change.
#[test]
fn subnormal_operands_give_exact_results() {
    let tiny = |k: u64| f64::from_bits(k);
    let five = DD::from_f64(tiny(5)).unwrap();
    assert_eq!(five.mul_f64(pow2(1000)).unwrap().hi(), 5.0 * pow2(-74));
    assert_eq!(five.mul_f64(-3.0).unwrap().hi(), -tiny(15));
    assert_eq!(
        DD::from_f64(-tiny(5))
            .unwrap()
            .mul(DD::from_f64(pow2(60)).unwrap())
            .unwrap()
            .hi(),
        -5.0 * pow2(-1014)
    );
    assert_eq!(
        DD::from_f64(1.0)
            .unwrap()
            .div_f64(tiny(1 << 51))
            .unwrap()
            .hi(),
        pow2(1023)
    );
    assert_eq!(
        DD::from_f64(tiny(12))
            .unwrap()
            .div(DD::from_f64(tiny(3)).unwrap())
            .unwrap(),
        DD::from_f64(4.0).unwrap()
    );
    assert_eq!(
        DD::from_f64(tiny(4)).unwrap().sqrt().unwrap().hi(),
        pow2(-536)
    );
    assert_eq!(five.mul(DD::ZERO).unwrap(), DD::ZERO);
    assert_eq!(DD::ZERO.mul_f64(tiny(5)).unwrap(), DD::ZERO);
}

/// Hashing feeds the value: distinct values (leading parts alike, trailing parts different, and
/// the other way) hash apart, and equal values alike.
///
/// Kills `replace <impl Hash for DD>::hash with ()` (`dd.rs:1023`), under which every value hashes
/// the same.
#[test]
fn distinct_values_hash_apart() {
    let hash = |x: DD| {
        let mut h = DefaultHasher::new();
        x.hash(&mut h);
        h.finish()
    };
    let mut values = Vec::new();
    for i in 1..=16 {
        let hi = f64::from(i);
        values.push(DD::from_f64(hi).unwrap());
        values.push(DD::from_sum(hi, pow2(-60)).unwrap());
        values.push(DD::from_sum(hi, -pow2(-61)).unwrap());
    }
    let hashes: BTreeSet<u64> = values.iter().map(|&x| hash(x)).collect();
    assert_eq!(hashes.len(), values.len());
    let one = DD::from_sum(0.5, 0.5).unwrap();
    assert_eq!(hash(one), hash(DD::ONE));
}
