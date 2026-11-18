//! One long test's cases cut into slices, each its own test, so that each ends within a Miri gate
//! part (decision 32.78: a row of at most 486 yardstick seconds). A test binary takes it with
//! `#[macro_use] mod slices;`.
//!
//! What it does: [`range`] cuts a list of `all` cases into [`SLICES`] ranges that join to the
//! whole list, each once; [`assert_joins`] checks that they do, cheaply, for the binary's own
//! `the_slices_join_to_every_case_once` test. [`run_slice`] runs one slice of a property, by the
//! main session's ruling of 19 Nov 2026:
//! - A property whose `Config` draws from a random seed (`rng_seed` Random) draws new cases on
//!   every run, natively and under Miri alike, so its cases are not a fixed set: each slice is a
//!   runner of the same strategy and `Config`, seed still random (a pinned seed would explore
//!   less), drawing and checking its own share of the cases, its range's length; the shares add
//!   up exactly to the whole's count, and no slice draws fewer than its share.
//! - A property of a fixed seed is one stream cut into ranges: slice `k` draws the stream from its
//!   first case to its range's end and checks its range, or, with [`run_slice_from`], starts at its
//!   first case's pinned seed and draws its range alone.
//!
//! `sliced!` and `sliced_from!` write a property as its four slices, `quarter_1` to `quarter_4`,
//! in a module named for the property.
//!
//! Guarantees: a random-seeded property's slices run, together, as many cases as one `proptest!`
//! test of the same strategy and configuration ran, from the same generator; a fixed-seed
//! property's slices run exactly its cases, in its order, never fewer draws. A failure names its
//! slice and its range of cases.
//!
//! Cost: a random-seeded slice draws and checks its share; a fixed-seed slice by [`run_slice`]
//! draws `(k + 1) / SLICES` of the stream, by [`run_slice_from`] only its own range.

#![allow(
    dead_code,
    unused_macros,
    reason = "each test binary uses the pieces it needs; the others are for its neighbours"
)]
#![allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::panic,
    reason = "counts of test cases; a broken cut must stop the test loudly (clippy.toml: tests may)"
)]

use std::cell::Cell;
use std::ops::Range;

use proptest::prelude::RngExt;
use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::{Config, RngAlgorithm, RngSeed, TestCaseError, TestRng, TestRunner};

/// How many slices a long test is cut into.
pub const SLICES: u32 = 4;

/// Slice `part` (from 0) of a list of `all` cases.
pub fn range(all: u32, part: u32) -> Range<u32> {
    let at = |k: u32| u32::try_from(u64::from(all) * u64::from(k) / u64::from(SLICES)).unwrap();
    at(part)..at(part + 1)
}

/// Slice `part` of a list of `all` cases, as `usize` indices.
pub fn range_usize(all: usize, part: u32) -> Range<usize> {
    let r = range(u32::try_from(all).unwrap(), part);
    usize::try_from(r.start).unwrap()..usize::try_from(r.end).unwrap()
}

/// Panics unless the [`SLICES`] ranges of `all` cases start at 0, each starts where the one before
/// it ends, none is empty, and the last ends at `all`: for a random-seeded property, that the
/// slices' shares add up exactly to the whole's count; for a fixed-seed one, that the ranges cut
/// its one stream with nothing missing or repeated (the module's text).
pub fn assert_joins(all: u32, what: &str) {
    let mut next = 0;
    for part in 0..SLICES {
        let r = range(all, part);
        assert_eq!(
            r.start,
            next,
            "{what}: slice {} starts where the one before ends",
            part + 1
        );
        assert!(r.end > r.start, "{what}: slice {} holds cases", part + 1);
        next = r.end;
    }
    assert_eq!(next, all, "{what}: the slices end at the last case");
}

/// Runs slice `part` of a property of `config.cases` cases (see the module's text): a
/// random-seeded property's share of fresh cases, or a fixed-seed property's range of its stream.
pub fn run_slice<S: Strategy>(
    config: Config,
    strategy: S,
    part: u32,
    check: impl Fn(S::Value) -> Result<(), TestCaseError>,
) {
    let cases = range(config.cases, part);
    if matches!(config.rng_seed, RngSeed::Random) {
        let share = cases.end - cases.start;
        let mut runner = TestRunner::new(Config {
            cases: share,
            ..config
        });
        if let Err(e) = runner.run(&strategy, check) {
            panic!("slice {} of {SLICES}, {share} cases: {e}", part + 1);
        }
        return;
    }
    let mut runner = TestRunner::new(Config {
        cases: cases.end,
        ..config
    });
    let drawn = Cell::new(0_u32);
    let outcome = runner.run(&strategy, |value| {
        let at = drawn.get();
        drawn.set(at + 1);
        if at < cases.start {
            Ok(())
        } else {
            check(value)
        }
    });
    if let Err(e) = outcome {
        panic!("slice {} of {SLICES}, cases {cases:?}: {e}", part + 1);
    }
}

/// The seed proptest's runner draws each case of `starts` from, in a run of `config` (a fixed
/// `rng_seed`, the default `ChaCha` generator). Before every case the runner reseeds its generator
/// with 32 bytes drawn from the generator as the case before left it (proptest 1.12.0's
/// `TestRunner::run`), so the seeds chain through every case's drawing, and a slice that starts
/// from its first case's seed draws no earlier case. `starts` ascending.
pub fn case_seeds<S: Strategy>(config: &Config, strategy: &S, starts: &[u32]) -> Vec<[u8; 32]> {
    assert_eq!(config.rng_algorithm, RngAlgorithm::ChaCha);
    let mut runner = TestRunner::new(config.clone());
    let mut seed: [u8; 32] = runner.rng().random();
    let mut out = Vec::new();
    for case in 0..=starts.last().copied().unwrap_or(0) {
        if starts.contains(&case) {
            out.push(seed);
        }
        let mut each = TestRunner::new_with_rng(
            config.clone(),
            TestRng::from_seed(RngAlgorithm::ChaCha, &seed),
        );
        strategy.new_tree(&mut each).unwrap();
        seed = each.rng().random();
    }
    out
}

/// The `Debug` text of the first `count` cases a runner of `config` gives its test, as
/// `TestRunner::run` gives them: what [`case_seeds`]' chain must reproduce.
pub fn runner_cases<S: Strategy>(config: &Config, strategy: &S, count: u32) -> Vec<String>
where
    S::Value: std::fmt::Debug,
{
    let seen = std::cell::RefCell::new(Vec::new());
    let mut runner = TestRunner::new(Config {
        cases: count,
        ..config.clone()
    });
    runner
        .run(strategy, |value| {
            seen.borrow_mut().push(format!("{value:?}"));
            Ok(())
        })
        .unwrap();
    seen.into_inner()
}

/// The `Debug` text of `count` cases drawn by [`case_seeds`]' chain from the first case's seed.
pub fn chained_cases<S: Strategy>(config: &Config, strategy: &S, count: u32) -> Vec<String>
where
    S::Value: std::fmt::Debug,
{
    let mut runner = TestRunner::new(config.clone());
    let mut seed: [u8; 32] = runner.rng().random();
    let mut out = Vec::new();
    for _ in 0..count {
        let mut each = TestRunner::new_with_rng(
            config.clone(),
            TestRng::from_seed(RngAlgorithm::ChaCha, &seed),
        );
        out.push(format!(
            "{:?}",
            strategy.new_tree(&mut each).unwrap().current()
        ));
        seed = each.rng().random();
    }
    out
}

/// Runs slice `part` of a property of `config.cases` cases from its first case's pinned seed
/// `first` ([`case_seeds`]): each of its cases drawn as the runner draws it, from the seed the
/// case before left, and checked; no earlier case is drawn. A failure names its case and input
/// (not shrunk).
pub fn run_slice_from<S: Strategy>(
    config: &Config,
    strategy: S,
    part: u32,
    first: [u8; 32],
    check: impl Fn(S::Value) -> Result<(), TestCaseError>,
) where
    S::Value: std::fmt::Debug,
{
    assert_eq!(config.rng_algorithm, RngAlgorithm::ChaCha);
    let cases = range(config.cases, part);
    let mut seed = first;
    for case in cases.clone() {
        let mut each = TestRunner::new_with_rng(
            config.clone(),
            TestRng::from_seed(RngAlgorithm::ChaCha, &seed),
        );
        let tree = strategy
            .new_tree(&mut each)
            .unwrap_or_else(|e| panic!("slice {}, case {case}: {e}", part + 1));
        if let Err(e) = check(tree.current()) {
            panic!(
                "slice {} of {SLICES}, case {case}: {e}\ninput: {:?}",
                part + 1,
                tree.current()
            );
        }
        seed = each.rng().random();
    }
}

/// A property of a fixed seed as its [`SLICES`] slices, each started from its pinned first seed:
/// `sliced_from!(name, config, (strategies,), SEEDS, |(values,)| check)`.
macro_rules! sliced_from {
    ($name:ident, $config:expr, $strategy:expr, $seeds:expr, |$value:pat_param| $check:expr) => {
        mod $name {
            use super::*;

            #[test]
            fn quarter_1() {
                crate::slices::run_slice_from(&$config, $strategy, 0, $seeds[0], |$value| $check);
            }

            #[test]
            fn quarter_2() {
                crate::slices::run_slice_from(&$config, $strategy, 1, $seeds[1], |$value| $check);
            }

            #[test]
            fn quarter_3() {
                crate::slices::run_slice_from(&$config, $strategy, 2, $seeds[2], |$value| $check);
            }

            #[test]
            fn quarter_4() {
                crate::slices::run_slice_from(&$config, $strategy, 3, $seeds[3], |$value| $check);
            }
        }
    };
}

/// A property as its [`SLICES`] slices, `quarter_1` to `quarter_4`, in a module named for it:
/// `sliced!(name, config, (strategies,), |(values,)| check)`.
macro_rules! sliced {
    ($name:ident, $config:expr, $strategy:expr, |$value:pat_param| $check:expr) => {
        mod $name {
            use super::*;

            #[test]
            fn quarter_1() {
                crate::slices::run_slice($config, $strategy, 0, |$value| $check);
            }

            #[test]
            fn quarter_2() {
                crate::slices::run_slice($config, $strategy, 1, |$value| $check);
            }

            #[test]
            fn quarter_3() {
                crate::slices::run_slice($config, $strategy, 2, |$value| $check);
            }

            #[test]
            fn quarter_4() {
                crate::slices::run_slice($config, $strategy, 3, |$value| $check);
            }
        }
    };
}

/// A deterministic test as its [`SLICES`] slices, `quarter_1` to `quarter_4`, in a module named
/// for it: `quarters!(name, |part| body)`, the body running its slice `part` (from 0).
macro_rules! quarters {
    ($name:ident, |$part:ident| $body:expr) => {
        mod $name {
            use super::*;

            fn slice($part: u32) {
                $body
            }

            #[test]
            fn quarter_1() {
                slice(0);
            }

            #[test]
            fn quarter_2() {
                slice(1);
            }

            #[test]
            fn quarter_3() {
                slice(2);
            }

            #[test]
            fn quarter_4() {
                slice(3);
            }
        }
    };
}
