//! Every outside package a development crate brings into a build is permissive and recorded, with
//! its licence, in that crate's header; and every record names such a package.
//!
//! For each of `geom-test`, `geom-fuzz` and `geom-gen`, the test runs
//! `cargo tree --target all -e normal,build,dev --no-dedupe` (every platform's dependencies, the
//! crate's own dev-dependencies, every path to every package) and takes the packages the crate
//! brings in itself: every package in the tree outside the subtrees of our own path crates, whose
//! headers record theirs. Each must have a permissive licence (`geom_test::licence`), our own crates
//! must be MPL-2.0 (or MPL-2.0 AND permissive licences, for a crate that holds code ported under
//! them, as geom-num holds CORE-MATH's under MIT), and each must appear in a record of the crate's `Licences` section under an
//! expression of the same meaning. A record naming a package the crate does not bring in fails too,
//! so the records can neither fall behind nor run ahead of `Cargo.lock`.

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

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use geom_test::licence::{Expr, records};

/// The development crates whose headers keep licence records.
const CRATES: [&str; 3] = ["geom-test", "geom-fuzz", "geom-gen"];

/// The folder holding every geometry crate.
fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

/// One line of `cargo tree --prefix depth --format "{p}|{l}"`.
#[derive(Debug)]
struct Line {
    depth: usize,
    name: String,
    /// Our own path crate: `{p}` ends in its folder in parentheses.
    local: bool,
    licence: String,
}

fn parse_line(line: &str) -> Line {
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    let depth = line[..digits].parse().unwrap();
    let (package, licence) = line[digits..].split_once('|').unwrap();
    let name = package.split(' ').next().unwrap().to_owned();
    // `{p}` is "name vX.Y.Z", then " (<folder>)" for a path package, " (<url>)" for a git package,
    // " (registry `<name>`)" for another registry's, and " (proc-macro)" for a procedural macro.
    let local = package.split_once(" (").is_some_and(|(_, rest)| {
        !rest.starts_with("proc-macro)") && !rest.starts_with("registry ") && !rest.contains("://")
    });
    Line {
        depth,
        name,
        local,
        licence: licence.trim().to_owned(),
    }
}

/// The outside packages `krate` brings in, with their licences, and our own crates in its tree.
fn brought_in(krate: &str) -> (BTreeMap<String, BTreeSet<String>>, BTreeMap<String, String>) {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from(env!("CARGO")));
    let out = Command::new(cargo)
        .args([
            "tree",
            "--locked",
            "-p",
            krate,
            "--target",
            "all",
            "-e",
            "normal,build,dev",
            "--no-dedupe",
            "--prefix",
            "depth",
            "--format",
            "{p}|{l}",
            "--manifest-path",
        ])
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "cargo tree failed for {krate}:\n{stdout}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let mut outside: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut ours: BTreeMap<String, String> = BTreeMap::new();
    // While inside the subtree of one of our crates (other than the root): its depth.
    let mut skip_below: Option<usize> = None;
    for raw in stdout.lines().filter(|l| !l.trim().is_empty()) {
        let line = parse_line(raw);
        if let Some(d) = skip_below {
            if line.depth > d {
                continue;
            }
            skip_below = None;
        }
        if line.local {
            ours.insert(line.name.clone(), line.licence.clone());
            if line.depth > 0 {
                skip_below = Some(line.depth);
            }
        } else {
            outside.entry(line.name).or_default().insert(line.licence);
        }
    }
    assert!(ours.contains_key(krate), "{krate} is not in its own tree");
    (outside, ours)
}

/// Whether one of our crates' licence is MPL-2.0: alone, or with permissive licences beside it
/// (`MPL-2.0 AND MIT`), for code ported into the crate under its own licence.
fn ours_is_mpl(licence: &str) -> bool {
    let mpl = |e: &Expr| matches!(e, Expr::Licence { id, exception: None } if id == "MPL-2.0");
    match Expr::parse(licence) {
        Ok(e) if mpl(&e) => true,
        Ok(Expr::And(terms)) => {
            terms.iter().any(mpl) && terms.iter().all(|t| mpl(t) || t.permissive())
        }
        _ => false,
    }
}

#[test]
fn our_crates_are_mpl_with_only_permissive_licences_beside_it() {
    assert!(ours_is_mpl("MPL-2.0"));
    assert!(ours_is_mpl("MPL-2.0 AND MIT"));
    assert!(ours_is_mpl("MIT AND MPL-2.0"));
    assert!(!ours_is_mpl("MIT"));
    assert!(!ours_is_mpl("MPL-2.0 OR MIT"));
    assert!(!ours_is_mpl("MPL-2.0 AND GPL-3.0-only"));
    assert!(!ours_is_mpl("MPL-2.0 WITH LLVM-exception"));
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "runs cargo as a child process, which WebAssembly cannot"
)]
#[test]
fn every_outside_package_is_permissive_and_recorded() {
    let mut problems = Vec::new();
    for krate in CRATES {
        let (outside, ours) = brought_in(krate);
        for (name, licence) in &ours {
            if !ours_is_mpl(licence) {
                problems.push(format!(
                    "{krate}: our crate {name} is {licence}, not MPL-2.0 (with only permissive                      licences beside it)"
                ));
            }
        }
        let lib = crates_dir().join(krate).join("src").join("lib.rs");
        let source = std::fs::read_to_string(&lib).unwrap();
        let recorded = records(&source).unwrap_or_else(|e| panic!("{}: {e}", lib.display()));
        let mut recorded_names = BTreeSet::new();
        for r in &recorded {
            for p in &r.packages {
                recorded_names.insert(p.clone());
            }
        }
        for (name, licences) in &outside {
            for licence in licences {
                let expr = match Expr::parse(licence) {
                    Ok(e) => e,
                    Err(e) => {
                        problems.push(format!("{krate}: {name}: {e}"));
                        continue;
                    }
                };
                if !expr.permissive() {
                    problems.push(format!(
                        "{krate}: {name} is {licence}, which is not permissive"
                    ));
                }
                let canonical = expr.canonical();
                let ok = recorded.iter().any(|r| {
                    r.expression.canonical() == canonical && r.packages.iter().any(|p| p == name)
                });
                if !ok {
                    problems.push(format!(
                        "{krate}: {name} ({licence}) has no record in {}'s Licences section",
                        lib.display()
                    ));
                }
            }
        }
        for name in &recorded_names {
            if !outside.contains_key(name) {
                problems.push(format!(
                    "{krate}: the Licences section records {name}, which {krate} does not bring in"
                ));
            }
        }
        println!(
            "{krate}: {} outside packages, all recorded and permissive unless listed below",
            outside.len()
        );
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

/// The tree reader sees through the format it relies on: depth digits, our crates by folder, and
/// procedural macros that are not ours.
#[test]
fn tree_lines_are_read_as_cargo_writes_them() {
    let l = parse_line("2serde_derive v1.0.229 (proc-macro)|MIT OR Apache-2.0");
    assert_eq!(
        (l.depth, l.name.as_str(), l.local),
        (2, "serde_derive", false)
    );
    let l = parse_line("11geom-num v0.0.0 (C:\\x\\geom-num)|MPL-2.0");
    assert_eq!((l.depth, l.name.as_str(), l.local), (11, "geom-num", true));
    assert_eq!(l.licence, "MPL-2.0");
    let l = parse_line("0zmij v1.0.23|MIT");
    assert_eq!((l.depth, l.local, l.licence.as_str()), (0, false, "MIT"));
    let l = parse_line("3forked v0.1.0 (https://github.com/x/y#0123abcd)|MIT");
    assert!(!l.local);
    let l = parse_line("3other v0.1.0 (registry `mirror`)|MIT");
    assert!(!l.local);
    let l = parse_line("3our-macro v0.0.0 (/home/x/our-macro) (proc-macro)|MPL-2.0");
    assert!(l.local);
}
