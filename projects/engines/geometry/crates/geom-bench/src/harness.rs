//! One benchmark ready to run: its batch, a result buffer and its kernel, run the same way by the
//! instruction-count gate, the WebAssembly fuel gate, the wall-clock harness and the golden tests.
//!
//! [`Bound`] holds a batch of inputs and a result buffer with room for one result per input, so a
//! [`Bench::pass`] (one run of the kernel over the batch) allocates nothing and frees nothing.
//! Hashing the inputs and results ([`Bench::digests`]) is separate and never part of a measured
//! run: the gate checks the digests after measuring, outside the counted region.
//!
//! # The counted region (instruction-count gate)
//!
//! [`Bench::measure`] runs [`Bench::pass`] twice. Each pass starts by calling [`zero_costs_here`],
//! and the gate runs Callgrind with `--zero-before=*zero_costs_here*`, which discards every cost
//! collected so far each time that function is entered (Valgrind's Callgrind manual,
//! <https://valgrind.org/docs/manual/cl-manual.html#cl-manual.options.activity>). So the counts are
//! those of the second pass alone, from the start of its kernel call to the end of the benchmark
//! function. Everything that pass executes was executed by the first pass at the same addresses and
//! the same stack depth, and it touches the same data, so the simulated caches are warm and no count
//! depends on where the linker placed code or where the allocator placed data:
//!
//! - the kernel, the `geom-num` code it calls and [`zero_costs_here`] ran in the first pass;
//! - the batch and the result buffer are the ones the first pass used, and the second pass runs at
//!   the same or a shallower stack depth (`measure` may hand over to it by a tail call), so its
//!   stack frames lie in stack the first pass used;
//! - the only code run once, the returns of `measure` and of the benchmark function, sits in
//!   functions shorter than one 64-byte cache line, which the gate's build aligns to cache lines
//!   (`-C llvm-args=-align-all-functions=6`), so those lines were fetched when the functions were
//!   entered.
//!
//! Guarantees: a pass allocates and frees nothing (the result buffer already holds one result per
//! input; the tests check its capacity never changes), every pass over a batch stores the same
//! results, and the digests always describe the batch and the results of the last pass.
//!
//! Method: the warm-cache, steady-state measurement of Callgrind's cache simulation, whose
//! simulator keeps running while costs are not collected (Valgrind's Callgrind manual,
//! <https://valgrind.org/docs/manual/cl-manual.html>, and its source, `callgrind/sim.c`), with the
//! warm-up inside the measured function so it covers that function too.
//!
//! Cost: a pass is one kernel call over the batch and allocates nothing; [`Bench::measure`] is two
//! passes; [`Bench::digests`] hashes the batch and the results, linear in their size.

use core::fmt;
use core::hint::black_box;

use crate::canonical::{Canonical, digest};
use crate::sha256::Digest;

/// A kernel: runs one operation on every input of a batch, in order, and appends each result to
/// the buffer (which it clears first). See [`crate::kernels`].
pub type Kernel<T, R> = fn(&[T], &mut Vec<R>);

/// The golden digests of one benchmark: SHA-256 of the canonical encodings of its inputs and of its
/// results ([`crate::canonical`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digests {
    /// The digest of every input of the batch, in order.
    pub inputs: Digest,
    /// The digest of every result of the last pass, in order.
    pub outputs: Digest,
}

impl fmt::Display for Digests {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.inputs, self.outputs)
    }
}

/// A benchmark ready to run, whatever its input and result types.
pub trait Bench {
    /// How many inputs the batch holds.
    fn len(&self) -> usize;

    /// Whether the batch is empty, which no catalogued benchmark's is.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// One run of the kernel over the batch into the result buffer. Allocates nothing.
    fn pass(&mut self);

    /// The instruction-count gate's measurement: two passes, the first warming the caches, with
    /// Callgrind's costs zeroed at the start of each (see the module documentation).
    fn measure(&mut self);

    /// The digests of the inputs and of the results of the last pass.
    fn digests(&self) -> Digests;

    /// One pass, then the digests.
    fn run(&mut self) -> Digests {
        self.pass();
        self.digests()
    }
}

/// A batch bound to its kernel, with a result buffer that holds one result per input.
pub struct Bound<T, R> {
    /// The inputs.
    batch: Vec<T>,
    /// The results of the last pass.
    out: Vec<R>,
    /// The measured function.
    kernel: Kernel<T, R>,
}

impl<T, R> Bound<T, R> {
    /// A batch bound to `kernel`, with room for one result per input. No pass has run yet.
    #[must_use]
    pub fn new(batch: Vec<T>, kernel: Kernel<T, R>) -> Bound<T, R> {
        let out = Vec::with_capacity(batch.len());
        Bound { batch, out, kernel }
    }

    /// The inputs.
    #[must_use]
    pub fn batch(&self) -> &[T] {
        &self.batch
    }

    /// The results of the last pass (empty before the first).
    #[must_use]
    pub fn outputs(&self) -> &[R] {
        &self.out
    }
}

impl<T: Canonical, R: Canonical> Bench for Bound<T, R> {
    fn len(&self) -> usize {
        self.batch.len()
    }

    #[inline(never)]
    fn pass(&mut self) {
        zero_costs_here();
        (self.kernel)(&self.batch, &mut self.out);
    }

    #[inline(never)]
    fn measure(&mut self) {
        self.pass();
        self.pass();
    }

    fn digests(&self) -> Digests {
        Digests {
            inputs: digest(&self.batch),
            outputs: digest(&self.out),
        }
    }
}

/// A benchmark and its name, as the instruction-count gate holds it: the gate measures `bench`
/// and then checks its digests against the golden ones recorded under `name`.
pub struct Named {
    /// The benchmark's name, `group/op/class`.
    pub name: &'static str,
    /// The benchmark.
    pub bench: Box<dyn Bench>,
}

/// Marks the start of a pass. The instruction-count gate runs Callgrind with
/// `--zero-before=*zero_costs_here*`, so entering this function discards every cost collected so far.
/// Elsewhere it does nothing. Never inlined, and its body is opaque to the optimiser, so every pass
/// really calls it.
#[inline(never)]
pub fn zero_costs_here() {
    black_box(());
}

/// The Callgrind option that makes [`zero_costs_here`] zero the costs (the gate passes it).
pub const ZERO_BEFORE: &str = "--zero-before=*zero_costs_here*";

#[cfg(test)]
mod tests {
    use super::*;

    fn double(batch: &[i64], out: &mut Vec<i64>) {
        out.clear();
        out.extend(batch.iter().map(|x| x * 2));
    }

    #[test]
    fn a_pass_fills_the_buffer_without_growing_it() {
        let mut b = Bound::new(vec![1, 2, 3], double as Kernel<i64, i64>);
        let capacity = b.out.capacity();
        assert!(capacity >= 3);
        b.measure();
        assert_eq!(b.outputs(), &[2, 4, 6]);
        b.pass();
        assert_eq!(b.outputs(), &[2, 4, 6]);
        assert_eq!(b.out.capacity(), capacity);
    }

    #[test]
    fn digests_cover_inputs_and_outputs() {
        let mut a = Bound::new(vec![1i64, 2, 3], double as Kernel<i64, i64>);
        let mut b = Bound::new(vec![1i64, 2, 4], double as Kernel<i64, i64>);
        let (da, db) = (a.run(), b.run());
        assert_ne!(da.inputs, db.inputs);
        assert_ne!(da.outputs, db.outputs);
        assert_eq!(a.digests(), da);
        assert!(ZERO_BEFORE.contains("zero_costs_here"));
    }
}
