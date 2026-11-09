//! Piece G414a's benchmark: the hardened XML reader's throughput on a mesh document laid out as
//! 3MF lays out its model part (a `<vertices>` list of `<vertex x= y= z=/>` and a `<triangles>`
//! list of `<triangle v1= v2= v3=/>`), the commonest large XML an importer reads: reading the bytes
//! through a byte source, pulling every event, and decoding every attribute value.
//!
//! Run with `cargo bench -p geom-exchange --bench xml`. It uses the standard library's clock only,
//! like the exchange benchmark beside it. Coordinates come from the kernel's one generator,
//! `geom_num::rng` (Philox4x32-10), with a fixed seed and stream, so every run measures the same
//! document.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::unwrap_used,
    reason = "a benchmark is development code, like a test; the rules are for kernel code"
)]
#![allow(
    clippy::disallowed_methods,
    reason = "a benchmark measures wall-clock time by design (Instant::now and elapsed); the time is printed, never fed into anything the kernel computes"
)]

use std::fmt::Write as _;
use std::hint::black_box;
use std::time::Instant;

use geom_exchange::xml::{Event, XmlBudget, XmlText};
use geom_format::source::SliceSource;
use geom_num::rng::{Philox, Seed, StreamId};
use geom_run::{Budget, WorkMeter};

const VERTICES: u64 = 200_000;
const TRIANGLES: u64 = 400_000;
const RUNS: u32 = 5;

fn document() -> String {
    let mut stream = Philox::new(Seed(0x6414_A0B1), StreamId(0)).stream();
    let mut doc = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<model unit=\"millimeter\" xml:lang=\"en-US\">\n\
         <resources><object id=\"1\" type=\"model\"><mesh>\n<vertices>\n",
    );
    for _ in 0..VERTICES {
        let mut c = || stream.below(2_000_000).unwrap() as f64 / 1_000.0 - 1_000.0;
        let (x, y, z) = (c(), c(), c());
        writeln!(doc, "<vertex x=\"{x}\" y=\"{y}\" z=\"{z}\"/>").unwrap();
    }
    doc.push_str("</vertices>\n<triangles>\n");
    for _ in 0..TRIANGLES {
        let mut v = || stream.below(VERTICES).unwrap();
        let (a, b, c) = (v(), v(), v());
        writeln!(doc, "<triangle v1=\"{a}\" v2=\"{b}\" v3=\"{c}\"/>").unwrap();
    }
    doc.push_str("</triangles>\n</mesh></object></resources>\n</model>\n");
    doc
}

fn main() {
    let doc = document();
    let bytes = doc.len() as f64;
    let mut best = f64::INFINITY;
    let mut events = 0_u64;
    for _ in 0..RUNS {
        let start = Instant::now();
        let text =
            XmlText::read(&mut SliceSource::new(doc.as_bytes()), &XmlBudget::DEFAULT).unwrap();
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        let mut reader = text.events(XmlBudget::DEFAULT);
        let mut value = String::new();
        events = 0;
        while let Some(e) = reader.next(&mut meter).unwrap() {
            if let Event::Attribute { value: v, .. } = e {
                value.clear();
                v.decode_into(&mut value).unwrap();
                black_box(&value);
            }
            events += 1;
        }
        let s = start.elapsed().as_secs_f64();
        if s < best {
            best = s;
        }
    }
    println!(
        "XML mesh document, {:.1} MB, {events} events: {:>8.1} MB/s, {:>6.1} ns per event (best of {RUNS})",
        bytes / 1e6,
        bytes / 1e6 / best,
        best * 1e9 / events as f64,
    );
}
