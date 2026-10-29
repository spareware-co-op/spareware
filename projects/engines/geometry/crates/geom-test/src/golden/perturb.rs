//! The perturbing allocator: a seeded shift and fragmentation of the heap before a golden case runs,
//! so that any result that depends on where memory lands changes from run to run and is caught.
//!
//! What it does: [`Heap::shift_and_fragment`] drives the program's own global allocator from a
//! [`Seed`] into a layout no other seed gives, and holds it there until the [`Heap`] is dropped:
//!
//! 1. **Shift.** It pins blocks of seeded, uneven sizes, so the allocator's frontier and the
//!    alignment of what comes next move by a seeded amount.
//! 2. **Fragment and shuffle.** For each of [`CLASSES`] block sizes, from one word to 64 KiB, it
//!    allocates a seeded number of blocks, then frees a seeded share of them in a seeded order and
//!    pins the rest. The freed blocks leave holes between pinned ones, and an allocator that reuses
//!    freed blocks of a size class from a list (glibc's tcache and fast bins and the dlmalloc that
//!    Rust's WebAssembly targets use take the last freed first; Windows' low-fragmentation heap
//!    picks at random) hands the next allocations of that size in the case out at shuffled
//!    addresses.
//! 3. **Box the small classes.** For each of [`BOX_SIZES`], the classes up to 128 bytes, it does the
//!    same with boxes of that many uninitialised bytes, made by `Box::new` as any value is boxed.
//!    Where an allocator keeps one list per size class whatever code allocates, this only adds to
//!    step 2; macOS's allocator on arm64 keeps them apart (the guarantees say how).
//!
//! Chapter 28.13 asks for "a test allocator that shifts and fragments memory, so any ordering by
//! address shows up", and chapter 21 for "tests that shuffle allocation" behind its rule that ties
//! never break by memory address. [`super::check`] runs every golden case twice, each time under a
//! different seed's layout; a case whose output depends on addresses gives two digests there, or a
//! digest that differs from the store's on some target.
//!
//! Why the global allocator is driven rather than replaced: replacing it means implementing
//! `GlobalAlloc`, an `unsafe` trait, and principle 13 ("Safe Rust. No `unsafe`") holds for every
//! crate, this one included (`#![forbid(unsafe_code)]`). No published allocator fills the gap within
//! the rules: `shuffling-allocator` (the one crate that shuffles heap placement) seeds itself from
//! the operating system's entropy, which this crate never reads (28.17), has no lock for WebAssembly
//! (it stops the build with `compile_error!` there) and is MPL-2.0 code with `unsafe` in it. Driving
//! the allocator from outside gives the same two effects, shifted addresses and shuffled reuse, with
//! no `unsafe`, and the tests prove the effect rather than assume it on every target CI runs them
//! on: `kernel.yml`'s six native runners (the allocators of glibc, macOS and Windows) and
//! `kernel-dev.yml`'s wasm32-wasip1 leg (the dlmalloc of Rust's WebAssembly targets). The same
//! answer under one seed twice in a row is not required (other threads' allocations perturb it
//! further, which only helps).
//!
//! What the perturbation reaches depends on the allocator. The last three guarantees give each of
//! CI's, with what CI measured for issue #370 (run 40823425292 measured this module; runs
//! 40807005781, 40813238936, 40816535727 and 40819713151 measured the module before step 3 and the
//! ways tried and set aside). The measure is a sweep: sixteen seeds, each perturbing the heap
//! before 64 values are boxed and sorted by address ([`address_order_digest`]), on one thread alone
//! and among busy tests; a sweep counts the different orders it sees.
//!
//! Guarantees:
//!
//! - **Seeded.** Every size and choice comes from [`SplitMix64`] keyed by the seed: the requests the
//!   allocator sees, in order, are a function of the seed (where they land is the allocator's).
//! - **Bounded.** At most [`MAX_BYTES`] bytes are requested in all, boxes included, and no block
//!   exceeds 64 KiB, so a perturbation never asks for more memory than a small test does and never
//!   reaches a size that allocators serve with fresh pages (`mmap`) and return at once, which would
//!   perturb nothing.
//! - **Nothing initialised, nothing read.** Blocks are reserved capacity (`Vec::with_capacity`) or
//!   boxes of `MaybeUninit` bytes, never written or read, so a perturbation costs allocator calls,
//!   not memory traffic; each block passes through `std::hint::black_box` so the optimiser cannot
//!   remove an allocation it can see unused.
//! - **Held while the case runs.** The pinned blocks and boxes live in the returned [`Heap`] and are
//!   freed when it is dropped, after the case.
//! - **No panic; no allocation failure ignored.** A failed allocation aborts the process, as every
//!   `Vec` and `Box` allocation does, never returns a smaller heap silently.
//! - **Every small allocation after it, on glibc, Windows and WebAssembly.** Their allocators keep
//!   one pool per size class (per thread or arena) whatever code allocates, so steps 2 and 3
//!   shuffle the very pools the case draws from. Measured: sixteen orders in every one of the 1,220
//!   sweeps on each Linux and Windows runner in each build mode, and in all 200 on wasm32-wasip1.
//! - **Nearly every one, on macOS on x86-64.** macOS 15's allocator keeps such lists too, but about
//!   one perturbed run in 25 still hands the 64 boxes out in the order they were allocated. Sweeps
//!   gave 13 to 16 orders (16 in 2,421 of 2,440), and two runs under [`super::check`]'s two seeds
//!   gave the same order once in 1,600 (issue #230's flake of the in-process repeat).
//! - **This module's boxes, on macOS on arm64, and other code's where it shares a pool.** macOS 26's
//!   allocator there, libmalloc's xzone malloc, keeps small blocks in separate pools chosen from a
//!   hash of each allocation's type with keys set per process (`_xzm_type_choose_ptr_bucket`), and
//!   allocations made by different code land in different pools: in debug builds a block freed from
//!   `Vec::with_capacity` never came back to `Box::new` (20 processes of 20), and boxes freed by
//!   another crate did not reorder this module's. Before step 3, sweeps there mostly saw only the
//!   boxes' own two orders (ascending, then descending, from their own reuse), and once one order
//!   (issue #370). Step 3's boxes come from this module, as [`address_order_digest`]'s do: with
//!   them, sixteen orders in every one of the 1,220 sweeps in each build mode. A golden case in
//!   another crate that orders boxed values by address may share no pool with the perturbation there
//!   and pass the repeat on that runner; the other runners still catch it.
//!
//! Method: heap-layout randomisation as in Stabilizer (C. Curtsinger and E. D. Berger, "Stabilizer:
//! Statistically Sound Performance Evaluation", ASPLOS 2013,
//! <https://doi.org/10.1145/2451116.2451141>), whose shuffling layer fills a pool per size class and
//! hands blocks out in random order, and `DieHard`'s randomised placement (E. D. Berger and B. G. Zorn,
//! "`DieHard`: Probabilistic Memory Safety for Unsafe Languages", PLDI 2006,
//! <https://doi.org/10.1145/1133981.1133985>), applied here from outside the allocator by freeing a
//! seeded share of each size class's pool, as blocks and as boxes, in a seeded order; the shuffle
//! is the Fisher-Yates shuffle of [`super::shuffle`].
//!
//! Cost: about 1,400 allocations (8 to 32 for each of the 52 classes and the 16 box sizes, and up to
//! 16 shift blocks) and half as many frees for one perturbation, at most [`MAX_BYTES`] bytes
//! reserved, linear in that count; nothing is written into the blocks.
//!
//! Consulted: shuffling-allocator, MPL-2.0,
//!   <https://github.com/fitzgen/shuffling-allocator/blob/983334621d6c6fd6aa4d40d24edd3221208bf085/src/lib.rs>,
//!   for its size classes and shuffling layer and to judge whether it could be used; not taken, for
//!   the reasons above. No code copied.
//! Consulted: libmalloc's xzone malloc, MIT,
//!   <https://github.com/apple-oss-distributions/libmalloc/blob/c49dafa25f1efe8607701ae6014a663ad2ee437f/src/xzone_malloc/xzone_malloc.c>,
//!   for how macOS on arm64 pools and reuses small blocks (its last-freed-first chunk lists and its
//!   choice of pool), to explain what CI measured. No code copied.

use core::any::Any;
use core::mem::MaybeUninit;
use core::num::NonZeroU64;

use super::shuffle::Orders;
use crate::rng::SplitMix64;
use crate::seed::Seed;

/// The block sizes, in bytes, the perturbation fragments: every multiple of one word up to 128
/// bytes, then four sizes per doubling up to 64 KiB, the size classes allocators keep lists for.
pub const CLASSES: [usize; 52] = [
    8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128, 160, 192, 224, 256, 320,
    384, 448, 512, 640, 768, 896, 1024, 1280, 1536, 1792, 2048, 2560, 3072, 3584, 4096, 5120, 6144,
    7168, 8192, 10240, 12288, 14336, 16384, 20480, 24576, 28672, 32768, 40960, 49152, 57344, 65536,
];

/// The block sizes, in bytes, the perturbation also fragments as boxes: the classes up to 128
/// bytes, where boxed values and small nodes land.
pub const BOX_SIZES: [usize; 16] = [
    8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128,
];

/// The most blocks of one size class the perturbation allocates (it allocates 8 to this many).
pub const MAX_PER_CLASS: u64 = 32;

/// The most bytes one perturbation requests in all (every class and box size at its most, plus the
/// shift).
pub const MAX_BYTES: usize = 32 * 1024 * 1024;

/// The most shift blocks.
const MAX_SHIFT_BLOCKS: u64 = 16;

/// The largest shift block, in bytes.
const MAX_SHIFT_BYTES: u64 = 4096;

/// A pinned box: `N` uninitialised bytes, whatever `N` is.
type Pinned = Box<dyn Any + Send + Sync>;

/// A perturbed heap: the blocks pinned to hold its layout while a case runs.
#[derive(Debug)]
pub struct Heap {
    /// The pinned blocks.
    pinned: Vec<Vec<u8>>,
    /// The pinned boxes.
    boxes: Vec<Pinned>,
    /// The seed that made this layout.
    seed: Seed,
    /// Blocks allocated in all.
    allocated: usize,
    /// Blocks freed again, leaving holes and shuffled free lists.
    freed: usize,
    /// Bytes requested in all.
    bytes: usize,
}

/// One block of `size` bytes of reserved, unwritten capacity, kept from being optimised away.
fn block(size: usize) -> Vec<u8> {
    let v: Vec<u8> = Vec::with_capacity(size);
    std::hint::black_box(v)
}

/// One box of `N` uninitialised bytes, allocated as `Box::new` allocates any value, kept from
/// being optimised away.
fn boxed<const N: usize>() -> Box<MaybeUninit<[u8; N]>> {
    std::hint::black_box(Box::new(MaybeUninit::uninit()))
}

/// A count in `lo..=hi` from `rng`.
fn between(rng: &mut SplitMix64, lo: u64, hi: u64) -> u64 {
    let span = hi.saturating_sub(lo).saturating_add(1);
    NonZeroU64::new(span).map_or(lo, |n| lo.saturating_add(rng.below(n)))
}

/// How many blocks of one class to allocate: 8 to [`MAX_PER_CLASS`], from `rng`.
fn pool_count(rng: &mut SplitMix64) -> usize {
    usize::try_from(between(rng, 8, MAX_PER_CLASS)).unwrap_or(8)
}

impl Heap {
    /// Shifts and fragments the heap from `seed`, and holds the layout until the result is dropped.
    #[must_use]
    pub fn shift_and_fragment(seed: Seed) -> Heap {
        let mut rng = SplitMix64::new(seed);
        let mut orders = Orders::new(Seed(rng.next_u64()));
        let mut heap = Heap {
            pinned: Vec::new(),
            boxes: Vec::new(),
            seed,
            allocated: 0,
            freed: 0,
            bytes: 0,
        };
        // 1. Shift: blocks of uneven sizes, pinned.
        for _ in 0..between(&mut rng, 1, MAX_SHIFT_BLOCKS) {
            let size = usize::try_from(between(&mut rng, 1, MAX_SHIFT_BYTES)).unwrap_or(1);
            heap.pinned.push(block(size));
            heap.allocated = heap.allocated.saturating_add(1);
            heap.bytes = heap.bytes.saturating_add(size);
        }
        // 2. Fragment and shuffle each size class: allocate a pool, free a seeded share in a
        // seeded order, pin the rest.
        for size in CLASSES {
            let count = pool_count(&mut rng);
            let pool: Vec<Vec<u8>> = (0..count).map(|_| block(size)).collect();
            let keep = heap.fragment(pool, size, &mut rng, &mut orders);
            heap.pinned.extend(keep);
        }
        // 3. The same for each of BOX_SIZES, as boxes, which macOS's allocator on arm64 keeps
        // apart from `Vec` blocks (the module header says why).
        heap.box_class::<8>(&mut rng, &mut orders);
        heap.box_class::<16>(&mut rng, &mut orders);
        heap.box_class::<24>(&mut rng, &mut orders);
        heap.box_class::<32>(&mut rng, &mut orders);
        heap.box_class::<40>(&mut rng, &mut orders);
        heap.box_class::<48>(&mut rng, &mut orders);
        heap.box_class::<56>(&mut rng, &mut orders);
        heap.box_class::<64>(&mut rng, &mut orders);
        heap.box_class::<72>(&mut rng, &mut orders);
        heap.box_class::<80>(&mut rng, &mut orders);
        heap.box_class::<88>(&mut rng, &mut orders);
        heap.box_class::<96>(&mut rng, &mut orders);
        heap.box_class::<104>(&mut rng, &mut orders);
        heap.box_class::<112>(&mut rng, &mut orders);
        heap.box_class::<120>(&mut rng, &mut orders);
        heap.box_class::<128>(&mut rng, &mut orders);
        heap
    }

    /// Fragments one pool of `size`-byte blocks: shuffles it, frees between a quarter and three
    /// quarters of it in the shuffled order, and returns the rest, to be pinned.
    fn fragment<T>(
        &mut self,
        mut pool: Vec<T>,
        size: usize,
        rng: &mut SplitMix64,
        orders: &mut Orders,
    ) -> Vec<T> {
        let count = pool.len();
        self.allocated = self.allocated.saturating_add(count);
        self.bytes = self.bytes.saturating_add(size.saturating_mul(count));
        orders.shuffle(&mut pool);
        let quarter = u64::try_from(count / 4).unwrap_or(2);
        let free =
            usize::try_from(between(rng, quarter, quarter.saturating_mul(3))).unwrap_or(count / 2);
        let keep = pool.split_off(free.min(count));
        for freed in pool {
            drop(std::hint::black_box(freed));
            self.freed = self.freed.saturating_add(1);
        }
        keep
    }

    /// Step 3 for one size: a pool of boxes of `N` bytes, fragmented, the rest pinned.
    fn box_class<const N: usize>(&mut self, rng: &mut SplitMix64, orders: &mut Orders) {
        let count = pool_count(rng);
        let pool: Vec<Box<MaybeUninit<[u8; N]>>> = (0..count).map(|_| boxed::<N>()).collect();
        for kept in self.fragment(pool, N, rng, orders) {
            self.boxes.push(kept);
        }
    }

    /// The seed that made this layout.
    #[must_use]
    pub const fn seed(&self) -> Seed {
        self.seed
    }

    /// Blocks allocated, blocks freed again, blocks pinned (boxes included) and bytes requested,
    /// for the report.
    #[must_use]
    pub fn counts(&self) -> (usize, usize, usize, usize) {
        (
            self.allocated,
            self.freed,
            self.pinned.len().saturating_add(self.boxes.len()),
            self.bytes,
        )
    }
}

/// The digest of the order a computation finds when it sorts values by the addresses they were
/// boxed at: the address-dependent computation the attack tests use to prove that a perturbation
/// changes what such code sees. It boxes `0..n` in order, sorts the boxes by address and hashes
/// the values in that order.
#[must_use]
pub fn address_order_digest(n: u64) -> [u8; 32] {
    let boxes: Vec<Box<u64>> = (0..n).map(Box::new).collect();
    let mut by_address: Vec<&u64> = boxes.iter().map(|b| &**b).collect();
    by_address.sort_by_key(|v| core::ptr::from_ref::<u64>(*v).addr());
    let mut h = super::sha256::Sha256::new();
    for v in by_address {
        h.write(&v.to_le_bytes());
    }
    h.finish()
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

    use super::*;

    #[test]
    fn classes_are_increasing_word_multiples_up_to_64_kib() {
        assert!(CLASSES.windows(2).all(|w| w[0] < w[1]));
        assert!(CLASSES.iter().all(|c| c % 8 == 0));
        assert_eq!(CLASSES.last(), Some(&65_536));
        let most: usize = CLASSES
            .iter()
            .chain(BOX_SIZES.iter())
            .fold(0, |a, c| a + c * MAX_PER_CLASS as usize)
            + (MAX_SHIFT_BLOCKS * MAX_SHIFT_BYTES) as usize;
        assert!(most <= MAX_BYTES, "{most} > {MAX_BYTES}");
        assert_eq!(BOX_SIZES[..], CLASSES[..BOX_SIZES.len()]);
    }

    /// Step 3 boxes every size of `BOX_SIZES`, and only those: each pool keeps at least a quarter
    /// of its boxes.
    #[test]
    fn every_box_size_is_fragmented_and_pinned() {
        for seed in [0u64, 7, u64::MAX] {
            let heap = Heap::shift_and_fragment(Seed(seed));
            let sizes: BTreeSet<usize> = heap
                .boxes
                .iter()
                .map(|b| core::mem::size_of_val::<dyn Any + Send + Sync>(&**b))
                .collect();
            assert_eq!(sizes, BOX_SIZES.into_iter().collect::<BTreeSet<_>>());
        }
    }

    #[test]
    fn a_perturbation_is_a_function_of_its_seed_and_stays_in_bounds() {
        let a = Heap::shift_and_fragment(Seed(7));
        let b = Heap::shift_and_fragment(Seed(7));
        let c = Heap::shift_and_fragment(Seed(8));
        assert_eq!(a.counts(), b.counts());
        assert_ne!(a.counts(), c.counts());
        for h in [&a, &b, &c] {
            let (allocated, freed, pinned, bytes) = h.counts();
            assert_eq!(allocated, freed + pinned);
            assert!(freed > 0 && pinned > 0);
            assert!(bytes <= MAX_BYTES);
        }
        assert_eq!(a.seed(), Seed(7));
    }

    /// The point of the perturbation: code that orders by address sees different orders under
    /// different seeds, at least four from eight. The header's last three guarantees say why on
    /// each of CI's allocators: every seed gave an order of its own in all but a few of the
    /// sweeps measured, so four is a wide margin, and a perturbation that stops reaching the boxes
    /// (macOS on arm64 before step 3, two orders from their own reuse) fails at once.
    #[test]
    fn perturbations_change_what_address_ordered_code_sees() {
        let mut seen = BTreeSet::new();
        for seed in 1..=8u64 {
            let heap = Heap::shift_and_fragment(Seed(seed));
            seen.insert(address_order_digest(64));
            drop(heap);
        }
        assert!(
            seen.len() >= 4,
            "eight perturbations gave {} orders of 64 boxes",
            seen.len()
        );
    }
}
