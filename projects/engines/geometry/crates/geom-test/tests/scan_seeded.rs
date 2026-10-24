//! The banned-source scanner, on seeded positives and on its cited fixture.
//!
//! Each category of marker (`geom_test::scan::markers`) is planted here at least once, in the
//! forms chapter 29.20 and evidence reports 12 and 13 describe, and the scanner must report each
//! with its line, category and marker; ordinary kernel prose with near misses must pass. The cited
//! fixture, `tests/scan/cash_karp.rs`, implements the same method as a routine the kernel may
//! never take, written from the primary sources with every constant derived and cited: the scan
//! must pass it, and the tests below show it is a working integrator, not a strawman. Last, the
//! workspace scan and its binary are run on scratch workspaces: a seeded file fails the check
//! naming file and line, build output is skipped, and an exemption that no longer applies fails.
//!
//! This file plants markers on purpose, so it is one of the scanner's named exemptions
//! (`geom_test::scan::repo::EXEMPT`).

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them, and std's reductions.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    clippy::float_cmp,
    reason = "fixtures and oracles; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken fixture must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

#[path = "scan/cash_karp.rs"]
mod cash_karp;

use geom_test::scan::{Category, Hit, scan_text};
use num_rational::Ratio;

/// The (line, category, marker) of every hit in `text`.
fn hits(text: &str) -> Vec<(usize, Category, String)> {
    scan_text(text)
        .into_iter()
        .map(|Hit { line, category, marker }| (line, category, marker))
        .collect()
}

/// Asserts that `text` holds a hit of `category` whose marker is `marker`, on `line`.
fn finds(text: &str, line: usize, category: Category, marker: &str) {
    let found = hits(text);
    assert!(
        found
            .iter()
            .any(|(l, c, m)| *l == line && *c == category && m == marker),
        "{category:?} {marker:?} on line {line} not found in {text:?}: {found:#?}"
    );
}

/// Asserts that `text` holds no hit at all.
fn passes(text: &str) {
    let found = hits(text);
    assert!(found.is_empty(), "{text:?} should pass: {found:#?}");
}

// ---------------------------------------------------------------------------------------------
// Numerical Recipes.
// ---------------------------------------------------------------------------------------------

#[test]
fn numerical_recipes_routine_names_are_found_in_any_identifier() {
    let nr = Category::NumericalRecipes;
    finds("void svdcmp(double **a, int m, int n)", 1, nr, "svdcmp");
    finds("fn tqli(d: &mut [f64])", 1, nr, "tqli");
    finds("\n\nlet r = rkqs(y, dydx);", 3, nr, "rkqs");
    finds("#define ST_pythag(a, b) hypot(a, b)", 1, nr, "ST_pythag");
    finds("double smos_Pythag(double a, double b);", 1, nr, "smos_Pythag");
    finds("fn smosPythag(a: f64, b: f64) -> f64", 1, nr, "smosPythag");
    finds("PolynomialInterpolation = polint;", 1, nr, "polint");
    finds("// trapzd, qromb and qsimp", 1, nr, "trapzd");
    finds("// trapzd, qromb and qsimp", 1, nr, "qromb");
    finds("// trapzd, qromb and qsimp", 1, nr, "qsimp");
    finds("call RKCK(y, dydx, n, x, h)", 1, nr, "RKCK");
    finds("fn ludcmp_in_place()", 1, nr, "ludcmp_in_place");
    finds("tred2(a, n, d, e);", 1, nr, "tred2");
    finds("float dbrent(float ax);", 1, nr, "dbrent");
    finds("\"svbksb\"", 1, nr, "svbksb");
}

#[test]
fn numerical_recipes_macros_variables_authors_and_constant_are_found() {
    let nr = Category::NumericalRecipes;
    finds("#define ZEPS 1.0e-10", 1, nr, "ZEPS");
    finds("SIGN(a, b)", 1, nr, "SIGN");
    finds("MOV3(a, b, c, d, e, f)", 1, nr, "MOV3");
    finds("#define ERRCON 1.89e-4", 1, nr, "ERRCON");
    finds("#define ERRCON 1.89e-4", 1, nr, "1.89e-4");
    finds("const PGROW: f64 = -0.2;\nconst PSHRNK: f64 = -0.25;", 2, nr, "PSHRNK");
    finds("let limit = 0.000_189;", 1, nr, "0.000_189");
    finds("let limit = 1.89E-04_f64;", 1, nr, "1.89E-04");
    finds("let ak2 = vec![0.0; n];", 1, nr, "ak2");
    finds("fn step(htry: f64, hdid: &mut f64, hnext: &mut f64)", 1, nr, "htry");
    finds("let yscal = y.abs() + (dydx * h).abs();", 1, nr, "yscal");
    finds("//! Taken from Numerical Recipes in C.", 1, nr, "Numerical Recipes");
    finds("* algorithms are taken from Numerical Recipies in C", 1, nr, "Numerical Recipies");
    finds("// Press, Teukolsky, Vetterling and Flannery", 1, nr, "Teukolsky");
    finds("//! numerical\n//! RECIPES, chapter 16", 1, nr, "numerical RECIPES");
}

// ---------------------------------------------------------------------------------------------
// Copyleft notices and taken-from statements.
// ---------------------------------------------------------------------------------------------

#[test]
fn copyleft_notices_are_found() {
    let notice = Category::CopyleftNotice;
    let header = "// This program is free software: you can redistribute it and/or modify\n\
                  // it under the terms of the GNU General Public License as published by\n\
                  // the Free Software Foundation, either version 3 of the License.";
    finds(header, 2, notice, "GNU General Public License");
    finds(header, 3, notice, "Free Software Foundation");
    finds(
        "under the terms of the GNU Lesser General Public License version 2.1",
        1,
        notice,
        "GNU Lesser General Public License",
    );
    finds(
        "GNU Affero General Public License",
        1,
        notice,
        "GNU Affero General Public License",
    );
}

#[test]
fn taken_from_copyleft_statements_are_found() {
    let taken = Category::TakenFromCopyleft;
    finds("// Ported from OCCT's BRepAlgoAPI_Fuse.", 1, taken, "Ported from OCCT");
    finds("//! Adapted from CGAL", 1, taken, "Adapted from CGAL");
    finds(
        "//! This follows the approach, based on the FreeCAD element map,",
        1,
        taken,
        "based on the FreeCAD",
    );
    finds("// taken from LibreDWG's decoder", 1, taken, "taken from LibreDWG");
    finds("/* mirrors SolveSpace's solver */", 1, taken, "mirrors SolveSpace");
    finds(
        "// derived from GPL-3.0 code",
        1,
        taken,
        "derived from GPL",
    );
    finds(
        "//! The arrangement is copied from\n//! OpenCascade.",
        1,
        taken,
        "copied from OpenCascade",
    );
    finds(
        "// translated, with changes, from the LGPL-2.1 sources",
        1,
        taken,
        "translated with changes from the LGPL",
    );
    finds("// a port of Open CASCADE's mesher", 1, taken, "port of Open CASCADE");
    finds("// based on BRL-CAD's raytracer", 1, taken, "based on BRL CAD");
}

#[test]
fn honest_statements_and_permissive_sources_pass() {
    passes("//! No code is taken from OCCT; the method note describes it.");
    passes("//! Nothing here was copied from CGAL.");
    passes("//! This is not based on FreeCAD's element map.");
    passes("//! A port of CORE-MATH's exp, MIT, with its notice kept.");
    passes("//! Based on Shewchuk's paper and Geogram's BSD-3-Clause predicates.");
    passes(
        "//! Consulted: OCCT `BRepTools_History`, LGPL-2.1-only WITH OCCT-exception-1.0,\n\
         //!   <https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/x.hxx>,\n\
         //!   for the history contract.",
    );
    // A copyleft name farther than twelve words after the phrase is another sentence's subject.
    passes(
        "//! Based on the paper by its authors, read in full and checked against our own \
         tests; OCCT is the oracle.",
    );
}

// ---------------------------------------------------------------------------------------------
// Autodesk patterns and linetypes.
// ---------------------------------------------------------------------------------------------

#[test]
fn autodesk_pattern_names_and_descriptions_are_found() {
    let acad = Category::AcadPattern;
    finds("*ANSI31, ANSI Iron, Brick, Stone masonry", 1, acad, "ANSI31");
    finds("let pattern = \"ansi37\";", 1, acad, "ansi37");
    finds("Pattern::new(\"AR-CONC\")", 1, acad, "AR-CONC");
    finds("// AR-B816C and AR-HBONE", 1, acad, "AR-B816C");
    finds("// AR-B816C and AR-HBONE", 1, acad, "AR-HBONE");
    finds("linetype ACAD_ISO02W100", 1, acad, "ACAD_ISO02W100");
    finds(
        "(\"ANSI31\", \"Iron, brick, stone masonry\")",
        1,
        acad,
        "Iron, brick, stone masonry",
    );
    finds("ZIGZAG, Zig zag /\\/\\/\\/\\/", 1, acad, "Zig zag /\\/\\/");
    finds("\n\"Fenceline circle ----0-----0----\"", 2, acad, "Fenceline circle");
}

// ---------------------------------------------------------------------------------------------
// Near misses.
// ---------------------------------------------------------------------------------------------

#[test]
fn ordinary_kernel_words_are_not_markers() {
    passes(
        "//! The Pythagorean sum, Brent's method, Jacobi rotations, a cubic spline and the golden \
         section search are written from Higham and from Golub and Van Loan. The sign of the \
         orientation, `Sign::Positive`, SIGNED distances and a sign_bit; a zig-zag polyline; \
         the CENTER line and HIDDEN edges; a brick wall; a safety factor of 0.9 and an exponent \
         of -0.2; 1.89 and 1.89e-3; ACAD_PROXY_ENTITY; AR-15 and ANSI-31; the clamped knot \
         vector; Triangle inequality; mirrors stay exact; geom_num::sign(x).",
    );
}

// ---------------------------------------------------------------------------------------------
// The cited Cash-Karp fixture.
// ---------------------------------------------------------------------------------------------

/// The fixture's text, as the repository holds it.
const CASH_KARP: &str = include_str!("scan/cash_karp.rs");

#[test]
fn the_cited_cash_karp_file_passes() {
    passes(CASH_KARP);
}

#[test]
fn the_same_method_with_the_books_names_and_constants_fails() {
    let tainted = format!(
        "{CASH_KARP}\n/// Step control as the book has it.\nconst ERRCON: f64 = 1.89e-4;\n\
         fn rkqs(htry: f64) -> f64 {{ htry }}\n"
    );
    let found = hits(&tainted);
    let markers: Vec<&str> = found.iter().map(|(_, _, m)| m.as_str()).collect();
    for wanted in ["ERRCON", "1.89e-4", "rkqs", "htry"] {
        assert!(markers.contains(&wanted), "{wanted} not found: {found:#?}");
    }
    assert!(
        found.iter().all(|(_, c, _)| *c == Category::NumericalRecipes),
        "{found:#?}"
    );
}

/// A coefficient of the tableau as an exact rational.
fn exact((numerator, denominator): cash_karp::Rational) -> Ratio<i128> {
    Ratio::new(i128::from(numerator), i128::from(denominator))
}

/// The coupling matrix as exact rationals, row by row, padded with zeros to six columns.
fn coupling() -> Vec<Vec<Ratio<i128>>> {
    cash_karp::COUPLING
        .iter()
        .map(|row| {
            let mut full: Vec<Ratio<i128>> = row.iter().map(|c| exact(*c)).collect();
            full.resize(6, Ratio::from_integer(0));
            full
        })
        .collect()
}

/// `a · v`: the matrix applied to a vector.
fn apply(a: &[Vec<Ratio<i128>>], v: &[Ratio<i128>]) -> Vec<Ratio<i128>> {
    a.iter()
        .map(|row| {
            row.iter()
                .zip(v)
                .fold(Ratio::from_integer(0), |sum, (x, y)| sum + x * y)
        })
        .collect()
}

/// `b · v`: the weights applied to a vector.
fn weigh(b: &[Ratio<i128>], v: &[Ratio<i128>]) -> Ratio<i128> {
    b.iter()
        .zip(v)
        .fold(Ratio::from_integer(0), |sum, (x, y)| sum + x * y)
}

/// The elementwise product of two vectors.
fn times(u: &[Ratio<i128>], v: &[Ratio<i128>]) -> Vec<Ratio<i128>> {
    u.iter().zip(v).map(|(x, y)| x * y).collect()
}

/// The conditions a Runge-Kutta method of order 5 meets, one per rooted tree up to five nodes
/// (Hairer, Nørsett and Wanner, section II.2), each as (left side, the value 1/γ(t)), for the
/// weights `b`.
fn order_conditions(b: &[Ratio<i128>]) -> Vec<(&'static str, Ratio<i128>, Ratio<i128>)> {
    let a = coupling();
    let c: Vec<Ratio<i128>> = cash_karp::NODES.iter().map(|n| exact(*n)).collect();
    let one = vec![Ratio::from_integer(1); 6];
    let c2 = times(&c, &c);
    let c3 = times(&c2, &c);
    let c4 = times(&c3, &c);
    let ac = apply(&a, &c);
    let ac2 = apply(&a, &c2);
    let ac3 = apply(&a, &c3);
    let aac = apply(&a, &ac);
    let aac2 = apply(&a, &ac2);
    let aaac = apply(&a, &aac);
    let r = |n: i128, d: i128| Ratio::new(n, d);
    vec![
        ("order 1", weigh(b, &one), r(1, 1)),
        ("order 2", weigh(b, &c), r(1, 2)),
        ("order 3, c^2", weigh(b, &c2), r(1, 3)),
        ("order 3, a c", weigh(b, &ac), r(1, 6)),
        ("order 4, c^3", weigh(b, &c3), r(1, 4)),
        ("order 4, c (a c)", weigh(b, &times(&c, &ac)), r(1, 8)),
        ("order 4, a c^2", weigh(b, &ac2), r(1, 12)),
        ("order 4, a a c", weigh(b, &aac), r(1, 24)),
        ("order 5, c^4", weigh(b, &c4), r(1, 5)),
        ("order 5, c^2 (a c)", weigh(b, &times(&c2, &ac)), r(1, 10)),
        ("order 5, c (a c^2)", weigh(b, &times(&c, &ac2)), r(1, 15)),
        ("order 5, c (a a c)", weigh(b, &times(&c, &aac)), r(1, 30)),
        ("order 5, (a c)^2", weigh(b, &times(&ac, &ac)), r(1, 20)),
        ("order 5, a c^3", weigh(b, &ac3), r(1, 20)),
        ("order 5, a (c (a c))", weigh(b, &apply(&a, &times(&c, &ac))), r(1, 40)),
        ("order 5, a a c^2", weigh(b, &aac2), r(1, 60)),
        ("order 5, a a a c", weigh(b, &aaac), r(1, 120)),
    ]
}

#[test]
fn the_tableau_meets_every_order_condition_exactly() {
    // Each row's coupling sums to its node, so each stage is consistent.
    for (row, node) in coupling().iter().zip(cash_karp::NODES) {
        let sum = row.iter().fold(Ratio::from_integer(0), |s, x| s + x);
        assert_eq!(sum, exact(node), "row sum");
    }
    let fifth: Vec<Ratio<i128>> = cash_karp::FIFTH_ORDER.iter().map(|w| exact(*w)).collect();
    let fourth: Vec<Ratio<i128>> = cash_karp::FOURTH_ORDER.iter().map(|w| exact(*w)).collect();
    let conditions = order_conditions(&fifth);
    assert_eq!(conditions.len(), 17, "one condition per rooted tree up to order 5");
    for (name, left, right) in &conditions {
        assert_eq!(left, right, "the fifth-order weights fail {name}");
    }
    let conditions = order_conditions(&fourth);
    for (name, left, right) in &conditions[..8] {
        assert_eq!(left, right, "the fourth-order weights fail {name}");
    }
    assert!(
        conditions[8..].iter().any(|(_, left, right)| left != right),
        "the embedded weights are of order 4 exactly, so the two solutions differ"
    );
}

/// Tight tolerances for the accuracy tests.
const TIGHT: cash_karp::Tolerance = cash_karp::Tolerance {
    absolute: 1e-12,
    relative: 1e-12,
};

#[test]
fn the_integrator_reaches_exact_solutions() {
    // y' = y from y(0) = 1: y(1) = e.
    let grow = |_: f64, y: &[f64]| vec![y[0]];
    let e = geom_num::math::exp(1.0).unwrap().get();
    let y = cash_karp::integrate(&grow, 0.0, &[1.0], 1.0, 0.1, TIGHT, 10_000).unwrap();
    assert!((y[0] - e).abs() < 1e-9, "{} against {e}", y[0]);

    // y' = -2ty from y(0) = 1: y(1) = 1/e.
    let decay = |t: f64, y: &[f64]| vec![-2.0 * t * y[0]];
    let inverse = geom_num::math::exp(-1.0).unwrap().get();
    let y = cash_karp::integrate(&decay, 0.0, &[1.0], 1.0, 0.1, TIGHT, 10_000).unwrap();
    assert!((y[0] - inverse).abs() < 1e-9, "{} against {inverse}", y[0]);

    // The harmonic oscillator from (1, 0): (cos 2, -sin 2) at t = 2.
    let swing = |_: f64, y: &[f64]| vec![y[1], -y[0]];
    let cos2 = geom_num::math::unproven::cos(2.0).unwrap().get();
    let sin2 = geom_num::math::unproven::sin(2.0).unwrap().get();
    let y = cash_karp::integrate(&swing, 0.0, &[1.0, 0.0], 2.0, 0.5, TIGHT, 10_000).unwrap();
    assert!((y[0] - cos2).abs() < 1e-9 && (y[1] + sin2).abs() < 1e-9, "{y:?}");
}

#[test]
fn the_error_estimate_shrinks_as_the_fifth_power_of_the_step() {
    // The estimate is the local error of the fourth-order solution, which behaves like h^5, so
    // halving the step divides it by about 2^5 = 32.
    let grow = |_: f64, y: &[f64]| vec![y[0]];
    let unit = cash_karp::Tolerance {
        absolute: 1.0,
        relative: 0.0,
    };
    let coarse = cash_karp::attempt(&grow, 0.0, &[1.0], 0.1, unit).unwrap().error;
    let fine = cash_karp::attempt(&grow, 0.0, &[1.0], 0.05, unit).unwrap().error;
    let ratio = coarse / fine;
    assert!((28.0..36.0).contains(&ratio), "ratio {ratio}");
}

#[test]
fn the_step_size_rule_keeps_its_bounds() {
    use cash_karp::{LARGEST_GROWTH, SAFETY_FACTOR, SMALLEST_SHRINK, step_factor};
    assert_eq!(step_factor(0.0, false).unwrap(), LARGEST_GROWTH);
    assert_eq!(step_factor(0.0, true).unwrap(), 1.0);
    assert_eq!(step_factor(1.0, false).unwrap(), SAFETY_FACTOR);
    assert_eq!(step_factor(1e12, false).unwrap(), SMALLEST_SHRINK);
    assert_eq!(step_factor(1e-12, false).unwrap(), LARGEST_GROWTH);
    assert_eq!(step_factor(1e-12, true).unwrap(), 1.0);
    // An error of 2^5 asks for half the step, times the safety factor.
    let half = step_factor(32.0, false).unwrap();
    assert!((half - 0.45).abs() < 1e-15, "{half}");
}

#[test]
fn the_integrator_fails_loudly() {
    let grow = |_: f64, y: &[f64]| vec![y[0]];
    assert!(matches!(
        cash_karp::integrate(&grow, 0.0, &[1.0], 1.0, 0.1, TIGHT, 3),
        Err(cash_karp::Failure::OutOfSteps { .. })
    ));
    let poison = |_: f64, _: &[f64]| vec![f64::NAN];
    assert_eq!(
        cash_karp::integrate(&poison, 0.0, &[1.0], 1.0, 0.1, TIGHT, 100),
        Err(cash_karp::Failure::NotFinite)
    );
    // y' = y^2 from 1 has no solution past t = 1, so the integration cannot reach t = 2.
    let blow_up = |_: f64, y: &[f64]| vec![y[0] * y[0]];
    let found = cash_karp::integrate(&blow_up, 0.0, &[1.0], 2.0, 0.1, TIGHT, 100_000);
    assert!(found.is_err(), "{found:?}");
}

// ---------------------------------------------------------------------------------------------
// The workspace scan and its binary, on scratch workspaces.
// ---------------------------------------------------------------------------------------------

#[cfg(not(target_family = "wasm"))]
mod workspace {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use geom_test::scan::Category;
    use geom_test::scan::repo::{self, Exempt, Kind};

    /// A scratch workspace under `CARGO_TARGET_TMPDIR`, emptied first and removed on drop.
    struct Scratch(PathBuf);

    const CRATE: &str = "projects/engines/geometry/crates/geom-x";

    impl Scratch {
        fn new(name: &str) -> Scratch {
            let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("scan-{name}"));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(root.join(CRATE).join("src")).unwrap();
            fs::create_dir_all(root.join("projects/engines/crates/engine-geometry/src")).unwrap();
            fs::create_dir_all(root.join("projects/engines/crates/engine-other/src")).unwrap();
            fs::write(root.join(CRATE).join("src/lib.rs"), "//! Clean.\n").unwrap();
            Scratch(root)
        }

        fn write(&self, rel: &str, bytes: &[u8]) {
            let path = self.0.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_seeded_file_is_found_with_its_file_and_line() {
        let s = Scratch::new("seeded");
        assert_eq!(repo::check(&s.0, &[]).unwrap(), []);
        s.write(
            &format!("{CRATE}/src/svd.rs"),
            b"//! Singular values.\n\nfn svdcmp() {}\n",
        );
        // Data files, manifests and binary files are scanned too.
        s.write(&format!("{CRATE}/tests/data/patterns.txt"), b"*AR-SAND, sand\n");
        s.write(&format!("{CRATE}/fixture.bin"), b"\x00\xff\xfeZEPS\x00");
        // The facade's files are scanned; another engine's are not.
        s.write(
            "projects/engines/crates/engine-geometry/src/lib.rs",
            b"// Ported from FreeCAD.\n",
        );
        s.write("projects/engines/crates/engine-other/src/lib.rs", b"fn tqli() {}\n");
        // Build output is skipped.
        s.write(&format!("{CRATE}/target/debug/build.rs"), b"fn rkqs() {}\n");
        let found = repo::check(&s.0, &[]).unwrap();
        let summary: Vec<(String, usize, Category)> = found
            .iter()
            .map(|f| match &f.kind {
                Kind::Hit(hit) => (f.file.clone(), hit.line, hit.category),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(
            summary,
            [
                (
                    "projects/engines/crates/engine-geometry/src/lib.rs".to_owned(),
                    1,
                    Category::TakenFromCopyleft
                ),
                (
                    format!("{CRATE}/fixture.bin"),
                    1,
                    Category::NumericalRecipes
                ),
                (format!("{CRATE}/src/svd.rs"), 3, Category::NumericalRecipes),
                (
                    format!("{CRATE}/tests/data/patterns.txt"),
                    1,
                    Category::AcadPattern
                ),
            ]
        );
        assert!(found[2].to_string().starts_with(&format!("{CRATE}/src/svd.rs:3: ")));
    }

    #[test]
    fn exemptions_hide_their_file_and_must_still_apply() {
        let s = Scratch::new("exempt");
        let file = "projects/engines/geometry/crates/geom-x/src/markers.rs";
        s.write(file, b"const NAMES: [&str; 1] = [\"svdcmp\"];\n");
        let exempt = [Exempt {
            file: "projects/engines/geometry/crates/geom-x/src/markers.rs",
            reason: "the marker table",
        }];
        assert_eq!(repo::check(&s.0, &exempt).unwrap(), []);
        // Once the file holds no marker, or is gone, the exemption itself fails.
        s.write(file, b"const NAMES: [&str; 0] = [];\n");
        let found = repo::check(&s.0, &exempt).unwrap();
        assert!(matches!(found[0].kind, Kind::StaleExemption(_)), "{found:?}");
        fs::remove_file(s.0.join(file)).unwrap();
        let found = repo::check(&s.0, &exempt).unwrap();
        assert!(matches!(found[0].kind, Kind::StaleExemption(_)), "{found:?}");
        // An exemption names one file: a marker in another is still found.
        s.write(&format!("{CRATE}/src/other.rs"), b"fn svdcmp() {}\n");
        s.write(file, b"const NAMES: [&str; 1] = [\"svdcmp\"];\n");
        let found = repo::check(&s.0, &exempt).unwrap();
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].file.ends_with("other.rs"));
    }

    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_is_reported_not_followed() {
        let s = Scratch::new("link");
        let outside = s.0.join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("a.rs"), "fn svdcmp() {}\n").unwrap();
        std::os::unix::fs::symlink(&outside, s.0.join(CRATE).join("src/linked")).unwrap();
        let found = repo::check(&s.0, &[]).unwrap();
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(matches!(found[0].kind, Kind::Unreadable(_)), "{found:?}");
    }

    fn run(args: &[&str]) -> (i32, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_source-scan"))
            .args(args)
            .output()
            .unwrap();
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        (out.status.code().unwrap(), text)
    }

    #[test]
    fn the_binary_exits_0_1_and_2() {
        let s = Scratch::new("binary");
        let root = s.0.to_str().unwrap();
        // The real exemptions name files this scratch workspace does not have, so they are stale.
        let (code, text) = run(&["check", "--root", root]);
        assert_eq!(code, 1, "{text}");
        assert!(text.contains("Stale exemption"), "{text}");
        // The workspace this binary was built in passes.
        let (code, text) = run(&["check"]);
        assert_eq!(code, 0, "{text}");
        s.write(&format!("{CRATE}/src/a.rs"), b"// Numerical Recipes in C\n");
        let (code, text) = run(&["check", "--root", root]);
        assert_eq!(code, 1, "{text}");
        assert!(text.contains("geom-x/src/a.rs:1: Numerical Recipes"), "{text}");
        let (code, text) = run(&["frobnicate"]);
        assert_eq!(code, 2, "{text}");
        let (code, text) = run(&["check", "--root"]);
        assert_eq!(code, 2, "{text}");
        let empty = s.0.join("no-workspace-here");
        fs::create_dir_all(&empty).unwrap();
        let (code, text) = run(&["check", "--root", empty.to_str().unwrap()]);
        assert_eq!(code, 2, "a root without the crates folder is an error, not a pass: {text}");
    }
}
