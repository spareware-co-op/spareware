//! One long test's cases cut into slices, each its own test, so that each ends within a Miri gate
//! part (decision 32.78: a row of at most 486 yardstick seconds). A test binary takes it with
//! `#[macro_use] mod slices;`.
//!
//! What it does: [`range`] cuts a list of `all` cases into [`SLICES`] ranges that join to the
//! whole list, each once; [`assert_joins`] checks that they do, cheaply, for the binary's own
//! `the_slices_join_to_every_case_once` test. [`run_slice`] runs one slice of a property: the
//! runner `proptest!` would make from the same configuration (so the same seeding, Miri's fixed
//! one under Miri) draws the property's one stream of cases from the first up to the slice's end,
//! every one of them, and checks those in the slice. `sliced!` writes a property as its four
//! slices, `quarter_1` to `quarter_4`, in a module named for the property.
//!
//! Guarantees: the slices of a property together run exactly the cases one `proptest!` test of
//! the same strategy and configuration ran, drawn in the same order, never fewer draws; a failure
//! names its slice and its range of cases.
//!
//! Cost: slice `k` draws `(k + 1) / SLICES` of the stream and checks a `1 / SLICES` of it.

#![allow(
    dead_code,
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

use proptest::strategy::Strategy;
use proptest::test_runner::{Config, TestCaseError, TestRunner};

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
/// it ends, none is empty, and the last ends at `all`.
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

/// Runs slice `part` of a property of `config.cases` cases (see the module's text).
pub fn run_slice<S: Strategy>(
    config: Config,
    strategy: S,
    part: u32,
    check: impl Fn(S::Value) -> Result<(), TestCaseError>,
) {
    let cases = range(config.cases, part);
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
