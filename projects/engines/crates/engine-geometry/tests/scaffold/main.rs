//! The kernel scaffold check (piece G09): the repository holds exactly chapter 27's crate table.
//!
//! Every crate folder under `projects/engines/geometry/crates`, and the facade, has a `lib.rs`
//! header stating its one job, its layer, its invariants and its sections; every kernel crate
//! forbids unsafe code and is `no_std` (`geom-pool` and the development crates may use `std`);
//! every crate's direct kernel dependencies are exactly its row of the table in `table`, less the
//! ones still waiting for a crate under way; no dependency forms a cycle; every edge goes to a
//! lower layer, or along an edge the layer table allows inside one layer; and nothing outside the
//! kernel depends on an internal `geom-*` crate. Chapter 27, "What Depends on means", asks CI to
//! check exactly this. It also checks that every crate that ships is built with exactly
//! `geom-num`'s lint settings, identical copies and not a subset (`lints` states the rule), and
//! that `rust-toolchain.toml` and the kernel workflows pin one compiler (`toolchain`).
//!
//! The text checks are fast and local; CI also proves `no_std` by building every kernel crate for
//! a target that has no `std` (`.github/workflows/kernel-build.yml`, which takes its list of
//! crates from `kernel_build_list` here, so the table stays in one place).
//!
//! The merge gate (piece G12, chapter 28.8; `.github/workflows/kernel-gate.yml`) adds its rules
//! here, beside the table they read: no crate that ships names a `static`, a thread-local or a
//! lazy global (`globals`); every crate that ships is built with chapter 21's disallowed methods
//! and types and chapter 27's denials, nothing in its settings lowers them, and no cargo
//! configuration file sits on the way to a crate of the family (`posture`); no crate that ships
//! lowers a denied lint in its source outside test code (`overrides`); the cargo-deny
//! configurations hold chapter 29.2's allow-list, the advisories, the allowed sources and the
//! dependency bans (`deny`); and the gate's own runs, which need tools a plain `cargo test` may
//! lack, are ignored tests the gate runs: cargo-deny over every crate of the family, with the
//! gate's own rules on the same graphs (`--ignored gate_deny`), the builds of every kernel crate
//! for `wasm32v1-none` and `wasm32-unknown-unknown` with every feature selection (`--ignored
//! gate_build`) and clippy over every crate that ships, likewise (`--ignored gate_clippy`), both
//! of which also hold every file the compiler read to the source rules, each with its seeded
//! violations (`deny`, `gate`). The recursion check (piece G13) runs there too: every recursive
//! cycle of the crates that ship has its depth bound stated and each bound's test runs on a small
//! `wasm32-wasip1` stack, and seeded workspaces fail it (`--ignored gate_recursion`,
//! `recursion`).
//!
//! The repository tests run natively only: they start cargo and read files. The table check and
//! the seeded tests of each rule run on every target.
//!
//! Run it with `cargo test -p engine-geometry --test scaffold`.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod check;
mod deny;
#[cfg(not(target_family = "wasm"))]
mod gate;
mod globals;
mod header;
#[cfg(not(target_family = "wasm"))]
mod json;
mod licence;
mod lints;
mod manifest;
mod overrides;
mod posture;
#[cfg(not(target_family = "wasm"))]
mod recursion;
#[cfg(not(target_family = "wasm"))]
mod repo;
mod seeded;
mod source;
mod table;
mod toolchain;

use check::Violation;

fn report(violations: &[Violation]) -> String {
    let mut text = format!("{} violation(s) of the crate table:", violations.len());
    for v in violations {
        text.push_str("\n  ");
        text.push_str(&v.to_string());
    }
    text
}

#[test]
fn the_crate_table_is_consistent() {
    let violations = check::table(&table::TABLE);
    assert!(violations.is_empty(), "{}", report(&violations));
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn the_repository_matches_the_crate_table() {
    let observed = repo::observe();
    let violations = check::workspace(&table::TABLE, &observed);
    assert!(violations.is_empty(), "{}", report(&violations));
    let present = table::TABLE
        .rows
        .iter()
        .filter(|row| observed.packages.contains_key(row.name))
        .count();
    println!(
        "{present} of the table's {} crates are in the workspace; every one matches its row",
        table::TABLE.rows.len()
    );
    // The header lines the table records as missing, shown on every run so they stay in sight
    // until the piece that may edit each header adds them (the check fails once one is stale).
    for gap in table::TABLE.header_gaps {
        println!(
            "recorded header gap: {} states no {:?} line: {}",
            table::full_name(gap.krate),
            gap.field,
            gap.reason
        );
    }
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn the_toolchain_pins_agree() {
    let (file, workflows) = repo::toolchain_files(toolchain::TOOLCHAIN_FILE);
    let workflows: Vec<(&str, &str)> = workflows
        .iter()
        .map(|(name, text)| (name.as_str(), text.as_str()))
        .collect();
    let violations = toolchain::pins(&file, &workflows);
    assert!(violations.is_empty(), "{}", report(&violations));
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn the_crates_that_ship_hold_no_global_state() {
    let observed = repo::observe();
    let sources = repo::crate_sources(&observed, &globals::covered(&table::TABLE));
    let violations = globals::check(&table::TABLE, &sources);
    assert!(violations.is_empty(), "{}", report(&violations));
    let files = sources
        .values()
        .fold(0usize, |count, files| count.saturating_add(files.len()));
    println!(
        "{} crates that ship, {files} source files: no static, thread-local or lazy global",
        sources.len()
    );
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn the_real_check_refuses_global_state_seeded_into_the_repository() {
    // The repository as read, with one more source file in a kernel crate and one in the facade,
    // in memory: each is refused, and nothing else.
    let observed = repo::observe();
    let mut sources = repo::crate_sources(&observed, &globals::covered(&table::TABLE));
    for (krate, text) in [
        ("geom-op", "pub static mut COUNT: u32 = 0;\n"),
        (
            "engine-geometry",
            "pub fn cache() -> &'static std::sync::OnceLock<u8> { todo!() }\n",
        ),
    ] {
        let mut path = sources[krate][0].path.clone();
        path.pop();
        path.push("seeded.rs".to_owned());
        sources
            .get_mut(krate)
            .unwrap()
            .push(globals::SourceFile::read(path, text));
    }
    let violations = globals::check(&table::TABLE, &sources);
    let crates: Vec<&str> = violations.iter().map(|v| v.krate.as_str()).collect();
    assert_eq!(
        crates,
        ["engine-geometry", "geom-op"],
        "{}",
        report(&violations)
    );
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn the_crates_that_ship_lower_no_denied_lint_in_their_source() {
    let observed = repo::observe();
    let sources = repo::crate_sources(&observed, &globals::covered(&table::TABLE));
    let violations = overrides::check(&table::TABLE, &sources);
    assert!(violations.is_empty(), "{}", report(&violations));
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn the_real_check_refuses_a_lint_override_seeded_into_the_repository() {
    // The repository as read, with one more source file in a kernel crate and one in the facade,
    // in memory, each lowering a denied lint outside test code: each is refused, and nothing else.
    let observed = repo::observe();
    let mut sources = repo::crate_sources(&observed, &globals::covered(&table::TABLE));
    for (krate, text) in [
        (
            "geom-op",
            "#![allow(clippy::unwrap_used)]\npub fn f(x: Option<u8>) -> u8 { x.unwrap() }\n",
        ),
        (
            "engine-geometry",
            "#[allow(clippy::restriction)]\npub fn g() {}\n",
        ),
    ] {
        let mut path = sources[krate][0].path.clone();
        path.pop();
        path.push("seeded.rs".to_owned());
        sources
            .get_mut(krate)
            .unwrap()
            .push(globals::SourceFile::read(path, text));
    }
    let violations = overrides::check(&table::TABLE, &sources);
    let crates: Vec<&str> = violations.iter().map(|v| v.krate.as_str()).collect();
    assert_eq!(
        crates,
        ["engine-geometry", "geom-op"],
        "{}",
        report(&violations)
    );
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn no_cargo_configuration_sits_on_the_way_to_the_family() {
    let observed = repo::observe();
    let dirs: Vec<Vec<String>> = table::TABLE
        .rows
        .iter()
        .filter_map(|row| observed.packages.get(row.name))
        .map(|package| package.dir.clone())
        .collect();
    assert!(
        !dirs.is_empty(),
        "no crate of the family is in the workspace"
    );
    let found = repo::cargo_config_files(&repo::workspace_root(), &dirs);
    let violations = posture::cargo_configs(&found);
    assert!(violations.is_empty(), "{}", report(&violations));
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn the_real_check_finds_a_cargo_configuration_on_the_way_to_a_crate() {
    // A copy of the way from a workspace root to a crate, in the test's temporary folder, with a
    // configuration in the root, one in a folder between and one beside it that cargo never reads
    // for the crate.
    let root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("gate-cargo-config");
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    let dir = ["projects", "engines", "geometry", "crates", "geom-x"].map(str::to_owned);
    std::fs::create_dir_all(dir.iter().fold(root.clone(), |p, c| p.join(c))).unwrap();
    for (folder, name) in [
        (&[][..], "config.toml"),
        (&["projects", "engines"][..], "config"),
        (&["projects", "kit"][..], "config.toml"),
    ] {
        let cargo = folder
            .iter()
            .fold(root.clone(), |p, c| p.join(c))
            .join(".cargo");
        std::fs::create_dir_all(&cargo).unwrap();
        std::fs::write(
            cargo.join(name),
            "[env]\nCLIPPY_CONF_DIR = { value = \"x\", relative = true }\n",
        )
        .unwrap();
    }
    let found = repo::cargo_config_files(&root, &[dir.to_vec()]);
    let violations = posture::cargo_configs(&found);
    let files: Vec<&str> = violations.iter().map(|v| v.krate.as_str()).collect();
    assert_eq!(
        files,
        [".cargo/config.toml", "projects/engines/.cargo/config"],
        "{}",
        report(&violations)
    );
    assert!(
        violations
            .iter()
            .all(|v| v.rule == check::Rule::CargoConfig),
        "{}",
        report(&violations)
    );
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn the_crates_that_ship_hold_chapter_21s_lists_and_chapter_27s_denials() {
    let violations = posture::check(&table::TABLE, &repo::observe());
    assert!(violations.is_empty(), "{}", report(&violations));
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn the_real_check_refuses_a_lint_rule_dropped_from_the_repository() {
    // The repository as read, with one platform maths function dropped from geom-op's clippy
    // configuration and one denial loosened in geom-bvh's manifest, in memory.
    let mut observed = repo::observe();
    let methods = observed
        .packages
        .get_mut("geom-op")
        .and_then(|p| p.lints.clippy.as_mut())
        .and_then(|c| c.settings.as_mut())
        .and_then(|s| s.get_mut("disallowed-methods"))
        .unwrap();
    let sin = "{path=\"f32::sin\",reason=\"platform maths; use geom_num::math (chapter 21)\"},";
    assert!(methods.contains(sin), "{methods}");
    *methods = methods.replace(sin, "");
    observed
        .packages
        .get_mut("geom-bvh")
        .and_then(|p| p.lints.tables.as_mut())
        .unwrap()
        .insert("lints.clippy.panic".to_owned(), "\"warn\"".to_owned());
    let violations = posture::check(&table::TABLE, &observed);
    let found: Vec<(&str, check::Rule)> = violations
        .iter()
        .map(|v| (v.krate.as_str(), v.rule))
        .collect();
    assert_eq!(
        found,
        [
            ("geom-bvh", check::Rule::PostureLacks),
            ("geom-op", check::Rule::PostureLacks)
        ],
        "{}",
        report(&violations)
    );
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn the_cargo_deny_configurations_hold_the_gate_rules() {
    let root = repo::workspace_root();
    let mut problems = Vec::new();
    for (file, check) in [
        (
            deny::FAMILY_CONFIG,
            deny::family_config as fn(&str) -> Vec<String>,
        ),
        (deny::NO_STD_CONFIG, deny::no_std_config),
        (deny::SHIPS_CONFIG, deny::ships_config),
    ] {
        let text = std::fs::read_to_string(root.join(file))
            .unwrap_or_else(|e| panic!("cannot read {file}: {e}"));
        for problem in check(&text) {
            problems.push(check::Violation {
                krate: file.to_owned(),
                rule: check::Rule::DenyConfig,
                detail: problem,
            });
        }
    }
    assert!(problems.is_empty(), "{}", report(&problems));
}

/// Not a check: prints, from the table, the crates `kernel-build.yml` builds, one per line, so the
/// workflow holds no second copy of the table. `no_std` lines are the kernel crates, which also
/// build for a target without `std`; `std` lines are `geom-pool` and the facade. The binding crates
/// get their own workflow with phase 13, and the development crates never ship.
#[cfg(not(target_family = "wasm"))]
#[test]
#[ignore = "kernel-build.yml runs it to list the crates it builds"]
fn kernel_build_list() {
    let observed = repo::observe();
    // The test harness prints "test kernel_build_list ... " without a line end first: start each
    // entry on a line of its own.
    println!();
    for row in table::TABLE
        .rows
        .iter()
        .filter(|row| observed.packages.contains_key(row.name))
    {
        let class = match row.kind {
            table::Kind::Kernel => "no_std",
            table::Kind::KernelStd | table::Kind::Facade => "std",
            table::Kind::Binding | table::Kind::Development => continue,
        };
        println!("kernel-build: {class} {}", row.name);
    }
}
