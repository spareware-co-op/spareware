//! The body analysis: every call a function body, closure, derived implementation or constant
//! initialiser may make, as edges of the call graph.
//!
//! What it does: [`analyse`] reads every node of the program. For a body, it evaluates each
//! expression to a [`Val`] (its type, as far as the source states it, and what it may carry),
//! binding patterns as it goes, and adds an edge for every call the expression may make:
//!
//! - **Calls by path** to functions, associated functions (`Type::f`, `Trait::f`, `T::f`,
//!   `Self::f`, `<T as Trait>::f`) and outside functions, and constructors, which call nothing.
//! - **Method calls**, resolved by the receiver's type ([`Index::method`]).
//! - **Implicit calls**: operators and compound assignments (`Add::add`, `PartialEq::eq`,
//!   `PartialOrd::partial_cmp`, `Neg::neg`, ...), indexing, `*` on a kernel type with `Deref`,
//!   `?` (`From::from` into the function's error type), `for` loops (`IntoIterator::into_iter`,
//!   `Iterator::next`), and the formatting traits of formatting macros
//!   ([`crate::recursion::macros`]).
//! - **Calls through values**: a closure, function item or anything else called as `f(x)` runs
//!   whatever the value may carry; parameters, fields, return values and constants are carrier
//!   nodes ([`Graph::param`], [`Graph::field`], [`Graph::ret`], [`Graph::constant`]) through
//!   which the callables that flow into them at one place reach the calls made through them at
//!   another, and a value that is written to (`v.push(f)`, `*slot = f`, `&mut v` passed on) takes
//!   in what is written. A carrier takes in callables only when its declared type can hold one
//!   ([`Ty::admits_callables`]): an `f64` parameter carries nothing, whatever is passed to it.
//!   A closure's parameters and result are the program's, not one body's: every call that may
//!   reach it, here or through carriers anywhere, joins its arguments into its parameters, and
//!   its result flows back to each.
//! - **Callbacks from outside code**: a call that may leave the kernel runs, through what it is
//!   handed, the outside-trait implementations of the kernel types, the closures and functions,
//!   and the bounds' implementations of the generic parameters its receiver and arguments carry
//!   ([`Index::callbacks`]), and for the standard conversions that construct their result
//!   (`into`, `try_into`, `collect`, `parse`, `sum`, `product`, `unzip`, `partition`) the
//!   matching kernel `From`, `TryFrom`, `FromIterator`, `FromStr`, `Sum`, `Product`, `Extend` and
//!   `Default` implementations.
//!
//! A body is evaluated repeatedly until no binding's value and no closure parameter grows, so a
//! value assigned late in a loop reaches its uses early in it; and the whole program is evaluated
//! again until no closure's parameters or result and no carrier's contents grow, so a closure
//! called through a carrier in one body sees the arguments of that call in another. Everything
//! starts empty and only grows, so no call is added on a guess that a later round would have
//! withdrawn.
//!
//! Each edge to a kernel function carries the size-change graph of its call
//! ([`crate::recursion::sizechange::relate`]): the callee's declared parameter and receiver types,
//! and the implementation's self type a dispatch chose, matched against the types at the call
//! site. A closure called where it was made, in the same running call of its function, keeps its
//! function's type parameters as they are.
//!
//! Guarantees:
//!
//! - **Sound for the calls Rust can make, given the standard library.** Every call a body can
//!   make into kernel code in some build that ships is an edge, by one of the routes above, or the
//!   code is reported: a path that resolves nowhere, a macro that cannot be read, a syntax form
//!   the analysis does not know ([`Problem`]). The assumptions are about the standard library:
//!   that it calls kernel code only through what it is handed (the module header of
//!   [`crate::recursion::index`]), that it calls a closure it is handed with values built from
//!   what it is handed, and that its prelude's variant constructors `Some`, `Ok` and `Err` call
//!   nothing. Where a value's type is lost, its kernel types are kept as what it may carry, and a
//!   type parameter of another function stands for every kernel type its bounds allow
//!   ([`Index::types_for`]).
//! - **Precise where the source is.** A receiver or operand of a stated type dispatches only to
//!   what that type can run, so a method that delegates to a field's method of the same name is
//!   not a cycle with itself.
//! - **Terminates.** Values only grow and the sets are finite; evaluation is also capped at
//!   [`MAX_ROUNDS`] rounds, after which a body still growing is reported.
//!
//! Method: abstract interpretation of each body over the lattice of [`crate::recursion::ty`]
//! (P. Cousot and R. Cousot, POPL 1977, <https://doi.org/10.1145/512950.512973>), to a fixed
//! point by round-robin iteration; callable flow by 0-CFA-style closure analysis with carrier
//! nodes for parameters, fields and returns (O. Shivers, "Control-flow analysis of higher-order
//! languages", PhD thesis, Carnegie Mellon University, 1991,
//! <https://www.ccs.neu.edu/home/shivers/papers/diss.pdf>); expression and pattern semantics from
//! The Rust Reference (<https://doc.rust-lang.org/reference/expressions.html>,
//! <https://doc.rust-lang.org/reference/patterns.html>), including its operator traits
//! (<https://doc.rust-lang.org/reference/expressions/operator-expr.html>).
//!
//! Cost: each round is linear in the size of the body times the cost of the lookups it makes;
//! at most [`MAX_ROUNDS`] rounds per body and [`MAX_PROGRAM_ROUNDS`] rounds of the program. The evaluator recurses over the syntax tree, so its depth is the
//! nesting depth of the source's expressions; [`crate::recursion::analyse`] runs it on a thread
//! with a large stack.

use std::collections::{BTreeMap, BTreeSet};

use syn::spanned::Spanned;
use syn::{BinOp, Expr, Pat, Stmt, UnOp};

use crate::recursion::Problem;
use crate::recursion::graph::{GKind, Graph, Site};
use crate::recursion::index::{Index, Key, Via, key};
use crate::recursion::sizechange::{Scg, relate};
use crate::recursion::macros::{self, FmtTrait, Formatted, MacroRead};
use crate::recursion::model::{Fields, ItemKind, NodeKind, Scope, SelfRef};
use crate::recursion::resolve::{Ns, Res, Resolver};
use crate::recursion::ty::{Carries, GenId, ItemId, NodeId, TraitRef, Ty, Val};

/// The most rounds a body is evaluated.
pub const MAX_ROUNDS: usize = 32;

/// The most rounds the whole program is evaluated.
pub const MAX_PROGRAM_ROUNDS: usize = 32;

/// How an implicit trait call's edges are described.
const IMPLICIT: &str = "an implicit trait call (operator, formatting, derive)";

/// A kernel function a call may run, with the receiver type it is called on when the source
/// states it.
type Target = (NodeId, Option<Ty>);

/// A body the analysis reads: a program node, or a constant's initialiser (its item and node).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Body {
    /// A function, method, trait default body or derived implementation.
    Node(NodeId),
    /// A constant's initialiser.
    Const(ItemId, NodeId),
}

/// What the analysis of every body shares: each closure's parameters and result, and what each
/// carrier may hold, all growing from empty; and which bodies read each, so a body is read again
/// only when something it read grew.
#[derive(Default)]
struct Shared {
    /// Each closure parameter's value, joined from every call that may reach the closure.
    closure_params: BTreeMap<(NodeId, usize), Val>,
    /// Each closure parameter's value as its context fixes it, where a kernel callee's closure
    /// bound does: its type there, with the callee's type parameters learned from the other
    /// arguments, and what those parameters' instantiations may hold. Rust gives the closure that
    /// type at the call that makes it, so values that reach it from elsewhere (the callee's own
    /// calls of it, in the callee's types) add only the callables they hold.
    typed: BTreeMap<(NodeId, usize), Val>,
    /// Each closure's result.
    closure_rets: BTreeMap<NodeId, Val>,
    /// The closures, functions and the unknown-call node each carrier may lead to, through
    /// carriers only, as of the last program round.
    reach: BTreeMap<NodeId, BTreeSet<NodeId>>,
    /// The body each closure is written in.
    made_in: BTreeMap<NodeId, Body>,
    /// The bodies that read each closure's result.
    ret_readers: BTreeMap<NodeId, BTreeSet<Body>>,
    /// The bodies that read each carrier's contents.
    reach_readers: BTreeMap<NodeId, BTreeSet<Body>>,
    /// The bodies to read again in the next program round.
    dirty: BTreeSet<Body>,
}

/// The standard conversions that construct their result, with the trait and method of the kernel
/// implementations they may call.
pub const CONVERSIONS: [(&str, &[(&str, &str)]); 10] = [
    ("into", &[("From", "from")]),
    ("try_into", &[("TryFrom", "try_from"), ("From", "from")]),
    ("collect", &[("FromIterator", "from_iter"), ("Extend", "extend"), ("Default", "default")]),
    ("sum", &[("Sum", "sum")]),
    ("product", &[("Product", "product")]),
    ("parse", &[("FromStr", "from_str")]),
    ("unzip", &[("Extend", "extend"), ("Default", "default")]),
    ("partition", &[("Extend", "extend"), ("Default", "default")]),
    ("from_iter", &[("FromIterator", "from_iter")]),
    ("try_from", &[("TryFrom", "try_from"), ("From", "from")]),
];

/// The outside traits a binary operator calls, with the method.
fn binop_trait(op: &BinOp) -> Option<(&'static str, &'static str)> {
    Some(match op {
        BinOp::Add(_) => ("Add", "add"),
        BinOp::Sub(_) => ("Sub", "sub"),
        BinOp::Mul(_) => ("Mul", "mul"),
        BinOp::Div(_) => ("Div", "div"),
        BinOp::Rem(_) => ("Rem", "rem"),
        BinOp::BitAnd(_) => ("BitAnd", "bitand"),
        BinOp::BitOr(_) => ("BitOr", "bitor"),
        BinOp::BitXor(_) => ("BitXor", "bitxor"),
        BinOp::Shl(_) => ("Shl", "shl"),
        BinOp::Shr(_) => ("Shr", "shr"),
        BinOp::AddAssign(_) => ("AddAssign", "add_assign"),
        BinOp::SubAssign(_) => ("SubAssign", "sub_assign"),
        BinOp::MulAssign(_) => ("MulAssign", "mul_assign"),
        BinOp::DivAssign(_) => ("DivAssign", "div_assign"),
        BinOp::RemAssign(_) => ("RemAssign", "rem_assign"),
        BinOp::BitAndAssign(_) => ("BitAndAssign", "bitand_assign"),
        BinOp::BitOrAssign(_) => ("BitOrAssign", "bitor_assign"),
        BinOp::BitXorAssign(_) => ("BitXorAssign", "bitxor_assign"),
        BinOp::ShlAssign(_) => ("ShlAssign", "shl_assign"),
        BinOp::ShrAssign(_) => ("ShrAssign", "shr_assign"),
        BinOp::Eq(_) | BinOp::Ne(_) => ("PartialEq", "*"),
        BinOp::Lt(_) | BinOp::Le(_) | BinOp::Gt(_) | BinOp::Ge(_) => ("PartialOrd", "*"),
        _ => return None,
    })
}

/// Where a value can be written to.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Place {
    /// A local binding: written as a whole (`x = v`, false), or through it (`*x = v`, `x[i] = v`,
    /// `x.push(v)`, `&mut x` passed on, true), which for a reference reaches what it refers to.
    Local(usize, bool),
    /// A field of a kernel type.
    Field(ItemId, Option<usize>, String),
    /// Nowhere the analysis follows.
    Other,
}

/// The analysis of one node.
struct Flow<'a, 'p> {
    ix: &'a Index<'p>,
    g: &'a mut Graph,
    problems: &'a mut Vec<Problem>,
    reported: &'a mut BTreeSet<(String, usize, String)>,
    /// Constants whose initialisers are waiting to be read.
    pending_consts: &'a mut Vec<(ItemId, NodeId)>,
    /// What every body shares.
    shared: &'a mut Shared,
    /// The body being read.
    body_id: Body,
    /// The node whose calls are being found: the body's own, or a closure's inside it.
    current: NodeId,
    scope: Scope,
    file: String,
    bindings: Vec<Val>,
    next_binding: usize,
    scopes: Vec<BTreeMap<String, usize>>,
    next_closure: usize,
    changed: bool,
    /// What the body's values carry, whose drop glue may run.
    dropped: Carries,
    /// The declared return type of the current function or closure.
    ret_ty: Ty,
    /// The break values of the enclosing loops.
    loops: Vec<Val>,
    /// How many macro expansions led here.
    depth: usize,
    /// How the edges being added were found, for the report.
    how: &'static str,
}

/// Finds every edge of every node of the program, into `g`; what cannot be read goes to
/// `problems`.
pub fn analyse(ix: &Index<'_>, g: &mut Graph, problems: &mut Vec<Problem>) {
    let mut reported = BTreeSet::new();
    let mut shared = Shared::default();
    let mut consts: Vec<(ItemId, NodeId)> = Vec::new();
    let mut todo: BTreeSet<Body> = (0..ix.program.nodes.len()).map(|i| Body::Node(NodeId(i))).collect();
    for round in 0..MAX_PROGRAM_ROUNDS {
        let mut pending: Vec<(ItemId, NodeId)> = Vec::new();
        for (i, node) in ix.program.nodes.iter().enumerate() {
            let id = NodeId(i);
            if !todo.contains(&Body::Node(id)) {
                continue;
            }
            let mut flow = Flow::new(
                ix,
                g,
                problems,
                &mut reported,
                &mut pending,
                &mut shared,
                Body::Node(id),
                node.scope.clone(),
                &node.file,
            );
            match &node.kind {
                NodeKind::Derived { item, trait_name } => flow.derived(*item, trait_name, node.line),
                _ => flow.body(id),
            }
        }
        // Constant initialisers: each newly read value's, and each earlier one whose inputs grew;
        // reading one may reach more.
        let mut queue: Vec<(ItemId, NodeId)> = consts
            .iter()
            .filter(|(item, node)| todo.contains(&Body::Const(*item, *node)))
            .copied()
            .collect();
        queue.append(&mut pending);
        let mut done: BTreeSet<ItemId> = BTreeSet::new();
        while let Some((item, node)) = queue.pop() {
            if !done.insert(item) {
                continue;
            }
            if !consts.contains(&(item, node)) {
                consts.push((item, node));
            }
            let it = ix.program.item(item);
            let file = ix.program.module(it.module).file.clone();
            let mut flow = Flow::new(
                ix,
                g,
                problems,
                &mut reported,
                &mut pending,
                &mut shared,
                Body::Const(item, node),
                it.scope.clone(),
                &file,
            );
            flow.constant(item, node);
            queue.append(&mut pending);
        }
        let reach = carrier_reach(g);
        let keys: BTreeSet<NodeId> = reach.keys().chain(shared.reach.keys()).copied().collect();
        for k in keys {
            if reach.get(&k) != shared.reach.get(&k)
                && let Some(readers) = shared.reach_readers.get(&k)
            {
                shared.dirty.extend(readers.iter().copied());
            }
        }
        shared.reach = reach;
        todo = core::mem::take(&mut shared.dirty);
        eprintln!("DEBUG round {round} next {} edges {}", todo.len(), g.edges.values().map(|m| m.len()).sum::<usize>());
        {
            let mut hist: BTreeMap<&str, usize> = BTreeMap::new();
            let mut per: BTreeMap<String, usize> = BTreeMap::new();
            for (from, m) in &g.edges { for site in m.values() { *hist.entry(site.how).or_default() += 1; } *per.entry(g.node(*from).map_or(String::new(), |n| n.name.clone())).or_default() += m.len(); }
            eprintln!("DEBUG hist {hist:?}");
            let mut v: Vec<_> = per.into_iter().collect(); v.sort_by_key(|x| std::cmp::Reverse(x.1));
            for (n, c) in v.iter().take(12) { eprintln!("DEBUG from {c} {n}"); }
            if round == 1 { let mut cs: Vec<_> = g.edges.iter().filter(|(f, m)| g.node(**f).is_some_and(|n| n.kind == GKind::Carrier) && m.len() > 40).collect(); cs.sort_by_key(|(_, m)| std::cmp::Reverse(m.len())); for (from, m) in cs.iter().take(8) { eprintln!("DEBUG carrier {} {}", m.len(), g.node(**from).map_or("", |n| n.name.as_str())); for (to, site) in m.iter().take(6) { eprintln!("DEBUG    {}:{} -> {}", site.file, site.line, g.node(*to).map_or("", |n| n.name.as_str())); } } }
        }
        if todo.is_empty() {
            break;
        }
        if round.saturating_add(1) == MAX_PROGRAM_ROUNDS {
            problems.push(Problem::unread(
                "",
                0,
                "the program's closure values and carriers still grow after the round limit",
            ));
        }
    }
    g.finish();
}

/// For each carrier, the closures, functions and the unknown-call node it may lead to through
/// carriers only.
fn carrier_reach(g: &Graph) -> BTreeMap<NodeId, BTreeSet<NodeId>> {
    let mut out = BTreeMap::new();
    for (i, n) in g.nodes.iter().enumerate() {
        if n.kind != GKind::Carrier {
            continue;
        }
        let start = NodeId(i);
        let mut seen: BTreeSet<NodeId> = BTreeSet::from([start]);
        let mut found: BTreeSet<NodeId> = BTreeSet::new();
        let mut work = vec![start];
        while let Some(c) = work.pop() {
            for to in g.edges.get(&c).into_iter().flat_map(BTreeMap::keys) {
                match g.node(*to).map(|x| x.kind) {
                    Some(GKind::Carrier) => {
                        if seen.insert(*to) {
                            work.push(*to);
                        }
                    }
                    Some(_) => {
                        found.insert(*to);
                    }
                    None => {}
                }
            }
        }
        if !found.is_empty() {
            out.insert(start, found);
        }
    }
    out
}

impl<'a, 'p> Flow<'a, 'p> {
    #[allow(clippy::too_many_arguments, reason = "the analysis's shared state, passed once")]
    fn new(
        ix: &'a Index<'p>,
        g: &'a mut Graph,
        problems: &'a mut Vec<Problem>,
        reported: &'a mut BTreeSet<(String, usize, String)>,
        pending_consts: &'a mut Vec<(ItemId, NodeId)>,
        shared: &'a mut Shared,
        body_id: Body,
        scope: Scope,
        file: &str,
    ) -> Flow<'a, 'p> {
        let current = match body_id {
            Body::Node(n) | Body::Const(_, n) => n,
        };
        Flow {
            ix,
            g,
            problems,
            reported,
            pending_consts,
            shared,
            body_id,
            current,
            scope,
            file: file.to_owned(),
            bindings: Vec::new(),
            next_binding: 0,
            scopes: vec![BTreeMap::new()],
            next_closure: 0,
            changed: false,
            dropped: Carries::default(),
            ret_ty: Ty::Unknown,
            loops: Vec::new(),
            depth: 0,
            how: "a call",
        }
    }

    fn resolver(&self) -> &Resolver<'p> {
        &self.ix.resolver
    }

    fn problem(&mut self, line: usize, what: String) {
        let key = (self.file.clone(), line, what.clone());
        if self.reported.insert(key) {
            self.problems.push(Problem::unread(&self.file, line, &what));
        }
    }

    fn site(&self, line: usize) -> Site {
        Site {
            file: self.file.clone(),
            line,
            how: self.how,
        }
    }

    fn edge(&mut self, to: NodeId, line: usize) {
        let site = self.site(line);
        self.g.edge(self.current, to, &site);
    }

    fn edge_rel(&mut self, to: NodeId, line: usize, scg: Scg) {
        let site = self.site(line);
        self.g.edge_rel(self.current, to, &site, scg);
    }

    fn edges(&mut self, to: &BTreeSet<NodeId>, line: usize) {
        for t in to {
            self.edge(*t, line);
        }
    }

    /// The function a node belongs to: itself, or for a closure the function it is written in.
    fn owner(&self, n: NodeId) -> NodeId {
        self.g.node(n).map_or(n, |x| x.owner)
    }

    /// The size-change graph of calling kernel function `target` with `args` (the receiver
    /// first for a method), the receiver's type `receiver` when a dispatch chose `target` for it.
    fn rel(&self, target: NodeId, receiver: Option<&Ty>, args: &[Val]) -> Scg {
        let caller = self.g.vars(self.current);
        let callee = self.g.vars(target);
        if caller.is_empty() || callee.is_empty() {
            return Scg::empty();
        }
        let Some(node) = self.ix.program.node(target) else {
            return Scg::empty();
        };
        let self_pattern: Option<Ty> = match &node.kind {
            NodeKind::Method(imp) => self.ix.impl_self.get(imp.0).cloned(),
            NodeKind::Derived { item, .. } => Some(Ty::Adt(
                *item,
                self.ix.program.item(*item).scope.generics.iter().map(|g| Ty::Param(*g)).collect(),
            )),
            NodeKind::TraitDefault(t) => match &self.ix.program.item(*t).kind {
                ItemKind::Trait { self_gen, .. } => Some(Ty::Param(*self_gen)),
                _ => None,
            },
            NodeKind::Fn => None,
        };
        let mut pairs: Vec<(Ty, Ty)> = Vec::new();
        if let (Some(sp), Some(r)) = (&self_pattern, receiver) {
            pairs.push((sp.clone(), r.clone()));
        }
        match &node.sig {
            Some(sig) => {
                for (input, a) in sig.inputs.iter().zip(args) {
                    let pattern = match input {
                        syn::FnArg::Receiver(r) => match &r.kind {
                            syn::ReceiverKind::Typed(_, t) => self.ix.resolver.ty(&node.scope, t),
                            _ => self_pattern.clone().unwrap_or(Ty::Unknown),
                        },
                        syn::FnArg::Typed(pt) => self.ix.resolver.ty(&node.scope, &pt.ty),
                    };
                    pairs.push((pattern, a.ty.clone()));
                }
            }
            None => {
                if let (Some(sp), Some(a)) = (&self_pattern, args.first()) {
                    pairs.push((sp.clone(), a.ty.clone()));
                }
            }
        }
        relate(&pairs, caller, callee)
    }

    /// The size-change graph of a call through a value that holds `target` itself: a closure
    /// made by the same function keeps its type parameters as they are; anything else states
    /// nothing.
    fn direct_rel(&self, target: NodeId) -> Scg {
        let is_closure = self.g.node(target).is_some_and(|n| n.kind == GKind::Closure);
        if is_closure && self.owner(target) == self.owner(self.current) {
            Scg::identity(self.g.vars(self.current).len())
        } else {
            Scg::empty()
        }
    }

    /// The declared type of parameter `index` of kernel function `f` (the receiver is 0), or
    /// unknown.
    fn param_ty(&self, f: NodeId, index: usize) -> Ty {
        let Some(node) = self.ix.program.node(f) else {
            return Ty::Unknown;
        };
        if let NodeKind::Derived { item, trait_name } = &node.kind {
            // A derived method takes `&self` and, for the comparisons, `&Self`; `fmt` a
            // `Formatter`; `hash` a generic hasher.
            let own = Ty::Adt(
                *item,
                self.ix.program.item(*item).scope.generics.iter().map(|g| Ty::Param(*g)).collect(),
            );
            return match (index, trait_name.as_str()) {
                (0, _) | (1, "PartialEq" | "PartialOrd" | "Ord") => Ty::Ref(Box::new(own)),
                (1, "Debug") => Ty::Ref(Box::new(Ty::Ext("Formatter".to_owned(), Vec::new()))),
                _ => Ty::Unknown,
            };
        }
        let Some(input) = node.sig.as_ref().and_then(|s| s.inputs.iter().nth(index)) else {
            return Ty::Unknown;
        };
        match input {
            syn::FnArg::Receiver(r) => match &r.kind {
                syn::ReceiverKind::Typed(_, t) => self.ix.resolver.ty(&node.scope, t),
                _ => self.ix.resolver.self_ty(&node.scope, 0),
            },
            syn::FnArg::Typed(pt) => self.ix.resolver.ty(&node.scope, &pt.ty),
        }
    }

    /// The declared type of field `name` of kernel type `item` (of its variant `variant`), or
    /// unknown.
    fn field_ty(&self, item: ItemId, variant: Option<usize>, name: &str) -> Ty {
        let it = self.ix.program.item(item);
        let fields = match (&it.kind, variant) {
            (ItemKind::Struct { fields, .. } | ItemKind::Union { fields, .. }, None) => Some(fields),
            (ItemKind::Enum { variants, .. }, Some(v)) => variants.get(v).map(|x| &x.fields),
            _ => None,
        };
        fields
            .and_then(|f| f.get(name))
            .map_or(Ty::Unknown, |t| self.ix.resolver.ty(&it.scope, t))
    }

    /// Callables in `carries` flow into carrier `into`, whose declared type is `declared`: only
    /// when that type can hold a callable.
    fn flow_typed(&mut self, into: NodeId, carries: &Carries, declared: &Ty, line: usize) {
        if declared.admits_callables() {
            self.flow_into(into, carries, line);
        }
    }

    /// Joins `v` into closure `c`'s parameter `i`.
    fn join_param(&mut self, c: NodeId, i: usize, v: &Val) {
        let v = match self.shared.typed.get(&(c, i)) {
            Some(t) => {
                let mut fixed = t.clone();
                fixed.carries.join(&v.carries_of_calls());
                fixed
            }
            None => v.clone(),
        };
        let slot = self.shared.closure_params.entry((c, i)).or_insert_with(|| Val::of(Ty::Never));
        let joined = slot.join(&v);
        if joined != *slot {
            *slot = joined;
            self.changed = true;
            if let Some(b) = self.shared.made_in.get(&c).copied()
                && b != self.body_id
            {
                self.shared.dirty.insert(b);
            }
        }
    }

    /// Records closure `c`'s parameter `i` as its context fixes it, `t`; a second context joins
    /// the first.
    fn fix_param(&mut self, c: NodeId, i: usize, t: &Val) {
        let joined = match self.shared.typed.get(&(c, i)) {
            Some(old) => {
                let j = old.join(t);
                if j == *old {
                    return;
                }
                j
            }
            None => t.clone(),
        };
        self.shared.typed.insert((c, i), joined.clone());
        // What reached it before keeps only its callables.
        let mut narrowed = joined;
        if let Some(v) = self.shared.closure_params.get(&(c, i)) {
            narrowed.carries.join(&v.carries_of_calls());
        }
        self.shared.closure_params.insert((c, i), narrowed);
        self.changed = true;
        if let Some(b) = self.shared.made_in.get(&c).copied()
            && b != self.body_id
        {
            self.shared.dirty.insert(b);
        }
    }

    /// A value as seen from a body of another function than the one its type was written in:
    /// that function's type parameters, unknown here, stand for every kernel type their bounds
    /// allow.
    fn foreign(&self, v: &Val, own: NodeId) -> Val {
        let mine: BTreeSet<GenId> = self.g.vars(self.owner(self.current)).iter().copied().collect();
        if self.owner(own) == self.owner(self.current) {
            return v.clone();
        }
        let theirs: BTreeSet<GenId> = v
            .carries
            .generics
            .iter()
            .chain(v.ty.carries().generics.iter())
            .copied()
            .filter(|g| !mine.contains(g))
            .collect();
        if theirs.is_empty() {
            return v.clone();
        }
        let mut carries = v.carries.clone();
        for g in &theirs {
            carries.generics.remove(g);
            carries.types.extend(self.ix.types_for(*g));
        }
        Val {
            ty: strip_params(&v.ty, &theirs),
            carries,
        }
    }

    /// Calls closure `c` with `vals` from this body: its parameters take them (unless its arity
    /// differs, which no real call can do); returns its result.
    fn enter_closure(&mut self, c: NodeId, vals: &[Val]) -> Val {
        let arity = self.g.node(c).and_then(|n| n.arity);
        if arity == Some(vals.len()) {
            for (i, v) in vals.iter().enumerate() {
                let v = self.foreign_into(v, c);
                self.join_param(c, i, &v);
            }
        }
        self.closure_ret(c)
    }

    /// Closure `c`'s result as this body sees it, recorded as read.
    fn closure_ret(&mut self, c: NodeId) -> Val {
        self.shared.ret_readers.entry(c).or_default().insert(self.body_id);
        let ret = self.shared.closure_rets.get(&c).cloned().unwrap_or_else(|| Val::of(Ty::Never));
        self.foreign(&ret, c)
    }

    /// Carrier `x`'s contents, recorded as read.
    fn reach_of(&mut self, x: NodeId) -> BTreeSet<NodeId> {
        self.shared.reach_readers.entry(x).or_default().insert(self.body_id);
        self.shared.reach.get(&x).cloned().unwrap_or_default()
    }

    /// A value of this body's, as closure `c`'s parameter sees it: unchanged if `c` belongs to
    /// this body's function, else with this function's type parameters made the kernel types
    /// their bounds allow.
    fn foreign_into(&self, v: &Val, c: NodeId) -> Val {
        if self.owner(c) == self.owner(self.current) {
            return v.clone();
        }
        let mine: BTreeSet<GenId> = self.g.vars(self.owner(self.current)).iter().copied().collect();
        let used: BTreeSet<GenId> = v
            .carries
            .generics
            .iter()
            .chain(v.ty.carries().generics.iter())
            .copied()
            .filter(|g| mine.contains(g))
            .collect();
        if used.is_empty() {
            return v.clone();
        }
        let mut carries = v.carries.clone();
        for g in &used {
            carries.generics.remove(g);
            carries.types.extend(self.ix.types_for(*g));
        }
        Val {
            ty: strip_params(&v.ty, &used),
            carries,
        }
    }

    /// Calls kernel function `f` through a value with `vals`: its parameters take the callables
    /// their types admit; returns its declared result.
    fn enter_fn(&mut self, f: NodeId, vals: &[Val], line: usize) -> Val {
        if self.ix.program.node(f).is_none() {
            return Val::unknown(Carries::default());
        }
        for (i, v) in vals.iter().enumerate() {
            if !v.carries.calls.is_empty() || v.carries.top {
                let p = self.g.param(f, i);
                let declared = self.param_ty(f, i);
                self.flow_typed(p, &v.carries, &declared, line);
            }
        }
        self.returns(&BTreeSet::from([f]), vals, None)
    }

    /// The closures and kernel functions a value holding `carries` may hold: itself, and through
    /// its carriers.
    fn held(&mut self, carries: &Carries) -> BTreeSet<NodeId> {
        let mut out = BTreeSet::new();
        for c in &carries.calls {
            match self.g.node(*c).map(|n| n.kind) {
                Some(GKind::Carrier) => {
                    let r = self.reach_of(*c);
                    out.extend(r);
                }
                Some(_) => {
                    out.insert(*c);
                }
                None => {}
            }
        }
        out
    }

    /// Callables in `carries` flow into carrier `into`.
    fn flow_into(&mut self, into: NodeId, carries: &Carries, line: usize) {
        let site = Site {
            file: self.file.clone(),
            line,
            how: "a callable flowing into a carrier",
        };
        for c in &carries.calls {
            if *c != into {
                self.g.edge(into, *c, &site);
            }
        }
        if carries.top {
            let any = self.g.any();
            self.g.edge(into, any, &site);
        }
    }

    fn ty(&self, t: &syn::Type) -> Ty {
        self.resolver().ty(&self.scope, t)
    }

    // ------------------------------------------------------------------ bindings

    fn bind(&mut self, name: &str, val: Val) {
        let id = self.next_binding;
        self.next_binding = self.next_binding.saturating_add(1);
        match self.bindings.get_mut(id) {
            Some(old) => {
                let joined = old.join(&val);
                if joined != *old {
                    *old = joined;
                    self.changed = true;
                }
            }
            None => {
                self.bindings.push(val);
                self.changed = true;
            }
        }
        if let Some(s) = self.scopes.last_mut() {
            s.insert(name.to_owned(), id);
        }
    }

    fn lookup_local(&self, name: &str) -> Option<usize> {
        self.scopes.iter().rev().find_map(|s| s.get(name).copied())
    }

    fn local(&self, id: usize) -> Val {
        self.bindings.get(id).cloned().unwrap_or_else(|| Val::unknown(Carries::top()))
    }

    fn push_scope(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    /// Writes `carries` into a place.
    fn write(&mut self, place: &Place, carries: &Carries, line: usize) {
        match place {
            Place::Local(id, through) => {
                // Only a write through a reference reaches what it refers to, which its carriers
                // stand for; a write to an owned value or to the binding itself stays in it.
                let carriers: Vec<NodeId> = self
                    .bindings
                    .get(*id)
                    .filter(|v| *through && matches!(v.ty, Ty::Ref(_) | Ty::Unknown | Ty::Param(_) | Ty::Assoc(..)))
                    .map(|v| {
                        v.carries
                            .calls
                            .iter()
                            .copied()
                            .filter(|c| self.g.node(*c).is_some_and(|n| n.kind == GKind::Carrier))
                            .collect()
                    })
                    .unwrap_or_default();
                if let Some(b) = self.bindings.get_mut(*id)
                    && b.carries.join(carries)
                {
                    self.changed = true;
                }
                for c in carriers {
                    self.flow_into(c, carries, line);
                }
            }
            Place::Field(item, variant, field) => {
                let node = self.g.field(self.ix, *item, *variant, field);
                let declared = self.field_ty(*item, *variant, field);
                self.flow_typed(node, carries, &declared, line);
            }
            Place::Other => {}
        }
    }

    /// The place an expression denotes, if it is one the analysis follows: a binding named
    /// alone is written as a whole, any other way through it.
    fn place(&mut self, e: &Expr) -> Place {
        match self.place_inner(e) {
            Place::Local(id, _) if !matches!(e, Expr::Path(_)) => Place::Local(id, true),
            other => other,
        }
    }

    /// The place `e` names, a binding named alone read as written whole.
    fn place_inner(&mut self, e: &Expr) -> Place {
        match e {
            Expr::Path(p) if p.qself.is_none() => match p.path.get_ident() {
                Some(id) => self
                    .lookup_local(&id.to_string())
                    .map_or(Place::Other, |b| Place::Local(b, false)),
                None => Place::Other,
            },
            Expr::Field(f) => {
                let base = self.peek(&f.base);
                match base.ty.peel() {
                    Ty::Adt(id, _) => Place::Field(*id, None, member_name(&f.member)),
                    _ => self.place_inner(&f.base),
                }
            }
            Expr::Index(i) => self.place_inner(&i.expr),
            Expr::Paren(p) => self.place_inner(&p.expr),
            Expr::Group(g) => self.place_inner(&g.expr),
            Expr::Reference(r) => self.place_inner(&r.expr),
            Expr::Unary(u) if matches!(u.op, UnOp::Deref(_)) => self.place_inner(&u.expr),
            _ => Place::Other,
        }
    }

    /// The value of a place expression without its calls (for `place`), cheaply: locals and
    /// fields only.
    fn peek(&mut self, e: &Expr) -> Val {
        match e {
            Expr::Path(p) if p.qself.is_none() => match p.path.get_ident() {
                Some(id) if id == "self" || self.lookup_local(&id.to_string()).is_some() => self
                    .lookup_local(&id.to_string())
                    .map_or_else(|| Val::unknown(Carries::default()), |b| self.local(b)),
                _ => Val::unknown(Carries::default()),
            },
            Expr::Field(f) => {
                let base = self.peek(&f.base);
                self.field_of(&base, &member_name(&f.member))
            }
            Expr::Paren(p) => self.peek(&p.expr),
            Expr::Reference(r) => self.peek(&r.expr),
            Expr::Unary(u) if matches!(u.op, UnOp::Deref(_)) => self.peek(&u.expr),
            _ => Val::unknown(Carries::default()),
        }
    }

    // ------------------------------------------------------------------ nodes

    /// A function, method or trait default body, to a fixed point.
    fn body(&mut self, id: NodeId) {
        let Some(node) = self.ix.program.node(id) else {
            return;
        };
        let (Some(sig), Some(block)) = (node.sig.clone(), node.body.clone()) else {
            return;
        };
        let line = node.line;
        for round in 0..MAX_ROUNDS {
            self.next_binding = 0;
            self.next_closure = 0;
            self.scopes = vec![BTreeMap::new()];
            self.changed = false;
            self.current = id;
            self.ret_ty = match &sig.output {
                syn::ReturnType::Type(_, t) => self.ty(t),
                syn::ReturnType::Default => Ty::unit(),
            };
            self.params(id, &sig);
            let v = self.block(&block);
            let ret = self.g.ret(id);
            let declared = self.ret_ty.clone();
            self.flow_typed(ret, &v.carries, &declared, line);
            self.drop_glue(line);
            if !self.changed {
                return;
            }
            if round.saturating_add(1) == MAX_ROUNDS {
                self.problem(line, "a body whose values still grow after the round limit".to_owned());
            }
        }
    }

    /// Binds a function's parameters: each carries its carrier node.
    fn params(&mut self, id: NodeId, sig: &syn::Signature) {
        for (i, input) in sig.inputs.iter().enumerate() {
            let ty = match input {
                syn::FnArg::Receiver(r) => {
                    let self_ty = self.resolver().self_ty(&self.scope, 0);
                    match &r.kind {
                        syn::ReceiverKind::Reference(..) => Ty::Ref(Box::new(self_ty)),
                        syn::ReceiverKind::Typed(_, t) => self.ty(t),
                        _ => self_ty,
                    }
                }
                syn::FnArg::Typed(t) => self.ty(&t.ty),
            };
            let mut extra = Carries::default();
            if ty.admits_callables() {
                extra.calls.insert(self.g.param(id, i));
            }
            match input {
                syn::FnArg::Receiver(_) => self.bind("self", Val::with(ty, &extra)),
                syn::FnArg::Typed(t) => self.pattern(&t.pat, &Val::with(ty, &extra)),
            }
        }
    }

    /// A derived implementation: the trait's method on every field.
    fn derived(&mut self, item: ItemId, trait_name: &str, line: usize) {
        let it = self.ix.program.item(item);
        let fields: Vec<syn::Type> = match &it.kind {
            ItemKind::Struct { fields, .. } | ItemKind::Union { fields, .. } => {
                fields.all().into_iter().map(|(_, t)| t.clone()).collect()
            }
            ItemKind::Enum { variants, .. } => variants
                .iter()
                .flat_map(|v| v.fields.all().into_iter().map(|(_, t)| t.clone()))
                .collect(),
            _ => Vec::new(),
        };
        let method = crate::recursion::model::DERIVABLE
            .iter()
            .find(|(t, _)| *t == trait_name)
            .and_then(|(_, m)| m.first().copied())
            .unwrap_or("");
        for f in fields {
            let v = Val::of(self.ty(&f));
            if trait_name == "Default" {
                self.trait_call(&v, "Default", "default", &[], line);
            } else {
                self.trait_call(&v, trait_name, method, &[v.clone()], line);
            }
        }
    }

    /// A constant's initialiser: its calls are made when the crate compiles, but the callables
    /// in its value flow out through its carrier.
    fn constant(&mut self, item: ItemId, node: NodeId) {
        let ItemKind::Const { init: Some(init), .. } = &self.ix.program.item(item).kind else {
            return;
        };
        let init = init.clone();
        let line = init.span().start().line;
        for _ in 0..MAX_ROUNDS {
            self.next_binding = 0;
            self.next_closure = 0;
            self.changed = false;
            self.current = node;
            let v = self.expr(&init);
            let ret = self.g.ret(node);
            let declared = match &self.ix.program.item(item).kind {
                ItemKind::Const { ty, .. } => {
                    let scope = self.ix.program.item(item).scope.clone();
                    self.ix.resolver.ty(&scope, ty)
                }
                _ => Ty::Unknown,
            };
            self.flow_typed(ret, &v.carries, &declared, line);
            if !self.changed {
                break;
            }
        }
    }

    /// The value of reading constant `item`.
    fn const_value(&mut self, item: ItemId) -> Val {
        let (node, new) = self.g.constant(self.ix, item);
        if new {
            self.pending_consts.push((item, node));
        }
        let ty = match &self.ix.program.item(item).kind {
            ItemKind::Const { ty, .. } => {
                let scope = self.ix.program.item(item).scope.clone();
                self.ix.resolver.ty(&scope, ty)
            }
            _ => Ty::Unknown,
        };
        let ret = self.g.ret(node);
        let mut extra = Carries::default();
        extra.calls.insert(ret);
        Val::with(ty, &extra)
    }

    // ------------------------------------------------------------------ statements

    fn block(&mut self, b: &syn::Block) -> Val {
        self.push_scope();
        let v = self.stmts(&b.stmts);
        self.pop_scope();
        v
    }

    fn stmts(&mut self, stmts: &[Stmt]) -> Val {
        let mut last = Val::unit();
        for s in stmts {
            last = self.stmt(s);
        }
        last
    }

    fn stmt(&mut self, s: &Stmt) -> Val {
        match s {
            Stmt::Local(l) => {
                if !crate::recursion::cfg::ships(&l.attrs) {
                    return Val::unit();
                }
                let v = match &l.init {
                    Some(init) => {
                        let v = self.expr(&init.expr);
                        if let Some((_, diverge)) = &init.diverge {
                            self.expr(diverge);
                        }
                        v
                    }
                    None => Val::unknown(Carries::default()),
                };
                self.pattern(&l.pat, &v);
                Val::unit()
            }
            Stmt::Item(_) => Val::unit(),
            Stmt::Expr(e, semi) => {
                let v = self.expr(e);
                if semi.is_some() { Val::unit() } else { v }
            }
            Stmt::Macro(m) => {
                if !crate::recursion::cfg::ships(&m.attrs) {
                    return Val::unit();
                }
                self.mac(&m.mac, true)
            }
        }
    }

    // ------------------------------------------------------------------ patterns

    /// Binds every name of `pat` to the part of `v` it takes.
    fn pattern(&mut self, pat: &Pat, v: &Val) {
        match pat {
            Pat::Ident(p) => {
                let name = p.ident.to_string();
                if p.subpat.is_none() && self.names_constant(&name) {
                    return;
                }
                self.bind(&name, v.clone());
                if let Some((_, sub)) = &p.subpat {
                    self.pattern(sub, v);
                }
            }
            Pat::Tuple(t) => {
                for (i, e) in t.elems.iter().enumerate() {
                    let part = match v.ty.peel() {
                        Ty::Tuple(items) => items.get(i).cloned().map_or_else(
                            || Val::unknown(v.carries.clone()),
                            |t| Val::with(t, &v.carries_of_calls()),
                        ),
                        _ => Val::unknown(v.carries.clone()),
                    };
                    self.pattern(e, &part);
                }
            }
            Pat::TupleStruct(ts) => {
                let parts = self.ctor_fields(&ts.path, v, ts.elems.len());
                for (e, part) in ts.elems.iter().zip(parts) {
                    self.pattern(e, &part);
                }
            }
            Pat::Struct(s) => {
                let names: Vec<String> = s.fields.iter().map(|f| member_name(&f.member)).collect();
                let parts = self.struct_fields(&s.path, v, &names);
                for (f, part) in s.fields.iter().zip(parts) {
                    self.pattern(&f.pat, &part);
                }
            }
            Pat::Reference(r) => {
                let inner = Val {
                    ty: v.ty.peel().clone(),
                    carries: v.carries.clone(),
                };
                self.pattern(&r.pat, &inner);
            }
            Pat::Slice(s) => {
                let elem = elem_of(v);
                for e in &s.elems {
                    self.pattern(e, &elem);
                }
            }
            Pat::Or(o) => {
                for case in &o.cases {
                    self.pattern(case, v);
                }
            }
            Pat::Type(t) => {
                let ty = self.ty(&t.ty);
                let typed = Val::with(ty, &v.carries_of_calls());
                self.pattern(&t.pat, &typed);
            }
            Pat::Paren(p) => self.pattern(&p.pat, v),
            Pat::Guard(g) => {
                self.pattern(&g.pat, v);
                self.expr(&g.guard);
            }
            Pat::Wild(_) | Pat::Rest(_) | Pat::Lit(_) | Pat::Range(_) | Pat::Path(_) | Pat::Const(_) => {}
            Pat::Macro(m) => {
                let line = m.mac.path.span().start().line;
                self.problem(line, "a macro in a pattern, which the recursion check does not expand".to_owned());
            }
            other => {
                let line = other.span().start().line;
                self.problem(line, "a pattern form the recursion check does not know".to_owned());
            }
        }
    }

    /// Whether an identifier pattern names a constant, unit struct or unit variant rather than
    /// binding a new name.
    fn names_constant(&self, name: &str) -> bool {
        let found = self.resolver().resolve(&self.scope, &[name.to_owned()], false, Ns::Value);
        found.iter().any(|r| match r {
            Res::Variant(..) => true,
            Res::Item(id) => matches!(
                self.ix.program.item(*id).kind,
                ItemKind::Const { .. } | ItemKind::Struct { .. }
            ),
            Res::Ext(p) => p.len() == 1 && p.first().is_some_and(|s| s == "None"),
            _ => false,
        })
    }

    /// The values of a tuple struct or tuple variant pattern's `n` fields.
    fn ctor_fields(&mut self, path: &syn::Path, v: &Val, n: usize) -> Vec<Val> {
        let segs = Resolver::segments(path);
        let found = self.resolver().resolve(&self.scope, &segs, path.leading_colon.is_some(), Ns::Value);
        let names: Vec<String> = (0..n).map(|i| i.to_string()).collect();
        match found.first() {
            Some(Res::Ext(p)) => {
                let inner = match (p.last().map(String::as_str), v.ty.peel()) {
                    (Some("Some"), Ty::Ext(o, args)) if o == "Option" => args.first().cloned(),
                    (Some("Ok"), Ty::Ext(r, args)) if r == "Result" => args.first().cloned(),
                    (Some("Err"), Ty::Ext(r, args)) if r == "Result" => args.get(1).cloned(),
                    _ => None,
                };
                (0..n)
                    .map(|_| match &inner {
                        Some(t) if t.is_known() => Val::with(t.clone(), &v.carries_of_calls()),
                        _ => Val::unknown(v.carries.clone()),
                    })
                    .collect()
            }
            _ => self.fields_by_res(found.first(), v, &names),
        }
    }

    /// The values of a struct pattern's named fields.
    fn struct_fields(&mut self, path: &syn::Path, v: &Val, names: &[String]) -> Vec<Val> {
        let segs = Resolver::segments(path);
        let mut found = self.resolver().resolve(&self.scope, &segs, path.leading_colon.is_some(), Ns::Value);
        if found.is_empty() {
            found = self.resolver().resolve(&self.scope, &segs, path.leading_colon.is_some(), Ns::Type);
        }
        self.fields_by_res(found.first(), v, names)
    }

    fn fields_by_res(&mut self, res: Option<&Res>, v: &Val, names: &[String]) -> Vec<Val> {
        let (item, variant) = match res {
            Some(Res::Item(id)) => (Some(*id), None),
            Some(Res::Variant(id, var)) => (Some(*id), Some(*var)),
            Some(Res::SelfTy) => match self.resolver().self_ty(&self.scope, 0) {
                Ty::Adt(id, _) => (Some(id), None),
                _ => (None, None),
            },
            _ => (None, None),
        };
        let Some(item) = item else {
            return names.iter().map(|_| Val::unknown(v.carries.clone())).collect();
        };
        let args = match v.ty.peel() {
            Ty::Adt(id, args) if *id == item => args.clone(),
            _ => Vec::new(),
        };
        names
            .iter()
            .map(|n| self.member(item, variant, &args, n, &v.carries))
            .collect()
    }

    /// The value of field `name` of kernel type `item` (variant `variant`) with arguments `args`.
    fn member(&mut self, item: ItemId, variant: Option<usize>, args: &[Ty], name: &str, outer: &Carries) -> Val {
        let it = self.ix.program.item(item);
        let fields: Option<&Fields> = match (&it.kind, variant) {
            (ItemKind::Struct { fields, .. } | ItemKind::Union { fields, .. }, None) => Some(fields),
            (ItemKind::Enum { variants, .. }, Some(v)) => variants.get(v).map(|x| &x.fields),
            _ => None,
        };
        let carrier = self.g.field(self.ix, item, variant, name);
        let mut extra = Carries::default();
        extra.calls.insert(carrier);
        let Some(t) = fields.and_then(|f| f.get(name)) else {
            let mut c = outer.clone();
            c.calls.insert(carrier);
            return Val::unknown(c);
        };
        let scope = it.scope.clone();
        let ty = self.ix.resolver.ty(&scope, t);
        let map: BTreeMap<GenId, Ty> = it
            .scope
            .generics
            .iter()
            .copied()
            .zip(args.iter().cloned())
            .filter(|(_, a)| a.is_known())
            .collect();
        Val::with(ty.subst(&map), &extra)
    }

    /// The value of `base.name`.
    fn field_of(&mut self, base: &Val, name: &str) -> Val {
        let mut ty = base.ty.peel().clone();
        for _ in 0..crate::recursion::index::DEREF_DEPTH {
            match &ty {
                Ty::Adt(id, args) => {
                    let has = match &self.ix.program.item(*id).kind {
                        ItemKind::Struct { fields, .. } | ItemKind::Union { fields, .. } => {
                            fields.get(name).is_some()
                        }
                        _ => false,
                    };
                    if has {
                        let (id, args) = (*id, args.clone());
                        return self.member(id, None, &args, name, &base.carries);
                    }
                    match self.deref_target(&ty) {
                        Some(t) => ty = t.peel().clone(),
                        None => break,
                    }
                }
                Ty::Tuple(items) => {
                    if let Some(t) = name.parse::<usize>().ok().and_then(|i| items.get(i)) {
                        return Val::with(t.clone(), &base.carries_of_calls());
                    }
                    break;
                }
                _ => break,
            }
        }
        Val::unknown(base.carries.clone())
    }

    fn deref_target(&self, ty: &Ty) -> Option<Ty> {
        let Ty::Adt(id, _) = ty else {
            return None;
        };
        for imp in self.ix.by_ext_trait.get("Deref")? {
            if key(self.ix.impl_self.get(imp.0)?) == Key::Adt(*id) {
                let target = self.ix.program.imp(*imp).assoc.get("Target")?;
                return Some(self.ix.resolver.ty(&self.ix.program.imp(*imp).scope, target));
            }
        }
        None
    }

    // ------------------------------------------------------------------ expressions

    #[allow(
        clippy::too_many_lines,
        reason = "one arm per expression form of the Rust grammar; each arm delegates"
    )]
    /// The value of an expression, its calls added; its kernel types join what the current body
    /// may drop.
    fn expr(&mut self, e: &Expr) -> Val {
        let v = self.expr_inner(e);
        self.dropped.types.extend(v.carries.types.iter().copied());
        self.dropped.generics.extend(v.carries.generics.iter().copied());
        self.dropped.top |= v.carries.top;
        v
    }

    fn expr_inner(&mut self, e: &Expr) -> Val {
        let line = e.span().start().line;
        match e {
            Expr::Array(a) => {
                let mut v = Val::of(Ty::Array(Box::new(Ty::Never)));
                let mut elem: Option<Val> = None;
                for x in &a.elems {
                    let xv = self.expr(x);
                    elem = Some(elem.map_or(xv.clone(), |p| p.join(&xv)));
                }
                if let Some(el) = elem {
                    v = Val::with(Ty::Array(Box::new(el.ty)), &el.carries);
                }
                v
            }
            Expr::Assign(a) => {
                let rhs = self.expr(&a.right);
                let place = self.place(&a.left);
                self.expr(&a.left);
                self.write(&place, &rhs.carries, line);
                Val::unit()
            }
            Expr::Async(a) => {
                self.block(&a.block);
                Val::unknown(Carries::default())
            }
            Expr::Await(a) => {
                let v = self.expr(&a.base);
                self.outside_call(&[v.clone()], line);
                Val::unknown(v.carries)
            }
            Expr::Binary(b) => self.binary(b, line),
            Expr::Block(b) => self.block(&b.block),
            Expr::Break(b) => {
                if let Some(x) = &b.expr {
                    let v = self.expr(x);
                    if let Some(l) = self.loops.last_mut() {
                        *l = l.join(&v);
                    }
                }
                Val::of(Ty::Never)
            }
            Expr::Call(c) => self.call(c, line),
            Expr::Cast(c) => {
                let v = self.expr(&c.expr);
                let ty = self.ty(&c.ty);
                Val::with(ty, &v.carries_of_calls())
            }
            Expr::Closure(c) => self.closure(c, None, line),
            Expr::Const(c) => self.block(&c.block),
            Expr::Continue(_) => Val::of(Ty::Never),
            Expr::Field(f) => {
                let base = self.expr(&f.base);
                self.field_of(&base, &member_name(&f.member))
            }
            Expr::ForLoop(f) => self.for_loop(f, line),
            Expr::Group(g) => self.expr(&g.expr),
            Expr::If(i) => {
                self.push_scope();
                self.expr(&i.cond);
                let then = self.block(&i.then_branch);
                self.pop_scope();
                match &i.else_branch {
                    Some((_, other)) => {
                        let o = self.expr(other);
                        then.join(&o)
                    }
                    None => Val::unit(),
                }
            }
            Expr::Index(i) => self.index(i, line),
            Expr::Infer(_) => Val::unknown(Carries::default()),
            Expr::Let(l) => {
                let v = self.expr(&l.expr);
                self.pattern(&l.pat, &v);
                Val::of(Ty::Prim("bool".to_owned()))
            }
            Expr::Lit(l) => Val::of(lit_ty(&l.lit)),
            Expr::Loop(l) => {
                self.loops.push(Val::of(Ty::Never));
                self.block(&l.body);
                self.loops.pop().unwrap_or_else(|| Val::unknown(Carries::default()))
            }
            Expr::Macro(m) => self.mac(&m.mac, false),
            Expr::Match(m) => {
                let scrutinee = self.expr(&m.expr);
                let mut out: Option<Val> = None;
                for arm in &m.arms {
                    self.push_scope();
                    self.pattern(&arm.pat, &scrutinee);
                    let v = self.expr(&arm.body);
                    self.pop_scope();
                    out = Some(out.map_or(v.clone(), |o| o.join(&v)));
                }
                out.unwrap_or_else(|| Val::of(Ty::Never))
            }
            Expr::MethodCall(m) => self.method_call(m, line),
            Expr::Paren(p) => self.expr(&p.expr),
            Expr::Path(p) => self.path_value(p, line),
            Expr::Range(r) => {
                let mut c = Carries::default();
                let mut t = Ty::Unknown;
                for x in [&r.start, &r.end].into_iter().flatten() {
                    let v = self.expr(x);
                    c.join(&v.carries);
                    t = v.ty;
                }
                Val::with(Ty::Ext("Range".to_owned(), vec![t]), &c)
            }
            Expr::RawAddr(r) => {
                let v = self.expr(&r.expr);
                Val::with(Ty::Ext("ptr".to_owned(), vec![v.ty.clone()]), &v.carries)
            }
            Expr::Reference(r) => {
                let v = self.expr(&r.expr);
                Val {
                    ty: if v.ty.is_known() { Ty::Ref(Box::new(v.ty)) } else { Ty::Unknown },
                    carries: v.carries,
                }
            }
            Expr::Repeat(r) => {
                let v = self.expr(&r.expr);
                self.expr(&r.len);
                Val::with(Ty::Array(Box::new(v.ty.clone())), &v.carries)
            }
            Expr::Return(r) => {
                if let Some(x) = &r.expr {
                    let v = self.expr(x);
                    let ret = self.g.ret(self.current);
                    let declared = self.ret_ty.clone();
                    self.flow_typed(ret, &v.carries, &declared, line);
                    if self.g.node(self.current).is_some_and(|n| n.kind == GKind::Closure) {
                        self.closure_result(&v);
                    }
                }
                Val::of(Ty::Never)
            }
            Expr::Struct(s) => self.struct_lit(s, line),
            Expr::Try(t) => self.try_op(t, line),
            Expr::TryBlock(t) => self.block(&t.block),
            Expr::Tuple(t) => {
                let mut tys = Vec::new();
                let mut c = Carries::default();
                for x in &t.elems {
                    let v = self.expr(x);
                    c.join(&v.carries);
                    tys.push(v.ty);
                }
                Val::with(Ty::Tuple(tys), &c)
            }
            Expr::Unary(u) => self.unary(u, line),
            Expr::Unsafe(u) => self.block(&u.block),
            Expr::While(w) => {
                self.push_scope();
                self.expr(&w.cond);
                self.loops.push(Val::of(Ty::Never));
                self.block(&w.body);
                self.loops.pop();
                self.pop_scope();
                Val::unit()
            }
            Expr::Yield(y) => {
                if let Some(x) = &y.expr {
                    self.expr(x);
                }
                Val::unknown(Carries::default())
            }
            _ => {
                self.problem(line, "an expression form the recursion check does not know".to_owned());
                Val::unknown(Carries::top())
            }
        }
    }

    /// The value of a path in value position (not called).
    fn path_value(&mut self, p: &syn::ExprPath, line: usize) -> Val {
        if p.qself.is_none()
            && let Some(id) = p.path.get_ident()
            && let Some(b) = self.lookup_local(&id.to_string())
        {
            return self.local(b);
        }
        let found = self.resolve_value(p);
        if found.is_empty() {
            let shown = quote::ToTokens::to_token_stream(&p.path).to_string();
            self.problem(line, format!("a path that resolves to nothing: `{shown}`"));
            return Val::unknown(Carries::top());
        }
        let mut out: Option<Val> = None;
        for r in found {
            let v = self.res_value(&r);
            out = Some(out.map_or(v.clone(), |o| o.join(&v)));
        }
        out.unwrap_or_else(|| Val::unknown(Carries::default()))
    }

    /// What a value path resolves to: a qualified path `<T as Trait>::f` resolves to `T`'s
    /// associated `f`.
    fn resolve_value(&self, p: &syn::ExprPath) -> Vec<Res> {
        if let Some(q) = &p.qself {
            let name = p.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
            let t = self.ty(&q.ty);
            let trait_segs: Vec<String> = p
                .path
                .segments
                .iter()
                .take(q.position)
                .map(|s| s.ident.to_string())
                .collect();
            let tr = if trait_segs.is_empty() {
                None
            } else {
                let found = self
                    .resolver()
                    .resolve(&self.scope, &trait_segs, p.path.leading_colon.is_some(), Ns::Type)
                    .into_iter()
                    .next()
                    .unwrap_or(Res::Ext(trait_segs));
                Some(Box::new(found))
            };
            return vec![Res::Assoc(Box::new(Res::Qualified(Box::new(t), tr)), name)];
        }
        let segs = Resolver::segments(&p.path);
        self.resolver().resolve(&self.scope, &segs, p.path.leading_colon.is_some(), Ns::Value)
    }

    fn res_value(&mut self, r: &Res) -> Val {
        match r {
            Res::Fn(n) => {
                self.g.address_taken.insert(*n);
                let mut c = Carries::default();
                c.calls.insert(*n);
                Val::with(Ty::FnDef(*n), &c)
            }
            Res::Item(id) => match &self.ix.program.item(*id).kind {
                ItemKind::Const { .. } => self.const_value(*id),
                ItemKind::Struct { .. } => Val::of(Ty::Adt(*id, Vec::new())),
                _ => Val::unknown(Carries::default()),
            },
            Res::Variant(id, _) => Val::of(Ty::Adt(*id, Vec::new())),
            Res::Assoc(base, name) => {
                // A method used as a value (`Self::helper` passed to `map`): its address is taken.
                let nodes = self.assoc_nodes(base, name, None);
                let mut c = Carries::default();
                for n in &nodes {
                    self.g.address_taken.insert(*n);
                    c.calls.insert(*n);
                }
                // Unless it names a kernel type's own function, it may be an outside one (an
                // implementation in the standard library, `i128::from`).
                let kernel_only = !nodes.is_empty()
                    && (matches!(&**base, Res::SelfTy)
                        || matches!(&**base, Res::Item(id) if self.ix.program.item(*id).is_adt()));
                if !kernel_only {
                    c.calls.insert(self.g.outside());
                }
                match nodes.iter().next() {
                    Some(first) => Val::with(Ty::FnDef(*first), &c),
                    None => Val::with(Ty::FnPtr(0), &c),
                }
            }
            Res::Ext(p) => match p.last().map(String::as_str) {
                Some("None") => Val::of(Ty::Ext("Option".to_owned(), vec![Ty::Unknown])),
                _ => {
                    // An outside function or constructor used as a value.
                    let mut c = Carries::default();
                    c.calls.insert(self.g.outside());
                    Val::with(Ty::FnPtr(0), &c)
                }
            },
            Res::ConstParam(_) => Val::of(Ty::Prim("usize".to_owned())),
            Res::SelfTy => Val::of(self.resolver().self_ty(&self.scope, 0)),
            Res::Prim(_) | Res::Generic(_) | Res::Mod(_) | Res::Macro(_) | Res::Qualified(..) => {
                Val::unknown(Carries::default())
            }
        }
    }

    // ------------------------------------------------------------------ calls

    /// Adds the callbacks outside code may make with `inputs`: what they carry.
    fn outside_call(&mut self, inputs: &[Val], line: usize) {
        let mut all = Carries::default();
        for v in inputs {
            all.join(&v.carries);
        }
        let targets = self.ix.callbacks(&all);
        self.how = "a callback of outside code";
        for (t, vias) in targets {
            for via in vias {
                let scg = match via {
                    Via::Value => self.direct_rel(t),
                    Via::Type => Scg::empty(),
                    Via::Generic(g) => self.rel(t, Some(&Ty::Param(g)), &[]),
                };
                self.edge_rel(t, line, scg);
            }
        }
        if all.top {
            let any = self.g.any();
            self.edge(any, line);
        }
    }

    /// Calls `targets` (kernel functions, each with the receiver type a dispatch chose it for)
    /// with `args` (the receiver first for a method), the argument places given: edges with their
    /// size-change graphs, callables into the parameter carriers whose types admit them, and
    /// parameter carriers into places passed by `&mut`.
    fn call_kernel(&mut self, targets: &[Target], args: &[Val], places: &[Place], line: usize, how: &'static str) {
        for (t, receiver) in targets {
            self.how = how;
            let scg = self.rel(*t, receiver.as_ref(), args);
            self.edge_rel(*t, line, scg);
            // A parameter takes callables only if its type, with the callee's type parameters
            // this call instantiates, can hold one.
            let map = self.learned(*t, args, receiver.as_ref());
            for (i, a) in args.iter().enumerate() {
                if !a.carries.calls.is_empty() || a.carries.top {
                    let p = self.g.param(*t, i);
                    let declared = self.param_ty(*t, i).subst(&map);
                    self.flow_typed(p, &a.carries, &declared, line);
                }
            }
            for (i, place) in places.iter().enumerate() {
                if *place != Place::Other && self.param_ty(*t, i).subst(&map).admits_callables() {
                    let p = self.g.param(*t, i);
                    let mut c = Carries::default();
                    c.calls.insert(p);
                    self.write(place, &c, line);
                }
            }
        }
    }

    /// The declared return value of calling `targets` with `args`, generic parameters learned
    /// from the arguments; each target's return carrier included when its type admits a
    /// callable, and the kernel types any type parameter of the callee left unknown may be.
    fn returns(&mut self, targets: &BTreeSet<NodeId>, args: &[Val], receiver: Option<&Ty>) -> Val {
        let mut out: Option<Val> = None;
        for t in targets {
            let (ty, unknown) = self.declared_return(*t, args, receiver);
            let mut extra = Carries::default();
            if ty.admits_callables() {
                extra.calls.insert(self.g.ret(*t));
            }
            for g in &unknown {
                let held = self.unlearned(*t, *g, args);
                extra.join(&held);
            }
            let v = Val::with(ty, &extra);
            out = Some(out.map_or(v.clone(), |o| o.join(&v)));
        }
        out.unwrap_or_else(|| Val::unknown(Carries::default()))
    }

    /// What a value of callee `t`'s type parameter `g`, which the call site did not show, may
    /// hold: what the arguments whose declared types mention `g` hold, since `g` is instantiated
    /// with a part of their types (a value of a type parameter holds only what its instantiation
    /// does); or, when no parameter's type mentions it (it is fixed by the expected result or a
    /// turbofish), every kernel type its bounds allow.
    fn unlearned(&self, t: NodeId, g: GenId, args: &[Val]) -> Carries {
        let mut out = Carries::default();
        let Some(node) = self.ix.program.node(t) else {
            return out;
        };
        let mut mentioned = false;
        if let Some(sig) = &node.sig {
            for (i, input) in sig.inputs.iter().enumerate() {
                let declared = match input {
                    syn::FnArg::Receiver(_) => self.param_ty(t, i),
                    syn::FnArg::Typed(pt) => self.ix.resolver.ty(&node.scope, &pt.ty),
                };
                if declared.carries().generics.contains(&g) {
                    mentioned = true;
                    if let Some(a) = args.get(i) {
                        out.join(&a.carries);
                    }
                }
            }
        }
        if !mentioned {
            out.types.extend(self.ix.types_for(g));
        }
        out
    }

    /// A kernel function's declared return type, with what can be learned of its generic
    /// parameters from the arguments and the receiver substituted, and the parameters left
    /// unknown.
    fn declared_return(&self, t: NodeId, args: &[Val], receiver: Option<&Ty>) -> (Ty, BTreeSet<GenId>) {
        let Some(node) = self.ix.program.node(t) else {
            return (Ty::Unknown, BTreeSet::new());
        };
        let Some(sig) = &node.sig else {
            return (Ty::Unknown, BTreeSet::new());
        };
        let r = &self.ix.resolver;
        let ret = match &sig.output {
            syn::ReturnType::Type(_, ty) => r.ty(&node.scope, ty),
            syn::ReturnType::Default => return (Ty::unit(), BTreeSet::new()),
        };
        let params: BTreeSet<GenId> = node.scope.generics.iter().copied().collect();
        let map = self.learned(t, args, receiver);
        let out = ret.subst(&map);
        // A return type naming `Self` was read in the impl's scope already; any parameter left
        // is unknown to the caller.
        let left: BTreeSet<GenId> = out
            .carries()
            .generics
            .iter()
            .copied()
            .filter(|g| params.contains(g))
            .collect();
        if left.is_empty() {
            (out, left)
        } else {
            (strip_params(&out, &params), left)
        }
    }

    /// What callee `t`'s type parameters are, as far as the receiver's type and the arguments'
    /// types show them.
    fn learned(&self, t: NodeId, args: &[Val], receiver: Option<&Ty>) -> BTreeMap<GenId, Ty> {
        let mut map = BTreeMap::new();
        let Some(node) = self.ix.program.node(t) else {
            return map;
        };
        let Some(sig) = &node.sig else {
            return map;
        };
        let params: BTreeSet<GenId> = node.scope.generics.iter().copied().collect();
        if let (Some(recv), SelfRef::Impl(imp)) = (receiver, node.scope.self_ref)
            && let Some(self_ty) = self.ix.impl_self.get(imp.0)
        {
            self_ty.unify(recv, &params, &mut map);
        }
        for (input, a) in sig.inputs.iter().zip(args) {
            if let syn::FnArg::Typed(pt) = input {
                self.ix.resolver.ty(&node.scope, &pt.ty).unify(&a.ty, &params, &mut map);
            }
        }
        map
    }

    /// The closure parameter values a callee's parameter `index` gives a closure argument, if the
    /// parameter is a closure type with stated argument types: those types, with the callee's
    /// type parameters learned from the other arguments `known` substituted, and what the ones
    /// not learned may hold ([`Flow::unlearned`]).
    fn closure_context(&self, targets: &BTreeSet<NodeId>, index: usize, known: &[Option<Val>]) -> Option<Vec<Val>> {
        for t in targets {
            let Some(node) = self.ix.program.node(*t) else {
                continue;
            };
            let Some(sig) = &node.sig else {
                continue;
            };
            let Some(syn::FnArg::Typed(pt)) = sig.inputs.iter().nth(index) else {
                continue;
            };
            let ty = self.ix.resolver.ty(&node.scope, &pt.ty);
            let traits = match ty.peel() {
                Ty::Param(g) => self.ix.resolver.bounds(*g),
                Ty::Dyn(traits) => traits.clone(),
                _ => Vec::new(),
            };
            let params: BTreeSet<GenId> = node.scope.generics.iter().copied().collect();
            let mut map = BTreeMap::new();
            for (input, a) in sig.inputs.iter().zip(known) {
                if let (syn::FnArg::Typed(pt), Some(a)) = (input, a) {
                    self.ix.resolver.ty(&node.scope, &pt.ty).unify(&a.ty, &params, &mut map);
                }
            }
            if let Some(args) = traits.iter().find_map(|tr| tr.fn_arity().map(|_| tr.args().to_vec())) {
                let given: Vec<Val> = known.iter().map(|a| a.clone().unwrap_or_else(|| Val::unknown(Carries::default()))).collect();
                return Some(
                    args.iter()
                        .map(|a| {
                            let learned = a.subst(&map);
                            let left: BTreeSet<GenId> = learned
                                .carries()
                                .generics
                                .into_iter()
                                .filter(|g| params.contains(g))
                                .collect();
                            let mut v = Val::of(strip_params(&learned, &params));
                            for g in left {
                                let held = self.unlearned(*t, g, &given);
                                v.carries.join(&held);
                            }
                            v
                        })
                        .collect(),
                );
            }
        }
        None
    }

    /// Evaluates call arguments: closures last, with the context `targets` give them (or, for
    /// an outside call, values carrying what the other inputs carry).
    fn args(&mut self, args: &[&Expr], targets: &BTreeSet<NodeId>, offset: usize, others: &Carries) -> (Vec<Val>, Vec<Place>) {
        let mut vals: Vec<Option<Val>> = vec![None; args.len()];
        let mut places = Vec::new();
        let mut known = others.clone();
        for (i, a) in args.iter().enumerate() {
            places.push(self.arg_place(a));
            if !matches!(a, Expr::Closure(_)) {
                let v = self.expr(a);
                known.join(&v.carries);
                if let Some(slot) = vals.get_mut(i) {
                    *slot = Some(v);
                }
            }
        }
        // The callee's parameters are numbered from its receiver: argument i is input i + offset.
        let mut inputs: Vec<Option<Val>> = vec![None; offset];
        inputs.extend(vals.iter().cloned());
        for (i, a) in args.iter().enumerate() {
            if let Expr::Closure(c) = a {
                let line = c.span().start().line;
                let fixed = self.closure_context(targets, i.saturating_add(offset), &inputs);
                let ctx = fixed
                    .clone()
                    .unwrap_or_else(|| vec![Val::unknown(known.clone()); c.inputs.len()]);
                let v = self.closure_fixed(c, Some(ctx), fixed.as_deref(), line);
                if let Some(slot) = vals.get_mut(i) {
                    *slot = Some(v);
                }
            }
        }
        let vals: Vec<Val> = vals.into_iter().map(|v| v.unwrap_or_else(Val::unit)).collect();
        // A closure passed by name, or held by what is passed, takes the same context as one
        // written in place.
        for (i, (a, v)) in args.iter().zip(&vals).enumerate() {
            if matches!(a, Expr::Closure(_)) {
                continue;
            }
            let closures: Vec<NodeId> = self
                .held(&v.carries)
                .into_iter()
                .filter(|c| self.g.node(*c).is_some_and(|n| n.kind == GKind::Closure))
                .collect();
            for c in closures {
                let arity = self.g.node(c).and_then(|n| n.arity).unwrap_or(0);
                let ctx: Vec<Val> = match self.closure_context(targets, i.saturating_add(offset), &inputs) {
                    Some(vals) => {
                        for (k, t) in vals.iter().enumerate() {
                            self.fix_param(c, k, t);
                        }
                        vals
                    }
                    None => (0..arity).map(|_| Val::unknown(known.clone())).collect(),
                };
                for (k, cv) in ctx.into_iter().enumerate() {
                    let cv = self.foreign_into(&cv, c);
                    self.join_param(c, k, &cv);
                }
            }
        }
        (vals, places)
    }

    /// The place an argument passed by `&mut` writes to.
    fn arg_place(&mut self, a: &Expr) -> Place {
        match a {
            Expr::Reference(r) if r.mutability.is_some() => match self.place(&r.expr) {
                Place::Local(id, _) => Place::Local(id, true),
                other => other,
            },
            _ => Place::Other,
        }
    }

    fn call(&mut self, c: &syn::ExprCall, line: usize) -> Val {
        let args: Vec<&Expr> = c.args.iter().collect();
        if let Expr::Path(p) = &*c.func
            && !(p.qself.is_none()
                && p.path.get_ident().is_some_and(|id| self.lookup_local(&id.to_string()).is_some()))
        {
            let found = self.resolve_value(p);
            if found.is_empty() {
                let shown = quote::ToTokens::to_token_stream(&p.path).to_string();
                self.problem(line, format!("a call to a path that resolves to nothing: `{shown}`"));
                let (vals, _) = self.args(&args, &BTreeSet::new(), 0, &Carries::default());
                let mut c = Carries::top();
                for v in &vals {
                    c.join(&v.carries);
                }
                return Val::unknown(c);
            }
            let mut out: Option<Val> = None;
            for r in found {
                let v = self.call_res(&r, &args, line);
                out = Some(out.map_or(v.clone(), |o| o.join(&v)));
            }
            return out.unwrap_or_else(|| Val::unknown(Carries::default()));
        }
        // A call through a value.
        let callee = self.expr(&c.func);
        let (vals, _) = self.args(&args, &BTreeSet::new(), 0, &callee.carries);
        self.call_value(&callee, &vals, line)
    }

    /// A call through a value: runs whatever it may carry, directly or through its carriers.
    fn call_value(&mut self, callee: &Val, vals: &[Val], line: usize) -> Val {
        let mut result = Val::of(Ty::Never);
        let mut any = callee.carries.top;
        for target in callee.carries.calls.clone() {
            self.how = "a call through a value";
            let kind = self.g.node(target).map(|n| n.kind);
            let scg = match kind {
                Some(GKind::Closure) => self.direct_rel(target),
                Some(GKind::Code | GKind::Derived) => self.rel(target, None, vals),
                _ => Scg::empty(),
            };
            self.edge_rel(target, line, scg);
            match kind {
                Some(GKind::Closure) => {
                    let r = self.enter_closure(target, vals);
                    result = result.join(&r);
                }
                Some(GKind::Carrier) => {
                    for r in self.reach_of(target) {
                        match self.g.node(r).map(|n| n.kind) {
                            Some(GKind::Closure) => {
                                let v = self.enter_closure(r, vals);
                                result = result.join(&v);
                            }
                            Some(GKind::Any) => any = true,
                            Some(GKind::Outside) => {
                                let v = self.call_outside_fn(vals, line);
                                result = result.join(&v);
                            }
                            Some(_) => {
                                let v = self.enter_fn(r, vals, line);
                                result = result.join(&v);
                            }
                            None => {}
                        }
                    }
                }
                Some(GKind::Any) => any = true,
                Some(GKind::Outside) => {
                    let v = self.call_outside_fn(vals, line);
                    result = result.join(&v);
                }
                Some(GKind::Code | GKind::Derived) => {
                    let v = self.enter_fn(target, vals, line);
                    result = result.join(&v);
                }
                None => {}
            }
        }
        if any {
            let anyn = self.g.any();
            self.how = "a call through a value the analysis cannot follow";
            self.edge(anyn, line);
            // Any closure may be the one called: each takes these arguments.
            let closures: Vec<NodeId> = (0..self.g.nodes.len())
                .map(NodeId)
                .filter(|c| self.g.node(*c).is_some_and(|n| n.kind == GKind::Closure))
                .collect();
            for c in closures {
                self.enter_closure(c, vals);
            }
            result.carries.top = true;
            result.ty = Ty::Unknown;
        }
        if callee.carries.calls.is_empty() {
            // Nothing the call may run is known (a callable from outside): its result is not.
            result.ty = Ty::Unknown;
        }
        // A callable of a generic or trait-object type returns its closure bound's output, in
        // this body's own types; what the closures it may be return adds only what they carry.
        if let Some(out) = self.fn_output(&callee.ty)
            && out.is_known()
        {
            result = Val::with(out, &result.carries_of_calls());
        }
        if any {
            // A callable the analysis cannot follow may hand back what it is given.
            for v in vals {
                result.carries.join(&v.carries);
            }
        }
        result
    }

    /// A call of a function from outside the kernel through a value: outside code handed `vals`,
    /// whose result may hold what they hold.
    fn call_outside_fn(&mut self, vals: &[Val], line: usize) -> Val {
        self.outside_call(vals, line);
        let mut c = Carries::default();
        for v in vals {
            c.join(&v.carries);
        }
        Val::unknown(c)
    }

    /// A call to what a path resolved to.
    fn call_res(&mut self, r: &Res, args: &[&Expr], line: usize) -> Val {
        match r {
            Res::Fn(n) => {
                let targets: BTreeSet<NodeId> = [*n].into();
                let (vals, places) = self.args(args, &targets, 0, &Carries::default());
                self.call_kernel(&[(*n, None)], &vals, &places, line, "a call by path");
                self.returns(&targets, &vals, None)
            }
            Res::Item(id) => match &self.ix.program.item(*id).kind {
                ItemKind::Struct { .. } => {
                    let (vals, _) = self.args(args, &BTreeSet::new(), 0, &Carries::default());
                    self.construct(*id, None, &vals, line)
                }
                ItemKind::Const { .. } => {
                    let callee = self.const_value(*id);
                    let (vals, _) = self.args(args, &BTreeSet::new(), 0, &callee.carries);
                    self.call_value(&callee, &vals, line)
                }
                _ => {
                    let (vals, _) = self.args(args, &BTreeSet::new(), 0, &Carries::default());
                    self.outside_call(&vals, line);
                    Val::unknown(Carries::default())
                }
            },
            Res::Variant(id, v) => {
                let (vals, _) = self.args(args, &BTreeSet::new(), 0, &Carries::default());
                self.construct(*id, Some(*v), &vals, line)
            }
            Res::SelfTy => {
                let (vals, _) = self.args(args, &BTreeSet::new(), 0, &Carries::default());
                match self.resolver().self_ty(&self.scope, 0) {
                    Ty::Adt(id, _) => self.construct(id, None, &vals, line),
                    _ => Val::unknown(Carries::default()),
                }
            }
            Res::Assoc(base, name) => {
                if let Some((id, v)) = self.variant_of(base, name) {
                    let (vals, _) = self.args(args, &BTreeSet::new(), 0, &Carries::default());
                    return self.construct(id, Some(v), &vals, line);
                }
                self.call_assoc(base, name, args, line)
            }
            Res::Ext(p) => self.call_outside_path(p, args, line),
            Res::ConstParam(_)
            | Res::Prim(_)
            | Res::Generic(_)
            | Res::Mod(_)
            | Res::Macro(_)
            | Res::Qualified(..) => {
                let (vals, _) = self.args(args, &BTreeSet::new(), 0, &Carries::default());
                self.outside_call(&vals, line);
                Val::unknown(Carries::default())
            }
        }
    }

    /// The enum variant `base::name` names, when `base` is an enum (by `Self` or an alias).
    fn variant_of(&self, base: &Res, name: &str) -> Option<(ItemId, usize)> {
        let ty = match base {
            Res::SelfTy => self.resolver().self_ty(&self.scope, 0),
            Res::Item(id) => self.resolver().item_ty(*id),
            _ => return None,
        };
        let Ty::Adt(id, _) = ty else {
            return None;
        };
        match &self.ix.program.item(id).kind {
            ItemKind::Enum { variants, .. } => variants.iter().position(|v| v.name == name).map(|v| (id, v)),
            _ => None,
        }
    }

    /// A tuple struct or tuple variant constructor: its arguments flow into its field carriers.
    fn construct(&mut self, id: ItemId, variant: Option<usize>, vals: &[Val], line: usize) -> Val {
        for (i, v) in vals.iter().enumerate() {
            if !v.carries.calls.is_empty() || v.carries.top {
                let name = i.to_string();
                let f = self.g.field(self.ix, id, variant, &name);
                let declared = self.field_ty(id, variant, &name);
                self.flow_typed(f, &v.carries, &declared, line);
            }
        }
        let args = self.ix.program.item(id).scope.generics.iter().map(|_| Ty::Unknown).collect();
        Val::of(Ty::Adt(id, args))
    }

    /// The kernel functions an associated path `base::name` may name, given the first
    /// argument's value (the receiver of a method called by path).
    fn assoc_nodes(&mut self, base: &Res, name: &str, first: Option<&Val>) -> BTreeSet<NodeId> {
        match base {
            Res::Item(id) => match &self.ix.program.item(*id).kind {
                ItemKind::Trait { .. } => {
                    let only = first.filter(|v| v.ty.is_known()).map(|v| key(&v.ty));
                    self.ix.trait_methods(&TraitRef::Kernel(*id, Vec::new()), name, only.as_ref())
                }
                _ => {
                    let ty = self.resolver().item_ty(*id);
                    self.ix.method(&Val::of(ty), name).nodes
                }
            },
            Res::Qualified(t, tr) => {
                let only = if t.is_known() { Some(key(t)) } else { None };
                match tr.as_deref() {
                    Some(Res::Item(id)) => {
                        self.ix.trait_methods(&TraitRef::Kernel(*id, Vec::new()), name, only.as_ref())
                    }
                    Some(Res::Ext(p)) => self.ix.trait_methods(
                        &TraitRef::Ext(p.last().cloned().unwrap_or_default(), Vec::new()),
                        name,
                        only.as_ref(),
                    ),
                    _ => self.ix.method(&Val::of((**t).clone()), name).nodes,
                }
            }
            Res::Generic(g) => {
                let v = Val::of(Ty::Param(*g));
                self.ix.method(&v, name).nodes
            }
            Res::SelfTy => {
                let ty = self.resolver().self_ty(&self.scope, 0);
                self.ix.method(&Val::of(ty), name).nodes
            }
            Res::Prim(p) => self.ix.method(&Val::of(Ty::Prim(p.clone())), name).nodes,
            Res::Ext(path) => {
                // An outside trait's method called by path, `Clone::clone(&x)`.
                let trait_name = path.last().cloned().unwrap_or_default();
                let only = first.filter(|v| v.ty.is_known()).map(|v| key(&v.ty));
                self.ix
                    .trait_methods(&TraitRef::Ext(trait_name, Vec::new()), name, only.as_ref())
            }
            Res::Assoc(inner, assoc) => {
                let base_ty = match &**inner {
                    Res::Generic(g) => Ty::Param(*g),
                    Res::SelfTy => self.resolver().self_ty(&self.scope, 0),
                    _ => Ty::Unknown,
                };
                let ty = self.ix.resolver.assoc_ty(base_ty, assoc, 0);
                let v = Val::of(ty);
                let mut out = self.ix.method(&v, name).nodes;
                if !v.ty.is_known() || matches!(v.ty, Ty::Assoc(..)) {
                    out.extend(self.ix.method(&Val::unknown(Carries::top()), name).nodes);
                }
                out
            }
            _ => BTreeSet::new(),
        }
    }

    /// A call of an associated function by path: `Type::f(..)`, `Trait::f(..)`, `T::f(..)`.
    fn call_assoc(&mut self, base: &Res, name: &str, args: &[&Expr], line: usize) -> Val {
        // The candidates may depend on the first argument (a receiver called by path).
        let first = args.first().map(|a| {
            if matches!(a, Expr::Closure(_)) {
                Val::unknown(Carries::default())
            } else {
                self.peek_expr_type(a)
            }
        });
        let targets = self.assoc_nodes(base, name, first.as_ref());
        let (vals, places) = self.args(args, &targets, 0, &Carries::default());
        let receiver: Option<Ty> = match base {
            Res::Generic(g) => Some(Ty::Param(*g)),
            Res::Qualified(t, _) if t.is_known() => Some((**t).clone()),
            Res::SelfTy => Some(self.resolver().self_ty(&self.scope, 0)),
            Res::Item(id) if !matches!(self.ix.program.item(*id).kind, ItemKind::Trait { .. }) => {
                Some(self.resolver().item_ty(*id))
            }
            _ => None,
        };
        let with: Vec<Target> = targets.iter().map(|t| (*t, receiver.clone())).collect();
        self.call_kernel(&with, &vals, &places, line, "a call by path");
        // Outside implementations may exist: of an outside trait, a trait object, a generic.
        let maybe_outside = match base {
            Res::Item(id) => matches!(self.ix.program.item(*id).kind, ItemKind::Trait { .. }) || targets.is_empty(),
            Res::Generic(_) | Res::Ext(_) | Res::Prim(_) | Res::Assoc(..) | Res::Qualified(..) => true,
            Res::SelfTy => targets.is_empty() || matches!(self.scope.self_ref, SelfRef::Trait(..)),
            _ => targets.is_empty(),
        };
        let mut result = self.returns(&targets, &vals, None);
        if maybe_outside {
            self.outside_call(&vals, line);
            let conv = self.conversions(name, vals.first(), None, line);
            for v in &vals {
                result.carries.join(&v.carries);
            }
            result.carries.join(&conv);
            if targets.is_empty() {
                result.ty = match base {
                    Res::Item(id) if name == "new" || name == "default" => self.resolver().item_ty(*id),
                    _ => Ty::Unknown,
                };
            }
        }
        result
    }

    /// The type of an expression without recording its calls, for choosing candidates: locals,
    /// fields, references and literals only.
    fn peek_expr_type(&mut self, e: &Expr) -> Val {
        match e {
            Expr::Lit(l) => Val::of(lit_ty(&l.lit)),
            _ => self.peek(e),
        }
    }

    /// A call to an outside path: an outside function, a prelude constructor, or an outside
    /// trait's method by path (`From::from`, `fmt::Display::fmt`).
    fn call_outside_path(&mut self, p: &[String], args: &[&Expr], line: usize) -> Val {
        let last = p.last().map(String::as_str).unwrap_or("");
        // An outside trait's method by path: its kernel implementations.
        if p.len() >= 2
            && let Some(trait_seg) = p.get(p.len().saturating_sub(2))
            && trait_seg.chars().next().is_some_and(char::is_uppercase)
        {
            let first = args.first().map(|a| self.peek_expr_type(a));
            let base = Res::Ext(p.iter().take(p.len().saturating_sub(1)).cloned().collect());
            let targets = self.assoc_nodes(&base, last, first.as_ref());
            let (vals, places) = self.args(args, &targets, 0, &Carries::default());
            let with: Vec<Target> = targets.iter().map(|t| (*t, None)).collect();
            self.call_kernel(&with, &vals, &places, line, "a call by path");
            self.outside_call(&vals, line);
            let conv = self.conversions(last, vals.first(), None, line);
            let mut result = self.returns(&targets, &vals, None);
            for v in &vals {
                result.carries.join(&v.carries);
            }
            result.carries.join(&conv);
            if matches!(last, "new" | "from") && trait_seg == "Box" {
                result.ty = Ty::Ext("Box".to_owned(), vals.first().map(|v| v.ty.clone()).into_iter().collect());
            }
            return result;
        }
        let (vals, places) = self.args(args, &BTreeSet::new(), 0, &Carries::default());
        // The prelude's variant constructors build a value and call nothing.
        let constructor = p.len() == 1 && matches!(last, "Some" | "Ok" | "Err");
        if !constructor {
            self.outside_call(&vals, line);
        }
        // Outside code may write into what it is handed by `&mut`.
        let mut all = Carries::default();
        for v in &vals {
            all.join(&v.carries);
        }
        for place in &places {
            self.write(place, &all, line);
        }
        let first_ty = vals.first().map(|v| v.ty.clone()).unwrap_or(Ty::Unknown);
        let ty = match last {
            "Some" => Ty::Ext("Option".to_owned(), vec![first_ty]),
            "Ok" => Ty::Ext("Result".to_owned(), vec![first_ty, Ty::Unknown]),
            "Err" => Ty::Ext("Result".to_owned(), vec![Ty::Unknown, first_ty]),
            _ => Ty::Unknown,
        };
        Val::with(ty, &all)
    }

    /// The kernel conversions the standard conversion `name` may call on `source`, edges added;
    /// returns what their results carry.
    fn conversions(&mut self, name: &str, source: Option<&Val>, target: Option<&Ty>, line: usize) -> Carries {
        let mut out = Carries::default();
        let Some((_, rules)) = CONVERSIONS.iter().find(|(n, _)| *n == name) else {
            return out;
        };
        for (trait_name, method) in *rules {
            let src = if matches!(*trait_name, "From" | "TryFrom") { source } else { None };
            let targets = self.ix.conversions(trait_name, method, src, target);
            for t in &targets {
                self.how = "a standard conversion";
                self.edge(*t, line);
                let (ty, _) = self.declared_return(*t, &[], None);
                if ty.admits_callables() {
                    out.calls.insert(self.g.ret(*t));
                }
                out.join(&ty.carries());
            }
        }
        out
    }

    fn method_call(&mut self, m: &syn::ExprMethodCall, line: usize) -> Val {
        let name = m.method.to_string();
        let receiver = self.expr(&m.receiver);
        let recv_place = match self.place(&m.receiver) {
            Place::Local(id, _) => Place::Local(id, true),
            other => other,
        };
        let lookup = self.ix.method(&receiver, &name);
        let how = match receiver.ty.peel() {
            Ty::Param(_) | Ty::Dyn(_) => "a method call through a bound",
            Ty::Unknown | Ty::Assoc(..) => "a method call by name (receiver type unknown)",
            _ => "a method call by the receiver's type",
        };
        let args: Vec<&Expr> = m.args.iter().collect();
        let (vals, mut places) = self.args(&args, &lookup.nodes, 1, &receiver.carries);
        let mut all = vec![receiver.clone()];
        all.extend(vals.iter().cloned());
        places.insert(0, Place::Other);
        let at = if lookup.at.is_known() { Some(lookup.at.clone()) } else { None };
        let with: Vec<Target> = lookup.nodes.iter().map(|t| (*t, at.clone())).collect();
        self.call_kernel(&with, &all, &places, line, how);
        let mut result = self.returns(&lookup.nodes, &all, Some(&receiver.ty));
        if lookup.outside {
            let mut inputs = all.clone();
            if let Some(turbofish) = &m.turbofish {
                for arg in &turbofish.args {
                    if let syn::GenericArgument::Type(t) = arg {
                        inputs.push(Val::of(self.ty(t)));
                    }
                }
            }
            self.outside_call(&inputs, line);
            let target = m.turbofish.as_ref().and_then(|t| {
                t.args.iter().find_map(|a| match a {
                    syn::GenericArgument::Type(t) => Some(self.ty(t)),
                    _ => None,
                })
            });
            let conv = self.conversions(&name, Some(&receiver), target.as_ref(), line);
            // On a receiver whose methods call the callables they are handed but never keep them
            // (`Option`, `Result`, the numbers, `bool`), a callable argument stays out of the
            // result and the receiver; only what it returns may come back.
            let keeps = !calls_and_drops_callables(&receiver.ty);
            let kept = |v: &Val| keeps || !is_callable(&v.ty);
            // An outside method on a place may store its arguments in it.
            let mut written = Carries::default();
            for v in vals.iter().filter(|v| kept(v)) {
                written.join(&v.carries);
            }
            if !written.is_empty() {
                self.write(&recv_place, &written, line);
            }
            let mut c = receiver.carries.clone();
            for v in inputs.iter().skip(1).filter(|v| kept(v)) {
                c.join(&v.carries);
            }
            c.join(&conv);
            // What the closures it is handed return may come back in its result.
            for v in &vals {
                for call in self.held(&v.carries) {
                    if self.g.node(call).is_some_and(|n| n.kind == GKind::Closure) {
                        let r = self.closure_ret(call);
                        c.join(&r.carries);
                    }
                }
            }
            let ty = outside_method_ty(&name, &receiver, &vals);
            if lookup.nodes.is_empty() {
                result = Val::with(ty, &c);
            } else {
                result = result.join(&Val::with(ty, &c));
            }
        }
        result
    }

    // ------------------------------------------------------------------ implicit calls

    /// A call of an outside trait's method on `v` (operators, formatting, derives) with `args`
    /// (`v` first, or empty when the method takes no receiver): edges to the kernel
    /// implementations it may dispatch to, each with its size-change graph; returns them.
    /// `method` `*` takes every method of the trait.
    fn trait_call(&mut self, v: &Val, trait_name: &str, method: &str, args: &[Val], line: usize) -> BTreeSet<NodeId> {
        let mut found: BTreeMap<NodeId, Vec<Ty>> = BTreeMap::new();
        let methods: Vec<&str> = if method == "*" {
            crate::recursion::model::DERIVABLE
                .iter()
                .find(|(t, _)| *t == trait_name)
                .map_or_else(Vec::new, |(_, ms)| ms.to_vec())
        } else {
            vec![method]
        };
        for m in methods {
            for (n, ats) in self.ix.dispatch(v, trait_name, m) {
                let list = found.entry(n).or_default();
                for a in ats {
                    if !list.contains(&a) {
                        list.push(a);
                    }
                }
            }
        }
        let targets: Vec<Target> = found
            .iter()
            .flat_map(|(n, ats)| {
                ats.iter()
                    .map(move |a| (*n, if a.is_known() { Some(a.clone()) } else { None }))
            })
            .collect();
        self.call_kernel(&targets, args, &[], line, IMPLICIT);
        if v.carries.top {
            self.how = IMPLICIT;
            let any = self.g.any();
            self.edge(any, line);
        }
        found.into_keys().collect()
    }

    fn binary(&mut self, b: &syn::ExprBinary, line: usize) -> Val {
        let l = self.expr(&b.left);
        let r = self.expr(&b.right);
        let Some((trait_name, method)) = binop_trait(&b.op) else {
            return Val::of(Ty::Prim("bool".to_owned()));
        };
        let args = [l.clone(), r.clone()];
        let targets = self.trait_call(&l, trait_name, method, &args, line);
        if trait_name.ends_with("Assign") {
            let place = self.place(&b.left);
            self.write(&place, &r.carries, line);
            return Val::unit();
        }
        if matches!(trait_name, "PartialEq" | "PartialOrd") {
            return Val::of(Ty::Prim("bool".to_owned()));
        }
        if l.ty.is_prim() && !matches!(l.ty.peel(), Ty::Never) {
            return Val::of(l.ty.peel().clone());
        }
        if !targets.is_empty() {
            return self.returns(&targets, &args, Some(&l.ty));
        }
        let mut c = l.carries.clone();
        c.join(&r.carries);
        Val::unknown(c)
    }

    fn unary(&mut self, u: &syn::ExprUnary, line: usize) -> Val {
        let v = self.expr(&u.expr);
        match u.op {
            UnOp::Deref(_) => {
                if let Ty::Ref(inner) = &v.ty {
                    return Val::with((**inner).clone(), &v.carries_of_calls());
                }
                if let Ty::Adt(..) = v.ty.peel() {
                    self.trait_call(&v, "Deref", "deref", &[v.clone()], line);
                    if let Some(t) = self.deref_target(v.ty.peel()) {
                        return Val::with(t, &v.carries_of_calls());
                    }
                    return Val::unknown(v.carries);
                }
                if !v.ty.is_known() {
                    self.trait_call(&v, "Deref", "deref", &[v.clone()], line);
                }
                Val::unknown(v.carries)
            }
            UnOp::Neg(_) | UnOp::Not(_) => {
                let (t, m) = if matches!(u.op, UnOp::Neg(_)) { ("Neg", "neg") } else { ("Not", "not") };
                if v.ty.is_prim() {
                    return v;
                }
                let targets = self.trait_call(&v, t, m, &[v.clone()], line);
                if targets.is_empty() {
                    v
                } else {
                    self.returns(&targets, &[v.clone()], Some(&v.ty))
                }
            }
            _ => {
                self.problem(line, "a unary operator the recursion check does not know".to_owned());
                Val::unknown(Carries::top())
            }
        }
    }

    fn index(&mut self, i: &syn::ExprIndex, line: usize) -> Val {
        let base = self.expr(&i.expr);
        let idx = self.expr(&i.index);
        if matches!(base.ty.peel(), Ty::Adt(..)) || !base.ty.is_known() {
            let args = [base.clone(), idx.clone()];
            let mut targets = self.trait_call(&base, "Index", "index", &args, line);
            targets.extend(self.trait_call(&base, "IndexMut", "index_mut", &args, line));
            if !targets.is_empty() {
                let r = self.returns(&targets, &args, Some(&base.ty));
                return Val {
                    ty: r.ty.peel().clone(),
                    carries: r.carries,
                };
            }
        }
        if matches!(idx.ty.peel(), Ty::Ext(n, _) if n.starts_with("Range")) {
            return Val::unknown(base.carries);
        }
        elem_of(&base)
    }

    fn for_loop(&mut self, f: &syn::ExprForLoop, line: usize) -> Val {
        let it = self.expr(&f.expr);
        let mut item = for_item(self.ix, &it);
        let into = self.ix.method(&it, "into_iter");
        let next = self.ix.method(&it, "next");
        let mut targets = into.nodes.clone();
        targets.extend(next.nodes.iter().copied());
        let mut with: Vec<Target> = Vec::new();
        for (l, ts) in [(&into, &into.nodes), (&next, &next.nodes)] {
            let at = if l.at.is_known() { Some(l.at.clone()) } else { None };
            with.extend(ts.iter().map(|t| (*t, at.clone())));
        }
        self.call_kernel(&with, &[it.clone()], &[], line, "a for loop");
        for t in &targets {
            item.carries.calls.insert(self.g.ret(*t));
        }
        if into.outside || next.outside {
            self.outside_call(&[it.clone()], line);
        }
        self.push_scope();
        self.pattern(&f.pat, &item);
        self.loops.push(Val::of(Ty::Never));
        self.block(&f.body);
        self.loops.pop();
        self.pop_scope();
        Val::unit()
    }

    fn try_op(&mut self, t: &syn::ExprTry, line: usize) -> Val {
        let v = self.expr(&t.expr);
        // The error converted into the function's error type by `From::from`.
        let (source, target) = match (v.ty.peel(), self.ret_ty.peel()) {
            (Ty::Ext(n, args), ret) if n == "Result" => (
                args.get(1).cloned().map(Val::of),
                match ret {
                    Ty::Ext(r, rargs) if r == "Result" => rargs.get(1).cloned(),
                    _ => None,
                },
            ),
            (Ty::Ext(n, _), _) if n == "Option" => (None, None),
            (_, Ty::Ext(r, rargs)) if r == "Result" => (None, rargs.get(1).cloned()),
            _ => (None, None),
        };
        let skip = matches!(v.ty.peel(), Ty::Ext(n, _) if n == "Option");
        if !skip {
            let targets = self.ix.conversions("From", "from", source.as_ref(), target.as_ref());
            self.how = "`?` converting an error";
            self.edges(&targets, line);
        }
        match v.ty.peel() {
            Ty::Ext(n, args) if n == "Result" || n == "Option" => args
                .first()
                .cloned()
                .filter(Ty::is_known)
                .map_or_else(|| Val::unknown(v.carries.clone()), |t| Val::with(t, &v.carries_of_calls())),
            _ => Val::unknown(v.carries.clone()),
        }
    }

    fn struct_lit(&mut self, s: &syn::ExprStruct, line: usize) -> Val {
        let segs = Resolver::segments(&s.path);
        let mut found = self.resolver().resolve(&self.scope, &segs, s.path.leading_colon.is_some(), Ns::Type);
        if found.is_empty() {
            found = self.resolver().resolve(&self.scope, &segs, s.path.leading_colon.is_some(), Ns::Value);
        }
        let (item, variant) = match found.first() {
            Some(Res::Item(id)) => (Some(*id), None),
            Some(Res::Variant(id, v)) => (Some(*id), Some(*v)),
            Some(Res::SelfTy) => match self.resolver().self_ty(&self.scope, 0) {
                Ty::Adt(id, _) => (Some(id), None),
                _ => (None, None),
            },
            Some(Res::Assoc(base, v)) => match &**base {
                Res::SelfTy => match self.resolver().self_ty(&self.scope, 0) {
                    Ty::Adt(id, _) => {
                        let var = match &self.ix.program.item(id).kind {
                            ItemKind::Enum { variants, .. } => variants.iter().position(|x| x.name == *v),
                            _ => None,
                        };
                        (Some(id), var)
                    }
                    _ => (None, None),
                },
                _ => (None, None),
            },
            _ => (None, None),
        };
        let mut all = Carries::default();
        for f in &s.fields {
            let v = self.expr(&f.expr);
            all.join(&v.carries);
            if let Some(id) = item
                && (!v.carries.calls.is_empty() || v.carries.top)
            {
                let name = member_name(&f.member);
                let node = self.g.field(self.ix, id, variant, &name);
                let declared = self.field_ty(id, variant, &name);
                self.flow_typed(node, &v.carries, &declared, line);
            }
        }
        if let Some(rest) = &s.rest {
            self.expr(rest);
        }
        match item {
            Some(id) => {
                let args = self.ix.program.item(id).scope.generics.iter().map(|_| Ty::Unknown).collect();
                Val::of(Ty::Adt(id, args))
            }
            None => {
                if found.is_empty() {
                    let shown = quote::ToTokens::to_token_stream(&s.path).to_string();
                    self.problem(line, format!("a struct path that resolves to nothing: `{shown}`"));
                }
                Val::unknown(all)
            }
        }
    }

    /// A closure: its own node, its body read with its parameters bound to the program's value of
    /// them, which `ctx` (the values its caller gives it, when known) joins.
    fn closure(&mut self, c: &syn::ExprClosure, ctx: Option<Vec<Val>>, line: usize) -> Val {
        self.closure_fixed(c, ctx, None, line)
    }

    /// [`Flow::closure`], with the parameter types its context fixes, when it does.
    fn closure_fixed(&mut self, c: &syn::ExprClosure, ctx: Option<Vec<Val>>, fixed: Option<&[Val]>, line: usize) -> Val {
        let parent = self.current;
        let index = self.next_closure;
        self.next_closure = self.next_closure.saturating_add(1);
        let id = self.g.closure(parent, index, c.inputs.len(), &self.file, line);
        self.shared.made_in.insert(id, self.body_id);
        if let Some(tys) = fixed {
            for (i, t) in tys.iter().enumerate().take(c.inputs.len()) {
                self.fix_param(id, i, t);
            }
        }
        if let Some(ctx) = &ctx {
            for (i, v) in ctx.iter().enumerate().take(c.inputs.len()) {
                self.join_param(id, i, v);
            }
        }
        let declared = match &c.output {
            syn::ReturnType::Type(_, t) => self.resolver().ty(&self.scope, t),
            syn::ReturnType::Default => Ty::Unknown,
        };
        let saved_ret = core::mem::replace(&mut self.ret_ty, declared.clone());
        let saved_dropped = core::mem::take(&mut self.dropped);
        self.current = id;
        self.push_scope();
        for (i, p) in c.inputs.iter().enumerate() {
            let v = self
                .shared
                .closure_params
                .get(&(id, i))
                .cloned()
                .unwrap_or_else(|| Val::of(Ty::Never));
            self.pattern(p, &v);
        }
        let body = self.expr(&c.body);
        let result = if declared.is_known() {
            Val::with(declared.clone(), &body.carries_of_calls())
        } else {
            body.clone()
        };
        self.closure_result(&result);
        let ret = self.g.ret(id);
        self.flow_typed(ret, &body.carries, &declared, line);
        self.drop_glue(line);
        self.pop_scope();
        self.current = parent;
        self.ret_ty = saved_ret;
        self.dropped = saved_dropped;
        let mut carries = Carries::default();
        carries.calls.insert(id);
        Val::with(Ty::Closure(id), &carries)
    }

    /// The output type of calling a value of type `ty`: its closure bound's, for a generic
    /// parameter or trait object.
    fn fn_output(&self, ty: &Ty) -> Option<Ty> {
        let traits = match ty.peel() {
            Ty::Param(g) => self.ix.resolver.bounds(*g),
            Ty::Dyn(traits) => traits.clone(),
            _ => return None,
        };
        traits.iter().find_map(|tr| tr.fn_output().cloned())
    }

    /// Joins `v` into the result of the closure being read.
    fn closure_result(&mut self, v: &Val) {
        let id = self.current;
        let slot = self.shared.closure_rets.entry(id).or_insert_with(|| Val::of(Ty::Never));
        let joined = slot.join(v);
        if joined != *slot {
            *slot = joined;
            self.changed = true;
            let readers: Vec<Body> = self
                .shared
                .ret_readers
                .get(&id)
                .into_iter()
                .flatten()
                .copied()
                .filter(|b| *b != self.body_id)
                .collect();
            self.shared.dirty.extend(readers);
        }
    }

    /// Edges to the kernel `Drop` implementations the body's values may run when dropped.
    fn drop_glue(&mut self, line: usize) {
        let targets = self.ix.drops(&self.dropped);
        if !targets.is_empty() {
            self.how = "drop glue";
            self.edges(&targets, line);
        }
    }

    // ------------------------------------------------------------------ macros

    fn mac(&mut self, mac: &syn::Macro, statement: bool) -> Val {
        let line = mac.path.span().start().line;
        let read = macros::read(&self.ix.resolver, &self.scope, mac, self.depth);
        match read {
            MacroRead::Nothing => Val::unknown(Carries::default()),
            MacroRead::Exprs(exprs) => {
                let mut last = Val::unit();
                for e in &exprs {
                    last = self.expr(e);
                }
                if exprs.len() == 1 { last } else { Val::unknown(Carries::default()) }
            }
            MacroRead::Format { plain, formatted, destination, compared } => {
                let mut values = Vec::new();
                for e in &plain {
                    values.push(self.expr(e));
                }
                if let Some((a, b)) = &compared {
                    let av = self.expr(a);
                    let bv = self.expr(b);
                    self.trait_call(&av, "PartialEq", "*", &[av.clone(), bv], line);
                }
                for f in &formatted {
                    let (v, t) = match f {
                        Formatted::Expr(e, t) => (self.expr(e), *t),
                        Formatted::Captured(name, t) => {
                            let v = if let Some(b) = self.lookup_local(name) {
                                self.local(b)
                            } else {
                                match syn::parse_str::<syn::ExprPath>(name) {
                                    Ok(p) => self.path_value(&p, line),
                                    Err(_) => Val::unknown(Carries::default()),
                                }
                            };
                            (v, *t)
                        }
                    };
                    if t != FmtTrait::Count {
                        self.trait_call(&v, t.name(), "fmt", &[v.clone()], line);
                    }
                    values.push(v);
                }
                if let Some(d) = &destination {
                    let dv = self.expr(d);
                    // `write!` calls `write_fmt` on its destination: a kernel `fmt::Write`
                    // implementation's `write_str` runs.
                    let lookup = self.ix.method(&dv, "write_fmt");
                    let at = if lookup.at.is_known() { Some(lookup.at.clone()) } else { None };
                    let with: Vec<Target> = lookup.nodes.iter().map(|t| (*t, at.clone())).collect();
                    self.call_kernel(&with, &[dv.clone()], &[], line, IMPLICIT);
                    self.trait_call(&dv, "Write", "write_str", &[dv.clone()], line);
                    if lookup.outside {
                        self.outside_call(&[dv], line);
                    }
                }
                Val::unknown(Carries::default())
            }
            MacroRead::Repeat(x, n) => {
                let v = self.expr(&x);
                self.expr(&n);
                self.trait_call(&v, "Clone", "clone", &[v.clone()], line);
                Val::with(Ty::Ext("Vec".to_owned(), vec![v.ty.clone()]), &v.carries)
            }
            MacroRead::Expanded(stmts) => {
                self.depth = self.depth.saturating_add(1);
                let v = if statement {
                    self.stmts(&stmts)
                } else {
                    self.push_scope();
                    let v = self.stmts(&stmts);
                    self.pop_scope();
                    v
                };
                self.depth = self.depth.saturating_sub(1);
                if stmts.iter().any(|s| matches!(s, Stmt::Item(_))) {
                    self.problem(line, "a macro that expands to an item inside a function body, which the recursion check does not read".to_owned());
                }
                v
            }
            MacroRead::Unread(why) => {
                self.problem(line, why);
                Val::unknown(Carries::top())
            }
        }
    }
}

impl Val {
    /// What the value carries beyond its type: its callables, generics and `top`, kept when the
    /// type is narrowed to a part.
    fn carries_of_calls(&self) -> Carries {
        Carries {
            types: std::collections::BTreeSet::new(),
            calls: self.carries.calls.clone(),
            generics: BTreeSet::new(),
            top: self.carries.top,
        }
    }
}

/// A field's name: its identifier, or its index.
fn member_name(m: &syn::Member) -> String {
    match m {
        syn::Member::Named(id) => id.to_string(),
        syn::Member::Unnamed(i) => i.index.to_string(),
    }
}

/// A literal's type: unsuffixed numbers are `int` and `float`, which match every integer or
/// float primitive.
fn lit_ty(l: &syn::Lit) -> Ty {
    match l {
        syn::Lit::Str(_) => Ty::Ref(Box::new(Ty::Prim("str".to_owned()))),
        syn::Lit::ByteStr(_) => Ty::Ref(Box::new(Ty::Array(Box::new(Ty::Prim("u8".to_owned()))))),
        syn::Lit::CStr(_) => Ty::Ref(Box::new(Ty::Ext("CStr".to_owned(), Vec::new()))),
        syn::Lit::Byte(_) => Ty::Prim("u8".to_owned()),
        syn::Lit::Char(_) => Ty::Prim("char".to_owned()),
        syn::Lit::Bool(_) => Ty::Prim("bool".to_owned()),
        syn::Lit::Int(i) => Ty::Prim(if i.suffix().is_empty() { "int".to_owned() } else { i.suffix().to_owned() }),
        syn::Lit::Float(f) => Ty::Prim(if f.suffix().is_empty() { "float".to_owned() } else { f.suffix().to_owned() }),
        _ => Ty::Unknown,
    }
}

/// The element of an indexable or iterable value: of a slice, array, `Vec` and the like.
fn elem_of(v: &Val) -> Val {
    let calls = v.carries_of_calls();
    match v.ty.peel() {
        Ty::Slice(t) | Ty::Array(t) => Val::with((**t).clone(), &calls),
        Ty::Ext(n, args) if matches!(n.as_str(), "Vec" | "VecDeque" | "Box" | "BinaryHeap") => args
            .first()
            .cloned()
            .map_or_else(|| Val::unknown(v.carries.clone()), |t| Val::with(t, &calls)),
        _ => Val::unknown(v.carries.clone()),
    }
}

/// What a `for` loop over `it` binds each time round.
fn for_item(ix: &Index<'_>, it: &Val) -> Val {
    let calls = it.carries_of_calls();
    let by_ref = matches!(it.ty, Ty::Ref(_));
    let wrap = |t: Ty| if by_ref { Ty::Ref(Box::new(t)) } else { t };
    match it.ty.peel() {
        Ty::Slice(t) | Ty::Array(t) => Val::with(wrap((**t).clone()), &calls),
        Ty::Ext(n, args) if matches!(n.as_str(), "Vec" | "VecDeque" | "Option" | "BTreeSet" | "BinaryHeap") || n.starts_with("Range") => args
            .first()
            .cloned()
            .filter(Ty::is_known)
            .map_or_else(|| Val::unknown(it.carries.clone()), |t| Val::with(wrap(t), &calls)),
        Ty::Adt(..) => {
            let item = ix.resolver.assoc_ty(it.ty.peel().clone(), "Item", 0);
            if matches!(item, Ty::Assoc(..)) {
                Val::unknown(it.carries.clone())
            } else {
                Val::with(item, &calls)
            }
        }
        _ => Val::unknown(it.carries.clone()),
    }
}

/// Whether a value of type `ty` is itself a callable: a closure, a function item or pointer.
fn is_callable(ty: &Ty) -> bool {
    matches!(ty.peel(), Ty::Closure(_) | Ty::FnDef(_) | Ty::FnPtr(_))
}

/// Whether the standard library's methods on a receiver of type `ty` call the callables they are
/// handed without keeping them: on `Option` and `Result`, the integers, the floats and `bool`, no
/// method's result type or receiver holds a type parameter of a closure argument, as their
/// documented signatures state (<https://doc.rust-lang.org/std/option/enum.Option.html>,
/// <https://doc.rust-lang.org/std/result/enum.Result.html>,
/// <https://doc.rust-lang.org/std/primitive.bool.html>). A `str` or slice method may keep one in
/// the iterator it returns (`split` with a closure pattern), so those are not listed.
fn calls_and_drops_callables(ty: &Ty) -> bool {
    match ty.peel() {
        Ty::Ext(n, _) => matches!(n.as_str(), "Option" | "Result"),
        Ty::Prim(p) => p != "str" && p != "char",
        _ => false,
    }
}

/// `Option`'s methods whose result is an `Option` of the same element (by value, by reference, or
/// copied).
const OPTION_SAME: [&str; 10] = ["filter", "inspect", "or", "or_else", "replace", "take", "take_if", "xor", "cloned", "copied"];

/// `Option`'s and `Result`'s methods whose result is an `Option` or `Result` of another element.
const SAME_FAMILY: [&str; 7] = ["and", "and_then", "map", "flatten", "as_deref", "as_deref_mut", "map_err"];

/// The type an outside method returns, where the standard library states it plainly: the clone
/// and unwrapping families, counts and tests; on `Option` and `Result`, the methods that keep the
/// family (`zip`, `map`, `ok`, ...); on the numbers, the checked and overflowing families and the
/// rest, which return their own type. Anything else is unknown.
fn outside_method_ty(name: &str, receiver: &Val, args: &[Val]) -> Ty {
    let recv = receiver.ty.peel();
    let ext = |n: &str, args: Vec<Ty>| Ty::Ext(n.to_owned(), args);
    let first = |t: &Ty| match t {
        Ty::Ext(_, a) => a.first().cloned().unwrap_or(Ty::Unknown),
        _ => Ty::Unknown,
    };
    match (recv, name) {
        (_, "clone" | "to_owned") if recv.is_known() => return recv.clone(),
        (Ty::Ext(n, a), "unwrap" | "expect" | "unwrap_or" | "unwrap_or_else" | "unwrap_or_default" | "unwrap_unchecked")
            if n == "Option" || n == "Result" =>
        {
            return a.first().cloned().unwrap_or(Ty::Unknown);
        }
        (_, "len" | "count") => return Ty::Prim("usize".to_owned()),
        (_, "is_empty" | "is_some" | "is_none" | "is_ok" | "is_err" | "contains" | "starts_with" | "ends_with") => {
            return Ty::Prim("bool".to_owned());
        }
        _ => {}
    }
    match recv {
        Ty::Ext(n, a) if n == "Option" => {
            let elem = a.first().cloned().unwrap_or(Ty::Unknown);
            if OPTION_SAME.contains(&name) {
                let elem = if matches!(name, "cloned" | "copied") { elem.peel().clone() } else { elem };
                return ext("Option", vec![elem]);
            }
            match name {
                "as_ref" | "as_mut" => return ext("Option", vec![Ty::Ref(Box::new(elem))]),
                "zip" => {
                    let other = args.first().map_or(Ty::Unknown, |v| first(v.ty.peel()));
                    return ext("Option", vec![Ty::Tuple(vec![elem, other])]);
                }
                "ok_or" | "ok_or_else" => return ext("Result", vec![elem, Ty::Unknown]),
                _ if SAME_FAMILY.contains(&name) => return ext("Option", vec![Ty::Unknown]),
                _ => {}
            }
        }
        Ty::Ext(n, a) if n == "Result" => {
            let ok = a.first().cloned().unwrap_or(Ty::Unknown);
            let err = a.get(1).cloned().unwrap_or(Ty::Unknown);
            match name {
                "ok" => return ext("Option", vec![ok]),
                "err" => return ext("Option", vec![err]),
                "as_ref" | "as_mut" => return ext("Result", vec![Ty::Ref(Box::new(ok)), Ty::Ref(Box::new(err))]),
                "map_err" | "or" | "or_else" | "inspect_err" => return ext("Result", vec![ok, Ty::Unknown]),
                "map" | "and" | "and_then" => return ext("Result", vec![Ty::Unknown, err]),
                "inspect" | "cloned" | "copied" => return ext("Result", vec![ok.peel().clone(), err]),
                _ if SAME_FAMILY.contains(&name) => return ext("Result", vec![Ty::Unknown, Ty::Unknown]),
                _ => {}
            }
        }
        Ty::Prim(p) if p != "str" && p != "char" && p != "bool" => {
            let own = Ty::Prim(p.clone());
            if name.starts_with("checked_") {
                return ext("Option", vec![own]);
            }
            if name.starts_with("overflowing_") {
                return Ty::Tuple(vec![own, Ty::Prim("bool".to_owned())]);
            }
            if name.starts_with("wrapping_")
                || name.starts_with("saturating_")
                || matches!(name, "abs" | "pow" | "min" | "max" | "clamp" | "signum" | "rem_euclid" | "div_euclid")
            {
                return own;
            }
        }
        _ => {}
    }
    Ty::Unknown
}

/// `ty` with the generic parameters in `params` (the callee's) made unknown, so they do not leak
/// into the caller's world.
fn strip_params(ty: &Ty, params: &BTreeSet<GenId>) -> Ty {
    let map: BTreeMap<GenId, Ty> = params.iter().map(|g| (*g, Ty::Unknown)).collect();
    ty.subst(&map)
}
