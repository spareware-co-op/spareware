//! Types for the recursion check's type checker: Rust's types as far as a call graph needs them,
//! with inference variables.
//!
//! What it does: [`Ty`] is a type of the program: a primitive, a struct, enum or union of the
//! crates read (the standard library's model among them, [`crate::recursion::stdlib`]) with its
//! arguments, a reference or raw pointer, a slice, array or tuple, a generic parameter left open,
//! a trait object, a return-position `impl Trait`, a projection not yet resolved, a function
//! pointer, one closure, one function item, the never type, an inference variable of the body
//! being checked, a `_` written in the source, or a type the checker could not infer
//! ([`Ty::Unknown`]). [`TraitRef`] is a trait with its arguments and its associated-type bindings
//! (`Iterator<Item = u64>`; `Fn(A) -> R` is `Fn<(A,)>` with `Output = R`).
//!
//! Guarantees:
//!
//! - **One type per value.** As in Rust, every expression of one instance has exactly one type;
//!   what the checker does not know yet is an inference variable, and what it cannot learn is
//!   [`Ty::Unknown`], which the checker reports wherever a call would depend on it.
//! - **Finite.** [`Ty::cut`] replaces everything nested deeper than [`MAX_DEPTH`] by
//!   [`Ty::Unknown`], so instances, which are keyed by types, are finitely many.
//! - **Deterministic.** Types are ordered (`Ord`), so every table keyed by them iterates the same
//!   way on every run.
//!
//! Method: the type grammar of The Rust Reference
//! (<https://doc.rust-lang.org/reference/types.html>), with closures and function items as types
//! of their own (<https://doc.rust-lang.org/reference/types/closure.html>,
//! <https://doc.rust-lang.org/reference/types/function-item.html>), and inference variables as
//! in rustc's type checker (<https://rustc-dev-guide.rust-lang.org/type-inference.html>).
//!
//! Cost: each operation is linear in the size of the types it touches; recursion follows the
//! nesting of a type, which [`MAX_DEPTH`] bounds for every type an instance is keyed by.

use std::collections::BTreeMap;

/// An item of the program: a struct, enum, union, trait, type alias, constant or static.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemId(pub usize);

/// A function body or derived implementation of the program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub usize);

/// A generic type parameter declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GenId(pub usize);

/// How deep a type an instance is keyed by may nest before its deeper parts become unknown.
pub const MAX_DEPTH: usize = 10;

/// A primitive type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Prim {
    /// `bool`.
    Bool,
    /// `char`.
    Char,
    /// `str`.
    Str,
    /// `i8`.
    I8,
    /// `i16`.
    I16,
    /// `i32`.
    I32,
    /// `i64`.
    I64,
    /// `i128`.
    I128,
    /// `isize`.
    Isize,
    /// `u8`.
    U8,
    /// `u16`.
    U16,
    /// `u32`.
    U32,
    /// `u64`.
    U64,
    /// `u128`.
    U128,
    /// `usize`.
    Usize,
    /// `f16`.
    F16,
    /// `f32`.
    F32,
    /// `f64`.
    F64,
}

/// Every primitive with its name.
pub const PRIMS: [(Prim, &str); 18] = [
    (Prim::Bool, "bool"),
    (Prim::Char, "char"),
    (Prim::Str, "str"),
    (Prim::I8, "i8"),
    (Prim::I16, "i16"),
    (Prim::I32, "i32"),
    (Prim::I64, "i64"),
    (Prim::I128, "i128"),
    (Prim::Isize, "isize"),
    (Prim::U8, "u8"),
    (Prim::U16, "u16"),
    (Prim::U32, "u32"),
    (Prim::U64, "u64"),
    (Prim::U128, "u128"),
    (Prim::Usize, "usize"),
    (Prim::F16, "f16"),
    (Prim::F32, "f32"),
    (Prim::F64, "f64"),
];

impl Prim {
    /// The primitive named `name`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Prim> {
        PRIMS.iter().find(|(_, n)| *n == name).map(|(p, _)| *p)
    }

    /// Its name.
    #[must_use]
    pub fn name(self) -> &'static str {
        PRIMS.iter().find(|(p, _)| *p == self).map_or("?", |(_, n)| n)
    }

    /// Whether it is an integer type.
    #[must_use]
    pub fn is_int(self) -> bool {
        !matches!(self, Prim::Bool | Prim::Char | Prim::Str | Prim::F16 | Prim::F32 | Prim::F64)
    }

    /// Whether it is a floating-point type.
    #[must_use]
    pub fn is_float(self) -> bool {
        matches!(self, Prim::F16 | Prim::F32 | Prim::F64)
    }
}

/// A trait with its generic arguments (without `Self`) and the associated types its bound fixes.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TraitRef {
    /// The trait.
    pub id: ItemId,
    /// Its generic arguments, defaults filled in where the source leaves them out.
    pub args: Vec<Ty>,
    /// Associated types the bound fixes: `Item = u64`, and a closure trait's `Output`.
    pub bindings: Vec<(String, Ty)>,
}

impl TraitRef {
    /// The binding of associated type `name`, if the bound fixes it.
    #[must_use]
    pub fn binding(&self, name: &str) -> Option<&Ty> {
        self.bindings.iter().find(|(n, _)| n == name).map(|(_, t)| t)
    }

    /// The trait reference with `f` applied to every type in it.
    #[must_use]
    pub fn map(&self, f: &mut dyn FnMut(&Ty) -> Ty) -> TraitRef {
        TraitRef {
            id: self.id,
            args: self.args.iter().map(|a| f(a)).collect(),
            bindings: self.bindings.iter().map(|(n, t)| (n.clone(), f(t))).collect(),
        }
    }
}

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

/// A type.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Ty {
    /// A primitive type.
    Prim(Prim),
    /// A struct, enum or union of the crates read, with its generic arguments.
    Adt(ItemId, Vec<Ty>),
    /// A reference or raw pointer, and whether it is mutable (`&mut T`, `*mut T`).
    Ref(Box<Ty>, bool),
    /// A slice `[T]`.
    Slice(Box<Ty>),
    /// An array `[T; N]`.
    Array(Box<Ty>),
    /// A tuple; the unit type is the empty tuple.
    Tuple(Vec<Ty>),
    /// A generic type parameter left open.
    Param(GenId),
    /// A trait object `dyn ...`.
    Dyn(Vec<TraitRef>),
    /// A return-position `impl ...` as a signature writes it.
    Opaque(Vec<TraitRef>),
    /// A projection `<base as Trait<..>>::Name` not resolved: on an open parameter, or before its
    /// base is known; with the trait and its arguments when known.
    Assoc(Box<Ty>, Option<Box<TraitRef>>, String),
    /// A function pointer type: its parameters and result.
    FnPtr(Vec<Ty>, Box<Ty>),
    /// One closure.
    Closure(Box<ClosureKey>),
    /// One function item used as a value, with its generic arguments.
    FnDef(NodeId, Vec<Ty>),
    /// The never type.
    Never,
    /// An inference variable of the body being checked; its kind is kept by the table.
    Var(u32),
    /// A type the source leaves to inference (`_`): the checker gives it a variable.
    Infer,
    /// A type the checker could not infer.
    Unknown,
}

impl Default for Ty {
    /// The unit type.
    fn default() -> Ty {
        Ty::unit()
    }
}

impl Ty {
    /// The unit type.
    #[must_use]
    pub fn unit() -> Ty {
        Ty::Tuple(Vec::new())
    }

    /// A shared reference to `t`.
    #[must_use]
    pub fn reference(t: Ty) -> Ty {
        Ty::Ref(Box::new(t), false)
    }

    /// A mutable reference to `t`.
    #[must_use]
    pub fn reference_mut(t: Ty) -> Ty {
        Ty::Ref(Box::new(t), true)
    }

    /// The type behind any number of references.
    #[must_use]
    pub fn peel(&self) -> &Ty {
        let mut t = self;
        while let Ty::Ref(inner, _) = t {
            t = inner;
        }
        t
    }

    /// Calls `f` on this type and every type inside it.
    pub fn walk(&self, f: &mut dyn FnMut(&Ty)) {
        f(self);
        match self {
            Ty::Adt(_, args) | Ty::Tuple(args) | Ty::FnDef(_, args) => {
                for a in args {
                    a.walk(f);
                }
            }
            Ty::Ref(inner, _) | Ty::Slice(inner) | Ty::Array(inner) => {
                inner.walk(f);
            }
            Ty::Assoc(inner, tr, _) => {
                inner.walk(f);
                if let Some(tr) = tr {
                    for a in &tr.args {
                        a.walk(f);
                    }
                }
            }
            Ty::FnPtr(params, ret) => {
                for p in params {
                    p.walk(f);
                }
                ret.walk(f);
            }
            Ty::Closure(k) => {
                for a in &k.subst {
                    a.walk(f);
                }
            }
            Ty::Dyn(traits) | Ty::Opaque(traits) => {
                for t in traits {
                    for a in &t.args {
                        a.walk(f);
                    }
                    for (_, b) in &t.bindings {
                        b.walk(f);
                    }
                }
            }
            Ty::Prim(_) | Ty::Param(_) | Ty::Never | Ty::Var(_) | Ty::Infer | Ty::Unknown => {}
        }
    }

    /// Whether `pred` holds of this type or any type inside it.
    #[must_use]
    pub fn any(&self, pred: &dyn Fn(&Ty) -> bool) -> bool {
        let mut found = false;
        self.walk(&mut |t| {
            if pred(t) {
                found = true;
            }
        });
        found
    }

    /// Whether the type holds a generic parameter anywhere.
    #[must_use]
    pub fn has_param(&self) -> bool {
        self.any(&|t| matches!(t, Ty::Param(_)))
    }

    /// Whether the type holds an inference variable or `_` anywhere.
    #[must_use]
    pub fn has_var(&self) -> bool {
        self.any(&|t| matches!(t, Ty::Var(_) | Ty::Infer))
    }

    /// Whether the type holds an unknown part anywhere.
    #[must_use]
    pub fn has_unknown(&self) -> bool {
        self.any(&|t| matches!(t, Ty::Unknown))
    }

    /// The type with `f` applied bottom-up: `f` sees each type after its parts were mapped, and
    /// returns its replacement or `None` to keep it.
    #[must_use]
    pub fn fold(&self, f: &mut dyn FnMut(&Ty) -> Option<Ty>) -> Ty {
        let inner = match self {
            Ty::Adt(id, args) => Ty::Adt(*id, args.iter().map(|a| a.fold(f)).collect()),
            Ty::Tuple(items) => Ty::Tuple(items.iter().map(|a| a.fold(f)).collect()),
            Ty::FnDef(n, args) => Ty::FnDef(*n, args.iter().map(|a| a.fold(f)).collect()),
            Ty::Ref(inner, m) => Ty::Ref(Box::new(inner.fold(f)), *m),
            Ty::Slice(inner) => Ty::Slice(Box::new(inner.fold(f))),
            Ty::Array(inner) => Ty::Array(Box::new(inner.fold(f))),
            Ty::Assoc(inner, tr, name) => Ty::Assoc(
                Box::new(inner.fold(f)),
                tr.as_ref().map(|t| Box::new(t.map(&mut |x| x.fold(f)))),
                name.clone(),
            ),
            Ty::FnPtr(params, ret) => Ty::FnPtr(params.iter().map(|p| p.fold(f)).collect(), Box::new(ret.fold(f))),
            Ty::Closure(k) => Ty::Closure(Box::new(ClosureKey {
                owner: k.owner,
                subst: k.subst.iter().map(|a| a.fold(f)).collect(),
                index: k.index,
            })),
            Ty::Dyn(traits) => Ty::Dyn(traits.iter().map(|t| t.map(&mut |x| x.fold(f))).collect()),
            Ty::Opaque(traits) => Ty::Opaque(traits.iter().map(|t| t.map(&mut |x| x.fold(f))).collect()),
            Ty::Prim(_) | Ty::Param(_) | Ty::Never | Ty::Var(_) | Ty::Infer | Ty::Unknown => self.clone(),
        };
        f(&inner).unwrap_or(inner)
    }

    /// The type with every generic parameter in `map` replaced by its value.
    #[must_use]
    pub fn subst(&self, map: &BTreeMap<GenId, Ty>) -> Ty {
        if map.is_empty() {
            return self.clone();
        }
        self.fold(&mut |t| match t {
            Ty::Param(g) => map.get(g).cloned(),
            _ => None,
        })
    }

    /// How deep the type nests: 1 for a type with nothing inside it.
    #[must_use]
    pub fn depth(&self) -> usize {
        let inner = match self {
            Ty::Adt(_, args) | Ty::Tuple(args) | Ty::FnDef(_, args) => {
                args.iter().map(Ty::depth).max().unwrap_or(0)
            }
            Ty::Ref(inner, _) | Ty::Slice(inner) | Ty::Array(inner) | Ty::Assoc(inner, ..) => inner.depth(),
            Ty::FnPtr(params, ret) => params.iter().map(Ty::depth).max().unwrap_or(0).max(ret.depth()),
            Ty::Closure(k) => k.subst.iter().map(Ty::depth).max().unwrap_or(0),
            Ty::Dyn(traits) | Ty::Opaque(traits) => traits
                .iter()
                .flat_map(|t| t.args.iter().chain(t.bindings.iter().map(|(_, b)| b)))
                .map(Ty::depth)
                .max()
                .unwrap_or(0),
            Ty::Prim(_) | Ty::Param(_) | Ty::Never | Ty::Var(_) | Ty::Infer | Ty::Unknown => 0,
        };
        inner.saturating_add(1)
    }

    /// The type with every part that starts deeper than [`MAX_DEPTH`] replaced by
    /// [`Ty::Unknown`].
    #[must_use]
    pub fn cut(&self) -> Ty {
        self.cut_at(0)
    }

    fn cut_at(&self, depth: usize) -> Ty {
        if depth >= MAX_DEPTH {
            return Ty::Unknown;
        }
        let d = depth.saturating_add(1);
        let all = |args: &[Ty]| args.iter().map(|a| a.cut_at(d)).collect::<Vec<_>>();
        match self {
            Ty::Adt(id, args) => Ty::Adt(*id, all(args)),
            Ty::Tuple(items) => Ty::Tuple(all(items)),
            Ty::FnDef(n, args) => Ty::FnDef(*n, all(args)),
            Ty::Ref(inner, m) => Ty::Ref(Box::new(inner.cut_at(d)), *m),
            Ty::Slice(inner) => Ty::Slice(Box::new(inner.cut_at(d))),
            Ty::Array(inner) => Ty::Array(Box::new(inner.cut_at(d))),
            Ty::Assoc(inner, tr, name) => Ty::Assoc(
                Box::new(inner.cut_at(d)),
                tr.as_ref().map(|t| Box::new(t.map(&mut |x| x.cut_at(d)))),
                name.clone(),
            ),
            Ty::FnPtr(params, ret) => Ty::FnPtr(all(params), Box::new(ret.cut_at(d))),
            Ty::Closure(k) => Ty::Closure(Box::new(ClosureKey {
                owner: k.owner,
                subst: all(&k.subst),
                index: k.index,
            })),
            Ty::Dyn(traits) => Ty::Dyn(traits.iter().map(|t| t.map(&mut |x| x.cut_at(d))).collect()),
            Ty::Opaque(traits) => Ty::Opaque(traits.iter().map(|t| t.map(&mut |x| x.cut_at(d))).collect()),
            Ty::Prim(_) | Ty::Param(_) | Ty::Never | Ty::Var(_) | Ty::Infer | Ty::Unknown => self.clone(),
        }
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
    fn substitution_reaches_closures_function_items_and_bindings() {
        let g = GenId(4);
        let map = BTreeMap::from([(g, adt(9, Vec::new()))]);
        let t = Ty::Tuple(vec![
            Ty::FnDef(NodeId(1), vec![Ty::Param(g)]),
            Ty::Closure(Box::new(ClosureKey {
                owner: NodeId(1),
                subst: vec![Ty::Param(g)],
                index: 3,
            })),
            Ty::Dyn(vec![TraitRef {
                id: ItemId(2),
                args: Vec::new(),
                bindings: vec![("Item".to_owned(), Ty::Param(g))],
            }]),
        ]);
        assert!(t.has_param());
        let s = t.subst(&map);
        assert!(!s.has_param());
        assert!(s.any(&|x| *x == adt(9, Vec::new())));
    }

    #[test]
    fn cutting_bounds_depth_with_unknown() {
        let mut t = adt(1, Vec::new());
        for _ in 0..30 {
            t = adt(2, vec![t]);
        }
        assert!(t.depth() > MAX_DEPTH);
        let c = t.cut();
        assert!(c.depth() <= MAX_DEPTH + 1, "{}", c.depth());
        assert!(c.has_unknown());
        assert_eq!(adt(3, vec![Ty::Prim(Prim::U8)]).cut(), adt(3, vec![Ty::Prim(Prim::U8)]));
    }

    #[test]
    fn primitives_round_trip_their_names() {
        for (p, n) in PRIMS {
            assert_eq!(Prim::from_name(n), Some(p));
            assert_eq!(p.name(), n);
        }
        assert!(Prim::U64.is_int() && !Prim::U64.is_float());
        assert!(Prim::F64.is_float() && !Prim::F64.is_int());
        assert!(!Prim::Bool.is_int());
        assert_eq!(Prim::from_name("String"), None);
    }

    #[test]
    fn peel_and_walk_see_through_references() {
        let r = Ty::reference(Ty::reference(Ty::Prim(Prim::F64)));
        assert_eq!(r.peel(), &Ty::Prim(Prim::F64));
        let v = Ty::Tuple(vec![Ty::Var(3), Ty::Unknown]);
        assert!(v.has_var() && v.has_unknown());
        assert!(!Ty::unit().has_var());
    }
}
