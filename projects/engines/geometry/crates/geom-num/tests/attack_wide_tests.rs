//! Adversarial tests of `geom_num::wide`: they try to break the fixed-width integers, not confirm
//! them.
//!
//! What they attack, each against an exact oracle:
//! - A second, independent oracle. For `Wide<2>`, whose range is exactly `i128`'s, native `i128`
//!   checked arithmetic. For every width, a byte-level two's-complement model built only on
//!   `num-bigint`'s own `to_signed_bytes_le` and `from_signed_bytes_le` (no code shared with
//!   tests/wide.rs, whose oracle folds limbs by hand).
//! - Widths tests/wide.rs never instantiates: odd limb counts (`Wide<3>`, `Wide<7>`) and wide ones
//!   (`Wide<16>`), with `Wide<14>`, `Wide<17>` and `Wide<32>` as resize and double-width targets.
//! - Exhaustive sweeps of raw limb patterns (0, 1, 2, all ones, the sign bit and its neighbours),
//!   every pair, where carry, borrow and sign logic break.
//! - Laws that tie operations and widths together: x·2 = x + x and x·k = repeated addition, with
//!   overflow at the same place; order is translation invariant and scales with the sign; a sum
//!   fits one limb wider and a product fits twice as wide; every operation agrees with the same
//!   operation done wider and narrowed back.
//! - The `L >= 2` guard. `Wide<1>` and `Wide<0>` can still be built through `ZERO` and
//!   `from_le_limbs`, and every operation that compiles for them must still be right.
//! - The kernel's extreme inputs: products of two `i128` values in `I256` (what the predicate
//!   module relies on) and the in-sphere determinant at coordinates ±(2^40 − 1), every step
//!   checked.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them, and std's reductions, to build their oracles.
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
    clippy::many_single_char_names,
    reason = "points and values are named a, b, c and so on, as in the formulas they check"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::cmp::Ordering;

use geom_num::{I256, I320, I384, I512, Sign, Wide};
use num_bigint::{BigInt, Sign as BigSign};
use num_traits::{One, Signed, ToPrimitive, Zero};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestCaseError};

/// The outcome of one check: `Err` carries the first disagreement.
type TestResult = Result<(), TestCaseError>;

/// Cases per property.
const CASES: u32 = 512;

/// The grid's extreme coordinate, 2^40 − 1 grid steps.
const C: i64 = (1 << 40) - 1;

/// Every property runs with this configuration. Failures are not persisted to a regressions file:
/// this file is the only one the attack may write.
fn cfg() -> Config {
    Config {
        cases: CASES,
        failure_persistence: None,
        ..Config::default()
    }
}

/// Turns a check's failure into a test failure, for checks run outside `proptest!`.
fn run(result: TestResult) {
    if let Err(e) = result {
        panic!("{e}");
    }
}

// ---------------------------------------------------------------------------------------------
// The byte-level oracle.
// ---------------------------------------------------------------------------------------------

/// The value a `Wide` holds: its limbs as little-endian bytes, read by `num-bigint` as two's
/// complement.
fn val<const L: usize>(x: &Wide<L>) -> BigInt {
    let bytes: Vec<u8> = x
        .to_le_limbs()
        .iter()
        .flat_map(|limb| limb.to_le_bytes())
        .collect();
    BigInt::from_signed_bytes_le(&bytes)
}

/// Limbs read as an unsigned number.
fn unsigned(limbs: &[u64]) -> BigInt {
    let bytes: Vec<u8> = limbs.iter().flat_map(|limb| limb.to_le_bytes()).collect();
    BigInt::from_bytes_le(BigSign::Plus, &bytes)
}

/// The sign bit's position in a `Wide<l>`, l >= 1.
fn top(l: usize) -> usize {
    64 * l - 1
}

/// 2^n.
fn pow2(n: usize) -> BigInt {
    BigInt::one() << n
}

/// The largest value of a `Wide<l>`.
fn max_of(l: usize) -> BigInt {
    pow2(top(l)) - 1u32
}

/// The smallest value of a `Wide<l>`.
fn min_of(l: usize) -> BigInt {
    -pow2(top(l))
}

/// Whether `v` is representable in a `Wide<l>`.
fn fits(v: &BigInt, l: usize) -> bool {
    *v >= min_of(l) && *v <= max_of(l)
}

/// `v` moved to the nearest value a `Wide<l>` holds.
fn clamp(v: BigInt, l: usize) -> BigInt {
    v.clamp(min_of(l), max_of(l))
}

/// The `Wide<L>` holding `v` (from `num-bigint`'s minimal two's-complement bytes, sign-extended to
/// 8L bytes), or `None` when `v` is out of range: the expected result of every checked operation
/// whose true result is `v`.
fn mk<const L: usize>(v: &BigInt) -> Option<Wide<L>> {
    if !fits(v, L) {
        return None;
    }
    let mut bytes = v.to_signed_bytes_le();
    assert!(bytes.len() <= 8 * L, "{v} needs {} bytes", bytes.len());
    bytes.resize(8 * L, if v.is_negative() { 0xFF } else { 0 });
    let mut limbs = [0u64; L];
    for (limb, chunk) in limbs.iter_mut().zip(bytes.chunks_exact(8)) {
        *limb = u64::from_le_bytes(chunk.try_into().unwrap());
    }
    Some(Wide::from_le_limbs(limbs))
}

/// The `Wide<L>` holding `v`, which the caller guarantees is in range.
fn must<const L: usize>(v: &BigInt) -> Wide<L> {
    mk::<L>(v).unwrap_or_else(|| panic!("{v} does not fit Wide<{L}>"))
}

/// The sign of `v`.
fn sign_of(v: &BigInt) -> Sign {
    match v.cmp(&BigInt::zero()) {
        Ordering::Less => Sign::Negative,
        Ordering::Equal => Sign::Zero,
        Ordering::Greater => Sign::Positive,
    }
}

/// The sign an ordering implies for the difference.
fn ordering_sign(o: Ordering) -> Sign {
    match o {
        Ordering::Less => Sign::Negative,
        Ordering::Equal => Sign::Zero,
        Ordering::Greater => Sign::Positive,
    }
}

/// What `Debug` must print: the two's-complement bits, most significant first, 16 hex digits a
/// limb.
fn debug_of<const L: usize>(x: &Wide<L>) -> String {
    format!(
        "Wide<{L}>(0x{:0w$x})",
        unsigned(&x.to_le_limbs()),
        w = 16 * L
    )
}

/// Every combination of `set` in each of L limbs.
fn patterns<const L: usize>(set: &[u64]) -> Vec<Wide<L>> {
    let mut all = vec![[0u64; L]];
    for i in 0..L {
        all = all
            .into_iter()
            .flat_map(|base| {
                set.iter().map(move |&p| {
                    let mut limbs = base;
                    limbs[i] = p;
                    limbs
                })
            })
            .collect();
    }
    all.into_iter().map(Wide::from_le_limbs).collect()
}

/// The limb values carry, borrow and sign logic trip on.
const LIMB_EDGES: [u64; 9] = [
    0,
    1,
    2,
    (1 << 63) - 1,
    1 << 63,
    (1 << 63) + 1,
    u64::MAX - 2,
    u64::MAX - 1,
    u64::MAX,
];

// ---------------------------------------------------------------------------------------------
// Generic checks.
// ---------------------------------------------------------------------------------------------

/// `resize::<M>` against the oracle; resizing back restores the value.
fn resize_to<const L: usize, const M: usize>(x: Wide<L>, a: &BigInt) -> TestResult {
    let r = x.resize::<M>();
    prop_assert_eq!(r, mk::<M>(a), "Wide<{}> to Wide<{}>: {}", L, M, a);
    if let Some(r) = r {
        prop_assert_eq!(r.resize::<L>(), Some(x), "back to Wide<{}>", L);
    }
    Ok(())
}

/// `resize` to every width, odd and wide ones included.
fn resize_all<const L: usize>(x: Wide<L>, a: &BigInt) -> TestResult {
    resize_to::<L, 2>(x, a)?;
    resize_to::<L, 3>(x, a)?;
    resize_to::<L, 4>(x, a)?;
    resize_to::<L, 5>(x, a)?;
    resize_to::<L, 6>(x, a)?;
    resize_to::<L, 7>(x, a)?;
    resize_to::<L, 8>(x, a)?;
    resize_to::<L, 16>(x, a)?;
    Ok(())
}

/// Every one-operand operation against the oracle, plus the identities that tie them together.
fn unary<const L: usize>(x: Wide<L>) -> TestResult {
    let a = val(&x);
    prop_assert_eq!(mk::<L>(&a), Some(x), "oracle round trip of {}", a);
    prop_assert_eq!(x.signum(), sign_of(&a), "Wide<{}>: sign of {}", L, a);
    prop_assert_eq!(x.is_zero(), a.is_zero());
    prop_assert_eq!(x.is_negative(), a.is_negative());
    prop_assert_eq!(x.checked_neg(), mk::<L>(&-&a), "Wide<{}>: −({})", L, a);
    prop_assert_eq!(x.to_i128(), a.to_i128(), "Wide<{}>: {} as i128", L, a);
    if let Some(v) = x.to_i128() {
        prop_assert_eq!(Wide::<L>::from_i128(v), x);
    }
    if let Some(v) = a.to_i64() {
        prop_assert_eq!(Wide::<L>::from_i64(v), x);
    }
    let two = Wide::<L>::from_i64(2);
    prop_assert_eq!(
        x.checked_mul(&two),
        mk::<L>(&(&a * 2)),
        "Wide<{}>: {} × 2",
        L,
        a
    );
    prop_assert_eq!(
        x.checked_mul(&two),
        x.checked_add(&x),
        "x·2 = x + x, overflow included"
    );
    prop_assert_eq!(
        x.checked_mul(&Wide::<L>::from_i64(-1)),
        x.checked_neg(),
        "x·(−1) = −x"
    );
    prop_assert_eq!(
        x.checked_mul(&Wide::<L>::ZERO),
        Some(Wide::<L>::ZERO),
        "x·0 = 0, never −0 bits"
    );
    prop_assert_eq!(x.checked_sub(&x), Some(Wide::<L>::ZERO), "x − x = 0");
    prop_assert_eq!(x.checked_add(&Wide::<L>::ZERO), Some(x), "x + 0 = x");
    prop_assert_eq!(
        Wide::<L>::ZERO.checked_sub(&x),
        x.checked_neg(),
        "0 − x = −x"
    );
    let square = x.checked_mul(&x);
    prop_assert_eq!(square, mk::<L>(&(&a * &a)), "Wide<{}>: {}²", L, a);
    if let Some(s) = square {
        prop_assert!(!s.is_negative(), "a square is never negative");
    }
    prop_assert_eq!(format!("{x:?}"), debug_of(&x));
    resize_all(x, &a)?;
    Ok(())
}

/// Every two-operand operation against the oracle.
fn binary<const L: usize>(x: Wide<L>, y: Wide<L>) -> TestResult {
    let (a, b) = (val(&x), val(&y));
    prop_assert_eq!(
        x.checked_add(&y),
        mk::<L>(&(&a + &b)),
        "Wide<{}>: {} + {}",
        L,
        a,
        b
    );
    prop_assert_eq!(
        x.checked_sub(&y),
        mk::<L>(&(&a - &b)),
        "Wide<{}>: {} − {}",
        L,
        a,
        b
    );
    prop_assert_eq!(
        x.checked_mul(&y),
        mk::<L>(&(&a * &b)),
        "Wide<{}>: {} × {}",
        L,
        a,
        b
    );
    let expected = a.cmp(&b);
    prop_assert_eq!(x.cmp(&y), expected, "Wide<{}>: {} vs {}", L, a, b);
    prop_assert_eq!(x.partial_cmp(&y), Some(expected));
    prop_assert_eq!(x == y, a == b);
    prop_assert_eq!(x < y, a < b);
    prop_assert_eq!(x >= y, a >= b);
    Ok(())
}

/// Laws over three operands that need no oracle: each holds wherever its operations are defined.
fn laws<const L: usize>(x: Wide<L>, y: Wide<L>, z: Wide<L>) -> TestResult {
    if let (Some(d), Some(e)) = (x.checked_sub(&y), y.checked_sub(&x)) {
        prop_assert_eq!(d.checked_neg(), Some(e), "x − y = −(y − x)");
    }
    if let (Some(nx), Some(ny)) = (x.checked_neg(), y.checked_neg()) {
        prop_assert_eq!(
            nx.checked_mul(&ny),
            x.checked_mul(&y),
            "(−x)(−y) = xy, overflow included"
        );
    }
    if let (Some(nx), Some(p)) = (x.checked_neg(), x.checked_mul(&y)) {
        // |xy| <= max, so −xy is representable and (−x)·y must give it.
        prop_assert_eq!(nx.checked_mul(&y), p.checked_neg(), "(−x)·y = −(xy)");
    }
    if let Some(p) = x.checked_mul(&y) {
        prop_assert_eq!(p.is_zero(), x.is_zero() || y.is_zero(), "no zero divisors");
        prop_assert_eq!(p.signum(), x.signum().mul(y.signum()), "sign of a product");
    }
    if let (Some(p), Some(q)) = (x.checked_add(&z), y.checked_add(&z)) {
        prop_assert_eq!(p.cmp(&q), x.cmp(&y), "order is translation invariant");
        prop_assert_eq!(p == q, x == y, "cancellation");
    }
    if let (Some(p), Some(q)) = (x.checked_mul(&z), y.checked_mul(&z)) {
        let expected = match z.signum() {
            Sign::Positive => x.cmp(&y),
            Sign::Negative => x.cmp(&y).reverse(),
            Sign::Zero => Ordering::Equal,
        };
        prop_assert_eq!(
            p.cmp(&q),
            expected,
            "scaling keeps or reverses order by the sign"
        );
    }
    if let Some(d) = x.checked_sub(&y) {
        prop_assert_eq!(d.signum(), ordering_sign(x.cmp(&y)), "sign of x − y");
        prop_assert_eq!(d.checked_add(&y), Some(x), "(x − y) + y = x");
        prop_assert_eq!(x.checked_sub(&d), Some(y), "x − (x − y) = y");
    }
    if let Some(s) = x.checked_add(&y) {
        prop_assert_eq!(s.checked_sub(&y), Some(x), "(x + y) − y = x");
        prop_assert_eq!(s.checked_sub(&x), Some(y), "(x + y) − x = y");
    }
    Ok(())
}

/// x·k against repeated addition (or subtraction for k < 0): the partial sums j·x lie between 0
/// and k·x, so both overflow exactly when k·x leaves the range.
fn repeated<const L: usize>(x: Wide<L>, k: i64) -> TestResult {
    let mut acc = Some(Wide::<L>::ZERO);
    for _ in 0..k.unsigned_abs() {
        acc = acc.and_then(|s| {
            if k > 0 {
                s.checked_add(&x)
            } else {
                s.checked_sub(&x)
            }
        });
    }
    let product = x.checked_mul(&Wide::<L>::from_i64(k));
    prop_assert_eq!(
        product,
        acc,
        "Wide<{}>: {} × {} vs repeated addition",
        L,
        val(&x),
        k
    );
    prop_assert_eq!(product, mk::<L>(&(val(&x) * k)));
    Ok(())
}

/// The operations in `Wide<L>` agree with the same operations in a wider `Wide<M>` narrowed back.
/// A sum or difference always fits one limb wider; a product always fits `Wide<2L>`.
fn widen<const L: usize, const M: usize>(x: Wide<L>, y: Wide<L>) -> TestResult {
    assert!(M > L, "Wide<{M}> must be wider than Wide<{L}>");
    let xw = x.resize::<M>().expect("widening always fits");
    let yw = y.resize::<M>().expect("widening always fits");
    prop_assert_eq!(xw.signum(), x.signum());
    prop_assert_eq!(xw.cmp(&yw), x.cmp(&y), "widening keeps order");
    prop_assert_eq!(xw.to_i128(), x.to_i128());
    let s = xw.checked_add(&yw);
    prop_assert!(s.is_some(), "a sum of two Wide<{}> fits Wide<{}>", L, M);
    prop_assert_eq!(
        x.checked_add(&y),
        s.and_then(|s| s.resize::<L>()),
        "add narrow = add wide"
    );
    let d = xw.checked_sub(&yw);
    prop_assert!(
        d.is_some(),
        "a difference of two Wide<{}> fits Wide<{}>",
        L,
        M
    );
    prop_assert_eq!(
        x.checked_sub(&y),
        d.and_then(|d| d.resize::<L>()),
        "sub narrow = sub wide"
    );
    let p = xw.checked_mul(&yw);
    if M >= 2 * L {
        prop_assert!(p.is_some(), "a product of two Wide<{}> fits Wide<{}>", L, M);
    }
    prop_assert_eq!(
        x.checked_mul(&y),
        p.and_then(|p| p.resize::<L>()),
        "mul narrow = mul wide"
    );
    prop_assert_eq!(
        x.checked_neg(),
        xw.checked_neg().and_then(|n| n.resize::<L>()),
        "neg narrow = neg wide"
    );
    Ok(())
}

/// The constants, the asymmetric limits, and every product 2^k × 2^(64L−1−k) with each factor
/// moved by −1, 0 or +1 and every sign.
fn constants<const L: usize>() -> TestResult {
    let (zero, one) = (Wide::<L>::ZERO, Wide::<L>::one());
    let (max, min) = (Wide::<L>::max_value(), Wide::<L>::min_value());
    let minus_one = Wide::<L>::from_i64(-1);
    prop_assert_eq!(val(&zero), BigInt::zero());
    prop_assert_eq!(val(&one), BigInt::one());
    prop_assert_eq!(val(&max), max_of(L));
    prop_assert_eq!(val(&min), min_of(L));
    prop_assert_eq!(zero, Wide::<L>::from_i128(0));
    prop_assert_eq!(one, Wide::<L>::from_i64(1));
    prop_assert_eq!(min.checked_mul(&one), Some(min), "min × 1");
    prop_assert_eq!(one.checked_mul(&min), Some(min), "1 × min");
    prop_assert_eq!(min.checked_mul(&minus_one), None, "min × −1");
    prop_assert_eq!(minus_one.checked_mul(&min), None, "−1 × min");
    prop_assert_eq!(min.checked_mul(&zero), Some(zero), "min × 0");
    prop_assert_eq!(
        max.checked_mul(&minus_one),
        mk::<L>(&-max_of(L)),
        "max × −1"
    );
    prop_assert_eq!(max.checked_mul(&max), None);
    prop_assert_eq!(min.checked_mul(&min), None);
    prop_assert_eq!(min.checked_mul(&max), None);
    prop_assert_eq!(min.checked_add(&max), Some(minus_one));
    prop_assert_eq!(max.checked_sub(&min), None);
    prop_assert_eq!(min.checked_sub(&max), None);
    prop_assert_eq!(minus_one.checked_sub(&max), Some(min), "−1 − max = min");
    prop_assert_eq!(zero.checked_sub(&min), None, "0 − min");
    prop_assert_eq!(minus_one.checked_sub(&min), Some(max), "−1 − min = max");
    prop_assert_eq!(min.checked_add(&minus_one), None);
    prop_assert_eq!(max.checked_add(&one), None);
    prop_assert_eq!(
        format!("{max:?}"),
        format!("Wide<{L}>(0x7{})", "f".repeat(16 * L - 1))
    );
    prop_assert_eq!(
        format!("{min:?}"),
        format!("Wide<{L}>(0x8{})", "0".repeat(16 * L - 1))
    );
    let p = top(L);
    for k in 0..=p {
        for da in -1i32..=1 {
            for db in -1i32..=1 {
                let (ma, mb) = (pow2(k) + da, pow2(p - k) + db);
                for (a, b) in [
                    (ma.clone(), mb.clone()),
                    (-&ma, mb.clone()),
                    (ma.clone(), -&mb),
                    (-&ma, -&mb),
                ] {
                    if fits(&a, L) && fits(&b, L) {
                        let (x, y) = (must::<L>(&a), must::<L>(&b));
                        prop_assert_eq!(
                            x.checked_mul(&y),
                            mk::<L>(&(&a * &b)),
                            "Wide<{}>: {} × {}",
                            L,
                            a,
                            b
                        );
                        prop_assert_eq!(
                            y.checked_mul(&x),
                            mk::<L>(&(&a * &b)),
                            "Wide<{}>: {} × {}",
                            L,
                            b,
                            a
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Strategies.
// ---------------------------------------------------------------------------------------------

/// A limb from the edge patterns, or random.
fn limb() -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(0u64),
        Just(1u64),
        Just(2u64),
        Just(u64::MAX),
        Just(u64::MAX - 1),
        Just(1u64 << 63),
        Just((1u64 << 63) - 1),
        Just((1u64 << 63) + 1),
        any::<u64>(),
    ]
}

/// Any `Wide<L>`, weighted towards edge limbs, short values, every bit length and the limits.
fn wide<const L: usize>() -> BoxedStrategy<Wide<L>> {
    prop_oneof![
        3 => prop::array::uniform::<_, L>(limb()).prop_map(Wide::from_le_limbs),
        2 => prop::array::uniform::<_, L>(any::<u64>()).prop_map(Wide::from_le_limbs),
        1 => any::<i64>().prop_map(Wide::from_i64),
        1 => any::<i128>().prop_map(Wide::from_i128),
        3 => (1..=top(L), prop::array::uniform::<_, L>(any::<u64>()), any::<bool>()).prop_map(
            |(n, r, negative)| {
                // 2^(n−1) <= m < 2^n <= 2^(64L−1): representable with either sign.
                let low = pow2(n - 1);
                let m = &low + unsigned(&r) % &low;
                must::<L>(&if negative { -m } else { m })
            }
        ),
        2 => (0u32..1000, any::<bool>()).prop_map(|(k, high)| {
            must::<L>(&if high { max_of(L) - k } else { min_of(L) + k })
        }),
    ]
    .boxed()
}

/// Pairs whose product magnitude lands within a few |a| of 2^(64L−1), on either side, any signs.
fn product_pair<const L: usize>() -> BoxedStrategy<(Wide<L>, Wide<L>)> {
    (wide::<L>(), -1i32..=1, -2i32..=2, any::<bool>())
        .prop_filter_map("a non-zero factor", |(x, t, d, negative)| {
            let a = val(&x);
            if a.is_zero() {
                return None;
            }
            let mb = (pow2(top(L)) + t) / a.abs() + d;
            let b = clamp(if negative { -mb } else { mb }, L);
            Some((x, must::<L>(&b)))
        })
        .boxed()
}

/// Pairs whose sum or difference lands within 3 of a limit.
fn limit_pair<const L: usize>() -> BoxedStrategy<(Wide<L>, Wide<L>)> {
    (wide::<L>(), -3i32..=3, any::<bool>(), any::<bool>())
        .prop_map(|(x, d, high, difference)| {
            let a = val(&x);
            let limit = if high { max_of(L) } else { min_of(L) };
            let b = if difference {
                &a - limit - d
            } else {
                limit - &a + d
            };
            (x, must::<L>(&clamp(b, L)))
        })
        .boxed()
}

// ---------------------------------------------------------------------------------------------
// Per-width suites, at widths tests/wide.rs does and does not use.
// ---------------------------------------------------------------------------------------------

/// Every property and fixed attack for one width: `$wider` is one limb wider, `$double` twice as
/// wide.
macro_rules! width_attack {
    ($module:ident, $limbs:literal, $wider:literal, $double:literal) => {
        mod $module {
            use super::*;

            const L: usize = $limbs;

            proptest! {
                #![proptest_config(cfg())]

                #[test]
                fn unary_matches_oracle(x in wide::<L>()) {
                    unary::<L>(x)?;
                }

                #[test]
                fn binary_matches_oracle(x in wide::<L>(), y in wide::<L>()) {
                    binary::<L>(x, y)?;
                }

                #[test]
                fn products_at_the_threshold((x, y) in product_pair::<L>()) {
                    binary::<L>(x, y)?;
                }

                #[test]
                fn sums_and_differences_at_the_limits((x, y) in limit_pair::<L>()) {
                    binary::<L>(x, y)?;
                }

                #[test]
                fn three_operand_laws(x in wide::<L>(), y in wide::<L>(), z in wide::<L>()) {
                    laws::<L>(x, y, z)?;
                }

                #[test]
                fn small_operand_laws(x in any::<i64>(), y in any::<i64>(), z in -1000i64..=1000) {
                    laws::<L>(Wide::from_i64(x), Wide::from_i64(y), Wide::from_i64(z))?;
                }

                #[test]
                fn multiplication_is_repeated_addition(x in wide::<L>(), k in -9i64..=9) {
                    repeated::<L>(x, k)?;
                }

                #[test]
                fn narrow_agrees_with_wide(x in wide::<L>(), y in wide::<L>()) {
                    widen::<L, $wider>(x, y)?;
                    widen::<L, $double>(x, y)?;
                }

                #[test]
                fn narrow_agrees_with_wide_at_the_threshold((x, y) in product_pair::<L>()) {
                    widen::<L, $wider>(x, y)?;
                    widen::<L, $double>(x, y)?;
                }
            }

            #[test]
            fn constants_and_threshold_splits() {
                run(constants::<L>());
            }
        }
    };
}

width_attack!(w2, 2, 3, 4);
width_attack!(w3, 3, 4, 6);
width_attack!(w4, 4, 5, 8);
width_attack!(w7, 7, 8, 14);
width_attack!(w16, 16, 17, 32);

// ---------------------------------------------------------------------------------------------
// A second oracle: native i128 for Wide<2>.
// ---------------------------------------------------------------------------------------------

mod native_i128 {
    use super::*;

    /// The two's-complement limbs of an `i128`: its low and high 64 bits.
    fn limbs_of(v: i128) -> [u64; 2] {
        [v as u64, (v >> 64) as u64]
    }

    /// The `i128` whose two's-complement limbs these are.
    fn of_limbs([lo, hi]: [u64; 2]) -> i128 {
        ((u128::from(hi) << 64) | u128::from(lo)) as i128
    }

    /// `i128` values at limb boundaries, at the `i64` and `i128` limits and at √(2^127).
    fn edges() -> Vec<i128> {
        let mut v = vec![0i128, 1, -1, 2, -2, 3, -3];
        for k in [31u32, 32, 62, 63, 64, 65, 126] {
            let p = 1i128 << k;
            v.extend([p - 1, p, p + 1, -(p - 1), -p, -(p + 1)]);
        }
        let root = pow2(127).sqrt().to_i128().unwrap();
        v.extend([root - 1, root, root + 1, -(root - 1), -root, -(root + 1)]);
        v.extend([i128::MAX, i128::MAX - 1, i128::MIN, i128::MIN + 1]);
        v.extend([
            i128::from(i64::MAX),
            i128::from(i64::MIN),
            i128::from(u64::MAX),
            -i128::from(u64::MAX),
        ]);
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Every operation on `Wide<2>` against the native `i128` operation, limb for limb.
    fn check(a: i128, b: i128) -> TestResult {
        let (x, y) = (Wide::<2>::from_i128(a), Wide::<2>::from_i128(b));
        prop_assert_eq!(x.to_le_limbs(), limbs_of(a), "from_i128({})", a);
        prop_assert_eq!(Wide::<2>::from_le_limbs(limbs_of(a)), x);
        prop_assert_eq!(
            x.checked_add(&y).map(Wide::to_le_limbs),
            a.checked_add(b).map(limbs_of),
            "{} + {}",
            a,
            b
        );
        prop_assert_eq!(
            x.checked_sub(&y).map(Wide::to_le_limbs),
            a.checked_sub(b).map(limbs_of),
            "{} − {}",
            a,
            b
        );
        prop_assert_eq!(
            x.checked_mul(&y).map(Wide::to_le_limbs),
            a.checked_mul(b).map(limbs_of),
            "{} × {}",
            a,
            b
        );
        prop_assert_eq!(
            x.checked_neg().map(Wide::to_le_limbs),
            a.checked_neg().map(limbs_of),
            "−({})",
            a
        );
        prop_assert_eq!(x.cmp(&y), a.cmp(&b), "{} vs {}", a, b);
        prop_assert_eq!(x.to_i128(), Some(a));
        prop_assert_eq!(x.signum(), Sign::of_i128(a));
        prop_assert_eq!(x.is_negative(), a < 0);
        prop_assert_eq!(x.is_zero(), a == 0);
        // In I256 every i128 product fits; it must equal the i128 product whenever that exists.
        let (xw, yw) = (I256::from_i128(a), I256::from_i128(b));
        let pw = xw.checked_mul(&yw);
        prop_assert!(pw.is_some(), "{} × {} fits I256", a, b);
        prop_assert_eq!(pw.and_then(|p| p.to_i128()), a.checked_mul(b));
        Ok(())
    }

    #[test]
    fn every_pair_of_edge_values_matches_i128() {
        let e = edges();
        for &a in &e {
            for &b in &e {
                run(check(a, b));
            }
        }
    }

    #[test]
    fn i128_extremes_by_hand() {
        let (max, min) = (Wide::<2>::max_value(), Wide::<2>::min_value());
        assert_eq!(max.to_i128(), Some(i128::MAX));
        assert_eq!(min.to_i128(), Some(i128::MIN));
        assert_eq!(max, Wide::<2>::from_i128(i128::MAX));
        assert_eq!(min, Wide::<2>::from_i128(i128::MIN));
        assert_eq!(of_limbs(min.to_le_limbs()), i128::MIN);
        assert_eq!(of_limbs(max.to_le_limbs()), i128::MAX);
        assert_eq!(
            format!("{:?}", Wide::<2>::from_i64(-2)),
            "Wide<2>(0xfffffffffffffffffffffffffffffffe)"
        );
        assert_eq!(
            format!("{:?}", Wide::<2>::one()),
            "Wide<2>(0x00000000000000000000000000000001)"
        );
    }

    proptest! {
        #![proptest_config(cfg())]

        #[test]
        fn random_pairs_match_i128(a in any::<i128>(), b in any::<i128>()) {
            check(a, b)?;
        }

        #[test]
        fn short_pairs_match_i128(a in any::<i64>(), b in any::<i64>()) {
            check(i128::from(a), i128::from(b))?;
        }

        #[test]
        fn raw_limbs_match_i128(x in prop::array::uniform2(limb()), y in prop::array::uniform2(limb())) {
            check(of_limbs(x), of_limbs(y))?;
        }

        #[test]
        fn products_at_the_i128_threshold(a in any::<i128>(), d in -2i128..=2, toward_min in any::<bool>()) {
            prop_assume!(a != 0);
            let limit = if toward_min { i128::MIN } else { i128::MAX };
            // −1 divides i128::MIN out of range: fall back to the other limit.
            let q = limit.checked_div(a).unwrap_or(i128::MAX / a);
            check(a, q.saturating_add(d))?;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Exhaustive limb-pattern sweeps.
// ---------------------------------------------------------------------------------------------

mod sweeps {
    use super::*;

    #[test]
    fn oracle_reads_and_writes_hand_written_limbs() {
        assert_eq!(
            val(&Wide::<2>::from_le_limbs([0, 1 << 63])),
            BigInt::from(i128::MIN)
        );
        assert_eq!(
            val(&Wide::<3>::from_le_limbs([u64::MAX; 3])),
            BigInt::from(-1)
        );
        assert_eq!(val(&Wide::<3>::from_le_limbs([0, 0, 1 << 63])), -pow2(191));
        assert_eq!(
            mk::<3>(&BigInt::from(-2)).map(Wide::to_le_limbs),
            Some([u64::MAX - 1, u64::MAX, u64::MAX])
        );
        assert_eq!(mk::<3>(&pow2(128)).map(Wide::to_le_limbs), Some([0, 0, 1]));
        assert_eq!(mk::<3>(&pow2(191)), None);
        assert_eq!(mk::<3>(&(-pow2(191) - 1u32)), None);
        assert_eq!(
            mk::<7>(&(-pow2(447))).map(Wide::to_le_limbs),
            Some([0, 0, 0, 0, 0, 0, 1 << 63])
        );
    }

    #[test]
    fn every_wide2_limb_pattern_and_pair() {
        let all = patterns::<2>(&LIMB_EDGES);
        assert_eq!(all.len(), 81);
        for &x in &all {
            run(unary(x));
            for &y in &all {
                run(binary(x, y));
            }
        }
    }

    #[test]
    fn every_wide3_limb_pattern_and_pair() {
        let set = [
            0,
            1,
            (1 << 63) - 1,
            1 << 63,
            (1 << 63) + 1,
            u64::MAX - 1,
            u64::MAX,
        ];
        let all = patterns::<3>(&set);
        assert_eq!(all.len(), 343);
        for &x in &all {
            run(unary(x));
            for &y in &all {
                run(binary(x, y));
            }
        }
    }

    #[test]
    fn every_i256_limb_pattern_and_pair() {
        let all = patterns::<4>(&[0, 1, 1 << 63, u64::MAX]);
        assert_eq!(all.len(), 256);
        for &x in &all {
            run(unary(x));
            for &y in &all {
                run(binary(x, y));
            }
        }
    }

    #[test]
    fn sorting_by_ord_sorts_by_value() {
        let set = [
            0,
            1,
            (1 << 63) - 1,
            1 << 63,
            (1 << 63) + 1,
            u64::MAX - 1,
            u64::MAX,
        ];
        let mut all = patterns::<3>(&set);
        all.reverse();
        all.sort();
        let values: Vec<BigInt> = all.iter().map(val).collect();
        assert!(
            values.windows(2).all(|w| w[0] < w[1]),
            "Ord sorts strictly by value, distinct limbs distinct values"
        );
        for (i, x) in all.iter().enumerate() {
            assert_eq!(all.binary_search(x), Ok(i));
        }
    }

    #[test]
    fn resize_chains_through_odd_widths() {
        for &x in &patterns::<2>(&LIMB_EDGES) {
            let a = val(&x);
            let chain = x
                .resize::<3>()
                .and_then(|v| v.resize::<7>())
                .and_then(|v| v.resize::<16>())
                .and_then(|v| v.resize::<5>())
                .and_then(|v| v.resize::<2>());
            assert_eq!(chain, Some(x), "2 → 3 → 7 → 16 → 5 → 2 restores {a}");
            let raw = x.resize::<7>().unwrap().to_le_limbs();
            let fill = if a.is_negative() { u64::MAX } else { 0 };
            assert!(
                raw[2..].iter().all(|&l| l == fill),
                "widening sign-extends {a}"
            );
        }
        // Narrowing checks the dropped limbs and the surviving sign bit, in odd widths too.
        let just_out = Wide::<7>::from_le_limbs([0, 0, 1 << 63, 0, 0, 0, 0]);
        assert_eq!(just_out.resize::<3>(), None, "2^191 does not fit Wide<3>");
        let just_in = Wide::<7>::from_le_limbs([u64::MAX, u64::MAX, u64::MAX >> 1, 0, 0, 0, 0]);
        assert_eq!(just_in.resize::<3>(), Some(Wide::<3>::max_value()));
        let low = Wide::<7>::from_le_limbs([0, 0, 1 << 63, u64::MAX, u64::MAX, u64::MAX, u64::MAX]);
        assert_eq!(low.resize::<3>(), Some(Wide::<3>::min_value()));
        let below = Wide::<7>::from_le_limbs([
            u64::MAX,
            u64::MAX,
            (1 << 63) - 1,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            u64::MAX,
        ]);
        assert_eq!(below.resize::<3>(), None, "−2^191 − 1 does not fit Wide<3>");
    }

    #[test]
    fn constants_usable_in_const_context() {
        const Z: I256 = I256::ZERO;
        const RAW: I320 = I320::from_le_limbs([1, 2, 3, 4, 5]);
        const BACK: [u64; 5] = RAW.to_le_limbs();
        assert!(Z.is_zero());
        assert_eq!(BACK, [1, 2, 3, 4, 5]);
        assert_eq!(val(&RAW), unsigned(&[1, 2, 3, 4, 5]));
        assert_eq!(I384::ZERO, I384::from_i64(0));
        assert_eq!(I512::one().to_le_limbs(), [1, 0, 0, 0, 0, 0, 0, 0]);
    }
}

// ---------------------------------------------------------------------------------------------
// The L >= 2 guard.
// ---------------------------------------------------------------------------------------------

mod guard {
    use super::*;

    /// Values of a one-limb `Wide`, as `i64`.
    fn one_limb_values() -> Vec<i64> {
        LIMB_EDGES
            .iter()
            .map(|&l| l as i64)
            .chain([-1, 7, -12_345, i64::MIN, i64::MAX])
            .collect()
    }

    /// `Wide<1>` cannot be made by `from_i128`, `one`, `max_value` or `min_value` (they evaluate
    /// the guard), but `ZERO` and `from_le_limbs` do not: once built, its arithmetic must be right.
    #[test]
    fn wide1_arithmetic_that_compiles_is_exact() {
        for a in one_limb_values() {
            let x = Wide::<1>::from_le_limbs([a as u64]);
            assert_eq!(x.signum(), Sign::of_i64(a));
            assert_eq!(x.is_negative(), a < 0);
            assert_eq!(
                x.checked_neg().map(|n| n.to_le_limbs()[0] as i64),
                a.checked_neg()
            );
            assert_eq!(
                x.resize::<2>(),
                Some(Wide::<2>::from_i64(a)),
                "Wide<1>({a}) widened"
            );
            for b in one_limb_values() {
                let y = Wide::<1>::from_le_limbs([b as u64]);
                assert_eq!(
                    x.checked_add(&y).map(|r| r.to_le_limbs()[0] as i64),
                    a.checked_add(b),
                    "{a} + {b}"
                );
                assert_eq!(
                    x.checked_sub(&y).map(|r| r.to_le_limbs()[0] as i64),
                    a.checked_sub(b),
                    "{a} − {b}"
                );
                assert_eq!(x.cmp(&y), a.cmp(&b), "{a} vs {b}");
            }
        }
        assert!(Wide::<1>::ZERO.is_zero());
    }

    /// `to_i128` of a non-negative `Wide<1>`.
    #[test]
    fn wide1_to_i128_of_non_negative_values() {
        for a in one_limb_values().into_iter().filter(|&a| a >= 0) {
            assert_eq!(
                Wide::<1>::from_le_limbs([a as u64]).to_i128(),
                Some(i128::from(a)),
                "Wide<1>({a})"
            );
        }
    }

    /// `to_i128` of a negative `Wide<1>`: "the value as an i128, if it fits" — every i64 fits.
    /// `to_i128` reads the missing second limb as 0 instead of the sign fill, gets 2^64 + a, sees
    /// the sign disagree and returns `None`.
    #[test]
    fn wide1_to_i128_of_negative_values() {
        for a in one_limb_values().into_iter().filter(|&a| a < 0) {
            assert_eq!(
                Wide::<1>::from_le_limbs([a as u64]).to_i128(),
                Some(i128::from(a)),
                "Wide<1>({a})"
            );
        }
    }

    /// `Wide<0>` is constructible too. It can only hold zero, and every operation that compiles
    /// treats it as zero.
    #[test]
    fn wide0_is_constructible_and_is_zero() {
        let z = Wide::<0>::from_le_limbs([]);
        assert_eq!(z, Wide::<0>::ZERO);
        assert!(z.is_zero() && !z.is_negative());
        assert_eq!(z.signum(), Sign::Zero);
        assert_eq!(z.checked_add(&z), Some(z));
        assert_eq!(z.checked_sub(&z), Some(z));
        assert_eq!(z.checked_neg(), Some(z));
        assert_eq!(z.to_i128(), Some(0));
        assert_eq!(z.resize::<2>(), Some(Wide::<2>::ZERO));
        assert_eq!(z.cmp(&z), Ordering::Equal);
        assert_eq!(format!("{z:?}"), "Wide<0>(0x)");
    }
}

// ---------------------------------------------------------------------------------------------
// The kernel's extreme inputs.
// ---------------------------------------------------------------------------------------------

mod kernel_extremes {
    use super::*;

    /// Products of two `i128` values in `I256`, the predicate module's `sum_of_products` step: the
    /// largest, 2^254, fits; two of them do not; the most negative pair sums exactly to the
    /// minimum.
    #[test]
    fn i128_products_fill_i256_exactly() {
        let (lo, hi) = (I256::from_i128(i128::MIN), I256::from_i128(i128::MAX));
        let big = lo.checked_mul(&lo).unwrap();
        assert_eq!(val(&big), pow2(254), "(−2^127)² = 2^254");
        assert_eq!(
            big.checked_add(&big),
            None,
            "2^254 + 2^254 = 2^255 leaves I256"
        );
        let neg = big.checked_neg().unwrap();
        assert_eq!(
            neg.checked_add(&neg),
            Some(I256::min_value()),
            "−2^254 − 2^254 = −2^255 = min"
        );
        assert_eq!(
            neg.checked_add(&neg)
                .and_then(|m| m.checked_sub(&I256::one())),
            None
        );
        let mixed = lo.checked_mul(&hi).unwrap();
        assert_eq!(val(&mixed), BigInt::from(i128::MIN) * i128::MAX);
        assert_eq!(
            val(&hi.checked_mul(&hi).unwrap()),
            BigInt::from(i128::MAX) * i128::MAX
        );
        // Three products of 2^254 magnitude: the partial sums leave I256 even when the total would not.
        let total = big.checked_add(&neg).and_then(|s| s.checked_add(&big));
        assert_eq!(total, Some(big));
        assert_eq!(
            big.checked_add(&big).and_then(|s| s.checked_add(&neg)),
            None,
            "order of a sum matters"
        );
        // Wide<2> holds none of these products.
        assert_eq!(
            Wide::<2>::from_i128(i128::MIN).checked_mul(&Wide::<2>::from_i128(2)),
            None
        );
    }

    /// A `Wide<L>` with its exact value: every operation is checked against the oracle, and gives
    /// `None` exactly when the true result leaves the range.
    #[derive(Clone)]
    struct Tracked<const L: usize> {
        w: Wide<L>,
        v: BigInt,
    }

    impl<const L: usize> Tracked<L> {
        fn of(c: i64) -> Self {
            Self {
                w: Wide::from_i64(c),
                v: BigInt::from(c),
            }
        }

        fn step(&self, got: Option<Wide<L>>, v: BigInt, op: &str, other: &Self) -> Option<Self> {
            assert_eq!(got, mk::<L>(&v), "Wide<{L}>: {} {op} {}", self.v, other.v);
            got.map(|w| Self { w, v })
        }

        fn add(&self, o: &Self) -> Option<Self> {
            self.step(self.w.checked_add(&o.w), &self.v + &o.v, "+", o)
        }

        fn sub(&self, o: &Self) -> Option<Self> {
            self.step(self.w.checked_sub(&o.w), &self.v - &o.v, "−", o)
        }

        fn mul(&self, o: &Self) -> Option<Self> {
            self.step(self.w.checked_mul(&o.w), &self.v * &o.v, "×", o)
        }
    }

    /// One row of the lifted in-sphere matrix: q − e and |q − e|².
    fn row<const L: usize>(q: &[i64; 3], e: &[i64; 3]) -> Option<[Tracked<L>; 4]> {
        let dx = Tracked::<L>::of(q[0]).sub(&Tracked::of(e[0]))?;
        let dy = Tracked::<L>::of(q[1]).sub(&Tracked::of(e[1]))?;
        let dz = Tracked::<L>::of(q[2]).sub(&Tracked::of(e[2]))?;
        let lift = dx.mul(&dx)?.add(&dy.mul(&dy)?)?.add(&dz.mul(&dz)?)?;
        Some([dx, dy, dz, lift])
    }

    /// The 3 × 3 determinant of the first three columns of rows p, q, r.
    fn det3<const L: usize>(
        p: &[Tracked<L>; 4],
        q: &[Tracked<L>; 4],
        r: &[Tracked<L>; 4],
    ) -> Option<Tracked<L>> {
        let m1 = q[1].mul(&r[2])?.sub(&q[2].mul(&r[1])?)?;
        let m2 = q[0].mul(&r[2])?.sub(&q[2].mul(&r[0])?)?;
        let m3 = q[0].mul(&r[1])?.sub(&q[1].mul(&r[0])?)?;
        p[0].mul(&m1)?.sub(&p[1].mul(&m2)?)?.add(&p[2].mul(&m3)?)
    }

    /// The in-sphere determinant of a, b, c, d against e, expanded along the lifted column, or
    /// `None` at the first intermediate that leaves `Wide<L>`.
    fn insphere<const L: usize>(p: &[[i64; 3]; 5]) -> Option<Tracked<L>> {
        let e = &p[4];
        let (a, b, c, d) = (
            row::<L>(&p[0], e)?,
            row::<L>(&p[1], e)?,
            row::<L>(&p[2], e)?,
            row::<L>(&p[3], e)?,
        );
        let (ma, mb, mc, md) = (
            det3(&b, &c, &d)?,
            det3(&a, &c, &d)?,
            det3(&a, &b, &d)?,
            det3(&a, &b, &c)?,
        );
        let t1 = b[3].mul(&mb)?.sub(&a[3].mul(&ma)?)?;
        let t2 = d[3].mul(&md)?.sub(&c[3].mul(&mc)?)?;
        t1.add(&t2)
    }

    /// The same determinant by the Leibniz formula over `BigInt`, independently of the expansion.
    fn insphere_leibniz(p: &[[i64; 3]; 5]) -> BigInt {
        let e = &p[4];
        let m: Vec<[BigInt; 4]> = p[..4]
            .iter()
            .map(|q| {
                let d: Vec<BigInt> = (0..3).map(|k| BigInt::from(q[k]) - e[k]).collect();
                let lift = &d[0] * &d[0] + &d[1] * &d[1] + &d[2] * &d[2];
                [d[0].clone(), d[1].clone(), d[2].clone(), lift]
            })
            .collect();
        let mut total = BigInt::zero();
        for s0 in 0..4usize {
            for s1 in 0..4usize {
                for s2 in 0..4usize {
                    for s3 in 0..4usize {
                        let s = [s0, s1, s2, s3];
                        if (0..4).any(|i| (i + 1..4).any(|j| s[i] == s[j])) {
                            continue;
                        }
                        let inversions = (0..4)
                            .map(|i| (i + 1..4).filter(|&j| s[i] > s[j]).count())
                            .sum::<usize>();
                        let term = &m[0][s0] * &m[1][s1] * &m[2][s2] * &m[3][s3];
                        if inversions % 2 == 0 {
                            total += term;
                        } else {
                            total -= term;
                        }
                    }
                }
            }
        }
        total
    }

    /// Where e sits for corner configuration `bits`: a corner (C, C, C), the box centre, a face
    /// centre or an edge midpoint, in turn.
    fn e_for(bits: u32) -> [i64; 3] {
        match bits % 4 {
            0 => [C, C, C],
            1 => [0, 0, 0],
            2 => [C, 0, 0],
            _ => [C, C, 0],
        }
    }

    /// Five points: a, b, c, d at the corners ±(2^40 − 1) chosen by the 12 bits, e from [`e_for`].
    fn corners(bits: u32) -> [[i64; 3]; 5] {
        let c = |i: u32| if (bits >> i) & 1 == 1 { C } else { -C };
        [
            [c(0), c(1), c(2)],
            [c(3), c(4), c(5)],
            [c(6), c(7), c(8)],
            [c(9), c(10), c(11)],
            e_for(bits),
        ]
    }

    /// The module doc's in-sphere budget (about 212 bits): at every corner pattern of a, b, c, d
    /// the I256 evaluation succeeds, matches the Leibniz determinant, and stays below 2^212.
    ///
    /// Degenerate inputs: the eight corners of a box lie on one sphere, so with e at a corner the
    /// determinant must be exactly zero, even though its intermediates exceed 2^191.
    ///
    /// `Wide<3>` (limit 2^191) is too narrow: it must overflow on some inputs, fit on others, and
    /// agree with the oracle at every step either way.
    #[test]
    fn insphere_at_every_extreme_corner() {
        let (mut fitted, mut overflowed) = (0, 0);
        let mut largest = BigInt::zero();
        for bits in 0u32..1 << 12 {
            let p = corners(bits);
            let exact = insphere_leibniz(&p);
            let det = insphere::<4>(&p).unwrap_or_else(|| panic!("insphere fits I256 at {p:?}"));
            assert_eq!(det.v, exact, "the expansion is the determinant at {p:?}");
            assert!(exact.abs() < pow2(212), "below 2^212 at {p:?}");
            if bits % 4 == 0 {
                assert_eq!(
                    det.w.signum(),
                    Sign::Zero,
                    "box corners are cospherical: {p:?}"
                );
                assert_eq!(det.w, I256::ZERO);
            }
            largest = largest.max(exact.abs());
            match insphere::<3>(&p) {
                Some(narrow) => {
                    assert_eq!(narrow.v, exact);
                    fitted += 1;
                }
                None => overflowed += 1,
            }
        }
        assert!(
            fitted > 0 && overflowed > 0,
            "fitted {fitted}, overflowed {overflowed}"
        );
        assert!(
            largest.bits() > 191,
            "the extremes exceed Wide<3>: {} bits",
            largest.bits()
        );
    }

    /// A coordinate, weighted towards the extremes.
    fn coordinate() -> impl Strategy<Value = i64> {
        prop_oneof![
            3 => Just(C),
            3 => Just(-C),
            1 => Just(C - 1),
            1 => Just(-C + 1),
            1 => Just(0i64),
            3 => -C..=C,
        ]
    }

    proptest! {
        #![proptest_config(Config { cases: 256, ..cfg() })]

        #[test]
        fn insphere_near_the_extremes(p in prop::array::uniform5(prop::array::uniform3(coordinate()))) {
            let exact = insphere_leibniz(&p);
            prop_assert!(exact.abs() < pow2(212));
            let det = insphere::<4>(&p);
            prop_assert!(det.is_some(), "insphere fits I256 at {:?}", p);
            prop_assert_eq!(det.map(|d| d.v), Some(exact.clone()));
            let wider = insphere::<8>(&p).map(|d| d.v);
            prop_assert_eq!(wider, Some(exact));
            // Every step in the narrow widths is checked inside `Tracked`, overflow or not.
            let _ = insphere::<3>(&p);
            let _ = insphere::<2>(&p);
        }
    }
}
