//! Chapter 23.4's ban on atomic counters, held over every source file of the crate.
//!
//! Chapter 23.4 forbids atomic counters in kernel crates: an index or ID taken from a shared
//! counter depends on scheduling, so the helper gives indices through `count_then_fill` instead.
//! Until this crate took the kernel's one lint posture, its clippy.toml banned the six atomic
//! integer types a counter would use (of `usize`, `isize`, `u64`, `i64`, `u32` and `i32`) over the
//! crate's code, tests and benchmark. The posture requires its clippy.toml to hold exactly
//! geom-num's settings (architecture chapter 27; G09's scaffold check), and geom-num's did not name
//! those types, so the ban moved here, over the same files. Since piece G33 the kernel's shared
//! settings ban the six types too, in every kernel crate's code and tests; this check stays, since
//! it also reads comments and holds without clippy. `CancelToken`'s `AtomicBool` is a flag, not a
//! counter, and stays allowed, as it was.
//!
//! What this file proves: no source file of the crate, its tests or its benchmark names one of
//! the six types, in code or in comments, by whatever path; the check finds each of them and lets
//! the flag through; and, where the file system can be read, every `.rs` file in the crate's
//! folder is one of the files checked, so a new file cannot escape the check.
//!
//! The six names are built from pieces, so this file, which is checked too, never writes them.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them to build their cases and oracles.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "test cases and oracles; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Every source file of the crate, its tests and its benchmark: its path in the crate's folder,
/// and its text, read when the test is built, so the check runs on every target.
const SOURCES: [(&str, &str); 24] = [
    ("benches/helper.rs", include_str!("../benches/helper.rs")),
    ("src/batch.rs", include_str!("../src/batch.rs")),
    ("src/budget.rs", include_str!("../src/budget.rs")),
    ("src/cancel.rs", include_str!("../src/cancel.rs")),
    ("src/error.rs", include_str!("../src/error.rs")),
    ("src/exec.rs", include_str!("../src/exec.rs")),
    ("src/lib.rs", include_str!("../src/lib.rs")),
    ("src/meter.rs", include_str!("../src/meter.rs")),
    ("src/phase.rs", include_str!("../src/phase.rs")),
    ("src/plan.rs", include_str!("../src/plan.rs")),
    ("src/pool.rs", include_str!("../src/pool.rs")),
    ("src/scan.rs", include_str!("../src/scan.rs")),
    ("src/sort.rs", include_str!("../src/sort.rs")),
    ("tests/attack_g04.rs", include_str!("attack_g04.rs")),
    ("tests/attack_g23.rs", include_str!("attack_g23.rs")),
    ("tests/budget_cancel.rs", include_str!("budget_cancel.rs")),
    ("tests/common/mod.rs", include_str!("common/mod.rs")),
    ("tests/determinism.rs", include_str!("determinism.rs")),
    ("tests/golden.rs", include_str!("golden.rs")),
    ("tests/helpers.rs", include_str!("helpers.rs")),
    ("tests/interface.rs", include_str!("interface.rs")),
    ("tests/kernel_rules.rs", include_str!("kernel_rules.rs")),
    ("tests/plan.rs", include_str!("plan.rs")),
    ("tests/properties.rs", include_str!("properties.rs")),
];

/// The atomic integer types a counter would use: the six the crate's clippy.toml banned.
const COUNTERS: [&str; 6] = [
    concat!("Atomic", "Usize"),
    concat!("Atomic", "Isize"),
    concat!("Atomic", "U64"),
    concat!("Atomic", "I64"),
    concat!("Atomic", "U32"),
    concat!("Atomic", "I32"),
];

/// Every line of `text` that names a banned type, as (line number from 1, the name), in order.
fn named(text: &str) -> Vec<(usize, &'static str)> {
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        for name in COUNTERS {
            if line.contains(name) {
                found.push((index + 1, name));
            }
        }
    }
    found
}

#[test]
fn no_source_file_names_an_atomic_counter_type() {
    let mut found = Vec::new();
    for (path, text) in SOURCES {
        for (line, name) in named(text) {
            found.push(format!("{path}:{line}: {name}"));
        }
    }
    assert!(
        found.is_empty(),
        "chapter 23.4 forbids atomic counters in kernel crates, and this crate's tests keep the \
         same rule: {found:?}"
    );
}

/// The check finds each banned type, however it is reached, and lets the flag through.
#[test]
fn the_check_finds_every_counter_type_and_lets_the_flag_through() {
    for name in COUNTERS {
        let text = format!(
            "use core::sync::atomic::{{AtomicBool, Ordering}};\n\
             static FLAG: AtomicBool = AtomicBool::new(false);\n\
             static COUNT: std::sync::atomic::{name} = std::sync::atomic::{name}::new(0);\n"
        );
        assert_eq!(named(&text), [(3, name)], "{name}");
    }
    let flag_only = "use core::sync::atomic::{AtomicBool, Ordering};";
    assert_eq!(named(flag_only), Vec::<(usize, &str)>::new());
}

/// The path of `file` in the crate's folder `root`, with `/` between its parts.
fn relative(root: &Path, file: &Path) -> String {
    let parts: Vec<&str> = file
        .strip_prefix(root)
        .unwrap()
        .components()
        .map(|part| part.as_os_str().to_str().unwrap())
        .collect();
    parts.join("/")
}

/// Every `.rs` file in the crate's folder is one of `SOURCES`, so none escapes the check.
#[test]
#[cfg_attr(
    target_os = "wasi",
    ignore = "reads the crate's folder; wasm32-wasip1 under wasmtime is given no folder to read"
)]
#[cfg_attr(
    miri,
    ignore = "reads the crate's folder; Miri isolates the test from the file system"
)]
fn every_source_file_in_the_crates_folder_is_checked() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut found = BTreeSet::new();
    let mut folders: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                folders.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.insert(relative(root, &path));
            }
        }
    }
    let checked: BTreeSet<String> = SOURCES.iter().map(|(path, _)| (*path).to_owned()).collect();
    assert_eq!(
        found, checked,
        "the crate's .rs files and the files checked differ"
    );
}
