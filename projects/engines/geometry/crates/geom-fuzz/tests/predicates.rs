//! The predicates fuzz target, run as tests on stable Rust: bolero's test mode and the seeded leg.
//!
//! - `predicates_fuzz` wraps `geom_fuzz::predicates::harness` in `bolero::check!`. Under
//!   `cargo test` it replays the files in `tests/__fuzz__/predicates/corpus` and `crashes`, if any,
//!   then runs [`ITERATIONS`] random byte strings. bolero draws their seeds from the operating
//!   system and prints the seed of a failing one as `BOLERO_RANDOM_SEED=<seed>`, which replays it;
//!   `BOLERO_RANDOM_ITERATIONS` changes the count. Under
//!   `cargo bolero test predicates_fuzz --package geom-fuzz` the same test is a libFuzzer target
//!   (see the crate header for the command).
//! - `bolero_fails_on_a_finding` shows the wiring: a harness that returns a `Finding` fails the
//!   bolero leg, not only one that panics. It catches bolero's panic itself rather than being a
//!   `#[should_panic]` test, which would break `ClusterFuzzLite`'s build (the crate README, the
//!   rules of the layout).
//! - `predicates_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed
//!   seed (`SPAREKIT_SEED` replays it), so every pull request also runs a fixed set of cases, the
//!   same on every target.

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

use core::sync::atomic::{AtomicBool, Ordering};

use geom_fuzz::predicates::{MAX_INPUT_BYTES, harness};
use geom_fuzz::seeded;
use geom_test::Seed;

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 10_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 20_000;

/// The longest byte string the seeded leg draws. The harness reads at most
/// [`MAX_INPUT_BYTES`] (270) bytes (derived in `geom_fuzz::predicates`); the 50 more let some
/// strings carry bytes nobody reads, which must change nothing.
const MAX_LEN: u16 = 320;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn predicates_fuzz() {
    println!(
        "bolero test mode: corpus and crash files, then {ITERATIONS} random inputs whose seeds \
         bolero draws from the operating system; a failing input's seed is printed as \
         BOLERO_RANDOM_SEED=<seed>, which replays it"
    );
    bolero::check!()
        .with_iterations(ITERATIONS)
        .with_max_len(usize::from(MAX_LEN))
        .for_each(|data: &[u8]| harness(data));
}

/// bolero's test mode fails, with its panic "test failed", on a harness that returns a
/// [`geom_fuzz::Finding`].
///
/// Not a `#[should_panic]` test. cargo-bolero's `build-clusterfuzz` builds each test binary by
/// running one of its bolero tests, the first it listed, with `CARGO_BOLERO_SELECT` set, where
/// `check!` returns without running its body (`bolero-engine`'s `TargetLocation::should_run`); a
/// `#[should_panic]` test fails there, and the build with it. So this test notes whether the body
/// ran, and holds bolero to failing exactly when it did; a body that did not run is accepted only
/// in cargo-bolero's selection mode.
#[cfg_attr(
    target_family = "wasm",
    ignore = "catching bolero's panic needs unwinding, which wasm32-wasip1 does not have"
)]
#[test]
fn bolero_fails_on_a_finding() {
    let ran = AtomicBool::new(false);
    let outcome = std::panic::catch_unwind(|| {
        bolero::check!().with_iterations(100).for_each(
            |_: &[u8]| -> Result<(), geom_fuzz::Finding> {
                ran.store(true, Ordering::Relaxed);
                Err(geom_fuzz::Finding {
                    law: "never holds".into(),
                    detail: String::new(),
                })
            },
        );
    });
    if ran.load(Ordering::Relaxed) {
        let payload = outcome.expect_err("bolero returned normally from a harness that failed");
        let message = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str));
        assert_eq!(message, Some("test failed"));
    } else {
        assert!(outcome.is_ok());
        assert!(
            std::env::var_os("CARGO_BOLERO_SELECT").is_some(),
            "bolero ran no input, outside cargo-bolero's selection mode"
        );
    }
}

#[test]
fn predicates_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "predicates fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}
