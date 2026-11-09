//! Drury sample truth tests: the constants and the steps the build is made of (`clipWays`, `simplify`, `pruneStubs`, `optionA`, `optionB`, `nodeOn`, `siteWith`,
//! `names`, `compass`) against the prototype (tools/transport-truth/gen-sample.mjs).

mod census_support;
mod edit_support;
mod support;

use census_support::ids_for;
use construct_transport::census::Bx;
use construct_transport::datahub::Data;
use construct_transport::edit::{IdSource, new_project};
use construct_transport::importers::{OsmOpts, osm_to_scenario};
use construct_transport::project::{Project, Route, Zone};
use construct_transport::sample::{
    COMMITTED, PROPOSAL, area_value, clip_ways, compass, names, node_on, option_a, option_b,
    prune_stubs, simplify, site_with, tg,
};
use edit_support::{Checker, load_json};
use serde_json::{Value, json};

fn lu(l: &[(&str, f64)]) -> Value {
    Value::Object(
        l.iter()
            .map(|(k, q)| ((*k).to_string(), json!(q)))
            .collect(),
    )
}

#[test]
fn the_constants_are_the_prototypes() {
    let ex = load_json("expected/sample-constants.json");
    let mut ck = Checker::default();
    ck.check(
        "BOUNDS",
        &json!(construct_transport::sample::BOUNDS),
        &ex["BOUNDS"],
    );
    ck.same(
        "ILLUSTRATIVE",
        &construct_transport::sample::ILLUSTRATIVE.to_string(),
        &ex["ILLUSTRATIVE"].as_str().unwrap().to_string(),
    );
    ck.same(
        "LODGEMENT",
        &construct_transport::sample::LODGEMENT.to_string(),
        &ex["LODGEMENT"].as_str().unwrap().to_string(),
    );
    ck.same(
        "JUSTIFY",
        &construct_transport::sample::JUSTIFY.to_string(),
        &ex["JUSTIFY"].as_str().unwrap().to_string(),
    );
    let committed: Vec<Value> = COMMITTED.iter().map(|c| json!({ "key": c.key, "reg": c.reg, "precinct": c.precinct, "name": c.name, "ll": c.ll, "lu": lu(c.lu), "year": c.year, "buildout": c.buildout })).collect();
    ck.check("COMMITTED", &json!(committed), &ex["COMMITTED"]);
    let proposal: Vec<Value> = PROPOSAL
        .iter()
        .map(|c| {
            let mut v =
                json!({ "key": c.key, "name": c.name, "ll": c.ll, "lu": lu(c.lu), "year": c.year });
            if c.tg {
                v["tg"] = json!(true);
            }
            v
        })
        .collect();
    ck.check("PROPOSAL", &json!(proposal), &ex["PROPOSAL"]);
    ck.check("TG", &tg(), &ex["TG"]);
    let _ = area_value();
    ck.finish("sample constants");
}

#[test]
fn the_steps_of_the_build_match_the_prototype() {
    let ex = load_json("expected/sample-helpers.json");
    let roads = load_json("fixtures/data/sample-roads.json");
    let mut ck = Checker::default();
    let mut ids = ids_for(&ex);
    let bx = |b: &Value| {
        Bx::new(
            b["s"].as_f64().unwrap(),
            b["w"].as_f64().unwrap(),
            b["n"].as_f64().unwrap(),
            b["e"].as_f64().unwrap(),
        )
    };
    // clipWays: on the roads and on made-up elements
    let odd = {
        let mut v: Vec<Value> = (1..=4).map(|i| json!({ "type": "node", "id": i, "lat": -37.1, "lon": 174.94 + 0.01 * f64::from(i) })).collect();
        v.extend([
            json!({ "type": "node", "id": "5", "lat": -37.11, "lon": 174.96 }),
            json!({ "type": "node", "id": 6, "lat": -37.09, "lon": 174.96 }),
            json!({ "type": "node", "id": 7 }),
            json!({ "type": "node", "id": 8, "lat": null, "lon": null }),
            json!({ "type": "way", "id": 100, "nodes": [1, 2, 3, 4], "tags": { "highway": "primary" } }),
            json!({ "type": "way", "id": 101, "nodes": [2, 3], "tags": { "highway": "primary" } }),
            json!({ "type": "way", "id": 102, "nodes": [1, 4, 2, 3, 1], "tags": { "highway": "secondary" } }),
            json!({ "type": "way", "id": 103, "nodes": [9, 10] }),
            json!({ "type": "way", "id": 104, "nodes": [] }),
            json!({ "type": "way", "id": 105 }),
            json!({ "type": "way", "id": 106, "nodes": [5, 2, 6], "tags": { "highway": "tertiary" } }),
            json!({ "type": "way", "id": 107, "nodes": [7, 2, 8, 3], "tags": {} }),
            json!({ "type": "way", "id": "108", "nodes": ["5", 5, 2], "tags": {} }),
            json!({ "type": "relation", "id": 9, "members": [] }),
        ]);
        v
    };
    for (i, c) in ex["clip"].as_array().unwrap().iter().enumerate() {
        let els: &[Value] = if c["input"] == "odd" {
            &odd
        } else {
            roads["elements"].as_array().unwrap()
        };
        let r = clip_ways(els, &bx(&c["box"]));
        ck.check(
            &format!("clipWays[{i}] {} {}", c["input"], c["box"]),
            &json!({ "elements": r.elements, "cut": r.cut }),
            &c["out"],
        );
    }
    for c in ex["compass"].as_array().unwrap() {
        let b = c["b"].as_f64().unwrap_or(f64::NAN);
        ck.same(
            &format!("compass {}", c["b"]),
            &compass(b).to_string(),
            &c["out"].as_str().unwrap_or("undefined").to_string(),
        );
    }
    let fresh = |ids: &mut dyn IdSource| -> Project {
        let mut p = new_project(ids, "helpers");
        osm_to_scenario(
            &mut p.scenarios[0],
            ids,
            &roads,
            &OsmOpts {
                sites: true,
                ..Default::default()
            },
        );
        p
    };
    {
        let p = fresh(&mut ids);
        let sc = &p.scenarios[0];
        for n in ex["names"].as_array().unwrap() {
            ck.same(
                &format!("names {}", n["id"]),
                &names(sc, n["id"].as_str().unwrap()),
                &n["out"].as_str().unwrap().to_string(),
            );
        }
        for n in ex["nodeOn"].as_array().unwrap() {
            let ll = [n["ll"][0].as_f64().unwrap(), n["ll"][1].as_f64().unwrap()];
            let max = n["max"].as_f64().unwrap_or(600.0);
            ck.check(
                &format!("nodeOn {} {}", n["road"], n["ll"]),
                &json!(node_on(sc, ll, n["road"].as_str().unwrap(), max)),
                &n["out"],
            );
        }
        for n in ex["siteWith"].as_array().unwrap() {
            let ll = [n["ll"][0].as_f64().unwrap(), n["ll"][1].as_f64().unwrap()];
            let pats: Vec<&str> = n["pats"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap())
                .collect();
            let got = site_with(sc, &pats, ll)
                .map(|h| json!({ "site": h.site, "c": h.c }))
                .unwrap_or(Value::Null);
            ck.check(
                &format!("siteWith {} {}", n["pats"], n["ll"]),
                &got,
                &n["out"],
            );
        }
        let keys: Vec<&str> = sc.sites.get().unwrap().keys().collect();
        ck.check(
            "sites of the made-up network",
            &json!(keys),
            &ex["inputSites"],
        );
    }
    {
        let mut p = fresh(&mut ids);
        let before: Vec<usize> = p.scenarios[0]
            .links
            .values()
            .map(|l| l.pts.get().map_or(0, Vec::len))
            .collect();
        let n = simplify(&mut p.scenarios[0], 2.0);
        let sc = &p.scenarios[0];
        let got = json!({
            "tol": 2, "dropped": n, "before": before, "after": sc.links.values().map(|l| l.pts.get().map_or(0, Vec::len)).collect::<Vec<_>>(),
            "pts": sc.links.values().take(6).map(|l| serde_json::to_value(&l.pts).unwrap()).collect::<Vec<_>>(),
        });
        ck.check("simplify", &got, &ex["simplify"]);
        let mut tol = Vec::new();
        for t in [0.5, 5.0, 50.0] {
            let mut q = fresh(&mut ids);
            tol.push(json!({ "tol": t, "dropped": simplify(&mut q.scenarios[0], t) }));
        }
        ck.check(
            "simplify at other tolerances",
            &json!(tol),
            &ex["simplifyTol"],
        );
    }
    {
        let mut p = fresh(&mut ids);
        let before = p.scenarios[0].links.len();
        let (s, d) = (p.settings.clone(), Data::read(&p));
        let n = prune_stubs(&mut p.scenarios[0], &s, &d);
        let sc = &p.scenarios[0];
        let got = json!({ "before": before, "removed": n, "links": sc.links.keys().collect::<Vec<_>>(), "nodes": sc.nodes.keys().collect::<Vec<_>>(), "control": sc.nodes.values().map(|x| x.control.as_str()).collect::<Vec<_>>() });
        ck.check("pruneStubs", &got, &ex["prune"]);
        let mut q = fresh(&mut ids);
        // the nodes with one road, in the order the roads first reach them
        let mut order: Vec<(String, usize)> = Vec::new();
        for l in q.scenarios[0].links.values() {
            for id in [&l.a, &l.b] {
                match order.iter_mut().find(|(x, _)| x == id) {
                    Some(o) => o.1 += 1,
                    None => order.push((id.clone(), 1)),
                }
            }
        }
        let ends: Vec<String> = order
            .into_iter()
            .filter(|(_, d)| *d == 1)
            .map(|(id, _)| id)
            .collect();
        let z: Zone = serde_json::from_value(json!({ "id": "zKeep", "kind": "zone", "node": ends[0], "lu": { "residential": 5 }, "connectors": null, "lat": 0, "lng": 0 })).unwrap();
        q.scenarios[0].zones.insert("zKeep", z);
        let r: Route = serde_json::from_value(
            json!({ "id": "rKeep", "name": "Bus", "wp": [ends[1]], "freq": 4 }),
        )
        .unwrap();
        q.scenarios[0].routes.insert("rKeep", r);
        q.data = Maybe::Set(serde_json::from_value(json!({ "counts": [{ "id": "user:1", "src": "user", "name": "GREAT SOUTH ROAD", "lat": -37.0985, "lng": 174.9505, "am": 100, "pm": 100 }] })).unwrap());
        let (s, d) = (q.settings.clone(), Data::read(&q));
        let n2 = prune_stubs(&mut q.scenarios[0], &s, &d);
        ck.check("pruneStubs with roads to keep", &json!({ "ends": ends.len(), "removed": n2, "links": q.scenarios[0].links.keys().collect::<Vec<_>>() }), &ex["pruneKept"]);
    }
    {
        let mut p = fresh(&mut ids);
        let made = option_a(&mut p.scenarios[0], &mut ids);
        let sc = &p.scenarios[0];
        let links: Vec<Value> = sc
            .links
            .values()
            .filter(|l| l.stage.is("proposal"))
            .map(|l| serde_json::to_value(l).unwrap())
            .collect();
        let nodes: Vec<Value> = sc
            .nodes
            .values()
            .filter(|n| n.upgrade.truthy() || n.control.is("roundabout"))
            .map(|n| {
                let mut v = json!({ "id": n.id, "control": n.control.as_str() });
                if n.upgrade.truthy() {
                    v["upgrade"] = json!(true);
                }
                v
            })
            .collect();
        ck.check(
            "optionA",
            &json!({ "made": made, "links": links, "nodes": nodes }),
            &ex["optionA"],
        );
        let mut w = fresh(&mut ids);
        let n = option_b(&mut w.scenarios[0]);
        let links: Vec<Value> = w.scenarios[0]
            .links
            .values()
            .filter(|l| l.stage.is("mitigation"))
            .map(|l| serde_json::to_value(l).unwrap())
            .collect();
        ck.check(
            "optionB",
            &json!({ "n": n, "links": links }),
            &ex["optionB"],
        );
        let mut e = new_project(&mut ids, "empty");
        ck.same(
            "optionA on nothing",
            &option_a(&mut e.scenarios[0], &mut ids).len(),
            &(ex["optionAEmpty"].as_u64().unwrap() as usize),
        );
        ck.same(
            "optionB on nothing",
            &option_b(&mut e.scenarios[0]),
            &(ex["optionBEmpty"].as_u64().unwrap() as usize),
        );
    }
    ck.same(
        "ids drawn in step",
        &ids.uid("t"),
        &ex["after"]["id"].as_str().unwrap().to_string(),
    );
    ck.finish("sample steps");
}

use construct_transport::jsnum::Maybe;
