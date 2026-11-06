//! Tests of what the oracle runner took from solidcore's difftest (piece S2): the waiver file, the
//! evidence a "theirs" carries, the register's hold on every later job, the night's report and its
//! ratchet, and the sensitivity check.
//!
//! What this file proves:
//!
//! - The waiver file reads strictly: an unknown field, a misspelt key, a blank operation or family, a
//!   missing or blank reason, an unknown check, another format and a waiver given twice are each
//!   refused, so nothing a typing mistake widens a waiver; and the only check there is to waive is
//!   lost coverage, so no waiver can name a difference.
//! - `apply` says what was waived, what was not and which waivers cover nothing.
//! - A "theirs" is recorded only with an `https` report, a pin and a reproducing input, and only on
//!   the exact reference's "theirs" verdict; "within bounds" only on "both"; no outcome rests on a
//!   verdict that clears the kernel; and the pin must be the catalogue's.
//! - `contradicts` never lets a record clear a job the exact reference finds ours, and holds a
//!   "theirs" or "within bounds" record to the verdict it was written on.
//! - A report counts a night's cells, round-trips and refuses counts that do not add up; the ratchet
//!   names a lost cell and nothing else.
//! - The perturbation moves an answer past the allowance, and every moved honest answer is caught.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::assert_is_empty,
    clippy::type_complexity,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    clippy::float_cmp,
    clippy::too_many_lines,
    reason = "test cases; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use geom_test::fixture::Triage;
use geom_test::oracle::catalogue::Catalogue;
use geom_test::oracle::compare::{Allowance, PERTURBATION, perturbed, within};
use geom_test::oracle::exact::float;
use geom_test::oracle::geomdl::{
    Answer, Compared, ID, Op, bound, jobs, judge, perturb, sensitivity,
};
use geom_test::oracle::triage::{
    Evidence, Record, Register, Settled, Signature, TriageError, is_report_address,
};
use geom_test::oracle::waiver::{
    Applied, Check, Observed, Report, ReportError, Violation, Waiver, WaiverError, Waivers, apply,
    ratchet,
};
use geom_test::seed::Seed;

mod common;
use common::{flawed, honest};

fn waivers(text: &str) -> Result<Waivers, WaiverError> {
    Waivers::parse(&format!("format = 1\n{text}"))
}

fn rule(text: &str) -> String {
    match waivers(text) {
        Err(e) => e.to_string(),
        Ok(w) => panic!("read: {w:?}"),
    }
}

const LOST: &str = "[[waiver]]\noracle = \"geomdl\"\ncheck = \"coverage\"\n";

#[test]
fn the_waiver_file_reads_strictly() {
    // The shipped file is valid and empty, and so is a file of its format alone.
    assert_eq!(Waivers::embedded().unwrap().waiver, Vec::<Waiver>::new());
    assert!(waivers("").unwrap().waiver.is_empty());
    let all = waivers(&format!("{LOST}reason = \"r\"\n")).unwrap();
    assert_eq!(all.waiver.len(), 1);
    assert_eq!(all.waiver[0].operation, None);
    let one = waivers(&format!(
        "{LOST}operation = \"nurbs.insert\"\nfamily = \"degree-3-2d\"\nreason = \"r\"\n"
    ))
    .unwrap();
    assert_eq!(one.waiver[0].operation.as_deref(), Some("nurbs.insert"));
    assert_eq!(one.waiver[0].family.as_deref(), Some("degree-3-2d"));
    // A reason is required, and not blank.
    assert!(rule(LOST).contains("reason"), "{}", rule(LOST));
    assert!(rule(&format!("{LOST}reason = \"  \"\n")).contains("no reason"));
    // A misspelt key never reads as an absent one, which would cover every family.
    for key in [
        "familly",
        "families",
        "signature",
        "kind",
        "job",
        "case",
        "unit",
    ] {
        let text = format!("{LOST}{key} = \"x\"\nreason = \"r\"\n");
        assert!(matches!(waivers(&text), Err(WaiverError::Toml(_))), "{key}");
    }
    // A blank scope never reads as "every".
    for what in ["operation", "family"] {
        let text = format!("{LOST}{what} = \" \"\nreason = \"r\"\n");
        assert!(rule(&text).contains(what), "{what}");
    }
    // A scope of the wrong type, an unknown check, no oracle, another format.
    assert!(matches!(
        waivers(&format!("{LOST}family = 7\nreason = \"r\"\n")),
        Err(WaiverError::Toml(_))
    ));
    for check in ["ratchet", "value-share", "difference", "Coverage"] {
        let text =
            format!("[[waiver]]\noracle = \"geomdl\"\ncheck = \"{check}\"\nreason = \"r\"\n");
        assert!(
            matches!(waivers(&text), Err(WaiverError::Toml(_))),
            "{check}"
        );
    }
    assert!(rule("[[waiver]]\ncheck = \"coverage\"\nreason = \"r\"\n").contains("oracle"));
    assert!(
        rule("[[waiver]]\noracle = \" \"\ncheck = \"coverage\"\nreason = \"r\"\n")
            .contains("oracle")
    );
    assert!(matches!(
        Waivers::parse("format = 2\n"),
        Err(WaiverError::Rule(w)) if w.contains("format")
    ));
    assert!(matches!(Waivers::parse(""), Err(WaiverError::Toml(_))));
    assert!(matches!(
        Waivers::parse("not toml ["),
        Err(WaiverError::Toml(_))
    ));
    // A waiver given twice is an error; a second scope is not.
    let twice = format!("{LOST}reason = \"a\"\n{LOST}reason = \"b\"\n");
    assert!(rule(&twice).contains("twice"));
    let scoped = format!("{LOST}reason = \"a\"\n{LOST}family = \"f\"\nreason = \"b\"\n");
    assert_eq!(waivers(&scoped).unwrap().waiver.len(), 2);
    // An oracle the catalogue does not hold is named.
    let ghost =
        waivers("[[waiver]]\noracle = \"ghost\"\ncheck = \"coverage\"\nreason = \"r\"\n").unwrap();
    let catalogue = Catalogue::embedded().unwrap();
    assert_eq!(ghost.check_catalogue(&catalogue).len(), 1);
    assert!(all.check_catalogue(&catalogue).is_empty());
}

#[test]
fn a_waiver_has_one_check_and_it_names_no_difference() {
    // `coverage` is the only check there is, and its name is the one the file spells.
    assert_eq!(Check::Coverage.to_string(), "coverage");
    let text = serde_json::to_string(&Check::Coverage).unwrap();
    assert_eq!(text, "\"coverage\"");
    // The fields of a waiver are the oracle, the check, a cell's two names and the reason: nothing
    // that names a signature, a kind of difference or a job can be written.
    let fields: Vec<String> = serde_json::to_value(Waiver {
        oracle: "o".to_owned(),
        check: Check::Coverage,
        operation: Some("op".to_owned()),
        family: Some("f".to_owned()),
        reason: "r".to_owned(),
    })
    .unwrap()
    .as_object()
    .unwrap()
    .keys()
    .cloned()
    .collect();
    assert_eq!(fields, ["check", "family", "operation", "oracle", "reason"]);
}

fn lost(operation: &str, family: &str) -> Violation {
    Violation {
        oracle: ID.to_owned(),
        check: Check::Coverage,
        operation: operation.to_owned(),
        family: family.to_owned(),
        detail: "d".to_owned(),
    }
}

fn waiver(operation: Option<&str>, family: Option<&str>, reason: &str) -> Waiver {
    Waiver {
        oracle: ID.to_owned(),
        check: Check::Coverage,
        operation: operation.map(str::to_owned),
        family: family.map(str::to_owned),
        reason: reason.to_owned(),
    }
}

#[test]
fn a_waiver_covers_its_scope_and_the_rest_is_reported() {
    let every = waiver(None, None, "all");
    let op = waiver(Some("nurbs.point"), None, "one operation");
    let cell = waiver(Some("nurbs.point"), Some("a"), "one cell");
    for (w, v, covers) in [
        (&every, lost("x", "y"), true),
        (&op, lost("nurbs.point", "y"), true),
        (&op, lost("nurbs.basis", "y"), false),
        (&cell, lost("nurbs.point", "a"), true),
        (&cell, lost("nurbs.point", "b"), false),
        (&cell, lost("nurbs.basis", "a"), false),
    ] {
        assert_eq!(w.covers(&v), covers, "{w:?} {v:?}");
    }
    let mut other = lost("nurbs.point", "a");
    other.oracle = "cgal".to_owned();
    assert!(!every.covers(&other));

    let set = Waivers {
        format: 1,
        waiver: vec![op, cell, waiver(Some("nothing"), None, "stale")],
    };
    let Applied {
        unwaived,
        waived,
        unused,
    } = apply(
        &set,
        vec![
            lost("nurbs.point", "a"),
            lost("nurbs.basis", "a"),
            lost("nurbs.point", "z"),
        ],
    );
    assert_eq!(unwaived, vec![lost("nurbs.basis", "a")]);
    // The first waiver in the file that covers a violation gives its reason.
    let reasons: Vec<&str> = waived.iter().map(|(_, r)| r.as_str()).collect();
    assert_eq!(reasons, ["one operation", "one operation"]);
    // Both of the first two covered something: the narrow one too, though the wide one gave the reason.
    assert_eq!(unused.len(), 1);
    assert_eq!(unused[0].reason, "stale");
    // Nothing to waive leaves every waiver unused, and nothing unwaived.
    let none = apply(&set, Vec::new());
    assert!(none.unwaived.is_empty() && none.waived.is_empty());
    assert_eq!(none.unused.len(), 3);
    // No waivers waive nothing.
    let bare = apply(&Waivers::default(), vec![lost("a", "b")]);
    assert_eq!(bare.unwaived.len(), 1);
}

fn seen(operation: &str, family: &str, found: Option<(&str, Settled)>) -> Observed {
    Observed {
        operation: operation.to_owned(),
        family: family.to_owned(),
        found: found.map(|(kind, settled)| (kind.to_owned(), settled)),
    }
}

fn night() -> Report {
    Report::from_night(
        ID,
        [
            seen("nurbs.point", "a", None),
            seen("nurbs.point", "a", Some(("value", Settled::Theirs))),
            seen("nurbs.point", "a", Some(("value", Settled::Theirs))),
            seen("nurbs.point", "a", Some(("refused", Settled::Unsettled))),
            seen("nurbs.basis", "a", None),
            seen("nurbs.point", "b", None),
        ],
    )
}

#[test]
fn a_report_counts_a_nights_cells() {
    let report = night();
    assert_eq!((report.format, report.jobs), (1, 6));
    let cells: Vec<(&str, &str, u64, u64)> = report
        .cells
        .iter()
        .map(|c| (c.operation.as_str(), c.family.as_str(), c.jobs, c.agree))
        .collect();
    assert_eq!(
        cells,
        [
            ("nurbs.basis", "a", 1, 1),
            ("nurbs.point", "a", 4, 1),
            ("nurbs.point", "b", 1, 1)
        ]
    );
    let point = &report.cells[1];
    let found: Vec<(&str, Settled, u64)> = point
        .differences
        .iter()
        .map(|f| (f.kind.as_str(), f.settled, f.count))
        .collect();
    assert_eq!(
        found,
        [
            ("refused", Settled::Unsettled, 1),
            ("value", Settled::Theirs, 2)
        ]
    );
    // The same night in another order is the same report, and it reads back as written.
    assert_eq!(
        Report::from_night(
            ID,
            [
                seen("nurbs.point", "b", None),
                seen("nurbs.basis", "a", None),
                seen("nurbs.point", "a", Some(("refused", Settled::Unsettled))),
                seen("nurbs.point", "a", Some(("value", Settled::Theirs))),
                seen("nurbs.point", "a", Some(("value", Settled::Theirs))),
                seen("nurbs.point", "a", None),
            ]
        ),
        report
    );
    let text = report.to_json().unwrap();
    assert!(text.ends_with("}\n"));
    assert_eq!(Report::parse(&text).unwrap(), report);
    assert_eq!(Report::from_night(ID, []).jobs, 0);
}

#[test]
fn a_report_that_does_not_add_up_is_refused() {
    let text = night().to_json().unwrap();
    assert!(matches!(Report::parse("{"), Err(ReportError::Json(_))));
    assert!(matches!(
        Report::parse(&text.replace("\"format\": 1", "\"format\": 2")),
        Err(ReportError::Rule(w)) if w.contains("format")
    ));
    assert!(matches!(
        Report::parse(&text.replace("\"jobs\": 6", "\"jobs\": 7")),
        Err(ReportError::Rule(w)) if w.contains("cells hold")
    ));
    let mut bad = night();
    bad.cells[1].agree = 3;
    assert!(matches!(
        Report::parse(&bad.to_json().unwrap()),
        Err(ReportError::Rule(w)) if w.contains("agree")
    ));
    assert!(matches!(
        Report::parse(&text.replace("\"agree\"", "\"agrees\"")),
        Err(ReportError::Json(_))
    ));
}

#[test]
fn the_ratchet_names_a_lost_cell_and_nothing_else() {
    let base = night();
    assert!(ratchet(&base, &base).unwrap().is_empty());
    // A night that adds a cell, or finds more or fewer differences, loses none.
    let more = Report::from_night(
        ID,
        [
            seen("nurbs.point", "a", Some(("value", Settled::Theirs))),
            seen("nurbs.basis", "a", None),
            seen("nurbs.point", "b", Some(("value", Settled::Theirs))),
            seen("nurbs.insert", "c", None),
        ],
    );
    assert!(ratchet(&base, &more).unwrap().is_empty());
    // A night without the basis cell and the b cell loses both.
    let less = Report::from_night(ID, [seen("nurbs.point", "a", None)]);
    let lost: Vec<String> = ratchet(&base, &less)
        .unwrap()
        .iter()
        .map(|v| format!("{} {}", v.operation, v.family))
        .collect();
    assert_eq!(lost, ["nurbs.basis a", "nurbs.point b"]);
    let v = &ratchet(&base, &less).unwrap()[0];
    assert_eq!((v.check, v.oracle.as_str()), (Check::Coverage, ID));
    assert!(
        v.to_string().starts_with("coverage nurbs.basis/geomdl/a: "),
        "{v}"
    );
    // An empty cell in the earlier night had nothing to lose.
    let mut empty = night();
    empty.cells[0].jobs = 0;
    empty.cells[0].agree = 0;
    assert_eq!(ratchet(&empty, &less).unwrap().len(), 1);
    // Reports of two oracles do not compare.
    let other = Report::from_night("cgal", [seen("nurbs.point", "a", None)]);
    assert!(matches!(
        ratchet(&base, &other),
        Err(ReportError::Rule(w)) if w.contains("cgal")
    ));
    // A waiver covers the loss it names and the report is clean.
    let set = Waivers {
        format: 1,
        waiver: vec![
            waiver(Some("nurbs.basis"), None, "r"),
            waiver(None, Some("b"), "s"),
        ],
    };
    let applied = apply(&set, ratchet(&base, &less).unwrap());
    assert!(applied.unwaived.is_empty() && applied.unused.is_empty());
    assert_eq!(applied.waived.len(), 2);
}

fn signature() -> Signature {
    Signature {
        operation: "nurbs.point".to_owned(),
        oracle: ID.to_owned(),
        kind: "value".to_owned(),
        family: "f".to_owned(),
    }
}

fn evidence() -> Evidence {
    Evidence {
        issue: "https://github.com/orbingol/NURBS-Python/issues/1".to_owned(),
        reproduced_on: "5.4.0".to_owned(),
        reproducer: r#"{"id":1,"op":"point"}"#.to_owned(),
    }
}

fn record(outcome: Triage, settled: Settled, evidence: Option<Evidence>) -> Record {
    Record {
        signature: signature(),
        outcome,
        settled,
        fixture: "oracle/0001-a".to_owned(),
        reference: Some("r".to_owned()),
        evidence,
        why: "w".to_owned(),
    }
}

fn register(records: Vec<Record>) -> Result<Register, TriageError> {
    Register::parse(
        &Register {
            format: 1,
            about: "a".to_owned(),
            records,
        }
        .to_json()
        .unwrap(),
    )
}

fn refused(records: Vec<Record>) -> String {
    match register(records) {
        Err(TriageError::Rule(why)) => why,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_report_address_is_https_with_a_host_and_a_path() {
    for good in [
        "https://github.com/orbingol/NURBS-Python/issues/1",
        "https://gitlab.com/g/p/-/issues/9",
        "https://example.org/a",
    ] {
        assert!(is_report_address(good), "{good}");
    }
    for bad in [
        "",
        "http://github.com/o/r/issues/1",
        "https://",
        "https://github.com",
        "https://github.com/",
        "https://localhost/issues/1",
        "https://.com/x",
        "https://github.com./x",
        "https://github.com/o r/issues/1",
        "https://github.com/o\n/issues/1",
        "issue 1",
        "#7",
        "ftp://github.com/o/r",
    ] {
        assert!(!is_report_address(bad), "{bad:?}");
    }
}

#[test]
fn a_theirs_is_recorded_with_its_evidence_and_nothing_less() {
    let theirs = |e: Option<Evidence>| record(Triage::Theirs, Settled::Theirs, e);
    assert!(register(vec![theirs(Some(evidence()))]).is_ok());
    // No evidence, and each part of it missing or empty.
    assert!(refused(vec![theirs(None)]).contains("evidence"));
    let cases: [(&str, fn(&mut Evidence)); 9] = [
        ("report", |e| e.issue = "issue 1".to_owned()),
        ("report", |e| e.issue = String::new()),
        ("report", |e| e.issue = "https://example.org".to_owned()),
        ("pin", |e| e.reproduced_on = " ".to_owned()),
        ("reproducer", |e| e.reproducer = String::new()),
        ("reproducer", |e| e.reproducer = "not json".to_owned()),
        ("reproducer", |e| e.reproducer = "[1]".to_owned()),
        ("reproducer", |e| e.reproducer = "{}".to_owned()),
        ("reproducer", |e| e.reproducer = "7".to_owned()),
    ];
    for (what, mutate) in cases {
        let mut e = evidence();
        mutate(&mut e);
        let why = refused(vec![theirs(Some(e.clone()))]);
        assert!(why.contains(what), "{what}: {why} for {e:?}");
    }
    // A "theirs" reference alone is not evidence any more.
    let mut named = theirs(None);
    named.reference = Some("upstream issue 1".to_owned());
    assert!(refused(vec![named]).contains("evidence"));
    // Evidence belongs to "theirs" only.
    for (outcome, settled) in [
        (Triage::Ours, Settled::Ours),
        (Triage::WithinBounds, Settled::Both),
        (Triage::Undecided, Settled::Unsettled),
    ] {
        let why = refused(vec![record(outcome, settled, Some(evidence()))]);
        assert!(why.contains("only Theirs"), "{outcome:?}: {why}");
    }
    // Unknown fields do not slip in either.
    let text = register(vec![theirs(Some(evidence()))])
        .unwrap()
        .to_json()
        .unwrap();
    assert!(matches!(
        Register::parse(&text.replace("\"reproducer\"", "\"proof\"")),
        Err(TriageError::Json(_))
    ));
}

#[test]
fn no_outcome_rests_on_a_verdict_that_clears_the_kernel() {
    // Each outcome with each verdict: only the pairs below stand.
    let outcomes = [
        Triage::Ours,
        Triage::Theirs,
        Triage::WithinBounds,
        Triage::Undecided,
    ];
    let verdicts = [
        Settled::Ours,
        Settled::Theirs,
        Settled::Both,
        Settled::Unsettled,
    ];
    let stands = |o: Triage, s: Settled| match o {
        Triage::Ours => matches!(s, Settled::Ours | Settled::Unsettled),
        Triage::Theirs => s == Settled::Theirs,
        Triage::WithinBounds => s == Settled::Both,
        Triage::Undecided => true,
    };
    for outcome in outcomes {
        for settled in verdicts {
            let e = (outcome == Triage::Theirs).then(evidence);
            let got = register(vec![record(outcome, settled, e)]);
            assert_eq!(
                got.is_ok(),
                stands(outcome, settled),
                "{outcome:?} on {settled:?}"
            );
        }
    }
    // The cases that matter, by name: "theirs" and "within bounds" never rest on ours or unsettled,
    // and "ours" never rests on a verdict that says the oracle was the one out.
    for settled in [Settled::Ours, Settled::Both, Settled::Unsettled] {
        assert!(
            refused(vec![record(Triage::Theirs, settled, Some(evidence()))]).contains("verdict")
        );
    }
    for settled in [Settled::Ours, Settled::Theirs, Settled::Unsettled] {
        assert!(refused(vec![record(Triage::WithinBounds, settled, None)]).contains("verdict"));
    }
    for settled in [Settled::Theirs, Settled::Both] {
        assert!(refused(vec![record(Triage::Ours, settled, None)]).contains("verdict"));
    }
}

#[test]
fn a_record_cannot_clear_a_wrong_answer_of_ours() {
    let known = register(vec![record(
        Triage::Theirs,
        Settled::Theirs,
        Some(evidence()),
    )])
    .unwrap();
    let sig = signature();
    // The job the record was written on stands.
    assert_eq!(known.contradicts(&sig, Settled::Theirs), None);
    // A job of the same signature the reference finds ours does not, nor one it cannot settle, nor
    // one where both are within the bound.
    for settled in [Settled::Ours, Settled::Both, Settled::Unsettled] {
        let why = known.contradicts(&sig, settled).unwrap();
        assert!(why.contains(&sig.to_string()), "{why}");
    }
    // "Within bounds" is held to "both" the same way.
    let both = register(vec![record(Triage::WithinBounds, Settled::Both, None)]).unwrap();
    assert_eq!(both.contradicts(&sig, Settled::Both), None);
    for settled in [Settled::Ours, Settled::Theirs, Settled::Unsettled] {
        assert!(both.contradicts(&sig, settled).is_some(), "{settled:?}");
    }
    // A fixed bug ("ours") and an open question ("undecided") stand until the job is found ours.
    for (outcome, settled) in [
        (Triage::Ours, Settled::Ours),
        (Triage::Undecided, Settled::Unsettled),
    ] {
        let reg = register(vec![record(outcome, settled, None)]).unwrap();
        for found in [Settled::Theirs, Settled::Both, Settled::Unsettled] {
            assert_eq!(reg.contradicts(&sig, found), None, "{outcome:?} {found:?}");
        }
        assert!(
            reg.contradicts(&sig, Settled::Ours).is_some(),
            "{outcome:?}"
        );
    }
    // Found ours, a job fails under any record, and under none.
    let empty = register(Vec::new()).unwrap();
    assert!(empty.contradicts(&sig, Settled::Ours).is_some());
    // A signature the register does not hold is a new signature, which has its own failure.
    for found in [Settled::Theirs, Settled::Both, Settled::Unsettled] {
        assert_eq!(empty.contradicts(&sig, found), None);
    }
    // Another signature's record is not this one's.
    let mut elsewhere = signature();
    elsewhere.family = "g".to_owned();
    assert_eq!(known.contradicts(&elsewhere, Settled::Both), None);
}

#[test]
fn a_theirs_is_reproduced_on_the_catalogues_pin() {
    let catalogue = Catalogue::embedded().unwrap();
    let pin = catalogue.get(ID).unwrap().pin.clone().unwrap();
    let mut e = evidence();
    e.reproduced_on = pin;
    let good = register(vec![record(Triage::Theirs, Settled::Theirs, Some(e))]).unwrap();
    assert!(good.check_catalogue(&catalogue).is_empty());
    // Another pin: the report was reproduced on another build than the one that runs.
    let mut e = evidence();
    e.reproduced_on = "0.0.0".to_owned();
    let stale = register(vec![record(Triage::Theirs, Settled::Theirs, Some(e))]).unwrap();
    let problems = stale.check_catalogue(&catalogue);
    assert_eq!(problems.len(), 1);
    assert!(
        problems[0].contains("reproduce it on the pin"),
        "{problems:?}"
    );
    // A planned oracle has no pin, so nothing was reproduced on it.
    let cgal = catalogue.get("cgal").unwrap();
    assert_eq!(cgal.pin, None);
    let mut s = signature();
    s.oracle = "cgal".to_owned();
    let mut r = record(Triage::Theirs, Settled::Theirs, Some(evidence()));
    r.signature = s;
    let problems = register(vec![r]).unwrap().check_catalogue(&catalogue);
    assert!(problems[0].contains("no pin"), "{problems:?}");
    // An oracle the catalogue does not hold, with or without evidence.
    let mut s = signature();
    s.oracle = "ghost".to_owned();
    let mut r = record(Triage::WithinBounds, Settled::Both, None);
    r.signature = s;
    let problems = register(vec![r]).unwrap().check_catalogue(&catalogue);
    assert!(problems[0].contains("ghost"), "{problems:?}");
    // The shipped register holds.
    assert!(
        Register::embedded()
            .unwrap()
            .check_catalogue(&catalogue)
            .is_empty()
    );
}

#[test]
fn a_moved_answer_is_past_the_allowance_and_caught() {
    // The step is relative to the value, or to one for a small one; a value that is not finite
    // becomes zero, which is not what it was.
    assert_eq!(perturbed(0.0), PERTURBATION);
    assert_eq!(perturbed(0.5), 0.5 + PERTURBATION);
    assert_eq!(perturbed(-8.0), -8.0 + PERTURBATION * 8.0);
    for x in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(perturbed(x), 0.0);
    }
    // It is far beyond any allowance a job states, and an allowance of that size would not see it.
    let all = jobs(Seed(1), 200);
    for job in &all {
        let stated = bound(job, 100.0);
        let allowance = Allowance {
            ours: stated,
            theirs: stated,
            rounding: 0.0,
            fit: stated,
        };
        assert!(allowance.total() < PERTURBATION, "{job:?}");
        assert!(!within(1.0, perturbed(1.0), &allowance));
    }
    // Every honest answer, moved, is a difference of the comparison; the kernel agrees with itself.
    let mut moved = 0;
    for job in &all {
        let answer = honest(job);
        assert_eq!(judge(job, &answer).unwrap(), Compared::Agree);
        let other = perturb(&answer).unwrap();
        assert_eq!(other.id, answer.id);
        assert_ne!(other, answer);
        // Only the first value moves.
        assert_eq!(
            other.values.as_ref().unwrap()[1..],
            answer.values.as_ref().unwrap()[1..]
        );
        assert!(
            matches!(judge(job, &other).unwrap(), Compared::Differ { .. }),
            "{job:?}"
        );
        moved += 1;
    }
    assert_eq!(moved, 200);
}

#[test]
fn nothing_to_move_is_not_moved() {
    let refusal = Answer {
        id: 1,
        values: None,
    };
    assert_eq!(perturb(&refusal), None);
    let empty = Answer {
        id: 1,
        values: Some(Vec::new()),
    };
    assert_eq!(perturb(&empty), None);
    let broken = Answer {
        id: 1,
        values: Some(vec!["not a number".to_owned()]),
    };
    assert_eq!(perturb(&broken), None);
    // A single value moves, and reads back as the moved value.
    let one = Answer {
        id: 9,
        values: Some(vec![float(2.0).unwrap()]),
    };
    let moved = perturb(&one).unwrap();
    let back: f64 = moved.values.unwrap()[0].parse().unwrap();
    assert_eq!(back, perturbed(2.0));
}

#[test]
fn a_flawed_reference_is_ours_to_settle_not_to_trust() {
    // The kernel is right and the stand-in moved: the exact reference settles it as theirs, and a
    // register entry written on that verdict covers the flaw and nothing the kernel got wrong.
    let all = jobs(Seed(3), 200);
    let job = all.iter().find(|j| j.op == Op::Point).unwrap();
    let Compared::Differ { verdict, .. } = judge(job, &flawed(job, 1.0)).unwrap() else {
        panic!("a moved point agrees")
    };
    assert_eq!(verdict, Settled::Theirs);
}

#[test]
fn a_blind_comparison_is_found_blind() {
    let all = jobs(Seed(7), 30);
    let answers: Vec<Answer> = all.iter().map(honest).collect();
    // The comparison as it is sees every moved answer.
    let found = sensitivity(&all, &answers, judge).unwrap();
    assert_eq!((found.moved, found.caught, found.unmoved), (30, 30, 0));
    assert!(found.missed.is_empty());
    // One that agrees with everything is blind to every one of them, and names each job.
    let blind = sensitivity(&all, &answers, |_, _| Ok(Compared::Agree)).unwrap();
    assert_eq!((blind.moved, blind.caught), (30, 0));
    let ids: Vec<u64> = all.iter().map(|j| j.id).collect();
    assert_eq!(blind.missed, ids);
    // One blind to a single operation misses those jobs and no others.
    let partly = sensitivity(&all, &answers, |job, answer| {
        if job.op == Op::Basis {
            Ok(Compared::Agree)
        } else {
            judge(job, answer)
        }
    })
    .unwrap();
    assert!(!partly.missed.is_empty());
    assert_eq!(partly.caught + partly.missed.len(), 30);
    for id in &partly.missed {
        assert_eq!(all.iter().find(|j| j.id == *id).unwrap().op, Op::Basis);
    }
    // A judge that cannot read a job is an error, not a pass.
    assert!(sensitivity(&all, &answers, |_, _| Err("broken".to_owned())).is_err());
    // An answer that already differs is not moved, and a shared refusal has nothing to move.
    let point = all.iter().position(|j| j.op == Op::Point).unwrap();
    let mut mixed = answers.clone();
    mixed[point] = flawed(&all[point], 1.0);
    let found = sensitivity(&all, &mixed, judge).unwrap();
    assert_eq!((found.moved, found.caught, found.unmoved), (29, 29, 0));
    let mut refused = all[0].clone();
    refused.at = "9".to_owned();
    let both_refuse = Answer {
        id: refused.id,
        values: None,
    };
    let found = sensitivity(&[refused], &[both_refuse], judge).unwrap();
    assert_eq!((found.moved, found.unmoved), (0, 1));
    // No jobs, nothing found.
    assert_eq!(
        sensitivity(&[], &[], judge).unwrap(),
        geom_test::oracle::geomdl::Sensitivity::default()
    );
}
