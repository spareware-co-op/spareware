//! How much work a test of many cases, rounds or levels does under Miri.
//!
//! Miri interprets each step thousands of times slower than it runs, and stops a test at ten
//! minutes (`.config/nextest.toml`); a stopped test gives no verdict. So a test of many cases or
//! rounds, or of a large input, runs a smaller one there: the same inputs, drawn the same way,
//! only fewer or smaller, and never so few that a kernel function the whole test reaches is left
//! out (measured natively, building the test with `--cfg miri` and listing the functions each
//! run enters). A native or WebAssembly run is unchanged. No test is ignored under Miri, and
//! geom-test's `kernel_quality_workflow.rs` holds each of this crate's tests to a recorded time
//! under Miri within its budget.

#![allow(
    dead_code,
    reason = "each test file that includes this module uses only some of it"
)]

/// `native` on a native or WebAssembly build, `miri` under Miri. `miri` is at least one and at
/// most `native`, so a test never runs nothing under Miri, nor more than it does natively.
pub const fn sized(native: usize, miri: usize) -> usize {
    assert!(
        0 < miri && miri <= native,
        "a Miri size is 1 to the native size"
    );
    if cfg!(miri) { miri } else { native }
}

/// A loop's step: `native` natively, `miri` under Miri, at least as long, so the loop takes every
/// `miri / native`th of its native rounds there (a whole multiple, so each is a native round).
pub const fn stride(native: usize, miri: usize) -> usize {
    assert!(
        0 < native && native <= miri && miri.is_multiple_of(native),
        "a Miri step is a whole multiple of the native step"
    );
    if cfg!(miri) { miri } else { native }
}

/// A lower bound whose rise makes a case smaller, such as the fewest polygons a box holds as a
/// leaf (fewer boxes) or the fewest solids of a scene: `native` natively, `miri`, no lower, under
/// Miri.
pub const fn at_least(native: usize, miri: usize) -> usize {
    assert!(native <= miri, "a Miri bound is at least the native one");
    if cfg!(miri) { miri } else { native }
}

/// [`sized`] for a proptest case count.
pub const fn cases(native: u32, miri: u32) -> u32 {
    assert!(
        0 < miri && miri <= native,
        "a Miri size is 1 to the native size"
    );
    if cfg!(miri) { miri } else { native }
}

/// The values a loop runs over: `native` natively, and under Miri `miri`, fewer and smaller cases
/// of the same kind. For a loop over leaf sizes, `miri` is the whole box and a leaf of one polygon
/// fewer than the scene holds, so the boxes split, and split only as far as that needs.
pub fn rounds<'a, T>(native: &'a [T], miri: &'a [T]) -> &'a [T] {
    assert!(
        !miri.is_empty(),
        "a loop keeps at least one round under Miri"
    );
    if cfg!(miri) { miri } else { native }
}

/// Whether a loop runs round `round`: every round natively, and under Miri only those of
/// `chosen`, the fewest rounds that between them reach every kernel function the whole loop does.
/// The loop still draws every round's input, so a chosen round is the same input as natively.
pub fn keeps(round: usize, chosen: &[usize]) -> bool {
    assert!(
        !chosen.is_empty(),
        "a loop keeps at least one round under Miri"
    );
    !cfg!(miri) || chosen.contains(&round)
}
