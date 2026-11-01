//! Study area truth tests: `StudyArea.setup` against the prototype with the network stubbed (tools/transport-truth/gen-studyarea.mjs): the requests, the progress
//! messages, the summary, the results of the model runs and the project afterwards (network, zones, counts, crashes and matrix estimation), in many ways.

mod census_support;
mod edit_support;
mod studyarea_support;
mod support;

use census_support::{Replay, ids_for};
use construct_transport::census::Downloads;
use construct_transport::jsnum::Maybe;
use construct_transport::project::{Project, Zone};
use construct_transport::studyarea::{SetupOpts, setup};
use edit_support::{Checker, load_json};
use serde_json::{Map, Value, json};
use studyarea_support::{corners, exchanges, project};

fn opts_of(o: &Value) -> SetupOpts {
    let mut s = SetupOpts::default();
    let flag = |k: &str| o.get(k).and_then(Value::as_bool);
    if let Some(v) = flag("roads") {
        s.roads = v;
    }
    if let Some(v) = flag("local") {
        s.local = v;
    }
    if let Some(v) = flag("service") {
        s.service = v;
    }
    if let Some(v) = flag("gateways") {
        s.gateways = v;
    }
    if let Some(v) = flag("counts") {
        s.counts = v;
    }
    if let Some(v) = flag("crashes") {
        s.crashes = v;
    }
    if let Some(v) = flag("existing") {
        s.existing = v;
    }
    if let Some(v) = flag("run") {
        s.run = v;
    }
    if let Some(v) = flag("fitCounts") {
        s.fit_counts = v;
    }
    s.all = flag("all");
    s
}

// Things done to the project between two runs of one case, by name (gen-studyarea.mjs HOOKS)
fn hook(p: &mut Project, name: &str) {
    match name {
        "dupUserZone" => {
            let sc = &mut p.scenarios[0];
            let z0 = sc
                .zones
                .values()
                .find(|z| {
                    z.existing.is_true()
                        && z.src.get().and_then(Value::as_str) == Some("existing-lu")
                        && z.kind.is("zone")
                })
                .cloned()
                .expect("a built zone");
            let (lat, lng) = (z0.lat.num(), z0.lng.num());
            let name0 = z0.name.as_str().unwrap().to_string();
            let mk = |v: Value| -> Zone { serde_json::from_value(v).expect("zone reads") };
            sc.zones.insert("zUserDup", mk(json!({ "id": "zUserDup", "kind": "zone", "name": format!("  {} ", name0.to_uppercase()), "lat": lat + 0.0004, "lng": lng, "lu": { "residential": 5 }, "connectors": null })));
            sc.zones.insert("zUserFar", mk(json!({ "id": "zUserFar", "kind": "zone", "name": name0, "lat": lat + 0.01, "lng": lng, "lu": { "residential": 5 }, "connectors": null })));
            sc.zones.insert("zOld", mk(json!({ "id": "zOld", "kind": "zone", "existing": true, "name": "Existing \u{b7} Old Road", "lat": lat - 0.0003, "lng": lng, "lu": { "residential": 7 }, "connectors": null })));
            sc.zones.insert("zOutside", mk(json!({ "id": "zOutside", "kind": "zone", "existing": true, "src": "existing-lu", "name": "Outside zone", "lat": -36.5, "lng": 174.5, "lu": { "residential": 7 }, "connectors": null })));
            let mut me = Map::new();
            me.insert("AM".into(), json!({ format!("{}>gateway", z0.id): 1.1 }));
            me.insert("info".into(), json!({ "AM": { "baseYear": 2026 } }));
            p.me = Maybe::Set(serde_json::from_value(Value::Object(me)).expect("factors read"));
        }
        "meUnrelated" => {
            p.me = Maybe::Set(
                serde_json::from_value(json!({ "AM": { "zzz>yyy": 1.1 } })).expect("factors read"),
            );
        }
        other => panic!("unknown hook {other}"),
    }
}

// What the generator records of each scenario's results
fn res_summary(p: &Project) -> Value {
    Value::Array(
        p.scenarios
            .iter()
            .map(|s| {
                let mut m = Map::new();
                if let Maybe::Set(Value::Object(res)) = &s.res {
                    for (k, r) in res {
                        m.insert(
                            k.clone(),
                            if r.is_null() { Value::Null } else { json!({ "period": r["period"], "it": r["it"], "gap": r["gap"], "converged": r["converged"], "stale": r["stale"], "kpi": r["kpi"] }) },
                        );
                    }
                }
                json!({ "id": s.id, "res": Value::Object(m) })
            })
            .collect(),
    )
}

fn state_of(p: &Project) -> Value {
    let scenarios: Vec<Value> = p
        .scenarios
        .iter()
        .map(|s| {
            let mut v = serde_json::to_value(s).unwrap();
            v.as_object_mut().unwrap().remove("res");
            v
        })
        .collect();
    json!({ "scenarios": scenarios, "data": serde_json::to_value(&p.data).unwrap(), "me": serde_json::to_value(&p.me).unwrap(), "active": serde_json::to_value(&p.active).unwrap() })
}

#[test]
fn setting_up_a_study_area_matches_the_prototype() {
    let ex = load_json("expected/studyarea-setup.json");
    let table = exchanges();
    let mut ck = Checker::default();
    for case in ex.as_array().expect("cases") {
        let name = case["name"].as_str().unwrap();
        let mut p = project(case["project"].as_str().unwrap());
        let active = case["active"].as_u64().unwrap() as usize;
        p.active = Maybe::Set(p.scenarios[active].id.clone());
        let b = corners(&case["bounds"]);
        let mut ids = ids_for(case);
        let mut net = Replay::new(&case["log"], &table);
        let mut dl = Downloads::default();
        let mut status: Vec<Value> = Vec::new();
        for (i, st) in case["steps"].as_array().unwrap().iter().enumerate() {
            let label = format!("{name} (step {i})");
            if let Some(h) = st["before"].as_str() {
                hook(&mut p, h);
            }
            let before = net.used();
            let smark = status.len();
            let r = setup(
                &mut p,
                &mut ids,
                &mut net,
                &mut dl,
                &b,
                &opts_of(&st["opts"]),
                &mut |t, f| status.push(json!([t, f])),
            );
            match r {
                Ok(out) => {
                    ck.same(&format!("{label}: no error"), &st["error"].is_null(), &true);
                    let v = out.to_value();
                    ck.check(&format!("{label}: summary"), &v, &st["out"]);
                }
                Err(e) => ck.same(&format!("{label}: error"), &json!(e), &st["error"]),
            }
            ck.same(
                &format!("{label}: requests"),
                &vec![before, net.used()],
                &vec![
                    st["requests"][0].as_u64().unwrap() as usize,
                    st["requests"][1].as_u64().unwrap() as usize,
                ],
            );
            let (s0, s1) = (
                st["status"][0].as_u64().unwrap() as usize,
                st["status"][1].as_u64().unwrap() as usize,
            );
            ck.check(
                &format!("{label}: progress"),
                &json!(status[smark..]),
                &json!(case["status"].as_array().unwrap()[s0..s1]),
            );
            ck.check(&format!("{label}: results"), &res_summary(&p), &st["res"]);
        }
        ck.same(&format!("{name}: every request made"), &net.done(), &true);
        ck.check(&format!("{name}: project"), &state_of(&p), &case["after"]);
    }
    ck.finish("study area setup");
}
