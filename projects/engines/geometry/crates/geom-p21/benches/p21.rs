//! The benchmarks of piece G415 (chapter 30: "100 MB/s per core"): a 64 MB Part 21 text of a
//! typical B-rep file's instances read as events, read through a source that returns 4 KiB a call,
//! read into trees, collected into a whole file, and written back canonically.
//!
//! Run with `cargo bench -p geom-p21 --bench p21`. It uses the standard library's clock only
//! (wall-clock time on this machine, one thread). The text is made by a fixed rule, with no random
//! numbers. Each line prints the best of five runs.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::format_push_string,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::{Duration, Instant};

use geom_format::source::{ByteSource, SliceSource, SourceError};
use geom_p21::{Event, Exchange, Options, Reader, Totals};
use geom_run::{Budget, WorkMeter};

fn time(name: &str, bytes: f64, mut f: impl FnMut() -> u64) {
    let mut best = Duration::MAX;
    let mut check = 0_u64;
    for _ in 0..5 {
        let start = Instant::now();
        check = black_box(f());
        best = best.min(start.elapsed());
    }
    println!(
        "{name:<40} {:>9.1} ms  {:>8.1} MB/s  (check {check})",
        best.as_secs_f64() * 1e3,
        bytes / best.as_secs_f64() / 1e6
    );
}

/// A source that returns 4 KiB a call.
struct Small<'a>(&'a [u8]);

impl ByteSource for Small<'_> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SourceError> {
        let n = buf.len().min(self.0.len()).min(4096);
        buf[..n].copy_from_slice(&self.0[..n]);
        self.0 = &self.0[n..];
        Ok(n)
    }
}

/// The instance `id` of the text: the mix of a B-rep file, mostly points, directions, edges and
/// curves, with a complex instance and a string now and then.
fn instance(id: u64, text: &mut String) {
    let (a, b, c) = (id % 1000, id % 77, id % 13);
    match id % 8 {
        0..=2 => text.push_str(&format!(
            "#{id}=CARTESIAN_POINT('',({a}.{b}5,-{b}.{c}25E-2,{c}.));\n"
        )),
        3 => text.push_str(&format!("#{id}=DIRECTION('',(0.,-1.,6.123233995736766E-17));\n")),
        4 => text.push_str(&format!(
            "#{id}=ORIENTED_EDGE('',*,*,#{},.T.);\n",
            id - 1
        )),
        5 => text.push_str(&format!(
            "#{id}=EDGE_CURVE('edge {a}',#{},#{},#{},.T.);\n",
            id - 3,
            id - 4,
            id - 5
        )),
        6 => text.push_str(&format!(
            "#{id}=B_SPLINE_CURVE_WITH_KNOTS('',3,(#{},#{},#{},#{}),.UNSPECIFIED.,.F.,.F.,(4,4),(0.,{a}.5),.UNSPECIFIED.);\n",
            id - 6, id - 5, id - 4, id - 3
        )),
        _ => text.push_str(&format!(
            "#{id}=(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{}))REPRESENTATION_CONTEXT('',$));\n",
            id - 1
        )),
    }
}

/// Reads `source` to its end; the count of parameters read (and of records built, with `trees`).
fn events<S: ByteSource>(source: S, trees: bool) -> u64 {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let mut reader = Reader::new(source, Options::DEFAULT, &mut meter);
    let mut count = 0_u64;
    while let Some(event) = reader.next_event().unwrap() {
        if let Event::Instance(_) = event {
            count += reader.tokens().params() as u64;
            if trees {
                count += black_box(reader.tokens().records()).len() as u64;
            }
        }
    }
    count
}

fn main() {
    let mut text = String::from(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\nFILE_NAME('bench','',(''),(''),'','','');\nFILE_SCHEMA(('S'));\nENDSEC;\nDATA;\n",
    );
    let mut id = 10;
    while text.len() < 64 << 20 {
        instance(id, &mut text);
        id += 1;
    }
    text.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    let text = text.into_bytes();
    let bytes = text.len() as f64;
    println!("{} instances, {:.1} MB", id - 10, bytes / 1e6);

    time("read, events", bytes, || {
        events(SliceSource::new(&text), false)
    });
    time("read, events, 4 KiB a call", bytes, || {
        events(Small(&text), false)
    });
    time("read, events and trees", bytes, || {
        events(SliceSource::new(&text), true)
    });
    let totals = Totals {
        params: usize::MAX,
        ..Totals::DEFAULT
    };
    let read = || {
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        Exchange::read(
            SliceSource::new(&text),
            Options::DEFAULT,
            totals,
            &mut meter,
        )
        .unwrap()
    };
    time("read, whole file in memory", bytes, || {
        read().entities.len() as u64
    });
    let exchange = read();
    let mut out = Vec::with_capacity(text.len() + (text.len() >> 3));
    time("write, canonical", bytes, || {
        out.clear();
        out = exchange.write(core::mem::take(&mut out)).unwrap();
        out.len() as u64
    });
}
