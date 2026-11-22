//! Tests of the `oracle` binary, run as CI runs it, on a scratch tree (piece G113).
//!
//! What this file proves: `check` holds on the real tree and audits a checkout of the oracle
//! repository; `jobs` and `compare` run the round a night runs, with a stand-in answer file;
//! `compare` fails on a difference not triaged, a second cause under a triaged signature among
//! them, and not on the triaged job itself; `triage` writes the record, pinned to its job, and the
//! fixture, and will not record a signature on a job twice.
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
use geom_test::fixture::repo::{DIR, embedded};
use geom_test::oracle::exact::{float, read_float};
use geom_test::oracle::geomdl::{Answer, Job};
use geom_test::oracle::meshes;
use geom_test::oracle::triage::{REGISTER_PATH, Register, job_digest, logged_job};
use geom_test::seed::Seed;

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

/// The case sets as `kernel-oracles.yml`'s job `meshes` asks for them: the seeded set with its
/// seed and count, a stored set with neither, and anything else refused as a usage error.
#[test]
fn a_mesh_oracle_s_jobs_come_from_its_case_set() {
    let seeded = tool(&["jobs", "manifold", "--seed", "ab", "--count", "5"]);
    assert!(seeded.status.success(), "{}", text(&seeded));
    let named = tool(&[
        "jobs", "manifold", "--cases", "seeded", "--seed", "ab", "--count", "5",
    ]);
    assert_eq!(named.stdout, seeded.stdout, "seeded is the default");
    let models = tool(&["jobs", "cgal", "--cases", "manifold-models"]);
    assert!(models.status.success(), "{}", text(&models));
    // The four models recorded as refused are not asked: 12 of the 69 jobs.
    assert_eq!(String::from_utf8_lossy(&models.stdout).lines().count(), 57);
    assert!(
        text(&models)
            .contains("cases: manifold-models, 57 jobs, 12 more recorded as refused and not asked")
    );
    // A comparison names the case set its jobs are of.
    let dir = scratch("cases");
    let jobs = dir.join("jobs.jsonl");
    fs::write(&jobs, &seeded.stdout).unwrap();
    let refusals: Vec<String> = (0..5).map(|id| format!("{{\"id\":{id}}}")).collect();
    let answers = dir.join("answers.jsonl");
    fs::write(&answers, refusals.join("\n") + "\n").unwrap();
    let (j, a) = (jobs.to_str().unwrap(), answers.to_str().unwrap());
    let wrong_set = tool(&[
        "compare",
        "manifold",
        "--cases",
        "manifold-models",
        "--jobs",
        j,
        "--answers",
        a,
    ]);
    assert_eq!(wrong_set.status.code(), Some(2), "{}", text(&wrong_set));
    assert!(text(&wrong_set).contains("not of the case set manifold-models"));
    let geomdl_set = tool(&[
        "compare",
        "geomdl",
        "--cases",
        "manifold-models",
        "--jobs",
        j,
        "--answers",
        a,
    ]);
    assert_eq!(geomdl_set.status.code(), Some(2));
    for refused in [
        &["jobs", "manifold", "--cases", "seeded"][..],
        &[
            "jobs",
            "manifold",
            "--cases",
            "manifold-models",
            "--seed",
            "ab",
            "--count",
            "5",
        ],
        &["jobs", "manifold", "--cases", "ember-pairs"],
        &["jobs", "geomdl", "--cases", "manifold-models"],
        &["jobs", "manifold", "--seed", "ab"],
    ] {
        assert_eq!(tool(refused).status.code(), Some(2), "{refused:?}");
    }
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
    assert!(text(&out).contains("is not triaged on this job"));
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

/// `job`'s own answer, each triangle moved 1,000 steps along x: the volume and area stand, so the
/// one difference is the Hausdorff distance.
fn moved_answer(job: &meshes::Job) -> String {
    let mut answer = meshes::own_answer(job).unwrap();
    for triangle in answer.triangles.as_mut().unwrap() {
        for corner in triangle.iter_mut() {
            let x = read_float(&corner[0]).unwrap() + 1000.0;
            corner[0] = float(x).unwrap();
        }
    }
    serde_json::to_string(&answer).unwrap()
}

/// `oracle compare` on one Manifold job and its moved answer.
fn compared_one(dir: &Path, job: &meshes::Job) -> Output {
    let (jobs, answers) = (dir.join("jobs.jsonl"), dir.join("answers.jsonl"));
    fs::write(&jobs, serde_json::to_string(job).unwrap() + "\n").unwrap();
    fs::write(&answers, moved_answer(job) + "\n").unwrap();
    tool(&[
        "compare",
        "manifold",
        "--jobs",
        jobs.to_str().unwrap(),
        "--answers",
        answers.to_str().unwrap(),
    ])
}

/// Issue 880: a Hausdorff difference on a Manifold union of tetrahedra, on a job other than the one
/// record 0001 was triaged on, has that record's signature but not its cause, so it fails the run;
/// the same difference on the triaged job itself stays known.
#[test]
fn a_second_cause_under_a_triaged_signature_fails_the_run() {
    let dir = scratch("second-cause");
    let record = Register::embedded()
        .unwrap()
        .records
        .into_iter()
        .find(|r| r.fixture == "meshes/0001-union-fold-in-the-export")
        .unwrap();
    assert_eq!(
        record.signature.to_string(),
        "mesh.union/manifold/hausdorff/tetrahedra"
    );
    let stored = embedded()
        .into_iter()
        .find(|f| f.path == "meshes/0001-union-fold-in-the-export.json")
        .unwrap();
    let triaged: meshes::Job =
        serde_json::from_str(&logged_job(&Fixture::parse(&stored.text).unwrap().log).unwrap())
            .unwrap();
    let other = meshes::jobs(Seed(0xab), 400)
        .into_iter()
        .find(|j| j.family == "tetrahedra" && j.op == meshes::Op::Union)
        .unwrap();
    assert_ne!(other, triaged);
    for (job, code, untriaged) in [(&triaged, 0, false), (&other, 1, true)] {
        let out = compared_one(&dir, job);
        let said = text(&out);
        assert!(
            said.contains("mesh.union/manifold/hausdorff/tetrahedra: 1"),
            "{said}"
        );
        assert_eq!(out.status.code(), Some(code), "{said}");
        assert_eq!(
            said.contains("is not triaged on this job"),
            untriaged,
            "{said}"
        );
    }
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
            format: 2,
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
    // The record pins the job it was triaged on: the line `jobs` wrote, which its fixture logs.
    let line = serde_json::to_string(&list[3]).unwrap();
    assert_eq!(logged_job(&f.log).unwrap(), line);
    assert_eq!(saved.records[0].job, job_digest(&line));
    // Each signature is judged once on its job.
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
