//! The analysis commands against the JavaScript's recorded answers: capital, economics, difference, with and without, extent test,
//! development distribution, cycling stress, export readiness, model metadata and status, select link, trip ends and the sensitivity pack.

mod support;

use construct_api::Session;
use serde_json::{Value, json};
use support::{PROJECTS, assert_close, input_text, load_expected};

fn open_and_run(name: &str) -> Session {
    let mut s = Session::seeded(11, 1_790_000_000_000.0);
    s.run(
        "transport.project.open",
        &json!({ "text": input_text(name) }),
    )
    .expect("opens");
    s.run("transport.run", &json!({ "all": true }))
        .expect("runs");
    s
}

fn ok(s: &mut Session, cmd: &str, p: Value) -> Value {
    s.run(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
}

#[test]
fn analysis_commands_match_the_recorded_answers() {
    let mut checks = 0;
    for name in PROJECTS {
        let mut s = open_and_run(name);
        let rec = load_expected(name);
        let scenarios = rec["scenarios"].as_object().unwrap();
        for (sid, sr) in scenarios {
            let at = format!("{name}.{sid}");
            let capital = ok(
                &mut s,
                "transport.analysis.capital",
                json!({ "scenario": sid }),
            );
            assert_close(&capital, &sr["capital"], &format!("{at}.capital"));
            let dd = ok(
                &mut s,
                "transport.analysis.dev_distribution",
                json!({ "scenario": sid }),
            );
            assert_close(
                &dd["distribution"],
                &sr["devDistribution"],
                &format!("{at}.devDistribution"),
            );
            let year = ok(&mut s, "transport.scenario.list", json!({}))["scenarios"]
                .as_array()
                .unwrap()
                .iter()
                .find(|x| x["id"] == json!(sid))
                .unwrap()["year"]
                .clone();
            assert_close(&year, &sr["yearOf"], &format!("{at}.yearOf"));
            checks += 3;
            for period in ["AM", "PM"] {
                let pr = &sr["periods"][period];
                let at = format!("{at}.{period}");
                let lts = ok(
                    &mut s,
                    "transport.analysis.lts",
                    json!({ "scenario": sid, "period": period }),
                );
                assert_close(&lts["lts"], &pr["lts"], &format!("{at}.lts"));
                let er = ok(
                    &mut s,
                    "transport.analysis.export_ready",
                    json!({ "scenario": sid, "period": period }),
                );
                assert_close(&er, &pr["exportReady"], &format!("{at}.exportReady"));
                let meta = ok(
                    &mut s,
                    "transport.model.meta",
                    json!({ "scenario": sid, "period": period }),
                );
                assert_close(&meta["meta"], &pr["modelMeta"], &format!("{at}.modelMeta"));
                // the busiest arc, by flow, as gen-expected.mjs picks it
                let key = pr["selectLinkKey"].as_str();
                if let Some(key) = key {
                    let sl = ok(
                        &mut s,
                        "transport.analysis.select_link",
                        json!({ "scenario": sid, "period": period, "key": key, "limit": 100000 }),
                    );
                    let want = &pr["selectLink"];
                    assert_close(
                        &sl["flows"],
                        &want["flows"],
                        &format!("{at}.selectLink.flows"),
                    );
                    assert_close(
                        &sl["totalPcu"],
                        &want["tot"],
                        &format!("{at}.selectLink.tot"),
                    );
                    let ods: Vec<Value> = sl["ods"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|o| json!({ "o": o["from"], "d": o["to"], "q": o["q"] }))
                        .collect();
                    assert_close(&json!(ods), &want["ods"], &format!("{at}.selectLink.ods"));
                    checks += 3;
                }
                // trip ends of each zone: O is trips leaving, D trips arriving
                let te = ok(
                    &mut s,
                    "transport.results.trip_ends",
                    json!({ "scenario": sid }),
                );
                for z in te["zones"].as_array().unwrap() {
                    if let Some(want) = pr["tripEnds"].get(z["id"].as_str().unwrap()) {
                        assert_close(
                            &z[format!("{period}out")],
                            &want["O"],
                            &format!("{at}.tripEnds.{}.O", z["id"]),
                        );
                        assert_close(
                            &z[format!("{period}in")],
                            &want["D"],
                            &format!("{at}.tripEnds.{}.D", z["id"]),
                        );
                        checks += 2;
                    }
                }
                checks += 3;
            }
        }
        for (pair, periods) in rec["pairs"].as_object().unwrap() {
            let (a, b) = pair.split_once('|').unwrap();
            for (period, want) in periods.as_object().unwrap() {
                let at = format!("{name}.{pair}.{period}");
                let args = json!({ "base": a, "scenario": b, "period": period });
                assert_close(
                    &ok(&mut s, "transport.analysis.econ", args.clone())["econ"],
                    &want["econ"],
                    &format!("{at}.econ"),
                );
                assert_close(
                    &ok(&mut s, "transport.analysis.difference", args.clone())["difference"],
                    &want["difference"],
                    &format!("{at}.difference"),
                );
                let ww = json!({ "without": a, "scenario": b, "period": period });
                assert_close(
                    &ok(&mut s, "transport.analysis.with_without", ww.clone())["withWithout"],
                    &want["withWithout"],
                    &format!("{at}.withWithout"),
                );
                assert_close(
                    &ok(&mut s, "transport.analysis.extent_test", ww)["extentTest"],
                    &want["extentTest"],
                    &format!("{at}.extentTest"),
                );
                checks += 4;
            }
        }
        for (period, want) in rec["modelStatus"].as_object().unwrap() {
            assert_close(
                &ok(
                    &mut s,
                    "transport.model.status",
                    json!({ "period": period }),
                ),
                want,
                &format!("{name}.modelStatus.{period}"),
            );
            checks += 1;
        }
        // the sensitivity pack runs the model for every test; it must equal the table the prototype made
        let first = s.project().scenarios[0].id.clone();
        let sens = ok(
            &mut s,
            "transport.analysis.sensitivity",
            json!({ "scenario": first, "junctionNames": false }),
        );
        assert_close(
            &sens["sensitivity"],
            &rec["sensitivity"],
            &format!("{name}.sensitivity"),
        );
        checks += 1;
    }
    println!("analysis truth: {checks} checks match the prototype");
    assert!(checks > 100);
}

#[test]
fn walk_and_cycle_catchments_match_the_prototype() {
    let rec = support::load_json("expected/api-catchment.json");
    let mut s = Session::seeded(11, 1_790_000_000_000.0);
    let mut open = String::new();
    let mut n = 0;
    for case in rec.as_array().unwrap() {
        let name = case["project"].as_str().unwrap();
        if open != name {
            s.run(
                "transport.project.open",
                &json!({ "text": input_text(name) }),
            )
            .unwrap();
            open = name.to_string();
        }
        let out = ok(
            &mut s,
            "transport.analysis.isochrone",
            json!({ "scenario": case["scenario"], "from": case["from"], "mode": case["mode"] }),
        );
        assert_close(
            &out,
            &case["result"],
            &format!(
                "{name}.{}.{}.{}",
                case["scenario"], case["from"], case["mode"]
            ),
        );
        n += 1;
    }
    assert_eq!(n, 30);
}
