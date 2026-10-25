//! Adversarial tests of piece G12, the kernel's merge gate (chapter 28.8): they try to break the
//! gate's checks in `tests/scaffold` (`deny.rs`, `gate.rs`, `globals.rs`, `posture.rs`) and the
//! two cargo-deny configurations, not to confirm them. Each test fails while the gate lets the
//! seeded violation through.
//!
//! What is attacked, against the board text, chapters 21, 27, 28.8 and 29, and G01's principles:
//!
//! - The licence gate (29.2; 29.20, rule 3: every crate "whether a normal, build or
//!   dev-dependency, must pass the licence gate"): `deny.toml` sets `private = { ignore = true }`,
//!   and cargo-deny 0.20.2 skips the licence of every crate whose manifest says `publish = false`
//!   (`Krate::is_private` in `src/lib.rs` reads `publish` only, not workspace membership), so a
//!   kernel crate of our own under GPL, a path crate outside the workspace under AGPL and a
//!   published crate whose manifest says `publish = false` all pass.
//! - "libm the only registry crate in kernel crates": a `libm` patched in from a local path is
//!   allowed by name and checked by no source rule.
//! - Chapter 21's and 27's lint rules, end to end with clippy 1.99.0 and `geom-num`'s own
//!   settings: a lint group allowed at a higher priority in `[lints.clippy]` (which the posture
//!   check does not read) and an `#![allow(...)]` in a kernel crate's source both switch every
//!   rule off; code behind a non-default feature is never linted nor built; `Sum::sum` and
//!   `Product::product` called as trait functions escape the `Iterator::sum` ban; and the panic
//!   macros `unreachable!`, `todo!` and `unimplemented!` pass (principle 13: "no panics in kernel
//!   paths"; 28.1: checked by lints).
//! - The ban on global state (principle 7, 28.8): statics compiled into a crate's library from a
//!   file the scan never reads (`#[path]` into `tests/`, `include!` of a non-`.rs` file, code a
//!   build script writes) pass.
//! - The workflow check (`gate::workflow`): a gate job that cannot fail (`continue-on-error`,
//!   `if: false`, `|| true`), test filters that select nothing, and the rules run moved into a
//!   comment all pass, so the required "Merge gate" check can be green while nothing ran.
//!
//! Oracles: cargo-deny 0.20.2 and clippy 1.99.0 themselves, run on graphs and probe crates built
//! here with the gate's own functions (`deny::run::Graph`, `gate::probe`, `gate::clippy`,
//! `gate::build`, `gate::reference_posture`); cargo-deny's source at commit
//! bca0dde53651ee946720e4540b5ce2610bec8f06 (MIT OR Apache-2.0, read for its behaviour only;
//! nothing copied); rustc 1.99.0 building the global-state probe.
//!
//! The tests start cargo, clippy and cargo-deny, so they run natively only. Those that run
//! cargo-deny need it installed, as the gate's own `gate_deny` tests do, and fetch the advisory
//! database first.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// The kernel lints deny native arithmetic in kernel code (Cargo.toml). This is test code.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "test code; the rule is for kernel code"
)]
#![cfg(not(target_family = "wasm"))]

// The scaffold check's own modules, compiled again here so the gate's functions can be attacked
// directly.
#[allow(dead_code)]
#[path = "scaffold/check.rs"]
mod check;
#[allow(dead_code)]
#[path = "scaffold/deny.rs"]
mod deny;
#[allow(dead_code)]
#[path = "scaffold/gate.rs"]
mod gate;
#[allow(dead_code)]
#[path = "scaffold/globals.rs"]
mod globals;
#[allow(dead_code)]
#[path = "scaffold/header.rs"]
mod header;
#[allow(dead_code)]
#[path = "scaffold/json.rs"]
mod json;
// The fixer's modules: the record of licence choices, and the rule on lint attributes.
#[allow(dead_code)]
#[path = "scaffold/licence.rs"]
mod licence;
#[allow(dead_code)]
#[path = "scaffold/lints.rs"]
mod lints;
#[allow(dead_code)]
#[path = "scaffold/manifest.rs"]
mod manifest;
#[allow(dead_code)]
#[path = "scaffold/overrides.rs"]
mod overrides;
#[allow(dead_code)]
#[path = "scaffold/posture.rs"]
mod posture;
// The recursion gate's runs (piece G13), so gate.rs's check that every gate test has its job
// holds in this binary too.
#[allow(dead_code)]
#[path = "scaffold/recursion.rs"]
mod recursion;
#[allow(dead_code)]
#[path = "scaffold/repo.rs"]
mod repo;
#[allow(dead_code)]
#[path = "scaffold/source.rs"]
mod source;
#[allow(dead_code)]
#[path = "scaffold/table.rs"]
mod table;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use deny::run::{BANS, Edge, Graph, LICENSES, SOURCES, Seed, published};
use deny::{FAMILY_CONFIG, NO_STD_CONFIG};
use json::Json;

/// What `gate.rs`'s own tests call to show violations (the scaffold's `main.rs` defines it).
fn report(violations: &[check::Violation]) -> String {
    let mut text = format!("{} violation(s):", violations.len());
    for v in violations {
        text.push_str("\n  ");
        text.push_str(&v.to_string());
    }
    text
}

/// A folder of this test's own under the test's temporary folder, created.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("attack-g12")
        .join(name);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// The text of a run, for a message.
fn shown(output: &Output) -> String {
    format!(
        "{}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    )
}

// ------------------------------------------------------------------ cargo-deny

/// The package named `name` of a `cargo metadata` graph, to change.
fn package_mut<'a>(json: &'a mut Json, name: &str) -> &'a mut Json {
    json.get_mut("packages")
        .and_then(Json::as_array_mut)
        .unwrap()
        .iter_mut()
        .find(|p| p.get("name").and_then(Json::as_str) == Some(name))
        .unwrap_or_else(|| panic!("no package {name} in the graph"))
}

/// Sets the existing member `key` of an object.
fn set(object: &mut Json, key: &str, value: Json) {
    *object
        .get_mut(key)
        .unwrap_or_else(|| panic!("no member {key}")) = value;
}

/// The licence gate skips every crate whose manifest says `publish = false`.
///
/// `deny.toml` sets `private = { ignore = true }` so that the facade, which has no licence field,
/// passes. cargo-deny 0.20.2 applies it to any crate in the graph whose `publish` is an empty
/// list (`Krate::is_private`, `src/lib.rs`; `src/licenses.rs` skips the crate with
/// `SkippedPrivateWorkspaceCrate`), whether or not it is a workspace member. `cargo metadata`
/// writes `publish = false` as `"publish": []`, which the gate's own seeds never do (they write
/// `null`), so its licence tests never meet the rule. Seeded here, through the gate's own
/// `Graph`, into `geom-num`'s graph:
///
/// - `geom-num` itself, its manifest saying `license = "GPL-3.0-only"` (29.20, rule 4: a crate's
///   licence field names the licences of its ported files, so it is what the gate must read);
/// - a path crate outside the workspace, `publish = false`, under AGPL, as a dev-dependency (an
///   oracle linked in, against 29.20 rule 8);
/// - a crates.io crate whose packaged manifest says `publish = false`, under GPL.
///
/// Each breaks 29.2's allow-list; each must be refused.
#[test]
fn attack_licence_gate_skips_every_crate_marked_publish_false() {
    deny::run::fetch(&repo::workspace_root());
    let mut graph = Graph::of("geom-num");
    graph
        .add(
            "geom-num",
            Edge::Dev,
            "=0.1.0",
            Seed {
                name: "attack-path-agpl",
                version: "0.1.0",
                source: None,
                license: Some("AGPL-3.0-only"),
            },
        )
        .add(
            "geom-num",
            Edge::Normal,
            "=1.0.0",
            published("attack-private-gpl", "1.0.0", "GPL-3.0-only"),
        );
    set(
        package_mut(&mut graph.json, "geom-num"),
        "license",
        Json::Str("GPL-3.0-only".to_owned()),
    );
    let seeded = ["geom-num", "attack-path-agpl", "attack-private-gpl"];
    // Control: the same three with `publish` unset are refused, so the seeds are sound and the
    // only difference below is `publish = false`.
    for name in seeded {
        set(package_mut(&mut graph.json, name), "publish", Json::Null);
    }
    let control = graph.deny("attack-g12-private-control", FAMILY_CONFIG, &["licenses"]);
    assert!(
        control.failed(LICENSES) && seeded.iter().all(|k| control.refused().contains(*k)),
        "control: the seeds without publish = false must be refused:\n{}",
        control.show()
    );
    for name in seeded {
        set(
            package_mut(&mut graph.json, name),
            "publish",
            Json::Array(Vec::new()),
        );
    }
    let outcome = graph.deny("attack-g12-private", FAMILY_CONFIG, &["licenses"]);
    let refused = outcome.refused();
    let missed: Vec<&str> = seeded
        .into_iter()
        .filter(|k| !refused.contains(*k))
        .collect();
    assert!(
        outcome.failed(LICENSES) && missed.is_empty(),
        "deny.toml's licence check passed {missed:?}, each under a licence 29.2 leaves out, \
         because each manifest says publish = false and deny.toml sets private.ignore = true:\n{}",
        outcome.show()
    );
}

/// A `libm` patched in from a local path passes as "the only registry crate".
///
/// `deny-no-std.toml` allows `libm` by name, and no source rule reads a path crate, so a
/// `[patch.crates-io] libm = { path = "..." }` (any code at all, named libm, never the audited
/// crates.io release 29.4 pins) passes both configurations. The seed changes only what such a
/// patch changes in `cargo metadata`: the resolved package's id and source; the declaration in
/// `geom-num`'s manifest still names crates.io. One of the gate's rules must refuse it.
#[test]
fn attack_libm_from_a_path_passes_as_the_registry_crate() {
    deny::run::fetch(&repo::workspace_root());
    let mut graph = Graph::of("geom-num");
    let libm = package_mut(&mut graph.json, "libm");
    let old_id = libm.get("id").and_then(Json::as_str).unwrap().to_owned();
    let version = libm
        .get("version")
        .and_then(Json::as_str)
        .unwrap()
        .to_owned();
    assert!(
        libm.get("source")
            .and_then(Json::as_str)
            .is_some_and(|s| s.contains("crates.io-index")),
        "geom-num's libm does not come from crates.io today"
    );
    let new_id = format!("path+file:///attack/vendor/libm#libm@{version}");
    let text = graph.json.to_text().replace(&old_id, &new_id);
    graph.json = json::parse(&text).unwrap();
    set(package_mut(&mut graph.json, "libm"), "source", Json::Null);
    let no_std = graph.deny("attack-g12-libm-path-no-std", NO_STD_CONFIG, &["bans"]);
    let family = graph.deny(
        "attack-g12-libm-path-family",
        FAMILY_CONFIG,
        &["bans", "sources"],
    );
    assert!(
        no_std.failed(BANS) || family.failed(SOURCES) || family.failed(BANS),
        "a libm from a local path passed deny-no-std.toml and deny.toml's sources:\n{}\n\n{}",
        no_std.show(),
        family.show()
    );
}

// ------------------------------------------------------------------ clippy

/// The clippy probe's library from `lines`, after a header whose last line is `attributes`, and
/// the line number of the first of `lines`.
fn probe_library(attributes: &str, lines: &[String]) -> (String, usize) {
    let mut text =
        format!("//! Attack probe of the merge gate.\n#![allow(dead_code)]\n{attributes}\n");
    let first = text.lines().count() + 1;
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
    (text, first)
}

/// Runs the gate's clippy (`gate::clippy`) on a probe crate `name` built with `geom-num`'s own
/// lint settings, `manifest_tail` changed by `change`, and returns the run and the lines of
/// `src/lib.rs` given an error.
fn gate_clippy_on(
    name: &str,
    change: impl Fn(&str) -> String,
    library: &str,
) -> (Output, BTreeSet<usize>) {
    let (manifest_lints, clippy_toml) = gate::reference_posture(&table::TABLE);
    let manifest = gate::probe(name, &change(&manifest_lints), library, Some(&clippy_toml));
    let output = gate::clippy(&manifest, &[], false, &scratch("clippy-target"));
    let lines = gate::messages(&output)
        .into_iter()
        .filter(|m| m.level == "error" && m.file.ends_with("src/lib.rs"))
        .map(|m| m.line)
        .collect();
    (output, lines)
}

/// The gate's own seeded lines (`gate::probe_lines`) that must each give an error. (The fixer
/// gave each probe line a list of lints in place of one, and put one line behind a feature, which
/// the probes here do not declare; this reads the lines that break a rule on their own.)
fn seeded_lines() -> Vec<String> {
    gate::probe_lines()
        .into_iter()
        .filter(|l| !l.lints.is_empty() && !l.source.contains("feature ="))
        .map(|l| l.source)
        .collect()
}

/// Every one of `lines`, from line `first` on, that clippy gave no error.
fn passed<'a>(lines: &'a [String], first: usize, refused: &BTreeSet<usize>) -> Vec<&'a str> {
    lines
        .iter()
        .enumerate()
        .filter(|(i, _)| !refused.contains(&(first + i)))
        .map(|(_, l)| l.as_str())
        .collect()
}

/// A lint group allowed at a higher priority switches chapter 21's and 27's rules off, and the
/// posture check does not see it.
///
/// `posture::lacks` reads each lint's own level (`unwrap_used = "deny"` and so on) and ignores
/// the groups. Cargo passes lints in priority order, so `restriction` (which holds `unwrap_used`,
/// `expect_used`, `panic` and `indexing_slicing`) and `style` (which holds `disallowed_methods`
/// and `disallowed_types`) allowed at priority 1 come after the denials and win. Added to every
/// crate that ships (so `lints`' identical-copies rule still holds), the posture check passes;
/// clippy 1.99.0 with those settings passes every one of the gate's own seeded lines.
#[test]
fn attack_a_group_allowed_at_higher_priority_switches_the_rules_off() {
    let overrides = "restriction = { level = \"allow\", priority = 1 }\n\
                     style = { level = \"allow\", priority = 1 }\n";
    // The manifest as the posture check reads it, with the two groups in [lints.clippy].
    let observed = repo::observe();
    let mut lints = observed.packages["geom-num"].lints.clone();
    let tables = lints.tables.as_mut().unwrap();
    for group in ["restriction", "style"] {
        tables.insert(
            format!("lints.clippy.{group}.level"),
            "\"allow\"".to_owned(),
        );
        tables.insert(format!("lints.clippy.{group}.priority"), "1".to_owned());
    }
    let lacks = posture::lacks(&lints);
    // The same settings, end to end.
    let seeded = seeded_lines();
    let (library, first) = probe_library("", &seeded);
    let (output, refused) = gate_clippy_on(
        "attack-g12-priority",
        |tail| {
            assert!(tail.contains("[lints.clippy]\n"), "{tail}");
            tail.replacen(
                "[lints.clippy]\n",
                &format!("[lints.clippy]\n{overrides}"),
                1,
            )
        },
        &library,
    );
    let through = passed(&seeded, first, &refused);
    assert!(
        through.is_empty() || !lacks.is_empty(),
        "with `{}` in [lints.clippy], posture::lacks reports nothing, and clippy ({}) passes \
         {} of the gate's {} seeded lines, among them {:?}",
        overrides.replace('\n', " "),
        output.status,
        through.len(),
        seeded.len(),
        &through[..through.len().min(4)]
    );
}

/// An `#![allow(...)]` in a kernel crate's source switches chapter 21's and 27's rules off, and
/// no check of the gate reads lint attributes in source.
///
/// Chapter 27: "`clippy` denies `unwrap`, `expect`, `panic` and unchecked indexing in kernel
/// code". The posture is `deny`, which an attribute in the code overrides; `lints.rs` leaves such
/// attributes to "code review" and `posture.rs` does not read sources. No kernel source allows
/// any of the six lints today, so a rule against it costs nothing. Here the gate's clippy, with
/// `geom-num`'s settings, and the gate's source scans (`globals::check`, and `overrides::check`,
/// the rule the fixer added for exactly this, both reading a crate's sources as the gate does)
/// both pass a kernel source that allows them all.
#[test]
fn attack_an_allow_in_kernel_source_switches_the_rules_off() {
    let allow = "#![allow(clippy::disallowed_methods, clippy::disallowed_types, \
                 clippy::unwrap_used, clippy::expect_used, clippy::panic, \
                 clippy::indexing_slicing)]";
    let lines = seeded_lines();
    let (library, first) = probe_library(allow, &lines);
    let (output, refused) = gate_clippy_on("attack-g12-allow", str::to_owned, &library);
    let through = passed(&lines, first, &refused);
    // The same source seeded into geom-op, through the gate's source reader's check.
    let observed = repo::observe();
    let mut sources = repo::crate_sources(&observed, &BTreeSet::from(["geom-op"]));
    let mut path = sources["geom-op"][0].path.clone();
    path.pop();
    path.push("allowed.rs".to_owned());
    sources
        .get_mut("geom-op")
        .unwrap()
        .push(globals::SourceFile::read(path, &library));
    let mut found = globals::check(&table::TABLE, &sources);
    found.extend(overrides::check(&table::TABLE, &sources));
    assert!(
        through.is_empty() || !found.is_empty(),
        "a kernel source that starts `{allow}` passes the gate: clippy ({}) passes {} of the \
         gate's {} seeded lines, and no source check reports it",
        output.status,
        through.len(),
        lines.len()
    );
}

/// Code behind a feature is never linted nor built by the gate.
///
/// cargo-deny's graph is read with every feature on (`graph.all-features = true`, "a feature's
/// dependencies are dependencies"), but the gate's clippy run (`gate::CLIPPY_ARGS`) and its
/// builds (`gate::build`) use default features only. The facade's `threads` feature (chapter 27)
/// is such code, and nothing stops a kernel crate from declaring a feature. A probe with
/// `geom-num`'s settings: `f64::max` and `unwrap` behind a non-default feature pass the gate's
/// clippy, and a 64-bit assumption behind it passes the gate's wasm32 build.
#[test]
fn attack_code_behind_a_feature_is_never_linted_or_built() {
    let lines = [
        "#[cfg(feature = \"extra\")] fn m(x: f64, y: f64) -> f64 { x.max(y) }".to_owned(),
        "#[cfg(feature = \"extra\")] fn u(x: Option<u8>) -> u8 { x.unwrap() }".to_owned(),
    ];
    let (library, first) = probe_library("", &lines);
    let (output, refused) = gate_clippy_on(
        "attack-g12-feature",
        |tail| format!("[features]\nextra = []\n\n{tail}"),
        &library,
    );
    let through = passed(&lines, first, &refused);
    let wasm = gate::probe(
        "attack-g12-feature-build",
        "[features]\nextra = []\n",
        "//! Probe.\n#![no_std]\n#[cfg(feature = \"extra\")]\n\
         const _: () = assert!(core::mem::size_of::<usize>() == 8);\n",
        None,
    );
    let build = gate::build(
        &wasm,
        None,
        Some(gate::WASM_TARGET),
        false,
        &scratch("build-target"),
    );
    assert!(
        through.is_empty() && !build.status.success(),
        "behind a non-default feature, clippy ({}) passes {through:?}, and the wasm32 build of a \
         crate that asserts a 64-bit usize there {}:\n{}",
        output.status,
        if build.status.success() {
            "passes"
        } else {
            "fails"
        },
        shown(&build)
    );
}

/// `Iterator::sum`'s ban misses the same reduction called as a trait function.
///
/// Chapter 21: "`clippy` disallowed-methods bans `Iterator::sum` and `Iterator::product` for
/// every element type ... so a sum is written in a stated order or goes through `ExactSum`".
/// `f64::sum(v.iter())` and `<u64 as Product<&u64>>::product(v.iter())` are `core::iter::Sum::sum`
/// and `core::iter::Product::product`, the very functions `Iterator::sum` calls, with the order
/// just as unstated; clippy 1.99.0 with `geom-num`'s settings passes both.
#[test]
fn attack_sum_and_product_called_as_trait_functions_pass() {
    let lines = [
        "fn s(v: &[f64]) -> f64 { <f64 as core::iter::Sum<&f64>>::sum(v.iter()) }".to_owned(),
        "fn p(v: &[u64]) -> u64 { <u64 as core::iter::Product<&u64>>::product(v.iter()) }"
            .to_owned(),
    ];
    let (library, first) = probe_library("", &lines);
    let (output, refused) = gate_clippy_on("attack-g12-sum", str::to_owned, &library);
    let through = passed(&lines, first, &refused);
    assert!(
        through.is_empty(),
        "clippy ({}) with geom-num's settings passes these unordered reductions: {through:?}",
        output.status
    );
}

/// The panic macros other than `panic!` pass the kernel's lints.
///
/// Principle 13: "no panics in kernel paths"; 28.1 checks it with "Lints"; chapter 27: "`clippy`
/// denies ... `panic`". The posture denies `clippy::panic`, which fires on `panic!` alone, so
/// `unreachable!`, `todo!` and `unimplemented!` (each a panic) pass clippy 1.99.0 with
/// `geom-num`'s settings.
#[test]
fn attack_panic_macros_other_than_panic_pass() {
    let lines = [
        "fn a() -> u8 { unreachable!() }".to_owned(),
        "fn b() -> u8 { todo!() }".to_owned(),
        "fn c() -> u8 { unimplemented!() }".to_owned(),
    ];
    let (library, first) = probe_library("", &lines);
    let (output, refused) = gate_clippy_on("attack-g12-panics", str::to_owned, &library);
    let through = passed(&lines, first, &refused);
    assert!(
        through.is_empty(),
        "clippy ({}) with geom-num's settings passes these panics in kernel code: {through:?}",
        output.status
    );
}

// ------------------------------------------------------------------ global state

/// Statics compiled into a crate's library from files the scan never reads.
///
/// `repo::crate_sources` reads the `.rs` files of a crate's folder outside `tests`, `benches`,
/// `examples` and `target`, and `source::global_state` reads each as text. rustc compiles more
/// than that into the library: a module that `#[path]` takes from `tests/`, a file of any name
/// that `include!` pastes in, and code a build script writes into `OUT_DIR` (here with the
/// keyword assembled from two strings, so the build script names no `static` either). The probe
/// builds (rustc 1.99.0, `gate::build`, warnings denied) with three statics in its library; seen
/// through the gate's own readers as geom-op's folder, each must be reported. (The fixer's gate
/// reads, besides the folder, every file the compiler read for the library, from the dep-info its
/// builds and clippy runs leave, `gate::compiled_sources`; this test reads both, as the gate's
/// `gate_build` and `gate_clippy` runs do.)
#[test]
fn attack_statics_from_files_the_scan_never_reads() {
    let root = repo::workspace_root();
    let dir = scratch("globals-probe");
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::create_dir_all(dir.join("tests")).unwrap();
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"attack-g12-globals\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\
         publish = false\nautotests = false\n\n[workspace]\n",
    )
    .unwrap();
    fs::write(
        dir.join("src").join("lib.rs"),
        "//! Probe: global state in files the scan does not read.\n\
         #![no_std]\n\
         #[path = \"../tests/state.rs\"]\n\
         mod state;\n\
         include!(\"counter.in\");\n\
         include!(concat!(env!(\"OUT_DIR\"), \"/generated.rs\"));\n\
         /// Reads the three globals.\n\
         pub fn read() -> u32 {\n\
         \x20   state::COUNT.fetch_add(1, core::sync::atomic::Ordering::Relaxed) + OTHER + GENERATED\n\
         }\n",
    )
    .unwrap();
    fs::write(
        dir.join("tests").join("state.rs"),
        "//! A counter shared by every call.\n\
         pub static COUNT: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);\n",
    )
    .unwrap();
    fs::write(
        dir.join("src").join("counter.in"),
        "static OTHER: u32 = 1;\n",
    )
    .unwrap();
    fs::write(
        dir.join("build.rs"),
        "fn main() {\n\
         \x20   let keyword = [\"sta\", \"tic\"].concat();\n\
         \x20   let out = std::env::var(\"OUT_DIR\").unwrap();\n\
         \x20   let file = std::path::Path::new(&out).join(\"generated.rs\");\n\
         \x20   std::fs::write(file, format!(\"pub {keyword} GENERATED: u32 = 2;\\n\")).unwrap();\n\
         }\n",
    )
    .unwrap();
    // rustc compiles the three statics into the library.
    let built = gate::build(
        &dir.join("Cargo.toml"),
        None,
        None,
        false,
        &scratch("globals-target"),
    );
    assert!(
        built.status.success(),
        "the probe must build: {}",
        shown(&built)
    );
    // The gate's reader and check, with geom-op's folder being the probe's.
    let canonical = |p: &Path| fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let probe_dir = canonical(&dir);
    let rest = probe_dir
        .strip_prefix(canonical(&root))
        .unwrap_or_else(|_| {
            panic!(
                "the test's temporary folder {} is not under the workspace root {}",
                dir.display(),
                root.display()
            )
        });
    let mut observed = repo::observe();
    observed.packages.get_mut("geom-op").unwrap().dir = rest
        .components()
        .map(|c| c.as_os_str().to_str().unwrap().to_owned())
        .collect();
    let mut sources = repo::crate_sources(&observed, &BTreeSet::from(["geom-op"]));
    let (compiled, problems) =
        gate::compiled_sources(&built, &dir, &BTreeSet::from(["attack-g12-globals"]));
    assert!(problems.is_empty(), "{problems:#?}");
    let files = compiled
        .get("attack-g12-globals")
        .cloned()
        .unwrap_or_default();
    globals::merge(
        &mut sources,
        repo::compiled_sources(&BTreeMap::from([("geom-op".to_owned(), files)])),
    );
    let read: Vec<String> = sources["geom-op"]
        .iter()
        .map(|f| f.path.join("/"))
        .collect();
    let found = globals::check(&table::TABLE, &sources);
    assert!(
        found.len() >= 3,
        "a library with three statics (from tests/state.rs by #[path], src/counter.in by \
         include!, and OUT_DIR by its build script) passes the ban on global state: it read \
         {read:?} and reported {}",
        report(&found)
    );
}

// ------------------------------------------------------------------ the workflow

/// Changes to kernel-gate.yml after which the required check, "Merge gate", is green while a
/// check did not run or its failure was swallowed. `gate::workflow` must refuse each.
#[test]
fn attack_gate_workflow_passes_jobs_that_cannot_fail() {
    let root = repo::workspace_root();
    let read = |relative: &str| {
        fs::read_to_string(root.join(relative))
            .unwrap()
            .replace("\r\n", "\n")
    };
    let yml = read(".github/workflows/kernel-gate.yml");
    let kernel = read(".github/workflows/kernel.yml");
    assert!(
        gate::workflow(&yml, &kernel).is_empty(),
        "the real workflow must pass for the seeds to mean anything"
    );
    let swap = |from: &str, to: &str| {
        assert!(yml.contains(from), "kernel-gate.yml lacks {from:?}");
        yml.replacen(from, to, 1)
    };
    let deny_step = "      - name: Licences, advisories, sources and bans of every crate, and each \
                     rule's seeded violations\n";
    let clippy_step =
        "      - name: Every crate that ships, and a probe that breaks each rule once\n";
    let rules_run = "        run: cargo test --locked --package engine-geometry --test scaffold\n";
    let faults: Vec<(&str, String)> = vec![
        (
            "the deny step's failure ignored",
            swap(
                deny_step,
                &format!("{deny_step}        continue-on-error: true\n"),
            ),
        ),
        (
            "the clippy step never runs",
            swap(
                clippy_step,
                &format!("{clippy_step}        if: ${{{{ false }}}}\n"),
            ),
        ),
        (
            "the builds' failure swallowed",
            swap(
                "--ignored gate_build --nocapture --test-threads 1",
                "--ignored gate_build --nocapture --test-threads 1 || true",
            ),
        ),
        (
            "a deny filter that selects no test",
            swap(
                "--ignored gate_deny --nocapture",
                "--ignored gate_deny_nothing --nocapture",
            ),
        ),
        (
            "every clippy test skipped",
            swap(
                "--ignored gate_clippy --nocapture",
                "--ignored gate_clippy --skip gate_clippy --nocapture",
            ),
        ),
        (
            "the rules run moved into a comment",
            swap(
                rules_run,
                "        run: cargo test --locked --package engine-geometry --test scaffold -- \
                 --skip the_\n        # run: cargo test --locked --package engine-geometry \
                 --test scaffold\n",
            ),
        ),
    ];
    let passed: Vec<&str> = faults
        .iter()
        .filter(|(_, text)| {
            assert_ne!(text, &yml);
            gate::workflow(text, &kernel).is_empty()
        })
        .map(|(name, _)| *name)
        .collect();
    assert!(
        passed.is_empty(),
        "gate::workflow passes kernel-gate.yml with each of these faults, after which the \
         required Merge gate check is green while the check did not run or its failure was \
         ignored: {passed:?}"
    );
}

// ------------------------------------------------------------------ sanity

/// The modules compiled here are the gate's own: the real repository passes the gate's fast
/// checks, so every failure above is the seed's.
#[test]
fn the_gate_passes_the_repository_as_it_stands() {
    let observed = repo::observe();
    assert_eq!(posture::check(&table::TABLE, &observed), []);
    let sources = repo::crate_sources(&observed, &globals::covered(&table::TABLE));
    assert_eq!(globals::check(&table::TABLE, &sources), []);
    assert_eq!(overrides::check(&table::TABLE, &sources), []);
    let root = repo::workspace_root();
    let config = |file: &str| fs::read_to_string(root.join(file)).unwrap();
    assert_eq!(
        deny::family_config(&config(FAMILY_CONFIG)),
        Vec::<String>::new()
    );
    assert_eq!(
        deny::no_std_config(&config(NO_STD_CONFIG)),
        Vec::<String>::new()
    );
    assert_eq!(
        deny::ships_config(&config(deny::SHIPS_CONFIG)),
        Vec::<String>::new()
    );
}
