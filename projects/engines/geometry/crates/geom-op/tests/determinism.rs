//! The golden hash of `geom-op`'s canonical CBOR encoder and SHA-256: a fixed corpus of encodings
//! and digests, hashed, must give the same digest on every target and on every number of threads.
//!
//! The kernel promises the same bits on every machine and for any thread count (principle 5,
//! chapter 21), and the canonical encoding is what names and content IDs hash, so its bytes can
//! never differ between machines (24.3). The corpus, in sections (the test prints a digest per
//! section, to find which part differs when one target disagrees):
//!
//! - `edges`: every power of two from 2^−1074 to 2^1023 and its negation, the doubles one unit
//!   either side of each binary16 and binary32 limit, and integers at every head and bignum
//!   boundary, each encoded alone;
//! - `values`: 3,000 nested values of SKG's subset (arrays, maps, sets, sorted lists, columns and
//!   every scalar), drawn from a fixed `SplitMix64` stream with explicit 64-bit draws, so the same
//!   values on 32-bit WebAssembly as on 64-bit targets;
//! - `hashes`: SHA-256 and its 128-bit cut of every encoding above, and of seeded messages of every
//!   length from 0 to 300 bytes.
//!
//! Each record is its bytes' length and then the bytes. The digest lives in the golden store
//! (`geom-test/golden/store.json`, id `geom-op/determinism`); `geom_test::golden::check` runs the
//! corpus twice in one process on two differently perturbed heaps before comparing it with the
//! store's (section 28.13). A changed digest is a bug, or a deliberate change to the encoding,
//! which is a new `NameScheme` and a new format version (32.19, 24.6): never update the store to
//! make one target pass.

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    reason = "a test corpus, and chapter 21's thread-count test spawns threads; the rules are for kernel code"
)]

mod common;

use common::{V, ours};
use geom_op::hash::{Sha256 as OpSha256, sha256, sha256_128};
use geom_test::golden::{self, Sha256, hex};
use geom_test::rng::SplitMix64;
use geom_test::seed::{Seed, fixed};

const GOLDEN: &str = "geom-op/determinism";

/// How many nested values the `values` section draws.
const VALUES: usize = 3000;

/// A finite double from a draw: a binary16 value, a binary32 value, or any finite bits.
fn double(g: &mut SplitMix64) -> f64 {
    let bits = g.next_u64();
    let x = match bits % 3 {
        0 => half::f16::from_bits((bits >> 16) as u16).to_f64(),
        1 => f64::from(f32::from_bits((bits >> 32) as u32)),
        _ => f64::from_bits(g.next_u64()),
    };
    if x.is_finite() { x } else { 0.5 }
}

/// An integer of any head width, or a bignum.
fn integer(g: &mut SplitMix64) -> i128 {
    let a = g.next_u64();
    let b = g.next_u64();
    match a % 4 {
        0 => i128::from(b % 300) - 150,
        1 => i128::from(b),
        2 => -1 - i128::from(b),
        _ => (i128::from(a) << 64) | i128::from(b),
    }
}

/// A small count, below `n`.
fn below(g: &mut SplitMix64, n: u64) -> usize {
    (g.next_u64() % n) as usize
}

/// A value of the subset, nested at most `depth` deep.
fn value(g: &mut SplitMix64, depth: u32) -> V {
    let kind = g.next_u64() % if depth == 0 { 7 } else { 11 };
    match kind {
        0 => V::U(g.next_u64() >> below(g, 64)),
        1 => V::I(integer(g)),
        2 => V::F(double(g)),
        3 => V::B((0..below(g, 40)).map(|_| g.next_u64() as u8).collect()),
        4 => V::T(
            (0..below(g, 12))
                .map(|_| char::from_u32((g.next_u64() % 0x2_0000) as u32).unwrap_or('\u{fffd}'))
                .collect(),
        ),
        5 => V::Ci((0..below(g, 9)).map(|_| g.next_u64() as i64).collect()),
        6 => V::Cf((0..below(g, 9)).map(|_| double(g)).collect()),
        7 => V::A((0..below(g, 6)).map(|_| value(g, depth - 1)).collect()),
        8 => {
            let mut key = 0_u64;
            V::M(
                (0..below(g, 6))
                    .map(|_| {
                        key += 1 + (g.next_u64() >> below(g, 64));
                        (key, value(g, depth - 1))
                    })
                    .collect(),
            )
        }
        9 => {
            let mut seen = std::collections::BTreeMap::new();
            for _ in 0..below(g, 6) {
                let x = value(g, depth - 1);
                seen.insert(ours(&x), x);
            }
            V::Set(seen.into_values().collect())
        }
        _ => V::Sorted((0..below(g, 6)).map(|_| value(g, depth - 1)).collect()),
    }
}

/// 2^k as a double, for k from −1074 to 1023, built from its bits.
fn pow2(k: i32) -> f64 {
    if k >= -1022 {
        f64::from_bits(((k + 1023) as u64) << 52)
    } else {
        f64::from_bits(1_u64 << (k + 1074))
    }
}

/// The edge values, each alone.
fn edges() -> Vec<V> {
    let mut out = Vec::new();
    for k in -1074..=1023 {
        out.push(V::F(pow2(k)));
        out.push(V::F(-pow2(k)));
    }
    for limit in [
        65504.0,
        pow2(-24),
        pow2(-14),
        f64::from(f32::MAX),
        pow2(-149),
        pow2(-126),
    ] {
        for d in [-1_i64, 1] {
            out.push(V::F(f64::from_bits(limit.to_bits().wrapping_add_signed(d))));
        }
    }
    for n in [
        23_u64,
        24,
        255,
        256,
        65535,
        65536,
        0xffff_ffff,
        0x1_0000_0000,
        u64::MAX,
    ] {
        out.push(V::U(n));
        out.push(V::I(-1 - i128::from(n)));
    }
    for k in 63..127 {
        out.push(V::I(1 << k));
        out.push(V::I(-(1 << k) - 1));
    }
    out.push(V::I(i128::MAX));
    out.push(V::I(i128::MIN));
    out
}

/// The corpus: its total digest and each section's name, records and digest.
struct Corpus {
    total: Sha256,
    report: Vec<(&'static str, usize, [u8; 32])>,
}

fn record(total: &mut Sha256, section: &mut Sha256, bytes: &[u8]) {
    for h in [total, section] {
        h.write(&(bytes.len() as u64).to_le_bytes());
        h.write(bytes);
    }
}

fn corpus(seed: Seed) -> Corpus {
    let mut total = Sha256::new();
    let mut report = Vec::new();

    let mut encodings = Vec::new();
    let mut section = Sha256::new();
    let edges = edges();
    for v in &edges {
        let bytes = ours(v);
        record(&mut total, &mut section, &bytes);
        encodings.push(bytes);
    }
    report.push(("edges", edges.len(), section.finish()));

    let mut section = Sha256::new();
    let mut g = SplitMix64::new(seed);
    for _ in 0..VALUES {
        let bytes = ours(&value(&mut g, 4));
        record(&mut total, &mut section, &bytes);
        encodings.push(bytes);
    }
    report.push(("values", VALUES, section.finish()));

    let mut section = Sha256::new();
    let mut count = 0;
    for bytes in &encodings {
        record(&mut total, &mut section, &sha256(bytes));
        record(&mut total, &mut section, &sha256_128(bytes));
        count += 2;
    }
    for len in 0..=300 {
        let mut message = vec![0_u8; len];
        g.fill_bytes(&mut message);
        let mut streamed = OpSha256::new();
        for piece in message.chunks(7) {
            streamed.write(piece);
        }
        record(&mut total, &mut section, &streamed.finish());
        count += 1;
    }
    report.push(("hashes", count, section.finish()));
    Corpus { total, report }
}

/// The seed of the `values` section, fixed: the golden depends on it.
fn seed() -> Seed {
    fixed("geom-op determinism", Seed(0x6386_de7e))
}

#[test]
fn golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let c = corpus(seed());
        (c.total.clone().finish(), c)
    });
    let c = &checked.value;
    for (name, records, digest) in &c.report {
        println!("  {name:<8} {records:>6} records  {}", hex(*digest));
    }
    println!("geom-op golden hash: {}", hex(c.total.clone().finish()));
    checked.verdict().unwrap_or_else(|e| {
        panic!("{e}. Compare the section digests above with another target's log to find the part that differs")
    });
}

/// Principle 5 and chapter 21's thread-count test: the corpus gives the same digest on 1, 2, 3
/// and 8 threads running it at once, every run in one process.
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
