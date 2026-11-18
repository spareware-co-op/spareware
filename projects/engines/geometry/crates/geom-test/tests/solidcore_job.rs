//! The job that runs solidcore's stored cases (decision 32.70): `kernel.yml`'s job `solidcore`
//! builds geom-solidcore's own workspace alone, checks it with cargo-deny, and passes only when the
//! tests that fail are exactly the named faults of `tests/named-faults.json`. The workflow is read
//! as YAML (yaml-rust2), so a check sees the structure GitHub sees:
//!
//! - `the_solidcore_job_runs_the_cases_against_the_named_faults`: the job runs on Linux, within
//!   the owner's 15 minutes, in geom-solidcore's folder; its build cache is keyed on the commit
//!   geom-solidcore's Cargo.toml pins and saved only by a push to main; it runs cargo-deny with the
//!   kernel's deny.toml, then the tests, and compares the failed tests with the named faults; the
//!   repository's root workspace excludes geom-solidcore, so no other job builds solidcore.
//!
//! - `the_webassembly_job_runs_the_same_cases_against_the_same_named_faults`: the job
//!   `solidcore-wasm` runs whenever `solidcore` runs, within the 15 minutes, on the wasm32-wasip1
//!   target with wasmtime installed as the kernel's other wasm32 jobs install it; it builds the
//!   same tests for wasm32-wasip1, runs each in its own wasmtime process, fails on a test that
//!   traps rather than counting it, compares the failed tests with the same named faults and asks
//!   for as many tests as the native job; geom-solidcore's `.cargo/config.toml` links the module
//!   with the 8 MiB stack solidcore asks for.
//!
//! Each check is also shown to fail on a seeded violation.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use yaml_rust2::YamlLoader;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap()
        .to_path_buf()
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel))
        .unwrap()
        .replace("\r\n", "\n")
}

/// The pinned commit, from geom-solidcore's Cargo.toml.
fn pin(manifest: &str) -> Option<String> {
    let line = manifest.lines().find(|l| l.starts_with("solidcore = "))?;
    let rev = line.split("rev = \"").nth(1)?;
    Some(rev.split('"').next()?.to_owned())
}

/// Everything wrong with the job, given the workflow's text, geom-solidcore's manifest and the
/// root manifest.
fn problems(kernel: &str, manifest: &str, root_manifest: &str) -> Vec<String> {
    let mut found = Vec::new();
    let docs = YamlLoader::load_from_str(kernel).unwrap();
    let job = &docs[0]["jobs"]["solidcore"];
    if job.is_badvalue() {
        return vec!["kernel.yml has no job solidcore".to_owned()];
    }
    if job["runs-on"].as_str() != Some("ubuntu-latest") {
        found.push("solidcore does not run on ubuntu-latest".to_owned());
    }
    if job["timeout-minutes"].as_i64().is_none_or(|m| m > 15) {
        found.push("solidcore may run over 15 minutes".to_owned());
    }
    if job["defaults"]["run"]["working-directory"].as_str()
        != Some("projects/engines/geometry/geom-solidcore")
    {
        found.push("solidcore does not run in geom-solidcore's folder".to_owned());
    }
    let Some(rev) = pin(manifest) else {
        return vec!["geom-solidcore's Cargo.toml pins no rev of solidcore".to_owned()];
    };
    let steps = job["steps"].as_vec().cloned().unwrap_or_default();
    let cache = steps.iter().find(|s| {
        s["uses"]
            .as_str()
            .is_some_and(|u| u.starts_with("Swatinem/rust-cache@"))
    });
    match cache {
        None => found.push("solidcore has no build cache".to_owned()),
        Some(c) => {
            if c["with"]["key"].as_str() != Some(&format!("solidcore-{rev}")) {
                found.push("solidcore's cache is not keyed on the pinned commit".to_owned());
            }
            if c["with"]["save-if"].as_str()
                != Some("${{ github.event_name == 'push' && github.ref == 'refs/heads/main' }}")
            {
                found.push("solidcore's cache is saved by more than a push to main".to_owned());
            }
        }
    }
    let runs: Vec<&str> = steps.iter().filter_map(|s| s["run"].as_str()).collect();
    let at = |needle: &str| runs.iter().position(|r| r.contains(needle));
    let deny = at("cargo deny --locked --config ../deny.toml check");
    let compare = at("jq -r '.faults[].test' tests/named-faults.json");
    if deny.is_none() {
        found.push("solidcore does not run cargo-deny on its workspace".to_owned());
    }
    match compare {
        None => found
            .push("solidcore does not compare the failed tests with the named faults".to_owned()),
        Some(i) => {
            let r = runs[i];
            if !r.contains("cargo test --locked --no-fail-fast")
                || !r.contains("if [ \"$failed\" != \"$named\" ]; then")
            {
                found.push("solidcore's comparison does not fail on a difference".to_owned());
            }
        }
    }
    if !root_manifest.contains("exclude = [\"projects/engines/geometry/geom-solidcore\"]") {
        found.push("the root workspace does not exclude geom-solidcore".to_owned());
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn the_solidcore_job_runs_the_cases_against_the_named_faults() {
    let kernel = read(".github/workflows/kernel.yml");
    let manifest = read("projects/engines/geometry/geom-solidcore/Cargo.toml");
    let root_manifest = read("Cargo.toml");
    assert_eq!(
        problems(&kernel, &manifest, &root_manifest),
        Vec::<String>::new()
    );
    let rev = pin(&manifest).unwrap();
    for (from, to, said) in [
        (
            "\n  solidcore:\n",
            "\n  solidcore-gone:\n",
            "no job solidcore",
        ),
        (
            "    timeout-minutes: 15\n    env:\n      # cargo-deny",
            "    timeout-minutes: 16\n    env:\n      # cargo-deny",
            "over 15 minutes",
        ),
        (
            &format!("key: solidcore-{rev}"),
            "key: solidcore",
            "keyed on the pinned commit",
        ),
        (
            "cargo deny --locked --config ../deny.toml check",
            "cargo deny --version",
            "cargo-deny",
        ),
        (
            "if [ \"$failed\" != \"$named\" ]; then",
            "if false; then",
            "does not fail on a difference",
        ),
    ] {
        let k = kernel.replacen(from, to, 1);
        assert_ne!(k, kernel, "the planted edit {from:?} applies");
        let found = problems(&k, &manifest, &root_manifest);
        assert!(found.iter().any(|p| p.contains(said)), "{said}: {found:#?}");
    }
    let unpinned = manifest.replacen(&format!("rev = \"{rev}\""), "branch = \"spareware\"", 1);
    assert!(
        problems(&kernel, &unpinned, &root_manifest)
            .iter()
            .any(|p| p.contains("pins no rev"))
    );
    let included = root_manifest.replacen(
        "exclude = [\"projects/engines/geometry/geom-solidcore\"]",
        "",
        1,
    );
    assert!(
        problems(&kernel, &manifest, &included)
            .iter()
            .any(|p| p.contains("does not exclude"))
    );
}

/// The least number of tests a step asks to have run (its `test "$ran" -ge` line).
fn ran(text: &str) -> Option<u32> {
    text.lines()
        .find_map(|l| l.trim().strip_prefix("test \"$ran\" -ge "))
        .and_then(|n| n.parse::<u32>().ok())
}

/// Everything wrong with the WebAssembly job's step that runs the tests and compares, given its
/// text (as YAML gives it, its block's indentation removed) and the native job's least number of
/// tests.
fn compare_problems(step: Option<&str>, native_ran: Option<u32>) -> Vec<String> {
    let Some(r) = step else {
        return vec![
            "solidcore-wasm does not compare the failed tests with the named faults".to_owned(),
        ];
    };
    let mut found = Vec::new();
    if !r.contains("wasmtime run --allow-precompiled \"$module.cwasm\" --exact \"$t\" --nocapture")
        || !r.contains("--list --format terse")
    {
        found.push("solidcore-wasm does not run every test in its own process".to_owned());
    }
    if !r.contains("if [ \"$failed\" != \"$named\" ]; then") {
        found.push("solidcore-wasm's comparison does not fail on a difference".to_owned());
    }
    if !r.contains(
        "neither passed nor failed by a panic under wasmtime (exit status $rc).\"\n      exit 1\n",
    ) {
        found.push("solidcore-wasm counts a trap as a named fault".to_owned());
    }
    if ran(r).is_none() || ran(r) != native_ran {
        found.push("solidcore-wasm does not ask for as many tests as solidcore".to_owned());
    }
    found
}

/// Everything wrong with the WebAssembly job, given the workflow's text, geom-solidcore's
/// manifest and its `.cargo/config.toml`.
fn wasm_problems(kernel: &str, manifest: &str, cargo_config: &str) -> Vec<String> {
    let mut found = Vec::new();
    let docs = YamlLoader::load_from_str(kernel).unwrap();
    let job = &docs[0]["jobs"]["solidcore-wasm"];
    if job.is_badvalue() {
        return vec!["kernel.yml has no job solidcore-wasm".to_owned()];
    }
    let native = &docs[0]["jobs"]["solidcore"];
    if job["if"].as_str().is_none() || job["if"].as_str() != native["if"].as_str() {
        found.push("solidcore-wasm does not run when solidcore runs".to_owned());
    }
    if job["runs-on"].as_str() != Some("ubuntu-latest") {
        found.push("solidcore-wasm does not run on ubuntu-latest".to_owned());
    }
    if job["timeout-minutes"].as_i64().is_none_or(|m| m > 15) {
        found.push("solidcore-wasm may run over 15 minutes".to_owned());
    }
    if job["defaults"]["run"]["working-directory"].as_str()
        != Some("projects/engines/geometry/geom-solidcore")
    {
        found.push("solidcore-wasm does not run in geom-solidcore's folder".to_owned());
    }
    let Some(rev) = pin(manifest) else {
        return vec!["geom-solidcore's Cargo.toml pins no rev of solidcore".to_owned()];
    };
    let steps = job["steps"].as_vec().cloned().unwrap_or_default();
    let uses = |prefix: &str| {
        steps
            .iter()
            .find(|s| s["uses"].as_str().is_some_and(|u| u.starts_with(prefix)))
            .cloned()
    };
    if uses("dtolnay/rust-toolchain@")
        .is_none_or(|s| s["with"]["targets"].as_str() != Some("wasm32-wasip1"))
    {
        found.push("solidcore-wasm does not install the wasm32-wasip1 target".to_owned());
    }
    match uses("Swatinem/rust-cache@") {
        None => found.push("solidcore-wasm has no build cache".to_owned()),
        Some(c) => {
            if c["with"]["key"].as_str() != Some(&format!("solidcore-wasm-{rev}")) {
                found.push("solidcore-wasm's cache is not keyed on the pinned commit".to_owned());
            }
            if c["with"]["save-if"].as_str()
                != Some("${{ github.event_name == 'push' && github.ref == 'refs/heads/main' }}")
            {
                found
                    .push("solidcore-wasm's cache is saved by more than a push to main".to_owned());
            }
        }
    }
    // wasmtime as the kernel's other wasm32 jobs install it: the pinned action, at the release
    // the job `wasm` names.
    let pinned = docs[0]["jobs"]["wasm"]["env"]["WASMTIME_VERSION"].as_str();
    if pinned.is_none() || job["env"]["WASMTIME_VERSION"].as_str() != pinned {
        found.push("solidcore-wasm's wasmtime is not the release the wasm job pins".to_owned());
    }
    if uses("bytecodealliance/actions/wasmtime/setup@")
        .is_none_or(|s| s["with"]["version"].as_str() != Some("${{ env.WASMTIME_VERSION }}"))
    {
        found.push("solidcore-wasm does not install wasmtime by the pinned action".to_owned());
    }
    let runs: Vec<&str> = steps.iter().filter_map(|s| s["run"].as_str()).collect();
    if !runs
        .iter()
        .any(|r| r.contains("cargo test --locked --target wasm32-wasip1 --no-run"))
    {
        found.push("solidcore-wasm does not build the tests for wasm32-wasip1".to_owned());
    }
    let native_ran = native["steps"]
        .as_vec()
        .and_then(|v| v.iter().filter_map(|s| s["run"].as_str()).find_map(ran));
    found.extend(compare_problems(
        runs.iter()
            .find(|r| r.contains("jq -r '.faults[].test' tests/named-faults.json"))
            .copied(),
        native_ran,
    ));
    if !cargo_config
        .contains("[target.wasm32-wasip1]\nrustflags = [\"-C\", \"link-arg=-zstack-size=8388608\"]")
    {
        found.push("the wasm32 module is not linked with solidcore's 8 MiB stack".to_owned());
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn the_webassembly_job_runs_the_same_cases_against_the_same_named_faults() {
    let kernel = read(".github/workflows/kernel.yml");
    let manifest = read("projects/engines/geometry/geom-solidcore/Cargo.toml");
    let config = read("projects/engines/geometry/geom-solidcore/.cargo/config.toml");
    assert_eq!(
        wasm_problems(&kernel, &manifest, &config),
        Vec::<String>::new()
    );
    let rev = pin(&manifest).unwrap();
    for (from, to, said) in [
        (
            "\n  solidcore-wasm:\n",
            "\n  solidcore-wasm-gone:\n",
            "no job solidcore-wasm",
        ),
        (
            "under WebAssembly\n    needs: changes\n    if: github.event_name != 'merge_group' && ",
            "under WebAssembly\n    needs: changes\n    if: github.event_name == 'push' && ",
            "does not run when solidcore runs",
        ),
        (
            "    timeout-minutes: 15\n    env:\n      # wasmtime",
            "    timeout-minutes: 30\n    env:\n      # wasmtime",
            "over 15 minutes",
        ),
        (
            "          targets: wasm32-wasip1\n",
            "",
            "wasm32-wasip1 target",
        ),
        (
            &format!("key: solidcore-wasm-{rev}"),
            "key: solidcore-wasm",
            "keyed on the pinned commit",
        ),
        (
            "jobs pin.\n      WASMTIME_VERSION: \"50.0.0\"",
            "jobs pin.\n      WASMTIME_VERSION: \"40.0.0\"",
            "the release the wasm job pins",
        ),
        (
            "cargo test --locked --target wasm32-wasip1 --no-run",
            "cargo test --locked --no-run",
            "build the tests for wasm32-wasip1",
        ),
        (
            "--allow-precompiled \"$module.cwasm\" --exact \"$t\"",
            "--allow-precompiled \"$module.cwasm\"",
            "its own process",
        ),
        (
            "if [ \"$failed\" != \"$named\" ]; then\n            echo \"::error::The tests that fail under",
            "if false; then\n            echo \"::error::The tests that fail under",
            "does not fail on a difference",
        ),
        (
            "(exit status $rc).\"\n                exit 1\n",
            "(exit status $rc).\"\n                echo \"test $t ... FAILED\" >> \"$log\"\n",
            "counts a trap as a named fault",
        ),
        (
            "named faults of tests/named-faults.json.\"\n            exit 1\n          fi\n          test \"$ran\" -ge 102\n\n  # The goldens",
            "named faults of tests/named-faults.json.\"\n            exit 1\n          fi\n          test \"$ran\" -ge 3\n\n  # The goldens",
            "as many tests as solidcore",
        ),
    ] {
        let k = kernel.replacen(from, to, 1);
        assert_ne!(k, kernel, "the planted edit {from:?} applies");
        let found = wasm_problems(&k, &manifest, &config);
        assert!(found.iter().any(|p| p.contains(said)), "{said}: {found:#?}");
    }
    let unstacked = config.replacen("link-arg=-zstack-size=8388608", "link-arg=--no-entry", 1);
    assert_ne!(unstacked, config);
    assert!(
        wasm_problems(&kernel, &manifest, &unstacked)
            .iter()
            .any(|p| p.contains("8 MiB stack"))
    );
}

/// A job removed from the workflow is reported, not passed.
#[test]
fn a_workflow_with_no_such_job_is_reported() {
    let found = problems("jobs:\n  lint:\n    runs-on: ubuntu-latest\n", "", "");
    assert_eq!(found, vec!["kernel.yml has no job solidcore".to_owned()]);
    let found = wasm_problems("jobs:\n  lint:\n    runs-on: ubuntu-latest\n", "", "");
    assert_eq!(
        found,
        vec!["kernel.yml has no job solidcore-wasm".to_owned()]
    );
}
