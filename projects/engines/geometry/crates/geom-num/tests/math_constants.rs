//! The maths oracle's stored constants are the values their series give: π by Machin's formula,
//! ln 2 as 2·atanh(1/3), and atan(j/8) for j from 1 to 8 by Euler's series, each to
//! `CONST_BITS` bits.
//!
//! The oracle (`math_reference.rs`) stores them as constants because a crate that ships holds no
//! `static`, even in a test build (the merge gate, G12), and computing them on every call would
//! cost thousands of big-number steps each time. This test computes each once and compares it with
//! what is stored, end for end and bit for bit, so the data cannot drift from its derivation.

#[path = "math_reference.rs"]
mod reference;

use reference::{Iv, atan_eighths, atan_eighths_series, ln2, ln2_series, pi, pi_series};

/// Asserts two intervals have the same ends, significand and exponent alike.
fn assert_same(what: &str, stored: &Iv, series: &Iv) {
    assert_eq!(stored.lo.m, series.lo.m, "{what}: low end's significand");
    assert_eq!(stored.lo.e, series.lo.e, "{what}: low end's exponent");
    assert_eq!(stored.hi.m, series.hi.m, "{what}: high end's significand");
    assert_eq!(stored.hi.e, series.hi.e, "{what}: high end's exponent");
}

#[test]
fn stored_constants_are_their_series() {
    assert_same("pi", &pi(), &pi_series());
    assert_same("ln 2", &ln2(), &ln2_series());
    for (j, series) in (1..=8).zip(&atan_eighths_series()) {
        assert_same(&format!("atan({j}/8)"), &atan_eighths(j), series);
    }
}
