//! Land use truth tests: the constants and the pure functions of features/landuse-plus.js against the prototype (tools/transport-truth/gen-census.mjs).

mod census_support;
mod edit_support;
mod support;

use construct_transport::census::{
    AupData, CensusData, CensusLayers, EmployeeData, Layer, Poly, constants, box_of, card_check, card_json, census_vars_of, lr, parse_aup, parse_census, parse_employees, parse_moe, parse_num,
    resolve, sample_share, Bx,
};
use construct_transport::jsnum::{plain_number, value_to_number};
use construct_transport::census::area_box;
use construct_transport::edit::new_project;
use construct_transport::project::Scenario;
use edit_support::{Checker, load_json};
use serde_json::{Map, Value, json};

fn plain(x: f64) -> Value {
    plain_number(x)
}

fn nums(v: &Value) -> Vec<f64> {
    v.as_array().expect("list").iter().map(value_to_number).collect()
}

fn bx(v: &Value) -> Bx {
    Bx::new(v["s"].as_f64().unwrap(), v["w"].as_f64().unwrap(), v["n"].as_f64().unwrap(), v["e"].as_f64().unwrap())
}

#[test]
fn the_constants_match_the_prototypes_exports() {
    let ex = load_json("expected/census-constants.json");
    let mut ck = Checker::default();
    ck.check("constants", &constants(), &ex);
    ck.finish("land use constants");
}

#[test]
fn quantity_text_and_the_zone_card() {
    let ex = load_json("expected/census-functions.json");
    let mut ck = Checker::default();
    for (i, c) in ex["parseNum"].as_array().expect("cases").iter().enumerate() {
        let input = &c["in"];
        let got = if input.get("nonFinite").is_some() {
            // a number that is not finite cannot be held in JSON: parseNum gives NaN for it
            plain(f64::NAN)
        } else {
            let v = if input.get("undefined").is_some() { Value::Null } else { input.clone() };
            parse_num(&v).map_or(Value::Null, plain)
        };
        ck.check(&format!("parseNum[{i}] {input}"), &got, &c["out"]);
    }
    for (i, c) in ex["cardCheck"].as_array().expect("cases").iter().enumerate() {
        let r = card_check(c["k"].as_str().unwrap(), &c["raw"]);
        ck.check(&format!("cardCheck[{i}]"), &card_json(&r), &c["out"]);
    }
    ck.finish("quantity text");
}

#[test]
fn largest_remainder_and_boxes() {
    let ex = load_json("expected/census-functions.json");
    let mut ck = Checker::default();
    for (i, c) in ex["lr"].as_array().expect("cases").iter().enumerate() {
        let got: Vec<Value> = lr(c["total"].as_f64().unwrap(), &nums(&c["weights"])).into_iter().map(plain).collect();
        ck.check(&format!("lr[{i}] {} {}", c["total"], c["weights"]), &Value::Array(got), &c["out"]);
    }
    for (i, c) in ex["box"].as_array().expect("cases").iter().enumerate() {
        let input = if c["in"].get("undefined").is_some() { Value::Null } else { c["in"].clone() };
        let got = box_of(&input).map_or(Value::Null, |b| {
            let mut v = b.to_value();
            v.as_object_mut().unwrap().remove("basis");
            v
        });
        ck.check(&format!("box[{i}] {}", c["in"]), &got, &c["out"]);
    }
    ck.finish("largest remainder and boxes");
}

#[test]
fn sampled_shares_of_polygons() {
    let ex = load_json("expected/census-functions.json");
    let ans = census_support::answers();
    let mut ck = Checker::default();
    let sa1 = ans["census"]["dw"]["features"].as_array().expect("features");
    for (i, c) in ex["sampleShare"].as_array().expect("cases").iter().enumerate() {
        let f = sa1.iter().find(|f| f["attributes"]["SA12023_V1_00"] == c["code"]).expect("sa1");
        let rings = construct_transport::census::rings_of(f.get("geometry"));
        let poly = Poly::new(rings);
        let n = c["n"].as_u64().map_or(24, |n| n as usize);
        let r = sample_share(&poly, &bx(&c["B"]), n);
        let got = json!({ "share": plain(r.share), "inside": r.inside, "pts": r.pts.iter().map(|p| json!([plain(p[0]), plain(p[1])])).collect::<Vec<_>>() });
        ck.check(&format!("sampleShare[{i}] {} n={}", c["code"], c["n"]), &got, &c["out"]);
    }
    ck.finish("sampled shares");
}

#[test]
fn census_items_are_found_by_alias_then_by_code() {
    let ex = load_json("expected/census-functions.json");
    let mut ck = Checker::default();
    for (i, c) in ex["resolve"].as_array().expect("cases").iter().enumerate() {
        let fields: Vec<Value> = c["fields"].as_array().expect("fields").clone();
        let year = c["year"].as_f64().unwrap_or(2023.0);
        let got = resolve(&fields, &census_vars_of(c["spec"].as_str().unwrap()), year);
        let mut m = Map::new();
        for r in got {
            // an item found by alias in a field with no name has no name key in the prototype's answer
            let v = match (&r.name, r.by) {
                (None, "alias") => json!({ "by": r.by }),
                _ => json!({ "name": r.name, "by": r.by }),
            };
            m.insert(r.key.clone(), v);
        }
        ck.check(&format!("resolve[{i}] {}", c["spec"]), &Value::Object(m), &c["out"]);
    }
    ck.finish("census item resolution");
}

fn poly_part(p: &Poly) -> Value {
    json!({ "bb": p.bb.iter().map(|x| plain(*x)).collect::<Vec<_>>(), "nRings": p.rings.len(), "nPts": p.rings.iter().map(Vec::len).sum::<usize>() })
}

fn merge(a: Value, b: Value) -> Value {
    let mut m = a.as_object().cloned().unwrap_or_default();
    m.extend(b.as_object().cloned().unwrap_or_default());
    Value::Object(m)
}

fn layer(v: &Value) -> Option<Layer> {
    if v.is_null() {
        return None;
    }
    Some(Layer {
        fields: v["meta"]["fields"].as_array().cloned().unwrap_or_default(),
        features: v["features"].as_array().cloned().unwrap_or_default(),
    })
}

fn census_json(c: &CensusData) -> Value {
    json!({
        "sa1": c.sa1.iter().map(|a| merge(json!({ "code": a.code, "v": Value::Object(a.v.clone()) }), poly_part(&a.poly))).collect::<Vec<_>>(),
        "fields": Value::Object(c.fields.iter().map(|r| (r.key.clone(), json!({ "name": r.name, "by": r.by }))).collect()),
        "missing": c.missing,
    })
}

#[test]
fn the_census_layers_are_read_the_same_way() {
    let ex = load_json("expected/census-functions.json");
    let ans = census_support::answers();
    let mut ck = Checker::default();
    let full = |k: &str| {
        let l = &ans["census"][k];
        json!({ "meta": { "fields": l["fields"] }, "features": l["features"] })
    };
    let strip = |l: &Value| json!({ "meta": l["meta"], "features": l["features"].as_array().unwrap().iter().map(|f| json!({ "attributes": f["attributes"] })).collect::<Vec<_>>() });
    let (dw, hh, ind0, ind2) = (full("dw"), full("hh"), full("ind0"), full("ind2"));
    let make = |a: Option<&Value>, b: Option<&Value>, c: Option<&Value>, d: Option<&Value>| CensusLayers {
        dw: a.and_then(layer),
        hh: b.and_then(layer),
        ind0: c.and_then(layer),
        ind2: d.and_then(layer),
    };
    let dw_nogeo = strip(&dw);
    let hh_nogeo = strip(&hh);
    let code_only = json!({ "meta": { "fields": [] }, "features": dw["features"] });
    let cases: Vec<(&str, CensusLayers)> = vec![
        ("all", make(Some(&dw), Some(&hh), Some(&ind0), Some(&ind2))),
        ("noDw", make(None, Some(&hh), Some(&ind0), Some(&ind2))),
        ("noGeometry", make(None, Some(&hh_nogeo), Some(&ind0), Some(&ind2))),
        ("hhFirst", make(Some(&dw_nogeo), Some(&hh), None, None)),
        ("empty", make(None, None, None, None)),
        ("codeOnly", make(Some(&code_only), None, None, None)),
    ];
    for (name, ls) in cases {
        ck.check(&format!("parseCensus {name}"), &census_json(&parse_census(&ls)), &ex["parseCensus"][name]);
    }
    ck.finish("census layers");
}

fn emp_json(e: &EmployeeData) -> Value {
    json!({
        "year": plain(e.year), "field": e.field,
        "sa2": e.sa2.iter().map(|a| merge(json!({ "code": a.code, "name": a.name, "n": a.n.map(plain), "year": plain(a.year), "growth": a.growth.map(plain) }), poly_part(&a.poly))).collect::<Vec<_>>(),
    })
}

fn aup_json(a: &AupData) -> Value {
    json!({
        "zones": a.zones.iter().map(|z| merge(json!({ "code": plain(z.code), "name": z.name, "group": z.group, "status": z.status }), poly_part(&z.poly))).collect::<Vec<_>>(),
        "names": Value::Object(a.names.iter().map(|(k, v)| (k.clone(), json!(v))).collect()),
    })
}

#[test]
fn employees_schools_and_zones_are_read_the_same_way() {
    let ex = load_json("expected/census-functions.json");
    let ans = census_support::answers();
    let mut ck = Checker::default();
    let emp = &ans["employees"];
    let all = json!({ "meta": { "fields": emp["fields"] }, "features": emp["features"] });
    let no_name_fields: Vec<Value> = emp["fields"].as_array().unwrap().iter().filter(|f| f["name"] != "SA22023_V1_00_NAME" && f["name"] != "ec_avperinc").cloned().collect();
    let feats = emp["features"].as_array().unwrap();
    let mut odd_feats = feats.clone();
    odd_feats.push(json!({ "attributes": { "SA22023_V1_00": "", "ec2025": 5 }, "geometry": feats[0]["geometry"] }));
    odd_feats.push(json!({ "attributes": { "SA22023_V1_00": "600999", "ec2025": -3, "ec_avperinc": "x" } }));
    odd_feats.push(json!({ "attributes": { "SA22023_V1_00": "600998", "SA22023_V1_00_NAME": "No count", "ec2025": null, "ec_avperinc": null }, "geometry": feats[1]["geometry"] }));
    let cases = vec![
        ("all", all.clone()),
        ("noName", json!({ "meta": { "fields": no_name_fields }, "features": emp["features"] })),
        ("empty", json!({ "meta": { "fields": [] }, "features": emp["features"] })),
        ("odd", json!({ "meta": { "fields": emp["fields"] }, "features": odd_feats })),
        ("none", json!({ "features": [] })),
    ];
    for (name, l) in cases {
        ck.check(&format!("parseEmployees {name}"), &emp_json(&parse_employees(&layer(&l).unwrap())), &ex["parseEmployees"][name]);
    }
    let moe = |v: &Value, kind: &str| Value::Array(parse_moe(v.as_array().map_or(&[][..], Vec::as_slice), kind).iter().map(|m| m.to_value()).collect());
    let odd_moe = ex["parseMoE"]["oddEce"].clone();
    let _ = odd_moe;
    ck.check("parseMoE schools", &moe(&ans["schools"], "schools"), &ex["parseMoE"]["schools"]);
    ck.check("parseMoE ece", &moe(&ans["ece"], "ece"), &ex["parseMoE"]["ece"]);
    ck.check("parseMoE none", &moe(&Value::Null, "schools"), &ex["parseMoE"]["none"]);
    let odd = json!([
        { "Org_Name": "  Spaced  Name  ", "Latitude": "-36.9", "Longitude": "174.8", "Total": "45" },
        { "Org_Name": "Prop (Proposed opening date:  2031-07-09)", "School_Id": "X1", "Latitude": 1, "Longitude": 2, "Status": "Open" },
        { "Org_Name": "closed one", "Latitude": 1, "Longitude": 2, "Status": "CLOSED" },
        { "Org_Name": "Bad (Proposed opening date: soon)", "Latitude": 3, "Longitude": 4 },
        { "School_Id": 0, "ECE_Id": 77, "Latitude": 5, "Longitude": 6, "All_Children": "12" },
        { "Latitude": "", "Longitude": 3 },
        { "Org_Name": "Two (proposed opening date: 2030-01-01) and (Proposed opening date: 2031-01-01)", "Latitude": 7, "Longitude": 8, "Roll_Date": "2024-12-31T10:00:00Z" },
        { "Org_Name": "N", "School_Id": 5, "Latitude": 1, "Longitude": 2, "Total": 0, "Org_Type": "" }
    ]);
    ck.check("parseMoE oddEce", &moe(&odd, "ece"), &ex["parseMoE"]["oddEce"]);
    ck.check("parseMoE oddSchools", &moe(&odd, "schools"), &ex["parseMoE"]["oddSchools"]);
    let aup = &ans["aup"];
    let aup_all = json!({ "meta": { "fields": aup["fields"] }, "features": aup["features"] });
    let plain_fields: Vec<Value> = aup["fields"].as_array().unwrap().iter().map(|f| json!({ "name": f["name"], "alias": f["alias"] })).collect();
    let mut nd_feats = aup["features"].as_array().unwrap().clone();
    nd_feats.push(json!({ "attributes": { "ZONE": "x" }, "geometry": nd_feats[0]["geometry"] }));
    nd_feats.push(json!({ "attributes": { "ZONE": 18 } }));
    for (name, l) in [("all", aup_all), ("noDomain", json!({ "meta": { "fields": plain_fields }, "features": nd_feats })), ("none", json!({ "features": [] }))] {
        ck.check(&format!("parseAUP {name}"), &aup_json(&parse_aup(&layer(&l).unwrap())), &ex["parseAUP"][name]);
    }
    ck.finish("employees, schools and zones");
}

#[test]
fn the_study_area_comes_from_the_drawn_box_then_the_network_then_the_last_fill() {
    let ex = load_json("expected/census-functions.json");
    let mut ck = Checker::default();
    for (i, c) in ex["areaBox"].as_array().expect("cases").iter().enumerate() {
        let mut p = new_project(&mut census_support::ids_for(&json!({ "seed": 1, "rngSkip": 0, "seq0": 0, "now": 0 })), "area");
        let mut nodes = Map::new();
        for (k, ll) in c.get("nodes").and_then(Value::as_array).into_iter().flatten().enumerate() {
            nodes.insert(format!("n{k}"), json!({ "id": format!("n{k}"), "lat": ll[0], "lng": ll[1] }));
        }
        let mut zones = Map::new();
        for (k, ll) in c.get("zones").and_then(Value::as_array).into_iter().flatten().enumerate() {
            zones.insert(format!("z{k}"), json!({ "id": format!("z{k}"), "kind": if k % 2 == 1 { "gateway" } else { "zone" }, "lat": ll[0], "lng": ll[1] }));
        }
        let sc: Scenario = serde_json::from_value(json!({ "id": "s", "nodes": nodes, "zones": zones })).expect("scenario reads");
        if let Some(o) = c.get("onboarding") {
            p.extra.insert("onboarding".into(), o.clone());
        }
        if let Some(l) = c.get("landuse") {
            p.extra.insert("landuse".into(), l.clone());
        }
        let got = area_box(&p, &sc).map_or(Value::Null, |b| b.to_value());
        ck.check(&format!("areaBox[{i}] {}", c["name"]), &got, &c["out"]);
    }
    ck.finish("study area");
}
