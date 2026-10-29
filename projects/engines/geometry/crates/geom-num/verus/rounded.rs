//! A Verus-annotated twin of the range check the generated predicates apply to a rounded input,
//! `within` in `geom-num`'s `src/generated/mod.rs` (chapter 28.7 of the kernel architecture; piece
//! G39), for the predicate twins with rounded inputs to call.
//!
//! It proves that `within(v, bits)` returns `Some(v)` exactly when |v| ≤ 2^bits − 1 (`in_range`,
//! with 2^bits written as `1u128 << bits`), and `None` otherwise, for every `bits` up to 127. The
//! function below the `verus!` block, with its `#[verus_spec]` attribute and its `proof!` blocks
//! removed and written public so that the predicate twins can call it, is token for token the
//! shipped one: `geom-gen`'s `tests/verus_twin.rs` compares them. Inside the block is one more fact
//! Verus is told about the standard library, `u128::checked_shl`'s documented behaviour (the
//! shift, or `None` for a shift of 128 or more).
//!
//! It is a module of `geom-num`'s Verus crate, `verus/lib.rs`. Verify: `verus --crate-type=lib
//! projects/engines/geometry/crates/geom-num/verus/lib.rs`, with the Verus release `kernel-dev.yml`
//! pins; its `verus` job does this on every pull request.

use vstd::prelude::*;

verus! {

pub assume_specification[ u128::checked_shl ](x: u128, n: u32) -> (r: Option<u128>)
    ensures
        r == if n < 128 { Some(x << n) } else { None::<u128> },
;

/// Whether v lies in the range a description declares for a rounded input of `bits` bits:
/// |v| < 2^bits, that is |v| ≤ 2^bits − 1.
pub open spec fn in_range(v: i128, bits: u32) -> bool {
    -((1u128 << bits) as int) < v as int && (v as int) < (1u128 << bits) as int
}

} // verus!

/// `v` itself if |v| ≤ 2^bits − 1, the range a description declares for a rounded input (bits
/// at most 127), within which its certificate and its error analysis hold; otherwise `None`.
#[inline]
#[verus_spec(r =>
    requires
        bits <= 127,
    ensures
        match r {
            Some(w) => w == v && in_range(v, bits),
            None => !in_range(v, bits),
        },
)]
pub fn within(v: i128, bits: u32) -> Option<i128> {
    proof! {
        assert((1u128 << bits) != 0u128) by (bit_vector)
            requires
                bits <= 127u32,
        ;
    }
    let limit = 1u128.checked_shl(bits)?.checked_sub(1)?;
    if v.unsigned_abs() <= limit {
        Some(v)
    } else {
        None
    }
}
