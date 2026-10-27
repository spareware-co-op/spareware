//! Graph truth-test helpers: the graph and its controllers as the JSON tools/transport-truth/gen-graph.mjs writes, and a checker that counts passes.
#![allow(dead_code)] // each test file uses its own part of this module

use super::support::diff_json;
use construct_transport::graph::{Arc, Ctl, Graph};
use construct_transport::jsnum::plain_number;
use construct_transport::junction::{Lane, MoveEntry};
use construct_transport::project::{Project, Scenario, Settings};
use serde_json::{Map, Value, json};
use std::fmt::Write as _;

/// A number as the truth files hold it.
pub fn n(x: f64) -> Value {
    plain_number(x)
}

fn nums(xs: &[f64]) -> Value {
    Value::Array(xs.iter().map(|&x| n(x)).collect())
}

fn opt_n(x: Option<f64>) -> Value {
    x.map_or(Value::Null, n)
}

fn idx_list(xs: &[usize]) -> Value {
    json!(xs)
}

fn pts(p: &[[f64; 2]]) -> Value {
    Value::Array(p.iter().map(|q| json!([n(q[0]), n(q[1])])).collect())
}

/// An arc as gen-graph.mjs arcOut writes it.
pub fn arc_value(a: &Arc) -> Value {
    let c = a.conn;
    let mut m = Map::new();
    let mut put = |k: &str, v: Value| {
        m.insert(k.to_string(), v);
    };
    put("i", json!(a.i));
    put("key", json!(a.key));
    put("link", if c { Value::Null } else { json!(a.link) });
    put("dir", if c { Value::Null } else { json!(a.dir) });
    put("conn", json!(c));
    put("from", json!(a.from));
    put("to", json!(a.to));
    put("len", n(a.len));
    put("lanes", json!(a.lanes));
    put("gl", json!(a.gl));
    put("bus", if c { Value::Null } else { json!(a.bus) });
    put("two", if c { Value::Null } else { json!(a.two) });
    put("ff", n(a.ff));
    put("capMid", n(a.cap_mid));
    put("cls", a.cls.as_ref().map_or(Value::Null, |s| json!(s)));
    put("rank", n(a.rank));
    put("speed", if c { Value::Null } else { n(a.speed) });
    put("lw", if c { Value::Null } else { n(a.lw) });
    put("ring", if c { Value::Null } else { json!(a.ring) });
    put("name", if c { Value::Null } else { json!(a.name) });
    put("pts", pts(&a.pts));
    put("cum", if c { Value::Null } else { nums(&a.cum) });
    put("inB", n(a.in_b));
    put("outB", n(a.out_b));
    put("ctrl", json!(a.ctrl.as_str()));
    put("internal", json!(a.internal));
    put("toll", if a.toll != 0.0 { n(a.toll) } else { Value::Null });
    put("zone", a.zone.as_ref().map_or(Value::Null, |z| json!(z)));
    put("node", a.node.map_or(Value::Null, |x| json!(x)));
    put("phase", a.phase.map_or(Value::Null, |x| json!(x)));
    put("C", opt_n(a.c));
    put("gC", opt_n(a.g_c));
    put("major", json!(a.major));
    put("stop", json!(a.stop));
    put(
        "conf",
        a.conf.as_ref().map_or(Value::Null, |cf| {
            Value::Array(cf.iter().map(|&(j, f)| json!([j, n(f)])).collect())
        }),
    );
    put("ringIn", a.ring_in.as_ref().map_or(Value::Null, |r| idx_list(r)));
    put("ringOut", a.ring_out.as_ref().map_or(Value::Null, |r| idx_list(r)));
    put("lmSrc", a.lm_src.clone().unwrap_or(Value::Null));
    Value::Object(m)
}

fn lanes_value(l: &[Lane]) -> Value {
    Value::Array(l.iter().map(Lane::to_value).collect())
}

/// A movement entry as gen-graph.mjs entOut writes it.
pub fn ent_value(e: &MoveEntry) -> Value {
    json!({
        "i": e.i, "major": e.major, "inB": n(e.in_b), "leg": e.leg, "gl": n(e.gl), "sLane": n(e.s_lane), "stop": e.stop,
        "gaps": e.gaps.clone().unwrap_or(Value::Null), "lanes": lanes_value(&e.lanes), "median": e.median, "slip": e.slip,
        "lmSrc": e.lm_src, "thru": n(e.thru),
        "mvT": e.mv_t.as_ref().map_or(Value::Null, |m| Value::Array(m.iter().map(|(t, mv, leg)| json!([t, mv, leg])).collect())),
        "nc": opt_n(e.nc),
    })
}

/// A controller as gen-graph.mjs ctlOut writes it.
pub fn ctl_value(c: &Ctl) -> Value {
    let sig = c.signal.as_ref();
    let site = c.site.as_ref();
    let map_obj = |m: &std::collections::HashMap<usize, usize>| {
        let mut o = Map::new();
        for (k, v) in m {
            o.insert(k.to_string(), json!(v));
        }
        Value::Object(o)
    };
    json!({
        "n": c.n, "type": c.kind, "arcs": c.arcs,
        "single": sig.map_or(Value::Null, |s| json!(s.single)),
        "C": sig.map_or(Value::Null, |s| n(s.c)),
        "lost": sig.map_or(Value::Null, |s| n(s.lost)),
        "split": sig.map_or(Value::Null, |s| nums(&s.split)),
        "offset": sig.map_or(Value::Null, |s| json!(s.offset)),
        "major": c.major.as_ref().map_or(Value::Null, |m| json!(m)),
        "src": c.src.id,
        "site": site.map_or(Value::Null, |s| json!(s.site)),
        "members": site.map_or(Value::Null, |s| json!(s.members)),
        "exits": site.map_or(Value::Null, |s| json!(s.exits)),
        "legs": site.map_or(Value::Null, |s| Value::Array(s.legs.iter().map(|l| json!({
            "k": l.k, "b": n(l.b), "name": l.name, "rank": n(l.rank), "ins": l.ins, "outs": l.outs })).collect())),
        "legIn": site.map_or(Value::Null, |s| map_obj(&s.leg_in)),
        "legOut": site.map_or(Value::Null, |s| map_obj(&s.leg_out)),
        "ent": c.ent.as_ref().map_or(Value::Null, |e| Value::Array(e.iter().map(ent_value).collect())),
    })
}

/// The graph as gen-graph.mjs graphOut writes it.
pub fn graph_value(g: &Graph) -> Value {
    let int_list = |v: &[i32]| json!(v);
    let mut key_idx = Map::new();
    for (k, &i) in &g.key_idx {
        key_idx.insert(k.clone(), json!(i));
    }
    let mut site_of = Map::new();
    for (i, s) in g.site_of.iter().enumerate() {
        if let Some(s) = s {
            site_of.insert(i.to_string(), json!(s));
        }
    }
    let mut conns = Map::new();
    for (k, v) in g.conns.iter() {
        conns.insert(k.to_string(), json!(v));
    }
    let t = &g.turn_s;
    let sm = g.sm.as_ref().map_or(Value::Null, |sm| {
        let mut of = Map::new();
        for (k, &v) in &sm.of {
            of.insert(k.clone(), json!(v));
        }
        json!({
            "list": sm.list.iter().map(|l| json!({ "id": l.id, "inc": l.inc, "exits": l.exits, "off": l.off, "nx": l.nx })).collect::<Vec<_>>(),
            "of": of, "entS": sm.ent_s, "entI": sm.ent_i, "extS": sm.ext_s, "extI": sm.ext_i, "n": sm.n,
        })
    });
    json!({
        "ids": g.ids, "N": g.n, "M": g.m, "ll": pts(&g.ll), "head": int_list(g.head()), "out": int_list(g.out()), "to": int_list(g.to()),
        "ffc": nums(&g.ffc), "isC": g.is_c(), "zoneNode": g.zone_node, "zones": g.zones.iter().map(|z| z.id.clone()).collect::<Vec<_>>(),
        "deg": g.deg, "into": g.into, "outR": g.out_r, "conns": conns, "keyIdx": key_idx,
        "nodes": g.nodes.iter().map(|x| x.as_ref().map_or(Value::Null, |x| json!(x.id))).collect::<Vec<_>>(),
        "siteOf": site_of, "sites": g.sites.iter().map(|s| json!({ "id": s.site.id, "members": s.members })).collect::<Vec<_>>(),
        "capMethod": g.cap_method.as_str(), "period": g.period,
        "turnS": { "left": serde_json::to_value(&t.left).unwrap(), "right": serde_json::to_value(&t.right).unwrap(),
            "rightSignals": serde_json::to_value(&t.right_signals).unwrap(), "uturn": serde_json::to_value(&t.uturn).unwrap() },
        "tOff": int_list(g.t_off()), "tpen": nums(g.tpen()), "tTo": int_list(g.t_to()), "tmv": g.tmv, "nT": g.n_t,
        "sm": sm, "arcs": g.arcs.iter().map(arc_value).collect::<Vec<_>>(), "ctl": g.ctl.iter().map(ctl_value).collect::<Vec<_>>(),
    })
}

/// The scenario and settings of a case: the project's own, or a variant's from the truth file.
pub fn case_inputs(p: &Project, expected: &Value, case: &Value) -> (Scenario, Settings) {
    let id = case["scenario"].as_str().expect("scenario id");
    if case["variant"] == Value::Bool(true) {
        let v = &expected["variants"][id];
        let sc: Scenario = serde_json::from_str(v["scenario"].as_str().expect("scenario text")).expect("variant scenario");
        let s: Settings = serde_json::from_str(v["settings"].as_str().expect("settings text")).expect("variant settings");
        (sc, s)
    } else {
        (p.scenario(id).expect("scenario").clone(), p.settings.clone())
    }
}

/// Counts checks and keeps the differences of the failing ones.
#[derive(Default)]
pub struct Checker {
    /// Checks run.
    pub run: usize,
    /// Checks that matched.
    pub passed: usize,
    report: String,
}

impl Checker {
    /// Compares one value with the JavaScript's; a mismatch is recorded with its first differences.
    pub fn check(&mut self, path: &str, actual: &Value, expected: &Value) {
        self.run += 1;
        let (diffs, total) = diff_json(actual, expected, path);
        if total == 0 {
            self.passed += 1;
            return;
        }
        let _ = write!(self.report, "\n{total} difference(s) under {path}:");
        for d in diffs.iter().take(8) {
            let _ = write!(self.report, "\n  {}\n      expected {}\n      actual   {}", d.path, d.expected, d.actual);
        }
    }
    /// Prints the counts and panics when any check failed.
    pub fn finish(self, what: &str) {
        println!("{what}: {} of {} checks match the JavaScript", self.passed, self.run);
        if self.passed != self.run {
            let mut r = self.report;
            if r.len() > 60000 {
                r.truncate(60000);
                r.push_str("\n...");
            }
            panic!("{what}: {} of {} checks differ from the JavaScript:{r}", self.run - self.passed, self.run);
        }
    }
}
