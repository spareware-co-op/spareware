//! buildGraph: arcs, zone connectors, node rows, degrees, turns, site slots and controllers.

use super::controls::setup_controls;
use super::sites::{site_index, site_move_index};
use super::turns::{TurnCtx, build_turns, turn_set};
use super::zones::{Cand, connector_targets, main_component};
use super::{Arc, Ctrl, Graph, Priority};
use crate::jsnum::{math_max, math_min, usize_to_f64, value_to_number};
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

// The graph's node table while it is built: ids, [lat, lng] and the index of each id
struct NodeTable {
    ids: Vec<String>,
    ll: Vec<[f64; 2]>,
    idx: HashMap<String, usize>,
}

/// `buildGraph(sc, S, period)`: the scenario's network graph; period (optional) applies its variable speed limits and charges.
pub fn build_graph(sc: &Scenario, s: &Settings, period: Option<&str>) -> Graph {
    let ids: Vec<String> = sc.nodes.keys().map(str::to_string).collect();
    let n0 = ids.len();
    let mut idx: HashMap<String, usize> = HashMap::new();
    for (i, id) in ids.iter().enumerate() {
        idx.insert(id.clone(), i);
    }
    let ll: Vec<[f64; 2]> = sc.nodes.values().map(crate::project::Node::ll).collect();
    let mut table = NodeTable { ids, ll, idx };
    let (site_of0, sites) = site_index(sc, &table.idx);
    let (mut arcs, ok_node) = link_arcs(sc, period, &table.idx, &site_of0, n0);
    let zones = located_zones(sc);
    let lat0 = if n0 > 0 {
        table.ll.iter().fold(0.0, |sum, p| sum + p[0]) / usize_to_f64(n0)
    } else {
        -36.85
    };
    let fl = flat(lat0);
    // Zones only connect to the main connected network, never to an unconnected fragment
    let edges: Vec<(usize, usize)> = arcs.iter().map(|a| (a.from, a.to)).collect();
    let main = main_component(n0, &edges);
    let cand: Vec<Cand> = table
        .ids
        .iter()
        .zip(&table.ll)
        .zip(ok_node.iter().zip(&main).zip(&site_of0))
        .filter(|&(_, ((&ok, &in_main), site))| ok != 0 && in_main != 0 && site.is_none())
        .map(|((id, p), _)| Cand {
            id: id.clone(),
            x: fl.x(p),
            y: fl.y(p),
        })
        .collect();
    let (zone_node, conns) = connector_arcs(sc, s, &zones, &cand, &fl, &mut table, &mut arcs);
    let NodeTable { ids, ll, idx } = table;
    let n = ids.len();
    let m = arcs.len();
    let from: Vec<usize> = arcs.iter().map(|a| a.from).collect();
    let to: Vec<usize> = arcs.iter().map(|a| a.to).collect();
    let net = Network::new(n, &from, &to);
    let ffc: Vec<f64> = arcs.iter().map(|a| a.ff).collect();
    let (into, out_r) = arc_rows(&mut arcs, n);
    let mut is_c = vec![0u8; n];
    for &i in &zone_node {
        if let Some(c) = is_c.get_mut(i) {
            *c = 1;
        }
    }
    let key_idx: HashMap<String, usize> = arcs.iter().enumerate().map(|(i, a)| (a.key.clone(), i)).collect();
    let deg = degrees(&arcs, &into, &out_r, &is_c);
    let nodes: Vec<Option<crate::project::Node>> =
        ids.iter().map(|id| sc.nodes.get(id).cloned()).collect();
    let turn_settings = turn_set(s);
    let (turns, tmv) = build_turns(
        &net,
        &TurnCtx {
            arcs: &arcs,
            deg: &deg,
            nodes: &nodes,
            turn_s: &turn_settings,
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
        turn_s: turn_settings,
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

// The zones with a numeric position, which the graph connects
fn located_zones(sc: &Scenario) -> Vec<crate::project::Zone> {
    sc.zones
        .values()
        .filter(|z| z.lat.num().is_finite() && z.lng.num().is_finite())
        .cloned()
        .collect()
}

// Numbers the arcs, and lists the arcs arriving at and leaving each of the n nodes (connectors not listed)
fn arc_rows(arcs: &mut [Arc], n: usize) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let mut into: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut out_r: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, a) in arcs.iter_mut().enumerate() {
        a.i = i;
        if !a.conn() {
            if let Some(v) = into.get_mut(a.to) {
                v.push(i);
            }
            if let Some(v) = out_r.get_mut(a.from) {
                v.push(i);
            }
        }
    }
    (into, out_r)
}

// Links meeting at each node (connectors not counted): 1 = dead end, 3 or more = junction
fn degrees(arcs: &[Arc], into: &[Vec<usize>], out_r: &[Vec<usize>], is_c: &[u8]) -> Vec<i32> {
    into.iter()
        .zip(out_r)
        .zip(is_c)
        .map(|((ins, outs), &c)| {
            if c != 0 {
                return 0;
            }
            let set: HashSet<&str> = ins
                .iter()
                .chain(outs)
                .filter_map(|&i| arcs.get(i))
                .map(|a| a.link.as_str())
                .collect();
            // Bound: a node meets far fewer than i32::MAX links
            i32::try_from(set.len()).unwrap_or(i32::MAX)
        })
        .collect()
}

// buildGraph's arcs for each link direction with lanes, and which nodes a non-motorway link reaches
// (ok_node: the candidates for zone connectors)
fn link_arcs(
    sc: &Scenario,
    period: Option<&str>,
    idx: &HashMap<String, usize>,
    site_of0: &[Option<String>],
    n0: usize,
) -> (Vec<Arc>, Vec<u8>) {
    let mut arcs: Vec<Arc> = Vec::new();
    let mut ok_node = vec![0u8; n0];
    for l in sc.links.values() {
        if l.status.is("closed") || l.a == l.b {
            continue;
        }
        let (Some(&ia), Some(&ib)) = (idx.get(&l.a), idx.get(&l.b)) else {
            continue;
        };
        let class = l.class();
        let geom = sc.geom(l);
        let cum = cum_len(&geom);
        let len = math_max(cum.last().copied().unwrap_or(0.0), 1.0);
        let spd = speed_of(l, class, period);
        let v = spd / 3.6;
        let cap_lane = l.cap_per_lane.or(class.cap);
        let toll = toll_of(l, period);
        if class.rank < 5.0 {
            for i in [ia, ib] {
                if let Some(ok) = ok_node.get_mut(i) {
                    *ok = 1;
                }
            }
        }
        let two = l.lanes_ab.int32() > 0 && l.lanes_ba.int32() > 0;
        let internal = matches!((site_of0.get(ia), site_of0.get(ib)), (Some(Some(sa)), Some(Some(sb))) if sa == sb);
        for dir in [1i8, -1] {
            let lanes = if dir > 0 { l.lanes_ab.int32() } else { l.lanes_ba.int32() };
            if lanes <= 0 {
                continue;
            }
            let bus = l.bus_lane.truthy() && lanes >= 2;
            let gl = if bus { lanes.saturating_sub(1) } else { lanes };
            let pts: Vec<[f64; 2]> = if dir > 0 {
                geom.clone()
            } else {
                geom.iter().rev().copied().collect()
            };
            let cumd = if dir > 0 { cum.clone() } else { cum_len(&pts) };
            let (Some(first), Some(last)) = (pts.first(), pts.last()) else {
                continue;
            };
            let e = math_min(20.0, len / 2.0);
            let in_b = bearing(&along(&pts, &cumd, len - e).p, last);
            let out_b = bearing(first, &along(&pts, &cumd, e).p);
            arcs.push(Arc {
                i: 0,
                key: format!("{}{}", l.id, if dir > 0 { ":ab" } else { ":ba" }),
                link: l.id.clone(),
                dir,
                from: if dir > 0 { ia } else { ib },
                to: if dir > 0 { ib } else { ia },
                len,
                lanes,
                gl,
                two,
                ff: len / v,
                cap_mid: f64::from(gl) * cap_lane,
                cls: l.cls.as_str().map(str::to_string),
                rank: class.rank,
                speed: spd,
                lw: class.lw,
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
                priority: Priority::Neither,
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
    (arcs, ok_node)
}

// buildGraph's zone connectors: a node per zone (added to the table) and an arc each way to each
// of its targets. Returns each zone's node and the zones' targets.
fn connector_arcs(
    sc: &Scenario,
    s: &Settings,
    zones: &[crate::project::Zone],
    cand: &[Cand],
    fl: &crate::util::Flat,
    table: &mut NodeTable,
    arcs: &mut Vec<Arc>,
) -> (Vec<usize>, OrderedMap<Vec<String>>) {
    let mut zone_node = Vec::new();
    let mut conns: OrderedMap<Vec<String>> = OrderedMap::new();
    let connector_speed = s.connector_speed.num();
    for z in zones {
        let zi = table.ids.len();
        let zll = z.ll();
        table.ids.push(format!("z:{}", z.id));
        table.ll.push(zll);
        table.idx.insert(format!("z:{}", z.id), zi);
        zone_node.push(zi);
        let targets = connector_targets(sc, z, cand, fl, s);
        conns.insert(z.id.clone(), targets.clone());
        for t in &targets {
            let Some((ti, tll)) = table.idx.get(t).and_then(|&ti| table.ll.get(ti).map(|&p| (ti, p))) else {
                continue;
            };
            let d = math_max(10.0, dist(&zll, &tll));
            let ff = d / (connector_speed / 3.6);
            let bo = bearing(&zll, &tll);
            let bd = bearing(&tll, &zll);
            for (suffix, from, to, pts, b) in [
                ("o", zi, ti, vec![zll, tll], bo),
                ("d", ti, zi, vec![tll, zll], bd),
            ] {
                arcs.push(Arc {
                    i: 0,
                    key: format!("c:{}:{}:{}", z.id, t, suffix),
                    link: String::new(),
                    dir: 0,
                    from,
                    to,
                    len: d,
                    lanes: 1,
                    gl: 1,
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
                    priority: Priority::Neither,
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
    (zone_node, conns)
}
