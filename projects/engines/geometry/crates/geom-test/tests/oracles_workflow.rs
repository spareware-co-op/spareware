//! `.github/workflows/kernel-oracles.yml` keeps the promises its header makes for the mesh
//! comparisons (piece G154, Gate 4's G156). The workflow is read as YAML (yaml-rust2), so a check
//! sees the structure GitHub sees, not a line that looks right:
//!
//! - **When.** It runs on a push to main and on a pull request, never on a timer. The mesh jobs
//!   run when the job `scope` names geom-mesh or geom-test, on either event; every other job but
//!   `scope` skips a pull request.
//! - **What.** Each mesh job's oracles are exactly the catalogue's live out-of-process mesh
//!   oracles, and `meshes` runs exactly the case sets `oracle jobs` writes
//!   (`geom_test::oracle::meshes::CASE_SETS`), so a set cannot be added to one and not the other.
//! - **Pinned and cached by source.** The oracle repository is checked out at `ORACLES_COMMIT`, a
//!   full commit, by every step that checks it out; an image's cache key is its oracle's folder's
//!   tree at that commit, and `meshes` fails when the cache does not hold it rather than build.
//! - **Judged, not swallowed.** `meshes` audits the oracle checkout, runs the container with no
//!   network, and runs `oracle compare` under `set -euo pipefail` with nothing that hides its exit;
//!   nothing in the workflow can publish an image.
//!
//! Each rule is also shown to fail on a planted violation.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "tests: a failure stops the test"
)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use geom_test::oracle::catalogue::{Catalogue, Run, State};
use geom_test::oracle::meshes::{CASE_SETS, holds};
use yaml_rust2::{Yaml, YamlLoader};

const WORKFLOW: &str = ".github/workflows/kernel-oracles.yml";
const MESH_JOBS: [&str; 2] = ["meshes-images", "meshes"];
const REACHES: &str = "contains(format(' {0} ', needs.scope.outputs.crates), ' geom-mesh ') || \
                       contains(format(' {0} ', needs.scope.outputs.crates), ' geom-test ')";
const ORACLES: &str = "spareware-co-op/oracles";
/// What would publish an image (as `oracle check --repository` holds the oracle repository).
const PUBLISHING: [&str; 5] = [
    "docker push",
    "docker login",
    "docker/login-action",
    "push: true",
    "ghcr.io",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap()
        .to_path_buf()
}

fn source() -> String {
    std::fs::read_to_string(root().join(WORKFLOW))
        .unwrap()
        .replace("\r\n", "\n")
}

fn strings(y: &Yaml) -> Vec<String> {
    y.as_vec()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect()
}

fn keys(y: &Yaml) -> Vec<String> {
    y.as_hash()
        .into_iter()
        .flat_map(|h| h.keys())
        .filter_map(|k| k.as_str().map(str::to_owned))
        .collect()
}

/// The catalogue's live out-of-process mesh oracles.
fn live_mesh_oracles() -> Vec<String> {
    Catalogue::embedded()
        .unwrap()
        .oracles
        .into_iter()
        .filter(|o| o.run == Run::OutOfProcess && o.state == State::Live && holds(&o.id))
        .map(|o| o.id)
        .collect()
}

/// What is wrong with the job `meshes`, `j`, whose steps are `steps` and scripts `runs`.
fn leg_problems(j: &Yaml, steps: &[Yaml], runs: &str) -> Vec<String> {
    let mut found = Vec::new();
    if strings(&j["strategy"]["matrix"]["cases"]) != CASE_SETS {
        found.push(format!(
            "meshes runs the case sets {:?}, not {CASE_SETS:?}",
            strings(&j["strategy"]["matrix"]["cases"])
        ));
    }
    let missing = steps.iter().any(|s| {
        s["uses"]
            .as_str()
            .is_some_and(|u| u.starts_with("actions/cache/restore@"))
            && s["with"]["fail-on-cache-miss"].as_bool() == Some(true)
    });
    if !missing {
        found.push("meshes does not fail when the cache lacks the image".to_owned());
    }
    if !runs.contains("--bin oracle -- check --repository") {
        found.push("meshes does not audit the oracle repository".to_owned());
    }
    if !runs.contains("docker run --rm -i --network none ") {
        found.push("meshes runs the oracle with a network".to_owned());
    }
    let judged = steps.iter().filter_map(|s| s["run"].as_str()).any(|r| {
        r.contains("--bin oracle -- compare")
            && r.trim_start().starts_with("set -euo pipefail")
            && !r.contains("|| true")
            && !r.contains("|| :")
    });
    if !judged {
        found.push("meshes does not fail on what oracle compare finds".to_owned());
    }
    found
}

/// Everything wrong with the workflow `text`.
fn problems(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let Ok(mut docs) = YamlLoader::load_from_str(text) else {
        return vec!["not YAML".to_owned()];
    };
    let w = docs.remove(0);
    for event in ["push", "pull_request"] {
        if w["on"][event].is_badvalue() {
            found.push(format!("it does not run on {event}"));
        }
    }
    if !w["on"]["schedule"].is_badvalue() {
        found.push("it runs on a timer".to_owned());
    }
    let commit = w["env"]["ORACLES_COMMIT"].as_str().unwrap_or_default();
    if !(commit.len() == 40 && commit.chars().all(|c| c.is_ascii_hexdigit())) {
        found.push(format!("ORACLES_COMMIT {commit:?} is not a full commit"));
    }
    let live: BTreeSet<String> = live_mesh_oracles().into_iter().collect();
    for job in keys(&w["jobs"]) {
        let j = &w["jobs"][job.as_str()];
        let condition = j["if"].as_str().unwrap_or_default();
        let steps = j["steps"].as_vec().cloned().unwrap_or_default();
        for step in &steps {
            if step["with"]["repository"].as_str() == Some(ORACLES)
                && step["with"]["ref"].as_str() != Some("${{ env.ORACLES_COMMIT }}")
            {
                found.push(format!(
                    "{job} checks out the oracle repository off the pin"
                ));
            }
        }
        if !MESH_JOBS.contains(&job.as_str()) {
            if job != "scope" && !condition.starts_with("github.event_name != 'pull_request' && ") {
                found.push(format!("{job} runs on a pull request"));
            }
            continue;
        }
        if condition != REACHES {
            found.push(format!(
                "{job} runs on {condition:?}, not what reaches the meshes"
            ));
        }
        let oracles: BTreeSet<String> = strings(&j["strategy"]["matrix"]["oracle"])
            .into_iter()
            .collect();
        if oracles != live {
            found.push(format!(
                "{job} runs {oracles:?}, not the live mesh oracles {live:?}"
            ));
        }
        let runs: String = steps
            .iter()
            .filter_map(|s| s["run"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if !runs.contains(r#"rev-parse "HEAD:$ORACLE""#) {
            found.push(format!("{job} does not key the image by its folder's tree"));
        }
        if job == "meshes" {
            found.extend(leg_problems(j, &steps, &runs));
        }
    }
    for word in PUBLISHING {
        if text
            .lines()
            .any(|l| l.contains(word) && !l.trim_start().starts_with('#'))
        {
            found.push(format!("`{word}` would publish an image"));
        }
    }
    found
}

/// `text` with `from` replaced by `to` once, which must apply.
fn planted(text: &str, from: &str, to: &str) -> String {
    let out = text.replacen(from, to, 1);
    assert_ne!(out, text, "the planted edit {from:?} applies");
    out
}

fn finds(text: &str, what: &str) {
    let found = problems(text);
    assert!(found.iter().any(|p| p.contains(what)), "{what}: {found:#?}");
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn the_mesh_jobs_run_what_reaches_them_on_every_case_set() {
    let text = source();
    let found = problems(&text);
    assert!(found.is_empty(), "{found:#?}");
    assert_eq!(live_mesh_oracles(), ["cgal", "manifold"]);

    // Planted: each rule broken once.
    finds(
        &planted(
            &text,
            "  workflow_dispatch:\n",
            "  schedule:\n    - cron: \"1 2 * * *\"\n  workflow_dispatch:\n",
        ),
        "on a timer",
    );
    finds(
        &planted(
            &text,
            "    if: contains(format(' {0} ', needs.scope.outputs.crates), ' geom-mesh ') || ",
            "    if: ",
        ),
        "not what reaches the meshes",
    );
    finds(
        &planted(&text, "          - manifold-models\n", ""),
        "case sets",
    );
    finds(
        &planted(
            &text,
            "        oracle: [manifold, cgal]\n        cases:",
            "        oracle: [manifold, cgal, cherchi]\n        cases:",
        ),
        "not the live mesh oracles",
    );
    finds(
        &planted(&text, "ORACLES_COMMIT: b", "ORACLES_COMMIT: main # b"),
        "not a full commit",
    );
    finds(
        &planted(
            &text,
            "          ref: ${{ env.ORACLES_COMMIT }}\n",
            "          ref: main\n",
        ),
        "off the pin",
    );
    finds(
        &planted(&text, "          fail-on-cache-miss: true\n", ""),
        "lacks the image",
    );
    finds(&planted(&text, "--network none ", ""), "with a network");
    finds(
        &planted(
            &text,
            "| tee \"$out/compare.txt\"",
            "| tee \"$out/compare.txt\" || true",
        ),
        "what oracle compare finds",
    );
    finds(
        &planted(
            &text,
            "          lookup-only: true\n",
            "          lookup-only: true\n          push: true\n",
        ),
        "would publish",
    );
    finds(
        &planted(
            &text,
            "    if: github.event_name != 'pull_request' && contains(format(' {0} ', needs.scope.outputs.crates), ' geom-op ')",
            "    if: contains(format(' {0} ', needs.scope.outputs.crates), ' geom-op ')",
        ),
        "cbor2 runs on a pull request",
    );
}
