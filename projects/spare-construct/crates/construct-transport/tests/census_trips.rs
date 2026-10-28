//! Land use truth tests: trip generation of packs/nz/landuse-plus.js (person and vehicle trips, the adjustments in order, how a zone's table reaches the
//! model, the tables, the RR 422 check and trip_generation.csv) against the prototype (tools/transport-truth/gen-census.mjs).

mod census_support;
mod edit_support;
mod support;

use construct_transport::census::{TablesOpts, csv, rate_flag, set_table, shares_of, sync, sync_all, tables, trip_gen};
use construct_transport::edit::new_project;
use construct_transport::jsnum::{Maybe, json_number, value_to_number};
use construct_transport::project::{Project, Scenario, Settings, Zone};
use edit_support::{Checker, load_json};
use serde_json::{Value, json};

struct Data {
    settings: Vec<Settings>,
    scenarios: Vec<Scenario>,
    zones: Vec<Zone>,
}

fn data(ex: &Value) -> Data {
    let settings = ex["settings"].as_array().unwrap().iter().map(|s| serde_json::from_value(s.clone()).expect("settings read")).collect();
    let scenarios = ex["scenarios"].as_array().unwrap().iter().map(|s| serde_json::from_value(s.clone()).expect("scenario reads")).collect();
    let zones = ex["zones"].as_array().unwrap().iter().map(|z| serde_json::from_value(z.clone()).expect("zone reads")).collect();
    Data { settings, scenarios, zones }
}

fn plain(x: f64) -> Value {
    json_number(x)
}

#[test]
fn trips_per_activity_and_zone_match_the_prototype() {
    let ex = load_json("expected/census-trips.json");
    let d = data(&ex);
    let mut ck = Checker::default();
    for (i, c) in ex["tripGen"].as_array().expect("cases").iter().enumerate() {
        let (zi, si, sc) = (c["zone"].as_u64().unwrap() as usize, c["S"].as_u64().unwrap() as usize, c["sc"].as_u64().unwrap() as usize);
        let got = trip_gen(&d.zones[zi], &d.settings[si], c["period"].as_str().unwrap(), &d.scenarios[sc]).to_value();
        ck.check(&format!("tripGen[{i}] zone {zi} settings {si} scenario {sc} {}", c["period"]), &got, &c["out"]);
    }
    ck.finish("trip generation");
}

#[test]
fn mode_shares_and_rate_flags_match_the_prototype() {
    let ex = load_json("expected/census-trips.json");
    let mut ck = Checker::default();
    for (i, c) in ex["sharesOf"].as_array().expect("cases").iter().enumerate() {
        let got = shares_of(&c["c"], c["g"].as_str().unwrap()).map_or(Value::Null, |s| {
            let mut o = serde_json::Map::new();
            for (k, m) in ["driver", "passenger", "pt", "walk", "cycle"].iter().enumerate() {
                o.insert(m.to_string(), plain(s.modes[k]));
            }
            o.insert("n".into(), plain(s.n));
            o.insert("carSpec".into(), s.car_spec.map_or(Value::Null, plain));
            Value::Object(o)
        });
        ck.check(&format!("sharesOf[{i}]"), &got, &c["out"]);
    }
    for (i, c) in ex["rateFlag"].as_array().expect("cases").iter().enumerate() {
        let year = if c["in"].get("undefined").is_some() {
            f64::NAN
        } else if c["in"].get("nonFinite").is_some() {
            f64::NAN
        } else {
            value_to_number(&c["in"])
        };
        let got = rate_flag(year).map_or(Value::Null, |f| json!({ "level": f.level, "text": f.text }));
        ck.check(&format!("rateFlag[{i}] {}", c["in"]), &got, &c["out"]);
    }
    ck.finish("mode shares and rate flags");
}

#[test]
fn loading_a_table_into_the_model_matches_the_prototype() {
    let ex = load_json("expected/census-trips.json");
    let d = data(&ex);
    let mut ck = Checker::default();
    for (i, c) in ex["sync"].as_array().expect("cases").iter().enumerate() {
        let (zi, si, sc) = (c["zone"].as_u64().unwrap() as usize, c["S"].as_u64().unwrap() as usize, c["sc"].as_u64().unwrap() as usize);
        let mut z = d.zones[zi].clone();
        let changed = sync(&mut z, &d.settings[si], &d.scenarios[sc]);
        let again = sync(&mut z, &d.settings[si], &d.scenarios[sc]);
        ck.same(&format!("sync[{i}] zone {zi}: changed"), &changed, &c["changed"].as_bool().unwrap());
        ck.same(&format!("sync[{i}] zone {zi}: changed again"), &again, &c["again"].as_bool().unwrap());
        ck.check(&format!("sync[{i}] zone {zi}: zone"), &serde_json::to_value(&z).unwrap(), &c["z"]);
    }
    for (i, c) in ex["setTable"].as_array().expect("cases").iter().enumerate() {
        let zi = c["zone"].as_u64().unwrap() as usize;
        let mut z = d.zones[zi].clone();
        for (k, st) in c["steps"].as_array().unwrap().iter().enumerate() {
            set_table(&mut z, st["op"] == "on", &d.settings[0], &d.scenarios[1]);
            ck.check(&format!("setTable[{i}] zone {zi} step {k} {}", st["op"]), &serde_json::to_value(&z).unwrap(), &st["z"]);
        }
    }
    ck.finish("loading tables");
}

fn project_with(ex: &Value, zones: &[Zone], si: usize, sc: usize, landuse: &Value) -> (Project, usize) {
    let mut p = new_project(&mut census_support::ids_for(&json!({ "seed": 1, "rngSkip": 0, "seq0": 0, "now": 0 })), "tables");
    p.settings = serde_json::from_value(ex["settings"][si].clone()).expect("settings read");
    let mut scn: Scenario = serde_json::from_value(ex["scenarios"][sc].clone()).expect("scenario reads");
    for z in zones {
        scn.zones.insert(z.id.clone(), z.clone());
    }
    p.scenarios = vec![scn];
    p.active = Maybe::Set(p.scenarios[0].id.clone());
    if !landuse.is_null() {
        p.extra.insert("landuse".into(), landuse.clone());
    }
    (p, 0)
}

#[test]
fn the_tables_and_the_csv_match_the_prototype() {
    let ex = load_json("expected/census-trips.json");
    let d = data(&ex);
    let mut ck = Checker::default();
    for (i, c) in ex["tables"].as_array().expect("cases").iter().enumerate() {
        let (si, sc) = (c["S"].as_u64().unwrap() as usize, c["sc"].as_u64().unwrap() as usize);
        let ids: Vec<usize> = c["ids"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as usize).collect();
        let zones: Vec<Zone> = ids.iter().map(|k| d.zones[*k].clone()).collect();
        let (p, _) = project_with(&ex, &zones, si, sc, &c["opts"]["landuse"]);
        let sc0 = &p.scenarios[0];
        let opts = TablesOpts {
            zones: c["opts"]["zones"].as_array().map(|l| l.iter().map(|k| d.zones[k.as_u64().unwrap() as usize].id.clone()).collect()),
            scope: c["opts"]["scope"].as_str().map(str::to_string),
        };
        let t = tables(&p, sc0, &p.settings, &opts);
        let got = json!({
            "zones": t.zones,
            "flags": t.flags.iter().map(|f| json!({ "zone": f.zone, "activity": f.activity, "level": f.level, "text": f.text })).collect::<Vec<_>>(),
            "sanity": t.sanity.iter().map(|s| json!({ "period": s.period, "dwellings": plain(s.dwellings), "veh": plain(s.veh), "rate": s.rate.map_or(Value::Null, plain), "ok": s.ok })).collect::<Vec<_>>(),
        });
        ck.check(&format!("tables[{i}]"), &got, &c["tables"]);
        ck.same(&format!("csv[{i}]"), &csv(&p, sc0, &p.settings, &opts), &c["csv"].as_str().unwrap().to_string());
    }
    ck.finish("tables and CSV");
}

#[test]
fn syncing_every_scenario_of_a_project_matches_the_prototype() {
    let ex = load_json("expected/census-trips.json");
    let mut ck = Checker::default();
    let mut p = Project::from_json(&ex["syncAll"]["before"].to_string()).expect("the project reads");
    let n = sync_all(&mut p);
    ck.same("syncAll: zones changed", &(n as u64), &ex["syncAll"]["n"].as_u64().unwrap());
    let after = Value::Array(p.scenarios.iter().map(|s| json!({ "id": s.id, "zones": serde_json::to_value(&s.zones).unwrap() })).collect());
    ck.check("syncAll: scenarios", &after, &ex["syncAll"]["after"]);
    ck.finish("syncing every scenario");
}
