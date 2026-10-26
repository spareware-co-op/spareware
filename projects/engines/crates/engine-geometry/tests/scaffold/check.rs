//! The checks: the crate table against itself, and the repository against the table.
//!
//! What it does: `table` proves the table consistent: every name known, every kind in a layer it
//! may have, every edge pointing to a lower layer or along an edge the layer table allows inside
//! one layer, no edge to a development crate, no cycle, every pending entry, feature gate, header
//! gap and registry entry pointing at a real edge or crate, and the lint posture read from a
//! kernel crate. `workspace` proves an observed workspace matches the table: which crates exist and
//! where, their headers, `no_std` and `forbid(unsafe_code)`, exactly their direct kernel
//! dependencies as normal path dependencies (less the ones still pending), optional only behind
//! their feature, that feature off by default and the only one that switches the dependency on, no
//! registry crate in a `no_std` kernel crate but the ones the table allows, no cycle and no edge
//! against the layer rule, nothing outside the kernel depending on a `geom-*` crate, and every
//! crate that ships built with exactly `geom-num`'s lint settings (`lints`, which states that rule
//! in full).
//!
//! What it guarantees: both are pure functions of their inputs and return every violation, sorted,
//! never only the first, so one run shows everything to fix. An empty list means the repository
//! is exactly what chapter 27's table, as encoded in `table`, says.
//!
//! Method: set comparison over ordered maps, the layer rule of chapter 27 ("a crate depends only
//! on crates in lower layers, or on a crate in its own layer along an edge this table allows"),
//! and Kahn's topological sort for cycles (Kahn 1962, <https://doi.org/10.1145/368996.369025>),
//! with an explicit walk to name one cycle; cargo's feature rules for what switches an optional
//! dependency on (<https://doc.rust-lang.org/cargo/reference/features.html>), walked with an
//! explicit work list. No recursion.
//!
//! Cost: O(crates + edges + features) set operations, each O(log n).

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::header::{FIELDS, Field, Header, MARK_27, MARK_30, STATUS_PREFIX, pending_line};
use crate::lints::{self, Lints};
use crate::source::Attributes;
use crate::table::{Deps, Kind, Layer, Origin, Row, State, Table, expand, full_name};

/// Which rule a violation breaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rule {
    // The table against itself.
    /// Two rows share a name.
    DuplicateRow,
    /// A row's kind, layer, name, state or dependency form do not fit together.
    BadRow,
    /// A row names a crate the table does not have.
    UnknownDependency,
    /// A row names itself or names a crate twice.
    BadDependencyList,
    /// A normal dependency on a development crate.
    DevelopmentDependency,
    /// An edge to a higher layer.
    LayerOrder,
    /// An edge inside one layer that the layer table does not allow.
    SameLayerNotAllowed,
    /// A same-layer entry names an unknown crate or crates of different layers.
    BadSameLayerEntry,
    /// The dependency graph has a cycle.
    Cycle,
    /// A pending entry does not match a row's edge to a crate under way.
    BadPending,
    /// A feature gate does not match a row's edge.
    BadFeatureGate,
    /// A header gap names a crate this table does not leave to another piece.
    BadHeaderGap,
    /// A registry entry names a crate that is not a `no_std` kernel crate.
    BadRegistryEntry,
    /// The crate whose lint settings every crate that ships copies is not a kernel crate's row.
    BadLintSource,
    // The workspace against the table.
    /// A crate of the kernel's family that the table does not have.
    UnknownCrate,
    /// A folder under `geometry/crates` that is not a workspace crate.
    OrphanFolder,
    /// A crate that is not in its folder, or whose folder is not named after it.
    WrongFolder,
    /// A crate this piece scaffolds is missing.
    MissingCrate,
    /// A deferred crate exists before its row says it may.
    DeferredPresent,
    /// A crate has no library target at `src/lib.rs`.
    NoLibrary,
    /// The header does not state one of the four fields.
    HeaderMissing(Field),
    /// The header's layer differs from the table's.
    HeaderWrongLayer,
    /// A header gap is listed for a field the header now states.
    HeaderGapStale,
    /// A scaffolded crate lacks its status line.
    StatusMissing,
    /// A scaffolded crate lacks the line for one of its pending dependencies.
    PendingLineMissing,
    /// A header names a pending dependency the table does not list.
    PendingLineStale,
    /// A proposed crate lacks its mark, or an accepted crate carries one.
    OriginMark,
    /// A kernel crate is not `no_std`.
    NotNoStd,
    /// A crate that must forbid unsafe code does not.
    UnsafeNotForbidden,
    /// A dependency of the row is missing from `Cargo.toml`.
    MissingDependency,
    /// `Cargo.toml` names a kernel crate the row does not.
    ExtraDependency,
    /// A kernel dependency that is not a path dependency.
    NotAPathDependency,
    /// A kernel crate named as a build dependency: the table's edges are normal dependencies.
    BuildDependency,
    /// An optional kernel dependency that no feature gate of the table puts behind a feature.
    OptionalDependency,
    /// A `no_std` kernel crate names a registry or git crate the table does not allow (29.1).
    RegistryDependency,
    /// A kernel crate depends on a workspace crate outside the kernel.
    ExternalDependency,
    /// A pending dependency's crate has merged, so the entry must go.
    PendingMerged,
    /// A pending dependency is already in `Cargo.toml`.
    PendingAlreadyAdded,
    /// A dependency's crate is absent and not listed as pending.
    AbsentDependency,
    /// A gated dependency is not optional, its feature does not switch it on, another feature
    /// does, or `default` switches its feature on.
    FeatureGate,
    /// A crate outside the kernel depends on an internal `geom-*` crate.
    OutsiderDependency,
    // The lint posture (`lints`).
    /// A crate that ships does not set exactly `geom-num`'s keys under `lints` in its manifest, or
    /// the manifest that holds them cannot be read.
    LintTables,
    /// The clippy configuration a crate that ships is built with does not hold exactly the
    /// settings of `geom-num`'s, or there is none, or it cannot be read.
    ClippyConfig,
    // The toolchain.
    /// `rust-toolchain.toml` and a workflow's `RUST_TOOLCHAIN` do not name one exact release.
    ToolchainPin,
    // The merge gate (G12): `globals`, `overrides`, `posture`, `deny` and `gate`.
    /// A crate that ships names global state: a `static`, a thread-local or a lazy global
    /// (principle 7, 28.8), or a source of it cannot be read.
    GlobalState,
    /// A crate that ships lacks one of chapter 21's disallowed methods or types, or one of chapter
    /// 27's denials, in the lint settings it is built with, or those settings lower one.
    PostureLacks,
    /// A crate that ships allows, warns on or expects a lint the kernel denies, outside test code,
    /// in a lint attribute of its source, or such an attribute cannot be read.
    LintOverride,
    /// A cargo configuration file sits in the repository on the way to a crate of the family.
    CargoConfig,
    /// A cargo-deny configuration of the kernel says something other than the gate requires
    /// (29.2's allow-list, advisories, allowed sources, the dependency bans).
    DenyConfig,
    /// The merge gate's workflow does not run, pin or require what the gate needs.
    GateWorkflow,
}

/// One broken rule, on one crate.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Violation {
    /// The crate concerned.
    pub krate: String,
    /// The rule broken.
    pub rule: Rule,
    /// What exactly, and what to do.
    pub detail: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {:?}: {}", self.krate, self.rule, self.detail)
    }
}

/// How a dependency is declared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepKind {
    /// `[dependencies]`.
    Normal,
    /// `[build-dependencies]`.
    Build,
    /// `[dev-dependencies]`.
    Dev,
}

/// One dependency of a workspace crate, as `cargo metadata` reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dep {
    /// The package it names.
    pub name: String,
    /// Which table declares it.
    pub kind: DepKind,
    /// Whether it is a path dependency.
    pub path: bool,
    /// Whether it is optional.
    pub optional: bool,
    /// Where it comes from when it is not a path dependency, as `cargo metadata` writes it
    /// (`registry+https://github.com/rust-lang/crates.io-index`, `git+...`).
    pub source: Option<String>,
}

/// The source `cargo metadata` gives crates.io, whichever protocol fetched it.
pub const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";

/// What the check sees of one workspace crate.
#[derive(Clone, Debug, PartialEq)]
pub struct Package {
    /// Its directory, relative to the workspace root, one component per entry.
    pub dir: Vec<String>,
    /// Its library target's source, relative to its directory, if it has one.
    pub lib: Option<Vec<String>>,
    /// The header of its library's source.
    pub header: Header,
    /// The attributes of its library's source.
    pub attributes: Attributes,
    /// Whether its manifest's lint table forbids unsafe code.
    pub manifest_forbids_unsafe: bool,
    /// Its lint settings: its manifest's `lints` keys and the clippy configuration it is built with.
    pub lints: Lints,
    /// Its dependencies of every kind.
    pub deps: Vec<Dep>,
    /// Its features and what each switches on.
    pub features: BTreeMap<String, Vec<String>>,
}

/// What the check sees of the whole workspace.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Observed {
    /// Every workspace crate, by name.
    pub packages: BTreeMap<String, Package>,
    /// The names of the folders under `projects/engines/geometry/crates`.
    pub geometry_dirs: BTreeSet<String>,
}

/// The folder of the kernel's internal crates, relative to the workspace root.
pub const GEOMETRY_CRATES: [&str; 4] = ["projects", "engines", "geometry", "crates"];

/// The folder of the facade and the binding crates, relative to the workspace root.
pub const ENGINE_CRATES: [&str; 3] = ["projects", "engines", "crates"];

/// Where a crate of the family lives: `engine-*` crates beside the other engines, every other one
/// under `geometry/crates`; the folder is named after the crate (chapter 27).
pub fn expected_dir(name: &str) -> Vec<String> {
    let parent: &[&str] = if name.starts_with("engine-") {
        &ENGINE_CRATES
    } else {
        &GEOMETRY_CRATES
    };
    parent
        .iter()
        .map(|s| (*s).to_owned())
        .chain([name.to_owned()])
        .collect()
}

/// Whether a workspace crate belongs to the kernel's family.
fn in_family(name: &str, package: &Package) -> bool {
    name.starts_with("geom-")
        || name.starts_with("engine-geometry")
        || package.dir.starts_with(&GEOMETRY_CRATES.map(str::to_owned))
}

fn violation(krate: &str, rule: Rule, detail: String) -> Violation {
    Violation {
        krate: krate.to_owned(),
        rule,
        detail,
    }
}

fn rows_by_name<'t>(table: &'t Table<'t>) -> BTreeMap<&'t str, &'t Row<'t>> {
    table.rows.iter().map(|row| (row.name, row)).collect()
}

/// The direct dependencies a row lists, in full names (none for a development crate).
fn row_deps(row: &Row<'_>) -> Vec<String> {
    match row.deps {
        Deps::Exact(list) | Deps::Partial(list) => expand(list),
        Deps::AsNeeded => Vec::new(),
    }
}

/// The edges the layer table allows inside one layer, in full names.
fn same_layer_edges(table: &Table<'_>) -> BTreeSet<(String, String)> {
    table
        .same_layer
        .iter()
        .flat_map(|(from, to)| expand(to).into_iter().map(move |t| (full_name(from), t)))
        .collect()
}

/// Every (crate, dependency, piece) the pending list holds, in full names.
fn pending_edges(table: &Table<'_>) -> BTreeMap<(String, String), String> {
    let mut edges = BTreeMap::new();
    for pending in table.pending {
        for krate in expand(pending.crates) {
            edges.insert((krate, full_name(pending.dep)), pending.piece.to_owned());
        }
    }
    edges
}

/// Checks one edge against the layer rule; `None` when it keeps it.
fn layer_rule(
    from: &Row<'_>,
    to: &Row<'_>,
    same_layer: &BTreeSet<(String, String)>,
) -> Option<(Rule, String)> {
    match (from.layer, to.layer) {
        (_, Layer::Development) if from.kind != Kind::Development => Some((
            Rule::DevelopmentDependency,
            format!(
                "{} is a development crate; only development dependencies may name it",
                to.name
            ),
        )),
        (Layer::N(a), Layer::N(b)) if b > a => Some((
            Rule::LayerOrder,
            format!("{} is in layer {b}, above layer {a}", to.name),
        )),
        (Layer::N(a), Layer::N(b))
            if a == b && !same_layer.contains(&(from.name.to_owned(), to.name.to_owned())) =>
        {
            Some((
                Rule::SameLayerNotAllowed,
                format!(
                    "{} is in the same layer ({a}) and the layer table allows no edge to it",
                    to.name
                ),
            ))
        }
        _ => None,
    }
}

/// Finds the crates on or above a cycle and names one cycle, or `None` if the graph is acyclic.
fn find_cycle(graph: &BTreeMap<String, BTreeSet<String>>) -> Option<Vec<String>> {
    // Kahn's algorithm, leaves first: repeatedly remove crates with no remaining dependency.
    let mut remaining: BTreeMap<&str, BTreeSet<&str>> = graph
        .iter()
        .map(|(node, deps)| {
            let deps = deps
                .iter()
                .filter(|d| graph.contains_key(*d))
                .map(String::as_str)
                .collect();
            (node.as_str(), deps)
        })
        .collect();
    loop {
        let leaves: Vec<&str> = remaining
            .iter()
            .filter(|(_, deps)| deps.is_empty())
            .map(|(node, _)| *node)
            .collect();
        if leaves.is_empty() {
            break;
        }
        for leaf in &leaves {
            remaining.remove(leaf);
        }
        for deps in remaining.values_mut() {
            for leaf in &leaves {
                deps.remove(leaf);
            }
        }
    }
    // Every crate left has a dependency left, so walking first dependencies must repeat a crate
    // within remaining.len() steps; the repeat closes a cycle.
    let start = *remaining.keys().next()?;
    let mut path = vec![start];
    let mut node = start;
    for _ in 0..=remaining.len() {
        node = *remaining[node].iter().next()?;
        if let Some(at) = path.iter().position(|n| *n == node) {
            let mut cycle: Vec<String> = path[at..].iter().map(|n| (*n).to_owned()).collect();
            cycle.push(node.to_owned());
            return Some(cycle);
        }
        path.push(node);
    }
    None
}

/// Checks one row against the rest of the table: its kind, layer and name fit together, its
/// dependency form fits its state, and every dependency is a known crate, named once, that the
/// layer rule allows. Returns the row's dependencies, for the cycle check.
fn table_row(
    row: &Row<'_>,
    rows: &BTreeMap<&str, &Row<'_>>,
    same_layer: &BTreeSet<(String, String)>,
    add: &mut impl FnMut(&str, Rule, String),
) -> BTreeSet<String> {
    let shape_ok = match row.kind {
        Kind::Kernel => matches!(row.layer, Layer::N(1..=12)) && row.name.starts_with("geom-"),
        Kind::KernelStd => row.layer == Layer::N(13) && row.name.starts_with("geom-"),
        Kind::Facade | Kind::Binding => {
            row.layer == Layer::N(13) && row.name.starts_with("engine-geometry")
        }
        Kind::Development => {
            row.layer == Layer::Development
                && row.name.starts_with("geom-")
                && row.deps == Deps::AsNeeded
        }
    };
    if !shape_ok {
        add(
            row.name,
            Rule::BadRow,
            format!(
                "kind {:?} does not fit layer {:?} and this name",
                row.kind, row.layer
            ),
        );
    }
    if row.deps == Deps::AsNeeded && row.kind != Kind::Development {
        add(
            row.name,
            Rule::BadRow,
            "only development crates take AsNeeded".to_owned(),
        );
    }
    if matches!(row.deps, Deps::Partial(_)) && !matches!(row.state, State::Deferred(_)) {
        add(
            row.name,
            Rule::BadRow,
            "a partial dependency list must be settled before the crate may exist".to_owned(),
        );
    }
    if !row.external.is_empty() && row.kind != Kind::Facade {
        add(
            row.name,
            Rule::BadRow,
            "only the facade may depend on crates outside the kernel".to_owned(),
        );
    }

    let mut seen = BTreeSet::new();
    for dep in row_deps(row) {
        if dep == row.name {
            add(
                row.name,
                Rule::BadDependencyList,
                "it names itself".to_owned(),
            );
        }
        if !seen.insert(dep.clone()) {
            add(
                row.name,
                Rule::BadDependencyList,
                format!("{dep} is named twice"),
            );
        }
        let Some(target) = rows.get(dep.as_str()) else {
            add(
                row.name,
                Rule::UnknownDependency,
                format!("{dep} has no row"),
            );
            continue;
        };
        if let Some((rule, detail)) = layer_rule(row, target, same_layer) {
            add(row.name, rule, detail);
        }
    }
    seen
}

/// Checks the table's lists against its rows: every pending entry, feature gate, registry entry
/// and header gap points at a real edge or crate of the kind it needs.
fn table_lists(
    table: &Table<'_>,
    rows: &BTreeMap<&str, &Row<'_>>,
    add: &mut impl FnMut(&str, Rule, String),
) {
    for pending in table.pending {
        let dep = full_name(pending.dep);
        match rows.get(dep.as_str()) {
            Some(row) if row.state == State::Elsewhere(pending.piece) => {}
            _ => add(
                &dep,
                Rule::BadPending,
                format!("a pending crate must be a row written by {}", pending.piece),
            ),
        }
        let mut seen = BTreeSet::new();
        for krate in expand(pending.crates) {
            if !seen.insert(krate.clone()) {
                add(
                    &krate,
                    Rule::BadPending,
                    format!("listed twice as waiting for {dep}"),
                );
            }
            let lists_it = rows
                .get(krate.as_str())
                .is_some_and(|row| row_deps(row).contains(&dep));
            if !lists_it {
                add(
                    &krate,
                    Rule::BadPending,
                    format!("its row does not list {dep}"),
                );
            }
        }
    }

    for gate in table.feature_gates {
        let (krate, dep) = (full_name(gate.krate), full_name(gate.dep));
        let lists_it = rows
            .get(krate.as_str())
            .is_some_and(|row| row_deps(row).contains(&dep));
        if !lists_it || gate.feature.is_empty() || gate.feature == "default" {
            add(
                &krate,
                Rule::BadFeatureGate,
                format!("its row must list {dep}, behind a feature other than `default`"),
            );
        }
    }

    for (krate, registry_crate) in table.registry {
        let krate = full_name(krate);
        let kernel = rows
            .get(krate.as_str())
            .is_some_and(|row| row.kind == Kind::Kernel);
        if !kernel || registry_crate.is_empty() || rows.contains_key(registry_crate) {
            add(
                &krate,
                Rule::BadRegistryEntry,
                format!("{registry_crate} may be allowed only to a no_std kernel crate"),
            );
        }
    }

    for gap in table.header_gaps {
        let krate = full_name(gap.krate);
        let elsewhere = rows
            .get(krate.as_str())
            .is_some_and(|row| matches!(row.state, State::Elsewhere(_)));
        if !elsewhere || gap.reason.is_empty() {
            add(
                &krate,
                Rule::BadHeaderGap,
                "only a crate another piece writes may have a header gap, with its reason"
                    .to_owned(),
            );
        }
    }
}

/// Proves the table consistent.
pub fn table(table: &Table<'_>) -> Vec<Violation> {
    let mut found = BTreeSet::new();
    let mut add = |krate: &str, rule: Rule, detail: String| {
        found.insert(violation(krate, rule, detail));
    };
    let mut rows: BTreeMap<&str, &Row<'_>> = BTreeMap::new();
    for row in table.rows {
        if rows.insert(row.name, row).is_some() {
            add(
                row.name,
                Rule::DuplicateRow,
                "two rows share this name".to_owned(),
            );
        }
    }
    let same_layer = same_layer_edges(table);
    let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for row in table.rows {
        let deps = table_row(row, &rows, &same_layer, &mut add);
        graph.insert(row.name.to_owned(), deps);
    }

    for (from, to) in &same_layer {
        match (rows.get(from.as_str()), rows.get(to.as_str())) {
            (Some(a), Some(b)) if a.layer == b.layer => {}
            _ => add(
                from,
                Rule::BadSameLayerEntry,
                format!("{from} and {to} are not two known crates of one layer"),
            ),
        }
    }

    if let Some(cycle) = find_cycle(&graph) {
        add(
            &cycle[0],
            Rule::Cycle,
            format!("the table's graph has a cycle: {}", cycle.join(" -> ")),
        );
    }

    table_lists(table, &rows, &mut add);
    lints::table(table, &rows, &mut add);
    found.into_iter().collect()
}

/// Checks the workspace's crates as a family: every crate of the family has a row, every folder
/// under `geometry/crates` holds a crate, and no crate outside the family names a `geom-*` crate.
fn family(
    rows: &BTreeMap<&str, &Row<'_>>,
    observed: &Observed,
    add: &mut impl FnMut(&str, Rule, String),
) {
    for (name, package) in &observed.packages {
        if in_family(name, package) && !rows.contains_key(name.as_str()) {
            add(
                name,
                Rule::UnknownCrate,
                "a crate of the kernel's family must have a row in the crate table".to_owned(),
            );
        }
    }
    for dir in &observed.geometry_dirs {
        let mut path: Vec<String> = GEOMETRY_CRATES.map(str::to_owned).to_vec();
        path.push(dir.clone());
        if !observed.packages.values().any(|p| p.dir == path) {
            add(
                dir,
                Rule::OrphanFolder,
                "this folder holds no workspace crate".to_owned(),
            );
        }
    }
    for (name, package) in &observed.packages {
        if in_family(name, package) {
            continue;
        }
        for dep in &package.deps {
            if dep.name.starts_with("geom-") {
                add(
                    name,
                    Rule::OutsiderDependency,
                    format!(
                        "depends on {}: every geom-* crate is internal (25); use engine-geometry",
                        dep.name
                    ),
                );
            }
        }
    }
}

/// Checks where a present crate is and what it is: not deferred, in its own folder, with its
/// library at `src/lib.rs`.
fn placement(row: &Row<'_>, package: &Package, add: &mut impl FnMut(&str, Rule, String)) {
    if let State::Deferred(piece) = row.state {
        add(
            row.name,
            Rule::DeferredPresent,
            format!("{piece} builds it; change its row to Here, with exact dependencies, first"),
        );
    }
    if package.dir != expected_dir(row.name) {
        add(
            row.name,
            Rule::WrongFolder,
            format!(
                "it is in {}, not {}",
                package.dir.join("/"),
                expected_dir(row.name).join("/")
            ),
        );
    }
    if package.lib.as_deref() != Some(&["src".to_owned(), "lib.rs".to_owned()][..]) {
        add(
            row.name,
            Rule::NoLibrary,
            "its library must be src/lib.rs".to_owned(),
        );
    }
}

/// Reads a present crate's declared dependencies against the table, reporting each that is not a
/// path dependency, is a build dependency, is optional without a gate, lies outside the kernel, or
/// comes from a registry the table does not allow. Returns its normal kernel dependencies, which
/// its row is compared with, and every kernel edge, build dependencies included, for the layer
/// rule and the cycle check.
fn declared_dependencies(
    row: &Row<'_>,
    package: &Package,
    table: &Table<'_>,
    rows: &BTreeMap<&str, &Row<'_>>,
    add: &mut impl FnMut(&str, Rule, String),
) -> (BTreeSet<String>, BTreeSet<String>) {
    let gated: BTreeSet<String> = table
        .feature_gates
        .iter()
        .filter(|g| full_name(g.krate) == row.name)
        .map(|g| full_name(g.dep))
        .collect();
    let registry_allowed: BTreeSet<&str> = table
        .registry
        .iter()
        .filter(|(krate, _)| full_name(krate) == row.name)
        .map(|(_, registry_crate)| *registry_crate)
        .collect();
    let mut kernel_deps = BTreeSet::new();
    let mut edges = BTreeSet::new();
    for dep in package.deps.iter().filter(|d| d.kind != DepKind::Dev) {
        match rows.get(dep.name.as_str()) {
            Some(_) => {
                if !dep.path {
                    add(
                        row.name,
                        Rule::NotAPathDependency,
                        format!("{} must be a path dependency", dep.name),
                    );
                }
                if dep.kind == DepKind::Build {
                    add(
                        row.name,
                        Rule::BuildDependency,
                        format!(
                            "{} is a build dependency; a kernel crate is only ever a normal \
                             dependency (27, \"What Depends on means\")",
                            dep.name
                        ),
                    );
                } else {
                    if dep.optional && !gated.contains(&dep.name) {
                        add(
                            row.name,
                            Rule::OptionalDependency,
                            format!(
                                "{} is optional, but no feature gate of the table puts it \
                                 behind a feature",
                                dep.name
                            ),
                        );
                    }
                    kernel_deps.insert(dep.name.clone());
                }
                // An edge to a development crate is caught by the layer rule.
                edges.insert(dep.name.clone());
            }
            None if dep.path
                && row.kind != Kind::Development
                && !row.external.split_whitespace().any(|e| e == dep.name) =>
            {
                add(
                    row.name,
                    Rule::ExternalDependency,
                    format!("{} is a workspace crate outside the kernel", dep.name),
                );
            }
            None if !dep.path && row.kind == Kind::Kernel => {
                let allowed = dep.kind == DepKind::Normal
                    && dep.source.as_deref() == Some(CRATES_IO)
                    && registry_allowed.contains(dep.name.as_str());
                if !allowed {
                    add(
                        row.name,
                        Rule::RegistryDependency,
                        format!(
                            "{} ({:?} dependency from {}): inside kernel crates the only \
                             published crate is libm, and only geom-num names it (29.1, 21)",
                            dep.name,
                            dep.kind,
                            dep.source.as_deref().unwrap_or("an unknown source")
                        ),
                    );
                }
            }
            None => {}
        }
    }
    (kernel_deps, edges)
}

/// Compares a present crate's normal kernel dependencies with its row, less the ones still
/// pending. A development crate's row lists none, so it has nothing to compare.
fn row_against_manifest(
    row: &Row<'_>,
    kernel_deps: &BTreeSet<String>,
    pending: &BTreeMap<(String, String), String>,
    observed: &Observed,
    add: &mut impl FnMut(&str, Rule, String),
) {
    if row.deps == Deps::AsNeeded {
        return;
    }
    let row_list = row_deps(row);
    for dep in &row_list {
        let waiting = pending.get(&(row.name.to_owned(), dep.clone()));
        let declared = kernel_deps.contains(dep);
        match (waiting, declared) {
            (Some(piece), true) => add(
                row.name,
                Rule::PendingAlreadyAdded,
                format!("{dep} is declared: delete its pending entry ({piece}) and header line"),
            ),
            (None, false) if observed.packages.contains_key(dep) => add(
                row.name,
                Rule::MissingDependency,
                format!("its row lists {dep}; add it to Cargo.toml as a path dependency"),
            ),
            (None, false) => add(
                row.name,
                Rule::AbsentDependency,
                format!("{dep} is not in the workspace and not listed as pending"),
            ),
            _ => {}
        }
    }
    for dep in kernel_deps {
        if !row_list.contains(dep) {
            add(
                row.name,
                Rule::ExtraDependency,
                format!("its row does not list {dep}: change chapter 27 and the table first"),
            );
        }
    }
}

/// Checks every dependency the table puts behind a feature of a present crate.
fn feature_gates(
    row: &Row<'_>,
    package: &Package,
    table: &Table<'_>,
    add: &mut impl FnMut(&str, Rule, String),
) {
    for gate in table
        .feature_gates
        .iter()
        .filter(|g| full_name(g.krate) == row.name)
    {
        let dep_name = full_name(gate.dep);
        let declared: Vec<&Dep> = package
            .deps
            .iter()
            .filter(|d| d.name == dep_name && d.kind == DepKind::Normal)
            .collect();
        if declared.is_empty() {
            continue;
        }
        let wrong = gate_faults(package, &declared, &dep_name, gate.feature);
        if !wrong.is_empty() {
            add(
                row.name,
                Rule::FeatureGate,
                format!(
                    "{dep_name} must be optional, switched on by `{}` alone and off by default \
                     (23.4, 27): {}",
                    gate.feature,
                    wrong.join("; ")
                ),
            );
        }
    }
}

/// Proves an observed workspace matches the table.
pub fn workspace(table: &Table<'_>, observed: &Observed) -> Vec<Violation> {
    let mut found = BTreeSet::new();
    let mut add = |krate: &str, rule: Rule, detail: String| {
        found.insert(violation(krate, rule, detail));
    };
    let rows = rows_by_name(table);
    let same_layer = same_layer_edges(table);
    let pending = pending_edges(table);
    family(&rows, observed, &mut add);

    let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for row in table.rows {
        let Some(package) = observed.packages.get(row.name) else {
            if row.state == State::Here {
                add(
                    row.name,
                    Rule::MissingCrate,
                    "this piece scaffolds it".to_owned(),
                );
            }
            continue;
        };
        placement(row, package, &mut add);
        check_header(row, package, table, &pending, &mut add);
        check_attributes(row, package, &mut add);
        let (kernel_deps, edges) = declared_dependencies(row, package, table, &rows, &mut add);
        row_against_manifest(row, &kernel_deps, &pending, observed, &mut add);
        feature_gates(row, package, table, &mut add);
        graph.insert(row.name.to_owned(), edges);
    }

    for ((krate, dep), piece) in &pending {
        if observed.packages.contains_key(dep) {
            add(
                krate,
                Rule::PendingMerged,
                format!(
                    "{dep} has merged ({piece}): add it to Cargo.toml, then delete the pending \
                     entry and the header line"
                ),
            );
        }
    }

    for (from, deps) in &graph {
        let Some(from_row) = rows.get(from.as_str()) else {
            continue;
        };
        for to in deps {
            let Some(to_row) = rows.get(to.as_str()) else {
                continue;
            };
            if let Some((rule, detail)) = layer_rule(from_row, to_row, &same_layer) {
                add(from, rule, detail);
            }
        }
    }
    if let Some(cycle) = find_cycle(&graph) {
        add(
            &cycle[0],
            Rule::Cycle,
            format!("the workspace has a cycle: {}", cycle.join(" -> ")),
        );
    }
    lints::workspace(table, observed, &mut add);
    found.into_iter().collect()
}

/// Whether one entry of a feature's list switches the dependency `dep` on: `dep:name`, `name`
/// (the dependency's implicit feature) or `name/feature`. The weak form `name?/feature` does not.
fn switches_on(entry: &str, dep: &str) -> bool {
    entry
        .strip_prefix("dep:")
        .unwrap_or(entry)
        .split('/')
        .next()
        .is_some_and(|name| name == dep)
}

/// Whether switching on feature `from` switches on feature `to`, through any chain of features.
/// A walk over at most every feature once, so it ends on any input, cycles included.
fn feature_reaches(features: &BTreeMap<String, Vec<String>>, from: &str, to: &str) -> bool {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut queue: Vec<&str> = vec![from];
    while let Some(feature) = queue.pop() {
        if feature == to {
            return true;
        }
        if !seen.insert(feature) {
            continue;
        }
        for entry in features.get(feature).into_iter().flatten() {
            // `dep:name` names a dependency only; `name/feature` and `name?/feature` switch on
            // feature `name` of this crate when it has one (the strong form does).
            if entry.starts_with("dep:") || entry.contains("?/") {
                continue;
            }
            let name = entry.split('/').next().unwrap_or(entry);
            if features.contains_key(name) {
                queue.push(name);
            }
        }
    }
    false
}

/// What is wrong with a dependency the table puts behind `feature`, if anything.
fn gate_faults(package: &Package, declared: &[&Dep], dep: &str, feature: &str) -> Vec<String> {
    let mut wrong = Vec::new();
    if declared.iter().any(|d| !d.optional) {
        wrong.push("it is declared without `optional = true`".to_owned());
    }
    let switched = |f: &str| {
        package
            .features
            .get(f)
            .is_some_and(|on| on.iter().any(|entry| switches_on(entry, dep)))
    };
    if !switched(feature) {
        wrong.push(format!("`{feature}` does not list `dep:{dep}`"));
    }
    for other in package.features.keys().filter(|f| *f != feature) {
        if switched(other) {
            wrong.push(format!("`{other}` switches it on too"));
        }
    }
    if feature_reaches(&package.features, "default", feature) {
        wrong.push(format!(
            "`default` switches `{feature}` on, so every default build links it"
        ));
    }
    wrong
}

fn check_header(
    row: &Row<'_>,
    package: &Package,
    table: &Table<'_>,
    pending: &BTreeMap<(String, String), String>,
    add: &mut impl FnMut(&str, Rule, String),
) {
    let header = &package.header;
    let gaps: BTreeSet<Field> = table
        .header_gaps
        .iter()
        .filter(|g| full_name(g.krate) == row.name)
        .map(|g| g.field)
        .collect();
    for field in FIELDS {
        match (header.has(field), gaps.contains(&field)) {
            (false, false) => add(
                row.name,
                Rule::HeaderMissing(field),
                format!("the lib.rs header must state its {field:?} (chapter 27)"),
            ),
            (true, true) => add(
                row.name,
                Rule::HeaderGapStale,
                format!("the header now states its {field:?}: delete the header gap"),
            ),
            _ => {}
        }
    }
    if let Some(layer) = header.layer()
        && layer != row.layer
    {
        add(
            row.name,
            Rule::HeaderWrongLayer,
            format!("the header says {layer:?}, the table {:?}", row.layer),
        );
    }
    let waiting: Vec<(&String, &String)> = pending
        .iter()
        .filter(|((krate, _), _)| krate == row.name)
        .map(|((_, dep), piece)| (dep, piece))
        .collect();
    for dep in header.pending_deps() {
        if !waiting.iter().any(|(d, _)| **d == dep) {
            add(
                row.name,
                Rule::PendingLineStale,
                format!("the header says {dep} is still to add, but nothing is pending on it"),
            );
        }
    }
    if row.state != State::Here {
        return;
    }
    if !header.has_line_starting(STATUS_PREFIX) {
        add(
            row.name,
            Rule::StatusMissing,
            format!("the header needs \"{STATUS_PREFIX}...\""),
        );
    }
    for (dep, piece) in waiting {
        if !header.has_line_starting(&pending_line(dep, piece)) {
            add(
                row.name,
                Rule::PendingLineMissing,
                format!("the header needs \"{}.\"", pending_line(dep, piece)),
            );
        }
    }
    let marks = (header.contains(MARK_27), header.contains(MARK_30));
    let expected = match row.origin {
        Origin::Accepted => (false, false),
        Origin::Proposed27 => (true, false),
        Origin::Proposed30 => (false, true),
    };
    if marks != expected {
        add(
            row.name,
            Rule::OriginMark,
            format!(
                "its row is {:?}: the header's proposal mark must say so",
                row.origin
            ),
        );
    }
}

fn check_attributes(row: &Row<'_>, package: &Package, add: &mut impl FnMut(&str, Rule, String)) {
    let forbids = package.attributes.forbids_unsafe || package.manifest_forbids_unsafe;
    if row.kind == Kind::Kernel && !package.attributes.no_std {
        add(
            row.name,
            Rule::NotNoStd,
            concat!(
                "kernel crates are no_std (21, 27): #![cfg_attr(not(test), no_std)] among the ",
                "crate root's leading attributes, and no `extern crate std` in any source file"
            )
            .to_owned(),
        );
    }
    if matches!(row.kind, Kind::Kernel | Kind::KernelStd | Kind::Facade) && !forbids {
        add(
            row.name,
            Rule::UnsafeNotForbidden,
            concat!(
                "#![forbid(unsafe_code)] among the crate root's leading attributes, or ",
                "unsafe_code = \"forbid\" in its manifest's [lints.rust] (27)"
            )
            .to_owned(),
        );
    }
}
