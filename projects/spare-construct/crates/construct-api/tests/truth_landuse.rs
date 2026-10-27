//! `transport.landuse.queries` and `transport.landuse.apply` against the prototype's recorded land-use builds
//! (tools/transport-truth gen-landuse.mjs): the zones and statistics made from Overpass answers, with the same seeded ids.

mod support;

use construct_api::Session;
use serde_json::{Value, json};
use support::{assert_close, load_json, read_truth};

fn fixture(f: &str) -> Value {
    serde_json::from_str(&read_truth(&format!("fixtures/{f}"))).expect("JSON")
}

/// The elements a recorded case was given, as the answers a host would pass in.
fn answers_of(case: &Value) -> Option<Value> {
    let inp = &case["input"];
    match inp["kind"].as_str()? {
        "object" => Some(json!([fixture(inp["file"].as_str()?)])),
        "inline" => match &inp["value"] {
            Value::Array(a) => Some(json!([{ "elements": a }])),
            v @ Value::Object(_) => Some(json!([v])),
            _ => None,
        },
        "list" => {
            if inp.get("empty").is_some() {
                return Some(json!([{ "elements": [] }]));
            }
            let all = fixture(inp["file"].as_str()?);
            let mut list: Vec<Value> = all["elements"].as_array()?.clone();
            if inp.get("reversed").is_some() {
                list.reverse();
            }
            if let Some(n) = inp.get("doubled").and_then(Value::as_u64) {
                let more: Vec<Value> = list[..n as usize].to_vec();
                list.extend(more);
            }
            match inp.get("filter").and_then(Value::as_str) {
                Some("buildings") => list.retain(|e| {
                    e["tags"]["building"].is_string()
                        || e["tags"]["building"].as_bool() == Some(true)
                }),
                Some("areas") => {
                    list.retain(|e| !(e["tags"]["building"].is_string()) || e["type"] == "node")
                }
                Some("houses") => {
                    list.retain(|e| matches!(e["tags"]["building"].as_str(), Some("yes" | "house")))
                }
                _ => {}
            }
            Some(json!([{ "elements": list }]))
        }
        _ => None,
    }
}

#[test]
fn landuse_apply_makes_the_prototypes_zones() {
    let rec = load_json("expected/landuse-build.json");
    let mut n = 0;
    for case in rec["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        // cases with no study area, or with named roads to name zones after, are for the engine's own tests
        let Some(answers) = answers_of(case) else {
            continue;
        };
        let b = &case["bounds"];
        if b.is_null() || !case["opts"]["links"].as_array().is_some_and(Vec::is_empty) {
            continue;
        }
        let (s_, w, n_, e) = (
            b["s"].as_f64().unwrap(),
            b["w"].as_f64().unwrap(),
            b["n"].as_f64().unwrap(),
            b["e"].as_f64().unwrap(),
        );
        if !(n_ > s_ && e > w) {
            continue;
        }
        let mut s = Session::harness(
            case["seed"].as_u64().unwrap() as u32,
            case["now"].as_f64().unwrap(),
            case["rngSkip"].as_u64().unwrap() as usize,
            case["seq0"].as_u64().unwrap(),
        );
        let mut p = json!({ "answers": answers, "bounds": { "south": s_, "west": w, "north": n_, "east": e } });
        if let Some(c) = case["opts"].get("cell") {
            // the prototype reads a cell given as text as a number
            p["cell"] = json!(match c {
                Value::String(t) => construct_transport::jsnum::string_to_number(t),
                o => o.as_f64().unwrap_or(f64::NAN),
            });
            if p["cell"].is_null() {
                p.as_object_mut().unwrap().remove("cell");
            }
        }
        let out = s
            .run("transport.landuse.apply", &p)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        // the zones the app keeps: what the build made, marked as made from existing land use
        let want: Vec<Value> = case["zones"]
            .as_array()
            .unwrap()
            .iter()
            .map(|z| {
                let mut z = z.clone();
                z["src"] = json!("existing-lu");
                z
            })
            .collect();
        let got: Vec<Value> = s
            .project()
            .active_scenario()
            .unwrap()
            .zones
            .values()
            .map(|z| serde_json::to_value(z).unwrap())
            .collect();
        assert_close(&json!(got), &json!(want), &format!("{name}.zones"));
        assert_close(&out["stats"], &case["stats"], &format!("{name}.stats"));
        n += 1;
    }
    println!("landuse truth: {n} builds match the prototype");
    assert!(n >= 15);
}

#[test]
fn landuse_queries_are_the_prototypes() {
    let rec = load_json("expected/landuse-download.json");
    let mut checked = 0;
    for run in rec["runs"].as_array().unwrap() {
        let b = &run["box"];
        let Some(queries) = run["queries"].as_array() else {
            continue;
        };
        let want: Vec<&str> = queries
            .iter()
            .filter(|q| q["url"].as_str() == Some("https://overpass-api.de/api/interpreter"))
            .filter_map(|q| q["q"].as_str())
            .collect();
        let mut s = Session::seeded(1, 0.0);
        let bounds = json!({ "south": b["s"], "west": b["w"], "north": b["n"], "east": b["e"] });
        let Ok(out) = s.run("transport.landuse.queries", &json!({ "bounds": bounds })) else {
            continue;
        };
        let got: Vec<&str> = out["jobs"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|j| j["query"].as_str())
            .collect();
        // the prototype asks again, to other servers, when one fails: the first server's requests are the queries
        let mut dedup: Vec<&str> = Vec::new();
        for q in &want {
            if !dedup.contains(q) {
                dedup.push(q);
            }
        }
        assert!(got.starts_with(&dedup), "{}: the queries", run["name"]);
        checked += 1;
    }
    println!("landuse queries: {checked} runs compared");
    assert!(checked >= 2);
}
