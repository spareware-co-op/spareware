//! `.github/workflows/kernel-certificates.yml`'s HOL Light shards restore only a saved state their
//! own run made, and only after checking its digest. The workflow is read as YAML (yaml-rust2), so
//! a check sees the structure GitHub sees, not a line that looks right:
//!
//! - **Made by this run.** The job `hol-light-states` saves the states (fptaylor.sh's
//!   `FPTAYLOR_STATE_OUT`), gives each one's sha256 as its output `sha256`, and hands them on as
//!   the artifact `hol-light-states`, kept a day at most. No job of the workflow uses a cache.
//!   Its saving step fails when a container fails (`set -o pipefail`: the default shell has none)
//!   or when a problem it kept has no state.
//! - **Restored from this run only.** Every job that restores a state (`FPTAYLOR_STATE=`), and the
//!   job `fptaylor-shards` is one, needs `hol-light-states`, starts only once it has passed, so
//!   never without the state, and downloads the artifact by its name alone: a download that names
//!   another run, repository or token could fetch a state this run did not make.
//! - **Checked before it is restored.** The step that restores a state is given the sha256 from
//!   `hol-light-states`' output and passes it into the container, and fptaylor.sh refuses a state
//!   without a full sha256, and compares the file's digest with it before it unpacks or restores
//!   anything.
//!
//! Each rule is also shown to fail on a planted violation.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "tests: a failure stops the test"
)]

use std::path::{Path, PathBuf};

use yaml_rust2::{Yaml, YamlLoader};

const WORKFLOW: &str = ".github/workflows/kernel-certificates.yml";
const SCRIPT: &str = "projects/engines/geometry/crates/geom-gen/certificates/fptaylor.sh";
const STATES_JOB: &str = "hol-light-states";
const SHARDS_JOB: &str = "fptaylor-shards";
const ARTIFACT: &str = "hol-light-states";
/// A restoring job starts only once the states job has passed.
const STATES_PASSED: &str = "needs.hol-light-states.result == 'success'";
/// The states job's saving step fails when a container fails, or a problem it kept has no state.
const PIPEFAIL: &str = "set -o pipefail\n";
const NO_STATE: &str = r#"echo "$problem: no saved state"; exit 1"#;
/// Where a restoring step's digest comes from: the states job's output.
const SHA256_FROM: &str = "needs.hol-light-states.outputs.sha256";
/// fptaylor.sh's checks: a state needs a full sha256, and the file's digest must be it.
const SHA256_REQUIRED: &str = "[[ $state_sha256 =~ ^[0-9a-f]{64}$ ]]";
const DIGEST_COMPARED: &str = r#"[ "$digest" = "$state_sha256" ] ||"#;
/// What fptaylor.sh does with a state once it is checked.
const UNPACKED: &str = r#"tar -C "$work" -xf "$state/"#;
const RESTORED: &str = r#""$work/dmtcp/bin/dmtcp_restart""#;

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

fn text(y: &Yaml) -> String {
    match y {
        Yaml::String(s) => s.clone(),
        Yaml::Integer(i) => i.to_string(),
        Yaml::Boolean(b) => b.to_string(),
        _ => String::new(),
    }
}

fn keys(y: &Yaml) -> Vec<String> {
    y.as_hash()
        .into_iter()
        .flat_map(|h| h.keys())
        .map(text)
        .collect()
}

fn steps(job: &Yaml) -> Vec<Yaml> {
    job["steps"].as_vec().cloned().unwrap_or_default()
}

fn needs(job: &Yaml) -> Vec<String> {
    match &job["needs"] {
        Yaml::Array(a) => a.iter().map(text).collect(),
        y => vec![text(y)],
    }
}

fn uses(step: &Yaml, action: &str) -> bool {
    text(&step["uses"]).starts_with(action)
}

/// What is wrong with the workflow `workflow` and the script `script`.
fn problems(workflow: &str, script: &str) -> Vec<String> {
    let mut found = Vec::new();
    let docs = YamlLoader::load_from_str(workflow).unwrap();
    let jobs = &docs[0]["jobs"];
    let names = keys(jobs);

    for name in &names {
        if steps(&jobs[name.as_str()])
            .iter()
            .any(|s| uses(s, "actions/cache"))
        {
            found.push(format!(
                "{name} uses a cache, which another run could have written"
            ));
        }
    }

    found.extend(states_problems(&jobs[STATES_JOB]));

    let mut restoring = Vec::new();
    for name in &names {
        if let Some(problems) = restoring_problems(name, &jobs[name.as_str()]) {
            restoring.push(name.clone());
            found.extend(problems);
        }
    }
    if !restoring.iter().any(|n| n == SHARDS_JOB) {
        found.push(format!("{SHARDS_JOB} restores no state"));
    }

    if !script.contains(SHA256_REQUIRED) {
        found.push("fptaylor.sh restores a state without a full sha256".to_owned());
    }
    match (
        script.find(DIGEST_COMPARED),
        script.find(UNPACKED),
        script.find(RESTORED),
    ) {
        (Some(check), Some(unpack), Some(restore)) if check < unpack && check < restore => {}
        _ => found.push(
            "fptaylor.sh skips the digest check before it unpacks and restores the state"
                .to_owned(),
        ),
    }
    found
}

/// What is wrong with the states job, `states`.
fn states_problems(states: &Yaml) -> Vec<String> {
    let mut found = Vec::new();
    if states.is_badvalue() {
        found.push(format!("no job {STATES_JOB}"));
    } else {
        let all = steps(states);
        if !all
            .iter()
            .any(|s| text(&s["run"]).contains("FPTAYLOR_STATE_OUT="))
        {
            found.push(format!("{STATES_JOB} saves no state"));
        }
        let held = all.iter().any(|s| {
            let run = text(&s["run"]);
            run.contains("FPTAYLOR_STATE_OUT=")
                && run.starts_with(PIPEFAIL)
                && run.contains(NO_STATE)
        });
        if !held {
            found.push(format!(
                "{STATES_JOB} can pass without a state for every problem it kept"
            ));
        }
        let output = text(&states["outputs"]["sha256"]);
        let from_a_step = all.iter().any(|s| {
            let id = text(&s["id"]);
            !id.is_empty() && output == format!("${{{{ steps.{id}.outputs.sha256 }}}}")
        });
        if !from_a_step {
            found.push(format!("{STATES_JOB} gives no sha256 of its states"));
        }
        let uploads: Vec<&Yaml> = all
            .iter()
            .filter(|s| uses(s, "actions/upload-artifact") && text(&s["with"]["name"]) == ARTIFACT)
            .collect();
        if uploads.is_empty() {
            found.push(format!("{STATES_JOB} uploads no artifact {ARTIFACT}"));
        }
        for upload in uploads {
            if text(&upload["with"]["retention-days"]) != "1" {
                found.push(format!("{ARTIFACT} is kept longer than a day"));
            }
        }
    }
    found
}

/// What is wrong with the job `name`, `job`, if it restores a state (`None` if it does not).
fn restoring_problems(name: &str, job: &Yaml) -> Option<Vec<String>> {
    let mut found = Vec::new();
    let all = steps(job);
    let restores: Vec<&Yaml> = all
        .iter()
        .filter(|s| text(&s["run"]).contains("FPTAYLOR_STATE="))
        .collect();
    if restores.is_empty() {
        return None;
    }
    if !needs(job).iter().any(|n| n == STATES_JOB) {
        found.push(format!(
            "{name} restores a state without needing {STATES_JOB}"
        ));
    }
    if !text(&job["if"]).contains(STATES_PASSED) {
        found.push(format!(
            "{name} can start without the state: it does not wait for {STATES_JOB} to pass"
        ));
    }
    let downloads: Vec<&Yaml> = all
        .iter()
        .filter(|s| uses(s, "actions/download-artifact"))
        .collect();
    if !downloads
        .iter()
        .any(|s| text(&s["with"]["name"]) == ARTIFACT)
    {
        found.push(format!("{name} does not download {ARTIFACT}"));
    }
    for download in downloads {
        let extra: Vec<String> = keys(&download["with"])
            .into_iter()
            .filter(|k| k != "name" && k != "path")
            .collect();
        if !extra.is_empty() {
            found.push(format!(
                "{name} downloads with {extra:?}: from another run or repository"
            ));
        }
    }
    for step in restores {
        if !text(&step["env"]["FPTAYLOR_STATE_SHA256"]).contains(SHA256_FROM) {
            found.push(format!(
                "{name}'s FPTAYLOR_STATE_SHA256 is not {STATES_JOB}'s output"
            ));
        }
        if !text(&step["run"]).contains("-e FPTAYLOR_STATE_SHA256 ") {
            found.push(format!(
                "{name} skips the digest check: no FPTAYLOR_STATE_SHA256 in the container"
            ));
        }
    }
    Some(found)
}

/// `text` with `from` replaced by `to` once, which must apply.
fn planted(text: &str, from: &str, to: &str) -> String {
    let out = text.replacen(from, to, 1);
    assert_ne!(out, text, "the planted edit {from:?} applies");
    out
}

fn finds(workflow: &str, script: &str, what: &str) {
    let found = problems(workflow, script);
    assert!(found.iter().any(|p| p.contains(what)), "{what}: {found:#?}");
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn the_shards_restore_only_a_state_their_run_made_and_checked() {
    let workflow = read(WORKFLOW);
    let script = read(SCRIPT);
    let found = problems(&workflow, &script);
    assert!(found.is_empty(), "{found:#?}");

    // Planted: a state from another run or a cache.
    let download = "        with:\n          name: hol-light-states\n          path: ${{ runner.temp }}/states\n      #";
    finds(
        &planted(
            &workflow,
            download,
            "        with:\n          name: hol-light-states\n          path: ${{ runner.temp }}/states\n          run-id: 1\n          github-token: ${{ github.token }}\n      #",
        ),
        &script,
        "from another run or repository",
    );
    finds(
        &planted(
            &workflow,
            "      - uses: actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333 # v8.0.2\n        if: steps.base.outputs.same != 'true'\n        with:\n          name: hol-light-states\n          path: ${{ runner.temp }}/states\n",
            "      - uses: actions/cache@0057852bfaa89a56745cba8c7296529d2fc39830 # v4.3.0\n        with:\n          key: hol-light-states\n          path: ${{ runner.temp }}/states\n",
        ),
        &script,
        "uses a cache",
    );
    finds(
        &planted(
            &workflow,
            "    needs: [fptaylor-problems, fptaylor-certificates, hol-light-states]",
            "    needs: [fptaylor-problems, fptaylor-certificates]",
        ),
        &script,
        "without needing hol-light-states",
    );
    finds(
        &planted(
            &workflow,
            "          retention-days: 1\n",
            "          retention-days: 30\n",
        ),
        &script,
        "kept longer than a day",
    );
    finds(
        &planted(
            &workflow,
            "      sha256: ${{ steps.save.outputs.sha256 }}",
            "      sha256: ${{ steps.other.outputs.sha256 }}",
        ),
        &script,
        "gives no sha256",
    );

    // Planted: the digest check skipped, in the workflow or in the script.
    finds(
        &planted(&workflow, "-e FPTAYLOR_STATE_SHA256 ", ""),
        &script,
        "skips the digest check",
    );
    finds(
        &planted(
            &workflow,
            "FPTAYLOR_STATE_SHA256: ${{ fromJSON(needs.hol-light-states.outputs.sha256 || '{}')[matrix.problem] }}",
            "FPTAYLOR_STATE_SHA256: ${{ matrix.sha256 }}",
        ),
        &script,
        "is not hol-light-states's output",
    );
    finds(
        &workflow,
        &planted(&script, DIGEST_COMPARED, r#"[ -n "$digest" ] ||"#),
        "skips the digest check before",
    );
    finds(
        &workflow,
        &planted(
            &script,
            SHA256_REQUIRED,
            r"[[ $state_sha256 =~ ^[0-9a-f]*$ ]]",
        ),
        "without a full sha256",
    );
    // The comparison moved after the unpacking.
    let after = planted(&script, DIGEST_COMPARED, "true ||");
    let after = planted(
        &after,
        "  for want in ",
        &format!("  {DIGEST_COMPARED} exit 1\n  for want in "),
    );
    finds(&workflow, &after, "skips the digest check before");
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn a_shard_never_starts_without_its_state() {
    let workflow = read(WORKFLOW);
    let script = read(SCRIPT);
    let found = problems(&workflow, &script);
    assert!(found.is_empty(), "{found:#?}");

    // Planted: a shard that can start without the state (run 44012975257: the states job passed
    // with no state, and every shard then failed to find the artifact).
    finds(
        &planted(
            &workflow,
            " && needs.hol-light-states.result == 'success' }}",
            " }}",
        ),
        &script,
        "can start without the state",
    );
    finds(
        &planted(&workflow, "          set -o pipefail\n", ""),
        &script,
        "can pass without a state",
    );
    finds(
        &planted(&workflow, NO_STATE, r#"echo "$problem: no saved state""#),
        &script,
        "can pass without a state",
    );
}
