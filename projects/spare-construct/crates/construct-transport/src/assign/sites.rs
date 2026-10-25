//! Results by junction site (engine.js `siteResults`): each site's legs, its controller node, the
//! movements through it traced along the assignment's paths, and for roundabout sites (one
//! controller per entry) the entries reported together.

use crate::graph::{Ctl, CtlSrc, Graph, Leg, js_sort_by, js_str_lt, site_control, site_legs, turn_type};
use crate::jsnum::{Maybe, math_max, math_max_of, math_round};
use crate::junction::site_exits;
use crate::project::{OrderedMap, Scenario};
use crate::results::{Approach, NodeRes, SiteLeg, SiteRes, SiteResult, TurnRow};
use crate::standards::{junction_set, los_lt, los_of};
use crate::util::{r1, r3};
use std::collections::{HashMap, HashSet};

/// Site results and the movements through each site.
pub(super) struct SiteOut {
    pub sites: OrderedMap<SiteRes>,
    pub site_turns: OrderedMap<Vec<TurnRow>>,
    pub traced: bool,
}

// siteMv(G, pseudo, a, b): "U" when the movement leaves on the leg it entered, else by the bearings
fn site_mv(g: &Graph, li: &HashMap<usize, usize>, lo: &HashMap<usize, usize>, a: usize, b: usize) -> &'static str {
    let bb = &g.arcs[b];
    if !bb.conn && li.get(&a) == lo.get(&b) {
        return "U";
    }
    turn_type(g.arcs[a].in_b, bb.out_b)
}

/// `siteResults(sc, G, xT, pf, nodesRes, sm)`: None when the graph has no sites.
///
/// sm is the traced movement flows (all traffic, development traffic) by site-movement slot,
/// when the run traced them; otherwise the movements are chained from the turn flows x_t.
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
    let mut sites: OrderedMap<SiteRes> = OrderedMap::new();
    let mut site_turns: OrderedMap<Vec<TurnRow>> = OrderedMap::new();
    let j = junction_set(&g.settings);
    for (s, gs) in g.sites.iter().enumerate() {
        let site = &gs.site;
        let members = &gs.members;
        let st = &g.sm.as_ref().expect("site movement slots").list[s];
        let (inc, exits) = (&st.inc, &st.exits);
        let ctl = g.ctl.iter().find(|c| c.site.as_ref().is_some_and(|x| x.site == site.id));
        let legs: Vec<Leg> = match ctl.and_then(|c| c.site.as_ref()) {
            Some(sc_ctl) => sc_ctl.legs.clone(),
            None => site_legs(g, inc, exits),
        };
        let mut leg_in: HashMap<usize, usize> = HashMap::new();
        let mut leg_out: HashMap<usize, usize> = HashMap::new();
        for l in &legs {
            for &i in &l.ins {
                leg_in.insert(i, l.k);
            }
            for &i in &l.outs {
                leg_out.insert(i, l.k);
            }
        }
        let mut rows: Vec<TurnRow> = Vec::new();
        if let Some((xm, xd)) = sm {
            for (e, &a) in inc.iter().enumerate() {
                for x in 0..st.nx {
                    let k = st.off + e * st.nx + x;
                    let f = xm[k];
                    let v = r1(f / pf);
                    if !(v > 0.0) {
                        continue;
                    }
                    let b = exits[x];
                    let dv = r1(xd[k] / pf);
                    rows.push(TurnRow {
                        from: g.arcs[a].key.clone(),
                        to: g.arcs[b].key.clone(),
                        mv: site_mv(g, &leg_in, &leg_out, a, b).to_string(),
                        v,
                        pcu: r1(f),
                        dev: (dv > 0.0).then(|| crate::jsnum::math_min(v, dv)),
                    });
                }
            }
        } else {
            let pseudo = Ctl {
                kind: String::new(),
                n: 0,
                arcs: inc.clone(),
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
            for &a in inc.iter() {
                let Some((_, outs)) = ex.iter().find(|e| e.0 == a) else {
                    continue;
                };
                for &(b, f) in outs {
                    let v = r1(f / pf);
                    if v > 0.0 {
                        rows.push(TurnRow {
                            from: g.arcs[a].key.clone(),
                            to: g.arcs[b].key.clone(),
                            mv: site_mv(g, &leg_in, &leg_out, a, b).to_string(),
                            v,
                            pcu: r1(f),
                            dev: None,
                        });
                    }
                }
            }
        }
        if !rows.is_empty() {
            site_turns.insert(site.id.clone(), rows);
        }
        let mut by_rank = legs.clone();
        js_sort_by(&mut by_rank, |p, q| q.rank - p.rank);
        let mut names: Vec<&str> = Vec::new();
        for l in &by_rank {
            if !l.name.is_empty() && !names.contains(&l.name.as_str()) {
                names.push(&l.name);
            }
        }
        let rep_n = match ctl {
            Some(c) => c.n,
            None => {
                let mut sorted = members.clone();
                js_sort_by(&mut sorted, |&a, &b| {
                    let d = (g.deg[b] - g.deg[a]) as f64;
                    if d != 0.0 {
                        d
                    } else if js_str_lt(&g.ids[a], &g.ids[b]) {
                        -1.0
                    } else {
                        1.0
                    }
                });
                sorted[0]
            }
        };
        let mut result: Option<SiteResult> = None;
        if ctl.is_none() {
            // per-node controllers (roundabouts): the approaches from outside the site, together
            let keys: HashSet<&str> = inc.iter().map(|&i| g.arcs[i].key.as_str()).collect();
            let mut appr: Vec<Approach> = Vec::new();
            for &n in members {
                if let Some(nr) = nodes_res.get(&g.ids[n]) {
                    for a in &nr.appr {
                        if keys.contains(a.key.as_str()) {
                            appr.push(a.clone());
                        }
                    }
                }
            }
            if !appr.is_empty() {
                let ctl_a: Vec<&Approach> = appr.iter().filter(|a| a.ctrl != "free").collect();
                let sv = ctl_a.iter().fold(0.0f64, |s, a| s + a.v);
                let avg = if sv > 0.0 {
                    ctl_a.iter().fold(0.0f64, |s, a| s + a.v * a.delay) / sv
                } else {
                    0.0
                };
                let first = members.iter().find_map(|&n| nodes_res.get(&g.ids[n]));
                let typ = first.map(|nr| nr.r#type.clone()).filter(|t| !t.is_empty()).unwrap_or_else(|| "roundabout".to_string());
                let los_m = j.los();
                let th = match typ.as_str() {
                    "signals" => &los_m.signals,
                    "roundabout" => &los_m.roundabout,
                    _ => &los_m.sign,
                };
                let los = if j.method == "rta" { los_lt(avg, th) } else { los_of(avg, th) };
                let doses: Vec<f64> = ctl_a.iter().map(|a| a.dos).collect();
                result = Some(SiteResult {
                    r#type: typ,
                    appr: appr.clone(),
                    delay: r1(avg),
                    avg_delay: r1(avg),
                    delay_basis: "average".to_string(),
                    dos: r3(math_max(0.0, math_max_of(&doses))),
                    vol: math_round(appr.iter().fold(0.0f64, |s, a| s + a.v)),
                    los: los.to_string(),
                    intersection_los: los.to_string(),
                    los_text: los.to_string(),
                });
            }
        }
        let name = match site.name.as_str() {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => {
                let joined = names.iter().take(3).copied().collect::<Vec<_>>().join(" / ");
                if joined.is_empty() { "Unnamed junction".to_string() } else { joined }
            }
        };
        sites.insert(
            site.id.clone(),
            SiteRes {
                id: site.id.clone(),
                name,
                nodes: members.iter().map(|&n| g.ids[n].clone()).collect(),
                rep: g.ids[rep_n].clone(),
                r#type: match ctl {
                    Some(c) => c.kind.clone(),
                    None => site_control(sc, site).to_string(),
                },
                at_site_no: site.at_site_no.get().cloned().unwrap_or(serde_json::Value::Null),
                one: ctl.is_some(),
                legs: legs
                    .iter()
                    .map(|l| SiteLeg {
                        k: l.k as f64,
                        name: l.name.clone(),
                        b: l.b,
                        ins: l.ins.iter().map(|&i| g.arcs[i].key.clone()).collect(),
                        outs: l.outs.iter().map(|&i| g.arcs[i].key.clone()).collect(),
                    })
                    .collect(),
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
