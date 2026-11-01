//! The gate's benchmarks: one `#[library_benchmark]` per operation of
//! `geom_bench::catalogue::OPS` and of `geom_bench::catalogue::GROWTH`, one `#[bench::<class>]`
//! per input class or size, one module and one Gungraun group per catalogue group.
//! `tests/coverage.rs` checks that this file and the catalogue list exactly the same benchmarks.
//!
//! Each benchmark's argument, `leak("group/op/class")`, builds the named benchmark from the
//! catalogue outside the measured function and leaks it, so nothing is freed while counting. The
//! measured function runs `Bench::measure` (`geom_bench::harness`): two passes over the batch,
//! with Callgrind's costs zeroed at the start of each (`--zero-before`, set in [`config`]), so only
//! the warm second pass is counted (`geom_bench::harness` explains why). The teardown, outside the
//! measured function, checks the benchmark's inputs and results against `golden.txt` and ends the
//! run with a failure if they differ.
//!
//! Guarantees: a rise over [`GATE_PERCENT`] in instructions, estimated cycles or any heap measure,
//! or a golden digest that differs, fails the run; the miss counts are judged after the run from
//! its summaries (`geom_bench::callgrind`: every miss count is listed and judged through estimated
//! cycles, which price every miss, because code layout alone moves miss counts);
//! `tests/coverage.rs`, `tests/attack_g06.rs` and `tests/attack_g18a.rs` check this file's wiring.
//!
//! Identifiers are `instructions::<group>::<op> <class>`, for example
//! `instructions::predicate::orient2d nearly_degenerate`; a baseline matches benchmarks by
//! identifier.
//!
//! Method: Gungraun library benchmarks with soft limits
//! (<https://github.com/gungraun/gungraun/blob/a11cdc156da6134f281302f889c1d25fa44e00da/crates/gungraun-tests/benches/guide/lib_bench_regression.rs#L15-L19>),
//! under Callgrind with cache simulation and under DHAT
//! (<https://valgrind.org/docs/manual/cl-manual.html>, <https://valgrind.org/docs/manual/dh-manual.html>).
//!
//! Cost: per benchmark, building and leaking one batch, two passes under each Valgrind tool, and
//! one SHA-256 of the batch and its results.

use std::process::exit;

use geom_bench::GATE_PERCENT;
use geom_bench::catalogue::prepare_named;
use geom_bench::golden;
use geom_bench::harness::{Named, ZERO_BEFORE};
use gungraun::{Callgrind, Dhat, DhatMetric, EventKind, LibraryBenchmarkConfig};
use gungraun::{library_benchmark, library_benchmark_group};

/// The benchmark named `name`, built and leaked: it lives until the process ends, so the measured
/// function frees nothing. An unknown name ends the run with a failure.
fn leak(name: &'static str) -> &'static mut Named {
    match prepare_named(name) {
        Some(bench) => Box::leak(Box::new(Named { name, bench })),
        None => {
            eprintln!("geom-bench gate: no benchmark named {name} in the catalogue");
            exit(2)
        }
    }
}

/// After the measured function: the benchmark's inputs and results must match `golden.txt`, or the
/// run ends with a failure, which fails the gate.
fn verify(run: &'static mut Named) {
    if let Err(message) = golden::check(run.name, &run.bench.digests()) {
        eprintln!("geom-bench gate: golden digests differ: {message}");
        exit(1);
    }
}

/// The gate's tools and limits for a bare local run. Callgrind, with its cache simulation, counts
/// instructions, cache misses and estimated cycles; DHAT counts the heap use of the measured
/// function. Each measure has a soft limit of [`GATE_PERCENT`] here: a rise beyond it against the
/// baseline is a regression, and the run exits with code 3 after reporting every benchmark. The
/// workflow passes its own limits on the command line, which replace these
/// (`geom_bench::CALLGRIND_LIMITS` and `DHAT_LIMITS`), so the gate does not depend on this file:
/// they gate instructions, estimated cycles and heap, and `geom_bench::callgrind` judges the miss
/// counts from the run's summaries, through estimated cycles. A bare run is stricter than the gate
/// on every miss count; pass the workflow's limits to compare as CI does (the crate
/// documentation). This code is part of the measured binary, so it is left as it was.
pub fn config() -> LibraryBenchmarkConfig {
    let mut callgrind = Callgrind::default();
    callgrind.args([ZERO_BEFORE]);
    callgrind.soft_limits([
        (EventKind::Ir, GATE_PERCENT),
        (EventKind::EstimatedCycles, GATE_PERCENT),
        (EventKind::I1mr, GATE_PERCENT),
        (EventKind::D1mr, GATE_PERCENT),
        (EventKind::D1mw, GATE_PERCENT),
        (EventKind::ILmr, GATE_PERCENT),
        (EventKind::DLmr, GATE_PERCENT),
        (EventKind::DLmw, GATE_PERCENT),
    ]);
    let mut dhat = Dhat::default();
    dhat.soft_limits([
        (DhatMetric::TotalBytes, GATE_PERCENT),
        (DhatMetric::TotalBlocks, GATE_PERCENT),
        (DhatMetric::AtTGmaxBytes, GATE_PERCENT),
        (DhatMetric::MaximumBytes, GATE_PERCENT),
    ]);
    let mut config = LibraryBenchmarkConfig::default();
    config.tool(callgrind);
    config.tool(dhat);
    config
}

/// The exact predicates.
pub mod predicate {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("predicate/orient2d/typical"))]
    #[bench::degenerate(leak("predicate/orient2d/degenerate"))]
    #[bench::nearly_degenerate(leak("predicate/orient2d/nearly_degenerate"))]
    #[bench::extreme(leak("predicate/orient2d/extreme"))]
    fn orient2d(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("predicate/orient3d/typical"))]
    #[bench::degenerate(leak("predicate/orient3d/degenerate"))]
    #[bench::nearly_degenerate(leak("predicate/orient3d/nearly_degenerate"))]
    #[bench::extreme(leak("predicate/orient3d/extreme"))]
    fn orient3d(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("predicate/incircle/typical"))]
    #[bench::degenerate(leak("predicate/incircle/degenerate"))]
    #[bench::nearly_degenerate(leak("predicate/incircle/nearly_degenerate"))]
    #[bench::extreme(leak("predicate/incircle/extreme"))]
    fn incircle(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("predicate/insphere/typical"))]
    #[bench::degenerate(leak("predicate/insphere/degenerate"))]
    #[bench::nearly_degenerate(leak("predicate/insphere/nearly_degenerate"))]
    #[bench::extreme(leak("predicate/insphere/extreme"))]
    fn insphere(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = predicate;
        benchmarks = orient2d, orient3d, incircle, insphere
    );
}

/// Signs of exact integers.
pub mod sign {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("sign/of_i128/typical"))]
    fn of_i128(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = sign;
        benchmarks = of_i128
    );
}

/// Fixed-width integers at each width the kernel uses.
pub mod wide {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i256_add/typical"))]
    #[bench::extreme(leak("wide/i256_add/extreme"))]
    #[bench::overflow(leak("wide/i256_add/overflow"))]
    fn i256_add(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i256_sub/typical"))]
    #[bench::extreme(leak("wide/i256_sub/extreme"))]
    #[bench::overflow(leak("wide/i256_sub/overflow"))]
    fn i256_sub(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i256_mul/typical"))]
    #[bench::extreme(leak("wide/i256_mul/extreme"))]
    #[bench::overflow(leak("wide/i256_mul/overflow"))]
    fn i256_mul(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i256_neg/typical"))]
    #[bench::extreme(leak("wide/i256_neg/extreme"))]
    #[bench::overflow(leak("wide/i256_neg/overflow"))]
    fn i256_neg(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i256_cmp/typical"))]
    #[bench::extreme(leak("wide/i256_cmp/extreme"))]
    fn i256_cmp(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i256_signum/typical"))]
    fn i256_signum(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i256_to_i128/typical"))]
    #[bench::overflow(leak("wide/i256_to_i128/overflow"))]
    fn i256_to_i128(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i256_from_i128/typical"))]
    fn i256_from_i128(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i320_add/typical"))]
    #[bench::extreme(leak("wide/i320_add/extreme"))]
    #[bench::overflow(leak("wide/i320_add/overflow"))]
    fn i320_add(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i320_sub/typical"))]
    #[bench::extreme(leak("wide/i320_sub/extreme"))]
    #[bench::overflow(leak("wide/i320_sub/overflow"))]
    fn i320_sub(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i320_mul/typical"))]
    #[bench::extreme(leak("wide/i320_mul/extreme"))]
    #[bench::overflow(leak("wide/i320_mul/overflow"))]
    fn i320_mul(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i320_neg/typical"))]
    #[bench::extreme(leak("wide/i320_neg/extreme"))]
    #[bench::overflow(leak("wide/i320_neg/overflow"))]
    fn i320_neg(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i320_cmp/typical"))]
    #[bench::extreme(leak("wide/i320_cmp/extreme"))]
    fn i320_cmp(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i320_signum/typical"))]
    fn i320_signum(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i320_to_i128/typical"))]
    #[bench::overflow(leak("wide/i320_to_i128/overflow"))]
    fn i320_to_i128(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i320_from_i128/typical"))]
    fn i320_from_i128(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i384_add/typical"))]
    #[bench::extreme(leak("wide/i384_add/extreme"))]
    #[bench::overflow(leak("wide/i384_add/overflow"))]
    fn i384_add(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i384_sub/typical"))]
    #[bench::extreme(leak("wide/i384_sub/extreme"))]
    #[bench::overflow(leak("wide/i384_sub/overflow"))]
    fn i384_sub(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i384_mul/typical"))]
    #[bench::extreme(leak("wide/i384_mul/extreme"))]
    #[bench::overflow(leak("wide/i384_mul/overflow"))]
    fn i384_mul(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i384_neg/typical"))]
    #[bench::extreme(leak("wide/i384_neg/extreme"))]
    #[bench::overflow(leak("wide/i384_neg/overflow"))]
    fn i384_neg(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i384_cmp/typical"))]
    #[bench::extreme(leak("wide/i384_cmp/extreme"))]
    fn i384_cmp(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i384_signum/typical"))]
    fn i384_signum(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i384_to_i128/typical"))]
    #[bench::overflow(leak("wide/i384_to_i128/overflow"))]
    fn i384_to_i128(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i384_from_i128/typical"))]
    fn i384_from_i128(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i512_add/typical"))]
    #[bench::extreme(leak("wide/i512_add/extreme"))]
    #[bench::overflow(leak("wide/i512_add/overflow"))]
    fn i512_add(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i512_sub/typical"))]
    #[bench::extreme(leak("wide/i512_sub/extreme"))]
    #[bench::overflow(leak("wide/i512_sub/overflow"))]
    fn i512_sub(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i512_mul/typical"))]
    #[bench::extreme(leak("wide/i512_mul/extreme"))]
    #[bench::overflow(leak("wide/i512_mul/overflow"))]
    fn i512_mul(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i512_neg/typical"))]
    #[bench::extreme(leak("wide/i512_neg/extreme"))]
    #[bench::overflow(leak("wide/i512_neg/overflow"))]
    fn i512_neg(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i512_cmp/typical"))]
    #[bench::extreme(leak("wide/i512_cmp/extreme"))]
    fn i512_cmp(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i512_signum/typical"))]
    fn i512_signum(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i512_to_i128/typical"))]
    #[bench::overflow(leak("wide/i512_to_i128/overflow"))]
    fn i512_to_i128(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i512_from_i128/typical"))]
    fn i512_from_i128(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i256_to_i384/typical"))]
    fn i256_to_i384(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i320_to_i384/typical"))]
    fn i320_to_i384(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide/i384_to_i256/typical"))]
    #[bench::overflow(leak("wide/i384_to_i256/overflow"))]
    fn i384_to_i256(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = wide;
        benchmarks = i256_add, i256_sub, i256_mul, i256_neg, i256_cmp, i256_signum, i256_to_i128,
            i256_from_i128, i320_add, i320_sub, i320_mul, i320_neg, i320_cmp, i320_signum,
            i320_to_i128, i320_from_i128, i384_add, i384_sub, i384_mul, i384_neg, i384_cmp,
            i384_signum, i384_to_i128, i384_from_i128, i512_add, i512_sub, i512_mul, i512_neg,
            i512_cmp, i512_signum, i512_to_i128, i512_from_i128, i256_to_i384, i320_to_i384,
            i384_to_i256
    );
}

/// Fixed-width integers in 32-bit limbs (piece G42), at the bit widths of [`wide`].
pub mod wide32 {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w8_add/typical"))]
    #[bench::extreme(leak("wide32/w8_add/extreme"))]
    #[bench::overflow(leak("wide32/w8_add/overflow"))]
    fn w8_add(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w8_sub/typical"))]
    #[bench::extreme(leak("wide32/w8_sub/extreme"))]
    #[bench::overflow(leak("wide32/w8_sub/overflow"))]
    fn w8_sub(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w8_product/typical"))]
    #[bench::extreme(leak("wide32/w8_product/extreme"))]
    #[bench::overflow(leak("wide32/w8_product/overflow"))]
    fn w8_product(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w8_neg/typical"))]
    #[bench::extreme(leak("wide32/w8_neg/extreme"))]
    #[bench::overflow(leak("wide32/w8_neg/overflow"))]
    fn w8_neg(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w8_cmp/typical"))]
    #[bench::extreme(leak("wide32/w8_cmp/extreme"))]
    fn w8_cmp(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w8_signum/typical"))]
    fn w8_signum(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w8_to_i128/typical"))]
    #[bench::overflow(leak("wide32/w8_to_i128/overflow"))]
    fn w8_to_i128(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w8_from_i128/typical"))]
    fn w8_from_i128(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w10_add/typical"))]
    #[bench::extreme(leak("wide32/w10_add/extreme"))]
    #[bench::overflow(leak("wide32/w10_add/overflow"))]
    fn w10_add(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w10_sub/typical"))]
    #[bench::extreme(leak("wide32/w10_sub/extreme"))]
    #[bench::overflow(leak("wide32/w10_sub/overflow"))]
    fn w10_sub(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w10_product/typical"))]
    #[bench::extreme(leak("wide32/w10_product/extreme"))]
    #[bench::overflow(leak("wide32/w10_product/overflow"))]
    fn w10_product(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w10_neg/typical"))]
    #[bench::extreme(leak("wide32/w10_neg/extreme"))]
    #[bench::overflow(leak("wide32/w10_neg/overflow"))]
    fn w10_neg(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w10_cmp/typical"))]
    #[bench::extreme(leak("wide32/w10_cmp/extreme"))]
    fn w10_cmp(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w10_signum/typical"))]
    fn w10_signum(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w10_to_i128/typical"))]
    #[bench::overflow(leak("wide32/w10_to_i128/overflow"))]
    fn w10_to_i128(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w10_from_i128/typical"))]
    fn w10_from_i128(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w12_add/typical"))]
    #[bench::extreme(leak("wide32/w12_add/extreme"))]
    #[bench::overflow(leak("wide32/w12_add/overflow"))]
    fn w12_add(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w12_sub/typical"))]
    #[bench::extreme(leak("wide32/w12_sub/extreme"))]
    #[bench::overflow(leak("wide32/w12_sub/overflow"))]
    fn w12_sub(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w12_product/typical"))]
    #[bench::extreme(leak("wide32/w12_product/extreme"))]
    #[bench::overflow(leak("wide32/w12_product/overflow"))]
    fn w12_product(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w12_neg/typical"))]
    #[bench::extreme(leak("wide32/w12_neg/extreme"))]
    #[bench::overflow(leak("wide32/w12_neg/overflow"))]
    fn w12_neg(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w12_cmp/typical"))]
    #[bench::extreme(leak("wide32/w12_cmp/extreme"))]
    fn w12_cmp(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w12_signum/typical"))]
    fn w12_signum(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w12_to_i128/typical"))]
    #[bench::overflow(leak("wide32/w12_to_i128/overflow"))]
    fn w12_to_i128(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w12_from_i128/typical"))]
    fn w12_from_i128(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w16_add/typical"))]
    #[bench::extreme(leak("wide32/w16_add/extreme"))]
    #[bench::overflow(leak("wide32/w16_add/overflow"))]
    fn w16_add(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w16_sub/typical"))]
    #[bench::extreme(leak("wide32/w16_sub/extreme"))]
    #[bench::overflow(leak("wide32/w16_sub/overflow"))]
    fn w16_sub(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w16_product/typical"))]
    #[bench::extreme(leak("wide32/w16_product/extreme"))]
    #[bench::overflow(leak("wide32/w16_product/overflow"))]
    fn w16_product(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w16_neg/typical"))]
    #[bench::extreme(leak("wide32/w16_neg/extreme"))]
    #[bench::overflow(leak("wide32/w16_neg/overflow"))]
    fn w16_neg(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w16_cmp/typical"))]
    #[bench::extreme(leak("wide32/w16_cmp/extreme"))]
    fn w16_cmp(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w16_signum/typical"))]
    fn w16_signum(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w16_to_i128/typical"))]
    #[bench::overflow(leak("wide32/w16_to_i128/overflow"))]
    fn w16_to_i128(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w16_from_i128/typical"))]
    fn w16_from_i128(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w8_to_w12/typical"))]
    fn w8_to_w12(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w10_to_w12/typical"))]
    fn w10_to_w12(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w12_to_w8/typical"))]
    #[bench::overflow(leak("wide32/w12_to_w8/overflow"))]
    fn w12_to_w8(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w12_to_wide/typical"))]
    fn w12_to_wide(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("wide32/w3_product_i64/typical"))]
    #[bench::extreme(leak("wide32/w3_product_i64/extreme"))]
    #[bench::overflow(leak("wide32/w3_product_i64/overflow"))]
    fn w3_product_i64(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = wide32;
        benchmarks = w8_add, w8_sub, w8_product, w8_neg, w8_cmp, w8_signum, w8_to_i128, w8_from_i128, w10_add, w10_sub, w10_product, w10_neg, w10_cmp, w10_signum, w10_to_i128, w10_from_i128, w12_add, w12_sub, w12_product, w12_neg, w12_cmp, w12_signum, w12_to_i128, w12_from_i128, w16_add, w16_sub, w16_product, w16_neg, w16_cmp, w16_signum, w16_to_i128, w16_from_i128, w8_to_w12, w10_to_w12, w12_to_w8, w12_to_wide, w3_product_i64
    );
}

/// Planes and the points where three planes meet.
pub mod plane {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("plane/through/typical"))]
    #[bench::degenerate(leak("plane/through/degenerate"))]
    #[bench::extreme(leak("plane/through/extreme"))]
    fn through(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("plane/side/typical"))]
    #[bench::degenerate(leak("plane/side/degenerate"))]
    #[bench::nearly_degenerate(leak("plane/side/nearly_degenerate"))]
    #[bench::extreme(leak("plane/side/extreme"))]
    fn side(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("plane/tpi_new/typical"))]
    #[bench::degenerate(leak("plane/tpi_new/degenerate"))]
    #[bench::extreme(leak("plane/tpi_new/extreme"))]
    fn tpi_new(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("plane/tpi_homogeneous/typical"))]
    #[bench::extreme(leak("plane/tpi_homogeneous/extreme"))]
    fn tpi_homogeneous(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("plane/side_of_tpi/typical"))]
    #[bench::degenerate(leak("plane/side_of_tpi/degenerate"))]
    #[bench::nearly_degenerate(leak("plane/side_of_tpi/nearly_degenerate"))]
    #[bench::extreme(leak("plane/side_of_tpi/extreme"))]
    fn side_of_tpi(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("plane/to_grid/typical"))]
    #[bench::on_grid(leak("plane/to_grid/on_grid"))]
    #[bench::extreme(leak("plane/to_grid/extreme"))]
    #[bench::overflow(leak("plane/to_grid/overflow"))]
    fn to_grid(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("plane/lpi_new/typical"))]
    #[bench::degenerate(leak("plane/lpi_new/degenerate"))]
    #[bench::extreme(leak("plane/lpi_new/extreme"))]
    fn lpi_new(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("plane/lpi_homogeneous/typical"))]
    #[bench::extreme(leak("plane/lpi_homogeneous/extreme"))]
    fn lpi_homogeneous(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("plane/side_of_lpi/typical"))]
    #[bench::degenerate(leak("plane/side_of_lpi/degenerate"))]
    #[bench::nearly_degenerate(leak("plane/side_of_lpi/nearly_degenerate"))]
    #[bench::extreme(leak("plane/side_of_lpi/extreme"))]
    fn side_of_lpi(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("plane/lpi_to_grid/typical"))]
    #[bench::on_grid(leak("plane/lpi_to_grid/on_grid"))]
    #[bench::extreme(leak("plane/lpi_to_grid/extreme"))]
    #[bench::overflow(leak("plane/lpi_to_grid/overflow"))]
    fn lpi_to_grid(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = plane;
        benchmarks = through, side, tpi_new, tpi_homogeneous, side_of_tpi, to_grid, lpi_new,
            lpi_homogeneous, side_of_lpi, lpi_to_grid
    );
}

/// Where two lines cross in the plane (G29).
pub mod crossing {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("crossing/ssi_new/typical"))]
    #[bench::degenerate(leak("crossing/ssi_new/degenerate"))]
    #[bench::extreme(leak("crossing/ssi_new/extreme"))]
    fn ssi_new(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("crossing/ssi_homogeneous/typical"))]
    #[bench::extreme(leak("crossing/ssi_homogeneous/extreme"))]
    fn ssi_homogeneous(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("crossing/ssi_orient2d/typical"))]
    #[bench::degenerate(leak("crossing/ssi_orient2d/degenerate"))]
    #[bench::nearly_degenerate(leak("crossing/ssi_orient2d/nearly_degenerate"))]
    #[bench::extreme(leak("crossing/ssi_orient2d/extreme"))]
    fn ssi_orient2d(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("crossing/ssi_to_grid/typical"))]
    #[bench::on_grid(leak("crossing/ssi_to_grid/on_grid"))]
    #[bench::extreme(leak("crossing/ssi_to_grid/extreme"))]
    #[bench::overflow(leak("crossing/ssi_to_grid/overflow"))]
    fn ssi_to_grid(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = crossing;
        benchmarks = ssi_new, ssi_homogeneous, ssi_orient2d, ssi_to_grid
    );
}

/// Error-free transformations.
pub mod eft {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("eft/two_sum/typical"))]
    #[bench::extreme(leak("eft/two_sum/extreme"))]
    fn two_sum(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("eft/fast_two_sum/typical"))]
    #[bench::extreme(leak("eft/fast_two_sum/extreme"))]
    fn fast_two_sum(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("eft/split/typical"))]
    #[bench::extreme(leak("eft/split/extreme"))]
    fn split(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("eft/two_prod/typical"))]
    #[bench::extreme(leak("eft/two_prod/extreme"))]
    fn two_prod(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = eft;
        benchmarks = two_sum, fast_two_sum, split, two_prod
    );
}

/// Interval arithmetic.
pub mod interval {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("interval/new/typical"))]
    #[bench::extreme(leak("interval/new/extreme"))]
    fn new(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("interval/contains/typical"))]
    fn contains(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("interval/add/typical"))]
    #[bench::extreme(leak("interval/add/extreme"))]
    fn add(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("interval/sub/typical"))]
    #[bench::extreme(leak("interval/sub/extreme"))]
    fn sub(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("interval/mul/typical"))]
    #[bench::extreme(leak("interval/mul/extreme"))]
    fn mul(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("interval/div/typical"))]
    #[bench::extreme(leak("interval/div/extreme"))]
    fn div(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("interval/mul_add/typical"))]
    #[bench::extreme(leak("interval/mul_add/extreme"))]
    fn mul_add(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("interval/square/typical"))]
    #[bench::extreme(leak("interval/square/extreme"))]
    fn square(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("interval/sqrt/typical"))]
    #[bench::extreme(leak("interval/sqrt/extreme"))]
    fn sqrt(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("interval/norm/typical"))]
    #[bench::extreme(leak("interval/norm/extreme"))]
    fn norm(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("interval/neg/typical"))]
    fn neg(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("interval/certain_sign/typical"))]
    fn certain_sign(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("interval/sinpi/typical"))]
    #[bench::extreme(leak("interval/sinpi/extreme"))]
    fn sinpi(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("interval/cospi/typical"))]
    #[bench::extreme(leak("interval/cospi/extreme"))]
    fn cospi(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("interval/exp/typical"))]
    #[bench::extreme(leak("interval/exp/extreme"))]
    fn exp(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("interval/ln/typical"))]
    #[bench::extreme(leak("interval/ln/extreme"))]
    fn ln(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("interval/atan2/typical"))]
    #[bench::extreme(leak("interval/atan2/extreme"))]
    fn atan2(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = interval;
        benchmarks = new, contains, add, sub, mul, div, mul_add, square, sqrt, norm, neg, sinpi,
            cospi, exp, ln, atan2, certain_sign
    );
}

/// Grid coordinates and grid steps.
pub mod grid {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("grid/coord_new/typical"))]
    #[bench::overflow(leak("grid/coord_new/overflow"))]
    fn coord_new(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("grid/coord_from_i128/typical"))]
    #[bench::overflow(leak("grid/coord_from_i128/overflow"))]
    fn coord_from_i128(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("grid/p2_new/typical"))]
    #[bench::overflow(leak("grid/p2_new/overflow"))]
    fn p2_new(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("grid/p3_new/typical"))]
    #[bench::overflow(leak("grid/p3_new/overflow"))]
    fn p3_new(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("grid/refine/typical"))]
    #[bench::overflow(leak("grid/refine/overflow"))]
    #[bench::wrong_direction(leak("grid/refine/wrong_direction"))]
    fn refine(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("grid/coarsen/typical"))]
    #[bench::extreme(leak("grid/coarsen/extreme"))]
    #[bench::wrong_direction(leak("grid/coarsen/wrong_direction"))]
    fn coarsen(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("grid/steps_refine/typical"))]
    #[bench::overflow(leak("grid/steps_refine/overflow"))]
    #[bench::wrong_direction(leak("grid/steps_refine/wrong_direction"))]
    fn steps_refine(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("grid/steps_per/typical"))]
    fn steps_per(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = grid;
        benchmarks = coord_new, coord_from_i128, p2_new, p3_new, refine, coarsen, steps_refine,
            steps_per
    );
}

/// Finite doubles in total order.
pub mod real {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("real/total_max/typical"))]
    fn total_max(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("real/total_min/typical"))]
    fn total_min(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("real/cmp/typical"))]
    fn cmp(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = real;
        benchmarks = total_max, total_min, cmp
    );
}

/// Exactness classes.
pub mod exactness {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("exactness/combine/typical"))]
    fn combine(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("exactness/within/typical"))]
    fn within(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("exactness/combine_within/typical"))]
    fn combine_within(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = exactness;
        benchmarks = combine, within, combine_within
    );
}

/// The deterministic maths functions with proven error bounds.
pub mod math {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("math/sqrt/typical"))]
    #[bench::extreme(leak("math/sqrt/extreme"))]
    fn sqrt(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("math/sinpi/typical"))]
    #[bench::extreme(leak("math/sinpi/extreme"))]
    fn sinpi(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("math/cospi/typical"))]
    #[bench::extreme(leak("math/cospi/extreme"))]
    fn cospi(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("math/exp/typical"))]
    #[bench::extreme(leak("math/exp/extreme"))]
    fn exp(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("math/ln/typical"))]
    #[bench::extreme(leak("math/ln/extreme"))]
    fn ln(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("math/atan2/typical"))]
    #[bench::extreme(leak("math/atan2/extreme"))]
    fn atan2(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("math/fma/typical"))]
    #[bench::extreme(leak("math/fma/extreme"))]
    fn fma(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("math/abs/typical"))]
    #[bench::extreme(leak("math/abs/extreme"))]
    fn abs(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("math/floor/typical"))]
    #[bench::extreme(leak("math/floor/extreme"))]
    fn floor(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("math/ceil/typical"))]
    #[bench::extreme(leak("math/ceil/extreme"))]
    fn ceil(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("math/round_ties_even/typical"))]
    #[bench::extreme(leak("math/round_ties_even/extreme"))]
    fn round_ties_even(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = math;
        benchmarks = sqrt, sinpi, cospi, exp, ln, atan2, fma, abs, floor, ceil, round_ties_even
    );
}

/// The maths functions without a proven error bound.
pub mod unproven {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("unproven/sin/typical"))]
    #[bench::extreme(leak("unproven/sin/extreme"))]
    fn sin(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("unproven/cos/typical"))]
    #[bench::extreme(leak("unproven/cos/extreme"))]
    fn cos(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("unproven/tan/typical"))]
    #[bench::extreme(leak("unproven/tan/extreme"))]
    fn tan(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("unproven/asin/typical"))]
    #[bench::extreme(leak("unproven/asin/extreme"))]
    fn asin(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("unproven/acos/typical"))]
    #[bench::extreme(leak("unproven/acos/extreme"))]
    fn acos(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("unproven/atan/typical"))]
    #[bench::extreme(leak("unproven/atan/extreme"))]
    fn atan(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("unproven/atan2/typical"))]
    #[bench::extreme(leak("unproven/atan2/extreme"))]
    fn atan2(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("unproven/exp/typical"))]
    #[bench::extreme(leak("unproven/exp/extreme"))]
    fn exp(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("unproven/ln/typical"))]
    #[bench::extreme(leak("unproven/ln/extreme"))]
    fn ln(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("unproven/log2/typical"))]
    #[bench::extreme(leak("unproven/log2/extreme"))]
    fn log2(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("unproven/log10/typical"))]
    #[bench::extreme(leak("unproven/log10/extreme"))]
    fn log10(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("unproven/pow/typical"))]
    #[bench::extreme(leak("unproven/pow/extreme"))]
    fn pow(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("unproven/powi/typical"))]
    #[bench::extreme(leak("unproven/powi/extreme"))]
    fn powi(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("unproven/hypot/typical"))]
    #[bench::extreme(leak("unproven/hypot/extreme"))]
    fn hypot(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("unproven/cbrt/typical"))]
    #[bench::extreme(leak("unproven/cbrt/extreme"))]
    fn cbrt(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("unproven/sinpi/typical"))]
    #[bench::extreme(leak("unproven/sinpi/extreme"))]
    fn sinpi(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("unproven/cospi/typical"))]
    #[bench::extreme(leak("unproven/cospi/extreme"))]
    fn cospi(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = unproven;
        benchmarks = sin, cos, tan, asin, acos, atan, atan2, exp, ln, log2, log10, pow, powi, hypot, cbrt, sinpi, cospi
    );
}

/// Counter-based random numbers.
pub mod rng {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("rng/philox4x32_10/typical"))]
    fn philox4x32_10(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("rng/stream_from/typical"))]
    #[bench::extreme(leak("rng/stream_from/extreme"))]
    fn stream_from(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("rng/next_u32/typical"))]
    #[bench::extreme(leak("rng/next_u32/extreme"))]
    #[bench::overflow(leak("rng/next_u32/overflow"))]
    fn next_u32(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("rng/next_u64/typical"))]
    #[bench::extreme(leak("rng/next_u64/extreme"))]
    #[bench::overflow(leak("rng/next_u64/overflow"))]
    fn next_u64(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("rng/next_f64/typical"))]
    #[bench::extreme(leak("rng/next_f64/extreme"))]
    #[bench::overflow(leak("rng/next_f64/overflow"))]
    fn next_f64(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("rng/below/typical"))]
    #[bench::extreme(leak("rng/below/extreme"))]
    #[bench::overflow(leak("rng/below/overflow"))]
    fn below(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("rng/range_inclusive/typical"))]
    #[bench::extreme(leak("rng/range_inclusive/extreme"))]
    #[bench::overflow(leak("rng/range_inclusive/overflow"))]
    fn range_inclusive(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("rng/range_philox/typical"))]
    #[bench::overflow(leak("rng/range_philox/overflow"))]
    fn range_philox(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("rng/chunk/typical"))]
    #[bench::extreme(leak("rng/chunk/extreme"))]
    #[bench::overflow(leak("rng/chunk/overflow"))]
    fn chunk(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = rng;
        benchmarks = philox4x32_10, stream_from, next_u32, next_u64, next_f64, below, range_inclusive, range_philox, chunk
    );
}

/// Double-double arithmetic.
pub mod dd {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("dd/two_prod_fma/typical"))]
    #[bench::extreme(leak("dd/two_prod_fma/extreme"))]
    fn two_prod_fma(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("dd/from_sum/typical"))]
    #[bench::extreme(leak("dd/from_sum/extreme"))]
    fn from_sum(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("dd/exact_product/typical"))]
    #[bench::extreme(leak("dd/exact_product/extreme"))]
    fn exact_product(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("dd/from_i64/typical"))]
    fn from_i64(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("dd/from_i128/typical"))]
    #[bench::overflow(leak("dd/from_i128/overflow"))]
    fn from_i128(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("dd/sign/typical"))]
    fn sign(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("dd/negate/typical"))]
    fn negate(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("dd/abs/typical"))]
    fn abs(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("dd/add/typical"))]
    #[bench::extreme(leak("dd/add/extreme"))]
    fn add(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("dd/add_f64/typical"))]
    #[bench::extreme(leak("dd/add_f64/extreme"))]
    fn add_f64(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("dd/mul_f64/typical"))]
    #[bench::extreme(leak("dd/mul_f64/extreme"))]
    fn mul_f64(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("dd/mul/typical"))]
    #[bench::extreme(leak("dd/mul/extreme"))]
    fn mul(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("dd/div_f64/typical"))]
    #[bench::extreme(leak("dd/div_f64/extreme"))]
    fn div_f64(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("dd/div/typical"))]
    #[bench::extreme(leak("dd/div/extreme"))]
    fn div(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("dd/sqrt/typical"))]
    #[bench::extreme(leak("dd/sqrt/extreme"))]
    fn sqrt(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("dd/round_shifted/typical"))]
    #[bench::extreme(leak("dd/round_shifted/extreme"))]
    #[bench::overflow(leak("dd/round_shifted/overflow"))]
    fn round_shifted(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("dd/cmp/typical"))]
    fn cmp(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = dd;
        benchmarks = two_prod_fma, from_sum, exact_product, from_i64, from_i128, sign, negate, abs, add, add_f64, mul_f64, mul, div_f64, div, sqrt, round_shifted, cmp
    );
}

/// The exact sum of doubles.
pub mod exact_sum {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("exact_sum/add/typical"))]
    #[bench::extreme(leak("exact_sum/add/extreme"))]
    fn add(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("exact_sum/add_slice/typical"))]
    #[bench::extreme(leak("exact_sum/add_slice/extreme"))]
    fn add_slice(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("exact_sum/add_iter/typical"))]
    #[bench::extreme(leak("exact_sum/add_iter/extreme"))]
    fn add_iter(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("exact_sum/merge/typical"))]
    #[bench::extreme(leak("exact_sum/merge/extreme"))]
    fn merge(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("exact_sum/round/typical"))]
    #[bench::extreme(leak("exact_sum/round/extreme"))]
    fn round(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("exact_sum/round_ieee/typical"))]
    #[bench::extreme(leak("exact_sum/round_ieee/extreme"))]
    fn round_ieee(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = exact_sum;
        benchmarks = add, add_slice, add_iter, merge, round, round_ieee
    );
}

/// The generated filters and exact values.
pub mod generated {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/orient2d_filter/typical"))]
    #[bench::degenerate(leak("generated/orient2d_filter/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/orient2d_filter/nearly_degenerate"))]
    #[bench::extreme(leak("generated/orient2d_filter/extreme"))]
    fn orient2d_filter(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/orient2d_value/typical"))]
    #[bench::degenerate(leak("generated/orient2d_value/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/orient2d_value/nearly_degenerate"))]
    #[bench::extreme(leak("generated/orient2d_value/extreme"))]
    fn orient2d_value(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/orient3d_filter/typical"))]
    #[bench::degenerate(leak("generated/orient3d_filter/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/orient3d_filter/nearly_degenerate"))]
    #[bench::extreme(leak("generated/orient3d_filter/extreme"))]
    fn orient3d_filter(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/orient3d_value/typical"))]
    #[bench::degenerate(leak("generated/orient3d_value/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/orient3d_value/nearly_degenerate"))]
    #[bench::extreme(leak("generated/orient3d_value/extreme"))]
    fn orient3d_value(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/incircle_filter/typical"))]
    #[bench::degenerate(leak("generated/incircle_filter/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/incircle_filter/nearly_degenerate"))]
    #[bench::extreme(leak("generated/incircle_filter/extreme"))]
    fn incircle_filter(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/incircle_value/typical"))]
    #[bench::degenerate(leak("generated/incircle_value/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/incircle_value/nearly_degenerate"))]
    #[bench::extreme(leak("generated/incircle_value/extreme"))]
    fn incircle_value(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/insphere_filter/typical"))]
    #[bench::degenerate(leak("generated/insphere_filter/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/insphere_filter/nearly_degenerate"))]
    #[bench::extreme(leak("generated/insphere_filter/extreme"))]
    fn insphere_filter(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/insphere_value/typical"))]
    #[bench::degenerate(leak("generated/insphere_value/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/insphere_value/nearly_degenerate"))]
    #[bench::extreme(leak("generated/insphere_value/extreme"))]
    fn insphere_value(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/plane_side_filter/typical"))]
    #[bench::degenerate(leak("generated/plane_side_filter/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/plane_side_filter/nearly_degenerate"))]
    #[bench::extreme(leak("generated/plane_side_filter/extreme"))]
    fn plane_side_filter(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/plane_side_value/typical"))]
    #[bench::degenerate(leak("generated/plane_side_value/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/plane_side_value/nearly_degenerate"))]
    #[bench::extreme(leak("generated/plane_side_value/extreme"))]
    fn plane_side_value(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/tpi_weight_filter/typical"))]
    #[bench::degenerate(leak("generated/tpi_weight_filter/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/tpi_weight_filter/nearly_degenerate"))]
    #[bench::extreme(leak("generated/tpi_weight_filter/extreme"))]
    fn tpi_weight_filter(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/tpi_weight_value/typical"))]
    #[bench::degenerate(leak("generated/tpi_weight_value/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/tpi_weight_value/nearly_degenerate"))]
    #[bench::extreme(leak("generated/tpi_weight_value/extreme"))]
    fn tpi_weight_value(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/tpi_side_filter/typical"))]
    #[bench::degenerate(leak("generated/tpi_side_filter/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/tpi_side_filter/nearly_degenerate"))]
    #[bench::extreme(leak("generated/tpi_side_filter/extreme"))]
    fn tpi_side_filter(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/tpi_side_value/typical"))]
    #[bench::degenerate(leak("generated/tpi_side_value/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/tpi_side_value/nearly_degenerate"))]
    #[bench::extreme(leak("generated/tpi_side_value/extreme"))]
    fn tpi_side_value(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/lpi_weight_filter/typical"))]
    #[bench::degenerate(leak("generated/lpi_weight_filter/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/lpi_weight_filter/nearly_degenerate"))]
    #[bench::extreme(leak("generated/lpi_weight_filter/extreme"))]
    fn lpi_weight_filter(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/lpi_weight_value/typical"))]
    #[bench::degenerate(leak("generated/lpi_weight_value/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/lpi_weight_value/nearly_degenerate"))]
    #[bench::extreme(leak("generated/lpi_weight_value/extreme"))]
    fn lpi_weight_value(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/lpi_side_filter/typical"))]
    #[bench::degenerate(leak("generated/lpi_side_filter/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/lpi_side_filter/nearly_degenerate"))]
    #[bench::extreme(leak("generated/lpi_side_filter/extreme"))]
    fn lpi_side_filter(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/lpi_side_value/typical"))]
    #[bench::degenerate(leak("generated/lpi_side_value/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/lpi_side_value/nearly_degenerate"))]
    #[bench::extreme(leak("generated/lpi_side_value/extreme"))]
    fn lpi_side_value(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/ssi_weight_filter/typical"))]
    #[bench::degenerate(leak("generated/ssi_weight_filter/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/ssi_weight_filter/nearly_degenerate"))]
    #[bench::extreme(leak("generated/ssi_weight_filter/extreme"))]
    fn ssi_weight_filter(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/ssi_weight_value/typical"))]
    #[bench::degenerate(leak("generated/ssi_weight_value/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/ssi_weight_value/nearly_degenerate"))]
    #[bench::extreme(leak("generated/ssi_weight_value/extreme"))]
    fn ssi_weight_value(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/ssi_side_filter/typical"))]
    #[bench::degenerate(leak("generated/ssi_side_filter/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/ssi_side_filter/nearly_degenerate"))]
    #[bench::extreme(leak("generated/ssi_side_filter/extreme"))]
    fn ssi_side_filter(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/ssi_side_value/typical"))]
    #[bench::degenerate(leak("generated/ssi_side_value/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/ssi_side_value/nearly_degenerate"))]
    #[bench::extreme(leak("generated/ssi_side_value/extreme"))]
    fn ssi_side_value(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/orient2d_value_plain/typical"))]
    #[bench::degenerate(leak("generated/orient2d_value_plain/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/orient2d_value_plain/nearly_degenerate"))]
    #[bench::extreme(leak("generated/orient2d_value_plain/extreme"))]
    fn orient2d_value_plain(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/orient3d_value_plain/typical"))]
    #[bench::degenerate(leak("generated/orient3d_value_plain/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/orient3d_value_plain/nearly_degenerate"))]
    #[bench::extreme(leak("generated/orient3d_value_plain/extreme"))]
    fn orient3d_value_plain(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/incircle_value_plain/typical"))]
    #[bench::degenerate(leak("generated/incircle_value_plain/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/incircle_value_plain/nearly_degenerate"))]
    #[bench::extreme(leak("generated/incircle_value_plain/extreme"))]
    fn incircle_value_plain(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/insphere_value_plain/typical"))]
    #[bench::degenerate(leak("generated/insphere_value_plain/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/insphere_value_plain/nearly_degenerate"))]
    #[bench::extreme(leak("generated/insphere_value_plain/extreme"))]
    fn insphere_value_plain(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/plane_side_value_plain/typical"))]
    #[bench::degenerate(leak("generated/plane_side_value_plain/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/plane_side_value_plain/nearly_degenerate"))]
    #[bench::extreme(leak("generated/plane_side_value_plain/extreme"))]
    fn plane_side_value_plain(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/tpi_weight_value_plain/typical"))]
    #[bench::degenerate(leak("generated/tpi_weight_value_plain/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/tpi_weight_value_plain/nearly_degenerate"))]
    #[bench::extreme(leak("generated/tpi_weight_value_plain/extreme"))]
    fn tpi_weight_value_plain(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/tpi_side_value_plain/typical"))]
    #[bench::degenerate(leak("generated/tpi_side_value_plain/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/tpi_side_value_plain/nearly_degenerate"))]
    #[bench::extreme(leak("generated/tpi_side_value_plain/extreme"))]
    fn tpi_side_value_plain(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/lpi_weight_value_plain/typical"))]
    #[bench::degenerate(leak("generated/lpi_weight_value_plain/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/lpi_weight_value_plain/nearly_degenerate"))]
    #[bench::extreme(leak("generated/lpi_weight_value_plain/extreme"))]
    fn lpi_weight_value_plain(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/lpi_side_value_plain/typical"))]
    #[bench::degenerate(leak("generated/lpi_side_value_plain/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/lpi_side_value_plain/nearly_degenerate"))]
    #[bench::extreme(leak("generated/lpi_side_value_plain/extreme"))]
    fn lpi_side_value_plain(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/ssi_weight_value_plain/typical"))]
    #[bench::degenerate(leak("generated/ssi_weight_value_plain/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/ssi_weight_value_plain/nearly_degenerate"))]
    #[bench::extreme(leak("generated/ssi_weight_value_plain/extreme"))]
    fn ssi_weight_value_plain(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/ssi_side_value_plain/typical"))]
    #[bench::degenerate(leak("generated/ssi_side_value_plain/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/ssi_side_value_plain/nearly_degenerate"))]
    #[bench::extreme(leak("generated/ssi_side_value_plain/extreme"))]
    fn ssi_side_value_plain(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/orient2d_avnaim/typical"))]
    #[bench::degenerate(leak("generated/orient2d_avnaim/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/orient2d_avnaim/nearly_degenerate"))]
    #[bench::extreme(leak("generated/orient2d_avnaim/extreme"))]
    fn orient2d_avnaim(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/orient3d_avnaim/typical"))]
    #[bench::degenerate(leak("generated/orient3d_avnaim/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/orient3d_avnaim/nearly_degenerate"))]
    #[bench::extreme(leak("generated/orient3d_avnaim/extreme"))]
    fn orient3d_avnaim(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/orient2d_value_limbs/typical"))]
    #[bench::degenerate(leak("generated/orient2d_value_limbs/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/orient2d_value_limbs/nearly_degenerate"))]
    #[bench::extreme(leak("generated/orient2d_value_limbs/extreme"))]
    fn orient2d_value_limbs(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/orient3d_value_limbs/typical"))]
    #[bench::degenerate(leak("generated/orient3d_value_limbs/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/orient3d_value_limbs/nearly_degenerate"))]
    #[bench::extreme(leak("generated/orient3d_value_limbs/extreme"))]
    fn orient3d_value_limbs(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/incircle_value_limbs/typical"))]
    #[bench::degenerate(leak("generated/incircle_value_limbs/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/incircle_value_limbs/nearly_degenerate"))]
    #[bench::extreme(leak("generated/incircle_value_limbs/extreme"))]
    fn incircle_value_limbs(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/insphere_value_limbs/typical"))]
    #[bench::degenerate(leak("generated/insphere_value_limbs/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/insphere_value_limbs/nearly_degenerate"))]
    #[bench::extreme(leak("generated/insphere_value_limbs/extreme"))]
    fn insphere_value_limbs(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/plane_side_value_limbs/typical"))]
    #[bench::degenerate(leak("generated/plane_side_value_limbs/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/plane_side_value_limbs/nearly_degenerate"))]
    #[bench::extreme(leak("generated/plane_side_value_limbs/extreme"))]
    fn plane_side_value_limbs(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/tpi_weight_value_limbs/typical"))]
    #[bench::degenerate(leak("generated/tpi_weight_value_limbs/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/tpi_weight_value_limbs/nearly_degenerate"))]
    #[bench::extreme(leak("generated/tpi_weight_value_limbs/extreme"))]
    fn tpi_weight_value_limbs(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/tpi_side_value_limbs/typical"))]
    #[bench::degenerate(leak("generated/tpi_side_value_limbs/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/tpi_side_value_limbs/nearly_degenerate"))]
    #[bench::extreme(leak("generated/tpi_side_value_limbs/extreme"))]
    fn tpi_side_value_limbs(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/lpi_weight_value_limbs/typical"))]
    #[bench::degenerate(leak("generated/lpi_weight_value_limbs/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/lpi_weight_value_limbs/nearly_degenerate"))]
    #[bench::extreme(leak("generated/lpi_weight_value_limbs/extreme"))]
    fn lpi_weight_value_limbs(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/lpi_side_value_limbs/typical"))]
    #[bench::degenerate(leak("generated/lpi_side_value_limbs/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/lpi_side_value_limbs/nearly_degenerate"))]
    #[bench::extreme(leak("generated/lpi_side_value_limbs/extreme"))]
    fn lpi_side_value_limbs(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/ssi_weight_value_limbs/typical"))]
    #[bench::degenerate(leak("generated/ssi_weight_value_limbs/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/ssi_weight_value_limbs/nearly_degenerate"))]
    #[bench::extreme(leak("generated/ssi_weight_value_limbs/extreme"))]
    fn ssi_weight_value_limbs(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("generated/ssi_side_value_limbs/typical"))]
    #[bench::degenerate(leak("generated/ssi_side_value_limbs/degenerate"))]
    #[bench::nearly_degenerate(leak("generated/ssi_side_value_limbs/nearly_degenerate"))]
    #[bench::extreme(leak("generated/ssi_side_value_limbs/extreme"))]
    fn ssi_side_value_limbs(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = generated;
        benchmarks = orient2d_filter, orient2d_value, orient3d_filter, orient3d_value, incircle_filter, incircle_value, insphere_filter, insphere_value, plane_side_filter, plane_side_value,
            tpi_weight_filter, tpi_weight_value, tpi_side_filter, tpi_side_value, lpi_weight_filter, lpi_weight_value, lpi_side_filter, lpi_side_value, ssi_weight_filter, ssi_weight_value, ssi_side_filter, ssi_side_value,
            orient2d_value_plain, orient3d_value_plain, incircle_value_plain, insphere_value_plain, plane_side_value_plain, tpi_weight_value_plain, tpi_side_value_plain, lpi_weight_value_plain, lpi_side_value_plain, ssi_weight_value_plain, ssi_side_value_plain,
            orient2d_avnaim, orient3d_avnaim, orient2d_value_limbs, orient3d_value_limbs, incircle_value_limbs, insphere_value_limbs, plane_side_value_limbs, tpi_weight_value_limbs, tpi_side_value_limbs, lpi_weight_value_limbs, lpi_side_value_limbs, ssi_weight_value_limbs, ssi_side_value_limbs
    );
}

/// The growth checks: each scale benchmark at n, 2n and 4n limbs.
pub mod growth {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::l2(leak("growth/wide_add/l2"))]
    #[bench::l4(leak("growth/wide_add/l4"))]
    #[bench::l8(leak("growth/wide_add/l8"))]
    fn wide_add(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::l2(leak("growth/wide_sub/l2"))]
    #[bench::l4(leak("growth/wide_sub/l4"))]
    #[bench::l8(leak("growth/wide_sub/l8"))]
    fn wide_sub(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::l2(leak("growth/wide_neg/l2"))]
    #[bench::l4(leak("growth/wide_neg/l4"))]
    #[bench::l8(leak("growth/wide_neg/l8"))]
    fn wide_neg(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::l2(leak("growth/wide_cmp/l2"))]
    #[bench::l4(leak("growth/wide_cmp/l4"))]
    #[bench::l8(leak("growth/wide_cmp/l8"))]
    fn wide_cmp(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::l2(leak("growth/wide_mul/l2"))]
    #[bench::l4(leak("growth/wide_mul/l4"))]
    #[bench::l8(leak("growth/wide_mul/l8"))]
    fn wide_mul(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = growth;
        benchmarks = wide_add, wide_sub, wide_neg, wide_cmp, wide_mul
    );
}

/// The perturbed predicates (G30).
pub mod perturb {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("perturb/orient2d/typical"))]
    #[bench::degenerate(leak("perturb/orient2d/degenerate"))]
    fn orient2d(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("perturb/orient3d/typical"))]
    #[bench::degenerate(leak("perturb/orient3d/degenerate"))]
    fn orient3d(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("perturb/incircle/typical"))]
    #[bench::degenerate(leak("perturb/incircle/degenerate"))]
    fn incircle(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("perturb/insphere/typical"))]
    #[bench::degenerate(leak("perturb/insphere/degenerate"))]
    fn insphere(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = perturb;
        benchmarks = orient2d, orient3d, incircle, insphere
    );
}

/// The interval Newton operators (G52).
pub mod newton {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("newton/krawczyk_2/typical"))]
    #[bench::extreme(leak("newton/krawczyk_2/extreme"))]
    fn krawczyk_2(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("newton/hansen_sengupta_2/typical"))]
    #[bench::extreme(leak("newton/hansen_sengupta_2/extreme"))]
    fn hansen_sengupta_2(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("newton/krawczyk_4/typical"))]
    #[bench::extreme(leak("newton/krawczyk_4/extreme"))]
    fn krawczyk_4(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("newton/hansen_sengupta_4/typical"))]
    #[bench::extreme(leak("newton/hansen_sengupta_4/extreme"))]
    fn hansen_sengupta_4(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = newton;
        benchmarks = krawczyk_2, hansen_sengupta_2, krawczyk_4, hansen_sengupta_4
    );
}

/// The Jacobians of the dual numbers (G52).
pub mod dual {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("dual/jacobian_real/typical"))]
    #[bench::extreme(leak("dual/jacobian_real/extreme"))]
    fn jacobian_real(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("dual/jacobian_interval/typical"))]
    #[bench::extreme(leak("dual/jacobian_interval/extreme"))]
    fn jacobian_interval(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = dual;
        benchmarks = jacobian_real, jacobian_interval
    );
}

/// The root-isolation engine (G53).
pub mod isolate {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("isolate/cubic_1/typical"))]
    #[bench::extreme(leak("isolate/cubic_1/extreme"))]
    fn cubic_1(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("isolate/circle_line_2/typical"))]
    #[bench::extreme(leak("isolate/circle_line_2/extreme"))]
    fn circle_line_2(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = isolate;
        benchmarks = cubic_1, circle_line_2
    );
}

/// The double-double balls (G53).
pub mod ball {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("ball/cubic/typical"))]
    #[bench::extreme(leak("ball/cubic/extreme"))]
    fn cubic(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = ball;
        benchmarks = cubic
    );
}

/// geom-math's hot paths (pieces G45, G46 and G48; decision 32.44).
pub mod math_types {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("math_types/apply_exact/typical"))]
    fn apply_exact(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("math_types/apply_rational/typical"))]
    fn apply_rational(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("math_types/apply_certified/typical"))]
    fn apply_certified(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("math_types/rotation_apply/typical"))]
    fn rotation_apply(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("math_types/plane_within/typical"))]
    fn plane_within(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("math_types/to_map/typical"))]
    fn to_map(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("math_types/decimal_parse/typical"))]
    fn decimal_parse(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = math_types;
        benchmarks = apply_exact, apply_rational, apply_certified, rotation_apply, plane_within, to_map, decimal_parse
    );
}

/// geom-linalg's hot paths (pieces G49 and G50; decision 32.44).
pub mod linalg {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("linalg/pivoted_qr/typical"))]
    fn pivoted_qr(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("linalg/svd_jacobi/typical"))]
    fn svd_jacobi(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = linalg;
        benchmarks = pivoted_qr, svd_jacobi
    );
}

/// geom-bvh's hot paths (pieces G54 and G55; decision 32.44).
pub mod bvh {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("bvh/tree_build_query/typical"))]
    fn tree_build_query(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("bvh/cells_within/typical"))]
    fn cells_within(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = bvh;
        benchmarks = tree_build_query, cells_within
    );
}

/// geom-nurbs's hot paths (pieces G56, G58 and G59; decision 32.44).
pub mod nurbs {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("nurbs/curve_point/typical"))]
    fn curve_point(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("nurbs/interpolate/typical"))]
    fn interpolate(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("nurbs/bernstein_bound/typical"))]
    fn bernstein_bound(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = nurbs;
        benchmarks = curve_point, interpolate, bernstein_bound
    );
}

/// geom-curve's hot paths (pieces G60 to G64 and G67; decision 32.44).
pub mod curve {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("curve/helix_point/typical"))]
    fn helix_point(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("curve/helix_enclose/typical"))]
    fn helix_enclose(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("curve/bulge_to_nurbs/typical"))]
    fn bulge_to_nurbs(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("curve/transition_point/typical"))]
    fn transition_point(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("curve/transition_to_nurbs/typical"))]
    fn transition_to_nurbs(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = curve;
        benchmarks = helix_point, helix_enclose, bulge_to_nurbs, transition_point, transition_to_nurbs
    );
}

/// geom-surface's hot paths (pieces G65 to G68; decision 32.44).
pub mod surface {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("surface/torus_point/typical"))]
    fn torus_point(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("surface/torus_enclose/typical"))]
    fn torus_enclose(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("surface/offset_point/typical"))]
    fn offset_point(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("surface/torus_second_bound/typical"))]
    fn torus_second_bound(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("surface/torus_normal_bound/typical"))]
    fn torus_normal_bound(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = surface;
        benchmarks = torus_point, torus_enclose, offset_point, torus_second_bound, torus_normal_bound
    );
}

/// Root finding and quadrature (piece G51; decision 32.44).
pub mod brent {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("brent/zero/typical"))]
    fn zero(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = brent;
        benchmarks = zero
    );
}

/// Root finding and quadrature (piece G51; decision 32.44).
pub mod quadrature {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("quadrature/gauss_legendre/typical"))]
    fn gauss_legendre(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = quadrature;
        benchmarks = gauss_legendre
    );
}

/// geom-intersect's hot paths (pieces G158, G162, G168 and G169; decision 32.44).
pub mod intersect {
    use super::*;

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("intersect/sort_items/typical"))]
    fn sort_items(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("intersect/key_circle/typical"))]
    fn key_circle(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("intersect/pair_lines/typical"))]
    fn pair_lines(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("intersect/project_arc/typical"))]
    fn project_arc(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("intersect/project_sphere/typical"))]
    fn project_sphere(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("intersect/trace_patch/typical"))]
    fn trace_patch(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("intersect/trace_silhouette/typical"))]
    fn trace_silhouette(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    #[library_benchmark(teardown = verify)]
    #[bench::typical(leak("intersect/subdivide_patch/typical"))]
    fn subdivide_patch(run: &'static mut Named) -> &'static mut Named {
        run.bench.measure();
        run
    }

    library_benchmark_group!(
        name = intersect;
        benchmarks = sort_items, key_circle, pair_lines, project_arc, project_sphere, trace_patch,
            subdivide_patch, trace_silhouette
    );
}
