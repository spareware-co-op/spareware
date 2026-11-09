//! The benchmarks of piece G108 (chapter 30): a document of one 10,000-face body (geom-topo's
//! `many_faces`) taken from a file into the reference container and written back, and opened
//! from it at both trusts, with a current stamp for the known open (24.7). The checker passes
//! every body and charges one unit, so the lines measure reading, rebuilding and the open's own
//! bookkeeping; the checker's cost is geom-check's to measure.
//!
//! Run with `cargo bench -p geom-format --bench open`. It uses the standard library's clock only
//! (wall-clock time on this machine; the count gate in geom-bench measures geom-num's functions
//! today). Each line prints the best of seven runs.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

#[path = "../../geom-topo/tests/common/mod.rs"]
mod topo;

use std::hint::black_box;
use std::time::{Duration, Instant};

use geom_format::body::write_body;
use geom_format::container::Container;
use geom_format::file::{read, write};
use geom_format::header::{Header, PRE_EDITION};
use geom_format::registry::knows;
use geom_format::source::SliceSource;
use geom_format::stamp::Stamp;
use geom_format::trust::{Checked, Checker, Level, Trust, open};
use geom_num::grid::Steps;
use geom_run::{Budget, Stopped, WorkMeter};
use geom_topo::Body;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

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

/// A checker that passes every body for one unit of work.
struct Pass;

impl Checker for Pass {
    type Report = ();

    fn version(&self) -> u64 {
        1
    }

    fn groups(&self) -> Vec<u64> {
        vec![1, 4]
    }

    fn check(
        &mut self,
        _: &Body,
        _: Level,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Checked<()>, Stopped> {
        meter.charge(1)?;
        Ok(Checked {
            ran: vec![1, 4],
            passed: true,
            report: (),
        })
    }
}

fn main() {
    let body = topo::many_faces(10_000);
    let stored = write_body(&body, &mut meter()).unwrap();
    let stamp = Stamp {
        body: stored.content_id(),
        checker: 1,
        groups: vec![1, 4],
        edition: PRE_EDITION,
    };
    let mut records = stored.chunks;
    records.push(stored.index);
    records.push(stamp.record().unwrap());
    let header = Header::v1("geom-format bench", Steps(640))
        .record()
        .unwrap();
    let bytes = write(&header, &records, &mut meter()).unwrap();
    let file = read(&mut SliceSource::new(&bytes), knows, &mut meter()).unwrap();
    let c = Container::from_file(&file, &mut meter()).unwrap();
    let n = bytes.len() as f64;

    time("container from a file", n, || {
        Container::from_file(&file, &mut meter())
            .unwrap()
            .blobs
            .len() as u64
    });
    time("container to a file", n, || {
        c.to_file(&mut meter()).unwrap().len() as u64
    });
    time("open known, stamped (cheap only)", n, || {
        let o = open(
            &c,
            &c.document,
            &c.roots,
            Trust::Known,
            &mut Pass,
            &mut meter(),
        )
        .unwrap();
        o.entries.len() as u64
    });
    time("open unknown, then the full check", n, || {
        let mut o = open(
            &c,
            &c.document,
            &c.roots,
            Trust::Unknown,
            &mut Pass,
            &mut meter(),
        )
        .unwrap();
        o.check_next(&mut Pass, &mut meter()).unwrap().unwrap_or(9) as u64
    });
}
