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
use crate::jsnum::{f64_to_i64, f64_to_usize, i64_to_f64, math_hypot, math_max, math_min, usize_to_f64};
use crate::project::{Link, Scenario, road_class};
use crate::util::{self, Flat, LatLng, ang_diff, bearing, flat, seg_dist};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

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
    // Bound: these count crashes held in memory, far below usize::MAX
    pub fn add(&mut self, c: &Crash) {
        self.crashes = self.crashes.saturating_add(1);
        match c.severity.as_str() {
            "F" => self.f = self.f.saturating_add(1),
            "S" => self.s = self.s.saturating_add(1),
            "M" => self.m = self.m.saturating_add(1),
            "N" => self.n = self.n.saturating_add(1),
            _ => {}
        }
        if c.severity == "F" || c.severity == "S" {
            self.dsi = self.dsi.saturating_add(1);
        }
        if c.severity == "F" || c.severity == "S" || c.severity == "M" {
            self.injury = self.injury.saturating_add(1);
        }
        if c.flags.contains('p') {
            self.p = self.p.saturating_add(1);
        }
        if c.flags.contains('c') {
            self.c = self.c.saturating_add(1);
        }
        if c.flags.contains('m') {
            self.mo = self.mo.saturating_add(1);
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
    #[must_use]
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
#[must_use]
pub fn crash_years(cr: &[Crash], cas: &CasLoad) -> Years {
    let (mut y0, mut y1) = (f64::INFINITY, f64::NEG_INFINITY);
    for c in cr {
        if c.year > 0.0 {
            y0 = math_min(y0, c.year);
            y1 = math_max(y1, c.year);
        }
    }
    if let Some(at) = cas.at {
        let year = i64_to_f64(nz_parts(at).year);
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
    f64_to_i64(cx * 1e6 + cy)
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

// The root of a junction node's cluster (union-find with path halving).
// Every entry of parent is a position in it: it starts as 0..len and only takes other entries' values.
fn find(parent: &mut [usize], mut id: usize) -> usize {
    while let Some(&up) = parent.get(id)
        && up != id
    {
        let grand = parent.get(up).copied().unwrap_or(up);
        if let Some(slot) = parent.get_mut(id) {
            *slot = grand;
        }
        id = grand;
    }
    id
}

// Joins two clusters; the smaller id is the root (the ids are in order)
fn join(parent: &mut [usize], a: usize, b: usize) {
    let ra = find(parent, a);
    let rb = find(parent, b);
    if ra != rb {
        let (root, child) = if ra < rb { (ra, rb) } else { (rb, ra) };
        if let Some(slot) = parent.get_mut(child) {
            *slot = root;
        }
    }
}

// A position in lk or jn as the -1-or-index arrays hold it.
// Bound: links and junctions in memory number far below i32::MAX, so the fallback never applies.
fn index_i32(i: usize) -> i32 {
    i32::try_from(i).unwrap_or(-1)
}

// A road leaving a junction: its name, its bearing from the centre, and its link (a position in links)
struct Leg {
    nm: String,
    b: f64,
    li: usize,
}

// The junctions found: their tallies, each node's junction, and the junction nodes gridded with the junction radius
struct Junctions {
    jn: Vec<JunctionTally>,
    j_of_node: HashMap<String, usize>,
    ngrid: HashMap<i64, Vec<usize>>,
    // flat position and junction of each junction node
    pt_node: Vec<(f64, f64, usize)>,
}

// The existing links of a scenario as the crash assignment reads them
struct Net<'a> {
    sc: &'a Scenario,
    // Existing links (not proposed, both ends present) in the scenario's order
    links: Vec<&'a Link>,
    // Their geometries, at the same positions
    geoms: Vec<Vec<LatLng>>,
    fl: Flat,
    // Existing links at each node: how many, and which (positions in links)
    deg: HashMap<&'a str, usize>,
    inc: HashMap<&'a str, Vec<usize>>,
    on_ring: HashSet<&'a str>,
}

impl<'a> Net<'a> {
    fn new(sc: &'a Scenario, cr: &[Crash]) -> Self {
        let links: Vec<&'a Link> = sc
            .links
            .values()
            .filter(|l| !l.status.is("proposed") && sc.nodes.contains_key(&l.a) && sc.nodes.contains_key(&l.b))
            .collect();
        let mut lat0 = 0.0;
        // Bound: a count of nodes in memory, far below usize::MAX
        let mut node_count = 0usize;
        for node in sc.nodes.values() {
            lat0 += node.lat.num();
            node_count = node_count.saturating_add(1);
        }
        lat0 = if node_count > 0 {
            lat0 / usize_to_f64(node_count)
        } else if let Some(first) = cr.first() {
            first.lat
        } else {
            0.0
        };
        let geoms: Vec<Vec<LatLng>> = links.iter().map(|l| sc.geom(l)).collect();
        // Junction nodes: three or more existing links, and every node of a roundabout ring
        let mut deg: HashMap<&'a str, usize> = HashMap::new();
        let mut inc: HashMap<&'a str, Vec<usize>> = HashMap::new();
        let mut on_ring: HashSet<&'a str> = HashSet::new();
        for (li, &road) in links.iter().enumerate() {
            for end in [road.a.as_str(), road.b.as_str()] {
                let count = deg.entry(end).or_insert(0);
                // Bound: a count of links, far below usize::MAX
                *count = count.saturating_add(1);
                inc.entry(end).or_default().push(li);
                if road.ring.truthy() {
                    on_ring.insert(end);
                }
            }
        }
        Net { sc, links, geoms, fl: flat(lat0), deg, inc, on_ring }
    }

    // Existing links at a node (every id asked about is a key of deg)
    fn deg_of(&self, id: &str) -> usize {
        self.deg.get(id).copied().unwrap_or(0)
    }

    fn xy(&self, id: &str) -> (f64, f64) {
        let ll = self.sc.nodes.get(id).map_or([f64::NAN; 2], |n| [n.lat.num(), n.lng.num()]);
        (self.fl.x(&ll), self.fl.y(&ll))
    }

    // The road names (as norm reads them) of the links at a node
    fn name_set(&self, lk: &[LinkTally], id: &str) -> HashSet<String> {
        self.inc
            .get(id)
            .map(|v| v.iter().filter_map(|&li| lk.get(li)).map(|t| t.nm.clone()).filter(|n| !n.is_empty()).collect())
            .unwrap_or_default()
    }

    // link segments in flat metres, gridded with the search radius as padding
    fn link_grid(&self) -> (Vec<[f64; 5]>, HashMap<i64, Vec<usize>>) {
        let mut seg: Vec<[f64; 5]> = Vec::new();
        let mut lgrid: HashMap<i64, Vec<usize>> = HashMap::new();
        for (li, geom) in self.geoms.iter().enumerate() {
            for pair in geom.windows(2) {
                let [pa, pb] = pair else { continue };
                let (ax, ay, bx, by) = (self.fl.x(pa), self.fl.y(pa), self.fl.x(pb), self.fl.y(pb));
                let s = seg.len();
                seg.push([ax, ay, bx, by, usize_to_f64(li)]);
                put(&mut lgrid, math_min(ax, bx), math_min(ay, by), math_max(ax, bx), math_max(ay, by), LINK_R, s);
            }
        }
        (seg, lgrid)
    }

    // The junction node ids in UTF-16 order, and their clusters: a roundabout's ring nodes together, and nodes of three
    // or more links within 30 m that share a road name. Clusters are in the order each root first appears among the ids.
    fn junction_groups(&self, lk: &[LinkTally]) -> (Vec<&'a str>, Vec<Vec<usize>>) {
        let mut jids: Vec<&'a str> = self.deg.keys().copied().filter(|id| self.deg_of(id) >= 3 || self.on_ring.contains(id)).collect();
        jids.sort_by(|a, b| utf16_cmp(a, b));
        let pos_of: HashMap<&str, usize> = jids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
        let mut parent: Vec<usize> = (0..jids.len()).collect();
        for link in &self.links {
            if link.ring.truthy()
                && let (Some(&a), Some(&b)) = (pos_of.get(link.a.as_str()), pos_of.get(link.b.as_str()))
            {
                join(&mut parent, a, b);
            }
        }
        let mut pgrid: HashMap<i64, Vec<usize>> = HashMap::new();
        for (ji, id) in jids.iter().enumerate() {
            if self.deg_of(id) >= 3 {
                let (nx, ny) = self.xy(id);
                put(&mut pgrid, nx, ny, nx, ny, 0.0, ji);
            }
        }
        for (ji, id) in jids.iter().enumerate() {
            if self.deg_of(id) < 3 {
                continue;
            }
            let (nx, ny) = self.xy(id);
            let mine = self.name_set(lk, id);
            let mut seen: HashSet<usize> = HashSet::new();
            let mut cx = ((nx - NODE_R) / CELL).floor();
            while cx <= ((nx + NODE_R) / CELL).floor() {
                let mut cy = ((ny - NODE_R) / CELL).floor();
                while cy <= ((ny + NODE_R) / CELL).floor() {
                    if let Some(cell) = pgrid.get(&cell_key(cx, cy)) {
                        for &oi in cell {
                            if oi <= ji || seen.contains(&oi) {
                                continue;
                            }
                            seen.insert(oi);
                            // pgrid holds only positions in jids
                            let Some(&other) = jids.get(oi) else { continue };
                            let (ox, oy) = self.xy(other);
                            if math_hypot(&[ox - nx, oy - ny]) <= NODE_R && self.name_set(lk, other).iter().any(|nm| mine.contains(nm)) {
                                join(&mut parent, ji, oi);
                            }
                        }
                    }
                    cy += 1.0;
                }
                cx += 1.0;
            }
        }
        let mut groups: Vec<Vec<usize>> = Vec::new();
        let mut group_of_root: HashMap<usize, usize> = HashMap::new();
        for ji in 0..jids.len() {
            let root = find(&mut parent, ji);
            if let Some(&gi) = group_of_root.get(&root) {
                if let Some(group) = groups.get_mut(gi) {
                    group.push(ji);
                }
            } else {
                group_of_root.insert(root, groups.len());
                groups.push(vec![ji]);
            }
        }
        (jids, groups)
    }

    // One cluster as a junction: its legs (links leaving the cluster; the two carriageways of one road on one side are
    // one leg), main node and name. None for a turning loop or a bend (under three legs).
    fn junction_of_group(&self, jids: &[&str], members: &[usize]) -> Option<JunctionTally> {
        // members are positions in jids (the clusters are built from them)
        let node_ids: Vec<&str> = members.iter().filter_map(|&i| jids.get(i).copied()).collect();
        let inside: HashSet<&str> = node_ids.iter().copied().collect();
        let mut sx = 0.0;
        let mut sy = 0.0;
        for id in &node_ids {
            sx += self.xy(id).0;
        }
        for id in &node_ids {
            sy += self.xy(id).1;
        }
        let (cx, cy) = (sx / usize_to_f64(members.len()), sy / usize_to_f64(members.len()));
        let mut legs: Vec<Leg> = Vec::new();
        let mut ext: Vec<usize> = Vec::new();
        for &id in &node_ids {
            for &li in self.inc.get(id).into_iter().flatten() {
                // inc holds positions in links, which geoms shares
                let (Some(&link), Some(geom)) = (self.links.get(li), self.geoms.get(li)) else { continue };
                if inside.contains(link.a.as_str()) && inside.contains(link.b.as_str()) {
                    continue;
                }
                if ext.contains(&li) {
                    continue;
                }
                ext.push(li);
                let len = usize_to_f64(geom.len());
                let far_i = if inside.contains(link.a.as_str()) { f64_to_usize(math_min(len - 1.0, 1.0)) } else { f64_to_usize(math_max(0.0, len - 2.0)) };
                // A link's geometry holds at least its two end nodes
                let Some(far) = geom.get(far_i) else { continue };
                let dir = (self.fl.x(far) - cx).atan2(self.fl.y(far) - cy) * 180.0 / std::f64::consts::PI;
                let nm = norm(link.name.as_str().unwrap_or(""));
                if !legs.iter().any(|lg| !nm.is_empty() && lg.nm == nm && ang_diff(lg.b, dir) < 45.0) {
                    legs.push(Leg { nm, b: dir, li });
                }
            }
        }
        if legs.len() < 3 {
            return None; // a turning loop or a bend, not a junction
        }
        let mut sorted: Vec<&str> = node_ids.clone();
        js_sort_by(&mut sorted, |&pa, &pb| {
            let d = usize_to_f64(self.deg_of(pb)) - usize_to_f64(self.deg_of(pa));
            if d != 0.0 { d } else if utf16_cmp(pa, pb) == std::cmp::Ordering::Less { -1.0 } else { 1.0 }
        });
        // A cluster holds at least one node
        let rep = sorted.first().copied().unwrap_or("");
        let interchange = ext.iter().all(|&li| self.links.get(li).is_some_and(|l| l.cls.is("motorway") || l.cls.is("ramp")));
        let mut named: Vec<&Link> = legs.iter().filter_map(|lg| self.links.get(lg.li).copied()).filter(|l| l.name.as_str().is_some_and(|n| !n.is_empty())).collect();
        js_sort_by(&mut named, |p, q| {
            let d = road_class(q.cls.as_str()).rank - road_class(p.cls.as_str()).rank;
            if d != 0.0 && !d.is_nan() { d } else { locale_compare_f64(p.name.as_str().unwrap_or(""), q.name.as_str().unwrap_or("")) }
        });
        let mut distinct: Vec<&str> = Vec::new();
        for link in &named {
            let road = link.name.as_str().unwrap_or("");
            if !distinct.contains(&road) {
                distinct.push(road);
            }
        }
        let joined = distinct.iter().take(3).copied().collect::<Vec<_>>().join(" / ");
        let name = if !joined.is_empty() {
            joined
        } else if interchange {
            "Motorway merge or diverge".to_string()
        } else {
            "Unnamed junction".to_string()
        };
        let node_names: Vec<String> = node_ids.iter().copied().map(str::to_string).collect();
        Some(JunctionTally { id: rep.to_string(), nodes: node_names, x: cx, y: cy, legs: legs.len(), interchange, name, t: Tally::default() })
    }

    fn junctions(&self, jids: &[&str], groups: &[Vec<usize>]) -> Junctions {
        let mut out = Junctions { jn: Vec::new(), j_of_node: HashMap::new(), ngrid: HashMap::new(), pt_node: Vec::new() };
        for members in groups {
            let Some(junction) = self.junction_of_group(jids, members) else { continue };
            for id in members.iter().filter_map(|&i| jids.get(i).copied()) {
                out.j_of_node.insert(id.to_string(), out.jn.len());
                let (nx, ny) = self.xy(id);
                put(&mut out.ngrid, nx, ny, nx, ny, NODE_R, out.pt_node.len());
                out.pt_node.push((nx, ny, out.jn.len()));
            }
            out.jn.push(junction);
        }
        out
    }

    // One-way carriageways paired with the opposite carriageway of the same road (a divided road)
    fn pair_carriageways(&self, lk: &[LinkTally]) -> HashMap<String, String> {
        let mut pair: HashMap<String, String> = HashMap::new();
        let mut by_name: Vec<(String, Vec<usize>)> = Vec::new();
        for (li, it) in lk.iter().enumerate() {
            if it.nm.is_empty() || (it.link.lanes_ab.int32() > 0 && it.link.lanes_ba.int32() > 0) {
                continue;
            }
            match by_name.iter_mut().find(|(nm, _)| *nm == it.nm) {
                Some(entry) => entry.1.push(li),
                None => by_name.push((it.nm.clone(), vec![li])),
            }
        }
        // li is a position in lk and geoms alike (both follow links); a geometry holds at least its two end nodes
        let dir_b = |li: usize| -> f64 {
            let Some((Some(first), Some(last))) = self.geoms.get(li).map(|g| (g.first(), g.last())) else { return f64::NAN };
            let dir = bearing(first, last);
            if lk.get(li).is_some_and(|t| t.link.lanes_ab.int32() > 0) { dir } else { (dir + 180.0) % 360.0 }
        };
        for (_, ids) in &by_name {
            for &li in ids {
                let (Some(geom), Some(own)) = (self.geoms.get(li), lk.get(li)) else { continue };
                let mid = util::along(geom, &util::cum_len(geom), util::poly_len(geom) / 2.0).p;
                let dir = dir_b(li);
                for &lj in ids {
                    if lj == li {
                        continue;
                    }
                    let Some(other) = lk.get(lj).map(|t| t.link) else { continue };
                    if ang_diff(dir, dir_b(lj)) > 135.0 && project(self.sc, other, mid).d <= PAIR_M {
                        pair.insert(own.link.id.clone(), other.id.clone());
                        break;
                    }
                }
            }
        }
        pair
    }
}

// The link a crash goes to: its nearest within 25 m, unless a link named in its first location is within 5 m more
fn nearest_link(crash: &Crash, px: f64, py: f64, cand: &[usize], seg: &[[f64; 5]], lk: &[LinkTally]) -> Option<usize> {
    // dl: link index to its nearest distance, in the order links first appear
    let mut dl: Vec<(usize, f64)> = Vec::new();
    for &si in cand {
        // The grid holds only positions of segments already pushed
        let Some(sq) = seg.get(si) else { continue };
        let sd = seg_dist(px, py, sq[0], sq[1], sq[2], sq[3]);
        if sd.d <= LINK_R {
            let li = f64_to_usize(sq[4]);
            match dl.iter_mut().find(|(seen, _)| *seen == li) {
                Some(entry) => {
                    // !(dl.get(li) <= r.d): a nearer piece replaces it, as does a NaN on either side
                    if entry.1 > sd.d || entry.1.is_nan() || sd.d.is_nan() {
                        entry.1 = sd.d;
                    }
                }
                None => dl.push((li, sd.d)),
            }
        }
    }
    let mut best: Option<usize> = None;
    let mut bd = f64::INFINITY;
    for &(li, dd) in &dl {
        if dd < bd {
            bd = dd;
            best = Some(li);
        }
    }
    let mut best = best?;
    let first = crash.location.split(" / ").next().unwrap_or("");
    let loc = norm(first);
    // Segments carry positions in lk (both follow links)
    if !loc.is_empty() && lk.get(best).is_some_and(|t| t.nm != loc) {
        let mut nb: Option<usize> = None;
        let mut nd = f64::INFINITY;
        for &(li, dd) in &dl {
            let Some(nm) = lk.get(li).map(|t| &t.nm) else { continue };
            if dd <= bd + NAME_SLACK && dd < nd && !nm.is_empty() && (*nm == loc || nm.contains(&loc) || loc.contains(nm.as_str())) {
                nd = dd;
                nb = Some(li);
            }
        }
        if let Some(named) = nb {
            best = named;
        }
    }
    Some(best)
}

// The junction of the nearest junction node within 30 m
fn nearest_junction(px: f64, py: f64, cand: &[usize], pt_node: &[(f64, f64, usize)]) -> Option<usize> {
    let mut best: Option<usize> = None;
    let mut bd = NODE_R;
    for &pi in cand {
        // The grid holds only positions of junction nodes already pushed
        let Some(&(nx, ny, ji)) = pt_node.get(pi) else { continue };
        let dd = math_hypot(&[px - nx, py - ny]);
        if dd <= bd {
            bd = dd;
            best = Some(ji);
        }
    }
    best
}

/// `assignCrashes(sc)`: assigns every crash to its link and its junction and finds the junctions and paired carriageways.
pub fn assign_crashes<'a>(sc: &'a Scenario, cr: &[Crash], cas: &CasLoad) -> Assignment<'a> {
    let net = Net::new(sc, cr);
    let (seg, lgrid) = net.link_grid();
    let mut lk: Vec<LinkTally<'a>> = net
        .links
        .iter()
        .map(|&l| LinkTally { link: l, nm: norm(l.name.as_str().unwrap_or("")), km: util::poly_len(&sc.geom(l)) / 1000.0, t: Tally::default() })
        .collect();
    let (jids, groups) = net.junction_groups(&lk);
    let mut js = net.junctions(&jids, &groups);
    let mut link_of: Vec<i32> = Vec::with_capacity(cr.len());
    let mut node_of: Vec<i32> = Vec::with_capacity(cr.len());
    for c in cr {
        let ll = [c.lat, c.lng];
        let (px, py) = (net.fl.x(&ll), net.fl.y(&ll));
        let k = cell_key((px / CELL).floor(), (py / CELL).floor());
        let li = lgrid.get(&k).and_then(|cand| nearest_link(c, px, py, cand, &seg, &lk));
        if let Some(li) = li
            && let Some(it) = lk.get_mut(li)
        {
            it.t.add(c);
        }
        link_of.push(li.map_or(-1, index_i32));
        // to the junction of the nearest junction node within 30 m
        let ji = js.ngrid.get(&k).and_then(|cand| nearest_junction(px, py, cand, &js.pt_node));
        if let Some(ji) = ji
            && let Some(j) = js.jn.get_mut(ji)
        {
            j.t.add(c);
        }
        node_of.push(ji.map_or(-1, index_i32));
    }
    let pair = net.pair_carriageways(&lk);
    Assignment { lk, jn: js.jn, j_of_node: js.j_of_node, link_of, node_of, pair, years: crash_years(cr, cas) }
}
