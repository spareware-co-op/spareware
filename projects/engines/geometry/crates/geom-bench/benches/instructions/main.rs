//! The merge gate's instruction counts, cache misses and heap use for every `geom-num` operation,
//! under Valgrind's Callgrind and DHAT, through Gungraun.
//!
//! A benchmark whose instructions, estimated cycles, cache misses or heap use rise by more than 5%
//! against a saved baseline makes this program exit with code 3, which fails the CI job
//! (`.github/workflows/kernel-bench.yml`); a benchmark whose inputs or results differ from
//! `golden.txt` fails it too. The crate documentation of `geom-bench` explains the gate, how noise
//! is removed and how to run it; the benchmarks themselves are in `gate.rs`.
//!
//! Valgrind runs on Linux here, so on other platforms this program only prints a note. That keeps
//! `cargo bench -p geom-bench` and the crate's tests working on Windows and macOS.
//!
//! Guarantees: on Linux it runs every group of `gate.rs`, so every catalogued benchmark, under
//! the configuration `gate::config` sets (`tests/coverage.rs` checks both).
//!
//! Method: Gungraun's library benchmarks (<https://github.com/gungraun/gungraun/blob/a11cdc156da6134f281302f889c1d25fa44e00da/README.md>).
//!
//! Cost: one Callgrind run and one DHAT run of every benchmark; each measures two passes of its
//! operation over its batch, and Valgrind runs it tens of times slower than native code.

#[cfg(target_os = "linux")]
mod gate;

#[cfg(target_os = "linux")]
use gate::{
    crossing::crossing, dd::dd, eft::eft, exact_sum::exact_sum, exactness::exactness,
    generated::generated, grid::grid, growth::growth, interval::interval, math::math,
    perturb::perturb, plane::plane, predicate::predicate, real::real, rng::rng, sign::sign,
    unproven::unproven, wide::wide,
};

#[cfg(target_os = "linux")]
gungraun::main!(
    config = gate::config();
    library_benchmark_groups = predicate, sign, wide, plane, crossing, eft, interval, grid, real, exactness,
        math, unproven, rng, dd, exact_sum, generated, perturb, growth
);

/// Off Linux: explain where the gate runs, and succeed.
#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!(
        "geom-bench: the instruction-count gate needs Valgrind, which runs on Linux (use WSL 2 on \
         Windows). Wall-clock timings run here: cargo bench -p geom-bench --bench wall_clock"
    );
}
