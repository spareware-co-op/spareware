//! A printed seed replays exactly: the `SPAREKIT_SEED` override, end to end.
//!
//! Rust 2024 makes changing this process's environment `unsafe`, which the kernel's lints forbid,
//! so the override is tested the way a person uses it: this test binary runs itself as a child
//! process, with `SPAREKIT_SEED` set or unset, and runs only `child_draws_from_its_seed`. The child
//! prints its seed line and its first draws; the parent checks that the child chose the seed it was
//! given, printed it in the documented form, and drew exactly what the same seed draws here. A
//! malformed override must make the child fail, naming the variable, rather than run another seed.
//!
//! The same way, `child_runs_a_seeded_property` checks `geom_test::prop`'s promise about proptest's
//! own environment variables: the runner's variables (`PROPTEST_RNG_SEED`, `PROPTEST_CASES`,
//! `PROPTEST_RNG_ALGORITHM`, `PROPTEST_MAX_SHRINK_ITERS`) change nothing, and
//! `PROPTEST_MAX_DEFAULT_SIZE_RANGE`, which proptest applies process-wide to collection strategies
//! where no runner setting can reach it, makes a seeded run refuse to draw rather than draw other
//! cases.

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

use std::process::{Command, Output};

use std::cell::RefCell;

use geom_test::grid::GridSampler;
use geom_test::prop;
use geom_test::seed::{ENV_VAR, resolve};
use geom_test::{Seed, SplitMix64};
use proptest::prelude::any;

/// The child's default seed: what it uses when `SPAREKIT_SEED` is unset.
const CHILD_DEFAULT: Seed = Seed(0x0123_4567_89ab_cdef);

/// The first draws from a seed: three `SplitMix64` words and two hard-case grid points.
fn draws(seed: Seed) -> String {
    let mut rng = SplitMix64::new(seed);
    let words = [rng.next_u64(), rng.next_u64(), rng.next_u64()];
    let mut grid = GridSampler::new();
    let points = [grid.p3(&mut rng), grid.p3(&mut rng)];
    format!("draws: {words:?} {points:?}")
}

/// Runs this test binary's child test with `SPAREKIT_SEED` set to `value`, or unset.
fn child(value: Option<&str>) -> Output {
    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.args([
        "child_draws_from_its_seed",
        "--exact",
        "--nocapture",
        "--test-threads=1",
    ]);
    match value {
        Some(v) => cmd.env(ENV_VAR, v),
        None => cmd.env_remove(ENV_VAR),
    };
    cmd.output().unwrap()
}

/// The child: resolves its seed and prints its first draws. Run directly, it simply passes.
#[test]
fn child_draws_from_its_seed() {
    let seed = match resolve("seed replay child", CHILD_DEFAULT) {
        Ok(seed) => seed,
        Err(e) => panic!("{e}"),
    };
    println!("{}", draws(seed));
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "runs this test binary as a child process, which WebAssembly cannot"
)]
#[test]
fn a_printed_seed_replays_exactly() {
    // Unset: the default, said to be the default.
    let out = child(None);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}");
    assert!(
        stdout.contains(
            "seed: 0123456789abcdef (seed replay child; the test's default; \
             replay with SPAREKIT_SEED=0123456789abcdef)"
        ),
        "{stdout}"
    );
    assert!(stdout.contains(&draws(CHILD_DEFAULT)), "{stdout}");

    // Set: every accepted spelling of one seed gives the same line and the same draws.
    let seed = Seed(0xfeed_f00d_dead_beef);
    for spelling in [
        "feedf00ddeadbeef",
        "0xFEEDF00DDEADBEEF",
        "0XfeedF00dDeadBeef",
    ] {
        let out = child(Some(spelling));
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success(), "{spelling}: {stdout}");
        assert!(
            stdout.contains(
                "seed: feedf00ddeadbeef (seed replay child; from SPAREKIT_SEED; \
                 replay with SPAREKIT_SEED=feedf00ddeadbeef)"
            ),
            "{spelling}: {stdout}"
        );
        assert!(stdout.contains(&draws(seed)), "{spelling}: {stdout}");
    }

    // A short seed is the same number with leading zeros.
    let out = child(Some("2a"));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("seed: 000000000000002a"), "{stdout}");
    assert!(stdout.contains(&draws(Seed(42))), "{stdout}");
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "runs this test binary as a child process, which WebAssembly cannot"
)]
#[test]
fn a_malformed_seed_fails_and_never_runs_another() {
    for bad in ["", "12z", "10000000000000000", " 1", "-1"] {
        let out = child(Some(bad));
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(), "{bad:?} was accepted: {stdout}");
        assert!(
            stdout.contains("SPAREKIT_SEED is not a seed") || stderr.contains("SPAREKIT_SEED"),
            "{bad:?}: {stdout}{stderr}"
        );
        assert!(!stdout.contains("draws:"), "{bad:?} drew anyway: {stdout}");
    }
}

/// Every environment variable proptest 1.12 reads (`test_runner/config.rs`,
/// `contextualize_config`), removed from every child so the parent's environment cannot leak in.
const PROPTEST_VARS: [&str; 13] = [
    "PROPTEST_CASES",
    "PROPTEST_MAX_LOCAL_REJECTS",
    "PROPTEST_MAX_GLOBAL_REJECTS",
    "PROPTEST_MAX_FLAT_MAP_REGENS",
    "PROPTEST_MAX_SHRINK_TIME",
    "PROPTEST_MAX_SHRINK_ITERS",
    "PROPTEST_MAX_DEFAULT_SIZE_RANGE",
    "PROPTEST_VERBOSE",
    "PROPTEST_RNG_ALGORITHM",
    "PROPTEST_RNG_SEED",
    "PROPTEST_DISABLE_FAILURE_PERSISTENCE",
    "PROPTEST_FORK",
    "PROPTEST_TIMEOUT",
];

/// The child: a seeded property over grid points and a default-sized collection, printing every
/// case it was given. Run directly, it simply passes.
#[test]
fn child_runs_a_seeded_property() {
    let cases = RefCell::new(Vec::new());
    let r = prop::run(
        "seeded property child",
        Seed(0x77),
        16,
        &(prop::p3(), any::<Vec<u8>>()),
        |(p, v)| {
            cases.borrow_mut().push(format!("{p:?} {}", v.len()));
            Ok(())
        },
    );
    if let Err(e) = r {
        panic!("{e}");
    }
    println!("property cases: {:?}", cases.borrow());
}

/// Runs `child_runs_a_seeded_property` with no proptest variable and no `SPAREKIT_SEED`, then
/// `set`; its exit status and stdout and stderr.
fn property_child(set: &[(&str, &str)]) -> (bool, String) {
    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.args([
        "child_runs_a_seeded_property",
        "--exact",
        "--nocapture",
        "--test-threads=1",
    ]);
    for v in PROPTEST_VARS {
        cmd.env_remove(v);
    }
    cmd.env_remove(ENV_VAR);
    for (k, v) in set {
        cmd.env(k, v);
    }
    let out = cmd.output().unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), text)
}

/// The `property cases:` line of a child's output.
fn cases_line(text: &str) -> &str {
    text.lines()
        .find_map(|l| l.find("property cases: ").map(|i| &l[i..]))
        .unwrap_or_else(|| panic!("no cases line in:\n{text}"))
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "runs this test binary as a child process, which WebAssembly cannot"
)]
#[test]
fn proptest_variables_cannot_change_a_seeded_run() {
    let (ok, base) = property_child(&[]);
    assert!(ok, "{base}");
    let base = cases_line(&base).to_owned();
    // The default-sized collection really is drawn: not every one of the 16 cases has an empty
    // vector (each case prints as "<point> <length>").
    assert!(base.matches(" 0\"").count() < 16, "{base}");

    // The runner's variables are fixed by `prop::config`, and the size variable at its default
    // value is no change.
    for set in [
        &[
            ("PROPTEST_RNG_SEED", "12345"),
            ("PROPTEST_CASES", "1"),
            ("PROPTEST_RNG_ALGORITHM", "xs"),
            ("PROPTEST_MAX_SHRINK_ITERS", "0"),
            ("PROPTEST_MAX_LOCAL_REJECTS", "1"),
            ("PROPTEST_MAX_GLOBAL_REJECTS", "1"),
            ("PROPTEST_VERBOSE", "2"),
        ][..],
        &[("PROPTEST_MAX_DEFAULT_SIZE_RANGE", "100")][..],
    ] {
        let (ok, text) = property_child(set);
        assert!(ok, "{set:?}: {text}");
        assert_eq!(cases_line(&text), base, "{set:?}");
    }

    // A different default collection size cannot be undone by any runner setting, so the run
    // refuses, naming the variable, and draws nothing. (With 0, proptest itself panics while the
    // child builds `any::<Vec<u8>>()`, an empty size range, before the run is asked to start.)
    for size in ["1", "5", "1000"] {
        let (ok, text) = property_child(&[("PROPTEST_MAX_DEFAULT_SIZE_RANGE", size)]);
        assert!(
            !ok,
            "PROPTEST_MAX_DEFAULT_SIZE_RANGE={size} was accepted:\n{text}"
        );
        assert!(
            text.contains("nothing was drawn: PROPTEST_MAX_DEFAULT_SIZE_RANGE"),
            "{size}: {text}"
        );
        assert!(!text.contains("property cases: "), "{size}: {text}");
    }
}
