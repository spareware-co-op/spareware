//! Workshop's materials and a part's mass: every density held to the value its source prints, and a
//! part's mass from the kernel's exact volume.

use std::collections::BTreeSet;

use construct_workshop::{build_part, materials};
use serde_json::{Value, json};

fn mass(part: &Value) -> Value {
    serde_json::to_value(build_part(part)).unwrap()["mass"].clone()
}

fn close(a: &Value, b: f64) -> bool {
    (a.as_f64().unwrap() - b).abs() <= 1e-9 * b.abs().max(1.0)
}

fn at(c: &Value, want: [f64; 3]) {
    for (k, w) in want.iter().enumerate() {
        assert!((c[k].as_f64().unwrap() - w).abs() < 1e-9, "{c}");
    }
}

#[test]
fn every_density_is_its_sources_printed_value_worked_out() {
    let all = materials();
    let mut ids = BTreeSet::new();
    for m in &all.materials {
        assert!(ids.insert(m.id.as_str()), "{} is listed twice", m.id);
        let worked = m
            .worked_out()
            .unwrap_or_else(|| panic!("{}: unit {}", m.id, m.unit));
        // kept to the whole kilogram per cubic metre: within half of one of the value worked out
        assert!(
            (m.density - worked).abs() <= 0.5,
            "{}: {} kg/m³, but {} {} is {worked}",
            m.id,
            m.density,
            m.printed,
            m.unit
        );
        let source = &all.sources[&m.source];
        assert!(source.url.starts_with("https://"), "{}", m.source);
        assert!(
            !m.at.is_empty() && m.at.contains("page"),
            "{}: where in {}",
            m.id,
            m.source
        );
        // metals from the metals handbook, timbers from the wood handbook
        match m.group.as_str() {
            "metal" => assert_eq!(
                (m.source.as_str(), m.unit.as_str()),
                ("mil-hdbk-5j", "lb/in3")
            ),
            "timber" => assert_eq!(
                (m.source.as_str(), m.unit.as_str()),
                ("fpl-gtr-282", "sg12")
            ),
            g => panic!("{}: no group {g}", m.id),
        }
    }
    // the demo's sample parts name materials that are listed
    for id in ["steel", "aluminium", "bronze", "oak", "stainless"] {
        assert!(ids.contains(id), "{id}");
    }
    for s in all.sources.values() {
        assert!(s.status.contains("public"), "{}", s.title);
    }
}

fn block(material: &str) -> Value {
    json!({ "params": [], "material": material, "features": [
        { "id": "s", "type": "sketch", "name": "S", "shape": "rect", "w": 10, "h": 20, "cx": 5, "cy": 10 },
        { "id": "e", "type": "extrude", "name": "E", "sketch": "s", "depth": 4 } ] })
}

#[test]
fn a_prisms_mass_comes_from_the_kernels_exact_volume() {
    let m = mass(&block("aluminium"));
    assert_eq!(m["material"], "aluminium");
    assert_eq!(m["name"], "Aluminium 6061");
    assert!(close(&m["exact"], 800.0));
    assert!(close(&m["from_recipe"], 0.0));
    assert!(close(&m["grams"], 800.0 * 2713.0 / 1e6));
    at(&m["centre"], [5.0, 10.0, 2.0]);
    let said = m["message"].as_str().unwrap();
    assert!(said.contains("the volume is the kernel's, exact"), "{said}");
    assert!(said.contains("MIL-HDBK-5J"), "{said}");
    assert!(said.contains("G247"), "{said}");
    assert!(
        m["source"]
            .as_str()
            .unwrap()
            .ends_with("Table 3.6.2.0(b1), page 3-264")
    );
    // a timber's comes from its specific gravity at 12% moisture
    let m = mass(&block("oak"));
    assert!(close(&m["grams"], 800.0 * 762.0 / 1e6));
    assert!(m["message"].as_str().unwrap().contains("Wood Handbook"));
}

#[test]
fn a_cylinders_volume_is_from_the_recipe_and_says_so() {
    let mut p = block("steel");
    p["features"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "id": "c", "type": "sketch", "name": "C", "shape": "circle", "r": 2, "cx": 30, "cy": 0 }));
    p["features"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "id": "x", "type": "extrude", "name": "X", "sketch": "c", "depth": 4 }));
    let m = mass(&p);
    let round = std::f64::consts::PI * 4.0 * 4.0;
    assert!(close(&m["exact"], 800.0));
    assert!(close(&m["from_recipe"], round));
    assert!(close(&m["grams"], (800.0 + round) * 7861.0 / 1e6));
    let said = m["message"].as_str().unwrap();
    assert!(said.contains("worked out from the recipe"), "{said}");
    // the centre of mass is the volume-weighted centre of both
    let x = (800.0 * 5.0 + round * 30.0) / (800.0 + round);
    assert!(close(&m["centre"][0], x), "{}", m["centre"]);
}

#[test]
fn the_centre_of_mass_follows_each_bodys_map() {
    let mut p = block("steel");
    p["features"].as_array_mut().unwrap().push(
        json!({ "id": "m", "type": "mirror", "name": "M", "of": "e", "plane": "x", "at": 0 }),
    );
    let m = mass(&p);
    assert!(close(&m["exact"], 1600.0));
    at(&m["centre"], [0.0, 10.0, 2.0]);
}

#[test]
fn a_material_that_is_not_listed_gives_no_mass_and_says_why() {
    let m = mass(&block("unobtainium"));
    assert!(m["grams"].is_null());
    assert!(m["density"].is_null());
    assert!(close(&m["exact"], 800.0));
    assert!(
        m["message"]
            .as_str()
            .unwrap()
            .contains("no material called unobtainium")
    );
    // a part that names no material has no mass at all
    let mut p = block("steel");
    p.as_object_mut().unwrap().remove("material");
    assert!(mass(&p).is_null());
    // the sample bracket is steel
    let bracket: Value = serde_json::from_str(include_str!("bracket.json")).unwrap();
    assert_eq!(mass(&bracket)["name"], "Carbon steel, AISI 1025");
}
