//! Tests of the oracle runner and triage tooling (piece G113), through the public interface.
//!
//! The board's done-when: "Separate oracle repository; each oracle in its own container built from
//! pinned source, never linked or published (checked); exact-input writer; exact and bounded
//! comparisons; signature grouping; four-outcome triage writing fixtures; the first oracle, geomdl,
//! on NURBS."
//!
//! What this file proves:
//!
//! - The catalogue holds 28.10's starting set; its rules refuse a copyleft crate as a dependency,
//!   an unpinned live oracle, and the rest; `linked` finds a linked oracle in a lock file, and
//!   `audit` finds a published image and an unpinned base.
//! - Inputs read back bit for bit, and a change of unit states a bound the exact value keeps.
//! - The allowance is what 28.10 says, capped whatever the oracle states.
//! - A difference has one signature, each is judged once, and every outcome ends as its fixture.
//! - geomdl's jobs are the same from one seed; a correct answer agrees; a wrong one is a difference
//!   the exact reference settles; a refusal, a wrong shape and a broken answer are each caught.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    clippy::float_cmp,
    clippy::too_many_lines,
    reason = "test cases; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use dashu_int::{IBig, UBig};
use dashu_ratio::RBig;
use geom_num::GridStep;
use geom_nurbs::{Basis, Curve, Limit};
use geom_op::ErrorKind;
use geom_run::{Budget, WorkMeter};
use geom_test::fixture::run::Replayed;
use geom_test::fixture::{Class, Counts, Expect, Hash, Measures, OpLog, Source, Status, Triage};
use geom_test::oracle::catalogue::{Catalogue, CatalogueError, Oracle, Run, State, audit, linked};
use geom_test::oracle::compare::{Allowance, exact, within};
use geom_test::oracle::exact::{float, read_float, steps, to_millimetres};
use geom_test::oracle::geomdl::{Answer, Compared, ID, Job, Op, jobs, judge};
use geom_test::oracle::triage::{Record, Register, Signature, TriageError, fixture, group};
use geom_test::seed::Seed;

fn oracle(id: &str, licence: &str, run: Run) -> Oracle {
    Oracle {
        id: id.to_owned(),
        what: "w".to_owned(),
        licence: licence.to_owned(),
        run,
        state: State::Planned,
        checks: "c".to_owned(),
        source: "s".to_owned(),
        pin: None,
        never_linked: Vec::new(),
        piece: "G1".to_owned(),
    }
}

fn parsed(oracles: Vec<Oracle>) -> Result<Catalogue, CatalogueError> {
    let c = Catalogue {
        format: 1,
        about: "a".to_owned(),
        repository: "r".to_owned(),
        oracles,
    };
    Catalogue::parse(&serde_json::to_string(&c).unwrap())
}

fn rule(o: Oracle) -> String {
    match parsed(vec![o]) {
        Err(CatalogueError::Rule { what, .. }) => what,
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_catalogue_holds_the_starting_set() {
    let c = Catalogue::embedded().unwrap();
    for id in [
        "cgal",
        "opencascade",
        "smlib-nlib",
        "geomdl",
        "ifcconvert",
        "web-ifc",
        "manifold",
        "manifold-rust",
        "opensubdiv",
        "libfive",
        "fidget",
        "jts",
        "clipper2-rust",
        "i-overlay",
        "cavalier-contours",
        "ezpz",
        "solvespace",
        "planegcs",
        "ezdxf",
        "librecad",
        "spade",
        "startin",
        "proj",
        "geographiclib",
        "krbn",
    ] {
        assert!(c.get(id).is_some(), "{id}");
    }
    let geomdl = c.get(ID).unwrap();
    assert_eq!((geomdl.run, geomdl.state), (Run::OutOfProcess, State::Live));
    assert!(c.get("no-such").is_none());
    // The real lock file links none of them.
    let lock = include_str!("../../../../../../Cargo.lock");
    assert_eq!(linked(&c, lock).unwrap(), Vec::<String>::new());
    Register::embedded().unwrap();
}

#[test]
fn the_catalogue_keeps_its_rules() {
    assert!(parsed(vec![oracle("a", "MIT", Run::DevDependency)]).is_ok());
    assert!(parsed(vec![oracle("a", "BSL-1.0", Run::DevDependency)]).is_ok());
    assert!(rule(oracle("a", "GPL-3.0-only", Run::DevDependency)).contains("not permissive"));
    assert!(rule(oracle("a", "LGPL-2.1-only", Run::KernelNightly)).contains("not permissive"));
    assert!(rule(oracle("Not Ok", "MIT", Run::OutOfProcess)).contains("id"));
    let mut live = oracle("a", "MIT", Run::OutOfProcess);
    live.state = State::Live;
    assert!(rule(live.clone()).contains("pinned"));
    live.pin = Some("1".to_owned());
    assert!(parsed(vec![live.clone()]).is_ok());
    let mut linked_dev = oracle("a", "MIT", Run::DevDependency);
    linked_dev.never_linked = vec!["x".to_owned()];
    assert!(rule(linked_dev).contains("linked"));
    assert!(rule(oracle("a", "GPL-3.0-only", Run::OutOfProcess)).contains("link"));
    for field in ["what", "licence", "checks", "source", "piece"] {
        let mut o = oracle("a", "MIT", Run::OutOfProcess);
        match field {
            "what" => o.what = String::new(),
            "licence" => o.licence = String::new(),
            "checks" => o.checks = " ".to_owned(),
            "source" => o.source = String::new(),
            _ => o.piece = String::new(),
        }
        assert!(rule(o).contains(field), "{field}");
    }
    let a = oracle("a", "MIT", Run::OutOfProcess);
    assert!(matches!(
        parsed(vec![a.clone(), a]),
        Err(CatalogueError::Rule { what, .. }) if what.contains("one id")
    ));
    assert!(matches!(
        Catalogue::parse("{"),
        Err(CatalogueError::Json(_))
    ));
    let c = Catalogue {
        format: 9,
        about: "a".to_owned(),
        repository: "r".to_owned(),
        oracles: Vec::new(),
    };
    assert_eq!(
        Catalogue::parse(&serde_json::to_string(&c).unwrap()),
        Err(CatalogueError::Format(9))
    );
}

#[test]
fn a_link_is_found_in_the_lock_file() {
    let mut cgal = oracle("cgal", "GPL-3.0-only", Run::OutOfProcess);
    cgal.never_linked = vec!["cgal-sys".to_owned(), "other".to_owned()];
    let c = parsed(vec![cgal]).unwrap();
    let lock = "version = 4\n[[package]]\nname = \"cgal-sys\"\nversion = \"1.0.0\"\n";
    assert_eq!(linked(&c, lock).unwrap(), ["cgal-sys: cgal"]);
    assert_eq!(linked(&c, "version = 4\n").unwrap(), Vec::<String>::new());
    assert!(linked(&c, "[[").is_err());
}

#[cfg(not(target_family = "wasm"))]
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oracle-test-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(dir.join(".github/workflows")).unwrap();
    fs::create_dir_all(dir.join("geomdl")).unwrap();
    dir
}

// WebAssembly test runs have no file system.
#[cfg(not(target_family = "wasm"))]
#[test]
fn the_audit_finds_what_could_publish_or_float() {
    let mut live = oracle("geomdl", "MIT", Run::OutOfProcess);
    live.state = State::Live;
    live.pin = Some("1".to_owned());
    let c = parsed(vec![live, oracle("planned", "MIT", Run::OutOfProcess)]).unwrap();
    let dir = scratch("audit");
    fs::write(
        dir.join(".github/workflows/nightly.yml"),
        "# docker push is banned here\nrun: x\n",
    )
    .unwrap();
    assert!(audit(&c, &dir).unwrap()[0].contains("needs a Dockerfile"));
    fs::write(
        dir.join("geomdl/Dockerfile"),
        "FROM python@sha256:abc\nRUN x\n",
    )
    .unwrap();
    assert_eq!(audit(&c, &dir).unwrap(), Vec::<String>::new());
    fs::write(dir.join("geomdl/Dockerfile"), "FROM python:3.13\n").unwrap();
    assert!(audit(&c, &dir).unwrap()[0].contains("not pinned by digest"));
    fs::write(dir.join("geomdl/Dockerfile"), "FROM python@sha256:abc\n").unwrap();
    for bad in [
        "docker push x",
        "docker login",
        "uses: docker/login-action@v3",
        "push: true",
        "ghcr.io/x",
    ] {
        fs::write(
            dir.join(".github/workflows/nightly.yml"),
            format!("run: {bad}\n"),
        )
        .unwrap();
        assert!(
            audit(&c, &dir).unwrap()[0].contains("would publish"),
            "{bad}"
        );
    }
    fs::remove_dir_all(dir.join(".github")).unwrap();
    assert!(audit(&c, &dir).is_err());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn inputs_read_back_bit_for_bit() {
    for x in [
        0.0,
        -0.0,
        1.5,
        0.1,
        f64::MIN_POSITIVE,
        5e-324,
        f64::MAX,
        -1.0 / 3.0,
    ] {
        let back = read_float(&float(x).unwrap()).unwrap();
        assert_eq!(back.to_bits(), x.to_bits(), "{x}");
    }
    assert_eq!(float(f64::NAN), None);
    assert_eq!(float(f64::INFINITY), None);
    assert_eq!(read_float("nan"), None);
    assert_eq!(read_float("inf"), None);
    assert_eq!(read_float("x"), None);
    assert_eq!(steps(-6400), "-6400");
}

#[test]
fn a_change_of_unit_states_its_rounding() {
    let exact_mm = |n: i64, step: GridStep| {
        RBig::from_parts(
            IBig::from(n) * IBig::from(step.quarter_nm()),
            UBig::from(4_000_000_u32),
        )
    };
    assert_eq!(to_millimetres(6400, GridStep::Q0).value, 1.0);
    for (n, step) in [
        (1, GridStep::Q0),
        (7, GridStep::Q0),
        (-123_456_789, GridStep::Q1),
        (i64::MAX / 3, GridStep::Q2),
        (i64::MAX, GridStep::Q0),
    ] {
        let mm = to_millimetres(n, step);
        let got = RBig::try_from(mm.value).unwrap();
        let exact = exact_mm(n, step);
        let gap = if got > exact {
            got - exact
        } else {
            exact - got
        };
        assert!(gap <= RBig::try_from(mm.rounding).unwrap(), "{n}");
    }
    assert!(to_millimetres(0, GridStep::Q0).rounding < f64::MIN_POSITIVE);
}

#[test]
fn the_allowance_is_the_chapters() {
    let a = Allowance {
        ours: 1.0,
        theirs: 2.0,
        rounding: 0.5,
        fit: 64.0,
    };
    assert!(a.total() >= 3.5 && a.total() < 3.5 + 1e-12);
    // The oracle's tolerance cannot grow the allowance past our bound and the fit bound.
    let wide = Allowance { theirs: 1e9, ..a };
    assert!(wide.total() >= 65.0 && wide.total() < 65.0 + 1e-12);
    assert!(within(10.0, 13.0, &a));
    assert!(within(13.0, 10.0, &a));
    assert!(!within(10.0, 14.0, &a));
    assert!(!within(f64::NAN, 1.0, &a));
    assert!(!within(1.0, f64::INFINITY, &a));
    assert!(exact(&[1, 2], &[1, 2]));
    assert!(!exact(&1, &2));
}

fn signature(family: &str) -> Signature {
    Signature {
        operation: "nurbs.point".to_owned(),
        oracle: ID.to_owned(),
        kind: "value".to_owned(),
        family: family.to_owned(),
    }
}

fn record(family: &str, outcome: Triage, reference: Option<&str>) -> Record {
    Record {
        signature: signature(family),
        outcome,
        fixture: "oracle/0001-a".to_owned(),
        reference: reference.map(str::to_owned),
        why: "w".to_owned(),
    }
}

fn register(records: Vec<Record>) -> Result<Register, TriageError> {
    let r = Register {
        format: 1,
        about: "a".to_owned(),
        records,
    };
    Register::parse(&r.to_json().unwrap())
}

#[test]
fn each_signature_is_judged_once() {
    let a = signature("a");
    let b = signature("b");
    assert_eq!(a.to_string(), "nurbs.point/geomdl/value/a");
    let groups = group([&a, &b, &a]);
    assert_eq!((groups[&a], groups[&b]), (2, 1));
    let reg = register(vec![record("a", Triage::Theirs, Some("issue 1"))]).unwrap();
    assert!(reg.knows(&a));
    assert!(!reg.knows(&b));
    assert_eq!(reg.new_signatures([&a, &b, &b]), std::slice::from_ref(&b));
    assert_eq!(reg.new_signatures([&a]), Vec::<Signature>::new());
}

#[test]
fn the_register_keeps_its_rules() {
    assert!(register(vec![record("a", Triage::WithinBounds, None)]).is_ok());
    for outcome in [Triage::Ours, Triage::Theirs, Triage::Undecided] {
        assert!(matches!(
            register(vec![record("a", outcome, None)]),
            Err(TriageError::Rule(why)) if why.contains("names its")
        ));
        assert!(register(vec![record("a", outcome, Some("x"))]).is_ok());
    }
    let mut bad = record("a", Triage::WithinBounds, None);
    bad.fixture = "nope".to_owned();
    assert!(matches!(register(vec![bad]), Err(TriageError::Rule(w)) if w.contains("fixture")));
    let mut bad = record("a", Triage::WithinBounds, None);
    bad.why = " ".to_owned();
    assert!(matches!(register(vec![bad]), Err(TriageError::Rule(w)) if w.contains("reason")));
    let one = record("a", Triage::WithinBounds, None);
    assert!(matches!(
        register(vec![one.clone(), one]),
        Err(TriageError::Rule(w)) if w.contains("twice")
    ));
    assert!(matches!(Register::parse("{"), Err(TriageError::Json(_))));
    let text = register(Vec::new()).unwrap().to_json().unwrap();
    assert!(text.ends_with("}\n"));
    assert!(matches!(
        Register::parse(&text.replace("\"format\": 1", "\"format\": 2")),
        Err(TriageError::Rule(w)) if w.contains("format")
    ));
}

fn log() -> OpLog {
    OpLog::new(
        Vec::new(),
        vec![geom_format::operation::Operation {
            op: geom_op::OpPath::new(geom_op::OpId::from_bytes([1; 16])),
            name: "nurbs.point".to_owned(),
            version: 1,
            inputs: Vec::new(),
            params: vec![0x01],
            budget: Budget::new(1, 1),
        }],
    )
}

fn done() -> Replayed {
    Replayed::Done {
        hash: Hash([1; 32]),
        measures: Measures {
            volume6: None,
            area2: None,
            counts: Counts::default(),
            bounds: None,
            exactness: Class::Exact,
        },
    }
}

#[test]
fn every_outcome_ends_as_its_fixture() {
    let key = (
        "pre-edition".to_owned(),
        BTreeMap::from([("nurbs.point".to_owned(), 1)]),
    );
    for (outcome, reference, fails_on_base) in [
        (Triage::Ours, Some("#7"), true),
        (Triage::Theirs, Some("upstream #1"), false),
        (Triage::WithinBounds, None, false),
        (Triage::Undecided, Some("recompute at 400 bits"), false),
    ] {
        let r = record("a", outcome, reference);
        let f = fixture(&r, log(), key.clone(), &done()).unwrap();
        assert_eq!(f.id, "oracle/0001-a");
        assert_eq!(f.status, Status::Pass);
        assert!(matches!(
            &f.source,
            Source::Oracle { oracle, outcome: o, signature } if oracle == ID && *o == outcome
                && signature == "nurbs.point/geomdl/value/a"
        ));
        assert_eq!(f.fixed_by.as_deref(), reference.filter(|_| fails_on_base));
        assert_eq!(f.must_fail_on_base(), fails_on_base);
    }
    // A typed error is kept as what it is; a broken run is no fixture.
    let r = record("a", Triage::Theirs, Some("x"));
    let failed = Replayed::Failed {
        hash: Hash([2; 32]),
        kind: ErrorKind::Degenerate,
        entities: Vec::new(),
    };
    let f = fixture(&r, log(), key.clone(), &failed).unwrap();
    assert!(matches!(
        f.expect.values().next().unwrap(),
        Expect::Err { .. }
    ));
    assert!(fixture(&r, log(), key.clone(), &Replayed::Broken("b".to_owned())).is_err());
    // A key that is not the log's is a rule of the fixture.
    let wrong = ("pre-edition".to_owned(), BTreeMap::new());
    assert!(fixture(&r, log(), wrong, &done()).is_err());
}

/// The kernel's own answer to a job, the way a correct oracle would give it.
fn honest(job: &Job) -> Answer {
    let read = |t: &String| t.parse::<f64>().unwrap();
    let knots: Vec<f64> = job.knots.iter().map(read).collect();
    let basis = Basis::new(job.degree, knots).unwrap();
    let values: Vec<f64> = match job.op {
        Op::Basis => {
            let mut out = vec![0.0; job.degree + 1];
            basis
                .eval(read(&job.at), Limit::FromRight, &mut out)
                .unwrap();
            out
        }
        _ => curve_values(job, basis),
    };
    Answer {
        id: job.id,
        values: Some(values.iter().map(|x| float(*x).unwrap()).collect()),
    }
}

fn curve_values(job: &Job, basis: Basis) -> Vec<f64> {
    let read = |t: &String| t.parse::<f64>().unwrap();
    let weights: Option<Vec<f64>> = job.weights.as_ref().map(|w| w.iter().map(read).collect());
    if job.points[0].len() == 2 {
        values::<2>(job, basis, weights)
    } else {
        values::<3>(job, basis, weights)
    }
}

fn values<const D: usize>(job: &Job, basis: Basis, weights: Option<Vec<f64>>) -> Vec<f64> {
    let read = |t: &String| t.parse::<f64>().unwrap();
    let homogeneous: Vec<[f64; D]> = job
        .points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let w = weights.as_ref().map_or(1.0, |w| w[i]);
            let mut h = [0.0; D];
            for (slot, x) in h.iter_mut().zip(p) {
                *slot = read(x) * w;
            }
            h
        })
        .collect();
    let curve = Curve::from_homogeneous(basis, homogeneous, weights).unwrap();
    let at = read(&job.at);
    if job.op == Op::Point {
        return curve.point(at, Limit::FromRight).unwrap().to_vec();
    }
    let changed = curve
        .insert_knot(at, 1, &mut WorkMeter::new(Budget::UNLIMITED))
        .unwrap();
    let new = changed.value;
    let mut out = new.basis().knots().to_vec();
    out.extend(new.homogeneous().iter().flatten());
    out
}

#[test]
fn geomdl_jobs_are_drawn_from_their_seed() {
    let a = jobs(Seed(1), 200);
    assert_eq!(a, jobs(Seed(1), 200));
    assert_ne!(a, jobs(Seed(2), 200));
    assert_eq!(a.len(), 200);
    for op in [Op::Point, Op::Basis, Op::Insert] {
        assert!(a.iter().any(|j| j.op == op), "{op:?}");
    }
    assert!(a.iter().any(|j| j.weights.is_some()));
    assert!(a.iter().any(|j| j.weights.is_none()));
    assert!(a.iter().any(|j| j.points[0].len() == 3));
    assert_eq!(Op::Point.name(), "nurbs.point");
    assert_eq!(Op::Basis.name(), "nurbs.basis");
    assert_eq!(Op::Insert.name(), "nurbs.insert");
    // Every job is one the kernel takes, and it agrees with itself.
    for job in &a {
        assert_eq!(
            judge(job, &honest(job)).unwrap(),
            Compared::Agree,
            "{job:?}"
        );
    }
}

#[test]
fn a_wrong_answer_is_a_difference_and_truth_settles_it() {
    let all = jobs(Seed(3), 200);
    let job = all.iter().find(|j| j.op == Op::Point).unwrap();
    let mut wrong = honest(job);
    let values = wrong.values.as_mut().unwrap();
    let moved = values[0].parse::<f64>().unwrap() + 1.0;
    values[0] = float(moved).unwrap();
    let Compared::Differ { signature, settled } = judge(job, &wrong).unwrap() else {
        panic!()
    };
    assert_eq!(signature.operation, "nurbs.point");
    assert_eq!(signature.oracle, ID);
    assert_eq!(signature.kind, "value");
    assert_eq!(signature.family, job.family);
    assert!(settled.starts_with("theirs"), "{settled}");
    // A difference in the other kinds of job is raised too.
    for op in [Op::Basis, Op::Insert] {
        let job = all.iter().find(|j| j.op == op).unwrap();
        let mut wrong = honest(job);
        let values = wrong.values.as_mut().unwrap();
        let last = values.len() - 1;
        let moved = values[last].parse::<f64>().unwrap() + 1.0;
        values[last] = float(moved).unwrap();
        assert!(matches!(
            judge(job, &wrong).unwrap(),
            Compared::Differ { .. }
        ));
    }
}

#[test]
fn a_refusal_a_shape_and_a_broken_answer_are_caught() {
    let all = jobs(Seed(4), 40);
    let job = &all[0];
    let refused = Answer {
        id: job.id,
        values: None,
    };
    assert!(matches!(
        judge(job, &refused).unwrap(),
        Compared::Differ { signature, .. } if signature.kind == "refused"
    ));
    let mut short = honest(job);
    short.values.as_mut().unwrap().pop();
    assert!(matches!(
        judge(job, &short).unwrap(),
        Compared::Differ { signature, .. } if signature.kind == "shape"
    ));
    let mut bad = honest(job);
    bad.values.as_mut().unwrap()[0] = "x".to_owned();
    assert!(judge(job, &bad).is_err());
    let mut other = honest(job);
    other.id += 1;
    assert!(judge(job, &other).is_err());
    // A job the kernel refuses too agrees with a refusal.
    let mut broken = job.clone();
    broken.at = "9".to_owned();
    assert_eq!(
        judge(
            &broken,
            &Answer {
                id: broken.id,
                values: None
            }
        )
        .unwrap(),
        Compared::Agree
    );
    let mut bad_job = job.clone();
    bad_job.at = "x".to_owned();
    assert!(judge(&bad_job, &honest(job)).is_err());
}
