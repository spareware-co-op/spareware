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
//! reads the files it compares as text (line by line, so a checkout with either line ending reads
//! the same) and requires the sets they list to be equal.
//!
//! Cost: a linear scan of a few thousand lines of source.

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
const GEOM_NUM: [(&str, &str); 19] = [
    ("dd", include_str!("../../geom-num/src/dd.rs")),
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
    ("ladder", include_str!("../../geom-num/src/ladder.rs")),
    ("lib", include_str!("../../geom-num/src/lib.rs")),
    ("math", include_str!("../../geom-num/src/math.rs")),
    ("perturb", include_str!("../../geom-num/src/perturb.rs")),
    ("plane", include_str!("../../geom-num/src/plane.rs")),
    ("crossing", include_str!("../../geom-num/src/crossing.rs")),
    ("predicate", include_str!("../../geom-num/src/predicate.rs")),
    ("real", include_str!("../../geom-num/src/real.rs")),
    ("rng", include_str!("../../geom-num/src/rng.rs")),
    ("sign", include_str!("../../geom-num/src/sign.rs")),
    ("wide", include_str!("../../geom-num/src/wide.rs")),
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
const GENERATED_PARTS: [(&str, &str); 11] = [
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

/// The public functions a `geom-num` source file defines: `module::function` for free functions,
/// `Type::method` for methods, and `Type::cmp` for an `Ord` implementation. Test modules are
/// skipped.
fn public_functions(module: &str, source: &str) -> Vec<String> {
    let mut out = Vec::new();
    // The type of the `impl` block being read, and whether it implements `Ord`.
    let mut current: Option<(String, bool)> = None;
    // After a `#[cfg(test)]`: a one-line module declaration (`mod x;`) is skipped, and a module
    // with a body (`mod tests {`) ends the public code.
    let mut test_item = false;
    for line in source.lines() {
        if line.starts_with("#[cfg(test)]") {
            test_item = true;
            continue;
        }
        if test_item && !line.starts_with("#[") && !line.starts_with(' ') && !line.starts_with(')')
        {
            if line.contains("mod ") && line.trim_end().ends_with('{') {
                break;
            }
            test_item = false;
            continue;
        }
        if let Some(rest) = line.strip_prefix("impl") {
            // Skip generic parameters, then read `Trait for Type` or `Type`.
            let mut depth = 0;
            let rest: String = rest
                .chars()
                .filter(|&c| {
                    match c {
                        '<' => depth += 1,
                        '>' => depth -= 1,
                        _ => return depth == 0,
                    }
                    false
                })
                .collect();
            let rest = rest.trim();
            current = Some(match rest.split_once(" for ") {
                Some((tr, ty)) => (
                    ident(ty.trim()),
                    tr.trim().rsplit("::").next() == Some("Ord"),
                ),
                None => (ident(rest), false),
            });
            continue;
        }
        if line.starts_with('}') {
            current = None;
            continue;
        }
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        let name = trimmed
            .strip_prefix("pub const fn ")
            .or_else(|| trimmed.strip_prefix("pub fn "))
            .map(ident);
        match (&current, indent, name) {
            (None, 0, Some(name)) => out.push(format!("{module}::{name}")),
            (Some((ty, _)), 4, Some(name)) => out.push(format!("{ty}::{name}")),
            (Some((ty, true)), 4, None) if trimmed.starts_with("fn cmp(") => {
                out.push(format!("{ty}::cmp"));
            }
            _ => {}
        }
    }
    out
}

#[test]
fn every_public_function_of_geom_num_is_measured_or_says_why_not() {
    // Principle 12: every operation ships with a benchmark.
    let (_, lib) = GEOM_NUM.iter().find(|(m, _)| *m == "lib").unwrap();
    let declared: BTreeSet<String> = lib
        .lines()
        .filter_map(|l| l.strip_prefix("pub mod "))
        .map(ident)
        .collect();
    let read: BTreeSet<String> = GEOM_NUM
        .iter()
        .map(|(m, _)| (*m).to_owned())
        .filter(|m| m != "lib")
        .collect();
    assert_eq!(declared, read, "a geom-num module this test does not read");
    // Every module the maths module declares is read or listed as crate-internal.
    let (_, math) = GEOM_NUM.iter().find(|(m, _)| *m == "math").unwrap();
    let math_modules: BTreeSet<String> = math
        .lines()
        .filter_map(|l| {
            l.strip_prefix("pub mod ")
                .or_else(|| l.strip_prefix("mod "))
        })
        .map(ident)
        .filter(|m| m != "tests")
        .collect();
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
    let generated_modules: BTreeSet<String> = generated
        .lines()
        .filter_map(|l| l.strip_prefix("pub mod "))
        .map(|m| format!("generated::{}", ident(m)))
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
    let measured: BTreeSet<String> = OPS
        .iter()
        .map(|op| op.measures.to_owned())
        .chain(GROWTH.iter().map(|g| g.measures.to_owned()))
        .collect();
    let unmeasured: BTreeSet<String> = UNMEASURED.iter().map(|(f, _)| (*f).to_owned()).collect();
    assert!(
        measured.is_disjoint(&unmeasured),
        "{:?}",
        measured.intersection(&unmeasured)
    );
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
    assert_eq!(
        refusals,
        ["Interval::sin", "Interval::cos", "Interval::atan2"]
    );
    for name in refusals {
        for x in &intervals {
            let got = match name {
                "Interval::sin" => x.sin(),
                "Interval::cos" => x.cos(),
                _ => x.atan2(*x),
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
    // (geom_bench::callgrind, G18a), which gate only those that can resolve 5%.
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
        assert_eq!(geom_bench::callgrind::floor(5), Some(20));
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
    assert!(MANIFEST.contains(&format!("gungraun = \"={GUNGRAUN_VERSION}\"")));
    // The summary model the miss-count judge reads (geom_bench::callgrind) is the runner's own, at
    // exactly the runner's version, so the schema read is the one written.
    assert!(MANIFEST.contains(&format!(
        "gungraun-runner = {{ version = \"={GUNGRAUN_VERSION}\", default-features = false, \
         features = [\"summary\"], optional = true }}"
    )));
    // Each `[[package]]` entry's lines; the gungraun entry's `version`.
    let locked = LOCK
        .split("[[package]]")
        .find(|entry| entry.lines().any(|line| line == "name = \"gungraun\""))
        .and_then(|entry| {
            entry
                .lines()
                .find_map(|line| line.strip_prefix("version = \""))
        })
        .and_then(|rest| rest.split('"').next());
    assert_eq!(locked, Some(GUNGRAUN_VERSION));
    let docs = include_str!("../src/lib.rs");
    assert!(docs.contains(&format!("gungraun-runner --version {GUNGRAUN_VERSION}")));
    // Wasmtime: its fuel model defines the counts, so its version is pinned in one place.
    assert!(FUEL_MANIFEST.contains(&format!("version = \"={WASMTIME_VERSION}\"")));
    assert!(FUEL_MAIN.contains(&format!(
        "const WASMTIME_VERSION: &str = \"{WASMTIME_VERSION}\";"
    )));
    assert!(
        FUEL_MANIFEST.lines().any(|line| line == "[workspace]"),
        "the fuel host must be its own workspace"
    );
    assert!(MANIFEST.contains("exclude = [\"fuel\"]"));
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
