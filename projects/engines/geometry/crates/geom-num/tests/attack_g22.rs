//! Attacks on piece G22: the coordinate grid of `geom_num::grid` and the tolerance ladder of
//! `geom_num::ladder` (chapters 6.1 and 6.5 of the kernel architecture). Every test tries to break a
//! claim of the piece's done-when or of the two modules' guarantees.
//!
//! What decides what is right shares no code with the modules:
//!
//! - **Exact rationals** (`num-rational`), for coarsening: the nearest whole number to c / 25^k,
//!   rounded by the library's own rule (half away from zero, never reached, because 25^k is odd and
//!   the test checks no case is a tie), against the module's Euclidean division.
//! - **The definitions of chapter 6.1 in millimetres**, for the units and the ladder: q0 = 1/6400
//!   mm and each step 25 times finer than the one above, 1 inch = 25.4 mm, 1 mil = 1/1000 inch,
//!   1 µm = 1/1000 mm, as rationals, never the modules' quarter-nanometre constants.
//!
//! The claims attacked:
//!
//! - **The range** (done-when: coordinates refused at magnitude 2^40 and beyond): every
//!   constructor accepts ±(2^40 − 1) and refuses ±2^40 with the value refused, and `P2` and `P3`
//!   report the first coordinate out of range, in the order x, y, z.
//! - **Refining then coarsening is identity** (done-when): for random coordinates at three scales
//!   and every pair of steps, refining multiplies by 25 per rung exactly, refuses with the exact
//!   product when it leaves the frame, and coarsening the result gives the coordinate back, exact.
//! - **Coarsening is reported** (done-when): it gives the nearest step, says it is exact exactly
//!   when the division is, never ties, moves a coordinate by at most half a coarse step, and never
//!   leaves the frame; refining it again gives the coordinate back exactly when it was exact.
//! - **Step counts refine exactly**: multiplied by 25 per rung, `None` on overflow or towards a
//!   coarser step, never saturated.
//! - **The unit ratios** (done-when: mm, inch, micron, mil): each unit is a whole number of steps
//!   exactly where the definitions make it one, and the quarter-nanometre constants are the units.
//! - **The ladder** (done-when): every rung re-derived from chapter 6.5's definitions; the module
//!   header gives every constant's derivation; no rung an owner has not accepted (such as
//!   `h_min` = 8q) is in the module; and commit rounding goes to the nearest step per coordinate,
//!   ties to even, saying it rounded.
//!
//! Seeds: each property test prints `seed: <hex>` and runs from a fixed default through
//! `geom_test::prop`; `SPAREKIT_SEED=<hex>` replays another (chapter 28.17).

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

use std::cell::Cell;
use std::collections::BTreeSet;

use geom_num::NumError;
use geom_num::grid::{
    COORD_BITS, COORD_MAX, Coord, GridStep, P2, P3, QUARTER_NM_PER_INCH, QUARTER_NM_PER_MICRON,
    QUARTER_NM_PER_MIL, QUARTER_NM_PER_MM, QUARTER_NM_PER_NM, Steps, coarsen, refine,
};
use geom_num::ladder::{
    ANGULAR, ANGULAR_LOG2, COMMIT_DISTANCE_2D, COMMIT_DISTANCE_3D, COMMIT_PER_AXIS, FIT, HEAL_CAP,
    HEAL_DEFAULT, HalfSteps, SHORT_EDGE_WARNING,
};
use geom_num::plane::{Plane, Tpi};
use geom_test::prop;
use geom_test::seed::Seed;
use num_rational::Ratio;
use proptest::prelude::*;

/// The largest coordinate magnitude, M = 2^40 − 1.
const M: i64 = COORD_MAX;

/// Every step, coarsest first.
const STEPS: [GridStep; 3] = [GridStep::Q0, GridStep::Q1, GridStep::Q2];

/// Cases per property test.
const CASES: u32 = 4_096;

/// 25 to the power `k`.
fn pow25(k: u32) -> i64 {
    25_i64.pow(k)
}

/// How many rungs `fine` is below `coarse`, or `None` if it is coarser.
fn rungs(coarse: GridStep, fine: GridStep) -> Option<u32> {
    let index = |s| STEPS.iter().position(|&t| t == s).unwrap() as u32;
    index(fine).checked_sub(index(coarse))
}

/// The coordinate at three scales: as drawn, and divided by 25 and by 625, so that refining by one
/// and by two rungs stays in the frame for a good share of the cases.
fn scales(c: Coord) -> [i64; 3] {
    let v = c.get();
    [v, v / 25, v / 625]
}

/// Runs a property through `geom_test::prop`, failing the test with its replay line.
fn check<S>(
    label: &str,
    seed: u64,
    strategy: &S,
    test: impl Fn(S::Value) -> Result<(), TestCaseError>,
) where
    S: Strategy,
    S::Value: core::fmt::Debug,
{
    if let Err(failure) = prop::run(label, Seed(seed), CASES, strategy, test) {
        panic!("{failure}");
    }
}

// ---------------------------------------------------------------------------------------------
// The range.
// ---------------------------------------------------------------------------------------------

/// Every constructor accepts exactly −(2^40 − 1)..=2^40 − 1 and refuses the rest with the value.
#[test]
fn attack_the_range_is_exactly_forty_bits() {
    assert_eq!(M, (1_i64 << 40) - 1);
    assert_eq!(COORD_BITS, 40);
    for v in [-M, -M + 1, -1, 0, 1, M - 1, M] {
        assert_eq!(Coord::new(v).map(Coord::get), Ok(v));
        assert_eq!(Coord::from_i128(i128::from(v)).map(Coord::get), Ok(v));
    }
    for v in [
        M + 1,
        -(M + 1),
        M + 2,
        -(M + 2),
        i64::MAX,
        i64::MIN,
        i64::MIN + 1,
    ] {
        let refused = Err(NumError::OutOfRange {
            value: i128::from(v),
        });
        assert_eq!(Coord::new(v), refused, "{v}");
        assert_eq!(Coord::from_i128(i128::from(v)), refused, "{v}");
    }
    for v in [i128::MAX, i128::MIN, 1 << 40, -(1 << 40), 1 << 64] {
        assert_eq!(Coord::from_i128(v), Err(NumError::OutOfRange { value: v }));
    }
    assert_eq!((Coord::MAX.get(), Coord::MIN.get()), (M, -M));
}

/// Points refuse the first coordinate out of range, x before y before z, at ±2^40.
#[test]
fn attack_points_name_the_first_coordinate_out_of_range() {
    fn out<T>(v: i64) -> Result<T, NumError> {
        Err(NumError::OutOfRange {
            value: i128::from(v),
        })
    }
    for edge in [M + 1, -(M + 1)] {
        assert_eq!(P2::new(edge, 0), out(edge));
        assert_eq!(P2::new(0, edge), out(edge));
        assert_eq!(P2::new(edge, -edge), out(edge));
        assert_eq!(P3::new(edge, 0, 0), out(edge));
        assert_eq!(P3::new(0, edge, 0), out(edge));
        assert_eq!(P3::new(0, 0, edge), out(edge));
        assert_eq!(P3::new(0, edge, edge + edge.signum()), out(edge));
    }
    for (x, y, z) in [(M, -M, M), (-M, M, -M)] {
        let p = P3::new(x, y, z).unwrap();
        assert_eq!((p.x.get(), p.y.get(), p.z.get()), (x, y, z));
        let q = P2::new(x, y).unwrap();
        assert_eq!((q.x.get(), q.y.get()), (x, y));
    }
}

// ---------------------------------------------------------------------------------------------
// Refining and coarsening.
// ---------------------------------------------------------------------------------------------

/// Refining multiplies by 25 per rung exactly, refuses with the exact product outside the frame,
/// and coarsening the result back gives the coordinate, exact.
#[test]
fn attack_refining_then_coarsening_is_identity() {
    let (fitted, refused) = (Cell::new(0_u32), Cell::new(0_u32));
    check(
        "refining then coarsening",
        0x6722_0000_5eed_0245,
        &prop::coord(),
        |c| {
            for v in scales(c) {
                let c = Coord::new(v).unwrap();
                for from in STEPS {
                    for to in STEPS {
                        let Some(k) = rungs(from, to) else {
                            prop_assert_eq!(refine(c, from, to), Err(NumError::WrongDirection));
                            continue;
                        };
                        let product = i128::from(v) * i128::from(pow25(k));
                        match refine(c, from, to) {
                            Ok(fine) => {
                                prop_assert_eq!(i128::from(fine.get()), product);
                                let back = coarsen(fine, to, from).unwrap();
                                prop_assert_eq!((back.value(), back.is_exact()), (c, true));
                                fitted.set(fitted.get() + 1);
                            }
                            Err(e) => {
                                prop_assert_eq!(e, NumError::OutOfRange { value: product });
                                prop_assert!(product.unsigned_abs() > M as u128);
                                refused.set(refused.get() + 1);
                            }
                        }
                    }
                }
            }
            Ok(())
        },
    );
    // Both outcomes were reached many times, so neither half of the claim went untested.
    assert!(
        fitted.get() > CASES && refused.get() > CASES / 4,
        "{fitted:?} {refused:?}"
    );
}

/// Coarsening gives the nearest step by exact rationals, never a tie, exact exactly when the
/// division is, within half a coarse step, in the frame; refining back restores exact values only.
#[test]
fn attack_coarsening_is_nearest_and_reported() {
    let inexact = Cell::new(0_u32);
    check("coarsening", 0x6722_0001_5eed_0245, &prop::coord(), |c| {
        for v in scales(c) {
            let c = Coord::new(v).unwrap();
            for from in STEPS {
                for to in STEPS {
                    let Some(k) = rungs(to, from) else {
                        prop_assert_eq!(coarsen(c, from, to), Err(NumError::WrongDirection));
                        continue;
                    };
                    let d = pow25(k);
                    // 25^k is odd, so twice the distance to the nearest multiple is never d.
                    prop_assert_ne!(2 * (v.rem_euclid(d)), d);
                    let nearest = Ratio::new(i128::from(v), i128::from(d))
                        .round()
                        .to_integer();
                    let r = coarsen(c, from, to).unwrap();
                    prop_assert_eq!(i128::from(r.value().get()), nearest);
                    prop_assert_eq!(r.is_exact(), v % d == 0);
                    let moved = (r.value().get() * d - v).abs();
                    prop_assert!(2 * moved < d, "moved {} of {}", moved, d);
                    prop_assert!(r.value().get().abs() <= v.abs());
                    // The frame's edge, 2^40 − 1, is a multiple of 25 and 275 past a multiple of
                    // 625, below half of 625, so a coarsened value always refines back in range.
                    let again = refine(r.value(), to, from).unwrap();
                    prop_assert_eq!(again == c, r.is_exact());
                    if !r.is_exact() {
                        inexact.set(inexact.get() + 1);
                    }
                }
            }
        }
        Ok(())
    });
    assert!(inexact.get() > CASES, "{inexact:?}");
}

/// The extremes and the values one step from them, every pair of steps, deterministically.
#[test]
fn attack_refine_and_coarsen_at_the_edge_of_the_frame() {
    assert_eq!(M % 25, 0);
    assert_eq!(M % 625, 275);
    for v in [
        M,
        M - 1,
        -M,
        -(M - 1),
        M / 25,
        M / 25 + 1,
        M / 625,
        M / 625 + 1,
    ] {
        for sign in [1, -1] {
            let v = v * sign;
            let c = Coord::new(v).unwrap();
            for (from, to) in [
                (GridStep::Q0, GridStep::Q1),
                (GridStep::Q1, GridStep::Q2),
                (GridStep::Q0, GridStep::Q2),
            ] {
                let product = i128::from(v) * i128::from(pow25(rungs(from, to).unwrap()));
                let fits = product.unsigned_abs() <= M as u128;
                match refine(c, from, to) {
                    Ok(f) => assert!(fits && i128::from(f.get()) == product, "{v} {from:?}"),
                    Err(e) => assert!(!fits && e == NumError::OutOfRange { value: product }),
                }
                let r = coarsen(c, to, from).unwrap();
                let d = i128::from(pow25(rungs(from, to).unwrap()));
                let nearest = Ratio::new(i128::from(v), d).round().to_integer();
                assert_eq!(
                    i128::from(r.value().get()),
                    nearest,
                    "{v} {to:?} to {from:?}"
                );
            }
        }
    }
    // The largest coordinates that refine by one and by two rungs, and the first that do not.
    assert!(refine(Coord::new(M / 25).unwrap(), GridStep::Q0, GridStep::Q1).is_ok());
    assert!(refine(Coord::new(M / 25 + 1).unwrap(), GridStep::Q0, GridStep::Q1).is_err());
    assert!(refine(Coord::new(M / 625).unwrap(), GridStep::Q0, GridStep::Q2).is_ok());
    assert!(refine(Coord::new(M / 625 + 1).unwrap(), GridStep::Q0, GridStep::Q2).is_err());
}

/// Step counts refine by 25 per rung exactly, and are never saturated.
#[test]
fn attack_step_counts_refine_exactly() {
    check("step counts", 0x6722_0002_5eed_0245, &any::<u64>(), |n| {
        for n in [n, n >> 10, n >> 40] {
            for from in STEPS {
                for to in STEPS {
                    let want = rungs(from, to).and_then(|k| n.checked_mul(pow25(k) as u64));
                    prop_assert_eq!(Steps(n).refine(from, to), want.map(Steps));
                }
            }
        }
        Ok(())
    });
    assert_eq!(
        Steps(u64::MAX / 625).refine(GridStep::Q0, GridStep::Q2),
        Some(Steps(u64::MAX / 625 * 625))
    );
    assert_eq!(
        Steps(u64::MAX / 625 + 1).refine(GridStep::Q0, GridStep::Q2),
        None
    );
}

// ---------------------------------------------------------------------------------------------
// The units.
// ---------------------------------------------------------------------------------------------

/// A length in millimetres, exactly.
type Mm = Ratio<i128>;

/// The steps by chapter 6.1's definitions: q0 = 1/6400 mm, each step 25 times finer.
fn step_mm(step: GridStep) -> Mm {
    let q0 = Mm::new(1, 6_400);
    q0 / i128::from(pow25(rungs(GridStep::Q0, step).unwrap()))
}

/// Each named unit is a whole number of steps exactly where the definitions make it one, and the
/// quarter-nanometre constants are the units.
#[test]
fn attack_the_unit_ratios_are_the_definitions() {
    let inch = Mm::new(254, 10);
    let units = [
        ("mm", QUARTER_NM_PER_MM, Mm::from_integer(1)),
        ("inch", QUARTER_NM_PER_INCH, inch),
        ("micron", QUARTER_NM_PER_MICRON, Mm::new(1, 1_000)),
        ("mil", QUARTER_NM_PER_MIL, inch / 1_000),
        ("nm", QUARTER_NM_PER_NM, Mm::new(1, 1_000_000)),
    ];
    let quarter_nm = Mm::new(1, 4_000_000);
    for (name, constant, length) in units {
        assert_eq!(
            Mm::from_integer(i128::from(constant)) * quarter_nm,
            length,
            "{name}"
        );
        for step in STEPS {
            let ratio = length / step_mm(step);
            let want = ratio
                .is_integer()
                .then(|| u64::try_from(ratio.to_integer()).unwrap());
            assert_eq!(step.steps_per(constant), want, "{name} on {step:?}");
            assert_eq!(
                Mm::from_integer(i128::from(step.quarter_nm())) * quarter_nm,
                step_mm(step)
            );
        }
    }
    // Chapter 6.1's headline numbers.
    assert_eq!(GridStep::Q0.steps_per(QUARTER_NM_PER_MM), Some(6_400));
    assert_eq!(GridStep::Q0.steps_per(QUARTER_NM_PER_INCH), Some(162_560));
    assert_eq!(GridStep::Q1.steps_per(QUARTER_NM_PER_MICRON), Some(160));
    assert_eq!(GridStep::Q1.steps_per(QUARTER_NM_PER_MIL), Some(4_064));
    assert_eq!(GridStep::Q0.steps_per(QUARTER_NM_PER_MICRON), None);
}

// ---------------------------------------------------------------------------------------------
// The ladder.
// ---------------------------------------------------------------------------------------------

/// A count of steps of `step` in millimetres.
fn length(steps: Steps, step: GridStep) -> Mm {
    step_mm(step) * i128::from(steps.get())
}

/// Every rung, re-derived from chapter 6.5's definitions.
#[test]
fn attack_every_rung_is_its_derivation() {
    assert_eq!(length(FIT, GridStep::Q0), Mm::new(1, 100)); // 0.01 mm
    assert_eq!(length(FIT, GridStep::Q1), Mm::new(4, 10_000)); // 0.4 µm
    assert_eq!(length(HEAL_DEFAULT, GridStep::Q0), Mm::new(1, 10)); // 0.1 mm
    assert_eq!(length(HEAL_CAP, GridStep::Q0), Mm::from_integer(1)); // 1 mm
    assert_eq!(length(HEAL_DEFAULT, GridStep::Q1), Mm::new(4, 1_000)); // 4 µm
    assert_eq!(length(HEAL_CAP, GridStep::Q1), Mm::new(4, 100)); // 40 µm
    assert_eq!(length(SHORT_EDGE_WARNING, GridStep::Q0), Mm::new(1, 2)); // 0.5 mm
    assert_eq!(length(SHORT_EDGE_WARNING, GridStep::Q1), Mm::new(2, 100)); // 20 µm
    // The angular bound is the fit bound over the frame's range, 64q / 2^40 q = 2^-34 rad.
    assert_eq!(
        Ratio::new(i128::from(FIT.get()), 1 << COORD_BITS),
        Ratio::new(1, 1 << 34)
    );
    assert_eq!(ANGULAR_LOG2, -34);
    assert_eq!(ANGULAR.get(), 1.0 / (1_u64 << 34) as f64);
    // Commit rounding moves a coordinate by at most half a step; as a distance in n dimensions
    // that is √n / 2 steps, whose square n / 4 is at most the ceiling's square for n = 2 and 3.
    assert_eq!(COMMIT_PER_AXIS, HalfSteps(1));
    for (n, ceiling) in [(2, COMMIT_DISTANCE_2D), (3, COMMIT_DISTANCE_3D)] {
        let half = Ratio::new(i128::from(COMMIT_PER_AXIS.0), 2);
        assert!(half * half * n <= Ratio::from_integer(i128::from(ceiling.get()).pow(2)));
        assert!(
            half * half * n < Ratio::from_integer(1),
            "√{n}/2 is below one step"
        );
    }
}

/// The module holds exactly the accepted rungs, and its header gives each constant's derivation.
/// A proposed rung such as `h_min` = 8q joins only when an owner accepts it (6.5), and then this
/// list with it.
#[test]
fn attack_the_ladder_holds_only_accepted_rungs_each_derived() {
    let source = include_str!("../src/ladder.rs");
    let header: String = source
        .lines()
        .take_while(|l| l.starts_with("//!"))
        .collect::<Vec<_>>()
        .join("\n");
    let constants: BTreeSet<&str> = source
        .lines()
        .filter_map(|l| l.strip_prefix("pub const "))
        .filter(|l| !l.starts_with("fn "))
        .map(|l| l.split(':').next().unwrap().trim())
        .collect();
    let accepted = BTreeSet::from([
        "ANGULAR",
        "ANGULAR_LOG2",
        "COMMIT_DISTANCE_2D",
        "COMMIT_DISTANCE_3D",
        "COMMIT_PER_AXIS",
        "FIT",
        "HEAL_CAP",
        "HEAL_DEFAULT",
        "SHORT_EDGE_WARNING",
    ]);
    assert_eq!(constants, accepted);
    let derivations = header
        .split("Derivations")
        .nth(1)
        .expect("a derivations list");
    for name in accepted {
        assert!(
            derivations.contains(&format!("[`{name}`]")),
            "{name} has no derivation"
        );
    }
    assert!(
        header.contains("ties to even"),
        "commit rounding's tie rule is in the header"
    );
}

/// Commit rounding goes to the nearest step per coordinate, ties to even, and says it rounded: the
/// three planes 2x + z = 1, y = 0 and z = 0, moved along x, meet at x = k + 1/2 exactly.
#[test]
fn attack_commit_rounding_ties_to_even() {
    let p = |x, y, z| P3::new(x, y, z).unwrap();
    for (shift, rounded) in [
        (0, 0),
        (1, 2),
        (2, 2),
        (3, 4),
        (-1, 0),
        (-2, -2),
        (-3, -2),
        (-4, -4),
    ] {
        let slanted = Plane::through(p(shift, 0, 1), p(shift + 1, 0, -1), p(shift, 1, 1)).unwrap();
        let y0 = Plane::through(p(0, 0, 0), p(1, 0, 0), p(0, 0, 1)).unwrap();
        let z0 = Plane::through(p(0, 0, 0), p(1, 0, 0), p(0, 1, 0)).unwrap();
        // The point is (shift + 1/2, 0, 0).
        let point = Tpi::new(slanted, y0, z0).unwrap().to_grid().unwrap();
        assert!(!point.is_exact(), "shift {shift}");
        assert_eq!(point.value(), p(rounded, 0, 0), "{shift} + 1/2");
        // The same point with its planes in another order rounds the same way.
        let again = Tpi::new(z0, slanted, y0).unwrap().to_grid().unwrap();
        assert_eq!(again, point);
    }
}

/// `ladder::quarter_nm` gives a length in steps exactly in quarter-nanometres, for every rung and
/// step, and for the largest count of steps (the arguments pass through `black_box`, so the call
/// runs when the test does, never folded into a constant).
#[test]
fn a_length_in_steps_is_exact_in_quarter_nanometres() {
    use std::hint::black_box;
    for step in [GridStep::Q0, GridStep::Q1, GridStep::Q2] {
        for steps in [
            FIT,
            HEAL_DEFAULT,
            HEAL_CAP,
            SHORT_EDGE_WARNING,
            Steps(0),
            Steps(u64::MAX),
        ] {
            let expected = u128::from(steps.0) * u128::from(step.quarter_nm());
            assert_eq!(
                geom_num::ladder::quarter_nm(black_box(steps), black_box(step)),
                Some(expected)
            );
        }
    }
}
