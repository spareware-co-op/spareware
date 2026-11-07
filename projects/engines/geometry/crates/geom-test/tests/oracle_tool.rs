//! Tests of the `oracle` binary, run as CI runs it, on a scratch tree (piece G113).
//!
//! What this file proves: `check` holds on the real tree and audits a checkout of the oracle
//! repository; `jobs` and `compare` run the round a night runs, with a stand-in answer file;
//! `compare` fails on a new signature and not on a triaged one; `triage` writes the record and the
//! fixture and will not record a signature twice.
//!
//! Native only: WebAssembly has no processes.
#![cfg(not(target_family = "wasm"))]
#![allow(
    clippy::arithmetic_side_effects,
    clippy::items_after_statements,
    clippy::disallowed_methods,
    reason = "test cases; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

mod common;
use common::{flawed, honest};

use geom_test::fixture::repo::DIR;
use geom_test::oracle::catalogue::Catalogue;
use geom_test::oracle::geomdl::{Answer, ID, Job, Op};
use geom_test::oracle::triage::{REGISTER_PATH, Register, Settled};

fn tool(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_oracle"))
        .args(args)
        .output()
        .unwrap()
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oracle-tool-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn jobs_file(dir: &Path) -> (PathBuf, Vec<Job>) {
    let out = tool(&["jobs", "geomdl", "--seed", "ab", "--count", "20"]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(String::from_utf8_lossy(&out.stderr).contains("seed: 00000000000000ab"));
    let path = dir.join("jobs.jsonl");
    fs::write(&path, &out.stdout).unwrap();
    let jobs = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    (path, jobs)
}

fn answers(dir: &Path, jobs: &[Job]) -> PathBuf {
    // A refusal of every job: a difference the kernel does not share, one signature per family.
    let lines: Vec<String> = jobs
        .iter()
        .map(|j| {
            serde_json::to_string(&Answer {
                id: j.id,
                values: None,
            })
            .unwrap()
        })
        .collect();
    let path = dir.join("answers.jsonl");
    fs::write(&path, lines.join("\n") + "\n").unwrap();
    path
}

#[test]
fn the_real_tree_is_checked() {
    let out = tool(&["check"]);
    assert!(out.status.success(), "{}", text(&out));
    let out = tool(&["list"]);
    assert!(text(&out).contains("geomdl"));
    assert_eq!(
        tool(&["jobs", "nobody", "--seed", "1", "--count", "1"])
            .status
            .code(),
        Some(2)
    );
    assert_eq!(
        tool(&["jobs", "geomdl", "--seed", "x", "--count", "1"])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn a_new_signature_fails_the_night() {
    let dir = scratch("night");
    let (jobs, list) = jobs_file(&dir);
    let refusals = answers(&dir, &list);
    let at = |p: &PathBuf| p.to_str().unwrap().to_owned();
    let (j, a) = (at(&jobs), at(&refusals));
    let out = tool(&["compare", "geomdl", "--jobs", &j, "--answers", &a]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert!(text(&out).contains("new signature"));
    // One answer short: the harness stopped, which is no pass.
    fs::write(&refusals, "").unwrap();
    assert_eq!(
        tool(&["compare", "geomdl", "--jobs", &j, "--answers", &a])
            .status
            .code(),
        Some(2)
    );
    fs::remove_dir_all(&dir).unwrap();
}

/// The stand-in reference: the kernel's own answer to each job, as an outside oracle writes it,
/// with a flaw on the job `flaw` names, if any.
fn reference(dir: &Path, name: &str, jobs: &[Job], flaw: Option<(u64, f64)>) -> PathBuf {
    let lines: Vec<String> = jobs
        .iter()
        .map(|j| {
            let answer = match flaw {
                Some((id, delta)) if id == j.id => flawed(j, delta),
                _ => honest(j),
            };
            serde_json::to_string(&answer).unwrap()
        })
        .collect();
    let path = dir.join(name);
    fs::write(&path, lines.join("\n") + "\n").unwrap();
    path
}

fn first_point(jobs: &[Job]) -> &Job {
    jobs.iter().find(|j| j.op == Op::Point).unwrap()
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

/// A scratch tree holding an empty register, the root `triage` writes into.
fn root_with_register(dir: &Path) -> PathBuf {
    let root = dir.join("tree");
    let register = REGISTER_PATH
        .iter()
        .fold(root.clone(), |p, part| p.join(part));
    fs::create_dir_all(register.parent().unwrap()).unwrap();
    fs::write(
        &register,
        Register {
            format: 1,
            about: "a".to_owned(),
            records: Vec::new(),
        }
        .to_json()
        .unwrap(),
    )
    .unwrap();
    root
}

fn register_of(root: &Path) -> Register {
    let path = REGISTER_PATH
        .iter()
        .fold(root.to_path_buf(), |p, part| p.join(part));
    Register::parse(&fs::read_to_string(path).unwrap()).unwrap()
}

fn fixtures_written(root: &Path) -> usize {
    let folder = DIR.iter().fold(root.to_path_buf(), |p, part| p.join(part));
    fn count(path: &Path) -> usize {
        fs::read_dir(path).map_or(0, |entries| {
            entries
                .map(|e| e.unwrap().path())
                .map(|p| if p.is_dir() { count(&p) } else { 1 })
                .sum()
        })
    }
    count(&folder)
}

const ISSUE: &str = "https://github.com/orbingol/NURBS-Python/issues/1";

/// The arguments of a triage of `job` as `outcome`, from `answers`, into `root`.
fn triage_args<'a>(
    jobs: &'a Path,
    id: &'a str,
    outcome: &'a str,
    answers: &'a Path,
    root: &'a Path,
) -> Vec<&'a str> {
    vec![
        "triage",
        "geomdl",
        "--jobs",
        s(jobs),
        "--id",
        id,
        "--kind",
        "value",
        "--outcome",
        outcome,
        "--fixture",
        "oracle/0001-case",
        "--why",
        "w",
        "--answers",
        s(answers),
        "--root",
        s(root),
    ]
}

#[test]
fn a_flawed_reference_fails_the_night_once_and_is_triaged_with_its_evidence() {
    let dir = scratch("flawed");
    let (jobs, list) = jobs_file(&dir);
    let point = first_point(&list);
    let id = point.id.to_string();
    let honest_answers = reference(&dir, "honest.jsonl", &list, None);
    let flawed_answers = reference(&dir, "flawed.jsonl", &list, Some((point.id, 1.0)));
    // The honest reference agrees with the kernel on every job: nothing to triage, a clean night.
    let out = tool(&[
        "compare",
        "geomdl",
        "--jobs",
        s(&jobs),
        "--answers",
        s(&honest_answers),
    ]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("0 differences"), "{}", text(&out));
    // The flawed one differs on one job, which the exact reference settles as the oracle's.
    let out = tool(&[
        "compare",
        "geomdl",
        "--jobs",
        s(&jobs),
        "--answers",
        s(&flawed_answers),
    ]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert!(
        text(&out).contains("theirs: the kernel is within its bound"),
        "{}",
        text(&out)
    );
    assert!(text(&out).contains("1 differences, 1 new signatures, 0 contradicted"));

    // Triaged as theirs, it is recorded with the report, the catalogue's pin and the input.
    let root = root_with_register(&dir);
    let mut args = triage_args(&jobs, &id, "theirs", &flawed_answers, &root);
    args.extend(["--issue", ISSUE]);
    let out = tool(&args);
    assert!(out.status.success(), "{}", text(&out));
    let saved = register_of(&root);
    let record = &saved.records[0];
    assert_eq!(saved.records.len(), 1);
    assert_eq!(record.settled, Settled::Theirs);
    let evidence = record.evidence.as_ref().unwrap();
    assert_eq!(evidence.issue, ISSUE);
    let pin = Catalogue::embedded()
        .unwrap()
        .get(ID)
        .unwrap()
        .pin
        .clone()
        .unwrap();
    assert_eq!(evidence.reproduced_on, pin);
    let reproducer: Job = serde_json::from_str(&evidence.reproducer).unwrap();
    assert_eq!(&reproducer, point);
    assert_eq!(fixtures_written(&root), 1);
    // Each signature is judged once.
    assert_eq!(tool(&args).status.code(), Some(1));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn triage_refuses_what_a_record_cannot_stand_on_and_writes_nothing() {
    let dir = scratch("refused");
    let (jobs, list) = jobs_file(&dir);
    let point = first_point(&list);
    let id = point.id.to_string();
    let honest_answers = reference(&dir, "honest.jsonl", &list, None);
    let flawed_answers = reference(&dir, "flawed.jsonl", &list, Some((point.id, 1.0)));
    let root = root_with_register(&dir);
    let run = |args: Vec<&str>| tool(&args).status.code();
    let nothing_written = || register_of(&root).records.is_empty() && fixtures_written(&root) == 0;

    // "Theirs" without its report, with a report that is no address, or on no difference.
    let theirs = triage_args(&jobs, &id, "theirs", &flawed_answers, &root);
    assert_eq!(run(theirs.clone()), Some(2));
    let mut bad_address = theirs.clone();
    bad_address.extend(["--issue", "issue 1"]);
    assert_eq!(run(bad_address), Some(2));
    let mut agrees = triage_args(&jobs, &id, "theirs", &honest_answers, &root);
    agrees.extend(["--issue", ISSUE]);
    assert_eq!(run(agrees), Some(2));
    // The verdict is judged again, not typed: "within bounds" on a job the oracle got wrong.
    assert_eq!(
        run(triage_args(
            &jobs,
            &id,
            "within-bounds",
            &flawed_answers,
            &root
        )),
        Some(2)
    );
    // No answers, a wrong kind, an answer of another job.
    let mut no_answers = triage_args(&jobs, &id, "theirs", &flawed_answers, &root);
    no_answers.truncate(14);
    no_answers.extend(["--root", s(&root), "--issue", ISSUE]);
    assert_eq!(run(no_answers), Some(2));
    let mut kind = triage_args(&jobs, &id, "theirs", &flawed_answers, &root);
    kind[7] = "shape";
    kind.extend(["--issue", ISSUE]);
    assert_eq!(run(kind), Some(2));
    // "Ours" is recorded after its fix: a job the kernel still differs on is not fixed.
    let mut open = triage_args(&jobs, &id, "ours", &flawed_answers, &root);
    open.extend(["--reference", "#7"]);
    assert_eq!(run(open), Some(2));
    assert!(nothing_written());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_fixed_bug_is_recorded_as_ours_once_its_job_agrees() {
    let dir = scratch("ours");
    let (jobs, list) = jobs_file(&dir);
    let point = first_point(&list);
    let id = point.id.to_string();
    let fixed = reference(&dir, "fixed.jsonl", &list, None);
    let root = root_with_register(&dir);
    let mut args = triage_args(&jobs, &id, "ours", &fixed, &root);
    args.extend(["--reference", "#7"]);
    let out = tool(&args);
    assert!(out.status.success(), "{}", text(&out));
    let saved = register_of(&root);
    assert_eq!(saved.records[0].settled, Settled::Ours);
    assert!(saved.records[0].evidence.is_none());
    assert_eq!(fixtures_written(&root), 1);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_night_is_held_to_the_last_one() {
    let dir = scratch("ratchet");
    let out = tool(&["jobs", "geomdl", "--seed", "cd", "--count", "80"]);
    let big = dir.join("big.jsonl");
    fs::write(&big, &out.stdout).unwrap();
    let big_jobs: Vec<Job> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let answers = reference(&dir, "big-answers.jsonl", &big_jobs, None);
    let base = dir.join("base.json");
    let out = tool(&[
        "compare",
        "geomdl",
        "--jobs",
        s(&big),
        "--answers",
        s(&answers),
        "--report",
        s(&base),
    ]);
    assert!(out.status.success(), "{}", text(&out));
    let report = fs::read_to_string(&base).unwrap();
    assert!(report.contains("\"oracle\": \"geomdl\""));
    assert!(report.contains("\"jobs\": 80"));
    // The same night is held to itself.
    let out = tool(&[
        "compare",
        "geomdl",
        "--jobs",
        s(&big),
        "--answers",
        s(&answers),
        "--baseline",
        s(&base),
    ]);
    assert!(out.status.success(), "{}", text(&out));
    // A night of a few jobs covers fewer cells: the ratchet names what was lost, and the embedded
    // waivers, which hold none, cover nothing.
    let out = tool(&["jobs", "geomdl", "--seed", "cd", "--count", "3"]);
    let small = dir.join("small.jsonl");
    fs::write(&small, &out.stdout).unwrap();
    let small_jobs: Vec<Job> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let small_answers = reference(&dir, "small-answers.jsonl", &small_jobs, None);
    let out = tool(&[
        "compare",
        "geomdl",
        "--jobs",
        s(&small),
        "--answers",
        s(&small_answers),
        "--baseline",
        s(&base),
    ]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert!(text(&out).contains("coverage nurbs."), "{}", text(&out));
    assert!(
        text(&out).contains("the earlier night drew"),
        "{}",
        text(&out)
    );
    // A baseline that is not a report is a broken input, not a pass.
    fs::write(&base, "{").unwrap();
    let out = tool(&[
        "compare",
        "geomdl",
        "--jobs",
        s(&small),
        "--answers",
        s(&small_answers),
        "--baseline",
        s(&base),
    ]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_comparison_is_shown_to_see_a_moved_answer() {
    let dir = scratch("perturb");
    let (jobs, list) = jobs_file(&dir);
    let point = first_point(&list);
    let honest_answers = reference(&dir, "honest.jsonl", &list, None);
    let out = tool(&[
        "compare",
        "geomdl",
        "--jobs",
        s(&jobs),
        "--answers",
        s(&honest_answers),
        "--perturb",
    ]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(
        text(&out).contains("perturbed 20, caught 20, nothing to move 0: SENSITIVE"),
        "{}",
        text(&out)
    );
    // A job the reference got wrong is not moved: it already differs.
    let flawed_answers = reference(&dir, "flawed.jsonl", &list, Some((point.id, 1.0)));
    let out = tool(&[
        "compare",
        "geomdl",
        "--jobs",
        s(&jobs),
        "--answers",
        s(&flawed_answers),
        "--perturb",
    ]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert!(
        text(&out).contains("perturbed 19, caught 19"),
        "{}",
        text(&out)
    );
    fs::remove_dir_all(&dir).unwrap();
}
