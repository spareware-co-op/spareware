//! The law catalogue's NURBS row (chapter 28.3; piece G69): `geom-nurbs`'s curves and surfaces
//! against the exact rationals of `geom_test::nurbs_reference`, on dense samples.
//!
//! - **Evaluation within its bound of exact rational evaluation.** At every knot, either side of
//!   it, and at evenly spaced and random parameters in every span, a curve's or surface's point
//!   lies within the bound `geom-nurbs` states of the exact point, coordinate by coordinate:
//!   `γ_(7p+1)`·Σ `N_j`·|`P_j`| for a nonrational curve, `γ_(14p+3)`·Σ `N_j`·|`w_j·P_j`| / Σ
//!   `N_j·w_j` for a rational one, and the surfaces' `γ_(7p+7q+2)` and `γ_(14p+14q+5)` forms, with
//!   `γ_n` = n·u/(1 − n·u), u = 2^−53, each sum exact. A clamped curve's ends are its end control
//!   points exactly (for a rational curve, w·P/w rounded once).
//! - **Knot insertion and degree elevation within their stated bound of the exact result.** A
//!   curve's insertion, refinement and elevation give the reference's knot vector exactly, and the
//!   new curve, evaluated exactly, lies within the stated deviation of the old one, evaluated
//!   exactly, everywhere sampled; a surface's, in each direction, likewise.
//! - **Knot removal reports its true error.** After a removal, the curve, evaluated exactly, lies
//!   within the reported deviation of the original everywhere sampled; the deviation is at most
//!   the tolerance asked for; and when nothing was removed the curve is unchanged, deviation 0.
//!
//! Each law lists every broken case it finds. Seeds: each law prints `seed: <hex>` and replays
//! with `SPAREKIT_SEED`; pull requests run the fixed default seed, and the nightly workflow a fresh
//! one (`kernel-nightly.yml`).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::needless_range_loop,
    clippy::disallowed_methods,
    reason = "tests: oracles and generators written as the formulas read, and a failure stops the test"
)]

use core::num::NonZeroU64;

use dashu_int::UBig;
use dashu_ratio::RBig;
use geom_nurbs::{Basis, Curve, Direction, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_test::nurbs_reference::{Patch, Spline};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// All the work in release builds, an eighth in debug builds (and under WebAssembly).
fn scale(n: usize) -> usize {
    if cfg!(debug_assertions) {
        (n / 8).max(1)
    } else {
        n
    }
}

fn q(x: f64) -> RBig {
    RBig::try_from(x).unwrap()
}

/// `γ_n` = n·u/(1 − n·u), u = 2^−53, exactly.
fn gamma(n: usize) -> RBig {
    let nu = RBig::from_parts(n.into(), UBig::ONE << 53);
    &nu / (RBig::ONE - &nu)
}

fn abs(x: &RBig) -> RBig {
    if *x < RBig::ZERO {
        -x.clone()
    } else {
        x.clone()
    }
}

/// |a − b|² ≤ r², exactly.
fn within(a: &[RBig], b: &[RBig], r: f64) -> bool {
    let s: RBig = a
        .iter()
        .zip(b)
        .fold(RBig::ZERO, |s, (x, y)| s + (x - y) * (x - y));
    s <= q(r) * q(r)
}

/// |a − b| as a double, for messages.
fn gap(a: &[RBig], b: &[RBig]) -> f64 {
    let s: RBig = a
        .iter()
        .zip(b)
        .fold(RBig::ZERO, |s, (x, y)| s + (x - y) * (x - y));
    s.to_f64().value().sqrt()
}

/// What a law found: the first failure of each case, and how many there were.
#[derive(Default)]
struct Findings {
    first: Vec<(String, String, usize)>,
}

impl Findings {
    fn add(&mut self, case: String, detail: impl FnOnce() -> String) {
        if let Some(f) = self.first.iter_mut().find(|f| f.0 == case) {
            f.2 += 1;
        } else {
            self.first.push((case, detail(), 1));
        }
    }

    fn assert_none(&self, label: &str) {
        let lines: Vec<String> = self
            .first
            .iter()
            .map(|(c, d, n)| format!("- {c} ({n} times), first: {d}"))
            .collect();
        assert!(
            self.first.is_empty(),
            "{label}: {} broken:\n{}",
            self.first.len(),
            lines.join("\n")
        );
    }
}

struct Rng(SplitMix64);

impl Rng {
    fn new(label: &str) -> Rng {
        Rng(SplitMix64::new(resolve(label, Seed::DEFAULT).unwrap()))
    }
    fn unit(&mut self) -> f64 {
        (self.0.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }
    fn below(&mut self, n: u64) -> u64 {
        self.0.below(NonZeroU64::new(n).unwrap())
    }
    /// 2^e·(1 + f), e a whole number from −10 to 9 and f in [0, 1): weights from about 10⁻³ to
    /// 10³, exactly doubles on every target.
    fn weight(&mut self) -> f64 {
        let e = self.below(20) as i32 - 10;
        let p = if e < 0 {
            1.0 / f64::from(1u32 << -e)
        } else {
            f64::from(1u32 << e)
        };
        p * (1.0 + self.unit())
    }
}

/// A knot vector of degree p: clamped for three in four, else uniform; interior knots possibly
/// repeated (at most p times).
fn knots(rng: &mut Rng, p: usize) -> Basis {
    let clamped = rng.below(4) != 0;
    let mut k = Vec::new();
    if clamped {
        k.extend(std::iter::repeat_n(0.0, p + 1));
        let mut x = 0.0;
        for _ in 0..rng.below(5) {
            x += 0.125 + rng.unit();
            for _ in 0..=rng.below(p as u64) {
                k.push(x);
            }
        }
        x += 0.25 + rng.unit();
        k.extend(std::iter::repeat_n(x, p + 1));
    } else {
        let n = 2 * p + 2 + rng.below(4) as usize;
        let mut x = 0.0;
        for _ in 0..n {
            k.push(x);
            x += 0.25 + rng.unit();
        }
    }
    Basis::new(p, k).unwrap()
}

fn curve<const D: usize>(rng: &mut Rng) -> Curve<D> {
    let p = 1 + rng.below(5) as usize;
    let b = knots(rng, p);
    let pts: Vec<[f64; D]> = (0..b.count())
        .map(|_| core::array::from_fn(|_| rng.range(-10.0, 10.0)))
        .collect();
    if rng.below(3) == 0 {
        Curve::new(b, &pts).unwrap()
    } else {
        let w: Vec<f64> = (0..pts.len()).map(|_| rng.weight()).collect();
        Curve::new_rational(b, &pts, &w).unwrap()
    }
}

fn surface(rng: &mut Rng) -> Surface<3> {
    let (p, qd) = (1 + rng.below(3) as usize, 1 + rng.below(3) as usize);
    let (bu, bv) = (knots(rng, p), knots(rng, qd));
    let n = bu.count() * bv.count();
    let pts: Vec<[f64; 3]> = (0..n)
        .map(|_| core::array::from_fn(|_| rng.range(-10.0, 10.0)))
        .collect();
    if rng.below(3) == 0 {
        Surface::new(bu, bv, &pts).unwrap()
    } else {
        let w: Vec<f64> = (0..n).map(|_| rng.weight()).collect();
        Surface::new_rational(bu, bv, &pts, &w).unwrap()
    }
}

fn spline_of<const D: usize>(c: &Curve<D>) -> Spline {
    let b = c.basis();
    let pts: Vec<&[f64]> = c.homogeneous().iter().map(|p| &p[..]).collect();
    match c.weights() {
        Some(w) => Spline::new_rational(b.degree(), b.knots(), &pts, w).unwrap(),
        None => Spline::new(b.degree(), b.knots(), &pts).unwrap(),
    }
}

fn patch_of(s: &Surface<3>, magnitudes: bool) -> Patch {
    let (bu, bv) = (s.basis_u(), s.basis_v());
    let net: Vec<[f64; 3]> = s
        .homogeneous()
        .iter()
        .map(|p| if magnitudes { p.map(f64::abs) } else { *p })
        .collect();
    let pts: Vec<&[f64]> = net.iter().map(|p| &p[..]).collect();
    let (deg, kn) = ((bu.degree(), bv.degree()), (bu.knots(), bv.knots()));
    match s.weights() {
        Some(w) => Patch::new_rational(deg, kn, &pts, w).unwrap(),
        None => Patch::new(deg, kn, &pts).unwrap(),
    }
}

/// The domain of a basis: [`u_p`, `u_(n+1)`].
fn domain(b: &Basis) -> (f64, f64) {
    let k = b.knots();
    (k[b.degree()], k[b.count()])
}

/// Parameters across a domain: every knot in it, either side of each, and n − 1 evenly spaced
/// and n random ones per span.
fn samples(rng: &mut Rng, b: &Basis, n: usize) -> Vec<f64> {
    let (a, z) = domain(b);
    let mut knots: Vec<f64> = b
        .knots()
        .iter()
        .copied()
        .filter(|&k| a <= k && k <= z)
        .collect();
    knots.dedup();
    let mut ts = Vec::new();
    for pair in knots.windows(2) {
        let (lo, hi) = (pair[0], pair[1]);
        ts.extend([lo, lo.next_up(), hi.next_down(), hi]);
        for i in 1..n {
            ts.push(lo + (hi - lo) * (i as f64 / n as f64));
        }
        for _ in 0..n {
            ts.push(rng.range(lo, hi));
        }
    }
    ts.retain(|&t| a <= t && t <= z);
    ts
}

fn curve_points<const D: usize>(label: &str) {
    let rng = &mut Rng::new(label);
    let mut found = Findings::default();
    let mut checked = 0usize;
    for case in 0..scale(160) {
        let c = curve::<D>(rng);
        let (s, p) = (spline_of(&c), c.degree());
        // Σ N_j·|h_j| per coordinate, and Σ N_j·w_j last for a rational curve.
        let mag = s.magnitudes();
        let rational = c.weights().is_some();
        let g = gamma(if rational { 14 * p + 3 } else { 7 * p + 1 });
        for t in samples(rng, c.basis(), scale(16)) {
            let got = c.point(t, Limit::FromRight).unwrap();
            let exact = s.point(&q(t)).unwrap();
            let m = mag.homogeneous_point(&q(t)).unwrap();
            for d in 0..D {
                let bound = if rational {
                    &g * &m[d] / &m[D]
                } else {
                    &g * &m[d]
                };
                if abs(&(q(got[d]) - &exact[d])) > bound {
                    found.add(
                        format!("degree {p}, rational {rational}: beyond its bound"),
                        || format!("case {case} at {t}, coordinate {d}: {c:?}"),
                    );
                }
            }
            checked += 1;
        }
        let (a, z) = domain(c.basis());
        if s.is_clamped() {
            let n = c.homogeneous().len() - 1;
            for (t, i) in [(a, 0), (z, n)] {
                let want: [f64; D] = match c.weights() {
                    Some(w) => c.homogeneous()[i].map(|x| x / w[i]),
                    None => c.homogeneous()[i],
                };
                if c.point(t, Limit::FromRight).unwrap() != want {
                    found.add("a clamped end is not its control point".into(), || {
                        format!("case {case} at {t}")
                    });
                }
            }
        }
    }
    println!("{label}: {checked} parameters checked");
    found.assert_none(label);
}

#[test]
fn plane_curve_points_are_within_their_bound() {
    curve_points::<2>("G69 plane NURBS curve points against the exact reference");
}

#[test]
fn space_curve_points_are_within_their_bound() {
    curve_points::<3>("G69 space NURBS curve points against the exact reference");
}

#[test]
fn surface_points_are_within_their_bound() {
    let label = "G69 NURBS surface points against the exact reference";
    let rng = &mut Rng::new(label);
    let mut found = Findings::default();
    let mut checked = 0usize;
    for case in 0..scale(48) {
        let s = surface(rng);
        let (exact_s, mag) = (patch_of(&s, false), patch_of(&s, true));
        let (p, qd) = (s.basis_u().degree(), s.basis_v().degree());
        let rational = s.weights().is_some();
        let g = gamma(if rational {
            14 * p + 14 * qd + 5
        } else {
            7 * p + 7 * qd + 2
        });
        let (us, vs) = (
            samples(rng, s.basis_u(), scale(4)),
            samples(rng, s.basis_v(), scale(4)),
        );
        for &u in &us {
            for &v in &vs {
                let got = s.point(u, v, (Limit::FromRight, Limit::FromRight)).unwrap();
                let exact = exact_s.point(&q(u), &q(v)).unwrap();
                let m = mag.homogeneous_point(&q(u), &q(v)).unwrap();
                for d in 0..3 {
                    let bound = if rational {
                        &g * &m[d] / &m[3]
                    } else {
                        &g * &m[d]
                    };
                    if abs(&(q(got[d]) - &exact[d])) > bound {
                        found.add(
                            format!("degrees ({p}, {qd}), rational {rational}: beyond its bound"),
                            || format!("case {case} at ({u}, {v}), coordinate {d}"),
                        );
                    }
                }
                checked += 1;
            }
        }
    }
    println!("{label}: {checked} parameter pairs checked");
    found.assert_none(label);
}

/// The new curve, exactly, within `deviation` of the old one, exactly, at dense parameters of the
/// old one's domain.
fn curve_kept<const D: usize>(
    found: &mut Findings,
    case: &str,
    rng: &mut Rng,
    old: &Curve<D>,
    new: &Curve<D>,
    deviation: f64,
) -> usize {
    let (a, b) = (spline_of(old), spline_of(new));
    let mut checked = 0;
    for t in samples(rng, old.basis(), scale(8)) {
        let (x, y) = (a.point(&q(t)).unwrap(), b.point(&q(t)).unwrap());
        if !within(&x, &y, deviation) {
            found.add(format!("{case}: beyond its deviation"), || {
                format!("at {t}: {} away, stated {deviation}", gap(&x, &y))
            });
        }
        checked += 1;
    }
    checked
}

fn same_knots(found: &mut Findings, case: &str, got: &[f64], want: &[RBig]) {
    let got: Vec<RBig> = got.iter().map(|&k| q(k)).collect();
    if got != want {
        found.add(
            format!("{case}: knots differ from the reference's"),
            String::new,
        );
    }
}

fn curve_operations<const D: usize>(label: &str) {
    let rng = &mut Rng::new(label);
    let m = &mut meter();
    let mut found = Findings::default();
    let mut checked = 0usize;
    for _ in 0..scale(120) {
        let c = curve::<D>(rng);
        let s = spline_of(&c);
        let p = c.degree();
        let (a, z) = domain(c.basis());
        // Insertion of a new knot, or of an existing one up to its room.
        let u = if rng.below(3) == 0 {
            let inner: Vec<f64> = c
                .basis()
                .knots()
                .iter()
                .copied()
                .filter(|&k| a < k && k < z)
                .collect();
            if inner.is_empty() {
                rng.range(a, z)
            } else {
                inner[rng.below(inner.len() as u64) as usize]
            }
        } else {
            rng.range(a, z)
        };
        let room = p - c.basis().knots().iter().filter(|&&k| k == u).count().min(p);
        if room > 0 && a < u && u < z {
            let times = 1 + rng.below(room as u64) as usize;
            match c.insert_knot(u, times, m) {
                Ok(r) => {
                    same_knots(
                        &mut found,
                        "insertion",
                        r.value.basis().knots(),
                        s.insert(&q(u), times).unwrap().knots(),
                    );
                    checked += curve_kept(&mut found, "insertion", rng, &c, &r.value, r.deviation);
                }
                Err(e) => found.add(format!("insertion refused: {e}"), String::new),
            }
        }
        // Refinement by a few new knots.
        let mut x: Vec<f64> = (0..=rng.below(4)).map(|_| rng.range(a, z)).collect();
        x.sort_by(f64::total_cmp);
        x.retain(|&k| a < k && k < z);
        if !x.is_empty() {
            match c.refine(&x, m) {
                Ok(r) => {
                    let xs: Vec<RBig> = x.iter().map(|&k| q(k)).collect();
                    match s.refine(&xs) {
                        Ok(reference) => same_knots(
                            &mut found,
                            "refinement",
                            r.value.basis().knots(),
                            reference.knots(),
                        ),
                        Err(e) => found.add(
                            format!("refinement: the reference refused: {e:?}"),
                            String::new,
                        ),
                    }
                    checked += curve_kept(&mut found, "refinement", rng, &c, &r.value, r.deviation);
                }
                Err(e) => found.add(format!("refinement refused: {e}"), String::new),
            }
        }
        // Elevation, where the curve allows it (clamped, no interior knot of multiplicity p + 1).
        if s.is_clamped() {
            let t = 1 + rng.below(2) as usize;
            if let Ok(r) = c.elevate(t, m) {
                same_knots(
                    &mut found,
                    "elevation",
                    r.value.basis().knots(),
                    s.elevate(t).unwrap().knots(),
                );
                checked += curve_kept(&mut found, "elevation", rng, &c, &r.value, r.deviation);
            }
        }
    }
    println!("{label}: {checked} parameters checked");
    found.assert_none(label);
    assert!(checked > 100);
}

#[test]
fn plane_curve_insertion_refinement_and_elevation_keep_the_curve() {
    curve_operations::<2>("G69 plane NURBS curve operations against the exact reference");
}

#[test]
fn space_curve_insertion_refinement_and_elevation_keep_the_curve() {
    curve_operations::<3>("G69 space NURBS curve operations against the exact reference");
}

fn removal<const D: usize>(label: &str) {
    let rng = &mut Rng::new(label);
    let m = &mut meter();
    let mut found = Findings::default();
    let mut checked = 0usize;
    for case in 0..scale(120) {
        let c = curve::<D>(rng);
        let p = c.degree();
        let (a, z) = domain(c.basis());
        // Half the time a knot inserted first (removable up to rounding), else an existing one.
        let (start, u) = if rng.below(2) == 0 {
            let u = rng.range(a, z);
            let times = 1 + rng.below(p as u64) as usize;
            match c.insert_knot(u, times, m) {
                Ok(r) => (r.value, u),
                Err(_) => continue,
            }
        } else {
            let inner: Vec<f64> = c
                .basis()
                .knots()
                .iter()
                .copied()
                .filter(|&k| a < k && k < z)
                .collect();
            if inner.is_empty() {
                continue;
            }
            (c.clone(), inner[rng.below(inner.len() as u64) as usize])
        };
        let times = 1 + rng.below(p as u64) as usize;
        let tol = [1e-12, 1e-6, 1e-2, 1.0, 100.0][rng.below(5) as usize];
        match start.remove_knot(u, times, tol, m) {
            Ok((r, removed)) => {
                if r.deviation > tol {
                    found.add("the deviation exceeds the tolerance".into(), || {
                        format!("case {case}: {} against {tol}", r.deviation)
                    });
                }
                if removed == 0 && (r.deviation != 0.0 || r.value != start) {
                    found.add("nothing removed, yet changed".into(), || {
                        format!("case {case}")
                    });
                }
                checked += curve_kept(&mut found, "removal", rng, &start, &r.value, r.deviation);
            }
            Err(e) => found.add(format!("removal refused: {e}"), || {
                format!("case {case} at {u}")
            }),
        }
    }
    println!("{label}: {checked} parameters checked");
    found.assert_none(label);
    assert!(checked > 100);
}

#[test]
fn plane_curve_knot_removal_reports_its_true_error() {
    removal::<2>("G69 plane NURBS knot removal against the exact reference");
}

#[test]
fn space_curve_knot_removal_reports_its_true_error() {
    removal::<3>("G69 space NURBS knot removal against the exact reference");
}

#[test]
fn surface_insertion_refinement_and_elevation_keep_the_surface() {
    let label = "G69 NURBS surface operations against the exact reference";
    let rng = &mut Rng::new(label);
    let m = &mut meter();
    let mut found = Findings::default();
    let mut checked = 0usize;
    for case in 0..scale(48) {
        let s = surface(rng);
        let old = patch_of(&s, false);
        for dir in [Direction::U, Direction::V] {
            let b = match dir {
                Direction::U => s.basis_u(),
                Direction::V => s.basis_v(),
            };
            let (a, z) = domain(b);
            let u = rng.range(a, z);
            let x = [rng.range(a, z)];
            let mut results = vec![
                ("insertion", s.insert_knot(dir, u, 1, m)),
                ("refinement", s.refine(dir, &x, m)),
            ];
            if b.knots()[0] == b.knots()[b.degree()]
                && b.knots()[b.count()] == *b.knots().last().unwrap()
            {
                results.push(("elevation", s.elevate(dir, 1, m)));
            }
            for (what, r) in results {
                let r = match r {
                    Ok(r) => r,
                    Err(e) => {
                        found.add(format!("{what} in {dir:?} refused: {e}"), || {
                            format!("case {case}")
                        });
                        continue;
                    }
                };
                let new = patch_of(&r.value, false);
                let (us, vs) = (
                    samples(rng, s.basis_u(), scale(3)),
                    samples(rng, s.basis_v(), scale(3)),
                );
                for &u in &us {
                    for &v in &vs {
                        let (x, y) = (
                            old.point(&q(u), &q(v)).unwrap(),
                            new.point(&q(u), &q(v)).unwrap(),
                        );
                        if !within(&x, &y, r.deviation) {
                            found.add(format!("{what} in {dir:?}: beyond its deviation"), || {
                                format!(
                                    "case {case} at ({u}, {v}): {} away, stated {}",
                                    gap(&x, &y),
                                    r.deviation
                                )
                            });
                        }
                        checked += 1;
                    }
                }
            }
        }
    }
    println!("{label}: {checked} parameter pairs checked");
    found.assert_none(label);
    assert!(checked > 100);
}
