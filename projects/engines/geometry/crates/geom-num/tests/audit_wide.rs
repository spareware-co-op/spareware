//! Independent audit of `geom_num::wide` (the fixed-width integers every exact path runs on).
//!
//! Finding shown here: `Wide::from_i128`, `Wide::from_i64` and `Wide::one` no longer evaluate the
//! `L >= 2` guard, so they compile for one limb (and for none) and return a truncated value with
//! no report. The module's header says every conversion is exact and that nothing wraps silently,
//! `tests/attack_wide_tests.rs` says these constructors cannot be made for `Wide<1>` because
//! "they evaluate the guard", and the Verus twin proves `from_i128` only under `2 <= L`
//! (`verus/wide.rs`), while the header states the proof "for every width `L` from 1".
//!
//! The right behaviour is the one the guard was written for: the call does not compile. A test
//! cannot show a build error, so each test below asks for the only other honest outcome, the exact
//! value, and fails on the truncated one. When the guard is restored these tests stop compiling,
//! which is the fix showing; they are then to be deleted, not repaired.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a broken expectation must stop the test loudly"
)]

use geom_num::{Sign, Wide};

/// 2^64 does not fit one 64-bit limb. `from_i128` is documented "From an `i128`, exactly", so a
/// one-limb result must read back as 2^64; it reads back as 0.
#[test]
fn from_i128_into_one_limb_is_exact_or_does_not_compile() {
    let v: i128 = 1 << 64;
    let w = Wide::<1>::from_i128(v);
    assert_eq!(
        w.to_i128(),
        Some(v),
        "Wide::<1>::from_i128(2^64) silently dropped its high limb: {w:?}"
    );
}

/// The largest `i128` becomes −1 in one limb: a positive value with a negative sign, the very
/// failure the checked arithmetic exists to rule out.
#[test]
fn from_i128_into_one_limb_keeps_the_sign() {
    let w = Wide::<1>::from_i128(i128::MAX);
    assert_eq!(
        w.signum(),
        Sign::Positive,
        "Wide::<1>::from_i128(i128::MAX) is {w:?}, a negative value"
    );
}

/// With no limbs at all every value is zero: `one()` is zero and has the sign of zero.
#[test]
fn one_is_not_zero_at_any_width_it_compiles_for() {
    let one = Wide::<0>::one();
    assert_eq!(one.signum(), Sign::Positive, "Wide::<0>::one() is zero");
}
