//! Study area truth tests: `estimate`, the crash window and `DataHub.loadAll` (the count and crash downloads) against the prototype with the network stubbed
//! (tools/transport-truth/gen-studyarea.mjs): the requests, progress messages, answers and what is left in the project, and the words for every failure.

mod census_support;
mod edit_support;
mod studyarea_support;
mod support;

use census_support::Replay;
use construct_transport::jsnum::{json_number, value_to_number};
use construct_transport::project::Project;
use construct_transport::studyarea::{LoadOpts, cas_window, estimate, load_all};
use edit_support::{Checker, load_json};
use serde_json::{Value, json};
use studyarea_support::{NOW, corners, exchanges, fixed_ids, project};

#[test]
fn the_size_of_a_study_area_is_judged_as_the_prototype_does() {
    let ex = load_json("expected/studyarea-estimate.json");
    let mut ck = Checker::default();
    ck.same(
        "warning limit",
        &construct_transport::studyarea::WARN_KM2,
        &ex["constants"]["WARN_KM2"].as_f64().unwrap(),
    );
    ck.same(
        "refusal limit",
        &construct_transport::studyarea::MAX_KM2,
        &ex["constants"]["MAX_KM2"].as_f64().unwrap(),
    );
    for (i, c) in ex["cases"].as_array().expect("cases").iter().enumerate() {
        let e = estimate(&corners(&c["bounds"]));
        ck.check(
            &format!("estimate[{i}] {}", c["bounds"]),
            &e.to_value(),
            &c["out"],
        );
    }
    ck.finish("study area estimates");
}

#[test]
fn the_crash_window_follows_new_zealands_calendar() {
    let ex = load_json("expected/studyarea-load.json");
    let mut ck = Checker::default();
    for (i, c) in ex["casWindow"]
        .as_array()
        .expect("cases")
        .iter()
        .enumerate()
    {
        let w = cas_window(c["now"].as_f64().unwrap(), c["years"].as_f64());
        let out = &c["out"];
        let got = json!({
            "from": w.from, "to": w.to, "ytd": w.ytd, "years": w.years, "lagYear": w.lag_year, "ytdLabel": "YTD, incomplete",
            "lagNote": if let Some(y) = w.lag_year { json!(format!("{y} may be incomplete (CAS entry lag up to 3 months, MBCM)")) } else { json!("") },
            "label": w.label, "where": w.where_,
        });
        ck.check(
            &format!("casWindow[{i}] {} years {}", c["now"], c["years"]),
            &got,
            out,
        );
    }
    ck.finish("crash windows");
}

// What the case did to the project before the download (named in the case)
fn pre(p: &mut Project, name: Option<&str>) {
    if name == Some("counts with switches already set") {
        let d = serde_json::from_value(json!({
            "counts": [
                { "id": "nzta:01N00610", "src": "nzta", "name": "old", "off": true, "lat": 0, "lng": 0 },
                { "id": "at:5001", "src": "at", "name": "old", "off": false },
                { "id": "user:1", "src": "user", "name": "Mine", "lat": -37.065, "lng": 174.94, "am": 100 }
            ],
            "loaded": { "user": { "at": 5, "n": 1 } }
        }))
        .expect("data reads");
        p.data = construct_transport::jsnum::Maybe::Set(d);
    }
}

#[test]
fn counts_and_crashes_download_and_fail_as_the_prototype_does() {
    let ex = load_json("expected/studyarea-load.json");
    let table = exchanges();
    let mut ck = Checker::default();
    for case in ex["loadAll"].as_array().expect("cases") {
        let name = case["name"].as_str().unwrap();
        let mut p = project("studyarea-project-one");
        pre(&mut p, case["pre"].as_str());
        let mut net = Replay::new(&case["log"], &table);
        let ids = fixed_ids();
        let b = corners(&case["bounds"]);
        let which: Vec<&str> = case["which"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| w.as_str().unwrap())
            .collect();
        let opts = LoadOpts {
            cas_years: case["opts"].get("casYears").map(value_to_number),
            timeout_ms: case["opts"].get("timeout").map(value_to_number),
        };
        let mut status: Vec<String> = Vec::new();
        for (i, r) in case["res"].as_array().unwrap().iter().enumerate() {
            let label = format!("{name} (call {i})");
            let before = net.used();
            let smark = status.len();
            let got = load_all(
                &mut p,
                &ids,
                &mut net,
                &b,
                &which,
                &mut |t| status.push(t.to_string()),
                &opts,
            );
            let v = match got {
                Ok(m) => json!({ "msgs": m, "error": null }),
                Err(e) => json!({ "msgs": null, "error": e }),
            };
            ck.check(
                &format!("{label}: answer"),
                &v,
                &json!({ "msgs": r["msgs"], "error": r["error"] }),
            );
            ck.same(
                &format!("{label}: requests"),
                &vec![before, net.used()],
                &vec![
                    r["requests"][0].as_u64().unwrap() as usize,
                    r["requests"][1].as_u64().unwrap() as usize,
                ],
            );
            let (s0, s1) = (
                r["status"][0].as_u64().unwrap() as usize,
                r["status"][1].as_u64().unwrap() as usize,
            );
            ck.check(
                &format!("{label}: messages"),
                &json!(status[smark..]),
                &json!(case["status"].as_array().unwrap()[s0..s1]),
            );
        }
        ck.same(&format!("{name}: every request made"), &net.done(), &true);
        let data = serde_json::to_value(&p.data).unwrap();
        if case.get("atAnswers") == Some(&json!(true)) {
            let counts: Vec<Value> = data["counts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|k| json!({ "id": k["id"], "ref": k["ref"], "name": k["name"], "date": k["date"], "note": k["note"], "peakTime": k["peakTime"] }))
                .collect();
            ck.check(
                &format!("{name}: counts"),
                &json!(counts),
                &case["data"]["counts"],
            );
        } else if case.get("atAnswers").is_some() {
            let counts: Vec<Value> = data["counts"].as_array().unwrap().iter().map(|k| json!({ "id": k["id"], "ref": k["ref"], "name": k["name"], "date": k["date"], "note": k["note"], "peakTime": k["peakTime"] })).collect();
            ck.check(
                &format!("{name}: counts"),
                &json!(counts),
                &case["data"]["counts"],
            );
        } else {
            ck.check(&format!("{name}: project data"), &data, &case["data"]);
        }
    }
    ck.finish("count and crash downloads");
    let _ = (NOW, json_number(0.0));
}
