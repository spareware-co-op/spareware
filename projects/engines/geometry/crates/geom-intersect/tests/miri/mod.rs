//! How much work a test does under Miri.
//!
//! Miri interprets each step thousands of times slower than it runs, and stops a test at ten
//! minutes (`.config/nextest.toml`); a stopped test gives no verdict. In this crate one search or
//! one meeting is often the whole cost: a nearest point on a surface takes tens of thousands of
//! work units, minutes under Miri, at any floor, because the cost is the first sweep over the
//! domain and not the narrowing to the floor. So a test runs a smaller case there: fewer of the
//! same cases, drawn the same way, or the same kind of case on a shorter domain, at a smaller
//! scale or under a smaller budget, chosen so that it reaches the functions the native case does
//! and makes the same claims. A count or a work total that depends on the size is pinned for the
//! Miri case as measured on it. A native or WebAssembly run is unchanged, no test is ignored under
//! Miri, and geom-test's `kernel_quality_workflow.rs` holds each of this crate's tests to a
//! recorded time under Miri within its budget.

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

/// The values a loop runs over: `native` natively, and under Miri `miri`, some of the same values,
/// at least one.
pub fn rounds<'a, T: PartialEq>(native: &'a [T], miri: &'a [T]) -> &'a [T] {
    assert!(
        !miri.is_empty(),
        "a loop keeps at least one round under Miri"
    );
    assert!(
        miri.iter().all(|m| native.contains(m)),
        "a Miri round is one of the native rounds"
    );
    if cfg!(miri) { miri } else { native }
}

/// One case's input: `native` natively, and under Miri `miri`, a smaller case of the same kind
/// (a shorter domain, a smaller scale, a smaller budget), which the test's comment beside it
/// describes.
pub fn case<T>(native: T, miri: T) -> T {
    if cfg!(miri) { miri } else { native }
}
