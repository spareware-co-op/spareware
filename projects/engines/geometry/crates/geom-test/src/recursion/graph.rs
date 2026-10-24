//! The recursion check's call graph, its recursive cycles, and the kernel's recursive types.
//!
//! What it does: [`Graph`] holds a node for every function body, derived implementation and
//! closure, and the carrier nodes through which callables flow: one for each parameter of each
//! function, each field of each kernel type, each function's return value and each constant's
//! value, and one ([`Graph::any`]) for a call through a value the analysis cannot follow, which
//! leads to every closure and every function whose address is taken. An edge `a -> b` says that
//! running `a` may call `b` (or, from a carrier, that the callables it holds include `b`).
//! Each edge also carries the size-change graphs of its call sites ([`crate::recursion::sizechange`]):
//! what the call states about the instantiation of the callee's type parameters relative to the
//! caller's. [`Graph::cycles`] finds the strongly connected components that hold a call cycle,
//! keeps the edges that lie on a closed walk some finite instantiation can run, and names each
//! strongly connected component of those edges by its functions; [`type_cycles`] finds the
//! kernel's structs, enums and unions that contain themselves, whose drop glue and derived
//! implementations recurse over them.
//!
//! Guarantees:
//!
//! - **Every cycle.** A cycle of running code is a closed walk of the graph whose instantiations
//!   come back to where they started, so its edges are kept, it lies inside one strongly connected
//!   component of the kept edges, and every such component with more than one node, or with an
//!   edge to itself, is reported, unless it holds no function, derived implementation or closure
//!   (carriers alone run nothing) or lies wholly in a crate's bound tests. Only closed walks whose
//!   composed size-change graph forces a type parameter strictly below or above itself are left
//!   out: generic recursion through ever smaller (or larger) types, which Rust's one copy per
//!   instantiation cannot run as a cycle.
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
//! *Introduction to Algorithms*, 3rd edition, section 22.2); the closure of a component's
//! size-change graphs by a work list, as Lee, Jones and Ben-Amram compute it (POPL 2001,
//! <https://doi.org/10.1145/360204.360210>).
//!
//! Cost: linear in nodes and edges for the components, plus one breadth-first search per cycle;
//! for a component whose edges state anything about type parameters, one work list per node over
//! the distinct size-change graphs of walks inside it, which are few because nodes have a handful
//! of type parameters.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use petgraph::graph::{DiGraph, NodeIndex};

use crate::recursion::index::Index;
use crate::recursion::model::{ItemKind, NodeKind, Role};
use crate::recursion::sizechange::{Labels, Scg};
use crate::recursion::ty::{GenId, ItemId, NodeId, Ty};

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
    /// A function from outside the kernel used as a value (`i128::from` passed to `map`): calling
    /// it is a call of outside code with the arguments.
    Outside,
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
    /// Its type parameters, in the order size-change graphs number them: a function's own and its
    /// implementation's or trait's (with the trait's `Self` last), a derived implementation's
    /// type's, a closure's function's; none for a carrier.
    pub vars: Vec<GenId>,
    /// For a closure, how many parameters it takes.
    pub arity: Option<usize>,
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
    /// The size-change graphs of each edge's call sites.
    pub labels: BTreeMap<(NodeId, NodeId), Labels>,
    /// Functions used as values (their address taken).
    pub address_taken: BTreeSet<NodeId>,
    closures: BTreeMap<(NodeId, usize), NodeId>,
    params: BTreeMap<(NodeId, usize), NodeId>,
    fields: BTreeMap<(ItemId, Option<usize>, String), NodeId>,
    returns: BTreeMap<NodeId, NodeId>,
    consts: BTreeMap<ItemId, NodeId>,
    any: Option<NodeId>,
    outside: Option<NodeId>,
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
    /// The edges inside it that lie on a closed walk some instantiation can run.
    pub edges: BTreeSet<(NodeId, NodeId)>,
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
                vars: vars_of(ix, NodeId(i)),
                arity: None,
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

    /// The `index`th closure written in node `parent`, taking `arity` parameters, made on first
    /// use.
    pub fn closure(&mut self, parent: NodeId, index: usize, arity: usize, file: &str, line: usize) -> NodeId {
        if let Some(id) = self.closures.get(&(parent, index)) {
            return *id;
        }
        let owner = self.owner_of(parent);
        let (name, krate, vars) = self.node(parent).map_or((String::new(), 0, Vec::new()), |p| {
            (format!("{}::{{closure#{index}}}", p.name), p.krate, p.vars.clone())
        });
        let id = self.push(GNode {
            name,
            kind: GKind::Closure,
            owner,
            file: file.to_owned(),
            line,
            krate,
            vars,
            arity: Some(arity),
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
            vars: Vec::new(),
            arity: None,
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
            vars: Vec::new(),
            arity: None,
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
            vars: Vec::new(),
            arity: None,
        });
        if let Some(n) = self.nodes.get_mut(id.0) {
            n.owner = id;
        }
        self.consts.insert(item, id);
        (id, true)
    }

    /// The node that stands for every function from outside the kernel used as a value.
    pub fn outside(&mut self) -> NodeId {
        if let Some(id) = self.outside {
            return id;
        }
        let id = self.push(GNode {
            name: "(a function from outside the kernel, used as a value)".to_owned(),
            kind: GKind::Outside,
            owner: NodeId(usize::MAX),
            file: String::new(),
            line: 0,
            krate: usize::MAX,
            vars: Vec::new(),
            arity: None,
        });
        self.outside = Some(id);
        id
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
            vars: Vec::new(),
            arity: None,
        });
        self.any = Some(id);
        id
    }

    /// Adds the edge `from -> to`, keeping the first site, with a call that states nothing about
    /// type parameters.
    pub fn edge(&mut self, from: NodeId, to: NodeId, site: &Site) {
        self.edge_rel(from, to, site, Scg::empty());
    }

    /// Adds the edge `from -> to`, keeping the first site, with the size-change graph `scg` of
    /// this call site.
    pub fn edge_rel(&mut self, from: NodeId, to: NodeId, site: &Site, scg: Scg) {
        self.edges
            .entry(from)
            .or_default()
            .entry(to)
            .or_insert_with(|| site.clone());
        self.labels.entry((from, to)).or_default().insert(scg);
    }

    /// The type parameters of node `id`.
    #[must_use]
    pub fn vars(&self, id: NodeId) -> &[GenId] {
        self.node(id).map_or(&[], |n| n.vars.as_slice())
    }

    /// The size-change graphs of the edge `from -> to`: none stated reads as one that states
    /// nothing.
    fn label(&self, from: NodeId, to: NodeId) -> Vec<Scg> {
        match self.labels.get(&(from, to)) {
            Some(l) => l.iter().cloned().collect(),
            None => vec![Scg::empty()],
        }
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
        let all: BTreeSet<(NodeId, NodeId)> = self
            .edges
            .iter()
            .flat_map(|(from, tos)| tos.keys().map(move |to| (*from, *to)))
            .collect();
        let mut out = Vec::new();
        for scc in components(self.nodes.len(), &all) {
            let inner = inside(&all, &scc);
            if !looped(&scc, &inner) {
                continue;
            }
            let kept = self.realisable_edges(&scc, &inner);
            if kept.len() == inner.len() {
                self.push_cycle(&mut out, scc, kept);
                continue;
            }
            for sub in components(self.nodes.len(), &kept) {
                if !sub.iter().all(|n| scc.contains(n)) {
                    continue;
                }
                let sub_edges = inside(&kept, &sub);
                if looped(&sub, &sub_edges) {
                    self.push_cycle(&mut out, sub, sub_edges);
                }
            }
        }
        out.sort_by(|a, b| a.members.iter().next().cmp(&b.members.iter().next()));
        out
    }

    /// Adds the cycle of component `nodes` with edges `edges` to `out`, unless it runs no code or
    /// lies wholly in bound tests.
    fn push_cycle(&self, out: &mut Vec<Cycle>, nodes: BTreeSet<NodeId>, edges: BTreeSet<(NodeId, NodeId)>) {
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
            return;
        }
        let path = self.path_round(&nodes, &edges);
        out.push(Cycle {
            members,
            member_nodes,
            nodes,
            edges,
            path,
        });
    }

    /// The edges of component `scc` (its edges `inner`) that lie on a closed walk inside it whose
    /// composed size-change graph can come back to the instantiation it started from.
    fn realisable_edges(
        &self,
        scc: &BTreeSet<NodeId>,
        inner: &BTreeSet<(NodeId, NodeId)>,
    ) -> BTreeSet<(NodeId, NodeId)> {
        let stated = inner
            .iter()
            .any(|e| self.labels.get(e).is_some_and(|l| !l.states_nothing()));
        if !stated {
            return inner.clone();
        }
        let mut out_edges: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
        for (a, b) in inner {
            out_edges.entry(*a).or_default().push(*b);
        }
        let walks: BTreeMap<NodeId, BTreeMap<NodeId, Labels>> = scc
            .iter()
            .map(|s| (*s, self.walks_from(*s, &out_edges)))
            .collect();
        let mut kept = BTreeSet::new();
        for &(a, b) in inner {
            let n = self.vars(a).len();
            let mut back: Vec<Scg> = walks
                .get(&b)
                .and_then(|w| w.get(&a))
                .map(|l| l.iter().cloned().collect())
                .unwrap_or_default();
            if a == b {
                back.push(Scg::identity(n));
            }
            let realisable = self
                .label(a, b)
                .iter()
                .any(|g| back.iter().any(|w| g.then(w).closes(n)));
            if realisable {
                kept.insert((a, b));
            }
        }
        kept
    }

    /// The size-change graphs of every walk of one or more edges from `start` inside a
    /// component, by the node the walk ends at.
    fn walks_from(&self, start: NodeId, out_edges: &BTreeMap<NodeId, Vec<NodeId>>) -> BTreeMap<NodeId, Labels> {
        let mut reached: BTreeMap<NodeId, Labels> = BTreeMap::new();
        let mut work: Vec<(NodeId, Scg)> = Vec::new();
        for b in out_edges.get(&start).into_iter().flatten() {
            for g in self.label(start, *b) {
                if reached.entry(*b).or_default().insert(g.clone()) {
                    work.push((*b, g));
                }
            }
        }
        while let Some((m, g)) = work.pop() {
            for c in out_edges.get(&m).into_iter().flatten() {
                for h in self.label(m, *c) {
                    let k = g.then(&h);
                    if reached.entry(*c).or_default().insert(k.clone()) {
                        work.push((*c, k));
                    }
                }
            }
        }
        reached
    }

    /// A shortest path round the component from its first code node back to it, along `edges`.
    fn path_round(&self, nodes: &BTreeSet<NodeId>, edges: &BTreeSet<(NodeId, NodeId)>) -> Vec<(String, Site)> {
        let start = nodes
            .iter()
            .filter(|n| self.node(**n).is_some_and(|x| matches!(x.kind, GKind::Code | GKind::Derived | GKind::Closure)))
            .min_by(|a, b| {
                let na = self.node(**a).map(|x| x.name.as_str());
                let nb = self.node(**b).map(|x| x.name.as_str());
                na.cmp(&nb)
            });
        let Some(&start) = start else {
            return Vec::new();
        };
        self.path_from(start, edges)
    }

    /// A shortest path from `start` round the cycle back to it, along `edges`: each node's name
    /// and the site of its call to the next. Empty when there is none.
    #[must_use]
    pub fn path_from(&self, start: NodeId, edges: &BTreeSet<(NodeId, NodeId)>) -> Vec<(String, Site)> {
        let mut prev: BTreeMap<NodeId, NodeId> = BTreeMap::new();
        let mut queue: VecDeque<NodeId> = VecDeque::new();
        queue.push_back(start);
        let mut closed = false;
        while let Some(n) = queue.pop_front() {
            for to in self.edges.get(&n).into_iter().flat_map(BTreeMap::keys) {
                if !edges.contains(&(n, *to)) {
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
            if chain.len() > edges.len() {
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

/// The strongly connected components of the graph on `n` nodes with `edges`, each as its set of
/// nodes.
fn components(n: usize, edges: &BTreeSet<(NodeId, NodeId)>) -> Vec<BTreeSet<NodeId>> {
    let mut g: DiGraph<NodeId, ()> = DiGraph::new();
    let idx: Vec<NodeIndex> = (0..n).map(|i| g.add_node(NodeId(i))).collect();
    for (from, to) in edges {
        if let (Some(a), Some(b)) = (idx.get(from.0), idx.get(to.0)) {
            g.add_edge(*a, *b, ());
        }
    }
    petgraph::algo::kosaraju_scc(&g)
        .into_iter()
        .map(|scc| scc.iter().filter_map(|i| g.node_weight(*i).copied()).collect())
        .collect()
}

/// The edges of `edges` with both ends in `nodes`.
fn inside(edges: &BTreeSet<(NodeId, NodeId)>, nodes: &BTreeSet<NodeId>) -> BTreeSet<(NodeId, NodeId)> {
    edges
        .iter()
        .filter(|(a, b)| nodes.contains(a) && nodes.contains(b))
        .copied()
        .collect()
}

/// Whether a component with these inner edges holds a cycle: two or more nodes, or an edge to
/// itself.
fn looped(nodes: &BTreeSet<NodeId>, inner: &BTreeSet<(NodeId, NodeId)>) -> bool {
    nodes.len() > 1 || inner.iter().any(|(a, b)| a == b)
}

/// The type parameters of program node `id`, as [`GNode::vars`] lists them.
fn vars_of(ix: &Index<'_>, id: NodeId) -> Vec<GenId> {
    let Some(node) = ix.program.node(id) else {
        return Vec::new();
    };
    match &node.kind {
        NodeKind::Derived { item, .. } => ix.program.item(*item).scope.generics.clone(),
        NodeKind::TraitDefault(t) => {
            let mut v = node.scope.generics.clone();
            if let ItemKind::Trait { self_gen, .. } = &ix.program.item(*t).kind
                && !v.contains(self_gen)
            {
                v.push(*self_gen);
            }
            v
        }
        NodeKind::Fn | NodeKind::Method(_) => node.scope.generics.clone(),
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
