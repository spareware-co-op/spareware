//! Inputs for the growth checks: the fixed-width integer operations at 2, 4 and 8 limbs (n, 2n
//! and 4n), on operands that fill the width, so each operation runs its full schoolbook loop.
//!
//! Each width has its own seed (`growth::<op>::<limbs>`), so these batches are distinct from the
//! per-width benchmarks of [`super::wide`], whose extreme class they otherwise follow: sums and
//! differences of 64L − 2-bit operands, negations of 64L − 1-bit values and comparisons of
//! 64L − 1-bit values that agree in every limb but the lowest, all of which use the top limb; and
//! products of 32L − 1-bit operands, which use the lower half of the limbs (any wider and the
//! product would not fit) and whose products reach the top limb, so the schoolbook loop runs
//! L/2 × L limb products: still size², as the stated cost needs.
//!
//! Method: the doubling test for empirical growth (run at n, 2n and 4n and compare the ratios with
//! the stated cost; Sedgewick and Wayne, *Algorithms*, 4th edition, section 1.4,
//! <https://algs4.cs.princeton.edu/14analysis/>), on the operand classes of Knuth's schoolbook
//! arithmetic (volume 2, section 4.3.1).
//!
//! Guarantees: at every size, every operand of a sum, difference, negation or comparison uses the
//! top limb, every product reaches it, and no result overflows (`tests/inputs.rs`); every result
//! is checked against exact integers by `tests/attack_g06.rs`.
//!
//! Cost: a few draws of [`crate::rng::SplitMix64`] per input, once per batch, outside every
//! measured run.

use geom_num::wide::Wide;

use super::Class;
use super::wide::{cmp_pairs, mul_pairs, neg_values, sum_pairs};

/// Operand pairs for `checked_add` at `L` limbs.
#[must_use]
pub fn add<const L: usize>() -> Vec<(Wide<L>, Wide<L>)> {
    sum_pairs::<L>(&format!("growth::wide_add::{L}"), Class::Extreme, false)
}

/// Operand pairs for `checked_sub` at `L` limbs.
#[must_use]
pub fn sub<const L: usize>() -> Vec<(Wide<L>, Wide<L>)> {
    sum_pairs::<L>(&format!("growth::wide_sub::{L}"), Class::Extreme, true)
}

/// Values for `checked_neg` at `L` limbs.
#[must_use]
pub fn neg<const L: usize>() -> Vec<Wide<L>> {
    neg_values::<L>(&format!("growth::wide_neg::{L}"), Class::Extreme)
}

/// Pairs for `Ord::cmp` at `L` limbs.
#[must_use]
pub fn cmp<const L: usize>() -> Vec<(Wide<L>, Wide<L>)> {
    cmp_pairs::<L>(&format!("growth::wide_cmp::{L}"), Class::Extreme)
}

/// Operand pairs for `checked_mul` at `L` limbs.
#[must_use]
pub fn mul<const L: usize>() -> Vec<(Wide<L>, Wide<L>)> {
    mul_pairs::<L>(&format!("growth::wide_mul::{L}"), Class::Extreme)
}
