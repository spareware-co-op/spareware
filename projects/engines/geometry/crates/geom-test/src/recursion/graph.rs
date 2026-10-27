//! The recursion check's cycles: the recursive cycles of the instance call graph, named by the
//! functions they run, and the kernel's recursive types.
//!
//! What it does: [`cycles`] finds the strongly connected components of the call graph over
//! instances ([`crate::recursion::mono`]) that hold a call cycle (more than one instance, or an
//! instance that calls itself), names each by the functions and derived implementations its
//! instances are copies of (a closure counted as the function it is written in), and joins
//! components that share a function into one [`Cycle`], since a statement names functions as
//! written. Each cycle carries a shortest path round it, with the file and line of each call and
//! how it was found. [`type_cycles`] finds the kernel's structs, enums and unions that contain
//! themselves, through any field type: their drop glue recurses over their nesting.
//!
//! Guarantees:
//!
//! - **Every cycle.** A function that can be re-entered while it runs lies on a closed walk of
//!   the instance graph, which lies inside one strongly connected component, and every component
//!   holding a closed walk is reported, unless none of its instances belongs to a crate that
//!   ships (the bound tests exercise cycles; the standard library's model is not checked).
//! - **Every recursive type.** A type that contains itself through its fields' types, at any
//!   depth and through the arguments of the standard library's types (`Box<T>`, `Vec<T>`,
//!   `Option<T>`), lies on a cycle of the field graph and is reported.
//! - **Deterministic.** Cycles come in the order of their first member's name, members and paths
//!   in a fixed order.
//!
//! Method: strongly connected components by Tarjan's algorithm (R. E. Tarjan, "Depth-first search
//! and linear graph algorithms", SIAM J. Computing 1 (1972), <https://doi.org/10.1137/0201010>),
//! from `petgraph`, whose implementation is iterative; components joined by union-find over
//! shared names (R. E. Tarjan, "Efficiency of a good but not linear set union algorithm", J. ACM
//! 22 (1975), <https://doi.org/10.1145/321879.321884>); shortest cycle paths by breadth-first
//! search (Cormen, Leiserson, Rivest and Stein, *Introduction to Algorithms*, 3rd edition, section
//! 22.2).
//!
//! Cost: linear in instances and edges for the components, plus one breadth-first search per
//! cycle.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::recursion::index::Index;
use crate::recursion::model::{NodeKind, Role};
use crate::recursion::mono::{InstId, InstKind, Mono, Site};
use crate::recursion::ty::{ItemId, NodeId, Ty};

/// A recursive cycle: the instances of one or more strongly connected components of the call
/// graph that share a function, named by their functions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cycle {
    /// Its functions and derived implementations by name, closures counted as their functions.
    pub members: BTreeSet<String>,
    /// The program nodes those names are.
    pub member_nodes: BTreeSet<NodeId>,
    /// Every instance of its components.
    pub insts: BTreeSet<InstId>,
    /// A shortest path round one of its components, from its first instance: each instance's
    /// name and the site of its call to the next.
    pub path: Vec<(String, Site)>,
}

/// A group of recursive types: a strongly connected component of the field graph of the kernel's
/// structs, enums and unions that holds a cycle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeCycle {
    /// The types, by path.
    pub members: BTreeSet<String>,
    /// The items.
    pub items: BTreeSet<ItemId>,
}

/// The name an instance is listed by: its function's, and for a closure its function's with the
/// closure's place.
#[must_use]
pub fn inst_name(ix: &Index<'_>, m: &Mono, id: InstId) -> String {
    match m.inst(id).map(|i| &i.kind) {
        Some(InstKind::Node(n, args)) => {
            let base = ix.name(*n).to_owned();
            if args.is_empty() || args.iter().all(|a| matches!(a, Ty::Param(_))) {
                base
            } else {
                let shown: Vec<String> = args.iter().map(|a| ix.show(a)).collect();
                format!("{base} [{}]", shown.join(", "))
            }
        }
        Some(InstKind::Closure(k)) => format!("{}::{{closure#{}}}", ix.name(k.owner), k.index),
        None => String::new(),
    }
}

/// Every recursive cycle of `m`'s call graph.
#[must_use]
#[allow(
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "the components, their joining and their naming, in order; the graph's usual names"
)]
pub fn cycles(ix: &Index<'_>, m: &Mono) -> Vec<Cycle> {
    let n = m.insts.len();
    let live = m.reachable();
    let mut g: petgraph::graph::DiGraph<(), ()> = petgraph::graph::DiGraph::with_capacity(n, 0);
    let nodes: Vec<petgraph::graph::NodeIndex> = (0..n).map(|_| g.add_node(())).collect();
    for (from, tos) in &m.edges {
        if !live.contains(from) {
            continue;
        }
        for to in tos.keys() {
            if let (Some(a), Some(b)) = (nodes.get(from.0), nodes.get(to.0)) {
                g.add_edge(*a, *b, ());
            }
        }
    }
    // Only the crates that ship are listed: the bound tests exercise cycles, and the standard
    // library's model is read for what it calls back, not checked.
    let shipping: BTreeSet<usize> = ix
        .program
        .crates
        .iter()
        .enumerate()
        .filter(|(_, c)| c.source.role == Role::Ships)
        .map(|(i, _)| i)
        .collect();
    let mut comps: Vec<BTreeSet<InstId>> = Vec::new();
    for scc in petgraph::algo::tarjan_scc(&g) {
        let set: BTreeSet<InstId> = scc.iter().map(|x| InstId(x.index())).collect();
        let looped = set.len() > 1
            || set
                .iter()
                .any(|i| m.edges.get(i).is_some_and(|e| e.contains_key(i)));
        if !looped {
            continue;
        }
        let ships = set.iter().any(|i| {
            m.inst(*i)
                .and_then(|x| ix.program.node(x.node))
                .is_some_and(|node| shipping.contains(&node.krate))
        });
        if ships {
            comps.push(set);
        }
    }
    // Join components that share a function.
    let mut parent: Vec<usize> = (0..comps.len()).collect();
    let find = |parent: &mut Vec<usize>, mut x: usize| {
        while let Some(&p) = parent.get(x) {
            if p == x {
                break;
            }
            let gp = parent.get(p).copied().unwrap_or(p);
            if let Some(slot) = parent.get_mut(x) {
                *slot = gp;
            }
            x = p;
        }
        x
    };
    let mut owner_of: BTreeMap<NodeId, usize> = BTreeMap::new();
    for (ci, c) in comps.iter().enumerate() {
        for i in c {
            let Some(node) = m.inst(*i).map(|x| x.node) else {
                continue;
            };
            match owner_of.get(&node) {
                Some(&other) => {
                    let a = find(&mut parent, ci);
                    let b = find(&mut parent, other);
                    if let Some(slot) = parent.get_mut(a.max(b)) {
                        *slot = a.min(b);
                    }
                }
                None => {
                    owner_of.insert(node, ci);
                }
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for ci in 0..comps.len() {
        let root = find(&mut parent, ci);
        groups.entry(root).or_default().push(ci);
    }
    let mut out: Vec<Cycle> = Vec::new();
    for members in groups.values() {
        let mut insts = BTreeSet::new();
        for ci in members {
            if let Some(c) = comps.get(*ci) {
                insts.extend(c.iter().copied());
            }
        }
        let member_nodes: BTreeSet<NodeId> = insts
            .iter()
            .filter_map(|i| m.inst(*i).map(|x| x.node))
            .filter(|node| {
                ix.program
                    .node(*node)
                    .is_some_and(|nd| shipping.contains(&nd.krate))
            })
            .collect();
        let names: BTreeSet<String> = member_nodes
            .iter()
            .map(|n| ix.name(*n).to_owned())
            .collect();
        let first = members
            .first()
            .and_then(|ci| comps.get(*ci))
            .cloned()
            .unwrap_or_default();
        let path = path_round(ix, m, &first);
        out.push(Cycle {
            members: names,
            member_nodes,
            insts,
            path,
        });
    }
    out.sort_by(|a, b| a.members.iter().next().cmp(&b.members.iter().next()));
    out
}

/// A shortest path round component `comp`, from its first instance back to it.
fn path_round(ix: &Index<'_>, m: &Mono, comp: &BTreeSet<InstId>) -> Vec<(String, Site)> {
    let Some(&start) = comp.iter().next() else {
        return Vec::new();
    };
    let mut prev: BTreeMap<InstId, (InstId, Site)> = BTreeMap::new();
    let mut queue = VecDeque::from([start]);
    let mut seen = BTreeSet::new();
    while let Some(x) = queue.pop_front() {
        for (to, site) in m.edges.get(&x).into_iter().flatten() {
            if !comp.contains(to) {
                continue;
            }
            if *to == start {
                prev.insert(start, (x, site.clone()));
                queue.clear();
                break;
            }
            if seen.insert(*to) {
                prev.insert(*to, (x, site.clone()));
                queue.push_back(*to);
            }
        }
    }
    let mut steps: Vec<(InstId, Site)> = Vec::new();
    let mut cur = start;
    for _ in 0..=comp.len() {
        let Some((from, site)) = prev.get(&cur).cloned() else {
            break;
        };
        steps.push((from, site));
        cur = from;
        if cur == start {
            break;
        }
    }
    steps.reverse();
    steps
        .into_iter()
        .map(|(i, site)| (inst_name(ix, m, i), site))
        .collect()
}

/// Every group of the kernel's structs, enums and unions that contain themselves through their
/// fields' types.
#[must_use]
pub fn type_cycles(ix: &Index<'_>) -> Vec<TypeCycle> {
    let program = ix.program;
    let adts: Vec<ItemId> = program
        .items
        .iter()
        .enumerate()
        .filter(|(_, it)| it.is_adt())
        .filter(|(_, it)| {
            program
                .crates
                .get(program.module(it.module).krate)
                .is_some_and(|c| c.source.role == Role::Ships)
        })
        .map(|(i, _)| ItemId(i))
        .collect();
    let mut g: petgraph::graph::DiGraph<ItemId, ()> = petgraph::graph::DiGraph::new();
    let mut idx: BTreeMap<ItemId, petgraph::graph::NodeIndex> = BTreeMap::new();
    for a in &adts {
        idx.insert(*a, g.add_node(*a));
    }
    let mut self_loops = BTreeSet::new();
    for a in &adts {
        for t in ix.all_field_types(*a) {
            let mut inside: Vec<ItemId> = Vec::new();
            t.walk(&mut |p| {
                if let Ty::Adt(b, _) = p {
                    inside.push(*b);
                }
            });
            for b in inside {
                if let (Some(x), Some(y)) = (idx.get(a), idx.get(&b)) {
                    g.update_edge(*x, *y, ());
                    if *a == b {
                        self_loops.insert(*a);
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    for scc in petgraph::algo::tarjan_scc(&g) {
        let items: BTreeSet<ItemId> = scc
            .iter()
            .filter_map(|n| g.node_weight(*n).copied())
            .collect();
        if items.len() < 2 && !items.iter().any(|i| self_loops.contains(i)) {
            continue;
        }
        let members = items
            .iter()
            .map(|i| program.item(*i).path.clone())
            .collect();
        out.push(TypeCycle { members, items });
    }
    out.sort_by(|a, b| a.members.iter().next().cmp(&b.members.iter().next()));
    out
}

/// Whether the derived implementations of node `n` are of an item.
#[must_use]
pub fn derived_item(ix: &Index<'_>, n: NodeId) -> Option<ItemId> {
    match ix.program.node(n).map(|x| &x.kind) {
        Some(NodeKind::Derived { item, .. }) => Some(*item),
        _ => None,
    }
}
