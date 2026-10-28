//! A Verus-annotated twin of `geom-num`'s `src/sign.rs`, the sign every exact predicate returns
//! (chapter 28.7 of the kernel architecture; piece G39), for the twins of the plane and crossing
//! code to use.
//!
//! It proves that `of_i128` gives the sign of its argument and that `mul` follows the sign rule of
//! multiplication. `mul`, below the `verus!` block, with its `#[verus_spec]` attribute removed, is
//! token for token the shipped one; `of_i128`, an associated function without `self`, which
//! `#[verus_spec]` cannot annotate, is written inside the block, and `geom-gen`'s
//! `tests/verus_twin.rs` compares its doc comment, attributes, parameters and body. Also inside the
//! block: a model of `Sign` with the same variants, the sign of an integer, and a sign's value.
//!
//! It is a module of `geom-num`'s Verus crate, `verus/lib.rs`. Verify: `verus --crate-type=lib
//! projects/engines/geometry/crates/geom-num/verus/lib.rs`, with the Verus release `kernel-dev.yml`
//! pins; its `verus` job does this on every pull request.

use vstd::prelude::*;

verus! {

/// The sign of an exactly computed quantity.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sign {
    /// Strictly less than zero.
    Negative,
    /// Exactly zero.
    Zero,
    /// Strictly greater than zero.
    Positive,
}

/// The sign of an integer.
pub open spec fn sign_of(v: int) -> Sign {
    if v < 0 {
        Sign::Negative
    } else if v > 0 {
        Sign::Positive
    } else {
        Sign::Zero
    }
}

/// A sign as a number: −1, 0 or 1.
pub open spec fn sign_value(s: Sign) -> int {
    match s {
        Sign::Negative => -1,
        Sign::Zero => 0,
        Sign::Positive => 1,
    }
}

/// The product of two signs.
pub open spec fn sign_mul(a: Sign, b: Sign) -> Sign {
    sign_of(sign_value(a) * sign_value(b))
}

/// The sign of a product is the product of the signs.
pub proof fn lemma_sign_of_product(x: int, y: int)
    ensures
        sign_of(x * y) == sign_of(sign_value(sign_of(x)) * sign_value(sign_of(y))),
{
    assert(x > 0 && y > 0 ==> x * y > 0) by (nonlinear_arith);
    assert(x < 0 && y < 0 ==> x * y > 0) by (nonlinear_arith);
    assert(x > 0 && y < 0 ==> x * y < 0) by (nonlinear_arith);
    assert(x < 0 && y > 0 ==> x * y < 0) by (nonlinear_arith);
    assert(x == 0 || y == 0 ==> x * y == 0) by (nonlinear_arith);
}

impl Sign {
    /// The sign of an `i128`.
    #[must_use]
    pub const fn of_i128(v: i128) -> (r: Sign)
        ensures
            r == sign_of(v as int),
    {
        if v < 0 {
            Sign::Negative
        } else if v > 0 {
            Sign::Positive
        } else {
            Sign::Zero
        }
    }
}

} // verus!

impl Sign {
    /// The sign of a product of two quantities with these signs.
    #[must_use]
    #[verus_spec(r =>
        ensures
            r == sign_of(sign_value(self) * sign_value(other)),
    )]
    pub const fn mul(self, other: Sign) -> Sign {
        match (self, other) {
            (Sign::Zero, _) | (_, Sign::Zero) => Sign::Zero,
            (Sign::Positive, Sign::Positive) | (Sign::Negative, Sign::Negative) => Sign::Positive,
            _ => Sign::Negative,
        }
    }
}
