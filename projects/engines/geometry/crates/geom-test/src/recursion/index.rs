//! What a call may reach: the recursion check's tables of implementations and signatures, and
//! its method lookup, trait selection and projection normalisation.
//!
//! What it does: [`Index::new`] resolves every implementation's self type and trait and files it
//! by the head of its self type ([`Head`]); every function's signature in types ([`Sig`]); every
//! node's name (`geom_num::wide::I256::mul`, `<geom_num::wide::I256 as Mul>::mul`). Given an
//! inference table ([`Infer`]), [`Index::probe`] finds what `receiver.name(...)` calls as rustc's
//! method lookup does; [`Index::select`] which implementation of a trait a type uses (operators,
//! `?`, `for`, conversions); [`Index::normalize`] resolves associated types; [`Index::assoc_fns`]
//! finds what `Type::name` names.
//!
//! Guarantees:
//!
//! - **The compiler's lookup.** Method lookup tries the receiver's auto-dereference steps in
//!   order (references, `Deref` implementations, an array unsized to a slice), at each step the
//!   receiver by value and then by reference, and at each of those inherent methods before trait
//!   methods; the first that has a candidate whose receiver unifies wins. Traits need not be in
//!   scope (code that compiles names only methods it may call, so this only adds candidates when
//!   two traits offer one name, and then both are kept).
//! - **Over-approximate where types are open.** Where unresolved variables let several
//!   implementations apply, every one of them is a candidate and none is committed to; a blanket
//!   implementation applies when its bounds may hold (a bound on a type not known yet may).
//! - **Generic receivers.** On a receiver whose type is a generic parameter, an `impl Trait` or a
//!   projection, a method of one of its bounds is a [`Pick::Bound`]: the trait's default body for
//!   it, and the blanket implementations, are what it may run from here (other implementations
//!   are checked from their own instances, [`crate::recursion::mono`]). On a trait object, a
//!   [`Pick::Dyn`]: every implementation of the trait may run.
//! - **Deterministic.** Every table is ordered and candidates come in implementation order.
//!
//! Method: Rust's method-call resolution (The Rust Reference,
//! <https://doc.rust-lang.org/reference/expressions/method-call-expr.html>; the rustc dev guide,
//! <https://rustc-dev-guide.rust-lang.org/method-lookup.html>); trait selection by unification of
//! implementation headers with their bounds checked (<https://rustc-dev-guide.rust-lang.org/traits/resolution.html>);
//! projection normalisation (<https://rustc-dev-guide.rust-lang.org/traits/goals-and-clauses.html#normalization>);
//! class hierarchy analysis for trait objects (J. Dean, D. Grove and C. Chambers, ECOOP 1995,
//! <https://doi.org/10.1007/3-540-49538-X_5>).
//!
//! Cost: building is linear in implementations and nodes; a lookup tries the implementations
//! filed under the heads of the receiver's auto-dereference steps (at most [`DEREF_DEPTH`]), each
//! by one unification; bounds of blanket implementations are checked to depth [`BOUND_DEPTH`].

use std::collections::{BTreeMap, BTreeSet};

use quote::ToTokens;

use crate::recursion::infer::Infer;
use crate::recursion::model::{DERIVABLE, ImplId, ItemKind, NodeKind, Program, Scope, SelfRef};
use crate::recursion::resolve::{Ns, Res, Resolver};
use crate::recursion::ty::{ClosureKey, GenId, ItemId, NodeId, Prim, TraitRef, Ty};

/// How many auto-dereference steps a method lookup follows.
pub const DEREF_DEPTH: usize = 8;

/// How deep the bounds of blanket implementations are checked.
pub const BOUND_DEPTH: usize = 4;

/// Generic arguments, by parameter.
pub type Map = BTreeMap<GenId, Ty>;

/// What a closure's signature is, when the checker knows it: its parameter types and result.
pub type ClosureSigs<'a> = &'a dyn Fn(&ClosureKey) -> Option<(Vec<Ty>, Ty)>;

/// A function's signature in types: its parameters (the receiver first, for a method that has
/// one) and its result, with its generic parameters open.
#[derive(Clone, Debug, Default)]
pub struct Sig {
    /// The parameter types, the receiver first when there is one.
    pub inputs: Vec<Ty>,
    /// Whether the first input is a receiver (`self`, `&self`, `self: Box<Self>`).
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
    /// A trait's default body or blanket implementation, on a generic receiver.
    pub const BOUND: &str = "a call through a generic parameter's bound (the trait's default body or a blanket implementation)";
    /// Every implementation of a trait, on a trait object.
    pub const DYN: &str = "a call on a trait object (every implementation of the trait)";
    /// An implicit trait call: an operator, `?`, `for`, indexing, a dereference, formatting.
    pub const IMPLICIT: &str = "an implicit trait call (operator, ?, for, index, deref, formatting)";
    /// A derived implementation's call of its fields' implementations.
    pub const DERIVED: &str = "a derived implementation's call on a field";
    /// A call through a value holding a closure or function item.
    pub const VALUE: &str = "a call through a value (closure or function item)";
    /// A call through a trait object or function pointer: every callable coerced to one.
    pub const ERASED: &str = "a call through a closure trait object or function pointer (every callable coerced to one)";
    /// Drop glue.
    pub const DROP: &str = "drop glue (a Drop implementation)";
}

/// The head of a type, by which implementations are filed.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Head {
    /// A primitive.
    Prim(Prim),
    /// A struct, enum or union.
    Adt(ItemId),
    /// A reference or raw pointer.
    Ref,
    /// A slice.
    Slice,
    /// An array.
    Array,
    /// A tuple of this many elements.
    Tuple(usize),
    /// A function pointer.
    FnPtr,
    /// A trait object.
    Dyn,
    /// The never type.
    Never,
}

/// The head of `t`, when it has one a lookup can use.
#[must_use]
pub fn head(t: &Ty) -> Option<Head> {
    Some(match t {
        Ty::Prim(p) => Head::Prim(*p),
        Ty::Adt(id, _) => Head::Adt(*id),
        Ty::Ref(_) => Head::Ref,
        Ty::Slice(_) => Head::Slice,
        Ty::Array(_) => Head::Array,
        Ty::Tuple(items) => Head::Tuple(items.len()),
        Ty::FnPtr(..) => Head::FnPtr,
        Ty::Dyn(_) => Head::Dyn,
        Ty::Never => Head::Never,
        _ => return None,
    })
}

/// One implementation, resolved.
#[derive(Clone, Debug)]
pub struct ImplInfo {
    /// Its self type, its generic parameters open.
    pub self_ty: Ty,
    /// Its trait, for a trait implementation.
    pub trait_ref: Option<TraitRef>,
}

/// What a method lookup chose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    /// A function of the program with its generic arguments (the caller's variables where not
    /// fixed yet).
    Node {
        /// The function.
        node: NodeId,
        /// Its generic arguments ([`Index::node_generics`]).
        args: Vec<Ty>,
    },
    /// A trait method on a receiver of generic type.
    Bound {
        /// The trait, with its arguments.
        trait_ref: TraitRef,
        /// The receiver's type at the step the method was found (the `Self` type).
        self_ty: Ty,
    },
    /// A trait method on a trait object.
    Dyn {
        /// The trait.
        trait_ref: TraitRef,
        /// The trait object type.
        self_ty: Ty,
    },
}

/// The standard library items the checker needs by name, found in the model.
#[derive(Clone, Debug, Default)]
pub struct Lang {
    /// `core::option::Option`.
    pub option: Option<ItemId>,
    /// `core::result::Result`.
    pub result: Option<ItemId>,
    /// `alloc::vec::Vec`.
    pub vec: Option<ItemId>,
    /// `alloc::string::String`.
    pub string: Option<ItemId>,
    /// `alloc::boxed::Box`.
    pub boxed: Option<ItemId>,
    /// `core::ops::Range` and its kin, by name.
    pub ranges: BTreeMap<String, ItemId>,
    /// Traits by name: `Fn`, `FnMut`, `FnOnce`, `Deref`, `Index`, `Iterator`, `IntoIterator`,
    /// `Add`, ..., `PartialEq`, `PartialOrd`, `From`, `Drop`, `Debug`, `Display`, ...
    pub traits: BTreeMap<String, ItemId>,
}

impl Lang {
    /// The model trait named `name`.
    #[must_use]
    pub fn trait_named(&self, name: &str) -> Option<ItemId> {
        self.traits.get(name).copied()
    }
}

/// The traits [`Lang`] looks up, with their paths in the model.
pub const LANG_TRAITS: [(&str, &str); 45] = [
    ("Fn", "ops::Fn"),
    ("FnMut", "ops::FnMut"),
    ("FnOnce", "ops::FnOnce"),
    ("Deref", "ops::Deref"),
    ("DerefMut", "ops::DerefMut"),
    ("Index", "ops::Index"),
    ("IndexMut", "ops::IndexMut"),
    ("Drop", "ops::Drop"),
    ("Add", "ops::Add"),
    ("Sub", "ops::Sub"),
    ("Mul", "ops::Mul"),
    ("Div", "ops::Div"),
    ("Rem", "ops::Rem"),
    ("BitAnd", "ops::BitAnd"),
    ("BitOr", "ops::BitOr"),
    ("BitXor", "ops::BitXor"),
    ("Shl", "ops::Shl"),
    ("Shr", "ops::Shr"),
    ("Neg", "ops::Neg"),
    ("Not", "ops::Not"),
    ("AddAssign", "ops::AddAssign"),
    ("SubAssign", "ops::SubAssign"),
    ("MulAssign", "ops::MulAssign"),
    ("DivAssign", "ops::DivAssign"),
    ("RemAssign", "ops::RemAssign"),
    ("BitAndAssign", "ops::BitAndAssign"),
    ("BitOrAssign", "ops::BitOrAssign"),
    ("BitXorAssign", "ops::BitXorAssign"),
    ("ShlAssign", "ops::ShlAssign"),
    ("ShrAssign", "ops::ShrAssign"),
    ("Try", "ops::Try"),
    ("PartialEq", "cmp::PartialEq"),
    ("PartialOrd", "cmp::PartialOrd"),
    ("Ord", "cmp::Ord"),
    ("Clone", "clone::Clone"),
    ("Default", "default::Default"),
    ("From", "convert::From"),
    ("Iterator", "iter::Iterator"),
    ("IntoIterator", "iter::IntoIterator"),
    ("FromIterator", "iter::FromIterator"),
    ("Debug", "fmt::Debug"),
    ("Display", "fmt::Display"),
    ("Hash", "hash::Hash"),
    ("Sized", "marker::Sized"),
    ("Copy", "marker::Copy"),
];

/// The formatting traits a placeholder may name, with their paths in the model.
pub const FMT_TRAITS: [(&str, &str); 9] = [
    ("Display", "fmt::Display"),
    ("Debug", "fmt::Debug"),
    ("LowerHex", "fmt::LowerHex"),
    ("UpperHex", "fmt::UpperHex"),
    ("Octal", "fmt::Octal"),
    ("Binary", "fmt::Binary"),
    ("LowerExp", "fmt::LowerExp"),
    ("UpperExp", "fmt::UpperExp"),
    ("Pointer", "fmt::Pointer"),
];

/// The program's implementations, filed for lookup, its signatures and its nodes' names.
pub struct Index<'p> {
    /// The resolver.
    pub resolver: Resolver<'p>,
    /// The program.
    pub program: &'p Program,
    /// Every implementation, resolved.
    pub impls: Vec<ImplInfo>,
    /// Implementations by the head of their self type.
    by_head: BTreeMap<Head, Vec<ImplId>>,
    /// Implementations whose self type is a generic parameter or has no head: they may apply to
    /// any type.
    blanket: Vec<ImplId>,
    /// Implementations of each trait.
    pub by_trait: BTreeMap<ItemId, Vec<ImplId>>,
    /// The derived implementations of each item: trait name and node.
    pub derived: BTreeMap<ItemId, Vec<(String, NodeId)>>,
    /// Every node's generic parameters, as instances list their arguments.
    pub generics: Vec<Vec<GenId>>,
    /// Every node's signature (derived implementations get theirs from their trait).
    pub sigs: Vec<Sig>,
    /// Every node's name.
    pub names: Vec<String>,
    /// The standard library items the checker needs.
    pub lang: Lang,
    /// The formatting traits by placeholder trait name.
    pub fmt_traits: BTreeMap<String, ItemId>,
}

impl<'p> Index<'p> {
    /// Files `program`'s implementations, reads its signatures and names its nodes.
    #[must_use]
    pub fn new(program: &'p Program) -> Index<'p> {
        let resolver = Resolver::new(program);
        let mut impls = Vec::new();
        let mut by_head: BTreeMap<Head, Vec<ImplId>> = BTreeMap::new();
        let mut blanket = Vec::new();
        let mut by_trait: BTreeMap<ItemId, Vec<ImplId>> = BTreeMap::new();
        for (i, imp) in program.impls.iter().enumerate() {
            let id = ImplId(i);
            let mut scope = imp.scope.clone();
            scope.self_ref = SelfRef::None;
            let self_ty = resolver.ty(&scope, &imp.self_ty);
            let trait_ref = imp.trait_path.as_ref().and_then(|p| resolver.trait_ref(&imp.scope, p));
            match head(&self_ty) {
                Some(h) => by_head.entry(h).or_default().push(id),
                None => blanket.push(id),
            }
            if let Some(tr) = &trait_ref {
                by_trait.entry(tr.id).or_default().push(id);
            }
            impls.push(ImplInfo { self_ty, trait_ref });
        }
        let mut derived: BTreeMap<ItemId, Vec<(String, NodeId)>> = BTreeMap::new();
        for (n, node) in program.nodes.iter().enumerate() {
            if let NodeKind::Derived { item, trait_name } = &node.kind {
                derived.entry(*item).or_default().push((trait_name.clone(), NodeId(n)));
            }
        }
        let mut index = Index {
            resolver,
            program,
            impls,
            by_head,
            blanket,
            by_trait,
            derived,
            generics: Vec::new(),
            sigs: Vec::new(),
            names: Vec::new(),
            lang: Lang::default(),
            fmt_traits: BTreeMap::new(),
        };
        index.lang = index.find_lang();
        index.fmt_traits = FMT_TRAITS
            .iter()
            .filter_map(|(n, p)| index.model_item(p).map(|id| ((*n).to_owned(), id)))
            .collect();
        index.complete_impl_traits();
        index.generics = (0..program.nodes.len()).map(|n| index.generics_of(NodeId(n))).collect();
        index.sigs = (0..program.nodes.len()).map(|n| index.sig_of(NodeId(n))).collect();
        index.names = (0..program.nodes.len()).map(|n| index.name_of(NodeId(n))).collect();
        index
    }

    /// The item at `path` (`option::Option`) in the standard library's model.
    #[must_use]
    pub fn model_item(&self, path: &str) -> Option<ItemId> {
        let root = self
            .program
            .crates
            .iter()
            .find(|c| c.source.role == crate::recursion::model::Role::Model && c.source.lib == "core")?
            .root;
        let scope = Scope {
            module: root,
            generics: Vec::new(),
            consts: Vec::new(),
            self_ref: SelfRef::None,
        };
        let segs: Vec<String> = path.split("::").map(str::to_owned).collect();
        self.resolver
            .resolve(&scope, &segs, false, Ns::Type)
            .into_iter()
            .find_map(|r| match r {
                Res::Item(id) => Some(id),
                _ => None,
            })
    }

    fn find_lang(&self) -> Lang {
        let mut lang = Lang {
            option: self.model_item("option::Option"),
            result: self.model_item("result::Result"),
            vec: self.model_item("vec::Vec"),
            string: self.model_item("string::String"),
            boxed: self.model_item("boxed::Box"),
            ..Lang::default()
        };
        for r in ["Range", "RangeInclusive", "RangeFrom", "RangeTo", "RangeToInclusive", "RangeFull"] {
            if let Some(id) = self.model_item(&format!("ops::{r}")) {
                lang.ranges.insert(r.to_owned(), id);
            }
        }
        for (name, path) in LANG_TRAITS {
            if let Some(id) = self.model_item(path) {
                lang.traits.insert(name.to_owned(), id);
            }
        }
        lang
    }

    /// Fills each implementation's trait arguments left out by their defaults (`impl Add for T`
    /// is `impl Add<T> for T`).
    fn complete_impl_traits(&mut self) {
        let done: Vec<Option<TraitRef>> = self
            .impls
            .iter()
            .map(|i| i.trait_ref.as_ref().map(|t| self.complete(t, &i.self_ty)))
            .collect();
        for (info, t) in self.impls.iter_mut().zip(done) {
            info.trait_ref = t;
        }
    }

    /// `tr` with the trait arguments it leaves out filled by their defaults, `Self` being
    /// `self_ty`.
    #[must_use]
    pub fn complete(&self, tr: &TraitRef, self_ty: &Ty) -> TraitRef {
        let item = self.program.item(tr.id);
        let ItemKind::Trait { self_gen, .. } = &item.kind else {
            return tr.clone();
        };
        let generics = &item.scope.generics;
        let mut args = tr.args.clone();
        while args.len() < generics.len() {
            let Some(g) = generics.get(args.len()) else {
                break;
            };
            let generic = self.program.generic(*g);
            let filled = match &generic.default {
                Some(d) => {
                    let mut map: Map = generics.iter().copied().zip(args.iter().cloned()).collect();
                    map.insert(*self_gen, self_ty.clone());
                    self.resolver.ty(&generic.scope, d).subst(&map)
                }
                None => Ty::Infer,
            };
            args.push(filled);
        }
        TraitRef {
            id: tr.id,
            args,
            bindings: tr.bindings.clone(),
        }
    }

    // ---------------------------------------------------------------- names

    /// A type as names show it.
    #[must_use]
    pub fn show(&self, ty: &Ty) -> String {
        let list = |items: &[Ty]| items.iter().map(|t| self.show(t)).collect::<Vec<_>>().join(", ");
        match ty {
            Ty::Adt(id, args) => {
                let item = self.program.item(*id);
                let model = self
                    .program
                    .crates
                    .get(self.program.module(item.module).krate)
                    .is_some_and(|c| c.source.role == crate::recursion::model::Role::Model);
                let base = if model { item.name.clone() } else { item.path.clone() };
                if args.is_empty() {
                    base
                } else {
                    format!("{base}<{}>", list(args))
                }
            }
            Ty::Prim(p) => p.name().to_owned(),
            Ty::Ref(inner) => format!("&{}", self.show(inner)),
            Ty::Slice(inner) => format!("[{}]", self.show(inner)),
            Ty::Array(inner) => format!("[{}; _]", self.show(inner)),
            Ty::Tuple(items) => format!("({})", list(items)),
            Ty::Param(g) => self.program.generic(*g).name.clone(),
            Ty::Dyn(traits) => format!(
                "dyn {}",
                traits.iter().map(|t| self.program.item(t.id).name.clone()).collect::<Vec<_>>().join(" + ")
            ),
            Ty::Opaque(traits) => format!(
                "impl {}",
                traits.iter().map(|t| self.program.item(t.id).name.clone()).collect::<Vec<_>>().join(" + ")
            ),
            Ty::Assoc(base, _, name) => format!("{}::{name}", self.show(base)),
            Ty::FnPtr(params, ret) => format!("fn({}) -> {}", list(params), self.show(ret)),
            Ty::Closure(k) => format!("{}::{{closure#{}}}", self.name(k.owner), k.index),
            Ty::FnDef(n, _) => self.name(*n).to_owned(),
            Ty::Never => "!".to_owned(),
            Ty::Var(_) | Ty::Infer => "_".to_owned(),
            Ty::Unknown => "{unknown}".to_owned(),
        }
    }

    /// The head of a type as names show it: a type's path, without its arguments.
    fn show_head(&self, ty: &Ty) -> String {
        match ty {
            Ty::Adt(id, _) => self.program.item(*id).path.clone(),
            other => self.show(other),
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
                    .impls
                    .get(imp.0)
                    .map(|i| self.show_head(&i.self_ty))
                    .filter(|s| s != "{unknown}")
                    .unwrap_or_else(|| self.program.imp(*imp).self_ty.to_token_stream().to_string());
                match self.impls.get(imp.0).and_then(|i| i.trait_ref.as_ref()) {
                    Some(tr) => format!("<{self_ty} as {}>::{}", self.program.item(tr.id).name, node.ident),
                    None => format!("{self_ty}::{}", node.ident),
                }
            }
            NodeKind::Derived { item, trait_name } => {
                format!("<{} as {trait_name}>::{}", self.program.item(*item).path, node.ident)
            }
        }
    }

    /// The name of node `id`.
    #[must_use]
    pub fn name(&self, id: NodeId) -> &str {
        self.names.get(id.0).map_or("", String::as_str)
    }

    /// Whether node `id` belongs to the standard library's model.
    #[must_use]
    pub fn is_model(&self, id: NodeId) -> bool {
        self.program
            .node(id)
            .and_then(|n| self.program.crates.get(n.krate))
            .is_some_and(|c| c.source.role == crate::recursion::model::Role::Model)
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
            NodeKind::Derived { item, trait_name } => {
                // The type's parameters, then the trait method's own (`Hash::hash`'s `H`).
                let mut out = self.program.item(*item).scope.generics.clone();
                if let Some(t) = self.lang.trait_named(trait_name)
                    && let Some((_, generics)) = self.trait_method(t, &node.ident)
                {
                    let skip = self.program.item(t).scope.generics.len().saturating_add(1);
                    out.extend(generics.into_iter().skip(skip));
                }
                out
            }
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
        self.node_generics(id).iter().copied().zip(args.iter().cloned()).collect()
    }

    /// The `Self` type of node `id`, its generic parameters open.
    #[must_use]
    pub fn self_ty(&self, id: NodeId) -> Option<Ty> {
        let node = self.program.node(id)?;
        match &node.kind {
            NodeKind::Method(imp) => self.impls.get(imp.0).map(|i| i.self_ty.clone()),
            NodeKind::TraitDefault(t) => match &self.program.item(*t).kind {
                ItemKind::Trait { self_gen, .. } => Some(Ty::Param(*self_gen)),
                _ => None,
            },
            NodeKind::Derived { item, .. } => Some(self.adt_open(*item)),
            NodeKind::Fn => None,
        }
    }

    /// Item `item` with its generic parameters open.
    #[must_use]
    pub fn adt_open(&self, item: ItemId) -> Ty {
        Ty::Adt(item, self.program.item(item).scope.generics.iter().map(|g| Ty::Param(*g)).collect())
    }

    fn sig_of(&self, id: NodeId) -> Sig {
        let Some(node) = self.program.node(id) else {
            return Sig::default();
        };
        if let NodeKind::Derived { trait_name, .. } = &node.kind {
            let own = self.self_ty(id).unwrap_or(Ty::Unknown);
            let found = self
                .lang
                .trait_named(trait_name)
                .and_then(|t| self.trait_method(t, &node.ident).map(|(s, g)| (t, s, g)));
            return match found {
                Some((t, sig, generics)) => {
                    let mut map = Map::new();
                    if let Some(self_gen) = generics.first() {
                        map.insert(*self_gen, own.clone());
                    }
                    // A derived trait's own parameters (`PartialEq<Rhs = Self>`) are `Self`.
                    for g in &self.program.item(t).scope.generics {
                        map.insert(*g, own.clone());
                    }
                    Sig {
                        inputs: sig.inputs.iter().map(|x| x.subst(&map)).collect(),
                        receiver: sig.receiver,
                        output: sig.output.subst(&map),
                    }
                }
                None => Sig {
                    inputs: vec![Ty::reference(own)],
                    receiver: true,
                    output: Ty::Unknown,
                },
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
                    let own = self_ty.cloned().unwrap_or(Ty::Unknown);
                    let t = match &r.kind {
                        syn::ReceiverKind::Typed(_, t) => self.resolver.ty(scope, t),
                        syn::ReceiverKind::Reference(..) => Ty::reference(own),
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
        Sig { inputs, receiver, output }
    }

    /// Node `id`'s signature.
    #[must_use]
    pub fn sig(&self, id: NodeId) -> Option<&Sig> {
        self.sigs.get(id.0)
    }

    /// The signature of trait `t`'s method `name` as the trait declares it, with its generic
    /// parameters in instance order (`Self`, the trait's, the method's); from `t` or a supertrait.
    #[must_use]
    pub fn trait_method(&self, t: ItemId, name: &str) -> Option<(Sig, Vec<GenId>)> {
        for tid in self.trait_closure(t) {
            if let ItemKind::Trait { sigs, self_gen, .. } = &self.program.item(tid).kind
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

    /// The trait among `t` and its supertraits that declares method `name`.
    #[must_use]
    pub fn trait_declaring_method(&self, t: ItemId, name: &str) -> Option<ItemId> {
        self.trait_closure(t).into_iter().find(|tid| {
            matches!(&self.program.item(*tid).kind, ItemKind::Trait { methods, .. } if methods.contains(name))
        })
    }

    /// Trait `t` and every supertrait it leads to, each once, `t` first.
    #[must_use]
    pub fn trait_closure(&self, t: ItemId) -> Vec<ItemId> {
        let mut out = Vec::new();
        let mut work = vec![t];
        let mut seen = BTreeSet::new();
        while let Some(x) = work.pop() {
            if !seen.insert(x) {
                continue;
            }
            out.push(x);
            let item = self.program.item(x);
            if let ItemKind::Trait { supertraits, .. } = &item.kind {
                let mut supers: Vec<ItemId> =
                    self.resolver.trait_refs(&item.scope, supertraits.iter()).into_iter().map(|r| r.id).collect();
                supers.reverse();
                work.extend(supers);
            }
        }
        out
    }

    /// `tr` and its supertraits with their arguments put in, `Self` being `self_ty`, each once.
    #[must_use]
    pub fn supertraits(&self, tr: &TraitRef, self_ty: &Ty) -> Vec<TraitRef> {
        let mut out: Vec<TraitRef> = Vec::new();
        let mut work = vec![self.complete(tr, self_ty)];
        while let Some(t) = work.pop() {
            if out.iter().any(|o| o.id == t.id && o.args == t.args) {
                continue;
            }
            let item = self.program.item(t.id);
            if let ItemKind::Trait {
                supertraits, self_gen, ..
            } = &item.kind
            {
                let mut map: Map = item.scope.generics.iter().copied().zip(t.args.iter().cloned()).collect();
                map.insert(*self_gen, self_ty.clone());
                for s in self.resolver.trait_refs(&item.scope, supertraits.iter()) {
                    let s = self.complete(&s, &Ty::Param(*self_gen));
                    let mut sub = s.map(&mut |x| x.subst(&map));
                    // A binding written on the subtrait (`Fn(A) -> R` fixes `FnOnce::Output`)
                    // belongs to whichever trait declares it.
                    for (n, b) in &t.bindings {
                        if self.resolver.declaring_trait(std::slice::from_ref(&sub), n).is_some()
                            && sub.binding(n).is_none()
                        {
                            sub.bindings.push((n.clone(), b.clone()));
                        }
                    }
                    work.push(sub);
                }
            }
            out.push(t);
        }
        out
    }

    /// The bounds of generic parameter `g` with their supertraits.
    #[must_use]
    pub fn bounds(&self, g: GenId) -> Vec<TraitRef> {
        let own = Ty::Param(g);
        let mut out: Vec<TraitRef> = Vec::new();
        for b in self.resolver.bounds(g) {
            for s in self.supertraits(&b, &own) {
                if !out.contains(&s) {
                    out.push(s);
                }
            }
        }
        out
    }

    /// The bounds a generic receiver of type `t` is known by: a parameter's, an `impl Trait`'s, a
    /// projection's declared bounds; with supertraits.
    #[must_use]
    pub fn bounds_of_ty(&self, t: &Ty) -> Vec<TraitRef> {
        match t {
            Ty::Param(g) => self.bounds(*g),
            Ty::Opaque(traits) | Ty::Dyn(traits) => {
                let mut out = Vec::new();
                for tr in traits {
                    for s in self.supertraits(tr, t) {
                        if !out.contains(&s) {
                            out.push(s);
                        }
                    }
                }
                out
            }
            Ty::Assoc(_, Some(tr), name) => {
                let item = self.program.item(*tr);
                let ItemKind::Trait { assoc_types, .. } = &item.kind else {
                    return Vec::new();
                };
                let Some(bounds) = assoc_types.get(name) else {
                    return Vec::new();
                };
                let mut out = Vec::new();
                for b in self.resolver.trait_refs(&item.scope, bounds.iter()) {
                    for s in self.supertraits(&b, t) {
                        if !out.contains(&s) {
                            out.push(s);
                        }
                    }
                }
                out
            }
            _ => Vec::new(),
        }
    }

    // ---------------------------------------------------------------- fields

    /// The fields of item `item` (of variant `variant` for an enum), each with its type in terms
    /// of the item's generic parameters.
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
        fields.all().into_iter().map(|(n, t)| (n, self.resolver.ty(&it.scope, t))).collect()
    }

    /// Every field type of every variant of item `item`, in terms of its parameters.
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

    /// The type of field `name` of a value of type `base`, after auto-dereference; `None` when no
    /// step has it.
    #[must_use]
    pub fn field_ty(&self, base: &Ty, name: &str, infer: &mut Infer, sigs: ClosureSigs<'_>) -> Option<Ty> {
        for step in self.autoderef(base, infer, sigs) {
            match &step {
                Ty::Adt(id, args) if matches!(self.program.item(*id).kind, ItemKind::Struct { .. } | ItemKind::Union { .. }) => {
                    let map: Map = self.program.item(*id).scope.generics.iter().copied().zip(args.iter().cloned()).collect();
                    if let Some((_, t)) = self.fields(*id, None).into_iter().find(|(n, _)| n == name) {
                        return Some(t.subst(&map));
                    }
                }
                Ty::Tuple(items) => {
                    if let Some(t) = name.parse::<usize>().ok().and_then(|i| items.get(i)) {
                        return Some(t.clone());
                    }
                }
                _ => {}
            }
        }
        None
    }

    // ---------------------------------------------------------------- dereference

    /// The type `*t` gives through `Deref`: a reference's referent, or a `Deref` implementation's
    /// target; `None` when there is none.
    #[must_use]
    pub fn deref_once(&self, t: &Ty, infer: &mut Infer, sigs: ClosureSigs<'_>) -> Option<Ty> {
        let t = infer.shallow(t);
        match &t {
            Ty::Ref(inner) => Some((**inner).clone()),
            Ty::Adt(..) | Ty::Param(_) | Ty::Opaque(_) | Ty::Assoc(..) => {
                let deref = self.lang.trait_named("Deref")?;
                let target = self.normalize(&Ty::Assoc(Box::new(t.clone()), Some(deref), "Target".to_owned()), infer, sigs);
                match infer.shallow(&target) {
                    Ty::Assoc(..) | Ty::Unknown => None,
                    other => Some(other),
                }
            }
            _ => None,
        }
    }

    /// The types a receiver of type `t` is tried at, in auto-dereference order, ending with an
    /// array's slice.
    #[must_use]
    pub fn autoderef(&self, t: &Ty, infer: &mut Infer, sigs: ClosureSigs<'_>) -> Vec<Ty> {
        let mut out = vec![infer.shallow(t)];
        let mut cur = infer.shallow(t);
        for _ in 0..DEREF_DEPTH {
            let Some(next) = self.deref_once(&cur, infer, sigs) else {
                break;
            };
            let next = infer.shallow(&next);
            if out.contains(&next) {
                break;
            }
            out.push(next.clone());
            cur = next;
        }
        if let Some(Ty::Array(inner)) = out.last().cloned() {
            out.push(Ty::Slice(inner));
        }
        out
    }

    // ---------------------------------------------------------------- matching

    /// Fresh variables for implementation `imp`'s generic parameters.
    fn fresh_impl(&self, imp: ImplId, infer: &mut Infer) -> Map {
        self.program.imp(imp).scope.generics.iter().map(|g| (*g, infer.fresh())).collect()
    }

    /// Whether implementation `imp`, its generic parameters as `map` gives them (variables), may
    /// apply to `ty` (and, for a trait implementation, with trait arguments `trait_args` when
    /// given): its header unifies and its bounds may hold. On success the unification is kept; on
    /// failure nothing changes.
    #[allow(clippy::too_many_arguments, reason = "the match's inputs and the lookup's state")]
    pub fn try_impl(
        &self,
        imp: ImplId,
        map: &Map,
        ty: &Ty,
        trait_args: Option<&[Ty]>,
        infer: &mut Infer,
        sigs: ClosureSigs<'_>,
        depth: usize,
    ) -> bool {
        let Some(info) = self.impls.get(imp.0) else {
            return false;
        };
        let snap = infer.snapshot();
        let pattern = info.self_ty.subst(map);
        let mut ok = infer.unify(&pattern, ty);
        if ok
            && let (Some(args), Some(tr)) = (trait_args, &info.trait_ref)
        {
            for (a, b) in tr.args.iter().zip(args) {
                if !infer.unify(&a.subst(map), b) {
                    ok = false;
                    break;
                }
            }
        }
        if ok && !self.bounds_may_hold(imp, map, infer, sigs, depth) {
            ok = false;
        }
        if !ok {
            infer.rollback_to(snap);
        }
        ok
    }

    /// [`Index::try_impl`] with fresh variables for the implementation's parameters, which are
    /// returned on success.
    pub fn match_impl(
        &self,
        imp: ImplId,
        ty: &Ty,
        trait_args: Option<&[Ty]>,
        infer: &mut Infer,
        sigs: ClosureSigs<'_>,
        depth: usize,
    ) -> Option<Map> {
        let map = self.fresh_impl(imp, infer);
        self.try_impl(imp, &map, ty, trait_args, infer, sigs, depth).then_some(map)
    }

    /// The implementations among `candidates` that may apply to `ty` (with `trait_args`), each
    /// with variables for its parameters; the unification is kept when exactly one applies.
    fn applicable(&self, candidates: Vec<ImplId>, ty: &Ty, trait_args: Option<&[Ty]>, infer: &mut Infer, sigs: ClosureSigs<'_>) -> Vec<(ImplId, Map)> {
        let mut found = Vec::new();
        for imp in candidates {
            // The variables are made before the snapshot, so they outlive its rollback.
            let map = self.fresh_impl(imp, infer);
            let snap = infer.snapshot();
            if self.try_impl(imp, &map, ty, trait_args, infer, sigs, 0) {
                found.push((imp, map));
            }
            infer.rollback_to(snap);
        }
        if let [(imp, map)] = found.as_slice() {
            let _ = self.try_impl(*imp, map, ty, trait_args, infer, sigs, 0);
        }
        found
    }

    /// Whether implementation `imp`'s generic parameters, as `map` gives them, may meet their
    /// bounds.
    fn bounds_may_hold(&self, imp: ImplId, map: &Map, infer: &mut Infer, sigs: ClosureSigs<'_>, depth: usize) -> bool {
        if depth >= BOUND_DEPTH {
            return true;
        }
        for g in &self.program.imp(imp).scope.generics {
            let Some(value) = map.get(g) else {
                continue;
            };
            for b in self.resolver.bounds(*g) {
                let b = self.complete(&b, value).map(&mut |x| x.subst(map));
                if !self.may_implement(value, &b, infer, sigs, depth.saturating_add(1)) {
                    return false;
                }
            }
        }
        true
    }

    /// Whether type `t` may implement `tr`: it is not known well enough to say, or an
    /// implementation (or a built-in one, for closures and markers) may apply.
    pub fn may_implement(&self, t: &Ty, tr: &TraitRef, infer: &mut Infer, sigs: ClosureSigs<'_>, depth: usize) -> bool {
        let t = infer.shallow(t);
        if matches!(t, Ty::Var(_) | Ty::Unknown | Ty::Never | Ty::Param(_) | Ty::Assoc(..) | Ty::Opaque(_) | Ty::Infer) {
            return true;
        }
        if self.is_marker(tr.id) {
            return true;
        }
        if self.is_fn_trait(tr.id) {
            return matches!(t, Ty::Closure(_) | Ty::FnDef(..) | Ty::FnPtr(..) | Ty::Ref(_))
                || matches!(&t, Ty::Dyn(ts) if ts.iter().any(|x| self.is_fn_trait(x.id)))
                || matches!(&t, Ty::Adt(id, _) if Some(*id) == self.lang.boxed);
        }
        if let Ty::Dyn(traits) = &t
            && traits.iter().any(|x| self.trait_closure(x.id).contains(&tr.id))
        {
            return true;
        }
        if let Ty::Adt(id, _) = &t
            && self.program.item(*id).derives().iter().any(|d| *d == self.program.item(tr.id).name)
        {
            return true;
        }
        let snap = infer.snapshot();
        let found = self
            .candidates_for(&t, Some(tr.id))
            .into_iter()
            .any(|imp| self.match_impl(imp, &t, Some(&tr.args), infer, sigs, depth).is_some());
        infer.rollback_to(snap);
        found
    }

    /// Whether trait `t` has no methods and no associated types: a marker such as `Copy`, `Send`
    /// or `Eq`, which the check takes to hold wherever it is asked.
    #[must_use]
    pub fn is_marker(&self, t: ItemId) -> bool {
        matches!(&self.program.item(t).kind, ItemKind::Trait { methods, assoc_types, .. }
            if methods.is_empty() && assoc_types.is_empty())
    }

    /// Whether trait `t` is `Fn`, `FnMut` or `FnOnce`.
    #[must_use]
    pub fn is_fn_trait(&self, t: ItemId) -> bool {
        ["Fn", "FnMut", "FnOnce"].iter().any(|n| self.lang.trait_named(n) == Some(t))
    }

    /// The implementations (of trait `tr`, when given) that may apply to a type with `t`'s head:
    /// those filed under its head, and the blanket ones.
    fn candidates_for(&self, t: &Ty, tr: Option<ItemId>) -> Vec<ImplId> {
        let mut out: Vec<ImplId> = Vec::new();
        if let Some(h) = head(t)
            && let Some(list) = self.by_head.get(&h)
        {
            out.extend(list.iter().copied());
        }
        out.extend(self.blanket.iter().copied());
        if let Some(tr) = tr {
            out.retain(|i| self.impls.get(i.0).and_then(|x| x.trait_ref.as_ref()).is_some_and(|x| x.id == tr));
        }
        out.sort();
        out.dedup();
        out
    }

    // ---------------------------------------------------------------- selection

    /// The implementations of trait `tr` that type `t` may use, each with its arguments, kept in
    /// `infer` when exactly one applies; a generic `t` gives none (see [`Index::probe`]).
    pub fn select(&self, t: &Ty, tr: &TraitRef, infer: &mut Infer, sigs: ClosureSigs<'_>) -> Vec<(ImplId, Map)> {
        let t = infer.shallow(t);
        if matches!(t, Ty::Var(_) | Ty::Unknown | Ty::Param(_) | Ty::Assoc(..) | Ty::Opaque(_) | Ty::Dyn(_)) {
            return Vec::new();
        }
        let tr = self.complete(tr, &t);
        let candidates = self.candidates_for(&t, Some(tr.id));
        self.applicable(candidates, &t, Some(&tr.args), infer, sigs)
    }

    /// The function implementation `imp` runs for trait method `name`: its own, or the trait's
    /// default body; with generic arguments from `map` (the implementation's), `Self` being
    /// `self_ty`, and `method_args` for the method's own parameters.
    #[must_use]
    pub fn impl_fn(&self, imp: ImplId, name: &str, map: &Map, self_ty: &Ty, method_args: &[Ty]) -> Option<(NodeId, Vec<Ty>)> {
        let it = self.program.imp(imp);
        if let Some(node) = it.methods.get(name).and_then(|v| v.first()) {
            let generics = self.node_generics(*node);
            let own = self.program.imp(imp).scope.generics.len();
            let mut args: Vec<Ty> = generics.iter().take(own).map(|g| map.get(g).cloned().unwrap_or(Ty::Param(*g))).collect();
            args.extend(method_args.iter().cloned());
            while args.len() < generics.len() {
                args.push(Ty::Infer);
            }
            return Some((*node, args));
        }
        let tr = self.impls.get(imp.0)?.trait_ref.as_ref()?;
        let tr = tr.map(&mut |x| x.subst(map));
        self.default_fn(&tr, name, self_ty, method_args)
    }

    /// Trait `tr`'s default body for method `name` (from `tr` or a supertrait) with `Self` as
    /// `self_ty`: the node and its instance arguments.
    #[must_use]
    pub fn default_fn(&self, tr: &TraitRef, name: &str, self_ty: &Ty, method_args: &[Ty]) -> Option<(NodeId, Vec<Ty>)> {
        for sup in self.supertraits(tr, self_ty) {
            let item = self.program.item(sup.id);
            let ItemKind::Trait { defaults, .. } = &item.kind else {
                continue;
            };
            let Some(node) = defaults.get(name) else {
                continue;
            };
            let generics = self.node_generics(*node);
            let mut args = vec![self_ty.clone()];
            args.extend(sup.args.iter().cloned());
            args.truncate(item.scope.generics.len().saturating_add(1));
            while args.len() < item.scope.generics.len().saturating_add(1) {
                args.push(Ty::Infer);
            }
            args.extend(method_args.iter().cloned());
            while args.len() < generics.len() {
                args.push(Ty::Infer);
            }
            args.truncate(generics.len());
            return Some((*node, args));
        }
        None
    }

    /// The derived implementation of trait `trait_name` for item `item`, if it derives one that
    /// provides method `name`.
    #[must_use]
    pub fn derived_fn(&self, item: ItemId, trait_name: &str, name: &str) -> Option<NodeId> {
        self.derived.get(&item)?.iter().find_map(|(t, node)| {
            (t == trait_name && DERIVABLE.iter().any(|(d, m)| d == t && m.contains(&name))).then_some(*node)
        })
    }

    // ---------------------------------------------------------------- method lookup

    /// What `receiver.name(...)` calls, the receiver being of type `recv`: the candidates of the
    /// first auto-dereference step (by value, then by reference; inherent before trait) at which
    /// any receiver unifies, with the receiver's type there. Variables are bound only when one
    /// candidate is found.
    pub fn probe(&self, recv: &Ty, name: &str, infer: &mut Infer, sigs: ClosureSigs<'_>) -> (Vec<Pick>, Option<Ty>) {
        let steps = self.autoderef(recv, infer, sigs);
        let mut heads: Vec<ImplId> = Vec::new();
        for s in &steps {
            heads.extend(self.candidates_for(s, None));
            if let Ty::Ref(inner) = s {
                heads.extend(self.candidates_for(inner, None));
            }
        }
        heads.sort();
        heads.dedup();
        for step in &steps {
            for autoref in [false, true] {
                let adjusted = if autoref { Ty::reference(step.clone()) } else { step.clone() };
                for inherent in [true, false] {
                    let picks = self.picks_at(step, &adjusted, name, inherent, &heads, infer, sigs);
                    if !picks.is_empty() {
                        return (picks, Some(adjusted));
                    }
                }
            }
        }
        (Vec::new(), None)
    }

    /// The candidates at one step: implementation methods (inherent ones, or trait ones and
    /// derived ones) whose receiver unifies with `adjusted`; for a generic or trait-object step,
    /// the methods of its bounds.
    #[allow(clippy::too_many_arguments, reason = "the lookup's state at one step")]
    fn picks_at(
        &self,
        step: &Ty,
        adjusted: &Ty,
        name: &str,
        inherent: bool,
        heads: &[ImplId],
        infer: &mut Infer,
        sigs: ClosureSigs<'_>,
    ) -> Vec<Pick> {
        let mut found: Vec<(Pick, Ty)> = Vec::new();
        for &imp in heads {
            let Some(info) = self.impls.get(imp.0) else {
                continue;
            };
            if info.trait_ref.is_some() == inherent {
                continue;
            }
            let provides = self.program.imp(imp).methods.contains_key(name)
                || info.trait_ref.as_ref().is_some_and(|tr| self.trait_default_exists(tr.id, name));
            if !provides {
                continue;
            }
            // The variables are made before the snapshot, so a pick's arguments outlive its
            // rollback.
            let map = self.fresh_impl(imp, infer);
            let self_ty = info.self_ty.subst(&map);
            let Some((node, args)) = self.impl_fn(imp, name, &map, &self_ty, &[]) else {
                continue;
            };
            let args = self.fill_infer(args, infer);
            let Some(recv_ty) = self.receiver_of(node, &args) else {
                continue;
            };
            let snap = infer.snapshot();
            let ok = infer.unify(&recv_ty, adjusted) && self.bounds_may_hold(imp, &map, infer, sigs, 0);
            infer.rollback_to(snap);
            if ok {
                found.push((Pick::Node { node, args }, self_ty));
            }
        }
        if !inherent {
            // Derived implementations of the step's type.
            let target = match adjusted {
                Ty::Ref(inner) => (**inner).clone(),
                other => other.clone(),
            };
            if let Ty::Adt(item, args) = infer.shallow(&target)
                && matches!(adjusted, Ty::Ref(_))
            {
                for (t, node) in self.derived.get(&item).into_iter().flatten() {
                    if DERIVABLE.iter().any(|(d, m)| d == t && m.contains(&name)) && self.program.node(*node).is_some_and(|n| n.ident == name) {
                        found.push((Pick::Node { node: *node, args: args.clone() }, target.clone()));
                    }
                }
            }
            // Methods of the bounds of a generic receiver, and of a trait object's traits.
            let referent = match adjusted {
                Ty::Ref(inner) => infer.shallow(inner),
                other => infer.shallow(other),
            };
            let _ = step;
            if matches!(referent, Ty::Param(_) | Ty::Opaque(_) | Ty::Assoc(..) | Ty::Dyn(_)) {
                for tr in self.bounds_of_ty(&referent) {
                    let Some(decl) = self.trait_declaring_method(tr.id, name) else {
                        continue;
                    };
                    if decl != tr.id {
                        continue;
                    }
                    let Some((sig, generics)) = self.trait_method(tr.id, name) else {
                        continue;
                    };
                    if !sig.receiver {
                        continue;
                    }
                    let mut map = Map::new();
                    if let Some(sg) = generics.first() {
                        map.insert(*sg, referent.clone());
                    }
                    for (g, a) in self.program.item(tr.id).scope.generics.iter().zip(&tr.args) {
                        map.insert(*g, a.clone());
                    }
                    let snap = infer.snapshot();
                    let recv = sig.inputs.first().map(|r| self.fill_infer_ty(&r.subst(&map), infer));
                    let ok = recv.is_some_and(|r| infer.unify(&r, adjusted));
                    infer.rollback_to(snap);
                    if ok {
                        let pick = if matches!(referent, Ty::Dyn(_)) {
                            Pick::Dyn { trait_ref: tr.clone(), self_ty: referent.clone() }
                        } else {
                            Pick::Bound { trait_ref: tr.clone(), self_ty: referent.clone() }
                        };
                        if !found.iter().any(|(p, _)| *p == pick) {
                            found.push((pick, referent.clone()));
                        }
                    }
                }
            }
        }
        if let [(Pick::Node { node, args }, _)] = found.as_slice() {
            // One candidate: keep what it fixes.
            if let Some(r) = self.receiver_of(*node, args) {
                let _ = infer.unify(&r, adjusted);
            }
        }
        found.into_iter().map(|(p, _)| p).collect()
    }

    /// Whether trait `t` or a supertrait has a default body for method `name`.
    fn trait_default_exists(&self, t: ItemId, name: &str) -> bool {
        self.trait_closure(t)
            .iter()
            .any(|x| matches!(&self.program.item(*x).kind, ItemKind::Trait { defaults, .. } if defaults.contains_key(name)))
    }

    /// The receiver type of node `node`'s instance with arguments `args`, if it has a receiver.
    #[must_use]
    pub fn receiver_of(&self, node: NodeId, args: &[Ty]) -> Option<Ty> {
        let sig = self.sig(node)?;
        if !sig.receiver {
            return None;
        }
        let map = self.map_of(node, args);
        sig.inputs.first().map(|t| t.subst(&map))
    }

    /// `args` with every `_` a fresh variable.
    pub fn fill_infer(&self, args: Vec<Ty>, infer: &mut Infer) -> Vec<Ty> {
        args.into_iter().map(|a| self.fill_infer_ty(&a, infer)).collect()
    }

    /// `t` with every `_` a fresh variable.
    pub fn fill_infer_ty(&self, t: &Ty, infer: &mut Infer) -> Ty {
        t.fold(&mut |x| match x {
            Ty::Infer => Some(infer.fresh()),
            _ => None,
        })
    }

    // ---------------------------------------------------------------- paths

    /// The functions `Type::name` may name, `Type` being `base`: inherent associated functions,
    /// then trait implementations' functions (and default bodies), each with its arguments (the
    /// implementation's fixed by matching `base`). For a generic `base`, the bounds' methods.
    pub fn assoc_fns(&self, base: &Ty, name: &str, infer: &mut Infer, sigs: ClosureSigs<'_>) -> Vec<Pick> {
        let base = infer.shallow(base);
        let mut out = Vec::new();
        if matches!(base, Ty::Param(_) | Ty::Opaque(_) | Ty::Assoc(..) | Ty::Dyn(_)) {
            for tr in self.bounds_of_ty(&base) {
                if self.trait_declaring_method(tr.id, name) == Some(tr.id) {
                    let pick = if matches!(base, Ty::Dyn(_)) {
                        Pick::Dyn { trait_ref: tr, self_ty: base.clone() }
                    } else {
                        Pick::Bound { trait_ref: tr, self_ty: base.clone() }
                    };
                    out.push(pick);
                }
            }
            return out;
        }
        for inherent in [true, false] {
            let candidates: Vec<ImplId> = self
                .candidates_for(&base, None)
                .into_iter()
                .filter(|imp| {
                    self.impls.get(imp.0).is_some_and(|info| {
                        info.trait_ref.is_some() != inherent
                            && (self.program.imp(*imp).methods.contains_key(name)
                                || info.trait_ref.as_ref().is_some_and(|tr| self.trait_default_exists(tr.id, name)))
                    })
                })
                .collect();
            for (imp, map) in self.applicable(candidates, &base, None, infer, sigs) {
                let Some(info) = self.impls.get(imp.0) else {
                    continue;
                };
                let self_ty = info.self_ty.subst(&map);
                if let Some((node, args)) = self.impl_fn(imp, name, &map, &self_ty, &[]) {
                    out.push(Pick::Node { node, args });
                }
            }
            if inherent && !out.is_empty() {
                return out;
            }
        }
        if let Ty::Adt(item, args) = &base {
            for (t, node) in self.derived.get(item).into_iter().flatten() {
                if DERIVABLE.iter().any(|(d, m)| d == t && m.contains(&name)) {
                    out.push(Pick::Node { node: *node, args: args.clone() });
                }
            }
        }
        out
    }

    /// The type of associated constant `name` of `base`: from an implementation for it, or a
    /// trait among a generic `base`'s bounds.
    pub fn assoc_const(&self, base: &Ty, name: &str, infer: &mut Infer, sigs: ClosureSigs<'_>) -> Option<Ty> {
        let base = infer.shallow(base);
        if matches!(base, Ty::Param(_) | Ty::Opaque(_) | Ty::Assoc(..)) {
            for tr in self.bounds_of_ty(&base) {
                let item = self.program.item(tr.id);
                if let ItemKind::Trait { consts, self_gen, .. } = &item.kind
                    && let Some(t) = consts.get(name)
                {
                    let mut map: Map = item.scope.generics.iter().copied().zip(tr.args.iter().cloned()).collect();
                    map.insert(*self_gen, base.clone());
                    return Some(self.resolver.ty(&item.scope, t).subst(&map));
                }
            }
            return None;
        }
        let candidates: Vec<ImplId> = self
            .candidates_for(&base, None)
            .into_iter()
            .filter(|imp| self.program.imp(*imp).consts.contains_key(name))
            .collect();
        let (imp, map) = self.applicable(candidates, &base, None, infer, sigs).into_iter().next()?;
        let it = self.program.imp(imp);
        let t = it.consts.get(name)?;
        let mut scope = it.scope.clone();
        scope.self_ref = SelfRef::Impl(imp);
        Some(self.resolver.ty(&scope, t).subst(&map))
    }

    // ---------------------------------------------------------------- normalisation

    /// `t` with its projections resolved where the implementations, bounds or closure signatures
    /// say what they are; a projection whose base is not known yet stays.
    pub fn normalize(&self, t: &Ty, infer: &mut Infer, sigs: ClosureSigs<'_>) -> Ty {
        self.normalize_at(t, infer, sigs, 0)
    }

    fn normalize_at(&self, t: &Ty, infer: &mut Infer, sigs: ClosureSigs<'_>, depth: usize) -> Ty {
        if depth > crate::recursion::ty::MAX_DEPTH.saturating_mul(2) {
            return Ty::Unknown;
        }
        let t = infer.resolve(t);
        if !t.any(&|x| matches!(x, Ty::Assoc(..))) {
            return t;
        }
        t.fold(&mut |x| match x {
            Ty::Assoc(base, tr, name) => Some(self.project(base, *tr, name, infer, sigs, depth)),
            _ => None,
        })
    }

    /// `<base as tr>::name`.
    fn project(&self, base: &Ty, tr: Option<ItemId>, name: &str, infer: &mut Infer, sigs: ClosureSigs<'_>, depth: usize) -> Ty {
        let keep = || Ty::Assoc(Box::new(base.clone()), tr, name.to_owned());
        let base = infer.shallow(base);
        match &base {
            Ty::Var(_) | Ty::Unknown | Ty::Infer => keep(),
            Ty::Param(_) | Ty::Opaque(_) | Ty::Dyn(_) | Ty::Assoc(..) => {
                for b in self.bounds_of_ty(&base) {
                    if let Some(t) = b.binding(name) {
                        return self.normalize_at(t, infer, sigs, depth.saturating_add(1));
                    }
                }
                keep()
            }
            Ty::Closure(k) if name == "Output" => sigs(k).map_or_else(keep, |(_, out)| out),
            Ty::FnDef(n, args) if name == "Output" => {
                let map = self.map_of(*n, args);
                self.sig(*n).map_or_else(keep, |s| s.output.subst(&map))
            }
            Ty::FnPtr(_, out) if name == "Output" => (**out).clone(),
            Ty::Ref(inner) if name == "Output" && matches!(infer.shallow(inner), Ty::Closure(_) | Ty::FnDef(..) | Ty::FnPtr(..)) => {
                self.project(inner, tr, name, infer, sigs, depth)
            }
            _ => {
                let traits: Vec<ItemId> = match tr {
                    Some(t) => vec![t],
                    None => self
                        .candidates_for(&base, None)
                        .into_iter()
                        .filter(|i| self.program.imp(*i).assoc.contains_key(name))
                        .filter_map(|i| self.impls.get(i.0).and_then(|x| x.trait_ref.as_ref()).map(|x| x.id))
                        .collect(),
                };
                for t in traits {
                    let candidates: Vec<ImplId> = self
                        .candidates_for(&base, Some(t))
                        .into_iter()
                        .filter(|imp| self.program.imp(*imp).assoc.contains_key(name))
                        .collect();
                    // One implementation applies: its definition, with what matching it fixed
                    // kept. Several (open variables): the projection waits.
                    let found = self.applicable(candidates, &base, None, infer, sigs);
                    if let [(imp, map)] = found.as_slice() {
                        let it = self.program.imp(*imp);
                        let Some(written) = it.assoc.get(name) else {
                            continue;
                        };
                        let mut scope = it.scope.clone();
                        scope.self_ref = SelfRef::Impl(*imp);
                        let resolved = self.resolver.ty(&scope, written).subst(map);
                        return self.normalize_at(&resolved, infer, sigs, depth.saturating_add(1));
                    }
                    if !found.is_empty() {
                        return keep();
                    }
                }
                keep()
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
    use super::*;
    use crate::recursion::model::{CrateSource, Role, load};

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

    fn item(p: &Program, name: &str) -> ItemId {
        ItemId(p.items.iter().position(|i| i.path == format!("a::{name}")).unwrap())
    }

    fn none(_: &ClosureKey) -> Option<(Vec<Ty>, Ty)> {
        None
    }

    fn names(ix: &Index<'_>, picks: &[Pick]) -> Vec<String> {
        let mut v: Vec<String> = picks
            .iter()
            .map(|p| match p {
                Pick::Node { node, .. } => ix.name(*node).to_owned(),
                Pick::Bound { trait_ref, .. } => format!("bound {}", ix.program.item(trait_ref.id).name),
                Pick::Dyn { trait_ref, .. } => format!("dyn {}", ix.program.item(trait_ref.id).name),
            })
            .collect();
        v.sort();
        v
    }

    #[test]
    fn methods_follow_the_receivers_type_and_auto_dereference() {
        let p = program(
            "pub struct A; pub struct B { a: A }\n\
             impl A { pub fn m(&self) {} pub fn own(self) {} }\n\
             impl B { pub fn m(&self) { self.a.m() } }\n\
             impl core::ops::Deref for B { type Target = A; fn deref(&self) -> &A { &self.a } }\n\
             pub trait T { fn t(&self) {} fn r(&self); }\n\
             impl T for A { fn r(&self) {} }\n\
             #[derive(Clone, Debug)] pub struct C;",
        );
        assert!(p.problems.is_empty(), "{:?}", p.problems);
        let ix = Index::new(&p);
        let mut infer = Infer::new();
        let a = Ty::Adt(item(&p, "A"), Vec::new());
        let b = Ty::Adt(item(&p, "B"), Vec::new());
        assert_eq!(names(&ix, &ix.probe(&a, "m", &mut infer, &none).0), ["a::A::m"]);
        assert_eq!(names(&ix, &ix.probe(&b, "m", &mut infer, &none).0), ["a::B::m"], "inherent first, no deref");
        assert_eq!(names(&ix, &ix.probe(&Ty::reference(b.clone()), "r", &mut infer, &none).0), ["<a::A as T>::r"]);
        assert_eq!(names(&ix, &ix.probe(&a, "t", &mut infer, &none).0), ["a::T::t"], "the default body");
        assert_eq!(names(&ix, &ix.probe(&b, "own", &mut infer, &none).0), ["a::A::own"], "lookup follows Deref to a by-value method (moving out is the borrow checker's to refuse)");
        let c = Ty::Adt(item(&p, "C"), Vec::new());
        assert_eq!(names(&ix, &ix.probe(&c, "clone", &mut infer, &none).0), ["<a::C as Clone>::clone"]);
    }

    #[test]
    fn generic_implementations_match_and_fix_their_parameters() {
        let p = program(
            "pub struct W<T>(T); #[derive(Clone)] pub struct X; pub struct NoClone;\n\
             impl<T: Clone> W<T> { pub fn get(&self) -> T { self.0.clone() } }\n\
             impl W<f64> { pub fn only_f(&self) {} }",
        );
        let ix = Index::new(&p);
        let mut infer = Infer::new();
        let w = item(&p, "W");
        let x = Ty::Adt(item(&p, "X"), Vec::new());
        let wx = Ty::Adt(w, vec![x.clone()]);
        let (picks, _) = ix.probe(&wx, "get", &mut infer, &none);
        match picks.as_slice() {
            [Pick::Node { args, .. }] => assert_eq!(infer.resolve(&args[0]), x),
            other => panic!("{other:?}"),
        }
        assert!(ix.probe(&wx, "only_f", &mut infer, &none).0.is_empty(), "W<f64> only");
        let wn = Ty::Adt(w, vec![Ty::Adt(item(&p, "NoClone"), Vec::new())]);
        assert!(ix.probe(&wn, "get", &mut infer, &none).0.is_empty(), "the implementation's bound does not hold");
        let wf = Ty::Adt(w, vec![Ty::Prim(Prim::F64)]);
        assert_eq!(ix.probe(&wf, "only_f", &mut infer, &none).0.len(), 1);
    }

    #[test]
    fn generic_receivers_pick_their_bounds() {
        let p = program(
            "pub trait S { fn s(&self); fn d(&self) {} }\n\
             pub struct A;\n\
             impl S for A { fn s(&self) {} }\n\
             pub fn g<T: S>(t: T) { t.s() }",
        );
        let ix = Index::new(&p);
        let mut infer = Infer::new();
        let g = p.nodes.iter().position(|n| n.ident == "g").unwrap();
        let tg = ix.node_generics(NodeId(g))[0];
        assert_eq!(names(&ix, &ix.probe(&Ty::Param(tg), "s", &mut infer, &none).0), ["bound S"]);
        assert_eq!(names(&ix, &ix.probe(&Ty::Param(tg), "d", &mut infer, &none).0), ["bound S"]);
    }

    #[test]
    fn associated_types_resolve_through_implementations_and_bounds() {
        let p = program(
            "pub struct It; pub struct Item;\n\
             impl Iterator for It { type Item = Item; fn next(&mut self) -> Option<Item> { None } }\n\
             pub fn g<I: Iterator<Item = u8>>(i: I) {}",
        );
        let ix = Index::new(&p);
        let mut infer = Infer::new();
        let it = Ty::Adt(item(&p, "It"), Vec::new());
        let iter = ix.lang.trait_named("Iterator").unwrap();
        let n = ix.normalize(&Ty::Assoc(Box::new(it), Some(iter), "Item".to_owned()), &mut infer, &none);
        assert_eq!(n, Ty::Adt(item(&p, "Item"), Vec::new()));
        let g = p.nodes.iter().position(|n| n.ident == "g").unwrap();
        let ig = ix.node_generics(NodeId(g))[0];
        let b = ix.normalize(&Ty::Assoc(Box::new(Ty::Param(ig)), Some(iter), "Item".to_owned()), &mut infer, &none);
        assert_eq!(b, Ty::Prim(Prim::U8));
    }

    #[test]
    fn operator_impls_are_selected_by_both_operands() {
        let p = program(
            "pub struct V;\n\
             impl core::ops::Mul<f64> for V { type Output = V; fn mul(self, k: f64) -> V { V } }\n\
             impl core::ops::Mul for V { type Output = f64; fn mul(self, k: V) -> f64 { 0.0 } }",
        );
        let ix = Index::new(&p);
        let mut infer = Infer::new();
        let v = Ty::Adt(item(&p, "V"), Vec::new());
        let mul = TraitRef { id: ix.lang.trait_named("Mul").unwrap(), args: vec![Ty::Prim(Prim::F64)], bindings: Vec::new() };
        assert_eq!(ix.select(&v, &mul, &mut infer, &none).len(), 1);
        let any = TraitRef { id: mul.id, args: vec![infer.fresh()], bindings: Vec::new() };
        assert_eq!(ix.select(&v, &any, &mut infer, &none).len(), 2, "an open right-hand side keeps both");
    }
}
