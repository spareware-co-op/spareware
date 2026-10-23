//! Each rule of the check, shown to fail on a seeded violation.
//!
//! What it does: builds a small in-memory crate table and a workspace that matches it, every crate
//! that ships carrying the reference crate's lint settings, and a toolchain file with two workflows
//! that agree, shows the checks pass them, then breaks one thing at a time and shows the checks
//! report exactly the rule that thing breaks. Nothing in the repository is changed.
//!
//! What it guarantees: every rule of `check::Rule` has at least one test here that makes it fire,
//! and most assert the exact set of rules that fire, so a check that fires too widely fails too.
//! The matching example shows the check is silent on correct input.
//!
//! Method: seeded faults, as chapter 28.16 asks of every check ("each check is shown to fail on
//! a seeded violation", G12).
//!
//! Cost: microseconds; no input or output.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, BTreeSet};

use crate::check::{self, CRATES_IO, Dep, DepKind, Observed, Package, Rule, Violation};
use crate::header::{Field, Header, MARK_27, pending_line};
use crate::lints::{ClippyConfig, Lints};
use crate::source::Attributes;
use crate::table::{Deps, FeatureGate, HeaderGap, Kind, Layer, Origin, Pending, Row, State, Table};
use crate::toolchain;

const fn row<'a>(
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

/// The example table: two layer-1 crates (one under way, one allowed a registry crate), two layer-2
/// crates with an allowed same-layer edge, a crate another piece wrote with a header gap, whose
/// lint settings the others copy, a deferred toolkit, a development crate, a `std` pool under way,
/// and a facade with an external dependency, a pending dependency and a feature gate.
const ROWS: &[Row<'static>] = &[
    row("geom-a", 1, Origin::Accepted, State::Here, ""),
    row("geom-u", 1, Origin::Proposed27, State::Elsewhere("G90"), ""),
    row("geom-b", 2, Origin::Accepted, State::Here, "a u"),
    row("geom-c", 2, Origin::Proposed27, State::Here, "a b"),
    row("geom-e", 3, Origin::Accepted, State::Elsewhere("G94"), "a"),
    Row {
        name: "geom-k",
        layer: Layer::N(11),
        kind: Kind::Kernel,
        origin: Origin::Accepted,
        state: State::Deferred("G91"),
        deps: Deps::Partial("c"),
        external: "",
    },
    Row {
        name: "geom-dev",
        layer: Layer::Development,
        kind: Kind::Development,
        origin: Origin::Accepted,
        state: State::Elsewhere("G92"),
        deps: Deps::AsNeeded,
        external: "",
    },
    Row {
        name: "geom-p",
        layer: Layer::N(13),
        kind: Kind::KernelStd,
        origin: Origin::Proposed27,
        state: State::Elsewhere("G93"),
        deps: Deps::Exact("u"),
        external: "",
    },
    Row {
        name: "engine-geometry",
        layer: Layer::N(13),
        kind: Kind::Facade,
        origin: Origin::Accepted,
        state: State::Here,
        deps: Deps::Exact("c p"),
        external: "kit-core",
    },
];

const SAME_LAYER: &[(&str, &str)] = &[("c", "b"), ("engine-geometry", "p")];

const PENDING: &[Pending<'static>] = &[
    Pending {
        dep: "u",
        piece: "G90",
        crates: "b",
    },
    Pending {
        dep: "p",
        piece: "G93",
        crates: "engine-geometry",
    },
];

const GATES: &[FeatureGate<'static>] = &[FeatureGate {
    krate: "engine-geometry",
    dep: "p",
    feature: "threads",
}];

const GAPS: &[HeaderGap<'static>] = &[HeaderGap {
    krate: "e",
    field: Field::Layer,
    reason: "written before the layer table",
}];

const REGISTRY: &[(&str, &str)] = &[("a", "libm")];

const TABLE: Table<'static> = Table {
    rows: ROWS,
    same_layer: SAME_LAYER,
    pending: PENDING,
    feature_gates: GATES,
    header_gaps: GAPS,
    registry: REGISTRY,
    lints_from: "e",
};

fn header(layer: Option<&str>, mark: bool, pending: &[(&str, &str)], status: bool) -> Header {
    let mut lines: Vec<String> = vec![
        "Spare Kit geometry kernel: an example.".to_owned(),
        String::new(),
        "One job: an example.".to_owned(),
        String::new(),
    ];
    if let Some(layer) = layer {
        lines.push(format!("Layer: {layer}, an example layer."));
        lines.push(String::new());
    }
    for line in [
        "Invariants this crate keeps:",
        "",
        "- One.",
        "",
        "Sections: 27.",
        "",
    ] {
        lines.push(line.to_owned());
    }
    for (dep, piece) in pending {
        lines.push(format!("{}.", pending_line(dep, piece)));
    }
    if mark {
        lines.push(String::new());
        lines.push(MARK_27.to_owned());
    }
    if status {
        lines.push(String::new());
        lines.push("Status: scaffold only; built by pieces G95.".to_owned());
    }
    let text: String = lines
        .iter()
        .flat_map(|line| ["//! ", line.as_str(), "\n"])
        .collect();
    Header::parse(&text)
}

fn settings(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

/// The example's lint posture, read from a crate in `dir`: geom-e's, which every crate that ships
/// copies.
fn kernel_lints(dir: &[&str]) -> Lints {
    let mut file: Vec<String> = dir.iter().map(|s| (*s).to_owned()).collect();
    file.push("clippy.toml".to_owned());
    Lints {
        tables: Some(settings(&[
            ("lints.rust.unsafe_code", "\"forbid\""),
            ("lints.clippy.pedantic.level", "\"warn\""),
            ("lints.clippy.pedantic.priority", "-1"),
            ("lints.clippy.arithmetic_side_effects", "\"deny\""),
        ])),
        clippy: Some(ClippyConfig {
            file,
            settings: Some(settings(&[
                ("allow-unwrap-in-tests", "true"),
                (
                    "disallowed-methods",
                    "[{path=\"f64::max\",reason=\"either zero\"}]",
                ),
            ])),
        }),
    }
}

fn dep(name: &str, kind: DepKind) -> Dep {
    Dep {
        name: name.to_owned(),
        kind,
        path: true,
        optional: false,
        source: None,
    }
}

/// A dependency on a published crate, from `source`.
fn published(name: &str, kind: DepKind, source: &str) -> Dep {
    Dep {
        name: name.to_owned(),
        kind,
        path: false,
        optional: false,
        source: Some(source.to_owned()),
    }
}

/// A crate in `dir`, carrying the example's lint posture.
fn package(dir: &[&str], header: Header, attributes: Attributes, deps: Vec<Dep>) -> Package {
    Package {
        dir: dir.iter().map(|s| (*s).to_owned()).collect(),
        lib: Some(vec!["src".to_owned(), "lib.rs".to_owned()]),
        header,
        attributes,
        manifest_forbids_unsafe: false,
        lints: kernel_lints(dir),
        deps,
        features: BTreeMap::new(),
    }
}

const KERNEL: Attributes = Attributes {
    no_std: true,
    forbids_unsafe: true,
};

fn geometry(name: &str) -> Vec<&str> {
    vec!["projects", "engines", "geometry", "crates", name]
}

/// A workspace that matches `TABLE` exactly.
fn good() -> Observed {
    let mut packages = BTreeMap::new();
    let normal = |name: &str| dep(name, DepKind::Normal);
    packages.insert(
        "geom-a".to_owned(),
        package(
            &geometry("geom-a"),
            header(Some("1"), false, &[], true),
            KERNEL,
            vec![
                published("libm", DepKind::Normal, CRATES_IO),
                published("proptest", DepKind::Dev, CRATES_IO),
            ],
        ),
    );
    packages.insert(
        "geom-b".to_owned(),
        package(
            &geometry("geom-b"),
            header(Some("2"), false, &[("geom-u", "G90")], true),
            KERNEL,
            vec![normal("geom-a")],
        ),
    );
    packages.insert(
        "geom-c".to_owned(),
        package(
            &geometry("geom-c"),
            header(Some("2"), true, &[], true),
            KERNEL,
            vec![
                normal("geom-a"),
                normal("geom-b"),
                dep("geom-dev", DepKind::Dev),
            ],
        ),
    );
    // Written by another piece: no status line, its Layer line listed as a gap, and unsafe code
    // forbidden through its manifest's lint table instead of the attribute.
    let mut e = package(
        &geometry("geom-e"),
        header(None, false, &[], false),
        Attributes {
            no_std: true,
            forbids_unsafe: false,
        },
        vec![normal("geom-a")],
    );
    e.manifest_forbids_unsafe = true;
    packages.insert("geom-e".to_owned(), e);
    packages.insert(
        "geom-dev".to_owned(),
        package(
            &geometry("geom-dev"),
            header(Some("Development"), false, &[], false),
            Attributes::default(),
            vec![
                normal("geom-a"),
                published("proptest", DepKind::Normal, CRATES_IO),
            ],
        ),
    );
    packages.insert(
        "engine-geometry".to_owned(),
        package(
            &["projects", "engines", "crates", "engine-geometry"],
            header(Some("13"), false, &[("geom-p", "G93")], true),
            Attributes {
                no_std: false,
                forbids_unsafe: true,
            },
            vec![normal("geom-c"), normal("kit-core")],
        ),
    );
    packages.insert(
        "kit-core".to_owned(),
        package(
            &["projects", "kit", "crates", "kit-core"],
            Header::default(),
            Attributes::default(),
            vec![],
        ),
    );
    packages.insert(
        "spare-app".to_owned(),
        package(
            &["projects", "spare-app", "crates", "spare-app"],
            Header::default(),
            Attributes::default(),
            vec![normal("engine-geometry")],
        ),
    );
    // A development crate and the crates outside the kernel are not held to the kernel's lint
    // posture: here they have no lint settings at all.
    for name in ["geom-dev", "kit-core", "spare-app"] {
        packages.get_mut(name).unwrap().lints = Lints::default();
    }
    let geometry_dirs = ["geom-a", "geom-b", "geom-c", "geom-e", "geom-dev"]
        .map(str::to_owned)
        .into();
    Observed {
        packages,
        geometry_dirs,
    }
}

fn rules(violations: &[Violation]) -> BTreeSet<Rule> {
    violations.iter().map(|v| v.rule).collect()
}

fn show(violations: &[Violation]) -> String {
    let mut text = String::new();
    for v in violations {
        text.push_str("\n  ");
        text.push_str(&v.to_string());
    }
    text
}

/// Runs both checks and asserts they report exactly `expected`.
fn expect_rules(table: &Table<'_>, observed: &Observed, expected: &[Rule]) {
    let mut found = check::table(table);
    found.extend(check::workspace(table, observed));
    let expected: BTreeSet<Rule> = expected.iter().copied().collect();
    assert_eq!(rules(&found), expected, "violations:{}", show(&found));
}

fn with_rows(
    rows: &[Row<'static>],
    change: impl FnOnce(&mut Vec<Row<'static>>),
) -> Vec<Row<'static>> {
    let mut rows = rows.to_vec();
    change(&mut rows);
    rows
}

fn set_deps(rows: &mut [Row<'static>], name: &str, deps: Deps<'static>) {
    rows.iter_mut().find(|r| r.name == name).unwrap().deps = deps;
}

fn package_mut<'o>(observed: &'o mut Observed, name: &str) -> &'o mut Package {
    observed.packages.get_mut(name).unwrap()
}

#[test]
fn the_example_passes() {
    expect_rules(&TABLE, &good(), &[]);
}

// ---------------------------------------------------------- the table against itself

#[test]
fn table_duplicate_row() {
    let rows = with_rows(ROWS, |rows| rows.push(rows[0]));
    expect_rules(
        &Table {
            rows: &rows,
            ..TABLE
        },
        &good(),
        &[Rule::DuplicateRow],
    );
}

#[test]
fn table_kind_and_layer_must_fit() {
    let rows = with_rows(ROWS, |rows| rows[0].layer = Layer::N(13));
    let found = check::table(&Table {
        rows: &rows,
        ..TABLE
    });
    assert!(rules(&found).contains(&Rule::BadRow), "{}", show(&found));
}

#[test]
fn table_unknown_dependency() {
    let rows = with_rows(ROWS, |rows| set_deps(rows, "geom-a", Deps::Exact("zz")));
    let found = check::table(&Table {
        rows: &rows,
        ..TABLE
    });
    assert_eq!(
        rules(&found),
        [Rule::UnknownDependency].into(),
        "{}",
        show(&found)
    );
}

#[test]
fn table_self_and_repeated_dependency() {
    let rows = with_rows(ROWS, |rows| {
        set_deps(rows, "geom-c", Deps::Exact("a a b c"));
    });
    let found = check::table(&Table {
        rows: &rows,
        ..TABLE
    });
    assert!(
        rules(&found).contains(&Rule::BadDependencyList),
        "{}",
        show(&found)
    );
}

#[test]
fn table_edge_to_a_higher_layer() {
    let rows = with_rows(ROWS, |rows| set_deps(rows, "geom-u", Deps::Exact("e")));
    let found = check::table(&Table {
        rows: &rows,
        ..TABLE
    });
    assert_eq!(rules(&found), [Rule::LayerOrder].into(), "{}", show(&found));
}

#[test]
fn table_same_layer_edge_not_allowed() {
    let found = check::table(&Table {
        same_layer: &[("engine-geometry", "p")],
        ..TABLE
    });
    assert_eq!(
        rules(&found),
        [Rule::SameLayerNotAllowed].into(),
        "{}",
        show(&found)
    );
}

#[test]
fn table_same_layer_entry_across_layers() {
    let found = check::table(&Table {
        same_layer: &[("c", "b"), ("engine-geometry", "p"), ("b", "a")],
        ..TABLE
    });
    assert_eq!(
        rules(&found),
        [Rule::BadSameLayerEntry].into(),
        "{}",
        show(&found)
    );
}

#[test]
fn table_cycle() {
    let rows = with_rows(ROWS, |rows| set_deps(rows, "geom-b", Deps::Exact("a u c")));
    let found = check::table(&Table {
        rows: &rows,
        same_layer: &[("c", "b"), ("b", "c"), ("engine-geometry", "p")],
        ..TABLE
    });
    assert_eq!(rules(&found), [Rule::Cycle].into(), "{}", show(&found));
    assert!(
        found[0].detail.ends_with("geom-c -> geom-b -> geom-c"),
        "{}",
        show(&found)
    );
}

#[test]
fn table_normal_dependency_on_a_development_crate() {
    let rows = with_rows(ROWS, |rows| {
        set_deps(rows, "geom-c", Deps::Exact("a b dev"));
    });
    let found = check::table(&Table {
        rows: &rows,
        ..TABLE
    });
    assert_eq!(
        rules(&found),
        [Rule::DevelopmentDependency].into(),
        "{}",
        show(&found)
    );
}

#[test]
fn table_pending_must_match_an_edge_to_a_crate_under_way() {
    let wrong_piece = [Pending {
        dep: "u",
        piece: "G99",
        crates: "b",
    }];
    let found = check::table(&Table {
        pending: &wrong_piece,
        ..TABLE
    });
    assert!(
        rules(&found).contains(&Rule::BadPending),
        "{}",
        show(&found)
    );
    let not_listed = [Pending {
        dep: "u",
        piece: "G90",
        crates: "c",
    }];
    let found = check::table(&Table {
        pending: &not_listed,
        ..TABLE
    });
    assert!(
        rules(&found).contains(&Rule::BadPending),
        "{}",
        show(&found)
    );
}

#[test]
fn table_feature_gate_must_match_an_edge() {
    for gate in [("a", "threads"), ("p", "default"), ("p", "")] {
        let gates = [FeatureGate {
            krate: "engine-geometry",
            dep: gate.0,
            feature: gate.1,
        }];
        let found = check::table(&Table {
            feature_gates: &gates,
            ..TABLE
        });
        assert_eq!(
            rules(&found),
            [Rule::BadFeatureGate].into(),
            "{gate:?}:{}",
            show(&found)
        );
    }
}

#[test]
fn table_header_gap_only_for_crates_written_elsewhere() {
    let gaps = [HeaderGap {
        krate: "a",
        field: Field::Layer,
        reason: "no reason",
    }];
    let found = check::table(&Table {
        header_gaps: &gaps,
        ..TABLE
    });
    assert_eq!(
        rules(&found),
        [Rule::BadHeaderGap].into(),
        "{}",
        show(&found)
    );
}

// ---------------------------------------------------------- the workspace against the table

#[test]
fn unknown_crate_of_the_family() {
    let mut observed = good();
    let stray = observed.packages["geom-a"].clone();
    observed.packages.insert("geom-zz".to_owned(), stray);
    expect_rules(&TABLE, &observed, &[Rule::UnknownCrate]);
}

#[test]
fn folder_without_a_crate() {
    let mut observed = good();
    observed.geometry_dirs.insert("geom-ghost".to_owned());
    expect_rules(&TABLE, &observed, &[Rule::OrphanFolder]);
}

#[test]
fn crate_in_the_wrong_folder() {
    let mut observed = good();
    package_mut(&mut observed, "geom-a").dir = geometry("geom-a-old")
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    expect_rules(&TABLE, &observed, &[Rule::WrongFolder, Rule::OrphanFolder]);
}

#[test]
fn scaffolded_crate_missing() {
    let mut observed = good();
    observed.packages.remove("geom-a");
    observed.geometry_dirs.remove("geom-a");
    // Its dependants still declare it, so only the missing crate itself is reported.
    expect_rules(&TABLE, &observed, &[Rule::MissingCrate]);
}

#[test]
fn deferred_crate_present() {
    let mut observed = good();
    let mut k = observed.packages["geom-a"].clone();
    k.dir = geometry("geom-k").iter().map(|s| (*s).to_owned()).collect();
    k.header = header(Some("11"), false, &[], true);
    k.deps = vec![dep("geom-c", DepKind::Normal)];
    observed.packages.insert("geom-k".to_owned(), k);
    observed.geometry_dirs.insert("geom-k".to_owned());
    expect_rules(&TABLE, &observed, &[Rule::DeferredPresent]);
}

#[test]
fn library_not_at_src_lib_rs() {
    let mut observed = good();
    package_mut(&mut observed, "geom-a").lib = Some(vec!["lib.rs".to_owned()]);
    expect_rules(&TABLE, &observed, &[Rule::NoLibrary]);
}

#[test]
fn every_header_field_is_required() {
    for (field, prefix) in [
        (Field::Job, "One job"),
        (Field::Layer, "Layer"),
        (Field::Invariants, "Invariants"),
        (Field::Sections, "Sections"),
    ] {
        let mut observed = good();
        let a = package_mut(&mut observed, "geom-a");
        a.header.lines.retain(|l| !l.starts_with(prefix));
        expect_rules(&TABLE, &observed, &[Rule::HeaderMissing(field)]);
    }
}

#[test]
fn header_states_another_layer() {
    let mut observed = good();
    package_mut(&mut observed, "geom-a").header = header(Some("4"), false, &[], true);
    expect_rules(&TABLE, &observed, &[Rule::HeaderWrongLayer]);
}

#[test]
fn header_gap_is_stale_once_filled() {
    let mut observed = good();
    package_mut(&mut observed, "geom-e").header = header(Some("3"), false, &[], false);
    expect_rules(&TABLE, &observed, &[Rule::HeaderGapStale]);
}

#[test]
fn scaffolded_crate_without_status() {
    let mut observed = good();
    package_mut(&mut observed, "geom-a").header = header(Some("1"), false, &[], false);
    expect_rules(&TABLE, &observed, &[Rule::StatusMissing]);
}

#[test]
fn pending_line_missing_and_stale() {
    let mut observed = good();
    package_mut(&mut observed, "geom-b").header = header(Some("2"), false, &[], true);
    expect_rules(&TABLE, &observed, &[Rule::PendingLineMissing]);
    let mut observed = good();
    package_mut(&mut observed, "geom-a").header =
        header(Some("1"), false, &[("geom-u", "G90")], true);
    expect_rules(&TABLE, &observed, &[Rule::PendingLineStale]);
}

#[test]
fn proposal_mark_must_match_the_row() {
    let mut observed = good();
    package_mut(&mut observed, "geom-c").header = header(Some("2"), false, &[], true);
    expect_rules(&TABLE, &observed, &[Rule::OriginMark]);
    let mut observed = good();
    package_mut(&mut observed, "geom-a").header = header(Some("1"), true, &[], true);
    expect_rules(&TABLE, &observed, &[Rule::OriginMark]);
}

#[test]
fn kernel_crate_with_std() {
    let mut observed = good();
    package_mut(&mut observed, "geom-a").attributes.no_std = false;
    expect_rules(&TABLE, &observed, &[Rule::NotNoStd]);
}

#[test]
fn unsafe_code_not_forbidden() {
    let mut observed = good();
    package_mut(&mut observed, "geom-a")
        .attributes
        .forbids_unsafe = false;
    expect_rules(&TABLE, &observed, &[Rule::UnsafeNotForbidden]);
    let mut observed = good();
    package_mut(&mut observed, "engine-geometry")
        .attributes
        .forbids_unsafe = false;
    expect_rules(&TABLE, &observed, &[Rule::UnsafeNotForbidden]);
}

#[test]
fn dependency_missing_from_the_manifest() {
    let mut observed = good();
    package_mut(&mut observed, "geom-c")
        .deps
        .retain(|d| d.name != "geom-b");
    expect_rules(&TABLE, &observed, &[Rule::MissingDependency]);
}

#[test]
fn dependency_the_row_does_not_list() {
    let mut observed = good();
    package_mut(&mut observed, "geom-e")
        .deps
        .push(dep("geom-b", DepKind::Normal));
    expect_rules(&TABLE, &observed, &[Rule::ExtraDependency]);
    // Upwards too: also against the layer rule.
    let mut observed = good();
    package_mut(&mut observed, "geom-b")
        .deps
        .push(dep("geom-e", DepKind::Normal));
    expect_rules(
        &TABLE,
        &observed,
        &[Rule::ExtraDependency, Rule::LayerOrder],
    );
}

#[test]
fn kernel_crate_as_a_build_dependency() {
    // A row's dependency declared only as a build dependency: it is not a normal one, so the row
    // is not met either.
    let mut observed = good();
    let c = package_mut(&mut observed, "geom-c");
    c.deps.retain(|d| d.name != "geom-b");
    c.deps.push(dep("geom-b", DepKind::Build));
    expect_rules(
        &TABLE,
        &observed,
        &[Rule::BuildDependency, Rule::MissingDependency],
    );
    // Upwards: the edge still counts for the layer rule.
    let mut observed = good();
    package_mut(&mut observed, "geom-b")
        .deps
        .push(dep("geom-e", DepKind::Build));
    expect_rules(
        &TABLE,
        &observed,
        &[Rule::BuildDependency, Rule::LayerOrder],
    );
}

#[test]
fn optional_dependency_without_a_gate() {
    let mut observed = good();
    package_mut(&mut observed, "geom-c")
        .deps
        .iter_mut()
        .find(|d| d.name == "geom-b")
        .unwrap()
        .optional = true;
    expect_rules(&TABLE, &observed, &[Rule::OptionalDependency]);
}

#[test]
fn registry_crates_in_a_no_std_kernel_crate() {
    let git = "git+https://example.org/libm?rev=0#0";
    for (krate, extra) in [
        // A crate the table allows nothing.
        ("geom-b", published("libm", DepKind::Normal, CRATES_IO)),
        // A crate the table does not allow.
        ("geom-a", published("hashbrown", DepKind::Normal, CRATES_IO)),
        // The allowed crate, from git or as a build dependency.
        ("geom-a", published("libm", DepKind::Normal, git)),
        ("geom-a", published("cc", DepKind::Build, CRATES_IO)),
    ] {
        let mut observed = good();
        package_mut(&mut observed, krate).deps.push(extra);
        expect_rules(&TABLE, &observed, &[Rule::RegistryDependency]);
    }
    // Development dependencies, std crates and development crates are not this rule's.
    let mut observed = good();
    package_mut(&mut observed, "geom-b")
        .deps
        .push(published("proptest", DepKind::Dev, CRATES_IO));
    package_mut(&mut observed, "engine-geometry")
        .deps
        .push(published("serde", DepKind::Normal, CRATES_IO));
    expect_rules(&TABLE, &observed, &[]);
}

#[test]
fn table_registry_entry_only_for_a_kernel_crate() {
    for registry in [
        &[("engine-geometry", "libm")][..],
        &[("zz", "libm")],
        &[("a", "geom-b")],
    ] {
        let found = check::table(&Table { registry, ..TABLE });
        assert_eq!(
            rules(&found),
            [Rule::BadRegistryEntry].into(),
            "{}",
            show(&found)
        );
    }
}

#[test]
fn kernel_dependency_from_the_registry() {
    let mut observed = good();
    let b = package_mut(&mut observed, "geom-b");
    b.deps[0].path = false;
    expect_rules(&TABLE, &observed, &[Rule::NotAPathDependency]);
}

#[test]
fn kernel_crate_on_a_crate_outside_the_kernel() {
    let mut observed = good();
    package_mut(&mut observed, "geom-a")
        .deps
        .push(dep("kit-core", DepKind::Normal));
    expect_rules(&TABLE, &observed, &[Rule::ExternalDependency]);
}

#[test]
fn development_crate_as_a_normal_dependency() {
    let mut observed = good();
    let c = package_mut(&mut observed, "geom-c");
    c.deps.retain(|d| d.name != "geom-dev");
    c.deps.push(dep("geom-dev", DepKind::Normal));
    expect_rules(
        &TABLE,
        &observed,
        &[Rule::ExtraDependency, Rule::DevelopmentDependency],
    );
}

#[test]
fn pending_crate_has_merged() {
    let mut observed = good();
    let mut u = observed.packages["geom-a"].clone();
    u.deps.clear();
    u.dir = geometry("geom-u").iter().map(|s| (*s).to_owned()).collect();
    u.header = header(Some("1"), true, &[], false);
    observed.packages.insert("geom-u".to_owned(), u);
    observed.geometry_dirs.insert("geom-u".to_owned());
    expect_rules(&TABLE, &observed, &[Rule::PendingMerged]);
    // Added to the manifest, but the entry and the header line are still there.
    package_mut(&mut observed, "geom-b")
        .deps
        .push(dep("geom-u", DepKind::Normal));
    expect_rules(
        &TABLE,
        &observed,
        &[Rule::PendingMerged, Rule::PendingAlreadyAdded],
    );
}

#[test]
fn absent_dependency_not_listed_as_pending() {
    let pending = [PENDING[1]];
    let mut observed = good();
    package_mut(&mut observed, "geom-b").header = header(Some("2"), false, &[], true);
    expect_rules(
        &Table {
            pending: &pending,
            ..TABLE
        },
        &observed,
        &[Rule::AbsentDependency],
    );
}

#[test]
fn feature_gated_dependency() {
    let mut observed = good();
    let mut p = observed.packages["geom-a"].clone();
    p.dir = vec![
        "projects".to_owned(),
        "engines".to_owned(),
        "geometry".to_owned(),
        "crates".to_owned(),
        "geom-p".to_owned(),
    ];
    p.header = header(Some("13"), true, &[], false);
    p.attributes.no_std = false;
    observed.packages.insert("geom-p".to_owned(), p);
    observed.geometry_dirs.insert("geom-p".to_owned());
    // geom-p depends on geom-u, which is absent: list it as pending for this example.
    let pending = [
        PENDING[0],
        PENDING[1],
        Pending {
            dep: "u",
            piece: "G90",
            crates: "p",
        },
    ];
    let table = Table {
        pending: &pending,
        ..TABLE
    };
    let facade = package_mut(&mut observed, "engine-geometry");
    facade.deps.push(dep("geom-p", DepKind::Normal));
    // Declared, but not optional and not switched on by `threads`.
    expect_rules(
        &table,
        &observed,
        &[
            Rule::PendingMerged,
            Rule::PendingAlreadyAdded,
            Rule::FeatureGate,
        ],
    );
    let facade = package_mut(&mut observed, "engine-geometry");
    facade.deps.last_mut().unwrap().optional = true;
    facade
        .features
        .insert("threads".to_owned(), vec!["dep:geom-p".to_owned()]);
    let merged = [Rule::PendingMerged, Rule::PendingAlreadyAdded];
    expect_rules(&table, &observed, &merged);
    let gate = [
        Rule::PendingMerged,
        Rule::PendingAlreadyAdded,
        Rule::FeatureGate,
    ];
    // Still correct: another feature that switches `threads` on, and a weak feature of the
    // dependency, which does not switch the dependency on.
    let features = |list: &[(&str, &[&str])]| -> BTreeMap<String, Vec<String>> {
        list.iter()
            .map(|(f, on)| {
                let on = on.iter().map(|s| (*s).to_owned()).collect();
                ((*f).to_owned(), on)
            })
            .collect()
    };
    let threads: (&str, &[&str]) = ("threads", &["dep:geom-p"]);
    let fine = [
        features(&[threads, ("parallel", &["threads"])]),
        features(&[threads, ("default", &[]), ("x", &["geom-p?/feature"])]),
    ];
    // Broken one way at a time: on by default, directly or through a chain; switched on by
    // another feature, or through the dependency's implicit feature; `threads` reaching it only
    // through another feature.
    let broken = [
        features(&[threads, ("default", &["threads"])]),
        features(&[
            threads,
            ("default", &["parallel"]),
            ("parallel", &["threads"]),
        ]),
        features(&[threads, ("parallel", &["geom-p/feature"])]),
        features(&[threads, ("default", &["dep:geom-p"])]),
        features(&[("threads", &["geom-p"]), ("geom-p", &["dep:geom-p"])]),
        features(&[("threads", &["parallel"]), ("parallel", &["dep:geom-p"])]),
    ];
    for (list, expected) in fine
        .into_iter()
        .map(|f| (f, &merged[..]))
        .chain(broken.into_iter().map(|f| (f, &gate[..])))
    {
        let mut observed = observed.clone();
        package_mut(&mut observed, "engine-geometry").features = list;
        expect_rules(&table, &observed, expected);
    }
    // Declared a second time, without `optional`.
    let mut twice = observed.clone();
    package_mut(&mut twice, "engine-geometry")
        .deps
        .push(dep("geom-p", DepKind::Normal));
    expect_rules(&table, &twice, &gate);
}

#[test]
fn program_depending_on_an_internal_crate() {
    let mut observed = good();
    package_mut(&mut observed, "spare-app")
        .deps
        .push(dep("geom-a", DepKind::Dev));
    expect_rules(&TABLE, &observed, &[Rule::OutsiderDependency]);
}

#[test]
fn workspace_cycle() {
    // Allow b -> c in the table so only the cycle itself is wrong, then declare it.
    let rows = with_rows(ROWS, |rows| set_deps(rows, "geom-b", Deps::Exact("a u c")));
    let table = Table {
        rows: &rows,
        same_layer: &[("c", "b"), ("b", "c"), ("engine-geometry", "p")],
        ..TABLE
    };
    let mut observed = good();
    package_mut(&mut observed, "geom-b")
        .deps
        .push(dep("geom-c", DepKind::Normal));
    let found = check::workspace(&table, &observed);
    assert_eq!(rules(&found), [Rule::Cycle].into(), "{}", show(&found));
}

// ---------------------------------------------------------- the lint posture

fn lint_tables<'o>(observed: &'o mut Observed, name: &str) -> &'o mut BTreeMap<String, String> {
    package_mut(observed, name).lints.tables.as_mut().unwrap()
}

fn clippy_settings<'o>(observed: &'o mut Observed, name: &str) -> &'o mut BTreeMap<String, String> {
    package_mut(observed, name)
        .lints
        .clippy
        .as_mut()
        .unwrap()
        .settings
        .as_mut()
        .unwrap()
}

/// The crates `violations` names.
fn crates(violations: &[Violation]) -> BTreeSet<&str> {
    violations.iter().map(|v| v.krate.as_str()).collect()
}

#[test]
fn table_lint_source_must_be_a_kernel_crate() {
    // A development crate, an unknown crate, the facade and the std pool.
    for lints_from in ["dev", "zz", "engine-geometry", "p"] {
        let found = check::table(&Table {
            lints_from,
            ..TABLE
        });
        assert_eq!(
            rules(&found),
            [Rule::BadLintSource].into(),
            "{lints_from}:{}",
            show(&found)
        );
    }
}

#[test]
fn lint_tables_must_be_the_references_exactly() {
    // A setting left out; a setting added, stricter but still not the reference's; a setting at
    // another level; and a manifest the reader cannot read.
    let mut left_out = good();
    lint_tables(&mut left_out, "geom-a").remove("lints.clippy.pedantic.level");
    let mut added = good();
    lint_tables(&mut added, "geom-c")
        .insert("lints.clippy.float_cmp".to_owned(), "\"deny\"".to_owned());
    let mut looser = good();
    lint_tables(&mut looser, "engine-geometry").insert(
        "lints.clippy.arithmetic_side_effects".to_owned(),
        "\"warn\"".to_owned(),
    );
    let mut unreadable = good();
    package_mut(&mut unreadable, "geom-b").lints.tables = None;
    for (observed, krate) in [
        (left_out, "geom-a"),
        (added, "geom-c"),
        (looser, "engine-geometry"),
        (unreadable, "geom-b"),
    ] {
        let found = check::workspace(&TABLE, &observed);
        assert_eq!(rules(&found), [Rule::LintTables].into(), "{}", show(&found));
        assert_eq!(crates(&found), [krate].into(), "{}", show(&found));
    }
}

#[test]
fn clippy_config_must_hold_the_references_settings_exactly() {
    // No configuration at all; one the reader cannot read; one entry of a disallowed list changed;
    // a setting added.
    let mut none = good();
    package_mut(&mut none, "geom-a").lints.clippy = None;
    let mut unreadable = good();
    package_mut(&mut unreadable, "geom-b")
        .lints
        .clippy
        .as_mut()
        .unwrap()
        .settings = None;
    let mut changed = good();
    clippy_settings(&mut changed, "geom-c").insert(
        "disallowed-methods".to_owned(),
        "[{path=\"f64::min\",reason=\"either zero\"}]".to_owned(),
    );
    let mut added = good();
    clippy_settings(&mut added, "engine-geometry")
        .insert("allow-panic-in-tests".to_owned(), "true".to_owned());
    for (observed, krate) in [
        (none, "geom-a"),
        (unreadable, "geom-b"),
        (changed, "geom-c"),
        (added, "engine-geometry"),
    ] {
        let found = check::workspace(&TABLE, &observed);
        assert_eq!(
            rules(&found),
            [Rule::ClippyConfig].into(),
            "{}",
            show(&found)
        );
        assert_eq!(crates(&found), [krate].into(), "{}", show(&found));
    }
}

#[test]
fn a_change_to_the_references_settings_reaches_every_crate_that_ships() {
    // The reference loosens one lint and drops one disallowed method: every crate that ships now
    // differs from it, and only those.
    let mut observed = good();
    lint_tables(&mut observed, "geom-e").insert(
        "lints.clippy.arithmetic_side_effects".to_owned(),
        "\"allow\"".to_owned(),
    );
    clippy_settings(&mut observed, "geom-e").remove("disallowed-methods");
    let found = check::workspace(&TABLE, &observed);
    assert_eq!(
        rules(&found),
        [Rule::LintTables, Rule::ClippyConfig].into(),
        "{}",
        show(&found)
    );
    let shipping: BTreeSet<&str> = ["geom-a", "geom-b", "geom-c", "engine-geometry"].into();
    assert_eq!(crates(&found), shipping, "{}", show(&found));
}

#[test]
fn the_reference_itself_must_be_readable_and_present() {
    let mut tables = good();
    package_mut(&mut tables, "geom-e").lints.tables = None;
    let mut clippy = good();
    package_mut(&mut clippy, "geom-e").lints.clippy = None;
    let mut unreadable = good();
    package_mut(&mut unreadable, "geom-e")
        .lints
        .clippy
        .as_mut()
        .unwrap()
        .settings = None;
    let mut absent = good();
    absent.packages.remove("geom-e");
    absent.geometry_dirs.remove("geom-e");
    for (observed, rule) in [
        (tables, Rule::LintTables),
        (clippy, Rule::ClippyConfig),
        (unreadable, Rule::ClippyConfig),
        (absent, Rule::LintTables),
    ] {
        let found = check::workspace(&TABLE, &observed);
        assert_eq!(rules(&found), [rule].into(), "{}", show(&found));
        assert_eq!(crates(&found), ["geom-e"].into(), "{}", show(&found));
    }
}

#[test]
fn development_crates_and_outsiders_keep_their_own_lints() {
    // Settings of their own, or none, on a development crate and on the crates outside the
    // kernel: the posture is for the crates that ship.
    let mut observed = good();
    for name in ["geom-dev", "kit-core", "spare-app"] {
        package_mut(&mut observed, name).lints = Lints {
            tables: Some(settings(&[(
                "lints.clippy.arithmetic_side_effects",
                "\"allow\"",
            )])),
            clippy: None,
        };
    }
    expect_rules(&TABLE, &observed, &[]);
}

// ---------------------------------------------------------- the toolchain pins

const TOOLCHAIN: &str = "[toolchain]\nchannel = \"1.99.0\"\n\
    components = [\"rustfmt\", \"clippy\"]\n\
    targets = [\"wasm32-wasip1\", \"wasm32-unknown-unknown\"]\n";

const PINNED: &str = "env:\n  RUST_TOOLCHAIN: \"1.99.0\"\njobs:\n  a:\n    steps:\n\
    \x20     - uses: dtolnay/rust-toolchain@master\n\
    \x20       with:\n\
    \x20         toolchain: ${{ env.RUST_TOOLCHAIN }}\n";

#[test]
fn the_toolchain_example_passes() {
    let workflows = [
        ("kernel.yml", PINNED),
        ("kernel-build.yml", PINNED),
        ("site.yml", "jobs: {}\n"),
    ];
    let found = toolchain::pins(TOOLCHAIN, &workflows);
    assert!(found.is_empty(), "{}", show(&found));
}

#[test]
fn toolchain_pins_that_disagree() {
    let other = PINNED.replace("1.99.0", "1.98.0");
    let twice = format!("{PINNED}  RUST_TOOLCHAIN: \"1.99.0\"\n");
    let hard_coded = PINNED.replace("${{ env.RUST_TOOLCHAIN }}", "stable");
    let both: [(&str, &str); 2] = [("kernel.yml", PINNED), ("kernel-build.yml", PINNED)];
    let cases: [(String, [(&str, &str); 2]); 8] = [
        // The file names no exact release, or lacks a component or a target, or is not TOML.
        (TOOLCHAIN.replace("\"1.99.0\"", "\"stable\""), both),
        (TOOLCHAIN.replace("\"clippy\"", "\"rust-src\""), both),
        (TOOLCHAIN.replace(", \"wasm32-unknown-unknown\"", ""), both),
        ("[toolchain\n".to_owned(), both),
        // A workflow pins another release, pins twice or installs another toolchain; a workflow
        // that must pin does not.
        (
            TOOLCHAIN.to_owned(),
            [("kernel.yml", PINNED), ("kernel-build.yml", other.as_str())],
        ),
        (
            TOOLCHAIN.to_owned(),
            [("kernel.yml", twice.as_str()), ("kernel-build.yml", PINNED)],
        ),
        (
            TOOLCHAIN.to_owned(),
            [
                ("kernel.yml", PINNED),
                ("kernel-build.yml", hard_coded.as_str()),
            ],
        ),
        (
            TOOLCHAIN.to_owned(),
            [("kernel.yml", PINNED), ("other.yml", PINNED)],
        ),
    ];
    for (file, workflows) in &cases {
        let found = toolchain::pins(file, workflows);
        assert_eq!(
            rules(&found),
            [Rule::ToolchainPin].into(),
            "{file}\n{workflows:?}{}",
            show(&found)
        );
    }
}
