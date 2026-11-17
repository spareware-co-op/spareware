//! `.github/workflows/kernel-certificates.yml`'s HOL Light shards restore only a saved state their
//! own run made, and only after checking its digest; and every shard of a problem, of whole terms
//! or of a cut term's pieces, is one job of the same run. The workflow is read as YAML
//! (yaml-rust2), so a check sees the structure GitHub sees, not a line that looks right:
//!
//! - **Made by this run.** The job `hol-light-states` saves the loaded states (fptaylor.sh's
//!   `FPTAYLOR_STATE_OUT`) and the job `hol-light-cut-states` the second states, which hold a cut
//!   term's Taylor theorems (`FPTAYLOR_CUT_STATE_OUT`). Each gives its states' sha256 as its
//!   output `sha256`, and hands them on as the artifact of its own name, kept a day at most. No
//!   job of the workflow uses a cache. A saving step fails when a container fails (`set -o
//!   pipefail`: the default shell has none) or when a problem it kept has no state.
//! - **Restored from this run only.** Every job that restores a state (`FPTAYLOR_STATE=`) is one
//!   of three: `fptaylor-shards` and `hol-light-cut-states` restore the loaded state, and
//!   `fptaylor-pieces` the second. Each needs the job that saved its state, starts only once that
//!   job has passed, so never without the state, and downloads that job's artifact by its name
//!   alone: a download that names another run, repository or token could fetch a state this run
//!   did not make.
//! - **Checked before it is restored.** The step that restores a state is given the sha256 from
//!   its saving job's output and passes it into the container, and fptaylor.sh refuses a state
//!   without a full sha256, and compares the file's digest with it before it unpacks or restores
//!   anything. The run that saves a second state fails unless it built each cut term's theorems
//!   and proved nothing.
//! - **Every shard is one job of the run.** `fptaylor-problems` sends a shard whose line names a
//!   piece to `fptaylor-pieces` and any other to `fptaylor-shards`, each job runs every shard of
//!   its list, on every event the workflow runs on, and neither can fail without failing the
//!   run or skip its check for a problem the base did not prove. In the committed problems every
//!   piece of a cut term is in exactly one shard, and no shard has pieces and whole terms. So
//!   what one shard's theorem assumes, another job of the same run proves.
//! - **A restored HOL Light is known by its output alone.** The run that saves a second state
//!   restarts HOL Light in the background and waits on it. fptaylor.sh never takes
//!   `dmtcp_restart`'s process for HOL Light's: DMTCP restores a process whose parent was process
//!   1 when it was saved, as HOL Light's is where the script is its container's first process, in
//!   a grandchild of `dmtcp_restart`, which ends at once with success (run 44076902825: the saving
//!   run watched that process and gave up within seconds of a good restore). What the script
//!   holds instead reads HOL Light's output from a pipe, so it ends when HOL Light does; and a
//!   saving run that gives up shows the end of that output and of DMTCP's coordinator's log.
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
const PROBLEMS: &str = "projects/engines/geometry/crates/geom-gen/generated";
const LIST_JOB: &str = "fptaylor-problems";
const SHARDS_JOB: &str = "fptaylor-shards";
const PIECES_JOB: &str = "fptaylor-pieces";

/// A saved state: the job that saves it, whose name its artifact has; how its saving step is
/// known; and the line that fails that step when a problem it kept has no state.
struct State {
    job: &'static str,
    saves: &'static str,
    missing: &'static str,
}

/// HOL Light loaded with its verifier.
const LOADED: State = State {
    job: "hol-light-states",
    saves: "-e FPTAYLOR_STATE_OUT=",
    missing: r#"echo "$problem: no saved state"; exit 1"#,
};
/// The loaded state with each cut term's Taylor theorems built.
const CUT: State = State {
    job: "hol-light-cut-states",
    saves: "-e FPTAYLOR_CUT_STATE_OUT=",
    missing: r#"echo "$problem: no saved cut state"; exit 1"#,
};
const STATES: [&State; 2] = [&LOADED, &CUT];
/// The jobs that restore a state, each with the one it restores.
const RESTORING: [(&str, &State); 3] = [
    ("hol-light-cut-states", &LOADED),
    (SHARDS_JOB, &LOADED),
    (PIECES_JOB, &CUT),
];

/// A saving step fails when a container fails.
const PIPEFAIL: &str = "set -o pipefail\n";
/// fptaylor.sh's checks: a state needs a full sha256, and the file's digest must be it.
const SHA256_REQUIRED: &str = "[[ $state_sha256 =~ ^[0-9a-f]{64}$ ]]";
const DIGEST_COMPARED: &str = r#"[ "$digest" = "$state_sha256" ] ||"#;
/// What fptaylor.sh does with a state once it is checked.
const UNPACKED: &str = r#"tar -C "$work" -xf "$state/"#;
const RESTORED: &str = r#""$work/dmtcp/bin/dmtcp_restart""#;
/// fptaylor.sh's checks of the run that saves a second state: each cut term's theorems built,
/// and nothing proved.
const CUT_BUILT: &str = r#"[ "$built" = "$cut_terms" ] ||"#;
const CUT_PROVED_NOTHING: &str = r#"! grep -q '^\(# \)*Proved: ' "$work/hol.log" ||"#;
/// fptaylor.sh's restart that saves again: HOL Light's output goes into a pipe, and the process
/// the script holds, `hol_ends`, is the pipe's last reader.
const HELD_BY_ITS_OUTPUT: &str = "\"$work/hol.dmtcp\") <\"$work/input\" 2>&1 |\n    tee \"$work/hol.log\" | grep --line-buffered -v '^val \\|^- : ' &\n  hol_ends=$!\n";
/// How a saving run of fptaylor.sh gives up: through `stopped`, which shows the end of HOL
/// Light's output and of the DMTCP coordinator's log, which the restart is told to write.
const STOP_SHOWS: &str = r#"tail -n 40 "$log" | cut -c 1-1000;"#;
const STOP_LOGS: &str = r#"for log in "$work/hol.log" "$work/coordinator.log"; do"#;
const COORDINATOR_LOG: &str = r#"--coord-logfile "$work/coordinator.log""#;
const STOPS: [&str; 4] = [
    r#"kill -0 "$hol_ends" 2>/dev/null || stopped "HOL Light stopped before it was loaded""#,
    r#"kill -0 "$hol_ends" 2>/dev/null || stopped "HOL Light stopped before the cut terms were built""#,
    r#"stopped "DMTCP wrote no restart script: the state was not saved""#,
    r#"stopped "DMTCP saved no single image""#,
];

/// How `fptaylor-problems` sends each shard to one of its two lists, and fails otherwise.
const ROUTING: [&str; 5] = [
    r#"if grep -q "^// hol-light-shard $shard: .*\." "$problem"; then pieces=true; else pieces=false; fi"#,
    r#"shards=$(jq -c '{include: map(select(.pieces == false) | del(.pieces))}' <<<"$legs")"#,
    r#"pieces=$(jq -c '{include: map(select(.pieces == true) | del(.pieces))}' <<<"$legs")"#,
    r#"{ echo "a shard is in neither list"; exit 1; }"#,
    r#"{ echo "the problems with shards of pieces are not those that cut a term"; exit 1; }"#,
];
/// The one rule that sets a shard aside: its problem, the scripts and the workflow are the base's.
const SAME_AS_BASE: &str = r#"if git diff --quiet "$BASE" HEAD -- "$PROBLEMS/$PROBLEM" "$SCRIPTS" .github/workflows/kernel-certificates.yml; then"#;
const NOT_SET_ASIDE: &str = "steps.base.outputs.same != 'true'";

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

    for state in STATES {
        found.extend(states_problems(&jobs[state.job], state));
    }

    let mut restoring = Vec::new();
    for name in &names {
        let job = &jobs[name.as_str()];
        if !restores(job) {
            continue;
        }
        restoring.push(name.clone());
        match RESTORING.iter().find(|(n, _)| n == name) {
            Some((_, state)) => found.extend(restoring_problems(name, job, state)),
            None => found.push(format!(
                "{name} restores a state, and no rule here says which"
            )),
        }
    }
    for (name, _) in RESTORING {
        if !restoring.iter().any(|n| n == name) {
            found.push(format!("{name} restores no state"));
        }
    }

    found.extend(shard_problems(jobs));

    if !script.contains(SHA256_REQUIRED) {
        found.push("fptaylor.sh restores a state without a full sha256".to_owned());
    }
    let restarts: Vec<usize> = script.match_indices(RESTORED).map(|(at, _)| at).collect();
    match (script.find(DIGEST_COMPARED), script.find(UNPACKED)) {
        (Some(check), Some(unpack))
            if check < unpack && !restarts.is_empty() && restarts.iter().all(|r| check < *r) => {}
        _ => found.push(
            "fptaylor.sh skips the digest check before it unpacks and restores the state"
                .to_owned(),
        ),
    }
    if !script.contains(CUT_BUILT) || !script.contains(CUT_PROVED_NOTHING) {
        found.push(
            "fptaylor.sh can save a cut state without each cut term's theorems, or one that proved a term"
                .to_owned(),
        );
    }
    found.extend(held_problems(script));
    found
}

/// What is wrong with how the script `script` knows that a HOL Light it restored in the
/// background runs, and with what it shows when a saving run gives up.
fn held_problems(script: &str) -> Vec<String> {
    let mut found = Vec::new();
    let lines: Vec<&str> = script.lines().collect();
    for (at, line) in lines.iter().enumerate() {
        if !line.trim_end().ends_with("=$!") {
            continue;
        }
        // The command put in the background before it: back over comments to its last line,
        // then over each line a backslash or a pipe continues.
        let mut before = lines[..at]
            .iter()
            .rev()
            .skip_while(|l| l.trim_start().starts_with('#'));
        let mut command: Vec<&str> = before.next().into_iter().copied().collect();
        command.extend(
            before
                .take_while(|l| [" \\", " |"].iter().any(|more| l.ends_with(more)))
                .copied(),
        );
        command.reverse();
        // `$!` is the last process of that command's pipeline.
        let command = command.join("\n");
        let last = command
            .rsplit_once(" |")
            .map_or(command.as_str(), |(_, last)| last);
        if last.contains("dmtcp_restart") {
            found.push("fptaylor.sh takes dmtcp_restart's process for HOL Light's".to_owned());
        }
    }
    if !script.contains(HELD_BY_ITS_OUTPUT) {
        found.push(
            "fptaylor.sh does not hold the HOL Light it restores to save again by its output"
                .to_owned(),
        );
    }
    if !script.contains(STOP_SHOWS)
        || !script.contains(STOP_LOGS)
        || !script.contains(COORDINATOR_LOG)
        || !STOPS.iter().all(|stop| script.contains(stop))
    {
        found.push(
            "fptaylor.sh's saving run can give up without showing the end of HOL Light's output and of DMTCP's log"
                .to_owned(),
        );
    }
    found
}

/// What is wrong with the job `saving`, which saves `state`.
fn states_problems(saving: &Yaml, state: &State) -> Vec<String> {
    let job = state.job;
    let mut found = Vec::new();
    if saving.is_badvalue() {
        found.push(format!("no job {job}"));
    } else {
        let all = steps(saving);
        if !all.iter().any(|s| text(&s["run"]).contains(state.saves)) {
            found.push(format!("{job} saves no state"));
        }
        let held = all.iter().any(|s| {
            let run = text(&s["run"]);
            run.contains(state.saves) && run.starts_with(PIPEFAIL) && run.contains(state.missing)
        });
        if !held {
            found.push(format!(
                "{job} can pass without a state for every problem it kept"
            ));
        }
        let output = text(&saving["outputs"]["sha256"]);
        let from_a_step = all.iter().any(|s| {
            let id = text(&s["id"]);
            !id.is_empty()
                && output == format!("${{{{ steps.{id}.outputs.sha256 }}}}")
                && text(&s["run"]).contains(state.saves)
        });
        if !from_a_step {
            found.push(format!("{job} gives no sha256 of its states"));
        }
        let uploads: Vec<&Yaml> = all
            .iter()
            .filter(|s| uses(s, "actions/upload-artifact") && text(&s["with"]["name"]) == job)
            .collect();
        if uploads.is_empty() {
            found.push(format!("{job} uploads no artifact {job}"));
        }
        for upload in uploads {
            if text(&upload["with"]["retention-days"]) != "1" {
                found.push(format!("{job} is kept longer than a day"));
            }
        }
    }
    found
}

/// Whether the job `job` restores a state.
fn restores(job: &Yaml) -> bool {
    steps(job)
        .iter()
        .any(|s| text(&s["run"]).contains("FPTAYLOR_STATE="))
}

/// What is wrong with the job `name`, `job`, which restores `state`.
fn restoring_problems(name: &str, job: &Yaml, state: &State) -> Vec<String> {
    let saved_by = state.job;
    let mut found = Vec::new();
    let all = steps(job);
    if !needs(job).iter().any(|n| n == saved_by) {
        found.push(format!(
            "{name} restores a state without needing {saved_by}"
        ));
    }
    if !text(&job["if"]).contains(&format!("needs.{saved_by}.result == 'success'")) {
        found.push(format!(
            "{name} can start without the state: it does not wait for {saved_by} to pass"
        ));
    }
    let downloads: Vec<&Yaml> = all
        .iter()
        .filter(|s| uses(s, "actions/download-artifact"))
        .collect();
    if !downloads
        .iter()
        .any(|s| text(&s["with"]["name"]) == saved_by)
    {
        found.push(format!("{name} does not download {saved_by}"));
    }
    for download in downloads {
        let artifact = text(&download["with"]["name"]);
        if STATES
            .iter()
            .any(|other| other.job == artifact && other.job != saved_by)
        {
            found.push(format!(
                "{name} downloads {artifact}, a state it does not restore"
            ));
        }
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
    for step in all
        .iter()
        .filter(|s| text(&s["run"]).contains("FPTAYLOR_STATE="))
    {
        if !text(&step["env"]["FPTAYLOR_STATE_SHA256"])
            .contains(&format!("needs.{saved_by}.outputs.sha256"))
        {
            found.push(format!(
                "{name}'s FPTAYLOR_STATE_SHA256 is not {saved_by}'s output"
            ));
        }
        if !text(&step["run"]).contains("-e FPTAYLOR_STATE_SHA256 ") {
            found.push(format!(
                "{name} skips the digest check: no FPTAYLOR_STATE_SHA256 in the container"
            ));
        }
    }
    found
}

/// What is wrong with how the workflow's jobs `jobs` share a problem's shards out: each shard
/// one job of the run, which runs whenever the others do and cannot pass unchecked.
fn shard_problems(jobs: &Yaml) -> Vec<String> {
    let mut found = Vec::new();
    let list = &jobs[LIST_JOB];
    let routed = steps(list).iter().any(|s| {
        let run = text(&s["run"]);
        text(&s["id"]) == "list" && ROUTING.iter().all(|line| run.contains(line))
    });
    let handed = ["shards", "pieces"].iter().all(|output| {
        text(&list["outputs"][*output]) == format!("${{{{ steps.list.outputs.{output} }}}}")
    });
    if !routed || !handed {
        found.push(format!(
            "{LIST_JOB} does not send every shard to exactly one job"
        ));
    }
    for (name, output) in [(SHARDS_JOB, "shards"), (PIECES_JOB, "pieces")] {
        let job = &jobs[name];
        if text(&job["strategy"]["matrix"])
            != format!("${{{{ fromJSON(needs.{LIST_JOB}.outputs.{output}) }}}}")
        {
            found.push(format!("{name} does not run every shard of its list"));
        }
        let all = steps(job);
        if !job["continue-on-error"].is_badvalue()
            || all.iter().any(|s| !s["continue-on-error"].is_badvalue())
        {
            found.push(format!("{name} can fail without failing the run"));
        }
        let when = text(&job["if"]);
        if ["github.event", "github.ref", "inputs."]
            .iter()
            .any(|event| when.contains(*event))
        {
            found.push(format!(
                "{name} runs on fewer events than the shards that assume what it proves"
            ));
        }
        let checks: Vec<&Yaml> = all
            .iter()
            .filter(|s| text(&s["run"]).contains("-e FPTAYLOR_SHARD "))
            .collect();
        let set_aside_by_the_rule = all
            .iter()
            .filter(|s| text(&s["id"]) == "base")
            .all(|s| text(&s["run"]).contains(SAME_AS_BASE));
        if checks.len() != 1
            || checks.iter().any(|s| text(&s["if"]) != NOT_SET_ASIDE)
            || !set_aside_by_the_rule
        {
            found.push(format!(
                "{name} can skip its check for a problem the base did not prove"
            ));
        }
    }
    found
}

/// What is wrong with the shards the problem `fpt`, of the file `file`, names: every piece of a
/// term it cuts is in exactly one shard, so one job, and no shard has pieces and whole terms.
/// With them, how many pieces its cut terms have.
fn pieces_problems(file: &str, fpt: &str) -> (Vec<String>, usize) {
    let mut found = Vec::new();
    let cuts: Vec<(u32, u32)> = fpt
        .lines()
        .filter_map(|l| l.strip_prefix("// hol-light-cut "))
        .map(|l| {
            let (t, d) = l.split_once(": ").unwrap();
            (t.parse().unwrap(), d.parse().unwrap())
        })
        .collect();
    let shards: Vec<Vec<&str>> = fpt
        .lines()
        .filter_map(|l| l.strip_prefix("// hol-light-shard "))
        .map(|l| l.split_once(": ").unwrap().1.split(' ').collect())
        .collect();
    let mut every = Vec::new();
    for (k, parts) in (1..).zip(&shards) {
        let pieces: Vec<&&str> = parts.iter().filter(|p| p.contains('.')).collect();
        if !pieces.is_empty() && pieces.len() != parts.len() {
            found.push(format!(
                "{file}: shard {k} has pieces and whole terms, and is one job"
            ));
        }
        for piece in pieces {
            let (t, p) = piece.split_once('.').unwrap();
            let (t, p): (u32, u32) = (t.parse().unwrap(), p.parse().unwrap());
            if !cuts
                .iter()
                .any(|&(c, d)| c == t && (1..=1 << d).contains(&p))
            {
                found.push(format!(
                    "{file}: shard {k} names {piece}, no piece of a cut term"
                ));
            }
        }
    }
    for (t, d) in cuts {
        for p in 1..=1u32 << d {
            let piece = format!("{t}.{p}");
            let jobs = shards
                .iter()
                .filter(|parts| parts.iter().any(|part| *part == piece))
                .count();
            if jobs != 1 {
                found.push(format!("{file}: piece {piece} is in {jobs} jobs"));
            }
            every.push(piece);
        }
    }
    (found, every.len())
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
    let download = "        if: steps.base.outputs.same != 'true'\n        with:\n          name: hol-light-states\n          path: ${{ runner.temp }}/states\n      #";
    finds(
        &planted(
            &workflow,
            download,
            "        if: steps.base.outputs.same != 'true'\n        with:\n          name: hol-light-states\n          path: ${{ runner.temp }}/states\n          run-id: 1\n          github-token: ${{ github.token }}\n      #",
        ),
        &script,
        "fptaylor-shards downloads with [\"run-id\", \"github-token\"]: from another run or repository",
    );
    finds(
        &planted(
            &workflow,
            "      - uses: actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333 # v8.0.2\n        if: steps.base.outputs.same != 'true'\n        with:\n          name: hol-light-states\n          path: ${{ runner.temp }}/states\n",
            "      - uses: actions/cache@0057852bfaa89a56745cba8c7296529d2fc39830 # v4.3.0\n        with:\n          key: hol-light-states\n          path: ${{ runner.temp }}/states\n",
        ),
        &script,
        "fptaylor-shards uses a cache",
    );
    finds(
        &planted(
            &workflow,
            "    needs: [fptaylor-problems, fptaylor-certificates, hol-light-states]\n    if: ${{ !cancelled() && needs.fptaylor-problems.result == 'success' && needs.fptaylor-problems.outputs.sharded != ''",
            "    needs: [fptaylor-problems, fptaylor-certificates]\n    if: ${{ !cancelled() && needs.fptaylor-problems.result == 'success' && needs.fptaylor-problems.outputs.sharded != ''",
        ),
        &script,
        "fptaylor-shards restores a state without needing hol-light-states",
    );
    finds(
        &planted(
            &workflow,
            "          path: ${{ runner.temp }}/states\n          retention-days: 1\n",
            "          path: ${{ runner.temp }}/states\n          retention-days: 30\n",
        ),
        &script,
        "hol-light-states is kept longer than a day",
    );
    finds(
        &planted(
            &workflow,
            "      sha256: ${{ steps.save.outputs.sha256 }}",
            "      sha256: ${{ steps.other.outputs.sha256 }}",
        ),
        &script,
        "hol-light-states gives no sha256",
    );

    // Planted: the digest check skipped, in the workflow or in the script.
    finds(
        &planted(
            &workflow,
            "-e FPTAYLOR_SHARD -e FPTAYLOR_CERTIFICATES=/certificates -e FPTAYLOR_STATE=/state \\\n            -e FPTAYLOR_STATE_SHA256 ",
            "-e FPTAYLOR_SHARD -e FPTAYLOR_CERTIFICATES=/certificates -e FPTAYLOR_STATE=/state \\\n            ",
        ),
        &script,
        "fptaylor-shards skips the digest check",
    );
    finds(
        &planted(
            &workflow,
            "FPTAYLOR_STATE_SHA256: ${{ fromJSON(needs.hol-light-states.outputs.sha256 || '{}')[matrix.problem] }}",
            "FPTAYLOR_STATE_SHA256: ${{ matrix.sha256 }}",
        ),
        &script,
        "fptaylor-shards's FPTAYLOR_STATE_SHA256 is not hol-light-states's output",
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
            "needs.fptaylor-problems.outputs.sharded != '' && needs.hol-light-states.result == 'success' }}",
            "needs.fptaylor-problems.outputs.sharded != '' }}",
        ),
        &script,
        "fptaylor-shards can start without the state",
    );
    finds(
        &planted(
            &workflow,
            "          set -o pipefail\n          mkdir -m 777 \"$RUNNER_TEMP/states\"\n",
            "          mkdir -m 777 \"$RUNNER_TEMP/states\"\n",
        ),
        &script,
        "hol-light-states can pass without a state",
    );
    finds(
        &planted(
            &workflow,
            LOADED.missing,
            r#"echo "$problem: no saved state""#,
        ),
        &script,
        "hol-light-states can pass without a state",
    );
}

/// The second state, which holds a cut term's Taylor theorems, is made by this run from the
/// first, handed on within the run only, and checked before a shard of pieces restores it.
#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn the_second_state_is_made_and_handed_on_within_its_run_only() {
    let workflow = read(WORKFLOW);
    let script = read(SCRIPT);
    let found = problems(&workflow, &script);
    assert!(found.is_empty(), "{found:#?}");

    // Planted: a second state from another run, or the first state in its place.
    let download = "          name: hol-light-cut-states\n          path: ${{ runner.temp }}/cut-states\n      #";
    finds(
        &planted(
            &workflow,
            download,
            "          name: hol-light-cut-states\n          path: ${{ runner.temp }}/cut-states\n          run-id: 1\n      #",
        ),
        &script,
        "fptaylor-pieces downloads with [\"run-id\"]: from another run or repository",
    );
    finds(
        &planted(
            &workflow,
            download,
            "          name: hol-light-states\n          path: ${{ runner.temp }}/cut-states\n      #",
        ),
        &script,
        "fptaylor-pieces downloads hol-light-states, a state it does not restore",
    );
    finds(
        &planted(
            &workflow,
            "    needs: [fptaylor-problems, fptaylor-certificates, hol-light-cut-states]",
            "    needs: [fptaylor-problems, fptaylor-certificates, hol-light-states]",
        ),
        &script,
        "fptaylor-pieces restores a state without needing hol-light-cut-states",
    );
    finds(
        &planted(
            &workflow,
            " && needs.hol-light-cut-states.result == 'success' }}",
            " }}",
        ),
        &script,
        "fptaylor-pieces can start without the state",
    );
    finds(
        &planted(
            &workflow,
            "          path: ${{ runner.temp }}/cut-states\n          retention-days: 1\n",
            "          path: ${{ runner.temp }}/cut-states\n          retention-days: 30\n",
        ),
        &script,
        "hol-light-cut-states is kept longer than a day",
    );

    // Planted: the second state's digest from somewhere other than its saving job's output, or
    // the first state's, for the job that makes the second.
    finds(
        &planted(
            &workflow,
            "FPTAYLOR_STATE_SHA256: ${{ fromJSON(needs.hol-light-cut-states.outputs.sha256 || '{}')[matrix.problem] }}",
            "FPTAYLOR_STATE_SHA256: ${{ fromJSON(needs.hol-light-states.outputs.sha256 || '{}')[matrix.problem] }}",
        ),
        &script,
        "fptaylor-pieces's FPTAYLOR_STATE_SHA256 is not hol-light-cut-states's output",
    );
    finds(
        &planted(
            &workflow,
            "          FPTAYLOR_STATE_SHA256: ${{ needs.hol-light-states.outputs.sha256 }}",
            "          FPTAYLOR_STATE_SHA256: ${{ vars.HOL_LIGHT_STATE_SHA256 }}",
        ),
        &script,
        "hol-light-cut-states's FPTAYLOR_STATE_SHA256 is not hol-light-states's output",
    );
    finds(
        &planted(
            &workflow,
            "-e FPTAYLOR_CUT_STATE_OUT=/cut-state -e FPTAYLOR_CERTIFICATES=/certificates -e FPTAYLOR_STATE=/state \\\n                -e FPTAYLOR_STATE_SHA256 ",
            "-e FPTAYLOR_CUT_STATE_OUT=/cut-state -e FPTAYLOR_CERTIFICATES=/certificates -e FPTAYLOR_STATE=/state \\\n                ",
        ),
        &script,
        "hol-light-cut-states skips the digest check",
    );
}

/// The job that saves the second state cannot pass without one for every problem it kept, and
/// fptaylor.sh saves one only from a run that built each cut term's theorems and proved nothing.
#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn a_shard_of_pieces_never_starts_without_its_state() {
    let workflow = read(WORKFLOW);
    let script = read(SCRIPT);
    let found = problems(&workflow, &script);
    assert!(found.is_empty(), "{found:#?}");

    // Planted: a saving job that can pass with no second state, so that the shards of pieces
    // would start without one.
    finds(
        &planted(
            &workflow,
            "          set -o pipefail\n          mkdir -m 777 \"$RUNNER_TEMP/cut-states\"\n",
            "          mkdir -m 777 \"$RUNNER_TEMP/cut-states\"\n",
        ),
        &script,
        "hol-light-cut-states can pass without a state",
    );
    finds(
        &planted(
            &workflow,
            CUT.missing,
            r#"echo "$problem: no saved cut state""#,
        ),
        &script,
        "hol-light-cut-states can pass without a state",
    );

    // Planted: a second state saved by a run that proved a term, or built no theorems.
    finds(
        &workflow,
        &planted(&script, CUT_PROVED_NOTHING, "true ||"),
        "can save a cut state",
    );
    finds(
        &workflow,
        &planted(&script, CUT_BUILT, r#"[ -n "$built" ] ||"#),
        "can save a cut state",
    );
}

/// The run that saves a second state knows the HOL Light it restored by its output, never by
/// `dmtcp_restart`'s process, and shows what HOL Light and DMTCP last said when it gives up.
#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn a_restored_hol_light_is_known_by_its_output_alone() {
    let workflow = read(WORKFLOW);
    let script = read(SCRIPT);
    let found = problems(&workflow, &script);
    assert!(found.is_empty(), "{found:#?}");

    // Planted: the restart as run 44076902825 had it, its output in a file and `dmtcp_restart`'s
    // process watched. In a container that process ends at once, HOL Light restored and running
    // in another, and the saving run gave up after five seconds.
    let piped = "\"$work/hol.dmtcp\") <\"$work/input\" 2>&1 |\n    tee \"$work/hol.log\" | grep --line-buffered -v '^val \\|^- : ' &\n";
    finds(
        &workflow,
        &planted(
            &script,
            piped,
            "\"$work/hol.dmtcp\") \\\n    <\"$work/input\" >\"$work/hol.log\" 2>&1 &\n",
        ),
        "takes dmtcp_restart's process for HOL Light's",
    );
    // Planted: the output still read by `tee`, but `$!` again `dmtcp_restart`'s; and HOL Light
    // looked for by name instead of held by its output.
    finds(
        &workflow,
        &planted(
            &script,
            piped,
            "\"$work/hol.dmtcp\") <\"$work/input\" > >(tee \"$work/hol.log\" | grep --line-buffered -v '^val \\|^- : ') 2>&1 &\n",
        ),
        "does not hold the HOL Light it restores to save again by its output",
    );
    finds(
        &workflow,
        &planted(
            &script,
            "'^val \\|^- : ' &\n  hol_ends=$!\n  exec 3>\"$work/input\"\n  cat \"$session\" >&3\n",
            "'^val \\|^- : ' &\n  hol_ends=$(pgrep -n ocaml-hol)\n  exec 3>\"$work/input\"\n  cat \"$session\" >&3\n",
        ),
        "does not hold the HOL Light it restores to save again by its output",
    );

    // Planted: a saving run that gives up and shows nothing of HOL Light or DMTCP, as that run did.
    let silent = "can give up without showing the end of HOL Light's output and of DMTCP's log";
    finds(
        &workflow,
        &planted(
            &script,
            STOPS[1],
            r#"kill -0 "$hol_ends" 2>/dev/null || { echo "HOL Light stopped before the cut terms were built"; exit 1; }"#,
        ),
        silent,
    );
    finds(&workflow, &planted(&script, STOP_SHOWS, "true;"), silent);
    finds(
        &workflow,
        &planted(
            &script,
            "--coord-logfile \"$work/coordinator.log\" \\\n    ",
            "",
        ),
        silent,
    );
}

/// Every shard is one job of the run, so nothing one shard assumes goes unchecked in it: each
/// piece of a cut term is in exactly one shard of the committed problem, and the workflow runs
/// every shard, of pieces or of whole terms, whenever it runs any.
#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn every_piece_is_checked_in_exactly_one_job_of_the_same_run() {
    let workflow = read(WORKFLOW);
    let script = read(SCRIPT);
    let found = problems(&workflow, &script);
    assert!(found.is_empty(), "{found:#?}");

    // The committed problems: line_cone's term 38 in 64 pieces, each in one shard.
    let mut pieces = 0;
    for entry in std::fs::read_dir(root().join(PROBLEMS)).unwrap() {
        let file = entry.unwrap().file_name().into_string().unwrap();
        if file.starts_with("fptaylor-")
            && Path::new(&file)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("fpt"))
        {
            let (found, count) = pieces_problems(&file, &read(&format!("{PROBLEMS}/{file}")));
            assert!(found.is_empty(), "{found:#?}");
            pieces += count;
        }
    }
    assert_eq!(pieces, 64, "the pieces of the cut terms");

    // Planted, in line_cone's problem: a piece in no job, in two, and beside whole terms.
    let file = "fptaylor-line-cone.fpt";
    let fpt = read(&format!("{PROBLEMS}/{file}"));
    let finds_in = |fpt: &str, what: &str| {
        let (found, _) = pieces_problems(file, fpt);
        assert!(found.iter().any(|p| p.contains(what)), "{what}: {found:#?}");
    };
    finds_in(
        &planted(&fpt, ": 38.4 38.5 38.6\n", ": 38.4 38.6\n"),
        "piece 38.5 is in 0 jobs",
    );
    finds_in(
        &planted(&fpt, ": 38.4 38.5 38.6\n", ": 38.4 38.5 38.6 38.7\n"),
        "piece 38.7 is in 2 jobs",
    );
    finds_in(
        &planted(&fpt, ": 38.4 38.5 38.6\n", ": 37 38.4 38.5 38.6\n"),
        "has pieces and whole terms",
    );
    finds_in(
        &planted(&fpt, ": 38.4 38.5 38.6\n", ": 38.4 38.5 38.6 38.65\n"),
        "names 38.65, no piece of a cut term",
    );

    // Planted, in the workflow: a shard of pieces sent to no job, a job that runs only some of
    // its shards or only on some events, and one that can fail or skip its check and pass.
    finds(
        &planted(
            &workflow,
            "select(.pieces == true)",
            "select(.pieces == true and .shard < 20)",
        ),
        &script,
        "fptaylor-problems does not send every shard to exactly one job",
    );
    finds(
        &planted(
            &workflow,
            "{ echo \"a shard is in neither list\"; exit 1; }",
            "echo \"a shard is in neither list\"",
        ),
        &script,
        "fptaylor-problems does not send every shard to exactly one job",
    );
    finds(
        &planted(
            &workflow,
            "matrix: ${{ fromJSON(needs.fptaylor-problems.outputs.pieces) }}",
            "matrix: ${{ fromJSON(needs.fptaylor-problems.outputs.shards) }}",
        ),
        &script,
        "fptaylor-pieces does not run every shard of its list",
    );
    finds(
        &planted(
            &workflow,
            "needs.fptaylor-problems.outputs.cut != '' && needs.hol-light-cut-states.result == 'success' }}",
            "needs.fptaylor-problems.outputs.cut != '' && github.event_name == 'workflow_dispatch' && needs.hol-light-cut-states.result == 'success' }}",
        ),
        &script,
        "fptaylor-pieces runs on fewer events",
    );
    finds(
        &planted(
            &workflow,
            "  fptaylor-pieces:\n",
            "  fptaylor-pieces:\n    continue-on-error: true\n",
        ),
        &script,
        "fptaylor-pieces can fail without failing the run",
    );
    finds(
        &planted(
            &workflow,
            "      - name: Check this shard's pieces of the certificate\n        if: steps.base.outputs.same != 'true'\n",
            "      - name: Check this shard's pieces of the certificate\n        if: steps.base.outputs.same != 'true' && matrix.shard < 20\n",
        ),
        &script,
        "fptaylor-pieces can skip its check",
    );
}
