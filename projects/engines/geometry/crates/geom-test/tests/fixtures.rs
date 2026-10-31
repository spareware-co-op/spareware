//! Tests of the fixture format, the runner, the minimiser and the rules of what a pull request may
//! do (piece G97), through the public interface, with a small stand-in engine.
//!
//! The board's done-when: "Fixture, Expect and Status with goldens keyed by edition and operation
//! versions; Pass, Fail and Improved; only typed errors may be Known; operation-log minimiser; the
//! fails-on-merge-base check."
//!
//! What this file proves:
//!
//! - A fixture round-trips through its JSON and its log through the native format, in one spelling;
//!   every rule of `fixture.rs`'s header refuses what it must.
//! - The verdicts are 28.16's: exact matching, Pass, Fail and Improved, with a panic and a refused
//!   log failing under every status, and a run that differs between two runs caught.
//! - The hash covers every operation, and a key at another version replays at that version.
//! - The minimiser's result still fails, and removing any one thing from it stops the failure.
//! - `diff` sorts every change to a stored fixture, and `must_fail_on_base` holds 28.16's exceptions.
//! - Every fixture stored in the tree holds the rules and passes through the kernel.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cloned_ref_to_slice_refs,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "test cases; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeMap;

use geom_format::content::ContentId;
use geom_format::operation::Operation;
use geom_format::recipe::RecipeInput;
use geom_format::record::{REQUIRED, Record};
use geom_num::Steps;
use geom_op::{ErrorKind, Exactness, NameId, NameSet, OpError, OpId, OpPath, RunDetail};
use geom_run::Budget;
use geom_test::canonical::{Outcome, canonical_output};
use geom_test::fixture::diff::diff;
use geom_test::fixture::kernel::Kernel;
use geom_test::fixture::minimise::{MinimiseError, minimise, same_failure};
use geom_test::fixture::repo::{File, embedded, must_fail_on_base, parse_all};
use geom_test::fixture::run::{Engine, Failure, PREFIX, Ran, Replayed, Step, Verdict, replay, run};
use geom_test::fixture::{
    Class, Counts, Expect, Fixture, FixtureError, Hash, Key, Kind, Measures, Name, OpLog,
    OpVersions, Quantity, Source, Speed, Status, Triage, is_id,
};
use geom_test::golden::store::{ChangeKind, State};
use geom_test::golden::{Sha256, Store};

const EDITION: &str = "pre-edition";

fn op(name: &str, version: u64) -> Operation {
    Operation {
        op: OpPath::new(OpId::from_bytes([name.len() as u8; 16])),
        name: name.to_owned(),
        version,
        inputs: Vec::new(),
        params: vec![0x01],
        budget: Budget::new(1_000, 1 << 20),
    }
}

fn body(tag: u8) -> Record {
    Record {
        tag: 3,
        flags: REQUIRED,
        schema: 1,
        depends: Vec::new(),
        payload: vec![0x41, tag],
    }
}

fn measures() -> Measures {
    Measures {
        volume6: Some(Quantity::exact(6)),
        area2: Some(Quantity {
            lo: "11".to_owned(),
            hi: "13".to_owned(),
        }),
        counts: Counts {
            bodies: 1,
            shells: 1,
            faces: 6,
            edges: 12,
            vertices: 8,
        },
        bounds: Some([[0, 0, 0], [1, 1, 1]]),
        exactness: Class::Exact,
    }
}

/// A stand-in engine: `good` succeeds with a body hash of its version and the bodies so far,
/// `bad` fails with `Internal`-free typed error kinds, `boom` panics, `nope` is refused, and
/// `flaky` gives another body on every run.
#[derive(Default)]
struct Toy {
    done: u8,
    runs: u8,
    stored: usize,
    /// When set, `bad` succeeds: the kernel has been improved.
    fixed: bool,
}

impl Engine for Toy {
    fn start(&mut self, _heal: Steps, inputs: &[Record]) -> Result<(), String> {
        self.done = 0;
        self.runs += 1;
        self.stored = inputs.len();
        if inputs.iter().any(|r| r.payload == [0x41, 0xff]) {
            Err("a broken input".to_owned())
        } else {
            Ok(())
        }
    }

    fn run(&mut self, operation: &Operation) -> Ran {
        self.done += 1;
        let step = |b: u8| {
            Ran::Done(Step {
                bodies: vec![[b; 32]],
                names: NameSet::default(),
                history: None,
                exactness: Exactness::Exact,
                diagnostics: Vec::new(),
            })
        };
        match operation.name.as_str() {
            "good" => step(operation.version as u8),
            "bad" if self.fixed => step(9),
            "bad" => Ran::Failed(
                OpError::new(operation.op.clone(), RunDetail::Internal { what: "x" })
                    .with_entities([NameId::from_bytes([7; 16])]),
            ),
            "typed" if operation.inputs.len() > 1 && self.stored > 0 => Ran::Failed(OpError::new(
                operation.op.clone(),
                geom_num::NumError::Degenerate,
            )),
            "typed" => step(1),
            "boom" => panic!("a planted panic"),
            "nope" => Ran::Refused("no such operation".to_owned()),
            "flaky" => step(self.runs),
            other => Ran::Refused(format!("{other}?")),
        }
    }

    fn measures(&mut self) -> Measures {
        measures()
    }
}

fn versions(log: &OpLog) -> OpVersions {
    log.versions()
}

fn key(log: &OpLog) -> Key {
    (EDITION.to_owned(), versions(log))
}

/// A fixture over `operations`, expecting what the toy gives.
fn fixture(operations: Vec<Operation>) -> Fixture {
    let log = OpLog::new(vec![body(1)], operations);
    let got = replay(&mut Toy::default(), &log);
    let expect = match got {
        Replayed::Done { hash, measures } => Expect::Ok {
            hash: Some(hash),
            measures,
        },
        Replayed::Failed { kind, entities, .. } => Expect::Err {
            kind: Kind(kind),
            entities: entities.into_iter().map(Name).collect(),
        },
        Replayed::Broken(why) => panic!("{why}"),
    };
    Fixture {
        id: "bool/0001-a-case".to_owned(),
        source: Source::Issue { issue: 412 },
        fixed_by: None,
        speed: Speed::Fast,
        expect: BTreeMap::from([(key(&log), expect)]),
        log,
        status: Status::Pass,
        history: Vec::new(),
    }
}

fn known() -> Fixture {
    let mut f = fixture(vec![op("typed", 1)]);
    let k = key(&f.log);
    f.expect.insert(
        k,
        Expect::Err {
            kind: Kind(ErrorKind::Degenerate),
            entities: Vec::new(),
        },
    );
    f.status = Status::Known {
        wanted: Expect::Ok {
            hash: None,
            measures: measures(),
        },
    };
    f
}

fn rule(f: &Fixture) -> String {
    match f.check() {
        Err(FixtureError::Rule { what, .. }) => what,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_log_round_trips_in_one_encoding() {
    let log = OpLog::new(vec![body(1), body(2)], vec![op("good", 1), op("good", 2)]);
    let bytes = log.encode().unwrap();
    assert_eq!(bytes[..3], [0xd9, 0xd9, 0xf8], "the native label");
    assert_eq!(OpLog::decode(&bytes).unwrap(), log);
    let mut tampered = bytes.clone();
    tampered[20] ^= 1;
    assert!(OpLog::decode(&tampered).is_err());
    // An input after an operation is a log nothing writes.
    let mut late = log.clone();
    late.inputs.clear();
    let header = geom_format::header::Header::v1("x", log.heal)
        .record()
        .unwrap();
    let mut meter = geom_run::WorkMeter::new(Budget::UNLIMITED);
    let records = [
        geom_format::operation::operation_record(&op("good", 1), &mut meter).unwrap(),
        body(3),
    ];
    let file = geom_format::file::write(&header, &records, &mut meter).unwrap();
    assert!(OpLog::decode(&file).is_err());
    // Another program's header is another spelling.
    let records = [
        body(1),
        geom_format::operation::operation_record(&op("good", 1), &mut meter).unwrap(),
    ];
    let file = geom_format::file::write(&header, &records, &mut meter).unwrap();
    assert!(OpLog::decode(&file).is_err());
    assert_eq!(log.versions(), OpVersions::from([("good".to_owned(), 2)]));
    let at = log.at_versions(&OpVersions::from([("good".to_owned(), 5)]));
    assert!(at.operations.iter().all(|o| o.version == 5));
}

#[test]
fn a_fixture_round_trips_in_one_spelling() {
    let mut f = fixture(vec![op("good", 1)]);
    f.fixed_by = Some("#500".to_owned());
    let text = f.to_json().unwrap();
    assert!(text.ends_with("}\n"));
    assert_eq!(Fixture::parse(&text).unwrap(), f);
    assert_eq!(Fixture::parse(&text).unwrap().to_json().unwrap(), text);
    // Each kind of source survives.
    for source in [
        Source::Fuzz {
            target: "arenas".to_owned(),
            seed: "ab".to_owned(),
        },
        Source::Property {
            test: "t".to_owned(),
            seed: "cd".to_owned(),
        },
        Source::Checker {
            fault: "f".to_owned(),
        },
        Source::Oracle {
            oracle: "geomdl".to_owned(),
            signature: "s".to_owned(),
            outcome: Triage::Undecided,
        },
        Source::Catalogue {
            catalogue: "truck".to_owned(),
            entry: "#57".to_owned(),
        },
        Source::ClosedForm {
            derivation: "a cube".to_owned(),
        },
    ] {
        f.source = source;
        assert_eq!(Fixture::parse(&f.to_json().unwrap()).unwrap(), f);
    }
    let k = known();
    assert_eq!(Fixture::parse(&k.to_json().unwrap()).unwrap(), k);
    // An error's entities keep their names, in lowercase hexadecimal only.
    let e = fixture(vec![op("bad", 1)]);
    let text = e.to_json().unwrap();
    assert!(text.contains(&"07".repeat(16)));
    assert_eq!(Fixture::parse(&text).unwrap(), e);
    let shouted = text.replace(&"07".repeat(16), &"0A".repeat(16));
    assert!(Fixture::parse(&shouted).is_err());
}

#[test]
fn nothing_ambiguous_parses() {
    let f = fixture(vec![op("good", 1)]);
    let text = f.to_json().unwrap();
    let refused = |bad: String| assert!(Fixture::parse(&bad).is_err(), "{bad}");
    refused(text.replace("\"format\": 1", "\"format\": 2"));
    refused(text.replace("\"speed\"", "\"sped\""));
    refused(text.replace("\"fast\"", "\"quick\""));
    refused(text.replace("\"log\": \"d9", "\"log\": \"D9"));
    refused(text.replace("\"log\": \"d9", "\"log\": \"zz"));
    refused(text.replace(
        "\"edition\": \"pre-edition\"",
        "\"edition\": \"Pre Edition\"",
    ));
    refused(text.replace('\n', " "));
    refused(format!("{text}\n"));
    // A digest in capitals is not the one spelling; a short one is not a digest.
    let Expect::Ok {
        hash: Some(hash), ..
    } = f.expect.values().next().unwrap()
    else {
        panic!()
    };
    let digest = String::from(*hash);
    refused(text.replace(&digest, &digest.to_uppercase()));
    refused(text.replace(&digest, &digest[..62]));
    refused(text.replacen('[', "[[", 1));
    let twice = text.replace(
        "\"expect\": [\n",
        &format!(
            "\"expect\": [\n{},\n",
            serde_json::to_string(
                &serde_json::from_str::<serde_json::Value>(&text).unwrap()["expect"][0]
            )
            .unwrap()
        ),
    );
    refused(twice);
    assert!(!is_id("bool"));
    assert!(!is_id("bool/412-short"));
    assert!(!is_id("bool/0412"));
    assert!(!is_id("Bool/0412-x"));
    assert!(!is_id("bool/0412-"));
    assert!(!is_id("/0412-x"));
    assert!(!is_id("bool/04a2-x"));
    assert!(is_id("bool/0412-coplanar-wall-cut"));
}

#[test]
fn the_rules_hold() {
    let good = fixture(vec![op("good", 1)]);
    assert_eq!(good.check(), Ok(()));
    let mut f = good.clone();
    f.id = "nope".to_owned();
    assert!(rule(&f).contains("id"));
    let mut f = good.clone();
    f.expect.clear();
    assert!(rule(&f).contains("no expectation"));
    let mut f = good.clone();
    f.log.operations.clear();
    assert!(rule(&f).contains("runs nothing"));
    // A key that names other operations, and no key for the log's own versions.
    let mut f = good.clone();
    let e = f.expect.pop_first().unwrap().1;
    f.expect.insert(
        (
            EDITION.to_owned(),
            OpVersions::from([("other".to_owned(), 1)]),
        ),
        e.clone(),
    );
    assert!(rule(&f).contains("does not name"));
    f.expect.clear();
    f.expect.insert(
        (
            EDITION.to_owned(),
            OpVersions::from([("good".to_owned(), 2)]),
        ),
        e.clone(),
    );
    assert!(rule(&f).contains("log's versions"));
    // An expectation must say its hash; only `wanted` may leave it out.
    let mut f = good.clone();
    f.expect.insert(
        key(&f.log),
        Expect::Ok {
            hash: None,
            measures: measures(),
        },
    );
    assert!(rule(&f).contains("without a hash"));
    // Well-formed measures.
    for bad in [
        Quantity {
            lo: "2".to_owned(),
            hi: "1".to_owned(),
        },
        Quantity {
            lo: "01".to_owned(),
            hi: "2".to_owned(),
        },
        Quantity {
            lo: "x".to_owned(),
            hi: "2".to_owned(),
        },
        Quantity {
            lo: "1".to_owned(),
            hi: "+2".to_owned(),
        },
    ] {
        let mut m = measures();
        m.volume6 = Some(bad);
        let mut f = good.clone();
        f.expect.insert(
            key(&f.log),
            Expect::Ok {
                hash: Some(Hash([0; 32])),
                measures: m,
            },
        );
        assert!(rule(&f).contains("canonical decimal"));
    }
    let mut m = measures();
    m.bounds = Some([[1, 0, 0], [0, 1, 1]]);
    let mut f = good.clone();
    f.expect.insert(
        key(&f.log),
        Expect::Ok {
            hash: Some(Hash([0; 32])),
            measures: m,
        },
    );
    assert!(rule(&f).contains("inside out"));
    // What never replays is never expected.
    for kind in [ErrorKind::Cancelled, ErrorKind::OutOfMemory] {
        let mut f = good.clone();
        f.expect.insert(
            key(&f.log),
            Expect::Err {
                kind: Kind(kind),
                entities: Vec::new(),
            },
        );
        assert!(rule(&f).contains("never logged"));
    }
}

#[test]
fn only_a_typed_error_can_be_known() {
    assert_eq!(known().check(), Ok(()));
    let mut f = known();
    f.status = Status::Known {
        wanted: Expect::Err {
            kind: Kind(ErrorKind::Degenerate),
            entities: Vec::new(),
        },
    };
    assert!(rule(&f).contains("must be a success"));
    let mut f = known();
    f.expect.insert(
        key(&f.log),
        Expect::Ok {
            hash: Some(Hash([1; 32])),
            measures: measures(),
        },
    );
    assert!(rule(&f).contains("typed error"));
    let mut f = known();
    f.expect.insert(
        key(&f.log),
        Expect::Err {
            kind: Kind(ErrorKind::Internal),
            entities: Vec::new(),
        },
    );
    assert!(rule(&f).contains("Internal"));
    let mut f = known();
    f.fixed_by = Some("#1".to_owned());
    assert!(rule(&f).contains("no fix"));
    // `wanted` keeps the same rules.
    let mut f = known();
    let mut m = measures();
    m.bounds = Some([[1, 0, 0], [0, 1, 1]]);
    f.status = Status::Known {
        wanted: Expect::Ok {
            hash: None,
            measures: m,
        },
    };
    assert!(rule(&f).contains("inside out"));
}

#[test]
fn the_hash_covers_every_operation_in_order() {
    let one = OpLog::new(Vec::new(), vec![op("good", 1)]);
    let two = OpLog::new(Vec::new(), vec![op("good", 1), op("good", 2)]);
    let swapped = OpLog::new(Vec::new(), vec![op("good", 2), op("good", 1)]);
    let hash = |log: &OpLog| match replay(&mut Toy::default(), log) {
        Replayed::Done { hash, .. } => hash,
        other => panic!("{other:?}"),
    };
    assert_ne!(hash(&one), hash(&two));
    assert_ne!(hash(&two), hash(&swapped));
    // The stated construction: the prefix, then each operation's canonical output.
    let outputs: Vec<[u8; 32]> = [1_u8, 2]
        .into_iter()
        .map(|b| {
            canonical_output(&Outcome::Output {
                bodies: &[[b; 32]],
                names: &NameSet::default(),
                history: None,
                exactness: &Exactness::Exact,
                diagnostics: &[],
            })
            .unwrap()
        })
        .collect();
    let mut sha = Sha256::new();
    sha.write(PREFIX);
    sha.write(&outputs[0]);
    sha.write(&outputs[1]);
    assert_eq!(hash(&two), Hash(sha.finish()));
    assert_eq!(PREFIX, b"SKG log output v1\0");
    // A failing operation ends the log, and its output counts.
    let mut toy = Toy::default();
    let stopped = OpLog::new(Vec::new(), vec![op("good", 1), op("bad", 1), op("boom", 1)]);
    let Replayed::Failed {
        kind,
        entities,
        hash,
    } = replay(&mut toy, &stopped)
    else {
        panic!()
    };
    assert_eq!(toy.done, 2, "nothing runs after the error");
    assert_eq!(kind, ErrorKind::Internal);
    assert_eq!(entities, [NameId::from_bytes([7; 16])]);
    let error = OpError::new(
        stopped.operations[1].op.clone(),
        RunDetail::Internal { what: "y" },
    )
    .with_entities([NameId::from_bytes([7; 16])]);
    let mut sha = Sha256::new();
    sha.write(PREFIX);
    sha.write(&outputs[0]);
    sha.write(&canonical_output(&Outcome::Error(&error)).unwrap());
    assert_eq!(hash, Hash(sha.finish()));
}

// WebAssembly aborts on a panic, so the planted one runs natively only.
#[cfg(not(target_family = "wasm"))]
#[test]
fn a_panic_and_a_refusal_are_broken_under_every_status() {
    for name in ["boom", "nope"] {
        let log = OpLog::new(Vec::new(), vec![op(name, 1)]);
        let Replayed::Broken(why) = replay(&mut Toy::default(), &log) else {
            panic!()
        };
        assert!(why.contains(name), "{why}");
        let mut f = known();
        f.log = log.clone();
        let want = f.expect.pop_first().unwrap().1;
        f.expect.insert(key(&log), want);
        assert!(matches!(
            run(&f, EDITION, &mut Toy::default()),
            Verdict::Fail(Failure::Mismatch { .. })
        ));
    }
    // An input the engine cannot take.
    let log = OpLog::new(vec![body(0xff)], vec![op("good", 1)]);
    assert!(matches!(
        replay(&mut Toy::default(), &log),
        Replayed::Broken(why) if why.contains("a broken input")
    ));
}

#[test]
fn the_verdicts_are_the_chapters() {
    // Pass: the same result.
    let f = fixture(vec![op("good", 1)]);
    assert_eq!(run(&f, EDITION, &mut Toy::default()), Verdict::Pass);
    // An expected typed error passes too.
    let f = fixture(vec![op("bad", 1)]);
    assert_eq!(run(&f, EDITION, &mut Toy::default()), Verdict::Pass);
    // Another hash, other measures, another kind, another entity, another order: Fail.
    let f = fixture(vec![op("good", 1)]);
    let mut other = f.clone();
    let k = key(&other.log);
    let Some(Expect::Ok { measures: m, .. }) = other.expect.get(&k).cloned() else {
        panic!()
    };
    other.expect.insert(
        k.clone(),
        Expect::Ok {
            hash: Some(Hash([0; 32])),
            measures: m.clone(),
        },
    );
    assert!(matches!(
        run(&other, EDITION, &mut Toy::default()),
        Verdict::Fail(Failure::Mismatch { .. })
    ));
    let mut moved = m.clone();
    moved.counts.faces = 7;
    let Some(Expect::Ok { hash, .. }) = f.expect.get(&k).cloned() else {
        panic!()
    };
    other.expect.insert(
        k.clone(),
        Expect::Ok {
            hash,
            measures: moved,
        },
    );
    assert!(matches!(
        run(&other, EDITION, &mut Toy::default()),
        Verdict::Fail(Failure::Mismatch { .. })
    ));
    let f = fixture(vec![op("bad", 1)]);
    let k = key(&f.log);
    let entity = Name(NameId::from_bytes([7; 16]));
    for wrong in [
        Expect::Err {
            kind: Kind(ErrorKind::Degenerate),
            entities: vec![entity],
        },
        Expect::Err {
            kind: Kind(ErrorKind::Internal),
            entities: Vec::new(),
        },
        Expect::Err {
            kind: Kind(ErrorKind::Internal),
            entities: vec![entity, entity],
        },
        Expect::Err {
            kind: Kind(ErrorKind::Internal),
            entities: vec![Name(NameId::from_bytes([8; 16]))],
        },
        Expect::Ok {
            hash: None,
            measures: measures(),
        },
    ] {
        let mut other = f.clone();
        other.expect.insert(k.clone(), wrong);
        assert!(matches!(
            run(&other, EDITION, &mut Toy::default()),
            Verdict::Fail(Failure::Mismatch { .. })
        ));
    }
    // A key another edition holds is not this edition's.
    let f = fixture(vec![op("good", 1)]);
    assert_eq!(
        run(&f, "edition-2", &mut Toy::default()),
        Verdict::Fail(Failure::NoGolden {
            edition: "edition-2".to_owned()
        })
    );
    // Two runs that differ are caught before the store is looked at, each from a fresh start.
    let mut f = fixture(vec![op("good", 1)]);
    f.log.operations[0].name = "flaky".to_owned();
    f.expect = BTreeMap::from([(key(&f.log), f.expect.pop_first().unwrap().1)]);
    let mut toy = Toy::default();
    assert!(matches!(
        run(&f, EDITION, &mut toy),
        Verdict::Fail(Failure::Repeat { .. })
    ));
    assert_eq!(toy.runs, 2);
    // Every key of the edition runs, at its own versions.
    let f = fixture(vec![op("good", 1)]);
    let mut two = f.clone();
    let new_key = OpVersions::from([("good".to_owned(), 2)]);
    let log_at = f.log.at_versions(&new_key);
    let Replayed::Done { hash, measures: m } = replay(&mut Toy::default(), &log_at) else {
        panic!()
    };
    two.expect.insert(
        (EDITION.to_owned(), new_key.clone()),
        Expect::Ok {
            hash: Some(hash),
            measures: m.clone(),
        },
    );
    assert_eq!(run(&two, EDITION, &mut Toy::default()), Verdict::Pass);
    two.expect.insert(
        (EDITION.to_owned(), new_key),
        Expect::Ok {
            hash: Some(Hash([0; 32])),
            measures: m,
        },
    );
    assert!(matches!(
        run(&two, EDITION, &mut Toy::default()),
        Verdict::Fail(_)
    ));
}

#[test]
fn a_known_fixture_passes_improves_or_fails() {
    // Its typed error today: Pass.
    let mut f = known();
    f.log.operations[0].inputs = vec![
        RecipeInput::Entity(NameId::from_bytes([1; 16])),
        RecipeInput::Entity(NameId::from_bytes([2; 16])),
    ];
    assert_eq!(run(&f, EDITION, &mut Toy::default()), Verdict::Pass);
    // Its wanted measures: Improved, with no hash to compare.
    let mut g = fixture(vec![op("bad", 1)]);
    let k = key(&g.log);
    let entity = Name(NameId::from_bytes([7; 16]));
    g.expect.insert(
        k,
        Expect::Err {
            kind: Kind(ErrorKind::Degenerate),
            entities: vec![entity],
        },
    );
    g.status = Status::Known {
        wanted: Expect::Ok {
            hash: None,
            measures: measures(),
        },
    };
    let mut toy = Toy {
        fixed: true,
        ..Toy::default()
    };
    assert_eq!(run(&g, EDITION, &mut toy), Verdict::Improved);
    // A stated hash must match too.
    g.status = Status::Known {
        wanted: Expect::Ok {
            hash: Some(Hash([0; 32])),
            measures: measures(),
        },
    };
    assert!(matches!(run(&g, EDITION, &mut toy), Verdict::Fail(_)));
    // A success that is not the one wanted is wrong, and can never be Known.
    let mut m = measures();
    m.counts.faces = 99;
    g.status = Status::Known {
        wanted: Expect::Ok {
            hash: None,
            measures: m,
        },
    };
    assert!(matches!(run(&g, EDITION, &mut toy), Verdict::Fail(_)));
    // Another error than the known one fails.
    let mut h = known();
    h.log.operations[0].name = "bad".to_owned();
    h.expect = BTreeMap::from([(key(&h.log), h.expect.pop_first().unwrap().1)]);
    assert!(matches!(
        run(&h, EDITION, &mut Toy::default()),
        Verdict::Fail(_)
    ));
}

#[test]
fn the_minimiser_keeps_the_failure_and_nothing_else() {
    let mut typed = op("typed", 1);
    typed.inputs = vec![
        RecipeInput::Entity(NameId::from_bytes([1; 16])),
        RecipeInput::Body(ContentId::of_payload(b"x")),
        RecipeInput::Entity(NameId::from_bytes([3; 16])),
    ];
    let log = OpLog::new(
        vec![body(1), body(2), body(3)],
        vec![op("good", 1), op("good", 2), typed, op("good", 3)],
    );
    let fails = |l: &OpLog| {
        matches!(
            replay(&mut Toy::default(), l),
            Replayed::Failed {
                kind: ErrorKind::Degenerate,
                ..
            }
        )
    };
    assert!(fails(&log));
    let mut calls = 0;
    let small = minimise(&log, |l| {
        calls += 1;
        fails(l)
    })
    .unwrap();
    assert!(calls > 1);
    assert!(fails(&small));
    assert_eq!(small.operations.len(), 1, "one operation: {small:?}");
    assert_eq!(small.operations[0].inputs.len(), 2);
    assert_eq!(small.inputs.len(), 1);
    // The same result every time.
    assert_eq!(minimise(&log, fails).unwrap(), small);
    // 1-minimal: every single removal stops the failure.
    assert_eq!(minimise(&small, fails).unwrap(), small);
    let mut less = small.clone();
    less.operations[0].inputs.remove(0);
    assert!(!fails(&less));
    let mut less = small.clone();
    less.inputs.clear();
    assert!(!fails(&less));
    // A log that does not fail has nothing to minimise.
    assert_eq!(
        minimise(&OpLog::new(Vec::new(), vec![op("good", 1)]), fails),
        Err(MinimiseError::NotFailing)
    );
}

#[test]
fn a_fixture_minimises_to_the_same_class_of_failure() {
    let good = fixture(vec![op("good", 1), op("bad", 1), op("good", 2)]);
    let mut failing = good.clone();
    let k = key(&failing.log);
    failing.expect.insert(
        k,
        Expect::Ok {
            hash: Some(Hash([0; 32])),
            measures: measures(),
        },
    );
    let mut toy = Toy::default();
    let Verdict::Fail(failure) = run(&failing, EDITION, &mut toy) else {
        panic!()
    };
    assert_eq!(failure.class(), "error Internal");
    // The keys name each log's operations by the fixture's versions, so the trial keeps them.
    let small = minimise(&failing.log, |l| {
        same_failure(&failing, EDITION, &mut toy, &failure, l)
    })
    .unwrap();
    assert_eq!(small.operations.len(), 1);
    assert_eq!(small.operations[0].name, "bad");
}

#[test]
fn failures_say_what_failed() {
    let f = Failure::NoGolden {
        edition: "e".to_owned(),
    };
    assert_eq!(f.class(), "no golden");
    assert!(f.to_string().contains("edition e"));
    let r = Failure::Repeat {
        operations: OpVersions::new(),
        first: Box::new(Replayed::Broken("a".to_owned())),
        other: Box::new(Replayed::Broken("b".to_owned())),
    };
    assert_eq!(r.class(), "not repeatable");
    assert!(r.to_string().contains("differ"));
    for (got, class) in [
        (Replayed::Broken("a".to_owned()), "broken"),
        (
            Replayed::Done {
                hash: Hash([0; 32]),
                measures: measures(),
            },
            "another result",
        ),
    ] {
        let m = Failure::Mismatch {
            operations: OpVersions::new(),
            got: Box::new(got),
            want: Box::new(Expect::Err {
                kind: Kind(ErrorKind::Degenerate),
                entities: Vec::new(),
            }),
        };
        assert_eq!(m.class(), class);
        assert!(m.to_string().contains("expected"));
    }
}

fn state(edition: &str) -> Option<State> {
    match edition {
        EDITION => Some(State::Open),
        "first" => Some(State::Frozen),
        _ => None,
    }
}

#[test]
fn a_pull_request_may_only_add_or_improve() {
    let base = fixture(vec![op("good", 1)]);
    let same = diff(&[base.clone()], &[base.clone()], state);
    assert!(same.is_clean());
    assert!(same.free.is_empty() && same.added.is_empty());
    // A new fixture is free.
    let mut new = fixture(vec![op("good", 2)]);
    new.id = "bool/0002-b".to_owned();
    let d = diff(&[base.clone()], &[base.clone(), new.clone()], state);
    assert!(d.is_clean());
    assert_eq!(d.added, ["bool/0002-b"]);
    // Removing one never stands.
    assert!(diff(&[base.clone()], &[], state).refused[0].contains("never deleted"));
    // The log and the source are the case itself.
    let mut f = base.clone();
    f.log.operations.push(op("good", 2));
    assert!(diff(&[base.clone()], &[f], state).refused[0].contains("log"));
    let mut f = base.clone();
    f.source = Source::Issue { issue: 1 };
    assert!(diff(&[base.clone()], &[f], state).refused[0].contains("source"));
    // A fix may be recorded once, and not changed.
    let mut f = base.clone();
    f.fixed_by = Some("#1".to_owned());
    let d = diff(&[base.clone()], &[f.clone()], state);
    assert!(d.is_clean());
    assert!(d.free[0].contains("#1"));
    let mut g = f.clone();
    g.fixed_by = Some("#2".to_owned());
    assert!(diff(&[f.clone()], &[g], state).refused[0].contains("fix"));
    let mut g = f.clone();
    g.fixed_by = None;
    assert!(diff(&[f], &[g], state).refused[0].contains("fix"));
    // Moving tier is free.
    let mut f = base.clone();
    f.speed = Speed::Slow;
    let d = diff(&[base.clone()], &[f], state);
    assert!(d.is_clean() && d.free.len() == 1);
}

#[test]
fn an_expectation_moves_only_on_the_record() {
    let base = fixture(vec![op("good", 1)]);
    let k = key(&base.log);
    let was = base.expect[&k].clone();
    let now = Expect::Ok {
        hash: Some(Hash([9; 32])),
        measures: measures(),
    };
    let mut f = base.clone();
    f.expect.insert(k.clone(), now.clone());
    // Without a record it is refused; with one it needs an owner.
    assert!(diff(&[base.clone()], &[f.clone()], state).refused[0].contains("without a record"));
    f.history.push(geom_test::fixture::Change {
        edition: EDITION.to_owned(),
        operations: k.1.clone(),
        was: was.clone(),
        kind: ChangeKind::Bug,
        piece: "G1".to_owned(),
        why: "w".to_owned(),
    });
    let d = diff(&[base.clone()], &[f.clone()], state);
    assert!(d.refused.is_empty(), "{d:?}");
    assert_eq!(d.approval.len(), 1);
    assert!(!d.is_clean());
    // A record of something else is no record.
    let mut wrong = f.clone();
    wrong.history[0].was = now.clone();
    assert!(diff(&[base.clone()], &[wrong], state).refused[0].contains("without a record"));
    // A record with no change is refused, and history is only added to.
    let mut idle = base.clone();
    idle.history = f.history.clone();
    assert!(diff(&[base.clone()], &[idle], state).refused[0].contains("new history records"));
    let mut rewritten = f.clone();
    rewritten.history.clear();
    let mut later = f.clone();
    later.history[0].why = "other".to_owned();
    assert!(
        diff(&[f.clone()], &[later], state).refused[0].contains("rewritten"),
        "a changed record"
    );
    assert!(diff(&[f.clone()], &[rewritten], state).refused[0].contains("rewritten"));
    // An expectation cannot go; a new one is free.
    let mut gone = base.clone();
    gone.expect.clear();
    assert!(diff(&[base.clone()], &[gone], state).refused[0].contains("removed"));
    let mut more = base.clone();
    more.expect
        .insert(("later".to_owned(), k.1.clone()), now.clone());
    let d = diff(&[base.clone()], &[more], state);
    assert!(d.is_clean() && d.free[0].contains("new expectation"));
    // Frozen and unknown editions never move, on the record or not.
    for edition in ["first", "unknown"] {
        let mut frozen = base.clone();
        let hold = frozen.expect.pop_first().unwrap().1;
        frozen
            .expect
            .insert((edition.to_owned(), k.1.clone()), hold);
        let mut moved = frozen.clone();
        moved
            .expect
            .insert((edition.to_owned(), k.1.clone()), now.clone());
        moved.history.push(geom_test::fixture::Change {
            edition: edition.to_owned(),
            operations: k.1.clone(),
            was: was.clone(),
            kind: ChangeKind::Improvement,
            piece: "G1".to_owned(),
            why: "w".to_owned(),
        });
        let d = diff(&[frozen], &[moved], state);
        assert!(d.refused[0].contains("frozen"), "{d:?}");
    }
}

#[test]
fn status_moves_one_way() {
    let k = known();
    let key = key(&k.log);
    let stored = Expect::Ok {
        hash: Some(Hash([3; 32])),
        measures: measures(),
    };
    // Known to Pass with what it wanted: free, no history record.
    let mut p = k.clone();
    p.status = Status::Pass;
    p.expect.insert(key.clone(), stored.clone());
    let d = diff(&[k.clone()], &[p.clone()], state);
    assert!(d.is_clean(), "{d:?}");
    assert!(d.free[0].contains("Known moved to Pass"));
    // Known to Pass with something else is an ordinary change, and is refused unrecorded.
    let mut m = measures();
    m.counts.faces = 99;
    let mut q = k.clone();
    q.status = Status::Pass;
    q.expect.insert(
        key,
        Expect::Ok {
            hash: Some(Hash([3; 32])),
            measures: m,
        },
    );
    assert!(diff(&[k.clone()], &[q], state).refused[0].contains("without a record"));
    // Pass never becomes Known.
    let mut r = p.clone();
    r.status = k.status.clone();
    assert!(diff(&[p], &[r], state).refused[0].contains("regression"));
    // A new wanted result needs an owner.
    let mut w = k.clone();
    w.status = Status::Known {
        wanted: Expect::Ok {
            hash: Some(Hash([4; 32])),
            measures: measures(),
        },
    };
    let d = diff(&[k], &[w], state);
    assert!(d.refused.is_empty() && d.approval[0].contains("wanted"));
}

#[test]
fn the_fails_on_base_list_holds_the_chapters_exceptions() {
    let plain = fixture(vec![op("good", 1)]);
    assert!(!plain.must_fail_on_base(), "no fix recorded");
    let mut fix = plain.clone();
    fix.fixed_by = Some("#9".to_owned());
    assert!(fix.must_fail_on_base());
    for (outcome, must) in [
        (Triage::Ours, true),
        (Triage::Undecided, true),
        (Triage::Theirs, false),
        (Triage::WithinBounds, false),
    ] {
        let mut f = fix.clone();
        f.source = Source::Oracle {
            oracle: "o".to_owned(),
            signature: "s".to_owned(),
            outcome,
        };
        assert_eq!(f.must_fail_on_base(), must, "{outcome:?}");
    }
    let mut k = known();
    k.fixed_by = Some("#9".to_owned());
    assert!(!k.must_fail_on_base(), "Known is exempt");
    // Only what the pull request adds.
    let mut other = fix.clone();
    other.id = "bool/0002-b".to_owned();
    assert_eq!(
        must_fail_on_base(&[fix.clone()], &[fix, other]),
        ["bool/0002-b"]
    );
}

#[test]
fn a_set_of_files_is_checked_together() {
    let f = fixture(vec![op("good", 1)]);
    let text = f.to_json().unwrap();
    let file = |path: &str| File {
        path: path.to_owned(),
        text: text.clone(),
    };
    assert_eq!(
        parse_all(&[file("bool/0001-a-case.json")]).unwrap(),
        [f.clone()]
    );
    let problems = parse_all(&[file("bool/0002-b.json")]).unwrap_err();
    assert!(problems[0].contains("belongs at"));
    let problems =
        parse_all(&[file("bool/0001-a-case.json"), file("bool/0001-a-case.json")]).unwrap_err();
    assert!(problems[0].contains("repeats"));
    let problems = parse_all(&[
        File {
            path: "a.json".to_owned(),
            text: "{".to_owned(),
        },
        File {
            path: "b.json".to_owned(),
            text: "{".to_owned(),
        },
    ])
    .unwrap_err();
    assert_eq!(problems.len(), 2, "every problem is listed");
}

#[test]
fn the_kernel_refuses_what_it_cannot_run() {
    let log = OpLog::new(Vec::new(), vec![op("no.such.op", 1)]);
    assert!(matches!(
        replay(&mut Kernel::new(), &log),
        Replayed::Broken(why) if why.contains("no.such.op")
    ));
    let log = OpLog::new(vec![body(1)], vec![op("no.such.op", 1)]);
    assert!(matches!(
        replay(&mut Kernel::new(), &log),
        Replayed::Broken(_)
    ));
}

#[test]
fn every_stored_fixture_holds_and_passes() {
    let store = Store::embedded().unwrap();
    let edition = store.current().unwrap().edition.clone();
    let fixtures = parse_all(&embedded()).unwrap();
    for fixture in &fixtures {
        assert_eq!(
            run(fixture, &edition, &mut Kernel::new()),
            Verdict::Pass,
            "{}",
            fixture.id
        );
    }
}
