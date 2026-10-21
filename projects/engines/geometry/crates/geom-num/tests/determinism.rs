//! The golden hash: a fixed corpus of calls into every part of `geom-num`, hashed, must give the
//! same 64-bit value on every target.
//!
//! The kernel promises the same bits on every machine (chapter 21): one model, one geometry, on
//! Windows, Linux, macOS and in the browser. This test holds the numbers layer to that promise. It
//! makes a fixed corpus of more than 20,000 calls (the test prints the exact count):
//!
//! - every exact predicate (`orient2d`, `orient3d`, `incircle`, `insphere`), on random points at
//!   every scale, on collinear, coplanar, cocircular and cospherical constructions with and without
//!   a one-step nudge, on repeated points and at the frame's extremes;
//! - plane construction and classification (`Plane::through`, `Plane::side`), from random,
//!   collinear, repeated and extreme points, against points on, near and off each plane;
//! - three-plane points and their grid commits (`Tpi::new`, `Tpi::homogeneous`, `Tpi::to_grid`,
//!   `Plane::side_of_tpi`): planes through one grid point, near one, meeting on exact ties (round
//!   half to even), meeting far outside the frame, parallel, in a pencil and repeated;
//! - `refine` and `coarsen` between every pair of grid steps, near the halfway points and at the
//!   edges of the frame;
//! - wide-integer arithmetic in `I256`, `I320`, `I384` and `I512`, at the edges of each range;
//! - the error-free transformations and every interval operation, on doubles of every exponent,
//!   with subnormals, the overflow edge and the special values;
//! - every function in `geom_num::math`, over its domain.
//!
//! The corpus comes from a 64-bit linear congruential generator, written here, from a fixed seed.
//! It uses integer arithmetic and exact conversions only, so the inputs are the same on every
//! target by construction and only the kernel's results can differ.
//!
//! Every result is written in a canonical little-endian byte encoding, in a fixed field order (see
//! `Recorder`), and hashed with FNV-1a 64, also written here and checked against its published
//! test vectors. Doubles go in as their bits, so a difference of one unit in the last place shows.
//! A NaN would be written as one canonical pattern, because Rust does not fix a NaN's sign or
//! payload (RFC 3514), but the domains here are chosen so that no result is NaN, and the test
//! checks that.
//!
//! **The same constant must hold on every target**: Linux, Windows and macOS on x86-64 and arm64,
//! and wasm32-wasip1 under wasmtime (`.github/workflows/kernel.yml`), in debug and release, with
//! overflow checks on or off. A changed hash is one of two things. Either it is a bug: some target
//! computes different bits, so the same model would give different geometry on different machines.
//! Or the kernel's results were changed on purpose, which is a deliberate kernel edition change
//! (chapter 21, section 28.13): its pull request updates `GOLDEN`, says which results changed and
//! why, and a kernel owner approves it. A change to this corpus changes the hash too and is
//! reviewed the same way. Never update the constant to make one target pass.
//!
//! To help find the cause when one target disagrees, the test prints a hash per section: compare
//! the CI logs of two targets to see which part of the kernel differs. It also checks that the
//! corpus still reaches every outcome it is built to reach (zero signs, degenerate constructions,
//! overflow, rounding, every error), so a corpus that silently stopped testing the hard cases
//! fails.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken corpus must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::cmp::Ordering;
use std::collections::BTreeSet;

use geom_num::eft::{fast_two_sum, split, two_prod, two_sum};
use geom_num::grid::{COORD_MAX, coarsen, refine};
use geom_num::interval::Interval;
use geom_num::math;
use geom_num::plane::{Plane, Tpi};
use geom_num::predicate::{incircle, insphere, orient2d, orient3d};
use geom_num::{Coord, GridStep, NumError, P2, P3, Rounded, Sign, Wide};

/// The golden hash of the corpus.
///
/// The same constant must hold on every target: Linux, Windows and macOS on x86-64 and arm64, and
/// wasm32-wasip1, in debug and release, with overflow checks on or off. A change is either a bug (a
/// target computes different bits, so the same model would give different geometry on different
/// machines) or a deliberate kernel edition change, made in a pull request that updates this
/// constant, says which, and is approved by a kernel owner. See the module documentation.
const GOLDEN: u64 = 0x33fb_4e73_065b_2f06;

/// The corpus makes at least this many calls into the kernel.
const MIN_CALLS: u64 = 20_000;

/// FNV-1a 64-bit offset basis (Fowler, Noll and Vo).
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// FNV-1a 64-bit prime, 2^40 + 2^8 + 0xb3.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Multiplier of the corpus generator: Knuth's MMIX linear congruential generator modulo 2^64
/// (*The Art of Computer Programming*, volume 2, section 3.3.4).
const LCG_MUL: u64 = 6_364_136_223_846_793_005;

/// Increment of the corpus generator (Knuth's MMIX).
const LCG_ADD: u64 = 1_442_695_040_888_963_407;

/// The generator's seed: "SPAREKIT" in ASCII.
const SEED: u64 = 0x5350_4152_454b_4954;

/// The bits any NaN would be written as: the quiet NaN with a clear sign and an empty payload.
const CANONICAL_NAN: u64 = 0x7ff8_0000_0000_0000;

/// The sign bit of a double.
const SIGN_BIT: u64 = 1 << 63;

/// The largest coordinate magnitude, 2^40 − 1 grid steps.
const M: i64 = COORD_MAX;

/// The grid steps, coarsest first.
const STEPS: [GridStep; 3] = [GridStep::Q0, GridStep::Q1, GridStep::Q2];

/// The six orders of three things.
const PERMUTATIONS: [[usize; 3]; 6] = [
    [0, 1, 2],
    [0, 2, 1],
    [1, 0, 2],
    [1, 2, 0],
    [2, 0, 1],
    [2, 1, 0],
];

/// The twelve grid points at distance 5 from the origin.
const CIRCLE5: [[i64; 2]; 12] = [
    [5, 0],
    [-5, 0],
    [0, 5],
    [0, -5],
    [3, 4],
    [3, -4],
    [-3, 4],
    [-3, -4],
    [4, 3],
    [4, -3],
    [-4, 3],
    [-4, -3],
];

/// Finite doubles where floating-point code goes wrong: both zeros, ones, a half and halfway
/// cases, the smallest normal and subnormal values of both signs, the largest finite values and
/// the neighbours of 1.
const SPECIAL: [f64; 16] = [
    0.0,
    -0.0,
    1.0,
    -1.0,
    0.5,
    -1.5,
    2.5,
    f64::MIN_POSITIVE,
    -f64::MIN_POSITIVE,
    f64::from_bits(1),
    f64::from_bits(0x800f_ffff_ffff_ffff),
    f64::MAX,
    f64::MIN,
    f64::EPSILON,
    f64::from_bits(0x3fef_ffff_ffff_ffff),
    f64::from_bits(0x3ff0_0000_0000_0001),
];

// ---------------------------------------------------------------------------------------------
// The hash and the canonical encoding.
// ---------------------------------------------------------------------------------------------

/// FNV-1a, 64 bits: for each byte, xor it into the state, then multiply by the prime modulo 2^64.
#[derive(Clone, Copy)]
struct Fnv(u64);

impl Fnv {
    /// The empty hash, the offset basis.
    const fn new() -> Fnv {
        Fnv(FNV_OFFSET)
    }

    /// Hashes `bytes`, in order.
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(FNV_PRIME);
        }
    }
}

/// The kernel operations the corpus calls. A call writes its discriminant first, so the byte
/// stream says which operation each result came from. The discriminants are part of the encoding:
/// never renumber them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Op {
    Orient2d = 1,
    Orient3d = 2,
    Incircle = 3,
    Insphere = 4,
    PlaneThrough = 5,
    PlaneSide = 6,
    TpiNew = 7,
    TpiHomogeneous = 8,
    ToGrid = 9,
    SideOfTpi = 10,
    Refine = 11,
    Coarsen = 12,
    WideConst = 13,
    WideFromInt = 14,
    WideAdd = 15,
    WideSub = 16,
    WideMul = 17,
    WideNeg = 18,
    WideCmp = 19,
    WideSignum = 20,
    WideResize = 21,
    WideToI128 = 22,
    TwoSum = 23,
    FastTwoSum = 24,
    TwoProd = 25,
    Split = 26,
    IntervalNew = 27,
    IntervalAdd = 28,
    IntervalSub = 29,
    IntervalMul = 30,
    IntervalDiv = 31,
    IntervalSquare = 32,
    IntervalSqrt = 33,
    IntervalSqrtNonneg = 34,
    IntervalNeg = 35,
    Contains = 36,
    CertainSign = 37,
    Math = 38,
}

/// Writes every result into the hash in the canonical encoding, and keeps the tallies.
///
/// The encoding, every value little-endian, in this field order:
///
/// - a call: its `Op` discriminant as one byte, then its result (a maths call also writes the
///   function's index, one byte, before its result);
/// - `Sign` and `Ordering`: one byte, 0 negative or less, 1 zero or equal, 2 positive or greater;
/// - `bool`: one byte, 0 or 1;
/// - integers: their two's-complement bytes at their own width (`i64` 8 bytes, `i128` 16);
/// - `Wide<L>`: its L limbs, least significant first, 8 bytes each;
/// - `f64`: its 8 IEEE 754 bytes, with any NaN as `CANONICAL_NAN`;
/// - `Option`: 0 for `None`, or 1 then the value;
/// - `Result`: 0 then the value, or the error;
/// - `NumError`: one byte, 1 out of range (then its `i128` value), 2 degenerate, 3 not finite,
///   4 wrong direction, 5 divisor contains zero, 6 negative square root, 7 empty interval;
/// - a point: x, y, z; a `Rounded`: the value, then `exact`; a plane: the normal's x, y, z, then
///   the offset; an interval: lo, then hi; a pair of doubles: the first, then the second.
struct Recorder {
    /// The hash of everything.
    total: Fnv,
    /// The hash of the current section.
    section: Fnv,
    /// Calls so far.
    calls: u64,
    /// Calls in the current section.
    section_calls: u64,
    /// NaN results so far.
    nans: u64,
    /// The operation being recorded.
    op: Op,
    /// The outcome classes each operation has reached.
    seen: BTreeSet<(Op, &'static str)>,
    /// Each finished section: name, calls, hash.
    report: Vec<(&'static str, u64, u64)>,
}

impl Recorder {
    /// Nothing recorded yet.
    fn new() -> Recorder {
        Recorder {
            total: Fnv::new(),
            section: Fnv::new(),
            calls: 0,
            section_calls: 0,
            nans: 0,
            op: Op::Orient2d,
            seen: BTreeSet::new(),
            report: Vec::new(),
        }
    }

    /// Closes the current section under `name`, keeping its count and hash for the report.
    fn end_section(&mut self, name: &'static str) {
        self.report.push((name, self.section_calls, self.section.0));
        self.section = Fnv::new();
        self.section_calls = 0;
    }

    /// Starts recording one call of `op`.
    fn call(&mut self, op: Op) {
        self.op = op;
        self.calls += 1;
        self.section_calls += 1;
        self.put_u8(op as u8);
    }

    /// Notes that the current operation reached the outcome `class`.
    fn note(&mut self, class: &'static str) {
        self.seen.insert((self.op, class));
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.total.write(bytes);
        self.section.write(bytes);
    }

    fn put_u8(&mut self, v: u8) {
        self.bytes(&[v]);
    }

    fn put_bool(&mut self, v: bool) {
        self.put_u8(u8::from(v));
    }

    fn put_u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }

    fn put_i64(&mut self, v: i64) {
        self.bytes(&v.to_le_bytes());
    }

    fn put_i128(&mut self, v: i128) {
        self.bytes(&v.to_le_bytes());
    }

    fn put_f64(&mut self, v: f64) {
        if v.is_nan() {
            self.nans += 1;
            self.put_u64(CANONICAL_NAN);
        } else {
            self.put_u64(v.to_bits());
        }
    }

    fn sign(&mut self, s: Sign) {
        let (byte, class) = match s {
            Sign::Negative => (0, "Negative"),
            Sign::Zero => (1, "Zero"),
            Sign::Positive => (2, "Positive"),
        };
        self.put_u8(byte);
        self.note(class);
    }

    fn ordering(&mut self, o: Ordering) {
        let (byte, class) = match o {
            Ordering::Less => (0, "Less"),
            Ordering::Equal => (1, "Equal"),
            Ordering::Greater => (2, "Greater"),
        };
        self.put_u8(byte);
        self.note(class);
    }

    fn error(&mut self, e: NumError) {
        let (byte, class) = match e {
            NumError::OutOfRange { .. } => (1, "OutOfRange"),
            NumError::Degenerate => (2, "Degenerate"),
            NumError::NotFinite => (3, "NotFinite"),
            NumError::WrongDirection => (4, "WrongDirection"),
            NumError::DivisorContainsZero => (5, "DivisorContainsZero"),
            NumError::NegativeSqrt => (6, "NegativeSqrt"),
            NumError::EmptyInterval => (7, "EmptyInterval"),
        };
        self.put_u8(byte);
        self.note(class);
        if let NumError::OutOfRange { value } = e {
            self.put_i128(value);
        }
    }

    fn result<T>(&mut self, r: Result<T, NumError>, ok: impl FnOnce(&mut Recorder, T)) {
        match r {
            Ok(v) => {
                self.put_u8(0);
                self.note("Ok");
                ok(self, v);
            }
            Err(e) => self.error(e),
        }
    }

    fn option<T>(&mut self, o: Option<T>, some: impl FnOnce(&mut Recorder, T)) {
        match o {
            None => {
                self.put_u8(0);
                self.note("None");
            }
            Some(v) => {
                self.put_u8(1);
                self.note("Some");
                some(self, v);
            }
        }
    }

    fn wide<const L: usize>(&mut self, w: Wide<L>) {
        for limb in w.to_le_limbs() {
            self.put_u64(limb);
        }
    }

    fn point(&mut self, p: P3) {
        self.put_i64(p.x.get());
        self.put_i64(p.y.get());
        self.put_i64(p.z.get());
    }

    fn exact(&mut self, exact: bool) {
        self.put_bool(exact);
        self.note(if exact { "exact" } else { "inexact" });
    }

    fn rounded_point(&mut self, r: Rounded<P3>) {
        self.point(r.value);
        self.exact(r.exact);
    }

    fn rounded_coord(&mut self, r: Rounded<Coord>) {
        self.put_i64(r.value.get());
        self.exact(r.exact);
    }

    fn plane(&mut self, p: &Plane) {
        for n in p.n() {
            self.put_i128(n);
        }
        self.put_i128(p.d());
    }

    fn interval(&mut self, x: Interval) {
        self.put_f64(x.lo());
        self.put_f64(x.hi());
    }

    /// An error-free transformation's pair; notes whether it is exact or reports that no exact
    /// pair exists (a non-finite second part).
    fn eft(&mut self, (x, e): (f64, f64)) {
        self.put_f64(x);
        self.put_f64(e);
        self.note(if e.is_finite() { "exact" } else { "reported" });
    }
}

// ---------------------------------------------------------------------------------------------
// The corpus generator.
// ---------------------------------------------------------------------------------------------

/// A 64-bit linear congruential generator. Integer arithmetic only, so it gives the same sequence
/// on every target.
struct Gen(u64);

impl Gen {
    /// The next 32 bits: the high half of the next state (the low bits of a linear congruential
    /// generator modulo a power of two have short periods).
    fn step(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(LCG_MUL).wrapping_add(LCG_ADD);
        self.0 >> 32
    }

    /// 64 bits, from two steps.
    fn bits(&mut self) -> u64 {
        let hi = self.step();
        (hi << 32) | self.step()
    }

    /// A number in 0..n, for n > 0.
    fn below(&mut self, n: u64) -> u64 {
        self.bits() % n
    }

    /// True one time in `n`.
    fn one_in(&mut self, n: u64) -> bool {
        self.below(n) == 0
    }

    /// A number in lo..=hi.
    fn int(&mut self, lo: i64, hi: i64) -> i64 {
        let span = u64::try_from(hi - lo).unwrap() + 1;
        lo + i64::try_from(self.below(span)).unwrap()
    }

    /// One of `items`.
    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        let n = u64::try_from(items.len()).unwrap();
        items[usize::try_from(self.below(n)).unwrap()]
    }

    /// 2^k for k in 0..=max_log2.
    fn span(&mut self, max_log2: i64) -> i64 {
        1 << self.int(0, max_log2)
    }

    /// A coordinate scale for one call: small scales make exact zeros common, the largest fills
    /// the frame.
    fn scale(&mut self) -> i64 {
        self.pick(&[3, 1 << 10, 1 << 20, 1 << 30, 1 << 39, M])
    }

    /// A coordinate at or next to the frame's edges, or next to zero.
    fn extreme(&mut self) -> i64 {
        let k = self.int(0, 2);
        match self.below(5) {
            0 => M - k,
            1 => k - M,
            2 => k - 1,
            3 => M,
            _ => -M,
        }
    }

    fn p2(&mut self, s: i64) -> [i64; 2] {
        [self.int(-s, s), self.int(-s, s)]
    }

    fn p3(&mut self, s: i64) -> [i64; 3] {
        [self.int(-s, s), self.int(-s, s), self.int(-s, s)]
    }

    /// A point at a scale of its own.
    fn anywhere3(&mut self) -> [i64; 3] {
        let s = self.scale();
        self.p3(s)
    }

    fn e2(&mut self) -> [i64; 2] {
        [self.extreme(), self.extreme()]
    }

    fn e3(&mut self) -> [i64; 3] {
        [self.extreme(), self.extreme(), self.extreme()]
    }

    /// No move half the time; otherwise −1, 0 or 1 step on each axis.
    fn nudge2(&mut self) -> [i64; 2] {
        if self.one_in(2) { [0; 2] } else { self.p2(1) }
    }

    fn nudge3(&mut self) -> [i64; 3] {
        if self.one_in(2) { [0; 3] } else { self.p3(1) }
    }

    /// A random `i128` with a random number of significant bits.
    fn int128(&mut self) -> i128 {
        let raw = (u128::from(self.bits()) << 64) | u128::from(self.bits());
        let shift = u32::try_from(self.below(128)).unwrap();
        (raw as i128) >> shift
    }

    /// A random `i64` with a random number of significant bits.
    fn int64(&mut self) -> i64 {
        let shift = u32::try_from(self.below(64)).unwrap();
        (self.bits() as i64) >> shift
    }

    /// The bits of a positive double whose leading bit has exponent e, for e uniform in lo..=hi
    /// (−1074 ≤ lo ≤ hi ≤ 1023), subnormals included; the bits below the leading one are random.
    fn magnitude(&mut self, lo: i64, hi: i64) -> u64 {
        let e = self.int(lo, hi);
        let fraction = self.bits() >> 12;
        if e >= -1022 {
            (u64::try_from(e + 1023).unwrap() << 52) | fraction
        } else {
            // Subnormal: the value is the fraction times 2^−1074, its leading bit at e + 1074.
            let lead = u32::try_from(e + 1074).unwrap();
            (1 << lead) | (fraction & ((1 << lead) - 1))
        }
    }

    /// A positive double, as `magnitude`.
    fn positive(&mut self, lo: i64, hi: i64) -> f64 {
        f64::from_bits(self.magnitude(lo, hi))
    }

    /// A double of either sign, as `magnitude`.
    fn double(&mut self, lo: i64, hi: i64) -> f64 {
        let sign = self.bits() & SIGN_BIT;
        f64::from_bits(sign | self.magnitude(lo, hi))
    }

    /// Any finite double: a special value, a moderate one or one of any exponent.
    fn any(&mut self) -> f64 {
        match self.below(4) {
            0 => self.pick(&SPECIAL),
            1 => self.double(-30, 30),
            _ => self.double(-1074, 1023),
        }
    }

    /// As `any`, or a zero of either sign one time in eight.
    fn any_or_zero(&mut self) -> f64 {
        if self.one_in(8) {
            self.pick(&[0.0, -0.0])
        } else {
            self.any()
        }
    }
}

/// `v` moved into the frame.
fn clamp(v: i64) -> i64 {
    v.clamp(-M, M)
}

fn pt2([x, y]: [i64; 2]) -> P2 {
    P2::new(clamp(x), clamp(y)).unwrap()
}

fn pt3([x, y, z]: [i64; 3]) -> P3 {
    P3::new(clamp(x), clamp(y), clamp(z)).unwrap()
}

/// `base` plus each multiple k · v of `terms`, axis by axis. Callers keep every value far below
/// 2^63, so nothing overflows; the result may leave the frame and is clamped when made a point.
fn along<const N: usize>(base: [i64; N], terms: &[(i64, [i64; N])]) -> [i64; N] {
    let mut out = base;
    for &(k, v) in terms {
        for (o, x) in out.iter_mut().zip(v) {
            *o += k * x;
        }
    }
    out
}

/// The 30 grid points at distance 3 from the origin: the permutations of (±1, ±2, ±2) and of
/// (±3, 0, 0).
fn sphere3() -> Vec<[i64; 3]> {
    let mut points = Vec::new();
    for axis in [0, 1, 2] {
        for signs in 0..8 {
            let sign = |bit: u32| if (signs >> bit) & 1 == 1 { -1 } else { 1 };
            let mut p = [2 * sign(0), 2 * sign(1), 2 * sign(2)];
            p[axis] /= 2;
            points.push(p);
        }
        for s in [-3, 3] {
            let mut p = [0; 3];
            p[axis] = s;
            points.push(p);
        }
    }
    points
}

// ---------------------------------------------------------------------------------------------
// The exact predicates.
// ---------------------------------------------------------------------------------------------

/// Three points for `orient2d`: random at one scale, collinear with or without a one-step nudge,
/// at the extremes, or with a repeated point.
fn triangle(g: &mut Gen) -> [[i64; 2]; 3] {
    match g.below(4) {
        0 => {
            let s = g.scale();
            [g.p2(s), g.p2(s), g.p2(s)]
        }
        1 => {
            let a = g.p2(M / 2);
            let s = g.span(37);
            let v = g.p2(s);
            let (i, j) = (g.int(-4, 4), g.int(-4, 4));
            let n = g.nudge2();
            [a, along(a, &[(i, v)]), along(a, &[(j, v), (1, n)])]
        }
        2 => [g.e2(), g.e2(), g.e2()],
        _ => {
            let s = g.scale();
            let (a, b) = (g.p2(s), g.p2(s));
            [a, b, g.pick(&[a, b])]
        }
    }
}

/// Four points for `orient3d`: random at one scale, coplanar with or without a nudge, at the
/// extremes, or with a repeated point.
fn tetrahedron(g: &mut Gen) -> [[i64; 3]; 4] {
    match g.below(4) {
        0 => {
            let s = g.scale();
            [g.p3(s), g.p3(s), g.p3(s), g.p3(s)]
        }
        1 => {
            let a = g.p3(M / 4);
            let (su, sv) = (g.span(36), g.span(36));
            let (u, v) = (g.p3(su), g.p3(sv));
            let (i, j) = (g.int(-3, 3), g.int(-3, 3));
            let n = g.nudge3();
            let d = along(a, &[(i, u), (j, v), (1, n)]);
            [a, along(a, &[(1, u)]), along(a, &[(1, v)]), d]
        }
        2 => [g.e3(), g.e3(), g.e3(), g.e3()],
        _ => {
            let s = g.scale();
            let (a, b, c) = (g.p3(s), g.p3(s), g.p3(s));
            [a, b, c, g.pick(&[a, b, c])]
        }
    }
}

/// Four points for `incircle`: random at one scale, cocircular (grid points at distance 5k from
/// a centre) with or without a nudge, at the extremes, or three collinear.
fn circle_quad(g: &mut Gen) -> [[i64; 2]; 4] {
    match g.below(4) {
        0 => {
            let s = g.scale();
            [g.p2(s), g.p2(s), g.p2(s), g.p2(s)]
        }
        1 => {
            let centre = g.p2(M / 4);
            let span = g.span(36);
            let k = g.int(1, span);
            let mut quad = [[0; 2]; 4];
            for p in &mut quad {
                let on = g.pick(&CIRCLE5);
                *p = along(centre, &[(k, on)]);
            }
            let n = g.nudge2();
            quad[3] = along(quad[3], &[(1, n)]);
            quad
        }
        2 => [g.e2(), g.e2(), g.e2(), g.e2()],
        _ => {
            let s = g.scale();
            let (a, v, d) = (g.p2(s), g.p2(s), g.p2(s));
            [a, along(a, &[(1, v)]), along(a, &[(2, v)]), d]
        }
    }
}

/// Five points for `insphere`: random at one scale, cospherical (grid points at distance 3k from
/// a centre) with or without a nudge, at the extremes, or four coplanar.
fn sphere_quint(g: &mut Gen, sphere: &[[i64; 3]]) -> [[i64; 3]; 5] {
    match g.below(4) {
        0 => {
            let s = g.scale();
            [g.p3(s), g.p3(s), g.p3(s), g.p3(s), g.p3(s)]
        }
        1 => {
            let centre = g.p3(M / 4);
            let span = g.span(36);
            let k = g.int(1, span);
            let mut quint = [[0; 3]; 5];
            for p in &mut quint {
                let on = g.pick(sphere);
                *p = along(centre, &[(k, on)]);
            }
            let n = g.nudge3();
            quint[4] = along(quint[4], &[(1, n)]);
            quint
        }
        2 => [g.e3(), g.e3(), g.e3(), g.e3(), g.e3()],
        _ => {
            let [a, b, c, d] = tetrahedron(g);
            [a, b, c, d, g.anywhere3()]
        }
    }
}

fn predicates(g: &mut Gen, r: &mut Recorder) {
    let sphere = sphere3();
    for _ in 0..3_000 {
        let [a, b, c] = triangle(g);
        r.call(Op::Orient2d);
        r.sign(orient2d(pt2(a), pt2(b), pt2(c)));

        let [a, b, c, d] = tetrahedron(g);
        r.call(Op::Orient3d);
        r.sign(orient3d(pt3(a), pt3(b), pt3(c), pt3(d)));

        let [a, b, c, d] = circle_quad(g);
        r.call(Op::Incircle);
        r.sign(incircle(pt2(a), pt2(b), pt2(c), pt2(d)));

        let [a, b, c, d, e] = sphere_quint(g, &sphere);
        r.call(Op::Insphere);
        r.sign(insphere(pt3(a), pt3(b), pt3(c), pt3(d), pt3(e)));
    }
}

// ---------------------------------------------------------------------------------------------
// Planes and three-plane points.
// ---------------------------------------------------------------------------------------------

/// Records `Plane::through` the three points; returns the plane when there is one.
fn through(r: &mut Recorder, [a, b, c]: [[i64; 3]; 3]) -> Option<Plane> {
    let plane = Plane::through(pt3(a), pt3(b), pt3(c));
    r.call(Op::PlaneThrough);
    r.result(plane, |r, p| r.plane(&p));
    plane.ok()
}

/// Records `side_of_tpi`.
fn classify(r: &mut Recorder, plane: &Plane, t: &Tpi) {
    r.call(Op::SideOfTpi);
    r.sign(plane.side_of_tpi(t));
}

/// Three points for a plane: random at one scale, collinear with or without a nudge, at the
/// extremes, or with a repeated point.
fn plane_points(g: &mut Gen) -> [[i64; 3]; 3] {
    match g.below(5) {
        0 | 1 => {
            let s = g.scale();
            [g.p3(s), g.p3(s), g.p3(s)]
        }
        2 => {
            let a = g.p3(M / 2);
            let s = g.span(37);
            let v = g.p3(s);
            let (i, j) = (g.int(-4, 4), g.int(-4, 4));
            let n = g.nudge3();
            [a, along(a, &[(i, v)]), along(a, &[(j, v), (1, n)])]
        }
        3 => [g.e3(), g.e3(), g.e3()],
        _ => {
            let s = g.scale();
            let (a, b) = (g.p3(s), g.p3(s));
            [a, b, g.pick(&[a, b])]
        }
    }
}

/// A point to test against the plane through `a`, `b`, `c`: one of them, a small combination of
/// them with or without a nudge (in the plane when it stays inside the frame), random, or extreme.
fn side_point(g: &mut Gen, [a, b, c]: [[i64; 3]; 3]) -> [i64; 3] {
    match g.below(4) {
        0 => g.pick(&[a, b, c]),
        1 => {
            let (i, j) = (g.int(-3, 3), g.int(-3, 3));
            let n = g.nudge3();
            let u = along(b, &[(-1, a)]);
            let v = along(c, &[(-1, a)]);
            along(a, &[(i, u), (j, v), (1, n)])
        }
        2 => g.anywhere3(),
        _ => g.e3(),
    }
}

fn planes(g: &mut Gen, r: &mut Recorder) {
    for _ in 0..2_000 {
        let abc = plane_points(g);
        let Some(plane) = through(r, abc) else {
            continue;
        };
        for _ in 0..2 {
            let p = side_point(g, abc);
            r.call(Op::PlaneSide);
            r.sign(plane.side(pt3(p)));
        }
    }
}

/// Three planes, each by three points.
fn tpi_planes(g: &mut Gen) -> [[[i64; 3]; 3]; 3] {
    match g.below(6) {
        0 => {
            // Three planes through one grid point: they meet exactly there.
            let x = g.p3(M / 2);
            let mut planes = [[x; 3]; 3];
            for plane in &mut planes {
                let (su, sv) = (g.span(38), g.span(38));
                let (u, v) = (g.p3(su), g.p3(sv));
                *plane = [x, along(x, &[(1, u)]), along(x, &[(1, v)])];
            }
            planes
        }
        1 => {
            // Each plane through points up to three steps from one point: they meet near it,
            // usually off the grid; nearly parallel planes meet far away, often outside the frame.
            let x = g.p3(M / 2);
            let mut planes = [[x; 3]; 3];
            for plane in &mut planes {
                let e = g.p3(3);
                let o = along(x, &[(1, e)]);
                let (su, sv) = (g.span(38), g.span(38));
                let (u, v) = (g.p3(su), g.p3(sv));
                *plane = [o, along(o, &[(1, u)]), along(o, &[(1, v)])];
            }
            planes
        }
        2 => {
            let s = g.scale();
            let mut planes = [[[0; 3]; 3]; 3];
            for point in planes.iter_mut().flatten() {
                *point = g.p3(s);
            }
            planes
        }
        3 => {
            // No single meeting point: a parallel copy, a pencil through one line, or a repeat.
            let s = g.scale();
            let (a, b, c) = (g.p3(s), g.p3(s), g.p3(s));
            let third = [g.p3(s), g.p3(s), g.p3(s)];
            match g.below(3) {
                0 => {
                    let t = g.p3(s);
                    let shifted = [a, b, c].map(|p| along(p, &[(1, t)]));
                    [[a, b, c], shifted, third]
                }
                1 => [[a, b, c], [a, b, third[0]], [a, b, third[1]]],
                _ => [[a, b, c], [b, c, a], third],
            }
        }
        4 => {
            // x + y = s, x − y = 0 and z = c meet at (s/2, s/2, c): a tie on two axes when s is
            // odd, which the commit rounds half to even. Mirrored, moved and in any order.
            let s = g.int(0, M / 4);
            let c = g.int(-M / 4, M / 4);
            let planes = [
                [[s, 0, 0], [0, s, 0], [s, 0, 1]],
                [[0, 0, 0], [1, 1, 0], [0, 0, 1]],
                [[0, 0, c], [1, 0, c], [0, 1, c]],
            ];
            let (mx, my) = (g.pick(&[-1, 1]), g.pick(&[-1, 1]));
            let t = g.p3(M / 4);
            let place = |p: [i64; 3]| [t[0] + mx * p[0], t[1] + my * p[1], t[2] + p[2]];
            let order = g.pick(&PERMUTATIONS);
            order.map(|k| planes[k].map(place))
        }
        _ => {
            let mut planes = [[[0; 3]; 3]; 3];
            for point in planes.iter_mut().flatten() {
                *point = g.e3();
            }
            planes
        }
    }
}

fn three_plane_points(g: &mut Gen, r: &mut Recorder) {
    for _ in 0..1_200 {
        let [pp, qp, sp] = tpi_planes(g);
        let (Some(p), Some(q), Some(s)) = (through(r, pp), through(r, qp), through(r, sp)) else {
            continue;
        };
        let t = Tpi::new(p, q, s);
        r.call(Op::TpiNew);
        r.result(t, |_, _| {});
        let Ok(t) = t else { continue };

        let (x, w) = t.homogeneous();
        r.call(Op::TpiHomogeneous);
        for xi in x {
            r.wide(xi);
        }
        r.wide(w);

        let committed = t.to_grid();
        r.call(Op::ToGrid);
        r.result(committed, |r, c| r.rounded_point(c));

        // Against one of its own planes (always zero), a plane through random points, and a
        // plane through the committed grid point.
        let own = g.pick(&[p, q, s]);
        classify(r, &own, &t);
        let random = [g.anywhere3(), g.anywhere3(), g.anywhere3()];
        if let Some(other) = through(r, random) {
            classify(r, &other, &t);
        }
        if let Ok(c) = committed {
            let c = [c.value.x.get(), c.value.y.get(), c.value.z.get()];
            let (su, sv) = (g.span(38), g.span(38));
            let (u, v) = (g.p3(su), g.p3(sv));
            if let Some(other) = through(r, [c, along(c, &[(1, u)]), along(c, &[(1, v)])]) {
                classify(r, &other, &t);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Grid steps.
// ---------------------------------------------------------------------------------------------

/// A coordinate to move between grid steps: anywhere, small enough to refine twice, near a
/// multiple of 25 or 625 (exact, or near halfway), at the extremes, or small.
fn grid_coord(g: &mut Gen) -> Coord {
    let v = match g.below(5) {
        0 => g.int(-M, M),
        1 => g.int(-M / 625, M / 625),
        2 => {
            let unit = g.pick(&[25, 625]);
            let k = g.int(-M / 625, M / 625);
            let off = g.pick(&[-313, -312, -13, -12, -1, 0, 1, 12, 13, 312, 313]);
            k * unit + off
        }
        3 => g.extreme(),
        _ => g.int(-50, 50),
    };
    Coord::new(clamp(v)).unwrap()
}

fn grid_steps(g: &mut Gen, r: &mut Recorder) {
    for _ in 0..2_000 {
        let (c, from, to) = (grid_coord(g), g.pick(&STEPS), g.pick(&STEPS));
        r.call(Op::Refine);
        r.result(refine(c, from, to), |r, v| r.put_i64(v.get()));

        let (c, from, to) = (grid_coord(g), g.pick(&STEPS), g.pick(&STEPS));
        r.call(Op::Coarsen);
        r.result(coarsen(c, from, to), |r, v| r.rounded_coord(v));
    }
}

// ---------------------------------------------------------------------------------------------
// Wide integers.
// ---------------------------------------------------------------------------------------------

/// Two's-complement negation of raw limbs, in place.
fn negate<const L: usize>(limbs: &mut [u64; L]) {
    let mut carry = true;
    for limb in limbs.iter_mut() {
        let (v, c) = (!*limb).overflowing_add(u64::from(carry));
        *limb = v;
        carry = c;
    }
}

/// An edge of the range of `Wide<L>`: 0, 1, −1, the largest and smallest values and their
/// neighbours, ±2^(64L − 2), or 2^64.
fn wide_edge<const L: usize>(g: &mut Gen) -> Wide<L> {
    let mut limbs = [0u64; L];
    let top = L - 1;
    match g.below(10) {
        0 => {}
        1 => limbs[0] = 1,
        2 => limbs = [u64::MAX; L],
        3 => {
            limbs = [u64::MAX; L];
            limbs[top] = u64::MAX >> 1;
        }
        4 => limbs[top] = 1 << 63,
        5 => {
            limbs = [u64::MAX; L];
            limbs[top] = u64::MAX >> 1;
            limbs[0] = u64::MAX - 1;
        }
        6 => {
            limbs[top] = 1 << 63;
            limbs[0] = 1;
        }
        7 => limbs[top] = 1 << 62,
        8 => limbs[top] = 3 << 62,
        _ => limbs[1] = 1,
    }
    Wide::from_le_limbs(limbs)
}

/// A `Wide<L>`: an edge of the range one time in eight; otherwise a magnitude of random length
/// (often just below the top, often within `i128`) and a random sign.
fn wide_value<const L: usize>(g: &mut Gen) -> Wide<L> {
    if g.one_in(8) {
        return wide_edge::<L>(g);
    }
    let top = 64 * u64::try_from(L).unwrap() - 1;
    let bits = match g.below(4) {
        0 => top - g.below(3),
        1 => g.below(129),
        _ => g.below(top + 1),
    };
    let mut limbs = [0u64; L];
    for (low, limb) in (0u64..).step_by(64).zip(limbs.iter_mut()) {
        *limb = if bits >= low + 64 {
            g.bits()
        } else if bits > low {
            g.bits() >> (64 - (bits - low))
        } else {
            0
        };
    }
    if g.one_in(2) {
        negate(&mut limbs);
    }
    Wide::from_le_limbs(limbs)
}

fn wide_ops<const L: usize>(g: &mut Gen, r: &mut Recorder) {
    let constants = [
        Wide::<L>::ZERO,
        Wide::one(),
        Wide::max_value(),
        Wide::min_value(),
    ];
    for w in constants {
        r.call(Op::WideConst);
        r.wide(w);
    }
    for _ in 0..300 {
        let a = wide_value::<L>(g);
        let b = if g.one_in(8) { a } else { wide_value::<L>(g) };
        r.call(Op::WideAdd);
        r.option(a.checked_add(&b), |r, v| r.wide(v));
        r.call(Op::WideSub);
        r.option(a.checked_sub(&b), |r, v| r.wide(v));
        r.call(Op::WideMul);
        r.option(a.checked_mul(&b), |r, v| r.wide(v));
        r.call(Op::WideNeg);
        r.option(a.checked_neg(), |r, v| r.wide(v));
        r.call(Op::WideCmp);
        r.ordering(a.cmp(&b));
        r.call(Op::WideSignum);
        r.sign(a.signum());
        r.call(Op::WideResize);
        r.option(a.resize::<4>(), |r, v| r.wide(v));
        r.call(Op::WideResize);
        r.option(a.resize::<5>(), |r, v| r.wide(v));
        r.call(Op::WideResize);
        r.option(a.resize::<6>(), |r, v| r.wide(v));
        r.call(Op::WideResize);
        r.option(a.resize::<8>(), |r, v| r.wide(v));
        r.call(Op::WideToI128);
        r.option(a.to_i128(), |r, v| r.put_i128(v));
        let (v128, v64) = (g.int128(), g.int64());
        r.call(Op::WideFromInt);
        r.wide(Wide::<L>::from_i128(v128));
        r.call(Op::WideFromInt);
        r.wide(Wide::<L>::from_i64(v64));
    }
}

fn wide_integers(g: &mut Gen, r: &mut Recorder) {
    wide_ops::<4>(g, r);
    wide_ops::<5>(g, r);
    wide_ops::<6>(g, r);
    wide_ops::<8>(g, r);
}

// ---------------------------------------------------------------------------------------------
// Error-free transformations.
// ---------------------------------------------------------------------------------------------

/// Two doubles for the error-free transformations: any exponents; nearly cancelling; grid
/// coordinates; at the overflow edge; tiny and subnormal; at the edges of `two_prod`'s direct
/// range; or special values.
fn eft_pair(g: &mut Gen) -> (f64, f64) {
    match g.below(7) {
        0 => (g.double(-1074, 1023), g.double(-1074, 1023)),
        1 => {
            // b is −a with up to 24 of its lowest bits flipped.
            let a = g.double(-1000, 1000);
            let shift = g.int(40, 63);
            let flip = g.bits() >> shift;
            (a, f64::from_bits((a.to_bits() ^ flip) ^ SIGN_BIT))
        }
        2 => (g.int(-M, M) as f64, g.int(-M, M) as f64),
        3 => {
            let sign = g.bits() & SIGN_BIT;
            let a = f64::from_bits(sign | g.magnitude(1020, 1023));
            let b = f64::from_bits(sign | g.magnitude(1015, 1023));
            (a, b)
        }
        4 => (g.double(-1074, -1000), g.double(-1074, -900)),
        5 => {
            let ea = g.int(-1022, 1023);
            let sum = if g.one_in(2) {
                g.int(-975, -965)
            } else {
                g.int(1018, 1023)
            };
            let eb = (sum - ea).clamp(-1074, 1023);
            (g.double(ea, ea), g.double(eb, eb))
        }
        _ => (g.pick(&SPECIAL), g.pick(&SPECIAL)),
    }
}

fn error_free(g: &mut Gen, r: &mut Recorder) {
    for _ in 0..1_500 {
        let (a, b) = eft_pair(g);
        r.call(Op::TwoSum);
        r.eft(two_sum(a, b));
        r.call(Op::FastTwoSum);
        r.eft(fast_two_sum(a, b));
        r.call(Op::TwoProd);
        r.eft(two_prod(a, b));
        r.call(Op::Split);
        r.eft(split(a));
    }
}

// ---------------------------------------------------------------------------------------------
// Intervals.
// ---------------------------------------------------------------------------------------------

fn sorted(a: f64, b: f64) -> (f64, f64) {
    if b < a { (b, a) } else { (a, b) }
}

/// Records `Interval::new` on bounds that are moderate, a point, around zero, huge (sums and
/// products overflow), tiny and subnormal, special, unordered (an empty interval about half the
/// time) or now and then not finite; returns the interval, or [1, 1] when there is none.
fn interval_input(g: &mut Gen, r: &mut Recorder) -> Interval {
    let (lo, hi) = match g.below(8) {
        0 | 1 => sorted(g.double(-60, 60), g.double(-60, 60)),
        2 => {
            let x = g.double(-60, 60);
            (x, x)
        }
        3 => (-g.positive(-60, 60), g.positive(-60, 60)),
        4 => sorted(g.double(1022, 1023), g.double(1022, 1023)),
        5 => sorted(g.double(-1074, -1000), g.double(-1074, -1000)),
        6 => {
            let x = g.pick(&SPECIAL);
            if g.one_in(2) {
                (x, x)
            } else {
                sorted(x, g.pick(&SPECIAL))
            }
        }
        _ => (g.double(-60, 60), g.double(-60, 60)),
    };
    let (lo, hi) = if g.one_in(64) {
        (lo, g.pick(&[f64::INFINITY, f64::NAN]))
    } else {
        (lo, hi)
    };
    let made = Interval::new(lo, hi);
    r.call(Op::IntervalNew);
    r.result(made, |r, x| r.interval(x));
    made.unwrap_or_else(|_| Interval::point(1.0).unwrap())
}

/// A double to test for membership: a bound, the double just outside a bound, or random.
fn probe(g: &mut Gen, x: Interval) -> f64 {
    match g.below(5) {
        0 => x.lo(),
        1 => x.hi(),
        2 => x.lo().next_down(),
        3 => x.hi().next_up(),
        _ => g.double(-60, 60),
    }
}

fn intervals(g: &mut Gen, r: &mut Recorder) {
    for _ in 0..1_200 {
        let x = interval_input(g, r);
        let y = interval_input(g, r);
        r.call(Op::IntervalAdd);
        r.result(x.add(y), |r, v| r.interval(v));
        r.call(Op::IntervalSub);
        r.result(x.sub(y), |r, v| r.interval(v));
        r.call(Op::IntervalMul);
        r.result(x.mul(y), |r, v| r.interval(v));
        r.call(Op::IntervalDiv);
        r.result(x.div(y), |r, v| r.interval(v));
        r.call(Op::IntervalSquare);
        r.result(x.square(), |r, v| r.interval(v));
        r.call(Op::IntervalSqrt);
        r.result(x.sqrt(), |r, v| r.interval(v));
        r.call(Op::IntervalSqrtNonneg);
        r.result(x.sqrt_nonneg(), |r, v| r.interval(v));
        r.call(Op::IntervalNeg);
        r.interval(x.neg());

        let p = probe(g, x);
        let inside = x.contains(p);
        r.call(Op::Contains);
        r.put_bool(inside);
        r.note(if inside { "true" } else { "false" });

        r.call(Op::CertainSign);
        r.option(x.certain_sign(), |r, s| r.sign(s));
        if let Ok(z) = x.sub(y) {
            r.call(Op::CertainSign);
            r.option(z.certain_sign(), |r, s| r.sign(s));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Maths functions.
// ---------------------------------------------------------------------------------------------

/// What a maths function is fed.
#[derive(Clone, Copy)]
enum Domain {
    /// Any finite double.
    Any,
    /// Not negative, −0 included.
    NonNegative,
    /// Positive (subnormals and exact powers of two and ten included), and zero now and then.
    Positive,
    /// [−1, 1].
    Unit,
    /// Angles in radians: small, huge (long argument reduction), tiny and special.
    Angle,
    /// Arguments of `exp`, past both its overflow and its underflow.
    Exponent,
    /// Values to round: exact halves, fractions, large whole numbers and specials.
    Rounding,
    /// Angles in half-turns: exact quarter turns, small, huge, tiny and special.
    HalfTurns,
}

fn math_input(g: &mut Gen, domain: Domain) -> f64 {
    match domain {
        Domain::Any => g.any(),
        Domain::NonNegative => {
            if g.one_in(16) {
                -0.0
            } else {
                f64::from_bits(g.any().to_bits() & !SIGN_BIT)
            }
        }
        Domain::Positive => match g.below(8) {
            0 => 0.0,
            1 => f64::from_bits(u64::try_from(g.int(1, 2046)).unwrap() << 52),
            2 => 10u64.pow(u32::try_from(g.below(20)).unwrap()) as f64,
            3 | 4 => g.positive(-30, 30),
            _ => g.positive(-1074, 1023),
        },
        Domain::Unit => match g.below(4) {
            0 => g.pick(&[1.0, -1.0, 0.0, -0.0, 0.5, -0.5]),
            _ => g.double(-60, -1),
        },
        Domain::Angle => match g.below(8) {
            0 => g.pick(&SPECIAL),
            1 => g.double(-1074, -30),
            2 | 3 => g.double(3, 1023),
            _ => g.double(-30, 3),
        },
        Domain::Exponent => match g.below(4) {
            0 => g.pick(&SPECIAL),
            _ => g.double(-40, 9),
        },
        Domain::Rounding => match g.below(4) {
            0 => g.pick(&SPECIAL),
            1 => (2 * g.int(-(1 << 20), 1 << 20) + 1) as f64 * 0.5,
            _ => g.double(-5, 60),
        },
        Domain::HalfTurns => match g.below(8) {
            0 => g.pick(&SPECIAL),
            1 => g.double(-1074, -30),
            2 | 3 => g.double(3, 1023),
            4 | 5 => g.int(-(1 << 30), 1 << 30) as f64 * 0.25,
            _ => g.double(-30, 3),
        },
    }
}

/// Arguments of `pow`: a positive base with any moderate power, a negative base with a whole
/// power, a base of 0, −0 or 1, or powers that overflow and underflow.
fn pow_input(g: &mut Gen) -> (f64, f64) {
    match g.below(5) {
        0 | 1 => (g.positive(-20, 20), g.double(-6, 6)),
        2 => (-g.positive(-10, 10), g.int(-60, 60) as f64),
        3 => (g.pick(&[0.0, -0.0, 1.0]), g.double(-10, 10)),
        _ => (g.positive(-30, 30), g.double(5, 12)),
    }
}

/// Records one maths call: the function's index, then its result.
fn math_call(r: &mut Recorder, index: u8, v: f64) {
    r.call(Op::Math);
    r.put_u8(index);
    r.put_f64(v);
}

fn maths(g: &mut Gen, r: &mut Recorder) {
    let unary: [(fn(f64) -> f64, Domain); 18] = [
        (math::sqrt, Domain::NonNegative),
        (math::sin, Domain::Angle),
        (math::cos, Domain::Angle),
        (math::tan, Domain::Angle),
        (math::asin, Domain::Unit),
        (math::acos, Domain::Unit),
        (math::atan, Domain::Any),
        (math::exp, Domain::Exponent),
        (math::ln, Domain::Positive),
        (math::log2, Domain::Positive),
        (math::log10, Domain::Positive),
        (math::cbrt, Domain::Any),
        (math::abs, Domain::Any),
        (math::floor, Domain::Rounding),
        (math::ceil, Domain::Rounding),
        (math::round_ties_even, Domain::Rounding),
        (math::sinpi, Domain::HalfTurns),
        (math::cospi, Domain::HalfTurns),
    ];
    for (index, (f, domain)) in (0u8..).zip(unary) {
        for _ in 0..300 {
            let x = math_input(g, domain);
            math_call(r, index, f(x));
        }
    }
    for _ in 0..300 {
        let (y, x) = (g.any_or_zero(), g.any_or_zero());
        math_call(r, 18, math::atan2(y, x));
    }
    for _ in 0..300 {
        let (x, y) = pow_input(g);
        math_call(r, 19, math::pow(x, y));
    }
    for _ in 0..300 {
        let (x, y) = if g.one_in(2) {
            (g.any_or_zero(), g.any_or_zero())
        } else {
            let e = g.int(-1074, 1023);
            (g.double(e, e), g.double(e, e))
        };
        math_call(r, 20, math::hypot(x, y));
    }
}

// ---------------------------------------------------------------------------------------------
// The tests.
// ---------------------------------------------------------------------------------------------

/// The outcomes the corpus is built to reach. If a change to the corpus or the kernel stops it
/// reaching one, the hash would no longer cover that case, so the test fails instead.
fn required() -> Vec<(Op, &'static str)> {
    let mut out = Vec::new();
    let signed = [
        Op::Orient2d,
        Op::Orient3d,
        Op::Incircle,
        Op::Insphere,
        Op::PlaneSide,
        Op::SideOfTpi,
        Op::WideSignum,
        Op::CertainSign,
    ];
    for op in signed {
        for class in ["Negative", "Zero", "Positive"] {
            out.push((op, class));
        }
    }
    let partial = [
        Op::WideAdd,
        Op::WideSub,
        Op::WideMul,
        Op::WideNeg,
        Op::WideResize,
        Op::WideToI128,
    ];
    for op in partial {
        out.push((op, "Some"));
        out.push((op, "None"));
    }
    for op in [Op::TwoSum, Op::FastTwoSum, Op::TwoProd, Op::Split] {
        out.push((op, "exact"));
        out.push((op, "reported"));
    }
    out.extend([
        (Op::PlaneThrough, "Ok"),
        (Op::PlaneThrough, "Degenerate"),
        (Op::TpiNew, "Ok"),
        (Op::TpiNew, "Degenerate"),
        (Op::ToGrid, "exact"),
        (Op::ToGrid, "inexact"),
        (Op::ToGrid, "OutOfRange"),
        (Op::Refine, "Ok"),
        (Op::Refine, "OutOfRange"),
        (Op::Refine, "WrongDirection"),
        (Op::Coarsen, "exact"),
        (Op::Coarsen, "inexact"),
        (Op::Coarsen, "WrongDirection"),
        (Op::WideCmp, "Less"),
        (Op::WideCmp, "Equal"),
        (Op::WideCmp, "Greater"),
        (Op::IntervalNew, "Ok"),
        (Op::IntervalNew, "NotFinite"),
        (Op::IntervalNew, "EmptyInterval"),
        (Op::IntervalAdd, "NotFinite"),
        (Op::IntervalSub, "NotFinite"),
        (Op::IntervalMul, "NotFinite"),
        (Op::IntervalDiv, "Ok"),
        (Op::IntervalDiv, "DivisorContainsZero"),
        (Op::IntervalDiv, "NotFinite"),
        (Op::IntervalSquare, "NotFinite"),
        (Op::IntervalSqrt, "Ok"),
        (Op::IntervalSqrt, "NegativeSqrt"),
        (Op::IntervalSqrtNonneg, "Ok"),
        (Op::IntervalSqrtNonneg, "NegativeSqrt"),
        (Op::Contains, "true"),
        (Op::Contains, "false"),
        (Op::CertainSign, "None"),
    ]);
    out
}

#[test]
fn fnv1a_64_matches_its_published_test_vectors() {
    let hash = |s: &str| {
        let mut h = Fnv::new();
        h.write(s.as_bytes());
        h.0
    };
    assert_eq!(hash(""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(hash("a"), 0xaf63_dc4c_8601_ec8c);
    assert_eq!(hash("foobar"), 0x8594_4171_f739_67e8);
}

/// The golden hash. The same constant must hold on every target; a change is a bug or a
/// deliberate kernel edition change (see the module documentation and `GOLDEN`).
#[test]
fn golden_hash_is_the_same_on_every_target() {
    let mut g = Gen(SEED);
    let mut r = Recorder::new();
    predicates(&mut g, &mut r);
    r.end_section("predicates");
    planes(&mut g, &mut r);
    r.end_section("planes");
    three_plane_points(&mut g, &mut r);
    r.end_section("tpi");
    grid_steps(&mut g, &mut r);
    r.end_section("grid");
    wide_integers(&mut g, &mut r);
    r.end_section("wide");
    error_free(&mut g, &mut r);
    r.end_section("eft");
    intervals(&mut g, &mut r);
    r.end_section("interval");
    maths(&mut g, &mut r);
    r.end_section("math");

    let hash = r.total.0;
    println!("geom-num determinism corpus: {} calls", r.calls);
    for (name, calls, section_hash) in &r.report {
        println!("  {name:<10} {calls:>6} calls  {section_hash:#018x}");
    }
    println!("geom-num golden hash: {hash:#018x}");

    // The corpus must still reach every outcome it is built for, make enough calls, and stay
    // inside the functions' domains (a NaN means a domain here or the kernel changed).
    let missing: Vec<_> = required()
        .into_iter()
        .filter(|k| !r.seen.contains(k))
        .collect();
    assert!(missing.is_empty(), "unreached outcomes: {missing:?}");
    assert!(r.calls >= MIN_CALLS, "only {} calls", r.calls);
    assert_eq!(r.nans, 0, "a result in the corpus is NaN");
    assert_eq!(
        hash, GOLDEN,
        "geom-num's golden hash is {hash:#018x} here but {GOLDEN:#018x} is expected. Either this \
         target computes different bits (a bug: compare the section hashes above with another \
         target's log) or the kernel's results changed (a deliberate kernel edition change must \
         update GOLDEN and say so)"
    );
}
