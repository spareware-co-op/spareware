//! `geom-nurbs` compared with the exact rational reference of `geom_test::nurbs_reference` (piece
//! G57; chapter 28.9: "on small cases, every pull request compares the kernel with them: exactly for
//! Exact results, and within the stated bound for Certified ones").
//!
//! What is compared, on seeded small curves of degree 1 to 4 in two and three dimensions, rational
//! and not, clamped and not, with knots, coordinates and weights that are short binary fractions:
//!
//! - **Points.** `Curve::point` against the exact point, coordinate by coordinate, within the bound
//!   `geom-nurbs` states: `γ_(7p+1)` · Σ `N_j`·|`P_j`| for a nonrational curve,
//!   `γ_(14p+3)` · (Σ `N_j`·|`w_j`·`P_j`|) / (Σ `N_j`·`w_j`) for a rational one; and exactly the
//!   end control points (the quotient w·P / w, rounded once, for a rational curve) at the ends of a
//!   clamped curve.
//! - **Knot insertion, refinement and degree elevation.** The new knot vector and degree exactly;
//!   then the certified deviation each returns: no control point of a nonrational result is
//!   farther than it from the exact one, and for every result, rational or not, the curve the
//!   result describes (evaluated exactly) lies within it of the exact result at every sampled
//!   parameter.
//!
//! Every case is drawn from a printed seed (`SPAREKIT_SEED` replays one), and the cases cover each
//! operation's refusals only through the reference agreeing that the input is out of range.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::too_many_lines,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use core::num::NonZeroU64;

use dashu_int::{IBig, UBig};
use dashu_ratio::RBig;
use geom_nurbs::{Basis, Curve, Limit};
use geom_run::{Budget, WorkMeter};
use geom_test::nurbs_reference::Spline;
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

/// How many curves each dimension draws.
const CASES: usize = 120;

fn r(x: f64) -> RBig {
    RBig::try_from(x).unwrap()
}

/// `γ_n` = n·u / (1 − n·u), u = 2^−53, exactly.
fn gamma(n: usize) -> RBig {
    let nu = RBig::from_parts(IBig::from(n), UBig::ONE << 53);
    &nu / (RBig::ONE - &nu)
}

fn magnitude(x: &RBig) -> RBig {
    if *x < RBig::ZERO {
        -x.clone()
    } else {
        x.clone()
    }
}

fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    rng.below(NonZeroU64::new(n).unwrap())
}

/// A small curve: degree, knots, homogeneous points (w·P) and weights if rational.
#[derive(Clone, Debug)]
struct Case<const D: usize> {
    degree: usize,
    knots: Vec<f64>,
    homs: Vec<[f64; D]>,
    weights: Option<Vec<f64>>,
}

impl<const D: usize> Case<D> {
    fn draw(rng: &mut SplitMix64) -> Case<D> {
        let p = 1 + below(rng, 4) as usize;
        let count = p + 1 + below(rng, 5) as usize;
        let clamped = below(rng, 4) != 0;
        // Interior knots repeated at most p times, or in one draw in eight p + 1 times, where the
        // curve may jump (elevation refuses that, and evaluation takes the limit from the right).
        let most = if below(rng, 8) == 0 { p + 1 } else { p };
        let knots = loop {
            let knots: Vec<f64> = if clamped {
                // p + 1 zeros, count − p − 1 interior knots in sixteenths, p + 1 ones.
                let mut interior: Vec<f64> = (0..count - p - 1)
                    .map(|_| (1 + below(rng, 15)) as f64 / 16.0)
                    .collect();
                interior.sort_by(f64::total_cmp);
                let mut k = vec![0.0; p + 1];
                k.extend(interior);
                k.extend(vec![1.0; p + 1]);
                k
            } else {
                // count + p + 1 knots in quarters on [0, 8].
                let mut all: Vec<f64> = (0..=count + p)
                    .map(|_| below(rng, 33) as f64 / 4.0)
                    .collect();
                all.sort_by(f64::total_cmp);
                all
            };
            let ends = if clamped { p + 1 } else { 0 };
            let inner = &knots[ends..knots.len() - ends];
            let runs_ok = inner
                .iter()
                .all(|k| knots.iter().filter(|&x| x == k).count() <= most);
            if runs_ok && knots[p] < knots[count] {
                break knots;
            }
        };
        let rational = below(rng, 2) == 0;
        let weights: Option<Vec<f64>> = rational.then(|| {
            (0..count)
                .map(|_| [0.5, 0.75, 1.0, 1.5, 2.0, 4.0][below(rng, 6) as usize])
                .collect()
        });
        let homs = (0..count)
            .map(|i| {
                let w = weights.as_ref().map_or(1.0, |w| w[i]);
                core::array::from_fn(|_| (below(rng, 33) as f64 - 16.0) / 8.0 * w)
            })
            .collect();
        Case {
            degree: p,
            knots,
            homs,
            weights,
        }
    }

    fn kernel(&self) -> Curve<D> {
        let basis = Basis::new(self.degree, self.knots.clone()).unwrap();
        Curve::from_homogeneous(basis, self.homs.clone(), self.weights.clone()).unwrap()
    }

    fn reference(&self) -> Spline {
        reference_of(
            self.degree,
            &self.knots,
            &self.homs,
            self.weights.as_deref(),
        )
    }
}

fn reference_of<const D: usize>(
    degree: usize,
    knots: &[f64],
    homs: &[[f64; D]],
    weights: Option<&[f64]>,
) -> Spline {
    let points: Vec<&[f64]> = homs.iter().map(<[f64; D]>::as_slice).collect();
    match weights {
        Some(w) => Spline::new_rational(degree, knots, &points, w).unwrap(),
        None => Spline::new(degree, knots, &points).unwrap(),
    }
}

/// The reference curve a kernel curve describes, exactly.
fn reference_of_kernel<const D: usize>(c: &Curve<D>) -> Spline {
    reference_of(c.degree(), c.basis().knots(), c.homogeneous(), c.weights())
}

/// Parameters to sample: every distinct knot of the domain, and the quarters of every span.
fn samples(s: &Spline) -> Vec<RBig> {
    let (lo, hi) = s.domain().unwrap();
    let mut out = Vec::new();
    for pair in s.knots().windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if a < b && *a >= lo && *b <= hi {
            for q in 0..4u8 {
                out.push(a + (b - a) * RBig::from_parts(IBig::from(q), UBig::from(4u8)));
            }
        }
    }
    out.push(hi);
    out
}

fn check_points<const D: usize>(case: &Case<D>, what: &str) {
    let curve = case.kernel();
    let spline = case.reference();
    let mags = spline.magnitudes();
    let p = case.degree;
    let rational = case.weights.is_some();
    let g = if rational {
        gamma(14 * p + 3)
    } else {
        gamma(7 * p + 1)
    };
    for u in samples(&spline) {
        let got = curve.point(u.to_f64().value(), Limit::FromRight).unwrap();
        let want = spline.point(&u).unwrap();
        let m = mags.homogeneous_point(&u).unwrap();
        let w = if rational {
            spline.homogeneous_point(&u).unwrap()[D].clone()
        } else {
            RBig::ONE
        };
        for c in 0..D {
            let bound = &g * &m[c] / &w;
            let err = magnitude(&(r(got[c]) - &want[c]));
            assert!(
                err <= bound,
                "{what}: point at {u} coordinate {c}: {} off, bound {}",
                err.to_f64().value(),
                bound.to_f64().value()
            );
        }
    }
    if case.knots[..=p].iter().all(|&k| k == case.knots[0])
        && case.knots[case.knots.len() - p - 1..]
            .iter()
            .all(|&k| k == case.knots[case.knots.len() - 1])
    {
        let n = case.homs.len() - 1;
        for (end, u, i) in [
            ("start", case.knots[0], 0),
            ("end", case.knots[case.knots.len() - 1], n),
        ] {
            let got = curve.point(u, Limit::FromRight).unwrap();
            for (c, &g) in got.iter().enumerate() {
                let want = case
                    .weights
                    .as_ref()
                    .map_or(case.homs[i][c], |w| case.homs[i][c] / w[i]);
                assert_eq!(g, want, "{what}: the {end} of a clamped curve");
            }
        }
    }
}

/// The certified deviation of a changed curve, checked against the exact result.
fn check_changed<const D: usize>(
    got: &Curve<D>,
    deviation: f64,
    want: &Spline,
    rational: bool,
    what: &str,
) {
    if let Some(problem) = changed_problem(got, deviation, want, rational) {
        panic!("{what}: {problem}");
    }
}

/// The first way a changed curve misses the exact result by more than its certified deviation, or
/// differs from it in degree or knots.
fn changed_problem<const D: usize>(
    got: &Curve<D>,
    deviation: f64,
    want: &Spline,
    rational: bool,
) -> Option<String> {
    if got.degree() != want.degree() {
        return Some(format!("degree {} for {}", got.degree(), want.degree()));
    }
    let knots: Vec<RBig> = got.basis().knots().iter().map(|&k| r(k)).collect();
    if knots != want.knots() {
        return Some(format!("knots {:?}", got.basis().knots()));
    }
    let dev = r(deviation);
    let dev2 = &dev * &dev;
    if !rational {
        for (i, (q, exact)) in got
            .homogeneous()
            .iter()
            .zip(want.homogeneous_points())
            .enumerate()
        {
            let d2 = (0..D).fold(RBig::ZERO, |acc, c| {
                let e = r(q[c]) - &exact[c];
                acc + &e * &e
            });
            if d2 > dev2 {
                return Some(format!(
                    "control point {i} is {} away, deviation {deviation}",
                    d2.to_f64().value().sqrt()
                ));
            }
        }
    }
    let described = reference_of_kernel(got);
    for u in samples(want) {
        let a = described.point(&u).unwrap();
        let b = want.point(&u).unwrap();
        let d2 = (0..D).fold(RBig::ZERO, |acc, c| {
            let e = &a[c] - &b[c];
            acc + &e * &e
        });
        if d2 > dev2 {
            return Some(format!(
                "at {u} the result is {} away, deviation {deviation}",
                d2.to_f64().value().sqrt()
            ));
        }
    }
    None
}

/// A knot strictly inside the domain that can take at least one more copy, in sixty-fourths.
fn insertable(case_knots: &[f64], p: usize, n: usize, rng: &mut SplitMix64) -> Option<f64> {
    let (lo, hi) = (case_knots[p], case_knots[n + 1]);
    for _ in 0..16 {
        let u = lo + (hi - lo) * (1 + below(rng, 63)) as f64 / 64.0;
        let m = case_knots.iter().filter(|&&k| k == u).count();
        if u > lo && u < hi && m < p {
            return Some(u);
        }
    }
    None
}

fn check_operations<const D: usize>(case: &Case<D>, rng: &mut SplitMix64, what: &str) {
    let curve = case.kernel();
    let spline = case.reference();
    let p = case.degree;
    let n = case.homs.len() - 1;
    let rational = case.weights.is_some();
    let mut meter = WorkMeter::new(Budget::UNLIMITED);

    if let Some(u) = insertable(&case.knots, p, n, rng) {
        let m = case.knots.iter().filter(|&&k| k == u).count();
        let times = 1 + below(rng, (p - m) as u64) as usize;
        let got = curve.insert_knot(u, times, &mut meter).unwrap();
        let want = spline.insert(&r(u), times).unwrap();
        check_changed(
            &got.value,
            got.deviation,
            &want,
            rational,
            &format!("{what}: insert {u} x{times}"),
        );
    }

    let mut xs = Vec::new();
    for _ in 0..=below(rng, 4) {
        let mut trial = case.knots.clone();
        trial.extend(&xs);
        trial.sort_by(f64::total_cmp);
        if let Some(u) = insertable(&trial, p, n, rng) {
            xs.push(u);
        }
    }
    xs.sort_by(f64::total_cmp);
    if !xs.is_empty() {
        let got = curve.refine(&xs, &mut meter).unwrap();
        let exact: Vec<RBig> = xs.iter().map(|&x| r(x)).collect();
        let want = spline.refine(&exact).unwrap();
        check_changed(
            &got.value,
            got.deviation,
            &want,
            rational,
            &format!("{what}: refine {xs:?}"),
        );
    }

    let t = 1 + below(rng, 2) as usize;
    match spline.elevate(t) {
        Ok(want) => {
            let got = curve.elevate(t, &mut meter).unwrap();
            check_changed(
                &got.value,
                got.deviation,
                &want,
                rational,
                &format!("{what}: elevate by {t}"),
            );
        }
        Err(_) => {
            // Unclamped, or a jump at an interior knot: the kernel refuses it too.
            assert!(
                curve.elevate(t, &mut meter).is_err(),
                "{what}: elevate by {t}"
            );
        }
    }
}

fn run<const D: usize>(label: &str) {
    let seed = resolve(label, Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    for i in 0..CASES {
        let case = Case::<D>::draw(&mut rng);
        let what = format!("{label}, case {i}: {case:?}");
        check_points(&case, &what);
        check_operations(&case, &mut rng, &what);
    }
}

#[test]
fn plane_curves_agree_with_the_exact_reference() {
    run::<2>("geom-nurbs against the NURBS reference, 2D");
}

#[test]
fn space_curves_agree_with_the_exact_reference() {
    run::<3>("geom-nurbs against the NURBS reference, 3D");
}

#[test]
fn a_known_curve_agrees_exactly() {
    // A quadratic rational conic arc from (1, 0) to (0, 1) about the corner (1, 1), its middle
    // weight 1/2 (every value a short binary fraction). Inserting 1/2 halves its homogeneous
    // points, which is exact in doubles, so the kernel's points equal the reference's exactly.
    // Elevation's coefficients (1/3, 2/3) round, so it is held to its deviation.
    let case = Case::<2> {
        degree: 2,
        knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        homs: vec![[1.0, 0.0], [0.5, 0.5], [0.0, 1.0]],
        weights: Some(vec![1.0, 0.5, 1.0]),
    };
    let curve = case.kernel();
    let spline = case.reference();
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let inserted = curve.insert_knot(0.5, 1, &mut meter).unwrap();
    let want = spline.insert(&r(0.5), 1).unwrap();
    check_changed(
        &inserted.value,
        inserted.deviation,
        &want,
        true,
        "arc insert",
    );
    let got: Vec<Vec<RBig>> = inserted
        .value
        .homogeneous()
        .iter()
        .zip(inserted.value.weights().unwrap())
        .map(|(h, &w)| vec![r(h[0]), r(h[1]), r(w)])
        .collect();
    assert_eq!(got, want.homogeneous_points());
    let elevated = curve.elevate(1, &mut meter).unwrap();
    check_changed(
        &elevated.value,
        elevated.deviation,
        &spline.elevate(1).unwrap(),
        true,
        "arc elevate",
    );
}

#[test]
fn derivatives_are_the_blossom_with_unit_directions() {
    // The cubic Bezier curve on (0, 0), (1, 3), (4, 2), (5, 0) at t = 1/4, by hand from its
    // hodographs: P′ = 3·Σ B²ᵢ·ΔPᵢ = (21/4, 57/16), P″ = 6·Σ B¹ᵢ·Δ²Pᵢ = (6, −39/2),
    // P‴ = 6·Δ³P = (−24, 18), and nothing past the degree.
    let s = Spline::new(
        3,
        &[0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        &[&[0.0, 0.0], &[1.0, 3.0], &[4.0, 2.0], &[5.0, 0.0]],
    )
    .unwrap();
    let t = r(0.25);
    let q = |a: i64, b: u64| RBig::from_parts(IBig::from(a), UBig::from(b));
    assert_eq!(
        s.homogeneous_derivative(&t, 0).unwrap(),
        s.point(&t).unwrap()
    );
    assert_eq!(
        s.homogeneous_derivative(&t, 1).unwrap(),
        vec![q(21, 4), q(57, 16)]
    );
    assert_eq!(
        s.homogeneous_derivative(&t, 2).unwrap(),
        vec![q(6, 1), q(-39, 2)]
    );
    assert_eq!(
        s.homogeneous_derivative(&t, 3).unwrap(),
        vec![q(-24, 1), q(18, 1)]
    );
    assert_eq!(
        s.homogeneous_derivative(&t, 4).unwrap(),
        vec![RBig::ZERO, RBig::ZERO]
    );
    assert!(s.homogeneous_derivative(&r(2.0), 1).is_err());
}

#[test]
fn the_comparison_catches_a_moved_point() {
    // The comparison has teeth: a result with one control point moved by 2^-20, or a weight
    // changed by as much, is outside its certified deviation, which is only rounding.
    for rational in [false, true] {
        let case = Case::<2> {
            degree: 3,
            knots: vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
            homs: vec![[0.0, 0.0], [1.0, 2.0], [2.0, -1.0], [3.0, 1.0], [4.0, 0.0]],
            weights: rational.then(|| vec![1.0, 2.0, 0.5, 1.0, 1.0]),
        };
        let curve = case.kernel();
        let spline = case.reference();
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        let got = curve.insert_knot(0.25, 1, &mut meter).unwrap();
        let want = spline.insert(&r(0.25), 1).unwrap();
        assert_eq!(
            changed_problem(&got.value, got.deviation, &want, rational),
            None
        );
        let mut homs = got.value.homogeneous().to_vec();
        homs[2][1] += 1.0 / 1_048_576.0;
        let basis = got.value.basis().clone();
        let weights = got.value.weights().map(<[f64]>::to_vec);
        let moved = Curve::from_homogeneous(basis.clone(), homs, weights.clone()).unwrap();
        assert!(changed_problem(&moved, got.deviation, &want, rational).is_some());
        if let Some(mut w) = weights {
            w[2] += 1.0 / 1_048_576.0;
            let reweighted =
                Curve::from_homogeneous(basis, got.value.homogeneous().to_vec(), Some(w)).unwrap();
            assert!(changed_problem(&reweighted, got.deviation, &want, true).is_some());
        }
    }
}
