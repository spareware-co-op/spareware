//! The type checker: every call one instance's body makes, as edges of the call graph.
//!
//! What it does: [`body`] checks the body of one instance ([`crate::recursion::mono`]) as rustc's
//! type checker does, in one pass over the source with inference variables ([`Infer`]): every
//! expression gets a type, from the signatures and field types the source writes (the standard
//! library's model among them, [`crate::recursion::stdlib`]), literals, and what unification
//! learns; an expected type flows down into each expression (a `let` annotation, a parameter's
//! type, a closure bound's parameters). Every call is recorded with what it calls:
//!
//! - **Calls by path** (`f`, `Type::f`, `Trait::f`, `T::f`, `<T as Trait>::f`), with the generic
//!   arguments inference fixes.
//! - **Method calls**, by method lookup on the receiver's type ([`Index::probe`]).
//! - **Implicit calls**: operators and compound assignments (`Add::add`, `PartialEq::eq`, ...) on
//!   types that are not primitives, unary `-` and `!`, `*` through `Deref`, indexing, `?` (the
//!   conversion `From::from` into the function's error type), `for` loops (`into_iter`, `next`),
//!   the formatting traits of formatting macros ([`crate::recursion::macros`]), and drop glue.
//! - **Calls through values**: a closure or function item called as `f(x)`; a closure trait
//!   object or function pointer, which may run every closure or function item coerced to one
//!   anywhere ([`Mono::coerce`]).
//!
//! What inference cannot settle at once (a method on a receiver whose type a later line fixes, a
//! projection on a type not known yet, a conversion whose target the result gives) waits as an
//! obligation and is settled when its type is known; at the end, unsuffixed literals get their
//! defaults. Then every recorded call becomes an edge to the instance its resolved types name. A
//! closure is a type of its own; its body is checked where it is written, with its parameters
//! from its annotations or from the bound it is passed to, and its calls are its own instance's
//! edges. [`derived`] checks a `#[derive]`d implementation: the same trait's method on each field.
//! [`constants`] reads constant and static initialisers (which run at compile time) only for the
//! closures and functions they coerce to function pointers.
//!
//! Guarantees:
//!
//! - **Every call is an edge, or the code is reported.** A call whose target cannot be known (a
//!   receiver whose type cannot be inferred, a path that resolves to nothing, a method no type
//!   has, a syntax form the checker does not read) is a [`Problem`]; the check fails on any.
//! - **Precise where the program is.** A call reaches exactly what the compiler would call for
//!   the instance's types, through the standard library's model, apart from what
//!   [`Index::probe`] and [`crate::recursion::mono`] state for generic receivers and trait
//!   objects.
//! - **Deterministic.** One pass in source order (closure arguments after the others, as rustc
//!   checks them), obligations in the order they were made.
//!
//! Assumptions: the program compiles (the checker does not look for type errors, and where a
//! coercion it does not model meets a type it expected, it keeps the expected type); the kernel's
//! naming lints hold (a pattern identifier that starts with an upper-case letter names a constant
//! or unit item, one that starts with a lower-case letter binds), which the merge gate's `-D
//! warnings` enforces.
//!
//! Method: Hindley-Milner inference with bidirectional checking as rustc's typeck performs it
//! (<https://rustc-dev-guide.rust-lang.org/type-inference.html>,
//! <https://rustc-dev-guide.rust-lang.org/ty-module/instantiating-binders.html>), with deferred
//! obligations (<https://rustc-dev-guide.rust-lang.org/traits/resolution.html>) and default
//! binding modes for patterns (RFC 2005,
//! <https://rust-lang.github.io/rfcs/2005-match-ergonomics.html>); expression, pattern and
//! statement semantics from The Rust Reference
//! (<https://doc.rust-lang.org/reference/expressions.html>,
//! <https://doc.rust-lang.org/reference/patterns.html>), its operator traits, `?`, `for` loops,
//! coercions (<https://doc.rust-lang.org/reference/type-coercions.html>) and drop scopes
//! (<https://doc.rust-lang.org/reference/destructors.html>).
//!
//! Cost: one pass over each instance's body, each expression's lookups as [`Index`] costs them,
//! and at most [`MAX_SETTLE`] rounds over the obligations. The checker recurses over the syntax
//! tree, so its depth is the nesting depth of the source's expressions;
//! [`crate::recursion::analyse`] runs it on a thread with a large stack.

use std::collections::{BTreeMap, BTreeSet};

use quote::ToTokens;
use syn::{BinOp, Expr, Pat, Stmt, UnOp};

use crate::recursion::Problem;
use crate::recursion::index::{ClosureSigs, Index, Map, Pick, how};
use crate::recursion::infer::{Infer, VarKind};
use crate::recursion::macros::{self, FmtTrait, Formatted, MacroRead};
use crate::recursion::model::{ImplId, ItemKind, NodeKind, Scope, SelfRef, line_of};
use crate::recursion::mono::{Dep, InstId, Mono, Site};
use crate::recursion::resolve::{Ns, Res, Resolver};
use crate::recursion::ty::{ClosureKey, ItemId, NodeId, Prim, TraitRef, Ty};

/// How many rounds over the waiting obligations one check makes before giving up on them.
pub const MAX_SETTLE: usize = 64;

/// Whose call a call is: the instance's own, or one of its closures' (by place).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Origin {
    Owner,
    Closure(usize),
}

/// What a call reaches, with types that may still hold variables.
#[derive(Clone, Debug)]
enum Target {
    /// A function with its generic arguments.
    Node(NodeId, Vec<Ty>),
    /// A closure.
    Closure(ClosureKey),
    /// A trait method on a generic receiver: the trait's default body, if any.
    Bound { trait_ref: TraitRef, name: String, self_ty: Ty, method_args: Vec<Ty> },
    /// A trait method on a trait object: every implementation of the trait.
    Dyn { trait_id: ItemId, name: String },
    /// A call through a closure trait object (`dynamic`) or function pointer of this arity.
    Erased { dynamic: bool, arity: usize },
}

/// One call, with where it is written and how it was found.
#[derive(Clone, Debug)]
struct Call {
    origin: Origin,
    target: Target,
    line: usize,
    how: &'static str,
}

/// One closure of the body.
#[derive(Clone, Debug)]
struct ClosureInfo {
    key: ClosureKey,
    params: Vec<Ty>,
    ret: Ty,
}

/// A pattern's default binding mode (RFC 2005): by value, or by shared or mutable reference
/// once the pattern matched through a reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Move,
    Ref,
    RefMut,
}

impl Mode {
    /// The mode after matching through a reference (mutable when `mutable`).
    fn through(self, mutable: bool) -> Mode {
        match (self, mutable) {
            (Mode::Move | Mode::RefMut, true) => Mode::RefMut,
            _ => Mode::Ref,
        }
    }

    /// The type a binding of a value of type `t` gets in this mode.
    fn bind(self, t: Ty) -> Ty {
        match self {
            Mode::Move => t,
            Mode::Ref => Ty::reference(t),
            Mode::RefMut => Ty::reference_mut(t),
        }
    }
}

/// What a call's arguments are: still to check, or checked.
#[derive(Clone, Copy)]
enum Args<'e> {
    Exprs(&'e [Expr]),
    Tys(&'e [Ty]),
}

/// An obligation that waits for a type to be known.
#[derive(Clone, Debug)]
enum Pending {
    Method { origin: Origin, recv: Ty, name: String, args: Vec<Ty>, turbofish: Vec<Ty>, out: Ty, line: usize },
    TraitFn {
        origin: Origin,
        trait_id: ItemId,
        name: String,
        self_ty: Ty,
        trait_args: Vec<Ty>,
        method_args: Vec<Ty>,
        line: usize,
        how: &'static str,
    },
    Field { base: Ty, name: String, out: Ty, line: usize },
    Index { origin: Origin, base: Ty, idx: Ty, out: Ty, line: usize },
    Binary { origin: Origin, lhs: Ty, rhs: Ty, op: (&'static str, &'static str), out: Ty, line: usize },
    Unary { origin: Origin, operand: Ty, op: (&'static str, &'static str), out: Ty, line: usize },
    Deref { origin: Origin, operand: Ty, out: Ty, line: usize },
    CallValue { origin: Origin, callee: Ty, args: Vec<Ty>, out: Ty, line: usize },
    Projection { assoc: Ty, out: Ty },
    /// A bound's associated-type binding on a generic argument (`I: IntoIterator<Item = T>`):
    /// once `ty` is known, its projection meets `value`.
    Binding { ty: Ty, trait_ref: TraitRef, name: String, value: Ty },
    Opaque { node: NodeId, args: Vec<Ty>, declared: Ty, line: usize },
    Iterate { origin: Origin, iterable: Ty, item: Ty, line: usize },
    Try { origin: Origin, operand: Ty, out: Ty, line: usize },
    FnBound { callee: Ty, inputs: Vec<Ty>, output: Ty },
}

/// The checker of one instance's body.
struct Checker<'a, 'p> {
    ix: &'a Index<'p>,
    m: &'a mut Mono,
    owner: InstId,
    node: NodeId,
    args: Vec<Ty>,
    map: Map,
    scope: Scope,
    file: String,
    infer: Infer,
    origin: Origin,
    scopes: Vec<BTreeMap<String, Ty>>,
    locals: Vec<Ty>,
    closures: Vec<ClosureInfo>,
    rets: Vec<Ty>,
    loops: Vec<(Option<String>, Ty)>,
    calls: Vec<Call>,
    pending: Vec<Pending>,
    problems: Vec<Problem>,
    reported: BTreeSet<(usize, String)>,
    coerced: Vec<(Ty, bool)>,
    depth: usize,
    compile_time: bool,
    closure_expect: Option<(Vec<Ty>, Ty)>,
    or_binding: bool,
    /// Whether the last round is running: generic arguments left open are taken as unknown.
    lenient: bool,
}

/// A candidate of a call made ready: its parameters, result, the call to record, and the
/// obligations its bounds make.
#[derive(Clone, Debug)]
struct Prepared {
    params: Vec<Ty>,
    output: Ty,
    target: Target,
    obligations: Vec<Pending>,
}

/// What instantiating a function's signature gave.
struct Instance {
    args: Vec<Ty>,
    inputs: Vec<Ty>,
    output: Ty,
}

/// The single-segment name of a path, if it is one.
fn ident_of(path: &syn::Path) -> Option<String> {
    if path.leading_colon.is_none() && path.segments.len() == 1 {
        path.segments.first().map(|s| s.ident.to_string())
    } else {
        None
    }
}

/// Whether an identifier starts with an upper-case letter.
fn upper(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_uppercase)
}

/// Whether an expression is a closure, through parentheses.
fn is_closure(e: &Expr) -> bool {
    match e {
        Expr::Closure(_) => true,
        Expr::Paren(p) => is_closure(&p.expr),
        Expr::Group(g) => is_closure(&g.expr),
        _ => false,
    }
}

/// The trait and method a binary operator calls.
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
        BinOp::Eq(_) => ("PartialEq", "eq"),
        BinOp::Ne(_) => ("PartialEq", "ne"),
        BinOp::Lt(_) => ("PartialOrd", "lt"),
        BinOp::Le(_) => ("PartialOrd", "le"),
        BinOp::Gt(_) => ("PartialOrd", "gt"),
        BinOp::Ge(_) => ("PartialOrd", "ge"),
        _ => return None,
    })
}

/// Checks instance `inst`'s body: node `node` with generic arguments `args`.
pub fn body(ix: &Index<'_>, m: &mut Mono, inst: InstId, node: NodeId, args: &[Ty]) {
    let Some(n) = ix.program.node(node) else {
        return;
    };
    let Some(block) = n.body.clone() else {
        return;
    };
    let syn_sig = n.sig.clone();
    let mut c = Checker::new(ix, m, inst, node, args, n.scope.clone(), &n.file);
    let sig = ix.sig(node).cloned().unwrap_or_default();
    // The body's own result: an `impl Trait` in it is the body's hidden type, a variable here.
    let declared = sig.output.subst(&c.map).fold(&mut |t| match t {
        Ty::Opaque(_) => Some(Ty::Infer),
        _ => None,
    });
    let declared = c.inst_ty(&declared);
    if let Some(s) = &syn_sig {
        for (i, input) in s.inputs.iter().enumerate() {
            let t = sig.inputs.get(i).map_or(Ty::Unknown, |t| t.subst(&c.map));
            let t = c.inst_ty(&t);
            match input {
                syn::FnArg::Receiver(_) => c.bind("self", t),
                syn::FnArg::Typed(pt) => c.bind_pat(&pt.pat, &t, Mode::Move),
            }
        }
    }
    c.rets.push(declared.clone());
    let tail = c.block(&block, Some(&declared));
    c.coerce(&tail, &declared, n.line);
    c.rets.pop();
    let has_opaque = sig.output.any(&|t| matches!(t, Ty::Opaque(_)));
    c.finish(if has_opaque { Some(declared) } else { None }, n.line);
}

/// Checks a derived implementation's instance: the same trait's method on every field's type.
pub fn derived(ix: &Index<'_>, m: &mut Mono, inst: InstId, node: NodeId, args: &[Ty]) {
    let Some(n) = ix.program.node(node) else {
        return;
    };
    let NodeKind::Derived { item, trait_name } = &n.kind else {
        return;
    };
    let method = n.ident.clone();
    let mut c = Checker::new(ix, m, inst, node, args, n.scope.clone(), &n.file);
    let item_map: Map = ix
        .program
        .item(*item)
        .scope
        .generics
        .iter()
        .copied()
        .zip(args.iter().cloned())
        .collect();
    let method_args: Vec<Ty> = args.iter().skip(item_map.len()).cloned().collect();
    let Some(trait_id) = ix.lang.trait_named(trait_name) else {
        return;
    };
    for f in ix.all_field_types(*item) {
        let f = f.subst(&item_map);
        let f = c.inst_ty(&f);
        let tr = TraitRef { id: trait_id, args: Vec::new(), bindings: Vec::new() };
        c.dispatch_trait_fn(Origin::Owner, &tr, &method, &f, &method_args, n.line, how::DERIVED, true);
    }
    c.finish(None, n.line);
}

/// Reads every constant's and static's initialiser for the closures and functions its value
/// coerces to a function pointer or closure trait object; its calls run at compile time.
pub fn constants(ix: &Index<'_>, m: &mut Mono) {
    for item in &ix.program.items {
        let ItemKind::Const { ty, init: Some(init) } = &item.kind else {
            continue;
        };
        let ships = ix
            .program
            .crates
            .get(ix.program.module(item.module).krate)
            .is_some_and(|c| c.source.role == crate::recursion::model::Role::Ships);
        if !ships {
            continue;
        }
        let file = ix.program.module(item.module).file.clone();
        let mut c = Checker::new(ix, m, InstId(usize::MAX), NodeId(usize::MAX), &[], item.scope.clone(), &file);
        c.compile_time = true;
        let declared = c.ty(ty);
        let v = c.expr(init, Some(&declared));
        c.coerce(&v, &declared, item.line);
        c.settle();
        for (t, dynamic) in std::mem::take(&mut c.coerced) {
            let t = c.resolve_final(&t);
            if matches!(t, Ty::Closure(_) | Ty::FnDef(..)) {
                c.m.coerce([t], dynamic);
            }
        }
        let problems = std::mem::take(&mut c.problems);
        c.m.global_problems.extend(problems);
    }
}

impl<'a, 'p> Checker<'a, 'p> {
    #[allow(clippy::too_many_arguments, reason = "the checker's shared state, passed once")]
    fn new(ix: &'a Index<'p>, m: &'a mut Mono, owner: InstId, node: NodeId, args: &[Ty], scope: Scope, file: &str) -> Checker<'a, 'p> {
        let map = if node.0 == usize::MAX { Map::new() } else { ix.map_of(node, args) };
        Checker {
            ix,
            m,
            owner,
            node,
            args: args.to_vec(),
            map,
            scope,
            file: file.to_owned(),
            infer: Infer::new(),
            origin: Origin::Owner,
            scopes: vec![BTreeMap::new()],
            locals: Vec::new(),
            closures: Vec::new(),
            rets: Vec::new(),
            loops: Vec::new(),
            calls: Vec::new(),
            pending: Vec::new(),
            problems: Vec::new(),
            reported: BTreeSet::new(),
            coerced: Vec::new(),
            depth: 0,
            compile_time: false,
            closure_expect: None,
            or_binding: false,
            lenient: false,
        }
    }

    // ------------------------------------------------------------ plumbing

    fn resolver(&self) -> &Resolver<'p> {
        &self.ix.resolver
    }

    fn problem(&mut self, line: usize, what: String) {
        if self.reported.insert((line, what.clone())) {
            self.problems.push(Problem::unread(&self.file, line, &what));
        }
    }

    /// Runs `f` with the index, the inference table and the closure signatures this body knows.
    fn with_ix<R>(&mut self, f: impl FnOnce(&Index<'p>, &mut Infer, ClosureSigs<'_>) -> R) -> R {
        let closures = &self.closures;
        let global = &self.m.closure_sigs;
        let node = self.node;
        let own = &self.args;
        let sigs = |k: &ClosureKey| -> Option<(Vec<Ty>, Ty)> {
            if k.owner == node && k.subst == *own {
                closures.get(k.index).map(|c| (c.params.clone(), c.ret.clone()))
            } else {
                global.get(k).cloned()
            }
        };
        f(self.ix, &mut self.infer, &sigs)
    }

    fn fresh(&mut self) -> Ty {
        self.infer.fresh()
    }

    fn shallow(&self, t: &Ty) -> Ty {
        self.infer.shallow(t)
    }

    fn unify(&mut self, a: &Ty, b: &Ty) -> bool {
        self.infer.unify(a, b)
    }

    /// `t` with its projections resolved where they can be; one whose base is not known yet
    /// becomes a variable with an obligation.
    fn norm(&mut self, t: &Ty) -> Ty {
        let t = self.with_ix(|ix, infer, sigs| ix.normalize(t, infer, sigs));
        if !t.any(&|x| matches!(x, Ty::Assoc(..))) {
            return t;
        }
        let mut waiting = Vec::new();
        let out = t.fold(&mut |x| match x {
            Ty::Assoc(base, ..) if matches!(**base, Ty::Var(_)) => {
                let v = self.infer.fresh();
                waiting.push(Pending::Projection { assoc: x.clone(), out: v.clone() });
                Some(v)
            }
            _ => None,
        });
        self.pending.extend(waiting);
        out
    }

    /// A type from a signature or field put into this body: `_` a fresh variable, projections
    /// normalised.
    fn inst_ty(&mut self, t: &Ty) -> Ty {
        let t = self.ix.fill_infer_ty(t, &mut self.infer);
        self.norm(&t)
    }

    /// A written type, in this instance.
    fn ty(&mut self, t: &syn::Type) -> Ty {
        let written = self.resolver().ty(&self.scope, t).subst(&self.map);
        self.inst_ty(&written)
    }

    /// `t` shallowly resolved and normalised.
    fn now(&mut self, t: &Ty) -> Ty {
        let s = self.shallow(t);
        if matches!(s, Ty::Assoc(..)) {
            let n = self.norm(&s);
            return self.shallow(&n);
        }
        s
    }

    /// `t` resolved for the call graph: variables left are unknown parts.
    fn resolve_final(&mut self, t: &Ty) -> Ty {
        let r0 = self.infer.resolve(t);
        let r = self.norm(&r0);
        let r = self.infer.resolve(&r);
        r.fold(&mut |x| match x {
            Ty::Var(_) | Ty::Infer => Some(Ty::Unknown),
            _ => None,
        })
    }

    fn record(&mut self, target: Target, line: usize, how_found: &'static str) {
        if self.compile_time {
            return;
        }
        self.calls.push(Call { origin: self.origin, target, line, how: how_found });
    }

    /// Makes `actual` fit `expected`: unification, or a coercion the compiler would make
    /// (dereference, unsizing to a slice or trait object, a closure or function to a function
    /// pointer); a closure or function coerced to a trait object or function pointer is recorded.
    fn coerce(&mut self, actual: &Ty, expected: &Ty, _line: usize) {
        if self.unify(actual, expected) {
            return;
        }
        let a = self.now(actual);
        let e = self.now(expected);
        match (&a, &e) {
            (Ty::Ref(x, mx), Ty::Ref(y, my)) if *mx || !*my => {
                let y = self.now(y);
                if let Ty::Dyn(traits) = &y {
                    self.coerced_to_dyn(x, traits);
                    return;
                }
                let steps = self.with_ix(|ix, infer, sigs| ix.autoderef(x, infer, sigs));
                for s in steps {
                    if self.unify(&s, &y) {
                        return;
                    }
                }
            }
            (Ty::Adt(b, xs), Ty::Adt(c, ys)) if b == c && Some(*b) == self.ix.lang.boxed => {
                if let (Some(x), Some(y)) = (xs.first(), ys.first()) {
                    let y = self.now(y);
                    if let Ty::Dyn(traits) = &y {
                        self.coerced_to_dyn(x, traits);
                    }
                }
            }
            (Ty::Closure(_) | Ty::FnDef(..), Ty::FnPtr(params, ret)) => {
                let (params, ret) = (params.clone(), (**ret).clone());
                if let Some((p, r)) = self.callable_sig(&a) {
                    for (x, y) in p.iter().zip(&params) {
                        self.unify(x, y);
                    }
                    self.unify(&r, &ret);
                }
                self.coerced.push((a.clone(), false));
            }
            _ => {}
        }
    }

    fn coerced_to_dyn(&mut self, from: &Ty, traits: &[TraitRef]) {
        if traits.iter().any(|t| self.ix.is_fn_trait(t.id)) {
            let f = self.now(from);
            if let Some(tr) = traits.iter().find(|t| self.ix.is_fn_trait(t.id))
                && let Some((p, r)) = self.callable_sig(&f)
            {
                if let Some(Ty::Tuple(inputs)) = tr.args.first() {
                    for (x, y) in p.iter().zip(inputs) {
                        self.unify(x, y);
                    }
                }
                if let Some(out) = tr.binding("Output") {
                    self.unify(&r, out);
                }
            }
            self.coerced.push((f, true));
        }
    }

    /// A callable's parameter types and result: a closure's, a function item's, a function
    /// pointer's.
    fn callable_sig(&mut self, t: &Ty) -> Option<(Vec<Ty>, Ty)> {
        match self.now(t) {
            Ty::Closure(k) => self.with_ix(|_, _, sigs| sigs(&k)),
            Ty::FnDef(n, args) => {
                let inst = self.instantiate(n, &args, 0);
                Some((inst.inputs, inst.output))
            }
            Ty::FnPtr(p, r) => Some((p, *r)),
            Ty::Ref(inner, _) => self.callable_sig(&inner),
            _ => None,
        }
    }

    // ------------------------------------------------------------ signatures

    /// Instantiates node `node`'s signature with generic arguments `given` (`_` or missing ones
    /// fresh variables); the closure bounds of its generic parameters become obligations, and an
    /// `impl Trait` result becomes a variable fixed by the callee instance's hidden type.
    fn instantiate(&mut self, node: NodeId, given: &[Ty], line: usize) -> Instance {
        let generics: Vec<_> = self.ix.node_generics(node).to_vec();
        let mut args = Vec::new();
        for (i, _) in generics.iter().enumerate() {
            let a = match given.get(i) {
                Some(t) if !matches!(t, Ty::Infer) => self.ix.fill_infer_ty(t, &mut self.infer),
                _ => self.fresh(),
            };
            args.push(a);
        }
        let map: Map = generics.iter().copied().zip(args.iter().cloned()).collect();
        let sig = self.ix.sig(node).cloned().unwrap_or_default();
        let inputs: Vec<Ty> = sig.inputs.iter().map(|t| t.subst(&map)).collect();
        let inputs: Vec<Ty> = inputs.iter().map(|t| self.inst_ty(t)).collect();
        let raw_out = sig.output.subst(&map);
        let output = if raw_out.any(&|t| matches!(t, Ty::Opaque(_))) {
            let declared = raw_out.fold(&mut |t| match t {
                Ty::Opaque(_) => Some(Ty::Infer),
                _ => None,
            });
            let declared = self.inst_ty(&declared);
            self.pending.push(Pending::Opaque { node, args: args.clone(), declared: declared.clone(), line });
            declared
        } else {
            self.inst_ty(&raw_out)
        };
        self.fn_bounds(&generics, &args, &map);
        Instance { args, inputs, output }
    }

    /// Obligations for the bounds of generic parameters `generics` (fixed to `args`): the
    /// closure bounds' signatures, and the associated-type bindings of the others.
    fn fn_bounds(&mut self, generics: &[crate::recursion::ty::GenId], args: &[Ty], map: &Map) {
        for (g, a) in generics.iter().zip(args) {
            for b in self.ix.bounds(*g) {
                if !self.ix.is_fn_trait(b.id) {
                    if b.bindings.is_empty() {
                        continue;
                    }
                    let b = b.map(&mut |x| x.subst(map));
                    let mut args_only = b.clone();
                    args_only.bindings.clear();
                    let args_only = args_only.map(&mut |x| self.ix.fill_infer_ty(x, &mut self.infer));
                    for (name, value) in &b.bindings {
                        let value = self.inst_ty(value);
                        self.pending.push(Pending::Binding { ty: a.clone(), trait_ref: args_only.clone(), name: name.clone(), value });
                    }
                    continue;
                }
                let b = b.map(&mut |x| x.subst(map));
                let inputs = match b.args.first() {
                    Some(Ty::Tuple(items)) => items.clone(),
                    _ => continue,
                };
                let inputs: Vec<Ty> = inputs.iter().map(|t| self.inst_ty(t)).collect();
                let output = match b.binding("Output") {
                    Some(o) => self.inst_ty(o),
                    None => continue,
                };
                self.pending.push(Pending::FnBound { callee: a.clone(), inputs, output });
            }
        }
    }

    /// Instantiates trait method `name` (declared by `tr` or a supertrait) with `Self` as
    /// `self_ty`: its inputs, result and the method's own generic arguments.
    fn instantiate_trait_method(&mut self, tr: &TraitRef, name: &str, self_ty: &Ty, turbofish: &[Ty]) -> Option<(Vec<Ty>, Ty, Vec<Ty>, TraitRef)> {
        let decl = self.ix.trait_declaring_method(tr.id, name)?;
        let tr = if decl == tr.id {
            self.ix.complete(tr, self_ty)
        } else {
            self.ix
                .supertraits(tr, self_ty)
                .into_iter()
                .find(|s| s.id == decl)
                .unwrap_or(TraitRef { id: decl, args: Vec::new(), bindings: Vec::new() })
        };
        let tr = self.ix.complete(&tr, self_ty);
        let (sig, generics) = self.ix.trait_method(decl, name)?;
        let trait_generics = self.ix.program.item(decl).scope.generics.clone();
        let mut map = Map::new();
        if let Some(sg) = generics.first() {
            map.insert(*sg, self_ty.clone());
        }
        for (g, a) in trait_generics.iter().zip(&tr.args) {
            let a = self.ix.fill_infer_ty(a, &mut self.infer);
            map.insert(*g, a);
        }
        let own: Vec<_> = generics.iter().skip(trait_generics.len().saturating_add(1)).copied().collect();
        let mut method_args = Vec::new();
        for (i, g) in own.iter().enumerate() {
            let a = match turbofish.get(i) {
                Some(t) if !matches!(t, Ty::Infer) => t.clone(),
                _ => self.fresh(),
            };
            map.insert(*g, a.clone());
            method_args.push(a);
        }
        let inputs: Vec<Ty> = sig.inputs.iter().map(|t| t.subst(&map)).collect();
        let inputs: Vec<Ty> = inputs.iter().map(|t| self.inst_ty(t)).collect();
        let output = self.inst_ty(&sig.output.subst(&map));
        self.fn_bounds(&own, &method_args, &map);
        Some((inputs, output, method_args, tr))
    }

    /// Puts `turbofish` in as node `node`'s own generic arguments (after its implementation's or
    /// trait's).
    fn apply_turbofish(&self, node: NodeId, mut args: Vec<Ty>, turbofish: &[Ty]) -> Vec<Ty> {
        if turbofish.is_empty() {
            return args;
        }
        let outer = match self.ix.program.node(node).map(|n| &n.kind) {
            Some(NodeKind::Method(imp)) => self.ix.program.imp(*imp).scope.generics.len(),
            Some(NodeKind::TraitDefault(t)) => self.ix.program.item(*t).scope.generics.len().saturating_add(1),
            _ => 0,
        };
        for (i, t) in turbofish.iter().enumerate() {
            if let Some(slot) = args.get_mut(outer.saturating_add(i))
                && !matches!(t, Ty::Infer)
            {
                *slot = t.clone();
            }
        }
        args
    }

    // ------------------------------------------------------------ arguments

    /// Checks call arguments against parameter types: the others first, then closures, which
    /// take their parameter types from the bound they are passed to.
    fn check_args(&mut self, params: &[Ty], args: Args<'_>) -> Vec<Ty> {
        match args {
            Args::Tys(tys) => {
                for (p, a) in params.iter().zip(tys) {
                    self.coerce(a, p, 0);
                }
                tys.to_vec()
            }
            Args::Exprs(exprs) => {
                let mut out = vec![Ty::Unknown; exprs.len()];
                for (i, a) in exprs.iter().enumerate() {
                    if is_closure(a) {
                        continue;
                    }
                    let p = params.get(i).cloned();
                    let t = self.expr(a, p.as_ref());
                    if let Some(p) = &p {
                        self.coerce(&t, p, line_of(a));
                    }
                    if let Some(slot) = out.get_mut(i) {
                        *slot = t;
                    }
                }
                self.settle_quick();
                for (i, a) in exprs.iter().enumerate() {
                    if !is_closure(a) {
                        continue;
                    }
                    let p = params.get(i).cloned();
                    self.closure_expect = p.as_ref().and_then(|p| self.closure_expectation(p));
                    let t = self.expr(a, p.as_ref());
                    if let Some(p) = &p {
                        self.coerce(&t, p, line_of(a));
                    }
                    if let Some(slot) = out.get_mut(i) {
                        *slot = t;
                    }
                }
                out
            }
        }
    }

    /// The parameter types and result a closure passed where `param` is expected takes.
    fn closure_expectation(&mut self, param: &Ty) -> Option<(Vec<Ty>, Ty)> {
        let p = self.now(param);
        let p = match p {
            Ty::Ref(inner, _) => self.now(&inner),
            other => other,
        };
        match &p {
            Ty::Var(v) => {
                let root = self.shallow(&Ty::Var(*v));
                let found = self.pending.iter().find_map(|x| match x {
                    Pending::FnBound { callee, inputs, output } if self.infer.shallow(callee) == root => {
                        Some((inputs.clone(), output.clone()))
                    }
                    _ => None,
                })?;
                let inputs = found.0.iter().map(|t| self.norm(t)).collect();
                let output = self.norm(&found.1);
                Some((inputs, output))
            }
            Ty::Dyn(traits) | Ty::Opaque(traits) => {
                let tr = traits.iter().find(|t| self.ix.is_fn_trait(t.id))?;
                let inputs = match tr.args.first() {
                    Some(Ty::Tuple(items)) => items.clone(),
                    _ => return None,
                };
                Some((inputs, tr.binding("Output").cloned().unwrap_or_else(Ty::unit)))
            }
            Ty::FnPtr(params, ret) => Some((params.clone(), (**ret).clone())),
            Ty::Param(g) => {
                let tr = self.ix.bounds(*g).into_iter().find(|t| self.ix.is_fn_trait(t.id))?;
                let inputs = match tr.args.first() {
                    Some(Ty::Tuple(items)) => items.clone(),
                    _ => return None,
                };
                Some((inputs, tr.binding("Output").cloned().unwrap_or_else(Ty::unit)))
            }
            _ => None,
        }
    }

    // ------------------------------------------------------------ obligations

    /// Settles every obligation that can be settled now, round after round.
    fn settle(&mut self) {
        for _ in 0..MAX_SETTLE {
            if !self.settle_round() {
                break;
            }
        }
    }

    /// One round over the obligations that do not wait on another body: cheap enough to run
    /// before a method lookup.
    fn settle_quick(&mut self) {
        for _ in 0..4 {
            if !self.settle_round() {
                break;
            }
        }
    }

    /// One round; whether anything was settled.
    fn settle_round(&mut self) -> bool {
        let all = std::mem::take(&mut self.pending);
        let mut keep = Vec::new();
        let mut progress = false;
        for p in all {
            if self.try_settle(&p) {
                progress = true;
            } else {
                keep.push(p);
            }
        }
        // Obligations made while settling come after the ones kept.
        keep.append(&mut self.pending);
        self.pending = keep;
        progress
    }

    /// Settles one obligation if its type is known; whether it did.
    #[allow(clippy::too_many_lines, reason = "one arm per kind of obligation")]
    fn try_settle(&mut self, p: &Pending) -> bool {
        match p {
            Pending::Method { origin, recv, name, args, turbofish, out, line } => {
                let r = self.now(recv);
                if self.unresolved(&r) {
                    return false;
                }
                let saved = self.origin;
                self.origin = *origin;
                let t = self.method_on(&r, name, Args::Tys(args), turbofish, None, *line);
                self.unify(out, &t);
                self.origin = saved;
                true
            }
            Pending::TraitFn { origin, trait_id, name, self_ty, trait_args, method_args, line, how: h } => {
                let s = self.now(self_ty);
                if self.unresolved(&s) {
                    return false;
                }
                let tr = TraitRef { id: *trait_id, args: trait_args.clone(), bindings: Vec::new() };
                self.dispatch_trait_fn(*origin, &tr, name, &s, method_args, *line, h, false);
                true
            }
            Pending::Field { base, name, out, line } => {
                let b = self.now(base);
                if self.unresolved(&b) {
                    return false;
                }
                let t = self.field_of(&b, name, *line);
                self.unify(out, &t);
                true
            }
            Pending::Index { origin, base, idx, out, line } => {
                let b = self.now(base);
                if self.unresolved(&b) {
                    return false;
                }
                let saved = self.origin;
                self.origin = *origin;
                let t = self.index_op(&b, idx, *line);
                self.origin = saved;
                self.unify(out, &t);
                true
            }
            Pending::Binary { origin, lhs, rhs, op, out, line } => {
                let l = self.now(lhs);
                if self.unresolved(&l) {
                    return false;
                }
                let saved = self.origin;
                self.origin = *origin;
                let t = self.operator(&l, Some(rhs), op.0, op.1, *line);
                self.origin = saved;
                self.unify(out, &t);
                true
            }
            Pending::Unary { origin, operand, op, out, line } => {
                let o = self.now(operand);
                if self.unresolved(&o) {
                    return false;
                }
                let saved = self.origin;
                self.origin = *origin;
                let t = self.operator(&o, None, op.0, op.1, *line);
                self.origin = saved;
                self.unify(out, &t);
                true
            }
            Pending::Deref { origin, operand, out, line } => {
                let o = self.now(operand);
                if self.unresolved(&o) {
                    return false;
                }
                let saved = self.origin;
                self.origin = *origin;
                let t = self.deref_op(&o, *line);
                self.origin = saved;
                self.unify(out, &t);
                true
            }
            Pending::CallValue { origin, callee, args, out, line } => {
                let c = self.now(callee);
                if self.unresolved(&c) {
                    return false;
                }
                let saved = self.origin;
                self.origin = *origin;
                let t = self.call_value(&c, Args::Tys(args), None, *line);
                self.origin = saved;
                self.unify(out, &t);
                true
            }
            Pending::Projection { assoc, out } => {
                let Ty::Assoc(base, ..) = assoc else {
                    return true;
                };
                let b = self.now(base);
                if matches!(b, Ty::Var(_)) {
                    return false;
                }
                let n = self.norm(assoc);
                self.unify(out, &n);
                true
            }
            Pending::Opaque { node, args, declared, line } => {
                let resolved: Vec<Ty> = args.iter().map(|a| self.infer.resolve(a)).collect();
                if resolved.iter().any(Ty::has_var) && !self.lenient {
                    return false;
                }
                if self.compile_time {
                    return true;
                }
                let resolved: Vec<Ty> = resolved.iter().map(|a| self.resolve_final(a)).collect();
                let inst = self.m.node_inst(self.ix, *node, resolved);
                self.m.read(Dep::Ret(inst), self.owner);
                if let Some(r) = self.m.rets.get(&inst).cloned() {
                    self.unify(declared, &r);
                } else {
                    let _ = line;
                }
                true
            }
            Pending::Iterate { origin, iterable, item, line } => {
                let t = self.now(iterable);
                if self.unresolved(&t) {
                    return false;
                }
                let saved = self.origin;
                self.origin = *origin;
                let got = self.iterate(&t, *line);
                self.origin = saved;
                self.unify(item, &got);
                true
            }
            Pending::Try { origin, operand, out, line } => {
                let t = self.now(operand);
                if self.unresolved(&t) {
                    return false;
                }
                let saved = self.origin;
                self.origin = *origin;
                let got = self.try_op(&t, *line);
                self.origin = saved;
                self.unify(out, &got);
                true
            }
            Pending::Binding { ty, trait_ref, name, value } => {
                let t = self.now(ty);
                if self.unresolved(&t) {
                    return false;
                }
                let proj = Ty::Assoc(Box::new(t), Some(Box::new(trait_ref.clone())), name.clone());
                let n = self.norm(&proj);
                if !matches!(self.shallow(&n), Ty::Assoc(..)) {
                    self.unify(&n, value);
                }
                true
            }
            Pending::FnBound { callee, inputs, output } => {
                let c = self.now(callee);
                if matches!(c, Ty::Var(_)) {
                    return false;
                }
                if let Some((p, r)) = self.callable_sig(&c) {
                    for (x, y) in p.iter().zip(inputs) {
                        self.unify(x, y);
                    }
                    self.unify(&r, output);
                }
                true
            }
        }
    }

    /// Whether a type is a variable no rule has fixed yet (a literal's variable is known enough
    /// to be a primitive).
    fn unresolved(&self, t: &Ty) -> bool {
        match t {
            Ty::Var(v) => self.infer.kind_of(*v) == VarKind::General,
            Ty::Assoc(base, ..) => matches!(self.shallow(base), Ty::Var(_)),
            _ => false,
        }
    }

    /// A literal's variable as its primitive default, for a lookup on it.
    fn numeric_default(&mut self, t: &Ty) -> Ty {
        if let Ty::Var(v) = t {
            match self.infer.kind_of(*v) {
                VarKind::Int => {
                    self.unify(t, &Ty::Prim(Prim::I32));
                    return Ty::Prim(Prim::I32);
                }
                VarKind::Float => {
                    self.unify(t, &Ty::Prim(Prim::F64));
                    return Ty::Prim(Prim::F64);
                }
                VarKind::General => {}
            }
        }
        t.clone()
    }

    // ------------------------------------------------------------ finishing

    /// Settles what is left, reports what cannot be settled, adds drop glue and turns the calls
    /// into edges.
    fn finish(&mut self, ret: Option<Ty>, line: usize) {
        self.settle();
        self.infer.default_numeric();
        self.settle();
        self.lenient = true;
        self.settle();
        let left = std::mem::take(&mut self.pending);
        for p in &left {
            self.report_unsettled(p);
        }
        self.drop_glue(line);
        let calls = std::mem::take(&mut self.calls);
        let mut closure_insts: BTreeMap<usize, InstId> = BTreeMap::new();
        let closures = self.closures.clone();
        for (i, c) in closures.iter().enumerate() {
            let key = self.resolve_key(&c.key);
            let params: Vec<Ty> = c.params.iter().map(|t| self.resolve_final(t)).collect();
            let ret = self.resolve_final(&c.ret);
            self.m.closure_sigs.insert(key.clone(), (params, ret));
            let inst = self.m.closure_inst(self.ix, &key);
            closure_insts.insert(i, inst);
        }
        for call in calls {
            let from = match call.origin {
                Origin::Owner => self.owner,
                Origin::Closure(i) => match closure_insts.get(&i) {
                    Some(x) => *x,
                    None => continue,
                },
            };
            let site = Site { file: self.file.clone(), line: call.line, how: call.how };
            for to in self.targets(&call.target, from) {
                self.m.edge(from, to, &site);
            }
        }
        for (t, dynamic) in std::mem::take(&mut self.coerced) {
            let t = self.resolve_final(&t);
            if matches!(t, Ty::Closure(_) | Ty::FnDef(..)) {
                self.m.coerce([t], dynamic);
            }
        }
        if let Some(r) = ret {
            let r = self.resolve_final(&r);
            self.m.set_ret(self.owner, r);
        }
        let problems = std::mem::take(&mut self.problems);
        if !problems.is_empty() {
            self.m.problems.insert(self.owner, problems);
        }
    }

    fn resolve_key(&mut self, k: &ClosureKey) -> ClosureKey {
        ClosureKey {
            owner: k.owner,
            subst: k.subst.iter().map(|t| self.resolve_final(t)).collect(),
            index: k.index,
        }
    }

    /// The instances a call reaches.
    fn targets(&mut self, target: &Target, from: InstId) -> Vec<InstId> {
        let ix = self.ix;
        match target {
            Target::Node(n, args) => {
                let args: Vec<Ty> = args.iter().map(|a| self.resolve_final(a)).collect();
                vec![self.m.node_inst(ix, *n, args)]
            }
            Target::Closure(k) => {
                let k = self.resolve_key(k);
                vec![self.m.closure_inst(ix, &k)]
            }
            Target::Bound { trait_ref, name, self_ty, method_args } => {
                let tr = trait_ref.map(&mut |t| self.resolve_final(t));
                let s = self.resolve_final(self_ty);
                let margs: Vec<Ty> = method_args.iter().map(|a| self.resolve_final(a)).collect();
                match ix.default_fn(&tr, name, &s, &margs) {
                    Some((n, args)) => {
                        let args: Vec<Ty> = args.iter().map(|a| self.resolve_final(a)).collect();
                        vec![self.m.node_inst(ix, n, args)]
                    }
                    None => Vec::new(),
                }
            }
            Target::Dyn { trait_id, name } => {
                let mut out = Vec::new();
                for imp in ix.by_trait.get(trait_id).cloned().unwrap_or_default() {
                    let Some(info) = ix.impls.get(imp.0) else {
                        continue;
                    };
                    let open: Map = ix.impl_generics(imp).iter().map(|g| (*g, Ty::Param(*g))).collect();
                    if let Some((n, args)) = ix.impl_fn(imp, name, &open, &info.self_ty, &[]) {
                        let args: Vec<Ty> = args.iter().map(|a| self.resolve_final(a)).collect();
                        out.push(self.m.node_inst(ix, n, args));
                    }
                }
                let trait_name = ix.program.item(*trait_id).name.clone();
                for (item, list) in &ix.derived {
                    for (t, n) in list {
                        if *t == trait_name && ix.program.node(*n).is_some_and(|x| x.ident == *name) {
                            let open: Vec<Ty> = ix.node_generics(*n).iter().map(|g| Ty::Param(*g)).collect();
                            let _ = item;
                            out.push(self.m.node_inst(ix, *n, open));
                        }
                    }
                }
                out
            }
            Target::Erased { dynamic, arity } => {
                self.m.read(if *dynamic { Dep::Dyn } else { Dep::FnPtr }, from);
                let set: Vec<Ty> = if *dynamic {
                    self.m.dyn_callables.iter().cloned().collect()
                } else {
                    self.m.fnptr_callables.iter().cloned().collect()
                };
                let mut out = Vec::new();
                for c in set {
                    match &c {
                        Ty::Closure(k) => {
                            let fits = self.m.closure_sigs.get(k).is_none_or(|(p, _)| p.len() == *arity);
                            if fits {
                                out.push(self.m.closure_inst(ix, k));
                            }
                        }
                        Ty::FnDef(n, args) => {
                            let fits = ix.sig(*n).is_none_or(|s| s.inputs.len() == *arity);
                            if fits {
                                out.push(self.m.node_inst(ix, *n, args.clone()));
                            }
                        }
                        _ => {}
                    }
                }
                out
            }
        }
    }

    fn report_unsettled(&mut self, p: &Pending) {
        let (line, what) = match p {
            Pending::Method { recv, name, line, .. } => {
                let shown = self.ix.show(&self.infer.resolve(recv));
                (*line, format!("the receiver of `.{name}(...)` has a type the check cannot infer (`{shown}`)"))
            }
            Pending::TraitFn { trait_id, name, line, .. } => (
                *line,
                format!(
                    "the type `{}::{name}` is called on cannot be inferred",
                    self.ix.program.item(*trait_id).name
                ),
            ),
            Pending::Field { name, line, .. } => (*line, format!("the type whose field `{name}` is read cannot be inferred")),
            Pending::Index { line, .. } => (*line, "the type of an indexed value cannot be inferred".to_owned()),
            Pending::Binary { op, line, .. } | Pending::Unary { op, line, .. } => {
                (*line, format!("the operand of an operator (`{}`) has a type the check cannot infer", op.0))
            }
            Pending::Deref { line, .. } => (*line, "the type of a dereferenced value cannot be inferred".to_owned()),
            Pending::CallValue { line, .. } => (*line, "the type of a value called as a function cannot be inferred".to_owned()),
            Pending::Iterate { line, .. } => (*line, "the type a `for` loop iterates over cannot be inferred".to_owned()),
            Pending::Try { line, .. } => (*line, "the type `?` is applied to cannot be inferred".to_owned()),
            Pending::Opaque { node, line, .. } => (
                *line,
                format!("the generic arguments of a call to `{}` (which returns `impl Trait`) cannot be inferred", self.ix.name(*node)),
            ),
            Pending::Projection { .. } | Pending::FnBound { .. } | Pending::Binding { .. } => return,
        };
        self.problem(line, what);
    }

    /// Drop glue: every `Drop` implementation of the crates read on a type of a local or
    /// parameter, or a type inside it.
    fn drop_glue(&mut self, line: usize) {
        let Some(drop) = self.ix.lang.trait_named("Drop") else {
            return;
        };
        let any_kernel_drop = self.ix.by_trait.get(&drop).is_some_and(|impls| {
            impls.iter().any(|i| {
                self.ix.impl_block(*i).is_some_and(|b| {
                    self.ix
                        .program
                        .crates
                        .get(self.ix.program.module(b.scope.module).krate)
                        .is_some_and(|c| c.source.role != crate::recursion::model::Role::Model)
                })
            })
        });
        if !any_kernel_drop || self.compile_time {
            return;
        }
        let mut todo: Vec<Ty> = self.locals.clone();
        let mut seen: BTreeSet<Ty> = BTreeSet::new();
        let saved = self.origin;
        self.origin = Origin::Owner;
        while let Some(t) = todo.pop() {
            let t = self.resolve_final(&t);
            if !seen.insert(t.clone()) || seen.len() > 4096 {
                continue;
            }
            match &t {
                Ty::Adt(item, args) => {
                    let tr = TraitRef { id: drop, args: Vec::new(), bindings: Vec::new() };
                    let picks = self.with_ix(|ix, infer, sigs| ix.select(&t, &tr, infer, sigs));
                    for (imp, map) in picks {
                        if let Some((n, a)) = self.ix.impl_fn(imp, "drop", &map, &t, &[]) {
                            self.record(Target::Node(n, a), line, how::DROP);
                        }
                    }
                    let kernel = self
                        .ix
                        .program
                        .crates
                        .get(self.ix.program.module(self.ix.program.item(*item).module).krate)
                        .is_some_and(|c| c.source.role != crate::recursion::model::Role::Model);
                    if kernel {
                        let map: Map = self.ix.program.item(*item).scope.generics.iter().copied().zip(args.iter().cloned()).collect();
                        for f in self.ix.all_field_types(*item) {
                            todo.push(f.subst(&map));
                        }
                    } else {
                        todo.extend(args.iter().cloned());
                    }
                }
                Ty::Tuple(items) => todo.extend(items.iter().cloned()),
                Ty::Array(inner) | Ty::Slice(inner) => todo.push((**inner).clone()),
                _ => {}
            }
        }
        self.origin = saved;
        // Drop glue for the calls just recorded is made into edges by `finish`.
    }

    // ------------------------------------------------------------ locals

    fn bind(&mut self, name: &str, t: Ty) {
        if self.or_binding
            && let Some(existing) = self.scopes.last().and_then(|s| s.get(name)).cloned()
        {
            self.unify(&existing, &t);
            return;
        }
        self.locals.push(t.clone());
        if let Some(s) = self.scopes.last_mut() {
            s.insert(name.to_owned(), t);
        }
    }

    fn local(&self, name: &str) -> Option<Ty> {
        self.scopes.iter().rev().find_map(|s| s.get(name).cloned())
    }

    fn push_scope(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
        if self.scopes.is_empty() {
            self.scopes.push(BTreeMap::new());
        }
    }

    // ------------------------------------------------------------ statements

    fn block(&mut self, b: &syn::Block, expected: Option<&Ty>) -> Ty {
        self.push_scope();
        let t = self.stmts(&b.stmts, expected);
        self.pop_scope();
        t
    }

    fn stmts(&mut self, stmts: &[Stmt], expected: Option<&Ty>) -> Ty {
        let mut last = Ty::unit();
        let n = stmts.len();
        let mut diverged = false;
        for (i, s) in stmts.iter().enumerate() {
            let tail = i.saturating_add(1) == n;
            last = match s {
                Stmt::Local(l) => {
                    self.local_stmt(l);
                    Ty::unit()
                }
                Stmt::Item(_) => Ty::unit(),
                Stmt::Expr(e, semi) => {
                    let t = self.expr(e, if tail && semi.is_none() { expected } else { None });
                    if matches!(self.shallow(&t), Ty::Never) {
                        diverged = true;
                    }
                    if semi.is_some() { Ty::unit() } else { t }
                }
                Stmt::Macro(sm) => {
                    let t = self.mac(&sm.mac, if tail && sm.semi_token.is_none() { expected } else { None });
                    if matches!(self.shallow(&t), Ty::Never) {
                        diverged = true;
                    }
                    if sm.semi_token.is_some() { Ty::unit() } else { t }
                }
            };
        }
        if diverged && matches!(last, Ty::Tuple(ref items) if items.is_empty()) {
            return Ty::Never;
        }
        last
    }

    fn local_stmt(&mut self, l: &syn::Local) {
        let (pat, declared) = match &l.pat {
            Pat::Type(pt) => (&*pt.pat, Some(self.ty(&pt.ty))),
            other => (other, None),
        };
        let t = match &l.init {
            Some(init) => {
                let t = self.expr(&init.expr, declared.as_ref());
                if let Some(d) = &declared {
                    self.coerce(&t, d, line_of(&init.expr));
                }
                if let Some((_, diverge)) = &init.diverge {
                    self.expr(diverge, None);
                }
                declared.unwrap_or(t)
            }
            None => declared.unwrap_or_else(|| self.fresh()),
        };
        self.bind_pat(pat, &t, Mode::Move);
    }

    // ------------------------------------------------------------ patterns

    /// Binds pattern `pat` against a value of type `t` in default binding mode `mode`.
    #[allow(clippy::too_many_lines, reason = "one arm per pattern form, each short")]
    fn bind_pat(&mut self, pat: &Pat, t: &Ty, mode: Mode) {
        match pat {
            Pat::Ident(pi) => {
                let name = pi.ident.to_string();
                if pi.subpat.is_none() && upper(&name) {
                    // A constant or unit item: its type meets the scrutinee's.
                    let segs = vec![name];
                    let res = self.resolver().resolve(&self.scope, &segs, false, Ns::Value);
                    if let Some(r) = res.first() {
                        let v = self.res_value(r, None, line_of(pat));
                        let s = self.peel_for_pattern(t).0;
                        self.unify(&v, &s);
                    }
                    return;
                }
                let bound = match (&pi.by_ref, &pi.mutability) {
                    (Some(_), Some(_)) => Ty::reference_mut(t.clone()),
                    (Some(_), None) => Ty::reference(t.clone()),
                    (None, _) => mode.bind(t.clone()),
                };
                self.bind(&pi.ident.to_string(), bound);
                if let Some((_, sub)) = &pi.subpat {
                    self.bind_pat(sub, t, mode);
                }
            }
            Pat::Tuple(pt) => {
                let (s, mode) = self.peel_for_pattern_mode(t, mode);
                let rest = pt.elems.iter().position(|p| matches!(p, Pat::Rest(_)));
                let n = pt.elems.len();
                let items = match self.now(&s) {
                    Ty::Tuple(items) => items,
                    Ty::Var(_) if rest.is_none() => {
                        let items: Vec<Ty> = (0..n).map(|_| self.fresh()).collect();
                        self.unify(&s, &Ty::Tuple(items.clone()));
                        items
                    }
                    _ => Vec::new(),
                };
                for (i, p) in pt.elems.iter().enumerate() {
                    if matches!(p, Pat::Rest(_)) {
                        continue;
                    }
                    let idx = match rest {
                        Some(r) if i > r => items.len().checked_sub(n.saturating_sub(i)),
                        _ => Some(i),
                    };
                    let et = idx.and_then(|j| items.get(j)).cloned().unwrap_or(Ty::Unknown);
                    self.bind_pat(p, &et, mode);
                }
            }
            Pat::TupleStruct(ts) => {
                let (s, mode) = self.peel_for_pattern_mode(t, mode);
                let fields = self.pattern_fields(&ts.path, &s, line_of(pat));
                let n = ts.elems.len();
                let rest = ts.elems.iter().position(|p| matches!(p, Pat::Rest(_)));
                for (i, p) in ts.elems.iter().enumerate() {
                    if matches!(p, Pat::Rest(_)) {
                        continue;
                    }
                    let idx = match rest {
                        Some(r) if i > r => fields.len().checked_sub(n.saturating_sub(i)),
                        _ => Some(i),
                    };
                    let ft = idx.and_then(|j| fields.get(j)).map_or(Ty::Unknown, |(_, t)| t.clone());
                    self.bind_pat(p, &ft, mode);
                }
            }
            Pat::Struct(ps) => {
                let (s, mode) = self.peel_for_pattern_mode(t, mode);
                let fields = self.pattern_fields(&ps.path, &s, line_of(pat));
                for fp in &ps.fields {
                    let name = match &fp.member {
                        syn::Member::Named(id) => id.to_string(),
                        syn::Member::Unnamed(ix) => ix.index.to_string(),
                    };
                    let ft = fields.iter().find(|(n, _)| *n == name).map_or(Ty::Unknown, |(_, t)| t.clone());
                    self.bind_pat(&fp.pat, &ft, mode);
                }
            }
            Pat::Path(pp) => {
                let v = self.path_value_of(&pp.qself, &pp.path, None, line_of(pat));
                let (s, _) = self.peel_for_pattern_mode(t, mode);
                self.unify(&v, &s);
            }
            Pat::Slice(ps) => {
                let (s, mode) = self.peel_for_pattern_mode(t, mode);
                let elem = match self.now(&s) {
                    Ty::Slice(e) | Ty::Array(e) => (*e).clone(),
                    _ => Ty::Unknown,
                };
                for p in &ps.elems {
                    match p {
                        Pat::Rest(_) => {}
                        Pat::Ident(pi) if matches!(pi.subpat.as_ref().map(|(_, s)| &**s), Some(Pat::Rest(_))) => {
                            let sl = Ty::Slice(Box::new(elem.clone()));
                            let bound = match (&pi.by_ref, &pi.mutability) {
                                (Some(_), Some(_)) => Ty::reference_mut(sl),
                                (Some(_), None) => Ty::reference(sl),
                                (None, _) => mode.bind(sl),
                            };
                            self.bind(&pi.ident.to_string(), bound);
                        }
                        other => self.bind_pat(other, &elem, mode),
                    }
                }
            }
            Pat::Reference(pr) => {
                let inner = match self.now(t) {
                    Ty::Ref(i, _) => (*i).clone(),
                    Ty::Var(_) => {
                        let i = self.fresh();
                        self.unify(t, &Ty::Ref(Box::new(i.clone()), pr.mutability.is_some()));
                        i
                    }
                    other => other,
                };
                self.bind_pat(&pr.pat, &inner, Mode::Move);
            }
            Pat::Or(po) => {
                let saved = self.or_binding;
                for (i, case) in po.cases.iter().enumerate() {
                    self.or_binding = saved || i > 0;
                    self.bind_pat(case, t, mode);
                }
                self.or_binding = saved;
            }
            Pat::Type(pt) => {
                let d = self.ty(&pt.ty);
                self.unify(t, &d);
                self.bind_pat(&pt.pat, &d, mode);
            }
            Pat::Paren(pp) => self.bind_pat(&pp.pat, t, mode),
            Pat::Guard(pg) => {
                self.bind_pat(&pg.pat, t, mode);
                let b = Ty::Prim(Prim::Bool);
                self.expr(&pg.guard, Some(&b));
            }
            Pat::Lit(pl) => {
                let lt = self.lit(&pl.lit, None);
                let (s, _) = self.peel_for_pattern_mode(t, mode);
                self.unify(&lt, &s);
            }
            Pat::Range(pr) => {
                let (s, _) = self.peel_for_pattern_mode(t, mode);
                if let Some(start) = &pr.start {
                    let x = self.expr(start, Some(&s));
                    self.unify(&x, &s);
                }
                if let Some(end) = &pr.end {
                    let x = self.expr(end, Some(&s));
                    self.unify(&x, &s);
                }
            }
            Pat::Const(pc) => {
                let saved = self.compile_time;
                self.compile_time = true;
                self.block(&pc.block, None);
                self.compile_time = saved;
            }
            Pat::Macro(pm) => {
                self.problem(line_of(pm), "a macro in a pattern, which the recursion check does not expand".to_owned());
            }
            Pat::Rest(_) | Pat::Wild(_) => {}
            _ => {
                self.problem(line_of(pat), format!("a pattern the recursion check cannot read: `{}`", macros::excerpt(&pat.to_token_stream())));
            }
        }
    }

    /// The scrutinee type behind references, for a non-reference pattern.
    fn peel_for_pattern(&mut self, t: &Ty) -> (Ty, Mode) {
        self.peel_for_pattern_mode(t, Mode::Move)
    }

    /// The scrutinee type behind references, for a non-reference pattern, and the default binding
    /// mode matching through them gives.
    fn peel_for_pattern_mode(&mut self, t: &Ty, mode: Mode) -> (Ty, Mode) {
        let mut cur = self.now(t);
        let mut mode = mode;
        while let Ty::Ref(inner, m) = cur {
            cur = self.now(&inner);
            mode = mode.through(m);
        }
        (cur, mode)
    }

    /// The fields a struct or variant pattern's path names, with types for scrutinee `s`.
    fn pattern_fields(&mut self, path: &syn::Path, s: &Ty, line: usize) -> Vec<(String, Ty)> {
        let segs = Resolver::segments(path);
        let mut res = self.resolver().resolve(&self.scope, &segs, path.leading_colon.is_some(), Ns::Value);
        res.extend(self.resolver().resolve(&self.scope, &segs, path.leading_colon.is_some(), Ns::Type));
        let target = res.iter().find_map(|r| match r {
            Res::Variant(item, v) => Some((*item, Some(*v))),
            Res::Item(item) if self.ix.program.item(*item).is_adt() => Some((*item, None)),
            Res::SelfTy => match self.self_type() {
                Some(Ty::Adt(item, _)) => Some((item, None)),
                _ => None,
            },
            _ => None,
        });
        let Some((item, variant)) = target else {
            self.problem(line, format!("a pattern path that names no struct or variant: `{}`", path.to_token_stream()));
            return Vec::new();
        };
        let generics = self.ix.program.item(item).scope.generics.clone();
        let vars: Vec<Ty> = generics.iter().map(|_| self.infer.fresh()).collect();
        self.unify(s, &Ty::Adt(item, vars.clone()));
        let map: Map = generics.iter().copied().zip(vars).collect();
        let fields = self.ix.fields(item, variant);
        fields
            .into_iter()
            .map(|(n, t)| {
                let t = t.subst(&map);
                let t = self.inst_ty(&t);
                (n, t)
            })
            .collect()
    }

    fn self_type(&mut self) -> Option<Ty> {
        if self.node.0 == usize::MAX {
            return None;
        }
        let t = self.ix.self_ty(self.node)?.subst(&self.map);
        Some(self.inst_ty(&t))
    }

    // ------------------------------------------------------------ expressions

    #[allow(clippy::too_many_lines, reason = "one arm per expression form, each short")]
    fn expr(&mut self, e: &Expr, expected: Option<&Ty>) -> Ty {
        let line = line_of(e);
        match e {
            Expr::Lit(l) => self.lit(&l.lit, expected),
            Expr::Path(p) => match self.eta_arity(p) {
                Some(arity) => self.eta(p, arity, expected),
                None => self.path_value_of(&p.qself, &p.path, expected, line),
            },
            Expr::Call(c) => self.call(c, expected, line),
            Expr::MethodCall(mc) => {
                let recv = self.expr(&mc.receiver, None);
                let turbofish: Vec<Ty> = mc
                    .turbofish
                    .as_ref()
                    .map(|t| {
                        t.args
                            .iter()
                            .filter_map(|a| match a {
                                syn::GenericArgument::Type(ty) => Some(ty.clone()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
                    .iter()
                    .map(|t| self.ty(t))
                    .collect();
                let name = mc.method.to_string();
                let args: Vec<Expr> = mc.args.iter().cloned().collect();
                self.method_call(&recv, &name, &args, &turbofish, expected, line)
            }
            Expr::Field(f) => {
                let base = self.expr(&f.base, None);
                let name = match &f.member {
                    syn::Member::Named(id) => id.to_string(),
                    syn::Member::Unnamed(ix) => ix.index.to_string(),
                };
                let b = self.now(&base);
                if self.unresolved(&b) {
                    self.settle_quick();
                }
                let b = self.now(&base);
                if self.unresolved(&b) {
                    let out = self.fresh();
                    self.pending.push(Pending::Field { base, name, out: out.clone(), line });
                    return out;
                }
                self.field_of(&b, &name, line)
            }
            Expr::Index(i) => {
                let base = self.expr(&i.expr, None);
                let idx = self.expr(&i.index, None);
                let b = self.now(&base);
                if self.unresolved(&b) {
                    let out = self.fresh();
                    self.pending.push(Pending::Index { origin: self.origin, base, idx, out: out.clone(), line });
                    return out;
                }
                self.index_op(&b, &idx, line)
            }
            Expr::Binary(b) => self.binary(b, expected, line),
            Expr::Unary(u) => {
                let operand = self.expr(&u.expr, if matches!(u.op, UnOp::Deref(_)) { None } else { expected });
                match u.op {
                    UnOp::Deref(_) => {
                        let o = self.now(&operand);
                        if self.unresolved(&o) {
                            let out = self.fresh();
                            self.pending.push(Pending::Deref { origin: self.origin, operand, out: out.clone(), line });
                            return out;
                        }
                        self.deref_op(&o, line)
                    }
                    UnOp::Not(_) => self.unary(&operand, ("Not", "not"), line),
                    UnOp::Neg(_) => self.unary(&operand, ("Neg", "neg"), line),
                    _ => {
                        self.problem(line, "a unary operator the recursion check does not know".to_owned());
                        Ty::Unknown
                    }
                }
            }
            Expr::Cast(c) => {
                let v = self.expr(&c.expr, None);
                let t = self.ty(&c.ty);
                let vs = self.now(&v);
                if matches!(vs, Ty::Closure(_) | Ty::FnDef(..)) && matches!(self.now(&t), Ty::FnPtr(..)) {
                    self.coerce(&v, &t, line);
                }
                t
            }
            Expr::Paren(p) => self.expr(&p.expr, expected),
            Expr::Group(g) => self.expr(&g.expr, expected),
            Expr::Reference(r) => {
                let inner_expected = expected.map(|t| self.now(t)).and_then(|t| match t {
                    Ty::Ref(i, _) => Some((*i).clone()),
                    _ => None,
                });
                let v = self.expr(&r.expr, inner_expected.as_ref());
                Ty::Ref(Box::new(v), r.mutability.is_some())
            }
            Expr::RawAddr(r) => {
                let v = self.expr(&r.expr, None);
                Ty::Ref(Box::new(v), matches!(r.mutability, syn::PointerMutability::Mut(_)))
            }
            Expr::Block(b) => self.labelled_block(b.label.as_ref(), &b.block, expected),
            Expr::Unsafe(u) => self.block(&u.block, expected),
            Expr::Const(c) => {
                let saved = self.compile_time;
                self.compile_time = true;
                let t = self.block(&c.block, expected);
                self.compile_time = saved;
                t
            }
            Expr::If(i) => {
                self.push_scope();
                let b = Ty::Prim(Prim::Bool);
                self.expr(&i.cond, Some(&b));
                let then = self.block(&i.then_branch, expected);
                self.pop_scope();
                match &i.else_branch {
                    Some((_, els)) => {
                        let et = self.expr(els, expected.or(Some(&then)));
                        self.join(&then, &et, expected, line)
                    }
                    None => Ty::unit(),
                }
            }
            Expr::Let(l) => {
                let v = self.expr(&l.expr, None);
                self.bind_pat(&l.pat, &v, Mode::Move);
                Ty::Prim(Prim::Bool)
            }
            Expr::Match(mt) => {
                let v = self.expr(&mt.expr, None);
                let mut out: Option<Ty> = expected.cloned();
                for arm in &mt.arms {
                    self.push_scope();
                    self.bind_pat(&arm.pat, &v, Mode::Move);
                    let b = self.expr(&arm.body, out.as_ref());
                    out = Some(match out {
                        Some(o) => self.join(&o, &b, expected, line),
                        None => b,
                    });
                    self.pop_scope();
                }
                out.unwrap_or(Ty::Never)
            }
            Expr::Loop(l) => {
                let label = l.label.as_ref().map(|x| x.name.ident.to_string());
                self.loops.push((label, Ty::Never));
                self.block(&l.body, None);
                let (_, t) = self.loops.pop().unwrap_or((None, Ty::Never));
                t
            }
            Expr::While(wh) => {
                let label = wh.label.as_ref().map(|x| x.name.ident.to_string());
                self.loops.push((label, Ty::unit()));
                self.push_scope();
                let b = Ty::Prim(Prim::Bool);
                self.expr(&wh.cond, Some(&b));
                self.block(&wh.body, None);
                self.pop_scope();
                self.loops.pop();
                Ty::unit()
            }
            Expr::ForLoop(f) => {
                let it = self.expr(&f.expr, None);
                let item = self.fresh();
                let t = self.now(&it);
                if self.unresolved(&t) {
                    self.pending.push(Pending::Iterate { origin: self.origin, iterable: it, item: item.clone(), line });
                } else {
                    let got = self.iterate(&t, line);
                    self.unify(&item, &got);
                }
                let label = f.label.as_ref().map(|x| x.name.ident.to_string());
                self.loops.push((label, Ty::unit()));
                self.push_scope();
                self.bind_pat(&f.pat, &item, Mode::Move);
                self.block(&f.body, None);
                self.pop_scope();
                self.loops.pop();
                Ty::unit()
            }
            Expr::Break(b) => {
                let label = b.label.as_ref().map(|l| l.ident.to_string());
                let target = match &label {
                    Some(name) => self.loops.iter().rposition(|(l, _)| l.as_deref() == Some(name.as_str())),
                    None => self.loops.iter().rposition(|_| true),
                };
                let want = target.and_then(|i| self.loops.get(i)).map(|(_, t)| t.clone());
                let v = match &b.expr {
                    Some(x) => self.expr(x, want.as_ref().filter(|t| !matches!(t, Ty::Never))),
                    None => Ty::unit(),
                };
                if let Some(i) = target
                    && let Some((_, acc)) = self.loops.get(i).cloned()
                {
                    let joined = if matches!(acc, Ty::Never) {
                        v
                    } else {
                        self.unify(&acc, &v);
                        acc
                    };
                    if let Some(slot) = self.loops.get_mut(i) {
                        slot.1 = joined;
                    }
                }
                Ty::Never
            }
            Expr::Continue(_) => Ty::Never,
            Expr::Return(r) => {
                let declared = self.rets.last().cloned();
                let v = match &r.expr {
                    Some(x) => self.expr(x, declared.as_ref()),
                    None => Ty::unit(),
                };
                if let Some(d) = &declared {
                    self.coerce(&v, d, line);
                }
                Ty::Never
            }
            Expr::Closure(c) => self.closure(c, line),
            Expr::Struct(s) => self.struct_lit(s, expected, line),
            Expr::Tuple(t) => {
                let want: Vec<Ty> = match expected.map(|x| self.now(x)) {
                    Some(Ty::Tuple(items)) if items.len() == t.elems.len() => items,
                    _ => Vec::new(),
                };
                let items: Vec<Ty> = t.elems.iter().enumerate().map(|(i, x)| self.expr(x, want.get(i))).collect();
                Ty::Tuple(items)
            }
            Expr::Array(a) => {
                let want = match expected.map(|x| self.now(x)) {
                    Some(Ty::Array(e) | Ty::Slice(e)) => Some(*e),
                    _ => None,
                };
                let elem = want.unwrap_or_else(|| self.fresh());
                for x in &a.elems {
                    let t = self.expr(x, Some(&elem));
                    self.coerce(&t, &elem, line);
                }
                Ty::Array(Box::new(elem))
            }
            Expr::Repeat(r) => {
                let want = match expected.map(|x| self.now(x)) {
                    Some(Ty::Array(e)) => Some(*e),
                    _ => None,
                };
                let v = self.expr(&r.expr, want.as_ref());
                let saved = self.compile_time;
                self.compile_time = true;
                let usize_ = Ty::Prim(Prim::Usize);
                self.expr(&r.len, Some(&usize_));
                self.compile_time = saved;
                Ty::Array(Box::new(v))
            }
            Expr::Range(r) => {
                let elem = self.fresh();
                if let Some(s) = &r.start {
                    let x = self.expr(s, Some(&elem));
                    self.unify(&x, &elem);
                }
                if let Some(x) = &r.end {
                    let y = self.expr(x, Some(&elem));
                    self.unify(&y, &elem);
                }
                let kind = match (&r.start, &r.end, &r.limits) {
                    (Some(_), Some(_), syn::RangeLimits::HalfOpen(_)) => "Range",
                    (Some(_), Some(_), syn::RangeLimits::Closed(_)) => "RangeInclusive",
                    (Some(_), None, _) => "RangeFrom",
                    (None, Some(_), syn::RangeLimits::HalfOpen(_)) => "RangeTo",
                    (None, Some(_), syn::RangeLimits::Closed(_)) => "RangeToInclusive",
                    (None, None, _) => "RangeFull",
                };
                match self.ix.lang.ranges.get(kind) {
                    Some(id) if kind == "RangeFull" => Ty::Adt(*id, Vec::new()),
                    Some(id) => Ty::Adt(*id, vec![elem]),
                    None => {
                        self.problem(line, format!("the model has no `{kind}`"));
                        Ty::Unknown
                    }
                }
            }
            Expr::Try(t) => {
                let v = self.expr(&t.expr, None);
                let s = self.now(&v);
                if self.unresolved(&s) {
                    let out = self.fresh();
                    self.pending.push(Pending::Try { origin: self.origin, operand: v, out: out.clone(), line });
                    return out;
                }
                self.try_op(&s, line)
            }
            Expr::Macro(m) => self.mac(&m.mac, expected),
            Expr::Assign(a) => {
                let place = self.expr(&a.left, None);
                let v = self.expr(&a.right, Some(&place));
                self.coerce(&v, &place, line);
                Ty::unit()
            }
            Expr::Infer(_) => self.fresh(),
            _ => {
                self.problem(
                    line,
                    format!("an expression the recursion check cannot read: `{}`", macros::excerpt(&e.to_token_stream())),
                );
                Ty::Unknown
            }
        }
    }

    /// The type of two branches meeting: the expected type when there is one, else their
    /// unification (a diverging branch takes the other's type).
    fn join(&mut self, a: &Ty, b: &Ty, expected: Option<&Ty>, line: usize) -> Ty {
        if let Some(e) = expected {
            self.coerce(a, e, line);
            self.coerce(b, e, line);
            return e.clone();
        }
        let (sa, sb) = (self.now(a), self.now(b));
        if matches!(sa, Ty::Never) {
            return b.clone();
        }
        if matches!(sb, Ty::Never) {
            return a.clone();
        }
        if !self.unify(a, b) {
            self.coerce(b, a, line);
        }
        a.clone()
    }

    fn labelled_block(&mut self, label: Option<&syn::Label>, b: &syn::Block, expected: Option<&Ty>) -> Ty {
        match label {
            Some(l) => {
                self.loops.push((Some(l.name.ident.to_string()), Ty::Never));
                let t = self.block(b, expected);
                let (_, brk) = self.loops.pop().unwrap_or((None, Ty::Never));
                self.join(&t, &brk, expected, line_of(b))
            }
            None => self.block(b, expected),
        }
    }

    fn lit(&mut self, l: &syn::Lit, expected: Option<&Ty>) -> Ty {
        match l {
            syn::Lit::Str(_) => Ty::reference(Ty::Prim(Prim::Str)),
            syn::Lit::ByteStr(_) => Ty::reference(Ty::Array(Box::new(Ty::Prim(Prim::U8)))),
            syn::Lit::CStr(_) => Ty::Unknown,
            syn::Lit::Byte(_) => Ty::Prim(Prim::U8),
            syn::Lit::Char(_) => Ty::Prim(Prim::Char),
            syn::Lit::Bool(_) => Ty::Prim(Prim::Bool),
            syn::Lit::Int(i) => {
                if let Some(p) = Prim::from_name(i.suffix()) {
                    return Ty::Prim(p);
                }
                let v = self.infer.fresh_int();
                if let Some(e) = expected {
                    let e = self.now(e);
                    if matches!(e, Ty::Prim(p) if p.is_int()) || matches!(e, Ty::Var(_)) {
                        self.unify(&v, &e);
                    }
                }
                v
            }
            syn::Lit::Float(f) => {
                if let Some(p) = Prim::from_name(f.suffix()) {
                    return Ty::Prim(p);
                }
                let v = self.infer.fresh_float();
                if let Some(e) = expected {
                    let e = self.now(e);
                    if matches!(e, Ty::Prim(p) if p.is_float()) || matches!(e, Ty::Var(_)) {
                        self.unify(&v, &e);
                    }
                }
                v
            }
            _ => Ty::Unknown,
        }
    }

    // ------------------------------------------------------------ paths

    /// How many parameters the trait method a path names as a value takes, when its `Self` is not
    /// known here: `Trait::method`, `<T as Trait>::method`, or `T::method` on a generic `T`. Such a
    /// value is the function item `<Self as Trait>::method`, which [`Checker::eta`] reads as the
    /// closure that calls it.
    fn eta_arity(&mut self, p: &syn::ExprPath) -> Option<usize> {
        let name = p.path.segments.last()?.ident.to_string();
        if upper(&name) {
            return None;
        }
        let trait_of = |ix: &Index<'_>, r: &Res| match r {
            Res::Item(t) if matches!(ix.program.item(*t).kind, ItemKind::Trait { .. }) => Some(*t),
            _ => None,
        };
        let arity_in = |ix: &Index<'_>, t: ItemId| -> Option<usize> {
            let decl = ix.trait_declaring_method(t, &name)?;
            ix.trait_method(decl, &name).map(|(sig, _)| sig.inputs.len())
        };
        if let Some(q) = &p.qself {
            if q.position > 0 {
                let trait_path: Vec<String> = p.path.segments.iter().take(q.position).map(|s| s.ident.to_string()).collect();
                let res = self.resolver().resolve(&self.scope, &trait_path, p.path.leading_colon.is_some(), Ns::Type);
                let base = self.ty(&q.ty);
                let b = self.now(&base);
                if !matches!(b, Ty::Param(_) | Ty::Opaque(_) | Ty::Assoc(..) | Ty::Dyn(_) | Ty::Var(_)) {
                    return None;
                }
                return res.iter().find_map(|r| trait_of(self.ix, r)).and_then(|t| arity_in(self.ix, t));
            }
            return None;
        }
        let segs = Resolver::segments(&p.path);
        let res = self.resolver().resolve(&self.scope, &segs, p.path.leading_colon.is_some(), Ns::Value);
        let Some(Res::Assoc(base, _)) = res.first() else {
            return None;
        };
        if let Some(t) = trait_of(self.ix, base) {
            return arity_in(self.ix, t);
        }
        let base_ty = match &**base {
            Res::Generic(g) => self.map.get(g).cloned().unwrap_or(Ty::Param(*g)),
            Res::SelfTy => self.self_type()?,
            _ => return None,
        };
        let b = self.now(&base_ty);
        if !matches!(b, Ty::Param(_) | Ty::Opaque(_) | Ty::Assoc(..) | Ty::Dyn(_)) {
            return None;
        }
        self.ix
            .bounds_of_ty(&b)
            .iter()
            .find(|tr| self.ix.trait_declaring_method(tr.id, &name) == Some(tr.id))
            .and_then(|tr| arity_in(self.ix, tr.id))
    }

    /// A trait method named as a value whose `Self` is not known here: the closure
    /// `|a0, .., an| path(a0, .., an)`, which calls exactly what the function item would (Rust's
    /// function items and closures are both callables of their own types).
    fn eta(&mut self, p: &syn::ExprPath, arity: usize, expected: Option<&Ty>) -> Ty {
        let span = syn::spanned::Spanned::span(p);
        let params: Vec<proc_macro2::Ident> = (0..arity).map(|i| quote::format_ident!("__eta{}", i, span = span)).collect();
        let tokens = quote::quote_spanned!(span=> |#(#params),*| #p(#(#params),*));
        match syn::parse2::<Expr>(tokens) {
            Ok(closure) => {
                self.closure_expect = expected.and_then(|e| self.closure_expectation(e));
                self.expr(&closure, expected)
            }
            Err(_) => {
                self.problem(line_of(p), format!("a trait method used as a value (`{}`)", p.to_token_stream()));
                Ty::Unknown
            }
        }
    }

    /// The value a path names (with its qualified self type, if written).
    fn path_value_of(&mut self, qself: &Option<syn::QSelf>, path: &syn::Path, expected: Option<&Ty>, line: usize) -> Ty {
        if let Some(q) = qself {
            let base = self.ty(&q.ty);
            let name = path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
            return self.assoc_value(&base, &name, line);
        }
        if let Some(name) = ident_of(path)
            && let Some(t) = self.local(&name)
        {
            return t;
        }
        let segs = Resolver::segments(path);
        let res = self.resolver().resolve(&self.scope, &segs, path.leading_colon.is_some(), Ns::Value);
        let Some(r) = res.first().cloned() else {
            self.problem(line, format!("a path that resolves to nothing: `{}`", path.to_token_stream()));
            return Ty::Unknown;
        };
        let t = self.res_value(&r, Some(path), line);
        if let Some(e) = expected {
            let s = self.now(&t);
            if matches!(s, Ty::Adt(..)) {
                self.unify(&t, e);
            }
        }
        t
    }

    /// The value a resolution names.
    fn res_value(&mut self, r: &Res, path: Option<&syn::Path>, line: usize) -> Ty {
        let last = path.and_then(|p| p.segments.last());
        match r {
            Res::Fn(n) => {
                let turbofish: Vec<Ty> = last
                    .map(|s| self.resolver().segment_args(&self.scope, s))
                    .unwrap_or_default()
                    .iter()
                    .map(|t| t.subst(&self.map))
                    .collect();
                let generics = self.ix.node_generics(*n).len();
                let mut args: Vec<Ty> = (0..generics).map(|_| self.infer.fresh()).collect();
                args = self.apply_turbofish(*n, args, &turbofish);
                Ty::FnDef(*n, args)
            }
            Res::Variant(item, v) => self.ctor_value(*item, Some(*v)),
            Res::Item(id) => {
                let item = self.ix.program.item(*id);
                match &item.kind {
                    ItemKind::Const { ty, .. } => {
                        let scope = item.scope.clone();
                        let t = self.resolver().ty(&scope, ty);
                        self.inst_ty(&t)
                    }
                    ItemKind::Struct { .. } => self.ctor_value(*id, None),
                    _ => {
                        self.problem(line, format!("a path that names no value: `{}`", self.ix.program.item(*id).path));
                        Ty::Unknown
                    }
                }
            }
            Res::Assoc(base, name) => {
                let base_args = path
                    .and_then(|p| p.segments.iter().rev().nth(1))
                    .map(|s| self.resolver().segment_args(&self.scope, s))
                    .unwrap_or_default();
                if let Res::Item(t) = &**base
                    && matches!(self.ix.program.item(*t).kind, ItemKind::Trait { .. })
                {
                    self.problem(line, format!("a trait's method used as a value (`{}::{name}`)", self.ix.program.item(*t).name));
                    return Ty::Unknown;
                }
                let base_ty = self.base_ty(base, base_args);
                self.assoc_value(&base_ty, name, line)
            }
            Res::ConstParam(_) => Ty::Prim(Prim::Usize),
            Res::SelfTy => match self.self_type() {
                Some(Ty::Adt(item, args)) => {
                    let fields = self.ix.fields(item, None);
                    if fields.is_empty() {
                        Ty::Adt(item, args)
                    } else {
                        self.ctor_value(item, None)
                    }
                }
                Some(t) => t,
                None => Ty::Unknown,
            },
            Res::Ext(segs) => {
                self.problem(line, format!("a path into a crate the check does not read: `{}`", segs.join("::")));
                Ty::Unknown
            }
            Res::Generic(_) | Res::Mod(_) | Res::Prim(_) | Res::Macro(_) | Res::Qualified(..) => {
                self.problem(line, "a path that names no value".to_owned());
                Ty::Unknown
            }
        }
    }

    /// A struct's or variant's value: the value itself for a unit one, else its constructor (a
    /// function that runs no code of the program).
    fn ctor_value(&mut self, item: ItemId, variant: Option<usize>) -> Ty {
        let generics = self.ix.program.item(item).scope.generics.clone();
        let vars: Vec<Ty> = generics.iter().map(|_| self.infer.fresh()).collect();
        let adt = Ty::Adt(item, vars.clone());
        let map: Map = generics.iter().copied().zip(vars).collect();
        let fields = self.ix.fields(item, variant);
        let is_unit = match &self.ix.program.item(item).kind {
            ItemKind::Enum { variants, .. } => variant
                .and_then(|v| variants.get(v))
                .is_some_and(|v| matches!(v.fields, crate::recursion::model::Fields::Unit)),
            ItemKind::Struct { fields, .. } => matches!(fields, crate::recursion::model::Fields::Unit),
            _ => false,
        };
        if is_unit {
            return adt;
        }
        let params: Vec<Ty> = fields.iter().map(|(_, t)| t.subst(&map)).collect();
        let params: Vec<Ty> = params.iter().map(|t| self.inst_ty(t)).collect();
        Ty::FnPtr(params, Box::new(adt))
    }

    /// The type an associated path's base names, in this instance.
    fn base_ty(&mut self, base: &Res, args: Vec<Ty>) -> Ty {
        let t = match base {
            Res::Generic(g) => self.map.get(g).cloned().unwrap_or(Ty::Param(*g)),
            Res::SelfTy => return self.self_type().unwrap_or(Ty::Unknown),
            other => self.resolver().res_ty(&self.scope, other, args),
        };
        let t = t.subst(&self.map);
        self.inst_ty(&t)
    }

    /// `Type::name` as a value: an associated constant, or a function item.
    fn assoc_value(&mut self, base: &Ty, name: &str, line: usize) -> Ty {
        if let Some(t) = self.with_ix(|ix, infer, sigs| ix.assoc_const(base, name, infer, sigs)) {
            return self.inst_ty(&t);
        }
        let picks = self.with_ix(|ix, infer, sigs| ix.assoc_fns(base, name, infer, sigs));
        match picks.first() {
            Some(Pick::Node { node, args }) => {
                let args = self.ix.fill_infer(args.clone(), &mut self.infer);
                Ty::FnDef(*node, args)
            }
            Some(_) => {
                self.problem(line, format!("a generic parameter's method used as a value (`{name}`)"));
                Ty::Unknown
            }
            None => {
                let shown = self.ix.show(&self.infer.resolve(base));
                self.problem(line, format!("no associated item `{name}` of `{shown}`"));
                Ty::Unknown
            }
        }
    }

    // ------------------------------------------------------------ calls

    /// A call expression `f(args)`.
    fn call(&mut self, c: &syn::ExprCall, expected: Option<&Ty>, line: usize) -> Ty {
        let args: Vec<Expr> = c.args.iter().cloned().collect();
        if let Expr::Path(p) = &*c.func {
            let local = p.qself.is_none() && ident_of(&p.path).is_some_and(|n| self.local(&n).is_some());
            if !local {
                return self.path_call(p, &args, expected, line);
            }
        }
        let callee = self.expr(&c.func, None);
        let cs = self.now(&callee);
        if self.unresolved(&cs) {
            let tys: Vec<Ty> = args.iter().map(|a| self.expr(a, None)).collect();
            let out = self.fresh();
            self.pending.push(Pending::CallValue { origin: self.origin, callee, args: tys, out: out.clone(), line });
            return out;
        }
        self.call_value(&cs, Args::Exprs(&args), expected, line)
    }

    /// A call by path.
    #[allow(clippy::too_many_lines, reason = "one arm per kind of callee a path may name")]
    fn path_call(&mut self, p: &syn::ExprPath, args: &[Expr], expected: Option<&Ty>, line: usize) -> Ty {
        let last = p.path.segments.last();
        let name = last.map(|s| s.ident.to_string()).unwrap_or_default();
        let turbofish: Vec<Ty> = last
            .map(|s| self.resolver().segment_args(&self.scope, s))
            .unwrap_or_default()
            .iter()
            .map(|t| {
                let t = t.subst(&self.map);
                self.inst_ty(&t)
            })
            .collect();
        if let Some(q) = &p.qself {
            let base = self.ty(&q.ty);
            if q.position > 0 {
                let trait_path: Vec<String> = p.path.segments.iter().take(q.position).map(|s| s.ident.to_string()).collect();
                let tid = self
                    .resolver()
                    .resolve(&self.scope, &trait_path, p.path.leading_colon.is_some(), Ns::Type)
                    .into_iter()
                    .find_map(|r| match r {
                        Res::Item(id) if matches!(self.ix.program.item(id).kind, ItemKind::Trait { .. }) => Some(id),
                        _ => None,
                    });
                let trait_args = p
                    .path
                    .segments
                    .iter()
                    .nth(q.position.saturating_sub(1))
                    .map(|s| self.resolver().segment_args(&self.scope, s))
                    .unwrap_or_default();
                let trait_args: Vec<Ty> = trait_args.iter().map(|t| {
                    let t = t.subst(&self.map);
                    self.inst_ty(&t)
                }).collect();
                return match tid {
                    Some(t) => self.trait_path_call(t, trait_args, &name, Args::Exprs(args), Some(base), &turbofish, expected, line),
                    None => {
                        self.problem(line, format!("a qualified path whose trait the check does not read: `{}`", p.to_token_stream()));
                        Ty::Unknown
                    }
                };
            }
            return self.assoc_call(&base, &name, Args::Exprs(args), &turbofish, expected, line);
        }
        let segs = Resolver::segments(&p.path);
        let res = self.resolver().resolve(&self.scope, &segs, p.path.leading_colon.is_some(), Ns::Value);
        let Some(r) = res.first().cloned() else {
            self.problem(line, format!("a call to a path that resolves to nothing: `{}`", p.path.to_token_stream()));
            for a in args {
                self.expr(a, None);
            }
            return Ty::Unknown;
        };
        match &r {
            Res::Fn(n) => {
                let given = self.apply_turbofish(*n, vec![Ty::Infer; self.ix.node_generics(*n).len()], &turbofish);
                let inst = self.instantiate(*n, &given, line);
                if let Some(e) = expected {
                    self.unify(&inst.output, e);
                }
                let arg_tys = self.check_args(&inst.inputs, Args::Exprs(args));
                // Other functions of the same name under other `cfg`s.
                for other in res.iter().skip(1) {
                    if let Res::Fn(m) = other {
                        let oi = self.instantiate(*m, &[], line);
                        for (x, y) in oi.inputs.iter().zip(&arg_tys) {
                            self.unify(x, y);
                        }
                        self.record(Target::Node(*m, oi.args), line, how::PATH);
                    }
                }
                self.record(Target::Node(*n, inst.args), line, how::PATH);
                inst.output
            }
            Res::Variant(item, v) => self.construct(*item, Some(*v), args, expected, line),
            Res::Item(id) => match &self.ix.program.item(*id).kind {
                ItemKind::Struct { .. } => self.construct(*id, None, args, expected, line),
                ItemKind::Const { .. } => {
                    let callee = self.res_value(&r, Some(&p.path), line);
                    self.call_value(&callee, Args::Exprs(args), expected, line)
                }
                _ => {
                    self.problem(line, format!("a call to something that is not a function: `{}`", p.path.to_token_stream()));
                    Ty::Unknown
                }
            },
            Res::Assoc(base, name) => {
                let base_args = p
                    .path
                    .segments
                    .iter()
                    .rev()
                    .nth(1)
                    .map(|s| self.resolver().segment_args(&self.scope, s))
                    .unwrap_or_default();
                if let Res::Item(t) = &**base
                    && matches!(self.ix.program.item(*t).kind, ItemKind::Trait { .. })
                {
                    let targs: Vec<Ty> = base_args.iter().map(|x| {
                        let x = x.subst(&self.map);
                        self.inst_ty(&x)
                    }).collect();
                    return self.trait_path_call(*t, targs, name, Args::Exprs(args), None, &turbofish, expected, line);
                }
                let base_ty = self.base_ty(base, base_args);
                self.assoc_call(&base_ty, name, Args::Exprs(args), &turbofish, expected, line)
            }
            Res::SelfTy => match self.self_type() {
                Some(Ty::Adt(item, _)) => self.construct(item, None, args, expected, line),
                _ => Ty::Unknown,
            },
            Res::Ext(segs) => {
                self.problem(line, format!("a call into a crate the check does not read: `{}`", segs.join("::")));
                for a in args {
                    self.expr(a, None);
                }
                Ty::Unknown
            }
            _ => {
                self.problem(line, format!("a call to something that is not a function: `{}`", p.path.to_token_stream()));
                Ty::Unknown
            }
        }
    }

    /// A struct's or variant's constructor called with `args`.
    fn construct(&mut self, item: ItemId, variant: Option<usize>, args: &[Expr], expected: Option<&Ty>, line: usize) -> Ty {
        let generics = self.ix.program.item(item).scope.generics.clone();
        let vars: Vec<Ty> = generics.iter().map(|_| self.infer.fresh()).collect();
        let adt = Ty::Adt(item, vars.clone());
        if let Some(e) = expected {
            self.unify(&adt, e);
        }
        let map: Map = generics.iter().copied().zip(vars).collect();
        let fields: Vec<Ty> = self.ix.fields(item, variant).iter().map(|(_, t)| t.subst(&map)).collect();
        let fields: Vec<Ty> = fields.iter().map(|t| self.inst_ty(t)).collect();
        let _ = line;
        self.check_args(&fields, Args::Exprs(args));
        adt
    }

    /// `Type::name(args)`: the functions of the type, its implementations' or its bounds'.
    #[allow(clippy::too_many_arguments, reason = "each is one part of the call")]
    fn assoc_call(&mut self, base: &Ty, name: &str, args: Args<'_>, turbofish: &[Ty], expected: Option<&Ty>, line: usize) -> Ty {
        let b = self.now(base);
        // A variant through an alias or `Self::Variant(..)`.
        if let Ty::Adt(item, _) = &b
            && let ItemKind::Enum { variants, .. } = &self.ix.program.item(*item).kind
            && let Some(v) = variants.iter().position(|v| v.name == name)
            && let Args::Exprs(exprs) = args
        {
            return self.construct(*item, Some(v), exprs, expected.or(Some(&b)), line);
        }
        let picks = self.with_ix(|ix, infer, sigs| ix.assoc_fns(&b, name, infer, sigs));
        if picks.is_empty() {
            let shown = self.ix.show(&self.infer.resolve(&b));
            self.problem(line, format!("no function `{name}` of `{shown}`"));
            if let Args::Exprs(exprs) = args {
                for a in exprs {
                    self.expr(a, None);
                }
            }
            return Ty::Unknown;
        }
        self.dispatch(&picks, name, args, turbofish, expected, line, how::PATH, false)
    }

    /// `Trait::name(args)`, `<T as Trait>::name(args)`: `Self` from `explicit_self`, the
    /// receiver argument or the expected result, and the implementation once `Self` is known.
    #[allow(clippy::too_many_arguments, reason = "each is one part of the call")]
    fn trait_path_call(
        &mut self,
        trait_id: ItemId,
        trait_args: Vec<Ty>,
        name: &str,
        args: Args<'_>,
        explicit_self: Option<Ty>,
        turbofish: &[Ty],
        expected: Option<&Ty>,
        line: usize,
    ) -> Ty {
        let self_ty = explicit_self.unwrap_or_else(|| self.fresh());
        let tr = TraitRef { id: trait_id, args: trait_args, bindings: Vec::new() };
        let Some((inputs, output, method_args, tr)) = self.instantiate_trait_method(&tr, name, &self_ty, turbofish) else {
            self.problem(line, format!("trait `{}` has no method `{name}`", self.ix.program.item(trait_id).name));
            if let Args::Exprs(exprs) = args {
                for a in exprs {
                    self.expr(a, None);
                }
            }
            return Ty::Unknown;
        };
        if let Some(e) = expected {
            self.unify(&output, e);
        }
        self.check_args(&inputs, args);
        let s = self.now(&self_ty);
        if self.unresolved(&s) {
            self.settle_quick();
        }
        let s = self.now(&self_ty);
        if self.unresolved(&s) {
            self.pending.push(Pending::TraitFn {
                origin: self.origin,
                trait_id: tr.id,
                name: name.to_owned(),
                self_ty,
                trait_args: tr.args,
                method_args,
                line,
                how: how::PATH,
            });
        } else {
            self.dispatch_trait_fn(self.origin, &tr, name, &s, &method_args, line, how::PATH, false);
        }
        output
    }

    /// Records what trait `tr`'s method `name` runs on a `Self` of type `s` (known): a bound or
    /// trait object's dispatch, or the implementation (or derived or built-in one) that applies.
    /// `quiet` leaves out the problem when none applies (a derived implementation's field whose
    /// bound the derive implies).
    #[allow(clippy::too_many_arguments, reason = "each is one part of the call")]
    fn dispatch_trait_fn(&mut self, origin: Origin, tr: &TraitRef, name: &str, s: &Ty, method_args: &[Ty], line: usize, how_found: &'static str, quiet: bool) {
        let saved = self.origin;
        self.origin = origin;
        let now = self.now(s);
        let s = self.numeric_default(&now);
        let decl = self.ix.trait_declaring_method(tr.id, name).unwrap_or(tr.id);
        match &s {
            Ty::Param(_) | Ty::Opaque(_) | Ty::Assoc(..) => {
                let tr = TraitRef { id: decl, args: tr.args.clone(), bindings: Vec::new() };
                self.record(Target::Bound { trait_ref: tr, name: name.to_owned(), self_ty: s.clone(), method_args: method_args.to_vec() }, line, how::BOUND);
            }
            Ty::Dyn(_) => {
                self.record(Target::Dyn { trait_id: decl, name: name.to_owned() }, line, how::DYN);
            }
            Ty::Closure(k) if self.ix.is_fn_trait(decl) => {
                self.record(Target::Closure((**k).clone()), line, how::VALUE);
            }
            Ty::FnDef(n, a) if self.ix.is_fn_trait(decl) => {
                self.record(Target::Node(*n, a.clone()), line, how::VALUE);
            }
            Ty::Unknown => {
                if !quiet {
                    self.problem(line, format!("`{}::{name}` on a type the check cannot infer", self.ix.program.item(decl).name));
                }
            }
            _ => {
                let picks = self.with_ix(|ix, infer, sigs| ix.select(&s, tr, infer, sigs));
                let mut found = false;
                for (imp, map) in &picks {
                    if let Some((n, args)) = self.ix.impl_fn(*imp, name, map, &s, method_args) {
                        let args = self.ix.fill_infer(args, &mut self.infer);
                        self.record(Target::Node(n, args), line, how_found);
                        found = true;
                    }
                }
                if !found
                    && let Ty::Adt(item, args) = &s
                    && let Some(n) = self.ix.derived_fn(*item, &self.ix.program.item(decl).name, name)
                {
                    let mut a = args.clone();
                    a.extend(method_args.iter().cloned());
                    self.record(Target::Node(n, a), line, how::DERIVED);
                    found = true;
                }
                if !found && !self.ix.is_marker(decl) && !quiet {
                    let shown = self.ix.show(&self.infer.resolve(&s));
                    self.problem(line, format!("no implementation of `{}` for `{shown}` (for `{name}`)", self.ix.program.item(decl).name));
                }
            }
        }
        self.origin = saved;
    }

    /// `receiver.name(args)` with the receiver's type `recv`.
    fn method_call(&mut self, recv: &Ty, name: &str, args: &[Expr], turbofish: &[Ty], expected: Option<&Ty>, line: usize) -> Ty {
        let r = self.now(recv);
        if self.unresolved(&r) {
            self.settle_quick();
        }
        let r = self.now(recv);
        if self.unresolved(&r) {
            let tys: Vec<Ty> = args.iter().map(|a| self.expr(a, None)).collect();
            let out = self.fresh();
            self.pending.push(Pending::Method {
                origin: self.origin,
                recv: recv.clone(),
                name: name.to_owned(),
                args: tys,
                turbofish: turbofish.to_vec(),
                out: out.clone(),
                line,
            });
            return out;
        }
        self.method_on(&r, name, Args::Exprs(args), turbofish, expected, line)
    }

    /// A method call on a receiver of known type `r`.
    fn method_on(&mut self, r: &Ty, name: &str, args: Args<'_>, turbofish: &[Ty], expected: Option<&Ty>, line: usize) -> Ty {
        let r = self.numeric_default(r);
        if matches!(r, Ty::Unknown) {
            self.problem(line, format!("the receiver of `.{name}(...)` has a type the check cannot infer"));
            if let Args::Exprs(exprs) = args {
                for a in exprs {
                    self.expr(a, None);
                }
            }
            return Ty::Unknown;
        }
        let (picks, _) = self.with_ix(|ix, infer, sigs| ix.probe(&r, name, infer, sigs));
        if picks.is_empty() {
            let shown = self.ix.show(&self.infer.resolve(&r));
            self.problem(line, format!("no method `{name}` on `{shown}`"));
            if let Args::Exprs(exprs) = args {
                for a in exprs {
                    self.expr(a, None);
                }
            }
            return Ty::Unknown;
        }
        self.dispatch(&picks, name, args, turbofish, expected, line, how::RECEIVER, true)
    }

    /// What calling `pick` takes and gives: its parameters (after the receiver, for method
    /// syntax), its result, and the call to record.
    fn prepare(&mut self, pick: &Pick, name: &str, turbofish: &[Ty], line: usize, skip: usize) -> Option<Prepared> {
        let before = self.pending.len();
        let (params, output, target) = match pick {
            Pick::Node { node, args } => {
                let gargs = self.apply_turbofish(*node, args.clone(), turbofish);
                let inst = self.instantiate(*node, &gargs, line);
                (inst.inputs, inst.output, Target::Node(*node, inst.args))
            }
            Pick::Bound { trait_ref, self_ty } | Pick::Dyn { trait_ref, self_ty } => {
                let (inputs, output, margs, tr) = self.instantiate_trait_method(trait_ref, name, self_ty, turbofish)?;
                let target = if matches!(pick, Pick::Dyn { .. }) {
                    Target::Dyn { trait_id: tr.id, name: name.to_owned() }
                } else {
                    Target::Bound { trait_ref: tr, name: name.to_owned(), self_ty: self_ty.clone(), method_args: margs }
                };
                (inputs, output, target)
            }
        };
        let obligations = self.pending.split_off(before.min(self.pending.len()));
        Some(Prepared { params: params.into_iter().skip(skip).collect(), output, target, obligations })
    }

    /// Whether the checked argument types `tys` fit a prepared candidate: its parameters unify
    /// with them, and its bounds' associated-type bindings hold where they can be told.
    fn fits(&mut self, p: &Prepared, tys: &[Ty]) -> bool {
        let snap = self.infer.snapshot();
        let mut ok = p.params.iter().zip(tys).all(|(x, a)| self.infer.unify(x, a));
        if ok {
            for o in &p.obligations {
                let Pending::Binding { ty, trait_ref, name, value } = o else {
                    continue;
                };
                let t = self.now(ty);
                if self.unresolved(&t) {
                    continue;
                }
                let proj = Ty::Assoc(Box::new(t), Some(Box::new(trait_ref.clone())), name.clone());
                let n = self.with_ix(|ix, infer, sigs| ix.normalize(&proj, infer, sigs));
                if !matches!(self.shallow(&n), Ty::Assoc(..)) && !self.infer.unify(&n, value) {
                    ok = false;
                    break;
                }
            }
        }
        self.infer.rollback_to(snap);
        ok
    }

    /// Calls `picks` with `args`; `method` says the receiver was given by method syntax (it is
    /// not among `args`). One candidate is checked with the arguments' expected types; several
    /// (where open types let more than one implementation apply) each get the arguments their
    /// parameters fit, and every one of them is recorded.
    #[allow(clippy::too_many_arguments, reason = "each is one part of the call")]
    fn dispatch(&mut self, picks: &[Pick], name: &str, args: Args<'_>, turbofish: &[Ty], expected: Option<&Ty>, line: usize, how_found: &'static str, method: bool) -> Ty {
        let skip = usize::from(method);
        let how_of = |t: &Target| match t {
            Target::Bound { .. } => how::BOUND,
            Target::Dyn { .. } => how::DYN,
            _ => how_found,
        };
        if let [pick] = picks {
            let Some(Prepared { params, output, target, obligations }) = self.prepare(pick, name, turbofish, line, skip) else {
                return Ty::Unknown;
            };
            self.pending.extend(obligations);
            if let Some(e) = expected {
                self.unify(&output, e);
            }
            self.check_args(&params, args);
            let h = how_of(&target);
            self.record(target, line, h);
            return output;
        }
        let tys: Vec<Ty> = match args {
            Args::Exprs(exprs) => exprs.iter().map(|a| self.expr(a, None)).collect(),
            Args::Tys(t) => t.to_vec(),
        };
        let mut prepared = Vec::new();
        for pick in picks {
            if let Some(p) = self.prepare(pick, name, turbofish, line, skip) {
                prepared.push(p);
            }
        }
        let mut fitting: Vec<usize> = Vec::new();
        for (i, p) in prepared.iter().enumerate() {
            if self.fits(p, &tys) {
                fitting.push(i);
            }
        }
        // When no candidate fits the arguments, every one is kept (the check does not judge
        // types it may have inferred too narrowly).
        let chosen: Vec<usize> = if fitting.is_empty() { (0..prepared.len()).collect() } else { fitting };
        let mut out = None;
        for i in &chosen {
            let Some(Prepared { params, output, target, obligations }) = prepared.get(*i).cloned() else {
                continue;
            };
            if chosen.len() == 1 {
                self.pending.extend(obligations);
                for (p, a) in params.iter().zip(&tys) {
                    self.coerce(a, p, line);
                }
                if let Some(e) = expected {
                    self.unify(&output, e);
                }
            }
            let h = how_of(&target);
            self.record(target, line, h);
            if out.is_none() {
                out = Some(output);
            }
        }
        out.unwrap_or(Ty::Unknown)
    }

    /// A call through a value of type `c` (known).
    fn call_value(&mut self, c: &Ty, args: Args<'_>, expected: Option<&Ty>, line: usize) -> Ty {
        let mut c = self.now(c);
        for _ in 0..crate::recursion::index::DEREF_DEPTH {
            match &c {
                Ty::Ref(inner, _) => c = self.now(inner),
                Ty::Adt(b, inner) if Some(*b) == self.ix.lang.boxed => {
                    c = inner.first().map_or(Ty::Unknown, |t| self.now(t));
                }
                _ => break,
            }
        }
        match &c {
            Ty::Closure(k) => {
                let sig = self.with_ix(|_, _, sigs| sigs(k));
                let (params, ret) = sig.unwrap_or_else(|| (Vec::new(), Ty::Unknown));
                if let Some(e) = expected {
                    self.unify(&ret, e);
                }
                self.check_args(&params, args);
                self.record(Target::Closure((**k).clone()), line, how::VALUE);
                ret
            }
            Ty::FnDef(n, gargs) => {
                let inst = self.instantiate(*n, gargs, line);
                if let Some(e) = expected {
                    self.unify(&inst.output, e);
                }
                self.check_args(&inst.inputs, args);
                self.record(Target::Node(*n, inst.args), line, how::VALUE);
                inst.output
            }
            Ty::FnPtr(params, ret) => {
                let (params, ret) = (params.clone(), (**ret).clone());
                self.check_args(&params, args);
                self.record(Target::Erased { dynamic: false, arity: params.len() }, line, how::ERASED);
                ret
            }
            Ty::Dyn(traits) | Ty::Opaque(traits) => {
                let traits = traits.clone();
                let Some((inputs, output)) = self.callable_bound(&c, traits) else {
                    self.problem(line, format!("a call of a value of type `{}`, which has no closure trait", self.ix.show(&c)));
                    return Ty::Unknown;
                };
                self.check_args(&inputs, args);
                if matches!(c, Ty::Dyn(_)) {
                    self.record(Target::Erased { dynamic: true, arity: inputs.len() }, line, how::ERASED);
                }
                output
            }
            Ty::Param(_) | Ty::Assoc(..) => {
                // A callable a generic parameter stands for comes from outside the crates read.
                if let Some((inputs, output)) = self.callable_bound(&c, Vec::new()) {
                    self.check_args(&inputs, args);
                    return output;
                }
                self.problem(line, format!("a call of a value of type `{}`, which has no closure bound", self.ix.show(&c)));
                Ty::Unknown
            }
            _ => {
                let shown = self.ix.show(&self.infer.resolve(&c));
                self.problem(line, format!("a call of a value of type `{shown}`, which the check cannot follow"));
                if let Args::Exprs(exprs) = args {
                    for a in exprs {
                        self.expr(a, None);
                    }
                }
                Ty::Unknown
            }
        }
    }

    /// The parameter types and result of a closure trait among `t`'s bounds (or `traits`).
    fn callable_bound(&mut self, t: &Ty, traits: Vec<TraitRef>) -> Option<(Vec<Ty>, Ty)> {
        let all = if traits.is_empty() { self.ix.bounds_of_ty(t) } else { traits };
        let tr = all.iter().find(|x| self.ix.is_fn_trait(x.id))?;
        let inputs = match tr.args.first() {
            Some(Ty::Tuple(items)) => items.clone(),
            _ => return None,
        };
        let output = all.iter().find_map(|x| x.binding("Output").cloned()).unwrap_or_else(Ty::unit);
        let inputs = inputs.iter().map(|x| self.inst_ty(x)).collect();
        let output = self.inst_ty(&output);
        Some((inputs, output))
    }

    /// A struct literal.
    fn struct_lit(&mut self, s: &syn::ExprStruct, expected: Option<&Ty>, line: usize) -> Ty {
        let segs = Resolver::segments(&s.path);
        let mut res = self.resolver().resolve(&self.scope, &segs, s.path.leading_colon.is_some(), Ns::Type);
        res.extend(self.resolver().resolve(&self.scope, &segs, s.path.leading_colon.is_some(), Ns::Value));
        let mut self_args: Option<Vec<Ty>> = None;
        let target = res.iter().find_map(|r| match r {
            Res::Variant(item, v) => Some((*item, Some(*v))),
            Res::Item(item) if self.ix.program.item(*item).is_adt() => Some((*item, None)),
            Res::Item(item) if matches!(self.ix.program.item(*item).kind, ItemKind::Alias { .. }) => {
                match self.resolver().item_ty(*item) {
                    Ty::Adt(id, _) => Some((id, None)),
                    _ => None,
                }
            }
            Res::SelfTy => None,
            Res::Assoc(base, name) => {
                // `Self::Variant { .. }` or `Alias::Variant { .. }`.
                let bt = match &**base {
                    Res::SelfTy => self.ix.self_ty(self.node).map(|t| t.subst(&self.map)),
                    other => Some(self.resolver().res_ty(&self.scope, other, Vec::new())),
                };
                match bt {
                    Some(Ty::Adt(item, _)) => match &self.ix.program.item(item).kind {
                        ItemKind::Enum { variants, .. } => variants.iter().position(|v| v.name == *name).map(|v| (item, Some(v))),
                        _ => None,
                    },
                    _ => None,
                }
            }
            _ => None,
        });
        let target = match target {
            Some(t) => Some(t),
            None if res.iter().any(|r| matches!(r, Res::SelfTy)) => match self.self_type() {
                Some(Ty::Adt(item, args)) => {
                    self_args = Some(args);
                    Some((item, None))
                }
                _ => None,
            },
            None => None,
        };
        let Some((item, variant)) = target else {
            for f in &s.fields {
                self.expr(&f.expr, None);
            }
            self.problem(line, format!("a struct literal of a type the check cannot name: `{}`", s.path.to_token_stream()));
            return Ty::Unknown;
        };
        let generics = self.ix.program.item(item).scope.generics.clone();
        let vars: Vec<Ty> = match self_args {
            Some(a) => a,
            None => generics.iter().map(|_| self.infer.fresh()).collect(),
        };
        let adt = Ty::Adt(item, vars.clone());
        if let Some(e) = expected {
            self.unify(&adt, e);
        }
        let map: Map = generics.iter().copied().zip(vars).collect();
        let fields = self.ix.fields(item, variant);
        for fv in &s.fields {
            let name = match &fv.member {
                syn::Member::Named(id) => id.to_string(),
                syn::Member::Unnamed(i) => i.index.to_string(),
            };
            let declared = fields.iter().find(|(n, _)| *n == name).map(|(_, t)| t.subst(&map));
            let declared = declared.map(|d| self.inst_ty(&d));
            let v = self.expr(&fv.expr, declared.as_ref());
            if let Some(d) = &declared {
                self.coerce(&v, d, line);
            }
        }
        if let Some(r) = &s.rest {
            let rv = self.expr(r, Some(&adt));
            self.unify(&rv, &adt);
        }
        adt
    }

    // ------------------------------------------------------------ operators and places

    fn field_of(&mut self, b: &Ty, name: &str, line: usize) -> Ty {
        let found = self.with_ix(|ix, infer, sigs| ix.field_ty(b, name, infer, sigs));
        match found {
            Some(t) => self.inst_ty(&t),
            None => {
                let shown = self.ix.show(&self.infer.resolve(b));
                self.problem(line, format!("no field `{name}` on `{shown}`"));
                Ty::Unknown
            }
        }
    }

    /// Whether a type is a number, a `bool` or a `char` (or a literal's variable), behind
    /// references: the operators on it are built in and call nothing.
    fn is_builtin_operand(&mut self, t: &Ty) -> bool {
        let mut cur = self.now(t);
        while let Ty::Ref(inner, _) = cur {
            cur = self.now(&inner);
        }
        match cur {
            Ty::Prim(p) => !matches!(p, Prim::Str),
            Ty::Var(v) => self.infer.kind_of(v) != VarKind::General,
            _ => false,
        }
    }

    fn peel_refs(&mut self, t: &Ty) -> Ty {
        let mut cur = self.now(t);
        while let Ty::Ref(inner, _) = cur {
            cur = self.now(&inner);
        }
        cur
    }

    fn binary(&mut self, b: &syn::ExprBinary, expected: Option<&Ty>, line: usize) -> Ty {
        let bool_ = Ty::Prim(Prim::Bool);
        if matches!(b.op, BinOp::And(_) | BinOp::Or(_)) {
            self.expr(&b.left, Some(&bool_));
            self.expr(&b.right, Some(&bool_));
            return bool_;
        }
        let Some((tr, m)) = binop_trait(&b.op) else {
            self.problem(line, "a binary operator the recursion check does not know".to_owned());
            return Ty::Unknown;
        };
        let compare = matches!(tr, "PartialEq" | "PartialOrd");
        let assign = tr.ends_with("Assign");
        let shift = matches!(tr, "Shl" | "Shr" | "ShlAssign" | "ShrAssign");
        let l = self.expr(&b.left, if compare || assign { None } else { expected });
        let rhs_expected = if shift || !self.is_builtin_operand(&l) { None } else { Some(self.peel_refs(&l)) };
        let r = self.expr(&b.right, rhs_expected.as_ref());
        if self.is_builtin_operand(&l) && self.is_builtin_operand(&r) {
            if !shift {
                let (pl, pr) = (self.peel_refs(&l), self.peel_refs(&r));
                self.unify(&pl, &pr);
            }
            return if compare {
                bool_
            } else if assign {
                Ty::unit()
            } else {
                self.peel_refs(&l)
            };
        }
        let lt = self.now(&l);
        let result = if self.unresolved(&lt) {
            let out = self.fresh();
            self.pending.push(Pending::Binary { origin: self.origin, lhs: l, rhs: r, op: (tr, m), out: out.clone(), line });
            out
        } else {
            self.operator(&lt, Some(&r), tr, m, line)
        };
        if compare {
            bool_
        } else if assign {
            Ty::unit()
        } else {
            result
        }
    }

    fn unary(&mut self, operand: &Ty, op: (&'static str, &'static str), line: usize) -> Ty {
        if self.is_builtin_operand(operand) {
            return self.peel_refs(operand);
        }
        let o = self.now(operand);
        if self.unresolved(&o) {
            let out = self.fresh();
            self.pending.push(Pending::Unary { origin: self.origin, operand: operand.clone(), op, out: out.clone(), line });
            return out;
        }
        self.operator(&o, None, op.0, op.1, line)
    }

    /// An operator on a left operand of known type `l` that is not built in: trait `tr`'s method
    /// `m`, with the right operand `r` as its argument; the trait's `Output`.
    fn operator(&mut self, l: &Ty, r: Option<&Ty>, tr: &str, m: &str, line: usize) -> Ty {
        if r.is_none_or(|x| self.is_builtin_operand(x)) && self.is_builtin_operand(l) {
            return self.peel_refs(l);
        }
        let Some(tid) = self.ix.lang.trait_named(tr) else {
            self.problem(line, format!("the model has no `{tr}`"));
            return Ty::Unknown;
        };
        let args: Vec<Ty> = r.into_iter().cloned().collect();
        let l = self.numeric_default(l);
        match &l {
            Ty::Param(_) | Ty::Opaque(_) | Ty::Assoc(..) | Ty::Dyn(_) => {
                let trf = TraitRef { id: tid, args: args.clone(), bindings: Vec::new() };
                self.dispatch_trait_fn(self.origin, &trf, m, &l, &[], line, how::IMPLICIT, false);
                let out = Ty::Assoc(Box::new(l.clone()), Some(Box::new(trf.clone())), "Output".to_owned());
                self.norm(&out)
            }
            _ => {
                let trf = TraitRef { id: tid, args: args.clone(), bindings: Vec::new() };
                let picks = self.with_ix(|ix, infer, sigs| ix.select(&l, &trf, infer, sigs));
                if picks.is_empty() {
                    let shown = self.ix.show(&self.infer.resolve(&l));
                    let rs = r.map(|x| self.ix.show(&self.infer.resolve(x))).unwrap_or_default();
                    self.problem(line, format!("no implementation of `{tr}` for `{shown}` (right operand `{rs}`)"));
                    return Ty::Unknown;
                }
                let mut out = None;
                for (imp, map) in &picks {
                    if let Some((n, a)) = self.ix.impl_fn(*imp, m, map, &l, &[]) {
                        let a = self.ix.fill_infer(a, &mut self.infer);
                        // The right operand meets the method's parameter.
                        if let Some(rt) = r {
                            let inst_inputs = {
                                let map2 = self.ix.map_of(n, &a);
                                self.ix.sig(n).and_then(|s| s.inputs.get(1).cloned()).map(|t| t.subst(&map2))
                            };
                            if picks.len() == 1
                                && let Some(p) = inst_inputs
                            {
                                let p = self.inst_ty(&p);
                                self.coerce(rt, &p, line);
                            }
                        }
                        self.record(Target::Node(n, a), line, how::IMPLICIT);
                    }
                    if out.is_none() {
                        out = self.impl_assoc(*imp, map, "Output");
                    }
                }
                out.unwrap_or(Ty::Unknown)
            }
        }
    }

    /// Associated type `name` of implementation `imp` with arguments `map`.
    fn impl_assoc(&mut self, imp: ImplId, map: &Map, name: &str) -> Option<Ty> {
        let it = self.ix.impl_block(imp)?;
        let written = it.assoc.get(name)?;
        let mut scope = it.scope.clone();
        scope.self_ref = SelfRef::Impl(imp);
        let t = self.resolver().ty(&scope, written).subst(map);
        Some(self.inst_ty(&t))
    }

    fn deref_op(&mut self, o: &Ty, line: usize) -> Ty {
        match o {
            Ty::Ref(inner, _) => (**inner).clone(),
            Ty::Adt(..) | Ty::Param(_) | Ty::Opaque(_) | Ty::Assoc(..) => {
                let Some(deref) = self.ix.lang.trait_named("Deref") else {
                    return Ty::Unknown;
                };
                let tr = TraitRef { id: deref, args: Vec::new(), bindings: Vec::new() };
                self.dispatch_trait_fn(self.origin, &tr, "deref", o, &[], line, how::IMPLICIT, false);
                let target = Ty::Assoc(Box::new(o.clone()), Some(Box::new(tr.clone())), "Target".to_owned());
                self.norm(&target)
            }
            Ty::Unknown => Ty::Unknown,
            other => {
                let shown = self.ix.show(other);
                self.problem(line, format!("a dereference of `{shown}`, which the check cannot follow"));
                Ty::Unknown
            }
        }
    }

    /// `base[idx]`, `base` known: the first auto-dereference step with an `Index` implementation
    /// for the index's type.
    fn index_op(&mut self, base: &Ty, idx: &Ty, line: usize) -> Ty {
        let now = self.now(idx);
        // An integer literal indexing a slice, array, `Vec` or `str` is a `usize`: the only
        // integer type with a `SliceIndex` implementation.
        if let Ty::Var(v) = &now
            && self.infer.kind_of(*v) == VarKind::Int
        {
            let peeled = self.peel_refs(base);
            let std_container = match &peeled {
                Ty::Slice(_) | Ty::Array(_) | Ty::Prim(Prim::Str) => true,
                Ty::Adt(id, _) => Some(*id) == self.ix.lang.vec || Some(*id) == self.ix.lang.string,
                _ => false,
            };
            if std_container {
                self.unify(&now, &Ty::Prim(Prim::Usize));
            }
        }
        let now = self.now(idx);
        let idx = self.numeric_default(&now);
        let Some(index) = self.ix.lang.trait_named("Index") else {
            return Ty::Unknown;
        };
        let steps = self.with_ix(|ix, infer, sigs| ix.autoderef(base, infer, sigs));
        for step in steps {
            let tr = TraitRef { id: index, args: vec![idx.clone()], bindings: Vec::new() };
            let picks = self.with_ix(|ix, infer, sigs| ix.select(&step, &tr, infer, sigs));
            if picks.is_empty() {
                continue;
            }
            let mut out = None;
            for (imp, map) in &picks {
                if let Some((n, a)) = self.ix.impl_fn(*imp, "index", map, &step, &[]) {
                    let a = self.ix.fill_infer(a, &mut self.infer);
                    self.record(Target::Node(n, a), line, how::IMPLICIT);
                }
                if out.is_none() {
                    out = self.impl_assoc(*imp, map, "Output");
                }
            }
            return out.unwrap_or(Ty::Unknown);
        }
        let shown = self.ix.show(&self.infer.resolve(base));
        let is = self.ix.show(&self.infer.resolve(&idx));
        self.problem(line, format!("no `Index<{is}>` for `{shown}`"));
        Ty::Unknown
    }

    /// The items a `for` loop over a value of type `t` (known) binds: `IntoIterator::into_iter`,
    /// then `Iterator::next` on its result.
    fn iterate(&mut self, t: &Ty, line: usize) -> Ty {
        let (Some(into), Some(iter)) = (self.ix.lang.trait_named("IntoIterator"), self.ix.lang.trait_named("Iterator")) else {
            return Ty::Unknown;
        };
        let it = self.trait_path_call(into, Vec::new(), "into_iter", Args::Tys(&[t.clone()]), Some(t.clone()), &[], None, line);
        let it = self.norm(&it);
        let next = self.trait_path_call(iter, Vec::new(), "next", Args::Tys(&[Ty::reference_mut(it.clone())]), Some(it), &[], None, line);
        let item = self.fresh();
        if let Some(option) = self.ix.lang.option {
            self.unify(&next, &Ty::Adt(option, vec![item.clone()]));
        }
        item
    }

    /// `e?` on a value of type `t` (known).
    fn try_op(&mut self, t: &Ty, line: usize) -> Ty {
        let ix = self.ix;
        let lang = &ix.lang;
        match t {
            Ty::Adt(id, args) if Some(*id) == lang.result => {
                let ok = args.first().cloned().unwrap_or(Ty::Unknown);
                let err = args.get(1).cloned().unwrap_or(Ty::Unknown);
                // The error converts into the function's (or closure's) error type.
                let ret = self.rets.last().cloned().unwrap_or(Ty::Unknown);
                let target_err = match self.now(&ret) {
                    Ty::Adt(r, ra) if Some(r) == self.ix.lang.result => ra.get(1).cloned(),
                    Ty::Var(_) => {
                        let (a, b) = (self.fresh(), self.fresh());
                        if let Some(result) = self.ix.lang.result {
                            self.unify(&ret, &Ty::Adt(result, vec![a, b.clone()]));
                        }
                        Some(b)
                    }
                    _ => None,
                };
                if let (Some(target), Some(from)) = (target_err, self.ix.lang.trait_named("From")) {
                    self.trait_path_call(from, vec![err.clone()], "from", Args::Tys(&[err]), Some(target), &[], None, line);
                }
                ok
            }
            Ty::Adt(id, args) if Some(*id) == lang.option => args.first().cloned().unwrap_or(Ty::Unknown),
            Ty::Adt(id, args) if self.ix.program.item(*id).name == "ControlFlow" => args.get(1).cloned().unwrap_or(Ty::Unknown),
            other => {
                let shown = self.ix.show(&self.infer.resolve(other));
                self.problem(line, format!("`?` on `{shown}`, which the check does not read"));
                Ty::Unknown
            }
        }
    }

    // ------------------------------------------------------------ closures

    fn closure(&mut self, c: &syn::ExprClosure, line: usize) -> Ty {
        let expect = self.closure_expect.take();
        if self.compile_time && self.node.0 == usize::MAX {
            self.problem(line, "a closure in a constant's or static's value, which the recursion check does not read".to_owned());
            return Ty::Unknown;
        }
        if c.asyncness.is_some() {
            self.problem(line, "an async closure, which the recursion check cannot read".to_owned());
        }
        let index = self.closures.len();
        let key = ClosureKey { owner: self.node, subst: self.args.clone(), index };
        self.closures.push(ClosureInfo { key: key.clone(), params: Vec::new(), ret: Ty::Unknown });
        let mut params = Vec::new();
        for (j, p) in c.inputs.iter().enumerate() {
            let expected = expect.as_ref().and_then(|(inputs, _)| inputs.get(j).cloned());
            let t = match p {
                Pat::Type(pt) => {
                    let d = self.ty(&pt.ty);
                    if let Some(e) = &expected {
                        self.unify(&d, e);
                    }
                    d
                }
                _ => expected.unwrap_or_else(|| self.fresh()),
            };
            params.push(t);
        }
        let ret = match &c.output {
            syn::ReturnType::Type(_, t) => {
                let d = self.ty(t);
                if let Some((_, o)) = &expect {
                    self.unify(&d, o);
                }
                d
            }
            syn::ReturnType::Default => expect.as_ref().map_or_else(|| self.fresh(), |(_, o)| o.clone()),
        };
        if let Some(info) = self.closures.get_mut(index) {
            info.params = params.clone();
            info.ret = ret.clone();
        }
        self.push_scope();
        for (p, t) in c.inputs.iter().zip(&params) {
            match p {
                Pat::Type(pt) => self.bind_pat(&pt.pat, t, Mode::Move),
                other => self.bind_pat(other, t, Mode::Move),
            }
        }
        let saved_origin = self.origin;
        self.origin = Origin::Closure(index);
        let saved_loops = std::mem::take(&mut self.loops);
        self.rets.push(ret.clone());
        let body = self.expr(&c.body, Some(&ret));
        self.coerce(&body, &ret, line);
        self.rets.pop();
        self.loops = saved_loops;
        self.origin = saved_origin;
        self.pop_scope();
        Ty::Closure(Box::new(key))
    }

    // ------------------------------------------------------------ macros

    #[allow(clippy::too_many_lines, reason = "one arm per kind of macro the reader gives")]
    fn mac(&mut self, mac: &syn::Macro, expected: Option<&Ty>) -> Ty {
        let line = line_of(mac);
        let name = mac.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
        let str_ref = Ty::reference(Ty::Prim(Prim::Str));
        match macros::read(self.resolver(), &self.scope, mac, self.depth) {
            MacroRead::Nothing => match name.as_str() {
                "line" | "column" => Ty::Prim(Prim::U32),
                "cfg" => Ty::Prim(Prim::Bool),
                "file" | "module_path" | "stringify" | "concat" | "env" | "include_str" => str_ref,
                "include_bytes" => Ty::reference(Ty::Array(Box::new(Ty::Prim(Prim::U8)))),
                "option_env" => self.ix.lang.option.map_or(Ty::Unknown, |o| Ty::Adt(o, vec![str_ref])),
                "compile_error" => Ty::Never,
                _ => Ty::unit(),
            },
            MacroRead::Exprs(es) => match name.as_str() {
                "vec" => {
                    let elem = self.fresh();
                    for e in &es {
                        let t = self.expr(e, Some(&elem));
                        self.coerce(&t, &elem, line);
                    }
                    self.ix.lang.vec.map_or(Ty::Unknown, |v| Ty::Adt(v, vec![elem]))
                }
                "matches" | "assert_matches" | "debug_assert_matches" => {
                    for e in &es {
                        self.expr(e, None);
                    }
                    if name == "matches" { Ty::Prim(Prim::Bool) } else { Ty::unit() }
                }
                "dbg" => {
                    let mut last = Ty::unit();
                    for e in &es {
                        last = self.expr(e, expected);
                    }
                    last
                }
                "addr_of" | "addr_of_mut" => {
                    let mut last = Ty::Unknown;
                    for e in &es {
                        last = Ty::reference(self.expr(e, None));
                    }
                    last
                }
                _ => {
                    for e in &es {
                        self.expr(e, None);
                    }
                    self.problem(line, format!("an outside macro `{name}!` the check does not know the result of"));
                    Ty::Unknown
                }
            },
            MacroRead::Format { plain, formatted, destination, compared } => {
                for e in &plain {
                    self.expr(e, None);
                }
                if let Some((a, b)) = &compared {
                    let av = self.expr(a, None);
                    let bv = self.expr(b, Some(&av));
                    if !(self.is_builtin_operand(&av) && self.is_builtin_operand(&bv)) {
                        let at = self.now(&av);
                        if self.unresolved(&at) {
                            let out = self.fresh();
                            self.pending.push(Pending::Binary { origin: self.origin, lhs: av, rhs: bv, op: ("PartialEq", "eq"), out, line });
                        } else {
                            self.operator(&at, Some(&bv), "PartialEq", "eq", line);
                        }
                    }
                }
                for f in &formatted {
                    let (v, t) = match f {
                        Formatted::Expr(e, t) => (self.expr(e, None), *t),
                        Formatted::Captured(n, t) => {
                            let v = match self.local(n) {
                                Some(v) => v,
                                None => {
                                    let segs = vec![n.clone()];
                                    let res = self.resolver().resolve(&self.scope, &segs, false, Ns::Value);
                                    match res.first() {
                                        Some(r) => self.res_value(r, None, line),
                                        None => {
                                            self.problem(line, format!("a captured name `{n}` that resolves to nothing"));
                                            Ty::Unknown
                                        }
                                    }
                                }
                            };
                            (v, *t)
                        }
                    };
                    if t == FmtTrait::Count {
                        continue;
                    }
                    let Some(tid) = self.ix.fmt_traits.get(t.name()).copied() else {
                        self.problem(line, format!("the model has no formatting trait `{}`", t.name()));
                        continue;
                    };
                    let s = self.now(&v);
                    if self.unresolved(&s) {
                        self.pending.push(Pending::TraitFn {
                            origin: self.origin,
                            trait_id: tid,
                            name: "fmt".to_owned(),
                            self_ty: v,
                            trait_args: Vec::new(),
                            method_args: Vec::new(),
                            line,
                            how: how::IMPLICIT,
                        });
                    } else {
                        let tr = TraitRef { id: tid, args: Vec::new(), bindings: Vec::new() };
                        self.dispatch_trait_fn(self.origin, &tr, "fmt", &s, &[], line, how::IMPLICIT, false);
                    }
                }
                let fmt_result = self.ix.model_item("fmt::Error").zip(self.ix.lang.result).map_or(Ty::Unknown, |(e, r)| {
                    Ty::Adt(r, vec![Ty::unit(), Ty::Adt(e, Vec::new())])
                });
                let mut result = match name.as_str() {
                    "format" => self.ix.lang.string.map_or(Ty::Unknown, |s| Ty::Adt(s, Vec::new())),
                    "write" | "writeln" => fmt_result,
                    "panic" | "unreachable" | "todo" | "unimplemented" => Ty::Never,
                    "format_args" => self.ix.model_item("fmt::Arguments").map_or(Ty::Unknown, |a| Ty::Adt(a, Vec::new())),
                    _ => Ty::unit(),
                };
                if let Some(d) = &destination {
                    let dv = self.expr(d, None);
                    let args_ty = self.ix.model_item("fmt::Arguments").map_or(Ty::Unknown, |a| Ty::Adt(a, Vec::new()));
                    let d_now = self.now(&dv);
                    if self.unresolved(&d_now) {
                        let out = self.fresh();
                        self.pending.push(Pending::Method {
                            origin: self.origin,
                            recv: dv,
                            name: "write_fmt".to_owned(),
                            args: vec![args_ty],
                            turbofish: Vec::new(),
                            out: out.clone(),
                            line,
                        });
                        result = out;
                    } else {
                        result = self.method_on(&d_now, "write_fmt", Args::Tys(&[args_ty]), &[], None, line);
                    }
                }
                result
            }
            MacroRead::Repeat(x, n) => {
                let v = self.expr(&x, None);
                let usize_ = Ty::Prim(Prim::Usize);
                self.expr(&n, Some(&usize_));
                if let Some(clone) = self.ix.lang.trait_named("Clone") {
                    let s = self.now(&v);
                    if self.unresolved(&s) {
                        self.pending.push(Pending::TraitFn {
                            origin: self.origin,
                            trait_id: clone,
                            name: "clone".to_owned(),
                            self_ty: v.clone(),
                            trait_args: Vec::new(),
                            method_args: Vec::new(),
                            line,
                            how: how::IMPLICIT,
                        });
                    } else {
                        let tr = TraitRef { id: clone, args: Vec::new(), bindings: Vec::new() };
                        self.dispatch_trait_fn(self.origin, &tr, "clone", &s, &[], line, how::IMPLICIT, false);
                    }
                }
                self.ix.lang.vec.map_or(Ty::Unknown, |vec| Ty::Adt(vec, vec![v]))
            }
            MacroRead::Expanded(stmts) => {
                self.depth = self.depth.saturating_add(1);
                let t = self.stmts(&stmts, expected);
                self.depth = self.depth.saturating_sub(1);
                t
            }
            MacroRead::Unread(why) => {
                self.problem(line, why);
                Ty::Unknown
            }
        }
    }
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
    use crate::recursion::model::{CrateSource, Role};
    use crate::recursion::{Report, analyse_here};

    /// Analyses one crate `a` whose `lib.rs` is `text`.
    fn run(text: &str) -> Report {
        let text = text.to_owned();
        let read = move |p: &str| {
            if p == "c/a/src/lib.rs" {
                Ok(text.clone())
            } else {
                Err(format!("no such file {p}"))
            }
        };
        let sources = vec![CrateSource {
            package: "a".to_owned(),
            lib: "a".to_owned(),
            folder: "c/a".to_owned(),
            root: "c/a/src/lib.rs".to_owned(),
            role: Role::Ships,
        }];
        analyse_here(&sources, &read)
    }

    fn problems(r: &Report) -> Vec<String> {
        r.problems.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn debug_snippet() {
        let Ok(path) = std::env::var("G13_SNIPPET") else {
            return;
        };
        let text = std::fs::read_to_string(path).unwrap();
        let r = run(&text);
        for p in problems(&r) {
            println!("PROBLEM {p}");
        }
        for c in &r.cycles {
            println!("CYCLE {:?}", c.members);
        }
    }
}
