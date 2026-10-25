//! Screenline suggestions from OpenStreetMap against the JavaScript's recorded answers
//! (expected/calibrate-suggest-*.json, from tools/transport-truth/gen-calibrate.mjs): joining ways that share end
//! points into lines in every order and direction, grouping rivers, canals and main railways (sidings and yards left
//! out), counting the roads each line crosses, adding a suggestion to the project, and the answers when there is
//! nothing to suggest. The network fetch is replaced by a made-up Overpass answer in both.

mod calibrate_support;
mod support;

use construct_transport::calibrate::{CalibStore, Suggestion, add_suggested, join_ways, suggest_screenlines};
use construct_transport::edit::IdSource;
use construct_transport::project::Project;
use serde_json::{Value, json};
use support::{assert_close, load_json};

const FILES: [&str; 2] = ["expected/calibrate-suggest-corridor.json", "expected/calibrate-suggest-grid.json"];

struct Queue(std::collections::VecDeque<String>);

impl IdSource for Queue {
    fn uid(&mut self, prefix: &str) -> String {
        let id = self.0.pop_front().expect("an id left");
        assert!(id.starts_with(prefix), "{id} does not start with {prefix}");
        id
    }
    fn now_ms(&self) -> f64 {
        1_792_540_800_000.0
    }
}

fn suggestion_out(s: &Suggestion) -> Value {
    json!({ "name": s.name, "kind": s.kind, "pts": s.pts, "osm": s.osm, "crossings": s.crossings, "few": s.few, "added": s.added })
}

fn step<'a>(all: &'a Value, name: &str) -> &'a Value {
    all["steps"].as_array().expect("steps").iter().find(|s| s["name"] == name).unwrap_or_else(|| panic!("no step {name}"))
}

/// The recorded list without the fetch cache flag (the port does not fetch).
fn want_list(r: &Value) -> Value {
    json!({ "ok": r["ok"], "note": r["note"], "list": r["list"] })
}

#[test]
fn joining_ways() {
    let all = load_json(FILES[0]);
    let cases = all["joinWays"].as_array().expect("joinWays");
    assert_eq!(cases.len(), 17);
    for c in cases {
        let ways: Vec<&Value> = c["ways"].as_array().expect("ways").iter().collect();
        let got: Vec<Value> = join_ways(&ways).iter().map(|j| json!({ "pts": j.pts, "ids": j.ids })).collect();
        assert_close(&Value::Array(got), &c["out"], &format!("joinWays: {}", c["name"]));
    }
}

#[test]
fn suggesting_from_an_overpass_answer() {
    for f in FILES {
        let all = load_json(f);
        let p = Project::from_json(all["projectText"].as_str().expect("text")).expect("project");
        let sc = &p.scenarios[0];
        let reply = json!({ "elements": all["elements"] });
        for name in ["first", "cached"] {
            let r = suggest_screenlines(sc, &CalibStore::read(&p), &reply);
            let got = json!({ "ok": r.ok, "note": r.note, "list": r.list.iter().map(suggestion_out).collect::<Vec<_>>() });
            assert_close(&got, &want_list(&step(&all, name)["r"]), &format!("{f}: {name}"));
        }
        assert!(step(&all, "first")["r"]["list"].as_array().expect("list").len() >= 4, "{f}: a list with several lines");
        // nothing to suggest
        let none = suggest_screenlines(sc, &CalibStore::read(&p), &json!({ "elements": [] }));
        let want = &step(&all, "no features");
        assert_close(&json!({ "ok": none.ok, "note": none.note, "list": none.list.len() }), &json!({ "ok": want["r"]["ok"], "note": want["r"]["note"], "list": 0 }), "no features");
        let none = suggest_screenlines(sc, &CalibStore::read(&p), &json!({}));
        assert_close(&json!({ "ok": none.ok, "note": none.note, "list": none.list.len() }), &json!({ "ok": want["noElements"]["ok"], "note": want["noElements"]["note"], "list": 0 }), "no elements");
        // one node is not a network
        let mut small = sc.clone();
        let keep: Vec<String> = small.nodes.keys().take(1).map(String::from).collect();
        small.nodes.retain(|k, _| keep.iter().any(|x| x == k));
        let r = suggest_screenlines(&small, &CalibStore::read(&p), &reply);
        let want = &step(&all, "no roads")["r"];
        assert_close(&json!({ "ok": r.ok, "note": r.note, "list": r.list.len() }), &json!({ "ok": want["ok"], "note": want["note"], "list": 0 }), "no roads");
    }
}

#[test]
fn adding_suggestions() {
    for f in FILES {
        let all = load_json(f);
        let mut p = Project::from_json(all["projectText"].as_str().expect("text")).expect("project");
        let reply = json!({ "elements": all["elements"] });
        let first = suggest_screenlines(&p.scenarios[0], &CalibStore::read(&p), &reply);
        let long = Suggestion {
            name: format!("{}end", "x".repeat(100)),
            kind: "river".to_string(),
            pts: vec![[-37.0, 174.0], [-37.1, 174.1]],
            osm: vec![json!(1), json!(2), json!(3)],
            crossings: 0,
            few: false,
            added: false,
        };
        let one_point = Suggestion { name: "short".to_string(), kind: "canal".to_string(), pts: vec![[-37.0, 174.0]], osm: Vec::new(), crossings: 0, few: false, added: false };
        let mut items = vec![first.list[0].clone(), first.list[first.list.len() - 1].clone(), long, one_point];
        let rec = step(&all, "added");
        let mut ids = Queue(all["ids"].as_array().expect("ids").iter().map(|x| x.as_str().expect("id").to_string()).collect());
        for (k, item) in items.iter_mut().enumerate() {
            let sl = add_suggested(&mut p, item, if k == 1 { "me" } else { "valid" }, &mut ids);
            let want = &rec["got"][k];
            assert_close(&sl.map_or(Value::Null, |s| serde_json::to_value(s).expect("screenline")), &want["sl"], &format!("{f}: add {k}"));
            assert_eq!(item.added, want["added"].as_bool().unwrap_or(false), "{f}: added flag {k}");
        }
        assert!(ids.0.is_empty(), "{f}: every recorded id was used");
        assert_close(&serde_json::to_value(CalibStore::read(&p).screenlines).expect("store"), &rec["store"], &format!("{f}: the screenlines"));
        // suggesting again marks the lines already added
        let again = suggest_screenlines(&p.scenarios[0], &CalibStore::read(&p), &reply);
        let got = json!({ "ok": again.ok, "note": again.note, "list": again.list.iter().map(suggestion_out).collect::<Vec<_>>() });
        assert_close(&got, &want_list(&rec["again"]), &format!("{f}: again"));
    }
}
