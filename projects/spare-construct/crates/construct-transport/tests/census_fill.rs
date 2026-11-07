//! Land use truth tests: `fillAll` of packs/nz/landuse-plus.js end to end against the prototype with the network stubbed
//! (tools/transport-truth/gen-census.mjs): the requests, the progress messages, the report, the project afterwards (zones, results marked out of date
//! and the saved summary), the Overpass server fallback, and the school rolls entered after the project is reopened.

mod census_support;
mod edit_support;
mod support;

use census_support::{Replay, exchanges, ids_for, suburb_elements};
use construct_transport::census::{
    ApplyOpts, Bx, Downloads, FillOpts, LastFill, apply_rolls, fill_all, rolls_ready,
};
use construct_transport::project::Project;
use edit_support::{Checker, load_json, read_truth};
use serde_json::{Map, Value, json};

fn bx(b: &Value) -> Bx {
    Bx::new(
        b["s"].as_f64().unwrap(),
        b["w"].as_f64().unwrap(),
        b["n"].as_f64().unwrap(),
        b["e"].as_f64().unwrap(),
    )
}

fn fill_opts(o: &Value, elements: bool) -> FillOpts {
    let which = |k: &str| {
        o.get("which")
            .and_then(|w| w.get(k))
            .and_then(Value::as_bool)
    };
    FillOpts {
        bounds: o.get("bounds").map(bx),
        want_census: which("census"),
        want_employees: which("employees"),
        want_schools: which("schools"),
        want_aup: which("aup"),
        want_osm: which("osm"),
        elements: elements.then(suburb_elements),
        all: o.get("all").and_then(Value::as_bool).unwrap_or(false),
        apply: ApplyOpts {
            newer: o.get("newer").and_then(Value::as_str).map(str::to_string),
            ..Default::default()
        },
    }
}

// What the generator records of a project's scenarios afterwards: id, zones and results
fn scenarios_of(p: &Project) -> Value {
    Value::Array(p.scenarios.iter().map(|s| json!({ "id": s.id, "zones": serde_json::to_value(&s.zones).unwrap(), "res": serde_json::to_value(&s.res).unwrap() })).collect())
}

fn landuse_of(p: &Project) -> Value {
    p.extra.get("landuse").cloned().unwrap_or(Value::Null)
}

// A school as `rollsReady` lists it: the saved record with a roll of null
fn school_json(m: &construct_transport::census::Moe) -> Value {
    json!({ "id": m.id, "name": m.name, "type": m.kind_of_org, "status": m.status, "opens": m.opens, "rollDate": m.roll_date, "lat": m.lat, "lng": m.lng, "roll": null })
}

fn rolls_of(v: &Value) -> Vec<(String, Value)> {
    v.as_object()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default()
}

#[test]
fn filling_a_project_downloads_and_places_everything_as_the_prototype_does() {
    let ex = load_json("expected/census-fill.json");
    let table = exchanges();
    let mut ck = Checker::default();
    for case in ex.as_array().expect("cases") {
        let name = case["name"].as_str().expect("name");
        let text = read_truth(&format!(
            "fixtures/data/{}.json",
            case["project"].as_str().expect("project")
        ));
        let mut p = Project::from_json(&text).expect("the project reads");
        let active = case["active"].as_u64().unwrap() as usize;
        p.active = construct_transport::jsnum::Maybe::Set(p.scenarios[active].id.clone());
        if !case["onboarding"].is_null() {
            p.extra
                .insert("onboarding".into(), case["onboarding"].clone());
        }
        let mut ids = ids_for(case);
        let mut net = Replay::new(&case["log"], &table);
        let mut dl = Downloads::default();
        let mut last = LastFill::default();
        let mut status: Vec<String> = Vec::new();
        let opts = fill_opts(&case["opts"], case["elements"].as_bool().unwrap());
        for (i, call) in case["out"].as_array().expect("calls").iter().enumerate() {
            let label = format!("{name} (call {i})");
            let before = net.used();
            let smark = status.len();
            let r = fill_all(
                &mut p,
                active,
                &mut ids,
                &mut net,
                &mut dl,
                &mut last,
                &opts,
                &mut |t| status.push(t.to_string()),
            );
            match r {
                Ok(rep) => {
                    ck.same(
                        &format!("{label}: no error"),
                        &call["error"].is_null(),
                        &true,
                    );
                    ck.check(
                        &format!("{label}: report"),
                        &rep.to_value(),
                        &call["report"],
                    );
                }
                Err(e) => ck.same(&format!("{label}: error"), &json!(e), &call["error"]),
            }
            ck.same(
                &format!("{label}: requests"),
                &[before, net.used()].to_vec(),
                &vec![
                    call["requests"][0].as_u64().unwrap() as usize,
                    call["requests"][1].as_u64().unwrap() as usize,
                ],
            );
            let (s0, s1) = (
                call["status"][0].as_u64().unwrap() as usize,
                call["status"][1].as_u64().unwrap() as usize,
            );
            ck.check(
                &format!("{label}: messages"),
                &json!(status[smark..]),
                &json!(case["status"].as_array().unwrap()[s0..s1]),
            );
        }
        ck.same(&format!("{name}: every request made"), &net.done(), &true);
        ck.check(
            &format!("{name}: saved summary"),
            &landuse_of(&p),
            &case["after"].get("landuse").cloned().unwrap_or(Value::Null),
        );
        ck.check(
            &format!("{name}: scenarios"),
            &scenarios_of(&p),
            &case["after"]["scenarios"],
        );
        if !case["reopen"].is_null() {
            let re = &case["reopen"];
            // the project is saved and opened again: nothing of the session is kept
            let mut p2 = Project::from_json(&p.serialise()).expect("the saved project reads");
            if re["dropLocations"].as_bool() == Some(true)
                && let Some(Value::Array(list)) = p2
                    .extra
                    .get_mut("landuse")
                    .and_then(|l| l.get_mut("last"))
                    .and_then(|l| l.get_mut("schools"))
                    .and_then(|s| s.get_mut("needsRoll"))
            {
                for s in list {
                    s["lat"] = Value::Null;
                    s["lng"] = Value::Null;
                }
            }
            let mut ids2 = ids_for(re);
            let last2 = LastFill::default();
            let targets: Vec<usize> = if re["all"].as_bool() == Some(true) {
                (0..p2.scenarios.len()).collect()
            } else {
                vec![p2.active_scenario().map_or(0, |s| {
                    p2.scenarios.iter().position(|x| x.id == s.id).unwrap_or(0)
                })]
            };
            for (k, step) in re["rolls"].as_array().expect("steps").iter().enumerate() {
                let label = format!("{name}: rolls step {k} ({})", step["op"]);
                let rolls = if step["rolls"].is_null() {
                    None
                } else {
                    Some(rolls_of(&step["rolls"]))
                };
                match step["op"].as_str().unwrap() {
                    "ready" => {
                        let r = rolls_ready(&p2, &last2, rolls.as_deref());
                        let mut v = json!({ "list": r.list.iter().map(school_json).collect::<Vec<_>>(), "B": r.bounds.as_ref().map_or(Value::Null, |b| { let mut v = b.to_value(); v.as_object_mut().unwrap().remove("basis"); v }) });
                        if let Some(w) = &r.why {
                            v["why"] = json!(w);
                        }
                        ck.check(&label, &v, &step["out"]);
                    }
                    _ => {
                        let r = apply_rolls(
                            &mut p2,
                            &last2,
                            &mut ids2,
                            &targets,
                            &rolls.unwrap_or_default(),
                        );
                        let v = match &r {
                            None => Value::Null,
                            Some(rep) => {
                                let mut v = rep.to_value();
                                v["scenarios"] = json!(targets.len());
                                v
                            }
                        };
                        ck.check(&format!("{label}: report"), &v, &step["out"]);
                        ck.check(
                            &format!("{label}: saved summary"),
                            &landuse_of(&p2),
                            &step["landuse"],
                        );
                        ck.check(
                            &format!("{label}: scenarios"),
                            &scenarios_of(&p2),
                            &step["scenarios"],
                        );
                    }
                }
            }
            let _ = Map::new();
        }
    }
    ck.finish("filling projects");
}
