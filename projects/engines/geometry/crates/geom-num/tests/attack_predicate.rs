//! Adversarial tests of `geom_num::predicate`: they try to break orient2d, orient3d, incircle and
//! insphere rather than confirm them, each against oracles that share no code with the module or
//! with tests/predicates.rs.
//!
//! Oracles:
//! - Bareiss fraction-free elimination on `num-bigint`, applied to the homogeneous matrices of the
//!   absolute coordinates (tests/predicates.rs uses cofactor expansion; the module uses
//!   differences from one point).
//! - A port of the formulas of Shewchuk's `predicates.c` (`orient2dfast` to `inspherefast`, which
//!   are exact when evaluated on big integers). It pins the documented conventions against the
//!   reference code: the same signs for orient2d and incircle, the opposite for orient3d and
//!   insphere. The port is itself checked against Shewchuk's own documented geometry.
//! - Geometry with no determinant at all: exact circumcentres in `num-rational`; the circumsphere
//!   of an axis-aligned box, whose power at a grid point is an integer; unimodular simplices whose
//!   determinant (±1) is known by construction.
//! - Laws that tie the predicates together: incircle is −orient3d of the points lifted onto the
//!   paraboloid; insphere with four points in one plane factors into orient3d × orient2d ×
//!   incircle; orient3d toward an axis is orient2d of the projection.
//!
//! Also: every permutation, reflection, rotation, scaling and corner translation; exhaustive small
//! grids scaled to the full grid; the documented degenerate cases at full scale (collinear, tilted
//! cocircular, three collinear and a fourth in the plane), including the documented sign of every
//! degenerate but non-zero answer, as exact values; the wide step's exact value at the
//! configurations that maximise its intermediates; and a scan of the module's code for native
//! arithmetic, which would let a build with overflow checks off change an answer.

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
#![allow(
    clippy::many_single_char_names,
    reason = "points and values are named a, b, c and so on, as in the formulas they check"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::needless_range_loop,
    clippy::too_many_arguments
)]

use std::collections::BTreeSet;

use geom_num::grid::{COORD_MAX, P2, P3};
use geom_num::predicate::{incircle, insphere, orient2d, orient3d};
use geom_num::{Sign, Wide};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use proptest::array::uniform;
use proptest::prelude::*;
use proptest::sample::Index;
use proptest::test_runner::Config;

/// The largest coordinate magnitude, 2^40 − 1.
const M: i64 = COORD_MAX;

type Pt2 = [i64; 2];
type Pt3 = [i64; 3];

/// Every property runs with this configuration. Failures are not persisted to a regressions file:
/// this file is the only one the attack may write.
fn cfg(cases: u32) -> Config {
    Config {
        cases,
        failure_persistence: None,
        ..Config::default()
    }
}

// ---------------------------------------------------------------------------------------------
// The predicates under test, on plain arrays.
// ---------------------------------------------------------------------------------------------

fn p2([x, y]: Pt2) -> P2 {
    P2::new(x, y).unwrap()
}

fn p3([x, y, z]: Pt3) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn o2(p: [Pt2; 3]) -> Sign {
    let [a, b, c] = p.map(p2);
    orient2d(a, b, c).unwrap()
}

fn o3(p: [Pt3; 4]) -> Sign {
    let [a, b, c, d] = p.map(p3);
    orient3d(a, b, c, d).unwrap()
}

fn ic(p: [Pt2; 4]) -> Sign {
    let [a, b, c, d] = p.map(p2);
    incircle(a, b, c, d).unwrap()
}

fn ins(p: [Pt3; 5]) -> Sign {
    let [a, b, c, d, e] = p.map(p3);
    insphere(a, b, c, d, e).unwrap()
}

// ---------------------------------------------------------------------------------------------
// Small helpers.
// ---------------------------------------------------------------------------------------------

fn big(v: i64) -> BigInt {
    BigInt::from(v)
}

fn pow2(n: u32) -> BigInt {
    BigInt::one() << n
}

fn sign_of(v: &BigInt) -> Sign {
    if v.is_negative() {
        Sign::Negative
    } else if v.is_zero() {
        Sign::Zero
    } else {
        Sign::Positive
    }
}

fn sign_of_rat(v: &BigRational) -> Sign {
    if v.is_negative() {
        Sign::Negative
    } else if v.is_zero() {
        Sign::Zero
    } else {
        Sign::Positive
    }
}

/// `s`, negated when `odd`.
fn odd_if(s: Sign, odd: bool) -> Sign {
    if odd { s.negate() } else { s }
}

fn in_range(v: i64) -> bool {
    (-M..=M).contains(&v)
}

/// p − q, exactly.
fn diff<const D: usize>(p: [i64; D], q: [i64; D]) -> [BigInt; D] {
    std::array::from_fn(|k| big(p[k]) - big(q[k]))
}

fn add<const D: usize>(p: [i64; D], r: [i64; D]) -> [i64; D] {
    std::array::from_fn(|k| p[k] + r[k])
}

/// `seed` mapped into lo..=hi.
fn pick(lo: i64, hi: i64, seed: u64) -> i64 {
    assert!(lo <= hi, "empty range {lo}..={hi}");
    lo + (seed % ((hi - lo) as u64 + 1)) as i64
}

fn floor_div(a: i64, b: i64) -> i64 {
    let q = a / b;
    if a % b != 0 && ((a < 0) != (b < 0)) {
        q - 1
    } else {
        q
    }
}

fn ceil_div(a: i64, b: i64) -> i64 {
    let q = a / b;
    if a % b != 0 && ((a < 0) == (b < 0)) {
        q + 1
    } else {
        q
    }
}

/// The k for which r + k·s stays on the grid (every component within ±M). Contains 0 when r is on
/// the grid; (−2M, 2M) when s is zero.
fn k_range<const D: usize>(r: [i64; D], s: [i64; D]) -> (i64, i64) {
    let (mut lo, mut hi) = (-2 * M, 2 * M);
    for k in 0..D {
        if s[k] == 0 {
            continue;
        }
        // −M − r ≤ k·s ≤ M − r.
        let (a, b) = (-M - r[k], M - r[k]);
        let (l, h) = if s[k] > 0 {
            (ceil_div(a, s[k]), floor_div(b, s[k]))
        } else {
            (ceil_div(b, s[k]), floor_div(a, s[k]))
        };
        lo = lo.max(l);
        hi = hi.min(h);
    }
    (lo, hi)
}

/// Every permutation of 0..N with whether it is odd.
fn permutations<const N: usize>() -> Vec<([usize; N], bool)> {
    let mut idx: [usize; N] = std::array::from_fn(|i| i);
    let mut out = Vec::new();
    permute(&mut idx, 0, &mut out);
    out
}

fn permute<const N: usize>(idx: &mut [usize; N], k: usize, out: &mut Vec<([usize; N], bool)>) {
    if k == N {
        let inversions = (0..N)
            .flat_map(|i| (i + 1..N).map(move |j| (i, j)))
            .filter(|&(i, j)| idx[i] > idx[j])
            .count();
        out.push((*idx, inversions % 2 == 1));
        return;
    }
    for i in k..N {
        idx.swap(k, i);
        permute(idx, k + 1, out);
        idx.swap(k, i);
    }
}

fn apply<T: Copy, const N: usize>(p: [T; N], perm: &[usize; N]) -> [T; N] {
    std::array::from_fn(|i| p[perm[i]])
}

/// Every k-element subset of 0..n, in increasing order.
fn combos(n: usize, k: usize) -> Vec<Vec<usize>> {
    fn go(start: usize, n: usize, k: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if cur.len() == k {
            out.push(cur.clone());
            return;
        }
        for i in start..n {
            cur.push(i);
            go(i + 1, n, k, cur, out);
            cur.pop();
        }
    }
    let mut out = Vec::new();
    go(0, n, k, &mut Vec::new(), &mut out);
    out
}

/// A tiny deterministic generator (`SplitMix64`) for constructions driven by a proptest seed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo) as u64 + 1) as i64
    }
}

// ---------------------------------------------------------------------------------------------
// Oracle 1: Bareiss elimination on the homogeneous matrices.
// ---------------------------------------------------------------------------------------------

/// The exact determinant by Bareiss's fraction-free elimination, with row swaps for zero pivots.
fn bareiss(mut a: Vec<Vec<BigInt>>) -> BigInt {
    let n = a.len();
    let mut negate = false;
    let mut prev = BigInt::one();
    for k in 0..n {
        if a[k][k].is_zero() {
            match (k + 1..n).find(|&i| !a[i][k].is_zero()) {
                Some(i) => {
                    a.swap(k, i);
                    negate = !negate;
                }
                None => return BigInt::zero(),
            }
        }
        for i in k + 1..n {
            for j in k + 1..n {
                let v = (&a[i][j] * &a[k][k] - &a[i][k] * &a[k][j]) / &prev;
                a[i][j] = v;
            }
        }
        prev.clone_from(&a[k][k]);
    }
    if negate { -prev } else { prev }
}

/// The homogeneous row [p, |p|², 1] (with the lift) or [p, 1].
fn hom_row<const D: usize>(p: [i64; D], lift: bool) -> Vec<BigInt> {
    let mut r: Vec<BigInt> = p.iter().map(|&v| big(v)).collect();
    if lift {
        r.push(p.iter().map(|&v| big(v) * big(v)).sum::<BigInt>());
    }
    r.push(BigInt::one());
    r
}

/// orient2d's value: det[a 1; b 1; c 1] = det[b − a; c − a].
fn orient2d_det(p: [Pt2; 3]) -> BigInt {
    bareiss(p.iter().map(|&q| hom_row(q, false)).collect())
}

/// orient3d's value: −det[a 1; b 1; c 1; d 1] = det[b − a; c − a; d − a].
fn orient3d_det(p: [Pt3; 4]) -> BigInt {
    -bareiss(p.iter().map(|&q| hom_row(q, false)).collect())
}

/// incircle's value: det[a |a|² 1; …; d |d|² 1].
fn incircle_det(p: [Pt2; 4]) -> BigInt {
    bareiss(p.iter().map(|&q| hom_row(q, true)).collect())
}

/// insphere's value: −det[a |a|² 1; …; e |e|² 1].
fn insphere_det(p: [Pt3; 5]) -> BigInt {
    -bareiss(p.iter().map(|&q| hom_row(q, true)).collect())
}

// ---------------------------------------------------------------------------------------------
// Oracle 2: Shewchuk's predicates.c formulas, evaluated exactly.
// ---------------------------------------------------------------------------------------------

/// `orient2dfast`: positive when a, b, c are counter-clockwise.
fn sh_orient2d([pa, pb, pc]: [Pt2; 3]) -> BigInt {
    let [acx, acy] = diff(pa, pc);
    let [bcx, bcy] = diff(pb, pc);
    &acx * &bcy - &acy * &bcx
}

/// `orient3dfast`: positive when d lies below the plane of a, b, c, "below" meaning a, b, c are
/// counter-clockwise seen from above.
fn sh_orient3d([pa, pb, pc, pd]: [Pt3; 4]) -> BigInt {
    let [adx, ady, adz] = diff(pa, pd);
    let [bdx, bdy, bdz] = diff(pb, pd);
    let [cdx, cdy, cdz] = diff(pc, pd);
    &adx * (&bdy * &cdz - &bdz * &cdy)
        + &bdx * (&cdy * &adz - &cdz * &ady)
        + &cdx * (&ady * &bdz - &adz * &bdy)
}

/// `incirclefast`: positive when d is inside the circle through counter-clockwise a, b, c.
fn sh_incircle([pa, pb, pc, pd]: [Pt2; 4]) -> BigInt {
    let [adx, ady] = diff(pa, pd);
    let [bdx, bdy] = diff(pb, pd);
    let [cdx, cdy] = diff(pc, pd);
    let abdet = &adx * &bdy - &bdx * &ady;
    let bcdet = &bdx * &cdy - &cdx * &bdy;
    let cadet = &cdx * &ady - &adx * &cdy;
    let alift = &adx * &adx + &ady * &ady;
    let blift = &bdx * &bdx + &bdy * &bdy;
    let clift = &cdx * &cdx + &cdy * &cdy;
    alift * bcdet + blift * cadet + clift * abdet
}

/// `inspherefast`: positive when e is inside the sphere through a, b, c, d and Shewchuk's
/// orient3d(a, b, c, d) is positive.
fn sh_insphere([pa, pb, pc, pd, pe]: [Pt3; 5]) -> BigInt {
    let [aex, aey, aez] = diff(pa, pe);
    let [bex, bey, bez] = diff(pb, pe);
    let [cex, cey, cez] = diff(pc, pe);
    let [dex, dey, dez] = diff(pd, pe);
    let ab = &aex * &bey - &bex * &aey;
    let bc = &bex * &cey - &cex * &bey;
    let cd = &cex * &dey - &dex * &cey;
    let da = &dex * &aey - &aex * &dey;
    let ac = &aex * &cey - &cex * &aey;
    let bd = &bex * &dey - &dex * &bey;
    let abc = &aez * &bc - &bez * &ac + &cez * &ab;
    let bcd = &bez * &cd - &cez * &bd + &dez * &bc;
    let cda = &cez * &da + &dez * &ac + &aez * &cd;
    let dab = &dez * &ab + &aez * &bd + &bez * &da;
    let alift = &aex * &aex + &aey * &aey + &aez * &aez;
    let blift = &bex * &bex + &bey * &bey + &bez * &bez;
    let clift = &cex * &cex + &cey * &cey + &cez * &cez;
    let dlift = &dex * &dex + &dey * &dey + &dez * &dez;
    (dlift * abc - clift * dab) + (blift * cda - alift * bcd)
}

// ---------------------------------------------------------------------------------------------
// Oracle 3: geometry (exact circumcentres, box circumspheres).
// ---------------------------------------------------------------------------------------------

fn cross2(u: &[BigInt; 2], v: &[BigInt; 2]) -> BigInt {
    &u[0] * &v[1] - &u[1] * &v[0]
}

fn det3b(m: &[[BigInt; 3]; 3]) -> BigInt {
    &m[0][0] * (&m[1][1] * &m[2][2] - &m[1][2] * &m[2][1])
        - &m[0][1] * (&m[1][0] * &m[2][2] - &m[1][2] * &m[2][0])
        + &m[0][2] * (&m[1][0] * &m[2][1] - &m[1][1] * &m[2][0])
}

/// r² − |d − o|² for the circle through a, b, c, which must not be collinear: positive exactly when
/// d is strictly inside. The centre comes from solving 2(b − a)·o′ = |b − a|², 2(c − a)·o′ =
/// |c − a|² for o′ = o − a by Cramer's rule, in rationals.
fn circle_power([a, b, c, d]: [Pt2; 4]) -> BigRational {
    let (u, v) = (diff(b, a), diff(c, a));
    let ru = &u[0] * &u[0] + &u[1] * &u[1];
    let rv = &v[0] * &v[0] + &v[1] * &v[1];
    let den = BigInt::from(2) * cross2(&u, &v);
    assert!(!den.is_zero(), "circle_power needs a, b, c not collinear");
    let o = [
        BigRational::new(&ru * &v[1] - &rv * &u[1], den.clone()),
        BigRational::new(&u[0] * &rv - &v[0] * &ru, den),
    ];
    let w = diff(d, a).map(BigRational::from_integer);
    let r2 = &o[0] * &o[0] + &o[1] * &o[1];
    let (ex, ey) = (&w[0] - &o[0], &w[1] - &o[1]);
    r2 - (&ex * &ex + &ey * &ey)
}

/// r² − |e − o|² for the sphere through a, b, c, d, which must not be coplanar.
fn sphere_power([a, b, c, d, e]: [Pt3; 5]) -> BigRational {
    let rows = [diff(b, a), diff(c, a), diff(d, a)];
    let r: [BigInt; 3] = std::array::from_fn(|i| rows[i].iter().map(|x| x * x).sum::<BigInt>());
    let den = BigInt::from(2) * det3b(&rows);
    assert!(!den.is_zero(), "sphere_power needs a, b, c, d not coplanar");
    let o: [BigRational; 3] = std::array::from_fn(|k| {
        let mut m = rows.clone();
        for i in 0..3 {
            m[i][k].clone_from(&r[i]);
        }
        BigRational::new(det3b(&m), den.clone())
    });
    let w = diff(e, a).map(BigRational::from_integer);
    let r2: BigRational = o.iter().map(|x| x * x).sum();
    let dist2: BigRational = (0..3)
        .map(|k| {
            let t = &w[k] - &o[k];
            &t * &t
        })
        .sum();
    r2 - dist2
}

/// The geometric meaning of incircle: orient2d's determinant times the power of d. `None` when
/// a, b, c are collinear.
fn incircle_by_geometry(p: [Pt2; 4]) -> Option<BigRational> {
    let orient = cross2(&diff(p[1], p[0]), &diff(p[2], p[0]));
    if orient.is_zero() {
        return None;
    }
    Some(BigRational::from_integer(orient) * circle_power(p))
}

/// The geometric meaning of insphere: orient3d's determinant ((b − a) × (c − a)) · (d − a) times
/// the power of e. `None` when a, b, c, d are coplanar.
fn insphere_by_geometry(p: [Pt3; 5]) -> Option<BigRational> {
    let orient = det3b(&[diff(p[1], p[0]), diff(p[2], p[0]), diff(p[3], p[0])]);
    if orient.is_zero() {
        return None;
    }
    Some(BigRational::from_integer(orient) * sphere_power(p))
}

/// A corner of an axis-aligned box: bit `axis` of `k` picks the low or high side.
fn corner<const D: usize>(bx: &[(i64, i64); D], k: usize) -> [i64; D] {
    std::array::from_fn(|axis| {
        if (k >> axis) & 1 == 0 {
            bx[axis].0
        } else {
            bx[axis].1
        }
    })
}

/// 4 (r² − |p − o|²) for the sphere (circle in 2D) through every corner of the box: o is the box's
/// centre and r its half-diagonal. Exact in `i128` (each square is below 2^85).
fn box_power4<const D: usize>(bx: &[(i64, i64); D], p: [i64; D]) -> i128 {
    (0..D)
        .map(|axis| {
            let (lo, hi) = (i128::from(bx[axis].0), i128::from(bx[axis].1));
            let t = 2 * i128::from(p[axis]) - lo - hi;
            (hi - lo) * (hi - lo) - t * t
        })
        .sum()
}

/// Grid points within a step or two of the box's circumsphere, on the line through `head` along
/// the last axis (whose coordinate in `head` is ignored), with every coordinate within ±limit.
fn near_sphere<const D: usize>(bx: &[(i64, i64); D], head: [i64; D], limit: i64) -> Vec<[i64; D]> {
    let last = D - 1;
    let mut rest: i128 = 0;
    for axis in 0..D {
        let (lo, hi) = (i128::from(bx[axis].0), i128::from(bx[axis].1));
        rest += (hi - lo) * (hi - lo);
        if axis != last {
            let t = 2 * i128::from(head[axis]) - lo - hi;
            rest -= t * t;
        }
    }
    let mut out = Vec::new();
    if rest < 0 {
        return out;
    }
    let root = num_integer::sqrt(rest as u128) as i128;
    let (lo, hi) = (i128::from(bx[last].0), i128::from(bx[last].1));
    for t in (-2..=2).flat_map(|off| [root + off, off - root]) {
        let s = t + lo + hi;
        if s.rem_euclid(2) != 0 {
            continue;
        }
        let x = s / 2;
        if (-i128::from(limit)..=i128::from(limit)).contains(&x) {
            let mut p = head;
            p[last] = x as i64;
            out.push(p);
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Checked calls: the predicate against both determinant oracles, and the documented bounds.
// ---------------------------------------------------------------------------------------------

fn check_o2(p: [Pt2; 3]) -> BigInt {
    let v = orient2d_det(p);
    assert_eq!(
        sh_orient2d(p),
        v,
        "test oracles disagree (orient2d) at {p:?}"
    );
    assert_eq!(o2(p), sign_of(&v), "orient2d {p:?}: exact value {v}");
    // The documented largest value, 4M² (half the bounding square).
    assert!(
        v.abs() <= BigInt::from(4) * big(M).pow(2),
        "orient2d {p:?} exceeds the documented maximum 4M²: {v}"
    );
    v
}

fn check_o3(p: [Pt3; 4]) -> BigInt {
    let v = orient3d_det(p);
    assert_eq!(
        -sh_orient3d(p),
        v,
        "test oracles disagree (orient3d) at {p:?}"
    );
    assert_eq!(o3(p), sign_of(&v), "orient3d {p:?}: exact value {v}");
    // The documented largest value, 16M³ (the largest tetrahedron in the cube).
    assert!(
        v.abs() <= BigInt::from(16) * big(M).pow(3),
        "orient3d {p:?} exceeds the documented maximum 16M³: {v}"
    );
    v
}

fn check_ic(p: [Pt2; 4]) -> BigInt {
    let v = incircle_det(p);
    assert_eq!(
        sh_incircle(p),
        v,
        "test oracles disagree (incircle) at {p:?}"
    );
    assert_eq!(ic(p), sign_of(&v), "incircle {p:?}: exact value {v}");
    assert!(
        v.abs() < pow2(168),
        "incircle {p:?} exceeds the documented budget 2^168: {v}"
    );
    v
}

fn check_ins(p: [Pt3; 5]) -> BigInt {
    let v = insphere_det(p);
    assert_eq!(
        -sh_insphere(p),
        v,
        "test oracles disagree (insphere) at {p:?}"
    );
    assert_eq!(ins(p), sign_of(&v), "insphere {p:?}: exact value {v}");
    assert!(
        v.abs() < pow2(212),
        "insphere {p:?} exceeds the documented budget 2^212: {v}"
    );
    v
}

// ---------------------------------------------------------------------------------------------
// Strategies.
// ---------------------------------------------------------------------------------------------

/// A coordinate: anywhere, near zero, at or next to an edge, or at a power-of-two boundary.
/// Boxed, like every leaf strategy here that arrays of points repeat: unboxed, a proptest over
/// several point sets needs more than a test thread's 2 MiB stack in debug builds.
fn coord() -> BoxedStrategy<i64> {
    prop_oneof![
        4 => -M..=M,
        2 => -3i64..=3,
        1 => (0i64..3).prop_map(|k| M - k),
        1 => (0i64..3).prop_map(|k| k - M),
        1 => prop_oneof![
            Just(1i64 << 39),
            Just(-(1i64 << 39)),
            Just((1i64 << 39) - 1),
            Just(1 - (1i64 << 39)),
        ],
    ]
    .boxed()
}

fn pt2() -> impl Strategy<Value = Pt2> {
    uniform::<_, 2>(coord())
}

fn pt3() -> impl Strategy<Value = Pt3> {
    uniform::<_, 3>(coord())
}

/// N points: anywhere on the grid, clustered within a few steps of a random point (clamped to the
/// grid), or clustered at the origin. Boxed, like `coord`.
fn config<const D: usize, const N: usize>() -> BoxedStrategy<[[i64; D]; N]> {
    prop_oneof![
        3 => uniform::<_, N>(uniform::<_, D>(coord())),
        2 => (uniform::<_, D>(coord()), uniform::<_, N>(uniform::<_, D>(-6i64..=6)))
            .prop_map(|(base, offs)| {
                offs.map(|o| std::array::from_fn(|k| (base[k] + o[k]).clamp(-M, M)))
            }),
        1 => uniform::<_, N>(uniform::<_, D>(-6i64..=6)),
    ]
    .boxed()
}

/// One axis of a box: any interval, a flat one, a one-to-three-step one, or the whole grid.
fn axis() -> impl Strategy<Value = (i64, i64)> {
    prop_oneof![
        6 => (coord(), coord()).prop_map(|(a, b)| (a.min(b), a.max(b))),
        1 => coord().prop_map(|v| (v, v)),
        2 => (coord(), 1i64..=3)
            .prop_map(|(v, w)| if v <= M - w { (v, v + w) } else { (v - w, v) }),
        1 => Just((-M, M)),
    ]
}

/// A direction for a line: tiny, middling, spanning the grid, or consecutive Fibonacci numbers.
fn direction<const D: usize>() -> impl Strategy<Value = [i64; D]> {
    prop_oneof![
        uniform::<_, D>(-3i64..=3),
        uniform::<_, D>(-(1i64 << 20)..=(1i64 << 20)),
        uniform::<_, D>(-(1i64 << 39)..=(1i64 << 39)),
        Just(std::array::from_fn(|k| {
            [956_722_026_041i64, 591_286_729_879, 365_435_296_162][k % 3]
        })),
    ]
}

// ---------------------------------------------------------------------------------------------
// Conventions against predicates.c.
// ---------------------------------------------------------------------------------------------

/// The port obeys the geometry Shewchuk documents for it, so the convention tests below rest on
/// predicates.c itself and not on a port that might be wrong.
#[test]
fn shewchuk_port_obeys_shewchuks_own_documentation() {
    // orient2d: positive when counter-clockwise.
    assert!(sh_orient2d([[0, 0], [1, 0], [0, 1]]).is_positive());
    // orient3d: positive when d lies below the plane, a, b, c counter-clockwise seen from above.
    let (a, b, c) = ([0, 0, 0], [1, 0, 0], [0, 1, 0]);
    assert!(sh_orient3d([a, b, c, [0, 0, -1]]).is_positive());
    assert!(sh_orient3d([a, b, c, [0, 0, 1]]).is_negative());
    // incircle: positive when d is inside the circle through counter-clockwise a, b, c.
    assert!(sh_incircle([[0, 0], [2, 0], [0, 2], [1, 1]]).is_positive());
    // insphere: positive when e is inside and Shewchuk's orient3d(a, b, c, d) is positive.
    let t = [[0, 0, 0], [2, 0, 0], [0, 2, 0], [0, 0, -2]];
    assert!(sh_orient3d(t).is_positive());
    assert!(sh_insphere([t[0], t[1], t[2], t[3], [1, 1, -1]]).is_positive());
    assert!(sh_insphere([t[0], t[1], t[2], t[3], [5, 5, 5]]).is_negative());
}

/// The module documents the same signs as predicates.c for orient2d and incircle and the opposite
/// for orient3d and insphere. Checked on the same geometric cases.
#[test]
fn conventions_match_the_documented_relation_to_predicates_c() {
    let tri = [[0, 0], [1, 0], [0, 1]];
    assert_eq!(o2(tri), sign_of(&sh_orient2d(tri)));
    let base = [[0, 0, 0], [1, 0, 0], [0, 1, 0]];
    for d in [[0, 0, -1], [0, 0, 1], [7, -3, 5]] {
        let p = [base[0], base[1], base[2], d];
        assert_eq!(o3(p), sign_of(&sh_orient3d(p)).negate(), "orient3d {p:?}");
    }
    let q = [[0, 0], [2, 0], [0, 2], [1, 1]];
    assert_eq!(ic(q), sign_of(&sh_incircle(q)));
    let t = [[0, 0, 0], [2, 0, 0], [0, 2, 0], [0, 0, -2]];
    for e in [[1, 1, -1], [5, 5, 5], [2, 2, 0]] {
        let p = [t[0], t[1], t[2], t[3], e];
        assert_eq!(ins(p), sign_of(&sh_insphere(p)).negate(), "insphere {p:?}");
    }
    // The design's own wording: orient3d positive when d is on the side (b − a) × (c − a) points
    // to.
    assert_eq!(o3([base[0], base[1], base[2], [0, 0, 1]]), Sign::Positive);
}

proptest! {
    #![proptest_config(cfg(512))]

    /// Random and clustered points against Bareiss and against predicates.c (inside the check_*
    /// helpers, which also enforce the documented maxima and budgets).
    #[test]
    fn random_points_against_bareiss_and_predicates_c(
        a in config::<2, 3>(), b in config::<3, 4>(), c in config::<2, 4>(), d in config::<3, 5>(),
    ) {
        check_o2(a);
        check_o3(b);
        check_ic(c);
        check_ins(d);
    }
}

// ---------------------------------------------------------------------------------------------
// Geometry: the documented meaning of incircle and insphere, exactly.
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(cfg(384))]

    /// The doc's identity: incircle's determinant is orient2d's determinant times r² − |d − o|².
    #[test]
    fn incircle_is_orientation_times_the_power_of_d(p in config::<2, 4>()) {
        let Some(value) = incircle_by_geometry(p) else { return Ok(()) };
        let got = BigRational::from_integer(check_ic(p));
        prop_assert_eq!(got, value.clone(), "documented identity fails at {:?}", p);
        prop_assert_eq!(ic(p), sign_of_rat(&value));
    }

    /// The doc's identity: insphere's determinant is orient3d's determinant times r² − |e − o|².
    #[test]
    fn insphere_is_orientation_times_the_power_of_e(p in config::<3, 5>()) {
        let Some(value) = insphere_by_geometry(p) else { return Ok(()) };
        let got = BigRational::from_integer(check_ins(p));
        prop_assert_eq!(got, value.clone(), "documented identity fails at {:?}", p);
        prop_assert_eq!(ins(p), sign_of_rat(&value));
    }

    /// Three corners of any axis-aligned rectangle (thin, flat, the whole grid) and d anywhere,
    /// at a corner, or within a step or two of the circle: the answer is orient2d times the side
    /// of the circle, decided in integers with no determinant.
    #[test]
    fn rectangle_circles_decide_incircle_at_full_scale(
        bx in uniform::<_, 2>(axis()), abc in uniform::<_, 3>(0usize..4), free in pt2(),
        x in coord(), k in 0usize..4,
    ) {
        let [a, b, c] = abc.map(|i| corner(&bx, i));
        let orient = check_o2([a, b, c]);
        let mut ds = vec![free, corner(&bx, k)];
        ds.extend(near_sphere(&bx, [x, 0], M));
        for d in ds {
            let p = [a, b, c, d];
            let expected = sign_of(&orient).mul(Sign::of_i128(box_power4(&bx, d)));
            prop_assert_eq!(ic(p), expected, "box {:?}, points {:?}", bx, p);
            check_ic(p);
        }
    }

    /// Four corners of any axis-aligned box and e anywhere, at a corner, or within a step or two of
    /// the sphere. Coplanar corners (faces, diagonal sections, flat boxes) lie on one circle, so
    /// the doc says every e gives zero, which is what orient3d × side gives too.
    #[test]
    fn box_spheres_decide_insphere_at_full_scale(
        bx in uniform::<_, 3>(axis()), abcd in uniform::<_, 4>(0usize..8), free in pt3(),
        xy in uniform::<_, 2>(coord()), k in 0usize..8,
    ) {
        let [a, b, c, d] = abcd.map(|i| corner(&bx, i));
        let orient = check_o3([a, b, c, d]);
        let mut es = vec![free, corner(&bx, k)];
        es.extend(near_sphere(&bx, [xy[0], xy[1], 0], M));
        for e in es {
            let p = [a, b, c, d, e];
            let expected = sign_of(&orient).mul(Sign::of_i128(box_power4(&bx, e)));
            prop_assert_eq!(ins(p), expected, "box {:?}, points {:?}", bx, p);
            check_ins(p);
        }
    }
}

/// The grid's own corners: every other grid point is strictly inside their circle (sphere), the
/// corners are on it. Every ordered choice of distinct corners, with the fourth (fifth) point at a
/// corner or one step inward along any set of axes: the largest terms cancel almost exactly.
#[test]
fn grid_corners_with_a_point_on_or_one_step_inside_their_sphere() {
    let full2 = [(-M, M); 2];
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                if a == b || b == c || a == c {
                    continue;
                }
                let tri = [corner(&full2, a), corner(&full2, b), corner(&full2, c)];
                let orient = o2(tri);
                assert_ne!(orient, Sign::Zero);
                for k in 0..4 {
                    for mv in 0..4usize {
                        let mut d = corner(&full2, k);
                        for axis in 0..2 {
                            if (mv >> axis) & 1 == 1 {
                                d[axis] -= d[axis].signum();
                            }
                        }
                        let p = [tri[0], tri[1], tri[2], d];
                        assert_eq!(ic(p), if mv == 0 { Sign::Zero } else { orient }, "{p:?}");
                        if a < b && b < c {
                            check_ic(p);
                        }
                    }
                }
            }
        }
    }
    let full3 = [(-M, M); 3];
    let mut nonzero = 0;
    for idx in combos(8, 4) {
        for (perm, _) in permutations::<4>() {
            let pts = apply([idx[0], idx[1], idx[2], idx[3]], &perm).map(|i| corner(&full3, i));
            let orient = o3(pts);
            if orient != Sign::Zero {
                nonzero += 1;
            }
            for k in 0..8 {
                for mv in 0..8usize {
                    let mut e = corner(&full3, k);
                    for axis in 0..3 {
                        if (mv >> axis) & 1 == 1 {
                            e[axis] -= e[axis].signum();
                        }
                    }
                    let p = [pts[0], pts[1], pts[2], pts[3], e];
                    // Coplanar corners lie on one circle (a rectangle): zero for every e.
                    let expected = if mv == 0 { Sign::Zero } else { orient };
                    assert_eq!(ins(p), expected, "{p:?}");
                    if perm == [0, 1, 2, 3] {
                        check_ins(p);
                    }
                }
            }
        }
    }
    // 58 of the 70 four-corner sets are not coplanar, each in 24 orders.
    assert_eq!(nonzero, 58 * 24);
}

// ---------------------------------------------------------------------------------------------
// Unimodular slivers: determinant exactly ±1 with entries near the whole grid.
// ---------------------------------------------------------------------------------------------

/// A D × D integer matrix with determinant −1 (if `negative`) or +1 and every entry within ±M,
/// grown from the identity by random row additions, each of which keeps the determinant.
fn unimodular<const D: usize>(rng: &mut Rng, negative: bool) -> [[i64; D]; D] {
    let mut m = [[0i64; D]; D];
    for i in 0..D {
        m[i][i] = 1;
    }
    if negative {
        m[0][0] = -1;
    }
    for _ in 0..80 * D {
        let i = rng.below(D as u64) as usize;
        let j = (i + 1 + rng.below(D as u64 - 1) as usize) % D;
        let (lo, hi) = k_range(m[i], m[j]);
        let k = match rng.below(4) {
            0 => lo,
            1 => hi,
            _ => rng.range(lo, hi),
        };
        for c in 0..D {
            m[i][c] += k * m[j][c];
        }
    }
    // The random steps alone can end small (seed 6214692301197557477 ended at
    // [[0, 1], [-1, -789502539]]). One last step by the largest multiple the grid allows always
    // reaches an entry above M/2: if that multiple k is 0, some axis c has
    // |m[i][c]| + |m[j][c]| > M; otherwise k ± 1 would leave the grid on some axis c, so the new
    // |m[i][c]| ≥ M − |m[j][c]|. Either way one of the two entries exceeds M/2 > 2^30.
    let i = rng.below(D as u64) as usize;
    let j = (i + 1 + rng.below(D as u64 - 1) as usize) % D;
    let (lo, hi) = k_range(m[i], m[j]);
    let k = if hi >= -lo { hi } else { lo };
    for c in 0..D {
        m[i][c] += k * m[j][c];
    }
    m
}

/// The generator reaches entries above 2^30 for the seed that once stayed small on CI, and for
/// every seed in a fixed sweep, in 2D and 3D, with both determinant signs.
#[test]
fn unimodular_generator_always_reaches_the_grid() {
    let largest = |rows: &[i64]| rows.iter().map(num_traits::Signed::abs).max().unwrap();
    let m = unimodular::<2>(&mut Rng(6_214_692_301_197_557_477), false);
    assert!(largest(m.as_flattened()) > 1 << 30, "{m:?}");
    for seed in 0..20_000u64 {
        for negative in [false, true] {
            let m2 = unimodular::<2>(&mut Rng(seed), negative);
            assert!(largest(m2.as_flattened()) > 1 << 30, "seed {seed}: {m2:?}");
            let m3 = unimodular::<3>(&mut Rng(seed), negative);
            assert!(largest(m3.as_flattened()) > 1 << 30, "seed {seed}: {m3:?}");
        }
    }
}

/// A base point a such that a + r stays on the grid for every row r (each axis's rows span at most
/// 2M because every entry is within ±M).
fn place<const D: usize>(rng: &mut Rng, rows: &[[i64; D]; D]) -> [i64; D] {
    std::array::from_fn(|k| {
        let lo = -M - rows.iter().map(|r| r[k]).min().unwrap().min(0);
        let hi = M - rows.iter().map(|r| r[k]).max().unwrap().max(0);
        match rng.below(3) {
            0 => lo,
            1 => hi,
            _ => rng.range(lo, hi),
        }
    })
}

#[test]
fn fibonacci_slivers_at_full_scale() {
    let mut f = vec![0i64, 1];
    while f.len() < 62 {
        let n = f[f.len() - 1] + f[f.len() - 2];
        f.push(n);
    }
    let (f58, f59, f60) = (f[58], f[59], f[60]);
    // F60 is the largest Fibonacci number whose spread fits the grid (2M).
    assert!(f60 <= 2 * M && f[61] > 2 * M);
    // Cassini: F60·F58 − F59² = (−1)^59 = −1, with products near 2^81.
    let p = [[-M, -M], [-M + f60, -M + f59], [-M + f59, -M + f58]];
    assert_eq!(check_o2(p), big(-1));
    for (perm, odd) in permutations::<3>() {
        assert_eq!(o2(apply(p, &perm)), odd_if(Sign::Negative, odd));
        let mirrored = apply(p, &perm).map(|[x, y]| [-x, y]);
        assert_eq!(o2(mirrored), odd_if(Sign::Positive, odd));
    }
    // In 3D: the same triangle in the plane z = −M and d one step above it, far across the grid.
    let q = [
        [-M, -M, -M],
        [-M + f60, -M + f59, -M],
        [-M + f59, -M + f58, -M],
        [M, M, 1 - M],
    ];
    assert_eq!(check_o3(q), big(-1));
    for (perm, odd) in permutations::<4>() {
        assert_eq!(o3(apply(q, &perm)), odd_if(Sign::Negative, odd));
    }
    // incircle and insphere on these slivers, with the fourth (fifth) point at extremes.
    for d in [[M, M], [-M, M], [0, 0], [M, -M], [-M + f59 + 1, -M + f58]] {
        let r = [p[0], p[1], p[2], d];
        let v = check_ic(r);
        assert_eq!(
            BigRational::from_integer(v),
            incircle_by_geometry(r).unwrap()
        );
    }
    for e in [
        [M, M, M],
        [-M, M, -M],
        [0, 0, 0],
        [M, -M, 1 - M],
        [-M + f59, -M + f58, M],
    ] {
        let r = [q[0], q[1], q[2], q[3], e];
        let v = check_ins(r);
        assert_eq!(
            BigRational::from_integer(v),
            insphere_by_geometry(r).unwrap()
        );
    }
}

proptest! {
    #![proptest_config(cfg(384))]

    /// Triangles of orient2d determinant exactly ±1 whose edge vectors have entries near M: the
    /// thinnest triangles the grid allows, with products near 2^80.
    #[test]
    fn unimodular_triangles_have_orient2d_plus_or_minus_one(
        seed in any::<u64>(), negative in any::<bool>(), d in pt2(),
    ) {
        let mut rng = Rng(seed);
        let m = unimodular::<2>(&mut rng, negative);
        let largest = m.iter().flatten().map(num_traits::Signed::abs).max().unwrap();
        prop_assert!(largest > 1 << 30, "the generator did not grow the matrix: {:?}", m);
        let a = place(&mut rng, &m);
        let tri = [a, add(a, m[0]), add(a, m[1])];
        let unit = if negative { -1 } else { 1 };
        prop_assert_eq!(check_o2(tri), big(unit), "{:?}", tri);
        for (perm, odd) in permutations::<3>() {
            prop_assert_eq!(o2(apply(tri, &perm)), odd_if(Sign::of_i64(unit), odd));
        }
        let p = [tri[0], tri[1], tri[2], d];
        let value = incircle_by_geometry(p).unwrap();
        prop_assert_eq!(BigRational::from_integer(check_ic(p)), value.clone());
        prop_assert_eq!(ic(p), sign_of_rat(&value));
    }

    /// Tetrahedra of orient3d determinant exactly ±1 with entries near M: products near 2^123
    /// cancelling to 1.
    #[test]
    fn unimodular_tetrahedra_have_orient3d_plus_or_minus_one(
        seed in any::<u64>(), negative in any::<bool>(), e in pt3(),
    ) {
        let mut rng = Rng(seed);
        let m = unimodular::<3>(&mut rng, negative);
        let largest = m.iter().flatten().map(num_traits::Signed::abs).max().unwrap();
        prop_assert!(largest > 1 << 30, "the generator did not grow the matrix: {:?}", m);
        let a = place(&mut rng, &m);
        let tet = [a, add(a, m[0]), add(a, m[1]), add(a, m[2])];
        let unit = if negative { -1 } else { 1 };
        prop_assert_eq!(check_o3(tet), big(unit), "{:?}", tet);
        for (perm, odd) in permutations::<4>() {
            prop_assert_eq!(o3(apply(tet, &perm)), odd_if(Sign::of_i64(unit), odd));
        }
        let p = [tet[0], tet[1], tet[2], tet[3], e];
        let value = insphere_by_geometry(p).unwrap();
        prop_assert_eq!(BigRational::from_integer(check_ins(p)), value.clone());
        prop_assert_eq!(ins(p), sign_of_rat(&value));
    }
}

// ---------------------------------------------------------------------------------------------
// Exhaustive small grids, scaled to the full grid.
// ---------------------------------------------------------------------------------------------

/// Every set of distinct points from {−2..2}² (2D) and {−1, 0, 1}³ (3D), its sign decided once at
/// small scale, then evaluated by the predicate scaled up to the whole grid (and shifted, and
/// point-reflected) in one of its orders. A similarity keeps every answer, and a point reflection
/// is a rotation in 2D but flips orientation in 3D. tests/predicates.rs sweeps only nine of the 27
/// points of {−M, 0, M}³ for insphere.
#[test]
fn small_grid_configurations_keep_their_signs_scaled_to_the_full_grid() {
    let s2 = (M - 1) / 2; // 2·s2 + 1 = M.
    let grid2: Vec<Pt2> = (-2i64..=2)
        .flat_map(|x| (-2i64..=2).map(move |y| [x, y]))
        .collect();
    let shifts2: [Pt2; 3] = [[0, 0], [1, -1], [-1, 1]];
    let perms3 = permutations::<3>();
    for (n, set) in combos(grid2.len(), 3).iter().enumerate() {
        let small: [Pt2; 3] = std::array::from_fn(|i| grid2[set[i]]);
        let s0 = sign_of(&orient2d_det(small));
        let (perm, odd) = &perms3[n % perms3.len()];
        let q = apply(small, perm);
        for t in shifts2 {
            for r in [1, -1] {
                let scaled = q.map(|p| [r * s2 * p[0] + t[0], r * s2 * p[1] + t[1]]);
                assert_eq!(
                    o2(scaled),
                    odd_if(s0, *odd),
                    "orient2d {small:?} scaled to {scaled:?}"
                );
            }
        }
    }
    let perms4 = permutations::<4>();
    for (n, set) in combos(grid2.len(), 4).iter().enumerate() {
        let small: [Pt2; 4] = std::array::from_fn(|i| grid2[set[i]]);
        let s0 = sign_of(&incircle_det(small));
        let (perm, odd) = &perms4[n % perms4.len()];
        let q = apply(small, perm);
        for t in shifts2 {
            for r in [1, -1] {
                let scaled = q.map(|p| [r * s2 * p[0] + t[0], r * s2 * p[1] + t[1]]);
                assert_eq!(
                    ic(scaled),
                    odd_if(s0, *odd),
                    "incircle {small:?} scaled to {scaled:?}"
                );
            }
        }
    }
    let mut grid3: Vec<Pt3> = Vec::new();
    for x in -1i64..=1 {
        for y in -1i64..=1 {
            for z in -1i64..=1 {
                grid3.push([x, y, z]);
            }
        }
    }
    let (h, g) = ((M - 1) / 2, (M + 1) / 2); // h + g = M.
    let maps: [(i64, Pt3); 4] = [
        (M, [0, 0, 0]),
        (h, [g, g, g]),
        (h, [-g, g, -g]),
        (h, [g, -g, -g]),
    ];
    for (n, set) in combos(grid3.len(), 4).iter().enumerate() {
        let small: [Pt3; 4] = std::array::from_fn(|i| grid3[set[i]]);
        let s0 = sign_of(&orient3d_det(small));
        let (perm, odd) = &perms4[n % perms4.len()];
        let q = apply(small, perm);
        for (s, t) in maps {
            for r in [1, -1] {
                let scaled = q.map(|p| std::array::from_fn(|k| r * s * p[k] + t[k]));
                let expected = odd_if(odd_if(s0, *odd), r < 0);
                assert_eq!(
                    o3(scaled),
                    expected,
                    "orient3d {small:?} scaled to {scaled:?}"
                );
            }
        }
    }
    let perms5 = permutations::<5>();
    for (n, set) in combos(grid3.len(), 5).iter().enumerate() {
        let small: [Pt3; 5] = std::array::from_fn(|i| grid3[set[i]]);
        let s0 = sign_of(&insphere_det(small));
        let (perm, odd) = &perms5[n % perms5.len()];
        let q = apply(small, perm);
        for (s, t) in maps {
            for r in [1, -1] {
                let scaled = q.map(|p| std::array::from_fn(|k| r * s * p[k] + t[k]));
                let expected = odd_if(odd_if(s0, *odd), r < 0);
                assert_eq!(
                    ins(scaled),
                    expected,
                    "insphere {small:?} scaled to {scaled:?}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Symmetries.
// ---------------------------------------------------------------------------------------------

/// Every permutation (sign of the permutation); the point reflection p → −p (orientation (−1)^D);
/// a quarter turn in the first two axes (kept); a cyclic shift of the axes (orientation
/// (−1)^(D−1)); scaling by an integer that keeps the points on the grid (kept); and translation
/// that pushes the configuration against each corner of the grid (kept).
fn check_all_laws<const D: usize, const N: usize>(
    f: fn([[i64; D]; N]) -> Sign,
    p: [[i64; D]; N],
    s: Sign,
    k: i64,
) {
    for (perm, odd) in permutations::<N>() {
        assert_eq!(
            f(apply(p, &perm)),
            odd_if(s, odd),
            "permutation {perm:?} of {p:?}"
        );
    }
    let reflected = p.map(|q| q.map(|v| -v));
    assert_eq!(
        f(reflected),
        odd_if(s, D % 2 == 1),
        "point reflection of {p:?}"
    );
    let turned = p.map(|q| {
        let mut r = q;
        r[0] = -q[1];
        r[1] = q[0];
        r
    });
    assert_eq!(f(turned), s, "quarter turn of {p:?}");
    let cycled: [[i64; D]; N] = p.map(|q| std::array::from_fn(|axis| q[(axis + 1) % D]));
    assert_eq!(
        f(cycled),
        odd_if(s, D % 2 == 0),
        "cyclic axis shift of {p:?}"
    );
    let largest = p
        .iter()
        .flatten()
        .map(num_traits::Signed::abs)
        .max()
        .unwrap();
    if largest > 0 {
        let k = k.min(M / largest);
        if k >= 2 {
            assert_eq!(f(p.map(|q| q.map(|v| v * k))), s, "scaling {p:?} by {k}");
        }
    }
    for side in 0..1usize << D {
        let t: [i64; D] = std::array::from_fn(|axis| {
            let lo = p.iter().map(|q| q[axis]).min().unwrap();
            let hi = p.iter().map(|q| q[axis]).max().unwrap();
            if (side >> axis) & 1 == 0 {
                -M - lo
            } else {
                M - hi
            }
        });
        let moved = p.map(|q| add(q, t));
        assert_eq!(f(moved), s, "translating {p:?} by {t:?}");
    }
}

proptest! {
    #![proptest_config(cfg(256))]

    #[test]
    fn orient2d_obeys_every_symmetry(p in config::<2, 3>(), k in 2i64..=1 << 40) {
        let v = check_o2(p);
        check_all_laws(o2, p, sign_of(&v), k);
    }

    #[test]
    fn orient3d_obeys_every_symmetry(p in config::<3, 4>(), k in 2i64..=1 << 40) {
        let v = check_o3(p);
        check_all_laws(o3, p, sign_of(&v), k);
    }

    #[test]
    fn incircle_obeys_every_symmetry(p in config::<2, 4>(), k in 2i64..=1 << 40) {
        let v = check_ic(p);
        check_all_laws(ic, p, sign_of(&v), k);
    }

    #[test]
    fn insphere_obeys_every_symmetry(p in config::<3, 5>(), k in 2i64..=1 << 40) {
        let v = check_ins(p);
        check_all_laws(ins, p, sign_of(&v), k);
    }

    /// Two equal points make every predicate zero, whichever two they are.
    #[test]
    fn any_two_equal_points_give_zero(
        a in config::<2, 3>(), b in config::<3, 4>(), c in config::<2, 4>(), d in config::<3, 5>(),
    ) {
        for i in 0..5 {
            for j in 0..5 {
                if i == j {
                    continue;
                }
                if i < 3 && j < 3 {
                    let mut q = a;
                    q[j] = q[i];
                    prop_assert_eq!(o2(q), Sign::Zero, "{:?}", q);
                }
                if i < 4 && j < 4 {
                    let mut q = b;
                    q[j] = q[i];
                    prop_assert_eq!(o3(q), Sign::Zero, "{:?}", q);
                    let mut q = c;
                    q[j] = q[i];
                    prop_assert_eq!(ic(q), Sign::Zero, "{:?}", q);
                }
                let mut q = d;
                q[j] = q[i];
                prop_assert_eq!(ins(q), Sign::Zero, "{:?}", q);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Laws between predicates.
// ---------------------------------------------------------------------------------------------

/// Coordinates up to 2^20 − 1, so that x² + y² − M stays on the grid.
const LIFT_MAX: i64 = (1 << 20) - 1;

/// The point lifted onto the paraboloid z = x² + y², shifted down by M to use the whole grid.
fn lifted(p: Pt2) -> Pt3 {
    [p[0], p[1], p[0] * p[0] + p[1] * p[1] - M]
}

fn lift_coord() -> impl Strategy<Value = i64> {
    prop_oneof![
        4 => -LIFT_MAX..=LIFT_MAX,
        2 => -3i64..=3,
        1 => (0i64..3).prop_map(|k| LIFT_MAX - k),
        1 => (0i64..3).prop_map(|k| k - LIFT_MAX),
    ]
}

/// Lattice points on the circle of radius l about the origin: (l/n)(p² − q², 2pq) with n = p² + q²
/// dividing l, with every sign and the swap of the axes.
fn circle_points(l: i64) -> Vec<Pt2> {
    let mut set = BTreeSet::new();
    for p in 0i64..=40 {
        for q in 0i64..=40 {
            let n = p * p + q * q;
            if n > 0 && l % n == 0 {
                let k = l / n;
                let (x, y) = (k * (p * p - q * q), k * 2 * p * q);
                for (sx, sy) in [(1, 1), (1, -1), (-1, 1), (-1, -1)] {
                    set.insert([sx * x, sy * y]);
                    set.insert([sx * y, sy * x]);
                }
            }
        }
    }
    let pts: Vec<Pt2> = set.into_iter().collect();
    let l2 = i128::from(l) * i128::from(l);
    let norm2 = |q: &Pt2| i128::from(q[0]).pow(2) + i128::from(q[1]).pow(2);
    assert!(pts.iter().all(|q| norm2(q) == l2));
    assert!(
        pts.len() >= 12,
        "radius {l}: only {} lattice points",
        pts.len()
    );
    pts
}

proptest! {
    #![proptest_config(cfg(384))]

    /// The module's own description of incircle: the orientation of the points lifted onto the
    /// paraboloid. With this module's conventions incircle(a, b, c, d) = −orient3d of the lifted
    /// points, exactly as values. Both sides use only the predicates' public API.
    #[test]
    fn incircle_is_minus_orient3d_of_the_points_lifted_onto_the_paraboloid(
        p in uniform::<_, 4>(uniform::<_, 2>(lift_coord())),
    ) {
        let q = p.map(lifted);
        let v = check_ic(p);
        let w = check_o3(q);
        prop_assert_eq!(&v, &-w);
        prop_assert_eq!(ic(p), o3(q).negate());
    }

    /// Cocircular lattice points lift to coplanar points; one step off the circle lifts off the
    /// plane, and both predicates still agree.
    #[test]
    fn lifted_cocircular_points_are_coplanar_and_one_step_off_is_not(
        picks in uniform::<_, 4>(any::<Index>()), centre in uniform::<_, 2>(any::<u64>()),
        big_radius in any::<bool>(), i in 0usize..4, axis in 0usize..2, up in any::<bool>(),
    ) {
        let l = if big_radius { 720_720 } else { 27_720 };
        let pts = circle_points(l);
        let o = centre.map(|s| pick(-(LIFT_MAX - l - 1), LIFT_MAX - l - 1, s));
        let p: [Pt2; 4] = picks.map(|ix| add(o, pts[ix.index(pts.len())]));
        prop_assert_eq!(ic(p), Sign::Zero);
        prop_assert_eq!(o3(p.map(lifted)), Sign::Zero);
        let mut m = p;
        m[i][axis] += if up { 1 } else { -1 };
        prop_assert_eq!(ic(m), o3(m.map(lifted)).negate(), "{:?}", m);
        check_ic(m);
    }

    /// d = a ± k along an axis: orient3d is the sign of that component of (b − a) × (c − a), which
    /// is orient2d of a, b, c projected onto the other two axes (in cyclic order).
    #[test]
    fn orient3d_toward_an_axis_is_orient2d_of_the_projection(
        p in uniform::<_, 3>(pt3()), axis in 0usize..3, k in 1i64..=M,
    ) {
        let [a, b, c] = p;
        let step = if a[axis] <= 0 { k.min(M - a[axis]) } else { -k.min(M + a[axis]) };
        let mut d = a;
        d[axis] += step;
        let proj = |q: Pt3| [q[(axis + 1) % 3], q[(axis + 2) % 3]];
        let expected = odd_if(o2([proj(a), proj(b), proj(c)]), step < 0);
        prop_assert_eq!(o3([a, b, c, d]), expected, "{:?}", [a, b, c, d]);
        check_o3([a, b, c, d]);
    }
}

/// u, v span a lattice plane through the origin: u · v = 0 and |u| = |v| = 3, so (x, y) ↦ x·u + y·v
/// is a similarity. n = (u × v) / 3 is its unit-free normal.
const U: Pt3 = [1, 2, 2];
const V: Pt3 = [2, 1, -2];
const NRM: Pt3 = [-2, 2, -1];

/// o + x·u + y·v.
fn tilt(o: Pt3, q: Pt2) -> Pt3 {
    std::array::from_fn(|k| o[k] + q[0] * U[k] + q[1] * V[k])
}

/// o + j·n.
fn along_normal(o: Pt3, j: i64) -> Pt3 {
    std::array::from_fn(|k| o[k] + j * NRM[k])
}

/// The largest |component| of x·u + y·v over the given plane points.
fn reach(pts: &[Pt2]) -> i64 {
    pts.iter()
        .map(|&q| {
            tilt([0, 0, 0], q)
                .iter()
                .map(num_traits::Signed::abs)
                .max()
                .unwrap()
        })
        .max()
        .unwrap()
}

/// 2D quadruples within ±limit: random, or three corners of a rectangle with the fourth point
/// within a step or two of their circle.
fn quad(limit: i64) -> impl Strategy<Value = [Pt2; 4]> {
    let c = move || prop_oneof![4 => -limit..=limit, 1 => -3i64..=3, 1 => Just(limit), 1 => Just(-limit)];
    prop_oneof![
        uniform::<_, 4>(uniform::<_, 2>(c())),
        (
            uniform::<_, 2>((c(), c())),
            uniform::<_, 3>(0usize..4),
            c(),
            any::<Index>()
        )
            .prop_map(move |(axes, idx, x, which)| {
                let bx = axes.map(|(u, v)| (u.min(v), u.max(v)));
                let [a, b, c] = idx.map(|k| corner(&bx, k));
                let near = near_sphere(&bx, [x, 0], limit);
                let d = if near.is_empty() {
                    [x, 0]
                } else {
                    near[which.index(near.len())]
                };
                [a, b, c, d]
            }),
    ]
}

/// Whether three 3D points lie on one line (including any two coinciding), exactly.
fn collinear3(a: Pt3, b: Pt3, c: Pt3) -> bool {
    let (u, v) = (diff(b, a), diff(c, a));
    (0..3).all(|k| {
        let (i, j) = ((k + 1) % 3, (k + 2) % 3);
        (&u[i] * &v[j] - &u[j] * &v[i]).is_zero()
    })
}

/// The doc's rule when a, b, c are collinear, which makes a, b, c, d coplanar for every d: zero for
/// every e if two of a, b, c coincide or d is on their line too (one line); otherwise (no single
/// circle or line) zero exactly when e is in the plane of a, b, d.
fn insphere_zero_by_doc_when_abc_collinear([a, b, c, d, e]: [Pt3; 5]) -> bool {
    assert!(collinear3(a, b, c));
    if a == b || b == c || a == c || collinear3(a, b, d) {
        return true;
    }
    orient3d_det([a, b, d, e]).is_zero()
}

proptest! {
    #![proptest_config(cfg(384))]

    /// a, b, c, e in an axis plane and d anywhere: e is inside the sphere exactly when it is inside
    /// the circle through a, b, c, so insphere = orient3d(a, b, c, d) × orient2d × incircle of the
    /// plane coordinates. When d is in the plane too both sides are zero. When a, b, c are
    /// collinear there is no such circle and the doc's degenerate rule applies instead.
    #[test]
    fn insphere_with_four_points_in_an_axis_plane_is_orient3d_times_incircle(
        axis in 0usize..3, level in coord(), q in quad(M), d in pt3(),
    ) {
        let embed = |p: Pt2| -> Pt3 {
            let mut r = [0i64; 3];
            r[axis] = level;
            r[(axis + 1) % 3] = p[0];
            r[(axis + 2) % 3] = p[1];
            r
        };
        let [a, b, c, e] = q.map(embed);
        let got = ins([a, b, c, d, e]);
        if o2([q[0], q[1], q[2]]) == Sign::Zero {
            let zero_by_doc = insphere_zero_by_doc_when_abc_collinear([a, b, c, d, e]);
            prop_assert_eq!(got.is_zero(), zero_by_doc, "{:?}", [a, b, c, d, e]);
        } else {
            let expected = o3([a, b, c, d]).mul(o2([q[0], q[1], q[2]])).mul(ic(q));
            prop_assert_eq!(got, expected, "{:?}", [a, b, c, d, e]);
        }
        check_ins([a, b, c, d, e]);
    }

    /// The same in a plane that is not an axis plane, through the similarity
    /// (x, y) ↦ o + x·u + y·v.
    #[test]
    fn insphere_with_four_points_in_a_tilted_plane_is_orient3d_times_incircle(
        q in quad(M / 8), oc in uniform::<_, 3>(any::<u64>()), d in pt3(),
    ) {
        let r = reach(&q);
        let o: Pt3 = oc.map(|s| pick(-(M - r), M - r, s));
        let [a, b, c, e] = q.map(|p| tilt(o, p));
        let got = ins([a, b, c, d, e]);
        if o2([q[0], q[1], q[2]]) == Sign::Zero {
            let zero_by_doc = insphere_zero_by_doc_when_abc_collinear([a, b, c, d, e]);
            prop_assert_eq!(got.is_zero(), zero_by_doc, "{:?}", [a, b, c, d, e]);
        } else {
            let expected = o3([a, b, c, d]).mul(o2([q[0], q[1], q[2]])).mul(ic(q));
            prop_assert_eq!(got, expected, "{:?}", [a, b, c, d, e]);
        }
        check_ins([a, b, c, d, e]);
    }

    /// Four points on one lattice circle in a tilted plane: coplanar and cocircular, so the doc
    /// says insphere is zero for every e, including points on the circle's axis. With three of
    /// them and any d off the plane, the sphere contains the circle, so the fourth point is on it.
    #[test]
    fn four_points_on_a_tilted_circle_make_insphere_zero_for_every_e(
        picks in uniform::<_, 4>(any::<Index>()), oc in uniform::<_, 3>(any::<u64>()), e in pt3(),
        j in any::<u64>(), d in pt3(), step in 0usize..6,
    ) {
        // 3·lcm(1..=28): many lattice points, and x·u + y·v reaches about 0.62 M.
        let pts = circle_points(240_940_299_600);
        let r = reach(&pts);
        let o: Pt3 = oc.map(|s| pick(-(M - r), M - r, s));
        let [a, b, c, f] = picks.map(|ix| tilt(o, pts[ix.index(pts.len())]));
        prop_assert_eq!(o3([a, b, c, f]), Sign::Zero);
        prop_assert_eq!(ins([a, b, c, f, e]), Sign::Zero, "{:?}", [a, b, c, f, e]);
        check_ins([a, b, c, f, e]);
        let (lo, hi) = k_range(o, NRM);
        let on_axis = along_normal(o, pick(lo, hi, j));
        prop_assert_eq!(ins([a, b, c, f, on_axis]), Sign::Zero);
        if o3([a, b, c, d]) != Sign::Zero {
            prop_assert_eq!(ins([a, b, c, d, f]), Sign::Zero, "{:?}", [a, b, c, d, f]);
            let mut moved = f;
            moved[step / 2] += if step % 2 == 0 { 1 } else { -1 };
            if !in_range(moved[step / 2]) {
                return Ok(());
            }
            let p = [a, b, c, d, moved];
            let value = insphere_by_geometry(p).unwrap();
            prop_assert_eq!(BigRational::from_integer(check_ins(p)), value.clone());
            prop_assert_eq!(ins(p), sign_of_rat(&value));
        }
    }

    /// The doc's subtle case: a, b, c collinear and distinct with d in their plane off their line.
    /// They lie on no single circle or line, so zero means exactly that e is in their plane.
    /// Checked in a tilted plane and in an axis plane, at up to half the grid.
    #[test]
    fn three_collinear_points_and_a_fourth_in_their_plane(
        oc in uniform::<_, 3>(any::<u64>()), k in 1i64..=M / 8, xd in -M / 8..=M / 8,
        yd in 1i64..=M / 8, flip in any::<bool>(), qe in uniform::<_, 2>(-M / 8..=M / 8),
        j in -3i64..=3, tilted in any::<bool>(),
    ) {
        let yd = if flip { -yd } else { yd };
        let plane = [[-k, 0], [0, 0], [k, 0], [xd, yd], qe];
        let margin = (if tilted { reach(&plane) } else { M / 8 }) + 8;
        let o: Pt3 = oc.map(|s| pick(-(M - margin), M - margin, s));
        let embed = |q: Pt2| if tilted { tilt(o, q) } else { [o[0] + q[0], o[1] + q[1], o[2]] };
        let normal: Pt3 = if tilted { NRM } else { [0, 0, 1] };
        let [a, b, c, d, e0] = plane.map(embed);
        let e = add(e0, normal.map(|v| v * j));
        prop_assert_eq!(o3([a, b, c, d]), Sign::Zero);
        prop_assert_eq!(o2([[-k, 0], [0, 0], [k, 0]]), Sign::Zero);
        let v = check_ins([a, b, c, d, e]);
        prop_assert_eq!(v.is_zero(), j == 0, "{:?}", [a, b, c, d, e]);
    }

    /// Collinear points at full scale: incircle with three distinct collinear points is zero
    /// exactly when d is on their line; orient3d with a, b, c collinear is zero for every d.
    #[test]
    fn collinear_points_at_full_scale(
        a in pt2(), w in direction::<2>(), ks in uniform::<_, 3>(any::<u64>()), axis in 0usize..2,
        up in any::<bool>(), a3 in pt3(), w3 in direction::<3>(),
        ks3 in uniform::<_, 2>(any::<u64>()), d3 in pt3(),
    ) {
        if w != [0, 0] {
            let (lo, hi) = k_range(a, w);
            let k: Vec<i64> = ks.iter().map(|&s| pick(lo, hi, s)).collect();
            let on = |k: i64| add(a, w.map(|v| v * k));
            if hi - lo >= 2 && k[0] != 0 && k[1] != 0 && k[0] != k[1] {
                let (b, c, d) = (on(k[0]), on(k[1]), on(k[2]));
                prop_assert_eq!(ic([a, b, c, d]), Sign::Zero, "{:?}", [a, b, c, d]);
                if w[1 - axis] != 0 {
                    let mut off = d;
                    off[axis] += if up { 1 } else { -1 };
                    if in_range(off[axis]) {
                        let v = check_ic([a, b, c, off]);
                        prop_assert!(!v.is_zero(), "{:?}", [a, b, c, off]);
                    }
                }
            }
        }
        if w3 != [0, 0, 0] {
            let (lo, hi) = k_range(a3, w3);
            let k: Vec<i64> = ks3.iter().map(|&s| pick(lo, hi, s)).collect();
            let on = |k: i64| add(a3, w3.map(|v| v * k));
            let p = [a3, on(k[0]), on(k[1]), d3];
            prop_assert_eq!(o3(p), Sign::Zero, "{:?}", p);
            check_o3(p);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The degenerate cases: which sign a degenerate but non-zero answer has, as documented.
// ---------------------------------------------------------------------------------------------

/// (c − a) · (c − b), exactly: for c on the line through a and b, its power with respect to every
/// circle (sphere) through a and b.
fn power_on_line<const D: usize>(c: [i64; D], a: [i64; D], b: [i64; D]) -> BigInt {
    let (u, v) = (diff(c, a), diff(c, b));
    (0..D).map(|k| &u[k] * &v[k]).sum()
}

/// |d − o|² − r² for the circle through a, b, c (3D, not collinear), o and r its centre and radius:
/// with u = b − a, v = c − a and w = d − a, o − a = (|v|²(|u|² − u·v) u + |u|²(|v|² − u·v) v) / D
/// for D = 2 |u × v|² = 2 (|u|²|v|² − (u·v)²), and the power is |w|² − 2 w · (o − a).
fn circle3_power_outside([a, b, c, d]: [Pt3; 4]) -> BigRational {
    let (u, v, w) = (diff(b, a), diff(c, a), diff(d, a));
    let dot = |p: &[BigInt; 3], q: &[BigInt; 3]| -> BigInt { (0..3).map(|k| &p[k] * &q[k]).sum() };
    let (uu, vv, uv) = (dot(&u, &u), dot(&v, &v), dot(&u, &v));
    let den = BigInt::from(2) * (&uu * &vv - &uv * &uv);
    assert!(
        !den.is_zero(),
        "circle3_power_outside needs a, b, c not collinear"
    );
    let alpha = &vv * (&uu - &uv);
    let beta = &uu * (&vv - &uv);
    let w_dot_o = BigRational::new(alpha * dot(&w, &u) + beta * dot(&w, &v), den);
    BigRational::from_integer(dot(&w, &w)) - BigRational::from_integer(BigInt::from(2)) * w_dot_o
}

/// Three points base + k·w on one line, with the k drawn from `ks`: any of them may coincide, and
/// the third may lie between the other two or outside them.
fn on_line<const D: usize>(base: [i64; D], w: [i64; D], ks: [u64; 3]) -> [[i64; D]; 3] {
    let (lo, hi) = k_range(base, w);
    ks.map(|s| {
        let k = pick(lo, hi, s);
        add(base, w.map(|v| v * k))
    })
}

/// The documented signs in words, at full scale. incircle with collinear a, b, c: the sign of
/// orient2d(a, b, d) when c is outside the segment ab, the opposite when c is between. insphere
/// with collinear a, b, c: the sign of orient3d(a, b, d, e) when c is between, the opposite when
/// outside. insphere with a, b, c, d in a plane: the sign of orient3d(a, b, c, e) when d is
/// outside the circle through a, b, c, the opposite when inside, zero on it.
#[test]
fn degenerate_signs_follow_the_documented_rules_at_full_scale() {
    let (a, o, b, d) = ([-M, -M], [0, 0], [M, M], [-M, M]);
    assert_eq!(o2([a, b, d]), Sign::Positive);
    assert_eq!(o2([a, o, d]), Sign::Positive);
    assert_eq!(ic([a, b, o, d]), Sign::Negative, "c between a and b");
    assert_eq!(ic([a, o, b, d]), Sign::Positive, "c outside the segment");
    check_ic([a, b, o, d]);
    check_ic([a, o, b, d]);

    let (a, o, b) = ([-M, -M, -M], [0, 0, 0], [M, M, M]);
    let (d, e) = ([M, -M, 0], [0, 0, M]);
    assert_eq!(o3([a, b, d, e]), Sign::Negative);
    assert_eq!(o3([a, o, d, e]), Sign::Negative);
    assert_eq!(ins([a, b, o, d, e]), Sign::Negative, "c between a and b");
    assert_eq!(
        ins([a, o, b, d, e]),
        Sign::Positive,
        "c outside the segment"
    );
    check_ins([a, b, o, d, e]);
    check_ins([a, o, b, d, e]);

    let (a, b, c) = ([M, 0, 0], [0, M, 0], [-M, 0, 0]);
    let e = [0, 0, M];
    assert_eq!(o3([a, b, c, e]), Sign::Positive);
    assert_eq!(
        ins([a, b, c, [0, 0, 0], e]),
        Sign::Negative,
        "d inside the circle"
    );
    assert_eq!(
        ins([a, b, c, [M, M, 0], e]),
        Sign::Positive,
        "d outside the circle"
    );
    assert_eq!(ins([a, b, c, [0, -M, 0], e]), Sign::Zero, "d on the circle");
    for d in [[0, 0, 0], [M, M, 0], [0, -M, 0]] {
        check_ins([a, b, c, d, e]);
    }
}

proptest! {
    #![proptest_config(cfg(384))]

    /// incircle's documented value when a, b, c are collinear: orient2d's determinant of
    /// (a, b, d) times (c − a) · (c − b), exactly, for d anywhere.
    #[test]
    fn incircle_with_collinear_a_b_c_is_orient2d_of_a_b_d_times_the_power_of_c(
        base in pt2(), w in direction::<2>(), ks in uniform::<_, 3>(any::<u64>()), d in pt2(),
    ) {
        let [a, b, c] = on_line(base, w, ks);
        let expected = orient2d_det([a, b, d]) * power_on_line(c, a, b);
        prop_assert_eq!(check_ic([a, b, c, d]), expected, "{:?}", [a, b, c, d]);
    }

    /// insphere's documented value when a, b, c are collinear: minus orient3d's determinant of
    /// (a, b, d, e) times (c − a) · (c − b), exactly, for d and e anywhere.
    #[test]
    fn insphere_with_collinear_a_b_c_is_minus_orient3d_of_a_b_d_e_times_the_power_of_c(
        base in pt3(), w in direction::<3>(), ks in uniform::<_, 3>(any::<u64>()),
        d in pt3(), e in pt3(),
    ) {
        let [a, b, c] = on_line(base, w, ks);
        let expected = -orient3d_det([a, b, d, e]) * power_on_line(c, a, b);
        prop_assert_eq!(check_ins([a, b, c, d, e]), expected, "{:?}", [a, b, c, d, e]);
    }

    /// insphere's documented value when a, b, c, d lie in an axis plane and a, b, c are not
    /// collinear: orient3d's determinant of (a, b, c, e) times |d − o|² − r² for the circle
    /// through a, b, c, exactly, for e anywhere. Full scale, with d random or within a step or two
    /// of the circle.
    #[test]
    fn insphere_with_four_points_in_an_axis_plane_is_orient3d_of_a_b_c_e_times_the_power_of_d(
        axis in 0usize..3, level in coord(), q in quad(M), e in pt3(),
    ) {
        if o2([q[0], q[1], q[2]]) == Sign::Zero {
            return Ok(());
        }
        let embed = |p: Pt2| -> Pt3 {
            let mut r = [0i64; 3];
            r[axis] = level;
            r[(axis + 1) % 3] = p[0];
            r[(axis + 2) % 3] = p[1];
            r
        };
        let [a, b, c, d] = q.map(embed);
        let expected = BigRational::from_integer(orient3d_det([a, b, c, e]))
            * circle3_power_outside([a, b, c, d]);
        let got = BigRational::from_integer(check_ins([a, b, c, d, e]));
        prop_assert_eq!(got, expected, "{:?}", [a, b, c, d, e]);
    }

    /// The same in a tilted lattice plane, through the similarity (x, y) ↦ o + x·u + y·v, with e
    /// anywhere or within three steps of d along the plane's normal.
    #[test]
    fn insphere_with_four_points_in_a_tilted_plane_is_orient3d_of_a_b_c_e_times_the_power_of_d(
        q in quad(M / 8), oc in uniform::<_, 3>(any::<u64>()), e in pt3(), j in -3i64..=3,
    ) {
        if o2([q[0], q[1], q[2]]) == Sign::Zero {
            return Ok(());
        }
        // Room for the plane's points and for three steps along the normal (|n| ≤ 2 per axis).
        let r = reach(&q) + 6;
        let o: Pt3 = oc.map(|s| pick(-(M - r), M - r, s));
        let [a, b, c, d] = q.map(|p| tilt(o, p));
        for e in [e, along_normal(d, j)] {
            let expected = BigRational::from_integer(orient3d_det([a, b, c, e]))
                * circle3_power_outside([a, b, c, d]);
            let got = BigRational::from_integer(check_ins([a, b, c, d, e]));
            prop_assert_eq!(got, expected, "{:?}", [a, b, c, d, e]);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The wide step: the documented formula evaluated through the public Wide API, exactly.
// ---------------------------------------------------------------------------------------------

/// The value a `Wide` holds, read from its two's-complement limbs by `num-bigint`.
fn wide_value<const L: usize>(w: &Wide<L>) -> BigInt {
    let bytes: Vec<u8> = w
        .to_le_limbs()
        .iter()
        .flat_map(|limb| limb.to_le_bytes())
        .collect();
    BigInt::from_signed_bytes_le(&bytes)
}

/// x₀·y₀ + x₁·y₁ + … in `Wide<L>` with checked operations, as the module's wide step does.
fn wide_sum<const L: usize>(terms: &[(i128, i128)]) -> Option<Wide<L>> {
    terms.iter().try_fold(Wide::<L>::ZERO, |sum, &(x, y)| {
        let product = Wide::<L>::from_i128(x).checked_mul(&Wide::<L>::from_i128(y))?;
        sum.checked_add(&product)
    })
}

fn to_i128(v: &BigInt) -> i128 {
    v.to_i128()
        .unwrap_or_else(|| panic!("an i128-stage value left i128: {v}"))
}

/// incircle's documented (lift, 2 × 2 determinant) pairs relative to d, computed exactly.
fn incircle_terms(p: [Pt2; 4]) -> Vec<(i128, i128)> {
    let [a, b, c] = [0usize, 1, 2].map(|i| diff(p[i], p[3]));
    let lift = |u: &[BigInt; 2]| to_i128(&(&u[0] * &u[0] + &u[1] * &u[1]));
    let cross = |u: &[BigInt; 2], v: &[BigInt; 2]| to_i128(&cross2(u, v));
    vec![
        (lift(&a), cross(&b, &c)),
        (lift(&b), cross(&c, &a)),
        (lift(&c), cross(&a, &b)),
    ]
}

/// insphere's documented (lift, signed 3 × 3 cofactor) pairs relative to e, computed exactly.
fn insphere_terms(p: [Pt3; 5]) -> Vec<(i128, i128)> {
    let [a, b, c, d] = [0usize, 1, 2, 3].map(|i| diff(p[i], p[4]));
    let lift = |u: &[BigInt; 3]| to_i128(&u.iter().map(|x| x * x).sum::<BigInt>());
    let det = |u: &[BigInt; 3], v: &[BigInt; 3], w: &[BigInt; 3]| {
        to_i128(&det3b(&[u.clone(), v.clone(), w.clone()]))
    };
    vec![
        (lift(&a), det(&b, &c, &d)),
        (lift(&b), -det(&a, &c, &d)),
        (lift(&c), det(&a, &b, &d)),
        (lift(&d), -det(&a, &b, &c)),
    ]
}

/// The factors within 2^`x_bits` and 2^`y_bits`, each product within 2^`product_bits`, the I256
/// evaluation never overflowing (so the module never needs its total path) and equal to the exact
/// determinant, and the I512 evaluation equal too.
fn check_wide_step(terms: &[(i128, i128)], exact: &BigInt, bits: [u32; 3], what: &str) {
    let [x_bits, y_bits, product_bits] = bits;
    for &(x, y) in terms {
        assert!(
            x.unsigned_abs() < 1u128 << x_bits,
            "{what}: lift {x} over 2^{x_bits}"
        );
        assert!(
            y.unsigned_abs() < 1u128 << y_bits,
            "{what}: determinant {y} over 2^{y_bits}"
        );
        let product = (BigInt::from(x) * y).abs();
        assert!(
            product < pow2(product_bits),
            "{what}: product over 2^{product_bits}"
        );
    }
    let narrow = wide_sum::<4>(terms)
        .unwrap_or_else(|| panic!("{what}: I256 overflowed, the module would need its total path"));
    assert_eq!(
        &wide_value(&narrow),
        exact,
        "{what}: I256 sum is not the determinant"
    );
    let wide = wide_sum::<8>(terms).unwrap();
    assert_eq!(
        &wide_value(&wide),
        exact,
        "{what}: I512 sum is not the determinant"
    );
}

fn check_wide_incircle(p: [Pt2; 4]) {
    let exact = check_ic(p);
    check_wide_step(
        &incircle_terms(p),
        &exact,
        [83, 83, 166],
        &format!("incircle {p:?}"),
    );
}

fn check_wide_insphere(p: [Pt3; 5]) {
    let exact = check_ins(p);
    check_wide_step(
        &insphere_terms(p),
        &exact,
        [84, 126, 210],
        &format!("insphere {p:?}"),
    );
}

/// The configurations that maximise each intermediate: for incircle d at a corner, a at the
/// opposite one (lift 2δ²) and b, c at the other two (2 × 2 determinant δ²); for insphere e at a
/// corner, a opposite (lift 3δ²) and b, c, d at the three corners adjacent to a (3 × 3 determinant
/// 2δ³, the largest for vectors in [0, δ]³). All are cospherical, so the largest terms cancel to
/// zero exactly; then the last point moved one step inward. Every order and reflection.
#[test]
fn wide_step_is_exact_where_its_intermediates_peak() {
    // q mirrored in the axes whose bit is set in `refl`.
    fn mirror<const D: usize>(q: [i64; D], refl: usize) -> [i64; D] {
        std::array::from_fn(|k| if (refl >> k) & 1 == 1 { -q[k] } else { q[k] })
    }
    let base2: [Pt2; 4] = [[M, M], [M, -M], [-M, M], [-M, -M]];
    for (perm, _) in permutations::<4>() {
        for refl in 0..4 {
            let p = apply(base2, &perm).map(|q| mirror(q, refl));
            check_wide_incircle(p);
            let mut q = p;
            q[3] = q[3].map(|v| v - v.signum());
            check_wide_incircle(q);
        }
    }
    let base3: [Pt3; 5] = [[M, M, M], [M, M, -M], [M, -M, M], [-M, M, M], [-M, -M, -M]];
    let peak = insphere_terms(base3);
    assert_eq!(
        peak[0].0,
        3 * i128::from(2 * M).pow(2),
        "the lift peaks at 3δ²"
    );
    assert_eq!(
        peak[0].1.abs(),
        2 * i128::from(2 * M).pow(3),
        "the cofactor peaks at 2δ³"
    );
    for (perm, _) in permutations::<5>() {
        for refl in 0..8 {
            let p = apply(base3, &perm).map(|q| mirror(q, refl));
            check_wide_insphere(p);
            let mut q = p;
            q[4] = q[4].map(|v| v - v.signum());
            check_wide_insphere(q);
        }
    }
    // Every configuration on {−M, 0, M}², exactly.
    let level = [-M, 0, M];
    let square: Vec<Pt2> = level
        .iter()
        .flat_map(|&x| level.iter().map(move |&y| [x, y]))
        .collect();
    let n = square.len();
    for code in 0..n.pow(4) {
        let p: [Pt2; 4] = std::array::from_fn(|k| square[code / n.pow(k as u32) % n]);
        check_wide_incircle(p);
    }
}

/// A coordinate at an edge, one step inside it, at the centre, or anywhere. Boxed, like `coord`.
fn edge_coord() -> BoxedStrategy<i64> {
    prop_oneof![
        Just(-M),
        Just(0i64),
        Just(M),
        Just(M - 1),
        Just(1 - M),
        -M..=M
    ]
    .boxed()
}

proptest! {
    #![proptest_config(cfg(512))]

    /// Random points with coordinates at the extremes, near them, or anywhere, through the
    /// documented formula in I256 and I512.
    #[test]
    fn wide_step_is_exact_on_extreme_and_random_points(
        p in uniform::<_, 4>(uniform::<_, 2>(edge_coord())),
        q in uniform::<_, 5>(uniform::<_, 3>(edge_coord())),
        r in config::<3, 5>(),
    ) {
        check_wide_incircle(p);
        check_wide_insphere(q);
        check_wide_insphere(r);
    }
}

// ---------------------------------------------------------------------------------------------
// No build profile can change an answer.
// ---------------------------------------------------------------------------------------------

/// Everything predicate.rs's code must not contain: native arithmetic (`+`, `-` other than in
/// `->`, `*`, `/`, `%` and shifts) and `pow` and `abs`, which wrap silently when overflow checks
/// are off; `as` casts, which truncate silently in every profile; and the wrapping, overflowing,
/// saturating and unchecked methods, which never report an overflow.
const NOT_IN_PREDICATE_CODE: [&str; 14] = [
    "+",
    "-",
    "*",
    "/",
    "%",
    "<<",
    " >> ",
    ".pow(",
    ".abs(",
    " as ",
    "wrapping_",
    "overflowing_",
    "saturating_",
    "unchecked_",
];

/// predicate.rs promises the same answer in every build profile: every fixed-width operation in its
/// code is a checked method, with an exact fallback behind the fast path, so a wrong bit budget
/// costs time, never a silently wrapped sign. One native operator would break that promise in any
/// build with overflow checks off, which is the default for release builds and the workspace's
/// setting. So this reads the module's code, outside comments and its test module, and fails on
/// anything in `NOT_IN_PREDICATE_CODE`.
#[test]
fn predicate_code_has_no_native_arithmetic_so_no_build_profile_can_change_an_answer() {
    let source = include_str!("../src/predicate.rs");
    let end = source
        .find("#[cfg(test)]")
        .expect("predicate.rs keeps its tests in a test module");
    let mut code_lines = 0;
    for (number, line) in source[..end].lines().enumerate() {
        let code = line.split("//").next().unwrap().replace("->", "");
        if code.trim().is_empty() {
            continue;
        }
        code_lines += 1;
        for token in NOT_IN_PREDICATE_CODE {
            let at = number + 1;
            assert!(
                !code.contains(token),
                "predicate.rs line {at} has `{token}`: {}",
                line.trim()
            );
        }
    }
    // The scan read the module's code, not an empty prefix of it.
    assert!(
        code_lines > 150,
        "only {code_lines} lines of code before the test module"
    );
}
