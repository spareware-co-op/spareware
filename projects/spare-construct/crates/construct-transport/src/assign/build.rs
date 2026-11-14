//! The result object of an assignment (the second half of engine.js `assign`): link results and
//! totals, junction results, turning movements, development tracking, the select-link analysis,
//! bus routes and the junction sites.

use super::paths::PathSet;
use super::run::{Run, T_C1, T_C2, T_TD};
use super::sites::site_results;
use crate::demand::Demand;
use crate::graph::{js_sort_by, turn_type};
use crate::jsnum::{Maybe, f64_to_usize, math_max, math_min, math_round, or_default, usize_to_f64};
use crate::junction::{ArcFlow, ConvInfo, JUNCTION_LABEL, cap_src, conv_text, eff_cap, junction_result};
use crate::project::{OrderedMap, Project, Scenario, Settings};
use crate::results::{ArcRes, TurnRow, ClassFlows, DevRes, DevZone, DevGateway, Select, OdFlow, SelectTurn, OdPaths, PathFlow, Res, NodeRes, KpiWorst, RouteRes, GrowthInfo, JunctionInfo, ScenarioInfo, model_meta, Convergence, Kpi};
use crate::standards::{JunctionSet, LOS_VC, STANDARDS, ef, growth, growth_basis, junction_set, los_of, year_of};
use crate::util::{r1, r3};
use crate::routes::bus_freq;
use std::collections::HashMap;

/// The assignment's `ZCLASS`: the development classes in the order the class flows list them.
const ZCLASS: [&str; 3] = ["existing", "committed", "development"];

/// `SELECT_LABEL`: the caveat printed with select-link results.
pub(super) const SELECT_LABEL: &str = "Frank-Wolfe path splits are not unique; select-link results are indicative.";

struct Totals {
    vkt: f64,
    vht: f64,
    dly: f64,
    co2: f64,
    over: f64,
    max_vc: f64,
    lane_km: f64,
    charge_h: f64,
}

// v[i] of a per-arc or per-turn array. The run sizes every such array to the graph, so an index is
// always in range; past the end reads as NaN, as an undefined element does in the JavaScript.
fn at(v: &[f64], i: usize) -> f64 {
    v.get(i).copied().unwrap_or(f64::NAN)
}

// run.trk[k].x[i]: a tracker's flow (NaN past the end, as at does)
fn trk_at(run: &Run, k: usize, i: usize) -> f64 {
    run.trk.get(k).map_or(f64::NAN, |tr| at(&tr.x, i))
}

/// Link results by arc key, the per-arc flows junctionResult reads, and the network totals before turn penalties.
fn arc_results(run: &Run) -> (OrderedMap<ArcRes>, Vec<ArcFlow>, Totals) {
    let (g, s, pf) = (&run.g, run.s, run.pf);
    let mut arcs_res: OrderedMap<ArcRes> = OrderedMap::new();
    let mut flows = vec![ArcFlow::default(); run.m];
    let mut tk = Totals {
        vkt: 0.0,
        vht: 0.0,
        dly: 0.0,
        co2: 0.0,
        over: 0.0,
        max_vc: 0.0,
        lane_km: 0.0,
        charge_h: 0.0,
    };
    // run.m is the graph's arc count, so this visits every arc in order
    for (ai, arc) in g.arcs.iter().enumerate().take(run.m) {
        if arc.conn() {
            continue;
        }
        // t is time only: a charge (apen) steers routes but is money, not time
        let tot = at(&run.xt, ai);
        let veh = at(&run.x, ai) / pf;
        let cap = eff_cap(arc, &run.perf);
        let apen = at(&run.apen, ai);
        let t = at(&run.cost, ai) - apen;
        let run_t = arc.ff * (1.0 + s.bpr_alpha.num() * crate::jsmath::pow(tot / arc.cap_mid, s.bpr_beta.num()));
        let vc = tot / cap;
        let spd = arc.len / t * 3.6;
        // What sets the capacity: the mid-block, or the junction at the arc's downstream end (results-trust (c))
        let node_cap = match arc.ctrl {
            crate::graph::Ctrl::Sig => {
                f64::from(arc.gl) * s.sat_flow.num() * arc.g_c.unwrap_or(f64::NAN)
            }
            crate::graph::Ctrl::Minor | crate::graph::Ctrl::Rbt => or_default(arc.cap_n.unwrap_or(f64::NAN), arc.cap_mid),
            crate::graph::Ctrl::Free => f64::INFINITY,
        };
        let cap_ctl = if node_cap < arc.cap_mid { arc.node.and_then(|node| g.ctl_of(node)) } else { None };
        let nd = r1(t - run_t) + 0.0;
        arcs_res.insert(
            arc.key.clone(),
            ArcRes {
                v: r1(veh),
                pcu: r1(tot),
                cap: math_round(cap),
                vc: r3(vc),
                t: r1(t),
                ff: r1(arc.ff),
                spd: r1(spd),
                nd,
                len: math_round(arc.len),
                los: los_of(vc, &LOS_VC).to_string(),
                bus: r1(at(&run.pre, ai) / run.bus_pcu),
                cap_mid: math_round(arc.cap_mid),
                cap_src: match cap_ctl {
                    Some(ctl) => cap_src(&ctl.kind).unwrap_or("junction").to_string(),
                    None => "mid-block".to_string(),
                },
                cap_node: cap_ctl.and_then(|ctl| g.ids.get(ctl.n).cloned()),
                toll: (apen > 0.0).then(|| r1(apen)),
            },
        );
        if let Some(slot) = flows.get_mut(ai) {
            *slot = ArcFlow { v: r1(veh), nd };
        }
        if apen > 0.0 {
            tk.charge_h += veh * apen / 3600.0;
        }
        tk.vkt += veh * arc.len / 1000.0;
        tk.vht += veh * t / 3600.0;
        tk.dly += veh * (t - arc.ff) / 3600.0;
        tk.co2 += veh * arc.len / 1000.0 * ef(spd, s) / 1e6;
        tk.lane_km += f64::from(arc.lanes) * arc.len / 1000.0;
        if vc > 1.0 {
            tk.over += 1.0;
        }
        if vc > tk.max_vc {
            tk.max_vc = vc;
        }
    }
    (arcs_res, flows, tk)
}

/// Turning movements with traffic at every junction: nodes with 3 or more links, or a road node where a zone connects and makes a third arm (a development access).
///
/// Zone movements are included (keys c:...), so turning totals add up to the approach volumes;
/// rows under 0.05 veh/h are left out to keep saves small.
fn turn_results(run: &Run) -> OrderedMap<Vec<TurnRow>> {
    let (g, pf) = (&run.g, run.pf);
    let mut turns: OrderedMap<Vec<TurnRow>> = OrderedMap::new();
    let mut conn_in: HashMap<usize, Vec<usize>> = HashMap::new();
    for a in &g.arcs {
        if a.conn() && g.is_c().get(a.to) == Some(&0) {
            conn_in.entry(a.to).or_default().push(a.i);
        }
    }
    let none: Vec<usize> = Vec::new();
    let off = g.t_off();
    for n in 0..g.n {
        let ci = conn_in.get(&n).unwrap_or(&none);
        // Node arrays hold one entry per node, so these are always found
        let is_c = g.is_c().get(n).copied().unwrap_or(0);
        let deg = g.deg.get(n).copied().unwrap_or(0);
        if is_c != 0 || (deg < 3 && (ci.is_empty() || deg < 2)) {
            continue;
        }
        let into = g.into.get(n).map_or(&[][..], Vec::as_slice);
        let mut list: Vec<TurnRow> = Vec::new();
        for &a in into.iter().chain(ci.iter()) {
            // tOff has one entry per arc and one more, never negative
            let (Some(&t0), Some(&t1)) = (off.get(a), a.checked_add(1).and_then(|a1| off.get(a1))) else {
                continue;
            };
            let (t0, t1) = (usize::try_from(t0).unwrap_or(0), usize::try_from(t1).unwrap_or(0));
            for t in t0..t1 {
                let v = r1(at(&run.x_t, t) / pf);
                if !(v > 0.0) || g.tpen().get(t) == Some(&f64::INFINITY) {
                    continue;
                }
                let Some(b) = g.t_to().get(t).and_then(|&b| usize::try_from(b).ok()) else {
                    continue;
                };
                let (Some(arc_a), Some(arc_b)) = (g.arcs.get(a), g.arcs.get(b)) else {
                    continue;
                };
                let dv = r1(trk_at(run, T_TD, t) / pf);
                let mv = if arc_a.conn() || arc_b.conn() {
                    turn_type(arc_a.in_b, arc_b.out_b)
                } else {
                    g.movement(a, b).unwrap_or("")
                };
                list.push(TurnRow {
                    from: arc_a.key.clone(),
                    to: arc_b.key.clone(),
                    mv: mv.to_string(),
                    v,
                    pcu: r1(at(&run.x_t, t)),
                    dev: (dv > 0.0).then(|| math_min(v, dv)),
                });
            }
        }
        if !list.is_empty()
            && let Some(id) = g.ids.get(n)
        {
            turns.insert(id.clone(), list);
        }
    }
    turns
}

/// Development tracking (dev-tracking): link flows by class where any is committed or development (other links are all existing traffic), each development zone's trips, and where development trips cross the boundary (gateway in = arriving through it, out = leaving through it). veh/h.
fn class_results(run: &Run) -> (ClassFlows, DevRes) {
    let (g, pf) = (&run.g, run.pf);
    let mut arcs: OrderedMap<Vec<f64>> = OrderedMap::new();
    for (a, arc) in g.arcs.iter().enumerate().take(run.m) {
        if arc.conn() {
            continue;
        }
        let tot = at(&run.x, a) / pf;
        let c1 = trk_at(run, T_C1, a) / pf;
        let c2 = trk_at(run, T_C2, a) / pf;
        if tot > 0.05 && (c1 > 0.05 || c2 > 0.05) {
            arcs.insert(arc.key.clone(), vec![r1(math_max(0.0, tot - c1 - c2)), r1(c1), r1(c2)]);
        }
    }
    let cls = ClassFlows {
        names: ZCLASS.iter().map(std::string::ToString::to_string).collect(),
        trips: run.cls_trips.iter().map(|&t| r1(t)).collect(),
        arcs,
    };
    let mut dev = DevRes {
        trips: r1(run.cls_trips[2]),
        zones: OrderedMap::new(),
        gateways: OrderedMap::new(),
        via_gateways: 0.0,
    };
    // The run lists zones by their index in g.zones
    for &(i, o, d) in &run.dev_zone {
        if let Some(zone) = g.zones.get(i) {
            dev.zones.insert(zone.id.clone(), DevZone { o: r1(o), d: r1(d) });
        }
    }
    for &(gi, inn, out) in &run.dev_gw {
        if let Some(zone) = g.zones.get(gi) {
            dev.gateways.insert(zone.id.clone(), DevGateway { r#in: r1(inn), out: r1(out) });
        }
        dev.via_gateways += inn + out;
    }
    dev.via_gateways = r1(dev.via_gateways);
    (cls, dev)
}

/// `res.select`: the select-link, select-turn or select-zone analysis the run tracked, or why it could not run.
fn select_result(run: &Run, turns: &OrderedMap<Vec<TurnRow>>) -> Option<Select> {
    let sel = run.sel.as_ref()?;
    let (g, pf) = (&run.g, run.pf);
    let field = |k: &str| sel.req.get(k).filter(|v| !v.is_null());
    if !sel.on {
        let kind = if field("zone").is_some() {
            "zone"
        } else if field("arc").is_some() {
            "arc"
        } else {
            "turn"
        };
        return Some(Select {
            kind: kind.to_string(),
            key: None,
            flows: None,
            turns: None,
            ods: None,
            od_pairs: None,
            tot: None,
            label: None,
            error: Some("The selected zone, link or turn is not in this network (or the turn is banned).".to_string()),
        });
    }
    // The run sets up all three trackers whenever the selection is on
    let tracker = |ix: Option<usize>| ix.and_then(|k| run.trk.get(k));
    let (Some(sel_link), Some(sel_turn), Some(sel_pair)) = (tracker(run.ix.s), tracker(run.ix.ts), tracker(run.ix.od)) else {
        return None;
    };
    let mut flows: OrderedMap<f64> = OrderedMap::new();
    for (a, arc) in g.arcs.iter().enumerate().take(run.m) {
        let q = at(&sel_link.x, a) / pf;
        if !arc.conn() && q > 0.05 {
            flows.insert(arc.key.clone(), r1(q));
        }
    }
    // tot counts every OD pair; the list keeps pairs over 0.05 veh/h, largest first, at most 2,000
    let (mut tot, mut n_sel) = (0.0f64, 0.0f64);
    let mut ods: Vec<OdFlow> = Vec::new();
    for (i, list) in run.od_list.iter().enumerate().take(run.zn) {
        for od in list {
            let q = at(&sel_pair.x, od.pair) / pf;
            if q > 1e-6 {
                tot += q;
                n_sel += 1.0;
            }
            if q > 0.05
                && let (Some(orig), Some(dest)) = (g.zones.get(i), g.zones.get(od.j))
            {
                ods.push(OdFlow {
                    o: orig.id.clone(),
                    d: dest.id.clone(),
                    q: r1(q),
                });
            }
        }
    }
    let mut s_turns: OrderedMap<Vec<SelectTurn>> = OrderedMap::new();
    for (id, list) in turns.iter() {
        for r in list {
            let t = match (g.key_idx.get(&r.from), g.key_idx.get(&r.to)) {
                (Some(&a), Some(&b)) => usize::try_from(g.turn_idx(a, b)).ok(),
                _ => None,
            };
            let v = t.map_or(0.0, |t| at(&sel_turn.x, t) / pf);
            if v > 0.05 {
                let row = SelectTurn {
                    from: r.from.clone(),
                    to: r.to.clone(),
                    mv: r.mv.clone(),
                    v: r1(v),
                };
                if let Some(rows) = s_turns.get_mut(id) {
                    rows.push(row);
                } else {
                    s_turns.insert(id, vec![row]);
                }
            }
        }
    }
    js_sort_by(&mut ods, |p, q| q.q - p.q);
    ods.truncate(2000);
    let kind = if sel.zone >= 0 {
        "zone"
    } else if sel.arc >= 0 {
        "arc"
    } else {
        "turn"
    };
    // key: sel.zone ?? sel.arc ?? sel.turn
    let key = ["zone", "arc", "turn"].iter().find_map(|k| field(k)).cloned();
    Some(Select {
        kind: kind.to_string(),
        key,
        flows: Some(flows),
        turns: Some(s_turns),
        ods: Some(ods),
        od_pairs: Some(n_sel),
        tot: Some(r1(tot)),
        label: Some(SELECT_LABEL.to_string()),
        error: None,
    })
}

/// The paths as `res.paths` lists them: per OD pair, the paths carrying over 0.01 pcu/h.
fn path_list(set: &PathSet, zn: usize) -> Vec<OdPaths> {
    // An OD pair is i * zn + j; there are paths only when there are zones, so zn is never 0
    set.iter()
        .map(|(od, m)| OdPaths {
            i: od.checked_div(zn).unwrap_or(0),
            j: od.checked_rem(zn).unwrap_or(0),
            list: m
                .iter()
                .filter(|(_, w)| **w > 0.01)
                .map(|(arcs, w)| PathFlow {
                    arcs: arcs.to_vec(),
                    w: *w,
                })
                .collect(),
        })
        .collect()
}

/// Assignment noise: 95th percentile of the change in arc flow (veh/h) over the last five updates.
fn noise_95(run: &Run) -> Option<f64> {
    if run.hist.len() != 6 {
        return None;
    }
    let (Some(last), Some(first)) = (run.hist.get(5), run.hist.front()) else {
        return None;
    };
    let mut diffs: Vec<f64> = Vec::new();
    for (a, arc) in run.g.arcs.iter().enumerate().take(run.m) {
        if !arc.conn() {
            diffs.push((at(last, a) - at(first, a)).abs() / run.pf);
        }
    }
    js_sort_by(&mut diffs, |da, db| da - db);
    if diffs.is_empty() {
        return None;
    }
    // diffs is not empty, so its last index is len - 1
    let top = usize_to_f64(diffs.len().saturating_sub(1));
    let pos = math_min(top, math_round(0.95 * top));
    diffs.get(f64_to_usize(pos)).map(|&dv| r1(dv))
}

/// Hours spent at turn penalties: they are time spent at junctions, so they count in VHT and delay as they do in routing.
fn turn_pen_hours(run: &Run) -> f64 {
    let mut tpen_h = 0.0f64;
    for (&pc, &xt) in run.pc.iter().zip(&run.x_t).take(run.nt) {
        if crate::jsnum::truthy_number(pc) && xt > 0.0 {
            tpen_h += xt / run.pf * pc / 3600.0;
        }
    }
    tpen_h
}

/// Junction results by node id, and the junction with the most delay.
fn node_results(run: &Run, s: &Settings, jset: &JunctionSet, flows: &[ArcFlow]) -> (OrderedMap<NodeRes>, Option<KpiWorst>) {
    let g = &run.g;
    let mut nodes_res: OrderedMap<NodeRes> = OrderedMap::new();
    let mut worst: Option<KpiWorst> = None;
    for ctl in &g.ctl {
        let jr = junction_result(g, ctl, &run.xt, flows, s, jset, run.pf);
        // A junction result has every field a NodeRes reads, and a controller sits on a graph node
        let Ok(nr) = serde_json::from_value::<NodeRes>(jr.to_value()) else {
            continue;
        };
        let Some(id) = g.ids.get(ctl.n).cloned() else {
            continue;
        };
        if worst.as_ref().is_none_or(|wk| nr.delay > wk.delay) {
            worst = Some(KpiWorst {
                id: id.clone(),
                delay: nr.delay,
                los: nr.los.clone(),
            });
        }
        nodes_res.insert(id, nr);
    }
    (nodes_res, worst)
}

/// Bus route results: length, time (running, turn penalties and 20 s a stop every 400 m), speed and frequency.
fn route_results(run: &Run) -> OrderedMap<RouteRes> {
    let g = &run.g;
    let mut routes_res: OrderedMap<RouteRes> = OrderedMap::new();
    let none: Vec<usize> = Vec::new();
    for (idx, route) in run.routes.iter().enumerate() {
        let (mut t, mut len) = (0.0f64, 0.0f64);
        // One resolved arc list per route, of arcs in the graph
        let ra = run.route_arcs.get(idx).unwrap_or(&none);
        for (q, &ai) in ra.iter().enumerate() {
            let Some(arc) = g.arcs.get(ai) else {
                continue;
            };
            len += arc.len;
            t += if arc.bus() { arc.ff * 1.1 } else { at(&run.cost, ai) - at(&run.apen, ai) };
            if let Some(&prev) = q.checked_sub(1).and_then(|pq| ra.get(pq))
                && let Ok(ti) = usize::try_from(g.turn_idx(prev, ai))
                && let Some(&pen) = g.tpen().get(ti)
                && pen.is_finite()
            {
                t += pen;
            }
        }
        t += len / 400.0 * 20.0;
        routes_res.insert(
            route.id.clone(),
            RouteRes {
                len: math_round(len),
                t: math_round(t),
                spd: (len > 0.0).then(|| r1(len / t * 3.6)),
                freq: bus_freq(route, Some(&run.period)),
            },
        );
    }
    routes_res
}

/// `res.junction`: the junction method and its labels.
fn junction_info(jset: &JunctionSet) -> JunctionInfo {
    JunctionInfo {
        method: jset.method.to_string(),
        label: jset.los().label.to_string(),
        pff: jset.pff,
        t: jset.t,
        queue_space: jset.queue_space,
        f95: jset.f95,
        honest: JUNCTION_LABEL.to_string(),
        cap_method: jset.cap_method.as_str().to_string(),
        cap_label: (jset.cap_method == crate::standards::CapMethod::Movement).then(|| STANDARDS.gaps.label.to_string()),
        rbt_label: (jset.cap_method == crate::standards::CapMethod::Movement).then(|| STANDARDS.rbt_lanes.label.to_string()),
    }
}

/// `res.kpi`: the network totals.
fn kpi_of(run: &Run, tk: &Totals, worst: Option<KpiWorst>) -> Kpi {
    Kpi {
        trips: run.trips,
        vkt: tk.vkt,
        vht: tk.vht,
        dly: tk.dly,
        spd: if tk.vht > 0.0 { tk.vkt / tk.vht } else { 0.0 },
        co2: tk.co2,
        over: tk.over,
        max_vc: tk.max_vc,
        unserved: run.unserved / run.pf,
        worst,
        lane_km: tk.lane_km,
        charge_h: run.has_pen.then_some(tk.charge_h),
    }
}

/// The convergence note convText writes for the run.
fn conv_note_of(run: &Run, converged: bool) -> String {
    let cv = run.cv;
    conv_text(&ConvInfo {
        converged,
        hit_max: run.hit_max,
        gap: run.gap,
        it: f64::from(run.it),
        gap_tol: cv.gap_tol,
        stable: cv.stable,
        share: cv.share,
        consec: cv.consec,
    })
}

/// `res.scenario`: the scenario's id, role, build-out and demand factor.
fn scenario_info(sc: &Scenario, df: f64) -> ScenarioInfo {
    ScenarioInfo {
        id: sc.id.clone(),
        role: sc.role.as_str().filter(|role| !role.is_empty()).map(String::from),
        buildout: if sc.buildout.is_nullish() { Maybe::Null } else { sc.buildout.clone() },
        demand_factor: df,
    }
}

/// The result object of the run, from its final flows (what `Engine.assign` returns).
pub(super) fn build_res(
    p: &Project,
    sc: &Scenario,
    s: &Settings,
    dem: &Demand,
    df: f64,
    run: &Run,
    ms: f64,
) -> Res {
    let g = &run.g;
    let cv = run.cv;
    let converged = !run.hit_max && run.gap <= cv.gap_tol;
    let noise95 = noise_95(run);
    let conv_note = conv_note_of(run, converged);
    let (arcs_res, flows, mut tk) = arc_results(run);
    let tpen_h = turn_pen_hours(run);
    tk.vht += tpen_h;
    tk.dly += tpen_h;
    let jn = junction_set(s);
    let (nodes_res, worst) = node_results(run, s, &jn, &flows);
    let turns = turn_results(run);
    let (cls, dev) = class_results(run);
    let select = select_result(run, &turns);
    let tracker = |ix: Option<usize>| ix.and_then(|k| run.trk.get(k));
    let sm = match (tracker(run.ix.m), tracker(run.ix.md)) {
        (Some(traced), Some(traced_dev)) => Some((traced.x.as_slice(), traced_dev.x.as_slice())),
        _ => None,
    };
    let site_out = site_results(sc, g, &run.x_t, run.pf, &nodes_res, sm);
    let routes_res = route_results(run);
    let zn = run.zn;
    let (sites, site_turns, basis) = match site_out {
        Some(out) => (Some(out.sites), Some(out.site_turns), Some(if out.traced { "paths" } else { "shares" }.to_string())),
        None => (None, None, None),
    };
    let ts = crate::util::now_ms();
    Res {
        period: run.period.clone(),
        year: year_of(Some(sc), s),
        ts,
        it: f64::from(run.it),
        gap: run.gap,
        ms,
        stale: false,
        converged,
        hit_max: run.hit_max,
        gap_tol: cv.gap_tol,
        max_iter: cv.max_iter,
        stable_rule: cv.stable,
        conv_note: conv_note.clone(),
        conv: run.conv.clone(),
        noise95,
        growth: GrowthInfo {
            factor: growth(s, Some(sc)),
            basis: growth_basis(s),
        },
        junction: junction_info(&jn),
        arcs: arcs_res,
        nodes: nodes_res,
        turns,
        routes: routes_res,
        cls,
        dev,
        select,
        sites,
        site_turns,
        site_turns_basis: basis,
        scenario: scenario_info(sc, df),
        meta: model_meta(
            p,
            Some(sc),
            &run.period,
            s,
            Convergence {
                converged,
                hit_max: run.hit_max,
                gap: run.gap,
                gap_tol: cv.gap_tol,
                it: f64::from(run.it),
                note: conv_note,
            },
        ),
        kpi: kpi_of(run, &tk, worst),
        bus_pcu: (!run.routes.is_empty()).then_some(run.bus_pcu),
        zone_ids: g.zones.iter().map(|zone| zone.id.clone()).collect(),
        o: dem.o.clone(),
        d: dem.d.clone(),
        o0: dem.o0.clone(),
        d0: dem.d0.clone(),
        t: (zn <= 150).then(|| dem.t.iter().map(|&val| r1(val)).collect()),
        note: dem.note.clone(),
        manual: dem.manual,
        arc_keys: Some(g.arcs.iter().map(|arc| arc.key.clone()).collect()),
        paths: match &run.paths {
            Some(set) => Maybe::Set(path_list(set, zn)),
            None => Maybe::Null,
        },
    }
}
