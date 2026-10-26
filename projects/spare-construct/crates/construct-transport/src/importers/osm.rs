//! Roads from OpenStreetMap: model.js `OSM_CLASS`, `Importers.overpass` (the query), `osmToScenario` and `autoGateways`.
//!
//! The download itself (the HTTP request to an Overpass server) stays outside: [`overpass_query`] writes the query text
//! and [`osm_to_scenario`] reads the JSON answer. Ways become roads with their class, lanes, speed limit, one-way rule,
//! roundabout ring, bus lane and cycle provision read from the tags; their nodes become junctions, with control from the tags
//! and the geometry. [`auto_gateways`] then adds external zones where roads leave the imported area.

use super::jsparse::{parse_int10, parse_int10_value};
use crate::edit::IdSource;
use crate::edit::net::{self, LinkProps, NodeProps};
use crate::edit::sites::{self, AtPoint, DetectOpts};
use crate::jsnum::{
    Maybe, Num, math_hypot, math_max, math_min, math_round, to_fixed, value_to_js_string,
    value_to_number,
};
use crate::project::{OrderedMap, Scenario, Zone, road_class, road_class_exact};
use crate::util::{self, LatLng, r7};
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};

/// model.js OSM_CLASS: the OpenStreetMap `highway` values the import reads, in its order, with the road class each becomes.
pub const OSM_CLASS: [(&str, &str); 14] = [
    ("motorway", "motorway"),
    ("trunk", "arterial"),
    ("primary", "arterial"),
    ("secondary", "arterial"),
    ("tertiary", "collector"),
    ("unclassified", "local"),
    ("residential", "local"),
    ("living_street", "local"),
    ("service", "local"),
    ("motorway_link", "ramp"),
    ("trunk_link", "ramp"),
    ("primary_link", "arterial"),
    ("secondary_link", "arterial"),
    ("tertiary_link", "collector"),
];

/// model.js OVERPASS: the Overpass servers the prototype tries in turn.
pub const OVERPASS: [&str; 4] = [
    "https://overpass-api.de/api/interpreter",
    "https://maps.mail.ru/osm/tools/overpass/api/interpreter",
    "https://overpass.private.coffee/api/interpreter",
    "https://overpass.kumi.systems/api/interpreter",
];

/// `OSM_CLASS[highway]`: the road class a `highway` value becomes.
pub fn osm_class(highway: &str) -> Option<&'static str> {
    OSM_CLASS
        .iter()
        .find(|(k, _)| *k == highway)
        .map(|(_, v)| *v)
}

/// A map box (south, west, north, east in degrees), as the prototype's Leaflet bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    /// South edge (latitude).
    pub south: f64,
    /// West edge (longitude).
    pub west: f64,
    /// North edge (latitude).
    pub north: f64,
    /// East edge (longitude).
    pub east: f64,
}

/// What `Importers.overpass(b, opts)` is given: whether to include service lanes and local streets.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OverpassOpts {
    /// Include `highway=service` (lanes and alleys).
    pub service: bool,
    /// False leaves out local and residential streets (anything else, including None, includes them).
    pub local: Option<bool>,
}

/// The Overpass QL query of `Importers.overpass(b, opts)`: every way with one of the road `highway` values inside the box.
pub fn overpass_query(b: &Bounds, opts: &OverpassOpts) -> String {
    let types: Vec<&str> = OSM_CLASS
        .iter()
        .filter(|(t, _)| opts.service || *t != "service")
        .filter(|(_, c)| opts.local != Some(false) || *c != "local")
        .map(|(t, _)| *t)
        .collect();
    let bbox = [b.south, b.west, b.north, b.east]
        .iter()
        .map(|v| to_fixed(*v, 6))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "[out:json][timeout:120];way[\"highway\"~\"^({})$\"]({});out body;>;out body qt;",
        types.join("|"),
        bbox
    )
}

/// What `Importers.osmToScenario(sc, data, opts)` is given: `sites` also groups the junction nodes into sites
/// (docs/spec/junction-sites; off by default so existing imports keep their results), with the AT points to use.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OsmOpts {
    /// Detect junction sites after the import (`opts.sites === true`).
    pub sites: bool,
    /// AT controlled-intersection points for the site detection.
    pub points: Vec<AtPoint>,
}

/// An OpenStreetMap id as a map key: the number's bits (so 0 and -0 are one key).
fn key_of(v: &Value) -> Option<u64> {
    let x = v.as_f64()?;
    Some(if x == 0.0 { 0 } else { x.to_bits() })
}

/// A tag's text (`t[k]`), if it is text; other scalars are read as their JavaScript text.
fn tag<'a>(t: &'a Value, k: &str) -> Option<std::borrow::Cow<'a, str>> {
    match t.get(k)? {
        Value::String(s) => Some(std::borrow::Cow::Borrowed(s)),
        Value::Null => None,
        other => Some(std::borrow::Cow::Owned(value_to_js_string(other))),
    }
}

/// `t[k] || ''` as text.
fn tag_or_empty(t: &Value, k: &str) -> String {
    tag(t, k).map(|s| s.into_owned()).unwrap_or_default()
}

/// `a.b.includes` on tags: the OpenStreetMap elements of one data set in file order, looked up by id.
struct Elements<'a> {
    list: Vec<&'a Value>,
    index: HashMap<u64, usize>,
}

impl<'a> Elements<'a> {
    fn get(&self, id: &Value) -> Option<&'a Value> {
        key_of(id)
            .and_then(|k| self.index.get(&k))
            .map(|&i| self.list[i])
    }
    fn has(&self, id: &Value) -> bool {
        key_of(id).is_some_and(|k| self.index.contains_key(&k))
    }
}

fn is_yes(s: &str) -> bool {
    matches!(s, "yes" | "true" | "1")
}

/// `+o[k]` of an element: NaN when the key is missing (undefined).
fn field_num(o: &Value, k: &str) -> f64 {
    o.get(k).map_or(f64::NAN, value_to_number)
}

/// The import's working state: the elements, the junction made for each OpenStreetMap node, and what it touched.
struct State<'a> {
    nodes: Elements<'a>,
    by_osm: HashMap<u64, String>,
    touched: Vec<String>,
    touched_set: HashSet<String>,
    ring_nodes: HashSet<String>,
    added: usize,
}

impl State<'_> {
    /// `getNode(oid)`: the junction for an OpenStreetMap node, made when it is not there yet.
    fn get_node(&mut self, sc: &mut Scenario, ids: &mut dyn IdSource, oid: &Value) -> String {
        let k = key_of(oid).expect("a number id");
        if let Some(id) = self.by_osm.get(&k)
            && !id.is_empty()
        {
            return id.clone();
        }
        let o = self.nodes.get(oid).expect("a node element");
        let ll = [field_num(o, "lat"), field_num(o, "lon")];
        let id = net::add_node(sc, ids, ll, &NodeProps::new().osm(oid.clone()));
        self.by_osm.insert(k, id.clone());
        id
    }

    /// `addSeg(seg, key)`: one road between the segment's end nodes, with the way's tags as props.
    fn add_seg(
        &mut self,
        sc: &mut Scenario,
        ids: &mut dyn IdSource,
        seg: &[&Value],
        key: String,
        props: &mut LinkProps,
        ring: bool,
    ) {
        let a = self.get_node(sc, ids, seg[0]);
        let b = self.get_node(sc, ids, seg[seg.len() - 1]);
        if a == b {
            return;
        }
        let pts: Vec<LatLng> = seg[1..seg.len() - 1]
            .iter()
            .map(|id| {
                let o = self.nodes.get(id).expect("a node element");
                [field_num(o, "lat"), field_num(o, "lon")]
            })
            .collect();
        props.0.osm = Maybe::Set(Value::String(key));
        net::add_link(sc, ids, &a, &b, &pts, props);
        self.added += 1;
        for n in [&a, &b] {
            if self.touched_set.insert(n.clone()) {
                self.touched.push(n.clone());
            }
        }
        if ring {
            self.ring_nodes.insert(a);
            self.ring_nodes.insert(b);
        }
    }
}

/// `Importers.osmToScenario(sc, data, opts)`: adds the roads in an Overpass JSON answer (`data.elements`, nodes and ways) to a
/// scenario. Roads already imported (by OpenStreetMap way and segment) are left alone. Returns how many road segments were added.
///
/// Junction control: three or more roads and no control give give-way control, roundabout rings are roundabouts,
/// `highway=stop` and `highway=traffic_signals` tags apply, motorway-only nodes stay uncontrolled, and signals tagged on the
/// stop line just before a junction move onto the junction (within 45 m).
pub fn osm_to_scenario(
    sc: &mut Scenario,
    ids: &mut dyn IdSource,
    data: &Value,
    opts: &OsmOpts,
) -> usize {
    let mut nodes = Elements {
        list: Vec::new(),
        index: HashMap::new(),
    };
    let mut ways: Vec<&Value> = Vec::new();
    if let Some(els) = data.get("elements").and_then(Value::as_array) {
        for e in els {
            match e.get("type").and_then(Value::as_str) {
                Some("node") => {
                    // Map.set: a repeated id keeps its first place and takes the later element
                    if let Some(k) = e.get("id").and_then(key_of) {
                        match nodes.index.get(&k) {
                            Some(&i) => nodes.list[i] = e,
                            None => {
                                nodes.index.insert(k, nodes.list.len());
                                nodes.list.push(e);
                            }
                        }
                    }
                }
                Some("way") => {
                    if e.get("nodes")
                        .and_then(Value::as_array)
                        .is_some_and(|n| n.len() > 1)
                    {
                        ways.push(e);
                    }
                }
                _ => {}
            }
        }
    }
    let empty = json!({});
    let keep: Vec<&Value> = ways
        .iter()
        .copied()
        .filter(|w| {
            let t = w.get("tags").filter(|t| t.is_object()).unwrap_or(&empty);
            let hw = tag(t, "highway");
            let Some(cls) = hw.as_deref().and_then(osm_class) else {
                return false;
            };
            if t.get("area").and_then(Value::as_str) == Some("yes") {
                return false;
            }
            let access = tag_or_empty(t, "access");
            if cls == "local" && (access == "no" || access == "private") {
                return false;
            }
            if hw.as_deref() == Some("service") {
                let svc = tag_or_empty(t, "service");
                if !svc.is_empty() && svc != "alley" {
                    return false;
                }
            }
            true
        })
        .collect();
    // How many ways use each node: ends count twice, so a shared node or an end is a junction
    let mut used: HashMap<u64, f64> = HashMap::new();
    for w in &keep {
        let ns = w["nodes"].as_array().expect("nodes");
        for (i, id) in ns.iter().enumerate() {
            if let Some(k) = key_of(id) {
                *used.entry(k).or_insert(0.0) += if i == 0 || i == ns.len() - 1 {
                    2.0
                } else {
                    1.0
                };
            }
        }
    }
    let mut st = State {
        nodes,
        by_osm: HashMap::new(),
        touched: Vec::new(),
        touched_set: HashSet::new(),
        ring_nodes: HashSet::new(),
        added: 0,
    };
    for n in sc.nodes.values() {
        if let Maybe::Set(osm) = &n.osm
            && crate::jsnum::value_truthy(osm)
            && let Some(k) = key_of(osm)
        {
            st.by_osm.insert(k, n.id.clone());
        }
    }
    let mut have: HashSet<String> = HashSet::new();
    for l in sc.links.values() {
        if let Maybe::Set(osm) = &l.osm
            && crate::jsnum::value_truthy(osm)
        {
            have.insert(value_to_js_string(osm));
        }
    }
    for w in &keep {
        let t = w.get("tags").filter(|t| t.is_object()).unwrap_or(&empty);
        let hw = tag_or_empty(t, "highway");
        let cls = osm_class(&hw).expect("kept ways have a class");
        let c = road_class_exact(cls).expect("a known class");
        let junction = tag_or_empty(t, "junction");
        let ring = junction == "roundabout" || junction == "circular";
        let mut ow: i32 = if ring || hw == "motorway" || hw == "motorway_link" {
            1
        } else {
            0
        };
        let oneway = tag_or_empty(t, "oneway");
        if is_yes(&oneway) {
            ow = 1;
        } else if oneway == "-1" || oneway == "reverse" {
            ow = -1;
        } else if oneway == "no" && !ring {
            ow = 0;
        }
        let lanes_t = parse_int10_value(t.get("lanes"));
        let (mut ab, mut ba);
        if ow != 0 {
            let n = if lanes_t > 0.0 {
                lanes_t
            } else if cls == "motorway" {
                2.0
            } else {
                1.0
            };
            ab = if ow > 0 { n } else { 0.0 };
            ba = if ow < 0 { n } else { 0.0 };
        } else {
            let f = parse_int10_value(t.get("lanes:forward"));
            let bk = parse_int10_value(t.get("lanes:backward"));
            ab = if f > 0.0 {
                f
            } else if lanes_t > 1.0 {
                (lanes_t / 2.0).ceil()
            } else {
                1.0
            };
            ba = if bk > 0.0 {
                bk
            } else if lanes_t > 1.0 {
                (lanes_t / 2.0).floor()
            } else {
                1.0
            };
        }
        ab = math_min(ab, 6.0);
        ba = math_min(ba, 6.0);
        let maxspeed = tag(t, "maxspeed");
        let mut speed = parse_int10_value(t.get("maxspeed"));
        if !(speed > 0.0) {
            speed = if maxspeed.as_deref() == Some("NZ:rural") {
                100.0
            } else if maxspeed.as_deref() == Some("NZ:urban") {
                50.0
            } else if hw == "trunk" {
                80.0
            } else {
                c.speed
            };
        }
        // [t.cycleway, t['cycleway:left'], ...].join(' ')
        let cyc = [
            "cycleway",
            "cycleway:left",
            "cycleway:right",
            "cycleway:both",
        ]
        .iter()
        .map(|k| tag_or_empty(t, k))
        .collect::<Vec<_>>()
        .join(" ");
        let bus_lanes = tag_or_empty(t, "bus:lanes");
        let mut bus_lane = bus_lanes.starts_with("designated")
            || bus_lanes.contains("|designated")
            || tag_or_empty(t, "lanes:psv").contains("bus");
        if bus_lane && math_max(ab, ba) < 2.0 {
            bus_lane = false;
        }
        let name = {
            let n = tag_or_empty(t, "name");
            if n.is_empty() {
                tag_or_empty(t, "ref")
            } else {
                n
            }
        };
        let cycle = if cyc.contains("track") || cyc.contains("separate") {
            "separated"
        } else if cyc.contains("lane") {
            "painted"
        } else {
            "none"
        };
        let mut props = LinkProps::new()
            .cls(cls)
            .lanes(ab, ba)
            .speed(speed)
            .name(&name)
            .status("existing")
            .ring(ring)
            .bus_lane(bus_lane)
            .cycle(cycle);
        let way_id = value_to_js_string(&w["id"]);
        let node_ids: Vec<&Value> = w["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .filter(|id| st.nodes.has(id))
            .collect();
        let mut s = 0usize;
        for i in 1..node_ids.len() {
            if i < node_ids.len() - 1
                && used
                    .get(&key_of(node_ids[i]).expect("id"))
                    .copied()
                    .unwrap_or(0.0)
                    < 2.0
            {
                continue;
            }
            let seg: Vec<&Value> = node_ids[s..=i].to_vec();
            let key = format!("{way_id}:{s}");
            s = i;
            if have.contains(&key) || have.contains(&format!("{key}a")) {
                continue;
            }
            if key_of(seg[0]) == key_of(seg[seg.len() - 1]) {
                if seg.len() < 4 {
                    continue;
                }
                let m = seg.len() >> 1;
                st.add_seg(sc, ids, &seg[..=m], format!("{key}a"), &mut props, ring);
                st.add_seg(sc, ids, &seg[m..], format!("{key}b"), &mut props, ring);
            } else {
                st.add_seg(sc, ids, &seg, key, &mut props, ring);
            }
        }
    }
    let State {
        nodes,
        by_osm,
        touched,
        ring_nodes,
        added,
        ..
    } = st;
    // Junction control from tags and geometry; motorway-only nodes (merges, diverges) stay uncontrolled
    let deg = net::degree(sc);
    let mut fwy: HashMap<String, bool> = HashMap::new();
    for l in sc.links.values() {
        let rank = road_class(l.cls.as_str()).rank;
        for id in [&l.a, &l.b] {
            let prev = fwy.get(id).copied().unwrap_or(true);
            fwy.insert(id.clone(), prev && rank >= 5.0);
        }
    }
    for nid in &touched {
        let d = deg.get(nid).copied().unwrap_or(0) as f64;
        let osm = sc.nodes[nid.as_str()].osm.get().cloned();
        let tags: Value = osm
            .as_ref()
            .and_then(|o| nodes.get(o))
            .and_then(|e| e.get("tags"))
            .filter(|t| !t.is_null())
            .cloned()
            .unwrap_or_else(|| json!({}));
        let hwtag = tags.get("highway").and_then(Value::as_str);
        let n = sc.nodes.get_mut(nid).expect("a touched node");
        if ring_nodes.contains(nid) && d >= 3.0 {
            n.control = Maybe::Set("roundabout".to_string());
        } else if d >= 3.0 && n.control.is("none") && !fwy.get(nid).copied().unwrap_or(false) {
            n.control = Maybe::Set("priority".to_string());
        }
        if hwtag == Some("stop") && d >= 3.0 {
            n.control = Maybe::Set("stop".to_string());
        }
        if hwtag == Some("traffic_signals") {
            n.control = Maybe::Set("signals".to_string());
        }
    }
    // Signals are often tagged on the stop line just before a junction: move them onto the junction
    let junctions: Vec<&String> = touched
        .iter()
        .filter(|id| deg.get(*id).copied().unwrap_or(0) >= 3)
        .collect();
    for o in &nodes.list {
        let Some(tags) = o.get("tags").filter(|t| t.is_object()) else {
            continue;
        };
        if tags.get("highway").and_then(Value::as_str) != Some("traffic_signals") {
            continue;
        }
        let own = o.get("id").and_then(key_of).and_then(|k| by_osm.get(&k));
        if let Some(own) = own
            && !own.is_empty()
            && deg.get(own).copied().unwrap_or(0) >= 3
        {
            continue;
        }
        let at: LatLng = [value_to_number(&o["lat"]), value_to_number(&o["lon"])];
        let mut best: Option<&String> = None;
        let mut bd = 45.0;
        for j in &junctions {
            let n = &sc.nodes[j.as_str()];
            let d = util::dist(&at, &n.ll());
            if d < bd {
                bd = d;
                best = Some(j);
            }
        }
        if let Some(b) = best {
            sc.nodes.get_mut(b).expect("a node").control = Maybe::Set("signals".to_string());
        }
    }
    // A signal tagged on a plain road bend is a pedestrian crossing; keep it only if it splits a link
    for nid in &touched {
        let n = sc.nodes.get_mut(nid).expect("a touched node");
        if n.control.is("signals") && deg.get(nid).copied().unwrap_or(0) < 2 {
            n.control = Maybe::Set("none".to_string());
        }
    }
    if opts.sites && added > 0 {
        sites::detect(
            sc,
            ids,
            &DetectOpts {
                points: opts.points.clone(),
                ..Default::default()
            },
        );
    }
    added
}

/// `Importers.autoGateways(sc, b, only)`: external zones (gateways) where roads leave the imported area, loaded with indicative
/// through traffic only. `only` limits it to those dead-end nodes (for instance the nodes one import added), so roads imported
/// earlier somewhere else never get gateways for this area. A dead end of collector rank or higher, within 5% of the box's
/// edge and not closed, gets a zone 60 m beyond it with flows from the road class and lanes. Returns how many were made.
pub fn auto_gateways(
    sc: &mut Scenario,
    ids: &mut dyn IdSource,
    b: &Bounds,
    only: Option<&HashSet<String>>,
) -> usize {
    // The links at each node, in the order the links first reach them
    let mut adj: Vec<(String, Vec<String>)> = Vec::new();
    let mut adj_ix: HashMap<String, usize> = HashMap::new();
    for l in sc.links.values() {
        for n in [&l.a, &l.b] {
            let k = *adj_ix.entry(n.clone()).or_insert_with(|| {
                adj.push((n.clone(), Vec::new()));
                adj.len() - 1
            });
            adj[k].1.push(l.id.clone());
        }
    }
    let w = b.east - b.west;
    let h = b.north - b.south;
    let (mx, my) = (w * 0.05, h * 0.05);
    let mut pinned: HashSet<String> = sc
        .zones
        .values()
        .filter_map(|z| {
            z.node
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
        .collect();
    let mut made = 0usize;
    for (nid, ls) in &adj {
        if ls.len() != 1 || pinned.contains(nid) {
            continue;
        }
        let Some(n) = sc.nodes.get(nid) else {
            continue;
        };
        let l = &sc.links[ls[0].as_str()];
        let c = road_class(l.cls.as_str());
        if c.rank < 3.0 || l.status.is("closed") {
            continue;
        }
        if let Some(o) = only
            && !o.contains(nid)
        {
            continue;
        }
        let (nlat, nlng) = (n.lat.num(), n.lng.num());
        let near =
            nlng - b.west < mx || b.east - nlng < mx || nlat - b.south < my || b.north - nlat < my;
        if !near {
            continue;
        }
        let at_a = l.a == *nid;
        let (in_lanes, out_lanes) = if at_a {
            (l.lanes_ab.num(), l.lanes_ba.num())
        } else {
            (l.lanes_ba.num(), l.lanes_ab.num())
        };
        let per = match l.cls.as_str() {
            Some("motorway") => 1500.0,
            Some("ramp") => 650.0,
            Some("arterial") => 550.0,
            Some("collector") => 320.0,
            _ => 250.0,
        };
        let inb = math_round(in_lanes * per * 0.85 / 10.0) * 10.0;
        let outb = math_round(out_lanes * per * 0.85 / 10.0) * 10.0;
        // place the gateway marker 60 m beyond the road end
        let g = sc.geom(l);
        let far = if at_a { g[1] } else { g[g.len() - 2] };
        let d_lat = nlat - far[0];
        let d_lng = nlng - far[1];
        let hyp = math_hypot(&[
            d_lat,
            d_lng * crate::jsmath::cos(nlat * std::f64::consts::PI / 180.0),
        ]);
        let m = if hyp != 0.0 && !hyp.is_nan() {
            hyp
        } else {
            1.0
        };
        let k = 60.0 / 111000.0 / m;
        let label = l.name.str_or(c.label).to_string();
        let lat = r7(nlat + d_lat * k);
        let lng = r7(nlng + d_lng * k);
        let id = ids.uid("z");
        let gw: OrderedMap<Num> = [
            ("AMin", inb),
            ("AMout", outb),
            ("PMin", inb),
            ("PMout", outb),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), Num::number(v)))
        .collect();
        let z = Zone {
            id: id.clone(),
            kind: Maybe::Set("gateway".to_string()),
            name: Maybe::Set(format!("{label} gateway")),
            lat: Num::number(lat),
            lng: Num::number(lng),
            lu: Maybe::Missing,
            connectors: Maybe::Missing,
            role: Maybe::Missing,
            year: Maybe::Missing,
            buildout: Maybe::Missing,
            existing: Maybe::Missing,
            src: Maybe::Missing,
            node: Maybe::Set(nid.clone()),
            gw: Maybe::Set(gw),
            gw_src: Maybe::Missing,
            tg: Maybe::Missing,
            tg_load: Maybe::Missing,
            extra_trips: Maybe::Missing,
            reduction: Maybe::Missing,
            reg: Maybe::Missing,
            extra: Map::new(),
        };
        sc.zones.insert(id, z);
        pinned.insert(nid.clone());
        made += 1;
    }
    made
}

/// `parseInt` of the text of a tag, for callers that already hold it as text.
pub fn parse_tag_int(s: &str) -> f64 {
    parse_int10(s)
}
