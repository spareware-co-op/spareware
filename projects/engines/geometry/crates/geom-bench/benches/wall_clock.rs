//! Wall-clock timings of every `geom-num` operation, for people: the same kernels and inputs as
//! the count gates, timed with divan on any platform.
//!
//! Timings vary between runs and machines, so this harness never gates a merge; use it to see what
//! a change costs in nanoseconds. Each row is one benchmark of `geom_bench::catalogue`, named
//! `op/class` under its group (`op/l<limbs>` under `growth`), and reports the time of one pass
//! over its batch of `geom_bench::inputs::BATCH` inputs and the throughput in inputs per second.
//! Before timing, each benchmark's inputs and results are checked against `golden.txt`, so nothing
//! is timed that computes the wrong thing.
//!
//! ```text
//! cargo bench -p geom-bench --bench wall_clock
//! cargo bench -p geom-bench --bench wall_clock -- predicate   # names matching a pattern
//! ```
//!
//! Guarantees: every benchmark of the catalogue is timed (each group's arguments come from it),
//! and a benchmark whose inputs or results differ from `golden.txt`, or that the catalogue does
//! not know, ends the run with a failure before anything is timed.
//!
//! Method: divan's sampling and timing (<https://docs.rs/divan/0.1.21/divan/>), on the shared
//! kernels of `geom_bench::kernels`.
//!
//! Cost: per benchmark, one checked pass, then divan's default 100 samples, each of as many passes
//! as divan picks to make a sample long enough to time.

use std::process::exit;

use divan::Bencher;
use divan::counter::ItemsCount;
use geom_bench::catalogue::{Case, GrowthCase, Prepared, cases_in, growth_cases};
use geom_bench::golden;

fn main() {
    divan::main();
}

/// Checks a benchmark's golden digests, ending the run with a failure if they differ, then times
/// passes over its batch.
fn time(bencher: Bencher, name: &str, prepared: Option<Prepared>) {
    let Some(mut prepared) = prepared else {
        eprintln!("geom-bench: no benchmark named {name}");
        exit(2);
    };
    if let Err(message) = golden::check(name, &prepared.run()) {
        eprintln!("geom-bench: golden digests differ: {message}");
        exit(1);
    }
    bencher
        .counter(ItemsCount::new(prepared.len()))
        .bench_local(|| prepared.pass());
}

/// Times one benchmark of the catalogue.
fn time_case(bencher: Bencher, case: Case) {
    time(bencher, &case.name(), Some(case.prepare()));
}

/// The exact predicates.
#[divan::bench(args = cases_in("predicate"))]
fn predicate(bencher: Bencher, case: Case) {
    time_case(bencher, case);
}

/// Signs of exact integers.
#[divan::bench(args = cases_in("sign"))]
fn sign(bencher: Bencher, case: Case) {
    time_case(bencher, case);
}

/// Fixed-width integers at each width the kernel uses.
#[divan::bench(args = cases_in("wide"))]
fn wide(bencher: Bencher, case: Case) {
    time_case(bencher, case);
}

/// Planes and the points where three planes meet.
#[divan::bench(args = cases_in("plane"))]
fn plane(bencher: Bencher, case: Case) {
    time_case(bencher, case);
}

/// Error-free transformations.
#[divan::bench(args = cases_in("eft"))]
fn eft(bencher: Bencher, case: Case) {
    time_case(bencher, case);
}

/// Interval arithmetic.
#[divan::bench(args = cases_in("interval"))]
fn interval(bencher: Bencher, case: Case) {
    time_case(bencher, case);
}

/// Grid coordinates and grid steps.
#[divan::bench(args = cases_in("grid"))]
fn grid(bencher: Bencher, case: Case) {
    time_case(bencher, case);
}

/// Finite doubles in total order.
#[divan::bench(args = cases_in("real"))]
fn real(bencher: Bencher, case: Case) {
    time_case(bencher, case);
}

/// Exactness classes.
#[divan::bench(args = cases_in("exactness"))]
fn exactness(bencher: Bencher, case: Case) {
    time_case(bencher, case);
}

/// The deterministic maths functions.
#[divan::bench(args = cases_in("math"))]
fn math(bencher: Bencher, case: Case) {
    time_case(bencher, case);
}

/// The scale benchmarks at n, 2n and 4n limbs.
#[divan::bench(args = growth_cases())]
fn growth(bencher: Bencher, case: GrowthCase) {
    time(bencher, &case.name(), case.prepare());
}
