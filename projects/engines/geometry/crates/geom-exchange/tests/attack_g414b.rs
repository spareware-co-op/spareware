//! Piece G414b's attack: tries to break every claim of the DEFLATE reader (`geom_exchange::inflate`)
//! with streams written bit by bit here: each malformed stream with its own error and offset, zip
//! bombs held to the size and ratio budgets, streams cut short or with bytes after them, byte
//! sources that fail, end early or lie, a work meter that stops, and every stream read again
//! through sources that split it, one byte at a time and at seeded points (`SPAREKIT_SEED` replays
//! one).

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
use geom_format::source::{ByteSource, SourceError};
use geom_run::{Budget, WorkMeter};
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

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn read(input: &[u8]) -> Result<Vec<u8>, InflateError> {
    inflate_slice(input, &InflateBudget::DEFAULT, &mut meter())
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
    inflate(&mut source, input.len() as u64, budget, &mut meter())
}

/// The same result read whole, one byte at a time and at seeded split points.
fn split_never_matters(input: &[u8], budget: &InflateBudget, rng: &mut Rng) {
    let whole = inflate_slice(input, budget, &mut meter());
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
    let got = inflate_slice(&s, &budget, &mut meter()).unwrap_err();
    assert_eq!(got.kind, InflateErrorKind::TooLarge);
    // And read in full when the budgets allow it.
    let budget = InflateBudget {
        output: size,
        ratio: 1_000,
        grace: 0,
    };
    let out = inflate_slice(&s, &budget, &mut meter()).unwrap();
    assert_eq!(out.len(), size);
    assert!(out.iter().all(|&b| b == 0));
    // One byte short of the size is refused, at the last match.
    let budget = InflateBudget {
        output: size - 1,
        ..budget
    };
    assert_eq!(
        inflate_slice(&s, &budget, &mut meter()).unwrap_err().kind,
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
    assert_eq!(inflate_slice(&[3, 0], &none, &mut meter()), Ok(Vec::new()));
    assert_eq!(
        inflate_slice(&stored(true, b"a"), &none, &mut meter())
            .unwrap_err()
            .kind,
        InflateErrorKind::TooLarge
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
        inflate(&mut Failing, 10, &b, &mut meter()),
        Err(InflateError {
            kind: InflateErrorKind::Source(SourceError::Failed(7)),
            offset: 0
        })
    );
    assert_eq!(
        inflate(&mut Lying, 10, &b, &mut meter()).unwrap_err().kind,
        InflateErrorKind::Source(SourceError::Overrun)
    );
    // Declared longer than the source: the source ends early, at its length.
    let s = stored(false, b"abc");
    let mut src = geom_format::source::SliceSource::new(&s);
    assert_eq!(
        inflate(&mut src, 100, &b, &mut meter()),
        Err(InflateError {
            kind: InflateErrorKind::Source(SourceError::UnexpectedEnd),
            offset: s.len() as u64
        })
    );
    // Declared shorter: only that many bytes are read, and the stream is cut short there.
    let s = stored(true, b"abc");
    let mut src = geom_format::source::SliceSource::new(&s);
    assert_eq!(
        inflate(&mut src, 4, &b, &mut meter()),
        Err(InflateError {
            kind: InflateErrorKind::Truncated,
            offset: 4
        })
    );
}

#[test]
fn the_meter_stops_the_read() {
    let s = stored(true, &[7; 100]);
    let mut m = WorkMeter::new(Budget::new(50, u64::MAX));
    assert_eq!(
        inflate_slice(&s, &InflateBudget::DEFAULT, &mut m)
            .unwrap_err()
            .kind,
        InflateErrorKind::Stopped
    );
    // Exactly enough: one unit for each byte read and each byte written.
    let mut m = WorkMeter::new(Budget::new((s.len() + 100) as u64, u64::MAX));
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
fn errors_say_what_and_where() {
    let e = InflateError {
        kind: InflateErrorKind::Ratio,
        offset: 12,
    };
    assert_eq!(e.to_string(), "DEFLATE stream refused at byte 12: Ratio");
}
