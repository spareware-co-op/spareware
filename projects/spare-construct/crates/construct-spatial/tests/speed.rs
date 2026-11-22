//! Speed, measured: 100,000 elements, queries timed the way the server's checklist times its own
//! (a clock around a loop, the numbers printed). Run `cargo test --release -p construct-spatial
//! --test speed -- --nocapture` to see them. The test also fails if a query is absurdly slow,
//! even in the slower test build, so a broken index is caught everywhere.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

mod common;

use common::{Seq, id, random_box};
use construct_model::geometry::Bounds;
use construct_spatial::{Entry, SpatialIndex};
use std::hint::black_box;
use std::time::Instant;

const ELEMENTS: u64 = 100_000;
/// Queries timed in each loop.
const RUNS: usize = 2_000;
/// A site 400 m by 400 m and 60 m high, in millimetres.
const EXTENT: [i64; 3] = [200_000, 200_000, 30_000];

fn micros(started: Instant, runs: usize) -> f64 {
    started.elapsed().as_secs_f64() * 1e6 / runs as f64
}

#[test]
fn a_hundred_thousand_elements() {
    let mut seq = Seq(100_000);
    let entries: Vec<Entry> = (0..ELEMENTS)
        .map(|n| Entry {
            id: id(n),
            bounds: random_box(&mut seq, EXTENT, 3_000),
            scope: None,
        })
        .collect();

    let started = Instant::now();
    let mut index = SpatialIndex::from_entries(entries.clone()).unwrap();
    println!("\n   {ELEMENTS} elements");
    println!(
        "      build in one pass: {:.0} ms",
        micros(started, 1) / 1e3
    );

    // A view-sized box: 30 m by 20 m, a storey (4 m) high, somewhere in the site.
    let views: Vec<Bounds> = (0..RUNS)
        .map(|_| {
            let x = seq.between(-EXTENT[0], EXTENT[0] - 30_000);
            let y = seq.between(-EXTENT[1], EXTENT[1] - 20_000);
            let z = seq.between(-EXTENT[2], EXTENT[2] - 4_000);
            Bounds {
                min: [x, y, z],
                max: [x + 30_000, y + 20_000, z + 4_000],
            }
        })
        .collect();

    let mut found = 0;
    let started = Instant::now();
    for view in &views {
        found += black_box(index.meeting(black_box(view), None).unwrap()).len();
    }
    let meeting = micros(started, RUNS);
    println!(
        "      meeting a view-sized box: {meeting:.1} us per query ({:.1} elements on average)",
        found as f64 / RUNS as f64
    );

    let started = Instant::now();
    for view in &views {
        black_box(index.inside(black_box(view), None).unwrap());
    }
    let inside = micros(started, RUNS);
    println!("      wholly inside a view-sized box: {inside:.1} us per query");

    let started = Instant::now();
    for view in &views {
        black_box(index.nearest(black_box(view.min), 10, None).unwrap());
    }
    let nearest = micros(started, RUNS);
    println!("      ten nearest a point: {nearest:.1} us per query");

    // One element moved: a remove and an insert in the tree.
    let moves: Vec<(usize, Bounds)> = (0..RUNS)
        .map(|_| {
            (
                usize::try_from(seq.next() % ELEMENTS).unwrap(),
                random_box(&mut seq, EXTENT, 3_000),
            )
        })
        .collect();
    let started = Instant::now();
    for (at, bounds) in &moves {
        index.update(&entries[*at].id, *bounds, None).unwrap();
    }
    let update = micros(started, RUNS);
    println!("      one element moved: {update:.1} us per update");

    // Not a promise of speed, a guard against a broken index: far above anything a working tree
    // takes, even unoptimised on a busy machine.
    assert!(meeting < 20_000.0, "meeting took {meeting} us");
    assert!(inside < 20_000.0, "inside took {inside} us");
    assert!(nearest < 20_000.0, "nearest took {nearest} us");
    assert!(update < 20_000.0, "update took {update} us");
}
