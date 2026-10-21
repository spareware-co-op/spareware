//! Legal speed limits from the NZTA register against the JavaScript's recorded runs: the variable limit periods, the where clause,
//! the download (with and without the layer's description), which versions are kept, the modal limit put on every link of both
//! test projects, the review of modelled against legal speeds and applying it, and the whole download for scenarios in several years.
//! Recorded by tools/transport-truth/gen-feeds.mjs (expected/feeds-nslr.json, fixtures/data/nslr-*.json).

mod feeds_support;
mod support;

use construct_transport::feeds::*;
use construct_transport::jsnum::{JsNum, Maybe};
use construct_transport::project::{Project, Scenario};
use feeds_support::*;
use serde_json::{Map, Value, json};
use support::assert_close;

fn year_arg(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => match s.as_str() {
            "NaN" | "x" => None,
            t => t.parse().ok(),
        },
        _ => None,
    }
}

#[test]
fn variable_limit_periods_match() {
    let e = expected("nslr");
    for t in e["varPeriods"].as_array().unwrap() {
        let text = match &t["text"] {
            Value::String(s) => s.clone(),
            _ => String::new(),
        };
        let got = var_periods(&text).map_or(Value::Null, |v| serde_json::to_value(v).unwrap());
        assert_close(&got, &t["result"], &format!("varPeriods of {:?}", t["text"]));
    }
}

#[test]
fn where_clauses_match() {
    let e = expected("nslr");
    for t in e["where"].as_array().unwrap() {
        let year = match &t["year"] {
            Value::Object(_) => None,
            v => year_arg(v),
        };
        // a year that is text ('2031') or not a number: 0, undefined and null read as none
        let now = t["now"].as_f64().unwrap();
        assert_eq!(nslr_where(year, now), t["where"].as_str().unwrap(), "where for {} at {now}", t["year"]);
    }
}

fn run_load(rec: &Value, cache: &MetaCache, from: usize, log: &[Value]) -> (NslrSet, usize) {
    let year = rec["year"].as_f64();
    let mut load = NslrLoad::new(bounds_of(&rec["bounds"]), year, rec["now"].as_f64().unwrap(), 30_000.0, cache);
    let (used, res) = replay(&mut load, log, from, "nslr load");
    res.expect("the download");
    (load.finish(), used)
}

fn recorded_set(e: &Value, project: &str, label: &str) -> NslrSet {
    let rec = e["loads"].as_array().unwrap().iter().find(|l| l["project"] == project && l["label"] == label).expect("a recorded load");
    run_load(rec, &MetaCache::new(), 0, rec["requests"].as_array().unwrap()).0
}

// The JavaScript keeps whatever the layer sent for each ring point (some answers carry a height as a third value) in the saved
// polygon; nothing reads it, and the Rust keeps longitude and latitude only. The recorded rings are cut to two values to compare.
fn lng_lat_only(set: &Value) -> Value {
    let mut set = set.clone();
    for poly in set["polys"].as_array_mut().into_iter().flatten() {
        for ring in poly["rings"].as_array_mut().into_iter().flatten() {
            for pt in ring.as_array_mut().into_iter().flatten() {
                if let Some(a) = pt.as_array_mut() {
                    a.truncate(2);
                }
            }
        }
    }
    set
}

#[test]
fn downloads_match() {
    let e = expected("nslr");
    let loads = e["loads"].as_array().unwrap();
    assert_eq!(loads.len(), 12);
    for rec in loads {
        let log = rec["requests"].as_array().unwrap();
        let (set, used) = run_load(rec, &MetaCache::new(), 0, log);
        all_used(log, used, &format!("{} {}", rec["project"], rec["label"]));
        assert_close(&serde_json::to_value(&set).unwrap(), &lng_lat_only(&rec["set"]), &format!("$.loads[{} {}].set", rec["project"], rec["label"]));
    }
}

// the link fields the review and the assignment read, as the recorded runs show them
fn link_fields(sc: &Scenario) -> Value {
    let mut out = Map::new();
    for l in sc.links.values() {
        let mut o = Map::new();
        if let Maybe::Set(v) = &l.speed {
            o.insert("speed".to_string(), v.to_value());
        }
        for k in ["speedLimit", "speedSource", "nslr", "speedOSM", "speedApplied"] {
            if let Some(v) = l.extra.get(k) {
                o.insert(k.to_string(), v.clone());
            }
        }
        if let Some(v) = l.var_speed.get() {
            o.insert("varSpeed".to_string(), v.clone());
        }
        out.insert(l.id.clone(), Value::Object(o));
    }
    Value::Object(out)
}

fn set_link_fields(sc: &mut Scenario, fields: &Value) {
    for (id, f) in fields.as_object().unwrap() {
        let Some(l) = sc.links.get_mut(id) else { continue };
        match f.get("speed") {
            Some(v) => l.speed = Maybe::Set(JsNum::from(v.clone())),
            None => l.speed = Maybe::Missing,
        }
        for k in ["speedLimit", "speedSource", "nslr", "speedOSM", "speedApplied"] {
            match f.get(k) {
                Some(v) => {
                    l.extra.insert(k.to_string(), v.clone());
                }
                None => {
                    l.extra.remove(k);
                }
            }
        }
        l.var_speed = f.get("varSpeed").map_or(Maybe::Missing, |v| Maybe::Set(v.clone()));
    }
}

fn scenario_mut<'a>(p: &'a mut Project, id: &str) -> &'a mut Scenario {
    p.scenarios.iter_mut().find(|s| s.id == id).expect("scenario")
}

#[test]
fn limits_are_put_on_links_and_reviewed_as_recorded() {
    let e = expected("nslr");
    let recs = e["assigns"].as_array().unwrap();
    assert_eq!(recs.len(), 20);
    for rec in recs {
        let (name, label, scid) = (rec["project"].as_str().unwrap(), rec["label"].as_str().unwrap(), rec["scenario"].as_str().unwrap());
        let at = format!("{name} {label} {scid}");
        let set = recorded_set(&e, name, label);
        let mut p = project(name);
        let sc = scenario_mut(&mut p, scid);
        set_link_fields(sc, &rec["before"]);
        assert_close(&link_fields(sc), &rec["before"], &format!("{at} before"));
        let stats = assign_nslr(sc, &set);
        assert_close(&serde_json::to_value(stats).unwrap(), &rec["stats"], &format!("{at} stats"));
        assert_close(&link_fields(sc), &rec["after"], &format!("{at} after"));
        assert_close(&serde_json::to_value(speed_proposals(sc)).unwrap(), &rec["proposals"], &format!("{at} proposals"));
        let ids: Vec<String> = rec["applyIds"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
        assert_eq!(apply_speeds(sc, &ids) as u64, rec["applied"].as_u64().unwrap(), "{at}: applied");
        assert_close(&link_fields(sc), &rec["afterApply"], &format!("{at} after applying"));
        if let Some(m) = rec.get("manualSpeed") {
            let l = sc.links.get_mut(m["id"].as_str().unwrap()).unwrap();
            l.speed = Maybe::number(m["speed"].as_f64().unwrap());
        }
        assert_close(&serde_json::to_value(speed_proposals(sc)).unwrap(), &rec["proposals2"], &format!("{at} proposals after a hand change"));
        let ids2: Vec<String> = rec["proposals2"].as_array().unwrap().iter().map(|x| x["id"].as_str().unwrap().to_string()).collect();
        assert_eq!(apply_speeds(sc, &ids2) as u64, rec["applied2"].as_u64().unwrap(), "{at}: applied again");
        assert_close(&link_fields(sc), &rec["afterApply2"], &format!("{at} after applying again"));
    }
}

#[test]
fn the_whole_download_for_every_scenario_matches() {
    let e = expected("nslr");
    let recs = e["alls"].as_array().unwrap();
    assert_eq!(recs.len(), 10);
    for rec in recs {
        let name = rec["project"].as_str().unwrap();
        let at = format!("{name} {}", rec["label"]);
        let mut p = project(name);
        if let Some(f) = rec["startFeeds"].as_object() {
            p.settings.extra.insert("feeds".to_string(), Value::Object(f.clone()));
        }
        for (s, y) in p.scenarios.iter_mut().zip(rec["years"].as_array().unwrap()) {
            s.year = y.as_f64().map_or(Maybe::Missing, Maybe::number);
        }
        let cfg = FeedsConfig::of(&p);
        let future = construct_transport::jsnum::value_truthy(&cfg.nslr_future);
        let years = nslr_years(&p, NOW, future);
        let log = rec["requests"].as_array().unwrap();
        let cache = MetaCache::new();
        let (mut sets, mut statuses, mut used) = (Vec::new(), Vec::new(), 0);
        for y in &years {
            statuses.push(nslr_status(*y));
            let mut load = NslrLoad::new(bounds_of(&rec["bounds"]), if *y != 0.0 { Some(*y) } else { None }, NOW, cfg.limit_ms(), &cache);
            let (u, res) = replay(&mut load, log, used, &at);
            res.expect("a download");
            used = u;
            sets.push((*y, load.finish()));
        }
        all_used(log, used, &at);
        assert_eq!(json!(statuses), rec["statuses"], "{at}: status lines");
        let res = apply_nslr_all(&mut p, &sets, NOW, future);
        assert_close(&serde_json::to_value(&res).unwrap(), &rec["result"], &format!("{at} result"));
        assert_close(&p.extra["feeds"], &rec["feeds"], &format!("{at} feeds"));
        let links: Map<String, Value> = p.scenarios.iter().map(|s| (s.id.clone(), link_fields(s))).collect();
        assert_close(&Value::Object(links), &rec["links"], &format!("{at} links"));
    }
}
