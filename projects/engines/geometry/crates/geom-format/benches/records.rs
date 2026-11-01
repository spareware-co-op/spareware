//! The benchmarks of piece G92 (chapter 30): records of a 4,096-row `f64` column chunk (24.2's
//! chunk) written and read, CRC-32C over a 64 KiB buffer, and a file of 64 such records read
//! whole and through a source that returns 4 KiB a call.
//!
//! Run with `cargo bench -p geom-format --bench records`. It uses the standard library's clock
//! only (wall-clock time on this machine; the count gate in geom-bench measures geom-num's
//! functions today). Inputs come from a fixed linear congruential sequence. Each line prints the
//! best of seven runs.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::{Duration, Instant};

use geom_format::checksum::crc32c;
use geom_format::file::{HEADER_TAG, read, write};
use geom_format::record::{REQUIRED, Record};
use geom_format::source::{ByteSource, SliceSource, SourceError};
use geom_op::cbor::Encoder;
use geom_run::{Budget, WorkMeter};

fn time(name: &str, bytes: f64, mut f: impl FnMut() -> u64) {
    let mut best = Duration::MAX;
    let mut check = 0_u64;
    for _ in 0..7 {
        let start = Instant::now();
        check = check.wrapping_add(black_box(f()));
        best = best.min(start.elapsed());
    }
    println!(
        "{name:<36} {:>9.3} ms  {:>9.1} MB/s  (check {check:016x})",
        best.as_secs_f64() * 1e3,
        bytes / best.as_secs_f64() / 1e6
    );
}

/// A source that returns 4 KiB a call.
struct Pages<'a>(&'a [u8]);

impl ByteSource for Pages<'_> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SourceError> {
        let n = buf.len().min(self.0.len()).min(4096);
        buf[..n].copy_from_slice(&self.0[..n]);
        self.0 = &self.0[n..];
        Ok(n)
    }
}

fn main() {
    let mut state = 0x6392_be4c_u64;
    let mut next = || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        state
    };
    let records: Vec<Record> = (0..64)
        .map(|i| {
            let column: Vec<f64> = (0..4096)
                .map(|_| f64::from_bits(0x3ff0_0000_0000_0000 | (next() >> 12)))
                .collect();
            let mut e = Encoder::new();
            e.f64_column(&column).unwrap();
            Record {
                tag: 20 + i % 2,
                flags: REQUIRED,
                schema: 1,
                depends: Vec::new(),
                payload: e.finish().unwrap().into_bytes(),
            }
        })
        .collect();
    let mut m = WorkMeter::new(Budget::UNLIMITED);
    let encoded: Vec<Vec<u8>> = records.iter().map(|r| r.encode(&mut m).unwrap()).collect();
    let mut total = 0.0;
    for e in &encoded {
        total += e.len() as f64;
    }
    time("record/encode 64 chunks of 4096 f64", total, || {
        let mut m = WorkMeter::new(Budget::UNLIMITED);
        let mut n = 0_u64;
        for r in &records {
            n = n.wrapping_add(r.encode(&mut m).unwrap().len() as u64);
        }
        n
    });
    time("record/decode 64 chunks of 4096 f64", total, || {
        let mut m = WorkMeter::new(Budget::UNLIMITED);
        let mut n = 0_u64;
        for b in &encoded {
            n = n.wrapping_add(Record::decode(b, &mut m).unwrap().1 as u64);
        }
        n
    });
    let buffer: Vec<u8> = (0..65_536).map(|_| (next() >> 56) as u8).collect();
    time("crc32c 64 KiB x 64", 64.0 * 65_536.0, || {
        (0..64)
            .map(|k| u64::from(crc32c(&buffer[k..])))
            .fold(0, |a, b| a ^ b)
    });
    let header = Record {
        tag: HEADER_TAG,
        flags: REQUIRED,
        schema: 1,
        depends: Vec::new(),
        payload: vec![0],
    };
    let file = write(&header, &records, &mut m).unwrap();
    let size = file.len() as f64;
    time("file/read 64 chunks, whole", size, || {
        let mut m = WorkMeter::new(Budget::UNLIMITED);
        read(&mut SliceSource::new(&file), |_, _| true, &mut m)
            .unwrap()
            .records
            .len() as u64
    });
    time("file/read 64 chunks, 4 KiB reads", size, || {
        let mut m = WorkMeter::new(Budget::UNLIMITED);
        read(&mut Pages(&file), |_, _| true, &mut m)
            .unwrap()
            .records
            .len() as u64
    });
}
