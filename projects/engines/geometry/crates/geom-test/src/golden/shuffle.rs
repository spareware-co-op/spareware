//! The input-shuffle hook: a case whose contract says the order of its inputs does not matter is
//! run on its inputs in their own order and in seeded shuffled orders, and every run must give the
//! same canonical output, bit for bit.
//!
//! What it does: [`Orders`] makes seeded permutations; [`order_free`] runs a case on its inputs in
//! the given order, in the reverse order and in `shuffles` shuffled orders, and returns the one
//! digest they all give, or an [`OrderError`] naming the first permutation that gave another.
//!
//! Chapter 28.13 lists, among the perturbations that must change nothing, "shuffled inputs,
//! wherever an operation's contract says order does not matter"; chapter 23.4 routes every float
//! reduction whose order could vary through `ExactSum`, whose result "depends on the multiset of
//! terms only", and `geom-num`'s order-free golden holds it to that through this hook.
//!
//! Guarantees:
//!
//! - **Seeded and portable.** A seed gives the same permutations on every target, 32-bit and
//!   64-bit: the shuffle is `rand`'s Fisher-Yates over [`SplitMix64`], which draws each index as a
//!   32-bit value for slices shorter than 2^32 elements, so the word stream and the permutation do
//!   not depend on the width of `usize`. A golden in the tests pins the first permutations.
//! - **Every permutation can come.** Durstenfeld's form of the Fisher-Yates shuffle with unbiased
//!   bounded draws gives each of the n! permutations of n inputs with equal probability, so a
//!   failure is not limited to orders a hand-written list happened to try.
//! - **The given order is always run first, then its reverse**, so a case that fails in its own
//!   order fails as such, and every case of two or more inputs meets at least one other order
//!   whatever the seed: for two inputs, the reverse is the only other order, so both are run.
//! - **No panic.** An error names the permutation and both digests.
//!
//! Method: R. Durstenfeld, "Algorithm 235: Random permutation", *Communications of the ACM* 7(7),
//! 1964, <https://doi.org/10.1145/364520.364540>, the modern form of the Fisher-Yates shuffle
//! (Knuth, *The Art of Computer Programming*, volume 2, section 3.4.2, Algorithm P), as `rand`'s
//! `SliceRandom::shuffle` implements it (a dependency, MIT OR Apache-2.0), driven by this crate's
//! [`SplitMix64`].
//!
//! Cost: one shuffle is linear in the number of inputs; [`order_free`] runs the case `shuffles + 2`
//! times (once for a single input or none) and copies the inputs once per run.
//!
//! Consulted: rand, MIT OR Apache-2.0,
//!   <https://github.com/rust-random/rand/blob/9e7d328f60d6372ae22927f37d1a816fc980f25e/src/seq/slice.rs>,
//!   to confirm that its shuffle draws 32-bit indices below 2^32 elements, which makes it portable.
//!   No code copied.

use core::convert::Infallible;
use core::fmt;

use rand::TryRng;
use rand::seq::SliceRandom;

use super::sha256::Digest;
use crate::rng::SplitMix64;
use crate::seed::Seed;

/// `rand`'s generator interface for this crate's `SplitMix64`, so `rand`'s shuffle draws from it.
/// A 32-bit draw is the high half of the next word, the better-mixed half.
impl TryRng for SplitMix64 {
    type Error = Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Infallible> {
        let [a, b, c, d, ..] = self.next_u64().to_be_bytes();
        Ok(u32::from_be_bytes([a, b, c, d]))
    }

    fn try_next_u64(&mut self) -> Result<u64, Infallible> {
        Ok(self.next_u64())
    }

    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Infallible> {
        self.fill_bytes(dst);
        Ok(())
    }
}

/// Seeded permutations.
#[derive(Clone, Debug)]
pub struct Orders(SplitMix64);

impl Orders {
    /// Permutations from `seed`.
    #[must_use]
    pub const fn new(seed: Seed) -> Orders {
        Orders(SplitMix64::new(seed))
    }

    /// Shuffles `items` in place.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        items.shuffle(&mut self.0);
    }

    /// A permutation of `0..n`: `p[i]` is the index of the input placed `i`-th.
    #[must_use]
    pub fn permutation(&mut self, n: usize) -> Vec<usize> {
        let mut p: Vec<usize> = (0..n).collect();
        self.shuffle(&mut p);
        p
    }
}

/// A run of an order-free case whose output differs from the run in the given order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderError {
    /// The case's name.
    pub label: String,
    /// The permutation that differed: `permutation[i]` is the input placed `i`-th.
    pub permutation: Vec<usize>,
    /// The digest in the given order.
    pub given: Digest,
    /// The digest in the permuted order.
    pub permuted: Digest,
}

impl fmt::Display for OrderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: the contract says the order of the inputs does not matter, but the inputs in the \
             order {:?} give {} where the given order gives {}",
            self.label, self.permutation, self.permuted, self.given
        )
    }
}

impl core::error::Error for OrderError {}

/// Runs `case` on `inputs` in their given order, then reversed (for two or more inputs), then in
/// `shuffles` orders from `seed`, and returns the digest every run gave.
///
/// # Errors
///
/// [`OrderError`] for the first permutation whose digest differs from the given order's.
pub fn order_free<T: Clone>(
    label: &str,
    inputs: &[T],
    seed: Seed,
    shuffles: usize,
    mut case: impl FnMut(&[T]) -> [u8; 32],
) -> Result<[u8; 32], OrderError> {
    let given = case(inputs);
    let mut orders = Orders::new(seed);
    let reverse: Vec<usize> = (0..inputs.len()).rev().collect();
    let first = (inputs.len() >= 2).then_some(reverse);
    let seeded = (0..shuffles).map(|_| orders.permutation(inputs.len()));
    for permutation in first.into_iter().chain(seeded) {
        let permuted: Vec<T> = permutation
            .iter()
            .filter_map(|&i| inputs.get(i).cloned())
            .collect();
        let digest = case(&permuted);
        if digest != given {
            return Err(OrderError {
                label: label.to_owned(),
                permutation,
                given: Digest(given),
                permuted: Digest(digest),
            });
        }
    }
    Ok(given)
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "tests may use native arithmetic and casts"
)]
mod tests {
    use std::collections::BTreeSet;

    use super::super::sha256::Sha256;
    use super::*;

    #[test]
    fn permutations_are_permutations_and_follow_the_seed() {
        let mut a = Orders::new(Seed(1));
        let mut b = Orders::new(Seed(1));
        for n in [0usize, 1, 2, 3, 10, 257] {
            let p = a.permutation(n);
            assert_eq!(p, b.permutation(n));
            let mut sorted = p.clone();
            sorted.sort_unstable();
            assert_eq!(sorted, (0..n).collect::<Vec<_>>());
        }
        assert_ne!(
            Orders::new(Seed(1)).permutation(10),
            Orders::new(Seed(2)).permutation(10)
        );
    }

    /// Every permutation of three inputs comes, so the shuffle is not stuck in a subset.
    #[test]
    fn every_permutation_of_three_comes() {
        let mut o = Orders::new(Seed(3));
        let seen: BTreeSet<Vec<usize>> = (0..200).map(|_| o.permutation(3)).collect();
        assert_eq!(seen.len(), 6);
    }

    /// The permutations a seed gives are pinned, in the golden store as
    /// `geom-test/shuffle-permutations`, so a change to rand's shuffle or to `SplitMix64` that would
    /// change every order-free golden's inputs shows here first, by name.
    #[test]
    fn the_permutation_stream_is_pinned() {
        let checked = super::super::check("geom-test/shuffle-permutations", || {
            let mut o = Orders::new(Seed::DEFAULT);
            let mut h = Sha256::new();
            for n in 0..64usize {
                for i in o.permutation(n) {
                    h.write(&u64::try_from(i).unwrap().to_le_bytes());
                }
            }
            (h.finish(), ())
        });
        checked.verdict().unwrap_or_else(|e| panic!("{e}"));
    }

    /// With two inputs and no shuffles at all, both orders still run.
    #[test]
    fn two_inputs_always_meet_both_orders() {
        let seen = std::cell::RefCell::new(Vec::new());
        let r = order_free("pair", &[1u8, 2], Seed(0), 0, |xs: &[u8]| {
            seen.borrow_mut().push(xs.to_vec());
            [0; 32]
        });
        assert!(r.is_ok());
        assert_eq!(*seen.borrow(), [vec![1, 2], vec![2, 1]]);
        let one = order_free("one", &[1u8], Seed(0), 3, |_: &[u8]| [0; 32]);
        assert!(one.is_ok());
    }

    #[test]
    fn order_free_passes_a_symmetric_case_and_names_the_order_that_breaks_an_asymmetric_one() {
        let inputs: Vec<u64> = (0..20).collect();
        let sum = |xs: &[u64]| {
            let mut h = Sha256::new();
            h.write(
                &xs.iter()
                    .fold(0u64, |a, x| a.wrapping_add(*x))
                    .to_le_bytes(),
            );
            h.finish()
        };
        assert!(order_free("sum", &inputs, Seed(5), 16, sum).is_ok());
        let first = |xs: &[u64]| {
            let mut h = Sha256::new();
            h.write(&xs.first().copied().unwrap_or(0).to_le_bytes());
            h.finish()
        };
        let e = order_free("first", &inputs, Seed(5), 16, first).unwrap_err();
        assert_eq!(
            e.permutation,
            (0..20).rev().collect::<Vec<_>>(),
            "the reverse runs first"
        );
        assert!(e.to_string().starts_with("first: the contract says"));
        assert_ne!(e.given, e.permuted);
    }
}
