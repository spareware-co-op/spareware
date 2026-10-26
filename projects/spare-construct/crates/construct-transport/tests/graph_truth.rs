//! Graph truth tests: buildGraph, turns, shortest-path trees, bus routes, connector maps and site flows against the prototype.
//!
//! tools/transport-truth/gen-graph.mjs ran the JavaScript engine on every project in inputs/
//! (each scenario and period, and variants that reach more branches) and wrote
//! expected/graph-<name>.json; these tests rebuild the same things in Rust and compare.

mod support;
mod graph_dump;

use construct_transport::graph::{
    approach_name, build_graph, connector_map, js_sort_by, speed_of, toll_of, turn_type, zone_class,
};
use construct_transport::jsnum::value_to_number;
use construct_transport::project::{Link, Zone, road_class_exact};
use construct_transport::standards::STANDARDS;
use graph_dump::{Checker, case_inputs, graph_value, n};
use serde_json::{Value, json};
use support::{PROJECTS, load_expected, load_json, load_project};

fn opt_path(p: Option<Vec<usize>>) -> Value {
    p.map_or(Value::Null, |p| json!(p))
}

#[test]
fn graphs_match_the_prototype() {
    let mut ck = Checker::default();
    for name in PROJECTS {
        let p = load_project(name);
        let ex = load_json(&format!("expected/graph-{name}.json"));
        for (ci, case) in ex["cases"].as_array().expect("cases").iter().enumerate() {
            let (sc, s) = case_inputs(&p, &ex, case);
            let period = case["period"].as_str();
            let at = format!("{name}.cases[{ci}]({}, {})", sc.id, period.unwrap_or("none"));
            let g = build_graph(&sc, &s, period);
            ck.check(&format!("{at}.graph"), &graph_value(&g), &case["graph"]);
            // Turn penalties, bans and movements for every arc pair, in turn-table order
            let (mut tp, mut tb, mut mv) = (Vec::new(), Vec::new(), Vec::new());
            for a in 0..g.m {
                for t in g.search.turns.of(a) {
                    let b = g.t_to()[t] as usize;
                    tp.push(n(g.turn_penalty(a, b)));
                    tb.push(json!(g.turn_banned(a, b)));
                    mv.push(g.movement(a, b).map_or(Value::Null, |m| json!(m)));
                }
            }
            ck.check(&format!("{at}.turns"), &json!({ "tp": tp, "tb": tb, "mv": mv }), &case["turns"]);
            // Shortest-path trees
            let mut w = g.workspace();
            for (k, tr) in case["spt"].as_array().expect("spt").iter().enumerate() {
                let src = tr["src"].as_u64().expect("src") as usize;
                let target = tr["target"].as_i64().expect("target") as i32;
                let from = tr["fromArc"].as_i64().expect("fromArc") as i32;
                g.spt(&g.ffc, src, &mut w, target, from);
                let paths: Vec<Value> = (0..g.n).map(|v| opt_path(g.path_to(&w, v))).collect();
                let got = json!({ "src": src, "target": target, "fromArc": from,
                    "dist": w.dist.iter().map(|&x| n(x)).collect::<Vec<_>>(), "pred": w.pred, "pt": w.pt,
                    "nd": w.nd.iter().map(|&x| n(x)).collect::<Vec<_>>(), "na": w.na, "paths": paths });
                ck.check(&format!("{at}.spt[{k}]"), &got, tr);
            }
            // The node-array interface
            let d = &case["dijkstra"];
            let s0 = d["src"].as_u64().expect("src") as usize;
            let mut tree = g.tree();
            g.dijkstra(&g.ffc, s0, &mut tree, -1);
            let trace: Vec<Value> = (0..g.n).map(|v| opt_path(g.trace(&tree, s0, v))).collect();
            let fresh = g.tree();
            let got = json!({ "src": s0, "dist": tree.dist.iter().map(|&x| n(x)).collect::<Vec<_>>(), "pred": tree.pred,
                "trace": trace, "wrongSrc": opt_path(g.trace(&tree, s0 + 1, 0)), "foreign": opt_path(g.trace(&fresh, s0, 0)) });
            ck.check(&format!("{at}.dijkstra"), &got, d);
            // Bus routes and connector maps
            let routes: Vec<Value> = sc.routes.values().map(|r| json!(g.resolve_route(r, None))).collect();
            ck.check(&format!("{at}.routes"), &json!(routes), &case["routes"]);
            let cm = connector_map(&sc, &p.settings);
            let cmv: serde_json::Map<String, Value> = cm.iter().map(|(k, v)| (k.to_string(), json!(v))).collect();
            ck.check(&format!("{at}.connectorMap"), &Value::Object(cmv), &case["connectorMap"]);
            // Traced site flows rebuilt from a result's siteTurns
            if let Some(sf) = case.get("siteFlows") {
                let st = &sf["siteTurns"];
                let f = |x: Option<Vec<f64>>| x.map_or(Value::Null, |v| json!(v.iter().map(|&y| n(y)).collect::<Vec<_>>()));
                let got = json!({ "siteTurns": st, "f1": f(construct_transport::graph::site_flows(&g, Some(st), 1.0)),
                    "f2": f(construct_transport::graph::site_flows(&g, Some(st), 1.5)) });
                ck.check(&format!("{at}.siteFlows"), &got, sf);
            }
        }
    }
    ck.finish("graph truth");
}

#[test]
fn standards_match_the_prototype() {
    let mut ck = Checker::default();
    let v = STANDARDS.to_value();
    for name in PROJECTS {
        ck.check(&format!("{name}.standards"), &v, &load_expected(name)["standards"]);
    }
    ck.check("junction-functions.standards", &v, &load_json("expected/junction-functions.json")["standards"]);
    ck.finish("standards");
}

#[test]
fn graph_helpers_match_the_prototype() {
    let f = load_json("expected/junction-functions.json");
    let mut ck = Checker::default();
    for (i, c) in f["approachName"].as_array().expect("cases").iter().enumerate() {
        let inb = value_to_number(&c["in"][0]);
        let eight = c["in"][1].as_bool().expect("eight");
        ck.check(&format!("approachName[{i}]"), &json!(approach_name(inb, eight)), &c["out"]);
    }
    for (i, c) in f["turnType"].as_array().expect("cases").iter().enumerate() {
        let a = c["in"][0].as_f64().expect("a");
        let b = c["in"][1].as_f64().expect("b");
        ck.check(&format!("turnType[{i}]"), &json!(turn_type(a, b)), &c["out"]);
    }
    for (i, c) in f["zoneClass"].as_array().expect("cases").iter().enumerate() {
        let z: Zone = serde_json::from_str(c["in"].as_str().expect("zone")).expect("zone");
        ck.check(&format!("zoneClass[{i}]"), &json!(zone_class(Some(&z))), &c["out"]);
    }
    let ramp = road_class_exact("ramp").expect("ramp");
    for (i, c) in f["tollSpeed"].as_array().expect("cases").iter().enumerate() {
        let l: Link = serde_json::from_str(c["in"]["L"].as_str().expect("link")).expect("link");
        let period = c["in"]["period"].as_str();
        ck.check(&format!("tollSpeed[{i}].speed"), &n(speed_of(&l, ramp, period)), &c["speed"]);
        ck.check(&format!("tollSpeed[{i}].toll"), &n(toll_of(&l, period)), &c["toll"]);
    }
    for (i, c) in f["sort"].as_array().expect("cases").iter().enumerate() {
        let keys: Vec<f64> = c["keys"].as_array().expect("keys").iter().map(|k| support::as_number(k).expect("key")).collect();
        let mut up: Vec<usize> = (0..keys.len()).collect();
        js_sort_by(&mut up, |&a, &b| keys[a] - keys[b]);
        let mut down: Vec<usize> = (0..keys.len()).collect();
        js_sort_by(&mut down, |&a, &b| keys[b] - keys[a]);
        ck.check(&format!("sort[{i}].up"), &json!(up), &c["up"]);
        ck.check(&format!("sort[{i}].down"), &json!(down), &c["down"]);
    }
    ck.finish("graph helpers");
}
