//! Inputs for double-double arithmetic, `geom_num::dd`: the product by a fused multiply-add
//! ([`two_prod_fma`]), the exact constructors ([`from_sum`], [`exact_product`], [`from_i64`],
//! [`from_i128`]), the sign, negation and magnitude, the seven arithmetic operations, rounding to a
//! whole number ([`round_shifted`]) and the order ([`cmp`]).
//!
//! A `DD` is made here only by `DD::from_sum`, which returns the normalised pair of an exact sum,
//! so every input is a pair the type can hold. Classes:
//!
//! - `typical`: leading parts with exponents from −30 to 30 and trailing parts the exact error of
//!   a sum with a term some 55 to 60 binades smaller, as survey and frame conversions give: the
//!   direct path of every operation. For the integer constructors, values of every size in range;
//!   for [`round_shifted`], values below 2^60 with a fraction, and offsets below 2^60.
//! - `extreme`, in rotation, every other path and the edges of the direct one:
//!   - [`two_prod_fma`] and [`exact_product`]: products near overflow, products just above and
//!     just below 2^−968 (where the fused multiply-add stops and `eft::two_prod` takes over), a
//!     zero factor, products that overflow, and products of two tiny factors whose errors lie
//!     below 2^−1074; for [`exact_product`], a factor that is not finite as well.
//!   - [`from_sum`]: sums near overflow, sums that overflow, subnormal sums, a huge term with a
//!     tiny one, and a NaN or an infinity.
//!   - the arithmetic: results near the top of the range and results that overflow (refused),
//!     results in the subnormal range, a zero operand (the shortcut), exact cancellation, and for
//!     the operations with a double a NaN or an infinity (refused); for the quotients a zero
//!     divisor (refused); for [`sqrt`] negative values (refused), zero, and values near both ends
//!     of the range with odd and even exponents.
//!   - [`round_shifted`]: exact halves of both parities, in the leading part and split across
//!     both, whole numbers, values at 2^100 (the largest it answers), and offsets near both ends
//!     of `i128`.
//! - `overflow`: [`from_i128`] from 2^106 up, and [`round_shifted`] above 2^100 or with a result
//!   outside `i128`, all refused.
//!
//! Guarantees: every batch is full and the same on every target, and takes the paths listed
//! (`tests/inputs.rs` checks each claim); every result is checked against exact rationals by
//! `tests/attack_g06.rs`.
//!
//! Method: boundary-value analysis (<https://en.wikipedia.org/wiki/Boundary-value_analysis>) on the
//! paths `geom_num::dd` documents: the fused multiply-add threshold of `two_prod_fma`, the scaling
//! of the multiplicative operations, the overflow limit `(1 − 2^−52)·2^1024`, gradual underflow,
//! and the halves of the rounding to a whole number.
//!
//! Cost: a few draws of [`SplitMix64`] per input, once per batch, outside every measured run.

use geom_num::dd::DD;

use super::eft::{double, pow2, subnormal};
use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// A pair whose leading part has an exponent in `lo..=hi` and whose trailing part is the exact
/// error of adding a term 55 to 60 binades smaller; `None` only if the sum overflows.
fn dd_in(g: &mut SplitMix64, lo: i64, hi: i64) -> Option<DD> {
    let a = double(g, lo, hi);
    let shift = g.range(55, 60);
    let b = double(g, lo - shift, hi - shift);
    DD::from_sum(a, b).ok()
}

/// A typical pair.
fn typical(g: &mut SplitMix64) -> Option<DD> {
    dd_in(g, -30, 30)
}

/// A pair near the top of the range, leading exponent 1015 to 1022.
fn huge(g: &mut SplitMix64) -> Option<DD> {
    dd_in(g, 1015, 1022)
}

/// A pair in the subnormal range: the exact sum of two subnormals.
fn tiny(g: &mut SplitMix64) -> Option<DD> {
    DD::from_sum(subnormal(g), subnormal(g)).ok()
}

/// A non-finite double: NaN, +∞ or −∞, in rotation.
fn not_finite(i: usize) -> f64 {
    [f64::NAN, f64::INFINITY, f64::NEG_INFINITY]
        .get(i % 3)
        .copied()
        .unwrap_or(f64::NAN)
}

/// A zero, +0 or −0.
fn zero(g: &mut SplitMix64) -> f64 {
    if g.sign() < 0 { -0.0 } else { 0.0 }
}

/// Factor pairs for the exact products: typical ones; or, in rotation, the extreme ones the module
/// documentation lists (with a non-finite factor when `with_not_finite`).
fn factors(g: &mut SplitMix64, class: Class, with_not_finite: bool) -> Vec<(f64, f64)> {
    match class {
        Class::Typical => batch_of(|_| Some((double(g, -30, 30), double(g, -30, 30)))),
        Class::Extreme => {
            let kinds = if with_not_finite { 7 } else { 6 };
            batch_of(|i| {
                Some(match i % kinds {
                    // Below 2^1023: near overflow, never over it.
                    0 => (double(g, 510, 510), double(g, 511, 511)),
                    // At 2^−968 and above it: the fused multiply-add's path.
                    1 => (double(g, -484, -484), double(g, -484, -484)),
                    // Below 2^−968: eft::two_prod's scaled path.
                    2 => (double(g, -486, -485), double(g, -485, -485)),
                    3 => (zero(g), double(g, -30, 30)),
                    // Overflow.
                    4 => (double(g, 600, 700), double(g, 600, 700)),
                    // Both tiny: the error lies below 2^−1074.
                    5 => (double(g, -700, -600), double(g, -700, -600)),
                    _ => (not_finite(i / kinds), double(g, -30, 30)),
                })
            })
        }
        _ => Vec::new(),
    }
}

/// (a, b) for `dd::two_prod_fma`.
#[must_use]
pub fn two_prod_fma(class: Class) -> Vec<(f64, f64)> {
    factors(&mut rng("dd::two_prod_fma", class), class, false)
}

/// (a, b) for `DD::exact_product`.
#[must_use]
pub fn exact_product(class: Class) -> Vec<(f64, f64)> {
    factors(&mut rng("dd::exact_product", class), class, true)
}

/// (a, b) for `DD::from_sum`.
#[must_use]
pub fn from_sum(class: Class) -> Vec<(f64, f64)> {
    let mut g = rng("dd::from_sum", class);
    match class {
        Class::Typical => batch_of(|_| Some((double(&mut g, -30, 30), double(&mut g, -30, 30)))),
        Class::Extreme => batch_of(|i| {
            let g = &mut g;
            Some(match i % 5 {
                // Near overflow, opposite signs: the sum stays in range.
                0 => {
                    let a = double(g, 1022, 1023);
                    (a, -double(g, 1022, 1023).copysign(a))
                }
                // Both at the top with one sign: the rounded sum overflows.
                1 => {
                    let a = double(g, 1023, 1023);
                    (a, double(g, 1023, 1023).copysign(a))
                }
                2 => (subnormal(g), subnormal(g)),
                // A huge term and a tiny one: the tiny one is all trailing part.
                3 => (double(g, 900, 1000), double(g, -1000, -900)),
                _ => (not_finite(i / 5), double(g, -30, 30)),
            })
        }),
        _ => Vec::new(),
    }
}

/// Values for `DD::from_i64`: every size, and the ends of `i64`, in rotation.
#[must_use]
pub fn from_i64(class: Class) -> Vec<i64> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("dd::from_i64", class);
    batch_of(|i| {
        Some(match i % 8 {
            0 => i64::MIN,
            1 => i64::MAX,
            2 => 0,
            3 => -1,
            _ => g.next_u64().cast_signed() >> g.below(63),
        })
    })
}

/// A pseudo-random `i128`, every bit drawn.
fn wide(g: &mut SplitMix64) -> i128 {
    (u128::from(g.next_u64()) << 64 | u128::from(g.next_u64())).cast_signed()
}

/// Values for `DD::from_i128`: below 2^106 in magnitude, of every size; or from 2^106 up.
#[must_use]
pub fn from_i128(class: Class) -> Vec<i128> {
    let mut g = rng("dd::from_i128", class);
    match class {
        Class::Typical => batch_of(|i| {
            Some(match i % 8 {
                0 => (1 << 106) - 1,
                1 => 1 - (1 << 106),
                2 => 0,
                _ => wide(&mut g) >> (22 + g.below(100)),
            })
        }),
        Class::Overflow => batch_of(|i| {
            Some(match i % 4 {
                0 => 1 << 106,
                1 => -(1 << 106),
                2 => i128::MIN,
                _ => {
                    let v = wide(&mut g) >> g.below(21);
                    if v.unsigned_abs() < 1 << 106 {
                        i128::MAX
                    } else {
                        v
                    }
                }
            })
        }),
        _ => Vec::new(),
    }
}

/// Typical pairs with both signs and zero, in rotation, for the sign, negation and magnitude.
fn signed(op: &str, class: Class) -> Vec<DD> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng(op, class);
    batch_of(|i| match i % 4 {
        0 => Some(DD::ZERO),
        1 => typical(&mut g).map(DD::negate),
        _ => typical(&mut g),
    })
}

/// Values for `DD::sign`.
#[must_use]
pub fn sign(class: Class) -> Vec<DD> {
    signed("dd::sign", class)
}

/// Values for `DD::negate`.
#[must_use]
pub fn negate(class: Class) -> Vec<DD> {
    signed("dd::negate", class)
}

/// Values for `DD::abs`.
#[must_use]
pub fn abs(class: Class) -> Vec<DD> {
    signed("dd::abs", class)
}

/// What a binary operation's extreme operands may be: the shared rotation, with each operation's
/// own edges added by its builder.
enum Edge {
    /// Both near the top of the range.
    Huge,
    /// Both in the subnormal range.
    Tiny,
    /// A zero operand.
    Zero,
    /// One near the top, the other tiny.
    HugeTiny,
    /// One tiny, the other near the top.
    TinyHuge,
}

/// The edge of input `i` of an extreme batch.
fn edge(i: usize) -> Edge {
    match i % 5 {
        0 => Edge::Huge,
        1 => Edge::Tiny,
        2 => Edge::Zero,
        3 => Edge::HugeTiny,
        _ => Edge::TinyHuge,
    }
}

/// A pair for an edge's first (`first`) or second operand.
fn edge_dd(g: &mut SplitMix64, e: &Edge, first: bool) -> Option<DD> {
    match (e, first) {
        (Edge::Huge, _) | (Edge::HugeTiny, true) | (Edge::TinyHuge, false) => huge(g),
        (Edge::Tiny, _) | (Edge::HugeTiny, false) | (Edge::TinyHuge, true) => tiny(g),
        (Edge::Zero, true) => typical(g),
        (Edge::Zero, false) => Some(DD::ZERO),
    }
}

/// A double for an edge's second operand.
fn edge_f64(g: &mut SplitMix64, e: &Edge) -> f64 {
    match e {
        Edge::Huge | Edge::TinyHuge => double(g, 1015, 1023),
        Edge::Tiny | Edge::HugeTiny => subnormal(g),
        Edge::Zero => zero(g),
    }
}

/// (x, y) for `DD::add`: typical pairs; or the edges, exact cancellation, and huge sums of one
/// sign that overflow.
#[must_use]
pub fn add(class: Class) -> Vec<(DD, DD)> {
    let mut g = rng("dd::add", class);
    match class {
        Class::Typical => batch_of(|_| Some((typical(&mut g)?, typical(&mut g)?))),
        Class::Extreme => batch_of(|i| match i % 7 {
            5 => {
                let x = typical(&mut g)?;
                Some((x, x.negate()))
            }
            6 => {
                let x = dd_in(&mut g, 1023, 1023)?;
                Some((x, x))
            }
            _ => {
                let e = edge(i);
                Some((edge_dd(&mut g, &e, true)?, edge_dd(&mut g, &e, false)?))
            }
        }),
        _ => Vec::new(),
    }
}

/// (x, y) for `DD::add_f64`: typical pairs; or the edges, a non-finite double, and huge sums
/// that overflow.
#[must_use]
pub fn add_f64(class: Class) -> Vec<(DD, f64)> {
    let mut g = rng("dd::add_f64", class);
    match class {
        Class::Typical => batch_of(|_| Some((typical(&mut g)?, double(&mut g, -30, 30)))),
        Class::Extreme => batch_of(|i| match i % 7 {
            5 => Some((typical(&mut g)?, not_finite(i / 7))),
            6 => {
                let x = dd_in(&mut g, 1023, 1023)?;
                Some((x, x.hi()))
            }
            _ => {
                let e = edge(i);
                Some((edge_dd(&mut g, &e, true)?, edge_f64(&mut g, &e)))
            }
        }),
        _ => Vec::new(),
    }
}

/// (x, y) for `DD::mul_f64`: typical pairs; or the edges (huge times huge overflows, tiny times
/// tiny underflows) and a non-finite double.
#[must_use]
pub fn mul_f64(class: Class) -> Vec<(DD, f64)> {
    let mut g = rng("dd::mul_f64", class);
    match class {
        Class::Typical => batch_of(|_| Some((typical(&mut g)?, double(&mut g, -30, 30)))),
        Class::Extreme => batch_of(|i| match i % 7 {
            5 => Some((typical(&mut g)?, not_finite(i / 7))),
            // A product in the subnormal range, from 2^−1060 to 2^−1040.
            6 => Some((dd_in(&mut g, -530, -520)?, double(&mut g, -530, -520))),
            _ => {
                let e = edge(i);
                Some((edge_dd(&mut g, &e, true)?, edge_f64(&mut g, &e)))
            }
        }),
        _ => Vec::new(),
    }
}

/// (x, y) for `DD::mul`: typical pairs; or the edges.
#[must_use]
pub fn mul(class: Class) -> Vec<(DD, DD)> {
    let mut g = rng("dd::mul", class);
    match class {
        Class::Typical => batch_of(|_| Some((typical(&mut g)?, typical(&mut g)?))),
        Class::Extreme => batch_of(|i| match i % 6 {
            // A product in the subnormal range, from 2^−1060 to 2^−1040.
            5 => Some((dd_in(&mut g, -530, -520)?, dd_in(&mut g, -530, -520)?)),
            _ => {
                let e = edge(i);
                Some((edge_dd(&mut g, &e, true)?, edge_dd(&mut g, &e, false)?))
            }
        }),
        _ => Vec::new(),
    }
}

/// (x, y) for `DD::div_f64`: typical pairs; or a zero or non-finite divisor, a zero dividend, and
/// quotients that overflow (huge over tiny) or underflow (tiny over huge).
#[must_use]
pub fn div_f64(class: Class) -> Vec<(DD, f64)> {
    let mut g = rng("dd::div_f64", class);
    match class {
        Class::Typical => batch_of(|_| Some((typical(&mut g)?, double(&mut g, -30, 30)))),
        Class::Extreme => batch_of(|i| {
            let g = &mut g;
            Some(match i % 7 {
                0 => (typical(g)?, zero(g)),
                1 => (typical(g)?, not_finite(i / 7)),
                2 => (DD::ZERO, double(g, -30, 30)),
                3 => (huge(g)?, subnormal(g)),
                4 => (tiny(g)?, double(g, 1015, 1023)),
                // A quotient in the subnormal range, from 2^−1060 to 2^−1030.
                5 => (dd_in(g, -540, -530)?, double(g, 500, 510)),
                _ => (huge(g)?, double(g, 1015, 1023)),
            })
        }),
        _ => Vec::new(),
    }
}

/// (x, y) for `DD::div`: typical pairs; or a zero divisor, a zero dividend, and quotients that
/// overflow, underflow, or stay near 1 at the ends of the range.
#[must_use]
pub fn div(class: Class) -> Vec<(DD, DD)> {
    let mut g = rng("dd::div", class);
    match class {
        Class::Typical => batch_of(|_| Some((typical(&mut g)?, typical(&mut g)?))),
        Class::Extreme => batch_of(|i| {
            let g = &mut g;
            Some(match i % 7 {
                0 => (typical(g)?, DD::ZERO),
                1 => (DD::ZERO, typical(g)?),
                2 => (huge(g)?, tiny(g)?),
                3 => (tiny(g)?, huge(g)?),
                4 => (huge(g)?, huge(g)?),
                // A quotient in the subnormal range, from 2^−1060 to 2^−1030.
                5 => (dd_in(g, -540, -530)?, dd_in(g, 500, 510)?),
                _ => (tiny(g)?, tiny(g)?),
            })
        }),
        _ => Vec::new(),
    }
}

/// Values for `DD::sqrt`: typical positive pairs; or negative values, zero, and values near both
/// ends of the range, with odd and even exponents.
#[must_use]
pub fn sqrt(class: Class) -> Vec<DD> {
    let mut g = rng("dd::sqrt", class);
    match class {
        Class::Typical => batch_of(|_| typical(&mut g).map(DD::abs)),
        Class::Extreme => batch_of(|i| match i % 6 {
            0 => typical(&mut g).map(|x| x.abs().negate()),
            1 => Some(DD::ZERO),
            2 => dd_in(&mut g, 1022, 1022).map(DD::abs),
            3 => dd_in(&mut g, 1023, 1023).map(DD::abs),
            4 => tiny(&mut g).map(DD::abs),
            _ => DD::from_sum(pow2(-1022), subnormal(&mut g).abs()).ok(),
        }),
        _ => Vec::new(),
    }
}

/// (value, offset) pairs for `DD::round_shifted`: typical values with fractions; the halves, whole
/// numbers, the largest values answered and the ends of `i128`; or values and offsets refused.
#[must_use]
pub fn round_shifted(class: Class) -> Vec<(DD, i128)> {
    let mut g = rng("dd::round_shifted", class);
    let offset = |g: &mut SplitMix64| i128::from(g.next_u64().cast_signed() >> 4);
    match class {
        Class::Typical => batch_of(|_| {
            let x = DD::from_sum(double(&mut g, 0, 59), double(&mut g, -50, -1)).ok()?;
            Some((x, offset(&mut g)))
        }),
        Class::Extreme => batch_of(|i| {
            let g = &mut g;
            // A whole number below 2^52, as a double.
            let k = f64::from(u32::try_from(g.below(1 << 31)).unwrap_or(0));
            Some(match i % 8 {
                // k + 1/2 in the leading part, for even and odd k.
                0 => (DD::from_sum(2.0 * k + 0.5, 0.0).ok()?, offset(g)),
                1 => (DD::from_sum(2.0 * k + 1.5, 0.0).ok()?, offset(g)),
                // A half held in the trailing part: a whole number near 2^60, plus 1/2.
                2 => (DD::from_sum(pow2(60) + 2.0 * k, 0.5).ok()?, offset(g)),
                3 => (DD::from_sum(-(2.0 * k) - 0.5, 0.0).ok()?, offset(g)),
                // A whole number: exact.
                4 => (DD::from_sum(k, 0.0).ok()?, offset(g)),
                // 2^100, the largest leading part answered.
                5 => (DD::from_sum(pow2(100), -0.75).ok()?, 0),
                // Offsets at the ends of i128, with values that keep the result in range.
                6 => (DD::from_sum(k + 0.25, 0.0).ok()?, i128::MAX),
                _ => (DD::from_sum(-k - 0.25, 0.0).ok()?, i128::MIN + 1),
            })
        }),
        Class::Overflow => batch_of(|i| {
            let g = &mut g;
            Some(match i % 2 {
                // Above 2^100.
                0 => (dd_in(g, 101, 1000)?, offset(g)),
                // 2^100 less an offset near −2^127: the result is past 2^127 − 1.
                _ => (DD::from_sum(pow2(100), 0.0).ok()?, i128::MIN + (1 << 99)),
            })
        }),
        _ => Vec::new(),
    }
}

/// (x, y) pairs for the order `DD::cmp`: in rotation, different leading parts, the same leading
/// part with different trailing parts, equal pairs, and opposite signs.
#[must_use]
pub fn cmp(class: Class) -> Vec<(DD, DD)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("dd::cmp", class);
    batch_of(|i| {
        let x = typical(&mut g)?;
        Some(match i % 4 {
            0 => (x, typical(&mut g)?),
            1 => {
                let tail = x.lo() * 0.5;
                (x, DD::from_sum(x.hi(), tail).ok()?)
            }
            2 => (x, x),
            _ => (x, x.negate()),
        })
    })
}
