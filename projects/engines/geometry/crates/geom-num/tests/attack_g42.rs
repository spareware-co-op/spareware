//! Attacks on piece G42's variants of the generated exact paths (chapter 23.3): the plain-`i128`
//! variant (`geom_num::generated::plain`) and the width-aware limb variant
//! (`geom_num::generated::limbs`, on `geom_num::Wide32`). For every registered predicate, each
//! variant's `<predicate>_value` must return exactly what the shipped module's `value` returns,
//! `None` included.
//!
//! - On random inputs across the whole domain: grid coordinates and rounded inputs of every bit
//!   length, mixed with the extremes ±(2^40 − 1) and ±(2^bits − 1), zeros and small values, so
//!   degenerate inputs (zero results) occur too.
//! - At every sign combination of the grid's extremes, for every predicate whose inputs are all
//!   points.
//! - A rounded input one past its declared range: every variant refuses it.
//! - Avnaim et al.'s signs of orient2d and orient3d (`geom_num::avnaim`), the reference
//!   chapter 23.3 names, give the shipped exact signs on the same inputs.
//! - `Wide32`'s extremes, order and `Debug` form, worked by hand.
//! - `avnaim` against exact determinants: every 2 × 2 with entries in −7..=7 and every 3 × 3
//!   with entries in −2..=2, entries at and one past its bounds, and random entries near them.
//! - `Wide32` against `i128` at every limb boundary and on random values: conversions at each
//!   width, limbs, sign and zero tests, negation, order, sums, differences and products.
//!
//! The shipped paths are tested against big integers in `generated_predicates.rs` and
//! `attack_g29.rs`, so agreement with them is agreement with the exact value. Values of different
//! types (the plain variant holds an `i64` result in an `i128`) are compared as `I512`.
//!
//! Seeds: each randomised test prints `seed: <hex>` and runs from a fixed default;
//! `SPAREKIT_SEED=<hex>` replays another (chapter 28.17).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    reason = "generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::similar_names
)]

use geom_num::avnaim;
use geom_num::generated::{
    incircle, insphere, limbs, lpi_side, lpi_weight, orient2d, orient3d, plain, plane_side,
    ssi_side, ssi_weight, tpi_side, tpi_weight,
};
use geom_num::grid::{COORD_MAX, P2, P3};
use geom_num::{I512, Sign, Wide, Wide32};
use geom_test::rng::SplitMix64;
use geom_test::seed::{self, Seed};
use std::cmp::Ordering;

/// Random cases per predicate.
const CASES: usize = 20_000;

/// The default seed of the randomised tests here.
const SEED: Seed = Seed(0x0642_0000_0000_0001);

/// A value of any exact-path result type, as an `I512`, which holds them all.
trait Exact {
    fn i512(self) -> I512;
}

impl Exact for i64 {
    fn i512(self) -> I512 {
        I512::from_i64(self)
    }
}

impl Exact for i128 {
    fn i512(self) -> I512 {
        I512::from_i128(self)
    }
}

impl<const L: usize> Exact for Wide<L> {
    fn i512(self) -> I512 {
        self.resize::<8>()
            .expect("no exact path is wider than I512")
    }
}

/// Two results, compared as `I512`s. Every input drawn here is in the domain, so the shipped path
/// must answer: a `None` from it would make the comparison empty.
fn same<A: Exact, B: Exact>(shipped: Option<A>, variant: Option<B>) -> bool {
    assert!(
        shipped.is_some(),
        "the shipped path refused an input in its domain"
    );
    shipped.map(Exact::i512) == variant.map(Exact::i512)
}

/// The shipped `value` of module `$m` and both variants' `$v`, on the same arguments, agree.
macro_rules! agree {
    ($m:ident, $v:ident; $($arg:expr),+ $(,)?) => {{
        let shipped = $m::value($($arg),+);
        assert!(same(shipped, plain::$v($($arg),+)), "plain {}", stringify!($v));
        assert!(same(shipped, limbs::$v($($arg),+)), "limbs {}", stringify!($v));
    }};
}

/// Draws inputs: a value of a random bit length and sign, an extreme, zero or a small value.
struct Draw(SplitMix64);

impl Draw {
    fn new(label: &str) -> Draw {
        Draw(SplitMix64::new(seed::resolve(label, SEED).unwrap()))
    }

    /// A value with |v| ≤ 2^bits − 1.
    fn value(&mut self, bits: u32) -> i128 {
        let max = (1i128 << bits) - 1;
        let pick = self.0.next_u64() % 16;
        let sign = if self.0.coin() { 1 } else { -1 };
        match pick {
            0..=3 => sign * max,
            4 | 5 => 0,
            6 | 7 => sign * i128::from(self.0.next_u64() % 4),
            _ => {
                let raw = (u128::from(self.0.next_u64()) << 64 | u128::from(self.0.next_u64()))
                    >> (128 - bits);
                let keep = self.0.next_u64() % u64::from(bits + 1);
                sign * (raw >> (u64::from(bits) - keep)) as i128
            }
        }
    }

    fn coord(&mut self) -> i64 {
        let v = self.value(40);
        assert!(v.unsigned_abs() <= COORD_MAX as u128);
        v as i64
    }

    fn p2(&mut self) -> P2 {
        P2::new(self.coord(), self.coord()).unwrap()
    }

    fn p3(&mut self) -> P3 {
        P3::new(self.coord(), self.coord(), self.coord()).unwrap()
    }

    /// A plane normal's coordinate: 83 bits.
    fn n(&mut self) -> i128 {
        self.value(83)
    }

    /// A plane offset: 125 bits.
    fn d(&mut self) -> i128 {
        self.value(125)
    }
}

#[test]
fn explicit_point_predicates_agree_on_random_inputs() {
    let mut g = Draw::new("attack_g42::explicit_point_predicates_agree_on_random_inputs");
    for _ in 0..CASES {
        let (a, b, c, d, e, f) = (g.p2(), g.p2(), g.p2(), g.p2(), g.p2(), g.p2());
        agree!(orient2d, orient2d_value; a, b, c);
        assert_eq!(avnaim::orient2d(a, b, c), orient2d::exact(a, b, c));
        agree!(incircle, incircle_value; a, b, c, d);
        agree!(ssi_weight, ssi_weight_value; a, b, c, d);
        agree!(ssi_side, ssi_side_value; a, b, c, d, e, f);
        let (a, b, c, d, e) = (g.p3(), g.p3(), g.p3(), g.p3(), g.p3());
        agree!(orient3d, orient3d_value; a, b, c, d);
        assert_eq!(avnaim::orient3d(a, b, c, d), orient3d::exact(a, b, c, d));
        agree!(insphere, insphere_value; a, b, c, d, e);
    }
}

#[test]
fn plane_predicates_agree_on_random_inputs() {
    let mut g = Draw::new("attack_g42::plane_predicates_agree_on_random_inputs");
    for _ in 0..CASES {
        let (p, q) = (g.p3(), g.p3());
        let [nx, ny, nz] = [g.n(), g.n(), g.n()];
        let d = g.d();
        agree!(plane_side, plane_side_value; nx, ny, nz, d, p);
        agree!(lpi_weight, lpi_weight_value; nx, ny, nz, p, q);
        let n: [i128; 12] = core::array::from_fn(|_| g.n());
        let o: [i128; 4] = core::array::from_fn(|_| g.d());
        agree!(tpi_weight, tpi_weight_value; n[0], n[1], n[2], n[3], n[4], n[5], n[6], n[7], n[8]);
        agree!(
            tpi_side, tpi_side_value;
            n[0], n[1], n[2], o[0], n[3], n[4], n[5], o[1], n[6], n[7], n[8], o[2], n[9], n[10],
            n[11], o[3],
        );
        agree!(lpi_side, lpi_side_value; n[0], n[1], n[2], o[0], n[3], n[4], n[5], o[1], p, q);
    }
}

/// Every sign combination of ±(2^40 − 1) over `count` coordinates.
fn extremes(count: u32) -> impl Iterator<Item = Vec<i64>> {
    (0u32..1 << count).map(move |bits| {
        (0..count)
            .map(|i| {
                if bits >> i & 1 == 1 {
                    -COORD_MAX
                } else {
                    COORD_MAX
                }
            })
            .collect()
    })
}

fn p2s<const N: usize>(c: &[i64]) -> [P2; N] {
    core::array::from_fn(|i| P2::new(c[2 * i], c[2 * i + 1]).unwrap())
}

fn p3s<const N: usize>(c: &[i64]) -> [P3; N] {
    core::array::from_fn(|i| P3::new(c[3 * i], c[3 * i + 1], c[3 * i + 2]).unwrap())
}

#[test]
fn explicit_point_predicates_agree_at_every_extreme() {
    for c in extremes(6) {
        let [a, b, e] = p2s(&c);
        agree!(orient2d, orient2d_value; a, b, e);
        assert_eq!(avnaim::orient2d(a, b, e), orient2d::exact(a, b, e));
    }
    for c in extremes(8) {
        let [a, b, e, f] = p2s(&c);
        agree!(incircle, incircle_value; a, b, e, f);
        agree!(ssi_weight, ssi_weight_value; a, b, e, f);
    }
    for c in extremes(12) {
        let [a, b, e, f, g, h] = p2s(&c);
        agree!(ssi_side, ssi_side_value; a, b, e, f, g, h);
        let [a, b, e, f] = p3s(&c);
        agree!(orient3d, orient3d_value; a, b, e, f);
        assert_eq!(avnaim::orient3d(a, b, e, f), orient3d::exact(a, b, e, f));
    }
    for c in extremes(15) {
        let [a, b, e, f, g] = p3s(&c);
        agree!(insphere, insphere_value; a, b, e, f, g);
    }
}

#[test]
fn a_rounded_input_past_its_range_is_refused_by_every_variant() {
    let p = P3::new(1, 2, 3).unwrap();
    let (n, d) = ((1i128 << 83) - 1, (1i128 << 125) - 1);
    assert!(plane_side::value(n, n, n, d, p).is_some());
    assert!(plain::plane_side_value(n, n, n, d, p).is_some());
    assert!(limbs::plane_side_value(n, n, n, d, p).is_some());
    for (nx, d) in [(n + 1, d), (-n - 1, d), (n, d + 1), (n, -d - 1)] {
        assert_eq!(plane_side::value(nx, n, n, d, p), None);
        assert_eq!(plain::plane_side_value(nx, n, n, d, p), None);
        assert_eq!(limbs::plane_side_value(nx, n, n, d, p), None);
    }
    assert_eq!(lpi_weight::value(n + 1, n, n, p, p), None);
    assert_eq!(plain::lpi_weight_value(n + 1, n, n, p, p), None);
    assert_eq!(limbs::lpi_weight_value(n + 1, n, n, p, p), None);
}

/// `Wide32`'s extremes, its order and its `Debug` form (the two's-complement bits, most significant
/// limb first), worked by hand at 128 and 64 bits.
#[test]
fn wide32_extremes_order_and_debug_by_hand() {
    let (max, min) = (Wide32::<4>::max_value(), Wide32::<4>::min_value());
    assert_eq!(max.to_i128(), Some(i128::MAX));
    assert_eq!(min.to_i128(), Some(i128::MIN));
    assert_eq!(
        Wide32::<2>::max_value().to_i128(),
        Some(i128::from(i64::MAX))
    );
    assert_eq!(
        Wide32::<2>::min_value().to_i128(),
        Some(i128::from(i64::MIN))
    );
    let zero = Wide32::<4>::ZERO;
    assert!(min < zero && zero < max && min < max);
    assert_eq!(max.partial_cmp(&min), Some(Ordering::Greater));
    assert_eq!(zero.partial_cmp(&zero), Some(Ordering::Equal));
    assert_eq!(
        format!("{:?}", Wide32::<4>::from_i64(-2)),
        "Wide32<4>(0xfffffffffffffffffffffffffffffffe)"
    );
    assert_eq!(
        format!("{:?}", Wide32::<2>::from_i64(1)),
        "Wide32<2>(0x0000000000000001)"
    );
}

/// The sign of an `i128`.
fn sign_of(v: i128) -> Sign {
    match v.cmp(&0) {
        Ordering::Less => Sign::Negative,
        Ordering::Equal => Sign::Zero,
        Ordering::Greater => Sign::Positive,
    }
}

/// The exact sign of x1·y2 − y1·x2: entries of at most 2^62 make products below 2^124 and their
/// difference below 2^125, so `i128` holds them.
fn det2_sign(x1: i64, y1: i64, x2: i64, y2: i64) -> Sign {
    let p = |a: i64, b: i64| i128::from(a) * i128::from(b);
    sign_of(p(x1, y2) - p(y1, x2))
}

/// The exact sign of the 3 × 3 determinant with rows `u1`, `u2`, `u3`, in `I512`, which holds
/// every product of three `i64` entries.
fn det3_sign(u1: [i64; 3], u2: [i64; 3], u3: [i64; 3]) -> Sign {
    let w = I512::from_i64;
    let mul = |a: I512, b: I512| a.checked_mul(&b).unwrap();
    let minor = |a: [i64; 3], b: [i64; 3], i: usize, j: usize| {
        mul(w(a[i]), w(b[j]))
            .checked_sub(&mul(w(a[j]), w(b[i])))
            .unwrap()
    };
    let t0 = mul(w(u1[0]), minor(u2, u3, 1, 2));
    let t1 = mul(w(u1[1]), minor(u2, u3, 0, 2));
    let t2 = mul(w(u1[2]), minor(u2, u3, 0, 1));
    t0.checked_sub(&t1)
        .unwrap()
        .checked_add(&t2)
        .unwrap()
        .signum()
}

/// `avnaim`'s signs against exact determinants on every 2 × 2 with entries in −7..=7 and every
/// 3 × 3 with entries in −2..=2: every equality its tests separate occurs there (equal entries,
/// equal or opposite rows, a zero row or column, collinear first rows, a halving landing exactly
/// on a half).
#[test]
fn avnaim_signs_are_exact_on_every_small_matrix() {
    for x1 in -7..=7 {
        for y1 in -7..=7 {
            for x2 in -7..=7 {
                for y2 in -7..=7 {
                    assert_eq!(
                        avnaim::sign_of_det2(x1, y1, x2, y2),
                        Some(det2_sign(x1, y1, x2, y2)),
                        "|{x1} {y1}; {x2} {y2}|"
                    );
                }
            }
        }
    }
    let entries = [-2i64, -1, 0, 1, 2];
    for i in 0..5usize.pow(9) {
        let mut k = i;
        let mut m = [0i64; 9];
        for v in &mut m {
            *v = entries[k % 5];
            k /= 5;
        }
        let (u1, u2, u3) = ([m[0], m[1], m[2]], [m[3], m[4], m[5]], [m[6], m[7], m[8]]);
        let small = |a: [i64; 3], b: [i64; 3], i: usize, j: usize| {
            i128::from(a[i]) * i128::from(b[j]) - i128::from(a[j]) * i128::from(b[i])
        };
        let d = i128::from(u1[0]) * small(u2, u3, 1, 2) - i128::from(u1[1]) * small(u2, u3, 0, 2)
            + i128::from(u1[2]) * small(u2, u3, 0, 1);
        assert_eq!(avnaim::sign_of_det3(u1, u2, u3), Some(sign_of(d)), "{m:?}");
    }
}

/// `avnaim` takes entries up to its bounds, in every position and with either sign, and refuses
/// one past them; at and near the bounds its signs are exact.
#[test]
fn avnaim_entries_at_and_past_their_bounds() {
    // The bounds the module documents, 2^62 − 1 and 2^59 − 1, as numbers.
    assert!(avnaim::sign_of_det2((1 << 62) - 1, 1, 1, 1).is_some());
    assert_eq!(avnaim::sign_of_det2(1 << 62, 1, 1, 1), None);
    assert!(avnaim::sign_of_det3([(1 << 59) - 1, 1, 1], [1, 2, 3], [4, 5, 7]).is_some());
    assert_eq!(
        avnaim::sign_of_det3([1 << 59, 1, 1], [1, 2, 3], [4, 5, 7]),
        None
    );
    let m = avnaim::ENTRY_MAX;
    for pos in 0..4 {
        for s in [1i64, -1] {
            let mut at = [3i64, 5, 7, 11];
            at[pos] = s * m;
            let [x1, y1, x2, y2] = at;
            assert_eq!(
                avnaim::sign_of_det2(x1, y1, x2, y2),
                Some(det2_sign(x1, y1, x2, y2))
            );
            let mut past = at;
            past[pos] = s * (m + 1);
            let [x1, y1, x2, y2] = past;
            assert_eq!(avnaim::sign_of_det2(x1, y1, x2, y2), None, "{past:?}");
        }
    }
    let big = [
        0,
        1,
        -1,
        2,
        m,
        -m,
        m - 1,
        -(m - 1),
        m / 2,
        -(m / 2),
        m / 2 + 1,
        -(m / 2 + 1),
    ];
    for &x1 in &big {
        for &y1 in &big {
            for &x2 in &big {
                for &y2 in &big {
                    assert_eq!(
                        avnaim::sign_of_det2(x1, y1, x2, y2),
                        Some(det2_sign(x1, y1, x2, y2)),
                        "|{x1} {y1}; {x2} {y2}|"
                    );
                }
            }
        }
    }
    let d = avnaim::DET3_ENTRY_MAX;
    for pos in 0..9 {
        for s in [1i64, -1] {
            let mut at = [2i64, 3, 5, 7, 11, 13, 17, 19, 23];
            at[pos] = s * d;
            let rows = |e: [i64; 9]| ([e[0], e[1], e[2]], [e[3], e[4], e[5]], [e[6], e[7], e[8]]);
            let (u1, u2, u3) = rows(at);
            assert_eq!(
                avnaim::sign_of_det3(u1, u2, u3),
                Some(det3_sign(u1, u2, u3))
            );
            let mut past = at;
            past[pos] = s * (d + 1);
            let (u1, u2, u3) = rows(past);
            assert_eq!(avnaim::sign_of_det3(u1, u2, u3), None, "{past:?}");
        }
    }
    let mut draw = Draw::new("avnaim_entries_at_and_past_their_bounds");
    let near = [0, 1, -1, d, -d, d - 1, -(d - 1), d / 2, -(d / 2)];
    for _ in 0..300 {
        let mut e = [0i64; 9];
        for v in &mut e {
            *v = if draw.0.coin() {
                near[usize::try_from(draw.0.next_u64() % 9).unwrap()]
            } else {
                i64::try_from(draw.value(59)).unwrap()
            };
        }
        let (u1, u2, u3) = ([e[0], e[1], e[2]], [e[3], e[4], e[5]], [e[6], e[7], e[8]]);
        assert_eq!(
            avnaim::sign_of_det3(u1, u2, u3),
            Some(det3_sign(u1, u2, u3)),
            "{e:?}"
        );
    }
}

/// Values at and beside every 32-bit limb boundary of 128 bits, both signs, and the extremes.
fn wide32_values() -> Vec<i128> {
    let mut v = vec![
        0,
        1,
        -1,
        2,
        -2,
        i128::MAX,
        i128::MIN,
        i128::MAX - 1,
        i128::MIN + 1,
    ];
    for bit in [31, 32, 33, 63, 64, 65, 95, 96, 97, 126] {
        let p = 1i128 << bit;
        v.extend([p, -p, p - 1, -(p - 1), p + 1, -(p + 1)]);
    }
    v
}

/// Whether `v` fits a signed integer of `bits` bits.
fn fits(v: i128, bits: u32) -> bool {
    bits >= 128 || (-(1i128 << (bits - 1)) <= v && v < 1i128 << (bits - 1))
}

/// `Wide32` against `i128`: every conversion, its limbs, sign and zero tests, negation, order, sum,
/// difference and products, at each width that holds a value or must refuse it, on the limb
/// boundaries and on random values.
#[test]
fn wide32_matches_i128() {
    let edges = wide32_values();
    let mut draw = Draw::new("wide32_matches_i128");
    let mut values = edges.clone();
    values.extend((0..300).map(|_| draw.value(126)));
    let back = |w: Wide32<4>| w.to_i128().unwrap();
    for &v in &values {
        let w = Wide32::<4>::from_i128(v).unwrap();
        assert_eq!(w.to_i128(), Some(v));
        let words: [u32; 4] = core::array::from_fn(|k| (v >> (32 * k)) as u32);
        assert_eq!(w.to_le_limbs(), words);
        assert_eq!(Wide32::<4>::from_le_limbs(words), w);
        assert_eq!(w.is_zero(), v == 0);
        assert_eq!(w.is_negative(), v < 0);
        assert_eq!(w.signum(), sign_of(v));
        assert_eq!(w.checked_neg().map(back), v.checked_neg());
        for (bits, narrow) in [
            (64, Wide32::<2>::from_i128(v).and_then(|n| n.to_i128())),
            (96, Wide32::<3>::from_i128(v).and_then(|n| n.to_i128())),
            (64, w.resize::<2>().and_then(|n| n.to_i128())),
            (96, w.resize::<3>().and_then(|n| n.to_i128())),
        ] {
            assert_eq!(narrow, fits(v, bits).then_some(v), "{v} in {bits} bits");
        }
        let six = w.resize::<6>().unwrap();
        assert_eq!(six.to_i128(), Some(v));
        assert_eq!(six.resize::<4>(), Some(w));
        assert_eq!(six.to_wide::<2>(), Some(Wide::<2>::from_i128(v)));
        assert_eq!(w.to_wide::<2>(), Some(Wide::<2>::from_i128(v)));
        if let Ok(s) = i64::try_from(v) {
            assert_eq!(Wide32::<2>::from_i64(s).to_i128(), Some(v));
            assert_eq!(Wide32::<3>::from_i64(s), Wide32::<3>::from_i128(v).unwrap());
            assert_eq!(Wide32::<4>::from_i64(s), w);
        }
    }
    for &a in &edges {
        for &b in &values {
            let (wa, wb) = (
                Wide32::<4>::from_i128(a).unwrap(),
                Wide32::<4>::from_i128(b).unwrap(),
            );
            assert_eq!(wa.cmp(&wb), a.cmp(&b), "{a} against {b}");
            assert_eq!(wa.checked_add(&wb).map(back), a.checked_add(b), "{a} + {b}");
            assert_eq!(wa.checked_sub(&wb).map(back), a.checked_sub(b), "{a} - {b}");
            assert_eq!(
                Wide32::<4>::product(&wa, &wb).map(back),
                a.checked_mul(b),
                "{a} * {b}"
            );
            // Every product of two 128-bit values fits 256 bits: the same product in eight limbs,
            // against `Wide<4>`, and read back at 192 and 128 bits.
            let exact = Wide::<4>::from_i128(a)
                .checked_mul(&Wide::<4>::from_i128(b))
                .unwrap();
            let eight = Wide32::<8>::product(&wa, &wb).unwrap();
            assert_eq!(eight.to_wide::<4>(), Some(exact));
            assert_eq!(eight.to_i128(), a.checked_mul(b));
            assert_eq!(
                eight.to_wide::<2>(),
                a.checked_mul(b).map(Wide::<2>::from_i128)
            );
            assert_eq!(
                Wide32::<6>::product(&wa, &wb).and_then(|p| p.to_wide::<3>()),
                exact.resize::<3>(),
                "{a} * {b} in 192 bits"
            );
            if let Ok(n) = i64::try_from(b) {
                assert_eq!(
                    Wide32::<4>::product_by_i64(&wa, n).map(back),
                    a.checked_mul(b)
                );
            }
            if let (Ok(x), Ok(y)) = (i64::try_from(a), i64::try_from(b)) {
                let p = a * b;
                assert_eq!(Wide32::<4>::product_i64(x, y).map(back), Some(p));
                assert_eq!(
                    Wide32::<3>::product_i64(x, y).and_then(|n| n.to_i128()),
                    fits(p, 96).then_some(p)
                );
            }
        }
    }
}

/// The exact 3 × 3 determinant in `i128`, for entries below 2^40 (products below 2^120).
fn det3_small(u1: [i64; 3], u2: [i64; 3], u3: [i64; 3]) -> i128 {
    let w = i128::from;
    let minor =
        |a: [i64; 3], b: [i64; 3], i: usize, j: usize| w(a[i]) * w(b[j]) - w(a[j]) * w(b[i]);
    w(u1[0]) * minor(u2, u3, 1, 2) - w(u1[1]) * minor(u2, u3, 0, 2) + w(u1[2]) * minor(u2, u3, 0, 1)
}

/// `avnaim::sign_of_det3` on random rows of every size up to 2^20, and on rows near and on the
/// plane of the first two (u3 = a·u1 + b·u2 plus a small offset, or none), where the lattice
/// walk's and the second step's comparisons meet equality: a third row on a lattice point, at a
/// cell's corner or on its diagonals, and at the corners' heights.
#[test]
fn avnaim_det3_on_random_and_near_plane_rows() {
    let mut draw = Draw::new("avnaim_det3_on_random_and_near_plane_rows");
    let check = |u1: [i64; 3], u2: [i64; 3], u3: [i64; 3]| {
        assert_eq!(
            avnaim::sign_of_det3(u1, u2, u3),
            Some(sign_of(det3_small(u1, u2, u3))),
            "{u1:?} {u2:?} {u3:?}"
        );
    };
    for _ in 0..20_000 {
        let bits = 1 + u32::try_from(draw.0.next_u64() % 20).unwrap();
        let mut e = [0i64; 9];
        for v in &mut e {
            *v = i64::try_from(draw.value(bits)).unwrap();
        }
        check([e[0], e[1], e[2]], [e[3], e[4], e[5]], [e[6], e[7], e[8]]);
    }
    let small = |draw: &mut Draw, n: u64| {
        i64::try_from(draw.0.next_u64() % (2 * n + 1)).unwrap() - i64::try_from(n).unwrap()
    };
    for _ in 0..40_000 {
        let bits = 1 + u32::try_from(draw.0.next_u64() % 12).unwrap();
        let mut row = || {
            [
                i64::try_from(draw.value(bits)).unwrap(),
                i64::try_from(draw.value(bits)).unwrap(),
                i64::try_from(draw.value(bits)).unwrap(),
            ]
        };
        let (u1, u2) = (row(), row());
        let (a, b) = (small(&mut draw, 6), small(&mut draw, 6));
        let off = if draw.0.coin() {
            [0; 3]
        } else {
            [
                small(&mut draw, 2),
                small(&mut draw, 2),
                small(&mut draw, 2),
            ]
        };
        let u3: [i64; 3] = core::array::from_fn(|k| a * u1[k] + b * u2[k] + off[k]);
        check(u1, u2, u3);
    }
}
