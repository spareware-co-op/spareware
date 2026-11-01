//! The stress test (`Calibration.stressTest`) against the JavaScript's recorded answers
//! (expected/calibrate-stress-*.json, from tools/transport-truth/gen-calibrate.mjs): demand times 1.1 and 1.2 or the
//! factors given, the junctions whose saturation rises faster than demand (flagged), the roads over capacity with no
//! junction delay, with the stored base result, a stale one, none, another period and another scenario. Each run
//! assigns with the real Rust assignment.

mod calibrate_support;
mod support;

use calibrate_support::pn;
use calibrate_support::states::run_both;
use construct_transport::assign::assign;
use construct_transport::calibrate::stress_test;
use construct_transport::project::Project;
use construct_transport::results::AssignOpts;
use serde_json::{Value, json};
use support::{assert_close, load_json};

fn po(x: Option<f64>) -> Value {
    x.map_or(Value::Null, pn)
}

#[test]
fn stress_tests() {
    let mut flagged = 0;
    let mut links = 0;
    let mut total = 0;
    for f in [
        "expected/calibrate-stress-corridor.json",
        "expected/calibrate-stress-grid.json",
    ] {
        let all = load_json(f);
        for c in all["cases"].as_array().expect("cases") {
            let name = c["name"].as_str().expect("name");
            let how = &c["how"];
            let mut p =
                Project::from_json(all["projectText"].as_str().expect("text")).expect("project");
            let sc_id = p.scenarios[0].id.clone();
            if !how["noRes"].as_bool().unwrap_or(false) {
                run_both(&mut p, &sc_id);
            }
            if how["stale"].as_bool().unwrap_or(false) {
                let r = p.scenarios[0].res.get_mut().expect("results");
                r["AM"]["stale"] = json!(true);
            }
            let scenario = how["scIndex"].as_u64().unwrap_or(0) as usize;
            if scenario > 0 {
                let sc = p.scenarios[scenario].clone();
                let res = assign(
                    &p,
                    &sc,
                    &p.settings,
                    &AssignOpts {
                        period: Some("AM".to_string()),
                        ..AssignOpts::default()
                    },
                    None,
                );
                calibrate_support::store_res(&mut p.scenarios[scenario], "AM", &res);
            }
            let sc_id = p.scenarios[scenario].id.clone();
            let opts = &c["opts"];
            let period = opts["period"].as_str();
            let factors: Option<Vec<f64>> = opts["factors"]
                .as_array()
                .map(|a| a.iter().map(|x| x.as_f64().expect("factor")).collect());
            let mut prog: Vec<Value> = Vec::new();
            let mut cb = |k: f64, n: f64, m: &str| prog.push(json!([pn(k), pn(n), m]));
            let r = stress_test(&p, &sc_id, period, factors.as_deref(), Some(&mut cb))
                .expect("stress test");
            let label = format!("{f}: {name}");
            assert_close(
                &Value::Array(prog),
                &c["progress"],
                &format!("{label}: progress"),
            );
            let got = json!({
                "scId": r.sc_id, "period": r.period, "factors": r.factors, "top": r.nodes.first().map(|n| n.id.clone()),
                "nodes": r.nodes.iter().map(|n| json!({ "id": n.id, "name": n.name, "dos0": pn(n.dos0), "sens": pn(n.sens), "flag": n.flag,
                    "runs": n.runs.iter().map(|x| json!({ "f": pn(x.f), "dos": po(x.dos), "rise": po(x.rise) })).collect::<Vec<_>>() })).collect::<Vec<_>>(),
                "links": r.links.iter().map(|l| json!({ "key": l.key, "link": l.link, "vc": pn(l.vc), "f": pn(l.f) })).collect::<Vec<_>>(),
            });
            let (mut got, mut want) = (got, c["state"].clone());
            if factors.as_ref().is_some_and(|f| f.is_empty()) {
                // every junction ties (no runs), so the list keeps the order of the base result's junctions; a result kept in
                // the project is held as JSON, whose objects serde_json sorts by key, so only the set of junctions is compared
                for v in [&mut got, &mut want] {
                    v["top"] = Value::Null;
                    v["nodes"]
                        .as_array_mut()
                        .expect("nodes")
                        .sort_by_key(|n| n["id"].as_str().unwrap_or("").to_string());
                }
            }
            assert_close(&got, &want, &label);
            flagged += r.nodes.iter().filter(|n| n.flag).count();
            links += r.links.len();
            total += 1;
        }
    }
    assert_eq!(total, 20);
    assert!(
        flagged >= 2 && links >= 10,
        "{flagged} flagged junctions and {links} roads over capacity across the cases"
    );
}
