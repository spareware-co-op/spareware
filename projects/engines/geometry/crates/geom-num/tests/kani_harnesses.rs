//! Every Kani harness beside `geom-num`'s code is proven in CI (piece G41; chapter 28.7).
//!
//! Each module's `#[cfg(kani)] mod proofs` holds bounded proofs that Kani checks on Linux, in
//! `kernel-fuzz.yml`'s `kani-num` job, one harness per matrix entry. A harness missing from the
//! matrix would never run; an entry naming no harness would fail in CI only. This test holds the
//! two to each other: the harnesses in the source, as `<module>::proofs::<name>`, are exactly the
//! job's `proof` list, except the generated limb variant's (piece G42), which the `kani-limbs` job
//! proves all at once by their module's prefix.

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

/// The workflow that proves them.
const WORKFLOW: &str = include_str!("../../../../../../.github/workflows/kernel-fuzz.yml");

/// The harnesses in `text`, the source of module `module`: every function after `#[kani::proof]`.
fn harnesses(module: &str, text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut marked = false;
    for line in text.lines() {
        let t = line.trim();
        if t == "#[kani::proof]" {
            marked = true;
        } else if marked {
            if let Some(rest) = t.strip_prefix("fn ") {
                let name = rest.split('(').next().unwrap();
                out.push(format!("{module}::proofs::{name}"));
                marked = false;
            } else if !t.starts_with("#[") {
                marked = false;
            }
        }
    }
    out
}

/// The `kani-num` job's `proof` list.
fn matrix(yaml: &str) -> BTreeSet<String> {
    let yaml = yaml.replace("\r\n", "\n");
    let job = yaml
        .split("\n  kani-num:\n")
        .nth(1)
        .expect("the kani-num job");
    let list = job
        .split("        proof:\n")
        .nth(1)
        .expect("its proof list");
    list.lines()
        .map_while(|l| l.strip_prefix("          - "))
        .map(str::to_owned)
        .collect()
}

/// Every `.rs` file under `dir`, with its module path relative to `src`.
fn sources(src: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut folders: Vec<PathBuf> = vec![src.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                folders.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let module = path
                    .strip_prefix(src)
                    .unwrap()
                    .with_extension("")
                    .components()
                    .map(|c| c.as_os_str().to_str().unwrap().to_owned())
                    .collect::<Vec<_>>()
                    .join("::");
                out.push((module, std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    out
}

#[test]
#[cfg_attr(
    target_os = "wasi",
    ignore = "reads the crate's folder; wasm32-wasip1 under wasmtime is given no folder to read"
)]
fn every_harness_is_in_the_kani_job_and_every_entry_is_a_harness() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let found: BTreeSet<String> = sources(&src)
        .iter()
        .flat_map(|(module, text)| harnesses(module, text))
        .collect();
    assert!(found.len() >= 15, "{found:?}");
    let (limbs, rest): (BTreeSet<String>, BTreeSet<String>) =
        found.into_iter().partition(|h| h.starts_with(LIMB_PROOFS));
    assert_eq!(
        rest,
        matrix(WORKFLOW),
        "the harnesses and kernel-fuzz.yml's kani-num matrix differ"
    );
    // The limb variant's proofs run in their own job, which names their module's prefix.
    assert!(limbs.len() >= 20, "{limbs:?}");
    let workflow = WORKFLOW.replace("\r\n", "\n");
    let after = workflow
        .split("\n  kani-limbs:\n")
        .nth(1)
        .expect("the kani-limbs job");
    // The job ends where the next job, a line indented two spaces, begins.
    let job: Vec<&str> = after
        .lines()
        .take_while(|l| !l.starts_with("  ") || l.starts_with("   "))
        .collect();
    let command = format!("cargo kani -p geom-num --harness {LIMB_PROOFS}");
    assert!(job.iter().any(|l| l.trim() == command), "{job:?}");
}

/// The module of the generated limb variant's proofs (piece G42).
const LIMB_PROOFS: &str = "generated::limbs::proofs::";

/// The check finds a harness however many attributes follow its mark, and only a marked function.
#[test]
fn the_scan_reads_marks_and_attributes() {
    let text =
        "#[kani::proof]\n#[kani::unwind(9)]\nfn a() {}\nfn b() {}\n#[kani::proof]\nfn c() {}\n";
    assert_eq!(harnesses("m", text), ["m::proofs::a", "m::proofs::c"]);
    let yaml = "jobs:\n  kani-num:\n    strategy:\n      matrix:\n        proof:\n          - m::proofs::a\n          - m::proofs::c\n    defaults:\n";
    assert_eq!(
        matrix(yaml),
        BTreeSet::from(["m::proofs::a".to_owned(), "m::proofs::c".to_owned()])
    );
}
