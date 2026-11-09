//! Piece G414b's differential test: the DEFLATE reader (`geom_exchange::inflate`) against the crate
//! its decoder is ported from, `miniz_oxide` 0.9.1 (a development dependency), byte for byte, on
//! streams `miniz_oxide` itself compresses at every level, on those streams cut, extended and with
//! bits flipped, on random bytes, and on attack streams. Each input is also read through a source
//! that splits it at seeded points. `SPAREKIT_SEED` replays a run.
//!
//! The oracle decodes the whole input in one call into a buffer the size of the reader's limit, so
//! its verdict maps one to one: done with every byte used is the same output, done early is
//! trailing bytes at the same offset, "cannot make progress" is a stream cut short, "has more
//! output" is past the limit, and "failed" is one of the reader's malformed kinds.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::indexing_slicing,
    reason = "test cases and the oracle; the rules are for kernel code"
)]

use geom_exchange::inflate::{
    InflateBudget, InflateError, InflateErrorKind, inflate, inflate_slice,
};
use geom_format::source::{ByteSource, SourceError};
use geom_run::{Budget, WorkMeter};
use geom_test::seed::{self, Seed};
use miniz_oxide::inflate::TINFLStatus;
use miniz_oxide::inflate::core::{DecompressorOxide, decompress, inflate_flags};

/// What `miniz_oxide` made of a stream.
#[derive(Debug, PartialEq, Eq)]
enum Oracle {
    /// Done, having used this many bytes, with this output.
    Done(usize, Vec<u8>),
    Truncated,
    TooLarge,
    Failed,
}

fn oracle(input: &[u8], limit: usize) -> Oracle {
    let mut out = vec![0; limit];
    let mut r = Box::<DecompressorOxide>::default();
    let (status, used, written) = decompress(
        &mut r,
        input,
        &mut out,
        0,
        inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF,
    );
    match status {
        TINFLStatus::Done => {
            out.truncate(written);
            Oracle::Done(used, out)
        }
        TINFLStatus::FailedCannotMakeProgress => Oracle::Truncated,
        TINFLStatus::HasMoreOutput => Oracle::TooLarge,
        TINFLStatus::Failed => Oracle::Failed,
        other => panic!("miniz_oxide answered {other:?}"),
    }
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The reader's result in the oracle's terms, and the check that they agree.
fn agree(input: &[u8], limit: usize, rng: &mut Rng) {
    let budget = InflateBudget {
        output: limit,
        ratio: u64::MAX,
        grace: 0,
    };
    let ours = inflate_slice(input, &budget, &mut meter());
    let theirs = oracle(input, limit);
    let same = match (&ours, &theirs) {
        (Ok(out), Oracle::Done(used, want)) => *used == input.len() && out == want,
        (Err(e), Oracle::Done(used, _)) => {
            e.kind == InflateErrorKind::TrailingBytes && e.offset == *used as u64
        }
        (Err(e), Oracle::Truncated) => {
            e.kind == InflateErrorKind::Truncated && e.offset == input.len() as u64
        }
        (Err(e), Oracle::TooLarge) => e.kind == InflateErrorKind::TooLarge,
        (Err(e), Oracle::Failed) => matches!(
            e.kind,
            InflateErrorKind::BlockType
                | InflateErrorKind::StoredLength
                | InflateErrorKind::TableSizes
                | InflateErrorKind::CodeLengths
                | InflateErrorKind::RepeatFirst
                | InflateErrorKind::RepeatOverrun
                | InflateErrorKind::Literal
                | InflateErrorKind::Distance
                | InflateErrorKind::DistanceTooFar
        ),
        _ => false,
    };
    assert!(
        same,
        "stream {input:02x?} (limit {limit}): ours {ours:?}, miniz_oxide {theirs:?}"
    );
    // Read again, a byte at a time (so every code is read near the end of its input) and split at
    // seeded points.
    let seeded: Vec<usize> = (0..6).map(|_| 1 + rng.below(30) as usize).collect();
    for sizes in [vec![1], seeded] {
        let mut source = Split {
            rest: input,
            sizes: sizes.clone(),
            at: 0,
        };
        let split = inflate(&mut source, input.len() as u64, &budget, &mut meter());
        assert_eq!(split, ours, "stream {input:02x?} split as {sizes:?}");
    }
}

/// A source that hands out its bytes in pieces of the given sizes, cycling.
struct Split<'a> {
    rest: &'a [u8],
    sizes: Vec<usize>,
    at: usize,
}

impl ByteSource for Split<'_> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SourceError> {
        let size = self.sizes[self.at % self.sizes.len()];
        self.at += 1;
        let n = size.min(buf.len()).min(self.rest.len());
        buf[..n].copy_from_slice(&self.rest[..n]);
        self.rest = &self.rest[n..];
        Ok(n)
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

/// Data with the shapes compressors meet: noise, runs, a small alphabet, repeated phrases, and
/// text like a 3MF mesh's.
fn data(rng: &mut Rng) -> Vec<u8> {
    let len = rng.below(20_000) as usize;
    match rng.below(5) {
        0 => (0..len).map(|_| rng.next() as u8).collect(),
        1 => {
            let mut v = Vec::with_capacity(len);
            while v.len() < len {
                let byte = rng.next() as u8;
                let run = 1 + rng.below(300) as usize;
                v.extend(std::iter::repeat_n(byte, run));
            }
            v.truncate(len);
            v
        }
        2 => (0..len).map(|_| b'a' + rng.below(4) as u8).collect(),
        3 => {
            let phrases: Vec<Vec<u8>> = (0..8)
                .map(|_| (0..=rng.below(40)).map(|_| rng.next() as u8).collect())
                .collect();
            let mut v = Vec::with_capacity(len);
            while v.len() < len {
                v.extend_from_slice(&phrases[rng.below(8) as usize]);
            }
            v.truncate(len);
            v
        }
        _ => {
            let mut v = Vec::new();
            while v.len() < len {
                let line = format!(
                    "<vertex x=\"{}.{}\" y=\"{}\" z=\"{}\"/>\n",
                    rng.below(1000),
                    rng.below(100),
                    rng.below(1000),
                    rng.below(50)
                );
                v.extend_from_slice(line.as_bytes());
            }
            v
        }
    }
}

#[test]
fn every_stream_miniz_oxide_writes_reads_back_the_same() {
    let mut rng = Rng::new(
        "inflate_differential::every_stream_miniz_oxide_writes",
        0x6414_b001,
    );
    for _ in 0..200 {
        let d = data(&mut rng);
        let level = rng.below(11) as u8;
        let s = miniz_oxide::deflate::compress_to_vec(&d, level);
        // Within budgets that hold it, ours equals the data and the oracle's output.
        let out = inflate_slice(
            &s,
            &InflateBudget {
                output: d.len(),
                ratio: u64::MAX,
                grace: 0,
            },
            &mut meter(),
        )
        .unwrap();
        assert_eq!(out, d, "level {level}");
        agree(&s, d.len(), &mut rng);
        // One byte less room is refused, as the oracle refuses it.
        if !d.is_empty() {
            agree(&s, d.len() - 1, &mut rng);
        }
    }
}

#[test]
fn damaged_streams_are_judged_as_miniz_oxide_judges_them() {
    let mut rng = Rng::new("inflate_differential::damaged_streams", 0x6414_b002);
    for _ in 0..600 {
        let d = data(&mut rng);
        let level = rng.below(11) as u8;
        let mut s = miniz_oxide::deflate::compress_to_vec(&d, level);
        let limit = d.len() + 1_000;
        match rng.below(4) {
            // Flip a few bits.
            0 | 1 => {
                for _ in 0..=rng.below(4) {
                    if !s.is_empty() {
                        let at = rng.below(s.len() as u64) as usize;
                        s[at] ^= 1 << rng.below(8);
                    }
                }
            }
            // Cut it.
            2 => {
                let keep = rng.below(s.len() as u64 + 1) as usize;
                s.truncate(keep);
            }
            // Append bytes.
            _ => {
                for _ in 0..=rng.below(8) {
                    s.push(rng.next() as u8);
                }
            }
        }
        agree(&s, limit, &mut rng);
    }
}

#[test]
fn random_bytes_are_judged_as_miniz_oxide_judges_them() {
    let mut rng = Rng::new("inflate_differential::random_bytes", 0x6414_b003);
    for _ in 0..10_000 {
        let len = rng.below(200) as usize;
        let mut s: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
        // Half of them start a fixed or dynamic final block, which random bytes rarely do, so more
        // of them reach the codes.
        if !s.is_empty() && rng.below(2) == 0 {
            s[0] = (s[0] & !0b110) | if rng.below(2) == 0 { 0b011 } else { 0b101 };
        }
        agree(&s, 1 << 16, &mut rng);
    }
}

#[test]
fn attack_streams_are_judged_as_miniz_oxide_judges_them() {
    let mut rng = Rng::new("inflate_differential::attack_streams", 0x6414_b004);
    let streams: Vec<Vec<u8>> = vec![
        vec![],
        vec![0b110],
        vec![0, 0, 0, 0, 0],
        vec![3, 0],
        vec![1, 1, 0, 0xfe, 0xff, 0],
        b"M\xff\xffM*\xad\xad\xad\xad\xad\xad\xad\xcd\xcd\xcdM".to_vec(),
        b"\xdd\xff\xff*M\x94ffffffffff".to_vec(),
        vec![4, 0, 0xfe, 0xff],
        vec![4, 0, 0x24, 0x49, 0],
        vec![
            4, 0x80, 0x49, 0x92, 0x24, 0x49, 0x92, 0x24, 0x71, 0xff, 0xff, 0x93, 0x11, 0,
        ],
        vec![
            4, 0x80, 0x49, 0x92, 0x24, 0x49, 0x92, 0x24, 0x0f, 0xb4, 0xff, 0xff, 0xc3, 0x84,
        ],
        vec![4, 0, 0x24, 0xe9, 0xff, 0x6d],
        vec![2, 0x7e, 0xff, 0xff],
        vec![0x0c, 0xc0, 0x81, 0, 0, 0, 0, 0, 0x90, 0xff, 0x6b, 0x4, 0],
        vec![0xfc, 0, 0],
        // A bomb: 10 MiB of zeros at the highest level.
        miniz_oxide::deflate::compress_to_vec(&vec![0; 10 << 20], 10),
    ];
    for s in &streams {
        agree(s, 1 << 16, &mut rng);
        agree(s, 1 << 20, &mut rng);
    }
    // The bomb under the default budget: refused for its ratio.
    let bomb = streams.last().unwrap();
    let e: InflateError = inflate_slice(bomb, &InflateBudget::DEFAULT, &mut meter()).unwrap_err();
    assert_eq!(e.kind, InflateErrorKind::Ratio);
}
