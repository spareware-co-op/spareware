//! The report's shape: what it holds and refuses, the names it takes out, and what a fixer, the
//! owner and the public read of it.

// The crate denies these outside tests (Cargo.toml); clippy.toml's allow-*-in-tests settings don't
// reach helpers outside `#[test]` functions, and a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    reason = "test code; the rules are for the crate's own code"
)]

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use construct_report::{
    ANOTHER, Failure, Kind, MAX_EDITS, MAX_RECORD_BYTES, MAX_SCREENSHOT_BYTES, Part, Program,
    Replay, Replayed, Report, SENDER, Screenshot, Source, State, Step, Stored, Topic, Where,
    by_topic, other, public_issue,
};
use serde_json::{Value, json};

fn step(seq: u64, by: &str, cmd: &str, args: Value, failed: Option<&str>) -> Step {
    Step {
        seq,
        by: by.to_owned(),
        at_ms: 1_000 + seq,
        cmd: cmd.to_owned(),
        args,
        failed: failed.map(str::to_owned),
    }
}

/// A report of a value turned back, as the page builds it with every part left in.
fn problem() -> Report {
    Report {
        format: 1,
        kind: Kind::Problem,
        summary: "Turned back: Sill height takes a length".to_owned(),
        words: "I typed a weight by mistake, and Ben said it should say why.".to_owned(),
        topic: None,
        failure: Some(Failure::refused(
            "element.param",
            "Sill height takes a length, such as 900 mm.",
        )),
        place: Some(Where {
            project: "example-1".to_owned(),
            model: "arch-model".to_owned(),
        }),
        elements: vec![Part {
            id: "D-9".to_owned(),
            record: json!({ "id": "D-9", "kind": "door", "recipe": { "op": "box" } }),
        }],
        edits: vec![
            step(3, other(1).as_str(), "element.create", json!({ "element": "D-9" }), None),
            step(7, SENDER, "element.claim", json!({ "element": "D-9" }), Some("Ben is working on this.")),
            step(
                9,
                SENDER,
                "element.param",
                json!({ "element": "D-9", "parameter": "P-sill", "value": "3 kg" }),
                Some("Sill height takes a length, such as 900 mm."),
            ),
        ],
        program: Some(Program {
            version: "0.1.0".to_owned(),
            engine: "stand-in 1".to_owned(),
        }),
        browser: Some("Mozilla/5.0 (a browser)".to_owned()),
        page_errors: vec!["TypeError: x is undefined at plan.js:10".to_owned()],
        server_log: vec!["INFO request=r1 status=200".to_owned()],
        screenshot: None,
    }
}

fn feedback(topic: Topic) -> Report {
    Report {
        format: 1,
        kind: Kind::Feedback,
        summary: "Snap to grid crossings".to_owned(),
        words: "It would help to snap a wall's end to where two grids cross.".to_owned(),
        topic: Some(topic),
        failure: None,
        place: None,
        elements: Vec::new(),
        edits: Vec::new(),
        program: None,
        browser: None,
        page_errors: Vec::new(),
        server_log: Vec::new(),
        screenshot: None,
    }
}

fn refusal(report: &Report) -> String {
    report.check().unwrap_err().0
}

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

#[test]
fn a_full_report_is_kept_and_reads_back_to_the_same_bytes() {
    let report = problem();
    report.check().unwrap();
    let text = serde_json::to_string(&report).unwrap();
    let back: Report = serde_json::from_str(&text).unwrap();
    assert_eq!(back, report);
    assert_eq!(serde_json::to_string(&back).unwrap(), text);
    // The model it is from is `where`, and a part left out is not written at all.
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["where"]["model"], "arch-model");
    assert!(value.get("screenshot").is_none() && value.get("topic").is_none());
}

#[test]
fn a_report_holds_only_the_parts_it_names() {
    let mut value = serde_json::to_value(problem()).unwrap();
    value["project_data"] = json!({ "walls": [] });
    let read = serde_json::from_value::<Report>(value).unwrap_err().to_string();
    assert!(read.contains("unknown field `project_data`"), "{read}");
    // Nor can a part carry more than its own fields: a person's name on an edit, say.
    let mut value = serde_json::to_value(problem()).unwrap();
    value["edits"][0]["person"] = json!("Ben");
    assert!(serde_json::from_value::<Report>(value).is_err());
}

#[test]
fn every_part_may_be_left_out_but_the_summary() {
    let bare = Report {
        words: String::new(),
        failure: None,
        place: None,
        elements: Vec::new(),
        edits: Vec::new(),
        program: None,
        browser: None,
        page_errors: Vec::new(),
        server_log: Vec::new(),
        ..problem()
    };
    bare.check().unwrap();
    let text = serde_json::to_string(&bare).unwrap();
    assert_eq!(
        text,
        r#"{"format":1,"kind":"problem","summary":"Turned back: Sill height takes a length","words":""}"#
    );
    let none = Report {
        summary: "  ".to_owned(),
        ..bare
    };
    assert_eq!(refusal(&none), "Give a summary.");
}

#[test]
fn a_report_over_its_caps_is_refused_in_words() {
    let two_lines = Report {
        summary: "one\ntwo".to_owned(),
        ..problem()
    };
    assert_eq!(refusal(&two_lines), "A summary is one line.");
    let long = Report {
        summary: "x".repeat(161),
        ..problem()
    };
    assert_eq!(refusal(&long), "A summary is longer than 160 characters.");
    let mut many = problem();
    many.edits = (1..=MAX_EDITS as u64 + 1)
        .map(|seq| step(seq, SENDER, "element.move", json!({}), None))
        .collect();
    assert_eq!(refusal(&many), "A report holds at most 500 edits.");
    let mut big = problem();
    big.elements[0].record["recipe"]["text"] = json!("x".repeat(MAX_RECORD_BYTES));
    let words = refusal(&big);
    assert!(
        words.starts_with("This report is 25") && words.ends_with("KB without its screenshot; the most is 256 KB. Leave a part out."),
        "{words}"
    );
    let mut lines = problem();
    lines.page_errors = vec!["e".to_owned(); 201];
    assert_eq!(refusal(&lines), "The page's errors holds more than 200 lines.");
}

#[test]
fn edits_are_a_models_own_in_log_order_by_stand_ins() {
    let mut named = problem();
    named.edits[0].by = "ben".to_owned();
    assert!(refusal(&named).starts_with("Edit 3 is by \"ben\""));
    let mut backwards = problem();
    backwards.edits.swap(0, 1);
    assert_eq!(refusal(&backwards), "The edits are in the order of the log, each once.");
    let mut adds_a_model = problem();
    adds_a_model.edits[0].cmd = "model.add".to_owned();
    assert_eq!(
        refusal(&adds_a_model),
        "Edit 3 is model.add: a report replays a model's own edits only."
    );
    let mut nowhere = problem();
    nowhere.place = None;
    assert_eq!(refusal(&nowhere), "Elements and edits come with the model they are from.");
    let mut twice = problem();
    twice.elements.push(twice.elements[0].clone());
    assert_eq!(refusal(&twice), "Element D-9 is in the report twice.");
}

#[test]
fn a_failure_is_named_by_its_source_and_a_name() {
    assert_eq!(problem().failure.unwrap().source(), Some(Source::Refused));
    let built = Failure::build("Unfilled", "The floor's boundary is missing.", "F-1");
    assert_eq!(built.code, "build.Unfilled");
    assert_eq!(built.source(), Some(Source::Build));
    assert!(Source::Build.replayed() && !Source::Mismatch.replayed());
    assert_eq!(Source::of("server.failed"), Some(Source::Server));
    assert_eq!(Source::of("refused."), None);
    assert_eq!(Source::of("crashed.badly"), None);
    let mut odd = problem();
    odd.failure.as_mut().unwrap().code = "crashed.badly".to_owned();
    assert!(refusal(&odd).contains("isn't a failure code"));
}

#[test]
fn feedback_has_a_topic_and_no_failure() {
    feedback(Topic::Modelling).check().unwrap();
    let untopical = Report {
        topic: None,
        ..feedback(Topic::Speed)
    };
    assert_eq!(refusal(&untopical), "Choose what your feedback is about.");
    let failing = Report {
        failure: problem().failure,
        ..feedback(Topic::Speed)
    };
    assert!(refusal(&failing).starts_with("Feedback holds your words"));
    let topical = Report {
        topic: Some(Topic::Data),
        ..problem()
    };
    assert_eq!(refusal(&topical), "A topic is for feedback.");
}

#[test]
fn a_screenshot_is_a_picture_within_its_cap() {
    let with = |media: &str, bytes: &[u8]| Report {
        screenshot: Some(Screenshot {
            media: media.to_owned(),
            data: STANDARD.encode(bytes),
        }),
        ..problem()
    };
    with("image/png", PNG).check().unwrap();
    with("image/jpeg", &[0xFF, 0xD8, 0xFF, 0xE0]).check().unwrap();
    assert_eq!(refusal(&with("image/jpeg", PNG)), "A screenshot is a PNG or a JPEG picture.");
    assert_eq!(refusal(&with("text/html", b"<p>")), "A screenshot is a PNG or a JPEG picture.");
    let mut huge = PNG.to_vec();
    huge.resize(MAX_SCREENSHOT_BYTES + 1, 0);
    assert_eq!(
        refusal(&with("image/png", &huge)),
        "The screenshot is larger than 1 MB. Send a smaller one."
    );
    // The screenshot does not count toward the record's own cap.
    let mut near = PNG.to_vec();
    near.resize(MAX_SCREENSHOT_BYTES, 0);
    with("image/png", &near).check().unwrap();
}

#[test]
fn other_peoples_names_are_taken_out_as_whole_words() {
    let mut report = problem();
    report.server_log.push("Anna asked; BEN answered; Bench seat".to_owned());
    report
        .scrub(&[
            ("Ben".to_owned(), other(1)),
            ("Cara Lee".to_owned(), ANOTHER.to_owned()),
            ("Anna".to_owned(), ANOTHER.to_owned()),
        ])
        .unwrap();
    assert_eq!(report.edits[1].failed.as_deref(), Some("other-1 is working on this."));
    assert_eq!(report.words, "I typed a weight by mistake, and other-1 said it should say why.");
    assert_eq!(report.server_log[1], "another person asked; other-1 answered; Bench seat");
    // The model's inputs are left as they are: a replay needs them.
    assert_eq!(report.edits[2].args, problem().edits[2].args);
    assert_eq!(report.elements, problem().elements);
    // No names, no change.
    let mut same = problem();
    same.scrub(&[]).unwrap();
    assert_eq!(same, problem());
}

#[test]
fn the_failing_edit_is_the_senders_last_turned_back() {
    let report = problem();
    let failing = report.failing().unwrap();
    assert_eq!((failing.seq, failing.cmd.as_str()), (9, "element.param"));
    let mut none = problem();
    none.edits.truncate(1);
    assert!(none.failing().is_none());
}

fn reproduced(number: u64) -> Stored {
    let (mut stored, _) = Stored::received(number, "anna", 5_000, problem());
    stored.replayed(Replay {
        engine: "stand-in 1".to_owned(),
        reproduced: true,
        code: Some("refused.element.param".to_owned()),
        words: Some("Sill height takes a length, such as 900 mm.".to_owned()),
        steps: vec![Replayed {
            seq: 9,
            cmd: "element.param".to_owned(),
            then: Some("Sill height takes a length, such as 900 mm.".to_owned()),
            now: Some("Sill height takes a length, such as 900 mm.".to_owned()),
        }],
        note: None,
    });
    stored
}

#[test]
fn a_stored_report_keeps_its_screenshot_apart_and_moves_on_in_states() {
    let with = Report {
        screenshot: Some(Screenshot {
            media: "image/png".to_owned(),
            data: STANDARD.encode(PNG),
        }),
        ..problem()
    };
    let (mut stored, screenshot) = Stored::received(7, "anna", 5_000, with);
    assert!(stored.screenshot && stored.report.screenshot.is_none() && screenshot.is_some());
    assert_eq!(stored.state, State::Received);
    assert_eq!(stored.listed().shown, "Received");
    // A replay that does not end in the failure leaves it received.
    stored.replayed(Replay {
        engine: "stand-in 1".to_owned(),
        reproduced: false,
        code: None,
        words: None,
        steps: Vec::new(),
        note: Some("The edit applied in the replay.".to_owned()),
    });
    assert_eq!(stored.state, State::Received);
    let mut stored = reproduced(7);
    assert_eq!(stored.state, State::Reproduced);
    stored.fixed("0.2.0").unwrap();
    assert_eq!(stored.listed().shown, "Fixed in version 0.2.0");
    assert_eq!(
        stored.fixed("0.3.0").unwrap_err().0,
        "Report 7 is already fixed in version 0.2.0."
    );
    let text = serde_json::to_string(&stored).unwrap();
    assert!(text.contains(r#""state":{"fixed":{"version":"0.2.0"}}"#), "{text}");
    assert_eq!(serde_json::from_str::<Stored>(&text).unwrap(), stored);
    let listed = serde_json::to_value(reproduced(8).listed()).unwrap();
    assert_eq!(listed["state"], "reproduced");
    assert_eq!(listed["code"], "refused.element.param");
}

#[test]
fn a_fixer_reads_one_line_first() {
    assert_eq!(
        reproduced(7).line(),
        "#7 reproduced | refused.element.param: Sill height takes a length, such as 900 mm. | elements D-9 \
         | 3 edits; the replay on stand-in 1 reproduced it | stand-in 1 | version 0.1.0"
    );
    let (idea, _) = Stored::received(8, "anna", 5_000, feedback(Topic::Views));
    assert_eq!(idea.line(), "#8 received | feedback (Views and sheets): Snap to grid crossings");
}

#[test]
fn a_reproduced_fault_becomes_a_public_issue_with_no_project_data() {
    let stored = reproduced(7);
    let issue = public_issue(&stored).unwrap();
    assert_eq!(issue.title, "Reproduced from report 7: refused.element.param");
    for held in ["D-9", "P-sill", "3 kg", "Sill height", "arch-model", "example-1", "anna", "Ben", "mistake"] {
        assert!(!issue.body.contains(held), "the issue holds {held:?}: {}", issue.body);
    }
    for said in ["`element.create`, `element.claim`, `element.param`", "`door`", "`stand-in 1`", "number 7"] {
        assert!(issue.body.contains(said), "the issue lacks {said:?}: {}", issue.body);
    }
    let (unreplayed, _) = Stored::received(9, "anna", 5_000, problem());
    assert!(public_issue(&unreplayed).is_err());
}

#[test]
fn feedback_is_grouped_by_topic() {
    let numbered = |number: u64, topic: Topic| Stored::received(number, "anna", 0, feedback(topic)).0;
    let all = [
        numbered(4, Topic::Speed),
        numbered(2, Topic::Speed),
        numbered(3, Topic::Views),
        reproduced(1),
    ];
    let grouped = by_topic(&all);
    assert_eq!(grouped.len(), 2);
    assert_eq!(grouped[&Topic::Speed], vec![2, 4]);
    assert_eq!(grouped[&Topic::Views], vec![3]);
    assert_eq!(Topic::ALL.len(), 7);
    assert_eq!(Topic::Data.label(), "Schedules, parameters and types");
}
