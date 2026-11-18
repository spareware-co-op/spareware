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
//! Each check is also shown to fail on a seeded violation.

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

/// A job removed from the workflow is reported, not passed.
#[test]
fn a_workflow_with_no_such_job_is_reported() {
    let found = problems("jobs:\n  lint:\n    runs-on: ubuntu-latest\n", "", "");
    assert_eq!(found, vec!["kernel.yml has no job solidcore".to_owned()]);
}
