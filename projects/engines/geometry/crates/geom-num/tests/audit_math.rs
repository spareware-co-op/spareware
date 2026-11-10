//! Independent audit of the correctly rounded maths functions (`geom_num::math`): `exp`, `ln`,
//! `sinpi`, `cospi` and `atan2` against values computed outside this repository.
//!
//! The crate's own tests hold these ports to `tests/math_reference.rs`, a reference written with
//! the ports (big-integer dyadic intervals). The vectors here come from a second reference that
//! shares nothing with it: Python's `decimal` module, whose `exp` and `ln` are correctly rounded
//! at any precision, and Taylor series in 110-digit decimals for the sine, cosine and arctangent
//! (argument reduction in exact fractions; π from Machin's formula). Each expected value is the
//! correctly rounded double of the 110-digit result, kept only when the values 10^−90 (relative)
//! either side of it round to the same double, so the working error cannot matter.
//!
//! The inputs are the regions where a port is most likely to go wrong:
//!
//! - `exp`: results in the subnormal range (x from −745.2 to −708.3) and either side of the
//!   overflow, underflow and subnormal thresholds, tiny arguments, and random ones;
//! - `ln`: the arm this port changed from CORE-MATH (`x ∈ [1 − 2^−8, 1)`, a Taylor series in the
//!   accurate path), every double within 39 steps of 1, `1 ± 2^−j`, subnormal arguments, both
//!   sides of the reduction's switch at √2, and random ones over the whole range;
//! - `sinpi`, `cospi`: subnormal arguments (results rounded on the subnormal grid), every
//!   `k/2 ± 2^−j`, arguments near 2^51 and 2^52 where the doubles are quarter- and half-integers,
//!   and random ones;
//! - `atan2`: quotients that underflow or are subnormal, subnormal operands, operands at the top
//!   of the range, points next to the axes and the diagonals, and random ones in every quadrant.
//!
//! The file `audit_math_vectors.txt` holds one case per line: the function, the bits of each
//! argument, and the bits of the correctly rounded result, or `overflow` where the exact value is
//! 2^1024 or more.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a malformed vector must stop the test loudly"
)]

use geom_num::math;

const VECTORS: &str = include_str!("audit_math_vectors.txt");

/// Every vector is the port's result, bit for bit. `sinpi` and `cospi` return +0 for every zero
/// (the crate's one-zero rule); the reference's zeros are +0 too.
#[test]
fn the_ports_match_an_outside_reference_bit_for_bit() {
    let (mut count, mut wrong) = (0usize, Vec::new());
    for line in VECTORS.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let arg = |i: usize| f64::from_bits(u64::from_str_radix(fields[i], 16).unwrap());
        let got = match fields[0] {
            "exp" => math::exp(arg(1)),
            "ln" => math::ln(arg(1)),
            "sinpi" => math::sinpi(arg(1)),
            "cospi" => math::cospi(arg(1)),
            "atan2" => math::atan2(arg(1), arg(2)),
            other => panic!("unknown function {other}"),
        };
        let want = *fields.last().unwrap();
        let agrees = match (got, want) {
            (Err(_), "overflow") => true,
            (Ok(_), "overflow") | (Err(_), _) => false,
            (Ok(r), bits) => r.get().to_bits() == u64::from_str_radix(bits, 16).unwrap(),
        };
        count += 1;
        if !agrees {
            wrong.push(format!("{line}: got {got:?}"));
        }
    }
    assert!(count > 2_000, "the vectors were not read: {count}");
    assert!(
        wrong.is_empty(),
        "{} of {count} vectors differ:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}
