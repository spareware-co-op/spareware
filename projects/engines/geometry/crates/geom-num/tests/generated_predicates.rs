//! The generated predicates (`geom_num::generated`, written by geom-gen from each predicate's
//! description) against exact arithmetic.
//!
//! - **The filter never gives a wrong sign.** On millions of random point sets across the whole
//!   grid and at every scale, and on near-degenerate constructions at the grid's extremes
//!   (collinear, coplanar, cocircular and cospherical points, exactly and nudged by one step),
//!   every sign the filter gives is the exact path's sign. Both outcomes occur on the
//!   near-degenerate sets, so the tests reach the filter's boundary from both sides.
//! - **The exact path is exact.** At every sign combination of ±(2^40 − 1) (64 for orient2d,
//!   4,096 for orient3d, 256 for incircle, 32,768 for insphere) and on random and near-degenerate
//!   sets, the exact path's value equals a `num-bigint` determinant of the homogeneous matrix
//!   (cofactor expansion, which shares no step with the generated code), and is never `None`.
//! - **The public predicates route through them** and agree with the exact path. Agreement
//!   alone cannot show the route, since every path gives the same sign, so the route is read from
//!   `predicate.rs`'s code: each public predicate calls its generated filter first (orient2d only
//!   where `ORIENT2D_FILTER_FIRST` holds, as its documentation's measured routing says), and its
//!   exact steps call the generated exact path before the hand-written one.
//! - **A zero scale answers zero at once**: points on one axis line or axis plane.
//!
//! The filters at their bounds (chapter 28.6) and the generated predicate with rounded inputs are
//! tested in `tests/generated_bounds.rs`.
//!
//! Seeds: each randomised test prints `seed: <hex>` and runs from a fixed default;
//! `SPAREKIT_SEED=<hex>` replays another (chapter 28.17). Case counts are in the millions in
//! release builds and a fiftieth of that in debug builds.

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
    reason = "points are a to e as in the predicates"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use geom_num::generated::{incircle, insphere, orient2d, orient3d};
use geom_num::grid::{COORD_MAX, P2, P3};
use geom_num::{Sign, Wide, predicate};
use num_bigint::BigInt;

const M: i64 = COORD_MAX;

// ---------------------------------------------------------------------------------------------
// Seeds and the generator.
// ---------------------------------------------------------------------------------------------

/// The default seed of every randomised test here.
const DEFAULT_SEED: u64 = 0x6727_0000_5eed_0186;

/// `SplitMix64` (Steele, Lea and Flood, *Fast splittable pseudorandom number generators*,
/// OOPSLA 2014): the generator `geom-test` also uses, written here because `geom-num` cannot
/// depend on `geom-test`.
struct Rng(u64);

impl Rng {
    /// The test's generator: the seed from `SPAREKIT_SEED` (hex) or the default, printed.
    fn for_test(name: &str) -> Rng {
        let seed = std::env::var("SPAREKIT_SEED")
            .ok()
            .and_then(|s| u64::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok())
            .unwrap_or(DEFAULT_SEED);
        println!("{name}: seed: {seed:016x}");
        Rng(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in lo..=hi.
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        let span = (hi - lo) as u64 + 1;
        lo + (self.next() % span) as i64
    }

    /// A coordinate: anywhere on the grid, at one of its edges, or within 2^k of zero for a
    /// random k, so every scale is reached.
    fn coord(&mut self) -> i64 {
        match self.next() % 8 {
            0 => M,
            1 => -M,
            2 => M - self.range(0, 3),
            3 => -M + self.range(0, 3),
            4 | 5 => {
                let k = self.range(0, 40);
                self.range(-(1 << k) + 1, (1 << k) - 1).clamp(-M, M)
            }
            _ => self.range(-M, M),
        }
    }
}

/// Release builds run every count in full; debug builds a fiftieth.
fn cases(release: usize) -> usize {
    if cfg!(debug_assertions) {
        release / 50
    } else {
        release
    }
}

fn p2([x, y]: [i64; 2]) -> P2 {
    P2::new(x, y).unwrap()
}

fn p3([x, y, z]: [i64; 3]) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn in_range(v: i64) -> bool {
    (-M..=M).contains(&v)
}

// ---------------------------------------------------------------------------------------------
// The oracle: homogeneous determinants by cofactor expansion on BigInt.
// ---------------------------------------------------------------------------------------------

fn laplace(m: &[Vec<BigInt>]) -> BigInt {
    let Some((first, rest)) = m.split_first() else {
        return BigInt::from(1);
    };
    let mut det = BigInt::from(0);
    for (j, x) in first.iter().enumerate() {
        if *x == BigInt::from(0) {
            continue;
        }
        let minor: Vec<Vec<BigInt>> = rest
            .iter()
            .map(|row| {
                let mut r = row.clone();
                r.remove(j);
                r
            })
            .collect();
        let term = x * laplace(&minor);
        det += if j % 2 == 0 { term } else { -term };
    }
    det
}

/// det[p 1] for the points (with the lift column |p|² if `lift`), negated if `negate`: orient2d
/// is det[a 1; b 1; c 1], orient3d −det[a 1; …; d 1], incircle det[a |a|² 1; …] and insphere
/// −det[a |a|² 1; …], the conventions of `geom_num::predicate`.
fn oracle(points: &[&[i64]], lift: bool, negate: bool) -> BigInt {
    let rows: Vec<Vec<BigInt>> = points
        .iter()
        .map(|p| {
            let mut row: Vec<BigInt> = p.iter().map(|&v| BigInt::from(v)).collect();
            if lift {
                row.push(p.iter().map(|&v| BigInt::from(v) * v).sum::<BigInt>());
            }
            row.push(BigInt::from(1));
            row
        })
        .collect();
    let det = laplace(&rows);
    if negate { -det } else { det }
}

fn big<const L: usize>(w: &Wide<L>) -> BigInt {
    let bytes: Vec<u8> = w
        .to_le_limbs()
        .iter()
        .flat_map(|l| l.to_le_bytes())
        .collect();
    BigInt::from_signed_bytes_le(&bytes)
}

// ---------------------------------------------------------------------------------------------
// One check per predicate: the filter against the exact path, and optionally the oracle.
// ---------------------------------------------------------------------------------------------

/// How often the filter answered and how often it left the call to the exact path.
#[derive(Default, Debug)]
struct Tally {
    certified: u64,
    zero_scale: u64,
    left: u64,
}

impl Tally {
    fn count(&mut self, filtered: Option<Sign>) {
        match filtered {
            Some(Sign::Zero) => self.zero_scale += 1,
            Some(_) => self.certified += 1,
            None => self.left += 1,
        }
    }
}

fn check_orient2d(p: [[i64; 2]; 3], with_oracle: bool, tally: &mut Tally) {
    let [a, b, c] = p.map(p2);
    let value = orient2d::value(a, b, c).expect("the certificate rules out None");
    let exact = Sign::of_i128(value);
    let filtered = orient2d::filter(a, b, c);
    if let Some(s) = filtered {
        assert_eq!(s, exact, "orient2d filter {p:?}");
    }
    assert_eq!(orient2d::exact(a, b, c), Some(exact));
    if with_oracle {
        let o = oracle(&[&p[0], &p[1], &p[2]], false, false);
        assert_eq!(BigInt::from(value), o, "orient2d value {p:?}");
        assert_eq!(
            predicate::orient2d(a, b, c),
            Ok(exact),
            "orient2d public {p:?}"
        );
    }
    tally.count(filtered);
}

fn check_orient3d(p: [[i64; 3]; 4], with_oracle: bool, tally: &mut Tally) {
    let [a, b, c, d] = p.map(p3);
    let value = orient3d::value(a, b, c, d).expect("the certificate rules out None");
    let exact = Sign::of_i128(value);
    let filtered = orient3d::filter(a, b, c, d);
    if let Some(s) = filtered {
        assert_eq!(s, exact, "orient3d filter {p:?}");
    }
    assert_eq!(orient3d::exact(a, b, c, d), Some(exact));
    if with_oracle {
        let o = oracle(&[&p[0], &p[1], &p[2], &p[3]], false, true);
        assert_eq!(BigInt::from(value), o, "orient3d value {p:?}");
        assert_eq!(
            predicate::orient3d(a, b, c, d),
            Ok(exact),
            "orient3d public {p:?}"
        );
    }
    tally.count(filtered);
}

fn check_incircle(p: [[i64; 2]; 4], with_oracle: bool, tally: &mut Tally) {
    let [a, b, c, d] = p.map(p2);
    let value = incircle::value(a, b, c, d).expect("the certificate rules out None");
    let exact = value.signum();
    let filtered = incircle::filter(a, b, c, d);
    if let Some(s) = filtered {
        assert_eq!(s, exact, "incircle filter {p:?}");
    }
    assert_eq!(incircle::exact(a, b, c, d), Some(exact));
    if with_oracle {
        let o = oracle(&[&p[0], &p[1], &p[2], &p[3]], true, false);
        assert_eq!(big(&value), o, "incircle value {p:?}");
        assert_eq!(
            predicate::incircle(a, b, c, d),
            Ok(exact),
            "incircle public {p:?}"
        );
    }
    tally.count(filtered);
}

fn check_insphere(p: [[i64; 3]; 5], with_oracle: bool, tally: &mut Tally) {
    let [a, b, c, d, e] = p.map(p3);
    let value = insphere::value(a, b, c, d, e).expect("the certificate rules out None");
    let exact = value.signum();
    let filtered = insphere::filter(a, b, c, d, e);
    if let Some(s) = filtered {
        assert_eq!(s, exact, "insphere filter {p:?}");
    }
    assert_eq!(insphere::exact(a, b, c, d, e), Some(exact));
    if with_oracle {
        let o = oracle(&[&p[0], &p[1], &p[2], &p[3], &p[4]], true, true);
        assert_eq!(big(&value), o, "insphere value {p:?}");
        assert_eq!(
            predicate::insphere(a, b, c, d, e),
            Ok(exact),
            "insphere public {p:?}"
        );
    }
    tally.count(filtered);
}

// ---------------------------------------------------------------------------------------------
// The extremes.
// ---------------------------------------------------------------------------------------------

/// The point set whose coordinate k is +M if bit k of `mask` is set, −M otherwise.
fn at_extremes<const N: usize, const D: usize>(mask: u64) -> [[i64; D]; N] {
    std::array::from_fn(|i| {
        std::array::from_fn(|k| {
            if (mask >> (i * D + k)) & 1 == 1 {
                M
            } else {
                -M
            }
        })
    })
}

/// Every sign combination of ±(2^40 − 1): the exact path equals the oracle, never `None`, and
/// the filter's signs are exact.
#[test]
fn exact_paths_match_the_oracle_at_every_sign_combination_of_the_extremes() {
    let mut t = [
        Tally::default(),
        Tally::default(),
        Tally::default(),
        Tally::default(),
    ];
    for mask in 0..1u64 << 6 {
        check_orient2d(at_extremes(mask), true, &mut t[0]);
    }
    for mask in 0..1u64 << 12 {
        check_orient3d(at_extremes(mask), true, &mut t[1]);
    }
    for mask in 0..1u64 << 8 {
        check_incircle(at_extremes(mask), true, &mut t[2]);
    }
    for mask in 0..1u64 << 15 {
        check_insphere(at_extremes(mask), true, &mut t[3]);
    }
    println!("{t:?}");
}

// ---------------------------------------------------------------------------------------------
// Random point sets.
// ---------------------------------------------------------------------------------------------

fn random<const N: usize, const D: usize>(rng: &mut Rng) -> [[i64; D]; N] {
    std::array::from_fn(|_| std::array::from_fn(|_| rng.coord()))
}

/// Millions of random point sets: every sign the filter gives is exact. One in 64 is also
/// checked against the oracle and the public predicate.
#[test]
fn filters_never_give_a_wrong_sign_on_random_points() {
    let mut rng = Rng::for_test("random points");
    let mut t = [
        Tally::default(),
        Tally::default(),
        Tally::default(),
        Tally::default(),
    ];
    for i in 0..cases(4_000_000) {
        check_orient2d(random(&mut rng), i % 64 == 0, &mut t[0]);
    }
    for i in 0..cases(2_000_000) {
        check_orient3d(random(&mut rng), i % 64 == 0, &mut t[1]);
    }
    for i in 0..cases(2_000_000) {
        check_incircle(random(&mut rng), i % 64 == 0, &mut t[2]);
    }
    for i in 0..cases(1_000_000) {
        check_insphere(random(&mut rng), i % 256 == 0, &mut t[3]);
    }
    println!("{t:?}");
    for tally in &t {
        assert!(tally.certified > 0, "{t:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Near-degenerate constructions at the extremes.
// ---------------------------------------------------------------------------------------------

/// A primitive-ish direction with components up to 2^k for a random k, and a whole multiple of
/// it that keeps a random base point within the grid: (base, direction, largest multiple).
fn lattice_line<const D: usize>(rng: &mut Rng) -> ([i64; D], [i64; D], i64) {
    loop {
        let k = rng.range(0, 39);
        let dir: [i64; D] = std::array::from_fn(|_| rng.range(-(1 << k), 1 << k));
        let largest = dir.iter().map(|v| v.abs()).max().unwrap();
        if largest == 0 {
            continue;
        }
        let steps = rng.range(1, (2 * M) / largest);
        // The base: each coordinate where base + steps · dir stays within the grid.
        let base: [i64; D] = std::array::from_fn(|i| {
            let span = steps * dir[i];
            let lo = -M - span.min(0);
            let hi = M - span.max(0);
            rng.range(lo, hi)
        });
        return (base, dir, steps);
    }
}

fn nudge<const D: usize>(p: [i64; D], rng: &mut Rng, reach: i64) -> [i64; D] {
    std::array::from_fn(|i| (p[i] + rng.range(-reach, reach)).clamp(-M, M))
}

/// orient2d on three points of one lattice line, at every scale and slope, exactly and with the
/// third point nudged by up to two steps, and on points rounded from a real line.
fn near_collinear(rng: &mut Rng) -> [[i64; 2]; 3] {
    let (a, d, steps) = lattice_line::<2>(rng);
    let at = |j: i64| [a[0] + j * d[0], a[1] + j * d[1]];
    let b = at(steps);
    let mut c = at(rng.range(0, steps));
    match rng.next() % 4 {
        0 => {}
        1 => c = nudge(c, rng, 1),
        2 => c = nudge(c, rng, 2),
        _ => {
            // Rounded from the real segment from a to b.
            let t = (rng.next() >> 11) as f64 / (1u64 << 53) as f64;
            c = [
                (a[0] as f64 + t * (b[0] - a[0]) as f64).round() as i64,
                (a[1] as f64 + t * (b[1] - a[1]) as f64).round() as i64,
            ];
            c = c.map(|v| v.clamp(-M, M));
        }
    }
    let mut p = [a, b, c];
    p.swap(0, (rng.next() % 3) as usize);
    p
}

/// orient3d on four points of one lattice plane, exactly and with the fourth nudged.
fn near_coplanar(rng: &mut Rng) -> [[i64; 3]; 4] {
    loop {
        let (a, u, su) = lattice_line::<3>(rng);
        let k = rng.range(0, 39);
        let v: [i64; 3] = std::array::from_fn(|_| rng.range(-(1 << k), 1 << k));
        let sv = rng.range(-4, 4);
        let s = rng.range(0, su);
        let point = |i: i64, j: i64| -> Option<[i64; 3]> {
            let p: [i64; 3] = std::array::from_fn(|n| a[n] + i * u[n] + j * v[n]);
            p.iter().all(|&x| in_range(x)).then_some(p)
        };
        let (Some(b), Some(c), Some(d)) = (point(su, 0), point(0, sv), point(s, sv / 2)) else {
            continue;
        };
        let d = match rng.next() % 3 {
            0 => d,
            1 => nudge(d, rng, 1),
            _ => nudge(d, rng, 2),
        };
        return [a, b, c, d];
    }
}

/// Gaussian integers (x, y) with x² + y² = 5^33, 13^20 or 5^14 · 13^10: products of the
/// primes' Gaussian factors (2 ± i, 3 ± 2i) in every split, with the eight symmetries of the
/// square. Each set lies on one circle of radius about 2^37 to 2^38.3.
fn circle_points(
    prime: (i128, i128),
    n: u32,
    other: Option<((i128, i128), u32)>,
) -> Vec<[i128; 2]> {
    let mul = |(a, b): (i128, i128), (c, d): (i128, i128)| (a * c - b * d, a * d + b * c);
    let pow = |z: (i128, i128), k: u32| (0..k).fold((1, 0), |acc, _| mul(acc, z));
    let mut out = Vec::new();
    let (p, conj) = (prime, (prime.0, -prime.1));
    for k in 0..=n {
        let mut z = mul(pow(p, k), pow(conj, n - k));
        if let Some((q, m)) = other {
            z = mul(z, pow(q, m));
        }
        let (x, y) = z;
        for (sx, sy, swap) in [
            (1, 1, false),
            (-1, 1, false),
            (1, -1, false),
            (-1, -1, false),
            (1, 1, true),
            (-1, 1, true),
            (1, -1, true),
            (-1, -1, true),
        ] {
            let (u, v) = if swap { (y, x) } else { (x, y) };
            out.push([sx * u, sy * v]);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn cocircular(rng: &mut Rng, circles: &[Vec<[i128; 2]>]) -> [[i64; 2]; 4] {
    let circle = &circles[(rng.next() % circles.len() as u64) as usize];
    let center = [
        rng.range(-(1 << 39), 1 << 39),
        rng.range(-(1 << 39), 1 << 39),
    ];
    let mut pick = || {
        let q = circle[(rng.next() % circle.len() as u64) as usize];
        [center[0] + q[0] as i64, center[1] + q[1] as i64]
    };
    let mut p = [pick(), pick(), pick(), pick()];
    match rng.next() % 3 {
        0 => {}
        1 => p[3] = nudge(p[3], rng, 1),
        _ => p[3] = nudge(p[3], rng, 2),
    }
    p.swap(3, (rng.next() % 4) as usize);
    p
}

/// Pythagorean quadruples a² + b² + c² = d²: their points (a, b, c) · (R / d) lie on the sphere of
/// radius R, a whole multiple of every d here, with every sign and order of the coordinates.
fn sphere_points() -> Vec<[i64; 3]> {
    const QUADRUPLES: [[i64; 4]; 14] = [
        [1, 2, 2, 3],
        [2, 3, 6, 7],
        [1, 4, 8, 9],
        [4, 4, 7, 9],
        [2, 6, 9, 11],
        [6, 6, 7, 11],
        [3, 4, 12, 13],
        [2, 5, 14, 15],
        [2, 10, 11, 15],
        [1, 12, 12, 17],
        [8, 9, 12, 17],
        [1, 6, 18, 19],
        [6, 6, 17, 19],
        [6, 10, 15, 19],
    ];
    // lcm(3, 7, 9, 11, 13, 15, 17, 19) = 14,549,535; times 18,889 the radius is about 2^38.
    let radius: i64 = 14_549_535 * 18_889;
    let mut out = Vec::new();
    for [a, b, c, d] in QUADRUPLES {
        let s = radius / d;
        assert_eq!(radius % d, 0);
        let base = [a * s, b * s, c * s];
        for perm in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            for signs in 0..8 {
                out.push(std::array::from_fn(|i| {
                    let v = base[perm[i]];
                    if (signs >> i) & 1 == 1 { -v } else { v }
                }));
            }
        }
    }
    out.push([radius, 0, 0]);
    out.push([0, -radius, 0]);
    out.push([0, 0, radius]);
    out.sort_unstable();
    out.dedup();
    out
}

fn cospherical(rng: &mut Rng, sphere: &[[i64; 3]]) -> [[i64; 3]; 5] {
    let center: [i64; 3] = std::array::from_fn(|_| rng.range(-(1 << 39), 1 << 39));
    let mut pick = || {
        let q = sphere[(rng.next() % sphere.len() as u64) as usize];
        std::array::from_fn(|i| center[i] + q[i])
    };
    let mut p = [pick(), pick(), pick(), pick(), pick()];
    match rng.next() % 3 {
        0 => {}
        1 => p[4] = nudge(p[4], rng, 1),
        _ => p[4] = nudge(p[4], rng, 2),
    }
    p.swap(4, (rng.next() % 5) as usize);
    p
}

/// Near-degenerate sets at the grid's extremes: every sign the filter gives is exact, and the
/// filter both answers and leaves calls to the exact path, so its boundary is crossed both ways.
/// One in 64 is also checked against the oracle and the public predicate.
#[test]
fn filters_never_give_a_wrong_sign_near_degeneracies() {
    let mut rng = Rng::for_test("near-degenerate points");
    let mut t = [
        Tally::default(),
        Tally::default(),
        Tally::default(),
        Tally::default(),
    ];
    for i in 0..cases(2_000_000) {
        check_orient2d(near_collinear(&mut rng), i % 64 == 0, &mut t[0]);
    }
    for i in 0..cases(1_000_000) {
        check_orient3d(near_coplanar(&mut rng), i % 64 == 0, &mut t[1]);
    }
    let circles = [
        circle_points((2, 1), 33, None),
        circle_points((3, 2), 20, None),
        circle_points((2, 1), 14, Some(((3, 2), 10))),
    ];
    for circle in &circles {
        let r2 = circle[0][0] * circle[0][0] + circle[0][1] * circle[0][1];
        assert!(circle.iter().all(|q| q[0] * q[0] + q[1] * q[1] == r2));
        assert!(
            circle
                .iter()
                .all(|q| q[0].abs() < 1 << 39 && q[1].abs() < 1 << 39)
        );
        assert!(circle.len() > 50, "{}", circle.len());
    }
    for i in 0..cases(1_000_000) {
        check_incircle(cocircular(&mut rng, &circles), i % 64 == 0, &mut t[2]);
    }
    let sphere = sphere_points();
    let r2 = sphere[0]
        .iter()
        .map(|v| i128::from(*v).pow(2))
        .sum::<i128>();
    assert!(
        sphere
            .iter()
            .all(|q| q.iter().map(|v| i128::from(*v).pow(2)).sum::<i128>() == r2)
    );
    for i in 0..cases(1_000_000) {
        check_insphere(cospherical(&mut rng, &sphere), i % 256 == 0, &mut t[3]);
    }
    println!("{t:?}");
    for tally in &t {
        assert!(tally.certified > 0 && tally.left > 0, "{t:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// The route.
// ---------------------------------------------------------------------------------------------

/// `predicate.rs`'s code, before its first test module, with `\n` line ends (a Windows checkout
/// may have converted them).
fn predicate_code() -> String {
    let source = include_str!("../src/predicate.rs").replace("\r\n", "\n");
    let end = source.find("#[cfg(test)]").unwrap();
    source[..end].to_owned()
}

/// The text of the function in `code` whose signature starts with `signature`, to its closing
/// brace at the start of a line.
fn function<'c>(code: &'c str, signature: &str) -> &'c str {
    let start = code
        .find(signature)
        .unwrap_or_else(|| panic!("no `{signature}` in predicate.rs"));
    let rest = &code[start..];
    &rest[..rest.find("\n}\n").unwrap()]
}

/// The public predicates route through the generated code, read from `predicate.rs`: each public
/// function calls its generated filter first (orient2d behind `ORIENT2D_FILTER_FIRST`, which is
/// false exactly on 64-bit targets other than WebAssembly), and its exact steps call the generated
/// exact path before the hand-written fast path. Removing a call to the generated code would leave
/// every answer the same, so only the code can show it.
#[test]
fn the_public_predicates_route_through_the_generated_code() {
    let code = predicate_code();
    for (name, signature, exact_steps) in [
        ("orient2d", "pub fn orient2d(", "fn orient2d_exact("),
        ("orient3d", "pub fn orient3d(", "fn orient3d_unfiltered("),
        ("incircle", "pub fn incircle(", "fn incircle_unfiltered("),
        ("insphere", "pub fn insphere(", "fn insphere_unfiltered("),
    ] {
        let public = function(&code, signature);
        let filter = format!("generated::{name}::filter(");
        assert!(
            public.contains(&filter),
            "`{signature}` does not call `{filter}`"
        );
        let steps = function(&code, exact_steps);
        let exact = steps
            .find(&format!("generated::{name}::exact("))
            .unwrap_or_else(|| panic!("`{exact_steps}` does not call the generated exact path"));
        let hand = steps
            .find(&format!("{name}_fast("))
            .unwrap_or_else(|| panic!("`{exact_steps}` lost the hand-written path"));
        assert!(
            exact < hand,
            "{name}: the generated exact path must come first"
        );
    }
    // orient2d: the filter only under the switch, and the switch is off exactly on 64-bit targets
    // other than WebAssembly.
    let public = function(&code, "pub fn orient2d(");
    let switch = public.find("if ORIENT2D_FILTER_FIRST {").unwrap();
    assert!(switch < public.find("generated::orient2d::filter(").unwrap());
    assert!(public.contains("orient2d_exact(a, b, c)"));
    assert!(function(&code, "fn orient2d_unfiltered(").contains("orient2d_exact(a, b, c)"));
    let condition = "const ORIENT2D_FILTER_FIRST: bool = !cfg!(all(\n    target_pointer_width = \
                     \"64\",\n    not(target_family = \"wasm\")\n));";
    assert!(code.contains(condition));
    // And the public functions agree with the generated exact path on a few sets.
    let mut rng = Rng::for_test("route");
    for _ in 0..cases(20_000) {
        let [a, b, c, d]: [[i64; 2]; 4] = random(&mut rng);
        let [pa, pb, pc, pd] = [a, b, c, d].map(p2);
        assert_eq!(
            predicate::orient2d(pa, pb, pc).ok(),
            orient2d::exact(pa, pb, pc)
        );
        assert_eq!(
            predicate::incircle(pa, pb, pc, pd).ok(),
            incircle::exact(pa, pb, pc, pd)
        );
    }
}

// ---------------------------------------------------------------------------------------------
// The zero scale.
// ---------------------------------------------------------------------------------------------

/// Points on one axis line (2D) or axis plane (3D) have a zero group maximum: the filter answers
/// `Zero` at once, and the predicates are exactly zero.
#[test]
fn a_zero_scale_answers_zero_at_once() {
    let mut rng = Rng::for_test("zero scale");
    for _ in 0..cases(50_000) {
        let x = rng.coord();
        let p: [[i64; 2]; 4] = std::array::from_fn(|_| [x, rng.coord()]);
        let [a, b, c, d] = p.map(p2);
        assert_eq!(orient2d::filter(a, b, c), Some(Sign::Zero));
        assert_eq!(incircle::filter(a, b, c, d), Some(Sign::Zero));
        assert_eq!(predicate::orient2d(a, b, c), Ok(Sign::Zero));
        assert_eq!(predicate::incircle(a, b, c, d), Ok(Sign::Zero));
        let z = rng.coord();
        let q: [[i64; 3]; 5] = std::array::from_fn(|_| [rng.coord(), rng.coord(), z]);
        let [a, b, c, d, e] = q.map(p3);
        assert_eq!(orient3d::filter(a, b, c, d), Some(Sign::Zero));
        assert_eq!(insphere::filter(a, b, c, d, e), Some(Sign::Zero));
        assert_eq!(orient3d::value(a, b, c, d), Some(0));
        assert!(insphere::value(a, b, c, d, e).unwrap().is_zero());
    }
}
