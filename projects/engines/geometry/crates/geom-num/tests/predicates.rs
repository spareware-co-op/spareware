//! The exact predicates checked against an exact oracle: on random points, on near-degenerate
//! constructions, at the extremes of the grid, and by their laws.
//!
//! The oracle is the cofactor expansion of the determinant on `num-bigint` integers, applied to the
//! homogeneous matrices of the absolute coordinates: rows [p, 1] for orientation, [p, |p|², 1] for
//! in-circle and in-sphere. That is a different formulation from the implementation (which
//! translates to one point and evaluates differences in fixed-width integers), so agreement is
//! evidence, not repetition. Expanding the homogeneous determinants gives the documented
//! conventions:
//!
//! - orient2d = sign det[a 1; b 1; c 1]
//! - orient3d = −sign det[a 1; b 1; c 1; d 1]
//! - incircle = sign det[a |a|² 1; b |b|² 1; c |c|² 1; d |d|² 1]
//! - insphere = −sign det[a |a|² 1; …; e |e|² 1]
//!
//! and `conventions_*` pin those signs with geometric cases that use no determinant at all.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use geom_num::Sign;
use geom_num::grid::{COORD_MAX, P2, P3};
use geom_num::predicate::{incircle, insphere, orient2d, orient3d};
use num_bigint::{BigInt, Sign as BigSign};
use num_traits::{One, Signed, Zero};
use proptest::array::uniform;
use proptest::prelude::*;
use proptest::sample::Index;

/// The largest coordinate magnitude, 2^40 − 1.
const M: i64 = COORD_MAX;

type Pt2 = [i64; 2];
type Pt3 = [i64; 3];

fn p2([x, y]: Pt2) -> P2 {
    P2::new(x, y).unwrap()
}

fn p3([x, y, z]: Pt3) -> P3 {
    P3::new(x, y, z).unwrap()
}

// ---------------------------------------------------------------------------------------------
// The predicates under test, on plain arrays.

fn orient2d_of(p: [Pt2; 3]) -> Sign {
    let [a, b, c] = p.map(p2);
    orient2d(a, b, c)
}

fn orient3d_of(p: [Pt3; 4]) -> Sign {
    let [a, b, c, d] = p.map(p3);
    orient3d(a, b, c, d)
}

fn incircle_of(p: [Pt2; 4]) -> Sign {
    let [a, b, c, d] = p.map(p2);
    incircle(a, b, c, d)
}

fn insphere_of(p: [Pt3; 5]) -> Sign {
    let [a, b, c, d, e] = p.map(p3);
    insphere(a, b, c, d, e)
}

// ---------------------------------------------------------------------------------------------
// The oracle.

/// The determinant by Laplace (cofactor) expansion along the top row, recursively, sharing minors.
///
/// `minors[S]` is the determinant of the last |S| rows restricted to the columns in the set S (a
/// bit mask, columns in increasing order), so it expands as Σ over the columns j of S, at position
/// k within S, of (−1)^k · rows[n − |S|][j] · minors[S without j]. Every subset is smaller than its
/// set as a number, so increasing order computes each minor after the ones it needs.
fn det(rows: &[Vec<BigInt>]) -> BigInt {
    let n = rows.len();
    let mut minors = vec![BigInt::zero(); 1 << n];
    minors[0] = BigInt::one();
    for set in 1usize..1 << n {
        let row = &rows[n - set.count_ones() as usize];
        let mut sum = BigInt::zero();
        for (k, j) in (0..n).filter(|&j| set >> j & 1 == 1).enumerate() {
            let term = &row[j] * &minors[set & !(1 << j)];
            if k % 2 == 0 {
                sum += term;
            } else {
                sum -= term;
            }
        }
        minors[set] = sum;
    }
    minors.pop().unwrap()
}

fn lift(p: &[i64]) -> BigInt {
    p.iter().map(|&v| BigInt::from(v) * v).sum()
}

fn orient2d_value(p: [Pt2; 3]) -> BigInt {
    det(&p.map(|[x, y]| vec![x.into(), y.into(), BigInt::one()]))
}

fn orient3d_value(p: [Pt3; 4]) -> BigInt {
    -det(&p.map(|[x, y, z]| vec![x.into(), y.into(), z.into(), BigInt::one()]))
}

fn incircle_value(p: [Pt2; 4]) -> BigInt {
    det(&p.map(|q| vec![q[0].into(), q[1].into(), lift(&q), BigInt::one()]))
}

fn insphere_value(p: [Pt3; 5]) -> BigInt {
    -det(&p.map(|q| vec![q[0].into(), q[1].into(), q[2].into(), lift(&q), BigInt::one()]))
}

fn sign_of(v: &BigInt) -> Sign {
    match v.sign() {
        BigSign::Minus => Sign::Negative,
        BigSign::NoSign => Sign::Zero,
        BigSign::Plus => Sign::Positive,
    }
}

// Each predicate checked against the oracle; returns the oracle's exact value.

fn checked_orient2d(p: [Pt2; 3]) -> BigInt {
    let v = orient2d_value(p);
    assert_eq!(orient2d_of(p), sign_of(&v), "orient2d {p:?}");
    v
}

fn checked_orient3d(p: [Pt3; 4]) -> BigInt {
    let v = orient3d_value(p);
    assert_eq!(orient3d_of(p), sign_of(&v), "orient3d {p:?}");
    v
}

fn checked_incircle(p: [Pt2; 4]) -> BigInt {
    let v = incircle_value(p);
    assert_eq!(incircle_of(p), sign_of(&v), "incircle {p:?}");
    v
}

fn checked_insphere(p: [Pt3; 5]) -> BigInt {
    let v = insphere_value(p);
    assert_eq!(insphere_of(p), sign_of(&v), "insphere {p:?}");
    v
}

// ---------------------------------------------------------------------------------------------
// Helpers for constructions.

fn in_range(v: i64) -> bool {
    (-M..=M).contains(&v)
}

/// `seed` mapped into lo..=hi (hi − lo is below 2^43 everywhere here).
fn pick(lo: i64, hi: i64, seed: u64) -> i64 {
    assert!(lo <= hi);
    lo + (seed % ((hi - lo) as u64 + 1)) as i64
}

fn floor_div(a: i64, b: i64) -> i64 {
    let q = a / b;
    if a % b != 0 && (a < 0) != (b < 0) { q - 1 } else { q }
}

fn ceil_div(a: i64, b: i64) -> i64 {
    let q = a / b;
    if a % b != 0 && (a < 0) == (b < 0) { q + 1 } else { q }
}

/// The k for which p + k·w stays on the grid: an interval that always contains 0.
fn k_range<const D: usize>(p: [i64; D], w: [i64; D]) -> (i64, i64) {
    if w.iter().all(|&c| c == 0) {
        return (0, 0);
    }
    let (mut lo, mut hi) = (-2 * M, 2 * M);
    for (pc, wc) in p.into_iter().zip(w) {
        if wc == 0 {
            continue;
        }
        // −M − pc ≤ k·wc ≤ M − pc.
        let (s, e) = (-M - pc, M - pc);
        let (klo, khi) = if wc > 0 { (ceil_div(s, wc), floor_div(e, wc)) } else { (ceil_div(e, wc), floor_div(s, wc)) };
        lo = lo.max(klo);
        hi = hi.min(khi);
    }
    (lo, hi)
}

/// p + k·w, which the caller has kept on the grid with [`k_range`].
fn along<const D: usize>(p: [i64; D], w: [i64; D], k: i64) -> [i64; D] {
    let mut q = p;
    for (qc, wc) in q.iter_mut().zip(w) {
        *qc += k * wc;
        assert!(in_range(*qc));
    }
    q
}

/// A point on the line p + k·w, k chosen by `seed`.
fn on_line<const D: usize>(p: [i64; D], w: [i64; D], seed: u64) -> [i64; D] {
    let (lo, hi) = k_range(p, w);
    along(p, w, pick(lo, hi, seed))
}

/// Every way of moving one point one grid step along one axis and staying on the grid, as
/// (index of the moved point, axis, moved configuration).
fn one_step_moves<const D: usize, const N: usize>(p: [[i64; D]; N]) -> Vec<(usize, usize, [[i64; D]; N])> {
    let mut out = Vec::new();
    for i in 0..N {
        for axis in 0..D {
            for step in [-1, 1] {
                let mut q = p;
                q[i][axis] += step;
                if in_range(q[i][axis]) {
                    out.push((i, axis, q));
                }
            }
        }
    }
    out
}

/// The points other than the one at index `skip`, in order.
fn others<const D: usize, const N: usize, const K: usize>(p: [[i64; D]; N], skip: usize) -> [[i64; D]; K] {
    let rest: Vec<[i64; D]> = p.iter().enumerate().filter(|&(i, _)| i != skip).map(|(_, q)| *q).collect();
    rest.try_into().unwrap()
}

fn cross3(u: Pt3, v: Pt3) -> [i128; 3] {
    let [u, v] = [u, v].map(|w| w.map(i128::from));
    [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]
}

fn sub3(a: Pt3, b: Pt3) -> Pt3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// Whether three points lie on one line (including any two coinciding).
fn collinear3(p: [Pt3; 3]) -> bool {
    cross3(sub3(p[1], p[0]), sub3(p[2], p[0])) == [0, 0, 0]
}

// ---------------------------------------------------------------------------------------------
// Laws, checked on the predicates alone.

/// Antisymmetry under every transposition of two points; invariance under a grid translation that
/// keeps every point in range (chosen by `seeds`); a sign flip under mirroring each axis and under
/// swapping two axes (also a reflection).
fn check_laws<const D: usize, const N: usize>(f: fn([[i64; D]; N]) -> Sign, p: [[i64; D]; N], seeds: [u64; D]) {
    let s = f(p);
    for i in 0..N {
        for j in i + 1..N {
            let mut q = p;
            q.swap(i, j);
            assert_eq!(f(q), s.negate(), "swapping points {i} and {j} of {p:?}");
        }
    }
    let mut t = [0i64; D];
    for axis in 0..D {
        let lo = p.iter().map(|q| q[axis]).min().unwrap();
        let hi = p.iter().map(|q| q[axis]).max().unwrap();
        t[axis] = pick(-M - lo, M - hi, seeds[axis]);
    }
    let moved = p.map(|q| std::array::from_fn(|axis| q[axis] + t[axis]));
    assert_eq!(f(moved), s, "translating {p:?} by {t:?}");
    for axis in 0..D {
        let mirrored = p.map(|mut q| {
            q[axis] = -q[axis];
            q
        });
        assert_eq!(f(mirrored), s.negate(), "mirroring axis {axis} of {p:?}");
    }
    for i in 0..D {
        for j in i + 1..D {
            let swapped = p.map(|mut q| {
                q.swap(i, j);
                q
            });
            assert_eq!(f(swapped), s.negate(), "swapping axes {i} and {j} of {p:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Exact lattice circles and spheres.

/// Radii with many exact lattice points: 27 720 = lcm(1..=12), 232 792 560 = lcm(1..=20) and
/// 963 761 198 400 = 12 · lcm(1..=28), the last close to the whole grid range.
const RADII: [i64; 3] = [27_720, 232_792_560, 963_761_198_400];

/// Lattice points on the circle of radius l about the origin: (l/n)(p² − q², 2pq) with
/// n = p² + q² dividing l, and their mirror images across the diagonal.
fn circle(l: i64) -> Vec<Pt2> {
    let mut set = BTreeSet::new();
    for p in -30i64..=30 {
        for q in -30i64..=30 {
            let n = p * p + q * q;
            if n > 0 && l % n == 0 {
                let k = l / n;
                let (x, y) = (k * (p * p - q * q), k * 2 * p * q);
                set.insert([x, y]);
                set.insert([y, x]);
            }
        }
    }
    set.into_iter().collect()
}

/// Lattice points on the sphere of radius l about the origin: (l/n)(2ps, 2qs, p² + q² − s²) with
/// n = p² + q² + s² dividing l, with every permutation of the axes.
fn sphere(l: i64) -> Vec<Pt3> {
    let mut set = BTreeSet::new();
    for p in -12i64..=12 {
        for q in -12i64..=12 {
            for s in -12i64..=12 {
                let n = p * p + q * q + s * s;
                if n > 0 && l % n == 0 {
                    let k = l / n;
                    let [x, y, z] = [k * 2 * p * s, k * 2 * q * s, k * (p * p + q * q - s * s)];
                    for v in [[x, y, z], [x, z, y], [y, x, z], [y, z, x], [z, x, y], [z, y, x]] {
                        set.insert(v);
                    }
                }
            }
        }
    }
    set.into_iter().collect()
}

fn circles() -> &'static [Vec<Pt2>] {
    static C: OnceLock<Vec<Vec<Pt2>>> = OnceLock::new();
    C.get_or_init(|| RADII.iter().map(|&l| circle(l)).collect())
}

fn spheres() -> &'static [Vec<Pt3>] {
    static S: OnceLock<Vec<Vec<Pt3>>> = OnceLock::new();
    S.get_or_init(|| RADII.iter().map(|&l| sphere(l)).collect())
}

/// Points on a lattice circle of radius `RADII[r]`, centred anywhere that keeps the circle on the grid.
fn on_circle<const N: usize>(r: usize, centre: [u64; 2], picks: [Index; N]) -> [Pt2; N] {
    let l = RADII[r];
    let pts = &circles()[r];
    let o = centre.map(|s| pick(-(M - l), M - l, s));
    picks.map(|i| {
        let q = pts[i.index(pts.len())];
        [o[0] + q[0], o[1] + q[1]]
    })
}

/// Points on a lattice sphere of radius `RADII[r]`, centred anywhere that keeps the sphere on the grid.
fn on_sphere<const N: usize>(r: usize, centre: [u64; 3], picks: [Index; N]) -> [Pt3; N] {
    let l = RADII[r];
    let pts = &spheres()[r];
    let o = centre.map(|s| pick(-(M - l), M - l, s));
    picks.map(|i| {
        let q = pts[i.index(pts.len())];
        [o[0] + q[0], o[1] + q[1], o[2] + q[2]]
    })
}

#[test]
fn lattice_circles_and_spheres_are_exact() {
    for (r, &l) in RADII.iter().enumerate() {
        let l2 = i128::from(l) * i128::from(l);
        let c = &circles()[r];
        let s = &spheres()[r];
        assert!(c.len() >= 12 && s.len() >= 48, "radius {l}: {} circle and {} sphere points", c.len(), s.len());
        assert!(c.iter().all(|q| q.iter().map(|&v| i128::from(v) * i128::from(v)).sum::<i128>() == l2));
        assert!(s.iter().all(|q| q.iter().map(|&v| i128::from(v) * i128::from(v)).sum::<i128>() == l2));
    }
}

// ---------------------------------------------------------------------------------------------
// Strategies.

/// A coordinate: anywhere on the grid, near zero (where degeneracies are common) or at an edge.
fn coord() -> impl Strategy<Value = i64> {
    prop_oneof![
        4 => -M..=M,
        2 => -4i64..=4,
        1 => (0i64..4).prop_map(|k| M - k),
        1 => (0i64..4).prop_map(|k| k - M),
    ]
}

/// A direction for a construction: tiny, middling, or spanning nearly the whole grid.
fn direction<const D: usize>() -> impl Strategy<Value = [i64; D]> {
    prop_oneof![Just(3i64), Just(1i64 << 20), Just(1i64 << 39)].prop_flat_map(|s| uniform::<_, D>(-s..=s))
}

// ---------------------------------------------------------------------------------------------
// Conventions, pinned by geometry rather than by any determinant.

#[test]
fn conventions_orient2d() {
    // Counter-clockwise is positive, clockwise negative, collinear zero; also at full scale.
    assert_eq!(orient2d_of([[0, 0], [1, 0], [0, 1]]), Sign::Positive);
    assert_eq!(orient2d_of([[0, 0], [0, 1], [1, 0]]), Sign::Negative);
    assert_eq!(orient2d_of([[0, 0], [1, 1], [2, 2]]), Sign::Zero);
    assert_eq!(orient2d_of([[-M, -M], [M, -M], [M, M]]), Sign::Positive);
    assert_eq!(orient2d_of([[-M, -M], [M, M], [0, 0]]), Sign::Zero);
}

#[test]
fn conventions_orient3d() {
    // d on the side (b − a) × (c − a) points to is positive: here that normal is +z.
    let base = [[0, 0, 0], [1, 0, 0], [0, 1, 0]];
    let with = |d: Pt3| orient3d_of([base[0], base[1], base[2], d]);
    assert_eq!(with([0, 0, 1]), Sign::Positive);
    assert_eq!(with([5, -7, -1]), Sign::Negative);
    assert_eq!(with([5, -7, 0]), Sign::Zero);
    assert_eq!(orient3d_of([[-M, -M, -M], [M, -M, -M], [-M, M, -M], [M, M, M]]), Sign::Positive);
}

#[test]
fn conventions_incircle() {
    // Counter-clockwise a, b, c; circle centre (1, 1), radius² 2.
    let abc = [[0, 0], [2, 0], [0, 2]];
    let with = |d: Pt2| incircle_of([abc[0], abc[1], abc[2], d]);
    assert_eq!(with([1, 1]), Sign::Positive);
    assert_eq!(with([3, 3]), Sign::Negative);
    assert_eq!(with([2, 2]), Sign::Zero);
    // Clockwise a, b, c flip the answer.
    assert_eq!(incircle_of([abc[0], abc[2], abc[1], [1, 1]]), Sign::Negative);
    // At full scale: three corners of the grid counter-clockwise, centre inside, fourth corner on.
    assert_eq!(incircle_of([[-M, -M], [M, -M], [M, M], [0, 0]]), Sign::Positive);
    assert_eq!(incircle_of([[-M, -M], [M, -M], [M, M], [-M, M]]), Sign::Zero);
    assert_eq!(incircle_of([[-M, -M], [M, -M], [0, 0], [M, M]]), Sign::Negative);
}

#[test]
fn conventions_insphere() {
    // orient3d(a, b, c, d) positive; sphere centre (1, 1, 1), radius² 3.
    let abcd = [[0, 0, 0], [2, 0, 0], [0, 2, 0], [0, 0, 2]];
    assert_eq!(orient3d_of(abcd), Sign::Positive);
    let with = |e: Pt3| insphere_of([abcd[0], abcd[1], abcd[2], abcd[3], e]);
    assert_eq!(with([1, 1, 1]), Sign::Positive);
    assert_eq!(with([3, 3, 3]), Sign::Negative);
    assert_eq!(with([2, 2, 0]), Sign::Zero);
    // Negative orientation flips the answer.
    assert_eq!(insphere_of([abcd[1], abcd[0], abcd[2], abcd[3], [1, 1, 1]]), Sign::Negative);
    // At full scale: a positively oriented regular tetrahedron on the grid's corners, centre inside.
    let t = [[-M, -M, -M], [M, M, -M], [-M, M, M], [M, -M, M]];
    assert_eq!(orient3d_of(t), Sign::Positive);
    assert_eq!(insphere_of([t[0], t[1], t[2], t[3], [0, 0, 0]]), Sign::Positive);
    assert_eq!(insphere_of([t[0], t[1], t[2], t[3], [M, M, M]]), Sign::Zero);
}

#[test]
fn orient3d_of_a_flat_triangle_is_orient2d() {
    // Points in a plane z = z0, d above it: orient3d = (d.z − z0) · orient2d, so the signs agree.
    for p in [[[0, 0], [1, 0], [0, 1]], [[0, 0], [0, 1], [1, 0]], [[-M, -M], [M, M], [3, 3]], [[M, -M], [-M, M], [M, M]]] {
        for (z0, d) in [(0, [0, 0, 1]), (-M, [M, -M, M]), (M - 1, [-M, 5, M])] {
            let lifted = [p[0], p[1], p[2]].map(|[x, y]| [x, y, z0]);
            assert_eq!(orient3d_of([lifted[0], lifted[1], lifted[2], d]), orient2d_of(p));
        }
    }
}

#[test]
fn degenerate_inputs_mean_what_the_docs_say() {
    // incircle with collinear distinct a, b, c: zero exactly when d is on their line.
    let line = [[0, 0], [1, 1], [3, 3]];
    assert_eq!(checked_incircle([line[0], line[1], line[2], [-M, -M]]), BigInt::zero());
    assert!(!checked_incircle([line[0], line[1], line[2], [0, 1]]).is_zero());
    // Two of a, b, c coinciding: zero for every d.
    for d in [[0, 1], [M, -M], [7, 7]] {
        assert_eq!(checked_incircle([[2, 3], [2, 3], [9, -4], d]), BigInt::zero());
    }
    // insphere with coplanar a, b, c, d not on one circle: zero exactly when e is in their plane.
    let flat = [[0, 0, 0], [1, 0, 0], [0, 1, 0], [1, 2, 0]];
    assert_eq!(checked_insphere([flat[0], flat[1], flat[2], flat[3], [5, 7, 0]]), BigInt::zero());
    assert!(!checked_insphere([flat[0], flat[1], flat[2], flat[3], [5, 7, 1]]).is_zero());
    // Coplanar and on one circle (a square), or on one line: zero for every e.
    for e in [[1, 1, 5], [-M, M, -M], [3, 3, 0]] {
        assert_eq!(checked_insphere([[0, 0, 0], [2, 0, 0], [0, 2, 0], [2, 2, 0], e]), BigInt::zero());
        assert_eq!(checked_insphere([[0, 0, 0], [1, 1, 1], [2, 2, 2], [-5, -5, -5], e]), BigInt::zero());
    }
}

// ---------------------------------------------------------------------------------------------
// Extremes: every ordered choice of points from the corners (and midpoints) of the grid, against
// the oracle, with the largest value each determinant reaches there.

/// The `n` base-`base` digits of `code`, least significant first.
fn digits(mut code: usize, base: usize, n: usize) -> Vec<usize> {
    (0..n)
        .map(|_| {
            let d = code % base;
            code /= base;
            d
        })
        .collect()
}

/// The corners of the grid's square or cube: every coordinate ±M.
fn corners<const D: usize>() -> Vec<[i64; D]> {
    (0..1usize << D).map(|k| std::array::from_fn(|axis| if k >> axis & 1 == 0 { -M } else { M })).collect()
}

/// Every point whose coordinates are in {−M, 0, M}: corners, edge midpoints, centre.
fn corners_and_midpoints<const D: usize>() -> Vec<[i64; D]> {
    (0..3usize.pow(D as u32))
        .map(|k| {
            let d = digits(k, 3, D);
            std::array::from_fn(|axis| [-M, 0, M][d[axis]])
        })
        .collect()
}

/// Every ordered choice of N points from `set`, repeats allowed, with `f` checked against the oracle
/// `value` on each. The oracle runs once per set of distinct points: another order of the same
/// points multiplies the determinant by the order's sign, and a repeated point makes two rows equal,
/// so the determinant is zero. Returns the largest |value| and the signs seen.
fn sweep<const D: usize, const N: usize>(
    set: &[[i64; D]],
    f: fn([[i64; D]; N]) -> Sign,
    value: fn([[i64; D]; N]) -> BigInt,
) -> (BigInt, BTreeSet<Sign>) {
    let mut by_choice: BTreeMap<[usize; N], BigInt> = BTreeMap::new();
    let (mut max, mut seen) = (BigInt::zero(), BTreeSet::new());
    for code in 0..set.len().pow(N as u32) {
        let idx: [usize; N] = digits(code, set.len(), N).try_into().unwrap();
        let mut sorted = idx;
        sorted.sort_unstable();
        let v = if sorted.windows(2).any(|w| w[0] == w[1]) {
            BigInt::zero()
        } else {
            let base = by_choice.entry(sorted).or_insert_with(|| value(sorted.map(|i| set[i]))).clone();
            let inversions = (0..N).flat_map(|i| (i + 1..N).map(move |j| (i, j))).filter(|&(i, j)| idx[i] > idx[j]).count();
            if inversions % 2 == 1 { -base } else { base }
        };
        let p = idx.map(|i| set[i]);
        assert_eq!(f(p), sign_of(&v), "{p:?}");
        seen.insert(sign_of(&v));
        max = max.max(v.abs());
    }
    (max, seen)
}

fn m_pow(k: u32) -> BigInt {
    BigInt::from(M).pow(k)
}

#[test]
fn orient2d_at_the_extremes() {
    // 9^3 configurations, including every one with all coordinates at ±M.
    let (max, seen) = sweep(&corners_and_midpoints::<2>(), orient2d_of, orient2d_value);
    assert_eq!(seen.len(), 3);
    // δ² = 4M²: half the bounding square, the largest triangle in a square.
    assert_eq!(max, BigInt::from(4) * m_pow(2));
    assert_eq!(checked_orient2d([[-M, -M], [M, -M], [M, M]]), BigInt::from(4) * m_pow(2));
}

#[test]
fn orient3d_at_the_extremes() {
    // Every coordinate at ±M: 8^4 configurations.
    let (max, seen) = sweep(&corners::<3>(), orient3d_of, orient3d_value);
    assert_eq!(seen.len(), 3);
    // 2δ³ = 16M³: the regular tetrahedron on alternate corners, a third of the cube.
    assert_eq!(max, BigInt::from(16) * m_pow(3));
    let t = [[-M, -M, -M], [M, M, -M], [-M, M, M], [M, -M, M]];
    assert_eq!(checked_orient3d(t), BigInt::from(16) * m_pow(3));
}

#[test]
fn incircle_at_the_extremes() {
    // Every coordinate at ±M: always zero, the corners being cocircular, with the largest
    // intermediate terms cancelling exactly.
    let (max, _) = sweep(&corners::<2>(), incircle_of, incircle_value);
    assert!(max.is_zero());
    // Every coordinate in {−M, 0, M}: 9^4 configurations.
    let (max, seen) = sweep(&corners_and_midpoints::<2>(), incircle_of, incircle_value);
    assert_eq!(seen.len(), 3);
    // 8M⁴ = δ⁴ / 2: three corners and the centre.
    assert_eq!(max, BigInt::from(8) * m_pow(4));
    assert_eq!(checked_incircle([[-M, -M], [M, -M], [M, M], [0, 0]]), BigInt::from(8) * m_pow(4));
}

#[test]
fn insphere_at_the_extremes() {
    // Every coordinate at ±M: always zero, the corners being cospherical, with the largest
    // intermediate terms cancelling exactly.
    let mut pts = corners::<3>();
    let (max, _) = sweep(&pts, insphere_of, insphere_value);
    assert!(max.is_zero());
    // The corners and the centre: 9^5 configurations.
    pts.push([0, 0, 0]);
    let (max, seen) = sweep(&pts, insphere_of, insphere_value);
    assert_eq!(seen.len(), 3);
    // 48M⁵ = 3δ⁵ / 2: a regular tetrahedron on alternate corners and e at the centre.
    assert_eq!(max, BigInt::from(48) * m_pow(5));
    let t = [[-M, -M, -M], [M, M, -M], [-M, M, M], [M, -M, M]];
    assert_eq!(checked_insphere([t[0], t[1], t[2], t[3], [0, 0, 0]]), BigInt::from(48) * m_pow(5));
}

// ---------------------------------------------------------------------------------------------
// Random points against the oracle, and the laws.

proptest! {
    #[test]
    fn orient2d_matches_the_oracle(p in uniform::<_, 3>(uniform::<_, 2>(coord())), seeds in any::<[u64; 2]>()) {
        checked_orient2d(p);
        check_laws(orient2d_of, p, seeds);
    }

    #[test]
    fn orient3d_matches_the_oracle(p in uniform::<_, 4>(uniform::<_, 3>(coord())), seeds in any::<[u64; 3]>()) {
        checked_orient3d(p);
        check_laws(orient3d_of, p, seeds);
    }

    #[test]
    fn incircle_matches_the_oracle(p in uniform::<_, 4>(uniform::<_, 2>(coord())), seeds in any::<[u64; 2]>()) {
        checked_incircle(p);
        check_laws(incircle_of, p, seeds);
    }

    #[test]
    fn insphere_matches_the_oracle(p in uniform::<_, 5>(uniform::<_, 3>(coord())), seeds in any::<[u64; 3]>()) {
        checked_insphere(p);
        check_laws(insphere_of, p, seeds);
    }
}

// ---------------------------------------------------------------------------------------------
// Near-degenerate constructions: exactly degenerate, then every one-step move off it.

proptest! {
    /// Three points on one lattice line; moving one of them one step is zero exactly when the
    /// step runs along the line (given the other two are distinct).
    #[test]
    fn orient2d_on_collinear_points_and_one_step_off(
        a in uniform::<_, 2>(coord()), w in direction::<2>(), s in any::<[u64; 2]>(), seeds in any::<[u64; 2]>(),
    ) {
        let p = [a, on_line(a, w, s[0]), on_line(a, w, s[1])];
        prop_assert!(checked_orient2d(p).is_zero());
        check_laws(orient2d_of, p, seeds);
        for (i, axis, q) in one_step_moves(p) {
            let v = checked_orient2d(q);
            let rest: [Pt2; 2] = others(p, i);
            if rest[0] != rest[1] {
                // The step is parallel to the line exactly when w has no component across it.
                prop_assert_eq!(v.is_zero(), w[1 - axis] == 0, "move point {} along axis {} in {:?}", i, axis, p);
            }
            check_laws(orient2d_of, q, seeds);
        }
    }

    /// Four points on one lattice plane; moving one of them one step is zero exactly when the
    /// plane contains the step (given the other three are not collinear).
    #[test]
    fn orient3d_on_coplanar_points_and_one_step_off(
        a in uniform::<_, 3>(coord()), u in direction::<3>(), w in direction::<3>(), s in any::<[u64; 6]>(),
        seeds in any::<[u64; 3]>(),
    ) {
        let pick_in_plane = |k: usize| on_line(on_line(a, u, s[2 * k]), w, s[2 * k + 1]);
        let p = [a, pick_in_plane(0), pick_in_plane(1), pick_in_plane(2)];
        prop_assert!(checked_orient3d(p).is_zero());
        check_laws(orient3d_of, p, seeds);
        let normal = cross3(u, w);
        for (i, axis, q) in one_step_moves(p) {
            let v = checked_orient3d(q);
            if !collinear3(others(p, i)) {
                prop_assert_eq!(v.is_zero(), normal[axis] == 0, "move point {} along axis {} in {:?}", i, axis, p);
            }
            check_laws(orient3d_of, q, seeds);
        }
    }

    /// Four points on one lattice circle (up to the whole grid in size); moving any one of them one
    /// step leaves the circle (2x + 1 is never 0), so the result is non-zero exactly when the
    /// other three are distinct.
    #[test]
    fn incircle_on_cocircular_points_and_one_step_off(
        r in 0..RADII.len(), centre in any::<[u64; 2]>(), picks in any::<[Index; 4]>(), seeds in any::<[u64; 2]>(),
    ) {
        let p = on_circle(r, centre, picks);
        prop_assert!(checked_incircle(p).is_zero());
        for (i, _, q) in one_step_moves(p) {
            let v = checked_incircle(q);
            let rest: [Pt2; 3] = others(p, i);
            let distinct = rest[0] != rest[1] && rest[1] != rest[2] && rest[0] != rest[2];
            prop_assert_eq!(v.is_zero(), !distinct, "move point {} in {:?}", i, p);
            check_laws(incircle_of, q, seeds);
        }
    }

    /// Five points on one lattice sphere (up to the whole grid in size); moving any one of them one
    /// step leaves the sphere, so the result is non-zero exactly when the other four are not
    /// coplanar (four coplanar points on a sphere lie on one circle).
    #[test]
    fn insphere_on_cospherical_points_and_one_step_off(
        r in 0..RADII.len(), centre in any::<[u64; 3]>(), picks in any::<[Index; 5]>(), seeds in any::<[u64; 3]>(),
    ) {
        let p = on_sphere(r, centre, picks);
        prop_assert!(checked_insphere(p).is_zero());
        for (i, _, q) in one_step_moves(p) {
            let v = checked_insphere(q);
            let coplanar = orient3d_of(others(p, i)) == Sign::Zero;
            prop_assert_eq!(v.is_zero(), coplanar, "move point {} in {:?}", i, p);
            check_laws(insphere_of, q, seeds);
        }
    }
}
