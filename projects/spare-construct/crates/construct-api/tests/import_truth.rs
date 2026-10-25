//! The import and export commands against the prototype (tools/transport-truth gen-import.mjs): an OpenStreetMap answer read into a
//! scenario, gateways at the box edge, GeoJSON lines read as roads, junction sites from controlled-intersection points, and the network
//! written as GeoJSON and CSV with and without results. The same seeded ids and the same made-up input files as the JavaScript.

mod support;

use construct_api::Session;
use serde_json::{Value, json};
use std::collections::HashSet;
use support::{diff_json, load_json, read_truth};

fn s_of(v: &Value) -> &str {
    v.as_str().expect("text")
}

fn bounds(v: &Value) -> Value {
    json!({ "south": v[0], "west": v[1], "north": v[2], "east": v[3] })
}

fn call(s: &mut Session, cmd: &str, p: Value) -> Value {
    s.run(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
}

fn active_state(s: &Session) -> Value {
    let mut v = serde_json::to_value(s.project().active_scenario().unwrap()).unwrap();
    if let Value::Object(m) = &mut v {
        m.remove("res");
    }
    v
}

#[test]
fn imports_and_exports_do_what_the_prototype_does() {
    let cases = load_json("expected/import-cases.json");
    let (mut run, mut failures) = (0usize, Vec::<String>::new());
    let mut check = |path: &str, actual: &Value, expected: &Value| {
        run += 1;
        let (d, total) = diff_json(actual, expected, path);
        if total > 0 {
            failures.push(format!(
                "{total} difference(s) under {path}: {:?}",
                d.iter().take(3).collect::<Vec<_>>()
            ));
        }
    };
    for case in cases.as_array().unwrap() {
        let name = s_of(&case["name"]);
        let mut s = Session::harness(
            case["seed"].as_u64().unwrap() as u32,
            case["now"].as_f64().unwrap(),
            case["rngSkip"].as_u64().unwrap() as usize,
            case["seq0"].as_u64().unwrap(),
        );
        let mut mark: HashSet<String> = HashSet::new();
        for (i, (o, step)) in case["ops"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["steps"].as_array().unwrap())
            .enumerate()
        {
            let at = format!("{name}[{i}] {}", s_of(&o["op"]));
            let ret = &step["ret"];
            match s_of(&o["op"]) {
                "newProject" => {
                    let r = call(
                        &mut s,
                        "transport.project.new",
                        json!({ "name": o["name"] }),
                    );
                    check(
                        &format!("{at}.ret"),
                        &json!({ "id": r["id"], "sc": r["scenarios"][0]["id"] }),
                        ret,
                    );
                }
                "openProject" => {
                    let text = read_truth(&format!("inputs/{}.json", s_of(&o["name"])));
                    call(&mut s, "transport.project.open", json!({ "text": text }));
                }
                "selectScenario" => {
                    call(
                        &mut s,
                        "transport.scenario.select",
                        json!({ "scenario": o["id"] }),
                    );
                }
                "data" => {
                    call(&mut s, "transport.data.set", o["set"].clone());
                }
                "mark" => {
                    mark = s
                        .project()
                        .active_scenario()
                        .unwrap()
                        .nodes
                        .keys()
                        .map(str::to_string)
                        .collect()
                }
                "osm" => {
                    let data: Value = serde_json::from_str(&read_truth(&format!(
                        "fixtures/{}",
                        s_of(&o["file"])
                    )))
                    .unwrap();
                    let r = call(
                        &mut s,
                        "transport.import.osm_roads",
                        json!({ "answer": data, "gateways": false, "allScenarios": false, "detectSites": o["sites"] == json!(true) }),
                    );
                    check(&format!("{at}.ret"), &r["segments"], ret);
                }
                "gateways" => {
                    let mut p = json!({ "bounds": bounds(&o["bounds"]) });
                    if o["only"] == json!("fresh") {
                        let fresh: Vec<String> = s
                            .project()
                            .active_scenario()
                            .unwrap()
                            .nodes
                            .keys()
                            .filter(|k| !mark.contains(*k))
                            .map(str::to_string)
                            .collect();
                        p["nodes"] = json!(fresh);
                    }
                    let r = call(&mut s, "transport.import.auto_gateways", p);
                    check(&format!("{at}.ret"), &r["gateways"], ret);
                }
                "geojson" => {
                    let gj = read_truth(&format!("fixtures/{}", s_of(&o["file"])));
                    let t = &o["template"];
                    let (ab, ba) = (
                        t["lanesAB"].as_f64().unwrap(),
                        t["lanesBA"].as_f64().unwrap(),
                    );
                    assert!(
                        ab == ba || ba == 0.0,
                        "{at}: the template's lanes are symmetric or one-way"
                    );
                    // the template's other keys are copied onto each road
                    let mut extra = t.as_object().unwrap().clone();
                    for k in ["cls", "lanesAB", "lanesBA", "speed", "status"] {
                        extra.remove(k);
                    }
                    let r = call(
                        &mut s,
                        "transport.import.geojson",
                        json!({ "geojson": gj, "cls": t["cls"], "lanes": ab, "oneway": ba == 0.0, "speed": t["speed"], "status": t["status"], "props": extra }),
                    );
                    check(&format!("{at}.ret"), &r["roads"], ret);
                }
                "export" => {
                    let (geojson, csv) = if let Some(period) = o["period"].as_str() {
                        call(&mut s, "transport.run", json!({ "periods": [period] }));
                        (
                            call(
                                &mut s,
                                "transport.export.geojson",
                                json!({ "period": period }),
                            ),
                            call(
                                &mut s,
                                "transport.export.links_csv",
                                json!({ "period": period }),
                            ),
                        )
                    } else {
                        (
                            call(&mut s, "transport.export.geojson", json!({})),
                            call(&mut s, "transport.export.links_csv", json!({})),
                        )
                    };
                    check(&format!("{at}.geojson"), &geojson["text"], &ret["geojson"]);
                    check(&format!("{at}.csv"), &csv["text"], &ret["csv"]);
                }
                other => panic!("unknown op {other}"),
            }
            if let Some(state) = step.get("state").filter(|x| !x.is_null()) {
                // the project-level steps record only some parts of the state
                if !state["sc"].is_null() {
                    check(&format!("{at}.sc"), &active_state(&s), &state["sc"]);
                }
                if !state["active"].is_null() {
                    check(
                        &format!("{at}.active"),
                        &json!(s.project().active.as_str()),
                        &state["active"],
                    );
                }
                if !state["scenarios"].is_null() {
                    let ids: Vec<&str> = s
                        .project()
                        .scenarios
                        .iter()
                        .map(|x| x.id.as_str())
                        .collect();
                    check(&format!("{at}.scenarios"), &json!(ids), &state["scenarios"]);
                }
            }
        }
    }
    println!(
        "import truth: {} of {} checks match the prototype",
        run - failures.len(),
        run
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
