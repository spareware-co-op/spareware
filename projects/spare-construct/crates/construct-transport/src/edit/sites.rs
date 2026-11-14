//! Junction sites: model.js `Sites`.
//!
//! A site groups the nodes of one intersection (a staggered junction, the two carriageways of a divided road, a
//! roundabout and its ring). [`detect`] finds them: candidate nodes have a control, or three or more links with one of
//! collector rank or higher, and no motorway or ramp link (interchange terminals are never joined). Two candidates join
//! along a link of 35 m or less, up to 60 m when both are signals, when the link is one carriageway of a dual pair, or
//! when both are within 30 m of the same AT controlled-intersection point. A roundabout's ring nodes join along the ring
//! and are never merged with a junction beside it. No site is wider than 80 m. Sites made by hand are kept.

use super::IdSource;
use super::collate::locale_compare_f64;
use crate::graph::{js_sort_by, js_str_lt};
use crate::jsnum::{JsNum, Maybe, value_to_number, value_truthy};
use crate::project::{Link, Node, OrderedMap, Project, Scenario, Site, SiteSource, road_class};
use crate::util::{self, LatLng};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// model.js `Sites.RULES`: the distances (m) and angle (degrees) the detection uses, and where they come from.
#[derive(Clone, Debug, PartialEq)]
pub struct SiteRules {
    /// Candidates this close (m) along a link join.
    pub near: f64,
    /// Candidates this close join when both are signals, the link is one carriageway of a dual pair, or both are at an AT point.
    pub wide: f64,
    /// A candidate this close (m) to an AT controlled-intersection point is at it.
    pub at: f64,
    /// No site is wider than this (m).
    pub diameter: f64,
    /// The two carriageways of a dual pair head within this many degrees of opposite.
    pub dual_bearing: f64,
    /// Carriageway midpoints this close (m) are a dual pair.
    pub dual_gap: f64,
    /// Where the rules come from (written on each site).
    pub source: String,
    /// What kind of figure they are.
    pub basis: String,
}

impl Default for SiteRules {
    fn default() -> Self {
        SiteRules {
            near: 35.0,
            wide: 60.0,
            at: 30.0,
            diameter: 80.0,
            dual_bearing: 15.0,
            dual_gap: 60.0,
            source: "Spare Construct junction-site rules (docs/spec/junction-sites)".to_string(),
            basis: "Spare Construct assumption".to_string(),
        }
    }
}

/// An AT controlled-intersection point (`P.data.controlled`): position and site number.
#[derive(Clone, Debug, PartialEq)]
pub struct AtPoint {
    /// Latitude.
    pub lat: f64,
    /// Longitude.
    pub lng: f64,
    /// The site number, `p.no ?? p.id` (None when the point has neither key).
    pub key: Option<Value>,
}

/// `Sites.points(P)`: the AT controlled-intersection points loaded into the project, if any.
pub fn at_points(p: &Project) -> Vec<AtPoint> {
    let Some(list) = p
        .data
        .get()
        .and_then(|d| d.controlled.get())
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    list.iter()
        .map(|o| {
            let num = |k: &str| o.get(k).map_or(f64::NAN, value_to_number);
            let key = match o.get("no") {
                Some(v) if !v.is_null() => Some(v.clone()),
                _ => o.get("id").cloned(),
            };
            AtPoint {
                lat: num("lat"),
                lng: num("lng"),
                key,
            }
        })
        .collect()
}

/// What `Sites.detect(sc, opts)` takes: the rules (default [`SiteRules::default`]) and the AT points (default none).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DetectOpts {
    /// Rule values.
    pub rules: SiteRules,
    /// AT controlled-intersection points.
    pub points: Vec<AtPoint>,
}

/// What `Sites.detect` returns: sites made, nodes in them, and sites kept (made by hand).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DetectResult {
    /// Sites made.
    pub sites: usize,
    /// Nodes in the sites made.
    pub nodes: usize,
    /// Hand-made sites kept.
    pub kept: usize,
}

/// `Sites.of(sc, nodeId)`: the first site that has this node.
#[must_use]
pub fn of<'a>(sc: &'a Scenario, node_id: &str) -> Option<&'a Site> {
    sc.sites
        .get()?
        .values()
        .find(|s| s.nodes.iter().any(|n| n == node_id))
}

/// `SameValueZero` for the site numbers of AT points.
fn same_value(a: Option<&Value>, b: Option<&Value>) -> bool {
    match (a, b) {
        (None, None) | (Some(Value::Null), Some(Value::Null)) => true,
        (Some(Value::Number(x)), Some(Value::Number(y))) => x.as_f64() == y.as_f64(),
        (Some(Value::String(x)), Some(Value::String(y))) => x == y,
        (Some(Value::Bool(x)), Some(Value::Bool(y))) => x == y,
        _ => false,
    }
}

/// `ids.sort()`: node ids in JavaScript's default string order.
pub fn sort_ids(v: &mut [String]) {
    js_sort_by(v, |a, b| {
        if js_str_lt(a, b) {
            -1.0
        } else if js_str_lt(b, a) {
            1.0
        } else {
            0.0
        }
    });
}

fn blank_site(id: &str) -> Site {
    Site {
        id: id.to_string(),
        nodes: Vec::new(),
        control: Maybe::Null,
        cycle: Maybe::Null,
        phases: Maybe::Null,
        name: Maybe::Set(String::new()),
        at_site_no: Maybe::Null,
        fixed_timing: false.into(),
        lanes: Maybe::Set(OrderedMap::new()),
        src: Maybe::Missing,
        gaps: Maybe::Missing,
        split: Maybe::Missing,
        extra: serde_json::Map::new(),
    }
}

/// `Sites.detect(sc, opts)`: groups the nodes of each intersection into sites, replacing the sites an earlier detection made
/// (src.nodes "auto") and keeping the ones made by hand. Leaves `sc.sites` set (empty when there are none).
pub fn detect(sc: &mut Scenario, ids: &mut dyn IdSource, opts: &DetectOpts) -> DetectResult {
    if !sc.sites.is_set() {
        sc.sites = Maybe::Set(OrderedMap::new());
    }
    let (keep_n, kept) = drop_auto_sites(sc);
    let new_sites: Vec<Site> = {
        let net = Detection::new(sc, opts, &kept);
        let groups = net.groups();
        net.sites(&groups, ids)
    };
    let made = new_sites.len();
    let in_sites: usize = new_sites.iter().map(|s| s.nodes.len()).sum();
    // site.name = this.name(sc, site); sc.sites[id] = site
    for mut site in new_sites {
        site.name = Maybe::Set(name(sc, &site));
        let id = site.id.clone();
        // sc.sites was set at the start
        if let Some(sites) = sc.sites.get_mut() {
            sites.insert(id, site);
        }
    }
    DetectResult {
        sites: made,
        nodes: in_sites,
        kept: keep_n,
    }
}

/// Sites made by hand stay, and their nodes are not candidates; the earlier automatic ones go. Returns how many sites were
/// kept and the nodes in them.
fn drop_auto_sites(sc: &mut Scenario) -> (usize, HashSet<String>) {
    let mut kept: HashSet<String> = HashSet::new();
    let Some(sites) = sc.sites.get_mut() else {
        return (0, kept);
    };
    let auto = |s: &Site| s.src.get().is_some_and(|x| x.nodes.is("auto"));
    let keep_n = sites.values().filter(|s| !auto(s)).count();
    for s in sites.values().filter(|s| !auto(s)) {
        for n in &s.nodes {
            kept.insert(n.clone());
        }
    }
    let gone: Vec<String> = sites
        .values()
        .filter(|s| auto(s))
        .map(|s| s.id.clone())
        .collect();
    for id in gone {
        sites.remove(&id);
    }
    (keep_n, kept)
}

/// Candidate nodes, in the order the links first reach them, and each one's position in that list: nodes with a control,
/// or with three or more links one of which is of collector rank or higher, and no motorway or ramp link. Nodes of sites
/// made by hand (`kept`) are not candidates.
fn candidates<'a>(
    nodes: &OrderedMap<Node>,
    links: &[&'a Link],
    kept: &HashSet<String>,
) -> (Vec<&'a str>, HashMap<&'a str, usize>) {
    // The links at each node, in the order the links first reach them
    let mut inc: Vec<(&'a str, Vec<usize>)> = Vec::new();
    let mut inc_ix: HashMap<&'a str, usize> = HashMap::new();
    for (li, &l) in links.iter().enumerate() {
        for n in [l.a.as_str(), l.b.as_str()] {
            if let Some(&k) = inc_ix.get(n) {
                // inc_ix holds positions in inc only
                if let Some(entry) = inc.get_mut(k) {
                    entry.1.push(li);
                }
            } else {
                inc_ix.insert(n, inc.len());
                inc.push((n, vec![li]));
            }
        }
    }
    let rank = |l: &Link| road_class(l.cls.as_str()).rank;
    let mut cand: Vec<&'a str> = Vec::new();
    let mut cand_ix: HashMap<&'a str, usize> = HashMap::new();
    for (id, ls) in &inc {
        if kept.contains(*id)
            || ls
                .iter()
                .any(|&li| links.get(li).is_some_and(|&l| rank(l) >= 5.0))
        {
            continue;
        }
        let controlled = nodes.get(id).is_some_and(|n| n.control.str_or("none") != "none");
        let max_rank = ls
            .iter()
            .filter_map(|&li| links.get(li))
            .map(|&l| rank(l))
            .fold(f64::NEG_INFINITY, f64::max);
        if controlled || (ls.len() >= 3 && max_rank >= 3.0) {
            cand_ix.insert(*id, cand.len());
            cand.push(*id);
        }
    }
    (cand, cand_ix)
}

/// One-way links between candidates short enough to be one carriageway of a pair: the link's index, bearing and midpoint.
fn carriageways(
    sc: &Scenario,
    links: &[&Link],
    lens: &[f64],
    cand_ix: &HashMap<&str, usize>,
    wide: f64,
) -> Vec<(usize, f64, LatLng)> {
    let oneway = |l: &Link| !(l.lanes_ab.int32() > 0 && l.lanes_ba.int32() > 0);
    let mut ow: Vec<(usize, f64, LatLng)> = Vec::new();
    for (li, (&l, &len)) in links.iter().zip(lens).enumerate() {
        if oneway(l)
            && cand_ix.contains_key(l.a.as_str())
            && cand_ix.contains_key(l.b.as_str())
            && len <= wide
        {
            let g = sc.geom(l);
            // A link's geometry always holds its two end nodes
            let (Some(first), Some(last)) = (g.first(), g.last()) else {
                continue;
            };
            let brg = if l.lanes_ab.int32() > 0 {
                util::bearing(first, last)
            } else {
                util::bearing(last, first)
            };
            let mid = util::along(&g, &util::cum_len(&g), len / 2.0).p;
            ow.push((li, brg, mid));
        }
    }
    ow
}

/// Union-find over the candidates: each candidate's parent, and each root's members (None for a candidate that is not a root).
struct Groups {
    par: Vec<usize>,
    mem: Vec<Option<Vec<usize>>>,
}

impl Groups {
    fn new(n: usize) -> Self {
        Groups {
            par: (0..n).collect(),
            mem: (0..n).map(|i| Some(vec![i])).collect(),
        }
    }
}

/// The root of candidate i, halving the path on the way.
fn find(par: &mut [usize], mut i: usize) -> usize {
    // Every parent is an index into par (it starts as 0..n and only ever takes roots), so the lookups never miss
    while let Some(&up) = par.get(i)
        && up != i
    {
        let grand = par.get(up).copied().unwrap_or(up);
        if let Some(slot) = par.get_mut(i) {
            *slot = grand;
        }
        i = grand;
    }
    i
}

/// The scenario as `Sites.detect` reads it: open links between two different existing nodes, their lengths, the
/// roundabout ring nodes, the candidates and the one-way carriageways.
struct Detection<'a> {
    sc: &'a Scenario,
    rules: &'a SiteRules,
    points: &'a [AtPoint],
    links: Vec<&'a Link>,
    lens: Vec<f64>,
    ring: HashSet<&'a str>,
    cand: Vec<&'a str>,
    cand_ix: HashMap<&'a str, usize>,
    ow: Vec<(usize, f64, LatLng)>,
}

impl<'a> Detection<'a> {
    fn new(sc: &'a Scenario, opts: &'a DetectOpts, kept: &HashSet<String>) -> Self {
        let nodes = &sc.nodes;
        let links: Vec<&'a Link> = sc
            .links
            .values()
            .filter(|l| {
                !l.status.is("closed")
                    && nodes.contains_key(&l.a)
                    && nodes.contains_key(&l.b)
                    && l.a != l.b
            })
            .collect();
        let mut ring: HashSet<&'a str> = HashSet::new();
        for &l in &links {
            if l.ring.truthy() {
                ring.insert(l.a.as_str());
                ring.insert(l.b.as_str());
            }
        }
        let (cand, cand_ix) = candidates(nodes, &links, kept);
        let lens: Vec<f64> = links.iter().map(|l| sc.link_len(l)).collect();
        let ow = carriageways(sc, &links, &lens, &cand_ix, opts.rules.wide);
        Detection {
            sc,
            rules: &opts.rules,
            points: &opts.points,
            links,
            lens,
            ring,
            cand,
            cand_ix,
            ow,
        }
    }

    fn ll(&self, id: &str) -> LatLng {
        self.sc.nodes.get(id).map_or([f64::NAN; 2], Node::ll)
    }

    /// True when link li is one carriageway of a dual pair.
    fn dual(&self, li: usize) -> bool {
        let Some(me) = self.ow.iter().find(|o| o.0 == li) else {
            return false;
        };
        self.ow.iter().any(|m| {
            m.0 != li
                && util::ang_diff(me.1, m.1) >= 180.0 - self.rules.dual_bearing
                && util::dist(&me.2, &m.2) <= self.rules.dual_gap
        })
    }

    /// The site numbers of the AT points near a node.
    fn near_at(&self, id: &str) -> Vec<Option<Value>> {
        let p0 = self.ll(id);
        self.points
            .iter()
            .filter(|p| util::dist(&p0, &[p.lat, p.lng]) <= self.rules.at)
            .map(|p| p.key.clone())
            .collect()
    }

    /// True when two nodes are near the same AT point.
    fn same_at(&self, a: &str, b: &str) -> bool {
        let here = self.near_at(a);
        !here.is_empty()
            && self
                .near_at(b)
                .iter()
                .any(|key| here.iter().any(|other| same_value(key.as_ref(), other.as_ref())))
    }

    /// Joins the groups of candidates a and b, unless they are one group already or the site would then be wider than
    /// the rules' diameter.
    fn join(&self, groups: &mut Groups, a: usize, b: usize) {
        let (left, right) = (find(&mut groups.par, a), find(&mut groups.par, b));
        if left == right {
            return;
        }
        // A root always has members
        let (Some(Some(left_mem)), Some(Some(right_mem))) = (groups.mem.get(left), groups.mem.get(right)) else {
            return;
        };
        let mut all: Vec<usize> = left_mem.clone();
        all.extend(right_mem.iter().copied());
        let pos = |ci: usize| self.cand.get(ci).map_or([f64::NAN; 2], |id| self.ll(id));
        // Every pair, in order
        let mut rest = all.as_slice();
        while let Some((&first, tail)) = rest.split_first() {
            for &other in tail {
                if util::dist(&pos(first), &pos(other)) > self.rules.diameter {
                    return;
                }
            }
            rest = tail;
        }
        if let Some(slot) = groups.par.get_mut(right) {
            *slot = left;
        }
        if let Some(slot) = groups.mem.get_mut(left) {
            *slot = Some(all);
        }
        if let Some(slot) = groups.mem.get_mut(right) {
            *slot = None;
        }
    }

    /// The candidates grouped by joining along the links between them, shortest first.
    fn groups(&self) -> Groups {
        let rules = self.rules;
        let mut groups = Groups::new(self.cand.len());
        let mut order: Vec<(usize, f64)> = Vec::new();
        for (li, (l, &len)) in self.links.iter().zip(&self.lens).enumerate() {
            if self.cand_ix.contains_key(l.a.as_str()) && self.cand_ix.contains_key(l.b.as_str()) {
                order.push((li, len));
            }
        }
        js_sort_by(&mut order, |first, second| first.1 - second.1);
        for (li, len) in order {
            let Some(l) = self.links.get(li) else {
                continue;
            };
            let (ra, rb) = (self.ring.contains(l.a.as_str()), self.ring.contains(l.b.as_str()));
            // order holds only links between two candidates
            let (Some(&ia), Some(&ib)) = (self.cand_ix.get(l.a.as_str()), self.cand_ix.get(l.b.as_str())) else {
                continue;
            };
            if l.ring.truthy() && ra && rb {
                self.join(&mut groups, ia, ib);
                continue;
            }
            if ra || rb {
                continue; // a roundabout is not merged with a junction beside it
            }
            let nodes = &self.sc.nodes;
            let sig = nodes.get(l.a.as_str()).is_some_and(|n| n.control.is("signals"))
                && nodes.get(l.b.as_str()).is_some_and(|n| n.control.is("signals"));
            if len <= rules.near || (len <= rules.wide && (sig || self.dual(li) || self.same_at(&l.a, &l.b))) {
                self.join(&mut groups, ia, ib);
            }
        }
        groups
    }

    /// The sites of the groups of two or more candidates, in the order of their first candidate.
    fn sites(&self, groups: &Groups, ids: &mut dyn IdSource) -> Vec<Site> {
        let rules = self.rules;
        let today = ids.today();
        let mut out: Vec<Site> = Vec::new();
        for m in groups.mem.iter().flatten().filter(|m| m.len() >= 2) {
            let members: Vec<&str> = m.iter().filter_map(|&ci| self.cand.get(ci).copied()).collect();
            let id = ids.uid("j");
            // new Set(m.flatMap(nearAT))
            let mut at: Vec<Option<Value>> = Vec::new();
            for n in &members {
                for key in self.near_at(n) {
                    if !at.iter().any(|seen| same_value(seen.as_ref(), key.as_ref())) {
                        at.push(key);
                    }
                }
            }
            let mut sorted: Vec<String> = members.iter().map(std::string::ToString::to_string).collect();
            sort_ids(&mut sorted);
            let mut site = blank_site(&id);
            site.nodes = sorted;
            site.at_site_no = match at.as_slice() {
                [] => Maybe::Null,
                [one] => one.clone().map_or(Maybe::Missing, Maybe::Set),
                _ => Maybe::Set(Value::Array(
                    at.iter()
                        .map(|key| key.clone().unwrap_or(Value::Null))
                        .collect(),
                )),
            };
            site.src = Maybe::Set(SiteSource {
                nodes: Maybe::Set("auto".to_string()),
                at: Maybe::Set(today.clone()),
                rule: Maybe::Set(rules.source.clone()),
                at_site_no: if at.is_empty() {
                    Maybe::Missing
                } else {
                    Maybe::Set("AT controlled intersections".to_string())
                },
                extra: serde_json::Map::new(),
            });
            out.push(site);
        }
        out
    }
}

/// `Sites.name(sc, site)`: the site's name, or the roads leaving it, highest class first ("Great South Road / Waihoehoe Road").
pub fn name(sc: &Scenario, site: &Site) -> String {
    if let Some(n) = site.name.as_str()
        && !n.is_empty()
    {
        return n.to_string();
    }
    let ms: HashSet<&str> = site.nodes.iter().map(String::as_str).collect();
    let mut ext: Vec<&Link> = sc
        .links
        .values()
        .filter(|l| {
            !l.status.is("closed")
                && ms.contains(l.a.as_str()) != ms.contains(l.b.as_str())
                && l.name.as_str().is_some_and(|n| !n.is_empty())
        })
        .collect();
    js_sort_by(&mut ext, |p, q| {
        let d = road_class(q.cls.as_str()).rank - road_class(p.cls.as_str()).rank;
        if d != 0.0 && !d.is_nan() {
            d
        } else {
            locale_compare_f64(p.name.as_str().unwrap_or(""), q.name.as_str().unwrap_or(""))
        }
    });
    let mut names: Vec<&str> = Vec::new();
    for l in ext {
        let n = l.name.as_str().unwrap_or("");
        if !names.contains(&n) {
            names.push(n);
        }
    }
    let joined = names.into_iter().take(3).collect::<Vec<_>>().join(" / ");
    if joined.is_empty() {
        "Unnamed junction".to_string()
    } else {
        joined
    }
}

/// `Sites.merge(sc, ids)`: merges sites and/or nodes into one site made by hand and returns its id; None when fewer than two
/// nodes result. The new site keeps the first old site's control, cycle, phases, AT site number, timing flag and lanes.
pub fn merge(sc: &mut Scenario, ids: &mut dyn IdSource, items: &[String]) -> Option<String> {
    if !sc.sites.is_set() {
        sc.sites = Maybe::Set(OrderedMap::new());
    }
    let mut nodes: Vec<String> = Vec::new();
    let mut old: Vec<(String, String)> = Vec::new(); // (key, id the site carries)
    // sc.sites was set just above
    let sites = sc.sites.get()?;
    for id in items {
        let found: Option<(String, &Site)> = match sites.get(id) {
            Some(s) => Some((id.clone(), s)),
            None if sc.nodes.contains_key(id) => sites
                .iter()
                .find(|(_, s)| s.nodes.iter().any(|n| n == id))
                .map(|(k, s)| (k.to_string(), s)),
            None => None,
        };
        if let Some((key, s)) = found {
            if !old.iter().any(|(k, _)| *k == key) {
                old.push((key, s.id.clone()));
                for n in &s.nodes {
                    if !nodes.contains(n) {
                        nodes.push(n.clone());
                    }
                }
            }
        } else if sc.nodes.contains_key(id) && !nodes.contains(id) {
            nodes.push(id.clone());
        }
    }
    if nodes.len() < 2 {
        return None;
    }
    let first: Option<Site> = old
        .first()
        .and_then(|(k, _)| sc.sites.get().and_then(|s| s.get(k)).cloned());
    let id = ids.uid("j");
    if let Some(sites) = sc.sites.get_mut() {
        for (_, sid) in &old {
            sites.remove(sid);
        }
    }
    sort_ids(&mut nodes);
    let mut site = blank_site(&id);
    site.nodes = nodes;
    if let Some(f) = &first {
        if f.control.as_str().is_some_and(|c| !c.is_empty()) {
            site.control = f.control.clone();
        }
        if f.cycle.truthy() {
            site.cycle = f.cycle.clone();
        }
        if f.phases.get().is_some_and(value_truthy) {
            site.phases = f.phases.clone();
        }
        site.at_site_no = if f.at_site_no.is_nullish() {
            Maybe::Null
        } else {
            f.at_site_no.clone()
        };
        site.fixed_timing = Maybe::Set(JsNum::Bool(f.fixed_timing.truthy()));
        site.lanes = Maybe::Set(f.lanes.get().cloned().unwrap_or_default());
    }
    site.src = Maybe::Set(SiteSource {
        nodes: Maybe::Set("user".to_string()),
        at: Maybe::Set(ids.today()),
        rule: Maybe::Missing,
        at_site_no: Maybe::Missing,
        extra: serde_json::Map::new(),
    });
    if let Some(sites) = sc.sites.get_mut() {
        sites.insert(id.clone(), site);
    }
    Some(id)
}

/// `Sites.split(sc, siteId)`: splits a site back into separate nodes (deletes it); false when there is no such site.
pub fn split(sc: &mut Scenario, site_id: &str) -> bool {
    match sc.sites.get_mut() {
        Some(s) if s.contains_key(site_id) => {
            s.remove(site_id);
            true
        }
        _ => false,
    }
}
