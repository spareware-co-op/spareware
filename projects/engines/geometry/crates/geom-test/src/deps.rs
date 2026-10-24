//! The dependency rule: no kernel crate depends, normally or at build time, on a development crate.
//!
//! What it does: [`check`] reads the output of `cargo metadata` (which [`cargo_metadata`] runs with
//! every feature of every workspace member on and every platform's dependencies in) and follows
//! every kernel crate's normal and build dependencies through the whole dependency graph, reporting
//! every chain that reaches `geom-test`, `geom-fuzz`, `geom-gen` or `geom-bench`. It also lists every
//! manifest of this repository the answer depends on ([`Report::manifests`]), so CI can check that a
//! change to any of them reruns the rule.
//!
//! Why: the development crates are "development only" (chapter 27, the crate table; 28.1: "No
//! kernel crate has a normal dependency on any of them"). They hold oracles, multiprecision
//! libraries, fuzzers and tools whose licences, size or nondeterminism must never reach a kernel
//! build, and the proposed rule for `geom-gen` (chapter 27, "Generated code and data have one
//! home") depends on it.
//!
//! Which crates are kernel crates: every package in the graph, a workspace member or not, whose
//! manifest is `projects/engines/geometry/crates/<folder>/Cargo.toml` under the workspace root and
//! whose name is not one of [`DEVELOPMENT_CRATES`]; and the facade `engine-geometry` and its
//! proposed binding crates `engine-geometry-*` under `projects/engines/crates/`, which chapter 27
//! says depend on "every kernel crate it re-exports, never a development crate".
//!
//! What counts as a dependency: two views of the graph, together.
//!
//! - **Declared.** Every entry of the `[dependencies]` and `[build-dependencies]` of every package
//!   in the graph, including optional, renamed and platform-specific ones, whether or not a feature
//!   or the host platform turns them on, because the rule is about what the manifests allow, not
//!   about one build. An entry naming a development crate is a violation: it is matched by package
//!   name, whatever it is renamed to and wherever it comes from, so even a registry or git crate
//!   that shares a development crate's name is refused. A path entry is followed to the package at
//!   that path when the graph holds it.
//! - **Resolved.** Every normal or build edge of cargo's resolved graph
//!   (`resolve.nodes[].deps[].dep_kinds`), resolved with every feature of every workspace member on
//!   and every platform's dependencies in. It reaches what the members' declarations alone cannot:
//!   crates that are not workspace members (a path crate outside the workspace folder or in its
//!   `exclude` list, a git crate), and their dependencies in turn.
//!
//! `[dev-dependencies]` are allowed: they build only the crate's own tests, and 28.1 lets any
//! kernel crate use bolero and proptest that way. Dev edges are never followed, and cargo resolves
//! no dev-dependencies of a crate outside the workspace. An optional dependency that no feature of
//! any workspace member turns on is in no build of this workspace, so the graph does not hold it
//! and it is not followed past its declaration.
//!
//! Guarantees:
//!
//! - **It never passes by accident.** Metadata it cannot read (a missing field, a field of the
//!   wrong type, an unknown dependency kind, a resolved edge without its kinds, or no resolved
//!   graph, which is what `cargo metadata --no-deps` gives) is an error, not a pass, and so is a
//!   workspace with no kernel crate, because a check that checked nothing must not pass.
//! - **Deterministic.** Kernel crates are visited in name order, declared dependencies in manifest
//!   order and resolved ones in name order; the report lists violations in that order.
//! - **Every chain is shown** from the kernel crate to the development crate, with the kind of each
//!   edge, so the fix is obvious.
//! - **It changes nothing.** [`cargo_metadata`] runs cargo with `--locked`, so it fails rather than
//!   write a `Cargo.lock`.
//!
//! Method: breadth-first search (Cormen, Leiserson, Rivest and Stein, *Introduction to Algorithms*,
//! 3rd edition, section 22.2) over the union of the declared and resolved graphs from each kernel
//! crate, on the JSON format documented at
//! <https://doc.rust-lang.org/cargo/commands/cargo-metadata.html>. Breadth-first search finds a
//! shortest chain to each development crate. The metadata is read into the typed model of the
//! `cargo_metadata` crate (MIT, <https://docs.rs/cargo_metadata/0.23.1/>), which also builds the
//! `cargo metadata` command: piece G12a put it in place of the field-by-field reading of the JSON
//! that was written here (chapter 29.1). [`check`] takes the JSON itself, not the model, so the
//! tests can seed a violation into real metadata as a manifest would and hand it over.
//!
//! Cost: O(K × (P + D)) for K kernel crates, P packages in the graph and D dependency entries,
//! declared and resolved, with a logarithmic factor for the ordered maps; plus one `cargo metadata`
//! run, which resolves the workspace from `Cargo.lock`.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use ::cargo_metadata::{CargoOpt, DependencyKind, Metadata, MetadataCommand};
use serde_json::Value;

/// The development crates: no kernel crate may depend on one, except as a dev-dependency.
pub const DEVELOPMENT_CRATES: [&str; 4] = ["geom-test", "geom-fuzz", "geom-gen", "geom-bench"];

/// The folder, relative to the workspace root, that holds one folder per kernel crate.
pub const KERNEL_CRATES_DIR: [&str; 4] = ["projects", "engines", "geometry", "crates"];

/// The folder, relative to the workspace root, that holds the facade `engine-geometry` and its
/// binding crates.
pub const ENGINE_CRATES_DIR: [&str; 3] = ["projects", "engines", "crates"];

/// The kind of a dependency edge, as `cargo metadata` reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// `[dependencies]`.
    Normal,
    /// `[build-dependencies]`.
    Build,
    /// `[dev-dependencies]`.
    Dev,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Kind::Normal => "normal",
            Kind::Build => "build",
            Kind::Dev => "dev",
        })
    }
}

/// One step of a chain: `to` is a dependency of the crate before it, of kind `kind`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// The dependency's package name.
    pub to: String,
    /// The kind of the edge.
    pub kind: Kind,
}

/// A kernel crate that reaches a development crate through normal or build dependencies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Violation {
    /// The kernel crate.
    pub kernel_crate: String,
    /// The chain from the kernel crate to the development crate; the last step is the development
    /// crate.
    pub chain: Vec<Step>,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kernel_crate)?;
        for s in &self.chain {
            write!(f, " -({})-> {}", s.kind, s.to)?;
        }
        Ok(())
    }
}

/// What [`check`] found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// Every kernel crate checked, in name order.
    pub kernel_crates: Vec<String>,
    /// Every development crate among the workspace members, in name order.
    pub development_crates: Vec<String>,
    /// Every violation, one per kernel crate and development crate it reaches.
    pub violations: Vec<Violation>,
    /// Every manifest of a local (path) package the answer depends on: the kernel crates and every
    /// local package they reach through normal or build edges, relative to the workspace root with
    /// `/` separators, sorted. A local package outside the workspace root is given by its absolute
    /// path. A change to any other manifest cannot add an edge out of this set, except through the
    /// workspace's own `Cargo.toml` (`[patch]`, `[workspace.dependencies]`), `Cargo.lock` or a
    /// cargo configuration file.
    pub manifests: Vec<String>,
}

/// Why the check could not run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DepsError {
    /// `cargo metadata` could not be started or failed.
    Cargo(String),
    /// Its output was not JSON.
    Json(String),
    /// The JSON does not fit `cargo metadata`'s format as the `cargo_metadata` crate models it: a
    /// field missing or of the wrong type. The crate's message, which names the field.
    Format(String),
    /// Something the check needs that the format leaves optional is not there: the resolved graph,
    /// or the kinds of a resolved edge.
    Field(String),
    /// A dependency kind other than normal, build or dev.
    UnknownKind {
        /// The package.
        package: String,
        /// The kind found, as the `cargo_metadata` crate names it: it reads every kind it does not
        /// know as `Unknown`, so the text cargo wrote is not kept.
        kind: String,
    },
    /// The workspace has no kernel crate, so the check would check nothing.
    NoKernelCrates,
}

impl fmt::Display for DepsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DepsError::Cargo(e) => write!(f, "cargo metadata failed: {e}"),
            DepsError::Json(e) => write!(f, "cargo metadata did not give JSON: {e}"),
            DepsError::Format(e) => write!(f, "cargo metadata does not fit its format: {e}"),
            DepsError::Field(e) => write!(f, "cargo metadata has no usable {e}"),
            DepsError::UnknownKind { package, kind } => {
                write!(f, "{package} has a dependency of unknown kind {kind:?}")
            }
            DepsError::NoKernelCrates => write!(
                f,
                "no kernel crate found under projects/engines/geometry/crates: \
                 the dependency rule would check nothing"
            ),
        }
    }
}

impl std::error::Error for DepsError {}

/// Runs `<cargo> metadata --format-version 1 --all-features --manifest-path <manifest> --locked`,
/// the command the `cargo_metadata` crate builds, and parses its output as JSON: the whole resolved
/// graph, every platform's dependencies included.
///
/// # Errors
///
/// [`DepsError::Cargo`] if cargo cannot start or exits unsuccessfully (for example because
/// `Cargo.lock` is missing or out of date), [`DepsError::Json`] if its output is not JSON.
pub fn cargo_metadata(cargo: &OsStr, manifest: &Path) -> Result<Value, DepsError> {
    let mut command = MetadataCommand::new();
    command
        .cargo_path(cargo)
        .manifest_path(manifest)
        .features(CargoOpt::AllFeatures)
        .other_options(["--locked".to_owned()]);
    let out = command
        .cargo_command()
        .output()
        .map_err(|e| DepsError::Cargo(e.to_string()))?;
    if !out.status.success() {
        return Err(DepsError::Cargo(format!(
            "{}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| DepsError::Json(e.to_string()))
}

/// One declared dependency.
struct Declared {
    name: String,
    kind: Kind,
    /// The folder of a path dependency.
    path: Option<PathBuf>,
}

/// One package of the graph.
struct Package {
    name: String,
    version: String,
    /// Whether it is a path package (no `source`): ours, in this repository.
    local: bool,
    /// Its manifest's path.
    manifest: PathBuf,
    /// Its manifest's path relative to the workspace root, by component, or `None` if it is not
    /// under the root.
    relative: Option<Vec<String>>,
    declared: Vec<Declared>,
}

/// The graph [`check`] searches.
struct Graph {
    /// Every package, by id.
    packages: BTreeMap<String, Package>,
    /// The workspace members' ids.
    members: BTreeSet<String>,
    /// Every package's normal and build edges in the resolved graph, by id: the dependency's id
    /// and the edge's kind, in the dependencies' name order.
    resolved: BTreeMap<String, Vec<(String, Kind)>>,
    /// Every package's id, by the folder of its manifest.
    by_folder: BTreeMap<PathBuf, String>,
}

/// A node of the search: a package of the graph, or a declared dependency the graph does not hold
/// (or whose target cannot be told from its declaration), known by name only.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Node {
    Package(String),
    Declared(String),
}

/// Checks the dependency rule on `cargo metadata` output (with its resolved graph).
///
/// # Errors
///
/// [`DepsError::Field`] or [`DepsError::UnknownKind`] if the metadata cannot be read, or
/// has no resolved graph; [`DepsError::NoKernelCrates`] if it names no kernel crate.
pub fn check(metadata: &Value) -> Result<Report, DepsError> {
    let graph = read_graph(metadata)?;
    let development: BTreeSet<&str> = DEVELOPMENT_CRATES.into_iter().collect();

    // Kernel crates in name order (then id, for two packages of one name).
    let mut kernel: Vec<(&str, &str)> = graph
        .packages
        .iter()
        .filter(|(_, p)| !development.contains(p.name.as_str()) && is_kernel(p))
        .map(|(id, p)| (p.name.as_str(), id.as_str()))
        .collect();
    kernel.sort_unstable();
    if kernel.is_empty() {
        return Err(DepsError::NoKernelCrates);
    }

    let mut violations = Vec::new();
    let mut manifests = BTreeSet::new();
    for (name, id) in &kernel {
        let (found, local) = search(name, id, &graph, &development);
        violations.extend(found);
        for p in local {
            manifests.insert(manifest_text(p));
        }
    }

    let mut kernel_crates: Vec<String> = kernel.iter().map(|(n, _)| (*n).to_owned()).collect();
    kernel_crates.dedup();
    let development_crates: BTreeSet<String> = graph
        .members
        .iter()
        .filter_map(|id| graph.packages.get(id))
        .filter(|p| development.contains(p.name.as_str()))
        .map(|p| p.name.clone())
        .collect();
    Ok(Report {
        kernel_crates,
        development_crates: development_crates.into_iter().collect(),
        violations,
        manifests: manifests.into_iter().collect(),
    })
}

/// Whether a package is a kernel crate: under the kernel crates folder, or the facade family under
/// the engines folder. Development crates are excluded by the caller.
fn is_kernel(p: &Package) -> bool {
    let Some(rel) = &p.relative else {
        return false;
    };
    // Two components below `dir`: the crate's folder and its `Cargo.toml`.
    let under = |dir: &[&str]| {
        rel.len().checked_sub(dir.len()) == Some(2)
            && rel.iter().zip(dir).all(|(a, b)| a == b)
            && rel.last().is_some_and(|l| l == "Cargo.toml")
    };
    under(&KERNEL_CRATES_DIR)
        || (under(&ENGINE_CRATES_DIR)
            && (p.name == "engine-geometry" || p.name.starts_with("engine-geometry-")))
}

/// A package's manifest as [`Report::manifests`] gives it.
fn manifest_text(p: &Package) -> String {
    match &p.relative {
        Some(parts) => parts.join("/"),
        None => p.manifest.display().to_string(),
    }
}

/// The package name of a node.
fn node_name<'a>(node: &'a Node, graph: &'a Graph) -> &'a str {
    match node {
        Node::Package(id) => graph
            .packages
            .get(id)
            .map_or(id.as_str(), |p| p.name.as_str()),
        Node::Declared(name) => name,
    }
}

/// The normal and build edges out of a package: its declarations in manifest order, then its
/// resolved edges in name order.
fn edges(id: &str, graph: &Graph) -> Vec<(Node, Kind)> {
    let mut out = Vec::new();
    if let Some(p) = graph.packages.get(id) {
        for d in p.declared.iter().filter(|d| d.kind != Kind::Dev) {
            let to = d
                .path
                .as_ref()
                .and_then(|folder| graph.by_folder.get(folder))
                .map_or_else(
                    || Node::Declared(d.name.clone()),
                    |t| Node::Package(t.clone()),
                );
            out.push((to, d.kind));
        }
    }
    if let Some(deps) = graph.resolved.get(id) {
        out.extend(
            deps.iter()
                .map(|(to, kind)| (Node::Package(to.clone()), *kind)),
        );
    }
    out
}

/// Breadth-first search from the kernel crate `start` over normal and build edges: one violation
/// per development crate reached, with a shortest chain to it, and every local package reached
/// (the kernel crate included).
fn search<'g>(
    name: &str,
    start: &str,
    graph: &'g Graph,
    development: &BTreeSet<&str>,
) -> (Vec<Violation>, Vec<&'g Package>) {
    let start = Node::Package(start.to_owned());
    // For each node reached: the node it was reached from and the edge's kind.
    let mut parent: BTreeMap<Node, Option<(Node, Kind)>> = BTreeMap::new();
    parent.insert(start.clone(), None);
    let mut queue = VecDeque::from([start]);
    let mut reached: Vec<Node> = Vec::new();
    let mut reached_names: BTreeSet<String> = BTreeSet::new();
    let mut local = Vec::new();
    while let Some(node) = queue.pop_front() {
        let Node::Package(id) = &node else {
            continue;
        };
        if let Some(p) = graph.packages.get(id).filter(|p| p.local) {
            local.push(p);
        }
        for (to, kind) in edges(id, graph) {
            if parent.contains_key(&to) {
                continue;
            }
            parent.insert(to.clone(), Some((node.clone(), kind)));
            let to_name = node_name(&to, graph);
            if development.contains(to_name) {
                // One violation per development crate: the first reached has a shortest chain.
                if reached_names.insert(to_name.to_owned()) {
                    reached.push(to);
                }
            } else {
                queue.push_back(to);
            }
        }
    }
    let violations = reached
        .into_iter()
        .map(|dev| {
            let mut chain = Vec::new();
            let mut at = dev;
            while let Some(Some((from, kind))) = parent.get(&at) {
                chain.push(Step {
                    to: node_name(&at, graph).to_owned(),
                    kind: *kind,
                });
                at = from.clone();
            }
            chain.reverse();
            Violation {
                kernel_crate: name.to_owned(),
                chain,
            }
        })
        .collect();
    (violations, local)
}

/// A dependency kind as the `cargo_metadata` crate reads it: cargo writes `null` (normal),
/// `"build"` or `"dev"`, and the crate reads any other text as `Unknown`, which is refused.
fn kind(kind: DependencyKind, package: &str) -> Result<Kind, DepsError> {
    match kind {
        DependencyKind::Normal => Ok(Kind::Normal),
        DependencyKind::Build => Ok(Kind::Build),
        DependencyKind::Development => Ok(Kind::Dev),
        DependencyKind::Unknown => Err(DepsError::UnknownKind {
            package: package.to_owned(),
            kind: kind.to_string(),
        }),
    }
}

/// The graph: every package with its declarations, and the resolved graph's normal and build edges.
fn read_graph(metadata: &Value) -> Result<Graph, DepsError> {
    let metadata: Metadata = serde_json::from_value(metadata.clone())
        .map_err(|e| DepsError::Format(e.to_string()))?;
    let root = metadata.workspace_root.as_std_path();
    let members: BTreeSet<String> = metadata
        .workspace_members
        .iter()
        .map(|id| id.repr.clone())
        .collect();

    let mut packages = BTreeMap::new();
    let mut by_folder = BTreeMap::new();
    for p in &metadata.packages {
        let id = p.id.repr.clone();
        let manifest = p.manifest_path.clone().into_std_path_buf();
        let mut declared = Vec::new();
        for d in &p.dependencies {
            declared.push(Declared {
                name: d.name.clone(),
                kind: kind(d.kind, &p.name)?,
                path: d.path.clone().map(::cargo_metadata::camino::Utf8PathBuf::into_std_path_buf),
            });
        }
        if let Some(folder) = manifest.parent() {
            by_folder.insert(folder.to_path_buf(), id.clone());
        }
        let relative = relative_components(&manifest, root);
        packages.insert(
            id,
            Package {
                name: p.name.to_string(),
                version: p.version.to_string(),
                local: p.source.is_none(),
                manifest,
                relative,
                declared,
            },
        );
    }

    // `cargo metadata --no-deps` gives `"resolve": null`: without the resolved graph the check
    // would miss every chain through a crate that is not a workspace member, so it refuses.
    let nodes = &metadata
        .resolve
        .as_ref()
        .ok_or_else(|| field("resolve.nodes (run cargo metadata without --no-deps)"))?
        .nodes;
    let mut resolved = BTreeMap::new();
    for n in nodes {
        let id = n.id.repr.clone();
        let from = packages.get(&id).map_or(id.as_str(), |p| p.name.as_str());
        // The model reads a missing `deps` or `dep_kinds` as empty, which would drop an edge
        // unseen: every dependency the node lists must be among its `deps`, each with a kind.
        let with_kinds: BTreeSet<&str> = n
            .deps
            .iter()
            .filter(|d| !d.dep_kinds.is_empty())
            .map(|d| d.pkg.repr.as_str())
            .collect();
        if n.deps.iter().any(|d| d.dep_kinds.is_empty())
            || n
                .dependencies
                .iter()
                .any(|d| !with_kinds.contains(d.repr.as_str()))
        {
            return Err(field(&format!(
                "resolve.nodes[].deps[].dep_kinds of {from} (every resolved edge with its kinds)"
            )));
        }
        let mut out: Vec<(&str, &str, String, Kind)> = Vec::new();
        for d in &n.deps {
            let to = d.pkg.repr.clone();
            let mut best: Option<Kind> = None;
            for k in &d.dep_kinds {
                let k = kind(k.kind, from)?;
                if k != Kind::Dev {
                    best = Some(best.map_or(k, |b| b.min(k)));
                }
            }
            if let Some(k) = best {
                let (to_name, to_version) = packages
                    .get(&to)
                    .map_or(("", ""), |p| (p.name.as_str(), p.version.as_str()));
                out.push((to_name, to_version, to, k));
            }
        }
        out.sort();
        let edges = out.into_iter().map(|(_, _, to, k)| (to, k)).collect();
        resolved.insert(id, edges);
    }

    Ok(Graph {
        packages,
        members,
        resolved,
        by_folder,
    })
}

/// `path` relative to `root`, as plain component names, or `None` if it is not under `root`.
fn relative_components(path: &Path, root: &Path) -> Option<Vec<String>> {
    let rel = path.strip_prefix(root).ok()?;
    rel.components()
        .map(|c| match c {
            Component::Normal(s) => s.to_str().map(str::to_owned),
            _ => None,
        })
        .collect()
}

fn field(name: &str) -> DepsError {
    DepsError::Field(name.to_owned())
}
