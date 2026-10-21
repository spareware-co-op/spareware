//! Adversarial tests for `geom_num::plane`: every claim the module makes, attacked with exact
//! oracles computed by methods different from the code's.
//!
//! Oracles (all in `num-bigint` and `num-rational`):
//! - planes: n = a × b + b × c + c × a and d = −det[a; b; c] (the code uses (b − a) × (c − a));
//! - sides of grid points: −det[[a, 1], [b, 1], [c, 1], [p, 1]] by Laplace expansion, and
//!   `predicate::orient3d`;
//! - three-plane points: Cramer's rule with Leibniz determinants, compared value for value with
//!   `Tpi::homogeneous` (not just as a ratio), and, for designed points, the line-meets-plane
//!   formula a − (t / D) e;
//! - sides of three-plane points: the plane evaluated at the exact rational point (no sign trick);
//! - grid commits: floor division with remainder in big integers, ties to even.
//!
//! Attacks: exact cancellation of 82-bit products, collinear points on lines across the whole
//! frame, parallel, anti-parallel, scaled, repeated, coaxial (pencil) and prism plane triples at the
//! extremes, three-plane points within 2^-116 of a half step and of grid planes with w of both
//! signs, exact ties with |w| near 2^235 of both signs, ties on the frame's edge, coordinates
//! around ±2^127 where the reported value saturates, and the order in which out-of-range axes are
//! reported.
//!
//! Budgets: every bit budget is checked on the bound of all its partial sums (the sum of the
//! magnitudes of the terms), not only on the final value. `side_of_tpi` and `to_grid` fall back
//! to an exact `I512` computation if a step in `I384` does not fit, so an `I384` overflow could
//! never show in their results; these checks are what show the budgeted path suffices.

use std::cmp::Ordering;

use geom_num::grid::COORD_MAX;
use geom_num::plane::{Plane, Tpi};
use geom_num::predicate::orient3d;
use geom_num::{NumError, P3, Rounded, Sign, Wide};
use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use proptest::prelude::*;

const M: i64 = COORD_MAX;
const ID: [usize; 3] = [0, 1, 2];

/// The six permutations of three things, with whether each is even.
const PERMS: [([usize; 3], bool); 6] = [
    ([0, 1, 2], true),
    ([1, 2, 0], true),
    ([2, 0, 1], true),
    ([0, 2, 1], false),
    ([2, 1, 0], false),
    ([1, 0, 2], false),
];

// ---------------------------------------------------------------------------------------------
// Small helpers.
// ---------------------------------------------------------------------------------------------

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn pa(q: [i64; 3]) -> P3 {
    p(q[0], q[1], q[2])
}

fn xyz(q: P3) -> [i64; 3] {
    [q.x.get(), q.y.get(), q.z.get()]
}

fn in_frame(q: [i64; 3]) -> bool {
    q.iter().all(|v| v.abs() <= M)
}

fn add3i(a: [i64; 3], b: [i64; 3]) -> [i64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn permuted<T: Copy>(v: [T; 3], perm: [usize; 3]) -> [T; 3] {
    [v[perm[0]], v[perm[1]], v[perm[2]]]
}

/// The plane through a, a + u, a + v if all three lie in the frame and are not collinear.
fn through_in_frame(a: [i64; 3], u: [i64; 3], v: [i64; 3]) -> Option<Plane> {
    let (b, c) = (add3i(a, u), add3i(a, v));
    if in_frame(a) && in_frame(b) && in_frame(c) { Plane::through(pa(a), pa(b), pa(c)).ok() } else { None }
}

/// q moved one step along `axis` towards the origin (or away from it at 0).
fn nudge_in(q: P3, axis: usize) -> P3 {
    let mut c = xyz(q);
    c[axis] += if c[axis] > 0 { -1 } else { 1 };
    pa(c)
}

fn wide_to_big<const L: usize>(w: &Wide<L>) -> BigInt {
    let bytes: Vec<u8> = w.to_le_limbs().iter().flat_map(|l| l.to_le_bytes()).collect();
    BigInt::from_signed_bytes_le(&bytes)
}

fn sign_big(v: &BigInt) -> Sign {
    match v.cmp(&BigInt::zero()) {
        Ordering::Less => Sign::Negative,
        Ordering::Equal => Sign::Zero,
        Ordering::Greater => Sign::Positive,
    }
}

fn big3(q: P3) -> [BigInt; 3] {
    xyz(q).map(BigInt::from)
}

fn dot3(u: &[BigInt; 3], v: &[BigInt; 3]) -> BigInt {
    &u[0] * &v[0] + &u[1] * &v[1] + &u[2] * &v[2]
}

fn cross3(u: &[BigInt; 3], v: &[BigInt; 3]) -> [BigInt; 3] {
    [&u[1] * &v[2] - &u[2] * &v[1], &u[2] * &v[0] - &u[0] * &v[2], &u[0] * &v[1] - &u[1] * &v[0]]
}

/// det of the rows of m by the Leibniz formula.
fn det3(m: &[[BigInt; 3]; 3]) -> BigInt {
    &m[0][0] * &m[1][1] * &m[2][2] + &m[0][1] * &m[1][2] * &m[2][0] + &m[0][2] * &m[1][0] * &m[2][1]
        - &m[0][2] * &m[1][1] * &m[2][0]
        - &m[0][0] * &m[1][2] * &m[2][1]
        - &m[0][1] * &m[1][0] * &m[2][2]
}

fn rat(n: impl Into<BigInt>, d: impl Into<BigInt>) -> BigRational {
    BigRational::new(n.into(), d.into())
}

/// Balanced residue of x modulo m: in (−m/2, m/2].
fn bmod(x: &BigInt, m: &BigInt) -> BigInt {
    let r = x.mod_floor(m);
    if BigInt::from(2) * &r > *m { r - m } else { r }
}

/// n / 2 rounded half to even, nudged by an infinitesimal of the sign of `delta` (which decides a
/// tie when n is odd and is irrelevant otherwise).
fn round_near(n: i64, delta: i64) -> i64 {
    if n % 2 == 0 {
        return n / 2;
    }
    let (lo, hi) = ((n - 1) / 2, (n + 1) / 2);
    match delta.signum() {
        1 => hi,
        -1 => lo,
        _ => {
            if lo % 2 == 0 {
                lo
            } else {
                hi
            }
        }
    }
}

fn half_even(n: i64) -> i64 {
    round_near(n, 0)
}

// ---------------------------------------------------------------------------------------------
// Oracles.
// ---------------------------------------------------------------------------------------------

/// The plane through a, b, c by the expanded formula n = a × b + b × c + c × a, d = −det[a; b; c].
fn oracle_plane(a: P3, b: P3, c: P3) -> ([BigInt; 3], BigInt) {
    let (a, b, c) = (big3(a), big3(b), big3(c));
    let (ab, bc, ca) = (cross3(&a, &b), cross3(&b, &c), cross3(&c, &a));
    let n = [&ab[0] + &bc[0] + &ca[0], &ab[1] + &bc[1] + &ca[1], &ab[2] + &bc[2] + &ca[2]];
    let d = -det3(&[a, b, c]);
    (n, d)
}

/// The orientation of a, b, c, q as −det[[a, 1], [b, 1], [c, 1], [q, 1]], by Laplace expansion along
/// the column of ones.
fn orient_oracle(a: P3, b: P3, c: P3, q: P3) -> Sign {
    let r = [big3(a), big3(b), big3(c), big3(q)];
    let minor = |skip: usize| {
        let rows: Vec<&[BigInt; 3]> = (0..4).filter(|&i| i != skip).map(|i| &r[i]).collect();
        det3(&[rows[0].clone(), rows[1].clone(), rows[2].clone()])
    };
    let det4 = -minor(0) + minor(1) - minor(2) + minor(3);
    sign_big(&-det4)
}

/// Checks `Plane::through` against the oracle, its budget, and the exact laws of point order (a
/// rotation gives the identical plane, a transposition the exactly negated one). Returns the plane,
/// or `None` when the points are collinear (after checking every order is `Degenerate`).
fn check_plane(a: P3, b: P3, c: P3) -> Option<Plane> {
    let (n, d) = oracle_plane(a, b, c);
    let got = Plane::through(a, b, c);
    if n.iter().all(Zero::is_zero) {
        for (x, y, z) in [(a, b, c), (b, c, a), (c, a, b), (a, c, b), (c, b, a), (b, a, c)] {
            assert_eq!(Plane::through(x, y, z), Err(NumError::Degenerate), "{a:?} {b:?} {c:?} are collinear");
        }
        return None;
    }
    let pl = got.unwrap_or_else(|e| panic!("{a:?} {b:?} {c:?} are not collinear, got {e:?}"));
    assert_eq!(pl.n().map(BigInt::from), n, "normal of {a:?} {b:?} {c:?}");
    assert_eq!(BigInt::from(pl.d()), d, "offset of {a:?} {b:?} {c:?}");
    assert!(n.iter().all(|v| v.bits() <= 83), "normal over budget");
    assert!(d.bits() <= 125, "offset over budget");
    // Every partial sum of d = −(n_x a_x + n_y a_y + n_z a_z) is below 2^125.
    assert!(abs_dot(&n, &big3(a)).bits() <= 125, "a partial sum of d over budget");
    assert_eq!(Plane::through(b, c, a), Ok(pl), "rotation changed the plane");
    assert_eq!(Plane::through(c, a, b), Ok(pl), "rotation changed the plane");
    let flipped = Plane::through(a, c, b).unwrap();
    assert_eq!(flipped.n(), pl.n().map(|v| -v), "transposition must negate n exactly");
    assert_eq!(flipped.d(), -pl.d(), "transposition must negate d exactly");
    for q in [a, b, c] {
        assert_eq!(pl.side(q), Sign::Zero);
    }
    Some(pl)
}

/// Checks `side` against the 4 × 4 determinant and `orient3d`, with its budget.
fn check_side(pl: &Plane, a: P3, b: P3, c: P3, q: P3) {
    let want = orient_oracle(a, b, c, q);
    assert_eq!(pl.side(q), want, "side of {q:?} against plane through {a:?} {b:?} {c:?}");
    assert_eq!(orient3d(a, b, c, q), want, "orient3d disagrees with the plane");
    let v = dot3(&pl.n().map(BigInt::from), &big3(q)) + BigInt::from(pl.d());
    assert!(v.bits() <= 126, "side over budget");
    let bound = abs_dot(&pl.n().map(BigInt::from), &big3(q)) + BigInt::from(pl.d()).abs();
    assert!(bound.bits() <= 126, "a partial sum of the side over budget");
}

/// |u_x v_x| + |u_y v_y| + |u_z v_z|: a bound on every partial sum of u · v.
fn abs_dot(u: &[BigInt; 3], v: &[BigInt; 3]) -> BigInt {
    u.iter().zip(v).map(|(a, b)| a.abs() * b.abs()).sum()
}

/// The budgets of `homogeneous` and of the `I384` path of `to_grid`, on bounds of every partial
/// sum: cross products below 2^167, w below 2^252, X below 2^294; then 2 |X_i| below 2^295,
/// (2^41 − 1) |w| below 2^293 and 2^127 |w| below 2^379, which also bounds twice the division's
/// T = 2^126 |w| at 127 bits.
fn check_wide_budgets(planes: &[Plane; 3], x: &[BigInt; 3], w: &BigInt) {
    let n = planes.map(|pl| pl.n().map(BigInt::from));
    let d = planes.map(|pl| BigInt::from(pl.d()));
    let pairs = [(1, 2), (2, 0), (0, 1)];
    for (i, j) in pairs {
        let (a, b) = (&n[i], &n[j]);
        let bounds = [
            a[1].abs() * b[2].abs() + a[2].abs() * b[1].abs(),
            a[2].abs() * b[0].abs() + a[0].abs() * b[2].abs(),
            a[0].abs() * b[1].abs() + a[1].abs() * b[0].abs(),
        ];
        assert!(bounds.iter().all(|v| v.bits() <= 167), "a cross product over budget");
    }
    let crosses = pairs.map(|(i, j)| cross3(&n[i], &n[j]));
    assert!(abs_dot(&n[0], &crosses[0]).bits() <= 252, "a partial sum of w over budget");
    for axis in 0..3 {
        let bound: BigInt = (0..3).map(|k| d[k].abs() * crosses[k][axis].abs()).sum();
        assert!(bound.bits() <= 294, "a partial sum of X over budget");
    }
    for xi in x {
        assert!((BigInt::from(2) * xi.abs()).bits() <= 295, "2 |X_i| over budget");
    }
    assert!((BigInt::from(2 * M + 1) * w.abs()).bits() <= 293, "(2^41 − 1) |w| over budget");
    assert!(((BigInt::one() << 127u32) * w.abs()).bits() <= 379, "2^127 |w| over budget");
}

/// (X, w) by Cramer's rule with Leibniz determinants: w = det N, X_i = det of N with column i
/// replaced by −d.
fn cramer(planes: &[Plane; 3]) -> ([BigInt; 3], BigInt) {
    let rows: [[BigInt; 3]; 3] = (*planes).map(|pl| pl.n().map(BigInt::from));
    let rhs: [BigInt; 3] = (*planes).map(|pl| -BigInt::from(pl.d()));
    let w = det3(&rows);
    let x = std::array::from_fn(|col| {
        let mut m = rows.clone();
        for (row, r) in m.iter_mut().zip(&rhs) {
            row[col] = r.clone();
        }
        det3(&m)
    });
    (x, w)
}

/// x / w rounded to the nearest integer, ties to even, and whether it was exact.
fn round_div(x: &BigInt, w: &BigInt) -> (BigInt, bool) {
    let (x, w) = if w.is_negative() { (-x, -w) } else { (x.clone(), w.clone()) };
    let (q, r) = x.div_mod_floor(&w);
    let up = match (BigInt::from(2) * &r).cmp(&w) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => q.is_odd(),
    };
    let exact = r.is_zero();
    (if up { q + 1 } else { q }, exact)
}

fn saturate(v: &BigInt) -> i128 {
    v.to_i128().unwrap_or(if v.is_negative() { i128::MIN } else { i128::MAX })
}

/// What `to_grid` must return for the point X / w.
fn expected_grid(x: &[BigInt; 3], w: &BigInt) -> Result<Rounded<P3>, NumError> {
    let mut out = [0i64; 3];
    let mut exact = true;
    for (o, xi) in out.iter_mut().zip(x) {
        let (r, e) = round_div(xi, w);
        if r.abs() > BigInt::from(M) {
            return Err(NumError::OutOfRange { value: saturate(&r) });
        }
        *o = r.to_i64().unwrap();
        exact &= e;
    }
    Ok(Rounded { value: pa(out), exact })
}

/// The sign of n · pt + d at an exact rational point.
fn side_at(pl: &Plane, pt: &[BigRational; 3]) -> Sign {
    let mut v = BigRational::from_integer(BigInt::from(pl.d()));
    for (ni, ci) in pl.n().iter().zip(pt) {
        v += ci * BigRational::from_integer(BigInt::from(*ni));
    }
    match v.cmp(&BigRational::zero()) {
        Ordering::Less => Sign::Negative,
        Ordering::Equal => Sign::Zero,
        Ordering::Greater => Sign::Positive,
    }
}

/// Checks `Tpi::new`, `homogeneous`, `to_grid` and the defining planes' sides against Cramer's
/// rule. Returns the point and its exact position, or `None` (after checking every order is
/// `Degenerate`) when the planes do not meet in one point.
fn check_tpi(planes: [Plane; 3]) -> Option<(Tpi, [BigRational; 3])> {
    let (cx, cw) = cramer(&planes);
    if cw.is_zero() {
        for (perm, _) in PERMS {
            let [x, y, z] = permuted(planes, perm);
            assert_eq!(Tpi::new(x, y, z), Err(NumError::Degenerate), "{planes:?} do not meet in one point");
        }
        return None;
    }
    let [a, b, c] = planes;
    let t = Tpi::new(a, b, c).unwrap_or_else(|e| panic!("{planes:?} meet in one point, got {e:?}"));
    assert_eq!(t.planes(), &planes);
    let (x, w) = t.homogeneous();
    let (xb, wb) = (x.map(|v| wide_to_big(&v)), wide_to_big(&w));
    assert_eq!(wb, cw, "w is not det(n_1, n_2, n_3)");
    assert_eq!(xb, cx, "X is not the Cramer numerator");
    assert!(wb.bits() <= 252, "w over budget");
    assert!(xb.iter().all(|v| v.bits() <= 294), "X over budget");
    check_wide_budgets(&planes, &cx, &cw);
    let pt: [BigRational; 3] = std::array::from_fn(|i| BigRational::new(cx[i].clone(), cw.clone()));
    for pl in &planes {
        assert_eq!(side_at(pl, &pt), Sign::Zero);
        assert_eq!(pl.side_of_tpi(&t), Sign::Zero, "a defining plane must contain its point");
    }
    assert_eq!(t.to_grid(), expected_grid(&cx, &cw), "to_grid of {pt:?}");
    Some((t, pt))
}

/// Checks `side_of_tpi` against the plane evaluated at the exact rational point, with its budget.
fn check_side_of_tpi(pl: &Plane, t: &Tpi, pt: &[BigRational; 3]) {
    assert_eq!(pl.side_of_tpi(t), side_at(pl, pt), "side_of_tpi of {pt:?} against {pl:?}");
    let (x, w) = t.homogeneous();
    let class = dot3(&pl.n().map(BigInt::from), &x.map(|v| wide_to_big(&v))) + BigInt::from(pl.d()) * wide_to_big(&w);
    assert!(class.bits() <= 379, "classification over budget");
    let bound = abs_dot(&pl.n().map(BigInt::from), &x.map(|v| wide_to_big(&v)))
        + BigInt::from(pl.d()).abs() * wide_to_big(&w).abs();
    assert!(bound.bits() <= 379, "a partial sum of the classification over budget");
}

// ---------------------------------------------------------------------------------------------
// Designed three-plane points: a line meeting a plane exactly where we want.
// ---------------------------------------------------------------------------------------------

/// D = n3 · e for n3 = (1, −P, P²).
fn d_of(pp: i64, e: [i64; 3]) -> BigInt {
    let pb = BigInt::from(pp);
    BigInt::from(e[0]) - &pb * BigInt::from(e[1]) + &pb * &pb * BigInt::from(e[2])
}

/// Planes 1 and 2 pass through the grid points a and a + e (through a, a + e, a + (1, 0, 0) and
/// through a, a + e, a + (0, 0, 1)), so they meet in the line a + λe. Plane 3 passes through c,
/// c + u, c + v with u = (P, 1, 0) and v = (0, P, 1), so its normal is n3 = (1, −P, P²); c is solved
/// so that n3 · (a − c) = t (balanced base-P digits, then shifted by whole multiples of u and v, which
/// n3 is orthogonal to, until every point lies in the frame). So the planes meet where λ = −t / D,
/// D = n3 · e: at a − (t / D) e. All points are then permuted by `perm` and negated if `mirror`, and
/// planes 1 and 2 swapped if `swap` (which negates w). Returns the planes and the designed exact
/// point, or `None` if a point would leave the frame or D = 0.
fn line_meets_plane(
    pp: i64,
    e: [i64; 3],
    a: [i64; 3],
    t: &BigInt,
    perm: [usize; 3],
    mirror: bool,
    swap: bool,
) -> Option<([Plane; 3], [BigRational; 3])> {
    line_meets_plane_scaled(pp, e, a, t, [1, 1], perm, mirror, swap)
}

/// [`line_meets_plane`] with planes 1 and 2 made through a + m_1 (1, 0, 0) and a + m_2 (0, 0, 1)
/// instead: the same planes as point sets, with normals scaled by m_1 and m_2, so the same point has
/// w scaled by m_1 m_2 (unswapped and unpermuted, w = −m_1 m_2 e_y D).
#[allow(clippy::too_many_arguments)]
fn line_meets_plane_scaled(
    pp: i64,
    e: [i64; 3],
    a: [i64; 3],
    t: &BigInt,
    m: [i64; 2],
    perm: [usize; 3],
    mirror: bool,
    swap: bool,
) -> Option<([Plane; 3], [BigRational; 3])> {
    let (f1, f2) = ([m[0], 0, 0], [0, 0, m[1]]);
    let pb = BigInt::from(pp);
    let rx = bmod(t, &pb);
    let t1 = (t - &rx) / &pb;
    let ry = -bmod(&t1, &pb);
    let rz = (&t1 + &ry) / &pb;
    let n3 = [BigInt::one(), -&pb, &pb * &pb];
    assert_eq!(dot3(&n3, &[rx.clone(), ry.clone(), rz.clone()]), *t, "digit solve");
    let r = [rx.to_i64()?, ry.to_i64()?, rz.to_i64()?];
    let (u, v) = ([pp, 1, 0], [0, pp, 1]);
    let mut found = None;
    'search: for s1 in -4i64..=4 {
        for s2 in -4i64..=4 {
            let shift = [s1 * u[0] + s2 * v[0], s1 * u[1] + s2 * v[1], s1 * u[2] + s2 * v[2]];
            let mut c = [0i64; 3];
            let mut ok = true;
            for (ci, ((&ai, &ri), &si)) in c.iter_mut().zip(a.iter().zip(&r).zip(&shift)) {
                match ai.checked_sub(ri).and_then(|x| x.checked_add(si)) {
                    Some(x) if x.abs() <= M => *ci = x,
                    _ => ok = false,
                }
            }
            if ok && in_frame(add3i(c, u)) && in_frame(add3i(c, v)) {
                found = Some(c);
                break 'search;
            }
        }
    }
    let c = found?;
    if !in_frame(a) || e.iter().any(|v| v.abs() > 2 * M) {
        return None;
    }
    let line = [a, add3i(a, e), add3i(a, f1), add3i(a, f2)];
    if !line.iter().all(|q| in_frame(*q)) {
        return None;
    }
    let tf = |q: [i64; 3]| {
        let r = permuted(q, perm);
        if mirror { p(-r[0], -r[1], -r[2]) } else { pa(r) }
    };
    let p1 = Plane::through(tf(a), tf(add3i(a, e)), tf(add3i(a, f1))).ok()?;
    let p2 = Plane::through(tf(a), tf(add3i(a, e)), tf(add3i(a, f2))).ok()?;
    let p3 = Plane::through(tf(c), tf(add3i(c, u)), tf(add3i(c, v))).ok()?;
    let dd = d_of(pp, e);
    if dd.is_zero() {
        return None;
    }
    let lambda = BigRational::new(-t.clone(), dd);
    let old: [BigRational; 3] =
        std::array::from_fn(|i| BigRational::from_integer(BigInt::from(a[i])) + &lambda * BigRational::from_integer(BigInt::from(e[i])));
    let mut pt = permuted([0, 1, 2], perm).map(|i| old[i].clone());
    if mirror {
        pt = pt.map(|v| -v);
    }
    Some((if swap { [p2, p1, p3] } else { [p1, p2, p3] }, pt))
}

/// P for the near-tie construction: odd, just below 2^39.
const P_NEAR: i64 = (1 << 39) - 1;
/// e_z for the near-tie construction: even, so D = 1 − P + P² e_z is even.
const EZ_NEAR: i64 = 1 << 38;

/// Planes meeting at x = x2 / 2 + δ / D, y = (x2 − 2 a_x) / 2 + δ / D, z = 2^38 · y, with
/// D = 1 − P + P² 2^38, just below 2^116. Unpermuted and unmirrored, w = −D (or +D if swapped).
fn near_tie(ax: i64, x2: i64, delta: i64, perm: [usize; 3], mirror: bool, swap: bool) -> ([Plane; 3], [BigRational; 3]) {
    near_tie_scaled(ax, x2, delta, [1, 1], perm, mirror, swap)
}

/// [`near_tie`] with planes 1 and 2 scaled by m_1 and m_2: w = −m_1 m_2 D (or +m_1 m_2 D swapped).
fn near_tie_scaled(
    ax: i64,
    x2: i64,
    delta: i64,
    m: [i64; 2],
    perm: [usize; 3],
    mirror: bool,
    swap: bool,
) -> ([Plane; 3], [BigRational; 3]) {
    let e = [1, 1, EZ_NEAR];
    let d = d_of(P_NEAR, e);
    assert!(d.is_even());
    let j = x2 - 2 * ax;
    let t = -(BigInt::from(j) * &d / BigInt::from(2)) - BigInt::from(delta);
    line_meets_plane_scaled(P_NEAR, e, [ax, 0, 0], &t, m, perm, mirror, swap).expect("near-tie construction fits the frame")
}

/// Odd scale factors just below 2^38 for planes 1 and 2.
const SCALE: [i64; 2] = [274_877_906_939, 274_877_906_929];

/// Planes meeting at x = x2 / 2 exactly, with w = −2 (or +2 if swapped), for |x| up to beyond
/// 2^127: P = 2^39 − 1 and e = (2^39 + 1, 1, 0), so D = 2.
fn half_far(x2: &BigInt, mirror: bool, swap: bool) -> Option<([Plane; 3], [BigRational; 3])> {
    half_far_scaled(x2, [1, 1], mirror, swap)
}

/// [`half_far`] with planes 1 and 2 scaled by m_1 and m_2: w = −2 m_1 m_2 (or +2 m_1 m_2 swapped).
fn half_far_scaled(x2: &BigInt, m: [i64; 2], mirror: bool, swap: bool) -> Option<([Plane; 3], [BigRational; 3])> {
    let ex = (1i64 << 39) + 1;
    let exb = BigInt::from(ex);
    let inv2 = BigInt::from((ex + 1) / 2);
    let ax = bmod(&(x2 * &inv2), &exb).to_i64().unwrap();
    let num = BigInt::from(2 * ax) - x2;
    assert!((&num % &exb).is_zero());
    let t = num / &exb;
    line_meets_plane_scaled(P_NEAR, [ex, 1, 0], [ax, 0, 0], &t, m, ID, mirror, swap)
}

/// Planes meeting at the integral point (x, −t, 0) with w = −1 (or +1 if swapped): P = 2^39 − 1
/// and e = (2^39, 1, 0), so D = 1.
fn integral_far(x: &BigInt, perm: [usize; 3], swap: bool) -> ([Plane; 3], [BigRational; 3]) {
    let ex = 1i64 << 39;
    let exb = BigInt::from(ex);
    let ax = bmod(x, &exb).to_i64().unwrap();
    let t = (BigInt::from(ax) - x) / &exb;
    line_meets_plane(P_NEAR, [ex, 1, 0], [ax, 0, 0], &t, perm, false, swap).expect("fits the frame")
}

/// Planes x − z = s, x + z = t and y = y0 with every normal scaled by k·l, so w = 2 (k l)^3, and the
/// point ((s + t) / 2, y0, (t − s) / 2) is a tie on x and z whenever s + t is odd.
fn scaled_tie(s: i64, t: i64, y0: i64, k: i64, l: i64) -> Option<[Plane; 3]> {
    let h = s.div_euclid(2);
    let p1 = through_in_frame([h, 0, h - s], [0, l, 0], [k, 0, k])?;
    let h = t.div_euclid(2);
    let p2 = through_in_frame([h, 0, t - h], [0, l, 0], [k, 0, -k])?;
    let p3 = through_in_frame([0, y0, 0], [0, 0, l], [k, 0, 0])?;
    Some([p1, p2, p3])
}

// ---------------------------------------------------------------------------------------------
// Strategies.
// ---------------------------------------------------------------------------------------------

fn coord() -> impl Strategy<Value = i64> {
    prop_oneof![
        3 => -M..=M,
        3 => prop::sample::select(vec![M, -M, M - 1, -M + 1, M - 2, -M + 2, 0, 1, -1, 2, -2]),
        1 => (M - 64)..=M,
        1 => -M..=(-M + 64),
        1 => -3i64..=3,
    ]
}

fn point() -> impl Strategy<Value = P3> {
    (coord(), coord(), coord()).prop_map(|(x, y, z)| p(x, y, z))
}

fn plane3() -> impl Strategy<Value = (Plane, [P3; 3])> {
    (point(), point(), point()).prop_filter_map("collinear", |(a, b, c)| Plane::through(a, b, c).ok().map(|pl| (pl, [a, b, c])))
}

fn perm() -> impl Strategy<Value = [usize; 3]> {
    prop::sample::select(PERMS.map(|(q, _)| q).to_vec())
}

// ---------------------------------------------------------------------------------------------
// Planes and sides.
// ---------------------------------------------------------------------------------------------

/// Orientation and offset by hand: the normal points to where a, b, c turn counter-clockwise.
#[test]
fn plane_hand_examples() {
    let pl = Plane::through(p(0, 0, 0), p(1, 0, 0), p(0, 1, 0)).unwrap();
    assert_eq!((pl.n(), pl.d()), ([0, 0, 1], 0));
    assert_eq!(pl.side(p(0, 0, 5)), Sign::Positive);
    assert_eq!(pl.side(p(M, -M, -1)), Sign::Negative);
    assert_eq!(pl.side(p(-M, M, 0)), Sign::Zero);
    let pl = Plane::through(p(1, 0, 0), p(1, 1, 0), p(1, 0, 1)).unwrap();
    assert_eq!((pl.n(), pl.d()), ([1, 0, 0], -1));
    let pl = Plane::through(p(6, 0, 0), p(0, 6, 0), p(0, 0, 6)).unwrap();
    assert_eq!((pl.n(), pl.d()), ([36, 36, 36], -216));
    assert_eq!(pl.side(p(2, 2, 2)), Sign::Zero);
    assert_eq!(pl.side(p(2, 2, 3)), Sign::Positive);
    // The largest axis plane: x = 0 through (0, ±M, ±M) has n = (4M², 0, 0).
    let pl = Plane::through(p(0, -M, -M), p(0, M, -M), p(0, -M, M)).unwrap();
    let m = i128::from(M);
    assert_eq!((pl.n(), pl.d()), ([4 * m * m, 0, 0], 0));
    assert_eq!(pl.side(p(1, M, M)), Sign::Positive);
    assert_eq!(pl.side(p(-1, -M, M)), Sign::Negative);
}

/// Normals made of 82-bit products that cancel to ±1 or ±2, and to exactly zero.
#[test]
fn plane_exact_cancellation_of_82_bit_products() {
    // n_z = 2M(2M − 2) − (2M − 1)² = −1.
    let pl = check_plane(p(-M, -M, 0), p(M, M - 1, 0), p(M - 1, M - 2, 0)).unwrap();
    assert_eq!((pl.n(), pl.d()), ([0, 0, -1], 0));
    assert_eq!(pl.side(p(0, 0, 1)), Sign::Negative);
    assert_eq!(pl.side(p(M, -M, 0)), Sign::Zero);
    // All three components cancel: u = (2M, 2M − 1, 2M − 2), v = (2M − 1, 2M − 2, 2M − 3).
    let (a, b, c) = (p(-M, -M, -M), p(M, M - 1, M - 2), p(M - 1, M - 2, M - 3));
    let pl = check_plane(a, b, c).unwrap();
    assert_eq!((pl.n(), pl.d()), ([-1, 2, -1], 0));
    for q in [p(M, M, M), p(-M, M, -M), p(0, 1, 0), p(1, 0, 0), p(M, -M, M)] {
        check_side(&pl, a, b, c, q);
    }
    // Exactly zero although every product is about 2^82: collinear across the whole frame.
    for (a, b, c) in [
        (p(-M, -M, -M), p(M, M, M), p(M - 2, M - 2, M - 2)),
        (p(-M, M, -M), p(M, -M, M), p(1, -1, 1)),
        (p(-M, -M, 0), p(M, M - 2, 2), p(0, -1, 1)),
        (p(M, -M, 0), p(-M, M, 0), p(M - 1, -M + 1, 0)),
    ] {
        assert!(check_plane(a, b, c).is_none(), "{a:?} {b:?} {c:?}");
    }
    // Repeated points.
    for (a, b) in [(p(M, M, M), p(-M, -M, -M)), (p(M, -M, 0), p(M, -M, 1)), (p(0, 0, 0), p(0, 0, 0))] {
        for (x, y, z) in [(a, a, b), (a, b, a), (b, a, a), (a, a, a)] {
            assert_eq!(Plane::through(x, y, z), Err(NumError::Degenerate));
        }
    }
}

/// Every ordered triple of points of {−1, 0, 1}³: degeneracy, coefficients and every side exact.
#[test]
fn plane_exhaustive_unit_cube() {
    let pts: Vec<P3> = (0..27).map(|i| p(i % 3 - 1, (i / 3) % 3 - 1, i / 9 - 1)).collect();
    let mut planes = 0;
    for (i, &a) in pts.iter().enumerate() {
        for (j, &b) in pts.iter().enumerate() {
            for (k, &c) in pts.iter().enumerate() {
                if let Some(pl) = check_plane(a, b, c) {
                    planes += 1;
                    if i < j && j < k {
                        for &q in &pts {
                            check_side(&pl, a, b, c, q);
                        }
                    }
                }
            }
        }
    }
    // 27·26·25 ordered triples of distinct points, minus the collinear ones: 49 lines of three
    // points in a 3 × 3 × 3 grid, each giving 6 ordered triples.
    assert_eq!(planes, 27 * 26 * 25 - 49 * 6);
}

/// Every ordered triple of points of {−M, 0, M}³: degeneracy and coefficients exact, every side
/// exact at all 27 points, and nothing over budget. The largest values reached are exactly the
/// maxima documented on `Plane::through`: 4M² for a normal component and 4M³ for the offset.
#[test]
fn plane_exhaustive_frame_corners_and_centres() {
    let s = [-M, 0, M];
    let pts: Vec<P3> = (0..27).map(|i| p(s[i % 3], s[(i / 3) % 3], s[i / 9])).collect();
    let mut planes = 0;
    let (mut max_n, mut max_d) = (0, 0);
    let (mut top_n, mut top_d) = (0u128, 0u128);
    for (i, &a) in pts.iter().enumerate() {
        for (j, &b) in pts.iter().enumerate() {
            for (k, &c) in pts.iter().enumerate() {
                if let Some(pl) = check_plane(a, b, c) {
                    planes += 1;
                    max_n = max_n.max(pl.n().iter().map(|v| BigInt::from(*v).bits()).max().unwrap());
                    max_d = max_d.max(BigInt::from(pl.d()).bits());
                    top_n = top_n.max(pl.n().iter().map(|v| v.unsigned_abs()).max().unwrap());
                    top_d = top_d.max(pl.d().unsigned_abs());
                    if i < j && j < k {
                        for &q in &pts {
                            check_side(&pl, a, b, c, q);
                        }
                    }
                }
            }
        }
    }
    assert_eq!(planes, 27 * 26 * 25 - 49 * 6);
    assert!(max_n <= 83 && max_d <= 125, "n {max_n} bits, d {max_d} bits");
    assert_eq!((max_n, max_d), (82, 122), "4M² and 4M³ are reached");
    let m = u128::from(M.unsigned_abs());
    assert_eq!((top_n, top_d), (4 * m * m, 4 * m * m * m), "the documented maxima, exactly");
}

/// `through` and `side` are `const fn`: usable at compile time.
const ORIGIN: P3 = match P3::new(0, 0, 0) {
    Ok(q) => q,
    Err(_) => panic!("origin"),
};
const X_END: P3 = match P3::new(COORD_MAX, 0, 0) {
    Ok(q) => q,
    Err(_) => panic!("x end"),
};
const Y_END: P3 = match P3::new(0, COORD_MAX, 0) {
    Ok(q) => q,
    Err(_) => panic!("y end"),
};
const FLOOR: Plane = match Plane::through(ORIGIN, X_END, Y_END) {
    Ok(pl) => pl,
    Err(_) => panic!("collinear"),
};
const ABOVE: Sign = FLOOR.side(match P3::new(0, 0, 1) {
    Ok(q) => q,
    Err(_) => panic!("above"),
});

#[test]
fn const_evaluation() {
    let m = i128::from(M);
    assert_eq!((FLOOR.n(), FLOOR.d()), ([0, 0, m * m], 0));
    assert_eq!(ABOVE, Sign::Positive);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    /// Random and extreme-biased points: coefficients, degeneracy and sides against the
    /// independent oracles; collinear triples orient every point to `Zero`; translating
    /// everything by a grid vector keeps n and changes d by exactly −n · t.
    #[test]
    fn plane_and_side_vs_independent_oracles(a in point(), b in point(), c in point(),
                                             qs in prop::collection::vec(point(), 6),
                                             tv in (-1000i64..=1000, -1000i64..=1000, -1000i64..=1000)) {
        match check_plane(a, b, c) {
            Some(pl) => {
                for &q in &qs {
                    check_side(&pl, a, b, c, q);
                }
                let t = [tv.0, tv.1, tv.2];
                let moved = [a, b, c].map(|q| add3i(xyz(q), t));
                if moved.iter().all(|q| in_frame(*q)) {
                    let m = Plane::through(pa(moved[0]), pa(moved[1]), pa(moved[2])).unwrap();
                    prop_assert_eq!(m.n(), pl.n());
                    let shift = dot3(&pl.n().map(BigInt::from), &t.map(BigInt::from));
                    prop_assert_eq!(BigInt::from(m.d()), BigInt::from(pl.d()) - shift);
                }
            }
            None => {
                for &q in &qs {
                    prop_assert_eq!(orient3d(a, b, c, q), Sign::Zero);
                }
            }
        }
    }

    /// Collinear points on lines across the whole frame (primitive direction q = (b − a) / g, any
    /// multiple of it) are degenerate; one step off the line is decided exactly.
    #[test]
    fn collinear_points_on_long_lines(a in point(), b in point(), s in prop::sample::select(vec![-3i64, -2, -1, 1, 2, 3]),
                                      axis in 0usize..3, sgn in prop::sample::select(vec![-1i64, 1]), q in point()) {
        let (ab, bb) = (xyz(a), xyz(b));
        let diff = [bb[0] - ab[0], bb[1] - ab[1], bb[2] - ab[2]];
        let g = diff[0].gcd(&diff[1]).gcd(&diff[2]);
        prop_assume!(g != 0);
        let dir = diff.map(|v| v / g);
        for mult in [s, g - s, g + s, -s, g / 2, 2 * g] {
            let c = add3i(ab, dir.map(|v| v * mult));
            if in_frame(c) {
                prop_assert_eq!(Plane::through(a, b, pa(c)), Err(NumError::Degenerate));
                prop_assert_eq!(Plane::through(pa(c), a, b), Err(NumError::Degenerate));
                prop_assert_eq!(orient3d(a, b, pa(c), q), Sign::Zero);
                let mut off = c;
                off[axis] += sgn;
                if !in_frame(off) {
                    off[axis] -= 2 * sgn;
                }
                if let Some(pl) = check_plane(a, b, pa(off)) {
                    check_side(&pl, a, b, pa(off), pa(c));
                    check_side(&pl, a, b, pa(off), q);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Three-plane points: degeneracy and the sign convention.
// ---------------------------------------------------------------------------------------------

/// Hand example: x = 1, y = 2, z = 3 in all six orders. Even orders give X = (1, 2, 3), w = 1; odd
/// orders X = (−1, −2, −3), w = −1. `side_of_tpi` must not depend on the order, so with w < 0 it
/// must multiply by the sign of w: the bare n · X + d · w has the wrong sign there.
#[test]
fn tpi_hand_sign_convention() {
    let g = |a, u, v| through_in_frame(a, u, v).unwrap();
    let px = g([1, 0, 0], [0, 1, 0], [0, 0, 1]);
    let py = g([0, 2, 0], [0, 0, 1], [1, 0, 0]);
    let pz = g([0, 0, 3], [1, 0, 0], [0, 1, 0]);
    assert_eq!([(px.n(), px.d()), (py.n(), py.d()), (pz.n(), pz.d())], [([1, 0, 0], -1), ([0, 1, 0], -2), ([0, 0, 1], -3)]);
    let x0 = g([0, 0, 0], [0, 1, 0], [0, 0, 1]); // x = 0, normal +x
    let x0_flipped = g([0, 0, 0], [0, 0, 1], [0, 1, 0]); // x = 0, normal −x
    let z4 = g([0, 0, 4], [1, 0, 0], [0, 1, 0]); // z = 4, normal +z
    let through_point = g([6, 0, 0], [-6, 6, 0], [-6, 0, 6]); // x + y + z = 6
    let planes = [px, py, pz];
    for (perm, even) in PERMS {
        let [a, b, c] = permuted(planes, perm);
        let t = Tpi::new(a, b, c).unwrap();
        let (x, w) = t.homogeneous();
        let s = if even { 1 } else { -1 };
        assert_eq!(x.map(|v| wide_to_big(&v)), [1, 2, 3].map(|v| BigInt::from(s * v)));
        assert_eq!(wide_to_big(&w), BigInt::from(s));
        assert_eq!(x0.side_of_tpi(&t), Sign::Positive, "order {perm:?}");
        assert_eq!(x0_flipped.side_of_tpi(&t), Sign::Negative, "order {perm:?}");
        assert_eq!(z4.side_of_tpi(&t), Sign::Negative, "order {perm:?}");
        assert_eq!(through_point.side_of_tpi(&t), Sign::Zero, "order {perm:?}");
        for pl in [a, b, c] {
            assert_eq!(pl.side_of_tpi(&t), Sign::Zero);
        }
        // The bare classification value, without the sign of w, is wrong for odd orders.
        let bare = dot3(&x0.n().map(BigInt::from), &x.map(|v| wide_to_big(&v))) + BigInt::from(x0.d()) * wide_to_big(&w);
        assert_eq!(sign_big(&bare), if even { Sign::Positive } else { Sign::Negative });
        assert_eq!(t.to_grid(), Ok(Rounded { value: p(1, 2, 3), exact: true }));
    }
}

/// Parallel, anti-parallel, scaled-parallel and repeated planes; a pencil of 32 planes through the
/// frame's long diagonal (every triple, normals near 2^82 whose determinant terms cancel exactly);
/// a prism of planes all parallel to one long direction; and the same configurations one step off,
/// which must meet in exactly the oracle's point.
#[test]
fn tpi_degenerate_configurations_at_the_extremes() {
    let g = |a, u, v| through_in_frame(a, u, v).unwrap();
    let x_lo = g([-M, 0, 0], [0, 1, 0], [0, 0, 1]);
    let x_hi = g([M, 0, 0], [0, 1, 0], [0, 0, 1]);
    let x_hi_flipped = g([M, 0, 0], [0, 0, 1], [0, 1, 0]);
    let x_0_scaled = g([0, -M, -M], [0, 2 * M, 0], [0, 0, 2 * M]);
    let y_0 = g([0, 0, 0], [0, 0, 1], [1, 0, 0]);
    let z_hi = g([0, 0, M], [1, 0, 0], [0, 1, 0]);
    let oblique = Plane::through(p(-M, -M, M), p(M, -M + 1, -M), p(-M + 3, M, -M + 7)).unwrap();
    assert_eq!(x_0_scaled.n(), [4 * i128::from(M) * i128::from(M), 0, 0]);
    assert_eq!(x_hi_flipped.n(), [-1, 0, 0]);
    for set in [
        [x_lo, x_hi, y_0],
        [x_lo, x_hi_flipped, z_hi],
        [x_hi, x_0_scaled, oblique],
        [x_lo, x_lo, y_0],
        [oblique, y_0, oblique],
        [x_hi, x_hi_flipped, x_0_scaled],
        [oblique, oblique, oblique],
    ] {
        assert!(check_tpi(set).is_none(), "{set:?}");
    }
    assert!(check_tpi([x_lo, y_0, z_hi]).is_some());
    assert!(check_tpi([x_0_scaled, oblique, z_hi]).is_some());

    // A pencil through the long diagonal, using points of the diagonal near its ends and its middle.
    let (lo, hi) = (p(-M, -M, -M), p(M, M, M));
    let thirds = [p(M, -M, -M), p(-M, M, -M), p(-M, -M, M), p(M, M, -M), p(M - 1, -M, M), p(0, 1, 0), p(-M, M, M - 1), p(1, -1, 0)];
    let mut pencil = Vec::new();
    for h in thirds {
        pencil.push(check_plane(lo, hi, h).unwrap());
        pencil.push(check_plane(hi, lo, h).unwrap());
        pencil.push(check_plane(p(0, 0, 0), p(1, 1, 1), h).unwrap());
        pencil.push(check_plane(p(-5, -5, -5), p(M - 3, M - 3, M - 3), h).unwrap());
    }
    for &a in &pencil {
        for &b in &pencil {
            for &c in &pencil {
                assert!(check_tpi([a, b, c]).is_none(), "pencil {a:?} {b:?} {c:?}");
            }
        }
    }
    // One step off the diagonal: the third plane no longer contains it.
    for h in thirds {
        let off = check_plane(lo, p(M, M, M - 1), h).unwrap();
        for (i, &a) in pencil.iter().enumerate() {
            for &b in pencil.iter().skip(i % 4 + 1).step_by(5) {
                if let Some((t, pt)) = check_tpi([a, b, off]) {
                    check_side_of_tpi(&x_hi, &t, &pt);
                    check_side_of_tpi(&oblique, &t, &pt);
                }
            }
        }
    }

    // A prism: every plane contains the long direction e, none shares a line with another.
    let e = [2 * M, 2 * M - 1, 1];
    let starts = [[-M, -M, -M], [-M, -M + 1, 0], [-M, -M, M - 1], [-M, -M + 1, -M + 5]];
    let thirds = [[M, -M, 0], [0, M, -M], [-M, M, M], [3, -7, 11]];
    let prism: Vec<Plane> = starts
        .iter()
        .zip(&thirds)
        .map(|(&s, &h)| check_plane(pa(s), pa(add3i(s, e)), pa(h)).unwrap())
        .collect();
    for &a in &prism {
        for &b in &prism {
            for &c in &prism {
                assert!(check_tpi([a, b, c]).is_none(), "prism {a:?} {b:?} {c:?}");
            }
        }
    }
    // The same prism with one plane's direction tilted by one step: one point, far away.
    let tilted = check_plane(pa(starts[0]), pa(add3i(starts[0], [e[0], e[1], e[2] - 1])), pa(thirds[0])).unwrap();
    let mut met = 0;
    for &a in &prism[1..] {
        for &b in &prism[1..] {
            if let Some((t, pt)) = check_tpi([a, b, tilted]) {
                met += 1;
                check_side_of_tpi(&oblique, &t, &pt);
                check_side_of_tpi(&prism[0], &t, &pt);
            }
        }
    }
    assert_eq!(met, 6, "each ordered pair of distinct prism planes meets the tilted plane once");
}

/// Every ordered triple of 40 small planes (spread over all distinct planes through points of
/// {−1, 0, 1}³): `Degenerate` exactly when Cramer's determinant is zero, and otherwise X, w, the
/// commit and the side against two more planes exact.
#[test]
fn tpi_exhaustive_small_planes() {
    let pts: Vec<P3> = (0..27).map(|i| p(i % 3 - 1, (i / 3) % 3 - 1, i / 9 - 1)).collect();
    let mut all: Vec<Plane> = Vec::new();
    for &a in &pts {
        for &b in &pts {
            for &c in &pts {
                if let Ok(pl) = Plane::through(a, b, c) {
                    if !all.contains(&pl) {
                        all.push(pl);
                    }
                }
            }
        }
    }
    let stride = all.len() / 40;
    let planes: Vec<Plane> = all.iter().copied().step_by(stride).take(40).collect();
    assert_eq!(planes.len(), 40);
    let (mut met, mut degenerate) = (0, 0);
    for (i, &a) in planes.iter().enumerate() {
        for (j, &b) in planes.iter().enumerate() {
            for (k, &c) in planes.iter().enumerate() {
                match check_tpi([a, b, c]) {
                    Some((t, pt)) => {
                        met += 1;
                        check_side_of_tpi(&planes[(i + j + k) % 40], &t, &pt);
                        check_side_of_tpi(&all[(i * 7 + j * 3 + k) % all.len()], &t, &pt);
                    }
                    None => degenerate += 1,
                }
            }
        }
    }
    assert!(met > 0 && degenerate > 0, "{met} met, {degenerate} degenerate");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Random extreme-biased planes: everything matches Cramer's rule; every order of the planes
    /// gives w of the permutation's sign and identical sides and commits.
    #[test]
    fn tpi_vs_cramer((p1, _) in plane3(), (p2, _) in plane3(), (p3, _) in plane3(),
                     others in prop::collection::vec(plane3(), 3)) {
        if let Some((t, pt)) = check_tpi([p1, p2, p3]) {
            let (x, w) = t.homogeneous();
            for (q, _) in &others {
                check_side_of_tpi(q, &t, &pt);
            }
            for (perm, even) in PERMS {
                let [a, b, c] = permuted([p1, p2, p3], perm);
                let u = Tpi::new(a, b, c).unwrap();
                let (ux, uw) = u.homogeneous();
                if even {
                    prop_assert_eq!((ux, uw), (x, w));
                } else {
                    prop_assert_eq!(wide_to_big(&uw), -wide_to_big(&w));
                    prop_assert_eq!(ux.map(|v| wide_to_big(&v)), x.map(|v| -wide_to_big(&v)));
                }
                for (q, _) in &others {
                    prop_assert_eq!(q.side_of_tpi(&u), q.side_of_tpi(&t));
                }
                prop_assert_eq!(u.to_grid(), t.to_grid());
            }
        }
    }

    /// Pencils (three planes through one line, using points anywhere on it) and prisms (three
    /// planes parallel to one direction) are degenerate; tilting one plane by a step is not, and
    /// meets exactly where the oracle says.
    #[test]
    fn pencils_and_prisms_are_degenerate(a in point(), b in point(), hs in prop::collection::vec(point(), 3),
                                         starts in prop::collection::vec(point(), 2), axis in 0usize..3, q in plane3()) {
        let (ab, bb) = (xyz(a), xyz(b));
        let diff = [bb[0] - ab[0], bb[1] - ab[1], bb[2] - ab[2]];
        let g = diff[0].gcd(&diff[1]).gcd(&diff[2]);
        prop_assume!(g != 0);
        let dir = diff.map(|v| v / g);
        // Pencil: through a and b, through b and a, and through a point strictly inside the segment.
        let mid = pa(add3i(ab, dir.map(|v| v * (g / 2).max(1))));
        let pencil = [Plane::through(a, b, hs[0]), Plane::through(b, a, hs[1]), Plane::through(mid, b, hs[2])];
        if let [Ok(p1), Ok(p2), Ok(p3)] = pencil {
            prop_assert!(check_tpi([p1, p2, p3]).is_none());
            if let Ok(off) = Plane::through(nudge_in(mid, axis), b, hs[2]) {
                if let Some((t, pt)) = check_tpi([p1, p2, off]) {
                    check_side_of_tpi(&q.0, &t, &pt);
                }
            }
        }
        // Prism: planes through s and s + diff for s = a and two other starts.
        let mut prism = vec![Plane::through(a, b, hs[0]).ok()];
        for s in &starts {
            let s = xyz(*s);
            let s2 = add3i(s, diff);
            prism.push(if in_frame(s2) { Plane::through(pa(s), pa(s2), hs[1]).ok() } else { None });
        }
        if let [Some(p1), Some(p2), Some(p3)] = prism[..] {
            prop_assert!(check_tpi([p1, p2, p3]).is_none());
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Grid commits.
// ---------------------------------------------------------------------------------------------

/// Exact ties with every normal scaled by k·l (k, l odd, just below 2^39), so |w| = 2 (k l)^3,
/// about 2^235, of both signs; and ties on the frame's edge with normals scaled by M.
#[test]
fn to_grid_exact_ties_with_huge_w_of_both_signs() {
    let want = |s: i64, t: i64, y0: i64| {
        let (x, z) = (half_even(s + t), half_even(t - s));
        for v in [x, y0, z] {
            if v.abs() > M {
                return Err(NumError::OutOfRange { value: i128::from(v) });
            }
        }
        Ok(Rounded { value: p(x, y0, z), exact: (s + t) % 2 == 0 })
    };
    let big = (549_755_813_881i64, 549_755_813_869i64);
    let ties = [(0, 1), (1, 2), (-3, -2), (5, -2), (12_345, 54_320), (-777_777, 2), (1 << 37, (1 << 37) + 1), (-(1 << 37) - 1, 1 << 37), (3, 5)];
    let edges = [(M, M + 1), (M, M - 1), (-M - 1, -M), (-M + 1, -M), (M - 1, M - 2), (2 * M - 2, 2 * M - 3), (-2 * M + 2, -2 * M + 3), (2 * M - 2, 2)];
    let mut cases = 0;
    for ((k, l), list) in [(big, &ties[..]), ((1, M), &edges[..]), ((1, M - 2), &edges[..])] {
        for &(s, t) in list {
            for y0 in [0, 7, -M, M] {
                for swap in [false, true] {
                    let [p1, p2, p3] = scaled_tie(s, t, y0, k, l).unwrap_or_else(|| panic!("s {s} t {t} k {k} l {l}"));
                    let planes = if swap { [p2, p1, p3] } else { [p1, p2, p3] };
                    let (tpi, _) = check_tpi(planes).unwrap();
                    let (_, w) = tpi.homogeneous();
                    let kl = BigInt::from(k) * BigInt::from(l);
                    assert_eq!(wide_to_big(&w), BigInt::from(if swap { -2 } else { 2 }) * &kl * &kl * &kl);
                    assert_eq!(tpi.to_grid(), want(s, t, y0), "s {s} t {t} y0 {y0} k {k} l {l} swap {swap}");
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, (9 + 8 + 8) * 4 * 2);
    let kl = BigInt::from(big.0) * BigInt::from(big.1);
    assert_eq!((BigInt::from(2) * &kl * &kl * &kl).bits(), 235);
}

/// Points within 1/D (D just below 2^116) of a half step, above, below and exactly on it, with w
/// of both signs, inside the frame and on its edges (M ± 1/2, −M ± 1/2), mirrored and permuted.
/// An f64 or a truncated division would get these wrong; the exact commit must not.
#[test]
fn to_grid_within_2_pow_minus_116_of_a_half_step() {
    let d = d_of(P_NEAR, [1, 1, EZ_NEAR]);
    assert_eq!(d.bits(), 116);
    let cases = [
        (0, 1),
        (0, -1),
        (5, 11),
        (5, 9),
        (-6, -11),
        (-6, -13),
        (7, 14),
        (123_456_789_012, 246_913_578_025),
        (M - 1, 2 * M - 1),
        (M - 1, 2 * M + 1),
        (M - 1, 2 * M - 3),
        (M - 1, 2 * M),
        (-M, -2 * M + 1),
        (-M, -2 * M - 1),
        (-M, -2 * M + 3),
        (-M, -2 * M),
    ];
    for (ax, x2) in cases {
        for delta in [-1i64, 0, 1] {
            for swap in [false, true] {
                let j = x2 - 2 * ax;
                let (planes, pt) = near_tie(ax, x2, delta, ID, false, swap);
                let tiny = BigRational::new(BigInt::from(delta), d.clone());
                assert_eq!(pt[0], rat(x2, 2) + &tiny, "construction");
                assert_eq!(pt[1], rat(j, 2) + &tiny, "construction");
                let (t, got) = check_tpi(planes).unwrap();
                assert_eq!(got, pt, "Cramer and the line formula disagree");
                let (_, w) = t.homogeneous();
                assert_eq!(wide_to_big(&w), if swap { d.clone() } else { -d.clone() });
                let (xr, yr, zr) = (round_near(x2, delta), round_near(j, delta), j * (EZ_NEAR / 2));
                let want = if xr.abs() > M {
                    Err(NumError::OutOfRange { value: i128::from(xr) })
                } else {
                    Ok(Rounded { value: p(xr, yr, zr), exact: delta == 0 && j % 2 == 0 })
                };
                assert_eq!(t.to_grid(), want, "x = {x2}/2 + {delta}/D, swap {swap}");
                for (perm, _) in PERMS {
                    for mirror in [false, true] {
                        let (planes, pt2) = near_tie(ax, x2, delta, perm, mirror, swap);
                        let (t2, got2) = check_tpi(planes).unwrap();
                        assert_eq!(got2, pt2);
                        if let Ok(r) = want {
                            let v = permuted([xr, yr, zr], perm).map(|c| if mirror { -c } else { c });
                            assert_eq!(t2.to_grid(), Ok(Rounded { value: pa(v), exact: r.exact }));
                        }
                    }
                }
            }
        }
    }
}

/// Points within 2^-116 of grid planes x = k, y = 0, z = 0 (and exactly on x − y = k and on planes
/// containing the defining line), with w negative and positive: `side_of_tpi` gives exactly the
/// sign of the offset, and its flip for the flipped plane.
#[test]
fn side_of_tpi_within_2_pow_minus_116_of_grid_planes() {
    for k in [0i64, 1, -1, 99, M - 1, -M] {
        for delta in [-2i64, -1, 0, 1, 2] {
            for swap in [false, true] {
                let (planes, pt) = near_tie(k, 2 * k, delta, ID, false, swap);
                let (t, _) = check_tpi(planes).unwrap();
                let (_, w) = t.homogeneous();
                assert_eq!(w.is_negative(), !swap);
                let s = Sign::of_i64(delta);
                let g = |a, u, v| through_in_frame(a, u, v).unwrap();
                let x_k = g([k, 0, 0], [0, 1, 0], [0, 0, 1]);
                let x_k_flipped = g([k, 0, 0], [0, 0, 1], [0, 1, 0]);
                let y_0 = g([0, 0, 0], [0, 0, 1], [1, 0, 0]);
                let z_0 = g([0, 0, 0], [1, 0, 0], [0, 1, 0]);
                let x_minus_y = g([k, 0, 0], [1, 1, 0], [0, 0, 1]);
                let along = g([k, 0, 0], [1, 1, EZ_NEAR], [0, 1, 0]);
                let sum = g([k, 0, 0], [1, -1, 0], [1, 0, -1]);
                assert_eq!(x_k.n(), [1, 0, 0]);
                assert_eq!(y_0.n(), [0, 1, 0]);
                assert_eq!(z_0.n(), [0, 0, 1]);
                assert_eq!(x_minus_y.n(), [1, -1, 0]);
                assert_eq!(sum.n(), [1, 1, 1]);
                assert_eq!(x_k.side_of_tpi(&t), s, "k {k} delta {delta} swap {swap}");
                assert_eq!(x_k_flipped.side_of_tpi(&t), s.negate());
                assert_eq!(y_0.side_of_tpi(&t), s);
                assert_eq!(z_0.side_of_tpi(&t), s);
                assert_eq!(sum.side_of_tpi(&t), s);
                assert_eq!(x_minus_y.side_of_tpi(&t), Sign::Zero);
                assert_eq!(along.side_of_tpi(&t), Sign::Zero);
                for pl in [x_k, x_k_flipped, y_0, z_0, x_minus_y, along, sum] {
                    check_side_of_tpi(&pl, &t, &pt);
                }
                for (perm, _) in PERMS {
                    for mirror in [false, true] {
                        let (planes, pt2) = near_tie(k, 2 * k, delta, perm, mirror, swap);
                        let (t2, _) = check_tpi(planes).unwrap();
                        for pl in [x_k, x_k_flipped, y_0, z_0, x_minus_y, along, sum] {
                            check_side_of_tpi(&pl, &t2, &pt2);
                        }
                    }
                }
            }
        }
    }
}

/// Coordinates (2^128 + j) / 2 and (−2^128 + j) / 2 with w = ∓2: the commit rounds half to even and
/// saturates only beyond `i128`, so 2^127 − 1 and −2^127 are reported exactly.
#[test]
fn to_grid_around_plus_minus_2_pow_127() {
    let two128 = BigInt::one() << 128u32;
    let (max, min) = (i128::MAX, i128::MIN);
    let up: [(i64, i128); 13] = [
        (-7, max - 3),
        (-6, max - 2),
        (-5, max - 1),
        (-4, max - 1),
        (-3, max - 1),
        (-2, max),
        (-1, max),
        (0, max),
        (1, max),
        (2, max),
        (3, max),
        (4, max),
        (5, max),
    ];
    let down: [(i64, i128); 13] = [
        (7, min + 4),
        (6, min + 3),
        (5, min + 2),
        (4, min + 2),
        (3, min + 2),
        (2, min + 1),
        (1, min),
        (0, min),
        (-1, min),
        (-2, min),
        (-3, min),
        (-4, min),
        (-5, min),
    ];
    for (base, list) in [(two128.clone(), up), (-two128.clone(), down)] {
        for (j, want) in list {
            let x2 = &base + j;
            for swap in [false, true] {
                let (planes, pt) = half_far(&x2, false, swap).unwrap();
                assert_eq!(pt[0], BigRational::new(x2.clone(), BigInt::from(2)), "construction");
                let (t, got) = check_tpi(planes).unwrap();
                assert_eq!(got, pt);
                let (_, w) = t.homogeneous();
                assert_eq!(wide_to_big(&w), BigInt::from(if swap { 2 } else { -2 }));
                assert_eq!(t.to_grid(), Err(NumError::OutOfRange { value: want }), "x = ({x2}) / 2");
                let (mirrored, mpt) = half_far(&x2, true, swap).unwrap();
                let (_, mgot) = check_tpi(mirrored).unwrap();
                assert_eq!(mgot, mpt);
                // The same point with |w| = 2 m_1 m_2, about 2^77: the 127-bit rounding path with a
                // large divisor.
                let (planes, pt) = half_far_scaled(&x2, SCALE, false, swap).unwrap();
                let (t, got) = check_tpi(planes).unwrap();
                assert_eq!(got, pt);
                let (_, w) = t.homogeneous();
                let mw = BigInt::from(2) * BigInt::from(SCALE[0]) * BigInt::from(SCALE[1]);
                assert_eq!(wide_to_big(&w), if swap { mw } else { -mw });
                assert_eq!(t.to_grid(), Err(NumError::OutOfRange { value: want }), "scaled, x = ({x2}) / 2");
            }
        }
    }
}

/// The near ties again with planes 1 and 2 scaled by about 2^38 each: |w| = m_1 m_2 D, about
/// 2^192, still deciding points 2^-116 from a half step exactly.
#[test]
fn to_grid_near_ties_with_w_near_2_pow_192() {
    let d = d_of(P_NEAR, [1, 1, EZ_NEAR]);
    let mw = BigInt::from(SCALE[0]) * BigInt::from(SCALE[1]) * &d;
    assert_eq!(mw.bits(), 192);
    for (ax, x2) in [(0, 1), (0, -1), (5, 11), (-6, -13), (7, 14), (123_456_789_012, 246_913_578_025), (-M, -2 * M + 1), (-M, -2 * M - 1)] {
        for delta in [-1i64, 0, 1] {
            for swap in [false, true] {
                let j = x2 - 2 * ax;
                let (planes, pt) = near_tie_scaled(ax, x2, delta, SCALE, ID, false, swap);
                let (t, got) = check_tpi(planes).unwrap();
                assert_eq!(got, pt);
                assert_eq!(pt[0], rat(x2, 2) + BigRational::new(BigInt::from(delta), d.clone()));
                let (_, w) = t.homogeneous();
                assert_eq!(wide_to_big(&w), if swap { mw.clone() } else { -mw.clone() });
                let (xr, yr, zr) = (round_near(x2, delta), round_near(j, delta), j * (EZ_NEAR / 2));
                let want = if xr.abs() > M {
                    Err(NumError::OutOfRange { value: i128::from(xr) })
                } else {
                    Ok(Rounded { value: p(xr, yr, zr), exact: delta == 0 && j % 2 == 0 })
                };
                assert_eq!(t.to_grid(), want, "x = {x2}/2 + {delta}/D, swap {swap}");
                let x_k = through_in_frame([ax, 0, 0], [0, 1, 0], [0, 0, 1]).unwrap();
                if j == 0 {
                    assert_eq!(x_k.side_of_tpi(&t), Sign::of_i64(delta));
                }
                check_side_of_tpi(&x_k, &t, &pt);
            }
        }
    }
}

/// Integral points with w = ∓1 at the frame's edge and at the edges of `i128`.
#[test]
fn to_grid_integral_points_at_frame_and_i128_edges() {
    let m = BigInt::from(M);
    let two127 = BigInt::one() << 127u32;
    let cases: Vec<(BigInt, Result<Rounded<P3>, NumError>)> = vec![
        (m.clone(), Ok(Rounded { value: p(M, 2, 0), exact: true })),
        (-m.clone(), Ok(Rounded { value: p(-M, -2, 0), exact: true })),
        (&m - 1, Ok(Rounded { value: p(M - 1, 2, 0), exact: true })),
        (&m + 1, Err(NumError::OutOfRange { value: 1 << 40 })),
        (-&m - 1, Err(NumError::OutOfRange { value: -(1 << 40) })),
        (&two127 - 1, Err(NumError::OutOfRange { value: i128::MAX })),
        (&two127 - 2, Err(NumError::OutOfRange { value: i128::MAX - 1 })),
        (two127.clone(), Err(NumError::OutOfRange { value: i128::MAX })),
        (&two127 + 1, Err(NumError::OutOfRange { value: i128::MAX })),
        (-two127.clone(), Err(NumError::OutOfRange { value: i128::MIN })),
        (-&two127 + 1, Err(NumError::OutOfRange { value: i128::MIN + 1 })),
        (-&two127 - 1, Err(NumError::OutOfRange { value: i128::MIN })),
    ];
    for (x, want) in cases {
        for swap in [false, true] {
            let (planes, pt) = integral_far(&x, ID, swap);
            assert_eq!(pt[0], BigRational::from_integer(x.clone()), "construction");
            let (t, got) = check_tpi(planes).unwrap();
            assert_eq!(got, pt);
            let (_, w) = t.homogeneous();
            assert_eq!(wide_to_big(&w), BigInt::from(if swap { 1 } else { -1 }));
            assert_eq!(t.to_grid(), want, "x = {x}");
        }
    }
}

/// `to_grid` reports the first out-of-range coordinate in the order x, y, z.
#[test]
fn to_grid_reports_the_first_out_of_range_axis() {
    // Unpermuted: x = −2^41 (2^39 + 1), y = −2^41, z = 5.
    let e = [(1i64 << 39) + 1, 1, 0];
    let t = BigInt::from(1i64 << 42);
    let x = -(1i128 << 41) * ((1i128 << 39) + 1);
    let y = -(1i128 << 41);
    for (perm, want) in [([0, 1, 2], x), ([2, 1, 0], y), ([2, 0, 1], x), ([1, 2, 0], y), ([1, 0, 2], y), ([0, 2, 1], x)] {
        for mirror in [false, true] {
            let (planes, pt) = line_meets_plane(P_NEAR, e, [0, 0, 5], &t, perm, mirror, false).unwrap();
            let (tpi, got) = check_tpi(planes).unwrap();
            assert_eq!(got, pt);
            let want = if mirror { -want } else { want };
            assert_eq!(tpi.to_grid(), Err(NumError::OutOfRange { value: want }), "perm {perm:?} mirror {mirror}");
        }
    }
    // x in range, y = 2^88 (exact) reported before x = 2^127 − 1 when y comes first.
    let (planes, _) = integral_far(&((BigInt::one() << 127u32) - 1), [1, 0, 2], false);
    let (tpi, _) = check_tpi(planes).unwrap();
    assert_eq!(tpi.to_grid(), Err(NumError::OutOfRange { value: 1 << 88 }));
}

/// Integral points with |w| = (k l)^3, about 2^234: the commit is exact and unmoved, and every plane
/// classifies the implicit point exactly as it classifies the grid point.
#[test]
fn to_grid_integral_points_with_huge_w_are_exact() {
    let (k, l) = (549_755_813_881i64, 549_755_813_869i64);
    let kl = BigInt::from(k) * BigInt::from(l);
    let kl3 = &kl * &kl * &kl;
    let others = [p(0, 0, 0), p(1, -2, 3), p(M, 0, -M), p(-5, M, 7), p(-M, -M, M)];
    for gp in [[3, -7, 11], [M, -M, M], [-M, M, -M], [0, 0, 0], [M, M, M]] {
        let base = [
            through_in_frame([gp[0], 0, 0], [0, l, 0], [0, 0, k]).unwrap(),
            through_in_frame([0, gp[1], 0], [0, 0, l], [k, 0, 0]).unwrap(),
            through_in_frame([0, 0, gp[2]], [l, 0, 0], [0, k, 0]).unwrap(),
        ];
        for (perm, even) in PERMS {
            let (t, pt) = check_tpi(permuted(base, perm)).unwrap();
            let (_, w) = t.homogeneous();
            assert_eq!(wide_to_big(&w), if even { kl3.clone() } else { -kl3.clone() });
            assert_eq!(t.to_grid(), Ok(Rounded { value: pa(gp), exact: true }));
            let g = pa(gp);
            let mut near = vec![g];
            for axis in 0..3 {
                near.push(nudge_in(g, axis));
            }
            for &q in &near {
                for (i, &o1) in others.iter().enumerate() {
                    for &o2 in &others[i + 1..] {
                        if let Ok(pl) = Plane::through(q, o1, o2) {
                            assert_eq!(pl.side_of_tpi(&t), pl.side(g));
                            check_side_of_tpi(&pl, &t, &pt);
                        }
                    }
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Random near ties: P odd, e = (1, e_y odd, e_z even) so D is even, the point at
    /// x = a_x + j/2 + δ/D with |D| up to about 2^116, any order, mirror and plane order. The
    /// commit, the homogeneous coordinates and the designed point all agree with the oracles.
    #[test]
    fn near_ties_random(pp in prop_oneof![3i64..=1023, (1i64 << 20)..=((1i64 << 39) - 1)].prop_map(|v| v | 1),
                        ey in prop_oneof![Just(1i64), Just(-1i64), -1023i64..=1023].prop_map(|v| v | 1),
                        ez_half in prop_oneof![Just(0i64), -(1i64 << 37)..=(1i64 << 37)],
                        ax in -M..M, j in prop::sample::select(vec![-5i64, -3, -2, -1, 0, 1, 2, 3, 5]),
                        delta in -3i64..=3, perm in perm(), mirror in any::<bool>(), swap in any::<bool>(),
                        m in prop_oneof![Just([1i64, 1]), (1i64..=(1 << 38), 1i64..=(1 << 38)).prop_map(|(a, b)| [a, b])],
                        q in plane3()) {
        let e = [1, ey, 2 * ez_half];
        let d = d_of(pp, e);
        prop_assume!(!d.is_zero());
        prop_assert!(d.is_even());
        let t = -(BigInt::from(j) * &d / BigInt::from(2)) - BigInt::from(delta);
        // A construction that does not fit the frame is a rejection, so the test fails if most did.
        let built = line_meets_plane_scaled(pp, e, [ax, 0, 0], &t, m, perm, mirror, swap);
        prop_assume!(built.is_some());
        let (planes, pt) = built.unwrap();
        let (tpi, got) = check_tpi(planes).unwrap();
        prop_assert_eq!(&got, &pt);
        check_side_of_tpi(&q.0, &tpi, &pt);
    }

    /// Half-integers ±2^k + off/2 for k from 40 to 129 with w = ∓2: the commit (or its saturated
    /// report) matches the oracle everywhere from the frame's edge to beyond `i128`.
    #[test]
    fn half_steps_from_the_frame_to_beyond_i128(kexp in 40u32..=129, off in -1000i64..=1000, neg in any::<bool>(),
                                                mirror in any::<bool>(), swap in any::<bool>(),
                                                m in prop_oneof![Just([1i64, 1]), (1i64..=(1 << 38), 1i64..=(1 << 38)).prop_map(|(a, b)| [a, b])],
                                                q in plane3()) {
        let mut x2 = (BigInt::one() << (kexp + 1)) + off;
        if neg {
            x2 = -x2;
        }
        let (planes, pt) = half_far_scaled(&x2, m, mirror, swap).expect("fits the frame for every k up to 129");
        let (t, got) = check_tpi(planes).unwrap();
        prop_assert_eq!(&got, &pt);
        check_side_of_tpi(&q.0, &t, &pt);
    }

    /// Extreme-biased planes drawn from {±M, ±(M − 1), ±(M − 2), 0, ±1}: every quantity inside its
    /// budget and every result exact.
    #[test]
    fn extreme_tpis_within_budget(pts in prop::collection::vec(
                                      (prop::sample::select(vec![M, -M, M - 1, -M + 1, M - 2, -M + 2, 0, 1, -1]),
                                       prop::sample::select(vec![M, -M, M - 1, -M + 1, M - 2, -M + 2, 0, 1, -1]),
                                       prop::sample::select(vec![M, -M, M - 1, -M + 1, M - 2, -M + 2, 0, 1, -1])), 12)) {
        let pts: Vec<P3> = pts.into_iter().map(|(x, y, z)| p(x, y, z)).collect();
        let planes: Vec<Option<Plane>> = pts.chunks(3).map(|w| check_plane(w[0], w[1], w[2])).collect();
        if let [Some(p1), Some(p2), Some(p3), Some(q)] = planes[..] {
            if let Some((t, pt)) = check_tpi([p1, p2, p3]) {
                check_side_of_tpi(&q, &t, &pt);
            }
        }
    }
}
