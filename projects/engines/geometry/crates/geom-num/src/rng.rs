//! Counter-based random numbers from explicit seeds (Philox4x32-10): the one generator every
//! randomised kernel method draws from.
//!
//! A few methods the kernel uses are randomised by design: RANSAC fitting of planes, cylinders and
//! other primitives to point clouds (chapters 16.3 and 17.10), the random sampling inside quad
//! layout (16.3), seeded grouping in panelisation (16.4), and the separate operation that makes the
//! seeds of a stochastic lattice (16.2). Every random number they use comes from here. Each one is a
//! pure function of three explicit integers: a seed, a stream id and a position in the stream. So a
//! randomised method gives the same bits on every target, on any number of threads, every time its
//! operation is replayed (chapter 21). Nothing here reads a clock, the machine or any hidden state.
//!
//! # The randomised-method rule
//!
//! Every randomised kernel method follows the randomised-method rule: one generator, this one; the
//! seed an explicit input, recorded in the operation log; one stream per consumer, numbered by its
//! position; the generator, its layout and the draws part of the operation's version; honest
//! results; and test generators kept apart. Piece G32 wrote the rule here, and piece G17 lifted it,
//! word for word, into the kernel's `AGENTS.md` (`projects/engines/geometry/AGENTS.md`, section 8),
//! which is now its one home; `tests/rng_ban.rs` reads rule 1 there. This module is rule 1's one
//! generator: its layout, its draw definitions and its stream ranges, below, are what rules 3 and 4
//! refer to.
//!
//! What this module guarantees:
//!
//! - **Specified, and checked against the reference.** [`philox4x32_10`] is Philox4x32-10 as its
//!   authors specify it (Method, below). The tests reproduce Random123's published known answers
//!   (`tests/rng_kat.rs`), and golden hashes pin every draw function's output on every CI target,
//!   WebAssembly included (`tests/rng_determinism.rs`).
//! - **Explicit inputs only.** A [`Seed`] and a [`StreamId`] are plain integers the caller writes.
//!   There is deliberately no `Default`, no constructor from entropy and no global or thread-local
//!   generator.
//! - **Integers, and no step that wraps or drops a bit unseen.** Every operation is on integers.
//!   Wrapping happens only where the method is defined modulo a power of two: the specification's
//!   32 × 32-bit product, formed modulo 2^64 as its reference forms it (it never reaches 2^64), and
//!   its key schedule, modulo 2^32; and Lemire's threshold, a negation modulo 2^64. Every other
//!   step is a checked method, or exact by construction (words split and joined through their
//!   bytes, widening casts, bit masks and shifts by constants with their ranges written beside
//!   them, and `abs_diff` where the invariants make it the difference), and each place says why. A
//!   checked step's failure either has a meaning, which is then the answer (a stream id past
//!   2^64 − 1 lies past every range's end), or is ruled out by a proof written beside it and
//!   reported as [`RngError::Defect`] should it happen all the same, never answered. The
//!   floating-point steps, in [`Stream::next_f64`], are exact. So results are the same bits on
//!   every target, in debug and release, with overflow checks on or off, and no lint on arithmetic
//!   or casts is allowed outside the tests.
//! - **Exactly uniform, given uniform words.** [`Stream::below`] and [`Stream::range_inclusive`]
//!   give every value of the range from exactly the same number of 64-bit words (Lemire's rejection
//!   method, so there is no modulo bias), and [`Stream::next_f64`] gives each of the 2^53 values
//!   k · 2^−53 in [0, 1) from exactly 2^11 words. That Philox's words are themselves
//!   indistinguishable from uniform and independent is the authors' claim, from the `TestU01`
//!   library's Crush batteries: seven rounds of Philox4x32 pass, and ten keep three rounds as a
//!   safety margin. This module relies on that claim; it does not prove it.
//! - **Streams never overlap and never wrap.** For one key, Philox is a bijection on counters (each
//!   round can be undone; `tests/rng_properties.rs` inverts it), so no two (stream, block) pairs
//!   share a block. A stream is 2^64 blocks of four words; moving to the
//!   next block carries from counter word 0 into word 1, never into the stream's words. Drawing past
//!   the end returns [`RngError::Exhausted`].
//! - **Ranges handed out never overlap.** Chunks of one size cut from one [`StreamRange`] are
//!   disjoint and lie inside it, with checked integer arithmetic; a range too small for a request
//!   returns [`RngError::TooFewStreams`] (`tests/rng_ranges.rs` checks both against a model in
//!   arbitrary-precision integers).
//! - **All or nothing.** A draw that fails leaves its stream exactly as it was.
//! - **No panics, no `unsafe`, no allocation, no `std`.** The block function, every draw and every
//!   range operation are `const fn`s, so a table of random numbers can be computed at compile time,
//!   with the same bits.
//!
//! # Layout
//!
//! This is part of the specification: changing any of it changes every stream, so none of it is
//! ever changed in place (rule 4 of the randomised-method rule).
//!
//! - **Key.** The two key words are the seed's low and high 32 bits: `k0 = seed & 0xffff_ffff`,
//!   `k1 = seed >> 32`.
//! - **Counter.** Block `b` of stream `s` has the counter `[c0, c1, c2, c3]` with `c0` and `c1` the
//!   low and high 32 bits of `b`, and `c2` and `c3` the low and high 32 bits of `s`
//!   ([`Philox::counter`]).
//! - **Words.** A [`Stream`] returns the four output words of block 0 in order (output word 0
//!   first), then those of block 1, and so on.
//! - **`next_u64`** is the next word plus the word after it times 2^32 (the first word is the low
//!   half), across a block boundary when it falls there.
//! - **`next_f64`** is the top 53 bits of `next_u64`, times 2^−53, as a [`Real`].
//! - **`below(n)`** draws `next_u64` words `x` until the low 64 bits of `x · n` are at least
//!   2^64 mod n, then returns the high 64 bits. **`range_inclusive(lo, hi)`** is
//!   `lo + below(hi − lo + 1)`, or `next_u64` itself when the range is all of `u64`.
//! - **Stream ranges.** [`StreamRange::all`] of a seed is stream ids 0 to 2^64 − 1. Stream `i` of a
//!   range starting at id `f` is id `f + i`, and chunk `i` of size `m` is ids `f + i·m` to
//!   `f + i·m + m − 1`.
//!
//! Method: the published methods below.
//!
//! - **Philox4x32-10**: J. K. Salmon, M. A. Moraes, R. O. Dror and D. E. Shaw, "Parallel random
//!   numbers: as easy as 1, 2, 3", SC '11, section 4.3, <https://doi.org/10.1145/2063384.2063405>.
//!   Each round is the paper's generalised Feistel S-box on two pairs of words, with the two
//!   multipliers its search found for Philox-4x32; the key steps through a Weyl sequence between
//!   rounds. The word order, the multiplier for each word and the order of key steps and rounds
//!   follow the authors' reference implementation, Random123's `philox.h` (BSD-3-Clause, D. E.
//!   Shaw Research), at
//!   <https://github.com/DEShawResearch/random123/blob/9545ff6413f258be2f04c1d319d99aaef7521150/include/Random123/philox.h>.
//!   The code here is written from the specification; none of Random123's code is copied. The
//!   tests quote its known-answer vectors, which are facts.
//! - **Integers in a range**: D. Lemire, "Fast random integer generation in an interval", *ACM
//!   Transactions on Modeling and Computer Simulation* 29(1), 2019,
//!   <https://doi.org/10.1145/3230636> (arXiv:1805.10941). Multiply the word by the bound, keep the
//!   high half, and reject the few words whose low half is below 2^64 mod n. As in the paper's
//!   nearly divisionless method, that remainder is computed at most once per call, and only once a
//!   word's low half is below n.
//! - **Doubles**: the top 53 bits of a 64-bit word times 2^−53, as S. Vigna recommends in
//!   "Generating uniform doubles in the unit interval", <https://prng.di.unimi.it/>.
//! - **Stream ranges**: consecutive, equal, disjoint blocks of stream ids, cut with checked 64-bit
//!   arithmetic, in which a step past 2^64 − 1 is past the range's last id. A counter-based
//!   generator reaches any stream directly from its id (Salmon and others, above), so handing each
//!   consumer its own ids is all that independent streams need.
//!
//! Consulted: Random123, BSD-3-Clause,
//!   <https://github.com/DEShawResearch/random123/blob/9545ff6413f258be2f04c1d319d99aaef7521150/include/Random123/philox.h>,
//!   for the word order, the multiplier for each word and the order of key steps and rounds of
//!   Philox4x32-10, as the authors' reference implementation fixes them. No code copied.
//!
//! Cost: a block is ten rounds, each two 32 × 32 → 64-bit multiplies and four exclusive ors, with nine
//! two-word key additions between them, and gives four 32-bit words. [`Stream::next_u32`] computes
//! a block on every fourth call, and [`Stream::next_u64`] and [`Stream::next_f64`] on every second
//! call. [`Stream::below`] draws on average 2^64 / (2^64 − (2^64 mod n)) words of 64 bits, fewer
//! than two for every n and fewer than 1 + 2^−32 for n below 2^32. It computes the 64-bit
//! remainder 2^64 mod n at most once per call, the first time a word's low half is below n, which
//! happens for each word with probability exactly n / 2^64 (for odd n the low half is uniform; for
//! even n = 2^k · m it is a multiple of 2^k, each from 2^k words). A [`StreamRange`] operation is a
//! few integer operations. A [`Stream`] is a few dozen bytes and allocates nothing. Speed is
//! measured once the benchmark harness exists (piece G18); no figure is claimed here.

use core::fmt;
use core::num::NonZeroU64;

use crate::real::Real;

/// Philox-4x32's multiplier for counter word 0. Found by the authors' search for the multipliers
/// whose four-round Philox shows the strongest avalanche (section 4.3); it has no closed form, and
/// the known-answer tests pin it.
const M0: u32 = 0xD251_1F53;

/// Philox-4x32's multiplier for counter word 2 (see [`M0`]).
const M1: u32 = 0xCD9E_8D57;

/// The Weyl step added to key word 0 between rounds: ⌊2^32 · (√5 − 1)/2⌋, the first 32 bits of the
/// golden ratio's fractional part. The unit tests derive it in exact integer arithmetic.
const W0: u32 = 0x9E37_79B9;

/// The Weyl step added to key word 1 between rounds: ⌊2^32 · (√3 − 1)⌋. The unit tests derive it in
/// exact integer arithmetic.
const W1: u32 = 0xBB67_AE85;

/// The number of rounds: seven pass the authors' Crush tests, and ten keep three as a safety margin,
/// which is the variant they recommend.
const ROUNDS: usize = 10;

/// One element for each round after the first, each of which first steps the key:
/// [`philox4x32_10`] walks them with a slice pattern, so that no round counter is stepped. The
/// compiler evaluates the length, and `ROUNDS` below 1 would stop the build.
const LATER_ROUNDS: [(); ROUNDS - 1] = [(); ROUNDS - 1];

/// The number of 32-bit words in one stream: 2^64 blocks of four.
pub const WORDS_PER_STREAM: u128 = 1 << 66;

/// 2^−53, exactly (a power of two, so the division that writes it is exact).
const TWO_POW_MINUS_53: f64 = 1.0 / 9_007_199_254_740_992.0;

/// 2^32, exactly (a whole number below 2^53, so a double).
const TWO_POW_32: f64 = 4_294_967_296.0;

/// A seed: the 64-bit Philox key, written by the caller and stored in the operation's input.
///
/// There is no `Default` and no constructor from entropy, on purpose (rule 2 of the
/// randomised-method rule, in the kernel's `AGENTS.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Seed(pub u64);

/// A stream id: which of a seed's 2^64 independent streams to draw from. Numbered by the consumer's
/// stable position, never by thread or timing, and usually handed out by a [`StreamRange`] (rule 3
/// of the randomised-method rule, in the kernel's `AGENTS.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StreamId(pub u64);

/// Why a draw or a stream range could not be made. A failed draw leaves its stream exactly as it
/// was.
///
/// This crate's own error never reaches a caller of an operation (chapter 22): the calling
/// operation wraps it in its own detail with a base kind. Which base kind fits depends on a cause
/// only the calling method knows: a bound, range or count that came from the operation's input is
/// `InvalidInput`, and one the method computed itself (a bound, a chunk size, a draw placed near a
/// stream's end) is `Internal`, a bug. [`RngError::Defect`] is always `Internal`.
///
/// `#[non_exhaustive]`, like chapter 22's per-operation details: a new draw function may add a
/// variant without breaking callers, who handle the variants they do not know as `Internal`.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RngError {
    /// The stream has given every one of its 2^66 words (or has fewer left than the draw needs).
    /// A stream never wraps around to repeat itself and never runs into another stream.
    Exhausted {
        /// The stream that ran out.
        stream: StreamId,
    },
    /// [`Stream::below`] was given the bound 0: no whole number is below it, so the range is empty.
    ZeroBound,
    /// [`Stream::range_inclusive`] was given a lower end above its upper end: the range is empty.
    LowAboveHigh {
        /// The lower end asked for.
        low: u64,
        /// The upper end asked for.
        high: u64,
    },
    /// [`StreamRange::chunk`] was asked for chunks of no streams.
    EmptyChunk,
    /// A [`StreamRange`] was asked for a stream or a chunk that reaches past its last stream id.
    /// Ranges never wrap and never reach into a neighbour's streams.
    TooFewStreams {
        /// The range's first stream id.
        first: StreamId,
        /// The range's last stream id.
        last: StreamId,
    },
    /// A step this module proves cannot fail did fail: Lemire's 128-bit product or its threshold
    /// ([`Stream::below`] and [`Stream::range_inclusive`]), the sum that places a value in its
    /// range ([`Stream::range_inclusive`]), or [`Real::new`] on the double [`Stream::next_f64`]
    /// made. Each step's proof is written beside it, so no input can cause this, only a defect in
    /// geom-num. It is reported rather than answered, because any answer would be a guess, as
    /// [`NumError::BudgetExceeded`](crate::NumError::BudgetExceeded) is.
    Defect,
}

impl fmt::Display for RngError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RngError::Exhausted { stream } => write!(
                f,
                "random stream {} has too few of its 2^66 words left for this draw",
                stream.0
            ),
            RngError::ZeroBound => f.write_str("a random integer below zero was asked for"),
            RngError::LowAboveHigh { low, high } => write!(
                f,
                "a random integer from {low} to {high} was asked for, but {low} is above {high}"
            ),
            RngError::EmptyChunk => f.write_str("a chunk of no random streams was asked for"),
            RngError::TooFewStreams { first, last } => write!(
                f,
                "random streams {} to {} are too few for what was asked",
                first.0, last.0
            ),
            RngError::Defect => {
                f.write_str("a random draw step that cannot fail did fail (a defect in geom-num)")
            }
        }
    }
}

/// The low and high 32-bit halves of `x`, x = high · 2^32 + low, read from its little-endian bytes:
/// an exact split, with no cast that could drop a bit and no shift.
const fn split_u64(x: u64) -> [u32; 2] {
    let [b0, b1, b2, b3, b4, b5, b6, b7] = x.to_le_bytes();
    [
        u32::from_le_bytes([b0, b1, b2, b3]),
        u32::from_le_bytes([b4, b5, b6, b7]),
    ]
}

/// `low + high · 2^32`, the inverse of [`split_u64`], written through little-endian bytes: exact,
/// with no cast and no shift.
const fn join_u32(low: u32, high: u32) -> u64 {
    let [l0, l1, l2, l3] = low.to_le_bytes();
    let [h0, h1, h2, h3] = high.to_le_bytes();
    u64::from_le_bytes([l0, l1, l2, l3, h0, h1, h2, h3])
}

/// The low and high 64-bit halves of `x`, x = high · 2^64 + low, read from its little-endian bytes
/// as [`split_u64`] reads its halves: exact, with no cast and no shift.
const fn split_u128(x: u128) -> [u64; 2] {
    let [b0, b1, b2, b3, b4, b5, b6, b7, high @ ..] = x.to_le_bytes();
    [
        u64::from_le_bytes([b0, b1, b2, b3, b4, b5, b6, b7]),
        u64::from_le_bytes(high),
    ]
}

/// The two halves of the exact 64-bit product of two 32-bit words, `(mulhi, mullo)` in the paper's
/// notation: `mulhi(a, b) = ⌊a · b / 2^32⌋` and `mullo(a, b) = a · b mod 2^32`.
///
/// `mullo` is a product modulo 2^32 by the specification's definition, so `wrapping_mul` on `u32`
/// is the specification, not an overflow. `mulhi` is the high half of the 64-bit product, which the
/// reference `philox.h` forms, in its portable code, with one 64-bit unsigned multiplication, which
/// C defines modulo 2^64: `wrapping_mul` on `u64` is that operation. It never wraps here, so the
/// product is exact: the largest is (2^32 − 1)^2 = 2^64 − 2^33 + 1 < 2^64. In a `const fn`, stable
/// Rust has no 32 × 32 → 64-bit multiplication that cannot fail by its type other than native `*`
/// and the wrapping and overflowing methods: `u32::widening_mul` is unstable, and
/// `u32::carrying_mul`, stable in Rust 1.99, is not stable as a `const fn`. The widening casts to
/// `u64` are lossless, and the high half is read from the product's bytes.
const fn mulhilo(a: u32, b: u32) -> (u32, u32) {
    let low = a.wrapping_mul(b);
    let [_, high] = split_u64((a as u64).wrapping_mul(b as u64));
    (high, low)
}

/// One Philox-4x32 round: two S-boxes, on the word pairs (0, 1) and (2, 3), with the P-box that
/// swaps their right halves, as in the reference `philox.h`.
const fn round(counter: [u32; 4], key: [u32; 2]) -> [u32; 4] {
    let [c0, c1, c2, c3] = counter;
    let [k0, k1] = key;
    let (high0, low0) = mulhilo(M0, c0);
    let (high1, low1) = mulhilo(M1, c2);
    [high1 ^ c1 ^ k0, low1, high0 ^ c3 ^ k1, low0]
}

/// One step of the key schedule: add the Weyl constants. The specification adds modulo 2^32 (the
/// paper's Weyl sequence on 32-bit key words, which the reference `philox.h` adds as 32-bit
/// unsigned integers, defined in C modulo 2^32), so `wrapping_add` is the specification, not an
/// overflow.
const fn bump_key(key: [u32; 2]) -> [u32; 2] {
    let [k0, k1] = key;
    [k0.wrapping_add(W0), k1.wrapping_add(W1)]
}

/// Philox4x32-10: the specified bijection from a 128-bit counter to a 128-bit block, keyed by 64
/// bits.
///
/// Round 1 uses the key as given; each of the nine later rounds first adds the Weyl constants to the
/// key, as in the reference `philox.h`. A pure function of its two arguments, the same bits on
/// every target. Most callers want [`Philox`] and [`Stream`], which fix the counter layout; this is
/// the raw function the known-answer tests check.
#[must_use]
pub const fn philox4x32_10(counter: [u32; 4], key: [u32; 2]) -> [u32; 4] {
    let mut block = round(counter, key);
    let mut key = key;
    // The nine later rounds, one element of `LATER_ROUNDS` each, so no counter is stepped.
    let mut later: &[()] = &LATER_ROUNDS;
    while let [(), rest @ ..] = later {
        key = bump_key(key);
        block = round(block, key);
        later = rest;
    }
    block
}

/// Philox4x32-10 keyed by a seed, for one stream: block `b` of the stream is a pure function of the
/// seed, the stream id and `b`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Philox {
    seed: Seed,
    stream: StreamId,
}

impl Philox {
    /// The generator for stream `stream` of `seed`.
    #[must_use]
    pub const fn new(seed: Seed, stream: StreamId) -> Philox {
        Philox { seed, stream }
    }

    /// The seed.
    #[must_use]
    pub const fn seed(self) -> Seed {
        self.seed
    }

    /// The stream id.
    #[must_use]
    pub const fn stream_id(self) -> StreamId {
        self.stream
    }

    /// The two key words: the seed's low and high 32 bits.
    #[must_use]
    pub const fn key(self) -> [u32; 2] {
        split_u64(self.seed.0)
    }

    /// The Philox counter of block `block`: the block index's low and high 32 bits, then the stream
    /// id's.
    #[must_use]
    pub const fn counter(self, block: u64) -> [u32; 4] {
        let [b0, b1] = split_u64(block);
        let [s0, s1] = split_u64(self.stream.0);
        [b0, b1, s0, s1]
    }

    /// Block `block` of this stream: `philox4x32_10(self.counter(block), self.key())`.
    #[must_use]
    pub const fn block(self, block: u64) -> [u32; 4] {
        philox4x32_10(self.counter(block), self.key())
    }

    /// This stream from its start.
    #[must_use]
    pub const fn stream(self) -> Stream {
        self.stream_from(0)
    }

    /// This stream from the start of block `block`, as though the `4 · block` words before it had
    /// been drawn. Skipping ahead costs one block, whatever the distance.
    #[must_use]
    pub const fn stream_from(self, block: u64) -> Stream {
        Stream {
            philox: self,
            // 4 · block, exactly: block < 2^64, so shifting it left two places stays below 2^66,
            // inside `u128`, and the widening cast is lossless.
            drawn: (block as u128) << 2,
            block: self.block(block),
        }
    }
}

/// A run of consecutive stream ids of one seed, owned by one randomised method (rule 3 of the
/// randomised-method rule, in the kernel's `AGENTS.md`).
///
/// An operation starts from [`StreamRange::all`] of its seed. A method gives consumer `i` either
/// stream `i` ([`StreamRange::philox`]) or chunk `i` of one fixed size ([`StreamRange::chunk`]), so
/// no two consumers share a stream, and a sub-method numbers its own consumers inside the chunk it
/// is given, so nested methods never reuse one. Never empty: it holds from 1 to 2^64 streams.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StreamRange {
    seed: Seed,
    /// The first stream id.
    first: u64,
    /// The last stream id, included; never below `first`.
    last: u64,
}

impl StreamRange {
    /// Every stream of `seed`: ids 0 to 2^64 − 1.
    #[must_use]
    pub const fn all(seed: Seed) -> StreamRange {
        StreamRange {
            seed,
            first: 0,
            last: u64::MAX,
        }
    }

    /// The seed.
    #[must_use]
    pub const fn seed(self) -> Seed {
        self.seed
    }

    /// The first stream id.
    #[must_use]
    pub const fn first(self) -> StreamId {
        StreamId(self.first)
    }

    /// The last stream id, included.
    #[must_use]
    pub const fn last(self) -> StreamId {
        StreamId(self.last)
    }

    /// How many streams the range holds: from 1 to 2^64.
    #[must_use]
    pub const fn count(self) -> u128 {
        // `last` ≥ `first`, so `abs_diff` is last − first, exactly. Adding one fails only when that
        // is 2^64 − 1, and then the range holds every id, 2^64 of them.
        match self.last.abs_diff(self.first).checked_add(1) {
            Some(count) => count as u128,
            None => 1 << 64,
        }
    }

    /// Stream `index` of the range, counted from its first stream: id `first + index`. The same as
    /// `self.chunk(index, 1)?.philox(0)`.
    ///
    /// # Errors
    ///
    /// [`RngError::TooFewStreams`] when `index` reaches past the last stream.
    pub const fn philox(self, index: u64) -> Result<Philox, RngError> {
        // A sum past 2^64 − 1 is past `last`, which is at most that, as is a sum above `last`.
        match self.first.checked_add(index) {
            Some(id) if id <= self.last => Ok(Philox::new(self.seed, StreamId(id))),
            _ => Err(self.too_few()),
        }
    }

    /// Chunk `index` of the range cut into consecutive chunks of `size` streams: ids
    /// `first + index · size` to `first + index · size + size − 1`. Chunks of one size are
    /// disjoint and lie inside the range; leftover streams at its end, fewer than `size`, belong to
    /// no chunk.
    ///
    /// # Errors
    ///
    /// [`RngError::EmptyChunk`] when `size` is zero, and [`RngError::TooFewStreams`] when the chunk
    /// reaches past the last stream.
    pub const fn chunk(self, index: u64, size: u64) -> Result<StreamRange, RngError> {
        // size − 1, which fails exactly when there is no stream in a chunk.
        let Some(size_less_one) = size.checked_sub(1) else {
            return Err(RngError::EmptyChunk);
        };
        // The chunk's first id, first + index · size, and its last, that plus size − 1, each step
        // checked. A step past 2^64 − 1 is past `last`, which is at most that, so the chunk reaches
        // past the range exactly when a step fails or its last id is above `last`; and from a first
        // id at most its last, the chunk lies inside the range.
        let start = match index.checked_mul(size) {
            Some(offset) => self.first.checked_add(offset),
            None => None,
        };
        let end = match start {
            Some(start) => start.checked_add(size_less_one),
            None => None,
        };
        match (start, end) {
            (Some(first), Some(last)) if last <= self.last => Ok(StreamRange {
                seed: self.seed,
                first,
                last,
            }),
            _ => Err(self.too_few()),
        }
    }

    /// The error for a request past the end of this range.
    const fn too_few(self) -> RngError {
        RngError::TooFewStreams {
            first: StreamId(self.first),
            last: StreamId(self.last),
        }
    }
}

/// A position in one Philox stream, drawing its words in order.
///
/// Not `Copy`, on purpose: a copy would draw the same numbers again. `clone` is there for the rare
/// caller that means it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Stream {
    philox: Philox,
    /// How many words have been drawn since the start of the stream: at most [`WORDS_PER_STREAM`].
    drawn: u128,
    /// The block that holds word `drawn` (`drawn / 4`), or the stream's last block once every word
    /// has been drawn. Kept up to date by every draw, so two streams at the same position are equal.
    block: [u32; 4],
}

impl Stream {
    /// The generator this stream draws from.
    #[must_use]
    pub const fn philox(&self) -> Philox {
        self.philox
    }

    /// How many 32-bit words have been drawn since the start of the stream, counting the words a
    /// [`Philox::stream_from`] skipped.
    #[must_use]
    pub const fn position(&self) -> u128 {
        self.drawn
    }

    /// How many 32-bit words are left.
    #[must_use]
    pub const fn remaining(&self) -> u128 {
        // `drawn` never exceeds WORDS_PER_STREAM, so `abs_diff` is WORDS_PER_STREAM − drawn,
        // exactly.
        WORDS_PER_STREAM.abs_diff(self.drawn)
    }

    /// The next 32-bit word.
    ///
    /// # Errors
    ///
    /// [`RngError::Exhausted`] when every word has been drawn; the stream is unchanged.
    pub const fn next_u32(&mut self) -> Result<u32, RngError> {
        // The word to give is word `drawn`, which the stream holds exactly when drawn + 1 ≤ 2^66.
        // The sum cannot fail, since `drawn` never exceeds 2^66; a sum past 2^128 − 1 would be past
        // the end all the same.
        let next = match self.drawn.checked_add(1) {
            Some(next) if next <= WORDS_PER_STREAM => next,
            _ => return Err(self.exhausted()),
        };
        let [w0, w1, w2, w3] = self.block;
        // Its place in its block, drawn mod 4: the two low bits.
        let place = self.drawn & 3;
        let word = match place {
            0 => w0,
            1 => w1,
            2 => w2,
            _ => w3,
        };
        self.drawn = next;
        if place == 3 {
            // That was its block's last word, so word `next` starts block next / 4, the shift right
            // by two places. The block exists when next / 4 < 2^64, its high half zero, which is
            // every case but the stream's end (next = 2^66, next / 4 = 2^64), where `block` keeps
            // the stream's last block.
            let [index, beyond] = split_u128(next >> 2);
            if beyond == 0 {
                self.block = self.philox.block(index);
            }
        }
        Ok(word)
    }

    /// The next 64-bit word: the next 32-bit word, plus the one after it times 2^32.
    ///
    /// # Errors
    ///
    /// [`RngError::Exhausted`] when fewer than two words are left; the stream is unchanged.
    pub const fn next_u64(&mut self) -> Result<u64, RngError> {
        // Checked first, so a failed draw takes nothing; after it, both words below are there.
        if self.remaining() < 2 {
            return Err(self.exhausted());
        }
        let low = match self.next_u32() {
            Ok(word) => word,
            Err(e) => return Err(e),
        };
        let high = match self.next_u32() {
            Ok(word) => word,
            Err(e) => return Err(e),
        };
        Ok(join_u32(low, high))
    }

    /// A double in [0, 1): the top 53 bits of [`Stream::next_u64`], times 2^−53, as a [`Real`].
    ///
    /// Exact: a whole number below 2^53 converts to `f64` exactly, and multiplying by a power of two
    /// is exact. So the result is k · 2^−53 for a whole k in [0, 2^53), each k from exactly 2^11
    /// words, never rounded, never 1.0 and never −0.
    ///
    /// # Errors
    ///
    /// [`RngError::Exhausted`] when fewer than two words are left, and [`RngError::Defect`] should
    /// [`Real::new`] refuse the double, which it cannot (the proof is in the code). Either way the
    /// stream is unchanged.
    pub const fn next_f64(&mut self) -> Result<Real, RngError> {
        let drawn = self.drawn;
        let block = self.block;
        let word = match self.next_u64() {
            Ok(word) => word,
            Err(e) => return Err(e),
        };
        // k = word >> 11 < 2^53 as a double, exactly: its high part (below 2^21) times 2^32, plus
        // its low 32 bits. Each `u32` converts to `f64` exactly; the product and the sum are whole
        // numbers below 2^53, which are all doubles, so neither rounds; and times 2^−53 only moves
        // the exponent, since k · 2^−53 is 0 or at least 2^−53, a normal double.
        let [low, high] = split_u64(word >> 11);
        let x = (high as f64 * TWO_POW_32 + low as f64) * TWO_POW_MINUS_53;
        let Ok(x) = Real::new(x) else {
            // Unreachable: x is k · 2^−53 with k below 2^53, so finite. Reported, not answered.
            self.drawn = drawn;
            self.block = block;
            return Err(RngError::Defect);
        };
        Ok(x)
    }

    /// A whole number in [0, `bound`), every value equally likely: Lemire's method on
    /// [`Stream::next_u64`] words (see the module documentation).
    ///
    /// Draws until a word is accepted. Each word is rejected with probability (2^64 mod `bound`) /
    /// 2^64, below one half, and the loop ends because the stream is finite. The remainder
    /// 2^64 mod `bound` is computed at most once per call.
    ///
    /// # Errors
    ///
    /// [`RngError::ZeroBound`] when `bound` is zero, [`RngError::Exhausted`] when the stream runs
    /// out first, and [`RngError::Defect`] should a step of Lemire's method that cannot fail fail
    /// (the proofs are in `lemire`). Every way, the stream is unchanged.
    pub const fn below(&mut self, bound: u64) -> Result<u64, RngError> {
        let Some(bound) = NonZeroU64::new(bound) else {
            return Err(RngError::ZeroBound);
        };
        let drawn = self.drawn;
        let block = self.block;
        let mut threshold = None;
        let failed = loop {
            match self.next_u64() {
                Ok(word) => match lemire(word, bound, &mut threshold) {
                    Ok(Some(value)) => return Ok(value),
                    Ok(None) => {}
                    Err(e) => break e,
                },
                Err(e) => break e,
            }
        };
        self.drawn = drawn;
        self.block = block;
        Err(failed)
    }

    /// A whole number in [`low`, `high`], both ends included, every value equally likely:
    /// `low + below(high − low + 1)`, or [`Stream::next_u64`] itself when the range is all of `u64`.
    ///
    /// # Errors
    ///
    /// [`RngError::LowAboveHigh`] when `low > high`, [`RngError::Exhausted`] when the stream runs
    /// out first, and [`RngError::Defect`] should a step that cannot fail fail (the proofs are in
    /// the code and in `lemire`). Every way, the stream is unchanged.
    pub const fn range_inclusive(&mut self, low: u64, high: u64) -> Result<u64, RngError> {
        // high − low, which fails exactly when low is above high.
        let Some(span) = high.checked_sub(low) else {
            return Err(RngError::LowAboveHigh { low, high });
        };
        // The number of values, span + 1, which fails exactly when the range is all of `u64`:
        // Lemire's method with n = 2^64 accepts every word and returns it unchanged.
        let Some(values) = span.checked_add(1) else {
            return self.next_u64();
        };
        let drawn = self.drawn;
        let block = self.block;
        let value = match self.below(values) {
            Ok(value) => value,
            Err(e) => return Err(e),
        };
        // value ≤ span = high − low, so low + value ≤ high and the sum never fails. Should it fail
        // all the same, that is reported, not answered.
        let Some(value) = low.checked_add(value) else {
            self.drawn = drawn;
            self.block = block;
            return Err(RngError::Defect);
        };
        Ok(value)
    }

    /// The error for this stream running out.
    const fn exhausted(&self) -> RngError {
        RngError::Exhausted {
            stream: self.philox.stream,
        }
    }
}

/// Lemire's method for one 64-bit word: the high half of `word · bound`, or `None` when the word is
/// rejected because the low half is below 2^64 mod `bound`.
///
/// `threshold` caches 2^64 mod `bound` across the words of one call: it starts as `None`, and is
/// computed the first time a low half falls below `bound` (only then can the word be rejected,
/// because the threshold is itself below `bound`), and reused after that. So one call computes the
/// remainder at most once, as the paper's method does.
///
/// Why this is unbiased: Lemire proves that, as the word runs over all 2^64 values, each r in
/// [0, `bound`) is the high half of exactly ⌊2^64 / bound⌋ words whose low half is at least
/// 2^64 mod `bound`, so every r is returned by the same number of words. The unit tests check this
/// by walking whole preimages.
///
/// # Errors
///
/// [`RngError::Defect`] should the product or the threshold fail, which neither can (the proofs
/// are in the code).
const fn lemire(
    word: u64,
    bound: NonZeroU64,
    threshold: &mut Option<u64>,
) -> Result<Option<u64>, RngError> {
    let bound = bound.get();
    // The exact product, the paper's 128-bit `m`: both factors are below 2^64 (the widening casts
    // are lossless), so it is at most (2^64 − 1)^2 < 2^128 and `checked_mul` never fails. Should it
    // fail all the same, that is reported, not answered.
    let Some(product) = (word as u128).checked_mul(bound as u128) else {
        return Err(RngError::Defect);
    };
    // Its low half, the product modulo 2^64, which the method compares, and its high half, which it
    // returns.
    let [low, high] = split_u128(product);
    if low < bound {
        let limit = if let Some(limit) = *threshold {
            limit
        } else {
            // 2^64 mod bound, computed in 64 bits as (2^64 − bound) mod bound, as the paper
            // computes it (`-s % s` on 64-bit unsigned integers, whose negation C defines modulo
            // 2^64): `wrapping_neg` is that negation, by the method's definition. `checked_rem`
            // fails only for a zero divisor, and the bound is not zero; should it fail all the
            // same, that is reported, not answered.
            let Some(limit) = bound.wrapping_neg().checked_rem(bound) else {
                return Err(RngError::Defect);
            };
            *threshold = Some(limit);
            limit
        };
        if low < limit {
            return Ok(None);
        }
    }
    Ok(Some(high))
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "tests may use native arithmetic and casts"
)]
mod tests {
    use super::*;

    /// The byte splits and the join are the shifts and casts they replace, at the edges and in
    /// between.
    #[test]
    fn splits_and_join_are_exact() {
        let words = [
            0u64,
            1,
            0xffff_ffff,
            1 << 32,
            0x0123_4567_89ab_cdef,
            0x8000_0000_0000_0000,
            u64::MAX,
        ];
        for x in words {
            let [low, high] = split_u64(x);
            assert_eq!((low, high), (x as u32, (x >> 32) as u32), "{x:#x}");
            assert_eq!(join_u32(low, high), x, "{x:#x}");
        }
        for x in [
            0u128,
            1,
            u128::from(u64::MAX),
            1 << 64,
            (1 << 66) - 1,
            1 << 66,
            0x0123_4567_89ab_cdef_fedc_ba98_7654_3210,
            u128::MAX,
        ] {
            assert_eq!(split_u128(x), [x as u64, (x >> 64) as u64], "{x:#x}");
        }
    }

    /// The Weyl constants are what the paper says they are, in exact integer arithmetic.
    #[test]
    fn weyl_constants_are_the_golden_ratio_and_root_three() {
        let two32 = 1u128 << 32;
        let two64 = 1u128 << 64;
        // W0 = ⌊2^32 (√5 − 1)/2⌋  ⇔  (2·W0 + 2^32)^2 ≤ 5 · 2^64 < (2·W0 + 2 + 2^32)^2.
        let w0 = u128::from(W0);
        assert!((2 * w0 + two32).pow(2) <= 5 * two64);
        assert!(5 * two64 < (2 * w0 + 2 + two32).pow(2));
        // W1 = ⌊2^32 (√3 − 1)⌋  ⇔  (W1 + 2^32)^2 ≤ 3 · 2^64 < (W1 + 1 + 2^32)^2.
        let w1 = u128::from(W1);
        assert!((w1 + two32).pow(2) <= 3 * two64);
        assert!(3 * two64 < (w1 + 1 + two32).pow(2));
    }

    /// Both halves of `mulhilo` match the exact product, at the edges and in between.
    #[test]
    fn mulhilo_is_the_exact_product() {
        let values = [
            0,
            1,
            2,
            0x7fff_ffff,
            0x8000_0000,
            0xffff_fffe,
            0xffff_ffff,
            M0,
            M1,
            W0,
            W1,
            0x1234_5678,
        ];
        for a in values {
            for b in values {
                let exact = u64::from(a) * u64::from(b);
                let (high, low) = mulhilo(a, b);
                assert_eq!(
                    (u64::from(high) << 32) | u64::from(low),
                    exact,
                    "{a:#x} {b:#x}"
                );
            }
        }
    }

    /// The threshold computed in 64 bits is 2^64 mod bound.
    #[test]
    fn lemire_threshold_is_two_to_the_64_mod_bound() {
        for bound in [
            1u64,
            2,
            3,
            6,
            7,
            1 << 32,
            (1 << 32) + 1,
            (1 << 63) - 1,
            1 << 63,
            (1 << 63) + 1,
            u64::MAX - 1,
            u64::MAX,
        ] {
            let exact = (1u128 << 64) % u128::from(bound);
            let t = bound.wrapping_neg().checked_rem(bound).unwrap();
            assert_eq!(u128::from(t), exact, "{bound}");
            // The cached threshold `lemire` stores is the same value: word 0 has low half 0, which
            // is below every bound, so it always computes the threshold.
            let mut cache = None;
            let _ = lemire(0, NonZeroU64::new(bound).unwrap(), &mut cache).unwrap();
            assert_eq!(cache.map(u128::from), Some(exact), "{bound}");
        }
    }

    /// The remainder is computed only once a low half falls below the bound, and then reused: a
    /// word whose low half is at least the bound leaves the cache empty, and a cache that holds a
    /// value is never recomputed (a planted value is used as it is).
    #[test]
    fn lemire_computes_the_threshold_at_most_once() {
        let bound = NonZeroU64::new(3).unwrap();
        // Word 2^63: 3 · 2^63 = 2^64 + 2^63, so the low half 2^63 is not below 3.
        let mut cache = None;
        assert_eq!(lemire(1 << 63, bound, &mut cache), Ok(Some(1)));
        assert_eq!(cache, None);
        // Word 0: low half 0 < 3, so the threshold 2^64 mod 3 = 1 is computed and 0 rejected.
        assert_eq!(lemire(0, bound, &mut cache), Ok(None));
        assert_eq!(cache, Some(1));
        // A planted threshold of 0 accepts word 0: the cache is read, not recomputed.
        let mut planted = Some(0);
        assert_eq!(lemire(0, bound, &mut planted), Ok(Some(0)));
        assert_eq!(planted, Some(0));
    }

    /// Counts the accepted words whose value is `r`, over every word that maps to `r`.
    fn accepted_preimages(r: u64, bound: NonZeroU64) -> u64 {
        let n = u128::from(bound.get());
        // The words x with ⌊x · n / 2^64⌋ = r are those in [⌈r · 2^64 / n⌉, ⌈(r + 1) · 2^64 / n⌉).
        let first = (u128::from(r) << 64).div_ceil(n);
        let end = ((u128::from(r) + 1) << 64).div_ceil(n);
        let mut count = 0;
        let mut cache = None;
        for x in first..end {
            let x = u64::try_from(x).unwrap();
            if let Some(v) = lemire(x, bound, &mut cache).unwrap() {
                assert_eq!(v, r);
                count += 1;
            }
        }
        count
    }

    /// Exhaustive over each value's preimage: every value in the range keeps exactly
    /// ⌊2^64 / bound⌋ accepted words, which is what "no modulo bias" means. Bounds are chosen so
    /// that each preimage is small enough to walk: about 2^16 words for bounds near 2^48, and one
    /// or two for bounds above 2^63.
    #[test]
    fn lemire_accepts_the_same_number_of_words_for_every_value() {
        let bounds = [
            (1u64 << 48) + 1,
            (1 << 48) + 12_345,
            3 << 46,
            (1 << 49) - 1,
            (1 << 63) + 1,
            (1 << 63) + (1 << 62) + 7,
            u64::MAX - 1,
            u64::MAX,
        ];
        for bound in bounds {
            let nz = NonZeroU64::new(bound).unwrap();
            let expected = u64::try_from((1u128 << 64) / u128::from(bound)).unwrap();
            let mut rs = vec![0, 1, 2, bound / 3, bound / 2, bound - 2, bound - 1];
            // A spread of values in between, from a fixed multiplicative sequence.
            let mut r = 0x9e37_79b9_7f4a_7c15u64;
            for _ in 0..64 {
                r = r.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                rs.push(r % bound);
            }
            for r in rs {
                assert_eq!(
                    accepted_preimages(r, nz),
                    expected,
                    "bound {bound:#x}, value {r:#x}"
                );
            }
        }
    }

    /// A power-of-two bound rejects nothing and returns the word's top bits.
    #[test]
    fn lemire_power_of_two_bounds_take_the_top_bits() {
        for k in 0..64 {
            let bound = NonZeroU64::new(1u64 << k).unwrap();
            for word in [
                0u64,
                1,
                0x8000_0000_0000_0000,
                u64::MAX,
                0x0123_4567_89ab_cdef,
            ] {
                let expected = if k == 0 { 0 } else { word >> (64 - k) };
                assert_eq!(
                    lemire(word, bound, &mut None),
                    Ok(Some(expected)),
                    "k {k}, word {word:#x}"
                );
            }
        }
    }

    #[test]
    fn two_pow_minus_53_is_exact() {
        assert_eq!(TWO_POW_MINUS_53.to_bits(), 0x3ca0_0000_0000_0000);
        assert_eq!(TWO_POW_MINUS_53 * 9_007_199_254_740_992.0, 1.0);
    }
}

/// Bounded proofs (chapter 28.7; piece G41): every key, counter, seed and stream.
#[cfg(kani)]
mod proofs {
    use super::*;

    /// Philox4x32-10 never panics or overflows, for every counter and key.
    #[kani::proof]
    fn philox_never_panics() {
        let counter: [u32; 4] = [kani::any(), kani::any(), kani::any(), kani::any()];
        let key: [u32; 2] = [kani::any(), kani::any()];
        let _ = philox4x32_10(counter, key);
    }

    /// A stream's draws never panic or overflow, from any block of any stream of any seed: each
    /// answers or reports that the stream ran out, and a draw that answers moves the position on.
    #[kani::proof]
    fn draws_never_panic() {
        let philox = Philox::new(Seed(kani::any()), StreamId(kani::any()));
        let mut stream = philox.stream_from(kani::any());
        let before = stream.position();
        if stream.next_u32().is_ok() {
            assert!(stream.position() > before);
        }
        let _ = stream.next_u64();
        // A double is drawn whenever the stream has words left, never refused as a defect: the
        // `Real::new` check after the draw cannot fail.
        assert!(!matches!(stream.next_f64(), Err(RngError::Defect)));
    }

    /// Lemire's method never reports a defect, for every word and every bound: the 128-bit
    /// product cannot overflow and the bound is never zero, so both defensive arms are unreachable.
    #[kani::proof]
    fn lemire_never_reports_a_defect() {
        let word: u64 = kani::any();
        let bound: u64 = kani::any();
        kani::assume(bound != 0);
        let mut threshold = None;
        let first = lemire(word, NonZeroU64::new(bound).unwrap(), &mut threshold);
        assert!(!matches!(first, Err(RngError::Defect)));
        // With the threshold cached, a second word is judged the same way.
        let second = lemire(kani::any(), NonZeroU64::new(bound).unwrap(), &mut threshold);
        assert!(!matches!(second, Err(RngError::Defect)));
    }

    /// Cutting a seed's streams into chunks never panics or overflows, and a chunk that is made
    /// lies inside the range it was cut from.
    #[kani::proof]
    fn stream_ranges_cut_without_overflow() {
        let all = StreamRange::all(Seed(kani::any()));
        let (index, size): (u64, u64) = (kani::any(), kani::any());
        if let Ok(chunk) = all.chunk(index, size) {
            assert!(chunk.first().0 >= all.first().0 && chunk.last().0 <= all.last().0);
            let _ = chunk.philox(kani::any());
        }
    }
}
