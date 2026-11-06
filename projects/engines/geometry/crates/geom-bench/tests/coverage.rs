//! The gates measure exactly what the catalogue lists, the catalogue covers every public function
//! of `geom-num`, and the limits are the ones principle 12 sets, wherever they are written.
//!
//! The instruction-count gate is Linux only and built from Gungraun's attribute macros, so it
//! cannot share the catalogue's data directly. Instead these tests read the gate's source and check
//! that its benchmarks are exactly the catalogue's, that each Gungraun group lists every benchmark
//! function in its module, and that `main.rs` runs every group. They read `geom-num`'s source and
//! check that every public function has a benchmark or a reasoned entry in
//! `catalogue::UNMEASURED` (principle 12). And they read the CI workflow and check it passes the
//! limits, the toolchain and the build flags the crate documents. They run on every platform, so
//! nothing can be added to one place and forgotten in another.
//!
//! Guarantees, when these tests pass: the gate, `main.rs`, the catalogue and the golden check list
//! the same benchmarks; every public function of `geom-num` is measured or excused with a reason,
//! and every function excused as a constant refusal still refuses;
//! the limits are principle 12's in the crate, the gate and the workflow alike; the toolchain,
//! Gungraun and Wasmtime pins agree wherever they are written; every action the workflow runs is
//! pinned to the full commit `kernel.yml` runs, and no checkout keeps credentials; and no retired
//! name is reused.
//!
//! Method: a traceability check (<https://en.wikipedia.org/wiki/Traceability_matrix>): each check
//! reads the files it compares and requires the sets they list to be equal. `geom-num`'s source is
//! parsed by the `syn` crate, so a public function is found wherever and however it is written,
//! and the manifests and `Cargo.lock` by the `toml` crate (both MIT OR Apache-2.0, development
//! dependencies; piece G12a put them in place of a line scanner, which found `pub fn` only at the
//! indents rustfmt gives, and of text searches; chapter 29.1). The gate's source and the workflows
//! are read as text, line by line, so a checkout with either line ending reads the same.
//!
//! Cost: a linear parse of a few thousand lines of source.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a source, manifest or lock file the readers cannot read
// must stop the test loudly.
#![allow(clippy::expect_used, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};

use geom_bench::catalogue::{GROUPS, GROWTH, GROWTH_GROUP, OPS, UNMEASURED, names};
use geom_bench::harness::ZERO_BEFORE;
use geom_bench::{
    CALLGRIND_LIMITS, DHAT_LIMITS, GATE_PERCENT, GATED_HEAP_METRICS, GATED_METRICS,
    GUNGRAUN_VERSION, LIST_PERCENT, TOOLCHAIN, WASMTIME_VERSION,
};

const GATE: &str = include_str!("../benches/instructions/gate.rs");
const MAIN: &str = include_str!("../benches/instructions/main.rs");
const MANIFEST: &str = include_str!("../Cargo.toml");
const FUEL_MANIFEST: &str = include_str!("../fuel/Cargo.toml");
const FUEL_MAIN: &str = include_str!("../fuel/src/main.rs");
const RETIRED: &str = include_str!("../retired.txt");
const WORKFLOW: &str = include_str!("../../../../../../.github/workflows/kernel-bench.yml");
/// The workspace lock file, read at compile time so the test needs no file access when it runs
/// (wasm32-wasip1 under Wasmtime has none).
const LOCK: &str = include_str!("../../../../../../Cargo.lock");
const KERNEL_WORKFLOW: &str = include_str!("../../../../../../.github/workflows/kernel.yml");

/// The source of every module of `geom-num`, by module name.
/// This crate's own reference, Avnaim et al.'s signs (chapter 23.3, piece G42): measured beside
/// `geom-num`'s exact paths, and not a `geom-num` function.
const AVNAIM: &str = include_str!("../src/avnaim.rs");
const GEOM_NUM: [(&str, &str); 30] = [
    ("quadric", include_str!("../../geom-num/src/quadric.rs")),
    ("sturm", include_str!("../../geom-num/src/sturm.rs")),
    ("ball", include_str!("../../geom-num/src/ball.rs")),
    ("brent", include_str!("../../geom-num/src/brent.rs")),
    ("dd", include_str!("../../geom-num/src/dd.rs")),
    ("dual", include_str!("../../geom-num/src/dual.rs")),
    ("eft", include_str!("../../geom-num/src/eft.rs")),
    ("error", include_str!("../../geom-num/src/error.rs")),
    ("exact_sum", include_str!("../../geom-num/src/exact_sum.rs")),
    (
        "generated",
        include_str!("../../geom-num/src/generated/mod.rs"),
    ),
    ("exactness", include_str!("../../geom-num/src/exactness.rs")),
    ("grid", include_str!("../../geom-num/src/grid.rs")),
    ("interval", include_str!("../../geom-num/src/interval.rs")),
    ("isolate", include_str!("../../geom-num/src/isolate.rs")),
    ("ladder", include_str!("../../geom-num/src/ladder.rs")),
    ("lib", include_str!("../../geom-num/src/lib.rs")),
    ("math", include_str!("../../geom-num/src/math.rs")),
    ("newton", include_str!("../../geom-num/src/newton.rs")),
    ("perturb", include_str!("../../geom-num/src/perturb.rs")),
    ("plane", include_str!("../../geom-num/src/plane.rs")),
    ("crossing", include_str!("../../geom-num/src/crossing.rs")),
    ("predicate", include_str!("../../geom-num/src/predicate.rs")),
    (
        "quadrature",
        include_str!("../../geom-num/src/quadrature.rs"),
    ),
    (
        "quadrature_rules",
        include_str!("../../geom-num/src/quadrature_rules/mod.rs"),
    ),
    ("real", include_str!("../../geom-num/src/real.rs")),
    ("rng", include_str!("../../geom-num/src/rng.rs")),
    ("scalar", include_str!("../../geom-num/src/scalar.rs")),
    ("sign", include_str!("../../geom-num/src/sign.rs")),
    ("wide", include_str!("../../geom-num/src/wide.rs")),
    ("wide32", include_str!("../../geom-num/src/wide32.rs")),
];

/// The files inside `geom-num`'s `math` module that hold public functions, by name: `unproven`, a
/// public module, whose free functions are `unproven::name`; and `enclose`, a private module whose
/// `impl Interval` adds public methods (`Interval::sinpi` and the rest).
const MATH_PARTS: [(&str, &str); 2] = [
    (
        "unproven",
        include_str!("../../geom-num/src/math/unproven.rs"),
    ),
    (
        "enclose",
        include_str!("../../geom-num/src/math/enclose.rs"),
    ),
];

/// The private modules of `geom-num`'s `math` that hold only crate-internal code: the proven
/// functions' ports and their support. Their `pub` items are not reachable from outside the crate,
/// so they are not read; a new module of `math` fails here until it is listed in one of the two.
const MATH_PRIVATE: [&str; 6] = ["atan2", "exp", "hexf", "log", "sincospi", "support"];

/// The modules of `geom-num`'s `generated`, one per predicate (piece G27), by path; their free
/// functions are `generated::<predicate>::<function>`.
const GENERATED_PARTS: [(&str, &str); 19] = [
    (
        "generated::line_sphere",
        include_str!("../../geom-num/src/generated/line_sphere.rs"),
    ),
    (
        "generated::line_cylinder",
        include_str!("../../geom-num/src/generated/line_cylinder.rs"),
    ),
    (
        "generated::line_cone",
        include_str!("../../geom-num/src/generated/line_cone.rs"),
    ),
    (
        "generated::line_cone_lead",
        include_str!("../../geom-num/src/generated/line_cone_lead.rs"),
    ),
    (
        "generated::cross_norm",
        include_str!("../../geom-num/src/generated/cross_norm.rs"),
    ),
    (
        "generated::axis_gap",
        include_str!("../../geom-num/src/generated/axis_gap.rs"),
    ),
    (
        "generated::orient2d",
        include_str!("../../geom-num/src/generated/orient2d.rs"),
    ),
    (
        "generated::orient3d",
        include_str!("../../geom-num/src/generated/orient3d.rs"),
    ),
    (
        "generated::incircle",
        include_str!("../../geom-num/src/generated/incircle.rs"),
    ),
    (
        "generated::insphere",
        include_str!("../../geom-num/src/generated/insphere.rs"),
    ),
    (
        "generated::plane_side",
        include_str!("../../geom-num/src/generated/plane_side.rs"),
    ),
    (
        "generated::tpi_weight",
        include_str!("../../geom-num/src/generated/tpi_weight.rs"),
    ),
    (
        "generated::tpi_side",
        include_str!("../../geom-num/src/generated/tpi_side.rs"),
    ),
    (
        "generated::lpi_weight",
        include_str!("../../geom-num/src/generated/lpi_weight.rs"),
    ),
    (
        "generated::lpi_side",
        include_str!("../../geom-num/src/generated/lpi_side.rs"),
    ),
    (
        "generated::ssi_weight",
        include_str!("../../geom-num/src/generated/ssi_weight.rs"),
    ),
    (
        "generated::ssi_side",
        include_str!("../../geom-num/src/generated/ssi_side.rs"),
    ),
    (
        "generated::plain",
        include_str!("../../geom-num/src/generated/plain.rs"),
    ),
    (
        "generated::limbs",
        include_str!("../../geom-num/src/generated/limbs.rs"),
    ),
];

/// The identifier at the start of `s`.
fn ident(s: &str) -> String {
    s.chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect()
}

/// A comma-separated list of identifiers.
fn idents(s: &str) -> BTreeSet<String> {
    s.split(',')
        .map(|item| ident(item.trim()))
        .filter(|item| !item.is_empty())
        .collect()
}

/// What the gate's source defines.
struct Gate {
    /// Every benchmark as (module, function, bench id), with the name passed to `leak`.
    benchmarks: BTreeMap<(String, String, String), String>,
    /// The benchmark functions of each module.
    functions: BTreeMap<String, BTreeSet<String>>,
    /// Each module's Gungraun group: its name and function list.
    groups: BTreeMap<String, (String, BTreeSet<String>)>,
    /// Benchmark functions without `teardown = verify`.
    unverified: Vec<String>,
}

fn parse_gate() -> Gate {
    let mut gate = Gate {
        benchmarks: BTreeMap::new(),
        functions: BTreeMap::new(),
        groups: BTreeMap::new(),
        unverified: Vec::new(),
    };
    let mut module = String::new();
    let mut pending: Vec<(String, String)> = Vec::new();
    let mut verified = false;
    let mut group_text: Option<String> = None;
    for line in GATE.lines() {
        let line = line.trim();
        if let Some(text) = group_text.as_mut() {
            text.push(' ');
            text.push_str(line);
            if line.ends_with(");") {
                let text = group_text.take().unwrap_or_default();
                let name = text
                    .split("name =")
                    .nth(1)
                    .map(|s| ident(s.trim()))
                    .unwrap_or_default();
                let list = text.split("benchmarks =").nth(1).unwrap_or_default();
                let list = idents(list.trim_end_matches(");"));
                gate.groups.insert(module.clone(), (name, list));
            }
        } else if let Some(rest) = line.strip_prefix("pub mod ") {
            module = ident(rest);
        } else if line.starts_with("#[library_benchmark") {
            verified = line == "#[library_benchmark(teardown = verify)]";
        } else if let Some(rest) = line.strip_prefix("#[bench::") {
            let id = ident(rest);
            let leaked = rest
                .split("leak(\"")
                .nth(1)
                .and_then(|s| s.split('"').next());
            pending.push((id, leaked.unwrap_or_default().to_owned()));
        } else if let Some(rest) = line.strip_prefix("fn ") {
            let name = ident(rest);
            if !pending.is_empty() {
                gate.functions
                    .entry(module.clone())
                    .or_default()
                    .insert(name.clone());
                if !verified {
                    gate.unverified.push(format!("{module}::{name}"));
                }
                assert!(
                    line.contains("(run: &'static mut Named) -> &'static mut Named"),
                    "{module}::{name} must take and return the leaked benchmark, so nothing is \
                     freed while counting"
                );
            }
            for (id, leaked) in pending.drain(..) {
                let key = (module.clone(), name.clone(), id);
                let new = gate.benchmarks.insert(key, leaked).is_none();
                assert!(
                    new,
                    "a benchmark is defined twice in gate.rs: {module}::{name}"
                );
            }
            verified = false;
        } else if line.starts_with("library_benchmark_group!(") {
            group_text = Some(String::new());
        }
    }
    assert!(
        pending.is_empty(),
        "#[bench] lines with no benchmark function after them"
    );
    gate
}

#[test]
fn the_gate_has_exactly_the_catalogued_benchmarks() {
    let gate = parse_gate();
    let gated: BTreeSet<String> = gate
        .benchmarks
        .iter()
        .map(|((module, function, id), leaked)| {
            let name = format!("{module}/{function}/{id}");
            assert_eq!(
                *leaked, name,
                "the bench id and the name it builds must agree"
            );
            name
        })
        .collect();
    let catalogue: BTreeSet<String> = names().into_iter().collect();
    let missing: Vec<_> = catalogue.difference(&gated).collect();
    let extra: Vec<_> = gated.difference(&catalogue).collect();
    assert!(
        missing.is_empty(),
        "in the catalogue but not gated: {missing:?}"
    );
    assert!(
        extra.is_empty(),
        "gated but not in the catalogue: {extra:?}"
    );
    assert_eq!(gated.len(), names().len());
    let unverified = &gate.unverified;
    assert!(
        unverified.is_empty(),
        "benchmarks without the golden check: {unverified:?}"
    );
}

#[test]
fn every_group_runs_every_function_in_its_module() {
    let gate = parse_gate();
    let modules: BTreeSet<String> = gate.functions.keys().cloned().collect();
    let mut expected: BTreeSet<String> = GROUPS.iter().map(|g| (*g).to_owned()).collect();
    expected.insert(GROWTH_GROUP.to_owned());
    assert_eq!(modules, expected);
    for (module, functions) in &gate.functions {
        let Some((name, listed)) = gate.groups.get(module) else {
            panic!("no Gungraun group in module {module}");
        };
        assert_eq!(
            name, module,
            "the group in module {module} must have the module's name"
        );
        assert_eq!(
            listed, functions,
            "group {module} must list every benchmark function"
        );
    }
}

#[test]
fn main_runs_every_group() {
    let groups = MAIN
        .split("library_benchmark_groups =")
        .nth(1)
        .and_then(|rest| rest.split(')').next())
        .map(idents)
        .unwrap_or_default();
    let mut expected: BTreeSet<String> = GROUPS.iter().map(|g| (*g).to_owned()).collect();
    expected.insert(GROWTH_GROUP.to_owned());
    assert_eq!(groups, expected);
    assert!(MAIN.contains("config = gate::config();"));
}

#[test]
fn catalogue_names_are_unique_and_grouped() {
    let all = names();
    let unique: BTreeSet<&String> = all.iter().collect();
    assert_eq!(unique.len(), all.len());
    for op in OPS {
        assert!(GROUPS.contains(&op.group), "{} is in no group", op.name);
        assert!(!op.classes.is_empty(), "{} has no classes", op.name);
    }
    for group in GROUPS {
        assert!(
            OPS.iter().any(|op| op.group == group),
            "group {group} is empty"
        );
    }
}

/// A `geom-num` source file, parsed by `syn`.
fn parse(module: &str, source: &str) -> syn::File {
    syn::parse_file(source).unwrap_or_else(|e| panic!("geom-num's {module} does not parse: {e}"))
}

/// Whether an item's attributes make it test or proof code: `#[cfg(test)]`, or `#[cfg(kani)]`
/// (the Kani harnesses beside the code, chapter 28.7), which no user can call.
fn is_test(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        a.path().is_ident("cfg")
            && a.parse_args::<syn::Ident>()
                .is_ok_and(|word| word == "test" || word == "kani")
    })
}

/// Whether an item is `pub` (not `pub(crate)` or narrower, which the crate's users cannot call).
fn is_pub(vis: &syn::Visibility) -> bool {
    matches!(vis, syn::Visibility::Public(_))
}

/// The modules a file declares, outside test code: its `pub` ones, or all of them.
fn modules(file: &syn::File, public_only: bool) -> BTreeSet<String> {
    file.items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Mod(m) if !is_test(&m.attrs) && (!public_only || is_pub(&m.vis)) => {
                Some(m.ident.to_string())
            }
            _ => None,
        })
        .collect()
}

/// The name of the type an `impl` block is for: the last segment of its path, without generic
/// arguments (`Wide<L>` is `Wide`); `None` for any other kind of type.
fn type_name(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(path) => path.path.segments.last().map(|s| s.ident.to_string()),
        _ => None,
    }
}

/// The public functions a `geom-num` source file defines: `module::function` for free functions
/// (`module::inner::function` inside a `pub mod inner { ... }`), `Type::method` for public
/// methods, and `Type::cmp` for an `Ord` implementation. Test code is skipped. Explicit work list,
/// no recursion.
fn public_functions(module: &str, source: &str) -> Vec<String> {
    let file = parse(module, source);
    let mut out = Vec::new();
    let mut work: Vec<(String, &[syn::Item])> = vec![(module.to_owned(), &file.items)];
    while let Some((path, items)) = work.pop() {
        for item in items {
            match item {
                syn::Item::Fn(f) if !is_test(&f.attrs) && is_pub(&f.vis) => {
                    out.push(format!("{path}::{}", f.sig.ident));
                }
                syn::Item::Impl(block) if !is_test(&block.attrs) => {
                    let Some(ty) = type_name(&block.self_ty) else {
                        continue;
                    };
                    let ord = block.trait_.as_ref().is_some_and(|(tr, _)| {
                        tr.segments.last().is_some_and(|s| s.ident == "Ord")
                    });
                    for inner in &block.items {
                        let syn::ImplItem::Fn(f) = inner else {
                            continue;
                        };
                        if is_test(&f.attrs) {
                            continue;
                        }
                        if is_pub(&f.vis) || (ord && f.sig.ident == "cmp") {
                            out.push(format!("{ty}::{}", f.sig.ident));
                        }
                    }
                }
                syn::Item::Mod(m) if !is_test(&m.attrs) && is_pub(&m.vis) => {
                    if let Some((_, inner)) = &m.content {
                        work.push((format!("{path}::{}", m.ident), inner));
                    }
                }
                _ => {}
            }
        }
    }
    out
}

#[test]
fn the_parser_finds_every_public_function_however_it_is_written() {
    // What the line scanner it replaced missed: a function on one line with its block, `pub fn`
    // written at another indent, a public function in a public inline module, attributes and doc
    // comments between, and an `impl` whose header spans lines.
    let source = "//! A module.\n\
        pub fn one() {}\n\
        #[inline]\n/// Two.\npub const fn two() -> u8 { 2 }\n\
        fn private() {}\n\
        pub(crate) fn crate_only() {}\n\
        pub mod inner { pub fn three() {} fn hidden() {} }\n\
        impl<const L: usize>\n    Wide<L>\n{\n        pub fn four(&self) {}\n    fn five() {}\n}\n\
        impl core::cmp::Ord for Sign { fn cmp(&self, o: &Self) -> Ordering { todo!() } }\n\
        impl Clone for Sign { fn clone(&self) -> Self { *self } }\n\
        #[cfg(test)]\nmod tests { pub fn test_only() {} }\n\
        #[cfg(test)]\npub fn also_test_only() {}\n\
        #[cfg(kani)]\nmod proofs { pub fn proof_only() {} }\n";
    let mut found = public_functions("m", source);
    found.sort();
    assert_eq!(
        found,
        [
            "Sign::cmp",
            "Wide::four",
            "m::inner::three",
            "m::one",
            "m::two"
        ]
    );
    let file = parse("m", source);
    assert_eq!(modules(&file, true), BTreeSet::from(["inner".to_owned()]));
    assert_eq!(modules(&file, false), BTreeSet::from(["inner".to_owned()]));
}

/// The other kernel crates whose hot paths the catalogue measures (decision 32.44: a count
/// benchmark for every phase-2 code piece), each with the source files that define the functions
/// it measures there, by module name.
const OTHER_CRATES: &[(&str, &[(&str, &str)])] = &[
    (
        "geom_intersect",
        &[
            ("curve", include_str!("../../geom-intersect/src/curve.rs")),
            (
                "curve_surface",
                include_str!("../../geom-intersect/src/curve_surface.rs"),
            ),
            ("order", include_str!("../../geom-intersect/src/order.rs")),
            (
                "quadrics",
                include_str!("../../geom-intersect/src/quadrics.rs"),
            ),
            (
                "project",
                include_str!("../../geom-intersect/src/project.rs"),
            ),
            ("result", include_str!("../../geom-intersect/src/result.rs")),
        ],
    ),
    (
        "geom_math",
        &[
            ("typed", include_str!("../../geom-math/src/typed.rs")),
            ("rotation", include_str!("../../geom-math/src/rotation.rs")),
            ("convert", include_str!("../../geom-math/src/convert.rs")),
            ("decimal", include_str!("../../geom-math/src/decimal.rs")),
        ],
    ),
    (
        "geom_linalg",
        &[
            ("qr", include_str!("../../geom-linalg/src/qr.rs")),
            ("svd", include_str!("../../geom-linalg/src/svd.rs")),
        ],
    ),
    (
        "geom_bvh",
        &[
            ("tree", include_str!("../../geom-bvh/src/tree.rs")),
            ("cells", include_str!("../../geom-bvh/src/cells.rs")),
        ],
    ),
    (
        "geom_nurbs",
        &[
            ("curve", include_str!("../../geom-nurbs/src/curve.rs")),
            ("interp", include_str!("../../geom-nurbs/src/interp.rs")),
            (
                "bernstein",
                include_str!("../../geom-nurbs/src/bernstein.rs"),
            ),
        ],
    ),
    (
        "geom_curve",
        &[
            ("param", include_str!("../../geom-curve/src/param.rs")),
            (
                "transition",
                include_str!("../../geom-curve/src/transition.rs"),
            ),
        ],
    ),
    (
        "geom_surface",
        &[("param", include_str!("../../geom-surface/src/param.rs"))],
    ),
];

/// [`public_functions`], with the methods of every trait implementation in the file too: a kind's
/// evaluation is its trait's (`impl ParamCurve<D> for CurveKind<D>`), and a trait's methods are as
/// public as the trait.
fn crate_functions(module: &str, source: &str) -> Vec<String> {
    let mut out = public_functions(module, source);
    for item in &parse(module, source).items {
        // A public trait's methods with a default body, which every implementing kind runs.
        if let syn::Item::Trait(tr) = item
            && is_pub(&tr.vis)
            && !is_test(&tr.attrs)
        {
            for inner in &tr.items {
                if let syn::TraitItem::Fn(f) = inner
                    && f.default.is_some()
                {
                    out.push(format!("{}::{}", tr.ident, f.sig.ident));
                }
            }
            continue;
        }
        let syn::Item::Impl(block) = item else {
            continue;
        };
        if block.trait_.is_none() || is_test(&block.attrs) {
            continue;
        }
        let Some(ty) = type_name(&block.self_ty) else {
            continue;
        };
        for inner in &block.items {
            if let syn::ImplItem::Fn(f) = inner {
                out.push(format!("{ty}::{}", f.sig.ident));
            }
        }
    }
    out
}

/// The crate path and the rest of a measured name that names another kernel crate's function
/// (`geom_math::TypedTransform::apply`), or `None` for a `geom-num` one.
fn other_crate(measured: &str) -> Option<(&'static str, &str)> {
    OTHER_CRATES.iter().find_map(|(krate, _)| {
        measured
            .strip_prefix(krate)
            .and_then(|rest| rest.strip_prefix("::"))
            .map(|rest| (*krate, rest))
    })
}

/// Every function the catalogue measures in another kernel crate is a public function of that
/// crate, read from its source as `Type::method` or `module::function`, so a renamed or removed
/// function fails here, as a stale `geom-num` entry fails the test below.
#[test]
fn every_function_measured_in_another_crate_is_public_there() {
    let mut seen = 0;
    for op in OPS {
        let Some((krate, name)) = other_crate(op.measures) else {
            continue;
        };
        seen += 1;
        let (_, files) = OTHER_CRATES.iter().find(|(k, _)| *k == krate).unwrap();
        let public: BTreeSet<String> = files
            .iter()
            .flat_map(|(m, src)| crate_functions(m, src))
            .collect();
        assert!(
            public.contains(name),
            "{}/{}: {} is not a public function of {krate}",
            op.group,
            op.name,
            op.measures
        );
    }
    assert!(seen > 0, "no benchmark measures another kernel crate");
}

#[test]
fn every_public_function_of_geom_num_is_measured_or_says_why_not() {
    // Principle 12: every operation ships with a benchmark.
    let (_, lib) = GEOM_NUM.iter().find(|(m, _)| *m == "lib").unwrap();
    let declared = modules(&parse("lib", lib), true);
    let read: BTreeSet<String> = GEOM_NUM
        .iter()
        .map(|(m, _)| (*m).to_owned())
        .filter(|m| m != "lib")
        .collect();
    assert_eq!(declared, read, "a geom-num module this test does not read");
    // Every module the maths module declares is read or listed as crate-internal.
    let (_, math) = GEOM_NUM.iter().find(|(m, _)| *m == "math").unwrap();
    let math_modules = modules(&parse("math", math), false);
    let classified: BTreeSet<String> = MATH_PARTS
        .iter()
        .map(|(m, _)| *m)
        .chain(MATH_PRIVATE)
        .map(str::to_owned)
        .collect();
    assert_eq!(
        math_modules, classified,
        "a module of geom-num's math this test does not know"
    );
    // Every module of `generated` is read.
    let (_, generated) = GEOM_NUM.iter().find(|(m, _)| *m == "generated").unwrap();
    let generated_modules: BTreeSet<String> = modules(&parse("generated", generated), true)
        .into_iter()
        .map(|m| format!("generated::{m}"))
        .collect();
    let generated_read: BTreeSet<String> = GENERATED_PARTS
        .iter()
        .map(|(m, _)| (*m).to_owned())
        .collect();
    assert_eq!(
        generated_modules, generated_read,
        "a module of geom-num's generated this test does not read"
    );
    let public: BTreeSet<String> = GEOM_NUM
        .iter()
        .chain(MATH_PARTS.iter())
        .chain(GENERATED_PARTS.iter())
        .flat_map(|(m, src)| public_functions(m, src))
        .collect();
    assert!(public.len() > 60, "the parser found only {public:?}");
    // Another kernel crate's functions (decision 32.44) are named with their crate's path and
    // checked by `every_function_measured_in_another_crate_is_public_there`.
    let measured: BTreeSet<String> = OPS
        .iter()
        .map(|op| op.measures.to_owned())
        .chain(GROWTH.iter().map(|g| g.measures.to_owned()))
        .filter(|m| other_crate(m).is_none())
        .collect();
    let unmeasured: BTreeSet<String> = UNMEASURED.iter().map(|(f, _)| (*f).to_owned()).collect();
    assert!(
        measured.is_disjoint(&unmeasured),
        "{:?}",
        measured.intersection(&unmeasured)
    );
    // The reference this crate measures beside geom-num's paths names its own public functions.
    let references: BTreeSet<String> = public_functions("avnaim", AVNAIM).into_iter().collect();
    let measured_references: BTreeSet<String> =
        measured.intersection(&references).cloned().collect();
    assert!(
        !measured_references.is_empty(),
        "the reference has no benchmark: {references:?}"
    );
    let measured: BTreeSet<String> = measured.difference(&references).cloned().collect();
    let covered: BTreeSet<String> = measured.union(&unmeasured).cloned().collect();
    let uncovered: Vec<_> = public.difference(&covered).collect();
    assert!(
        uncovered.is_empty(),
        "public geom-num functions with no benchmark: {uncovered:?}"
    );
    let stale: Vec<_> = covered.difference(&public).collect();
    assert!(
        stale.is_empty(),
        "catalogue entries naming no geom-num function: {stale:?}"
    );
    assert!(UNMEASURED.iter().all(|(_, why)| !why.is_empty()));
}

/// The interval functions `catalogue::UNMEASURED` excuses as constants, because each refuses every
/// input with `UnprovenMaths` until its enclosure is built: they must still refuse every interval,
/// from every typical and extreme input of the interval benchmarks, or the excuse is stale and the
/// function needs its benchmark. A new entry with that reason fails here until it is listed.
#[test]
fn every_function_excused_as_a_constant_refusal_still_refuses() {
    use geom_bench::inputs::Class::{Extreme, Typical};
    use geom_bench::inputs::interval as i;
    use geom_num::NumError;
    use geom_num::interval::Interval;
    let mut intervals: Vec<Interval> = Vec::new();
    for class in [Typical, Extreme] {
        for (x, y) in i::add(class) {
            intervals.extend([x, y]);
        }
        intervals.extend(i::sqrt(class));
    }
    assert!(intervals.len() > 300);
    let refusals: Vec<&str> = UNMEASURED
        .iter()
        .filter(|(_, why)| why.starts_with("a constant: always Err(UnprovenMaths)"))
        .map(|(f, _)| *f)
        .collect();
    assert_eq!(refusals, ["Interval::sin", "Interval::cos"]);
    for name in refusals {
        for x in &intervals {
            let got = if name == "Interval::sin" {
                x.sin()
            } else {
                x.cos()
            };
            assert_eq!(
                got,
                Err(NumError::UnprovenMaths),
                "{name} computes now: give it a benchmark instead of an UNMEASURED entry"
            );
        }
    }
}

#[test]
fn the_gate_is_five_percent_on_every_measure() {
    // Principle 12: a speed regression over 5% blocks a merge; chapter 23.13 gates instructions,
    // L1 and last-level misses, estimated cycles and peak heap, and lists changes over 1%.
    // Changing these needs an accepted decision note, not just an edit.
    assert_eq!(GATE_PERCENT, 5.0);
    assert_eq!(LIST_PERCENT, 1.0);
    let misses = ["I1mr", "D1mr", "D1mw", "ILmr", "DLmr", "DLmw"];
    assert_eq!(GATED_METRICS[..2], ["Ir", "EstimatedCycles"]);
    assert_eq!(GATED_METRICS[2..], misses);
    assert_eq!(
        GATED_HEAP_METRICS,
        ["TotalBytes", "TotalBlocks", "AtTGmaxBytes", "MaximumBytes"]
    );
    // A bare local run gates every measure at a plain 5%. The workflow's limits replace these and
    // gate instructions and estimated cycles; the miss counts are judged from Gungraun's summaries
    // (geom_bench::callgrind), which lists them and judges them through estimated cycles.
    for metric in GATED_METRICS {
        assert!(
            GATE.contains(&format!("(EventKind::{metric}, GATE_PERCENT)")),
            "{metric}"
        );
    }
    #[cfg(all(feature = "judge", any(unix, windows)))]
    {
        let judged: Vec<&str> = geom_bench::callgrind::JUDGED
            .iter()
            .map(|&(_, name)| name)
            .collect();
        assert_eq!(judged, GATED_METRICS);
    }
    for metric in GATED_HEAP_METRICS {
        assert!(
            GATE.contains(&format!("(DhatMetric::{metric}, GATE_PERCENT)")),
            "{metric}"
        );
    }
    let limits = |metrics: &[&str]| {
        metrics
            .iter()
            .map(|m| format!("{}={GATE_PERCENT}%", m.to_lowercase()))
            .collect::<Vec<_>>()
    };
    let as_written = |s: &str| s.replace(".0%", "%");
    assert_eq!(
        as_written(&limits(&GATED_METRICS[..2]).join(",")),
        CALLGRIND_LIMITS
    );
    assert_eq!(
        as_written(&limits(&GATED_HEAP_METRICS).join(",")),
        DHAT_LIMITS
    );
    assert!(GATE.contains("callgrind.args([ZERO_BEFORE]);"));
    assert!(ZERO_BEFORE.starts_with("--zero-before="));
}

#[test]
fn the_workflow_passes_the_documented_limits_and_build() {
    // The limits come from the workflow, not from the tree under test.
    assert!(WORKFLOW.contains(&format!("CALLGRIND_LIMITS: \"{CALLGRIND_LIMITS}\"")));
    assert!(WORKFLOW.contains(&format!("DHAT_LIMITS: \"{DHAT_LIMITS}\"")));
    assert!(WORKFLOW.contains("--callgrind-limits=\"$CALLGRIND_LIMITS\""));
    assert!(WORKFLOW.contains("--dhat-limits=\"$DHAT_LIMITS\""));
    assert!(WORKFLOW.contains(&format!("GATE_PERCENT: \"{GATE_PERCENT}\"").replace(".0\"", "\"")));
    assert!(WORKFLOW.contains(&format!("LIST_PERCENT: \"{LIST_PERCENT}\"").replace(".0\"", "\"")));
    // The steady-state measurement depends on cache-line-aligned functions.
    assert!(WORKFLOW.contains("-C llvm-args=-align-all-functions=6"));
    // Both trees' measured builds are one codegen unit per crate, so inlining does not move with
    // how rustc splits geom-bench into units: the instructions job's benchmark binaries (the bench
    // profile) and the fuel job's guests (the release profile, on their build step only).
    assert!(WORKFLOW.contains("CARGO_PROFILE_BENCH_CODEGEN_UNITS: \"1\""));
    assert!(WORKFLOW.contains("CARGO_PROFILE_RELEASE_CODEGEN_UNITS: \"1\""));
    // The toolchain comes from toolchain.txt; nothing installs a floating one.
    assert!(WORKFLOW.contains("toolchain.txt"));
    assert!(!WORKFLOW.contains("@stable") && !WORKFLOW.contains("toolchain: stable"));
    // The fuel gate, growth, removals and the golden check on wasm32 all run, and so do this
    // crate's own checks as kernel.yml makes them for geom-num: formatting, Clippy with the kernel
    // lints on every target (on Linux, the only place the Valgrind gate compiles), and the tests in
    // release with overflow checks and in debug, with warnings as errors.
    for needle in [
        "wasm32-wasip1",
        "geom-bench-guest",
        "geom-bench-fuel",
        "COUNTS=$CARGO_TARGET_DIR/release/geom-bench-counts",
        "\"$COUNTS\" compare",
        "\"$COUNTS\" growth",
        "geom-bench-counts\" removed",
        "--save-summary=json",
        "\"$CARGO_TARGET_DIR/release/geom-bench-counts\" callgrind",
        "\"$GUNGRAUN_HOME/geom-bench/instructions\"",
        // Each tree builds apart (cargo names a path crate's build by its place in the workspace),
        // and both trees' saved runs share one Gungraun home.
        "GUNGRAUN_HOME: ${{ github.workspace }}/gungraun",
        "CARGO_TARGET_DIR: ${{ github.workspace }}/target-base",
        "--target-dir \"$CARGO_TARGET_DIR/guest-$1\"",
        "retired.txt",
        "test --locked --release -p geom-bench",
        "fmt --check -p geom-bench",
        "fmt --check --manifest-path \"$BENCH/fuel/Cargo.toml\"",
        "clippy --locked -p geom-bench --all-targets -- -D warnings",
        "clippy --locked --manifest-path \"$BENCH/fuel/Cargo.toml\"",
        "RUSTFLAGS: -C overflow-checks=on -D warnings",
        "RUSTFLAGS: -D warnings",
        "test --locked -p geom-bench",
    ] {
        assert!(
            WORKFLOW.contains(needle),
            "the workflow does not run {needle}"
        );
    }
    // Read-only: one `permissions` block, at the top, granting `contents: read` and nothing else,
    // and no job widens it.
    let lines: Vec<&str> = WORKFLOW.lines().collect();
    let blocks: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim_start().starts_with("permissions:"))
        .map(|(at, _)| at)
        .collect();
    assert_eq!(blocks.len(), 1, "permissions blocks at lines {blocks:?}");
    let at = blocks[0];
    assert_eq!(
        lines[at..at + 3],
        ["permissions:", "  contents: read", ""],
        "the workflow's permissions"
    );
}

#[test]
fn the_pins_are_exact_and_agree() {
    // The toolchain: an exact release, x.y.z.
    let parts: Vec<&str> = TOOLCHAIN.split('.').collect();
    assert_eq!(parts.len(), 3, "{TOOLCHAIN}");
    assert!(
        parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
    );
    // The same compiler as the kernel's own workflow, so geom-num's golden hash and this crate's
    // golden digests are checked by one compiler (kernel.yml: "One pinned compiler for every job").
    let pin = format!("  RUST_TOOLCHAIN: \"{TOOLCHAIN}\"");
    assert!(
        KERNEL_WORKFLOW.lines().any(|line| line == pin),
        "toolchain.txt ({TOOLCHAIN}) and kernel.yml's RUST_TOOLCHAIN differ"
    );
    // Gungraun: the runner binary must match the library exactly; the manifest pins it, the
    // documentation and the lock file must agree.
    let exact = format!("={GUNGRAUN_VERSION}");
    assert_eq!(requirements(MANIFEST, "gungraun"), [exact.as_str()]);
    assert_eq!(locked_versions("gungraun"), [GUNGRAUN_VERSION]);
    // The summary model the miss-count judge reads (geom_bench::callgrind) is the runner's own, at
    // exactly the runner's version, so the schema read is the one written.
    assert!(MANIFEST.contains(&format!(
        "gungraun-runner = {{ version = \"={GUNGRAUN_VERSION}\", default-features = false, \
         features = [\"summary\"], optional = true }}"
    )));
    assert_eq!(requirements(MANIFEST, "gungraun-runner"), [exact.as_str()]);
    assert_eq!(locked_versions("gungraun-runner"), [GUNGRAUN_VERSION]);
    let docs = include_str!("../src/lib.rs");
    assert!(docs.contains(&format!("gungraun-runner --version {GUNGRAUN_VERSION}")));
    // Wasmtime: its fuel model defines the counts, so its version is pinned in one place.
    let exact = format!("={WASMTIME_VERSION}");
    assert_eq!(requirements(FUEL_MANIFEST, "wasmtime"), [exact]);
    assert!(FUEL_MAIN.contains(&format!(
        "const WASMTIME_VERSION: &str = \"{WASMTIME_VERSION}\";"
    )));
    let fuel: toml::Table = FUEL_MANIFEST.parse().expect("fuel/Cargo.toml is not TOML");
    assert!(
        fuel.get("workspace").is_some_and(toml::Value::is_table),
        "the fuel host must be its own workspace"
    );
    let manifest: toml::Table = MANIFEST.parse().expect("Cargo.toml is not TOML");
    let excluded = manifest
        .get("package")
        .and_then(|p| p.get("exclude"))
        .and_then(toml::Value::as_array);
    assert!(
        excluded.is_some_and(|list| list.iter().any(|item| item.as_str() == Some("fuel"))),
        "geom-bench's package must exclude the fuel host"
    );
}

/// Every version requirement a manifest gives the dependency `name`, in any dependency table:
/// `[dependencies]`, `[dev-dependencies]` and `[build-dependencies]`, each also under any
/// `[target.'...']`; in either form, a string or a table with `version`. Read by the `toml` crate.
fn requirements(manifest: &str, name: &str) -> Vec<String> {
    let table: toml::Table = manifest.parse().expect("a manifest that is not TOML");
    let mut tables: Vec<&toml::Table> = vec![&table];
    if let Some(targets) = table.get("target").and_then(toml::Value::as_table) {
        tables.extend(targets.values().filter_map(toml::Value::as_table));
    }
    let mut found: Vec<String> = Vec::new();
    for t in tables {
        for kind in ["dependencies", "dev-dependencies", "build-dependencies"] {
            let Some(dep) = t.get(kind).and_then(|d| d.get(name)) else {
                continue;
            };
            let req = dep
                .as_str()
                .or_else(|| dep.get("version").and_then(toml::Value::as_str))
                .unwrap_or_else(|| panic!("{name} has no version requirement: {dep}"));
            found.push(req.to_owned());
        }
    }
    found
}

/// Every version of the package `name` that `Cargo.lock` holds, read by the `toml` crate.
fn locked_versions(name: &str) -> Vec<String> {
    let lock: toml::Table = LOCK.parse().expect("Cargo.lock is not TOML");
    lock.get("package")
        .and_then(toml::Value::as_array)
        .expect("Cargo.lock has no [[package]] tables")
        .iter()
        .filter(|p| p.get("name").and_then(toml::Value::as_str) == Some(name))
        .filter_map(|p| p.get("version").and_then(toml::Value::as_str))
        .map(str::to_owned)
        .collect()
}

#[test]
fn every_action_is_pinned_to_the_commit_kernel_yml_runs() {
    // A tag or a branch can be moved to other code; a full commit cannot. Every action the gates
    // run is pinned to a full commit (40 hexadecimal digits) with a comment naming the tag or
    // branch it was checked against, and to the very commit kernel.yml runs for that action, so
    // the two workflows move together and only after review; and every checkout keeps no
    // credentials.
    let uses = |text: &'static str| -> Vec<&'static str> {
        text.lines()
            .map(|line| line.trim().trim_start_matches("- "))
            .filter(|line| line.starts_with("uses: "))
            .collect()
    };
    let kernel = uses(KERNEL_WORKFLOW);
    let ours = uses(WORKFLOW);
    assert!(!ours.is_empty());
    for line in &ours {
        let pin = line
            .split_once('@')
            .and_then(|(_, pin)| pin.split_once(" # "));
        assert!(
            pin.is_some_and(|(commit, checked_against)| commit.len() == 40
                && commit
                    .chars()
                    .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
                && !checked_against.trim().is_empty()),
            "not pinned to a full commit with the tag it was checked against: {line}"
        );
        assert!(
            kernel.contains(line),
            "kernel.yml runs another commit of this action, or does not run it: {line}"
        );
    }
    let lines: Vec<&str> = WORKFLOW.lines().map(str::trim).collect();
    let checkouts: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| {
            line.trim_start_matches("- ")
                .starts_with("uses: actions/checkout@")
        })
        .map(|(at, _)| at)
        .collect();
    assert!(!checkouts.is_empty());
    for at in checkouts {
        // The step's `with:` block: the lines after `uses:`, up to the next step.
        let with: Vec<&str> = lines[at + 1..]
            .iter()
            .copied()
            .take_while(|line| !line.is_empty() && !line.starts_with("- "))
            .collect();
        assert!(
            with.contains(&"persist-credentials: false"),
            "a checkout keeps its credentials: {with:?}"
        );
    }
}

#[test]
fn retired_names_are_not_reused() {
    let current: BTreeSet<String> = names().into_iter().collect();
    let retired = geom_bench::counts::parse_names(RETIRED);
    let reused: Vec<_> = retired.intersection(&current).collect();
    assert!(reused.is_empty(), "retired names in use again: {reused:?}");
}
