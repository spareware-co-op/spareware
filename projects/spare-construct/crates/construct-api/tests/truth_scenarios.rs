//! The assessment-set commands against the prototype (tools/transport-truth gen-edit.mjs, edit-scenarios.json): the scenario's
//! description and checks, the assessment set the wizard makes (several pathways and options), and the table and year groups.
//!
//! The recorded project (built in the JavaScript by a long list of engine calls, with a register and committed developments) is
//! opened from its recorded state; the seeded id generator is put where the JavaScript's stood, so the ids made must match.

mod support;

use construct_api::Session;
use serde_json::{Value, json};
use support::{assert_close, load_json};

fn call(s: &mut Session, cmd: &str, p: Value) -> Value {
    s.run(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
}

#[test]
fn assessment_set_commands_match_the_prototype() {
    let rec = load_json("expected/edit-scenarios.json");
    let ops = rec["ops"].as_array().unwrap();
    let steps = rec["steps"].as_array().unwrap();
    let at87 = ops
        .iter()
        .position(|o| o["op"] == json!("createSet"))
        .expect("a createSet");
    // the id counter and the generator stood 31 draws on from where loading left them
    let seq = rec["seq0"].as_u64().unwrap() + 31;
    let skip = rec["rngSkip"].as_u64().unwrap() as usize + 31;
    let mut s = Session::harness(
        rec["seed"].as_u64().unwrap() as u32,
        rec["now"].as_f64().unwrap(),
        skip,
        seq,
    );
    call(
        &mut s,
        "transport.settings.set",
        json!({ "values": { "baseYear": 2026, "year": 2026, "growth": 1.5, "growthMode": "linear" } }),
    );
    // the project the JavaScript had just before the wizard
    let last_p = (0..at87)
        .rev()
        .find(|i| steps[*i]["state"]["P"].is_object())
        .unwrap();
    let recorded = &steps[last_p]["state"]["P"];
    let mut text: Value = serde_json::from_str(
        call(&mut s, "transport.project.save", json!({}))["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    for k in ["scenarios", "register", "active", "base"] {
        text[k] = recorded[k].clone();
    }
    call(
        &mut s,
        "transport.project.open",
        json!({ "text": text.to_string() }),
    );
    let mut checks = 0;
    for (i, o) in ops.iter().enumerate().skip(at87 - 2) {
        // the sensitivity copies and the filtered copies that follow call the engine directly
        if matches!(
            o["op"].as_str(),
            Some("sensitivityOf" | "copy" | "setScenario")
        ) {
            break;
        }
        let at = format!("edit-scenarios[{i}] {}", o["op"].as_str().unwrap());
        let ret = &steps[i]["ret"];
        match o["op"].as_str().unwrap() {
            "describe" => assert_close(
                &call(&mut s, "transport.scenario.describe", json!({})),
                ret,
                &at,
            ),
            "check" => assert_close(
                &call(&mut s, "transport.scenario.check", json!({}))["findings"],
                ret,
                &at,
            ),
            "createSet" => {
                let opts = &o["opts"];
                let mut p = json!({});
                for (k, v) in [
                    ("type", "pathway"),
                    ("countYear", "countYear"),
                    ("opening", "opening"),
                    ("design", "design"),
                    ("mitigation", "mitigation"),
                    ("periods", "periods"),
                    ("buildout", "buildout"),
                    ("from", "from"),
                ] {
                    if !opts[k].is_null() {
                        p[v] = opts[k].clone();
                    }
                }
                // an unknown pathway reads as the default one
                if p["pathway"] == json!("nonsense") {
                    p.as_object_mut().unwrap().remove("pathway");
                }
                let r = call(&mut s, "transport.scenario.create_set", p);
                let ids: Vec<Value> = r["scenarios"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x["id"].clone())
                    .collect();
                assert_close(&json!(ids), &ret["scenarios"], &format!("{at}.scenarios"));
                assert_close(&r["excluded"], &ret["excluded"], &format!("{at}.excluded"));
                assert_close(&r["warnings"], &ret["warnings"], &format!("{at}.warnings"));
                assert_close(&r["set"], &ret["set"], &format!("{at}.set"));
                // the scenarios made, as the JavaScript recorded the project after the step
                let want = &steps[i]["state"]["P"]["scenarios"];
                let got: Vec<Value> = s
                    .project()
                    .scenarios
                    .iter()
                    .map(|x| serde_json::to_value(x).unwrap())
                    .collect();
                let strip = |v: &Value| {
                    let mut v = v.clone();
                    if let Some(a) = v.as_array_mut() {
                        for x in a {
                            if let Some(m) = x.as_object_mut() {
                                m.remove("res");
                            }
                        }
                    }
                    v
                };
                assert_close(&strip(&json!(got)), &strip(want), &format!("{at}.project"));
                checks += 4;
            }
            "table" => {
                assert_close(
                    &call(&mut s, "transport.scenario.table", json!({}))["table"],
                    ret,
                    &at,
                );
                checks += 1;
            }
            "groups" => {
                assert_close(
                    &call(&mut s, "transport.scenario.table", json!({}))["groups"],
                    ret,
                    &at,
                );
                checks += 1;
            }
            // the rest of the recorded script calls the engine directly
            _ => {}
        }
    }
    println!("scenario truth: {checks} checks match the prototype");
    assert!(checks >= 15);
}
