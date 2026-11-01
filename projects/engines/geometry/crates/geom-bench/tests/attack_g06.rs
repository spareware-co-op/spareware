//! Adversarial tests for piece G06 (`geom-bench`, issue #175): the benchmark harness and the gate
//! that blocks a speed regression over 5%.
//!
//! The builder's own tests check that the catalogue, the gate and the golden digests agree with one
//! another. These check the piece against things outside it:
//!
//! - **Exact oracles.** Every benchmark batch is run through `geom-num` and each answer is compared
//!   with the exact value computed here from scratch: a sign-and-magnitude big integer, Laplace
//!   determinants, Cramer's rule, and exact rationals for doubles. None of it shares code with
//!   `geom-num`.
//! - **Wiring.** Every catalogued digest is recomputed by calling the `geom-num` operation directly
//!   on the same inputs. `tests/golden.rs` pins whatever the kernels returned when the digests were
//!   recorded, so a catalogue entry bound to the wrong kernel would be pinned too; this would not.
//! - **The output hash.** Chapter 23.13: no benchmark may get faster by being wrong. Wrong answers
//!   are built here and compared with the right ones through the digest the gates check.
//! - **Determinism.** The same digests at 1, 2, 3 and 8 threads.
//! - **The input classes.** The claims the input builders' documentation makes.
//! - **The chapters.** What chapter 23.13, chapter 28.14 and board entry G18 (which G06 covers in
//!   full, chapter 30.0) require of the gate, read from the gate's source and the CI workflow.
//!
//! Guarantees, when these tests pass: every benchmark's results are those of the direct `geom-num`
//! call on its inputs; every result except the maths functions' agrees with the independent
//! oracle here (equal to the exact value, or for intervals enclosing it within two steps, except
//! `Interval::norm`, whose roundings compound as `geom-num` documents: its length encloses the
//! exact one and is, bit for bit, its documented composition of operations checked here), and so
//! does the fused multiply-add, which IEEE 754 rounds correctly; the certified enclosures of the
//! maths functions find every peak and trough exactly (decided here in exact rationals), refuse
//! only where documented, and hold the correctly rounded values at their ends; the digests change when any
//! result does and do not depend on thread count or order; and the gate and workflow have each
//! property listed above.
//!
//! Methods: schoolbook arithmetic on magnitudes and restoring binary long division (Knuth, *The Art
//! of Computer Programming*, volume 2, section 4.3.1,
//! <https://www-cs-faculty.stanford.edu/~knuth/taocp.html>); determinants by Laplace expansion
//! (<https://en.wikipedia.org/wiki/Laplace_expansion>); Cramer's rule for the point where three
//! planes meet (<https://en.wikipedia.org/wiki/Cramer%27s_rule>); doubles as exact multiples of
//! 2^−1074 (IEEE 754-2019, <https://doi.org/10.1109/IEEESTD.2019.8766229>).
//!
//! Cost: exact big-integer and rational arithmetic on every input of every batch, and every
//! benchmark run again at 1, 2, 3 and 8 threads.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code, including its oracle helpers: a failure should stop loudly"
)]

use core::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use geom_bench::canonical::{self, Canonical};
use geom_bench::catalogue::{Case, cases, growth_cases};
use geom_bench::golden;
use geom_bench::harness::Digests;
use geom_bench::inputs::Class::{
    Degenerate, Extreme, NearlyDegenerate, OnGrid, Overflow, Typical, WrongDirection,
};
use geom_bench::inputs::{self, BATCH, Class};
use geom_bench::kernels;
use geom_bench::rng::{SplitMix64, fnv1a, seed_for};
use geom_bench::sha256::{Digest, Sha256};
use geom_num::crossing::Ssi;
use geom_num::dd::{self as dd, DD};
use geom_num::exact_sum::ExactSum;
use geom_num::grid::{self, COORD_MAX, Coord, GridStep, P2, P3, Rounded, Steps};
use geom_num::interval::Interval;
use geom_num::plane::{Lpi, Plane, Tpi};
use geom_num::real::Real;
use geom_num::rng::{self as rng, Philox, RngError, Seed, Stream, StreamId, StreamRange};
use geom_num::wide::Wide;
use geom_num::wide32::Wide32;
use geom_num::{Exactness, NumError, Sign, eft, math, predicate};

const GATE: &str = include_str!("../benches/instructions/gate.rs");
const KERNELS: &str = include_str!("../src/kernels.rs");
const WORKFLOW: &str = include_str!("../../../../../../.github/workflows/kernel-bench.yml");
const KERNEL_WORKFLOW: &str = include_str!("../../../../../../.github/workflows/kernel.yml");

// ------------------------------------------------------------------------------------------------
// An exact big integer, and exact rationals, for the oracles.
// ------------------------------------------------------------------------------------------------

/// A signed integer of any size: a sign and a magnitude in little-endian 32-bit limbs with no high
/// zero limbs, so zero has no limbs and is never negative.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Big {
    negative: bool,
    mag: Vec<u32>,
}

impl Big {
    fn new(negative: bool, mut mag: Vec<u32>) -> Big {
        while mag.last() == Some(&0) {
            mag.pop();
        }
        Big {
            negative: negative && !mag.is_empty(),
            mag,
        }
    }

    fn zero() -> Big {
        Big::new(false, Vec::new())
    }

    fn one() -> Big {
        Big::from_u128(1)
    }

    fn from_u128(mut v: u128) -> Big {
        let mut mag = Vec::new();
        while v != 0 {
            mag.push(v as u32);
            v >>= 32;
        }
        Big::new(false, mag)
    }

    fn from_i128(v: i128) -> Big {
        Big::new(v < 0, Big::from_u128(v.unsigned_abs()).mag)
    }

    fn from_i64(v: i64) -> Big {
        Big::from_i128(i128::from(v))
    }

    fn pow2(k: u32) -> Big {
        Big::one().shifted(k)
    }

    fn sign(&self) -> Sign {
        if self.mag.is_empty() {
            Sign::Zero
        } else if self.negative {
            Sign::Negative
        } else {
            Sign::Positive
        }
    }

    fn is_zero(&self) -> bool {
        self.mag.is_empty()
    }

    fn negated(&self) -> Big {
        Big::new(!self.negative, self.mag.clone())
    }

    fn magnitude(&self) -> Big {
        Big::new(false, self.mag.clone())
    }

    fn cmp_mag(a: &[u32], b: &[u32]) -> Ordering {
        a.len()
            .cmp(&b.len())
            .then_with(|| a.iter().rev().cmp(b.iter().rev()))
    }

    fn add_mag(a: &[u32], b: &[u32]) -> Vec<u32> {
        let n = a.len().max(b.len());
        let mut out = Vec::with_capacity(n + 1);
        let mut carry = 0u64;
        for i in 0..n {
            let x = u64::from(a.get(i).copied().unwrap_or(0));
            let y = u64::from(b.get(i).copied().unwrap_or(0));
            let s = x + y + carry;
            out.push(s as u32);
            carry = s >> 32;
        }
        out.push(carry as u32);
        out
    }

    /// |a| − |b|, for |a| ≥ |b|.
    fn sub_mag(a: &[u32], b: &[u32]) -> Vec<u32> {
        let mut out = Vec::with_capacity(a.len());
        let mut borrow = 0u64;
        for (i, &x) in a.iter().enumerate() {
            let x = u64::from(x);
            let y = u64::from(b.get(i).copied().unwrap_or(0)) + borrow;
            if x >= y {
                out.push((x - y) as u32);
                borrow = 0;
            } else {
                out.push((x + (1 << 32) - y) as u32);
                borrow = 1;
            }
        }
        out
    }

    fn plus(&self, other: &Big) -> Big {
        if self.negative == other.negative {
            return Big::new(self.negative, Big::add_mag(&self.mag, &other.mag));
        }
        match Big::cmp_mag(&self.mag, &other.mag) {
            Ordering::Less => Big::new(other.negative, Big::sub_mag(&other.mag, &self.mag)),
            _ => Big::new(self.negative, Big::sub_mag(&self.mag, &other.mag)),
        }
    }

    fn minus(&self, other: &Big) -> Big {
        self.plus(&other.negated())
    }

    fn times(&self, other: &Big) -> Big {
        let mut out = vec![0u32; self.mag.len() + other.mag.len()];
        for (i, &x) in self.mag.iter().enumerate() {
            let mut carry = 0u64;
            for (j, &y) in other.mag.iter().enumerate() {
                let t = u64::from(out[i + j]) + u64::from(x) * u64::from(y) + carry;
                out[i + j] = t as u32;
                carry = t >> 32;
            }
            out[i + other.mag.len()] = carry as u32;
        }
        Big::new(self.negative != other.negative, out)
    }

    /// self · 2^k.
    fn shifted(&self, k: u32) -> Big {
        let bits = k % 32;
        let mut out = vec![0u32; (k / 32) as usize];
        let mut carry = 0u32;
        for &x in &self.mag {
            if bits == 0 {
                out.push(x);
            } else {
                out.push((x << bits) | carry);
                carry = x >> (32 - bits);
            }
        }
        out.push(carry);
        Big::new(self.negative, out)
    }

    fn bit_len(&self) -> u32 {
        self.mag.last().map_or(0, |&top| {
            32 * (self.mag.len() as u32 - 1) + 32 - top.leading_zeros()
        })
    }

    fn bit(&self, i: u32) -> bool {
        self.mag
            .get((i / 32) as usize)
            .is_some_and(|&w| ((w >> (i % 32)) & 1) == 1)
    }

    /// floor(|self| / d) and |self| mod d, for d > 0, by restoring binary long division.
    fn div_rem(&self, d: &Big) -> (Big, Big) {
        let mut q = vec![0u32; self.mag.len()];
        let mut r = Big::zero();
        for i in (0..self.bit_len()).rev() {
            r = r.shifted(1);
            if self.bit(i) {
                r = r.plus(&Big::one());
            }
            if r >= *d {
                r = r.minus(d);
                q[(i / 32) as usize] |= 1 << (i % 32);
            }
        }
        (Big::new(false, q), r)
    }

    fn to_i128(&self) -> Option<i128> {
        if self.bit_len() <= 127 {
            let v = self
                .mag
                .iter()
                .rev()
                .fold(0i128, |acc, &w| (acc << 32) | i128::from(w));
            Some(if self.negative { -v } else { v })
        } else if *self == Big::pow2(127).negated() {
            Some(i128::MIN)
        } else {
            None
        }
    }
}

impl Ord for Big {
    fn cmp(&self, other: &Big) -> Ordering {
        match (self.negative, other.negative) {
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
            (false, false) => Big::cmp_mag(&self.mag, &other.mag),
            (true, true) => Big::cmp_mag(&other.mag, &self.mag),
        }
    }
}

impl PartialOrd for Big {
    fn partial_cmp(&self, other: &Big) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A finite double as an exact integer multiple of 2^−1074, the smallest subnormal.
fn scaled(x: f64) -> Big {
    assert!(x.is_finite(), "{x:e} is not finite");
    let bits = x.to_bits();
    let exponent = ((bits >> 52) & 0x7FF) as u32;
    let fraction = bits & ((1 << 52) - 1);
    let m = if exponent == 0 {
        Big::from_u128(u128::from(fraction))
    } else {
        Big::from_u128(u128::from(fraction | (1 << 52))).shifted(exponent - 1)
    };
    if bits >> 63 == 1 { m.negated() } else { m }
}

/// How many significant bits a double's significand has (0 for zero).
fn significant_bits(x: f64) -> u32 {
    if x == 0.0 {
        return 0;
    }
    let bits = x.to_bits();
    let fraction = bits & ((1 << 52) - 1);
    let m = if (bits >> 52) & 0x7FF == 0 {
        fraction
    } else {
        fraction | (1 << 52)
    };
    64 - m.leading_zeros() - m.trailing_zeros()
}

/// How many significant bits an exact integer's magnitude has (0 for zero).
fn significant_bits_of(v: &Big) -> u32 {
    let len = v.bit_len();
    let trailing = (0..len).find(|&i| v.bit(i)).unwrap_or(len);
    len - trailing
}

/// An exact rational with a positive denominator.
#[derive(Clone, Debug)]
struct Rat {
    num: Big,
    den: Big,
}

impl Rat {
    fn of(x: f64) -> Rat {
        Rat {
            num: scaled(x),
            den: Big::pow2(1074),
        }
    }

    fn int(v: Big) -> Rat {
        Rat {
            num: v,
            den: Big::one(),
        }
    }

    fn plus(&self, o: &Rat) -> Rat {
        Rat {
            num: self.num.times(&o.den).plus(&o.num.times(&self.den)),
            den: self.den.times(&o.den),
        }
    }

    fn negated(&self) -> Rat {
        Rat {
            num: self.num.negated(),
            den: self.den.clone(),
        }
    }

    fn minus(&self, o: &Rat) -> Rat {
        self.plus(&o.negated())
    }

    fn times(&self, o: &Rat) -> Rat {
        Rat {
            num: self.num.times(&o.num),
            den: self.den.times(&o.den),
        }
    }

    /// self / o, for o ≠ 0.
    fn over(&self, o: &Rat) -> Rat {
        let num = self.num.times(&o.den);
        let den = self.den.times(&o.num);
        if den.negative {
            Rat {
                num: num.negated(),
                den: den.negated(),
            }
        } else {
            Rat { num, den }
        }
    }

    fn compare(&self, o: &Rat) -> Ordering {
        self.num.times(&o.den).cmp(&o.num.times(&self.den))
    }

    fn min_of(values: &[Rat]) -> Rat {
        let mut best = values[0].clone();
        for v in values {
            if v.compare(&best) == Ordering::Less {
                best = v.clone();
            }
        }
        best
    }

    fn max_of(values: &[Rat]) -> Rat {
        let mut best = values[0].clone();
        for v in values {
            if v.compare(&best) == Ordering::Greater {
                best = v.clone();
            }
        }
        best
    }
}

/// The exact value of a `Wide`.
fn big_of_wide<const L: usize>(w: &Wide<L>) -> Big {
    let negative = w.is_negative();
    let mut limbs = w.to_le_limbs();
    if negative {
        let mut carry = 1u128;
        for limb in &mut limbs {
            let s = u128::from(!*limb) + carry;
            *limb = s as u64;
            carry = s >> 64;
        }
    }
    Big::new(
        negative,
        limbs
            .iter()
            .flat_map(|&l| [l as u32, (l >> 32) as u32])
            .collect(),
    )
}

/// `v` as a `Wide<L>`, or `None` when it does not fit −2^(64L−1) ..= 2^(64L−1) − 1.
fn wide_of_big<const L: usize>(v: &Big) -> Option<Wide<L>> {
    let half = Big::pow2(64 * L as u32 - 1);
    if *v >= half || *v < half.negated() {
        return None;
    }
    let word = |k: usize| u64::from(v.mag.get(k).copied().unwrap_or(0));
    let mut limbs = [0u64; L];
    for (i, limb) in limbs.iter_mut().enumerate() {
        *limb = word(2 * i) | (word(2 * i + 1) << 32);
    }
    if v.negative {
        let mut carry = 1u128;
        for limb in &mut limbs {
            let s = u128::from(!*limb) + carry;
            *limb = s as u64;
            carry = s >> 64;
        }
    }
    Some(Wide::from_le_limbs(limbs))
}

fn big(c: Coord) -> Big {
    Big::from_i64(c.get())
}

/// The determinant of a square matrix by Laplace expansion along the first row. Matrices here are
/// at most 5 × 5, so the recursion is at most five deep.
fn det(m: &[Vec<Big>]) -> Big {
    let Some((first, rest)) = m.split_first() else {
        return Big::one();
    };
    let mut acc = Big::zero();
    for (j, a) in first.iter().enumerate() {
        let minor: Vec<Vec<Big>> = rest
            .iter()
            .map(|row| {
                let kept = row.iter().enumerate().filter(|&(k, _)| k != j);
                kept.map(|(_, v)| v.clone()).collect()
            })
            .collect();
        let term = a.times(&det(&minor));
        acc = if j.is_multiple_of(2) {
            acc.plus(&term)
        } else {
            acc.minus(&term)
        };
    }
    acc
}

// ------------------------------------------------------------------------------------------------
// The oracles. Signs follow the conventions `geom_num::predicate` documents; the calibration test
// checks them on configurations whose answer is known by geometry.
// ------------------------------------------------------------------------------------------------

/// det [x y 1] over a, b, c: twice the signed area, positive counter-clockwise.
fn orient2d_exact(points: [P2; 3]) -> Big {
    det(&points.map(|p| vec![big(p.x), big(p.y), Big::one()]))
}

/// −det [x y z 1] over a, b, c, d, which is det [b − a; c − a; d − a].
fn orient3d_exact(points: [P3; 4]) -> Big {
    det(&points.map(|p| vec![big(p.x), big(p.y), big(p.z), Big::one()])).negated()
}

/// det [x y x²+y² 1] over a, b, c, d: positive when d is inside the counter-clockwise circle.
fn incircle_exact(points: [P2; 4]) -> Big {
    det(&points.map(|p| {
        let (x, y) = (big(p.x), big(p.y));
        let lift = x.times(&x).plus(&y.times(&y));
        vec![x, y, lift, Big::one()]
    }))
}

/// −det [x y z x²+y²+z² 1] over a, b, c, d, e: `geom-num` has the opposite sign to Shewchuk's.
fn insphere_exact(points: [P3; 5]) -> Big {
    det(&points.map(|p| {
        let (x, y, z) = (big(p.x), big(p.y), big(p.z));
        let lift = x.times(&x).plus(&y.times(&y)).plus(&z.times(&z));
        vec![x, y, z, lift, Big::one()]
    }))
    .negated()
}

/// The plane through a, b, c: n = (b − a) × (c − a) and d = −n · a.
fn plane_exact([a, b, c]: [P3; 3]) -> ([Big; 3], Big) {
    let coords = |p: P3| [big(p.x), big(p.y), big(p.z)];
    let (a, b, c) = (coords(a), coords(b), coords(c));
    let u = [0, 1, 2].map(|i| b[i].minus(&a[i]));
    let v = [0, 1, 2].map(|i| c[i].minus(&a[i]));
    let n = [
        u[1].times(&v[2]).minus(&u[2].times(&v[1])),
        u[2].times(&v[0]).minus(&u[0].times(&v[2])),
        u[0].times(&v[1]).minus(&u[1].times(&v[0])),
    ];
    let d = n
        .iter()
        .zip(&a)
        .fold(Big::zero(), |acc, (ni, ai)| acc.plus(&ni.times(ai)));
    (n, d.negated())
}

/// A plane's coefficients as exact values.
fn coefficients(p: &Plane) -> ([Big; 3], Big) {
    (p.n().map(Big::from_i128), Big::from_i128(p.d()))
}

/// The point where three planes meet, by Cramer's rule: w = det N and X_i = det N_i, where N has
/// the normals as rows and N_i has column i replaced by −d.
fn tpi_exact(t: &Tpi) -> ([Big; 3], Big) {
    let rows: Vec<([Big; 3], Big)> = t.planes().iter().map(coefficients).collect();
    let w = det(&rows.iter().map(|(n, _)| n.to_vec()).collect::<Vec<_>>());
    let x = [0, 1, 2].map(|i| {
        let replaced: Vec<Vec<Big>> = rows
            .iter()
            .map(|(n, d)| {
                let mut row = n.to_vec();
                row[i] = d.negated();
                row
            })
            .collect();
        det(&replaced)
    });
    (x, w)
}

/// num / den rounded to the nearest integer, ties to even, and whether it was exact.
fn round_half_even(num: &Big, den: &Big) -> (Big, bool) {
    let (num, den) = if den.negative {
        (num.negated(), den.negated())
    } else {
        (num.clone(), den.clone())
    };
    let (q, r) = num.magnitude().div_rem(&den);
    let up = match r.shifted(1).cmp(&den) {
        Ordering::Greater => true,
        Ordering::Equal => q.bit(0),
        Ordering::Less => false,
    };
    let q = if up { q.plus(&Big::one()) } else { q };
    (if num.negative { q.negated() } else { q }, r.is_zero())
}

/// A rounded result as its value and its flag (`Rounded`'s fields are private to `geom-num`, so an
/// oracle states what they must be through these two).
fn parts<T: Copy>(r: Result<Rounded<T>, NumError>) -> Result<(T, bool), NumError> {
    r.map(|r| (r.value(), r.is_exact()))
}

/// What `Tpi::to_grid` must return, from its documentation: the rounded point and whether it is
/// exact, or the error.
fn to_grid_exact(t: &Tpi) -> Result<(P3, bool), NumError> {
    let (x, w) = tpi_exact(t);
    let m = Big::from_i64(COORD_MAX);
    let mut coords = [0i64; 3];
    let mut exact = true;
    for (coord, xi) in coords.iter_mut().zip(&x) {
        let (q, e) = round_half_even(xi, &w);
        if q > m || q < m.negated() {
            let saturated = if q.negative { i128::MIN } else { i128::MAX };
            return Err(NumError::OutOfRange {
                value: q.to_i128().unwrap_or(saturated),
            });
        }
        *coord = i64::try_from(q.to_i128().unwrap()).unwrap();
        exact &= e;
    }
    let [x, y, z] = coords;
    Ok((P3::new(x, y, z).unwrap(), exact))
}

/// The exact value of n · X + d · w for a plane and a meeting point.
fn side_of_tpi_value(p: &Plane, t: &Tpi) -> (Big, Big) {
    let (n, d) = coefficients(p);
    let (x, w) = tpi_exact(t);
    let value = n
        .iter()
        .zip(&x)
        .fold(d.times(&w), |acc, (ni, xi)| acc.plus(&ni.times(xi)));
    (value, w)
}

/// What `refine` must return: c · 25^k, inside the frame.
fn refine_exact(c: Coord, from: GridStep, to: GridStep) -> Result<Coord, NumError> {
    if to.rung() < from.rung() {
        return Err(NumError::WrongDirection);
    }
    let value = i128::from(c.get()) * 25i128.pow(to.rung() - from.rung());
    match i64::try_from(value).ok().and_then(|v| Coord::new(v).ok()) {
        Some(c) => Ok(c),
        None => Err(NumError::OutOfRange { value }),
    }
}

/// What `coarsen` must return: the nearest integer to c / 25^k (25 is odd, so there are no ties),
/// and whether it is exact.
fn coarsen_exact(c: Coord, from: GridStep, to: GridStep) -> Result<(Coord, bool), NumError> {
    if to.rung() > from.rung() {
        return Err(NumError::WrongDirection);
    }
    let d = 25i128.pow(from.rung() - to.rung());
    let v = i128::from(c.get());
    let q = (2 * v + d).div_euclid(2 * d);
    let value = Coord::new(i64::try_from(q).unwrap()).unwrap();
    Ok((value, v % d == 0))
}

/// The size of a grid step in quarter-nanometres, from chapter 6.1 (156.25 nm, 6.25 nm and
/// 0.25 nm), written here rather than read from `geom-num`.
fn quarter_nm(step: GridStep) -> u128 {
    match step {
        GridStep::Q0 => 625,
        GridStep::Q1 => 25,
        GridStep::Q2 => 1,
    }
}

/// What `Steps::refine` must return: the same length in steps of `to`, exactly, when `to` is no
/// coarser than `from` and the count fits `u64`; `None` otherwise.
fn steps_refine_exact(s: Steps, from: GridStep, to: GridStep) -> Option<Steps> {
    let (coarse, fine) = (quarter_nm(from), quarter_nm(to));
    if coarse % fine != 0 {
        return None;
    }
    let count = u128::from(s.get()) * (coarse / fine);
    u64::try_from(count).ok().map(Steps)
}

/// What `GridStep::steps_per` must return: how many steps make the unit, when that is whole.
fn steps_per_exact(step: GridStep, unit: u64) -> Option<u64> {
    let size = quarter_nm(step);
    let unit = u128::from(unit);
    (unit % size == 0).then(|| u64::try_from(unit / size).unwrap())
}

/// A class's bound or deviation and its step, `None` for `Exact`: read from the variant here, not
/// with `Exactness::bound`.
fn bound_of(e: Exactness) -> Option<(u64, GridStep)> {
    match e {
        Exactness::Exact => None,
        Exactness::Certified { bound, step } => Some((bound.get(), step)),
        Exactness::Approximate { deviation, step } => Some((deviation.get(), step)),
    }
}

/// The finer of two steps.
fn finer(a: GridStep, b: GridStep) -> GridStep {
    if quarter_nm(a) <= quarter_nm(b) { a } else { b }
}

/// `count` steps of `from` counted in steps of `to`, a step no coarser, exactly.
fn count_in(count: u64, from: GridStep, to: GridStep) -> u128 {
    u128::from(count) * (quarter_nm(from) / quarter_nm(to))
}

/// What `Exactness::combine` must return (chapter 9): the less exact class, `Exact` being the
/// identity, with the bounds added in the finer of their steps; `None` when the sum does not fit
/// a `u64` count of that step.
fn combine_exact(x: Exactness, y: Exactness) -> Option<Exactness> {
    let (Some((a, sa)), Some((b, sb))) = (bound_of(x), bound_of(y)) else {
        return Some(if bound_of(x).is_none() { y } else { x });
    };
    let step = finer(sa, sb);
    let total = u64::try_from(count_in(a, sa, step) + count_in(b, sb, step)).ok()?;
    let both_certified = matches!(
        (x, y),
        (Exactness::Certified { .. }, Exactness::Certified { .. })
    );
    Some(if both_certified {
        Exactness::Certified {
            bound: Steps(total),
            step,
        }
    } else {
        Exactness::Approximate {
            deviation: Steps(total),
            step,
        }
    })
}

/// What `Exactness::within` must return (chapter 6.5): the class itself when its bound is at most
/// the ceiling, compared exactly in the finer step, or `Exact`; otherwise `ToleranceExceeded`,
/// also when the bound does not fit a `u64` count of the finer step. The benchmark's ceilings are
/// below 2^40 steps, so the ceiling always fits; this asserts it rather than guess what an
/// unconvertible ceiling would mean.
fn within_exact(x: Exactness, ceiling: Steps, step: GridStep) -> Result<Exactness, NumError> {
    let Some((bound, bound_step)) = bound_of(x) else {
        return Ok(x);
    };
    let to = finer(bound_step, step);
    let (b, c) = (
        count_in(bound, bound_step, to),
        count_in(ceiling.get(), step, to),
    );
    assert!(
        c <= u128::from(u64::MAX),
        "a ceiling that does not fit: {ceiling:?} {step:?}"
    );
    if b <= c {
        Ok(x)
    } else {
        Err(NumError::ToleranceExceeded)
    }
}

/// The output digest the gates check: SHA-256 of the canonical encoding of every result, in order
/// (`geom_bench::harness::Bound::digests`, outputs half). `the_rewritten_digest_matches_the_kernels`
/// checks that this is what every benchmark reports, and `the_canonical_layout_is_the_documented_one`
/// checks the encoding byte by byte.
fn fold<R: Canonical>(results: impl IntoIterator<Item = R>) -> Digest {
    let results: Vec<R> = results.into_iter().collect();
    canonical::digest(&results)
}

/// The output digest of one run of `kernel` over `batch`, as a benchmark computes it.
fn kernel_digest<T, R: Canonical>(kernel: fn(&[T], &mut Vec<R>), batch: &[T]) -> Digest {
    let mut out = Vec::with_capacity(batch.len());
    kernel(batch, &mut out);
    canonical::digest(&out)
}

// ------------------------------------------------------------------------------------------------
// Wiring: every benchmark measures the operation it is named after.
// ------------------------------------------------------------------------------------------------

type Unary = (fn(Class) -> Vec<f64>, fn(f64) -> Result<Real, NumError>);
type Binary = (
    fn(Class) -> Vec<(f64, f64)>,
    fn(f64, f64) -> Result<Real, NumError>,
);

fn math_digest(name: &str, class: Class) -> Option<Digest> {
    use inputs::math as m;
    let unary: [(&str, Unary); 9] = [
        ("sqrt", (m::sqrt, math::sqrt)),
        ("exp", (m::exp, math::exp)),
        ("ln", (m::ln, math::ln)),
        ("abs", (m::abs, math::abs)),
        ("floor", (m::floor, math::floor)),
        ("ceil", (m::ceil, math::ceil)),
        (
            "round_ties_even",
            (m::round_ties_even, math::round_ties_even),
        ),
        ("sinpi", (m::sinpi, math::sinpi)),
        ("cospi", (m::cospi, math::cospi)),
    ];
    if let Some((_, (build, f))) = unary.iter().find(|(n, _)| *n == name) {
        return Some(fold(build(class).into_iter().map(*f)));
    }
    Some(match name {
        "atan2" => fold(m::atan2(class).into_iter().map(|(y, x)| math::atan2(y, x))),
        "fma" => fold(
            m::fma(class)
                .into_iter()
                .map(|(a, b, c)| math::fma(a, b, c)),
        ),
        _ => return None,
    })
}

/// The digest of the benchmark of `math::unproven`'s function `name` on `class`, from direct
/// calls on its inputs.
fn unproven_digest(name: &str, class: Class) -> Option<Digest> {
    use geom_num::math::unproven as f;
    use inputs::math::unproven as m;
    let unary: [(&str, Unary); 13] = [
        ("sin", (m::sin, f::sin)),
        ("cos", (m::cos, f::cos)),
        ("tan", (m::tan, f::tan)),
        ("asin", (m::asin, f::asin)),
        ("acos", (m::acos, f::acos)),
        ("atan", (m::atan, f::atan)),
        ("exp", (m::exp, f::exp)),
        ("ln", (m::ln, f::ln)),
        ("log2", (m::log2, f::log2)),
        ("log10", (m::log10, f::log10)),
        ("cbrt", (m::cbrt, f::cbrt)),
        ("sinpi", (m::sinpi, f::sinpi)),
        ("cospi", (m::cospi, f::cospi)),
    ];
    let binary: [(&str, Binary); 3] = [
        ("atan2", (m::atan2, f::atan2)),
        ("pow", (m::pow, f::pow)),
        ("hypot", (m::hypot, f::hypot)),
    ];
    if let Some((_, (build, g))) = unary.iter().find(|(n, _)| *n == name) {
        return Some(fold(build(class).into_iter().map(*g)));
    }
    if name == "powi" {
        return Some(fold(m::powi(class).into_iter().map(|(x, n)| f::powi(x, n))));
    }
    let (_, (build, g)) = binary.iter().find(|(n, _)| *n == name)?;
    Some(fold(build(class).into_iter().map(|(x, y)| g(x, y))))
}

fn wide_digest_at<const L: usize>(kind: &str, class: Class) -> Option<Digest> {
    use inputs::wide as w;
    Some(match kind {
        "add" => fold(w::add::<L>(class).iter().map(|(x, y)| x.checked_add(y))),
        "sub" => fold(w::sub::<L>(class).iter().map(|(x, y)| x.checked_sub(y))),
        "mul" => fold(w::mul::<L>(class).iter().map(|(x, y)| x.checked_mul(y))),
        "neg" => fold(w::neg::<L>(class).iter().map(Wide::checked_neg)),
        "cmp" => fold(w::cmp::<L>(class).iter().map(|(x, y)| x.cmp(y))),
        "signum" => fold(w::signum::<L>(class).iter().map(Wide::signum)),
        "to_i128" => fold(w::to_i128::<L>(class).iter().map(Wide::to_i128)),
        "from_i128" => fold(
            w::from_i128::<L>(class)
                .into_iter()
                .map(Wide::<L>::from_i128),
        ),
        _ => return None,
    })
}

fn wide_digest(name: &str, class: Class) -> Option<Digest> {
    use inputs::wide as w;
    match name {
        "i256_to_i384" => {
            return Some(fold(w::resize::<4, 6>(class).iter().map(Wide::resize::<6>)));
        }
        "i320_to_i384" => {
            return Some(fold(w::resize::<5, 6>(class).iter().map(Wide::resize::<6>)));
        }
        "i384_to_i256" => {
            return Some(fold(w::resize::<6, 4>(class).iter().map(Wide::resize::<4>)));
        }
        _ => {}
    }
    let (width, kind) = name.split_once('_')?;
    match width {
        "i256" => wide_digest_at::<4>(kind, class),
        "i320" => wide_digest_at::<5>(kind, class),
        "i384" => wide_digest_at::<6>(kind, class),
        "i512" => wide_digest_at::<8>(kind, class),
        _ => None,
    }
}

/// The output digest of a `Wide32` benchmark at `M` limbs (piece G42), by calling `geom-num`
/// directly on the benchmark's inputs.
fn wide32_digest_at<const L: usize, const M: usize>(kind: &str, class: Class) -> Option<Digest> {
    use inputs::wide32 as w;
    Some(match kind {
        "add" => fold(w::add::<L, M>(class).iter().map(|(x, y)| x.checked_add(y))),
        "sub" => fold(w::sub::<L, M>(class).iter().map(|(x, y)| x.checked_sub(y))),
        "product" => fold(
            w::product::<L, M>(class)
                .iter()
                .map(|(x, y)| Wide32::<M>::product(x, y)),
        ),
        "neg" => fold(w::neg::<L, M>(class).iter().map(Wide32::checked_neg)),
        "cmp" => fold(w::cmp::<L, M>(class).iter().map(|(x, y)| x.cmp(y))),
        "signum" => fold(w::signum::<L, M>(class).iter().map(Wide32::signum)),
        "to_i128" => fold(w::to_i128::<L, M>(class).iter().map(Wide32::to_i128)),
        "from_i128" => fold(
            w::from_i128::<M>(class)
                .into_iter()
                .map(Wide32::<M>::from_i128),
        ),
        _ => return None,
    })
}

fn wide32_digest(name: &str, class: Class) -> Option<Digest> {
    use inputs::wide32 as w;
    match name {
        "w8_to_w12" => {
            return Some(fold(
                w::resize::<4, 6, 8>(class).iter().map(Wide32::resize::<12>),
            ));
        }
        "w10_to_w12" => {
            return Some(fold(
                w::resize::<5, 6, 10>(class)
                    .iter()
                    .map(Wide32::resize::<12>),
            ));
        }
        "w12_to_w8" => {
            return Some(fold(
                w::resize::<6, 4, 12>(class).iter().map(Wide32::resize::<8>),
            ));
        }
        "w12_to_wide" => {
            return Some(fold(
                w::to_wide::<6, 12>(class).iter().map(Wide32::to_wide::<6>),
            ));
        }
        "w3_product_i64" => {
            return Some(fold(
                w::product_i64(class)
                    .into_iter()
                    .map(|(a, b)| Wide32::<3>::product_i64(a, b)),
            ));
        }
        _ => {}
    }
    let (width, kind) = name.split_once('_')?;
    match width {
        "w8" => wide32_digest_at::<4, 8>(kind, class),
        "w10" => wide32_digest_at::<5, 10>(kind, class),
        "w12" => wide32_digest_at::<6, 12>(kind, class),
        "w16" => wide32_digest_at::<8, 16>(kind, class),
        _ => None,
    }
}

/// The output digest of the growth benchmark `name` at `limbs`, by calling `geom-num` directly.
fn growth_digest(name: &str, limbs: usize) -> Option<Digest> {
    use inputs::growth as g;
    fn at<const L: usize>(name: &str) -> Option<Digest> {
        Some(match name {
            "wide_add" => fold(g::add::<L>().iter().map(|(x, y)| x.checked_add(y))),
            "wide_sub" => fold(g::sub::<L>().iter().map(|(x, y)| x.checked_sub(y))),
            "wide_neg" => fold(g::neg::<L>().iter().map(Wide::checked_neg)),
            "wide_cmp" => fold(g::cmp::<L>().iter().map(|(x, y)| x.cmp(y))),
            "wide_mul" => fold(g::mul::<L>().iter().map(|(x, y)| x.checked_mul(y))),
            _ => return None,
        })
    }
    match limbs {
        2 => at::<2>(name),
        4 => at::<4>(name),
        8 => at::<8>(name),
        _ => None,
    }
}

/// The digest of the catalogued benchmark `group/name/class`, computed by calling the `geom-num`
/// operation directly, or `None` for an operation this table does not know.
fn direct_digest(group: &str, name: &str, class: Class) -> Option<Digest> {
    use geom_num::quadric;
    use inputs::quadric as iv;
    use inputs::{eft as ie, grid as ig, interval as ii, plane as ip, predicate as iq};
    use inputs::{exactness as ix, real as ir, sign as is};
    Some(match (group, name) {
        ("quadric", "line_sphere") => fold(
            iv::spheres(class)
                .iter()
                .map(|(l, s)| quadric::line_sphere(l, s)),
        ),
        ("quadric", "line_cylinder") => fold(
            iv::cylinders(class)
                .iter()
                .map(|(l, c)| quadric::line_cylinder(l, c)),
        ),
        ("quadric", "line_cone") => fold(
            iv::cones(class)
                .iter()
                .map(|(l, c)| quadric::line_cone(l, c)),
        ),
        ("quadric", "line_torus") => fold(
            iv::tori(class)
                .iter()
                .map(|(l, t)| quadric::roots(l, &quadric::Surface::Torus(*t))),
        ),
        ("quadric", "parallel") => fold(
            iv::directions(class)
                .iter()
                .map(|(u, v)| quadric::parallel(u.a, u.b, v.a, v.b)),
        ),
        ("quadric", "axis_gap") => fold(
            iv::gaps(class)
                .iter()
                .map(|(u, v)| quadric::axis_gap(u.a, u.b, v.a, geom_num::grid::Steps(1024))),
        ),
        ("quadric", "tangent") => fold(
            iv::tangents(class)
                .iter()
                .map(|(c, k)| quadric::tangent_cylinder_cone(c, k)),
        ),
        ("predicate", "orient2d") => fold(
            iq::orient2d(class)
                .into_iter()
                .map(|[a, b, c]| predicate::orient2d(a, b, c)),
        ),
        ("predicate", "orient3d") => fold(
            iq::orient3d(class)
                .into_iter()
                .map(|[a, b, c, d]| predicate::orient3d(a, b, c, d)),
        ),
        ("predicate", "incircle") => fold(
            iq::incircle(class)
                .into_iter()
                .map(|[a, b, c, d]| predicate::incircle(a, b, c, d)),
        ),
        ("predicate", "insphere") => fold(
            iq::insphere(class)
                .into_iter()
                .map(|[a, b, c, d, e]| predicate::insphere(a, b, c, d, e)),
        ),
        ("sign", "of_i128") => fold(is::of_i128(class).into_iter().map(Sign::of_i128)),
        ("wide", _) => wide_digest(name, class)?,
        ("wide32", _) => wide32_digest(name, class)?,
        ("plane", "through") => fold(
            ip::through(class)
                .into_iter()
                .map(|[a, b, c]| Plane::through(a, b, c)),
        ),
        ("plane", "side") => fold(ip::side(class).into_iter().map(|(p, q)| p.side(q))),
        ("plane", "tpi_new") => fold(
            ip::tpi_new(class)
                .into_iter()
                .map(|[p, q, r]| Tpi::new(p, q, r)),
        ),
        ("plane", "tpi_homogeneous") => {
            fold(ip::tpi_homogeneous(class).iter().map(Tpi::homogeneous))
        }
        ("plane", "side_of_tpi") => {
            fold(ip::side_of_tpi(class).iter().map(|(p, t)| p.side_of_tpi(t)))
        }
        ("plane", "to_grid") => fold(ip::to_grid(class).iter().map(Tpi::to_grid)),
        ("plane", "lpi_new") => fold(
            ip::lpi_new(class)
                .into_iter()
                .map(|(p, q, plane)| Lpi::new(p, q, plane)),
        ),
        ("plane", "lpi_homogeneous") => {
            fold(ip::lpi_homogeneous(class).iter().map(Lpi::homogeneous))
        }
        ("plane", "side_of_lpi") => {
            fold(ip::side_of_lpi(class).iter().map(|(p, l)| p.side_of_lpi(l)))
        }
        ("plane", "lpi_to_grid") => fold(ip::lpi_to_grid(class).iter().map(Lpi::to_grid)),
        ("crossing", "ssi_new") => fold(
            inputs::crossing::ssi_new(class)
                .into_iter()
                .map(|[a, b, c, d]| Ssi::new(a, b, c, d)),
        ),
        ("crossing", "ssi_homogeneous") => fold(
            inputs::crossing::ssi_homogeneous(class)
                .iter()
                .map(Ssi::homogeneous),
        ),
        ("crossing", "ssi_orient2d") => fold(
            inputs::crossing::ssi_orient2d(class)
                .into_iter()
                .map(|(s, p, q)| s.orient2d(p, q)),
        ),
        ("crossing", "ssi_to_grid") => fold(
            inputs::crossing::ssi_to_grid(class)
                .iter()
                .map(Ssi::to_grid),
        ),
        ("perturb", "orient2d") => fold(
            inputs::perturb::orient2d(class)
                .into_iter()
                .map(|[x, y, z]| geom_num::perturb::orient2d(x, y, z)),
        ),
        ("perturb", "orient3d") => fold(
            inputs::perturb::orient3d(class)
                .into_iter()
                .map(|[x, y, z, w]| geom_num::perturb::orient3d(x, y, z, w)),
        ),
        ("perturb", "incircle") => fold(
            inputs::perturb::incircle(class)
                .into_iter()
                .map(|[x, y, z, w]| geom_num::perturb::incircle(x, y, z, w)),
        ),
        ("perturb", "insphere") => fold(
            inputs::perturb::insphere(class)
                .into_iter()
                .map(|[x, y, z, w, v]| geom_num::perturb::insphere(x, y, z, w, v)),
        ),
        ("newton", "krawczyk_2") => fold(
            inputs::newton::krawczyk_2(class)
                .into_iter()
                .map(|b| geom_num::newton::krawczyk(&inputs::newton::CircleLine, b)),
        ),
        ("newton", "hansen_sengupta_2") => fold(
            inputs::newton::hansen_sengupta_2(class)
                .into_iter()
                .map(|b| geom_num::newton::hansen_sengupta(&inputs::newton::CircleLine, b)),
        ),
        ("newton", "krawczyk_4") => fold(
            inputs::newton::krawczyk_4(class)
                .into_iter()
                .map(|b| geom_num::newton::krawczyk(&inputs::newton::Four, b)),
        ),
        ("newton", "hansen_sengupta_4") => fold(
            inputs::newton::hansen_sengupta_4(class)
                .into_iter()
                .map(|b| geom_num::newton::hansen_sengupta(&inputs::newton::Four, b)),
        ),
        ("dual", "jacobian_real") => {
            fold(inputs::newton::jacobian_real(class).into_iter().map(|x| {
                geom_num::dual::jacobian(
                    |v| geom_num::newton::Residuals::residuals(&inputs::newton::Four, v),
                    x,
                )
            }))
        }
        ("dual", "jacobian_interval") => fold(
            inputs::newton::jacobian_interval(class)
                .into_iter()
                .map(|x| {
                    geom_num::dual::jacobian(
                        |v| geom_num::newton::Residuals::residuals(&inputs::newton::Four, v),
                        x,
                    )
                }),
        ),
        ("isolate", "cubic_1") => fold(inputs::isolate::cubic_1(class).into_iter().map(
            |(domain, target)| {
                let options = geom_num::isolate::Options {
                    target,
                    ..geom_num::isolate::Options::DEFAULT
                };
                let mut meter = geom_run::WorkMeter::new(geom_run::Budget::UNLIMITED);
                geom_num::isolate::isolate(&inputs::isolate::Cubic, domain, options, &mut meter)
            },
        )),
        ("isolate", "circle_line_2") => fold(
            inputs::isolate::circle_line_2(class)
                .into_iter()
                .map(|(domain, target)| {
                    let options = geom_num::isolate::Options {
                        target,
                        ..geom_num::isolate::Options::DEFAULT
                    };
                    let mut meter = geom_run::WorkMeter::new(geom_run::Budget::UNLIMITED);
                    geom_num::isolate::isolate(
                        &inputs::newton::CircleLine,
                        domain,
                        options,
                        &mut meter,
                    )
                }),
        ),
        ("ball", "cubic") => fold(inputs::isolate::ball_cubic(class).into_iter().map(|[x]| {
            let ball = geom_num::ball::Ball::from_interval(x)?;
            let [value] = geom_num::newton::Residuals::residuals(&inputs::isolate::Cubic, [ball])?;
            value.to_interval()
        })),
        ("eft", "two_sum") => fold(
            ie::two_sum(class)
                .into_iter()
                .map(|(a, b)| eft::two_sum(a, b)),
        ),
        ("eft", "fast_two_sum") => fold(
            ie::fast_two_sum(class)
                .into_iter()
                .map(|(a, b)| eft::fast_two_sum(a, b)),
        ),
        ("eft", "split") => fold(ie::split(class).into_iter().map(eft::split)),
        ("eft", "two_prod") => fold(
            ie::two_prod(class)
                .into_iter()
                .map(|(a, b)| eft::two_prod(a, b)),
        ),
        ("interval", "new") => fold(
            ii::new(class)
                .into_iter()
                .map(|(lo, hi)| Interval::new(lo, hi)),
        ),
        ("interval", "contains") => {
            fold(ii::contains(class).into_iter().map(|(x, v)| x.contains(v)))
        }
        ("interval", "add") => fold(ii::add(class).into_iter().map(|(x, y)| x.add(y))),
        ("interval", "sub") => fold(ii::sub(class).into_iter().map(|(x, y)| x.sub(y))),
        ("interval", "mul") => fold(ii::mul(class).into_iter().map(|(x, y)| x.mul(y))),
        ("interval", "div") => fold(ii::div(class).into_iter().map(|(x, y)| x.div(y))),
        ("interval", "mul_add") => fold(
            ii::mul_add(class)
                .into_iter()
                .map(|(a, b, c)| a.mul_add(b, c)),
        ),
        ("interval", "square") => fold(ii::square(class).into_iter().map(Interval::square)),
        ("interval", "sqrt") => fold(ii::sqrt(class).into_iter().map(Interval::sqrt)),
        ("interval", "norm") => fold(ii::norm(class).iter().map(|v| Interval::norm(v))),
        ("interval", "neg") => fold(ii::neg(class).into_iter().map(Interval::neg)),
        ("interval", "sinpi") => fold(ii::sinpi(class).into_iter().map(Interval::sinpi)),
        ("interval", "cospi") => fold(ii::cospi(class).into_iter().map(Interval::cospi)),
        ("interval", "exp") => fold(ii::exp(class).into_iter().map(Interval::exp)),
        ("interval", "ln") => fold(ii::ln(class).into_iter().map(Interval::ln)),
        ("interval", "atan2") => fold(ii::atan2(class).into_iter().map(|(y, x)| y.atan2(x))),
        ("interval", "certain_sign") => fold(
            ii::certain_sign(class)
                .into_iter()
                .map(Interval::certain_sign),
        ),
        ("grid", "coord_new") => fold(ig::coord_new(class).into_iter().map(Coord::new)),
        ("grid", "coord_from_i128") => {
            fold(ig::coord_from_i128(class).into_iter().map(Coord::from_i128))
        }
        ("grid", "p2_new") => fold(ig::p2_new(class).into_iter().map(|(x, y)| P2::new(x, y))),
        ("grid", "p3_new") => fold(
            ig::p3_new(class)
                .into_iter()
                .map(|(x, y, z)| P3::new(x, y, z)),
        ),
        ("real", "total_max") => fold(
            ir::total_max(class)
                .into_iter()
                .map(|(x, y)| x.total_max(y)),
        ),
        ("real", "total_min") => fold(
            ir::total_min(class)
                .into_iter()
                .map(|(x, y)| x.total_min(y)),
        ),
        ("real", "cmp") => fold(ir::cmp(class).iter().map(|(x, y)| x.cmp(y))),
        ("exactness", "combine") => fold(ix::combine(class).into_iter().map(|(x, y)| x.combine(y))),
        ("exactness", "within") => fold(
            ix::within(class)
                .into_iter()
                .map(|(x, c, s)| x.within(c, s)),
        ),
        ("exactness", "combine_within") => fold(
            ix::combine_within(class)
                .into_iter()
                .map(|(x, y, c, s)| x.combine_within(y, c, s)),
        ),
        ("grid", "steps_refine") => fold(
            ig::steps_refine(class)
                .into_iter()
                .map(|(s, f, t)| s.refine(f, t)),
        ),
        ("grid", "steps_per") => fold(
            ig::steps_per(class)
                .into_iter()
                .map(|(s, u)| s.steps_per(u)),
        ),
        ("grid", "refine") => fold(
            ig::refine(class)
                .into_iter()
                .map(|(c, f, t)| grid::refine(c, f, t)),
        ),
        ("grid", "coarsen") => fold(
            ig::coarsen(class)
                .into_iter()
                .map(|(c, f, t)| grid::coarsen(c, f, t)),
        ),
        ("math", _) => math_digest(name, class)?,
        ("unproven", _) => unproven_digest(name, class)?,
        ("rng", _) => rng_digest(name, class)?,
        ("dd", _) => dd_digest(name, class)?,
        ("exact_sum", _) => exact_sum_digest(name, class)?,
        ("generated", _) => generated_digest(name, class)?,
        ("math_types", _) => math_types_digest(name, class)?,
        ("curve", _) => curve_digest(name, class)?,
        ("brent", "zero") => {
            let tol = Real::new(1e-12).unwrap();
            fold(
                inputs::roots::zero(class)
                    .into_iter()
                    .map(|(a, b)| geom_num::brent::zero(inputs::roots::cubic, a, b, tol, 100)),
            )
        }
        ("intersect", "curve_segments") => fold(
            inputs::intersect::segment_pairs(class)
                .iter()
                .map(|(a, b)| {
                    let mut meter = geom_run::WorkMeter::new(geom_run::Budget::UNLIMITED);
                    geom_intersect::curve::curves(a, b, &mut meter)
                }),
        ),
        ("intersect", "curve_circles") => {
            fold(inputs::intersect::circle_pairs(class).iter().map(|(a, b)| {
                let mut meter = geom_run::WorkMeter::new(geom_run::Budget::UNLIMITED);
                geom_intersect::curve::curves(a, b, &mut meter)
            }))
        }
        ("intersect", "curve_line_circle") => {
            fold(inputs::intersect::line_circles(class).iter().map(|(a, b)| {
                let mut meter = geom_run::WorkMeter::new(geom_run::Budget::UNLIMITED);
                geom_intersect::curve::curves(a, b, &mut meter)
            }))
        }
        ("intersect", "curve_cubics") => {
            fold(inputs::intersect::cubic_pairs(class).iter().map(|(a, b)| {
                let mut meter = geom_run::WorkMeter::new(geom_run::Budget::UNLIMITED);
                geom_intersect::curve::curves(a, b, &mut meter)
            }))
        }
        ("intersect", "curve_loop") => fold(inputs::intersect::loops(class).iter().map(|c| {
            let mut meter = geom_run::WorkMeter::new(geom_run::Budget::UNLIMITED);
            geom_intersect::curve::self_crossings(c, &mut meter)
        })),
        ("intersect", "sort_items") => {
            fold(inputs::intersect::items(class).into_iter().map(|items| {
                geom_intersect::Intersection::new(
                    items,
                    geom_intersect::Coverage::CompleteOutsideFlagged,
                    Vec::new(),
                    &mut geom_run::WorkMeter::new(geom_run::Budget::UNLIMITED),
                )
            }))
        }
        ("intersect", "project_arc") => {
            fold(inputs::intersect::arcs(class).iter().map(|(curve, p)| {
                let mut meter = geom_run::WorkMeter::new(geom_run::Budget::UNLIMITED);
                geom_intersect::project_curve(curve, p, 1e-6, &mut meter)
            }))
        }
        ("intersect", "project_sphere") => fold(inputs::intersect::spheres(class).iter().map(
            |(surface, p)| {
                let mut meter = geom_run::WorkMeter::new(geom_run::Budget::UNLIMITED);
                geom_intersect::project_surface(surface, p, 1e-6, &mut meter)
            },
        )),
        ("intersect", "key_circle") => fold(inputs::intersect::circles(class).iter().map(|c| {
            let mut meter = geom_run::WorkMeter::new(geom_run::Budget::UNLIMITED);
            geom_intersect::CanonicalKey::of_curve(c, &mut meter)
        })),
        ("intersect", "pair_lines") => {
            fold(inputs::intersect::lines(class).iter().map(|(a, b)| {
                let operand = |curve| {
                    let mut meter = geom_run::WorkMeter::new(geom_run::Budget::UNLIMITED);
                    geom_intersect::CanonicalKey::of_curve(curve, &mut meter).map(|key| {
                        geom_intersect::Operand {
                            geom: geom_intersect::Geom::Curve(curve),
                            domain: None,
                            bound: Steps(0),
                            key,
                        }
                    })
                };
                let (x, y) = (operand(a)?, operand(b)?);
                geom_intersect::pair(&x, &y, |_, _| {
                    geom_intersect::Intersection::new(
                        Vec::new(),
                        geom_intersect::Coverage::TopologyCertified,
                        Vec::new(),
                        &mut geom_run::WorkMeter::new(geom_run::Budget::UNLIMITED),
                    )
                })
            }))
        }
        ("quadrature", "gauss_legendre") => fold(
            inputs::roots::gauss_legendre(class)
                .into_iter()
                .map(|(a, b)| {
                    geom_num::quadrature::gauss_legendre(inputs::roots::quintic, a, b, 8)
                }),
        ),
        ("surface", "torus_point" | "offset_point") => {
            use geom_surface::ParamSurface;
            let (surface, pairs) = if name == "torus_point" {
                (
                    inputs::surface::torus(),
                    inputs::surface::torus_point(class),
                )
            } else {
                (
                    inputs::surface::offset_cylinder(),
                    inputs::surface::offset_point(class),
                )
            };
            let s = surface.unwrap();
            let sides = (geom_nurbs::Limit::FromRight, geom_nurbs::Limit::FromRight);
            fold(pairs.into_iter().map(|(u, v)| s.point(u, v, sides)))
        }
        ("surface", "torus_second_bound") => {
            use geom_surface::ParamSurface;
            let s = inputs::surface::torus().unwrap();
            let sides = (geom_nurbs::Limit::FromRight, geom_nurbs::Limit::FromRight);
            fold(
                inputs::surface::torus_second_bound(class)
                    .into_iter()
                    .map(|region| {
                        s.second_derivative_bound(
                            region,
                            sides,
                            &mut geom_run::WorkMeter::new(geom_run::Budget::UNLIMITED),
                        )
                    }),
            )
        }
        ("surface", "torus_normal_bound") => {
            use geom_surface::ParamSurface;
            let s = inputs::surface::torus().unwrap();
            let sides = (geom_nurbs::Limit::FromRight, geom_nurbs::Limit::FromRight);
            fold(
                inputs::surface::torus_normal_bound(class)
                    .into_iter()
                    .map(|region| {
                        s.normal_derivative_bound(
                            region,
                            sides,
                            &mut geom_run::WorkMeter::new(geom_run::Budget::UNLIMITED),
                        )
                    }),
            )
        }
        ("surface", "torus_enclose") => {
            use geom_surface::ParamSurface;
            let s = inputs::surface::torus().unwrap();
            let sides = (geom_nurbs::Limit::FromRight, geom_nurbs::Limit::FromRight);
            fold(
                inputs::surface::torus_enclose(class)
                    .into_iter()
                    .map(|(u, v)| {
                        s.enclose(
                            u,
                            v,
                            sides,
                            &mut geom_run::WorkMeter::new(geom_run::Budget::UNLIMITED),
                        )
                    }),
            )
        }
        ("nurbs", "curve_point") => {
            let curve = inputs::nurbs::cubic().unwrap();
            fold(
                inputs::nurbs::curve_point(class)
                    .into_iter()
                    .map(|u| curve.point(u, geom_nurbs::Limit::FromRight)),
            )
        }
        ("nurbs", "interpolate") => fold(inputs::nurbs::interpolate(class).iter().map(|q| {
            geom_nurbs::Curve::interpolate(
                q,
                3,
                geom_nurbs::Parametrization::ChordLength,
                geom_nurbs::FitVersion::V1,
                &mut geom_run::WorkMeter::new(geom_run::Budget::UNLIMITED),
            )
        })),
        ("nurbs", "bernstein_bound") => fold(
            inputs::nurbs::bernstein_bound(class)
                .into_iter()
                .map(|points| {
                    geom_nurbs::BernsteinCurve::new((0.0, 1.0), points, None)?
                        .bound(&mut geom_run::WorkMeter::new(geom_run::Budget::UNLIMITED))
                }),
        ),
        ("bvh", "tree_build_query") => fold(inputs::bvh::tree_build_query(class).iter().map(
            |(entries, query)| {
                let mut meter = geom_run::WorkMeter::new(geom_run::Budget::UNLIMITED);
                let tree =
                    geom_bvh::BoxTree::build(entries, &geom_run::Exec::serial(), &mut meter)?;
                tree.overlapping(query, &mut meter)
            },
        )),
        ("bvh", "cells_within") => fold(inputs::bvh::cells_within(class).iter().map(
            |(points, center, radius)| {
                let mut meter = geom_run::WorkMeter::new(geom_run::Budget::UNLIMITED);
                let index =
                    geom_bvh::CellIndex::build(points, &geom_run::Exec::serial(), &mut meter)?;
                index.within(*center, *radius, &mut meter)
            },
        )),
        ("linalg", "pivoted_qr") => fold(
            inputs::linalg::pivoted_qr(class)
                .iter()
                .map(|a| geom_linalg::PivotedQr::factor(a, &mut geom_linalg::Work::new(u64::MAX))),
        ),
        ("linalg", "svd_jacobi") => fold(
            inputs::linalg::svd_jacobi(class)
                .iter()
                .map(|a| geom_linalg::Svd::jacobi(a, &mut geom_linalg::Work::new(u64::MAX))),
        ),
        _ => return None,
    })
}

/// The digest of the `geom-curve` benchmark `name` on `class` (decision 32.44), from direct calls
/// on its inputs through the kind contract, with the kernels' helix and transition.
fn curve_digest(name: &str, class: Class) -> Option<Digest> {
    use geom_bench::inputs::curve as m;
    use geom_curve::{BulgePolyline, CurveKind, ParamCurve};
    use geom_nurbs::Limit;
    let meter = || geom_run::WorkMeter::new(geom_run::Budget::UNLIMITED);
    Some(match name {
        "helix_point" => {
            let h = inputs::curve::helix().unwrap();
            fold(
                m::helix_point(class)
                    .into_iter()
                    .map(|t| h.point(t, Limit::FromRight)),
            )
        }
        "helix_enclose" => {
            let h = inputs::curve::helix().unwrap();
            fold(
                m::helix_enclose(class)
                    .into_iter()
                    .map(|t| h.enclose(t, Limit::FromRight, 2, &mut meter())),
            )
        }
        "bulge_to_nurbs" => fold(m::bulge_to_nurbs(class).into_iter().map(|(v, b)| {
            CurveKind::Bulge(BulgePolyline::new(v, b, None, true)?).to_nurbs(&mut meter())
        })),
        "transition_point" => {
            let t = inputs::curve::transition(120.0).unwrap();
            fold(
                m::transition_point(class)
                    .into_iter()
                    .map(|s| t.point(s, Limit::FromRight)),
            )
        }
        "transition_to_nurbs" => fold(
            m::transition_to_nurbs(class)
                .into_iter()
                .map(|l| inputs::curve::transition(l)?.to_nurbs(&mut meter())),
        ),
        _ => return None,
    })
}

/// The digest of the `geom-math` benchmark `name` on `class` (decision 32.44), from direct calls on
/// its inputs: each transform built here as `geom-math`'s `benches/transform.rs` builds it, and
/// the rotation and frame from the kernels' own definitions.
fn math_types_digest(name: &str, class: Class) -> Option<Digest> {
    use geom_bench::inputs::math_types as m;
    use geom_math::{
        Axis, Decimal, F3, GridTransform, RationalRotation, RationalTransform, Rotation, Transform,
        TypedTransform,
    };
    let apply = |t: TypedTransform, points: Vec<P3>| fold(points.into_iter().map(|p| t.apply(p)));
    Some(match name {
        "apply_exact" => {
            let shift = GridTransform::translation([12_345, -678, 9]).unwrap();
            let t = GridTransform::quarter_turns(Axis::Z, 1)
                .compose(&shift)
                .unwrap();
            apply(TypedTransform::Exact(t), m::apply_exact(class))
        }
        "apply_rational" => {
            let turn =
                RationalTransform::rotation(Axis::Z, Rotation::direction(3, 4).unwrap()).unwrap();
            let shift = RationalTransform::translation([1, 2, 3], 2).unwrap();
            apply(
                TypedTransform::Rational(turn.compose(&shift).unwrap()),
                m::apply_rational(class),
            )
        }
        "apply_certified" => {
            let turn = Transform::rotation(Axis::Z, 1.0 / 3.0).unwrap();
            let shift = Transform::translation(F3::new(0.25, -1.0, 0.375).unwrap());
            apply(
                TypedTransform::Certified(shift.compose(&turn).unwrap()),
                m::apply_certified(class),
            )
        }
        "rotation_apply" => {
            let r = RationalRotation::new(kernels::math_types::BIG_QUATERNION).unwrap();
            fold(m::rotation_apply(class).into_iter().map(|p| r.apply(p)))
        }
        "plane_within" => fold(
            m::plane_within(class)
                .into_iter()
                .map(|d| RationalRotation::plane_within(d, geom_num::ladder::ANGULAR)),
        ),
        "to_map" => {
            let frame = inputs::math_types::ground_frame().unwrap();
            fold(m::to_map(class).into_iter().map(|p| frame.to_map(p)))
        }
        "decimal_parse" => fold(m::decimal_parse(class).iter().map(|s| Decimal::parse(s))),
        _ => return None,
    })
}

/// The digest of the double-double benchmark `name` on `class`, from direct calls on its inputs.
fn dd_digest(name: &str, class: Class) -> Option<Digest> {
    use inputs::dd as i;
    let unary = |f: fn(DD) -> Result<DD, NumError>, xs: Vec<DD>| fold(xs.into_iter().map(f));
    let with_f64 = |f: fn(DD, f64) -> Result<DD, NumError>, xs: Vec<(DD, f64)>| {
        fold(xs.into_iter().map(|(x, y)| f(x, y)))
    };
    let with_dd = |f: fn(DD, DD) -> Result<DD, NumError>, xs: Vec<(DD, DD)>| {
        fold(xs.into_iter().map(|(x, y)| f(x, y)))
    };
    Some(match name {
        "two_prod_fma" => fold(
            i::two_prod_fma(class)
                .into_iter()
                .map(|(a, b)| dd::two_prod_fma(a, b)),
        ),
        "from_sum" => fold(
            i::from_sum(class)
                .into_iter()
                .map(|(a, b)| DD::from_sum(a, b)),
        ),
        "exact_product" => fold(
            i::exact_product(class)
                .into_iter()
                .map(|(a, b)| DD::exact_product(a, b)),
        ),
        "from_i64" => fold(i::from_i64(class).into_iter().map(DD::from_i64)),
        "from_i128" => fold(i::from_i128(class).into_iter().map(DD::from_i128)),
        "sign" => fold(i::sign(class).into_iter().map(DD::sign)),
        "negate" => fold(i::negate(class).into_iter().map(DD::negate)),
        "abs" => fold(i::abs(class).into_iter().map(DD::abs)),
        "add" => with_dd(DD::add, i::add(class)),
        "add_f64" => with_f64(DD::add_f64, i::add_f64(class)),
        "mul_f64" => with_f64(DD::mul_f64, i::mul_f64(class)),
        "mul" => with_dd(DD::mul, i::mul(class)),
        "div_f64" => with_f64(DD::div_f64, i::div_f64(class)),
        "div" => with_dd(DD::div, i::div(class)),
        "sqrt" => unary(DD::sqrt, i::sqrt(class)),
        "round_shifted" => fold(
            i::round_shifted(class)
                .into_iter()
                .map(|(x, k)| x.round_shifted(k)),
        ),
        "cmp" => fold(i::cmp(class).into_iter().map(|(x, y)| x.cmp(&y))),
        _ => return None,
    })
}

/// The digest of the exact-sum benchmark `name` on `class`, from direct calls on its inputs.
fn exact_sum_digest(name: &str, class: Class) -> Option<Digest> {
    use inputs::exact_sum as i;
    let fresh = |add: &dyn Fn(&mut ExactSum)| {
        let mut sum = ExactSum::new();
        add(&mut sum);
        sum
    };
    Some(match name {
        "add" => fold(
            i::add(class)
                .iter()
                .map(|terms| fresh(&|s: &mut ExactSum| terms.iter().for_each(|&x| s.add(x)))),
        ),
        "add_slice" => fold(
            i::add_slice(class)
                .iter()
                .map(|terms| fresh(&|s: &mut ExactSum| s.add_slice(terms))),
        ),
        "add_iter" => fold(i::add_iter(class).iter().map(|(terms, known)| {
            fresh(&|s: &mut ExactSum| {
                if *known {
                    s.add_iter(terms.iter().copied());
                } else {
                    let mut rest = terms.iter().copied();
                    s.add_iter(core::iter::from_fn(move || rest.next()));
                }
            })
        })),
        "merge" => fold(i::merge(class).iter().map(|(a, b)| {
            let mut sum = a.clone();
            sum.merge(b);
            sum
        })),
        "round" => fold(i::round(class).iter().map(ExactSum::round)),
        "round_ieee" => fold(i::round_ieee(class).iter().map(ExactSum::round_ieee)),
        _ => return None,
    })
}

/// The digest of the generated-predicate benchmark `name` on `class`, from direct calls on its
/// inputs.
fn generated_digest(name: &str, class: Class) -> Option<Digest> {
    use geom_num::generated as gp;
    use inputs::generated as i;
    Some(match name {
        "orient2d_filter" => fold(
            i::orient2d_filter(class)
                .into_iter()
                .map(|[a, b, c]| gp::orient2d::filter(a, b, c)),
        ),
        "orient2d_value" => fold(
            i::orient2d_value(class)
                .into_iter()
                .map(|[a, b, c]| gp::orient2d::value(a, b, c)),
        ),
        "orient3d_filter" => fold(
            i::orient3d_filter(class)
                .into_iter()
                .map(|[a, b, c, d]| gp::orient3d::filter(a, b, c, d)),
        ),
        "orient3d_value" => fold(
            i::orient3d_value(class)
                .into_iter()
                .map(|[a, b, c, d]| gp::orient3d::value(a, b, c, d)),
        ),
        "incircle_filter" => fold(
            i::incircle_filter(class)
                .into_iter()
                .map(|[a, b, c, d]| gp::incircle::filter(a, b, c, d)),
        ),
        "incircle_value" => fold(
            i::incircle_value(class)
                .into_iter()
                .map(|[a, b, c, d]| gp::incircle::value(a, b, c, d)),
        ),
        "insphere_filter" => fold(
            i::insphere_filter(class)
                .into_iter()
                .map(|[a, b, c, d, e]| gp::insphere::filter(a, b, c, d, e)),
        ),
        "insphere_value" => fold(
            i::insphere_value(class)
                .into_iter()
                .map(|[a, b, c, d, e]| gp::insphere::value(a, b, c, d, e)),
        ),
        "plane_side_filter" => fold(
            i::plane_side_filter(class)
                .into_iter()
                .map(|([x, y, z], d, p)| gp::plane_side::filter(x, y, z, d, p)),
        ),
        "plane_side_value" => fold(
            i::plane_side_value(class)
                .into_iter()
                .map(|([x, y, z], d, p)| gp::plane_side::value(x, y, z, d, p)),
        ),
        "tpi_weight_filter" => fold(i::tpi_weight_filter(class).into_iter().map(
            |[[ax, ay, az], [bx, by, bz], [cx, cy, cz]]| {
                gp::tpi_weight::filter(ax, ay, az, bx, by, bz, cx, cy, cz)
            },
        )),
        "tpi_side_filter" => fold(i::tpi_side_filter(class).into_iter().map(
            |[
                ([ax, ay, az], ad),
                ([bx, by, bz], bd),
                ([cx, cy, cz], cd),
                ([sx, sy, sz], sd),
            ]| {
                gp::tpi_side::filter(
                    ax, ay, az, ad, bx, by, bz, bd, cx, cy, cz, cd, sx, sy, sz, sd,
                )
            },
        )),
        "lpi_weight_filter" => fold(
            i::lpi_weight_filter(class)
                .into_iter()
                .map(|([nx, ny, nz], p, q)| gp::lpi_weight::filter(nx, ny, nz, p, q)),
        ),
        "lpi_side_filter" => fold(i::lpi_side_filter(class).into_iter().map(
            |(([ax, ay, az], ad), ([bx, by, bz], bd), p, q)| {
                gp::lpi_side::filter(ax, ay, az, ad, bx, by, bz, bd, p, q)
            },
        )),
        "ssi_weight_filter" => fold(
            i::ssi_weight_filter(class)
                .into_iter()
                .map(|[a, b, c, d]| gp::ssi_weight::filter(a, b, c, d)),
        ),
        "ssi_side_filter" => fold(
            i::ssi_side_filter(class)
                .into_iter()
                .map(|([a, b, c, d], p, q)| gp::ssi_side::filter(a, b, c, d, p, q)),
        ),
        "tpi_weight_value" => fold(i::tpi_weight_value(class).into_iter().map(
            |[[ax, ay, az], [bx, by, bz], [cx, cy, cz]]| {
                gp::tpi_weight::value(ax, ay, az, bx, by, bz, cx, cy, cz)
            },
        )),
        "tpi_side_value" => fold(i::tpi_side_value(class).into_iter().map(
            |[
                ([ax, ay, az], ad),
                ([bx, by, bz], bd),
                ([cx, cy, cz], cd),
                ([sx, sy, sz], sd),
            ]| {
                gp::tpi_side::value(
                    ax, ay, az, ad, bx, by, bz, bd, cx, cy, cz, cd, sx, sy, sz, sd,
                )
            },
        )),
        "lpi_weight_value" => fold(
            i::lpi_weight_value(class)
                .into_iter()
                .map(|([nx, ny, nz], p, q)| gp::lpi_weight::value(nx, ny, nz, p, q)),
        ),
        "lpi_side_value" => fold(i::lpi_side_value(class).into_iter().map(
            |(([ax, ay, az], ad), ([bx, by, bz], bd), p, q)| {
                gp::lpi_side::value(ax, ay, az, ad, bx, by, bz, bd, p, q)
            },
        )),
        "ssi_weight_value" => fold(
            i::ssi_weight_value(class)
                .into_iter()
                .map(|[a, b, c, d]| gp::ssi_weight::value(a, b, c, d)),
        ),
        "ssi_side_value" => fold(
            i::ssi_side_value(class)
                .into_iter()
                .map(|([a, b, c, d], p, q)| gp::ssi_side::value(a, b, c, d, p, q)),
        ),
        // The plain-i128 variant (piece G42): the shipped value on the variant's inputs, so a
        // variant that returned anything else fails its digest.
        "orient2d_value_plain" => fold(
            i::orient2d_value_plain(class)
                .into_iter()
                .map(|[a, b, c]| gp::orient2d::value(a, b, c)),
        ),
        "orient3d_value_plain" => fold(
            i::orient3d_value_plain(class)
                .into_iter()
                .map(|[a, b, c, d]| gp::orient3d::value(a, b, c, d)),
        ),
        "incircle_value_plain" => fold(
            i::incircle_value_plain(class)
                .into_iter()
                .map(|[a, b, c, d]| gp::incircle::value(a, b, c, d)),
        ),
        "insphere_value_plain" => fold(
            i::insphere_value_plain(class)
                .into_iter()
                .map(|[a, b, c, d, e]| gp::insphere::value(a, b, c, d, e)),
        ),
        "plane_side_value_plain" => fold(
            i::plane_side_value_plain(class)
                .into_iter()
                .map(|([x, y, z], d, p)| gp::plane_side::value(x, y, z, d, p)),
        ),
        "tpi_weight_value_plain" => fold(i::tpi_weight_value_plain(class).into_iter().map(
            |[[ax, ay, az], [bx, by, bz], [cx, cy, cz]]| {
                gp::tpi_weight::value(ax, ay, az, bx, by, bz, cx, cy, cz)
            },
        )),
        "tpi_side_value_plain" => fold(i::tpi_side_value_plain(class).into_iter().map(
            |[
                ([ax, ay, az], ad),
                ([bx, by, bz], bd),
                ([cx, cy, cz], cd),
                ([sx, sy, sz], sd),
            ]| {
                gp::tpi_side::value(
                    ax, ay, az, ad, bx, by, bz, bd, cx, cy, cz, cd, sx, sy, sz, sd,
                )
            },
        )),
        "lpi_weight_value_plain" => fold(
            i::lpi_weight_value_plain(class)
                .into_iter()
                .map(|([nx, ny, nz], p, q)| gp::lpi_weight::value(nx, ny, nz, p, q)),
        ),
        "lpi_side_value_plain" => fold(i::lpi_side_value_plain(class).into_iter().map(
            |(([ax, ay, az], ad), ([bx, by, bz], bd), p, q)| {
                gp::lpi_side::value(ax, ay, az, ad, bx, by, bz, bd, p, q)
            },
        )),
        "ssi_weight_value_plain" => fold(
            i::ssi_weight_value_plain(class)
                .into_iter()
                .map(|[a, b, c, d]| gp::ssi_weight::value(a, b, c, d)),
        ),
        "ssi_side_value_plain" => fold(
            i::ssi_side_value_plain(class)
                .into_iter()
                .map(|([a, b, c, d], p, q)| gp::ssi_side::value(a, b, c, d, p, q)),
        ),
        // Avnaim et al.'s sign of orient2d (piece G42): the shipped exact sign on its inputs.
        "orient2d_avnaim" => fold(
            i::orient2d_avnaim(class)
                .into_iter()
                .map(|[a, b, c]| gp::orient2d::exact(a, b, c)),
        ),
        "orient3d_avnaim" => fold(
            i::orient3d_avnaim(class)
                .into_iter()
                .map(|[a, b, c, d]| gp::orient3d::exact(a, b, c, d)),
        ),
        // The width-aware limb variant (piece G42): the shipped value on the variant's inputs,
        // so a variant that returned anything else fails its digest.
        "orient2d_value_limbs" => fold(
            i::orient2d_value_limbs(class)
                .into_iter()
                .map(|[a, b, c]| gp::orient2d::value(a, b, c)),
        ),
        "orient3d_value_limbs" => fold(
            i::orient3d_value_limbs(class)
                .into_iter()
                .map(|[a, b, c, d]| gp::orient3d::value(a, b, c, d)),
        ),
        "incircle_value_limbs" => fold(
            i::incircle_value_limbs(class)
                .into_iter()
                .map(|[a, b, c, d]| gp::incircle::value(a, b, c, d)),
        ),
        "insphere_value_limbs" => fold(
            i::insphere_value_limbs(class)
                .into_iter()
                .map(|[a, b, c, d, e]| gp::insphere::value(a, b, c, d, e)),
        ),
        "plane_side_value_limbs" => fold(
            i::plane_side_value_limbs(class)
                .into_iter()
                .map(|([x, y, z], d, p)| gp::plane_side::value(x, y, z, d, p)),
        ),
        "tpi_weight_value_limbs" => fold(i::tpi_weight_value_limbs(class).into_iter().map(
            |[[ax, ay, az], [bx, by, bz], [cx, cy, cz]]| {
                gp::tpi_weight::value(ax, ay, az, bx, by, bz, cx, cy, cz)
            },
        )),
        "tpi_side_value_limbs" => fold(i::tpi_side_value_limbs(class).into_iter().map(
            |[
                ([ax, ay, az], ad),
                ([bx, by, bz], bd),
                ([cx, cy, cz], cd),
                ([sx, sy, sz], sd),
            ]| {
                gp::tpi_side::value(
                    ax, ay, az, ad, bx, by, bz, bd, cx, cy, cz, cd, sx, sy, sz, sd,
                )
            },
        )),
        "lpi_weight_value_limbs" => fold(
            i::lpi_weight_value_limbs(class)
                .into_iter()
                .map(|([nx, ny, nz], p, q)| gp::lpi_weight::value(nx, ny, nz, p, q)),
        ),
        "lpi_side_value_limbs" => fold(i::lpi_side_value_limbs(class).into_iter().map(
            |(([ax, ay, az], ad), ([bx, by, bz], bd), p, q)| {
                gp::lpi_side::value(ax, ay, az, ad, bx, by, bz, bd, p, q)
            },
        )),
        "ssi_weight_value_limbs" => fold(
            i::ssi_weight_value_limbs(class)
                .into_iter()
                .map(|[a, b, c, d]| gp::ssi_weight::value(a, b, c, d)),
        ),
        "ssi_side_value_limbs" => fold(
            i::ssi_side_value_limbs(class)
                .into_iter()
                .map(|([a, b, c, d], p, q)| gp::ssi_side::value(a, b, c, d, p, q)),
        ),
        _ => return None,
    })
}

/// The digest of the random-number benchmark `name` on `class`, from direct calls on its inputs.
fn rng_digest(name: &str, class: Class) -> Option<Digest> {
    use inputs::rng as i;
    Some(match name {
        "philox4x32_10" => fold(
            i::philox4x32_10(class)
                .into_iter()
                .map(|(c, k)| rng::philox4x32_10(c, k)),
        ),
        "stream_from" => fold(
            i::stream_from(class)
                .into_iter()
                .map(|(p, b)| p.stream_from(b)),
        ),
        "next_u32" => fold(i::next_u32(class).into_iter().map(|mut s| s.next_u32())),
        "next_u64" => fold(i::next_u64(class).into_iter().map(|mut s| s.next_u64())),
        "next_f64" => fold(i::next_f64(class).into_iter().map(|mut s| s.next_f64())),
        "below" => fold(i::below(class).into_iter().map(|(mut s, n)| s.below(n))),
        "range_inclusive" => fold(
            i::range_inclusive(class)
                .into_iter()
                .map(|(mut s, lo, hi)| s.range_inclusive(lo, hi)),
        ),
        "range_philox" => fold(i::range_philox(class).into_iter().map(|(r, k)| r.philox(k))),
        "chunk" => fold(i::chunk(class).into_iter().map(|(r, k, n)| r.chunk(k, n))),
        _ => return None,
    })
}

#[test]
fn the_rewritten_digest_matches_the_kernels() {
    let batch = inputs::predicate::orient2d(Typical);
    let direct = fold(batch.iter().map(|&[a, b, c]| predicate::orient2d(a, b, c)));
    assert_eq!(direct, kernel_digest(kernels::predicate::orient2d, &batch));
    let case = cases()
        .find(|c| c.name() == "predicate/orient2d/typical")
        .unwrap();
    assert_eq!(case.prepare().run().outputs, direct);
    let batch = inputs::plane::to_grid(Typical);
    let direct = fold(batch.iter().map(Tpi::to_grid));
    assert_eq!(direct, kernel_digest(kernels::plane::to_grid, &batch));
    // The input digest is the same encoding over the batch.
    let inputs = canonical::digest(&inputs::predicate::orient2d(Typical));
    assert_eq!(case.prepare().run().inputs, inputs);
}

#[test]
fn the_canonical_layout_is_the_documented_one() {
    // geom_bench::canonical: integers little-endian at full width, doubles as their bits, a tag
    // byte before every Option and Result, a Rounded value then its flag.
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(-3i64).to_le_bytes());
    bytes.extend_from_slice(&7i64.to_le_bytes());
    assert_eq!(fold([P2::new(-3, 7).unwrap()]), Sha256::digest(&bytes));
    // 126 steps of Q1 are 5.04 steps of Q0: 5, rounded.
    let rounded = grid::coarsen(Coord::new(126).unwrap(), GridStep::Q1, GridStep::Q0);
    assert_eq!(parts(rounded), Ok((Coord::new(5).unwrap(), false)));
    let mut bytes = vec![0u8];
    bytes.extend_from_slice(&5i64.to_le_bytes());
    bytes.push(0);
    assert_eq!(fold([rounded]), Sha256::digest(&bytes));
    assert_eq!(
        fold([-0.0f64]),
        Sha256::digest(&(-0.0f64).to_bits().to_le_bytes())
    );
}

#[test]
fn every_benchmark_runs_the_operation_it_is_named_after() {
    let mut unknown = Vec::new();
    for case in cases() {
        match direct_digest(case.op.group, case.op.name, case.class) {
            Some(direct) => assert_eq!(case.prepare().run().outputs, direct, "{}", case.name()),
            None => unknown.push(case.name()),
        }
    }
    for case in growth_cases() {
        match growth_digest(case.growth.name, case.size) {
            Some(direct) => {
                let prepared = case.prepare().unwrap().run();
                assert_eq!(prepared.outputs, direct, "{}", case.name());
            }
            None => unknown.push(case.name()),
        }
    }
    assert!(
        unknown.is_empty(),
        "benchmarks of operations no test knows: {unknown:?}"
    );
}

#[test]
fn every_kernel_is_never_inlined() {
    // Claim 4 of the crate documentation: the warm-up runs the very instructions then measured.
    let public = KERNELS
        .lines()
        .filter(|l| l.trim_start().starts_with("pub fn "))
        .count();
    let never = KERNELS
        .lines()
        .filter(|l| l.trim() == "#[inline(never)]")
        .count();
    assert!(public > 0);
    assert_eq!(
        public, never,
        "every pub fn in kernels.rs must be #[inline(never)]"
    );
}

#[test]
fn the_fuel_loop_is_never_inlined() {
    // geom_bench::guest: the fuel of one pass is the difference of two runs, `run <name> 1` and
    // `run <name> 2`, so nothing but the loop may use the pass count (a test of `passes == 2`
    // added to `main` moved every count by about 20), and the loop sits in its own never-inlined
    // function, whose code cannot depend on the code around its call.
    let guest: Vec<&str> = include_str!("../src/guest.rs")
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("//"))
        .collect();
    let at = guest
        .iter()
        .position(|line| line.starts_with("pub fn run_passes("))
        .expect("guest.rs has run_passes");
    assert_eq!(guest[at - 1], "#[inline(never)]");
    let loops = guest.iter().filter(|line| line.starts_with("for ")).count();
    assert_eq!(loops, 1, "the passes run in run_passes only");
    // Every use of the count: the pattern that names it, its parsing, the call, and the loop. A
    // line that starts with a string literal (the usage text) only mentions the word.
    let code = || guest.iter().copied().filter(|line| !line.starts_with('"'));
    let uses: Vec<&str> = code()
        .filter(|line| line.contains("passes") && !line.contains("run_passes("))
        .chain(code().filter(|line| line.contains("run_passes(")))
        .collect();
    assert_eq!(
        uses,
        [
            "[\"run\", name, passes] => {",
            "let Ok(passes) = passes.parse::<u8>() else {",
            "for _ in 0..passes {",
            "run_passes(&mut *bench, passes);",
            "pub fn run_passes(bench: &mut dyn Bench, passes: u8) {",
        ],
        "the pass count is used outside run_passes"
    );
    // And it runs the passes it is asked for.
    let mut bench = cases().next().unwrap().prepare();
    geom_bench::guest::run_passes(&mut *bench, 2);
    assert_eq!(bench.digests(), cases().next().unwrap().prepare().run());
}

// ------------------------------------------------------------------------------------------------
// Exact oracles on every benchmark input.
// ------------------------------------------------------------------------------------------------

#[test]
fn the_oracle_signs_are_calibrated_by_geometry() {
    let q2 = |x, y| P2::new(x, y).unwrap();
    let q3 = |x, y, z| P3::new(x, y, z).unwrap();
    // Counter-clockwise; (1, 1) is inside the circle through (0, 0), (4, 0), (0, 4).
    let ccw = [q2(0, 0), q2(4, 0), q2(0, 4)];
    assert_eq!(orient2d_exact(ccw).sign(), Sign::Positive);
    assert_eq!(
        incircle_exact([ccw[0], ccw[1], ccw[2], q2(1, 1)]).sign(),
        Sign::Positive
    );
    // d on the side (b − a) × (c − a) = +z points to; (1, 1, 1) is inside the sphere through the
    // four, whose centre is (2, 2, 2) with radius² 12.
    let tet = [q3(0, 0, 0), q3(4, 0, 0), q3(0, 4, 0), q3(0, 0, 4)];
    assert_eq!(orient3d_exact(tet).sign(), Sign::Positive);
    let inside = [tet[0], tet[1], tet[2], tet[3], q3(1, 1, 1)];
    assert_eq!(insphere_exact(inside).sign(), Sign::Positive);
    assert_eq!(
        predicate::insphere(tet[0], tet[1], tet[2], tet[3], q3(1, 1, 1)),
        Ok(Sign::Positive)
    );
}

#[test]
fn predicates_agree_with_exact_determinants_on_every_input() {
    use inputs::predicate as q;
    for class in [Typical, Degenerate, NearlyDegenerate, Extreme] {
        // On grid points every answer is the exact sign, never `BudgetExceeded`.
        for t @ [a, b, c] in q::orient2d(class) {
            assert_eq!(
                predicate::orient2d(a, b, c),
                Ok(orient2d_exact(t).sign()),
                "{class} {t:?}"
            );
        }
        for t @ [a, b, c, d] in q::orient3d(class) {
            assert_eq!(
                predicate::orient3d(a, b, c, d),
                Ok(orient3d_exact(t).sign()),
                "{class} {t:?}"
            );
        }
        for t @ [a, b, c, d] in q::incircle(class) {
            assert_eq!(
                predicate::incircle(a, b, c, d),
                Ok(incircle_exact(t).sign()),
                "{class} {t:?}"
            );
        }
        for t @ [a, b, c, d, e] in q::insphere(class) {
            let got = predicate::insphere(a, b, c, d, e);
            assert_eq!(got, Ok(insphere_exact(t).sign()), "{class} {t:?}");
        }
    }
}

#[test]
fn extreme_predicate_inputs_start_at_the_documented_largest_values() {
    // inputs/predicate.rs: "the first input is the configuration that reaches the predicate's
    // documented largest determinant": 4M², 16M³, 8M⁴ and 48M⁵.
    use inputs::predicate as q;
    let m = Big::from_i64(COORD_MAX);
    let power = |k: u32| (1..k).fold(m.clone(), |acc, _| acc.times(&m));
    let times = |k: i64, v: Big| Big::from_i64(k).times(&v);
    assert_eq!(
        orient2d_exact(q::orient2d(Extreme)[0]).magnitude(),
        times(4, power(2))
    );
    assert_eq!(
        orient3d_exact(q::orient3d(Extreme)[0]).magnitude(),
        times(16, power(3))
    );
    assert_eq!(
        incircle_exact(q::incircle(Extreme)[0]).magnitude(),
        times(8, power(4))
    );
    assert_eq!(
        insphere_exact(q::insphere(Extreme)[0]).magnitude(),
        times(48, power(5))
    );
}

#[test]
fn the_extreme_plane_reaches_the_largest_normal_the_frame_allows() {
    // inputs/plane.rs used to claim "every normal component reaches 8M²". That contradicted exact
    // arithmetic: n_x = u_y v_z − u_z v_y is twice the signed area of the triangle projected on the
    // yz plane, and a triangle inside a square of side 2M has area at most half the square, 2M², so
    // |n_x| ≤ 4M² (likewise n_y, n_z). The bound 2(2M)² = 8M² is not reachable. The documentation
    // now says 4M²; this checks the corrected claim, and that it is the maximum: the first input
    // reaches 4M² in every component, and no extreme input exceeds it.
    let m = Big::from_i64(COORD_MAX);
    let four_m2 = Big::from_i64(4).times(&m).times(&m);
    let first = inputs::plane::through(Extreme)[0];
    let (n, _) = plane_exact(first);
    let reached: Vec<Big> = n.iter().map(Big::magnitude).collect();
    assert!(
        reached.iter().all(|c| *c == four_m2),
        "normal components are {:?}, not 4M² = {:?}",
        reached.iter().map(Big::to_i128).collect::<Vec<_>>(),
        four_m2.to_i128()
    );
    for t in inputs::plane::through(Extreme) {
        let (n, _) = plane_exact(t);
        assert!(n.iter().all(|c| c.magnitude() <= four_m2), "{t:?}");
    }
    let doc = include_str!("../src/inputs/plane.rs");
    assert!(!doc.contains("8M²") && doc.contains("4M²"));
}

#[test]
fn planes_agree_with_exact_values_on_every_input() {
    use inputs::plane as p;
    for class in [Typical, Degenerate, NearlyDegenerate, Extreme] {
        for t @ [a, b, c] in p::through(class) {
            let (n, d) = plane_exact(t);
            match Plane::through(a, b, c) {
                Ok(plane) => {
                    assert_eq!(coefficients(&plane), (n, d), "{class} {t:?}");
                }
                Err(e) => {
                    assert_eq!(e, NumError::Degenerate);
                    assert!(n.iter().all(Big::is_zero), "{class} {t:?}: not collinear");
                }
            }
        }
        for (plane, q) in p::side(class) {
            let (n, d) = coefficients(&plane);
            let v = [big(q.x), big(q.y), big(q.z)];
            let value = n
                .iter()
                .zip(&v)
                .fold(d, |acc, (ni, vi)| acc.plus(&ni.times(vi)));
            assert_eq!(plane.side(q), Ok(value.sign()), "{class}");
        }
        for planes @ [a, b, c] in p::tpi_new(class) {
            let rows: Vec<Vec<Big>> = planes.iter().map(|q| coefficients(q).0.to_vec()).collect();
            let w = det(&rows);
            match Tpi::new(a, b, c) {
                Ok(_) => assert!(!w.is_zero(), "{class}: dependent normals accepted"),
                Err(e) => assert!(e == NumError::Degenerate && w.is_zero(), "{class}: {e:?}"),
            }
        }
    }
}

#[test]
fn three_plane_points_agree_with_cramers_rule_on_every_input() {
    use inputs::plane as p;
    for class in [Typical, Extreme] {
        for t in p::tpi_homogeneous(class) {
            let (x, w) = t.homogeneous().unwrap();
            let (ex, ew) = tpi_exact(&t);
            assert_eq!(big_of_wide(&w), ew, "{class}");
            assert_eq!(x.map(|xi| big_of_wide(&xi)), ex, "{class}");
        }
    }
    for class in [Typical, Degenerate, NearlyDegenerate, Extreme] {
        for (plane, t) in p::side_of_tpi(class) {
            let (value, w) = side_of_tpi_value(&plane, &t);
            assert_eq!(
                plane.side_of_tpi(&t),
                Ok(value.sign().mul(w.sign())),
                "{class}"
            );
        }
    }
    for class in [Typical, OnGrid, Extreme, Overflow] {
        for t in p::to_grid(class) {
            assert_eq!(parts(t.to_grid()), to_grid_exact(&t), "{class}");
        }
    }
    // inputs/plane.rs, `far_tpi`: the overflow points lie beyond the frame but below 2^60.
    for t in p::to_grid(Overflow) {
        let Err(NumError::OutOfRange { value }) = to_grid_exact(&t) else {
            panic!("an overflow input is inside the frame");
        };
        assert!(value.unsigned_abs() < 1 << 60, "{value}");
    }
}

#[test]
fn extreme_planes_reach_the_largest_values_the_frame_allows() {
    // The extreme class is meant to drive the wide arithmetic at its largest widths. geom-num's
    // plane module: planes from the corners of the frame's cube reach 82 bits for n, 122 for d,
    // 248 for w, 288 for X and 372 for n · X + d · w. The extreme batches should come within a few
    // bits of each at least once (they are not required to every time).
    let most = |bits: &[u32]| bits.iter().copied().max().unwrap_or(0);
    let planes = inputs::plane::through(Extreme);
    let n_bits: Vec<u32> = planes
        .iter()
        .map(|&t| plane_exact(t).0.iter().map(Big::bit_len).max().unwrap_or(0))
        .collect();
    let d_bits: Vec<u32> = planes.iter().map(|&t| plane_exact(t).1.bit_len()).collect();
    let points = inputs::plane::tpi_homogeneous(Extreme);
    let w_bits: Vec<u32> = points.iter().map(|t| tpi_exact(t).1.bit_len()).collect();
    let x_bits: Vec<u32> = points
        .iter()
        .map(|t| tpi_exact(t).0.iter().map(Big::bit_len).max().unwrap_or(0))
        .collect();
    let side_bits: Vec<u32> = inputs::plane::side_of_tpi(Extreme)
        .iter()
        .map(|(p, t)| side_of_tpi_value(p, t).0.bit_len())
        .collect();
    for (what, bits, reachable) in [
        ("n", &n_bits, 82),
        ("d", &d_bits, 122),
        ("w", &w_bits, 248),
        ("X", &x_bits, 288),
        ("n · X + d · w", &side_bits, 372),
    ] {
        assert!(
            most(bits) + 4 >= reachable,
            "{what}: bit lengths {bits:?}, reachable {reachable}"
        );
    }
}

fn check_wide<const L: usize>() {
    use inputs::wide as w;
    type Build<const L: usize> = fn(Class) -> Vec<(Wide<L>, Wide<L>)>;
    type Op<const L: usize> = fn(&Wide<L>, &Wide<L>) -> Option<Wide<L>>;
    type Exact = fn(&Big, &Big) -> Big;
    let ops: [(Build<L>, Op<L>, Exact); 3] = [
        (w::add::<L>, Wide::checked_add, |a, b| a.plus(b)),
        (w::sub::<L>, Wide::checked_sub, |a, b| a.minus(b)),
        (w::mul::<L>, Wide::checked_mul, |a, b| a.times(b)),
    ];
    for (build, op, exact) in ops {
        for class in [Typical, Extreme, Overflow] {
            for (x, y) in build(class) {
                let want = wide_of_big::<L>(&exact(&big_of_wide(&x), &big_of_wide(&y)));
                assert_eq!(op(&x, &y), want, "L = {L}, {class}, {x:?}, {y:?}");
            }
        }
    }
    for class in [Typical, Extreme, Overflow] {
        for x in w::neg::<L>(class) {
            assert_eq!(
                x.checked_neg(),
                wide_of_big::<L>(&big_of_wide(&x).negated()),
                "neg {x:?}"
            );
        }
        for x in w::to_i128::<L>(class) {
            assert_eq!(x.to_i128(), big_of_wide(&x).to_i128(), "to_i128 {x:?}");
        }
    }
    for class in [Typical, Extreme] {
        for (x, y) in w::cmp::<L>(class) {
            assert_eq!(
                x.cmp(&y),
                big_of_wide(&x).cmp(&big_of_wide(&y)),
                "cmp {x:?} {y:?}"
            );
        }
    }
    for x in w::signum::<L>(Typical) {
        assert_eq!(x.signum(), big_of_wide(&x).sign(), "signum {x:?}");
    }
    for v in w::from_i128::<L>(Typical) {
        assert_eq!(
            big_of_wide(&Wide::<L>::from_i128(v)),
            Big::from_i128(v),
            "from_i128 {v}"
        );
    }
}

#[test]
fn width_changes_agree_with_exact_values_on_every_input() {
    use inputs::growth as g;
    use inputs::wide as w;
    fn at<const L: usize>() {
        for (x, y) in g::add::<L>() {
            let want = wide_of_big::<L>(&big_of_wide(&x).plus(&big_of_wide(&y)));
            assert_eq!(x.checked_add(&y), want);
        }
        for (x, y) in g::sub::<L>() {
            let want = wide_of_big::<L>(&big_of_wide(&x).minus(&big_of_wide(&y)));
            assert_eq!(x.checked_sub(&y), want);
        }
        for (x, y) in g::mul::<L>() {
            let want = wide_of_big::<L>(&big_of_wide(&x).times(&big_of_wide(&y)));
            assert_eq!(x.checked_mul(&y), want);
        }
        for x in g::neg::<L>() {
            assert_eq!(
                x.checked_neg(),
                wide_of_big::<L>(&big_of_wide(&x).negated())
            );
        }
        for (x, y) in g::cmp::<L>() {
            assert_eq!(x.cmp(&y), big_of_wide(&x).cmp(&big_of_wide(&y)));
        }
    }
    for class in [Typical, Overflow] {
        for x in w::resize::<4, 6>(class) {
            assert_eq!(x.resize::<6>(), wide_of_big::<6>(&big_of_wide(&x)));
        }
        for x in w::resize::<5, 6>(class) {
            assert_eq!(x.resize::<6>(), wide_of_big::<6>(&big_of_wide(&x)));
        }
        for x in w::resize::<6, 4>(class) {
            assert_eq!(x.resize::<4>(), wide_of_big::<4>(&big_of_wide(&x)));
        }
    }
    at::<2>();
    at::<4>();
    at::<8>();
}

#[test]
fn scalar_operations_agree_with_their_definitions_on_every_input() {
    use inputs::{exactness as ix, grid as ig, interval as ii, real as ir, sign as is};
    let m = i128::from(COORD_MAX);
    for v in is::of_i128(Typical) {
        assert_eq!(Sign::of_i128(v), Big::from_i128(v).sign());
    }
    for v in ig::coord_new(Typical)
        .into_iter()
        .chain(ig::coord_new(Overflow))
    {
        let inside = i128::from(v).abs() <= m;
        assert_eq!(Coord::new(v).is_ok(), inside, "{v}");
        if !inside {
            assert_eq!(
                Coord::new(v),
                Err(NumError::OutOfRange {
                    value: i128::from(v)
                })
            );
        }
    }
    for v in ig::coord_from_i128(Typical)
        .into_iter()
        .chain(ig::coord_from_i128(Overflow))
    {
        let got = Coord::from_i128(v).map(|c| i128::from(c.get())).ok();
        assert_eq!(
            got,
            (v.unsigned_abs() <= m.unsigned_abs()).then_some(v),
            "{v}"
        );
    }
    for (x, y, z) in ig::p3_new(Typical).into_iter().chain(ig::p3_new(Overflow)) {
        let inside = [x, y, z].iter().all(|&c| i128::from(c).abs() <= m);
        assert_eq!(P3::new(x, y, z).is_ok(), inside);
    }
    for (x, y) in ig::p2_new(Typical).into_iter().chain(ig::p2_new(Overflow)) {
        assert_eq!(
            P2::new(x, y).is_ok(),
            [x, y].iter().all(|&c| i128::from(c).abs() <= m)
        );
    }
    for class in [Typical, Extreme] {
        for (lo, hi) in ii::new(class) {
            // geom_num::interval: NotFinite first, then EmptyInterval; zero bounds stored as +0.
            let want = if !lo.is_finite() || !hi.is_finite() {
                Err(NumError::NotFinite)
            } else if lo > hi {
                Err(NumError::EmptyInterval)
            } else {
                Ok(((lo + 0.0).to_bits(), (hi + 0.0).to_bits()))
            };
            let got = Interval::new(lo, hi).map(|x| (x.lo().to_bits(), x.hi().to_bits()));
            assert_eq!(got, want, "new({lo:e}, {hi:e})");
        }
    }
    for (x, v) in ii::contains(Typical) {
        assert_eq!(x.contains(v), x.lo() <= v && v <= x.hi());
    }
    for (x, y) in ir::cmp(Typical) {
        assert_eq!(x.cmp(&y), x.get().total_cmp(&y.get()));
    }
    for (x, y) in ir::total_max(Typical) {
        let want: Real = if x.get().total_cmp(&y.get()) == Ordering::Less {
            y
        } else {
            x
        };
        assert_eq!(x.total_max(y).get().to_bits(), want.get().to_bits());
    }
    for (x, y) in ir::total_min(Typical) {
        let want: Real = if x.get().total_cmp(&y.get()) == Ordering::Greater {
            y
        } else {
            x
        };
        assert_eq!(x.total_min(y).get().to_bits(), want.get().to_bits());
    }
    for (x, y) in ix::combine(Typical) {
        // Chapter 9: the less exact class wins, and bounds add in the finer step, never saturated.
        assert_eq!(x.combine(y), combine_exact(x, y), "{x:?} {y:?}");
    }
    for (x, ceiling, step) in ix::within(Typical) {
        assert_eq!(
            x.within(ceiling, step),
            within_exact(x, ceiling, step),
            "{x:?} {ceiling:?} {step:?}"
        );
    }
    for (x, y, ceiling, step) in ix::combine_within(Typical) {
        // Combined, then held to the ceiling; a sum that does not fit is over every ceiling.
        let want = combine_exact(x, y).map_or(Err(NumError::ToleranceExceeded), |z| {
            within_exact(z, ceiling, step)
        });
        assert_eq!(
            x.combine_within(y, ceiling, step),
            want,
            "{x:?} {y:?} {ceiling:?} {step:?}"
        );
    }
}

#[test]
fn wide_arithmetic_agrees_with_exact_values_on_every_input() {
    check_wide::<4>();
    check_wide::<5>();
    check_wide::<6>();
    check_wide::<8>();
    // The builders are generic: the narrowest widths `Wide` allows, where carries and overflow
    // reach the top limb soonest.
    check_wide::<2>();
    check_wide::<3>();
}

/// What `two_sum` and `fast_two_sum` must return (`geom_num::eft`): the IEEE sum `s`, then the exact
/// error when `s` is finite, and −s when it is not.
fn check_sum(what: &str, a: f64, b: f64, (s, e): (f64, f64)) {
    assert_eq!(
        s.to_bits(),
        (a + b).to_bits(),
        "{what}({a:e}, {b:e}): s is not a + b"
    );
    if !s.is_finite() {
        assert_eq!(
            e.to_bits(),
            (-s).to_bits(),
            "{what}({a:e}, {b:e}): an overflowed sum must give (s, −s)"
        );
        return;
    }
    let exact = Rat::of(a).plus(&Rat::of(b));
    let got = Rat::of(s).plus(&Rat::of(e));
    assert_eq!(got.compare(&exact), Ordering::Equal, "{what}({a:e}, {b:e})");
    assert!(s + e == s, "{what}({a:e}, {b:e}): s and e overlap");
}

#[test]
fn error_free_transformations_are_exact_on_every_input() {
    use inputs::eft as e;
    for class in [Typical, Extreme] {
        for (a, b) in e::two_sum(class) {
            check_sum("two_sum", a, b, eft::two_sum(a, b));
        }
        for (a, b) in e::fast_two_sum(class) {
            check_sum("fast_two_sum", a, b, eft::fast_two_sum(a, b));
        }
        for a in e::split(class) {
            let (hi, lo) = eft::split(a);
            // The high half is `a` rounded to 26 significant bits. Near ±Ω those are multiples of
            // 2^998, so it is ±2^1024, which no double holds, exactly when |a| passes the midpoint
            // 2^1024 − 2^997 (either way at the midpoint itself, a tie).
            let midpoint = Rat::int(Big::pow2(1024).minus(&Big::pow2(997)));
            let size = Rat::of(a.abs()).compare(&midpoint);
            if !hi.is_finite() {
                assert!(
                    hi.is_infinite() && (hi > 0.0) == (a > 0.0),
                    "split({a:e}): an unrepresentable high half must be an infinity with a's sign"
                );
                assert_eq!(lo.to_bits(), (-hi).to_bits(), "split({a:e})");
                assert_ne!(
                    size,
                    Ordering::Less,
                    "split({a:e}) reported a representable half"
                );
                continue;
            }
            assert_ne!(
                size,
                Ordering::Greater,
                "split({a:e}): a high half of 2^1024"
            );
            let sum = Rat::of(hi).plus(&Rat::of(lo));
            assert_eq!(sum.compare(&Rat::of(a)), Ordering::Equal, "split({a:e})");
            assert!(
                significant_bits(hi) <= 26 && significant_bits(lo) <= 26,
                "split({a:e})"
            );
            assert!(hi.abs() >= lo.abs(), "split({a:e})");
        }
        for (a, b) in e::two_prod(class) {
            let (p, err) = eft::two_prod(a, b);
            assert_eq!(p.to_bits(), (a * b).to_bits(), "two_prod({a:e}, {b:e})");
            if !p.is_finite() {
                assert_eq!(
                    err.to_bits(),
                    (-p).to_bits(),
                    "two_prod({a:e}, {b:e}): an overflowed product must give (p, −p)"
                );
                continue;
            }
            if a == 0.0 || b == 0.0 {
                assert_eq!(
                    err.to_bits(),
                    0,
                    "two_prod({a:e}, {b:e}): a zero factor gives +0"
                );
            }
            // The exact error a · b − p in units of 2^−2148 (each factor is a whole multiple of
            // 2^−1074). A double holds it exactly when it is a whole number of 2^−1074 with at most
            // 53 significant bits; then the pair must be exact, otherwise the error must be an
            // infinity with the exact error's sign.
            let error = scaled(a).times(&scaled(b)).minus(&scaled(p).shifted(1074));
            let (units, rest) = error.magnitude().div_rem(&Big::pow2(1074));
            if rest.is_zero() && significant_bits_of(&units) <= 53 {
                let exact = Rat::of(a).times(&Rat::of(b));
                let got = Rat::of(p).plus(&Rat::of(err));
                assert_eq!(
                    got.compare(&exact),
                    Ordering::Equal,
                    "two_prod({a:e}, {b:e})"
                );
                assert!(p + err == p, "two_prod({a:e}, {b:e}): p and e overlap");
            } else {
                assert!(
                    err.is_infinite() && (err > 0.0) == (error.sign() == Sign::Positive),
                    "two_prod({a:e}, {b:e}) = ({p:e}, {err:e}): no double holds the error, so it \
                     must be reported as an infinity with its sign"
                );
            }
        }
    }
}

/// What an interval operation must return from the exact bounds of its result: `NotFinite` exactly
/// when a bound passes Ω − 2^970 (it then rounds to ±Ω or beyond, and widening leaves the doubles),
/// otherwise an interval that encloses them, each bound less than two steps outside
/// (`geom_num::interval`'s guarantees), with no −0 bound.
fn check_enclosure(what: &str, lo: &Rat, hi: &Rat, got: Result<Interval, NumError>) {
    let t = Rat::of(f64::MAX).minus(&Rat::int(Big::pow2(970)));
    let overflows =
        hi.compare(&t) == Ordering::Greater || lo.compare(&t.negated()) == Ordering::Less;
    if overflows {
        assert_eq!(got, Err(NumError::NotFinite), "{what}");
        return;
    }
    let r = got.unwrap_or_else(|e| panic!("{what}: {e:?}"));
    assert!(
        r.lo().is_finite() && r.hi().is_finite() && r.lo() <= r.hi(),
        "{what}: {r:?}"
    );
    let negative_zero = (-0.0f64).to_bits();
    assert!(
        r.lo().to_bits() != negative_zero && r.hi().to_bits() != negative_zero,
        "{what}"
    );
    assert_ne!(
        Rat::of(r.lo()).compare(lo),
        Ordering::Greater,
        "{what}: lo misses: {r:?}"
    );
    assert_ne!(
        Rat::of(r.hi()).compare(hi),
        Ordering::Less,
        "{what}: hi misses: {r:?}"
    );
    let lo_tight = Rat::of(r.lo().next_up().next_up()).compare(lo) == Ordering::Greater;
    let hi_tight = Rat::of(r.hi().next_down().next_down()).compare(hi) == Ordering::Less;
    assert!(
        lo_tight && hi_tight,
        "{what}: a bound is two or more steps out: {r:?}"
    );
}

/// What a square root of [lo, hi], 0 ≤ lo ≤ hi, must return (`Interval::sqrt`): bounds whose
/// squares enclose [lo, hi], each less than two steps outside the exact root, and exactly 0 for a
/// zero bound.
fn check_root(what: &str, lo: f64, hi: f64, r: Interval) {
    let square = |v: f64| Rat::of(v).times(&Rat::of(v));
    let (exact_lo, exact_hi) = (Rat::of(lo), Rat::of(hi));
    assert_ne!(
        square(r.lo()).compare(&exact_lo),
        Ordering::Greater,
        "{what} -> {r:?}"
    );
    assert_ne!(
        square(r.hi()).compare(&exact_hi),
        Ordering::Less,
        "{what} -> {r:?}"
    );
    if lo > 0.0 {
        let up = r.lo().next_up().next_up();
        assert_eq!(
            square(up).compare(&exact_lo),
            Ordering::Greater,
            "{what} -> {r:?}"
        );
    } else {
        assert_eq!(r.lo().to_bits(), 0, "{what} -> {r:?}");
    }
    if hi > 0.0 {
        let down = r.hi().next_down().next_down();
        assert_eq!(
            square(down).compare(&exact_hi),
            Ordering::Less,
            "{what} -> {r:?}"
        );
    } else {
        assert_eq!(r.hi().to_bits(), 0, "{what} -> {r:?}");
    }
}

/// What `Interval::norm` must return for a vector `v`:
///
/// - its documented method, step by step through the public operations: each component squared,
///   the squares added in order, and the root of the sum's non-negative part, which is
///   `Interval::sqrt` of `[max(lo, 0), hi]` (`NegativeSqrt` only if the whole sum is negative);
/// - an enclosure of the exact length over the box, checked with exact rationals: the squared lower
///   bound at most the smallest exact sum of squares, the squared upper bound at least the largest;
///   the lower bound never negative;
/// - `NotFinite` only when the largest exact sum of squares is above 2^1023, the binade where
///   the rounded and widened partial sums can leave the doubles.
fn check_norm(v: &[Interval; 3]) {
    let what = format!("norm {v:?}");
    let got = Interval::norm(v);
    let method = (|| {
        let mut sum = v[0].square()?;
        for c in &v[1..] {
            sum = sum.add(c.square()?)?;
        }
        if sum.hi() < 0.0 {
            return Err(NumError::NegativeSqrt);
        }
        let lo = if sum.lo() < 0.0 { 0.0 } else { sum.lo() };
        Interval::new(lo, sum.hi())?.sqrt()
    })();
    let bits = |r: Result<Interval, NumError>| r.map(|x| (x.lo().to_bits(), x.hi().to_bits()));
    assert_eq!(bits(got), bits(method), "{what}: not the documented method");
    // The exact smallest and largest sums of squares over the box.
    let zero = Rat::int(Big::zero());
    let (mut least, mut most) = (zero.clone(), zero.clone());
    for c in v {
        let (lo, hi) = (Rat::of(c.lo()), Rat::of(c.hi()));
        let (ll, hh) = (lo.times(&lo), hi.times(&hi));
        let low = if c.lo() <= 0.0 && c.hi() >= 0.0 {
            zero.clone()
        } else {
            Rat::min_of(&[ll.clone(), hh.clone()])
        };
        least = least.plus(&low);
        most = most.plus(&Rat::max_of(&[ll, hh]));
    }
    match got {
        Ok(r) => {
            assert!(
                r.lo() >= 0.0 && r.lo().to_bits() != (-0.0f64).to_bits(),
                "{what}: {r:?}"
            );
            let square = |x: f64| Rat::of(x).times(&Rat::of(x));
            assert_ne!(
                square(r.lo()).compare(&least),
                Ordering::Greater,
                "{what}: lo misses: {r:?}"
            );
            assert_ne!(
                square(r.hi()).compare(&most),
                Ordering::Less,
                "{what}: hi misses: {r:?}"
            );
        }
        Err(e) => {
            assert_eq!(e, NumError::NotFinite, "{what}");
            assert_eq!(
                most.compare(&Rat::int(Big::pow2(1023))),
                Ordering::Greater,
                "{what}: refused a length whose squares sum below 2^1023"
            );
        }
    }
}

#[test]
fn intervals_enclose_the_exact_results_on_every_input() {
    use inputs::interval as i;
    let ends = |x: Interval| (Rat::of(x.lo()), Rat::of(x.hi()));
    for class in [Typical, Extreme] {
        for (x, y) in i::add(class) {
            let ((a, b), (c, d)) = (ends(x), ends(y));
            check_enclosure("add", &a.plus(&c), &b.plus(&d), x.add(y));
        }
        for (x, y) in i::sub(class) {
            let ((a, b), (c, d)) = (ends(x), ends(y));
            check_enclosure("sub", &a.minus(&d), &b.minus(&c), x.sub(y));
        }
        for (x, y) in i::mul(class) {
            let ((a, b), (c, d)) = (ends(x), ends(y));
            let corners = [a.times(&c), a.times(&d), b.times(&c), b.times(&d)];
            check_enclosure(
                "mul",
                &Rat::min_of(&corners),
                &Rat::max_of(&corners),
                x.mul(y),
            );
        }
        for (x, y) in i::div(class) {
            if y.lo() <= 0.0 && y.hi() >= 0.0 {
                assert_eq!(x.div(y), Err(NumError::DivisorContainsZero));
                continue;
            }
            let ((a, b), (c, d)) = (ends(x), ends(y));
            let corners = [a.over(&c), a.over(&d), b.over(&c), b.over(&d)];
            check_enclosure(
                "div",
                &Rat::min_of(&corners),
                &Rat::max_of(&corners),
                x.div(y),
            );
        }
        for x in i::square(class) {
            // Every x² for x in [lo, hi]: [lo², hi²] above zero, [hi², lo²] below it, and
            // [0, max(lo², hi²)] around it.
            let (a, b) = ends(x);
            let (aa, bb) = (a.times(&a), b.times(&b));
            let (lo, hi) = if x.lo() >= 0.0 {
                (aa, bb)
            } else if x.hi() <= 0.0 {
                (bb, aa)
            } else {
                (Rat::int(Big::zero()), Rat::max_of(&[aa, bb]))
            };
            let got = x.square();
            if let Ok(r) = got {
                // A square is never negative, and a square around zero starts at exactly 0.
                assert!(r.lo() >= 0.0, "square {x:?} -> {r:?}");
                if x.lo() <= 0.0 && x.hi() >= 0.0 {
                    assert_eq!(r.lo().to_bits(), 0, "square {x:?} -> {r:?}");
                }
            }
            check_enclosure(&format!("square {x:?}"), &lo, &hi, got);
        }
        for x in i::sqrt(class) {
            match x.sqrt() {
                Ok(r) => {
                    assert!(x.lo() >= 0.0, "{x:?}");
                    check_root(&format!("sqrt {x:?}"), x.lo(), x.hi(), r);
                }
                Err(e) => assert!(x.lo() < 0.0 && e == NumError::NegativeSqrt, "{x:?}"),
            }
        }
        for (x, y, z) in i::mul_add(class) {
            // x · y + z over the box: the extreme corners of x · y, plus the matching bound of z.
            let ((a, b), (c, d), (e, f)) = (ends(x), ends(y), ends(z));
            let corners = [a.times(&c), a.times(&d), b.times(&c), b.times(&d)];
            check_enclosure(
                &format!("mul_add {x:?} {y:?} {z:?}"),
                &Rat::min_of(&corners).plus(&e),
                &Rat::max_of(&corners).plus(&f),
                x.mul_add(y, z),
            );
        }
        for v in i::norm(class) {
            check_norm(&v);
        }
    }
    for x in i::neg(Typical) {
        let r = x.neg();
        assert!(r.lo() == -x.hi() && r.hi() == -x.lo(), "neg {x:?} -> {r:?}");
    }
    for x in i::certain_sign(Typical) {
        let want = if x.lo() > 0.0 {
            Some(Sign::Positive)
        } else if x.hi() < 0.0 {
            Some(Sign::Negative)
        } else if x.lo() == 0.0 && x.hi() == 0.0 {
            Some(Sign::Zero)
        } else {
            None
        };
        assert_eq!(x.certain_sign(), want, "{x:?}");
    }
}

#[test]
fn grid_steps_agree_with_exact_values_on_every_input() {
    use inputs::grid as g;
    for class in [Typical, Overflow, WrongDirection] {
        for (c, from, to) in g::refine(class) {
            assert_eq!(
                grid::refine(c, from, to),
                refine_exact(c, from, to),
                "{class}"
            );
        }
    }
    for class in [Typical, Extreme, WrongDirection] {
        for (c, from, to) in g::coarsen(class) {
            assert_eq!(
                parts(grid::coarsen(c, from, to)),
                coarsen_exact(c, from, to),
                "{class}"
            );
        }
    }
    for class in [Typical, Overflow, WrongDirection] {
        for (s, from, to) in g::steps_refine(class) {
            assert_eq!(
                s.refine(from, to),
                steps_refine_exact(s, from, to),
                "{class} {s:?} {from:?} {to:?}"
            );
        }
    }
    for (step, unit) in g::steps_per(Typical) {
        assert_eq!(
            step.steps_per(unit),
            steps_per_exact(step, unit),
            "{step:?} {unit}"
        );
    }
}

/// 2^e as a double, for e from −1074 to 1023, built from its bits.
fn two_to(e: i64) -> f64 {
    if e >= -1022 {
        f64::from_bits(((e + 1023) as u64) << 52)
    } else {
        f64::from_bits(1 << (e + 1074))
    }
}

/// The double nearest n · 2^−k, ties to even (IEEE 754 round to nearest), or `None` when that is
/// past the largest double; a zero is +0. For k ≥ 1074, so the last place kept is a whole bit of n:
/// the quantum is 2^(e − 52) for a value in [2^e, 2^(e + 1)), and never below 2^−1074.
fn round_scaled(n: &Big, k: u32) -> Option<f64> {
    assert!(k >= 1074);
    if n.is_zero() {
        return Some(0.0);
    }
    let len = i64::from(n.bit_len());
    let e = len - 1 - i64::from(k);
    if e > 1023 {
        // At least 2^1024: past every double.
        return None;
    }
    let quantum = (e - 52).max(-1074);
    // The bits of |n| from `shift` up are the significand; bit shift − 1 is the half, and the
    // bits below it are sticky.
    let shift = u32::try_from(quantum + i64::from(k)).unwrap();
    let mut m: u64 = 0;
    for i in (shift..n.bit_len()).rev() {
        m = (m << 1) | u64::from(n.bit(i));
    }
    let half = shift >= 1 && n.bit(shift - 1);
    let sticky = (0..shift.saturating_sub(1)).any(|i| n.bit(i));
    if half && (sticky || m % 2 == 1) {
        m += 1;
    }
    // m ≤ 2^53, exact as a double, and 2^quantum is a double: the product is exact or infinite.
    let magnitude = m as f64 * two_to(quantum);
    if !magnitude.is_finite() {
        return None;
    }
    Some(if n.negative { -magnitude } else { magnitude })
}

#[test]
fn fma_is_correctly_rounded_and_powi_is_pow_on_every_input() {
    // geom_num::math: the fused multiply-add is a · b + c rounded once, correctly (IEEE 754), and
    // NotFinite exactly when that overflows; powi(x, n) is pow(x, n) with n converted exactly.
    use inputs::math as m;
    for class in [Typical, Extreme] {
        for (a, b, c) in m::fma(class) {
            // a · b + c in units of 2^−2148: each factor is a whole number of 2^−1074.
            let exact = scaled(a).times(&scaled(b)).plus(&scaled(c).shifted(1074));
            let want = round_scaled(&exact, 2148);
            let got = math::fma(a, b, c);
            match want {
                Some(r) => {
                    // A zero's sign follows IEEE 754's rules for the operands' zeros, which the
                    // exact value does not carry: compare values, then the bits of a nonzero.
                    let value = got.unwrap_or_else(|e| panic!("fma({a:e}, {b:e}, {c:e}): {e:?}"));
                    assert_eq!(value.get(), r, "fma({a:e}, {b:e}, {c:e})");
                    if r != 0.0 {
                        assert_eq!(
                            value.get().to_bits(),
                            r.to_bits(),
                            "fma({a:e}, {b:e}, {c:e})"
                        );
                    }
                }
                None => assert_eq!(got, Err(NumError::NotFinite), "fma({a:e}, {b:e}, {c:e})"),
            }
        }
        for (x, n) in m::unproven::powi(class) {
            assert_eq!(
                math::unproven::powi(x, n),
                math::unproven::pow(x, f64::from(n)),
                "powi({x:e}, {n})"
            );
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Counter-based random numbers: an independent Philox4x32-10, and the draws as `geom_num::rng`
// defines them.
// ------------------------------------------------------------------------------------------------

/// 2^32.
const TWO_32: u64 = 1 << 32;

/// The words in one stream: 2^64 blocks of four.
const STREAM_WORDS: u128 = 1 << 66;

/// Philox4x32-10 (Salmon, Moraes, Dror and Shaw, "Parallel random numbers: as easy as 1, 2, 3",
/// SC 2011, <https://doi.org/10.1145/2063384.2063405>), in 64-bit arithmetic: each product of two
/// 32-bit words is exact in 64 bits, its high word is the quotient by 2^32 and its low word the
/// remainder; before every round but the first the key steps by the Weyl constants, modulo 2^32.
fn philox_oracle(counter: [u32; 4], key: [u32; 2]) -> [u32; 4] {
    const MUL: [u64; 2] = [0xD251_1F53, 0xCD9E_8D57];
    const WEYL: [u64; 2] = [0x9E37_79B9, 0xBB67_AE85];
    let mut x = counter.map(u64::from);
    let mut k = key.map(u64::from);
    for round in 0..10 {
        if round > 0 {
            k = [(k[0] + WEYL[0]) % TWO_32, (k[1] + WEYL[1]) % TWO_32];
        }
        let (p0, p1) = (MUL[0] * x[0], MUL[1] * x[2]);
        x = [
            (p1 / TWO_32) ^ x[1] ^ k[0],
            p1 % TWO_32,
            (p0 / TWO_32) ^ x[3] ^ k[1],
            p0 % TWO_32,
        ];
    }
    x.map(|w| u32::try_from(w).unwrap())
}

/// A 64-bit value's low and high 32-bit words.
fn halves(v: u64) -> [u32; 2] {
    [
        u32::try_from(v % TWO_32).unwrap(),
        u32::try_from(v / TWO_32).unwrap(),
    ]
}

/// A stream as a seed, a stream id and a position, every word recomputed by [`philox_oracle`] in
/// the layout `geom_num::rng` documents: word p is word p mod 4 of block p / 4, whose counter is
/// the block index's low and high words then the stream id's, keyed by the seed's low and high
/// words.
struct StreamModel {
    seed: u64,
    stream: u64,
    pos: u128,
}

impl StreamModel {
    fn of(s: &Stream) -> StreamModel {
        StreamModel {
            seed: s.philox().seed().0,
            stream: s.philox().stream_id().0,
            pos: s.position(),
        }
    }

    fn exhausted(&self) -> RngError {
        RngError::Exhausted {
            stream: StreamId(self.stream),
        }
    }

    /// The next word, or `Exhausted` past the stream's 2^66 words.
    fn word(&mut self) -> Result<u32, RngError> {
        if self.pos >= STREAM_WORDS {
            return Err(self.exhausted());
        }
        let block = u64::try_from(self.pos / 4).unwrap();
        let [b0, b1] = halves(block);
        let [s0, s1] = halves(self.stream);
        let words = philox_oracle([b0, b1, s0, s1], halves(self.seed));
        let word = words[usize::try_from(self.pos % 4).unwrap()];
        self.pos += 1;
        Ok(word)
    }

    /// The next two words, low then high, or `Exhausted` with fewer than two left.
    fn word64(&mut self) -> Result<u64, RngError> {
        if self.pos + 2 > STREAM_WORDS {
            return Err(self.exhausted());
        }
        let low = u64::from(self.word()?);
        Ok(low + u64::from(self.word()?) * TWO_32)
    }

    /// The top 53 bits of the next 64-bit word, times 2^−53.
    fn unit(&mut self) -> Result<Real, RngError> {
        let k = self.word64()? >> 11;
        // k < 2^53 converts exactly, and dividing by 2^53 only moves the exponent.
        #[allow(clippy::cast_precision_loss, reason = "k is below 2^53, so exact")]
        let x = k as f64 / 9_007_199_254_740_992.0;
        Ok(Real::new(x).unwrap())
    }

    /// Lemire's method (2019, <https://doi.org/10.1145/3230636>): a word w gives ⌊w · n / 2^64⌋
    /// unless the low half of w · n is below 2^64 mod n, when the next word is drawn instead.
    fn below(&mut self, n: u64) -> Result<u64, RngError> {
        if n == 0 {
            return Err(RngError::ZeroBound);
        }
        let threshold = (1u128 << 64) % u128::from(n);
        loop {
            let m = u128::from(self.word64()?) * u128::from(n);
            if m % (1u128 << 64) >= threshold {
                return Ok(u64::try_from(m >> 64).unwrap());
            }
        }
    }

    /// low + below(high − low + 1), or one 64-bit word for the whole of `u64`.
    fn range_inclusive(&mut self, low: u64, high: u64) -> Result<u64, RngError> {
        if low > high {
            return Err(RngError::LowAboveHigh { low, high });
        }
        match (high - low).checked_add(1) {
            Some(values) => Ok(low + self.below(values)?),
            None => self.word64(),
        }
    }
}

#[test]
fn the_independent_philox_gives_random123s_known_answers() {
    // The three philox4x32 10 lines of Random123's tests/kat_vectors
    // (<https://github.com/DEShawResearch/random123/blob/9545ff6413f258be2f04c1d319d99aaef7521150/tests/kat_vectors>,
    // BSD-3-Clause, "Copyright 2010-2012, D. E. Shaw Research"; test vectors are facts).
    let kats: [([u32; 4], [u32; 2], [u32; 4]); 3] = [
        (
            [0; 4],
            [0; 2],
            [0x6627_e8d5, 0xe169_c58d, 0xbc57_ac4c, 0x9b00_dbd8],
        ),
        (
            [u32::MAX; 4],
            [u32::MAX; 2],
            [0x408f_276d, 0x41c8_3b0e, 0xa20b_c7c6, 0x6d54_51fd],
        ),
        (
            [0x243f_6a88, 0x85a3_08d3, 0x1319_8a2e, 0x0370_7344],
            [0xa409_3822, 0x299f_31d0],
            [0xd16c_fe09, 0x94fd_cceb, 0x5001_e420, 0x2412_6ea1],
        ),
    ];
    for (counter, key, expected) in kats {
        assert_eq!(philox_oracle(counter, key), expected, "{counter:x?}");
    }
}

/// What `StreamRange::chunk` must give, as (seed, first id, last id), in exact 128-bit arithmetic.
fn chunk_oracle(range: StreamRange, index: u64, size: u64) -> Result<(Seed, u128, u128), RngError> {
    let (first, last) = (u128::from(range.first().0), u128::from(range.last().0));
    let start = first + u128::from(index) * u128::from(size);
    match (start + u128::from(size)).checked_sub(1) {
        _ if size == 0 => Err(RngError::EmptyChunk),
        Some(end) if end <= last => Ok((range.seed(), start, end)),
        _ => Err(RngError::TooFewStreams {
            first: range.first(),
            last: range.last(),
        }),
    }
}

#[test]
fn every_random_number_result_matches_the_independent_philox() {
    use inputs::rng as i;
    for class in Class::ALL {
        for (counter, key) in i::philox4x32_10(class) {
            assert_eq!(
                rng::philox4x32_10(counter, key),
                philox_oracle(counter, key)
            );
        }
        for (philox, block) in i::stream_from(class) {
            let mut stream = philox.stream_from(block);
            assert_eq!(stream.philox(), philox);
            assert_eq!(stream.position(), u128::from(block) * 4);
            // The block the stream holds is the right one: its first word is the oracle's.
            let mut model = StreamModel::of(&stream);
            assert_eq!(stream.next_u32(), model.word(), "{philox:?} {block}");
        }
        for mut s in i::next_u32(class) {
            let mut model = StreamModel::of(&s);
            assert_eq!(s.next_u32(), model.word(), "{s:?}");
        }
        for mut s in i::next_u64(class) {
            let mut model = StreamModel::of(&s);
            assert_eq!(s.next_u64(), model.word64(), "{s:?}");
        }
        for mut s in i::next_f64(class) {
            let mut model = StreamModel::of(&s);
            assert_eq!(s.next_f64(), model.unit(), "{s:?}");
        }
        for (mut s, n) in i::below(class) {
            let mut model = StreamModel::of(&s);
            assert_eq!(s.below(n), model.below(n), "{s:?} {n}");
        }
        for (mut s, low, high) in i::range_inclusive(class) {
            let mut model = StreamModel::of(&s);
            let expected = model.range_inclusive(low, high);
            assert_eq!(s.range_inclusive(low, high), expected, "{s:?} {low} {high}");
        }
        for (range, index) in i::range_philox(class) {
            let id = u128::from(range.first().0) + u128::from(index);
            let expected = if id <= u128::from(range.last().0) {
                Ok(Philox::new(
                    range.seed(),
                    StreamId(u64::try_from(id).unwrap()),
                ))
            } else {
                Err(RngError::TooFewStreams {
                    first: range.first(),
                    last: range.last(),
                })
            };
            assert_eq!(range.philox(index), expected, "{range:?} {index}");
        }
        for (range, index, size) in i::chunk(class) {
            let got = range
                .chunk(index, size)
                .map(|c| (c.seed(), u128::from(c.first().0), u128::from(c.last().0)));
            assert_eq!(
                got,
                chunk_oracle(range, index, size),
                "{range:?} {index} {size}"
            );
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Double-double arithmetic and the exact sum, against exact rationals.
// ------------------------------------------------------------------------------------------------

/// The exact value of a pair.
fn rat_dd(x: DD) -> Rat {
    Rat::of(x.hi()).plus(&Rat::of(x.lo()))
}

/// |q|.
fn rat_abs(q: &Rat) -> Rat {
    if q.num.negative {
        q.negated()
    } else {
        q.clone()
    }
}

/// (1 − 2^−52)·2^1024 = 2^1024 − 2^972: an operation refuses a result as too large only above it
/// (`geom_num::dd`, *Never NaN, never infinite*).
fn refusal_floor() -> Rat {
    Rat::int(Big::pow2(1024).minus(&Big::pow2(972)))
}

/// 2^1024 − 2^970: no pair holds a value this large, since its leading part would round to
/// infinity.
fn holdable_ceiling() -> Rat {
    Rat::int(Big::pow2(1024).minus(&Big::pow2(970)))
}

/// A pair in the invariant `geom_num::dd` documents: both parts finite, the leading part the value
/// rounded to nearest (so adding the parts in binary64 gives it back), and no −0.
fn assert_pair(x: DD, what: &str) {
    assert!(x.hi().is_finite() && x.lo().is_finite(), "{what}: {x:?}");
    assert_eq!(x.hi() + x.lo(), x.hi(), "{what}: {x:?} is not normalised");
    let minus_zero = (-0.0f64).to_bits();
    assert!(
        x.hi().to_bits() != minus_zero && x.lo().to_bits() != minus_zero,
        "{what}: {x:?} holds a −0"
    );
}

/// A pair's bits, for comparing results bit for bit.
fn dd_bits(x: Result<DD, NumError>) -> Result<(u64, u64), NumError> {
    x.map(|v| (v.hi().to_bits(), v.lo().to_bits()))
}

/// Checks a rounded operation's answer against its exact result: `Err` when an operand must be
/// refused (with that error); otherwise within B·|r| + A, a pair in the invariant, or refused as
/// too large only above [`refusal_floor`].
fn check_bound(
    what: &str,
    got: Result<DD, NumError>,
    exact: Result<Rat, NumError>,
    b: f64,
    a: f64,
) {
    match (got, exact) {
        (got, Err(e)) => assert_eq!(got.map(|_| ()), Err(e), "{what}"),
        (Ok(x), Ok(r)) => {
            assert_pair(x, what);
            let err = rat_abs(&rat_dd(x).minus(&r));
            let allowed = Rat::of(b).times(&rat_abs(&r)).plus(&Rat::of(a));
            assert_ne!(
                err.compare(&allowed),
                Ordering::Greater,
                "{what}: {x:?} is outside its bound"
            );
        }
        (Err(e), Ok(r)) => {
            assert_eq!(e, NumError::NotFinite, "{what}");
            assert_eq!(
                rat_abs(&r).compare(&refusal_floor()),
                Ordering::Greater,
                "{what}: refused a result below the documented limit"
            );
        }
    }
}

/// Whether the exact error `a·b − p` of a finite product is a double: a whole multiple of 2^−1074
/// (it is at most half a unit of `p`, so never too large).
fn product_error_is_a_double(a: f64, b: f64, p: f64) -> bool {
    let d = scaled(a).times(&scaled(b)).minus(&scaled(p).shifted(1074));
    (0..1074).all(|i| !d.bit(i))
}

/// The exact real `v` rounded to the nearest whole number, ties to even, and whether it was whole.
fn rat_round_half_even(v: &Rat) -> (Big, bool) {
    let (q, r) = v.num.div_rem(&v.den);
    // v = floor + rem / den with 0 ≤ rem < den.
    let (floor, rem) = match (v.num.negative, r.is_zero()) {
        (false, _) => (q, r),
        (true, true) => (q.negated(), r),
        (true, false) => (q.negated().minus(&Big::one()), v.den.minus(&r)),
    };
    let up = match rem.shifted(1).cmp(&v.den) {
        Ordering::Less => false,
        Ordering::Greater => true,
        Ordering::Equal => floor.bit(0),
    };
    let whole = rem.is_zero();
    (if up { floor.plus(&Big::one()) } else { floor }, whole)
}

#[test]
fn the_double_double_bounds_are_at_least_the_published_ones() {
    use geom_num::dd::{
        ADD_BOUND, ADD_F64_BOUND, DIV_BOUND, DIV_F64_BOUND, MUL_BOUND, MUL_F64_BOUND, SQRT_BOUND,
    };
    // u = 2^−53. JMP Theorems 3.1, 2.2, 4.3, 6.2 and 7.2, Muller and Rideau's 2.8, and Lefèvre et
    // al.'s 3.6, as `geom_num::dd`'s table cites them.
    let u = Rat::of(f64::from_bits(0x3ca0_0000_0000_0000));
    let u2 = u.times(&u);
    let int = |v: i64| Rat::int(Big::from_i64(v));
    let one = int(1);
    let add = int(3).times(&u2).over(&one.minus(&int(4).times(&u)));
    let mul = int(4)
        .times(&u2)
        .plus(&u2.times(&u).over(&int(2)))
        .over(&one.plus(&u).times(&one.plus(&u)));
    let published = [
        ("add", ADD_BOUND, add),
        ("add_f64", ADD_F64_BOUND, int(2).times(&u2)),
        ("mul_f64", MUL_F64_BOUND, int(2).times(&u2)),
        ("mul", MUL_BOUND, mul),
        ("div_f64", DIV_F64_BOUND, int(3).times(&u2)),
        ("div", DIV_BOUND, int(98).times(&u2).over(&int(10))),
        ("sqrt", SQRT_BOUND, int(25).times(&u2).over(&int(8))),
    ];
    for (what, bound, theorem) in published {
        assert_ne!(Rat::of(bound).compare(&theorem), Ordering::Less, "{what}");
    }
}

#[test]
fn every_double_double_result_meets_its_proven_bound() {
    use geom_num::dd::{
        ADD_BOUND, ADD_F64_BOUND, DIV_BOUND, DIV_F64_BOUND, MUL_BOUND, MUL_F64_BOUND, SQRT_BOUND,
        UNDERFLOW_BOUND,
    };
    use inputs::dd as i;
    let finite = |y: f64| {
        if y.is_finite() {
            Ok(Rat::of(y))
        } else {
            Err(NumError::NotFinite)
        }
    };
    for class in Class::ALL {
        for (x, y) in i::add(class) {
            let exact = Ok(rat_dd(x).plus(&rat_dd(y)));
            check_bound("add", x.add(y), exact, ADD_BOUND, 0.0);
            assert_eq!(dd_bits(x.add(y)), dd_bits(y.add(x)), "add commutes");
        }
        for (x, y) in i::add_f64(class) {
            let exact = finite(y).map(|y| rat_dd(x).plus(&y));
            check_bound("add_f64", x.add_f64(y), exact, ADD_F64_BOUND, 0.0);
        }
        for (x, y) in i::mul_f64(class) {
            let exact = finite(y).map(|y| rat_dd(x).times(&y));
            check_bound(
                "mul_f64",
                x.mul_f64(y),
                exact,
                MUL_F64_BOUND,
                UNDERFLOW_BOUND,
            );
        }
        for (x, y) in i::mul(class) {
            let exact = Ok(rat_dd(x).times(&rat_dd(y)));
            check_bound("mul", x.mul(y), exact, MUL_BOUND, UNDERFLOW_BOUND);
            assert_eq!(dd_bits(x.mul(y)), dd_bits(y.mul(x)), "mul commutes");
        }
        for (x, y) in i::div_f64(class) {
            let exact = match finite(y) {
                Ok(q) if y != 0.0 => Ok(rat_dd(x).over(&q)),
                _ => Err(NumError::NotFinite),
            };
            check_bound(
                "div_f64",
                x.div_f64(y),
                exact,
                DIV_F64_BOUND,
                UNDERFLOW_BOUND,
            );
        }
        for (x, y) in i::div(class) {
            let exact = if y.hi() == 0.0 {
                Err(NumError::NotFinite)
            } else {
                Ok(rat_dd(x).over(&rat_dd(y)))
            };
            check_bound("div", x.div(y), exact, DIV_BOUND, UNDERFLOW_BOUND);
        }
        for x in i::sqrt(class) {
            if x.hi() < 0.0 {
                assert_eq!(x.sqrt(), Err(NumError::OutsideDomain), "{x:?}");
                continue;
            }
            // |s − √v| ≤ B·√v, for s ≥ 0, is (1 − B)²·v ≤ s² ≤ (1 + B)²·v.
            let s = x.sqrt().unwrap();
            assert_pair(s, "sqrt");
            assert!(s.hi() >= 0.0, "{x:?}");
            let (v, s) = (rat_dd(x), rat_dd(s));
            let b = Rat::of(SQRT_BOUND);
            let one = Rat::int(Big::one());
            let (low, high) = (one.minus(&b), one.plus(&b));
            let s2 = s.times(&s);
            assert_ne!(
                s2.compare(&low.times(&low).times(&v)),
                Ordering::Less,
                "{x:?}"
            );
            assert_ne!(
                s2.compare(&high.times(&high).times(&v)),
                Ordering::Greater,
                "{x:?}"
            );
        }
    }
}

#[test]
fn every_exact_double_double_result_is_exact() {
    use inputs::dd as i;
    let ceiling = holdable_ceiling();
    for class in Class::ALL {
        for (a, b) in i::from_sum(class) {
            let got = DD::from_sum(a, b);
            if !a.is_finite() || !b.is_finite() {
                assert_eq!(got, Err(NumError::NotFinite));
                continue;
            }
            let r = Rat::of(a).plus(&Rat::of(b));
            match got {
                Ok(x) => {
                    assert_pair(x, "from_sum");
                    assert_eq!(rat_dd(x).compare(&r), Ordering::Equal, "{a:e} + {b:e}");
                }
                Err(e) => {
                    assert_eq!(e, NumError::NotFinite);
                    assert_ne!(
                        rat_abs(&r).compare(&ceiling),
                        Ordering::Less,
                        "{a:e} + {b:e}"
                    );
                }
            }
        }
        let products = i::two_prod_fma(class)
            .into_iter()
            .chain(i::exact_product(class));
        for (a, b) in products {
            let exact = DD::exact_product(a, b);
            if !a.is_finite() || !b.is_finite() {
                assert_eq!(exact, None);
                continue;
            }
            let (p, e) = dd::two_prod_fma(a, b);
            assert_eq!(p.to_bits(), (a * b).to_bits(), "{a:e} · {b:e}");
            let holds = p.is_finite() && product_error_is_a_double(a, b, p);
            assert_eq!(e.is_finite(), holds, "{a:e} · {b:e}: ({p:e}, {e:e})");
            let r = Rat::of(a).times(&Rat::of(b));
            if holds {
                let pair = Rat::of(p).plus(&Rat::of(e));
                assert_eq!(pair.compare(&r), Ordering::Equal, "{a:e} · {b:e}");
                let x = exact.unwrap();
                assert_pair(x, "exact_product");
                assert_eq!(rat_dd(x).compare(&r), Ordering::Equal, "{a:e} · {b:e}");
            } else {
                assert_eq!(exact, None, "{a:e} · {b:e}");
            }
        }
        for v in i::from_i64(class) {
            let x = DD::from_i64(v);
            assert_pair(x, "from_i64");
            let r = Rat::int(Big::from_i64(v));
            assert_eq!(rat_dd(x).compare(&r), Ordering::Equal, "{v}");
        }
        for v in i::from_i128(class) {
            match DD::from_i128(v) {
                Some(x) => {
                    assert!(v.unsigned_abs() < 1 << 106, "{v}");
                    assert_pair(x, "from_i128");
                    let r = Rat::int(Big::from_i128(v));
                    assert_eq!(rat_dd(x).compare(&r), Ordering::Equal, "{v}");
                }
                None => assert!(v.unsigned_abs() >= 1 << 106, "{v}"),
            }
        }
        for x in i::sign(class) {
            assert_eq!(x.sign(), rat_dd(x).num.sign(), "{x:?}");
        }
        for x in i::negate(class) {
            let n = x.negate();
            assert_pair(n, "negate");
            let r = rat_dd(x).negated();
            assert_eq!(rat_dd(n).compare(&r), Ordering::Equal, "{x:?}");
        }
        for x in i::abs(class) {
            let m = x.abs();
            assert_pair(m, "abs");
            let r = rat_abs(&rat_dd(x));
            assert_eq!(rat_dd(m).compare(&r), Ordering::Equal, "{x:?}");
        }
        for (x, y) in i::cmp(class) {
            assert_eq!(x.cmp(&y), rat_dd(x).compare(&rat_dd(y)), "{x:?} {y:?}");
        }
        for (x, offset) in i::round_shifted(class) {
            // 2^100, the largest leading part answered.
            let expected = if x.hi().abs() > f64::from_bits(0x4630_0000_0000_0000) {
                None
            } else {
                let v = rat_dd(x).minus(&Rat::int(Big::from_i128(offset)));
                let (n, whole) = rat_round_half_even(&v);
                n.to_i128().map(|n| (n, whole))
            };
            let got = x.round_shifted(offset).map(|r| (r.value(), r.is_exact()));
            assert_eq!(got, expected, "{x:?} − {offset}");
        }
    }
}

/// Every double from a whole multiple `s` of 2^−1074: the nearest, ties to the even significand;
/// ±∞ from 2^1024 − 2^970 up, where rounding to nearest overflows; +0 for zero.
fn nearest_double(s: &Big) -> f64 {
    let m = s.magnitude();
    let overflow = Big::pow2(1024).minus(&Big::pow2(970)).shifted(1074);
    let bits = if m >= overflow {
        0x7ff0_0000_0000_0000
    } else if m.bit_len() <= 53 {
        // Below 2^53 steps of 2^−1074 a double's bits are the count itself: subnormal below 2^52,
        // exponent field 1 from there.
        u64::try_from(m.to_i128().unwrap()).unwrap()
    } else {
        let shift = m.bit_len() - 53;
        let (q, r) = m.div_rem(&Big::pow2(shift));
        let mut q = u64::try_from(q.to_i128().unwrap()).unwrap();
        let mut shift = shift;
        let up = match r.shifted(1).cmp(&Big::pow2(shift)) {
            Ordering::Less => false,
            Ordering::Greater => true,
            Ordering::Equal => q % 2 == 1,
        };
        if up {
            q += 1;
            if q == 1 << 53 {
                q >>= 1;
                shift += 1;
            }
        }
        // The value is q·2^shift steps, q in [2^52, 2^53): exponent field shift + 1.
        (u64::from(shift + 1) << 52) | (q - (1 << 52))
    };
    let sign = if s.negative { 1u64 << 63 } else { 0 };
    f64::from_bits(if bits == 0 { 0 } else { bits | sign })
}

/// What `ExactSum::round_ieee` must give for `terms` (`geom_num::exact_sum`, *Special values as
/// Neal specifies*): the NaN with the largest payload, sign cleared; a NaN for both infinities;
/// the one infinity; or the exact sum of the finite terms rounded once.
fn exact_sum_oracle(terms: &[f64]) -> f64 {
    let fraction = (1u64 << 52) - 1;
    let payload = terms
        .iter()
        .filter(|x| x.is_nan())
        .map(|x| x.to_bits() & fraction)
        .max();
    if let Some(payload) = payload {
        return f64::from_bits(0x7ff0_0000_0000_0000 | payload);
    }
    match (
        terms.contains(&f64::INFINITY),
        terms.contains(&f64::NEG_INFINITY),
    ) {
        (true, true) => return f64::from_bits(0x7ff8_0000_0000_0000),
        (true, false) => return f64::INFINITY,
        (false, true) => return f64::NEG_INFINITY,
        (false, false) => {}
    }
    nearest_double(
        &terms
            .iter()
            .fold(Big::zero(), |acc, &x| acc.plus(&scaled(x))),
    )
}

#[test]
fn the_nearest_double_oracle_rounds_as_ieee_does() {
    // Sums binary64 rounds once: two terms whose exact sum it rounds to nearest, ties to even.
    let mut g = SplitMix64::new(seed_for("attack_g06", "nearest_double"));
    for _ in 0..2000 {
        // Any finite double: a sign, an exponent field below 2047, and a fraction.
        let pick = |g: &mut SplitMix64| {
            let sign = g.next_u64() & (1 << 63);
            f64::from_bits(sign | (g.below(2047) << 52) | (g.next_u64() >> 12))
        };
        let (a, b) = (pick(&mut g), pick(&mut g));
        let expected = a + b;
        let got = nearest_double(&scaled(a).plus(&scaled(b)));
        if expected == 0.0 {
            assert_eq!(got.to_bits(), 0, "{a:e} + {b:e}");
        } else {
            assert_eq!(got.to_bits(), expected.to_bits(), "{a:e} + {b:e}");
        }
    }
}

#[test]
fn every_exact_sum_result_is_the_sum_rounded_once() {
    use inputs::exact_sum as i;
    let same = |what: &str, got: f64, expected: f64| {
        assert_eq!(
            got.to_bits(),
            expected.to_bits(),
            "{what}: {got:e} against {expected:e}"
        );
    };
    for class in Class::ALL {
        for terms in i::add(class) {
            let mut sum = ExactSum::new();
            for &x in &terms {
                sum.add(x);
            }
            same("add", sum.round_ieee(), exact_sum_oracle(&terms));
        }
        for terms in i::add_slice(class) {
            let mut sum = ExactSum::new();
            sum.add_slice(&terms);
            same("add_slice", sum.round_ieee(), exact_sum_oracle(&terms));
        }
        for (terms, known) in i::add_iter(class) {
            let mut sum = ExactSum::new();
            if known {
                sum.add_iter(terms.iter().copied());
            } else {
                let mut rest = terms.iter().copied();
                sum.add_iter(core::iter::from_fn(move || rest.next()));
            }
            same("add_iter", sum.round_ieee(), exact_sum_oracle(&terms));
        }
        for (terms, sum) in i::round_terms(class).iter().zip(i::round(class)) {
            let expected = exact_sum_oracle(terms);
            match sum.round() {
                Ok(r) => same("round", r.get(), expected),
                Err(e) => {
                    assert_eq!(e, NumError::NotFinite);
                    assert!(!expected.is_finite(), "round refused {expected:e}");
                }
            }
        }
        for (terms, sum) in i::round_ieee_terms(class).iter().zip(i::round_ieee(class)) {
            same("round_ieee", sum.round_ieee(), exact_sum_oracle(terms));
        }
        for ((a, b), (sa, sb)) in i::merge_terms(class).iter().zip(i::merge(class)) {
            let mut merged = sa.clone();
            merged.merge(&sb);
            let all: Vec<f64> = a.iter().chain(b).copied().collect();
            same("merge", merged.round_ieee(), exact_sum_oracle(&all));
        }
    }
}

// ------------------------------------------------------------------------------------------------
// The certified enclosures of the proven maths functions: exact peak tests, domains, and the ends.
// ------------------------------------------------------------------------------------------------

/// ⌊v⌋ of an exact rational.
fn rat_floor(v: &Rat) -> Big {
    let (q, r) = v.num.div_rem(&v.den);
    match (v.num.negative, r.is_zero()) {
        (false, _) => q,
        (true, true) => q.negated(),
        (true, false) => q.negated().minus(&Big::one()),
    }
}

/// Whether the real interval [lo, hi] holds a point ≡ `a` (mod 2), decided exactly: some whole k
/// has lo ≤ 2k + a ≤ hi, that is ⌈(lo − a)/2⌉ ≤ ⌊(hi − a)/2⌋.
fn holds_point_mod_2(x: Interval, a: f64) -> bool {
    let two = Rat::int(Big::from_i64(2));
    let low = Rat::of(x.lo()).minus(&Rat::of(a)).over(&two);
    let high = Rat::of(x.hi()).minus(&Rat::of(a)).over(&two);
    let ceil_low = rat_floor(&low.negated()).negated();
    ceil_low <= rat_floor(&high)
}

#[test]
fn every_enclosure_finds_its_peaks_exactly_and_holds_its_ends() {
    use inputs::interval as i;
    // sin(πt) peaks at ½ (mod 2), troughs at 3/2; cos(πt) peaks at 0 (mod 2), troughs at 1.
    type Periodic = (
        fn(Class) -> Vec<Interval>,
        fn(Interval) -> Result<Interval, NumError>,
    );
    type Point = fn(f64) -> Result<Real, NumError>;
    let periodic: [(&str, Periodic, Point, f64, f64); 2] = [
        ("sinpi", (i::sinpi, Interval::sinpi), math::sinpi, 0.5, 1.5),
        ("cospi", (i::cospi, Interval::cospi), math::cospi, 0.0, 1.0),
    ];
    for (name, (build, enclose), point, peak, trough) in periodic {
        for class in Class::ALL {
            for x in build(class) {
                let y = enclose(x).unwrap();
                assert!(-1.0 <= y.lo() && y.hi() <= 1.0, "{name}({x:?}) = {y:?}");
                if holds_point_mod_2(x, peak) {
                    assert_eq!(y.hi(), 1.0, "{name}({x:?}) missed a peak");
                }
                if holds_point_mod_2(x, trough) {
                    assert_eq!(y.lo(), -1.0, "{name}({x:?}) missed a trough");
                }
                for t in [x.lo(), x.hi()] {
                    let v = point(t).unwrap().get();
                    assert!(
                        y.contains(v),
                        "{name}({x:?}) = {y:?} leaves out {name}({t:e})"
                    );
                }
            }
        }
    }
    // exp: refused exactly when e^hi overflows; the lower bound at least +0; the ends held.
    let overflow = f64::from_bits(0x4086_2e42_fefa_39f0);
    for class in Class::ALL {
        for x in i::exp(class) {
            match x.exp() {
                Ok(y) => {
                    assert!(x.hi() < overflow, "exp({x:?})");
                    assert!(y.lo() >= 0.0 && y.lo() <= y.hi(), "exp({x:?}) = {y:?}");
                    for t in [x.lo(), x.hi()] {
                        let v = math::exp(t).map_or(0.0, |v| v.get());
                        assert!(y.contains(v), "exp({x:?}) = {y:?} leaves out exp({t:e})");
                    }
                    if x.lo() == 0.0 {
                        assert_eq!(y.lo(), 1.0, "exp({x:?}): e^0 is exactly 1");
                    }
                }
                Err(e) => {
                    assert_eq!(e, NumError::NotFinite, "exp({x:?})");
                    assert!(x.hi() >= overflow, "exp({x:?}) refused below overflow");
                }
            }
        }
        // ln: below zero OutsideDomain, a zero lower bound NotFinite, otherwise the ends held.
        for x in i::ln(class) {
            let y = x.ln();
            if x.lo() < 0.0 {
                assert_eq!(y, Err(NumError::OutsideDomain), "ln({x:?})");
            } else if x.lo() == 0.0 {
                assert_eq!(y, Err(NumError::NotFinite), "ln({x:?})");
            } else {
                let y = y.unwrap();
                for t in [x.lo(), x.hi()] {
                    let v = math::ln(t).unwrap().get();
                    assert!(y.contains(v), "ln({x:?}) = {y:?} leaves out ln({t:e})");
                }
                if x.lo() == 1.0 {
                    assert_eq!(y.lo(), 0.0, "ln({x:?}): ln 1 is exactly 0");
                }
            }
        }
    }
    // atan2: the whole range where the box meets the branch cut from both sides, the origin alone
    // refused, and otherwise every corner but the origin held, inside [−π, π] rounded outward.
    let pi_up = std::f64::consts::PI.next_up();
    for class in Class::ALL {
        for (y, x) in i::atan2(class) {
            let r = y.atan2(x);
            if x.lo() < 0.0 && y.lo() < 0.0 && y.hi() >= 0.0 {
                assert_eq!(
                    r,
                    Ok(Interval::new(-pi_up, pi_up).unwrap()),
                    "{y:?}.atan2({x:?})"
                );
            } else if (y.lo(), y.hi(), x.lo(), x.hi()) == (0.0, 0.0, 0.0, 0.0) {
                assert_eq!(r, Err(NumError::OutsideDomain), "{y:?}.atan2({x:?})");
            } else {
                let r = r.unwrap();
                assert!(
                    -pi_up <= r.lo() && r.hi() <= pi_up,
                    "{y:?}.atan2({x:?}) = {r:?}"
                );
                for (cy, cx) in [
                    (y.lo(), x.lo()),
                    (y.lo(), x.hi()),
                    (y.hi(), x.lo()),
                    (y.hi(), x.hi()),
                ] {
                    if cy != 0.0 || cx != 0.0 {
                        let v = math::atan2(cy, cx).unwrap().get();
                        assert!(
                            r.contains(v),
                            "{y:?}.atan2({x:?}) = {r:?} leaves out ({cx:e}, {cy:e})"
                        );
                    }
                }
            }
        }
    }
    // The exact peak test itself, on cases worked by hand.
    let iv = |lo: f64, hi: f64| Interval::new(lo, hi).unwrap();
    assert!(holds_point_mod_2(iv(0.25, 0.75), 0.5));
    assert!(!holds_point_mod_2(iv(0.6, 2.4), 0.5));
    assert!(holds_point_mod_2(iv(-0.5, -0.5), 1.5));
    assert!(holds_point_mod_2(
        iv(2f64.powi(53), 2f64.powi(53) + 2.0),
        0.5
    ));
}

// ------------------------------------------------------------------------------------------------
// The generated predicate modules (G27): exact values and filters against Laplace determinants.
// ------------------------------------------------------------------------------------------------

/// Checks one generated predicate on one input: its value, when it gives one, is the exact
/// determinant (in `geom-num`'s sign convention, as the oracles above are calibrated); its exact
/// sign is that value's sign; and its filter, when it decides, gives the exact sign.
fn check_generated(
    what: &str,
    value: Option<Big>,
    exact: Option<Sign>,
    filter: Option<Sign>,
    truth: &Big,
) {
    if let Some(v) = &value {
        assert_eq!(
            v.cmp(truth),
            Ordering::Equal,
            "{what}: value {v:?}, exact {truth:?}"
        );
    }
    assert_eq!(
        exact,
        value.as_ref().map(Big::sign),
        "{what}: exact is not the value's sign"
    );
    if let Some(s) = filter {
        assert_eq!(s, truth.sign(), "{what}: the filter decided wrongly");
    }
}

#[test]
fn every_generated_value_is_the_exact_determinant() {
    use geom_num::generated as gp;
    use inputs::generated as i;
    for class in Class::ALL {
        for batch in [i::orient2d_filter(class), i::orient2d_value(class)] {
            for [a, b, c] in batch {
                let truth = orient2d_exact([a, b, c]);
                let value = gp::orient2d::value(a, b, c).map(Big::from_i128);
                check_generated(
                    "orient2d",
                    value,
                    gp::orient2d::exact(a, b, c),
                    gp::orient2d::filter(a, b, c),
                    &truth,
                );
            }
        }
        for batch in [i::orient3d_filter(class), i::orient3d_value(class)] {
            for [a, b, c, d] in batch {
                let truth = orient3d_exact([a, b, c, d]);
                let value = gp::orient3d::value(a, b, c, d).map(Big::from_i128);
                check_generated(
                    "orient3d",
                    value,
                    gp::orient3d::exact(a, b, c, d),
                    gp::orient3d::filter(a, b, c, d),
                    &truth,
                );
            }
        }
        for batch in [i::incircle_filter(class), i::incircle_value(class)] {
            for [a, b, c, d] in batch {
                let truth = incircle_exact([a, b, c, d]);
                let value = gp::incircle::value(a, b, c, d).map(|v| big_of_wide(&v));
                check_generated(
                    "incircle",
                    value,
                    gp::incircle::exact(a, b, c, d),
                    gp::incircle::filter(a, b, c, d),
                    &truth,
                );
            }
        }
        for batch in [i::insphere_filter(class), i::insphere_value(class)] {
            for [a, b, c, d, e] in batch {
                let truth = insphere_exact([a, b, c, d, e]);
                let value = gp::insphere::value(a, b, c, d, e).map(|v| big_of_wide(&v));
                check_generated(
                    "insphere",
                    value,
                    gp::insphere::exact(a, b, c, d, e),
                    gp::insphere::filter(a, b, c, d, e),
                    &truth,
                );
            }
        }
        for batch in [i::plane_side_filter(class), i::plane_side_value(class)] {
            for ([x, y, z], d, p) in batch {
                // n · p + d, exactly.
                let truth = Big::from_i128(x)
                    .times(&big(p.x))
                    .plus(&Big::from_i128(y).times(&big(p.y)))
                    .plus(&Big::from_i128(z).times(&big(p.z)))
                    .plus(&Big::from_i128(d));
                let value = gp::plane_side::value(x, y, z, d, p).map(Big::from_i128);
                check_generated(
                    "plane_side",
                    value,
                    gp::plane_side::exact(x, y, z, d, p),
                    gp::plane_side::filter(x, y, z, d, p),
                    &truth,
                );
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Determinism.
// ------------------------------------------------------------------------------------------------

#[test]
#[cfg_attr(
    target_family = "wasm",
    ignore = "wasm32-wasip1 has no threads; the fuel gate checks every digest there"
)]
fn digests_do_not_depend_on_thread_count_or_order() {
    let all: Vec<Case> = cases().collect();
    let sequential: Vec<Digests> = all.iter().map(|case| case.prepare().run()).collect();
    for threads in [1usize, 2, 3, 8] {
        let mut got: Vec<Option<Digests>> = vec![None; all.len()];
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..threads)
                .map(|t| {
                    let all = &all;
                    scope.spawn(move || {
                        // Each thread walks its share backwards, so the order differs too.
                        let mine = all.iter().enumerate().filter(|(i, _)| i % threads == t);
                        let mut out: Vec<(usize, Digests)> =
                            mine.map(|(i, case)| (i, case.prepare().run())).collect();
                        out.reverse();
                        out
                    })
                })
                .collect();
            for handle in handles {
                for (i, digest) in handle.join().unwrap() {
                    got[i] = Some(digest);
                }
            }
        });
        let got: Vec<Digests> = got.into_iter().map(Option::unwrap).collect();
        assert_eq!(got, sequential, "{threads} threads");
    }
    // A prepared benchmark gives the same digest every time it runs.
    for case in &all {
        let mut prepared = case.prepare();
        let first = prepared.run();
        assert!((0..3).all(|_| prepared.run() == first), "{}", case.name());
    }
}

#[test]
fn the_generator_is_exact_at_its_edges() {
    // FNV-1a's published vector for "foobar".
    assert_eq!(fnv1a(b"foobar"), 0x8594_4171_F739_67E8);
    let mut g = SplitMix64::new(42);
    for (lo, hi) in [
        (i64::MIN, i64::MIN + 1),
        (i64::MAX - 1, i64::MAX),
        (-1, 0),
        (i64::MIN, -1),
        (0, i64::MAX),
        (-5, i64::MAX),
    ] {
        let mut seen_lo = false;
        let mut seen_hi = false;
        for _ in 0..200 {
            let v = g.range(lo, hi);
            assert!((lo..=hi).contains(&v), "{v} outside {lo}..={hi}");
            seen_lo |= v == lo;
            seen_hi |= v == hi;
        }
        if hi.abs_diff(lo) == 1 {
            assert!(seen_lo && seen_hi, "{lo}..={hi} never reached an end");
        }
    }
    assert_eq!(g.below(0), 0);
    assert_eq!(g.index(0), 0);
    assert_eq!(g.range(7, -7), 7);
}

#[test]
fn every_input_set_has_its_own_seed() {
    // inputs/mod.rs: "every input set starts from a seed derived from its own name".
    let mut names: Vec<String> = Vec::new();
    for case in cases() {
        let op = case.op;
        let name = match op.group {
            "wide" => {
                let (width, kind) = op.name.split_once('_').unwrap();
                let limbs = match width {
                    "i256" => 4,
                    "i320" => 5,
                    "i384" => 6,
                    _ => 8,
                };
                format!("wide::{kind}::{limbs}")
            }
            group => format!("{group}::{}", op.name),
        };
        names.push(format!("{name}|{}", case.class));
    }
    let mut seeds: BTreeMap<u64, String> = BTreeMap::new();
    for entry in &names {
        let (op, class) = entry.split_once('|').unwrap();
        if let Some(other) = seeds.insert(seed_for(op, class), entry.clone()) {
            panic!("{entry} and {other} share a seed");
        }
    }
}

#[test]
fn every_batch_fits_the_simulated_first_level_data_cache() {
    // inputs/mod.rs, `BATCH`: "small enough that every batch fits the simulated 32 KiB first-level
    // data cache". The largest input types:
    let largest = [
        size_of::<(Plane, Tpi)>(),
        size_of::<[Plane; 3]>(),
        size_of::<(Wide<8>, Wide<8>)>(),
        size_of::<[P3; 5]>(),
    ];
    for bytes in largest {
        assert!(bytes * BATCH <= 32 * 1024, "{bytes} bytes × {BATCH}");
    }
}

// ------------------------------------------------------------------------------------------------
// The input classes against their documentation.
// ------------------------------------------------------------------------------------------------

#[test]
fn degenerate_meeting_points_cover_every_documented_kind() {
    // `Tpi::new` documents three ways planes fail to meet in one point: two parallel planes, three
    // planes through one line, a plane given twice. inputs/plane.rs builds only one: the third
    // plane is the first moved by a whole-step offset, so its normal is the first's exactly. A
    // Tpi::new that only compared normals for equality would pass every tpi_new benchmark and its
    // digest.
    let batch = inputs::plane::tpi_new(Degenerate);
    let parallel = |a: &Plane, b: &Plane| {
        let (n, _) = coefficients(a);
        let (m, _) = coefficients(b);
        let c = [
            n[1].times(&m[2]).minus(&n[2].times(&m[1])),
            n[2].times(&m[0]).minus(&n[0].times(&m[2])),
            n[0].times(&m[1]).minus(&n[1].times(&m[0])),
        ];
        c.iter().all(Big::is_zero)
    };
    let through_one_line = batch
        .iter()
        .filter(|[p, q, r]| !parallel(p, q) && !parallel(q, r) && !parallel(p, r))
        .count();
    let scaled_parallel = batch
        .iter()
        .filter(|[p, q, r]| {
            [(p, q), (q, r), (p, r)]
                .iter()
                .any(|(a, b)| parallel(a, b) && a.n() != b.n())
        })
        .count();
    assert!(
        through_one_line > 0 && scaled_parallel > 0,
        "of {} degenerate triples, {through_one_line} are three planes through one line and \
         {scaled_parallel} have parallel but unequal normals; the rest repeat a normal exactly",
        batch.len()
    );
}

#[test]
fn typical_intervals_come_in_rotation() {
    // inputs/interval.rs: typical intervals are "narrow positive or negative intervals and
    // intervals around zero, with bounds of ordinary size (exponents −10 to 20), in rotation".
    // `typical` picks the kind with `g.below(3)`, pseudo-randomly.
    let kind = |x: &Interval| {
        if x.lo() > 0.0 {
            0
        } else if x.hi() < 0.0 {
            1
        } else {
            2
        }
    };
    let kinds: Vec<u8> = inputs::interval::add(Typical)
        .iter()
        .map(|(x, _)| kind(x))
        .collect();
    let periodic = kinds.windows(4).all(|w| w[0] == w[3]);
    let all_three = (0..3).all(|k| kinds[..3].contains(&k));
    assert!(periodic && all_three, "kinds in batch order: {kinds:?}");
}

// ------------------------------------------------------------------------------------------------
// The output hash (chapter 23.13: "no benchmark gets faster by being wrong").
// ------------------------------------------------------------------------------------------------

#[test]
fn changed_inputs_change_the_digest() {
    // tests/golden.rs: a digest changes on "new or changed inputs". The golden file now pins each
    // benchmark's inputs as well as its results, so the inputs cannot change unnoticed: every
    // benchmark's input digest is distinct, and changing one input of a batch changes it.
    let mut by_inputs: BTreeMap<Digest, Vec<String>> = BTreeMap::new();
    for case in cases() {
        by_inputs
            .entry(case.prepare().run().inputs)
            .or_default()
            .push(case.name());
    }
    for case in growth_cases() {
        by_inputs
            .entry(case.prepare().unwrap().run().inputs)
            .or_default()
            .push(case.name());
    }
    let shared: Vec<&Vec<String>> = by_inputs.values().filter(|names| names.len() > 1).collect();
    assert!(
        shared.is_empty(),
        "benchmarks with the same inputs: {shared:#?}"
    );
    // One changed input: the golden check names the inputs.
    let name = "wide/i256_add/overflow";
    let mut batch = inputs::wide::add::<4>(Overflow);
    batch[17].0 = batch[17].0.checked_neg().unwrap();
    let results: Vec<_> = batch.iter().map(|(x, y)| x.checked_add(y)).collect();
    let changed = Digests {
        inputs: canonical::digest(&batch),
        outputs: canonical::digest(&results),
    };
    let message = golden::check(name, &changed).unwrap_err();
    assert!(message.contains("inputs changed"), "{message}");
}

/// A rounded value with any flag: what a defective `geom-num` could return. `Rounded`'s fields are
/// private, so no code outside `geom-num` can make one with a wrong flag; this stands in for it,
/// with `Rounded`'s canonical layout (the value, then the flag), which
/// `invisible_flag_bugs` checks against the real type before using it.
#[derive(Clone, Copy, PartialEq)]
struct AnyFlag<T> {
    value: T,
    exact: bool,
}

impl<T: Canonical> Canonical for AnyFlag<T> {
    fn encode(&self, h: &mut Sha256) {
        self.value.encode(h);
        self.exact.encode(h);
    }
}

/// Records in `out` each of two plausible bugs, "`exact` always true" and "`exact` always false",
/// that would change these results but not their digest.
fn invisible_flag_bugs<T: Canonical + Copy + PartialEq>(
    name: &str,
    truth: &[Result<Rounded<T>, NumError>],
    digest: Digest,
    out: &mut Vec<String>,
) {
    assert_eq!(fold(truth.iter().copied()), digest, "{name}");
    let honest: Vec<Result<AnyFlag<T>, NumError>> = truth
        .iter()
        .map(|r| {
            r.map(|v| AnyFlag {
                value: v.value(),
                exact: v.is_exact(),
            })
        })
        .collect();
    assert_eq!(
        fold(honest.iter().copied()),
        digest,
        "{name}: the stand-in's layout"
    );
    for (bug, flag) in [("exact always true", true), ("exact always false", false)] {
        let lying: Vec<Result<AnyFlag<T>, NumError>> = honest
            .iter()
            .map(|r| r.map(|v| AnyFlag { exact: flag, ..v }))
            .collect();
        if lying != honest && fold(lying.iter().copied()) == digest {
            out.push(format!("{name}: {bug}"));
        }
    }
}

#[test]
fn the_digest_sees_a_wrong_exact_flag() {
    // The old 64-bit summary put `exact` in bit 63, where an even number of wrong flags cancelled.
    // The digest is now SHA-256 of every result's full encoding, flag included.
    let mut invisible = Vec::new();
    for class in [Typical, OnGrid, Extreme, Overflow] {
        let batch = inputs::plane::to_grid(class);
        let truth: Vec<_> = batch.iter().map(Tpi::to_grid).collect();
        let digest = kernel_digest(kernels::plane::to_grid, &batch);
        invisible_flag_bugs(
            &format!("plane/to_grid/{class}"),
            &truth,
            digest,
            &mut invisible,
        );
    }
    for class in [Typical, Extreme] {
        let batch = inputs::grid::coarsen(class);
        let truth: Vec<_> = batch
            .iter()
            .map(|&(c, f, t)| grid::coarsen(c, f, t))
            .collect();
        let digest = kernel_digest(kernels::grid::coarsen, &batch);
        invisible_flag_bugs(
            &format!("grid/coarsen/{class}"),
            &truth,
            digest,
            &mut invisible,
        );
    }
    assert!(
        invisible.is_empty(),
        "wrong `exact` flags tests/golden.rs cannot see: {invisible:?}"
    );
}

#[test]
fn the_digest_sees_two_sign_errors() {
    // The same carry argument for doubles: the sign was bit 63, so two sign errors cancelled.
    let right = kernel_digest(kernels::math::round_ties_even, &[1.5, 2.5]);
    let wrong = kernel_digest(kernels::math::round_ties_even, &[-1.5, -2.5]);
    assert_ne!(right, wrong, "[2, 2] and [−2, −2] have one digest");
    let batch = inputs::math::unproven::sin(Typical);
    let mut results: Vec<Result<Real, NumError>> =
        batch.iter().copied().map(math::unproven::sin).collect();
    assert_eq!(
        fold(results.iter().copied()),
        kernel_digest(kernels::math::unproven::sin, &batch)
    );
    let flip = |r: Result<Real, NumError>| r.and_then(|v| Real::new(-v.get()));
    results[0] = flip(results[0]);
    results[1] = flip(results[1]);
    let right = kernel_digest(kernels::math::unproven::sin, &batch);
    assert_ne!(fold(results), right, "sin with two signs wrong passes");
}

#[test]
fn the_digest_tells_two_to_the_63_from_two_to_the_64() {
    // A `Wide` was summarised as rotl(limb0, L−1) ^ … ^ limb(L−1): bit 63 of one limb landed where
    // bit 0 of the next did, so a carry kept in the wrong limb was invisible.
    let zero = Wide::<4>::ZERO;
    let low = kernel_digest(kernels::wide::add::<4>, &[(Wide::from_i128(1 << 63), zero)]);
    let high = kernel_digest(kernels::wide::add::<4>, &[(Wide::from_i128(1 << 64), zero)]);
    assert_ne!(low, high, "2^63 and 2^64 have one digest");
}

#[test]
fn the_digest_tells_the_plane_x_0_from_the_plane_y_0() {
    // A plane was summarised as n_x ^ rotl(n_y, 16) ^ rotl(n_z, 32) ^ rotl(d, 48).
    let q = |x, y, z| P3::new(x, y, z).unwrap();
    // n = (2^16, 0, 0) and n = (0, 1, 0), both with d = 0.
    let x0 = kernel_digest(
        kernels::plane::through,
        &[[q(0, 0, 0), q(0, 256, 0), q(0, 0, 256)]],
    );
    let y0 = kernel_digest(
        kernels::plane::through,
        &[[q(0, 0, 0), q(0, 0, 1), q(1, 0, 0)]],
    );
    assert_ne!(x0, y0, "the planes x = 0 and y = 0 have one digest");
}

#[test]
fn the_digest_tells_distinct_grid_points_apart() {
    // A point was summarised as x ^ rotl(y, 16) ^ rotl(z, 32): (65536, 0, 0) and (0, 1, 0), 10.24 mm
    // apart at q0, were one word. A to_grid that returned the wrong one would have passed.
    let meeting = |x: i64, y: i64, z: i64| {
        let q = |a, b, c| P3::new(a, b, c).unwrap();
        let px = Plane::through(q(x, y, z), q(x, y + 1, z), q(x, y, z + 1)).unwrap();
        let py = Plane::through(q(x, y, z), q(x, y, z + 1), q(x + 1, y, z)).unwrap();
        let pz = Plane::through(q(x, y, z), q(x + 1, y, z), q(x, y + 1, z)).unwrap();
        Tpi::new(px, py, pz).unwrap()
    };
    let a = meeting(65_536, 0, 0);
    let b = meeting(0, 1, 0);
    assert_ne!(a.to_grid(), b.to_grid());
    let digest = |t: Tpi| kernel_digest(kernels::plane::to_grid, &[t]);
    assert_ne!(digest(a), digest(b), "one digest");
}

#[test]
fn the_digest_tells_a_result_from_an_error() {
    // Errors were summarised as small constants rotated by 8, which were also valid coordinates:
    // Err(WrongDirection) was the word of Ok(4,194,304 steps).
    let c = Coord::new(0x0040_0000).unwrap();
    let ok = kernel_digest(kernels::grid::refine, &[(c, GridStep::Q0, GridStep::Q0)]);
    let backwards = (Coord::ZERO, GridStep::Q1, GridStep::Q0);
    let refused = kernel_digest(kernels::grid::refine, &[backwards]);
    let (zero, from, to) = backwards;
    assert_eq!(grid::refine(zero, from, to), Err(NumError::WrongDirection));
    assert_ne!(
        ok, refused,
        "Ok(4194304) and Err(WrongDirection) have one digest"
    );
}

#[test]
fn every_benchmark_checks_its_golden_digests_where_it_runs() {
    // Chapter 23.13, "Output hash: each benchmark checks its output's content hash against the
    // golden value". The gate checks after measuring (teardown), the wall-clock harness before
    // timing, the fuel guest on wasm32; the golden file pins every catalogued benchmark.
    assert!(GATE.contains("golden::check(run.name, &run.bench.digests())"));
    let functions = geom_bench::catalogue::OPS.len() + geom_bench::catalogue::GROWTH.len();
    assert_eq!(
        GATE.matches("#[library_benchmark(teardown = verify)]")
            .count(),
        functions
    );
    let wall = include_str!("../benches/wall_clock.rs");
    assert!(wall.contains("golden::check(name, &prepared.run())"));
    let guest = include_str!("../src/guest.rs");
    assert!(guest.contains("golden::check(name, &bench.run())"));
    let recorded: BTreeSet<&str> = golden::parse(golden::GOLDEN)
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    let all = geom_bench::catalogue::names();
    assert_eq!(recorded.len(), all.len());
    assert!(all.iter().all(|n| recorded.contains(n.as_str())));
}

// ------------------------------------------------------------------------------------------------
// The gate against chapters 23.13 and 28.14 and board entry G18 ("gungraun instruction and cache
// counts, DHAT peak heap and wasmtime fuel on every pull request, ...; growth checks at n, 2n and
// 4n; ... a rise over 5% blocks and every change over 1% is listed").
// ------------------------------------------------------------------------------------------------

/// The text of the workflow step named `name`, up to the next step.
fn workflow_step(name: &str) -> &'static str {
    let start = WORKFLOW
        .find(&format!("- name: {name}"))
        .unwrap_or_else(|| panic!("no step {name}"));
    let rest = &WORKFLOW[start + 1..];
    let end = rest.find("\n      - name:").map_or(rest.len(), |e| e + 1);
    &WORKFLOW[start..start + end]
}

#[test]
fn cache_misses_are_gated() {
    // 23.13: "Instructions executed, L1 and last-level cache misses, and estimated cycles ...
    // Blocks: a merge, any measure over 5%."
    let misses = ["I1mr", "D1mr", "D1mw", "ILmr", "DLmr", "DLmw"];
    let gated: Vec<&str> = geom_bench::GATED_METRICS.to_vec();
    assert!(
        misses
            .iter()
            .any(|m| gated.contains(m) || GATE.contains(&format!("EventKind::{m}"))),
        "gated measures: {gated:?}; no L1 or last-level miss count is gated"
    );
}

#[test]
fn peak_heap_is_gated_with_dhat() {
    // 23.13, "Memory: peak heap bytes from Gungraun's DHAT runs ... Blocks: a merge, over 5%".
    assert!(
        GATE.contains("Dhat") || WORKFLOW.contains("dhat"),
        "no DHAT run in the gate"
    );
}

#[test]
fn webassembly_fuel_is_gated() {
    // 23.13, "WebAssembly fuel ... wasm32, every pull request ... Blocks: a merge, over 5%".
    assert!(
        WORKFLOW.contains("wasmtime") || WORKFLOW.contains("wasm32"),
        "no wasm32 fuel run"
    );
}

#[test]
fn growth_is_checked_at_n_2n_and_4n() {
    // 28.14 and G18: "Each scale benchmark runs at n, 2n and 4n, on every pull request, and its
    // work must grow no faster than the operation's stated cost."
    //
    // Corrected: this test used to read the batch lengths of every benchmark. A batch is the
    // harness repeating an operation on independent inputs, not the operation's size: the numbers
    // layer's operations have one size parameter, the width of a fixed-width integer, and those are
    // the scale benchmarks (catalogue::GROWTH), run at 2, 4 and 8 limbs. This checks they exist at
    // n, 2n and 4n, that the check fails on faster growth, and that the workflow runs it.
    use geom_bench::catalogue::GROWTH;
    use geom_bench::counts::GrowthCheck;
    assert!(!GROWTH.is_empty());
    for g in GROWTH {
        let [n, n2, n4] = g.sizes;
        assert!(n2 == 2 * n && n4 == 4 * n, "{}: {:?}", g.name, g.sizes);
        let sizes: BTreeSet<usize> = growth_cases()
            .filter(|c| c.growth.name == g.name)
            .map(|c| c.size)
            .collect();
        assert_eq!(sizes, BTreeSet::from(g.sizes));
    }
    // A quadratic operation stated as linear fails; stated as quadratic it passes.
    let check = |exponent| GrowthCheck {
        name: "t",
        exponent,
        sizes: [2, 4, 8],
        counts: [Some(1000), Some(4000), Some(16000)],
    };
    assert!(!check(1).passed());
    assert!(check(2).passed());
    assert!(
        WORKFLOW.contains("\"$COUNTS\" growth"),
        "the workflow does not run the growth check"
    );
}

#[test]
fn the_toolchain_is_pinned() {
    // 28.14: "The merge base and the change are built in the same job with the same pinned
    // toolchain". Guards against installing whatever `stable` is that day.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../..");
    let pinned_file =
        root.join("rust-toolchain.toml").exists() || root.join("rust-toolchain").exists();
    assert!(
        pinned_file || !WORKFLOW.contains("dtolnay/rust-toolchain@stable"),
        "no rust-toolchain file, and the workflow uses dtolnay/rust-toolchain@stable"
    );
    // Every toolchain is installed as kernel.yml installs its own, by the commit of
    // dtolnay/rust-toolchain kernel.yml pins, with an exact `toolchain:`, here the one each tree
    // pins in geom-bench/toolchain.txt (the first job reads it and refuses anything but an exact
    // release); nothing installs one another way.
    assert!(!WORKFLOW.contains("rustup toolchain install") && !WORKFLOW.contains("rustup default"));
    let kernel_install = KERNEL_WORKFLOW
        .lines()
        .map(|line| line.trim().trim_start_matches("- "))
        .find(|line| line.starts_with("uses: dtolnay/rust-toolchain@"))
        .expect("kernel.yml installs its toolchain with dtolnay/rust-toolchain");
    let lines: Vec<&str> = WORKFLOW.lines().collect();
    let installs: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim_start().starts_with("uses:") && line.contains("toolchain"))
        .map(|(at, _)| at)
        .collect();
    assert!(!installs.is_empty());
    for at in installs {
        assert_eq!(lines[at].trim(), kernel_install);
        // The step's `with:` block: the lines after `uses:`, up to the next step.
        let with: Vec<&str> = lines[at + 1..]
            .iter()
            .map(|line| line.trim())
            .take_while(|line| !line.is_empty() && !line.starts_with("- "))
            .collect();
        assert!(
            with.contains(&"toolchain: ${{ env.HEAD_TOOLCHAIN }}")
                || with.contains(&"toolchain: ${{ env.BASE_TOOLCHAIN }}"),
            "{with:?}"
        );
    }
    let step = workflow_step("Decide what to measure");
    assert!(step.contains("toolchain.txt") && step.contains("'^[0-9]+\\.[0-9]+\\.[0-9]+$'"));
}

#[test]
fn a_dependency_bump_runs_the_gate() {
    // Principle 12 blocks speed regressions; a libm or toolchain bump in Cargo.lock changes every
    // maths count, so such a pull request must be gated.
    //
    // The workflow runs, as kernel.yml does, on pull requests and on pushes to main whose changes
    // match its path filters, and its first job checks the same list again. Both filters and that
    // decision must cover the kernel, Cargo.lock, the workspace manifest, the toolchain files,
    // Cargo's configuration and the workflow itself, and nothing may be filtered out.
    let lines: Vec<&str> = WORKFLOW.lines().collect();
    let start = lines.iter().position(|line| *line == "on:").unwrap();
    let end = lines
        .iter()
        .position(|line| *line == "permissions:")
        .unwrap();
    let on = &lines[start..end];
    assert!(
        !on.iter().any(|line| line.contains("paths-ignore:")),
        "the trigger is: {on:#?}"
    );
    // The lines of one event's settings, under `  <event>:`.
    let event = |name: &str| -> Vec<&str> {
        let at = on
            .iter()
            .position(|line| *line == format!("  {name}:"))
            .unwrap_or_else(|| panic!("the workflow does not run on {name}: {on:#?}"));
        on[at + 1..]
            .iter()
            .take_while(|line| line.starts_with("    "))
            .copied()
            .collect()
    };
    let filters = [
        "\"projects/engines/geometry/**\"",
        "\"Cargo.toml\"",
        "\"Cargo.lock\"",
        "\"rust-toolchain\"",
        "\"rust-toolchain.toml\"",
        "\".cargo/**\"",
        "\".github/workflows/kernel-bench.yml\"",
    ];
    for name in ["pull_request", "push"] {
        let settings = event(name);
        let paths: Vec<&str> = settings
            .iter()
            .filter_map(|line| line.strip_prefix("      - "))
            .collect();
        assert_eq!(paths, filters, "{name} path filters: {settings:#?}");
    }
    assert!(
        event("push").contains(&"    branches: [main]"),
        "pushes to main are not gated: {on:#?}"
    );
    let step = workflow_step("Decide what to measure");
    let needles = [
        "Cargo\\.lock$",
        "Cargo\\.toml$",
        "rust-toolchain(\\.toml)?$",
        "\\.cargo/",
        "kernel-bench\\.yml$",
        "projects/engines/geometry/",
    ];
    for needle in needles {
        assert!(
            step.contains(needle),
            "{needle} does not trigger the gate:\n{step}"
        );
    }
}

#[test]
fn only_the_kernel_is_counted() {
    // Crate documentation, noise point 5: "Only the kernel is counted ... building the inputs,
    // hashing the results and freeing memory happen outside the measured function." Gungraun's
    // toggle-collect covers the whole benchmark function, so a benchmark that took its batch `Vec`
    // by value and returned only a digest would free the batch (glibc `free`) inside the measured
    // region. Guards against that shape.
    let by_value: Vec<&str> = GATE
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("fn ") && l.contains("(batch: Vec<") && l.ends_with("-> u64 {"))
        .collect();
    assert!(
        by_value.is_empty(),
        "{} benchmarks drop their batch while counted",
        by_value.len()
    );
    // Every benchmark function takes the leaked benchmark and hands it back, so nothing is freed
    // while counting, and the teardown (outside the counted region) checks the golden digests.
    let functions: Vec<&str> = GATE
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("fn ") && l.contains("(run: ") && l.contains(" -> "))
        .collect();
    let expected = geom_bench::catalogue::OPS.len() + geom_bench::catalogue::GROWTH.len();
    assert_eq!(functions.len(), expected);
    let signature = "(run: &'static mut Named) -> &'static mut Named {";
    assert!(functions.iter().all(|l| l.ends_with(signature)));
    assert!(GATE.contains("Box::leak"));
}

#[test]
fn the_pull_request_cannot_loosen_its_own_gate() {
    // If the 5% limit lived only in benches/instructions/gate.rs of the tree under test, a pull
    // request that edited GATE_PERCENT (and the test that pins it) would gate itself at whatever
    // it chose. The gate step must pass the workflow's own limits, which take precedence.
    let step = workflow_step("Benchmark the pull request against the merge base");
    assert!(
        step.contains("--callgrind-limits"),
        "the gate step sets no limits:\n{step}"
    );
}

#[test]
fn a_removed_benchmark_fails_the_gate() {
    // Renaming a benchmark drops it from the comparison, so a workflow that only warned would let
    // a rename hide a regression. The step must fail on an unretired removal.
    let step = workflow_step("List changes over 1% and benchmarks not compared");
    let fails = step.contains("::error::") || !step.trim_end().ends_with("exit 0");
    assert!(fails, "a removed benchmark only raises a warning:\n{step}");
}

#[test]
fn an_unfinished_listing_fails_the_gate() {
    // 23.13: every change over 1% is listed. The two 1% listings exit with a failure whenever they
    // list something, so the step knows a listing finished only by Gungraun's closing line. One
    // without it could leave a change over 1% unlisted while the summary said "None.", so it is an
    // error that fails the step after everything else is reported, and the summary says so.
    let step = workflow_step("List changes over 1% and benchmarks not compared");
    let lines: Vec<&str> = step.lines().map(str::trim).collect();
    let check = lines
        .iter()
        .position(|line| *line == "if ! grep -q '^Gungraun result:' \"$log\"; then")
        .expect("the listing step checks that each listing finished");
    let unfinished: Vec<&str> = lines[check + 1..]
        .iter()
        .copied()
        .take_while(|line| *line != "fi")
        .collect();
    assert!(
        unfinished
            .iter()
            .any(|line| line.starts_with("echo \"::error::"))
            && unfinished.contains(&"incomplete=1"),
        "an unfinished listing only warns: {unfinished:#?}"
    );
    assert!(!step.contains("::warning::"), "{step}");
    let fails = ["if [ \"$incomplete\" -ne 0 ]; then", "exit 1", "fi"];
    assert!(
        lines.windows(3).any(|w| w == fails.as_slice()),
        "an unfinished listing does not fail the step:\n{step}"
    );
    let summary = workflow_step("Write the summary");
    assert!(
        summary.contains("the listing did not complete"),
        "the summary reports an unfinished listing as listing nothing:\n{summary}"
    );
}

#[test]
fn every_step_is_written_for_bash_e() {
    // GitHub runs every `run:` step with `bash -e`, and `set -uo pipefail` leaves `-e` on. On #190
    // (run 39598200605) the fuel job's summary printed the comparison's report with `cat`; with no
    // comparison to make (the merge base had no fuel benchmarks) there was no report, `cat` failed,
    // and `-e` ended the job in failure although every gate had passed. The same shell never
    // reached a `status=$?` after a failing check, so the check's annotation was never written, and
    // the listing of changes over 1% would have ended at the first change it listed.
    //
    // So: an exit status a step reports is caught where the command fails (`|| status=$?`), and a
    // step that runs on with `-e` off says so (`set +e`) before `set -uo pipefail`.
    let lines: Vec<&str> = WORKFLOW.lines().map(str::trim).collect();
    for (at, line) in lines.iter().enumerate() {
        // Comments may describe the rule.
        if line.starts_with('#') {
            continue;
        }
        if line.contains("=$?") {
            assert!(
                line.starts_with("|| ") || line.contains(" || "),
                "kernel-bench.yml line {}: `bash -e` ends the step before this reads a failing \
                 exit status: {line}",
                at + 1
            );
        }
        if *line == "set -uo pipefail" {
            let before = at.checked_sub(1).and_then(|b| lines.get(b)).copied();
            assert_eq!(
                before,
                Some("set +e"),
                "kernel-bench.yml line {}: GitHub's `-e` is still on here; write `set -euo \
                 pipefail`, or turn `-e` off first",
                at + 1
            );
        }
    }
    // Both summaries only report: they run whatever an earlier step did, with `-e` off, and pass.
    // A step is its `- name:` line and the lines indented under it (a summary is the last step of
    // its job, so the next step or job ends it).
    let raw: Vec<&str> = WORKFLOW.lines().collect();
    let summaries: Vec<Vec<&str>> = raw
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim() == "- name: Write the summary")
        .map(|(at, _)| {
            raw[at + 1..]
                .iter()
                .take_while(|line| line.trim().is_empty() || line.starts_with("        "))
                .map(|line| line.trim())
                .filter(|line| !line.is_empty())
                .collect()
        })
        .collect();
    assert_eq!(summaries.len(), 2, "one summary per measuring job");
    for step in summaries {
        assert!(step.contains(&"if: always()"), "{step:#?}");
        assert!(step.contains(&"set +e"), "{step:#?}");
        assert_eq!(step.last(), Some(&"exit 0"), "{step:#?}");
    }
}

#[test]
fn the_gate_limit_is_principle_12s() {
    // Sanity, and must pass: the soft limits are 5% on the two measures the crate names.
    assert_eq!(geom_bench::GATE_PERCENT, 5.0);
    assert!(GATE.contains("(EventKind::Ir, GATE_PERCENT)"));
    assert!(include_str!("../benches/instructions/main.rs").contains("config = gate::config();"));
}

#[test]
fn every_class_is_measured_somewhere() {
    // Sanity, and must pass: no class is defined and never used.
    for class in Class::ALL {
        assert!(
            cases().any(|case| case.class == class),
            "{class} is never benchmarked"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// The line-plane points and the crossings (G29): exact values from their definitions.
// ---------------------------------------------------------------------------------------------

/// N homogeneous numerators over a denominator rounded half to even to the grid, as every
/// implicit point's `to_grid` documents: the coordinates and whether all were exact, or the first
/// coordinate outside the frame.
fn commit_exact<const N: usize>(x: &[Big; N], w: &Big) -> Result<([i64; N], bool), NumError> {
    let m = Big::from_i64(COORD_MAX);
    let mut coords = [0i64; N];
    let mut exact = true;
    for (coord, xi) in coords.iter_mut().zip(x) {
        let (q, e) = round_half_even(xi, w);
        if q > m || q < m.negated() {
            let saturated = if q.negative { i128::MIN } else { i128::MAX };
            return Err(NumError::OutOfRange {
                value: q.to_i128().unwrap_or(saturated),
            });
        }
        *coord = i64::try_from(q.to_i128().unwrap()).unwrap();
        exact &= e;
    }
    Ok((coords, exact))
}

/// The point where the line from p to q meets the plane (n, d): with sₚ = n · p + d and
/// s_q = n · q + d, X = s_q p − sₚ q and w = s_q − sₚ.
fn lpi_exact(l: &Lpi) -> ([Big; 3], Big) {
    let (n, d) = coefficients(l.plane());
    let [p, q] = l.points();
    let coords = |r: P3| [big(r.x), big(r.y), big(r.z)];
    let (p, q) = (coords(p), coords(q));
    let offset = |r: &[Big; 3]| {
        n.iter()
            .zip(r)
            .fold(d.clone(), |acc, (ni, ri)| acc.plus(&ni.times(ri)))
    };
    let (sp, sq) = (offset(&p), offset(&q));
    let x = [0, 1, 2].map(|i| sq.times(&p[i]).minus(&sp.times(&q[i])));
    (x, sq.minus(&sp))
}

/// The point where the line from a to b crosses the line from c to d: with oa and ob the cross
/// products (d − c) × (a − c) and (d − c) × (b − c), X = oa b − ob a and w = oa − ob.
fn ssi_exact(s: &Ssi) -> ([Big; 2], Big) {
    let [a, b, c, d] = s.points().map(|r| [big(r.x), big(r.y)]);
    let cross = |o: &[Big; 2], e: &[Big; 2], f: &[Big; 2]| {
        e[0].minus(&o[0])
            .times(&f[1].minus(&o[1]))
            .minus(&e[1].minus(&o[1]).times(&f[0].minus(&o[0])))
    };
    let (oa, ob) = (cross(&c, &d, &a), cross(&c, &d, &b));
    let x = [0, 1].map(|i| oa.times(&b[i]).minus(&ob.times(&a[i])));
    (x, oa.minus(&ob))
}

#[test]
fn line_plane_points_and_crossings_agree_with_exact_values_on_every_input() {
    use inputs::crossing as c;
    use inputs::plane as p;
    for (a, b, plane) in p::lpi_new(Degenerate) {
        assert_eq!(Lpi::new(a, b, plane), Err(NumError::Degenerate));
    }
    for class in [Typical, Extreme] {
        for l in p::lpi_homogeneous(class) {
            let (x, w) = l.homogeneous().unwrap();
            let (ex, ew) = lpi_exact(&l);
            assert_eq!(Big::from_i128(w), ew, "{class}");
            assert_eq!(x.map(|xi| big_of_wide(&xi)), ex, "{class}");
        }
    }
    for class in [Typical, Degenerate, NearlyDegenerate, Extreme] {
        for (plane, l) in p::side_of_lpi(class) {
            let (n, d) = coefficients(&plane);
            let (x, w) = lpi_exact(&l);
            let value = n
                .iter()
                .zip(&x)
                .fold(d.times(&w), |acc, (ni, xi)| acc.plus(&ni.times(xi)));
            assert_eq!(
                plane.side_of_lpi(&l),
                Ok(value.sign().mul(w.sign())),
                "{class}"
            );
        }
    }
    for class in [Typical, OnGrid, Extreme, Overflow] {
        for l in p::lpi_to_grid(class) {
            let (x, w) = lpi_exact(&l);
            let want = commit_exact(&x, &w).map(|([a, b, c], e)| (P3::new(a, b, c).unwrap(), e));
            assert_eq!(parts(l.to_grid()), want, "{class}");
        }
    }
    for [a, b, cc, d] in c::ssi_new(Degenerate) {
        assert_eq!(Ssi::new(a, b, cc, d), Err(NumError::Degenerate));
    }
    for class in [Typical, Extreme] {
        for s in c::ssi_homogeneous(class) {
            let (x, w) = s.homogeneous().unwrap();
            let (ex, ew) = ssi_exact(&s);
            assert_eq!(Big::from_i128(w), ew, "{class}");
            assert_eq!(x.map(Big::from_i128), ex, "{class}");
        }
    }
    for class in [Typical, Degenerate, NearlyDegenerate, Extreme] {
        for (s, p, q) in c::ssi_orient2d(class) {
            let (x, w) = ssi_exact(&s);
            let [px, py] = [big(p.x), big(p.y)];
            let (qx, qy) = (big(q.x).minus(&px), big(q.y).minus(&py));
            // (q − p) × (X − w p): orient2d(p, q, X / w) times w².
            let rx = x[0].minus(&w.times(&px));
            let ry = x[1].minus(&w.times(&py));
            let value = qx.times(&ry).minus(&qy.times(&rx));
            assert_eq!(s.orient2d(p, q), Ok(value.sign().mul(w.sign())), "{class}");
        }
    }
    for class in [Typical, OnGrid, Extreme, Overflow] {
        for s in c::ssi_to_grid(class) {
            let (x, w) = ssi_exact(&s);
            let want = commit_exact(&x, &w).map(|([a, b], e)| (P2::new(a, b).unwrap(), e));
            assert_eq!(parts(s.to_grid()), want, "{class}");
        }
    }
}
