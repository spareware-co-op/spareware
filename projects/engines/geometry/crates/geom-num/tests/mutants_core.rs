//! Tests that kill the mutants cargo-mutants found surviving in `error`, `grid` and `real` (piece
//! G21, the mutation gate of section 28.8), through the crate's public interface only.
//!
//! Each test names the mutant it kills, in cargo-mutants' words. The mutants survived because no
//! integration test looked at what they change: the text of an error, the constant `Coord::MIN`,
//! and `Real`'s equality, hashing and partial order, which callers reach through `==`, `<`,
//! `Hash` and `PartialOrd` rather than through `Real`'s own methods.

// The crate denies native arithmetic in kernel code (Cargo.toml); tests may use it to state the
// values they expect.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "expected values; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a failing test should stop loudly"
)]

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

use geom_num::grid::COORD_MAX;
use geom_num::{Coord, NumError, Real};

/// Kills `replace <impl fmt::Display for NumError>::fmt -> fmt::Result with Ok(Default::default())`
/// (every message would be empty) and pins every message, so a deleted or swapped arm shows.
#[test]
fn every_error_has_its_message() {
    let cases = [
        (
            NumError::Degenerate,
            "the inputs are degenerate for this construction",
        ),
        (
            NumError::NotFinite,
            "a floating-point value was NaN or infinite",
        ),
        (
            NumError::WrongDirection,
            "grid-step conversion asked to go the wrong way",
        ),
        (
            NumError::DivisorContainsZero,
            "interval division by an interval that contains zero",
        ),
        (
            NumError::NegativeSqrt,
            "interval square root of negative values",
        ),
        (
            NumError::EmptyInterval,
            "interval lower bound is above its upper bound",
        ),
        (
            NumError::OutsideDomain,
            "maths function argument outside the function's domain",
        ),
        (
            NumError::BudgetExceeded,
            "an exact computation exceeded its proven bit budget (a defect in geom-num)",
        ),
        (
            NumError::ToleranceExceeded,
            "a bound would exceed its tolerance ceiling",
        ),
        (
            NumError::UnprovenMaths,
            "an enclosure needs a maths function that has no certified enclosure yet",
        ),
    ];
    for (error, text) in cases {
        assert_eq!(error.to_string(), text, "{error:?}");
    }
}

/// Kills `delete match arm i128::MAX` and `delete match arm i128::MIN` in `NumError`'s `fmt`: a
/// saturated coordinate stands for that value or beyond, and the message must say "at least" or
/// "at most"; any other value is stated exactly, with no qualifier.
#[test]
fn saturated_coordinates_are_qualified_in_the_message() {
    let text = |value| NumError::OutOfRange { value }.to_string();
    let tail = " grid steps is outside the frame's range of ±(2^40 − 1) steps";
    assert_eq!(
        text(i128::MAX),
        format!("coordinate at least {}{tail}", i128::MAX)
    );
    assert_eq!(
        text(i128::MIN),
        format!("coordinate at most {}{tail}", i128::MIN)
    );
    for v in [1 << 40, -(1 << 40), i128::MAX - 1, i128::MIN + 1] {
        assert_eq!(text(v), format!("coordinate {v}{tail}"));
    }
}

/// Kills `delete -` in `pub const MIN: Coord = Coord(-COORD_MAX);`: the smallest coordinate is
/// −(2^40 − 1), the one `Coord::new` accepts last.
#[test]
fn the_smallest_coordinate_is_minus_coord_max() {
    assert_eq!(Coord::MIN.get(), -((1i64 << 40) - 1));
    assert_eq!(Coord::MIN.get(), -COORD_MAX);
    assert_eq!(Coord::new(-COORD_MAX), Ok(Coord::MIN));
    assert!(Coord::new(-COORD_MAX - 1).is_err());
    assert!(Coord::MIN < Coord::ZERO && Coord::ZERO < Coord::MAX);
    assert_eq!(Coord::MIN.get(), -Coord::MAX.get());
}

fn real(v: f64) -> Real {
    Real::new(v).unwrap()
}

/// Kills `replace <impl PartialEq for Real>::eq -> bool with true`, `... with false` and
/// `replace == with !=`: equal values are equal, different values are not, and −0 and +0 are two
/// values.
#[test]
fn reals_are_equal_exactly_when_their_bits_are() {
    let values = [0.0, -0.0, 1.0, -1.0, f64::from_bits(1), f64::MAX, 0.1 + 0.2];
    for (i, &a) in values.iter().enumerate() {
        for (j, &b) in values.iter().enumerate() {
            assert_eq!(real(a) == real(b), i == j, "{a:e} and {b:e}");
            assert_eq!(real(a) != real(b), i != j, "{a:e} and {b:e}");
        }
    }
}

/// A hasher that records every word it is given, in order.
#[derive(Default)]
struct Recorder(Vec<u64>);

impl Hasher for Recorder {
    fn finish(&self) -> u64 {
        self.0.iter().fold(0, |h, w| h.rotate_left(7) ^ w)
    }

    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.0.push(u64::from_le_bytes(word));
        }
    }

    fn write_u64(&mut self, i: u64) {
        self.0.push(i);
    }
}

/// Kills `replace <impl Hash for Real>::hash with ()`: a `Real` hashes its bits (its documented
/// contract), so equal values hash equally and −0 and +0, which are different `Real`s, feed the
/// hasher different words.
#[test]
fn reals_hash_their_bits() {
    for v in [0.0, -0.0, 1.5, -2.0, f64::MIN_POSITIVE, f64::MAX] {
        let mut h = Recorder::default();
        real(v).hash(&mut h);
        assert_eq!(h.0, vec![v.to_bits()], "{v:e}");
    }
    let mut a = Recorder::default();
    let mut b = Recorder::default();
    real(0.0).hash(&mut a);
    real(-0.0).hash(&mut b);
    assert_ne!(a.finish(), b.finish());
}

/// Kills `replace <impl PartialOrd for Real>::partial_cmp -> Option<Ordering> with None`: `Real`
/// is totally ordered, so `partial_cmp` always answers, and `<` and `>` follow IEEE totalOrder
/// (−0 before +0).
#[test]
fn reals_are_totally_ordered_through_partial_ord() {
    let sorted = [
        f64::MIN,
        -1.0,
        -f64::from_bits(1),
        -0.0,
        0.0,
        f64::from_bits(1),
        1.0,
        f64::MAX,
    ];
    for (i, &a) in sorted.iter().enumerate() {
        for (j, &b) in sorted.iter().enumerate() {
            let want = i.cmp(&j);
            assert_eq!(real(a).partial_cmp(&real(b)), Some(want), "{a:e} and {b:e}");
            assert_eq!(real(a) < real(b), want == Ordering::Less, "{a:e} < {b:e}");
            assert_eq!(
                real(a) > real(b),
                want == Ordering::Greater,
                "{a:e} > {b:e}"
            );
        }
    }
}
