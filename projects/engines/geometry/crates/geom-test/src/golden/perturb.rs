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
//! 3. **Box the small classes, from many call sites.** For each of [`BOX_SIZES`], the classes up to
//!    128 bytes, it does the same with boxes of that many uninitialised bytes, made by `Box::new`
//!    as any value is boxed, one or two from each of [`BOX_SITES`] call sites. Where an allocator
//!    keeps one list per size class whatever code allocates, this only adds to step 2; macOS's
//!    allocator on arm64 keeps four lists per size and, in release builds, picks one by call site
//!    (the guarantees say how).
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
//! CI's, with what CI measured for issues #370 and #377 (run 40823425292 measured this module with
//! step 3 from one call site per size, on every runner; run 40890282881 measured it as it is, on
//! macOS; runs 40807005781, 40813238936, 40816535727, 40819713151, 40871785914 and 40886703835
//! measured earlier versions and the ways tried and set aside). The measure is a sweep: sixteen
//! seeds (or eight), each perturbing the heap before 64 values are boxed and sorted by address
//! ([`address_order_digest`]), on one thread alone and among busy tests; a sweep counts the
//! different orders it sees.
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
//!   shuffle the very pools the case draws from. Measured (with step 3 from one call site, which
//!   there only means fewer boxes in the same pools): sixteen orders in every one of the 1,220
//!   sweeps on each Linux and Windows runner in each build mode, and in all 200 on wasm32-wasip1.
//! - **Nearly every one, on macOS on x86-64.** macOS 15's allocator keeps such lists too, but about
//!   one perturbed run in 25 still hands the 64 boxes out in the order they were allocated. Sweeps
//!   gave 13 to 16 orders (16 in 2,421 of 2,440; with step 3 as it is, 16 in 99 of 100), and two
//!   runs under [`super::check`]'s two seeds gave the same order once in 1,600 (issue #230's flake
//!   of the in-process repeat).
//! - **Every box up to 128 bytes, on macOS on arm64.** macOS 26's allocator there, libmalloc's xzone
//!   malloc, keeps four pools for each 16-byte size class and puts each block in the one a keyed
//!   hash of the block's type picks (`_xzm_type_choose_ptr_bucket`). Rust's allocations carry no
//!   type, and in release builds the pool followed the allocating call site and changed from
//!   process to process (measured; the part of libmalloc that stands in a type for plain `malloc` is
//!   under the APSL and is not relied on here). With step 3's boxes from one call site per size,
//!   about one release process in eight, quiet or busy, put [`address_order_digest`]'s boxes in a
//!   pool nothing perturbed, and every seed then saw their own two orders, ascending and descending
//!   (issue #377, runs 40871785914 and 40886703835; #370 was the same before step 3). From
//!   [`BOX_SITES`] call sites for each of the two sizes in a class, 64 sites draw pools in each
//!   class. Measured (run 40890282881, four runners, each a fresh boot with keys of its own): in
//!   release, sweeps of sixteen seeds gave 14 to 16 orders (16 in 718 of 760) and of eight seeds 6
//!   to 8 (8 in 716 of 760), quiet and busy; in debug, every order in all 80 processes. A golden
//!   case that orders blocks over 128 bytes by address may still pass the repeat on that runner in
//!   release, since step 2 allocates each class from one call site and reaches one of its four
//!   pools; the other runners still catch it.
//!
//! Method: heap-layout randomisation as in Stabilizer (C. Curtsinger and E. D. Berger, "Stabilizer:
//! Statistically Sound Performance Evaluation", ASPLOS 2013,
//! <https://doi.org/10.1145/2451116.2451141>), whose shuffling layer fills a pool per size class and
//! hands blocks out in random order, and `DieHard`'s randomised placement (E. D. Berger and B. G. Zorn,
//! "`DieHard`: Probabilistic Memory Safety for Unsafe Languages", PLDI 2006,
//! <https://doi.org/10.1145/1133981.1133985>), applied here from outside the allocator by freeing a
//! seeded share of each size class's pool, as blocks and as boxes from many call sites, in a seeded
//! order; the shuffle is the Fisher-Yates shuffle of [`super::shuffle`].
//!
//! Cost: about 1,800 allocations (8 to 32 for each of the 52 classes, 32 to 64 for each of the 16
//! box sizes, and up to 16 shift blocks) and half as many frees for one perturbation, at most
//! [`MAX_BYTES`] bytes reserved, linear in that count; nothing is written into the blocks. Step 3's
//! call sites are 512 small functions in the binary.
//!
//! Consulted: shuffling-allocator, MPL-2.0,
//!   <https://github.com/fitzgen/shuffling-allocator/blob/983334621d6c6fd6aa4d40d24edd3221208bf085/src/lib.rs>,
//!   for its size classes and shuffling layer and to judge whether it could be used; not taken, for
//!   the reasons above. No code copied.
//! Consulted: libmalloc's xzone malloc, MIT,
//!   <https://github.com/apple-oss-distributions/libmalloc/blob/c49dafa25f1efe8607701ae6014a663ad2ee437f/src/xzone_malloc/xzone_malloc.c>,
//!   with `src/xzone_malloc/xzone_malloc.h` (MIT) at the same commit, for how macOS on arm64 pools
//!   and reuses small blocks (its last-freed-first chunk lists, its four pools per size class and
//!   its keyed choice of pool), to explain what CI measured. No code copied.

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

/// The most blocks of one class of [`CLASSES`] the perturbation allocates (it allocates 8 to this
/// many).
pub const MAX_PER_CLASS: u64 = 32;

/// The call sites step 3 allocates each box size from, one or two boxes from each.
pub const BOX_SITES: usize = 32;

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

/// One box of `N` uninitialised bytes, allocated as `Box::new` allocates any value, at call site
/// `SITE`: each `N` and `SITE` is a function of its own that is never inlined, so each calls the
/// allocator from its own address, and `SITE` passes through `std::hint::black_box` so no two
/// compile to the same code and get merged into one. The box is kept from being optimised away.
#[inline(never)]
fn boxed_at<const N: usize, const SITE: usize>() -> Box<MaybeUninit<[u8; N]>> {
    std::hint::black_box(SITE);
    std::hint::black_box(Box::new(MaybeUninit::uninit()))
}

/// Pushes one or two boxes of `$n` bytes (the count from `$rng`) from each call site listed, onto
/// `$pool`. The list names every site number below [`BOX_SITES`] once (a test checks the count).
macro_rules! from_sites {
    ($pool:ident, $n:ident, $rng:ident; $($site:literal)*) => {
        $(
            for _ in 0..between($rng, 1, 2) {
                $pool.push(boxed_at::<$n, $site>());
            }
        )*
    };
}

/// A pool of boxes of `N` bytes for step 3: one or two from each of [`BOX_SITES`] call sites.
fn box_pool<const N: usize>(rng: &mut SplitMix64) -> Vec<Box<MaybeUninit<[u8; N]>>> {
    let mut pool = Vec::with_capacity(BOX_SITES.saturating_mul(2));
    from_sites!(pool, N, rng;
        0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31);
    pool
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
        // 3. The same for each of BOX_SIZES, as boxes from BOX_SITES call sites each, since
        // macOS's allocator on arm64 picks a small block's pool from its call site (the module
        // header says how).
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

    /// Step 3 for one size: a pool of boxes of `N` bytes from [`BOX_SITES`] call sites,
    /// fragmented, the rest pinned.
    fn box_class<const N: usize>(&mut self, rng: &mut SplitMix64, orders: &mut Orders) {
        let pool = box_pool::<N>(rng);
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
            .fold(0, |a, c| a + c * MAX_PER_CLASS as usize)
            + BOX_SIZES.iter().fold(0, |a, c| a + c * 2 * BOX_SITES)
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

    /// Step 3 draws one or two boxes from each of `BOX_SITES` call sites: the pool holds exactly
    /// the boxes the seed's draws ask for, so the macro's list of sites matches the constant.
    #[test]
    fn a_box_pool_draws_from_every_site() {
        for seed in 0..64u64 {
            let mut rng = SplitMix64::new(Seed(seed));
            let got = box_pool::<8>(&mut rng).len();
            let mut replay = SplitMix64::new(Seed(seed));
            let want = (0..BOX_SITES).fold(0, |n, _| n + between(&mut replay, 1, 2));
            assert_eq!(got as u64, want);
            assert!((BOX_SITES..=2 * BOX_SITES).contains(&got));
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
    /// different seeds, at least four from eight. The header's last three guarantees say what each
    /// of CI's allocators gave: every seed an order of its own in most sweeps measured, and never
    /// fewer than six orders from eight (macOS on arm64) or 13 from 16 (macOS on x86-64), so four
    /// is a wide margin, and a perturbation that stops reaching the boxes
    /// (as on macOS on arm64 in release when step 3 boxed from one call site: two orders, from the
    /// boxes' own reuse) fails at once.
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
