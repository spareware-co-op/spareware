//! Piece G63's out-of-process comparison with the Clothoids library (chapter 28.10's oracles; 17.8:
//! "the Clothoids library is only an out-of-process test oracle"). The nightly workflow's
//! `clothoids` job runs three steps:
//!
//! 1. `write_the_cases` (ignored) writes seeded clothoids and stations to the file named by
//!    `CLOTHOIDS_CASES`, one line each: x₀ y₀ θ₀ κ κ′ L s, every value the shortest decimal that
//!    reads back to the same double, so the oracle sees what the kernel sees. Each clothoid's data
//!    are dyadic, so its end curvature κ + κ′·L is a double and the kernel's
//!    [`CurvatureLaw::Clothoid`] from κ to it is the same curve exactly.
//! 2. `oracles/clothoids.sh` builds Bertolazzi and Frego's Clothoids (BSD-2-Clause) at a pinned
//!    commit with `oracles/clothoids_harness.cc`, in a container, and writes its point for each
//!    line to the file named by `CLOTHOIDS_ANSWERS`.
//! 3. `compare_with_the_answers` (ignored) evaluates each case with the kernel and compares.
//!
//! The comparison: Clothoids states no error bound, so a point within 10⁻¹² of the length (plus
//! one) of the kernel's certified enclosure agrees. A farther point is settled by the fixed-point
//! truth of `tests/series`, never by widening the allowance: if the kernel's enclosure holds the
//! truth, the difference is Clothoids' and is reported; if not, the kernel is wrong and the job
//! fails. `the_comparison_runs_on_the_kernels_own_points` runs steps 1 and 3 on every build, with
//! the kernel's own doubles as the answers and one answer moved, so the comparison itself is
//! tested where the oracle is not built.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: native arithmetic and indexing, and a failure stops the test"
)]

#[allow(
    dead_code,
    reason = "the shared truth; this test uses the clothoid's part of it"
)]
mod series;

use std::fmt::Write as _;

use dashu_int::IBig;
use geom_curve::{CurvatureLaw, ParamCurve, Transition};
use geom_nurbs::Limit;
use geom_run::{Budget, WorkMeter};
use geom_test::rng::SplitMix64;
use geom_test::seed::{Seed, resolve};
use series::{Truth, abs, fx, one};

/// Clothoids per run, and stations along each.
const CLOTHOIDS: usize = 400;
const STATIONS: i64 = 16;

/// One case: the clothoid's data and a station.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Case {
    x0: f64,
    y0: f64,
    k: f64,
    dk: f64,
    length: f64,
    s: f64,
}

/// m·2^e, exactly.
fn dyadic(m: i64, e: i32) -> f64 {
    let mut x = m as f64;
    for _ in 0..e.unsigned_abs() {
        x = if e < 0 { x * 0.5 } else { x * 2.0 };
    }
    x
}

/// The seeded cases: curvature up to 1/16 at the start and rates up to 2⁻¹⁰ per unit length, over
/// lengths from 1 to 512, so headings turn from a few degrees to many turns.
fn cases(seed: Seed) -> Vec<Case> {
    let mut rng = SplitMix64::new(seed);
    let mut out = Vec::new();
    for _ in 0..CLOTHOIDS {
        let mut draw = |lo, hi| rng.range_i64(lo, hi).unwrap();
        let (x0, y0) = (
            dyadic(draw(-1 << 20, 1 << 20), -8),
            dyadic(draw(-1 << 20, 1 << 20), -8),
        );
        let (k, dk) = (dyadic(draw(-256, 256), -12), dyadic(draw(-1024, 1024), -20));
        let length = dyadic(draw(4, 2048), -2);
        for q in 0..=STATIONS {
            out.push(Case {
                x0,
                y0,
                k,
                dk,
                length,
                s: length * dyadic(q, -4),
            });
        }
    }
    out
}

/// The cases as the harness reads them: θ₀ is 0, the kernel's direction (1, 0).
fn written(cases: &[Case]) -> String {
    let mut out = String::new();
    for c in cases {
        writeln!(
            out,
            "{} {} 0 {} {} {} {}",
            c.x0, c.y0, c.k, c.dk, c.length, c.s
        )
        .unwrap();
    }
    out
}

/// The kernel's transition for a case: a clothoid from κ to κ + κ′·L, which is exact.
fn transition(c: &Case) -> Transition<2> {
    let end = c.k + c.dk * c.length;
    assert_eq!(end - c.k, c.dk * c.length, "the end curvature is exact");
    Transition::new(
        [c.x0, c.y0],
        [1.0, 0.0],
        c.length,
        CurvatureLaw::Clothoid { start: c.k, end },
        &mut WorkMeter::new(Budget::UNLIMITED),
    )
    .unwrap()
}

/// The fixed-point truth of a case's point.
fn truth(c: &Case) -> [IBig; 2] {
    let p = vec![IBig::ZERO, fx(c.k), fx(c.dk) >> 1];
    let t = Truth {
        parts: vec![(IBig::ZERO, p)],
        wave: None,
        start: [fx(c.x0), fx(c.y0)],
        unit: [one(), IBig::ZERO],
    };
    t.placed(t.local(&fx(c.s)))
}

/// What the comparison found.
#[derive(Debug, Default, PartialEq)]
struct Outcome {
    /// Cases within the allowance.
    agree: usize,
    /// Cases beyond it whose truth the kernel encloses: Clothoids' differences.
    theirs: Vec<String>,
    /// Cases beyond it whose truth the kernel misses: the kernel's bugs.
    ours: Vec<String>,
}

fn compare(cases: &[Case], answers: &str) -> Outcome {
    let lines: Vec<&str> = answers.lines().collect();
    assert_eq!(lines.len(), cases.len(), "one answer per case");
    let mut out = Outcome::default();
    let mut built: Option<(Case, Transition<2>)> = None;
    for (c, line) in cases.iter().zip(lines) {
        let theirs: Vec<f64> = line
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        assert_eq!(theirs.len(), 2, "{line}");
        let same = built.as_ref().is_some_and(|(b, _)| {
            (b.x0, b.y0, b.k, b.dk, b.length) == (c.x0, c.y0, c.k, c.dk, c.length)
        });
        if !same {
            built = Some((*c, transition(c)));
        }
        let t = &built.as_ref().unwrap().1;
        let e = t
            .enclose(
                c.s,
                Limit::FromRight,
                0,
                &mut WorkMeter::new(Budget::UNLIMITED),
            )
            .unwrap()[0];
        let allowance = 1e-12 * (1.0 + c.length);
        let beyond = e
            .iter()
            .zip(&theirs)
            .any(|(i, &v)| v < i.lo() - allowance || v > i.hi() + allowance);
        if !beyond {
            out.agree += 1;
            continue;
        }
        let truth = truth(c);
        let slack = one() >> 200;
        let held = e
            .iter()
            .zip(&truth)
            .all(|(i, v)| fx(i.lo()) - &slack <= *v && *v <= fx(i.hi()) + &slack);
        let off: Vec<f64> = theirs
            .iter()
            .zip(&truth)
            .map(|(&v, w)| abs(&(fx(v) - w)).to_f64().value() / one().to_f64().value())
            .collect();
        let report = format!("{c:?}: Clothoids {theirs:?}, off the truth by {off:?}; kernel {e:?}");
        if held {
            out.theirs.push(report);
        } else {
            out.ours.push(report);
        }
    }
    out
}

fn path(var: &str) -> String {
    std::env::var(var).unwrap_or_else(|_| panic!("set {var} to the file's path"))
}

#[test]
#[ignore = "the nightly workflow's first step: writes the cases for the Clothoids harness"]
fn write_the_cases() {
    let seed = resolve("clothoids oracle", Seed::DEFAULT).unwrap();
    std::fs::write(path("CLOTHOIDS_CASES"), written(&cases(seed))).unwrap();
}

#[test]
#[ignore = "the nightly workflow's last step: compares the harness's answers with the kernel"]
fn compare_with_the_answers() {
    let seed = resolve("clothoids oracle", Seed::DEFAULT).unwrap();
    let cases = cases(seed);
    let written_now = written(&cases);
    let read = std::fs::read_to_string(path("CLOTHOIDS_CASES")).unwrap();
    assert_eq!(read, written_now, "the cases file is this seed's");
    let answers = std::fs::read_to_string(path("CLOTHOIDS_ANSWERS")).unwrap();
    let out = compare(&cases, &answers);
    println!(
        "{} of {} points agree; {} differ where the kernel holds the truth (Clothoids'), {} where it does not (the kernel's)",
        out.agree,
        cases.len(),
        out.theirs.len(),
        out.ours.len()
    );
    for r in &out.theirs {
        println!("Clothoids' difference: {r}");
    }
    assert!(
        out.ours.is_empty(),
        "the kernel missed the truth:\n{}",
        out.ours.join("\n")
    );
}

#[test]
fn the_comparison_runs_on_the_kernels_own_points() {
    let all = cases(Seed::DEFAULT);
    let cases: Vec<Case> = all.into_iter().take(3 * (STATIONS as usize + 1)).collect();
    let text = written(&cases);
    assert_eq!(text.lines().count(), cases.len());
    // Every value reads back to its double.
    for (line, c) in text.lines().zip(&cases) {
        let v: Vec<f64> = line
            .split_whitespace()
            .map(|x| x.parse().unwrap())
            .collect();
        assert_eq!(v, [c.x0, c.y0, 0.0, c.k, c.dk, c.length, c.s]);
    }
    let mut answers = String::new();
    for c in &cases {
        let p = transition(c).point(c.s, Limit::FromRight).unwrap();
        writeln!(answers, "{} {}", p[0], p[1]).unwrap();
    }
    let out = compare(&cases, &answers);
    assert_eq!(out.agree, cases.len());
    // An answer moved by a millimetre is a difference, and the truth says whose.
    let mut moved: Vec<String> = answers.lines().map(str::to_owned).collect();
    let p = transition(&cases[5])
        .point(cases[5].s, Limit::FromRight)
        .unwrap();
    moved[5] = format!("{} {}", p[0] + 1e-3, p[1]);
    let out = compare(&cases, &(moved.join("\n") + "\n"));
    assert_eq!(out.agree, cases.len() - 1);
    assert_eq!(out.theirs.len(), 1, "{out:?}");
    assert_eq!(out.ours, Vec::<String>::new());
}
