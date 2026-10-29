//! Instances: the recursion check's call graph over functions as the compiler copies them, one
//! per instantiation of their generic parameters, and the work list that builds it.
//!
//! What it does: Rust compiles one copy of a generic function for each instantiation of its type
//! parameters, and a closure is a type of its own whose body belongs to the copy of the function
//! that writes it. [`analyse`] builds the call graph over those copies ([`Inst`]): its roots are
//! every function, method, trait default body and derived implementation of the crates that ship
//! and of the bound tests, a generic one with its parameters left open ([`Ty::Param`]: the copy
//! that stands for every instantiation from outside the crates read). It checks each body
//! ([`crate::recursion::typeck`]) and adds, for every call, the copy the call reaches with the
//! generic arguments the checker inferred, queueing new copies. A body is checked again when
//! something it read grows: the hidden type a callee's `impl Trait` result turned out to be, or
//! the callables coerced to closure trait objects and function pointers anywhere, which a call
//! through one may run. Each check of a body replaces that body's edges and problems.
//!
//! Guarantees:
//!
//! - **Every copy a call can reach is an instance**, with the generic arguments the call fixes; a
//!   type nested deeper than [`crate::recursion::ty::MAX_DEPTH`] is cut to an unknown part, and a
//!   function with more than [`MAX_COPIES`] copies is reported, so the graph is finite.
//! - **Generic parameters left open.** Dispatch on a receiver of a bare generic parameter reaches
//!   the trait's default body and blanket implementations, never the other implementations
//!   ([`crate::recursion::index::Pick::Bound`]), and a call through a bare generic callable reaches
//!   nothing. This loses no cycle: a cycle runs concrete copies, and every implementation and
//!   closure is a root of its own (a non-generic one is checked with its exact types, a generic
//!   one through its own open copy), from which the checker follows the cycle with types whose
//!   heads are known, dispatching exactly; and a cycle whose every copy is generic and that passes
//!   through a bare parameter's implementation would need a type that contains itself. What it
//!   avoids is the false cycle a generic implementation would form with its own copy at a nested
//!   type (`Vec<Vec<T>>`'s `clone` calling `Vec<T>`'s). Callables an open parameter stands for come
//!   from outside the crates read, and recursion through them is their caller's.
//! - **A fixed point.** Hidden result types only become known and the callable sets only grow,
//!   and a body is checked again only when something it read changed; a body checked more than
//!   [`MAX_RUNS`] times is reported.
//! - **Deterministic.** Instances are numbered in the order they are found, the work list is
//!   first in, first out, and every table is ordered.
//!
//! Method: monomorphisation as rustc's collector performs it
//! (<https://rustc-dev-guide.rust-lang.org/backend/monomorph.html>), here over the source with the
//! checker's types, closures as types (The Rust Reference,
//! <https://doc.rust-lang.org/reference/types/closure.html>); chaotic iteration to the least fixed
//! point with a work list (P. Cousot and R. Cousot, POPL 1977,
//! <https://doi.org/10.1145/512950.512973>; F. Nielson, H. R. Nielson and C. Hankin, *Principles
//! of Program Analysis*, Springer 1999, section 1.6).
//!
//! Cost: one check of each instance's body, plus one more each time something it read changed.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::recursion::Problem;
use crate::recursion::index::Index;
use crate::recursion::model::{NodeKind, Role};
use crate::recursion::ty::{ClosureKey, NodeId, Ty};

/// How many copies of one function the graph holds before it is reported: a bound that keeps the
/// graph finite, not a rule of the code. With the Phase 2 pieces through G62 the generic
/// `IntoIterator::into_iter` passed 512 copies; 2048 leaves room for the kernel to grow.
pub const MAX_COPIES: usize = 2048;

/// How many times one instance's body is checked before it is reported.
pub const MAX_RUNS: usize = 64;

/// An instance of the call graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstId(pub usize);

/// What an instance is.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum InstKind {
    /// A function, method, trait default body or derived implementation, with its generic
    /// arguments ([`Index::node_generics`]).
    Node(NodeId, Vec<Ty>),
    /// A closure.
    Closure(ClosureKey),
}

/// One instance.
#[derive(Clone, Debug)]
pub struct Inst {
    /// What it is.
    pub kind: InstKind,
    /// The program node it is counted as: itself, or for a closure the function it is written in.
    pub node: NodeId,
    /// The instance whose body holds it: itself, or for a closure its function's instance.
    pub owner: InstId,
}

/// Where a call is written, and how it was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    /// The file, relative to the workspace root.
    pub file: String,
    /// The line.
    pub line: usize,
    /// How the call was found (one of [`crate::recursion::index::how`]).
    pub how: &'static str,
}

/// What a body read that another may change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Dep {
    /// An instance's hidden result type.
    Ret(InstId),
    /// The callables coerced to closure trait objects.
    Dyn,
    /// The callables coerced to function pointers.
    FnPtr,
}

/// The call graph over instances, and what the bodies share.
#[derive(Default)]
pub struct Mono {
    /// Every instance.
    pub insts: Vec<Inst>,
    by_kind: BTreeMap<InstKind, InstId>,
    /// The edges from each instance, each with the first site seen.
    pub edges: BTreeMap<InstId, BTreeMap<InstId, Site>>,
    /// Each instance's result type, resolved, once its body was checked: the hidden types of its
    /// `impl Trait` result.
    pub rets: BTreeMap<InstId, Ty>,
    /// Each closure's parameter types and result, once its function's body was checked.
    pub closure_sigs: BTreeMap<ClosureKey, (Vec<Ty>, Ty)>,
    /// Closures and function items coerced to a closure trait object.
    pub dyn_callables: BTreeSet<Ty>,
    /// Closures and function items coerced to a function pointer.
    pub fnptr_callables: BTreeSet<Ty>,
    /// What each instance's last check could not read.
    pub problems: BTreeMap<InstId, Vec<Problem>>,
    readers: BTreeMap<Dep, BTreeSet<InstId>>,
    queue: VecDeque<InstId>,
    queued: BTreeSet<InstId>,
    runs: BTreeMap<InstId, usize>,
    copies: BTreeMap<NodeId, usize>,
    owned: BTreeMap<InstId, BTreeSet<InstId>>,
    /// Problems not tied to one instance's check.
    pub global_problems: Vec<Problem>,
    /// The roots: every node of the crates that ship and of the bound tests, open.
    pub roots: Vec<InstId>,
}

impl Mono {
    /// The instance `id`.
    #[must_use]
    pub fn inst(&self, id: InstId) -> Option<&Inst> {
        self.insts.get(id.0)
    }

    /// The instance of kind `kind`, if it was made.
    #[must_use]
    pub fn find(&self, kind: &InstKind) -> Option<InstId> {
        self.by_kind.get(kind).copied()
    }

    /// The instance of node `node` with generic arguments `args`, made and queued on first use.
    pub fn node_inst(&mut self, ix: &Index<'_>, node: NodeId, args: Vec<Ty>) -> InstId {
        let args: Vec<Ty> = args.into_iter().map(|a| a.cut()).collect();
        let kind = InstKind::Node(node, args);
        if let Some(id) = self.by_kind.get(&kind) {
            return *id;
        }
        let count = self.copies.entry(node).or_default();
        *count = count.saturating_add(1);
        if *count == MAX_COPIES.saturating_add(1)
            && let Some(n) = ix.program.node(node)
        {
            self.global_problems.push(Problem::unread(
                &n.file,
                n.line,
                &format!(
                    "`{}` has more than {MAX_COPIES} instantiations",
                    ix.name(node)
                ),
            ));
        }
        let id = InstId(self.insts.len());
        self.insts.push(Inst {
            kind: kind.clone(),
            node,
            owner: id,
        });
        self.by_kind.insert(kind, id);
        self.enqueue(id);
        id
    }

    /// The instance of closure `key`, made on first use (with its function's instance).
    pub fn closure_inst(&mut self, ix: &Index<'_>, key: &ClosureKey) -> InstId {
        let key = ClosureKey {
            owner: key.owner,
            subst: key.subst.iter().map(Ty::cut).collect(),
            index: key.index,
        };
        let kind = InstKind::Closure(key.clone());
        if let Some(id) = self.by_kind.get(&kind) {
            return *id;
        }
        let owner = self.node_inst(ix, key.owner, key.subst.clone());
        let id = InstId(self.insts.len());
        self.insts.push(Inst {
            kind: kind.clone(),
            node: key.owner,
            owner,
        });
        self.by_kind.insert(kind, id);
        self.owned.entry(owner).or_default().insert(id);
        id
    }

    /// Queues instance `id`'s body (its owner's, for a closure) to be checked.
    pub fn enqueue(&mut self, id: InstId) {
        let owner = self.inst(id).map_or(id, |i| i.owner);
        if self.queued.insert(owner) {
            self.queue.push_back(owner);
        }
    }

    /// Clears what the last check of `owner`'s body found: its edges and its closures', and its
    /// problems.
    pub fn begin_check(&mut self, owner: InstId) {
        self.edges.remove(&owner);
        for c in self.owned.get(&owner).cloned().unwrap_or_default() {
            self.edges.remove(&c);
        }
        self.problems.remove(&owner);
    }

    /// Adds the edge `from -> to`, keeping the first site.
    pub fn edge(&mut self, from: InstId, to: InstId, site: &Site) {
        self.edges
            .entry(from)
            .or_default()
            .entry(to)
            .or_insert_with(|| site.clone());
    }

    /// Records that instance `reader` read `dep`, so it is checked again when `dep` changes.
    pub fn read(&mut self, dep: Dep, reader: InstId) {
        let owner = self.inst(reader).map_or(reader, |i| i.owner);
        self.readers.entry(dep).or_default().insert(owner);
    }

    fn changed(&mut self, dep: Dep) {
        let readers: Vec<InstId> = self
            .readers
            .get(&dep)
            .map(|r| r.iter().copied().collect())
            .unwrap_or_default();
        for r in readers {
            self.enqueue(r);
        }
    }

    /// Sets instance `id`'s resolved result type; its readers are checked again if it changed.
    pub fn set_ret(&mut self, id: InstId, ty: Ty) {
        if self.rets.get(&id) != Some(&ty) {
            self.rets.insert(id, ty);
            self.changed(Dep::Ret(id));
        }
    }

    /// Adds callables to the closure-trait-object set (`dynamic`) or the function-pointer set;
    /// their readers are checked again if it grew.
    pub fn coerce(&mut self, callables: impl IntoIterator<Item = Ty>, dynamic: bool) {
        let mut grew = false;
        for c in callables {
            let set = if dynamic {
                &mut self.dyn_callables
            } else {
                &mut self.fnptr_callables
            };
            grew |= set.insert(c);
        }
        if grew {
            self.changed(if dynamic { Dep::Dyn } else { Dep::FnPtr });
        }
    }

    fn pop(&mut self) -> Option<InstId> {
        let id = self.queue.pop_front()?;
        self.queued.remove(&id);
        Some(id)
    }

    /// The instances the roots reach by the edges of the last check of every body: an instance
    /// made by an earlier check that a later one no longer calls is not among them.
    #[must_use]
    pub fn reachable(&self) -> BTreeSet<InstId> {
        let mut seen: BTreeSet<InstId> = self.roots.iter().copied().collect();
        let mut queue: VecDeque<InstId> = self.roots.iter().copied().collect();
        while let Some(x) = queue.pop_front() {
            for to in self.edges.get(&x).into_iter().flat_map(BTreeMap::keys) {
                if seen.insert(*to) {
                    queue.push_back(*to);
                }
            }
        }
        seen
    }

    /// Every problem the checks found, sorted. A problem in the standard library's model, or in
    /// an instance whose generic arguments the check could not infer, is reported where the
    /// crates that ship reach it (the nearest call from a crate read whose own arguments are
    /// known), naming the instance, since that call is what to look at.
    #[must_use]
    pub fn all_problems(&self, ix: &Index<'_>) -> Vec<Problem> {
        let mut out: Vec<Problem> = self.global_problems.clone();
        let live = self.reachable();
        let mut callers: BTreeMap<InstId, Vec<(InstId, &Site)>> = BTreeMap::new();
        for (from, tos) in &self.edges {
            for (to, site) in tos {
                callers.entry(*to).or_default().push((*from, site));
            }
        }
        let suspect = |id: InstId| -> bool {
            self.inst(id).is_some_and(|i| {
                ix.is_model(i.node)
                    || match &i.kind {
                        InstKind::Node(_, args) => args.iter().any(Ty::has_unknown),
                        InstKind::Closure(k) => k.subst.iter().any(Ty::has_unknown),
                    }
            })
        };
        for (id, problems) in &self.problems {
            if !live.contains(id) {
                continue;
            }
            if !suspect(*id) {
                out.extend(problems.iter().cloned());
                continue;
            }
            // Breadth-first back along the calls to the nearest caller that is not suspect.
            let mut seen = BTreeSet::from([*id]);
            let mut queue = VecDeque::from([*id]);
            let mut site: Option<Site> = None;
            while let Some(x) = queue.pop_front() {
                for (from, s) in callers.get(&x).into_iter().flatten() {
                    if !suspect(*from) {
                        site = Some((*s).clone());
                        queue.clear();
                        break;
                    }
                    if seen.insert(*from) {
                        queue.push_back(*from);
                    }
                }
            }
            let name = self.inst(*id).map(|i| match &i.kind {
                InstKind::Node(n, args) => {
                    let shown: Vec<String> = args.iter().map(|a| ix.show(a)).collect();
                    format!("{} [{}]", ix.name(*n), shown.join(", "))
                }
                InstKind::Closure(k) => format!("{}::{{closure#{}}}", ix.name(k.owner), k.index),
            });
            for p in problems {
                match &site {
                    Some(s) => out.push(Problem::new(
                        &s.file,
                        s.line,
                        p.rule,
                        format!(
                            "{} (in `{}`, which this call reaches, at {}:{})",
                            p.what,
                            name.clone().unwrap_or_default(),
                            p.file,
                            p.line
                        ),
                    )),
                    None => out.push(p.clone()),
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }
}

/// Builds the call graph of every instance reachable from the roots: every node of the crates
/// that ship and of the bound tests, generic ones with their parameters open.
#[must_use]
pub fn analyse(ix: &Index<'_>) -> Mono {
    let mut m = Mono::default();
    for (i, node) in ix.program.nodes.iter().enumerate() {
        let role = ix.program.crates.get(node.krate).map(|c| c.source.role);
        if !matches!(role, Some(Role::Ships | Role::BoundTests)) {
            continue;
        }
        let id = NodeId(i);
        let open: Vec<Ty> = ix.node_generics(id).iter().map(|g| Ty::Param(*g)).collect();
        let root = m.node_inst(ix, id, open);
        m.roots.push(root);
    }
    crate::recursion::typeck::constants(ix, &mut m);
    while let Some(id) = m.pop() {
        let runs = m.runs.entry(id).or_default();
        *runs = runs.saturating_add(1);
        if *runs > MAX_RUNS {
            if *runs == MAX_RUNS.saturating_add(1)
                && let Some(n) = m.inst(id).and_then(|i| ix.program.node(i.node))
            {
                m.global_problems.push(Problem::unread(
                    &n.file,
                    n.line,
                    &format!(
                        "the check of `{}` still changes after {MAX_RUNS} readings",
                        n.name
                    ),
                ));
            }
            continue;
        }
        let Some(inst) = m.inst(id).cloned() else {
            continue;
        };
        if let InstKind::Node(node, args) = inst.kind {
            m.begin_check(id);
            let derived = ix
                .program
                .node(node)
                .is_some_and(|n| matches!(n.kind, NodeKind::Derived { .. }));
            if derived {
                crate::recursion::typeck::derived(ix, &mut m, id, node, &args);
            } else {
                crate::recursion::typeck::body(ix, &mut m, id, node, &args);
            }
        }
    }
    m
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests may use native arithmetic, casts, unwrap and indexing"
)]
mod tests {
    use super::*;
    use crate::recursion::model::{CrateSource, Program, load};

    fn program(text: &str) -> Program {
        let text = text.to_owned();
        let read = move |p: &str| {
            if p == "c/a/src/lib.rs" {
                Ok(text.clone())
            } else {
                crate::recursion::stdlib::read_model(p).ok_or_else(|| "no such file".to_owned())
            }
        };
        let mut sources = crate::recursion::stdlib::sources();
        sources.push(CrateSource {
            package: "a".to_owned(),
            lib: "a".to_owned(),
            folder: "c/a".to_owned(),
            root: "c/a/src/lib.rs".to_owned(),
            role: Role::Ships,
        });
        load(&sources, &read)
    }

    #[test]
    #[allow(
        clippy::many_single_char_names,
        reason = "the fixture's functions are f and g"
    )]
    fn instances_are_made_once_and_queued() {
        let p = program("pub fn g<T>(t: T) {} pub fn f() {}");
        let ix = Index::new(&p);
        let mut m = Mono::default();
        let g = NodeId(p.nodes.iter().position(|n| n.ident == "g").unwrap());
        let a = m.node_inst(&ix, g, vec![Ty::unit()]);
        let b = m.node_inst(&ix, g, vec![Ty::unit()]);
        assert_eq!(a, b);
        assert_eq!(m.pop(), Some(a));
        assert_eq!(m.pop(), None);
    }

    #[test]
    fn changed_results_queue_their_readers() {
        let p = program("pub fn f() {} pub fn g() {}");
        let ix = Index::new(&p);
        let mut m = Mono::default();
        let f = m.node_inst(
            &ix,
            NodeId(p.nodes.iter().position(|n| n.ident == "f").unwrap()),
            Vec::new(),
        );
        let g = m.node_inst(
            &ix,
            NodeId(p.nodes.iter().position(|n| n.ident == "g").unwrap()),
            Vec::new(),
        );
        while m.pop().is_some() {}
        m.read(Dep::Ret(f), g);
        m.set_ret(f, Ty::unit());
        assert_eq!(m.pop(), Some(g));
        m.set_ret(f, Ty::unit());
        assert_eq!(m.pop(), None, "no change, no check");
    }
}
