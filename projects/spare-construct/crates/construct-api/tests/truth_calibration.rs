//! The calibration commands against the prototype's recorded fits (tools/transport-truth gen-calibrate.mjs): a project with traffic
//! counts of every kind is opened, the matrix is estimated through `transport.calibration.fit`, and the fit's result and the
//! validation report afterwards must equal the JavaScript's.

mod support;

use construct_api::Session;
use serde_json::{Value, json};
use support::{assert_close, load_json};

/// The options of a recorded fit as the command takes them.
fn opts_of(o: &Value) -> Value {
    let mut p = json!({});
    for (js, cmd) in [
        ("period", "period"),
        ("iters", "iters"),
        ("damp", "damp"),
        ("maxFactor", "maxFactor"),
        ("maxStep", "maxStep"),
        ("minVol", "minVol"),
        ("constrain", "constrain"),
        ("weights", "weights"),
        ("freezeGG", "freezeGg"),
    ] {
        if !o[js].is_null() {
            p[cmd] = o[js].clone();
        }
    }
    p
}

#[test]
fn fits_through_the_command_match_the_prototype() {
    let mut checks = 0;
    for file in ["calibrate-grid", "calibrate-corridor"] {
        let rec = load_json(&format!("expected/{file}.json"));
        for case in rec["cases"].as_array().unwrap() {
            // the plain fits: no set-up of roles or groups, no earlier fit, no caller-supplied weights
            let name = case["name"].as_str().unwrap();
            // (the cases that pick counts or groups in the harness, or fit twice, are for the engine's own tests)
            if !case["setup"].is_null()
                || !case["meBefore"].is_null()
                || case["weightKind"] == json!("caller")
                || case["opts"].get("countIds").is_some()
                || ["count-ids", "groups", "caller", "refit"]
                    .iter()
                    .any(|k| name.contains(k))
            {
                continue;
            }
            let text = rec["projectText"]
                .as_str()
                .or_else(|| case["projectText"].as_str())
                .unwrap();
            let mut s = Session::seeded(11, 1_792_540_800_000.0);
            s.run("transport.project.open", &json!({ "text": text }))
                .unwrap_or_else(|e| panic!("{file} {name}: {e}"));
            let sid = case["scenario"].as_str().unwrap();
            let period = case["period"].as_str().unwrap();
            // the other scenarios were run before the fit, as the prototype's harness did (the scenario fitted was not)
            let others: Vec<&str> = case["otherStale"]
                .as_object()
                .map(|o| o.keys().map(String::as_str).collect())
                .unwrap_or_default();
            if !others.is_empty() {
                s.run(
                    "transport.run",
                    &json!({ "scenarios": others, "periods": ["AM", "PM"] }),
                )
                .unwrap();
            }
            let mut p = opts_of(&case["opts"]);
            p["scenario"] = json!(sid);
            let out = s
                .run("transport.calibration.fit", &p)
                .unwrap_or_else(|e| panic!("{file} {name}: {e}"));
            let at = format!("{file}.{name}");
            assert_eq!(
                out["warnings"],
                json!([]),
                "{at}: every result the fit wrote reads back"
            );
            // the fit: statistics before and after, passes, matrix change, the history and the sentence
            let want = &case["result"];
            let got = &out["fit"];
            for k in [
                "before",
                "after",
                "iters",
                "changedPairs",
                "stopped",
                "note",
                "history",
                "matrixChange",
            ] {
                if !want[k].is_null() || !got[k].is_null() {
                    assert_close(&got[k], &want[k], &format!("{at}.fit.{k}"));
                    checks += 1;
                }
            }
            // the result the scenario now holds, and the other scenarios' results gone out of date
            let list = s.run("transport.results.list", &json!({})).unwrap();
            let held = list["results"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["scenario"] == json!(sid) && r["period"] == json!(period));
            match (held, case["scRes"].is_null()) {
                (None, true) => {}
                (Some(r), false) => assert_close(
                    &r["iterations"],
                    &case["scRes"]["it"],
                    &format!("{at}.scRes.it"),
                ),
                (h, _) => panic!(
                    "{at}: the result held {h:?} against the prototype's {}",
                    case["scRes"]
                ),
            }
            for (other, per) in case["otherStale"].as_object().into_iter().flatten() {
                for (pd, stale) in per.as_object().unwrap() {
                    let row = list["results"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|r| r["scenario"] == json!(other) && r["period"] == json!(pd));
                    if let Some(r) = row {
                        assert_eq!(r["stale"], *stale, "{at}: {other} {pd} out of date");
                        checks += 1;
                    }
                }
            }
            // the validation report after the fit
            let rep = s
                .run(
                    "transport.calibration.report",
                    &json!({ "scenario": sid, "period": period }),
                )
                .unwrap();
            let want_rep = &case["report"][period];
            // (when the fit changed nothing the prototype's report is of a run its harness made, which the project does not hold)
            if want_rep.is_object() && !case["scRes"].is_null() {
                assert_close(&rep["report"], want_rep, &format!("{at}.report"));
                checks += 1;
            }
            checks += 2;
        }
    }
    println!("calibration truth: {checks} checks match the prototype");
    assert!(checks >= 40);
}
