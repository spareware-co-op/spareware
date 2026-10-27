//! The movement method on a graph: entry data per controller, movement flows and the capacity update.

use super::lanes::{Lmv, lane_model};
use super::movement::{CapOpts, MajorState, MoveEntry, priority_caps, rbt_caps};
use crate::graph::{Ctl, Graph, MVC, traced_exits, turn_type};
use crate::jsnum::{math_max, math_max_of, math_min, value_truthy};
use crate::project::Scenario;
use crate::standards::junction_set;
use serde_json::Value;

fn leg_key(m: Option<&usize>) -> String {
    match m {
        Some(k) => format!("g{k}"),
        None => "gundefined".to_string(),
    }
}

/// `prepMoves(sc, G, ctl)`: the static entry data of controller k (not signals): lane model, gaps, legs and, at node controllers, the turns out of each entry.
pub(crate) fn prep_moves(sc: &Scenario, g: &mut Graph, k: usize) {
    let ctl = &g.ctl[k];
    if ctl.kind == "signals" {
        return;
    }
    let site = ctl
        .site
        .as_ref()
        .and_then(|s| sc.sites.get().and_then(|m| m.get(&s.site)));
    let mut ent: Vec<MoveEntry> = Vec::new();
    for &i in &ctl.arcs {
        let a = &g.arcs[i];
        if a.ring || (a.ctrl == crate::graph::Ctrl::Free && !a.major) {
            continue;
        }
        let node = sc.nodes.get(&g.ids[a.to]);
        let from_site = site
            .and_then(|s| s.lanes.get())
            .and_then(|l| l.get(&a.link))
            .filter(|v| value_truthy(v));
        let from_node = node
            .and_then(|n| n.lanes.get())
            .and_then(|l| l.get(&a.link))
            .filter(|v| value_truthy(v));
        let raw: Option<&Value> = from_site.or(from_node);
        let gaps = site
            .and_then(|s| s.gaps.get())
            .filter(|v| value_truthy(v))
            .or_else(|| node.and_then(|n| n.gaps.get()).filter(|v| value_truthy(v)))
            .cloned();
        let lm = lane_model(raw, a.gl as f64);
        let thru = math_max(
            1.0,
            lm.lanes
                .iter()
                .filter(|l| l.d.contains('T') && !(l.len > 0.0))
                .count() as f64,
        );
        let mv_t = if ctl.site.is_none() {
            let mut v = Vec::new();
            for t in g.search.turns.of(i) {
                if g.tpen()[t] != f64::INFINITY {
                    let b = &g.arcs[g.t_to()[t] as usize];
                    let leg = if b.conn {
                        format!("c:{}", b.zone.as_deref().unwrap_or("undefined"))
                    } else {
                        b.link.clone()
                    };
                    v.push((t, MVC[g.tmv[t] as usize], leg));
                }
            }
            Some(v)
        } else {
            None
        };
        let nc = if ctl.kind == "roundabout" {
            Some(match &a.ring_in {
                Some(ri) if !ri.is_empty() => math_max_of(
                    &ri.iter()
                        .map(|&j| math_max(1.0, g.arcs[j].gl as f64))
                        .collect::<Vec<_>>(),
                ),
                _ => 0.0,
            })
        } else {
            None
        };
        ent.push(MoveEntry {
            i,
            major: a.major,
            in_b: a.in_b,
            leg: match &ctl.site {
                Some(s) => leg_key(s.leg_in.get(&i)),
                None => a.link.clone(),
            },
            gl: math_max(1.0, a.gl as f64),
            s_lane: a.cap_mid / math_max(1.0, a.gl as f64),
            stop: a.stop,
            gaps,
            lanes: lm.lanes,
            median: lm.median,
            slip: lm.slip,
            lm_src: lm.lm_src,
            thru,
            mv_t,
            nc,
            vc: None,
            v: Lmv::default(),
            l_to: Vec::new(),
            res: None,
            g: None,
            near: None,
            far: None,
            opp_minor: None,
            opp: None,
        });
    }
    if ctl.kind == "roundabout" {
        let mx = math_min(
            2.0,
            math_max(1.0, math_max_of(&ent.iter().map(|e| e.gl).collect::<Vec<_>>())),
        );
        for e in &mut ent {
            if !crate::jsnum::truthy_number(e.nc.unwrap_or(0.0)) {
                e.nc = Some(mx);
            }
        }
    }
    for e in &ent {
        g.arcs[e.i].lm_src = Some(e.lm_src.clone());
    }
    g.ctl[k].ent = Some(ent);
}

/// `siteExits(G, ctl, xT)`: site movements by chaining the internal links' turn shares node by node (entry arc to exit arcs and pcu/h).
///
/// The fallback for a graph without traced site flows: chaining shares invents U-turns on
/// multi-node sites, so the assignment traces the movements along its paths instead.
pub fn site_exits(g: &Graph, ctl: &Ctl, x_t: &[f64]) -> Vec<(usize, Vec<(usize, f64)>)> {
    fn add(m: &mut Vec<(usize, f64)>, b: usize, v: f64) {
        match m.iter_mut().find(|e| e.0 == b) {
            Some(e) => e.1 += v,
            None => m.push((b, v)),
        }
    }
    fn walk(g: &Graph, x_t: &[f64], m: &mut Vec<(usize, f64)>, a: usize, f: f64, depth: u32) {
        let mut tot = 0.0;
        for t in g.search.turns.of(a) {
            tot += x_t[t];
        }
        if !(tot > 0.0) {
            return;
        }
        for t in g.search.turns.of(a) {
            let v = x_t[t];
            if !(v > 0.0) {
                continue;
            }
            let b = g.t_to()[t] as usize;
            let gg = f * v / tot;
            if g.arcs[b].internal && depth < 8 {
                walk(g, x_t, m, b, gg, depth + 1);
            } else {
                add(m, b, gg);
            }
        }
    }
    let mut out: Vec<(usize, Vec<(usize, f64)>)> = Vec::new();
    for &a in &ctl.arcs {
        let mut m = Vec::new();
        for t in g.search.turns.of(a) {
            let v = x_t[t];
            if !(v > 0.0) {
                continue;
            }
            let b = g.t_to()[t] as usize;
            if g.arcs[b].internal {
                walk(g, x_t, &mut m, b, v, 1);
            } else {
                add(&mut m, b, v);
            }
        }
        crate::graph::sites_map_set(&mut out, a, m);
    }
    out
}

/// `siteMv(G, ctl, a, b)`: movement of a site movement from entry arc a to exit arc b ("U" when it leaves on the leg it entered).
pub fn site_mv(g: &Graph, ctl: &Ctl, a: usize, b: usize) -> &'static str {
    let bb = &g.arcs[b];
    if !bb.conn {
        let st = ctl.site.as_ref();
        let li = st.and_then(|s| s.leg_in.get(&a));
        let lo = st.and_then(|s| s.leg_out.get(&b));
        if li == lo {
            return "U";
        }
    }
    turn_type(g.arcs[a].in_b, bb.out_b)
}

/// `ctlFlows(G, ctl, xT, pff)`: each entry's movement flows (pcu/h at the peak rate) and left-turn flows by leg, from turn flows or traced site flows.
pub fn ctl_flows(g: &Graph, ctl: &Ctl, ent: &mut [MoveEntry], x_t: &[f64], pff: f64) {
    let s = match (&ctl.site, &g.sm, &g.sm_x) {
        (Some(st), Some(sm), Some(_)) => sm.of.get(&st.site).copied(),
        _ => None,
    };
    let ex = if ctl.site.is_some() {
        Some(match s {
            Some(s) => traced_exits(g, s, g.sm_x.as_deref().expect("traced flows")),
            None => site_exits(g, ctl, x_t),
        })
    } else {
        None
    };
    for e in ent.iter_mut() {
        let mut v = Lmv::default();
        let mut l_to: Vec<(String, f64)> = Vec::new();
        let mut add = |m: &str, f: f64, leg: String| {
            let k = match m {
                "U" => 'R',
                "" => 'T',
                _ => m.chars().next().unwrap_or('T'),
            };
            *v.get_mut(k) += f;
            if k == 'L' {
                match l_to.iter_mut().find(|x| x.0 == leg) {
                    Some(x) => x.1 = crate::jsnum::or_default(x.1, 0.0) + f,
                    None => l_to.push((leg, f)),
                }
            }
        };
        match &ex {
            None => {
                for (t, m, leg) in e.mv_t.iter().flatten() {
                    let f = x_t[*t] / pff;
                    if f > 0.0 {
                        add(m, f, leg.clone());
                    }
                }
            }
            Some(ex) => {
                if let Some((_, m)) = ex.iter().find(|x| x.0 == e.i) {
                    for &(b, f) in m {
                        let bb = &g.arcs[b];
                        let leg = if bb.conn {
                            format!("c:{}", bb.zone.as_deref().unwrap_or("undefined"))
                        } else {
                            leg_key(ctl.site.as_ref().and_then(|s| s.leg_out.get(&b)))
                        };
                        add(site_mv(g, ctl, e.i, b), f / pff, leg);
                    }
                }
            }
        }
        e.v = v;
        e.l_to = l_to;
    }
}

/// `movementCaps(G, xt, w, xT)`: the movement method's capacity update for every controller with entries.
///
/// Each minor or roundabout entry's capN becomes min(capMid, max(80 or 50, its capacity)); a
/// major approach keeps its right-turn state for the delay it adds. w below 1 damps the update
/// (MSA). The movement detail stays on the arc (`mv_res`).
pub fn movement_caps(g: &mut Graph, xt: &[f64], w: f64, x_t: &[f64]) {
    let j = junction_set(&g.settings);
    let damp = |old: Option<f64>, nw: f64| match old {
        Some(o) if w < 1.0 && o.is_finite() => o + (nw - o) * w,
        _ => nw,
    };
    let opts = CapOpts::new(j.t, j.queue_space, g.mv_detail);
    for k in 0..g.ctl.len() {
        let Some(mut ent) = g.ctl[k].ent.take() else {
            continue;
        };
        let rbt = g.ctl[k].kind == "roundabout";
        ctl_flows(g, &g.ctl[k], &mut ent, x_t, j.pff);
        if rbt {
            for e in &mut ent {
                let a = &g.arcs[e.i];
                let mut vc = 0.0;
                if let Some(ro) = &a.ring_out {
                    for &jj in ro {
                        vc += xt[jj];
                    }
                    vc = math_max(0.0, vc - xt[a.i]);
                } else if let Some(conf) = &a.conf {
                    for &(jj, f) in conf {
                        vc += xt[jj] * f;
                    }
                }
                e.vc = Some(vc / j.pff);
            }
            rbt_caps(&mut ent, &opts);
        } else {
            priority_caps(&mut ent, &opts);
        }
        for e in &ent {
            let r = e.res.as_ref().expect("capacity result");
            let a = &mut g.arcs[e.i];
            a.mv_res = Some(r.clone());
            if e.major {
                let mr = r.major.expect("major detail");
                let cm_r = r.mov.r.map_or(f64::NAN, |x| x.cm);
                a.mj = Some(match a.mj {
                    None => MajorState {
                        s_r: mr.s_r,
                        sh_tl: mr.sh_tl,
                        cm_r,
                        rb: mr.rb,
                    },
                    Some(old) => MajorState {
                        s_r: damp(Some(old.s_r), mr.s_r),
                        sh_tl: damp(Some(old.sh_tl), mr.sh_tl),
                        cm_r: damp(Some(old.cm_r), cm_r),
                        rb: mr.rb,
                    },
                });
            } else {
                let floor = if rbt { 50.0 } else { 80.0 };
                a.cap_n = Some(damp(a.cap_n, math_min(a.cap_mid, math_max(floor, r.cap))));
            }
        }
        g.ctl[k].ent = Some(ent);
    }
}
