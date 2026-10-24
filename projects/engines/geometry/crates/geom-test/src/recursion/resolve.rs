//! Name resolution for the recursion check: what a path in the kernel's source names, and what
//! type a written type is.
//!
//! What it does: [`Resolver::resolve`] follows a path as the compiler's name resolution does:
//! `crate`, `self`, `super` and `Self`; generic and const parameters; the items of a function
//! body's scope, then of its module; `use` declarations, renamed, grouped, `self` and glob, and
//! the re-exports they lead through; the other crates read by name; and the outside crates
//! (`core`, `alloc`, `std`, `libm`) and the standard prelude, which resolve to outside paths.
//! Later segments step into modules, enum variants and associated items. [`Resolver::ty`] turns a
//! written type into a [`Ty`], expanding type aliases and `Self`, and [`Resolver::trait_ref`] a
//! bound into a [`TraitRef`].
//!
//! Guarantees:
//!
//! - **Every alternative.** A name defined twice in one namespace (as under `#[cfg]`s that are
//!   both kept, [`crate::recursion::cfg`]) resolves to both, so a call reaches every function it
//!   may name in some build.
//! - **Terminates on any input.** `use` cycles and glob cycles are cut by a visited set; alias
//!   expansion stops at a fixed depth ([`ALIAS_DEPTH`]).
//! - **Outside is outside.** A path is an outside path only when it starts at an outside crate,
//!   the standard prelude, or an outside glob import; a path that resolves nowhere resolves to
//!   nothing, and the body analysis reports it.
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
use crate::recursion::ty::{GenId, ItemId, NodeId, PRIMITIVES, TraitRef, Ty};

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

/// The names of the standard prelude (Rust 2024): every one is outside the kernel.
pub const PRELUDE: [&str; 46] = [
    "Copy", "Send", "Sized", "Sync", "Unpin", "Drop", "Fn", "FnMut", "FnOnce", "AsyncFn",
    "AsyncFnMut", "AsyncFnOnce", "drop", "size_of", "size_of_val", "align_of", "align_of_val",
    "Box", "ToOwned", "Clone", "PartialEq", "PartialOrd", "Eq", "Ord", "AsRef", "AsMut", "Into",
    "From", "Default", "Iterator", "Extend", "IntoIterator", "DoubleEndedIterator",
    "ExactSizeIterator", "Option", "Some", "None", "Result", "Ok", "Err", "String", "ToString",
    "Vec", "TryFrom", "TryInto", "FromIterator",
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
}

impl<'p> Resolver<'p> {
    /// A resolver for `program`.
    #[must_use]
    pub fn new(program: &'p Program) -> Resolver<'p> {
        let crates = program
            .crates
            .iter()
            .map(|c| (c.source.lib.clone(), c.root))
            .collect();
        Resolver {
            program,
            cache: RefCell::new(BTreeMap::new()),
            crates,
        }
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
        if ns == Ns::Type && PRIMITIVES.contains(&name) {
            return vec![Res::Prim(name.to_owned())];
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

    /// The type arguments of a path segment (`Vec<T>`: `T`; `Fn(A, B) -> C`: `A`, `B`).
    /// The type arguments written on one path segment, in `scope`.
    #[must_use]
    pub fn segment_args(&self, scope: &Scope, seg: &syn::PathSegment) -> Vec<Ty> {
        self.args(scope, seg, 0)
    }

    fn args(&self, scope: &Scope, seg: &syn::PathSegment, depth: usize) -> Vec<Ty> {
        match &seg.arguments {
            syn::PathArguments::AngleBracketed(a) => a
                .args
                .iter()
                .filter_map(|arg| match arg {
                    syn::GenericArgument::Type(t) => Some(self.ty_at(scope, t, depth)),
                    _ => None,
                })
                .collect(),
            syn::PathArguments::Parenthesized(p) => {
                p.inputs.iter().map(|t| self.ty_at(scope, &t.ty, depth)).collect()
            }
            syn::PathArguments::None => Vec::new(),
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
            return Ty::unknown();
        }
        match ty {
            syn::Type::Reference(r) => Ty::Ref(Box::new(self.ty_at(scope, &r.elem, depth))),
            syn::Type::Ptr(p) => Ty::Ref(Box::new(self.ty_at(scope, &p.elem, depth))),
            syn::Type::Slice(s) => Ty::Slice(Box::new(self.ty_at(scope, &s.elem, depth))),
            syn::Type::Array(a) => Ty::Array(Box::new(self.ty_at(scope, &a.elem, depth))),
            syn::Type::Tuple(t) => Ty::Tuple(t.elems.iter().map(|e| self.ty_at(scope, e, depth)).collect()),
            syn::Type::Paren(p) => self.ty_at(scope, &p.elem, depth),
            syn::Type::Group(g) => self.ty_at(scope, &g.elem, depth),
            syn::Type::Never(_) => Ty::Never,
            syn::Type::Infer(_) => Ty::Infer,
            syn::Type::FnPtr(f) => Ty::FnPtr(f.inputs.len()),
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
                    return self.assoc_ty(base, &name, depth);
                }
                let segs = Self::segments(&tp.path);
                let resolved = self.resolve(scope, &segs, tp.path.leading_colon.is_some(), Ns::Type);
                let last = tp.path.segments.last();
                let args = last.map(|s| self.args(scope, s, depth)).unwrap_or_default();
                let Some(res) = resolved.into_iter().next() else {
                    return Ty::unknown();
                };
                self.ty_of_res(scope, &res, args, depth)
            }
            _ => Ty::unknown(),
        }
    }

    /// The type a resolution names with type arguments `args`, in `scope`.
    #[must_use]
    pub fn res_ty(&self, scope: &Scope, res: &Res, args: Vec<Ty>) -> Ty {
        self.ty_of_res(scope, res, args, 0)
    }

    fn ty_of_res(&self, scope: &Scope, res: &Res, args: Vec<Ty>, depth: usize) -> Ty {
        match res {
            Res::Prim(p) => Ty::Prim(p.clone()),
            Res::Generic(g) => Ty::Param(*g),
            Res::SelfTy => self.self_ty(scope, depth),
            Res::Ext(path) => Ty::Ext(path.last().cloned().unwrap_or_default(), args),
            Res::Item(id) => {
                let item = self.program.item(*id);
                match &item.kind {
                    ItemKind::Struct { .. } | ItemKind::Union { .. } | ItemKind::Enum { .. } => {
                        let mut args = args;
                        // Missing arguments are left to inference.
                        while args.len() < item.scope.generics.len() {
                            args.push(Ty::Infer);
                        }
                        Ty::Adt(*id, args)
                    }
                    ItemKind::Alias { ty } => {
                        let inner = self.ty_at(&item.scope, ty, depth.saturating_add(1));
                        let map: BTreeMap<GenId, Ty> =
                            item.scope.generics.iter().copied().zip(args).collect();
                        inner.subst(&map)
                    }
                    ItemKind::Trait { .. } => Ty::Dyn(vec![TraitRef::Kernel(*id, args)]),
                    ItemKind::Const { .. } => Ty::unknown(),
                }
            }
            Res::Assoc(base, name) => {
                let base_ty = self.ty_of_res(scope, base, Vec::new(), depth);
                self.assoc_ty(base_ty, name, depth)
            }
            Res::Qualified(t, _) => (**t).clone(),
            Res::Mod(_)
            | Res::Variant(..)
            | Res::Fn(_)
            | Res::ConstParam(_)
            | Res::Macro(_) => Ty::unknown(),
        }
    }

    /// The type item `id` names when written alone: a struct, enum or union with unknown
    /// arguments, or what an alias expands to.
    #[must_use]
    pub fn item_ty(&self, id: ItemId) -> Ty {
        let item = self.program.item(id);
        self.ty_of_res(&item.scope, &Res::Item(id), Vec::new(), 0)
    }

    /// What `Self` is in `scope`.
    #[must_use]
    pub fn self_ty(&self, scope: &Scope, depth: usize) -> Ty {
        match scope.self_ref {
            SelfRef::None => Ty::unknown(),
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

    /// The associated type `name` of `base`: from the kernel's implementations for a kernel type,
    /// else left as a projection.
    #[must_use]
    pub fn assoc_ty(&self, base: Ty, name: &str, depth: usize) -> Ty {
        if let Ty::Adt(id, _) = base.peel() {
            for (i, imp) in self.program.impls.iter().enumerate() {
                let Some(t) = imp.assoc.get(name) else {
                    continue;
                };
                let mut inner = imp.scope.clone();
                inner.self_ref = SelfRef::Impl(crate::recursion::model::ImplId(i));
                let self_ty = {
                    let mut s = imp.scope.clone();
                    s.self_ref = SelfRef::None;
                    self.ty_at(&s, &imp.self_ty, depth.saturating_add(1))
                };
                if matches!(self_ty.peel(), Ty::Adt(x, _) if x == id) {
                    return self.ty_at(&inner, t, depth.saturating_add(1));
                }
            }
        }
        Ty::Assoc(Box::new(base), name.to_owned())
    }

    /// The traits of a list of bounds; lifetimes and unreadable bounds are left out.
    #[must_use]
    pub fn trait_refs<'b>(
        &self,
        scope: &Scope,
        bounds: impl Iterator<Item = &'b syn::TypeParamBound>,
    ) -> Vec<TraitRef> {
        bounds
            .filter_map(|b| match b {
                syn::TypeParamBound::Trait(t) => self.trait_ref(scope, &t.path),
                _ => None,
            })
            .collect()
    }

    /// The trait a path names, with its arguments.
    #[must_use]
    pub fn trait_ref(&self, scope: &Scope, path: &syn::Path) -> Option<TraitRef> {
        let segs = Self::segments(path);
        let args = path
            .segments
            .last()
            .map(|s| self.args(scope, s, 0))
            .unwrap_or_default();
        // A closure trait written with its parentheses: `Fn(A, B) -> R`.
        if let Some(last) = path.segments.last()
            && let syn::PathArguments::Parenthesized(p) = &last.arguments
        {
            let name = last.ident.to_string();
            if crate::recursion::ty::FN_TRAITS.contains(&name.as_str()) {
                let output = match &p.output {
                    syn::ReturnType::Type(_, t) => self.ty_at(scope, t, 0),
                    syn::ReturnType::Default => Ty::unit(),
                };
                return Some(TraitRef::Fn(name, args, Box::new(output)));
            }
        }
        let res = self
            .resolve(scope, &segs, path.leading_colon.is_some(), Ns::Type)
            .into_iter()
            .next();
        match res {
            Some(Res::Item(id))
                if matches!(self.program.item(id).kind, ItemKind::Trait { .. }) =>
            {
                Some(TraitRef::Kernel(id, args))
            }
            Some(Res::Ext(p)) => Some(TraitRef::Ext(p.last().cloned().unwrap_or_default(), args)),
            // A trait the resolver cannot find is named by its last segment, as an outside one.
            None => segs.last().map(|s| TraitRef::Ext(s.clone(), args)),
            _ => None,
        }
    }

    /// The traits a generic parameter is bounded by, with their supertraits, each once.
    #[must_use]
    pub fn bounds(&self, g: GenId) -> Vec<TraitRef> {
        let generic = self.program.generic(g);
        let direct = self.trait_refs(&generic.scope, generic.bounds.iter());
        self.with_supertraits(direct)
    }

    /// `traits` and every kernel supertrait they lead to, each once, in a fixed order.
    #[must_use]
    pub fn with_supertraits(&self, traits: Vec<TraitRef>) -> Vec<TraitRef> {
        let mut out: Vec<TraitRef> = Vec::new();
        let mut work = traits;
        work.reverse();
        let mut seen: BTreeSet<TraitRef> = BTreeSet::new();
        while let Some(t) = work.pop() {
            if !seen.insert(t.clone()) {
                continue;
            }
            if let TraitRef::Kernel(id, _) = &t
                && let ItemKind::Trait { supertraits, .. } = &self.program.item(*id).kind
            {
                let item_scope = self.program.item(*id).scope.clone();
                let mut supers = self.trait_refs(&item_scope, supertraits.iter());
                supers.reverse();
                work.extend(supers);
            }
            out.push(t);
        }
        out
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
            .filter(|lib| files.iter().any(|(p, _)| p.starts_with(&format!("c/{lib}/"))))
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
            ("c/b/src/lib.rs", "use a::n::g; pub enum E { V } use E::*; fn caller() {}"),
        ]);
        let r = Resolver::new(&p);
        let (_, user) = fn_named(&p, "a::user");
        let f = fn_named(&p, "a::m::f").0;
        assert_eq!(r.resolve(&user.scope, &segs("renamed"), false, Ns::Value), [Res::Fn(f)]);
        assert_eq!(r.resolve(&user.scope, &segs("n::f"), false, Ns::Value), [Res::Fn(f)]);
        let gs = r.resolve(&user.scope, &segs("m::g"), false, Ns::Value);
        assert_eq!(gs.len(), 2, "both cfg alternatives: {gs:?}");
        assert!(r.resolve(&user.scope, &segs("cyc::nothing"), false, Ns::Value).is_empty());
        let (_, caller) = fn_named(&p, "b::caller");
        assert_eq!(r.resolve(&caller.scope, &segs("g"), false, Ns::Value).len(), 2);
        assert!(matches!(
            r.resolve(&caller.scope, &segs("V"), false, Ns::Value).as_slice(),
            [Res::Variant(_, 0)]
        ));
        assert_eq!(
            r.resolve(&caller.scope, &segs("core::cmp::max"), false, Ns::Value),
            [Res::Ext(segs("core::cmp::max"))]
        );
        assert_eq!(
            r.resolve(&caller.scope, &segs("Some"), false, Ns::Value),
            [Res::Ext(segs("Some"))]
        );
        assert!(r.resolve(&caller.scope, &segs("nowhere"), false, Ns::Value).is_empty());
    }

    #[test]
    fn types_expand_aliases_self_and_generics() {
        let p = program(&[(
            "c/a/src/lib.rs",
            "pub struct W<T> { t: T } pub type Wu = W<u64>;\n\
             impl<T: Clone> W<T> { fn get(&self) -> &T { &self.t } }\n\
             fn f(x: Wu, y: &[W<bool>], z: impl Fn(u8) -> u8) {}",
        )]);
        let r = Resolver::new(&p);
        let (_, f) = fn_named(&p, "a::f");
        let sig = f.sig.as_ref().unwrap();
        let tys: Vec<Ty> = sig
            .inputs
            .iter()
            .map(|i| match i {
                syn::FnArg::Typed(t) => r.ty(&f.scope, &t.ty),
                syn::FnArg::Receiver(_) => Ty::unknown(),
            })
            .collect();
        let w = ItemId(0);
        assert_eq!(tys[0], Ty::Adt(w, vec![Ty::Prim("u64".to_owned())]));
        assert_eq!(
            tys[1],
            Ty::Ref(Box::new(Ty::Slice(Box::new(Ty::Adt(w, vec![Ty::Prim("bool".to_owned())])))))
        );
        assert!(matches!(&tys[2], Ty::Dyn(t) if t[0].fn_arity() == Some(1)));
        let get = p.nodes.iter().find(|n| n.ident == "get").unwrap();
        assert!(matches!(r.self_ty(&get.scope, 0), Ty::Adt(id, ref a) if id == w && matches!(a[0], Ty::Param(_))));
    }
}
