//! Piece G47's oracle check: the proven maths functions against the multiprecision reference on
//! ten million seeded inputs each, and never a result off by more than the bound.
//!
//! `sinpi`, `cospi`, `exp`, `ln` and `atan2` are correctly rounded (the module documentation of
//! `geom_num::math`): an error of at most half an ulp, the bound the board asks to be "never
//! exceeded against a multiprecision oracle on 10M inputs". Each input here is checked against
//! `tests/math_reference.rs`, which brackets the exact value in big-integer interval arithmetic
//! until both ends round alike, so the check is bit for bit: a result one ulp off, or a refusal
//! the contract does not owe, fails. The result owed comes from `reference::owed`, with a zero
//! `sinpi` or `cospi` as +0 (the crate's one-zero rule).
//!
//! The inputs come from `geom_test`'s `SplitMix64`, one generator per block of
//! [`BLOCK`] inputs, from the seed `geom_test::seed::resolve` prints (`SPAREKIT_SEED` replaces
//! it, so the nightly run draws ten million new inputs per function each night). Each function
//! mixes every binade of its domain with the ranges where its code paths and hard cases are: near
//! the zeros and peaks of `sinpi` and `cospi`, the tiny, the underflowing and the overflowing
//! arguments of `exp`, arguments next to 1 for `ln`, and pairs near the axes and the diagonals,
//! of very different magnitudes and subnormal, for `atan2`. Arguments outside a function's domain
//! and past its overflow are drawn too, and must be refused as owed.
//!
//! Every build checks the first [`PREFIX`] inputs of each function. The full run, ten million per
//! function, is the ignored test `ten_million_inputs_per_function`, which the nightly workflow
//! runs in a release build with CORE-MATH's worst-case lists (`math_full_worst_cases.rs`):
//!
//! ```text
//! cargo test -p geom-num --release --test math_oracle -- --ignored --nocapture
//! ```
//!
//! It shares the blocks among the machine's threads; which thread checks a block changes nothing
//! but the time, and every mismatch is reported with its arguments' bits, in input order.

// Test code may use native arithmetic and casts (Cargo.toml); helpers outside `#[test]` functions
// may panic, as clippy.toml allows inside them.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests may use native arithmetic, casts and std reductions, and stop loudly"
)]

#[path = "math_reference.rs"]
mod reference;

use std::num::NonZeroU64;

use geom_num::{NumError, Real, math};
use geom_test::SplitMix64;
use geom_test::seed::{Seed, resolve};
use reference::{Owed, owed};

/// Inputs per function in the full run: the board's ten million.
const INPUTS: usize = 10_000_000;

/// The inputs per function every build checks.
const PREFIX: usize = 400;

/// Inputs per generator: the unit the threads share.
const BLOCK: usize = 10_000;

/// The default seed. `SPAREKIT_SEED` replaces it.
const SEED: u64 = 0x6747_0000_0000_5eed;

/// The outcome of a call of a public maths function, to compare with what it owes.
fn outcome(r: Result<Real, NumError>) -> Owed {
    match r {
        Ok(v) => Owed::Value(v.get()),
        Err(NumError::NotFinite) => Owed::NotFinite,
        Err(NumError::OutsideDomain) => Owed::OutsideDomain,
        Err(e) => panic!("a maths function returned {e:?}"),
    }
}

/// Whether a function's outcome is what it owes: the same error, or the same value bit for bit.
fn agrees(got: Owed, want: Owed) -> bool {
    match (got, want) {
        (Owed::Value(g), Owed::Value(w)) => g.to_bits() == w.to_bits(),
        (Owed::NotFinite, Owed::NotFinite) | (Owed::OutsideDomain, Owed::OutsideDomain) => true,
        _ => false,
    }
}

/// `2^k` for `−1074 ≤ k ≤ 1023`, exactly.
fn pow2(k: i64) -> f64 {
    if k >= -1022 {
        f64::from_bits(((k + 1023) as u64) << 52)
    } else {
        f64::from_bits(1u64 << (k + 1074))
    }
}

/// A value in `0..n`.
fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    rng.below(NonZeroU64::new(n).unwrap())
}

/// A double with a random significand in a random binade of `lo..=hi` (biased exponents, 0 for
/// the subnormals), and a random sign when `signed`.
fn binade(rng: &mut SplitMix64, lo: u64, hi: u64, signed: bool) -> f64 {
    let e = lo + below(rng, hi - lo + 1);
    let bits = rng.next_u64();
    let sign = if signed { bits & (1 << 63) } else { 0 };
    f64::from_bits(sign | (e << 52) | (bits & ((1 << 52) - 1)))
}

/// Uniform in `lo..hi`, from 53 random bits.
fn uniform(rng: &mut SplitMix64, lo: f64, hi: f64) -> f64 {
    lo + (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64 * (hi - lo)
}

/// A random sign.
fn signed(x: f64, rng: &mut SplitMix64) -> f64 {
    if rng.next_u64() & 1 == 0 { x } else { -x }
}

/// An argument of `sinpi` or `cospi`, in half-turns.
fn half_turns(rng: &mut SplitMix64) -> f64 {
    match below(rng, 5) {
        // Every finite double: from the subnormals to the even integers above 2^53.
        0 => binade(rng, 0, 2046, true),
        // Every binade the reduction and the polynomials see, below 2^52.
        1 => binade(rng, 900, 1074, true),
        // An ordinary angle, within a turn either way.
        2 => uniform(rng, -2.0, 2.0),
        // Next to a multiple of a quarter turn: next to a zero, a peak or ±1/√2, where the
        // relative accuracy is hardest, at a random distance down to a few ulps.
        3 => {
            let k = rng.next_u64() % (1 << 20);
            let base = k as f64 * 0.25;
            let near = signed(binade(rng, 960, 1020, false), rng);
            signed(base + near, rng)
        }
        // Large angles with a fraction: the exact reduction of big arguments.
        _ => binade(rng, 1023 + 20, 1023 + 52, true),
    }
}

/// An argument of `exp`.
fn exponent(rng: &mut SplitMix64) -> f64 {
    match below(rng, 4) {
        // The whole range with a finite, nonzero result, and a little past both ends.
        0 => uniform(rng, -746.0, 710.0),
        // Tiny arguments, down to the subnormals: e^x next to 1.
        1 => binade(rng, 0, 1022, true),
        // Next to the ends: overflow from 0x1.62e42fefa39fp+9, underflow and subnormal results.
        2 => {
            if rng.next_u64() & 1 == 0 {
                uniform(rng, 709.0, 709.8)
            } else {
                uniform(rng, -745.2, -708.0)
            }
        }
        // Every finite double: most overflow (refused) or underflow to zero.
        _ => binade(rng, 0, 2046, true),
    }
}

/// An argument of `ln`.
fn logarithm(rng: &mut SplitMix64) -> f64 {
    match below(rng, 4) {
        // Every positive finite double, subnormals included.
        0 => binade(rng, 0, 2046, false),
        // Next to 1, where the result is tiny and the accurate path works hardest.
        1 => 1.0 + signed(binade(rng, 960, 1020, false), rng),
        // An ordinary positive value.
        2 => uniform(rng, 0.0, 16.0),
        // Rarely, a negative argument or zero, which the function refuses.
        _ => {
            if rng.next_u64() % 16 == 0 {
                -binade(rng, 0, 2046, false) * (rng.next_u64() & 1) as f64
            } else {
                binade(rng, 1, 2046, false)
            }
        }
    }
}

/// A pair `(y, x)` of arguments of `atan2`.
fn point(rng: &mut SplitMix64) -> (f64, f64) {
    match below(rng, 6) {
        // Any two finite doubles.
        0 => (binade(rng, 0, 2046, true), binade(rng, 0, 2046, true)),
        // Comparable magnitudes: the fast path and every table index.
        1 => (uniform(rng, -1.0, 1.0), uniform(rng, -1.0, 1.0)),
        // Exponents up to 60 apart: near the axes, and the tiny-quotient path.
        2 => {
            let e = below(rng, 1800) as i64 - 900;
            let d = below(rng, 120) as i64 - 60;
            let y = signed(pow2(e) * uniform(rng, 1.0, 2.0), rng);
            let x = signed(pow2(e + d) * uniform(rng, 1.0, 2.0), rng);
            (y, x)
        }
        // Next to the diagonals |y| = |x|.
        3 => {
            let x = signed(binade(rng, 973, 1073, false), rng);
            let y = x * (1.0 + uniform(rng, -1e-6, 1e-6));
            (signed(y, rng), x)
        }
        // Subnormal and huge arguments: the scaled extremes.
        4 => (binade(rng, 0, 101, true), binade(rng, 0, 101, true)),
        _ => (binade(rng, 2038, 2046, true), binade(rng, 2038, 2046, true)),
    }
}

/// One function: how its inputs are drawn, the function and the correctly rounded reference.
struct Function {
    name: &'static str,
    input: fn(&mut SplitMix64) -> (f64, f64),
    ours: fn(f64, f64) -> Result<Real, NumError>,
    reference: fn(f64, f64) -> f64,
    arity: usize,
}

/// The five proven functions, in seed order.
const FUNCTIONS: [Function; 5] = [
    Function {
        name: "sinpi",
        input: |r| (half_turns(r), 0.0),
        ours: |x, _| math::sinpi(x),
        reference: |x, _| reference::sinpi(x) + 0.0, // a zero as +0
        arity: 1,
    },
    Function {
        name: "cospi",
        input: |r| (half_turns(r), 0.0),
        ours: |x, _| math::cospi(x),
        reference: |x, _| reference::cospi(x) + 0.0, // a zero as +0
        arity: 1,
    },
    Function {
        name: "exp",
        input: |r| (exponent(r), 0.0),
        ours: |x, _| math::exp(x),
        reference: |x, _| reference::exp(x),
        arity: 1,
    },
    Function {
        name: "ln",
        input: |r| (logarithm(r), 0.0),
        ours: |x, _| math::ln(x),
        reference: |x, _| reference::ln(x),
        arity: 1,
    },
    Function {
        name: "atan2",
        input: point,
        ours: math::atan2,
        reference: reference::atan2,
        arity: 2,
    },
];

/// The generator of block `block` of function `index`. `SplitMix64`'s first word is a one-to-one
/// mix of its state, so each block of each function starts its own stream at a point of its own,
/// and a block's inputs do not depend on which thread checks it or in what order.
fn generator(seed: Seed, index: usize, block: usize) -> SplitMix64 {
    let key = ((index as u64) << 32) | block as u64;
    let mut start = SplitMix64::new(Seed(
        seed.0.wrapping_add(key.wrapping_mul(0x9e37_79b9_7f4a_7c15)),
    ));
    SplitMix64::new(Seed(start.next_u64()))
}

/// The arguments' bits and the outcomes, for a report.
fn describe(f: &Function, x: f64, y: f64, got: Owed, want: Owed) -> String {
    let args = if f.arity == 1 {
        format!("{:#018x}", x.to_bits())
    } else {
        format!("{:#018x}, {:#018x}", x.to_bits(), y.to_bits())
    };
    format!("{}({args}): got {got:?}, owed {want:?}", f.name)
}

/// The mismatches among the inputs of the given blocks of function `index`, below `n`.
fn check_blocks(
    seed: Seed,
    index: usize,
    n: usize,
    blocks: impl Iterator<Item = usize>,
) -> Vec<(usize, String)> {
    let f = &FUNCTIONS[index];
    let mut found = Vec::new();
    for block in blocks {
        let mut rng = generator(seed, index, block);
        let start = block * BLOCK;
        for i in start..(start + BLOCK).min(n) {
            let (x, y) = (f.input)(&mut rng);
            let args = [x, y];
            let got = outcome((f.ours)(x, y));
            let want = owed(&args[..f.arity], (f.reference)(x, y));
            if !agrees(got, want) {
                found.push((i, describe(f, x, y, got, want)));
            }
        }
    }
    found
}

/// Checks the first `n` inputs of function `index` on `threads` threads, thread `t` taking the
/// blocks `t`, `t + threads`, `t + 2·threads` and so on, and returns every mismatch, in input
/// order.
fn check(seed: Seed, index: usize, n: usize, threads: usize) -> Vec<(usize, String)> {
    let blocks = n.div_ceil(BLOCK);
    let mut found = if threads <= 1 {
        check_blocks(seed, index, n, 0..blocks)
    } else {
        std::thread::scope(|s| {
            let handles: Vec<_> = (0..threads)
                .map(|t| {
                    s.spawn(move || check_blocks(seed, index, n, (t..blocks).step_by(threads)))
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap())
                .collect::<Vec<_>>()
        })
    };
    found.sort();
    found
}

/// Runs the check of the first `n` inputs of every function and fails, listing the first
/// mismatches, if any result is not the correctly rounded value or the refusal owed.
fn run(label: &str, n: usize, threads: usize) {
    let seed = resolve(label, Seed(SEED)).unwrap();
    let mut report = Vec::new();
    for (index, f) in FUNCTIONS.iter().enumerate() {
        let found = check(seed, index, n, threads);
        println!(
            "{}: {n} inputs, {} not correctly rounded",
            f.name,
            found.len()
        );
        report.extend(found.into_iter().take(10).map(|(_, line)| line));
    }
    assert!(report.is_empty(), "{}", report.join("\n"));
}

/// The first inputs of every function, in every build.
#[test]
fn a_prefix_of_every_function_is_correctly_rounded() {
    run("math oracle, prefix", PREFIX, 1);
}

/// Ten million inputs per function, on every thread the machine has (the nightly workflow).
#[test]
#[ignore = "ten million inputs per function: minutes on many threads; the nightly runs it"]
fn ten_million_inputs_per_function() {
    let threads = std::thread::available_parallelism().map_or(1, usize::from);
    run("math oracle, ten million", INPUTS, threads);
}
