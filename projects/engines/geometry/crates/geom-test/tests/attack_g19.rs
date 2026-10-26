//! Attacks on piece G19 (the determinism matrix and the golden store). Each test states what the
//! board's done-when, chapters 21 and 28.13 of the kernel architecture, or a guarantee in a
//! `golden` module header requires, and tries to break it. A test that passes is a guard on a
//! guarantee the piece keeps.
//!
//! The done-when: "Canonical outputs of every geom-num test vector are hashed with SHA-256 on every
//! target, build mode and target-cpu=native; a golden store keyed by edition with the
//! owner-approval rule for edits; the perturbing allocator, the in-process repeat and input-shuffle
//! hooks; all legs green."
//!
//! The in-memory attacks run on every target, WebAssembly included; the ones that read the real
//! tree, run git or run the `golden` binary run natively only.

// Tests may use native arithmetic, casts and std's reductions to build their fixtures.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "fixtures; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken fixture must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

use geom_test::Seed;
use geom_test::golden::diff::{ApprovalError, approved, diff};
use geom_test::golden::matrix::{RunError, plan, verify};
use geom_test::golden::perturb::{Heap, MAX_BYTES, address_order_digest};
use geom_test::golden::shuffle::{Orders, order_free};
use geom_test::golden::store::{ChangeKind, EMBEDDED, State, Store, StoreError};
use geom_test::golden::{self, Digest, GoldenError, Sha256, check_in, hex, sha256};
use serde_json::Value;

/// The kernel's CI workflow, as committed, read when the test is built so the test runs on every
/// target.
const KERNEL_YML: &str = include_str!("../../../../../../.github/workflows/kernel.yml");

/// The golden store's own workflow.
const GOLDEN_YML: &str = include_str!("../../../../../../.github/workflows/kernel-golden.yml");

/// The store as JSON, to mutate.
fn store_json() -> Value {
    serde_json::from_str(EMBEDDED).unwrap()
}

/// A store from JSON, which must parse.
fn parse(v: &Value) -> Store {
    Store::parse(&v.to_string()).unwrap_or_else(|e| panic!("{e}\n{v:#}"))
}

/// The current edition's goldens, mutable.
fn goldens(v: &mut Value) -> &mut Vec<Value> {
    let editions = v["editions"].as_array_mut().unwrap();
    editions.last_mut().unwrap()["goldens"]
        .as_array_mut()
        .unwrap()
}

/// The digest `c` repeated: a well-formed digest that is no golden's.
fn fake(c: char) -> String {
    std::iter::repeat_n(c, 64).collect()
}

// ---------------------------------------------------------------------------------------------
// 1. Canonical outputs hashed with SHA-256 on every target, build mode and target-cpu=native.
// ---------------------------------------------------------------------------------------------

/// Every golden in the store is a SHA-256 digest (64 lowercase hexadecimal digits) of a geom-num,
/// geom-math, geom-run, geom-test or geom-bench corpus, and geom-num's corpora are all there: its
/// determinism corpus over every module, the seeded generator's two, G25's, G05's and the
/// order-free one.
#[test]
fn every_golden_is_sha256_and_geom_num_has_all_its_corpora() {
    let store = Store::embedded().unwrap();
    let edition = store.current().unwrap();
    assert_eq!(edition.edition, "pre-edition");
    assert_eq!(edition.state, State::Open);
    for g in &edition.goldens {
        assert!(Digest::is_canonical_hex(&g.sha256), "{}", g.id);
        assert_eq!(Digest::from_hex(&g.sha256).unwrap().to_string(), g.sha256);
    }
    let ids: BTreeSet<&str> = edition.goldens.iter().map(|g| g.id.as_str()).collect();
    for id in [
        "geom-num/determinism",
        "geom-num/rng-draws",
        "geom-num/rng-stream-ranges",
        "geom-num/dd-exact-sum",
        "geom-num/proven-maths",
        "geom-num/order-free",
        "geom-math/determinism",
        "geom-math/workload",
        "geom-run/helpers",
        "geom-test/grid-sampler",
        "geom-test/prop-cases",
        "geom-test/shuffle-permutations",
        "geom-bench/output-hashes",
    ] {
        assert!(ids.contains(id), "{id} is not in the store");
    }
}

/// The digests every golden had before this piece moved them into the store, byte for byte: the
/// SHA-256 goldens G02, G03, G04, G25 and G32 pinned as constants, which the store must hold
/// unchanged. Their histories hold exactly the changes made to them on main before the store
/// existed, each with the digest the constant had before: G05 moved geom-num's corpus and both
/// of geom-math's goldens (its correctly rounded maths); nothing else moved, so no other golden has
/// a record.
#[test]
fn the_goldens_the_crates_pinned_are_byte_identical_in_the_store() {
    let before: [(&str, &str); 7] = [
        (
            "geom-num/determinism",
            "fb19edfc7c58ca5e93a8921dfbdd2ab2dfacc2d180fbe6dec540efbede954564",
        ),
        (
            "geom-num/rng-draws",
            "706bf19f86a2ac5ba9be55866a1a5b151702dd6707b9b3b67a5c4c347cbabb7f",
        ),
        (
            "geom-num/rng-stream-ranges",
            "4d2c3a986913d7751a8f49eb3a7e5340eb973b591154c7acdaaf0ad47b6472f7",
        ),
        (
            "geom-num/dd-exact-sum",
            "ce0e1901d8d11ff1b2d490e929b51fae2adfd7479c63126f8359ac917d0cb0fd",
        ),
        (
            "geom-math/determinism",
            "dd9daf8861292fda5cd2de1d789a6f9739a4672145c9756cdc5299a6fe772ab6",
        ),
        (
            "geom-math/workload",
            "977828731c890a725eae9d2f6eb79b9d04a69f927ffcf20525228c93f2cfda25",
        ),
        (
            "geom-run/helpers",
            "b5e1993aceab1c8f0a6cdfd0c786974375634b5470c635da384122a4bc015172",
        ),
    ];
    let store = Store::embedded().unwrap();
    let edition = store.current().unwrap();
    for (id, digest) in before {
        let g = edition.golden(id, &BTreeMap::new()).unwrap();
        assert_eq!(g.sha256, digest, "{id} moved");
    }
    // The changes main made before the store, each the constant's earlier value (G02's and G03's
    // SHA-256 goldens, moved by G05), and no other record anywhere.
    let changed: BTreeMap<&str, &str> = [
        (
            "geom-num/determinism",
            "c9c61875e6da8cff4ffa79f147d8f32187fa2929564100ce834849bb43a211f8",
        ),
        (
            "geom-math/determinism",
            "d79932a18468bf79d55bdb204597c001ad9b512c5000fae5e87d7db01a72f126",
        ),
        (
            "geom-math/workload",
            "c1c5d1b624f95d7ce6b6640acef43dda0a90ae2c4beb6c30483fa87a234747c9",
        ),
    ]
    .into_iter()
    .collect();
    for g in &edition.goldens {
        match changed.get(g.id.as_str()) {
            Some(old) => {
                assert_eq!(g.history.len(), 1, "{}", g.id);
                assert_eq!(g.history[0].sha256, *old, "{}", g.id);
                assert_eq!(g.history[0].kind, ChangeKind::Improvement, "{}", g.id);
                assert_eq!(g.history[0].piece, "G05", "{}", g.id);
            }
            None => assert!(g.history.is_empty(), "{} has a history record", g.id),
        }
    }
}

/// The steps of kernel.yml's native job that run every golden, in each build mode the done-when
/// names: release with overflow checks on, release as shipped, release for the machine's own CPU
/// (in its own build folder) and debug; and geom-num's whole suite, every test vector, in the two
/// release modes the whole-suite steps do not cover. Each is checked as written, so a step that
/// lost its environment or its arguments fails here.
#[test]
fn kernel_yml_runs_every_golden_in_every_native_build_mode() {
    let yml = KERNEL_YML.replace("\r\n", "\n");
    let steps = [
        "      - name: Every golden in release, with overflow checks on
        if: ${{ !cancelled() && matrix.mode == 'release' }}
        env:
          RUSTFLAGS: -C overflow-checks=on
        run: cargo run --locked -p geom-test --bin golden -- run --release
",
        "      - name: Every golden in release as shipped, without overflow checks
        if: ${{ !cancelled() && matrix.mode == 'release' }}
        run: cargo run --locked -p geom-test --bin golden -- run --release
",
        "      - name: geom-num's every test vector in release as shipped
        if: ${{ !cancelled() && matrix.mode == 'release' }}
        run: cargo test -p geom-num --release --locked --no-fail-fast
",
        "        env:
          RUSTFLAGS: -C overflow-checks=on -C target-cpu=native
          # Its own build folder, which the build cache never saves or restores: the cache's key
          # does not name the processor, so a restored native build from another machine's CPU
          # can hold instructions this one lacks (SIGILL). Built fresh here, it is this CPU's.
          CARGO_TARGET_DIR: target-native
        run: cargo run --locked -p geom-test --bin golden -- run --release
",
        "      - name: geom-num's every test vector in release for this machine's own CPU
        if: ${{ !cancelled() && matrix.mode == 'release' }}
        env:
          RUSTFLAGS: -C overflow-checks=on -C target-cpu=native
          CARGO_TARGET_DIR: target-native
        run: cargo test -p geom-num --release --locked --no-fail-fast
",
        "      - name: Every golden in debug
        if: ${{ !cancelled() && matrix.mode == 'debug' }}
        run: cargo run --locked -p geom-test --bin golden -- run
",
        "      - name: Test suite in release, with overflow checks on
        if: ${{ !cancelled() && matrix.mode == 'release' }}
        env:
          RUSTFLAGS: -C overflow-checks=on
        run: cargo test $KERNEL_CRATES --release --locked --no-fail-fast
",
        "      - name: Test suite in debug
        if: ${{ !cancelled() && matrix.mode == 'debug' }}
        run: cargo test $KERNEL_CRATES --locked --no-fail-fast
",
    ];
    for step in steps {
        assert!(yml.contains(step), "kernel.yml lacks the step:\n{step}");
    }
    // On every operating system and processor of chapter 21's list.
    for os in [
        "os: ubuntu-latest",
        "os: ubuntu-24.04-arm",
        "os: windows-latest",
        "os: windows-11-arm",
        "os: macos-latest",
        "os: macos-15-intel",
    ] {
        assert!(yml.contains(os), "kernel.yml's test job lacks {os}");
    }
    // No golden step may be skipped by a failure before it, or continue on its own failure.
    assert!(!yml.contains("continue-on-error"));
}

/// The wasm32 job runs every golden in debug and in both release modes, under wasmtime and under
/// Node, and geom-num's every test vector in both release modes.
#[test]
fn kernel_yml_runs_every_golden_on_wasm32_in_every_build_mode() {
    let yml = KERNEL_YML.replace("\r\n", "\n");
    assert!(yml.contains("runtime: [wasmtime, node]"));
    for step in [
        "        env:
          RUSTFLAGS: -C overflow-checks=on
        run: cargo run --locked -p geom-test --bin golden -- run --target wasm32-wasip1 --release
",
        "      - name: Every golden on wasm32 in release as shipped, without overflow checks
        if: ${{ !cancelled() }}
        run: cargo run --locked -p geom-test --bin golden -- run --target wasm32-wasip1 --release
",
        "      - name: Every golden on wasm32 in debug
        if: ${{ !cancelled() }}
        run: cargo run --locked -p geom-test --bin golden -- run --target wasm32-wasip1
",
        "      - name: geom-num's every test vector on wasm32 in release as shipped
        if: ${{ !cancelled() }}
        run: cargo test -p geom-num --target wasm32-wasip1 --release --locked --no-fail-fast
",
        "        env:
          RUSTFLAGS: -C overflow-checks=on
        run: cargo test $KERNEL_CRATES --target wasm32-wasip1 --release --locked --no-fail-fast
",
    ] {
        assert!(yml.contains(step), "kernel.yml lacks the step:\n{step}");
    }
    // The single-golden steps G02 started with are gone: the store is the one list.
    assert!(!yml.contains("--test determinism -- --nocapture"));
}

/// The matrix runs exactly the store's test-checked goldens, each once, with libtest's exact
/// filters, and a golden checked by its file is left to `golden check`.
#[test]
fn the_plan_covers_every_golden_once_with_exact_filters() {
    let store = Store::embedded().unwrap();
    let groups = plan(&store);
    let edition = store.current().unwrap();
    let planned: Vec<&str> = groups
        .iter()
        .flat_map(|g| g.ids.iter().map(String::as_str))
        .collect();
    let with_check: Vec<&str> = edition
        .goldens
        .iter()
        .filter(|g| g.check.is_some())
        .map(|g| g.id.as_str())
        .collect();
    let a: BTreeSet<&str> = planned.iter().copied().collect();
    let b: BTreeSet<&str> = with_check.iter().copied().collect();
    assert_eq!(a, b);
    assert_eq!(planned.len(), a.len(), "a golden is planned twice");
    let pairs: BTreeSet<(&str, &str)> = groups
        .iter()
        .map(|g| (g.krate.as_str(), g.test.as_str()))
        .collect();
    assert_eq!(pairs.len(), groups.len(), "a test target is run twice");
    for g in &groups {
        let args = g.cargo_args(&["--release".to_owned()]);
        let sep = args.iter().position(|a| a == "--").unwrap();
        assert_eq!(args[sep + 1], "--exact");
        assert_eq!(&args[sep + 3..], g.functions.as_slice());
        assert!(args[..sep].contains(&"--locked".to_owned()));
        assert!(args[..sep].contains(&"--release".to_owned()));
    }
    assert!(
        edition
            .goldens
            .iter()
            .any(|g| g.file.is_some() && !planned.contains(&g.id.as_str()))
    );
}

/// A run proves its group only when libtest ran exactly its functions, all passed, and each golden
/// printed its agreeing line: a filter that matched nothing, a skipped (ignored) golden, an extra
/// test, a crash before the summary or a line for the wrong golden are all refused.
#[test]
fn a_run_that_does_not_prove_its_goldens_is_refused() {
    let store = Store::embedded().unwrap();
    let groups = plan(&store);
    let rng = groups.iter().find(|g| g.test == "rng_determinism").unwrap();
    assert_eq!(rng.functions.len(), 2);
    let line = |id: &str| {
        format!(
            "golden {id} (edition pre-edition): {} agrees with the store\n",
            fake('a')
        )
    };
    let good = format!(
        "{}{}test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out\n",
        line("geom-num/rng-draws"),
        line("geom-num/rng-stream-ranges")
    );
    assert_eq!(verify(rng, &good), Ok(()));
    // On one test thread (wasm32-wasip1 has none to spare) libtest writes `test <name> ... `
    // before each test runs, so each agreeing line follows a test's name on its output line, as
    // the wasm32 legs print it: that run proves its goldens too, and the same run disagreeing
    // does not.
    let one_thread = format!(
        "running 2 tests\ntest rng_golden_hash_is_the_same_on_every_target ... {}geom-num rng \
         corpus: 28040 calls, 770664 bytes\nok\ntest \
         rng_stream_range_golden_hash_is_the_same_on_every_target ... {}ok\n\ntest result: ok. 2 \
         passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.09s\n",
        line("geom-num/rng-draws"),
        line("geom-num/rng-stream-ranges")
    );
    assert_eq!(verify(rng, &one_thread), Ok(()));
    assert!(
        verify(
            rng,
            &one_thread.replace("agrees with the store", "differs from the store's")
        )
        .is_err()
    );
    let bad = [
        good.replace("2 passed", "1 passed"),
        good.replace("2 passed", "3 passed"),
        good.replace("0 ignored", "1 ignored"),
        good.replace("0 failed", "1 failed"),
        good.replace("test result: ok.", "test result: FAILED."),
        good.replace(
            "geom-num/rng-stream-ranges (",
            "geom-num/rng-stream-range (",
        ),
        good.replace("geom-num/rng-draws (", "geom-num/rng-draws-extra ("),
        good.replace("agrees with the store", "differs from the store's"),
        good.lines().take(2).collect::<Vec<_>>().join("\n"),
        String::new(),
    ];
    for (i, out) in bad.iter().enumerate() {
        let r = verify(rng, out);
        assert!(r.is_err(), "case {i} verified:\n{out}");
    }
    assert!(matches!(verify(rng, ""), Err(RunError::NoSummary)));
}

// ---------------------------------------------------------------------------------------------
// 2. A golden store keyed by edition, with the owner-approval rule for edits.
// ---------------------------------------------------------------------------------------------

/// Nothing ambiguous parses: every way to make the real store ambiguous or malformed is refused,
/// including JSON's own ambiguity, a field written twice.
#[test]
fn every_malformed_variant_of_the_real_store_is_refused() {
    let base = store_json();
    let mut cases: Vec<(&str, String)> = Vec::new();
    let mut push = |name: &'static str, f: &dyn Fn(&mut Value)| {
        let mut v = base.clone();
        f(&mut v);
        cases.push((name, v.to_string()));
    };
    push("digest upper case", &|v| {
        let g = &mut goldens(v)[0];
        let up = g["sha256"].as_str().unwrap().to_uppercase();
        g["sha256"] = Value::from(up);
    });
    push("digest 63 digits", &|v| {
        let g = &mut goldens(v)[0];
        let short = g["sha256"].as_str().unwrap()[1..].to_owned();
        g["sha256"] = Value::from(short);
    });
    push("unknown field on a golden", &|v| {
        goldens(v)[0]["approved_by"] = Value::from("someone");
    });
    push("duplicate key", &|v| {
        let g = goldens(v)[1].clone();
        goldens(v).push(g);
    });
    push("check in another crate", &|v| {
        goldens(v)[0]["check"]["crate"] = Value::from("geom-math");
    });
    push("both check and file", &|v| {
        goldens(v)[0]["file"] = Value::from("a/b.txt");
    });
    push("neither check nor file", &|v| {
        goldens(v)[0].as_object_mut().unwrap().remove("check");
    });
    push("history kind unknown", &|v| {
        goldens(v)[0]["history"][0]["kind"] = Value::from("tweak");
    });
    push("history without a piece", &|v| {
        goldens(v)[0]["history"][0]["piece"] = Value::from(" ");
    });
    push("history equal to its successor", &|v| {
        let digest = goldens(v)[0]["sha256"].clone();
        goldens(v)[0]["history"][0]["sha256"] = digest;
    });
    push("negative operation version", &|v| {
        goldens(v)[0]["operations"] = serde_json::json!({"extrude": -1});
    });
    push("open edition frozen", &|v| {
        let editions = v["editions"].as_array_mut().unwrap();
        editions.last_mut().unwrap()["state"] = Value::from("frozen");
    });
    push("a second open edition before it", &|v| {
        let first = v["editions"][0].clone();
        let mut renamed = first;
        renamed["edition"] = Value::from("one");
        v["editions"].as_array_mut().unwrap().insert(0, renamed);
    });
    push("edition label with a capital", &|v| {
        v["editions"][0]["edition"] = Value::from("Pre-edition");
    });
    push("file path escaping the tree", &|v| {
        let g = goldens(v)
            .iter_mut()
            .find(|g| g.get("file").is_some())
            .unwrap();
        g["file"] = Value::from("../outside.txt");
    });
    push("format 2", &|v| v["format"] = Value::from(2));
    for (name, text) in &cases {
        assert!(Store::parse(text).is_err(), "{name} parsed");
    }
    // A field written twice in one object: serde refuses it rather than taking either.
    let twice = EMBEDDED.replacen(
        "\"id\": \"geom-num/determinism\",",
        "\"id\": \"geom-num/determinism\", \"id\": \"geom-num/other\",",
        1,
    );
    assert_ne!(twice, EMBEDDED);
    assert!(matches!(Store::parse(&twice), Err(StoreError::Json(_))));
}

/// The lookup is keyed by edition (the current one), id and operation versions: a golden of an
/// older, frozen edition is not the current one's, and the same id under other operation versions
/// is a different golden.
#[test]
fn lookups_are_keyed_by_the_current_edition_and_operation_versions() {
    let mut v = store_json();
    // Release the current edition, and open a new one whose determinism golden moved.
    let mut released = v["editions"].as_array().unwrap().last().unwrap().clone();
    released["state"] = Value::from("frozen");
    let mut next = released.clone();
    next["edition"] = Value::from("2027");
    next["state"] = Value::from("open");
    let old = released["goldens"][0]["sha256"]
        .as_str()
        .unwrap()
        .to_owned();
    next["goldens"][0]["history"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "sha256": old, "kind": "improvement", "piece": "G999", "why": "a new edition"
        }));
    next["goldens"][0]["sha256"] = Value::from(fake('e'));
    let mut versioned = next["goldens"][1].clone();
    versioned["operations"] = serde_json::json!({"extrude": 2});
    versioned["sha256"] = Value::from(fake('f'));
    next["goldens"].as_array_mut().unwrap().push(versioned);
    v["editions"] = Value::Array(vec![released, next]);
    let store = parse(&v);
    let id = "geom-num/determinism";
    assert_eq!(store.expected(id).unwrap().to_string(), fake('e'));
    let ops: BTreeMap<String, u32> = [("extrude".to_owned(), 2)].into_iter().collect();
    let second = store.current().unwrap().goldens[1].id.clone();
    assert_eq!(
        store.expected_with(&second, &ops).unwrap().to_string(),
        fake('f')
    );
    assert_ne!(store.expected(&second).unwrap().to_string(), fake('f'));
    // The case that gives the old edition's digest fails against the new edition.
    let checked = check_in(&store, id, || (Digest::from_hex(&old).unwrap().0, ()));
    assert!(matches!(
        checked.verdict(),
        Err(GoldenError::Mismatch { ref edition, .. }) if edition == "2027"
    ));
}

/// An edit to the store as JSON.
type Edit = Box<dyn Fn(&mut Value)>;

/// Every edit to the real store that the rule refuses is refused, whatever approval it has.
#[test]
fn the_refusals_hold_on_the_real_store() {
    let base_v = store_json();
    let base = parse(&base_v);
    let classify = |f: &dyn Fn(&mut Value)| {
        let mut v = base_v.clone();
        f(&mut v);
        let head = Store::parse(&v.to_string());
        head.map(|h| diff(Some(&base), &h))
    };
    // Refused outright (or not even a store).
    let refused: Vec<(&str, Edit)> = vec![
        (
            "digest changed without a record",
            Box::new(|v| goldens(v)[1]["sha256"] = Value::from(fake('b'))),
        ),
        (
            "digest changed with a record of another digest",
            Box::new(|v| {
                goldens(v)[1]["sha256"] = Value::from(fake('b'));
                goldens(v)[1]["history"] = serde_json::json!([{
                    "sha256": fake('c'), "kind": "bug", "piece": "G1", "why": "w"}]);
            }),
        ),
        (
            "history rewritten",
            Box::new(|v| goldens(v)[0]["history"][0]["why"] = Value::from("rewritten")),
        ),
        (
            "history removed",
            Box::new(|v| {
                goldens(v)[0].as_object_mut().unwrap().remove("history");
            }),
        ),
        (
            "golden removed",
            Box::new(|v| {
                goldens(v).remove(2);
            }),
        ),
        (
            "golden renamed",
            Box::new(|v| goldens(v)[2]["id"] = Value::from("geom-num/renamed")),
        ),
        (
            "operation versions changed in place",
            Box::new(|v| goldens(v)[2]["operations"] = serde_json::json!({"extrude": 1})),
        ),
        (
            "edition renamed",
            Box::new(|v| v["editions"][0]["edition"] = Value::from("first")),
        ),
    ];
    for (name, f) in &refused {
        // An edit that is not even a valid store is refused before any approval.
        if let Ok(d) = classify(f.as_ref()) {
            assert!(!d.is_acceptable(), "{name} was acceptable:\n{d}");
        }
    }
}

/// The edits to the real store that a kernel owner must approve say so and are not refused; a
/// golden added needs ordinary review only; and once an edition is released, nothing in it changes.
#[test]
fn the_owner_rule_holds_on_the_real_store() {
    let base_v = store_json();
    let base = parse(&base_v);
    let classify = |f: &dyn Fn(&mut Value)| -> Result<_, StoreError> {
        let mut v = base_v.clone();
        f(&mut v);
        Ok(diff(Some(&base), &Store::parse(&v.to_string())?))
    };
    let owner: Vec<(&str, Edit)> = vec![
        (
            "digest changed with its record",
            Box::new(|v| {
                let old = goldens(v)[1]["sha256"].clone();
                goldens(v)[1]["history"] = serde_json::json!([{
                    "sha256": old, "kind": "improvement", "piece": "G999", "why": "w"}]);
                goldens(v)[1]["sha256"] = Value::from(fake('b'));
            }),
        ),
        (
            "check moved",
            Box::new(|v| goldens(v)[1]["check"]["function"] = Value::from("another_test")),
        ),
        (
            "what changed",
            Box::new(|v| goldens(v)[1]["what"] = Value::from("something else")),
        ),
        (
            "pinned file moved",
            Box::new(|v| {
                let g = goldens(v)
                    .iter_mut()
                    .find(|g| g.get("file").is_some())
                    .unwrap();
                g["file"] = Value::from("projects/elsewhere/golden.txt");
            }),
        ),
        (
            "edition released",
            Box::new(|v| {
                let editions = v["editions"].as_array_mut().unwrap();
                let mut next = editions.last().unwrap().clone();
                editions.last_mut().unwrap()["state"] = Value::from("frozen");
                next["edition"] = Value::from("2027");
                editions.push(next);
            }),
        ),
    ];
    for (name, f) in &owner {
        let d = classify(f.as_ref()).unwrap();
        assert!(d.is_acceptable(), "{name} was refused:\n{d}");
        assert!(d.needs_approval(), "{name} needs no approval:\n{d}");
    }
    // Ordinary review: a golden added.
    let d = classify(&|v| {
        let mut g = goldens(v)[1].clone();
        g["id"] = Value::from("geom-num/new-corpus");
        g.as_object_mut().unwrap().remove("history");
        goldens(v).push(g);
    })
    .unwrap();
    assert!(d.is_acceptable() && !d.needs_approval(), "{d}");
    assert_eq!(d.added.len(), 1);
    // And a frozen edition, once released, never changes.
    let mut released_v = base_v.clone();
    {
        let editions = released_v["editions"].as_array_mut().unwrap();
        let mut next = editions.last().unwrap().clone();
        editions.last_mut().unwrap()["state"] = Value::from("frozen");
        next["edition"] = Value::from("2027");
        editions.push(next);
    }
    let released = parse(&released_v);
    let mut edited_frozen = released_v.clone();
    edited_frozen["editions"][0]["goldens"][0]["what"] = Value::from("edited after release");
    let d = diff(Some(&released), &parse(&edited_frozen));
    assert!(!d.is_acceptable(), "{d}");
}

/// The review rule: only a qualified reviewer who is not the author, approving the current head
/// commit, with no qualified reviewer's latest review asking for changes.
#[test]
fn approvals_that_must_not_count_do_not() {
    let review = |login: &str, state: &str, commit: &str, association: &str| {
        serde_json::json!({
            "id": 1, "user": {"login": login}, "body": "", "state": state,
            "commit_id": commit, "author_association": association,
            "submitted_at": "2026-10-25T00:00:00Z"
        })
    };
    let run = |reviews: Vec<Value>, perms: &[(&str, &str)]| {
        let perms: BTreeMap<String, String> = perms
            .iter()
            .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
            .collect();
        approved(&Value::Array(reviews).to_string(), "head", "author", &perms)
    };
    // Counts: an organisation owner or member, or write access, on the head commit.
    for association in ["OWNER", "MEMBER"] {
        assert!(run(vec![review("o", "APPROVED", "head", association)], &[]).is_ok());
    }
    for permission in ["admin", "maintain", "write"] {
        assert!(
            run(
                vec![review("c", "APPROVED", "head", "CONTRIBUTOR")],
                &[("c", permission)]
            )
            .is_ok()
        );
    }
    // Does not count.
    let not_counted = [
        vec![review("o", "APPROVED", "older", "MEMBER")],
        vec![review("author", "APPROVED", "head", "OWNER")],
        vec![review("c", "APPROVED", "head", "CONTRIBUTOR")],
        vec![review("c", "APPROVED", "head", "FIRST_TIME_CONTRIBUTOR")],
        vec![review("c", "APPROVED", "head", "NONE")],
        vec![review("o", "COMMENTED", "head", "MEMBER")],
        vec![review("o", "PENDING", "head", "MEMBER")],
        vec![review("o", "DISMISSED", "head", "MEMBER")],
        vec![
            review("o", "APPROVED", "head", "MEMBER"),
            review("o", "DISMISSED", "head", "MEMBER"),
        ],
        vec![],
    ];
    for (i, reviews) in not_counted.into_iter().enumerate() {
        assert_eq!(
            run(reviews, &[("c", "read")]),
            Err(ApprovalError::NoApproval),
            "case {i}"
        );
    }
    // A qualified reviewer asking for changes blocks another's approval; an outsider's does not.
    assert!(matches!(
        run(
            vec![
                review("o1", "APPROVED", "head", "MEMBER"),
                review("o2", "CHANGES_REQUESTED", "older", "MEMBER"),
            ],
            &[]
        ),
        Err(ApprovalError::ChangesRequested(who)) if who == "o2"
    ));
    assert!(
        run(
            vec![
                review("o1", "APPROVED", "head", "MEMBER"),
                review("x", "CHANGES_REQUESTED", "head", "NONE"),
            ],
            &[]
        )
        .is_ok()
    );
    // A request for changes, then a fresh approval of the head, approves.
    assert!(
        run(
            vec![
                review("o1", "CHANGES_REQUESTED", "older", "MEMBER"),
                review("o1", "APPROVED", "head", "MEMBER"),
            ],
            &[]
        )
        .is_ok()
    );
    // A deleted account's review (no user) is skipped, not a crash.
    let ghost = serde_json::json!([{"user": null, "state": "APPROVED", "commit_id": "head",
        "author_association": "MEMBER"}]);
    assert_eq!(
        approved(&ghost.to_string(), "head", "author", &BTreeMap::new()),
        Err(ApprovalError::NoApproval)
    );
}

/// The store's workflow holds pull requests to the rule: it runs on every review as well as every
/// pull request, reads the reviews with a read-only token, runs `golden check` and `golden diff`
/// with the reviews, the head commit and the author, and pins the same toolchain.
#[test]
fn kernel_golden_yml_runs_the_check_and_the_approval_rule() {
    let yml = GOLDEN_YML.replace("\r\n", "\n");
    for needle in [
        "  pull_request_review:\n    types: [submitted, edited, dismissed]\n",
        "permissions:\n  contents: read\n  pull-requests: read\n",
        "run: cargo run --locked -p geom-test --bin golden -- check\n",
        "cargo run --locked -p geom-test --bin golden -- diff\n",
        "--base \"$BASE\"\n",
        "BASE: ${{ steps.edits.outputs.base }}\n",
        "BASE=$(git rev-parse HEAD^1)\n",
        "--reviews \"$RUNNER_TEMP/reviews.json\"\n",
        "--permissions \"$RUNNER_TEMP/permissions.json\"\n",
        "--head \"$HEAD_SHA\"\n",
        "--author \"$AUTHOR\"\n",
        "HEAD_SHA: ${{ github.event.pull_request.head.sha }}\n",
        "AUTHOR: ${{ github.event.pull_request.user.login }}\n",
        "fetch-depth: 0\n",
        "persist-credentials: false\n",
    ] {
        assert!(yml.contains(needle), "kernel-golden.yml lacks {needle:?}");
    }
    assert!(
        !yml.contains(": write") && !yml.contains("write-all"),
        "the workflow's token must only read"
    );
    assert!(!yml.contains("continue-on-error"));
    assert!(!yml.contains("pull_request_target"));
}

// ---------------------------------------------------------------------------------------------
// 3. The perturbing allocator, the in-process repeat and the input-shuffle hook.
// ---------------------------------------------------------------------------------------------

/// Code that orders by address sees different orders under different perturbations: on whatever
/// allocator this target has, sixteen seeds give at least two orders of 64 boxes, and a golden
/// whose case orders by address fails as a repeat.
#[test]
fn the_perturbation_changes_what_address_ordered_code_sees() {
    let mut seen = BTreeSet::new();
    for seed in 0..16u64 {
        let heap = Heap::shift_and_fragment(Seed(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15)));
        seen.insert(address_order_digest(64));
        drop(heap);
    }
    assert!(seen.len() >= 2, "every perturbation left the order alone");
    let store = Store::parse(&format!(
        r#"{{"format": 1, "about": "t", "editions": [{{"edition": "pre-edition", "state": "open",
        "goldens": [{{"id": "geom-x/by-address", "what": "w", "sha256": "{}",
        "check": {{"crate": "geom-x", "test": "t", "function": "f"}}}}]}}]}}"#,
        fake('0')
    ))
    .unwrap();
    // Address-ordered code over many boxes: in some round, the two perturbed runs disagree.
    let caught = (0..8).any(|_| {
        let c = check_in(&store, "geom-x/by-address", || {
            (address_order_digest(256), ())
        });
        matches!(c.verdict(), Err(GoldenError::Repeat { .. }))
    });
    assert!(caught, "address-ordered code passed the perturbed repeat");
}

/// The perturbation is bounded and seeded: the same seed makes the same requests, every block is
/// either freed (a hole) or pinned, and no seed asks for more than its bound.
#[test]
fn the_perturbation_is_seeded_and_bounded() {
    for seed in [0u64, 1, 2, u64::MAX, 0x5045_5254_5552_4231] {
        let a = Heap::shift_and_fragment(Seed(seed));
        let b = Heap::shift_and_fragment(Seed(seed));
        assert_eq!(a.counts(), b.counts(), "seed {seed:#x}");
        let (allocated, freed, pinned, bytes) = a.counts();
        assert_eq!(allocated, freed + pinned);
        assert!(freed >= 52 * 2, "seed {seed:#x} freed {freed}");
        assert!(bytes <= MAX_BYTES);
        assert_eq!(a.seed(), Seed(seed));
    }
}

/// The repeat catches state left over from an earlier run (a hidden cache) and reports it before
/// any store comparison; a case that matches the store and repeats passes; the store is never
/// written (the embedded text is unchanged by a failing check).
#[test]
fn the_repeat_catches_hidden_state_and_never_writes_the_store() {
    const ID: &str = "geom-test/shuffle-permutations";
    let store = Store::embedded().unwrap();
    let id = ID;
    let want = store.expected(id).unwrap();
    // A hidden cache: the second run sees the first run's state.
    let cache = Cell::new(None::<[u8; 32]>);
    let c = check_in(&store, id, || {
        let d = cache.get().map_or(want.0, |prev| sha256(&prev));
        cache.set(Some(d));
        (d, ())
    });
    assert!(matches!(
        c.verdict(),
        Err(GoldenError::Repeat { run: 2, .. })
    ));
    // A case that differs from the store in both runs is a mismatch, not a repeat.
    let c = check_in(&store, id, || (sha256(b"other"), ()));
    assert!(matches!(c.verdict(), Err(GoldenError::Mismatch { .. })));
    // The real case passes, through the embedded store.
    let c = golden::check(ID, || {
        let mut o = Orders::new(Seed::DEFAULT);
        let mut h = Sha256::new();
        for n in 0..64usize {
            for i in o.permutation(n) {
                h.write(&(i as u64).to_le_bytes());
            }
        }
        (h.finish(), ())
    });
    assert_eq!(c.verdict(), Ok(()));
    assert_eq!(Store::embedded().unwrap(), store);
}

/// The shuffle hook runs the given order first and then its reverse, and fails an order-dependent
/// case naming the order that broke it; every permutation of four inputs comes about equally often.
#[test]
fn the_shuffle_hook_meets_every_order() {
    let mut o = Orders::new(Seed(19));
    let mut counts: BTreeMap<Vec<usize>, u32> = BTreeMap::new();
    for _ in 0..24_000 {
        *counts.entry(o.permutation(4)).or_default() += 1;
    }
    assert_eq!(counts.len(), 24);
    for (p, n) in &counts {
        // Expected 1,000 each, standard deviation about 31: far outside these bounds is a bias.
        assert!((850..=1150).contains(n), "{p:?} came {n} times");
    }
    // A case that depends on the position of one input, other than the first or last, is caught
    // by the seeded orders even though the reverse leaves the middle alone.
    let inputs: Vec<u32> = (0..5).collect();
    let middle = |xs: &[u32]| sha256(&xs[2].to_le_bytes());
    let e = order_free("middle", &inputs, Seed(3), 8, middle).unwrap_err();
    assert_ne!(e.permutation[2], 2);
    assert_eq!(e.given, Digest(sha256(&2u32.to_le_bytes())));
    // A symmetric case passes and returns the given order's digest.
    let sum = |xs: &[u32]| sha256(&xs.iter().sum::<u32>().to_le_bytes());
    assert_eq!(
        order_free("sum", &inputs, Seed(3), 8, sum),
        Ok(sha256(&10u32.to_le_bytes()))
    );
}

// ---------------------------------------------------------------------------------------------
// 4. The repository check, the binary and git, on the real tree (natively only).
// ---------------------------------------------------------------------------------------------

#[cfg(not(target_family = "wasm"))]
mod native {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use geom_test::golden::repo::{Scanned, Source, kernel_sources, problems_of, scan_source};
    use geom_test::golden::store::{STORE_PATH, Store};
    use geom_test::map::repo::workspace_root;

    fn root() -> PathBuf {
        workspace_root().unwrap()
    }

    fn read_under(root: &Path) -> impl Fn(&str) -> Result<String, String> + '_ {
        move |rel: &str| {
            let p = rel.split('/').fold(root.to_path_buf(), |p, c| p.join(c));
            std::fs::read_to_string(p).map_err(|e| e.to_string())
        }
    }

    /// The real tree agrees with the store, as the `golden check` command CI runs reports it.
    #[test]
    fn the_real_tree_agrees_with_the_store() {
        let out = golden_bin().arg("check").output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(text.contains("each checked where the store says"), "{text}");
    }

    /// Seeded violations of the real tree, each made in memory on the real sources, are found:
    /// the determinism check commented out, the check moved to a helper, a digest constant put
    /// back beside the store, a check of a golden the store lacks, an id that is not literal, and
    /// the store's check imported by itself.
    #[test]
    fn seeded_violations_of_the_real_tree_are_found() {
        let root = root();
        let store = Store::embedded().unwrap();
        let sources = kernel_sources(&root).unwrap();
        // Every source is scanned once; each mutation rescans only the file it changes.
        let scanned: Vec<Scanned> = sources.iter().map(scan_source).collect();
        let read = read_under(&root);
        let baseline = problems_of(&store, &scanned, &read);
        assert!(baseline.is_empty(), "{baseline:#?}");
        let det = "projects/engines/geometry/crates/geom-num/tests/determinism.rs";
        let mutate = |path: &str, f: &dyn Fn(&str) -> String| -> Vec<Scanned> {
            let original = sources.iter().find(|s| s.path == path).unwrap();
            let changed = f(&original.text);
            assert_ne!(
                changed, original.text,
                "the mutation of {path} changed nothing"
            );
            let mut out = scanned.clone();
            let at = out.iter().position(|s| s.path == path).unwrap();
            out[at] = scan_source(&Source {
                path: path.to_owned(),
                text: changed,
            });
            out
        };
        let call = "let checked = golden::check(GOLDEN, || {";
        let cases: Vec<(&str, Vec<Scanned>)> = vec![
            (
                "check commented out",
                mutate(det, &|t| {
                    t.replace(
                        call,
                        "let checked = /* golden::check(GOLDEN, */ helper(|| {",
                    )
                }),
            ),
            (
                "check moved to a helper",
                mutate(det, &|t| {
                    t.replace(call, "let checked = helper(|| {")
                        + "\nfn helper<T>(f: impl FnMut() -> ([u8; 32], T)) -> golden::Checked<T> \
                           { golden::check(GOLDEN, f) }\n"
                }),
            ),
            (
                "a digest beside the store",
                mutate(det, &|t| {
                    format!(
                        "{t}\nconst OLD_GOLDEN: &str = \
                         \"fb19edfc7c58ca5e93a8921dfbdd2ab2dfacc2d180fbe6dec540efbede954564\";\n"
                    )
                }),
            ),
            (
                "an unknown golden",
                mutate(det, &|t| {
                    t.replace(
                        "const GOLDEN: &str = \"geom-num/determinism\";",
                        "const GOLDEN: &str = \"geom-num/determinism-v2\";",
                    )
                }),
            ),
            (
                "an id that is not literal",
                mutate(det, &|t| {
                    t.replace(call, "let checked = golden::check(&id(), || {")
                }),
            ),
            (
                "the check imported by itself",
                mutate(det, &|t| {
                    t.replace(
                        "use geom_test::golden::{self, Sha256, hex};",
                        "use geom_test::golden::{self, Sha256, check, hex};",
                    )
                }),
            ),
        ];
        for (name, mutated) in cases {
            let found = problems_of(&store, &mutated, &read);
            assert!(!found.is_empty(), "{name}: no problem found");
        }
        // A pinned golden file edited without the store.
        let edited = |rel: &str| read(rel).map(|t| t.replacen('a', "b", 1));
        let found = problems_of(&store, &scanned, &edited);
        assert!(
            found
                .iter()
                .any(|p| p.file.ends_with("geom-bench/src/golden.txt")),
            "{found:#?}"
        );
    }

    /// No kernel crate's tests carry their own SHA-256 any more: the round constants of FIPS
    /// 180-4's compression function appear in no Rust source of the kernel.
    #[test]
    fn no_hand_written_sha256_is_left() {
        let sources = kernel_sources(&root()).unwrap();
        for s in &sources {
            // Built here, so this file does not carry them itself: the first round constant
            // and the first word of the initial hash value, with and without separators.
            let constants = [
                format!("0x{}_{}", "428a", "2f98"),
                format!("0x{}{}", "428a", "2f98"),
                format!("0x{}_{}", "6a09", "e667"),
                format!("0x{}{}", "6a09", "e667"),
            ];
            for constant in &constants {
                assert!(
                    !s.text.contains(constant.as_str()),
                    "{} carries a SHA-256 constant ({constant})",
                    s.path
                );
            }
        }
    }

    fn golden_bin() -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_golden"));
        c.current_dir(root());
        c
    }

    /// The binary's commands answer with the exit codes its header promises.
    #[test]
    fn the_binary_lists_checks_and_refuses_bad_usage() {
        let out = golden_bin().arg("list").output().unwrap();
        assert!(out.status.success());
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(text.contains("geom-num/determinism"));
        for bad in [
            vec!["nonsense"],
            vec![],
            vec!["check", "--rooot", "x"],
            vec!["diff"],
        ] {
            let out = golden_bin().args(&bad).output().unwrap();
            assert_eq!(out.status.code(), Some(2), "{bad:?}");
        }
    }

    /// `golden diff` end to end in a scratch repository: an unchanged store passes, an added golden
    /// passes, a recorded change fails without an owner's approval and passes with one on the head
    /// commit, and an unrecorded change fails whatever the reviews say.
    #[test]
    fn golden_diff_holds_a_change_to_the_owner_rule_end_to_end() {
        let dir = std::env::temp_dir().join(format!("geom-test-attack-g19-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store_path = STORE_PATH.iter().fold(dir.clone(), |p, c| p.join(c));
        std::fs::create_dir_all(store_path.parent().unwrap()).unwrap();
        let base_text = geom_test::golden::store::EMBEDDED.replace("\r\n", "\n");
        std::fs::write(&store_path, &base_text).unwrap();
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
                .args(args)
                .current_dir(&dir)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8(out.stdout).unwrap()
        };
        git(&["init", "-q"]);
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "base"]);
        let base = git(&["rev-parse", "HEAD"]).trim().to_owned();
        let diff_cmd = |extra: &[&str]| {
            let mut c = Command::new(env!("CARGO_BIN_EXE_golden"));
            c.args(["diff", "--base", &base, "--root"])
                .arg(&dir)
                .args(extra);
            c.output().unwrap()
        };
        assert!(diff_cmd(&[]).status.success(), "unchanged store");
        // A recorded change to the first golden.
        let mut v: serde_json::Value = serde_json::from_str(&base_text).unwrap();
        let first = &mut super::goldens(&mut v)[1];
        let old = first["sha256"].clone();
        first["history"] = serde_json::json!([{
            "sha256": old, "kind": "improvement", "piece": "G999", "why": "attack"}]);
        first["sha256"] = serde_json::Value::from(super::fake('b'));
        std::fs::write(&store_path, serde_json::to_string_pretty(&v).unwrap()).unwrap();
        let out = diff_cmd(&[]);
        assert_eq!(
            out.status.code(),
            Some(1),
            "a change without reviews passed"
        );
        let reviews = dir.join("reviews.json");
        let write_reviews = |login: &str, commit: &str, association: &str| {
            let r = serde_json::json!([{"user": {"login": login}, "state": "APPROVED",
                "commit_id": commit, "author_association": association}]);
            std::fs::write(&reviews, r.to_string()).unwrap();
        };
        let with_reviews = |head: &str| {
            let r = reviews.to_str().unwrap().to_owned();
            diff_cmd(&["--reviews", &r, "--head", head, "--author", "author"])
        };
        write_reviews("owner", "headsha", "MEMBER");
        assert!(
            with_reviews("headsha").status.success(),
            "an owner's approval did not pass"
        );
        assert_eq!(
            with_reviews("newersha").status.code(),
            Some(1),
            "a stale approval passed"
        );
        write_reviews("author", "headsha", "OWNER");
        assert_eq!(
            with_reviews("headsha").status.code(),
            Some(1),
            "self-approval passed"
        );
        // An unrecorded change is refused whatever the reviews say.
        let mut v: serde_json::Value = serde_json::from_str(&base_text).unwrap();
        super::goldens(&mut v)[1]["sha256"] = serde_json::Value::from(super::fake('b'));
        std::fs::write(&store_path, serde_json::to_string_pretty(&v).unwrap()).unwrap();
        write_reviews("owner", "headsha", "MEMBER");
        assert_eq!(with_reviews("headsha").status.code(), Some(1));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// `hex` and `Digest` agree, so a digest printed by one golden reads back as the store's.
#[test]
fn hex_and_digest_agree() {
    let d = sha256(b"G19");
    assert_eq!(Digest::from_hex(&hex(d)), Some(Digest(d)));
    assert_eq!(Digest(d).to_string(), hex(d));
}
