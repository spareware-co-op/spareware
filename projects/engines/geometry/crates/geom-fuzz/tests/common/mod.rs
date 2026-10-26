//! What geom-fuzz's test files share: [`holds`], which turns a harness's verdict into a panic.

use geom_fuzz::Finding;

/// Panics with `verdict`'s finding, if it has one.
///
/// A bolero test's body hands its verdict here rather than returning the [`Finding`], so that the
/// same test can be a Kani proof (the crate README, the rules of the layout). Under Kani, bolero's
/// engine turns a returned error into its `PanicError`, whose constructor may capture a backtrace
/// (`bolero-engine`'s `panic.rs`), and Kani would have to explore that capture, which kept the
/// first proofs in symbolic execution for an hour. A panic under Kani is a failed property and
/// nothing more: Kani's `panic!` formats nothing, and its report traces every value. Under
/// `cargo test`, libFuzzer and `ClusterFuzzLite` a panic fails the input as a returned finding
/// would, and its message is the finding.
pub fn holds(verdict: Result<(), Finding>) {
    if let Err(finding) = verdict {
        panic!("{finding}");
    }
}
