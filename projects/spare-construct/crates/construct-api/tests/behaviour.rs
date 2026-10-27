//! How the command layer behaves: structured errors that never panic, the envelope the browser and scripts use, undo of settings and
//! scenarios, saving and reopening with results, progress events, repeatable ids, and a stability sweep over every command.

mod support;

use construct_api::Session;
use construct_api::transport::registry;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::rc::Rc;
use support::{input_text, read_truth};

fn grid() -> Session {
    let mut s = Session::seeded(3, 1_790_000_000_000.0);
    s.run(
        "transport.project.open",
        &json!({ "text": input_text("grid") }),
    )
    .unwrap();
    s
}

fn ok(s: &mut Session, c: &str, p: Value) -> Value {
    s.run(c, &p).unwrap_or_else(|e| panic!("{c} {p}: {e}"))
}

#[test]
fn errors_are_structured() {
    let mut s = grid();
    let e = s.run("transport.nothing.here", &json!({})).unwrap_err();
    assert_eq!(e.code, "unknown_command");
    let e = s
        .run("transport.network.get", &json!({ "kind": "node" }))
        .unwrap_err();
    assert_eq!(
        (e.code, e.message.as_str()),
        ("bad_params", "missing parameter `id`")
    );
    let e = s
        .run(
            "transport.network.get",
            &json!({ "kind": "tree", "id": "x" }),
        )
        .unwrap_err();
    assert!(
        e.message
            .contains("must be one of: node, link, zone, route"),
        "{}",
        e.message
    );
    let e = s
        .run("transport.network.get", &json!({ "kind": "node", "id": 5 }))
        .unwrap_err();
    assert!(e.message.contains("`id` must be text"), "{}", e.message);
    let e = s
        .run(
            "transport.network.get",
            &json!({ "kind": "node", "id": "n", "colour": "red" }),
        )
        .unwrap_err();
    assert!(
        e.message.contains("unknown parameter `colour`"),
        "{}",
        e.message
    );
    let e = s.run("transport.network.get", &json!([1, 2])).unwrap_err();
    assert_eq!(e.code, "bad_params");
    let e = s
        .run(
            "transport.scenario.select",
            &json!({ "scenario": "Nonexistent" }),
        )
        .unwrap_err();
    assert_eq!(e.code, "not_found");
    assert!(
        e.to_value()["details"]["scenarios"].is_array(),
        "the error lists the scenarios"
    );
    // the envelope: one line of JSON, ok or not
    let good: Value = serde_json::from_str(&s.execute("transport.project.info", "")).unwrap();
    assert_eq!(good["ok"], json!(true));
    assert_eq!(good["result"]["name"], json!("Truth test: grid"));
    let bad: Value =
        serde_json::from_str(&s.execute("transport.network.get", "{not json")).unwrap();
    assert_eq!(
        (bad["ok"].clone(), bad["error"]["code"].clone()),
        (json!(false), json!("bad_params"))
    );
    let bad: Value = serde_json::from_str(&s.execute("transport.run", r#"{"assign":5}"#)).unwrap();
    assert_eq!(bad["ok"], json!(false));
}

#[test]
fn no_command_panics_on_empty_or_wrong_parameters() {
    let mut s = grid();
    ok(&mut s, "transport.run", json!({ "all": true }));
    let junk = [
        json!({}),
        json!({ "x": 1 }),
        json!({ "kind": 5, "id": [], "scenario": {}, "period": "XX", "point": "here", "points": 7, "values": 3 }),
    ];
    let mut calls = 0;
    for c in registry().all() {
        // commands that replace or delete the project are left for the other tests
        if [
            "transport.project.new",
            "transport.project.open",
            "transport.scenario.delete",
        ]
        .contains(&c.name)
        {
            continue;
        }
        for p in &junk {
            if let Err(e) = s.run(c.name, p) {
                assert_ne!(e.code, "internal", "{} {p}: {e}", c.name);
            }
            calls += 1;
        }
    }
    assert!(calls > 300);
}

#[test]
fn settings_and_scenarios_undo_in_one_history() {
    let mut s = grid();
    let before = ok(
        &mut s,
        "transport.settings.get",
        json!({ "path": "growth" }),
    );
    ok(
        &mut s,
        "transport.settings.set",
        json!({ "values": { "growth": 4.5, "econ.vot": 20 } }),
    );
    let first = ok(
        &mut s,
        "transport.network.list",
        json!({ "kind": "link", "limit": 1 }),
    )["items"][0]["id"]
        .clone();
    ok(
        &mut s,
        "transport.network.reverse",
        json!({ "link": first }),
    );
    let state = ok(&mut s, "transport.history.state", json!({}));
    assert_eq!(state["undo"], json!(["settings", ""]));
    let u = ok(&mut s, "transport.history.undo", json!({}));
    assert_eq!(u["done"], json!(true));
    let u = ok(&mut s, "transport.history.undo", json!({}));
    assert_eq!(u["label"], json!("settings"));
    assert_eq!(
        ok(
            &mut s,
            "transport.settings.get",
            json!({ "path": "growth" })
        ),
        before
    );
    ok(&mut s, "transport.history.redo", json!({}));
    assert_eq!(
        ok(
            &mut s,
            "transport.settings.get",
            json!({ "path": "growth" })
        ),
        json!(4.5)
    );
    // nothing more to undo or redo is said, not an error
    for _ in 0..3 {
        ok(&mut s, "transport.history.redo", json!({}));
    }
    let r = ok(&mut s, "transport.history.redo", json!({}));
    assert_eq!(r["done"], json!(false));
    assert_eq!(r["message"], json!("Nothing to redo."));
    // a deleted scenario takes its undo steps with it
    ok(
        &mut s,
        "transport.scenario.select",
        json!({ "scenario": "s-dm" }),
    );
    ok(
        &mut s,
        "transport.zone.add",
        json!({ "point": [-36.9, 174.77] }),
    );
    ok(
        &mut s,
        "transport.scenario.select",
        json!({ "scenario": "s-opt" }),
    );
    ok(
        &mut s,
        "transport.scenario.delete",
        json!({ "scenario": "s-dm" }),
    );
    let labels = ok(&mut s, "transport.history.state", json!({}))["undo"]
        .as_array()
        .unwrap()
        .clone();
    assert!(
        !labels
            .iter()
            .any(|l| l.as_str().unwrap().starts_with("new zone")),
        "{labels:?}"
    );
    // the active scenario and base moved to the first one left
    let info = ok(&mut s, "transport.project.info", json!({}));
    assert_eq!(info["scenarios"].as_array().unwrap().len(), 2);
    assert_eq!(info["active"], info["scenarios"][0]["id"]);
}

#[test]
fn a_saved_project_reopens_with_its_results() {
    let mut s = grid();
    ok(
        &mut s,
        "transport.run",
        json!({ "scenario": "s-opt", "periods": ["AM"] }),
    );
    let kpi = ok(
        &mut s,
        "transport.results.summary",
        json!({ "scenario": "s-opt", "period": "AM" }),
    )["kpi"]
        .clone();
    let first = ok(
        &mut s,
        "transport.network.list",
        json!({ "kind": "node", "limit": 1 }),
    )["items"][0]["id"]
        .clone();
    ok(
        &mut s,
        "transport.network.set",
        json!({ "scenario": "s-opt", "kind": "node", "id": first, "values": { "control": "stop" } }),
    );
    // the edit made the result out of date, and it stays so in the file
    let text = ok(&mut s, "transport.project.save", json!({}))["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        !text.contains("\"paths\"") && !text.contains("\"arcKeys\""),
        "saving leaves the paths out"
    );
    let mut t = Session::seeded(4, 1.0);
    ok(&mut t, "transport.project.open", json!({ "text": text }));
    let list = ok(&mut t, "transport.results.list", json!({}));
    assert_eq!(list["results"].as_array().unwrap().len(), 1);
    assert_eq!(list["results"][0]["stale"], json!(true));
    assert_eq!(
        ok(
            &mut t,
            "transport.results.summary",
            json!({ "scenario": "s-opt", "period": "AM" })
        )["kpi"],
        kpi
    );
    // select-link needs the paths, which the file does not hold
    let e = t
        .run("transport.analysis.select_link", &json!({ "link": "x" }))
        .unwrap_err();
    assert_eq!(e.code, "not_found", "the road is checked first");
    let first_link = ok(
        &mut t,
        "transport.network.list",
        json!({ "kind": "link", "limit": 1 }),
    )["items"][0]["id"]
        .clone();
    let e = t
        .run(
            "transport.analysis.select_link",
            &json!({ "link": first_link, "scenario": "s-opt", "period": "AM" }),
        )
        .unwrap_err();
    assert_eq!(e.code, "invalid_state");
    assert!(e.message.contains("no stored paths"), "{}", e.message);
    // running again brings the paths back
    ok(
        &mut t,
        "transport.run",
        json!({ "scenario": "s-opt", "periods": ["AM"] }),
    );
    ok(
        &mut t,
        "transport.analysis.select_link",
        json!({ "link": first_link, "scenario": "s-opt", "period": "AM" }),
    );
}

#[test]
fn progress_events_say_which_run_and_iteration() {
    let mut s = grid();
    let events: Rc<RefCell<Vec<Value>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = events.clone();
    s.set_progress(move |e| sink.borrow_mut().push(e.clone()));
    ok(
        &mut s,
        "transport.run",
        json!({ "scenario": "s-dm", "periods": ["AM", "PM"] }),
    );
    let ev = events.borrow();
    assert_eq!(ev.iter().filter(|e| e["event"] == "run_start").count(), 2);
    assert_eq!(ev.iter().filter(|e| e["event"] == "run_done").count(), 2);
    let p = ev.iter().find(|e| e["event"] == "progress").unwrap();
    assert_eq!(
        (p["name"].clone(), p["period"].clone(), p["runs"].clone()),
        (json!("Do-minimum 2036"), json!("AM"), json!(2))
    );
    assert!(p["iteration"].is_number() && p["gap"].is_number());
}

#[test]
fn the_same_seed_gives_the_same_ids() {
    let run = |seed: u32| {
        let mut s = Session::seeded(seed, 1_790_000_000_000.0);
        s.run(
            "transport.project.open",
            &json!({ "text": input_text("grid") }),
        )
        .unwrap();
        let a = ok(
            &mut s,
            "transport.network.add_node",
            json!({ "point": [-36.9, 174.8] }),
        )["id"]
            .clone();
        let b = ok(
            &mut s,
            "transport.zone.add",
            json!({ "point": [-36.9, 174.8] }),
        )["id"]
            .clone();
        (a, b)
    };
    assert_eq!(run(5), run(5));
    assert_ne!(run(5), run(6));
    // a system session makes ids the app's way: a prefix, the time, a counter and three random characters
    let mut s = Session::system();
    let id = ok(
        &mut s,
        "transport.network.add_node",
        json!({ "point": [-36.9, 174.8] }),
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(id.starts_with('n') && id.len() >= 9, "{id}");
}

#[test]
fn detecting_sites_after_an_import_equals_detecting_while_importing() {
    let answer: Value =
        serde_json::from_str(&read_truth("fixtures/overpass-papakura.json")).unwrap();
    let points = json!([{ "lat": -37.0648282, "lng": 174.9388742, "no": "AT-GSR1" }, { "lat": -37.0647649, "lng": 174.9416887, "id": "AT-GSR2" }]);
    let make = |during: bool| {
        let mut s = Session::seeded(8, 1_790_000_000_000.0);
        ok(
            &mut s,
            "transport.data.set",
            json!({ "controlled": points }),
        );
        ok(
            &mut s,
            "transport.import.osm_roads",
            json!({ "answer": answer, "gateways": false, "detectSites": during }),
        );
        if !during {
            ok(&mut s, "transport.sites.detect", json!({}));
        }
        let sites = ok(&mut s, "transport.sites.list", json!({}));
        assert!(sites["count"].as_u64().unwrap() > 0, "sites found");
        sites["sites"].clone()
    };
    // the ids differ by what each call drew, so the sites are compared by the junctions in them
    let nodes = |v: Value| -> Vec<Vec<String>> {
        let mut n: Vec<Vec<String>> = v
            .as_array()
            .unwrap()
            .iter()
            .map(|x| {
                x["nodes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|y| y.as_str().unwrap().to_string())
                    .collect()
            })
            .collect();
        n.sort();
        n
    };
    assert_eq!(nodes(make(true)), nodes(make(false)));
}

#[test]
fn the_command_list_describes_every_command() {
    let list: Value = serde_json::from_str(&Session::commands_json()).unwrap();
    let all = list.as_array().unwrap();
    assert!(all.len() >= 135);
    for c in all {
        let name = c["name"].as_str().unwrap();
        assert!(
            name.starts_with("transport.") && name.matches('.').count() <= 2,
            "{name}"
        );
        assert!(
            c["description"].as_str().unwrap().len() >= 10,
            "{name} has a description"
        );
        assert!(
            !c["description"].as_str().unwrap().contains('\u{2014}'),
            "{name}: no em-dashes"
        );
        assert_eq!(c["params"]["type"], json!("object"), "{name}");
        assert_eq!(c["params"]["additionalProperties"], json!(false), "{name}");
        assert!(c["changes"].is_boolean());
    }
}
