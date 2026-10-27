//! The regression comparison of `libm` with correctly rounded references (piece G31): how often
//! each unproven function `geom_num::math::unproven` takes from `libm` differs from the correctly
//! rounded result, and by how many units in the last place at most, on 1,000,000 seeded inputs
//! per function.
//!
//! **A regression test, never a bound** (the board's G31; chapter 6.4). The counts below are what
//! `libm` 0.2.16 gives on these inputs, recorded once. They promise nothing about other inputs, and
//! no code may use them as an error bound: a function with no proven bound stays in
//! `math::unproven`, and an enclosure that needs it refuses with `UnprovenMaths`. The recorded
//! counts show why. `libm`'s `exp`, `ln`, `atan`, `atan2`, `sin` and `cos` miss by one unit at
//! most here, while the unproven `sinpi` and `cospi`, which compute `sin(π·r)` with π rounded,
//! miss near their zeros by hundreds of thousands and tens of millions of units. What the test
//! catches is change: a new `libm` release, a different code path on some target, or a change to a
//! reference shows here as a count that moved, and the pull request that moved it says why. Since
//! `libm`'s results are the same bits on every target (`projects/kit/notes/G24-libm-audit.md`,
//! section 7), the counts are the same on every leg of the matrix too.
//!
//! The references, each correctly rounded:
//!
//! - `exp`, `ln`, `atan2`, `sinpi` and `cospi`: `geom-num`'s CORE-MATH ports (`geom_num::math`,
//!   piece G05), proven correctly rounded; `atan(x)` is `atan2(x, 1)`, which is exactly atan x.
//!   Each runs on 1,000,000 inputs in every release build.
//! - `sin` and `cos` in radians: the rigorous interval oracle in `tests/math_reference.rs`, which
//!   brackets the true value in big-integer interval arithmetic until both ends round alike. It is
//!   slow, so every release build runs the first 20,000 inputs, and the full 1,000,000 run is the
//!   ignored test `libm_sin_and_cos_on_a_million_inputs` (`cargo test --release -p geom-num --test
//!   libm_regression -- --ignored`), whose counts are recorded the same way.
//!
//! `tan`, `asin`, `acos`, `log2`, `log10`, `pow`, `hypot` and `cbrt` have no correctly rounded
//! reference in the repository, so they are not compared here.
//!
//! The inputs come from `geom_test`'s `SplitMix64` from a fixed seed per function (`SPAREKIT_SEED`
//! would change the counts, so this test ignores it and prints the seed it uses). A debug build
//! runs the first twentieth of each function's inputs, whose counts are recorded too, so both
//! build modes check exact values; a release build checks both.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them, and std's reductions, to build their oracles.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "oracles and generators; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

#[path = "math_reference.rs"]
mod reference;

use std::num::NonZeroU64;

use geom_num::{NumError, Real, math};
use geom_test::SplitMix64;
use geom_test::seed::{Seed, fixed};

/// Inputs per function in the full run.
const INPUTS: usize = 1_000_000;

/// The first inputs of `sin` and `cos` every release build runs, the oracle being slow.
const ORACLE_INPUTS: usize = 20_000;

/// The prefix a debug build runs: a twentieth.
const fn prefix(n: usize) -> usize {
    n / 20
}

/// The seed every function's inputs come from, plus the function's place in [`FUNCTIONS`].
const SEED: u64 = 0x6731_0000_5eed_0254;

/// What a comparison found: how many inputs `libm` misrounded, and the largest distance in units
/// in the last place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Found {
    misrounded: u64,
    max_ulps: u64,
}

/// A recorded count.
const fn found(misrounded: u64, max_ulps: u64) -> Found {
    Found {
        misrounded,
        max_ulps,
    }
}

/// The functions compared, in seed order.
const FUNCTIONS: [&str; 8] = ["exp", "ln", "atan", "atan2", "sinpi", "cospi", "sin", "cos"];

/// The recorded counts of the regular test, per function: on the debug prefix, then on the run a
/// release build makes (1,000,000 inputs; 20,000 for `sin` and `cos`). `libm` 0.2.16.
const RECORDED: [(&str, Found, Found); 8] = [
    ("exp", found(4811, 1), found(95531, 1)),
    ("ln", found(54, 1), found(1054, 1)),
    ("atan", found(92, 1), found(1854, 1)),
    ("atan2", found(3479, 1), found(70691, 1)),
    ("sinpi", found(14575, 2956), found(293_743, 559_254)),
    ("cospi", found(7302, 12044), found(144_322, 66_289_133)),
    ("sin", found(12, 1), found(359, 1)),
    ("cos", found(11, 1), found(334, 1)),
];

/// The recorded counts of `sin` and `cos` on the full 1,000,000 inputs (the ignored test).
const RECORDED_MILLION: [(&str, Found); 2] = [("sin", found(17451, 1)), ("cos", found(16993, 1))];

/// The value of a result the maths module returns, or `None` where it refuses.
fn value(r: Result<Real, NumError>) -> Option<f64> {
    r.ok().map(Real::get)
}

/// The distance between two finite doubles in units in the last place: how many steps of one
/// double lie between them, −0 and +0 counting as one point.
fn ulps(a: f64, b: f64) -> u64 {
    // Map each double to an integer that orders as the doubles do (sign-magnitude to two's
    // complement), so that adjacent doubles differ by one and both zeros map to 0.
    let key = |x: f64| -> i64 {
        let bits = x.to_bits() as i64;
        if bits < 0 { i64::MIN - bits } else { bits }
    };
    key(a).abs_diff(key(b))
}

/// A double of a random binade in `lo..=hi` (biased exponents), with a random significand and,
/// when `signed`, a random sign.
fn binade(rng: &mut SplitMix64, lo: u64, hi: u64, signed: bool) -> f64 {
    let e = lo + rng.below(NonZeroU64::new(hi - lo + 1).unwrap());
    let bits = rng.next_u64();
    let sign = if signed { bits & (1 << 63) } else { 0 };
    f64::from_bits(sign | (e << 52) | (bits & ((1 << 52) - 1)))
}

/// Uniform in `lo..hi`, from 53 random bits.
fn uniform(rng: &mut SplitMix64, lo: f64, hi: f64) -> f64 {
    lo + (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64 * (hi - lo)
}

/// One function: its inputs, the unproven function, and the correctly rounded reference.
struct Function {
    name: &'static str,
    input: fn(&mut SplitMix64) -> (f64, f64),
    unproven: fn(f64, f64) -> Option<f64>,
    reference: fn(f64, f64) -> Option<f64>,
}

/// Every function compared.
fn functions() -> [Function; 8] {
    [
        Function {
            name: "exp",
            input: |r| (uniform(r, -745.0, 709.7), 0.0),
            unproven: |x, _| value(math::unproven::exp(x)),
            reference: |x, _| value(math::exp(x)),
        },
        Function {
            name: "ln",
            input: |r| (binade(r, 1, 2046, false), 0.0),
            unproven: |x, _| value(math::unproven::ln(x)),
            reference: |x, _| value(math::ln(x)),
        },
        Function {
            name: "atan",
            input: |r| (binade(r, 900, 1150, true), 0.0),
            unproven: |x, _| value(math::unproven::atan(x)),
            reference: |x, _| value(math::atan2(x, 1.0)),
        },
        Function {
            name: "atan2",
            input: |r| (binade(r, 900, 1150, true), binade(r, 900, 1150, true)),
            unproven: |y, x| value(math::unproven::atan2(y, x)),
            reference: |y, x| value(math::atan2(y, x)),
        },
        Function {
            name: "sinpi",
            input: |r| (binade(r, 900, 1080, true), 0.0),
            unproven: |x, _| value(math::unproven::sinpi(x)),
            reference: |x, _| value(math::sinpi(x)),
        },
        Function {
            name: "cospi",
            input: |r| (binade(r, 900, 1080, true), 0.0),
            unproven: |x, _| value(math::unproven::cospi(x)),
            reference: |x, _| value(math::cospi(x)),
        },
        Function {
            name: "sin",
            input: |r| (binade(r, 970, 1080, true), 0.0),
            unproven: |x, _| value(math::unproven::sin(x)),
            reference: |x, _| Some(reference::sin(x)),
        },
        Function {
            name: "cos",
            input: |r| (binade(r, 970, 1080, true), 0.0),
            unproven: |x, _| value(math::unproven::cos(x)),
            reference: |x, _| Some(reference::cos(x)),
        },
    ]
}

/// Compares `libm` with the reference on the function's inputs, and returns what was found after
/// each count in `checkpoints` (ascending). Inputs where either refuses are skipped and not
/// counted: the maths module refuses NaN-making and overflowing cases, the same for both.
fn compare(f: &Function, checkpoints: &[usize]) -> Vec<Found> {
    let index = FUNCTIONS.iter().position(|n| *n == f.name).unwrap() as u64;
    let mut rng = SplitMix64::new(fixed(
        &format!("libm regression, {}", f.name),
        Seed(SEED + index),
    ));
    let mut found = Found {
        misrounded: 0,
        max_ulps: 0,
    };
    let mut out = Vec::new();
    let mut compared = 0_usize;
    for &stop in checkpoints {
        while compared < stop {
            let (x, y) = (f.input)(&mut rng);
            let (Some(got), Some(want)) = ((f.unproven)(x, y), (f.reference)(x, y)) else {
                continue;
            };
            compared += 1;
            if got.to_bits() != want.to_bits() {
                found.misrounded += 1;
                found.max_ulps = found.max_ulps.max(ulps(got, want));
            }
        }
        out.push(found);
    }
    out
}

/// The counts are the recorded ones: the debug prefix in every build, and the release run in a
/// release build.
#[test]
fn libm_misrounds_exactly_as_recorded() {
    let release = !cfg!(debug_assertions);
    for (f, (name, debug, full)) in functions().iter().zip(RECORDED) {
        assert_eq!(f.name, name);
        let n = if matches!(name, "sin" | "cos") {
            ORACLE_INPUTS
        } else {
            INPUTS
        };
        let checkpoints: Vec<usize> = if release {
            vec![prefix(n), n]
        } else {
            vec![prefix(n)]
        };
        let got = compare(f, &checkpoints);
        println!("{name}: {got:?}");
        assert_eq!(
            got[0],
            debug,
            "{name}, first {} inputs: libm's misroundings moved; say why in the pull request, \
             then record the new counts",
            prefix(n)
        );
        if release {
            assert_eq!(
                got[1], full,
                "{name}, {n} inputs: libm's misroundings moved; say why in the pull request, then \
                 record the new counts"
            );
        }
    }
    // The comparison has teeth: libm is not correctly rounded, and the references tell.
    assert!(
        RECORDED.iter().all(|(_, _, full)| full.misrounded > 0),
        "a reference never disagrees with libm"
    );
}

/// `sin` and `cos` on the full 1,000,000 inputs, against the slow interval oracle. Its first
/// 20,000 inputs are the regular test's.
#[test]
#[ignore = "the interval oracle takes minutes for a million inputs: run with --release -- --ignored"]
fn libm_sin_and_cos_on_a_million_inputs() {
    for (name, want) in RECORDED_MILLION {
        let f = functions().into_iter().find(|f| f.name == name).unwrap();
        let regular = RECORDED.iter().find(|r| r.0 == name).unwrap().2;
        let got = compare(&f, &[ORACLE_INPUTS, INPUTS]);
        println!("{name}: {got:?}");
        assert_eq!(
            got[0], regular,
            "{name}: the first {ORACLE_INPUTS} inputs are the regular test's"
        );
        assert_eq!(
            got[1], want,
            "{name}, {INPUTS} inputs: libm's misroundings moved"
        );
    }
}

#[test]
fn the_ulp_distance_counts_doubles_between() {
    assert_eq!(ulps(1.0, 1.0), 0);
    assert_eq!(ulps(1.0, 1.0f64.next_up()), 1);
    assert_eq!(ulps(-0.0, 0.0), 0);
    assert_eq!(ulps(f64::from_bits(1), -f64::from_bits(1)), 2);
    assert_eq!(ulps(f64::MAX, f64::MAX.next_down()), 1);
    assert_eq!(ulps(-1.0, 1.0), 2 * 1.0f64.to_bits());
}
