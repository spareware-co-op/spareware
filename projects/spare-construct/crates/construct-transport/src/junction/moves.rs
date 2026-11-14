//! The movement method on a graph: entry data per controller, movement flows and the capacity update.

use super::lanes::{Lmv, lane_model};
use super::movement::{CapOpts, MajorState, MoveEntry, priority_caps, rbt_caps};
use crate::graph::{Arc, Ctl, Graph, MVC, traced_exits, turn_type};
use crate::jsnum::{math_max, math_max_of, math_min, usize_to_f64, value_truthy};
use crate::project::Scenario;
use crate::standards::junction_set;
use serde_json::Value;

fn leg_key(m: Option<&usize>) -> String {
    match m {
        Some(k) => format!("g{k}"),
        None => "gundefined".to_string(),
    }
}

// The arc turn t enters, with its index. Every turn of the graph enters one of its arcs.
fn turn_target(g: &Graph, t: usize) -> Option<(usize, &Arc)> {
    let to = usize::try_from(*g.t_to().get(t)?).ok()?;
    Some((to, g.arcs.get(to)?))
}

// The turns out of arc ai that are not banned: turn index, movement and the leg it goes to.
fn turns_out(g: &Graph, ai: usize) -> Vec<(usize, &'static str, String)> {
    let mut out = Vec::new();
    for t in g.search.turns.of(ai) {
        // Every turn has a penalty, a movement and an arc it enters, so nothing is skipped
        if g.tpen().get(t).is_some_and(|&p| p != f64::INFINITY)
            && let Some((_, to_arc)) = turn_target(g, t)
        {
            let leg = if to_arc.conn() {
                format!("c:{}", to_arc.zone.as_deref().unwrap_or("undefined"))
            } else {
                to_arc.link.clone()
            };
            let mv = g
                .tmv
                .get(t)
                .and_then(|&m| MVC.get(usize::from(m)))
                .copied()
                .unwrap_or("");
            out.push((t, mv, leg));
        }
    }
    out
}

// Circulating lanes at a roundabout entry: the most general traffic lanes of the ring arcs coming in (0 with none).
fn circ_lanes(g: &Graph, arc: &Arc) -> f64 {
    match &arc.ring_in {
        // Ring arcs are arcs of the graph, so the NaN fallback never applies
        Some(ri) if !ri.is_empty() => math_max_of(
            &ri.iter()
                .map(|&rj| math_max(1.0, g.arcs.get(rj).map_or(f64::NAN, |ra| f64::from(ra.gl))))
                .collect::<Vec<_>>(),
        ),
        _ => 0.0,
    }
}

// Roundabout entries without circulating lanes take the entries' most lanes, within 1 to 2.
fn default_circ_lanes(ent: &mut [MoveEntry]) {
    let mx = math_min(
        2.0,
        math_max(1.0, math_max_of(&ent.iter().map(|e| e.gl).collect::<Vec<_>>())),
    );
    for e in ent.iter_mut() {
        if !crate::jsnum::truthy_number(e.nc.unwrap_or(0.0)) {
            e.nc = Some(mx);
        }
    }
}

/// `prepMoves(sc, G, ctl)`: the static entry data of controller k (not signals): lane model, gaps, legs and, at node controllers, the turns out of each entry.
pub(crate) fn prep_moves(sc: &Scenario, g: &mut Graph, k: usize) {
    let Some(ctl) = g.ctl.get(k) else {
        return;
    };
    if ctl.kind == "signals" {
        return;
    }
    let site = ctl
        .site
        .as_ref()
        .and_then(|s| sc.sites.get().and_then(|m| m.get(&s.site)));
    let mut ent: Vec<MoveEntry> = Vec::new();
    for &i in &ctl.arcs {
        // A controller's arcs are arcs of the graph
        let Some(arc) = g.arcs.get(i) else {
            continue;
        };
        if arc.ring || (arc.ctrl == crate::graph::Ctrl::Free && !arc.major()) {
            continue;
        }
        let node = g.ids.get(arc.to).and_then(|id| sc.nodes.get(id));
        let from_site = site
            .and_then(|s| s.lanes.get())
            .and_then(|l| l.get(&arc.link))
            .filter(|v| value_truthy(v));
        let from_node = node
            .and_then(|n| n.lanes.get())
            .and_then(|l| l.get(&arc.link))
            .filter(|v| value_truthy(v));
        let raw: Option<&Value> = from_site.or(from_node);
        let gaps = site
            .and_then(|s| s.gaps.get())
            .filter(|v| value_truthy(v))
            .or_else(|| node.and_then(|n| n.gaps.get()).filter(|v| value_truthy(v)))
            .cloned();
        let lm = lane_model(raw, f64::from(arc.gl));
        // !(len > 0): a NaN bay length counts as a full lane
        let thru = math_max(
            1.0,
            usize_to_f64(
                lm.lanes
                    .iter()
                    .filter(|l| l.d.contains('T') && (l.len <= 0.0 || l.len.is_nan()))
                    .count(),
            ),
        );
        let mv_t = ctl.site.is_none().then(|| turns_out(g, i));
        let nc = (ctl.kind == "roundabout").then(|| circ_lanes(g, arc));
        ent.push(MoveEntry {
            i,
            priority: arc.priority,
            in_b: arc.in_b,
            leg: match &ctl.site {
                Some(s) => leg_key(s.leg_in.get(&i)),
                None => arc.link.clone(),
            },
            gl: math_max(1.0, f64::from(arc.gl)),
            s_lane: arc.cap_mid / math_max(1.0, f64::from(arc.gl)),
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
        default_circ_lanes(&mut ent);
    }
    for e in &ent {
        if let Some(arc) = g.arcs.get_mut(e.i) {
            arc.lm_src = Some(e.lm_src.clone());
        }
    }
    if let Some(ctl) = g.ctl.get_mut(k) {
        ctl.ent = Some(ent);
    }
}

/// `siteExits(G, ctl, xT)`: site movements by chaining the internal links' turn shares node by node (entry arc to exit arcs and pcu/h).
///
/// The fallback for a graph without traced site flows: chaining shares invents U-turns on
/// multi-node sites, so the assignment traces the movements along its paths instead.
#[must_use]
pub fn site_exits(g: &Graph, ctl: &Ctl, x_t: &[f64]) -> Vec<(usize, Vec<(usize, f64)>)> {
    fn add(m: &mut Vec<(usize, f64)>, b: usize, v: f64) {
        match m.iter_mut().find(|e| e.0 == b) {
            Some(e) => e.1 += v,
            None => m.push((b, v)),
        }
    }
    // xT holds one flow per turn; a turn outside it reads NaN, as JavaScript reads undefined
    fn turn_flow(x_t: &[f64], turn: usize) -> f64 {
        x_t.get(turn).copied().unwrap_or(f64::NAN)
    }
    fn walk(g: &Graph, x_t: &[f64], acc: &mut Vec<(usize, f64)>, arc: usize, share: f64, depth: u32) {
        let mut tot = 0.0;
        for turn in g.search.turns.of(arc) {
            tot += turn_flow(x_t, turn);
        }
        if tot <= 0.0 || tot.is_nan() {
            return;
        }
        for turn in g.search.turns.of(arc) {
            let flow = turn_flow(x_t, turn);
            if flow <= 0.0 || flow.is_nan() {
                continue;
            }
            let Some((to, to_arc)) = turn_target(g, turn) else {
                continue;
            };
            let gg = share * flow / tot;
            if to_arc.internal && depth < 8 {
                // Bound: depth stops at 8
                walk(g, x_t, acc, to, gg, depth.saturating_add(1));
            } else {
                add(acc, to, gg);
            }
        }
    }
    let mut out: Vec<(usize, Vec<(usize, f64)>)> = Vec::new();
    for &a in &ctl.arcs {
        let mut m = Vec::new();
        for t in g.search.turns.of(a) {
            let v = turn_flow(x_t, t);
            if v <= 0.0 || v.is_nan() {
                continue;
            }
            let Some((b, b_arc)) = turn_target(g, t) else {
                continue;
            };
            if b_arc.internal {
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
#[must_use]
pub fn site_mv(g: &Graph, ctl: &Ctl, a: usize, b: usize) -> &'static str {
    // a and b are arcs of the graph, so the NaN bearings never apply
    let bb = g.arcs.get(b);
    if bb.is_some_and(|x| !x.conn()) {
        let st = ctl.site.as_ref();
        let li = st.and_then(|s| s.leg_in.get(&a));
        let lo = st.and_then(|s| s.leg_out.get(&b));
        if li == lo {
            return "U";
        }
    }
    turn_type(
        g.arcs.get(a).map_or(f64::NAN, |x| x.in_b),
        bb.map_or(f64::NAN, |x| x.out_b),
    )
}

/// `ctlFlows(G, ctl, xT, pff)`: each entry's movement flows (pcu/h at the peak rate) and left-turn flows by leg, from turn flows or traced site flows.
pub fn ctl_flows(g: &Graph, ctl: &Ctl, ent: &mut [MoveEntry], x_t: &[f64], pff: f64) {
    let traced = match (&ctl.site, &g.sm, g.sm_x.as_deref()) {
        (Some(st), Some(sm), Some(xm)) => sm.of.get(&st.site).copied().map(|slot| (slot, xm)),
        _ => None,
    };
    let ex = if ctl.site.is_some() {
        Some(match traced {
            Some((slot, xm)) => traced_exits(g, slot, xm),
            None => site_exits(g, ctl, x_t),
        })
    } else {
        None
    };
    for e in ent.iter_mut() {
        let mut flows = Lmv::default();
        let mut l_to: Vec<(String, f64)> = Vec::new();
        let mut add = |mvc: &str, flow: f64, leg: String| {
            let mv = match mvc {
                "U" => 'R',
                "" => 'T',
                _ => mvc.chars().next().unwrap_or('T'),
            };
            *flows.get_mut(mv) += flow;
            if mv == 'L' {
                match l_to.iter_mut().find(|x| x.0 == leg) {
                    Some(x) => x.1 = crate::jsnum::or_default(x.1, 0.0) + flow,
                    None => l_to.push((leg, flow)),
                }
            }
        };
        match &ex {
            None => {
                for (t, m, leg) in e.mv_t.iter().flatten() {
                    // xT holds one flow per turn; NaN (undefined) is not above 0 and adds nothing
                    let f = x_t.get(*t).copied().unwrap_or(f64::NAN) / pff;
                    if f > 0.0 {
                        add(m, f, leg.clone());
                    }
                }
            }
            Some(ex) => {
                if let Some((_, m)) = ex.iter().find(|x| x.0 == e.i) {
                    for &(b, f) in m {
                        // Exit arcs are arcs of the graph
                        let Some(bb) = g.arcs.get(b) else {
                            continue;
                        };
                        let leg = if bb.conn() {
                            format!("c:{}", bb.zone.as_deref().unwrap_or("undefined"))
                        } else {
                            leg_key(ctl.site.as_ref().and_then(|s| s.leg_out.get(&b)))
                        };
                        add(site_mv(g, ctl, e.i, b), f / pff, leg);
                    }
                }
            }
        }
        e.v = flows;
        e.l_to = l_to;
    }
}

/// `movementCaps(G, xt, w, xT)`: the movement method's capacity update for every controller with entries.
///
/// Each minor or roundabout entry's capN becomes min(capMid, max(80 or 50, its capacity)); a
/// major approach keeps its right-turn state for the delay it adds. w below 1 damps the update
/// (MSA). The movement detail stays on the arc (`mv_res`).
pub fn movement_caps(g: &mut Graph, xt: &[f64], w: f64, x_t: &[f64]) {
    let js = junction_set(&g.settings);
    let damp = |old: Option<f64>, nw: f64| match old {
        Some(o) if w < 1.0 && o.is_finite() => o + (nw - o) * w,
        _ => nw,
    };
    // xt holds one flow per arc; an arc outside it reads NaN, as JavaScript reads undefined
    let arc_flow = |ai: usize| xt.get(ai).copied().unwrap_or(f64::NAN);
    let opts = CapOpts::new(js.t, js.queue_space, g.mv_detail);
    for k in 0..g.ctl.len() {
        let Some(mut ent) = g.ctl.get_mut(k).and_then(|ctl| ctl.ent.take()) else {
            continue;
        };
        let rbt = g.ctl.get(k).is_some_and(|ctl| ctl.kind == "roundabout");
        if let Some(ctl) = g.ctl.get(k) {
            ctl_flows(g, ctl, &mut ent, x_t, js.pff);
        }
        if rbt {
            for e in &mut ent {
                // An entry's arc is an arc of the graph
                let Some(arc) = g.arcs.get(e.i) else {
                    continue;
                };
                let mut vc = 0.0;
                if let Some(ro) = &arc.ring_out {
                    for &jj in ro {
                        vc += arc_flow(jj);
                    }
                    vc = math_max(0.0, vc - arc_flow(arc.i));
                } else if let Some(conf) = &arc.conf {
                    for &(jj, f) in conf {
                        vc += arc_flow(jj) * f;
                    }
                }
                e.vc = Some(vc / js.pff);
            }
            rbt_caps(&mut ent, &opts);
        } else {
            priority_caps(&mut ent, &opts);
        }
        for e in &ent {
            // rbt_caps and priority_caps set every entry's result, a major entry's with the major detail
            let Some(res) = e.res.as_ref() else {
                continue;
            };
            let Some(arc) = g.arcs.get_mut(e.i) else {
                continue;
            };
            arc.mv_res = Some(res.clone());
            if e.major() {
                let Some(mr) = res.major else {
                    continue;
                };
                let cm_r = res.mov.r.map_or(f64::NAN, |x| x.cm);
                arc.mj = Some(match arc.mj {
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
                arc.cap_n = Some(damp(arc.cap_n, math_min(arc.cap_mid, math_max(floor, res.cap))));
            }
        }
        if let Some(ctl) = g.ctl.get_mut(k) {
            ctl.ent = Some(ent);
        }
    }
}
