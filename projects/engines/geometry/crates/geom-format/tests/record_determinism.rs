//! The golden hash of `geom-format`'s record layer: a fixed corpus of files, records, checks,
//! salvage results and range checks, hashed, must give the same digest on every target and on every
//! number of threads (principle 5, chapter 21).
//!
//! The corpus, drawn from a fixed `SplitMix64` stream with explicit 64-bit draws (so 32-bit
//! WebAssembly draws the same values):
//!
//! - `files`: 40 files of up to 12 records each, with random tags, flags, `depends` and payloads
//!   (byte strings and coordinate columns), written whole;
//! - `salvage`: each file with one byte flipped, salvaged: every intact record's offset and the
//!   damaged spans;
//! - `checks`: the CRC-32C of seeded messages of every length to 300 bytes;
//! - `ranges`: the grid, real and plane checks on values at and around the frame's edge.
//!
//! The digest lives in the golden store (`geom-test/golden/store.json`, id
//! `geom-format/record-layer`). A changed digest is a bug, or a change to the format, which is a new
//! format version (24.6): never update the store to make one target pass.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "a test corpus, and chapter 21's thread-count test spawns threads"
)]

use geom_format::checksum::crc32c;
use geom_format::content::ContentId;
use geom_format::file::{HEADER_TAG, salvage, write};
use geom_format::range::{grid, plane, real};
use geom_format::record::Record;
use geom_op::cbor::Encoder;
use geom_run::{Budget, WorkMeter};
use geom_test::golden::{self, Sha256, hex};
use geom_test::rng::SplitMix64;
use geom_test::seed::{Seed, fixed};

const GOLDEN: &str = "geom-format/record-layer";

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn put(total: &mut Sha256, section: &mut Sha256, bytes: &[u8]) {
    for h in [total, section] {
        h.write(&(bytes.len() as u64).to_le_bytes());
        h.write(bytes);
    }
}

fn payload(g: &mut SplitMix64) -> Vec<u8> {
    let mut e = Encoder::new();
    match g.next_u64() % 3 {
        0 => {
            let n = (g.next_u64() % 40) as usize;
            let bytes: Vec<u8> = (0..n).map(|_| g.next_u64() as u8).collect();
            e.bytes(&bytes).unwrap();
        }
        1 => {
            let n = (g.next_u64() % 20) as usize;
            let column: Vec<i64> = (0..n).map(|_| (g.next_u64() >> 24) as i64).collect();
            e.i64_column(&column).unwrap();
        }
        _ => {
            e.array(2).unwrap();
            e.uint(g.next_u64()).unwrap();
            e.text("role").unwrap();
        }
    }
    e.finish().unwrap().into_bytes()
}

fn record(g: &mut SplitMix64, tag: u64) -> Record {
    let n = (g.next_u64() % 3) as usize;
    let mut depends: Vec<ContentId> = (0..n)
        .map(|_| ContentId::of_payload(&g.next_u64().to_le_bytes()))
        .collect();
    depends.sort_by_key(|d| d.item().unwrap().into_bytes());
    depends.dedup();
    Record {
        tag,
        flags: g.next_u64() % 4,
        schema: 1 + g.next_u64() % 3,
        depends,
        payload: payload(g),
    }
}

struct Corpus {
    total: Sha256,
    report: Vec<(&'static str, [u8; 32])>,
}

fn corpus(seed: Seed) -> Corpus {
    let mut g = SplitMix64::new(seed);
    let mut total = Sha256::new();
    let mut report = Vec::new();

    let mut section = Sha256::new();
    let mut files = Vec::new();
    for _ in 0..40 {
        let header = record(&mut g, HEADER_TAG);
        let n = (g.next_u64() % 12) as usize;
        let records: Vec<Record> = (0..n)
            .map(|_| {
                let tag = 10 + g.next_u64() % 5;
                record(&mut g, tag)
            })
            .collect();
        let bytes = write(&header, &records, &mut meter()).unwrap();
        put(&mut total, &mut section, &bytes);
        files.push(bytes);
    }
    report.push(("files", section.finish()));

    let mut section = Sha256::new();
    for bytes in &files {
        let mut broken = bytes.clone();
        let at = (g.next_u64() % broken.len() as u64) as usize;
        broken[at] ^= 1 << (g.next_u64() % 8);
        let s = salvage(&broken, &mut meter()).unwrap();
        let mut out = vec![u8::from(s.label)];
        for (offset, r) in &s.records {
            out.extend(offset.to_le_bytes());
            out.extend(r.tag.to_le_bytes());
        }
        for (from, to) in &s.damaged {
            out.extend(from.to_le_bytes());
            out.extend(to.to_le_bytes());
        }
        put(&mut total, &mut section, &out);
    }
    report.push(("salvage", section.finish()));

    let mut section = Sha256::new();
    for len in 0..=300 {
        let message: Vec<u8> = (0..len).map(|_| g.next_u64() as u8).collect();
        put(&mut total, &mut section, &crc32c(&message).to_le_bytes());
    }
    report.push(("checks", section.finish()));

    let mut section = Sha256::new();
    let edge = (1_i64 << 40) - 1;
    for v in [0, edge, -edge, edge + 1, -edge - 1, i64::MAX, i64::MIN] {
        let x = v as f64;
        let verdicts = [
            grid(v),
            real(x),
            real(f64::from_bits(x.to_bits() + 1)),
            plane([i128::from(v), 1, 0], 0, 1, 0, "p").is_ok(),
        ];
        put(&mut total, &mut section, &verdicts.map(u8::from));
    }
    report.push(("ranges", section.finish()));
    Corpus { total, report }
}

fn seed() -> Seed {
    fixed("geom-format record layer", Seed(0x6392_de7e))
}

#[test]
fn golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let c = corpus(seed());
        (c.total.clone().finish(), c)
    });
    for (name, digest) in &checked.value.report {
        println!("  {name:<8} {}", hex(*digest));
    }
    println!(
        "geom-format record-layer golden hash: {}",
        hex(checked.value.total.clone().finish())
    );
    checked.verdict().unwrap_or_else(|e| {
        panic!("{e}. Compare the section digests above with another target's log to find the part that differs")
    });
}

/// Chapter 21's thread-count test: the same digest on 1, 2, 3 and 8 threads at once.
#[test]
#[cfg_attr(
    all(target_family = "wasm", not(target_feature = "atomics")),
    ignore = "WebAssembly without the atomics feature (wasm32-wasip1) has no threads: \
              std::thread::scope cannot spawn"
)]
fn golden_hash_is_the_same_on_every_thread_count() {
    let one = hex(corpus(seed()).total.finish());
    for threads in golden::THREAD_COUNTS {
        let digests: Vec<String> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..threads)
                .map(|_| s.spawn(|| hex(corpus(seed()).total.finish())))
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert!(
            digests.iter().all(|d| *d == one),
            "{threads} threads: {digests:?}"
        );
    }
}
