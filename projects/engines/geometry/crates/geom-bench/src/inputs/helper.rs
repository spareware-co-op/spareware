//! Inputs for `geom-run`'s parallel helper (decision 32.44): lists of [`LIST`] words, each one call
//! of `Exec::map` whose items fit a single chunk, so its one wave is a lone task.
//!
//! Class `typical` only: every word is a draw of [`SplitMix64`], and every list has the same
//! length, so each call does the same work whatever the words are. The helper's refusals (a
//! budget run out, a cancellation, an error in an item) are tested in `geom-run` itself.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Ok` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the shape of the kernel's own calls that fit one chunk (a mesh's handful of faces, a
//! sort of a few keys): one map over a short list, under an unlimited budget, on the serial
//! executor and on a pool. Each is measured as chapter 28.14 sets benchmarks out,
//! `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: [`LIST`] draws of [`SplitMix64`] per input, once per batch, outside every measured run.

use super::{Class, batch_of, rng};

/// Words per list: below the kernels' chunk size, so each list is one chunk.
pub const LIST: usize = 16;

/// Lists of [`LIST`] words for the operation named `op`.
fn lists(op: &str, class: Class) -> Vec<Vec<u64>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng(op, class);
    batch_of(|_| Some((0..LIST).map(|_| g.next_u64()).collect()))
}

/// Lists for `Exec::map` on the serial executor.
#[must_use]
pub fn lone_serial(class: Class) -> Vec<Vec<u64>> {
    lists("geom_run::Exec::map serial", class)
}

/// Lists for `Exec::map` on a pool.
#[must_use]
pub fn lone_pool(class: Class) -> Vec<Vec<u64>> {
    lists("geom_run::Exec::map pool", class)
}
