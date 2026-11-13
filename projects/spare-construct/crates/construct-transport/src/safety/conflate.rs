//! Future Connect classes put on model links (`conflateFC`).
//!
//! A link takes the classes of the section whose polyline lies within the radius (15 m) of at least two of three
//! points along the link (a quarter, a half, three quarters), with the section's bearing there within 30 degrees of
//! the link's (either direction), and whose street name matches the link's where both have one. Rail-only sections
//! are skipped. A class the user set (`fc.user`) is never replaced. Short pieces (under 60 m) at junctions often
//! miss: they take the classes of an adjacent matched link of the same road, and a roundabout ring link takes the
//! highest general-traffic class of the roads meeting the ring.

use crate::datahub::norm;
use crate::feeds::nztime::utc_date;
use crate::jsnum::{f64_to_i64, f64_to_usize, math_max, math_min, usize_to_f64, value_truthy};
use crate::project::Scenario;
use crate::util::{self, Flat, LatLng, ang_diff, bearing, flat};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::HashMap;

/// One AT Future Connect section (`loadFutureConnect` entry): its classes now and in the first decade, and its polyline.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FutureConnect {
    /// The section's OBJECTID.
    pub id: Value,
    /// Street name ("" when none).
    pub name: String,
    /// General traffic class now.
    pub gt: Option<String>,
    /// Public transport class now.
    pub pt: Option<String>,
    /// Freight class now.
    pub freight: Option<String>,
    /// Cycle class now.
    pub cycle: Option<String>,
    /// General traffic class in the first decade.
    pub gt10: Option<String>,
    /// Public transport class in the first decade.
    pub pt10: Option<String>,
    /// Freight class in the first decade.
    pub freight10: Option<String>,
    /// Cycle class in the first decade.
    pub cycle10: Option<String>,
    /// The polylines, each a list of [lat, lng].
    pub paths: Vec<Vec<LatLng>>,
}

/// What `conflateFC` reports: how many links were classified and how many were arterials.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflateResult {
    /// Links classified.
    pub matched: usize,
    /// Links considered (not closed, not set by the user).
    pub links: usize,
    /// Arterial links considered.
    pub arterials: usize,
    /// Arterial links classified.
    pub arterials_matched: usize,
    /// matched / links (0 with no links).
    pub share: f64,
    /// arterialsMatched / arterials (null with no arterials).
    pub arterial_share: Option<f64>,
}

fn truthy(s: Option<&str>) -> bool {
    s.is_some_and(|x| !x.is_empty())
}

fn opt(s: Option<&str>) -> Value {
    match s {
        Some(x) => json!(x),
        None => Value::Null,
    }
}

// order of general-traffic classes, highest first
const GT_ORDER: [&str; 6] = ["motorway", "strategic", "primary", "secondary", "collector", "local"];

fn gt_rank(g: &Value) -> f64 {
    let text = if value_truthy(g) { crate::jsnum::value_to_js_string(g) } else { String::new() };
    let n = norm(&text);
    match GT_ORDER.iter().position(|x| n.starts_with(x)) {
        Some(k) => usize_to_f64(k),
        None => 9.0,
    }
}

fn cell_key(cx: f64, cy: f64) -> i64 {
    f64_to_i64(cx * 1e6 + cy)
}

// Grid cell size (m)
const CELL_M: f64 = 60.0;

fn name_ok(a: &str, b: &str) -> bool {
    let (x, y) = (norm(a), norm(b));
    x.is_empty() || y.is_empty() || x == y || x.contains(&y) || y.contains(&x)
}

// The Future Connect sections in use, their segments in a grid of flat cells, and the matching limits
struct SegIndex<'a> {
    use_: Vec<&'a FutureConnect>,
    fl: Flat,
    // [ax, ay, bx, by, feature, bearing]
    segs: Vec<[f64; 6]>,
    grid: HashMap<i64, Vec<usize>>,
    radius: f64,
    b_lim: f64,
}

impl<'a> SegIndex<'a> {
    fn new(use_: Vec<&'a FutureConnect>, fl: Flat, radius: f64, b_lim: f64) -> Self {
        let mut grid: HashMap<i64, Vec<usize>> = HashMap::new();
        let mut segs: Vec<[f64; 6]> = Vec::new();
        for (fi, feat) in use_.iter().enumerate() {
            for path in &feat.paths {
                for pair in path.windows(2) {
                    let [pa, pb] = pair else { continue };
                    let (ax, ay, bx, by) = (fl.x(pa), fl.y(pa), fl.x(pb), fl.y(pb));
                    let k = segs.len();
                    segs.push([ax, ay, bx, by, usize_to_f64(fi), bearing(pa, pb)]);
                    let mut cx = ((math_min(ax, bx) - radius) / CELL_M).floor();
                    while cx <= ((math_max(ax, bx) + radius) / CELL_M).floor() {
                        let mut cy = ((math_min(ay, by) - radius) / CELL_M).floor();
                        while cy <= ((math_max(ay, by) + radius) / CELL_M).floor() {
                            grid.entry(cell_key(cx, cy)).or_default().push(k);
                            cy += 1.0;
                        }
                        cx += 1.0;
                    }
                }
            }
        }
        SegIndex { use_, fl, segs, grid, radius, b_lim }
    }

    // votes: feature to (count, total distance), in the order features were first seen
    fn votes(&self, geom: &[LatLng], cum: &[f64], len: f64) -> Vec<(usize, f64, f64)> {
        let mut votes: Vec<(usize, f64, f64)> = Vec::new();
        for fr in [0.25, 0.5, 0.75] {
            let at = util::along(geom, cum, len * fr);
            let b0 = bearing(
                &util::along(geom, cum, math_max(0.0, len * fr - 5.0)).p,
                &util::along(geom, cum, math_min(len, len * fr + 5.0)).p,
            );
            let (px, py) = (self.fl.x(&at.p), self.fl.y(&at.p));
            let mut best: Vec<(usize, f64)> = Vec::new();
            if let Some(cell) = self.grid.get(&cell_key((px / CELL_M).floor(), (py / CELL_M).floor())) {
                for &k in cell {
                    // The grid holds only positions of segments already pushed
                    let Some(q) = self.segs.get(k) else { continue };
                    let dist = util::seg_dist(px, py, q[0], q[1], q[2], q[3]).d;
                    let ang = ang_diff(b0, q[5]);
                    if dist > self.radius || math_min(ang, 180.0 - ang) > self.b_lim {
                        continue;
                    }
                    let fi = f64_to_usize(q[4]);
                    match best.iter_mut().find(|(seen, _)| *seen == fi) {
                        Some(e) => {
                            // !(best.get(fi) <= d): a NaN on either side replaces it too
                            if e.1 > dist || e.1.is_nan() || dist.is_nan() {
                                e.1 = dist;
                            }
                        }
                        None => best.push((fi, dist)),
                    }
                }
            }
            for (fi, dist) in best {
                match votes.iter_mut().find(|(seen, _, _)| *seen == fi) {
                    Some(v) => {
                        v.1 += 1.0;
                        v.2 += dist;
                    }
                    None => votes.push((fi, 1.0, dist)),
                }
            }
        }
        votes
    }

    // The section a link takes: most votes (at least two), then the least total distance, with a matching name
    fn pick(&self, votes: &[(usize, f64, f64)], name: &str) -> Option<(&'a FutureConnect, f64, f64)> {
        let mut pick: Option<(&'a FutureConnect, f64, f64)> = None;
        for &(fi, n, d) in votes {
            // fi is always a position in use_, as the segments were built from it
            let Some(&feat) = self.use_.get(fi) else { continue };
            if n >= 2.0 && name_ok(name, &feat.name) && pick.is_none_or(|p| n > p.1 || (n == p.1 && d < p.2)) {
                pick = Some((feat, n, d));
            }
        }
        pick
    }
}

/// `conflateFC(sc, feats, opts)`: puts Future Connect classes on a scenario's links (`link.fc`), and returns how many matched.
///
/// `radius` (default 15 m) and `bearing` (default 30 degrees) are the options of the prototype, where 0 or a missing value
/// gives the default. `now_ms` stamps `fc.at` with the date (UTC). Links with an `fc` that has no Future Connect source are
/// left; a link that lost its match loses an earlier Future Connect class.
pub fn conflate_fc(sc: &mut Scenario, feats: &[FutureConnect], radius: Option<f64>, bearing_limit: Option<f64>, now_ms: f64) -> ConflateResult {
    let option_or = |v: Option<f64>, d: f64| match v {
        Some(x) if x != 0.0 && !x.is_nan() => x,
        _ => d,
    };
    let radius_m = option_or(radius, 15.0);
    let b_lim = option_or(bearing_limit, 30.0);
    let now = utc_date(now_ms);
    let use_: Vec<&FutureConnect> = feats
        .iter()
        .filter(|f| truthy(f.gt.as_deref()) || truthy(f.pt.as_deref()) || truthy(f.gt10.as_deref()) || truthy(f.pt10.as_deref()) || (truthy(f.freight.as_deref()) && f.freight.as_deref() != Some("Rail")))
        .collect();
    let lat0 = sc.nodes.values().next().map_or(-36.85, |n| n.lat.num());
    let index = SegIndex::new(use_, flat(lat0), radius_m, b_lim);
    // Bound: these count links in memory, far below usize::MAX
    let (mut matched, mut considered, mut art, mut art_m) = (0usize, 0usize, 0usize, 0usize);
    let ids: Vec<String> = sc.links.keys().map(str::to_string).collect();
    for id in &ids {
        let Some(link) = sc.links.get(id.as_str()) else {
            continue;
        };
        if link.status.is("closed") || !sc.nodes.contains_key(&link.a) || !sc.nodes.contains_key(&link.b) {
            continue;
        }
        if link.fc.get().is_some_and(|fc| fc.get("user").is_some_and(value_truthy)) {
            continue;
        }
        considered = considered.saturating_add(1);
        if link.cls.is("arterial") {
            art = art.saturating_add(1);
        }
        let geom = sc.geom(link);
        let cum = util::cum_len(&geom);
        // cum_len always holds at least the starting 0
        let len = cum.last().copied().unwrap_or(0.0);
        let votes = index.votes(&geom, &cum, len);
        // id is a key read from sc.links, and no link is removed here, so get_mut always finds it
        let Some((feat, n, d)) = index.pick(&votes, link.name.as_str().unwrap_or("")) else {
            if let Some(lm) = sc.links.get_mut(id.as_str())
                && lm.fc.get().is_some_and(|fc| fc.get("src").and_then(Value::as_str) == Some("AT Future Connect"))
            {
                lm.fc = crate::jsnum::Maybe::Missing;
            }
            continue;
        };
        let fc = json!({
            "gt": opt(feat.gt.as_deref()), "pt": opt(feat.pt.as_deref()), "freight": opt(feat.freight.as_deref()), "cycle": opt(feat.cycle.as_deref()),
            "gt10": opt(feat.gt10.as_deref()), "pt10": opt(feat.pt10.as_deref()), "freight10": opt(feat.freight10.as_deref()), "cycle10": opt(feat.cycle10.as_deref()),
            "src": "AT Future Connect", "fid": feat.id.clone(), "d": util::r1(d / n), "at": now,
        });
        let Some(lm) = sc.links.get_mut(id.as_str()) else { continue };
        let is_art = lm.cls.is("arterial");
        lm.fc = crate::jsnum::Maybe::Set(fc);
        matched = matched.saturating_add(1);
        if is_art {
            art_m = art_m.saturating_add(1);
        }
    }
    let (more, more_art) = fill_short_links(sc, &ids);
    matched = matched.saturating_add(more);
    art_m = art_m.saturating_add(more_art);
    ConflateResult {
        matched,
        links: considered,
        arterials: art,
        arterials_matched: art_m,
        share: if considered > 0 { usize_to_f64(matched) / usize_to_f64(considered) } else { 0.0 },
        arterial_share: if art > 0 { Some(usize_to_f64(art_m) / usize_to_f64(art)) } else { None },
    }
}

// Short pieces at junctions take the classes of an adjacent matched link of the same road (three passes).
// Gives how many links were classified, and how many of them are arterials.
fn fill_short_links(sc: &mut Scenario, ids: &[String]) -> (usize, usize) {
    // Bound: these count links in memory, far below usize::MAX
    let (mut matched, mut art_m) = (0usize, 0usize);
    let mut at: HashMap<String, Vec<String>> = HashMap::new();
    for link in sc.links.values() {
        if !link.status.is("closed") {
            for end in [&link.a, &link.b] {
                at.entry(end.clone()).or_default().push(link.id.clone());
            }
        }
    }
    for _pass in 0..3 {
        for id in ids {
            let Some(link) = sc.links.get(id.as_str()) else {
                continue;
            };
            if link.fc.get().is_some_and(value_truthy) || link.status.is("closed") || !sc.nodes.contains_key(&link.a) || !sc.nodes.contains_key(&link.b) || sc.link_len(link) >= 60.0 {
                continue;
            }
            let fc_src = |m: &crate::project::Link| m.fc.get().and_then(|fc| fc.get("src")).and_then(Value::as_str).map(str::to_string);
            let mut adj: Vec<&crate::project::Link> = Vec::new();
            for end in [&link.a, &link.b] {
                for mid in at.get(end).into_iter().flatten() {
                    let Some(other) = sc.links.get(mid.as_str()) else {
                        continue;
                    };
                    if other.id != link.id && other.fc.get().is_some_and(value_truthy) && fc_src(other).unwrap_or_default().starts_with("AT Future Connect") {
                        adj.push(other);
                    }
                }
            }
            let pool: Vec<&crate::project::Link> = if link.ring.truthy() {
                adj.iter().copied().filter(|m| !m.ring.truthy() || fc_src(m).as_deref() == Some("AT Future Connect")).collect()
            } else {
                let mine = norm(link.name.as_str().unwrap_or(""));
                adj.iter().copied().filter(|m| { let n = norm(m.name.as_str().unwrap_or("")); !n.is_empty() && n == mine }).collect()
            };
            let Some((&first, rest)) = pool.split_first() else {
                continue;
            };
            let gt_of = |m: &crate::project::Link| m.fc.get().and_then(|fc| fc.get("gt")).cloned().unwrap_or(Value::Null);
            let mut from = first;
            for c in rest {
                if gt_rank(&gt_of(c)) < gt_rank(&gt_of(from)) {
                    from = c;
                }
            }
            let mut fc: Map<String, Value> = match from.fc.get() {
                Some(Value::Object(o)) => o.clone(),
                _ => Map::new(),
            };
            fc.insert("src".to_string(), json!("AT Future Connect (from the adjacent link)"));
            fc.insert("from".to_string(), json!(from.id));
            let is_art = link.cls.is("arterial");
            // id is a key read from sc.links, and no link is removed here, so get_mut always finds it
            let Some(target) = sc.links.get_mut(id.as_str()) else { continue };
            target.fc = crate::jsnum::Maybe::Set(Value::Object(fc));
            matched = matched.saturating_add(1);
            if is_art {
                art_m = art_m.saturating_add(1);
            }
        }
    }
    (matched, art_m)
}
