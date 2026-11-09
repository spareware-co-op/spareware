//! The zip reader's fuzz target (piece G414c), as tests: `zip_fuzz` is bolero's test mode, which
//! cargo-bolero, `ClusterFuzzLite` and Kani select by name, and `zip_seeded` the seeded leg that
//! runs on every target, WebAssembly included (the crate README sets out the layout). Each hands
//! the fuzzer's raw bytes to `geom_fuzz::zip::harness`, which reads them with `geom-exchange`'s
//! zip reader.
//!
//! `seeded_archives` adds a leg random bytes never reach: archives written here with stored and
//! deflated entries, then cut, extended or with bytes changed, so the end record, the central
//! directory, the local headers and the entries' streams run on every pull request too.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::holds;

use geom_format::checksum::crc32;
use geom_fuzz::seeded;
use geom_fuzz::zip::harness;
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 20_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 20_000;

/// The longest byte string the fuzzing legs draw.
const MAX_LEN: u16 = 1_024;

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn zip_fuzz() {
    println!(
        "bolero test mode: corpus and crash files, then {ITERATIONS} random inputs whose seeds \
         bolero draws from the operating system; a failing input's seed is printed as \
         BOLERO_RANDOM_SEED=<seed>, which replays it"
    );
    bolero::check!()
        .with_iterations(ITERATIONS)
        .with_max_len(usize::from(MAX_LEN))
        .for_each(|data: &[u8]| holds(harness(data)));
}

#[test]
fn zip_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "zip fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

fn u16le(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn u32le(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// An archive of `entries` (name, bytes, deflated), laid out as PKWARE's APPNOTE lays one out:
/// each local header and its body, the central directory, the end record.
fn archive(entries: &[(Vec<u8>, Vec<u8>, bool)]) -> Vec<u8> {
    let mut b = Vec::new();
    let mut central = Vec::new();
    for (name, data, deflated) in entries {
        let body = if *deflated {
            miniz_oxide::deflate::compress_to_vec(data, 6)
        } else {
            data.clone()
        };
        let at = b.len() as u32;
        for (sink, local) in [(&mut b, true), (&mut central, false)] {
            u32le(sink, if local { 0x0403_4b50 } else { 0x0201_4b50 });
            if !local {
                u16le(sink, 20);
            }
            u16le(sink, 20);
            u16le(sink, 0);
            u16le(sink, if *deflated { 8 } else { 0 });
            u32le(sink, 0);
            u32le(sink, crc32(data));
            u32le(sink, body.len() as u32);
            u32le(sink, data.len() as u32);
            u16le(sink, name.len() as u16);
            u16le(sink, 0);
            if !local {
                sink.extend_from_slice(&[0; 10]);
                u32le(sink, at);
            }
            sink.extend_from_slice(name);
            if local {
                sink.extend_from_slice(&body);
            }
        }
    }
    let directory = b.len() as u32;
    b.extend_from_slice(&central);
    u32le(&mut b, 0x0605_4b50);
    u32le(&mut b, 0);
    u16le(&mut b, entries.len() as u16);
    u16le(&mut b, entries.len() as u16);
    u32le(&mut b, central.len() as u32);
    u32le(&mut b, directory);
    u16le(&mut b, 0);
    b
}

#[test]
fn seeded_archives() {
    let seed = resolve("zip fuzz target, archives", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let names: [&[u8]; 8] = [
        b"[Content_Types].xml",
        b"3D/3dmodel.model",
        b"_rels/.rels",
        b"3D/",
        b"../evil",
        b"Doc.xml",
        b"doc.XML",
        b"a%41",
    ];
    for _ in 0..1_000 {
        let count = (rng.next_u64() % 5) as usize;
        let entries: Vec<(Vec<u8>, Vec<u8>, bool)> = (0..count)
            .map(|_| {
                let name = names[(rng.next_u64() % names.len() as u64) as usize].to_vec();
                let len = (rng.next_u64() % 2_000) as usize;
                let alphabet = 1 + rng.next_u64() % 256;
                let data = (0..len)
                    .map(|_| (rng.next_u64() % alphabet) as u8)
                    .collect();
                (name, data, rng.next_u64() % 2 == 0)
            })
            .collect();
        let mut z = archive(&entries);
        match rng.next_u64() % 5 {
            0 => {}
            1 => {
                let keep = (rng.next_u64() % (z.len() as u64 + 1)) as usize;
                z.truncate(keep);
            }
            2 => z.push(rng.next_u64() as u8),
            _ => {
                for _ in 0..=rng.next_u64() % 3 {
                    let at = (rng.next_u64() % z.len() as u64) as usize;
                    z[at] = rng.next_u64() as u8;
                }
            }
        }
        holds(harness(&z));
    }
}
