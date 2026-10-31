//! Adversarial tests of piece G09, the kernel workspace and crate scaffold: they try to break the
//! scaffold check in `tests/scaffold` and the repository it guards, not to confirm them.
//!
//! What is attacked:
//!
//! - The check's own logic, on in-memory inputs: crate attributes the check accepts although
//!   rustc does not apply them to the crate (inside a nested module, inside a block comment), a
//!   `no_std` crate that brings `std` back with `extern crate std`, header fields that pass by a
//!   word in passing, a manifest whose lint table sits inside a string, the facade's `threads`
//!   gate switched on by default, and the JSON reader's depth bound.
//! - The same holes end to end: the real workspace as `cargo metadata` reports it, with one
//!   crate's `lib.rs` replaced by an attack source, must be refused.
//! - The lint posture rule, identical copies of `geom-num`'s settings: every way a crate's lint
//!   tables or clippy configuration can drift, on the real workspace, must be refused; layout and
//!   comments must not count; and the clippy configuration the check reads must be the one clippy
//!   reads.
//! - The repository against the brief and the chapters it implements: chapter 27's proposed crate
//!   table and layer table (with chapter 30.5's amendments) row by row, the workspace fields, lint
//!   tables and `clippy.toml` of every crate, the status lines against `30-build-order.json`, the
//!   sections each header names against the chapters its own build pieces cite, the toolchain pins
//!   of `rust-toolchain.toml`, `kernel.yml` and `kernel-build.yml`, and the workflow itself, with
//!   its actions pinned to `kernel.yml`'s commits, none of `kernel.yml`'s tests or lints run again,
//!   and every crate still built on its own on each target.
//!
//! Oracles: rustc's own behaviour for the attribute cases (each source here was compiled with
//! rustc 1.99.0 as a library and builds, for `x86_64` and for wasm32-unknown-unknown, while the
//! check calls it `no_std` or unsafe-free); clippy 1.99.0's own lookup of its configuration, run on
//! a probe crate; chapter 27's tables and chapter 30.5 typed in again here by hand; and data taken
//! from `architecture/30-build-order.json` on piece/G01 (commit b71caab), embedded below so the
//! test needs no file outside the repository.
//!
//! The repository tests run natively only, as the scaffold's own repository test does.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// The kernel lints deny native arithmetic in kernel code (Cargo.toml). This is test code: its
// oracles count positions in a text they hold whole, which tests may do with native arithmetic.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "test code; the rule is for kernel code"
)]

// The scaffold check's own modules, compiled again here so its functions can be attacked directly.
#[allow(dead_code)]
#[path = "scaffold/check.rs"]
mod check;
#[allow(dead_code)]
#[path = "scaffold/header.rs"]
mod header;
#[allow(dead_code)]
#[path = "scaffold/lints.rs"]
mod lints;
#[cfg(not(target_family = "wasm"))]
#[allow(dead_code)]
#[path = "scaffold/repo.rs"]
mod repo;
#[allow(dead_code)]
#[path = "scaffold/table.rs"]
mod table;
// The fixer split header.rs's attribute and manifest readers into modules of their own.
#[allow(dead_code)]
#[path = "scaffold/manifest.rs"]
mod manifest;
#[allow(dead_code)]
#[path = "scaffold/source.rs"]
mod source;
// The merge gate (G12) reads crates' sources for global state and lint attributes through
// `repo`, with the denied lints `posture` names.
#[allow(dead_code)]
#[path = "scaffold/globals.rs"]
mod globals;
#[allow(dead_code)]
#[path = "scaffold/posture.rs"]
mod posture;

use std::collections::{BTreeMap, BTreeSet};

use check::{Dep, DepKind, Observed, Package, Rule};
use header::{Field, Header};
use lints::{ClippyConfig, Lints};
use source::Attributes;
use table::{Deps, FeatureGate, Kind, Layer, Origin, Row, State, Table};

// ------------------------------------------------------------------ attack sources

/// Crate attributes written only inside a nested module. rustc applies a module's inner
/// attributes to that module: `no_std` there is an `unused_attributes` warning and the crate links
/// `std`; `forbid(unsafe_code)` there leaves the crate root free to use `unsafe`. Compiled with
/// rustc 1.99.0 (`--crate-type lib --edition 2024`): builds, one warning.
const NESTED: &str = "\
//! Probe: crate attributes only inside a nested module.
pub mod inner {
    #![cfg_attr(not(test), no_std)]
    #![forbid(unsafe_code)]
}
/// Uses unsafe code and std at the crate root.
pub fn read(p: *const u8) -> std::vec::Vec<u8> {
    let b = unsafe { *p };
    std::vec![b]
}
";

/// A `no_std` crate that brings `std` back. Chapter 21: kernel crates are `no_std` so the
/// platform's maths functions "are not even available"; here `f64::sin` and the clock are.
/// Compiled with rustc 1.99.0 for `x86_64` and for wasm32-unknown-unknown: builds, no warning.
const EXTERN_STD: &str = "\
//! Probe: no_std, then std brought back.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate std;
/// Uses std.
pub fn now() -> std::time::SystemTime {
    std::time::SystemTime::now()
}
/// Platform maths through std.
pub fn s(x: f64) -> f64 {
    x.sin()
}
";

/// Both attributes inside a block comment, so neither applies. Compiled with rustc 1.99.0:
/// builds, no warning.
const COMMENTED: &str = "\
//! Probe: attributes in a block comment.
/*
#![no_std]
#![forbid(unsafe_code)]
*/
/// Unsafe and std.
pub fn read(p: *const u8) -> std::vec::Vec<u8> {
    std::vec![unsafe { *p }]
}
";

// ------------------------------------------------------------------ the check's attribute reader

#[test]
fn forbid_inside_a_nested_module_is_not_crate_level() {
    let got = source::attributes(NESTED);
    assert!(
        !got.forbids_unsafe,
        "source::attributes(NESTED) = {got:?}; expected forbids_unsafe = false: \
         `#![forbid(unsafe_code)]` sits inside `pub mod inner {{}}`, so rustc applies it to \
         `inner` only and `pub fn read` at the crate root compiles with `unsafe {{ *p }}`. The \
         reader scans every line of the file, not the crate root's leading attributes.\n\
         source:\n{NESTED}"
    );
}

#[test]
fn no_std_inside_a_nested_module_is_not_crate_level() {
    let got = source::attributes(NESTED);
    assert!(
        !got.no_std,
        "source::attributes(NESTED) = {got:?}; expected no_std = false: rustc 1.99.0 says \
         \"the `#![no_std]` attribute can only be used at the crate root\" (a warning, so \
         `cargo build` passes) and the crate links std (`std::vec!` at the root compiles).\n\
         source:\n{NESTED}"
    );
}

#[test]
fn attributes_inside_a_block_comment_do_not_count() {
    let got = source::attributes(COMMENTED);
    assert_eq!(
        got,
        Attributes::default(),
        "source::attributes(COMMENTED) = {got:?}; expected Attributes {{ no_std: false, \
         forbids_unsafe: false }}: both attributes are inside `/* ... */`, so the crate is a std \
         crate with unsafe code, and it builds.\nsource:\n{COMMENTED}"
    );
}

#[test]
fn extern_crate_std_defeats_no_std() {
    let got = source::attributes(EXTERN_STD);
    assert!(
        !got.no_std,
        "source::attributes(EXTERN_STD) = {got:?}; expected no_std = false: \
         `extern crate std;` after `#![cfg_attr(not(test), no_std)]` links std again, so \
         `f64::sin` (platform maths, forbidden by chapter 21) and `SystemTime::now` compile. \
         Every CI target in kernel-build.yml (x86_64-unknown-linux-gnu, wasm32-unknown-unknown, \
         wasm32-wasip1) has std, so no build catches it either.\nsource:\n{EXTERN_STD}"
    );
}

// ------------------------------------------------------------------ the check's header grammar

fn header_of(lines: &[&str]) -> Header {
    let text: String = lines.iter().flat_map(|l| ["//! ", l, "\n"]).collect();
    Header::parse(&text)
}

#[test]
fn one_job_in_passing_is_not_a_job_line() {
    // header.rs: "a field counts only when written in the grammar below, so a header cannot pass
    // by mentioning a word in passing."
    let lines = [
        "Spare Kit geometry kernel: a thing.",
        "",
        "Each module here has one job.",
    ];
    let got = header_of(&lines).has(Field::Job);
    assert!(
        !got,
        "Header::has(Job) on {lines:?} = {got}; expected false: the only mention of a job is \
         \"Each module here has one job.\", a phrase in passing that states no job. The reader \
         accepts any line containing \"one job\" anywhere."
    );
}

#[test]
fn an_empty_job_line_states_no_job() {
    let lines = ["Spare Kit geometry kernel: a thing.", "", "One job:"];
    let got = header_of(&lines).has(Field::Job);
    assert!(
        !got,
        "Header::has(Job) on {lines:?} = {got}; expected false: \"One job:\" with nothing after \
         it states no job. The Sections field already refuses an empty label; the Job field \
         does not."
    );
}

#[test]
fn a_sections_field_must_name_a_section() {
    let mut wrong = Vec::new();
    for line in [
        "Sections: to be decided.",
        "A heat Map: one colour per vertex.",
    ] {
        let lines = ["Spare Kit geometry kernel: a thing.", "", line];
        if header_of(&lines).has(Field::Sections) {
            wrong.push(line);
        }
    }
    assert!(
        wrong.is_empty(),
        "Header::has(Sections) = true for {wrong:?}; expected false for both: neither names a \
         section of the architecture (no chapter number at all), and the second is the word \
         \"Map: \" in passing in the middle of a line. The reader accepts \"Sections: \" or \
         \"Map: \" anywhere in a line followed by any text."
    );
}

// ------------------------------------------------------------------ the manifest lint reader

#[test]
fn a_lint_table_inside_a_string_is_not_a_lint_table() {
    let manifest = "[package]\nname = \"geom-x\"\ndescription = \"\"\"\n[lints.rust]\n\
                    unsafe_code = \"forbid\"\n\"\"\"\n";
    let got = crate::manifest::manifest_forbids_unsafe(manifest, "");
    assert!(
        !got,
        "manifest::manifest_forbids_unsafe({manifest:?}, \"\") = {got}; expected false: the \
         `[lints.rust]` lines are the text of a multi-line `description` string, so cargo sets \
         no lint and unsafe code is allowed. The reader treats every line that starts with `[` \
         as a table header, inside strings too."
    );
}

// ------------------------------------------------------------------ the JSON reader's depth bound

/// The scaffold reads `cargo metadata` and cargo-deny's output with `serde_json` (piece G12a put it
/// in place of G09's reader): `serde_json::from_str`, `from_slice` and `cargo_metadata`'s
/// `MetadataCommand::exec` all keep its recursion limit, which `cargo_metadata`'s use of the
/// crate's `unbounded_depth` feature leaves on (the feature only adds a way to switch it off). JSON
/// nested one level deeper than the limit is an error, not a stack overflow. Cargo's build
/// messages are the one input read without the limit: `cargo_metadata`'s `Message::parse_stream`
/// switches it off on purpose, because rustc nests the spans of macro expansions deeper than 128
/// levels, and that input is the pinned compiler's own output (gate.rs, `cargo_messages`).
#[test]
fn nesting_one_deeper_than_the_json_readers_limit_is_refused() {
    // serde_json's limit: `remaining_depth` starts at 128 in its de.rs and each array or object
    // takes one before it is read, refusing the one that reaches 0, so 127 levels read and 128 do
    // not.
    let parse = |depth: usize| {
        let text = "[".repeat(depth) + &"]".repeat(depth);
        serde_json::from_str::<serde_json::Value>(&text)
    };
    assert!(
        parse(127).is_ok(),
        "127 nested arrays are within serde_json's limit"
    );
    let got = parse(128);
    assert!(
        got.is_err(),
        "serde_json read 128 nested arrays: {got:?}; its recursion limit is off"
    );
    let objects = "{\"a\":".repeat(128) + "1" + &"}".repeat(128);
    assert!(serde_json::from_str::<serde_json::Value>(&objects).is_err());
    assert!(
        serde_json::from_slice::<serde_json::Value>(
            ("[".repeat(128) + &"]".repeat(128)).as_bytes()
        )
        .is_err()
    );
}

// ------------------------------------------------------------------ the facade's threads gate

const GATE_ROWS: &[Row<'static>] = &[
    Row {
        name: "geom-run",
        layer: Layer::N(1),
        kind: Kind::Kernel,
        origin: Origin::Accepted,
        state: State::Here,
        deps: Deps::Exact(""),
        external: "",
    },
    Row {
        name: "geom-pool",
        layer: Layer::N(13),
        kind: Kind::KernelStd,
        origin: Origin::Accepted,
        state: State::Here,
        deps: Deps::Exact("run"),
        external: "",
    },
    Row {
        name: "engine-geometry",
        layer: Layer::N(13),
        kind: Kind::Facade,
        origin: Origin::Accepted,
        state: State::Here,
        deps: Deps::Exact("pool"),
        external: "kit-core",
    },
];

const GATE_TABLE: Table<'static> = Table {
    rows: GATE_ROWS,
    same_layer: &[("engine-geometry", "pool")],
    pending: &[],
    feature_gates: &[FeatureGate {
        krate: "engine-geometry",
        dep: "pool",
        feature: "threads",
    }],
    header_gaps: &[],
    registry: &[],
    lints_from: "run",
};

/// One lint posture for every crate of the gate example, so only the gate is under test.
fn gate_lints(dir: &[String]) -> Lints {
    let mut file = dir.to_vec();
    file.push("clippy.toml".to_owned());
    Lints {
        tables: Some(BTreeMap::new()),
        clippy: Some(ClippyConfig {
            file,
            settings: Some(BTreeMap::new()),
        }),
    }
}

fn scaffold_header(layer: u8) -> Header {
    Header::parse(&format!(
        "//! Spare Kit geometry kernel: an example.\n//!\n//! One job: an example.\n//!\n\
         //! Layer: {layer}, an example layer.\n//!\n//! Invariants this crate keeps:\n//!\n\
         //! - One.\n//!\n//! Sections: 23.4.\n//!\n\
         //! Status: scaffold only; built by pieces G33.\n"
    ))
}

fn path_dep(name: &str, optional: bool) -> Dep {
    Dep {
        name: name.to_owned(),
        kind: DepKind::Normal,
        path: true,
        optional,
        source: None,
    }
}

fn strings(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| (*s).to_owned()).collect()
}

/// A workspace that matches `GATE_TABLE`, with the facade's features as given.
fn gate_workspace(features: &[(&str, &[&str])]) -> Observed {
    let lib = Some(strings(&["src", "lib.rs"]));
    let mut packages = BTreeMap::new();
    let run_dir = strings(&["projects", "engines", "geometry", "crates", "geom-run"]);
    let pool_dir = strings(&["projects", "engines", "geometry", "crates", "geom-pool"]);
    let facade_dir = strings(&["projects", "engines", "crates", "engine-geometry"]);
    packages.insert(
        "geom-run".to_owned(),
        Package {
            lints: gate_lints(&run_dir),
            dir: run_dir,
            lib: lib.clone(),
            header: scaffold_header(1),
            attributes: Attributes {
                no_std: true,
                forbids_unsafe: true,
            },
            manifest_forbids_unsafe: true,
            deps: vec![],
            features: BTreeMap::new(),
        },
    );
    packages.insert(
        "geom-pool".to_owned(),
        Package {
            lints: gate_lints(&pool_dir),
            dir: pool_dir,
            lib: lib.clone(),
            header: scaffold_header(13),
            attributes: Attributes {
                no_std: false,
                forbids_unsafe: true,
            },
            manifest_forbids_unsafe: true,
            deps: vec![path_dep("geom-run", false)],
            features: BTreeMap::new(),
        },
    );
    packages.insert(
        "engine-geometry".to_owned(),
        Package {
            lints: gate_lints(&facade_dir),
            dir: facade_dir,
            lib,
            header: scaffold_header(13),
            attributes: Attributes {
                no_std: false,
                forbids_unsafe: true,
            },
            manifest_forbids_unsafe: true,
            deps: vec![path_dep("geom-pool", true)],
            features: features
                .iter()
                .map(|(f, on)| ((*f).to_owned(), strings(on)))
                .collect(),
        },
    );
    Observed {
        packages,
        geometry_dirs: ["geom-run", "geom-pool"].map(str::to_owned).into(),
    }
}

fn all_violations(table: &Table<'_>, observed: &Observed) -> Vec<check::Violation> {
    let mut found = check::table(table);
    found.extend(check::workspace(table, observed));
    found
}

#[test]
fn the_threads_gate_example_is_valid() {
    // The example itself is correct: geom-pool optional and switched on by `threads` alone.
    let observed = gate_workspace(&[("threads", &["dep:geom-pool"])]);
    let found = all_violations(&GATE_TABLE, &observed);
    assert!(found.is_empty(), "the valid example is refused: {found:?}");
}

#[test]
fn geom_pool_on_by_default_is_not_behind_threads() {
    let features: &[(&str, &[&str])] =
        &[("default", &["threads"]), ("threads", &["dep:geom-pool"])];
    let observed = gate_workspace(features);
    let found = all_violations(&GATE_TABLE, &observed);
    let rules: BTreeSet<Rule> = found.iter().map(|v| v.rule).collect();
    assert!(
        rules.contains(&Rule::FeatureGate),
        "engine-geometry features {features:?}, geom-pool optional: check reports {found:?}; \
         expected a FeatureGate violation: with `default = [\"threads\"]` every default build of \
         the facade, wasm32-unknown-unknown included, links geom-pool and its std threads, so \
         geom-pool is not behind its `threads` feature (chapter 27 row, 23.4). The check only asks \
         that `threads` switches it on, not that nothing else does."
    );
}

// ------------------------------------------------------------------ the repository

#[cfg(not(target_family = "wasm"))]
mod repository {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::{COMMENTED, EXTERN_STD, NESTED, check, repo, table};
    use crate::check::{Observed, Rule};
    use crate::table::{Deps, Kind, Layer, Origin, State, expand};

    fn root() -> PathBuf {
        // projects/engines/crates/engine-geometry: four folders below the workspace root.
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(4)
            .expect("the workspace root is four folders up")
            .to_path_buf()
    }

    /// A repository file's text, with line endings normalised: a Windows checkout gives committed
    /// files CRLF and new ones LF, which is not a difference in content.
    fn read(relative: &str) -> String {
        let path = root().join(relative);
        fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
            .replace("\r\n", "\n")
    }

    fn crate_dir(name: &str) -> String {
        if name.starts_with("engine-") {
            format!("projects/engines/crates/{name}")
        } else {
            format!("projects/engines/geometry/crates/{name}")
        }
    }

    fn header_lines(source: &str) -> Vec<String> {
        source
            .lines()
            .map_while(|l| l.strip_prefix("//!"))
            .map(|r| r.strip_prefix(' ').unwrap_or(r).to_owned())
            .collect()
    }

    /// The header paragraph that starts with a line starting `label`, joined with spaces.
    fn paragraph(lines: &[String], labels: &[&str]) -> Option<String> {
        let at = lines
            .iter()
            .position(|l| labels.iter().any(|label| l.starts_with(label)))?;
        Some(
            lines[at..]
                .iter()
                .take_while(|l| !l.trim().is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join(" "),
        )
    }

    /// Chapter numbers named in a text: every run of digits not right after a '.'.
    fn chapters_named(text: &str) -> BTreeSet<u8> {
        let chars: Vec<char> = text.chars().collect();
        let mut out = BTreeSet::new();
        let mut i = 0;
        while i < chars.len() {
            if chars[i].is_ascii_digit() {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                let after_dot = start > 0 && chars[start - 1] == '.';
                let digits: String = chars[start..i].iter().collect();
                if let (false, Ok(n)) = (after_dot, digits.parse::<u8>()) {
                    out.insert(n);
                }
            } else {
                i += 1;
            }
        }
        out
    }

    fn piece_ids(text: &str) -> BTreeSet<String> {
        text.split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|w| {
                w.len() > 1
                    && w.starts_with('G')
                    && w[1..].chars().next().is_some_and(|c| c.is_ascii_digit())
            })
            .map(str::to_owned)
            .collect()
    }

    fn set(list: &str) -> BTreeSet<String> {
        expand(list).into_iter().collect()
    }

    // -------------------------------------------------------------- end to end

    #[test]
    fn the_real_check_accepts_crates_that_link_std_or_allow_unsafe() {
        // Each attack source replaces one real crate's lib.rs, as `repo::observe` would read it;
        // its manifest is taken to lack `unsafe_code = "forbid"`, as a crate merged from another
        // piece may. Each must be refused.
        let cases = [
            ("geom-linalg", EXTERN_STD, vec![Rule::NotNoStd]),
            (
                "geom-p21",
                NESTED,
                vec![Rule::NotNoStd, Rule::UnsafeNotForbidden],
            ),
            (
                "geom-heal",
                COMMENTED,
                vec![Rule::NotNoStd, Rule::UnsafeNotForbidden],
            ),
        ];
        let mut observed = repo::observe();
        assert!(
            check::workspace(&table::TABLE, &observed).is_empty(),
            "the unmodified repository already fails the check"
        );
        for (name, source, _) in &cases {
            let package = observed.packages.get_mut(*name).unwrap();
            package.attributes = crate::source::attributes(source);
            package.manifest_forbids_unsafe = false;
        }
        let found = check::workspace(&table::TABLE, &observed);
        let mut missed = Vec::new();
        for (name, _, expected) in &cases {
            for rule in expected {
                if !found.iter().any(|v| v.krate == *name && v.rule == *rule) {
                    missed.push(format!("{name}: {rule:?}"));
                }
            }
        }
        assert!(
            missed.is_empty(),
            "check::workspace on the real workspace with geom-linalg's lib.rs = EXTERN_STD, \
             geom-p21's = NESTED and geom-heal's = COMMENTED (manifest lint tables without \
             unsafe_code) reports {found:?}; expected these violations, which it misses: \
             {missed:?}. Each of those sources compiles with rustc 1.99.0 as a std crate (and \
             NESTED and COMMENTED with `unsafe` at the root), so the board's \"kernel crates are \
             no_std\" and \"forbid(unsafe_code) in every kernel crate\" are not what the check \
             proves."
        );
    }

    #[test]
    fn the_repository_check_passes_today() {
        let observed = repo::observe();
        let found = check::workspace(&table::TABLE, &observed);
        assert!(found.is_empty(), "{found:?}");
        let present: Vec<&str> = table::TABLE
            .rows
            .iter()
            .filter(|r| observed.packages.contains_key(r.name))
            .map(|r| r.name)
            .collect();
        // 44 scaffold crates, the facade and geom-pool, plus geom-run (G04), geom-math (G03), the
        // development crates geom-test, geom-fuzz and geom-gen (G11), and geom-bench (G06) now that
        // they have merged.
        assert_eq!(present.len(), 51, "{present:?}");
    }

    // -------------------------------------------------------------- chapter 27, row by row

    /// Chapter 27's crate table (accepted in 32.13) typed in again from the chapter (layer, direct
    /// dependencies in its notation), with chapter 30.5's four amendments applied, and
    /// `geom-format`'s "everything stored" as section 24.4's list.
    const CHAPTER_27: &[(&str, u8, &str)] = &[
        ("geom-run", 1, ""),
        ("geom-num", 1, "run"),
        ("geom-linalg", 1, "num"),
        ("geom-op", 1, "run num"),
        ("geom-math", 2, "num op"),
        ("geom-bvh", 2, "run num math"),
        ("geom-nurbs", 2, "base math linalg"),
        ("geom-curve", 2, "base math nurbs"),
        ("geom-surface", 2, "base math nurbs curve"),
        ("geom-topo", 3, "base math curve surface"),
        ("geom-name", 3, "base topo"),
        ("geom-intersect", 4, "base math nurbs curve surface bvh"),
        ("geom-check", 5, "base curve topo intersect"),
        ("geom-poly", 6, "base math topo"),
        ("geom-bool", 6, "base poly intersect topo name check bvh"),
        ("geom-2d", 6, "base math curve topo name intersect bvh"),
        (
            "geom-mesh",
            7,
            "base math topo poly curve surface intersect bvh linalg 2d",
        ),
        ("geom-spatial", 7, "base math mesh bvh poly intersect"),
        ("geom-subd", 8, "base math nurbs surface"),
        ("geom-field", 8, "base math mesh surface 2d bvh"),
        ("geom-cloud", 8, "base math spatial nurbs surface linalg"),
        ("geom-skeleton", 9, "base math 2d topo poly"),
        (
            "geom-sweep",
            9,
            "base topo surface name check poly intersect bool skeleton",
        ),
        ("geom-make", 9, "base math curve surface topo check"),
        ("geom-blend", 10, "base bool intersect surface topo check"),
        (
            "geom-offset",
            10,
            "base bool intersect surface skeleton topo",
        ),
        ("geom-sketch", 10, "base math curve linalg"),
        (
            "geom-profile",
            10,
            "base sketch 2d topo name check intersect",
        ),
        (
            "geom-terrain",
            10,
            "base math 2d mesh topo poly bool skeleton curve surface name bvh",
        ),
        ("geom-align", 10, "base math curve sweep name"),
        (
            "geom-draw",
            10,
            "base topo name 2d intersect spatial math curve surface mesh bvh",
        ),
        (
            "geom-convert",
            10,
            "base topo mesh subd field nurbs check bool cloud spatial intersect sweep",
        ),
        (
            "geom-combine",
            10,
            "base bool mesh field convert check subd",
        ),
        (
            "geom-panel",
            10,
            "base topo check surface intersect poly mesh 2d linalg",
        ),
        ("geom-analysis", 10, "base surface topo intersect spatial"),
        ("geom-civil", 11, "base terrain align"),
        ("geom-assembly", 11, "base sketch spatial"),
        (
            "geom-format",
            12,
            "base num math curve surface topo poly name mesh subd field 2d cloud draw sketch",
        ),
        (
            "geom-exchange",
            12,
            "base math curve surface topo format p21 intersect check",
        ),
        ("geom-p21", 12, "base format"),
        ("geom-heal", 12, "base topo check mesh exchange"),
        ("geom-landxml", 12, "base format exchange 2d terrain align"),
        ("geom-gerber", 12, "base format exchange 2d"),
        ("geom-ipc2581", 12, "base format exchange 2d"),
        ("geom-pool", 13, "run"),
    ];

    #[test]
    fn the_table_is_chapter_27_with_chapter_30_5() {
        let rows: BTreeMap<&str, &table::Row<'_>> =
            table::TABLE.rows.iter().map(|r| (r.name, r)).collect();
        let mut wrong = Vec::new();
        for (name, layer, deps) in CHAPTER_27 {
            let Some(row) = rows.get(name) else {
                wrong.push(format!("{name}: no row"));
                continue;
            };
            if row.layer != Layer::N(*layer) {
                wrong.push(format!("{name}: layer {:?}, chapter 27 {layer}", row.layer));
            }
            let got = match row.deps {
                Deps::Exact(list) => set(list),
                other => {
                    wrong.push(format!("{name}: deps {other:?}, expected an exact list"));
                    continue;
                }
            };
            if got != set(deps) {
                wrong.push(format!("{name}: table {got:?}, chapter {:?}", set(deps)));
            }
        }
        // The format crates: base, format, exchange and heal (ifc and step also p21), plus kinds.
        for (name, p21) in [
            ("geom-ifc", true),
            ("geom-step", true),
            ("geom-dxf", false),
            ("geom-3dm", false),
            ("geom-meshio", false),
        ] {
            let row = rows[name];
            let Deps::Exact(list) = row.deps else {
                wrong.push(format!("{name}: not exact"));
                continue;
            };
            let mut must = set("base format exchange heal");
            if p21 {
                must.insert("geom-p21".to_owned());
            }
            if !must.is_subset(&set(list)) || row.layer != Layer::N(12) {
                wrong.push(format!("{name}: {list} lacks {must:?}"));
            }
        }
        // Toolkits: "base and the operation crates they compose"; board also on sheet.
        for (name, at_least) in [
            ("geom-sheet", "base"),
            ("geom-thread", "base"),
            ("geom-stair", "base"),
            ("geom-board", "base sheet"),
        ] {
            let row = rows[name];
            let ok = matches!(row.deps, Deps::Partial(list) if set(at_least).is_subset(&set(list)))
                && row.layer == Layer::N(11)
                && matches!(row.state, State::Deferred(_));
            if !ok {
                wrong.push(format!("{name}: {row:?}"));
            }
        }
        let facade = rows["engine-geometry"];
        if facade.kind != Kind::Facade
            || facade.layer != Layer::N(13)
            || facade.external != "kit-core"
            || facade.origin != Origin::Accepted
        {
            wrong.push(format!("engine-geometry: {facade:?}"));
        }
        for name in [
            "engine-geometry-c",
            "engine-geometry-wasm",
            "engine-geometry-py",
        ] {
            let row = rows[name];
            if row.kind != Kind::Binding || row.deps != Deps::Exact("engine-geometry") {
                wrong.push(format!("{name}: {row:?}"));
            }
        }
        for name in ["geom-test", "geom-fuzz", "geom-bench", "geom-gen"] {
            if rows[name].kind != Kind::Development {
                wrong.push(format!("{name}: {:?}", rows[name]));
            }
        }
        assert_eq!(rows.len(), 62, "rows: {:?}", rows.keys());
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    #[test]
    fn the_same_layer_edges_are_the_layer_table_with_chapter_30_5() {
        let mut expected: BTreeSet<(String, String)> = BTreeSet::new();
        let mut allow = |from: &str, to: &str| {
            for t in expand(to) {
                expected.insert((table::full_name(from), t));
            }
        };
        allow("num", "run");
        allow("linalg", "num");
        allow("op", "run num");
        allow("bvh", "math");
        allow("nurbs", "math");
        allow("curve", "math nurbs");
        allow("surface", "math nurbs curve");
        allow("name", "topo");
        allow("bool", "poly");
        allow("spatial", "mesh");
        allow("sweep", "skeleton");
        allow("profile", "sketch");
        allow("combine", "convert");
        allow("board", "sheet");
        for f in [
            "ifc", "step", "dxf", "3dm", "meshio", "landxml", "gerber", "ipc2581",
        ] {
            allow(f, "format exchange heal");
        }
        allow("ifc", "p21");
        allow("step", "p21");
        allow("engine-geometry", "pool");
        for b in [
            "engine-geometry-c",
            "engine-geometry-wasm",
            "engine-geometry-py",
        ] {
            allow(b, "engine-geometry");
        }
        // Chapter 30.5's amendments.
        allow("p21", "format");
        allow("exchange", "format p21");
        allow("heal", "exchange");
        let got: BTreeSet<(String, String)> = table::TABLE
            .same_layer
            .iter()
            .flat_map(|(from, to)| {
                expand(to)
                    .into_iter()
                    .map(move |t| (table::full_name(from), t))
            })
            .collect();
        assert_eq!(got, expected);
    }

    // -------------------------------------------------------------- the crates on disk

    /// The crates this piece scaffolds or edits, beside geom-num, which it may not edit.
    fn scaffolded() -> Vec<&'static str> {
        table::TABLE
            .rows
            .iter()
            .filter(|r| r.state == State::Here)
            .map(|r| r.name)
            .collect()
    }

    /// A TOML file's setting lines, in order: every line but blank lines and comment lines,
    /// trimmed. This oracle shares no code with the scaffold check's reader; it holds for files
    /// with no comment after a value on its line, which `posture_files_have_no_trailing_comment`
    /// checks for geom-num's files.
    fn setting_lines(text: &str) -> Vec<&str> {
        text.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect()
    }

    /// A manifest's lint tables: everything from its `[lints.rust]` line on, as G09's crates and
    /// geom-num lay them out (the lint tables last).
    fn lint_tail(manifest: &str) -> String {
        manifest
            .split_once("\n[lints.rust]")
            .map(|(_, rest)| format!("[lints.rust]{rest}"))
            .unwrap_or_default()
    }

    const NUM: &str = "projects/engines/geometry/crates/geom-num";

    #[test]
    fn posture_files_have_no_trailing_comment() {
        // `setting_lines` keeps a comment that follows a value on its line; geom-num's files have
        // none, so the oracle compares settings only. '#' appears in neither file's settings.
        for file in ["Cargo.toml", "clippy.toml"] {
            let text = read(&format!("{NUM}/{file}"));
            let text = if file == "Cargo.toml" {
                lint_tail(&text)
            } else {
                text
            };
            let with_hash: Vec<&str> = setting_lines(&text)
                .into_iter()
                .filter(|l| l.contains('#'))
                .collect();
            assert_eq!(with_hash, Vec::<&str>::new(), "{file}");
        }
    }

    #[test]
    fn every_scaffold_carries_the_workspace_fields_lints_and_clippy_toml() {
        // The rule the scaffold check states (scaffold/lints.rs): identical copies of geom-num's
        // settings, comments aside. Checked here by an oracle of its own: the same setting lines in
        // the same order, from the [lints.rust] line to the end of Cargo.toml, and in clippy.toml.
        let num_lints = lint_tail(&read(&format!("{NUM}/Cargo.toml")));
        let num_clippy = read(&format!("{NUM}/clippy.toml"));
        // geom-num's posture as G02's second round set it: pedantic, native arithmetic and lossy
        // casts denied, chapter 21's lists. A copy of an empty posture would prove nothing.
        for needle in [
            "pedantic = { level = \"warn\", priority = -1 }",
            "arithmetic_side_effects = \"deny\"",
            "cast_possible_truncation = \"deny\"",
            "cast_possible_wrap = \"deny\"",
            "cast_sign_loss = \"deny\"",
        ] {
            assert!(setting_lines(&num_lints).contains(&needle), "{needle}");
        }
        for needle in [
            "disallowed-methods = [",
            "{ path = \"f64::max\", reason = \"may return either zero; use Real::total_max (chapter 21)\" },",
            "disallowed-types = [",
        ] {
            assert!(setting_lines(&num_clippy).contains(&needle), "{needle}");
        }
        let mut wrong = Vec::new();
        for name in scaffolded() {
            let dir = crate_dir(name);
            let manifest = read(&format!("{dir}/Cargo.toml"));
            for key in ["edition.workspace = true", "publish.workspace = true"] {
                if !manifest.lines().any(|l| l == key) {
                    wrong.push(format!("{name}: no `{key}`"));
                }
            }
            if name != "engine-geometry"
                && !manifest.lines().any(|l| l == "license.workspace = true")
            {
                wrong.push(format!("{name}: no `license.workspace = true`"));
            }
            if setting_lines(&lint_tail(&manifest)) != setting_lines(&num_lints) {
                wrong.push(format!("{name}: lint tables differ from geom-num's"));
            }
            // Its own clippy.toml, and no .clippy.toml, which clippy would read instead.
            if setting_lines(&read(&format!("{dir}/clippy.toml"))) != setting_lines(&num_clippy) {
                wrong.push(format!("{name}: clippy.toml differs from geom-num's"));
            }
            if root().join(&dir).join(".clippy.toml").exists() {
                wrong.push(format!("{name}: a .clippy.toml, which clippy reads first"));
            }
        }
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    fn tables_of<'o>(observed: &'o mut Observed, name: &str) -> &'o mut crate::manifest::Keys {
        observed
            .packages
            .get_mut(name)
            .unwrap()
            .lints
            .tables
            .as_mut()
            .unwrap()
    }

    fn clippy_of<'o>(observed: &'o mut Observed, name: &str) -> &'o mut crate::manifest::Keys {
        observed
            .packages
            .get_mut(name)
            .unwrap()
            .lints
            .clippy
            .as_mut()
            .unwrap()
            .settings
            .as_mut()
            .unwrap()
    }

    #[test]
    fn the_real_check_refuses_every_drift_from_geom_nums_lints() {
        // On the real workspace, one crate's settings at a time changed one way: a lint left out,
        // a lint added at a stricter level, a lint loosened (on the facade, which ships too), a
        // disallowed method dropped, a setting added to clippy.toml, no clippy configuration at
        // all, and a manifest the reader cannot read.
        let mut observed = repo::observe();
        assert!(
            check::workspace(&table::TABLE, &observed).is_empty(),
            "the unmodified repository already fails the check"
        );
        assert!(
            tables_of(&mut observed, "geom-2d")
                .remove("lints.clippy.cast_sign_loss")
                .is_some()
        );
        tables_of(&mut observed, "geom-bool")
            .insert("lints.clippy.float_cmp".to_owned(), "deny".into());
        tables_of(&mut observed, "engine-geometry").insert(
            "lints.clippy.arithmetic_side_effects".to_owned(),
            "warn".into(),
        );
        let methods = clippy_of(&mut observed, "geom-op")
            .get_mut("disallowed-methods")
            .unwrap();
        let list = methods.as_array_mut().unwrap();
        let before = list.len();
        list.retain(|item| item.get("path").and_then(|p| p.as_str()) != Some("f64::hypot"));
        assert_eq!(list.len() + 1, before, "f64::hypot is listed once");
        clippy_of(&mut observed, "geom-ifc")
            .insert("too-many-lines-threshold".to_owned(), 50.into());
        observed.packages.get_mut("geom-make").unwrap().lints.clippy = None;
        observed.packages.get_mut("geom-p21").unwrap().lints.tables = None;
        let expected: BTreeSet<(String, Rule)> = [
            ("geom-2d", Rule::LintTables),
            ("geom-bool", Rule::LintTables),
            ("engine-geometry", Rule::LintTables),
            ("geom-op", Rule::ClippyConfig),
            ("geom-ifc", Rule::ClippyConfig),
            ("geom-make", Rule::ClippyConfig),
            ("geom-p21", Rule::LintTables),
        ]
        .into_iter()
        .map(|(k, r)| (k.to_owned(), r))
        .collect();
        let found: BTreeSet<(String, Rule)> = check::workspace(&table::TABLE, &observed)
            .into_iter()
            .map(|v| (v.krate, v.rule))
            .collect();
        assert_eq!(found, expected);
    }

    #[test]
    fn layout_and_comments_are_not_settings() {
        // geom-num's real files, laid out again: comments dropped or reworded, spacing changed,
        // a table written as dotted keys and an inline table as its own table, a string requoted.
        // The settings read the same, as TOML values (piece G12a: the `toml` crate reads them, and
        // `'deny'` is the same value to cargo as `"deny"`); a changed value does not.
        let manifest = read(&format!("{NUM}/Cargo.toml"));
        let clippy = read(&format!("{NUM}/clippy.toml"));
        let relaid_manifest = manifest
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .map(|l| {
                l.replace(
                    "pedantic = { level = \"warn\", priority = -1 }",
                    "pedantic.level = \"warn\" # the group\npedantic.priority = -1",
                )
                .replace(" = ", "   =   ")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let relaid_clippy = clippy
            .replace("# ", "# reworded: ")
            .replace("{ path", "{path")
            .replace("\" },", "\"} ,");
        assert_eq!(
            crate::manifest::lint_settings(&relaid_manifest),
            crate::manifest::lint_settings(&manifest)
        );
        assert_eq!(
            crate::manifest::keys(&relaid_clippy),
            crate::manifest::keys(&clippy)
        );
        assert!(crate::manifest::lint_settings(&manifest).is_some_and(|s| s.len() > 10));
        let requoted = manifest.replace("panic = \"deny\"", "panic = 'deny'");
        assert_ne!(requoted, manifest, "geom-num's manifest denies panic");
        assert_eq!(
            crate::manifest::lint_settings(&requoted),
            crate::manifest::lint_settings(&manifest)
        );
        let loosened = manifest.replace("panic = \"deny\"", "panic = 'warn'");
        assert_ne!(
            crate::manifest::lint_settings(&loosened),
            crate::manifest::lint_settings(&manifest)
        );
    }

    #[test]
    #[allow(
        clippy::disallowed_methods,
        reason = "the process id only names this test's own temporary 
              folder so parallel runs do not collide; nothing the kernel computes depends on it"
    )]
    fn the_clippy_configuration_is_the_one_clippy_reads() {
        // clippy 1.99.0, run on a probe crate, reads `.clippy.toml` before `clippy.toml` in one
        // folder (warning that it ignores the other), and the nearest folder above a crate that
        // holds neither. `repo::clippy_config` must name the same file.
        // The process id only names a scratch folder; the clock and process rules (chapter 21)
        // are for kernel code.
        #[allow(
            clippy::disallowed_methods,
            reason = "test code names a scratch folder"
        )]
        let base = std::env::temp_dir().join(format!("g09-clippy-lookup-{}", std::process::id()));
        if base.exists() {
            // Left by an earlier run that stopped half way.
            fs::remove_dir_all(&base).unwrap();
        }
        let krate = base.join("up").join("krate");
        fs::create_dir_all(&krate).unwrap();
        let write = |path: &Path, text: &str| fs::write(path, text).unwrap();
        write(&base.join("clippy.toml"), "a = 1\n");
        let found = |dir: &Path| -> Option<(Vec<String>, String)> {
            repo::clippy_config(dir, &base).map(|c| {
                let a = c.settings.unwrap()["a"].to_string();
                (c.file, a)
            })
        };
        let at =
            |parts: &[&str]| -> Vec<String> { parts.iter().map(|p| (*p).to_owned()).collect() };
        // Neither file in the crate's folder: the root's.
        assert_eq!(found(&krate), Some((at(&["clippy.toml"]), "1".to_owned())));
        // One in the folder above: that one, not the root's.
        write(&base.join("up").join(".clippy.toml"), "a = 2\n");
        assert_eq!(
            found(&krate),
            Some((at(&["up", ".clippy.toml"]), "2".to_owned()))
        );
        // One in the crate's own folder: that one, even as `clippy.toml` against a `.clippy.toml`
        // above; then both in the crate's folder: `.clippy.toml`.
        write(&krate.join("clippy.toml"), "a = 3\n");
        assert_eq!(
            found(&krate),
            Some((at(&["up", "krate", "clippy.toml"]), "3".to_owned()))
        );
        write(&krate.join(".clippy.toml"), "a = 4\n");
        assert_eq!(
            found(&krate),
            Some((at(&["up", "krate", ".clippy.toml"]), "4".to_owned()))
        );
        // Nothing at or above the crate within the root: none, whatever lies above the root.
        let inner = base.join("up").join("krate").join("inner");
        fs::create_dir_all(&inner).unwrap();
        assert_eq!(repo::clippy_config(&inner, &inner), None);
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn the_scaffolds_hold_no_code() {
        let mut wrong = Vec::new();
        for name in scaffolded().into_iter().filter(|n| *n != "engine-geometry") {
            let source = read(&format!("{}/src/lib.rs", crate_dir(name)));
            let body: Vec<&str> = source
                .lines()
                .filter(|l| !l.starts_with("//!") && !l.trim().is_empty())
                .collect();
            // geom-pool may use std (27): it forbids unsafe code and declares nothing else.
            let kind = table::TABLE
                .rows
                .iter()
                .find(|r| r.name == name)
                .unwrap()
                .kind;
            let expected: &[&str] = if kind == Kind::KernelStd {
                &["#![forbid(unsafe_code)]"]
            } else {
                &["#![cfg_attr(not(test), no_std)]", "#![forbid(unsafe_code)]"]
            };
            if body != expected {
                wrong.push(format!("{name}: {body:?}"));
            }
        }
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    /// Every piece of `30-build-order.json` whose crate field names the crate.
    const STATUS_PIECES: &[(&str, &str)] = &[
        (
            "geom-2d",
            "G192 G193 G194 G195 G196 G197 G198 G199 G200 G201 G202 G203 G204 G205 G206 G207 \
             G208 G209 G210 G211 G212 G213 G359 G360 G361",
        ),
        ("geom-3dm", "G438"),
        ("geom-align", "G373 G374 G376"),
        ("geom-analysis", "G346 G347 G348 G349"),
        ("geom-blend", "G262 G263 G264 G265 G266 G267 G268 G269"),
        (
            "geom-bool",
            "G129 G130 G131 G132 G133 G137 G138 G139 G144 G242 G243 G244 G245 G246 G247 G248 \
             G249 G250 G324",
        ),
        ("geom-cloud", "G350 G379 G380 G436"),
        ("geom-combine", "G336 G337"),
        (
            "geom-convert",
            "G322 G323 G333 G334 G335 G340 G350 G351 G425",
        ),
        (
            "geom-draw",
            "G387 G389 G390 G391 G394 G395 G396 G397 G398 G399 G400 G401 G402 G403 G404 G405",
        ),
        ("geom-dxf", "G430 G431 G432 G433"),
        ("geom-exchange", "G413 G414 G418 G436"),
        (
            "geom-field",
            "G326 G327 G328 G329 G330 G331 G332 G338 G339 G352 G469",
        ),
        ("geom-heal", "G417 G419 G420"),
        ("geom-ifc", "G421 G422 G423 G424 G426"),
        (
            "geom-intersect",
            "G158 G159 G162 G163 G164 G165 G166 G167 G168 G169 G170 G171 G172 G173 G174 G175 \
             G176 G292 G320 G321 G372 G390 G392 G393 G466 G467 G468",
        ),
        ("geom-landxml", "G437"),
        ("geom-make", "G101"),
        (
            "geom-mesh",
            "G146 G147 G148 G149 G150 G151 G152 G153 G288 G289 G290 G291 G298 G299 G300 G301 \
             G302 G303 G304 G305 G306 G319 G341 G350 G411",
        ),
        ("geom-meshio", "G354 G434 G435"),
        ("geom-offset", "G270 G275 G276 G277 G278 G279"),
        ("geom-p21", "G415"),
        ("geom-panel", "G342 G343 G344 G345"),
        ("geom-poly", "G123 G124 G125 G126 G127 G128 G142"),
        ("geom-profile", "G229 G230"),
        ("geom-skeleton", "G271 G272 G274 G471"),
        (
            "geom-sketch",
            "G216 G217 G218 G219 G220 G221 G222 G223 G224 G225 G226 G227 G228 G230 G477 G478",
        ),
        ("geom-spatial", "G293 G294 G295 G296 G297"),
        ("geom-step", "G427 G428 G429 G476"),
        ("geom-subd", "G315 G316 G319 G323 G325"),
        ("geom-sweep", "G255 G256 G257 G259 G260 G261 G280 G375"),
        (
            "geom-terrain",
            "G362 G363 G364 G365 G366 G367 G368 G369 G370 G374 G377 G380",
        ),
        ("engine-geometry", "G100 G482"),
    ];

    #[test]
    fn status_lines_name_exactly_the_pieces_of_the_build_order() {
        let listed: BTreeSet<&str> = STATUS_PIECES.iter().map(|(n, _)| *n).collect();
        let here: BTreeSet<&str> = scaffolded().into_iter().collect();
        assert_eq!(
            listed, here,
            "the embedded list and the table's Here rows differ"
        );
        let mut wrong = Vec::new();
        for (name, pieces) in STATUS_PIECES {
            let source = read(&format!("{}/src/lib.rs", crate_dir(name)));
            let lines = header_lines(&source);
            let got = paragraph(&lines, &["Status: scaffold only; built by pieces "])
                .map(|p| piece_ids(&p))
                .unwrap_or_default();
            let expected: BTreeSet<String> = pieces.split_whitespace().map(str::to_owned).collect();
            if got != expected {
                wrong.push(format!("{name}: header {got:?}, build order {expected:?}"));
            }
        }
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    /// For each crate, every chapter (below 26) cited by a piece of `30-build-order.json` whose
    /// crate field names that crate alone, with the pieces that cite it. Chapters 26 to 32 (growth,
    /// crates, testing, sources, build order, risks, decisions) are left out: every crate follows
    /// them.
    const CHAPTERS_CITED: &[(&str, &[(u8, &str)])] = &[
        ("geom-2d", &[(14, "G192-G213"), (17, "G360")]),
        ("geom-3dm", &[(16, "G438"), (24, "G438")]),
        ("geom-align", &[(17, "G373 G376")]),
        ("geom-analysis", &[(16, "G346 G347 G348 G349")]),
        ("geom-blend", &[(13, "G262 G263 G265 G266 G267 G268 G269")]),
        (
            "geom-bool",
            &[(11, "G129-G250"), (16, "G324"), (23, "G130 G131 G139")],
        ),
        ("geom-bvh", &[(11, "G54"), (19, "G54 G55"), (23, "G54")]),
        ("geom-check", &[(8, "G84 G85 G134 G177")]),
        ("geom-cloud", &[(17, "G379")]),
        ("geom-combine", &[(9, "G336"), (11, "G336"), (13, "G337")]),
        (
            "geom-convert",
            &[(9, "G335"), (13, "G340"), (16, "G333 G334 G335 G340 G351")],
        ),
        (
            "geom-curve",
            &[(7, "G61 G62 G63 G371"), (17, "G63 G64 G371")],
        ),
        ("geom-draw", &[(18, "G387-G404"), (23, "G402")]),
        ("geom-dxf", &[(14, "G430"), (24, "G430 G431 G432 G433")]),
        ("geom-exchange", &[(24, "G413 G414 G418")]),
        (
            "geom-field",
            &[(13, "G338 G339"), (16, "G326-G339 G469"), (19, "G352")],
        ),
        ("geom-format", &[(23, "G105"), (24, "G91-G95 G410")]),
        ("geom-heal", &[(24, "G417 G419 G420")]),
        (
            "geom-ifc",
            &[(13, "G421 G422"), (17, "G421 G424"), (24, "G421-G426")],
        ),
        (
            "geom-intersect",
            &[
                (12, "G158-G176 G320 G466 G467 G468"),
                (16, "G321"),
                (17, "G372"),
                (18, "G392 G393"),
                (19, "G292"),
            ],
        ),
        ("geom-landxml", &[(17, "G437"), (24, "G437")]),
        ("geom-linalg", &[(15, "G49 G50 G215"), (16, "G49 G50 G215")]),
        ("geom-make", &[(10, "G101")]),
        (
            "geom-mesh",
            &[(16, "G341"), (17, "G153"), (19, "G146-G306"), (23, "G291")],
        ),
        ("geom-meshio", &[(16, "G354"), (24, "G354 G434 G435")]),
        ("geom-name", &[(20, "G89"), (24, "G109")]),
        (
            "geom-nurbs",
            &[(7, "G56 G58"), (12, "G59"), (13, "G58"), (14, "G58")],
        ),
        ("geom-offset", &[(13, "G270-G279")]),
        (
            "geom-op",
            &[
                (10, "G82 G104"),
                (20, "G86"),
                (22, "G82"),
                (23, "G83 G104"),
                (24, "G86"),
            ],
        ),
        ("geom-p21", &[(24, "G415")]),
        ("geom-panel", &[(16, "G342 G343 G344 G345")]),
        (
            "geom-poly",
            &[(11, "G123-G128"), (13, "G142"), (19, "G127"), (23, "G123")],
        ),
        ("geom-profile", &[(15, "G229")]),
        ("geom-skeleton", &[(13, "G271 G272 G274 G471")]),
        ("geom-sketch", &[(15, "G216-G228 G477 G478")]),
        (
            "geom-spatial",
            &[(10, "G297"), (19, "G293-G297"), (23, "G295")],
        ),
        ("geom-step", &[(24, "G427 G428 G429 G476")]),
        ("geom-subd", &[(16, "G325")]),
        (
            "geom-surface",
            &[(7, "G65 G66"), (13, "G66 G253 G254"), (16, "G470")],
        ),
        (
            "geom-sweep",
            &[(13, "G255 G256 G257 G261 G280"), (17, "G375")],
        ),
        ("geom-terrain", &[(17, "G362-G370 G377")]),
        ("geom-topo", &[(8, "G80 G81"), (23, "G80 G145")]),
        ("engine-geometry", &[(22, "G100"), (25, "G100 G482")]),
    ];

    #[test]
    fn each_header_names_the_chapters_its_own_pieces_implement() {
        // The brief: "the sections it implements ... (chapter numbers are in the table and in
        // 30-build-order.json under each crate)". A chapter counts as named when its number
        // appears anywhere in the Sections paragraph, open questions included.
        let mut wrong = Vec::new();
        for (name, cited) in CHAPTERS_CITED {
            let source = read(&format!("{}/src/lib.rs", crate_dir(name)));
            let lines = header_lines(&source);
            let named = paragraph(&lines, &["Sections: ", "Map: "])
                .map(|p| chapters_named(&p))
                .unwrap_or_default();
            let missing: Vec<String> = cited
                .iter()
                .filter(|(chapter, _)| !named.contains(chapter))
                .map(|(chapter, pieces)| format!("chapter {chapter} (pieces {pieces})"))
                .collect();
            if !missing.is_empty() {
                wrong.push(format!(
                    "{name}: Sections name chapters {named:?}; missing {}",
                    missing.join(", ")
                ));
            }
        }
        assert!(
            wrong.is_empty(),
            "{} crate headers leave out chapters their own build pieces implement \
             (30-build-order.json, pieces whose crate field is that crate alone):\n{}",
            wrong.len(),
            wrong.join("\n")
        );
    }

    #[test]
    fn every_crate_of_the_accepted_table_is_here_or_under_way() {
        // G01's accepted table (chapter 27's first table), less the crates other pieces build.
        let accepted = "num math nurbs curve surface intersect topo name check poly bool sweep \
                        blend offset 2d sketch mesh spatial subd field cloud terrain align \
                        skeleton draw convert format ifc step dxf 3dm meshio test fuzz bench \
                        engine-geometry";
        let under_way = set("math test fuzz bench");
        let observed = repo::observe();
        let missing: Vec<String> = expand(accepted)
            .into_iter()
            .filter(|n| !under_way.contains(n) && !observed.packages.contains_key(n))
            .collect();
        assert!(missing.is_empty(), "missing: {missing:?}");
    }

    #[test]
    fn the_readme_lists_the_toolkits_not_built() {
        let readme = read("projects/engines/geometry/crates/README.md");
        let absent: Vec<&str> = [
            "geom-sheet",
            "geom-thread",
            "geom-board",
            "geom-stair",
            "geom-civil",
            "geom-assembly",
        ]
        .into_iter()
        .filter(|t| !readme.contains(&format!("`{t}`")))
        .collect();
        assert!(absent.is_empty(), "not in the README: {absent:?}");
    }

    // -------------------------------------------------------------- toolchain and CI

    fn quoted_after(text: &str, key: &str) -> Option<String> {
        let line = text.lines().find(|l| l.trim_start().starts_with(key))?;
        let rest = line.split_once(key)?.1;
        Some(rest.split('"').nth(1)?.to_owned())
    }

    #[test]
    fn the_three_toolchain_pins_are_equal() {
        let file = read("rust-toolchain.toml");
        let kernel = read(".github/workflows/kernel.yml");
        let build = read(".github/workflows/kernel-build.yml");
        let pins = [
            ("rust-toolchain.toml", quoted_after(&file, "channel =")),
            ("kernel.yml", quoted_after(&kernel, "RUST_TOOLCHAIN:")),
            ("kernel-build.yml", quoted_after(&build, "RUST_TOOLCHAIN:")),
        ];
        assert!(
            pins.iter().all(|(_, p)| p.as_deref() == Some("1.99.0")),
            "{pins:?}"
        );
        assert!(
            file.contains("components = [\"rustfmt\", \"clippy\"]"),
            "{file}"
        );
        assert!(
            file.contains("targets = [\"wasm32-wasip1\", \"wasm32-unknown-unknown\"]"),
            "{file}"
        );
        let root = read("Cargo.toml");
        assert!(
            root.contains("\"projects/engines/geometry/crates/*\""),
            "the root Cargo.toml lacks the member line"
        );
    }

    /// Every action a workflow uses, with the refs it pins it to.
    fn action_pins(yml: &str) -> BTreeMap<String, BTreeSet<String>> {
        let mut pins: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for line in yml.lines() {
            let line = line.trim_start();
            let line = line.strip_prefix("- ").unwrap_or(line);
            let Some(spec) = line.strip_prefix("uses:") else {
                continue;
            };
            let spec = spec.split_whitespace().next().unwrap_or_default();
            let (action, pin) = spec.split_once('@').unwrap_or((spec, ""));
            pins.entry(action.to_owned())
                .or_default()
                .insert(pin.to_owned());
        }
        pins
    }

    #[test]
    fn kernel_build_yml_does_what_the_brief_asks() {
        let yml = read(".github/workflows/kernel-build.yml");
        let mut wrong = Vec::new();
        for needle in [
            "  pull_request:\n    paths:",
            "  push:\n    branches: [main]\n    paths:",
            "permissions:\n  contents: read\n",
            "toolchain: ${{ env.RUST_TOOLCHAIN }}",
            "- x86_64-unknown-linux-gnu",
            "- wasm32-unknown-unknown",
            "- wasm32-wasip1",
            "cargo build --locked --target \"$TARGET\" --package \"$package\"",
            "run: cargo test --locked --package engine-geometry --test scaffold",
        ] {
            if !yml.contains(needle) {
                wrong.push(format!("missing {needle:?}"));
            }
        }
        // Both triggers carry the same path filters, and these among them.
        for path in [
            "\"projects/engines/geometry/**\"",
            "\"projects/engines/crates/engine-geometry/**\"",
            "\"projects/kit/crates/kit-core/**\"",
            "\"Cargo.lock\"",
            "\"rust-toolchain.toml\"",
            "\"**/Cargo.toml\"",
            "\".cargo/**\"",
            "\"**/clippy.toml\"",
            "\"**/.clippy.toml\"",
        ] {
            if yml.matches(path).count() != 2 {
                wrong.push(format!("{path} is not in both path filters"));
            }
        }
        if yml.contains("write") {
            wrong.push("a write permission".to_owned());
        }
        // The crates it builds come from the table itself (the scaffold test's ignored
        // `kernel_build_list`), so the workflow holds no second copy of the table to drift: no
        // quoted crate name, no hard-coded exclusion list. (This replaces the adversary's check
        // that a hard-coded exclusion list equalled the table's development rows: the fixer
        // removed that list, as the review asked.)
        if !yml.contains("--ignored --exact kernel_build_list") {
            wrong.push("the crate list does not come from the table".to_owned());
        }
        if yml.contains("\"geom-") || yml.contains("IN(") {
            wrong.push("the workflow names crates itself".to_owned());
        }
        // The no_std proof: the kernel crates build for a target without std.
        for needle in [
            "- wasm32v1-none",
            "PACKAGES: ${{ needs.list.outputs.no_std }}",
        ] {
            if !yml.contains(needle) {
                wrong.push(format!("missing {needle:?}"));
            }
        }
        // The development crates are not built: the list prints only kernel, pool and facade
        // rows, which the scaffold's own main.rs decides from each row's kind.
        let development: BTreeSet<&str> = table::TABLE
            .rows
            .iter()
            .filter(|r| r.kind == Kind::Development)
            .map(|r| r.name)
            .collect();
        assert_eq!(development.len(), 4, "{development:?}");
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    #[test]
    fn kernel_build_yml_pins_kernel_yml_commits_and_reruns_none_of_its_tests_or_lints() {
        // kernel.yml tests, formats and lints every crate of the kernel's folder on seven targets;
        // kernel-build.yml runs none of that again, and pins its actions as kernel.yml does. It
        // keeps its builds of each crate on its own, which kernel.yml, building its crates in one
        // cargo run with warnings allowed, does not do.
        let yml = read(".github/workflows/kernel-build.yml");
        let kernel = read(".github/workflows/kernel.yml");
        let mut wrong = Vec::new();
        // Every action pinned to a full commit, the one kernel.yml pins it to, and every checkout
        // leaves no token behind.
        let pins = action_pins(&yml);
        let kernel_pins = action_pins(&kernel);
        if pins.is_empty() {
            wrong.push("no action found".to_owned());
        }
        for (action, refs) in &pins {
            let full = refs.iter().all(|r| {
                r.len() == 40
                    && r.chars()
                        .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
            });
            if !full || kernel_pins.get(action) != Some(refs) || refs.len() != 1 {
                wrong.push(format!(
                    "{action} pinned to {refs:?}; kernel.yml pins it to {:?}; one full commit, \
                     the same in both, is required",
                    kernel_pins.get(action)
                ));
            }
        }
        let checkouts = yml.matches("- uses: actions/checkout@").count();
        if checkouts == 0 || yml.matches("persist-credentials: false").count() != checkouts {
            wrong.push("a checkout keeps its credentials".to_owned());
        }
        // None of kernel.yml's tests or lints is run again: no list of the kernel's folder, no test
        // but the facade's own (the scaffold check and the pieces' attack tests, which kernel.yml's
        // list of the kernel's folder never names; G10), and formatting and clippy only for the
        // table's crates outside the kernel's folder.
        if yml.contains("KERNEL_CRATES") {
            wrong.push("repeats kernel.yml's list of the kernel's folder".to_owned());
        }
        // Both per-crate build jobs, the targets with std and wasm32v1-none, deny warnings.
        if yml.matches("RUSTFLAGS: -D warnings").count() != 2 {
            wrong.push("a per-crate build job does not deny warnings".to_owned());
        }
        let tests = yml.matches("cargo test ").count();
        let facade_tests = yml
            .matches("cargo test --locked --package engine-geometry ")
            .count();
        if tests != facade_tests {
            wrong.push(format!(
                "{tests} cargo test runs, {facade_tests} of the facade"
            ));
        }
        for needle in [
            "if [ ! -d \"projects/engines/geometry/crates/$package\" ]; then",
            "PACKAGES: ${{ steps.outside.outputs.packages }}",
            "cargo fmt --check \"${args[@]}\"",
            "cargo clippy --locked --all-targets \"${args[@]}\" -- -D warnings",
        ] {
            if !yml.contains(needle) {
                wrong.push(format!("missing {needle:?}"));
            }
        }
        if yml
            .matches("PACKAGES: ${{ steps.outside.outputs.packages }}")
            .count()
            != 2
        {
            wrong.push("formatting and clippy do not both take the outside list".to_owned());
        }
        // What moved to kernel.yml is there: every crate of the kernel's folder tested natively on
        // six runners and on wasm32-wasip1, formatted and linted.
        for needle in [
            "for d in projects/engines/geometry/crates/*/",
            "os: ubuntu-latest",
            "os: ubuntu-24.04-arm",
            "os: windows-latest",
            "os: windows-11-arm",
            "os: macos-latest",
            "os: macos-15-intel",
            "run: cargo test $KERNEL_CRATES --release --locked --no-fail-fast",
            "run: cargo test $KERNEL_CRATES --locked --no-fail-fast",
            "run: cargo test $KERNEL_CRATES --target wasm32-wasip1 --release --locked --no-fail-fast",
            "run: cargo fmt --check $KERNEL_CRATES",
            "run: cargo clippy $KERNEL_CRATES --all-targets --locked -- -D warnings",
        ] {
            if !kernel.contains(needle) {
                wrong.push(format!("kernel.yml lacks {needle:?}"));
            }
        }
        assert!(wrong.is_empty(), "{wrong:#?}");
    }
}
