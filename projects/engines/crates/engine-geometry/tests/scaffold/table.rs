//! The crate table: chapter 27's crate table and layer table (accepted in 32.13), in one place.
//!
//! What it holds: every crate of the kernel's family with its layer, its kind, where its row comes
//! from (G01's table, or the amendments of chapter 27 or chapter 30.5 that owner action G08
//! accepted, section 32.13), which piece writes it, and exactly its direct kernel dependencies; the dependencies still waiting for a
//! crate that another piece is building; the edges the layer table allows inside one layer; the
//! dependencies behind a feature; the registry crates a `no_std` kernel crate may name; the header
//! lines missing from crates written before this check existed; and the crate whose lint settings
//! every crate that ships copies.
//!
//! What it guarantees: nothing by itself. `check::table` proves it consistent (every name known,
//! every edge down a layer or along an allowed same-layer edge, no cycle) and `check::workspace`
//! proves the repository matches it.
//!
//! Method: the crate-graph check of chapter 27, "What Depends on means", which takes the layering
//! tool of `storytold/cadcraft` as its model (chapter 14.14;
//! <https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/xtask/src/layers.rs#L35-L75>).
//!
//! Cost: constant data; reading a dependency list splits one short string.
//!
//! Notation, chapter 27's own: in dependency lists a name without a hyphen is a `geom-` crate
//! (`num` is `geom-num`), a name with one is written in full (`engine-geometry`), and `base`
//! stands for `geom-run`, `geom-num` and `geom-op`.
//!
//! How the table changes:
//!
//! - Owner action G08 accepted every row (section 32.13). A new crate, or a change to a row's
//!   layer or job, needs an owner's change to chapter 27 first; chapter 27, this table and the
//!   crates then change in one pull request.
//! - When a crate under way merges (`geom-math`, `geom-run`), the same pull request adds its path
//!   dependency to every crate its `PENDING` entry names, deletes the entry, and deletes the
//!   "Direct dependencies still to add" line from each of those crates' headers. The check fails
//!   until all three are done.
//! - A deferred crate (a toolkit, a board format, a binding) is built by first making its row
//!   `Here` with its exact dependencies.
//! - A design note that adds an edge changes the row; the check then asks for the `Cargo.toml`.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use crate::header::Field;

/// A crate's layer: one of the thirteen of chapter 27's layer table, or development only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    /// Layer 1 (numbers and the contract) to 13 (facade).
    N(u8),
    /// The development crates, which never ship.
    Development,
}

/// What a crate is allowed to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A `geom-*` crate: `no_std` and `forbid(unsafe_code)` (chapters 21 and 27).
    Kernel,
    /// `geom-pool`: threads need `std` (23.4); `forbid(unsafe_code)` still holds.
    KernelStd,
    /// `engine-geometry`: may use `std` because it holds no kernel logic; `forbid(unsafe_code)`.
    Facade,
    /// A binding crate: the audited foreign-function boundary, the only crates in the family that
    /// may hold `unsafe` (chapter 27, "Proposed rule changes").
    Binding,
    /// `geom-test`, `geom-fuzz`, `geom-bench`, `geom-gen`: `std`, dependencies as needed, never a
    /// normal dependency of a crate that ships.
    Development,
}

/// Where a crate's row comes from. Every row is accepted: G01's table, and the amendments owner
/// action G08 accepted (section 32.13).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// G01's crate table (chapter 27's first table).
    Accepted,
    /// Chapter 27's amendments, accepted by owner action G08.
    Amended27,
    /// Chapter 30.5's amendments, accepted by owner action G08.
    Amended30,
}

/// Who writes the crate, and so whether it must exist yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State<'a> {
    /// Scaffolded by this piece (G09): it must exist, with the full scaffold header.
    Here,
    /// Written by the named piece, which may not have merged yet: absent is allowed, and once
    /// present it is checked like any other crate.
    Elsewhere(&'a str),
    /// Not to be built before the named piece: it must be absent until its row changes.
    Deferred(&'a str),
}

/// A crate's direct kernel dependencies, in chapter 27's notation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deps<'a> {
    /// Exactly these, no more and no fewer.
    Exact(&'a str),
    /// At least these; the crate's design note settles the rest before it may exist.
    Partial(&'a str),
    /// A development crate: whatever it needs.
    AsNeeded,
}

/// One row of the crate table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row<'a> {
    /// The crate's name, which is also its folder's name.
    pub name: &'a str,
    /// Its layer.
    pub layer: Layer,
    /// What it is allowed to be.
    pub kind: Kind,
    /// Where its row comes from.
    pub origin: Origin,
    /// Who writes it.
    pub state: State<'a>,
    /// Its direct kernel dependencies.
    pub deps: Deps<'a>,
    /// Path dependencies outside the kernel it may have, in full: only the facade's `kit-core`.
    pub external: &'a str,
}

/// Dependencies on a crate that is being built in another piece and is not on this branch yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pending<'a> {
    /// The crate under way.
    pub dep: &'a str,
    /// The piece building it.
    pub piece: &'a str,
    /// The crates whose rows name it and which add it when it merges.
    pub crates: &'a str,
}

/// A dependency that must be optional and switched on by one feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeatureGate<'a> {
    /// The crate that depends.
    pub krate: &'a str,
    /// The dependency.
    pub dep: &'a str,
    /// The feature that switches it on.
    pub feature: &'a str,
}

/// A header field missing from a crate this piece may not edit, and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeaderGap<'a> {
    /// The crate.
    pub krate: &'a str,
    /// The missing field.
    pub field: Field,
    /// Why it is missing and who adds it.
    pub reason: &'a str,
}

/// The whole table.
#[derive(Clone, Copy, Debug)]
pub struct Table<'a> {
    /// Every crate of the family.
    pub rows: &'a [Row<'a>],
    /// Edges allowed inside one layer: each crate, then the crates of its own layer it may use.
    pub same_layer: &'a [(&'a str, &'a str)],
    /// Dependencies waiting for a crate under way.
    pub pending: &'a [Pending<'a>],
    /// Dependencies behind a feature.
    pub feature_gates: &'a [FeatureGate<'a>],
    /// Header fields known to be missing, each with its reason.
    pub header_gaps: &'a [HeaderGap<'a>],
    /// The crates.io crates a `no_std` kernel crate may name as a normal dependency: each crate,
    /// then one registry crate. Any other registry or git dependency of such a crate is refused.
    pub registry: &'a [(&'a str, &'a str)],
    /// The kernel crate whose lint settings every crate that ships copies exactly (`lints`).
    pub lints_from: &'a str,
}

/// Expands one name of chapter 27's notation to the crate's full name.
pub fn full_name(token: &str) -> String {
    if token.contains('-') {
        token.to_owned()
    } else {
        format!("geom-{token}")
    }
}

/// Expands a list in chapter 27's notation to full names, in the order written; `base` becomes
/// `geom-run`, `geom-num` and `geom-op`.
pub fn expand(list: &str) -> Vec<String> {
    let mut out = Vec::new();
    for token in list.split_whitespace() {
        if token == "base" {
            out.extend(["geom-run", "geom-num", "geom-op"].map(str::to_owned));
        } else {
            out.push(full_name(token));
        }
    }
    out
}

const fn kernel<'a>(
    name: &'a str,
    layer: u8,
    origin: Origin,
    state: State<'a>,
    deps: &'a str,
) -> Row<'a> {
    Row {
        name,
        layer: Layer::N(layer),
        kind: Kind::Kernel,
        origin,
        state,
        deps: Deps::Exact(deps),
        external: "",
    }
}

const fn toolkit<'a>(name: &'a str, origin: Origin, piece: &'a str, deps: Deps<'a>) -> Row<'a> {
    Row {
        name,
        layer: Layer::N(11),
        kind: Kind::Kernel,
        origin,
        state: State::Deferred(piece),
        deps,
        external: "",
    }
}

const fn binding<'a>(name: &'a str, piece: &'a str) -> Row<'a> {
    Row {
        name,
        layer: Layer::N(13),
        kind: Kind::Binding,
        origin: Origin::Amended27,
        state: State::Deferred(piece),
        deps: Deps::Exact("engine-geometry"),
        external: "",
    }
}

const fn development<'a>(name: &'a str, origin: Origin, piece: &'a str) -> Row<'a> {
    Row {
        name,
        layer: Layer::Development,
        kind: Kind::Development,
        origin,
        state: State::Elsewhere(piece),
        deps: Deps::AsNeeded,
        external: "",
    }
}

use Origin::{Accepted, Amended27, Amended30};
use State::{Deferred, Elsewhere, Here};

/// Every crate of the family, layer by layer (chapter 27's crate table, with `geom-make` and the
/// dependency amendments of chapter 30.5, all accepted in section 32.13). One row per line, as
/// chapter 27 prints it, so rustfmt is told to leave the rows as they are.
#[rustfmt::skip]
const ROWS: &[Row<'static>] = &[
    // 1. Numbers and the contract.
    kernel("geom-run",       1, Amended27, Elsewhere("G04"), ""),
    kernel("geom-num",       1, Accepted,   Elsewhere("G02"), "run"),
    kernel("geom-linalg",    1, Amended27, Elsewhere("G49"), "num"),
    kernel("geom-op",        1, Amended27, Elsewhere("G86"), "run num"),
    // 2. Geometry.
    kernel("geom-math",      2, Accepted,   Elsewhere("G03"), "num op"),
    kernel("geom-bvh",       2, Amended27, Elsewhere("G54"), "run num math"),
    kernel("geom-nurbs",     2, Accepted,   Elsewhere("G56"), "base math linalg"),
    kernel("geom-curve",     2, Accepted,   Elsewhere("G60"), "base math nurbs"),
    kernel("geom-surface",   2, Accepted,   Elsewhere("G60"), "base math nurbs curve"),
    // 3. Topology.
    kernel("geom-topo",      3, Accepted,   Here, "base math curve surface"),
    kernel("geom-name",      3, Accepted,   Here, "base topo"),
    // 4. Intersections.
    kernel("geom-intersect", 4, Accepted,   Here, "base math nurbs curve surface bvh"),
    // 5. Checker.
    kernel("geom-check",     5, Accepted,   Here, "base topo intersect"),
    // 6. Arrangements.
    kernel("geom-poly",      6, Accepted,   Here, "base math topo"),
    kernel("geom-bool",      6, Accepted,   Here, "base poly intersect topo name check bvh"),
    kernel("geom-2d",        6, Accepted,   Here, "base math curve topo name intersect bvh"),
    // 7. Meshes and spatial.
    kernel("geom-mesh",      7, Accepted,   Here,
        "base math topo poly curve surface intersect bvh linalg 2d"),
    kernel("geom-spatial",   7, Accepted,   Here, "base math mesh bvh poly intersect"),
    // 8. Other representations.
    kernel("geom-subd",      8, Accepted,   Here, "base math nurbs surface"),
    kernel("geom-field",     8, Accepted,   Here, "base math mesh surface 2d bvh"),
    kernel("geom-cloud",     8, Accepted,   Here, "base math spatial nurbs surface linalg"),
    // 9. Shared operation services; geom-make is chapter 30.5's.
    kernel("geom-skeleton",  9, Accepted,   Here, "base math 2d topo poly"),
    kernel("geom-sweep",     9, Accepted,   Here,
        "base topo surface name check poly intersect bool skeleton"),
    kernel("geom-make",      9, Amended30, Here, "base math curve surface topo check"),
    // 10. Operations.
    kernel("geom-blend",    10, Accepted,   Here, "base bool intersect surface topo check"),
    kernel("geom-offset",   10, Accepted,   Here, "base bool intersect surface skeleton topo"),
    kernel("geom-sketch",   10, Accepted,   Here, "base math curve linalg"),
    kernel("geom-profile",  10, Amended27, Here, "base sketch 2d topo name check intersect"),
    kernel("geom-terrain",  10, Accepted,   Here,
        "base math 2d mesh topo poly bool skeleton curve surface name bvh"),
    kernel("geom-align",    10, Accepted,   Here, "base math curve sweep name"),
    kernel("geom-draw",     10, Accepted,   Here,
        "base topo name 2d intersect spatial math curve surface mesh bvh"),
    kernel("geom-convert",  10, Accepted,   Here,
        "base topo mesh subd field nurbs check bool cloud spatial intersect sweep"),
    kernel("geom-combine",  10, Amended27, Here, "base bool mesh field convert check subd"),
    kernel("geom-panel",    10, Amended27, Here,
        "base topo check surface intersect poly mesh 2d linalg"),
    kernel("geom-analysis", 10, Amended27, Here, "base surface topo intersect spatial"),
    // 11. Domain toolkits: a toolkit enters the kernel when a second program needs it (26.1),
    // so none is built yet; owner action G238 decides whether geom-civil, geom-thread and
    // geom-stair enter as exceptions. Chapter 27 gives the sheet, thread and stair toolkits "base and the
    // operation crates they compose": their design notes settle the rest.
    toolkit("geom-sheet",    Accepted,   "G472", Deps::Partial("base")),
    toolkit("geom-thread",   Accepted,   "G258", Deps::Partial("base")),
    toolkit("geom-board",    Accepted,   "G473", Deps::Partial("base sheet")),
    toolkit("geom-stair",    Accepted,   "G281", Deps::Partial("base")),
    toolkit("geom-civil",    Amended27, "G378", Deps::Exact("base terrain align")),
    toolkit("geom-assembly", Amended27, "G479", Deps::Exact("base sketch spatial")),
    // 12. Format and exchange. geom-format's "everything stored" is section 24.4's list; the
    // format crates' "kinds they map" are section 24.11's; p21, exchange and heal carry chapter
    // 30.5's amendments.
    kernel("geom-format",   12, Accepted,   Elsewhere("G91"),
        "base math curve surface topo poly name mesh subd field 2d cloud draw sketch"),
    kernel("geom-exchange", 12, Amended27, Here,
        "base math curve surface topo format p21 intersect check"),
    kernel("geom-p21",      12, Amended27, Here, "base format"),
    kernel("geom-heal",     12, Amended27, Here, "base topo check mesh exchange"),
    kernel("geom-ifc",      12, Accepted,   Here, concat!(
        "base format exchange heal p21 ",
        "math curve surface topo poly mesh 2d sweep bool align terrain cloud")),
    kernel("geom-step",     12, Accepted,   Here,
        "base format exchange heal p21 math curve surface topo poly mesh"),
    kernel("geom-dxf",      12, Accepted,   Here,
        "base format exchange heal math curve topo 2d sweep mesh subd"),
    kernel("geom-3dm",      12, Accepted,   Here,
        "base format exchange heal math curve surface topo mesh subd"),
    kernel("geom-meshio",   12, Accepted,   Here, "base format exchange heal math mesh subd"),
    kernel("geom-landxml",  12, Amended27, Here, "base format exchange 2d terrain align"),
    // Board exchange follows the two-program rule too (24.11), so it waits with the toolkits.
    kernel("geom-gerber",   12, Amended27, Deferred("G474"), "base format exchange 2d"),
    kernel("geom-ipc2581",  12, Amended27, Deferred("G475"), "base format exchange 2d"),
    // 13. Facade. engine-geometry's row: every kernel crate it re-exports (none yet: G100 adds
    // them), and geom-pool behind its `threads` feature. geom-pool was scaffolded here: G04 built
    // only geom-run; G33 wrote its native pool (rayon), and G456 adds Web Workers.
    Row { name: "engine-geometry", layer: Layer::N(13), kind: Kind::Facade, origin: Accepted,
          state: Here, deps: Deps::Exact("pool"), external: "kit-core" },
    Row { name: "geom-pool", layer: Layer::N(13), kind: Kind::KernelStd, origin: Amended27,
          state: Elsewhere("G33"), deps: Deps::Exact("run"), external: "" },
    binding("engine-geometry-c",    "G453"),
    binding("engine-geometry-wasm", "G454"),
    binding("engine-geometry-py",   "G455"),
    // Development.
    development("geom-test",  Accepted,   "G11"),
    development("geom-fuzz",  Accepted,   "G11"),
    development("geom-bench", Accepted,   "G06"),
    development("geom-gen",   Amended27, "G11"),
];

/// Edges allowed inside one layer (chapter 27's layer table, with chapter 30.5's amendments in
/// layer 12): each crate, then the crates of its own layer it may depend on.
const SAME_LAYER: &[(&str, &str)] = &[
    ("num", "run"),
    ("linalg", "num"),
    ("op", "run num"),
    ("bvh", "math"),
    ("nurbs", "math"),
    ("curve", "math nurbs"),
    ("surface", "math nurbs curve"),
    ("name", "topo"),
    ("bool", "poly"),
    ("spatial", "mesh"),
    ("sweep", "skeleton"),
    ("profile", "sketch"),
    ("combine", "convert"),
    ("board", "sheet"),
    ("ifc", "format exchange heal p21"),
    ("step", "format exchange heal p21"),
    ("dxf", "format exchange heal"),
    ("3dm", "format exchange heal"),
    ("meshio", "format exchange heal"),
    ("landxml", "format exchange heal"),
    ("gerber", "format exchange heal"),
    ("ipc2581", "format exchange heal"),
    ("p21", "format"),
    ("exchange", "format p21"),
    ("heal", "exchange"),
    ("engine-geometry", "pool"),
    ("engine-geometry-c", "engine-geometry"),
    ("engine-geometry-wasm", "engine-geometry"),
    ("engine-geometry-py", "engine-geometry"),
];

/// Dependencies on crates under way in other pieces, added when each merges.
const PENDING: &[Pending<'static>] = &[];

/// The facade reaches threads only through `geom-pool`, behind its `threads` feature (27).
const FEATURE_GATES: &[FeatureGate<'static>] = &[FeatureGate {
    krate: "engine-geometry",
    dep: "pool",
    feature: "threads",
}];

/// Header fields missing from crates this piece may not edit. None now: G02 added geom-num's
/// Layer line, the last gap.
const HEADER_GAPS: &[HeaderGap<'static>] = &[];

/// Inside kernel crates the published crates used as they are, are `libm` (29.1) and `sha2`
/// (owner action G79, 32.20). Every maths function reaches the kernel through `geom_num::math`
/// (21), so only `geom-num` names `libm`; SHA-256 hashes names in `geom-op` and content in
/// `geom-format`, so those two name `sha2` (32.20); records' CRC-32C is `geom-format`'s, so it
/// alone names `crc` (24.2, piece G92). The `std` crates (`geom-pool`, with rayon, and
/// the facade) and the development crates are left to the merge gate's dependency rules (G12).
const REGISTRY: &[(&str, &str)] = &[
    ("num", "libm"),
    ("op", "sha2"),
    ("format", "sha2"),
    ("format", "crc"),
];

/// The kernel's one lint posture is `geom-num`'s: the numbers layer set it first, with chapter 21's
/// disallowed methods and types and chapter 27's denials, and every crate that ships is built with
/// exactly its settings (`lints`).
const LINTS_FROM: &str = "num";

/// The kernel's crate table.
pub const TABLE: Table<'static> = Table {
    rows: ROWS,
    same_layer: SAME_LAYER,
    pending: PENDING,
    feature_gates: FEATURE_GATES,
    header_gaps: HEADER_GAPS,
    registry: REGISTRY,
    lints_from: LINTS_FROM,
};
