//! No generator hides a seed or reads the world: the rule `generator.rs` states, checked on the
//! source, because Rust's types cannot check it.
//!
//! A generator module is every file in `src/` except the framework (`lib.rs`, `main.rs`,
//! `files.rs`, `generator.rs`, `registry.rs`). In its code before `#[cfg(test)]`, comments
//! skipped, it may not:
//!
//! - make or name a seed (`Seed(`, `Seed {`, `Seed::`, `seed::`), or build a `SplitMix64` from
//!   anything but the `seed` its function receives (`SplitMix64::new(seed)`), or use another
//!   source of randomness (`rand::`, `getrandom`, `RandomState`, `ByteSource`);
//! - read the environment, a file, a clock or another process (`std::env`, `env!`, `option_env!`,
//!   `std::fs`, `include_bytes!`, `include_str!`, `std::io`, `std::time`, `SystemTime`,
//!   `Instant`, `std::process`, `std::thread`, `std::net`);
//! - iterate a hash map or set, whose order is not fixed (`HashMap`, `HashSet`).
//!
//! In `registry.rs`, a seed may be written only as the seed of a `Generate::Seeded` entry.
//!
//! A pattern list can be evaded on purpose; it is here to stop accidents and to make a deliberate
//! evasion visible in review, beside the rule in `generator.rs`.

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

use std::path::{Path, PathBuf};

/// The framework files, which may read files and the environment and handle seeds.
const FRAMEWORK: [&str; 5] = [
    "lib.rs",
    "main.rs",
    "files.rs",
    "generator.rs",
    "registry.rs",
];

/// What a generator module may not contain.
const FORBIDDEN: [&str; 25] = [
    "Seed(",
    "Seed {",
    "Seed::",
    "seed::",
    "rand::",
    "getrandom",
    "RandomState",
    "ByteSource",
    "Entropy",
    "std::env",
    "env!",
    "option_env!",
    "std::fs",
    "include_bytes!",
    "include_str!",
    "std::io",
    "std::time",
    "SystemTime",
    "Instant",
    "std::process",
    "std::thread",
    "std::net",
    "HashMap",
    "HashSet",
    "unsafe",
];

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// The code of a source file before `#[cfg(test)]`, with comment lines and trailing comments
/// removed, as (line number, text).
fn code(path: &Path) -> Vec<(usize, String)> {
    let source = std::fs::read_to_string(path).unwrap();
    let mut out = Vec::new();
    for (n, line) in source.lines().enumerate() {
        if line.contains("#[cfg(test)]") {
            break;
        }
        let t = line.trim_start();
        if t.starts_with("//") {
            continue;
        }
        let text = match t.find(" //") {
            Some(i) if !t[..i].contains('"') => &t[..i],
            _ => t,
        };
        out.push((n + 1, text.to_owned()));
    }
    out
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the crate's source from the host, which WASI cannot without a preopened folder"
)]
#[test]
fn no_generator_hides_a_seed_or_reads_the_world() {
    let mut modules: Vec<PathBuf> = std::fs::read_dir(src())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "rs"))
        .filter(|p| !FRAMEWORK.contains(&p.file_name().unwrap().to_str().unwrap()))
        .collect();
    modules.sort();
    assert!(
        modules.iter().any(|p| p.ends_with("grid_steps.rs")),
        "{modules:?}"
    );
    let mut hits = Vec::new();
    for path in &modules {
        for (n, line) in code(path) {
            for f in FORBIDDEN {
                if line.contains(f) {
                    hits.push(format!("{}:{n}: {f}: {line}", path.display()));
                }
            }
            let mut rest = line.as_str();
            while let Some(i) = rest.find("SplitMix64::new(") {
                if !rest[i..].starts_with("SplitMix64::new(seed)") {
                    hits.push(format!(
                        "{}:{n}: SplitMix64 from something other than the received seed: {line}",
                        path.display()
                    ));
                }
                rest = &rest[i + 1..];
            }
        }
    }
    assert!(hits.is_empty(), "\n{}", hits.join("\n"));
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the crate's source from the host, which WASI cannot without a preopened folder"
)]
#[test]
fn registry_seeds_are_only_generator_seeds() {
    let mut hits = Vec::new();
    for (n, line) in code(&src().join("registry.rs")) {
        let mut rest = line.as_str();
        while let Some(i) = rest.find("Seed(") {
            let before = &rest[..i];
            if !before.ends_with("Generate::Seeded(") && !before.ends_with("Seeded(") {
                hits.push(format!("registry.rs:{n}: {line}"));
            }
            rest = &rest[i + 1..];
        }
        for f in ["Seed::", "seed::", "Seed {"] {
            if line.contains(f) {
                hits.push(format!("registry.rs:{n}: {f}: {line}"));
            }
        }
    }
    assert!(hits.is_empty(), "\n{}", hits.join("\n"));
}

/// The scanner sees what it must: code lines before the tests, without their comments.
#[cfg_attr(
    target_family = "wasm",
    ignore = "writes a scratch file on the host, which WASI cannot without a preopened folder"
)]
#[test]
fn the_scanner_finds_what_it_looks_for() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("geom-gen-purity-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("sneaky.rs");
    std::fs::write(
        &file,
        "//! header with Seed( in a comment\n\
         let s = Seed(7); // a seed\n\
         let r = SplitMix64::new(other);\n\
         let ok = SplitMix64::new(seed);\n\
         #[cfg(test)]\n\
         let in_tests = Seed(1);\n",
    )
    .unwrap();
    let lines = code(&file);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert!(lines[0].1.contains("Seed(7)"));
    assert!(!lines[0].1.contains("a seed"));
    assert!(lines[1].1.contains("SplitMix64::new(other)"));
}
