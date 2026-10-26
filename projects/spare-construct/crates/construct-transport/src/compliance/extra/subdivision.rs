//! Urban subdivision mode, AUP E38 (compliance-extra.js section 12).

use super::ctx::{assumptions, data, dev_zones, esc, link_name, nn, num, pair, periods_of, pct, res_of};
use super::packs::{n0, t_table};
use crate::compliance::data::{c as core_table, x as xdata};
use crate::compliance::engine;
use crate::compliance::html::ReportCtx;
use crate::compliance::jv::{Jx, V, arr, jobj, ns, strs};
use crate::compliance::net;
use crate::graph::{Graph, js_sort_by};
use crate::jsnum::{math_min_of, truthy_number};
use crate::util::{self, fmt};
use std::collections::HashMap;

/// The result of [`dist_skim`]: the graph and, for each start node that exists, the road distance (m) to every node.
pub struct DistSkim {
    /// The routing graph the distances were measured on.
    pub g: Graph,
    /// The distances by start node id (`z:<zone id>`).
    pub nd: HashMap<String, Vec<f64>>,
}

/// `distSkim(sc, fromIds)`: shortest road distances (arc lengths) from each start node (ids such as `z:zDev`) to every node of the scenario's graph.
pub fn dist_skim(p: &V, sc: &V, from_ids: &[String]) -> DistSkim {
    let g = engine::build_graph(p, sc, None);
    let cost: Vec<f64> = g.arcs.iter().map(|a| if truthy_number(a.len) { a.len } else { 0.0 }).collect();
    let mut w = g.workspace();
    let mut nd: HashMap<String, Vec<f64>> = HashMap::new();
    for id in from_ids {
        let Some(&s) = g.idx.get(id) else { continue };
        g.spt(&cost, s, &mut w, -1, -1);
        nd.insert(id.clone(), w.nd.clone());
    }
    DistSkim { g, nd }
}

/// `rearAccess(a)`: a rear-site access checked against AUP Table E38.8.1.2.1: the entry with its table row, the flags and `ok`.
pub fn rear_access(a: &V) -> V {
    let n = n0(a.g("sites"));
    let std = xdata("STD.e38");
    let mut flags: Vec<String> = Vec::new();
    let max_sites = std.g("maxSites").n();
    if n > max_sites {
        flags.push(format!("{} rear sites share this access: no more than {} rear sites per access (Table E38.8.1.2.1)", ns(n), ns(max_sites)));
    }
    let rows = std.g("rows").items();
    let row = rows.iter().find(|r| n <= r.g("max").n()).unwrap_or_else(|| rows.last().expect("e38 rows"));
    let plural = if n == 1.0 { "" } else { "s" };
    if nn(a.g("formed")).is_some() && a.g("formed").n() < row.g("formed").n() {
        flags.push(format!("Formed width {} m is below {} m for {} site{}", num(&V::Number(a.g("formed").n()), 1), num(row.g("formed"), 1), ns(n), plural));
    }
    if nn(a.g("legal")).is_some() && a.g("legal").n() < row.g("legal").n() {
        flags.push(format!("Legal width {} m is below {} m for {} site{}", num(&V::Number(a.g("legal").n()), 1), num(row.g("legal"), 1), ns(n), plural));
    }
    if nn(a.g("len")).is_some() && a.g("len").n() > row.g("len").n() {
        flags.push(format!("Length {} m is over the {} m maximum", fmt::int(a.g("len").n()), row.g("len").s()));
    }
    if row.g("ped").t() && n > 5.0 && !(nn(a.g("ped")).unwrap_or(0.0) >= row.g("ped").n()) {
        flags.push(format!("Needs separated pedestrian access at least {} m wide", row.g("ped").s()));
    }
    let ok = flags.is_empty();
    let mut out = if a.is_obj() { a.clone() } else { V::object() };
    out.set("row", row.clone());
    out.set("flags", strs(flags));
    out.set("ok", V::Bool(ok));
    out
}

/// `subdivision(P)`: the subdivision check: `{ lots, trips, e27, e27Text, tripsBasis, links, linksBasis, connectivity, accesses, warning }`.
pub fn subdivision(p: &V) -> V {
    let cx = data(p).g("subdivision").clone();
    let pr = pair(p);
    let sc = pr.opt;
    let a = assumptions(p);
    let lots = n0(cx.g("lots"));
    let trips: Vec<V> = ["AM", "PM"]
        .iter()
        .map(|per| {
            let z = jobj! { "id" => "_lots", "kind" => "zone", "lu" => jobj! { "residential" => lots } };
            let (o, d) = engine::trip_ends(p, &z, per, sc);
            jobj! { "period" => *per, "O" => o, "D" => d, "total" => o + d }
        })
        .collect();
    let affected_pct = a.g("affectedPct").n();
    let mut links: Vec<(f64, V)> = Vec::new();
    for per in periods_of(sc).into_iter().filter(|x| res_of(pr.base, x).is_some() && !pr.same) {
        let rb = res_of(pr.base, per).expect("base results");
        let ro = res_of(sc, per).expect("option results");
        for l in pr.base.g("links").vals() {
            let id = l.g("id").s();
            if !sc.g("links").g(&id).t() || l.g("status").eq_s("proposed") {
                continue;
            }
            let flow = |r: &V| ["ab", "ba"].iter().fold(0.0, |s, d| s + { let v = r.g("arcs").g(&format!("{id}:{d}")).g("v"); if v.t() { v.n() } else { 0.0 } });
            let (vb, vo) = (flow(rb), flow(ro));
            let d = vo - vb;
            if d < 1.0 {
                continue;
            }
            let pc = pct(vo, vb);
            let significant = d >= 20.0 && (pc.is_none() || pc.unwrap_or(0.0) >= affected_pct);
            links.push((d, jobj! { "id" => l.g("id"), "name" => link_name(l), "period" => per, "base" => vb, "opt" => vo, "d" => d, "pct" => pc, "significant" => significant }));
        }
    }
    js_sort_by(&mut links, |x, y| y.0 - x.0);
    // connectivity indicators for the development zones
    let dz = dev_zones(sc);
    let mut con = jobj! { "zones" => dz.len() };
    if !dz.is_empty() {
        let c = [dz.iter().fold(0.0, |s, z| s + z.g("lat").n()) / dz.len() as f64, dz.iter().fold(0.0, |s, z| s + z.g("lng").n()) / dz.len() as f64];
        let r = 800.0;
        let inter = sc
            .g("nodes")
            .keys()
            .into_iter()
            .filter(|id| net::road_links(sc, id).len() >= 3 && util::dist(&[sc.g("nodes").g(id).g("lat").n(), sc.g("nodes").g(id).g("lng").n()], &c) <= r)
            .count();
        let side = r / 1000.0;
        con.set("density", V::Number(inter as f64 / (std::f64::consts::PI * (side * side))));
        con.set("densityBasis", V::String(format!("junctions (3 or more roads) within {} m of the development centroid, per km\u{b2}", ns(r))));
        let mut stops: Vec<String> = Vec::new();
        let mut add_stop = |id: String| {
            if !stops.contains(&id) {
                stops.push(id);
            }
        };
        for rt in sc.g("routes").vals() {
            for n in rt.g("wp").items() {
                if sc.g("nodes").g(&n.s()).t() {
                    add_stop(n.s());
                }
            }
        }
        for n in sc.g("nodes").vals() {
            if n.g("stop").t() {
                add_stop(n.g("id").s());
            }
        }
        let schools: Vec<&V> = sc.g("zones").vals().into_iter().filter(|z| super::ctx::lu_of(z, "school") > 0.0 && !z.g("kind").eq_s("gateway")).collect();
        let from: Vec<String> = dz.iter().map(|z| format!("z:{}", z.g("id").s())).collect();
        let sk = dist_skim(p, sc, &from);
        let (mut dirs, mut w_stop, mut w_school): (Vec<f64>, Vec<f64>, Vec<f64>) = (vec![], vec![], vec![]);
        let walk_speed = a.g("walkSpeed").n();
        for z in &dz {
            let Some(nd) = sk.nd.get(&format!("z:{}", z.g("id").s())) else { continue };
            for o in sc.g("zones").vals() {
                if std::ptr::eq(o, *z) || o.g("kind").eq_s("gateway") {
                    continue;
                }
                let i = sk.g.idx.get(&format!("z:{}", o.g("id").s()));
                let cr = util::dist(&[z.g("lat").n(), z.g("lng").n()], &[o.g("lat").n(), o.g("lng").n()]);
                if let Some(&i) = i {
                    if nd[i].is_finite() && cr > 200.0 && cr < 3000.0 {
                        dirs.push(nd[i] / cr);
                    }
                }
            }
            let near = |ids: &[String]| -> f64 { math_min_of(&ids.iter().map(|id| sk.g.idx.get(id).map_or(f64::INFINITY, |&i| nd[i])).collect::<Vec<_>>()) };
            if !stops.is_empty() {
                let d = near(&stops);
                if d.is_finite() {
                    w_stop.push(d / walk_speed / 60.0);
                }
            }
            let sch: Vec<&&V> = schools.iter().filter(|o| !std::ptr::eq(**o, *z)).collect();
            if !sch.is_empty() {
                let ids: Vec<String> = sch.iter().map(|o| format!("z:{}", o.g("id").s())).collect();
                let d = near(&ids);
                if d.is_finite() {
                    w_school.push(d / walk_speed / 60.0);
                }
            }
        }
        let avg = |x: &[f64]| -> V { if x.is_empty() { V::Null } else { V::Number(x.iter().fold(0.0, |s, v| s + v) / x.len() as f64) } };
        con.set("directness", avg(&dirs));
        con.set("directnessBasis", V::String("road-network distance \u{f7} straight-line distance to other zones 200 m to 3 km away (1.0 is perfectly direct)".to_string()));
        con.set("walkStop", avg(&w_stop));
        con.set("walkSchool", avg(&w_school));
        con.set("walkBasis", V::String(format!("road-network distance at {} m/s ({}); bus stops from modelled bus routes", a.g("walkSpeed").s(), xdata("ASSUMPTION").s())));
    }
    // AUP Table E27.6.1.1 subdivision rows (T3A, T3B): from the Compliance lane's E27 data when loaded
    let e27_data: Vec<&V> = core_table("E27.rows").items().iter().filter(|r| r.g("key").eq_s("subdivision")).collect();
    let (e27_rows, e27src): (Vec<V>, &str) = if !e27_data.is_empty() {
        (e27_data.into_iter().cloned().collect(), "Compliance tab E27 data")
    } else {
        (vec![jobj! { "id" => "T3A", "thr" => 40, "tier" => 1 }, jobj! { "id" => "T3B", "thr" => 100, "tier" => 2 }], "Compliance+ copy (verify)")
    };
    let e27: Vec<V> = e27_rows.iter().map(|r| jobj! { "id" => r.g("id"), "thr" => r.g("thr"), "tier" => r.g("tier"), "exceeded" => lots > r.g("thr").n() }).collect();
    let mut exceeded: Vec<&V> = e27.iter().filter(|r| r.g("exceeded").t()).collect();
    js_sort_by(&mut exceeded, |x, y| y.g("thr").n() - x.g("thr").n());
    let hit = exceeded.first();
    let e27_text = if lots > 0.0 {
        match hit {
            Some(h) => format!(
                "{} lots exceed Table E27.6.1.1 row {} ({} dwellings of capacity): restricted discretionary under E27.6.1, tier {}; check the E27.6.1(2) exemptions and any precinct ({})",
                fmt::int(lots),
                h.g("id").s(),
                fmt::int(h.g("thr").n()),
                h.g("tier").s(),
                e27src
            ),
            None => format!(
                "{} lots are within the Table E27.6.1.1 subdivision thresholds ({}; {})",
                fmt::int(lots),
                e27.iter().map(|r| format!("{} {}", r.g("id").s(), r.g("thr").s())).collect::<Vec<_>>().join(", "),
                e27src
            ),
        }
    } else {
        String::new()
    };
    let accesses: Vec<V> = if cx.g("accesses").t() { cx.g("accesses").items().iter().map(rear_access).collect() } else { vec![] };
    jobj! {
        "lots" => lots,
        "trips" => arr(trips),
        "e27" => arr(e27),
        "e27Text" => e27_text,
        "tripsBasis" => "Lot yield as dwellings at the project's residential trip rate; AUP E27 rows T3A/T3B decide whether the subdivision triggers E27.6.1",
        "links" => arr(links.into_iter().take(20).map(|x| x.1)),
        "linksBasis" => format!("'Significant increase' screen: at least 20 veh/h and {}% ({}); E38.12.2(g) is a judgment", a.g("affectedPct").s(), xdata("ASSUMPTION").s()),
        "connectivity" => con,
        "accesses" => arr(accesses),
        "warning" => xdata("STD.labels.e38pc120"),
    }
}

/// The report sections of this part the report lane can ask for (see [`report`]).
pub const REPORT_IDS: &[&str] = &["compliance-extra-subdivision"];

/// The HTML of a report section of this part (`REPORT[id].render(P, ctx)`); None when the id is not one of this part's. A section that has
/// nothing to say gives an empty string, as the prototype's does.
pub fn report(id: &str, p: &V, _ctx: Option<&dyn ReportCtx>) -> Option<String> {
    if id != "compliance-extra-subdivision" {
        return None;
    }
    let s = subdivision(p);
    if !(s.g("lots").n() > 0.0) && s.g("accesses").items().is_empty() {
        return Some(String::new());
    }
    let con = s.g("connectivity");
    let dash = V::String("\u{2013}".to_string());
    let rows: Vec<Vec<String>> = s
        .g("accesses")
        .items()
        .iter()
        .map(|a| {
            vec![
                esc(if a.g("name").t() { a.g("name") } else { &dash }),
                num(a.g("sites"), 0),
                if a.g("ok").t() { "Complies".to_string() } else { super::ctx::esc_s(&a.g("flags").items().iter().map(|f| f.s()).collect::<Vec<_>>().join("; ")) },
            ]
        })
        .collect();
    Some(format!(
        "<p class=\"note\">{}</p>{}{}<p class=\"note\">Connectivity: intersection density {}, route directness {}.</p>",
        esc(s.g("warning")),
        if s.g("e27Text").t() { format!("<p class=\"note\">{}.</p>", esc(s.g("e27Text"))) } else { String::new() },
        t_table(&["Rear access", "Sites", "Check"], rows, "No rear accesses."),
        if !con.g("density").nullish() { format!("{} per km\u{b2}", num(con.g("density"), 0)) } else { "\u{2013}".to_string() },
        if !con.g("directness").nullish() { num(con.g("directness"), 2) } else { "\u{2013}".to_string() },
    ))
}

/// The exports of this part (`EXPORTS`, ids as the prototype registers them: "compliance-extra-<id>").
pub const EXPORT_IDS: &[&str] = &[];

/// The text of an export of this part (`build(P)`); None when the id is not one of this part's.
pub fn export_build(_id: &str, _p: &V) -> Option<String> {
    None
}

/// Whether an export of this part is offered for the project (`enabled()`); None when the id is not one of this part's.
pub fn export_enabled(_id: &str, _p: &V) -> Option<bool> {
    None
}
