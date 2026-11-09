//! Piece G414b's attack: tries to break every claim of the DEFLATE reader (`geom_exchange::inflate`)
//! with streams written bit by bit here: each malformed stream with its own error and offset, zip
//! bombs held to the size and ratio budgets, streams cut short or with bytes after them, byte
//! sources that fail, end early or lie, a work meter that stops, and every stream read again
//! through sources that split it, one byte at a time and at seeded points (`SPAREKIT_SEED` replays
//! one). Every read runs under a watchdog, so a reader that stops making progress fails instead of
//! hanging.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    reason = "test cases and stream writers; the rules are for kernel code"
)]

use geom_exchange::inflate::{
    InflateBudget, InflateError, InflateErrorKind, inflate, inflate_slice,
};
use std::sync::Mutex;

use geom_format::source::{ByteSource, SourceError};
use geom_run::{Budget, Cancel, WorkMeter};
use geom_test::seed::{self, Seed};

/// A stream written bit by bit, the first bit lowest, as RFC 1951 packs it.
#[derive(Default)]
struct Bits {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl Bits {
    /// `count` bits of `value`, its lowest first (header fields and extra bits).
    fn put(&mut self, value: u64, count: u32) -> &mut Self {
        self.acc |= value << self.n;
        self.n += count;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
        self
    }

    /// A Huffman code of `len` bits, its highest bit first (RFC 1951, 3.1.1).
    fn code(&mut self, code: u64, len: u32) -> &mut Self {
        for i in (0..len).rev() {
            self.put((code >> i) & 1, 1);
        }
        self
    }

    /// The fixed code of literal and length symbol `s` (RFC 1951, 3.2.6).
    fn lit(&mut self, s: u64) -> &mut Self {
        match s {
            0..=143 => self.code(0b0011_0000 + s, 8),
            144..=255 => self.code(0b1_1001_0000 + s - 144, 9),
            256..=279 => self.code(s - 256, 7),
            _ => self.code(0b1100_0000 + s - 280, 8),
        }
    }

    /// The fixed code of distance symbol `s`.
    fn dist(&mut self, s: u64) -> &mut Self {
        self.code(s, 5)
    }

    /// A block header: the last-block bit and the type.
    fn header(&mut self, last: bool, kind: u64) -> &mut Self {
        self.put(u64::from(last), 1).put(kind, 2)
    }

    /// A dynamic block's header (RFC 1951, 3.2.7) for the literal and length code lengths `lit`
    /// and the distance code lengths `dist`. The code-length code gives each of the lengths 0 to
    /// 15 four bits, so every length is written as itself, with no repeats.
    fn dynamic(&mut self, last: bool, lit: &[u8], dist: &[u8]) -> &mut Self {
        const ORDER: [u64; 19] = [
            16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
        ];
        self.header(last, 2)
            .put(lit.len() as u64 - 257, 5)
            .put(dist.len() as u64 - 1, 5)
            .put(19 - 4, 4);
        for s in ORDER {
            self.put(if s < 16 { 4 } else { 0 }, 3);
        }
        for &l in lit.iter().chain(dist) {
            self.code(u64::from(l), 4);
        }
        self
    }

    fn bytes(&mut self) -> Vec<u8> {
        let mut out = self.out.clone();
        if self.n > 0 {
            out.push(self.acc as u8);
        }
        out
    }
}

/// A stored block holding `data`.
fn stored(last: bool, data: &[u8]) -> Vec<u8> {
    let len = data.len() as u16;
    let mut out = vec![u8::from(last)];
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&(!len).to_le_bytes());
    out.extend_from_slice(data);
    out
}

/// A fixed-code bomb: one zero, then `matches` matches of 258 bytes at distance 1, 13 bits each.
fn bomb(matches: usize) -> (Vec<u8>, usize) {
    let mut b = Bits::default();
    b.header(true, 1).lit(0);
    for _ in 0..matches {
        b.lit(285).dist(0);
    }
    b.lit(256);
    (b.bytes(), 1 + 258 * matches)
}

/// The canonical codes of the code lengths `lengths` (RFC 1951, 3.2.2).
fn canonical(lengths: &[u8]) -> Vec<u64> {
    let mut count = [0_u64; 16];
    for &l in lengths.iter().filter(|&&l| l > 0) {
        count[usize::from(l)] += 1;
    }
    let mut next = [0_u64; 16];
    let mut code = 0;
    for bits in 1..16 {
        code = (code + count[bits - 1]) << 1;
        next[bits] = code;
    }
    lengths
        .iter()
        .map(|&l| {
            let at = usize::from(l);
            let c = next[at];
            next[at] += u64::from(l > 0);
            c
        })
        .collect()
}

/// Literal and length code lengths for a chain of `n`: the first `n - 1` of `letters` take 1, 2, ...,
/// `n - 1` bits, and the next letter and the end code `n` bits each. The code is complete, and its
/// longest codes pass the ten bits of the decoder's fast table from `n` = 11.
fn chain(n: u8, letters: &[u8]) -> Vec<u8> {
    let mut lit = vec![0; 257];
    for (i, &c) in letters.iter().take(usize::from(n)).enumerate() {
        lit[usize::from(c)] = (i as u8 + 1).min(n);
    }
    lit[256] = n;
    lit
}

/// A cancellation that fires once it has been read [`Watchdog::POLLS`] times. Every read here
/// runs under one, so a reader that stops making progress (steps that neither read, write nor end,
/// again and again) is refused as `Stopped` instead of hanging the tests: the meter is charged once
/// a step and reads its cancellation every 64 charges, so it fires after 2^18 steps, and no read
/// here takes more than about twenty thousand. Under an unlimited budget only the watchdog stops a
/// read, so [`unstopped`] fails a test at once when it does.
#[derive(Default)]
struct Watchdog(Mutex<u64>);

impl Watchdog {
    const POLLS: u64 = 1 << 12;

    /// A meter of `budget` that reads this watchdog.
    fn meter(&self, budget: Budget) -> WorkMeter<'_> {
        WorkMeter::new(budget).with_cancel(self)
    }
}

impl Cancel for Watchdog {
    fn requested(&self) -> bool {
        let mut polls = self.0.lock().unwrap();
        *polls += 1;
        *polls > Self::POLLS
    }
}

/// `read`, failing the test if the watchdog stopped it: a read that made no progress.
fn unstopped(read: Result<Vec<u8>, InflateError>) -> Result<Vec<u8>, InflateError> {
    if let Err(e) = &read {
        assert_ne!(e.kind, InflateErrorKind::Stopped, "a read made no progress");
    }
    read
}

/// Reads `input` within `budget`, under a watchdog.
fn read_within(input: &[u8], budget: &InflateBudget) -> Result<Vec<u8>, InflateError> {
    unstopped(inflate_slice(
        input,
        budget,
        &mut Watchdog::default().meter(Budget::UNLIMITED),
    ))
}

fn read(input: &[u8]) -> Result<Vec<u8>, InflateError> {
    read_within(input, &InflateBudget::DEFAULT)
}

fn refused(input: &[u8], kind: InflateErrorKind, offset: u64) {
    let got = read(input);
    assert_eq!(
        got,
        Err(InflateError { kind, offset }),
        "stream {input:02x?}"
    );
}

/// A source that hands out its bytes in pieces of the given sizes, cycling.
struct Split<'a> {
    rest: &'a [u8],
    sizes: Vec<usize>,
    at: usize,
}

impl ByteSource for Split<'_> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SourceError> {
        let size = self.sizes[self.at % self.sizes.len()].max(1);
        self.at += 1;
        let n = size.min(buf.len()).min(self.rest.len());
        buf[..n].copy_from_slice(&self.rest[..n]);
        self.rest = &self.rest[n..];
        Ok(n)
    }
}

fn read_split(
    input: &[u8],
    sizes: Vec<usize>,
    budget: &InflateBudget,
) -> Result<Vec<u8>, InflateError> {
    let mut source = Split {
        rest: input,
        sizes,
        at: 0,
    };
    unstopped(inflate(
        &mut source,
        input.len() as u64,
        budget,
        &mut Watchdog::default().meter(Budget::UNLIMITED),
    ))
}

/// The same result read whole, one byte at a time and at seeded split points.
fn split_never_matters(input: &[u8], budget: &InflateBudget, rng: &mut Rng) {
    let whole = read_within(input, budget);
    assert_eq!(
        read_split(input, vec![1], budget),
        whole,
        "one byte at a time"
    );
    for _ in 0..4 {
        let sizes = (0..8).map(|_| 1 + rng.below(40) as usize).collect();
        assert_eq!(read_split(input, sizes, budget), whole, "seeded splits");
    }
}

/// splitmix64: a test generator, outside the kernel.
struct Rng(u64);

impl Rng {
    fn new(label: &str, default: u64) -> Rng {
        Rng(seed::resolve(label, Seed(default)).unwrap().get())
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

#[test]
fn stored_fixed_and_empty_streams_read_back() {
    assert_eq!(read(&stored(true, b"spare kit")).unwrap(), b"spare kit");
    // An empty stored block, and a fixed block holding only its end.
    assert_eq!(read(&stored(true, b"")).unwrap(), b"");
    assert_eq!(read(&[3, 0]).unwrap(), b"");
    assert_eq!(read(&[1, 1, 0, 0xfe, 0xff, 0]).unwrap(), [0]);
    // Literals of all four fixed lengths, then a match of 3 at distance 2 ("abab" + "a").
    let mut b = Bits::default();
    b.header(true, 1)
        .lit(u64::from(b'a'))
        .lit(200)
        .lit(u64::from(b'a'))
        .lit(200)
        .lit(257)
        .dist(1)
        .lit(256);
    assert_eq!(
        read(&b.bytes()).unwrap(),
        [b'a', 200, b'a', 200, b'a', 200, b'a']
    );
    // A stored block after a fixed one: its header comes from the bit buffer.
    let mut b = Bits::default();
    b.header(false, 1)
        .lit(u64::from(b'x'))
        .lit(256)
        .header(true, 0);
    let mut s = b.bytes();
    s.extend_from_slice(&stored(true, b"yz")[1..]);
    assert_eq!(read(&s).unwrap(), b"xyz");
}

#[test]
fn every_malformed_stream_is_refused_with_its_kind_and_offset() {
    use InflateErrorKind as K;
    // Block type 3.
    refused(&[0b110], K::BlockType, 0);
    // A stored length whose complement disagrees.
    refused(&[0, 0, 0, 0, 0], K::StoredLength, 5);
    // zlib's and miniz_oxide's cases: too many code lengths, a bad code-length count, an
    // incomplete code, a repeat with nothing before it, an over-subscribed literal code, a
    // distance code that cannot be.
    refused(
        b"M\xff\xffM*\xad\xad\xad\xad\xad\xad\xad\xcd\xcd\xcdM",
        K::TableSizes,
        2,
    );
    refused(b"\xdd\xff\xff*M\x94ffffffffff", K::TableSizes, 2);
    refused(&[4, 0, 0xfe, 0xff], K::CodeLengths, 3);
    refused(&[4, 0, 0x24, 0x49, 0], K::RepeatFirst, 3);
    refused(
        &[
            4, 0x80, 0x49, 0x92, 0x24, 0x49, 0x92, 0x24, 0x71, 0xff, 0xff, 0x93, 0x11, 0,
        ],
        K::CodeLengths,
        13,
    );
    refused(&[2, 0x7e, 0xff, 0xff], K::Distance, 1);
    // A match before the first byte of output.
    refused(
        &[0x0c, 0xc0, 0x81, 0, 0, 0, 0, 0, 0x90, 0xff, 0x6b, 0x4, 0],
        K::DistanceTooFar,
        11,
    );
    // The fixed codes for literal and length 286 and distance 30.
    let mut b = Bits::default();
    b.header(true, 1).lit(u64::from(b'a')).lit(286);
    refused(&b.bytes(), K::Literal, 2);
    let mut b = Bits::default();
    b.header(true, 1).lit(u64::from(b'a')).lit(257).dist(30);
    refused(&b.bytes(), K::Distance, 2);
    // A match of distance 2 after one byte. Each offset is the byte holding the first bit not yet
    // used: three header bits, an 8-bit literal, a 7-bit length and a 5-bit distance end at bit 23.
    let mut b = Bits::default();
    b.header(true, 1)
        .lit(u64::from(b'a'))
        .lit(257)
        .dist(1)
        .lit(256);
    refused(&b.bytes(), K::DistanceTooFar, 2);
    // A dynamic block whose zero repeats run past its 258 code lengths: HLIT 257, HDIST 1,
    // HCLEN 4 giving 18 and 0 one-bit codes, then two repeats of 138 zeros.
    let mut b = Bits::default();
    b.header(true, 2).put(0, 5).put(0, 5).put(0, 4);
    b.put(0, 3).put(0, 3).put(1, 3).put(1, 3);
    b.code(1, 1).put(127, 7).code(1, 1).put(127, 7);
    refused(&b.bytes(), K::RepeatOverrun, 5);
}

#[test]
fn a_stream_cut_short_or_with_bytes_after_it_is_refused() {
    let whole = stored(true, b"spare kit");
    for cut in 0..whole.len() {
        refused(&whole[..cut], InflateErrorKind::Truncated, cut as u64);
    }
    let mut longer = whole.clone();
    longer.push(0);
    refused(&longer, InflateErrorKind::TrailingBytes, whole.len() as u64);
    let mut b = Bits::default();
    b.header(true, 1).lit(u64::from(b'k')).lit(256);
    let mut s = b.bytes();
    let end = s.len() as u64;
    s.extend_from_slice(&[0; 20]);
    refused(&s, InflateErrorKind::TrailingBytes, end);
    // A non-final block, then nothing.
    let mut b = Bits::default();
    b.header(false, 1).lit(256);
    let s = b.bytes();
    refused(&s, InflateErrorKind::Truncated, s.len() as u64);
}

#[test]
fn a_bomb_is_held_to_the_ratio_and_size_budgets() {
    // 2 MiB from about 4 KiB: over 100 to 1.
    let (s, size) = bomb(8_200);
    let ratio_limit = 100 * s.len();
    let got = read(&s).unwrap_err();
    assert_eq!(got.kind, InflateErrorKind::Ratio);
    assert!(got.offset < s.len() as u64);
    assert!(size > ratio_limit);
    // The same stream under a higher ratio but a smaller size budget.
    let budget = InflateBudget {
        output: 1 << 20,
        ratio: 1_000,
        grace: 0,
    };
    let got = read_within(&s, &budget).unwrap_err();
    assert_eq!(got.kind, InflateErrorKind::TooLarge);
    // And read in full when the budgets allow it.
    let budget = InflateBudget {
        output: size,
        ratio: 1_000,
        grace: 0,
    };
    let out = read_within(&s, &budget).unwrap();
    assert_eq!(out.len(), size);
    assert!(out.iter().all(|&b| b == 0));
    // One byte short of the size is refused, at the last match.
    let budget = InflateBudget {
        output: size - 1,
        ..budget
    };
    assert_eq!(
        read_within(&s, &budget).unwrap_err().kind,
        InflateErrorKind::TooLarge
    );
    // Within the grace size the ratio does not count.
    let (small, small_size) = bomb(300);
    assert!(small_size > 100 * small.len() && small_size < 100 << 10);
    assert_eq!(read(&small).unwrap().len(), small_size);
}

#[test]
fn the_limit_is_fixed_by_the_input_length() {
    let b = InflateBudget::DEFAULT;
    assert_eq!(b.limit(0), (100 << 10, InflateErrorKind::Ratio));
    assert_eq!(b.limit(1_024), (100 << 10, InflateErrorKind::Ratio));
    assert_eq!(b.limit(10_000), (1_000_000, InflateErrorKind::Ratio));
    assert_eq!(b.limit(1 << 30), (256 << 20, InflateErrorKind::TooLarge));
    assert_eq!(b.limit(u64::MAX), (256 << 20, InflateErrorKind::TooLarge));
    // Exactly at the size budget the size budget is named.
    let at = InflateBudget {
        output: 1_000,
        ratio: 10,
        grace: 0,
    };
    assert_eq!(at.limit(100), (1_000, InflateErrorKind::TooLarge));
    assert_eq!(at.limit(99), (990, InflateErrorKind::Ratio));
    // A zero budget refuses any output, and reads an empty stream.
    let none = InflateBudget {
        output: 0,
        ratio: 0,
        grace: 0,
    };
    assert_eq!(read_within(&[3, 0], &none), Ok(Vec::new()));
    // A stored byte with no room for it: refused at that byte, after the block's five-byte header,
    // with the byte itself unread.
    assert_eq!(
        read_within(&stored(true, b"a"), &none),
        Err(InflateError {
            kind: InflateErrorKind::TooLarge,
            offset: 5
        })
    );
}

struct Failing;

impl ByteSource for Failing {
    fn read(&mut self, _: &mut [u8]) -> Result<usize, SourceError> {
        Err(SourceError::Failed(7))
    }
}

struct Lying;

impl ByteSource for Lying {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SourceError> {
        Ok(buf.len() + 1)
    }
}

#[test]
fn a_source_that_fails_ends_early_or_lies_is_refused() {
    let b = InflateBudget::DEFAULT;
    assert_eq!(
        inflate(
            &mut Failing,
            10,
            &b,
            &mut Watchdog::default().meter(Budget::UNLIMITED)
        ),
        Err(InflateError {
            kind: InflateErrorKind::Source(SourceError::Failed(7)),
            offset: 0
        })
    );
    assert_eq!(
        inflate(
            &mut Lying,
            10,
            &b,
            &mut Watchdog::default().meter(Budget::UNLIMITED)
        )
        .unwrap_err()
        .kind,
        InflateErrorKind::Source(SourceError::Overrun)
    );
    // Declared longer than the source: the source ends early, at its length.
    let s = stored(false, b"abc");
    let mut src = geom_format::source::SliceSource::new(&s);
    assert_eq!(
        inflate(
            &mut src,
            100,
            &b,
            &mut Watchdog::default().meter(Budget::UNLIMITED)
        ),
        Err(InflateError {
            kind: InflateErrorKind::Source(SourceError::UnexpectedEnd),
            offset: s.len() as u64
        })
    );
    // Declared shorter: only that many bytes are read, and the stream is cut short there.
    let s = stored(true, b"abc");
    let mut src = geom_format::source::SliceSource::new(&s);
    assert_eq!(
        inflate(
            &mut src,
            4,
            &b,
            &mut Watchdog::default().meter(Budget::UNLIMITED)
        ),
        Err(InflateError {
            kind: InflateErrorKind::Truncated,
            offset: 4
        })
    );
}

#[test]
fn the_meter_stops_the_read() {
    let s = stored(true, &[7; 100]);
    let dog = Watchdog::default();
    let mut m = dog.meter(Budget::new(50, u64::MAX));
    assert_eq!(
        inflate_slice(&s, &InflateBudget::DEFAULT, &mut m)
            .unwrap_err()
            .kind,
        InflateErrorKind::Stopped
    );
    // Exactly enough: one unit for each byte read and each byte written.
    let mut m = dog.meter(Budget::new((s.len() + 100) as u64, u64::MAX));
    assert_eq!(
        inflate_slice(&s, &InflateBudget::DEFAULT, &mut m).unwrap(),
        [7; 100]
    );
}

#[test]
fn long_streams_cross_every_buffer_boundary() {
    // Stored blocks over many input chunks and output windows, then matches reaching back across
    // them: 300 KiB of a counting pattern.
    let data: Vec<u8> = (0..300 * 1024).map(|i| (i * 7 + i / 251) as u8).collect();
    let mut s = Vec::new();
    let chunks: Vec<&[u8]> = data.chunks(65_535).collect();
    for (i, c) in chunks.iter().enumerate() {
        s.extend_from_slice(&stored(i + 1 == chunks.len(), c));
    }
    assert_eq!(read(&s).unwrap(), data);
    // A fixed block of literals and long overlapping and distant matches.
    let mut b = Bits::default();
    b.header(true, 1);
    let mut want = Vec::new();
    for i in 0..40_000_u64 {
        let lit = (i * 13 % 256) as u8;
        b.lit(u64::from(lit));
        want.push(lit);
    }
    for (len_sym, extra_bits, extra, dist_sym, dist_bits, dist_extra) in [
        (285_u64, 0, 0, 0_u64, 0, 0), // 258 at 1
        (266, 1, 1, 2, 0, 0),         // 14 at 3
        (284, 5, 30, 29, 13, 8_000),  // 257 at 32,577
        (270, 2, 3, 13, 5, 31),       // 26 at 128
        (257, 0, 0, 4, 1, 1),         // 3 at 6
    ] {
        b.lit(len_sym)
            .put(extra, extra_bits)
            .dist(dist_sym)
            .put(dist_extra, dist_bits);
        let len = match len_sym {
            285 => 258,
            266 => 13 + extra as usize,
            284 => 227 + extra as usize,
            270 => 23 + extra as usize,
            _ => 3,
        };
        let dist = match dist_sym {
            0 => 1,
            2 => 3,
            29 => 24_577 + dist_extra as usize,
            13 => 97 + dist_extra as usize,
            _ => 5 + dist_extra as usize,
        };
        for _ in 0..len {
            want.push(want[want.len() - dist]);
        }
    }
    b.lit(256);
    assert_eq!(read(&b.bytes()).unwrap(), want);
}

#[test]
fn the_split_never_matters() {
    let mut rng = Rng::new("attack_g414b::the_split_never_matters", 0x6414_b5b1);
    let mut streams: Vec<Vec<u8>> = vec![
        stored(true, b"spare kit"),
        vec![0b110],
        vec![0, 0, 0, 0, 0],
        vec![4, 0, 0xfe, 0xff],
        vec![4, 0, 0x24, 0x49, 0],
        vec![2, 0x7e, 0xff, 0xff],
        vec![0x0c, 0xc0, 0x81, 0, 0, 0, 0, 0, 0x90, 0xff, 0x6b, 0x4, 0],
        bomb(400).0,
    ];
    let mut b = Bits::default();
    b.header(false, 1)
        .lit(u64::from(b'x'))
        .lit(256)
        .header(true, 0);
    let mut s = b.bytes();
    s.extend_from_slice(&stored(true, b"yz")[1..]);
    streams.push(s);
    // Random bytes, which are mostly malformed somewhere, and fixed blocks of random symbols.
    for _ in 0..200 {
        let len = 1 + rng.below(64) as usize;
        streams.push((0..len).map(|_| rng.next() as u8).collect());
        let mut b = Bits::default();
        b.header(true, 1);
        for _ in 0..rng.below(300) {
            let s = rng.below(300);
            b.lit(s.min(287));
            if (257..=285).contains(&s) {
                b.dist(rng.below(32));
                b.put(rng.next(), 13);
            }
        }
        b.lit(256);
        streams.push(b.bytes());
    }
    let small = InflateBudget {
        output: 1_000,
        ratio: 100,
        grace: 0,
    };
    for s in &streams {
        split_never_matters(s, &InflateBudget::DEFAULT, &mut rng);
        split_never_matters(s, &small, &mut rng);
    }
}

#[test]
fn a_stored_block_after_a_huffman_block_reads_the_bytes_held_for_it() {
    // The fast loop reads four bytes at a time, so when a fixed block's end code is read, up to six
    // whole bytes after it can be held in the bit buffer: the next stored block's length, its
    // complement and its first bytes then come from there, and the rest from the input. Each count
    // of literals before the end code moves where it falls, and so how many bytes are held.
    let mut rng = Rng::new(
        "attack_g414b::a_stored_block_after_a_huffman_block",
        0x6414_b5b2,
    );
    for k in 0..48_u64 {
        for (first, second) in [(0, 0), (1, 0), (1, 1), (2, 3), (3, 0), (5, 2), (0, 9)] {
            let mut b = Bits::default();
            let mut want = Vec::new();
            b.header(false, 1);
            for i in 0..k {
                let lit = (i * 37 + 11) % 256;
                b.lit(lit);
                want.push(lit as u8);
            }
            b.lit(256).header(false, 0);
            let mut s = b.bytes();
            let one: Vec<u8> = (0..first).map(|i| 0xa0 + i).collect();
            let two: Vec<u8> = (0..second).map(|i| 0xc0 + i).collect();
            s.extend_from_slice(&stored(false, &one)[1..]);
            s.extend_from_slice(&stored(false, &two));
            want.extend_from_slice(&one);
            want.extend_from_slice(&two);
            // A final fixed block, so the input is long enough for the fast loop from the start.
            let mut t = Bits::default();
            t.header(true, 1);
            for i in 0..24_u64 {
                t.lit(i);
                want.push(i as u8);
            }
            t.lit(256);
            s.extend_from_slice(&t.bytes());
            assert_eq!(
                read(&s).unwrap(),
                want,
                "{k} literals, then stored blocks of {first} and {second}"
            );
            split_never_matters(&s, &InflateBudget::DEFAULT, &mut rng);
        }
    }
}

#[test]
fn huffman_codes_cross_every_output_window() {
    // A fixed block of 140,000 literals: the output fills its first 64 KiB, its doubled buffer and
    // the next 64 KiB window while codes remain, so the decoder meets each window's end with two,
    // one or no bytes of room and must stop there, writing nothing past it.
    let mut b = Bits::default();
    b.header(true, 1);
    let mut want = Vec::with_capacity(140_000);
    for i in 0..140_000_u64 {
        let lit = (i * 7 + i / 300) % 256;
        b.lit(lit);
        want.push(lit as u8);
    }
    b.lit(256);
    let s = b.bytes();
    assert_eq!(read(&s).unwrap(), want);
    // Read in uneven pieces too, so each window ends at another point of the input.
    assert_eq!(
        read_split(&s, vec![300, 7, 1_000], &InflateBudget::DEFAULT).unwrap(),
        want
    );
}

#[test]
fn codes_longer_than_the_fast_table_read_back_however_split() {
    // Dynamic blocks whose longest codes, the end code's among them, are 11 to 15 bits: past the
    // decoder's ten-bit table, so each is read by walking a tree, one node a bit. Read a byte at a
    // time, the walk runs with only some of a code's bits held and must ask for more input exactly
    // when the code is not yet whole; `shift` one-bit codes before the end code make the stream end
    // at each bit of its last byte, so the end code is the last bits held. Two blocks in one stream
    // build the second tree over the first, deeper and with the letters the other way round.
    let mut rng = Rng::new(
        "attack_g414b::codes_longer_than_the_fast_table",
        0x6414_b5b3,
    );
    let letters: Vec<u8> = (b'a'..=b'o').collect();
    let backwards: Vec<u8> = letters.iter().rev().copied().collect();
    let mut plans: Vec<Vec<Vec<u8>>> = (11..=15).map(|n| vec![chain(n, &letters)]).collect();
    for (first, second) in [(11, 15), (12, 14), (13, 15)] {
        plans.push(vec![chain(first, &letters), chain(second, &backwards)]);
    }
    for blocks in &plans {
        for shift in 0..8 {
            let mut b = Bits::default();
            let mut want = Vec::new();
            for (i, lit) in blocks.iter().enumerate() {
                let last = i + 1 == blocks.len();
                let codes = canonical(lit);
                let used: Vec<usize> = (0..256).filter(|&s| lit[s] > 0).collect();
                // Every letter, longest code first, then shortest first, then seeded picks.
                let mut text: Vec<usize> = used.clone();
                text.sort_by_key(|&s| std::cmp::Reverse(lit[s]));
                text.extend(used.iter().copied());
                for _ in 0..40 {
                    text.push(used[rng.below(used.len() as u64) as usize]);
                }
                if last {
                    let one_bit = used.iter().copied().find(|&s| lit[s] == 1).unwrap();
                    text.extend(std::iter::repeat_n(one_bit, shift));
                }
                b.dynamic(last, lit, &[0]);
                for &s in &text {
                    b.code(codes[s], u32::from(lit[s]));
                    want.push(s as u8);
                }
                b.code(codes[256], u32::from(lit[256]));
            }
            let s = b.bytes();
            assert_eq!(read(&s).unwrap(), want, "stream {s:02x?}");
            split_never_matters(&s, &InflateBudget::DEFAULT, &mut rng);
        }
    }
}

#[test]
fn the_output_grows_by_doubling_only_once_it_is_full() {
    // 100,000 stored bytes read a thousand at a time: a hundred steps, under a limit of 16 MiB.
    // The output grows only when the decoder has filled it, so the buffer handed back holds at most
    // twice the output (or the first 64 KiB), however many steps the read took and however far the
    // limit is.
    let data: Vec<u8> = (0..100_000_u32).map(|i| (i % 251) as u8).collect();
    let mut s = Vec::new();
    let chunks: Vec<&[u8]> = data.chunks(30_000).collect();
    for (i, c) in chunks.iter().enumerate() {
        s.extend_from_slice(&stored(i + 1 == chunks.len(), c));
    }
    let budget = InflateBudget {
        output: 16 << 20,
        ratio: u64::MAX,
        grace: 0,
    };
    let out = read_split(&s, vec![1_000], &budget).unwrap();
    assert_eq!(out, data);
    assert!(
        out.capacity() <= (2 * out.len()).max(64 << 10),
        "capacity {} for {} bytes",
        out.capacity(),
        out.len()
    );
}

#[test]
fn errors_say_what_and_where() {
    let e = InflateError {
        kind: InflateErrorKind::Ratio,
        offset: 12,
    };
    assert_eq!(e.to_string(), "DEFLATE stream refused at byte 12: Ratio");
}
