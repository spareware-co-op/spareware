//! The golden hash: a fixed corpus of calls into every module of `geom-num`, hashed with SHA-256,
//! must give the same digest on every target.
//!
//! The kernel promises the same bits on every machine (chapter 21): one model, one geometry, on
//! Windows, Linux, macOS and in the browser. This test holds the numbers layer to that promise. It
//! makes a fixed corpus of 88,218 calls (the test prints the count per section, and fails, naming
//! the section, if one shrinks):
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
//! - the error-free transformations, on doubles of every exponent, with subnormals, the overflow
//!   edge, the special values and fixed pairs for their rare routes; and every interval operation,
//!   the length `norm` and the refusals of the enclosures that are not certified (`sin` and
//!   `cos`);
//! - every function in `geom_num::math` and `geom_num::math::unproven`, over its domain, and
//!   outside it, at its poles, past its overflow and with arguments that are not finite, each of
//!   which must be reported; and the interval versions of the proven ones (`Interval::sinpi`,
//!   `cospi`, `exp`, `ln` and `atan2`), inside and outside their domains;
//! - the scalars and small types: `Real` and its total order (both orders of ±0 included),
//!   `Coord` and `P3` at and beyond the frame's edges, `Sign`, `GridStep` and its units, `Steps`,
//!   `Exactness`, the ladder's constants and `Wide`'s sign tests.
//!
//! The corpus comes from a 64-bit linear congruential generator, written here, from a fixed seed.
//! It uses integer arithmetic and exact conversions only, so the inputs are the same on every
//! target by construction and only the kernel's results can differ.
//!
//! Every result is written in a canonical little-endian byte encoding, in a fixed field order (see
//! `Recorder`), and hashed with SHA-256 (FIPS 180-4), the hash chapters 21 and 28.13 specify, from
//! `geom_test::golden`, the one implementation every golden uses (piece G19), checked there against
//! the standard's test vectors. Doubles go in as their bits, so a difference of one unit in the
//! last place shows. A NaN would be written as
//! one canonical pattern, because Rust does not fix a NaN's sign or payload (RFC 3514), but no
//! result here may be NaN, and the test checks that.
//!
//! **The same digest must hold on every target `.github/workflows/kernel.yml` runs**: Linux and
//! Windows on x86-64 and arm64, macOS on arm64 and x86-64, and wasm32-wasip1 under wasmtime and
//! under Node; in debug and in release, release both with overflow checks on and as it ships
//! (without them), and natively with `-C target-cpu=native`. The digest lives in the golden store
//! (`geom-test/golden/store.json`, id `geom-num/determinism`), and `geom_test::golden::check` runs
//! the corpus twice in the process, each time on a heap the perturbing allocator has shifted and
//! fragmented differently, before it compares the digest with the store's. A changed digest is one
//! of two things. Either it is a bug: some target computes different bits, so the same model would
//! give different geometry on different machines. Or the kernel's results were changed on purpose,
//! which is a deliberate kernel edition change (chapter 21, section 28.13): its pull request
//! records the change in the store (the old digest in the golden's history, with its kind, piece
//! and why), says which results changed, and a kernel owner approves it. A change to this corpus
//! changes the digest too and is recorded the same way. Never update the store to make one target
//! pass.
//!
//! To help find the cause when one target disagrees, the test prints a digest per section: compare
//! the CI logs of two targets to see which part of the kernel differs. It also checks that the
//! corpus still reaches every outcome it is built to reach (zero signs, degenerate constructions,
//! overflow, rounding, ties, every error), so a corpus that silently stopped testing the hard
//! cases fails.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); the corpus
// generator may use them. It may not use std's maxima or reductions: nothing here relies on them.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the corpus generator; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken corpus must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
#![allow(
    clippy::many_single_char_names,
    reason = "points are a to e as in the predicates; g and r are the generator and recorder"
)]

use std::cmp::Ordering;
use std::collections::BTreeSet;

use geom_num::eft::{fast_two_sum, split, two_prod, two_sum};
use geom_num::grid::{
    COORD_MAX, QUARTER_NM_PER_INCH, QUARTER_NM_PER_MICRON, QUARTER_NM_PER_MIL, QUARTER_NM_PER_MM,
    QUARTER_NM_PER_NM, coarsen, refine,
};
use geom_num::interval::Interval;
use geom_num::math::unproven;
use geom_num::plane::{Plane, Tpi};
use geom_num::predicate::{incircle, insphere, orient2d, orient3d};
use geom_num::{Coord, Exactness, GridStep, NumError, P2, P3, Real, Rounded, Sign, Steps, Wide};
use geom_num::{ladder, math};
use geom_test::golden::{self, Sha256, hex};

/// The corpus's golden in the golden store (`geom-test/golden/store.json`).
///
/// The same digest must hold on every target `.github/workflows/kernel.yml` runs (the module
/// documentation lists them). A change is either a bug (a target computes different bits, so the
/// same model would give different geometry on different machines) or a deliberate kernel edition
/// change, recorded in the store in a pull request that says which and is approved by a kernel
/// owner. See the module documentation.
///
/// The digest's history is in the store: G02 set it, and it changed once, when the correctly
/// rounded maths of piece G05 (issue #174) merged. The sections `predicates`, `planes`, `tpi`,
/// `grid`, `wide`, `eft` and `scalars` give G02's digests unchanged. `interval` no longer records
/// the refusals of `Interval::exp` and `Interval::ln` (indices 3 and 4), which are certified now
/// and recorded in `math`; with those two records put back it gives G02's digest, so nothing else
/// in it moved. `math`: of G02's 7,677 records, 92 moved, each by one unit in the last place to the
/// correctly rounded value, where `libm` (and, for `sinpi` and `cospi`, G02's reduction before
/// `libm`) was one unit off: `exp` 9, `ln` 3, `sinpi` 52, `cospi` 19 and `atan2` 9 (for example
/// `exp(1)`, from 2.7182818284590455 to 2.718281828459045, the double nearest e). No error
/// changed, and no other function's result. 2,700 records were added, from a generator of their
/// own (`SEED_G05`): the `libm` twins of the five, now in `math::unproven` (indices 23 to 27), and
/// the interval versions `Interval::sinpi`, `cospi`, `exp` and `ln` (28 to 31). It changed again
/// when piece G47 built `Interval::atan2`: `interval` no longer records its refusal (index 2), and
/// `math` gains 601 records of it (index 32) from a third generator (`SEED_G47`), after every
/// earlier record, so nothing else in either section moved.
const GOLDEN: &str = "geom-num/determinism";

/// The calls each section makes, in order: the corpus is fixed, so these are its exact counts,
/// and a section that loses calls fails by name.
const SECTION_CALLS: [(&str, u64); 9] = [
    ("predicates", 12_000),
    ("planes", 4_732),
    ("tpi", 11_491),
    ("grid", 4_000),
    ("wide", 15_616),
    ("eft", 6_044),
    ("interval", 19_148),
    ("math", 10_978),
    ("scalars", 4_209),
];

/// Multiplier of the corpus generator: Knuth's MMIX linear congruential generator modulo 2^64
/// (*The Art of Computer Programming*, volume 2, section 3.3.4).
const LCG_MUL: u64 = 6_364_136_223_846_793_005;

/// Increment of the corpus generator (Knuth's MMIX).
const LCG_ADD: u64 = 1_442_695_040_888_963_407;

/// The generator's seed: "SPAREKIT" in ASCII.
const SEED: u64 = 0x5350_4152_454b_4954;

/// The bits any NaN would be written as: the quiet NaN with a clear sign and an empty payload.
const CANONICAL_NAN: u64 = 0x7ff8_0000_0000_0000;

/// The seed of the maths corpus's second generator, for the calls added with the correctly
/// rounded functions (their `libm` twins and the interval versions): `G05_MATH` in ASCII. A
/// generator of their own, so that the calls before them and the sections after them see the
/// same inputs as before they were added.
const SEED_G05: u64 = 0x4730_355f_4d41_5448;

/// The seed of the maths corpus's third generator, for `Interval::atan2`'s records (piece G47):
/// `G47_MATH` in ASCII, a generator of its own for the same reason as [`SEED_G05`].
const SEED_G47: u64 = 0x4734_375f_4d41_5448;

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
/// cases, the smallest normal values of both signs, the smallest positive subnormal and the
/// negative subnormal of largest magnitude, the largest finite values and the neighbours of 1.
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
    // 34 was `Interval::sqrt_nonneg`, private since the third review round: never reuse it.
    IntervalNeg = 35,
    Contains = 36,
    CertainSign = 37,
    Math = 38,
    IntervalNorm = 39,
    IntervalPoint = 40,
    RealNew = 41,
    RealMax = 42,
    RealMin = 43,
    RealCmp = 44,
    CoordNew = 45,
    CoordFromI128 = 46,
    P3New = 47,
    IntervalMulAdd = 48,
    IntervalUnproven = 49,
    SignOps = 50,
    GridStepOps = 51,
    StepsOps = 52,
    ExactnessOps = 53,
    Ladder = 54,
    WideTests = 55,
}

/// Writes every result into the hash in the canonical encoding, and keeps the tallies.
///
/// The encoding, every value little-endian, in this field order:
///
/// - a call: its `Op` discriminant as one byte, then its result (a maths call also writes the
///   function's index, one byte, before its result, and so does each refusal of an unproven
///   interval function; an interval version of a maths function is a maths call whose result is
///   an interval);
/// - a predicate's or a plane's `Result`: only its `Ok` value, unwrapped, since no grid input can
///   make one an error (a `BudgetExceeded` here would stop the test, louder than a changed hash);
/// - `Real`: its value as an `f64`;
/// - `Sign` and `Ordering`: one byte, 0 negative or less, 1 zero or equal, 2 positive or greater;
/// - `bool`: one byte, 0 or 1;
/// - integers: their two's-complement bytes at their own width (`i64` 8 bytes, `i128` 16);
/// - `Wide<L>`: its L limbs, least significant first, 8 bytes each;
/// - `f64`: its 8 IEEE 754 bytes, with any NaN as `CANONICAL_NAN`;
/// - `Option`: 0 for `None`, or 1 then the value;
/// - `Result`: 0 then the value, or the error;
/// - `NumError`: one byte, 1 out of range (then its `i128` value), 2 degenerate, 3 not finite,
///   4 wrong direction, 5 divisor contains zero, 6 negative square root, 7 empty interval,
///   8 outside a domain, 9 budget exceeded, 10 tolerance exceeded, 11 unproven maths;
/// - a point: x, y, z; a `Rounded`: the value, then `exact`; a plane: the normal's x, y, z, then
///   the offset; an interval: lo, then hi; a pair of doubles: the first, then the second;
/// - a `GridStep`: one byte, 0, 1 or 2 for `Q0`, `Q1` and `Q2`; `Steps`: its count as a `u64`; an
///   `Exactness`: one byte, 0 exact, 1 certified, 2 approximate, then for the last two the count
///   and its step.
struct Recorder {
    /// The hash of everything.
    total: Sha256,
    /// The hash of the current section.
    section: Sha256,
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
    /// Each finished section: name, calls, digest.
    report: Vec<(&'static str, u64, [u8; 32])>,
}

impl Recorder {
    /// Nothing recorded yet.
    fn new() -> Recorder {
        Recorder {
            total: Sha256::new(),
            section: Sha256::new(),
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
        let section = std::mem::replace(&mut self.section, Sha256::new());
        self.report
            .push((name, self.section_calls, section.finish()));
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
            NumError::OutsideDomain => (8, "OutsideDomain"),
            NumError::BudgetExceeded => (9, "BudgetExceeded"),
            NumError::ToleranceExceeded => (10, "ToleranceExceeded"),
            NumError::UnprovenMaths => (11, "UnprovenMaths"),
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
        self.point(r.value());
        self.exact(r.is_exact());
    }

    fn rounded_coord(&mut self, r: Rounded<Coord>) {
        self.put_i64(r.value().get());
        self.exact(r.is_exact());
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
        if e == 0.0 {
            self.note("zero");
        }
    }

    fn real(&mut self, v: Real) {
        self.put_f64(v.get());
    }

    fn steps(&mut self, s: Steps) {
        self.put_u64(s.get());
    }

    fn grid_step(&mut self, s: GridStep) {
        self.put_u8(match s {
            GridStep::Q0 => 0,
            GridStep::Q1 => 1,
            GridStep::Q2 => 2,
        });
    }

    /// An exactness class: 0 exact; 1 certified or 2 approximate, then the count and its step.
    fn exactness(&mut self, x: Exactness) {
        let (byte, class) = match x {
            Exactness::Exact => (0, "Exact"),
            Exactness::Certified { .. } => (1, "Certified"),
            Exactness::Approximate { .. } => (2, "Approximate"),
        };
        self.put_u8(byte);
        self.note(class);
        if let Some((count, step)) = x.bound() {
            self.steps(count);
            self.grid_step(step);
        }
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

    /// 2^k for k in 0..=`max_log2`.
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
    #[allow(
        clippy::cast_possible_wrap,
        reason = "the random bits are reinterpreted as two's complement on purpose"
    )]
    fn int128(&mut self) -> i128 {
        let raw = (u128::from(self.bits()) << 64) | u128::from(self.bits());
        let shift = u32::try_from(self.below(128)).unwrap();
        (raw as i128) >> shift
    }

    /// A random `i64` with a random number of significant bits.
    #[allow(
        clippy::cast_possible_wrap,
        reason = "the random bits are reinterpreted as two's complement on purpose"
    )]
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
        r.sign(orient2d(pt2(a), pt2(b), pt2(c)).unwrap());

        let [a, b, c, d] = tetrahedron(g);
        r.call(Op::Orient3d);
        r.sign(orient3d(pt3(a), pt3(b), pt3(c), pt3(d)).unwrap());

        let [a, b, c, d] = circle_quad(g);
        r.call(Op::Incircle);
        r.sign(incircle(pt2(a), pt2(b), pt2(c), pt2(d)).unwrap());

        let [a, b, c, d, e] = sphere_quint(g, &sphere);
        r.call(Op::Insphere);
        r.sign(insphere(pt3(a), pt3(b), pt3(c), pt3(d), pt3(e)).unwrap());
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
    r.sign(plane.side_of_tpi(t).unwrap());
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
            r.sign(plane.side(pt3(p)).unwrap());
        }
    }
}

/// Three planes, each by three points, and whether they were built to meet on a tie (case 4 with
/// an odd s, where the commit rounds half to even on two axes).
fn tpi_planes(g: &mut Gen) -> ([[[i64; 3]; 3]; 3], bool) {
    let mut tie = false;
    let planes = match g.below(6) {
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
            tie = s % 2 == 1;
            order.map(|k| planes[k].map(place))
        }
        _ => {
            let mut planes = [[[0; 3]; 3]; 3];
            for point in planes.iter_mut().flatten() {
                *point = g.e3();
            }
            planes
        }
    };
    (planes, tie)
}

fn three_plane_points(g: &mut Gen, r: &mut Recorder) {
    for _ in 0..1_200 {
        let ([pp, qp, sp], tie) = tpi_planes(g);
        let (Some(p), Some(q), Some(s)) = (through(r, pp), through(r, qp), through(r, sp)) else {
            continue;
        };
        let t = Tpi::new(p, q, s);
        r.call(Op::TpiNew);
        r.result(t, |_, _| {});
        let Ok(t) = t else { continue };

        let (x, w) = t.homogeneous().unwrap();
        r.call(Op::TpiHomogeneous);
        for xi in x {
            r.wide(xi);
        }
        r.wide(w);

        let committed = t.to_grid();
        r.call(Op::ToGrid);
        r.result(committed, Recorder::rounded_point);
        if tie && committed.is_ok_and(|c| !c.is_exact()) {
            r.note("tie");
        }

        // Against one of its own planes (always zero), a plane through random points, and a
        // plane through the committed grid point.
        let own = g.pick(&[p, q, s]);
        classify(r, &own, &t);
        let random = [g.anywhere3(), g.anywhere3(), g.anywhere3()];
        if let Some(other) = through(r, random) {
            classify(r, &other, &t);
        }
        if let Ok(c) = committed {
            let c = c.value();
            let c = [c.x.get(), c.y.get(), c.z.get()];
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
        r.result(coarsen(c, from, to), Recorder::rounded_coord);
    }
}

// ---------------------------------------------------------------------------------------------
// Wide integers.
// ---------------------------------------------------------------------------------------------

/// Two's-complement negation of raw limbs, in place.
fn negate<const L: usize>(limbs: &mut [u64; L]) {
    let mut carry = true;
    for limb in &mut *limbs {
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
        r.option(a.checked_add(&b), Recorder::wide);
        r.call(Op::WideSub);
        r.option(a.checked_sub(&b), Recorder::wide);
        r.call(Op::WideMul);
        r.option(a.checked_mul(&b), Recorder::wide);
        r.call(Op::WideNeg);
        r.option(a.checked_neg(), Recorder::wide);
        r.call(Op::WideCmp);
        r.ordering(a.cmp(&b));
        r.call(Op::WideSignum);
        r.sign(a.signum());
        r.call(Op::WideResize);
        r.option(a.resize::<4>(), Recorder::wide);
        r.call(Op::WideResize);
        r.option(a.resize::<5>(), Recorder::wide);
        r.call(Op::WideResize);
        r.option(a.resize::<6>(), Recorder::wide);
        r.call(Op::WideResize);
        r.option(a.resize::<8>(), Recorder::wide);
        r.call(Op::WideToI128);
        r.option(a.to_i128(), Recorder::put_i128);
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
#[allow(
    clippy::cast_precision_loss,
    reason = "every converted integer is below 2^53 in magnitude, so the conversion is exact"
)]
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

/// Knuth's two-sum error computed here, as `two_sum` would before its fallback: infinite or NaN
/// exactly when an intermediate overflows.
fn knuth_error(a: f64, b: f64) -> f64 {
    let s = a + b;
    let b_virtual = s - a;
    let a_virtual = s - b_virtual;
    (a - a_virtual) + (b - b_virtual)
}

/// Records the four transformations of one pair (split of the first).
fn eft_calls(r: &mut Recorder, a: f64, b: f64) {
    r.call(Op::TwoSum);
    r.eft(two_sum(a, b));
    if (a + b).is_finite() && !knuth_error(a, b).is_finite() {
        r.note("fallback");
    }
    r.call(Op::FastTwoSum);
    r.eft(fast_two_sum(a, b));
    r.call(Op::TwoProd);
    r.eft(two_prod(a, b));
    r.call(Op::Split);
    r.eft(split(a));
}

/// Pairs that reach the transformations' rare routes, which random pairs miss: an intermediate of
/// Knuth's two-sum that overflows while the sum is finite (−3 · 2^970 and Ω, both orders); zero
/// errors of either sign of zero; a zero error of a product whose last Dekker step is −0
/// (2 − 2^−52 times 1); a product whose error is finer than 2^−1074 (2^−600 squared); and split's
/// midpoint ±(2^1024 − 2^997), where the high half becomes ±2^1024.
const EFT_FIXED: [(u64, u64); 11] = [
    (0xfca8_0000_0000_0000, 0x7fef_ffff_ffff_ffff),
    (0x7fef_ffff_ffff_ffff, 0xfca8_0000_0000_0000),
    (0x3ff0_0000_0000_0000, 0x8000_0000_0000_0000),
    (0x8000_0000_0000_0000, 0x3ff0_0000_0000_0000),
    (0x0000_0000_0000_0000, 0x8000_0000_0000_0000),
    (0x8000_0000_0000_0000, 0x8000_0000_0000_0000),
    (0x3fff_ffff_ffff_ffff, 0x3ff0_0000_0000_0000),
    (0xbfff_ffff_ffff_ffff, 0x3ff0_0000_0000_0000),
    (0x1a70_0000_0000_0000, 0x1a70_0000_0000_0000),
    (0x7fef_ffff_fc00_0000, 0x3ff0_0000_0000_0000),
    (0xffef_ffff_fc00_0000, 0x3ff0_0000_0000_0000),
];

fn error_free(g: &mut Gen, r: &mut Recorder) {
    for (a, b) in EFT_FIXED {
        eft_calls(r, f64::from_bits(a), f64::from_bits(b));
    }
    for _ in 0..1_500 {
        let (a, b) = eft_pair(g);
        eft_calls(r, a, b);
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
    r.result(made, Recorder::interval);
    r.call(Op::IntervalPoint);
    r.result(Interval::point(hi), Recorder::interval);
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

/// Records `Interval::norm` of `components`, noting a lower bound of exactly 0.
fn norm_call(r: &mut Recorder, components: &[Interval]) {
    r.call(Op::IntervalNorm);
    r.result(Interval::norm(components), |r, v| {
        if v.lo() == 0.0 {
            r.note("zero lower bound");
        }
        r.interval(v);
    });
}

fn intervals(g: &mut Gen, r: &mut Recorder) {
    // The length of no components; between coincident points, whose coordinate differences are
    // widened around 0; and of a component whose square overflows.
    let d = Interval::point(1.5)
        .unwrap()
        .sub(Interval::point(1.5).unwrap())
        .unwrap();
    norm_call(r, &[]);
    norm_call(r, &[d, d, d]);
    norm_call(r, &[Interval::point(f64::MAX).unwrap()]);
    // The enclosures that are not certified refuse every interval: one call each, the function's
    // index before the result. Indices 3 and 4 were `exp` and `ln`, refused until piece G05 made
    // them certified, and index 2 `atan2`, refused until piece G47 did; they are recorded with the
    // maths functions now (`maths`), and their indices here are not reused.
    let unit = Interval::new(-1.0, 1.0).unwrap();
    let refusals = [unit.sin(), unit.cos()];
    for (index, refused) in (0u8..).zip(refusals) {
        r.call(Op::IntervalUnproven);
        r.put_u8(index);
        r.result(refused, Recorder::interval);
    }
    for _ in 0..1_200 {
        let x = interval_input(g, r);
        let y = interval_input(g, r);
        r.call(Op::IntervalAdd);
        r.result(x.add(y), Recorder::interval);
        r.call(Op::IntervalSub);
        r.result(x.sub(y), Recorder::interval);
        r.call(Op::IntervalMul);
        r.result(x.mul(y), Recorder::interval);
        r.call(Op::IntervalDiv);
        r.result(x.div(y), Recorder::interval);
        r.call(Op::IntervalSquare);
        r.result(x.square(), Recorder::interval);
        r.call(Op::IntervalSqrt);
        r.result(x.sqrt(), Recorder::interval);
        r.call(Op::IntervalNeg);
        r.interval(x.neg());

        let p = probe(g, x);
        let inside = x.contains(p);
        r.call(Op::Contains);
        r.put_bool(inside);
        r.note(if inside { "true" } else { "false" });

        r.call(Op::CertainSign);
        r.option(x.certain_sign(), Recorder::sign);
        if let Ok(z) = x.sub(y) {
            r.call(Op::CertainSign);
            r.option(z.certain_sign(), Recorder::sign);
        }

        norm_call(r, &[x, y]);
        r.call(Op::IntervalMulAdd);
        r.result(x.mul_add(y, x), Recorder::interval);
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

#[allow(
    clippy::cast_precision_loss,
    reason = "every converted integer is below 2^53 in magnitude, so the conversion is exact"
)]
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
#[allow(
    clippy::cast_precision_loss,
    reason = "every converted integer is below 2^53 in magnitude, so the conversion is exact"
)]
fn pow_input(g: &mut Gen) -> (f64, f64) {
    match g.below(5) {
        0 | 1 => (g.positive(-20, 20), g.double(-6, 6)),
        2 => (-g.positive(-10, 10), g.int(-60, 60) as f64),
        3 => (g.pick(&[0.0, -0.0, 1.0]), g.double(-10, 10)),
        _ => (g.positive(-30, 30), g.double(5, 12)),
    }
}

/// Records one maths call: the function's index, then its result.
fn math_call(r: &mut Recorder, index: u8, v: Result<Real, NumError>) {
    r.call(Op::Math);
    r.put_u8(index);
    r.result(v, Recorder::real);
}

/// Records one call of an interval version of a maths function: the function's index, then its
/// result.
fn interval_math_call(r: &mut Recorder, index: u8, v: Result<Interval, NumError>) {
    r.call(Op::Math);
    r.put_u8(index);
    r.result(v, Recorder::interval);
}

/// The largest finite double's bits: `math_interval` keeps a bound's magnitude at or below it.
const MAX_BITS: u64 = 0x7fef_ffff_ffff_ffff;

/// An argument for an interval version of a maths function, from inputs of `domain`: a point, two
/// inputs in order, or an input and the double up to 2^32 steps farther from zero (narrow
/// intervals at and next to the peaks of `sinpi` and `cospi` and the edges of the domains).
fn math_interval(g: &mut Gen, domain: Domain) -> Interval {
    let x = math_input(g, domain);
    let (lo, hi) = match g.below(3) {
        0 => (x, x),
        1 => sorted(x, math_input(g, domain)),
        _ => {
            let bits = x.to_bits();
            let far = (bits & !SIGN_BIT) + g.below(1 << 32);
            let magnitude = if far > MAX_BITS { MAX_BITS } else { far };
            sorted(x, f64::from_bits((bits & SIGN_BIT) | magnitude))
        }
    };
    Interval::new(lo, hi).unwrap()
}

/// A one-argument maths function and the inputs it is sampled on.
type Unary = (fn(f64) -> Result<Real, NumError>, Domain);

/// The value one-argument functions are given outside their domains: NaN or an infinity.
const NOT_FINITE: [f64; 3] = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY];

fn maths(g: &mut Gen, r: &mut Recorder) {
    // A function's index is its place in the encoding: never renumber. The correctly rounded
    // `exp`, `ln`, `sinpi`, `cospi` and `atan2` of `math` hold 7, 8, 16, 17 and 18, which the
    // `libm` versions held before them; those are now `math::unproven`'s and hold 23 to 27. The
    // other `libm` functions moved to `math::unproven` with their indices.
    let unary: [Unary; 18] = [
        (math::sqrt, Domain::NonNegative),
        (unproven::sin, Domain::Angle),
        (unproven::cos, Domain::Angle),
        (unproven::tan, Domain::Angle),
        (unproven::asin, Domain::Unit),
        (unproven::acos, Domain::Unit),
        (unproven::atan, Domain::Any),
        (math::exp, Domain::Exponent),
        (math::ln, Domain::Positive),
        (unproven::log2, Domain::Positive),
        (unproven::log10, Domain::Positive),
        (unproven::cbrt, Domain::Any),
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
        math_call(r, 19, unproven::pow(x, y));
    }
    for _ in 0..300 {
        let (x, y) = if g.one_in(2) {
            (g.any_or_zero(), g.any_or_zero())
        } else {
            let e = g.int(-1074, 1023);
            (g.double(e, e), g.double(e, e))
        };
        math_call(r, 20, unproven::hypot(x, y));
    }
    for _ in 0..300 {
        let (a, b, c) = (g.any_or_zero(), g.any_or_zero(), g.any_or_zero());
        math_call(r, 21, math::fma(a, b, c));
    }
    for _ in 0..300 {
        let x = g.double(-10, 10);
        let n = i32::try_from(g.int(-80, 80)).unwrap();
        math_call(r, 22, unproven::powi(x, n));
    }
    for _ in 0..300 {
        maths_outside(g, r, &unary);
    }
    proven_twins_and_intervals(r);
}

/// The calls added with the correctly rounded functions, from a generator of their own
/// (`SEED_G05`): the `libm` twins of the proven functions, on the same domains, then the interval
/// versions of the proven ones, inside and outside their domains (`ln` over any interval one time
/// in four, so its refusals of negative numbers and of the pole are recorded too).
fn proven_twins_and_intervals(r: &mut Recorder) {
    let g = &mut Gen(SEED_G05);
    let twins: [Unary; 4] = [
        (unproven::exp, Domain::Exponent),
        (unproven::ln, Domain::Positive),
        (unproven::sinpi, Domain::HalfTurns),
        (unproven::cospi, Domain::HalfTurns),
    ];
    for (index, (f, domain)) in (23u8..).zip(twins) {
        for _ in 0..300 {
            let x = math_input(g, domain);
            math_call(r, index, f(x));
        }
    }
    for _ in 0..300 {
        let (y, x) = (g.any_or_zero(), g.any_or_zero());
        math_call(r, 27, unproven::atan2(y, x));
    }
    for _ in 0..300 {
        let x = math_interval(g, Domain::HalfTurns);
        interval_math_call(r, 28, x.sinpi());
        interval_math_call(r, 29, x.cospi());
        let x = math_interval(g, Domain::Exponent);
        interval_math_call(r, 30, x.exp());
        let domain = if g.one_in(4) {
            Domain::Any
        } else {
            Domain::Positive
        };
        let x = math_interval(g, domain);
        interval_math_call(r, 31, x.ln());
    }
    // `Interval::atan2` (index 32), from a generator of its own: boxes of any finite doubles, which
    // meet the axes, the branch cut and the origin, and the origin alone, which it refuses.
    let g = &mut Gen(SEED_G47);
    for _ in 0..600 {
        let (y, x) = (math_interval(g, Domain::Any), math_interval(g, Domain::Any));
        interval_math_call(r, 32, y.atan2(x));
    }
    let origin = Interval::point(0.0).unwrap();
    interval_math_call(r, 32, origin.atan2(origin));
}

/// One call outside a function's domain, at a pole, past an overflow or with an argument that is
/// not finite: each must be reported, `OutsideDomain` or `NotFinite`, never NaN or an infinity.
fn maths_outside(g: &mut Gen, r: &mut Recorder, unary: &[Unary; 18]) {
    let beyond_one = |g: &mut Gen| {
        let sign = if g.one_in(2) { -1.0 } else { 1.0 };
        sign * (1.0 + g.positive(-52, 1000))
    };
    match g.below(10) {
        0 => math_call(r, 0, math::sqrt(-g.positive(-1074, 1023))),
        1 => {
            let x = beyond_one(g);
            math_call(r, 4, unproven::asin(x));
            math_call(r, 5, unproven::acos(x));
        }
        2 => {
            let x = -g.positive(-1074, 1023);
            math_call(r, 8, math::ln(x));
            math_call(r, 9, unproven::log2(x));
            math_call(r, 10, unproven::log10(x));
        }
        3 => {
            let zero = g.pick(&[0.0, -0.0]);
            math_call(r, 8, math::ln(zero));
            math_call(r, 9, unproven::log2(zero));
            math_call(r, 10, unproven::log10(zero));
        }
        4 => {
            // A negative base and a power with a half in it: no real value.
            let y = f64::from(i32::try_from(g.int(-20, 20)).unwrap()) + 0.5;
            math_call(r, 19, unproven::pow(-g.positive(-30, 30), y));
        }
        5 => {
            let zero = g.pick(&[0.0, -0.0]);
            math_call(r, 19, unproven::pow(zero, -g.positive(-10, 10)));
            math_call(
                r,
                22,
                unproven::powi(zero, -1 - i32::try_from(g.int(0, 9)).unwrap()),
            );
        }
        6 => {
            math_call(r, 7, math::exp(g.double(10, 1023).abs()));
            math_call(
                r,
                20,
                unproven::hypot(g.double(1023, 1023), g.double(1023, 1023)),
            );
        }
        7 => {
            let (a, b) = (g.double(600, 1023), g.double(600, 1023));
            math_call(r, 21, math::fma(a, b, g.any_or_zero()));
            math_call(r, 22, unproven::powi(g.double(4, 20), 1100));
        }
        _ => {
            let bad = g.pick(&NOT_FINITE);
            let k = usize::try_from(g.below(18)).unwrap();
            let (f, _) = unary[k];
            math_call(r, u8::try_from(k).unwrap(), f(bad));
            let x = g.any();
            let (y, z) = if g.one_in(2) { (bad, x) } else { (x, bad) };
            math_call(r, 18, math::atan2(y, z));
            math_call(r, 19, unproven::pow(y, z));
            math_call(r, 20, unproven::hypot(y, z));
            math_call(r, 21, math::fma(y, z, x));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Scalars, constants and the small types.
// ---------------------------------------------------------------------------------------------

/// The two orders of +0 and −0, for the total-order helpers: `f64::max` and `f64::min` may return
/// either zero here, and `Real`'s helpers must not.
const ZERO_PAIRS: [(f64, f64); 2] = [(0.0, -0.0), (-0.0, 0.0)];

/// `Real`: made from any double, NaN and the infinities included; then the total-order helpers and
/// the order on pairs of them, with both orders of the zeros first.
fn reals(g: &mut Gen, r: &mut Recorder) {
    for (a, b) in ZERO_PAIRS {
        let (a, b) = (Real::new(a).unwrap(), Real::new(b).unwrap());
        r.call(Op::RealMax);
        r.real(a.total_max(b));
        r.note("zeros");
        r.call(Op::RealMin);
        r.real(a.total_min(b));
        r.note("zeros");
        r.call(Op::RealCmp);
        r.ordering(a.cmp(&b));
        r.note("zeros");
    }
    for _ in 0..500 {
        let x = if g.one_in(16) {
            g.pick(&NOT_FINITE)
        } else {
            g.any_or_zero()
        };
        r.call(Op::RealNew);
        r.result(Real::new(x), Recorder::real);
        let (Ok(a), Ok(b)) = (Real::new(x), Real::new(g.any_or_zero())) else {
            continue;
        };
        r.call(Op::RealMax);
        r.real(a.total_max(b));
        r.call(Op::RealMin);
        r.real(a.total_min(b));
        r.call(Op::RealCmp);
        r.ordering(a.cmp(&b));
    }
}

/// Coordinates and points from values at and beyond the frame's edges, unclamped.
fn coordinates(g: &mut Gen, r: &mut Recorder) {
    for _ in 0..500 {
        let v = g.extreme() + g.int(-2, 2);
        r.call(Op::CoordNew);
        r.result(Coord::new(v), |r, c| r.put_i64(c.get()));
        r.call(Op::CoordFromI128);
        r.result(Coord::from_i128(g.int128()), |r, c| r.put_i64(c.get()));
        let (x, y, z) = (
            g.extreme() + g.int(-1, 1),
            g.extreme(),
            g.extreme() + g.int(-1, 1),
        );
        r.call(Op::P3New);
        r.result(P3::new(x, y, z), Recorder::point);
    }
}

/// `Sign`: every method on every sign and pair of signs. `GridStep`: sizes, rungs, neighbours and
/// the named units.
fn small_types(r: &mut Recorder) {
    let signs = [Sign::Negative, Sign::Zero, Sign::Positive];
    for a in signs {
        r.call(Op::SignOps);
        r.sign(a.negate());
        r.put_bool(a.is_zero());
        r.put_u8(a.as_i8().to_le_bytes()[0]);
        for b in signs {
            r.sign(a.mul(b));
        }
    }
    r.call(Op::SignOps);
    r.sign(Sign::of_i64(i64::MIN));
    r.sign(Sign::of_i128(i128::MAX));
    let units = [
        QUARTER_NM_PER_MM,
        QUARTER_NM_PER_INCH,
        QUARTER_NM_PER_MICRON,
        QUARTER_NM_PER_MIL,
        QUARTER_NM_PER_NM,
    ];
    for step in STEPS {
        r.call(Op::GridStepOps);
        r.put_u64(u64::from(step.quarter_nm()));
        r.put_u64(u64::from(step.rung()));
        r.option(step.finer(), Recorder::grid_step);
        r.option(step.coarser(), Recorder::grid_step);
        for unit in units {
            r.option(step.steps_per(unit), Recorder::put_u64);
        }
    }
}

/// A count of steps: small, or one of the four largest.
fn step_count(g: &mut Gen) -> u64 {
    if g.one_in(8) {
        u64::MAX - g.below(4)
    } else {
        g.below(10_000)
    }
}

/// An exactness class with a bound of `n` steps.
fn exactness_class(g: &mut Gen, n: u64) -> Exactness {
    match g.below(3) {
        0 => Exactness::Exact,
        1 => Exactness::Certified {
            bound: Steps(n),
            step: g.pick(&STEPS),
        },
        _ => Exactness::Approximate {
            deviation: Steps(n),
            step: g.pick(&STEPS),
        },
    }
}

/// `Steps` and `Exactness`: sums and conversions that fit and that do not, and classes combined and
/// judged against the fit bound.
fn bounds(g: &mut Gen, r: &mut Recorder) {
    for _ in 0..300 {
        let (a, b) = (Steps(step_count(g)), Steps(step_count(g)));
        let (from, to) = (g.pick(&STEPS), g.pick(&STEPS));
        r.call(Op::StepsOps);
        r.option(a.checked_add(b), Recorder::steps);
        r.option(a.refine(from, to), Recorder::steps);
        let (na, nb) = (step_count(g), step_count(g));
        let (x, y) = (exactness_class(g, na), exactness_class(g, nb));
        let ceiling_step = g.pick(&STEPS);
        r.call(Op::ExactnessOps);
        r.option(x.combine(y), Recorder::exactness);
        r.result(x.within(ladder::FIT, ceiling_step), Recorder::exactness);
        let combined = x.combine_within(y, ladder::FIT, ceiling_step);
        r.result(combined, Recorder::exactness);
        r.put_u8(x.rank());
    }
}

/// The ladder's constants, and its rungs in quarter-nanometres on every step.
fn ladder_constants(r: &mut Recorder) {
    r.call(Op::Ladder);
    for rung in [
        ladder::FIT,
        ladder::HEAL_DEFAULT,
        ladder::HEAL_CAP,
        ladder::SHORT_EDGE_WARNING,
        ladder::COMMIT_DISTANCE_2D,
        ladder::COMMIT_DISTANCE_3D,
    ] {
        r.steps(rung);
        for step in STEPS {
            r.option(ladder::quarter_nm(rung, step), |r, q| {
                r.bytes(&q.to_le_bytes());
            });
        }
    }
    r.put_u64(ladder::COMMIT_PER_AXIS.0);
    r.put_i64(i64::from(ladder::ANGULAR_LOG2));
    r.real(ladder::ANGULAR);
}

/// `Wide::is_negative` and `is_zero`, on edges of the range and random values.
fn wide_tests(g: &mut Gen, r: &mut Recorder) {
    for _ in 0..200 {
        let w = wide_value::<6>(g);
        r.call(Op::WideTests);
        r.put_bool(w.is_negative());
        r.put_bool(w.is_zero());
        r.note(if w.is_zero() { "zero" } else { "nonzero" });
    }
}

/// The scalars and small types, in a fixed order.
fn scalars(g: &mut Gen, r: &mut Recorder) {
    reals(g, r);
    coordinates(g, r);
    small_types(r);
    bounds(g, r);
    ladder_constants(r);
    wide_tests(g, r);
}

// ---------------------------------------------------------------------------------------------
// The tests.
// ---------------------------------------------------------------------------------------------

/// Outcomes the corpus is built to reach, beyond the families `required` lists.
const OUTCOMES: [(Op, &str); 71] = [
    (Op::PlaneThrough, "Ok"),
    (Op::PlaneThrough, "Degenerate"),
    (Op::TpiNew, "Ok"),
    (Op::TpiNew, "Degenerate"),
    (Op::TwoSum, "fallback"),
    (Op::ToGrid, "exact"),
    (Op::ToGrid, "inexact"),
    (Op::ToGrid, "tie"),
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
    (Op::IntervalAdd, "Ok"),
    (Op::IntervalAdd, "NotFinite"),
    (Op::IntervalSub, "Ok"),
    (Op::IntervalSub, "NotFinite"),
    (Op::IntervalMul, "Ok"),
    (Op::IntervalMul, "NotFinite"),
    (Op::IntervalMulAdd, "Ok"),
    (Op::IntervalMulAdd, "NotFinite"),
    (Op::IntervalDiv, "Ok"),
    (Op::IntervalDiv, "DivisorContainsZero"),
    (Op::IntervalDiv, "NotFinite"),
    (Op::IntervalSquare, "Ok"),
    (Op::IntervalSquare, "NotFinite"),
    (Op::IntervalSqrt, "Ok"),
    (Op::IntervalSqrt, "NegativeSqrt"),
    (Op::IntervalNorm, "Ok"),
    (Op::IntervalNorm, "NotFinite"),
    (Op::IntervalNorm, "zero lower bound"),
    (Op::IntervalPoint, "Ok"),
    (Op::IntervalPoint, "NotFinite"),
    (Op::IntervalUnproven, "UnprovenMaths"),
    (Op::Contains, "true"),
    (Op::Contains, "false"),
    (Op::CertainSign, "None"),
    (Op::Math, "Ok"),
    (Op::Math, "OutsideDomain"),
    (Op::Math, "NotFinite"),
    (Op::RealNew, "Ok"),
    (Op::RealNew, "NotFinite"),
    (Op::RealMax, "zeros"),
    (Op::RealMin, "zeros"),
    (Op::RealCmp, "zeros"),
    (Op::RealCmp, "Less"),
    (Op::RealCmp, "Equal"),
    (Op::RealCmp, "Greater"),
    (Op::CoordNew, "Ok"),
    (Op::CoordNew, "OutOfRange"),
    (Op::CoordFromI128, "Ok"),
    (Op::CoordFromI128, "OutOfRange"),
    (Op::P3New, "Ok"),
    (Op::P3New, "OutOfRange"),
    (Op::StepsOps, "Some"),
    (Op::StepsOps, "None"),
    (Op::ExactnessOps, "Exact"),
    (Op::ExactnessOps, "Certified"),
    (Op::ExactnessOps, "Approximate"),
    (Op::ExactnessOps, "ToleranceExceeded"),
    (Op::ExactnessOps, "None"),
    (Op::WideTests, "zero"),
    (Op::WideTests, "nonzero"),
];

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
        out.push((op, "zero"));
    }
    out.extend(OUTCOMES);
    out
}

/// The whole corpus, recorded section by section.
fn corpus() -> Recorder {
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
    scalars(&mut g, &mut r);
    r.end_section("scalars");
    r
}

/// The golden digest. The same digest must hold on every target; a change is a bug or a
/// deliberate kernel edition change (see the module documentation and `GOLDEN`).
#[test]
fn golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let r = corpus();
        (r.total.clone().finish(), r)
    });
    let r = &checked.value;
    let hash = hex(r.total.clone().finish());
    println!("geom-num determinism corpus: {} calls", r.calls);
    for (name, calls, digest) in &r.report {
        println!("  {name:<10} {calls:>6} calls  {}", hex(*digest));
    }
    println!("geom-num golden hash: {hash}");

    // The corpus must still reach every outcome it is built for, make every call of every
    // section, and never see NaN (no result in the kernel may be NaN).
    let missing: Vec<_> = required()
        .into_iter()
        .filter(|k| !r.seen.contains(k))
        .collect();
    assert!(missing.is_empty(), "unreached outcomes: {missing:?}");
    let counts: Vec<(&str, u64)> = r.report.iter().map(|&(n, c, _)| (n, c)).collect();
    for ((name, calls), (want_name, want)) in counts.iter().zip(SECTION_CALLS) {
        assert_eq!(*name, want_name, "the sections are in a fixed order");
        assert!(
            *calls >= want,
            "section {name} makes {calls} calls, fewer than its {want}"
        );
    }
    assert_eq!(counts.len(), SECTION_CALLS.len(), "sections {counts:?}");
    assert_eq!(r.nans, 0, "a result in the corpus is NaN");
    checked.verdict().unwrap_or_else(|e| {
        panic!("{e}. Compare the section digests above with another target's log to find the part of the kernel that differs")
    });
}
