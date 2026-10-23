//! Benchmark of the proven maths functions against `libm` (principle 12; piece G47's target and
//! risk R13's early warning: within 2 times of `libm`).
//!
//! Run it in a release build, on a quiet machine:
//!
//! ```text
//! cargo test -p geom-num --release --test math_bench -- --ignored --nocapture
//! ```
//!
//! For each function it times a fixed, generated set of arguments (the same for the kernel's
//! function and for `libm`'s counterpart: `sinpi(x)` against `libm::sin(π·x)` on the same angles,
//! `cospi` against `libm::cos`, and `exp`, `ln` and `atan2` against `libm::exp`, `libm::log` and
//! `libm::atan2`), takes the best of several runs, prints nanoseconds per call and the ratio, and
//! fails if a ratio exceeds 2. The arguments cover each function's ordinary range, where nearly
//! every call returns from the fast path; the slow paths run about once in a thousand calls or
//! less, and the correctness tests cover them.
//!
//! It is `#[ignore]`d because timings depend on the machine and its load, so it never runs as part
//! of the ordinary test suite. The interval versions cost two point evaluations each plus exact
//! comparisons, and are timed here too, for the record (no target).
//!
//! The kernel's functions are timed as the kernel calls them: each checks its domain and returns a
//! `Real` or an error, and the benchmark reads the value (`libm` checks nothing and returns a raw
//! double), so the ratio includes the price of the contract.

// Test code may use native arithmetic, casts and std's maxima and minima (Cargo.toml).
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_num::interval::Interval;
use geom_num::{NumError, Real, math};

/// The value of a kernel function's result, as the benchmark adds it up (0 for an error, which
/// none of the timed arguments gives).
fn value(r: Result<Real, NumError>) -> f64 {
    r.map_or(0.0, Real::get)
}

/// `SplitMix64`: a deterministic generator for the arguments.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in `[lo, hi)`.
    fn uniform(&mut self, lo: f64, hi: f64) -> f64 {
        let unit = (self.next() >> 11) as f64 * (1.0 / (1u64 << 53) as f64);
        lo + (hi - lo) * unit
    }
}

/// How many arguments each function is timed on, and how many timed runs are made.
const N: usize = 1 << 20;
const RUNS: usize = 9;

/// The best time per call, in nanoseconds, of `f` over `args`.
fn per_call(args: &[(f64, f64)], f: impl Fn(f64, f64) -> f64) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..RUNS {
        let start = Instant::now();
        let mut acc = 0.0;
        for &(a, b) in args {
            acc += f(black_box(a), black_box(b));
        }
        black_box(acc);
        let ns = start.elapsed().as_nanos() as f64 / args.len() as f64;
        best = best.min(ns);
    }
    best
}

/// One line of the report: the kernel's time, `libm`'s, and their ratio.
struct Line {
    name: &'static str,
    kernel: f64,
    libm: f64,
}

#[test]
#[ignore = "benchmark: run in release with --ignored --nocapture"]
fn proven_functions_are_within_twice_libm() {
    let mut rng = Rng(0x00be_0c4a_2026);
    let pi = core::f64::consts::PI;

    // Angles in half-turns over four full turns, and the same angles in radians for libm.
    let half_turns: Vec<(f64, f64)> = (0..N).map(|_| (rng.uniform(-4.0, 4.0), 0.0)).collect();
    let radians: Vec<(f64, f64)> = half_turns.iter().map(|&(x, _)| (pi * x, 0.0)).collect();
    // exp over its whole finite, non-overflowing range.
    let exp_args: Vec<(f64, f64)> = (0..N).map(|_| (rng.uniform(-700.0, 700.0), 0.0)).collect();
    // ln over 2^−60 to 2^60, uniform in the exponent.
    let ln_args: Vec<(f64, f64)> = (0..N)
        .map(|_| (libm::exp2(rng.uniform(-60.0, 60.0)), 0.0))
        .collect();
    // atan2 over a square around the origin, every quadrant.
    let atan2_args: Vec<(f64, f64)> = (0..N)
        .map(|_| (rng.uniform(-1000.0, 1000.0), rng.uniform(-1000.0, 1000.0)))
        .collect();

    let lines = [
        Line {
            name: "sinpi",
            kernel: per_call(&half_turns, |x, _| value(math::sinpi(x))),
            libm: per_call(&radians, |x, _| libm::sin(x)),
        },
        Line {
            name: "cospi",
            kernel: per_call(&half_turns, |x, _| value(math::cospi(x))),
            libm: per_call(&radians, |x, _| libm::cos(x)),
        },
        Line {
            name: "exp",
            kernel: per_call(&exp_args, |x, _| value(math::exp(x))),
            libm: per_call(&exp_args, |x, _| libm::exp(x)),
        },
        Line {
            name: "ln",
            kernel: per_call(&ln_args, |x, _| value(math::ln(x))),
            libm: per_call(&ln_args, |x, _| libm::log(x)),
        },
        Line {
            name: "atan2",
            kernel: per_call(&atan2_args, |y, x| value(math::atan2(y, x))),
            libm: per_call(&atan2_args, libm::atan2),
        },
    ];

    // The interval versions, for the record: intervals one unit wide at the same places.
    let intervals: Vec<(f64, f64)> = half_turns.iter().map(|&(x, _)| (x, x + 0.25)).collect();
    let interval_sinpi = per_call(&intervals, |lo, hi| match Interval::new(lo, hi) {
        Ok(i) => i.sinpi().map_or(0.0, Interval::hi),
        Err(_) => 0.0,
    });
    let interval_exp = per_call(&exp_args, |x, _| match Interval::new(x, x + 1.0) {
        Ok(i) => i.exp().map_or(0.0, Interval::hi),
        Err(_) => 0.0,
    });

    eprintln!("function   kernel ns   libm ns   ratio");
    let mut over = Vec::new();
    for l in &lines {
        let ratio = l.kernel / l.libm;
        eprintln!(
            "{:<8} {:>11.1} {:>9.1} {:>7.2}",
            l.name, l.kernel, l.libm, ratio
        );
        if ratio > 2.0 {
            over.push(format!("{} is {ratio:.2} times libm", l.name));
        }
    }
    eprintln!("Interval::sinpi {interval_sinpi:.1} ns, Interval::exp {interval_exp:.1} ns");
    assert!(over.is_empty(), "over the 2x target: {over:?}");
}
