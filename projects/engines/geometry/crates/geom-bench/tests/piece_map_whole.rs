//! A measurement for Gate 4's piece map (G156, gap 9), on a measurement branch only: the
//! catalogued benchmarks whose names start with one of the prefixes in `PIECE_MAP_NAMES`, each
//! built and run for one pass, so cargo-llvm-cov reports the lines their inputs and kernels reach.
//! `piece_map_kernel.rs` is the same with the counters zeroed after the inputs are built.

#![allow(clippy::expect_used, clippy::panic, missing_docs)]

use geom_bench::catalogue::{names, prepare_named};
use geom_bench::harness::Bench as _;

fn chosen() -> Vec<String> {
    let prefixes = std::env::var("PIECE_MAP_NAMES").expect("PIECE_MAP_NAMES is set");
    let prefixes: Vec<&str> = prefixes.split_whitespace().collect();
    let chosen: Vec<String> = names()
        .into_iter()
        .filter(|n| prefixes.iter().any(|p| *p == "*" || n.starts_with(p)))
        .collect();
    assert!(!chosen.is_empty(), "no benchmark matches {prefixes:?}");
    println!("measured: {chosen:?}");
    chosen
}

#[test]
fn run_named() {
    for name in chosen() {
        let mut bench = prepare_named(&name).expect("every catalogued name prepares");
        bench.pass();
    }
}
