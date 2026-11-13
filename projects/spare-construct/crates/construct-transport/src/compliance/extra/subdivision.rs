//! Urban subdivision mode, AUP E38 (compliance-extra.js section 12).

use super::ctx::{Pair, assumptions, data, dev_zones, esc, link_name, nn, num, pair, periods_of, pct, res_of};
use super::packs::{n0, t_table};
use crate::compliance::data::{c as core_table, x as xdata};
use crate::compliance::engine;
use crate::compliance::html::ReportCtx;
use crate::compliance::jv::{Jx, NULL, V, arr, jobj, ns, strs};
use crate::compliance::net;
use crate::graph::{Graph, js_sort_by};
use crate::jsnum::{math_min_of, truthy_number, usize_to_f64};
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
#[must_use]
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
#[must_use]
pub fn rear_access(a: &V) -> V {
    let n = n0(a.g("sites"));
    let std = xdata("STD.e38");
    let mut flags: Vec<String> = Vec::new();
    let max_sites = std.g("maxSites").n();
    if n > max_sites {
        flags.push(format!("{} rear sites share this access: no more than {} rear sites per access (Table E38.8.1.2.1)", ns(n), ns(max_sites)));
    }
    let rows = std.g("rows").items();
    // The table always has rows; null (an undefined row) is only the total fallback
    let row = rows.iter().find(|r| n <= r.g("max").n()).or_else(|| rows.last()).unwrap_or(&NULL);
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
    let (ped, need_ped) = (nn(a.g("ped")).unwrap_or(0.0), row.g("ped").n());
    if row.g("ped").t() && n > 5.0 && (ped < need_ped || ped.is_nan() || need_ped.is_nan()) {
        flags.push(format!("Needs separated pedestrian access at least {} m wide", row.g("ped").s()));
    }
    let ok = flags.is_empty();
    let mut out = if a.is_obj() { a.clone() } else { V::object() };
    out.set("row", row.clone());
    out.set("flags", strs(flags));
    out.set("ok", V::Bool(ok));
    out
}

/// The existing roads whose flow rises 1 veh/h or more from the do-minimum to the option, by period, with the size of the rise (largest first).
fn subdivision_links(pr: &Pair, affected_pct: f64) -> Vec<(f64, V)> {
    let sc = pr.opt;
    let mut links: Vec<(f64, V)> = Vec::new();
    for per in periods_of(sc).into_iter().filter(|x| res_of(pr.base, x).is_some() && !pr.same) {
        // periods_of gives the periods with option results, and the filter those with base results
        let (Some(rb), Some(ro)) = (res_of(pr.base, per), res_of(sc, per)) else { continue };
        for l in pr.base.g("links").vals() {
            let id = l.g("id").s();
            if !sc.g("links").g(&id).t() || l.g("status").eq_s("proposed") {
                continue;
            }
            let flow = |r: &V| ["ab", "ba"].iter().fold(0.0, |s, d| s + { let v = r.g("arcs").g(&format!("{id}:{d}")).g("v"); if v.t() { v.n() } else { 0.0 } });
            let (vb, vo) = (flow(rb), flow(ro));
            let rise = vo - vb;
            if rise < 1.0 {
                continue;
            }
            let pc = pct(vo, vb);
            let significant = rise >= 20.0 && (pc.is_none() || pc.unwrap_or(0.0) >= affected_pct);
            links.push((rise, jobj! { "id" => l.g("id"), "name" => link_name(l), "period" => per, "base" => vb, "opt" => vo, "d" => rise, "pct" => pc, "significant" => significant }));
        }
    }
    js_sort_by(&mut links, |x, y| y.0 - x.0);
    links
}

/// The road distance (m) from a start node to the node of a zone id (`z:<zone id>`); NaN (an undefined distance) when the id is not a node.
fn dist_to(sk: &DistSkim, nd: &[f64], id: &str) -> f64 {
    // A node index of the graph is always inside its distance list
    sk.g.idx.get(id).map_or(f64::INFINITY, |&i| nd.get(i).copied().unwrap_or(f64::NAN))
}

/// The connectivity indicators for the development zones: junction density near the centroid, route directness and walking times to bus stops
/// and schools.
fn connectivity(p: &V, sc: &V, asm: &V) -> V {
    let dz = dev_zones(sc);
    let mut con = jobj! { "zones" => dz.len() };
    if dz.is_empty() {
        return con;
    }
    let count = usize_to_f64(dz.len());
    let centre = [dz.iter().fold(0.0, |s, z| s + z.g("lat").n()) / count, dz.iter().fold(0.0, |s, z| s + z.g("lng").n()) / count];
    let radius = 800.0;
    let inter = sc
        .g("nodes")
        .keys()
        .into_iter()
        .filter(|id| net::road_links(sc, id).len() >= 3 && util::dist(&[sc.g("nodes").g(id).g("lat").n(), sc.g("nodes").g(id).g("lng").n()], &centre) <= radius)
        .count();
    let side = radius / 1000.0;
    con.set("density", V::Number(usize_to_f64(inter) / (std::f64::consts::PI * (side * side))));
    con.set("densityBasis", V::String(format!("junctions (3 or more roads) within {} m of the development centroid, per km\u{b2}", ns(radius))));
    let mut stops: Vec<String> = Vec::new();
    let mut add_stop = |id: String| {
        if !stops.contains(&id) {
            stops.push(id);
        }
    };
    for rt in sc.g("routes").vals() {
        for wp in rt.g("wp").items() {
            if sc.g("nodes").g(&wp.s()).t() {
                add_stop(wp.s());
            }
        }
    }
    for node in sc.g("nodes").vals() {
        if node.g("stop").t() {
            add_stop(node.g("id").s());
        }
    }
    let schools: Vec<&V> = sc.g("zones").vals().into_iter().filter(|z| super::ctx::lu_of(z, "school") > 0.0 && !z.g("kind").eq_s("gateway")).collect();
    let from: Vec<String> = dz.iter().map(|z| format!("z:{}", z.g("id").s())).collect();
    let sk = dist_skim(p, sc, &from);
    let (mut dirs, mut w_stop, mut w_school): (Vec<f64>, Vec<f64>, Vec<f64>) = (vec![], vec![], vec![]);
    let walk_speed = asm.g("walkSpeed").n();
    for z in &dz {
        let Some(nd) = sk.nd.get(&format!("z:{}", z.g("id").s())) else { continue };
        for other in sc.g("zones").vals() {
            if std::ptr::eq(other, *z) || other.g("kind").eq_s("gateway") {
                continue;
            }
            let idx = sk.g.idx.get(&format!("z:{}", other.g("id").s()));
            let cr = util::dist(&[z.g("lat").n(), z.g("lng").n()], &[other.g("lat").n(), other.g("lng").n()]);
            // A node index of the graph is always inside its distance list
            if let Some(&dist) = idx.and_then(|&i| nd.get(i))
                && dist.is_finite() && cr > 200.0 && cr < 3000.0 {
                    dirs.push(dist / cr);
                }
        }
        let near = |ids: &[String]| -> f64 { math_min_of(&ids.iter().map(|id| dist_to(&sk, nd, id)).collect::<Vec<_>>()) };
        if !stops.is_empty() {
            let dist = near(&stops);
            if dist.is_finite() {
                w_stop.push(dist / walk_speed / 60.0);
            }
        }
        let sch: Vec<&&V> = schools.iter().filter(|other| !std::ptr::eq(**other, *z)).collect();
        if !sch.is_empty() {
            let ids: Vec<String> = sch.iter().map(|other| format!("z:{}", other.g("id").s())).collect();
            let dist = near(&ids);
            if dist.is_finite() {
                w_school.push(dist / walk_speed / 60.0);
            }
        }
    }
    let avg = |x: &[f64]| -> V { if x.is_empty() { V::Null } else { V::Number(x.iter().fold(0.0, |s, v| s + v) / usize_to_f64(x.len())) } };
    con.set("directness", avg(&dirs));
    con.set("directnessBasis", V::String("road-network distance \u{f7} straight-line distance to other zones 200 m to 3 km away (1.0 is perfectly direct)".to_string()));
    con.set("walkStop", avg(&w_stop));
    con.set("walkSchool", avg(&w_school));
    con.set("walkBasis", V::String(format!("road-network distance at {} m/s ({}); bus stops from modelled bus routes", asm.g("walkSpeed").s(), xdata("ASSUMPTION").s())));
    con
}

/// The AUP Table E27.6.1.1 subdivision rows (T3A, T3B), from the Compliance lane's E27 data when loaded, each with whether the lots exceed it, and
/// the wording of the result.
fn e27_check(lots: f64) -> (Vec<V>, String) {
    let e27_data: Vec<&V> = core_table("E27.rows").items().iter().filter(|r| r.g("key").eq_s("subdivision")).collect();
    let (e27_rows, e27src): (Vec<V>, &str) = if e27_data.is_empty() {
        (vec![jobj! { "id" => "T3A", "thr" => 40, "tier" => 1 }, jobj! { "id" => "T3B", "thr" => 100, "tier" => 2 }], "Compliance+ copy (verify)")
    } else {
        (e27_data.into_iter().cloned().collect(), "Compliance tab E27 data")
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
    (e27, e27_text)
}

/// `subdivision(P)`: the subdivision check: `{ lots, trips, e27, e27Text, tripsBasis, links, linksBasis, connectivity, accesses, warning }`.
pub fn subdivision(p: &V) -> V {
    let cx = data(p).g("subdivision").clone();
    let pr = pair(p);
    let sc = pr.opt;
    let asm = assumptions(p);
    let lots = n0(cx.g("lots"));
    let trips: Vec<V> = ["AM", "PM"]
        .iter()
        .map(|per| {
            let zone = jobj! { "id" => "_lots", "kind" => "zone", "lu" => jobj! { "residential" => lots } };
            let (o, d) = engine::trip_ends(p, &zone, per, sc);
            jobj! { "period" => *per, "O" => o, "D" => d, "total" => o + d }
        })
        .collect();
    let links = subdivision_links(&pr, asm.g("affectedPct").n());
    let con = connectivity(p, sc, &asm);
    let (e27, e27_text) = e27_check(lots);
    let accesses: Vec<V> = if cx.g("accesses").t() { cx.g("accesses").items().iter().map(rear_access).collect() } else { vec![] };
    jobj! {
        "lots" => lots,
        "trips" => arr(trips),
        "e27" => arr(e27),
        "e27Text" => e27_text,
        "tripsBasis" => "Lot yield as dwellings at the project's residential trip rate; AUP E27 rows T3A/T3B decide whether the subdivision triggers E27.6.1",
        "links" => arr(links.into_iter().take(20).map(|x| x.1)),
        "linksBasis" => format!("'Significant increase' screen: at least 20 veh/h and {}% ({}); E38.12.2(g) is a judgment", asm.g("affectedPct").s(), xdata("ASSUMPTION").s()),
        "connectivity" => con,
        "accesses" => arr(accesses),
        "warning" => xdata("STD.labels.e38pc120"),
    }
}

/// The report sections of this part the report lane can ask for (see [`report`]).
pub const REPORT_IDS: &[&str] = &["compliance-extra-subdivision"];

/// The HTML of a report section of this part (`REPORT[id].render(P, ctx)`); None when the id is not one of this part's. A section that has
/// nothing to say gives an empty string, as the prototype's does.
#[must_use]
pub fn report(id: &str, p: &V, _ctx: Option<&dyn ReportCtx>) -> Option<String> {
    if id != "compliance-extra-subdivision" {
        return None;
    }
    let s = subdivision(p);
    let lots = s.g("lots").n();
    if (lots <= 0.0 || lots.is_nan()) && s.g("accesses").items().is_empty() {
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
                if a.g("ok").t() { "Complies".to_string() } else { super::ctx::esc_s(&a.g("flags").items().iter().map(crate::compliance::jv::Jx::s).collect::<Vec<_>>().join("; ")) },
            ]
        })
        .collect();
    Some(format!(
        "<p class=\"note\">{}</p>{}{}<p class=\"note\">Connectivity: intersection density {}, route directness {}.</p>",
        esc(s.g("warning")),
        if s.g("e27Text").t() { format!("<p class=\"note\">{}.</p>", esc(s.g("e27Text"))) } else { String::new() },
        t_table(&["Rear access", "Sites", "Check"], rows, "No rear accesses."),
        if con.g("density").nullish() { "\u{2013}".to_string() } else { format!("{} per km\u{b2}", num(con.g("density"), 0)) },
        if con.g("directness").nullish() { "\u{2013}".to_string() } else { num(con.g("directness"), 2) },
    ))
}

/// The exports of this part (`EXPORTS`, ids as the prototype registers them: "compliance-extra-<id>").
pub const EXPORT_IDS: &[&str] = &[];

/// The text of an export of this part (`build(P)`); None when the id is not one of this part's.
#[must_use]
pub fn export_build(_id: &str, _p: &V) -> Option<String> {
    None
}

/// Whether an export of this part is offered for the project (`enabled()`); None when the id is not one of this part's.
#[must_use]
pub fn export_enabled(_id: &str, _p: &V) -> Option<bool> {
    None
}
