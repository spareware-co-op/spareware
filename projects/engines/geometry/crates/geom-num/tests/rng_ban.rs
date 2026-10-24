//! The shape of the randomness ban in `clippy.toml` (rule 1 of the randomised-method rule in
//! `src/rng.rs`), read from the file itself.
//!
//! Clippy checks that each banned path is not called; it cannot check the list. These tests keep
//! the list honest where a mistake would be silent:
//!
//! - **No std or core entry carries `allow-invalid`**, except the `core::arch` ones that exist only
//!   on x86. Without it, a toolchain that renames a std path makes clippy print a warning naming
//!   the entry, instead of the ban lapsing in silence. It is only a warning (under clippy 0.1.99,
//!   `-D warnings` does not turn it into an error), so failing CI on it belongs to the merge gate
//!   (piece G12). Every other crate's entry carries it, because kernel crates do not depend on
//!   those crates, so their paths need not resolve. A path on a built-in type, such as chapter 21's
//!   bans of `f64::max` and the platform maths `f64::sin`, names no crate and always resolves, so
//!   it is held to the std rule.
//! - **Every entry has a reason**, and no path is listed twice.
//! - **Rule 1 names exactly the crates the file bans**, so the rule and the ban cannot drift apart.
//!
//! The same lists hold chapter 21's other bans (maths functions, `max` and `min`, and reductions
//! whose order is not written out), which are not randomness. The file is read by the `toml` crate
//! (MIT OR Apache-2.0, a development dependency; piece G12a put it in place of a line reader), so
//! an entry may be laid out and commented as TOML allows, and is read as clippy reads it: a path,
//! or a table with a `path`, a `reason` and an `allow-invalid`.
//!
//! Whether each path still resolves and fires is clippy's to show, on a seeded violation; that
//! check belongs to the merge gate (piece G12).

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them to build their oracles, models and generators.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "oracles, models and generators; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a malformed file must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;

/// The crate's `clippy.toml`.
const CLIPPY_TOML: &str = include_str!("../clippy.toml");

/// The generator's module, whose header holds rule 1.
const RNG_RS: &str = include_str!("../src/rng.rs");

/// One entry of a disallowed list.
#[derive(Debug)]
struct Entry {
    path: String,
    reason: String,
    allow_invalid: bool,
}

/// The entries of one list, `disallowed-types` or `disallowed-methods`, as clippy reads them: a
/// path string (no reason, invalid paths not allowed), or a table with a `path` and, optionally, a
/// `reason` and an `allow-invalid` flag.
fn entries(list: &str) -> Vec<Entry> {
    let config: toml::Table = CLIPPY_TOML.parse().expect("clippy.toml is not TOML");
    let items = config
        .get(list)
        .and_then(toml::Value::as_array)
        .unwrap_or_else(|| panic!("clippy.toml has no {list} list"));
    items
        .iter()
        .map(|item| match item {
            toml::Value::String(path) => Entry {
                path: path.clone(),
                reason: String::new(),
                allow_invalid: false,
            },
            toml::Value::Table(entry) => Entry {
                path: entry
                    .get("path")
                    .and_then(toml::Value::as_str)
                    .unwrap_or_else(|| panic!("{list}: an entry without a path: {item}"))
                    .to_owned(),
                reason: entry
                    .get("reason")
                    .and_then(toml::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                allow_invalid: entry.get("allow-invalid").is_some_and(|flag| {
                    flag.as_bool()
                        .unwrap_or_else(|| panic!("{list}: allow-invalid is not a boolean: {item}"))
                }),
            },
            other => panic!("{list}: an entry is a path or a table, not {other}"),
        })
        .collect()
}

/// Both lists.
fn all_entries() -> Vec<Entry> {
    let mut all = entries("disallowed-types");
    all.extend(entries("disallowed-methods"));
    all
}

/// The first segment of a path: a crate, or a built-in type.
fn krate(path: &str) -> &str {
    path.split("::").next().unwrap()
}

/// The language's built-in types, whose methods a path such as `f64::max` names. They belong to no
/// crate a kernel crate depends on, and they always resolve.
const BUILT_IN_TYPES: [&str; 17] = [
    "bool", "char", "str", "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16",
    "u32", "u64", "u128", "usize",
];

/// Whether a path's first segment is std, core, alloc or a built-in type: always there, so its
/// paths must resolve.
fn always_present(first: &str) -> bool {
    matches!(first, "std" | "core" | "alloc") || BUILT_IN_TYPES.contains(&first)
}

#[test]
fn only_paths_that_need_not_resolve_carry_allow_invalid() {
    let all = all_entries();
    assert!(all.len() > 50, "only {} entries were read", all.len());
    for e in &all {
        let std_or_core = always_present(krate(&e.path));
        let arch = e.path.starts_with("core::arch::");
        let should = arch || !std_or_core;
        assert_eq!(
            e.allow_invalid, should,
            "{}: allow-invalid is {}, but it belongs only on paths outside std, core and the \
             built-in types (and on core::arch, which exists only on its own architecture)",
            e.path, e.allow_invalid
        );
    }
}

#[test]
fn every_entry_has_a_reason_and_none_is_listed_twice() {
    for list in ["disallowed-types", "disallowed-methods"] {
        let mut seen = BTreeSet::new();
        for e in entries(list) {
            assert!(
                e.reason.len() > 20,
                "{list}: {} has no real reason ({:?})",
                e.path,
                e.reason
            );
            assert!(
                seen.insert(e.path.clone()),
                "{list}: {} is listed twice",
                e.path
            );
        }
    }
}

/// The crates besides std and core that rule 1 names.
const RULE_1_CRATES: [&str; 7] = [
    "fastrand",
    "foldhash",
    "getrandom",
    "hashbrown",
    "rand",
    "rand_core",
    "rayon",
];

/// Rule 1 names the crates whose entry points are banned; the file bans exactly those, besides std
/// and core.
#[test]
fn rule_1_names_exactly_the_crates_the_ban_lists() {
    let banned: BTreeSet<String> = all_entries()
        .iter()
        .map(|e| krate(&e.path).to_owned())
        .filter(|k| !always_present(k))
        .collect();
    let named: BTreeSet<String> = RULE_1_CRATES.iter().map(|k| (*k).to_owned()).collect();
    assert_eq!(
        banned, named,
        "clippy.toml bans these crates; rule 1 names those"
    );
    let rule_1 = RNG_RS
        .split("//! 1. **One generator.**")
        .nth(1)
        .and_then(|rest| rest.split("//! 2. ").next())
        .expect("rng.rs has rule 1");
    // Each name in backticks, as rule 1 writes it: a bare substring would always find `rand`,
    // inside "random" and "rand_core", whether or not the rule names the crate.
    for k in RULE_1_CRATES {
        assert!(
            rule_1.contains(&format!("`{k}`")),
            "clippy.toml bans {k}, but rule 1 does not name it as `{k}`"
        );
    }
}
