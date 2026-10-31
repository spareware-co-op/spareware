//! Attacks on piece G14's claims: the orient2d target's oracle and proof domain, and the layout
//! every target follows (`README.md`), its registration in CI and the pins it names.
//!
//! The runtime attacks try to break what `geom_fuzz::orient2d`'s header guarantees: the reference
//! against an independent formula and its laws, the oracle at the extremes of the frame, the
//! hardest near-degenerate input the frame holds, that the proof's domain reaches the filter's
//! rounding and both of its answers, and that every stored corpus and crash file is a case.
//!
//! The layout attacks read the repository: every target has its fuzz and seeded tests, the CI
//! matrices list exactly the targets' fuzz and proof tests, the workflow's path filters cover every
//! crate a target builds, the tool versions agree wherever they are written, the build check the
//! workflow turns off is made by `.clusterfuzzlite/build.sh` instead, and the workspace has the
//! profile cargo-bolero builds with. Each of those checks is a function of the files' text, and
//! each is shown to fail on a seeded violation, so none can pass by reading nothing.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them, and std's reductions, to build their oracles.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "oracles and generators; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use geom_fuzz::orient2d::{Case, PROOF_BOUND, ProofDraw, check, check_with, proof_case, reference};
use geom_fuzz::{
    Harness, boxtree, curves, linalg, nurbs, orient2d, predicates, roots, surfaces, transforms,
    variants, wide,
};
use geom_num::Sign;
use geom_num::generated::orient2d::{filter, value};
use geom_num::grid::{COORD_MAX, P2};
use geom_num::predicate::orient2d as orient2d_predicate;
use geom_test::grid::GridSampler;
use geom_test::seed::fixed;
use geom_test::{Seed, SplitMix64};

/// Every target of the crate, by module name, with its harness. A target module that is not here,
/// or a name here that is not a target module, fails `attack_every_target_is_listed_here`.
const TARGETS: [(&str, Harness); 11] = [
    ("boxtree", boxtree::harness),
    ("curves", curves::harness),
    ("linalg", linalg::harness),
    ("nurbs", nurbs::harness),
    ("orient2d", orient2d::harness),
    ("predicates", predicates::harness),
    ("roots", roots::harness),
    ("surfaces", surfaces::harness),
    ("transforms", transforms::harness),
    ("variants", variants::harness),
    ("wide", wide::harness),
];

const M: i64 = COORD_MAX;

fn p(x: i64, y: i64) -> P2 {
    P2::new(x, y).unwrap()
}

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn workspace_root() -> PathBuf {
    crate_dir().ancestors().nth(5).unwrap().to_path_buf()
}

fn read(relative: &str) -> String {
    let path = workspace_root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .replace("\r\n", "\n")
}

fn cargo() -> OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into())
}

// ---------------------------------------------------------------------------------------------
// The orient2d target at run time.
// ---------------------------------------------------------------------------------------------

/// (b − a) × (c − a), the translated form, written here in plain `i128`: a second formula for the
/// reference, which expands along the column of ones instead.
fn translated(a: P2, b: P2, c: P2) -> i128 {
    let [ax, ay, bx, by, cx, cy] = [a.x, a.y, b.x, b.y, c.x, c.y].map(|v| i128::from(v.get()));
    (bx - ax) * (cy - ay) - (by - ay) * (cx - ax)
}

/// The reference agrees with the translated formula, swaps flip it, rotations keep it and a
/// translation that stays in the frame leaves it unchanged, on uniformly random points over the
/// whole frame and on the grid sampler's hard cases. The reference is the oracle every claim is
/// judged by, so it is attacked first.
#[test]
fn attack_the_reference_agrees_with_a_second_formula_and_keeps_its_laws() {
    let mut rng = SplitMix64::new(fixed("G14 reference laws", Seed(0x6014)));
    let coord = |rng: &mut SplitMix64| rng.range_i64(-M, M).unwrap();
    let mut grid = GridSampler::new();
    for i in 0..60_000 {
        let [a, b, c] = if i % 2 == 0 {
            [0; 3].map(|_| p(coord(&mut rng), coord(&mut rng)))
        } else {
            [0; 3].map(|_| grid.p2(&mut rng))
        };
        let r = reference(a, b, c).unwrap();
        assert_eq!(r, translated(a, b, c), "{a:?} {b:?} {c:?}");
        assert_eq!(reference(b, a, c), Some(-r));
        assert_eq!(reference(a, c, b), Some(-r));
        assert_eq!(reference(c, b, a), Some(-r));
        assert_eq!(reference(b, c, a), Some(r));
        assert_eq!(reference(c, a, b), Some(r));
        // A translation by t that keeps every point in the frame.
        let (tx, ty) = (coord(&mut rng) / 4, coord(&mut rng) / 4);
        let moved = [a, b, c].map(|q| P2::new(q.x.get() + tx, q.y.get() + ty));
        if let [Ok(ma), Ok(mb), Ok(mc)] = moved {
            assert_eq!(reference(ma, mb, mc), Some(r), "translated by ({tx}, {ty})");
        }
        check(&Case { a, b, c }).unwrap();
    }
}

/// At the edges of the frame the oracle's products are largest: every combination of the six
/// coordinates from the frame's two edges and the steps just inside them (4^6 cases) has a
/// reference, the translated formula's value, and passes the check.
#[test]
fn attack_the_oracle_holds_at_every_combination_of_the_frames_edges() {
    let values = [M, -M, M - 1, -M + 1];
    for n in 0..4usize.pow(6) {
        let v: [i64; 6] = core::array::from_fn(|i| values[n / 4usize.pow(i as u32) % 4]);
        let (a, b, c) = (p(v[0], v[1]), p(v[2], v[3]), p(v[4], v[5]));
        assert_eq!(reference(a, b, c), Some(translated(a, b, c)));
        check(&Case { a, b, c }).unwrap();
    }
    // The grid's largest value, 4M², is reached and fits.
    let big = reference(p(-M, -M), p(M, -M), p(M, M)).unwrap();
    assert_eq!(big, 4 * i128::from(M) * i128::from(M));
}

/// The hardest near-degenerate input of the frame: b − a and c − a are consecutive Fibonacci pairs
/// near 2^40, so by Cassini's identity the determinant is exactly ±1 while each product is near
/// 2^79, where a double's step is about 2^27. The filter must decline (its products round to the
/// same double); orient2d and the exact path must still give ±1; and a filter that rounded its
/// uncertain band to `Zero` would be caught here, where no random input would find it.
#[test]
fn attack_the_check_holds_on_the_frames_hardest_near_degenerate_input() {
    // F(57), F(58), F(59): the largest consecutive Fibonacci numbers below 2^40 − 1.
    let (f57, f58, f59): (i64, i64, i64) = (365_435_296_162, 591_286_729_879, 956_722_026_041);
    assert_eq!(f57 + f58, f59);
    assert!(f59 < M);
    for (ox, oy) in [(0, 0), (-M, -M), (-M, M - f59), (M - f59, -M)] {
        let a = p(ox, oy);
        let b = p(ox + f59, oy + f58);
        let c = p(ox + f58, oy + f57);
        // Cassini: F(n+1)·F(n−1) − F(n)² = (−1)^n, with n = 58.
        assert_eq!(reference(a, b, c), Some(1));
        assert_eq!(reference(a, c, b), Some(-1));
        assert_eq!(filter(a, b, c), None, "the filter must decline at det = 1");
        assert_eq!(orient2d_predicate(a, b, c), Ok(Sign::Positive));
        assert_eq!(value(a, b, c), Some(1));
        let case = Case { a, b, c };
        check(&case).unwrap();
        // A filter that calls its uncertain band Zero.
        let banded = |a, b, c| filter(a, b, c).or(Some(Sign::Zero));
        let e = check_with(&case, orient2d_predicate, banded, value).unwrap_err();
        assert!(e.law.contains("filter"), "{e}");
        assert!(e.detail.contains("reference: 1"), "{e}");
    }
}

/// The proof's domain is not a toy: it holds cases whose filter products round in `f64`, cases
/// where the filter declines, cases where it decides a sign, and degenerate cases where the exact
/// value is zero. So Kani's proof over it covers both of the filter's answers and its error bound,
/// as the target's header claims. Searched over every edge and sign pattern at offsets 0 and the
/// bound, the same 2^18 cases the target's unit test checks.
#[test]
fn attack_the_proof_domain_reaches_rounding_and_both_filter_answers() {
    let b = PROOF_BOUND;
    let (mut rounded, mut declined, mut decided, mut degenerate) = (0u32, 0u32, 0u32, 0u32);
    for pattern in 0u32..(1 << 18) {
        let bit = |i: u32| pattern >> i & 1 == 1;
        let draw = ProofDraw {
            near_edge: core::array::from_fn(|i| bit(i as u32)),
            negative: core::array::from_fn(|i| bit(6 + i as u32)),
            offset: core::array::from_fn(|i| if bit(12 + i as u32) { b } else { 0 }),
        };
        let Case { a, b: bb, c } = proof_case(b, &draw).unwrap();
        let [ax, ay, bx, by, cx, cy] =
            [a.x, a.y, bb.x, bb.y, c.x, c.y].map(|v| i128::from(v.get()));
        let products = [(bx - ax) * (cy - ay), (by - ay) * (cx - ax)];
        if products.iter().any(|&q| (q as f64) as i128 != q) {
            rounded += 1;
        }
        match filter(a, bb, c) {
            None => declined += 1,
            Some(Sign::Zero) => {}
            Some(_) => decided += 1,
        }
        if reference(a, bb, c) == Some(0) {
            degenerate += 1;
        }
    }
    assert!(
        rounded > 0,
        "no case of the proof's domain rounds a filter product"
    );
    assert!(
        declined > 0,
        "the filter decides every case of the proof's domain"
    );
    assert!(
        decided > 0,
        "the filter decides no case of the proof's domain"
    );
    assert!(degenerate > 0, "the proof's domain has no degenerate case");
}

/// Every file stored in a target's corpus or crash folder is a case its harness passes, and every
/// stored folder belongs to a target: the files every engine starts from and `cargo test` replays.
#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the corpus folders from the host, which WASI cannot without a preopened folder"
)]
#[test]
fn attack_every_stored_corpus_and_crash_file_is_a_passing_case() {
    let fuzz = crate_dir().join("tests").join("__fuzz__");
    let harnesses: BTreeMap<&str, Harness> = TARGETS.into_iter().collect();
    let mut files = 0;
    for entry in std::fs::read_dir(&fuzz).unwrap() {
        let entry = entry.unwrap();
        if !entry.file_type().unwrap().is_dir() {
            continue;
        }
        let name = entry.file_name().into_string().unwrap();
        let harness = harnesses
            .get(name.as_str())
            .unwrap_or_else(|| panic!("tests/__fuzz__/{name} belongs to no target"));
        for kind in ["corpus", "crashes"] {
            let Ok(listing) = std::fs::read_dir(entry.path().join(kind)) else {
                continue;
            };
            for f in listing {
                let path = f.unwrap().path();
                let bytes = std::fs::read(&path).unwrap();
                harness(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                files += 1;
            }
        }
    }
    assert!(
        files > 0,
        "no corpus file is stored, so this attack checked nothing"
    );
}

// ---------------------------------------------------------------------------------------------
// The layout, read from the repository.
// ---------------------------------------------------------------------------------------------

/// The target modules of `src/`: every module that defines `pub fn harness(`.
fn target_modules() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for entry in std::fs::read_dir(crate_dir().join("src")).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        if text.contains("pub fn harness(") {
            out.insert(path.file_stem().unwrap().to_str().unwrap().to_owned());
        }
    }
    out
}

/// The test files of `tests/`, by file stem, with their text.
fn test_files() -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for entry in std::fs::read_dir(crate_dir().join("tests")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs") {
            let text = std::fs::read_to_string(&path)
                .unwrap()
                .replace("\r\n", "\n");
            out.insert(path.file_stem().unwrap().to_str().unwrap().to_owned(), text);
        }
    }
    out
}

/// The items of a workflow list under `key:`, either inline (`key: [a, b]`) or as `- item` lines
/// indented below it; comment lines are skipped. Empty if the key is absent.
fn workflow_list(yaml: &str, key: &str) -> Vec<String> {
    let lines: Vec<&str> = yaml.lines().collect();
    let Some(at) = lines
        .iter()
        .position(|l| l.trim_start().starts_with(&format!("{key}:")))
    else {
        return Vec::new();
    };
    let line = lines[at];
    let indent = line.len() - line.trim_start().len();
    let rest = line.trim_start()[key.len() + 1..].trim();
    if let Some(inline) = rest.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        return inline
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();
    }
    let mut out = Vec::new();
    for l in &lines[at + 1..] {
        let t = l.trim_start();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if l.len() - t.len() <= indent || !t.starts_with("- ") {
            break;
        }
        out.push(t[2..].trim().to_owned());
    }
    out
}

/// The names of the test functions marked `#[cfg_attr(kani, kani::proof)]` in `text`, the form
/// the layout prescribes.
fn proof_tests(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if l.trim() != "#[cfg_attr(kani, kani::proof)]" {
            continue;
        }
        let name = lines[i + 1..]
            .iter()
            .find_map(|l| l.trim().strip_prefix("fn "))
            .and_then(|rest| rest.split('(').next())
            .unwrap_or_default();
        out.push(name.to_owned());
    }
    out
}

/// The functions of a test file's `text` that call bolero's `check!` without being `#[test]`
/// functions themselves. bolero names a target after the function `check!` is written in
/// (`bolero-engine`'s `TargetLocation::item_path`), and cargo-bolero selects a target by running
/// the test of that name, so a target written in a helper cannot be built for libFuzzer or for
/// `ClusterFuzzLite` ("no targets matched"). Comment lines are skipped.
fn checks_outside_tests(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut marked_test = false;
    let mut current: Option<(String, bool)> = None;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("//") {
            continue;
        }
        if t == "#[test]" {
            marked_test = true;
            continue;
        }
        if let Some(rest) = t.strip_prefix("fn ").or_else(|| t.strip_prefix("pub fn ")) {
            let name = rest.split(['(', '<']).next().unwrap_or_default().to_owned();
            current = Some((name, marked_test));
            marked_test = false;
            continue;
        }
        // Assembled, so this function is not a call itself.
        if t.contains(concat!("check", "!(")) {
            match &current {
                Some((_, true)) => {}
                Some((name, false)) => out.push(name.clone()),
                None => out.push("no function".to_owned()),
            }
        }
    }
    out
}

/// The `#[should_panic]` test functions of a test file's `text` that call bolero's `check!`.
/// cargo-bolero's `build-clusterfuzz` builds each test binary by running one of its bolero tests,
/// the first it listed (in no fixed order), with `CARGO_BOLERO_SELECT` set, where `check!` returns
/// without running its body (`bolero-engine`'s `TargetLocation::should_run`). A `#[should_panic]`
/// test fails there, and `ClusterFuzzLite`'s build fails with it, on some runs and not others.
/// Comment lines are skipped.
fn should_panic_checks(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut should_panic = false;
    let mut current: Option<(String, bool)> = None;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("//") {
            continue;
        }
        if t.starts_with("#[should_panic") {
            should_panic = true;
            continue;
        }
        if let Some(rest) = t.strip_prefix("fn ").or_else(|| t.strip_prefix("pub fn ")) {
            let name = rest.split(['(', '<']).next().unwrap_or_default().to_owned();
            current = Some((name, should_panic));
            should_panic = false;
            continue;
        }
        // Assembled, so this function is not a call itself.
        if t.contains(concat!("check", "!(")) {
            if let Some((name, true)) = &current {
                if !out.contains(name) {
                    out.push(name.clone());
                }
            }
        }
    }
    out
}

/// The proof tests of a test file's `text` whose body does not hand its verdict to `holds`, which
/// panics with a finding. Returned instead, the finding would reach bolero's `PanicError` under
/// Kani, whose backtrace capture kept the first proofs in symbolic execution for the job's hour
/// (the crate README, the rules of the layout). A body runs from its `fn` line to the first line
/// that closes it, `}` at the start of a line.
fn proofs_without_holds(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for name in proof_tests(text) {
        let Some(start) = text.find(&format!("fn {name}(")) else {
            out.push(name);
            continue;
        };
        let body = &text[start..];
        let body = &body[..body.find("\n}").unwrap_or(body.len())];
        if !body.contains("holds(") {
            out.push(name);
        }
    }
    out
}

/// What the layout gets wrong: each target's test file and its tests, and the CI matrices.
fn layout_violations(
    targets: &BTreeSet<String>,
    tests: &BTreeMap<String, String>,
    workflow: &str,
) -> Vec<String> {
    let mut found = Vec::new();
    for t in targets {
        let Some(text) = tests.get(t) else {
            found.push(format!("target {t} has no tests/{t}.rs"));
            continue;
        };
        for test in [format!("fn {t}_fuzz()"), format!("fn {t}_seeded()")] {
            if !text.contains(&test) {
                found.push(format!("tests/{t}.rs has no `{test}`"));
            }
        }
        if !text.contains("for_each(|data: &[u8]| holds(harness(data)))") {
            found.push(format!(
                "tests/{t}.rs does not hand the fuzzer's raw bytes to its harness"
            ));
        }
    }
    for (file, text) in tests {
        for function in checks_outside_tests(text) {
            found.push(format!(
                "tests/{file}.rs calls bolero's check! in {function}, which is not a #[test] \
                 function, so cargo-bolero cannot select that target by its test's name"
            ));
        }
        for function in proofs_without_holds(text) {
            found.push(format!(
                "tests/{file}.rs's proof test {function} returns its finding instead of handing \
                 it to holds, so under Kani bolero's error path, backtrace capture included, \
                 enters the proof"
            ));
        }
        for function in should_panic_checks(text) {
            found.push(format!(
                "tests/{file}.rs calls bolero's check! in {function}, a #[should_panic] test, \
                 which fails when cargo-bolero selects targets, and ClusterFuzzLite's build \
                 with it"
            ));
        }
    }
    let fuzz_tests: BTreeSet<String> = targets.iter().map(|t| format!("{t}_fuzz")).collect();
    for text in tests.values() {
        let names = text
            .lines()
            .filter_map(|l| l.trim().strip_prefix("fn "))
            .filter_map(|rest| rest.split('(').next());
        for name in names.filter(|n| n.ends_with("_fuzz") && !fuzz_tests.contains(*n)) {
            found.push(format!(
                "test {name} ends in _fuzz, so ClusterFuzzLite fuzzes it, but no target module \
                 has it"
            ));
        }
    }
    let libfuzzer: BTreeSet<String> = workflow_list(workflow, "target").into_iter().collect();
    if libfuzzer != fuzz_tests {
        found.push(format!(
            "kernel-fuzz.yml's libFuzzer matrix {libfuzzer:?} is not the targets' fuzz tests \
             {fuzz_tests:?}"
        ));
    }
    let proofs: BTreeSet<String> = tests.values().flat_map(|t| proof_tests(t)).collect();
    let kani: BTreeSet<String> = workflow_list(workflow, "harness").into_iter().collect();
    if kani != proofs {
        found.push(format!(
            "kernel-fuzz.yml's Kani matrix {kani:?} is not the proof tests {proofs:?}"
        ));
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which WASI cannot without a preopened folder"
)]
#[test]
fn attack_every_target_is_listed_here() {
    let listed: BTreeSet<String> = TARGETS.iter().map(|(n, _)| (*n).to_owned()).collect();
    assert_eq!(target_modules(), listed);
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which WASI cannot without a preopened folder"
)]
#[test]
fn attack_every_target_has_its_tests_and_is_in_both_ci_matrices() {
    let workflow = read(".github/workflows/kernel-fuzz.yml");
    let targets = target_modules();
    let tests = test_files();
    assert_eq!(
        layout_violations(&targets, &tests, &workflow),
        Vec::<String>::new()
    );

    // Seeded violations: each must be reported.
    let drop_fuzz = workflow.replace(
        "[boxtree_fuzz, curves_fuzz, linalg_fuzz, nurbs_fuzz, orient2d_fuzz, predicates_fuzz, roots_fuzz, surfaces_fuzz, transforms_fuzz, variants_fuzz, wide_fuzz]",
        "[boxtree_fuzz, curves_fuzz, linalg_fuzz, nurbs_fuzz, predicates_fuzz, roots_fuzz, surfaces_fuzz, transforms_fuzz, variants_fuzz, wide_fuzz]",
    );
    assert_ne!(drop_fuzz, workflow);
    assert!(
        layout_violations(&targets, &tests, &drop_fuzz)
            .iter()
            .any(|v| v.contains("libFuzzer matrix"))
    );
    let drop_proof = workflow.replace("- orient2d_proof\n", "");
    assert_ne!(drop_proof, workflow);
    assert!(
        layout_violations(&targets, &tests, &drop_proof)
            .iter()
            .any(|v| v.contains("Kani matrix"))
    );
    let mut no_seeded = tests.clone();
    let text = no_seeded.get_mut("orient2d").unwrap();
    *text = text.replace("fn orient2d_seeded()", "fn orient2d_seeds()");
    assert!(
        layout_violations(&targets, &no_seeded, &workflow)
            .iter()
            .any(|v| v.contains("orient2d_seeded"))
    );
    let mut typed = tests.clone();
    let text = typed.get_mut("orient2d").unwrap();
    *text = text.replace(
        ".for_each(|data: &[u8]| holds(harness(data)))",
        ".for_each(|data: &[u8]| holds(harness(&data[1..])))",
    );
    assert!(
        layout_violations(&targets, &typed, &workflow)
            .iter()
            .any(|v| v.contains("raw bytes"))
    );
    let mut stray = tests.clone();
    stray.insert(
        "extra".to_owned(),
        "#[test]\nfn extra_fuzz() {}\n".to_owned(),
    );
    assert!(
        layout_violations(&targets, &stray, &workflow)
            .iter()
            .any(|v| v.contains("extra_fuzz"))
    );
    // A target written in a helper, as the first Kani trial harnesses were: ClusterFuzzLite's
    // build failed on it with "no targets matched".
    // The call is assembled, so this file's own text, which the layout check reads, has none.
    let call = ["check", "!()"].concat();
    let mut helper = tests.clone();
    helper.insert(
        "helper".to_owned(),
        format!(
            "fn trial(bound: u64) {{\n    {call}.for_each(|data: &[u8]| harness(data));\n}}\n\n\
             #[test]\nfn helper_trial() {{\n    trial(0);\n}}\n"
        ),
    );
    let v = layout_violations(&targets, &helper, &workflow);
    assert!(
        v.iter()
            .any(|v| v.contains("tests/helper.rs") && v.contains("in trial,")),
        "{v:?}"
    );
    // Written in the test itself, the same call passes, and so does a commented-out one.
    assert_eq!(
        checks_outside_tests(&format!(
            "#[test]\n#[cfg_attr(kani, kani::solver(cadical))]\nfn t() {{\n    {call}.for_each(|_| ());\n}}\n\
             fn helper() {{\n    // {call}\n}}\n"
        )),
        Vec::<String>::new()
    );
    let mut missing = targets.clone();
    missing.insert("newtarget".to_owned());
    assert!(
        layout_violations(&missing, &tests, &workflow)
            .iter()
            .any(|v| v.contains("tests/newtarget.rs"))
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which WASI cannot without a preopened folder"
)]
#[test]
fn attack_a_should_panic_bolero_test_is_refused() {
    let workflow = read(".github/workflows/kernel-fuzz.yml");
    let targets = target_modules();
    let tests = test_files();
    // The call is assembled, so this file's own text, which the layout check reads, has none.
    let call = ["check", "!()"].concat();
    // A #[should_panic] bolero test, as G11's bolero_fails_on_a_finding was: ClusterFuzzLite's
    // build failed whenever cargo-bolero happened to list it first in its binary.
    let mut panicking = tests.clone();
    panicking.insert(
        "panicking".to_owned(),
        format!(
            "#[test]\n#[should_panic(expected = \"test failed\")]\nfn fails() {{\n    \
             {call}.for_each(|_: &[u8]| Err(()));\n}}\n"
        ),
    );
    let v = layout_violations(&targets, &panicking, &workflow);
    assert!(
        v.iter()
            .any(|v| v.contains("tests/panicking.rs") && v.contains("#[should_panic]")),
        "{v:?}"
    );
    // A proof test that returns its finding, as the first proofs did.
    let mut returning = tests.clone();
    let text = returning.get_mut("orient2d").unwrap();
    *text = text.replace(
        ".for_each(|draw| holds(prove(PROOF_BOUND, draw)));",
        ".for_each(|draw| prove(PROOF_BOUND, draw));",
    );
    let v = layout_violations(&targets, &returning, &workflow);
    assert!(
        v.iter()
            .any(|v| v.contains("orient2d_proof") && v.contains("holds")),
        "{v:?}"
    );
    // A #[should_panic] test without check!, and a bolero test that catches the panic itself,
    // pass.
    assert_eq!(
        should_panic_checks(&format!(
            "#[test]\n#[should_panic]\nfn plain() {{\n    panic!();\n}}\n\
             #[test]\nfn caught() {{\n    let _ = std::panic::catch_unwind(|| {{\n        \
             {call}.for_each(|_: &[u8]| ());\n    }});\n}}\n"
        )),
        Vec::<String>::new()
    );
}

/// The `paths:` lists of each trigger of a workflow's `on:` block, by trigger.
fn workflow_path_filters(yaml: &str) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut in_on = false;
    let mut trigger: Option<String> = None;
    let mut in_paths = false;
    for line in yaml.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if indent == 0 {
            in_on = t == "on:";
            trigger = None;
            in_paths = false;
            continue;
        }
        if !in_on {
            continue;
        }
        if indent == 2 && t.ends_with(':') {
            trigger = Some(t.trim_end_matches(':').to_owned());
            in_paths = false;
        } else if t == "paths:" {
            in_paths = true;
            out.entry(trigger.clone().unwrap()).or_default();
        } else if in_paths && t.starts_with("- ") {
            let item = t[2..].trim().trim_matches('"').to_owned();
            out.get_mut(trigger.as_ref().unwrap()).unwrap().push(item);
        } else {
            in_paths = false;
        }
    }
    out
}

/// Whether a path filter written as an exact path or as `<folder>/**` matches `path`. Those are
/// the only two forms kernel-fuzz.yml uses; any other form matches nothing here, so a filter
/// written another way is reported rather than trusted.
fn filter_matches(pattern: &str, path: &str) -> bool {
    match pattern.strip_suffix("/**") {
        Some(folder) => !folder.contains('*') && path.starts_with(&format!("{folder}/")),
        None => !pattern.contains('*') && pattern == path,
    }
}

/// What the workflow's path filters miss: every crate folder in `folders` (its `Cargo.toml` and
/// `src/lib.rs`) and the files every build reads, under both triggers.
fn path_filter_violations(workflow: &str, folders: &BTreeSet<String>) -> Vec<String> {
    let filters = workflow_path_filters(workflow);
    let mut found = Vec::new();
    if filters.keys().map(String::as_str).collect::<Vec<_>>() != ["pull_request", "push"] {
        found.push(format!(
            "could not read both triggers' path filters: {filters:?}"
        ));
        return found;
    }
    let mut needed: Vec<String> = folders
        .iter()
        .flat_map(|f| [format!("{f}/Cargo.toml"), format!("{f}/src/lib.rs")])
        .collect();
    needed.extend(
        [
            "Cargo.toml",
            "Cargo.lock",
            ".cargo/config.toml",
            "rust-toolchain.toml",
            ".clusterfuzzlite/build.sh",
            ".clusterfuzzlite/Dockerfile",
            ".github/workflows/kernel-fuzz.yml",
        ]
        .map(str::to_owned),
    );
    for (trigger, patterns) in &filters {
        for path in &needed {
            if !patterns.iter().any(|pat| filter_matches(pat, path)) {
                found.push(format!("{trigger}: no path filter covers {path}"));
            }
        }
    }
    found
}

/// The folders, relative to the workspace root, of every crate of this repository that geom-fuzz
/// builds with: itself and every path package it reaches by normal or build dependencies, read
/// from cargo's resolved graph.
fn crates_geom_fuzz_builds() -> BTreeSet<String> {
    let metadata =
        geom_test::deps::cargo_metadata(&cargo(), &crate_dir().join("Cargo.toml")).unwrap();
    let root = PathBuf::from(metadata["workspace_root"].as_str().unwrap());
    let mut folder_of: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut fuzz_id = None;
    for package in metadata["packages"].as_array().unwrap() {
        let id = package["id"].as_str().unwrap().to_owned();
        if package["name"] == "geom-fuzz" {
            fuzz_id = Some(id.clone());
        }
        let folder = package["source"].is_null().then(|| {
            Path::new(package["manifest_path"].as_str().unwrap())
                .parent()
                .unwrap()
                .strip_prefix(&root)
                .unwrap()
                .components()
                .map(|c| c.as_os_str().to_str().unwrap().to_owned())
                .collect::<Vec<_>>()
                .join("/")
        });
        folder_of.insert(id, folder);
    }
    let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for node in metadata["resolve"]["nodes"].as_array().unwrap() {
        let from = node["id"].as_str().unwrap().to_owned();
        for dep in node["deps"].as_array().unwrap() {
            let built = dep["dep_kinds"]
                .as_array()
                .unwrap()
                .iter()
                .any(|k| k["kind"].is_null() || k["kind"] == "build");
            if built {
                edges
                    .entry(from.clone())
                    .or_default()
                    .push(dep["pkg"].as_str().unwrap().to_owned());
            }
        }
    }
    let mut seen = BTreeSet::new();
    let mut todo = vec![fuzz_id.unwrap()];
    while let Some(id) = todo.pop() {
        if seen.insert(id.clone()) {
            todo.extend(edges.get(&id).cloned().unwrap_or_default());
        }
    }
    seen.iter().filter_map(|id| folder_of[id].clone()).collect()
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "runs cargo as a child process, which WebAssembly cannot"
)]
#[test]
fn attack_the_workflow_runs_when_any_crate_a_target_builds_changes() {
    let folders = crates_geom_fuzz_builds();
    for expected in ["geom-fuzz", "geom-num", "geom-test"] {
        assert!(
            folders.contains(&format!("projects/engines/geometry/crates/{expected}")),
            "{folders:?}"
        );
    }
    let workflow = read(".github/workflows/kernel-fuzz.yml");
    assert_eq!(
        path_filter_violations(&workflow, &folders),
        Vec::<String>::new()
    );

    // Seeded violations: a crate dropped from one trigger, a filter written in a form not
    // trusted, and a new crate that a target starts to build.
    let dropped = workflow.replacen(
        "      - \"projects/engines/geometry/crates/geom-num/**\"\n",
        "",
        1,
    );
    assert_ne!(dropped, workflow);
    let v = path_filter_violations(&dropped, &folders);
    assert!(
        v.iter()
            .any(|v| v.starts_with("pull_request:") && v.contains("geom-num")),
        "{v:?}"
    );
    let wildcard = workflow.replace(
        "\"projects/engines/geometry/crates/geom-run/**\"",
        "\"projects/engines/geometry/crates/geom-r*/**\"",
    );
    assert!(
        path_filter_violations(&wildcard, &folders)
            .iter()
            .any(|v| v.contains("geom-run"))
    );
    let mut more = folders.clone();
    more.insert("projects/engines/geometry/crates/geom-bool".to_owned());
    assert!(
        path_filter_violations(&workflow, &more)
            .iter()
            .any(|v| v.contains("geom-bool"))
    );
}

/// The value of `KEY:` in a workflow's `env:` block, unquoted.
fn env_value(yaml: &str, key: &str) -> Option<String> {
    yaml.lines()
        .find_map(|l| l.trim().strip_prefix(&format!("{key}:")))
        .map(|v| v.trim().trim_matches('"').to_owned())
}

/// Every token in `text` that follows `marker` (up to the next space), e.g. the version after
/// `cargo-bolero --version `.
fn after(text: &str, marker: &str) -> Vec<String> {
    text.match_indices(marker)
        .map(|(i, _)| {
            text[i + marker.len()..]
                .split(|c: char| c.is_whitespace() || c == '`' || c == '"')
                .next()
                .unwrap_or_default()
                .to_owned()
        })
        .collect()
}

/// Every dated nightly toolchain named in `text`: `nightly-` and a date.
fn nightlies(text: &str) -> Vec<String> {
    after(text, "nightly-")
        .into_iter()
        .filter(|d| d.len() >= 10 && d[..10].chars().filter(char::is_ascii_digit).count() == 8)
        .map(|d| format!("nightly-{}", &d[..10]))
        .collect()
}

/// Where the pins disagree: the nightly compiler, cargo-bolero and Kani, between the workflow and
/// every file that names them.
fn pin_violations(workflow: &str, files: &[(&str, &str)]) -> Vec<String> {
    let mut found = Vec::new();
    let Some(nightly) = env_value(workflow, "FUZZ_NIGHTLY") else {
        return vec!["kernel-fuzz.yml pins no FUZZ_NIGHTLY".to_owned()];
    };
    let Some(bolero) = env_value(workflow, "CARGO_BOLERO_VERSION") else {
        return vec!["kernel-fuzz.yml pins no CARGO_BOLERO_VERSION".to_owned()];
    };
    let Some(kani) = env_value(workflow, "KANI_VERSION") else {
        return vec!["kernel-fuzz.yml pins no KANI_VERSION".to_owned()];
    };
    for (name, text) in files {
        for n in nightlies(text).into_iter().filter(|n| *n != nightly) {
            found.push(format!("{name} names {n}; kernel-fuzz.yml pins {nightly}"));
        }
        for v in after(text, "cargo-bolero --version ")
            .into_iter()
            .filter(|v| *v != bolero)
        {
            found.push(format!(
                "{name} installs cargo-bolero {v}; kernel-fuzz.yml pins {bolero}"
            ));
        }
        for v in after(text, "kani-verifier --version ")
            .into_iter()
            .filter(|v| *v != kani)
        {
            found.push(format!(
                "{name} installs Kani {v}; kernel-fuzz.yml pins {kani}"
            ));
        }
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which WASI cannot without a preopened folder"
)]
#[test]
fn attack_the_tool_pins_agree_everywhere_they_are_written() {
    let workflow = read(".github/workflows/kernel-fuzz.yml");
    let dockerfile = read(".clusterfuzzlite/Dockerfile");
    let readme = read("projects/engines/geometry/crates/geom-fuzz/README.md");
    let lib = read("projects/engines/geometry/crates/geom-fuzz/src/lib.rs");
    let files = [
        ("Dockerfile", dockerfile.as_str()),
        ("README.md", readme.as_str()),
        ("src/lib.rs", lib.as_str()),
    ];
    assert_eq!(pin_violations(&workflow, &files), Vec::<String>::new());
    // Each file names each pin it should, so the agreement is not vacuous.
    assert_ne!(
        after(&dockerfile, "cargo-bolero --version "),
        Vec::<String>::new()
    );
    assert_ne!(
        after(&readme, "cargo-bolero --version "),
        Vec::<String>::new()
    );
    assert_ne!(
        after(&readme, "kani-verifier --version "),
        Vec::<String>::new()
    );
    assert_ne!(nightlies(&readme), Vec::<String>::new());
    assert_ne!(nightlies(&lib), Vec::<String>::new());

    // Seeded violations.
    let old = readme.replace("nightly-2026-10-01", "nightly-2026-09-01");
    assert!(
        pin_violations(&workflow, &[("README.md", &old)])
            .iter()
            .any(|v| v.contains("nightly-2026-09-01"))
    );
    let docker = dockerfile.replace("--version 0.13.5", "--version 0.13.4");
    assert!(
        pin_violations(&workflow, &[("Dockerfile", &docker)])
            .iter()
            .any(|v| v.contains("0.13.4"))
    );
    let kani = readme.replace(
        "kani-verifier --version 0.68.0",
        "kani-verifier --version 0.67.0",
    );
    assert!(
        pin_violations(&workflow, &[("README.md", &kani)])
            .iter()
            .any(|v| v.contains("0.67.0"))
    );
}

/// What goes wrong if the workflow turns OSS-Fuzz's build check off: build.sh must then make the
/// same checks, keep only the fuzz tests' scripts, and parallel fuzzing must stay off.
fn build_check_violations(workflow: &str, build_sh: &str) -> Vec<String> {
    let mut found = Vec::new();
    if workflow.contains("parallel-fuzzing: true") {
        found.push(
            "parallel fuzzing is on; libFuzzer's -jobs and -fork relaunch the binary without \
             the start-up script's arguments"
                .to_owned(),
        );
    }
    if !workflow.contains("bad-build-check: false") {
        return found;
    }
    for (needle, what) in [
        ("readelf -h", "that each binary is an x86-64 ELF executable"),
        (
            "objdump -dC",
            "the count of calls into the address sanitizer",
        ),
        ("-lt 1000", "the threshold of 1000 address-sanitizer calls"),
        ("Done 64 runs", "a clean run of 64 inputs"),
        ("-lt 100 ", "the threshold of 100 instrumented edges"),
        (
            "*_fuzz_fuzzer",
            "keeping only the <target>_fuzz tests' scripts",
        ),
        ("exit 1", "failing the build"),
    ] {
        if !build_sh.contains(needle) {
            found.push(format!(
                "kernel-fuzz.yml turns OSS-Fuzz's build check off, but build.sh does not check \
                 {what} (`{needle}`)"
            ));
        }
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which WASI cannot without a preopened folder"
)]
#[test]
fn attack_the_build_check_turned_off_is_made_by_build_sh() {
    let workflow = read(".github/workflows/kernel-fuzz.yml");
    let build_sh = read(".clusterfuzzlite/build.sh");
    assert!(workflow.contains("bad-build-check: false"));
    assert_eq!(
        build_check_violations(&workflow, &build_sh),
        Vec::<String>::new()
    );
    for needle in ["readelf -h", "objdump -dC", "Done 64 runs", "*_fuzz_fuzzer"] {
        let weakened = build_sh.replace(needle, "true");
        assert!(
            build_check_violations(&workflow, &weakened)
                .iter()
                .any(|v| v.contains(needle)),
            "{needle}"
        );
    }
    let parallel = workflow.replace(
        "mode: code-change",
        "mode: code-change\n          parallel-fuzzing: true",
    );
    assert!(
        build_check_violations(&parallel, &build_sh)
            .iter()
            .any(|v| v.contains("parallel"))
    );
}

/// What the workspace's `fuzz` profile gets wrong for cargo-bolero, given the root `Cargo.toml`.
fn profile_violations(cargo_toml: &str) -> Vec<String> {
    let Some(start) = cargo_toml.find("\n[profile.fuzz]\n") else {
        return vec![
            "the root Cargo.toml has no [profile.fuzz], cargo-bolero's default".to_owned(),
        ];
    };
    let body = &cargo_toml[start + "\n[profile.fuzz]\n".len()..];
    let body = body.split("\n[").next().unwrap_or_default();
    let keys: BTreeMap<&str, &str> = body
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim(), v.trim()))
        .collect();
    let mut found = Vec::new();
    if keys.get("inherits") != Some(&"\"dev\"") {
        found.push(
            "[profile.fuzz] must inherit the dev profile, so debug assertions and overflow \
             checks stay on"
                .to_owned(),
        );
    }
    for off in ["debug-assertions", "overflow-checks"] {
        if keys.get(off) == Some(&"false") {
            found.push(format!("[profile.fuzz] turns {off} off"));
        }
    }
    if keys.contains_key("lto") {
        found.push(
            "[profile.fuzz] sets lto, which bolero's book says fuzzing does not support well"
                .to_owned(),
        );
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which WASI cannot without a preopened folder"
)]
#[test]
fn attack_the_workspace_has_the_profile_cargo_bolero_builds_with() {
    let cargo_toml = read("Cargo.toml");
    assert_eq!(profile_violations(&cargo_toml), Vec::<String>::new());
    let none = cargo_toml.replace("[profile.fuzz]", "[profile.fuzzing]");
    assert_ne!(profile_violations(&none), Vec::<String>::new());
    let release = cargo_toml.replace("inherits = \"dev\"", "inherits = \"release\"");
    assert_ne!(profile_violations(&release), Vec::<String>::new());
    let unchecked = cargo_toml.replace(
        "codegen-units = 1",
        "codegen-units = 1\noverflow-checks = false",
    );
    assert_ne!(profile_violations(&unchecked), Vec::<String>::new());
    let lto = cargo_toml.replace("codegen-units = 1", "codegen-units = 1\nlto = true");
    assert_ne!(profile_violations(&lto), Vec::<String>::new());
}
