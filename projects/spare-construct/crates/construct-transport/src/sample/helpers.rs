//! The small steps the sample build is made of: finding roads and junction sites by name, the made-up turning count, lane coding, cutting OpenStreetMap
//! roads at the edge of the study area, leaving out dead ends, simplifying road shapes and the two network options.

use super::hooks::SampleHooks;
use super::{BIN, ILLUSTRATIVE, PERIODS, turn_vol, window};
use crate::census::Bx;
use crate::datahub::{Data, matches};
use crate::edit::IdSource;
use crate::edit::net::{LinkProps, add_link, auto_control, delete_link, links_at, nearest_node};
use crate::graph::{connector_map, js_sort_by};
use crate::jsnum::{Maybe, Num, f64_to_i64, f64_to_usize, math_max, math_round, number_to_js_string, usize_to_f64, value_to_js_string, value_to_number};
use crate::project::{Point, Scenario, Settings};
use crate::util::{self, LatLng};
use serde_json::{Map, Value, json};
use std::collections::HashSet;

/// `names(sc, id)`: the names of the roads at a junction node, each once, joined with " | " (a road with no name is an empty name).
#[must_use]
pub fn names(sc: &Scenario, id: &str) -> String {
    let mut seen: Vec<&str> = Vec::new();
    for l in links_at(sc, id) {
        let nm = l.name.as_str().unwrap_or("");
        if !seen.contains(&nm) {
            seen.push(nm);
        }
    }
    seen.join(" | ")
}

// `new RegExp(word, 'i').test(text)` for a plain word (the sample only searches for road names)
fn has_word(text: &str, word: &str) -> bool {
    text.to_lowercase().contains(&word.to_lowercase())
}

/// `nodeOn(sc, ll, road, maxM)`: the junction node nearest to a point, within `max_m` metres, on a road whose name has `road` in it (a plain word, any case).
#[must_use]
pub fn node_on(sc: &Scenario, ll: LatLng, road: &str, max_m: f64) -> Option<String> {
    let mut ids: HashSet<&str> = HashSet::new();
    for l in sc.links.values() {
        if has_word(l.name.as_str().unwrap_or(""), road) {
            ids.insert(&l.a);
            ids.insert(&l.b);
        }
    }
    nearest_node(sc, ll, max_m, Some(&|n: &crate::project::Node| ids.contains(n.id.as_str())))
}

/// A junction site `site_with` found and where its middle is.
#[derive(Clone, Debug, PartialEq)]
pub struct SiteHit {
    /// The site's id.
    pub site: String,
    /// The middle of the site's nodes, `[lat, lng]`.
    pub c: LatLng,
}

/// `siteWith(sc, pats, ll)`: the junction site (`Sites.detect`) whose roads include every word, nearest to the point.
#[must_use]
pub fn site_with(sc: &Scenario, pats: &[&str], ll: LatLng) -> Option<SiteHit> {
    let sites = sc.sites.get()?;
    let mut list: Vec<SiteHit> = Vec::new();
    for s in sites.values() {
        let t = s.nodes.iter().map(|id| names(sc, id)).collect::<Vec<_>>().join(" | ");
        if !pats.iter().all(|p| has_word(&t, p)) {
            continue;
        }
        let n: Vec<&crate::project::Node> = s.nodes.iter().filter_map(|id| sc.nodes.get(id)).collect();
        let (mut sa, mut so) = (0.0, 0.0);
        for x in &n {
            sa += x.lat.num();
        }
        for x in &n {
            so += x.lng.num();
        }
        list.push(SiteHit { site: s.id.clone(), c: [sa / usize_to_f64(n.len()), so / usize_to_f64(n.len())] });
    }
    js_sort_by(&mut list, |a, b| util::dist(&a.c, &ll) - util::dist(&b.c, &ll));
    list.into_iter().next()
}

/// `compass(b)`: the compass side N, E, S or W a bearing points to (the prototype's "undefined" for a bearing that is not a number).
#[must_use]
pub fn compass(b: f64) -> &'static str {
    const COMPASS: [&str; 4] = ["N", "E", "S", "W"];
    let i = math_round((((b % 360.0) + 360.0) % 360.0) / 90.0) % 4.0;
    // i is a whole number from 0 to 3 here, so the side is always there
    if i.is_nan() { "undefined" } else { COMPASS.get(f64_to_usize(i)).copied().unwrap_or("undefined") }
}

/// What `turn_count_csv` is asked for: the name to print (else the site's, else "junction") and the date of the count (default 2026-03-10).
#[derive(Clone, Debug, Default)]
pub struct CsvOpts {
    /// The junction's name.
    pub name: Option<String>,
    /// The count date.
    pub date: Option<String>,
}

/// An illustrative turning-count file.
#[derive(Clone, Debug, PartialEq)]
pub struct TurnCsv {
    /// The long-format CSV.
    pub csv: String,
    /// The road leaving the site on each compass side, in the order they were met.
    pub legs: Vec<(String, String)>,
    /// The middle of the site, `[lat, lng]`.
    pub centre: LatLng,
    /// The count's name, with the label that it is illustrative.
    pub name: String,
}

/// `turnCountCSV(sc, site, opts)`: an illustrative long-format turning-count file for a junction site (calibration feature layout): legs from the site's roads by
/// compass side, 15-minute light and heavy counts for 07:00 to 09:00 and 16:00 to 18:00.
pub fn turn_count_csv(sc: &Scenario, site: &crate::project::Site, opts: &CsvOpts) -> TurnCsv {
    let ms: HashSet<&str> = site.nodes.iter().map(String::as_str).collect();
    let pts: Vec<&crate::project::Node> = site.nodes.iter().filter_map(|id| sc.nodes.get(id)).collect();
    let (mut sa, mut so) = (0.0, 0.0);
    for nd in &pts {
        sa += nd.lat.num();
    }
    for nd in &pts {
        so += nd.lng.num();
    }
    let centre: LatLng = [sa / usize_to_f64(pts.len()), so / usize_to_f64(pts.len())];
    // each road leaving the site, by the compass side it leaves on (the bearing of its first segment)
    let mut legs: Vec<(String, String)> = Vec::new();
    for link in sc.links.values() {
        let nm = link.name.as_str().unwrap_or("");
        if link.status.is("closed") || ms.contains(link.a.as_str()) == ms.contains(link.b.as_str()) || nm.is_empty() || link.ring.truthy() {
            continue;
        }
        let geom = sc.geom(link);
        // a link's shape runs from its first node to its last, so it has two points at least; one without is left out
        let ends = if ms.contains(link.a.as_str()) {
            geom.first().zip(geom.get(1))
        } else {
            geom.last().zip(geom.len().checked_sub(2).and_then(|at| geom.get(at)))
        };
        let Some((from, to)) = ends else { continue };
        let heading = compass(util::bearing(from, to));
        if !legs.iter().any(|(have, _)| have == heading) {
            legs.push((heading.to_string(), nm.to_string()));
        }
    }
    let hv = 0.06;
    let shown = opts.name.as_deref().filter(|s| !s.is_empty()).or(site.name.as_str().filter(|s| !s.is_empty())).unwrap_or("junction");
    let name = format!("{ILLUSTRATIVE}: {shown}");
    let date = opts.date.as_deref().filter(|s| !s.is_empty()).unwrap_or("2026-03-10");
    let text = |txt: &str| Value::String(txt.to_string());
    let mut rows: Vec<Vec<Value>> = vec![
        ["site_id", "site_name", "lat", "lng", "date", "time_start", "interval_min", "from_leg", "from_road", "movement", "class", "count"].iter().map(|head| text(head)).collect(),
    ];
    let (lat7, lng7) = (util::r7(centre[0]), util::r7(centre[1]));
    for period in PERIODS {
        for (i, bin) in BIN.iter().enumerate() {
            let minute = window(period) + 15.0 * usize_to_f64(i);
            let hh = format!("{:02}:{:02}", f64_to_i64((minute / 60.0).floor()), f64_to_i64(minute % 60.0));
            for (leg, mv) in turn_vol(period) {
                let Some((_, road)) = legs.iter().find(|(side, _)| side == leg) else { continue };
                for (movement, vol) in mv {
                    let n = vol * bin;
                    let heavy = math_round(n * hv);
                    let light = math_max(0.0, math_round(n) - heavy);
                    for (class, count) in [("LV", light), ("HV", heavy)] {
                        rows.push(vec![
                            text("SAMPLE-1"),
                            text(&name),
                            crate::jsnum::json_number(lat7),
                            crate::jsnum::json_number(lng7),
                            text(date),
                            text(&hh),
                            json!(15),
                            text(leg),
                            text(road),
                            text(movement),
                            text(class),
                            crate::jsnum::json_number(count),
                        ]);
                    }
                }
            }
        }
    }
    TurnCsv { csv: util::csv(&rows), legs, centre, name }
}

/// `codeLanes(sc, site, JN)`: illustrative lane coding on every approach into a junction site: a kerb lane for left and through and a right lane for through and
/// right; a one-lane road gets a 40 m flare for the right lane. Stored on the site (junction tools).
pub fn code_lanes(sc: &mut Scenario, site_id: &str, hooks: &mut dyn SampleHooks) {
    const SRC: &str = "illustrative (sample)";
    let Some(nodes) = sc.sites.get().and_then(|m| m.get(site_id)).map(|s| s.nodes.clone()) else { return };
    let ms: HashSet<&str> = nodes.iter().map(String::as_str).collect();
    if let Some(site) = sc.sites.get_mut().and_then(|m| m.get_mut(site_id))
        && !site.lanes.is_set()
    {
        site.lanes = Maybe::Set(crate::project::OrderedMap::new());
    }
    for nid in &nodes {
        for ap in hooks.approaches(sc, nid) {
            let Some(l) = sc.links.get(&ap.link) else { continue };
            if ms.contains(l.a.as_str()) && ms.contains(l.b.as_str()) {
                continue;
            }
            // the site was found and given its lanes record above, so both are there
            let Some(site) = sc.sites.get_mut().and_then(|m| m.get_mut(site_id)) else { continue };
            let Some(lanes) = site.lanes.get_mut() else { continue };
            // an empty record on the site first, so the lanes are stored on the junction site rather than the node
            if !lanes.get(&ap.link).is_some_and(crate::jsnum::value_truthy) {
                lanes.insert(ap.link.clone(), json!({ "lanes": [], "src": SRC }));
            }
            // `Junctions.editLanes`: the lanes of the record, then who set them (the user); the sample then says it is illustrative
            let mut cur = lanes.get(&ap.link).cloned().unwrap_or(Value::Null);
            if let Some(o) = cur.as_object_mut() {
                o.insert("lanes".into(), json!([{ "d": "LT", "len": 0 }, { "d": "TR", "len": if ap.gl >= 2.0 { 0 } else { 40 } }]));
                o.insert("src".into(), json!("user"));
                let mut by = o.get("srcBy").and_then(Value::as_object).cloned().unwrap_or_default();
                by.insert("lanes".into(), json!("user"));
                by.insert("len".into(), json!("user"));
                o.insert("srcBy".into(), Value::Object(by));
                o.insert("src".into(), json!(SRC));
                o.insert("srcBy".into(), json!({ "lanes": SRC, "len": SRC }));
            }
            lanes.insert(ap.link.clone(), cur);
        }
    }
}

/// What `clip_ways` made.
#[derive(Clone, Debug, PartialEq)]
pub struct Clipped {
    /// The elements: everything that is not a way, the ways cut at the edge, then the new nodes on the edge.
    pub elements: Vec<Value>,
    /// How many ways were cut.
    pub cut: usize,
}

// A Map key for an OpenStreetMap id (a number or a string), the way `Map` tells them apart
fn id_key(v: &Value) -> String {
    match v {
        Value::Number(n) => format!("n{}", number_to_js_string(n.as_f64().unwrap_or(f64::NAN))),
        Value::String(s) => format!("s{s}"),
        other => format!("o{other}"),
    }
}

fn in_box(b: &Bx, lat: f64, lon: f64) -> bool {
    lat >= b.s && lat <= b.n && lon >= b.w && lon <= b.e
}

fn node_in_box(b: &Bx, o: Option<&Value>) -> bool {
    match o {
        Some(o) => {
            let g = |k: &str| o.get(k).map_or(f64::NAN, value_to_number);
            in_box(b, g("lat"), g("lon"))
        }
        None => false,
    }
}

/// `clipWays(elements, B)`: cuts OpenStreetMap ways (Overpass `out body; >; out body qt`) where they leave the box: each run of nodes inside becomes its own way,
/// ending at a new node on the edge.
pub fn clip_ways(elements: &[Value], b: &Bx) -> Clipped {
    let mut nodes: std::collections::HashMap<String, &Value> = std::collections::HashMap::new();
    for el in elements {
        if el.get("type").and_then(Value::as_str) == Some("node") {
            nodes.insert(id_key(el.get("id").unwrap_or(&Value::Null)), el);
        }
    }
    let mut out: Vec<Value> = elements.iter().filter(|el| el.get("type").and_then(Value::as_str) != Some("way")).cloned().collect();
    let mut extra: Vec<Value> = Vec::new();
    // Bound: counts of ways and of new nodes in memory, far below usize::MAX
    let (mut cut, mut made) = (0usize, 0usize);
    let num = |obj: &Value, key: &str| obj.get(key).map_or(f64::NAN, value_to_number);
    // the point where inside to outside crosses the edge
    let mut edge = |inside: &Value, outside: &Value| -> Value {
        let (plat, plon, qlat, qlon) = (num(inside, "lat"), num(inside, "lon"), num(outside, "lat"), num(outside, "lon"));
        let (mut lo, mut hi) = (0.0f64, 1.0f64);
        for _ in 0..30 {
            let mid = f64::midpoint(lo, hi);
            if in_box(b, plat + (qlat - plat) * mid, plon + (qlon - plon) * mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        made = made.saturating_add(1);
        let id = format!("edge{made}");
        let node = json!({ "type": "node", "id": id, "lat": crate::jsnum::json_number(util::r7(plat + (qlat - plat) * lo)), "lon": crate::jsnum::json_number(util::r7(plon + (qlon - plon) * lo)) });
        extra.push(node);
        Value::String(id)
    };
    for way in elements {
        if way.get("type").and_then(Value::as_str) != Some("way") {
            continue;
        }
        let ids: Vec<&Value> = way.get("nodes").and_then(Value::as_array).map(|arr| arr.iter().filter(|id| nodes.contains_key(&id_key(id))).collect()).unwrap_or_default();
        let ins: Vec<bool> = ids.iter().map(|id| node_in_box(b, nodes.get(&id_key(id)).copied())).collect();
        if ins.iter().all(|inb| *inb) {
            out.push(way.clone());
            continue;
        }
        cut = cut.saturating_add(1);
        // the node at a position of the way (every id in ids has a node: ids was filtered on it)
        let node_at = |at: usize| ids.get(at).and_then(|id| nodes.get(&id_key(id))).copied();
        // ins has one flag for each id; positions stay below ids.len(), so the steps never saturate
        let (mut start, mut piece_no) = (0usize, 0usize);
        while start < ids.len() {
            if ins.get(start) != Some(&true) {
                start = start.saturating_add(1);
                continue;
            }
            let mut end = start;
            while ins.get(end.saturating_add(1)) == Some(&true) {
                end = end.saturating_add(1);
            }
            let mut run: Vec<Value> = ids.get(start..=end).unwrap_or_default().iter().map(|val| (*val).clone()).collect();
            if let Some(prev) = start.checked_sub(1)
                && let (Some(inside), Some(outside)) = (node_at(start), node_at(prev))
            {
                let el = edge(inside, outside);
                run.insert(0, el);
            }
            if let Some(next) = end.checked_add(1).filter(|nx| *nx < ids.len())
                && let (Some(inside), Some(outside)) = (node_at(end), node_at(next))
            {
                let el = edge(inside, outside);
                run.push(el);
            }
            if run.len() > 1 {
                let mut piece = way.as_object().cloned().unwrap_or_default();
                let id = if piece_no > 0 { Value::String(format!("{}-{}", value_to_js_string(way.get("id").unwrap_or(&Value::Null)), piece_no)) } else { way.get("id").cloned().unwrap_or(Value::Null) };
                piece.insert("id".into(), id);
                piece.insert("nodes".into(), Value::Array(run));
                out.push(Value::Object(piece));
            }
            piece_no = piece_no.saturating_add(1);
            start = end.saturating_add(1);
        }
    }
    out.extend(extra);
    Clipped { elements: out, cut }
}

/// `pruneStubs(sc)`: removes dead-end chains that cannot carry traffic: a road end (one link) that is not a zone connector or gateway node, not in a junction site,
/// not on a bus route, and whose link has no count matched to it. Repeats inward along the chain. Exact for the assignment (no path can use a dead end it does not
/// start or end in). Gives how many links were removed.
pub fn prune_stubs(sc: &mut Scenario, s: &Settings, data: &Data) -> usize {
    let mut keep_n: HashSet<String> = HashSet::new();
    let mut keep_l: HashSet<String> = HashSet::new();
    for ids in connector_map(sc, s).values() {
        for id in ids {
            keep_n.insert(id.clone());
        }
    }
    for z in sc.zones.values() {
        if let Some(n) = z.node.as_str().filter(|n| !n.is_empty()) {
            keep_n.insert(n.to_string());
        }
    }
    if let Some(sites) = sc.sites.get() {
        for st in sites.values() {
            for id in &st.nodes {
                keep_n.insert(id.clone());
            }
        }
    }
    for r in sc.routes.values() {
        for id in r.wp.get().map_or(&[][..], Vec::as_slice) {
            keep_n.insert(id.clone());
        }
    }
    for mt in matches(sc, s, data).values() {
        keep_l.insert(mt.link.clone());
    }
    // Bound: a count of links in memory, far below usize::MAX
    let mut n = 0usize;
    let mut again = true;
    while again {
        again = false;
        // `Net.degree(sc)`: how many link ends meet at each node, in the order the nodes are first met
        let mut order: Vec<(String, usize)> = Vec::new();
        let mut at: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for l in sc.links.values() {
            for id in [&l.a, &l.b] {
                let i = *at.entry(id.clone()).or_insert_with(|| {
                    order.push((id.clone(), 0));
                    // just pushed, so the list is not empty
                    order.len().saturating_sub(1)
                });
                // Bound: a count of link ends in memory, far below usize::MAX
                if let Some(slot) = order.get_mut(i) {
                    slot.1 = slot.1.saturating_add(1);
                }
            }
        }
        for (id, d) in order {
            if d != 1 || keep_n.contains(&id) {
                continue;
            }
            let first = links_at(sc, &id).first().map(|l| (l.id.clone(), l.status.is("existing")));
            match first {
                Some((lid, existing)) if existing && !keep_l.contains(&lid) => {
                    delete_link(sc, &lid);
                    n = n.saturating_add(1);
                    again = true;
                }
                _ => {
                    keep_n.insert(id);
                }
            }
        }
    }
    n
}

/// `simplify(sc, tol)`: Douglas-Peucker simplification of each link's shape points to within `tol` metres (lengths change by well under 0.1%). Gives how many
/// shape points were dropped.
pub fn simplify(sc: &mut Scenario, tol: f64) -> usize {
    let mut dropped = 0usize;
    let ids: Vec<String> = sc.links.keys().map(str::to_string).collect();
    for id in ids {
        let Some(l) = sc.links.get(id.as_str()) else {
            continue;
        };
        let geom = sc.geom(l);
        if geom.len() < 3 {
            continue;
        }
        let Some(start) = geom.first() else { continue };
        // the shape has three points or more, so it has a last one
        let last = geom.len().saturating_sub(1);
        let proj = util::flat(start[0]);
        let xy: Vec<[f64; 2]> = geom.iter().map(|pt| [proj.x(pt), proj.y(pt)]).collect();
        let mut keep = vec![false; geom.len()];
        if let Some(slot) = keep.first_mut() {
            *slot = true;
        }
        if let Some(slot) = keep.last_mut() {
            *slot = true;
        }
        let mut stack: Vec<(usize, usize)> = vec![(0, last)];
        while let Some((lo, hi)) = stack.pop() {
            // lo and hi are positions in the shape (lo below hi), so their points are there
            let (Some(at_lo), Some(at_hi)) = (xy.get(lo), xy.get(hi)) else { continue };
            let mut far: Option<usize> = None;
            let mut dm = tol;
            for mid in lo.saturating_add(1)..hi {
                let Some(at_mid) = xy.get(mid) else { continue };
                let d = util::seg_dist(at_mid[0], at_mid[1], at_lo[0], at_lo[1], at_hi[0], at_hi[1]).d;
                if d > dm {
                    dm = d;
                    far = Some(mid);
                }
            }
            if let Some(far) = far {
                if let Some(slot) = keep.get_mut(far) {
                    *slot = true;
                }
                stack.push((lo, far));
                stack.push((far, hi));
            }
        }
        // the points between the two ends, each with its own keep flag (the flags of the ends are skipped)
        let inner = geom.get(1..last).unwrap_or_default();
        let pts: Vec<Point> = inner.iter().zip(keep.iter().skip(1)).filter(|(_, kept)| **kept).map(|(pt, _)| Point(*pt)).collect();
        let before = sc.links.get(id.as_str()).and_then(|l| l.pts.get()).map_or(0, Vec::len);
        // Bound: simplifying only drops points, so the new shape is never longer than the old one
        dropped = dropped.saturating_add(before.saturating_sub(pts.len()));
        if let Some(link) = sc.links.get_mut(&id) {
            link.pts = Maybe::Set(pts);
        }
    }
    dropped
}

/// `optionA(sc)`: Option A, East links: a collector loop to Appleby Road and an arterial north to Sutton Road (proposal links). Gives the ids of the links made.
pub fn option_a(sc: &mut Scenario, ids: &mut dyn IdSource) -> Vec<String> {
    let s = node_on(sc, [-37.0988, 174.9600], "Takotowhenua", 600.0);
    let e = node_on(sc, [-37.1006, 174.9722], "Appleby", 600.0);
    let n2 = node_on(sc, [-37.0936, 174.9552], "Sutton", 600.0);
    let mut made: Vec<String> = Vec::new();
    let props = |cls: &str, name: &str| {
        let mut p = LinkProps::new().cls(cls).lanes(1.0, 1.0).speed(50.0).status("proposed").name(name).cycle("separated");
        p.0.stage = Maybe::Set("proposal".to_string());
        p
    };
    if let (Some(s), Some(e)) = (&s, &e) {
        made.push(add_link(
            sc,
            ids,
            s,
            e,
            &[[-37.0983, 174.9628], [-37.0981, 174.9660], [-37.0987, 174.9692], [-37.0997, 174.9710]],
            &props("collector", "Drury East Link (Option A, illustrative)"),
        ));
    }
    if let (Some(s), Some(n2)) = (&s, &n2) {
        made.push(add_link(sc, ids, s, n2, &[[-37.0968, 174.9590], [-37.0950, 174.9570]], &props("arterial", "Takotowhenua Avenue extension (Option A, illustrative)")));
    }
    let list: Vec<String> = [&s, &e, &n2].into_iter().flatten().cloned().collect();
    auto_control(sc, &list);
    if let Some(s) = &s
        && made.len() == 2
        && let Some(n) = sc.nodes.get_mut(s)
    {
        n.control = Maybe::Set("roundabout".to_string());
        n.upgrade = true.into();
    }
    made
}

/// `optionB(sc)`: Option B, Waihoehoe Road four-laned between Great South Road and Takotowhenua Avenue (mitigation stage). Gives how many links were widened.
pub fn option_b(sc: &mut Scenario) -> usize {
    let mut n = 0usize;
    let ids: Vec<String> = sc.links.keys().map(str::to_string).collect();
    for id in ids {
        let Some(l) = sc.links.get(id.as_str()) else {
            continue;
        };
        if !has_word(l.name.as_str().unwrap_or(""), "waihoehoe") || l.ring.truthy() {
            continue;
        }
        let (Some(a), Some(b)) = (sc.nodes.get(&l.a), sc.nodes.get(&l.b)) else { continue };
        if math_max(a.lng.num(), b.lng.num()) > 174.9600 || crate::jsnum::math_min(a.lng.num(), b.lng.num()) < 174.9519 {
            continue;
        }
        // the link was found above, under the same id
        let Some(l) = sc.links.get_mut(&id) else { continue };
        l.lanes_ab = Num::number(math_max(2.0, l.lanes_ab.num()));
        l.lanes_ba = Num::number(math_max(2.0, l.lanes_ba.num()));
        l.cls = Maybe::Set("arterial".to_string());
        l.speed = Num::number(50.0);
        l.status = Maybe::Set("proposed".to_string());
        l.stage = Maybe::Set("mitigation".to_string());
        // Bound: a count of links in memory, far below usize::MAX
        n = n.saturating_add(1);
    }
    n
}

/// The object a JSON value is, made when it is not one (`x ||= {}` on a project key).
///
/// Always `Some`: the key holds an object once this returns. It is an `Option` only because a JSON value's type cannot say so.
pub(crate) fn object_at<'a>(m: &'a mut Map<String, Value>, key: &str) -> Option<&'a mut Map<String, Value>> {
    let v = m.entry(key.to_string()).or_insert_with(|| Value::Object(Map::new()));
    if !v.is_object() {
        *v = Value::Object(Map::new());
    }
    v.as_object_mut()
}
