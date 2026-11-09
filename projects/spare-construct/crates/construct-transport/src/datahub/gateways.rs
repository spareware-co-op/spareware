//! Gateway flows from counts (`DataHub.gatewaysFromCounts`).

use super::{AsText, Count, Data, is_estimate, matches, norm, period_vol, source};
use crate::edit::net::links_at;
use crate::graph::js_sort_by;
use crate::jsnum::{Maybe, Num, math_round};
use crate::project::{OrderedMap, Scenario, Settings};
use crate::util::{ang_diff, bearing, dist, fmt};
use serde_json::Value;
use std::rc::Rc;

/// How a gateway's flows are credited: "NZTA 12 Great South Road (9,800/day)", several joined with "; ".
fn label(c: &Count) -> String {
    let short = match c.src.as_text().and_then(source) {
        Some(s) => s.short.to_string(),
        None => c.src.js_string(),
    };
    format!(
        "{}{} {} {}{}",
        short,
        if c.virtual_.truthy() { " estimate" } else { "" },
        if c.reference.truthy() {
            c.reference.js_string()
        } else {
            String::new()
        },
        c.name.js_string(),
        if c.adt.truthy() {
            format!(" ({}/day)", fmt::int(c.adt.num()))
        } else {
            String::new()
        }
    )
}

struct Cand {
    c: Rc<Count>,
    dist: f64,
}

/// `DataHub.gatewaysFromCounts(sc)`: sets each gateway's AM and PM flows in and out from the counts near it.
///
/// Physical counts first (a count in use that is not a RAMM estimate and is matched to a link of the gateway's road
/// class, on the gateway's own road or, on arterial roads and above, within 3 km); RAMM centreline estimates only
/// where no count is near a gateway; NZTA virtual sites last, when that source is switched on. A directional count
/// within 70 degrees of the traffic entering (or leaving) the area gives the flow in (or out); two-way counts are
/// split by the lanes each way. Flows are rounded to 10 veh/h, and the gateway's `gwSrc` names the counts used. Gives
/// the number of gateways set.
///
/// A matched count that gives no volume in a period stops the prototype with a TypeError (`periodVol(...).v` of
/// null); here it adds no flow for that period and is not named in `gwSrc`.
pub fn gateways_from_counts(sc: &mut Scenario, s: &Settings, data: &Data) -> usize {
    let m = matches(sc, s, data);
    let in_use_matched = |est: bool| -> Vec<Rc<Count>> {
        data.counts
            .iter()
            .filter(|c| data.in_use(c) && is_estimate(c) == est && m.get(&c.id).is_some())
            .cloned()
            .collect()
    };
    let usable = in_use_matched(false);
    let estimates = in_use_matched(true);
    let nzta_on = data.use_of("nzta").truthy();
    let fallback: Vec<Rc<Count>> = data
        .counts
        .iter()
        .filter(|c| c.virtual_.truthy() && !c.off.truthy() && nzta_on && m.get(&c.id).is_some())
        .cloned()
        .collect();
    let mut set = 0;
    let ids: Vec<String> = sc.zones.keys().map(String::from).collect();
    for zid in ids {
        let (flows, src) = {
            let Some(z) = sc.zones.get(&zid) else {
                continue;
            };
            if !z.kind.is("gateway") {
                continue;
            }
            let Some(node) = z.node.as_str().filter(|n| !n.is_empty()) else {
                continue;
            };
            let Some(g_node) = sc.nodes.get(node) else {
                continue;
            };
            let ls = links_at(sc, node);
            let Some(lg) = ls.first() else { continue };
            let g_name = norm(lg.name.as_str().unwrap_or(""));
            let g_rank = lg.class().rank;
            let g = sc.geom(lg);
            let in_b = if lg.a == node {
                bearing(&g[0], &g[1.min(g.len() - 1)])
            } else {
                bearing(&g[g.len() - 1], &g[g.len() - 2])
            };
            let g_ll = [g_node.lat.num(), g_node.lng.num()];
            let near = |pool: &[Rc<Count>]| -> Vec<Cand> {
                let mut out: Vec<Cand> = pool
                    .iter()
                    .filter_map(|c| {
                        let mt = m.get(&c.id)?;
                        let d = dist(&g_ll, &c.ll());
                        if d > 3000.0 {
                            return None;
                        }
                        let lc = sc.links.get(&mt.link)?;
                        if lc.cls != lg.cls {
                            return None;
                        }
                        if c.src_is("nzta") && c.ramp.truthy() != lg.cls.is("ramp") {
                            return None;
                        }
                        let same =
                            !g_name.is_empty() && norm(lc.name.as_str().unwrap_or("")) == g_name;
                        if same || (g_rank >= 5.0 && d < 3000.0) {
                            Some(Cand {
                                c: c.clone(),
                                dist: d,
                            })
                        } else {
                            None
                        }
                    })
                    .collect();
                js_sort_by(&mut out, |a, b| a.dist - b.dist);
                out
            };
            let mut cand = near(&usable);
            if cand.is_empty() && !estimates.is_empty() {
                cand = near(&estimates);
            }
            if cand.is_empty() {
                cand = near(&fallback);
            }
            if cand.is_empty() {
                continue;
            }
            let (in_lanes, out_lanes) = if lg.a == node {
                (lg.lanes_ab.num(), lg.lanes_ba.num())
            } else {
                (lg.lanes_ba.num(), lg.lanes_ab.num())
            };
            let mut flows: Vec<(String, f64)> = Vec::new();
            let mut used: Vec<Rc<Count>> = Vec::new();
            for per in ["AM", "PM"] {
                let mut vin: Option<f64> = None;
                let mut vout: Option<f64> = None;
                let dirs: Vec<&Cand> = cand
                    .iter()
                    .filter(|x| x.c.directional.truthy() && x.c.dir.truthy())
                    .collect();
                if !dirs.is_empty() {
                    let in_c = dirs
                        .iter()
                        .find(|x| ang_diff(super::dir_bearing(&x.c.dir.js_string()), in_b) < 70.0);
                    let out_c = dirs.iter().find(|x| {
                        ang_diff(
                            super::dir_bearing(&x.c.dir.js_string()),
                            (in_b + 180.0) % 360.0,
                        ) < 70.0
                    });
                    if let Some(x) = in_c {
                        vin = period_vol(&x.c, per, s).map(|p| p.v);
                        if vin.is_some() {
                            used.push(x.c.clone());
                        }
                    }
                    if let Some(x) = out_c {
                        vout = period_vol(&x.c, per, s).map(|p| p.v);
                        if vout.is_some() {
                            used.push(x.c.clone());
                        }
                    }
                    if !(in_lanes > 0.0) {
                        vin = Some(0.0);
                    }
                    if !(out_lanes > 0.0) {
                        vout = Some(0.0);
                    }
                } else if let Some(tw) = cand.iter().find(|x| !x.c.directional.truthy()) {
                    let v = period_vol(&tw.c, per, s).map(|p| p.v);
                    let split = if in_lanes > 0.0 && out_lanes > 0.0 {
                        0.5
                    } else if in_lanes > 0.0 {
                        1.0
                    } else {
                        0.0
                    };
                    vin = v.map(|v| v * split);
                    vout = v.map(|v| v * (1.0 - split));
                    if v.is_some() {
                        used.push(tw.c.clone());
                    }
                }
                if let Some(v) = vin {
                    flows.push((format!("{per}in"), math_round(v / 10.0) * 10.0));
                }
                if let Some(v) = vout {
                    flows.push((format!("{per}out"), math_round(v / 10.0) * 10.0));
                }
            }
            let mut uniq: Vec<Rc<Count>> = Vec::new();
            for c in &used {
                if !uniq.iter().any(|u| Rc::ptr_eq(u, c)) {
                    uniq.push(c.clone());
                }
            }
            let src = if uniq.is_empty() {
                None
            } else {
                Some(uniq.iter().map(|c| label(c)).collect::<Vec<_>>().join("; "))
            };
            (flows, src)
        };
        if let Some(z) = sc.zones.get_mut(&zid) {
            // `z.gw ||= {}`
            if z.gw.get().is_none() {
                z.gw = Maybe::Set(OrderedMap::new());
            }
            if let Some(gw) = z.gw.get_mut() {
                for (k, v) in flows {
                    gw.insert(k, Num::number(v));
                }
            }
            if let Some(src) = src {
                z.gw_src = Maybe::Set(Value::String(src));
                set += 1;
            }
        }
    }
    set
}
