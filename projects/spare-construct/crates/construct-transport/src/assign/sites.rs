//! Results by junction site (engine.js `siteResults`): each site's legs, its controller node, the
//! movements through it traced along the assignment's paths, and for roundabout sites (one
//! controller per entry) the entries reported together.

use crate::graph::{Ctl, CtlSrc, Graph, Leg, SiteMoveList, js_sort_by, js_str_lt, site_control, site_legs, turn_type};
use crate::jsnum::{Maybe, math_max, math_max_of, math_round, usize_to_f64};
use crate::junction::site_exits;
use crate::project::{OrderedMap, Scenario, Site};
use crate::results::{Approach, NodeRes, SiteLeg, SiteRes, SiteResult, TurnRow};
use crate::standards::{JunctionSet, junction_set, los_lt, los_of};
use crate::util::{r1, r3};
use std::collections::{HashMap, HashSet};

/// Site results and the movements through each site.
pub(super) struct SiteOut {
    pub sites: OrderedMap<SiteRes>,
    pub site_turns: OrderedMap<Vec<TurnRow>>,
    pub traced: bool,
}

/// Leg of each arc entering (or leaving) a site.
type LegMap = HashMap<usize, usize>;

// siteMv(G, pseudo, a, b): "U" when the movement leaves on the leg it entered, else by the bearings
fn site_mv(g: &Graph, li: &LegMap, lo: &LegMap, a: usize, b: usize) -> &'static str {
    // Callers pass arcs of the graph; a missing one has no movement
    let Some(bb) = g.arcs.get(b) else {
        return "";
    };
    if !bb.conn() && li.get(&a) == lo.get(&b) {
        return "U";
    }
    g.arcs.get(a).map_or("", |aa| turn_type(aa.in_b, bb.out_b))
}

// The leg of each entry arc and of each exit arc
fn leg_maps(legs: &[Leg]) -> (LegMap, LegMap) {
    let mut leg_in: LegMap = HashMap::new();
    let mut leg_out: LegMap = HashMap::new();
    for leg in legs {
        for &i in &leg.ins {
            leg_in.insert(i, leg.k);
        }
        for &i in &leg.outs {
            leg_out.insert(i, leg.k);
        }
    }
    (leg_in, leg_out)
}

// The movements through a site from the traced flows (all traffic, development traffic) by slot
fn traced_rows(g: &Graph, st: &SiteMoveList, (xm, xd): (&[f64], &[f64]), pf: f64, (li, lo): (&LegMap, &LegMap)) -> Vec<TurnRow> {
    let mut rows: Vec<TurnRow> = Vec::new();
    for (row_k, &entry) in st.inc.iter().enumerate() {
        for (col_k, &exit) in st.exits.iter().enumerate().take(st.nx) {
            // off + e * nx + x. Bound: the slots are counted in memory, so this never overflows;
            // past usize it is a slot no array reaches
            let slot = row_k
                .checked_mul(st.nx)
                .and_then(|step| st.off.checked_add(step))
                .and_then(|step| step.checked_add(col_k))
                .unwrap_or(usize::MAX);
            let pcu = xm.get(slot).copied().unwrap_or(f64::NAN);
            let veh = r1(pcu / pf);
            if !(veh > 0.0) {
                continue;
            }
            let dv = r1(xd.get(slot).copied().unwrap_or(f64::NAN) / pf);
            let (Some(arc_a), Some(arc_b)) = (g.arcs.get(entry), g.arcs.get(exit)) else {
                continue;
            };
            rows.push(TurnRow {
                from: arc_a.key.clone(),
                to: arc_b.key.clone(),
                mv: site_mv(g, li, lo, entry, exit).to_string(),
                v: veh,
                pcu: r1(pcu),
                dev: (dv > 0.0).then(|| crate::jsnum::math_min(veh, dv)),
            });
        }
    }
    rows
}

// The movements through a site chained from the turn flows x_t, as one pseudo controller over its entries
fn chained_rows(g: &Graph, inc: &[usize], x_t: &[f64], pf: f64, (li, lo): (&LegMap, &LegMap)) -> Vec<TurnRow> {
    let pseudo = Ctl {
        kind: String::new(),
        n: 0,
        arcs: inc.to_vec(),
        src: CtlSrc {
            site: false,
            id: String::new(),
            cycle: Maybe::Missing,
            fixed_timing: Maybe::Missing,
        },
        signal: None,
        major: None,
        site: None,
        ent: None,
    };
    let ex = site_exits(g, &pseudo, x_t);
    let mut rows: Vec<TurnRow> = Vec::new();
    for &entry in inc {
        let Some((_, outs)) = ex.iter().find(|row| row.0 == entry) else {
            continue;
        };
        for &(exit, pcu) in outs {
            let veh = r1(pcu / pf);
            if veh > 0.0
                && let (Some(arc_a), Some(arc_b)) = (g.arcs.get(entry), g.arcs.get(exit))
            {
                rows.push(TurnRow {
                    from: arc_a.key.clone(),
                    to: arc_b.key.clone(),
                    mv: site_mv(g, li, lo, entry, exit).to_string(),
                    v: veh,
                    pcu: r1(pcu),
                    dev: None,
                });
            }
        }
    }
    rows
}

// The site's representative node: its controller's, else the member with the most links (ties by id)
fn rep_node(g: &Graph, ctl: Option<&Ctl>, members: &[usize]) -> Option<usize> {
    if let Some(cl) = ctl {
        return Some(cl.n);
    }
    // Node arrays hold one entry per node, so the fallbacks never apply
    let deg = |n: usize| f64::from(g.deg.get(n).copied().unwrap_or(0));
    let id = |n: usize| g.ids.get(n).map_or("", String::as_str);
    let mut sorted = members.to_vec();
    js_sort_by(&mut sorted, |&na, &nb| {
        // The difference of two i32 is exact in f64
        let dd = deg(nb) - deg(na);
        if dd != 0.0 {
            dd
        } else if js_str_lt(id(na), id(nb)) {
            -1.0
        } else {
            1.0
        }
    });
    sorted.first().copied()
}

// Per-node controllers (roundabouts): the approaches from outside the site, reported together
fn roundabout_result(g: &Graph, jset: &JunctionSet, nodes_res: &OrderedMap<NodeRes>, inc: &[usize], members: &[usize]) -> Option<SiteResult> {
    let res_at = |n: usize| g.ids.get(n).and_then(|id| nodes_res.get(id));
    let keys: HashSet<&str> = inc.iter().filter_map(|&i| g.arcs.get(i)).map(|arc| arc.key.as_str()).collect();
    let mut appr: Vec<Approach> = Vec::new();
    for &n in members {
        if let Some(nr) = res_at(n) {
            for ap in &nr.appr {
                if keys.contains(ap.key.as_str()) {
                    appr.push(ap.clone());
                }
            }
        }
    }
    if appr.is_empty() {
        return None;
    }
    let ctl_a: Vec<&Approach> = appr.iter().filter(|ap| ap.ctrl != "free").collect();
    let sv = ctl_a.iter().fold(0.0f64, |sum, ap| sum + ap.v);
    let avg = if sv > 0.0 {
        ctl_a.iter().fold(0.0f64, |sum, ap| sum + ap.v * ap.delay) / sv
    } else {
        0.0
    };
    let first = members.iter().find_map(|&n| res_at(n));
    let typ = first.map(|nr| nr.r#type.clone()).filter(|ty| !ty.is_empty()).unwrap_or_else(|| "roundabout".to_string());
    let los_m = jset.los();
    let th = match typ.as_str() {
        "signals" => &los_m.signals,
        "roundabout" => &los_m.roundabout,
        _ => &los_m.sign,
    };
    let los = if jset.method == "rta" { los_lt(avg, th) } else { los_of(avg, th) };
    let doses: Vec<f64> = ctl_a.iter().map(|ap| ap.dos).collect();
    Some(SiteResult {
        r#type: typ,
        appr: appr.clone(),
        delay: r1(avg),
        avg_delay: r1(avg),
        delay_basis: "average".to_string(),
        dos: r3(math_max(0.0, math_max_of(&doses))),
        vol: math_round(appr.iter().fold(0.0f64, |sum, ap| sum + ap.v)),
        los: los.to_string(),
        intersection_los: los.to_string(),
        los_text: los.to_string(),
    })
}

// The site's name, else the names of its three highest-ranked legs
fn site_name(site: &Site, legs: &[Leg]) -> String {
    match site.name.as_str() {
        Some(nm) if !nm.is_empty() => nm.to_string(),
        _ => {
            let mut by_rank = legs.to_vec();
            js_sort_by(&mut by_rank, |la, lb| lb.rank - la.rank);
            let mut names: Vec<&str> = Vec::new();
            for leg in &by_rank {
                if !leg.name.is_empty() && !names.contains(&leg.name.as_str()) {
                    names.push(&leg.name);
                }
            }
            let joined = names.iter().take(3).copied().collect::<Vec<_>>().join(" / ");
            if joined.is_empty() { "Unnamed junction".to_string() } else { joined }
        }
    }
}

// The legs as results list them, with arc keys
fn leg_results(g: &Graph, legs: &[Leg]) -> Vec<SiteLeg> {
    let keys = |arcs: &[usize]| -> Vec<String> { arcs.iter().filter_map(|&i| g.arcs.get(i)).map(|arc| arc.key.clone()).collect() };
    legs.iter()
        .map(|leg| SiteLeg {
            k: usize_to_f64(leg.k),
            name: leg.name.clone(),
            b: leg.b,
            ins: keys(&leg.ins),
            outs: keys(&leg.outs),
        })
        .collect()
}

/// `siteResults(sc, G, xT, pf, nodesRes, sm)`: None when the graph has no sites.
///
/// sm is the traced movement flows (all traffic, development traffic) by site-movement slot,
/// when the run traced them; otherwise the movements are chained from the turn flows `x_t`.
pub(super) fn site_results(
    sc: &Scenario,
    g: &Graph,
    x_t: &[f64],
    pf: f64,
    nodes_res: &OrderedMap<NodeRes>,
    sm: Option<(&[f64], &[f64])>,
) -> Option<SiteOut> {
    if g.sites.is_empty() {
        return None;
    }
    // The graph builds its movement slots whenever it has sites, one list per site
    let slots = g.sm.as_ref()?;
    let mut sites: OrderedMap<SiteRes> = OrderedMap::new();
    let mut site_turns: OrderedMap<Vec<TurnRow>> = OrderedMap::new();
    let jset = junction_set(&g.settings);
    for (si, gs) in g.sites.iter().enumerate() {
        let site = &gs.site;
        let members = &gs.members;
        let Some(st) = slots.list.get(si) else {
            continue;
        };
        let (inc, exits) = (&st.inc, &st.exits);
        let ctl = g.ctl.iter().find(|cl| cl.site.as_ref().is_some_and(|sctl| sctl.site == site.id));
        let legs: Vec<Leg> = match ctl.and_then(|cl| cl.site.as_ref()) {
            Some(sc_ctl) => sc_ctl.legs.clone(),
            None => site_legs(g, inc, exits),
        };
        let (leg_in, leg_out) = leg_maps(&legs);
        let rows = match sm {
            Some(flows) => traced_rows(g, st, flows, pf, (&leg_in, &leg_out)),
            None => chained_rows(g, inc, x_t, pf, (&leg_in, &leg_out)),
        };
        if !rows.is_empty() {
            site_turns.insert(site.id.clone(), rows);
        }
        let rep_n = rep_node(g, ctl, members);
        let result = if ctl.is_none() { roundabout_result(g, &jset, nodes_res, inc, members) } else { None };
        sites.insert(
            site.id.clone(),
            SiteRes {
                id: site.id.clone(),
                name: site_name(site, &legs),
                nodes: members.iter().filter_map(|&n| g.ids.get(n).cloned()).collect(),
                rep: rep_n.and_then(|n| g.ids.get(n)).cloned().unwrap_or_default(),
                r#type: ctl.map_or_else(|| site_control(sc, site).to_string(), |cl| cl.kind.clone()),
                at_site_no: site.at_site_no.get().cloned().unwrap_or(serde_json::Value::Null),
                one: ctl.is_some(),
                legs: leg_results(g, &legs),
                result,
            },
        );
    }
    Some(SiteOut {
        sites,
        site_turns,
        traced: sm.is_some(),
    })
}
