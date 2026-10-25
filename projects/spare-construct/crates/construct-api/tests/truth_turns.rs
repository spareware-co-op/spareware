//! The turning-movement commands against the prototype's recorded answers (tools/transport-truth gen-turns.mjs): movement rows, junction
//! names, turning-count sheets, junction diagrams and the SIDRA export, for each scenario and period of the truth projects.

mod support;

use construct_api::Session;
use serde_json::{Value, json};
use support::{PROJECTS, assert_close, input_text, load_json};

fn call(s: &mut Session, cmd: &str, p: Value) -> Value {
    s.run(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
}

#[test]
fn turning_commands_match_the_recorded_answers() {
    let mut checks = 0;
    for name in PROJECTS {
        let rec = load_json(&format!("expected/turns-{name}.json"));
        let mut s = Session::seeded(11, 1_790_000_000_000.0);
        call(
            &mut s,
            "transport.project.open",
            json!({ "text": input_text(name) }),
        );
        call(&mut s, "transport.run", json!({ "all": true }));
        for case in rec["cases"].as_array().unwrap() {
            let (sid, period) = (
                case["scenario"].as_str().unwrap(),
                case["period"].as_str().unwrap(),
            );
            let periods: Vec<&str> = if period == "both" {
                vec!["AM", "PM"]
            } else {
                vec![period]
            };
            for (node, n) in case["nodes"].as_object().into_iter().flatten() {
                if n["has"] != json!(true) {
                    continue;
                }
                let at = format!("{name}.{sid}.{period}.{node}");
                let m = call(
                    &mut s,
                    "transport.turns.movements",
                    json!({ "scenario": sid, "period": period, "id": node }),
                );
                assert_close(&m["rows"], &n["rows"], &format!("{at}.rows"));
                assert_close(&m["name"], &n["name"], &format!("{at}.name"));
                let c = call(
                    &mut s,
                    "transport.turns.count_csv",
                    json!({ "scenario": sid, "period": period, "node": node }),
                );
                assert_close(&c["text"], &n["count"], &format!("{at}.count"));
                let d = call(
                    &mut s,
                    "transport.turns.diagram",
                    json!({ "scenario": sid, "period": period, "node": node }),
                );
                assert_close(&d["svg"], &n["diagram"]["total"], &format!("{at}.diagram"));
                let small = call(
                    &mut s,
                    "transport.turns.diagram",
                    json!({ "scenario": sid, "period": period, "node": node, "size": 100 }),
                );
                assert_close(
                    &small["svg"],
                    &n["diagram"]["small"],
                    &format!("{at}.diagram.small"),
                );
                checks += 5;
            }
            for (key, params) in [
                ("default", json!({})),
                ("codes", json!({ "codes": true })),
                ("all", json!({ "codes": true, "all": true })),
            ] {
                if case["csv"].get(key).is_none() {
                    continue;
                }
                let mut p = json!({ "scenario": sid, "periods": periods, "force": true });
                for (k, v) in params.as_object().unwrap() {
                    p[k] = v.clone();
                }
                let out = call(&mut s, "transport.turns.sidra_csv", p);
                assert_close(
                    &out["text"],
                    &case["csv"][key],
                    &format!("{name}.{sid}.{period}.sidra.{key}"),
                );
                checks += 1;
            }
        }
    }
    println!("turns truth: {checks} checks match the prototype");
    assert!(checks > 100);
}
