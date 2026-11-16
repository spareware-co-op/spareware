//! A measurement for Gate 4's piece map (G156, gap 9), on a measurement branch only: the
//! catalogued benchmarks whose names start with one of the prefixes in `PIECE_MAP_NAMES`, every
//! one built first, then the profile counters zeroed with the LLVM profile runtime's own
//! `__llvm_profile_reset_counters` (compiler-rt, `lib/profile/InstrProfiling.h`), then one pass of
//! each, so cargo-llvm-cov reports only the lines the measured kernels reach, not their inputs'.

#![allow(unsafe_code, clippy::expect_used, clippy::panic, missing_docs)]

use geom_bench::catalogue::{names, prepare_named};
use geom_bench::harness::Bench as _;

unsafe extern "C" {
    fn __llvm_profile_reset_counters();
}

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
    let mut benches: Vec<_> = chosen()
        .iter()
        .map(|name| prepare_named(name).expect("every catalogued name prepares"))
        .collect();
    // SAFETY: the profile runtime is linked into every binary built with -C instrument-coverage;
    // the call takes nothing and only zeroes its counters, and this test runs alone in its process.
    unsafe { __llvm_profile_reset_counters() };
    for bench in &mut benches {
        bench.pass();
    }
}
