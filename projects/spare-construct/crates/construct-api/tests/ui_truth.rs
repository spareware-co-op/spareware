//! The commands against what the prototype's interface does for the same action (tools/transport-truth gen-api-ui.mjs).
//!
//! The JavaScript opened a truth-test project and did each step through the interface's own code (drawing a road, the zone and gateway
//! tools, deleting, changing a field, copying a scenario, undo and redo), recording after each step the active scenario, the ids it
//! made, the undo and redo labels and which results were out of date. The command layer replays the same steps with the same seed
//! and clock, so every id must match, and the project after each step must equal the JavaScript's.

mod support;

use construct_api::Session;
use serde_json::{Value, json};
use support::{diff_json, input_text, load_json};

struct Tally {
    run: usize,
    failures: Vec<String>,
}

impl Tally {
    fn check(&mut self, path: &str, actual: &Value, expected: &Value) {
        self.run += 1;
        let (diffs, total) = diff_json(actual, expected, path);
        if total > 0 {
            let mut m = format!("{total} difference(s) under {path}:");
            for d in diffs.iter().take(6) {
                m.push_str(&format!(
                    "\n  {}\n      expected {}\n      actual   {}",
                    d.path, d.expected, d.actual
                ));
            }
            self.failures.push(m);
        }
    }
}

fn s_of(v: &Value) -> &str {
    v.as_str().expect("text")
}

fn latlng(st: &Value) -> Value {
    json!([st["lat"], st["lng"]])
}

/// The command or commands that do what the interface did for one recorded step, and whether the interface refused it.
fn replay(s: &mut Session, st: &Value) -> Result<Value, String> {
    let call =
        |s: &mut Session, name: &str, p: Value| s.run(name, &p).map_err(|e| format!("{name}: {e}"));
    match s_of(&st["do"]) {
        "run" => call(s, "transport.run", json!({ "periods": st["periods"] })),
        "addRoad" => {
            let t = &st["template"];
            let mut p = json!({ "points": st["points"] });
            for k in ["cls", "lanes", "oneway", "speed", "status"] {
                if !t[k].is_null() {
                    p[k] = t[k].clone();
                }
            }
            call(s, "transport.network.add_road", p)
        }
        "addBus" => call(s, "transport.route.add", json!({ "stops": st["stops"] })),
        "addZone" => call(s, "transport.zone.add", json!({ "point": latlng(st) })),
        "addGateway" => call(
            s,
            "transport.zone.add_gateway",
            json!({ "point": latlng(st) }),
        ),
        "delete" => call(
            s,
            "transport.network.delete",
            json!({ "kind": st["type"], "id": st["id"] }),
        ),
        "field" => {
            let values = json!({ st["k"].as_str().unwrap(): st["v"] });
            match s_of(&st["kind"]) {
                "link" | "node" => call(
                    s,
                    "transport.network.set",
                    json!({ "kind": st["kind"], "id": st["id"], "values": values }),
                ),
                "zone" => call(
                    s,
                    "transport.zone.set",
                    json!({ "id": st["id"], "values": values }),
                ),
                "route" => call(
                    s,
                    "transport.route.set",
                    json!({ "id": st["id"], "values": values }),
                ),
                _ => call(s, "transport.scenario.set", json!({ "values": values })),
            }
        }
        "reverse" => call(s, "transport.network.reverse", json!({ "link": st["id"] })),
        "move" => call(
            s,
            "transport.network.move",
            json!({ "kind": st["kind"], "id": st["id"], "point": latlng(st) }),
        ),
        "copy" => call(s, "transport.scenario.copy", json!({})),
        "newScenario" => call(s, "transport.scenario.add", json!({})),
        "setYear" => call(
            s,
            "transport.scenario.set_year",
            json!({ "year": st["year"] }),
        ),
        "select" => call(
            s,
            "transport.scenario.select",
            json!({ "scenario": st["id"] }),
        ),
        "setBase" => call(s, "transport.scenario.set_base", json!({})),
        "undo" => call(
            s,
            if st["redo"] == json!(true) {
                "transport.history.redo"
            } else {
                "transport.history.undo"
            },
            json!({}),
        ),
        other => panic!("unknown step {other}"),
    }
}

#[test]
fn every_edit_does_what_the_interface_does() {
    let cases = load_json("expected/api-ui.json");
    let mut t = Tally {
        run: 0,
        failures: Vec::new(),
    };
    let mut steps = 0;
    for case in cases.as_array().expect("cases") {
        let name = s_of(&case["name"]);
        let mut s = Session::harness(
            case["seed"].as_u64().unwrap() as u32,
            case["now"].as_f64().unwrap(),
            case["rngSkip"].as_u64().unwrap() as usize,
            case["seq0"].as_u64().unwrap(),
        );
        s.run(
            "transport.project.open",
            &json!({ "text": input_text(s_of(&case["project"])) }),
        )
        .expect("opens");
        for (i, rec) in case["steps"].as_array().unwrap().iter().enumerate() {
            let st = &rec["step"];
            let at = format!("{name}[{i}] {}", s_of(&st["do"]));
            let before: Vec<String> = s
                .project()
                .active_scenario()
                .map(|sc| sc.nodes.keys().map(str::to_string).collect())
                .unwrap_or_default();
            let before_links: Vec<String> = s
                .project()
                .active_scenario()
                .map(|sc| sc.links.keys().map(str::to_string).collect())
                .unwrap_or_default();
            let out = replay(&mut s, st);
            steps += 1;
            let rejected = rec["result"]["rejected"] == json!(true);
            match (&out, rejected) {
                (Err(e), false) => {
                    t.failures.push(format!("{at}: the command failed: {e}"));
                    continue;
                }
                (Ok(_), true) => {
                    t.failures.push(format!(
                        "{at}: the interface refused this change but the command made it"
                    ));
                    continue;
                }
                _ => {}
            }
            // the ids the step made
            if let Ok(r) = &out {
                let want = &rec["result"];
                let sc = s.project().active_scenario().unwrap();
                match s_of(&st["do"]) {
                    "addRoad" => {
                        // the interface lists every road the step added, the splits of existing roads included; the command lists the roads drawn
                        let new_links: Vec<String> = sc
                            .links
                            .keys()
                            .filter(|k| !before_links.iter().any(|b| b == k))
                            .map(str::to_string)
                            .collect();
                        t.check(&format!("{at}.links"), &json!(new_links), &want["links"]);
                        let drawn = r["links"].as_array().unwrap();
                        assert!(
                            drawn
                                .iter()
                                .all(|d| new_links.iter().any(|n| Some(n.as_str()) == d.as_str())),
                            "{at}: drawn roads are new roads"
                        );
                        let new_nodes: Vec<String> = sc
                            .nodes
                            .keys()
                            .filter(|k| !before.iter().any(|b| b == k))
                            .map(str::to_string)
                            .collect();
                        t.check(&format!("{at}.nodes"), &json!(new_nodes), &want["nodes"]);
                    }
                    "addBus" => t.check(&format!("{at}.id"), &json!([r["id"]]), &want["routes"]),
                    "addZone" | "addGateway" => {
                        t.check(&format!("{at}.id"), &json!([r["id"]]), &want["zones"])
                    }
                    "copy" | "newScenario" => t.check(&format!("{at}.id"), &r["id"], &want["id"]),
                    _ => {}
                }
            }
            // the project after the step
            let p = s.project();
            t.check(
                &format!("{at}.active"),
                &json!(p.active.as_str()),
                &rec["active"],
            );
            t.check(&format!("{at}.base"), &json!(p.base.as_str()), &rec["base"]);
            let list: Vec<Value> = p
                .scenarios
                .iter()
                .map(|x| {
                    json!({
                        "id": x.id, "name": x.name.as_str(), "year": x.year.get().map(|y| y.to_number()),
                        "nodes": x.nodes.len(), "links": x.links.len(), "zones": x.zones.len(), "routes": x.routes.len(),
                    })
                })
                .collect();
            t.check(&format!("{at}.scenarios"), &json!(list), &rec["scenarios"]);
            let mut sc = serde_json::to_value(p.active_scenario().unwrap()).unwrap();
            if let Value::Object(m) = &mut sc {
                m.remove("res");
            }
            t.check(&format!("{at}.scenario"), &sc, &rec["scenario"]);
            let (u, r) = s.step_labels();
            t.check(&format!("{at}.undo"), &json!(u), &rec["undo"]);
            t.check(&format!("{at}.redo"), &json!(r), &rec["redo"]);
            let stale: Value = Value::Object(
                s.project()
                    .scenarios
                    .iter()
                    .map(|x| {
                        let per: serde_json::Map<String, Value> = ["AM", "PM"]
                            .iter()
                            .filter_map(|per| {
                                s.result(&x.id, per)
                                    .map(|r| (per.to_string(), json!(r.stale)))
                            })
                            .collect();
                        (x.id.clone(), Value::Object(per))
                    })
                    .collect(),
            );
            t.check(&format!("{at}.stale"), &stale, &rec["stale"]);
        }
    }
    println!(
        "ui truth: {} of {} checks match the prototype's interface over {steps} steps",
        t.run - t.failures.len(),
        t.run
    );
    assert!(t.failures.is_empty(), "{}", t.failures.join("\n"));
}
