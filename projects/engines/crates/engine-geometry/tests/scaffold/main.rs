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
mod header;
#[cfg(not(target_family = "wasm"))]
mod json;
mod lints;
mod manifest;
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
