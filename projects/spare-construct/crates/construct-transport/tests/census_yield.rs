//! Land use truth tests: zoned yield of packs/nz/landuse-plus.js (the land use a zoned area holds at full build-out, the capacity table and filling a zone from a
//! yield) against the prototype (tools/transport-truth/gen-census.mjs).

mod census_support;
mod edit_support;
mod support;

use construct_transport::census::{apply_yield, yield_of, zoned_capacity};
use construct_transport::edit::new_project;
use construct_transport::jsnum::value_to_number;
use construct_transport::project::{Project, Zone};
use edit_support::{Checker, load_json};
use serde_json::{Value, json};

// The projects the cases name: none, one with nothing saved, one with a landuse record, one with the user's own yields and proposals
fn projects(ex: &Value) -> Vec<Project> {
    ex["Ps"]
        .as_array()
        .expect("projects")
        .iter()
        .map(|p| {
            let mut pr = new_project(
                &mut census_support::ids_for(
                    &json!({ "seed": 1, "rngSkip": 0, "seq0": 0, "now": 0 }),
                ),
                "yield",
            );
            if let Some(l) = p.get("landuse") {
                pr.extra.insert("landuse".into(), l.clone());
            }
            pr
        })
        .collect()
}

#[test]
fn the_yield_of_a_zoned_area_matches_the_prototype() {
    let ex = load_json("expected/census-yield.json");
    let ps = projects(&ex);
    let mut ck = Checker::default();
    for (i, c) in ex["yieldOf"].as_array().expect("cases").iter().enumerate() {
        let code = value_to_number(&c["code"]);
        let ha = value_to_number(&c["ha"]);
        let pi = c["P"].as_u64().unwrap() as usize;
        // a missing project reads as one with nothing saved
        let got = yield_of(code, ha, Some(&ps[pi]), c["name"].as_str()).to_value();
        ck.check(
            &format!("yieldOf[{i}] {} {} P{pi}", c["code"], c["ha"]),
            &got,
            &c["out"],
        );
    }
    ck.finish("zoned yield");
}

#[test]
fn the_capacity_table_matches_the_prototype() {
    let ex = load_json("expected/census-yield.json");
    let ps = projects(&ex);
    let mut ck = Checker::default();
    for (i, c) in ex["zonedCapacity"]
        .as_array()
        .expect("cases")
        .iter()
        .enumerate()
    {
        let summary: Vec<Value> = ex["summaries"][c["summary"].as_str().unwrap()]
            .as_array()
            .unwrap()
            .clone();
        let codes: Option<Vec<f64>> = c["codes"]
            .as_array()
            .map(|l| l.iter().map(value_to_number).collect());
        let pi = c["P"].as_u64().unwrap() as usize;
        let got = zoned_capacity(&ps[pi], &summary, codes.as_deref()).to_value();
        ck.check(
            &format!("zonedCapacity[{i}] {} P{pi} {}", c["summary"], c["codes"]),
            &got,
            &c["out"],
        );
    }
    ck.finish("capacity table");
}

#[test]
fn filling_a_zone_from_a_yield_matches_the_prototype() {
    let ex = load_json("expected/census-yield.json");
    let ps = projects(&ex);
    let ids = census_support::ids_for(
        &json!({ "seed": 1, "rngSkip": 0, "seq0": 0, "now": 1792540800000u64 }),
    );
    let mut ck = Checker::default();
    for (i, c) in ex["applyYield"]
        .as_array()
        .expect("cases")
        .iter()
        .enumerate()
    {
        let mut z: Zone = serde_json::from_value(json!({
            "id": "zy", "kind": "zone", "name": "Yield zone", "lat": -36.97, "lng": 174.86, "connectors": null, "lu": { "residential": 9, "retail": 100 },
            "luSrc": { "residential": { "source": "x", "basis": "user", "q": 9 } }, "osm": { "residential": 3 }
        }))
        .unwrap();
        let pi = c["P"].as_u64().unwrap() as usize;
        let y = apply_yield(
            &mut z,
            &ps[pi],
            &ids,
            value_to_number(&c["code"]),
            value_to_number(&c["ha"]),
        );
        ck.check(&format!("applyYield[{i}]: yield"), &y.to_value(), &c["y"]);
        ck.check(
            &format!("applyYield[{i}]: zone"),
            &serde_json::to_value(&z).unwrap(),
            &c["z"],
        );
    }
    ck.finish("filling a zone from a yield");
}
