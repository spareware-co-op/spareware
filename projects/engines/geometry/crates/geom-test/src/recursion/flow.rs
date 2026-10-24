//! The body analysis: every call one instance's body may make, as edges of the call graph.
//!
//! What it does: [`body`] reads the body of one instance ([`crate::recursion::mono`]): a function,
//! method or trait default body with its generic arguments put in. It gives every expression a
//! value ([`Val`]: the types it may have, from the signatures and field types the source writes,
//! the standard library's model ([`crate::recursion::stdlib`]), and what each local was bound or
//! assigned), and adds an edge for every call the expression may make:
//!
//! - **Calls by path** to functions and associated functions (`f`, `Type::f`, `Trait::f`, `T::f`,
//!   `Self::f`, `<T as Trait>::f`), with the generic arguments the arguments fix.
//! - **Method calls**, by the receiver's type ([`Index::method`]).
//! - **Implicit calls**: operators and compound assignments (`Add::add`, `PartialEq::eq`, ...),
//!   unary `-` and `!`, `*` on a kernel type with `Deref`, indexing, `?` (`From::from` into the
//!   function's error type), `for` loops (`into_iter`, `next`), the formatting traits of
//!   formatting macros ([`crate::recursion::macros`]), and drop glue.
//! - **Calls through values**: a closure or function item called as `f(x)`, directly or through
//!   a generic parameter the instance fixed to it; a closure trait object or function pointer,
//!   which may run every closure or function item coerced to one anywhere ([`Mono::coerce`]).
//! - **Callbacks of outside code**: a call into the standard library calls the closures it is
//!   handed and the trait implementations of the kernel types it is handed, as its model says.
//!
//! A closure's body is read where it is written, as part of its function's instance, with its
//! parameters as every call that may reach it gave them; its calls are its own instance's edges.
//! A body is read again until no local's value grows ([`MAX_ROUNDS`]); a local whose type is not
//! fully known takes in what a call through `&mut` or a method on it may store
//! ([`Walker::feed`]). [`derived`] reads a `#[derive]`d implementation: the same trait's method
//! on each field. [`constants`] reads constant and static initialisers, which run at compile
//! time, only for the closures and functions their values coerce to function pointers.
//!
//! Guarantees:
//!
//! - **Sound for the calls Rust can make, given the standard library's model.** Every call a body
//!   can make into kernel code in some build that ships is an edge by one of the routes above,
//!   or the code is reported as unread ([`Problem`]): a path that resolves nowhere, a macro that
//!   cannot be read, a syntax form the analysis does not know. The assumptions: the standard
//!   library calls kernel code only as [`crate::recursion::stdlib`] models it; the kernel's
//!   naming lints hold (a pattern's identifier that starts with an upper-case letter names a
//!   constant, unit struct or variant, and one that starts with a lower-case letter binds), which
//!   the merge gate's `-D warnings` enforces; and callables an open generic parameter stands for
//!   come from outside the crates that ship, so recursion through them is their caller's.
//! - **Precise where the source is.** A receiver or operand of a known type dispatches only to
//!   what that type can run, so a method that calls a field's method of the same name is not a
//!   cycle with itself, and a generic function's copy for one closure calls only that closure.
//! - **Terminates.** Values only grow, in a lattice of finite height; a body still growing after
//!   [`MAX_ROUNDS`] readings is reported.
//!
//! Method: abstract interpretation of each body over the lattice of [`crate::recursion::ty`]
//! (P. Cousot and R. Cousot, POPL 1977, <https://doi.org/10.1145/512950.512973>), to a fixed
//! point by round-robin iteration; expression, pattern and statement semantics from The Rust
//! Reference (<https://doc.rust-lang.org/reference/expressions.html>,
//! <https://doc.rust-lang.org/reference/patterns.html>,
//! <https://doc.rust-lang.org/reference/statements.html>), including its operator traits
//! (<https://doc.rust-lang.org/reference/expressions/operator-expr.html>), the `?` operator
//! (<https://doc.rust-lang.org/reference/expressions/operator-expr.html#the-question-mark-operator>),
//! `for` loops (<https://doc.rust-lang.org/reference/expressions/loop-expr.html#iterator-loops>),
//! closure coercions (<https://doc.rust-lang.org/reference/type-coercions.html>) and drop scopes
//! (<https://doc.rust-lang.org/reference/destructors.html>).
//!
//! Cost: each reading is linear in the size of the body times the cost of the lookups it makes;
//! at most [`MAX_ROUNDS`] readings per run. The reader recurses over the syntax tree, so its depth
//! is the nesting depth of the source's expressions; [`crate::recursion::analyse`] runs it on a
//! thread with a large stack.

use std::collections::{BTreeMap, BTreeSet};

use quote::ToTokens;
use syn::{BinOp, Expr, Pat, Stmt, UnOp};

use crate::recursion::Problem;
use crate::recursion::index::{Index, Map, Target, how};
use crate::recursion::macros::{self, FmtTrait, Formatted, MacroRead};
use crate::recursion::model::{ItemKind, NodeKind, Scope, line_of};
use crate::recursion::mono::{Dep, InstId, InstKind, Mono, Site};
use crate::recursion::resolve::{Ns, Res, Resolver};
use crate::recursion::stdlib::{self, Model};
use crate::recursion::ty::{ClosureKey, GenId, NodeId, TraitRef, Ty, Val};

/// How many times one run reads a body before a still-growing local is reported.
pub const MAX_ROUNDS: usize = 24;

/// How many instances one call may reach, from generic arguments with several values each, before
/// those values merge into one hidden value.
pub const MAX_COMBINATIONS: usize = 8;

/// The standard traits whose methods a path like `Clone::clone(&x)` calls on its first argument.
pub const RECEIVER_TRAITS: [&str; 44] = [
    "Clone", "PartialEq", "Eq", "PartialOrd", "Ord", "Hash", "Debug", "Display", "Iterator",
    "DoubleEndedIterator", "ExactSizeIterator", "IntoIterator", "Extend", "Into", "TryInto",
    "AsRef", "AsMut", "Borrow", "BorrowMut", "Deref", "DerefMut", "Drop", "Add", "Sub", "Mul",
    "Div", "Rem", "Neg", "Not", "BitAnd", "BitOr", "BitXor", "Shl", "Shr", "Index", "IndexMut",
    "ToString", "ToOwned", "Error", "Write", "AddAssign", "SubAssign", "MulAssign", "DivAssign",
];

/// The standard traits' associated functions that build a value of the type they are called for:
/// trait and function.
pub const STATIC_TRAIT_FNS: [(&str, &str); 7] = [
    ("Default", "default"),
    ("From", "from"),
    ("TryFrom", "try_from"),
    ("FromIterator", "from_iter"),
    ("FromStr", "from_str"),
    ("Sum", "sum"),
    ("Product", "product"),
];

/// The standard methods that build their result with a kernel implementation on the result's
/// type: method, trait and the trait's function.
pub const CONVERSIONS: [(&str, &str, &str); 5] = [
    ("into", "From", "from"),
    ("try_into", "TryFrom", "try_from"),
    ("parse", "FromStr", "from_str"),
    ("unwrap_or_default", "Default", "default"),
    ("to_owned", "Clone", "clone"),
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
        BinOp::Eq(_) => ("PartialEq", "eq"),
        BinOp::Ne(_) => ("PartialEq", "ne"),
        BinOp::Lt(_) => ("PartialOrd", "lt"),
        BinOp::Le(_) => ("PartialOrd", "le"),
        BinOp::Gt(_) => ("PartialOrd", "gt"),
        BinOp::Ge(_) => ("PartialOrd", "ge"),
        _ => return None,
    })
}

/// Whether a value's type is fully known: no hidden part anywhere.
fn exact(v: &Val) -> bool {
    v.0.iter().all(|t| {
        let mut ok = true;
        t.walk(&mut |x| {
            if matches!(x, Ty::Hidden(_)) {
                ok = false;
            }
        });
        ok
    })
}

/// `t` with every hidden part also listing `extra`.
fn widen_holes(t: &Ty, extra: &BTreeSet<Ty>) -> Ty {
    let all = |args: &[Ty]| args.iter().map(|a| widen_holes(a, extra)).collect::<Vec<_>>();
    match t {
        Ty::Hidden(parts) => {
            let mut p = parts.clone();
            p.extend(extra.iter().cloned());
            Ty::Hidden(p)
        }
        Ty::Adt(id, args) => Ty::Adt(*id, all(args)),
        Ty::Ext(n, args) => Ty::Ext(n.clone(), all(args)),
        Ty::Tuple(items) => Ty::Tuple(all(items)),
        Ty::Ref(inner) => Ty::Ref(Box::new(widen_holes(inner, extra))),
        Ty::Slice(inner) => Ty::Slice(Box::new(widen_holes(inner, extra))),
        Ty::Array(inner) => Ty::Array(Box::new(widen_holes(inner, extra))),
        other => other.clone(),
    }
}

/// Whether a type has a return-position `impl Trait` anywhere.
fn has_opaque(t: &Ty) -> bool {
    let mut found = false;
    t.walk(&mut |x| {
        if matches!(x, Ty::Opaque(_)) {
            found = true;
        }
    });
    found
}

/// Whether a declared type holds a closure trait object, and whether it holds a function pointer.
fn erased(t: &Ty) -> (bool, bool) {
    let (mut d, mut f) = (false, false);
    t.walk(&mut |x| match x {
        Ty::Dyn(traits) if traits.iter().any(|tr| tr.fn_arity().is_some()) => d = true,
        Ty::FnPtr(_) => f = true,
        _ => {}
    });
    (d, f)
}

/// The callables among a value's alternatives and parts.
fn callables(v: &Val) -> BTreeSet<Ty> {
    let mut out: BTreeSet<Ty> = v
        .parts()
        .into_iter()
        .filter(|t| matches!(t, Ty::Closure(_) | Ty::FnDef(..)))
        .collect();
    for t in &v.0 {
        if matches!(t.peel(), Ty::Closure(_) | Ty::FnDef(..)) {
            out.insert(t.peel().clone());
        }
    }
    out
}

/// The single-segment name of a path expression, if it is one.
fn ident_of(path: &syn::Path) -> Option<String> {
    if path.leading_colon.is_none() && path.segments.len() == 1 {
        path.segments.first().map(|s| s.ident.to_string())
    } else {
        None
    }
}

/// Whether an identifier starts with an upper-case letter: by the naming lints, a constant, a
/// unit struct or a variant rather than a binding.
fn upper(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_uppercase)
}

/// The reader of one instance's body.
struct Walker<'a, 'p> {
    ix: &'a Index<'p>,
    m: &'a mut Mono,
    problems: &'a mut Vec<Problem>,
    reported: &'a mut BTreeSet<(String, usize, String)>,
    /// The instance whose body is read.
    owner: InstId,
    /// Its node.
    node: NodeId,
    /// Its generic arguments, as listed.
    args: Vec<Ty>,
    /// Its generic arguments, by parameter.
    map: Map,
    /// Where names are looked up.
    scope: Scope,
    /// Its file.
    file: String,
    /// The instance whose edges calls add: the owner, or the closure being read.
    current: InstId,
    /// Every local's value, by binding order.
    locals: Vec<Val>,
    /// Names in scope, innermost last.
    scopes: Vec<BTreeMap<String, usize>>,
    next_local: usize,
    next_closure: usize,
    /// Whether a local or one of the owner's closures' parameters grew this reading.
    changed: bool,
    /// The results being gathered: the function's or each enclosing closure's, with its declared
    /// type.
    rets: Vec<(Val, Option<Ty>)>,
    /// The enclosing loops and labelled blocks: label and break values.
    loops: Vec<(Option<String>, Val)>,
    /// How many macro expansions led here.
    depth: usize,
    /// Whether an or-pattern's later alternatives are being bound (joining into the first's
    /// locals).
    or_join: bool,
    /// Whether edges are left out (a constant initialiser, which runs at compile time).
    compile_time: bool,
    /// The parts of every value bound or returned by a call, for drop glue.
    seen: BTreeSet<Ty>,
}

/// Reads instance `inst`'s body: node `node` with generic arguments `args`.
pub fn body(
    ix: &Index<'_>,
    m: &mut Mono,
    problems: &mut Vec<Problem>,
    reported: &mut BTreeSet<(String, usize, String)>,
    inst: InstId,
    node: NodeId,
    args: &[Ty],
) {
    let Some(n) = ix.program.node(node) else {
        return;
    };
    let Some(block) = n.body.clone() else {
        return;
    };
    let syn_sig = n.sig.clone();
    let mut w = Walker::new(ix, m, problems, reported, inst, node, args, n.scope.clone(), &n.file);
    let sig = ix.sig(node).cloned().unwrap_or_default();
    let declared = ix.normalize(&sig.output.subst(&w.map));
    let mut finished = false;
    for _ in 0..MAX_ROUNDS {
        w.reset();
        if let Some(s) = &syn_sig {
            for (i, input) in s.inputs.iter().enumerate() {
                let t = sig
                    .inputs
                    .get(i)
                    .map_or_else(Ty::unknown, |t| ix.normalize(&t.subst(&w.map)));
                match input {
                    syn::FnArg::Receiver(_) => w.bind("self", &Val::of(t)),
                    syn::FnArg::Typed(pt) => w.bind_pat(&pt.pat, &Val::of(t)),
                }
            }
        }
        w.rets.push((Val::none(), Some(declared.clone())));
        let tail = w.block(&block, Some(&declared));
        let (mut acc, _) = w.rets.pop().unwrap_or_default();
        acc.join(&tail);
        w.m.add_ret(inst, &acc);
        if !w.changed {
            finished = true;
            break;
        }
    }
    if !finished {
        w.problem(n.line, format!("the values of `{}`'s locals still grow after {MAX_ROUNDS} readings", n.name));
    }
    w.drop_glue(n.line);
}

/// Reads a derived implementation's instance: the same trait's method on every field's type.
pub fn derived(
    ix: &Index<'_>,
    m: &mut Mono,
    problems: &mut Vec<Problem>,
    reported: &mut BTreeSet<(String, usize, String)>,
    inst: InstId,
    node: NodeId,
    args: &[Ty],
) {
    let Some(n) = ix.program.node(node) else {
        return;
    };
    let NodeKind::Derived { item, trait_name } = &n.kind else {
        return;
    };
    let (method, receiver) = match trait_name.as_str() {
        "Clone" => ("clone", true),
        "PartialEq" => ("eq", true),
        "PartialOrd" => ("partial_cmp", true),
        "Ord" => ("cmp", true),
        "Hash" => ("hash", true),
        "Debug" => ("fmt", true),
        "Default" => ("default", false),
        _ => return,
    };
    let mut w = Walker::new(ix, m, problems, reported, inst, node, args, n.scope.clone(), &n.file);
    let mut ret = Val::none();
    for f in ix.all_field_types(*item) {
        let f = ix.normalize(&f.subst(&w.map));
        let v = Val::of(f.clone());
        let extra = if matches!(method, "eq" | "partial_cmp" | "cmp") { vec![v.clone()] } else { Vec::new() };
        w.trait_call(&v, trait_name, method, &extra, receiver, n.line, how::DERIVED);
        ret.add(f);
    }
    let own = ix.self_ty(node).map(|t| t.subst(&w.map));
    if let Some(own) = own {
        w.m.add_ret(inst, &Val::of(own));
    }
    w.seen.extend(ret.parts());
}

/// Reads every constant's and static's initialiser for the closures and functions its value
/// coerces to a function pointer or closure trait object; its calls run at compile time.
pub fn constants(
    ix: &Index<'_>,
    m: &mut Mono,
    problems: &mut Vec<Problem>,
    reported: &mut BTreeSet<(String, usize, String)>,
) {
    for item in &ix.program.items {
        let ItemKind::Const { ty, init: Some(init) } = &item.kind else {
            continue;
        };
        let file = ix.program.module(item.module).file.clone();
        let mut w = Walker::new(
            ix,
            m,
            problems,
            reported,
            InstId(usize::MAX),
            NodeId(usize::MAX),
            &[],
            item.scope.clone(),
            &file,
        );
        w.compile_time = true;
        let declared = w.ty(ty);
        let v = w.eval(init, Some(&declared));
        w.coerce(&v, &declared);
    }
}

impl<'a, 'p> Walker<'a, 'p> {
    #[allow(clippy::too_many_arguments, reason = "the reader's shared state, passed once")]
    fn new(
        ix: &'a Index<'p>,
        m: &'a mut Mono,
        problems: &'a mut Vec<Problem>,
        reported: &'a mut BTreeSet<(String, usize, String)>,
        owner: InstId,
        node: NodeId,
        args: &[Ty],
        scope: Scope,
        file: &str,
    ) -> Walker<'a, 'p> {
        let map = if node.0 == usize::MAX {
            Map::new()
        } else {
            ix.map_of(node, args)
        };
        Walker {
            ix,
            m,
            problems,
            reported,
            owner,
            node,
            args: args.to_vec(),
            map,
            scope,
            file: file.to_owned(),
            current: owner,
            locals: Vec::new(),
            scopes: vec![BTreeMap::new()],
            next_local: 0,
            next_closure: 0,
            changed: false,
            rets: Vec::new(),
            loops: Vec::new(),
            depth: 0,
            or_join: false,
            compile_time: false,
            seen: BTreeSet::new(),
        }
    }

    fn reset(&mut self) {
        self.scopes = vec![BTreeMap::new()];
        self.next_local = 0;
        self.next_closure = 0;
        self.changed = false;
        self.rets.clear();
        self.loops.clear();
        self.current = self.owner;
    }

    fn resolver(&self) -> &Resolver<'p> {
        &self.ix.resolver
    }

    /// A written type, in this instance.
    fn ty(&self, t: &syn::Type) -> Ty {
        self.ix.normalize(&self.resolver().ty(&self.scope, t).subst(&self.map))
    }

    fn problem(&mut self, line: usize, what: String) {
        let key = (self.file.clone(), line, what.clone());
        if self.reported.insert(key) {
            self.problems.push(Problem::unread(&self.file, line, &what));
        }
    }

    fn edge(&mut self, to: InstId, line: usize, how_found: &'static str) {
        if self.compile_time {
            return;
        }
        let site = Site {
            file: self.file.clone(),
            line,
            how: how_found,
        };
        self.m.edge(self.current, to, &site);
    }

    // ------------------------------------------------------------ locals

    fn bind(&mut self, name: &str, v: &Val) {
        self.seen.extend(v.parts());
        if self.or_join
            && let Some(idx) = self.scopes.last().and_then(|s| s.get(name)).copied()
        {
            if let Some(l) = self.locals.get_mut(idx)
                && l.join(v)
            {
                self.changed = true;
            }
            return;
        }
        let idx = self.next_local;
        self.next_local = self.next_local.saturating_add(1);
        if idx >= self.locals.len() {
            self.locals.push(v.clone());
            if !v.is_none() {
                self.changed = true;
            }
        } else if let Some(l) = self.locals.get_mut(idx)
            && l.join(v)
        {
            self.changed = true;
        }
        if let Some(s) = self.scopes.last_mut() {
            s.insert(name.to_owned(), idx);
        }
    }

    fn local(&self, name: &str) -> Option<usize> {
        self.scopes.iter().rev().find_map(|s| s.get(name).copied())
    }

    fn assign_local(&mut self, idx: usize, v: &Val) {
        if let Some(l) = self.locals.get_mut(idx)
            && l.join(v)
        {
            self.changed = true;
        }
    }

    /// The local a place expression is rooted at.
    fn root_local(&self, e: &Expr) -> Option<usize> {
        match e {
            Expr::Path(p) if p.qself.is_none() => ident_of(&p.path).and_then(|n| self.local(&n)),
            Expr::Field(f) => self.root_local(&f.base),
            Expr::Index(i) => self.root_local(&i.expr),
            Expr::Paren(p) => self.root_local(&p.expr),
            Expr::Group(g) => self.root_local(&g.expr),
            Expr::Reference(r) => self.root_local(&r.expr),
            Expr::Unary(u) if matches!(u.op, UnOp::Deref(_)) => self.root_local(&u.expr),
            _ => None,
        }
    }

    /// A local whose type is not fully known takes in the parts of `v`: a method call on it or a
    /// call handed `&mut` it may store them in it.
    fn feed(&mut self, place: &Expr, v: &Val) {
        let Some(idx) = self.root_local(place) else {
            return;
        };
        let Some(cur) = self.locals.get(idx).cloned() else {
            return;
        };
        if exact(&cur) {
            return;
        }
        let extra: BTreeSet<Ty> = v.parts().iter().map(Ty::shallow).collect();
        if extra.is_empty() {
            return;
        }
        let mut next = Val::none();
        for t in &cur.0 {
            next.add(widen_holes(t, &extra));
        }
        self.assign_local(idx, &next);
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

    // ------------------------------------------------------------ statements and blocks

    fn block(&mut self, b: &syn::Block, expected: Option<&Ty>) -> Val {
        self.push_scope();
        let v = self.stmts(&b.stmts, expected);
        self.pop_scope();
        v
    }

    fn stmts(&mut self, stmts: &[Stmt], expected: Option<&Ty>) -> Val {
        let mut last = Val::unit();
        let n = stmts.len();
        for (i, s) in stmts.iter().enumerate() {
            let tail = i.saturating_add(1) == n;
            last = match s {
                Stmt::Local(l) => {
                    self.local_stmt(l);
                    Val::unit()
                }
                Stmt::Item(_) => Val::unit(),
                Stmt::Expr(e, semi) => {
                    let v = self.eval(e, if tail && semi.is_none() { expected } else { None });
                    if semi.is_some() {
                        if v.is_none() { Val::none() } else { Val::unit() }
                    } else {
                        v
                    }
                }
                Stmt::Macro(sm) => {
                    let v = self.mac(&sm.mac, if tail { expected } else { None });
                    if sm.semi_token.is_some() && !v.is_none() { Val::unit() } else { v }
                }
            };
        }
        last
    }

    fn local_stmt(&mut self, l: &syn::Local) {
        let declared = match &l.pat {
            Pat::Type(pt) => Some(self.ty(&pt.ty)),
            _ => None,
        };
        let v = match &l.init {
            Some(init) => {
                let v = self.eval(&init.expr, declared.as_ref());
                if let Some((_, diverge)) = &init.diverge {
                    self.eval(diverge, None);
                }
                v
            }
            None => Val::none(),
        };
        let bound = match &declared {
            Some(d) if !d.has_infer() && !matches!(d, Ty::Hidden(_)) => {
                self.coerce(&v, d);
                Val::of(d.clone())
            }
            _ => v,
        };
        match &l.pat {
            Pat::Type(pt) => self.bind_pat(&pt.pat, &bound),
            other => self.bind_pat(other, &bound),
        }
    }

    // ------------------------------------------------------------ patterns

    fn bind_pat(&mut self, pat: &Pat, v: &Val) {
        match pat {
            Pat::Ident(pi) => {
                let name = pi.ident.to_string();
                if pi.subpat.is_none() && upper(&name) {
                    return;
                }
                self.bind(&name, v);
                if let Some((_, sub)) = &pi.subpat {
                    self.bind_pat(sub, v);
                }
            }
            Pat::Tuple(pt) => {
                let n = pt.elems.len();
                let rest = pt.elems.iter().position(|p| matches!(p, Pat::Rest(_)));
                for (i, p) in pt.elems.iter().enumerate() {
                    if matches!(p, Pat::Rest(_)) {
                        continue;
                    }
                    let mut ev = Val::none();
                    for alt in v.peeled() {
                        match &alt {
                            Ty::Tuple(items) => {
                                let idx = match rest {
                                    Some(r) if i > r => items.len().checked_sub(n.saturating_sub(i)),
                                    _ => Some(i),
                                };
                                match idx.and_then(|j| items.get(j)) {
                                    Some(t) => ev.add(t.clone()),
                                    None => ev.add(Ty::Hidden(alt.parts())),
                                };
                            }
                            other => {
                                ev.add(Ty::Hidden(other.parts()));
                            }
                        }
                    }
                    self.bind_pat(p, &ev);
                }
            }
            Pat::TupleStruct(ts) => {
                let fields = self.pattern_fields(&ts.path, v);
                let n = ts.elems.len();
                let rest = ts.elems.iter().position(|p| matches!(p, Pat::Rest(_)));
                for (i, p) in ts.elems.iter().enumerate() {
                    if matches!(p, Pat::Rest(_)) {
                        continue;
                    }
                    let name = match rest {
                        Some(r) if i > r => {
                            let total = fields.iter().map(BTreeMap::len).max().unwrap_or(0);
                            total.checked_sub(n.saturating_sub(i)).map(|j| j.to_string())
                        }
                        _ => Some(i.to_string()),
                    };
                    let fv = self.field_val(&fields, name.as_deref(), v);
                    self.bind_pat(p, &fv);
                }
            }
            Pat::Struct(ps) => {
                let fields = self.pattern_fields(&ps.path, v);
                for fp in &ps.fields {
                    let name = match &fp.member {
                        syn::Member::Named(id) => id.to_string(),
                        syn::Member::Unnamed(ix) => ix.index.to_string(),
                    };
                    let fv = self.field_val(&fields, Some(&name), v);
                    self.bind_pat(&fp.pat, &fv);
                }
            }
            Pat::Slice(ps) => {
                let mut elem = Val::none();
                for alt in v.peeled() {
                    match stdlib::item_of(&alt) {
                        Some(t) => elem.add(t),
                        None => elem.add(Ty::Hidden(alt.parts())),
                    };
                }
                for p in &ps.elems {
                    match p {
                        Pat::Rest(_) => {}
                        Pat::Ident(pi) if matches!(pi.subpat.as_ref().map(|(_, s)| &**s), Some(Pat::Rest(_))) => {
                            let mut sl = Val::none();
                            for t in &elem.0 {
                                sl.add(Ty::Slice(Box::new(t.clone())));
                            }
                            self.bind(&pi.ident.to_string(), &sl);
                        }
                        other => self.bind_pat(other, &elem),
                    }
                }
            }
            Pat::Reference(pr) => {
                let mut inner = Val::none();
                for t in &v.0 {
                    inner.add(match t {
                        Ty::Ref(i) => (**i).clone(),
                        other => other.clone(),
                    });
                }
                self.bind_pat(&pr.pat, &inner);
            }
            Pat::Or(po) => {
                let saved = self.or_join;
                for (i, case) in po.cases.iter().enumerate() {
                    self.or_join = saved || i > 0;
                    self.bind_pat(case, v);
                }
                self.or_join = saved;
            }
            Pat::Type(pt) => {
                let d = self.ty(&pt.ty);
                let bound = if d.has_infer() { v.clone() } else { Val::of(d) };
                self.bind_pat(&pt.pat, &bound);
            }
            Pat::Paren(pp) => self.bind_pat(&pp.pat, v),
            Pat::Guard(pg) => {
                self.bind_pat(&pg.pat, v);
                self.eval(&pg.guard, None);
            }
            Pat::Macro(pm) => {
                let line = line_of(pm);
                self.problem(line, "a macro in a pattern, which the recursion check does not expand".to_owned());
            }
            Pat::Verbatim(_) => {
                let line = line_of(pat);
                self.problem(line, "a pattern syn keeps as tokens, which the recursion check cannot read".to_owned());
            }
            Pat::Const(pc) => {
                self.block(&pc.block, None);
            }
            _ => {}
        }
    }

    /// The fields a struct or variant pattern's path names, for each alternative of the
    /// scrutinee: field name to type, with the alternative's generic arguments put in.
    fn pattern_fields(&mut self, path: &syn::Path, v: &Val) -> Vec<BTreeMap<String, Ty>> {
        let segs = Resolver::segments(path);
        let mut res = self.resolver().resolve(&self.scope, &segs, path.leading_colon.is_some(), Ns::Value);
        if res.is_empty() {
            res = self.resolver().resolve(&self.scope, &segs, path.leading_colon.is_some(), Ns::Type);
        }
        let mut out = Vec::new();
        for r in res {
            let (item, variant) = match r {
                Res::Variant(item, v) => (item, Some(v)),
                Res::Item(item) if self.ix.program.item(item).is_adt() => (item, None),
                Res::SelfTy => match self.self_type().map(|t| t.peel().clone()) {
                    Some(Ty::Adt(item, _)) => (item, None),
                    _ => continue,
                },
                Res::Ext(p) => {
                    let name = p.last().cloned().unwrap_or_default();
                    for alt in v.peeled() {
                        let mut m = BTreeMap::new();
                        match (&alt, name.as_str()) {
                            (Ty::Ext(o, args), "Some") if o == "Option" => {
                                m.insert("0".to_owned(), args.first().cloned().unwrap_or_else(Ty::unknown));
                            }
                            (Ty::Ext(o, args), "Ok") if o == "Result" => {
                                m.insert("0".to_owned(), args.first().cloned().unwrap_or_else(Ty::unknown));
                            }
                            (Ty::Ext(o, args), "Err") if o == "Result" => {
                                m.insert("0".to_owned(), args.get(1).cloned().unwrap_or_else(Ty::unknown));
                            }
                            _ => {
                                m.insert("*".to_owned(), Ty::Hidden(alt.parts()));
                            }
                        }
                        out.push(m);
                    }
                    continue;
                }
                _ => continue,
            };
            let generics = self.ix.program.item(item).scope.generics.clone();
            let fields = self.ix.fields(item, variant);
            let mut matched = false;
            for alt in v.peeled() {
                let map: Map = match &alt {
                    Ty::Adt(id, args) if *id == item => generics.iter().copied().zip(args.iter().cloned()).collect(),
                    Ty::Hidden(_) | Ty::Param(_) => generics
                        .iter()
                        .map(|g| (*g, Ty::Hidden(alt.parts())))
                        .collect(),
                    _ => continue,
                };
                matched = true;
                out.push(fields.iter().map(|(n, t)| (n.clone(), self.ix.normalize(&t.subst(&map)))).collect());
            }
            if !matched {
                let open: Map = generics.iter().map(|g| (*g, Ty::unknown())).collect();
                out.push(fields.iter().map(|(n, t)| (n.clone(), self.ix.normalize(&t.subst(&open)))).collect());
            }
        }
        out
    }

    /// The value of field `name` across the alternatives' field tables; a hidden value of the
    /// scrutinee when no table has it.
    fn field_val(&self, fields: &[BTreeMap<String, Ty>], name: Option<&str>, v: &Val) -> Val {
        let mut out = Val::none();
        for f in fields {
            match name.and_then(|n| f.get(n)).or_else(|| f.get("*")) {
                Some(t) => {
                    out.add(t.clone());
                }
                None => {
                    out.add(Ty::Hidden(v.parts()));
                }
            }
        }
        if fields.is_empty() {
            out.add(Ty::Hidden(v.parts()));
        }
        out
    }

    fn self_type(&self) -> Option<Ty> {
        if self.node.0 == usize::MAX {
            return None;
        }
        self.ix.self_ty(self.node).map(|t| self.ix.normalize(&t.subst(&self.map)))
    }

    // ------------------------------------------------------------ expressions

    #[allow(clippy::too_many_lines, reason = "one arm per expression form, each short")]
    fn eval(&mut self, e: &Expr, expected: Option<&Ty>) -> Val {
        let line = line_of(e);
        match e {
            Expr::Lit(l) => lit(&l.lit, expected),
            Expr::Path(p) => self.path_value(p, line),
            Expr::Call(c) => self.call(c, expected, line),
            Expr::MethodCall(mc) => {
                let recv = self.eval(&mc.receiver, None);
                let args: Vec<Val> = mc.args.iter().map(|a| self.eval(a, None)).collect();
                let turbofish: Vec<Ty> = mc
                    .turbofish
                    .as_ref()
                    .map(|t| {
                        t.args
                            .iter()
                            .filter_map(|a| match a {
                                syn::GenericArgument::Type(ty) => Some(self.ty(ty)),
                                _ => None,
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let name = mc.method.to_string();
                let out = self.method_call(&recv, &name, &args, &turbofish, expected, line);
                let mut fed = Val::none();
                for a in &args {
                    fed.join(a);
                }
                self.feed(&mc.receiver, &fed);
                for a in &mc.args {
                    if matches!(a, Expr::Reference(r) if r.mutability.is_some()) {
                        self.feed(a, &fed);
                    }
                }
                out
            }
            Expr::Field(f) => {
                let base = self.eval(&f.base, None);
                let name = match &f.member {
                    syn::Member::Named(id) => id.to_string(),
                    syn::Member::Unnamed(ix) => ix.index.to_string(),
                };
                let mut out = Val::none();
                for alt in &base.0 {
                    for t in self.ix.field(alt, &name) {
                        out.add(self.ix.normalize(&t));
                    }
                }
                out
            }
            Expr::Index(i) => {
                let base = self.eval(&i.expr, None);
                let idx = self.eval(&i.index, None);
                self.index(&base, &idx, line)
            }
            Expr::Binary(b) => self.binary(b, line),
            Expr::Unary(u) => {
                let v = self.eval(&u.expr, None);
                match u.op {
                    UnOp::Deref(_) => self.deref(&v, line),
                    UnOp::Not(_) => self.operator(&v, &Val::none(), "Not", "not", false, line),
                    UnOp::Neg(_) => self.operator(&v, &Val::none(), "Neg", "neg", false, line),
                    _ => Val::unknown(),
                }
            }
            Expr::Cast(c) => {
                let v = self.eval(&c.expr, None);
                let t = self.ty(&c.ty);
                self.coerce(&v, &t);
                if t.has_infer() { v } else { Val::of(t) }
            }
            Expr::Paren(p) => self.eval(&p.expr, expected),
            Expr::Group(g) => self.eval(&g.expr, expected),
            Expr::Reference(r) => {
                let inner = expected.and_then(|t| match t {
                    Ty::Ref(i) => Some((**i).clone()),
                    _ => None,
                });
                let v = self.eval(&r.expr, inner.as_ref());
                let mut out = Val::none();
                for t in &v.0 {
                    out.add(Ty::Ref(Box::new(t.clone())));
                }
                out
            }
            Expr::RawAddr(r) => {
                let v = self.eval(&r.expr, None);
                let mut out = Val::none();
                for t in &v.0 {
                    out.add(Ty::Ref(Box::new(t.clone())));
                }
                out
            }
            Expr::Block(b) => self.labelled_block(b.label.as_ref(), &b.block, expected),
            Expr::Unsafe(u) => self.block(&u.block, expected),
            Expr::Const(c) => {
                let saved = self.compile_time;
                self.compile_time = true;
                let v = self.block(&c.block, expected);
                self.compile_time = saved;
                v
            }
            Expr::If(i) => {
                self.push_scope();
                self.eval(&i.cond, None);
                let then = self.block(&i.then_branch, expected);
                self.pop_scope();
                match &i.else_branch {
                    Some((_, els)) => {
                        let ev = self.eval(els, expected);
                        then.or(&ev)
                    }
                    None => Val::unit(),
                }
            }
            Expr::Let(l) => {
                let v = self.eval(&l.expr, None);
                self.bind_pat(&l.pat, &v);
                Val::prim("bool")
            }
            Expr::Match(mt) => {
                let v = self.eval(&mt.expr, None);
                let mut out = Val::none();
                for arm in &mt.arms {
                    self.push_scope();
                    self.bind_pat(&arm.pat, &v);
                    let b = self.eval(&arm.body, expected);
                    out.join(&b);
                    self.pop_scope();
                }
                out
            }
            Expr::Loop(l) => {
                let label = l.label.as_ref().map(|x| x.name.ident.to_string());
                self.loops.push((label, Val::none()));
                self.block(&l.body, None);
                self.loops.pop().map(|(_, v)| v).unwrap_or_default()
            }
            Expr::While(wh) => {
                let label = wh.label.as_ref().map(|x| x.name.ident.to_string());
                self.loops.push((label, Val::none()));
                self.push_scope();
                self.eval(&wh.cond, None);
                self.block(&wh.body, None);
                self.pop_scope();
                self.loops.pop();
                Val::unit()
            }
            Expr::ForLoop(f) => {
                let it = self.eval(&f.expr, None);
                let items = self.iterate(&it, line);
                let label = f.label.as_ref().map(|x| x.name.ident.to_string());
                self.loops.push((label, Val::none()));
                self.push_scope();
                self.bind_pat(&f.pat, &items);
                self.block(&f.body, None);
                self.pop_scope();
                self.loops.pop();
                Val::unit()
            }
            Expr::Break(b) => {
                let v = match &b.expr {
                    Some(x) => self.eval(x, None),
                    None => Val::unit(),
                };
                let label = b.label.as_ref().map(|l| l.ident.to_string());
                let target = match &label {
                    Some(name) => self.loops.iter_mut().rev().find(|(l, _)| l.as_deref() == Some(name.as_str())),
                    None => self.loops.iter_mut().rev().find(|_| true),
                };
                if let Some((_, acc)) = target {
                    acc.join(&v);
                }
                Val::none()
            }
            Expr::Continue(_) => Val::none(),
            Expr::Return(r) => {
                let declared = self.rets.last().and_then(|(_, d)| d.clone());
                let v = match &r.expr {
                    Some(x) => self.eval(x, declared.as_ref()),
                    None => Val::unit(),
                };
                if let Some(d) = &declared {
                    self.coerce(&v, d);
                }
                if let Some((acc, _)) = self.rets.last_mut() {
                    acc.join(&v);
                }
                Val::none()
            }
            Expr::Closure(c) => self.closure(c, line),
            Expr::Struct(s) => self.struct_lit(s, line),
            Expr::Tuple(t) => {
                let want: Vec<Option<Ty>> = match expected {
                    Some(Ty::Tuple(items)) => items.iter().cloned().map(Some).collect(),
                    _ => Vec::new(),
                };
                let vals: Vec<Val> = t
                    .elems
                    .iter()
                    .enumerate()
                    .map(|(i, x)| self.eval(x, want.get(i).and_then(Option::as_ref)))
                    .collect();
                if vals.is_empty() {
                    return Val::unit();
                }
                Val::of(Ty::Tuple(vals.iter().map(stdlib::one).collect()))
            }
            Expr::Array(a) => {
                let mut elem = Val::none();
                for x in &a.elems {
                    let v = self.eval(x, None);
                    elem.join(&v);
                }
                Val::of(Ty::Array(Box::new(stdlib::one(&elem))))
            }
            Expr::Repeat(r) => {
                let v = self.eval(&r.expr, None);
                self.eval(&r.len, None);
                Val::of(Ty::Array(Box::new(stdlib::one(&v))))
            }
            Expr::Range(r) => {
                let mut v = Val::none();
                if let Some(s) = &r.start {
                    let x = self.eval(s, None);
                    v.join(&x);
                }
                if let Some(x) = &r.end {
                    let y = self.eval(x, None);
                    v.join(&y);
                }
                Val::of(Ty::ext("Range", vec![stdlib::one(&v)]))
            }
            Expr::Try(t) => {
                let v = self.eval(&t.expr, None);
                self.try_op(&v, line)
            }
            Expr::Macro(m) => self.mac(&m.mac, expected),
            Expr::Assign(a) => {
                let v = self.eval(&a.right, None);
                let place = self.eval(&a.left, None);
                if let Expr::Path(p) = &*a.left
                    && let Some(idx) = ident_of(&p.path).and_then(|n| self.local(&n))
                {
                    self.assign_local(idx, &v);
                } else {
                    self.feed(&a.left, &v);
                }
                for t in &place.0 {
                    self.coerce(&v, t);
                }
                Val::unit()
            }
            Expr::Infer(_) => Val::unknown(),
            Expr::Async(_) | Expr::Await(_) | Expr::Yield(_) | Expr::TryBlock(_) | Expr::Verbatim(_) => {
                self.problem(
                    line,
                    format!(
                        "an expression the recursion check cannot read: `{}`",
                        macros::excerpt(&e.to_token_stream())
                    ),
                );
                Val::unknown()
            }
            _ => {
                self.problem(
                    line,
                    format!(
                        "an expression form the recursion check does not know: `{}`",
                        macros::excerpt(&e.to_token_stream())
                    ),
                );
                Val::unknown()
            }
        }
    }

    fn labelled_block(&mut self, label: Option<&syn::Label>, b: &syn::Block, expected: Option<&Ty>) -> Val {
        match label {
            Some(l) => {
                self.loops.push((Some(l.name.ident.to_string()), Val::none()));
                let v = self.block(b, expected);
                let (_, brk) = self.loops.pop().unwrap_or_default();
                v.or(&brk)
            }
            None => self.block(b, expected),
        }
    }

    // ------------------------------------------------------------ paths

    /// The value a path expression names.
    fn path_value(&mut self, p: &syn::ExprPath, line: usize) -> Val {
        if let Some(q) = &p.qself {
            let base = self.ty(&q.ty);
            let name = p.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
            return self.assoc_value(&base, &name, None);
        }
        if let Some(name) = ident_of(&p.path)
            && let Some(idx) = self.local(&name)
        {
            return self.locals.get(idx).cloned().unwrap_or_default();
        }
        let segs = Resolver::segments(&p.path);
        let res = self.resolver().resolve(&self.scope, &segs, p.path.leading_colon.is_some(), Ns::Value);
        if res.is_empty() {
            self.problem(
                line,
                format!("a path that resolves to nothing: `{}`", p.path.to_token_stream()),
            );
            return Val::unknown();
        }
        let mut out = Val::none();
        for r in res {
            let v = self.res_value(&r, &p.path, line);
            out.join(&v);
        }
        out
    }

    /// The value a single name denotes: a local, or what it resolves to.
    fn name_value(&mut self, name: &str, line: usize) -> Val {
        if let Some(idx) = self.local(name) {
            return self.locals.get(idx).cloned().unwrap_or_default();
        }
        match syn::parse_str::<syn::Path>(name) {
            Ok(path) => {
                let ep = syn::ExprPath {
                    attrs: Vec::new(),
                    qself: None,
                    path,
                };
                self.path_value(&ep, line)
            }
            Err(_) => Val::unknown(),
        }
    }

    fn res_value(&mut self, r: &Res, path: &syn::Path, line: usize) -> Val {
        let last = path.segments.last();
        match r {
            Res::Fn(n) => {
                let mut fixed = Map::new();
                if let Some(seg) = last {
                    let turbofish = self.resolver().segment_args(&self.scope, seg);
                    let turbofish: Vec<Ty> = turbofish.iter().map(|t| self.ix.normalize(&t.subst(&self.map))).collect();
                    self.turbofish(*n, &turbofish, &mut fixed);
                }
                let args: Vec<Ty> = self
                    .ix
                    .node_generics(*n)
                    .iter()
                    .map(|g| fixed.get(g).cloned().unwrap_or(Ty::Param(*g)))
                    .collect();
                Val::of(Ty::FnDef(*n, args))
            }
            Res::Variant(item, v) => {
                let fields = self.ix.fields(*item, Some(*v));
                let adt = self.adt_unknown(*item);
                if fields.is_empty() {
                    Val::of(adt)
                } else {
                    Val::of(Ty::ext("Ctor", vec![adt]))
                }
            }
            Res::Item(id) => {
                let item = self.ix.program.item(*id);
                match &item.kind {
                    ItemKind::Const { ty, .. } => {
                        let t = self.ix.normalize(&self.resolver().ty(&item.scope, ty));
                        Val::of(t)
                    }
                    ItemKind::Struct { fields, .. } => {
                        if fields.all().is_empty() {
                            Val::of(self.adt_unknown(*id))
                        } else {
                            Val::of(Ty::ext("Ctor", vec![self.adt_unknown(*id)]))
                        }
                    }
                    _ => Val::unknown(),
                }
            }
            Res::Assoc(base, name) => {
                let base_args = path
                    .segments
                    .iter()
                    .rev()
                    .nth(1)
                    .map(|s| self.resolver().segment_args(&self.scope, s))
                    .unwrap_or_default();
                let base_ty = self.base_ty(base, base_args);
                let turbofish: Vec<Ty> = last
                    .map(|s| self.resolver().segment_args(&self.scope, s))
                    .unwrap_or_default();
                self.assoc_value(&base_ty, name, Some(&turbofish))
            }
            Res::Ext(segs) => {
                let name = segs.last().map(String::as_str).unwrap_or_default();
                let qualifier = segs.iter().rev().nth(1).map(String::as_str);
                match (qualifier, name) {
                    (_, "None") => Val::of(Ty::ext("Option", vec![Ty::unknown()])),
                    (Some(q), _) if crate::recursion::ty::PRIMITIVES.contains(&q) && upper(name) => {
                        Val::prim(q)
                    }
                    _ if upper(name) => Val::unknown(),
                    _ => Val::of(Ty::ExtFn),
                }
            }
            Res::ConstParam(_) => Val::prim("usize"),
            Res::SelfTy => self.self_type().map_or_else(Val::unknown, Val::of),
            Res::Generic(_) | Res::Mod(_) | Res::Prim(_) | Res::Macro(_) | Res::Qualified(..) => {
                self.problem(line, format!("a path that names no value: `{}`", path.to_token_stream()));
                Val::unknown()
            }
        }
    }

    /// Kernel type `item` with its arguments unknown.
    fn adt_unknown(&self, item: crate::recursion::ty::ItemId) -> Ty {
        Ty::Adt(
            item,
            self.ix.program.item(item).scope.generics.iter().map(|_| Ty::unknown()).collect(),
        )
    }

    /// The type an associated path's base names, in this instance, with written arguments
    /// `args` (missing ones left to inference).
    fn base_ty(&self, base: &Res, args: Vec<Ty>) -> Ty {
        let t = match base {
            Res::Generic(g) => self.map.get(g).cloned().unwrap_or(Ty::Param(*g)),
            Res::SelfTy => self.self_type().unwrap_or_else(Ty::unknown),
            other => self.resolver().res_ty(&self.scope, other, args),
        };
        self.ix.normalize(&t.subst(&self.map))
    }

    /// `Type::name` as a value: the kernel functions it may be, as function items; an outside
    /// function otherwise.
    fn assoc_value(&mut self, base: &Ty, name: &str, turbofish: Option<&[Ty]>) -> Val {
        let found = self.ix.assoc(base, name);
        let mut out = Val::none();
        for t in &found.targets {
            let mut fixed = t.map.clone();
            if let Some(tf) = turbofish {
                let tf: Vec<Ty> = tf.iter().map(|x| self.ix.normalize(&x.subst(&self.map))).collect();
                self.turbofish(t.node, &tf, &mut fixed);
            }
            let args: Vec<Ty> = self
                .ix
                .node_generics(t.node)
                .iter()
                .map(|g| fixed.get(g).cloned().unwrap_or(Ty::Param(*g)))
                .collect();
            out.add(Ty::FnDef(t.node, args));
        }
        if !found.outside.is_empty() {
            if upper(name) {
                // An associated constant of an outside type: a value of that type, or of a
                // primitive.
                out.add(Ty::Hidden(base.parts()));
                if let Ty::Prim(_) = base {
                    out.add(base.clone());
                }
            } else {
                out.add(Ty::ExtFn);
            }
        }
        if out.is_none() {
            out.add(Ty::Hidden(base.parts()));
        }
        out
    }

    /// Fixes node `node`'s own generic parameters (not its implementation's or trait's) from a
    /// turbofish.
    fn turbofish(&self, node: NodeId, turbofish: &[Ty], fixed: &mut Map) {
        if turbofish.is_empty() {
            return;
        }
        let Some(n) = self.ix.program.node(node) else {
            return;
        };
        let generics = self.ix.node_generics(node);
        let outer = match &n.kind {
            NodeKind::Method(imp) => self.ix.impl_generics(*imp).len(),
            NodeKind::TraitDefault(t) => self.ix.program.item(*t).scope.generics.len().saturating_add(1),
            NodeKind::Fn | NodeKind::Derived { .. } => 0,
        };
        for (g, t) in generics.iter().skip(outer).zip(turbofish) {
            if !t.has_infer() {
                fixed.insert(*g, t.clone());
            }
        }
    }

    // ------------------------------------------------------------ calls

    /// A call expression `f(args)`.
    fn call(&mut self, c: &syn::ExprCall, expected: Option<&Ty>, line: usize) -> Val {
        let local_callee = matches!(&*c.func, Expr::Path(p)
            if p.qself.is_none() && ident_of(&p.path).is_some_and(|n| self.local(&n).is_some()));
        let callee = match &*c.func {
            Expr::Path(_) if !local_callee => None,
            other => Some(self.eval(other, None)),
        };
        let args: Vec<Val> = c.args.iter().map(|a| self.eval(a, None)).collect();
        let out = match (&*c.func, callee) {
            (_, Some(callee)) => self.call_values(&callee, &args, line),
            (Expr::Path(p), None) => self.path_call(p, &args, expected, line),
            (_, None) => Val::unknown(),
        };
        let mut fed = Val::none();
        for a in &args {
            fed.join(a);
        }
        for a in &c.args {
            if matches!(a, Expr::Reference(r) if r.mutability.is_some()) {
                self.feed(a, &fed);
            }
        }
        out
    }

    /// Calls through every alternative of a callee value.
    fn call_values(&mut self, callee: &Val, args: &[Val], line: usize) -> Val {
        let mut out = Val::none();
        for alt in &callee.0 {
            let v = self.call_value(alt, args, line, how::VALUE, true);
            out.join(&v);
        }
        out
    }

    /// A call by path.
    #[allow(clippy::too_many_lines, reason = "one arm per kind of callee a path may name")]
    fn path_call(&mut self, p: &syn::ExprPath, args: &[Val], expected: Option<&Ty>, line: usize) -> Val {
        if let Some(q) = &p.qself {
            let base = self.ty(&q.ty);
            let name = p.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
            let trait_name = (q.position > 0)
                .then(|| p.path.segments.iter().nth(q.position.saturating_sub(1)))
                .flatten()
                .map(|s| s.ident.to_string());
            return self.assoc_call(&base, &name, trait_name.as_deref(), args, &[], expected, line);
        }
        let segs = Resolver::segments(&p.path);
        let res = self.resolver().resolve(&self.scope, &segs, p.path.leading_colon.is_some(), Ns::Value);
        if res.is_empty() {
            self.problem(
                line,
                format!("a call to a path that resolves to nothing: `{}`", p.path.to_token_stream()),
            );
            return Val::unknown();
        }
        let last = p.path.segments.last();
        let turbofish: Vec<Ty> = last
            .map(|s| self.resolver().segment_args(&self.scope, s))
            .unwrap_or_default()
            .iter()
            .map(|t| self.ix.normalize(&t.subst(&self.map)))
            .collect();
        let mut out = Val::none();
        for r in res {
            let v = match &r {
                Res::Fn(n) => {
                    let mut fixed = Map::new();
                    self.turbofish(*n, &turbofish, &mut fixed);
                    self.call_node(*n, &fixed, args, false, line, how::PATH)
                }
                Res::Variant(item, v) => self.construct(*item, Some(*v), args),
                Res::Item(id) => match &self.ix.program.item(*id).kind {
                    ItemKind::Struct { .. } => self.construct(*id, None, args),
                    ItemKind::Const { .. } => {
                        let callee = self.res_value(&r, &p.path, line);
                        self.call_values(&callee, args, line)
                    }
                    _ => Val::unknown(),
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
                        self.kernel_trait_call(*t, name, args, expected, line)
                    } else {
                        let base_ty = self.base_ty(base, base_args);
                        self.assoc_call(&base_ty, name, None, args, &turbofish, expected, line)
                    }
                }
                Res::Ext(segs) => self.ext_call(segs, args, &turbofish, expected, line),
                Res::SelfTy => {
                    // `Self(a, b)`: a tuple struct's constructor.
                    match self.self_type().map(|t| t.peel().clone()) {
                        Some(Ty::Adt(item, _)) => self.construct(item, None, args),
                        _ => Val::unknown(),
                    }
                }
                _ => {
                    self.problem(
                        line,
                        format!("a call to something that is not a function: `{}`", p.path.to_token_stream()),
                    );
                    Val::unknown()
                }
            };
            out.join(&v);
        }
        out
    }

    /// A struct's or variant's constructor called with `args`.
    fn construct(&mut self, item: crate::recursion::ty::ItemId, variant: Option<usize>, args: &[Val]) -> Val {
        let generics: BTreeSet<GenId> = self.ix.program.item(item).scope.generics.iter().copied().collect();
        let fields = self.ix.fields(item, variant);
        let mut found = BTreeMap::new();
        for ((_, t), a) in fields.iter().zip(args) {
            for alt in &a.0 {
                self.ix.unify(t, alt, &generics, &mut found);
            }
            self.coerce(a, t);
        }
        let map = crate::recursion::index::single(&found, &generics);
        let list: Vec<Ty> = self
            .ix
            .program
            .item(item)
            .scope
            .generics
            .iter()
            .map(|g| match map.get(g) {
                Some(Ty::Param(x)) if x == g => Ty::unknown(),
                Some(t) => t.clone(),
                None => Ty::unknown(),
            })
            .collect();
        Val::of(Ty::Adt(item, list))
    }

    /// A struct literal.
    fn struct_lit(&mut self, s: &syn::ExprStruct, line: usize) -> Val {
        let segs = Resolver::segments(&s.path);
        let mut res = self.resolver().resolve(&self.scope, &segs, s.path.leading_colon.is_some(), Ns::Type);
        res.extend(self.resolver().resolve(&self.scope, &segs, s.path.leading_colon.is_some(), Ns::Value));
        let target = res.iter().find_map(|r| match r {
            Res::Variant(item, v) => Some((*item, Some(*v))),
            Res::Item(item) if self.ix.program.item(*item).is_adt() => Some((*item, None)),
            Res::SelfTy => match self.self_type().map(|t| t.peel().clone()) {
                Some(Ty::Adt(item, _)) => Some((item, None)),
                _ => None,
            },
            _ => None,
        });
        let Some((item, variant)) = target else {
            for f in &s.fields {
                self.eval(&f.expr, None);
            }
            if let Some(r) = &s.rest {
                self.eval(r, None);
            }
            self.problem(line, format!("a struct literal of an unknown type: `{}`", s.path.to_token_stream()));
            return Val::unknown();
        };
        let fields = self.ix.fields(item, variant);
        let generics: BTreeSet<GenId> = self.ix.program.item(item).scope.generics.iter().copied().collect();
        let mut found = BTreeMap::new();
        for fv in &s.fields {
            let name = match &fv.member {
                syn::Member::Named(id) => id.to_string(),
                syn::Member::Unnamed(i) => i.index.to_string(),
            };
            let declared = fields.iter().find(|(n, _)| *n == name).map(|(_, t)| t.clone());
            let v = self.eval(&fv.expr, None);
            if let Some(d) = &declared {
                for alt in &v.0 {
                    self.ix.unify(d, alt, &generics, &mut found);
                }
                self.coerce(&v, d);
            }
        }
        if let Some(r) = &s.rest {
            let rv = self.eval(r, None);
            for alt in &rv.0 {
                if let Ty::Adt(id, args) = alt.peel()
                    && *id == item
                {
                    for (g, a) in self.ix.program.item(item).scope.generics.iter().zip(args) {
                        found.entry(*g).or_default().add(a.clone());
                    }
                }
            }
        }
        let map = crate::recursion::index::single(&found, &generics);
        let self_args: Vec<Ty> = match self.self_type().map(|t| t.peel().clone()) {
            Some(Ty::Adt(id, a)) if id == item && segs.first().is_some_and(|f| f == "Self") => a,
            _ => Vec::new(),
        };
        let list: Vec<Ty> = self
            .ix
            .program
            .item(item)
            .scope
            .generics
            .iter()
            .enumerate()
            .map(|(i, g)| match map.get(g) {
                Some(Ty::Param(x)) if x == g => self_args.get(i).cloned().unwrap_or_else(Ty::unknown),
                Some(t) => t.clone(),
                None => Ty::unknown(),
            })
            .collect();
        Val::of(Ty::Adt(item, list))
    }

    /// `Type::name(args)`: the kernel functions of the type (or of its bounds, for an open
    /// parameter), and the outside function otherwise; `trait_name` filters a qualified path's.
    #[allow(clippy::too_many_arguments, reason = "each is one part of the call")]
    fn assoc_call(
        &mut self,
        base: &Ty,
        name: &str,
        trait_name: Option<&str>,
        args: &[Val],
        turbofish: &[Ty],
        expected: Option<&Ty>,
        line: usize,
    ) -> Val {
        let found = self.ix.assoc(base, name);
        let mut out = Val::none();
        for t in &found.targets {
            if let Some(tn) = trait_name
                && self.target_trait(t.node).is_some_and(|x| x != tn)
            {
                continue;
            }
            let mut fixed = t.map.clone();
            self.turbofish(t.node, turbofish, &mut fixed);
            let v = self.call_node(t.node, &fixed, args, false, line, t.how);
            out.join(&v);
        }
        if !found.outside.is_empty() {
            let qualifier = match base {
                Ty::Prim(p) => Some(p.clone()),
                Ty::Ext(n, _) => Some(n.clone()),
                _ => None,
            };
            if let Some((tr, m)) = STATIC_TRAIT_FNS.iter().find(|(_, m)| *m == name) {
                let target = Some(base.clone()).filter(|b| !matches!(b, Ty::Hidden(_) | Ty::Param(_)));
                let v = self.conversion(tr, m, target.as_ref().or(expected), args, line);
                out.join(&v);
            }
            let v = match stdlib::function(qualifier.as_deref(), name, args, &|_| Val::none()) {
                Some(_) => self.std_call(None, qualifier.as_deref(), name, args, expected, line),
                None if !args.is_empty() && args.first().is_some_and(|a| {
                    a.peeled().iter().any(|r| stdlib::method(r, name, args.get(1..).unwrap_or(&[]), &|_| Val::none()).is_some())
                }) => {
                    let mut v = Val::none();
                    if let Some((first, rest)) = args.split_first() {
                        for r in first.peeled() {
                            let x = self.std_call(Some(&r), None, name, rest, expected, line);
                            v.join(&x);
                        }
                    }
                    v
                }
                None => {
                    let mut v = self.std_call(None, qualifier.as_deref(), name, args, expected, line);
                    if let Ty::Prim(_) = base {
                        v.add(base.clone());
                    }
                    v
                }
            };
            out.join(&v);
        }
        out
    }

    /// `Trait::name(args)` for a kernel trait: on the first argument's type for a method with a
    /// receiver; on the expected type, or every implementation, for one without.
    fn kernel_trait_call(&mut self, t: crate::recursion::ty::ItemId, name: &str, args: &[Val], expected: Option<&Ty>, line: usize) -> Val {
        let trait_name = self.ix.program.item(t).name.clone();
        let mut out = Val::none();
        match self.ix.trait_method_has_receiver(t, name) {
            Some(true) => {
                if let Some(first) = args.first() {
                    for alt in first.peeled() {
                        let found = self.ix.assoc(&alt, name);
                        for target in &found.targets {
                            if self.target_trait(target.node).as_deref() == Some(trait_name.as_str())
                                || matches!(alt, Ty::Param(_) | Ty::Dyn(_) | Ty::Hidden(_))
                            {
                                let v = self.call_node(target.node, &target.map, args, false, line, target.how);
                                out.join(&v);
                            }
                        }
                    }
                }
            }
            _ => {
                let on = expected.cloned().unwrap_or_else(Ty::unknown);
                let targets: Vec<Target> = match &on {
                    Ty::Hidden(_) | Ty::Param(_) => self.ix.bound_impls(&[TraitRef::Kernel(t, Vec::new())], name, &on),
                    other => self
                        .ix
                        .assoc(other, name)
                        .targets
                        .into_iter()
                        .filter(|x| self.target_trait(x.node).as_deref() == Some(trait_name.as_str()))
                        .collect(),
                };
                for target in targets {
                    let v = self.call_node(target.node, &target.map, args, false, line, target.how);
                    out.join(&v);
                }
            }
        }
        if out.is_none() {
            out = Val::unknown();
        }
        out
    }

    /// A call of an outside function by path: a standard trait's method (`Clone::clone(&x)`), a
    /// standard conversion (`Default::default()`), or a function the model knows or does not.
    fn ext_call(&mut self, segs: &[String], args: &[Val], turbofish: &[Ty], expected: Option<&Ty>, line: usize) -> Val {
        let name = segs.last().map(String::as_str).unwrap_or_default();
        let qualifier = segs.iter().rev().nth(1).map(String::as_str);
        if let Some(q) = qualifier {
            if let Some((tr, m)) = STATIC_TRAIT_FNS.iter().find(|(tr, m)| *tr == q && *m == name) {
                let target = turbofish.first().or(expected);
                let mut v = self.conversion(tr, m, target, args, line);
                v.join(&self.std_call(None, Some(q), name, args, target, line));
                return v;
            }
            if RECEIVER_TRAITS.contains(&q)
                && let Some((first, rest)) = args.split_first()
            {
                return self.method_call(first, name, rest, turbofish, expected, line);
            }
        }
        self.std_call(None, qualifier, name, args, turbofish.first().or(expected), line)
    }

    /// The trait of node `n`'s implementation, derived implementation or trait default body.
    fn target_trait(&self, n: NodeId) -> Option<String> {
        let node = self.ix.program.node(n)?;
        match &node.kind {
            NodeKind::Method(imp) => self
                .ix
                .impl_trait
                .get(imp.0)
                .and_then(Option::as_ref)
                .map(|t| self.ix.trait_name(t)),
            NodeKind::Derived { trait_name, .. } => Some(trait_name.clone()),
            NodeKind::TraitDefault(t) => Some(self.ix.program.item(*t).name.clone()),
            NodeKind::Fn => None,
        }
    }

    /// `receiver.name(args)` on every alternative of the receiver.
    fn method_call(&mut self, recv: &Val, name: &str, args: &[Val], turbofish: &[Ty], expected: Option<&Ty>, line: usize) -> Val {
        let mut out = Val::none();
        for alt in &recv.0 {
            let alt = self.ix.normalize(alt);
            let found = self.ix.method(&alt, name);
            for t in &found.targets {
                let mut fixed = t.map.clone();
                self.turbofish(t.node, turbofish, &mut fixed);
                let mut call_args = vec![Val::of(alt.clone())];
                call_args.extend(args.iter().cloned());
                let v = self.call_node(t.node, &fixed, &call_args, true, line, t.how);
                out.join(&v);
            }
            for o in &found.outside {
                let target = turbofish.first().or(expected).cloned();
                let v = self.std_call(Some(o), None, name, args, target.as_ref(), line);
                out.join(&v);
                if let Some((_, tr, m)) = CONVERSIONS.iter().find(|(x, _, _)| *x == name) {
                    let mut conv_args = vec![Val::of(o.clone())];
                    conv_args.extend(args.iter().cloned());
                    let c = self.conversion(tr, m, target.as_ref(), &conv_args, line);
                    out.join(&c);
                }
            }
        }
        out
    }

    /// Calls kernel function `node`, its generic parameters in `fixed` already fixed, with
    /// `args` (the receiver first for a method): one instance per combination of the generic
    /// arguments the arguments fix, each an edge; the result. `receiver` says the first argument
    /// was the receiver a method lookup chose the function for, so it is not checked again.
    fn call_node(&mut self, node: NodeId, fixed: &Map, args: &[Val], receiver: bool, line: usize, how_found: &'static str) -> Val {
        let generics: Vec<GenId> = self.ix.node_generics(node).to_vec();
        let Some(sig) = self.ix.sig(node).cloned() else {
            return Val::unknown();
        };
        let fixed: Map = fixed
            .iter()
            .filter(|(g, v)| !matches!(v, Ty::Param(x) if x == *g))
            .map(|(g, v)| (*g, v.clone()))
            .collect();
        let vars: BTreeSet<GenId> = generics.iter().filter(|g| !fixed.contains_key(g)).copied().collect();
        let inputs: Vec<Ty> = sig.inputs.iter().map(|t| self.ix.normalize(&t.subst(&fixed))).collect();
        // A call no alternative of an argument fits is not a call this function can take.
        for (i, (p, a)) in inputs.iter().zip(args).enumerate() {
            if (receiver && i == 0) || a.is_none() {
                continue;
            }
            if a.0.iter().all(|alt| !self.fits(p, alt, &vars)) {
                return Val::none();
            }
        }
        let mut found: BTreeMap<GenId, Val> = BTreeMap::new();
        for (p, a) in inputs.iter().zip(args) {
            for alt in &a.0 {
                self.ix.unify(p, alt, &vars, &mut found);
            }
        }
        let mut out = Val::none();
        for full in combinations(&generics, &fixed, &found) {
            let list: Vec<Ty> = generics
                .iter()
                .map(|g| full.get(g).cloned().unwrap_or(Ty::Param(*g)))
                .collect();
            let inst = self.m.node_inst(self.ix, node, list);
            self.edge(inst, line, how_found);
            for (i, a) in args.iter().enumerate() {
                let Some(p) = sig.inputs.get(i) else {
                    continue;
                };
                let declared = self.ix.normalize(&p.subst(&full));
                self.coerce(a, &declared);
                self.hint(a, p, &full);
            }
            let ret = self.ix.normalize(&sig.output.subst(&full));
            if has_opaque(&ret) {
                let r = self.m.ret_of(inst, self.current);
                out.join(&r);
            } else {
                out.add(ret);
            }
        }
        self.seen.extend(out.parts());
        out
    }

    /// Whether a value of type `actual` may be passed where `declared` is written: their heads
    /// agree, or a coercion makes them (dereference, unsizing, a closure or function to a function
    /// pointer, anything to a trait object).
    fn fits(&self, declared: &Ty, actual: &Ty, vars: &BTreeSet<GenId>) -> bool {
        if self.ix.heads_agree(declared, actual, vars, 0) {
            return true;
        }
        match (declared.peel(), actual.peel()) {
            (Ty::Dyn(_) | Ty::Opaque(_) | Ty::FnPtr(_), _) => true,
            (d, a) => self
                .ix
                .deref_steps(a)
                .iter()
                .any(|s| self.ix.heads_agree(d, s, vars, 0)),
        }
    }

    /// A closure passed where a closure bound or closure trait object is written takes that
    /// bound's parameter types, as the compiler gives them to it.
    fn hint(&mut self, a: &Val, declared: &Ty, full: &Map) {
        let traits: Vec<TraitRef> = match declared.peel() {
            Ty::Param(g) => self.resolver().bounds(*g),
            Ty::Dyn(t) | Ty::Opaque(t) => t.clone(),
            _ => return,
        };
        let Some(TraitRef::Fn(_, inputs, _)) = traits.iter().find(|t| t.fn_arity().is_some()) else {
            return;
        };
        let params: Vec<Val> = inputs
            .iter()
            .map(|t| Val::of(self.ix.normalize(&t.subst(full))))
            .collect();
        for alt in &a.0 {
            if let Ty::Closure(k) = alt.peel() {
                let ci = self.m.closure_inst(self.ix, k);
                if self.m.add_params(ci, &params) && self.m.inst(ci).is_some_and(|i| i.owner == self.owner) {
                    self.changed = true;
                }
            }
        }
    }

    /// Records the closures and functions a value holds as coerced to a closure trait object or
    /// function pointer, when `declared` holds one.
    fn coerce(&mut self, v: &Val, declared: &Ty) {
        let (d, f) = erased(declared);
        if !d && !f {
            return;
        }
        let cs = callables(v);
        if cs.is_empty() {
            return;
        }
        if d {
            self.m.coerce(cs.iter().cloned(), true);
        }
        if f {
            self.m.coerce(cs, false);
        }
    }

    /// A call through a value of type `callee`; `join` says the arguments are new values for a
    /// closure's parameters (a model's already-applied closure takes none).
    fn call_value(&mut self, callee: &Ty, args: &[Val], line: usize, how_found: &'static str, join: bool) -> Val {
        match callee {
            Ty::Ref(inner) => self.call_value(inner, args, line, how_found, join),
            Ty::Ext(n, inner) if matches!(n.as_str(), "Box" | "Rc" | "Arc") && inner.len() == 1 => {
                let i = inner.first().cloned().unwrap_or_else(Ty::unknown);
                self.call_value(&i, args, line, how_found, join)
            }
            Ty::Ext(n, inner) if n == "Ctor" => inner.first().cloned().map_or_else(Val::unknown, Val::of),
            Ty::Closure(k) => {
                if self.compile_time {
                    return Val::unknown();
                }
                let ci = self.m.closure_inst(self.ix, k);
                self.edge(ci, line, how_found);
                if join && self.m.add_params(ci, args) && self.m.inst(ci).is_some_and(|i| i.owner == self.owner) {
                    self.changed = true;
                }
                self.m.ret_of(ci, self.current)
            }
            Ty::FnDef(n, list) => {
                let fixed: Map = self
                    .ix
                    .node_generics(*n)
                    .iter()
                    .copied()
                    .zip(list.iter().cloned())
                    .filter(|(g, v)| !matches!(v, Ty::Param(x) if x == g))
                    .collect();
                self.call_node(*n, &fixed, args, false, line, how_found)
            }
            Ty::Dyn(traits) | Ty::Opaque(traits) => {
                let fn_trait = traits.iter().find(|t| t.fn_arity().is_some()).cloned();
                let Some(TraitRef::Fn(_, inputs, output)) = fn_trait else {
                    return self.std_call(None, None, "call", args, None, line);
                };
                if !self.compile_time {
                    self.m.read(Dep::Dyn, self.current);
                }
                let mut out = Val::of((*output).clone());
                let set: Vec<Ty> = self.m.dyn_callables.iter().cloned().collect();
                for c in set {
                    if self.arity_fits(&c, inputs.len()) {
                        let v = self.call_value(&c, args, line, how::ERASED, true);
                        out.join(&v);
                    }
                }
                out
            }
            Ty::FnPtr(n) => {
                if !self.compile_time {
                    self.m.read(Dep::FnPtr, self.current);
                }
                let mut out = Val::none();
                let set: Vec<Ty> = self.m.fnptr_callables.iter().cloned().collect();
                for c in set {
                    if self.arity_fits(&c, *n) {
                        let v = self.call_value(&c, args, line, how::ERASED, true);
                        out.join(&v);
                    }
                }
                out.join(&self.std_call(None, None, "call", args, None, line));
                out
            }
            Ty::Hidden(parts) => {
                let cs: Vec<Ty> = parts.iter().filter(|t| t.is_callable()).cloned().collect();
                if cs.is_empty() {
                    return self.std_call(None, None, "call", args, None, line);
                }
                let mut out = Val::none();
                for c in cs {
                    let v = self.call_value(&c, args, line, how_found, join);
                    out.join(&v);
                }
                out
            }
            Ty::Param(g) => {
                // An open parameter's callables come from outside the crates that ship.
                let output = self.resolver().bounds(*g).into_iter().find_map(|t| match t {
                    TraitRef::Fn(_, _, out) => Some(*out),
                    _ => None,
                });
                output.map_or_else(Val::unknown, |o| Val::of(self.ix.normalize(&o.subst(&self.map))))
            }
            _ => self.std_call(None, None, "call", args, None, line),
        }
    }

    /// Whether callable `c` takes `n` parameters, or may.
    fn arity_fits(&self, c: &Ty, n: usize) -> bool {
        match c {
            Ty::Closure(k) => {
                let known = self
                    .m
                    .find(&InstKind::Closure((**k).clone()))
                    .and_then(|i| self.m.arity.get(&i).copied());
                known.is_none_or(|a| a == n)
            }
            Ty::FnDef(node, _) => self.ix.sig(*node).is_none_or(|s| s.inputs.len() == n),
            _ => true,
        }
    }

    /// What callable `c` returns, recording this instance as reading it.
    fn ret_of_callable(&mut self, c: &Ty) -> Val {
        match c {
            Ty::Ref(inner) => self.ret_of_callable(inner),
            Ty::Closure(k) => {
                if self.compile_time {
                    return Val::unknown();
                }
                let ci = self.m.closure_inst(self.ix, k);
                self.m.ret_of(ci, self.current)
            }
            Ty::FnDef(n, list) => {
                let map = self.ix.map_of(*n, list);
                let out = self
                    .ix
                    .sig(*n)
                    .map_or_else(Ty::unknown, |s| self.ix.normalize(&s.output.subst(&map)));
                if has_opaque(&out) && !self.compile_time {
                    let inst = self.m.node_inst(self.ix, *n, list.clone());
                    return self.m.ret_of(inst, self.current);
                }
                Val::of(out)
            }
            Ty::Dyn(traits) => traits
                .iter()
                .find_map(|t| match t {
                    TraitRef::Fn(_, _, out) => Some(Val::of((**out).clone())),
                    _ => None,
                })
                .unwrap_or_else(Val::unknown),
            _ => Val::unknown(),
        }
    }

    /// A call into the standard library: `receiver.name(args)` with `receiver` of an outside type
    /// (or one without a kernel method of that name), or `qualifier::name(args)`; `target` is the
    /// result type a conversion builds, when the source states it.
    #[allow(clippy::too_many_lines, reason = "the model's parts applied in turn")]
    fn std_call(&mut self, receiver: Option<&Ty>, qualifier: Option<&str>, name: &str, args: &[Val], target: Option<&Ty>, line: usize) -> Val {
        let mut inputs: Vec<Val> = Vec::new();
        if let Some(r) = receiver {
            inputs.push(Val::of(r.clone()));
        }
        inputs.extend(args.iter().cloned());
        let mut parts = BTreeSet::new();
        for v in &inputs {
            parts.extend(v.parts());
        }
        // What each callable handed over returns.
        let mut table: BTreeMap<Ty, Val> = BTreeMap::new();
        for v in &inputs {
            for c in callables(v) {
                if !table.contains_key(&c) {
                    let r = self.ret_of_callable(&c);
                    table.insert(c, r);
                }
            }
        }
        let rets = |v: &Val| -> Val {
            let mut out = Val::none();
            for c in callables(v) {
                if let Some(r) = table.get(&c) {
                    out.join(r);
                }
            }
            out
        };
        let model = match receiver {
            Some(r) => stdlib::method(r.peel(), name, args, &rets),
            None => stdlib::function(qualifier, name, args, &rets),
        }
        .unwrap_or_else(Model::unknown);
        // Callables handed into a container of closure trait objects or function pointers are
        // coerced to them.
        if let Some(r) = receiver {
            for a in args {
                self.coerce(a, r);
            }
        }
        let mut extra = Val::none();
        let mut applied: BTreeSet<Ty> = BTreeSet::new();
        for (at, params) in &model.apply {
            let Some(v) = inputs.get(*at).cloned() else {
                continue;
            };
            for alt in &v.0 {
                let c = alt.peel().clone();
                if c.is_callable() {
                    let r = self.call_value(&c, params, line, how::CALLBACK, true);
                    extra.join(&r);
                    applied.insert(c);
                }
            }
        }
        let hidden_args = |n: usize, parts: &BTreeSet<Ty>| vec![Val::of(Ty::Hidden(parts.clone())); n.max(1)];
        if model.unknown {
            for c in parts.iter().filter(|t| matches!(t, Ty::Closure(_) | Ty::FnDef(..))).cloned().collect::<Vec<_>>() {
                if applied.contains(&c) {
                    continue;
                }
                let r = if self.is_applied(&parts, &c) {
                    self.call_value(&c, &[], line, how::CALLBACK, false)
                } else {
                    self.call_value(&c, &hidden_args(3, &parts), line, how::CALLBACK, true)
                };
                extra.join(&r);
            }
        }
        // Running an iterator runs what it was built from.
        if model.consumes {
            let mut sources: BTreeSet<Ty> = BTreeSet::new();
            for v in &inputs {
                for alt in &v.0 {
                    match stdlib::sources_of(alt) {
                        Some(s) => sources.extend(s.iter().flat_map(Ty::parts)),
                        None => sources.extend(alt.parts()),
                    }
                }
            }
            for c in sources.iter().filter(|t| matches!(t, Ty::Closure(_) | Ty::FnDef(..))).cloned().collect::<Vec<_>>() {
                if applied.contains(&c) || model.unknown {
                    continue;
                }
                let r = if self.is_applied(&sources, &c) {
                    self.call_value(&c, &[], line, how::CALLBACK, false)
                } else {
                    self.call_value(&c, &hidden_args(3, &sources), line, how::CALLBACK, true)
                };
                extra.join(&r);
            }
            for t in self.ix.outside_impls(&sources, Some(&stdlib::ITERATOR_TRAITS)) {
                let r = self.call_node(t.node, &t.map, &[], true, line, t.how);
                extra.join(&r);
            }
        }
        // Trait implementations of the kernel types handed over.
        let listed: Option<Vec<&str>> = model.traits.as_ref().map(|t| t.to_vec());
        if listed.as_ref().is_none_or(|t| !t.is_empty()) {
            for t in self.ix.outside_impls(&parts, listed.as_deref()) {
                let r = self.call_node(t.node, &t.map, &[], true, line, t.how);
                extra.join(&r);
            }
        }
        // A conversion builds its result with the kernel implementation on the result's type.
        let mut converted = None;
        if let Some((tr, m)) = model.conversion {
            converted = Some(self.conversion(tr, m, target, &inputs, line));
        }
        let mut out = match (model.ret, converted) {
            (_, Some(c)) => c,
            (Some(r), None) => r,
            (None, None) => {
                let mut p = parts.clone();
                p.extend(extra.parts());
                Val::of(Ty::Hidden(p))
            }
        };
        if let Some(t) = target
            && model.conversion.is_some()
            && !t.has_infer()
        {
            out.add(t.clone());
        }
        self.seen.extend(out.parts());
        out
    }

    /// Whether closure `c` is listed among `parts` as already applied by a model.
    fn is_applied(&self, parts: &BTreeSet<Ty>, c: &Ty) -> bool {
        parts.iter().any(|p| matches!(p, Ty::Ext(n, a) if n == stdlib::APPLIED && a.first() == Some(c)))
    }

    /// A standard conversion: the kernel implementations of trait `tr`'s function `m` on
    /// `target`, or on every kernel type when it is not known, called with `args`; the result.
    fn conversion(&mut self, tr: &str, m: &str, target: Option<&Ty>, args: &[Val], line: usize) -> Val {
        let mut out = Val::none();
        let known = target.filter(|t| !t.has_infer() && !matches!(t, Ty::Hidden(_)));
        let targets: Vec<Target> = match known {
            Some(t) => self
                .ix
                .assoc(t, m)
                .targets
                .into_iter()
                .filter(|x| self.target_trait(x.node).as_deref() == Some(tr))
                .collect(),
            None => self
                .ix
                .bound_impls(&[TraitRef::Ext(tr.to_owned(), Vec::new())], m, &Ty::unknown()),
        };
        for t in targets {
            let call_args: Vec<Val> = if m == "default" { Vec::new() } else { args.to_vec() };
            let v = self.call_node(t.node, &t.map, &call_args, false, line, how::CONVERSION);
            out.join(&v);
        }
        if let Some(t) = known {
            out.add(t.clone());
        }
        if out.is_none() {
            let mut p = BTreeSet::new();
            for a in args {
                p.extend(a.parts());
            }
            out.add(Ty::Hidden(p));
        }
        out
    }

    /// A trait's method called on each alternative of `recv` with no auto-dereference: the kernel
    /// implementations of that trait for its type; for an outside type, the callbacks of that
    /// trait on its parts. `receiver` says `recv` is the first argument.
    #[allow(clippy::too_many_arguments, reason = "each is one part of the call")]
    fn trait_call(&mut self, recv: &Val, trait_name: &str, method: &str, args: &[Val], receiver: bool, line: usize, how_found: &'static str) -> Val {
        let mut out = Val::none();
        for alt in &recv.0 {
            let alt = self.ix.normalize(alt);
            let found = self.ix.assoc(&alt, method);
            let mut hit = false;
            for t in &found.targets {
                let tt = self.target_trait(t.node);
                if tt.as_deref() != Some(trait_name) && !matches!(alt, Ty::Param(_) | Ty::Dyn(_)) {
                    continue;
                }
                hit = true;
                let mut call_args = Vec::new();
                if receiver {
                    call_args.push(Val::of(alt.clone()));
                }
                call_args.extend(args.iter().cloned());
                let v = self.call_node(t.node, &t.map, &call_args, receiver, line, how_found);
                out.join(&v);
            }
            if !hit || !found.outside.is_empty() {
                let mut parts = alt.parts();
                for a in args {
                    parts.extend(a.parts());
                }
                for t in self.ix.outside_impls(&parts, Some(&[trait_name])) {
                    let v = self.call_node(t.node, &t.map, &[], true, line, how::CALLBACK_IMPL);
                    out.join(&v);
                }
                for c in parts.iter().filter(|t| matches!(t, Ty::Closure(_) | Ty::FnDef(..))).cloned().collect::<Vec<_>>() {
                    if method == "call" || method == "call_mut" || method == "call_once" {
                        let v = self.call_value(&c, args, line, how::CALLBACK, true);
                        out.join(&v);
                    }
                }
            }
        }
        out
    }

    // ------------------------------------------------------------ operators

    fn binary(&mut self, b: &syn::ExprBinary, line: usize) -> Val {
        let l = self.eval(&b.left, None);
        let r = self.eval(&b.right, None);
        match &b.op {
            BinOp::And(_) | BinOp::Or(_) => Val::prim("bool"),
            op => {
                let Some((tr, m)) = binop_trait(op) else {
                    return Val::unknown();
                };
                let compare = matches!(tr, "PartialEq" | "PartialOrd");
                let assign = tr.ends_with("Assign");
                let v = self.operator(&l, &r, tr, m, true, line);
                if assign {
                    self.feed(&b.left, &r);
                    Val::unit()
                } else if compare {
                    Val::prim("bool")
                } else {
                    v
                }
            }
        }
    }

    /// An operator: trait `tr`'s method `m` on each alternative of the left operand; primitives
    /// run nothing and give their own type.
    fn operator(&mut self, l: &Val, r: &Val, tr: &str, m: &str, binary: bool, line: usize) -> Val {
        let mut out = Val::none();
        for alt in &l.0 {
            let peeled = alt.peel().clone();
            if let Ty::Prim(_) = peeled {
                out.add(peeled);
                continue;
            }
            let args: Vec<Val> = if binary { vec![r.clone()] } else { Vec::new() };
            let v = self.trait_call(&Val::of(alt.clone()), tr, m, &args, true, line, how::IMPLICIT);
            if v.is_none() {
                out.add(Ty::Hidden(alt.parts()));
            } else {
                out.join(&v);
            }
        }
        out
    }

    fn deref(&mut self, v: &Val, line: usize) -> Val {
        let mut out = Val::none();
        for alt in &v.0 {
            match alt {
                Ty::Ref(inner) => {
                    out.add((**inner).clone());
                }
                Ty::Ext(n, inner) if matches!(n.as_str(), "Box" | "Rc" | "Arc") => {
                    out.add(inner.first().cloned().unwrap_or_else(Ty::unknown));
                }
                Ty::Prim(_) => {
                    out.add(alt.clone());
                }
                other => {
                    let r = self.trait_call(&Val::of(other.clone()), "Deref", "deref", &[], true, line, how::IMPLICIT);
                    let steps = self.ix.deref_steps(other);
                    match steps.get(1) {
                        Some(t) => out.add(t.clone()),
                        None => out.add(Ty::Hidden(other.parts())),
                    };
                    for t in &r.0 {
                        if let Ty::Ref(i) = t {
                            out.add((**i).clone());
                        }
                    }
                }
            }
        }
        out
    }

    fn index(&mut self, base: &Val, idx: &Val, line: usize) -> Val {
        let ranged = idx.peeled().iter().any(|t| matches!(t, Ty::Ext(n, _) if n.starts_with("Range")));
        let mut out = Val::none();
        for alt in &base.0 {
            let peeled = alt.peel().clone();
            match &peeled {
                Ty::Ext(n, a) if n == "Vec" || n == "VecDeque" => {
                    let e = a.first().cloned().unwrap_or_else(Ty::unknown);
                    if ranged {
                        out.add(Ty::Slice(Box::new(e.clone())));
                    }
                    out.add(e);
                }
                Ty::Slice(e) | Ty::Array(e) => {
                    if ranged {
                        out.add(Ty::Slice(e.clone()));
                    }
                    out.add((**e).clone());
                }
                Ty::Prim(p) if p == "str" => {
                    out.add(peeled.clone());
                }
                _ => {
                    let r = self.trait_call(&Val::of(peeled.clone()), "Index", "index", &[idx.clone()], true, line, how::IMPLICIT);
                    if r.is_none() {
                        out.add(Ty::Hidden(peeled.parts()));
                    }
                    for t in &r.0 {
                        out.add(t.peel().clone());
                    }
                }
            }
        }
        out
    }

    fn try_op(&mut self, v: &Val, line: usize) -> Val {
        let mut out = Val::none();
        let declared = self.rets.last().and_then(|(_, d)| d.clone());
        let target_err = declared.as_ref().and_then(|d| match d.peel() {
            Ty::Ext(n, a) if n == "Result" => a.get(1).cloned(),
            _ => None,
        });
        for alt in v.peeled() {
            match &alt {
                Ty::Ext(n, a) if n == "Result" => {
                    out.add(a.first().cloned().unwrap_or_else(Ty::unknown));
                    let err = a.get(1).cloned().unwrap_or_else(Ty::unknown);
                    if target_err.as_ref() != Some(&err) {
                        self.conversion("From", "from", target_err.as_ref(), &[Val::of(err)], line);
                    }
                }
                Ty::Ext(n, a) if n == "Option" => {
                    out.add(a.first().cloned().unwrap_or_else(Ty::unknown));
                }
                other => {
                    out.add(Ty::Hidden(other.parts()));
                    if !matches!(other, Ty::Prim(_)) {
                        self.conversion("From", "from", target_err.as_ref(), &[Val::of(Ty::Hidden(other.parts()))], line);
                    }
                }
            }
        }
        out
    }

    /// The items a `for` loop over a value of these types binds; the calls iterating makes.
    fn iterate(&mut self, v: &Val, line: usize) -> Val {
        let mut items = Val::none();
        for alt in v.peeled() {
            if stdlib::item_of(&alt).is_some() {
                let it = self.std_call(Some(&alt), None, "into_iter", &[], None, line);
                for i in &it.0 {
                    let n = self.std_call(Some(i), None, "next", &[], None, line);
                    items.join(&option_inner(&n));
                }
                continue;
            }
            let it = self.method_call(&Val::of(alt.clone()), "into_iter", &[], &[], None, line);
            for i in &it.0 {
                let n = self.method_call(&Val::of(i.clone()), "next", &[], &[], None, line);
                items.join(&option_inner(&n));
            }
        }
        items
    }

    // ------------------------------------------------------------ closures

    fn closure(&mut self, c: &syn::ExprClosure, line: usize) -> Val {
        let index = self.next_closure;
        self.next_closure = self.next_closure.saturating_add(1);
        if self.compile_time && self.node.0 == usize::MAX {
            self.problem(line, "a closure in a constant's or static's value, which the recursion check does not read".to_owned());
            return Val::unknown();
        }
        if c.asyncness.is_some() {
            self.problem(line, "an async closure, which the recursion check cannot read".to_owned());
        }
        let key = ClosureKey {
            owner: self.node,
            subst: self.args.clone(),
            index,
        };
        let ci = self.m.closure_inst(self.ix, &key);
        self.m.arity.insert(ci, c.inputs.len());
        let mut params = self.m.params.get(&ci).cloned().unwrap_or_default();
        while params.len() < c.inputs.len() {
            params.push(Val::none());
        }
        let declared_ret = match &c.output {
            syn::ReturnType::Type(_, t) => Some(self.ty(t)),
            syn::ReturnType::Default => None,
        };
        self.push_scope();
        for (p, v) in c.inputs.iter().zip(params) {
            self.bind_pat(p, &v);
        }
        let saved = self.current;
        self.current = ci;
        let saved_loops = std::mem::take(&mut self.loops);
        self.rets.push((Val::none(), declared_ret.clone()));
        let v = self.eval(&c.body, declared_ret.as_ref());
        let (mut acc, _) = self.rets.pop().unwrap_or_default();
        acc.join(&v);
        self.loops = saved_loops;
        self.current = saved;
        self.pop_scope();
        self.m.add_ret(ci, &acc);
        Val::of(Ty::Closure(Box::new(key)))
    }

    // ------------------------------------------------------------ macros

    fn mac(&mut self, mac: &syn::Macro, expected: Option<&Ty>) -> Val {
        let line = line_of(mac);
        let name = mac.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
        match macros::read(self.resolver(), &self.scope, mac, self.depth) {
            MacroRead::Nothing => {
                if matches!(name.as_str(), "line" | "column") {
                    Val::prim("u32")
                } else {
                    Val::unknown()
                }
            }
            MacroRead::Exprs(es) => {
                let vals: Vec<Val> = es.iter().map(|e| self.eval(e, None)).collect();
                match name.as_str() {
                    "vec" => {
                        let mut elem = Val::none();
                        for v in &vals {
                            elem.join(v);
                        }
                        Val::of(Ty::ext("Vec", vec![stdlib::one(&elem)]))
                    }
                    "matches" | "assert_matches" | "debug_assert_matches" => Val::prim("bool"),
                    "dbg" => vals.last().cloned().unwrap_or_else(Val::unit),
                    "addr_of" | "addr_of_mut" => {
                        let mut out = Val::none();
                        for t in vals.last().map(|v| v.0.clone()).unwrap_or_default() {
                            out.add(Ty::Ref(Box::new(t)));
                        }
                        out
                    }
                    _ => {
                        let mut p = BTreeSet::new();
                        for v in &vals {
                            p.extend(v.parts());
                        }
                        Val::of(Ty::Hidden(p))
                    }
                }
            }
            MacroRead::Format {
                plain,
                formatted,
                destination,
                compared,
            } => {
                for e in &plain {
                    self.eval(e, None);
                }
                if let Some((a, b)) = &compared {
                    let av = self.eval(a, None);
                    let bv = self.eval(b, None);
                    self.operator(&av, &bv, "PartialEq", "eq", true, line);
                    self.trait_call(&av, "Debug", "fmt", &[], true, line, how::IMPLICIT);
                    self.trait_call(&bv, "Debug", "fmt", &[], true, line, how::IMPLICIT);
                }
                for f in &formatted {
                    let (v, t) = match f {
                        Formatted::Expr(e, t) => (self.eval(e, None), *t),
                        Formatted::Captured(n, t) => (self.name_value(n, line), *t),
                    };
                    if t != FmtTrait::Count {
                        self.trait_call(&v, t.name(), "fmt", &[], true, line, how::IMPLICIT);
                    }
                }
                let mut result = match name.as_str() {
                    "format" => Val::of(Ty::ext("String", Vec::new())),
                    "write" | "writeln" => Val::of(Ty::ext("Result", vec![Ty::unit(), Ty::ext("Error", Vec::new())])),
                    "panic" | "unreachable" | "todo" | "unimplemented" => Val::none(),
                    "format_args" => Val::of(Ty::ext("Arguments", Vec::new())),
                    _ => Val::unit(),
                };
                if let Some(d) = &destination {
                    let dv = self.eval(d, None);
                    let w = self.method_call(&dv, "write_fmt", &[Val::of(Ty::ext("Arguments", Vec::new()))], &[], None, line);
                    result.join(&w);
                    self.method_call(&dv, "write_str", &[Val::of(Ty::Ref(Box::new(Ty::prim("str"))))], &[], None, line);
                }
                result
            }
            MacroRead::Repeat(x, n) => {
                let v = self.eval(&x, None);
                self.eval(&n, None);
                self.trait_call(&v, "Clone", "clone", &[], true, line, how::IMPLICIT);
                Val::of(Ty::ext("Vec", vec![stdlib::one(&v)]))
            }
            MacroRead::Expanded(stmts) => {
                self.depth = self.depth.saturating_add(1);
                let v = self.stmts(&stmts, expected);
                self.depth = self.depth.saturating_sub(1);
                v
            }
            MacroRead::Unread(why) => {
                self.problem(line, why);
                Val::unknown()
            }
        }
    }

    // ------------------------------------------------------------ drop glue

    /// Drop glue: every kernel `Drop` implementation of a type among the parts of the values the
    /// body handled, and of their fields' types, transitively.
    fn drop_glue(&mut self, line: usize) {
        if !self.ix.by_ext_trait.contains_key("Drop") {
            return;
        }
        let mut todo: Vec<Ty> = self.seen.iter().cloned().collect();
        for l in &self.locals {
            todo.extend(l.parts());
        }
        let mut done: BTreeSet<Ty> = BTreeSet::new();
        let mut parts: BTreeSet<Ty> = BTreeSet::new();
        while let Some(t) = todo.pop() {
            if !done.insert(t.clone()) {
                continue;
            }
            if let Ty::Adt(item, args) = &t {
                let map: Map = self
                    .ix
                    .program
                    .item(*item)
                    .scope
                    .generics
                    .iter()
                    .copied()
                    .zip(args.iter().cloned())
                    .collect();
                for f in self.ix.all_field_types(*item) {
                    todo.extend(f.subst(&map).parts().iter().map(Ty::shallow));
                }
            }
            parts.insert(t);
        }
        for t in self.ix.outside_impls(&parts, Some(&["Drop"])) {
            self.call_node(t.node, &t.map, &[], true, line, how::DROP);
        }
    }
}

/// Every combination of the generic arguments a call fixes: the fixed ones, and each value found
/// for the others (an open parameter where none was); past [`MAX_COMBINATIONS`], each parameter
/// with several values gets their hidden join instead.
fn combinations(generics: &[GenId], fixed: &Map, found: &BTreeMap<GenId, Val>) -> Vec<Map> {
    let choices: Vec<(GenId, Vec<Ty>)> = generics
        .iter()
        .map(|g| {
            let options = match fixed.get(g) {
                Some(t) => vec![t.clone()],
                None => match found.get(g) {
                    Some(v) if !v.is_none() => v.0.iter().cloned().collect(),
                    _ => vec![Ty::Param(*g)],
                },
            };
            (*g, options)
        })
        .collect();
    let total = choices
        .iter()
        .map(|(_, o)| o.len())
        .try_fold(1_usize, usize::checked_mul)
        .unwrap_or(usize::MAX);
    if total > MAX_COMBINATIONS {
        let mut map = Map::new();
        for (g, options) in choices {
            let t = if options.len() == 1 {
                options.into_iter().next().unwrap_or(Ty::Param(g))
            } else {
                let mut parts = BTreeSet::new();
                for o in &options {
                    parts.extend(o.parts().iter().map(Ty::shallow));
                    if !matches!(o, Ty::Hidden(_)) {
                        parts.insert(o.shallow());
                    }
                }
                Ty::Hidden(parts)
            };
            map.insert(g, t);
        }
        return vec![map];
    }
    let mut out: Vec<Map> = vec![Map::new()];
    for (g, options) in choices {
        let mut next = Vec::new();
        for m in &out {
            for o in &options {
                let mut m2 = m.clone();
                m2.insert(g, o.clone());
                next.push(m2);
            }
        }
        out = next;
    }
    out
}

/// The inner type of each `Option` alternative; a hidden value of anything else.
fn option_inner(v: &Val) -> Val {
    let mut out = Val::none();
    for t in v.peeled() {
        match &t {
            Ty::Ext(n, a) if n == "Option" => {
                out.add(a.first().cloned().unwrap_or_else(Ty::unknown));
            }
            other => {
                out.add(Ty::Hidden(other.parts()));
            }
        }
    }
    out
}

/// A literal's value: its suffix's type, the expected primitive, or for an unsuffixed number a
/// hidden value (which may be any primitive).
fn lit(l: &syn::Lit, expected: Option<&Ty>) -> Val {
    let expected_prim = expected.and_then(|t| match t.peel() {
        Ty::Prim(p) => Some(p.clone()),
        _ => None,
    });
    match l {
        syn::Lit::Str(_) => Val::of(Ty::Ref(Box::new(Ty::prim("str")))),
        syn::Lit::ByteStr(_) => Val::of(Ty::Ref(Box::new(Ty::Array(Box::new(Ty::prim("u8")))))),
        syn::Lit::CStr(_) => Val::of(Ty::Ref(Box::new(Ty::ext("CStr", Vec::new())))),
        syn::Lit::Byte(_) => Val::prim("u8"),
        syn::Lit::Char(_) => Val::prim("char"),
        syn::Lit::Bool(_) => Val::prim("bool"),
        syn::Lit::Int(i) => {
            if i.suffix().is_empty() {
                expected_prim.map_or_else(Val::unknown, |p| Val::prim(&p))
            } else {
                Val::prim(i.suffix())
            }
        }
        syn::Lit::Float(f) => {
            if f.suffix().is_empty() {
                expected_prim.map_or_else(Val::unknown, |p| Val::prim(&p))
            } else {
                Val::prim(f.suffix())
            }
        }
        _ => Val::unknown(),
    }
}
