//! The benchmark of piece G91 (chapter 30): column decoding, which the board asks to reach at least
//! 500 MB/s per core, and the strict decoder and text form on a record-sized item.
//!
//! Run with `cargo bench -p geom-format --bench cbor`. It uses the standard library's clock only
//! (wall-clock time on this machine, one thread; the reference machine and its nightly timings are
//! piece G107's). The inputs come from a fixed linear congruential sequence.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_format::cbor::{check, float_column, from_text, int_column, to_text};
use geom_run::{Budget, WorkMeter};

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

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// Runs `f` once to warm up, then `runs` times, and prints the mean time and the rate over `bytes`.
fn time(name: &str, runs: u32, bytes: usize, mut f: impl FnMut()) {
    f();
    let start = Instant::now();
    for _ in 0..runs {
        f();
    }
    let seconds = start.elapsed().as_secs_f64() / f64::from(runs);
    let rate = bytes as f64 / seconds / 1e6;
    println!("{name:<44} {:>10.1} µs {rate:>10.0} MB/s", seconds * 1e6);
}

/// A tagged column: tag `tag`, then a byte string head of 9 bytes, then the values.
fn tagged(tag: u8, values: &[u8]) -> Vec<u8> {
    let mut out = vec![0xd8, tag, 0x5b];
    out.extend((values.len() as u64).to_be_bytes());
    out.extend(values);
    out
}

fn main() {
    let mut lcg = Lcg(0x5350_4152_454b_4954);
    // 4,096 rows is one chunk of a body table (24.2); 2^21 rows is a 16 MiB column.
    for rows in [4096_usize, 1 << 21] {
        let ints: Vec<u8> = (0..rows).flat_map(|_| lcg.next().to_le_bytes()).collect();
        let floats: Vec<u8> = (0..rows)
            .flat_map(|_| {
                let x = (lcg.next() >> 11) as f64 / (1u64 << 53) as f64 * 1e6 - 5e5;
                x.to_le_bytes()
            })
            .collect();
        let runs = if rows > 4096 { 20 } else { 20_000 };
        let mut out_i = Vec::with_capacity(rows);
        time(
            &format!("int_column, {rows} rows"),
            runs,
            ints.len(),
            || {
                out_i.clear();
                int_column(black_box(&ints), &mut out_i, &mut meter()).unwrap();
                black_box(&out_i);
            },
        );
        let mut out_f = Vec::with_capacity(rows);
        time(
            &format!("float_column, {rows} rows"),
            runs,
            floats.len(),
            || {
                out_f.clear();
                float_column(black_box(&floats), &mut out_f, &mut meter()).unwrap();
                black_box(&out_f);
            },
        );
        let column = tagged(86, &floats);
        time(
            &format!("check of a tag 86 column, {rows} rows"),
            runs,
            column.len(),
            || {
                check(black_box(&column), 1, &mut meter()).unwrap();
            },
        );
    }
    // A record-sized item: an array of 1,000 small maps of integers, text and floats.
    let mut item = vec![0x99, 0x03, 0xe8];
    for i in 0..1000_u16 {
        item.extend([0xa3, 0x01]);
        // The shortest head of the unsigned integer i.
        match i {
            0..24 => item.push(i as u8),
            24..256 => item.extend([0x18, i as u8]),
            _ => {
                item.push(0x19);
                item.extend(i.to_be_bytes());
            }
        }
        item.extend([0x02, 0x63, b'a', b'b', b'c', 0x03, 0xfb]);
        item.extend((f64::from(i) + 0.1).to_bits().to_be_bytes());
    }
    time("check of 1,000 maps", 2000, item.len(), || {
        check(black_box(&item), 4, &mut meter()).unwrap();
    });
    let text = to_text(&item, 4, &mut meter()).unwrap();
    time("to_text of 1,000 maps", 500, item.len(), || {
        black_box(to_text(black_box(&item), 4, &mut meter()).unwrap());
    });
    time("from_text of 1,000 maps", 500, item.len(), || {
        black_box(from_text(black_box(&text), 4, &mut meter()).unwrap());
    });
}
