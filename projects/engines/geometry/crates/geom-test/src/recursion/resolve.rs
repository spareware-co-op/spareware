//! Name resolution for the recursion check: what a path in the kernel's source names, and what
//! type a written type is.
//!
//! What it does: [`Resolver::resolve`] follows a path as the compiler's name resolution does:
//! `crate`, `self`, `super` and `Self`; generic and const parameters; the items of a function
//! body's scope, then of its module; `use` declarations, renamed, grouped, `self` and glob, and
//! the re-exports they lead through; the other crates read by name, the standard library's model
//! among them under its names `core`, `alloc` and `std` ([`crate::recursion::stdlib`]); and the
//! standard prelude, from the model's `prelude` module. Later segments step into modules, enum
//! variants and associated items. [`Resolver::ty`] turns a written type into a [`Ty`], expanding
//! type aliases and `Self`, and [`Resolver::trait_ref`] a bound into a [`TraitRef`] with its
//! associated-type bindings (`Fn(A) -> R` as `Fn<(A,)>` with `Output = R`).
//!
//! Guarantees:
//!
//! - **Every alternative.** A name defined twice in one namespace (as under `#[cfg]`s that are
//!   both kept, [`crate::recursion::cfg`]) resolves to both, so a call reaches every function it
//!   may name in some build.
//! - **Terminates on any input.** `use` cycles and glob cycles are cut by a visited set; alias
//!   expansion stops at a fixed depth ([`ALIAS_DEPTH`]).
//! - **Outside is outside.** A path is an outside path only when it starts at a crate that is
//!   neither read nor modelled (or, without a model, at the standard library or its prelude); a
//!   path that resolves nowhere resolves to nothing, and the type checker reports it.
//!
//! Method: The Rust Reference on paths, namespaces, `use` declarations and the preludes
//! (<https://doc.rust-lang.org/reference/paths.html>,
//! <https://doc.rust-lang.org/reference/names/namespaces.html>,
//! <https://doc.rust-lang.org/reference/items/use-declarations.html>,
//! <https://doc.rust-lang.org/reference/names/preludes.html>); visibility is not checked (code
//! that compiles only names what it may see, so ignoring visibility only adds alternatives).
//!
//! Cost: each lookup is linear in the number of `use` declarations it follows, memoised per
//! module, name and namespace; no recursion (lookups use an explicit work list).

use core::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use crate::recursion::model::{Def, ItemKind, ModId, Program, Scope, SelfRef};
use crate::recursion::ty::{GenId, ItemId, NodeId, Prim, TraitRef, Ty};

/// What a path names.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Res {
    /// A module.
    Mod(ModId),
    /// A struct, enum, union, trait, alias, constant or static.
    Item(ItemId),
    /// An enum variant.
    Variant(ItemId, usize),
    /// A free function.
    Fn(NodeId),
    /// A path outside the kernel, as written from its crate or prelude name.
    Ext(Vec<String>),
    /// A primitive type.
    Prim(String),
    /// A generic type parameter.
    Generic(GenId),
    /// `Self`.
    SelfTy,
    /// A const generic parameter.
    ConstParam(String),
    /// An associated item (function, constant or type) of what the first part names.
    Assoc(Box<Res>, String),
    /// A `macro_rules!` macro.
    Macro(crate::recursion::model::MacroId),
    /// A qualified path's base, `<T as Trait>` or `<T>`: the type, and the trait when written.
    Qualified(Box<Ty>, Option<Box<Res>>),
}

/// A namespace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Ns {
    /// Types, traits and modules.
    Type,
    /// Functions, constants, statics and constructors.
    Value,
    /// Macros.
    Macro,
}

/// The crates outside the kernel that kernel code may name.
pub const OUTSIDE_CRATES: [&str; 6] = ["core", "alloc", "std", "libm", "proc_macro", "test"];

/// The names the standard library's model answers to: the crate read as `core` is also `alloc`
/// and `std`.
pub const MODEL_ALIASES: [&str; 2] = ["alloc", "std"];

/// The names of the standard prelude (Rust 2024): every one is outside the kernel.
pub const PRELUDE: [&str; 46] = [
    "Copy",
    "Send",
    "Sized",
    "Sync",
    "Unpin",
    "Drop",
    "Fn",
    "FnMut",
    "FnOnce",
    "AsyncFn",
    "AsyncFnMut",
    "AsyncFnOnce",
    "drop",
    "size_of",
    "size_of_val",
    "align_of",
    "align_of_val",
    "Box",
    "ToOwned",
    "Clone",
    "PartialEq",
    "PartialOrd",
    "Eq",
    "Ord",
    "AsRef",
    "AsMut",
    "Into",
    "From",
    "Default",
    "Iterator",
    "Extend",
    "IntoIterator",
    "DoubleEndedIterator",
    "ExactSizeIterator",
    "Option",
    "Some",
    "None",
    "Result",
    "Ok",
    "Err",
    "String",
    "ToString",
    "Vec",
    "TryFrom",
    "TryInto",
    "FromIterator",
];

/// How deep type aliases are expanded.
pub const ALIAS_DEPTH: usize = 16;

/// Resolves names in a [`Program`], with a memo of module lookups.
pub struct Resolver<'p> {
    /// The program.
    pub program: &'p Program,
    cache: RefCell<BTreeMap<(ModId, String, Ns), Vec<Res>>>,
    /// The crate root of each crate read, by the name its code is referred to by.
    crates: BTreeMap<String, ModId>,
    /// The standard library model's `prelude` module, when the model is read.
    prelude: Option<ModId>,
}

impl<'p> Resolver<'p> {
    /// A resolver for `program`.
    #[must_use]
    pub fn new(program: &'p Program) -> Resolver<'p> {
        let mut crates: BTreeMap<String, ModId> = program
            .crates
            .iter()
            .map(|c| (c.source.lib.clone(), c.root))
            .collect();
        let model_core = program
            .crates
            .iter()
            .find(|c| {
                c.source.role == crate::recursion::model::Role::Model && c.source.lib == "core"
            })
            .map(|c| c.root);
        let mut prelude = None;
        if let Some(root) = model_core {
            for alias in MODEL_ALIASES {
                crates.entry(alias.to_owned()).or_insert(root);
            }
            prelude = program
                .module(root)
                .types
                .get("prelude")
                .into_iter()
                .flatten()
                .find_map(|d| match d {
                    Def::Mod(m) => Some(*m),
                    _ => None,
                });
        }
        Resolver {
            program,
            cache: RefCell::new(BTreeMap::new()),
            crates,
            prelude,
        }
    }

    /// Whether the standard library's model is read.
    #[must_use]
    pub fn has_model(&self) -> bool {
        self.prelude.is_some()
    }

    /// The root module of the crate `module` belongs to.
    #[must_use]
    pub fn crate_root(&self, module: ModId) -> ModId {
        let krate = self.program.module(module).krate;
        self.program.crates.get(krate).map_or(module, |c| c.root)
    }

    /// The root module of crate number `krate`.
    #[must_use]
    pub fn root_of(&self, krate: usize) -> Option<ModId> {
        self.program.crates.get(krate).map(|c| c.root)
    }

    /// What `name` is in `module` itself, in `ns`: its items, then its `use` bindings, then its
    /// glob imports.
    #[must_use]
    pub fn lookup(&self, module: ModId, name: &str, ns: Ns) -> Vec<Res> {
        let key = (module, name.to_owned(), ns);
        if let Some(hit) = self.cache.borrow().get(&key) {
            return hit.clone();
        }
        let mut visited = BTreeSet::new();
        let found = self.lookup_in(module, name, ns, &mut visited);
        self.cache.borrow_mut().insert(key, found.clone());
        found
    }

    fn lookup_in(
        &self,
        module: ModId,
        name: &str,
        ns: Ns,
        visited: &mut BTreeSet<(ModId, String, Ns)>,
    ) -> Vec<Res> {
        if !visited.insert((module, name.to_owned(), ns)) {
            return Vec::new();
        }
        let m = self.program.module(module);
        let mut out: Vec<Res> = Vec::new();
        let defs = match ns {
            Ns::Type => m.types.get(name),
            Ns::Value => m.values.get(name),
            Ns::Macro => None,
        };
        if let Some(defs) = defs {
            out.extend(defs.iter().map(def_res));
        }
        if ns == Ns::Macro
            && let Some(ids) = m.macros.get(name)
        {
            out.extend(ids.iter().map(|id| Res::Macro(*id)));
        }
        let home = self.program.non_block(module);
        let scope = Scope {
            module: home,
            generics: Vec::new(),
            consts: Vec::new(),
            self_ref: SelfRef::None,
        };
        for u in &m.uses {
            if u.name.as_deref() != Some(name) {
                continue;
            }
            out.extend(self.resolve_with(&scope, &u.segments, u.leading_colon, ns, visited));
        }
        if !out.is_empty() {
            out.sort();
            out.dedup();
            return out;
        }
        for u in m.uses.iter().filter(|u| u.name.is_none()) {
            for source in self.resolve_with(&scope, &u.segments, u.leading_colon, Ns::Type, visited)
            {
                match source {
                    Res::Mod(inner) => out.extend(self.lookup_in(inner, name, ns, visited)),
                    Res::Item(id) => {
                        if let ItemKind::Enum { variants, .. } = &self.program.item(id).kind
                            && let Some(i) = variants.iter().position(|v| v.name == name)
                            && ns != Ns::Macro
                        {
                            out.push(Res::Variant(id, i));
                        }
                    }
                    Res::Ext(mut path) => {
                        path.push(name.to_owned());
                        out.push(Res::Ext(path));
                    }
                    _ => {}
                }
            }
        }
        // A kernel name found through a glob hides the outside guesses of other globs.
        if out.iter().any(|r| !matches!(r, Res::Ext(_))) {
            out.retain(|r| !matches!(r, Res::Ext(_)));
        }
        out.sort();
        out.dedup();
        out
    }

    /// What `segments` names from `scope`, the last segment in `ns`.
    #[must_use]
    pub fn resolve(&self, scope: &Scope, segments: &[String], leading: bool, ns: Ns) -> Vec<Res> {
        let mut visited = BTreeSet::new();
        self.resolve_with(scope, segments, leading, ns, &mut visited)
    }

    fn resolve_with(
        &self,
        scope: &Scope,
        segments: &[String],
        leading: bool,
        ns: Ns,
        visited: &mut BTreeSet<(ModId, String, Ns)>,
    ) -> Vec<Res> {
        let Some((first, rest)) = segments.split_first() else {
            return Vec::new();
        };
        let first_ns = if rest.is_empty() { ns } else { Ns::Type };
        let mut current = if leading {
            self.extern_crate(first)
        } else {
            self.first(scope, first, first_ns, visited)
        };
        for (i, seg) in rest.iter().enumerate() {
            let last = i.saturating_add(1) == rest.len();
            let seg_ns = if last { ns } else { Ns::Type };
            let mut next = Vec::new();
            for res in current {
                next.extend(self.step(&res, seg, seg_ns, visited));
            }
            next.sort();
            next.dedup();
            current = next;
        }
        current
    }

    fn extern_crate(&self, name: &str) -> Vec<Res> {
        if let Some(root) = self.crates.get(name) {
            return vec![Res::Mod(*root)];
        }
        if OUTSIDE_CRATES.contains(&name) {
            return vec![Res::Ext(vec![name.to_owned()])];
        }
        Vec::new()
    }

    fn first(
        &self,
        scope: &Scope,
        name: &str,
        ns: Ns,
        visited: &mut BTreeSet<(ModId, String, Ns)>,
    ) -> Vec<Res> {
        let home = self.program.non_block(scope.module);
        match name {
            "crate" | "$crate" => return vec![Res::Mod(self.crate_root(scope.module))],
            "self" => return vec![Res::Mod(home)],
            "super" => {
                return self
                    .program
                    .module(home)
                    .parent
                    .map(|p| vec![Res::Mod(self.program.non_block(p))])
                    .unwrap_or_default();
            }
            "Self" => return vec![Res::SelfTy],
            _ => {}
        }
        if ns == Ns::Type
            && let Some(g) = scope
                .generics
                .iter()
                .rev()
                .find(|g| self.program.generic(**g).name == name)
        {
            return vec![Res::Generic(*g)];
        }
        if ns == Ns::Value && scope.consts.iter().any(|c| c == name) {
            return vec![Res::ConstParam(name.to_owned())];
        }
        // The function body's scopes, innermost first, then the module.
        let mut m = Some(scope.module);
        while let Some(id) = m {
            let found = self.lookup_in(id, name, ns, visited);
            if !found.is_empty() {
                return found;
            }
            if self.program.module(id).block_of.is_none() {
                break;
            }
            m = self.program.module(id).parent;
        }
        let found = self.extern_crate(name);
        if !found.is_empty() {
            return found;
        }
        if ns == Ns::Type && Prim::from_name(name).is_some() {
            return vec![Res::Prim(name.to_owned())];
        }
        if let Some(prelude) = self.prelude {
            return self.lookup_in(prelude, name, ns, visited);
        }
        if PRELUDE.contains(&name) {
            return vec![Res::Ext(vec![name.to_owned()])];
        }
        Vec::new()
    }

    fn step(
        &self,
        res: &Res,
        seg: &str,
        ns: Ns,
        visited: &mut BTreeSet<(ModId, String, Ns)>,
    ) -> Vec<Res> {
        match res {
            Res::Mod(m) => self.lookup_in(*m, seg, ns, visited),
            Res::Item(id) => {
                if let ItemKind::Enum { variants, .. } = &self.program.item(*id).kind
                    && let Some(i) = variants.iter().position(|v| v.name == seg)
                {
                    return vec![Res::Variant(*id, i)];
                }
                vec![Res::Assoc(Box::new(res.clone()), seg.to_owned())]
            }
            Res::Generic(_) | Res::SelfTy | Res::Prim(_) | Res::Assoc(..) => {
                vec![Res::Assoc(Box::new(res.clone()), seg.to_owned())]
            }
            Res::Ext(path) => {
                let mut p = path.clone();
                p.push(seg.to_owned());
                vec![Res::Ext(p)]
            }
            Res::Qualified(..) => vec![Res::Assoc(Box::new(res.clone()), seg.to_owned())],
            Res::Variant(..) | Res::Fn(_) | Res::ConstParam(_) | Res::Macro(_) => Vec::new(),
        }
    }

    /// The segments of a `syn` path, as strings.
    #[must_use]
    pub fn segments(path: &syn::Path) -> Vec<String> {
        path.segments.iter().map(|s| s.ident.to_string()).collect()
    }

    /// The type arguments written on one path segment, in `scope` (`Vec<T>`: `T`; `Fn(A, B)`:
    /// `A`, `B`); lifetimes and const arguments are left out.
    #[must_use]
    pub fn segment_args(&self, scope: &Scope, seg: &syn::PathSegment) -> Vec<Ty> {
        self.args(scope, seg, 0)
    }

    /// Whether a written type is a const generic parameter in `scope` (which `syn` reads as a
    /// type path when it is a generic argument).
    fn is_const_param(scope: &Scope, ty: &syn::Type) -> bool {
        matches!(ty, syn::Type::Path(tp) if tp.qself.is_none()
            && tp.path.get_ident().is_some_and(|i| scope.consts.iter().any(|c| i == c)))
    }

    fn args(&self, scope: &Scope, seg: &syn::PathSegment, depth: usize) -> Vec<Ty> {
        match &seg.arguments {
            syn::PathArguments::AngleBracketed(a) => a
                .args
                .iter()
                .filter_map(|arg| match arg {
                    syn::GenericArgument::Type(t) if !Self::is_const_param(scope, t) => {
                        Some(self.ty_at(scope, t, depth))
                    }
                    _ => None,
                })
                .collect(),
            syn::PathArguments::Parenthesized(p) => p
                .inputs
                .iter()
                .map(|t| self.ty_at(scope, &t.ty, depth))
                .collect(),
            syn::PathArguments::None => Vec::new(),
        }
    }

    /// The associated-type bindings written on one path segment (`Iterator<Item = T>`).
    fn bindings(&self, scope: &Scope, seg: &syn::PathSegment, depth: usize) -> Vec<(String, Ty)> {
        match &seg.arguments {
            syn::PathArguments::AngleBracketed(a) => a
                .args
                .iter()
                .filter_map(|arg| match arg {
                    syn::GenericArgument::AssocType(b) => {
                        Some((b.ident.to_string(), self.ty_at(scope, &b.ty, depth)))
                    }
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The type a written type denotes in `scope`.
    #[must_use]
    pub fn ty(&self, scope: &Scope, ty: &syn::Type) -> Ty {
        self.ty_at(scope, ty, 0)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one arm per form of the type grammar; splitting would scatter it"
    )]
    fn ty_at(&self, scope: &Scope, ty: &syn::Type, depth: usize) -> Ty {
        if depth > ALIAS_DEPTH {
            return Ty::Unknown;
        }
        match ty {
            syn::Type::Reference(r) => Ty::Ref(
                Box::new(self.ty_at(scope, &r.elem, depth)),
                r.mutability.is_some(),
            ),
            syn::Type::Ptr(p) => Ty::Ref(
                Box::new(self.ty_at(scope, &p.elem, depth)),
                matches!(p.mutability, syn::PointerMutability::Mut(_)),
            ),
            syn::Type::Slice(s) => Ty::Slice(Box::new(self.ty_at(scope, &s.elem, depth))),
            syn::Type::Array(a) => Ty::Array(Box::new(self.ty_at(scope, &a.elem, depth))),
            syn::Type::Tuple(t) => Ty::Tuple(
                t.elems
                    .iter()
                    .map(|e| self.ty_at(scope, e, depth))
                    .collect(),
            ),
            syn::Type::Paren(p) => self.ty_at(scope, &p.elem, depth),
            syn::Type::Group(g) => self.ty_at(scope, &g.elem, depth),
            syn::Type::Never(_) => Ty::Never,
            syn::Type::Infer(_) => Ty::Infer,
            syn::Type::FnPtr(f) => {
                let params = f
                    .inputs
                    .iter()
                    .map(|a| self.ty_at(scope, &a.ty, depth))
                    .collect();
                let ret = match &f.output {
                    syn::ReturnType::Default => Ty::unit(),
                    syn::ReturnType::Type(_, t) => self.ty_at(scope, t, depth),
                };
                Ty::FnPtr(params, Box::new(ret))
            }
            syn::Type::TraitObject(o) => Ty::Dyn(self.trait_refs(scope, o.bounds.iter())),
            syn::Type::ImplTrait(i) => Ty::Opaque(self.trait_refs(scope, i.bounds.iter())),
            syn::Type::Path(tp) => {
                if let Some(q) = &tp.qself {
                    let base = self.ty_at(scope, &q.ty, depth);
                    let name = tp
                        .path
                        .segments
                        .last()
                        .map(|s| s.ident.to_string())
                        .unwrap_or_default();
                    let trait_ref = if q.position > 0 {
                        let trait_path: Vec<String> = tp
                            .path
                            .segments
                            .iter()
                            .take(q.position)
                            .map(|s| s.ident.to_string())
                            .collect();
                        let id = self
                            .resolve(
                                scope,
                                &trait_path,
                                tp.path.leading_colon.is_some(),
                                Ns::Type,
                            )
                            .into_iter()
                            .find_map(|r| match r {
                                Res::Item(id)
                                    if matches!(
                                        self.program.item(id).kind,
                                        ItemKind::Trait { .. }
                                    ) =>
                                {
                                    Some(id)
                                }
                                _ => None,
                            });
                        let seg = tp.path.segments.iter().nth(q.position.saturating_sub(1));
                        id.zip(seg)
                            .map(|(id, seg)| self.trait_ref_of(scope, id, seg, depth))
                    } else {
                        None
                    };
                    return Ty::Assoc(Box::new(base), trait_ref.map(Box::new), name);
                }
                let segs = Self::segments(&tp.path);
                let resolved =
                    self.resolve(scope, &segs, tp.path.leading_colon.is_some(), Ns::Type);
                let last = tp.path.segments.last();
                let args = last.map(|s| self.args(scope, s, depth)).unwrap_or_default();
                let Some(res) = resolved.into_iter().next() else {
                    return Ty::Unknown;
                };
                if let (Res::Item(id), Some(seg)) = (&res, last)
                    && matches!(self.program.item(*id).kind, ItemKind::Trait { .. })
                {
                    // A trait named as a type: a trait object (edition 2015 syntax).
                    return Ty::Dyn(vec![self.trait_ref_of(scope, *id, seg, depth)]);
                }
                self.ty_of_res(scope, &res, args, depth)
            }
            _ => Ty::Unknown,
        }
    }

    /// The type a resolution names with type arguments `args`, in `scope`.
    #[must_use]
    pub fn res_ty(&self, scope: &Scope, res: &Res, args: Vec<Ty>) -> Ty {
        self.ty_of_res(scope, res, args, 0)
    }

    fn ty_of_res(&self, scope: &Scope, res: &Res, args: Vec<Ty>, depth: usize) -> Ty {
        match res {
            Res::Prim(p) => Prim::from_name(p).map_or(Ty::Unknown, Ty::Prim),
            Res::Generic(g) => Ty::Param(*g),
            Res::SelfTy => self.self_ty(scope, depth),
            Res::Item(id) => {
                let item = self.program.item(*id);
                match &item.kind {
                    ItemKind::Struct { .. } | ItemKind::Union { .. } | ItemKind::Enum { .. } => {
                        Ty::Adt(*id, self.fill_defaults(&item.scope, args, depth))
                    }
                    ItemKind::Alias { ty } => {
                        let args = self.fill_defaults(&item.scope, args, depth);
                        let inner = self.ty_at(&item.scope, ty, depth.saturating_add(1));
                        let map: BTreeMap<GenId, Ty> =
                            item.scope.generics.iter().copied().zip(args).collect();
                        inner.subst(&map)
                    }
                    ItemKind::Trait { .. } | ItemKind::Const { .. } => Ty::Unknown,
                }
            }
            Res::Assoc(base, name) => {
                let base_ty = self.ty_of_res(scope, base, Vec::new(), depth);
                self.assoc_ty(scope, base, base_ty, name, depth)
            }
            Res::Qualified(t, _) => (**t).clone(),
            Res::Ext(_)
            | Res::Mod(_)
            | Res::Variant(..)
            | Res::Fn(_)
            | Res::ConstParam(_)
            | Res::Macro(_) => Ty::Unknown,
        }
    }

    /// `args` for the generic parameters of an item whose scope is `item_scope`, with each one left
    /// out filled by its default (earlier arguments put in) or left to inference.
    fn fill_defaults(&self, item_scope: &Scope, mut args: Vec<Ty>, depth: usize) -> Vec<Ty> {
        let generics = &item_scope.generics;
        while args.len() < generics.len() {
            let Some(g) = generics.get(args.len()) else {
                break;
            };
            let generic = self.program.generic(*g);
            let filled = match &generic.default {
                Some(d) => {
                    let map: BTreeMap<GenId, Ty> =
                        generics.iter().copied().zip(args.iter().cloned()).collect();
                    self.ty_at(&generic.scope, d, depth.saturating_add(1))
                        .subst(&map)
                }
                None => Ty::Infer,
            };
            args.push(filled);
        }
        args.truncate(generics.len());
        args
    }

    /// The type item `id` names when written alone: a struct, enum or union with its arguments
    /// left to inference, or what an alias expands to.
    #[must_use]
    pub fn item_ty(&self, id: ItemId) -> Ty {
        let item = self.program.item(id);
        self.ty_of_res(&item.scope, &Res::Item(id), Vec::new(), 0)
    }

    /// What `Self` is in `scope`: an implementation's self type, a trait's `Self` parameter, or
    /// a type in its own definition.
    #[must_use]
    pub fn self_ty(&self, scope: &Scope, depth: usize) -> Ty {
        match scope.self_ref {
            SelfRef::None => Ty::Unknown,
            SelfRef::Impl(id) => {
                let imp = self.program.imp(id);
                let mut inner = imp.scope.clone();
                inner.self_ref = SelfRef::None;
                self.ty_at(&inner, &imp.self_ty, depth.saturating_add(1))
            }
            SelfRef::Trait(_, g) => Ty::Param(g),
            SelfRef::Adt(id) => Ty::Adt(
                id,
                self.program
                    .item(id)
                    .scope
                    .generics
                    .iter()
                    .map(|g| Ty::Param(*g))
                    .collect(),
            ),
        }
    }

    /// The associated type `name` of `base_ty` (which `base` resolved to): an implementation's
    /// own definition for `Self::Name` inside it; else a projection, with the trait that declares
    /// `name` among the bounds of a generic parameter or `Self` when there is one.
    fn assoc_ty(&self, scope: &Scope, base: &Res, base_ty: Ty, name: &str, depth: usize) -> Ty {
        match (base, scope.self_ref) {
            (Res::SelfTy, SelfRef::Impl(imp)) => {
                let it = self.program.imp(imp);
                if let Some(t) = it.assoc.get(name) {
                    let mut inner = it.scope.clone();
                    inner.self_ref = SelfRef::Impl(imp);
                    return self.ty_at(&inner, t, depth.saturating_add(1));
                }
                let declaring = it
                    .trait_path
                    .as_ref()
                    .and_then(|p| self.trait_ref(&it.scope, p))
                    .and_then(|tr| self.declaring_ref(&[tr], name));
                Ty::Assoc(Box::new(base_ty), declaring.map(Box::new), name.to_owned())
            }
            (Res::SelfTy, SelfRef::Trait(t, _)) => {
                let own = TraitRef {
                    id: t,
                    args: self
                        .program
                        .item(t)
                        .scope
                        .generics
                        .iter()
                        .map(|g| Ty::Param(*g))
                        .collect(),
                    bindings: Vec::new(),
                };
                let declaring = self.declaring_ref(&[own], name);
                Ty::Assoc(Box::new(base_ty), declaring.map(Box::new), name.to_owned())
            }
            (Res::Generic(g), _) => {
                let declaring = self.declaring_ref(&self.bounds(*g), name);
                Ty::Assoc(Box::new(base_ty), declaring.map(Box::new), name.to_owned())
            }
            _ => Ty::Assoc(Box::new(base_ty), None, name.to_owned()),
        }
    }

    /// The bound among `traits` that declares associated type `name`, with its arguments; or the
    /// supertrait that does, its arguments not known here.
    #[must_use]
    pub fn declaring_ref(&self, traits: &[TraitRef], name: &str) -> Option<TraitRef> {
        let id = self.declaring_trait(traits, name)?;
        Some(
            traits
                .iter()
                .find(|t| t.id == id)
                .cloned()
                .unwrap_or(TraitRef {
                    id,
                    args: Vec::new(),
                    bindings: Vec::new(),
                }),
        )
    }

    /// The trait among `traits` and their supertraits that declares associated type `name`.
    #[must_use]
    pub fn declaring_trait(&self, traits: &[TraitRef], name: &str) -> Option<ItemId> {
        let mut work: Vec<ItemId> = traits.iter().map(|t| t.id).collect();
        let mut seen = BTreeSet::new();
        while let Some(t) = work.pop() {
            if !seen.insert(t) {
                continue;
            }
            let item = self.program.item(t);
            if let ItemKind::Trait {
                assoc_types,
                supertraits,
                ..
            } = &item.kind
            {
                if assoc_types.contains_key(name) {
                    return Some(t);
                }
                for s in self.trait_refs(&item.scope, supertraits.iter()) {
                    work.push(s.id);
                }
            }
        }
        None
    }

    /// The traits of a list of bounds; lifetimes, `?Sized` and bounds that name no trait read are
    /// left out.
    #[must_use]
    pub fn trait_refs<'b>(
        &self,
        scope: &Scope,
        bounds: impl Iterator<Item = &'b syn::TypeParamBound>,
    ) -> Vec<TraitRef> {
        bounds
            .filter_map(|b| match b {
                syn::TypeParamBound::Trait(t) if t.maybe.is_none() => {
                    self.trait_ref(scope, &t.path)
                }
                _ => None,
            })
            .collect()
    }

    /// The trait a path names, with its arguments and bindings; a closure trait written
    /// `Fn(A, B) -> R` is `Fn<(A, B)>` with `Output = R`. `None` when the path names no trait the
    /// check reads.
    #[must_use]
    pub fn trait_ref(&self, scope: &Scope, path: &syn::Path) -> Option<TraitRef> {
        let segs = Self::segments(path);
        let id = self
            .resolve(scope, &segs, path.leading_colon.is_some(), Ns::Type)
            .into_iter()
            .find_map(|r| match r {
                Res::Item(id) if matches!(self.program.item(id).kind, ItemKind::Trait { .. }) => {
                    Some(id)
                }
                _ => None,
            })?;
        let seg = path.segments.last()?;
        Some(self.trait_ref_of(scope, id, seg, 0))
    }

    fn trait_ref_of(
        &self,
        scope: &Scope,
        id: ItemId,
        seg: &syn::PathSegment,
        depth: usize,
    ) -> TraitRef {
        if let syn::PathArguments::Parenthesized(p) = &seg.arguments {
            let inputs: Vec<Ty> = p
                .inputs
                .iter()
                .map(|t| self.ty_at(scope, &t.ty, depth))
                .collect();
            let output = match &p.output {
                syn::ReturnType::Type(_, t) => self.ty_at(scope, t, depth),
                syn::ReturnType::Default => Ty::unit(),
            };
            return TraitRef {
                id,
                args: vec![Ty::Tuple(inputs)],
                bindings: vec![("Output".to_owned(), output)],
            };
        }
        TraitRef {
            id,
            args: self.args(scope, seg, depth),
            bindings: self.bindings(scope, seg, depth),
        }
    }

    /// The traits a generic parameter is bounded by, as written (supertraits are the index's,
    /// [`crate::recursion::index::Index::supertraits`]).
    #[must_use]
    pub fn bounds(&self, g: GenId) -> Vec<TraitRef> {
        let generic = self.program.generic(g);
        self.trait_refs(&generic.scope, generic.bounds.iter())
    }
}

fn def_res(def: &Def) -> Res {
    match def {
        Def::Mod(m) => Res::Mod(*m),
        Def::Item(i) => Res::Item(*i),
        Def::Fn(n) => Res::Fn(*n),
        Def::Variant(i, v) => Res::Variant(*i, *v),
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
    use crate::recursion::model::{CrateSource, Role, load};

    fn program(files: &[(&str, &str)]) -> Program {
        let map: BTreeMap<String, String> = files
            .iter()
            .map(|(p, t)| ((*p).to_owned(), (*t).to_owned()))
            .collect();
        let read = move |p: &str| map.get(p).cloned().ok_or_else(|| "no such file".to_owned());
        let sources: Vec<CrateSource> = ["a", "b"]
            .iter()
            .filter(|lib| {
                files
                    .iter()
                    .any(|(p, _)| p.starts_with(&format!("c/{lib}/")))
            })
            .map(|lib| CrateSource {
                package: (*lib).to_owned(),
                lib: (*lib).to_owned(),
                folder: format!("c/{lib}"),
                root: format!("c/{lib}/src/lib.rs"),
                role: Role::Ships,
            })
            .collect();
        load(&sources, &read)
    }

    fn fn_named<'p>(p: &'p Program, name: &str) -> (NodeId, &'p crate::recursion::model::Node) {
        p.nodes
            .iter()
            .enumerate()
            .find(|(_, n)| n.name == name)
            .map(|(i, n)| (NodeId(i), n))
            .unwrap()
    }

    fn segs(path: &str) -> Vec<String> {
        path.split("::").map(str::to_owned).collect()
    }

    #[test]
    #[allow(
        clippy::many_single_char_names,
        reason = "the fixture's functions are f, g and so on"
    )]
    fn uses_globs_reexports_and_cfg_alternatives_resolve() {
        let p = program(&[
            (
                "c/a/src/lib.rs",
                "pub mod m { pub fn f() {} #[cfg(feature = \"x\")] pub fn g() {} \
                 #[cfg(not(feature = \"x\"))] pub fn g() {} }\n\
                 pub use m::f as renamed;\n\
                 mod n { pub use super::m::*; }\n\
                 mod cyc { pub use self::cyc2::*; mod cyc2 { pub use super::*; } }\n\
                 fn user() {}",
            ),
            (
                "c/b/src/lib.rs",
                "use a::n::g; pub enum E { V } use E::*; fn caller() {}",
            ),
        ]);
        let r = Resolver::new(&p);
        let (_, user) = fn_named(&p, "a::user");
        let f = fn_named(&p, "a::m::f").0;
        assert_eq!(
            r.resolve(&user.scope, &segs("renamed"), false, Ns::Value),
            [Res::Fn(f)]
        );
        assert_eq!(
            r.resolve(&user.scope, &segs("n::f"), false, Ns::Value),
            [Res::Fn(f)]
        );
        let gs = r.resolve(&user.scope, &segs("m::g"), false, Ns::Value);
        assert_eq!(gs.len(), 2, "both cfg alternatives: {gs:?}");
        assert_eq!(
            r.resolve(&user.scope, &segs("cyc::nothing"), false, Ns::Value),
            Vec::<Res>::new()
        );
        let (_, caller) = fn_named(&p, "b::caller");
        assert_eq!(
            r.resolve(&caller.scope, &segs("g"), false, Ns::Value).len(),
            2
        );
        assert!(matches!(
            r.resolve(&caller.scope, &segs("V"), false, Ns::Value)
                .as_slice(),
            [Res::Variant(_, 0)]
        ));
        assert_eq!(
            r.resolve(&caller.scope, &segs("core::cmp::max"), false, Ns::Value),
            [Res::Ext(segs("core::cmp::max"))],
            "without the model, the standard library is outside"
        );
        assert_eq!(
            r.resolve(&caller.scope, &segs("Some"), false, Ns::Value),
            [Res::Ext(segs("Some"))]
        );
        assert_eq!(
            r.resolve(&caller.scope, &segs("nowhere"), false, Ns::Value),
            Vec::<Res>::new()
        );
    }

    #[test]
    #[allow(
        clippy::many_single_char_names,
        reason = "the fixture's items are f, w and so on"
    )]
    fn types_expand_aliases_self_and_generics() {
        let p = program(&[(
            "c/a/src/lib.rs",
            "pub struct W<T> { t: T } pub type Wu = W<u64>;\n\
             impl<T: Clone> W<T> { fn get(&self) -> &T { &self.t } }\n\
             fn f(x: Wu, y: &[W<bool>], z: &dyn core::fmt::Debug) {}",
        )]);
        let r = Resolver::new(&p);
        let (_, f) = fn_named(&p, "a::f");
        let sig = f.sig.as_ref().unwrap();
        let tys: Vec<Ty> = sig
            .inputs
            .iter()
            .map(|i| match i {
                syn::FnArg::Typed(t) => r.ty(&f.scope, &t.ty),
                syn::FnArg::Receiver(_) => Ty::Unknown,
            })
            .collect();
        let w = ItemId(0);
        assert_eq!(tys[0], Ty::Adt(w, vec![Ty::Prim(Prim::U64)]));
        assert_eq!(
            tys[1],
            Ty::reference(Ty::Slice(Box::new(Ty::Adt(w, vec![Ty::Prim(Prim::Bool)]))))
        );
        assert_eq!(
            tys[2],
            Ty::reference(Ty::Dyn(Vec::new())),
            "an outside trait is no bound"
        );
        let get = p.nodes.iter().find(|n| n.ident == "get").unwrap();
        assert!(
            matches!(r.self_ty(&get.scope, 0), Ty::Adt(id, ref a) if id == w && matches!(a[0], Ty::Param(_)))
        );
    }
}
