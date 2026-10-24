//! The recursion check's call graph, its recursive cycles, and the kernel's recursive types.
//!
//! What it does: [`Graph`] holds a node for every function body, derived implementation and
//! closure, and the carrier nodes through which callables flow: one for each parameter of each
//! function, each field of each kernel type, each function's return value and each constant's
//! value, and one ([`Graph::any`]) for a call through a value the analysis cannot follow, which
//! leads to every closure and every function whose address is taken. An edge `a -> b` says that
//! running `a` may call `b` (or, from a carrier, that the callables it holds include `b`).
//! [`Graph::cycles`] finds the strongly connected components that hold a call cycle and names
//! each by its functions; [`type_cycles`] finds the kernel's structs, enums and unions that
//! contain themselves, whose drop glue and derived implementations recurse over them.
//!
//! Guarantees:
//!
//! - **Every cycle.** A cycle of the graph lies inside one strongly connected component, and every
//!   component with more than one node, or with an edge to itself, is reported, unless it holds no
//!   function, derived implementation or closure (carriers alone run nothing) or lies wholly in a
//!   crate's bound tests.
//! - **Named as written.** A cycle's members are the names of its functions and derived
//!   implementations, with each closure counted as the function it is written in, so a statement
//!   in a module header can name it; a shortest path round the cycle, with the file and line of
//!   each call, shows how it closes.
//! - **Deterministic.** Nodes, edges and cycles are ordered by id and name.
//!
//! Method: strongly connected components by Kosaraju's algorithm (S. R. Kosaraju, 1978; as
//! published in M. Sharir, "A strong-connectivity algorithm and its applications in data flow
//! analysis", *Computers & Mathematics with Applications* 7 (1981),
//! <https://doi.org/10.1016/0898-1221(81)90008-0>), from `petgraph`, whose implementation is
//! iterative; shortest cycle paths by breadth-first search (Cormen, Leiserson, Rivest and Stein,
//! *Introduction to Algorithms*, 3rd edition, section 22.2).
//!
//! Cost: linear in nodes and edges for the components, plus one breadth-first search per cycle.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use petgraph::graph::{DiGraph, NodeIndex};

use crate::recursion::index::Index;
use crate::recursion::model::{ItemKind, NodeKind, Role};
use crate::recursion::ty::{ItemId, NodeId, Ty};

/// What a graph node is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GKind {
    /// A function, method or trait default body.
    Code,
    /// A derived implementation.
    Derived,
    /// A closure, written inside `owner`.
    Closure,
    /// A carrier: a parameter, field, return value or constant value.
    Carrier,
    /// The node a call through an unknown value leads to.
    Any,
}

/// A node of the graph.
#[derive(Clone, Debug)]
pub struct GNode {
    /// Its name.
    pub name: String,
    /// What it is.
    pub kind: GKind,
    /// The named function or derived implementation it is counted as: itself, or for a closure
    /// the function it is written in.
    pub owner: NodeId,
    /// Its file, relative to the workspace root.
    pub file: String,
    /// Its line.
    pub line: usize,
    /// The crate it belongs to.
    pub krate: usize,
}

/// Where a call is written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    /// The file, relative to the workspace root.
    pub file: String,
    /// The line.
    pub line: usize,
    /// How the call was found: by path, by the receiver's type, through a bound, by name, as an
    /// implicit trait call, as a callback of outside code, through a value, or a callable flowing
    /// into a carrier.
    pub how: &'static str,
}

/// The call graph.
#[derive(Clone, Debug, Default)]
pub struct Graph {
    /// Every node, by id: the program's nodes first, with the same ids.
    pub nodes: Vec<GNode>,
    /// The edges from each node, each with the first site seen.
    pub edges: BTreeMap<NodeId, BTreeMap<NodeId, Site>>,
    /// Functions used as values (their address taken).
    pub address_taken: BTreeSet<NodeId>,
    closures: BTreeMap<(NodeId, usize), NodeId>,
    params: BTreeMap<(NodeId, usize), NodeId>,
    fields: BTreeMap<(ItemId, Option<usize>, String), NodeId>,
    returns: BTreeMap<NodeId, NodeId>,
    consts: BTreeMap<ItemId, NodeId>,
    any: Option<NodeId>,
    /// The crates that are bound tests, whose cycles are not listed.
    test_crates: BTreeSet<usize>,
}

/// A recursive cycle: a strongly connected component of the call graph holding a call cycle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cycle {
    /// Its functions and derived implementations by name, closures counted as their functions.
    pub members: BTreeSet<String>,
    /// The program nodes those names are (functions and derived implementations).
    pub member_nodes: BTreeSet<NodeId>,
    /// Every node of the component, carriers and closures included.
    pub nodes: BTreeSet<NodeId>,
    /// A shortest path round the cycle from its first member: each node's name and the site of
    /// its call to the next.
    pub path: Vec<(String, Site)>,
}

/// A recursive type: a strongly connected component of the "field mentions" graph of the
/// kernel's structs, enums and unions holding a cycle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeCycle {
    /// The types, by path.
    pub members: BTreeSet<String>,
    /// The items.
    pub items: BTreeSet<ItemId>,
}

impl Graph {
    /// A graph holding the program's nodes and no edges.
    #[must_use]
    pub fn new(ix: &Index<'_>) -> Graph {
        let program = ix.program;
        let nodes = program
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| GNode {
                name: ix.name(NodeId(i)).to_owned(),
                kind: if matches!(n.kind, NodeKind::Derived { .. }) {
                    GKind::Derived
                } else {
                    GKind::Code
                },
                owner: NodeId(i),
                file: n.file.clone(),
                line: n.line,
                krate: n.krate,
            })
            .collect();
        let test_crates = program
            .crates
            .iter()
            .enumerate()
            .filter(|(_, c)| c.source.role == Role::BoundTests)
            .map(|(i, _)| i)
            .collect();
        Graph {
            nodes,
            test_crates,
            ..Graph::default()
        }
    }

    fn push(&mut self, node: GNode) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(node);
        id
    }

    /// The node `id`.
    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&GNode> {
        self.nodes.get(id.0)
    }

    /// The owner of `id`: the named function it is counted as.
    fn owner_of(&self, id: NodeId) -> NodeId {
        self.node(id).map_or(id, |n| n.owner)
    }

    /// The `index`th closure written in node `parent`, made on first use.
    pub fn closure(&mut self, parent: NodeId, index: usize, file: &str, line: usize) -> NodeId {
        if let Some(id) = self.closures.get(&(parent, index)) {
            return *id;
        }
        let owner = self.owner_of(parent);
        let (name, krate) = self.node(parent).map_or((String::new(), 0), |p| {
            (format!("{}::{{closure#{index}}}", p.name), p.krate)
        });
        let id = self.push(GNode {
            name,
            kind: GKind::Closure,
            owner,
            file: file.to_owned(),
            line,
            krate,
        });
        self.closures.insert((parent, index), id);
        id
    }

    fn carrier(&mut self, name: String, near: NodeId) -> NodeId {
        let (file, line, krate) = self
            .node(near)
            .map_or((String::new(), 0, 0), |n| (n.file.clone(), n.line, n.krate));
        self.push(GNode {
            name,
            kind: GKind::Carrier,
            owner: near,
            file,
            line,
            krate,
        })
    }

    /// The carrier of parameter `index` (the receiver is 0) of function `of`.
    pub fn param(&mut self, of: NodeId, index: usize) -> NodeId {
        if let Some(id) = self.params.get(&(of, index)) {
            return *id;
        }
        let name = format!(
            "{} (parameter {index})",
            self.node(of).map_or("", |n| n.name.as_str())
        );
        let id = self.carrier(name, of);
        self.params.insert((of, index), id);
        id
    }

    /// The carrier of field `field` of kernel type `owner` (of its variant `variant`).
    pub fn field(&mut self, ix: &Index<'_>, owner: ItemId, variant: Option<usize>, field: &str) -> NodeId {
        let key = (owner, variant, field.to_owned());
        if let Some(id) = self.fields.get(&key) {
            return *id;
        }
        let item = ix.program.item(owner);
        let variant_name = match (&item.kind, variant) {
            (ItemKind::Enum { variants, .. }, Some(v)) => variants
                .get(v)
                .map_or_else(String::new, |x| format!("::{}", x.name)),
            _ => String::new(),
        };
        let name = format!("{}{variant_name}.{field}", item.path);
        let file = ix.program.module(item.module).file.clone();
        let krate = ix.program.module(item.module).krate;
        let id = self.push(GNode {
            name,
            kind: GKind::Carrier,
            owner: NodeId(usize::MAX),
            file,
            line: item.line,
            krate,
        });
        self.fields.insert(key, id);
        id
    }

    /// The carrier of node `of`'s return value.
    pub fn ret(&mut self, of: NodeId) -> NodeId {
        if let Some(id) = self.returns.get(&of) {
            return *id;
        }
        let name = format!("{} (return)", self.node(of).map_or("", |n| n.name.as_str()));
        let id = self.carrier(name, of);
        self.returns.insert(of, id);
        id
    }

    /// The carrier of constant or static `item`'s value, and whether it was just made (so its
    /// initialiser must be read).
    pub fn constant(&mut self, ix: &Index<'_>, item: ItemId) -> (NodeId, bool) {
        if let Some(id) = self.consts.get(&item) {
            return (*id, false);
        }
        let it = ix.program.item(item);
        let file = ix.program.module(it.module).file.clone();
        let krate = ix.program.module(it.module).krate;
        let id = self.push(GNode {
            name: it.path.clone(),
            kind: GKind::Code,
            owner: NodeId(usize::MAX),
            file,
            line: it.line,
            krate,
        });
        if let Some(n) = self.nodes.get_mut(id.0) {
            n.owner = id;
        }
        self.consts.insert(item, id);
        (id, true)
    }

    /// The node a call through an unknown value leads to.
    pub fn any(&mut self) -> NodeId {
        if let Some(id) = self.any {
            return id;
        }
        let id = self.push(GNode {
            name: "(a call through a value the analysis cannot follow)".to_owned(),
            kind: GKind::Any,
            owner: NodeId(usize::MAX),
            file: String::new(),
            line: 0,
            krate: usize::MAX,
        });
        self.any = Some(id);
        id
    }

    /// Adds the edge `from -> to`, keeping the first site.
    pub fn edge(&mut self, from: NodeId, to: NodeId, site: &Site) {
        self.edges
            .entry(from)
            .or_default()
            .entry(to)
            .or_insert_with(|| site.clone());
    }

    /// Joins the unknown-call node to every closure and every function whose address is taken.
    pub fn finish(&mut self) {
        let Some(any) = self.any else {
            return;
        };
        let targets: Vec<NodeId> = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(i, n)| n.kind == GKind::Closure || self.address_taken.contains(&NodeId(*i)))
            .map(|(i, _)| NodeId(i))
            .collect();
        let site = Site {
            file: String::new(),
            line: 0,
            how: "a call through an unknown value",
        };
        for t in targets {
            self.edge(any, t, &site);
        }
    }

    /// Every recursive cycle, in order of its first member's name.
    #[must_use]
    pub fn cycles(&self) -> Vec<Cycle> {
        let mut g: DiGraph<NodeId, ()> = DiGraph::new();
        let idx: Vec<NodeIndex> = (0..self.nodes.len()).map(|i| g.add_node(NodeId(i))).collect();
        for (from, tos) in &self.edges {
            for to in tos.keys() {
                if let (Some(a), Some(b)) = (idx.get(from.0), idx.get(to.0)) {
                    g.add_edge(*a, *b, ());
                }
            }
        }
        let mut out = Vec::new();
        for scc in petgraph::algo::kosaraju_scc(&g) {
            let nodes: BTreeSet<NodeId> = scc.iter().filter_map(|i| g.node_weight(*i).copied()).collect();
            let looped = match nodes.iter().next() {
                Some(only) if nodes.len() == 1 => {
                    self.edges.get(only).is_some_and(|tos| tos.contains_key(only))
                }
                _ => nodes.len() > 1,
            };
            if !looped {
                continue;
            }
            let mut members = BTreeSet::new();
            let mut member_nodes = BTreeSet::new();
            let mut runs_code = false;
            let mut outside_tests = false;
            for n in &nodes {
                let Some(node) = self.node(*n) else {
                    continue;
                };
                if matches!(node.kind, GKind::Code | GKind::Derived | GKind::Closure) {
                    runs_code = true;
                    if !self.test_crates.contains(&node.krate) {
                        outside_tests = true;
                    }
                    let owner = node.owner;
                    if let Some(o) = self.node(owner) {
                        members.insert(o.name.clone());
                        member_nodes.insert(owner);
                    }
                }
            }
            if !runs_code || !outside_tests {
                continue;
            }
            let path = self.path_round(&nodes);
            out.push(Cycle {
                members,
                member_nodes,
                nodes,
                path,
            });
        }
        out.sort_by(|a, b| a.members.iter().next().cmp(&b.members.iter().next()));
        out
    }

    /// A shortest path round the component from its first code node back to it.
    fn path_round(&self, nodes: &BTreeSet<NodeId>) -> Vec<(String, Site)> {
        let start = nodes
            .iter()
            .filter(|n| self.node(**n).is_some_and(|x| x.kind != GKind::Carrier && x.kind != GKind::Any))
            .min_by(|a, b| {
                let na = self.node(**a).map(|x| x.name.as_str());
                let nb = self.node(**b).map(|x| x.name.as_str());
                na.cmp(&nb)
            });
        let Some(&start) = start else {
            return Vec::new();
        };
        self.path_from(start, nodes)
    }

    /// A shortest path from `start` round the cycle back to it, inside `nodes`: each node's name
    /// and the site of its call to the next. Empty when there is none.
    #[must_use]
    pub fn path_from(&self, start: NodeId, nodes: &BTreeSet<NodeId>) -> Vec<(String, Site)> {
        let mut prev: BTreeMap<NodeId, NodeId> = BTreeMap::new();
        let mut queue: VecDeque<NodeId> = VecDeque::new();
        queue.push_back(start);
        let mut closed = false;
        while let Some(n) = queue.pop_front() {
            for to in self.edges.get(&n).into_iter().flat_map(BTreeMap::keys) {
                if !nodes.contains(to) {
                    continue;
                }
                if *to == start {
                    prev.insert(start, n);
                    closed = true;
                    break;
                }
                if !prev.contains_key(to) {
                    prev.insert(*to, n);
                    queue.push_back(*to);
                }
            }
            if closed {
                break;
            }
        }
        let mut chain = vec![start];
        let mut at = start;
        while let Some(p) = prev.get(&at) {
            if *p == start {
                break;
            }
            chain.push(*p);
            at = *p;
            if chain.len() > nodes.len() {
                break;
            }
        }
        chain.reverse();
        // chain: start, ..., last; each calls the next, and the last calls start.
        let mut out = Vec::new();
        for (i, n) in chain.iter().enumerate() {
            let next = chain.get(i.saturating_add(1)).copied().unwrap_or(start);
            let site = self
                .edges
                .get(n)
                .and_then(|tos| tos.get(&next))
                .cloned()
                .unwrap_or(Site {
                    file: String::new(),
                    line: 0,
                    how: "",
                });
            let name = self.node(*n).map_or_else(String::new, |x| x.name.clone());
            out.push((name, site));
        }
        out
    }
}

/// The kernel types each field type of `item` mentions, outside `PhantomData` and the argument
/// lists of function pointers and closure traits.
fn mentioned(ix: &Index<'_>, item: ItemId) -> BTreeSet<ItemId> {
    let it = ix.program.item(item);
    let fields: Vec<&syn::Type> = match &it.kind {
        ItemKind::Struct { fields, .. } | ItemKind::Union { fields, .. } => {
            fields.all().into_iter().map(|(_, t)| t).collect()
        }
        ItemKind::Enum { variants, .. } => variants
            .iter()
            .flat_map(|v| v.fields.all().into_iter().map(|(_, t)| t))
            .collect(),
        _ => Vec::new(),
    };
    let mut out = BTreeSet::new();
    for f in fields {
        let mut work = vec![ix.resolver.ty(&it.scope, f)];
        while let Some(t) = work.pop() {
            match t {
                Ty::Adt(id, args) => {
                    out.insert(id);
                    work.extend(args);
                }
                Ty::Ext(name, args) => {
                    if name != "PhantomData" {
                        work.extend(args);
                    }
                }
                Ty::Tuple(items) => work.extend(items),
                Ty::Ref(inner) | Ty::Slice(inner) | Ty::Array(inner) => work.push(*inner),
                _ => {}
            }
        }
    }
    out
}

/// Every recursive type of the crates that ship, in order of its first member's path.
#[must_use]
pub fn type_cycles(ix: &Index<'_>) -> Vec<TypeCycle> {
    let program = ix.program;
    let ships = |item: &crate::recursion::model::Item| {
        let krate = program.module(item.module).krate;
        program
            .crates
            .get(krate)
            .is_some_and(|c| c.source.role == Role::Ships)
    };
    let mut g: DiGraph<ItemId, ()> = DiGraph::new();
    let mut idx: BTreeMap<ItemId, NodeIndex> = BTreeMap::new();
    for (i, item) in program.items.iter().enumerate() {
        if item.is_adt() && ships(item) {
            idx.insert(ItemId(i), g.add_node(ItemId(i)));
        }
    }
    let mut self_loops = BTreeSet::new();
    for (id, a) in &idx {
        for m in mentioned(ix, *id) {
            if m == *id {
                self_loops.insert(*id);
            }
            if let Some(b) = idx.get(&m) {
                g.add_edge(*a, *b, ());
            }
        }
    }
    let mut out = Vec::new();
    for scc in petgraph::algo::kosaraju_scc(&g) {
        let items: BTreeSet<ItemId> = scc.iter().filter_map(|i| g.node_weight(*i).copied()).collect();
        let looped = items.len() > 1 || items.iter().any(|i| self_loops.contains(i));
        if !looped {
            continue;
        }
        let members = items.iter().map(|i| program.item(*i).path.clone()).collect();
        out.push(TypeCycle { members, items });
    }
    out.sort_by(|a, b| a.members.iter().next().cmp(&b.members.iter().next()));
    out
}
