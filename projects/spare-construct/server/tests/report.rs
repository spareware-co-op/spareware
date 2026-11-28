//! Reports of problems and feedback (piece A100), in-process on the test store: a report built from
//! a known failure replays, in an empty project, to the same failure code; it names nobody but by
//! stand-ins; nothing of a replay lands in the store; and a report is its sender's alone.

// The server denies these outside tests (Cargo.toml). Tests may use them, as the geometry kernel's
// do: clippy.toml's allow-*-in-tests settings don't reach helpers outside `#[test]` functions, and
// a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    reason = "tests; the rules are for the server's own code"
)]

mod support;

use construct_model::geometry::Recipe;
use construct_model::rows::BuiltResult;
use construct_report::{Failure, Kind, OTHER, Report, SENDER, State, Topic};
use kit_core::Id;
use kit_store::memory::{Memory, MemoryFiles};
use kit_store::{Kv, Scope};
use kit_sync::read_log;
use serde_json::{Value, json};
use spare_server::backup;
use spare_server::report::Ask;
use spare_server::seed::seed;
use spare_server::service::{Refusal, Service};
use std::time::Duration;

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

struct Stack {
    store: Memory,
    files: MemoryFiles,
    service: Service<Memory, MemoryFiles>,
}

async fn seeded() -> Stack {
    let store = Memory::new();
    let files = MemoryFiles::default();
    let service = support::build(&store, &files, construct_regen::geometry());
    seed(&store, &service).await.unwrap();
    Stack {
        store,
        files,
        service,
    }
}

/// Waits for the shape keeper's checkpoints to stop landing.
async fn quiet(stack: &Stack) {
    let mut last = backup::digest(&stack.store, &stack.files).await.unwrap();
    loop {
        tokio::time::sleep(Duration::from_millis(1600)).await;
        let now = backup::digest(&stack.store, &stack.files).await.unwrap();
        if now == last {
            return;
        }
        last = now;
    }
}

/// The browser tab and counter of `person`'s last edit that was turned back, as the page knows
/// them for its own edit.
async fn last_turned_back(stack: &Stack, person: &str) -> (String, u64) {
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let log = read_log(&mut snapshot, &Scope::Project(i("example-1")), 0, u64::MAX - 1)
        .await
        .unwrap();
    let entry = log
        .iter()
        .rev()
        .find(|e| e.author.as_str() == person && e.failed.is_some())
        .unwrap();
    (entry.client.to_string(), entry.counter)
}

/// The report a page sends from a draft: the server's parts, the failure as the draft names it,
/// and the person's own words; the browser left out.
fn from_draft(report: Report, summary: &str, words: &str) -> Report {
    Report {
        summary: summary.to_owned(),
        words: words.to_owned(),
        browser: None,
        ..report
    }
}

/// A length parameter bound to doors, as Ben's page defines it in the browser test.
async fn sill(stack: &Stack) {
    let s = &stack.service;
    let (ben, project, model) = (i("ben"), i("example-1"), i("arch-model"));
    let definition = json!({ "id": "P-sill", "name": "Sill height", "kind": "length",
                             "holder": "instance", "choices": [] });
    s.edit(&ben, &project, &model, "parameter.define", json!({ "definition": definition }))
        .await
        .unwrap();
    let bind = json!({ "parameter": "P-sill", "categories": [300], "default": "0" });
    s.edit(&ben, &project, &model, "parameter.bind", bind)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_report_of_a_refused_edit_replays_to_the_same_failure_code() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, ben, project, model) = (i("anna"), i("ben"), i("example-1"), i("arch-model"));
    sill(&stack).await;
    let set = |value: &str| json!({ "element": "D-103", "parameter": "P-sill", "value": value });
    s.edit(&anna, &project, &model, "element.param", set("0.15 m"))
        .await
        .unwrap();
    let Err(Refusal::Refused(words)) = s
        .edit(&anna, &project, &model, "element.param", set("3 kg"))
        .await
    else {
        panic!("a weight for a length is turned back");
    };
    let (client, counter) = last_turned_back(&stack, "anna").await;
    let ask = Ask {
        elements: vec!["D-103".to_owned()],
        client: Some(client),
        counter: Some(counter),
        requests: Vec::new(),
    };
    let draft = s.reports.draft(&anna, &project, &model, &ask).await.unwrap();

    // The server's parts: the door and the table its value is read through, every edit that
    // touched them by stand-in, the failing edit last, and the program.
    let ids: Vec<&str> = draft.report.elements.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"D-103") && ids.contains(&"parameters"), "{ids:?}");
    let failing = draft.failing.unwrap();
    let last = draft.report.edits.last().unwrap();
    assert_eq!((last.seq, last.by.as_str(), last.cmd.as_str()), (failing, SENDER, "element.param"));
    assert_eq!(last.failed.as_deref(), Some(words.as_str()));
    assert!(draft.report.edits.iter().any(|e| e.cmd == "parameter.define" && e.by.starts_with(OTHER)));
    assert!(draft.report.edits.iter().all(|e| e.by == SENDER || e.by.starts_with(OTHER)));
    let failure = draft.report.failure.clone().unwrap();
    assert_eq!(failure, Failure::refused("element.param", words.clone()));
    assert_eq!(draft.report.program.as_ref().unwrap().engine, "construct.stand-in 1");
    assert_eq!(draft.left_out, 0);

    let report = from_draft(draft.report, "Turned back: a weight for a sill height", "Ben saw it too.");
    let listed = s.reports.send(&anna, report).await.unwrap();
    assert_eq!(listed.state, State::Reproduced);
    assert_eq!(listed.shown, "Reproduced");
    assert_eq!(listed.code.as_deref(), Some("refused.element.param"));
    let stored = s.reports.one(&anna, listed.number).await.unwrap();
    let replay = stored.replay.clone().unwrap();
    assert!(replay.reproduced, "{replay:?}");
    assert_eq!(replay.code.as_deref(), Some("refused.element.param"));
    assert_eq!(replay.words.as_deref(), Some(words.as_str()));
    assert_eq!(replay.engine, "construct.stand-in 1");
    assert_eq!(replay.steps.len(), stored.report.edits.len());
    // The words of the person's own report name nobody else either.
    assert_eq!(stored.report.words, "another person saw it too.");
    assert_eq!(stored.sender, "anna");
    assert!(stored.line().starts_with(&format!("#{} reproduced | refused.element.param: ", listed.number)));

    // It is Anna's alone.
    assert!(matches!(s.reports.one(&ben, listed.number).await, Err(Refusal::NotFound)));
    assert_eq!(s.reports.mine(&anna).await.unwrap(), vec![listed.clone()]);
    assert_eq!(s.reports.mine(&ben).await.unwrap(), Vec::new());
    // The co-op's side reads it, makes the generic issue, and marks it fixed.
    let issue = s.reports.issue(listed.number).await.unwrap();
    assert!(!issue.body.contains("D-103") && !issue.body.contains("Sill"));
    let fixed = s.reports.fixed(listed.number, "0.2.0").await.unwrap();
    assert_eq!(fixed.state.words(), "Fixed in version 0.2.0");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_report_names_people_only_by_stand_ins_and_its_replay_keeps_them() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, ben, project, model) = (i("anna"), i("ben"), i("example-1"), i("arch-model"));
    s.edit(&ben, &project, &model, "element.claim", json!({ "element": "D-101" }))
        .await
        .unwrap();
    let moved = json!({ "element": "D-101", "placement": { "x": 1, "y": 1, "z": 0 } });
    let Err(Refusal::Refused(words)) = s.edit(&anna, &project, &model, "element.move", moved).await
    else {
        panic!("a door Ben is working on is turned back");
    };
    assert_eq!(words, "Ben is working on this.");
    let (client, counter) = last_turned_back(&stack, "anna").await;
    let ask = Ask {
        elements: vec!["D-101".to_owned()],
        client: Some(client),
        counter: Some(counter),
        requests: Vec::new(),
    };
    let draft = s.reports.draft(&anna, &project, &model, &ask).await.unwrap();
    let claim = draft.report.edits.iter().find(|e| e.cmd == "element.claim").unwrap();
    assert!(claim.by.starts_with(OTHER));
    let failure = draft.report.failure.clone().unwrap();
    assert_eq!(failure.words, format!("{} is working on this.", claim.by));
    let text = serde_json::to_string(&draft.report).unwrap();
    for name in ["Ben", "ben", "Anna", "anna"] {
        assert!(!names(&text, name), "the draft names {name}: {text}");
    }
    let report = from_draft(draft.report, "Turned back while someone else works on it", "");
    let listed = s.reports.send(&anna, report).await.unwrap();
    let stored = s.reports.one(&anna, listed.number).await.unwrap();
    assert_eq!(stored.state, State::Reproduced, "{:?}", stored.replay);
    let text = serde_json::to_string(&stored.report).unwrap();
    assert!(!names(&text, "Ben") && !names(&text, "ben"), "{text}");
}

/// Whether `text` holds `name` as a word of its own.
fn names(text: &str, name: &str) -> bool {
    text.split(|c: char| !c.is_alphanumeric()).any(|word| word == name)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_build_failure_replays_to_the_same_code_and_nothing_of_a_replay_lands() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    let args = serde_json::from_value(json!({ "width": 0, "depth": 400, "height": 900 })).unwrap();
    let recipe = serde_json::to_value(Recipe::construct("block", args)).unwrap();
    let create = json!({ "element": "B-1", "kind": "furniture", "level": 1,
                         "placement": { "x": 100, "y": 100, "z": 0 }, "recipe": recipe,
                         "props": { "category": "furniture" } });
    s.edit(&anna, &project, &model, "element.create", create)
        .await
        .unwrap();
    s.regenerate(&project).await.unwrap();
    let shape = s.shape(&anna, &project, &model, &i("B-1"), None).await.unwrap();
    let BuiltResult::Failed { code, message } = shape.built.result else {
        panic!("a block with no width does not build");
    };
    quiet(&stack).await;
    let outside = |store: &Memory| -> Vec<String> {
        store.keys().into_iter().filter(|k| !k.starts_with("rpt/")).collect()
    };
    let before = outside(&stack.store);

    let ask = Ask {
        elements: vec!["B-1".to_owned()],
        ..Ask::default()
    };
    let draft = s.reports.draft(&anna, &project, &model, &ask).await.unwrap();
    assert!(draft.failing.is_none() && draft.report.failure.is_none());
    let report = Report {
        failure: Some(Failure::build(&code, message.clone(), "B-1")),
        ..from_draft(draft.report, "A block does not build", "")
    };
    let listed = s.reports.send(&anna, report).await.unwrap();
    let stored = s.reports.one(&anna, listed.number).await.unwrap();
    let replay = stored.replay.unwrap();
    assert!(replay.reproduced, "{replay:?}");
    assert_eq!(replay.code, Some(format!("build.{code}")));
    assert_eq!(replay.words, Some(message));

    // The replay ran in a transaction that was never committed: the store holds the report, under
    // its own keys, and not one key more.
    assert_eq!(outside(&stack.store), before);
    let mine: Vec<String> = stack.store.keys().into_iter().filter(|k| k.starts_with("rpt/")).collect();
    assert_eq!(
        mine,
        [
            "rpt/by/anna/00000000000000000001",
            "rpt/next",
            "rpt/report/00000000000000000001"
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_report_is_checked_and_belongs_to_its_sender_and_their_project() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project, model) = (i("anna"), i("dan"), i("example-1"), i("arch-model"));
    let idea = Report {
        format: 1,
        kind: Kind::Feedback,
        summary: "Snap walls to grid crossings".to_owned(),
        words: "Cara asked for this too.".to_owned(),
        topic: Some(Topic::Modelling),
        failure: None,
        place: None,
        elements: Vec::new(),
        edits: Vec::new(),
        program: None,
        browser: None,
        page_errors: Vec::new(),
        server_log: Vec::new(),
        screenshot: None,
    };
    let listed = s.reports.send(&anna, idea.clone()).await.unwrap();
    assert_eq!(listed.state, State::Received);
    let stored = s.reports.one(&anna, listed.number).await.unwrap();
    assert!(stored.replay.is_none());
    assert_eq!(stored.report.words, "another person asked for this too.");
    assert_eq!(s.reports.feedback().await.unwrap()[&Topic::Modelling], vec![listed.number]);

    let two_lines = Report {
        summary: "one\ntwo".to_owned(),
        ..idea
    };
    let Err(Refusal::Bad(why)) = s.reports.send(&anna, two_lines).await else {
        panic!("a summary of two lines is refused");
    };
    assert_eq!(why, "A summary is one line.");

    // Dan cannot see the Architecture team's model, so he can neither draft nor send a report of it.
    assert!(matches!(
        s.reports.draft(&dan, &project, &model, &Ask::default()).await,
        Err(Refusal::Refused(_))
    ));
    let draft = s
        .reports
        .draft(&anna, &project, &model, &Ask { elements: vec!["D-101".to_owned()], ..Ask::default() })
        .await
        .unwrap();
    let theirs = from_draft(draft.report, "Something about a door", "");
    assert!(matches!(s.reports.send(&dan, theirs).await, Err(Refusal::Refused(_))));
    // A field the shape does not name is refused before it is ever read.
    let mut extra = serde_json::to_value(idea_like()).unwrap();
    extra["project"] = Value::String("everything".to_owned());
    assert!(serde_json::from_value::<Report>(extra).is_err());
}

fn idea_like() -> Report {
    Report {
        format: 1,
        kind: Kind::Feedback,
        summary: "x".to_owned(),
        words: String::new(),
        topic: Some(Topic::Other),
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
