//! The network helpers of model.js (`Net.geom`, `Net.len`, `Net.linksAt`, `Net.project`) on a scenario held as loose JSON.

use super::jv::{Jx, V};
use crate::util::{self, LatLng};

/// `[lat, lng]` of a node (or point) value.
pub fn ll_of(n: &V) -> LatLng {
    [n.g("lat").n(), n.g("lng").n()]
}

/// A stored `[lat, lng]` pair.
pub fn pt(p: &V) -> LatLng {
    [p.ix(0).n(), p.ix(1).n()]
}

/// `Net.geom(sc, L)`: the link's points from its a-node through its shape points to its b-node (None when a node is missing).
pub fn geom(sc: &V, l: &V) -> Option<Vec<LatLng>> {
    let a = sc.g("nodes").get(&l.g("a").s())?;
    let b = sc.g("nodes").get(&l.g("b").s())?;
    let mut g = vec![ll_of(a)];
    g.extend(l.g("pts").items().iter().map(pt));
    g.push(ll_of(b));
    Some(g)
}

/// `Net.len(sc, L)`: the link's length in metres.
pub fn len(sc: &V, l: &V) -> f64 {
    geom(sc, l).map_or(f64::NAN, |g| util::poly_len(&g))
}

/// `Net.linksAt(sc, nid)`: the links that start or end at a node, in the scenario's order.
pub fn links_at<'a>(sc: &'a V, nid: &str) -> Vec<&'a V> {
    sc.g("links")
        .vals()
        .into_iter()
        .filter(|l| l.g("a").eq_s(nid) || l.g("b").eq_s(nid))
        .collect()
}

/// The result of [`project`].
pub struct Projection {
    /// Distance from the point to the link (m).
    pub d: f64,
    /// Index of the nearest segment.
    pub i: usize,
    /// The nearest point on the link.
    pub p: LatLng,
}

/// `Net.project(sc, L, ll)`: the nearest point on a link to a position.
pub fn project(sc: &V, l: &V, ll: LatLng) -> Option<Projection> {
    let g = geom(sc, l)?;
    let f = util::flat(ll[0]);
    let (px, py) = (f.x(&ll), f.y(&ll));
    let mut best: Option<(f64, usize, f64, f64)> = None;
    for i in 0..g.len().saturating_sub(1) {
        let r = util::seg_dist(
            px,
            py,
            f.x(&g[i]),
            f.y(&g[i]),
            f.x(&g[i + 1]),
            f.y(&g[i + 1]),
        );
        if best.is_none_or(|b| r.d < b.0) {
            best = Some((r.d, i, r.x, r.y));
        }
    }
    let (d, i, x, y) = best.unwrap_or((f64::INFINITY, 0, f64::NAN, f64::NAN));
    Some(Projection {
        d,
        i,
        p: [y / f.ky, x / f.kx],
    })
}

/// `Object.values(sc.links)` that are open (`L.status !== 'closed'`).
pub fn open_links(sc: &V) -> Vec<&V> {
    sc.g("links")
        .vals()
        .into_iter()
        .filter(|l| !l.g("status").eq_s("closed"))
        .collect()
}

/// `Kit.junctionName(id, sc)`: the names of the roads at a junction (up to three, joined with " / "), or "Unnamed junction".
pub fn junction_name(sc: &V, id: &str) -> String {
    let mut names: Vec<String> = Vec::new();
    for l in links_at(sc, id) {
        let n = l.g("name");
        if n.t() && !names.contains(&n.s()) {
            names.push(n.s());
        }
    }
    if names.is_empty() {
        "Unnamed junction".to_string()
    } else {
        names.into_iter().take(3).collect::<Vec<_>>().join(" / ")
    }
}

/// `roadLinks(sc, id)`: the open links at a junction.
pub fn road_links<'a>(sc: &'a V, id: &str) -> Vec<&'a V> {
    links_at(sc, id)
        .into_iter()
        .filter(|l| !l.g("status").eq_s("closed"))
        .collect()
}

/// `Scenarios.zoneRole(z)`: "external" for a gateway, else the zone's role when it is a known one, else "existing" for an existing
/// land use and "development" for anything else.
pub fn zone_role(z: &V) -> &'static str {
    if z.g("kind").eq_s("gateway") {
        return "external";
    }
    for r in ["existing", "committed", "development"] {
        if z.g("role").eq_s(r) {
            return r;
        }
    }
    if z.g("existing").strict_eq(&V::Bool(true)) {
        "existing"
    } else {
        "development"
    }
}

/// `Engine.zoneClass(z)`: "external" for a gateway, else the zone's role when it is existing, committed or development, else existing
/// (`existing === true`) or development.
pub fn zone_class(z: &V) -> &'static str {
    if !z.t() {
        return "existing";
    }
    if z.g("kind").eq_s("gateway") {
        return "external";
    }
    for r in ["existing", "committed", "development"] {
        if z.g("role").eq_s(r) {
            return r;
        }
    }
    if z.g("existing").strict_eq(&V::Bool(true)) {
        "existing"
    } else {
        "development"
    }
}

/// `Engine.stageOf(z, sc)`: the share of a zone built out: its own `buildout` percent, times the scenario's for a development zone.
pub fn stage_of(z: &V, sc: &V) -> f64 {
    let ok = |v: &V| v.entered();
    let mut k = if ok(z.g("buildout")) {
        util::clamp(z.g("buildout").n(), 0.0, 100.0) / 100.0
    } else {
        1.0
    };
    if ok(sc.g("buildout")) && zone_class(z) == "development" {
        k *= util::clamp(sc.g("buildout").n(), 0.0, 100.0) / 100.0;
    }
    k
}

/// `Scenarios.linkStage(L)`: the link's stage, or "proposal" for a proposed link and "existing" otherwise.
pub fn link_stage(l: &V) -> &'static str {
    for s in [
        "existing",
        "committed",
        "unfunded",
        "proposal",
        "mitigation",
    ] {
        if l.g("stage").eq_s(s) {
            return s;
        }
    }
    if l.g("status").eq_s("proposed") {
        "proposal"
    } else {
        "existing"
    }
}

/// `Scenarios.unfunded(L)`: stage "unfunded", or committed with funding "unfunded".
pub fn link_unfunded(l: &V) -> bool {
    let s = link_stage(l);
    s == "unfunded" || (s == "committed" && l.g("funding").eq_s("unfunded"))
}

/// The land use quantity of a zone (`+(z.lu || {})[k] || 0`).
pub fn lu_of(z: &V, k: &str) -> f64 {
    let v = z.g("lu").g(k).n();
    if v.is_nan() { 0.0 } else { v }
}

/// The development zones of a scenario (`Object.values(sc.zones).filter(z => z.kind !== 'gateway' && role is development)`).
pub fn dev_zones(sc: &V) -> Vec<&V> {
    sc.g("zones")
        .vals()
        .into_iter()
        .filter(|z| !z.g("kind").eq_s("gateway") && zone_role(z) == "development")
        .collect()
}
