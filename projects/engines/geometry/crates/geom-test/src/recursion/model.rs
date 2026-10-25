//! The program the recursion check reads: every crate that ships, parsed by `syn`, as modules,
//! items, implementations, generic parameters and function bodies.
//!
//! What it does: [`load`] parses each crate from its root file and follows its module tree as the
//! compiler does: `mod name;` from `name.rs` or `name/mod.rs` beside its parent (or from a
//! `#[path]`), inline `mod name { ... }`, and the items of function bodies, which get a scope of
//! their own. It records every item that ships ([`crate::recursion::cfg`]): functions, methods of
//! inherent and trait implementations, trait methods with default bodies, structs, enums, unions,
//! traits, type aliases, constants, statics, `use` declarations and `macro_rules!` macros, and
//! expands every macro invocation in item position ([`crate::recursion::macros`]). Each function
//! body, and each implementation a `#[derive]` writes, is a [`Node`] of the call graph.
//!
//! Guarantees:
//!
//! - **The compiler's module tree.** A file is read only when a `mod` declaration that ships
//!   reaches it, by the reference's rules for `mod` files and `#[path]`, so code the compiler does
//!   not build is not analysed and code it builds is.
//! - **Nothing unread passes silently.** A module file that cannot be found or parsed, an item
//!   form the reader does not know, and a macro in item position that cannot be expanded are each
//!   a [`Problem`]; the check fails on any problem.
//! - **Deterministic.** Crates, modules and items are numbered in the order the files declare
//!   them, so node ids and every listing are the same on every run.
//!
//! Method: The Rust Reference on modules and the `path` attribute
//! (<https://doc.rust-lang.org/reference/items/modules.html>), on items and associated items
//! (<https://doc.rust-lang.org/reference/items.html>), and on derive
//! (<https://doc.rust-lang.org/reference/attributes/derive.html>); parsing by `syn`
//! (<https://docs.rs/syn/2.0.119/syn/>).
//!
//! Cost: one parse of every file the module tree reaches; linear in the number of items; no
//! recursion (modules and function bodies are queued).

use std::collections::{BTreeMap, BTreeSet};

use syn::spanned::Spanned;
use syn::visit::Visit;

use crate::recursion::Problem;
use crate::recursion::cfg;
use crate::recursion::ty::{GenId, ItemId, NodeId};

/// A module, or the scope of a function body's items.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModId(pub usize);

/// An `impl` block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ImplId(pub usize);

/// A `macro_rules!` macro.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MacroId(pub usize);

/// What a crate is to the check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// A crate that ships: its cycles are listed and must be bounded.
    Ships,
    /// A crate's bound tests (`tests/recursion.rs`), read to check that each test reaches its
    /// cycle; its own cycles are not listed.
    BoundTests,
    /// The model of the standard library ([`crate::recursion::stdlib`]): read for what it calls
    /// back; its own cycles are not listed.
    Model,
}

/// One crate to read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrateSource {
    /// The package name, `geom-num`.
    pub package: String,
    /// The name its code is referred to by, `geom_num`.
    pub lib: String,
    /// The crate's folder, relative to the workspace root.
    pub folder: String,
    /// The root file, relative to the workspace root.
    pub root: String,
    /// What it is to the check.
    pub role: Role,
}

/// A crate read.
#[derive(Clone, Debug)]
pub struct Crate {
    /// Where it came from.
    pub source: CrateSource,
    /// Its root module.
    pub root: ModId,
}

/// A name a module defines or imports, in one namespace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Def {
    /// A module.
    Mod(ModId),
    /// A struct, enum, union, trait, alias, constant or static.
    Item(ItemId),
    /// A free function.
    Fn(NodeId),
    /// An enum variant brought in by `use Enum::Variant` or a glob of an enum.
    Variant(ItemId, usize),
}

/// One `use` binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Use {
    /// The path imported.
    pub segments: Vec<String>,
    /// Whether the path starts with `::`.
    pub leading_colon: bool,
    /// The name it binds, or `None` for a glob (`use path::*`).
    pub name: Option<String>,
}

/// A module or block scope.
#[derive(Clone, Debug)]
pub struct Module {
    /// The crate it belongs to.
    pub krate: usize,
    /// The enclosing module.
    pub parent: Option<ModId>,
    /// Its display path, `geom_num::math::log`.
    pub path: String,
    /// The source file, relative to the workspace root.
    pub file: String,
    /// Where the files of `mod name;` declarations inside it are looked for.
    pub children_dir: String,
    /// What a `#[path]` inside it is relative to.
    pub path_base: String,
    /// For the scope of a function body's items: that function.
    pub block_of: Option<NodeId>,
    /// The type namespace.
    pub types: BTreeMap<String, Vec<Def>>,
    /// The value namespace.
    pub values: BTreeMap<String, Vec<Def>>,
    /// `macro_rules!` macros defined here.
    pub macros: BTreeMap<String, Vec<MacroId>>,
    /// `use` declarations.
    pub uses: Vec<Use>,
}

/// The fields of a struct, union or variant.
#[derive(Clone, Debug)]
pub enum Fields {
    /// Named fields.
    Named(Vec<(String, syn::Type)>),
    /// Positional fields.
    Tuple(Vec<syn::Type>),
    /// No fields.
    Unit,
}

impl Fields {
    fn of(fields: &syn::Fields) -> Fields {
        match fields {
            syn::Fields::Named(named) => Fields::Named(
                named
                    .named
                    .iter()
                    .filter(|f| cfg::ships(&f.attrs))
                    .map(|f| {
                        (
                            f.ident
                                .as_ref()
                                .map(ToString::to_string)
                                .unwrap_or_default(),
                            f.ty.clone(),
                        )
                    })
                    .collect(),
            ),
            syn::Fields::Unnamed(unnamed) => Fields::Tuple(
                unnamed
                    .unnamed
                    .iter()
                    .filter(|f| cfg::ships(&f.attrs))
                    .map(|f| f.ty.clone())
                    .collect(),
            ),
            syn::Fields::Unit => Fields::Unit,
        }
    }

    /// Every field's name (`0`, `1`, ... for positional fields) and type.
    #[must_use]
    pub fn all(&self) -> Vec<(String, &syn::Type)> {
        match self {
            Fields::Named(f) => f.iter().map(|(n, t)| (n.clone(), t)).collect(),
            Fields::Tuple(f) => f
                .iter()
                .enumerate()
                .map(|(i, t)| (i.to_string(), t))
                .collect(),
            Fields::Unit => Vec::new(),
        }
    }

    /// The type of the field `name`.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&syn::Type> {
        match self {
            Fields::Named(f) => f.iter().find(|(n, _)| n == name).map(|(_, t)| t),
            Fields::Tuple(f) => name.parse::<usize>().ok().and_then(|i| f.get(i)),
            Fields::Unit => None,
        }
    }
}

/// An enum variant.
#[derive(Clone, Debug)]
pub struct Variant {
    /// Its name.
    pub name: String,
    /// Its fields.
    pub fields: Fields,
}

/// What an item is.
#[derive(Clone, Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "a trait carries its signatures; items are few and read once"
)]
pub enum ItemKind {
    /// A struct.
    Struct {
        /// Its fields.
        fields: Fields,
        /// The traits it derives.
        derives: Vec<String>,
    },
    /// A union.
    Union {
        /// Its fields.
        fields: Fields,
        /// The traits it derives.
        derives: Vec<String>,
    },
    /// An enum.
    Enum {
        /// Its variants.
        variants: Vec<Variant>,
        /// The traits it derives.
        derives: Vec<String>,
    },
    /// A trait (or trait alias).
    Trait {
        /// The generic parameter standing for `Self` in its default bodies.
        self_gen: GenId,
        /// Its supertraits.
        supertraits: Vec<syn::TypeParamBound>,
        /// Its methods with default bodies.
        defaults: BTreeMap<String, NodeId>,
        /// Its associated types, with their bounds.
        assoc_types: BTreeMap<String, Vec<syn::TypeParamBound>>,
        /// Its associated constants, with their types.
        consts: BTreeMap<String, syn::Type>,
        /// Every method it declares.
        methods: BTreeSet<String>,
        /// Every method's signature, with the scope its types are read in (the trait's, with the
        /// method's own generic parameters).
        sigs: BTreeMap<String, (syn::Signature, Scope)>,
    },
    /// A type alias.
    Alias {
        /// The type it names.
        ty: syn::Type,
    },
    /// A constant or static.
    Const {
        /// Its type.
        ty: syn::Type,
        /// Its value.
        init: Option<syn::Expr>,
    },
}

/// A named item.
#[derive(Clone, Debug)]
pub struct Item {
    /// Its name.
    pub name: String,
    /// The module that defines it.
    pub module: ModId,
    /// Its display path.
    pub path: String,
    /// What it is.
    pub kind: ItemKind,
    /// Its scope: the module and its own generic parameters.
    pub scope: Scope,
    /// The line it starts on.
    pub line: usize,
}

impl Item {
    /// The traits it derives, if it is a struct, enum or union.
    #[must_use]
    pub fn derives(&self) -> &[String] {
        match &self.kind {
            ItemKind::Struct { derives, .. }
            | ItemKind::Union { derives, .. }
            | ItemKind::Enum { derives, .. } => derives,
            _ => &[],
        }
    }

    /// Whether it is a struct, enum or union.
    #[must_use]
    pub fn is_adt(&self) -> bool {
        matches!(
            self.kind,
            ItemKind::Struct { .. } | ItemKind::Union { .. } | ItemKind::Enum { .. }
        )
    }
}

/// An `impl` block.
#[derive(Clone, Debug)]
pub struct Impl {
    /// Its scope: module, generic parameters and `Self`.
    pub scope: Scope,
    /// The type it implements for.
    pub self_ty: syn::Type,
    /// The trait, for a trait implementation.
    pub trait_path: Option<syn::Path>,
    /// Its methods by name.
    pub methods: BTreeMap<String, Vec<NodeId>>,
    /// Its associated types.
    pub assoc: BTreeMap<String, syn::Type>,
    /// Its associated constants, with their types.
    pub consts: BTreeMap<String, syn::Type>,
    /// The file and line it starts on.
    pub file: String,
    /// The line.
    pub line: usize,
}

/// What `Self` names in a scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelfRef {
    /// Nothing.
    None,
    /// The self type of an implementation.
    Impl(ImplId),
    /// The implementing type of a trait, in its default bodies: a generic parameter bounded by it.
    Trait(ItemId, GenId),
    /// A struct, enum or union, in its own definition.
    Adt(ItemId),
}

/// Where names are looked up.
#[derive(Clone, Debug)]
pub struct Scope {
    /// The innermost module or block scope.
    pub module: ModId,
    /// Generic type parameters in scope.
    pub generics: Vec<GenId>,
    /// Const generic parameters in scope.
    pub consts: Vec<String>,
    /// What `Self` names.
    pub self_ref: SelfRef,
}

/// A generic type parameter.
#[derive(Clone, Debug)]
pub struct Generic {
    /// Its name.
    pub name: String,
    /// Its bounds, inline and from `where` clauses.
    pub bounds: Vec<syn::TypeParamBound>,
    /// Its default, `Rhs = Self`.
    pub default: Option<syn::Type>,
    /// The scope its bounds are read in.
    pub scope: Scope,
}

/// What a call-graph node is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// A free function.
    Fn,
    /// A method or associated function of an implementation.
    Method(ImplId),
    /// A trait method's default body.
    TraitDefault(ItemId),
    /// The implementation of one trait that `#[derive]` writes for an item.
    Derived {
        /// The struct, enum or union.
        item: ItemId,
        /// The trait.
        trait_name: String,
    },
}

/// A function body or derived implementation: a node of the call graph.
#[derive(Clone, Debug)]
pub struct Node {
    /// What it is.
    pub kind: NodeKind,
    /// Its name, as listed: completed when the program is indexed.
    pub name: String,
    /// Its own name: the function's or method's.
    pub ident: String,
    /// Where names in it are looked up.
    pub scope: Scope,
    /// Its file, relative to the workspace root.
    pub file: String,
    /// Its first line.
    pub line: usize,
    /// Its signature, for a function.
    pub sig: Option<syn::Signature>,
    /// Its body, for a function.
    pub body: Option<syn::Block>,
    /// The crate it belongs to.
    pub krate: usize,
}

/// A `macro_rules!` macro.
#[derive(Clone, Debug)]
pub struct MacroDef {
    /// Its name.
    pub name: String,
    /// The crate that defines it, for `$crate`.
    pub krate: usize,
    /// Its rules' tokens.
    pub body: proc_macro2::TokenStream,
}

/// The traits `#[derive]` can write, with the methods each provides; the first is the one the
/// derived implementation's node is named by.
pub const DERIVABLE: [(&str, &[&str]); 7] = [
    ("Clone", &["clone", "clone_from"]),
    ("PartialEq", &["eq", "ne"]),
    ("PartialOrd", &["partial_cmp", "lt", "le", "gt", "ge"]),
    ("Ord", &["cmp", "max", "min", "clamp"]),
    ("Hash", &["hash", "hash_slice"]),
    ("Debug", &["fmt"]),
    ("Default", &["default"]),
];

/// The program: every crate read, and what was found in it.
#[derive(Clone, Debug, Default)]
pub struct Program {
    /// The crates, in the order given.
    pub crates: Vec<Crate>,
    /// Every module and block scope.
    pub modules: Vec<Module>,
    /// Every item.
    pub items: Vec<Item>,
    /// Every implementation.
    pub impls: Vec<Impl>,
    /// Every node.
    pub nodes: Vec<Node>,
    /// Every generic parameter.
    pub generics: Vec<Generic>,
    /// Every `macro_rules!` macro.
    pub macros: Vec<MacroDef>,
    /// The text of every file read, by path.
    pub files: BTreeMap<String, String>,
    /// What could not be read.
    pub problems: Vec<Problem>,
}

impl Program {
    /// The module `id`.
    ///
    /// # Panics
    ///
    /// Never for an id this program made.
    #[must_use]
    pub fn module(&self, id: ModId) -> &Module {
        self.modules
            .get(id.0)
            .unwrap_or_else(|| unreachable_id("module", id.0))
    }

    /// The item `id`.
    #[must_use]
    pub fn item(&self, id: ItemId) -> &Item {
        self.items
            .get(id.0)
            .unwrap_or_else(|| unreachable_id("item", id.0))
    }

    /// The implementation `id`.
    #[must_use]
    pub fn imp(&self, id: ImplId) -> &Impl {
        self.impls
            .get(id.0)
            .unwrap_or_else(|| unreachable_id("impl", id.0))
    }

    /// The node `id`, if the program made it (carriers and closures belong to the graph).
    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.0)
    }

    /// The generic parameter `id`.
    #[must_use]
    pub fn generic(&self, id: GenId) -> &Generic {
        self.generics
            .get(id.0)
            .unwrap_or_else(|| unreachable_id("generic", id.0))
    }

    /// The nearest module that is not a block scope, from `id` outwards.
    #[must_use]
    pub fn non_block(&self, id: ModId) -> ModId {
        let mut m = id;
        while self.module(m).block_of.is_some() {
            match self.module(m).parent {
                Some(p) => m = p,
                None => break,
            }
        }
        m
    }
}

/// Ids are made by the program and index its tables; one out of range is a bug in this module.
#[allow(
    clippy::panic,
    reason = "an id out of range is a bug in the loader, not an input"
)]
fn unreachable_id(what: &str, id: usize) -> ! {
    panic!("recursion check: {what} id {id} out of range")
}

/// The line a syntax node starts on.
pub fn line_of<T: Spanned>(node: &T) -> usize {
    node.span().start().line
}

/// A path with `.` and `..` segments folded, `/`-separated.
#[must_use]
pub fn normalise(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            p => out.push(p),
        }
    }
    out.join("/")
}

/// The folder of a `/`-separated path.
fn dir_of(path: &str) -> String {
    path.rsplit_once('/')
        .map_or(String::new(), |(d, _)| d.to_owned())
}

/// `a/b` joined, either side possibly empty.
fn join(a: &str, b: &str) -> String {
    if a.is_empty() {
        b.to_owned()
    } else if b.is_empty() {
        a.to_owned()
    } else {
        format!("{a}/{b}")
    }
}

/// The file reader the loader uses: a repository path to its text, or why it cannot be read.
pub type Reader<'a> = &'a dyn Fn(&str) -> Result<String, String>;

/// Work for the loader.
enum Job {
    /// Add these items to this module.
    Items(ModId, Vec<syn::Item>),
}

/// The loader's state.
struct Loader<'a> {
    program: Program,
    read: Reader<'a>,
    jobs: Vec<Job>,
    /// Item-position macro invocations, expanded once every `macro_rules!` is known.
    pending_macros: Vec<(ModId, syn::Macro)>,
}

/// Reads every crate of `sources` with `read`.
#[must_use]
pub fn load(sources: &[CrateSource], read: Reader<'_>) -> Program {
    let mut loader = Loader {
        program: Program::default(),
        read,
        jobs: Vec::new(),
        pending_macros: Vec::new(),
    };
    for (krate, source) in sources.iter().enumerate() {
        loader.load_crate(krate, source);
    }
    loader.run();
    // Macros in item position may expand into more items and more macros.
    let mut rounds = 0_usize;
    while !loader.pending_macros.is_empty() {
        rounds = rounds.saturating_add(1);
        if rounds > crate::recursion::macros::EXPANSION_LIMIT {
            let left = core::mem::take(&mut loader.pending_macros);
            for (module, mac) in left {
                let file = loader.program.module(module).file.clone();
                loader.problem(
                    &file,
                    line_of(&mac),
                    "a macro in item position still expands after the expansion limit",
                );
            }
            break;
        }
        let pending = core::mem::take(&mut loader.pending_macros);
        for (module, mac) in pending {
            loader.expand_item_macro(module, &mac);
        }
        loader.run();
    }
    loader.program
}

impl Loader<'_> {
    fn problem(&mut self, file: &str, line: usize, what: &str) {
        self.program
            .problems
            .push(Problem::unread(file, line, what));
    }

    fn load_crate(&mut self, krate: usize, source: &CrateSource) {
        let root_file = source.root.clone();
        let dir = dir_of(&root_file);
        let id = self.new_module(Module {
            krate,
            parent: None,
            path: source.lib.clone(),
            file: root_file.clone(),
            children_dir: dir.clone(),
            path_base: dir,
            block_of: None,
            types: BTreeMap::new(),
            values: BTreeMap::new(),
            macros: BTreeMap::new(),
            uses: Vec::new(),
        });
        self.program.crates.push(Crate {
            source: source.clone(),
            root: id,
        });
        if let Some(file) = self.parse(&root_file, 0) {
            self.jobs.push(Job::Items(id, file.items));
        }
    }

    fn new_module(&mut self, module: Module) -> ModId {
        let id = ModId(self.program.modules.len());
        self.program.modules.push(module);
        id
    }

    fn module_mut(&mut self, id: ModId) -> Option<&mut Module> {
        self.program.modules.get_mut(id.0)
    }

    /// Reads and parses a file, recording its text; a problem if it cannot be.
    fn parse(&mut self, path: &str, line: usize) -> Option<syn::File> {
        let text = match (self.read)(path) {
            Ok(text) => text,
            Err(why) => {
                self.problem(
                    path,
                    line,
                    &format!("the module file cannot be read: {why}"),
                );
                return None;
            }
        };
        let parsed = syn::parse_file(&text);
        self.program.files.insert(path.to_owned(), text);
        match parsed {
            Ok(file) => Some(file),
            Err(e) => {
                let at = e.span().start().line;
                self.problem(path, at, &format!("the file does not parse as Rust: {e}"));
                None
            }
        }
    }

    fn run(&mut self) {
        while let Some(job) = self.jobs.pop() {
            match job {
                Job::Items(module, items) => {
                    for item in items {
                        self.add_item(module, item);
                    }
                }
            }
        }
    }

    fn define(&mut self, module: ModId, name: &str, def: Def, value: bool, ty: bool) {
        if let Some(m) = self.module_mut(module) {
            if ty {
                m.types.entry(name.to_owned()).or_default().push(def);
            }
            if value {
                m.values.entry(name.to_owned()).or_default().push(def);
            }
        }
    }

    fn new_item(&mut self, item: Item) -> ItemId {
        let id = ItemId(self.program.items.len());
        self.program.items.push(item);
        id
    }

    fn new_node(&mut self, node: Node) -> NodeId {
        let id = NodeId(self.program.nodes.len());
        self.program.nodes.push(node);
        id
    }

    /// Declares the generic type parameters of `generics` in `scope` (whose generics they join),
    /// with their inline bounds and those of `where` clauses that bound them alone.
    fn declare_generics(&mut self, scope: &mut Scope, generics: &syn::Generics) {
        let mut ids: Vec<(String, GenId)> = Vec::new();
        for param in &generics.params {
            match param {
                syn::GenericParam::Type(t) => {
                    let id = GenId(self.program.generics.len());
                    self.program.generics.push(Generic {
                        name: t.ident.to_string(),
                        bounds: t.bounds.iter().cloned().collect(),
                        default: t.default.clone(),
                        scope: scope.clone(),
                    });
                    scope.generics.push(id);
                    ids.push((t.ident.to_string(), id));
                }
                syn::GenericParam::Const(c) => scope.consts.push(c.ident.to_string()),
                syn::GenericParam::Lifetime(_) => {}
            }
        }
        if let Some(clause) = &generics.where_clause {
            for predicate in &clause.predicates {
                let syn::WherePredicate::Type(pt) = predicate else {
                    continue;
                };
                let syn::Type::Path(tp) = &pt.bounded_ty else {
                    continue;
                };
                if tp.qself.is_some() || tp.path.segments.len() != 1 {
                    continue;
                }
                let Some(seg) = tp.path.segments.first() else {
                    continue;
                };
                let name = seg.ident.to_string();
                if let Some((_, id)) = ids.iter().find(|(n, _)| *n == name)
                    && let Some(g) = self.program.generics.get_mut(id.0)
                {
                    g.bounds.extend(pt.bounds.iter().cloned());
                }
            }
        }
        // Each parameter's scope sees every parameter of its list.
        for (_, id) in &ids {
            if let Some(g) = self.program.generics.get_mut(id.0) {
                g.scope = scope.clone();
            }
        }
    }

    fn base_scope(module: ModId) -> Scope {
        Scope {
            module,
            generics: Vec::new(),
            consts: Vec::new(),
            self_ref: SelfRef::None,
        }
    }

    fn path_of(&self, module: ModId, name: &str) -> String {
        format!("{}::{name}", self.program.module(module).path)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one arm per item form, each short; splitting would scatter the item grammar"
    )]
    fn add_item(&mut self, module: ModId, item: syn::Item) {
        let file = self.program.module(module).file.clone();
        let line = line_of(&item);
        let attrs: &[syn::Attribute] = match &item {
            syn::Item::Const(i) => &i.attrs,
            syn::Item::Enum(i) => &i.attrs,
            syn::Item::ExternCrate(i) => &i.attrs,
            syn::Item::Fn(i) => &i.attrs,
            syn::Item::ForeignMod(i) => &i.attrs,
            syn::Item::Impl(i) => &i.attrs,
            syn::Item::Macro(i) => &i.attrs,
            syn::Item::Mod(i) => &i.attrs,
            syn::Item::Static(i) => &i.attrs,
            syn::Item::Struct(i) => &i.attrs,
            syn::Item::Trait(i) => &i.attrs,
            syn::Item::TraitAlias(i) => &i.attrs,
            syn::Item::Type(i) => &i.attrs,
            syn::Item::Union(i) => &i.attrs,
            syn::Item::Use(i) => &i.attrs,
            _ => &[],
        };
        if !cfg::ships(attrs) {
            return;
        }
        match item {
            syn::Item::Fn(mut f) => {
                lift_impl_params(&mut f.sig);
                let name = f.sig.ident.to_string();
                let mut scope = Self::base_scope(module);
                self.declare_generics(&mut scope, &f.sig.generics);
                let node = self.new_node(Node {
                    kind: NodeKind::Fn,
                    name: self.path_of(module, &name),
                    ident: name.clone(),
                    scope,
                    file,
                    line,
                    sig: Some(f.sig.clone()),
                    body: Some((*f.block).clone()),
                    krate: self.program.module(module).krate,
                });
                self.define(module, &name, Def::Fn(node), true, false);
                self.body_items(node, &f.block);
            }
            syn::Item::Struct(s) => {
                let name = s.ident.to_string();
                let derives = cfg::derives(&s.attrs);
                let unit_or_tuple = !matches!(s.fields, syn::Fields::Named(_));
                let id = self.adt(module, &name, &s.generics, line, |_| ItemKind::Struct {
                    fields: Fields::of(&s.fields),
                    derives,
                });
                self.define(module, &name, Def::Item(id), unit_or_tuple, true);
            }
            syn::Item::Union(u) => {
                let name = u.ident.to_string();
                let derives = cfg::derives(&u.attrs);
                let named = syn::Fields::Named(u.fields.clone());
                let id = self.adt(module, &name, &u.generics, line, |_| ItemKind::Union {
                    fields: Fields::of(&named),
                    derives,
                });
                self.define(module, &name, Def::Item(id), false, true);
            }
            syn::Item::Enum(e) => {
                let name = e.ident.to_string();
                let derives = cfg::derives(&e.attrs);
                let variants = e
                    .variants
                    .iter()
                    .filter(|v| cfg::ships(&v.attrs))
                    .map(|v| Variant {
                        name: v.ident.to_string(),
                        fields: Fields::of(&v.fields),
                    })
                    .collect();
                let id = self.adt(module, &name, &e.generics, line, |_| ItemKind::Enum {
                    variants,
                    derives,
                });
                self.define(module, &name, Def::Item(id), false, true);
            }
            syn::Item::Trait(t) => self.add_trait(module, &t, &file, line),
            syn::Item::TraitAlias(t) => {
                let name = t.ident.to_string();
                let mut scope = Self::base_scope(module);
                let self_gen = GenId(self.program.generics.len());
                self.program.generics.push(Generic {
                    name: "Self".to_owned(),
                    bounds: Vec::new(),
                    default: None,
                    scope: scope.clone(),
                });
                self.declare_generics(&mut scope, &t.generics);
                let id = self.new_item(Item {
                    name: name.clone(),
                    module,
                    path: self.path_of(module, &name),
                    kind: ItemKind::Trait {
                        self_gen,
                        supertraits: t.bounds.iter().cloned().collect(),
                        defaults: BTreeMap::new(),
                        assoc_types: BTreeMap::new(),
                        consts: BTreeMap::new(),
                        methods: BTreeSet::new(),
                        sigs: BTreeMap::new(),
                    },
                    scope,
                    line,
                });
                self.define(module, &name, Def::Item(id), false, true);
            }
            syn::Item::Type(t) => {
                let name = t.ident.to_string();
                let mut scope = Self::base_scope(module);
                self.declare_generics(&mut scope, &t.generics);
                let id = self.new_item(Item {
                    name: name.clone(),
                    module,
                    path: self.path_of(module, &name),
                    kind: ItemKind::Alias {
                        ty: (*t.ty).clone(),
                    },
                    scope,
                    line,
                });
                self.define(module, &name, Def::Item(id), false, true);
            }
            syn::Item::Const(c) => {
                let name = c.ident.to_string();
                let mut scope = Self::base_scope(module);
                self.declare_generics(&mut scope, &c.generics);
                let id = self.new_item(Item {
                    name: name.clone(),
                    module,
                    path: self.path_of(module, &name),
                    kind: ItemKind::Const {
                        ty: (*c.ty).clone(),
                        init: Some((*c.expr).clone()),
                    },
                    scope,
                    line,
                });
                self.define(module, &name, Def::Item(id), true, false);
            }
            syn::Item::Static(s) => {
                let name = s.ident.to_string();
                let id = self.new_item(Item {
                    name: name.clone(),
                    module,
                    path: self.path_of(module, &name),
                    kind: ItemKind::Const {
                        ty: (*s.ty).clone(),
                        init: Some((*s.expr).clone()),
                    },
                    scope: Self::base_scope(module),
                    line,
                });
                self.define(module, &name, Def::Item(id), true, false);
            }
            syn::Item::Mod(m) => self.add_mod(module, &m, &file, line),
            syn::Item::Use(u) => {
                let mut uses = Vec::new();
                flatten_use(&u.tree, Vec::new(), u.leading_colon.is_some(), &mut uses);
                if let Some(m) = self.module_mut(module) {
                    m.uses.extend(uses);
                }
            }
            syn::Item::Impl(i) => self.add_impl(module, &i, &file, line),
            syn::Item::Macro(m) => {
                if m.mac.path.is_ident("macro_rules") {
                    let Some(ident) = &m.ident else {
                        self.problem(&file, line, "a macro_rules! without a name");
                        return;
                    };
                    let name = ident.to_string();
                    let id = MacroId(self.program.macros.len());
                    self.program.macros.push(MacroDef {
                        name: name.clone(),
                        krate: self.program.module(module).krate,
                        body: m.mac.tokens.clone(),
                    });
                    let target = if cfg::macro_export(&m.attrs) {
                        let krate = self.program.module(module).krate;
                        self.program.crates.get(krate).map_or(module, |c| c.root)
                    } else {
                        module
                    };
                    if let Some(md) = self.module_mut(target) {
                        md.macros.entry(name.clone()).or_default().push(id);
                    }
                    if target != module
                        && let Some(md) = self.module_mut(module)
                    {
                        md.macros.entry(name).or_default().push(id);
                    }
                } else {
                    self.pending_macros.push((module, m.mac));
                }
            }
            syn::Item::ExternCrate(e) => {
                // `extern crate alloc;` and its kind name a crate the resolver knows by name; a
                // rename binds the new name to it.
                if let Some((_, rename)) = &e.rename {
                    if let Some(m) = self.module_mut(module) {
                        m.uses.push(Use {
                            segments: vec![e.ident.to_string()],
                            leading_colon: true,
                            name: Some(rename.to_string()),
                        });
                    }
                }
            }
            syn::Item::ForeignMod(_) => {
                // Foreign functions can only be called in `unsafe` code, which every crate that
                // ships forbids; they have no body to read.
            }
            other => {
                let what = format!(
                    "an item the recursion check cannot read: `{}`",
                    crate::recursion::macros::excerpt(&quote::ToTokens::to_token_stream(&other))
                );
                self.problem(&file, line, &what);
            }
        }
    }

    fn adt(
        &mut self,
        module: ModId,
        name: &str,
        generics: &syn::Generics,
        line: usize,
        kind: impl FnOnce(&Self) -> ItemKind,
    ) -> ItemId {
        let mut scope = Self::base_scope(module);
        self.declare_generics(&mut scope, generics);
        let kind = kind(self);
        let id = self.new_item(Item {
            name: name.to_owned(),
            module,
            path: self.path_of(module, name),
            kind,
            scope: scope.clone(),
            line,
        });
        // `Self` in the definition names the type itself.
        scope.self_ref = SelfRef::Adt(id);
        if let Some(item) = self.program.items.get_mut(id.0) {
            item.scope = scope;
        }
        let derived: Vec<String> = self
            .program
            .item(id)
            .derives()
            .iter()
            .filter(|d| DERIVABLE.iter().any(|(t, _)| t == d))
            .cloned()
            .collect();
        for trait_name in derived {
            let file = self.program.module(module).file.clone();
            let item_scope = self.program.item(id).scope.clone();
            self.new_node(Node {
                kind: NodeKind::Derived {
                    item: id,
                    trait_name: trait_name.clone(),
                },
                name: String::new(),
                ident: DERIVABLE
                    .iter()
                    .find(|(t, _)| *t == trait_name)
                    .and_then(|(_, m)| m.first())
                    .map_or_else(String::new, |m| (*m).to_owned()),
                scope: item_scope,
                file,
                line,
                sig: None,
                body: None,
                krate: self.program.module(module).krate,
            });
        }
        id
    }

    #[allow(
        clippy::too_many_lines,
        reason = "a trait's items in their written order; each is short"
    )]
    fn add_trait(&mut self, module: ModId, t: &syn::ItemTrait, file: &str, line: usize) {
        let name = t.ident.to_string();
        let mut scope = Self::base_scope(module);
        let self_gen = GenId(self.program.generics.len());
        self.program.generics.push(Generic {
            name: "Self".to_owned(),
            bounds: Vec::new(),
            default: None,
            scope: scope.clone(),
        });
        self.declare_generics(&mut scope, &t.generics);
        let id = self.new_item(Item {
            name: name.clone(),
            module,
            path: self.path_of(module, &name),
            kind: ItemKind::Trait {
                self_gen,
                supertraits: t.supertraits.iter().cloned().collect(),
                defaults: BTreeMap::new(),
                assoc_types: BTreeMap::new(),
                consts: BTreeMap::new(),
                methods: BTreeSet::new(),
                sigs: BTreeMap::new(),
            },
            scope: scope.clone(),
            line,
        });
        self.define(module, &name, Def::Item(id), false, true);
        scope.self_ref = SelfRef::Trait(id, self_gen);
        // The trait's generic parameters see `Self` too (`trait Add<Rhs = Self>`).
        for g in &scope.generics {
            if let Some(generic) = self.program.generics.get_mut(g.0) {
                generic.scope.self_ref = SelfRef::Trait(id, self_gen);
            }
        }
        // `Self` in default bodies is bounded by the trait itself.
        let own = syn::parse2::<syn::TypeParamBound>(quote::ToTokens::to_token_stream(&t.ident));
        if let Some(g) = self.program.generics.get_mut(self_gen.0) {
            g.scope = scope.clone();
            if let Ok(bound) = own {
                g.bounds.push(bound);
            }
        }
        if let Some(item) = self.program.items.get_mut(id.0) {
            item.scope = scope.clone();
        }
        let mut defaults = BTreeMap::new();
        let mut methods = BTreeSet::new();
        let mut sigs = BTreeMap::new();
        let mut assoc_types = BTreeMap::new();
        let mut consts = BTreeMap::new();
        for ti in &t.items {
            match ti {
                syn::TraitItem::Type(ty) if cfg::ships(&ty.attrs) => {
                    assoc_types.insert(ty.ident.to_string(), ty.bounds.iter().cloned().collect());
                }
                syn::TraitItem::Const(c) if cfg::ships(&c.attrs) => {
                    consts.insert(c.ident.to_string(), c.ty.clone());
                }
                _ => {}
            }
            let syn::TraitItem::Fn(f) = ti else {
                continue;
            };
            if !cfg::ships(&f.attrs) {
                continue;
            }
            let mut sig = f.sig.clone();
            lift_impl_params(&mut sig);
            let mname = sig.ident.to_string();
            methods.insert(mname.clone());
            let mut fscope = scope.clone();
            self.declare_generics(&mut fscope, &sig.generics);
            sigs.insert(mname.clone(), (sig.clone(), fscope.clone()));
            let Some(block) = &f.default else {
                continue;
            };
            let node = self.new_node(Node {
                kind: NodeKind::TraitDefault(id),
                name: format!("{}::{mname}", self.path_of(module, &name)),
                ident: mname.clone(),
                scope: fscope,
                file: file.to_owned(),
                line: line_of(f),
                sig: Some(sig),
                body: Some(block.clone()),
                krate: self.program.module(module).krate,
            });
            defaults.insert(mname, node);
            self.body_items(node, block);
        }
        if let Some(Item {
            kind:
                ItemKind::Trait {
                    defaults: d,
                    methods: m,
                    sigs: s,
                    assoc_types: a,
                    consts: c,
                    ..
                },
            ..
        }) = self.program.items.get_mut(id.0)
        {
            *d = defaults;
            *m = methods;
            *s = sigs;
            *a = assoc_types;
            *c = consts;
        }
    }

    fn add_impl(&mut self, module: ModId, i: &syn::ItemImpl, file: &str, line: usize) {
        let id = ImplId(self.program.impls.len());
        let mut scope = Self::base_scope(module);
        self.declare_generics(&mut scope, &i.generics);
        scope.self_ref = SelfRef::Impl(id);
        // The impl's generic parameters see `Self` too.
        for g in &scope.generics {
            if let Some(generic) = self.program.generics.get_mut(g.0) {
                generic.scope.self_ref = SelfRef::Impl(id);
            }
        }
        let mut assoc = BTreeMap::new();
        let mut consts = BTreeMap::new();
        for ii in &i.items {
            match ii {
                syn::ImplItem::Type(t) if cfg::ships(&t.attrs) => {
                    assoc.insert(t.ident.to_string(), t.ty.clone());
                }
                syn::ImplItem::Const(c) if cfg::ships(&c.attrs) => {
                    consts.insert(c.ident.to_string(), c.ty.clone());
                }
                _ => {}
            }
        }
        self.program.impls.push(Impl {
            scope: scope.clone(),
            self_ty: (*i.self_ty).clone(),
            trait_path: i.trait_.as_ref().map(|(_, p, _)| p.clone()),
            methods: BTreeMap::new(),
            assoc,
            consts,
            file: file.to_owned(),
            line,
        });
        let mut methods: BTreeMap<String, Vec<NodeId>> = BTreeMap::new();
        for ii in &i.items {
            match ii {
                syn::ImplItem::Fn(f) => {
                    if !cfg::ships(&f.attrs) {
                        continue;
                    }
                    let mut sig = f.sig.clone();
                    lift_impl_params(&mut sig);
                    let mname = sig.ident.to_string();
                    let mut fscope = scope.clone();
                    self.declare_generics(&mut fscope, &sig.generics);
                    let node = self.new_node(Node {
                        kind: NodeKind::Method(id),
                        name: String::new(),
                        ident: mname.clone(),
                        scope: fscope,
                        file: file.to_owned(),
                        line: line_of(f),
                        sig: Some(sig),
                        body: Some(f.block.clone()),
                        krate: self.program.module(module).krate,
                    });
                    methods.entry(mname).or_default().push(node);
                    self.body_items(node, &f.block);
                }
                syn::ImplItem::Macro(m) => {
                    let what = "a macro inside an impl block: the recursion check does not expand \
                                macros into associated items";
                    self.problem(file, line_of(m), what);
                }
                syn::ImplItem::Const(_) | syn::ImplItem::Type(_) => {}
                other => {
                    let what = format!(
                        "an associated item the recursion check cannot read: `{}`",
                        crate::recursion::macros::excerpt(&quote::ToTokens::to_token_stream(other))
                    );
                    self.problem(file, line_of(other), &what);
                }
            }
        }
        if let Some(imp) = self.program.impls.get_mut(id.0) {
            imp.methods = methods;
        }
    }

    fn add_mod(&mut self, module: ModId, m: &syn::ItemMod, file: &str, line: usize) {
        let name = m.ident.to_string();
        let parent = self.program.module(module).clone();
        let path = format!("{}::{name}", parent.path);
        let krate = parent.krate;
        if let Some((_, items)) = &m.content {
            // Inline: its own files and `#[path]`s are below the parent's folder.
            let dir = join(&parent.children_dir, &name);
            let id = self.new_module(Module {
                krate,
                parent: Some(module),
                path,
                file: file.to_owned(),
                children_dir: dir.clone(),
                path_base: dir,
                block_of: None,
                types: BTreeMap::new(),
                values: BTreeMap::new(),
                macros: BTreeMap::new(),
                uses: Vec::new(),
            });
            self.define(module, &name, Def::Mod(id), false, true);
            self.jobs.push(Job::Items(id, items.clone()));
            return;
        }
        let (child_file, mod_rs) = if let Some(p) = cfg::path_attribute(&m.attrs) {
            (normalise(&join(&parent.path_base, &p)), true)
        } else {
            let flat = normalise(&join(&parent.children_dir, &format!("{name}.rs")));
            let nested = normalise(&join(&parent.children_dir, &format!("{name}/mod.rs")));
            match ((self.read)(&flat), (self.read)(&nested)) {
                (Ok(_), Err(_)) => (flat, false),
                (Err(_), Ok(_)) => (nested, true),
                (Ok(_), Ok(_)) => {
                    self.problem(
                        file,
                        line,
                        &format!("both {flat} and {nested} exist for `mod {name};`"),
                    );
                    return;
                }
                (Err(_), Err(_)) => {
                    self.problem(
                        file,
                        line,
                        &format!("neither {flat} nor {nested} exists for `mod {name};`"),
                    );
                    return;
                }
            }
        };
        let Some(parsed) = self.parse(&child_file, line) else {
            return;
        };
        let base = dir_of(&child_file);
        let children_dir = if mod_rs {
            base.clone()
        } else {
            join(&base, &name)
        };
        let id = self.new_module(Module {
            krate,
            parent: Some(module),
            path,
            file: child_file,
            children_dir,
            path_base: base,
            block_of: None,
            types: BTreeMap::new(),
            values: BTreeMap::new(),
            macros: BTreeMap::new(),
            uses: Vec::new(),
        });
        self.define(module, &name, Def::Mod(id), false, true);
        self.jobs.push(Job::Items(id, parsed.items));
    }

    /// Gives a function body's items a scope of their own and queues them.
    fn body_items(&mut self, node: NodeId, block: &syn::Block) {
        let mut finder = ItemFinder::default();
        finder.visit_block(block);
        if finder.items.is_empty() {
            return;
        }
        let Some(n) = self.program.node(node).cloned() else {
            return;
        };
        let parent = self.program.module(n.scope.module).clone();
        let id = self.new_module(Module {
            krate: parent.krate,
            parent: Some(n.scope.module),
            path: if n.name.is_empty() {
                format!("{}::{}", parent.path, n.ident)
            } else {
                n.name.clone()
            },
            file: n.file.clone(),
            children_dir: parent.children_dir.clone(),
            path_base: parent.path_base.clone(),
            block_of: Some(node),
            types: BTreeMap::new(),
            values: BTreeMap::new(),
            macros: BTreeMap::new(),
            uses: Vec::new(),
        });
        if let Some(nm) = self.program.nodes.get_mut(node.0) {
            nm.scope.module = id;
        }
        self.jobs.push(Job::Items(id, finder.items));
    }

    fn expand_item_macro(&mut self, module: ModId, mac: &syn::Macro) {
        let file = self.program.module(module).file.clone();
        let line = line_of(mac);
        match crate::recursion::macros::expand_items(&self.program, module, mac) {
            Ok(Some(items)) => self.jobs.push(Job::Items(module, items)),
            Ok(None) => {}
            Err(why) => self.problem(&file, line, &why),
        }
    }
}

/// Finds the items of a function body: in its blocks, closures and nested expressions, but not
/// inside the items themselves.
#[derive(Default)]
struct ItemFinder {
    items: Vec<syn::Item>,
}

impl<'ast> Visit<'ast> for ItemFinder {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        self.items.push(item.clone());
    }
}

/// Makes every `impl Trait` in a function's parameter types a generic parameter of its own, as
/// the language defines it (an anonymous type parameter, The Rust Reference, "Impl trait type",
/// <https://doc.rust-lang.org/reference/types/impl-trait.html#anonymous-type-parameters>): each
/// becomes a parameter named `__impl<n>` with the same bounds, so a call fixes it as it fixes any
/// other.
pub fn lift_impl_params(sig: &mut syn::Signature) {
    struct Lift {
        found: Vec<(
            syn::Ident,
            syn::punctuated::Punctuated<syn::TypeParamBound, syn::Token![+]>,
        )>,
    }
    impl syn::visit_mut::VisitMut for Lift {
        fn visit_type_mut(&mut self, ty: &mut syn::Type) {
            syn::visit_mut::visit_type_mut(self, ty);
            if let syn::Type::ImplTrait(it) = ty {
                let ident = quote::format_ident!("__impl{}", self.found.len());
                self.found.push((ident.clone(), it.bounds.clone()));
                *ty = syn::parse_quote!(#ident);
            }
        }
    }
    let mut lift = Lift { found: Vec::new() };
    for input in &mut sig.inputs {
        if let syn::FnArg::Typed(pt) = input {
            syn::visit_mut::VisitMut::visit_type_mut(&mut lift, &mut pt.ty);
        }
    }
    for (ident, bounds) in lift.found {
        sig.generics.params.push(syn::parse_quote!(#ident: #bounds));
    }
}

/// Flattens a `use` tree into one [`Use`] per binding.
fn flatten_use(tree: &syn::UseTree, prefix: Vec<String>, leading: bool, out: &mut Vec<Use>) {
    let mut work: Vec<(&syn::UseTree, Vec<String>)> = vec![(tree, prefix)];
    while let Some((tree, prefix)) = work.pop() {
        match tree {
            syn::UseTree::Path(p) => {
                let mut next = prefix;
                next.push(p.ident.to_string());
                work.push((&p.tree, next));
            }
            syn::UseTree::Name(n) => {
                let name = n.ident.to_string();
                if name == "self" {
                    let bound = prefix.last().cloned();
                    out.push(Use {
                        segments: prefix,
                        leading_colon: leading,
                        name: bound,
                    });
                } else {
                    let mut segments = prefix;
                    segments.push(name.clone());
                    out.push(Use {
                        segments,
                        leading_colon: leading,
                        name: Some(name),
                    });
                }
            }
            syn::UseTree::Rename(r) => {
                let mut segments = prefix;
                let ident = r.ident.to_string();
                if ident != "self" {
                    segments.push(ident);
                }
                out.push(Use {
                    segments,
                    leading_colon: leading,
                    name: Some(r.rename.to_string()),
                });
            }
            syn::UseTree::Glob(_) => out.push(Use {
                segments: prefix,
                leading_colon: leading,
                name: None,
            }),
            syn::UseTree::Group(g) => {
                for t in &g.items {
                    work.push((t, prefix.clone()));
                }
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
    reason = "tests may use native arithmetic and casts"
)]
mod tests {
    use super::*;

    fn reader(files: &[(&str, &str)]) -> impl Fn(&str) -> Result<String, String> {
        let map: BTreeMap<String, String> = files
            .iter()
            .map(|(p, t)| ((*p).to_owned(), (*t).to_owned()))
            .collect();
        move |p: &str| map.get(p).cloned().ok_or_else(|| "no such file".to_owned())
    }

    fn source(lib: &str) -> CrateSource {
        CrateSource {
            package: lib.replace('_', "-"),
            lib: lib.to_owned(),
            folder: format!("c/{lib}"),
            root: format!("c/{lib}/src/lib.rs"),
            role: Role::Ships,
        }
    }

    #[test]
    fn the_module_tree_is_the_compilers() {
        let read = reader(&[
            (
                "c/a/src/lib.rs",
                "mod flat; mod nested; #[path = \"elsewhere/p.rs\"] mod moved; \
                 #[cfg(test)] mod tests; mod inline { mod deeper; }",
            ),
            ("c/a/src/flat.rs", "mod child; fn f() {}"),
            ("c/a/src/flat/child.rs", "fn g() {}"),
            ("c/a/src/nested/mod.rs", "mod inner;"),
            ("c/a/src/nested/inner.rs", "fn h() {}"),
            ("c/a/src/elsewhere/p.rs", "mod q;"),
            ("c/a/src/elsewhere/q.rs", "fn k() {}"),
            ("c/a/src/inline/deeper.rs", "fn m() {}"),
        ]);
        let p = load(&[source("a")], &read);
        assert!(p.problems.is_empty(), "{:?}", p.problems);
        let mut names: Vec<&str> = p.nodes.iter().map(|n| n.name.as_str()).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "a::flat::child::g",
                "a::flat::f",
                "a::inline::deeper::m",
                "a::moved::q::k",
                "a::nested::inner::h",
            ]
        );
    }

    #[test]
    fn missing_and_unparsable_files_are_problems() {
        let read = reader(&[
            ("c/a/src/lib.rs", "mod gone; mod bad;"),
            ("c/a/src/bad.rs", "fn ("),
        ]);
        let p = load(&[source("a")], &read);
        let what: Vec<&str> = p.problems.iter().map(|x| x.what.as_str()).collect();
        assert_eq!(what.len(), 2, "{what:?}");
        assert!(what.iter().any(|w| w.contains("neither")), "{what:?}");
        assert!(
            what.iter().any(|w| w.contains("does not parse")),
            "{what:?}"
        );
    }

    #[test]
    fn body_items_get_a_scope_and_derives_get_nodes() {
        let read = reader(&[(
            "c/a/src/lib.rs",
            "fn outer() { fn inner() {} let _ = || { struct S; impl S { fn m() {} } }; }\n\
             #[derive(Clone, Debug, Copy)] struct T(u8);",
        )]);
        let p = load(&[source("a")], &read);
        assert!(p.problems.is_empty(), "{:?}", p.problems);
        let inner = p.nodes.iter().find(|n| n.ident == "inner").unwrap();
        assert_eq!(inner.name, "a::outer::inner");
        let outer_id = NodeId(p.nodes.iter().position(|n| n.ident == "outer").unwrap());
        assert_eq!(
            p.module(inner.scope.module).block_of,
            Some(outer_id),
            "an item of a body sees its sibling items, in its function's block scope"
        );
        let outer = p.nodes.iter().find(|n| n.ident == "outer").unwrap();
        assert!(p.module(outer.scope.module).block_of.is_some());
        let derived: Vec<&NodeKind> = p
            .nodes
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::Derived { .. }))
            .map(|n| &n.kind)
            .collect();
        assert_eq!(derived.len(), 2, "Clone and Debug; Copy has no method");
    }

    #[test]
    fn use_trees_flatten() {
        let item: syn::ItemUse = syn::parse_str("use a::{b, c::{self as d, *}, e as f};").unwrap();
        let mut out = Vec::new();
        flatten_use(&item.tree, Vec::new(), false, &mut out);
        let mut got: Vec<(String, Option<String>)> = out
            .iter()
            .map(|u| (u.segments.join("::"), u.name.clone()))
            .collect();
        got.sort();
        assert_eq!(
            got,
            [
                ("a::b".to_owned(), Some("b".to_owned())),
                ("a::c".to_owned(), None),
                ("a::c".to_owned(), Some("d".to_owned())),
                ("a::e".to_owned(), Some("f".to_owned())),
            ]
        );
    }
}
