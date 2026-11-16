//! Plane-based geometry against exact oracles.
//!
//! Every result of `geom_num::plane` is checked against arbitrary-precision arithmetic
//! (`num-bigint`, `num-rational`) computed by a different method: planes by the cross product in
//! big integers, sides by a Leibniz determinant, three-plane points by Gaussian elimination over
//! the rationals, sides of three-plane points by evaluating the plane at the exact rational point,
//! and grid commits by rounding that rational point half to even. Inputs are random, adversarial
//! (nearly collinear, nearly parallel, points almost on a plane, ties, points far outside the
//! frame) and extreme (every coordinate at ±(2^40 − 1)), and the bit budgets the module documents
//! are asserted on every value.

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
    clippy::indexing_slicing
)]

use std::cmp::Ordering;

use geom_num::grid::COORD_MAX;
use geom_num::plane::{Plane, Tpi, determinant_sign, normals_sign};
use geom_num::{NumError, P3, Sign, Wide};
use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive, Zero};
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

// ---------------------------------------------------------------------------------------------
// Helpers and oracles.
// ---------------------------------------------------------------------------------------------

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn xyz(q: P3) -> [i64; 3] {
    [q.x.get(), q.y.get(), q.z.get()]
}

/// The point with coordinates permuted by `perm` (new axis i takes old axis perm[i]).
fn permute(q: P3, perm: [usize; 3]) -> P3 {
    let c = xyz(q);
    p(c[perm[0]], c[perm[1]], c[perm[2]])
}

fn neg(q: P3) -> P3 {
    let [x, y, z] = xyz(q);
    p(-x, -y, -z)
}

/// q + t, or `None` if it leaves the frame.
fn shift(q: P3, t: [i64; 3]) -> Option<P3> {
    let [x, y, z] = xyz(q);
    P3::new(x + t[0], y + t[1], z + t[2]).ok()
}

/// q moved one step along `axis` in direction `dir`, or the other way if that leaves the frame.
fn nudge(q: P3, axis: usize, dir: i64) -> P3 {
    let mut t = [0i64; 3];
    t[axis] = dir;
    shift(q, t).unwrap_or_else(|| {
        t[axis] = -dir;
        shift(q, t).unwrap()
    })
}

fn wide_to_big<const L: usize>(w: &Wide<L>) -> BigInt {
    let bytes: Vec<u8> = w
        .to_le_limbs()
        .iter()
        .flat_map(|l| l.to_le_bytes())
        .collect();
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

fn sub3(a: &[BigInt; 3], b: &[BigInt; 3]) -> [BigInt; 3] {
    [&a[0] - &b[0], &a[1] - &b[1], &a[2] - &b[2]]
}

fn cross3(u: &[BigInt; 3], v: &[BigInt; 3]) -> [BigInt; 3] {
    [
        &u[1] * &v[2] - &u[2] * &v[1],
        &u[2] * &v[0] - &u[0] * &v[2],
        &u[0] * &v[1] - &u[1] * &v[0],
    ]
}

fn dot3(u: &[BigInt; 3], v: &[BigInt; 3]) -> BigInt {
    &u[0] * &v[0] + &u[1] * &v[1] + &u[2] * &v[2]
}

/// The determinant of the rows of `m` by the Leibniz formula (independent of cross-then-dot).
fn det3(m: &[[BigInt; 3]; 3]) -> BigInt {
    let terms: [([usize; 3], bool); 6] = [
        ([0, 1, 2], true),
        ([1, 2, 0], true),
        ([2, 0, 1], true),
        ([0, 2, 1], false),
        ([2, 1, 0], false),
        ([1, 0, 2], false),
    ];
    let mut s = BigInt::zero();
    for (perm, even) in terms {
        let prod = &m[0][perm[0]] * &m[1][perm[1]] * &m[2][perm[2]];
        if even {
            s += prod;
        } else {
            s -= prod;
        }
    }
    s
}

/// The oracle plane through a, b, c: n = (b − a) × (c − a), d = −n · a, in big integers.
fn oracle_plane(a: P3, b: P3, c: P3) -> ([BigInt; 3], BigInt) {
    let (a, b, c) = (big3(a), big3(b), big3(c));
    let n = cross3(&sub3(&b, &a), &sub3(&c, &a));
    let d = -dot3(&n, &a);
    (n, d)
}

fn plane_big(pl: &Plane) -> ([BigInt; 3], BigInt) {
    (pl.n().map(BigInt::from), BigInt::from(pl.d()))
}

/// An exact rational point, also held over its least common denominator so that evaluating a plane
/// at it is integer arithmetic: `coords_i` = `num_i` / den with den > 0.
#[derive(Debug)]
struct Exact {
    coords: [BigRational; 3],
    num: [BigInt; 3],
    den: BigInt,
}

impl Exact {
    fn new(coords: [BigRational; 3]) -> Exact {
        let den = coords.iter().fold(BigInt::from(1), |l, c| l.lcm(c.denom()));
        let num = std::array::from_fn(|i| coords[i].numer() * (&den / coords[i].denom()));
        Exact { coords, num, den }
    }

    /// The sign of the plane evaluated at this point, n · P + d, as (n · num + d · den) / den.
    fn side(&self, pl: &Plane) -> Sign {
        let (n, d) = plane_big(pl);
        sign_big(&(dot3(&n, &self.num) + d * &self.den))
    }
}

/// The point where three planes meet, by Gaussian elimination over the rationals, or `None` when
/// they do not meet in exactly one point.
fn solve(planes: &[Plane; 3]) -> Option<[BigRational; 3]> {
    let mut m: Vec<Vec<BigRational>> = planes
        .iter()
        .map(|pl| {
            let mut row: Vec<BigRational> = pl
                .n()
                .iter()
                .map(|&v| BigRational::from_integer(BigInt::from(v)))
                .collect();
            row.push(BigRational::from_integer(-BigInt::from(pl.d())));
            row
        })
        .collect();
    for col in 0..3 {
        let pivot = (col..3).find(|&r| !m[r][col].is_zero())?;
        m.swap(col, pivot);
        let pivot_row = m[col].clone();
        for (r, row) in m.iter_mut().enumerate() {
            if r != col && !row[col].is_zero() {
                let f = &row[col] / &pivot_row[col];
                for k in col..4 {
                    row[k] -= &f * &pivot_row[k];
                }
            }
        }
    }
    Some(std::array::from_fn(|i| &m[i][3] / &m[i][i]))
}

/// Round half to even, exactly.
fn round_half_even(v: &BigRational) -> BigInt {
    let fl = v.floor().to_integer();
    let frac = v - BigRational::from_integer(fl.clone());
    let half = BigRational::new(BigInt::from(1), BigInt::from(2));
    match frac.cmp(&half) {
        Ordering::Greater => fl + 1,
        Ordering::Less => fl,
        Ordering::Equal => {
            if fl.is_even() {
                fl
            } else {
                fl + 1
            }
        }
    }
}

/// What `to_grid` must return for the exact point `pt`.
fn expected_to_grid(pt: &[BigRational; 3]) -> Result<Commit, NumError> {
    let mut out = [0i64; 3];
    for (o, v) in out.iter_mut().zip(pt) {
        let r = round_half_even(v);
        if r.abs() > BigInt::from(M) {
            let saturated = if r.is_negative() {
                i128::MIN
            } else {
                i128::MAX
            };
            return Err(NumError::OutOfRange {
                value: r.to_i128().unwrap_or(saturated),
            });
        }
        *o = r.to_i64().unwrap();
    }
    Ok(Commit {
        value: p(out[0], out[1], out[2]),
        exact: pt.iter().all(BigRational::is_integer),
    })
}

/// The largest magnitude bit-lengths seen, to show the tests reach near each budget.
#[derive(Default, Debug)]
struct MaxBits {
    n: u64,
    d: u64,
    w: u64,
    x: u64,
    class: u64,
}

/// Checks `Plane::through` against the oracle, with the budget, and returns the plane.
fn check_plane(a: P3, b: P3, c: P3, max: &mut MaxBits) -> Option<Plane> {
    let (n, d) = oracle_plane(a, b, c);
    match Plane::through(a, b, c) {
        Err(e) => {
            assert_eq!(e, NumError::Degenerate);
            assert!(
                n.iter().all(BigInt::is_zero),
                "{a:?} {b:?} {c:?} are not collinear"
            );
            None
        }
        Ok(pl) => {
            assert!(
                !n.iter().all(BigInt::is_zero),
                "{a:?} {b:?} {c:?} are collinear"
            );
            assert_eq!(plane_big(&pl), (n.clone(), d.clone()));
            for ni in &n {
                assert!(ni.bits() <= 83, "normal over budget");
                max.n = max.n.max(ni.bits());
            }
            assert!(d.bits() <= 125, "offset over budget");
            max.d = max.d.max(d.bits());
            // The defining points lie exactly on it.
            for q in [a, b, c] {
                assert_eq!(pl.side(q).unwrap(), Sign::Zero);
            }
            Some(pl)
        }
    }
}

/// Checks `side` against two oracles: n · q + d in big integers, and the Leibniz determinant of
/// [b − a, c − a, q − a] (the orientation of a, b, c, q).
fn check_side(pl: &Plane, a: P3, b: P3, c: P3, q: P3) {
    let (n, d) = plane_big(pl);
    let v = dot3(&n, &big3(q)) + d;
    assert!(v.bits() <= 126, "side over budget");
    let (ab, bb, cb, qb) = (big3(a), big3(b), big3(c), big3(q));
    let orient = det3(&[sub3(&bb, &ab), sub3(&cb, &ab), sub3(&qb, &ab)]);
    assert_eq!(pl.side(q).unwrap(), sign_big(&v));
    assert_eq!(pl.side(q).unwrap(), sign_big(&orient));
}

/// How the oracle finds the exact three-plane point.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Oracle {
    /// Gaussian elimination over the rationals, independent of the code under test.
    Eliminate,
    /// Substitution, for large sweeps (no rational reductions): the point returned must satisfy all
    /// three plane equations exactly in big integers, and w must be the Leibniz determinant, which
    /// is non-zero. A non-singular system has exactly one solution, so this proves the point.
    Substitute,
}

/// Checks `Tpi::new`, `homogeneous` and `to_grid` against the oracle (Gaussian elimination), and
/// that the three defining planes classify the point as on them. Returns the point and its exact
/// position.
fn check_tpi(planes: [Plane; 3], max: &mut MaxBits) -> Option<(Tpi, Exact)> {
    check_tpi_with(planes, Oracle::Eliminate, max)
}

fn check_tpi_with(planes: [Plane; 3], oracle: Oracle, max: &mut MaxBits) -> Option<(Tpi, Exact)> {
    let det = det3(&planes.map(|pl| pl.n().map(BigInt::from)));
    let [a, b, c] = planes;
    let t = match Tpi::new(a, b, c) {
        Err(e) => {
            assert_eq!(e, NumError::Degenerate);
            assert!(det.is_zero(), "{planes:?} meet in one point");
            if oracle == Oracle::Eliminate {
                assert!(
                    solve(&planes).is_none(),
                    "elimination and determinant disagree"
                );
            }
            return None;
        }
        Ok(t) => t,
    };
    assert!(!det.is_zero(), "{planes:?} do not meet in one point");
    assert_eq!(t.planes(), &planes);
    let (x, w) = t.homogeneous().unwrap();
    let wb = wide_to_big(&w);
    let xb = x.map(|xi| wide_to_big(&xi));
    assert_eq!(wb, det, "w is not the determinant of the normals");
    assert!(wb.bits() <= 252, "w over budget");
    max.w = max.w.max(wb.bits());
    for xi in &xb {
        assert!(xi.bits() <= 294, "numerator over budget");
        max.x = max.x.max(xi.bits());
    }
    let coords: [BigRational; 3] =
        std::array::from_fn(|i| BigRational::new(xb[i].clone(), wb.clone()));
    match oracle {
        Oracle::Eliminate => assert_eq!(solve(&planes).unwrap(), coords, "wrong point"),
        Oracle::Substitute => {
            for pl in &planes {
                let (n, d) = plane_big(pl);
                assert!(
                    (dot3(&n, &xb) + d * &wb).is_zero(),
                    "the point is not on {pl:?}"
                );
            }
        }
    }
    let pt = Exact::new(coords);
    for pl in &planes {
        assert_eq!(pt.side(pl), Sign::Zero);
        assert_eq!(pl.side_of_tpi(&t).unwrap(), Sign::Zero);
    }
    assert_eq!(
        t.commit(),
        expected_to_grid(&pt.coords),
        "to_grid of {pt:?}"
    );
    Some((t, pt))
}

/// Checks `side_of_tpi` against the plane evaluated at the exact rational point, with the budget.
fn check_side_of_tpi(pl: &Plane, t: &Tpi, pt: &Exact, max: &mut MaxBits) {
    assert_eq!(
        pl.side_of_tpi(t).unwrap(),
        pt.side(pl),
        "side_of_tpi of {pt:?}"
    );
    let (x, w) = t.homogeneous().unwrap();
    let (n, d) = plane_big(pl);
    let x = x.map(|xi| wide_to_big(&xi));
    let class = dot3(&n, &x) + d * wide_to_big(&w);
    assert!(class.bits() <= 379, "classification over budget");
    max.class = max.class.max(class.bits());
}

// ---------------------------------------------------------------------------------------------
// Strategies.
// ---------------------------------------------------------------------------------------------

/// A coordinate: anywhere in the frame, at or near its edges, or small (to hit degeneracies).
fn coord() -> impl Strategy<Value = i64> {
    prop_oneof![
        4 => -M..=M,
        2 => prop::sample::select(vec![M, -M, M - 1, -M + 1, 0, 1, -1]),
        1 => (M - 1000)..=M,
        1 => -M..=(-M + 1000),
        2 => -4i64..=4,
    ]
}

fn point() -> impl Strategy<Value = P3> {
    (coord(), coord(), coord()).prop_map(|(x, y, z)| p(x, y, z))
}

fn small_point() -> impl Strategy<Value = P3> {
    (-2i64..=2, -2i64..=2, -2i64..=2).prop_map(|(x, y, z)| p(x, y, z))
}

/// A point in [−r, r]^3.
fn point_within(r: i64) -> impl Strategy<Value = P3> {
    (-r..=r, -r..=r, -r..=r).prop_map(|(x, y, z)| p(x, y, z))
}

/// A plane through three random points, with the points.
fn plane() -> impl Strategy<Value = (Plane, [P3; 3])> {
    (point(), point(), point()).prop_filter_map("collinear", |(a, b, c)| {
        Plane::through(a, b, c).ok().map(|pl| (pl, [a, b, c]))
    })
}

fn small_plane() -> impl Strategy<Value = Plane> {
    (small_point(), small_point(), small_point())
        .prop_filter_map("collinear", |(a, b, c)| Plane::through(a, b, c).ok())
}

const PERMUTATIONS: [([usize; 3], bool); 6] = [
    ([0, 1, 2], true),
    ([1, 2, 0], true),
    ([2, 0, 1], true),
    ([0, 2, 1], false),
    ([2, 1, 0], false),
    ([1, 0, 2], false),
];

// ---------------------------------------------------------------------------------------------
// Planes.
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// `through` gives exactly the oracle's coefficients, fails exactly on collinear points, and
    /// `side` agrees with both side oracles.
    #[test]
    fn plane_matches_oracle(a in point(), b in point(), c in point(), qs in prop::collection::vec(point(), 4)) {
        let mut max = MaxBits::default();
        if let Some(pl) = check_plane(a, b, c, &mut max) {
            for q in qs {
                check_side(&pl, a, b, c, q);
            }
        }
    }

    /// Small coordinates make collinear triples common: degeneracy is decided exactly.
    #[test]
    fn plane_degenerate_exactly(a in small_point(), b in small_point(), c in small_point(), q in small_point()) {
        let mut max = MaxBits::default();
        if let Some(pl) = check_plane(a, b, c, &mut max) {
            check_side(&pl, a, b, c, q);
        }
    }

    /// Collinear points anywhere in the frame are degenerate; one step off the line is not.
    #[test]
    fn collinear_and_nearly_collinear(a in point(), dir in point_within(1 << 20), k in -64i64..=64, j in -64i64..=64,
                                      axis in 0usize..3, sgn in prop::sample::select(vec![-1i64, 1])) {
        let along = |s: i64| {
            let [dx, dy, dz] = xyz(dir);
            shift(a, [dx * s, dy * s, dz * s])
        };
        if let (Some(b), Some(c)) = (along(k), along(j)) {
            prop_assert_eq!(Plane::through(a, b, c), Err(NumError::Degenerate));
            let mut max = MaxBits::default();
            let c2 = nudge(c, axis, sgn);
            if let Some(pl) = check_plane(a, b, c2, &mut max) {
                check_side(&pl, a, b, c2, c);
            }
        }
    }

    /// Points exactly on a plane are `Zero`; one step off it agrees with the oracle.
    #[test]
    fn points_almost_on_a_plane(a in point_within(M / 16), b in point_within(M / 16), c in point_within(M / 16),
                                s in -3i64..=3, t in -3i64..=3, axis in 0usize..3, sgn in prop::sample::select(vec![-1i64, 1])) {
        let mut max = MaxBits::default();
        if let Some(pl) = check_plane(a, b, c, &mut max) {
            let (ab, bb, cb) = (xyz(a), xyz(b), xyz(c));
            let on: [i64; 3] = std::array::from_fn(|i| ab[i] + s * (bb[i] - ab[i]) + t * (cb[i] - ab[i]));
            let g = p(on[0], on[1], on[2]);
            prop_assert_eq!(pl.side(g).unwrap(), Sign::Zero);
            check_side(&pl, a, b, c, nudge(g, axis, sgn));
        }
    }

    /// Laws: swapping two points flips every side; rotating them changes nothing; translating
    /// everything by a grid vector changes nothing; mirroring through the origin flips every side.
    #[test]
    fn plane_laws(a in point_within(M / 2), b in point_within(M / 2), c in point_within(M / 2),
                  q in point_within(M / 2), t in point_within(M / 2)) {
        if let Ok(pl) = Plane::through(a, b, c) {
            let s = pl.side(q).unwrap();
            prop_assert_eq!(Plane::through(a, c, b).unwrap().side(q).unwrap(), s.negate());
            prop_assert_eq!(Plane::through(b, a, c).unwrap().side(q).unwrap(), s.negate());
            prop_assert_eq!(Plane::through(b, c, a).unwrap().side(q).unwrap(), s);
            prop_assert_eq!(Plane::through(c, a, b).unwrap().side(q).unwrap(), s);
            let tv = xyz(t);
            let moved = Plane::through(shift(a, tv).unwrap(), shift(b, tv).unwrap(), shift(c, tv).unwrap()).unwrap();
            prop_assert_eq!(moved.n(), pl.n());
            prop_assert_eq!(moved.side(shift(q, tv).unwrap()).unwrap(), s);
            let mirrored = Plane::through(neg(a), neg(b), neg(c)).unwrap();
            prop_assert_eq!(mirrored.side(neg(q)).unwrap(), s.negate());
        }
    }
}

/// The 8 corners of the frame's cube, every coordinate ±(2^40 − 1).
fn corners() -> Vec<P3> {
    let s = |bit: i32| if bit == 0 { M } else { -M };
    (0..8).map(|i| p(s(i & 1), s(i & 2), s(i & 4))).collect()
}

/// Every plane through three corners of the frame's cube, and through two corners and a point one
/// step from a corner, tested at every corner and near-corner point: the largest normals and
/// offsets the frame allows, with no overflow.
#[test]
fn extreme_planes() {
    let near = [M, M - 1, -M + 1, -M];
    let mut pts = Vec::new();
    for &x in &near {
        for &y in &near {
            for &z in &near {
                pts.push(p(x, y, z));
            }
        }
    }
    let corners = corners();
    let mut max = MaxBits::default();
    let mut planes = 0;
    for &a in &corners {
        for &b in &corners {
            for &c in &corners {
                if let Some(pl) = check_plane(a, b, c, &mut max) {
                    planes += 1;
                    for &q in &pts {
                        check_side(&pl, a, b, c, q);
                    }
                }
            }
            for &c in &pts {
                if let Some(pl) = check_plane(a, b, c, &mut max) {
                    for &q in &corners {
                        check_side(&pl, a, b, c, q);
                    }
                }
            }
        }
    }
    // No three corners of a cube are collinear: every ordered triple of distinct corners is a plane.
    assert_eq!(planes, 8 * 7 * 6);
    // Budgets 2^83 and 2^125. A triangle in the cube has normal components at most 4M² (twice the
    // largest triangle in a square of side 2M) and offset at most about 4M³ (a corner-cut triangle
    // such as x + y + z = M, or a face), which these reach: 82 and 122 bits.
    assert_eq!(max.n, 82, "{max:?}");
    assert_eq!(max.d, 122, "{max:?}");
}

// ---------------------------------------------------------------------------------------------
// Three-plane points.
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Random planes: the point, its homogeneous coordinates, its grid commit and its side against
    /// a fourth plane all match the oracle.
    #[test]
    fn tpi_matches_oracle((p1, _) in plane(), (p2, _) in plane(), (p3, _) in plane(), others in prop::collection::vec(plane(), 3)) {
        let mut max = MaxBits::default();
        if let Some((t, pt)) = check_tpi([p1, p2, p3], &mut max) {
            for (pl, _) in &others {
                check_side_of_tpi(pl, &t, &pt, &mut max);
            }
        }
    }

    /// Small planes make dependent normals common: degeneracy is decided exactly.
    #[test]
    fn tpi_degenerate_exactly(p1 in small_plane(), p2 in small_plane(), p3 in small_plane(), q in small_plane()) {
        let mut max = MaxBits::default();
        if let Some((t, pt)) = check_tpi([p1, p2, p3], &mut max) {
            check_side_of_tpi(&q, &t, &pt, &mut max);
        }
    }

    /// Parallel planes, planes through one line and a repeated plane are degenerate.
    #[test]
    fn tpi_structural_degeneracies((p1, [a, b, c]) in plane(), (p2, _) in plane(), off in point_within(4), h in point()) {
        prop_assert_eq!(Tpi::new(p1, p1, p2), Err(NumError::Degenerate));
        prop_assert_eq!(Tpi::new(p2, p1, p1), Err(NumError::Degenerate));
        // The same triangle moved: a parallel plane (same normal).
        let tv = xyz(off);
        if let (Some(a2), Some(b2), Some(c2)) = (shift(a, tv), shift(b, tv), shift(c, tv)) {
            let parallel = Plane::through(a2, b2, c2).unwrap();
            prop_assert_eq!(parallel.n(), p1.n());
            prop_assert_eq!(Tpi::new(p1, parallel, p2), Err(NumError::Degenerate));
        }
        // A pencil: three planes through the line a–b.
        if let (Ok(q1), Ok(q2)) = (Plane::through(a, b, h), Plane::through(b, a, nudge(h, 0, 1))) {
            prop_assert_eq!(Tpi::new(p1, q1, q2), Err(NumError::Degenerate));
        }
    }

    /// Nearly parallel planes: the same triangle with one vertex moved by one step, so the planes
    /// meet at a sharp angle, often far away. Everything still matches the oracle exactly.
    #[test]
    fn tpi_nearly_parallel(a in point(), b in point(), c in point(), axes in (0usize..3, 0usize..3),
                           sgns in (prop::sample::select(vec![-1i64, 1]), prop::sample::select(vec![-1i64, 1])),
                           (p3, _) in plane(), (other, _) in plane()) {
        if let Ok(p1) = Plane::through(a, b, c) {
            let p2 = Plane::through(nudge(a, axes.0, sgns.0), b, c);
            let p2b = Plane::through(a, nudge(b, axes.1, sgns.1), c);
            let mut max = MaxBits::default();
            let triples = [p2.map(|p2| [p1, p2, p3]), p2.and_then(|p2| p2b.map(|p2b| [p1, p2, p2b]))];
            for planes in triples.into_iter().flatten() {
                if let Some((t, pt)) = check_tpi(planes, &mut max) {
                    check_side_of_tpi(&other, &t, &pt, &mut max);
                    check_side_of_tpi(&p1, &t, &pt, &mut max);
                }
            }
        }
    }

    /// Three planes through one grid point g meet exactly at g: the commit is exact, and every
    /// plane classifies the implicit point exactly as it classifies g, including planes through g
    /// and one step away from it.
    #[test]
    fn tpi_on_a_grid_point(g in point(), others in prop::collection::vec(point(), 6), (pl, _) in plane(),
                           u in point(), v in point(), axis in 0usize..3) {
        let planes: Vec<Plane> = others.chunks(2).filter_map(|w| Plane::through(g, w[0], w[1]).ok()).collect();
        if let [p1, p2, p3] = planes[..] {
            let mut max = MaxBits::default();
            if let Some((t, pt)) = check_tpi([p1, p2, p3], &mut max) {
                prop_assert_eq!(t.commit(), Ok(Commit { value: g, exact: true }));
                check_side_of_tpi(&pl, &t, &pt, &mut max);
                prop_assert_eq!(pl.side_of_tpi(&t).unwrap(), pl.side(g).unwrap());
                for q in [g, nudge(g, axis, 1), nudge(g, axis, -1)] {
                    if let Ok(near) = Plane::through(q, u, v) {
                        check_side_of_tpi(&near, &t, &pt, &mut max);
                        prop_assert_eq!(near.side_of_tpi(&t).unwrap(), near.side(g).unwrap());
                    }
                }
            }
        }
    }

    /// A point off the grid lying exactly on a fourth plane: planes 1 and 2 meet in the line
    /// through grid points a and b, so every plane containing that line passes exactly through the
    /// (usually fractional) point where plane 3 crosses it, and classifies it as `Zero`; tilting it
    /// by one step does not.
    #[test]
    fn tpi_on_a_fourth_plane(a in point(), b in point(), h1 in point(), h2 in point(), h in point(),
                             (p3, _) in plane(), axis in 0usize..3) {
        if let (Ok(p1), Ok(p2), Ok(p4)) = (Plane::through(a, b, h1), Plane::through(a, b, h2), Plane::through(b, a, h)) {
            let mut max = MaxBits::default();
            if let Some((t, pt)) = check_tpi([p1, p2, p3], &mut max) {
                prop_assert_eq!(p4.side_of_tpi(&t).unwrap(), Sign::Zero);
                check_side_of_tpi(&p4, &t, &pt, &mut max);
                if let Ok(tilted) = Plane::through(nudge(a, axis, 1), b, h) {
                    check_side_of_tpi(&tilted, &t, &pt, &mut max);
                }
            }
        }
    }

    /// Laws: permuting the planes keeps the point, with (X, w) negated by odd permutations;
    /// `side_of_tpi` ignores the order; flipping the fourth plane flips the side.
    #[test]
    fn tpi_permutation_laws((p1, _) in plane(), (p2, _) in plane(), (p3, _) in plane(), (q, [a, b, c]) in plane()) {
        if let Ok(t) = Tpi::new(p1, p2, p3) {
            let (x, w) = t.homogeneous().unwrap();
            let side = q.side_of_tpi(&t).unwrap();
            let flipped = Plane::through(a, c, b).unwrap();
            prop_assert_eq!(flipped.side_of_tpi(&t).unwrap(), side.negate());
            let grid = t.commit();
            let planes = [p1, p2, p3];
            for (perm, even) in PERMUTATIONS {
                let u = Tpi::new(planes[perm[0]], planes[perm[1]], planes[perm[2]]).unwrap();
                let (ux, uw) = u.homogeneous().unwrap();
                if even {
                    prop_assert_eq!((ux, uw), (x, w));
                } else {
                    prop_assert_eq!(uw.checked_neg(), Some(w));
                    prop_assert_eq!(ux.map(|v| v.checked_neg().unwrap()), x);
                }
                prop_assert_eq!(q.side_of_tpi(&u).unwrap(), side);
                prop_assert_eq!(u.commit(), grid);
            }
        }
    }

    /// Laws: translating every plane by a grid vector translates the point (and its side does not
    /// change); by an even vector, the grid commit moves by exactly that vector (ties to even are
    /// preserved). Mirroring through the origin mirrors the commit, ties included.
    #[test]
    fn tpi_translation_and_mirror_laws(pts in prop::collection::vec(point_within(M / 4), 12), half in point_within(M / 8)) {
        let tv = xyz(half).map(|v| 2 * v);
        let make = |f: &dyn Fn(P3) -> P3| -> Option<[Plane; 4]> {
            let mut out = Vec::new();
            for w in pts.chunks(3) {
                out.push(Plane::through(f(w[0]), f(w[1]), f(w[2])).ok()?);
            }
            out.try_into().ok()
        };
        if let Some([p1, p2, p3, q]) = make(&|x| x) {
            if let Ok(t) = Tpi::new(p1, p2, p3) {
                let [m1, m2, m3, mq] = make(&|x| shift(x, tv).unwrap()).unwrap();
                let moved = Tpi::new(m1, m2, m3).unwrap();
                prop_assert_eq!(mq.side_of_tpi(&moved).unwrap(), q.side_of_tpi(&t).unwrap());
                // Where the commit and its shift both lie in the frame, the moved commit is the shifted one.
                if let Ok(r) = t.commit() {
                    if let Some(value) = shift(r.value, tv) {
                        prop_assert_eq!(moved.commit(), Ok(Commit { value, exact: r.exact }));
                    }
                }
                let [n1, n2, n3, nq] = make(&neg).unwrap();
                let mirrored = Tpi::new(n1, n2, n3).unwrap();
                prop_assert_eq!(nq.side_of_tpi(&mirrored).unwrap(), q.side_of_tpi(&t).unwrap().negate());
                match (t.commit(), mirrored.commit()) {
                    (Ok(r), got) => prop_assert_eq!(got, Ok(Commit { value: neg(r.value), exact: r.exact })),
                    (Err(NumError::OutOfRange { value }), Err(NumError::OutOfRange { value: got })) => {
                        if value == i128::MIN || value == i128::MAX {
                            // Saturated (or at the edge of `i128`): only the side is certain.
                            prop_assert_eq!(got.signum(), -value.signum());
                        } else {
                            prop_assert_eq!(got, -value);
                        }
                    }
                    (want, got) => prop_assert!(false, "{:?} mirrored to {:?}", want, got),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Grid commits: ties and far points.
// ---------------------------------------------------------------------------------------------

/// The largest |s| and |t| that `tie_planes` accepts.
const TIE_MAX: i64 = 2 * M - 2;

/// Planes x − z = s, x + z = t and y = y0, each made through three grid points inside the frame,
/// so the point is ((s + t) / 2, y0, (t − s) / 2): a tie on x and z whenever s + t is odd, and
/// outside the frame when |s + t| or |t − s| is large. Needs |s|, |t| ≤ 2^41 − 4.
fn tie_planes(s: i64, t: i64, y0: i64) -> [Plane; 3] {
    // x − z = s through (h, 0, h − s), (h, 1, h − s), (h + 1, 0, h + 1 − s) with h = floor(s / 2).
    let h = s.div_euclid(2);
    let p1 = Plane::through(p(h, 0, h - s), p(h, 1, h - s), p(h + 1, 0, h + 1 - s)).unwrap();
    // x + z = t through (h, 0, t − h), (h, 1, t − h), (h + 1, 0, t − h − 1) with h = floor(t / 2).
    let h = t.div_euclid(2);
    let p2 = Plane::through(p(h, 0, t - h), p(h, 1, t - h), p(h + 1, 0, t - h - 1)).unwrap();
    let p3 = Plane::through(p(0, y0, 0), p(1, y0, 0), p(0, y0, 1)).unwrap();
    [p1, p2, p3]
}

#[test]
fn ties_round_to_even() {
    let mut max = MaxBits::default();
    let commit = |s: i64, t: i64| {
        let [a, b, c] = tie_planes(s, t, 7);
        Tpi::new(a, b, c).unwrap().commit()
    };
    // (x, z) = (1/2, 1/2), (3/2, 1/2), (−1/2, −3/2), (5/2, −1/2), (−5/2, 1/2).
    let cases = [
        (0, 1, 0, 0),
        (1, 2, 2, 0),
        (1, -2, 0, -2),
        (3, 2, 2, 0),
        (-3, -2, -2, 0),
    ];
    for (s, t, x, z) in cases {
        assert_eq!(
            commit(s, t),
            Ok(Commit {
                value: p(x, 7, z),
                exact: false
            }),
            "s = {s}, t = {t}"
        );
    }
    // Not a tie: s + t even gives an exact point.
    assert_eq!(
        commit(3, 5),
        Ok(Commit {
            value: p(4, 7, 1),
            exact: true
        })
    );
    // At the frame's edge (M = 2^40 − 1 is odd): x = M + 1/2 is a tie between M and the even
    // 2^40, which is outside the frame.
    assert_eq!(
        commit(M, M + 1),
        Err(NumError::OutOfRange { value: 1 << 40 })
    );
    assert_eq!(
        commit(-M - 1, -M),
        Err(NumError::OutOfRange { value: -(1 << 40) })
    );
    // x = M − 1/2 is a tie between the even M − 1 and M: inside. z = ∓1/2 rounds to 0.
    assert_eq!(
        commit(M, M - 1),
        Ok(Commit {
            value: p(M - 1, 7, 0),
            exact: false
        })
    );
    assert_eq!(
        commit(-M + 1, -M),
        Ok(Commit {
            value: p(-M + 1, 7, 0),
            exact: false
        })
    );
    // x = M − 3/2 is a tie between M − 2 (odd) and the even M − 1.
    assert_eq!(
        commit(M - 1, M - 2),
        Ok(Commit {
            value: p(M - 1, 7, 0),
            exact: false
        })
    );
    // x exactly M, z exactly 2 − M: on the frame's edge, exact.
    assert_eq!(
        commit(TIE_MAX, 2),
        Ok(Commit {
            value: p(M, 7, 2 - M),
            exact: true
        })
    );
    // x = 2^41 − 9/2 on the far side of the frame: reported rounded, half to even.
    assert_eq!(
        commit(TIE_MAX, TIE_MAX - 1),
        Err(NumError::OutOfRange {
            value: (1 << 41) - 4
        })
    );
    for (s, t) in [
        (0, 1),
        (1, 2),
        (-3, -2),
        (M, M + 1),
        (-M - 1, -M),
        (M, M - 1),
        (TIE_MAX, 2),
        (TIE_MAX, TIE_MAX - 1),
        (-TIE_MAX, TIE_MAX),
    ] {
        check_tpi(tie_planes(s, t, 7), &mut max).unwrap();
        check_tpi(tie_planes(s, t, -M), &mut max).unwrap();
    }
}

/// Far points. Plane 3 has normal (1, −P, P²) and the line where planes 1 and 2 meet runs along
/// e = (k, P, 1), with (1, −P, P²) · e = k: a line nearly parallel to a plane, so they meet up to
/// about 2^160 steps away. The points are then permuted by `perm` and negated if `mirror`.
fn far_planes(pp: i64, k: i64, a: P3, b: P3, perm: [usize; 3], mirror: bool) -> Option<[Plane; 3]> {
    let at = |q: P3, d: [i64; 3]| {
        let r = permute(shift(q, d)?, perm);
        Some(if mirror { neg(r) } else { r })
    };
    let (e, u, v) = ([k, pp, 1], [pp, 1, 0], [0, pp, 1]);
    let p1 = Plane::through(at(a, [0; 3])?, at(a, e)?, at(a, [1, 0, 0])?).ok()?;
    let p2 = Plane::through(at(a, [0; 3])?, at(a, e)?, at(a, [0, 0, 1])?).ok()?;
    let p3 = Plane::through(at(b, [0; 3])?, at(b, u)?, at(b, v)?).ok()?;
    Some([p1, p2, p3])
}

#[test]
fn far_points_report_their_rounded_coordinate() {
    let mut max = MaxBits::default();
    let (a, b) = (p(0, 0, M - 1), p(-M, -M, 0));
    let m = i128::from(M);
    let far = m * (m - 1) * (m - 1);
    let commit = |perm, mirror| {
        let (t, pt) = check_tpi(
            far_planes(M, 1, a, b, perm, mirror).unwrap(),
            &mut MaxBits::default(),
        )
        .unwrap();
        // The line meets the plane at a − M(M − 1)² e: a grid point, about 2^160 steps away.
        assert!(pt.coords.iter().all(BigRational::is_integer));
        t.commit()
    };
    // x = −M(M − 1)², about −2^120: outside the frame, reported exactly.
    assert_eq!(
        commit([0, 1, 2], false),
        Err(NumError::OutOfRange { value: -far })
    );
    assert_eq!(
        commit([0, 1, 2], true),
        Err(NumError::OutOfRange { value: far })
    );
    // Axes swapped so x is −M²(M − 1)², about −2^160: beyond `i128`, saturated.
    assert_eq!(
        commit([1, 0, 2], false),
        Err(NumError::OutOfRange { value: i128::MIN })
    );
    assert_eq!(
        commit([1, 0, 2], true),
        Err(NumError::OutOfRange { value: i128::MAX })
    );
    // k = 2 halves the distance and leaves a fraction; the oracle decides it.
    for k in [-3, -2, 2, 3] {
        for (perm, _) in PERMUTATIONS {
            check_tpi(far_planes(M, k, a, b, perm, false).unwrap(), &mut max).unwrap();
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Ties anywhere in and around the frame commit exactly as the rational oracle rounds them.
    #[test]
    fn ties_match_oracle(s in prop_oneof![-TIE_MAX..=TIE_MAX, (M - 4)..=(M + 4), (-M - 4)..=(-M + 4)],
                         t in prop_oneof![-TIE_MAX..=TIE_MAX, (M - 4)..=(M + 4), (-M - 4)..=(-M + 4)],
                         y0 in coord(), (q, _) in plane()) {
        let mut max = MaxBits::default();
        if let Some((tpi, pt)) = check_tpi(tie_planes(s, t, y0), &mut max) {
            check_side_of_tpi(&q, &tpi, &pt, &mut max);
        }
    }

    /// Lines nearly parallel to planes, anywhere: points from inside the frame to about 2^160 steps
    /// away, integral or not, mirrored or not, commit or report exactly as the oracle says. With
    /// k = 0 the line is parallel to the plane: degenerate, decided exactly.
    #[test]
    fn far_points_match_oracle((pp, k, a, b, mirror) in far_case(), perm in prop::sample::select(PERMUTATIONS.to_vec()),
                               (q, _) in plane()) {
        let planes = far_planes(pp, k, a, b, perm.0, mirror).unwrap();
        let mut max = MaxBits::default();
        if let Some((t, pt)) = check_tpi(planes, &mut max) {
            check_side_of_tpi(&q, &t, &pt, &mut max);
        }
    }
}

/// Inputs for `far_planes` that keep every point inside the frame: P, k, the line's point a and the
/// plane's point b, within a window of random size so the meeting point lands anywhere from inside
/// the frame to far beyond `i128`, and whether to mirror.
fn far_case() -> impl Strategy<Value = (i64, i64, P3, P3, bool)> {
    (
        prop_oneof![1i64..=64, 1i64..=M],
        -3i64..=3,
        0u32..=40,
        any::<bool>(),
    )
        .prop_flat_map(|(pp, k, bits, mirror)| {
            let r = (1i64 << bits).min(M);
            // Each range is [−r, r] cut to leave room for the step added to that coordinate.
            let range = |room: i64| (-r).max(-M)..=r.min(M - room);
            let a = (range(3).prop_map(|x| x.max(-M + 3)), range(pp), range(1));
            let b = (range(pp), range(pp), range(1));
            (Just(pp), Just(k), a, b, Just(mirror))
        })
        .prop_map(|(pp, k, (ax, ay, az), (bx, by, bz), mirror)| {
            (pp, k, p(ax, ay, az), p(bx, by, bz), mirror)
        })
}

/// A plane's coefficients divided by their greatest common divisor, with the normal's first
/// non-zero component positive: the same for every equation of the same point set.
fn canonical(pl: &Plane) -> ([i128; 3], i128) {
    let [x, y, z] = pl.n();
    let d = pl.d();
    let g = x.gcd(&y).gcd(&z).gcd(&d);
    let first = [x, y, z].into_iter().find(|v| *v != 0).unwrap();
    let s = first.signum() * g;
    ([x / s, y / s, z / s], d / s)
}

/// Three-plane points from the 20 distinct planes through corners of the frame's cube (6 faces,
/// 6 diagonal planes, 8 corner cuts) and the same planes tilted by moving one vertex one step in:
/// perpendicular, oblique and nearly parallel extreme planes. Every point matches the oracle, every
/// quantity stays inside its budget, and the largest reach within a few bits of it.
#[test]
fn extreme_tpis() {
    let corners = corners();
    let mut max = MaxBits::default();
    let mut keys = Vec::new();
    let mut planes = Vec::new();
    for (i, &a) in corners.iter().enumerate() {
        for (j, &b) in corners.iter().enumerate().skip(i + 1) {
            for &c in corners.iter().skip(j + 1) {
                let pl = check_plane(a, b, c, &mut max).unwrap();
                let key = canonical(&pl);
                if !keys.contains(&key) {
                    keys.push(key);
                    planes.push(pl);
                    // nudge moves a corner one step towards the origin (outwards would leave the frame).
                    planes.push(check_plane(nudge(a, (i + j) % 3, 1), b, c, &mut max).unwrap());
                }
            }
        }
    }
    assert_eq!(keys.len(), 20);
    let mut tpis = 0;
    let n = planes.len();
    for i in 0..n {
        for j in (i + 1)..n {
            for k in (j + 1)..n {
                if let Some((t, pt)) = check_tpi_with(
                    [planes[i], planes[j], planes[k]],
                    Oracle::Substitute,
                    &mut max,
                ) {
                    tpis += 1;
                    for q in planes.iter().skip((i + j + k) % 8).step_by(8) {
                        check_side_of_tpi(q, &t, &pt, &mut max);
                    }
                }
            }
        }
    }
    // Every quantity inside its budget (checked on each), and the largest within a few bits of it:
    // n 82 of 83, d 122 of 125, w 248 of 252, X 288 of 294, n · X + d · w 372 of 379.
    assert_eq!(tpis, 8396);
    assert_eq!(
        (max.n, max.d, max.w, max.x, max.class),
        (82, 122, 248, 288, 372),
        "{max:?}"
    );
}

#[test]
fn a_plane_from_coefficients_is_checked_as_a_plane_through_points_is() {
    assert_eq!(Plane::new([0, 0, 0], 1), Err(NumError::Degenerate));
    for n in [[1, 0, 0], [0, 1, 0], [0, 0, 1], [-1, 0, 0]] {
        let plane = Plane::new(n, 5).unwrap();
        assert_eq!((plane.n(), plane.d()), (n, 5));
    }
    let top_n = (1i128 << 83) - 1;
    let top_d = (1i128 << 125) - 1;
    assert!(Plane::new([top_n, -top_n, top_n], top_d).is_ok());
    assert!(Plane::new([-top_n, top_n, -top_n], -top_d).is_ok());
    for n in [[top_n + 1, 0, 0], [0, -top_n - 1, 0], [0, 0, top_n + 1]] {
        assert_eq!(Plane::new(n, 0), Err(NumError::BudgetExceeded));
    }
    assert_eq!(
        Plane::new([1, 0, 0], top_d + 1),
        Err(NumError::BudgetExceeded)
    );
    assert_eq!(
        Plane::new([1, 0, 0], -top_d - 1),
        Err(NumError::BudgetExceeded)
    );
}

#[test]
fn a_point_says_the_sign_of_its_weight() {
    let at = |n: [i128; 3]| Plane::new(n, 0).unwrap();
    let (x, y, z) = (at([1, 0, 0]), at([0, 1, 0]), at([0, 0, 1]));
    assert_eq!(Tpi::new(x, y, z).unwrap().weight(), Sign::Positive);
    assert_eq!(Tpi::new(y, x, z).unwrap().weight(), Sign::Negative);
}

/// A coefficient of any length from a bit to past a machine word, so that every step of the word
/// path overflows in some case.
fn any_length() -> impl Strategy<Value = i128> {
    (0u32..70, -3i128..=3).prop_map(|(bits, low)| (1i128 << bits) * low.signum() + low)
}

fn any_length_plane() -> impl Strategy<Value = Plane> {
    (prop::array::uniform3(any_length()), any_length())
        .prop_filter_map("a normal", |(n, d)| Plane::new(n, d).ok())
}

/// A plane of very short coefficients, so that determinants are often zero.
fn short_plane() -> impl Strategy<Value = Plane> {
    (prop::array::uniform3(-2i128..=2), -3i128..=3)
        .prop_filter_map("a normal", |(n, d)| Plane::new(n, d).ok())
}

fn normals_big(p: &Plane, q: &Plane, r: &Plane) -> BigInt {
    det3(&[plane_big(p).0, plane_big(q).0, plane_big(r).0])
}

/// The 4 × 4 determinant, expanded in big integers.
fn determinant_big(p: &Plane, q: &Plane, r: &Plane, s: &Plane) -> BigInt {
    let offset = |plane: &Plane| plane_big(plane).1;
    offset(q) * normals_big(p, r, s)
        - offset(p) * normals_big(q, r, s)
        - offset(r) * normals_big(p, q, s)
        + offset(s) * normals_big(p, q, r)
}

fn check_determinants(p: &Plane, q: &Plane, r: &Plane, s: &Plane) {
    let want = sign_big(&determinant_big(p, q, r, s));
    assert_eq!(determinant_sign(p, q, r, s).unwrap(), want);
    let weight = sign_big(&normals_big(p, q, r));
    assert_eq!(normals_sign(p, q, r).unwrap(), weight);
    let (full, quick) = (Tpi::new(*p, *q, *r), Tpi::meeting(*p, *q, *r));
    assert_eq!(full, quick);
    if weight == Sign::Zero {
        assert_eq!(quick, Err(NumError::Degenerate));
    } else {
        assert_eq!(quick.unwrap().weight(), weight);
        assert_eq!(s.side_of_tpi(&quick.unwrap()).unwrap(), want.mul(weight));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn determinants_are_exact_at_any_length(
        p in any_length_plane(),
        q in any_length_plane(),
        r in any_length_plane(),
        s in any_length_plane(),
    ) {
        check_determinants(&p, &q, &r, &s);
    }

    #[test]
    fn determinants_are_exact_when_they_vanish(
        p in short_plane(),
        q in short_plane(),
        r in short_plane(),
        s in short_plane(),
    ) {
        check_determinants(&p, &q, &r, &s);
    }

    /// One step from every degeneracy (chapter 28.6): a fourth plane that is a combination of the
    /// first three makes the determinant exactly zero, and a third normal that is a combination
    /// of the first two makes the weight zero; moving any one coefficient by one either way then
    /// gives ± a minor, whose sign must be exact.
    #[test]
    fn determinants_one_step_from_zero(
        p in any_length_plane(),
        q in any_length_plane(),
        r in any_length_plane(),
        (a, b, c) in (-3i128..=3, -3i128..=3, -3i128..=3),
        coefficient in 0usize..4,
    ) {
        if let Some(s) = mix(&[(a, &p), (b, &q), (c, &r)]) {
            prop_assert_eq!(determinant_sign(&p, &q, &r, &s).unwrap(), Sign::Zero);
            for k in [-1, 1] {
                if let Some(moved) = nudged(&s, coefficient, k) {
                    check_determinants(&p, &q, &r, &moved);
                }
            }
        }
        if let Some(dependent) = mix(&[(a, &p), (b, &q)]) {
            prop_assert_eq!(normals_sign(&p, &q, &dependent).unwrap(), Sign::Zero);
            for k in [-1, 1] {
                if let Some(moved) = nudged(&dependent, coefficient, k) {
                    check_determinants(&p, &q, &moved, &r);
                }
            }
        }
    }

    /// The laws of a determinant: swapping any two planes flips the sign, negating a plane's
    /// coefficients flips it, and a repeated plane makes it zero. The same for three normals,
    /// whose sign is the weight of the point where the planes meet.
    #[test]
    fn determinant_laws(
        p in any_length_plane(),
        q in any_length_plane(),
        r in any_length_plane(),
        s in any_length_plane(),
    ) {
        let rows = [p, q, r, s];
        let det = determinant_sign(&p, &q, &r, &s).unwrap();
        for i in 0..4 {
            for j in i + 1..4 {
                let mut swapped = rows;
                swapped.swap(i, j);
                let [w, x, y, z] = swapped;
                prop_assert_eq!(determinant_sign(&w, &x, &y, &z).unwrap(), det.negate());
            }
            let mut negated = rows;
            negated[i] = Plane::new(rows[i].n().map(|v| -v), -rows[i].d()).unwrap();
            let [w, x, y, z] = negated;
            prop_assert_eq!(determinant_sign(&w, &x, &y, &z).unwrap(), det.negate());
            let mut repeated = rows;
            repeated[(i + 1) % 4] = rows[i];
            let [w, x, y, z] = repeated;
            prop_assert_eq!(determinant_sign(&w, &x, &y, &z).unwrap(), Sign::Zero);
        }
        let weight = normals_sign(&p, &q, &r).unwrap();
        prop_assert_eq!(normals_sign(&q, &p, &r).unwrap(), weight.negate());
        prop_assert_eq!(normals_sign(&p, &r, &q).unwrap(), weight.negate());
        prop_assert_eq!(normals_sign(&r, &p, &q).unwrap(), weight);
        let minus = Plane::new(p.n().map(|v| -v), p.d()).unwrap();
        prop_assert_eq!(normals_sign(&minus, &q, &r).unwrap(), weight.negate());
        prop_assert_eq!(normals_sign(&p, &q, &p).unwrap(), Sign::Zero);
    }
}

/// `terms` summed coefficient by coefficient, as a plane, if every step fits and the plane is
/// within the budget.
fn mix(terms: &[(i128, &Plane)]) -> Option<Plane> {
    let mut n = [0i128; 3];
    let mut d = 0i128;
    for &(k, plane) in terms {
        for (sum, v) in n.iter_mut().zip(plane.n()) {
            *sum = sum.checked_add(k.checked_mul(v)?)?;
        }
        d = d.checked_add(k.checked_mul(plane.d())?)?;
    }
    Plane::new(n, d).ok()
}

/// `plane` with one coefficient moved by `k`: 0 to 2 a component of the normal, 3 the offset.
fn nudged(plane: &Plane, coefficient: usize, k: i128) -> Option<Plane> {
    let (mut n, mut d) = (plane.n(), plane.d());
    match n.get_mut(coefficient) {
        Some(v) => *v = v.checked_add(k)?,
        None => d = d.checked_add(k)?,
    }
    Plane::new(n, d).ok()
}

/// Every sign corner of the budget (chapter 28.6): each normal component at ±(2^83 − 1) and each
/// offset at ±(2^125 − 1), the largest coefficients `Plane::new` accepts, so these are the
/// largest determinants any planes have. Every one of the 16^4 determinants and 16^3 weights
/// against the big-integer oracle, and none past the exact path's budget, which
/// `determinant_sign` and `normals_sign` document no planes reach. Weights also with components
/// at zero, the centres of the frame.
#[test]
fn determinants_at_the_coefficient_corners() {
    let (n, d) = ((1i128 << 83) - 1, (1i128 << 125) - 1);
    let at = |bits: u32, i: u32, v: i128| if bits >> i & 1 == 1 { -v } else { v };
    let corners: Vec<Plane> = (0..16u32)
        .map(|bits| {
            Plane::new(
                [at(bits, 0, n), at(bits, 1, n), at(bits, 2, n)],
                at(bits, 3, d),
            )
            .unwrap()
        })
        .collect();
    let mut seen = [0usize; 3];
    let tally = |seen: &mut [usize; 3], s: Sign| {
        seen[match s {
            Sign::Negative => 0,
            Sign::Zero => 1,
            Sign::Positive => 2,
        }] += 1;
    };
    for p in &corners {
        for q in &corners {
            for r in &corners {
                assert_eq!(
                    normals_sign(p, q, r).unwrap(),
                    sign_big(&normals_big(p, q, r))
                );
                for s in &corners {
                    let got = determinant_sign(p, q, r, s).unwrap();
                    assert_eq!(got, sign_big(&determinant_big(p, q, r, s)));
                    tally(&mut seen, got);
                }
            }
        }
    }
    assert!(
        seen.iter().all(|&k| k > 0),
        "zero, negative and positive all reached: {seen:?}"
    );
    let values = [-n, 0, n];
    let centres: Vec<Plane> = (0..27usize)
        .filter_map(|i| Plane::new([values[i % 3], values[i / 3 % 3], values[i / 9]], d).ok())
        .collect();
    assert_eq!(centres.len(), 26, "every normal but zero");
    let mut weights = [0usize; 3];
    for p in &centres {
        for q in &centres {
            for r in &centres {
                let got = normals_sign(p, q, r).unwrap();
                assert_eq!(got, sign_big(&normals_big(p, q, r)));
                tally(&mut weights, got);
            }
        }
    }
    assert!(
        weights.iter().all(|&k| k > 0),
        "zero, negative and positive all reached: {weights:?}"
    );
}
