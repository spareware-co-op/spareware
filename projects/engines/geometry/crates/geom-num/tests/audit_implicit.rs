//! Independent audit of the implicit points (`geom_num::crossing::Ssi`, `geom_num::plane::Lpi`
//! and `geom_num::plane::Tpi`) where the generated weight filter cannot decide.
//!
//! The quality ledger lists the exact weight paths (`ssi_weight::exact`, `lpi_weight::exact`
//! behind their filters) as never run by a test: every tested crossing has a weight the filter
//! certifies. A weight the filter cannot certify is a weight that is tiny beside the product of
//! the coordinate differences, which is two nearly parallel lines, or a line nearly parallel to
//! its plane. Those are built here with Bézout's identity: for a direction u = (ux, uy) the
//! extended Euclidean algorithm gives v with ux·vy − uy·vx = gcd(ux, uy), so the two directions
//! are as close to parallel as grid vectors of that size can be, and the weight is the gcd (1 for
//! most pairs) where the filter's bound is about 2^31 (crossings) or 2^73 (line and plane).
//!
//! Oracle: `num-rational`. The crossing point is solved from the lines' own equations (a parameter
//! along the first line), never from the homogeneous coordinates the kernel uses, and each side is
//! the sign of the third line's or plane's equation at that rational point. Committing to the
//! grid is rounding each rational coordinate half to even.
//!
//! Seeds: each test prints `seed: <hex>`; `SPAREKIT_SEED=<hex>` replays another.

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
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines,
    reason = "test code: a broken oracle must stop the test loudly"
)]

use geom_num::crossing::Ssi;
use geom_num::generated::{lpi_weight, ssi_weight};
use geom_num::grid::{COORD_MAX, P2, P3};
use geom_num::plane::{Lpi, Plane, Tpi};
use geom_num::{NumError, Sign};
use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};

const M: i64 = COORD_MAX;
const DEFAULT_SEED: u64 = 0x6ad1_7000_5eed_0001;

/// `SplitMix64`, written out as the crate's other tests do.
struct Rng(u64);

impl Rng {
    fn for_test(name: &str) -> Rng {
        let seed = std::env::var("SPAREKIT_SEED")
            .ok()
            .and_then(|s| u64::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok())
            .unwrap_or(DEFAULT_SEED);
        println!("{name}: seed: {seed:016x}");
        let mut h = 0xcbf2_9ce4_8422_2325_u64;
        for b in name.bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
        Rng(seed ^ h)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        let span = (i128::from(hi) - i128::from(lo) + 1) as u128;
        (i128::from(lo) + (u128::from(self.next()) % span) as i128) as i64
    }

    /// A magnitude at a random scale up to `reach`, never zero.
    fn size(&mut self, reach: i64) -> i64 {
        let bits = self.range(1, 63 - i64::from(reach.leading_zeros()));
        self.range(1, ((1i64 << bits) - 1).min(reach))
    }

    fn sign(&mut self) -> i64 {
        if self.next() & 1 == 0 { 1 } else { -1 }
    }
}

fn cases(release: usize) -> usize {
    if cfg!(debug_assertions) {
        release / 20
    } else {
        release
    }
}

fn big(v: i64) -> BigInt {
    BigInt::from(v)
}

fn rat(v: &BigInt) -> BigRational {
    BigRational::from_integer(v.clone())
}

fn sign_of(v: &BigRational) -> Sign {
    if v.is_negative() {
        Sign::Negative
    } else if v.is_zero() {
        Sign::Zero
    } else {
        Sign::Positive
    }
}

/// x, y with a·x + b·y = gcd(a, b), for positive a and b: the extended Euclidean algorithm.
fn bezout(a: i64, b: i64) -> (i64, i64, i64) {
    let (mut r0, mut r1) = (i128::from(a), i128::from(b));
    let (mut s0, mut s1, mut t0, mut t1) = (1i128, 0i128, 0i128, 1i128);
    while r1 != 0 {
        let q = r0 / r1;
        (r0, r1) = (r1, r0 - q * r1);
        (s0, s1) = (s1, s0 - q * s1);
        (t0, t1) = (t1, t0 - q * t1);
    }
    (r0 as i64, s0 as i64, t0 as i64)
}

/// A rational rounded to the nearest integer, ties to even.
fn round_half_even(x: &BigRational) -> BigInt {
    let two = BigInt::from(2);
    let floor = x.floor().to_integer();
    let twice_rest = (x - rat(&floor)) * rat(&two);
    match twice_rest.cmp(&BigRational::one()) {
        std::cmp::Ordering::Less => floor,
        std::cmp::Ordering::Greater => floor + 1,
        std::cmp::Ordering::Equal => {
            if floor.is_even() {
                floor
            } else {
                floor + 1
            }
        }
    }
}

/// What committing the rational point must return: the rounded coordinates and whether all were
/// whole, or `OutOfRange` with the first coordinate that leaves the frame, saturated to `i128`.
fn commit<const N: usize>(x: &[BigRational; N]) -> Result<([i64; N], bool), NumError> {
    let mut out = [0i64; N];
    let mut exact = true;
    for (o, c) in out.iter_mut().zip(x) {
        let r = round_half_even(c);
        if r.abs() > big(M) {
            let value = r.to_i128().unwrap_or(if r.is_negative() {
                i128::MIN
            } else {
                i128::MAX
            });
            return Err(NumError::OutOfRange { value });
        }
        *o = r.to_i64().unwrap();
        exact &= c.is_integer();
    }
    Ok((out, exact))
}

type Pt2 = [i64; 2];
type Pt3 = [i64; 3];

fn p2([x, y]: Pt2) -> P2 {
    P2::new(x, y).unwrap()
}

fn p3([x, y, z]: Pt3) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn on_grid(c: &[i64]) -> bool {
    c.iter().all(|v| (-M..=M).contains(v))
}

/// (q − p) × (x − p) for a rational point x.
fn orient2d_at(p: Pt2, q: Pt2, x: &[BigRational; 2]) -> BigRational {
    let (qx, qy) = (rat(&(big(q[0]) - big(p[0]))), rat(&(big(q[1]) - big(p[1]))));
    let (xx, xy) = (&x[0] - rat(&big(p[0])), &x[1] - rat(&big(p[1])));
    qx * xy - qy * xx
}

/// The point where the line a → b crosses the line c → d: a + t (b − a) with
/// t = ((c − a) × (d − c)) / ((b − a) × (d − c)), from the second line's equation.
fn crossing_point(a: Pt2, b: Pt2, c: Pt2, d: Pt2) -> Option<[BigRational; 2]> {
    let sub = |p: Pt2, q: Pt2| [big(p[0]) - big(q[0]), big(p[1]) - big(q[1])];
    let cross = |u: &[BigInt; 2], v: &[BigInt; 2]| &u[0] * &v[1] - &u[1] * &v[0];
    let (u, v, ca) = (sub(b, a), sub(d, c), sub(c, a));
    let den = cross(&u, &v);
    if den.is_zero() {
        return None;
    }
    let t = BigRational::new(cross(&ca, &v), den);
    Some([
        rat(&big(a[0])) + &t * rat(&u[0]),
        rat(&big(a[1])) + &t * rat(&u[1]),
    ])
}

/// Two nearly parallel lines: directions u and v with u × v = ±m·gcd(u), a small number.
fn nearly_parallel_lines(rng: &mut Rng) -> (Pt2, Pt2, Pt2, Pt2) {
    loop {
        let (ux, uy) = (rng.size(M / 2), rng.size(M / 2));
        let (_, s, t) = bezout(ux, uy);
        // ux·s + uy·t = g, so with v = (−t, s): ux·vy − uy·vx = g.
        let (mut vx, mut vy) = (-t, s);
        // The same cross product after adding a multiple of u, and a small multiple of it.
        let k = rng.range(-2, 2);
        let m = rng.range(1, 3);
        vx = m * vx + k * ux;
        vy = m * vy + k * uy;
        let (su, sv) = (rng.sign(), rng.sign());
        let (u, v) = ([su * ux, su * uy], [sv * vx, sv * vy]);
        let a = [rng.range(-M / 2, M / 2), rng.range(-M / 2, M / 2)];
        let c = [rng.range(-M / 2, M / 2), rng.range(-M / 2, M / 2)];
        let b = [a[0] + u[0], a[1] + u[1]];
        let d = [c[0] + v[0], c[1] + v[1]];
        if on_grid(&b) && on_grid(&d) {
            return (a, b, c, d);
        }
    }
}

/// Nearly parallel lines: the weight's filter cannot certify its sign, so the exact weight path
/// decides. Every side and every commit must still be the rational point's.
#[test]
fn crossings_of_nearly_parallel_lines_are_exact() {
    let mut rng = Rng::for_test("crossings_of_nearly_parallel_lines_are_exact");
    let (mut undecided, mut sides, mut zeros) = (0usize, 0usize, 0usize);
    for case in 0..cases(40_000) {
        let (a, b, c, d) = nearly_parallel_lines(&mut rng);
        let x = crossing_point(a, b, c, d).expect("the lines are not parallel");
        let s = Ssi::new(p2(a), p2(b), p2(c), p2(d)).expect("a crossing");
        if ssi_weight::filter(p2(a), p2(b), p2(c), p2(d)).is_none() {
            undecided += 1;
        }
        // The homogeneous coordinates are the rational point.
        let ([hx, hy], w) = s.homogeneous().unwrap();
        assert!(w != 0);
        assert_eq!(
            BigRational::new(hx.into(), w.into()),
            x[0],
            "case {case}: x"
        );
        assert_eq!(
            BigRational::new(hy.into(), w.into()),
            x[1],
            "case {case}: y"
        );
        // The commit.
        let want = commit(&x);
        let got = s.to_grid().map(|r| {
            let p = r.value();
            ([p.x.get(), p.y.get()], r.is_exact())
        });
        assert_eq!(got, want, "case {case}: to_grid of {a:?} {b:?} {c:?} {d:?}");
        // Sides: against the two lines themselves (exactly zero), against random lines, and
        // against lines through the rounded crossing when it is in the frame.
        let mut queries = vec![(a, b), (b, a), (c, d), (d, c)];
        for _ in 0..4 {
            let p = [rng.range(-M, M), rng.range(-M, M)];
            let q = [rng.range(-M, M), rng.range(-M, M)];
            queries.push((p, q));
        }
        queries.push((a, c));
        queries.push((b, d));
        if let Ok((near, _)) = want {
            let q = [rng.range(-M, M), rng.range(-M, M)];
            queries.push((near, q));
            queries.push((q, near));
        }
        for (p, q) in queries {
            let want = sign_of(&orient2d_at(p, q, &x));
            let got = s.orient2d(p2(p), p2(q)).unwrap();
            assert_eq!(
                got, want,
                "case {case}: side of {p:?} → {q:?} for the crossing of {a:?} → {b:?} and {c:?} → {d:?}"
            );
            sides += 1;
            zeros += usize::from(want == Sign::Zero);
        }
    }
    println!("weights the filter left undecided: {undecided}; sides {sides}, zero {zeros}");
    assert!(undecided > 0, "no case reached the exact weight path");
}

/// n = (b − a) × (c − a) and d = −n · a, in big integers.
fn plane_of(a: Pt3, b: Pt3, c: Pt3) -> ([BigInt; 3], BigInt) {
    let u: [BigInt; 3] = std::array::from_fn(|i| big(b[i]) - big(a[i]));
    let v: [BigInt; 3] = std::array::from_fn(|i| big(c[i]) - big(a[i]));
    let n = [
        &u[1] * &v[2] - &u[2] * &v[1],
        &u[2] * &v[0] - &u[0] * &v[2],
        &u[0] * &v[1] - &u[1] * &v[0],
    ];
    let d = -(&n[0] * big(a[0]) + &n[1] * big(a[1]) + &n[2] * big(a[2]));
    (n, d)
}

/// n · x + d at a rational point.
fn plane_at((n, d): &([BigInt; 3], BigInt), x: &[BigRational; 3]) -> BigRational {
    rat(d) + rat(&n[0]) * &x[0] + rat(&n[1]) * &x[1] + rat(&n[2]) * &x[2]
}

/// The kernel's plane through three points, checked to have the oracle's coefficients.
fn plane(a: Pt3, b: Pt3, c: Pt3) -> Option<(Plane, ([BigInt; 3], BigInt))> {
    let oracle = plane_of(a, b, c);
    match Plane::through(p3(a), p3(b), p3(c)) {
        Ok(pl) => {
            let n = pl.n().map(BigInt::from);
            assert_eq!(
                (n, BigInt::from(pl.d())),
                oracle,
                "plane through {a:?} {b:?} {c:?}"
            );
            Some((pl, oracle))
        }
        Err(NumError::Degenerate) => {
            assert!(oracle.0.iter().all(Zero::is_zero));
            None
        }
        Err(e) => panic!("plane through {a:?} {b:?} {c:?}: {e:?}"),
    }
}

/// The point where the line p → q meets the plane: p + t (q − p) with
/// t = −(n · p + d) / (n · (q − p)).
fn line_plane_point(p: Pt3, q: Pt3, pl: &([BigInt; 3], BigInt)) -> Option<[BigRational; 3]> {
    let u: [BigInt; 3] = std::array::from_fn(|i| big(q[i]) - big(p[i]));
    let den = &pl.0[0] * &u[0] + &pl.0[1] * &u[1] + &pl.0[2] * &u[2];
    if den.is_zero() {
        return None;
    }
    let at_p = &pl.0[0] * big(p[0]) + &pl.0[1] * big(p[1]) + &pl.0[2] * big(p[2]) + &pl.1;
    let t = BigRational::new(-at_p, den);
    Some(std::array::from_fn(|i| rat(&big(p[i])) + &t * rat(&u[i])))
}

/// A direction u, and a plane's two edge vectors e1 = u + s·(0, 0, 1) and e2 = v with
/// ux·vy − uy·vx small: the plane's normal times u is ∓s·m·gcd(ux, uy), so a line along u is as
/// nearly parallel to the plane as grid vectors allow.
fn nearly_parallel_line_and_plane(rng: &mut Rng) -> (Pt3, Pt3, Pt3, Pt3, Pt3) {
    loop {
        let (ux, uy) = (rng.size(M / 2), rng.size(M / 2));
        let (_, s, t) = bezout(ux, uy);
        let k = rng.range(-1, 1);
        let m = rng.range(1, 2);
        let (vx, vy) = (m * -t + k * ux, m * s + k * uy);
        let (su, sv) = (rng.sign(), rng.sign());
        let u = [su * ux, su * uy, rng.sign() * rng.size(M / 2)];
        let v = [sv * vx, sv * vy, rng.sign() * rng.size(M / 2)];
        let lift = rng.sign() * rng.range(1, 2);
        let reach = M / 2 - 2;
        let a: Pt3 = std::array::from_fn(|_| rng.range(-reach, reach));
        let p: Pt3 = std::array::from_fn(|_| rng.range(-reach, reach));
        let b = [a[0] + u[0], a[1] + u[1], a[2] + u[2] + lift];
        let c = [a[0] + v[0], a[1] + v[1], a[2] + v[2]];
        let q = [p[0] + u[0], p[1] + u[1], p[2] + u[2]];
        if on_grid(&b) && on_grid(&c) && on_grid(&q) {
            return (a, b, c, p, q);
        }
    }
}

fn commit3(x: &[BigRational; 3]) -> Result<([i64; 3], bool), NumError> {
    commit(x)
}

/// A line nearly parallel to its plane: the weight's filter cannot certify its sign, so the exact
/// weight path decides. Every side and every commit must still be the rational point's.
#[test]
fn lines_nearly_parallel_to_their_plane_are_exact() {
    let mut rng = Rng::for_test("lines_nearly_parallel_to_their_plane_are_exact");
    let (mut undecided, mut sides, mut zeros) = (0usize, 0usize, 0usize);
    for case in 0..cases(20_000) {
        let (a, b, c, p, q) = nearly_parallel_line_and_plane(&mut rng);
        let Some((pl, oracle)) = plane(a, b, c) else {
            continue;
        };
        let (p, q) = if rng.next() & 1 == 0 { (p, q) } else { (q, p) };
        let Some(x) = line_plane_point(p, q, &oracle) else {
            assert_eq!(Lpi::new(p3(p), p3(q), pl), Err(NumError::Degenerate));
            continue;
        };
        let l = Lpi::new(p3(p), p3(q), pl).expect("a line that meets its plane");
        let [nx, ny, nz] = pl.n();
        if lpi_weight::filter(nx, ny, nz, p3(p), p3(q)).is_none() {
            undecided += 1;
        }
        let want = commit3(&x);
        let got = l.to_grid().map(|r| {
            let v = r.value();
            ([v.x.get(), v.y.get(), v.z.get()], r.is_exact())
        });
        assert_eq!(
            got, want,
            "case {case}: to_grid of {p:?} → {q:?} on {a:?} {b:?} {c:?}"
        );
        // Sides: the plane itself and planes through the line (exactly zero), random planes, and
        // planes through the rounded point when it is in the frame.
        let mut queries: Vec<[Pt3; 3]> = vec![[a, b, c], [a, c, b]];
        for _ in 0..2 {
            let r: Pt3 = std::array::from_fn(|_| rng.range(-M, M));
            queries.push([p, q, r]);
            queries.push([q, r, p]);
        }
        for _ in 0..4 {
            queries.push(std::array::from_fn(|_| {
                std::array::from_fn(|_| rng.range(-M, M))
            }));
        }
        if let Ok((near, _)) = want {
            let r: Pt3 = std::array::from_fn(|_| rng.range(-M, M));
            let t: Pt3 = std::array::from_fn(|_| rng.range(-M, M));
            queries.push([near, r, t]);
        }
        for [qa, qb, qc] in queries {
            let Some((side_plane, side_oracle)) = plane(qa, qb, qc) else {
                continue;
            };
            let want = sign_of(&plane_at(&side_oracle, &x));
            let got = side_plane.side_of_lpi(&l).unwrap();
            assert_eq!(
                got, want,
                "case {case}: side of the plane {qa:?} {qb:?} {qc:?} for the line {p:?} → {q:?} on the plane {a:?} {b:?} {c:?}"
            );
            sides += 1;
            zeros += usize::from(want == Sign::Zero);
        }
    }
    println!("weights the filter left undecided: {undecided}; sides {sides}, zero {zeros}");
    assert!(undecided > 0, "no case reached the exact weight path");
}

/// The point where three planes meet, by Gaussian elimination on rationals (the kernel uses
/// Cramer's rule in homogeneous integers).
fn three_plane_point(planes: [&([BigInt; 3], BigInt); 3]) -> Option<[BigRational; 3]> {
    let mut m: Vec<Vec<BigRational>> = planes
        .iter()
        .map(|(n, d)| vec![rat(&n[0]), rat(&n[1]), rat(&n[2]), -rat(d)])
        .collect();
    for col in 0..3 {
        let pivot = (col..3).find(|&r| !m[r][col].is_zero())?;
        m.swap(col, pivot);
        let lead = m[col][col].clone();
        for v in &mut m[col] {
            *v = &*v / &lead;
        }
        for r in 0..3 {
            if r != col {
                let factor = m[r][col].clone();
                for k in 0..4 {
                    let delta = &factor * &m[col][k];
                    m[r][k] -= delta;
                }
            }
        }
    }
    Some([m[0][3].clone(), m[1][3].clone(), m[2][3].clone()])
}

/// Three planes that nearly share a line: two hold the direction u, and the third is as nearly
/// parallel to u as grid vectors allow. The point is far from the frame or just inside it; every
/// side and every commit must be the rational point's.
#[test]
fn three_planes_that_nearly_share_a_line_are_exact() {
    let mut rng = Rng::for_test("three_planes_that_nearly_share_a_line_are_exact");
    let (mut made, mut sides, mut zeros, mut inside) = (0usize, 0usize, 0usize, 0usize);
    for case in 0..cases(20_000) {
        let (a, b, c, p, q) = nearly_parallel_line_and_plane(&mut rng);
        let u = [q[0] - p[0], q[1] - p[1], q[2] - p[2]];
        // Two planes through the line p → q, or through lines parallel to it.
        let shift = |rng: &mut Rng| -> Pt3 {
            if rng.next() & 3 == 0 {
                std::array::from_fn(|_| rng.range(-3, 3))
            } else {
                [0, 0, 0]
            }
        };
        let (s1, s2) = (shift(&mut rng), shift(&mut rng));
        let p1 = [p[0] + s1[0], p[1] + s1[1], p[2] + s1[2]];
        let p2_ = [p[0] + s2[0], p[1] + s2[1], p[2] + s2[2]];
        let q1 = [p1[0] + u[0], p1[1] + u[1], p1[2] + u[2]];
        let q2 = [p2_[0] + u[0], p2_[1] + u[1], p2_[2] + u[2]];
        let r1: Pt3 = std::array::from_fn(|_| rng.range(-M, M));
        let r2: Pt3 = std::array::from_fn(|_| rng.range(-M, M));
        if !(on_grid(&p1) && on_grid(&q1) && on_grid(&p2_) && on_grid(&q2)) {
            continue;
        }
        let (Some((pa, oa)), Some((pb, ob)), Some((pc, oc))) =
            (plane(p1, q1, r1), plane(p2_, q2, r2), plane(a, b, c))
        else {
            continue;
        };
        let mut order = [(pa, &oa), (pb, &ob), (pc, &oc)];
        let rot = (rng.next() % 3) as usize;
        order.rotate_left(rot);
        if rng.next() & 1 == 0 {
            order.swap(0, 1);
        }
        let Some(x) = three_plane_point([order[0].1, order[1].1, order[2].1]) else {
            assert_eq!(
                Tpi::new(order[0].0, order[1].0, order[2].0),
                Err(NumError::Degenerate),
                "case {case}"
            );
            continue;
        };
        let t = Tpi::new(order[0].0, order[1].0, order[2].0).expect("three planes that meet");
        made += 1;
        let want = commit3(&x);
        inside += usize::from(want.is_ok());
        let got = t.to_grid().map(|r| {
            let v = r.value();
            ([v.x.get(), v.y.get(), v.z.get()], r.is_exact())
        });
        assert_eq!(got, want, "case {case}: to_grid");
        let mut queries: Vec<[Pt3; 3]> = vec![[p1, q1, r1], [p2_, q2, r2], [a, b, c], [a, c, b]];
        for _ in 0..4 {
            queries.push(std::array::from_fn(|_| {
                std::array::from_fn(|_| rng.range(-M, M))
            }));
        }
        if let Ok((near, _)) = want {
            let r: Pt3 = std::array::from_fn(|_| rng.range(-M, M));
            let s: Pt3 = std::array::from_fn(|_| rng.range(-M, M));
            queries.push([near, r, s]);
        }
        for [qa, qb, qc] in queries {
            let Some((side_plane, side_oracle)) = plane(qa, qb, qc) else {
                continue;
            };
            let want = sign_of(&plane_at(&side_oracle, &x));
            let got = side_plane.side_of_tpi(&t).unwrap();
            assert_eq!(
                got, want,
                "case {case}: side of the plane {qa:?} {qb:?} {qc:?}"
            );
            sides += 1;
            zeros += usize::from(want == Sign::Zero);
        }
    }
    println!("points made: {made} ({inside} commit inside the frame); sides {sides}, zero {zeros}");
    assert!(made > 0);
}
