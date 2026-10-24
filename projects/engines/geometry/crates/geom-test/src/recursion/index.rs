//! What a call may reach: the recursion check's tables of implementations and its method and
//! trait dispatch.
//!
//! What it does: [`Index::new`] resolves every implementation's self type and trait, names every
//! node ([`Index::names`]: `geom_num::wide::I256::mul`, `<geom_num::wide::I256 as Mul>::mul`,
//! `<geom_num::sign::Sign as Clone>::clone` for a derived one), and files implementations by the
//! kind of their self type ([`Key`]) and by trait. [`Index::method`] answers which kernel
//! functions `receiver.name(...)` may run, given what is known of the receiver;
//! [`Index::dispatch`] which ones a call of one trait's method on a value may run (operators,
//! formatting, derives, `?`); and [`Index::callbacks`] which kernel code outside code may run when
//! it is handed a value: the outside-trait implementations of the kernel types the value carries,
//! the closures and functions it carries, and the implementations of its generic parameters'
//! bounds.
//!
//! Guarantees:
//!
//! - **Over-approximate, never under.** When the receiver's type is known, the candidates are the
//!   inherent methods, trait implementations, trait default bodies, derived implementations and
//!   blanket implementations that can apply to it, followed through `Deref`; when it is a generic
//!   parameter or trait object, every implementation of its bounds; when it is unknown, every
//!   method of that name on the kernel types it may carry and on outside types. A method that may
//!   come from outside the kernel is flagged ([`Lookup::outside`]) so the caller adds the
//!   callbacks of outside code.
//! - **Outside code calls only what it is handed.** The callbacks of an outside call are exactly
//!   what Rust lets outside code reach through its arguments: trait implementations of the types
//!   in them (outside code knows only outside traits and the bounds of generics), and the
//!   closures and functions in them. Outside code that names kernel functions directly does not
//!   exist: kernel crates depend only on `core`, `alloc`, `std` and `libm`.
//! - **Deterministic.** Every table is ordered, so candidates come in the same order every run.
//!
//! Method: Rust's method-call resolution (receiver autoderef, inherent before trait methods,
//! which this keeps both of) and trait dispatch, from The Rust Reference
//! (<https://doc.rust-lang.org/reference/expressions/method-call-expr.html>,
//! <https://doc.rust-lang.org/reference/items/implementations.html>,
//! <https://doc.rust-lang.org/reference/attributes/derive.html>); class hierarchy analysis for
//! dynamic dispatch (J. Dean, D. Grove and C. Chambers, "Optimization of object-oriented programs
//! using static class hierarchy analysis", ECOOP 1995, <https://doi.org/10.1007/3-540-49538-X_5>),
//! with a trait's implementations as the hierarchy.
//!
//! Cost: building is linear in the number of implementations and nodes; each lookup is linear in
//! the candidates it returns, plus a walk of at most [`DEREF_DEPTH`] `Deref` steps.

use std::collections::{BTreeMap, BTreeSet};

use quote::ToTokens;

use crate::recursion::model::{DERIVABLE, ImplId, ItemKind, NodeKind, Program, SelfRef};
use crate::recursion::resolve::Resolver;
use crate::recursion::ty::{Carries, GenId, ItemId, NodeId, TraitRef, Ty, Val};

/// How many `Deref` steps a method lookup follows.
pub const DEREF_DEPTH: usize = 8;

/// The kind of a self type, for filing implementations.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Key {
    /// A kernel struct, enum or union.
    Adt(ItemId),
    /// An outside type, by name.
    Ext(String),
    /// A primitive type.
    Prim(String),
    /// Slices.
    Slice,
    /// Arrays.
    Array,
    /// Tuples.
    Tuple,
    /// Function pointers, closures and function items.
    Callable,
    /// Trait objects and `impl Trait`.
    Dyn,
    /// A generic parameter: a blanket implementation, for every type its bounds allow.
    Blanket,
}

/// The integer primitives, which an unsuffixed integer literal may be.
pub const INTEGERS: [&str; 12] = [
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
];

/// The key of a type: references looked through.
#[must_use]
pub fn key(ty: &Ty) -> Key {
    match ty.peel() {
        Ty::Adt(id, _) => Key::Adt(*id),
        Ty::Ext(name, _) => Key::Ext(name.clone()),
        Ty::Prim(p) => Key::Prim(p.clone()),
        Ty::Slice(_) => Key::Slice,
        Ty::Array(_) => Key::Array,
        Ty::Tuple(_) => Key::Tuple,
        Ty::FnPtr(_) | Ty::Closure(_) | Ty::FnDef(_) => Key::Callable,
        Ty::Dyn(_) => Key::Dyn,
        Ty::Param(_) | Ty::Assoc(..) | Ty::Never | Ty::Unknown | Ty::Ref(_) => Key::Blanket,
    }
}

/// Which kernel functions a call may run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lookup {
    /// The kernel functions.
    pub nodes: BTreeSet<NodeId>,
    /// Whether the method may be one outside the kernel, whose callbacks the caller adds.
    pub outside: bool,
}

/// The program's implementations, filed for lookup, and its nodes' names.
pub struct Index<'p> {
    /// The resolver.
    pub resolver: Resolver<'p>,
    /// The program.
    pub program: &'p Program,
    /// Each implementation's self type.
    pub impl_self: Vec<Ty>,
    /// Each implementation's trait, for a trait implementation.
    pub impl_trait: Vec<Option<TraitRef>>,
    /// Implementations by the key of their self type.
    pub by_key: BTreeMap<Key, Vec<ImplId>>,
    /// Implementations of each outside trait, by its name.
    pub by_ext_trait: BTreeMap<String, Vec<ImplId>>,
    /// Implementations of each kernel trait.
    pub by_kernel_trait: BTreeMap<ItemId, Vec<ImplId>>,
    /// The derived implementations of each item: trait name and node.
    pub derived: BTreeMap<ItemId, Vec<(String, NodeId)>>,
    /// Every node's name, by node id.
    pub names: Vec<String>,
}

impl<'p> Index<'p> {
    /// Files `program`'s implementations and names its nodes.
    #[must_use]
    pub fn new(program: &'p Program) -> Index<'p> {
        let resolver = Resolver::new(program);
        let mut impl_self = Vec::new();
        let mut impl_trait = Vec::new();
        let mut by_key: BTreeMap<Key, Vec<ImplId>> = BTreeMap::new();
        let mut by_ext_trait: BTreeMap<String, Vec<ImplId>> = BTreeMap::new();
        let mut by_kernel_trait: BTreeMap<ItemId, Vec<ImplId>> = BTreeMap::new();
        for (i, imp) in program.impls.iter().enumerate() {
            let id = ImplId(i);
            let mut scope = imp.scope.clone();
            scope.self_ref = SelfRef::None;
            let self_ty = resolver.ty(&scope, &imp.self_ty);
            let tr = imp
                .trait_path
                .as_ref()
                .and_then(|p| resolver.trait_ref(&imp.scope, p));
            by_key.entry(key(&self_ty)).or_default().push(id);
            match &tr {
                Some(TraitRef::Ext(name, _)) => by_ext_trait.entry(name.clone()).or_default().push(id),
                Some(TraitRef::Kernel(t, _)) => by_kernel_trait.entry(*t).or_default().push(id),
                None => {}
            }
            impl_self.push(self_ty);
            impl_trait.push(tr);
        }
        let mut derived: BTreeMap<ItemId, Vec<(String, NodeId)>> = BTreeMap::new();
        for (n, node) in program.nodes.iter().enumerate() {
            if let NodeKind::Derived { item, trait_name } = &node.kind {
                derived
                    .entry(*item)
                    .or_default()
                    .push((trait_name.clone(), NodeId(n)));
            }
        }
        let mut index = Index {
            resolver,
            program,
            impl_self,
            impl_trait,
            by_key,
            by_ext_trait,
            by_kernel_trait,
            derived,
            names: Vec::new(),
        };
        index.names = (0..program.nodes.len()).map(|n| index.name_of(NodeId(n))).collect();
        index
    }

    /// A type as names show it: a kernel type by its path, others as written.
    #[must_use]
    pub fn show(&self, ty: &Ty) -> String {
        match ty {
            Ty::Adt(id, _) => self.program.item(*id).path.clone(),
            Ty::Ext(name, _) | Ty::Prim(name) => name.clone(),
            Ty::Ref(inner) => format!("&{}", self.show(inner)),
            Ty::Slice(inner) => format!("[{}]", self.show(inner)),
            Ty::Array(inner) => format!("[{}; _]", self.show(inner)),
            Ty::Tuple(items) => format!(
                "({})",
                items.iter().map(|t| self.show(t)).collect::<Vec<_>>().join(", ")
            ),
            Ty::Param(g) => self.program.generic(*g).name.clone(),
            Ty::Dyn(traits) => format!(
                "dyn {}",
                traits.iter().map(|t| self.trait_name(t)).collect::<Vec<_>>().join(" + ")
            ),
            Ty::Assoc(base, name) => format!("{}::{name}", self.show(base)),
            Ty::FnPtr(n) => format!("fn/{n}"),
            Ty::Closure(_) => "closure".to_owned(),
            Ty::FnDef(_) => "fn item".to_owned(),
            Ty::Never => "!".to_owned(),
            Ty::Unknown => "_".to_owned(),
        }
    }

    /// A trait's name: its last path segment.
    #[must_use]
    pub fn trait_name(&self, tr: &TraitRef) -> String {
        match tr {
            TraitRef::Kernel(id, _) => self.program.item(*id).name.clone(),
            TraitRef::Ext(name, _) => name.clone(),
        }
    }

    fn name_of(&self, id: NodeId) -> String {
        let Some(node) = self.program.node(id) else {
            return String::new();
        };
        match &node.kind {
            NodeKind::Fn | NodeKind::TraitDefault(_) => node.name.clone(),
            NodeKind::Method(imp) => {
                let self_ty = self
                    .impl_self
                    .get(imp.0)
                    .map(|t| self.show(t))
                    .filter(|s| s != "_")
                    .unwrap_or_else(|| self.program.imp(*imp).self_ty.to_token_stream().to_string());
                match self.impl_trait.get(imp.0).and_then(Option::as_ref) {
                    Some(tr) => format!("<{self_ty} as {}>::{}", self.trait_name(tr), node.ident),
                    None => format!("{self_ty}::{}", node.ident),
                }
            }
            NodeKind::Derived { item, trait_name } => format!(
                "<{} as {trait_name}>::{}",
                self.program.item(*item).path,
                node.ident
            ),
        }
    }

    /// The name of node `id`, for a node the program made.
    #[must_use]
    pub fn name(&self, id: NodeId) -> &str {
        self.names.get(id.0).map_or("", String::as_str)
    }

    /// The implementations whose self type has key `k`, and the blanket ones; an unsuffixed
    /// integer or float literal's key matches every integer or float primitive.
    fn impls_for(&self, k: &Key) -> Vec<ImplId> {
        let mut keys: Vec<Key> = vec![k.clone(), Key::Blanket];
        if let Key::Prim(p) = k {
            if p == "int" {
                keys.extend(INTEGERS.iter().map(|i| Key::Prim((*i).to_owned())));
            } else if p == "float" {
                keys.extend(["f32", "f64"].iter().map(|f| Key::Prim((*f).to_owned())));
            }
        }
        let mut out: Vec<ImplId> = keys
            .iter()
            .filter_map(|k| self.by_key.get(k))
            .flatten()
            .copied()
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// The methods named `name` of implementation `imp`.
    fn impl_methods(&self, imp: ImplId, name: &str) -> Vec<NodeId> {
        self.program.imp(imp).methods.get(name).cloned().unwrap_or_default()
    }

    /// Default bodies of trait `tr` named `name` that implementation `imp` does not override.
    fn defaults_used(&self, imp: ImplId, name: &str) -> Vec<NodeId> {
        let Some(Some(TraitRef::Kernel(t, _))) = self.impl_trait.get(imp.0) else {
            return Vec::new();
        };
        if self.program.imp(imp).methods.contains_key(name) {
            return Vec::new();
        }
        self.trait_default(*t, name).into_iter().collect()
    }

    /// The default body of the kernel trait `t`'s method `name`, or of a supertrait's.
    fn trait_default(&self, t: ItemId, name: &str) -> Option<NodeId> {
        let traits = self
            .resolver
            .with_supertraits(vec![TraitRef::Kernel(t, Vec::new())]);
        traits.iter().find_map(|tr| match tr {
            TraitRef::Kernel(id, _) => match &self.program.item(*id).kind {
                ItemKind::Trait { defaults, .. } => defaults.get(name).copied(),
                _ => None,
            },
            TraitRef::Ext(..) => None,
        })
    }

    /// The derived implementation nodes of `item` that provide `name`.
    fn derived_providing(&self, item: ItemId, name: &str) -> Vec<NodeId> {
        self.derived
            .get(&item)
            .map(|list| {
                list.iter()
                    .filter(|(t, _)| {
                        DERIVABLE
                            .iter()
                            .any(|(d, methods)| d == t && methods.contains(&name))
                    })
                    .map(|(_, n)| *n)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Which kernel functions `receiver.name(...)` may run.
    #[must_use]
    pub fn method(&self, receiver: &Val, name: &str) -> Lookup {
        let mut out = Lookup::default();
        let mut ty = receiver.ty.clone();
        for _ in 0..DEREF_DEPTH {
            let peeled = ty.peel().clone();
            match &peeled {
                Ty::Adt(id, _) => {
                    let found = self.on_key(&Key::Adt(*id), name);
                    if !found.is_empty() {
                        out.nodes.extend(found);
                        return out;
                    }
                    if let Some(target) = self.deref_target(&peeled) {
                        ty = target;
                        continue;
                    }
                    out.outside = true;
                    return out;
                }
                Ty::Param(g) => {
                    out.nodes.extend(self.bound_methods(*g, name));
                    out.outside = true;
                    return out;
                }
                Ty::Dyn(traits) => {
                    for tr in self.resolver.with_supertraits(traits.clone()) {
                        out.nodes.extend(self.trait_methods(&tr, name, None));
                    }
                    out.outside = true;
                    return out;
                }
                Ty::Unknown | Ty::Assoc(..) | Ty::Never => {
                    out.nodes.extend(self.by_name_on(&receiver.carries, name));
                    out.outside = true;
                    return out;
                }
                other => {
                    out.nodes.extend(self.on_key(&key(other), name));
                    out.outside = true;
                    return out;
                }
            }
        }
        out.outside = true;
        out
    }

    /// Every method `name` that a value with key `k` has in the kernel: inherent, trait
    /// implementations, trait defaults, derived and blanket.
    fn on_key(&self, k: &Key, name: &str) -> BTreeSet<NodeId> {
        let mut out = BTreeSet::new();
        for imp in self.impls_for(k) {
            out.extend(self.impl_methods(imp, name));
            out.extend(self.defaults_used(imp, name));
        }
        if let Key::Adt(id) = k {
            out.extend(self.derived_providing(*id, name));
        }
        out
    }

    /// `Deref::Target` of a kernel type with a `Deref` implementation.
    fn deref_target(&self, ty: &Ty) -> Option<Ty> {
        let Ty::Adt(id, _) = ty else {
            return None;
        };
        for imp in self.by_ext_trait.get("Deref")? {
            if key(self.impl_self.get(imp.0)?) == Key::Adt(*id) {
                let target = self.program.imp(*imp).assoc.get("Target")?;
                return Some(self.resolver.ty(&self.program.imp(*imp).scope, target));
            }
        }
        None
    }

    /// The methods `name` a value of generic parameter `g` may run: those of its bounds.
    fn bound_methods(&self, g: GenId, name: &str) -> BTreeSet<NodeId> {
        let mut out = BTreeSet::new();
        let bounds = self.resolver.bounds(g);
        for tr in &bounds {
            out.extend(self.trait_methods(tr, name, None));
        }
        if out.is_empty() && bounds.iter().all(|t| matches!(t, TraitRef::Ext(..))) {
            // A method the bounds do not show (a supertrait the resolver cannot see): every
            // method of that name.
            out.extend(self.by_name_on(&Carries::top(), name));
        }
        out
    }

    /// The methods `name` of every implementation of `tr` (kernel trait: with its default; outside
    /// trait: with the derived implementations when `tr` is derivable), for self types of key
    /// `only` when given.
    #[must_use]
    pub fn trait_methods(&self, tr: &TraitRef, name: &str, only: Option<&Key>) -> BTreeSet<NodeId> {
        let mut out = BTreeSet::new();
        let impls: Vec<ImplId> = match tr {
            TraitRef::Kernel(t, _) => {
                if let Some(d) = self.trait_default(*t, name) {
                    out.insert(d);
                }
                self.by_kernel_trait.get(t).cloned().unwrap_or_default()
            }
            TraitRef::Ext(n, _) => {
                for (item, list) in &self.derived {
                    if only.is_some_and(|k| *k != Key::Adt(*item) && *k != Key::Blanket) {
                        continue;
                    }
                    for (t, node) in list {
                        if t == n {
                            out.insert(*node);
                        }
                    }
                }
                self.by_ext_trait.get(n).cloned().unwrap_or_default()
            }
        };
        for imp in impls {
            if let Some(k) = only
                && *k != Key::Blanket
                && let Some(self_ty) = self.impl_self.get(imp.0)
            {
                let ik = key(self_ty);
                if ik != *k && ik != Key::Blanket {
                    continue;
                }
            }
            out.extend(self.impl_methods(imp, name));
        }
        out
    }

    /// Every method `name` of the kernel types in `carries`, and of implementations for outside
    /// types and blanket ones; with `top`, every method of that name.
    fn by_name_on(&self, carries: &Carries, name: &str) -> BTreeSet<NodeId> {
        let mut out = BTreeSet::new();
        for (k, impls) in &self.by_key {
            let take = match k {
                Key::Adt(id) => carries.top || carries.types.contains(id),
                _ => true,
            };
            if !take {
                continue;
            }
            for imp in impls {
                out.extend(self.impl_methods(*imp, name));
                out.extend(self.defaults_used(*imp, name));
            }
        }
        for (item, _) in &self.derived {
            if carries.top || carries.types.contains(item) {
                out.extend(self.derived_providing(*item, name));
            }
        }
        for g in &carries.generics {
            out.extend(self.bound_methods_shallow(*g, name));
        }
        if carries.top {
            for item in &self.program.items {
                if let ItemKind::Trait { defaults, .. } = &item.kind
                    && let Some(d) = defaults.get(name)
                {
                    out.insert(*d);
                }
            }
        }
        out
    }

    fn bound_methods_shallow(&self, g: GenId, name: &str) -> BTreeSet<NodeId> {
        self.resolver
            .bounds(g)
            .iter()
            .flat_map(|tr| self.trait_methods(tr, name, None))
            .collect()
    }

    /// Which kernel functions a call of trait `trait_name`'s method `method` on `value` may run:
    /// the implementation for its type, or, for an outside type, the same trait's implementations
    /// for the kernel types it carries (as the standard library's implementations for containers
    /// call their elements'), or for a generic parameter or unknown value every implementation it
    /// may reach.
    #[must_use]
    pub fn dispatch(&self, value: &Val, trait_name: &str, method: &str) -> BTreeSet<NodeId> {
        let tr = TraitRef::Ext(trait_name.to_owned(), Vec::new());
        let mut out = BTreeSet::new();
        match value.ty.peel() {
            Ty::Adt(id, _) => {
                out.extend(self.trait_methods(&tr, method, Some(&Key::Adt(*id))));
                if out.is_empty() && trait_name == "Deref" {
                    return out;
                }
            }
            Ty::Param(_) | Ty::Dyn(_) | Ty::Unknown | Ty::Assoc(..) if value.carries.top => {
                out.extend(self.trait_methods(&tr, method, None));
            }
            Ty::Param(_) | Ty::Dyn(_) => out.extend(self.trait_methods(&tr, method, None)),
            other => {
                let k = key(other);
                if k != Key::Blanket {
                    out.extend(self.trait_methods(&tr, method, Some(&k)));
                }
                for t in &value.carries.types {
                    out.extend(self.trait_methods(&tr, method, Some(&Key::Adt(*t))));
                }
                if !value.carries.generics.is_empty() {
                    out.extend(self.trait_methods(&tr, method, None));
                }
            }
        }
        out
    }

    /// The kernel code outside code may run when handed values carrying `carries`: every
    /// outside-trait implementation (and derived one) of the kernel types carried, the closures,
    /// functions and carriers carried, and every implementation of the carried generic
    /// parameters' bounds. `top` is the caller's to handle (it reaches everything).
    #[must_use]
    pub fn callbacks(&self, carries: &Carries) -> BTreeSet<NodeId> {
        let mut out: BTreeSet<NodeId> = carries.calls.clone();
        for t in &carries.types {
            out.extend(self.outside_trait_methods(*t));
        }
        for g in &carries.generics {
            for tr in self.resolver.bounds(*g) {
                if tr.fn_arity().is_some() {
                    continue;
                }
                out.extend(self.all_trait_methods(&tr));
            }
        }
        if carries.top {
            for t in 0..self.program.items.len() {
                out.extend(self.outside_trait_methods(ItemId(t)));
            }
        }
        out
    }

    /// Every method of every implementation of an outside trait for `t`, and its derived ones.
    #[must_use]
    pub fn outside_trait_methods(&self, t: ItemId) -> BTreeSet<NodeId> {
        let mut out = BTreeSet::new();
        for imp in self.by_key.get(&Key::Adt(t)).into_iter().flatten() {
            if matches!(self.impl_trait.get(imp.0), Some(Some(TraitRef::Ext(..)))) {
                out.extend(self.program.imp(*imp).methods.values().flatten().copied());
            }
        }
        if let Some(list) = self.derived.get(&t) {
            out.extend(list.iter().map(|(_, n)| *n));
        }
        out
    }

    /// Every method of every implementation of `tr`, and its defaults (kernel) or derived
    /// implementations (outside).
    #[must_use]
    pub fn all_trait_methods(&self, tr: &TraitRef) -> BTreeSet<NodeId> {
        let mut out = BTreeSet::new();
        match tr {
            TraitRef::Kernel(t, _) => {
                if let ItemKind::Trait { defaults, .. } = &self.program.item(*t).kind {
                    out.extend(defaults.values().copied());
                }
                for imp in self.by_kernel_trait.get(t).into_iter().flatten() {
                    out.extend(self.program.imp(*imp).methods.values().flatten().copied());
                }
            }
            TraitRef::Ext(n, _) => {
                for imp in self.by_ext_trait.get(n).into_iter().flatten() {
                    out.extend(self.program.imp(*imp).methods.values().flatten().copied());
                }
                for list in self.derived.values() {
                    out.extend(list.iter().filter(|(t, _)| t == n).map(|(_, node)| *node));
                }
            }
        }
        out
    }

    /// The `method` nodes of the kernel's implementations of the outside trait `trait_name` whose
    /// first trait argument (`From<S>`'s `S`) may be `source` and whose self type may be
    /// `target`, each given when known.
    #[must_use]
    pub fn conversions(
        &self,
        trait_name: &str,
        method: &str,
        source: Option<&Val>,
        target: Option<&Ty>,
    ) -> BTreeSet<NodeId> {
        let mut out = BTreeSet::new();
        for imp in self.by_ext_trait.get(trait_name).into_iter().flatten() {
            if let Some(t) = target.filter(|t| t.is_known())
                && let Some(self_ty) = self.impl_self.get(imp.0)
                && key(self_ty) != key(t)
                && key(self_ty) != Key::Blanket
            {
                continue;
            }
            if let Some(src) = source
                && let Some(Some(TraitRef::Ext(_, args))) = self.impl_trait.get(imp.0)
                && let Some(s) = args.first()
                && !self.may_be(s, src)
            {
                continue;
            }
            out.extend(self.impl_methods(*imp, method));
        }
        out
    }

    /// Whether a value `v` may have the type `pattern`.
    fn may_be(&self, pattern: &Ty, v: &Val) -> bool {
        let pk = key(pattern);
        if pk == Key::Blanket || v.carries.top {
            return true;
        }
        if v.ty.is_known() && !matches!(v.ty.peel(), Ty::Param(_) | Ty::Dyn(_) | Ty::Assoc(..)) {
            let vk = key(&v.ty);
            if let (Key::Prim(a), Key::Prim(b)) = (&pk, &vk) {
                return a == b
                    || (b == "int" && INTEGERS.contains(&a.as_str()))
                    || (b == "float" && (a == "f32" || a == "f64"));
            }
            return pk == vk;
        }
        match pk {
            Key::Adt(id) => v.carries.types.contains(&id) || !v.carries.generics.is_empty(),
            _ => true,
        }
    }
}
