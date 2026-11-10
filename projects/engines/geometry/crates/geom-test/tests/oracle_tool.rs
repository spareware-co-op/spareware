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
    clippy::disallowed_methods,
    reason = "test cases; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use geom_test::fixture::Fixture;
use geom_test::fixture::repo::DIR;
use geom_test::oracle::geomdl::{Answer, Job};
use geom_test::oracle::triage::{REGISTER_PATH, Register};

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

#[test]
fn triage_writes_the_record_and_the_fixture_once() {
    let dir = scratch("triage");
    let register = REGISTER_PATH
        .iter()
        .fold(dir.clone(), |p, part| p.join(part));
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
    let (jobs, list) = jobs_file(&dir);
    let args = [
        "triage",
        "geomdl",
        "--jobs",
        jobs.to_str().unwrap(),
        "--id",
        "3",
        "--kind",
        "value",
        "--outcome",
        "within-bounds",
        "--fixture",
        "oracle/0001-case",
        "--why",
        "w",
        "--root",
        dir.to_str().unwrap(),
    ];
    let out = tool(&args);
    assert!(out.status.success(), "{}", text(&out));
    let stored = DIR
        .iter()
        .fold(dir.clone(), |p, part| p.join(part))
        .join("oracle/0001-case.json");
    let f = Fixture::parse(&fs::read_to_string(stored).unwrap()).unwrap();
    assert_eq!(f.log.operations[0].name, list[3].op.name());
    let saved = Register::parse(&fs::read_to_string(&register).unwrap()).unwrap();
    assert_eq!(saved.records.len(), 1);
    assert_eq!(saved.records[0].signature.family, list[3].family);
    // Each signature is judged once.
    assert_eq!(tool(&args).status.code(), Some(1));
    // A bad outcome and an unknown job are usage errors.
    let mut bad = args;
    bad[9] = "maybe";
    assert_eq!(tool(&bad).status.code(), Some(2));
    let mut bad = args;
    bad[5] = "9999";
    assert_eq!(tool(&bad).status.code(), Some(2));
    fs::remove_dir_all(&dir).unwrap();
}
