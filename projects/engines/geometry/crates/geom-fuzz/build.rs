//! Declares `cfg(kani)` and `cfg(fuzzing_libfuzzer)` to the compiler's check of configuration
//! names.
//!
//! Kani sets `cfg(kani)` when it compiles a proof, and each target's `<target>_proof` test is
//! marked `#[cfg_attr(kani, kani::proof)]` (`README.md`, the layout of a target). cargo-bolero sets
//! `cfg(fuzzing_libfuzzer)` when it builds the tests for libFuzzer, and the tests' alarm thread
//! (`tests/common/mod.rs`, issue #764) is built only then. Under every other build the names are
//! unknown, and `unexpected_cfgs` would warn on them, which CI denies. The declarations belong
//! here rather than in the manifest's `[lints]` table, which repeats `geom-num`'s exactly
//! (`geom-test`'s `tests/attack_g11.rs`); a build script's `cargo::rustc-check-cfg` instruction is
//! Cargo's other way to declare one (the Cargo book, "Build Scripts", `cargo::rustc-check-cfg`).
//! The script reads nothing, so Cargo runs it once.

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-check-cfg=cfg(kani)");
    println!("cargo::rustc-check-cfg=cfg(fuzzing_libfuzzer)");
}
