//! Tests that run, through the crate's public interface, the two arms of `ExactSum` that report a
//! carry propagation the accumulator cannot hold (the coverage gate of section 28.8; lines piece
//! G41 left waiting).
//!
//! The accumulator holds magnitudes below 2^1100 (its module documentation, *No silent overflow*).
//! A sum can be brought to exactly 2^1100 without any step failing: its top chunk then holds
//! 2^63 − 1 and the chunk below it holds a carry of one, still to be propagated. The next carry
//! propagation cannot hold the value, and whichever step makes it must report the sum lost
//! (`round_ieee` gives the NaN with every payload bit set, `round` gives `NumError::NotFinite`),
//! never a finite number or an infinity that the terms after it could turn into a wrong answer:
//!
//! - `merge`, propagating the other sum's chunks, when the value is merged into another sum;
//! - the sum's own propagation, which `merge` makes first and `add` makes once every 2047
//!   additions.
//!
//! No oracle computes 2^1100 here. The test builds the value from powers of two doubled by
//! merging a sum with a copy of itself, and proves what it built by cancellation: the same parts,
//! merged in an order that never reaches 2^1100, give exactly zero with the sum −2^1100.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a failing test should stop loudly"
)]

use geom_num::{ExactSum, NumError};

/// Neal's report of a sum that left the accumulator: a NaN with every payload bit set.
const LOST: u64 = 0x7fff_ffff_ffff_ffff;

/// 2^1023, the largest power of two a double holds.
const TWO_POW_1023: f64 = f64::from_bits(2046 << 52);

/// `x·2^times`, exactly: `x` doubled `times` times by merging the sum with a copy of itself.
fn doubled(x: f64, times: u32) -> ExactSum {
    let mut sum = ExactSum::new();
    sum.add(x);
    for _ in 0..times {
        let copy = sum.clone();
        sum.merge(&copy);
    }
    sum
}

/// `a + b`: a copy of `a` with `b` merged into it.
fn plus(a: &ExactSum, b: &ExactSum) -> ExactSum {
    let mut sum = a.clone();
    sum.merge(b);
    sum
}

/// The sum is reported lost, by both readings.
fn assert_lost(what: &str, sum: &ExactSum) {
    assert_eq!(sum.round_ieee().to_bits(), LOST, "{what}");
    assert_eq!(sum.round(), Err(NumError::NotFinite), "{what}");
}

/// The sum is a finite value too large for a double, and not lost: +∞ from `round_ieee`.
fn assert_overflows(what: &str, sum: &ExactSum) {
    assert_eq!(
        sum.round_ieee().to_bits(),
        f64::INFINITY.to_bits(),
        "{what}"
    );
    assert_eq!(sum.round(), Err(NumError::NotFinite), "{what}");
}

/// The parts: `a` = 2^1099 + 2^1036 and `q` = 2^1099 − 2^1036, each below 2^1100, and `m` =
/// −2^1100. In the accumulator's chunks (the top one weighs 2^1037, the one below it 2^1005), `a`
/// is 2^62 and 2^31, `q` is 2^62 − 1 and 2^31, so `a + q` is 2^63 − 1 and 2^32: exactly 2^1100,
/// one carry short of what the top chunk can hold.
fn parts() -> (ExactSum, ExactSum, ExactSum) {
    let p = doubled(TWO_POW_1023, 76);
    let a = plus(&p, &doubled(TWO_POW_1023, 13));
    let q = plus(&p, &doubled(-TWO_POW_1023, 13));
    let m = doubled(-TWO_POW_1023, 77);
    (a, q, m)
}

/// The parts are what [`parts`] says: each of `a` and `q` is held (an overflow of the double, not
/// a lost sum), and `a + q + m` is exactly zero when merged so that no partial sum reaches 2^1100
/// (`q + m` = −2^1099 − 2^1036 first). So `a + q` = −`m` = 2^1100.
#[test]
fn the_parts_of_two_to_the_1100_are_held_exactly_and_cancel_to_zero() {
    let (a, q, m) = parts();
    assert_overflows("a = 2^1099 + 2^1036", &a);
    assert_overflows("q = 2^1099 - 2^1036", &q);
    assert_overflows("2^1099", &doubled(TWO_POW_1023, 76));
    let zero = plus(&plus(&q, &m), &a);
    assert_eq!(zero.round_ieee().to_bits(), 0, "a + (q + m)");
    assert_eq!(
        zero.round().map(geom_num::Real::get),
        Ok(0.0),
        "a + (q + m)"
    );
    // And in the other association of the same order-free sum, a + m first (−2^1099 + 2^1036).
    let zero = plus(&plus(&a, &m), &q);
    assert_eq!(zero.round_ieee().to_bits(), 0, "(a + m) + q");
}

/// `merge` propagates the other sum's chunks before adding them. Merging the sum of exactly
/// 2^1100 into any other sum fails there, and the result is reported lost: into an empty sum, into
/// a small one, and into one that would cancel it (−2^1100 is the exact negation, so a merge that
/// ignored the failure would return zero or an infinity).
#[test]
fn merging_a_sum_of_two_to_the_1100_into_another_reports_it_lost() {
    let (a, q, m) = parts();
    let s = plus(&a, &q);
    assert_lost("2^1100 itself", &s);

    let mut into_empty = ExactSum::new();
    into_empty.merge(&s);
    assert_lost("0 + 2^1100", &into_empty);

    let mut into_one = ExactSum::new();
    into_one.add(1.0);
    into_one.merge(&s);
    assert_lost("1 + 2^1100", &into_one);

    let mut into_negation = m.clone();
    into_negation.merge(&s);
    assert_lost("-2^1100 + 2^1100", &into_negation);

    // Lost is for good: terms that would cancel the value do not bring a sum back.
    into_empty.merge(&m);
    assert_lost("(0 + 2^1100) - 2^1100", &into_empty);
    into_empty.add(-f64::MAX);
    assert_lost("(0 + 2^1100) - 2^1100 - MAX", &into_empty);
}

/// A sum propagates its own chunks at the start of every `merge`, and in `add` once the 2046
/// additions a merge allows are used. The sum of exactly 2^1100 fails there too, and is reported
/// lost, whatever is merged or added: nothing, zeros, or its exact negation.
#[test]
fn a_sum_of_two_to_the_1100_is_reported_lost_by_its_own_propagation() {
    let (a, q, m) = parts();
    let s = plus(&a, &q);

    let mut merged_with_nothing = s.clone();
    merged_with_nothing.merge(&ExactSum::new());
    assert_lost("2^1100 + 0", &merged_with_nothing);

    let mut merged_with_negation = s.clone();
    merged_with_negation.merge(&m);
    assert_lost("2^1100 - 2^1100", &merged_with_negation);

    // 2046 additions follow a merge without a propagation; the next one propagates first.
    let mut added_to = s.clone();
    for _ in 0..2047 {
        added_to.add(0.0);
    }
    assert_lost("2^1100 + 2047 zeros", &added_to);
    // After that propagation failed the flag keeps the report: the exact negation, merged in
    // through a sum whose own chunks propagate, cannot bring it back.
    added_to.merge(&m);
    assert_lost("2^1100 + 2047 zeros - 2^1100", &added_to);
}
