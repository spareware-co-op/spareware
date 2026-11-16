//! The count benchmark of EMBER's pair 827, natively: the same batch, kernel and golden check as
//! `benches/ember` (whose `pair.rs` this file includes), on any platform, with its result checked.
//!
//! The models are a download (two files of the fetched corpus `ember-cleared-pairs`), so the test
//! is ignored: run it with `SPAREKIT_EMBER` naming a folder that holds `375267.stl` and
//! `251402.stl`. Its failure message prints the golden line to record when a digest changes.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "a test: a failure stops it with its message"
)]

#[allow(
    dead_code,
    reason = "the benchmark's module, of which this test uses a part"
)]
#[path = "../benches/ember/pair.rs"]
mod pair;

#[test]
#[ignore = "reads two models of the fetched corpus ember-cleared-pairs from SPAREKIT_EMBER; \
            kernel-bench.yml measures them in the instruction gate's last part"]
fn the_pairs_digests_are_golden_after_both_passes() {
    let mut bench = pair::prepare().unwrap();
    let digests = bench.run();
    // The same digests after the gate's two passes.
    bench.measure();
    assert_eq!(bench.digests(), digests);
    if let Err(message) = pair::check(&digests) {
        panic!("{message}");
    }
}

#[test]
fn the_golden_file_holds_the_benchmark_alone() {
    let entries = geom_bench::golden::parse(pair::GOLDEN).unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e.name).collect();
    assert_eq!(names, [pair::NAME]);
}
