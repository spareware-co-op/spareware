//! The benchmarks of piece G86 (chapter 30): the canonical CBOR encoder on what SKG stores most,
//! coordinate columns of 4,096 rows (24.2's chunk) and a record of mixed scalars, and SHA-256 on a
//! 64 KiB payload and on 64-byte names.
//!
//! Run with `cargo bench -p geom-op`. It uses the standard library's clock only (wall-clock time
//! on this machine; the reference machine and instruction counts are geom-bench's, piece G18,
//! which measures geom-num's functions today). The inputs come from a fixed linear congruential
//! sequence, so every run measures the same work. Each line prints the best of seven runs and the
//! rate it means, with a checksum of the outputs so the work cannot be optimised away.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::{Duration, Instant};

use geom_op::cbor::Encoder;
use geom_op::hash::{sha256, sha256_128};

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }
}

/// The best of seven runs of `f`, which does `work` bytes or items of work per run.
fn time(name: &str, work: f64, unit: &str, mut f: impl FnMut() -> u64) {
    let mut best = Duration::MAX;
    let mut check = 0_u64;
    for _ in 0..7 {
        let start = Instant::now();
        check = check.wrapping_add(black_box(f()));
        best = best.min(start.elapsed());
    }
    let rate = work / best.as_secs_f64();
    println!(
        "{name:<34} {:>10.3} ms  {:>10.1} {unit}/s  (check {check:016x})",
        best.as_secs_f64() * 1e3,
        rate / 1e6
    );
}

fn main() {
    let mut g = Lcg(0x6386_be4c);
    // 64 chunks of 4,096 rows: grid coordinates as i64 and reals as f64.
    let ints: Vec<Vec<i64>> = (0..64)
        .map(|_| {
            (0..4096)
                .map(|_| (g.next() >> 24) as i64 - (1 << 39))
                .collect()
        })
        .collect();
    let reals: Vec<Vec<f64>> = (0..64)
        .map(|_| {
            (0..4096)
                .map(|_| f64::from_bits(0x3ff0_0000_0000_0000 | (g.next() >> 12)))
                .collect()
        })
        .collect();
    let column_bytes = f64::from(64 * 4096 * 8);
    time("cbor/i64_column 64 x 4096", column_bytes, "MB", || {
        ints.iter()
            .map(|c| {
                let mut e = Encoder::new();
                e.i64_column(c).unwrap();
                e.finish().unwrap().as_bytes().len() as u64
            })
            .sum()
    });
    time("cbor/f64_column 64 x 4096", column_bytes, "MB", || {
        reals
            .iter()
            .map(|c| {
                let mut e = Encoder::new();
                e.f64_column(c).unwrap();
                e.finish().unwrap().as_bytes().len() as u64
            })
            .sum()
    });

    // 10,000 records of mixed scalars, as a record's header and a small payload hold them.
    let scalars: Vec<(u64, i128, f64)> = (0..10_000)
        .map(|_| {
            let a = g.next();
            let b = g.next();
            let x = match a % 3 {
                0 => f64::from((b >> 48) as u16) / 4.0,
                1 => f64::from(f32::from_bits((b >> 32) as u32 & 0x7f7f_ffff)),
                _ => f64::from_bits(b >> 2),
            };
            (a >> (a % 64), (i128::from(a) << 64) >> (b % 120), x)
        })
        .collect();
    time(
        "cbor/record of 8 scalars x 10000",
        10_000.0,
        "M records",
        || {
            scalars
                .iter()
                .map(|&(u, i, x)| {
                    let mut e = Encoder::new();
                    e.array(8).unwrap();
                    e.uint(u).unwrap();
                    e.int(i).unwrap();
                    e.float(x).unwrap();
                    e.map(2).unwrap();
                    e.key(0).unwrap();
                    e.uint(u >> 7).unwrap();
                    e.key(9).unwrap();
                    e.float(-x).unwrap();
                    e.bytes(&u.to_le_bytes()).unwrap();
                    e.text("role").unwrap();
                    e.int(-i).unwrap();
                    e.uint(1).unwrap();
                    e.finish().unwrap().as_bytes().len() as u64
                })
                .sum()
        },
    );

    let payload: Vec<u8> = (0..65_536).map(|_| (g.next() >> 56) as u8).collect();
    time("hash/sha256 64 KiB x 64", 64.0 * 65_536.0, "MB", || {
        (0..64).map(|k| u64::from(sha256(&payload[k..])[0])).sum()
    });
    let names: Vec<[u8; 64]> = (0..10_000)
        .map(|_| core::array::from_fn(|_| (g.next() >> 56) as u8))
        .collect();
    time("hash/sha256_128 64 B x 10000", 10_000.0, "M names", || {
        names.iter().map(|n| u64::from(sha256_128(n)[0])).sum()
    });
}
