//! Gateway flows from counts (`DataHub.gatewaysFromCounts`).

use super::{AsText, Count, Data, Matches, is_estimate, matches, norm, period_vol, source};
use crate::edit::net::links_at;
use crate::graph::js_sort_by;
use crate::jsnum::{Maybe, Num, math_round};
use crate::project::{Link, OrderedMap, Scenario, Settings};
use crate::util::{LatLng, ang_diff, bearing, dist, fmt};
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
        if c.reference.truthy() { c.reference.js_string() } else { String::new() },
        c.name.js_string(),
        if c.adt.truthy() { format!(" ({}/day)", fmt::int(c.adt.num())) } else { String::new() }
    )
}

struct Cand {
    c: Rc<Count>,
    dist: f64,
}

// The gateway's own link (the first link at its node) and what the search for counts near it reads.
struct Gate<'a> {
    link: &'a Link,
    ll: LatLng,
    name: String,
    rank: f64,
}

// The pools of counts tried in turn: physical counts, RAMM estimates, NZTA virtual sites.
struct Pools {
    usable: Vec<Rc<Count>>,
    estimates: Vec<Rc<Count>>,
    fallback: Vec<Rc<Count>>,
}

// Flows by key ("AMin", "PMout") and the `gwSrc` text, when a count gave any.
type GateFlows = (Vec<(String, f64)>, Option<String>);

// The heading of traffic entering the area along the gateway's link: `bearing(g[0], g[Math.min(1, g.length - 1)])`
// from the link's a end, `bearing(g[g.length - 1], g[g.length - 2])` from its b end. A link's shape holds both end
// nodes, so the points are there; a shape too short gives NaN (no direction matches it).
fn entry_bearing(geom: &[LatLng], from_a: bool) -> f64 {
    let ends = if from_a {
        geom.first().zip(geom.get(1).or(geom.last()))
    } else {
        geom.split_last().and_then(|(last, rest)| rest.last().map(|prev| (last, prev)))
    };
    ends.map_or(f64::NAN, |(from, to)| bearing(from, to))
}

// The counts of a pool near the gateway, nearest first: matched to a link of the gateway's road class within 3 km,
// on the gateway's own road or (on arterial roads and above) anywhere in that radius.
fn near(sc: &Scenario, matched: &Matches, gate: &Gate, pool: &[Rc<Count>]) -> Vec<Cand> {
    let lg = gate.link;
    let mut out: Vec<Cand> = pool
        .iter()
        .filter_map(|c| {
            let mt = matched.get(&c.id)?;
            let d = dist(&gate.ll, &c.ll());
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
            let same = !gate.name.is_empty() && norm(lc.name.as_str().unwrap_or("")) == gate.name;
            if same || (gate.rank >= 5.0 && d < 3000.0) {
                Some(Cand { c: c.clone(), dist: d })
            } else {
                None
            }
        })
        .collect();
    js_sort_by(&mut out, |a, b| a.dist - b.dist);
    out
}

// One period's flows in and out (`vin`, `vout`), adding the counts that gave them to `used`.
fn period_flow(
    cand: &[Cand],
    per: &str,
    s: &Settings,
    in_b: f64,
    (in_lanes, out_lanes): (f64, f64),
    used: &mut Vec<Rc<Count>>,
) -> (Option<f64>, Option<f64>) {
    let mut vin: Option<f64> = None;
    let mut vout: Option<f64> = None;
    let dirs: Vec<&Cand> = cand.iter().filter(|x| x.c.directional.truthy() && x.c.dir.truthy()).collect();
    if !dirs.is_empty() {
        let in_c = dirs.iter().find(|x| ang_diff(super::dir_bearing(&x.c.dir.js_string()), in_b) < 70.0);
        let out_c = dirs
            .iter()
            .find(|x| ang_diff(super::dir_bearing(&x.c.dir.js_string()), (in_b + 180.0) % 360.0) < 70.0);
        if let Some(x) = in_c {
            vin = period_vol(&x.c, per, s).map(|pv| pv.v);
            if vin.is_some() {
                used.push(x.c.clone());
            }
        }
        if let Some(x) = out_c {
            vout = period_vol(&x.c, per, s).map(|pv| pv.v);
            if vout.is_some() {
                used.push(x.c.clone());
            }
        }
        // `!(lanes > 0)`: NaN lanes count as none
        if in_lanes <= 0.0 || in_lanes.is_nan() {
            vin = Some(0.0);
        }
        if out_lanes <= 0.0 || out_lanes.is_nan() {
            vout = Some(0.0);
        }
    } else if let Some(tw) = cand.iter().find(|x| !x.c.directional.truthy()) {
        let v = period_vol(&tw.c, per, s).map(|pv| pv.v);
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
    (vin, vout)
}

// The flows and `gwSrc` text of one zone; None when it is not a gateway, has no link, or no count is near it.
fn gateway_flows(sc: &Scenario, s: &Settings, matched: &Matches, pools: &Pools, zid: &str) -> Option<GateFlows> {
    let zone = sc.zones.get(zid)?;
    if !zone.kind.is("gateway") {
        return None;
    }
    let node = zone.node.as_str().filter(|n| !n.is_empty())?;
    let g_node = sc.nodes.get(node)?;
    let ls = links_at(sc, node);
    let lg: &Link = *ls.first()?;
    let gate = Gate {
        link: lg,
        ll: [g_node.lat.num(), g_node.lng.num()],
        name: norm(lg.name.as_str().unwrap_or("")),
        rank: lg.class().rank,
    };
    let from_a = lg.a == node;
    let in_b = entry_bearing(&sc.geom(lg), from_a);
    let mut cand = near(sc, matched, &gate, &pools.usable);
    if cand.is_empty() && !pools.estimates.is_empty() {
        cand = near(sc, matched, &gate, &pools.estimates);
    }
    if cand.is_empty() {
        cand = near(sc, matched, &gate, &pools.fallback);
    }
    if cand.is_empty() {
        return None;
    }
    let lanes = if from_a {
        (lg.lanes_ab.num(), lg.lanes_ba.num())
    } else {
        (lg.lanes_ba.num(), lg.lanes_ab.num())
    };
    let mut flows: Vec<(String, f64)> = Vec::new();
    let mut used: Vec<Rc<Count>> = Vec::new();
    for per in ["AM", "PM"] {
        let (vin, vout) = period_flow(&cand, per, s, in_b, lanes, &mut used);
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
    let src = if uniq.is_empty() { None } else { Some(uniq.iter().map(|c| label(c)).collect::<Vec<_>>().join("; ")) };
    Some((flows, src))
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
/// A matched count that gives no volume in a period stops the prototype with a `TypeError` (`periodVol(...).v` of
/// null); here it adds no flow for that period and is not named in `gwSrc`.
pub fn gateways_from_counts(sc: &mut Scenario, s: &Settings, data: &Data) -> usize {
    let matched = matches(sc, s, data);
    let in_use_matched = |est: bool| -> Vec<Rc<Count>> {
        data.counts
            .iter()
            .filter(|c| data.in_use(c) && is_estimate(c) == est && matched.get(&c.id).is_some())
            .cloned()
            .collect()
    };
    let usable = in_use_matched(false);
    let estimates = in_use_matched(true);
    let nzta_on = data.use_of("nzta").truthy();
    let fallback: Vec<Rc<Count>> = data
        .counts
        .iter()
        .filter(|c| c.virtual_.truthy() && !c.off.truthy() && nzta_on && matched.get(&c.id).is_some())
        .cloned()
        .collect();
    let pools = Pools { usable, estimates, fallback };
    let mut set: usize = 0;
    let ids: Vec<String> = sc.zones.keys().map(String::from).collect();
    for zid in ids {
        let Some((flows, src)) = gateway_flows(sc, s, &matched, &pools, &zid) else { continue };
        if let Some(zone) = sc.zones.get_mut(&zid) {
            // `z.gw ||= {}`
            if zone.gw.get().is_none() {
                zone.gw = Maybe::Set(OrderedMap::new());
            }
            if let Some(gw) = zone.gw.get_mut() {
                for (k, v) in flows {
                    gw.insert(k, Num::number(v));
                }
            }
            if let Some(src) = src {
                zone.gw_src = Maybe::Set(Value::String(src));
                // Bound: one per zone, far below usize::MAX
                set = set.saturating_add(1);
            }
        }
    }
    set
}
