//! Auckland Council's growth scenario (AGS23) against the JavaScript's recorded runs: the download for several year lists and base
//! years (with and without the layer's description), the growth factors at points in and outside the zones, the proposed growth of
//! existing-land-use zones and applying it, and the growth beyond each gateway, in several states of the scenarios' years.
//! Recorded by tools/transport-truth/gen-feeds.mjs (expected/feeds-ags.json, fixtures/data/ags-*.json).

mod feeds_support;
mod support;

use construct_transport::feeds::*;
use construct_transport::jsnum::{JsNum, Maybe};
use construct_transport::project::Project;
use feeds_support::*;
use serde_json::{Map, Value, json};
use support::assert_close;

// a year or base year the way the prototype read it: a number, a number written as text, or left out (undefined)
fn number_arg(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().unwrap(),
        Value::String(s) => s.trim().parse().unwrap_or(f64::NAN),
        _ => f64::NAN,
    }
}

fn load_ags(bounds: &Value, years: &[f64], base: f64, log: &[Value], label: &str) -> AgsData {
    let mut load = AgsLoad::new(bounds_of(bounds), years, base, 30_000.0, &MetaCache::new());
    let (used, res) = replay(&mut load, log, 0, label);
    res.expect("the download");
    all_used(log, used, label);
    load.finish()
}

fn feeds_of_run(p: &Project) -> Value {
    p.extra.get("feeds").cloned().unwrap_or_else(|| json!({ "vintage": {} }))
}

#[test]
fn downloads_match() {
    let e = nan_as_null(&expected("ags"));
    let loads = e["loads"].as_array().unwrap();
    assert_eq!(loads.len(), 28);
    for (n, rec) in loads.iter().enumerate() {
        let label = format!("{n} {} meta {} years {} base {}", rec["project"], rec["meta"], rec["years"], rec["base"]);
        let years: Vec<f64> = rec["years"].as_array().unwrap().iter().map(number_arg).collect();
        let data = load_ags(&rec["bounds"], &years, number_arg(&rec["base"]), rec["requests"].as_array().unwrap(), &label);
        assert_close(&serde_json::to_value(&data).unwrap(), &rec["data"], &format!("{label} data"));
        let mut p = project(rec["project"].as_str().unwrap());
        apply_ags_load(&mut p, &data, NOW);
        assert_close(&feeds_of_run(&p), &rec["feeds"], &format!("{label} feeds"));
    }
}

#[test]
fn growth_factors_match() {
    let e = nan_as_null(&expected("ags"));
    let recs = e["factors"].as_array().unwrap();
    assert_eq!(recs.len(), 2);
    for rec in recs {
        let label = format!("factors {}", rec["project"]);
        let data = load_ags(&rec["bounds"], &[2031.0, 2041.0, 2052.0], 2026.0, rec["requests"].as_array().unwrap(), &label);
        let cases = rec["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 80);
        for c in cases {
            let ll = [c["ll"][0].as_f64().unwrap(), c["ll"][1].as_f64().unwrap()];
            let got = ags_zone_factor(ll, number_arg(&c["base"]), number_arg(&c["year"]), Some(&data));
            assert_close(&serde_json::to_value(got).unwrap(), &c["f"], &format!("{label} {} base {} year {}", c["ll"], c["base"], c["year"]));
        }
        let ll = [rec["cases"][0]["ll"][0].as_f64().unwrap(), rec["cases"][0]["ll"][1].as_f64().unwrap()];
        assert_eq!(ags_zone_factor(ll, 2026.0, 2031.0, None), None, "{label}: without data");
    }
}

fn zones_by_scenario(p: &Project) -> Value {
    let mut m = Map::new();
    for s in &p.scenarios {
        m.insert(s.id.clone(), serde_json::to_value(&s.zones).unwrap());
    }
    Value::Object(m)
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect()
}

#[test]
fn proposals_applying_and_gateways_match() {
    let e = nan_as_null(&expected("ags"));
    let recs = e["proposals"].as_array().unwrap();
    assert_eq!(recs.len(), 12);
    for rec in recs {
        let label = format!("{} {}", rec["project"], rec["label"]);
        let years: Vec<f64> = rec["years"].as_array().unwrap().iter().map(number_arg).collect();
        let data = load_ags(&rec["bounds"], &years, number_arg(&rec["baseYear"]), rec["requests"].as_array().unwrap(), &label);
        let text = rec["text"].as_str().unwrap();
        let p = project_of_text(text);
        let rows = ags_proposals(&p, Some(&data));
        assert_close(&serde_json::to_value(&rows).unwrap(), &rec["rows"], &format!("{label} rows"));
        for pick in ["all", "some", "none"] {
            let a = &rec[format!("apply_{pick}")];
            let mut p2 = reopen(text);
            let rows2 = ags_proposals(&p2, Some(&data));
            let n = apply_ags(&mut p2, &rows2, &strings(&a["ids"]), "2026-10-21");
            assert_eq!(n as u64, a["n"].as_u64().unwrap(), "{label} apply {pick}: applied");
            assert_close(&zones_by_scenario(&p2), &a["zones"], &format!("{label} apply {pick} zones"));
        }
        // the growth beyond gateways
        let mut p3 = reopen(text);
        let gw = ags_gateways(&mut p3, Some(&data), NOW);
        assert_close(&serde_json::to_value(&gw).unwrap(), &rec["gateways"], &format!("{label} gateways"));
        let mut kept = Map::new();
        for s in &p3.scenarios {
            let mut zs = Map::new();
            for z in s.zones.values().filter(|z| z.kind.is("gateway")) {
                zs.insert(z.id.clone(), z.extra.get("agsGrowth").cloned().unwrap_or(Value::Null));
            }
            kept.insert(s.id.clone(), Value::Object(zs));
        }
        assert_close(&Value::Object(kept), &rec["gatewayZones"], &format!("{label} growth kept on the zones"));
        for g in rec["gateway"].as_array().unwrap() {
            let sc = p3.scenarios.iter().find(|s| s.id == g["scenario"].as_str().unwrap()).unwrap();
            let z = &sc.zones[g["zone"].as_str().unwrap()];
            let base: construct_transport::jsnum::Num = if g["base"].get("undefined").is_some() { Maybe::Missing } else { Maybe::Set(JsNum::from(g["base"].clone())) };
            let got = ags_gateway(sc, z, &base, number_arg(&g["year"]), Some(&data));
            assert_close(&serde_json::to_value(got).unwrap(), &g["g"], &format!("{label} gateway {} {} base {} year {}", g["scenario"], g["zone"], g["base"], g["year"]));
        }
        let sc0 = &p3.scenarios[0];
        let z0 = sc0.zones.values().find(|z| z.kind.is("gateway")).unwrap();
        assert!(ags_gateway(sc0, z0, &Maybe::number(2026.0), 2031.0, None).is_none(), "{label}: gateway without data");
    }
}
