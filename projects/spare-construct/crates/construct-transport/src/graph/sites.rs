//! Junction sites in the graph: membership, movement slots and traced movement flows.

use super::{Graph, GraphSite};
use crate::jsnum::value_to_number;
use crate::project::Scenario;
use crate::standards::pcu_f;
use serde_json::Value;
use std::collections::HashMap;

/// `siteIndex(sc, idx)`: each node's site id (indexed like idx) and the sites with their member nodes.
///
/// A node already taken by an earlier site is left out of later ones; a site with no members
/// left is dropped.
pub fn site_index(
    sc: &Scenario,
    idx: &HashMap<String, usize>,
) -> (Vec<Option<String>>, Vec<GraphSite>) {
    let mut of: Vec<Option<String>> = vec![None; idx.len()];
    let mut list = Vec::new();
    let Some(sites) = sc.sites.get() else {
        return (of, list);
    };
    for s in sites.values() {
        let m: Vec<usize> = s
            .nodes
            .iter()
            .filter_map(|id| idx.get(id).copied())
            .filter(|&i| of[i].is_none())
            .collect();
        if m.is_empty() {
            continue;
        }
        for &i in &m {
            of[i] = Some(s.id.clone());
        }
        list.push(GraphSite {
            site: s.clone(),
            members: m,
        });
    }
    (of, list)
}

/// One site's movement slots: entry arcs, exit arcs and the slot of (entry e, exit x) at `off + e * nx + x`.
#[derive(Clone, Debug, PartialEq)]
pub struct SiteMoveList {
    /// Site id.
    pub id: String,
    /// Arcs entering the site from outside.
    pub inc: Vec<usize>,
    /// Arcs leaving the site.
    pub exits: Vec<usize>,
    /// First slot.
    pub off: usize,
    /// Number of exits.
    pub nx: usize,
}

/// Site movements traced along paths (`siteMoveIndex`): per site its entries, exits and slots; per arc its site and position.
#[derive(Clone, Debug, PartialEq)]
pub struct SiteMoves {
    /// Sites in graph order.
    pub list: Vec<SiteMoveList>,
    /// Site position by site id.
    pub of: HashMap<String, usize>,
    /// Site each arc enters (-1 for none).
    pub ent_s: Vec<i32>,
    /// Position of each arc among its site's entries.
    pub ent_i: Vec<i32>,
    /// Site each arc leaves (-1 for none).
    pub ext_s: Vec<i32>,
    /// Position of each arc among its site's exits.
    pub ext_i: Vec<i32>,
    /// Number of slots.
    pub n: usize,
}

/// `siteMoveIndex(G)`, from the graph's sites, arrival and departure lists (None without sites).
pub(crate) fn site_move_index(g: &Graph) -> Option<SiteMoves> {
    if g.sites.is_empty() {
        return None;
    }
    let m = g.m;
    let mut ent_s = vec![-1i32; m];
    let mut ent_i = vec![0i32; m];
    let mut ext_s = vec![-1i32; m];
    let mut ext_i = vec![0i32; m];
    let mut list = Vec::new();
    let mut of = HashMap::new();
    let mut n = 0usize;
    for (s, gs) in g.sites.iter().enumerate() {
        let (inc, exits) = site_entries(g, &gs.members);
        for (k, &a) in inc.iter().enumerate() {
            ent_s[a] = s as i32;
            ent_i[a] = k as i32;
        }
        for (k, &a) in exits.iter().enumerate() {
            ext_s[a] = s as i32;
            ext_i[a] = k as i32;
        }
        let nx = exits.len();
        let add = inc.len() * nx;
        list.push(SiteMoveList {
            id: gs.site.id.clone(),
            inc,
            exits,
            off: n,
            nx,
        });
        of.insert(gs.site.id.clone(), s);
        n += add;
    }
    Some(SiteMoves {
        list,
        of,
        ent_s,
        ent_i,
        ext_s,
        ext_i,
        n,
    })
}

/// Arcs entering a set of member nodes from outside it, and arcs leaving it, in member order.
pub(crate) fn site_entries(g: &Graph, members: &[usize]) -> (Vec<usize>, Vec<usize>) {
    let ms: std::collections::HashSet<usize> = members.iter().copied().collect();
    let mut inc = Vec::new();
    let mut exits = Vec::new();
    for &mm in members {
        for &i in &g.into[mm] {
            if !ms.contains(&g.arcs[i].from) {
                inc.push(i);
            }
        }
        for &i in &g.out_r[mm] {
            if !ms.contains(&g.arcs[i].to) {
                exits.push(i);
            }
        }
    }
    (inc, exits)
}

/// `siteFlows(G, res, f)`: traced site-movement flows (pcu/h by slot) rebuilt from a result's `res.siteTurns` (JSON), times f.
///
/// Each row adds its pcu (when set and finite), otherwise its v times the graph's PCU factor.
/// None without sites or without siteTurns.
pub fn site_flows(g: &Graph, site_turns: Option<&Value>, f: f64) -> Option<Vec<f64>> {
    let sm = g.sm.as_ref()?;
    let site_turns = site_turns?;
    if !crate::jsnum::value_truthy(site_turns) {
        return None;
    }
    let mut x = vec![0.0f64; sm.n];
    let pf = pcu_f(&g.settings);
    let Value::Object(map) = site_turns else {
        return Some(x);
    };
    for (id, rows) in map {
        let Some(&s) = sm.of.get(id) else {
            continue;
        };
        let Value::Array(rows) = rows else {
            continue;
        };
        let st = &sm.list[s];
        let ei: HashMap<&str, usize> = st
            .inc
            .iter()
            .enumerate()
            .map(|(k, &a)| (g.arcs[a].key.as_str(), k))
            .collect();
        let xi: HashMap<&str, usize> = st
            .exits
            .iter()
            .enumerate()
            .map(|(k, &a)| (g.arcs[a].key.as_str(), k))
            .collect();
        for t in rows {
            let from = js_key(t.get("from"));
            let to = js_key(t.get("to"));
            let (Some(&e), Some(&o)) = (ei.get(from.as_str()), xi.get(to.as_str())) else {
                continue;
            };
            let pcu = t.get("pcu");
            let add = match pcu {
                Some(p) if !p.is_null() && value_to_number(p).is_finite() => value_to_number(p),
                _ => crate::jsnum::or_default(t.get("v").map_or(f64::NAN, value_to_number), 0.0) * pf,
            };
            x[st.off + e * st.nx + o] += add * f;
        }
    }
    Some(x)
}

// A Map lookup key from a row field: the text itself (other values never equal an arc key)
fn js_key(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        _ => "\u{0}not a key".to_string(),
    }
}

/// `tracedExits(G, s, xm)`: for each entry arc of site s, the exit arcs with flow and their pcu/h, from traced flows xm.
pub fn traced_exits(g: &Graph, s: usize, xm: &[f64]) -> Vec<(usize, Vec<(usize, f64)>)> {
    let st = &g.sm.as_ref().expect("site movement slots").list[s];
    let mut out: Vec<(usize, Vec<(usize, f64)>)> = Vec::new();
    for (e, &a) in st.inc.iter().enumerate() {
        let mut m: Vec<(usize, f64)> = Vec::new();
        for x in 0..st.nx {
            let v = xm[st.off + e * st.nx + x];
            if v > 0.0 {
                map_set(&mut m, st.exits[x], v);
            }
        }
        map_set(&mut out, a, m);
    }
    out
}

/// `map.set(k, v)` on a list kept in insertion order.
pub(crate) fn map_set<V>(m: &mut Vec<(usize, V)>, k: usize, v: V) {
    if let Some(e) = m.iter_mut().find(|e| e.0 == k) {
        e.1 = v;
    } else {
        m.push((k, v));
    }
}
