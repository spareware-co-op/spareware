//! Second-round adversarial tests for `geom_num::plane`, against the round that made the `i128`
//! arithmetic of `Plane::through` and `Plane::side` checked (still `const fn`), made `Tpi::new`
//! compute the full homogeneous coordinates, and gave `side_of_tpi` and `to_grid` an `I512`
//! fallback (since the third review round called their total path, as in `predicate.rs`, and every
//! step that could not complete is reported as `BudgetExceeded` rather than answered). Every test
//! fails if the module is wrong, with the exact inputs and the expected and actual values.
//!
//! Oracles (none shares code with the module, `tests/plane.rs` or `tests/attack_plane.rs`):
//! - planes: the cofactors of the 4 × 4 orientation determinant, each 3 × 3 minor by Bareiss's
//!   fraction-free elimination (Bareiss 1968, doi:10.1090/S0025-5718-1968-0226829-0);
//! - sides of grid points: −det[[a, 1], [b, 1], [c, 1], [p, 1]] by Bareiss, and
//!   `predicate::orient3d`;
//! - three-plane points: Gauss–Jordan elimination over the rationals, and w = det(`n_1`, `n_2`,
//!   `n_3`) by Bareiss. For planes through the frame's corners: the same on the unit cube in `i64`,
//!   scaled by powers of M, and X / w substituted back into all three planes;
//! - sides of three-plane points: the sign of the 4 × 4 determinant of the rows (`n_k`, `d_k`),
//!   which equals n · X + d · w, times the sign of w; and, separately, the plane evaluated at the
//!   exact rational point;
//! - commits: the rational point rounded half to even by floor and fractional part.
//!
//! Attacks: the bounds at their extremes (all 40 oriented planes through the frame's corners and
//! all 64,000 ordered triples of them, which reach the largest n, d, w, X and n · X + d · w any
//! grid input can give); the `I512` fallback's budget recomputed at the types' own extremes;
//! parallel, anti-parallel, repeated, rescaled, pencil and prism triples at the extremes in every
//! order; planes tilted one step from parallel; planes whose normals differ by one unit, meeting
//! about 2^123 and 2^164 steps away with |w| = 1; ties at ±1/2, ±3/2, ±(M − 1/2) and ±(M + 1/2)
//! with |w| from 2 to about 2^235 and both signs; commits that move at most half a step;
//! three-plane points that are grid points; permutation, orientation, mirror and translation
//! laws; and compile-time tables of `through` and `side` against the same calls at run time.

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

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::hint::black_box;

use geom_num::grid::COORD_MAX;
use geom_num::ladder;
use geom_num::plane::{Plane, Tpi};
use geom_num::predicate::orient3d;
use geom_num::{I256, I320, I512, NumError, P3, Sign, Wide};
use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use proptest::prelude::*;

/// A grid commit with public fields, for comparing with expected values: `Rounded`'s fields are
/// private, so that only the crate can say a value is exact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Commit {
    value: P3,
    exact: bool,
}

/// `Tpi::to_grid` as a [`Commit`].
trait ToCommit {
    fn commit(&self) -> Result<Commit, NumError>;
}

impl ToCommit for Tpi {
    fn commit(&self) -> Result<Commit, NumError> {
        self.to_grid().map(|r| Commit {
            value: r.value(),
            exact: r.is_exact(),
        })
    }
}

const M: i64 = COORD_MAX;

/// The six orders of three things, with whether each is an even permutation.
const PERMS: [([usize; 3], bool); 6] = [
    ([0, 1, 2], true),
    ([1, 2, 0], true),
    ([2, 0, 1], true),
    ([0, 2, 1], false),
    ([2, 1, 0], false),
    ([1, 0, 2], false),
];

/// The corners of the frame's cube.
const CORNERS: [[i64; 3]; 8] = [
    [-M, -M, -M],
    [M, -M, -M],
    [-M, M, -M],
    [M, M, -M],
    [-M, -M, M],
    [M, -M, M],
    [-M, M, M],
    [M, M, M],
];

/// The same corners on the unit cube: the frame's corners divided by M.
const UNIT: [[i64; 3]; 8] = [
    [-1, -1, -1],
    [1, -1, -1],
    [-1, 1, -1],
    [1, 1, -1],
    [-1, -1, 1],
    [1, -1, 1],
    [-1, 1, 1],
    [1, 1, 1],
];

// ---------------------------------------------------------------------------------------------
// Small helpers.
// ---------------------------------------------------------------------------------------------

fn p3(c: [i64; 3]) -> P3 {
    P3::new(c[0], c[1], c[2]).unwrap_or_else(|e| panic!("{c:?} is not a grid point: {e:?}"))
}

fn coords(q: P3) -> [i64; 3] {
    [q.x.get(), q.y.get(), q.z.get()]
}

fn in_frame(c: [i64; 3]) -> bool {
    c.iter().all(|v| v.abs() <= M)
}

fn add(a: [i64; 3], b: [i64; 3]) -> [i64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn neg(a: [i64; 3]) -> [i64; 3] {
    a.map(|v| -v)
}

fn permute<T: Copy>(v: [T; 3], perm: [usize; 3]) -> [T; 3] {
    perm.map(|i| v[i])
}

/// The plane through a, a + u and a + v, or `None` if a point leaves the frame or the three are
/// collinear.
fn plane_at(a: [i64; 3], u: [i64; 3], v: [i64; 3]) -> Option<Plane> {
    let (b, c) = (add(a, u), add(a, v));
    if in_frame(a) && in_frame(b) && in_frame(c) {
        Plane::through(p3(a), p3(b), p3(c)).ok()
    } else {
        None
    }
}

fn big(v: impl Into<BigInt>) -> BigInt {
    v.into()
}

fn wide_big<const L: usize>(v: &Wide<L>) -> BigInt {
    let bytes: Vec<u8> = v
        .to_le_limbs()
        .iter()
        .flat_map(|l| l.to_le_bytes())
        .collect();
    BigInt::from_signed_bytes_le(&bytes)
}

fn m_pow(k: u32) -> BigInt {
    big(M).pow(k)
}

fn sign_of(v: &BigInt) -> Sign {
    match v.cmp(&BigInt::zero()) {
        Ordering::Less => Sign::Negative,
        Ordering::Equal => Sign::Zero,
        Ordering::Greater => Sign::Positive,
    }
}

fn rat(v: impl Into<BigInt>) -> BigRational {
    BigRational::from_integer(v.into())
}

fn show(pt: &[BigRational; 3]) -> String {
    format!("({}, {}, {})", pt[0], pt[1], pt[2])
}

fn sub_i(a: [i64; 3], b: [i64; 3]) -> [i64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross_i(u: [i64; 3], v: [i64; 3]) -> [i64; 3] {
    [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ]
}

fn dot_i(u: [i64; 3], v: [i64; 3]) -> i64 {
    u[0] * v[0] + u[1] * v[1] + u[2] * v[2]
}

/// det of three rows of small integers by the Leibniz formula.
fn det3_i(r: [[i64; 3]; 3]) -> i64 {
    r[0][0] * r[1][1] * r[2][2] + r[0][1] * r[1][2] * r[2][0] + r[0][2] * r[1][0] * r[2][1]
        - r[0][2] * r[1][1] * r[2][0]
        - r[0][0] * r[1][2] * r[2][1]
        - r[0][1] * r[1][0] * r[2][2]
}

/// det of four rows of small integers by Laplace expansion along the last column.
fn det4_i(r: [[i64; 4]; 4]) -> i64 {
    let minor = |skip: usize| {
        let rows: Vec<[i64; 3]> = (0..4)
            .filter(|&i| i != skip)
            .map(|i| [r[i][0], r[i][1], r[i][2]])
            .collect();
        det3_i([rows[0], rows[1], rows[2]])
    };
    (0..4)
        .map(|i| {
            let term = r[i][3] * minor(i);
            // The sign of the cofactor at (i, 3) is (−1)^(i + 3).
            if (i + 3) % 2 == 0 { term } else { -term }
        })
        .sum()
}

// ---------------------------------------------------------------------------------------------
// Oracles.
// ---------------------------------------------------------------------------------------------

/// det of a square matrix by Bareiss's fraction-free elimination, with row swaps for zero pivots:
/// every division is exact, so it stays in the integers.
fn bareiss(mut m: Vec<Vec<BigInt>>) -> BigInt {
    let n = m.len();
    let mut negate = false;
    let mut prev = BigInt::one();
    for k in 0..n {
        if m[k][k].is_zero() {
            let Some(r) = (k + 1..n).find(|&r| !m[r][k].is_zero()) else {
                return BigInt::zero();
            };
            m.swap(k, r);
            negate = !negate;
        }
        for i in k + 1..n {
            for j in k + 1..n {
                let v = (&m[i][j] * &m[k][k] - &m[i][k] * &m[k][j]) / &prev;
                m[i][j] = v;
            }
        }
        prev.clone_from(&m[k][k]);
    }
    let det = m[n - 1][n - 1].clone();
    if negate { -det } else { det }
}

/// The plane through a, b, c as the cofactors of the last row of det[[a, 1], [b, 1], [c, 1],
/// [p, 1]], negated: n · p + d = −det[[a, 1], [b, 1], [c, 1], [p, 1]].
fn cofactor_plane(a: [i64; 3], b: [i64; 3], c: [i64; 3]) -> ([BigInt; 3], BigInt) {
    let minor = |i: usize, j: usize| {
        bareiss(
            [a, b, c]
                .iter()
                .map(|r| vec![big(r[i]), big(r[j]), BigInt::one()])
                .collect(),
        )
    };
    let n = [minor(1, 2), -minor(0, 2), minor(0, 1)];
    let d = -bareiss(
        [a, b, c]
            .iter()
            .map(|r| r.iter().map(|&v| big(v)).collect())
            .collect(),
    );
    (n, d)
}

/// The orientation of a, b, c, q: the sign of −det[[a, 1], [b, 1], [c, 1], [q, 1]], by Bareiss.
fn orientation(a: [i64; 3], b: [i64; 3], c: [i64; 3], q: [i64; 3]) -> Sign {
    let rows = [a, b, c, q]
        .iter()
        .map(|r| vec![big(r[0]), big(r[1]), big(r[2]), BigInt::one()])
        .collect();
    sign_of(&-bareiss(rows))
}

/// w = det(`n_1`, `n_2`, `n_3`) by Bareiss.
fn det_normals(planes: &[Plane; 3]) -> BigInt {
    bareiss(
        planes
            .iter()
            .map(|pl| pl.n().iter().map(|&v| big(v)).collect())
            .collect(),
    )
}

/// The point where three planes meet, by Gauss–Jordan elimination over the rationals, or `None`
/// if they do not meet in exactly one point.
fn solve(planes: &[Plane; 3]) -> Option<[BigRational; 3]> {
    let mut a: Vec<Vec<BigRational>> = planes
        .iter()
        .map(|pl| {
            let n = pl.n();
            vec![rat(n[0]), rat(n[1]), rat(n[2]), -rat(pl.d())]
        })
        .collect();
    for col in 0..3 {
        let pivot = (col..3).find(|&r| !a[r][col].is_zero())?;
        a.swap(col, pivot);
        let inv = a[col][col].recip();
        for j in col..4 {
            let v = &a[col][j] * &inv;
            a[col][j] = v;
        }
        for r in 0..3 {
            if r != col && !a[r][col].is_zero() {
                let f = a[r][col].clone();
                for j in col..4 {
                    let v = &a[r][j] - &f * &a[col][j];
                    a[r][j] = v;
                }
            }
        }
    }
    Some([a[0][3].clone(), a[1][3].clone(), a[2][3].clone()])
}

/// The sign of n · pt + d at an exact rational point.
fn side_at_point(pl: &Plane, pt: &[BigRational; 3]) -> Sign {
    let mut v = rat(pl.d());
    for (ni, ci) in pl.n().iter().zip(pt) {
        v += ci * rat(*ni);
    }
    match v.cmp(&BigRational::zero()) {
        Ordering::Less => Sign::Negative,
        Ordering::Equal => Sign::Zero,
        Ordering::Greater => Sign::Positive,
    }
}

/// The side of the point where `planes` meet against `q`, as the sign of det[(`n_k`, `d_k`)] (rows
/// k = 1, 2, 3, then q), which equals `n_q` · X + `d_q` · w, times the sign of w. Also checks the
/// determinant against the largest value any grid input reaches, 4096 M^9 (372 bits).
fn side_by_det4(q: &Plane, planes: &[Plane; 3]) -> Sign {
    let rows: Vec<Vec<BigInt>> = planes
        .iter()
        .chain(std::iter::once(q))
        .map(|pl| {
            let n = pl.n();
            vec![big(n[0]), big(n[1]), big(n[2]), big(pl.d())]
        })
        .collect();
    let class = bareiss(rows);
    let cap = big(4096) * m_pow(9);
    assert!(
        class.abs() <= cap,
        "n · X + d · w = {class} exceeds 4096 M^9 for {planes:?} against {q:?}"
    );
    sign_of(&class).mul(sign_of(&det_normals(planes)))
}

/// r rounded to the nearest integer, ties to even, from its floor and fractional part.
fn round_half_even(r: &BigRational) -> BigInt {
    let floor = r.floor().to_integer();
    let twice_frac = (r - rat(floor.clone())) * rat(2);
    match twice_frac.cmp(&BigRational::one()) {
        Ordering::Less => floor,
        Ordering::Greater => floor + 1,
        Ordering::Equal => {
            if floor.is_even() {
                floor
            } else {
                floor + 1
            }
        }
    }
}

fn saturate(v: &BigInt) -> i128 {
    v.to_i128().unwrap_or(if v.is_negative() {
        i128::MIN
    } else {
        i128::MAX
    })
}

/// What `to_grid` must return for the exact point `pt`.
fn expect_grid(pt: &[BigRational; 3]) -> Result<Commit, NumError> {
    let mut out = [0i64; 3];
    for (o, c) in out.iter_mut().zip(pt) {
        let r = round_half_even(c);
        if r.abs() > big(M) {
            return Err(NumError::OutOfRange {
                value: saturate(&r),
            });
        }
        *o = r.to_i64().unwrap();
    }
    Ok(Commit {
        value: p3(out),
        exact: pt.iter().all(BigRational::is_integer),
    })
}

/// A commit moves each coordinate by at most half a step and the point by less than one step (the
/// ladder's commit rungs, `ladder::COMMIT_PER_AXIS` and `ladder::COMMIT_DISTANCE_3D`), a move of
/// exactly half a step lands on an even coordinate, and `exact` is true exactly when nothing moved.
fn check_half_step(r: &Commit, pt: &[BigRational; 3], ctx: &str) {
    // The ladder's commit rungs: half a step per axis, and under one step as a distance.
    let half = BigRational::new(BigInt::from(ladder::COMMIT_PER_AXIS.0), big(2));
    let distance = BigRational::from_integer(BigInt::from(ladder::COMMIT_DISTANCE_3D.get()));
    let got = coords(r.value);
    let square = |(c, x): (&i64, &BigRational)| {
        let d = rat(*c) - x;
        &d * &d
    };
    let squared: BigRational = got.iter().zip(pt).map(square).sum();
    assert!(
        squared <= &distance * &distance,
        "{ctx}: {} committed to {got:?}, a move of more than {distance} step",
        show(pt)
    );
    for (axis, (c, x)) in got.iter().zip(pt).enumerate() {
        let moved = (rat(*c) - x).abs();
        assert!(
            moved <= half,
            "{ctx}: axis {axis} of {} committed to {c}, a move of {moved} (over half a step)",
            show(pt)
        );
        if moved == half {
            assert_eq!(
                c.rem_euclid(2),
                0,
                "{ctx}: tie {x} on axis {axis} went to odd {c}"
            );
        }
    }
    let unmoved = got.iter().zip(pt).all(|(c, x)| rat(*c) == *x);
    assert_eq!(
        r.exact,
        unmoved,
        "{ctx}: exact flag of {} committed to {got:?}",
        show(pt)
    );
}

/// `Plane::through(a, b, c)` against the cofactor oracle: `Degenerate` exactly when every cofactor
/// is zero, never an over-budget error, the same coefficients, and within the documented reach
/// (|`n_i`| ≤ 4M², |d| ≤ 4M³). Returns the plane, or `None` when the points are collinear.
fn attack_plane(a: [i64; 3], b: [i64; 3], c: [i64; 3]) -> Option<Plane> {
    let (n, d) = cofactor_plane(a, b, c);
    let got = Plane::through(p3(a), p3(b), p3(c));
    if n.iter().all(Zero::is_zero) {
        assert_eq!(
            got,
            Err(NumError::Degenerate),
            "through({a:?}, {b:?}, {c:?}): collinear, expected Degenerate"
        );
        return None;
    }
    let pl = got.unwrap_or_else(|e| {
        panic!("through({a:?}, {b:?}, {c:?}): expected n = {n:?}, d = {d}, got {e:?}")
    });
    assert_eq!(
        pl.n().map(BigInt::from),
        n,
        "through({a:?}, {b:?}, {c:?}): normal"
    );
    assert_eq!(big(pl.d()), d, "through({a:?}, {b:?}, {c:?}): offset");
    let cap_n = big(4) * big(M) * big(M);
    let cap_d = &cap_n * big(M);
    assert!(
        n.iter().all(|v| v.abs() <= cap_n),
        "through({a:?}, {b:?}, {c:?}): n = {n:?} > 4M²"
    );
    assert!(
        d.abs() <= cap_d,
        "through({a:?}, {b:?}, {c:?}): d = {d} > 4M³"
    );
    Some(pl)
}

/// `side` on a grid point against the orientation oracle and against `orient3d`, which the
/// documentation of `through` says it equals.
fn check_grid_side(pl: &Plane, abc: [[i64; 3]; 3], q: [i64; 3]) {
    let [a, b, c] = abc;
    let want = orientation(a, b, c, q);
    let got = pl.side(p3(q)).unwrap();
    assert_eq!(
        got, want,
        "side of {q:?} against through({a:?}, {b:?}, {c:?}) = {pl:?}"
    );
    let o = orient3d(p3(a), p3(b), p3(c), p3(q)).unwrap();
    assert_eq!(
        o, got,
        "doc: through(a, b, c).side(p) is orient3d(a, b, c, p); {abc:?}, {q:?}"
    );
}

/// `Tpi::new`, `homogeneous` and `to_grid` against the oracles, and every defining plane classifies
/// the point as `Zero`. Returns the point and its exact position, or `None` (after checking every
/// order is `Degenerate`) when the planes do not meet in one point.
fn attack_tpi(planes: [Plane; 3], ctx: &str) -> Option<(Tpi, [BigRational; 3])> {
    let det = det_normals(&planes);
    if det.is_zero() {
        assert!(
            solve(&planes).is_none(),
            "{ctx}: oracle bug, det 0 but a unique solution"
        );
        for (perm, _) in PERMS {
            let [a, b, c] = permute(planes, perm);
            assert_eq!(
                Tpi::new(a, b, c),
                Err(NumError::Degenerate),
                "{ctx}: {planes:?} in order {perm:?} have det(n) = 0"
            );
        }
        return None;
    }
    let [a, b, c] = planes;
    let t = Tpi::new(a, b, c).unwrap_or_else(|e| {
        panic!("{ctx}: {planes:?} meet in one point (det(n) = {det}), Tpi::new gave {e:?}")
    });
    let pt = solve(&planes).expect("a non-zero determinant has a unique solution");
    let (x, w) = t.homogeneous().unwrap();
    let (xb, wb) = (x.map(|v| wide_big(&v)), wide_big(&w));
    assert_eq!(
        wb, det,
        "{ctx}: w of {planes:?}: expected det(n) = {det}, got {wb}"
    );
    for (axis, (xi, pi)) in xb.iter().zip(&pt).enumerate() {
        assert_eq!(
            BigRational::new(xi.clone(), wb.clone()),
            *pi,
            "{ctx}: X_{axis} / w of {planes:?}: expected {pi}, got {xi} / {wb}"
        );
    }
    assert!(
        wb.abs() <= big(256) * m_pow(6),
        "{ctx}: |w| = {wb} > 256 M^6 for {planes:?}"
    );
    assert!(
        xb.iter().all(|v| v.abs() <= big(256) * m_pow(7)),
        "{ctx}: X = {xb:?} > 256 M^7 for {planes:?}"
    );
    let want = expect_grid(&pt);
    let got = t.commit();
    assert_eq!(got, want, "{ctx}: to_grid of {} for {planes:?}", show(&pt));
    match got {
        Ok(r) => check_half_step(&r, &pt, ctx),
        Err(NumError::OutOfRange { value }) => assert!(
            value.unsigned_abs() > u128::from(M.unsigned_abs()),
            "{ctx}: OutOfRange carries {value}, inside the frame (0 marks a budget failure)"
        ),
        Err(e) => panic!("{ctx}: to_grid of {} gave {e:?}", show(&pt)),
    }
    for (k, pl) in planes.iter().enumerate() {
        assert_eq!(
            pl.side_of_tpi(&t).unwrap(),
            Sign::Zero,
            "{ctx}: defining plane {k} of {planes:?}"
        );
    }
    Some((t, pt))
}

/// `side_of_tpi` against both oracles, which must agree with each other first.
fn attack_side_of_tpi(q: &Plane, t: &Tpi, pt: &[BigRational; 3], ctx: &str) {
    let by_point = side_at_point(q, pt);
    let by_det = side_by_det4(q, t.planes());
    assert_eq!(
        by_det, by_point,
        "{ctx}: oracle bug, the two oracles disagree on {q:?}"
    );
    let got = q.side_of_tpi(t).unwrap();
    assert_eq!(
        got,
        by_point,
        "{ctx}: side_of_tpi of {q:?} at {} (planes {:?}): expected {by_point:?}, got {got:?}",
        show(pt),
        t.planes()
    );
}

// ---------------------------------------------------------------------------------------------
// Strategies.
// ---------------------------------------------------------------------------------------------

fn coord() -> impl Strategy<Value = i64> {
    prop_oneof![
        3 => -M..=M,
        3 => prop::sample::select(vec![M, -M, M - 1, -M + 1, M - 2, -M + 2, 0, 1, -1]),
        1 => (M - 16)..=M,
        1 => -M..=(-M + 16),
        1 => -4i64..=4,
    ]
}

fn point() -> impl Strategy<Value = [i64; 3]> {
    [coord(), coord(), coord()]
}

fn near_corner() -> impl Strategy<Value = i64> {
    prop::sample::select(vec![
        M,
        M - 1,
        M - 2,
        M - 3,
        -M,
        -M + 1,
        -M + 2,
        -M + 3,
        0,
        1,
        -1,
    ])
}

fn small_step() -> impl Strategy<Value = [i64; 3]> {
    [-1i64..=1, -1i64..=1, -1i64..=1]
}

// ---------------------------------------------------------------------------------------------
// 1. The bounds at their extremes.
// ---------------------------------------------------------------------------------------------

/// Every ordered triple of the frame's corners. Repeats are `Degenerate`; no three distinct
/// corners are collinear, so every other triple is a plane, exactly the cofactor oracle's and
/// exactly M² and M³ times the unit cube's. The documented largest values, |`n_i`| = 4M² and
/// |d| = 4M³, are reached, and by one plane at once; and the largest grid point against a plane,
/// |n · p + d| = 16M³ (124 bits, within the 126-bit budget of `side`), is reached too.
#[test]
fn corner_planes_reach_both_documented_maxima_at_once() {
    let cap_n = big(4) * big(M) * big(M);
    let cap_d = &cap_n * big(M);
    let mut top_n = BigInt::zero();
    let mut top_d = BigInt::zero();
    let mut top_side = BigInt::zero();
    let mut both_at_once = Vec::new();
    for (i, (&a, &ua)) in CORNERS.iter().zip(&UNIT).enumerate() {
        for (j, (&b, &ub)) in CORNERS.iter().zip(&UNIT).enumerate() {
            for (k, (&c, &uc)) in CORNERS.iter().zip(&UNIT).enumerate() {
                let got = Plane::through(p3(a), p3(b), p3(c));
                if i == j || j == k || i == k {
                    assert_eq!(
                        got,
                        Err(NumError::Degenerate),
                        "repeated: {a:?} {b:?} {c:?}"
                    );
                    continue;
                }
                let pl = attack_plane(a, b, c)
                    .unwrap_or_else(|| panic!("corners {a:?} {b:?} {c:?} are not collinear"));
                let un = cross_i(sub_i(ub, ua), sub_i(uc, ua));
                let ud = -dot_i(un, ua);
                let n = pl.n().map(BigInt::from);
                assert_eq!(n, un.map(|v| big(v) * m_pow(2)), "{a:?} {b:?} {c:?}");
                assert_eq!(big(pl.d()), big(ud) * m_pow(3), "{a:?} {b:?} {c:?}");
                let d = big(pl.d());
                for v in &n {
                    top_n = top_n.max(v.abs());
                }
                top_d = top_d.max(d.abs());
                if n.iter().all(|v| v.abs() == cap_n) && d.abs() == cap_d {
                    both_at_once.push([a, b, c]);
                }
                for &q in &CORNERS {
                    check_grid_side(&pl, [a, b, c], q);
                    let np: BigInt = n.iter().zip(q).map(|(ni, qi)| ni * big(qi)).sum();
                    let value = np + &d;
                    top_side = top_side.max(value.abs());
                }
            }
        }
    }
    assert_eq!(
        top_n, cap_n,
        "the largest |n_i| over the frame's corners must be 4M²"
    );
    assert_eq!(
        top_d, cap_d,
        "the largest |d| over the frame's corners must be 4M³"
    );
    assert_eq!(
        (top_n.bits(), top_d.bits()),
        (82, 122),
        "module docs: n, d reach 82, 122 bits"
    );
    assert!(
        !both_at_once.is_empty(),
        "no corner plane has every |n_i| = 4M² and |d| = 4M³"
    );
    assert_eq!(
        top_side,
        big(16) * m_pow(3),
        "largest |n · p + d| at the corners"
    );
    assert!(top_side.bits() <= 126, "side's documented budget is 2^126");
}

/// Every distinct oriented plane through three corners of the frame's cube, with its coefficients
/// on the unit cube (n = M² n', d = M³ d').
fn corner_planes() -> Vec<(Plane, [i64; 3], i64)> {
    let mut seen: BTreeMap<([i64; 3], i64), [usize; 3]> = BTreeMap::new();
    for (i, &a) in UNIT.iter().enumerate() {
        for (j, &b) in UNIT.iter().enumerate() {
            for (k, &c) in UNIT.iter().enumerate() {
                if i == j || j == k || i == k {
                    continue;
                }
                let n = cross_i(sub_i(b, a), sub_i(c, a));
                let d = -dot_i(n, a);
                seen.entry((n, d)).or_insert([i, j, k]);
            }
        }
    }
    seen.into_iter()
        .map(|((n, d), [i, j, k])| {
            let pl = Plane::through(p3(CORNERS[i]), p3(CORNERS[j]), p3(CORNERS[k])).unwrap();
            (pl, n, d)
        })
        .collect()
}

/// All 64,000 ordered triples of the 40 oriented corner planes. Dependent triples are
/// `Degenerate`; the others have w = M^6 w' and X = M^7 X' exactly (unit-cube Cramer in `i64`),
/// X / w lies on all three planes (checked by substitution, independent of Cramer), and the
/// commit is the rational point rounded half to even (corner points include ties such as M / 2).
/// For every unordered triple and every corner plane as the fourth, `side_of_tpi` is the sign of
/// the 4 × 4 determinant times the sign of w. The largest values are exactly the multilinear box
/// bounds 256 M^6, 256 M^7 and 4096 M^9, which bound every grid input, so the module's "reach 248,
/// 288 and 372 bits" is the largest the frame allows, and its budgets 2^252, 2^294 and 2^379 hold.
#[test]
fn corner_tpis_reach_the_largest_w_x_and_classification_exactly() {
    let planes = corner_planes();
    assert_eq!(
        planes.len(),
        40,
        "6 faces, 6 diagonals and 8 corner cuts, both orientations"
    );
    let m = big(M);
    let (m6, m7) = (m_pow(6), m_pow(7));
    let (mut top_w, mut top_x, mut top_class) = (0i64, 0i64, 0i64);
    let mut ties = 0usize;
    for (i, (pi, ni, di)) in planes.iter().enumerate() {
        for (j, (pj, nj, dj)) in planes.iter().enumerate() {
            for (k, (pk, nk, dk)) in planes.iter().enumerate() {
                let ctx = format!("corner planes {i}, {j}, {k}: {pi:?} {pj:?} {pk:?}");
                let wu = det3_i([*ni, *nj, *nk]);
                let got = Tpi::new(*pi, *pj, *pk);
                if wu == 0 {
                    assert_eq!(got, Err(NumError::Degenerate), "{ctx}: dependent normals");
                    continue;
                }
                let t = got.unwrap_or_else(|e| panic!("{ctx}: w' = {wu}, Tpi::new gave {e:?}"));
                let rhs = [-*di, -*dj, -*dk];
                let xu: [i64; 3] = std::array::from_fn(|col| {
                    let mut rows = [*ni, *nj, *nk];
                    for (row, r) in rows.iter_mut().zip(rhs) {
                        row[col] = r;
                    }
                    det3_i(rows)
                });
                let (x, w) = t.homogeneous().unwrap();
                let (xb, wb) = (x.map(|v| wide_big(&v)), wide_big(&w));
                assert_eq!(wb, big(wu) * &m6, "{ctx}: w");
                assert_eq!(xb, xu.map(|v| big(v) * &m7), "{ctx}: X");
                for pl in [pi, pj, pk] {
                    let nx: BigInt = pl.n().iter().zip(&xb).map(|(a, b)| big(*a) * b).sum();
                    let on = nx + big(pl.d()) * &wb;
                    assert!(
                        on.is_zero(),
                        "{ctx}: X / w is not on {pl:?}: n · X + d · w = {on}"
                    );
                }
                top_w = top_w.max(wu.abs());
                top_x = top_x.max(xu.iter().map(num_traits::Signed::abs).max().unwrap());
                let pt = xu.map(|v| BigRational::new(big(v) * &m, big(wu)));
                if pt
                    .iter()
                    .any(|v| !v.is_integer() && (v * rat(2)).is_integer())
                {
                    ties += 1;
                }
                let want = expect_grid(&pt);
                assert_eq!(t.commit(), want, "{ctx}: to_grid of {}", show(&pt));
                if i < j && j < k {
                    for (l, (q, nq, dq)) in planes.iter().enumerate() {
                        let cu = det4_i([
                            [ni[0], ni[1], ni[2], *di],
                            [nj[0], nj[1], nj[2], *dj],
                            [nk[0], nk[1], nk[2], *dk],
                            [nq[0], nq[1], nq[2], *dq],
                        ]);
                        // The identity side_by_det4 rests on: det[(n_k, d_k)] = n_q · X + d_q · w.
                        assert_eq!(
                            cu,
                            dot_i(*nq, xu) + dq * wu,
                            "{ctx}: det4 identity, plane {l}"
                        );
                        let want = Sign::of_i64(cu).mul(Sign::of_i64(wu));
                        let got = q.side_of_tpi(&t).unwrap();
                        assert_eq!(got, want, "{ctx}: side_of_tpi of corner plane {l} {q:?}");
                        top_class = top_class.max(cu.abs());
                    }
                }
            }
        }
    }
    assert!(
        ties > 0,
        "the corner triples include ties, so the commit's tie rule was exercised"
    );
    assert_eq!(
        (top_w, top_x, top_class),
        (256, 256, 4096),
        "unit-cube maxima of w', X', det4'"
    );
    let (cap_w, cap_x, cap_c) = (big(256) * &m6, big(256) * &m7, big(4096) * m_pow(9));
    assert_eq!(
        (cap_w.bits(), cap_x.bits(), cap_c.bits()),
        (248, 288, 372),
        "module docs: reached"
    );
    assert!(
        cap_w.bits() < 252 && cap_x.bits() < 294 && cap_c.bits() < 379,
        "within the budgets"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Points one to three steps in from the corners: never over the corner maxima (which bound
    /// every grid input), and every result exact against the oracles.
    #[test]
    fn near_corner_inputs_stay_within_the_corner_maxima(
        pts in prop::collection::vec([near_corner(), near_corner(), near_corner()], 12)
    ) {
        let made: Vec<Option<Plane>> =
            pts.chunks(3).map(|c| attack_plane(c[0], c[1], c[2])).collect();
        if let [Some(p1), Some(p2), Some(p3_), Some(q)] = made[..] {
            if let Some((t, pt)) = attack_tpi([p1, p2, p3_], "near corners") {
                attack_side_of_tpi(&q, &t, &pt, "near corners");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 2. The I512 fallback (the total path), recomputed at the types' own extremes.
// ---------------------------------------------------------------------------------------------

/// The fallback of `side_of_tpi` and `to_grid` is claimed to be unable to overflow for any value
/// the types hold: |`n_i`|, |d| ≤ 2^127, |`X_i`| ≤ 2^319, |w| ≤ 2^255. The private functions cannot
/// be reached with such values, so this runs the same `Wide` operations on them, with every sign
/// chosen to make every term the same sign (the largest partial sums), and recomputes the
/// documented figures in big integers. It also shows the `I384` path does depend on the plane
/// budget: one type-extreme product does not fit `I384`.
#[test]
fn i512_fallback_cannot_overflow_for_any_value_the_types_hold() {
    let cases = [
        (i128::MIN, I320::min_value(), i128::MIN, I256::min_value()),
        (i128::MIN, I320::max_value(), i128::MAX, I256::min_value()),
        (i128::MAX, I320::max_value(), i128::MAX, I256::max_value()),
        (i128::MAX, I320::min_value(), i128::MIN, I256::max_value()),
    ];
    for (n, x, d, w) in cases {
        let ctx = format!("n_i = {n}, X_i = {x:?}, d = {d}, w = {w:?}");
        let mut acc = I512::from_i128(d)
            .checked_mul(&w.resize::<8>().unwrap())
            .unwrap_or_else(|| panic!("{ctx}: d · w overflowed I512"));
        let mut want = big(d) * wide_big(&w);
        for axis in 0..3 {
            let term = I512::from_i128(n)
                .checked_mul(&x.resize::<8>().unwrap())
                .unwrap_or_else(|| panic!("{ctx}: n_{axis} · X_{axis} overflowed I512"));
            acc = acc
                .checked_add(&term)
                .unwrap_or_else(|| panic!("{ctx}: partial sum {axis} overflowed"));
            want += big(n) * wide_big(&x);
        }
        assert_eq!(wide_big(&acc), want, "{ctx}: I512 classification sum");
        assert!(
            want.abs() < BigInt::one() << 448u32,
            "{ctx}: docs say every partial sum < 2^448"
        );
        assert!(
            Wide::<6>::from_i128(n)
                .checked_mul(&x.resize::<6>().unwrap())
                .is_none(),
            "{ctx}: a type-extreme product fits I384, so the fallback would be pointless"
        );
    }
    let two = BigInt::from(2);
    assert_eq!(
        big(i128::MIN).abs() * wide_big(&I320::min_value()).abs(),
        two.pow(446)
    );
    assert_eq!(
        big(i128::MIN).abs() * wide_big(&I256::min_value()).abs(),
        two.pow(382)
    );

    // The commit's steps at |X_i| = 2^319 and |w| = 2^255, in the order `round_axis` runs them.
    let w = I256::min_value()
        .resize::<8>()
        .unwrap()
        .checked_neg()
        .expect("2^255 fits I512");
    let mag = I320::min_value()
        .resize::<8>()
        .unwrap()
        .checked_neg()
        .expect("2^319 fits I512");
    let twice = mag.checked_add(&mag).expect("2 |X_i| = 2^320 fits I512");
    let frame = I512::from_i64(2 * M + 1)
        .checked_mul(&w)
        .expect("(2^41 − 1) w fits I512");
    let half = I512::from_i128(1 << 126);
    let sat = half
        .checked_add(&half)
        .unwrap()
        .checked_mul(&w)
        .expect("2^127 w fits I512");
    let top = I512::from_i128(1 << 126)
        .checked_mul(&w)
        .expect("T = 2^126 w fits I512");
    let doubled = top.checked_add(&top).expect("2T fits I512");
    assert_eq!(wide_big(&twice), two.pow(320));
    assert!(
        wide_big(&frame) < two.pow(296),
        "docs: (2^41 − 1) w < 2^296"
    );
    assert_eq!(wide_big(&sat), two.pow(382), "docs: 2^127 w ≤ 2^382");
    assert_eq!(
        wide_big(&doubled),
        two.pow(382),
        "docs: the division stays below 2^382"
    );
    // The largest scaled remainder the division can hold before a subtraction is just under 2T.
    let below = doubled.checked_sub(&I512::one()).unwrap();
    assert!(
        below.checked_add(&below).is_some(),
        "doubling a remainder below 2T fits I512"
    );
}

// ---------------------------------------------------------------------------------------------
// 3. Degenerate and nearly degenerate triples.
// ---------------------------------------------------------------------------------------------

fn degenerate_in_every_order(name: &str, planes: [Plane; 3]) {
    assert!(
        det_normals(&planes).is_zero(),
        "{name}: oracle says {planes:?} are independent"
    );
    for (perm, _) in PERMS {
        let [a, b, c] = permute(planes, perm);
        assert_eq!(
            Tpi::new(a, b, c),
            Err(NumError::Degenerate),
            "{name}: {planes:?} in order {perm:?}"
        );
    }
}

/// Parallel, anti-parallel, repeated, rescaled and reversed copies of one plane, pencils (three
/// planes through one line) and prisms (normals coplanar, no common line), all at the frame's
/// extremes, in every order. A one-step change to each pencil makes it a point, checked exactly.
#[test]
fn degenerate_triples_at_the_extremes_in_every_order() {
    let m2 = 2 * M;
    let floor = plane_at([-M, -M, -M], [m2, 0, 0], [0, m2, 0]).unwrap();
    let floor_reversed = plane_at([-M, -M, -M], [0, m2, 0], [m2, 0, 0]).unwrap();
    let floor_small = plane_at([0, 0, -M], [1, 0, 0], [0, 1, 0]).unwrap();
    let ceiling = plane_at([-M, -M, M], [m2, 0, 0], [0, m2, 0]).unwrap();
    let ceiling_down = plane_at([-M, -M, M], [0, m2, 0], [m2, 0, 0]).unwrap();
    let wall = plane_at([-M, -M, -M], [0, m2, 0], [0, 0, m2]).unwrap();
    let side = plane_at([-M, -M, -M], [0, 0, m2], [m2, 0, 0]).unwrap();
    let cut = plane_at([-M, -M, M], [0, m2, -m2], [m2, 0, -m2]).unwrap();
    assert_eq!(floor.n(), [0, 0, 4 * i128::from(M) * i128::from(M)]);
    assert_eq!(
        floor_small.n(),
        [0, 0, 1],
        "the same point set as floor, other coefficients"
    );
    assert_ne!(floor, floor_small);
    let (lo, hi) = ([-M, -M, -M], [M, M, M]);
    let (x_end, y_end, z_end) = ([M, -M, -M], [-M, M, -M], [-M, -M, M]);
    let pencil = |c: [i64; 3]| Plane::through(p3(lo), p3(hi), p3(c)).unwrap();
    let pencil_half = Plane::through(p3(lo), p3([0, 0, 0]), p3([M, -M, -M])).unwrap();
    let edge = |c: [i64; 3]| Plane::through(p3([M, M, -M]), p3([M, M, M]), p3(c)).unwrap();
    let prism = plane_at([0, 0, 0], [0, 0, M], [M, -M, 0]).unwrap();
    let prism_other = plane_at([0, 0, 0], [0, 0, 1], [1, 1, 0]).unwrap();
    let cases: Vec<(&str, [Plane; 3])> = vec![
        ("parallel floor and ceiling", [floor, ceiling, wall]),
        (
            "anti-parallel floor and ceiling",
            [floor, ceiling_down, wall],
        ),
        (
            "anti-parallel, both the frame's faces",
            [floor, ceiling_down, cut],
        ),
        ("floor given twice", [floor, floor, wall]),
        ("floor given twice, apart", [floor, wall, floor]),
        ("floor given three times", [floor, floor, floor]),
        ("floor and its reverse", [floor, floor_reversed, side]),
        ("floor and a rescaled copy", [floor, floor_small, cut]),
        (
            "rescaled and reversed copies",
            [floor_small, floor_reversed, wall],
        ),
        (
            "pencil about the space diagonal",
            [pencil(x_end), pencil(y_end), pencil(z_end)],
        ),
        (
            "pencil with a near-origin plane",
            [pencil(x_end), pencil([0, 1, 0]), pencil(z_end)],
        ),
        (
            "pencil with a rescaled copy",
            [pencil(x_end), pencil_half, pencil(y_end)],
        ),
        (
            "pencil about an edge",
            [edge([-M, -M, -M]), edge([-M, M, 0]), edge([0, -M, M])],
        ),
        ("prism", [wall, side, prism]),
        ("prism about the other diagonal", [wall, side, prism_other]),
    ];
    for (name, planes) in cases {
        degenerate_in_every_order(name, planes);
    }
    // One step off each pencil: a single point, exactly where the oracles say.
    let off_diagonal = Plane::through(p3(lo), p3([M, M, M - 1]), p3(y_end)).unwrap();
    let off_edge = Plane::through(p3([M, M - 1, -M]), p3([M, M, M]), p3([0, -M, M])).unwrap();
    let off_prism = plane_at([0, 0, 0], [0, 1, M], [M, -M, 0]).unwrap();
    let near = [
        [pencil(x_end), off_diagonal, pencil(z_end)],
        [edge([-M, -M, -M]), edge([-M, M, 0]), off_edge],
        [wall, side, off_prism],
    ];
    for planes in near {
        let (t, pt) = attack_tpi(planes, "one step off a pencil or prism").expect("a point");
        for q in [floor, wall, side, cut, prism] {
            attack_side_of_tpi(&q, &t, &pt, "one step off a pencil or prism");
        }
    }
}

/// The plane z = 0 against a plane through (−M, −M, h + `δ_1`), (M, −M, h + `δ_2`) and
/// (−M, M, h + `δ_3`) for every δ in {−1, 0, 1}³: parallel when every δ is equal, otherwise tilted
/// by one step across the frame. With a third plane, in every order: `Degenerate` exactly when
/// the oracle's determinant is zero, otherwise the exact point, commit and sides.
#[test]
fn planes_one_step_from_parallel() {
    let m2 = 2 * M;
    let base = plane_at([-M, -M, 0], [m2, 0, 0], [0, m2, 0]).unwrap();
    let thirds = [
        plane_at([0, -M, -M], [0, m2, 0], [0, 0, m2]).unwrap(),
        plane_at([-M, 0, -M], [0, 0, m2], [m2, 0, 0]).unwrap(),
        plane_at([-M, -M, M], [0, m2, -m2], [m2, 0, -m2]).unwrap(),
        Plane::through(p3([M, -M, 1]), p3([-M, M, -1]), p3([0, 3, M])).unwrap(),
    ];
    let mut points = 0usize;
    let mut degenerate = 0usize;
    for h in [1, -1, 2, M - 1, -(M - 1)] {
        for d1 in -1i64..=1 {
            for d2 in -1i64..=1 {
                for d3 in -1i64..=1 {
                    let pts = [[-M, -M, h + d1], [M, -M, h + d2], [-M, M, h + d3]];
                    let tilted = Plane::through(p3(pts[0]), p3(pts[1]), p3(pts[2])).unwrap();
                    for (c_ix, third) in thirds.iter().enumerate() {
                        for (perm, _) in PERMS {
                            let ctx =
                                format!("h {h}, δ ({d1}, {d2}, {d3}), third {c_ix}, {perm:?}");
                            let planes = permute([base, tilted, *third], perm);
                            match attack_tpi(planes, &ctx) {
                                Some((t, pt)) => {
                                    points += 1;
                                    for q in [base, tilted, thirds[0], thirds[2], thirds[3]] {
                                        attack_side_of_tpi(&q, &t, &pt, &ctx);
                                    }
                                }
                                None => degenerate += 1,
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(
        points > 1000 && degenerate > 100,
        "{points} points, {degenerate} degenerate"
    );
}

/// The third plane's two spanning vectors, |w| and the largest |`X_i`| in bits, for one design.
type Design = (([i64; 3], [i64; 3]), i64, u64);

/// Two planes through u = (p, 0, 1) whose normals differ by one unit in two components
/// (`n_1` × `n_2` = (−p, 0, −1)), s steps apart, so their line runs about s · 2^123 steps from the
/// origin; a third plane nearly containing that line meets it with |w| = 1, 2 or 3 (|X| from
/// about 2^121 to 2^129: reported exactly up to 2^127 − 1 and saturated beyond, which brackets
/// the saturation threshold 2^127 · w from both sides), or with w = −1 about s · 2^164 steps away
/// (far beyond `i128`). Every order, both orientations, and the mirror image.
#[test]
fn planes_one_normal_unit_apart_meet_far_away() {
    let (p, q) = (2 * M, 2 * M - 1);
    let a1 = [-M, -M, -M];
    let build = |mirror: bool, s: i64, third: ([i64; 3], [i64; 3])| -> [Plane; 3] {
        let f = |a: [i64; 3], u: [i64; 3], v: [i64; 3]| {
            let (a, u, v) = if mirror {
                (neg(a), neg(u), neg(v))
            } else {
                (a, u, v)
            };
            plane_at(a, u, v).unwrap()
        };
        let a2 = add(a1, [0, 0, s]);
        [
            f(a1, [p, 0, 1], [p - 1, q, 1]),
            f(a2, [p, 0, 1], [p - 1, q + 1, 1]),
            f(a1, third.0, third.1),
        ]
    };
    // The third plane, |w|, and the largest |X_i| in bits at s = 1 (computed independently).
    let designs: [Design; 4] = [
        (([p - 1, 0, 1], [0, 1, 0]), 1, 123),
        (([p - 2, 0, 1], [0, 1, 0]), 2, 124),
        (([p - 3, 0, 1], [0, 1, 0]), 3, 125),
        (([p, 1, 1], [p - 1, 0, 1]), 1, 164),
    ];
    let probes: Vec<Plane> = corner_planes().iter().map(|c| c.0).take(12).collect();
    let two126 = BigInt::one() << 126u32;
    let mut window = 0usize;
    let mut saturated = [0usize; 2];
    for s in [1, 8, 12, 15, 16, 17, 32] {
        for (third, w_abs, x_bits) in designs {
            for mirror in [false, true] {
                let planes = build(mirror, s, third);
                let n1 = planes[0].n().map(BigInt::from);
                let n2 = planes[1].n().map(BigInt::from);
                let c12 = [
                    &n1[1] * &n2[2] - &n1[2] * &n2[1],
                    &n1[2] * &n2[0] - &n1[0] * &n2[2],
                    &n1[0] * &n2[1] - &n1[1] * &n2[0],
                ];
                assert_eq!(c12, [big(-p), big(0), big(-1)], "design: n_1 × n_2");
                for (perm, _) in PERMS {
                    let ctx = format!("far design {third:?}, s {s}, mirror {mirror}, {perm:?}");
                    let (t, pt) = attack_tpi(permute(planes, perm), &ctx).expect("a point");
                    let (x, w) = t.homogeneous().unwrap();
                    assert_eq!(wide_big(&w).abs(), big(w_abs), "{ctx}: |w|");
                    if s == 1 {
                        let bits = x.iter().map(|v| wide_big(v).bits()).max().unwrap();
                        assert_eq!(bits, x_bits, "{ctx}: the design's reach");
                    }
                    for c in &pt {
                        let r = round_half_even(c);
                        if r.abs() >= two126 && r.to_i128().is_some_and(|v| v != i128::MAX) {
                            window += 1;
                        }
                    }
                    match t.commit() {
                        Err(NumError::OutOfRange { value: i128::MAX }) => saturated[0] += 1,
                        Err(NumError::OutOfRange { value: i128::MIN }) => saturated[1] += 1,
                        _ => {}
                    }
                    for q in probes.iter().chain(&planes) {
                        attack_side_of_tpi(q, &t, &pt, &ctx);
                    }
                }
            }
        }
    }
    assert!(
        window > 0,
        "no coordinate landed in [2^126, 2^127 − 1), where nothing may saturate"
    );
    assert!(
        saturated[0] > 0 && saturated[1] > 0,
        "saturated reports {saturated:?}"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(384))]

    /// Three planes that all nearly contain the line through a and b (each through a and b moved
    /// by at most one step per coordinate): w is tiny against |n|³, so the points range from inside
    /// the frame to far beyond `i128`. Every result against the oracles.
    #[test]
    fn near_pencils(a in point(), b in point(), cs in prop::collection::vec(point(), 3),
                    ds in prop::collection::vec(small_step(), 4),
                    qs in prop::collection::vec(point(), 6), perm in 0usize..6) {
        let nudge = |x: [i64; 3], d: [i64; 3]| -> [i64; 3] {
            std::array::from_fn(|i| (x[i] + d[i]).clamp(-M, M))
        };
        let tri = [
            [a, b, cs[0]],
            [nudge(a, ds[0]), nudge(b, ds[1]), cs[1]],
            [nudge(a, ds[2]), nudge(b, ds[3]), cs[2]],
        ];
        let made: Option<Vec<Plane>> = tri.iter().map(|t| attack_plane(t[0], t[1], t[2])).collect();
        let Some(made) = made else { return Ok(()) };
        let planes = permute([made[0], made[1], made[2]], PERMS[perm].0);
        if let Some((t, pt)) = attack_tpi(planes, "near pencil") {
            for q in qs.chunks(3) {
                if let Some(qp) = attack_plane(q[0], q[1], q[2]) {
                    attack_side_of_tpi(&qp, &t, &pt, "near pencil");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 4. Commits: ties, half steps, the frame's edge and the order of reports.
// ---------------------------------------------------------------------------------------------

/// The plane `x_i` + `x_j` = s through grid points a (`a_i` + `a_j` = s, `a_m` = 0), a + k
/// (`e_i` − `e_j`) and a + l `e_m`, with `axes` = [i, j, m]: its normal is ±k l (`e_i` + `e_j`).
/// Tries the four sign choices of k and l, reversed if `flip`, and returns the first whose points
/// all lie in the frame.
fn sum_plane(axes: [usize; 3], s: i64, k: i64, l: i64, flip: bool) -> Option<Plane> {
    let [i, j, m] = axes;
    let h = s.div_euclid(2);
    let mut a = [0i64; 3];
    a[i] = h;
    a[j] = s - h;
    for (sk, sl) in [(1, 1), (-1, 1), (1, -1), (-1, -1)] {
        let mut u = [0i64; 3];
        u[i] = sk * k;
        u[j] = -sk * k;
        let mut v = [0i64; 3];
        v[m] = sl * l;
        let (u, v) = if flip { (v, u) } else { (u, v) };
        if let Some(pl) = plane_at(a, u, v) {
            return Some(pl);
        }
    }
    None
}

/// Planes x + y = A, y + z = B and z + x = C with scales `kl` and reversals `flips`, meeting at
/// the point whose doubled coordinates are `doubled` (all of one parity): A = (x2 + y2) / 2 and so
/// on, w = ±2 Π k l.
fn sums_meeting_at(doubled: [i64; 3], kl: [(i64, i64); 3], flips: [bool; 3]) -> Option<[Plane; 3]> {
    let [x2, y2, z2] = doubled;
    let s = [(x2 + y2) / 2, (y2 + z2) / 2, (z2 + x2) / 2];
    let axes = [[0, 1, 2], [1, 2, 0], [2, 0, 1]];
    let made: Option<Vec<Plane>> = (0..3)
        .map(|n| sum_plane(axes[n], s[n], kl[n].0, kl[n].1, flips[n]))
        .collect();
    made.map(|v| [v[0], v[1], v[2]])
}

/// Exact ties and their neighbours, by hand: ±1/2 → 0, ±3/2 → ±2, ±5/2 → ±2, ±(M − 3/2) and
/// ±(M − 1/2) → ±(M − 1) (M is odd), ±(M + 1/2) → ±2^40, outside the frame. Each with |w| from 2
/// to about 2^235, both signs of w (scale signs and reversed planes), in every order. Out-of-range
/// reports name the first axis outside, in the order x, y, z, with the exact rounded value.
#[test]
fn ties_round_half_to_even_at_every_scale_and_sign() {
    // A point's coordinates times two, and its commit or the coordinate it reports.
    type Case = ([i64; 3], Result<[i64; 3], i128>);
    let e40 = 1i128 << 40;
    let mm = M - 1;
    let cases: [Case; 11] = [
        ([1, -1, 3], Ok([0, 0, 2])),
        ([-3, 5, -5], Ok([-2, 2, -2])),
        ([-1, 1, -3], Ok([0, 0, -2])),
        ([2 * M - 1, -(2 * M - 1), 1], Ok([mm, -mm, 0])),
        ([2 * M - 3, 2 * M - 1, -(2 * M - 1)], Ok([mm, mm, -mm])),
        ([-(2 * M - 3), 1, 5], Ok([-mm, 0, 2])),
        ([2, -4, 6], Ok([1, -2, 3])),
        ([2 * M, -2 * M, 0], Ok([M, -M, 0])),
        ([2 * M + 1, 1, -(2 * M + 1)], Err(e40)),
        ([1, -(2 * M + 1), 2 * M + 1], Err(-e40)),
        ([1, 3, -(2 * M + 1)], Err(-e40)),
    ];
    let scales: [[(i64, i64); 3]; 5] = [
        [(1, 1), (1, 1), (1, 1)],
        [(2, 1), (1, 3), (5, 7)],
        [
            ((1 << 20) + 1, (1 << 19) - 1),
            (3, (1 << 30) + 7),
            ((1 << 33) - 9, 11),
        ],
        [
            ((1 << 39) - 1, (1 << 39) - 3),
            ((1 << 39) - 5, (1 << 39) - 7),
            ((1 << 39) - 9, M),
        ],
        [(M, 1), (1, M), (M, M)],
    ];
    let mut built = 0usize;
    let mut largest_w = BigInt::zero();
    for (doubled, want) in cases {
        let exact = doubled.iter().all(|v| v % 2 == 0);
        let want: Result<Commit, NumError> = match want {
            Ok(c) => Ok(Commit {
                value: p3(c),
                exact,
            }),
            Err(value) => Err(NumError::OutOfRange { value }),
        };
        for kl in scales {
            for bits in 0..8u8 {
                let flips = [bits & 1 == 1, bits & 2 == 2, bits & 4 == 4];
                let Some(planes) = sums_meeting_at(doubled, kl, flips) else {
                    continue;
                };
                for (perm, _) in PERMS {
                    let ctx = format!("doubled {doubled:?}, scales {kl:?}, {flips:?}, {perm:?}");
                    let (t, pt) = attack_tpi(permute(planes, perm), &ctx).expect("a point");
                    let half: [BigRational; 3] = doubled.map(|v| BigRational::new(big(v), big(2)));
                    assert_eq!(pt, half, "{ctx}: construction");
                    assert_eq!(t.commit(), want, "{ctx}: the hand-computed commit");
                    let (_, w) = t.homogeneous().unwrap();
                    largest_w = largest_w.max(wide_big(&w).abs());
                    built += 1;
                }
            }
        }
    }
    assert!(
        built > 1500,
        "only {built} tie configurations fitted the frame"
    );
    assert!(
        largest_w.bits() >= 230,
        "largest |w| exercised: {} bits",
        largest_w.bits()
    );
}

/// The plane x − z = s through grid points a (`a_x` − `a_z` = s), a ± k (1, 0, 1) and a ± l `e_y`,
/// reversed if `flip`; the first sign choice whose points lie in the frame.
fn diff_plane(s: i64, k: i64, l: i64, flip: bool) -> Option<Plane> {
    let h = s.div_euclid(2);
    let a = [h, 0, h - s];
    for (sk, sl) in [(1, 1), (-1, 1), (1, -1), (-1, -1)] {
        let (u, v) = ([sk * k, 0, sk * k], [0, sl * l, 0]);
        let (u, v) = if flip { (v, u) } else { (u, v) };
        if let Some(pl) = plane_at(a, u, v) {
            return Some(pl);
        }
    }
    None
}

/// The plane y = y0 through (0, y0, 0), (±k, y0, 0) and (0, y0, ±l), reversed if `flip`; the
/// first sign choice whose points lie in the frame.
fn level_plane(y0: i64, k: i64, l: i64, flip: bool) -> Option<Plane> {
    let a = [0, y0, 0];
    for (sk, sl) in [(1, 1), (-1, 1), (1, -1), (-1, -1)] {
        let (u, v) = ([sk * k, 0, 0], [0, 0, sl * l]);
        let (u, v) = if flip { (v, u) } else { (u, v) };
        if let Some(pl) = plane_at(a, u, v) {
            return Some(pl);
        }
    }
    None
}

/// x + z = C, x − z = s and y = y0: ties on x and z with y on the grid, so `exact` must be false
/// although one coordinate did not move; and the same with every coordinate on the grid.
#[test]
fn exact_flag_needs_every_coordinate_on_the_grid() {
    let cases: [([i64; 3], i64); 6] = [
        ([1, 0, 3], 7),
        ([-1, 0, 1], -M),
        ([2 * M - 1, 0, -(2 * M - 1)], M),
        ([2, 0, -4], 5),
        ([2 * M, 0, 0], -M),
        ([-(2 * M + 1), 0, 1], 0),
    ];
    for (doubled_xz, y0) in cases {
        let [x2, _, z2] = doubled_xz;
        for (k, l) in [(1, 1), (3, 5), ((1 << 39) - 1, M)] {
            for flip in [false, true] {
                let made = [
                    sum_plane([2, 0, 1], (x2 + z2) / 2, k, l, flip),
                    diff_plane((x2 - z2) / 2, k, l, !flip),
                    level_plane(y0, k, l, flip),
                ];
                let [Some(p1), Some(p2), Some(p3_)] = made else {
                    continue;
                };
                let ctx = format!("doubled x, z {x2}, {z2}, y {y0}, scales {k}, {l}, flip {flip}");
                let (t, pt) = attack_tpi([p1, p2, p3_], &ctx).expect("a point");
                let half = |v: i64| BigRational::new(big(v), big(2));
                let want = [half(x2), rat(y0), half(z2)];
                assert_eq!(pt, want, "{ctx}: construction");
                if let Ok(r) = t.commit() {
                    let all_integral = x2 % 2 == 0 && z2 % 2 == 0;
                    assert_eq!(r.exact, all_integral, "{ctx}: exact flag of {r:?}");
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Planes through points near a grid point g and through far extreme points: the point lies
    /// near g with a large w. The commit moves each coordinate by at most half a step, ties go to
    /// even, `exact` is right, and everything matches the oracles.
    #[test]
    fn commits_move_at_most_half_a_step(
        g in point(), offs in prop::collection::vec([-2i64..=2, -2i64..=2, -2i64..=2], 3),
        far in prop::collection::vec(point(), 6), qs in prop::collection::vec(point(), 3),
        perm in 0usize..6
    ) {
        let near: Vec<[i64; 3]> = offs
            .iter()
            .map(|o| std::array::from_fn(|i| (g[i] + o[i]).clamp(-M, M)))
            .collect();
        let made: Option<Vec<Plane>> =
            (0..3).map(|k| attack_plane(near[k], far[2 * k], far[2 * k + 1])).collect();
        let Some(made) = made else { return Ok(()) };
        let planes = permute([made[0], made[1], made[2]], PERMS[perm].0);
        if let Some((t, pt)) = attack_tpi(planes, "near a grid point") {
            if let Some(q) = attack_plane(qs[0], qs[1], qs[2]) {
                attack_side_of_tpi(&q, &t, &pt, "near a grid point");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 5. side_of_tpi against exact oracles.
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Three planes through a common grid point g (and random, often extreme, other points): the
    /// point is exactly g with a large w. It commits to g exactly; every plane classifies it as it
    /// classifies the grid point g with `side` (an oracle tested against `orient3d`); a plane
    /// through g gives `Zero`, and a plane through g moved one step gives the grid answer.
    #[test]
    fn tpis_at_a_grid_point_classify_like_the_grid_point(
        g in point(), others in prop::collection::vec(point(), 6),
        qs in prop::collection::vec(point(), 6), step in small_step(), perm in 0usize..6
    ) {
        let made: Option<Vec<Plane>> =
            (0..3).map(|k| attack_plane(g, others[2 * k], others[2 * k + 1])).collect();
        let Some(made) = made else { return Ok(()) };
        let planes = permute([made[0], made[1], made[2]], PERMS[perm].0);
        let Some((t, pt)) = attack_tpi(planes, "at a grid point") else { return Ok(()) };
        prop_assert_eq!(pt.clone(), g.map(rat), "construction");
        prop_assert_eq!(t.commit(), Ok(Commit { value: p3(g), exact: true }));
        for q in qs.chunks(3) {
            if let Some(qp) = attack_plane(q[0], q[1], q[2]) {
                prop_assert_eq!(qp.side_of_tpi(&t).unwrap(), qp.side(p3(g)).unwrap(), "{:?} at g = {:?}", qp, g);
                check_grid_side(&qp, [q[0], q[1], q[2]], g);
            }
            if let Some(through_g) = attack_plane(g, q[0], q[1]) {
                prop_assert_eq!(through_g.side_of_tpi(&t).unwrap(), Sign::Zero, "plane through g = {:?}", g);
            }
            let moved: [i64; 3] = std::array::from_fn(|i| (g[i] + step[i]).clamp(-M, M));
            if let Some(off) = attack_plane(moved, q[0], q[1]) {
                prop_assert_eq!(off.side_of_tpi(&t).unwrap(), off.side(p3(g)).unwrap(), "plane through {:?}", moved);
                attack_side_of_tpi(&off, &t, &pt, "at a grid point, plane one step off");
            }
        }
    }

    /// Random, extreme-biased planes and fourth planes: `side_of_tpi` against the 4 × 4
    /// determinant and against the plane evaluated at the rational point.
    #[test]
    fn side_of_tpi_against_both_oracles(pts in prop::collection::vec(point(), 18)) {
        let made: Vec<Option<Plane>> =
            pts.chunks(3).map(|c| attack_plane(c[0], c[1], c[2])).collect();
        if let [Some(p1), Some(p2), Some(p3_), ref rest @ ..] = made[..] {
            if let Some((t, pt)) = attack_tpi([p1, p2, p3_], "random") {
                for q in rest.iter().flatten() {
                    attack_side_of_tpi(q, &t, &pt, "random");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 6. Laws: order, orientation, mirror and translation.
// ---------------------------------------------------------------------------------------------

/// The plane through the three points, reversed.
fn reversed(tri: [[i64; 3]; 3]) -> Plane {
    Plane::through(p3(tri[0]), p3(tri[2]), p3(tri[1])).unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(384))]

    /// Every order of the planes gives the same commit and sides, with (X, w) negated for odd
    /// orders; reversing a defining plane negates (X, w) and changes nothing else; reversing the
    /// fourth plane negates its side; the mirror image negates the commit and the side (half to
    /// even is symmetric); translating every point by an even vector translates the commit.
    #[test]
    fn order_orientation_mirror_and_translation_laws(
        pts in prop::collection::vec(point(), 12), shift in [-3i64..=3, -3i64..=3, -3i64..=3]
    ) {
        let tri: Vec<[[i64; 3]; 3]> = pts.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
        let made: Option<Vec<Plane>> = tri.iter().map(|t| attack_plane(t[0], t[1], t[2])).collect();
        let Some(made) = made else { return Ok(()) };
        let planes = [made[0], made[1], made[2]];
        let q = made[3];
        let Some((t, pt)) = attack_tpi(planes, "laws") else { return Ok(()) };
        let grid = t.commit();
        let (x0, w0) = t.homogeneous().unwrap();
        let (x0, w0) = (x0.map(|v| wide_big(&v)), wide_big(&w0));
        let side0 = q.side_of_tpi(&t).unwrap();
        for (perm, even) in PERMS {
            let [a, b, c] = permute(planes, perm);
            let tp = Tpi::new(a, b, c).unwrap();
            let (x, w) = tp.homogeneous().unwrap();
            let s = if even { BigInt::one() } else { -BigInt::one() };
            prop_assert_eq!(wide_big(&w), &s * &w0, "w in order {:?}", perm);
            let want_x = x0.clone().map(|v| &s * v);
            prop_assert_eq!(x.map(|v| wide_big(&v)), want_x, "X in order {:?}", perm);
            prop_assert_eq!(tp.commit(), grid, "to_grid in order {:?}", perm);
            prop_assert_eq!(q.side_of_tpi(&tp).unwrap(), side0, "side in order {:?}", perm);
            if perm != [0, 1, 2] {
                prop_assert_ne!(tp, t, "Tpi equality is equality of the planes in order");
            }
        }
        for f in 0..3 {
            let mut flipped = planes;
            flipped[f] = reversed(tri[f]);
            let tf = Tpi::new(flipped[0], flipped[1], flipped[2]).unwrap();
            let (x, w) = tf.homogeneous().unwrap();
            prop_assert_eq!(wide_big(&w), -w0.clone(), "w with plane {} reversed", f);
            let want_x = x0.clone().map(|v| -v);
            prop_assert_eq!(x.map(|v| wide_big(&v)), want_x, "X with plane {} reversed", f);
            prop_assert_eq!(tf.commit(), grid, "to_grid with plane {} reversed", f);
            prop_assert_eq!(q.side_of_tpi(&tf).unwrap(), side0, "side with plane {} reversed", f);
        }
        prop_assert_eq!(reversed(tri[3]).side_of_tpi(&t).unwrap(), side0.negate(), "reversed fourth plane");

        let mirror = |tr: [[i64; 3]; 3]| {
            Plane::through(p3(neg(tr[0])), p3(neg(tr[1])), p3(neg(tr[2]))).unwrap()
        };
        let mp = [mirror(tri[0]), mirror(tri[1]), mirror(tri[2])];
        let mpt = pt.clone().map(|v| -v);
        let (mt, got) = attack_tpi(mp, "mirror").unwrap();
        prop_assert_eq!(&got, &mpt, "mirrored point");
        match (grid, mt.commit()) {
            (Ok(r), Ok(mr)) => {
                prop_assert_eq!(coords(mr.value), neg(coords(r.value)), "mirror commit");
            }
            (Err(NumError::OutOfRange { value }), Err(NumError::OutOfRange { value: mv }))
                if value != i128::MAX && value != i128::MIN =>
            {
                prop_assert_eq!(mv, -value, "mirror report");
            }
            (Err(_), Err(_)) => {}
            (a, b) => prop_assert!(false, "mirror changed the outcome: {:?} vs {:?}", a, b),
        }
        prop_assert_eq!(mirror(tri[3]).side_of_tpi(&mt).unwrap(), side0.negate(), "mirrored fourth plane");

        for factor in [1i64, 2] {
            let v = shift.map(|s| s * factor);
            let moved: Vec<[i64; 3]> = pts.iter().map(|&q| add(q, v)).collect();
            if !moved.iter().all(|&q| in_frame(q)) {
                continue;
            }
            let shifted: Vec<Plane> = moved
                .chunks(3)
                .take(3)
                .map(|c| Plane::through(p3(c[0]), p3(c[1]), p3(c[2])).unwrap())
                .collect();
            for (a, b) in shifted.iter().zip(&planes) {
                prop_assert_eq!(a.n(), b.n(), "translation keeps every normal");
            }
            let (tt, tpt) = attack_tpi([shifted[0], shifted[1], shifted[2]], "translated").unwrap();
            let want: [BigRational; 3] = std::array::from_fn(|i| &pt[i] + rat(v[i]));
            prop_assert_eq!(&tpt, &want, "translated point");
            if factor == 2 {
                if let (Ok(r), Ok(rt)) = (grid, tt.commit()) {
                    let want_c = add(coords(r.value), v);
                    prop_assert_eq!(coords(rt.value), want_c, "even translation and the commit");
                    prop_assert_eq!(rt.exact, r.exact);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 7. The documented orientation.
// ---------------------------------------------------------------------------------------------

/// `through(a, b, c)` is positive on the side (b − a) × (c − a) points to, where a, b, c turn
/// counter-clockwise; one step along the normal's largest component from a is positive.
#[test]
fn orientation_is_as_documented() {
    let pl = Plane::through(p3([0, 0, 0]), p3([1, 0, 0]), p3([0, 1, 0])).unwrap();
    assert_eq!((pl.n(), pl.d()), ([0, 0, 1], 0));
    assert_eq!(
        pl.side(p3([0, 0, 1])).unwrap(),
        Sign::Positive,
        "above a counter-clockwise triangle"
    );
    assert_eq!(pl.side(p3([0, 0, -1])).unwrap(), Sign::Negative);
    assert_eq!(pl.side(p3([M, -M, 0])).unwrap(), Sign::Zero);
    for tri in [
        [[-M, -M, -M], [M, -M, M], [M, M, -M]],
        [[M, M, M], [-M, M - 1, -M], [0, -M, 1]],
        [[1, 2, 3], [-M, 7, M], [M, -M + 1, -5]],
    ] {
        let pl = attack_plane(tri[0], tri[1], tri[2]).unwrap();
        let n = pl.n();
        let axis = (0..3).max_by_key(|&i| n[i].unsigned_abs()).unwrap();
        for a in tri {
            let mut q = a;
            let step = if n[axis] > 0 { 1 } else { -1 };
            q[axis] += step;
            if in_frame(q) {
                assert_eq!(
                    pl.side(p3(q)).unwrap(),
                    Sign::Positive,
                    "one step along the normal from {a:?}"
                );
            } else {
                q[axis] -= 2 * step;
                assert_eq!(
                    pl.side(p3(q)).unwrap(),
                    Sign::Negative,
                    "a step against the normal from {a:?}"
                );
            }
            check_grid_side(&pl, tri, q);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 8. Compile-time against run-time evaluation.
// ---------------------------------------------------------------------------------------------

/// A grid point for constants; a coordinate outside the frame is a compile-time error.
const fn pt(c: [i64; 3]) -> P3 {
    match P3::new(c[0], c[1], c[2]) {
        Ok(q) => q,
        Err(_) => panic!("a constant point lies outside the frame"),
    }
}

/// `Plane::through` on every ordered triple of the frame's corners, repeats included, evaluated at
/// compile time.
static CONST_CORNER_PLANES: [[[Result<Plane, NumError>; 8]; 8]; 8] = {
    let mut out = [[[Err(NumError::Degenerate); 8]; 8]; 8];
    let mut i = 0;
    while i < 8 {
        let mut j = 0;
        while j < 8 {
            let mut k = 0;
            while k < 8 {
                out[i][j][k] = Plane::through(pt(CORNERS[i]), pt(CORNERS[j]), pt(CORNERS[k]));
                k += 1;
            }
            j += 1;
        }
        i += 1;
    }
    out
};

/// `side` of every corner against every corner plane, at compile time (`Zero` for repeats).
const CONST_CORNER_SIDES: [[[[Sign; 8]; 8]; 8]; 8] = {
    let mut out = [[[[Sign::Zero; 8]; 8]; 8]; 8];
    let mut i = 0;
    while i < 8 {
        let mut j = 0;
        while j < 8 {
            let mut k = 0;
            while k < 8 {
                if let Ok(pl) = CONST_CORNER_PLANES[i][j][k] {
                    let mut m = 0;
                    while m < 8 {
                        out[i][j][k][m] = match pl.side(pt(CORNERS[m])) {
                            Ok(s) => s,
                            Err(_) => panic!("side"),
                        };
                        m += 1;
                    }
                }
                k += 1;
            }
            j += 1;
        }
        i += 1;
    }
    out
};

/// Levels for the probe points: the extremes, one step in, and zero.
const LEVELS: [i64; 5] = [-M, -M + 1, 0, M - 1, M];

/// Triples near the extremes, including a collinear and a repeated one.
const PROBE_TRIPLES: [[[i64; 3]; 3]; 8] = [
    [[-M, -M, -M], [M, -M + 1, M], [M - 1, M, -M]],
    [[M, M, M], [-M, -M + 1, M - 1], [0, -M, M]],
    [[-M + 1, 0, M], [M, -M, -M + 1], [-M, M, 0]],
    [[0, 0, 0], [M, M - 1, -M], [-M, M, M - 1]],
    [[M, -M, M], [-M, M, -M], [M - 1, M, M]],
    [[-M, -M, M], [M, M, -M], [0, 0, 0]],
    [[-M, -M, -M], [-M, -M, -M], [M, M, M]],
    [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
];

const fn level_point(index: usize) -> [i64; 3] {
    [
        LEVELS[index % 5],
        LEVELS[(index / 5) % 5],
        LEVELS[index / 25],
    ]
}

const CONST_PROBE_PLANES: [Result<Plane, NumError>; 8] = {
    let mut out = [Err(NumError::Degenerate); 8];
    let mut i = 0;
    while i < 8 {
        let [a, b, c] = PROBE_TRIPLES[i];
        out[i] = Plane::through(pt(a), pt(b), pt(c));
        i += 1;
    }
    out
};

/// `side` of all 125 level points against every probe plane, at compile time.
const CONST_PROBE_SIDES: [[Sign; 125]; 8] = {
    let mut out = [[Sign::Zero; 125]; 8];
    let mut i = 0;
    while i < 8 {
        if let Ok(pl) = CONST_PROBE_PLANES[i] {
            let mut m = 0;
            while m < 125 {
                out[i][m] = match pl.side(pt(level_point(m))) {
                    Ok(s) => s,
                    Err(_) => panic!("side"),
                };
                m += 1;
            }
        }
        i += 1;
    }
    out
};

/// A plane with |`n_i`| = 4M² and |d| = 4M³, and its coefficients, at compile time.
const MAX_PLANE: Result<Plane, NumError> =
    Plane::through(pt([-M, -M, -M]), pt([M, -M, M]), pt([M, M, -M]));
const MAX_N: [i128; 3] = match MAX_PLANE {
    Ok(pl) => pl.n(),
    Err(_) => [0; 3],
};
const MAX_D: i128 = match MAX_PLANE {
    Ok(pl) => pl.d(),
    Err(_) => 0,
};

/// `Plane::through` and `Plane::side` are `const fn`: every value computed at compile time equals
/// the same call at run time (inputs passed through `black_box`, so nothing is folded), at the
/// frame's corners and around them, including the `Degenerate` results.
#[test]
fn const_and_runtime_evaluations_agree() {
    for (i, &a) in CORNERS.iter().enumerate() {
        for (j, &b) in CORNERS.iter().enumerate() {
            for (k, &c) in CORNERS.iter().enumerate() {
                let run = Plane::through(black_box(p3(a)), black_box(p3(b)), black_box(p3(c)));
                assert_eq!(
                    CONST_CORNER_PLANES[i][j][k], run,
                    "through({a:?}, {b:?}, {c:?})"
                );
                for (m, &q) in CORNERS.iter().enumerate() {
                    let side = run.map_or(Sign::Zero, |pl| pl.side(black_box(p3(q))).unwrap());
                    let at_compile_time = CONST_CORNER_SIDES[i][j][k][m];
                    assert_eq!(at_compile_time, side, "side of {q:?} vs {a:?} {b:?} {c:?}");
                }
            }
        }
    }
    for (i, tri) in PROBE_TRIPLES.iter().enumerate() {
        let [a, b, c] = tri.map(|v| black_box(p3(v)));
        let run = Plane::through(a, b, c);
        assert_eq!(CONST_PROBE_PLANES[i], run, "through{tri:?}");
        let oracle = attack_plane(tri[0], tri[1], tri[2]);
        assert_eq!(run.ok(), oracle, "through{tri:?} against the oracle");
        for (m, &at_compile_time) in CONST_PROBE_SIDES[i].iter().enumerate() {
            let q = level_point(m);
            let side = run.map_or(Sign::Zero, |pl| pl.side(black_box(p3(q))).unwrap());
            assert_eq!(at_compile_time, side, "side of {q:?} vs {tri:?}");
            if let Ok(pl) = run {
                check_grid_side(&pl, *tri, q);
            }
        }
    }
    assert_eq!(
        CONST_PROBE_PLANES[5],
        Err(NumError::Degenerate),
        "collinear at compile time"
    );
    assert_eq!(
        CONST_PROBE_PLANES[6],
        Err(NumError::Degenerate),
        "repeated at compile time"
    );
    let (m2, m3) = (
        i128::from(M) * i128::from(M),
        i128::from(M) * i128::from(M) * i128::from(M),
    );
    assert_eq!(
        (MAX_N, MAX_D),
        ([-4 * m2, 4 * m2, 4 * m2], 4 * m3),
        "compile-time maximal plane"
    );
}
