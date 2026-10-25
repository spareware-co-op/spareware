//! The random generator's golden hashes: a fixed set of streams, drawn by every draw function, and
//! a fixed set of stream ranges, cut every way, hashed with SHA-256, must give the same digests on
//! every target.
//!
//! A randomised kernel method (RANSAC fitting, sampling) must give the same bits on every machine
//! (chapter 21; the randomised-method rule in the kernel's `AGENTS.md`), so its random numbers must
//! too. This test holds `geom_num::rng` to that. Its corpus:
//!
//! - **Streams.** Six seeds (zero, one, all ones, the top bit alone, "SPAREKIT" and the π key of
//!   Random123's known answers) times six stream ids (zero, one, both sides of the 32-bit boundary,
//!   the π counter's high half and all ones) times six starting blocks (0, 1, both sides of the
//!   32-bit boundary, and the last two blocks' neighbourhood, where draws run out).
//! - **Draws on each stream**, in a fixed order: five `next_u32` (so the 64-bit draws that follow
//!   straddle block boundaries), three `next_u64`, `below` for sixteen bounds from 0 (an error)
//!   through powers of two and their neighbours to `u64::MAX`, `range_inclusive` for six ranges
//!   (one empty, one the whole of `u64`), and four `next_f64`. Streams near their end run out
//!   part-way, so the errors are hashed too.
//! - **The raw block function** on 4,096 counters and keys from a fixed linear congruential
//!   sequence, and **one long stream** of 16,384 words.
//! - **Stream ranges**, with a golden of their own: for each seed, the whole stream space, a chunk
//!   of it, a chunk of that and a range of one stream, each cut by eight chunk sizes (0, an error,
//!   to `u64::MAX`) at eight indices, and each asked for streams from 0 to one past its last.
//!
//! Every result is written in a canonical little-endian byte encoding behind a one-byte tag naming
//! the draw, with the stream's position after each draw. The recorded bytes are hashed with
//! SHA-256 (FIPS 180-4), the hash chapters 21 and 28.13 specify for golden hashes, from the
//! `sha2` crate (piece G12a), which `determinism.rs` holds to the standard's test vectors.
//! Doubles go in as their bits. An error is written as its own tag and fields, so a
//! target that fails a draw another target makes shows.
//!
//! **From FNV-1a to SHA-256.** These goldens were first 64-bit FNV-1a values; chapter 28.13 asks
//! for SHA-256. Before the piece merged, the hash function changed and nothing else did: the
//! corpus and its encoding are the same, byte for byte. The test
//! `rng_goldens_moved_to_sha256_over_the_same_bytes` shows it on every target: it records each
//! corpus once and hashes that one recording both ways, giving the former FNV-1a value and the
//! SHA-256 golden. The FNV-1a values are kept beside the goldens, held to the same promise, as the
//! record that the bytes did not move.
//!
//! **The same constants must hold on every target**: Linux, Windows and macOS on x86-64 and arm64,
//! and wasm32-wasip1 under wasmtime, in debug and release, with overflow checks on or off, and
//! built for the machine's own CPU. A changed digest is a bug: some target draws different numbers,
//! so a randomised method would build different geometry on different machines. Apart from that
//! one change of hash function, made before the piece merged, these constants are never edited.
//! Under rule 4 of the randomised-method rule nothing in `rng` changes in place: a new generator,
//! layout or draw definition goes beside the old one, and its corpus gets a golden of its own
//! beside these, as the stream ranges' did. A bigger corpus is likewise a new test with its own
//! golden. Never update a constant to make one target pass.
//!
//! The draw test prints a digest per section, so the CI logs of two targets show which part
//! differs. Both golden tests check that their corpus still reaches every outcome it is built to
//! reach (each error, a rejected word inside `below`, a draw that straddles blocks, a range too
//! small), so a corpus that silently stopped testing the hard cases fails. This file is separate
//! from `determinism.rs`, which holds the rest of the numbers layer to the same promise.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them to build their oracles, models and generators.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "oracles, models and generators; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken corpus must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;

use geom_num::Real;
use geom_num::rng::{Philox, RngError, Seed, Stream, StreamId, StreamRange, philox4x32_10};
use sha2::{Digest as _, Sha256};

/// The golden digest of the draw corpus: SHA-256, in hexadecimal.
///
/// The same constant must hold on every target: Linux, Windows and macOS on x86-64 and arm64, and
/// wasm32-wasip1, in debug and release, with overflow checks on or off. A change is a bug; this
/// constant is never edited (see the module documentation).
const GOLDEN: &str = "706bf19f86a2ac5ba9be55866a1a5b151702dd6707b9b3b67a5c4c347cbabb7f";

/// The golden digest of the stream-range corpus, held to the same promise as [`GOLDEN`].
const GOLDEN_RANGES: &str = "4d2c3a986913d7751a8f49eb3a7e5340eb973b591154c7acdaaf0ad47b6472f7";

/// The draw corpus's golden before the move to SHA-256: the FNV-1a 64 hash of the same bytes.
/// Kept, and held to the same promise as [`GOLDEN`], to show the move changed only the hash.
const FNV_GOLDEN: u64 = 0x23f5_7fe3_69e7_16ab;

/// The stream-range corpus's golden before the move to SHA-256, kept as [`FNV_GOLDEN`] is.
const FNV_GOLDEN_RANGES: u64 = 0xbc77_bb37_1b9e_60c5;

/// The draw corpus makes at least this many draws.
const MIN_CALLS: u64 = 20_000;

/// The stream-range corpus makes at least this many calls.
const MIN_RANGE_CALLS: u64 = 1_500;

/// FNV-1a 64-bit offset basis (Fowler, Noll and Vo).
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// FNV-1a 64-bit prime, 2^40 + 2^8 + 0xb3.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Multiplier of the raw-block corpus sequence: Knuth's MMIX linear congruential generator modulo
/// 2^64 (*The Art of Computer Programming*, volume 2, section 3.3.4). It only makes inputs; it is
/// never a source of random numbers for the kernel.
const LCG_MUL: u64 = 6_364_136_223_846_793_005;

/// Increment of the corpus sequence (Knuth's MMIX).
const LCG_ADD: u64 = 1_442_695_040_888_963_407;

/// The seeds.
const SEEDS: [u64; 6] = [
    0,
    1,
    u64::MAX,
    0x8000_0000_0000_0000,
    // "SPAREKIT" in ASCII.
    0x5350_4152_454b_4954,
    // The π key of Random123's known answers, as this crate's layout reads it.
    0x299f_31d0_a409_3822,
];

/// The stream ids.
const STREAMS: [u64; 6] = [
    0,
    1,
    0xffff_ffff,
    0x1_0000_0000,
    // The high half of the π counter of Random123's known answers.
    0x0370_7344_1319_8a2e,
    u64::MAX,
];

/// The starting blocks. The last two leave 16 and 4 words, so draws run out part-way.
const STARTS: [u64; 6] = [0, 1, 0xffff_ffff, 0x1_0000_0000, u64::MAX - 3, u64::MAX];

/// The bounds given to `below`; 0 is an error.
const BOUNDS: [u64; 16] = [
    1,
    2,
    3,
    6,
    7,
    10,
    1000,
    0xffff_ffff,
    1 << 32,
    (1 << 32) + 1,
    0x0123_4567_89ab_cdef,
    1 << 63,
    (1 << 63) + 1,
    u64::MAX - 1,
    u64::MAX,
    0,
];

/// The ranges given to `range_inclusive`; (5, 4) is an error.
const RANGES: [(u64, u64); 6] = [
    (0, 0),
    (5, 4),
    (10, 20),
    (u64::MAX - 1, u64::MAX),
    (0, u64::MAX),
    (1, u64::MAX),
];

/// FNV-1a, 64 bits: for each byte, xor it into the state, then multiply by the prime modulo 2^64.
/// The goldens' hash before SHA-256; now it only reproduces [`FNV_GOLDEN`] and
/// [`FNV_GOLDEN_RANGES`].
#[derive(Clone, Copy)]
struct Fnv(u64);

impl Fnv {
    /// The empty hash, the offset basis.
    const fn new() -> Fnv {
        Fnv(FNV_OFFSET)
    }

    /// Hashes `bytes`, in order.
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(FNV_PRIME);
        }
    }
}

/// The draws the corpus makes. A draw writes its tag first, so the byte stream says which draw
/// each result came from. The tags are part of the encoding: never renumber them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Op {
    Block = 1,
    NextU32 = 2,
    NextU64 = 3,
    NextF64 = 4,
    Below = 5,
    RangeInclusive = 6,
    StreamStart = 7,
    RangeChunk = 8,
    RangeStream = 9,
}

/// What a draw can come to, for the reached-outcomes check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Outcome {
    Ok(Op),
    Exhausted(Op),
    ZeroBound,
    LowAboveHigh,
    EmptyChunk,
    TooFewStreams(Op),
    /// `below` rejected at least one word before accepting one.
    Rejected,
    /// A `next_u64` took its two words from different blocks.
    Straddled,
}

/// Writes results into the recording and keeps count.
struct Recorder {
    /// Every byte written, in order: the recording the hashes read.
    recorded: Vec<u8>,
    /// Where the current section starts in `recorded`.
    section_start: usize,
    calls: u64,
    section_calls: u64,
    seen: BTreeSet<Outcome>,
    /// Each finished section: name, calls, bytes and SHA-256 digest.
    report: Vec<(&'static str, u64, usize, [u8; 32])>,
}

impl Recorder {
    fn new() -> Recorder {
        Recorder {
            recorded: Vec::new(),
            section_start: 0,
            calls: 0,
            section_calls: 0,
            seen: BTreeSet::new(),
            report: Vec::new(),
        }
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.recorded.extend_from_slice(bytes);
    }

    fn op(&mut self, op: Op) {
        self.bytes(&[op as u8]);
        self.calls += 1;
        self.section_calls += 1;
    }

    fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }

    fn u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }

    fn u128(&mut self, v: u128) {
        self.bytes(&v.to_le_bytes());
    }

    /// A draw's result: tag 0 and the value's bytes, or an error's tag and fields; then the
    /// stream's position after the draw.
    fn result<T>(
        &mut self,
        op: Op,
        result: Result<T, RngError>,
        stream: &Stream,
        bytes: impl Fn(T) -> Vec<u8>,
    ) {
        self.op(op);
        match result {
            Ok(v) => {
                self.bytes(&[0]);
                self.bytes(&bytes(v));
                self.seen.insert(Outcome::Ok(op));
            }
            Err(RngError::Exhausted { stream }) => {
                self.bytes(&[1]);
                self.u64(stream.0);
                self.seen.insert(Outcome::Exhausted(op));
            }
            Err(RngError::ZeroBound) => {
                self.bytes(&[2]);
                self.seen.insert(Outcome::ZeroBound);
            }
            Err(RngError::LowAboveHigh { low, high }) => {
                self.bytes(&[3]);
                self.u64(low);
                self.u64(high);
                self.seen.insert(Outcome::LowAboveHigh);
            }
            // `RngError` is non-exhaustive; no draw returns the stream-range errors.
            Err(other) => panic!("a draw returned {other:?}, which no draw can"),
        }
        self.u128(stream.position());
    }

    /// A stream range's result: tag 0, its first and last ids and the first block of its first
    /// stream, or an error's tag and fields.
    fn range(&mut self, op: Op, result: Result<StreamRange, RngError>) {
        self.op(op);
        match result {
            Ok(range) => {
                self.bytes(&[0]);
                self.u64(range.first().0);
                self.u64(range.last().0);
                self.u128(range.count());
                let first = range.philox(0).expect("a range is never empty");
                for w in first.block(0) {
                    self.u32(w);
                }
                self.seen.insert(Outcome::Ok(op));
            }
            Err(e) => self.range_error(op, e),
        }
    }

    /// A stream taken from a range: tag 0, its seed and id and its first block, or an error.
    fn range_stream(&mut self, result: Result<Philox, RngError>) {
        self.op(Op::RangeStream);
        match result {
            Ok(philox) => {
                self.bytes(&[0]);
                self.u64(philox.seed().0);
                self.u64(philox.stream_id().0);
                for w in philox.block(0) {
                    self.u32(w);
                }
                self.seen.insert(Outcome::Ok(Op::RangeStream));
            }
            Err(e) => self.range_error(Op::RangeStream, e),
        }
    }

    /// A stream-range error: its tag and fields.
    fn range_error(&mut self, op: Op, e: RngError) {
        match e {
            RngError::EmptyChunk => {
                self.bytes(&[4]);
                self.seen.insert(Outcome::EmptyChunk);
            }
            RngError::TooFewStreams { first, last } => {
                self.bytes(&[5]);
                self.u64(first.0);
                self.u64(last.0);
                self.seen.insert(Outcome::TooFewStreams(op));
            }
            other => panic!("a stream range returned {other:?}, which no range operation can"),
        }
    }

    fn end_section(&mut self, name: &'static str) {
        let section = &self.recorded[self.section_start..];
        self.report
            .push((name, self.section_calls, section.len(), sha256(section)));
        self.section_start = self.recorded.len();
        self.section_calls = 0;
    }
}

/// The SHA-256 digest of `bytes`.
fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// The FNV-1a 64 hash of `bytes`.
fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut h = Fnv::new();
    h.write(bytes);
    h.0
}

/// Every outcome the corpus is built to reach.
fn required() -> Vec<Outcome> {
    let mut out = vec![
        Outcome::ZeroBound,
        Outcome::LowAboveHigh,
        Outcome::Rejected,
        Outcome::Straddled,
        Outcome::Ok(Op::Block),
    ];
    for op in [
        Op::NextU32,
        Op::NextU64,
        Op::NextF64,
        Op::Below,
        Op::RangeInclusive,
    ] {
        out.push(Outcome::Ok(op));
        out.push(Outcome::Exhausted(op));
    }
    out
}

/// Runs the fixed script of draws on one stream.
fn script(s: &mut Stream, r: &mut Recorder) {
    let u32_bytes = |v: u32| v.to_le_bytes().to_vec();
    let u64_bytes = |v: u64| v.to_le_bytes().to_vec();
    let f64_bytes = |v: f64| v.to_bits().to_le_bytes().to_vec();

    r.op(Op::StreamStart);
    r.u64(s.philox().seed().0);
    r.u64(s.philox().stream_id().0);
    r.u128(s.position());

    for _ in 0..5 {
        let v = s.next_u32();
        r.result(Op::NextU32, v, s, u32_bytes);
    }
    for _ in 0..3 {
        let before = s.position();
        let v = s.next_u64();
        if v.is_ok() && before / 4 != (before + 1) / 4 {
            r.seen.insert(Outcome::Straddled);
        }
        r.result(Op::NextU64, v, s, u64_bytes);
    }
    for bound in BOUNDS {
        let before = s.position();
        let v = s.below(bound);
        if v.is_ok() && s.position() - before > 2 {
            r.seen.insert(Outcome::Rejected);
        }
        r.u64(bound);
        r.result(Op::Below, v, s, u64_bytes);
    }
    for (low, high) in RANGES {
        let v = s.range_inclusive(low, high);
        r.u64(low);
        r.u64(high);
        r.result(Op::RangeInclusive, v, s, u64_bytes);
    }
    for _ in 0..4 {
        // The double's bits, as before `next_f64` returned a `Real`: the golden hash is unchanged.
        let v = s.next_f64().map(Real::get);
        if let Ok(x) = v {
            assert!((0.0..1.0).contains(&x), "next_f64 gave {x}");
        }
        r.result(Op::NextF64, v, s, f64_bytes);
    }
}

/// Every seed, stream id and starting block, each run through the script.
fn streams(r: &mut Recorder) {
    for seed in SEEDS {
        for stream in STREAMS {
            let philox = Philox::new(Seed(seed), StreamId(stream));
            for start in STARTS {
                let mut s = philox.stream_from(start);
                script(&mut s, r);
            }
        }
    }
}

/// The raw block function on counters and keys from a fixed sequence.
fn raw_blocks(r: &mut Recorder) {
    let mut state = 0x243f_6a88_85a3_08d3_u64;
    let mut next = || {
        state = state.wrapping_mul(LCG_MUL).wrapping_add(LCG_ADD);
        // The high half: the low bits of this sequence are weak.
        (state >> 32) as u32
    };
    for _ in 0..4_096 {
        let counter = [next(), next(), next(), next()];
        let key = [next(), next()];
        let block = philox4x32_10(counter, key);
        r.op(Op::Block);
        for w in counter.into_iter().chain(key).chain(block) {
            r.u32(w);
        }
        r.seen.insert(Outcome::Ok(Op::Block));
    }
}

/// The chunk sizes given to `StreamRange::chunk`; 0 is an error.
const SIZES: [u64; 8] = [0, 1, 2, 3, 0xffff_ffff, 1 << 32, 1 << 63, u64::MAX];

/// The chunk and stream indices given to `StreamRange::chunk` and `StreamRange::philox`.
const INDICES: [u64; 8] = [
    0,
    1,
    2,
    0xffff_ffff,
    1 << 32,
    (1 << 63) - 1,
    1 << 63,
    u64::MAX,
];

/// Stream ranges: for each seed, the whole space, a chunk of it, a chunk of that and a range of one
/// stream, each cut by every size and index above, and each asked for streams up to one past its
/// last.
fn ranges(r: &mut Recorder) {
    for seed in SEEDS {
        let all = StreamRange::all(Seed(seed));
        let middle = all.chunk(5, 1 << 40);
        let parents = [
            Ok(all),
            middle,
            middle.and_then(|c| c.chunk(3, 1000)),
            all.chunk(u64::MAX, 1),
        ];
        for parent in parents {
            r.range(Op::RangeChunk, parent);
            let parent = parent.expect("every parent range fits");
            for size in SIZES {
                for index in INDICES {
                    r.u64(size);
                    r.u64(index);
                    r.range(Op::RangeChunk, parent.chunk(index, size));
                }
            }
            for index in INDICES {
                r.u64(index);
                r.range_stream(parent.philox(index));
            }
            let last =
                u64::try_from(parent.count() - 1).expect("a range holds at most 2^64 streams");
            r.u64(last);
            r.range_stream(parent.philox(last));
            if let Some(past) = last.checked_add(1) {
                r.u64(past);
                r.range_stream(parent.philox(past));
            }
        }
    }
}

/// Every outcome the stream-range corpus is built to reach.
fn required_ranges() -> Vec<Outcome> {
    vec![
        Outcome::Ok(Op::RangeChunk),
        Outcome::Ok(Op::RangeStream),
        Outcome::EmptyChunk,
        Outcome::TooFewStreams(Op::RangeChunk),
        Outcome::TooFewStreams(Op::RangeStream),
    ]
}

/// One long stream, word by word.
fn long_stream(r: &mut Recorder) {
    let mut s = Philox::new(Seed(0x5350_4152_454b_4954), StreamId(7)).stream();
    for _ in 0..16_384 {
        let v = s.next_u32();
        r.result(Op::NextU32, v, &s, |v: u32| v.to_le_bytes().to_vec());
    }
}

#[test]
fn fnv1a_64_matches_its_published_test_vectors() {
    let hash = |s: &str| {
        let mut h = Fnv::new();
        h.write(s.as_bytes());
        h.0
    };
    assert_eq!(hash(""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(hash("a"), 0xaf63_dc4c_8601_ec8c);
    assert_eq!(hash("foobar"), 0x8594_4171_f739_67e8);
}

/// The draw corpus, recorded section by section.
fn draw_corpus() -> Recorder {
    let mut r = Recorder::new();
    streams(&mut r);
    r.end_section("streams");
    raw_blocks(&mut r);
    r.end_section("blocks");
    long_stream(&mut r);
    r.end_section("long");
    r
}

/// The stream-range corpus, recorded.
fn range_corpus() -> Recorder {
    let mut r = Recorder::new();
    ranges(&mut r);
    r.end_section("ranges");
    r
}

/// The golden hash. The same constant must hold on every target; a change is a bug (see the module
/// documentation and `GOLDEN`).
#[test]
fn rng_golden_hash_is_the_same_on_every_target() {
    let r = draw_corpus();

    let hash = hex::encode(sha256(&r.recorded));
    println!(
        "geom-num rng corpus: {} calls, {} bytes",
        r.calls,
        r.recorded.len()
    );
    for (name, calls, bytes, digest) in &r.report {
        println!(
            "  {name:<8} {calls:>6} calls {bytes:>7} bytes  {}",
            hex::encode(digest)
        );
    }
    println!("geom-num rng golden hash: {hash}");

    let missing: Vec<_> = required()
        .into_iter()
        .filter(|k| !r.seen.contains(k))
        .collect();
    assert!(missing.is_empty(), "unreached outcomes: {missing:?}");
    assert!(r.calls >= MIN_CALLS, "only {} calls", r.calls);
    assert_eq!(
        hash, GOLDEN,
        "geom-num's rng golden digest is {hash} here but {GOLDEN} is expected. Either this target \
         draws different numbers (a bug: compare the section digests above with another target's \
         log) or a definition was edited in place, which rule 4 forbids: a new definition goes \
         beside the old one with a golden of its own, and this constant never changes"
    );
}

/// The stream-range golden hash, beside the draws' (rule 4: a new definition gets its own golden).
/// The same constant must hold on every target; a change is a bug.
#[test]
fn rng_stream_range_golden_hash_is_the_same_on_every_target() {
    let r = range_corpus();

    let hash = hex::encode(sha256(&r.recorded));
    println!(
        "geom-num rng stream-range corpus: {} calls, {} bytes",
        r.calls,
        r.recorded.len()
    );
    println!("geom-num rng stream-range golden hash: {hash}");

    let missing: Vec<_> = required_ranges()
        .into_iter()
        .filter(|k| !r.seen.contains(k))
        .collect();
    assert!(missing.is_empty(), "unreached outcomes: {missing:?}");
    assert!(r.calls >= MIN_RANGE_CALLS, "only {} calls", r.calls);
    assert_eq!(
        hash, GOLDEN_RANGES,
        "geom-num's rng stream-range golden digest is {hash} here but {GOLDEN_RANGES} is expected. \
         Either this target cuts ranges differently (a bug) or a definition was edited in place, \
         which rule 4 forbids"
    );
}

/// The goldens moved from FNV-1a 64 to SHA-256 and the corpora did not move: each corpus is
/// recorded once, and that one recording hashes to its former FNV-1a golden and to its SHA-256
/// golden, on every target. So the move changed the hash function and nothing else.
#[test]
fn rng_goldens_moved_to_sha256_over_the_same_bytes() {
    for (name, r, fnv_golden, golden) in [
        ("draw", draw_corpus(), FNV_GOLDEN, GOLDEN),
        (
            "stream-range",
            range_corpus(),
            FNV_GOLDEN_RANGES,
            GOLDEN_RANGES,
        ),
    ] {
        let fnv = fnv1a_64(&r.recorded);
        let digest = hex::encode(sha256(&r.recorded));
        println!(
            "geom-num rng {name} corpus: {} bytes, FNV-1a 64 {fnv:#018x}, SHA-256 {digest}",
            r.recorded.len()
        );
        assert_eq!(
            fnv, fnv_golden,
            "the {name} corpus's FNV-1a 64 hash is {fnv:#018x} here but {fnv_golden:#018x} before \
             the move to SHA-256: these are not the bytes the former golden hashed"
        );
        assert_eq!(
            digest, golden,
            "the {name} corpus's bytes are the ones its FNV-1a golden hashed, but their SHA-256 is \
             {digest}, not the golden {golden}"
        );
    }
}
