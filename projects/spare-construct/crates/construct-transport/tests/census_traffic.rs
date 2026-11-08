//! Land use truth tests: development traffic of packs/nz/landuse-plus.js (where it goes by gateway, junctions without and with the development, the "without"
//! scenario, the text naming a gateway's count and setting a gateway from one chosen count) against the prototype (tools/transport-truth/gen-census.mjs).
//! The results come from this crate's own assignment, which the assignment truth tests hold to the prototype's.

mod census_support;
mod edit_support;
mod support;

use construct_transport::assign::assign;
use construct_transport::census::{
    distribution_of, gw_src_text, use_count, with_without_res, without_scenario,
};
use construct_transport::datahub::Data;
use construct_transport::jsnum::{JsNum, Maybe};
use construct_transport::project::{Project, Scenario};
use construct_transport::results::{AssignOpts, Res};
use edit_support::{Checker, load_json, read_truth};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

/// The results of each scenario and period, as the run made them (the order of junctions and gateways is the run's own).
type Runs = BTreeMap<String, Res>;

fn load() -> (Project, Runs) {
    let p = Project::from_json(&read_truth("fixtures/data/census-traffic-project.json"))
        .expect("the project reads");
    let mut runs = Runs::new();
    for s in &p.scenarios {
        for period in ["AM", "PM"] {
            let opts = AssignOpts {
                period: Some(period.to_string()),
                ..Default::default()
            };
            runs.insert(
                format!("{}|{period}", s.id),
                assign(&p, s, &p.settings, &opts, None),
            );
        }
    }
    (p, runs)
}

fn sc<'a>(p: &'a Project, id: &str) -> &'a Scenario {
    p.scenario(id).expect("scenario")
}

fn sc_mut<'a>(p: &'a mut Project, id: &str) -> &'a mut Scenario {
    p.scenarios
        .iter_mut()
        .find(|s| s.id == id)
        .expect("scenario")
}

fn variant(p: &mut Project, runs: &mut Runs, name: &str) {
    match name {
        "some gateways from counts" => {
            sc_mut(p, "sWith").zones.get_mut("gE").unwrap().gw_src = Maybe::Set(json!("x"))
        }
        "every gateway from counts" => {
            for z in sc_mut(p, "sWith").zones.values_mut() {
                if z.kind.is("gateway") {
                    z.gw_src = Maybe::Set(json!("NZTA 01N00478 SH1 Main Road west (18,200/day)"));
                }
            }
        }
        "matrix estimation factors" => {
            p.me = Maybe::Set(serde_json::from_value(json!({ "AM": { "gW>zDev": 1.1, "zEx>gE": 0.9 }, "PM": { "gW>zDev": 1 }, "info": {} })).expect("factors read"));
        }
        "stale results" => runs.get_mut("sWith|AM").unwrap().stale = true,
        "a manual matrix" => runs.get_mut("sWith|PM").unwrap().manual = true,
        "no gateways" => {
            let ids: Vec<String> = sc(p, "sWith")
                .zones
                .iter()
                .filter(|(_, z)| z.kind.is("gateway"))
                .map(|(k, _)| k.to_string())
                .collect();
            for id in ids {
                sc_mut(p, "sWith").zones.remove(&id);
            }
        }
        "without stale" => runs.get_mut("sBase|PM").unwrap().stale = true,
        _ => {}
    }
}

fn dist_json(p: &Project, s: &Scenario, runs: &Runs) -> Value {
    let res: Vec<&Res> = ["AM", "PM"]
        .iter()
        .filter_map(|per| runs.get(&format!("{}|{per}", s.id)))
        .collect();
    match distribution_of(p, s, &res) {
        None => Value::Null,
        Some(d) => {
            let v = serde_json::to_value(&d.dist).expect("a distribution writes");
            let sums: Map<String, Value> = d.sums.iter().map(|(k, i, o)| (k.clone(), json!({ "in": construct_transport::jsnum::plain_number(*i), "out": construct_transport::jsnum::plain_number(*o) }))).collect();
            json!({ "rows": v["rows"], "periods": v["periods"], "label": v["label"], "sums": Value::Object(sums), "basis": d.basis, "stale": d.stale })
        }
    }
}

#[test]
fn where_development_traffic_goes_and_the_junctions_without_and_with_it() {
    let ex = load_json("expected/census-traffic.json");
    let (base, base_runs) = load();
    let mut ck = Checker::default();
    for case in ex["cases"].as_array().expect("cases") {
        let name = case["name"].as_str().unwrap();
        let mut p = base.clone();
        let mut runs = base_runs.clone();
        variant(&mut p, &mut runs, name);
        let with = sc(&p, "sWith");
        ck.check(
            &format!("{name}: distribution"),
            &dist_json(&p, with, &runs),
            &case["distribution"],
        );
        for (k, expected) in case["withWithout"].as_object().unwrap() {
            let (wo, period) = k.split_once('|').unwrap();
            let got = with_without_res(with, &runs[&format!("{wo}|{period}")], &runs[&format!("sWith|{period}")], period).map_or(Value::Null, |r| {
                let v = serde_json::to_value(&r.ww).expect("a comparison writes");
                json!({ "rows": v["rows"], "note": v["note"], "label": v["label"], "stale": r.stale, "period": r.period })
            });
            ck.check(&format!("{name}: junctions {k}"), &got, expected);
        }
        for s in &p.scenarios {
            ck.check(
                &format!("{name}: distribution of {}", s.id),
                &dist_json(&p, s, &runs),
                &case["others"][&s.id],
            );
        }
    }
    ck.finish("development traffic");
}

#[test]
fn the_scenario_without_the_development_is_chosen_as_the_prototype_does() {
    let ex = load_json("expected/census-traffic.json");
    let (base, _) = load();
    let mut ck = Checker::default();
    for c in ex["without"].as_array().expect("cases") {
        let name = c["name"].as_str().unwrap();
        let mut p = base.clone();
        if let Some(l) = c["landuse"].as_object().filter(|l| !l.is_empty()) {
            p.extra.insert("landuse".into(), Value::Object(l.clone()));
        }
        let i = |p: &mut Project, id: &str| {
            p.scenarios
                .iter_mut()
                .find(|s| s.id == id)
                .map(|s| s as *mut Scenario)
        };
        let _ = i;
        match name {
            "no baseline, base is same year" => {
                sc_mut(&mut p, "sBase").role = Maybe::Set("sensitivity".into());
                p.base = Maybe::Set("sBase".into());
            }
            "no baseline, another same-year scenario" => {
                sc_mut(&mut p, "sBase").role = Maybe::Set("proposal".into());
                sc_mut(&mut p, "sMit").role = Maybe::Set("mitigation".into());
            }
            "only a different year" => {
                for s in p.scenarios.iter_mut() {
                    if s.id != "sWith" && s.id != "sOld" {
                        s.year = Maybe::Set(JsNum::Number(2040.0));
                    }
                }
            }
            "nothing else" => p.scenarios.retain(|s| s.id == "sWith"),
            "project year" => {
                for s in p.scenarios.iter_mut() {
                    s.year = Maybe::Missing;
                }
                p.settings.year = Maybe::Set(JsNum::Number(2026.0));
            }
            _ => {}
        }
        let idx = p.scenarios.iter().position(|s| s.id == "sWith").unwrap();
        let got = without_scenario(&p, idx).map(|i| p.scenarios[i].id.clone());
        ck.same(
            &format!("withoutScenario {name}"),
            &got,
            &c["picked"].as_str().map(str::to_string),
        );
    }
    ck.finish("scenario without the development");
}

#[test]
fn the_text_naming_a_gateways_count_matches_the_prototype() {
    let ex = load_json("expected/census-traffic.json");
    let (p, _) = load();
    let data = Data::read(&p);
    let mut ck = Checker::default();
    let labels: Vec<String> = ex["labels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l.as_str().unwrap().to_string())
        .collect();
    ck.same(
        "the labels of the counts",
        &labels.len(),
        &data.counts.len(),
    );
    for (i, c) in ex["gwSrcText"]
        .as_array()
        .expect("cases")
        .iter()
        .enumerate()
    {
        let mut s = p.scenarios[0].clone();
        if c["gwSrc"] == "no zone" {
            ck.same(
                &format!("gwSrcText[{i}] no gateway"),
                &gw_src_text(&s, "absent", &data.counts),
                &c["out"].as_str().unwrap().to_string(),
            );
            continue;
        }
        let z = s.zones.get_mut("gW").unwrap();
        z.gw_src = if c["gwSrc"].is_null() {
            Maybe::Missing
        } else {
            Maybe::Set(c["gwSrc"].clone())
        };
        ck.same(
            &format!("gwSrcText[{i}] {}", c["gwSrc"]),
            &gw_src_text(&s, "gW", &data.counts),
            &c["out"].as_str().unwrap().to_string(),
        );
    }
    ck.finish("count text");
}

#[test]
fn setting_a_gateway_from_one_chosen_count_matches_the_prototype() {
    let ex = load_json("expected/census-traffic.json");
    let (base, _) = load();
    let mut ck = Checker::default();
    for (i, c) in ex["useCount"].as_array().expect("cases").iter().enumerate() {
        let mut p = base.clone();
        if c["useVirtual"].as_bool() == Some(true) {
            p.data
                .get_mut()
                .unwrap()
                .extra
                .insert("use".into(), json!({ "virtual": true }));
        }
        let (gw, count) = (c["gw"].as_str().unwrap(), c["count"].as_str().unwrap());
        let before = p.scenarios[0]
            .zones
            .get(gw)
            .map_or(Value::Null, |z| serde_json::to_value(z).unwrap());
        // the project the prototype used for the zone has no results; the zone is compared without them
        ck.check(
            &format!("useCount[{i}] {gw} {count}: zone before"),
            &before,
            &c["before"],
        );
        let r = use_count(&mut p, 0, gw, count);
        let mut got = json!({ "n": r.n });
        if let Some(s) = &r.src {
            got["src"] = json!(s);
        }
        if let Some(e) = &r.error {
            got["error"] = json!(e);
        }
        ck.check(
            &format!("useCount[{i}] {gw} {count}: answer"),
            &got,
            &c["out"],
        );
        let after = p.scenarios[0]
            .zones
            .get(gw)
            .map_or(Value::Null, |z| serde_json::to_value(z).unwrap());
        ck.check(
            &format!("useCount[{i}] {gw} {count}: zone after"),
            &after,
            &c["after"],
        );
        let counts = p
            .data
            .get()
            .and_then(|d| d.extra.get("counts"))
            .cloned()
            .unwrap_or(Value::Null);
        let off: Vec<Value> = counts
            .as_array()
            .unwrap()
            .iter()
            .map(|k| k.get("off").cloned().unwrap_or(Value::Null))
            .collect();
        ck.check(
            &format!("useCount[{i}] {gw} {count}: counts put back"),
            &Value::Array(off),
            &c["off"],
        );
    }
    ck.finish("one chosen count");
}
