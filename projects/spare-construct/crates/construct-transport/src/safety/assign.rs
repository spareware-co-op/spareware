//! Crashes assigned to links and junctions (`assignCrashes`, `crashYears`, the tallies and `norm`'s partner helpers).
//!
//! Each crash goes to its nearest existing (not proposed) link within 25 m; where several links are about equally
//! near (a junction), the link named in the crash's first location wins if it is within 5 m of the nearest. Crashes
//! within 30 m of a junction are also tallied to that junction. A junction is a cluster of nodes: a roundabout's ring
//! nodes are one junction, and nodes of three or more links within 30 m of each other that share a road name (a
//! divided-road intersection) are one junction. Clusters of motorway and ramp links only are interchange merges and
//! diverges, kept apart from junctions. One-way carriageways paired with the opposite carriageway of the same road
//! count road kilometres once.

use super::cas::{Crash, CAS_YEARS_DEFAULT};
use crate::datahub::norm;
use crate::edit::collate::locale_compare_f64;
use crate::edit::json::JsonValue;
use crate::edit::net::project;
use crate::feeds::nztime::{date_of, nz_midnight_ms, nz_parts};
use crate::graph::js_sort_by;
use crate::jsnum::{math_hypot, math_max, math_min};
use crate::project::{Link, Scenario, road_class};
use crate::util::{self, ang_diff, bearing, flat, seg_dist};
use serde::Serialize;
use std::collections::HashMap;

/// `LINK_R`: a crash belongs to a link within this distance (m).
pub const LINK_R: f64 = 25.0;
/// `NODE_R`: a crash belongs to a junction within this distance (m).
pub const NODE_R: f64 = 30.0;
/// `NAME_SLACK`: the road named in a crash wins when it is within this much further than the nearest road (m).
pub const NAME_SLACK: f64 = 5.0;
/// `CELL`: the search grid cell (m).
pub const CELL: f64 = 50.0;
/// `PAIR_M`: carriageways of one road are paired within this distance (m).
pub const PAIR_M: f64 = 60.0;
/// `SHORT_KM`: links shorter than this (km) get no risk band.
pub const SHORT_KM: f64 = 0.5;
/// `LOW_AADT`: links with less daily traffic than this get no personal risk band.
pub const LOW_AADT: f64 = 200.0;

/// Crashes by severity and kind (`tally()`): F fatal, S serious, M minor, N non-injury; DSI is F plus S; p, c and m are walking, cycling and motorcycle crashes.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Tally {
    /// All crashes.
    pub crashes: usize,
    /// Fatal.
    #[serde(rename = "F")]
    pub f: usize,
    /// Serious injury.
    #[serde(rename = "S")]
    pub s: usize,
    /// Minor injury.
    #[serde(rename = "M")]
    pub m: usize,
    /// Non-injury.
    #[serde(rename = "N")]
    pub n: usize,
    /// Deaths and serious injuries (F plus S).
    pub dsi: usize,
    /// Injury crashes (F, S and M).
    pub injury: usize,
    /// Crashes involving a person walking.
    pub p: usize,
    /// Crashes involving a person cycling.
    pub c: usize,
    /// Crashes involving a motorcycle.
    #[serde(rename = "m")]
    pub mo: usize,
}

impl Tally {
    /// `addTo(t, c)`: counts one crash.
    pub fn add(&mut self, c: &Crash) {
        self.crashes += 1;
        match c.severity.as_str() {
            "F" => self.f += 1,
            "S" => self.s += 1,
            "M" => self.m += 1,
            "N" => self.n += 1,
            _ => {}
        }
        if c.severity == "F" || c.severity == "S" {
            self.dsi += 1;
        }
        if c.severity == "F" || c.severity == "S" || c.severity == "M" {
            self.injury += 1;
        }
        if c.flags.contains('p') {
            self.p += 1;
        }
        if c.flags.contains('c') {
            self.c += 1;
        }
        if c.flags.contains('m') {
            self.mo += 1;
        }
    }
}

/// The record of the CAS download a project holds (`loaded.cas`): when it was made and the years it asked for.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CasLoad {
    /// `at`: the time of the download (ms since 1970); None when the project has none (or it is not a date).
    pub at: Option<f64>,
    /// `from`: first full year (0 or missing: five years back).
    pub from: f64,
    /// `to`: last full year.
    pub to: f64,
    /// `ytd`: the year to date.
    pub ytd: f64,
    /// `lagYear`: a full year still inside the CAS entry lag (0 or missing: none).
    pub lag_year: f64,
}

impl CasLoad {
    /// Reads `loaded.cas` as saved (an object, or anything else for none).
    pub fn from_value(v: Option<&serde_json::Value>) -> CasLoad {
        let Some(serde_json::Value::Object(o)) = v else { return CasLoad { at: None, from: f64::NAN, to: f64::NAN, ytd: f64::NAN, lag_year: f64::NAN } };
        let num = |k: &str| o.get(k).map_or(f64::NAN, crate::jsnum::value_to_number);
        let at = match o.get("at") {
            Some(a) if crate::jsnum::value_truthy(a) => date_of(&JsonValue::from_value(a)),
            _ => None,
        };
        CasLoad { at, from: num("from"), to: num("to"), ytd: num("ytd"), lag_year: num("lagYear") }
    }
}

/// The years of crash data covered (`crashYears`): the downloaded window, with the current year counted to the load date.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Years {
    /// First year (null with no data).
    pub from: Option<f64>,
    /// Last year, or the year of the load (null with no data).
    pub to: Option<f64>,
    /// Years covered, the current year only to the load date (null with no data).
    pub span: Option<f64>,
    /// The latest year is only to date.
    pub partial: bool,
    /// The span is the download window, not just the years that have crashes.
    pub window: bool,
    /// The last full year (a downloaded window only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_to: Option<f64>,
    /// The year to date (a downloaded window only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ytd: Option<f64>,
    /// A full year still inside the CAS entry lag, else null (a downloaded window only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lag_year: Option<Option<f64>>,
}

/// `crashYears(cr, cas)`: the window the crashes were downloaded for, else the span of the crash years.
///
/// With a load time the window is `from` to the load year, the load year counted to the load date (at least one
/// month of it); a project without one falls back to the years that have crashes. Years and dates are New Zealand time.
pub fn crash_years(cr: &[Crash], cas: &CasLoad) -> Years {
    let (mut y0, mut y1) = (f64::INFINITY, f64::NEG_INFINITY);
    for c in cr {
        if c.year > 0.0 {
            y0 = math_min(y0, c.year);
            y1 = math_max(y1, c.year);
        }
    }
    if let Some(at) = cas.at {
        let year = nz_parts(at).year as f64;
        let from = if cas.from > 0.0 { cas.from } else { year - CAS_YEARS_DEFAULT };
        let start = nz_midnight_ms(year, 0.0, 1.0);
        let next = nz_midnight_ms(year + 1.0, 0.0, 1.0);
        let frac = math_max(1.0 / 12.0, (at - start) / (next - start));
        return Years {
            from: Some(from),
            to: Some(year),
            span: Some(year - from + frac),
            partial: true,
            window: true,
            full_to: Some(if cas.to > 0.0 { cas.to } else { year - 1.0 }),
            ytd: Some(if cas.ytd > 0.0 { cas.ytd } else { year }),
            lag_year: Some(if cas.lag_year != 0.0 && !cas.lag_year.is_nan() { Some(cas.lag_year) } else { None }),
        };
    }
    if !y0.is_finite() {
        return Years { from: None, to: None, span: None, partial: false, window: false, full_to: None, ytd: None, lag_year: None };
    }
    Years { from: Some(y0), to: Some(y1), span: Some(y1 - y0 + 1.0), partial: false, window: false, full_to: None, ytd: None, lag_year: None }
}

/// One existing link with its length and the crashes assigned to it.
#[derive(Clone, Debug)]
pub struct LinkTally<'a> {
    /// The link.
    pub link: &'a Link,
    /// Its name as `norm` reads it.
    pub nm: String,
    /// Length (km).
    pub km: f64,
    /// Crashes assigned to it.
    pub t: Tally,
}

/// One junction (a cluster of nodes) with the crashes within 30 m of it.
#[derive(Clone, Debug)]
pub struct JunctionTally {
    /// The main node's id.
    pub id: String,
    /// The nodes of the cluster, in id order.
    pub nodes: Vec<String>,
    /// Centre, easting (flat metres).
    pub x: f64,
    /// Centre, northing (flat metres).
    pub y: f64,
    /// Roads meeting here.
    pub legs: usize,
    /// Only motorway and ramp links meet here.
    pub interchange: bool,
    /// Names of the roads meeting here.
    pub name: String,
    /// Crashes assigned to it.
    pub t: Tally,
}

/// What `assignCrashes` builds: links and junctions with their tallies, which crash went where, and the paired carriageways.
#[derive(Clone, Debug)]
pub struct Assignment<'a> {
    /// Existing links (not proposed, both ends present) in the scenario's order.
    pub lk: Vec<LinkTally<'a>>,
    /// Junctions in the order found.
    pub jn: Vec<JunctionTally>,
    /// Node id to its junction (index into `jn`).
    pub j_of_node: HashMap<String, usize>,
    /// For each crash, the index in `lk` of its link, or -1.
    pub link_of: Vec<i32>,
    /// For each crash, the index in `jn` of its junction, or -1.
    pub node_of: Vec<i32>,
    /// One-way carriageway link id to the opposite carriageway's link id.
    pub pair: HashMap<String, String>,
    /// The years of crash data covered.
    pub years: Years,
}

fn cell_key(cx: f64, cy: f64) -> i64 {
    (cx * 1e6 + cy) as i64
}

// put(grid, x0, y0, x1, y1, pad, v)
fn put<T>(grid: &mut HashMap<i64, Vec<T>>, x0: f64, y0: f64, x1: f64, y1: f64, pad: f64, v: T)
where
    T: Clone,
{
    let cx0 = ((x0 - pad) / CELL).floor();
    let cx1 = ((x1 + pad) / CELL).floor();
    let cy0 = ((y0 - pad) / CELL).floor();
    let cy1 = ((y1 + pad) / CELL).floor();
    let mut cx = cx0;
    while cx <= cx1 {
        let mut cy = cy0;
        while cy <= cy1 {
            grid.entry(cell_key(cx, cy)).or_default().push(v.clone());
            cy += 1.0;
        }
        cx += 1.0;
    }
}

fn utf16_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// `assignCrashes(sc)`: assigns every crash to its link and its junction and finds the junctions and paired carriageways.
pub fn assign_crashes<'a>(sc: &'a Scenario, cr: &[Crash], cas: &CasLoad) -> Assignment<'a> {
    let links: Vec<&Link> = sc
        .links
        .values()
        .filter(|l| !l.status.is("proposed") && sc.nodes.contains_key(&l.a) && sc.nodes.contains_key(&l.b))
        .collect();
    let node_ids: Vec<&str> = sc.nodes.keys().collect();
    let mut lat0 = 0.0;
    for id in &node_ids {
        lat0 += sc.nodes[*id].lat.num();
    }
    lat0 = if !node_ids.is_empty() {
        lat0 / node_ids.len() as f64
    } else if !cr.is_empty() {
        cr[0].lat
    } else {
        0.0
    };
    let f = flat(lat0);
    // link segments in flat metres, gridded with the search radius as padding
    let mut seg: Vec<[f64; 5]> = Vec::new();
    let mut lgrid: HashMap<i64, Vec<usize>> = HashMap::new();
    let mut lk: Vec<LinkTally> = links
        .iter()
        .map(|l| LinkTally { link: l, nm: norm(l.name.as_str().unwrap_or("")), km: util::poly_len(&sc.geom(l)) / 1000.0, t: Tally::default() })
        .collect();
    let geoms: Vec<Vec<util::LatLng>> = links.iter().map(|l| sc.geom(l)).collect();
    for (li, g) in geoms.iter().enumerate() {
        for i in 0..g.len().saturating_sub(1) {
            let (ax, ay, bx, by) = (f.x(&g[i]), f.y(&g[i]), f.x(&g[i + 1]), f.y(&g[i + 1]));
            let s = seg.len();
            seg.push([ax, ay, bx, by, li as f64]);
            put(&mut lgrid, math_min(ax, bx), math_min(ay, by), math_max(ax, bx), math_max(ay, by), LINK_R, s);
        }
    }
    // Junction nodes: three or more existing links, and every node of a roundabout ring
    let mut deg: HashMap<&str, usize> = HashMap::new();
    let mut inc: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut on_ring: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for (li, l) in links.iter().enumerate() {
        for n in [l.a.as_str(), l.b.as_str()] {
            *deg.entry(n).or_insert(0) += 1;
            inc.entry(n).or_default().push(li);
            if l.ring.truthy() {
                on_ring.insert(n);
            }
        }
    }
    let mut jids: Vec<&str> = deg.keys().copied().filter(|id| deg[id] >= 3 || on_ring.contains(id)).collect();
    jids.sort_by(|a, b| utf16_cmp(a, b));
    let jidx: HashMap<&str, usize> = jids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let mut par: Vec<usize> = (0..jids.len()).collect();
    fn find(par: &mut [usize], mut id: usize) -> usize {
        while par[id] != id {
            par[id] = par[par[id]];
            id = par[id];
        }
        id
    }
    let join = |par: &mut Vec<usize>, a: usize, b: usize| {
        let x = find(par, a);
        let y = find(par, b);
        if x != y {
            // the smaller id is the root (the ids are in order)
            if x < y {
                par[y] = x;
            } else {
                par[x] = y;
            }
        }
    };
    for l in &links {
        if l.ring.truthy()
            && let (Some(&a), Some(&b)) = (jidx.get(l.a.as_str()), jidx.get(l.b.as_str()))
        {
            join(&mut par, a, b);
        }
    }
    let xy = |id: &str| -> (f64, f64) {
        let n = &sc.nodes[id];
        let ll = [n.lat.num(), n.lng.num()];
        (f.x(&ll), f.y(&ll))
    };
    let name_set = |id: &str| -> std::collections::HashSet<String> {
        inc.get(id)
            .map(|v| v.iter().map(|&li| lk[li].nm.clone()).filter(|n| !n.is_empty()).collect())
            .unwrap_or_default()
    };
    let mut pgrid: HashMap<i64, Vec<usize>> = HashMap::new();
    for (i, id) in jids.iter().enumerate() {
        if deg[id] >= 3 {
            let (x, y) = xy(id);
            put(&mut pgrid, x, y, x, y, 0.0, i);
        }
    }
    for (i, id) in jids.iter().enumerate() {
        if deg[id] < 3 {
            continue;
        }
        let (x, y) = xy(id);
        let mine = name_set(id);
        let mut seen: std::collections::HashSet<usize> = std::collections::HashSet::new();
        let mut cx = ((x - NODE_R) / CELL).floor();
        while cx <= ((x + NODE_R) / CELL).floor() {
            let mut cy = ((y - NODE_R) / CELL).floor();
            while cy <= ((y + NODE_R) / CELL).floor() {
                if let Some(cell) = pgrid.get(&cell_key(cx, cy)) {
                    for &o in cell {
                        if o <= i || seen.contains(&o) {
                            continue;
                        }
                        seen.insert(o);
                        let (ox, oy) = xy(jids[o]);
                        if math_hypot(&[ox - x, oy - y]) <= NODE_R && name_set(jids[o]).iter().any(|n| mine.contains(n)) {
                            join(&mut par, i, o);
                        }
                    }
                }
                cy += 1.0;
            }
            cx += 1.0;
        }
    }
    // groups in the order each root first appears among the sorted ids
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut group_of_root: HashMap<usize, usize> = HashMap::new();
    for i in 0..jids.len() {
        let r = find(&mut par, i);
        match group_of_root.get(&r) {
            Some(&g) => groups[g].push(i),
            None => {
                group_of_root.insert(r, groups.len());
                groups.push(vec![i]);
            }
        }
    }
    let mut jn: Vec<JunctionTally> = Vec::new();
    let mut j_of_node: HashMap<String, usize> = HashMap::new();
    let mut ngrid: HashMap<i64, Vec<usize>> = HashMap::new();
    let mut pt_node: Vec<(f64, f64, usize)> = Vec::new();
    for members in &groups {
        let inside: std::collections::HashSet<&str> = members.iter().map(|&i| jids[i]).collect();
        // Legs: links leaving the cluster; the two carriageways of one road on one side are one leg
        let mut sx = 0.0;
        let mut sy = 0.0;
        for &i in members {
            sx += xy(jids[i]).0;
        }
        for &i in members {
            sy += xy(jids[i]).1;
        }
        let (cx, cy) = (sx / members.len() as f64, sy / members.len() as f64);
        struct Leg {
            nm: String,
            b: f64,
            li: usize,
        }
        let mut legs: Vec<Leg> = Vec::new();
        let mut ext: Vec<usize> = Vec::new();
        for &i in members {
            for &li in inc.get(jids[i]).map(Vec::as_slice).unwrap_or(&[]) {
                let l = links[li];
                if inside.contains(l.a.as_str()) && inside.contains(l.b.as_str()) {
                    continue;
                }
                if ext.contains(&li) {
                    continue;
                }
                ext.push(li);
                let g = &geoms[li];
                let far = if inside.contains(l.a.as_str()) { g[math_min(g.len() as f64 - 1.0, 1.0) as usize] } else { g[math_max(0.0, g.len() as f64 - 2.0) as usize] };
                let b = (f.x(&far) - cx).atan2(f.y(&far) - cy) * 180.0 / std::f64::consts::PI;
                let nm = norm(l.name.as_str().unwrap_or(""));
                if !legs.iter().any(|lg| !nm.is_empty() && lg.nm == nm && ang_diff(lg.b, b) < 45.0) {
                    legs.push(Leg { nm, b, li });
                }
            }
        }
        if legs.len() < 3 {
            continue; // a turning loop or a bend, not a junction
        }
        let mut sorted: Vec<usize> = members.clone();
        js_sort_by(&mut sorted, |&a, &b| {
            let d = deg[jids[b]] as f64 - deg[jids[a]] as f64;
            if d != 0.0 { d } else if utf16_cmp(jids[a], jids[b]) == std::cmp::Ordering::Less { -1.0 } else { 1.0 }
        });
        let rep = jids[sorted[0]];
        let interchange = ext.iter().all(|&li| links[li].cls.is("motorway") || links[li].cls.is("ramp"));
        let mut named: Vec<&Link> = legs.iter().map(|l| links[l.li]).filter(|l| l.name.as_str().is_some_and(|n| !n.is_empty())).collect();
        js_sort_by(&mut named, |p, q| {
            let d = road_class(q.cls.as_str()).rank - road_class(p.cls.as_str()).rank;
            if d != 0.0 && !d.is_nan() { d } else { locale_compare_f64(p.name.as_str().unwrap_or(""), q.name.as_str().unwrap_or("")) }
        });
        let mut names: Vec<&str> = Vec::new();
        for l in &named {
            let n = l.name.as_str().unwrap_or("");
            if !names.contains(&n) {
                names.push(n);
            }
        }
        let joined = names.iter().take(3).copied().collect::<Vec<_>>().join(" / ");
        let name = if !joined.is_empty() {
            joined
        } else if interchange {
            "Motorway merge or diverge".to_string()
        } else {
            "Unnamed junction".to_string()
        };
        let node_names: Vec<String> = members.iter().map(|&i| jids[i].to_string()).collect();
        let j = JunctionTally { id: rep.to_string(), nodes: node_names, x: cx, y: cy, legs: legs.len(), interchange, name, t: Tally::default() };
        for &i in members {
            j_of_node.insert(jids[i].to_string(), jn.len());
            let (x, y) = xy(jids[i]);
            put(&mut ngrid, x, y, x, y, NODE_R, pt_node.len());
            pt_node.push((x, y, jn.len()));
        }
        jn.push(j);
    }
    let mut link_of = vec![-1i32; cr.len()];
    let mut node_of = vec![-1i32; cr.len()];
    for (ci, c) in cr.iter().enumerate() {
        let ll = [c.lat, c.lng];
        let (px, py) = (f.x(&ll), f.y(&ll));
        let k = cell_key((px / CELL).floor(), (py / CELL).floor());
        if let Some(cand) = lgrid.get(&k) {
            // dl: link index to its nearest distance, in the order links first appear
            let mut dl: Vec<(usize, f64)> = Vec::new();
            for &s in cand {
                let q = &seg[s];
                let r = seg_dist(px, py, q[0], q[1], q[2], q[3]);
                if r.d <= LINK_R {
                    let li = q[4] as usize;
                    match dl.iter_mut().find(|(l, _)| *l == li) {
                        Some(e) => {
                            // !(dl.get(li) <= r.d): a nearer piece replaces it
                            if !(e.1 <= r.d) {
                                e.1 = r.d;
                            }
                        }
                        None => dl.push((li, r.d)),
                    }
                }
            }
            let mut best: i64 = -1;
            let mut bd = f64::INFINITY;
            for &(li, dd) in &dl {
                if dd < bd {
                    bd = dd;
                    best = li as i64;
                }
            }
            if best >= 0 {
                let first = c.location.split(" / ").next().unwrap_or("");
                let loc = norm(first);
                if !loc.is_empty() && lk[best as usize].nm != loc {
                    let mut nb: i64 = -1;
                    let mut nd = f64::INFINITY;
                    for &(li, dd) in &dl {
                        let nm = &lk[li].nm;
                        if dd <= bd + NAME_SLACK && dd < nd && !nm.is_empty() && (*nm == loc || nm.contains(&loc) || loc.contains(nm.as_str())) {
                            nd = dd;
                            nb = li as i64;
                        }
                    }
                    if nb >= 0 {
                        best = nb;
                    }
                }
                link_of[ci] = best as i32;
                lk[best as usize].t.add(c);
            }
        }
        // to the junction of the nearest junction node within 30 m
        if let Some(nc) = ngrid.get(&k) {
            let mut best: i64 = -1;
            let mut bd = NODE_R;
            for &p in nc {
                let dd = math_hypot(&[px - pt_node[p].0, py - pt_node[p].1]);
                if dd <= bd {
                    bd = dd;
                    best = pt_node[p].2 as i64;
                }
            }
            if best >= 0 {
                node_of[ci] = best as i32;
                jn[best as usize].t.add(c);
            }
        }
    }
    // One-way carriageways paired with the opposite carriageway of the same road (a divided road)
    let mut pair: HashMap<String, String> = HashMap::new();
    let mut by_name: Vec<(String, Vec<usize>)> = Vec::new();
    for (li, it) in lk.iter().enumerate() {
        if it.nm.is_empty() || (it.link.lanes_ab.int32() > 0 && it.link.lanes_ba.int32() > 0) {
            continue;
        }
        match by_name.iter_mut().find(|(n, _)| *n == it.nm) {
            Some(e) => e.1.push(li),
            None => by_name.push((it.nm.clone(), vec![li])),
        }
    }
    let dir_b = |li: usize| -> f64 {
        let g = &geoms[li];
        let b = bearing(&g[0], &g[g.len() - 1]);
        if lk[li].link.lanes_ab.int32() > 0 { b } else { (b + 180.0) % 360.0 }
    };
    for (_, ids) in &by_name {
        for &li in ids {
            let g = &geoms[li];
            let mid = util::along(g, &util::cum_len(g), util::poly_len(g) / 2.0).p;
            let b = dir_b(li);
            for &lj in ids {
                if lj == li {
                    continue;
                }
                let o = lk[lj].link;
                if ang_diff(b, dir_b(lj)) > 135.0 && project(sc, o, mid).d <= PAIR_M {
                    pair.insert(lk[li].link.id.clone(), o.id.clone());
                    break;
                }
            }
        }
    }
    Assignment { lk, jn, j_of_node, link_of, node_of, pair, years: crash_years(cr, cas) }
}
