//! The generated filters at their bounds (chapter 28.6: inputs "placed just inside and just
//! outside the f64 filter's error bound"), and the generated predicate with rounded inputs,
//! `plane_side`, against exact arithmetic.
//!
//! - **At the bound.** For orient3d, incircle and insphere, point sets built so close to
//!   degenerate that the exact value is of the order of the filter's own bound: points rounded to
//!   the grid from one real plane through two nearly parallel edges, from one real circle, and
//!   from one real sphere, at every scale that brings the exact value near the bound. The bound is
//!   computed here as the filter computes it (its `ERROR` read from the committed module, its
//!   scale from the group maxima by the module's scale expression), and the sets whose exact value
//!   lies between a quarter of the bound and four times it are counted. Every sign the filter
//!   gives is the exact sign (a `num-bigint` determinant of the homogeneous matrix, by cofactor
//!   expansion), and in the band the filter both answers and leaves calls to the exact path, so
//!   its boundary is crossed both ways. orient2d's own construction, by the extended Euclidean
//!   algorithm, is in geom-gen's `tests/attack_g27.rs`.
//! - **Rounded inputs in emitted code.** `plane_side` takes a plane's coefficients as exact
//!   integers, 83 and 125 bits wide, and its filter works on their rounded `f64` copies. Its exact
//!   path equals n · p + d in `i128` and `Plane::side` for planes through grid points; at full
//!   widths, with n · p + d a small whole number of either sign and in the band around the bound,
//!   every sign its filter gives is exact and both outcomes occur; and an input outside its
//!   declared range is refused (`None`), not filtered. This tests the compiled module, not the
//!   generator's steps, so it is independent of the generator's flattening.
//!
//! Seeds: each randomised test prints `seed: <hex>` and runs from a fixed default;
//! `SPAREKIT_SEED=<hex>` replays another (chapter 28.17). Case counts are a fiftieth in debug
//! builds.

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

use geom_num::generated::{incircle, insphere, orient3d, plane_side};
use geom_num::grid::{COORD_MAX, P2, P3};
use geom_num::plane::Plane;
use geom_num::{Sign, Wide};
use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive, Zero};

const M: i64 = COORD_MAX;

// ---------------------------------------------------------------------------------------------
// Seeds, the generator and the oracle.
// ---------------------------------------------------------------------------------------------

/// The default seed of every randomised test here.
const DEFAULT_SEED: u64 = 0x6727_0000_b0d5_0186;

/// `SplitMix64` (Steele, Lea and Flood, OOPSLA 2014), as `geom-test` has it, written here because
/// `geom-num` cannot depend on `geom-test`.
struct Rng(u64);

impl Rng {
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

    /// Uniform in [0, 1).
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform in [lo, hi).
    fn real(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }

    /// About 2^e for e uniform in [lo, hi), every binade between equally often: 2^⌊e⌋ times
    /// 1 + (e − ⌊e⌋), with + and × only, so the same on every platform (no `exp2`).
    fn scale(&mut self, lo: f64, hi: f64) -> f64 {
        let e = self.real(lo, hi);
        let k = e.floor();
        let two_to_k = f64::from_bits(((k as i64 + 1023) as u64) << 52);
        two_to_k * (1.0 + (e - k))
    }

    fn sign(&mut self) -> f64 {
        if self.next() & 1 == 0 { 1.0 } else { -1.0 }
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

fn on_grid(p: &[i64]) -> bool {
    p.iter().all(|v| (-M..=M).contains(v))
}

/// A real point rounded to the nearest grid point.
fn round<const D: usize>(p: [f64; D]) -> [i64; D] {
    p.map(|v| v.round() as i64)
}

fn laplace(m: &[Vec<BigInt>]) -> BigInt {
    let Some((first, rest)) = m.split_first() else {
        return BigInt::from(1);
    };
    let mut det = BigInt::zero();
    for (j, x) in first.iter().enumerate() {
        if x.is_zero() {
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

/// det[p 1] for the points (with the lift column |p|² if `lift`), negated if `negate`: orient3d is
/// −det[a 1; …; d 1], incircle det[a |a|² 1; …] and insphere −det[a |a|² 1; …], the conventions
/// of `geom_num::predicate`.
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

fn sign_of(v: &BigInt) -> Sign {
    if v.is_zero() {
        Sign::Zero
    } else if v.is_negative() {
        Sign::Negative
    } else {
        Sign::Positive
    }
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
// The filters' bounds, as the filters compute them.
// ---------------------------------------------------------------------------------------------

/// The `ERROR` constant of a committed generated module.
fn error_of(module: &str) -> f64 {
    let line = module
        .lines()
        .find(|l| l.starts_with("const ERROR: f64 = f64::from_bits(0x"))
        .expect("an ERROR constant");
    let hex = line
        .trim_start_matches("const ERROR: f64 = f64::from_bits(0x")
        .trim_end_matches(");")
        .replace('_', "");
    f64::from_bits(u64::from_str_radix(&hex, 16).unwrap())
}

/// The module's scale line, which the bound functions below transcribe.
fn scale_line(module: &str) -> &str {
    module
        .lines()
        .find(|l| l.starts_with("    let scale = "))
        .unwrap()
        .trim()
}

const ORIENT3D_RS: &str = include_str!("../src/generated/orient3d.rs");
const INCIRCLE_RS: &str = include_str!("../src/generated/incircle.rs");
const INSPHERE_RS: &str = include_str!("../src/generated/insphere.rs");
const PLANE_SIDE_RS: &str = include_str!("../src/generated/plane_side.rs");

fn larger(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// The largest |v| per axis over the differences `points[i] − base`, in `f64`, exactly.
fn maxima<const D: usize>(points: &[[i64; D]], base: [i64; D]) -> [f64; D] {
    std::array::from_fn(|k| {
        points
            .iter()
            .map(|p| ((p[k] - base[k]) as f64).abs())
            .fold(0.0, larger)
    })
}

/// orient3d's bound, as its filter computes it: differences from a.
fn orient3d_bound([a, b, c, d]: [[i64; 3]; 4]) -> f64 {
    assert_eq!(scale_line(ORIENT3D_RS), "let scale = m_x * m_y * m_z;");
    let [mx, my, mz] = maxima(&[b, c, d], a);
    error_of(ORIENT3D_RS) * (mx * my * mz)
}

/// incircle's bound: differences from d.
fn incircle_bound([a, b, c, d]: [[i64; 2]; 4]) -> f64 {
    assert_eq!(
        scale_line(INCIRCLE_RS),
        "let scale = m_x * m_y * larger(m_y * m_y, m_x * m_x);"
    );
    let [mx, my] = maxima(&[a, b, c], d);
    error_of(INCIRCLE_RS) * (mx * my * larger(my * my, mx * mx))
}

/// insphere's bound: differences from e.
fn insphere_bound([a, b, c, d, e]: [[i64; 3]; 5]) -> f64 {
    assert_eq!(
        scale_line(INSPHERE_RS),
        "let scale = m_x * m_y * m_z * larger(larger(m_z * m_z, m_y * m_y), m_x * m_x);"
    );
    let [mx, my, mz] = maxima(&[a, b, c, d], e);
    error_of(INSPHERE_RS) * (mx * my * mz * larger(larger(mz * mz, my * my), mx * mx))
}

/// How the filter fared on point sets near its bound.
#[derive(Default, Debug)]
struct Band {
    /// Sets in the band, |exact| between a quarter of the bound and four times it.
    inside: u64,
    /// In the band, the filter gave the (exact) sign.
    certified: u64,
    /// In the band, the filter left the call to the exact path.
    left: u64,
    /// Every set, in or out of the band.
    all: u64,
}

impl Band {
    /// Checks one set: any sign the filter gives is `exact`'s sign; tallies the band.
    fn check(&mut self, what: &str, exact: &BigInt, bound: f64, filtered: Option<Sign>) {
        if let Some(s) = filtered {
            assert_eq!(s, sign_of(exact), "{what}: the filter's sign is wrong");
        }
        self.all += 1;
        let ratio = exact.abs().to_f64().unwrap() / bound;
        if (0.25..=4.0).contains(&ratio) {
            self.inside += 1;
            match filtered {
                Some(_) => self.certified += 1,
                None => self.left += 1,
            }
        }
    }

    fn assert_both_ways(&self, name: &str, at_least: u64) {
        println!("{name}: {self:?}");
        assert!(
            self.inside >= at_least && self.certified > 0 && self.left > 0,
            "{name}: the band was not crossed both ways: {self:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// At the bound.
// ---------------------------------------------------------------------------------------------

/// orient3d: a, b, c with c − a nearly parallel to b − a (off by a vector of 2^k, k random), so
/// the plane's normal is small for its edges, and d rounded to the grid from a random real point
/// of that plane: the exact value is the normal times the rounding, of the order of the bound
/// when k is near 31.
#[test]
fn orient3d_filter_is_exact_just_inside_and_just_outside_its_bound() {
    let mut rng = Rng::for_test("orient3d at its bound");
    let mut band = Band::default();
    for _ in 0..cases(200_000) {
        let a: [i64; 3] = std::array::from_fn(|_| rng.range(-(1 << 37), 1 << 37));
        let u: [f64; 3] = std::array::from_fn(|_| rng.real(-1.0, 1.0) * (1u64 << 38) as f64);
        let k = rng.real(20.0, 38.0);
        let e: [f64; 3] = std::array::from_fn(|_| rng.real(-1.0, 1.0) * k.exp2());
        let lambda = rng.real(-1.0, 1.0);
        let ba = round(u);
        let ca = round(std::array::from_fn(|i| lambda * u[i] + e[i]));
        let (s, t) = (rng.real(-0.5, 0.5), rng.real(-0.5, 0.5));
        let da = round(std::array::from_fn(|i| s * ba[i] as f64 + t * ca[i] as f64));
        let at = |v: [i64; 3]| -> [i64; 3] { std::array::from_fn(|i| a[i] + v[i]) };
        let p = [a, at(ba), at(ca), at(da)];
        assert!(p.iter().all(|q| on_grid(q)));
        let exact = oracle(&[&p[0], &p[1], &p[2], &p[3]], false, true);
        let [pa, pb, pc, pd] = p.map(p3);
        assert_eq!(
            BigInt::from(orient3d::value(pa, pb, pc, pd).unwrap()),
            exact
        );
        band.check(
            &format!("orient3d {p:?}"),
            &exact,
            orient3d_bound(p),
            orient3d::filter(pa, pb, pc, pd),
        );
    }
    band.assert_both_ways("orient3d", cases(1_000) as u64);
}

/// A point of the unit circle from a rational parameter, with + − × ÷ only (no `sin` or `cos`,
/// whose results differ between platforms): ((1 − t²) / (1 + t²), 2t / (1 + t²)), every quadrant.
fn on_circle(rng: &mut Rng) -> [f64; 2] {
    let t = rng.real(-1.0, 1.0);
    let w = 1.0 + t * t;
    let (x, y) = ((1.0 - t * t) / w, 2.0 * t / w);
    [rng.sign() * x, rng.sign() * y]
}

/// incircle: four points rounded to the grid from one real circle of radius 2^36 to 2^39.6 (each
/// rounding moves a point off the circle by up to half a step, so the exact value is of the order
/// of the radius cubed, and of the bound at the larger radii).
#[test]
fn incircle_filter_is_exact_just_inside_and_just_outside_its_bound() {
    let mut rng = Rng::for_test("incircle at its bound");
    let mut band = Band::default();
    for _ in 0..cases(200_000) {
        let center = [
            rng.real(-1.0, 1.0) * (1u64 << 36) as f64,
            rng.real(-1.0, 1.0) * (1u64 << 36) as f64,
        ];
        let r = rng.scale(36.0, 39.6);
        let p: [[i64; 2]; 4] = std::array::from_fn(|_| {
            let q = on_circle(&mut rng);
            round([center[0] + r * q[0], center[1] + r * q[1]])
        });
        assert!(p.iter().all(|q| on_grid(q)));
        let exact = oracle(&[&p[0], &p[1], &p[2], &p[3]], true, false);
        let [pa, pb, pc, pd] = p.map(p2);
        assert_eq!(big(&incircle::value(pa, pb, pc, pd).unwrap()), exact);
        band.check(
            &format!("incircle {p:?}"),
            &exact,
            incircle_bound(p),
            incircle::filter(pa, pb, pc, pd),
        );
    }
    band.assert_both_ways("incircle", cases(1_000) as u64);
}

/// A point of the unit sphere from a rational parameter, by inverse stereographic projection:
/// (2u, 2v, u² + v² − 1) / (1 + u² + v²), with + − × ÷ only, both hemispheres.
fn on_sphere(rng: &mut Rng) -> [f64; 3] {
    let (u, v) = (rng.real(-1.0, 1.0), rng.real(-1.0, 1.0));
    let w = 1.0 + u * u + v * v;
    [
        2.0 * u / w,
        2.0 * v / w,
        rng.sign() * (u * u + v * v - 1.0) / w,
    ]
}

/// insphere: five points rounded to the grid from one real sphere of radius 2^30 to 2^39.6.
#[test]
fn insphere_filter_is_exact_just_inside_and_just_outside_its_bound() {
    let mut rng = Rng::for_test("insphere at its bound");
    let mut band = Band::default();
    for _ in 0..cases(100_000) {
        let center: [f64; 3] = std::array::from_fn(|_| rng.real(-1.0, 1.0) * (1u64 << 36) as f64);
        let r = rng.scale(30.0, 39.6);
        let p: [[i64; 3]; 5] = std::array::from_fn(|_| {
            let q = on_sphere(&mut rng);
            round(std::array::from_fn(|i| center[i] + r * q[i]))
        });
        assert!(p.iter().all(|q| on_grid(q)));
        let exact = oracle(&[&p[0], &p[1], &p[2], &p[3], &p[4]], true, true);
        let [pa, pb, pc, pd, pe] = p.map(p3);
        assert_eq!(big(&insphere::value(pa, pb, pc, pd, pe).unwrap()), exact);
        band.check(
            &format!("insphere {p:?}"),
            &exact,
            insphere_bound(p),
            insphere::filter(pa, pb, pc, pd, pe),
        );
    }
    band.assert_both_ways("insphere", cases(1_000) as u64);
}

// ---------------------------------------------------------------------------------------------
// plane_side: rounded inputs in emitted code.
// ---------------------------------------------------------------------------------------------

/// n · p + d, exactly, in `i128` (|`n_i`| < 2^83, |`p_i`| < 2^40, |d| < 2^125: below 2^127).
fn side_value(n: [i128; 3], d: i128, p: [i64; 3]) -> i128 {
    n[0] * i128::from(p[0]) + n[1] * i128::from(p[1]) + n[2] * i128::from(p[2]) + d
}

/// `plane_side`'s bound, as its filter computes it: the group maxima of the rounded coefficients,
/// the offset and each coordinate.
fn plane_side_bound(n: [i128; 3], d: i128, p: [i64; 3]) -> f64 {
    assert_eq!(
        scale_line(PLANE_SIDE_RS),
        "let scale = larger(larger(larger(m_d, m_z * m_n), m_y * m_n), m_x * m_n);"
    );
    let m_n = n.iter().map(|v| (*v as f64).abs()).fold(0.0, larger);
    let m_d = (d as f64).abs();
    let [m_x, m_y, m_z] = p.map(|v| (v as f64).abs());
    error_of(PLANE_SIDE_RS) * larger(larger(larger(m_d, m_z * m_n), m_y * m_n), m_x * m_n)
}

/// For planes through grid points, the generated exact path is n · p + d and its sign is
/// `Plane::side`'s; the filter's signs are that sign.
#[test]
fn plane_side_agrees_with_plane_side_for_planes_through_grid_points() {
    let mut rng = Rng::for_test("plane_side and Plane::side");
    let coord = |rng: &mut Rng| match rng.next() % 4 {
        0 => M,
        1 => -M,
        _ => rng.range(-M, M),
    };
    let mut checked = 0;
    for _ in 0..cases(200_000) {
        let [a, b, c, p]: [[i64; 3]; 4] =
            std::array::from_fn(|_| std::array::from_fn(|_| coord(&mut rng)));
        let Ok(plane) = Plane::through(p3(a), p3(b), p3(c)) else {
            continue;
        };
        let (n, d) = (plane.n(), plane.d());
        let side = plane.side(p3(p)).unwrap();
        let v = side_value(n, d, p);
        assert_eq!(plane_side::value(n[0], n[1], n[2], d, p3(p)), Some(v));
        assert_eq!(plane_side::exact(n[0], n[1], n[2], d, p3(p)), Some(side));
        if let Some(s) = plane_side::filter(n[0], n[1], n[2], d, p3(p)) {
            assert_eq!(s, side, "{a:?} {b:?} {c:?} {p:?}");
        }
        checked += 1;
    }
    assert!(checked > 0);
}

/// Coefficients at their full widths, 83 and 125 bits, rounded in the filter, and n · p + d a
/// whole number t of either sign: small (up to 2^90), or within a factor of four of the filter's
/// bound. Every sign the filter gives is exact; in the band both outcomes occur.
#[test]
fn plane_side_filter_is_exact_on_rounded_inputs_at_full_width() {
    let mut rng = Rng::for_test("plane_side at full width");
    let magnitude = |bits: u32, rng: &mut Rng| -> i128 {
        let v = (i128::from(rng.next()) << 64) | i128::from(rng.next());
        v & ((1 << bits) - 1)
    };
    let mut band = Band::default();
    for i in 0..cases(1_000_000) {
        // n at its full 83 bits: the top bit set, the rest random, so its f64 copy is rounded.
        let n: [i128; 3] = std::array::from_fn(|_| {
            let v = magnitude(82, &mut rng) | (1 << 82);
            if rng.next() & 1 == 0 { v } else { -v }
        });
        let p: [i64; 3] = std::array::from_fn(|_| match rng.next() % 4 {
            0 => M,
            1 => -M,
            _ => rng.range(-M, M),
        });
        let dot = side_value(n, 0, p);
        // t: small, or placed against the bound computed for d = −dot (the bound moves little
        // with t, which is far smaller than d).
        let t = if i % 2 == 0 {
            let v = magnitude((rng.next() % 91) as u32, &mut rng);
            if rng.next() & 1 == 0 { v } else { -v }
        } else {
            let bound = plane_side_bound(n, -dot, p);
            (rng.sign() * rng.scale(-2.0, 2.0) * bound) as i128
        };
        let d = t - dot;
        assert!(d.unsigned_abs() < 1 << 125);
        assert_eq!(side_value(n, d, p), t);
        assert_eq!(plane_side::value(n[0], n[1], n[2], d, p3(p)), Some(t));
        band.check(
            &format!("plane_side n {n:?} d {d} p {p:?}"),
            &BigInt::from(t),
            plane_side_bound(n, d, p),
            plane_side::filter(n[0], n[1], n[2], d, p3(p)),
        );
    }
    band.assert_both_ways("plane_side", cases(10_000) as u64);
}

/// A coefficient outside its declared range (|`n_i`| ≤ 2^83 − 1, |d| ≤ 2^125 − 1) is refused by
/// the filter and the exact path, never filtered with a bound that does not hold for it; at the
/// range's ends it is accepted.
#[test]
fn plane_side_refuses_inputs_outside_their_range() {
    let p = p3([M, -M, 1]);
    let n_max: i128 = (1 << 83) - 1;
    let d_max: i128 = (1 << 125) - 1;
    for (n, d) in [
        ([n_max + 1, 0, 0], 0),
        ([0, -(n_max + 1), 0], 0),
        ([0, 0, n_max + 1], 1),
        ([1, 1, 1], d_max + 1),
        ([1, 1, 1], -(d_max + 1)),
        ([i128::MAX, 0, 0], 0),
        ([0, 0, 0], i128::MIN),
    ] {
        assert_eq!(
            plane_side::filter(n[0], n[1], n[2], d, p),
            None,
            "{n:?} {d}"
        );
        assert_eq!(plane_side::value(n[0], n[1], n[2], d, p), None, "{n:?} {d}");
        assert_eq!(plane_side::exact(n[0], n[1], n[2], d, p), None, "{n:?} {d}");
    }
    for (n, d) in [
        ([n_max, -n_max, n_max], d_max),
        ([-n_max, n_max, -n_max], -d_max),
    ] {
        let v = side_value(n, d, [M, -M, 1]);
        assert_eq!(plane_side::value(n[0], n[1], n[2], d, p), Some(v));
        assert_eq!(
            plane_side::exact(n[0], n[1], n[2], d, p),
            Some(Sign::of_i128(v))
        );
        if let Some(s) = plane_side::filter(n[0], n[1], n[2], d, p) {
            assert_eq!(s, Sign::of_i128(v));
        }
    }
}
