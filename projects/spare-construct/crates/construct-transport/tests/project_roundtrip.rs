//! The typed project model against the truth-test inputs and the prototype's model.js.
//!
//! Both saved projects read into `Project`, write back to the same JSON value, keep the
//! file's key order for nodes, links, zones, routes and sites, and the defaults and
//! constructors match what model.js makes (expected/util-cases.json, "model").

mod support;

use construct_transport::jsnum::{JsNum, Maybe};
use construct_transport::project::{
    Link, Node, OrderedMap, Project, RegisterRow, Scenario, default_settings, in_baseline,
    link_programmed, link_stage, link_unfunded, new_project, new_scenario,
};
use serde::Deserialize;
use serde::de::{Deserializer, IgnoredAny, MapAccess, Visitor};
use serde_json::{Value, json};
use std::fmt;
use support::{PROJECTS, input_text, load_input, load_json, load_project};

// Key order of a JSON object read straight from the text, independent of OrderedMap
struct KeyOrder(Vec<String>);

impl<'de> Deserialize<'de> for KeyOrder {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = KeyOrder;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<KeyOrder, A::Error> {
                let mut keys = Vec::new();
                while let Some(k) = a.next_key::<String>()? {
                    a.next_value::<IgnoredAny>()?;
                    keys.push(k);
                }
                Ok(KeyOrder(keys))
            }
        }
        d.deserialize_map(V)
    }
}

#[derive(Deserialize)]
struct RawProject {
    scenarios: Vec<RawScenario>,
}

#[derive(Deserialize)]
struct RawScenario {
    id: String,
    nodes: KeyOrder,
    links: KeyOrder,
    zones: KeyOrder,
    routes: KeyOrder,
    #[serde(default)]
    sites: Option<KeyOrder>,
}

// Paths where two values differ exactly (number representation included)
fn strict_diff(a: &Value, b: &Value, path: &str, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for (k, v) in y {
                match x.get(k) {
                    Some(w) => strict_diff(w, v, &format!("{path}.{k}"), out),
                    None => out.push(format!("{path}.{k}: missing after the round trip")),
                }
            }
            for k in x.keys().filter(|k| !y.contains_key(*k)) {
                out.push(format!("{path}.{k}: added by the round trip"));
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            for (i, (w, v)) in x.iter().zip(y).enumerate() {
                strict_diff(w, v, &format!("{path}[{i}]"), out);
            }
        }
        _ if a != b => out.push(format!("{path}: wrote {a}, read {b}")),
        _ => {}
    }
}

fn assert_same(actual: &Value, expected: &Value, what: &str) {
    let mut d = Vec::new();
    strict_diff(actual, expected, "$", &mut d);
    assert!(
        d.is_empty(),
        "{what}: {} difference(s):\n  {}",
        d.len(),
        d.iter().take(30).cloned().collect::<Vec<_>>().join("\n  ")
    );
}

// JavaScript lists array-index keys ("0" to "4294967294") before other keys; OrderedMap assumes there are none
fn is_array_index(k: &str) -> bool {
    !k.is_empty()
        && k.bytes().all(|c| c.is_ascii_digit())
        && (k == "0" || !k.starts_with('0'))
        && k.parse::<u64>().is_ok_and(|n| n < 4294967295)
}

#[test]
fn both_inputs_read_as_typed_projects() {
    for name in PROJECTS {
        let p = load_project(name);
        assert!(p.scenarios.len() >= 2, "{name}: scenarios");
        assert_eq!(p.settings.sv.num(), 3.0, "{name}: settings version");
        assert!(
            p.settings.rates.get().is_some_and(|r| r.len() == 14),
            "{name}: trip rates"
        );
        for sc in &p.scenarios {
            assert!(
                !sc.nodes.is_empty() && !sc.links.is_empty() && !sc.zones.is_empty(),
                "{name}/{}: network",
                sc.id
            );
            for (k, n) in sc.nodes.iter() {
                assert_eq!(k, n.id);
                assert!(n.lat.num().is_finite() && n.lng.num().is_finite());
            }
            for (k, l) in sc.links.iter() {
                assert_eq!(k, l.id);
                assert!(
                    sc.nodes.contains_key(&l.a) && sc.nodes.contains_key(&l.b),
                    "{name}/{}: link {k} ends",
                    sc.id
                );
                assert!(sc.link_len(l) > 0.0);
            }
            for (k, z) in sc.zones.iter() {
                assert_eq!(k, z.id);
            }
        }
        assert!(p.active_scenario().is_some() && p.base_scenario().is_some());
        // Every key in the inputs has a typed field (a misspelt field name would leave its key in extra)
        let mut extras: Vec<String> = Vec::new();
        let mut note = |what: String, m: &serde_json::Map<String, Value>| {
            extras.extend(m.keys().map(|k| format!("{what}.{k}")));
        };
        note("project".into(), &p.extra);
        note("settings".into(), &p.settings.extra);
        for sc in &p.scenarios {
            note(format!("scenario {}", sc.id), &sc.extra);
            sc.nodes
                .values()
                .for_each(|n| note(format!("node {}", n.id), &n.extra));
            sc.links
                .values()
                .for_each(|l| note(format!("link {}", l.id), &l.extra));
            sc.zones
                .values()
                .for_each(|z| note(format!("zone {}", z.id), &z.extra));
            sc.routes
                .values()
                .for_each(|r| note(format!("route {}", r.id), &r.extra));
            if let Some(sites) = sc.sites.get() {
                sites
                    .values()
                    .for_each(|s| note(format!("site {}", s.id), &s.extra));
            }
        }
        assert!(extras.is_empty(), "{name}: keys not typed: {extras:?}");
        let sc = &p.scenarios[0];
        let typed =
            sc.links.values().any(|l| l.toll.is_set()) || sc.zones.values().any(|z| z.gw.is_set());
        assert!(typed, "{name}: typed fields read");
    }
}

#[test]
fn round_trip_keeps_every_value() {
    for name in PROJECTS {
        let raw = load_input(name);
        let p = load_project(name);
        let back = p.to_value();
        assert_same(&back, &raw, &format!("{name}: typed project written back"));
        // Read again from text: a serde_json::Value object is sorted by key, so reading from one loses the order
        let text = serde_json::to_string(&p).expect("writes");
        let again = Project::from_json(&text).expect("reads again");
        assert_eq!(again, p, "{name}: read again (order included)");
        assert_same(
            &again.to_value(),
            &back,
            &format!("{name}: second round trip"),
        );
        let reread: Value = serde_json::from_str(&text).expect("JSON");
        assert_same(&reread, &raw, &format!("{name}: through text"));
        let serialised: Value = serde_json::from_str(&p.serialise()).expect("JSON");
        assert_same(&serialised, &raw, &format!("{name}: Store.serialise form"));
    }
}

#[test]
fn iteration_order_follows_the_file() {
    for name in PROJECTS {
        let raw: RawProject = serde_json::from_str(&input_text(name)).expect("raw key order");
        let p = load_project(name);
        assert_eq!(raw.scenarios.len(), p.scenarios.len());
        for (rs, sc) in raw.scenarios.iter().zip(&p.scenarios) {
            assert_eq!(rs.id, sc.id);
            let check = |what: &str, got: Vec<&str>, want: &KeyOrder| {
                assert_eq!(
                    got,
                    want.0.iter().map(String::as_str).collect::<Vec<_>>(),
                    "{name}/{}: {what} order",
                    sc.id
                );
                for k in &want.0 {
                    assert!(
                        !is_array_index(k),
                        "{name}/{}: {what} id {k} is integer-like (JavaScript would list it first)",
                        sc.id
                    );
                }
            };
            check("nodes", sc.nodes.keys().collect(), &rs.nodes);
            check("links", sc.links.keys().collect(), &rs.links);
            check("zones", sc.zones.keys().collect(), &rs.zones);
            check("routes", sc.routes.keys().collect(), &rs.routes);
            match (&rs.sites, sc.sites.get()) {
                (Some(want), Some(got)) => check("sites", got.keys().collect(), want),
                (None, None) => {}
                _ => panic!("{name}/{}: sites present in one and not the other", sc.id),
            }
        }
    }
}

#[test]
fn ordered_map_behaves_like_a_javascript_object() {
    let mut m: OrderedMap<i32> = OrderedMap::new();
    m.insert("b", 1);
    m.insert("a", 2);
    m.insert("c", 3);
    assert_eq!(m.insert("a", 20), Some(2));
    assert_eq!(m.keys().collect::<Vec<_>>(), ["b", "a", "c"]);
    assert_eq!(m.remove("b"), Some(1));
    m.insert("b", 4);
    assert_eq!(m.keys().collect::<Vec<_>>(), ["a", "c", "b"]);
    assert_eq!((m["a"], m["c"], m["b"]), (20, 3, 4));
    assert_eq!(m.index_of("b"), Some(2));
    m.retain(|k, _| k != "c");
    assert_eq!(m.iter().collect::<Vec<_>>(), [("a", &20), ("b", &4)]);
    assert_eq!(m.get("c"), None);
    // JSON.parse keeps a repeated key at its first place with its last value
    let d: OrderedMap<i32> = serde_json::from_str(r#"{"x":1,"y":2,"x":3}"#).unwrap();
    assert_eq!(d.iter().collect::<Vec<_>>(), [("x", &3), ("y", &2)]);
    assert_eq!(serde_json::to_string(&d).unwrap(), r#"{"x":3,"y":2}"#);
}

#[test]
fn loose_fields_read_as_javascript_does() {
    let text = r#"{"id":"l","a":"n1","b":"n2","name":null,"lanesAB":"2","lanesBA":null,"speed":"","capPerLane":true,
        "busLane":"yes","ring":0,"toll":{"AM":30},"pts":[[-36.9,174],[-36.91,174.5]],"custom":{"k":[1,"x"]}}"#;
    let raw: Value = serde_json::from_str(text).unwrap();
    let l: Link = serde_json::from_str(text).unwrap();
    assert_eq!((l.lanes_ab.num(), l.lanes_ab.int32()), (2.0, 2));
    assert!(l.lanes_ba.is_null() && l.lanes_ba.num() == 0.0 && l.lanes_ba.int32() == 0);
    assert_eq!((l.speed.num(), l.speed.or(50.0)), (0.0, 50.0));
    assert_eq!(l.cap_per_lane.num(), 1.0);
    assert!(l.bus_lane.truthy() && !l.bus_lane.is_true());
    assert!(!l.ring.truthy());
    assert_eq!(l.name.str_or("Unnamed"), "Unnamed");
    assert!(l.status.is_missing() && l.cycle.is_nullish());
    assert!(l.var_speed.is_missing());
    assert_eq!(l.class().key, "local");
    assert_eq!(l.extra.get("custom"), Some(&json!({"k": [1, "x"]})));
    assert_same(
        &serde_json::to_value(&l).unwrap(),
        &raw,
        "loose link written back",
    );
    let n: Node =
        serde_json::from_str(r#"{"id":"n","lat":"-36.9","lng":174.5,"allowU":true}"#).unwrap();
    assert_eq!(n.ll(), [-36.9, 174.5]);
    assert!(n.allow_u.is_true() && n.control.is_missing());
    assert!(matches!(n.lat, Maybe::Set(JsNum::Text(_))));
}

#[test]
fn defaults_and_constructors_match_the_prototype() {
    let cases = load_json("expected/util-cases.json");
    let m = &cases["model"];
    assert_same(
        &serde_json::to_value(default_settings()).unwrap(),
        &m["defaultSettings"],
        "defaultSettings()",
    );
    assert_same(
        &new_project("p1", "Untitled project", "s1").to_value(),
        &m["newProject"],
        "newProject()",
    );
    assert_same(
        &serde_json::to_value(new_scenario("s2", "Option")).unwrap(),
        &m["newScenario"],
        "newScenario()",
    );
    let n1 = Node::new("n1", [-36.84851234567, 174.76331234567]);
    let mut n2 = Node::new("n2", [-36.85, 174.77]);
    n2.control = Maybe::Set("signals".to_string());
    assert_same(
        &serde_json::to_value(&n1).unwrap(),
        &m["addNode"][0],
        "Net.addNode",
    );
    assert_same(
        &serde_json::to_value(&n2).unwrap(),
        &m["addNode"][1],
        "Net.addNode with control",
    );
    let pts = [[-36.8491111111111, 174.765555555555], [-36.8495, 174.7688]];
    for (i, c) in m["addLink"].as_array().unwrap().iter().enumerate() {
        let l = Link::new(&format!("l{i}"), "n1", "n2", &pts, c["cls"].as_str());
        assert_same(
            &serde_json::to_value(&l).unwrap(),
            &c["link"],
            &format!("Net.addLink {}", c["cls"]),
        );
    }
    let mut sc: Scenario = serde_json::from_value(m["geomScenario"].clone()).unwrap();
    for g in m["geom"].as_array().unwrap() {
        let l = &sc.links[g["link"].as_str().unwrap()];
        let want: Vec<[f64; 2]> = serde_json::from_value(g["geom"].clone()).unwrap();
        assert_eq!(sc.geom(l), want, "Net.geom {}", l.id);
        assert_eq!(
            sc.link_len(l),
            g["len"].as_f64().unwrap(),
            "Net.len {}",
            l.id
        );
    }
    sc.add_node("n3", [-36.91, 174.77]);
    sc.add_link("l3", "n2", "n3", &[], Some("arterial")).name = Maybe::Set("New".to_string());
    assert_eq!(sc.links.keys().last(), Some("l3"));
    assert_eq!(sc.links["l3"].lanes_ab.num(), 2.0);
    for c in m["linkStage"].as_array().unwrap() {
        let l: Link = serde_json::from_value(c["link"].clone()).unwrap();
        assert_eq!(
            link_stage(&l),
            c["stage"].as_str().unwrap(),
            "linkStage {}",
            l.id
        );
        assert_eq!(
            link_unfunded(&l),
            c["unfunded"].as_bool().unwrap(),
            "unfunded {}",
            l.id
        );
        assert_eq!(
            link_programmed(&l),
            c["programmed"].as_bool().unwrap(),
            "programmed {}",
            l.id
        );
    }
    for c in m["inBaseline"].as_array().unwrap() {
        let row: Option<RegisterRow> = serde_json::from_value(c["row"].clone()).unwrap();
        let (included, reason) = in_baseline(row.as_ref());
        assert_eq!(
            included,
            c["included"].as_bool().unwrap(),
            "inBaseline {}",
            c["row"]
        );
        assert_eq!(
            reason,
            c["reason"].as_str().unwrap(),
            "inBaseline reason {}",
            c["row"]
        );
    }
}
