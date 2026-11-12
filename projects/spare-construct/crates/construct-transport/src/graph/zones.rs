//! Zones in the graph: the main connected component, connector targets and zone classes.

use super::sites::site_index;
use super::sort::js_sort_by;
use crate::jsnum::{math_hypot, usize_to_f64};
use crate::project::{OrderedMap, Scenario, Settings, Zone, road_class};
use crate::util::{Flat, clamp, flat};
use std::collections::{HashMap, HashSet};

/// `mainComponent(N, arcs)`: flags (1) the nodes of the largest weakly connected piece of network, given each arc's (from, to).
#[must_use]
pub fn main_component(n: usize, edges: &[(usize, usize)]) -> Vec<u8> {
    let mut p: Vec<usize> = (0..n).collect();
    for &(a, b) in edges {
        let x = uf_find(&mut p, a);
        let y = uf_find(&mut p, b);
        if x != y
            && let Some(slot) = p.get_mut(x)
        {
            *slot = y;
        }
    }
    let mut size = vec![0usize; n];
    let (mut best, mut bs) = (usize::MAX, 0usize);
    for i in 0..n {
        let r = uf_find(&mut p, i);
        if let Some(sz) = size.get_mut(r) {
            *sz = sz.saturating_add(1);
            if *sz > bs {
                bs = *sz;
                best = r;
            }
        }
    }
    (0..n).map(|i| u8::from(uf_find(&mut p, i) == best)).collect()
}

/// Union-find's root of k in the parent array p, halving the path as it goes (an index past the end is its own root).
pub(super) fn uf_find(p: &mut [usize], mut k: usize) -> usize {
    loop {
        let Some(&up) = p.get(k) else {
            return k;
        };
        if up == k {
            return k;
        }
        let next = p.get(up).copied().unwrap_or(up);
        if let Some(slot) = p.get_mut(k) {
            *slot = next;
        }
        k = next;
    }
}

/// A candidate node for zone connectors, in flat coordinates.
pub(crate) struct Cand {
    pub id: String,
    pub x: f64,
    pub y: f64,
}

/// `connectorTargets(sc, z, cand, F, S)`: the zone's own node, or its k nearest candidates (a gateway 1; a zone its connectors, else the settings', 1 to 4).
pub(crate) fn connector_targets(
    sc: &Scenario,
    z: &Zone,
    cand: &[Cand],
    fl: &Flat,
    s: &Settings,
) -> Vec<String> {
    if let Some(node) = z.node.as_str()
        && !node.is_empty()
        && sc.nodes.contains_key(node)
    {
        return vec![node.to_string()];
    }
    let want = if z.kind.is("gateway") {
        1.0
    } else {
        let count = if z.connectors.truthy() {
            z.connectors.int32()
        } else if s.connectors.truthy() {
            s.connectors.int32()
        } else {
            2
        };
        clamp(f64::from(count), 1.0, 4.0)
    };
    let ll = z.ll();
    let zx = fl.x(&ll);
    let zy = fl.y(&ll);
    let mut best: Vec<(String, f64)> = Vec::new();
    for cn in cand {
        let dist = math_hypot(&[cn.x - zx, cn.y - zy]);
        if usize_to_f64(best.len()) < want || best.last().is_some_and(|b| dist < b.1) {
            best.push((cn.id.clone(), dist));
            js_sort_by(&mut best, |a, b| a.1 - b.1);
            if usize_to_f64(best.len()) > want {
                best.pop();
            }
        }
    }
    best.into_iter().map(|b| b.0).collect()
}

/// `connectorMap(sc)`: connector targets for every zone without building a full graph (used for drawing).
///
/// s is the project's settings (`Store.S` in the prototype).
#[must_use]
pub fn connector_map(sc: &Scenario, s: &Settings) -> OrderedMap<Vec<String>> {
    let ids: Vec<&str> = sc.nodes.keys().collect();
    let idx: HashMap<String, usize> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.to_string(), i))
        .collect();
    let mut edges = Vec::new();
    let mut deg: HashSet<&str> = HashSet::new();
    for l in sc.links.values() {
        if l.status.is("closed") {
            continue;
        }
        let (Some(&a), Some(&b)) = (idx.get(&l.a), idx.get(&l.b)) else {
            continue;
        };
        edges.push((a, b));
        if road_class(l.cls.as_str()).rank >= 5.0 {
            continue;
        }
        deg.insert(&l.a);
        deg.insert(&l.b);
    }
    let main = main_component(ids.len(), &edges);
    let in_site = site_index(sc, &idx).0;
    let nodes: Vec<&crate::project::Node> = sc
        .nodes
        .values()
        .filter(|n| {
            deg.contains(n.id.as_str())
                && idx.get(&n.id).is_some_and(|&i| main.get(i).is_some_and(|&m| m != 0))
                && idx.get(&n.id).is_none_or(|&i| in_site.get(i).is_none_or(Option::is_none))
        })
        .collect();
    let lat0 = nodes.first().map_or(-36.85, |n| n.lat.num());
    let f = flat(lat0);
    let cand: Vec<Cand> = nodes
        .iter()
        .map(|n| {
            let ll = n.ll();
            Cand {
                id: n.id.clone(),
                x: f.x(&ll),
                y: f.y(&ll),
            }
        })
        .collect();
    let mut m = OrderedMap::new();
    for z in sc.zones.values() {
        m.insert(z.id.clone(), connector_targets(sc, z, &cand, &f, s));
    }
    m
}

/// `zoneClass(z)`: "external" for a gateway, else the zone's role when it is existing, committed or development, else existing (`existing === true`) or development.
#[must_use]
pub fn zone_class(z: Option<&Zone>) -> &'static str {
    let Some(z) = z else {
        return "existing";
    };
    if z.kind.is("gateway") {
        return "external";
    }
    match z.role.as_str() {
        Some("existing") => "existing",
        Some("committed") => "committed",
        Some("development") => "development",
        _ => {
            if z.existing.is_true() {
                "existing"
            } else {
                "development"
            }
        }
    }
}

/// `zcIdx(z)`: 2 development, 1 committed, otherwise 0.
#[must_use]
pub fn zc_idx(z: Option<&Zone>) -> usize {
    match zone_class(z) {
        "development" => 2,
        "committed" => 1,
        _ => 0,
    }
}
