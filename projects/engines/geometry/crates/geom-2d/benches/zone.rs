//! How long one edit takes on a drawing of a million edges, by the standard library's clock (piece
//! G199): the done-when's "one edit on a 1M-edge drawing". Builds a lattice of about a million unit
//! edges once, then inserts a short segment, a segment across a hundred cells and a segment along
//! a whole row, each into the same version, and locates points. `ZONE_SIDE` replaces the side of
//! the lattice, 707 cells. Prints one line per stage; the gate is the count benchmark in `geom-bench`.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::print_stdout,
    reason = "a benchmark program; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::hint::black_box;
use std::time::Instant;

use geom_2d::update::Live;
use geom_num::carrier::Rational2;
use geom_num::grid::P2;
use geom_run::{Budget, WorkMeter};

fn p(x: i64, y: i64) -> P2 {
    P2::new(x, y).unwrap()
}

fn main() {
    let side: i64 = std::env::var("ZONE_SIDE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(707);
    let start = Instant::now();
    let mut segments = Vec::new();
    let mut label = 0_u32;
    for y in 0..=side {
        for x in 0..=side {
            if x < side {
                segments.push((p(x, y), p(x + 1, y), label));
                label += 1;
            }
            if y < side {
                segments.push((p(x, y), p(x, y + 1), label));
                label += 1;
            }
        }
    }
    let live = Live::from_noded(&segments).unwrap();
    println!(
        "build: {} edges in {} chunks, {:?}",
        live.len(),
        live.chunk_count(),
        start.elapsed()
    );
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let mid = side / 2;
    let edits = [
        ("short", p(mid, mid), p(mid + 2, mid + 1)),
        ("hundred cells", p(mid, mid), p(mid + 100, mid + 37)),
        ("whole row", p(0, mid), p(side, mid + 1)),
    ];
    for (name, a, b) in edits {
        let start = Instant::now();
        let (next, cost) = live.insert_costed(a, b, label, &mut meter).unwrap();
        let took = start.elapsed();
        println!(
            "edit {name}: {took:?}; {} -> {} edges; {cost:?}; {} of {} chunks shared",
            live.len(),
            next.len(),
            next.shared_chunks(&live),
            live.chunk_count()
        );
        black_box(next);
    }
    let start = Instant::now();
    let probes = 1000;
    for i in 0..probes {
        let at = Rational2::new(
            i128::from((i * 7919) % (side * 2)) + 1,
            i128::from((i * 104_729) % (side * 2)) + 1,
            2,
        )
        .unwrap();
        black_box(live.locate(&at, &mut meter).unwrap());
    }
    println!("locate: {probes} points in {:?}", start.elapsed());
}
