//! Helpers several parts of Compliance+ call (packs/nz/compliance-extra.js sections 7 and 8): the change between two scenarios, per-peak
//! differences, what an option changes, crash counts on changed roads and the economics of an option.

use super::ctx::{class_of, link_name, node_name, periods_of, res_of};
use crate::compliance::jv::{Jx, V, jobj};
use crate::compliance::{data, engine, net};
use crate::util::fmt;

/// `routeTime(sc, ps)`: the bus route times of a scenario in the periods, summed (s).
pub fn route_time(sc: &V, ps: &[&str]) -> f64 {
    ps.iter().fold(0.0, |s, p| {
        let routes = res_of(sc, p).map_or(&crate::compliance::jv::NULL, |r| r.g("routes"));
        s + routes.vals().iter().fold(0.0, |t, r| t + { let x = r.g("t").n(); if x.is_nan() { 0.0 } else { x } })
    })
}

/// `kpiP(k)(sc, p)`: a network total of a scenario in a period (0 when missing).
pub fn kpi_p(k: &str, sc: &V, p: &str) -> f64 {
    let x = res_of(sc, p).map_or(&crate::compliance::jv::NULL, |r| r.g("kpi")).g(k);
    if x.t() { x.n() } else { 0.0 }
}

/// `routeP(sc, p)`: the bus route times of a scenario in one period.
pub fn route_p(sc: &V, p: &str) -> f64 {
    route_time(sc, &[p])
}

/// `perPeriod(base, opt, f, ps, d, unit)`: a difference (option minus base) for each modelled peak hour, kept per period (AM and PM are
/// never added together): `{ by: { AM, PM }, mean, text }` where text reads "AM -720.7, PM -482.7 veh·h per peak hour".
pub fn per_period(base: &V, opt: &V, f: &dyn Fn(&V, &str) -> f64, ps: &[&str], d: usize, unit: &str) -> V {
    let mut by = V::object();
    let mut vals: Vec<f64> = Vec::new();
    for p in ps {
        if res_of(base, p).is_some() && res_of(opt, p).is_some() {
            let x = f(opt, p) - f(base, p);
            by.set(*p, V::Number(x));
            vals.push(x);
        }
    }
    let mean = if vals.is_empty() { V::Null } else { V::Number(vals.iter().fold(0.0, |s, x| s + x) / vals.len() as f64) };
    let text = if vals.is_empty() { fmt::DASH.to_string() } else { format!("{}{}", pp_entries(&by, d), if unit.is_empty() { String::new() } else { format!(" {unit}") }) };
    jobj! { "by" => by, "mean" => mean, "text" => text }
}

fn pp_entries(by: &V, d: usize) -> String {
    by.entries().iter().map(|(p, x)| format!("{} {}", p, fmt::signed(x.n(), d))).collect::<Vec<_>>().join(", ")
}

/// `negPP(pp)`: a per-period difference with every value negated (`{ by, mean }`).
pub fn neg_pp(pp: &V) -> V {
    let mut by = V::object();
    for (p, v) in pp.g("by").entries() {
        by.set(p, V::Number(-v.n()));
    }
    jobj! { "by" => by, "mean" => if pp.g("mean").nullish() { V::Null } else { V::Number(-pp.g("mean").n()) } }
}

/// `ppText(pp, d, unit)`: the per-period values as text ("AM +1.2, PM -3.4 unit"), an en dash when there are none.
pub fn pp_text(pp: &V, d: usize, unit: &str) -> String {
    if pp.g("by").keys().is_empty() {
        fmt::DASH.to_string()
    } else {
        format!("{}{}", pp_entries(pp.g("by"), d), if unit.is_empty() { String::new() } else { format!(" {unit}") })
    }
}

/// `changedLinks(a, b)`: ids of roads that differ between scenario a and scenario b: new or changed status, lanes, speed, bus lane or cycle
/// facility, and every road at a junction whose control changed or that was upgraded.
pub fn changed_links(a: &V, b: &V) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    let mut add = |id: String| {
        if !ids.contains(&id) {
            ids.push(id);
        }
    };
    for l in b.g("links").vals() {
        let o = a.g("links").g(&l.g("id").s());
        let same = o.t()
            && o.g("status").strict_eq(l.g("status"))
            && o.g("lanesAB").strict_eq(l.g("lanesAB"))
            && o.g("lanesBA").strict_eq(l.g("lanesBA"))
            && o.g("speed").strict_eq(l.g("speed"))
            && o.g("busLane").strict_eq(l.g("busLane"))
            && o.g("cycle").strict_eq(l.g("cycle"));
        // JavaScript's `!==` on two missing fields is false, and null is not undefined: the saved projects name every field
        if !same {
            add(l.g("id").s());
        }
    }
    for n in b.g("nodes").vals() {
        let o = a.g("nodes").g(&n.g("id").s());
        if o.t() && (!o.g("control").strict_eq(n.g("control")) || (n.g("upgrade").t() && !o.g("upgrade").t())) {
            for l in net::road_links(b, &n.g("id").s()) {
                add(l.g("id").s());
            }
        }
    }
    ids
}

/// `networkDiff(a, b)`: what scenario b changes against scenario a: junction controls and upgrades, new roads, extra lanes and bus lanes
/// (`[{ kind, id, text }]`).
pub fn network_diff(a: &V, b: &V) -> V {
    let mut out: Vec<V> = Vec::new();
    if !a.t() || !b.t() || std::ptr::eq(a, b) {
        return V::Array(out);
    }
    let controls = data::m("CONTROLS");
    let cname = |c: &V| -> String {
        let k = c.s();
        if controls.has(&k) && controls.g(&k).t() { controls.g(&k).s() } else { k }
    };
    for n in b.g("nodes").vals() {
        let o = a.g("nodes").g(&n.g("id").s());
        if o.t() && (!o.g("control").strict_eq(n.g("control")) || (n.g("upgrade").t() && !o.g("upgrade").t())) {
            let was = if !o.g("control").strict_eq(n.g("control")) { format!(" (was {})", cname(o.g("control")).to_lowercase()) } else { String::new() };
            out.push(jobj! { "kind" => "node", "id" => n.g("id"), "text" => format!("{} at {}{}", cname(n.g("control")), node_name(b, &n.g("id").s()), was) });
        }
    }
    for l in b.g("links").vals() {
        let o = a.g("links").g(&l.g("id").s());
        let lanes = |x: &V| (x.g("lanesAB").n() as i64 as i32 | 0) as f64 + (x.g("lanesBA").n() as i64 as i32 | 0) as f64;
        if !o.t() && !l.g("status").eq_s("closed") {
            out.push(jobj! { "kind" => "link", "id" => l.g("id"), "text" => format!("New road: {} ({})", link_name(l), fmt::len(net::len(b, l))) });
        } else if o.t() && lanes(l) > lanes(o) {
            out.push(jobj! { "kind" => "link", "id" => l.g("id"), "text" => format!("Extra lanes on {}", link_name(l)) });
        } else if o.t() && l.g("busLane").t() && !o.g("busLane").t() {
            out.push(jobj! { "kind" => "link", "id" => l.g("id"), "text" => format!("Bus lane on {}", link_name(l)) });
        }
    }
    V::Array(out)
}

/// `crashesOn(P, sc, ids)`: the crashes and DSI crashes recorded on roads (`{ crashes, dsi }`), or null when no crash analysis is loaded.
pub fn crashes_on(p: &V, sc: &V, ids: &[String]) -> V {
    let Some(risk) = engine::link_risk(p, sc) else { return V::Null };
    let (mut c, mut d) = (0.0, 0.0);
    for id in ids {
        let r = risk.g(id);
        if r.t() {
            c += if r.g("crashes").t() { r.g("crashes").n() } else { 0.0 };
            d += if r.g("dsi").t() { r.g("dsi").n() } else { 0.0 };
        }
    }
    jobj! { "crashes" => c, "dsi" => d }
}

/// `econOf(P, base, sc)`: the economics of an option against the do-minimum for the first period both were run in, or null.
pub fn econ_of(p: &V, base: &V, sc: &V) -> V {
    let per = periods_of(sc).into_iter().find(|x| res_of(base, x).is_some()).unwrap_or("AM");
    engine::econ(p, base, sc, per)
}

/// `devMix(sc)`: a text of the development zones, their land use and build-out, for comparing two scenarios' inputs.
pub fn dev_mix(sc: &V) -> String {
    let mut rows: Vec<V> = super::ctx::dev_zones(sc)
        .iter()
        .map(|z| V::Array(vec![z.g("id").clone(), z.g("lu").clone(), V::Number(super::ctx::stage_of(z, sc))]))
        .collect();
    // `.sort()` compares the rows as text
    rows.sort_by_key(|r| r.js_string());
    V::Array(rows).stringify()
}

/// A road class's label in lower case ("arterial"), for the unnamed road wording.
pub fn class_label_lower(l: &V) -> String {
    class_of(l.g("cls")).g("label").s().to_lowercase()
}
