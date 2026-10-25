//! The map for people and AI: `projects/engines/geometry/MAP.json`, generated from the crate and
//! module headers of every crate of the geometry kernel, and the check that every header is
//! complete and the committed map is current.
//!
//! What it does: [`build`] reads an [`Input`] (each crate's manifest and every `.rs` file under its
//! `src/`) and returns the map as JSON text with every [`Problem`]: a crate or module header that
//! lacks a field of the grammar in `projects/engines/geometry/HEADERS.md`, a provenance record that
//! is incomplete or takes more than chapter 29.2 allows, a link to outside code that nothing
//! credits, or an error kind without a doc comment. [`repo`] reads the input from a workspace on
//! disk (the crates under `projects/engines/geometry/crates`, and the facade `engine-geometry` and
//! its binding crates under `projects/engines/crates`) and writes or checks the committed map; the
//! binary `map` (`cargo run -p geom-test --bin map -- check`) runs it in CI. The map lists each
//! crate's job, layer, invariants, sections, family, status and published dependencies; each
//! module's job, what it does, guarantees, method with its links, cost and family; every
//! provenance record, merged into one entry per source; and every error kind of each kernel crate.
//!
//! Why: chapter 26.3 asks for a machine-readable map of every crate, module, invariant and error
//! kind, "generated from annotations in the code and checked in CI, so it can never drift from the
//! code", and chapter 29.20 asks the same generator to read the `Source:` and `Consulted:` lines
//! into one record per source. Operations and geometry kinds join the map when the pieces that
//! define them land.
//!
//! Guarantees:
//!
//! - **One map per source tree.** The text depends only on the files read: crates in name order,
//!   modules in path order, fields in a fixed order, object keys sorted, list items in header
//!   order, no timestamps, no absolute paths (every path is relative to the workspace root, with
//!   `/`), and `\n` line ends. The same tree gives the same bytes on every target.
//! - **Every problem names its file and field.** A [`Problem`] carries the file's path, the
//!   [`Rule`] (the field of the grammar) and what to write; nothing stops at the first problem.
//! - **Known gaps are visible, exact and cannot rot.** A problem listed in [`GAPS`] (or the gaps a
//!   test passes) by its file, rule and exact message is not reported, but the map lists the gap
//!   with its reason; any other problem in the same file and rule is still reported, and a gap
//!   that no longer occurs is itself a problem ([`Rule::GapStale`]), as the scaffold check's header
//!   gaps work.
//! - **One record per source.** Records are merged by how the source was taken and the source's
//!   link without its fragment, so one file at one commit is one source whatever line range a
//!   header shows; two headers that give one source different licences are a problem in each, not
//!   a silent choice (29.20, rule 1).
//! - **The map is built even when headers are incomplete**, with `null` or empty fields, so a
//!   person can regenerate it and read what is missing; the check fails on any problem.
//!
//! Method: regenerate and compare, the way the kernel keeps every generated file honest (chapter
//! 27, "Generated code and data have one home",
//! `projects/engines/geometry/architecture/27-crates-folders.md`), and the way Clippy's
//! `cargo dev update_lints` and rust-analyzer's tidy tests keep their generated lint lists and
//! module documentation current; headers are read by the written grammar ([`header`]), never by
//! guessing, as rustc's tidy reads its error-code registry rather than the compiler's messages
//! (<https://github.com/rust-lang/rust/blob/1731faea0da5b4ebf881d8e53f98066c17a74711/src/tools/tidy/src/error_codes.rs>).
//! rustdoc's JSON output would give the same doc text with resolved items, but it needs a nightly
//! compiler and its format changes with every breaking release (`FORMAT_VERSION` 61 at
//! <https://github.com/rust-lang/rustdoc-types/blob/2e63bdc44b94e86fe12d6e0034d1ed9c2ac6e9dc/src/lib.rs>),
//! so the map reads the source, which the pinned stable toolchain can always do.
//!
//! Cost: linear in the size of the source files read, plus sorting the crates, modules and sources.
//!
//! Consulted: rustc's tidy, MIT OR Apache-2.0,
//!   <https://github.com/rust-lang/rust/blob/1731faea0da5b4ebf881d8e53f98066c17a74711/src/tools/tidy/src/error_codes.rs>,
//!   for reading a catalogue from one registry and its lists of known exemptions. No code copied.
//! Consulted: `rustdoc-types`, MIT OR Apache-2.0,
//!   <https://github.com/rust-lang/rustdoc-types/blob/2e63bdc44b94e86fe12d6e0034d1ed9c2ac6e9dc/src/lib.rs>,
//!   for what rustdoc's JSON format holds and how it is versioned, to decide not to use it. No code
//!   copied.

pub mod classes;
pub mod errors;
pub mod header;
pub mod items;
pub mod json;
pub mod links;
pub mod provenance;
pub mod repo;
pub mod rust;

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use crate::deps::DEVELOPMENT_CRATES;
use crate::licence::{Expr, records};
pub use header::CrateKind;
use items::Items;
use json::{Value, object};

/// The committed map, relative to the workspace root.
pub const MAP_FILE: &str = "projects/engines/geometry/MAP.json";

/// The written grammar the map is read by, relative to the workspace root.
pub const GRAMMAR_FILE: &str = "projects/engines/geometry/HEADERS.md";

/// The folder holding one folder per kernel crate, relative to the workspace root.
pub const CRATES_DIR: &str = "projects/engines/geometry/crates";

/// The folder holding the facade and its binding crates, beside the other shared engines.
pub const FACADE_DIR: &str = "projects/engines/crates";

/// The facade's name; its binding crates are named `engine-geometry-<binding>` (chapter 27).
pub const FACADE: &str = "engine-geometry";

/// The map's format. It changes when a field's meaning changes or a field is removed; adding a
/// field does not change it.
pub const FORMAT: u64 = 1;

/// The command that writes the map.
pub const WRITE_COMMAND: &str = "cargo run -p geom-test --bin map -- write";

/// One rule of the grammar: a field, or a check on the files and the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rule {
    /// The crate's `Cargo.toml` is missing, names another crate, or moves a target's root.
    Manifest,
    /// The crate has neither `src/lib.rs` nor `src/main.rs`.
    Root,
    /// A file is not UTF-8, is a symbolic link, or cannot be read as Rust tokens.
    Unreadable,
    /// The file does not open with a `//!` header.
    Header,
    /// The first paragraph is not a summary in the form the grammar writes.
    Summary,
    /// A crate header states no job.
    Job,
    /// A crate header states no layer, or a wrong one.
    Layer,
    /// A crate header lists no invariants, or one that states nothing.
    Invariants,
    /// A crate header names no chapter it implements.
    Sections,
    /// A module header states no guarantees.
    Guarantees,
    /// A module header names no method with a link.
    Method,
    /// A module header states no cost.
    Cost,
    /// A family that is not one of chapter 10.2's catalogue, or a crate without its family.
    Family,
    /// An incomplete `Source:` record, or a port 29.2 does not allow.
    Source,
    /// An incomplete `Consulted:` record, or a read 29.2 does not allow.
    Consulted,
    /// An unreadable or missing `SPDX-License-Identifier:` line, or a licence a crate's manifest
    /// does not name.
    Spdx,
    /// A published crate a kernel crate depends on whose licence the header does not record.
    Depend,
    /// A link to outside code that no record credits.
    Uncredited,
    /// An error enum the map cannot place.
    ErrorEnum,
    /// An error enum or error kind without a doc comment.
    ErrorDoc,
    /// The committed map is missing or differs from the generated one.
    Stale,
    /// A known gap that no longer occurs.
    GapStale,
}

impl Rule {
    /// Every rule, in the order `HEADERS.md` lists them.
    pub const ALL: [Rule; 22] = [
        Rule::Manifest,
        Rule::Root,
        Rule::Unreadable,
        Rule::Header,
        Rule::Summary,
        Rule::Job,
        Rule::Layer,
        Rule::Invariants,
        Rule::Sections,
        Rule::Guarantees,
        Rule::Method,
        Rule::Cost,
        Rule::Family,
        Rule::Source,
        Rule::Consulted,
        Rule::Spdx,
        Rule::Depend,
        Rule::Uncredited,
        Rule::ErrorEnum,
        Rule::ErrorDoc,
        Rule::Stale,
        Rule::GapStale,
    ];

    /// The rule's name, as `HEADERS.md` and every problem message write it.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Rule::Manifest => "Manifest",
            Rule::Root => "Root",
            Rule::Unreadable => "Unreadable",
            Rule::Header => "Header",
            Rule::Summary => "Summary",
            Rule::Job => "Job",
            Rule::Layer => "Layer",
            Rule::Invariants => "Invariants",
            Rule::Sections => "Sections",
            Rule::Guarantees => "Guarantees",
            Rule::Method => "Method",
            Rule::Cost => "Cost",
            Rule::Family => "Family",
            Rule::Source => "Source",
            Rule::Consulted => "Consulted",
            Rule::Spdx => "SPDX",
            Rule::Depend => "Depend",
            Rule::Uncredited => "Uncredited",
            Rule::ErrorEnum => "Error enum",
            Rule::ErrorDoc => "Error doc",
            Rule::Stale => "Stale map",
            Rule::GapStale => "Stale gap",
        }
    }
}

/// What is wrong, where, and by which rule.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Problem {
    /// The file, relative to the workspace root, with `/`.
    pub file: String,
    /// The rule broken: the field of the grammar, or the check.
    pub rule: Rule,
    /// What is wrong and what to write.
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}: {}", self.file, self.rule.label(), self.message)
    }
}

/// A known gap: one problem that is accepted for now, with the reason and who closes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gap {
    /// The file, relative to the workspace root.
    pub file: &'static str,
    /// The rule the file breaks.
    pub rule: Rule,
    /// The problem's message, exactly: the gap hides that problem and no other.
    pub problem: &'static str,
    /// Why it is accepted, and which piece closes it.
    pub reason: &'static str,
}

/// The known gaps of this workspace. Each is listed in the map; each must still occur. There are
/// none: the last, `geom-num`'s interval module crediting CGAL's `FPU.h` (LGPL) without a method
/// note, was closed by build piece G16, which wrote the note
/// (`projects/engines/geometry/methods/cgal-nextafter-widening.md`).
pub const GAPS: &[Gap] = &[];

/// A folder that headers link into but that this tree does not hold yet, with the piece that
/// brings it. While it is absent, links into it are not checked for existence; once it exists,
/// every link into it must resolve and the entry is stale ([`repo::check`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pending {
    /// The folder, relative to the workspace root, ending in `/`.
    pub folder: &'static str,
    /// Why it is absent, and which piece brings it.
    pub reason: &'static str,
}

/// The folders this workspace's headers link into before they arrive.
pub const PENDING: &[Pending] = &[];

/// One source file, relative to the workspace root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    /// The path, relative to the workspace root, with `/`.
    pub path: String,
    /// The file's text, or why it cannot be read.
    pub text: Result<String, String>,
}

/// One crate folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrateInput {
    /// The folder's name, which is the crate's name (chapter 27).
    pub name: String,
    /// The folder, relative to the workspace root.
    pub folder: String,
    /// The text of its `Cargo.toml`, if it has one.
    pub manifest: Option<String>,
    /// Every `.rs` file under its `src/`, in path order.
    pub files: Vec<SourceFile>,
}

/// Everything the map is built from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Input {
    /// Every crate folder, in name order.
    pub crates: Vec<CrateInput>,
    /// Paths the map cannot read (a crate folder that is a symbolic link), each with why.
    pub unreadable: Vec<SourceFile>,
    /// The repository paths that headers link to and that exist, when the input was read from a
    /// workspace ([`repo::read`]); `None` for an input built in memory, whose links into the
    /// repository are then not checked.
    pub repo_paths: Option<BTreeSet<String>>,
    /// The entries of [`PENDING`] whose folder the workspace does not hold, so links into them
    /// are not checked yet.
    pub pending: Vec<Pending>,
}

impl Input {
    /// Whether a link into the repository resolves, or points into a folder still pending; always
    /// true for an input built in memory.
    fn resolves(&self, path: &str) -> bool {
        self.repo_paths
            .as_ref()
            .is_none_or(|paths| paths.contains(path))
            || self.pending.iter().any(|p| path.starts_with(p.folder))
    }
}

/// The generated map and every problem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Generated {
    /// The map as JSON text.
    pub text: String,
    /// Every problem not covered by a gap, sorted by file, rule and message.
    pub problems: Vec<Problem>,
    /// The map as a value, which [`crate::ledger`] reads to write `PROVENANCE.md` and `NOTICE`
    /// from the same records, so the ledger never parses a header a second way.
    pub map: Value,
}

/// Whether `name` is a development crate (chapter 27; [`DEVELOPMENT_CRATES`]).
#[must_use]
pub fn is_development(name: &str) -> bool {
    DEVELOPMENT_CRATES.contains(&name)
}

/// Whether `name` is the facade or one of its binding crates.
#[must_use]
pub fn is_facade(name: &str) -> bool {
    name == FACADE
        || name
            .strip_prefix(FACADE)
            .is_some_and(|rest| rest.starts_with('-'))
}

/// The kind of the crate `name` in `folder`.
#[must_use]
pub fn crate_kind(name: &str, folder: &str) -> CrateKind {
    if folder
        .strip_prefix(FACADE_DIR)
        .is_some_and(|rest| rest.starts_with('/'))
    {
        CrateKind::Facade
    } else if is_development(name) {
        CrateKind::Development
    } else {
        CrateKind::Kernel
    }
}

/// Every problem, before gaps.
struct Collected {
    problems: Vec<Problem>,
}

impl Collected {
    fn add(&mut self, file: &str, found: Vec<(Rule, String)>) {
        self.problems
            .extend(found.into_iter().map(|(rule, message)| Problem {
                file: file.to_owned(),
                rule,
                message,
            }));
    }

    fn one(&mut self, file: &str, rule: Rule, message: String) {
        self.add(file, vec![(rule, message)]);
    }
}

/// What a file is to its crate.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Role {
    /// The crate root: `src/lib.rs`, or `src/main.rs` of a crate without a library.
    CrateRoot,
    /// A module of the library, with its path.
    Module(String),
    /// A binary's root, with the binary's name.
    Binary(String),
    /// A module of a binary (`src/bin/<name>/<module>.rs`), with its path from the binary.
    BinModule(String),
}

impl Role {
    /// Whether the file belongs to a binary target.
    fn in_binary(&self) -> bool {
        matches!(self, Role::Binary(_) | Role::BinModule(_))
    }
}

/// A module path from a file path under a target's folder: `a/b.rs` and `a/b/mod.rs` are `a::b`.
fn module_path(base: &str, relative: &str) -> String {
    let without = relative
        .strip_suffix("/mod.rs")
        .or_else(|| relative.strip_suffix(".rs"))
        .unwrap_or(relative);
    let mut path = base.to_owned();
    for part in without.split('/') {
        path.push_str("::");
        path.push_str(part);
    }
    path
}

/// The role of `relative` (a path under `src/`) in crate `name`, as Cargo's target layout makes
/// it: `src/bin/<name>.rs` and `src/bin/<name>/main.rs` are binaries, and the other files under
/// `src/bin/<name>/` are that binary's modules.
fn role(name: &str, relative: &str, has_lib: bool) -> Role {
    if relative == "lib.rs" || (relative == "main.rs" && !has_lib) {
        return Role::CrateRoot;
    }
    if relative == "main.rs" {
        return Role::Binary(name.to_owned());
    }
    if let Some(bin) = relative.strip_prefix("bin/") {
        return match bin.split_once('/') {
            None => Role::Binary(bin.strip_suffix(".rs").unwrap_or(bin).to_owned()),
            Some((binary, "main.rs")) => Role::Binary(binary.to_owned()),
            Some((binary, inner)) => Role::BinModule(module_path(binary, inner)),
        };
    }
    Role::Module(module_path(&name.replace('-', "_"), relative))
}

/// A published dependency named in a manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Dependency {
    /// The package's name.
    name: String,
    /// The version requirement, if the manifest gives one.
    version: Option<String>,
}

/// What the map reads from a manifest besides its problems.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Manifest {
    /// The `license` field: the expression, or `None` when absent or inherited from the workspace.
    licence: Option<String>,
    /// The published crates named by `[dependencies]` and `[build-dependencies]` (any target).
    dependencies: Vec<Dependency>,
}

/// A table of normal or build dependencies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DependencyTable<'a> {
    /// `[dependencies]`: one dependency per line.
    Plain,
    /// `[dependencies.<package>]`: one dependency, its fields one per line.
    Package(&'a str),
}

/// Whether a table header names normal or build dependencies, and which package a dotted header
/// (`[dependencies.libm]`) names.
fn dependency_table(table: &str) -> Option<DependencyTable<'_>> {
    let inner = table.trim_start_matches('[').trim_end_matches(']');
    let inner = inner
        .strip_prefix("target.")
        .and_then(|t| t.rsplit_once("'.").or_else(|| t.rsplit_once("\".")))
        .map_or(inner, |(_, rest)| rest);
    let (kind, package) = inner.split_once('.').unwrap_or((inner, ""));
    matches!(kind, "dependencies" | "build-dependencies").then_some(if package.is_empty() {
        DependencyTable::Plain
    } else {
        DependencyTable::Package(package)
    })
}

/// A dotted dependency table being read (`[dependencies.<package>]`): its package, its version and
/// whether it is a path dependency.
type Dotted = Option<(String, Option<String>, bool)>;

/// Ends a dotted dependency table: a published package joins the manifest's dependencies.
fn finish(dotted: &mut Dotted, manifest: &mut Manifest) {
    if let Some((name, version, local)) = dotted.take()
        && !local
    {
        manifest.dependencies.push(Dependency { name, version });
    }
}

/// A TOML value as text: a basic string's content (whatever follows it on the line), or the
/// value itself.
fn string_value(value: &str) -> String {
    if value.starts_with('"') {
        value.split('"').nth(1).unwrap_or_default().to_owned()
    } else {
        value.to_owned()
    }
}

/// The manifest rule (a `Cargo.toml` whose package name is the folder's, and no target whose root
/// is moved with `path`), and what the map reads from it.
fn check_manifest(krate: &CrateInput) -> (Manifest, Vec<(Rule, String)>) {
    let Some(text) = &krate.manifest else {
        return (
            Manifest::default(),
            vec![(
                Rule::Manifest,
                "the crate folder has no Cargo.toml".to_owned(),
            )],
        );
    };
    let mut problems = Vec::new();
    let mut manifest = Manifest::default();
    let mut table = String::new();
    let mut name: Option<String> = None;
    let mut dotted: Dotted = None;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            finish(&mut dotted, &mut manifest);
            line.clone_into(&mut table);
            if let Some(DependencyTable::Package(package)) = dependency_table(&table) {
                dotted = Some((package.to_owned(), None, false));
            }
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        if table == "[package]" && key == "name" {
            name = Some(string_value(value));
        }
        if table == "[package]" && key == "license" {
            manifest.licence = Some(string_value(value));
        }
        if key == "path" && matches!(table.as_str(), "[lib]" | "[[bin]]") {
            problems.push((
                Rule::Manifest,
                format!(
                    "{table} sets path = {value}; the map reads crate roots where cargo finds them \
                     by default (src/lib.rs, src/main.rs, src/bin/)"
                ),
            ));
        }
        match (dependency_table(&table), &mut dotted) {
            (Some(DependencyTable::Package(_)), Some((_, version, local))) => {
                if key == "version" {
                    *version = Some(string_value(value));
                }
                *local |= key == "path";
            }
            (Some(DependencyTable::Plain), _) if !line.starts_with('#') => {
                let package = key.split('.').next().unwrap_or(key).trim().to_owned();
                let local = value.contains("path") && value.starts_with('{');
                let version = if value.starts_with('"') {
                    Some(string_value(value))
                } else {
                    value
                        .split_once("version")
                        .and_then(|(_, rest)| rest.split('"').nth(1))
                        .map(str::to_owned)
                };
                if !local {
                    manifest.dependencies.push(Dependency {
                        name: package,
                        version,
                    });
                }
            }
            _ => {}
        }
    }
    finish(&mut dotted, &mut manifest);
    if name.as_deref() != Some(krate.name.as_str()) {
        problems.push((
            Rule::Manifest,
            format!(
                "the package is named {name:?}, not the folder's name {:?} (chapter 27: folder name \
                 = crate name)",
                krate.name
            ),
        ));
    }
    (manifest, problems)
}

/// One file of a crate, read.
struct Read<'a> {
    /// The file, relative to the workspace root.
    path: &'a str,
    /// What it is to the crate.
    role: Role,
    /// Its text.
    text: &'a str,
    /// What it declares; empty when it cannot be read as Rust tokens.
    items: Items,
    /// Its tokens, when it can be read as Rust tokens.
    tokens: Option<Vec<rust::Token>>,
}

/// A header's provenance records as map values, its SPDX line and its ported licences.
struct ProvenanceValues {
    records: Vec<Value>,
    spdx: Option<String>,
    ports: Vec<String>,
}

/// The provenance records of one header, as map values, and the problems.
fn provenance_values(
    file: &str,
    header: &header::Header,
    docs: &[String],
    input: &Input,
    out: &mut Collected,
) -> ProvenanceValues {
    let p = provenance::read(header, docs);
    out.add(file, p.problems);
    let mut ports = Vec::new();
    let records = p
        .records
        .into_iter()
        .map(|r| {
            if r.take == provenance::Take::Port {
                ports.push(r.licence.clone());
            }
            if let Some(note) = &r.method_note
                && !input.resolves(note)
            {
                out.one(
                    file,
                    Rule::Consulted,
                    format!("the method note `{note}` of {} does not exist", r.name),
                );
            }
            record_value(&r)
        })
        .collect();
    ProvenanceValues {
        records,
        spdx: p.spdx,
        ports,
    }
}

/// A provenance record as a map value.
fn record_value(r: &provenance::Record) -> Value {
    object([
        ("licence", Value::from(r.licence.as_str())),
        ("link", Value::from(r.link.as_str())),
        ("method_note", Value::from(r.method_note.clone())),
        ("name", Value::from(r.name.as_str())),
        ("notice", Value::from(r.notice.clone())),
        ("pinned", Value::from(r.pinned)),
        ("purpose", Value::from(r.purpose.as_str())),
        ("take", Value::from(r.take.name())),
    ])
}

/// A module's entry in the map, and its ported licences. For a crate root that holds code, the
/// crate entry carries the header's provenance and a missing header, so they are not repeated.
fn module_value(
    file: &Read<'_>,
    root: bool,
    library: &str,
    input: &Input,
    out: &mut Collected,
) -> (Value, Vec<String>) {
    let header = header::Header::parse(file.text);
    if header.is_empty() && !root {
        out.one(
            file.path,
            Rule::Header,
            "the file does not open with a //! header (HEADERS.md, module headers)".to_owned(),
        );
    }
    let (fields, problems) = header::module_fields(&header);
    if !header.is_empty() {
        out.add(file.path, problems);
    }
    match &fields.method {
        Some(header::Method::None { .. })
            if !file.role.in_binary() && !file.items.functions.is_empty() =>
        {
            out.one(
                file.path,
                Rule::Method,
                format!(
                    "\"Method: none\" is for a module that holds only types, constants and trait \
                     implementations, or a binary's argument parsing; this one defines functions \
                     ({}), so name the published method they follow, with a link",
                    file.items.functions.join(", ")
                ),
            );
        }
        Some(header::Method::Linked { links, .. }) => {
            for link in links.iter().filter(|l| l.starts_with("projects/")) {
                if !input.resolves(link) {
                    out.one(
                        file.path,
                        Rule::Method,
                        format!(
                            "the method's link `{link}` names no file or folder in this repository"
                        ),
                    );
                }
            }
        }
        _ => {}
    }
    let provenance = if root {
        ProvenanceValues {
            records: Vec::new(),
            spdx: None,
            ports: Vec::new(),
        }
    } else {
        provenance_values(file.path, &header, &file.items.docs, input, out)
    };
    let method = match fields.method {
        Some(header::Method::Linked { text, links }) => {
            object([("links", Value::from(links)), ("text", Value::from(text))])
        }
        Some(header::Method::None { reason }) => object([("none", Value::from(reason))]),
        None => Value::Null,
    };
    let (path, target) = match &file.role {
        Role::Module(path) => (path.clone(), "lib"),
        Role::Binary(name) | Role::BinModule(name) => (name.clone(), "bin"),
        Role::CrateRoot => (library.to_owned(), "lib"),
    };
    let value = object([
        ("cost", Value::from(fields.cost)),
        ("does", Value::from(fields.does)),
        ("family", Value::from(fields.family)),
        ("file", Value::from(file.path)),
        ("guarantees", Value::from(fields.guarantees)),
        ("job", Value::from(fields.job)),
        ("method", method),
        ("path", Value::from(path)),
        ("provenance", Value::Array(provenance.records)),
        ("spdx", Value::from(provenance.spdx)),
        ("target", Value::from(target)),
    ]);
    (value, provenance.ports)
}

/// The error enums of one kernel-crate file, as map values.
fn error_values(
    file: &Read<'_>,
    error_types: &BTreeSet<String>,
    out: &mut Collected,
) -> Vec<Value> {
    let Some(tokens) = &file.tokens else {
        return Vec::new();
    };
    let found = errors::find(tokens, error_types);
    out.add(file.path, found.problems);
    let module = match &file.role {
        Role::Module(path) | Role::Binary(path) | Role::BinModule(path) => path.clone(),
        Role::CrateRoot => String::new(),
    };
    found
        .enums
        .into_iter()
        .map(|e| {
            let kinds = e
                .variants
                .into_iter()
                .map(|v| {
                    object([
                        ("doc", Value::from(v.doc)),
                        ("name", Value::from(v.name)),
                        ("shape", Value::from(v.shape.name())),
                    ])
                })
                .collect();
            object([
                ("doc", Value::from(e.doc)),
                ("file", Value::from(file.path)),
                ("kinds", Value::Array(kinds)),
                ("module", Value::from(module.clone())),
                ("name", Value::from(e.name)),
                ("non_exhaustive", Value::from(e.non_exhaustive)),
            ])
        })
        .collect()
}

/// The crate header's entry fields, read from its root file, and its ported licences.
fn crate_header_values(
    krate: &CrateInput,
    root: Option<&Read<'_>>,
    kind: CrateKind,
    input: &Input,
    out: &mut Collected,
) -> (Vec<(&'static str, Value)>, Vec<String>) {
    let Some(root) = root else {
        return (Vec::new(), Vec::new());
    };
    let header = header::Header::parse(root.text);
    if header.is_empty() {
        out.one(
            root.path,
            Rule::Header,
            "the crate root does not open with a //! header (HEADERS.md, crate headers)".to_owned(),
        );
    }
    let (fields, problems) = header::crate_fields(&header, kind);
    if !header.is_empty() {
        out.add(root.path, problems);
    }
    for family in header::required_families(&krate.name) {
        if !fields.family.iter().any(|f| f == family) {
            out.one(
                root.path,
                Rule::Family,
                format!(
                    "{}'s job in chapter 27's crate table is chapter 10.2's {family} family: \
                     state it, \"Family: {}.\"",
                    krate.name,
                    header::required_families(&krate.name).join(" and ")
                ),
            );
        }
    }
    let provenance = provenance_values(root.path, &header, &root.items.docs, input, out);
    let values = vec![
        (
            "chapters",
            Value::Array(
                fields
                    .chapters
                    .into_iter()
                    .map(|c| Value::Number(u64::from(c)))
                    .collect(),
            ),
        ),
        ("family", Value::from(fields.family)),
        ("file", Value::from(root.path)),
        ("invariants", Value::from(fields.invariants)),
        ("job", Value::from(fields.job)),
        ("layer", Value::from(fields.layer)),
        ("provenance", Value::Array(provenance.records)),
        ("sections", Value::from(fields.sections)),
        ("spdx", Value::from(provenance.spdx)),
        ("status", Value::from(fields.status)),
        ("summary", Value::from(fields.summary)),
    ];
    (values, provenance.ports)
}

/// The published crates a kernel or facade crate depends on (DEPEND, 29.1), as map values, each
/// with the licence its crate header's `Licences` list records.
fn depend_values(
    krate: &CrateInput,
    manifest: &Manifest,
    root: Option<&Read<'_>>,
    out: &mut Collected,
) -> Vec<Value> {
    if manifest.dependencies.is_empty() {
        return Vec::new();
    }
    let manifest_file = format!("{}/Cargo.toml", krate.folder);
    // Without a crate header, the header's own problem says what is missing.
    let Some(root) = root.filter(|r| !header::Header::parse(r.text).is_empty()) else {
        return Vec::new();
    };
    let root_file = root.path;
    let recorded = records(root.text).unwrap_or_default();
    manifest
        .dependencies
        .iter()
        .map(|dep| {
            let licence = recorded
                .iter()
                .find(|r| r.packages.contains(&dep.name))
                .map(|r| r.expression.canonical());
            match &licence {
                None => out.one(
                    root_file,
                    Rule::Depend,
                    format!(
                        "{} depends on the published crate `{}` (DEPEND, 29.1): record its licence \
                         in the crate header's Licences list, \"- <licence>: `{}`.\"",
                        krate.name, dep.name, dep.name
                    ),
                ),
                Some(l) if classes::of(l) != Ok(classes::Class::Allowed) => out.one(
                    root_file,
                    Rule::Depend,
                    format!(
                        "`{}` is under {l}, which is not on chapter 29.2's allow-list for the \
                         kernel's Cargo.lock",
                        dep.name
                    ),
                ),
                Some(_) => {}
            }
            let exact = dep.version.as_deref().filter(|v| {
                v.split('.').count() == 3 && v.chars().all(|c| c.is_ascii_digit() || c == '.')
            });
            let link = match exact {
                Some(v) => format!("https://crates.io/crates/{}/{v}", dep.name),
                None => format!("https://crates.io/crates/{}", dep.name),
            };
            object([
                ("licence", Value::from(licence.unwrap_or_default())),
                ("link", Value::from(link)),
                ("manifest", Value::from(manifest_file.as_str())),
                ("method_note", Value::Null),
                ("name", Value::from(dep.name.as_str())),
                ("pinned", Value::from(false)),
                (
                    "purpose",
                    Value::from(dep.version.clone().unwrap_or_default()),
                ),
                ("take", Value::from(provenance::Take::Depend.name())),
            ])
        })
        .collect()
}

/// The crate's `license` field names every ported file's licence beside MPL-2.0 (29.20, rule 4).
fn check_crate_licence(
    krate: &CrateInput,
    manifest: &Manifest,
    ports: &[String],
    out: &mut Collected,
) {
    if ports.is_empty() {
        return;
    }
    let file = format!("{}/Cargo.toml", krate.folder);
    let operands: Vec<String> = match manifest.licence.as_deref().map(Expr::parse) {
        Some(Ok(expr)) => {
            let mut all = vec![expr.canonical()];
            if let Expr::And(terms) = &expr {
                all.extend(terms.iter().map(Expr::canonical));
            }
            all
        }
        _ => Vec::new(),
    };
    let mut wanted: Vec<String> = vec!["MPL-2.0".to_owned()];
    wanted.extend(
        ports
            .iter()
            .map(|l| Expr::parse(l).map_or_else(|_| l.clone(), |e| e.canonical())),
    );
    wanted.sort();
    wanted.dedup();
    let missing: Vec<&String> = wanted.iter().filter(|w| !operands.contains(w)).collect();
    if !missing.is_empty() {
        out.one(
            &file,
            Rule::Spdx,
            format!(
                "the crate ports files under {}, so its own `license` field names each beside \
                 MPL-2.0 (29.20, rule 4): license = \"{}\"",
                ports.join(", "),
                wanted.join(" AND ")
            ),
        );
    }
}

/// One crate's entry in the map.
fn crate_value(krate: &CrateInput, input: &Input, out: &mut Collected) -> Value {
    let kind = crate_kind(&krate.name, &krate.folder);
    let (manifest, problems) = check_manifest(krate);
    out.add(&format!("{}/Cargo.toml", krate.folder), problems);
    let src = format!("{}/src/", krate.folder);
    let library = krate.name.replace('-', "_");
    let has_lib = krate
        .files
        .iter()
        .any(|f| f.path.strip_prefix(&src) == Some("lib.rs"));

    let mut read: Vec<Read<'_>> = Vec::new();
    for file in &krate.files {
        let Some(rel) = file.path.strip_prefix(&src) else {
            continue;
        };
        let text = match &file.text {
            Ok(t) => t.as_str(),
            Err(why) => {
                out.one(&file.path, Rule::Unreadable, why.clone());
                continue;
            }
        };
        let tokens = match rust::tokens(text) {
            Ok(t) => Some(t),
            Err(e) => {
                out.one(
                    &file.path,
                    Rule::Unreadable,
                    format!("the file cannot be read as Rust: {e}"),
                );
                None
            }
        };
        read.push(Read {
            path: &file.path,
            role: role(&krate.name, rel, has_lib),
            text,
            items: tokens.as_deref().map(items::scan).unwrap_or_default(),
            tokens,
        });
    }
    let root = read.iter().find(|r| r.role == Role::CrateRoot);
    let root_present = krate.files.iter().any(|f| {
        f.path
            .strip_prefix(&src)
            .is_some_and(|rel| role(&krate.name, rel, has_lib) == Role::CrateRoot)
    });
    if !root_present {
        out.one(
            &krate.folder,
            Rule::Root,
            "the crate has neither src/lib.rs nor src/main.rs".to_owned(),
        );
    }

    let mut entry: BTreeMap<String, Value> = BTreeMap::new();
    let (values, mut ports) = crate_header_values(krate, root, kind, input, out);
    for (key, value) in values {
        entry.insert(key.to_owned(), value);
    }

    let error_types: BTreeSet<String> = read
        .iter()
        .flat_map(|r| r.items.error_impls.iter().chain(&r.items.result_errors))
        .cloned()
        .collect();
    let mut modules = Vec::new();
    let mut error_enums = Vec::new();
    for file in &read {
        let is_root = file.role == Role::CrateRoot;
        // A crate root that holds code is a module too, with a module's fields.
        if !is_root || file.items.code {
            let (value, more) = module_value(file, is_root, &library, input, out);
            modules.push(value);
            ports.extend(more);
        }
        if kind != CrateKind::Development {
            error_enums.extend(error_values(file, &error_types, out));
        }
    }
    check_crate_licence(krate, &manifest, &ports, out);
    let depends = if kind == CrateKind::Development {
        Vec::new()
    } else {
        depend_values(krate, &manifest, root, out)
    };
    entry.insert("depends".to_owned(), Value::Array(depends));
    entry.insert("errors".to_owned(), Value::Array(error_enums));
    entry.insert("folder".to_owned(), Value::from(krate.folder.as_str()));
    entry.insert("kind".to_owned(), Value::from(kind.name()));
    entry.insert("modules".to_owned(), Value::Array(modules));
    entry.insert("name".to_owned(), Value::from(krate.name.as_str()));
    Value::Object(entry)
}

/// One source's merged record while merging: its summary, the files using it and the licences
/// each gives it.
struct Merging {
    summary: BTreeMap<String, Value>,
    used_by: Vec<String>,
    licences: BTreeMap<String, Vec<String>>,
}

/// The text of `key` in an object, or empty.
fn text_of(object: &BTreeMap<String, Value>, key: &str) -> String {
    match object.get(key) {
        Some(Value::String(s)) => s.clone(),
        _ => String::new(),
    }
}

/// Adds `file`'s provenance `records` to the sources being merged, keyed by take and source.
fn merge(merged: &mut BTreeMap<(String, String), Merging>, file: &str, records: &[Value]) {
    for record in records {
        let Value::Object(r) = record else {
            continue;
        };
        let (take, link) = (text_of(r, "take"), text_of(r, "link"));
        let source = links::source_key(&link);
        let file = match r.get("manifest") {
            Some(Value::String(m)) => m.clone(),
            _ => file.to_owned(),
        };
        let licence = text_of(r, "licence");
        let canonical = Expr::parse(&licence).map_or_else(|_| licence.clone(), |e| e.canonical());
        let entry = merged
            .entry((take.clone(), source.clone()))
            .or_insert_with(|| {
                let mut summary = BTreeMap::new();
                summary.insert("licence".to_owned(), Value::from(licence.as_str()));
                summary.insert("link".to_owned(), Value::from(source.as_str()));
                summary.insert("name".to_owned(), Value::from(text_of(r, "name")));
                summary.insert(
                    "pinned".to_owned(),
                    r.get("pinned").cloned().unwrap_or(Value::Null),
                );
                summary.insert("take".to_owned(), Value::from(take.as_str()));
                summary.insert("method_note".to_owned(), Value::Null);
                Merging {
                    summary,
                    used_by: Vec::new(),
                    licences: BTreeMap::new(),
                }
            });
        if let Some(note @ Value::String(_)) = r.get("method_note") {
            entry
                .summary
                .entry("method_note".to_owned())
                .and_modify(|v| {
                    if *v == Value::Null {
                        *v = note.clone();
                    }
                });
        }
        entry.used_by.push(file.clone());
        entry.licences.entry(canonical).or_default().push(file);
    }
}

/// The sources of every record in the map, merged by take and source ([`links::source_key`]), and
/// a problem in each file of a source that two headers give different licences.
fn merged_sources(crates: &[Value]) -> (Vec<Value>, Vec<Problem>) {
    let mut merged: BTreeMap<(String, String), Merging> = BTreeMap::new();
    let mut visit = |file: &str, records: &[Value]| merge(&mut merged, file, records);
    for krate in crates {
        let Value::Object(c) = krate else {
            continue;
        };
        let file = text_of(c, "file");
        if let Some(Value::Array(records)) = c.get("provenance") {
            visit(&file, records);
        }
        if let Some(Value::Array(records)) = c.get("depends") {
            visit(&file, records);
        }
        if let Some(Value::Array(modules)) = c.get("modules") {
            for m in modules {
                if let Value::Object(m) = m
                    && let Some(Value::Array(records)) = m.get("provenance")
                {
                    visit(&text_of(m, "file"), records);
                }
            }
        }
    }
    let mut problems = Vec::new();
    let sources = merged
        .into_iter()
        .map(|((take, source), mut m)| {
            if m.licences.len() > 1 {
                let list: Vec<String> = m
                    .licences
                    .iter()
                    .map(|(licence, files)| format!("{licence} ({})", files.join(", ")))
                    .collect();
                let rule = match take.as_str() {
                    "port" => Rule::Source,
                    "depend" => Rule::Depend,
                    _ => Rule::Consulted,
                };
                let mut files: Vec<&String> = m.licences.values().flatten().collect();
                files.sort();
                files.dedup();
                for file in files {
                    problems.push(Problem {
                        file: file.clone(),
                        rule,
                        message: format!(
                            "{source} is credited under different licences: {}; 29.20 rule 1: \
                             where the licences a source states disagree, the strictest applies, \
                             so stop and ask an owner, then write the one licence everywhere",
                            list.join("; ")
                        ),
                    });
                }
            }
            m.used_by.sort();
            m.used_by.dedup();
            m.summary
                .insert("used_by".to_owned(), Value::from(m.used_by));
            Value::Object(m.summary)
        })
        .collect();
    (sources, problems)
}

/// Builds the map from `input`, with `gaps` as the accepted problems.
#[must_use]
pub fn build(input: &Input, gaps: &[Gap]) -> Generated {
    let mut collected = Collected {
        problems: Vec::new(),
    };
    for skipped in &input.unreadable {
        if let Err(why) = &skipped.text {
            collected.one(&skipped.path, Rule::Unreadable, why.clone());
        }
    }
    let mut crates: Vec<&CrateInput> = input.crates.iter().collect();
    crates.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.folder.cmp(&b.folder)));
    let crate_values: Vec<Value> = crates
        .iter()
        .map(|k| {
            let mut k = (*k).clone();
            k.files.sort_by(|a, b| a.path.cmp(&b.path));
            crate_value(&k, input, &mut collected)
        })
        .collect();
    let (sources, source_problems) = merged_sources(&crate_values);
    collected.problems.extend(source_problems);

    let mut problems = Vec::new();
    let mut used = vec![false; gaps.len()];
    collected.problems.sort();
    collected.problems.dedup();
    for p in collected.problems {
        let gap = gaps
            .iter()
            .position(|g| g.file == p.file && g.rule == p.rule && g.problem == p.message);
        match gap.and_then(|at| used.get_mut(at)) {
            Some(flag) => *flag = true,
            None => problems.push(p),
        }
    }
    for (gap, was_used) in gaps.iter().zip(&used) {
        if !was_used {
            problems.push(Problem {
                file: gap.file.to_owned(),
                rule: Rule::GapStale,
                message: format!(
                    "the known gap for {} (\"{}\") no longer occurs: delete it from map::GAPS (and \
                     from any check that records the same gap)",
                    gap.rule.label(),
                    gap.problem
                ),
            });
        }
    }
    problems.sort();
    problems.dedup();

    let gap_values = gaps
        .iter()
        .map(|g| {
            object([
                ("file", Value::from(g.file)),
                ("problem", Value::from(g.problem)),
                ("reason", Value::from(g.reason)),
                ("rule", Value::from(g.rule.label())),
            ])
        })
        .collect();
    let pending_values = input
        .pending
        .iter()
        .map(|p| {
            object([
                ("folder", Value::from(p.folder)),
                ("reason", Value::from(p.reason)),
            ])
        })
        .collect();
    let map = object([
        ("crates", Value::Array(crate_values)),
        ("format", Value::Number(FORMAT)),
        ("gaps", Value::Array(gap_values)),
        ("generated_by", Value::from(WRITE_COMMAND)),
        ("grammar", Value::from(GRAMMAR_FILE)),
        ("pending", Value::Array(pending_values)),
        ("sources", Value::Array(sources)),
    ]);
    Generated {
        text: map.to_text(),
        problems,
        map,
    }
}

/// The problems of comparing a committed map with the generated text: line ends are compared as
/// `\n` (a Windows checkout may turn them into `\r\n`), every other byte exactly.
#[must_use]
pub fn compare(committed: Option<&str>, generated: &str) -> Option<Problem> {
    let problem = |message: String| Problem {
        file: MAP_FILE.to_owned(),
        rule: Rule::Stale,
        message,
    };
    let Some(committed) = committed else {
        return Some(problem(format!(
            "the map is missing; run `{WRITE_COMMAND}` and commit it"
        )));
    };
    let committed = committed.replace("\r\n", "\n");
    if committed == generated {
        return None;
    }
    let line = committed
        .lines()
        .zip(generated.lines())
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| committed.lines().count().min(generated.lines().count()))
        .saturating_add(1);
    Some(problem(format!(
        "the committed map differs from the headers from line {line}; run `{WRITE_COMMAND}` and \
         commit the result"
    )))
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    use super::*;

    #[test]
    fn roles_follow_cargo_s_layout() {
        assert_eq!(role("geom-x", "lib.rs", true), Role::CrateRoot);
        assert_eq!(role("geom-x", "main.rs", false), Role::CrateRoot);
        assert_eq!(
            role("geom-x", "main.rs", true),
            Role::Binary("geom-x".into())
        );
        assert_eq!(
            role("geom-x", "a/b/mod.rs", true),
            Role::Module("geom_x::a::b".into())
        );
        assert_eq!(
            role("geom-x", "bin/map.rs", true),
            Role::Binary("map".into())
        );
        assert_eq!(
            role("geom-x", "bin/tool/main.rs", true),
            Role::Binary("tool".into())
        );
        assert_eq!(
            role("geom-x", "bin/tool/helper.rs", true),
            Role::BinModule("tool::helper".into())
        );
        assert_eq!(
            role("geom-x", "bin/tool/a/mod.rs", true),
            Role::BinModule("tool::a".into())
        );
    }

    #[test]
    fn manifests_name_their_licence_and_published_dependencies() {
        let krate = CrateInput {
            name: "geom-x".into(),
            folder: format!("{CRATES_DIR}/geom-x"),
            manifest: Some(
                "[package]\nname = \"geom-x\"\nlicense = \"MPL-2.0 AND BSD-3-Clause\"\n\n\
                 [dependencies]\n# A comment = no\ngeom-num = { path = \"../geom-num\" }\n\
                 libm = \"0.2.16\"\nfoo = { version = \"1.2.3\", default-features = false }\n\
                 bar.workspace = true\n\n[dev-dependencies]\nproptest = \"1\"\n\n\
                 [target.'cfg(unix)'.build-dependencies]\ncc = \"1.0.0\"\n\n\
                 [dependencies.baz]\nversion = \"0.1.0\"\n\n[dependencies.local]\npath = \"x\"\n"
                    .into(),
            ),
            files: Vec::new(),
        };
        let (manifest, problems) = check_manifest(&krate);
        assert_eq!(problems, []);
        assert_eq!(
            manifest.licence.as_deref(),
            Some("MPL-2.0 AND BSD-3-Clause")
        );
        let names: Vec<(&str, Option<&str>)> = manifest
            .dependencies
            .iter()
            .map(|d| (d.name.as_str(), d.version.as_deref()))
            .collect();
        assert_eq!(
            names,
            [
                ("libm", Some("0.2.16")),
                ("foo", Some("1.2.3")),
                ("bar", None),
                ("cc", Some("1.0.0")),
                ("baz", Some("0.1.0")),
            ]
        );
    }

    #[test]
    fn kinds() {
        assert_eq!(
            crate_kind("geom-num", &format!("{CRATES_DIR}/geom-num")),
            CrateKind::Kernel
        );
        assert_eq!(
            crate_kind("geom-test", &format!("{CRATES_DIR}/geom-test")),
            CrateKind::Development
        );
        assert_eq!(
            crate_kind("engine-geometry", &format!("{FACADE_DIR}/engine-geometry")),
            CrateKind::Facade
        );
        assert!(is_facade("engine-geometry-wasm"));
        assert!(!is_facade("engine-geometryx"));
        assert!(!is_facade("engine-draw2d"));
    }
}
