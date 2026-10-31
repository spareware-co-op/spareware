//! Attacks on piece G68's claims in `geom-surface`: first and second derivative bounds, the
//! interval quotient rule for rational surfaces, and bounds on the unit normal's partials.
//!
//! - **Never below the exact values, with weights from 10⁻³ to 10³.** Seeded NURBS patches of
//!   degree 1 to 3 in each direction, most rational with weights 2^e·(1 + f), e a whole
//!   number from −10 to 9 and f in [0, 1) (about 10⁻³ to 2·10³), with 10⁻³ and 10³ themselves, random boxes in their domains, and a dense grid of
//!   parameters in each box. At each parameter the exact `S_u`, `S_v`, `S_uu`, `S_uv` and `S_vv` come
//!   from `geom_test::nurbs_reference` in exact rationals (the homogeneous partials from blossoms,
//!   then the quotient rule exactly), and must lie in the first-derivative bounds and the
//!   second-derivative bound over the box; chapter 19.3's M must be at least
//!   max(|`S_uu`|, |`S_vv`|) + |`S_uv`| there; and the exact `N_u` and `N_v` must lie in the
//!   normal-derivative bound wherever it is bounded. |n| is irrational, so each comparison
//!   c·√s ≤ x is decided exactly by squaring with the signs kept.
//! - **The patch bounded by its control points.** Over a box, a rational patch's partials start
//!   from the hull of its Euclidean control points (chapter 19.3): S's enclosure among the partials
//!   lies inside the patch's box, however far the weights spread.
//! - **Bounded where it can be.** On boxes of 1/4096 of the domain, all but a few in a hundred of
//!   the patches' normal partials are bounded where the normal is not nearly zero: an interval
//!   bound that cannot exclude a zero normal over a wide box says `Unbounded`, and a caller
//!   subdivides.
//! - **Unbounded, never a guess.** Where the normal vanishes (a sphere's pole, a cone's apex) the
//!   normal's partials are `Unbounded`; a plane's second partials and normal partials are zero
//!   and its M is 0.
//! - **The contract catches a wrong bound.** A kind whose second-derivative or normal-derivative
//!   bound over a box misses the truth breaks the contract's derivative law.
//! - **Refusals.** A region outside the domain is refused.
//!
//! Every surface is drawn from a printed seed (`SPAREKIT_SEED` replays one).
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::disallowed_methods,
    clippy::needless_range_loop,
    reason = "tests: oracles and generators, and a failure stops the test"
)]

use core::num::NonZeroU64;

use dashu_ratio::RBig;
use geom_curve::{CurveError, DerivativeBound, Frame, Law};
use geom_math::{Axis, GridTransform, TypedTransform};
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Bounds, Changed, Direction, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::contract::check;
use geom_surface::{
    Cone, ParamBox, ParamSurface, Plane, SecondDerivativeBound, SecondPartials, Sphere,
    SurfaceForm, SurfaceKind,
};
use geom_test::nurbs_reference::Spline;
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

const SIDES: (Limit, Limit) = (Limit::FromRight, Limit::FromRight);

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// All the work in release builds, a tenth in debug builds.
fn scale(n: usize) -> usize {
    if cfg!(debug_assertions) {
        (n / 10).max(1)
    } else {
        n
    }
}

fn q(x: f64) -> RBig {
    RBig::try_from(x).unwrap()
}

fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    rng.below(NonZeroU64::new(n).unwrap())
}

fn unit(rng: &mut SplitMix64) -> f64 {
    (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64
}

/// 2^e, exactly.
fn pow2(e: i64) -> f64 {
    let mut x = 1.0;
    for _ in 0..e.unsigned_abs() {
        x *= 2.0;
    }
    if e < 0 { 1.0 / x } else { x }
}

/// A clamped knot vector of degree p on [0, 1 + …], with up to two interior knots.
fn knots(rng: &mut SplitMix64, p: usize) -> Vec<f64> {
    let mut k = vec![0.0; p + 1];
    let mut x = 0.0;
    for _ in 0..below(rng, 3) {
        x += 0.25 + unit(rng);
        k.push(x);
    }
    x += 0.5 + unit(rng);
    k.extend(std::iter::repeat_n(x, p + 1));
    k
}

/// A seeded NURBS patch: degrees 1 to 3, points in [−10, 10]³, and (for three in four) weights
/// 2^e·(1 + f), e a whole number from −10 to 9 and f in [0, 1), with 10⁻³ and 10³ themselves.
fn patch(rng: &mut SplitMix64) -> Surface<3> {
    let (p, r) = (1 + below(rng, 3) as usize, 1 + below(rng, 3) as usize);
    let bu = Basis::new(p, knots(rng, p)).unwrap();
    let bv = Basis::new(r, knots(rng, r)).unwrap();
    let n = bu.count() * bv.count();
    let pts: Vec<[f64; 3]> = (0..n)
        .map(|_| [0, 1, 2].map(|_| unit(rng) * 20.0 - 10.0))
        .collect();
    if below(rng, 4) == 0 {
        return Surface::new(bu, bv, &pts).unwrap();
    }
    let mut w: Vec<f64> = (0..n)
        .map(|_| pow2(below(rng, 20).cast_signed() - 10) * (1.0 + unit(rng)))
        .collect();
    let (i, j) = (below(rng, n as u64) as usize, below(rng, n as u64) as usize);
    w[i] = 1e-3;
    w[j] = 1e3;
    Surface::new_rational(bu, bv, &pts, &w).unwrap()
}

/// A random box of the domain: all of it, a small one, or two random ends in each direction.
fn region(rng: &mut SplitMix64, ((a, b), (c, d)): ParamBox) -> ParamBox {
    let mut side = |lo: f64, hi: f64| match below(rng, 4) {
        0 => (lo, hi),
        1 => {
            let t = lo + (hi - lo) * unit(rng);
            let w = (hi - lo) * 0.01;
            (t, (t + w).min(hi))
        }
        _ => {
            let (s, t) = (lo + (hi - lo) * unit(rng), lo + (hi - lo) * unit(rng));
            (s.min(t), s.max(t))
        }
    };
    (side(a, b), side(c, d))
}

/// The exact surface of a NURBS patch: one reference curve per row of the net (in v), and the u
/// basis functions as the coordinates of one curve whose control points are the unit vectors.
struct Exact {
    rows: Vec<Spline>,
    basis: Spline,
    rational: bool,
}

impl Exact {
    fn of(s: &Surface<3>) -> Exact {
        let (bu, bv) = (s.basis_u(), s.basis_v());
        let (nu, nv) = (bu.count(), bv.count());
        let h = s.homogeneous();
        let rows = (0..nu)
            .map(|i| {
                let pts: Vec<&[f64]> = (0..nv).map(|j| &h[i * nv + j][..]).collect();
                match s.weights() {
                    Some(w) => Spline::new_rational(
                        bv.degree(),
                        bv.knots(),
                        &pts,
                        &w[i * nv..(i + 1) * nv],
                    )
                    .unwrap(),
                    None => Spline::new(bv.degree(), bv.knots(), &pts).unwrap(),
                }
            })
            .collect();
        let eye: Vec<Vec<f64>> = (0..nu)
            .map(|i| (0..nu).map(|k| if k == i { 1.0 } else { 0.0 }).collect())
            .collect();
        let refs: Vec<&[f64]> = eye.iter().map(|e| &e[..]).collect();
        Exact {
            rows,
            basis: Spline::new(bu.degree(), bu.knots(), &refs).unwrap(),
            rational: s.weights().is_some(),
        }
    }

    /// The rows' homogeneous partials in v of orders 0 to 2 at v.
    fn rows_at(&self, v: &RBig) -> Vec<[Vec<RBig>; 3]> {
        self.rows
            .iter()
            .map(|r| [0, 1, 2].map(|b| r.homogeneous_derivative(v, b).unwrap()))
            .collect()
    }

    /// The exact S, `S_u`, `S_v`, `S_uu`, `S_uv` and `S_vv` at (u, v), from the rows at v.
    fn partials(&self, u: &RBig, rows: &[[Vec<RBig>; 3]]) -> [[RBig; 3]; 6] {
        let n: [Vec<RBig>; 3] = [0, 1, 2].map(|a| self.basis.homogeneous_derivative(u, a).unwrap());
        // h[a][b]: the homogeneous partial of order (a, b).
        let h = |a: usize, b: usize| -> Vec<RBig> {
            let dim = rows[0][0].len();
            let mut out = vec![RBig::ZERO; dim];
            for (ni, row) in n[a].iter().zip(rows) {
                for (o, x) in out.iter_mut().zip(&row[b]) {
                    *o += ni * x;
                }
            }
            out
        };
        let v3 = |x: &[RBig]| [x[0].clone(), x[1].clone(), x[2].clone()];
        if !self.rational {
            return [
                v3(&h(0, 0)),
                v3(&h(1, 0)),
                v3(&h(0, 1)),
                v3(&h(2, 0)),
                v3(&h(1, 1)),
                v3(&h(0, 2)),
            ];
        }
        let hs = [h(0, 0), h(1, 0), h(0, 1), h(2, 0), h(1, 1), h(0, 2)];
        let a = |k: usize| v3(&hs[k]);
        let w = |k: usize| hs[k][3].clone();
        let w0 = w(0);
        let f = |x: [RBig; 3], terms: &[(RBig, &[RBig; 3])]| -> [RBig; 3] {
            let mut x = x;
            for (c, y) in terms {
                for d in 0..3 {
                    x[d] = &x[d] - c * &y[d];
                }
            }
            x.map(|t| t / &w0)
        };
        let s = a(0).map(|t| t / &w0);
        let su = f(a(1), &[(w(1), &s)]);
        let sv = f(a(2), &[(w(2), &s)]);
        let two = RBig::from(2u8);
        let suu = f(a(3), &[(&two * w(1), &su), (w(3), &s)]);
        let suv = f(a(4), &[(w(1), &sv), (w(2), &su), (w(4), &s)]);
        let svv = f(a(5), &[(&two * w(2), &sv), (w(5), &s)]);
        [s, su, sv, suu, suv, svv]
    }
}

fn cross(a: &[RBig; 3], b: &[RBig; 3]) -> [RBig; 3] {
    [
        &a[1] * &b[2] - &a[2] * &b[1],
        &a[2] * &b[0] - &a[0] * &b[2],
        &a[0] * &b[1] - &a[1] * &b[0],
    ]
}

fn dot(a: &[RBig; 3], b: &[RBig; 3]) -> RBig {
    &a[0] * &b[0] + &a[1] * &b[1] + &a[2] * &b[2]
}

fn add3(a: &[RBig; 3], b: &[RBig; 3]) -> [RBig; 3] {
    [&a[0] + &b[0], &a[1] + &b[1], &a[2] + &b[2]]
}

/// Whether c·√s ≤ x, exactly (s ≥ 0).
fn le_sqrt(c: &RBig, s: &RBig, x: &RBig) -> bool {
    let zero = RBig::ZERO;
    match (*c <= zero, *x >= zero) {
        (true, true) => true,
        (false, false) => false,
        // Both sides negative: |c|·√s ≥ |x|.
        (true, false) => c * c * s >= x * x,
        // Both sides non-negative: c·√s ≤ x.
        (false, true) => c * c * s <= x * x,
    }
}

/// Whether m ≥ √x + √b, exactly (x, b ≥ 0).
fn ge_sum_sqrt(m: &RBig, x: &RBig, b: &RBig) -> bool {
    // m − √x ≥ √b ⇔ m² + x − b ≥ 2m√x with both sides non-negative.
    if *m < RBig::ZERO || m * m < *x {
        return false;
    }
    let l = m * m + x - b;
    l >= RBig::ZERO && &l * &l >= RBig::from(4u8) * m * m * x
}

fn holds(b: &Bounds<3>, x: &[RBig; 3]) -> bool {
    (0..3).all(|d| q(b.lo[d]) <= x[d] && x[d] <= q(b.hi[d]))
}

/// Whether the unit normal's partial (n′ s − n (n·n′)) / s^(3/2) lies in `b`, s = n·n > 0.
fn normal_holds(b: &Bounds<3>, n: &[RBig; 3], dn: &[RBig; 3], s: &RBig) -> bool {
    let ndn = dot(n, dn);
    (0..3).all(|d| {
        let x = &dn[d] * s - &n[d] * &ndn;
        le_sqrt(&(q(b.lo[d]) * s), s, &x) && le_sqrt(&(-q(b.hi[d]) * s), s, &(-x))
    })
}

/// The grid of g + 1 values from lo to hi, ends included.
fn grid(lo: f64, hi: f64, g: usize) -> Vec<f64> {
    (0..=g)
        .map(|i| (lo + (hi - lo) * (i as f64 / g as f64)).min(hi))
        .collect()
}

#[test]
fn rational_patches_with_weights_from_a_thousandth_to_a_thousand_are_never_below_their_bounds() {
    let seed = resolve(
        "G68 rational patches against the exact reference",
        Seed::DEFAULT,
    )
    .unwrap();
    let mut rng = SplitMix64::new(seed);
    let (mut checked, mut normals) = (0usize, 0usize);
    for case in 0..scale(40) {
        let s = patch(&mut rng);
        let exact = Exact::of(&s);
        let m = &mut meter();
        for _ in 0..3 {
            let r = region(&mut rng, ParamSurface::domain(&s));
            let what = format!("case {case} over {r:?}: {s:?}");
            let first =
                [Direction::U, Direction::V].map(|dir| {
                    match ParamSurface::derivative_bound(&s, r, dir, m).unwrap() {
                        DerivativeBound::Bounded(b) => b,
                        DerivativeBound::Unbounded => {
                            panic!("{what}: a first derivative unbounded")
                        }
                    }
                });
            let SecondDerivativeBound::Bounded(second) =
                s.second_derivative_bound(r, SIDES, m).unwrap()
            else {
                panic!("{what}: second partials unbounded");
            };
            let chord = q(second.chord().unwrap());
            let normal = s.normal_derivative_bound(r, SIDES, m).unwrap();
            let g = if cfg!(debug_assertions) { 3 } else { 6 };
            for v in grid(r.1.0, r.1.1, g) {
                let rows = exact.rows_at(&q(v));
                for u in grid(r.0.0, r.0.1, g) {
                    let [_, su, sv, suu, suv, svv] = exact.partials(&q(u), &rows);
                    let at = format!("{what} at ({u}, {v})");
                    assert!(
                        holds(&first[0], &su),
                        "{at}: S_u {su:?} outside {:?}",
                        first[0]
                    );
                    assert!(
                        holds(&first[1], &sv),
                        "{at}: S_v {sv:?} outside {:?}",
                        first[1]
                    );
                    assert!(
                        holds(&second.uu, &suu),
                        "{at}: S_uu outside {:?}",
                        second.uu
                    );
                    assert!(
                        holds(&second.uv, &suv),
                        "{at}: S_uv outside {:?}",
                        second.uv
                    );
                    assert!(
                        holds(&second.vv, &svv),
                        "{at}: S_vv outside {:?}",
                        second.vv
                    );
                    let b = dot(&suv, &suv);
                    for x in [dot(&suu, &suu), dot(&svv, &svv)] {
                        assert!(ge_sum_sqrt(&chord, &x, &b), "{at}: M {chord} too small");
                    }
                    let n = cross(&su, &sv);
                    let sq = dot(&n, &n);
                    if sq == RBig::ZERO {
                        continue;
                    }
                    let nu = add3(&cross(&suu, &sv), &cross(&su, &suv));
                    let nv = add3(&cross(&suv, &sv), &cross(&su, &svv));
                    for (bound, dn, name) in [(&normal[0], &nu, "N_u"), (&normal[1], &nv, "N_v")] {
                        if let DerivativeBound::Bounded(b) = bound {
                            assert!(normal_holds(b, &n, dn, &sq), "{at}: {name} outside {b:?}");
                            normals += 1;
                        }
                    }
                    checked += 1;
                }
            }
        }
    }
    println!("{checked} parameters checked, {normals} normal partials bounded");
    assert!(checked > scale(40) * 30);
    assert!(normals > 0);
}

/// The share of small boxes, 1/4096 of each direction's domain, whose normal partials are bounded,
/// among those where the exact normal is not nearly zero (|n| at least 10⁻³·|`S_u`|·|`S_v`| at the
/// box's middle): interval bounds converge as boxes shrink, so a caller that subdivides gets one.
#[test]
fn small_boxes_have_bounded_normal_partials() {
    let seed = resolve("G68 normal partials on small boxes", Seed(0x0067_686E)).unwrap();
    let mut rng = SplitMix64::new(seed);
    let m = &mut meter();
    let (mut regular, mut bounded) = (0usize, 0usize);
    for _ in 0..scale(600) {
        let s = patch(&mut rng);
        let ((a, b), (c, d)) = ParamSurface::domain(&s);
        let (u, v) = (a + (b - a) * unit(&mut rng), c + (d - c) * unit(&mut rng));
        let (hu, hv) = ((b - a) / 4096.0, (d - c) / 4096.0);
        let r = (
            (u.min(b - hu), u.min(b - hu) + hu),
            (v.min(d - hv), v.min(d - hv) + hv),
        );
        let p = s
            .partials(
                f64::midpoint(r.0.0, r.0.1),
                f64::midpoint(r.1.0, r.1.1),
                SIDES,
            )
            .unwrap();
        let (x, y) = (p[1], p[2]);
        let n = [
            x[1] * y[2] - x[2] * y[1],
            x[2] * y[0] - x[0] * y[2],
            x[0] * y[1] - x[1] * y[0],
        ];
        let len = |z: [f64; 3]| (z[0] * z[0] + z[1] * z[1] + z[2] * z[2]).sqrt();
        if len(n) < 1e-3 * len(x) * len(y) {
            continue;
        }
        regular += 1;
        if s.normal_derivative_bound(r, SIDES, m)
            .unwrap()
            .iter()
            .all(|b| matches!(b, DerivativeBound::Bounded(_)))
        {
            bounded += 1;
        }
    }
    println!("{bounded} of {regular} small boxes bounded");
    assert!(bounded * 100 >= regular * 95, "{bounded} of {regular}");
}

#[test]
fn a_rational_patch_is_bounded_by_its_control_points() {
    let seed = resolve("G68 rational partials inside the hull", Seed(0x6768)).unwrap();
    let mut rng = SplitMix64::new(seed);
    let m = &mut meter();
    for _ in 0..scale(200) {
        let s = patch(&mut rng);
        let r = region(&mut rng, ParamSurface::domain(&s));
        let hull = ParamSurface::bound(&s, r, m).unwrap();
        let p = s.partials_over(r, SIDES, 2, m).unwrap();
        for d in 0..3 {
            let slack = 1e-12 * (1.0 + hull.lo[d].abs().max(hull.hi[d].abs()));
            assert!(
                hull.lo[d] - slack <= p[0][d].lo() && p[0][d].hi() <= hull.hi[d] + slack,
                "over {r:?}: S {:?} outside the hull {hull:?}: {s:?}",
                p[0]
            );
        }
    }
}

#[test]
fn where_the_normal_vanishes_its_partials_are_unbounded() {
    let m = &mut meter();
    let z = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let sphere = Sphere::new([0.0; 3], 2.0, z, ((0.0, 2.0), (-0.5, 0.5))).unwrap();
    let pole = ((0.0, 0.5), (0.25, 0.5));
    assert_eq!(
        sphere.normal_derivative_bound(pole, SIDES, m).unwrap(),
        [DerivativeBound::Unbounded; 2]
    );
    let away = ((0.0, 0.015_625), (-0.25, -0.234_375));
    assert!(
        sphere
            .normal_derivative_bound(away, SIDES, m)
            .unwrap()
            .iter()
            .all(|b| matches!(b, DerivativeBound::Bounded(_)))
    );
    let cone = Cone::new([0.0; 3], 0.0, 1.0, z, ((0.0, 2.0), (0.0, 1.0))).unwrap();
    assert_eq!(
        cone.normal_derivative_bound(((0.0, 1.0), (0.0, 0.5)), SIDES, m)
            .unwrap(),
        [DerivativeBound::Unbounded; 2]
    );
    let plane = SurfaceKind::Plane(
        Plane::new(
            [1.0, 2.0, 3.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 1.0],
            ((-1.0, 1.0), (-1.0, 1.0)),
        )
        .unwrap(),
    );
    let r = ((-1.0, 1.0), (-1.0, 1.0));
    let SecondDerivativeBound::Bounded(b) = plane.second_derivative_bound(r, SIDES, m).unwrap()
    else {
        panic!("a plane's second partials are bounded");
    };
    let zero = Bounds {
        lo: [0.0; 3],
        hi: [0.0; 3],
    };
    assert_eq!((b.uu, b.uv, b.vv), (zero, zero, zero));
    assert_eq!(b.chord().unwrap(), 0.0);
    for n in plane.normal_derivative_bound(r, SIDES, m).unwrap() {
        let DerivativeBound::Bounded(n) = n else {
            panic!("a plane's normal partials are bounded");
        };
        assert!((0..3).all(|d| n.lo[d] <= 0.0 && 0.0 <= n.hi[d]), "{n:?}");
    }
}

#[test]
fn regions_outside_are_refused() {
    let m = &mut meter();
    let mut rng = SplitMix64::new(Seed(0x6869));
    let s = patch(&mut rng);
    let ((a, b), (c, d)) = ParamSurface::domain(&s);
    for r in [
        ((a - 1.0, b), (c, d)),
        ((a, b), (c, d + 1.0)),
        ((b, a), (c, d)),
    ] {
        assert_eq!(
            s.second_derivative_bound(r, SIDES, m),
            Err(CurveError::SpanOutside),
            "{r:?}"
        );
        assert_eq!(
            s.normal_derivative_bound(r, SIDES, m),
            Err(CurveError::SpanOutside),
            "{r:?}"
        );
    }
    let _ = Interval::point(0.0);
}

/// Which bound a kind broken on purpose gets wrong.
#[derive(Clone, Copy, Debug)]
enum Part {
    Second,
    Normal,
}

/// A NURBS surface whose second-derivative or normal-derivative bound over any box wider than a
/// point is moved a million units from the truth.
struct Moved(Surface<3>, Part);

fn far(b: Bounds<3>) -> Bounds<3> {
    Bounds {
        lo: b.lo.map(|x| x + 1e6),
        hi: b.hi.map(|x| x + 1e6),
    }
}

impl ParamSurface for Moved {
    fn domain(&self) -> ParamBox {
        ParamSurface::domain(&self.0)
    }
    fn point(&self, u: f64, v: f64, s: (Limit, Limit)) -> Result<[f64; 3], CurveError> {
        ParamSurface::point(&self.0, u, v, s)
    }
    fn partials(&self, u: f64, v: f64, s: (Limit, Limit)) -> Result<[[f64; 3]; 3], CurveError> {
        self.0.partials(u, v, s)
    }
    fn enclose(
        &self,
        u: f64,
        v: f64,
        s: (Limit, Limit),
        m: &mut WorkMeter<'_>,
    ) -> Result<[[Interval; 3]; 3], CurveError> {
        self.0.enclose(u, v, s, m)
    }
    fn bound(&self, r: ParamBox, m: &mut WorkMeter<'_>) -> Result<Bounds<3>, CurveError> {
        ParamSurface::bound(&self.0, r, m)
    }
    fn derivative_bound(
        &self,
        r: ParamBox,
        d: Direction,
        m: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<3>, CurveError> {
        ParamSurface::derivative_bound(&self.0, r, d, m)
    }
    fn partials_over(
        &self,
        r: ParamBox,
        s: (Limit, Limit),
        k: usize,
        m: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; 3]>, CurveError> {
        self.0.partials_over(r, s, k, m)
    }
    fn breaks(&self) -> (Vec<f64>, Vec<f64>) {
        self.0.breaks()
    }
    fn normal_bound(&self, r: ParamBox, m: &mut WorkMeter<'_>) -> Result<Bounds<3>, CurveError> {
        self.0.normal_bound(r, m)
    }
    fn second_derivative_bound(
        &self,
        r: ParamBox,
        s: (Limit, Limit),
        m: &mut WorkMeter<'_>,
    ) -> Result<SecondDerivativeBound, CurveError> {
        let b = self.0.second_derivative_bound(r, s, m)?;
        Ok(match (self.1, b) {
            (Part::Second, SecondDerivativeBound::Bounded(p)) if r.0.0 < r.0.1 => {
                SecondDerivativeBound::Bounded(SecondPartials {
                    uu: far(p.uu),
                    uv: far(p.uv),
                    vv: far(p.vv),
                })
            }
            _ => b,
        })
    }
    fn normal_derivative_bound(
        &self,
        r: ParamBox,
        s: (Limit, Limit),
        m: &mut WorkMeter<'_>,
    ) -> Result<[DerivativeBound<3>; 2], CurveError> {
        let n = self.0.normal_derivative_bound(r, s, m)?;
        if matches!(self.1, Part::Normal) && r.0.0 < r.0.1 {
            return Ok(n.map(|b| match b {
                DerivativeBound::Bounded(x) => DerivativeBound::Bounded(far(x)),
                DerivativeBound::Unbounded => b,
            }));
        }
        Ok(n)
    }
    fn to_nurbs(&self, m: &mut WorkMeter<'_>) -> Result<Changed<SurfaceForm>, CurveError> {
        self.0.to_nurbs(m)
    }
    fn split(
        &self,
        d: Direction,
        t: f64,
        m: &mut WorkMeter<'_>,
    ) -> Result<(Changed<SurfaceKind>, Changed<SurfaceKind>), CurveError> {
        ParamSurface::split(&self.0, d, t, m)
    }
    fn reversed(
        &self,
        d: Direction,
        m: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        self.0.reversed(d, m)
    }
    fn transformed(
        &self,
        t: &TypedTransform,
        m: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        self.0.transformed(t, m)
    }
}

#[test]
fn wrong_second_and_normal_bounds_are_caught() {
    // A gently curved biquadratic patch (z = x² + y² sampled on its net): every box the contract
    // takes has bounded second and normal partials, so a moved one must be caught.
    let b = Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let mut pts = Vec::new();
    for i in 0..3 {
        for j in 0..3 {
            let (x, y) = (f64::from(i) * 0.5, f64::from(j) * 0.5);
            pts.push([x, y, (x * x + y * y) * 0.25]);
        }
    }
    let s = Surface::new(b.clone(), b, &pts).unwrap();
    let params: Vec<(f64, f64)> = [0.25, 0.5, 0.75]
        .iter()
        .flat_map(|&u| [0.25, 0.5, 0.75].map(|v| (u, v)))
        .collect();
    let t = TypedTransform::Exact(GridTransform::quarter_turns(Axis::X, 1));
    let m = &mut meter();
    assert_eq!(check(&s, &params, &t, m), Ok(()));
    for part in [Part::Second, Part::Normal] {
        let e = check(&Moved(s.clone(), part), &params, &t, m).unwrap_err();
        assert_eq!(e.law, Law::DerivativeBoundHoldsDerivatives, "{part:?}: {e}");
    }
}
