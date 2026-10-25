//! buildGraph: arcs, zone connectors, node rows, degrees, turns, site slots and controllers.

use super::controls::setup_controls;
use super::sites::{site_index, site_move_index};
use super::turns::{TurnCtx, build_turns, turn_set};
use super::zones::{Cand, connector_targets, main_component};
use super::{Arc, Ctrl, Graph};
use crate::jsnum::{math_max, math_min, value_to_number};
use crate::project::{Link, OrderedMap, RoadClass, Scenario, Settings};
use crate::standards::cap_method_of;
use crate::util::{along, bearing, cum_len, dist, flat};
use engine_sim::{EdgeGraph, Network};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// `speedOf(L, C, period)`: the link speed (at least 5 km/h), capped in the period a variable speed limit covers (`varSpeed[period] === true`).
pub fn speed_of(l: &Link, c: &RoadClass, period: Option<&str>) -> f64 {
    let mut spd = math_max(5.0, l.speed.or(c.speed));
    if let Some(p) = period.filter(|p| !p.is_empty())
        && let Some(Value::Object(vs)) = l.var_speed.get()
        && vs.get(p) == Some(&Value::Bool(true))
    {
        let v = vs.get("v").map_or(f64::NAN, value_to_number);
        if v > 0.0 {
            spd = math_max(5.0, math_min(spd, v));
        }
    }
    spd
}

/// `tollOf(L, period)`: the time-of-use charge (s of generalised cost): L.toll, or L.toll[period] when it is an object; 0 unless positive and finite.
pub fn toll_of(l: &Link, period: Option<&str>) -> f64 {
    let t = match l.toll.get() {
        None | Some(Value::Null) => return 0.0,
        Some(Value::String(s)) if s.is_empty() => return 0.0,
        Some(t) => t,
    };
    let v = match t {
        Value::Object(o) => match period.filter(|p| !p.is_empty()) {
            Some(p) => o.get(p).map_or(f64::NAN, value_to_number),
            None => 0.0,
        },
        // an array's [period] is undefined
        Value::Array(_) => f64::NAN,
        other => value_to_number(other),
    };
    if v > 0.0 && v.is_finite() { v } else { 0.0 }
}

/// `buildGraph(sc, S, period)`: the scenario's network graph; period (optional) applies its variable speed limits and charges.
pub fn build_graph(sc: &Scenario, s: &Settings, period: Option<&str>) -> Graph {
    let mut ids: Vec<String> = sc.nodes.keys().map(str::to_string).collect();
    let n0 = ids.len();
    let mut idx: HashMap<String, usize> = HashMap::new();
    for (i, id) in ids.iter().enumerate() {
        idx.insert(id.clone(), i);
    }
    let mut ll: Vec<[f64; 2]> = sc.nodes.values().map(|n| n.ll()).collect();
    let mut arcs: Vec<Arc> = Vec::new();
    let mut ok_node = vec![0u8; n0];
    let (site_of0, sites) = site_index(sc, &idx);
    for l in sc.links.values() {
        if l.status.is("closed") || l.a == l.b {
            continue;
        }
        let (Some(&ia), Some(&ib)) = (idx.get(&l.a), idx.get(&l.b)) else {
            continue;
        };
        let c = l.class();
        let g = sc.geom(l);
        let cum = cum_len(&g);
        let len = math_max(cum[cum.len() - 1], 1.0);
        let spd = speed_of(l, c, period);
        let v = spd / 3.6;
        let cap_lane = l.cap_per_lane.or(c.cap);
        let toll = toll_of(l, period);
        if c.rank < 5.0 {
            ok_node[ia] = 1;
            ok_node[ib] = 1;
        }
        let two = l.lanes_ab.int32() > 0 && l.lanes_ba.int32() > 0;
        let sa = &site_of0[ia];
        let internal = sa.is_some() && *sa == site_of0[ib];
        for dir in [1i8, -1] {
            let lanes = if dir > 0 { l.lanes_ab.int32() } else { l.lanes_ba.int32() };
            if lanes <= 0 {
                continue;
            }
            let bus = l.bus_lane.truthy() && lanes >= 2;
            let gl = if bus { lanes - 1 } else { lanes };
            let pts: Vec<[f64; 2]> = if dir > 0 {
                g.clone()
            } else {
                g.iter().rev().copied().collect()
            };
            let cumd = if dir > 0 { cum.clone() } else { cum_len(&pts) };
            let e = math_min(20.0, len / 2.0);
            let in_b = bearing(&along(&pts, &cumd, len - e).p, &pts[pts.len() - 1]);
            let out_b = bearing(&pts[0], &along(&pts, &cumd, e).p);
            arcs.push(Arc {
                i: 0,
                key: format!("{}{}", l.id, if dir > 0 { ":ab" } else { ":ba" }),
                link: l.id.clone(),
                dir,
                conn: false,
                from: if dir > 0 { ia } else { ib },
                to: if dir > 0 { ib } else { ia },
                len,
                lanes,
                gl,
                bus,
                two,
                ff: len / v,
                cap_mid: gl as f64 * cap_lane,
                cls: l.cls.as_str().map(str::to_string),
                rank: c.rank,
                speed: spd,
                lw: c.lw,
                ring: l.ring.truthy(),
                name: l.name.str_or("").to_string(),
                pts,
                cum: cumd,
                in_b,
                out_b,
                ctrl: Ctrl::Free,
                internal,
                toll,
                zone: None,
                node: None,
                phase: None,
                c: None,
                g_c: None,
                major: false,
                stop: false,
                conf: None,
                ring_in: None,
                ring_out: None,
                cap_n: None,
                mj: None,
                mv_res: None,
                lm_src: None,
            });
        }
    }
    let zones: Vec<crate::project::Zone> = sc
        .zones
        .values()
        .filter(|z| z.lat.num().is_finite() && z.lng.num().is_finite())
        .cloned()
        .collect();
    let lat0 = if n0 > 0 {
        ll.iter().fold(0.0, |s, p| s + p[0]) / n0 as f64
    } else {
        -36.85
    };
    let f = flat(lat0);
    // Zones only connect to the main connected network, never to an unconnected fragment
    let edges: Vec<(usize, usize)> = arcs.iter().map(|a| (a.from, a.to)).collect();
    let main = main_component(n0, &edges);
    let mut cand = Vec::new();
    for i in 0..n0 {
        if ok_node[i] != 0 && main[i] != 0 && site_of0[i].is_none() {
            cand.push(Cand {
                id: ids[i].clone(),
                x: f.x(&ll[i]),
                y: f.y(&ll[i]),
            });
        }
    }
    let mut zone_node = Vec::new();
    let mut conns: OrderedMap<Vec<String>> = OrderedMap::new();
    let connector_speed = s.connector_speed.num();
    for z in &zones {
        let zi = ids.len();
        let zll = z.ll();
        ids.push(format!("z:{}", z.id));
        ll.push(zll);
        idx.insert(format!("z:{}", z.id), zi);
        zone_node.push(zi);
        let tg = connector_targets(sc, z, &cand, &f, s);
        conns.insert(z.id.clone(), tg.clone());
        for t in &tg {
            let Some(&ti) = idx.get(t) else {
                continue;
            };
            let d = math_max(10.0, dist(&zll, &ll[ti]));
            let ff = d / (connector_speed / 3.6);
            let bo = bearing(&zll, &ll[ti]);
            let bd = bearing(&ll[ti], &zll);
            for (suffix, from, to, pts, b) in [
                ("o", zi, ti, vec![zll, ll[ti]], bo),
                ("d", ti, zi, vec![ll[ti], zll], bd),
            ] {
                arcs.push(Arc {
                    i: 0,
                    key: format!("c:{}:{}:{}", z.id, t, suffix),
                    link: String::new(),
                    dir: 0,
                    conn: true,
                    from,
                    to,
                    len: d,
                    lanes: 1,
                    gl: 1,
                    bus: false,
                    two: false,
                    ff,
                    cap_mid: f64::INFINITY,
                    cls: None,
                    rank: 0.0,
                    speed: f64::NAN,
                    lw: f64::NAN,
                    ring: false,
                    name: String::new(),
                    pts,
                    cum: Vec::new(),
                    in_b: b,
                    out_b: b,
                    ctrl: Ctrl::Free,
                    internal: false,
                    toll: 0.0,
                    zone: Some(z.id.clone()),
                    node: None,
                    phase: None,
                    c: None,
                    g_c: None,
                    major: false,
                    stop: false,
                    conf: None,
                    ring_in: None,
                    ring_out: None,
                    cap_n: None,
                    mj: None,
                    mv_res: None,
                    lm_src: None,
                });
            }
        }
    }
    let n = ids.len();
    let m = arcs.len();
    let from: Vec<usize> = arcs.iter().map(|a| a.from).collect();
    let to: Vec<usize> = arcs.iter().map(|a| a.to).collect();
    let net = Network::new(n, &from, &to);
    let mut ffc = vec![0.0f64; m];
    let mut into: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut out_r: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, a) in arcs.iter_mut().enumerate() {
        a.i = i;
        ffc[i] = a.ff;
        if !a.conn {
            into[a.to].push(i);
            out_r[a.from].push(i);
        }
    }
    let mut is_c = vec![0u8; n];
    for &i in &zone_node {
        is_c[i] = 1;
    }
    let mut key_idx = HashMap::new();
    for (i, a) in arcs.iter().enumerate() {
        key_idx.insert(a.key.clone(), i);
    }
    // Links meeting at each node (connectors not counted): 1 = dead end, 3 or more = junction
    let mut deg = vec![0i32; n];
    for v in 0..n {
        if is_c[v] != 0 {
            continue;
        }
        let mut set: HashSet<&str> = HashSet::new();
        for &i in &into[v] {
            set.insert(&arcs[i].link);
        }
        for &i in &out_r[v] {
            set.insert(&arcs[i].link);
        }
        deg[v] = set.len() as i32;
    }
    let nodes: Vec<Option<crate::project::Node>> =
        ids.iter().map(|id| sc.nodes.get(id).cloned()).collect();
    let turn_s = turn_set(s);
    let (turns, tmv) = build_turns(
        &net,
        &TurnCtx {
            arcs: &arcs,
            deg: &deg,
            nodes: &nodes,
            turn_s: &turn_s,
        },
    );
    let n_t = turns.len();
    let mut site_of = site_of0;
    site_of.resize(n, None);
    let mut g = Graph {
        ids,
        idx,
        ll,
        arcs,
        n,
        m,
        search: EdgeGraph::new(net, turns, is_c),
        ffc,
        into,
        out_r,
        zones,
        zone_node,
        key_idx,
        conns,
        ctl: Vec::new(),
        ctl_at: vec![-1; n],
        nodes,
        deg,
        turn_s,
        site_of,
        sites,
        cap_method: cap_method_of(s),
        settings: s.clone(),
        period: period.map(str::to_string),
        tmv,
        n_t,
        sm: None,
        sm_x: None,
        mv_detail: true,
    };
    g.sm = site_move_index(&g);
    setup_controls(sc, &mut g, s);
    g
}
