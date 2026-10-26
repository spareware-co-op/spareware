//! Network edits: model.js `Net`.
//!
//! Adding junctions and roads, snapping to the nearest ones, splitting a road at a point, deleting a road
//! or junction (and the junctions left with nothing), default junction control, and reversing a one-way road.
//! Positions are `[lat, lng]` pairs rounded to 7 decimals when stored. Ids come from an [`IdSource`].

use super::{EditError, IdSource};
use crate::jsnum::{Maybe, Num, math_max, math_min};
use crate::project::{Link, Node, Scenario, road_class, road_class_exact};
use crate::util::{self, LatLng};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

/* ---------------- props ---------------- */

/// The keys a caller gives `Net.addNode(sc, ll, props)` beyond the position: any node field left [`Maybe::Missing`]
/// is not set. The id and position come from the call.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeProps(pub Node);

impl Default for NodeProps {
    fn default() -> Self {
        NodeProps::new()
    }
}

impl NodeProps {
    /// No keys set.
    pub fn new() -> NodeProps {
        NodeProps(Node {
            id: String::new(),
            lat: Maybe::Missing,
            lng: Maybe::Missing,
            control: Maybe::Missing,
            osm: Maybe::Missing,
            allow_u: Maybe::Missing,
            bans: Maybe::Missing,
            lanes: Maybe::Missing,
            gaps: Maybe::Missing,
            cycle: Maybe::Missing,
            fixed_timing: Maybe::Missing,
            split: Maybe::Missing,
            upgrade: Maybe::Missing,
            extra: Map::new(),
        })
    }
    /// Sets the control ("none", "priority", "stop", "roundabout" or "signals").
    pub fn control(mut self, control: &str) -> Self {
        self.0.control = Maybe::Set(control.to_string());
        self
    }
    /// Sets the OpenStreetMap node id.
    pub fn osm(mut self, osm: Value) -> Self {
        self.0.osm = Maybe::Set(osm);
        self
    }
}

/// The keys a caller gives `Net.addLink(sc, a, b, pts, props)`: any link field left [`Maybe::Missing`] is not set, so the
/// class's lanes and speed (and the other defaults) stand. The id, ends and shape points come from the call.
#[derive(Clone, Debug, PartialEq)]
pub struct LinkProps(pub Link);

impl Default for LinkProps {
    fn default() -> Self {
        LinkProps::new()
    }
}

impl LinkProps {
    /// No keys set.
    pub fn new() -> LinkProps {
        LinkProps(Link {
            id: String::new(),
            a: String::new(),
            b: String::new(),
            name: Maybe::Missing,
            cls: Maybe::Missing,
            lanes_ab: Maybe::Missing,
            lanes_ba: Maybe::Missing,
            speed: Maybe::Missing,
            cap_per_lane: Maybe::Missing,
            status: Maybe::Missing,
            bus_lane: Maybe::Missing,
            cycle: Maybe::Missing,
            pts: Maybe::Missing,
            ring: Maybe::Missing,
            toll: Maybe::Missing,
            var_speed: Maybe::Missing,
            stage: Maybe::Missing,
            funding: Maybe::Missing,
            fc: Maybe::Missing,
            osm: Maybe::Missing,
            extra: Map::new(),
        })
    }
    /// Everything a link has except its id, ends and shape points (`const { id, a, b, pts, ...props } = L`).
    pub fn of_link(l: &Link) -> LinkProps {
        let mut p = l.clone();
        p.id = String::new();
        p.a = String::new();
        p.b = String::new();
        p.pts = Maybe::Missing;
        LinkProps(p)
    }
    /// Sets the road class key.
    pub fn cls(mut self, cls: &str) -> Self {
        self.0.cls = Maybe::Set(cls.to_string());
        self
    }
    /// Sets the road name.
    pub fn name(mut self, name: &str) -> Self {
        self.0.name = Maybe::Set(name.to_string());
        self
    }
    /// Sets the lanes each way (from a to b, from b to a).
    pub fn lanes(mut self, ab: f64, ba: f64) -> Self {
        self.0.lanes_ab = Num::number(ab);
        self.0.lanes_ba = Num::number(ba);
        self
    }
    /// Sets the speed limit (km/h).
    pub fn speed(mut self, speed: f64) -> Self {
        self.0.speed = Num::number(speed);
        self
    }
    /// Sets the status ("existing", "proposed" or "closed").
    pub fn status(mut self, status: &str) -> Self {
        self.0.status = Maybe::Set(status.to_string());
        self
    }
    /// Sets the bus lane flag.
    pub fn bus_lane(mut self, on: bool) -> Self {
        self.0.bus_lane = on.into();
        self
    }
    /// Sets the cycling provision ("none", "painted" or "separated").
    pub fn cycle(mut self, cycle: &str) -> Self {
        self.0.cycle = Maybe::Set(cycle.to_string());
        self
    }
    /// Sets the roundabout ring flag.
    pub fn ring(mut self, on: bool) -> Self {
        self.0.ring = on.into();
        self
    }
    /// Sets the OpenStreetMap way segment key.
    pub fn osm(mut self, osm: Value) -> Self {
        self.0.osm = Maybe::Set(osm);
        self
    }
}

macro_rules! overlay {
    ($dst:expr, $src:expr, [$($f:ident),* $(,)?]) => {
        $( if !$src.$f.is_missing() { $dst.$f = $src.$f.clone(); } )*
    };
}

/// `{ ...base, ...props }` for a node: every key the props set replaces the base's.
fn overlay_node(n: &mut Node, p: &Node) {
    overlay!(
        n,
        p,
        [
            lat,
            lng,
            control,
            osm,
            allow_u,
            bans,
            lanes,
            gaps,
            cycle,
            fixed_timing,
            split,
            upgrade
        ]
    );
    for (k, v) in &p.extra {
        n.extra.insert(k.clone(), v.clone());
    }
}

/// `{ ...base, ...props }` for a link (the id, ends and shape points are not overlaid).
fn overlay_link(l: &mut Link, p: &Link) {
    overlay!(
        l,
        p,
        [
            name,
            cls,
            lanes_ab,
            lanes_ba,
            speed,
            cap_per_lane,
            status,
            bus_lane,
            cycle,
            ring,
            toll,
            var_speed,
            stage,
            funding,
            fc,
            osm
        ]
    );
    for (k, v) in &p.extra {
        l.extra.insert(k.clone(), v.clone());
    }
}

/* ---------------- geometry ---------------- */

/// `Net.geom(sc, L)`: [node a, shape points..., node b] (both end nodes must exist).
pub fn geom(sc: &Scenario, l: &Link) -> Vec<LatLng> {
    sc.geom(l)
}

/// `Net.len(sc, L)`: length of a link along its shape (m).
pub fn len(sc: &Scenario, l: &Link) -> f64 {
    sc.link_len(l)
}

/// What `Net.project` finds: the distance to the link, the segment it is nearest to and the snapped point.
#[derive(Clone, Debug, PartialEq)]
pub struct Projection {
    /// Distance from the point to the link (m); infinite when no segment could be measured.
    pub d: f64,
    /// Index of the nearest segment of the link's geometry (None when none could be measured).
    pub i: Option<usize>,
    /// The snapped position [lat, lng] (NaN when none could be measured).
    pub p: LatLng,
    /// The link's geometry.
    pub g: Vec<LatLng>,
}

/// `Net.project(sc, L, ll)`: the point on a link nearest to ll, with its segment index.
pub fn project(sc: &Scenario, l: &Link, ll: LatLng) -> Projection {
    let g = sc.geom(l);
    let f = util::flat(ll[0]);
    let (px, py) = (f.x(&ll), f.y(&ll));
    let mut best_d = f64::INFINITY;
    let mut best: Option<(usize, f64, f64)> = None;
    for i in 0..g.len().saturating_sub(1) {
        let r = util::seg_dist(
            px,
            py,
            f.x(&g[i]),
            f.y(&g[i]),
            f.x(&g[i + 1]),
            f.y(&g[i + 1]),
        );
        if r.d < best_d {
            best_d = r.d;
            best = Some((i, r.x, r.y));
        }
    }
    match best {
        Some((i, x, y)) => Projection {
            d: best_d,
            i: Some(i),
            p: [y / f.ky, x / f.kx],
            g,
        },
        None => Projection {
            d: best_d,
            i: None,
            p: [f64::NAN, f64::NAN],
            g,
        },
    }
}

/// The nearest link `Net.nearestLink` found.
#[derive(Clone, Debug, PartialEq)]
pub struct NearestLink {
    /// Link id.
    pub id: String,
    /// Distance (m).
    pub d: f64,
    /// The snapped position [lat, lng].
    pub p: LatLng,
}

/// `Net.nearestLink(sc, ll, maxM, filter)`: the link nearest to ll within maxM metres (pass infinity for no limit).
pub fn nearest_link(
    sc: &Scenario,
    ll: LatLng,
    max_m: f64,
    filter: Option<&dyn Fn(&Link) -> bool>,
) -> Option<NearestLink> {
    let mut best: Option<NearestLink> = None;
    for l in sc.links.values() {
        if let Some(f) = filter
            && !f(l)
        {
            continue;
        }
        let (Some(a), Some(b)) = (sc.nodes.get(&l.a), sc.nodes.get(&l.b)) else {
            continue;
        };
        // quick reject on bounding box
        let pad = max_m / 90000.0 + 0.0005;
        let (alat, alng, blat, blng) = (a.lat.num(), a.lng.num(), b.lat.num(), b.lng.num());
        let (mut min_la, mut max_la) = (math_min(alat, blat), math_max(alat, blat));
        let (mut min_ln, mut max_ln) = (math_min(alng, blng), math_max(alng, blng));
        if let Some(pts) = l.pts.get() {
            for p in pts {
                min_la = math_min(min_la, p.0[0]);
                max_la = math_max(max_la, p.0[0]);
                min_ln = math_min(min_ln, p.0[1]);
                max_ln = math_max(max_ln, p.0[1]);
            }
        }
        if max_m.is_finite()
            && (ll[0] < min_la - pad
                || ll[0] > max_la + pad
                || ll[1] < min_ln - pad * 1.3
                || ll[1] > max_ln + pad * 1.3)
        {
            continue;
        }
        let r = project(sc, l, ll);
        if r.d <= max_m && best.as_ref().is_none_or(|b| r.d < b.d) {
            best = Some(NearestLink {
                id: l.id.clone(),
                d: r.d,
                p: r.p,
            });
        }
    }
    best
}

/// `Net.nearestNode(sc, ll, maxM, filter)`: the id of the node nearest to ll within maxM metres (pass infinity for no limit).
pub fn nearest_node(
    sc: &Scenario,
    ll: LatLng,
    max_m: f64,
    filter: Option<&dyn Fn(&Node) -> bool>,
) -> Option<String> {
    let mut best: Option<String> = None;
    let mut bd = max_m;
    for n in sc.nodes.values() {
        if let Some(f) = filter
            && !f(n)
        {
            continue;
        }
        let d = util::dist(&ll, &n.ll());
        if d < bd {
            bd = d;
            best = Some(n.id.clone());
        }
    }
    best
}

/* ---------------- adding ---------------- */

/// `Net.addNode(sc, ll, props)`: adds a junction (last in order) with control "none" unless the props say otherwise; returns its id.
pub fn add_node(
    sc: &mut Scenario,
    ids: &mut dyn IdSource,
    ll: LatLng,
    props: &NodeProps,
) -> String {
    let id = ids.uid("n");
    let mut n = Node::new(&id, ll);
    overlay_node(&mut n, &props.0);
    sc.nodes.insert(id.clone(), n);
    id
}

/// `Net.addLink(sc, a, b, pts, props)`: adds a road (last in order); returns its id.
///
/// Without props it is a local road, existing, with the class's lanes each way and speed. The class named in the props sets
/// those defaults, and the props then replace any key they set. Shape points are rounded to 7 decimals.
pub fn add_link(
    sc: &mut Scenario,
    ids: &mut dyn IdSource,
    a: &str,
    b: &str,
    pts: &[LatLng],
    props: &LinkProps,
) -> String {
    // `CLASSES[props.cls || 'local'] || CLASSES.local`
    let key = props.0.cls.str_or("local");
    let class_key = road_class_exact(key).map_or("local", |c| c.key);
    let id = ids.uid("l");
    let mut l = Link::new(&id, a, b, pts, Some(class_key));
    // the base object's own cls is 'local' until the props replace it
    l.cls = Maybe::Set("local".to_string());
    overlay_link(&mut l, &props.0);
    sc.links.insert(id.clone(), l);
    id
}

/// `Net.linksAt(sc, nid)`: the links that start or end at a junction, in the order the scenario lists them.
pub fn links_at<'a>(sc: &'a Scenario, nid: &str) -> Vec<&'a Link> {
    sc.links
        .values()
        .filter(|l| l.a == nid || l.b == nid)
        .collect()
}

/// `Net.degree(sc)`: how many link ends meet at each junction (a link that starts and ends there counts twice).
pub fn degree(sc: &Scenario) -> HashMap<String, usize> {
    let mut d: HashMap<String, usize> = HashMap::new();
    for l in sc.links.values() {
        *d.entry(l.a.clone()).or_insert(0) += 1;
        *d.entry(l.b.clone()).or_insert(0) += 1;
    }
    d
}

/* ---------------- splitting ---------------- */

/// `s.slice(start, end)` with the JavaScript's index rules for a number that may be negative or NaN.
fn js_slice<T: Clone>(v: &[T], start: f64, end: f64) -> Vec<T> {
    let n = v.len() as f64;
    let rel = |x: f64| {
        let x = if x.is_nan() { 0.0 } else { x.trunc() };
        if x < 0.0 {
            math_max(n + x, 0.0)
        } else {
            math_min(x, n)
        }
    };
    let (s, e) = (rel(start), rel(end));
    if s >= e {
        Vec::new()
    } else {
        v[s as usize..e as usize].to_vec()
    }
}

/// `Net.splitLink(sc, id, ll)`: splits a road at the point nearest ll and returns the junction there.
///
/// Within 2.5 m of an end of the road, no split is made and that end's junction is returned. Otherwise a new junction is
/// added, the road now ends at it, and a new road (a copy of the road's other keys) runs from it to the old end.
pub fn split_link(
    sc: &mut Scenario,
    ids: &mut dyn IdSource,
    id: &str,
    ll: LatLng,
) -> Result<String, EditError> {
    let l = sc
        .links
        .get(id)
        .ok_or_else(|| EditError(format!("there is no road {id}")))?
        .clone();
    if !sc.nodes.contains_key(&l.a) || !sc.nodes.contains_key(&l.b) {
        return Err(EditError(format!("road {id} has a missing end junction")));
    }
    let r = project(sc, &l, ll);
    let g = &r.g;
    if util::dist(&r.p, &g[0]) < 2.5 {
        return Ok(l.a);
    }
    if util::dist(&r.p, &g[g.len() - 1]) < 2.5 {
        return Ok(l.b);
    }
    let nid = add_node(sc, ids, r.p, &NodeProps::new());
    let i = r.i.map_or(f64::NAN, |i| i as f64);
    let pts1 = js_slice(g, 1.0, i + 1.0);
    let pts2 = js_slice(g, i + 1.0, (g.len() - 1) as f64);
    let props = LinkProps::of_link(&l);
    if let Some(m) = sc.links.get_mut(id) {
        m.b = nid.clone();
        m.pts = Maybe::Set(
            pts1.iter()
                .map(|p| crate::project::Point(*p))
                .collect::<Vec<_>>(),
        );
    }
    add_link(sc, ids, &nid, &l.b, &pts2, &props);
    Ok(nid)
}

/* ---------------- junction control ---------------- */

/// `Net.autoControl(sc, ids)`: junctions with three or more roads that have no control get give-way control (not where
/// every road is a motorway or ramp); junctions with fewer than three roads lose give-way or stop control.
pub fn auto_control(sc: &mut Scenario, ids: &[String]) {
    let mut deg: HashMap<String, f64> = HashMap::new();
    let mut fwy: HashMap<String, bool> = HashMap::new();
    for l in sc.links.values() {
        let rank = road_class(l.cls.as_str()).rank;
        for id in [&l.a, &l.b] {
            *deg.entry(id.clone()).or_insert(0.0) += 1.0;
            let prev = fwy.get(id).copied().unwrap_or(true);
            fwy.insert(id.clone(), prev && rank >= 5.0);
        }
    }
    for id in ids {
        let Some(n) = sc.nodes.get_mut(id) else {
            continue;
        };
        let d = deg.get(id).copied().unwrap_or(0.0);
        if d >= 3.0 && n.control.is("none") && !fwy.get(id).copied().unwrap_or(false) {
            n.control = Maybe::Set("priority".to_string());
        }
        if d < 3.0 && (n.control.is("priority") || n.control.is("stop")) {
            n.control = Maybe::Set("none".to_string());
        }
    }
}

/* ---------------- deleting ---------------- */

/// `Net.pruneNode(sc, nid)`: removes a junction that has no roads, is no zone's node and is on no bus route.
pub fn prune_node(sc: &mut Scenario, nid: &str) {
    if !sc.nodes.contains_key(nid) {
        return;
    }
    if sc.links.values().any(|l| l.a == nid || l.b == nid) {
        return;
    }
    if sc.zones.values().any(|z| z.node.is(nid)) {
        return;
    }
    if sc
        .routes
        .values()
        .any(|r| r.wp.get().is_some_and(|wp| wp.iter().any(|w| w == nid)))
    {
        return;
    }
    sc.nodes.remove(nid);
}

/// `Net.deleteLink(sc, id)`: deletes a road, then any junction it leaves unused, and resets the ends' control.
pub fn delete_link(sc: &mut Scenario, id: &str) {
    let Some(l) = sc.links.remove(id) else {
        return;
    };
    prune_node(sc, &l.a);
    prune_node(sc, &l.b);
    auto_control(sc, &[l.a.clone(), l.b.clone()]);
}

/// `Net.deleteNode(sc, nid)`: deletes a junction and its roads; zones and bus routes drop it; the junctions at the far
/// ends are pruned if unused and their control reset.
pub fn delete_node(sc: &mut Scenario, nid: &str) {
    let mut nbrs: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let touching: Vec<(String, String, String)> = links_at(sc, nid)
        .iter()
        .map(|l| (l.id.clone(), l.a.clone(), l.b.clone()))
        .collect();
    for (lid, a, b) in touching {
        sc.links.remove(&lid);
        for n in [a, b] {
            if seen.insert(n.clone()) {
                nbrs.push(n);
            }
        }
    }
    nbrs.retain(|n| n != nid);
    sc.nodes.remove(nid);
    for z in sc.zones.values_mut() {
        if z.node.is(nid) {
            z.node = Maybe::Null;
        }
    }
    for r in sc.routes.values_mut() {
        let kept: Vec<String> =
            r.wp.get()
                .map(|wp| wp.iter().filter(|w| w.as_str() != nid).cloned().collect())
                .unwrap_or_default();
        r.wp = Maybe::Set(kept);
    }
    for n in &nbrs {
        prune_node(sc, n);
    }
    auto_control(sc, &nbrs);
}

/// `Net.reverse(sc, id)`: swaps the lanes each way of a road (a one-way road then runs the other way).
pub fn reverse(sc: &mut Scenario, id: &str) -> Result<(), EditError> {
    let l = sc
        .links
        .get_mut(id)
        .ok_or_else(|| EditError(format!("there is no road {id}")))?;
    std::mem::swap(&mut l.lanes_ab, &mut l.lanes_ba);
    Ok(())
}
