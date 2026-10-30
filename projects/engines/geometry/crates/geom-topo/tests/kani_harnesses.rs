//! Every Kani harness beside `geom-topo`'s code is proven in CI (piece G112; chapter 28.7).
//!
//! Each module's `#[cfg(kani)] mod proofs` holds bounded proofs that Kani checks on Linux, in
//! `kernel-fuzz.yml`'s `kani-topo` job, one harness per matrix entry. A harness missing from the
//! matrix would never run; an entry naming no harness would fail in CI only. This test holds the
//! two to each other, as `geom-num`'s `tests/kani_harnesses.rs` does for `kani-num`, and holds the
//! workflow's path filters to this crate, so a change here starts the job.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;
use std::path::Path;

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

/// The `kani-topo` job's `proof` list.
fn matrix(yaml: &str) -> BTreeSet<String> {
    let yaml = yaml.replace("\r\n", "\n");
    let job = yaml
        .split("\n  kani-topo:\n")
        .nth(1)
        .expect("the kani-topo job");
    let list = job
        .split("        proof:\n")
        .nth(1)
        .expect("its proof list");
    list.lines()
        .map_while(|l| l.strip_prefix("          - "))
        .map(str::to_owned)
        .collect()
}

#[test]
#[cfg_attr(
    target_os = "wasi",
    ignore = "reads the crate's folder; wasm32-wasip1 under wasmtime is given no folder to read"
)]
#[cfg_attr(
    miri,
    ignore = "reads the crate's folder; Miri isolates the test from the file system"
)]
fn every_harness_is_in_the_kani_job_and_every_entry_is_a_harness() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = BTreeSet::new();
    for entry in std::fs::read_dir(&src).unwrap() {
        let path = entry.unwrap().path();
        assert!(
            !path.is_dir(),
            "geom-topo's source has no folders yet: {path:?}"
        );
        let module = path.file_stem().unwrap().to_str().unwrap().to_owned();
        found.extend(harnesses(&module, &std::fs::read_to_string(&path).unwrap()));
    }
    assert!(found.len() >= 5, "{found:?}");
    assert_eq!(
        found,
        matrix(WORKFLOW),
        "the harnesses and kernel-fuzz.yml's kani-topo matrix differ"
    );
}

#[test]
fn a_change_to_this_crate_starts_the_job() {
    let filter = "      - \"projects/engines/geometry/crates/geom-topo/**\"\n";
    assert_eq!(
        WORKFLOW.replace("\r\n", "\n").matches(filter).count(),
        2,
        "geom-topo is in both the pull-request and the push path filters"
    );
}

/// The check finds a harness however many attributes follow its mark, and only a marked function.
#[test]
fn the_scan_reads_marks_and_attributes() {
    let text =
        "#[kani::proof]\n#[kani::unwind(9)]\nfn a() {}\nfn b() {}\n#[kani::proof]\nfn c() {}\n";
    assert_eq!(harnesses("m", text), ["m::proofs::a", "m::proofs::c"]);
    let yaml = "jobs:\n  kani-topo:\n    strategy:\n      matrix:\n        proof:\n          - m::proofs::a\n          - m::proofs::c\n    defaults:\n";
    assert_eq!(
        matrix(yaml),
        BTreeSet::from(["m::proofs::a".to_owned(), "m::proofs::c".to_owned()])
    );
}
