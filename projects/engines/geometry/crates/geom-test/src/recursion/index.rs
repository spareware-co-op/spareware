//! What a call may reach: the recursion check's tables of implementations, its type matching, and
//! its method and trait dispatch.
//!
//! What it does: [`Index::new`] resolves every implementation's self type and trait, every
//! function's signature in types ([`Sig`]), and every node's name ([`Index::name`]:
//! `geom_num::wide::I256::mul`, `<geom_num::wide::I256 as Mul>::mul`,
//! `<geom_num::sign::Sign as Clone>::clone` for a derived one). [`Index::impl_for`] says whether an
//! implementation applies to a type and with which generic arguments; [`Index::method`] which
//! kernel functions `receiver.name(...)` may run on a receiver of a given type, following
//! auto-dereference as the compiler does, and whether the method may instead be one from outside
//! the kernel; [`Index::assoc`] the same for `Type::name(...)`; [`Index::bound_impls`] every
//! implementation a call through a generic parameter's or trait object's bound may run;
//! [`Index::outside_impls`] every implementation of an outside trait that outside code handed a
//! value may call back; [`Index::normalize`] resolves associated types.
//!
//! Guarantees:
//!
//! - **Over-approximate, never under.** For a receiver of a known type, the candidates are the
//!   inherent methods, trait implementations (blanket ones included, when their bounds may hold),
//!   trait default bodies and derived implementations that can apply to it, at each step of
//!   auto-dereference until one is found; traits need not be in scope, so a method of any trait
//!   implemented for the type is a candidate. For an open generic parameter or a trait object,
//!   every implementation of its bounds' traits. For a [`Ty::Hidden`] value, every candidate of
//!   every type it may be built from, and every kernel implementation for an outside type. A
//!   method that may come from outside the kernel is reported as such, so the caller adds what
//!   outside code may call back.
//! - **Matching is the compiler's, loosened.** A type matches an implementation's self type unless
//!   their heads differ (another struct, another outside type, another primitive); a part the
//!   analysis does not know matches anything.
//! - **Deterministic.** Every table is ordered.
//!
//! Method: Rust's method-call resolution (receiver auto-dereference, inherent before trait
//! methods, which this keeps both of) and trait dispatch, from The Rust Reference
//! (<https://doc.rust-lang.org/reference/expressions/method-call-expr.html>,
//! <https://doc.rust-lang.org/reference/items/implementations.html>,
//! <https://doc.rust-lang.org/reference/attributes/derive.html>); class hierarchy analysis for
//! dynamic dispatch (J. Dean, D. Grove and C. Chambers, "Optimization of object-oriented programs
//! using static class hierarchy analysis", ECOOP 1995, <https://doi.org/10.1007/3-540-49538-X_5>),
//! with a trait's implementations as the hierarchy; first-order syntactic unification of type
//! terms (J. A. Robinson, "A machine-oriented logic based on the resolution principle", J. ACM 12
//! (1965), <https://doi.org/10.1145/321250.321253>).
//!
//! Cost: building is linear in the implementations and nodes; a lookup is linear in the
//! implementations, times at most [`DEREF_DEPTH`] auto-dereference steps; a blanket
//! implementation's bounds are checked to depth [`BOUND_DEPTH`]. Recursion follows the nesting of
//! the types matched.

use std::collections::{BTreeMap, BTreeSet};

use quote::ToTokens;

use crate::recursion::model::{DERIVABLE, ImplId, ItemKind, NodeKind, Program, Scope, SelfRef};
use crate::recursion::resolve::Resolver;
use crate::recursion::ty::{GenId, ItemId, NodeId, TraitRef, Ty, Val};

/// How many `Deref` steps a method lookup follows.
pub const DEREF_DEPTH: usize = 8;

/// How deep the bounds of blanket implementations are checked.
pub const BOUND_DEPTH: usize = 4;

/// Generic arguments, by parameter.
pub type Map = BTreeMap<GenId, Ty>;

/// A function's signature in types: its parameters (the receiver first, for a method that has
/// one) and its result, in the scope of the function, with its generic parameters open.
#[derive(Clone, Debug, Default)]
pub struct Sig {
    /// The parameter types, the receiver first when there is one.
    pub inputs: Vec<Ty>,
    /// Whether the first input is a receiver (`self`, `&self`, `self: T`).
    pub receiver: bool,
    /// The result type.
    pub output: Ty,
}

/// How a call's target was found, for the report.
pub mod how {
    /// A path that names the function.
    pub const PATH: &str = "a call by path";
    /// A method found on the receiver's type.
    pub const RECEIVER: &str = "a method call on the receiver's type";
    /// An implementation of a generic parameter's or trait object's bound.
    pub const BOUND: &str = "a call through a bound (every implementation of the trait)";
    /// A kernel implementation for an outside type, on a value whose type is not known.
    pub const OUTSIDE_TYPE: &str = "a kernel implementation for an outside type, on a value of unknown type";
    /// An implicit trait call: an operator, `?`, `for`, indexing, a dereference, formatting.
    pub const IMPLICIT: &str = "an implicit trait call (operator, ?, for, index, deref, formatting)";
    /// A derived implementation's call of its fields' implementations.
    pub const DERIVED: &str = "a derived implementation's call on a field";
    /// A call through a value holding a closure or function item.
    pub const VALUE: &str = "a call through a value (closure or function item)";
    /// A call through a trait object or function pointer: every callable coerced to one.
    pub const ERASED: &str = "a call through a trait object or function pointer (every callable coerced to one)";
    /// Outside code calling back a closure or function it was handed.
    pub const CALLBACK: &str = "a callback of outside code (a closure or function it was handed)";
    /// Outside code calling back a trait implementation of a kernel type it was handed.
    pub const CALLBACK_IMPL: &str = "a callback of outside code (a trait implementation of a type it was handed)";
    /// A standard conversion's call of the kernel implementation that builds its result.
    pub const CONVERSION: &str = "a standard conversion (into, collect, sum, default, ...)";
    /// Drop glue.
    pub const DROP: &str = "drop glue (a Drop implementation)";
}

/// A kernel function a call may run, with the generic arguments the dispatch fixed (an
/// implementation's, and `Self` of a trait default body), and how it was found.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Target {
    /// The function.
    pub node: NodeId,
    /// The generic arguments fixed so far.
    pub map: Map,
    /// How it was found.
    pub how: &'static str,
}

/// What a method lookup found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Found {
    /// The kernel functions.
    pub targets: BTreeSet<Target>,
    /// The receiver types at which the method may be one from outside the kernel.
    pub outside: BTreeSet<Ty>,
}

/// The program's implementations, filed for lookup, its signatures and its nodes' names.
pub struct Index<'p> {
    /// The resolver.
    pub resolver: Resolver<'p>,
    /// The program.
    pub program: &'p Program,
    /// Each implementation's self type, its generic parameters open.
    pub impl_self: Vec<Ty>,
    /// Each implementation's trait, for a trait implementation.
    pub impl_trait: Vec<Option<TraitRef>>,
    /// Implementations of each kernel trait.
    pub by_kernel_trait: BTreeMap<ItemId, Vec<ImplId>>,
    /// Implementations of each outside trait, by its name.
    pub by_ext_trait: BTreeMap<String, Vec<ImplId>>,
    /// The derived implementations of each item: trait name and node.
    pub derived: BTreeMap<ItemId, Vec<(String, NodeId)>>,
    /// Every node's generic parameters, as instances list their arguments.
    pub generics: Vec<Vec<GenId>>,
    /// Every node's signature (derived implementations get theirs from their trait).
    pub sigs: Vec<Sig>,
    /// Every node's name.
    pub names: Vec<String>,
}

impl<'p> Index<'p> {
    /// Files `program`'s implementations, reads its signatures and names its nodes.
    #[must_use]
    pub fn new(program: &'p Program) -> Index<'p> {
        let resolver = Resolver::new(program);
        let mut impl_self = Vec::new();
        let mut impl_trait = Vec::new();
        let mut by_kernel_trait: BTreeMap<ItemId, Vec<ImplId>> = BTreeMap::new();
        let mut by_ext_trait: BTreeMap<String, Vec<ImplId>> = BTreeMap::new();
        for (i, imp) in program.impls.iter().enumerate() {
            let id = ImplId(i);
            let mut scope = imp.scope.clone();
            scope.self_ref = SelfRef::None;
            let self_ty = resolver.ty(&scope, &imp.self_ty);
            let tr = imp
                .trait_path
                .as_ref()
                .and_then(|p| resolver.trait_ref(&imp.scope, p));
            match &tr {
                Some(TraitRef::Ext(name, _) | TraitRef::Fn(name, ..)) => {
                    by_ext_trait.entry(name.clone()).or_default().push(id);
                }
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
            by_kernel_trait,
            by_ext_trait,
            derived,
            generics: Vec::new(),
            sigs: Vec::new(),
            names: Vec::new(),
        };
        index.generics = (0..program.nodes.len())
            .map(|n| index.generics_of(NodeId(n)))
            .collect();
        index.sigs = (0..program.nodes.len()).map(|n| index.sig_of(NodeId(n))).collect();
        index.names = (0..program.nodes.len()).map(|n| index.name_of(NodeId(n))).collect();
        index
    }

    // ---------------------------------------------------------------- names

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
            Ty::Opaque(traits) => format!(
                "impl {}",
                traits.iter().map(|t| self.trait_name(t)).collect::<Vec<_>>().join(" + ")
            ),
            Ty::Assoc(base, name) => format!("{}::{name}", self.show(base)),
            Ty::FnPtr(n) => format!("fn/{n}"),
            Ty::Closure(k) => format!("{}::{{closure#{}}}", self.name(k.owner), k.index),
            Ty::FnDef(n, _) => self.name(*n).to_owned(),
            Ty::ExtFn => "an outside function".to_owned(),
            Ty::Never => "!".to_owned(),
            Ty::Infer | Ty::Hidden(_) => "_".to_owned(),
        }
    }

    /// A trait's name: its last path segment.
    #[must_use]
    pub fn trait_name(&self, tr: &TraitRef) -> String {
        match tr {
            TraitRef::Kernel(id, _) => self.program.item(*id).name.clone(),
            TraitRef::Ext(name, _) | TraitRef::Fn(name, ..) => name.clone(),
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
                    .unwrap_or_else(|| {
                        self.program.imp(*imp).self_ty.to_token_stream().to_string()
                    });
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

    /// The name of node `id`.
    #[must_use]
    pub fn name(&self, id: NodeId) -> &str {
        self.names.get(id.0).map_or("", String::as_str)
    }

    // ---------------------------------------------------------------- signatures

    /// Node `id`'s generic parameters in the order its instances list their arguments: for a trait
    /// default body, the trait's `Self` first; for a derived implementation, its type's.
    fn generics_of(&self, id: NodeId) -> Vec<GenId> {
        let Some(node) = self.program.node(id) else {
            return Vec::new();
        };
        match &node.kind {
            NodeKind::TraitDefault(t) => {
                let mut out = Vec::new();
                if let ItemKind::Trait { self_gen, .. } = &self.program.item(*t).kind {
                    out.push(*self_gen);
                }
                out.extend(node.scope.generics.iter().copied());
                out
            }
            NodeKind::Derived { item, .. } => self.program.item(*item).scope.generics.clone(),
            NodeKind::Fn | NodeKind::Method(_) => node.scope.generics.clone(),
        }
    }

    /// The generic parameters of node `id`'s instances.
    #[must_use]
    pub fn node_generics(&self, id: NodeId) -> &[GenId] {
        self.generics.get(id.0).map_or(&[], Vec::as_slice)
    }

    /// The map from node `id`'s generic parameters to `args`.
    #[must_use]
    pub fn map_of(&self, id: NodeId, args: &[Ty]) -> Map {
        self.node_generics(id)
            .iter()
            .copied()
            .zip(args.iter().cloned())
            .collect()
    }

    /// The `Self` type of node `id`, its generic parameters open: an implementation's self type,
    /// a trait's `Self` parameter, a derived implementation's type.
    #[must_use]
    pub fn self_ty(&self, id: NodeId) -> Option<Ty> {
        let node = self.program.node(id)?;
        match &node.kind {
            NodeKind::Method(imp) => self.impl_self.get(imp.0).cloned(),
            NodeKind::TraitDefault(t) => match &self.program.item(*t).kind {
                ItemKind::Trait { self_gen, .. } => Some(Ty::Param(*self_gen)),
                _ => None,
            },
            NodeKind::Derived { item, .. } => Some(self.adt_open(*item)),
            NodeKind::Fn => None,
        }
    }

    /// Kernel type `item` with its generic parameters open.
    #[must_use]
    pub fn adt_open(&self, item: ItemId) -> Ty {
        Ty::Adt(
            item,
            self.program
                .item(item)
                .scope
                .generics
                .iter()
                .map(|g| Ty::Param(*g))
                .collect(),
        )
    }

    fn sig_of(&self, id: NodeId) -> Sig {
        let Some(node) = self.program.node(id) else {
            return Sig::default();
        };
        if let NodeKind::Derived { trait_name, .. } = &node.kind {
            let own = self.self_ty(id).unwrap_or_else(Ty::unknown);
            let r = Ty::Ref(Box::new(own.clone()));
            let (inputs, output) = match trait_name.as_str() {
                "Clone" => (vec![r], own),
                "PartialEq" => (vec![r.clone(), r], Ty::prim("bool")),
                "PartialOrd" => (
                    vec![r.clone(), r],
                    Ty::ext("Option", vec![Ty::ext("Ordering", Vec::new())]),
                ),
                "Ord" => (vec![r.clone(), r], Ty::ext("Ordering", Vec::new())),
                "Hash" => (vec![r, Ty::unknown()], Ty::unit()),
                "Debug" => (
                    vec![r, Ty::Ref(Box::new(Ty::ext("Formatter", Vec::new())))],
                    Ty::ext("Result", vec![Ty::unit(), Ty::ext("Error", Vec::new())]),
                ),
                "Default" => (Vec::new(), own),
                _ => (vec![r], Ty::unknown()),
            };
            let receiver = !inputs.is_empty();
            return Sig {
                inputs,
                receiver,
                output,
            };
        }
        let Some(sig) = &node.sig else {
            return Sig::default();
        };
        self.sig_types(&node.scope, sig, self.self_ty(id).as_ref())
    }

    /// A written signature in types, in `scope`, with `Self` as `self_ty`.
    #[must_use]
    pub fn sig_types(&self, scope: &Scope, sig: &syn::Signature, self_ty: Option<&Ty>) -> Sig {
        let mut inputs = Vec::new();
        let mut receiver = false;
        for input in &sig.inputs {
            match input {
                syn::FnArg::Receiver(r) => {
                    receiver = true;
                    let own = self_ty.cloned().unwrap_or_else(Ty::unknown);
                    let t = match &r.kind {
                        syn::ReceiverKind::Typed(_, t) => self.resolver.ty(scope, t),
                        syn::ReceiverKind::Reference(..) => Ty::Ref(Box::new(own)),
                        _ => own,
                    };
                    inputs.push(t);
                }
                syn::FnArg::Typed(pt) => inputs.push(self.resolver.ty(scope, &pt.ty)),
            }
        }
        let output = match &sig.output {
            syn::ReturnType::Default => Ty::unit(),
            syn::ReturnType::Type(_, t) => self.resolver.ty(scope, t),
        };
        Sig {
            inputs,
            receiver,
            output,
        }
    }

    /// Node `id`'s signature.
    #[must_use]
    pub fn sig(&self, id: NodeId) -> Option<&Sig> {
        self.sigs.get(id.0)
    }

    /// The signature of kernel trait `t`'s method `name`, with `Self` as the trait's parameter,
    /// and its scope's generic parameters; from the trait or one of its supertraits.
    #[must_use]
    pub fn trait_method_sig(&self, t: ItemId, name: &str) -> Option<(Sig, Vec<GenId>)> {
        for tr in self
            .resolver
            .with_supertraits(vec![TraitRef::Kernel(t, Vec::new())])
        {
            let TraitRef::Kernel(id, _) = tr else {
                continue;
            };
            if let ItemKind::Trait { sigs, self_gen, .. } = &self.program.item(id).kind
                && let Some((sig, scope)) = sigs.get(name)
            {
                let own = Ty::Param(*self_gen);
                let mut generics = vec![*self_gen];
                generics.extend(scope.generics.iter().copied());
                return Some((self.sig_types(scope, sig, Some(&own)), generics));
            }
        }
        None
    }

    // ---------------------------------------------------------------- fields

    /// The fields of kernel type `item` (of variant `variant` for an enum), each with its type in
    /// terms of the item's generic parameters.
    #[must_use]
    pub fn fields(&self, item: ItemId, variant: Option<usize>) -> Vec<(String, Ty)> {
        let it = self.program.item(item);
        let fields = match (&it.kind, variant) {
            (ItemKind::Struct { fields, .. } | ItemKind::Union { fields, .. }, _) => fields,
            (ItemKind::Enum { variants, .. }, Some(v)) => match variants.get(v) {
                Some(var) => &var.fields,
                None => return Vec::new(),
            },
            _ => return Vec::new(),
        };
        fields
            .all()
            .into_iter()
            .map(|(n, t)| (n, self.resolver.ty(&it.scope, t)))
            .collect()
    }

    /// Every field type of every variant of kernel type `item`, in terms of its parameters.
    #[must_use]
    pub fn all_field_types(&self, item: ItemId) -> Vec<Ty> {
        match &self.program.item(item).kind {
            ItemKind::Enum { variants, .. } => (0..variants.len())
                .flat_map(|v| self.fields(item, Some(v)))
                .map(|(_, t)| t)
                .collect(),
            _ => self.fields(item, None).into_iter().map(|(_, t)| t).collect(),
        }
    }

    /// The type of field `name` of a value of type `ty`: through references, boxes and kernel
    /// `Deref` implementations; a tuple's element; for a [`Ty::Hidden`], the field of every kernel
    /// type it may be built from, and the hidden value itself.
    #[must_use]
    pub fn field(&self, ty: &Ty, name: &str) -> Vec<Ty> {
        let mut out = Vec::new();
        for step in self.deref_steps(ty) {
            match &step {
                Ty::Adt(id, args) => {
                    let map: Map = self
                        .program
                        .item(*id)
                        .scope
                        .generics
                        .iter()
                        .copied()
                        .zip(args.iter().cloned())
                        .collect();
                    if let Some((_, t)) = self.fields(*id, None).iter().find(|(n, _)| n == name) {
                        out.push(t.subst(&map));
                        return out;
                    }
                }
                Ty::Tuple(items) => {
                    if let Some(t) = name.parse::<usize>().ok().and_then(|i| items.get(i)) {
                        out.push(t.clone());
                        return out;
                    }
                }
                Ty::Hidden(parts) => {
                    for p in parts {
                        if let Ty::Adt(..) = p {
                            out.extend(self.field(p, name));
                        }
                    }
                    out.push(step.clone());
                    return out;
                }
                Ty::Param(_) | Ty::Assoc(..) => {
                    out.push(Ty::Hidden(step.parts()));
                    return out;
                }
                _ => {}
            }
        }
        out.push(Ty::Hidden(ty.parts()));
        out
    }

    // ---------------------------------------------------------------- matching

    /// Matches `pattern` (whose parameters in `vars` are unknowns) against `actual`, adding each
    /// unknown's values to `out`. Heads that differ add nothing; references on one side only are
    /// looked through; a [`Ty::Hidden`] actual gives every unknown in its pattern the hidden
    /// value, besides what its parts match.
    pub fn unify(&self, pattern: &Ty, actual: &Ty, vars: &BTreeSet<GenId>, out: &mut BTreeMap<GenId, Val>) {
        self.unify_at(pattern, actual, vars, out, 0);
    }

    fn unify_at(
        &self,
        pattern: &Ty,
        actual: &Ty,
        vars: &BTreeSet<GenId>,
        out: &mut BTreeMap<GenId, Val>,
        depth: usize,
    ) {
        if depth > crate::recursion::ty::MAX_DEPTH.saturating_mul(2) {
            return;
        }
        let d = depth.saturating_add(1);
        match (pattern, actual) {
            (Ty::Param(g), a) if vars.contains(g) => {
                if !matches!(a, Ty::Infer | Ty::Never) {
                    out.entry(*g).or_default().add(a.clone());
                }
            }
            (_, Ty::Hidden(parts)) => {
                let mut free = BTreeSet::new();
                pattern.walk(&mut |t| {
                    if let Ty::Param(g) = t
                        && vars.contains(g)
                    {
                        free.insert(*g);
                    }
                });
                for g in free {
                    out.entry(g).or_default().add(actual.clone());
                }
                for p in parts {
                    self.unify_at(pattern, p, vars, out, d);
                }
            }
            (Ty::Ref(p), Ty::Ref(a)) => self.unify_at(p, a, vars, out, d),
            (Ty::Ref(p), a) => self.unify_at(p, a, vars, out, d),
            (p, Ty::Ref(a)) => self.unify_at(p, a, vars, out, d),
            (Ty::Adt(x, xs), Ty::Adt(y, ys)) if x == y => {
                for (p, a) in xs.iter().zip(ys) {
                    self.unify_at(p, a, vars, out, d);
                }
            }
            (Ty::Ext(x, xs), Ty::Ext(y, ys)) if x == y => {
                for (p, a) in xs.iter().zip(ys) {
                    self.unify_at(p, a, vars, out, d);
                }
            }
            (Ty::Tuple(xs), Ty::Tuple(ys)) if xs.len() == ys.len() => {
                for (p, a) in xs.iter().zip(ys) {
                    self.unify_at(p, a, vars, out, d);
                }
            }
            (Ty::Slice(p) | Ty::Array(p), Ty::Slice(a) | Ty::Array(a)) => {
                self.unify_at(p, a, vars, out, d);
            }
            // A `Vec<T>` given where `[T]` is declared, by deref coercion.
            (Ty::Slice(p), Ty::Ext(v, ys)) if v == "Vec" => {
                if let Some(a) = ys.first() {
                    self.unify_at(p, a, vars, out, d);
                }
            }
            (Ty::Assoc(..), a) => {
                // A projection's parameters are not fixed by its value; nothing to learn.
                let _ = a;
            }
            _ => {}
        }
    }

    /// Whether `actual` may be of the form `pattern` (whose parameters in `vars` are unknowns):
    /// false only when a head differs. Each unknown's values are added to `out`.
    #[must_use]
    pub fn may_match(&self, pattern: &Ty, actual: &Ty, vars: &BTreeSet<GenId>, out: &mut BTreeMap<GenId, Val>) -> bool {
        if !self.heads_agree(pattern, actual, vars, 0) {
            return false;
        }
        self.unify(pattern, actual, vars, out);
        true
    }

    /// Whether `actual` may be of the form `pattern` as far as their heads go (no unknown bound).
    #[must_use]
    pub fn heads_agree(&self, pattern: &Ty, actual: &Ty, vars: &BTreeSet<GenId>, depth: usize) -> bool {
        if depth > crate::recursion::ty::MAX_DEPTH.saturating_mul(2) {
            return true;
        }
        let d = depth.saturating_add(1);
        // The analysis does not track references exactly, so their presence never decides.
        match (pattern.peel(), actual.peel()) {
            (Ty::Param(g), _) if vars.contains(g) => true,
            (_, Ty::Hidden(_) | Ty::Param(_) | Ty::Assoc(..) | Ty::Infer | Ty::Never) => true,
            (Ty::Assoc(..) | Ty::Infer | Ty::Hidden(_), _) => true,
            (Ty::Ref(p), Ty::Ref(a)) | (Ty::Slice(p), Ty::Slice(a)) | (Ty::Array(p), Ty::Array(a)) => {
                self.heads_agree(p, a, vars, d)
            }
            (Ty::Adt(x, xs), Ty::Adt(y, ys)) => {
                x == y && xs.iter().zip(ys).all(|(p, a)| self.heads_agree(p, a, vars, d))
            }
            (Ty::Ext(x, xs), Ty::Ext(y, ys)) => {
                x == y && xs.iter().zip(ys).all(|(p, a)| self.heads_agree(p, a, vars, d))
            }
            (Ty::Tuple(xs), Ty::Tuple(ys)) => {
                xs.len() == ys.len() && xs.iter().zip(ys).all(|(p, a)| self.heads_agree(p, a, vars, d))
            }
            (Ty::Prim(x), Ty::Prim(y)) => x == y,
            (Ty::FnPtr(x), Ty::FnPtr(y)) => x == y,
            (Ty::Dyn(_), Ty::Dyn(_)) => true,
            _ => false,
        }
    }

    /// The generic parameters of implementation `imp`.
    #[must_use]
    pub fn impl_generics(&self, imp: ImplId) -> &[GenId] {
        &self.program.imp(imp).scope.generics
    }

    /// Whether implementation `imp` applies to a value of type `ty`, and with which generic
    /// arguments (one value per parameter; an argument with several values is their hidden
    /// join, an unfixed one is left open). A blanket implementation applies when its parameter's
    /// kernel-trait bounds may hold.
    #[must_use]
    pub fn impl_for(&self, imp: ImplId, ty: &Ty) -> Option<Map> {
        self.impl_for_at(imp, ty, 0)
    }

    fn impl_for_at(&self, imp: ImplId, ty: &Ty, depth: usize) -> Option<Map> {
        let pattern = self.impl_self.get(imp.0)?;
        let vars: BTreeSet<GenId> = self.impl_generics(imp).iter().copied().collect();
        let mut found = BTreeMap::new();
        // References are not tracked exactly: an implementation for `&T` is taken to apply to
        // `T` too, which only adds candidates.
        let applies = self.may_match(pattern, ty, &vars, &mut found);
        if !applies {
            return None;
        }
        let map = single(&found, &vars);
        // A blanket implementation's parameter must meet its kernel-trait bounds.
        if let Ty::Param(g) = pattern.peel()
            && vars.contains(g)
            && !self.bounds_may_hold(*g, ty.peel(), depth)
        {
            return None;
        }
        Some(map)
    }

    /// Whether `ty` may meet generic parameter `g`'s kernel-trait bounds: it has an
    /// implementation of each (to depth [`BOUND_DEPTH`]), or is not known well enough to say.
    fn bounds_may_hold(&self, g: GenId, ty: &Ty, depth: usize) -> bool {
        if depth >= BOUND_DEPTH || matches!(ty, Ty::Hidden(_) | Ty::Param(_) | Ty::Assoc(..) | Ty::Infer) {
            return true;
        }
        let generic = self.program.generic(g);
        let traits = self.resolver.trait_refs(&generic.scope, generic.bounds.iter());
        traits.iter().all(|tr| match tr {
            TraitRef::Kernel(t, _) => self.by_kernel_trait.get(t).is_some_and(|impls| {
                impls
                    .iter()
                    .any(|i| self.impl_for_at(*i, ty, depth.saturating_add(1)).is_some())
            }),
            TraitRef::Ext(..) | TraitRef::Fn(..) => true,
        })
    }

    /// The implementations of kernel trait `t` and of outside traits named `ext` whose self type
    /// may be `ty`, each with its generic arguments.
    #[must_use]
    pub fn impls_of_for(&self, kernel: Option<ItemId>, ext: Option<&str>, ty: &Ty) -> Vec<(ImplId, Map)> {
        let mut out = Vec::new();
        let ids: Vec<ImplId> = kernel
            .and_then(|t| self.by_kernel_trait.get(&t))
            .into_iter()
            .chain(ext.and_then(|n| self.by_ext_trait.get(n)))
            .flatten()
            .copied()
            .collect();
        for imp in ids {
            if let Some(map) = self.impl_for(imp, ty) {
                out.push((imp, map));
            }
        }
        out
    }

    // ---------------------------------------------------------------- associated types

    /// The type with its associated types resolved where the kernel's implementations say what
    /// they are; a projection of an outside or unknown type becomes a hidden value of its base's
    /// parts (an outside type's associated types are built from its arguments); one of an open
    /// parameter stays.
    #[must_use]
    pub fn normalize(&self, ty: &Ty) -> Ty {
        self.normalize_at(ty, 0)
    }

    fn normalize_at(&self, ty: &Ty, depth: usize) -> Ty {
        if depth > crate::recursion::ty::MAX_DEPTH.saturating_mul(2) {
            return Ty::Hidden(ty.parts());
        }
        let d = depth.saturating_add(1);
        let all = |args: &[Ty]| args.iter().map(|a| self.normalize_at(a, d)).collect::<Vec<_>>();
        match ty {
            Ty::Assoc(base, name) => {
                let base = self.normalize_at(base, d);
                if let Some(t) = self.assoc_of(&base, name, d) {
                    return t;
                }
                match base.peel() {
                    Ty::Param(_) | Ty::Assoc(..) => Ty::Assoc(Box::new(base), name.clone()),
                    _ => Ty::Hidden(base.parts()),
                }
            }
            Ty::Adt(id, args) => Ty::Adt(*id, all(args)),
            Ty::Ext(n, args) => Ty::Ext(n.clone(), all(args)),
            Ty::Ref(inner) => Ty::Ref(Box::new(self.normalize_at(inner, d))),
            Ty::Slice(inner) => Ty::Slice(Box::new(self.normalize_at(inner, d))),
            Ty::Array(inner) => Ty::Array(Box::new(self.normalize_at(inner, d))),
            Ty::Tuple(items) => Ty::Tuple(all(items)),
            Ty::Hidden(parts) => {
                let mut out = BTreeSet::new();
                for p in parts {
                    match self.normalize_at(p, d) {
                        Ty::Hidden(inner) => out.extend(inner),
                        other => {
                            out.insert(other);
                        }
                    }
                }
                Ty::Hidden(out)
            }
            other => other.clone(),
        }
    }

    /// Associated type `name` of `base` from a kernel implementation that applies to it.
    fn assoc_of(&self, base: &Ty, name: &str, depth: usize) -> Option<Ty> {
        let mut found: Vec<Ty> = Vec::new();
        for (i, imp) in self.program.impls.iter().enumerate() {
            let Some(t) = imp.assoc.get(name) else {
                continue;
            };
            let id = ImplId(i);
            let Some(map) = self.impl_for(id, base) else {
                continue;
            };
            let written = self.resolver.ty(&imp.scope, t).subst(&map);
            found.push(self.normalize_at(&written, depth));
        }
        match found.len() {
            0 => None,
            1 => found.pop(),
            _ => {
                let mut parts = BTreeSet::new();
                for f in &found {
                    parts.insert(f.clone());
                }
                Some(Ty::Hidden(parts))
            }
        }
    }

    // ---------------------------------------------------------------- dispatch

    /// The types a receiver of type `ty` is tried at, in auto-dereference order: itself, then
    /// through references, `Box`, `Rc`, `Arc`, kernel `Deref` implementations, `Vec` to slice,
    /// `String` to `str` and array to slice, at most [`DEREF_DEPTH`] steps.
    #[must_use]
    pub fn deref_steps(&self, ty: &Ty) -> Vec<Ty> {
        let mut out = vec![ty.clone()];
        let mut cur = ty.clone();
        for _ in 0..DEREF_DEPTH {
            let next = match &cur {
                Ty::Ref(inner) => Some((**inner).clone()),
                Ty::Ext(name, args) if matches!(name.as_str(), "Box" | "Rc" | "Arc" | "Cow") => {
                    args.first().cloned()
                }
                Ty::Ext(name, args) if name == "Vec" => {
                    args.first().map(|a| Ty::Slice(Box::new(a.clone())))
                }
                Ty::Ext(name, _) if name == "String" => Some(Ty::prim("str")),
                Ty::Array(inner) => Some(Ty::Slice(inner.clone())),
                Ty::Adt(..) => self
                    .impls_of_for(None, Some("Deref"), &cur)
                    .first()
                    .and_then(|(imp, map)| {
                        let it = self.program.imp(*imp);
                        it.assoc
                            .get("Target")
                            .map(|t| self.normalize(&self.resolver.ty(&it.scope, t).subst(map)))
                    }),
                _ => None,
            };
            match next {
                Some(n) if !out.contains(&n) => {
                    out.push(n.clone());
                    cur = n;
                }
                _ => break,
            }
        }
        out
    }

    /// The kernel functions `receiver.name(...)` may run on a receiver of type `ty`, with
    /// auto-dereference, and the steps at which the method may be one from outside the kernel.
    #[must_use]
    pub fn method(&self, ty: &Ty, name: &str) -> Found {
        let mut found = Found::default();
        for step in self.deref_steps(ty) {
            let here = self.on(&step, name, how::RECEIVER);
            let kernel_type = matches!(step, Ty::Adt(..));
            let hit = !here.targets.is_empty();
            found.targets.extend(here.targets);
            found.outside.extend(here.outside);
            if hit && kernel_type {
                break;
            }
        }
        found
    }

    /// The kernel functions `Type::name(...)` may run, `Type` being `ty`: no auto-dereference.
    #[must_use]
    pub fn assoc(&self, ty: &Ty, name: &str) -> Found {
        self.on(ty, name, how::PATH)
    }

    /// The candidates for method `name` on a value of type `ty` exactly.
    fn on(&self, ty: &Ty, name: &str, how_found: &'static str) -> Found {
        let mut found = Found::default();
        match ty {
            Ty::Param(g) => {
                let traits = self.resolver.bounds(*g);
                found.targets.extend(self.bound_impls(&traits, name, ty));
                found.outside.insert(ty.clone());
            }
            Ty::Dyn(traits) | Ty::Opaque(traits) => {
                let all = self.resolver.with_supertraits(traits.clone());
                found.targets.extend(self.bound_impls(&all, name, ty));
                if all.iter().any(|t| t.ext_name().is_some()) {
                    found.outside.insert(ty.clone());
                }
            }
            Ty::Assoc(..) => {
                found.targets.extend(self.by_name(name));
                found.outside.insert(ty.clone());
            }
            Ty::Hidden(parts) => {
                for p in parts {
                    let sub = self.method(p, name);
                    found.targets.extend(sub.targets);
                    found.outside.extend(sub.outside);
                }
                found.targets.extend(self.outside_type_impls(name));
                found.outside.insert(ty.clone());
            }
            Ty::Never | Ty::Infer => {}
            _ => {
                found.targets.extend(self.concrete(ty, name, how_found));
                let kernel_type = matches!(ty, Ty::Adt(..));
                if !kernel_type || found.targets.is_empty() {
                    found.outside.insert(ty.clone());
                }
            }
        }
        found
    }

    /// The candidates for `name` on a known type: inherent methods, trait implementations and
    /// their unoverridden default bodies, derived implementations.
    fn concrete(&self, ty: &Ty, name: &str, how_found: &'static str) -> Vec<Target> {
        let mut out = Vec::new();
        for (i, imp) in self.program.impls.iter().enumerate() {
            let id = ImplId(i);
            let provides = imp.methods.contains_key(name)
                || self.default_of_impl(id, name).is_some();
            if !provides {
                continue;
            }
            let Some(map) = self.impl_for(id, ty) else {
                continue;
            };
            out.extend(self.impl_targets(id, name, &map, ty, how_found));
        }
        if let Ty::Adt(item, args) = ty {
            out.extend(self.derived_targets(*item, args, name));
        }
        out
    }

    /// The functions implementation `imp` runs for method `name`: its own, or its trait's default
    /// body (with `Self` as `self_ty`).
    fn impl_targets(&self, imp: ImplId, name: &str, map: &Map, self_ty: &Ty, how_found: &'static str) -> Vec<Target> {
        let it = self.program.imp(imp);
        if let Some(nodes) = it.methods.get(name) {
            return nodes
                .iter()
                .map(|n| Target {
                    node: *n,
                    map: map.clone(),
                    how: how_found,
                })
                .collect();
        }
        let Some((node, self_gen, trait_args)) = self.default_of_impl(imp, name) else {
            return Vec::new();
        };
        let mut m = Map::new();
        m.insert(self_gen, self_ty.clone());
        // The trait's own parameters, as the implementation's trait reference fixes them.
        if let Some(Some(TraitRef::Kernel(t, _))) = self.impl_trait.get(imp.0) {
            let trait_generics = &self.program.item(*t).scope.generics;
            for (g, a) in trait_generics.iter().zip(trait_args) {
                m.insert(*g, a.subst(map));
            }
        }
        vec![Target {
            node,
            map: m,
            how: how_found,
        }]
    }

    /// The default body implementation `imp` takes for method `name`, if it does not define it:
    /// the node, its trait's `Self` parameter and the implementation's trait arguments.
    fn default_of_impl(&self, imp: ImplId, name: &str) -> Option<(NodeId, GenId, Vec<Ty>)> {
        let Some(Some(TraitRef::Kernel(t, args))) = self.impl_trait.get(imp.0) else {
            return None;
        };
        if self.program.imp(imp).methods.contains_key(name) {
            return None;
        }
        let (node, self_gen) = self.trait_default(*t, name)?;
        Some((node, self_gen, args.clone()))
    }

    /// The default body of kernel trait `t`'s method `name`, or of a supertrait's, with that
    /// trait's `Self` parameter.
    #[must_use]
    pub fn trait_default(&self, t: ItemId, name: &str) -> Option<(NodeId, GenId)> {
        let traits = self
            .resolver
            .with_supertraits(vec![TraitRef::Kernel(t, Vec::new())]);
        traits.iter().find_map(|tr| match tr {
            TraitRef::Kernel(id, _) => match &self.program.item(*id).kind {
                ItemKind::Trait {
                    defaults, self_gen, ..
                } => defaults.get(name).map(|n| (*n, *self_gen)),
                _ => None,
            },
            TraitRef::Ext(..) | TraitRef::Fn(..) => None,
        })
    }

    /// The derived implementations of `item` that provide method `name`.
    fn derived_targets(&self, item: ItemId, args: &[Ty], name: &str) -> Vec<Target> {
        let map: Map = self
            .program
            .item(item)
            .scope
            .generics
            .iter()
            .copied()
            .zip(args.iter().cloned())
            .collect();
        self.derived
            .get(&item)
            .into_iter()
            .flatten()
            .filter(|(t, _)| {
                DERIVABLE
                    .iter()
                    .any(|(d, methods)| d == t && methods.contains(&name))
            })
            .map(|(_, n)| Target {
                node: *n,
                map: map.clone(),
                how: how::DERIVED,
            })
            .collect()
    }

    /// Every implementation of `traits` (kernel ones, and outside ones by name, derived
    /// implementations included) that provides method `name`, for a call on a value of type `on`
    /// whose instantiation is not known: class hierarchy analysis.
    #[must_use]
    pub fn bound_impls(&self, traits: &[TraitRef], name: &str, on: &Ty) -> Vec<Target> {
        let mut out = Vec::new();
        for tr in traits {
            let ids: Vec<ImplId> = match tr {
                TraitRef::Kernel(t, _) => self.by_kernel_trait.get(t).cloned().unwrap_or_default(),
                TraitRef::Ext(n, _) | TraitRef::Fn(n, ..) => {
                    self.by_ext_trait.get(n).cloned().unwrap_or_default()
                }
            };
            for imp in ids {
                let open: Map = self
                    .impl_generics(imp)
                    .iter()
                    .map(|g| (*g, Ty::Param(*g)))
                    .collect();
                let self_ty = self.impl_self.get(imp.0).cloned().unwrap_or_else(Ty::unknown);
                for mut t in self.impl_targets(imp, name, &open, &self_ty, how::BOUND) {
                    t.map.retain(|_, v| !matches!(v, Ty::Param(_)));
                    out.push(t);
                }
            }
            if let TraitRef::Kernel(t, _) = tr
                && let Some((node, self_gen)) = self.trait_default(*t, name)
            {
                out.push(Target {
                    node,
                    map: BTreeMap::from([(self_gen, on.clone())]),
                    how: how::BOUND,
                });
            }
            if let TraitRef::Ext(n, _) = tr
                && DERIVABLE.iter().any(|(d, m)| d == n && m.contains(&name))
            {
                for list in self.derived.values() {
                    for (t, node) in list {
                        if t == n {
                            out.push(Target {
                                node: *node,
                                map: Map::new(),
                                how: how::BOUND,
                            });
                        }
                    }
                }
            }
        }
        out
    }

    /// Every method named `name` of every implementation and trait default body: what a call on a
    /// projection of an open parameter may run.
    fn by_name(&self, name: &str) -> Vec<Target> {
        let mut out = Vec::new();
        for imp in &self.program.impls {
            for n in imp.methods.get(name).into_iter().flatten() {
                out.push(Target {
                    node: *n,
                    map: Map::new(),
                    how: how::BOUND,
                });
            }
        }
        for item in &self.program.items {
            if let ItemKind::Trait { defaults, .. } = &item.kind
                && let Some(n) = defaults.get(name)
            {
                out.push(Target {
                    node: *n,
                    map: Map::new(),
                    how: how::BOUND,
                });
            }
        }
        out
    }

    /// The kernel implementations for outside types (primitives, outside types, slices, arrays,
    /// tuples, references) and blanket ones that provide method `name`: what a value of unknown
    /// type may run besides its listed parts' methods.
    fn outside_type_impls(&self, name: &str) -> Vec<Target> {
        let mut out = Vec::new();
        for (i, imp) in self.program.impls.iter().enumerate() {
            let id = ImplId(i);
            let Some(self_ty) = self.impl_self.get(i) else {
                continue;
            };
            if matches!(self_ty.peel(), Ty::Adt(..)) {
                continue;
            }
            if !imp.methods.contains_key(name) && self.default_of_impl(id, name).is_none() {
                continue;
            }
            let open: Map = self
                .impl_generics(id)
                .iter()
                .map(|g| (*g, Ty::Param(*g)))
                .collect();
            for mut t in self.impl_targets(id, name, &open, self_ty, how::OUTSIDE_TYPE) {
                t.map.retain(|_, v| !matches!(v, Ty::Param(_)));
                out.push(t);
            }
        }
        out
    }

    /// Every implementation of an outside trait that outside code handed values built from
    /// `parts` may call back: each trait implementation (written or derived) of an outside trait
    /// whose self type may be one of the parts, or whose trait arguments name a kernel type among
    /// them; with `traits`, only those traits.
    #[must_use]
    pub fn outside_impls(&self, parts: &BTreeSet<Ty>, traits: Option<&[&str]>) -> Vec<Target> {
        let mut out = Vec::new();
        let kernel_items: BTreeSet<ItemId> = parts
            .iter()
            .filter_map(|p| match p {
                Ty::Adt(id, _) => Some(*id),
                _ => None,
            })
            .collect();
        let wanted = |name: &str| traits.is_none_or(|ts| ts.contains(&name));
        for (tname, ids) in &self.by_ext_trait {
            if !wanted(tname) {
                continue;
            }
            for imp in ids {
                let it = self.program.imp(*imp);
                let mut maps: Vec<Map> = Vec::new();
                for p in parts {
                    if matches!(p, Ty::Adt(..) | Ty::Ext(..) | Ty::Prim(_) | Ty::Param(_))
                        && let Some(map) = self.impl_for(*imp, p)
                    {
                        maps.push(map);
                    }
                }
                let names_part = self.impl_trait.get(imp.0).and_then(Option::as_ref).is_some_and(|tr| {
                    let mut hit = false;
                    for a in tr.args() {
                        a.walk(&mut |t| {
                            if let Ty::Adt(id, _) = t
                                && kernel_items.contains(id)
                            {
                                hit = true;
                            }
                        });
                    }
                    hit
                });
                if maps.is_empty() && names_part {
                    maps.push(Map::new());
                }
                for map in maps {
                    for nodes in it.methods.values() {
                        for n in nodes {
                            out.push(Target {
                                node: *n,
                                map: map.clone(),
                                how: how::CALLBACK_IMPL,
                            });
                        }
                    }
                }
            }
        }
        for p in parts {
            if let Ty::Adt(item, args) = p {
                let map: Map = self
                    .program
                    .item(*item)
                    .scope
                    .generics
                    .iter()
                    .copied()
                    .zip(args.iter().cloned())
                    .collect();
                for (t, node) in self.derived.get(item).into_iter().flatten() {
                    if wanted(t) {
                        out.push(Target {
                            node: *node,
                            map: map.clone(),
                            how: how::CALLBACK_IMPL,
                        });
                    }
                }
            }
        }
        out
    }

    /// Whether kernel trait `t` (or a supertrait) declares method `name` with a receiver.
    #[must_use]
    pub fn trait_method_has_receiver(&self, t: ItemId, name: &str) -> Option<bool> {
        self.trait_method_sig(t, name).map(|(s, _)| s.receiver)
    }
}

/// One value per unknown: the single type it was bound to, the hidden join of several, or the
/// unknown left open.
#[must_use]
pub fn single(found: &BTreeMap<GenId, Val>, vars: &BTreeSet<GenId>) -> Map {
    let mut map = Map::new();
    for g in vars {
        let t = match found.get(g) {
            Some(v) if v.0.len() == 1 => v.0.iter().next().cloned().unwrap_or(Ty::Param(*g)),
            Some(v) if !v.is_none() => {
                let mut parts = BTreeSet::new();
                for t in &v.0 {
                    match t {
                        Ty::Hidden(inner) => parts.extend(inner.iter().cloned()),
                        other => {
                            parts.insert(other.clone());
                        }
                    }
                }
                Ty::Hidden(parts)
            }
            _ => Ty::Param(*g),
        };
        map.insert(*g, t);
    }
    map
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
    use crate::recursion::model::{CrateSource, Role, load};

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

    fn item(p: &Program, name: &str) -> ItemId {
        ItemId(p.items.iter().position(|i| i.name == name).unwrap())
    }

    fn names(ix: &Index<'_>, found: &Found) -> Vec<String> {
        let mut v: Vec<String> = found.targets.iter().map(|t| ix.name(t.node).to_owned()).collect();
        v.sort();
        v.dedup();
        v
    }

    #[test]
    fn methods_follow_the_receivers_type_and_auto_dereference() {
        let p = program(
            "pub struct A; pub struct B { a: A }\n\
             impl A { pub fn m(&self) {} }\n\
             impl B { pub fn m(&self) { self.a.m() } pub fn only_b(&self) {} }\n\
             impl core::ops::Deref for B { type Target = A; fn deref(&self) -> &A { &self.a } }\n\
             pub trait T { fn t(&self) {} fn r(&self); }\n\
             impl T for A { fn r(&self) {} }\n\
             #[derive(Clone, Debug)] pub struct C;",
        );
        let ix = Index::new(&p);
        let a = Ty::Adt(item(&p, "A"), Vec::new());
        let b = Ty::Adt(item(&p, "B"), Vec::new());
        assert_eq!(names(&ix, &ix.method(&a, "m")), ["a::A::m"]);
        assert_eq!(names(&ix, &ix.method(&b, "m")), ["a::B::m"], "inherent first, no deref");
        assert_eq!(names(&ix, &ix.method(&Ty::Ref(Box::new(b.clone())), "r")), ["<a::A as T>::r"]);
        assert_eq!(names(&ix, &ix.method(&a, "t")), ["a::T::t"], "the default body");
        let c = Ty::Adt(item(&p, "C"), Vec::new());
        assert_eq!(names(&ix, &ix.method(&c, "clone")), ["<a::C as Clone>::clone"]);
        assert!(ix.method(&c, "to_owned").outside.contains(&c), "a std method on a kernel type");
        assert!(ix.method(&a, "m").outside.is_empty());
    }

    #[test]
    fn generic_implementations_match_and_fix_their_parameters() {
        let p = program(
            "pub struct W<T>(T); pub struct X;\n\
             impl<T: Clone> W<T> { pub fn get(&self) -> T { self.0.clone() } }\n\
             impl W<f64> { pub fn only_f(&self) {} }",
        );
        let ix = Index::new(&p);
        let w = item(&p, "W");
        let x = Ty::Adt(item(&p, "X"), Vec::new());
        let wx = Ty::Adt(w, vec![x.clone()]);
        let found = ix.method(&wx, "get");
        let t = found.targets.iter().next().unwrap();
        assert_eq!(t.map.values().next(), Some(&x));
        assert!(ix.method(&wx, "only_f").targets.is_empty(), "W<f64> only");
        let wf = Ty::Adt(w, vec![Ty::prim("f64")]);
        assert_eq!(ix.method(&wf, "only_f").targets.len(), 1);
    }

    #[test]
    fn bounds_and_hidden_values_dispatch_to_every_implementation() {
        let p = program(
            "pub trait S { fn s(&self); }\n\
             pub struct A; pub struct B;\n\
             impl S for A { fn s(&self) {} }\n\
             impl S for B { fn s(&self) {} }\n\
             impl S for f64 { fn s(&self) {} }\n\
             pub fn g<T: S>(t: T) { t.s() }",
        );
        let ix = Index::new(&p);
        let g = p.nodes.iter().position(|n| n.ident == "g").unwrap();
        let tg = ix.node_generics(NodeId(g))[0];
        assert_eq!(
            names(&ix, &ix.method(&Ty::Param(tg), "s")),
            ["<a::A as S>::s", "<a::B as S>::s", "<f64 as S>::s"]
        );
        let a = Ty::Adt(item(&p, "A"), Vec::new());
        let hidden = Ty::Hidden(BTreeSet::from([a]));
        assert_eq!(
            names(&ix, &ix.method(&hidden, "s")),
            ["<a::A as S>::s", "<f64 as S>::s"],
            "a hidden value's parts, and kernel implementations for outside types"
        );
    }

    #[test]
    fn associated_types_resolve_through_implementations() {
        let p = program(
            "pub struct It; pub struct Item;\n\
             impl Iterator for It { type Item = Item; fn next(&mut self) -> Option<Item> { None } }",
        );
        let ix = Index::new(&p);
        let it = Ty::Adt(item(&p, "It"), Vec::new());
        let n = ix.normalize(&Ty::Assoc(Box::new(it), "Item".to_owned()));
        assert_eq!(n, Ty::Adt(item(&p, "Item"), Vec::new()));
        let v = ix.normalize(&Ty::Assoc(Box::new(Ty::ext("Vec", vec![Ty::prim("u8")])), "Item".to_owned()));
        assert!(matches!(v, Ty::Hidden(_)));
    }
}
