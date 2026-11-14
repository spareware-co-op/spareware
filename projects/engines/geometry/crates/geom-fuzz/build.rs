//! Declares `cfg(kani)` to the compiler's check of configuration names, and sets
//! `cfg(unoptimised)` in a build at `opt-level` 0.
//!
//! Kani sets `cfg(kani)` when it compiles a proof, and each target's `<target>_proof` test is
//! marked `#[cfg_attr(kani, kani::proof)]` (`README.md`, the layout of a target). Under every other
//! build the name is unknown, and `unexpected_cfgs` would warn on them, which CI denies. The declarations belong
//! here rather than in the manifest's `[lints]` table, which repeats `geom-num`'s exactly
//! (`geom-test`'s `tests/attack_g11.rs`); a build script's `cargo::rustc-check-cfg` instruction is
//! Cargo's other way to declare one (the Cargo book, "Build Scripts", `cargo::rustc-check-cfg`).
//! The script reads no file, so Cargo runs it once per profile.
//!
//! `cfg(unoptimised)`: the seeded tests run a tenth of their cases in an unoptimised build
//! (`seeded::for_this_build`), which `debug_assertions` cannot say, since the pull requests' test
//! run is optimised with debug assertions on. Cargo gives a build script the profile's
//! optimisation level as `OPT_LEVEL` (the Cargo book, "Build Scripts", environment variables).

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-check-cfg=cfg(kani)");
    println!("cargo::rustc-check-cfg=cfg(unoptimised)");
    if std::env::var("OPT_LEVEL").is_ok_and(|level| level == "0") {
        println!("cargo::rustc-cfg=unoptimised");
    }
}
