//! Piece G414c's benchmark: the zip reader on a package laid out as 3MF lays one out (a content
//! types stream, a relationships part, a deflated mesh model part and 200 small thumbnails and
//! textures, stored), timed for the three things an importer does: read and check the central
//! directory, hold every name to the packaging rules, and read the model part. Beside it, the same
//! model stream inflated alone, so what the container adds over its entry's DEFLATE is measured.
//!
//! Run with `cargo bench -p geom-exchange --bench zip`. It uses the standard library's clock only,
//! like the other benchmarks beside it. Coordinates come from the kernel's one generator,
//! `geom_num::rng` (Philox4x32-10), with a fixed seed and stream, so every run measures the same
//! package.
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

use geom_exchange::inflate::{InflateBudget, inflate_slice};
use geom_exchange::zip::{ZipArchive, ZipBudget};
use geom_format::checksum::crc32;
use geom_num::rng::{Philox, Seed, StreamId};
use geom_run::{Budget, WorkMeter};

const VERTICES: u64 = 100_000;
const TRIANGLES: u64 = 200_000;
const SMALL: usize = 200;
const RUNS: u32 = 5;

fn model() -> Vec<u8> {
    let mut stream = Philox::new(Seed(0x6414_C0B1), StreamId(0)).stream();
    let mut doc =
        String::from("<model unit=\"millimeter\"><resources><object id=\"1\"><mesh><vertices>\n");
    for _ in 0..VERTICES {
        let mut c = || stream.below(2_000_000).unwrap() as f64 / 1_000.0 - 1_000.0;
        let (x, y, z) = (c(), c(), c());
        writeln!(doc, "<vertex x=\"{x}\" y=\"{y}\" z=\"{z}\"/>").unwrap();
    }
    doc.push_str("</vertices><triangles>\n");
    for _ in 0..TRIANGLES {
        let mut v = || stream.below(VERTICES).unwrap();
        let (a, b, c) = (v(), v(), v());
        writeln!(doc, "<triangle v1=\"{a}\" v2=\"{b}\" v3=\"{c}\"/>").unwrap();
    }
    doc.push_str("</triangles></mesh></object></resources></model>\n");
    doc.into_bytes()
}

fn put16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn put32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// An archive of `(name, bytes, deflated)` entries, as APPNOTE 4.3.6 lays one out.
fn archive(entries: &[(String, Vec<u8>, bool)]) -> Vec<u8> {
    let (mut b, mut central) = (Vec::new(), Vec::new());
    for (name, data, deflated) in entries {
        let body = if *deflated {
            miniz_oxide::deflate::compress_to_vec(data, 6)
        } else {
            data.clone()
        };
        let at = b.len() as u32;
        for (sink, local) in [(&mut b, true), (&mut central, false)] {
            put32(sink, if local { 0x0403_4b50 } else { 0x0201_4b50 });
            if !local {
                put16(sink, 20);
            }
            put16(sink, 20);
            put16(sink, 0);
            put16(sink, if *deflated { 8 } else { 0 });
            put32(sink, 0);
            put32(sink, crc32(data));
            put32(sink, body.len() as u32);
            put32(sink, data.len() as u32);
            put16(sink, name.len() as u16);
            put16(sink, 0);
            if !local {
                sink.extend_from_slice(&[0; 10]);
                put32(sink, at);
            }
            sink.extend_from_slice(name.as_bytes());
            if local {
                sink.extend_from_slice(&body);
            }
        }
    }
    let directory = b.len() as u32;
    b.extend_from_slice(&central);
    put32(&mut b, 0x0605_4b50);
    put32(&mut b, 0);
    put16(&mut b, entries.len() as u16);
    put16(&mut b, entries.len() as u16);
    put32(&mut b, central.len() as u32);
    put32(&mut b, directory);
    put16(&mut b, 0);
    b
}

/// The best of [`RUNS`] timings of `f`, in seconds.
fn best(mut f: impl FnMut()) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..RUNS {
        let start = Instant::now();
        f();
        best = best.min(start.elapsed().as_secs_f64());
    }
    best
}

fn main() {
    let model = model();
    let mut entries = vec![
        (
            "[Content_Types].xml".to_owned(),
            b"<Types/>".to_vec(),
            false,
        ),
        (
            "_rels/.rels".to_owned(),
            b"<Relationships/>".to_vec(),
            false,
        ),
        ("3D/3dmodel.model".to_owned(), model.clone(), true),
    ];
    for i in 0..SMALL {
        entries.push((
            format!("Metadata/texture_{i}.png"),
            vec![i as u8; 2_000],
            false,
        ));
    }
    let zip = archive(&entries);
    let budget = ZipBudget::DEFAULT;
    let meter = || WorkMeter::new(Budget::UNLIMITED);
    let directory = best(|| {
        black_box(ZipArchive::read(&zip[..], &budget, &mut meter()).unwrap());
    });
    let archive = ZipArchive::read(&zip[..], &budget, &mut meter()).unwrap();
    let names = best(|| archive.check_parts().unwrap());
    let part = archive.find("3D/3dmodel.model").unwrap();
    let read = best(|| {
        let out = archive
            .read_entry(&zip[..], part, &budget, &mut meter())
            .unwrap();
        assert_eq!(out.len(), model.len());
        black_box(out);
    });
    let stream = miniz_oxide::deflate::compress_to_vec(&model, 6);
    let limit = InflateBudget {
        output: model.len(),
        ..InflateBudget::DEFAULT
    };
    let alone = best(|| {
        black_box(inflate_slice(&stream, &limit, &mut meter()).unwrap());
    });
    let mb = model.len() as f64 / 1e6;
    println!(
        "3MF package of {} entries, {:.1} MB: directory {:.0} us, names {:.0} us, model part {:.1} MB/s, \
         inflated alone {:.1} MB/s, {:.2}x its time (best of {RUNS})",
        entries.len(),
        zip.len() as f64 / 1e6,
        directory * 1e6,
        names * 1e6,
        mb / read,
        mb / alone,
        read / alone,
    );
}
