//! Types and abstract values for the recursion check's call graph.
//!
//! What it does: [`Ty`] is a type as far as the analysis knows it: a primitive, one of the
//! kernel's own structs, enums or unions with its arguments, an outside type such as `Vec<T>` by
//! name, a reference, slice, array or tuple, a generic parameter left open, a trait object, a
//! return-position `impl Trait`, a projection, a function pointer, one closure of one instance, one
//! kernel function item with its arguments, an outside function item, or [`Ty::Hidden`]: a type
//! not known, built from the types it lists (and from primitives). [`Val`] is what an expression
//! may be: a set of such types, its alternatives.
//!
//! Guarantees:
//!
//! - **Hidden keeps what it may hide.** A value whose exact type is not known is a
//!   [`Ty::Hidden`] listing every type it may be built from ([`Ty::parts`]): each kernel type,
//!   closure, function item and open parameter inside it, so a call dispatched on it reaches every
//!   implementation those types have.
//! - **Finite.** Types deeper than [`MAX_DEPTH`] are cut to a [`Ty::Hidden`] of their parts and a
//!   value holds at most [`MAX_ALTERNATIVES`] alternatives before they merge into one
//!   [`Ty::Hidden`], so the lattice the analysis climbs has finite height and its fixed point is
//!   reached.
//! - **Joins only grow.** [`Val::join`] is a set union followed by the two cuts above, each of
//!   which keeps every part of what it cuts.
//! - **Deterministic.** Every set is ordered (`BTreeSet`).
//!
//! Method: a type lattice in the style of abstract interpretation (P. Cousot and R. Cousot,
//! "Abstract interpretation: a unified lattice model for static analysis of programs", POPL 1977,
//! <https://doi.org/10.1145/512950.512973>), over the type grammar of The Rust Reference
//! (<https://doc.rust-lang.org/reference/types.html>), with closures and function items as types
//! of their own, as the compiler gives them
//! (<https://doc.rust-lang.org/reference/types/closure.html>,
//! <https://doc.rust-lang.org/reference/types/function-item.html>).
//!
//! Cost: each operation is linear in the size of the types it touches; recursion follows the
//! nesting of a type, which [`MAX_DEPTH`] bounds.

use std::collections::{BTreeMap, BTreeSet};

/// An item of the program: a struct, enum, union, trait, type alias, constant or static.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemId(pub usize);

/// A function body or derived implementation of the program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub usize);

/// A generic type parameter declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GenId(pub usize);

/// How deep a type may nest before it is cut to a [`Ty::Hidden`] of its parts.
pub const MAX_DEPTH: usize = 8;

/// How many alternatives a value holds before they merge into one [`Ty::Hidden`].
pub const MAX_ALTERNATIVES: usize = 12;

/// A trait named in a bound, an `impl` or a trait object.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TraitRef {
    /// One of the kernel's own traits, with its generic arguments.
    Kernel(ItemId, Vec<Ty>),
    /// A trait from outside the kernel, by its last path segment (`Add`, `Iterator`), with its
    /// generic arguments.
    Ext(String, Vec<Ty>),
    /// A closure trait (`Fn`, `FnMut`, `FnOnce`) written `Fn(A, B) -> R`: its name, its inputs and
    /// its output (the unit type when none is written).
    Fn(String, Vec<Ty>, Box<Ty>),
}

impl TraitRef {
    /// The trait's name for an outside trait; `None` for a kernel trait.
    #[must_use]
    pub fn ext_name(&self) -> Option<&str> {
        match self {
            TraitRef::Ext(name, _) | TraitRef::Fn(name, ..) => Some(name),
            TraitRef::Kernel(..) => None,
        }
    }

    /// The arity of a closure trait, if this is one.
    #[must_use]
    pub fn fn_arity(&self) -> Option<usize> {
        match self {
            TraitRef::Fn(_, args, _) => Some(args.len()),
            _ => None,
        }
    }

    /// The trait's generic arguments: for a closure trait, its inputs.
    #[must_use]
    pub fn args(&self) -> &[Ty] {
        match self {
            TraitRef::Kernel(_, args) | TraitRef::Ext(_, args) | TraitRef::Fn(_, args, _) => args,
        }
    }

    /// The trait with every generic parameter in `map` replaced by its value.
    #[must_use]
    pub fn subst(&self, map: &BTreeMap<GenId, Ty>) -> TraitRef {
        let all = |args: &[Ty]| args.iter().map(|a| a.subst(map)).collect();
        match self {
            TraitRef::Kernel(id, args) => TraitRef::Kernel(*id, all(args)),
            TraitRef::Ext(name, args) => TraitRef::Ext(name.clone(), all(args)),
            TraitRef::Fn(name, args, output) => {
                TraitRef::Fn(name.clone(), all(args), Box::new(output.subst(map)))
            }
        }
    }
}

/// The closure traits.
pub const FN_TRAITS: [&str; 3] = ["Fn", "FnMut", "FnOnce"];

/// One closure of one instance: the function whose body holds it, that instance's generic
/// arguments, and its place among the closures of that body in reading order.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClosureKey {
    /// The function, method, trait default body or derived implementation holding it.
    pub owner: NodeId,
    /// The owner instance's generic arguments.
    pub subst: Vec<Ty>,
    /// Its place among the owner body's closures, counted in reading order from 0.
    pub index: usize,
}

/// A type, as far as the analysis knows it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Ty {
    /// A primitive type: `u64`, `f64`, `bool`, `char`, `str`, ...
    Prim(String),
    /// One of the kernel's structs, enums or unions, with its generic arguments.
    Adt(ItemId, Vec<Ty>),
    /// A type from outside the kernel, by its last path segment, with its generic arguments.
    Ext(String, Vec<Ty>),
    /// A shared or mutable reference, or a raw pointer.
    Ref(Box<Ty>),
    /// A slice `[T]`.
    Slice(Box<Ty>),
    /// An array `[T; N]`.
    Array(Box<Ty>),
    /// A tuple; the unit type is the empty tuple.
    Tuple(Vec<Ty>),
    /// A generic type parameter left open: in an instance analysed for every instantiation, or
    /// one the call did not determine.
    Param(GenId),
    /// A trait object `dyn ...`.
    Dyn(Vec<TraitRef>),
    /// A return-position `impl ...`, as a signature writes it; a call's value is the callee
    /// instance's own result instead.
    Opaque(Vec<TraitRef>),
    /// An associated type not resolved further, such as `T::Item` with `T` open.
    Assoc(Box<Ty>, String),
    /// A function pointer type with this many parameters.
    FnPtr(usize),
    /// One closure.
    Closure(Box<ClosureKey>),
    /// One kernel function item used as a value, with its generic arguments (open ones as
    /// [`Ty::Param`], fixed by the call through it).
    FnDef(NodeId, Vec<Ty>),
    /// A function from outside the kernel used as a value (`i128::from` handed to `map`).
    ExtFn,
    /// The never type.
    Never,
    /// A type the source leaves to inference (`_`, or generic arguments not written): never a
    /// value's type, only a written type's part, which a call or literal fills in.
    Infer,
    /// A type not known, built only from these types and from primitives and outside types.
    Hidden(BTreeSet<Ty>),
}

impl Default for Ty {
    /// The unit type.
    fn default() -> Ty {
        Ty::unit()
    }
}

/// The primitive type names.
pub const PRIMITIVES: [&str; 18] = [
    "bool", "char", "str", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64",
    "i128", "isize", "f32", "f64", "f16",
];

impl Ty {
    /// The unit type.
    #[must_use]
    pub fn unit() -> Ty {
        Ty::Tuple(Vec::new())
    }

    /// A type not known that holds no kernel type: [`Ty::Hidden`] of nothing.
    #[must_use]
    pub fn unknown() -> Ty {
        Ty::Hidden(BTreeSet::new())
    }

    /// A primitive type by name.
    #[must_use]
    pub fn prim(name: &str) -> Ty {
        Ty::Prim(name.to_owned())
    }

    /// An outside type by name with arguments.
    #[must_use]
    pub fn ext(name: &str, args: Vec<Ty>) -> Ty {
        Ty::Ext(name.to_owned(), args)
    }

    /// The type behind any number of references.
    #[must_use]
    pub fn peel(&self) -> &Ty {
        let mut t = self;
        while let Ty::Ref(inner) = t {
            t = inner;
        }
        t
    }

    /// Whether this type is a closure, kernel function item, outside function item, function
    /// pointer or closure trait object: something a call through a value may run.
    #[must_use]
    pub fn is_callable(&self) -> bool {
        match self.peel() {
            Ty::Closure(_) | Ty::FnDef(..) | Ty::ExtFn | Ty::FnPtr(_) => true,
            Ty::Dyn(traits) => traits.iter().any(|t| t.fn_arity().is_some()),
            Ty::Ext(name, args) if name == "Box" || name == "Rc" || name == "Arc" => {
                args.first().is_some_and(Ty::is_callable)
            }
            _ => false,
        }
    }

    /// Whether the type holds a [`Ty::Param`] anywhere.
    #[must_use]
    pub fn has_param(&self) -> bool {
        let mut found = false;
        self.walk(&mut |t| {
            if matches!(t, Ty::Param(_)) {
                found = true;
            }
        });
        found
    }

    /// Whether the type holds a [`Ty::Infer`] anywhere.
    #[must_use]
    pub fn has_infer(&self) -> bool {
        let mut found = false;
        self.walk(&mut |t| {
            if matches!(t, Ty::Infer) {
                found = true;
            }
        });
        found
    }

    /// The type with every [`Ty::Infer`] replaced by `with`.
    #[must_use]
    pub fn fill(&self, with: &Ty) -> Ty {
        let all = |args: &[Ty]| args.iter().map(|a| a.fill(with)).collect::<Vec<_>>();
        match self {
            Ty::Infer => with.clone(),
            Ty::Adt(id, args) => Ty::Adt(*id, all(args)),
            Ty::Ext(name, args) => Ty::Ext(name.clone(), all(args)),
            Ty::Ref(inner) => Ty::Ref(Box::new(inner.fill(with))),
            Ty::Slice(inner) => Ty::Slice(Box::new(inner.fill(with))),
            Ty::Array(inner) => Ty::Array(Box::new(inner.fill(with))),
            Ty::Tuple(items) => Ty::Tuple(all(items)),
            Ty::Assoc(inner, name) => Ty::Assoc(Box::new(inner.fill(with)), name.clone()),
            Ty::FnDef(n, args) => Ty::FnDef(*n, all(args)),
            Ty::Hidden(parts) => Ty::Hidden(parts.iter().map(|p| p.fill(with)).collect()),
            other => other.clone(),
        }
    }

    /// Calls `f` on this type and every type inside it (arguments, elements, closure and function
    /// item arguments, trait arguments, hidden parts).
    pub fn walk(&self, f: &mut dyn FnMut(&Ty)) {
        f(self);
        match self {
            Ty::Adt(_, args) | Ty::Ext(_, args) | Ty::Tuple(args) | Ty::FnDef(_, args) => {
                for a in args {
                    a.walk(f);
                }
            }
            Ty::Ref(inner) | Ty::Slice(inner) | Ty::Array(inner) | Ty::Assoc(inner, _) => {
                inner.walk(f);
            }
            Ty::Closure(k) => {
                for a in &k.subst {
                    a.walk(f);
                }
            }
            Ty::Dyn(traits) | Ty::Opaque(traits) => {
                for t in traits {
                    for a in t.args() {
                        a.walk(f);
                    }
                    if let TraitRef::Fn(_, _, out) = t {
                        out.walk(f);
                    }
                }
            }
            Ty::Hidden(parts) => {
                for p in parts {
                    p.walk(f);
                }
            }
            Ty::Prim(_) | Ty::Param(_) | Ty::FnPtr(_) | Ty::ExtFn | Ty::Never | Ty::Infer => {}
        }
    }

    /// The parts a value of this type may hold that kernel code can be reached through: every
    /// type inside it that is not a primitive, a reference, a tuple, a slice, an array, the never
    /// type or a hidden type itself (whose parts are listed instead).
    #[must_use]
    pub fn parts(&self) -> BTreeSet<Ty> {
        let mut out = BTreeSet::new();
        self.walk(&mut |t| match t {
            Ty::Prim(_)
            | Ty::Ref(_)
            | Ty::Slice(_)
            | Ty::Array(_)
            | Ty::Tuple(_)
            | Ty::Never
            | Ty::Infer
            | Ty::Hidden(_) => {}
            other => {
                out.insert(other.clone());
            }
        });
        out
    }

    /// The type with every generic parameter in `map` replaced by its value.
    #[must_use]
    pub fn subst(&self, map: &BTreeMap<GenId, Ty>) -> Ty {
        if map.is_empty() {
            return self.clone();
        }
        let all = |args: &[Ty]| args.iter().map(|a| a.subst(map)).collect::<Vec<_>>();
        match self {
            Ty::Param(g) => map.get(g).cloned().unwrap_or(Ty::Param(*g)),
            Ty::Adt(id, args) => Ty::Adt(*id, all(args)),
            Ty::Ext(name, args) => Ty::Ext(name.clone(), all(args)),
            Ty::Ref(inner) => Ty::Ref(Box::new(inner.subst(map))),
            Ty::Slice(inner) => Ty::Slice(Box::new(inner.subst(map))),
            Ty::Array(inner) => Ty::Array(Box::new(inner.subst(map))),
            Ty::Tuple(items) => Ty::Tuple(all(items)),
            Ty::Assoc(inner, name) => Ty::Assoc(Box::new(inner.subst(map)), name.clone()),
            Ty::Dyn(traits) => Ty::Dyn(traits.iter().map(|t| t.subst(map)).collect()),
            Ty::Opaque(traits) => Ty::Opaque(traits.iter().map(|t| t.subst(map)).collect()),
            Ty::FnDef(n, args) => Ty::FnDef(*n, all(args)),
            Ty::Closure(k) => Ty::Closure(Box::new(ClosureKey {
                owner: k.owner,
                subst: all(&k.subst),
                index: k.index,
            })),
            Ty::Hidden(parts) => Ty::Hidden(parts.iter().map(|p| p.subst(map)).collect()),
            Ty::Prim(_) | Ty::FnPtr(_) | Ty::ExtFn | Ty::Never | Ty::Infer => self.clone(),
        }
    }

    /// How deep the type nests: 1 for a type with nothing inside it.
    #[must_use]
    pub fn depth(&self) -> usize {
        let inner = match self {
            Ty::Adt(_, args) | Ty::Ext(_, args) | Ty::Tuple(args) | Ty::FnDef(_, args) => {
                args.iter().map(Ty::depth).max().unwrap_or(0)
            }
            Ty::Ref(inner) | Ty::Slice(inner) | Ty::Array(inner) | Ty::Assoc(inner, _) => {
                inner.depth()
            }
            Ty::Closure(k) => k.subst.iter().map(Ty::depth).max().unwrap_or(0),
            Ty::Dyn(traits) | Ty::Opaque(traits) => traits
                .iter()
                .flat_map(|t| t.args().iter())
                .map(Ty::depth)
                .max()
                .unwrap_or(0),
            Ty::Hidden(parts) => parts.iter().map(Ty::depth).max().unwrap_or(0),
            Ty::Prim(_) | Ty::Param(_) | Ty::FnPtr(_) | Ty::ExtFn | Ty::Never | Ty::Infer => 0,
        };
        inner.saturating_add(1)
    }

    /// The type cut at [`MAX_DEPTH`]: every subtree that starts deeper is replaced by a
    /// [`Ty::Hidden`] of the shallow forms of its parts (a kernel type, closure or function item
    /// with its own arguments hidden), so no part is lost and the result is never deeper than
    /// [`MAX_DEPTH`] plus two.
    #[must_use]
    pub fn cut(&self) -> Ty {
        self.cut_at(0)
    }

    fn cut_at(&self, depth: usize) -> Ty {
        if depth >= MAX_DEPTH {
            return Ty::Hidden(self.parts().iter().map(Ty::shallow).collect());
        }
        let next = depth.saturating_add(1);
        let all = |args: &[Ty]| args.iter().map(|a| a.cut_at(next)).collect::<Vec<_>>();
        match self {
            Ty::Adt(id, args) => Ty::Adt(*id, all(args)),
            Ty::Ext(name, args) => Ty::Ext(name.clone(), all(args)),
            Ty::Tuple(items) => Ty::Tuple(all(items)),
            Ty::FnDef(n, args) => Ty::FnDef(*n, all(args)),
            Ty::Ref(inner) => Ty::Ref(Box::new(inner.cut_at(next))),
            Ty::Slice(inner) => Ty::Slice(Box::new(inner.cut_at(next))),
            Ty::Array(inner) => Ty::Array(Box::new(inner.cut_at(next))),
            Ty::Assoc(inner, name) => Ty::Assoc(Box::new(inner.cut_at(next)), name.clone()),
            Ty::Closure(k) => Ty::Closure(Box::new(ClosureKey {
                owner: k.owner,
                subst: all(&k.subst),
                index: k.index,
            })),
            Ty::Hidden(parts) => Ty::Hidden(parts.iter().map(|p| p.cut_at(next)).collect()),
            Ty::Dyn(traits) => Ty::Dyn(traits.iter().map(|t| cut_trait(t, next)).collect()),
            Ty::Opaque(traits) => Ty::Opaque(traits.iter().map(|t| cut_trait(t, next)).collect()),
            Ty::Prim(_) | Ty::Param(_) | Ty::FnPtr(_) | Ty::ExtFn | Ty::Never | Ty::Infer => {
                self.clone()
            }
        }
    }

    /// The type with its own arguments hidden: a kernel or outside type's arguments, a closure's
    /// or function item's generic arguments, each replaced by a [`Ty::Hidden`] of its parts in
    /// their shallow forms; a part with nothing inside it is itself.
    #[must_use]
    pub fn shallow(&self) -> Ty {
        let hide = |args: &[Ty]| {
            args.iter()
                .map(|a| Ty::Hidden(a.parts().iter().map(Ty::atom).collect()))
                .collect::<Vec<_>>()
        };
        match self {
            Ty::Adt(id, args) => Ty::Adt(*id, hide(args)),
            Ty::Ext(name, args) => Ty::Ext(name.clone(), hide(args)),
            Ty::FnDef(n, args) => Ty::FnDef(*n, hide(args)),
            Ty::Closure(k) => Ty::Closure(Box::new(ClosureKey {
                owner: k.owner,
                subst: hide(&k.subst),
                index: k.index,
            })),
            Ty::Assoc(..) | Ty::Dyn(_) | Ty::Opaque(_) => Ty::atom(self),
            other => other.clone(),
        }
    }

    /// The type with everything inside it hidden and nothing listed: the head alone.
    fn atom(&self) -> Ty {
        let none = |args: &[Ty]| args.iter().map(|_| Ty::unknown()).collect::<Vec<_>>();
        match self {
            Ty::Adt(id, args) => Ty::Adt(*id, none(args)),
            Ty::Ext(name, args) => Ty::Ext(name.clone(), none(args)),
            Ty::FnDef(n, args) => Ty::FnDef(*n, none(args)),
            Ty::Closure(k) => Ty::Closure(Box::new(ClosureKey {
                owner: k.owner,
                subst: none(&k.subst),
                index: k.index,
            })),
            Ty::Assoc(..) => Ty::unknown(),
            Ty::Dyn(traits) => Ty::Dyn(traits.iter().map(atom_trait).collect()),
            Ty::Opaque(traits) => Ty::Opaque(traits.iter().map(atom_trait).collect()),
            other => other.clone(),
        }
    }
}

fn cut_trait(t: &TraitRef, depth: usize) -> TraitRef {
    let all = |args: &[Ty]| args.iter().map(|a| a.cut_at(depth)).collect::<Vec<_>>();
    match t {
        TraitRef::Kernel(id, args) => TraitRef::Kernel(*id, all(args)),
        TraitRef::Ext(name, args) => TraitRef::Ext(name.clone(), all(args)),
        TraitRef::Fn(name, args, out) => {
            TraitRef::Fn(name.clone(), all(args), Box::new(out.cut_at(depth)))
        }
    }
}

fn atom_trait(t: &TraitRef) -> TraitRef {
    let none = |args: &[Ty]| args.iter().map(|_| Ty::unknown()).collect::<Vec<_>>();
    match t {
        TraitRef::Kernel(id, args) => TraitRef::Kernel(*id, none(args)),
        TraitRef::Ext(name, args) => TraitRef::Ext(name.clone(), none(args)),
        TraitRef::Fn(name, args, _) => {
            TraitRef::Fn(name.clone(), none(args), Box::new(Ty::unknown()))
        }
    }
}

/// What an expression may be: the set of its alternatives. The empty set is a value no run
/// reaches (an expression that diverges, or that the analysis has not yet seen reached).
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Val(pub BTreeSet<Ty>);

impl Val {
    /// No value: unreached.
    #[must_use]
    pub fn none() -> Val {
        Val(BTreeSet::new())
    }

    /// A value of one type.
    #[must_use]
    pub fn of(ty: Ty) -> Val {
        let mut v = Val::none();
        v.add(ty);
        v
    }

    /// The unit value.
    #[must_use]
    pub fn unit() -> Val {
        Val::of(Ty::unit())
    }

    /// A value whose type is not known and holds nothing of the kernel.
    #[must_use]
    pub fn unknown() -> Val {
        Val::of(Ty::unknown())
    }

    /// A value of a primitive type.
    #[must_use]
    pub fn prim(name: &str) -> Val {
        Val::of(Ty::prim(name))
    }

    /// Whether no run reaches it.
    #[must_use]
    pub fn is_none(&self) -> bool {
        self.0.is_empty()
    }

    /// Adds one alternative, cut to [`MAX_DEPTH`]; returns whether the value grew.
    pub fn add(&mut self, ty: Ty) -> bool {
        let ty = if ty.has_infer() { ty.fill(&Ty::unknown()) } else { ty };
        let ty = if ty.depth() > MAX_DEPTH { ty.cut() } else { ty };
        if matches!(ty, Ty::Never) {
            return false;
        }
        let grew = self.0.insert(ty);
        if grew && self.0.len() > MAX_ALTERNATIVES {
            let mut parts = BTreeSet::new();
            for t in &self.0 {
                parts.extend(t.parts().iter().map(Ty::shallow));
            }
            self.0 = BTreeSet::from([Ty::Hidden(parts)]);
        }
        grew
    }

    /// Adds every alternative of `other`; returns whether the value grew.
    pub fn join(&mut self, other: &Val) -> bool {
        let before = self.clone();
        for t in &other.0 {
            self.add(t.clone());
        }
        *self != before
    }

    /// The join of two values.
    #[must_use]
    pub fn or(&self, other: &Val) -> Val {
        let mut v = self.clone();
        v.join(other);
        v
    }

    /// The parts of every alternative ([`Ty::parts`]).
    #[must_use]
    pub fn parts(&self) -> BTreeSet<Ty> {
        let mut out = BTreeSet::new();
        for t in &self.0 {
            out.extend(t.parts());
        }
        out
    }

    /// A value of unknown type built from what this value holds: one [`Ty::Hidden`] of its parts.
    #[must_use]
    pub fn hidden(&self) -> Val {
        Val::of(Ty::Hidden(self.parts()))
    }

    /// The value with every generic parameter in `map` replaced.
    #[must_use]
    pub fn subst(&self, map: &BTreeMap<GenId, Ty>) -> Val {
        let mut v = Val::none();
        for t in &self.0 {
            v.add(t.subst(map));
        }
        v
    }

    /// Each alternative with references peeled.
    #[must_use]
    pub fn peeled(&self) -> Vec<Ty> {
        let mut out: Vec<Ty> = self.0.iter().map(|t| t.peel().clone()).collect();
        out.sort();
        out.dedup();
        out
    }
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "tests may use native arithmetic and casts"
)]
mod tests {
    use super::*;

    fn adt(i: usize, args: Vec<Ty>) -> Ty {
        Ty::Adt(ItemId(i), args)
    }

    #[test]
    fn parts_reach_through_every_type_form() {
        let closure = Ty::Closure(Box::new(ClosureKey {
            owner: NodeId(2),
            subst: vec![adt(7, Vec::new())],
            index: 0,
        }));
        let t = Ty::Tuple(vec![
            Ty::Ref(Box::new(Ty::ext("Vec", vec![adt(3, vec![Ty::Param(GenId(1))])]))),
            Ty::Array(Box::new(closure.clone())),
            Ty::Hidden(BTreeSet::from([adt(5, Vec::new())])),
            Ty::prim("u64"),
        ]);
        let p = t.parts();
        assert!(p.contains(&adt(3, vec![Ty::Param(GenId(1))])));
        assert!(p.contains(&Ty::Param(GenId(1))));
        assert!(p.contains(&closure));
        assert!(p.contains(&adt(7, Vec::new())), "a closure's generic arguments are parts");
        assert!(p.contains(&adt(5, Vec::new())), "hidden parts are parts");
        assert!(!p.iter().any(|x| matches!(x, Ty::Prim(_) | Ty::Tuple(_) | Ty::Ref(_))));
        assert!(Ty::prim("f64").parts().is_empty());
    }

    #[test]
    fn cutting_keeps_every_part_and_bounds_depth() {
        let mut t = adt(1, Vec::new());
        for _ in 0..30 {
            t = Ty::ext("Box", vec![adt(2, vec![t])]);
        }
        assert!(t.depth() > MAX_DEPTH);
        let c = t.cut();
        assert!(c.depth() <= MAX_DEPTH + 2, "{}", c.depth());
        let heads = |x: &Ty| -> BTreeSet<ItemId> {
            x.parts()
                .iter()
                .filter_map(|p| match p {
                    Ty::Adt(id, _) => Some(*id),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(heads(&c), heads(&t));
        let mut v = Val::none();
        assert!(v.add(t));
        assert!(v.0.iter().all(|x| x.depth() <= MAX_DEPTH + 2));
    }

    #[test]
    fn values_merge_past_the_limit_and_only_grow() {
        let mut v = Val::none();
        for i in 0..MAX_ALTERNATIVES {
            assert!(v.add(adt(i, Vec::new())));
        }
        assert_eq!(v.0.len(), MAX_ALTERNATIVES);
        assert!(v.add(adt(100, Vec::new())));
        assert_eq!(v.0.len(), 1);
        let p = v.parts();
        assert_eq!(p.len(), MAX_ALTERNATIVES + 1);
        assert!(!v.add(Ty::Never), "never adds nothing");
        let w = v.clone();
        assert!(!v.join(&w));
    }

    #[test]
    fn substitution_reaches_closures_function_items_and_hidden_parts() {
        let g = GenId(4);
        let map = BTreeMap::from([(g, adt(9, Vec::new()))]);
        let t = Ty::Tuple(vec![
            Ty::FnDef(NodeId(1), vec![Ty::Param(g)]),
            Ty::Closure(Box::new(ClosureKey {
                owner: NodeId(1),
                subst: vec![Ty::Param(g)],
                index: 3,
            })),
            Ty::Hidden(BTreeSet::from([Ty::Param(g)])),
        ]);
        let s = t.subst(&map);
        assert!(!s.has_param());
        assert!(t.has_param());
        assert!(s.parts().contains(&adt(9, Vec::new())));
    }

    #[test]
    fn callables_are_recognised_through_references_and_boxes() {
        let c = Ty::Closure(Box::new(ClosureKey {
            owner: NodeId(0),
            subst: Vec::new(),
            index: 0,
        }));
        assert!(Ty::Ref(Box::new(c.clone())).is_callable());
        assert!(Ty::ext("Box", vec![Ty::Dyn(vec![TraitRef::Fn(
            "Fn".to_owned(),
            Vec::new(),
            Box::new(Ty::unit())
        )])])
        .is_callable());
        assert!(!Ty::ext("Vec", vec![c]).is_callable());
        assert!(!Ty::prim("u8").is_callable());
    }
}
