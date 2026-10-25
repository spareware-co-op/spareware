//! Replays the editing operation lists the prototype ran (tools/transport-truth/edit-ops.mjs and edit-scripts.mjs) on the Rust
//! port: the same operations, in the same order, with the same seeded id generator. Each operation's answer and the project
//! afterwards are compared with the recorded ones by the caller.
#![allow(dead_code)]

use construct_transport::edit::Ids;
use construct_transport::edit::history::History;
use construct_transport::edit::net::{self, LinkProps, NodeProps};
use construct_transport::edit::register::{self, PRESET_AS_AT};
use construct_transport::edit::scenarios::{self, CopyFilter, CreateSetOpts};
use construct_transport::edit::sites::{self, DetectOpts, at_points};
use construct_transport::project::{
    Link, Node, Project, RegisterRow, Route, Scenario, Site, Zone, link_programmed, link_unfunded,
    new_project,
};
use engine_sim::{Mulberry32, Rng};
use serde_json::{Value, json};
use std::collections::HashMap;

/// The model being edited, its id generator and the names bound so far.
pub struct Ctx {
    /// The project.
    pub p: Project,
    /// Ids and the clock.
    pub ids: Ids<Mulberry32>,
    /// Results bound by `as`.
    pub env: HashMap<String, Value>,
    /// Undo and redo.
    pub hist: History,
}

/// A fresh model for a recorded file: the seeded generator skipped past what loading the scripts drew, and the counter set.
pub fn ctx_for(file: &Value) -> Ctx {
    ids_for(file).0
}

/// The same, with the generator's seed too.
pub fn ids_for(file: &Value) -> (Ctx, u32) {
    let seed = file["seed"].as_u64().expect("seed") as u32;
    let mut rng = Mulberry32::new(seed);
    for _ in 0..file["rngSkip"].as_u64().expect("rngSkip") {
        rng.next_f64();
    }
    let ids = Ids::new(rng, file["now"].as_f64().expect("now"))
        .with_seq(file["seq0"].as_u64().expect("seq0"));
    (
        Ctx {
            p: new_project("placeholder", "placeholder", "placeholder"),
            ids,
            env: HashMap::new(),
            hist: History::new(),
        },
        seed,
    )
}

/// Resolves a `$name` (or `$name.key`) to the value an earlier operation bound, anywhere in an argument.
pub fn resolve(env: &HashMap<String, Value>, v: &Value) -> Value {
    match v {
        Value::String(s) if s.starts_with('$') => {
            let mut parts = s[1..].split('.');
            let mut cur = env.get(parts.next().expect("a name"));
            for k in parts {
                cur = cur.and_then(|c| c.get(k));
            }
            cur.cloned().unwrap_or(Value::Null)
        }
        Value::Array(a) => Value::Array(a.iter().map(|x| resolve(env, x)).collect()),
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, x)| (k.clone(), resolve(env, x)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Like [`resolve`], and `@n:3` (also `@l`, `@z`, `@r`, `@j`) is the id at that position (counting round) among the active scenario's nodes
/// (links, zones, routes, sites); nothing there gives null.
pub fn resolve_in(ctx: &Ctx, v: &Value) -> Value {
    match v {
        Value::String(s) if s.starts_with('@') => {
            let (kind, i) = s[1..].split_once(':').expect("@kind:index");
            let i: usize = i.parse().expect("an index");
            let sc = &ctx.p.scenarios[active_index(&ctx.p)];
            let keys: Vec<&str> = match kind {
                "n" => sc.nodes.keys().collect(),
                "l" => sc.links.keys().collect(),
                "z" => sc.zones.keys().collect(),
                "r" => sc.routes.keys().collect(),
                "j" => sc
                    .sites
                    .get()
                    .map(|m| m.keys().collect())
                    .unwrap_or_default(),
                other => panic!("unknown selector {other}"),
            };
            if keys.is_empty() {
                Value::Null
            } else {
                json!(keys[i % keys.len()])
            }
        }
        Value::Array(a) => Value::Array(a.iter().map(|x| resolve_in(ctx, x)).collect()),
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, x)| (k.clone(), resolve_in(ctx, x)))
                .collect(),
        ),
        other => resolve(&ctx.env, other),
    }
}

fn num(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().expect("a number"),
        Value::String(s) => match s.as_str() {
            "Infinity" => f64::INFINITY,
            "-Infinity" => f64::NEG_INFINITY,
            "NaN" => f64::NAN,
            _ => panic!("not a number: {s}"),
        },
        Value::Null => f64::NAN,
        other => panic!("not a number: {other}"),
    }
}

fn ll(v: &Value) -> [f64; 2] {
    [num(&v[0]), num(&v[1])]
}

fn strs(v: &Value) -> Vec<String> {
    v.as_array()
        .expect("a list")
        .iter()
        .map(|x| x.as_str().unwrap_or("undefined").to_string())
        .collect()
}

/// A number as the recorded files hold it.
pub fn nv(x: f64) -> Value {
    if x.is_finite() {
        json!(x)
    } else if x.is_nan() {
        json!("NaN")
    } else if x > 0.0 {
        json!("Infinity")
    } else {
        json!("-Infinity")
    }
}

fn pt(p: [f64; 2]) -> Value {
    json!([nv(p[0]), nv(p[1])])
}

/// The index of the active scenario (`Store.sc`), the first when none is.
pub fn active_index(p: &Project) -> usize {
    p.active
        .as_str()
        .and_then(|id| p.scenarios.iter().position(|s| s.id == id))
        .unwrap_or(0)
}

/// `{ ...o, ...set }` then the unset keys deleted, on any typed item (through its JSON text, so key order is kept).
fn patch<T: serde::Serialize + serde::de::DeserializeOwned>(item: &T, a: &Value) -> T {
    use construct_transport::edit::json::JsonValue;
    let mut j = JsonValue::parse(&serde_json::to_string(item).expect("serialises")).expect("JSON");
    if let Some(set) = a.get("set").and_then(Value::as_object) {
        for (k, v) in set {
            j.set(k.clone(), JsonValue::from_value(v));
        }
    }
    if let Some(unset) = a.get("unset").and_then(Value::as_array) {
        for k in unset {
            j.remove(k.as_str().expect("a key"));
        }
    }
    serde_json::from_str(&j.stringify()).expect("reads back")
}

fn node_props(v: Option<&Value>) -> NodeProps {
    let mut j = v.cloned().unwrap_or_else(|| json!({}));
    j.as_object_mut()
        .expect("props")
        .insert("id".into(), json!(""));
    NodeProps(serde_json::from_value::<Node>(j).expect("node props"))
}

fn link_props(v: Option<&Value>) -> LinkProps {
    let mut j = v.cloned().unwrap_or_else(|| json!({}));
    let o = j.as_object_mut().expect("props");
    for k in ["id", "a", "b"] {
        o.insert(k.into(), json!(""));
    }
    LinkProps(serde_json::from_value::<Link>(j).expect("link props"))
}

fn link_pred(name: Option<&str>) -> Option<Box<dyn Fn(&Link) -> bool>> {
    Some(match name? {
        "none" => return None,
        "arterial" => Box::new(|l: &Link| l.class().rank >= 4.0),
        "proposed" => Box::new(|l: &Link| l.status.is("proposed")),
        "notProposed" => Box::new(|l: &Link| !l.status.is("proposed")),
        "hasName" => Box::new(|l: &Link| l.name.as_str().is_some_and(|n| !n.is_empty())),
        "fundedOnly" => Box::new(|l: &Link| !link_unfunded(l) && !link_programmed(l)),
        "keepAll" => Box::new(|_: &Link| true),
        other => panic!("unknown link filter {other}"),
    })
}

fn zone_pred(name: Option<&str>) -> Option<Box<dyn Fn(&Zone) -> bool>> {
    Some(match name? {
        "none" => return None,
        "noGateways" => Box::new(|z: &Zone| !z.kind.is("gateway")),
        "residential" => Box::new(|z: &Zone| {
            z.lu.get()
                .and_then(|lu| lu.get("residential"))
                .is_some_and(|v| v.truthy())
        }),
        other => panic!("unknown zone filter {other}"),
    })
}

/// The scenario as the harness records it after an operation: the active scenario without results, and the order of its keys.
pub fn sc_state(ctx: &Ctx) -> Value {
    let sc = &ctx.p.scenarios[active_index(&ctx.p)];
    let mut v = serde_json::to_value(sc).expect("serialises");
    v.as_object_mut().expect("object").remove("res");
    json!({
        "active": ctx.p.active,
        "scenarios": ctx.p.scenarios.iter().map(|s| s.id.clone()).collect::<Vec<_>>(),
        "sc": v,
        "order": scenario_order(sc),
    })
}

fn scenario_order(sc: &Scenario) -> Value {
    json!({
        "nodes": sc.nodes.keys().collect::<Vec<_>>(),
        "links": sc.links.keys().collect::<Vec<_>>(),
        "zones": sc.zones.keys().collect::<Vec<_>>(),
        "routes": sc.routes.keys().collect::<Vec<_>>(),
        "sites": sc.sites.get().map(|s| s.keys().collect::<Vec<_>>()).unwrap_or_default(),
    })
}

/// The whole project as the harness records it: every scenario without results, no settings.
pub fn project_state(ctx: &Ctx) -> Value {
    let mut v = serde_json::to_value(&ctx.p).expect("serialises");
    let o = v.as_object_mut().expect("object");
    o.remove("settings");
    if let Some(Value::Array(list)) = o.get_mut("scenarios") {
        for s in list {
            s.as_object_mut().expect("object").remove("res");
        }
    }
    let order: Vec<Value> = ctx
        .p
        .scenarios
        .iter()
        .map(|s| {
            let mut o = scenario_order(s);
            o.as_object_mut()
                .expect("object")
                .insert("id".into(), json!(s.id));
            o
        })
        .collect();
    json!({ "P": v, "order": order })
}

/// Which state the harness records after an operation (see edit-ops.mjs).
pub fn default_mode(op: &str) -> Option<&'static str> {
    match op {
        "newProject" | "settings" | "data" | "register" | "presets" | "selectScenario" | "copy"
        | "sensitivityOf" | "createSet" | "snapshot" | "undo" | "redo" => Some("project"),
        "addNode" | "addLink" | "splitLink" | "autoControl" | "deleteLink" | "deleteNode"
        | "reverse" | "setNode" | "setLink" | "setZone" | "setRoute" | "setScenario"
        | "addZone" | "addRoute" | "detect" | "merge" | "splitSite" => Some("sc"),
        _ => None,
    }
}

/// Runs one operation; returns what it returns and the state to compare (if the harness recorded one).
pub fn apply(ctx: &mut Ctx, o: &Value) -> (Value, Option<Value>) {
    let a = resolve_in(ctx, o);
    let name = a["op"].as_str().expect("op name").to_string();
    let ret = run_op(ctx, &name, &a);
    if let Some(b) = a.get("as").and_then(Value::as_str) {
        ctx.env.insert(b.to_string(), ret.clone());
    }
    let mode = match a.get("rec") {
        Some(Value::Bool(false)) => None,
        Some(Value::String(s)) => Some(if s == "sc" { "sc" } else { "project" }),
        _ => default_mode(&name),
    };
    let state = match mode {
        Some("sc") => Some(sc_state(ctx)),
        Some(_) => Some(project_state(ctx)),
        None => None,
    };
    (ret, state)
}

fn opt_str<'a>(a: &'a Value, k: &str) -> Option<&'a str> {
    a.get(k).and_then(Value::as_str)
}

fn run_op(ctx: &mut Ctx, name: &str, a: &Value) -> Value {
    let i = active_index(&ctx.p);
    match name {
        "newProject" => {
            ctx.p = construct_transport::edit::new_project(
                &mut ctx.ids,
                a["name"].as_str().expect("name"),
            );
            json!({ "id": ctx.p.id, "sc": ctx.p.scenarios[0].id })
        }
        "settings" => {
            ctx.p.settings = patch(&ctx.p.settings, &json!({ "set": a["set"] }));
            Value::Null
        }
        "data" => {
            let cur = ctx.p.data.get().cloned();
            let base = match cur {
                Some(d) => serde_json::to_value(&d).expect("serialises"),
                None => json!({}),
            };
            let mut m = base.as_object().cloned().unwrap_or_default();
            for (k, v) in a["set"].as_object().expect("set") {
                m.insert(k.clone(), v.clone());
            }
            ctx.p.data = serde_json::from_value(Value::Object(m)).expect("data");
            Value::Null
        }
        "register" => {
            let rows: Vec<RegisterRow> = serde_json::from_value(a["rows"].clone()).expect("rows");
            ctx.p.register = rows.into();
            Value::Null
        }
        "presets" => {
            let rows = register::presets(opt_str(a, "asAt").unwrap_or(PRESET_AS_AT));
            let v = serde_json::to_value(&rows).expect("serialises");
            ctx.p.register = rows.into();
            v
        }
        "selectScenario" => {
            ctx.p.active = a["id"].as_str().expect("id").to_string().into();
            Value::Null
        }
        "addNode" => {
            let props = node_props(a.get("props"));
            json!(net::add_node(
                &mut ctx.p.scenarios[i],
                &mut ctx.ids,
                ll(&a["ll"]),
                &props
            ))
        }
        "addLink" => {
            let pts: Vec<[f64; 2]> = a
                .get("pts")
                .and_then(Value::as_array)
                .map_or(Vec::new(), |l| l.iter().map(ll).collect());
            let props = link_props(a.get("props"));
            json!(net::add_link(
                &mut ctx.p.scenarios[i],
                &mut ctx.ids,
                a["a"].as_str().expect("a"),
                a["b"].as_str().expect("b"),
                &pts,
                &props
            ))
        }
        "splitLink" => {
            let r = net::split_link(
                &mut ctx.p.scenarios[i],
                &mut ctx.ids,
                a["id"].as_str().expect("id"),
                ll(&a["ll"]),
            );
            json!(r.expect("the road exists"))
        }
        "autoControl" => {
            net::auto_control(&mut ctx.p.scenarios[i], &strs(&a["ids"]));
            Value::Null
        }
        "deleteLink" => {
            net::delete_link(&mut ctx.p.scenarios[i], a["id"].as_str().expect("id"));
            Value::Null
        }
        "deleteNode" => {
            net::delete_node(&mut ctx.p.scenarios[i], a["id"].as_str().expect("id"));
            Value::Null
        }
        "reverse" => {
            net::reverse(&mut ctx.p.scenarios[i], a["id"].as_str().expect("id"))
                .expect("the road exists");
            Value::Null
        }
        "setNode" => {
            let sc = &mut ctx.p.scenarios[i];
            let id = a["id"].as_str().expect("id");
            let n = patch(&sc.nodes[id], a);
            sc.nodes.insert(id, n);
            Value::Null
        }
        "setLink" => {
            let sc = &mut ctx.p.scenarios[i];
            let id = a["id"].as_str().expect("id");
            let l = patch(&sc.links[id], a);
            sc.links.insert(id, l);
            Value::Null
        }
        "setZone" => {
            let sc = &mut ctx.p.scenarios[i];
            let id = a["id"].as_str().expect("id");
            let z = patch(&sc.zones[id], a);
            sc.zones.insert(id, z);
            Value::Null
        }
        "setRoute" => {
            let sc = &mut ctx.p.scenarios[i];
            let id = a["id"].as_str().expect("id");
            let r = patch(&sc.routes[id], a);
            sc.routes.insert(id, r);
            Value::Null
        }
        "setScenario" => {
            ctx.p.scenarios[i] = patch(&ctx.p.scenarios[i], a);
            Value::Null
        }
        "addZone" => {
            let z: Zone = serde_json::from_value(a["zone"].clone()).expect("zone");
            ctx.p.scenarios[i].zones.insert(z.id.clone(), z);
            Value::Null
        }
        "addRoute" => {
            let r: Route = serde_json::from_value(a["route"].clone()).expect("route");
            ctx.p.scenarios[i].routes.insert(r.id.clone(), r);
            Value::Null
        }
        "nearestLink" => {
            let max = a.get("maxM").map_or(f64::INFINITY, num);
            let pred = link_pred(opt_str(a, "filter"));
            let r = net::nearest_link(&ctx.p.scenarios[i], ll(&a["ll"]), max, pred.as_deref());
            match r {
                Some(r) => json!({ "id": r.id, "d": nv(r.d), "p": pt(r.p) }),
                None => Value::Null,
            }
        }
        "nearestNode" => {
            let max = a.get("maxM").map_or(f64::INFINITY, num);
            let sc = &ctx.p.scenarios[i];
            let r = match opt_str(a, "filter") {
                Some("signals") => net::nearest_node(
                    sc,
                    ll(&a["ll"]),
                    max,
                    Some(&|n: &Node| n.control.is("signals")),
                ),
                Some("connected") => {
                    let deg = net::degree(sc);
                    net::nearest_node(
                        sc,
                        ll(&a["ll"]),
                        max,
                        Some(&move |n: &Node| deg.get(&n.id).copied().unwrap_or(0) > 0),
                    )
                }
                _ => net::nearest_node(sc, ll(&a["ll"]), max, None),
            };
            json!(r)
        }
        "project" => {
            let sc = &ctx.p.scenarios[i];
            let l = &sc.links[a["id"].as_str().expect("id")];
            let r = net::project(sc, l, ll(&a["ll"]));
            let mut o = serde_json::Map::new();
            o.insert("d".into(), nv(r.d));
            if let Some(k) = r.i {
                o.insert("i".into(), json!(k));
            }
            o.insert("p".into(), pt(r.p));
            o.insert(
                "g".into(),
                Value::Array(r.g.iter().map(|p| pt(*p)).collect()),
            );
            Value::Object(o)
        }
        "linksAt" => {
            let sc = &ctx.p.scenarios[i];
            json!(
                net::links_at(sc, a["id"].as_str().expect("id"))
                    .iter()
                    .map(|l| l.id.clone())
                    .collect::<Vec<_>>()
            )
        }
        "degree" => {
            let d = net::degree(&ctx.p.scenarios[i]);
            Value::Object(d.into_iter().map(|(k, v)| (k, json!(v))).collect())
        }
        "geomLen" => {
            let sc = &ctx.p.scenarios[i];
            let l = &sc.links[a["id"].as_str().expect("id")];
            json!({ "geom": net::geom(sc, l).iter().map(|p| pt(*p)).collect::<Vec<_>>(), "len": nv(net::len(sc, l)) })
        }
        "detect" => {
            let mut opts = DetectOpts {
                points: at_points(&ctx.p),
                ..Default::default()
            };
            if let Some(r) = a.get("rules").and_then(Value::as_object) {
                let rules = &mut opts.rules;
                for (k, v) in r {
                    match k.as_str() {
                        "near" => rules.near = num(v),
                        "wide" => rules.wide = num(v),
                        "at" => rules.at = num(v),
                        "diameter" => rules.diameter = num(v),
                        "dualBearing" => rules.dual_bearing = num(v),
                        "dualGap" => rules.dual_gap = num(v),
                        "source" => rules.source = v.as_str().expect("text").to_string(),
                        other => panic!("unknown rule {other}"),
                    }
                }
            }
            let r = sites::detect(&mut ctx.p.scenarios[i], &mut ctx.ids, &opts);
            json!({ "sites": r.sites, "nodes": r.nodes, "kept": r.kept })
        }
        "merge" => json!(sites::merge(
            &mut ctx.p.scenarios[i],
            &mut ctx.ids,
            &strs(&a["ids"])
        )),
        "splitSite" => json!(sites::split(
            &mut ctx.p.scenarios[i],
            a["id"].as_str().unwrap_or("undefined")
        )),
        "siteName" => {
            let site: Site =
                serde_json::from_value(json!({ "id": "x", "nodes": a["nodes"], "name": "" }))
                    .expect("site");
            json!(sites::name(&ctx.p.scenarios[i], &site))
        }
        "siteOf" => json!(
            sites::of(
                &ctx.p.scenarios[i],
                a["node"].as_str().unwrap_or("undefined")
            )
            .map(|s| s.id.clone())
        ),
        "copy" => {
            let from = opt_str(a, "from")
                .or(ctx.p.active.as_str())
                .expect("a scenario")
                .to_string();
            let src = ctx
                .p
                .scenarios
                .iter()
                .find(|s| s.id == from)
                .expect("source")
                .clone();
            let (lp, zp) = (
                link_pred(opt_str(a, "links")),
                zone_pred(opt_str(a, "zones")),
            );
            let out = scenarios::copy(
                &src,
                &mut ctx.ids,
                &CopyFilter {
                    links: lp.as_deref(),
                    zones: zp.as_deref(),
                },
            );
            let id = out.id.clone();
            if a.get("push").and_then(Value::as_bool) == Some(true) {
                ctx.p.scenarios.push(out);
            }
            json!({ "id": id })
        }
        "sensitivityOf" => {
            let sc = opt_str(a, "sc")
                .map(str::to_string)
                .or(ctx.p.active.as_str().map(str::to_string))
                .expect("a scenario");
            let unfunded = a.get("unfunded").and_then(Value::as_bool) == Some(true);
            json!(scenarios::sensitivity_of(
                &mut ctx.p,
                &mut ctx.ids,
                &sc,
                unfunded
            ))
        }
        "createSet" => {
            let o = a.get("opts").cloned().unwrap_or_else(|| json!({}));
            let opts = CreateSetOpts {
                pathway: o.get("type").and_then(Value::as_str).map(str::to_string),
                count_year: o.get("countYear").and_then(Value::as_f64),
                opening: o.get("opening").and_then(Value::as_f64),
                design: o.get("design").and_then(Value::as_f64),
                from: o.get("from").and_then(Value::as_str).map(str::to_string),
                mitigation: o.get("mitigation").and_then(Value::as_bool),
                periods: o.get("periods").map(strs),
                buildout: o
                    .get("buildout")
                    .and_then(Value::as_object)
                    .map_or_else(HashMap::new, |m| {
                        m.iter().map(|(k, v)| (k.clone(), num(v))).collect()
                    }),
            };
            let r = scenarios::create_set(&mut ctx.p, &mut ctx.ids, &opts).expect("a set");
            json!({ "scenarios": r.scenarios, "excluded": r.excluded, "warnings": r.warnings, "set": r.set })
        }
        "describe" => {
            let id = opt_str(a, "sc")
                .or(ctx.p.active.as_str())
                .expect("a scenario");
            let sc = ctx
                .p
                .scenarios
                .iter()
                .find(|s| s.id == id)
                .expect("scenario");
            serde_json::to_value(scenarios::describe(sc, &ctx.p)).expect("serialises")
        }
        "describeEach" => Value::Array(
            ctx.p
                .scenarios
                .iter()
                .map(|s| json!({ "id": s.id, "d": scenarios::describe(s, &ctx.p) }))
                .collect(),
        ),
        "checkEach" => Value::Array(
            ctx.p
                .scenarios
                .iter()
                .map(|s| json!({ "id": s.id, "c": scenarios::check(s, &ctx.p) }))
                .collect(),
        ),
        "table" => serde_json::to_value(scenarios::table(&ctx.p)).expect("serialises"),
        "groups" => serde_json::to_value(scenarios::groups(&ctx.p)).expect("serialises"),
        "check" => {
            let id = opt_str(a, "sc")
                .or(ctx.p.active.as_str())
                .expect("a scenario");
            let sc = ctx
                .p
                .scenarios
                .iter()
                .find(|s| s.id == id)
                .expect("scenario");
            serde_json::to_value(scenarios::check(sc, &ctx.p)).expect("serialises")
        }
        "registerTable" => Value::Array(register::table(&ctx.p)),
        "inBaseline" => {
            let row: Option<RegisterRow> = match &a["row"] {
                Value::Null => None,
                v => Some(serde_json::from_value(v.clone()).expect("row")),
            };
            let (included, reason) = register::in_baseline(row.as_ref());
            json!({ "included": included, "reason": reason })
        }
        "zoneRole" => json!(scenarios::zone_role(
            &ctx.p.scenarios[i].zones[a["id"].as_str().expect("id")]
        )),
        "linkStage" => json!(scenarios::stage_of(
            &ctx.p.scenarios[i].links[a["id"].as_str().expect("id")]
        )),
        "designYear" => json!(scenarios::design_year(
            a["type"].as_str().expect("type"),
            a.get("opening").map_or(f64::NAN, num)
        )),
        "snapshot" => {
            ctx.hist.snapshot(&ctx.p);
            Value::Null
        }
        "undo" => json!(ctx.hist.undo(&mut ctx.p)),
        "redo" => json!(ctx.hist.redo(&mut ctx.p)),
        "historyLength" => json!({ "undo": ctx.hist.undo_len(), "redo": ctx.hist.redo_len() }),
        other => panic!("unknown operation {other}"),
    }
}
