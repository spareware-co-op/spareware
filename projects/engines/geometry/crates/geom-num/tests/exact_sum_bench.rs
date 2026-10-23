//! The benchmark the board sets for `ExactSum` (piece G25): summing 10^6 doubles exactly, with the
//! large superaccumulator, takes at most twice as long as a plain ordered sum of the same doubles
//! natively. Neal (arXiv 1505.05571, section 6) reports under twice on modern 64-bit processors.
//!
//! Ignored by default, because timings mean nothing in a debug build or under a loaded machine.
//! Run it in release, one test at a time:
//!
//! ```text
//! cargo test -p geom-num --release --test exact_sum_bench -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Each case times the sums over the same 10^6 terms many times, interleaved, and compares the
//! fastest run of each, which is the least disturbed by the rest of the machine. The plain sum is
//! a left-to-right loop of `+=`, which the compiler cannot reorder (floating-point addition is not
//! associative), so it runs at the latency of one addition per term. The exact sum's time includes
//! making the accumulator, adding the terms and rounding the result. Two exact sums are held to
//! the bar: `add_slice` on the slice, and `add_iter` (by `collect`) on a stream of unknown length
//! (a filter that keeps every term, whose lower size bound is 0, so it takes the path that starts
//! one by one). Adding the terms one at a time with `add` is reported, not asserted: it uses the
//! small accumulator only, and the module documentation states its cost.
//!
//! The cases include the inputs hardest for the large accumulator, where consecutive terms hit
//! the same few chunks in an unpredictable order: subnormals and one binade with random signs,
//! four chunks in random order, and every exponent with both signs.
//!
//! Also reported, not asserted: `add_slice` against adding the same terms one at a time, just
//! below and from `LARGE_SLICE`, where `add_slice` switches to the large accumulator.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "a benchmark harness; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use geom_num::ExactSum;
use geom_num::exact_sum::LARGE_SLICE;
use std::hint::black_box;
use std::time::{Duration, Instant};

/// The number of terms the board names.
const TERMS: usize = 1_000_000;

/// Timed runs of each sum per case.
const RUNS: usize = 31;

/// `SplitMix64`: a small deterministic generator, so every run sums the same terms.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A double uniform in [0, 1), on the 2^−53 grid.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1_u64 << 53) as f64
    }
}

/// The plain ordered sum: `((x₀ + x₁) + x₂) + …`.
fn plain(terms: &[f64]) -> f64 {
    let mut s = 0.0;
    for &x in terms {
        s += x;
    }
    s
}

/// The exact sum of a slice, correctly rounded.
fn exact(terms: &[f64]) -> f64 {
    let mut sum = ExactSum::new();
    sum.add_slice(terms);
    sum.round_ieee()
}

/// The exact sum of a stream of unknown length, correctly rounded. The filter keeps every term
/// (so it costs nothing itself), but its lower size bound is 0.
fn exact_stream(terms: &[f64]) -> f64 {
    let sum: ExactSum = terms.iter().copied().filter(|_| true).collect();
    sum.round_ieee()
}

/// The exact sum, one term at a time.
fn exact_one_by_one(terms: &[f64]) -> f64 {
    let mut sum = ExactSum::new();
    for &x in terms {
        sum.add(x);
    }
    sum.round_ieee()
}

/// The fastest of `RUNS` timed runs of each sum, interleaved: plain, `add_slice`, `add_iter` and
/// `add`.
fn fastest(terms: &[f64]) -> [Duration; 4] {
    let sums: [fn(&[f64]) -> f64; 4] = [plain, exact, exact_stream, exact_one_by_one];
    let mut best = [Duration::MAX; 4];
    for _ in 0..RUNS {
        for (sum, best) in sums.iter().zip(&mut best) {
            let start = Instant::now();
            black_box(sum(black_box(terms)));
            *best = (*best).min(start.elapsed());
        }
    }
    best
}

/// A double with a random 52-bit fraction, the given exponent field and sign.
fn with_field(rng: &mut Rng, field: u64, negative: bool) -> f64 {
    f64::from_bits((u64::from(negative) << 63) | (field << 52) | (rng.next() >> 12))
}

/// The cases: named term generators.
fn cases() -> Vec<(&'static str, Vec<f64>)> {
    let mut rng = Rng(0x6732_3530_6732_3530);
    let uniform = (0..TERMS).map(|_| 2.0 * rng.unit() - 1.0).collect();
    // One sign and one exponent: every term hits the same large chunk, the worst case for its
    // read-modify-write chain.
    let one_binade = (0..TERMS).map(|_| 1.0 + rng.unit()).collect();
    // Exponents spread over ±60 binades, both signs.
    let spread = (0..TERMS)
        .map(|_| {
            let m = 1.0 + rng.unit();
            let e = (rng.next() % 121) as i32 - 60;
            let s = if rng.next() & 1 == 0 { 1.0 } else { -1.0 };
            s * m * 2.0f64.powi(e)
        })
        .collect();
    // Coordinates in metres on a 50 km site, as a survey sum would see them.
    let coordinates = (0..TERMS).map(|_| 50_000.0 * rng.unit()).collect();
    // The hardest for the large accumulator: two or four chunks in an unpredictable order.
    let subnormals = (0..TERMS)
        .map(|_| {
            let negative = rng.next() & 1 == 1;
            with_field(&mut rng, 0, negative)
        })
        .collect();
    let binade_signs = (0..TERMS)
        .map(|_| {
            let negative = rng.next() & 1 == 1;
            with_field(&mut rng, 1023, negative)
        })
        .collect();
    let four_chunks = (0..TERMS)
        .map(|_| {
            let pick = rng.next() % 4;
            with_field(&mut rng, 1023 + (pick & 1), pick >= 2)
        })
        .collect();
    let every_exponent = (0..TERMS)
        .map(|_| {
            let pick = rng.next();
            with_field(&mut rng, (pick >> 1) % 2047, pick & 1 == 1)
        })
        .collect();
    vec![
        ("uniform in [-1, 1)", uniform),
        ("one binade [1, 2)", one_binade),
        ("spread over 2^-60..2^60, both signs", spread),
        ("coordinates in [0, 50 km)", coordinates),
        ("subnormals, random signs", subnormals),
        ("[1, 2), random signs", binade_signs),
        ("four chunks, random order", four_chunks),
        ("every exponent, both signs", every_exponent),
    ]
}

#[test]
#[ignore = "a timing benchmark: run in release with --ignored"]
#[allow(
    clippy::assertions_on_constants,
    reason = "the build profile is what is checked: a debug build would time the wrong code"
)]
fn exact_sum_of_a_million_terms_within_twice_a_plain_sum() {
    assert!(
        !cfg!(debug_assertions),
        "run the benchmark in release (cargo test --release)"
    );
    let mut worst: f64 = 0.0;
    let mut worst_one: f64 = 0.0;
    for (name, terms) in cases() {
        // Warm every sum up, and check that they agree.
        let p = black_box(plain(&terms));
        let e = exact(&terms);
        assert_eq!(e.to_bits(), exact_stream(&terms).to_bits(), "{name}");
        assert_eq!(e.to_bits(), exact_one_by_one(&terms).to_bits(), "{name}");
        black_box((p, e));
        let [p, slice, stream, one] = fastest(&terms);
        let ratio = |d: Duration| d.as_secs_f64() / p.as_secs_f64();
        println!(
            "{name:36} plain {:6.3} ms  add_slice {:5.2}x  add_iter {:5.2}x  (add {:5.2}x)",
            p.as_secs_f64() * 1e3,
            ratio(slice),
            ratio(stream),
            ratio(one)
        );
        worst = worst.max(ratio(slice)).max(ratio(stream));
        worst_one = worst_one.max(ratio(one));
    }
    println!(
        "worst ratio {worst:.2} (the board's limit is 2); one by one, not asserted: {worst_one:.2}"
    );
    assert!(
        worst <= 2.0,
        "exact summation took {worst:.2} times a plain sum"
    );
}

#[test]
#[ignore = "a timing report: run in release with --ignored"]
fn add_slice_against_add_around_the_switch() {
    let mut rng = Rng(7);
    let s = LARGE_SLICE;
    for n in [s / 2, s - 1, s, 2 * s, 8 * s, 32 * s] {
        let terms: Vec<f64> = (0..n).map(|_| 2.0 * rng.unit() - 1.0).collect();
        let time = |large: bool| {
            let mut best = Duration::MAX;
            for _ in 0..2001 {
                let start = Instant::now();
                let mut sum = ExactSum::new();
                if large {
                    sum.add_slice(black_box(&terms));
                } else {
                    for &x in black_box(&terms) {
                        sum.add(x);
                    }
                }
                black_box(sum.round_ieee());
                best = best.min(start.elapsed());
            }
            best
        };
        let (one_by_one, slice) = (time(false), time(true));
        println!(
            "{n:6} terms: one by one {:9.2} us, add_slice {:9.2} us",
            one_by_one.as_secs_f64() * 1e6,
            slice.as_secs_f64() * 1e6
        );
    }
}
