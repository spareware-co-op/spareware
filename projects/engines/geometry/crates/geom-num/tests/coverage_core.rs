//! Tests that run, through the crate's public interface, the lines no other integration test
//! reached (piece G21, the coverage gate of section 28.8).
//!
//! - **`grid`.** `P2::new` refuses a coordinate outside the frame's range, as `P3::new` does; the
//!   integration tests made only points inside it, so its refusing arm never ran.
//! - **`generated::plane_side`.** The generated filter and exact path of the side of a plane,
//!   which `Plane::side` does not call yet (its module header), so nothing ran them: here the exact
//!   value against a `num-bigint` sum, at every sign combination of the extremes of the declared
//!   ranges, on random planes and points at every scale and on points placed exactly on a plane
//!   and one unit off it, where the filter must answer the exact sign or nothing; `None` for an
//!   input one past its range; `Some(Sign::Zero)` at once for a zero scale.
//! - **`DD`.** `DD::from_real`, which nothing called; `from_sum` and `exact_product` refusing a
//!   non-finite second operand (only a non-finite first one was tried); and the zero shortcuts of
//!   `mul_f64`, `mul` and `div_f64` with a zero `DD`.
//! - **`math::sinpi` and `math::cospi`.** The accurate path's hard cases are tested at their own
//!   inputs, never at their mirror images (−a, and 1 − a, ½ − a and ½ + a where those are doubles),
//!   which the path recognises by the second half of its test.
//!
//! Seeds: the randomised test prints `seed: <hex>` and runs from a fixed default;
//! `SPAREKIT_SEED=<hex>` replays another (chapter 28.17).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a failing test should stop loudly"
)]

use geom_num::generated::plane_side;
use geom_num::grid::COORD_MAX;
use geom_num::math;
use geom_num::{DD, NumError, P2, P3, Real, Sign};
use num_bigint::BigInt;

/// `P2::new` reports the first coordinate outside ±(2^40 − 1), x before y, with its value.
#[test]
fn a_2d_point_outside_the_frame_is_refused_naming_the_first_bad_coordinate() {
    let over = COORD_MAX + 1;
    let under = -COORD_MAX - 1;
    let out = |v: i64| {
        Err(NumError::OutOfRange {
            value: i128::from(v),
        })
    };
    assert_eq!(P2::new(over, 0), out(over));
    assert_eq!(P2::new(0, under), out(under));
    assert_eq!(P2::new(under, over), out(under));
    assert_eq!(P2::new(i64::MAX, i64::MIN), out(i64::MAX));
    let edge = P2::new(COORD_MAX, -COORD_MAX);
    assert_eq!(
        edge.map(|p| (p.x.get(), p.y.get())),
        Ok((COORD_MAX, -COORD_MAX))
    );
}

// ---------------------------------------------------------------------------------------------
// generated::plane_side
// ---------------------------------------------------------------------------------------------

/// The largest normal component the description declares: `|n_i| ≤ 2^83 − 1`.
const N_MAX: i128 = (1 << 83) - 1;

/// The largest offset the description declares: `|d| ≤ 2^125 − 1`.
const D_MAX: i128 = (1 << 125) - 1;

/// The default seed of the randomised test.
const DEFAULT_SEED: u64 = 0x6721_0000_5eed_0001;

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

    /// A value of magnitude at most `max`, at a random scale: within 2^k of zero for a random k up
    /// to `max`'s bit length, or one of `±max` exactly.
    fn scaled(&mut self, max: i128) -> i128 {
        let bits = 128 - max.leading_zeros();
        match self.next() % 8 {
            0 => max,
            1 => -max,
            _ => {
                let k = (self.next() % u64::from(bits + 1)) as u32;
                let wide = (u128::from(self.next()) << 64) | u128::from(self.next());
                let magnitude = if k >= 127 {
                    wide >> 1
                } else {
                    wide & ((1u128 << k) - 1)
                };
                let v = (magnitude as i128).min(max);
                if self.next() & 1 == 0 { v } else { -v }
            }
        }
    }
}

/// n · p + d exactly.
fn oracle(n: [i128; 3], d: i128, p: [i64; 3]) -> BigInt {
    n.iter()
        .zip(p)
        .map(|(&ni, pi)| BigInt::from(ni) * BigInt::from(pi))
        .sum::<BigInt>()
        + BigInt::from(d)
}

fn sign_of(v: &BigInt) -> Sign {
    match v.sign() {
        num_bigint::Sign::Minus => Sign::Negative,
        num_bigint::Sign::NoSign => Sign::Zero,
        num_bigint::Sign::Plus => Sign::Positive,
    }
}

fn point(p: [i64; 3]) -> P3 {
    P3::new(p[0], p[1], p[2]).unwrap()
}

/// Checks `value`, `exact` and `filter` at one input of the domain; returns whether the filter
/// answered.
fn check_plane_side(n: [i128; 3], d: i128, p: [i64; 3]) -> bool {
    let want = oracle(n, d, p);
    let [nx, ny, nz] = n;
    let at = point(p);
    let value = plane_side::value(nx, ny, nz, d, at)
        .unwrap_or_else(|| panic!("value is None inside the domain: n {n:?}, d {d}, p {p:?}"));
    assert_eq!(BigInt::from(value), want, "n {n:?}, d {d}, p {p:?}");
    let sign = sign_of(&want);
    assert_eq!(plane_side::exact(nx, ny, nz, d, at), Some(sign));
    match plane_side::filter(nx, ny, nz, d, at) {
        Some(s) => {
            assert_eq!(
                s, sign,
                "the filter's sign is wrong: n {n:?}, d {d}, p {p:?}"
            );
            true
        }
        None => false,
    }
}

/// At every sign combination of ±(2^83 − 1) for each normal component, ±(2^125 − 1) for the
/// offset and ±(2^40 − 1) for each coordinate, the largest values the domain holds, the exact
/// path is exact and never `None`.
#[test]
fn plane_side_is_exact_at_every_sign_combination_of_the_extremes() {
    for mask in 0u32..128 {
        let bit = |i: u32| if mask >> i & 1 == 0 { 1i8 } else { -1 };
        let n = [
            N_MAX * i128::from(bit(0)),
            N_MAX * i128::from(bit(1)),
            N_MAX * i128::from(bit(2)),
        ];
        let d = D_MAX * i128::from(bit(3));
        let p = [
            COORD_MAX * i64::from(bit(4)),
            COORD_MAX * i64::from(bit(5)),
            COORD_MAX * i64::from(bit(6)),
        ];
        check_plane_side(n, d, p);
    }
}

/// On random planes and points at every scale the exact path is exact and the filter is right
/// whenever it answers; on a point exactly on a random plane, and one unit of the offset to
/// either side, the filter answers the exact sign or nothing, and it both answers and declines
/// over the run, so its bound is reached from both sides.
#[test]
fn plane_side_filter_is_never_wrong_on_random_and_near_degenerate_inputs() {
    let mut rng = Rng::for_test("plane_side");
    let coord = |rng: &mut Rng| rng.scaled(i128::from(COORD_MAX)) as i64;
    let (mut answered, mut declined) = (0u32, 0u32);
    for _ in 0..20_000 {
        let n = [rng.scaled(N_MAX), rng.scaled(N_MAX), rng.scaled(N_MAX)];
        let p = [coord(&mut rng), coord(&mut rng), coord(&mut rng)];
        check_plane_side(n, rng.scaled(D_MAX), p);
        // n · p is below 3 · 2^123 in magnitude, so −n · p ± 1 is inside the offset's range.
        let on = -oracle(n, 0, p);
        let on = i128::try_from(on).unwrap();
        for delta in [-1, 0, 1] {
            if check_plane_side(n, on + delta, p) {
                answered += 1;
            } else {
                declined += 1;
            }
        }
    }
    assert!(
        answered > 0 && declined > 0,
        "{answered} answered, {declined} declined"
    );
}

/// One past its declared range, an input makes every path refuse (`None`), never a value
/// computed from an input the certificate does not cover; at the range's edge they all answer.
#[test]
fn plane_side_refuses_an_input_one_past_its_range() {
    let p = [7, -3, 11];
    let at = point(p);
    let past_n = N_MAX + 1;
    let past_d = D_MAX + 1;
    for (n, d) in [
        ([past_n, 0, 0], 0),
        ([0, -past_n, 0], 0),
        ([0, 0, past_n], 0),
        ([1, 1, 1], past_d),
        ([1, 1, 1], -past_d),
        ([i128::MIN, 0, 0], 0),
        ([0, 0, 0], i128::MAX),
    ] {
        let [nx, ny, nz] = n;
        assert_eq!(plane_side::value(nx, ny, nz, d, at), None, "n {n:?}, d {d}");
        assert_eq!(plane_side::exact(nx, ny, nz, d, at), None, "n {n:?}, d {d}");
        assert_eq!(
            plane_side::filter(nx, ny, nz, d, at),
            None,
            "n {n:?}, d {d}"
        );
    }
    check_plane_side([N_MAX, -N_MAX, N_MAX], -D_MAX, p);
}

/// A zero scale (the point at the origin and a zero offset, or a zero normal and offset) makes
/// the value zero, and the filter says so at once.
#[test]
fn plane_side_answers_zero_at_once_for_a_zero_scale() {
    let origin = point([0, 0, 0]);
    assert_eq!(plane_side::filter(5, -9, 3, 0, origin), Some(Sign::Zero));
    let p = point([COORD_MAX, -1, 2]);
    assert_eq!(plane_side::filter(0, 0, 0, 0, p), Some(Sign::Zero));
    assert_eq!(plane_side::exact(0, 0, 0, 0, p), Some(Sign::Zero));
    assert_eq!(plane_side::value(5, -9, 3, 0, origin), Some(0));
}

// ---------------------------------------------------------------------------------------------
// DD
// ---------------------------------------------------------------------------------------------

/// The bits of a `DD`'s two parts, so that a zero's sign is compared too.
fn bits(x: DD) -> (u64, u64) {
    (x.hi().to_bits(), x.lo().to_bits())
}

/// `DD::from_real` holds the `Real` exactly, as `from_f64` does, with a zero part +0: at both
/// zeros, the extremes of the range and a subnormal.
#[test]
fn dd_from_a_real_is_the_real_exactly() {
    for v in [
        0.0,
        -0.0,
        1.5,
        -3.25,
        f64::MAX,
        f64::MIN,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        -f64::from_bits(0x000f_ffff_ffff_ffff),
    ] {
        let d = DD::from_real(Real::new(v).unwrap());
        let plus = if v == 0.0 { 0.0 } else { v };
        assert_eq!(bits(d), (plus.to_bits(), 0), "{v:e}");
        assert_eq!(bits(d), bits(DD::from_f64(v).unwrap()), "{v:e}");
    }
}

/// `from_sum` and `exact_product` refuse a NaN or infinite operand in either place.
#[test]
fn dd_constructors_refuse_a_non_finite_operand_in_either_place() {
    for bad in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        assert_eq!(DD::from_sum(1.0, bad), Err(NumError::NotFinite), "{bad}");
        assert_eq!(DD::from_sum(bad, 1.0), Err(NumError::NotFinite), "{bad}");
        assert!(DD::exact_product(3.0, bad).is_none(), "{bad}");
        assert!(DD::exact_product(bad, 3.0).is_none(), "{bad}");
    }
}

/// A zero `DD` times anything finite, anything times a zero, and a zero `DD` divided by a
/// non-zero double are +0 exactly; dividing by zero is refused before the zero shortcut.
#[test]
fn dd_products_and_quotients_with_a_zero_are_plus_zero() {
    let x = DD::from_sum(3.0, libm::scalbn(1.0, -60)).unwrap();
    let zero = (0u64, 0u64);
    for y in [3.0, -3.0, f64::MAX, f64::from_bits(1)] {
        assert_eq!(DD::ZERO.mul_f64(y).map(bits), Ok(zero), "{y:e}");
        assert_eq!(DD::ZERO.div_f64(y).map(bits), Ok(zero), "{y:e}");
    }
    assert_eq!(x.mul_f64(0.0).map(bits), Ok(zero));
    assert_eq!(x.mul_f64(-0.0).map(bits), Ok(zero));
    assert_eq!(DD::ZERO.mul(x).map(bits), Ok(zero));
    assert_eq!(x.mul(DD::ZERO).map(bits), Ok(zero));
    assert_eq!(x.negate().mul(DD::ZERO).map(bits), Ok(zero));
    assert_eq!(DD::ZERO.div_f64(0.0), Err(NumError::NotFinite));
    assert_eq!(DD::ZERO.div_f64(-0.0), Err(NumError::NotFinite));
}

// ---------------------------------------------------------------------------------------------
// math::sinpi and math::cospi
// ---------------------------------------------------------------------------------------------

/// Each input whose accurate path ends at one of sinpi's listed hard cases (`SINPI_HARD` in
/// src/math/sincospi.rs, from CORE-MATH's `as_sinpi_refine`): a = (iq + x/2)/2^11 for its entry
/// (iq, x, r), exactly a double, with the correctly rounded sin(πa), the entry's r, as bits.
const SINPI_HARD_CASES: [(u64, u64); 4] = [
    (0x3fa2_f6c4_a600_85cd, 0x3fbd_b8a7_9a80_c3a0), // iq 76
    (0x3faa_f661_a8af_86da, 0x3fc5_146c_0bc4_5bce), // iq 108
    (0x3fce_99d7_85d7_05f5, 0x3fe5_d656_1936_b699), // iq 490
    (0x3fdc_3642_2fd2_e52a, 0x3fef_72c9_0696_2631), // iq 903, also in cospi's list
];

/// The mirror images of the hard cases answer as the identities say: sinpi(−a) = −sinpi(a) for
/// every one (its reduction gives iq = 2^12 − iq_a and z = −z_a), and for the entry at 903, whose
/// mirrors are doubles too, sinpi(1 − a) = sinpi(a), cospi(½ − a) = cospi(a − ½) = sinpi(a) and
/// cospi(½ + a) = −sinpi(a). The accurate path recognises a mirror image by the second half of its
/// hard-case test (x = −h.x and iq = 2048 − h.iq).
#[test]
fn sinpi_and_cospi_at_the_mirror_images_of_the_hard_cases() {
    let bits_of = |v: Result<Real, NumError>| v.unwrap().get().to_bits();
    for (a, r) in SINPI_HARD_CASES {
        let (a, r) = (f64::from_bits(a), f64::from_bits(r));
        assert_eq!(bits_of(math::sinpi(a)), r.to_bits(), "sinpi({a:e})");
        assert_eq!(bits_of(math::sinpi(-a)), (-r).to_bits(), "sinpi(-{a:e})");
    }
    let (a, r) = SINPI_HARD_CASES[3];
    let (a, r) = (f64::from_bits(a), f64::from_bits(r));
    // Each mirror is exactly a double (Sterbenz's lemma for 1 − a and ½ − a; ½ + a has a's last
    // place), so these are the mirror images themselves.
    assert_eq!((1.0 - a).to_bits(), 0x3fe1_e4de_e816_8d6b);
    assert_eq!((0.5 - a).to_bits(), 0x3fae_4dee_8168_d6b0);
    assert_eq!((0.5 + a).to_bits(), 0x3fee_1b21_17e9_7295);
    assert_eq!(bits_of(math::sinpi(1.0 - a)), r.to_bits());
    assert_eq!(bits_of(math::cospi(0.5 - a)), r.to_bits());
    assert_eq!(bits_of(math::cospi(a - 0.5)), r.to_bits());
    assert_eq!(bits_of(math::cospi(0.5 + a)), (-r).to_bits());
}
