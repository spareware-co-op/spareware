//! Instances: the recursion check's call graph over functions as the compiler copies them, one
//! per instantiation of their generic parameters, and the work list that builds it.
//!
//! What it does: Rust compiles one copy of a generic function for each instantiation of its type
//! parameters, and a closure is a type of its own whose body is part of the copy of the function
//! that writes it. [`analyse`] builds the call graph over those copies ([`Inst`]): it starts from
//! every function, method, trait default body and derived implementation with its generic
//! parameters left open ([`Ty::Param`], the copy every instantiation shares), reads each body
//! ([`crate::recursion::flow`]), and adds the copy each call reaches, with the generic arguments
//! the call fixes, to the work list. What one body learns that another needs grows here, and the
//! bodies that read it are read again: each closure's parameters (from every call that may reach
//! it) and result, each instance's result (for `impl Trait` returns), and the closures and
//! functions coerced to trait objects (`dyn Fn`) and function pointers, which a call through one
//! may run.
//!
//! Guarantees:
//!
//! - **Every copy a call can reach is an instance.** A call's generic arguments are those its
//!   argument types fix; one it leaves unfixed stays open, and an open parameter dispatches to
//!   every implementation of its bounds ([`crate::recursion::index`]). A node with more than
//!   [`MAX_COPIES`] instances gets one more whose arguments are hidden values of every argument
//!   seen, which reaches all their implementations.
//! - **A fixed point.** Every value only grows, in a lattice of finite height
//!   ([`crate::recursion::ty`]), and a body is read again only when something it read grew, so
//!   the work list empties; a body read more than [`MAX_RUNS`] times is reported rather than
//!   followed further.
//! - **Deterministic.** Instances are numbered in the order they are found, the work list is
//!   first in, first out, and every table is ordered.
//!
//! Method: monomorphisation as rustc's collector performs it
//! (<https://rustc-dev-guide.rust-lang.org/backend/monomorph.html>), here over the source with
//! the analysis's types, with closures as types (The Rust Reference,
//! <https://doc.rust-lang.org/reference/types/closure.html>); chaotic iteration to the least
//! fixed point with a work list (P. Cousot and R. Cousot, POPL 1977,
//! <https://doi.org/10.1145/512950.512973>; F. Nielson, H. R. Nielson and C. Hankin,
//! *Principles of Program Analysis*, Springer 1999, section 1.6).
//!
//! Cost: one reading of each instance's body per round in which something it read grew;
//! instances at most [`MAX_COPIES`] plus the widened ones per node.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::recursion::Problem;
use crate::recursion::index::Index;
use crate::recursion::model::NodeKind;
use crate::recursion::ty::{ClosureKey, NodeId, Ty, Val};

/// How many instances a node gets before calls reach a widened one.
pub const MAX_COPIES: usize = 64;

/// How many times one instance's body is read before it is reported.
pub const MAX_RUNS: usize = 400;

/// An instance of the call graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstId(pub usize);

/// What an instance is.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum InstKind {
    /// A function, method, trait default body or derived implementation, with generic arguments
    /// for [`Index::node_generics`].
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

/// What a body read that another may grow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Dep {
    /// An instance's result.
    Ret(InstId),
    /// The callables coerced to closure trait objects.
    Dyn,
    /// The callables coerced to function pointers.
    FnPtr,
}

/// The call graph over instances, and everything the bodies share.
#[derive(Default)]
pub struct Mono {
    /// Every instance.
    pub insts: Vec<Inst>,
    by_kind: BTreeMap<InstKind, InstId>,
    /// The edges from each instance, each with the first site seen.
    pub edges: BTreeMap<InstId, BTreeMap<InstId, Site>>,
    /// Each instance's result: its body's returns and tail, for a function; for a closure, what
    /// its body returns.
    pub rets: BTreeMap<InstId, Val>,
    /// Each closure's parameters, from every call that may reach it.
    pub params: BTreeMap<InstId, Vec<Val>>,
    /// Each closure's number of parameters, once its body was read.
    pub arity: BTreeMap<InstId, usize>,
    /// Closures and function items coerced to a closure trait object.
    pub dyn_callables: BTreeSet<Ty>,
    /// Closures and function items coerced to a function pointer.
    pub fnptr_callables: BTreeSet<Ty>,
    readers: BTreeMap<Dep, BTreeSet<InstId>>,
    queue: VecDeque<InstId>,
    queued: BTreeSet<InstId>,
    runs: BTreeMap<InstId, usize>,
    /// For each node, every generic argument list its instances were asked for.
    copies: BTreeMap<NodeId, Vec<Vec<Ty>>>,
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

    /// The instance of node `node` with generic arguments `args`, made and queued on first use;
    /// past [`MAX_COPIES`] copies of the node, the widened instance.
    pub fn node_inst(&mut self, ix: &Index<'_>, node: NodeId, args: Vec<Ty>) -> InstId {
        let args: Vec<Ty> = args
            .into_iter()
            .map(|a| {
                let n = ix.normalize(&a);
                if n.depth() > crate::recursion::ty::MAX_DEPTH {
                    n.cut()
                } else {
                    n
                }
            })
            .collect();
        let orig = InstKind::Node(node, args.clone());
        if let Some(id) = self.by_kind.get(&orig) {
            return *id;
        }
        let seen = self.copies.entry(node).or_default();
        let kind = if seen.len() >= MAX_COPIES {
            seen.push(args.clone());
            let mut widened: Vec<BTreeSet<Ty>> = vec![BTreeSet::new(); args.len()];
            for list in seen.iter() {
                for (w, a) in widened.iter_mut().zip(list) {
                    w.extend(a.parts().iter().map(Ty::shallow));
                    if !matches!(a, Ty::Hidden(_)) {
                        w.insert(a.shallow());
                    }
                }
            }
            InstKind::Node(node, widened.into_iter().map(Ty::Hidden).collect())
        } else {
            seen.push(args);
            orig.clone()
        };
        let id = if let Some(id) = self.by_kind.get(&kind) {
            *id
        } else {
            let id = InstId(self.insts.len());
            self.insts.push(Inst {
                kind: kind.clone(),
                node,
                owner: id,
            });
            self.by_kind.insert(kind.clone(), id);
            self.enqueue(id);
            id
        };
        if kind != orig {
            self.by_kind.insert(orig, id);
        }
        id
    }

    /// The instance of closure `key`, made on first use; its function's instance is made (and
    /// queued) too.
    pub fn closure_inst(&mut self, ix: &Index<'_>, key: &ClosureKey) -> InstId {
        let kind = InstKind::Closure(key.clone());
        if let Some(id) = self.by_kind.get(&kind) {
            return *id;
        }
        let owner = self.node_inst(ix, key.owner, key.subst.clone());
        // The owner may have been widened; the closure is the widened owner's.
        let key = match self.inst(owner).map(|i| &i.kind) {
            Some(InstKind::Node(_, args)) if *args != key.subst => ClosureKey {
                owner: key.owner,
                subst: args.clone(),
                index: key.index,
            },
            _ => key.clone(),
        };
        let kind2 = InstKind::Closure(key);
        if let Some(&id) = self.by_kind.get(&kind2) {
            self.by_kind.insert(kind, id);
            return id;
        }
        let id = InstId(self.insts.len());
        self.insts.push(Inst {
            kind: kind2.clone(),
            node: key_owner(&kind2),
            owner,
        });
        self.by_kind.insert(kind2, id);
        self.by_kind.insert(kind, id);
        id
    }

    /// Queues instance `id`'s body (its owner's, for a closure) to be read.
    pub fn enqueue(&mut self, id: InstId) {
        let owner = self.inst(id).map_or(id, |i| i.owner);
        if self.queued.insert(owner) {
            self.queue.push_back(owner);
        }
    }

    /// Adds the edge `from -> to`, keeping the first site.
    pub fn edge(&mut self, from: InstId, to: InstId, site: &Site) {
        self.edges
            .entry(from)
            .or_default()
            .entry(to)
            .or_insert_with(|| site.clone());
    }

    /// Records that instance `reader` read `dep`, so it is read again when `dep` grows.
    pub fn read(&mut self, dep: Dep, reader: InstId) {
        let owner = self.inst(reader).map_or(reader, |i| i.owner);
        self.readers.entry(dep).or_default().insert(owner);
    }

    /// Queues every reader of `dep`.
    pub fn grew(&mut self, dep: Dep) {
        let readers: Vec<InstId> = self
            .readers
            .get(&dep)
            .map(|r| r.iter().copied().collect())
            .unwrap_or_default();
        for r in readers {
            self.enqueue(r);
        }
    }

    /// Joins `v` into instance `id`'s result; queues its readers if it grew.
    pub fn add_ret(&mut self, id: InstId, v: &Val) {
        if self.rets.entry(id).or_default().join(v) {
            self.grew(Dep::Ret(id));
        }
    }

    /// Instance `id`'s result as known so far, recording `reader` as reading it.
    pub fn ret_of(&mut self, id: InstId, reader: InstId) -> Val {
        self.read(Dep::Ret(id), reader);
        self.rets.get(&id).cloned().unwrap_or_default()
    }

    /// Joins `args` into closure `id`'s parameters; queues its body if they grew, and says
    /// whether they did.
    pub fn add_params(&mut self, id: InstId, args: &[Val]) -> bool {
        let params = self.params.entry(id).or_default();
        let mut grew = false;
        for (i, a) in args.iter().enumerate() {
            while params.len() <= i {
                params.push(Val::none());
            }
            if let Some(p) = params.get_mut(i) {
                grew |= p.join(a);
            }
        }
        if grew {
            self.enqueue(id);
        }
        grew
    }

    /// Adds callables to the closure-trait-object set (`dynamic`) or the function-pointer set;
    /// queues the readers if it grew.
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
            self.grew(if dynamic { Dep::Dyn } else { Dep::FnPtr });
        }
    }

    fn pop(&mut self) -> Option<InstId> {
        let id = self.queue.pop_front()?;
        self.queued.remove(&id);
        Some(id)
    }
}

fn key_owner(kind: &InstKind) -> NodeId {
    match kind {
        InstKind::Node(n, _) | InstKind::Closure(ClosureKey { owner: n, .. }) => *n,
    }
}

/// Builds the call graph of every instance reachable from every node of the program; what cannot
/// be read goes to `problems`.
#[must_use]
pub fn analyse(ix: &Index<'_>, problems: &mut Vec<Problem>) -> Mono {
    let mut m = Mono::default();
    let mut reported = BTreeSet::new();
    for (i, _) in ix.program.nodes.iter().enumerate() {
        let id = NodeId(i);
        let open: Vec<Ty> = ix.node_generics(id).iter().map(|g| Ty::Param(*g)).collect();
        m.node_inst(ix, id, open);
    }
    crate::recursion::flow::constants(ix, &mut m, problems, &mut reported);
    let mut pops = 0_usize;
    while let Some(id) = m.pop() {
        pops += 1;
        if pops % 1 == 0 { eprintln!("DEBUG pops {pops} insts {} queue {} node {}", m.insts.len(), m.queue.len(), m.inst(id).map_or(String::new(), |i| ix.name(i.node).to_owned())); }
        let runs = m.runs.entry(id).or_default();
        *runs = runs.saturating_add(1);
        if *runs > MAX_RUNS {
            if *runs == MAX_RUNS.saturating_add(1) {
                let (file, line, name) = ix
                    .program
                    .node(m.inst(id).map_or(NodeId(0), |i| i.node))
                    .map_or((String::new(), 0, String::new()), |n| {
                        (n.file.clone(), n.line, n.name.clone())
                    });
                problems.push(Problem::unread(
                    &file,
                    line,
                    &format!("the analysis of `{name}` still grows after {MAX_RUNS} readings"),
                ));
            }
            continue;
        }
        let Some(inst) = m.inst(id).cloned() else {
            continue;
        };
        match inst.kind {
            InstKind::Node(node, args) => {
                let derived = ix
                    .program
                    .node(node)
                    .is_some_and(|n| matches!(n.kind, NodeKind::Derived { .. }));
                if derived {
                    crate::recursion::flow::derived(ix, &mut m, problems, &mut reported, id, node, &args);
                } else {
                    crate::recursion::flow::body(ix, &mut m, problems, &mut reported, id, node, &args);
                }
            }
            InstKind::Closure(_) => {}
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
    use crate::recursion::model::{CrateSource, Program, Role, load};

    fn program(text: &str) -> Program {
        let text = text.to_owned();
        let read = move |p: &str| {
            if p == "c/a/src/lib.rs" {
                Ok(text.clone())
            } else {
                Err("no such file".to_owned())
            }
        };
        load(
            &[CrateSource {
                package: "a".to_owned(),
                lib: "a".to_owned(),
                folder: "c/a".to_owned(),
                root: "c/a/src/lib.rs".to_owned(),
                role: Role::Ships,
            }],
            &read,
        )
    }

    #[test]
    fn copies_past_the_limit_reach_one_widened_instance() {
        let p = program("pub fn g<T>(t: T) {}");
        let ix = Index::new(&p);
        let mut m = Mono::default();
        let g = NodeId(0);
        let mut ids = BTreeSet::new();
        for i in 0..MAX_COPIES + 5 {
            ids.insert(m.node_inst(&ix, g, vec![Ty::Adt(crate::recursion::ty::ItemId(i), Vec::new())]));
        }
        assert!(ids.len() <= MAX_COPIES + 5);
        let last = m.inst(*ids.iter().max().unwrap()).unwrap();
        match &last.kind {
            InstKind::Node(_, args) => assert!(matches!(&args[0], Ty::Hidden(p) if p.len() > MAX_COPIES)),
            InstKind::Closure(_) => panic!("a node"),
        }
    }

    #[test]
    fn growing_values_queue_their_readers() {
        let p = program("pub fn f() {} pub fn g() {}");
        let ix = Index::new(&p);
        let mut m = Mono::default();
        let f = m.node_inst(&ix, NodeId(0), Vec::new());
        let g = m.node_inst(&ix, NodeId(1), Vec::new());
        while m.pop().is_some() {}
        assert!(m.ret_of(f, g).is_none());
        m.add_ret(f, &Val::unit());
        assert_eq!(m.pop(), Some(g));
        m.add_ret(f, &Val::unit());
        assert_eq!(m.pop(), None, "no growth, no reading");
    }
}
