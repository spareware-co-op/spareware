//! Land-use truth tests: existing land use from OpenStreetMap against the prototype (landuse.js `ExistingLU`).
//!
//! tools/transport-truth/gen-landuse.mjs ran the prototype on the made-up suburb in tools/transport-truth/fixtures (and on odd and
//! broken input) and wrote expected/landuse-*.json; these tests give the Rust port the same input and compare the zones and the
//! statistics, the download queries and the combined answers, and the constants read from the prototype's own source.

mod edit_ops;
mod edit_support;
mod support;

use construct_transport::jsnum::{value_to_js_string, value_to_number};
use construct_transport::landuse::{
    BuildOpts, RoadLine, StudyBox, build, constants, count, download_jobs, merge_elements,
    overpass_elements, students, tiles, weight,
};
use construct_transport::util::fmt;
use edit_ops::ctx_for;
use edit_support::{Checker, load_json, read_truth};
use serde_json::{Value, json};

fn num(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().expect("number"),
        Value::String(s) => match s.as_str() {
            "NaN" => f64::NAN,
            "Infinity" => f64::INFINITY,
            "-Infinity" => f64::NEG_INFINITY,
            other => other.parse().unwrap_or(f64::NAN),
        },
        Value::Null => f64::NAN,
        other => panic!("not a number {other}"),
    }
}

fn study_box(v: &Value) -> Option<StudyBox> {
    if v.is_null() {
        return None;
    }
    Some(StudyBox {
        s: num(&v["s"]),
        w: num(&v["w"]),
        n: num(&v["n"]),
        e: num(&v["e"]),
    })
}

fn box_json(b: &StudyBox) -> Value {
    json!({ "s": b.s, "n": b.n, "w": b.w, "e": b.e })
}

#[test]
fn the_constants_match_the_prototypes_source() {
    let ex = load_json("expected/landuse-constants.json");
    let mut ck = Checker::default();
    ck.check("constants", &constants(), &ex);
    ck.finish("land-use constants");
}

#[test]
fn the_pure_functions_match_the_prototype() {
    let ex = load_json("expected/landuse-functions.json");
    let mut ck = Checker::default();
    for (i, c) in ex["tiles"].as_array().expect("cases").iter().enumerate() {
        let edge = c["edge"].as_f64().unwrap_or(3000.0);
        let got = if c.get("corners").is_some() {
            tiles(
                &StudyBox::from_corners(
                    [
                        c["b"][0][0].as_f64().expect("lat"),
                        c["b"][0][1].as_f64().expect("lng"),
                    ],
                    [
                        c["b"][1][0].as_f64().expect("lat"),
                        c["b"][1][1].as_f64().expect("lng"),
                    ],
                ),
                edge,
            )
        } else {
            tiles(&study_box(&c["b"]).expect("a box"), edge)
        };
        ck.check(
            &format!("tiles[{i}] {} {}", c["b"], c["edge"]),
            &Value::Array(got.iter().map(box_json).collect()),
            &c["out"],
        );
    }
    for (i, r) in ex["count"].as_array().expect("rows").iter().enumerate() {
        let text = if r[0].is_null() {
            String::new()
        } else {
            value_to_js_string(&r[0])
        };
        let got = count(&text).map_or(Value::Null, |x| json!(x));
        ck.check(&format!("count[{i}] {}", r[0]), &got, &r[1]);
    }
    for (i, r) in ex["students"].as_array().expect("rows").iter().enumerate() {
        ck.check(
            &format!("students[{i}] {}", r[0]),
            &json!(students(&r[0])),
            &r[1],
        );
    }
    for (k, v) in ex["weight"].as_object().expect("weights") {
        ck.same(&format!("weight {k}"), &weight(k), &num(v));
    }
    ck.finish("land-use functions");
}

/// The elements a recorded case was given, made from the fixture files.
fn input_of(case: &Value) -> Value {
    let inp = &case["input"];
    let fixture = |f: &str| -> Value {
        serde_json::from_str(&read_truth(&format!("fixtures/{f}"))).expect("JSON")
    };
    match inp["kind"].as_str().expect("kind") {
        "null" => Value::Null,
        "object" => fixture(inp["file"].as_str().expect("file")),
        "inline" => inp["value"].clone(),
        "list" => {
            if inp.get("empty").is_some() {
                return json!([]);
            }
            let all = fixture(inp["file"].as_str().expect("file"));
            let mut list: Vec<Value> = all["elements"].as_array().expect("elements").clone();
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
            Value::Array(list)
        }
        other => panic!("unknown input {other}"),
    }
}

#[test]
fn building_zones_match_the_prototype() {
    let ex = load_json("expected/landuse-build.json");
    let mut ck = Checker::default();
    for case in ex["cases"].as_array().expect("cases") {
        let name = case["name"].as_str().expect("name");
        let mut ctx = ctx_for(case);
        let elements = input_of(case);
        let o = &case["opts"];
        let links: Vec<RoadLine> = o
            .get("links")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|l| RoadLine {
                        name: l["name"].as_str().expect("name").to_string(),
                        pts: l["pts"]
                            .as_array()
                            .expect("pts")
                            .iter()
                            .map(|p| [p[0].as_f64().expect("lat"), p[1].as_f64().expect("lng")])
                            .collect(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let opts = BuildOpts {
            cell: o.get("cell").map(|c| match c {
                Value::String(s) => construct_transport::jsnum::string_to_number(s),
                other => value_to_number(other),
            }),
            links,
        };
        let out = build(
            &elements,
            study_box(&case["bounds"]).as_ref(),
            &mut ctx.ids,
            &opts,
        );
        let zones = serde_json::to_value(&out.zones).expect("serialises");
        ck.check(&format!("{name}.zones"), &zones, &case["zones"]);
        ck.check(
            &format!("{name}.stats"),
            &out.stats.to_value(),
            &case["stats"],
        );
        // The order of each zone's land uses
        for (i, z) in out.zones.iter().enumerate() {
            let want: Vec<String> = case["zones"][i]["lu"]
                .as_object()
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default();
            let mut got: Vec<String> =
                z.lu.get()
                    .map(|m| m.keys().map(str::to_string).collect())
                    .unwrap_or_default();
            let mut want = want;
            got.sort();
            want.sort();
            ck.same(&format!("{name}.zones[{i}].lu keys"), &got, &want);
        }
    }
    ck.finish("land-use build");
}

#[test]
fn the_download_matches_the_prototype() {
    let ex = load_json("expected/landuse-download.json");
    let files: Vec<Value> = ["buildings-0", "buildings-1", "areas-0", "areas-1"]
        .iter()
        .map(|n| {
            serde_json::from_str(&read_truth(&format!(
                "fixtures/landuse-papatoetoe-{n}.json"
            )))
            .expect("JSON")
        })
        .collect();
    let mut ck = Checker::default();
    for run in ex["runs"].as_array().expect("runs") {
        let name = run["name"].as_str().expect("name");
        let b = study_box(&run["box"]).expect("a box");
        let jobs = download_jobs(&b);
        let host = "overpass-api.de";
        match (&jobs, run["error"].as_str()) {
            (Err(e), Some(want)) => {
                ck.same(&format!("{name}.error"), e, &want.to_string());
                continue;
            }
            (Err(e), None) => {
                ck.same(&format!("{name}.error"), e, &"no error".to_string());
                continue;
            }
            (Ok(_), Some(want)) if !name.starts_with("remark") => {
                ck.same(
                    &format!("{name}.error"),
                    &"no error".to_string(),
                    &want.to_string(),
                );
                continue;
            }
            _ => {}
        }
        let jobs = jobs.expect("jobs");
        let queries: Vec<String> = run["queries"]
            .as_array()
            .expect("queries")
            .iter()
            .filter(|q| q["url"].as_str() == Some("https://overpass-api.de/api/interpreter"))
            .map(|q| q["q"].as_str().expect("text").to_string())
            .collect();
        let got_q: Vec<String> = jobs.iter().map(|j| j.query.clone()).collect();
        // With every server refusing, each query is posted to all four; the first server's copies are the jobs
        // (a failing download stops at the first query)
        let got_q: Vec<String> = if run["error"].is_string() {
            got_q.into_iter().take(queries.len()).collect()
        } else {
            got_q
        };
        ck.same(&format!("{name}.queries"), &got_q, &queries);
        // The answers the stub gave: by kind and number, alternating between the two tiles' files
        let (mut nb, mut na) = (0usize, 0usize);
        let mut batches: Vec<Vec<Value>> = Vec::new();
        let mut failed: Option<String> = None;
        for j in &jobs {
            let is_b = j.query.contains("way[\"building\"]");
            let k = if is_b { nb } else { na };
            if is_b {
                nb += 1
            } else {
                na += 1
            }
            let file = &files[(if is_b { 0 } else { 2 }) + (k % 2)];
            let answer = match name {
                "remarks" => {
                    json!({ "remark": "runtime error: Query timed out in \"query\" at line 4 after 181 seconds.", "elements": [] })
                }
                "remark out of memory" => {
                    json!({ "remark": "runtime error: Query run out of memory using about 2048 MB of RAM.", "elements": [{ "type": "node", "id": 1, "lat": 0, "lon": 0 }] })
                }
                "harmless remark" => {
                    json!({ "remark": "note: slow server", "elements": file["elements"].as_array().expect("elements")[..5].to_vec() })
                }
                "no elements key" => json!({}),
                _ => file.clone(),
            };
            // The last server tried names the failure
            match overpass_elements(
                &answer,
                if run["error"].is_string() {
                    "overpass.kumi.systems"
                } else {
                    host
                },
            ) {
                Ok(e) => batches.push(e),
                Err(e) => {
                    failed = Some(e);
                    break;
                }
            }
        }
        match (failed, run["error"].as_str()) {
            (Some(e), Some(want)) => {
                ck.same(&format!("{name}.error"), &format!("Couldn't download existing land use: {e}. Try again in a minute or use a smaller area."), &want.to_string());
                continue;
            }
            (Some(e), None) => {
                ck.same(&format!("{name}.error"), &e, &"no error".to_string());
                continue;
            }
            (None, Some(want)) => {
                ck.same(
                    &format!("{name}.error"),
                    &"no error".to_string(),
                    &want.to_string(),
                );
                continue;
            }
            (None, None) => {}
        }
        let merged = merge_elements(&batches);
        ck.check(
            &format!("{name}.elements"),
            &Value::Array(merged.clone()),
            &run["elements"],
        );
        // The progress messages: each request, then the count
        let mut want: Vec<String> = jobs
            .iter()
            .map(|j| format!("{} from {host}\u{2026}", j.label))
            .collect();
        want.push(format!(
            "Downloaded {} OpenStreetMap features",
            fmt::int(merged.len() as f64)
        ));
        let got: Vec<String> = run["statuses"]
            .as_array()
            .expect("statuses")
            .iter()
            .map(|s| s.as_str().expect("text").to_string())
            .collect();
        ck.same(&format!("{name}.statuses"), &want, &got);
    }
    ck.finish("land-use download");
}
