//! The result object of an assignment (the second half of engine.js `assign`): link results and
//! totals, junction results, turning movements, development tracking, the select-link analysis,
//! bus routes and the junction sites.

use super::paths::PathSet;
use super::run::{Run, T_C1, T_C2, T_TD};
use super::sites::site_results;
use crate::demand::Demand;
use crate::graph::{js_sort_by, turn_type};
use crate::jsnum::{Maybe, math_max, math_min, math_round, or_default};
use crate::junction::{ArcFlow, ConvInfo, JUNCTION_LABEL, cap_src, conv_text, eff_cap, junction_result};
use crate::project::{OrderedMap, Project, Scenario, Settings};
use crate::results::*;
use crate::standards::{LOS_VC, STANDARDS, ef, growth, growth_basis, junction_set, los_of, year_of};
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

/// Link results by arc key, the per-arc flows junctionResult reads, and the network totals before turn penalties.
fn arc_results(run: &Run) -> (OrderedMap<ArcRes>, Vec<ArcFlow>, Totals) {
    let (g, s, pf) = (&run.g, run.s, run.pf);
    let mut arcs_res: OrderedMap<ArcRes> = OrderedMap::new();
    let mut flows = vec![ArcFlow::default(); run.m];
    let mut k = Totals {
        vkt: 0.0,
        vht: 0.0,
        dly: 0.0,
        co2: 0.0,
        over: 0.0,
        max_vc: 0.0,
        lane_km: 0.0,
        charge_h: 0.0,
    };
    for a in 0..run.m {
        let arc = &g.arcs[a];
        if arc.conn {
            continue;
        }
        // t is time only: a charge (apen) steers routes but is money, not time
        let tot = run.xt[a];
        let veh = run.x[a] / pf;
        let cap = eff_cap(arc, &run.perf);
        let t = run.cost[a] - run.apen[a];
        let run_t = arc.ff * (1.0 + s.bpr_alpha.num() * crate::jsmath::pow(tot / arc.cap_mid, s.bpr_beta.num()));
        let vc = tot / cap;
        let spd = arc.len / t * 3.6;
        // What sets the capacity: the mid-block, or the junction at the arc's downstream end (results-trust (c))
        let node_cap = match arc.ctrl {
            crate::graph::Ctrl::Sig => {
                arc.gl as f64 * s.sat_flow.num() * arc.g_c.unwrap_or(f64::NAN)
            }
            crate::graph::Ctrl::Minor | crate::graph::Ctrl::Rbt => or_default(arc.cap_n.unwrap_or(f64::NAN), arc.cap_mid),
            crate::graph::Ctrl::Free => f64::INFINITY,
        };
        let cap_ctl = if node_cap < arc.cap_mid { arc.node.and_then(|n| g.ctl_of(n)) } else { None };
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
                bus: r1(run.pre[a] / run.bus_pcu),
                cap_mid: math_round(arc.cap_mid),
                cap_src: match cap_ctl {
                    Some(c) => cap_src(&c.kind).unwrap_or("junction").to_string(),
                    None => "mid-block".to_string(),
                },
                cap_node: cap_ctl.map(|c| g.ids[c.n].clone()),
                toll: (run.apen[a] > 0.0).then(|| r1(run.apen[a])),
            },
        );
        flows[a] = ArcFlow { v: r1(veh), nd };
        if run.apen[a] > 0.0 {
            k.charge_h += veh * run.apen[a] / 3600.0;
        }
        k.vkt += veh * arc.len / 1000.0;
        k.vht += veh * t / 3600.0;
        k.dly += veh * (t - arc.ff) / 3600.0;
        k.co2 += veh * arc.len / 1000.0 * ef(spd, s) / 1e6;
        k.lane_km += arc.lanes as f64 * arc.len / 1000.0;
        if vc > 1.0 {
            k.over += 1.0;
        }
        if vc > k.max_vc {
            k.max_vc = vc;
        }
    }
    (arcs_res, flows, k)
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
        if a.conn && g.is_c()[a.to] == 0 {
            conn_in.entry(a.to).or_default().push(a.i);
        }
    }
    let none: Vec<usize> = Vec::new();
    for n in 0..g.n {
        let ci = conn_in.get(&n).unwrap_or(&none);
        if g.is_c()[n] != 0 || (g.deg[n] < 3 && !(!ci.is_empty() && g.deg[n] >= 2)) {
            continue;
        }
        let mut list: Vec<TurnRow> = Vec::new();
        for &a in g.into[n].iter().chain(ci.iter()) {
            for t in g.t_off()[a] as usize..g.t_off()[a + 1] as usize {
                let v = r1(run.x_t[t] / pf);
                if !(v > 0.0) || g.tpen()[t] == f64::INFINITY {
                    continue;
                }
                let b = g.t_to()[t] as usize;
                let (arc_a, arc_b) = (&g.arcs[a], &g.arcs[b]);
                let dv = r1(run.trk[T_TD].x[t] / pf);
                let mv = if arc_a.conn || arc_b.conn {
                    turn_type(arc_a.in_b, arc_b.out_b)
                } else {
                    g.movement(a, b).unwrap_or("")
                };
                list.push(TurnRow {
                    from: arc_a.key.clone(),
                    to: arc_b.key.clone(),
                    mv: mv.to_string(),
                    v,
                    pcu: r1(run.x_t[t]),
                    dev: (dv > 0.0).then(|| math_min(v, dv)),
                });
            }
        }
        if !list.is_empty() {
            turns.insert(g.ids[n].clone(), list);
        }
    }
    turns
}

/// Development tracking (dev-tracking): link flows by class where any is committed or development (other links are all existing traffic), each development zone's trips, and where development trips cross the boundary (gateway in = arriving through it, out = leaving through it). veh/h.
fn class_results(run: &Run) -> (ClassFlows, DevRes) {
    let (g, pf) = (&run.g, run.pf);
    let mut arcs: OrderedMap<Vec<f64>> = OrderedMap::new();
    for a in 0..run.m {
        let arc = &g.arcs[a];
        if arc.conn {
            continue;
        }
        let tot = run.x[a] / pf;
        let c1 = run.trk[T_C1].x[a] / pf;
        let c2 = run.trk[T_C2].x[a] / pf;
        if tot > 0.05 && (c1 > 0.05 || c2 > 0.05) {
            arcs.insert(arc.key.clone(), vec![r1(math_max(0.0, tot - c1 - c2)), r1(c1), r1(c2)]);
        }
    }
    let cls = ClassFlows {
        names: ZCLASS.iter().map(|s| s.to_string()).collect(),
        trips: run.cls_trips.iter().map(|&t| r1(t)).collect(),
        arcs,
    };
    let mut dev = DevRes {
        trips: r1(run.cls_trips[2]),
        zones: OrderedMap::new(),
        gateways: OrderedMap::new(),
        via_gateways: 0.0,
    };
    for &(i, o, d) in &run.dev_zone {
        dev.zones.insert(g.zones[i].id.clone(), DevZone { o: r1(o), d: r1(d) });
    }
    for &(gi, inn, out) in &run.dev_gw {
        dev.gateways.insert(g.zones[gi].id.clone(), DevGateway { r#in: r1(inn), out: r1(out) });
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
    let (xs, xts, xod) = (
        run.ix.s.expect("selection"),
        run.ix.ts.expect("selection"),
        run.ix.od.expect("selection"),
    );
    let mut flows: OrderedMap<f64> = OrderedMap::new();
    for a in 0..run.m {
        if !g.arcs[a].conn && run.trk[xs].x[a] / pf > 0.05 {
            flows.insert(g.arcs[a].key.clone(), r1(run.trk[xs].x[a] / pf));
        }
    }
    // tot counts every OD pair; the list keeps pairs over 0.05 veh/h, largest first, at most 2,000
    let (mut tot, mut n_sel) = (0.0f64, 0.0f64);
    let mut ods: Vec<OdFlow> = Vec::new();
    for i in 0..run.zn {
        for od in &run.od_list[i] {
            let q = run.trk[xod].x[od.od] / pf;
            if q > 1e-6 {
                tot += q;
                n_sel += 1.0;
            }
            if q > 0.05 {
                ods.push(OdFlow {
                    o: g.zones[i].id.clone(),
                    d: g.zones[od.j].id.clone(),
                    q: r1(q),
                });
            }
        }
    }
    let mut s_turns: OrderedMap<Vec<SelectTurn>> = OrderedMap::new();
    for (id, list) in turns.iter() {
        for r in list {
            let t = match (g.key_idx.get(&r.from), g.key_idx.get(&r.to)) {
                (Some(&a), Some(&b)) => g.turn_idx(a, b),
                _ => -1,
            };
            let v = if t >= 0 { run.trk[xts].x[t as usize] / pf } else { 0.0 };
            if v > 0.05 {
                if s_turns.get(id).is_none() {
                    s_turns.insert(id, Vec::new());
                }
                s_turns.get_mut(id).expect("just added").push(SelectTurn {
                    from: r.from.clone(),
                    to: r.to.clone(),
                    mv: r.mv.clone(),
                    v: r1(v),
                });
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
    set.iter()
        .map(|(od, m)| OdPaths {
            i: od / zn,
            j: od % zn,
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
    let (g, pf) = (&run.g, run.pf);
    let cv = run.cv;
    let converged = !run.hit_max && run.gap <= cv.gap_tol;
    // Assignment noise: 95th percentile of the change in arc flow (veh/h) over the last five updates
    let mut noise95: Option<f64> = None;
    if run.hist.len() == 6 {
        let mut d: Vec<f64> = Vec::new();
        for a in 0..run.m {
            if !g.arcs[a].conn {
                d.push((run.hist[5][a] - run.hist[0][a]).abs() / pf);
            }
        }
        js_sort_by(&mut d, |p, q| p - q);
        if !d.is_empty() {
            let at = math_min((d.len() - 1) as f64, math_round(0.95 * (d.len() - 1) as f64));
            noise95 = Some(r1(d[at as usize]));
        }
    }
    let conv_note = conv_text(&ConvInfo {
        converged,
        hit_max: run.hit_max,
        gap: run.gap,
        it: run.it as f64,
        gap_tol: cv.gap_tol,
        stable: cv.stable,
        share: cv.share,
        consec: cv.consec,
    });
    let (arcs_res, flows, mut k) = arc_results(run);
    // Turn penalties are time spent at junctions, so they count in VHT and delay as they do in routing
    let mut tpen_h = 0.0f64;
    for t in 0..run.nt {
        if crate::jsnum::truthy_number(run.pc[t]) && run.x_t[t] > 0.0 {
            tpen_h += run.x_t[t] / pf * run.pc[t] / 3600.0;
        }
    }
    k.vht += tpen_h;
    k.dly += tpen_h;
    let j = junction_set(s);
    let mut nodes_res: OrderedMap<NodeRes> = OrderedMap::new();
    let mut worst: Option<KpiWorst> = None;
    for ctl in &g.ctl {
        let jr = junction_result(g, ctl, &run.xt, &flows, s, &j, pf);
        let nr: NodeRes = serde_json::from_value(jr.to_value()).expect("a junction result reads as a NodeRes");
        let id = g.ids[ctl.n].clone();
        if worst.as_ref().is_none_or(|w| nr.delay > w.delay) {
            worst = Some(KpiWorst {
                id: id.clone(),
                delay: nr.delay,
                los: nr.los.clone(),
            });
        }
        nodes_res.insert(id, nr);
    }
    let turns = turn_results(run);
    let (cls, dev) = class_results(run);
    let select = select_result(run, &turns);
    let sm = match (run.ix.m, run.ix.md) {
        (Some(xm), Some(xd)) => Some((run.trk[xm].x.as_slice(), run.trk[xd].x.as_slice())),
        _ => None,
    };
    let site_out = site_results(sc, g, &run.x_t, pf, &nodes_res, sm);
    let mut routes_res: OrderedMap<RouteRes> = OrderedMap::new();
    for (idx, r) in run.routes.iter().enumerate() {
        let (mut t, mut len) = (0.0f64, 0.0f64);
        let ra = &run.route_arcs[idx];
        for q in 0..ra.len() {
            let ai = ra[q];
            let arc = &g.arcs[ai];
            len += arc.len;
            t += if arc.bus { arc.ff * 1.1 } else { run.cost[ai] - run.apen[ai] };
            if q > 0 {
                let ti = g.turn_idx(ra[q - 1], ai);
                if ti >= 0 && g.tpen()[ti as usize].is_finite() {
                    t += g.tpen()[ti as usize];
                }
            }
        }
        t += len / 400.0 * 20.0;
        routes_res.insert(
            r.id.clone(),
            RouteRes {
                len: math_round(len),
                t: math_round(t),
                spd: (len > 0.0).then(|| r1(len / t * 3.6)),
                freq: bus_freq(r, Some(&run.period)),
            },
        );
    }
    let zn = run.zn;
    let (sites, site_turns, basis) = match site_out {
        Some(o) => (Some(o.sites), Some(o.site_turns), Some(if o.traced { "paths" } else { "shares" }.to_string())),
        None => (None, None, None),
    };
    let ts = crate::util::now_ms();
    Res {
        period: run.period.clone(),
        year: year_of(Some(sc), s),
        ts,
        it: run.it as f64,
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
        junction: JunctionInfo {
            method: j.method.to_string(),
            label: j.los().label.to_string(),
            pff: j.pff,
            t: j.t,
            queue_space: j.queue_space,
            f95: j.f95,
            honest: JUNCTION_LABEL.to_string(),
            cap_method: j.cap_method.as_str().to_string(),
            cap_label: (j.cap_method == crate::standards::CapMethod::Movement).then(|| STANDARDS.gaps.label.to_string()),
            rbt_label: (j.cap_method == crate::standards::CapMethod::Movement).then(|| STANDARDS.rbt_lanes.label.to_string()),
        },
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
        scenario: ScenarioInfo {
            id: sc.id.clone(),
            role: sc.role.as_str().filter(|r| !r.is_empty()).map(String::from),
            buildout: if sc.buildout.is_nullish() { Maybe::Null } else { sc.buildout.clone() },
            demand_factor: df,
        },
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
                it: run.it as f64,
                note: conv_note,
            },
        ),
        kpi: Kpi {
            trips: run.trips,
            vkt: k.vkt,
            vht: k.vht,
            dly: k.dly,
            spd: if k.vht > 0.0 { k.vkt / k.vht } else { 0.0 },
            co2: k.co2,
            over: k.over,
            max_vc: k.max_vc,
            unserved: run.unserved / pf,
            worst,
            lane_km: k.lane_km,
            charge_h: run.has_pen.then_some(k.charge_h),
        },
        bus_pcu: (!run.routes.is_empty()).then_some(run.bus_pcu),
        zone_ids: g.zones.iter().map(|z| z.id.clone()).collect(),
        o: dem.o.clone(),
        d: dem.d.clone(),
        o0: dem.o0.clone(),
        d0: dem.d0.clone(),
        t: (zn <= 150).then(|| dem.t.iter().map(|&v| r1(v)).collect()),
        note: dem.note.clone(),
        manual: dem.manual,
        arc_keys: Some(g.arcs.iter().map(|a| a.key.clone()).collect()),
        paths: match &run.paths {
            Some(set) => Maybe::Set(path_list(set, zn)),
            None => Maybe::Null,
        },
    }
}
