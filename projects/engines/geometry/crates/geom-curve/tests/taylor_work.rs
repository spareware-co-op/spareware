//! What the meter charges a Taylor conversion (issue #837): the counts are the ones the module
//! states, so each piece's change to Bernstein form is paid for before it is made.
//!
//! - A piece's change of degree p in D coordinates costs `TERM_WORK` for each coordinate and for
//!   the factor, in each of its (p + 1)·(p + 2) terms; a count past `u64` is a defect, not a wrap.
//! - A conversion costs exactly its kind's own enclosures and derivative bounds, its n·p + 1
//!   stored points and each of its n pieces' change: for a polynomial whose remainder is zero and
//!   one whose remainder is not, in 2D and in 3D, at degrees 1, 2, 5 and 9, and for a helix and an
//!   involute at their own degree.
//! - A budget that holds all of that but the changes, what a conversion was charged before issue
//!   #837, is a typed stop.
//!
//! Covers: `CurveKind::Polynomial`, `CurveKind::Helix`, `CurveKind::Involute`.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    reason = "tests: native arithmetic and indexing, the formulas' names, and a failure stops the test"
)]

use geom_curve::taylor::{self, TERM_WORK, change_work};
use geom_curve::{CurveError, Frame, Helix, HelixAxes, Involute, ParamCurve, Polynomial};
use geom_nurbs::Limit;
use geom_run::{Budget, BudgetKind, Stop, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

#[test]
fn a_change_costs_its_terms_for_each_coordinate_and_its_factor() {
    for p in 0..=12_u64 {
        for d in 1..=4_u64 {
            let want = (p + 1) * (p + 2) * (d + 1) * TERM_WORK;
            assert_eq!(
                change_work(usize::try_from(p).unwrap(), usize::try_from(d).unwrap()),
                Ok(want),
                "degree {p} dimension {d}"
            );
        }
    }
    assert_eq!(TERM_WORK, 1);
    assert_eq!(change_work(9, 3), Ok(440));
    assert_eq!(change_work(9, 2), Ok(330));
    assert_eq!(change_work(usize::MAX, 3), Err(CurveError::Defect));
    assert_eq!(change_work(usize::MAX - 1, 3), Err(CurveError::Defect));
    assert_eq!(change_work(3, usize::MAX), Err(CurveError::Defect));
    // Degree 2^31: on a 64-bit target its terms, about 2^62, fit and four times them do not; on a
    // 32-bit target the terms already overflow. Refused either way.
    let big = usize::try_from(1_u64 << 31).unwrap();
    assert_eq!(change_work(big, 3), Err(CurveError::Defect));
}

/// The kind's own charges in `taylor::from_cuts` at `degree` on `cuts`, replayed call by call on a
/// meter of its own: each cut's point, each piece's derivatives at its middle and its derivative
/// bound of order `degree` + 1.
fn kind_work<K: ParamCurve<D>, const D: usize>(kind: &K, degree: usize, cuts: &[f64]) -> u128 {
    let m = &mut meter();
    for &t in cuts {
        kind.enclose(t, Limit::FromRight, 0, m).unwrap();
    }
    for w in cuts.windows(2) {
        kind.enclose(f64::midpoint(w[0], w[1]), Limit::FromRight, degree, m)
            .unwrap();
        kind.derivative_bound((w[0], w[1]), degree + 1, m).unwrap();
    }
    m.used().work
}

/// Converts `kind` at `degree` on `per` pieces: what it was charged and what the module says it
/// costs, and the second without the changes.
fn charged<K: ParamCurve<D>, const D: usize>(
    kind: &K,
    degree: usize,
    per: usize,
) -> (u128, u128, u128) {
    let cuts = taylor::cuts_between(kind.domain(), &[], per).unwrap();
    let n = u128::try_from(cuts.len() - 1).unwrap();
    let m = &mut meter();
    taylor::from_cuts(kind, degree, &cuts, m).unwrap();
    let p = u128::try_from(degree).unwrap();
    let without = kind_work(kind, degree, &cuts) + n * p + 1;
    let change = u128::from(change_work(degree, D).unwrap());
    (m.used().work, without + n * change, without)
}

fn polynomial<const D: usize>(degree: usize) -> Polynomial<D> {
    let coefficients = (0..=degree)
        .map(|i| {
            let mut c = [0.0; D];
            for (j, x) in c.iter_mut().enumerate() {
                *x = 0.75 + 0.25 * f64::from(u8::try_from(i).unwrap())
                    - 0.125 * f64::from(u8::try_from(j).unwrap());
            }
            c
        })
        .collect();
    Polynomial::new(coefficients, (-1.5, 2.0)).unwrap()
}

fn helix() -> Helix<3> {
    Helix::new(
        [1.0, -2.0, 0.5],
        HelixAxes::Vectors {
            u: [2.0, 0.0, 0.0],
            v: [0.0, 2.0, 0.0],
            w: [0.0, 0.0, 0.75],
        },
        (-1.0, 2.0),
    )
    .unwrap()
}

fn involute() -> Involute<2> {
    let frame = Frame::new([1.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    Involute::new([0.5, -1.0], 1.5, frame, (0.25, 2.0)).unwrap()
}

#[test]
fn a_conversion_costs_its_kind_its_points_and_each_pieces_change() {
    for degree in [1, 2, 5, 9] {
        // A remainder that is not zero (the polynomial is one degree higher), and one that is.
        for q in [degree + 1, degree] {
            let (got, want, _) = charged(&polynomial::<2>(q), degree, 7);
            assert_eq!(got, want, "2D polynomial of degree {q} at degree {degree}");
            let (got, want, _) = charged(&polynomial::<3>(q), degree, 7);
            assert_eq!(got, want, "3D polynomial of degree {q} at degree {degree}");
        }
    }
    let (got, want, _) = charged(&helix(), 9, 24);
    assert_eq!(got, want, "helix");
    let (got, want, _) = charged(&involute(), 9, 14);
    assert_eq!(got, want, "involute");
}

#[test]
fn a_budget_without_the_changes_stops() {
    let h = helix();
    let (_, _, without) = charged(&h, 9, 24);
    let cuts = taylor::cuts_between(h.domain(), &[], 24).unwrap();
    let m = &mut WorkMeter::new(Budget::of_work(u64::try_from(without).unwrap()));
    match taylor::from_cuts(&h, 9, &cuts, m) {
        Err(CurveError::Stopped(s)) => assert_eq!(s.reason(), Stop::OverBudget(BudgetKind::Work)),
        other => panic!("not a budget stop: {:?}", other.map(|_| ())),
    }
    // The helix's own conversion, an eighth of a turn a piece, is charged the same way, with a
    // unit a piece for its cuts.
    let m = &mut meter();
    h.to_nurbs(m).unwrap();
    let (got, want, _) = charged(&h, 9, 24);
    assert_eq!(got, want);
    assert_eq!(m.used().work, want + 24);
}
