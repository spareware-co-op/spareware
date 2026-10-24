//! Types and abstract values for the recursion check's call graph.
//!
//! What it does: [`Ty`] is a static type as far as the source states it: a primitive, one of the
//! kernel's own structs, enums or unions with its arguments, an outside type such as `Vec<T>` by
//! name, a reference, slice, array or tuple, a generic parameter, a trait object or `impl Trait`,
//! a function pointer, a closure, a function item, or unknown. [`Carries`] is what a value may
//! carry that the kernel's code can be called through: the kernel types inside it (whose trait
//! implementations outside code may call), the closures, functions and carrier nodes it may hold
//! (which a call through the value may run), the generic parameters whose values it may hold, or
//! anything at all. [`Val`] is the pair the body analysis computes for every expression.
//!
//! Guarantees:
//!
//! - **A type's carries are complete for what the type states.** [`Ty::carries`] returns every
//!   kernel type, closure, function item and generic parameter written anywhere inside the type,
//!   through references, slices, arrays, tuples and the arguments of outside types, so a value of
//!   a known type never hides a kernel type from the callbacks of outside code.
//! - **Joins only grow.** [`Carries::join`] and [`Val::join`] are set unions (with `top` absorbing),
//!   so the body analysis's fixed point exists and is reached.
//! - **Deterministic.** Every set is ordered (`BTreeSet`), so every walk over a value is in the
//!   same order on every run.
//!
//! Method: a type lattice in the style of abstract interpretation (P. Cousot and R. Cousot,
//! "Abstract interpretation: a unified lattice model for static analysis of programs", POPL 1977,
//! <https://doi.org/10.1145/512950.512973>), over the type grammar of The Rust Reference
//! (<https://doc.rust-lang.org/reference/types.html>).
//!
//! Cost: each operation is linear in the size of the types and sets it touches; no recursion
//! beyond the nesting of a type as written.

use std::collections::{BTreeMap, BTreeSet};

/// An item of the program: a struct, enum, union, trait, type alias, constant or static.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemId(pub usize);

/// A node of the call graph: a function body, closure, derived implementation or carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub usize);

/// A generic type parameter declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GenId(pub usize);

/// A trait named in a bound, an `impl` or a trait object.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TraitRef {
    /// One of the kernel's own traits, with its generic arguments.
    Kernel(ItemId, Vec<Ty>),
    /// A trait from outside the kernel, by its last path segment (`Add`, `Iterator`, `Fn`), with
    /// its generic arguments (for `Fn(A, B) -> R`, the inputs `A` and `B`).
    Ext(String, Vec<Ty>),
}

impl TraitRef {
    /// The trait's name: its last path segment for an outside trait; `None` for a kernel trait,
    /// whose name the program knows.
    #[must_use]
    pub fn ext_name(&self) -> Option<&str> {
        match self {
            TraitRef::Ext(name, _) => Some(name),
            TraitRef::Kernel(..) => None,
        }
    }

    /// The arity of a closure trait (`Fn`, `FnMut`, `FnOnce`), if this is one.
    #[must_use]
    pub fn fn_arity(&self) -> Option<usize> {
        match self {
            TraitRef::Ext(name, args) if FN_TRAITS.contains(&name.as_str()) => Some(args.len()),
            _ => None,
        }
    }

    /// The trait's generic arguments.
    #[must_use]
    pub fn args(&self) -> &[Ty] {
        match self {
            TraitRef::Kernel(_, args) | TraitRef::Ext(_, args) => args,
        }
    }
}

/// The closure traits.
pub const FN_TRAITS: [&str; 3] = ["Fn", "FnMut", "FnOnce"];

/// A static type, as far as the source states it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Ty {
    /// A primitive type: `u64`, `f64`, `bool`, `char`, `str`, ...
    Prim(String),
    /// One of the kernel's structs, enums or unions, with its generic arguments.
    Adt(ItemId, Vec<Ty>),
    /// A type from outside the kernel, by its last path segment, with its generic arguments.
    Ext(String, Vec<Ty>),
    /// A shared or mutable reference.
    Ref(Box<Ty>),
    /// A slice `[T]`.
    Slice(Box<Ty>),
    /// An array `[T; N]`.
    Array(Box<Ty>),
    /// A tuple; the unit type is the empty tuple.
    Tuple(Vec<Ty>),
    /// A generic type parameter.
    Param(GenId),
    /// A trait object `dyn ...` or an `impl ...` type: some type with these traits.
    Dyn(Vec<TraitRef>),
    /// An associated type of a type, such as `T::Item` or `Self::Output`, not resolved further.
    Assoc(Box<Ty>, String),
    /// A function pointer type with this many parameters.
    FnPtr(usize),
    /// The type of one closure.
    Closure(NodeId),
    /// The type of one function item, used as a value.
    FnDef(NodeId),
    /// The never type.
    Never,
    /// Not known.
    Unknown,
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

    /// Whether this is a primitive (or a reference to one), which carries nothing.
    #[must_use]
    pub fn is_prim(&self) -> bool {
        match self {
            Ty::Prim(_) | Ty::Never => true,
            Ty::Ref(inner) => inner.is_prim(),
            Ty::Tuple(items) => items.iter().all(Ty::is_prim),
            _ => false,
        }
    }

    /// Whether the type is known at all.
    #[must_use]
    pub fn is_known(&self) -> bool {
        !matches!(self, Ty::Unknown)
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

    /// Every kernel type, closure, function item and generic parameter written in the type.
    #[must_use]
    pub fn carries(&self) -> Carries {
        let mut out = Carries::default();
        let mut work: Vec<&Ty> = vec![self];
        while let Some(t) = work.pop() {
            match t {
                Ty::Adt(id, args) => {
                    out.types.insert(*id);
                    work.extend(args.iter());
                }
                Ty::Ext(_, args) | Ty::Tuple(args) => work.extend(args.iter()),
                Ty::Ref(inner) | Ty::Slice(inner) | Ty::Array(inner) | Ty::Assoc(inner, _) => {
                    work.push(inner);
                }
                Ty::Param(g) => {
                    out.generics.insert(*g);
                }
                Ty::Dyn(traits) => {
                    for tr in traits {
                        work.extend(tr.args().iter());
                    }
                }
                Ty::Closure(n) | Ty::FnDef(n) => {
                    out.calls.insert(*n);
                }
                Ty::Prim(_) | Ty::FnPtr(_) | Ty::Never | Ty::Unknown => {}
            }
        }
        out
    }

    /// The type with every generic parameter in `map` replaced by its value.
    #[must_use]
    pub fn subst(&self, map: &BTreeMap<GenId, Ty>) -> Ty {
        if map.is_empty() {
            return self.clone();
        }
        match self {
            Ty::Param(g) => map.get(g).cloned().unwrap_or(Ty::Param(*g)),
            Ty::Adt(id, args) => Ty::Adt(*id, args.iter().map(|a| a.subst(map)).collect()),
            Ty::Ext(name, args) => Ty::Ext(name.clone(), args.iter().map(|a| a.subst(map)).collect()),
            Ty::Ref(inner) => Ty::Ref(Box::new(inner.subst(map))),
            Ty::Slice(inner) => Ty::Slice(Box::new(inner.subst(map))),
            Ty::Array(inner) => Ty::Array(Box::new(inner.subst(map))),
            Ty::Tuple(items) => Ty::Tuple(items.iter().map(|a| a.subst(map)).collect()),
            Ty::Assoc(inner, name) => Ty::Assoc(Box::new(inner.subst(map)), name.clone()),
            Ty::Dyn(traits) => Ty::Dyn(
                traits
                    .iter()
                    .map(|tr| match tr {
                        TraitRef::Kernel(id, args) => {
                            TraitRef::Kernel(*id, args.iter().map(|a| a.subst(map)).collect())
                        }
                        TraitRef::Ext(name, args) => {
                            TraitRef::Ext(name.clone(), args.iter().map(|a| a.subst(map)).collect())
                        }
                    })
                    .collect(),
            ),
            other => other.clone(),
        }
    }

    /// Matches `self`, a pattern whose generic parameters in `params` are unknowns, against
    /// `actual`, adding each parameter's value to `map`; references on either side are looked
    /// through. A mismatch adds nothing; it is never an error, because the caller only uses what
    /// was learned.
    pub fn unify(&self, actual: &Ty, params: &BTreeSet<GenId>, map: &mut BTreeMap<GenId, Ty>) {
        let mut work: Vec<(&Ty, &Ty)> = vec![(self, actual)];
        while let Some((p, a)) = work.pop() {
            match (p.peel(), a.peel()) {
                (Ty::Param(g), actual) if params.contains(g) && actual.is_known() => {
                    map.entry(*g).or_insert_with(|| actual.clone());
                }
                (Ty::Adt(x, xs), Ty::Adt(y, ys)) if x == y => work.extend(xs.iter().zip(ys)),
                (Ty::Ext(x, xs), Ty::Ext(y, ys)) if x == y => work.extend(xs.iter().zip(ys)),
                (Ty::Tuple(xs), Ty::Tuple(ys)) => work.extend(xs.iter().zip(ys)),
                (Ty::Slice(x) | Ty::Array(x), Ty::Slice(y) | Ty::Array(y)) => work.push((x, y)),
                _ => {}
            }
        }
    }
}

/// What a value may carry that kernel code can be called through.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Carries {
    /// Kernel structs, enums and unions inside the value.
    pub types: BTreeSet<ItemId>,
    /// Closures, function items and carrier nodes the value may hold or lead to.
    pub calls: BTreeSet<NodeId>,
    /// Generic parameters whose values the value may hold.
    pub generics: BTreeSet<GenId>,
    /// The value may be anything at all.
    pub top: bool,
}

impl Carries {
    /// Everything: a value the analysis cannot follow.
    #[must_use]
    pub fn top() -> Carries {
        Carries {
            top: true,
            ..Carries::default()
        }
    }

    /// Adds everything `other` carries; returns whether anything was added.
    pub fn join(&mut self, other: &Carries) -> bool {
        let before = (
            self.types.len(),
            self.calls.len(),
            self.generics.len(),
            self.top,
        );
        self.types.extend(other.types.iter().copied());
        self.calls.extend(other.calls.iter().copied());
        self.generics.extend(other.generics.iter().copied());
        self.top |= other.top;
        before
            != (
                self.types.len(),
                self.calls.len(),
                self.generics.len(),
                self.top,
            )
    }

    /// Whether it carries nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.types.is_empty() && self.calls.is_empty() && self.generics.is_empty() && !self.top
    }
}

/// An abstract value: its static type, if known, and what it may carry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Val {
    /// The static type, or [`Ty::Unknown`].
    pub ty: Ty,
    /// What the value may carry, including everything its type states.
    pub carries: Carries,
}

impl Val {
    /// A value of a known type, carrying what the type states.
    #[must_use]
    pub fn of(ty: Ty) -> Val {
        let carries = ty.carries();
        Val { ty, carries }
    }

    /// A value of unknown type carrying `carries`.
    #[must_use]
    pub fn unknown(carries: Carries) -> Val {
        Val {
            ty: Ty::Unknown,
            carries,
        }
    }

    /// The unit value, which carries nothing.
    #[must_use]
    pub fn unit() -> Val {
        Val::of(Ty::unit())
    }

    /// A value of type `ty` that also carries `extra`.
    #[must_use]
    pub fn with(ty: Ty, extra: &Carries) -> Val {
        let mut v = Val::of(ty);
        v.carries.join(extra);
        v
    }

    /// The join of two values: the type if both agree, else unknown, and the union of what both
    /// carry.
    #[must_use]
    pub fn join(&self, other: &Val) -> Val {
        let ty = if self.ty == other.ty {
            self.ty.clone()
        } else if matches!(self.ty, Ty::Never) {
            other.ty.clone()
        } else if matches!(other.ty, Ty::Never) {
            self.ty.clone()
        } else {
            Ty::Unknown
        };
        let mut carries = self.carries.clone();
        carries.join(&other.carries);
        Val { ty, carries }
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

    #[test]
    fn carries_reach_through_every_type_form() {
        let t = Ty::Tuple(vec![
            Ty::Ref(Box::new(Ty::Ext(
                "Vec".to_owned(),
                vec![Ty::Adt(ItemId(3), vec![Ty::Param(GenId(1))])],
            ))),
            Ty::Array(Box::new(Ty::Closure(NodeId(7)))),
            Ty::Dyn(vec![TraitRef::Ext(
                "Fn".to_owned(),
                vec![Ty::Adt(ItemId(4), Vec::new())],
            )]),
            Ty::Prim("u64".to_owned()),
        ]);
        let c = t.carries();
        assert_eq!(c.types, [ItemId(3), ItemId(4)].into());
        assert_eq!(c.calls, [NodeId(7)].into());
        assert_eq!(c.generics, [GenId(1)].into());
        assert!(!c.top);
        assert!(Ty::Prim("f64".to_owned()).carries().is_empty());
    }

    #[test]
    fn joins_only_grow_and_top_absorbs() {
        let mut a = Carries::default();
        assert!(a.join(&Ty::Adt(ItemId(1), Vec::new()).carries()));
        assert!(!a.join(&Ty::Adt(ItemId(1), Vec::new()).carries()));
        assert!(a.join(&Carries::top()));
        assert!(a.top);
        let v = Val::of(Ty::Prim("u8".to_owned())).join(&Val::of(Ty::Prim("u16".to_owned())));
        assert_eq!(v.ty, Ty::Unknown);
        let w = Val::of(Ty::Never).join(&Val::of(Ty::Prim("u8".to_owned())));
        assert_eq!(w.ty, Ty::Prim("u8".to_owned()));
    }

    #[test]
    fn unification_learns_parameters_through_references() {
        let t = GenId(0);
        let pattern = Ty::Ref(Box::new(Ty::Slice(Box::new(Ty::Param(t)))));
        let actual = Ty::Ref(Box::new(Ty::Ext(
            "Vec".to_owned(),
            vec![Ty::Adt(ItemId(9), Vec::new())],
        )));
        let mut map = BTreeMap::new();
        pattern.unify(&actual, &[t].into(), &mut map);
        assert!(map.is_empty(), "a slice does not match a Vec");
        let actual = Ty::Ref(Box::new(Ty::Array(Box::new(Ty::Adt(ItemId(9), Vec::new())))));
        pattern.unify(&actual, &[t].into(), &mut map);
        assert_eq!(map.get(&t), Some(&Ty::Adt(ItemId(9), Vec::new())));
        assert_eq!(
            Ty::Ext("Option".to_owned(), vec![Ty::Param(t)]).subst(&map),
            Ty::Ext("Option".to_owned(), vec![Ty::Adt(ItemId(9), Vec::new())])
        );
    }
}
