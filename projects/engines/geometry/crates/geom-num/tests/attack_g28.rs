//! Attacks on piece G28: the explicit-point predicates of `geom_num::predicate` (orient2d and
//! orient3d in `i128`, incircle and insphere in 256 bits), each behind its generated filter and
//! returning a sign in a `Result` whose error only an internal defect can cause (chapter 6.2 of the
//! kernel architecture). Every test tries to break a claim of the piece's done-when or of the
//! module's guarantees.
//!
//! Two methods that share no code with the module decide what is right:
//!
//! - **A big-integer oracle**: the Leibniz expansion, the definition of the determinant, of the
//!   homogeneous matrix of the absolute coordinates (rows [p, 1], or [p, |p|², 1] for the lifted
//!   tests), on `dashu` integers. The module translates to one point and evaluates differences in
//!   fixed-width integers, and the other predicate tests use `num-bigint`, so this oracle shares
//!   neither the formulation nor the library.
//! - **Shewchuk's adaptive predicates**, from the `robust` crate (georust, MIT OR Apache-2.0),
//!   chapter 28.6's second method. Grid coordinates are doubles exactly, so its answers are exact.
//!   `robust::orient2d` and `robust::incircle` have the module's signs; `robust::orient3d` and
//!   `robust::insphere` have the opposite (chapter 6.2) and are negated here, and the conventions
//!   test pins that with geometry that uses no determinant.
//!
//! Every input every test makes passes one check: the public predicate answers `Ok` with the
//! oracle's sign, `robust` agrees, and the generated filter, called on its own, either declines or
//! gives the oracle's sign. On the sign corners and at the filter's boundary the value of the
//! generated exact path must equal the oracle's too.
//!
//! The claims attacked:
//!
//! - **Every sign corner** (chapter 28.6): every configuration with every coordinate at
//!   ±(2^40 − 1): 64 for orient2d, 4,096 for orient3d, 256 for incircle and 32,768 for insphere.
//!   The largest values the module documents (4M² and 16M³) are reached there, and no value passes
//!   the budget its header states. And one step inside them: every coordinate in {±M, ±(M − 1)},
//!   all 4,096 configurations for orient2d and all 65,536 for incircle, a sample for the others.
//! - **The written bit budget** in the module header: its table is read, and every cell
//!   re-derived in big integers from the rules it states. The largest in-circle and in-sphere
//!   values it states over the points with coordinates in {−M, 0, M} (8M⁴ and 48M⁵) are found by
//!   taking every set of distinct such points through the public predicates.
//! - **The oracle on random inputs**, at every scale and every position on the grid.
//! - **One step from every degeneracy**: exactly collinear, coplanar, cocircular and cospherical
//!   points (circles from Gaussian integers and spheres from Lipschitz quaternions, up to full
//!   scale), and the lower-dimensional degeneracies the module documents, each then moved by one
//!   step in every direction.
//! - **The filter's boundary, crossed both ways**: families of inputs whose exact value grows by
//!   small steps from zero, or shrinks towards zero from inside a circle or sphere, searched for
//!   the step where the filter's decision changes. Every input in a window around that step is
//!   checked, so the filter's answers just outside its bound and its refusals just inside it are
//!   both checked against the oracle, and the refusals reach the exact path.
//! - **The laws** (chapter 28.6): swapping two points flips the sign and every permutation
//!   multiplies it by its parity; moving every point by one grid vector changes nothing. Beyond
//!   the done-when, a signed permutation of the axes multiplies the sign by its determinant, and
//!   scaling by a positive whole number keeps it.
//!
//! Seeds: each randomised test prints `seed: <hex>` and runs from a fixed default;
//! `SPAREKIT_SEED=<hex>` replays another (chapter 28.17). Case counts are a twentieth in debug
//! builds; the corners always run in full.

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

use std::array;

use dashu_int::IBig;
use geom_num::grid::{COORD_MAX, P2, P3};
use geom_num::{I256, NumError, Sign, generated, predicate};

/// The largest coordinate magnitude, M = 2^40 − 1.
const M: i64 = COORD_MAX;

type Pt2 = [i64; 2];
type Pt3 = [i64; 3];

// ---------------------------------------------------------------------------------------------
// Seeds and the generator.
// ---------------------------------------------------------------------------------------------

/// The default seed of every randomised test here (G28, issue 206).
const DEFAULT_SEED: u64 = 0x6728_0000_5eed_0206;

/// `SplitMix64` (Steele, Lea and Flood, *Fast splittable pseudorandom number generators*,
/// OOPSLA 2014), the generator the other predicate tests write out too: `geom-num` cannot depend
/// on `geom-test`.
struct Rng(u64);

impl Rng {
    /// The test's generator: the seed from `SPAREKIT_SEED` (hex) or the default, printed.
    fn for_test(name: &str) -> Rng {
        let seed = std::env::var("SPAREKIT_SEED")
            .ok()
            .and_then(|s| u64::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok())
            .unwrap_or(DEFAULT_SEED);
        println!("{name}: seed: {seed:016x}");
        // Each test its own stream, so adding a test never moves another's inputs.
        let mut name_hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in name.bytes() {
            name_hash = (name_hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
        Rng(seed ^ name_hash)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in 0..n (the modulo's bias is far below anything a test could notice).
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    /// Uniform in lo..=hi.
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        assert!(lo <= hi, "empty range {lo}..={hi}");
        let span = (i128::from(hi) - i128::from(lo) + 1) as u128;
        (i128::from(lo) + (u128::from(self.next()) % span) as i128) as i64
    }

    fn coin(&mut self) -> bool {
        self.next() & 1 == 1
    }

    /// 1 or −1.
    fn sign(&mut self) -> i64 {
        if self.coin() { 1 } else { -1 }
    }

    /// A coordinate: anywhere on the grid, at or next to one of its edges, or within 2^k of zero
    /// for a random k, so every scale is reached.
    fn coord(&mut self) -> i64 {
        match self.next() % 8 {
            0 => M,
            1 => -M,
            2 => M - self.range(0, 3),
            3 => -M + self.range(0, 3),
            4 | 5 => {
                let k = self.range(0, 40);
                self.range(-(1 << k) + 1, (1 << k) - 1)
            }
            _ => self.range(-M, M),
        }
    }

    /// A point within `reach` of the origin on every axis.
    fn within<const D: usize>(&mut self, reach: i64) -> [i64; D] {
        array::from_fn(|_| self.range(-reach, reach))
    }

    /// The N points in a random order.
    fn shuffle<T: Copy, const N: usize>(&mut self, mut items: [T; N]) -> [T; N] {
        for i in (1..N).rev() {
            let j = self.below(i + 1);
            items.swap(i, j);
        }
        items
    }
}

/// Release builds run every count in full; debug builds a twentieth.
fn cases(release: usize) -> usize {
    if cfg!(debug_assertions) {
        (release / 20).max(1)
    } else {
        release
    }
}

// ---------------------------------------------------------------------------------------------
// Points and small integer helpers.
// ---------------------------------------------------------------------------------------------

fn p2([x, y]: Pt2) -> P2 {
    P2::new(x, y).unwrap()
}

fn p3([x, y, z]: Pt3) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// Whether every coordinate of every point is on the grid, |v| ≤ M.
fn on_grid<const D: usize, const N: usize>(points: &[[i64; D]; N]) -> bool {
    points.iter().flatten().all(|v| (-M..=M).contains(v))
}

fn add<const D: usize>(p: [i64; D], q: [i64; D]) -> [i64; D] {
    array::from_fn(|i| p[i] + q[i])
}

fn sub<const D: usize>(p: [i64; D], q: [i64; D]) -> [i64; D] {
    array::from_fn(|i| p[i] - q[i])
}

fn times<const D: usize>(k: i64, p: [i64; D]) -> [i64; D] {
    array::from_fn(|i| k * p[i])
}

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// (g, x, y) with a·x + b·y = g = gcd(a, b) ≥ 0: the extended Euclidean algorithm (Knuth,
/// volume 2, section 4.5.2, algorithm X).
fn extended_gcd(a: i128, b: i128) -> (i128, i128, i128) {
    let (mut r0, mut r1) = (a, b);
    let (mut x0, mut x1) = (1i128, 0i128);
    let (mut y0, mut y1) = (0i128, 1i128);
    while r1 != 0 {
        let q = r0.div_euclid(r1);
        (r0, r1) = (r1, r0 - q * r1);
        (x0, x1) = (x1, x0 - q * x1);
        (y0, y1) = (y1, y0 - q * y1);
    }
    if r0 < 0 {
        (-r0, -x0, -y0)
    } else {
        (r0, x0, y0)
    }
}

/// ⌊√n⌋ for n ≥ 0, exactly.
fn isqrt(n: i128) -> i128 {
    assert!(n >= 0);
    let mut x = (n as f64).sqrt() as i128;
    while x * x > n {
        x -= 1;
    }
    while (x + 1) * (x + 1) <= n {
        x += 1;
    }
    x
}

/// The range of k for which p + k·w stays on the grid (it includes 0 when p is on it).
fn multiples_on_grid<const D: usize>(p: [i64; D], w: [i64; D]) -> (i64, i64) {
    let (mut lo, mut hi) = (i128::from(i64::MIN), i128::from(i64::MAX));
    for (&pi, &wi) in p.iter().zip(&w) {
        let (pi, wi, m) = (i128::from(pi), i128::from(wi), i128::from(M));
        if wi == 0 {
            continue;
        }
        // −M ≤ p + k·w ≤ M.
        let (a, b) = ((-m - pi), (m - pi));
        let (from, to) = if wi > 0 {
            (
                a.div_euclid(wi) + i128::from(a.rem_euclid(wi) != 0),
                b.div_euclid(wi),
            )
        } else {
            let (wi, a, b) = (-wi, -b, -a);
            (
                a.div_euclid(wi) + i128::from(a.rem_euclid(wi) != 0),
                b.div_euclid(wi),
            )
        };
        lo = lo.max(from);
        hi = hi.min(to);
    }
    (lo as i64, hi as i64)
}

// ---------------------------------------------------------------------------------------------
// The oracle: the homogeneous determinant by the Leibniz expansion, on dashu integers.
// ---------------------------------------------------------------------------------------------

/// Orderings of 0..n, each with whether it is odd.
type Orderings = Vec<(Vec<usize>, bool)>;

/// Every ordering of 0..n, with whether it is odd (an odd number of inversions).
fn orderings(n: usize) -> Orderings {
    let mut all: Vec<Vec<usize>> = vec![Vec::new()];
    for _ in 0..n {
        let mut longer = Vec::new();
        for order in &all {
            for v in 0..n {
                if !order.contains(&v) {
                    let mut next = order.clone();
                    next.push(v);
                    longer.push(next);
                }
            }
        }
        all = longer;
    }
    all.into_iter()
        .map(|order| {
            let mut inversions = 0;
            for i in 0..n {
                for j in i + 1..n {
                    inversions += usize::from(order[i] > order[j]);
                }
            }
            (order, inversions % 2 == 1)
        })
        .collect()
}

/// The row [p, |p|², 1] (lifted) or [p, 1] of a homogeneous matrix.
fn row<const D: usize>(p: [i64; D], lift: bool) -> Vec<IBig> {
    let mut entries: Vec<IBig> = p.iter().map(|&v| IBig::from(v)).collect();
    if lift {
        let squares = p.iter().map(|&v| IBig::from(v) * IBig::from(v));
        entries.push(squares.fold(IBig::ZERO, |sum, square| sum + square));
    }
    entries.push(IBig::ONE);
    entries
}

/// The exact determinants every check is decided by.
struct Oracle {
    /// The orderings of 0..n with their parities, for n up to 5.
    orderings: [Orderings; 6],
}

impl Oracle {
    fn new() -> Oracle {
        Oracle {
            orderings: array::from_fn(orderings),
        }
    }

    /// det(m) = Σ over orderings σ of sign(σ) · Π m[i][σ(i)].
    fn det(&self, m: &[Vec<IBig>]) -> IBig {
        let n = m.len();
        assert!(m.iter().all(|r| r.len() == n), "a square matrix");
        let mut total = IBig::ZERO;
        'terms: for (order, odd) in &self.orderings[n] {
            let mut term = IBig::ONE;
            for (r, &column) in m.iter().zip(order) {
                let entry = &r[column];
                if entry.is_zero() {
                    continue 'terms;
                }
                term *= entry;
            }
            if *odd {
                total -= term;
            } else {
                total += term;
            }
        }
        total
    }

    /// The homogeneous determinant of the points.
    fn homogeneous<const D: usize, const N: usize>(
        &self,
        points: [[i64; D]; N],
        lift: bool,
    ) -> IBig {
        let rows: Vec<Vec<IBig>> = points.iter().map(|&p| row(p, lift)).collect();
        self.det(&rows)
    }
}

/// |v|.
fn magnitude(v: &IBig) -> IBig {
    if *v < IBig::ZERO { -v } else { v.clone() }
}

fn sign_of(v: &IBig) -> Sign {
    if v.is_zero() {
        Sign::Zero
    } else if *v > IBig::ZERO {
        Sign::Positive
    } else {
        Sign::Negative
    }
}

/// The sign of a double that `robust` returned; it is never NaN for finite inputs.
fn sign_of_f64(v: f64) -> Sign {
    assert!(!v.is_nan(), "robust returned NaN");
    if v > 0.0 {
        Sign::Positive
    } else if v < 0.0 {
        Sign::Negative
    } else {
        Sign::Zero
    }
}

/// An `I256` as a big integer: its limbs are two's complement, least significant first.
fn ibig_of(w: I256) -> IBig {
    let limbs = w.to_le_limbs();
    let mut v = IBig::ZERO;
    for &limb in limbs.iter().rev() {
        v = (v << 64) + IBig::from(limb);
    }
    if limbs[3] >> 63 == 1 {
        v -= IBig::ONE << 256;
    }
    v
}

// ---------------------------------------------------------------------------------------------
// The four predicates, each with its oracle, its generated parts and robust's version.
// ---------------------------------------------------------------------------------------------

/// A grid coordinate as a double: exactly, since |v| ≤ 2^40 < 2^53.
fn exactly(v: i64) -> f64 {
    let f = v as f64;
    assert_eq!(f as i64, v, "{v} is not a double exactly");
    f
}

fn c2([x, y]: Pt2) -> robust::Coord<f64> {
    robust::Coord {
        x: exactly(x),
        y: exactly(y),
    }
}

fn c3([x, y, z]: Pt3) -> robust::Coord3D<f64> {
    robust::Coord3D {
        x: exactly(x),
        y: exactly(y),
        z: exactly(z),
    }
}

/// One explicit-point predicate on N points in D dimensions.
struct Predicate<const D: usize, const N: usize> {
    name: &'static str,
    /// Whether the oracle's rows carry the lift |p|².
    lift: bool,
    /// Whether the module's sign is the opposite of the homogeneous determinant's, which is also
    /// the sign of Shewchuk's predicate and so of `robust`'s (chapter 6.2).
    negate: bool,
    /// The public predicate.
    public: fn([[i64; D]; N]) -> Result<Sign, NumError>,
    /// The generated filter, on its own.
    filter: fn([[i64; D]; N]) -> Option<Sign>,
    /// The generated exact path's value.
    value: fn([[i64; D]; N]) -> Option<IBig>,
    /// `robust`'s answer, a double whose sign is Shewchuk's.
    robust: fn([[i64; D]; N]) -> f64,
}

const ORIENT2D: Predicate<2, 3> = Predicate {
    name: "orient2d",
    lift: false,
    negate: false,
    public: |[a, b, c]| predicate::orient2d(p2(a), p2(b), p2(c)),
    filter: |[a, b, c]| generated::orient2d::filter(p2(a), p2(b), p2(c)),
    value: |[a, b, c]| generated::orient2d::value(p2(a), p2(b), p2(c)).map(IBig::from),
    robust: |[a, b, c]| robust::orient2d(c2(a), c2(b), c2(c)),
};

const ORIENT3D: Predicate<3, 4> = Predicate {
    name: "orient3d",
    lift: false,
    negate: true,
    public: |[a, b, c, d]| predicate::orient3d(p3(a), p3(b), p3(c), p3(d)),
    filter: |[a, b, c, d]| generated::orient3d::filter(p3(a), p3(b), p3(c), p3(d)),
    value: |[a, b, c, d]| generated::orient3d::value(p3(a), p3(b), p3(c), p3(d)).map(IBig::from),
    robust: |[a, b, c, d]| robust::orient3d(c3(a), c3(b), c3(c), c3(d)),
};

const INCIRCLE: Predicate<2, 4> = Predicate {
    name: "incircle",
    lift: true,
    negate: false,
    public: |[a, b, c, d]| predicate::incircle(p2(a), p2(b), p2(c), p2(d)),
    filter: |[a, b, c, d]| generated::incircle::filter(p2(a), p2(b), p2(c), p2(d)),
    value: |[a, b, c, d]| generated::incircle::value(p2(a), p2(b), p2(c), p2(d)).map(ibig_of),
    robust: |[a, b, c, d]| robust::incircle(c2(a), c2(b), c2(c), c2(d)),
};

const INSPHERE: Predicate<3, 5> = Predicate {
    name: "insphere",
    lift: true,
    negate: true,
    public: |[a, b, c, d, e]| predicate::insphere(p3(a), p3(b), p3(c), p3(d), p3(e)),
    filter: |[a, b, c, d, e]| generated::insphere::filter(p3(a), p3(b), p3(c), p3(d), p3(e)),
    value: |[a, b, c, d, e]| {
        generated::insphere::value(p3(a), p3(b), p3(c), p3(d), p3(e)).map(ibig_of)
    },
    robust: |[a, b, c, d, e]| robust::insphere(c3(a), c3(b), c3(c), c3(d), c3(e)),
};

/// What a run of checks saw.
#[derive(Debug, Default)]
struct Tally {
    checked: u64,
    /// Inputs the filter decided, and inputs it left to the exact path.
    decided: u64,
    declined: u64,
    positive: u64,
    zero: u64,
    negative: u64,
}

impl Tally {
    fn note(&mut self, sign: Sign, filtered: Option<Sign>) {
        self.checked += 1;
        match filtered {
            Some(_) => self.decided += 1,
            None => self.declined += 1,
        }
        match sign {
            Sign::Positive => self.positive += 1,
            Sign::Zero => self.zero += 1,
            Sign::Negative => self.negative += 1,
        }
    }
}

impl Oracle {
    /// The predicate's exact value, with the module's sign convention.
    fn exact<const D: usize, const N: usize>(
        &self,
        pred: &Predicate<D, N>,
        p: [[i64; D]; N],
    ) -> IBig {
        let v = self.homogeneous(p, pred.lift);
        if pred.negate { -v } else { v }
    }

    /// The check every input passes: the public predicate answers `Ok` with the oracle's sign,
    /// `robust` agrees, and the filter declines or agrees. Returns the exact value.
    fn check<const D: usize, const N: usize>(
        &self,
        pred: &Predicate<D, N>,
        p: [[i64; D]; N],
        tally: &mut Tally,
    ) -> IBig {
        let name = pred.name;
        assert!(
            on_grid(&p),
            "{name}{p:?}: the test made a point off the grid"
        );
        let exact = self.exact(pred, p);
        let sign = sign_of(&exact);
        assert_eq!(
            (pred.public)(p),
            Ok(sign),
            "{name}{p:?}: the public predicate against the oracle ({exact})"
        );
        let shewchuk = sign_of_f64((pred.robust)(p));
        let robust = if pred.negate {
            shewchuk.negate()
        } else {
            shewchuk
        };
        assert_eq!(
            robust, sign,
            "{name}{p:?}: robust against the oracle ({exact})"
        );
        let filtered = (pred.filter)(p);
        if let Some(filtered) = filtered {
            assert_eq!(
                filtered, sign,
                "{name}{p:?}: the generated filter gave a wrong sign ({exact})"
            );
        }
        tally.note(sign, filtered);
        exact
    }

    /// [`Oracle::check`], and the generated exact path's value is the oracle's, exactly.
    fn check_value<const D: usize, const N: usize>(
        &self,
        pred: &Predicate<D, N>,
        p: [[i64; D]; N],
        tally: &mut Tally,
    ) -> IBig {
        let exact = self.check(pred, p, tally);
        let name = pred.name;
        assert_eq!(
            (pred.value)(p),
            Some(exact.clone()),
            "{name}{p:?}: the generated exact path's value against the oracle"
        );
        exact
    }
}

// ---------------------------------------------------------------------------------------------
// Conventions, pinned with geometry and no determinant.
// ---------------------------------------------------------------------------------------------

/// The four predicates' signs on configurations whose answer is known from geometry alone, at
/// small and at full scale: the module's sign, the oracle's, and `robust`'s, which is the module's
/// for orient2d and incircle and the opposite for orient3d and insphere (chapter 6.2).
#[test]
fn conventions_are_pinned_by_geometry_and_robust_is_negated_exactly_where_documented() {
    let oracle = Oracle::new();
    let mut tally = Tally::default();
    for k in [1, 2, 1 << 20, (M - 1) / 4] {
        // a, b, c counter-clockwise: c lies left of the line from a to b.
        let tri = [[-k, -k], [k, -k], [-k, k]];
        assert_eq!(
            sign_of(&oracle.check(&ORIENT2D, tri, &mut tally)),
            Sign::Positive
        );
        assert_eq!(sign_of_f64((ORIENT2D.robust)(tri)), Sign::Positive);
        // d on the side the right-hand normal (b − a) × (c − a) = +z points to.
        let up = [[0, 0, 0], [2 * k, 0, 0], [0, 2 * k, 0], [0, 0, 2 * k]];
        let down = [[0, 0, 0], [2 * k, 0, 0], [0, 2 * k, 0], [0, 0, -2 * k]];
        assert_eq!(
            sign_of(&oracle.check(&ORIENT3D, up, &mut tally)),
            Sign::Positive
        );
        assert_eq!(
            sign_of(&oracle.check(&ORIENT3D, down, &mut tally)),
            Sign::Negative
        );
        assert_eq!(sign_of_f64((ORIENT3D.robust)(up)), Sign::Negative);
        // The circle through (−k, −k), (k, −k), (−k, k) has centre 0 and radius √2·k: its centre is
        // inside, (k, k) on it, (2k, 2k) outside.
        let centre = [[-k, -k], [k, -k], [-k, k], [0, 0]];
        let on = [[-k, -k], [k, -k], [-k, k], [k, k]];
        let outside = [[-k, -k], [k, -k], [-k, k], [2 * k, 2 * k]];
        assert_eq!(
            sign_of(&oracle.check(&INCIRCLE, centre, &mut tally)),
            Sign::Positive
        );
        assert_eq!(
            sign_of(&oracle.check(&INCIRCLE, on, &mut tally)),
            Sign::Zero
        );
        assert_eq!(
            sign_of(&oracle.check(&INCIRCLE, outside, &mut tally)),
            Sign::Negative
        );
        assert_eq!(sign_of_f64((INCIRCLE.robust)(centre)), Sign::Positive);
        // The sphere through 0, 2k·x, 2k·y, 2k·z has centre (k, k, k) and radius √3·k, and
        // orient3d of those four is Positive (above): its centre is inside, (2k, 2k, 2k) on it,
        // (3k, 3k, 3k) outside.
        let base = [[0, 0, 0], [2 * k, 0, 0], [0, 2 * k, 0], [0, 0, 2 * k]];
        let with = |e: Pt3| [base[0], base[1], base[2], base[3], e];
        let inside = with([k, k, k]);
        assert_eq!(
            sign_of(&oracle.check(&INSPHERE, inside, &mut tally)),
            Sign::Positive
        );
        assert_eq!(
            sign_of(&oracle.check(&INSPHERE, with([2 * k, 2 * k, 2 * k]), &mut tally)),
            Sign::Zero
        );
        assert_eq!(
            sign_of(&oracle.check(&INSPHERE, with([3 * k, 3 * k, 3 * k]), &mut tally)),
            Sign::Negative
        );
        assert_eq!(sign_of_f64((INSPHERE.robust)(inside)), Sign::Negative);
    }
}

// ---------------------------------------------------------------------------------------------
// The written bit budget: the module header's table, re-derived.
// ---------------------------------------------------------------------------------------------

/// δ = 2M = 2^41 − 2, the largest magnitude of a difference of two grid coordinates.
fn delta() -> IBig {
    IBig::from(2 * M)
}

/// One cell of the header's table, k·δ^n < 2^bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cell {
    k: u32,
    n: u32,
    bits: u32,
}

impl Cell {
    /// k·δ^n.
    fn value(self) -> IBig {
        let mut v = IBig::from(self.k);
        for _ in 0..self.n {
            v *= delta();
        }
        v
    }

    /// The product rule: values at most k₁·δ^n₁ and k₂·δ^n₂ have a product at most
    /// k₁k₂·δ^(n₁ + n₂). The bits are left for [`Cell::tight`] to check.
    fn times(self, other: Cell, bits: u32) -> Cell {
        Cell {
            k: self.k * other.k,
            n: self.n + other.n,
            bits,
        }
    }

    /// The sum rule: a sum of `terms` values each at most k·δ^n is at most terms·k·δ^n.
    fn sum_of(self, terms: u32, bits: u32) -> Cell {
        Cell {
            k: terms * self.k,
            ..self
        }
        .with_bits(bits)
    }

    fn with_bits(self, bits: u32) -> Cell {
        Cell { bits, ..self }
    }

    /// Whether 2^bits is the smallest power of two above k·δ^n: the bound holds and is not loose.
    fn tight(self) -> bool {
        let v = self.value();
        v < (IBig::ONE << self.bits as usize) && v >= (IBig::ONE << (self.bits - 1) as usize)
    }
}

/// One row of the header's table.
#[derive(Debug)]
struct Budget {
    name: String,
    difference: Cell,
    lift: Option<Cell>,
    minor: Option<Cell>,
    term: Cell,
    result: Cell,
    /// The type's limit: it holds every magnitude below 2 to this power.
    type_bits: u32,
    headroom: u32,
}

/// A cell `kδⁿ < 2^b`, or `—` for none.
fn parse_cell(text: &str) -> Option<Cell> {
    let text = text.trim();
    if text == "—" {
        return None;
    }
    let (left, right) = text
        .split_once(" < 2^")
        .unwrap_or_else(|| panic!("the cell `{text}` is not `kδⁿ < 2^b`"));
    let bits = right.parse().unwrap();
    let digits: String = left.chars().take_while(char::is_ascii_digit).collect();
    let k = if digits.is_empty() {
        1
    } else {
        digits.parse().unwrap()
    };
    let mut rest = left[digits.len()..].chars();
    assert_eq!(rest.next(), Some('δ'), "the cell `{text}`");
    let n = match rest.next() {
        None => 1,
        Some('²') => 2,
        Some('³') => 3,
        Some('⁴') => 4,
        Some('⁵') => 5,
        Some(other) => panic!("the cell `{text}` has the power `{other}`"),
    };
    assert!(rest.next().is_none(), "the cell `{text}`");
    Some(Cell { k, n, bits })
}

/// The table in the section `# Bit budgets` of `predicate.rs`'s module header.
fn header_budgets() -> Vec<Budget> {
    let source = include_str!("../src/predicate.rs").replace("\r\n", "\n");
    let header: Vec<&str> = source
        .lines()
        .take_while(|line| line.starts_with("//!"))
        .map(|line| line.trim_start_matches("//!").trim())
        .collect();
    let section = header
        .iter()
        .position(|line| *line == "# Bit budgets")
        .expect("predicate.rs's module header has a section `# Bit budgets`");
    let mut lines = header[section..]
        .iter()
        .skip_while(|line| !line.starts_with('|'));
    assert_eq!(
        lines.next().copied(),
        Some("| Predicate | Difference | Lift | Minor | Term | Result | Type | Headroom |"),
        "the table's columns"
    );
    assert!(lines.next().is_some_and(|line| line.starts_with("| ---")));
    lines
        .take_while(|line| line.starts_with('|'))
        .map(|line| {
            let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
            assert_eq!(cells.len(), 8, "the row `{line}`");
            let type_bits = match cells[6] {
                "`i128`" => 127,
                "`I256`" => 255,
                other => panic!("the type `{other}`"),
            };
            let headroom = cells[7]
                .trim_end_matches(" bits")
                .trim_end_matches(" bit")
                .parse()
                .unwrap();
            Budget {
                name: cells[0].to_string(),
                difference: parse_cell(cells[1]).expect("every predicate has differences"),
                lift: parse_cell(cells[2]),
                minor: parse_cell(cells[3]),
                term: parse_cell(cells[4]).expect("every predicate has terms"),
                result: parse_cell(cells[5]).expect("every predicate has a result"),
                type_bits,
                headroom,
            }
        })
        .collect()
}

/// The bound the header states on a predicate's result.
fn result_bound(budgets: &[Budget], name: &str) -> IBig {
    let budget = budgets.iter().find(|b| b.name == name).unwrap();
    budget.result.value()
}

/// The header's table is re-derived from its own rules: each difference at most δ; orient2d's
/// terms are products of two differences and its result the difference of two; orient3d's minors
/// are 2 × 2 determinants of differences, its terms a difference times a minor and its result
/// three terms; incircle's lifts are two squares and its minors 2 × 2 determinants, its terms a
/// lift times a minor and its result three terms; insphere's lifts are three squares and its
/// minors 3 × 3 determinants (three terms, each a difference times a 2 × 2 determinant), its terms
/// a lift times a minor and its result four terms. Every power of two is the smallest above its
/// bound, every result fits its type with the stated headroom, the lifted tests' factors fit
/// `i128` (where they are computed), and the table names exactly the four predicates.
#[test]
fn the_bit_budget_table_in_the_module_header_rederives_from_its_rules() {
    let budgets = header_budgets();
    let names: Vec<&str> = budgets.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["orient2d", "orient3d", "incircle", "insphere"]);
    let difference = Cell {
        k: 1,
        n: 1,
        bits: 41,
    };
    for b in &budgets {
        let name = &b.name;
        assert_eq!(
            b.difference, difference,
            "{name}: a difference is at most δ < 2^41"
        );
        let (lift, minor, term, result) = match name.as_str() {
            "orient2d" => {
                let term = difference.times(difference, b.term.bits);
                (None, None, term, term.sum_of(2, b.result.bits))
            }
            "orient3d" => {
                let minor = difference
                    .times(difference, 0)
                    .sum_of(2, b.minor.unwrap().bits);
                let term = difference.times(minor, b.term.bits);
                (None, Some(minor), term, term.sum_of(3, b.result.bits))
            }
            "incircle" => {
                let lift = difference
                    .times(difference, 0)
                    .sum_of(2, b.lift.unwrap().bits);
                let minor = difference
                    .times(difference, 0)
                    .sum_of(2, b.minor.unwrap().bits);
                let term = lift.times(minor, b.term.bits);
                (Some(lift), Some(minor), term, term.sum_of(3, b.result.bits))
            }
            "insphere" => {
                let lift = difference
                    .times(difference, 0)
                    .sum_of(3, b.lift.unwrap().bits);
                let det2 = difference.times(difference, 0).sum_of(2, 0);
                let minor = difference.times(det2, 0).sum_of(3, b.minor.unwrap().bits);
                let term = lift.times(minor, b.term.bits);
                (Some(lift), Some(minor), term, term.sum_of(4, b.result.bits))
            }
            other => panic!("the table names `{other}`"),
        };
        assert_eq!((b.lift, b.minor), (lift, minor), "{name}: lift and minor");
        assert_eq!(b.term, term, "{name}: each term");
        assert_eq!(b.result, result, "{name}: the result");
        for cell in [
            Some(b.difference),
            b.lift,
            b.minor,
            Some(b.term),
            Some(b.result),
        ]
        .into_iter()
        .flatten()
        {
            assert!(
                cell.tight(),
                "{name}: {cell:?} is not the smallest power of two above"
            );
        }
        assert!(
            b.result.bits <= b.type_bits,
            "{name}: the result fits its type"
        );
        assert_eq!(b.headroom, b.type_bits - b.result.bits, "{name}: headroom");
        // Every term fits the result's type, and in the lifted tests the factors fit `i128`, so
        // each product of two of them fits I256 (|x · y| < 2^254).
        assert!(b.term.bits <= b.type_bits, "{name}: each term fits");
        for factor in [b.lift, b.minor].into_iter().flatten() {
            assert!(factor.bits <= 127, "{name}: {factor:?} fits i128");
        }
    }
    // The types the done-when names: orientation in i128, circle and sphere tests in 256 bits.
    let type_of = |name: &str| budgets.iter().find(|b| b.name == name).unwrap().type_bits;
    assert_eq!(
        [
            type_of("orient2d"),
            type_of("orient3d"),
            type_of("incircle"),
            type_of("insphere")
        ],
        [127, 127, 255, 255]
    );
}

// ---------------------------------------------------------------------------------------------
// The sign corners.
// ---------------------------------------------------------------------------------------------

/// The configuration whose base-`levels.len()` digits, least significant first, pick each
/// coordinate from `levels`.
fn configuration<const D: usize, const N: usize>(levels: &[i64], code: u64) -> [[i64; D]; N] {
    let base = levels.len() as u64;
    let mut rest = code;
    array::from_fn(|_| {
        array::from_fn(|_| {
            let level = levels[(rest % base) as usize];
            rest /= base;
            level
        })
    })
}

/// Every configuration with coordinates from `levels` (exactly `expected` of them), each checked
/// with its exact value and against the header's bound on the result. Returns the largest
/// magnitude seen, and the tally.
fn sweep<const D: usize, const N: usize>(
    oracle: &Oracle,
    pred: &Predicate<D, N>,
    levels: &[i64],
    expected: u64,
    bound: &IBig,
) -> (IBig, Tally) {
    let count = (levels.len() as u64).pow((D * N) as u32);
    assert_eq!(
        count, expected,
        "{}: the number of configurations",
        pred.name
    );
    let mut tally = Tally::default();
    let mut largest = IBig::ZERO;
    for code in 0..count {
        let p = configuration::<D, N>(levels, code);
        let exact = oracle.check_value(pred, p, &mut tally);
        let magnitude = if exact < IBig::ZERO { -exact } else { exact };
        assert!(
            magnitude <= *bound,
            "{}{p:?}: |{magnitude}| passes the header's budget",
            pred.name
        );
        largest = largest.max(magnitude);
    }
    (largest, tally)
}

/// Swapping the first two points maps the corner configurations onto themselves and negates
/// every value, so the two non-zero signs occur equally often; and both occur, and so does zero.
fn assert_balanced(name: &str, tally: &Tally) {
    assert_eq!(tally.positive, tally.negative, "{name}: {tally:?}");
    assert!(tally.positive > 0 && tally.zero > 0, "{name}: {tally:?}");
}

/// Every sign corner of every predicate (chapter 28.6: "every sign combination (4,096 for
/// orient3d's twelve coordinates)"; chapter 6.2: every coordinate at ±(2^40 − 1)): the public
/// predicate, robust and the filter against the oracle, the generated exact value equal to it, no
/// value past the header's budget, and the largest orient2d and orient3d values exactly the 4M²
/// and 16M³ the header documents. Every corner of a square lies on one circle and every corner of
/// a cube on one sphere, so every incircle and insphere corner configuration is exactly `Zero`:
/// the documented degenerate case at the largest scale, which one step inside the corners
/// (`one_step_inside_the_corners`) breaks.
#[test]
fn every_sign_corner_against_the_oracle_robust_and_the_written_budget() {
    let oracle = Oracle::new();
    let budgets = header_budgets();
    let corners = [-M, M];
    let m = IBig::from(M);

    let (largest, tally) = sweep(
        &oracle,
        &ORIENT2D,
        &corners,
        64,
        &result_bound(&budgets, "orient2d"),
    );
    assert_eq!(
        largest,
        IBig::from(4) * &m * &m,
        "orient2d's largest value is 4M²"
    );
    assert_balanced("orient2d", &tally);

    let (largest, tally) = sweep(
        &oracle,
        &ORIENT3D,
        &corners,
        4096,
        &result_bound(&budgets, "orient3d"),
    );
    assert_eq!(
        largest,
        IBig::from(16) * &m * &m * &m,
        "orient3d's largest value is 16M³"
    );
    assert_balanced("orient3d", &tally);

    let (largest, tally) = sweep(
        &oracle,
        &INCIRCLE,
        &corners,
        256,
        &result_bound(&budgets, "incircle"),
    );
    assert!(
        largest.is_zero() && tally.zero == 256,
        "incircle: {tally:?}"
    );

    let (largest, tally) = sweep(
        &oracle,
        &INSPHERE,
        &corners,
        32_768,
        &result_bound(&budgets, "insphere"),
    );
    assert!(
        largest.is_zero() && tally.zero == 32_768,
        "insphere: {tally:?}"
    );
}

/// One step inside the corners, every coordinate in {−M, 1 − M, M − 1, M}: all 4,096
/// configurations of orient2d and all 65,536 of incircle, and random ones of orient3d and
/// insphere, which have too many (4^12 and 4^15) to run in full. Here incircle's corners are no
/// longer all cocircular, so both signs occur.
#[test]
fn one_step_inside_the_corners() {
    let oracle = Oracle::new();
    let budgets = header_budgets();
    let levels = [-M, 1 - M, M - 1, M];
    let (_, tally) = sweep(
        &oracle,
        &ORIENT2D,
        &levels,
        4096,
        &result_bound(&budgets, "orient2d"),
    );
    assert_balanced("orient2d", &tally);
    let (_, tally) = sweep(
        &oracle,
        &INCIRCLE,
        &levels,
        65_536,
        &result_bound(&budgets, "incircle"),
    );
    assert_balanced("incircle", &tally);
    let mut rng = Rng::for_test("one_step_inside_the_corners");
    let mut tally = Tally::default();
    for _ in 0..cases(20_000) {
        let p = configuration::<3, 4>(&levels, rng.next() % (1 << 24));
        oracle.check_value(&ORIENT3D, p, &mut tally);
    }
    for _ in 0..cases(5_000) {
        let p = configuration::<3, 5>(&levels, rng.next() % (1 << 30));
        oracle.check_value(&INSPHERE, p, &mut tally);
    }
}

/// Every k-element subset of 0..n, each in increasing order.
fn subsets(n: usize, k: usize) -> Vec<Vec<usize>> {
    let mut all: Vec<Vec<usize>> = vec![Vec::new()];
    for _ in 0..k {
        let mut longer = Vec::new();
        for set in &all {
            let from = set.last().map_or(0, |&last| last + 1);
            for v in from..n {
                let mut next = set.clone();
                next.push(v);
                longer.push(next);
            }
        }
        all = longer;
    }
    all
}

/// Every point with coordinates in {−1, 0, 1}.
fn unit_cube<const D: usize>() -> Vec<[i64; D]> {
    let count = 3u32.pow(D as u32);
    (0..count)
        .map(|code| {
            let mut rest = code;
            array::from_fn(|_| {
                let level = i64::from(rest % 3) - 1;
                rest /= 3;
                level
            })
        })
        .collect()
}

/// The lifted predicates' values over the points with coordinates in {−M, 0, M}, against the
/// largest the module header states there: 8M⁴ for incircle, at three corners of the bounding
/// square and its centre, and 48M⁵ for insphere, at a regular tetrahedron on alternate corners of
/// the bounding cube and its centre. Each value is homogeneous in the coordinates (degree 4 and 5),
/// so it is M⁴ or M⁵ times the value of the same points of {−1, 0, 1}, which the oracle evaluates
/// for every set of distinct points: all 126 of incircle and all 80,730 of insphere (a repeated
/// point gives zero, and an order changes only the sign). Every set, scaled to the full grid, is
/// then checked: the public predicate and `robust` give the small value's sign, the filter declines
/// or agrees, and the generated exact value is exactly M⁴ or M⁵ times it; the sets that reach the
/// largest value, and the configurations the header names, go through the full oracle too.
#[test]
fn the_largest_lifted_values_over_minus_m_zero_m_are_the_ones_the_header_states() {
    let oracle = Oracle::new();
    let m = IBig::from(M);
    let mut tally = Tally::default();

    fn scaled<const D: usize, const N: usize>(p: [[i64; D]; N]) -> [[i64; D]; N] {
        p.map(|q| times(M, q))
    }
    /// Checks every set of N distinct points of the unit cube scaled by M, and returns the
    /// largest magnitude at unit scale and the sets that reach it.
    fn sweep_scaled<const D: usize, const N: usize>(
        oracle: &Oracle,
        pred: &Predicate<D, N>,
        power: usize,
        tally: &mut Tally,
    ) -> (IBig, Vec<[[i64; D]; N]>) {
        let points = unit_cube::<D>();
        let scale = IBig::from(M).pow(power);
        let (mut largest, mut at) = (IBig::ZERO, Vec::new());
        for set in subsets(points.len(), N) {
            let small: [[i64; D]; N] = array::from_fn(|i| points[set[i]]);
            let value = oracle.exact(pred, small);
            let sign = sign_of(&value);
            let p = scaled(small);
            let name = pred.name;
            assert_eq!((pred.public)(p), Ok(sign), "{name}{p:?}");
            let shewchuk = sign_of_f64((pred.robust)(p));
            let robust = if pred.negate {
                shewchuk.negate()
            } else {
                shewchuk
            };
            assert_eq!(robust, sign, "{name}{p:?}: robust");
            let filtered = (pred.filter)(p);
            if let Some(filtered) = filtered {
                assert_eq!(filtered, sign, "{name}{p:?}: the filter");
            }
            assert_eq!(
                (pred.value)(p),
                Some(&value * &scale),
                "{name}{p:?}: the generated value is M^{power} times the unit one"
            );
            tally.note(sign, filtered);
            let size = magnitude(&value);
            if size > largest {
                (largest, at) = (size, vec![small]);
            } else if size == largest {
                at.push(small);
            }
        }
        (largest, at)
    }

    let (largest, at) = sweep_scaled(&oracle, &INCIRCLE, 4, &mut tally);
    assert_eq!(
        largest,
        IBig::from(8),
        "incircle's largest over {{−M, 0, M}} is 8M⁴"
    );
    for small in at {
        let exact = oracle.check_value(&INCIRCLE, scaled(small), &mut tally);
        assert_eq!(magnitude(&exact), IBig::from(8) * m.pow(4));
    }
    let named = [[-M, -M], [M, -M], [-M, M], [0, 0]];
    assert_eq!(
        oracle.check_value(&INCIRCLE, named, &mut tally),
        IBig::from(8) * m.pow(4),
        "three corners of the square and its centre"
    );

    let (largest, at) = sweep_scaled(&oracle, &INSPHERE, 5, &mut tally);
    assert_eq!(
        largest,
        IBig::from(48),
        "insphere's largest over {{−M, 0, M}} is 48M⁵"
    );
    for small in at {
        let exact = oracle.check_value(&INSPHERE, scaled(small), &mut tally);
        assert_eq!(magnitude(&exact), IBig::from(48) * m.pow(5));
    }
    let named = [[M, M, M], [M, -M, -M], [-M, M, -M], [-M, -M, M], [0, 0, 0]];
    assert_eq!(
        magnitude(&oracle.check_value(&INSPHERE, named, &mut tally)),
        IBig::from(48) * m.pow(5),
        "a regular tetrahedron on alternate corners of the cube and its centre"
    );
    assert!(
        tally.positive > 0 && tally.negative > 0 && tally.zero > 0,
        "{tally:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// Random inputs.
// ---------------------------------------------------------------------------------------------

/// N random points: anywhere, or clustered within 2^k of a random place for a random k, so the
/// differences have every size whatever the absolute coordinates.
fn random<const D: usize, const N: usize>(rng: &mut Rng) -> [[i64; D]; N] {
    if rng.coin() {
        return array::from_fn(|_| array::from_fn(|_| rng.coord()));
    }
    loop {
        let k = rng.range(0, 41);
        let reach = (1i64 << k) - 1;
        let centre: [i64; D] = array::from_fn(|_| rng.coord());
        let p: [[i64; D]; N] = array::from_fn(|_| add(centre, rng.within(reach.min(M))));
        if on_grid(&p) {
            return p;
        }
    }
}

/// Random inputs across the whole grid and at every scale: every predicate against the oracle
/// and robust, and the filter never wrong.
#[test]
fn random_inputs_at_every_scale_against_the_oracle_and_robust() {
    let oracle = Oracle::new();
    let mut rng = Rng::for_test("random_inputs_at_every_scale_against_the_oracle_and_robust");
    let mut tallies: [Tally; 4] = Default::default();
    for _ in 0..cases(100_000) {
        oracle.check(&ORIENT2D, random(&mut rng), &mut tallies[0]);
    }
    for _ in 0..cases(50_000) {
        oracle.check(&ORIENT3D, random(&mut rng), &mut tallies[1]);
    }
    for _ in 0..cases(50_000) {
        oracle.check(&INCIRCLE, random(&mut rng), &mut tallies[2]);
    }
    for _ in 0..cases(10_000) {
        oracle.check(&INSPHERE, random(&mut rng), &mut tallies[3]);
    }
    for tally in &tallies {
        assert!(
            tally.decided > 0 && tally.positive > 0 && tally.negative > 0,
            "{tally:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Exact degeneracies, and one step from them.
// ---------------------------------------------------------------------------------------------

/// A direction with components below 2^k for a random k, not zero.
fn direction<const D: usize>(rng: &mut Rng) -> [i64; D] {
    loop {
        let k = rng.range(0, 20);
        let w: [i64; D] = rng.within((1 << k) - 1);
        if w.iter().any(|&v| v != 0) {
            return w;
        }
    }
}

/// A point of the line through `p` along `w`, on the grid.
fn along<const D: usize>(rng: &mut Rng, p: [i64; D], w: [i64; D]) -> [i64; D] {
    let (lo, hi) = multiples_on_grid(p, w);
    add(p, times(rng.range(lo, hi), w))
}

/// K collinear grid points (possibly coinciding), all on the line through the first along w.
fn collinear<const D: usize, const K: usize>(rng: &mut Rng) -> [[i64; D]; K] {
    let a: [i64; D] = array::from_fn(|_| rng.coord());
    let w = direction(rng);
    array::from_fn(|i| if i == 0 { a } else { along(rng, a, w) })
}

/// K coplanar grid points (possibly collinear or coinciding).
fn coplanar<const K: usize>(rng: &mut Rng) -> [Pt3; K] {
    let a: Pt3 = array::from_fn(|_| rng.coord());
    let (u, v) = (direction(rng), direction(rng));
    array::from_fn(|i| {
        if i == 0 {
            a
        } else {
            let b = along(rng, a, u);
            along(rng, b, v)
        }
    })
}

/// The norms p ≡ 1 (mod 4) of Gaussian primes (each a sum of two squares, Fermat).
const PRIMES_1_MOD_4: [i128; 24] = [
    5, 13, 17, 29, 37, 41, 53, 61, 73, 89, 97, 101, 109, 113, 137, 149, 157, 173, 181, 193, 197,
    229, 233, 241,
];

/// Odd primes, each the norm of exactly 8(p + 1) Lipschitz quaternions (Jacobi's four-square
/// theorem).
const ODD_PRIMES: [i128; 15] = [3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53];

/// The lattices circles and spheres are drawn from.
struct Lattices {
    /// A Gaussian integer x + iy of each norm in [`PRIMES_1_MOD_4`].
    gaussian: Vec<[i128; 2]>,
    /// Every Lipschitz quaternion of each norm in [`ODD_PRIMES`].
    quaternions: Vec<Vec<[i128; 4]>>,
}

impl Lattices {
    fn new() -> Lattices {
        let gaussian = PRIMES_1_MOD_4
            .iter()
            .map(|&p| {
                let x = (1..p)
                    .find(|&x| {
                        let y2 = p - x * x;
                        y2 > 0 && isqrt(y2).pow(2) == y2
                    })
                    .unwrap();
                [x, isqrt(p - x * x)]
            })
            .collect();
        let quaternions = ODD_PRIMES
            .iter()
            .map(|&p| {
                let r = isqrt(p);
                let mut all = Vec::new();
                for a in -r..=r {
                    for b in -r..=r {
                        for c in -r..=r {
                            for d in -r..=r {
                                if a * a + b * b + c * c + d * d == p {
                                    all.push([a, b, c, d]);
                                }
                            }
                        }
                    }
                }
                assert_eq!(all.len() as i128, 8 * (p + 1), "Jacobi's count for {p}");
                all
            })
            .collect();
        Lattices {
            gaussian,
            quaternions,
        }
    }

    /// `wanted` distinct points on one circle about the origin, and a bound on its radius, which
    /// is below 2^`radius_bits`: the Gaussian integers u·Π (x ± iy) of a random product of
    /// Gaussian primes, each conjugated at random, times a random unit u, all scaled by one
    /// random whole number.
    fn circle(&self, rng: &mut Rng, radius_bits: u32, wanted: usize) -> Option<(Vec<Pt2>, i64)> {
        let limit = 1i128 << (2 * radius_bits);
        let mut factors = Vec::new();
        let mut norm = 1i128;
        loop {
            let g = self.gaussian[rng.below(self.gaussian.len())];
            let p = g[0] * g[0] + g[1] * g[1];
            if norm * p > limit {
                break;
            }
            norm *= p;
            factors.push(g);
        }
        if factors.is_empty() {
            return None;
        }
        let radius = isqrt(norm) + 1;
        let scale = rng.range(1, ((1i128 << radius_bits) / radius).max(1) as i64);
        let mut points: Vec<Pt2> = Vec::new();
        for _ in 0..16 * wanted {
            let mut z = [[1, 0], [0, 1], [-1, 0], [0, -1]][rng.below(4)];
            for &[x, y] in &factors {
                let y = if rng.coin() { y } else { -y };
                z = [z[0] * x - z[1] * y, z[0] * y + z[1] * x];
            }
            assert_eq!(
                z[0] * z[0] + z[1] * z[1],
                norm,
                "a Gaussian integer of that norm"
            );
            let p = [
                (z[0] * i128::from(scale)) as i64,
                (z[1] * i128::from(scale)) as i64,
            ];
            if !points.contains(&p) {
                points.push(p);
            }
            if points.len() == wanted {
                return Some((points, (radius * i128::from(scale)) as i64));
            }
        }
        None
    }

    /// A random quaternion whose norm is a random product of odd primes no larger than
    /// 2^`norm_bits`, and that norm.
    fn quaternion(&self, rng: &mut Rng, norm_bits: u32) -> ([i128; 4], i128) {
        let limit = 1i128 << norm_bits;
        let mut q = [1i128, 0, 0, 0];
        let mut norm = 1i128;
        loop {
            let i = rng.below(ODD_PRIMES.len());
            if norm * ODD_PRIMES[i] > limit {
                break;
            }
            norm *= ODD_PRIMES[i];
            let f = self.quaternions[i][rng.below(self.quaternions[i].len())];
            q = quaternion_product(q, f);
        }
        (q, norm)
    }

    /// `wanted` distinct points on one sphere about the origin, and its radius n·scale, where
    /// n ≤ 2^`radius_bits` is a product of odd primes: each point is q·i·q̄, q·j·q̄ or q·k·q̄ (each
    /// of length |q|²) for a quaternion q of norm n made of random factors of those primes.
    fn sphere(&self, rng: &mut Rng, radius_bits: u32, wanted: usize) -> Option<(Vec<Pt3>, i64)> {
        let limit = 1i128 << radius_bits;
        let mut primes = Vec::new();
        let mut norm = 1i128;
        loop {
            let i = rng.below(ODD_PRIMES.len());
            if norm * ODD_PRIMES[i] > limit {
                break;
            }
            norm *= ODD_PRIMES[i];
            primes.push(i);
        }
        if primes.is_empty() {
            return None;
        }
        let scale = rng.range(1, (limit / norm).max(1) as i64);
        let mut points: Vec<Pt3> = Vec::new();
        for _ in 0..16 * wanted {
            let mut q = [1i128, 0, 0, 0];
            for &i in &primes {
                let f = self.quaternions[i][rng.below(self.quaternions[i].len())];
                q = quaternion_product(q, f);
            }
            let v = rotated_axis(q, rng.below(3));
            assert_eq!(
                v.iter().map(|c| c * c).sum::<i128>(),
                norm * norm,
                "|q·i·q̄| = |q|²"
            );
            let p: Pt3 = array::from_fn(|i| (v[i] * i128::from(scale)) as i64);
            if !points.contains(&p) {
                points.push(p);
            }
            if points.len() == wanted {
                return Some((points, (norm * i128::from(scale)) as i64));
            }
        }
        None
    }
}

/// The Hamilton product of two quaternions (w, x, y, z) = w + xi + yj + zk.
fn quaternion_product(p: [i128; 4], q: [i128; 4]) -> [i128; 4] {
    let [a1, b1, c1, d1] = p;
    let [a2, b2, c2, d2] = q;
    [
        a1 * a2 - b1 * b2 - c1 * c2 - d1 * d2,
        a1 * b2 + b1 * a2 + c1 * d2 - d1 * c2,
        a1 * c2 - b1 * d2 + c1 * a2 + d1 * b2,
        a1 * d2 + b1 * c2 - c1 * b2 + d1 * a2,
    ]
}

/// q·e·q̄ for the axis e = i, j or k (axis 0, 1 or 2): a column of the rotation q makes, times
/// |q|², so the three are orthogonal and each has length |q|².
fn rotated_axis([w, x, y, z]: [i128; 4], axis: usize) -> [i128; 3] {
    match axis {
        0 => [
            w * w + x * x - y * y - z * z,
            2 * (x * y + w * z),
            2 * (x * z - w * y),
        ],
        1 => [
            2 * (x * y - w * z),
            w * w - x * x + y * y - z * z,
            2 * (y * z + w * x),
        ],
        _ => [
            2 * (x * z + w * y),
            2 * (y * z - w * x),
            w * w - x * x - y * y + z * z,
        ],
    }
}

/// A centre that keeps a circle or sphere of radius below `radius` on the grid with room for
/// `margin` more steps.
fn centre<const D: usize>(rng: &mut Rng, radius: i64, margin: i64) -> [i64; D] {
    rng.within(M - radius - margin)
}

/// The exact orient3d determinant of four points in `i128`: differences below 2^41, so it is at
/// most 6·2^123 (`predicate.rs`'s budget).
fn orient3d_i128(p: [Pt3; 4]) -> i128 {
    let [a, b, c, d] = p;
    let [u, v, w] = [sub(b, a), sub(c, a), sub(d, a)].map(|r| r.map(i128::from));
    u[0] * (v[1] * w[2] - v[2] * w[1])
        + u[1] * (v[2] * w[0] - v[0] * w[2])
        + u[2] * (v[0] * w[1] - v[1] * w[0])
}

/// Four cocircular grid points, from a Gaussian-integer circle at a random scale up to 2^38.
fn cocircular(rng: &mut Rng, lattices: &Lattices) -> [Pt2; 4] {
    loop {
        let bits = rng.range(3, 38) as u32;
        if let Some((points, radius)) = lattices.circle(rng, bits, 4) {
            let o: Pt2 = centre(rng, radius, 2);
            return rng.shuffle(array::from_fn(|i| add(o, points[i])));
        }
    }
}

/// Five cospherical grid points, the first four not coplanar, from a quaternion sphere at a
/// random scale up to 2^38.
fn cospherical(rng: &mut Rng, lattices: &Lattices) -> [Pt3; 5] {
    loop {
        let bits = rng.range(2, 38) as u32;
        if let Some((points, radius)) = lattices.sphere(rng, bits, 5) {
            let o: Pt3 = centre(rng, radius, 2);
            let p: [Pt3; 5] = rng.shuffle(array::from_fn(|i| add(o, points[i])));
            if orient3d_i128([p[0], p[1], p[2], p[3]]) != 0 {
                return p;
            }
        }
    }
}

/// Four points on a circle in a tilted plane: the Gaussian circle's points (x, y) placed at
/// o + x·u + y·v, where u = q·i·q̄ and v = q·j·q̄ are orthogonal and of equal length.
fn tilted_cocircular(rng: &mut Rng, lattices: &Lattices) -> [Pt3; 4] {
    loop {
        let norm_bits = rng.range(1, 14) as u32;
        let (q, norm) = lattices.quaternion(rng, norm_bits);
        let (u, v) = (rotated_axis(q, 0), rotated_axis(q, 1));
        let bits = (37 - (128 - norm.leading_zeros())).max(2);
        if let Some((points, radius)) = lattices.circle(rng, bits, 4) {
            let reach = (i128::from(radius) * norm * 2) as i64;
            let o: Pt3 = centre(rng, reach, 2);
            let lift = |[x, y]: Pt2| -> Pt3 {
                array::from_fn(|i| o[i] + (i128::from(x) * u[i] + i128::from(y) * v[i]) as i64)
            };
            return rng.shuffle(array::from_fn(|i| lift(points[i])));
        }
    }
}

/// Each point moved by each step in `steps`, one at a time, where that stays on the grid.
fn moved<const D: usize, const N: usize>(
    p: [[i64; D]; N],
    steps: &[[i64; D]],
) -> Vec<[[i64; D]; N]> {
    let mut out = Vec::new();
    for i in 0..N {
        for &s in steps {
            let mut q = p;
            q[i] = add(q[i], s);
            if on_grid(&q) {
                out.push(q);
            }
        }
    }
    out
}

/// The eight steps to a neighbouring grid point in 2D.
const STEPS_2D: [Pt2; 8] = [
    [1, 0],
    [-1, 0],
    [0, 1],
    [0, -1],
    [1, 1],
    [1, -1],
    [-1, 1],
    [-1, -1],
];

/// The six unit steps in 3D.
const STEPS_3D: [Pt3; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];

/// Checks a degenerate configuration, which must be exactly zero, and every configuration one
/// step from it; returns how many of those moved off the degeneracy.
fn degenerate_and_moved<const D: usize, const N: usize>(
    oracle: &Oracle,
    pred: &Predicate<D, N>,
    p: [[i64; D]; N],
    steps: &[[i64; D]],
    tally: &mut Tally,
) -> u64 {
    let exact = oracle.check(pred, p, tally);
    assert!(
        exact.is_zero(),
        "{}{p:?}: the construction is not degenerate",
        pred.name
    );
    let mut off = 0;
    for q in moved(p, steps) {
        off += u64::from(!oracle.check(pred, q, tally).is_zero());
    }
    off
}

/// Exactly degenerate inputs (chapter 28.6: "collinear, coplanar, cocircular ... and then moved
/// one step either way"), each moved by one step in every direction: collinear points for
/// orient2d; coplanar points, and three collinear with any fourth, for orient3d; cocircular
/// points, three collinear with the fourth on their line, and two coinciding, for incircle;
/// cospherical points, four on a tilted circle with any fifth, five in one plane, and three
/// collinear with the fourth on their line, for insphere. Every degenerate configuration is
/// `Zero`, every moved one agrees with the oracle and robust, and the moves leave the degeneracy
/// often (a construction that a move cannot break would test nothing).
#[test]
fn one_step_from_every_degeneracy_against_the_oracle_and_robust() {
    let oracle = Oracle::new();
    let lattices = Lattices::new();
    let mut rng = Rng::for_test("one_step_from_every_degeneracy_against_the_oracle_and_robust");
    let mut tally = Tally::default();
    let mut off = [0u64; 4];
    let bases = cases(2_000);
    for _ in 0..bases {
        let line = collinear::<2, 3>(&mut rng);
        off[0] +=
            degenerate_and_moved(&oracle, &ORIENT2D, rng.shuffle(line), &STEPS_2D, &mut tally);

        let plane = coplanar::<4>(&mut rng);
        off[1] += degenerate_and_moved(
            &oracle,
            &ORIENT3D,
            rng.shuffle(plane),
            &STEPS_3D,
            &mut tally,
        );
        let [a, b, c] = collinear::<3, 3>(&mut rng);
        let d = array::from_fn(|_| rng.coord());
        off[1] += degenerate_and_moved(
            &oracle,
            &ORIENT3D,
            rng.shuffle([a, b, c, d]),
            &STEPS_3D,
            &mut tally,
        );

        let circle = cocircular(&mut rng, &lattices);
        off[2] += degenerate_and_moved(&oracle, &INCIRCLE, circle, &STEPS_2D, &mut tally);
        let on_line = collinear::<2, 4>(&mut rng);
        off[2] += degenerate_and_moved(
            &oracle,
            &INCIRCLE,
            rng.shuffle(on_line),
            &STEPS_2D,
            &mut tally,
        );
        let a: Pt2 = array::from_fn(|_| rng.coord());
        let [c, d]: [Pt2; 2] = array::from_fn(|_| array::from_fn(|_| rng.coord()));
        off[2] += degenerate_and_moved(&oracle, &INCIRCLE, [a, a, c, d], &STEPS_2D, &mut tally);

        let sphere = cospherical(&mut rng, &lattices);
        off[3] += degenerate_and_moved(&oracle, &INSPHERE, sphere, &STEPS_3D, &mut tally);
        let [a, b, c, d] = tilted_cocircular(&mut rng, &lattices);
        let e = array::from_fn(|_| rng.coord());
        off[3] += degenerate_and_moved(&oracle, &INSPHERE, [a, b, c, d, e], &STEPS_3D, &mut tally);
        let flat = coplanar::<5>(&mut rng);
        off[3] +=
            degenerate_and_moved(&oracle, &INSPHERE, rng.shuffle(flat), &STEPS_3D, &mut tally);
        let [a, b, c, d] = collinear::<3, 4>(&mut rng);
        let e = array::from_fn(|_| rng.coord());
        off[3] += degenerate_and_moved(&oracle, &INSPHERE, [a, b, c, d, e], &STEPS_3D, &mut tally);
    }
    // Most moves break each kind of degeneracy.
    let bases = bases as u64;
    assert!(off[0] > 8 * bases, "orient2d: {off:?}");
    assert!(off[1] > 8 * bases, "orient3d: {off:?}");
    assert!(off[2] > 8 * bases, "incircle: {off:?}");
    assert!(off[3] > 8 * bases, "insphere: {off:?}");
    assert!(tally.decided > 0 && tally.declined > 0, "{tally:?}");
}

// ---------------------------------------------------------------------------------------------
// The filter's boundary.
// ---------------------------------------------------------------------------------------------

/// A family of inputs indexed by a whole number, `None` where an input leaves the grid.
type Family<'f, const D: usize, const N: usize> = Box<dyn Fn(i64) -> Option<[[i64; D]; N]> + 'f>;

/// Whether the filter decides the family's input at n; `None` off the grid.
fn decides<const D: usize, const N: usize>(
    pred: &Predicate<D, N>,
    family: &Family<'_, D, N>,
    n: i64,
) -> Option<bool> {
    family(n).map(|p| (pred.filter)(p).is_some())
}

/// Adjacent indices lo, lo + 1 in [lo, hi] at which the filter's decision changes, found by
/// bisection from ends it decides differently; `None` if the ends agree or leave the grid.
fn transition<const D: usize, const N: usize>(
    pred: &Predicate<D, N>,
    family: &Family<'_, D, N>,
    mut lo: i64,
    mut hi: i64,
) -> Option<(i64, i64)> {
    let at_lo = decides(pred, family, lo)?;
    if decides(pred, family, hi)? == at_lo {
        return None;
    }
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if decides(pred, family, mid)? == at_lo {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some((lo, hi))
}

/// The first power of two at which the filter decides the family's input, if it does before the
/// family leaves the grid.
fn first_decided<const D: usize, const N: usize>(
    pred: &Predicate<D, N>,
    family: &Family<'_, D, N>,
) -> Option<i64> {
    let mut n = 1i64;
    loop {
        if decides(pred, family, n)? {
            return Some(n);
        }
        n = n.checked_mul(2)?;
    }
}

/// Half the width of the window checked around each transition.
const WINDOW: i64 = 12;

/// Checks every input of the family within [`WINDOW`] of the transition (lo, lo + 1), with its
/// exact value. Returns whether the window held both a decided and a declined input with a
/// non-zero exact value: a crossing of the bound, not of zero.
fn check_window<const D: usize, const N: usize>(
    oracle: &Oracle,
    pred: &Predicate<D, N>,
    family: &Family<'_, D, N>,
    (lo, hi): (i64, i64),
    tally: &mut Tally,
) -> bool {
    let (mut decided, mut declined) = (false, false);
    for n in lo - WINDOW..=hi + WINDOW {
        if let Some(p) = family(n) {
            let exact = oracle.check_value(pred, p, tally);
            let filtered = (pred.filter)(p).is_some();
            decided |= filtered && !exact.is_zero();
            declined |= !filtered && !exact.is_zero();
        }
    }
    decided && declined
}

/// A random unimodular 3 × 3 integer matrix with small entries (rows e₁, e₂, e₃, determinant ±1):
/// the identity under random shears, row swaps and sign changes.
fn unimodular3(rng: &mut Rng) -> [Pt3; 3] {
    loop {
        let mut e = [[1i64, 0, 0], [0, 1, 0], [0, 0, 1]];
        for _ in 0..8 {
            let (i, j) = (rng.below(3), rng.below(3));
            if i != j {
                let c = rng.range(-2, 2);
                e[i] = add(e[i], times(c, e[j]));
            }
        }
        let e = rng.shuffle(e).map(|r| times(rng.sign(), r));
        if e.iter().flatten().all(|v| v.abs() < 1 << 12) {
            let det = i128::from(e[0][0])
                * (i128::from(e[1][1]) * i128::from(e[2][2])
                    - i128::from(e[1][2]) * i128::from(e[2][1]))
                - i128::from(e[0][1])
                    * (i128::from(e[1][0]) * i128::from(e[2][2])
                        - i128::from(e[1][2]) * i128::from(e[2][0]))
                + i128::from(e[0][2])
                    * (i128::from(e[1][0]) * i128::from(e[2][1])
                        - i128::from(e[1][1]) * i128::from(e[2][0]));
            assert_eq!(det.abs(), 1, "shears keep the determinant ±1");
            return e;
        }
    }
}

/// The largest |component| of a vector.
fn norm_inf<const D: usize>(v: [i64; D]) -> i64 {
    v.iter().map(|c| c.abs()).max().unwrap()
}

/// orient2d's family: a, b = a + m·u and c(j) = a + k·u + j·w, where det(u, w) = 1, so the exact
/// value is m·j, growing by m per step from zero at j = 0, with c at full scale.
fn orient2d_family(rng: &mut Rng) -> Family<'static, 2, 3> {
    let (u, w) = loop {
        let u: Pt2 = rng.within(1 << 12);
        let (g, x, y) = extended_gcd(i128::from(u[0]), i128::from(u[1]));
        if g == 1 {
            // det(u, w) = u.x·x + u.y·y = 1.
            break (u, [-y as i64, x as i64]);
        }
    };
    let m_bits = rng.range(0, 24);
    let m = rng.range(1, 1 << m_bits);
    let big = norm_inf(u);
    let k = rng.sign() * rng.range((1 << 37) / big, (1 << 38) / big);
    let a: Pt2 = rng.within(1 << 38);
    let order = rng.shuffle([0usize, 1, 2]);
    Box::new(move |j| {
        let p = [
            a,
            add(a, times(m, u)),
            add(add(a, times(k, u)), times(j, w)),
        ];
        let p = order.map(|i| p[i]);
        on_grid(&p).then_some(p)
    })
}

/// orient3d's family: with e₁, e₂, e₃ unimodular, a, a + m·e₁, a + k₁e₁ + j·e₂ and
/// a + l₁e₁ + l₂e₂ + i·e₃, whose exact value is ±m·j·i, growing by m·j per step from zero.
fn orient3d_family(rng: &mut Rng) -> Family<'static, 3, 4> {
    let [e1, e2, e3] = unimodular3(rng);
    let m_bits = rng.range(0, 16);
    let m = rng.range(1, 1 << m_bits);
    let (b1, b2) = (norm_inf(e1), norm_inf(e2));
    let k1 = rng.range(-(1 << 36) / b1, (1 << 36) / b1);
    let j = rng.sign() * rng.range((1 << 36) / b2, (1 << 37) / b2);
    let l1 = rng.range(-(1 << 36) / b1, (1 << 36) / b1);
    let l2 = rng.sign() * rng.range((1 << 36) / b2, (1 << 37) / b2);
    let a: Pt3 = rng.within(1 << 37);
    let order = rng.shuffle([0usize, 1, 2, 3]);
    Box::new(move |i| {
        let b = add(a, times(m, e1));
        let c = add(a, add(times(k1, e1), times(j, e2)));
        let d = add(a, add(add(times(l1, e1), times(l2, e2)), times(i, e3)));
        let p = [a, b, c, d];
        let p = order.map(|n| p[n]);
        on_grid(&p).then_some(p)
    })
}

/// A primitive Pythagorean triple (p, q, h), p² + q² = h², by Euclid's formula.
fn pythagorean_triple(rng: &mut Rng) -> [i64; 3] {
    loop {
        let s = rng.range(2, 1 << 7);
        let t = rng.range(1, s - 1);
        if gcd(i128::from(s), i128::from(t)) == 1 && (s - t) % 2 == 1 {
            return [s * s - t * t, 2 * s * t, s * s + t * t];
        }
    }
}

/// The inward offset K of the inside families, in steps: enough that the exact value starts far
/// above the filter's bound, while the smallest value the family reaches stays far below it.
const INWARD: i64 = 1 << 10;

/// incircle's two families on one circle of radius m·h ≈ 2^38 about o, through three of its
/// lattice points a, b, c, the fourth point moving along a tangent. Outside: d(n) = r + n·t, with
/// r = o + m·g·r̂ a fourth lattice point (r̂ primitive, g its multiple) and t ⊥ r̂, whose power with
/// respect to the circle is n²|t|², so the exact value grows from zero. Inside: from the axis point
/// s = o + m·h·x̂, d(n) = s − K·x̂ + n·ŷ (K = [`INWARD`]), whose power n² − 2Kmh + K² is negative
/// and shrinks in magnitude as n grows to ⌊√(2Kmh − K²)⌋, where it is at most 2n + 1. Returns the
/// families and that last index.
fn incircle_families(rng: &mut Rng) -> (Family<'static, 2, 4>, Family<'static, 2, 4>, i64) {
    let [p, q, h] = pythagorean_triple(rng);
    let m = rng.range((1 << 37) / h, (1 << 38) / h);
    let o: Pt2 = centre(rng, m * h, 1 << 24);
    let axes: [Pt2; 4] = [[1, 0], [-1, 0], [0, 1], [0, -1]];
    let x = axes[rng.below(4)];
    let mut dirs: Vec<Pt2> = vec![
        [p, q],
        [-p, q],
        [p, -q],
        [-p, -q],
        [q, p],
        [-q, p],
        [q, -p],
        [-q, -p],
    ];
    dirs.extend(axes.iter().filter(|&&a| a != x).map(|&a| times(h, a)));
    let mut pick = [[0i64; 2]; 4];
    for slot in &mut pick {
        let i = rng.below(dirs.len());
        *slot = dirs.swap_remove(i);
    }
    let [a, b, c] = [0, 1, 2].map(|i| add(o, times(m, pick[i])));
    let r = pick[3];
    let g = gcd(i128::from(r[0]), i128::from(r[1])) as i64;
    let unit = [r[0] / g, r[1] / g];
    let t = [-unit[1], unit[0]];
    let at = add(o, times(m, r));
    let order = rng.shuffle([0usize, 1, 2, 3]);
    let outside: Family<'static, 2, 4> = Box::new(move |n| {
        let p = [a, b, c, add(at, times(n, t))];
        let p = order.map(|i| p[i]);
        on_grid(&p).then_some(p)
    });
    let y = [-x[1], x[0]];
    let start = add(o, times(m * h - INWARD, x));
    let inside: Family<'static, 2, 4> = Box::new(move |n| {
        let p = [a, b, c, add(start, times(n, y))];
        let p = order.map(|i| p[i]);
        on_grid(&p).then_some(p)
    });
    let radius = i128::from(m * h);
    let last = isqrt(2 * i128::from(INWARD) * radius - i128::from(INWARD).pow(2)) as i64;
    (outside, inside, last)
}

/// A primitive Pythagorean quadruple (p, q, r, s), p² + q² + r² = s², from a quaternion
/// (Euler's four-square identity).
fn pythagorean_quadruple(rng: &mut Rng) -> [i64; 4] {
    loop {
        let [a, b, c, d]: [i64; 4] = rng.within(1 << 5);
        let v = [
            a * a + b * b - c * c - d * d,
            2 * (a * d + b * c),
            2 * (b * d - a * c),
        ];
        let s = a * a + b * b + c * c + d * d;
        let g = gcd(gcd(i128::from(v[0]), i128::from(v[1])), i128::from(v[2])) as i64;
        if s > 0 && g > 0 {
            let v = v.map(|x| x / g);
            assert_eq!(v.iter().map(|x| x * x).sum::<i64>(), (s / g) * (s / g));
            return [v[0], v[1], v[2], s / g];
        }
    }
}

/// insphere's two families, as [`incircle_families`] in 3D: a sphere of radius m·s ≈ 2^37 about
/// o through four of its lattice points a, b, c, d (not coplanar), the fifth point moving along a
/// tangent. Outside: from a fifth lattice point r = o + m·g·r̂, e(n) = r + n·t with t ⊥ r̂, power
/// n²|t|². Inside: from the axis point o + m·s·x̂, e(n) = o + (ms − K)·x̂ + n·ŷ, power
/// n² − 2Kms + K². Returns the families and the inside family's last index, √(2Kms − K²).
fn insphere_families(rng: &mut Rng) -> (Family<'static, 3, 5>, Family<'static, 3, 5>, i64) {
    loop {
        let [p, q, r, s] = pythagorean_quadruple(rng);
        let mut axes: Vec<Pt3> = Vec::new();
        for axis in 0..3 {
            for sign in [-1, 1] {
                let mut v = [0; 3];
                v[axis] = sign;
                axes.push(v);
            }
        }
        let x = axes[rng.below(axes.len())];
        let mut dirs: Vec<Pt3> = Vec::new();
        for (order, _) in orderings(3) {
            for mask in 0..8 {
                let base = [p, q, r];
                let v: Pt3 = array::from_fn(|i| {
                    let c = base[order[i]];
                    if mask >> i & 1 == 1 { -c } else { c }
                });
                if !dirs.contains(&v) && v != times(s, x) {
                    dirs.push(v);
                }
            }
        }
        dirs.extend(axes.iter().filter(|&&a| a != x).map(|&a| times(s, a)));
        dirs.dedup();
        if dirs.len() < 5 {
            continue;
        }
        let m = rng.range((1 << 36) / s, (1 << 37) / s);
        let o: Pt3 = centre(rng, m * s, 1 << 24);
        let mut picks = Vec::new();
        while picks.len() < 5 {
            let v = dirs[rng.below(dirs.len())];
            if !picks.contains(&v) {
                picks.push(v);
            }
        }
        let [a, b, c, d] = [0, 1, 2, 3].map(|i| add(o, times(m, picks[i])));
        if orient3d_i128([a, b, c, d]) == 0 {
            continue;
        }
        let rd = picks[4];
        let g = gcd(gcd(i128::from(rd[0]), i128::from(rd[1])), i128::from(rd[2])) as i64;
        let unit = rd.map(|v| v / g);
        // t = unit × the axis along which unit is smallest, made primitive: never zero.
        let smallest = (0..3).min_by_key(|&i| unit[i].abs()).unwrap();
        let mut e = [0i64; 3];
        e[smallest] = 1;
        let t = [
            unit[1] * e[2] - unit[2] * e[1],
            unit[2] * e[0] - unit[0] * e[2],
            unit[0] * e[1] - unit[1] * e[0],
        ];
        let gt = gcd(gcd(i128::from(t[0]), i128::from(t[1])), i128::from(t[2])) as i64;
        let t = t.map(|v| v / gt);
        let at = add(o, times(m, rd));
        let order = rng.shuffle([0usize, 1, 2, 3, 4]);
        let outside: Family<'static, 3, 5> = Box::new(move |n| {
            let pts = [a, b, c, d, add(at, times(n, t))];
            let pts = order.map(|i| pts[i]);
            on_grid(&pts).then_some(pts)
        });
        // ŷ: a unit axis perpendicular to x̂.
        let y = [x[2].abs(), x[0].abs(), x[1].abs()];
        let start = add(o, times(m * s - INWARD, x));
        let inside: Family<'static, 3, 5> = Box::new(move |n| {
            let pts = [a, b, c, d, add(start, times(n, y))];
            let pts = order.map(|i| pts[i]);
            on_grid(&pts).then_some(pts)
        });
        let radius = i128::from(m * s);
        let last = isqrt(2 * i128::from(INWARD) * radius - i128::from(INWARD).pow(2)) as i64;
        return (outside, inside, last);
    }
}

/// What the boundary search found for one predicate.
#[derive(Debug, Default)]
struct Crossings {
    tried: u64,
    crossed: u64,
}

/// Searches one family for the filter's transition between `lo` and `hi` and checks the window
/// around it.
fn cross<const D: usize, const N: usize>(
    oracle: &Oracle,
    pred: &Predicate<D, N>,
    family: &Family<'_, D, N>,
    ends: Option<(i64, i64)>,
    crossings: &mut Crossings,
    tally: &mut Tally,
) {
    crossings.tried += 1;
    if let Some((lo, hi)) = ends {
        if let Some(found) = transition(pred, family, lo, hi) {
            if check_window(oracle, pred, family, found, tally) {
                crossings.crossed += 1;
            }
        }
    }
}

/// Inputs placed just inside and just outside each filter's error bound (chapter 28.6), found by
/// searching families whose exact value moves in small steps for the step where the filter's
/// decision changes, and checked in a window around it: orient2d and orient3d from exact zero
/// outwards on lattice constructions with a determinant ±1 at full scale; incircle and insphere
/// along a tangent of a circle or sphere through lattice points, outward from a point on it, and
/// inward from one step inside it. Each crossing found has, near the bound, inputs the filter
/// decides and inputs it declines, all with non-zero exact values, and every one agrees with the
/// oracle and robust; most families cross.
#[test]
fn every_filter_boundary_crossed_both_ways_against_the_oracle_and_robust() {
    let oracle = Oracle::new();
    let mut rng =
        Rng::for_test("every_filter_boundary_crossed_both_ways_against_the_oracle_and_robust");
    let instances = cases(400);
    let mut tally = Tally::default();
    let mut found: [Crossings; 6] = Default::default();
    for _ in 0..instances {
        let family = orient2d_family(&mut rng);
        let ends = first_decided(&ORIENT2D, &family).map(|hi| (0, hi));
        cross(&oracle, &ORIENT2D, &family, ends, &mut found[0], &mut tally);
        let ends = first_decided(&ORIENT2D, &family).map(|hi| (-hi, 0));
        let mirrored: Family<'_, 2, 3> = Box::new(|n| family(-n));
        let ends = ends.map(|(lo, _)| (0, -lo));
        cross(
            &oracle,
            &ORIENT2D,
            &mirrored,
            ends,
            &mut found[0],
            &mut tally,
        );

        let family = orient3d_family(&mut rng);
        let ends = first_decided(&ORIENT3D, &family).map(|hi| (0, hi));
        cross(&oracle, &ORIENT3D, &family, ends, &mut found[1], &mut tally);

        let (outside, inside, last) = incircle_families(&mut rng);
        let ends = first_decided(&INCIRCLE, &outside).map(|hi| (0, hi));
        cross(
            &oracle,
            &INCIRCLE,
            &outside,
            ends,
            &mut found[2],
            &mut tally,
        );
        cross(
            &oracle,
            &INCIRCLE,
            &inside,
            Some((0, last)),
            &mut found[3],
            &mut tally,
        );

        let (outside, inside, last) = insphere_families(&mut rng);
        let ends = first_decided(&INSPHERE, &outside).map(|hi| (0, hi));
        cross(
            &oracle,
            &INSPHERE,
            &outside,
            ends,
            &mut found[4],
            &mut tally,
        );
        cross(
            &oracle,
            &INSPHERE,
            &inside,
            Some((0, last)),
            &mut found[5],
            &mut tally,
        );
    }
    println!("crossings, per family: {found:?}; checks: {tally:?}");
    for (i, crossings) in found.iter().enumerate() {
        assert!(
            crossings.crossed * 2 > crossings.tried,
            "family {i}: only {crossings:?} crossed the filter's bound"
        );
    }
    assert!(tally.decided > 0 && tally.declined > 0, "{tally:?}");
}

// ---------------------------------------------------------------------------------------------
// The laws.
// ---------------------------------------------------------------------------------------------

/// The public predicate's sign, which must be `Ok`.
fn answer<const D: usize, const N: usize>(pred: &Predicate<D, N>, p: [[i64; D]; N]) -> Sign {
    (pred.public)(p).unwrap_or_else(|e| panic!("{}{p:?}: {e:?}", pred.name))
}

/// `s` negated when `odd`.
fn by_parity(s: Sign, odd: bool) -> Sign {
    if odd { s.negate() } else { s }
}

/// Checks every law on one input: every ordering of the points multiplies the sign by its parity
/// (so every transposition flips it); translating every point by one grid vector, at random and
/// to every corner of the grid, keeps it; every signed permutation of the axes multiplies it by
/// its determinant; and scaling by 2, 3 or 1000 keeps it, where the result stays on the grid.
fn check_laws<const D: usize, const N: usize>(
    pred: &Predicate<D, N>,
    p: [[i64; D]; N],
    point_orders: &Orderings,
    axis_orders: &Orderings,
    rng: &mut Rng,
) {
    let name = pred.name;
    let s = answer(pred, p);
    for (order, odd) in point_orders {
        let q: [[i64; D]; N] = array::from_fn(|i| p[order[i]]);
        assert_eq!(
            answer(pred, q),
            by_parity(s, *odd),
            "{name}{p:?} reordered as {order:?}"
        );
    }
    // Translations: each axis's room on the grid, at random and all the way to either edge.
    let low: [i64; D] = array::from_fn(|i| p.iter().map(|q| q[i]).min().unwrap());
    let high: [i64; D] = array::from_fn(|i| p.iter().map(|q| q[i]).max().unwrap());
    let mut shifts: Vec<[i64; D]> = vec![array::from_fn(|i| rng.range(-M - low[i], M - high[i]))];
    for mask in 0..1 << D {
        shifts.push(array::from_fn(|i| {
            if mask >> i & 1 == 1 {
                M - high[i]
            } else {
                -M - low[i]
            }
        }));
    }
    for t in shifts {
        let q = p.map(|r| add(r, t));
        assert!(on_grid(&q));
        assert_eq!(answer(pred, q), s, "{name}{p:?} translated by {t:?}");
    }
    // Signed permutations of the axes: the sign times the map's determinant.
    for (order, odd) in axis_orders {
        for mask in 0..1u32 << D {
            let q = p.map(|r| -> [i64; D] {
                array::from_fn(|i| {
                    if mask >> i & 1 == 1 {
                        -r[order[i]]
                    } else {
                        r[order[i]]
                    }
                })
            });
            let flips = *odd ^ (mask.count_ones() % 2 == 1);
            assert_eq!(
                answer(pred, q),
                by_parity(s, flips),
                "{name}{p:?} mapped by {order:?}, {mask:b}"
            );
        }
    }
    for k in [2, 3, 1000] {
        let q = p.map(|r| r.map(|v| v.saturating_mul(k)));
        if on_grid(&q) {
            assert_eq!(answer(pred, q), s, "{name}{p:?} scaled by {k}");
        }
    }
}

/// The laws (chapter 28.6: "Swapping two points flips the sign; even permutations keep it; moving
/// every point by one grid vector changes nothing"), and the axis maps and scalings beyond them,
/// on the sign corners, random points, exact degeneracies and one step from them
/// (`laws_hold_on_both_sides_of_every_filter_boundary` takes the inputs at the filter's boundary).
#[test]
fn antisymmetry_permutation_and_translation_laws_on_every_input_class() {
    let lattices = Lattices::new();
    let mut rng =
        Rng::for_test("antisymmetry_permutation_and_translation_laws_on_every_input_class");
    let axes2 = orderings(2);
    let axes3 = orderings(3);
    let (o3, o4, o5) = (orderings(3), orderings(4), orderings(5));
    let corners = [-M, M];

    // Sign corners: every one in 2D, every eighth of orient3d's and every 256th of insphere's.
    for code in 0..64 {
        check_laws(
            &ORIENT2D,
            configuration(&corners, code),
            &o3,
            &axes2,
            &mut rng,
        );
    }
    for code in (0..4096).step_by(8) {
        check_laws(
            &ORIENT3D,
            configuration(&corners, code),
            &o4,
            &axes3,
            &mut rng,
        );
    }
    for code in 0..256 {
        check_laws(
            &INCIRCLE,
            configuration(&corners, code),
            &o4,
            &axes2,
            &mut rng,
        );
    }
    for code in (0..32_768).step_by(256) {
        check_laws(
            &INSPHERE,
            configuration(&corners, code),
            &o5,
            &axes3,
            &mut rng,
        );
    }
    for _ in 0..cases(1_000) {
        // Random points.
        check_laws(&ORIENT2D, random(&mut rng), &o3, &axes2, &mut rng);
        check_laws(&ORIENT3D, random(&mut rng), &o4, &axes3, &mut rng);
        check_laws(&INCIRCLE, random(&mut rng), &o4, &axes2, &mut rng);
        check_laws(&INSPHERE, random(&mut rng), &o5, &axes3, &mut rng);
        // Exact degeneracies, and one step from them.
        let line = collinear::<2, 3>(&mut rng);
        check_laws(&ORIENT2D, line, &o3, &axes2, &mut rng);
        if let Some(&q) = moved(line, &STEPS_2D).get(rng.below(24)) {
            check_laws(&ORIENT2D, q, &o3, &axes2, &mut rng);
        }
        let plane = coplanar::<4>(&mut rng);
        check_laws(&ORIENT3D, plane, &o4, &axes3, &mut rng);
        if let Some(&q) = moved(plane, &STEPS_3D).get(rng.below(24)) {
            check_laws(&ORIENT3D, q, &o4, &axes3, &mut rng);
        }
        let circle = cocircular(&mut rng, &lattices);
        check_laws(&INCIRCLE, circle, &o4, &axes2, &mut rng);
        if let Some(&q) = moved(circle, &STEPS_2D).get(rng.below(32)) {
            check_laws(&INCIRCLE, q, &o4, &axes2, &mut rng);
        }
        let sphere = cospherical(&mut rng, &lattices);
        check_laws(&INSPHERE, sphere, &o5, &axes3, &mut rng);
        if let Some(&q) = moved(sphere, &STEPS_3D).get(rng.below(30)) {
            check_laws(&INSPHERE, q, &o5, &axes3, &mut rng);
        }
    }
}

/// The laws of `antisymmetry_permutation_and_translation_laws_on_every_input_class` on the inputs
/// at each filter's boundary: both sides of every transition the boundary search finds.
#[test]
fn laws_hold_on_both_sides_of_every_filter_boundary() {
    let mut rng = Rng::for_test("laws_hold_on_both_sides_of_every_filter_boundary");
    let axes2 = orderings(2);
    let axes3 = orderings(3);
    let (o3, o4, o5) = (orderings(3), orderings(4), orderings(5));
    for _ in 0..cases(100) {
        let family = orient2d_family(&mut rng);
        if let Some((lo, hi)) =
            first_decided(&ORIENT2D, &family).and_then(|hi| transition(&ORIENT2D, &family, 0, hi))
        {
            for n in [lo, hi] {
                check_laws(&ORIENT2D, family(n).unwrap(), &o3, &axes2, &mut rng);
            }
        }
        let family = orient3d_family(&mut rng);
        if let Some((lo, hi)) =
            first_decided(&ORIENT3D, &family).and_then(|hi| transition(&ORIENT3D, &family, 0, hi))
        {
            for n in [lo, hi] {
                check_laws(&ORIENT3D, family(n).unwrap(), &o4, &axes3, &mut rng);
            }
        }
        let (outside, inside, last) = incircle_families(&mut rng);
        if let Some((lo, hi)) =
            first_decided(&INCIRCLE, &outside).and_then(|hi| transition(&INCIRCLE, &outside, 0, hi))
        {
            for n in [lo, hi] {
                check_laws(&INCIRCLE, outside(n).unwrap(), &o4, &axes2, &mut rng);
            }
        }
        if let Some((lo, hi)) = transition(&INCIRCLE, &inside, 0, last) {
            for n in [lo, hi] {
                check_laws(&INCIRCLE, inside(n).unwrap(), &o4, &axes2, &mut rng);
            }
        }
        let (outside, inside, last) = insphere_families(&mut rng);
        if let Some((lo, hi)) =
            first_decided(&INSPHERE, &outside).and_then(|hi| transition(&INSPHERE, &outside, 0, hi))
        {
            for n in [lo, hi] {
                check_laws(&INSPHERE, outside(n).unwrap(), &o5, &axes3, &mut rng);
            }
        }
        if let Some((lo, hi)) = transition(&INSPHERE, &inside, 0, last) {
            for n in [lo, hi] {
                check_laws(&INSPHERE, inside(n).unwrap(), &o5, &axes3, &mut rng);
            }
        }
    }
}
