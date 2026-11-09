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
use crate::jsnum::{math_max, math_min, value_truthy};
use crate::project::Scenario;
use crate::util::{self, LatLng, ang_diff, bearing, flat};
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

fn truthy(s: &Option<String>) -> bool {
    s.as_deref().is_some_and(|x| !x.is_empty())
}

fn opt(s: &Option<String>) -> Value {
    match s {
        Some(x) => json!(x),
        None => Value::Null,
    }
}

// order of general-traffic classes, highest first
const GT_ORDER: [&str; 6] = [
    "motorway",
    "strategic",
    "primary",
    "secondary",
    "collector",
    "local",
];

fn gt_rank(g: &Value) -> f64 {
    let text = if value_truthy(g) {
        crate::jsnum::value_to_js_string(g)
    } else {
        String::new()
    };
    let n = norm(&text);
    match GT_ORDER.iter().position(|x| n.starts_with(x)) {
        Some(k) => k as f64,
        None => 9.0,
    }
}

fn cell_key(cx: f64, cy: f64) -> i64 {
    (cx * 1e6 + cy) as i64
}

/// `conflateFC(sc, feats, opts)`: puts Future Connect classes on a scenario's links (`link.fc`), and returns how many matched.
///
/// `radius` (default 15 m) and `bearing` (default 30 degrees) are the options of the prototype, where 0 or a missing value
/// gives the default. `now_ms` stamps `fc.at` with the date (UTC). Links with an `fc` that has no Future Connect source are
/// left; a link that lost its match loses an earlier Future Connect class.
pub fn conflate_fc(
    sc: &mut Scenario,
    feats: &[FutureConnect],
    radius: Option<f64>,
    bearing_limit: Option<f64>,
    now_ms: f64,
) -> ConflateResult {
    let pick = |v: Option<f64>, d: f64| match v {
        Some(x) if x != 0.0 && !x.is_nan() => x,
        _ => d,
    };
    let r = pick(radius, 15.0);
    let b_lim = pick(bearing_limit, 30.0);
    let cell_m = 60.0;
    let now = utc_date(now_ms);
    let use_: Vec<&FutureConnect> = feats
        .iter()
        .filter(|f| {
            truthy(&f.gt)
                || truthy(&f.pt)
                || truthy(&f.gt10)
                || truthy(&f.pt10)
                || (truthy(&f.freight) && f.freight.as_deref() != Some("Rail"))
        })
        .collect();
    let lat0 = sc.nodes.values().next().map_or(-36.85, |n| n.lat.num());
    let fl = flat(lat0);
    let mut grid: HashMap<i64, Vec<usize>> = HashMap::new();
    // [ax, ay, bx, by, feature, bearing]
    let mut segs: Vec<[f64; 6]> = Vec::new();
    for (fi, f) in use_.iter().enumerate() {
        for p in &f.paths {
            for i in 0..p.len().saturating_sub(1) {
                let (ax, ay, bx, by) = (fl.x(&p[i]), fl.y(&p[i]), fl.x(&p[i + 1]), fl.y(&p[i + 1]));
                let k = segs.len();
                segs.push([ax, ay, bx, by, fi as f64, bearing(&p[i], &p[i + 1])]);
                let mut cx = ((math_min(ax, bx) - r) / cell_m).floor();
                while cx <= ((math_max(ax, bx) + r) / cell_m).floor() {
                    let mut cy = ((math_min(ay, by) - r) / cell_m).floor();
                    while cy <= ((math_max(ay, by) + r) / cell_m).floor() {
                        grid.entry(cell_key(cx, cy)).or_default().push(k);
                        cy += 1.0;
                    }
                    cx += 1.0;
                }
            }
        }
    }
    let name_ok = |a: &str, b: &str| {
        let (x, y) = (norm(a), norm(b));
        x.is_empty() || y.is_empty() || x == y || x.contains(&y) || y.contains(&x)
    };
    let (mut matched, mut links, mut art, mut art_m) = (0usize, 0usize, 0usize, 0usize);
    let ids: Vec<String> = sc.links.keys().map(str::to_string).collect();
    for id in &ids {
        let l = &sc.links[id.as_str()];
        if l.status.is("closed") || !sc.nodes.contains_key(&l.a) || !sc.nodes.contains_key(&l.b) {
            continue;
        }
        if l.fc
            .get()
            .is_some_and(|fc| fc.get("user").is_some_and(value_truthy))
        {
            continue;
        }
        links += 1;
        if l.cls.is("arterial") {
            art += 1;
        }
        let g = sc.geom(l);
        let cum = util::cum_len(&g);
        let len = cum[cum.len() - 1];
        // votes: feature to (count, total distance), in the order features were first seen
        let mut votes: Vec<(usize, f64, f64)> = Vec::new();
        for fr in [0.25, 0.5, 0.75] {
            let at = util::along(&g, &cum, len * fr);
            let b0 = bearing(
                &util::along(&g, &cum, math_max(0.0, len * fr - 5.0)).p,
                &util::along(&g, &cum, math_min(len, len * fr + 5.0)).p,
            );
            let (px, py) = (fl.x(&at.p), fl.y(&at.p));
            let mut best: Vec<(usize, f64)> = Vec::new();
            if let Some(cell) = grid.get(&cell_key((px / cell_m).floor(), (py / cell_m).floor())) {
                for &k in cell {
                    let q = &segs[k];
                    let d = util::seg_dist(px, py, q[0], q[1], q[2], q[3]).d;
                    let ang = ang_diff(b0, q[5]);
                    if d > r || math_min(ang, 180.0 - ang) > b_lim {
                        continue;
                    }
                    let fi = q[4] as usize;
                    match best.iter_mut().find(|(f, _)| *f == fi) {
                        Some(e) => {
                            // !(best.get(fi) <= d)
                            if !(e.1 <= d) {
                                e.1 = d;
                            }
                        }
                        None => best.push((fi, d)),
                    }
                }
            }
            for (fi, d) in best {
                match votes.iter_mut().find(|(f, _, _)| *f == fi) {
                    Some(v) => {
                        v.1 += 1.0;
                        v.2 += d;
                    }
                    None => votes.push((fi, 1.0, d)),
                }
            }
        }
        let mut pick: Option<(usize, f64, f64)> = None;
        for &(fi, n, d) in &votes {
            if n >= 2.0
                && name_ok(l.name.as_str().unwrap_or(""), &use_[fi].name)
                && pick.is_none_or(|p| n > p.1 || (n == p.1 && d < p.2))
            {
                pick = Some((fi, n, d));
            }
        }
        let Some((fi, n, d)) = pick else {
            let l = sc.links.get_mut(id.as_str()).expect("link");
            if l.fc.get().is_some_and(|fc| {
                fc.get("src").and_then(Value::as_str) == Some("AT Future Connect")
            }) {
                l.fc = crate::jsnum::Maybe::Missing;
            }
            continue;
        };
        let f = use_[fi];
        let fc = json!({
            "gt": opt(&f.gt), "pt": opt(&f.pt), "freight": opt(&f.freight), "cycle": opt(&f.cycle),
            "gt10": opt(&f.gt10), "pt10": opt(&f.pt10), "freight10": opt(&f.freight10), "cycle10": opt(&f.cycle10),
            "src": "AT Future Connect", "fid": f.id.clone(), "d": util::r1(d / n), "at": now,
        });
        let l = sc.links.get_mut(id.as_str()).expect("link");
        let is_art = l.cls.is("arterial");
        l.fc = crate::jsnum::Maybe::Set(fc);
        matched += 1;
        if is_art {
            art_m += 1;
        }
    }
    // Short pieces at junctions take the classes of an adjacent matched link of the same road
    let mut at: HashMap<String, Vec<String>> = HashMap::new();
    for l in sc.links.values() {
        if !l.status.is("closed") {
            for n in [&l.a, &l.b] {
                at.entry(n.clone()).or_default().push(l.id.clone());
            }
        }
    }
    for _pass in 0..3 {
        for id in &ids {
            let l = &sc.links[id.as_str()];
            if l.fc.get().is_some_and(value_truthy)
                || l.status.is("closed")
                || !sc.nodes.contains_key(&l.a)
                || !sc.nodes.contains_key(&l.b)
                || sc.link_len(l) >= 60.0
            {
                continue;
            }
            let fc_src = |m: &crate::project::Link| {
                m.fc.get()
                    .and_then(|fc| fc.get("src"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            };
            let mut adj: Vec<&crate::project::Link> = Vec::new();
            for end in [&l.a, &l.b] {
                for mid in at.get(end).map(Vec::as_slice).unwrap_or(&[]) {
                    let m = &sc.links[mid.as_str()];
                    if m.id != l.id
                        && m.fc.get().is_some_and(value_truthy)
                        && fc_src(m)
                            .unwrap_or_default()
                            .starts_with("AT Future Connect")
                    {
                        adj.push(m);
                    }
                }
            }
            let pool: Vec<&crate::project::Link> = if l.ring.truthy() {
                adj.iter()
                    .copied()
                    .filter(|m| {
                        !m.ring.truthy() || fc_src(m).as_deref() == Some("AT Future Connect")
                    })
                    .collect()
            } else {
                let mine = norm(l.name.as_str().unwrap_or(""));
                adj.iter()
                    .copied()
                    .filter(|m| {
                        let n = norm(m.name.as_str().unwrap_or(""));
                        !n.is_empty() && n == mine
                    })
                    .collect()
            };
            if pool.is_empty() {
                continue;
            }
            let gt_of = |m: &crate::project::Link| {
                m.fc.get()
                    .and_then(|fc| fc.get("gt"))
                    .cloned()
                    .unwrap_or(Value::Null)
            };
            let mut m = pool[0];
            for c in &pool[1..] {
                if gt_rank(&gt_of(c)) < gt_rank(&gt_of(m)) {
                    m = c;
                }
            }
            let mut fc: Map<String, Value> = match m.fc.get() {
                Some(Value::Object(o)) => o.clone(),
                _ => Map::new(),
            };
            fc.insert(
                "src".to_string(),
                json!("AT Future Connect (from the adjacent link)"),
            );
            fc.insert("from".to_string(), json!(m.id));
            let is_art = l.cls.is("arterial");
            let target = sc.links.get_mut(id.as_str()).expect("link");
            target.fc = crate::jsnum::Maybe::Set(Value::Object(fc));
            matched += 1;
            if is_art {
                art_m += 1;
            }
        }
    }
    ConflateResult {
        matched,
        links,
        arterials: art,
        arterials_matched: art_m,
        share: if links > 0 {
            matched as f64 / links as f64
        } else {
            0.0
        },
        arterial_share: if art > 0 {
            Some(art_m as f64 / art as f64)
        } else {
            None
        },
    }
}
