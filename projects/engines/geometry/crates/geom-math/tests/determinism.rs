//! The golden hash: a fixed corpus of calls into every public operation of `geom-math`, hashed
//! with SHA-256, must give the same digest on every target and on every number of threads.
//!
//! The kernel promises the same bits on every machine and for any thread count (principle 5,
//! chapter 21). `geom-num`'s golden hash holds the numbers layer to that promise; this test holds
//! the geometry maths built on it. It makes a fixed corpus of more than 30,000 calls (the test
//! prints the exact count):
//!
//! - grid vectors: construction at and beyond the vector range, differences of points at the
//!   frame's edges, dot, cross and triple products, sums, differences, negations and moved points;
//! - floating-point vectors: arithmetic, lengths and unit vectors of doubles of every exponent,
//!   subnormals, both zeros and the overflow edge included;
//! - double-double arithmetic: every operation on operands of every exponent, the integer and
//!   exact-product constructors at their limits, and exact rounding to integers with offsets, on
//!   whole numbers, ties and near-ties;
//! - grid transforms: all 48 signed permutations with translations up to and beyond their limit,
//!   applied to points and vectors at the frame's edges, composed, inverted and moving boxes;
//! - rational transforms: Pythagorean rotations, rational scales, fractional translations and
//!   random integer matrices, applied to points (ties included) and vectors, composed, inverted,
//!   classified and converted both ways;
//! - general transforms: entries of every magnitude, entries near 2^±900 that cancel, subnormal
//!   entries, halves that put grid images on midpoints and entries one ulp from one that defeat
//!   the floating-point filter, so every stage of `apply_grid` runs; rotations by `sinpi` and
//!   `cospi`, scales, mirrors, shears and singular maps, classified, composed and inverted;
//! - typed transforms: rotations of each variant, composition across variants, inverses,
//!   simplification and classification;
//! - boxes: grid boxes in 2D and 3D and floating-point boxes, combined, tested, moved by grid and
//!   general transforms, and rounded out to the grid;
//! - frames: map to frame and frame to map in Map frames, quarter-turned, Pythagorean and
//!   irrational Ground frames at every grid step, with origins at the site, far away and at the
//!   origin bound; on map values that land exactly on steps and midpoints, values a few ulps from
//!   a grid point's image, values on an irrational frame's axis, values that cancel the origin
//!   (the exact bisection of `to_map`), and values beyond the range, infinite or NaN; exact shifts
//!   between frames and every mismatch; lattice homes, cell ownership and the range refusal.
//!
//! The corpus comes from a 64-bit linear congruential generator, written here, from a fixed seed.
//! The inputs are made with integer arithmetic, exact conversions and IEEE 754 basic operations
//! only, so they are the same on every target by construction and only the crate's results can
//! differ.
//!
//! Every result, success or error, is written in a canonical little-endian byte encoding, in a
//! fixed field order (see `Recorder`), and hashed with SHA-256 (FIPS 180-4), the hash chapters 21
//! and 28.13 specify, from `geom_test::golden`, the one implementation every golden uses (piece
//! G19), checked there against the standard's test vectors. Doubles go in as their bits, so a
//! difference of one unit in the last place shows. No result of this crate is ever NaN; a NaN
//! would be written as one canonical pattern, because Rust does not fix a NaN's sign or payload
//! (RFC 3514), and the test fails on any.
//!
//! **The same digest must hold on every target**: Linux, Windows and macOS on x86-64 and arm64,
//! and wasm32-wasip1 under wasmtime and Node, in debug and release, with overflow checks on or off
//! and with `-C target-cpu=native` (`kernel.yml` runs it in each, through `golden run`); and the
//! corpus must give the same digest on 1, 2, 3 and 8 threads at once (chapter 21's thread-count
//! test). The digest lives in the golden store (`geom-test/golden/store.json`, id
//! `geom-math/determinism`); `geom_test::golden::check` runs the corpus twice in one process on two
//! differently perturbed heaps before comparing it with the store's (section 28.13), so a hidden
//! cache, global state or an ordering by address would show. A changed digest is one of two
//! things. Either it is a bug: some target or thread count computes different bits, so the same
//! model would give different geometry on different machines. Or the crate's results were changed
//! on purpose, which is a deliberate kernel edition change (chapter 21, section 28.13): its pull
//! request records the change in the store, says which results changed and why, and a kernel owner
//! approves it. A change to this corpus changes the digest too and is recorded the same way. Never
//! update the store to make one target pass.
//!
//! To help find the cause when one target disagrees, the test prints a digest per section: compare
//! the logs of two targets to see which part of the crate differs. It also checks that the corpus
//! still reaches every outcome it is built to reach (every transform kind, exact and inexact
//! roundings, every error these calls document), so a corpus that silently stopped testing the
//! hard cases fails.

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::disallowed_methods,
    clippy::too_many_lines,
    clippy::single_match_else,
    clippy::unreadable_literal,
    reason = "test cases and oracles, named as in the formulas they check; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken corpus must stop the test loudly. A result
// the crate documents as always `Ok` (an exact product, a classification) is unwrapped, so a
// defect reported as `BudgetExceeded` stops the test too, and the encoding is unchanged.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "the corpus generator; the rules are for kernel code"
)]

use std::collections::BTreeSet;

use geom_math::frame::{CELL_HALF, ORIGIN_MAX, steps_per_metre};
use geom_math::grid_transform::TRANSLATION_MAX;
use geom_math::vector::VEC_MAX;
use geom_math::{
    Axis, Box2, Box3, Classification, CrsId, DD, F2, F3, FBox3, FrameKind, FrameMismatch,
    FrameRecord, FrameShift, GridTransform, MapPoint, MathError, RationalTransform, Rotation,
    Rounded, Scale, Transform, TransformKind, TypedTransform, V2, V3, W3,
};
use geom_num::NumError;
use geom_num::grid::{COORD_MAX, GridStep, P2, P3};
use geom_test::golden::{self, Sha256, hex};

/// The corpus's golden in the golden store: its SHA-256 digest.
///
/// The same digest must hold on every target and thread count: see the module documentation. A
/// change is either a bug or a deliberate kernel edition change, recorded in the store in a pull
/// request that says which and is approved by a kernel owner.
///
/// Its digest replaced the corpus's 64-bit FNV-1a golden, `0x8643_5a99_5463_851e`, when the hash
/// moved to SHA-256 (chapter 28.13): both are hashes of the same bytes, so no result changed. It
/// replaced `d79932a1…72f126` (the store's history keeps the whole digest) when piece G05 made `geom_num::math::sinpi` and `cospi` correctly rounded
/// (ports of CORE-MATH in place of an msun-method approximation): only the two sections that build
/// rotations from them changed, `transform` (`6773dadd…852b18` before) and `boxes`
/// (`2e8f3297…bc7e3e` before); every other section's digest is the same.
const GOLDEN: &str = "geom-math/determinism";

/// The corpus makes at least this many calls into the crate.
const MIN_CALLS: u64 = 30_000;

/// Multiplier of the corpus generator: Knuth's MMIX linear congruential generator modulo 2^64
/// (*The Art of Computer Programming*, volume 2, section 3.3.4).
const LCG_MUL: u64 = 6_364_136_223_846_793_005;

/// Increment of the corpus generator (Knuth's MMIX).
const LCG_ADD: u64 = 1_442_695_040_888_963_407;

/// The generator's seed: "GEOMMATH" in ASCII.
const SEED: u64 = 0x4745_4f4d_4d41_5448;

/// The bits any NaN would be written as: the quiet NaN with a clear sign and an empty payload.
const CANONICAL_NAN: u64 = 0x7ff8_0000_0000_0000;

/// The sign bit of a double.
const SIGN_BIT: u64 = 1 << 63;

/// The largest coordinate magnitude, 2^40 − 1 grid steps.
const M: i64 = COORD_MAX;

/// The grid steps, coarsest first.
const STEPS: [GridStep; 3] = [GridStep::Q0, GridStep::Q1, GridStep::Q2];

/// The six orders of the three axes.
const PERMUTATIONS: [[Axis; 3]; 6] = [
    [Axis::X, Axis::Y, Axis::Z],
    [Axis::X, Axis::Z, Axis::Y],
    [Axis::Y, Axis::X, Axis::Z],
    [Axis::Y, Axis::Z, Axis::X],
    [Axis::Z, Axis::X, Axis::Y],
    [Axis::Z, Axis::Y, Axis::X],
];

/// Pythagorean directions (c² + s² a perfect square), the last the largest with 32-bit parts:
/// (m² − 1, 2m) for m = 46,340.
const PYTHAGOREAN: [(i32, i32); 6] = [
    (3, 4),
    (-5, 12),
    (8, -15),
    (-20, -21),
    (119, 120),
    (2_147_395_599, 92_680),
];

/// Directions whose length √(c² + s²) is irrational, the last two at the edge of `i32`.
const IRRATIONAL: [(i32, i32); 6] = [
    (1, 1),
    (1, 2),
    (-2, 3),
    (7, -3),
    (i32::MAX, 1),
    (i32::MIN, 1),
];

/// Scale factors as numerator and denominator, including the largest and smallest.
const SCALES: [(u32, u32); 7] = [
    (1, 1),
    (1, 2),
    (9_996, 10_000),
    (99_987_654, 100_000_000),
    (3, 7),
    (u32::MAX, 1),
    (1, u32::MAX),
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

/// 2^k as a double, for a normal power of two (−1022 ≤ k ≤ 1023), from its bits.
fn pow2(k: i32) -> f64 {
    f64::from_bits(u64::try_from(k + 1023).unwrap() << 52)
}

// ---------------------------------------------------------------------------------------------
// The canonical encoding. The hash, SHA-256, is `geom_test::golden`'s.
// ---------------------------------------------------------------------------------------------

/// The crate's operations the corpus calls. A call writes its discriminant first, so the byte
/// stream says which operation each result came from. The discriminants are part of the encoding:
/// never renumber them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Op {
    VecNew = 1,
    VecBetween = 2,
    VecDot = 3,
    VecCross = 4,
    VecTriple = 5,
    VecNorm2 = 6,
    VecAdd = 7,
    VecSub = 8,
    VecOffset = 9,
    VecNeg = 10,
    CrossDot = 11,
    FloatNew = 12,
    FloatAdd = 13,
    FloatSub = 14,
    FloatScale = 15,
    FloatDot = 16,
    FloatCross = 17,
    FloatLength = 18,
    FloatNormalized = 19,
    FloatFromGrid = 20,
    DdNew = 21,
    DdFromInt = 22,
    DdExactProduct = 23,
    DdAdd = 24,
    DdSub = 25,
    DdAddF64 = 26,
    DdMulF64 = 27,
    DdMul = 28,
    DdDiv = 29,
    DdSqrt = 30,
    DdRound = 31,
    GridNew = 32,
    GridApply = 33,
    GridApplyVector = 34,
    GridCompose = 35,
    GridInverse = 36,
    GridDeterminant = 37,
    RationalNew = 38,
    RationalApply = 39,
    RationalApplyVector = 40,
    RationalCompose = 41,
    RationalInverse = 42,
    RationalClassify = 43,
    RationalToGrid = 44,
    RationalToTransform = 45,
    TransformNew = 46,
    TransformApplyPoint = 47,
    TransformApplyVector = 48,
    TransformApplyGrid = 49,
    TransformCompose = 50,
    TransformClassify = 51,
    TransformInverse = 52,
    TypedRotation = 53,
    TypedApply = 54,
    TypedCompose = 55,
    TypedInverse = 56,
    TypedClassify = 57,
    TypedSimplified = 58,
    Box2Ops = 59,
    Box3Ops = 60,
    BoxTransformed = 61,
    FBoxNew = 62,
    FBoxOps = 63,
    FBoxTransformed = 64,
    FBoxToGrid = 65,
    FrameNew = 66,
    ToFrame = 67,
    ToMap = 68,
    ShiftTo = 69,
    ShiftApply = 70,
    HomeOf = 71,
    Owns = 72,
}

/// Writes every result into the hash in the canonical encoding, and keeps the tallies.
///
/// The encoding, every value little-endian, in this field order:
///
/// - a call: its `Op` discriminant as one byte, then its result;
/// - `bool` and `u8`: one byte; `i8`: its byte; integers otherwise their two's-complement bytes at
///   their own width (`u32` 4 bytes, `i64` and `u64` 8, `i128` 16);
/// - `f64`: its 8 IEEE 754 bytes, with any NaN as `CANONICAL_NAN`;
/// - `Option`: 0 for `None`, or 1 then the value;
/// - `Result`: 0 then the value, or the error;
/// - `NumError`: one byte, 1 out of range (then its `i128` value), 2 degenerate, 3 not finite,
///   4 wrong direction, 5 divisor contains zero, 6 negative square root, 7 empty interval,
///   8 outside a maths function's domain, 9 budget exceeded, 10 tolerance exceeded, 11 unproven
///   maths;
/// - `MathError`: 10 then the `NumError`; 11 singular; 12 then one byte for the frame mismatch
///   (1 CRS, 2 vertical CRS, 3 rotation, 4 scale, 5 not on the lattice); 13 then the `i128` value
///   of a frame range exceeded;
/// - a grid point or vector: x, y (and z) as `i64`; a cross product: x, y, z as `i128`; a
///   floating-point point: x, y (and z); a double-double: hi, then lo; a `Rounded`: the value,
///   then `exact`;
/// - a classification: one byte for the kind (0 identity, 1 translation, 2 rigid, 3 uniform
///   scale, 4 non-uniform, 5 degenerate), then `mirror`;
/// - a grid transform: per row, the source axis's index, the negation and the offset; a rational
///   transform: its matrix by rows, its translation numerators and its denominator; a general
///   transform: its rows `[l₀, l₁, l₂, t]`; a typed transform: one byte for the variant (0 exact,
///   1 rational, 2 certified), then the transform;
/// - a box: its lower corner, then its upper corner;
/// - a frame record: the CRS (0 EPSG, 1 local, then the `u32` code), the vertical CRS as an
///   `Option` of the same, the step's rung as one byte, the origin, the kind (0 map; 1 ground,
///   then the rotation's c and s as `i64` and the scale's numerator and denominator as `u32`),
///   and the lattice cell as an `Option` of its base and index (all `i64`);
/// - a frame shift: the source and target rungs (one byte each), the refinement and the offset;
///   a map point: easting, northing, height.
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
            op: Op::VecNew,
            seen: BTreeSet::new(),
            report: Vec::new(),
        }
    }

    /// Closes the current section under `name`, keeping its count and digest for the report.
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

    fn put_i8(&mut self, v: i8) {
        self.bytes(&v.to_le_bytes());
    }

    fn put_bool(&mut self, v: bool) {
        self.put_u8(u8::from(v));
    }

    fn put_u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
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

    fn num_error(&mut self, e: NumError) {
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

    fn math_error(&mut self, e: MathError) {
        match e {
            MathError::Num(n) => {
                self.put_u8(10);
                self.num_error(n);
            }
            MathError::Singular => {
                self.put_u8(11);
                self.note("Singular");
            }
            MathError::FrameMismatch(m) => {
                let (byte, class) = match m {
                    FrameMismatch::Crs => (1, "Crs"),
                    FrameMismatch::VerticalCrs => (2, "VerticalCrs"),
                    FrameMismatch::Rotation => (3, "Rotation"),
                    FrameMismatch::Scale => (4, "Scale"),
                    FrameMismatch::NotOnLattice => (5, "NotOnLattice"),
                };
                self.put_u8(12);
                self.put_u8(byte);
                self.note(class);
            }
            MathError::FrameRangeExceeded { value } => {
                self.put_u8(13);
                self.put_i128(value);
                self.note("FrameRangeExceeded");
            }
            MathError::NotInPlane => {
                self.put_u8(14);
                self.note("NotInPlane");
            }
        }
    }

    fn result<T>(&mut self, r: Result<T, NumError>, ok: impl FnOnce(&mut Recorder, T)) {
        match r {
            Ok(v) => {
                self.put_u8(0);
                self.note("Ok");
                ok(self, v);
            }
            Err(e) => self.num_error(e),
        }
    }

    fn math_result<T>(&mut self, r: Result<T, MathError>, ok: impl FnOnce(&mut Recorder, T)) {
        match r {
            Ok(v) => {
                self.put_u8(0);
                self.note("Ok");
                ok(self, v);
            }
            Err(e) => self.math_error(e),
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

    fn exact(&mut self, exact: bool) {
        self.put_bool(exact);
        self.note(if exact { "exact" } else { "inexact" });
    }

    fn point(&mut self, p: P3) {
        self.put_i64(p.x.get());
        self.put_i64(p.y.get());
        self.put_i64(p.z.get());
    }

    fn point2(&mut self, p: P2) {
        self.put_i64(p.x.get());
        self.put_i64(p.y.get());
    }

    fn rounded_point(&mut self, r: Rounded<P3>) {
        self.point(r.value());
        self.exact(r.is_exact());
    }

    fn v2(&mut self, v: V2) {
        self.put_i64(v.x());
        self.put_i64(v.y());
    }

    fn v3(&mut self, v: V3) {
        for c in v.to_array() {
            self.put_i64(c);
        }
    }

    fn w3(&mut self, w: W3) {
        self.put_i128(w.x());
        self.put_i128(w.y());
        self.put_i128(w.z());
        self.note(if w.is_zero() { "parallel" } else { "crossing" });
    }

    /// An exact integer whose sign the operation decides, noting the sign.
    fn signed(&mut self, v: i128) {
        self.put_i128(v);
        self.note(match v.signum() {
            -1 => "Negative",
            0 => "Zero",
            _ => "Positive",
        });
    }

    fn f2(&mut self, v: F2) {
        self.put_f64(v.x());
        self.put_f64(v.y());
    }

    fn f3(&mut self, v: F3) {
        for c in v.to_array() {
            self.put_f64(c);
        }
    }

    fn dd(&mut self, d: DD) {
        self.put_f64(d.hi());
        self.put_f64(d.lo());
    }

    fn classification(&mut self, c: Classification) {
        let (byte, class) = match c.kind {
            TransformKind::Identity => (0, "Identity"),
            TransformKind::Translation => (1, "Translation"),
            TransformKind::Rigid => (2, "Rigid"),
            TransformKind::UniformScale => (3, "UniformScale"),
            TransformKind::NonUniform => (4, "NonUniform"),
            TransformKind::Degenerate => (5, "Degenerate"),
        };
        self.put_u8(byte);
        self.note(class);
        self.put_bool(c.mirror);
        if c.mirror {
            self.note("mirror");
        }
    }

    fn grid(&mut self, t: &GridTransform) {
        let (source, negate, offset) = (t.source(), t.negate(), t.translation_part());
        for i in 0..3 {
            self.put_u8(source[i].index());
            self.put_bool(negate[i]);
            self.put_i64(offset[i]);
        }
    }

    fn rational(&mut self, t: &RationalTransform) {
        for row in t.matrix() {
            for v in row {
                self.put_i64(v);
            }
        }
        for v in t.translation_part() {
            self.put_i128(v);
        }
        self.put_i64(t.denominator());
    }

    fn rows(&mut self, t: &Transform) {
        for row in t.rows() {
            for v in row {
                self.put_f64(v);
            }
        }
    }

    fn typed(&mut self, t: &TypedTransform) {
        match t {
            TypedTransform::Exact(g) => {
                self.put_u8(0);
                self.note("Exact");
                self.grid(g);
            }
            TypedTransform::Rational(r) => {
                self.put_u8(1);
                self.note("Rational");
                self.rational(r);
            }
            TypedTransform::Certified(c) => {
                self.put_u8(2);
                self.note("Certified");
                self.rows(c);
            }
            // Piece G46's variant; this corpus never makes one, so its digest is unchanged.
            TypedTransform::Rotation(r) => {
                self.put_u8(3);
                self.note("Rotation");
                for v in r.quaternion() {
                    self.put_i64(v);
                }
            }
        }
    }

    fn box2(&mut self, b: Box2) {
        self.point2(b.min());
        self.point2(b.max());
    }

    fn box3(&mut self, b: Box3) {
        self.point(b.min());
        self.point(b.max());
    }

    fn fbox(&mut self, b: FBox3) {
        self.f3(b.lo());
        self.f3(b.hi());
    }

    fn crs(&mut self, c: CrsId) {
        match c {
            CrsId::Epsg(code) => {
                self.put_u8(0);
                self.put_u32(code);
            }
            CrsId::Local(code) => {
                self.put_u8(1);
                self.put_u32(code);
            }
        }
    }

    fn frame(&mut self, f: &FrameRecord) {
        self.crs(f.crs());
        self.option(f.vertical_crs(), Recorder::crs);
        self.put_u8(u8::try_from(f.step().rung()).unwrap());
        for o in f.origin() {
            self.put_i64(o);
        }
        match f.kind() {
            FrameKind::Map => {
                self.put_u8(0);
                self.note("Map");
            }
            FrameKind::Ground { rotation, scale } => {
                self.put_u8(1);
                self.note("Ground");
                self.put_i64(i64::from(rotation.cos()));
                self.put_i64(i64::from(rotation.sin()));
                self.put_u32(scale.num());
                self.put_u32(scale.den());
            }
        }
        self.option(f.lattice_cell(), |r, cell| {
            for v in cell.base() {
                r.put_i64(v);
            }
            for v in cell.index() {
                r.put_i64(v);
            }
        });
    }

    fn shift(&mut self, s: FrameShift) {
        self.put_u8(u8::try_from(s.source_step().rung()).unwrap());
        self.put_u8(u8::try_from(s.target_step().rung()).unwrap());
        self.put_i64(s.refine());
        for v in s.offset() {
            self.put_i64(v);
        }
    }

    fn map_point(&mut self, m: MapPoint) {
        self.put_f64(m.easting);
        self.put_f64(m.northing);
        self.put_f64(m.height);
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

    /// True or false, evenly.
    fn coin(&mut self) -> bool {
        self.one_in(2)
    }

    /// A number in lo..=hi (hi − lo below 2^63).
    fn int(&mut self, lo: i64, hi: i64) -> i64 {
        let span = u64::try_from(hi - lo).unwrap() + 1;
        lo + i64::try_from(self.below(span)).unwrap()
    }

    /// One of `items`.
    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        let n = u64::try_from(items.len()).unwrap();
        items[usize::try_from(self.below(n)).unwrap()]
    }

    /// A random `i64` with a random number of significant bits.
    fn int64(&mut self) -> i64 {
        let shift = u32::try_from(self.below(64)).unwrap();
        (self.bits() as i64) >> shift
    }

    /// A random `i128` with a random number of significant bits.
    fn int128(&mut self) -> i128 {
        let raw = (u128::from(self.bits()) << 64) | u128::from(self.bits());
        let shift = u32::try_from(self.below(128)).unwrap();
        (raw as i128) >> shift
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

    /// A grid coordinate: at an edge, or at a scale of its own (small scales make exact zeros,
    /// ties and coplanar sets common; the largest fills the frame).
    fn coord(&mut self) -> i64 {
        if self.one_in(4) {
            self.extreme()
        } else {
            let s = self.pick(&[3, 1 << 10, 1 << 20, 1 << 30, 1 << 39, M]);
            self.int(-s, s)
        }
    }

    fn point3(&mut self) -> P3 {
        P3::new(self.coord(), self.coord(), self.coord()).unwrap()
    }

    fn point2(&mut self) -> P2 {
        P2::new(self.coord(), self.coord()).unwrap()
    }

    /// A component for a grid vector's constructor: inside the vector range, at its edge or past
    /// it.
    fn vec_comp(&mut self) -> i64 {
        if self.one_in(5) {
            self.pick(&[
                VEC_MAX,
                -VEC_MAX,
                VEC_MAX + 1,
                -VEC_MAX - 1,
                i64::MAX,
                i64::MIN,
            ])
        } else {
            self.int(-VEC_MAX, VEC_MAX)
        }
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

    /// A double for vector arithmetic: mostly moderate, sometimes anything, sometimes zero.
    fn component(&mut self) -> f64 {
        match self.below(8) {
            0 => self.pick(&[0.0, -0.0]),
            1 | 2 => self.any(),
            _ => self.double(-60, 60),
        }
    }

    /// A normalized double-double: a leading part of any exponent and a trailing part up to half
    /// an ulp of it, or zero.
    fn dd(&mut self) -> DD {
        let hi = if self.one_in(3) {
            self.double(-80, 80)
        } else {
            self.any()
        };
        if hi == 0.0 || self.one_in(4) {
            return DD::from_f64(hi).unwrap();
        }
        // An exponent 54 or more below the leading part's, never below the subnormal range.
        let e = i64::from(((hi.to_bits() >> 52) & 0x7ff) as i32) - 1023;
        let lo_top = (e - 54).max(-1074);
        let lo = self.double((lo_top - 50).max(-1074), lo_top);
        DD::from_sum(hi, lo).unwrap_or(DD::from_f64(hi).unwrap())
    }

    /// An entry of a general transform: zero, ±1, a half, a small integer, a moderate double, an
    /// entry of any exponent, or a subnormal.
    fn entry(&mut self) -> f64 {
        match self.below(10) {
            0 | 1 => self.pick(&[0.0, 1.0, -1.0, 0.5, -0.5, 2.0, 0.25]),
            2 => self.pick(&[pow2(900), -pow2(900), pow2(-900), f64::from_bits(1)]),
            3 => f64::from(i32::try_from(self.int(-9, 9)).unwrap()),
            4 => self.double(-1074, 1023),
            _ => self.double(-8, 2),
        }
    }
}

/// `x` moved `k` ulps up (k > 0) or down (k < 0).
fn nudge(x: f64, k: i64) -> f64 {
    let mut v = x;
    for _ in 0..k.unsigned_abs() {
        v = if k > 0 { v.next_up() } else { v.next_down() };
    }
    v
}

// ---------------------------------------------------------------------------------------------
// The corpus, section by section.
// ---------------------------------------------------------------------------------------------

/// Grid vectors in 3D and 2D.
fn vectors(g: &mut Gen, r: &mut Recorder) {
    for _ in 0..800 {
        let (p, q, s, t) = (g.point3(), g.point3(), g.point3(), g.point3());
        let a = V3::between(p, q).unwrap();
        // Sometimes parallel to a (the cross product is zero) or perpendicular to it (the dot
        // product is zero), usually independent.
        let b = match g.below(8) {
            0 => a.neg().unwrap(),
            1 => V3::new(-a.y(), a.x(), 0).unwrap(),
            _ => V3::between(s, t).unwrap(),
        };
        let c = V3::between(q, t).unwrap();
        r.call(Op::VecBetween);
        r.v3(a);
        r.call(Op::VecDot);
        r.signed(a.dot(b).unwrap());
        r.call(Op::VecCross);
        r.w3(a.cross(b).unwrap());
        r.call(Op::VecTriple);
        r.signed(a.triple(b, c).unwrap());
        r.call(Op::CrossDot);
        r.signed(a.cross(b).unwrap().dot(c).unwrap());
        r.call(Op::VecNorm2);
        r.put_i128(a.norm2().unwrap());
        r.call(Op::VecAdd);
        r.result(a.add(b), Recorder::v3);
        r.call(Op::VecSub);
        r.result(a.sub(b), Recorder::v3);
        r.call(Op::VecOffset);
        r.result(a.offset(s), Recorder::point);
        r.call(Op::VecNeg);
        r.v3(a.neg().unwrap());
        r.call(Op::VecNew);
        r.result(
            V3::new(g.vec_comp(), g.vec_comp(), g.vec_comp()),
            Recorder::v3,
        );

        let (p2, q2, s2, t2) = (g.point2(), g.point2(), g.point2(), g.point2());
        let a2 = V2::between(p2, q2).unwrap();
        let b2 = match g.below(8) {
            0 => a2.neg().unwrap(),
            1 => V2::new(-a2.y(), a2.x()).unwrap(),
            _ => V2::between(s2, t2).unwrap(),
        };
        r.call(Op::VecBetween);
        r.v2(a2);
        r.call(Op::VecDot);
        r.signed(a2.dot(b2).unwrap());
        r.call(Op::VecCross);
        r.signed(a2.cross(b2).unwrap());
        r.call(Op::VecNorm2);
        r.put_i128(a2.norm2().unwrap());
        r.call(Op::VecAdd);
        r.result(a2.add(b2), Recorder::v2);
        r.call(Op::VecSub);
        r.result(a2.sub(b2), Recorder::v2);
        r.call(Op::VecOffset);
        r.result(a2.offset(s2), Recorder::point2);
        r.call(Op::VecNeg);
        r.v2(a2.neg().unwrap());
        r.call(Op::VecNew);
        r.result(V2::new(g.vec_comp(), g.vec_comp()), Recorder::v2);
    }
}

/// Floating-point vectors in 3D and 2D.
fn floats(g: &mut Gen, r: &mut Recorder) {
    for _ in 0..800 {
        let a = F3::new(g.component(), g.component(), g.component()).unwrap();
        let b = F3::new(g.component(), g.component(), g.component()).unwrap();
        let k = g.component();
        r.call(Op::FloatAdd);
        r.result(a.add(b), Recorder::f3);
        r.call(Op::FloatSub);
        r.result(a.sub(b), Recorder::f3);
        r.call(Op::FloatScale);
        r.result(a.scale(k), Recorder::f3);
        r.call(Op::FloatDot);
        r.result(a.dot(b), Recorder::put_f64);
        r.call(Op::FloatCross);
        r.result(a.cross(b), Recorder::f3);
        r.call(Op::FloatLength);
        r.result(a.norm2(), Recorder::put_f64);
        r.call(Op::FloatLength);
        r.result(a.length(), Recorder::put_f64);
        r.call(Op::FloatNormalized);
        r.result(a.normalized(), Recorder::f3);
        r.call(Op::FloatNew);
        let bad = g.pick(&[f64::INFINITY, f64::NEG_INFINITY, f64::NAN, 1.0]);
        r.result(F3::new(g.component(), bad, g.component()), Recorder::f3);

        let c = F2::new(g.component(), g.component()).unwrap();
        let d = F2::new(g.component(), g.component()).unwrap();
        r.call(Op::FloatAdd);
        r.result(c.add(d), Recorder::f2);
        r.call(Op::FloatSub);
        r.result(c.sub(d), Recorder::f2);
        r.call(Op::FloatScale);
        r.result(c.scale(k), Recorder::f2);
        r.call(Op::FloatDot);
        r.result(c.dot(d), Recorder::put_f64);
        r.call(Op::FloatCross);
        r.result(c.cross(d), Recorder::put_f64);
        r.call(Op::FloatLength);
        r.result(c.length(), Recorder::put_f64);
        r.call(Op::FloatNormalized);
        r.result(c.normalized(), Recorder::f2);
        r.call(Op::FloatNew);
        r.result(F2::new(bad, g.component()), Recorder::f2);

        let (p, q) = (g.point3(), g.point3());
        r.call(Op::FloatFromGrid);
        r.f3(F3::from(p));
        r.f3(F3::from(V3::between(p, q).unwrap()));
        let (p2, q2) = (g.point2(), g.point2());
        r.f2(F2::from(p2));
        r.f2(F2::from(V2::between(p2, q2).unwrap()));
    }
}

/// Double-double arithmetic.
fn double_double(g: &mut Gen, r: &mut Recorder) {
    for _ in 0..1500 {
        let (a, b, y) = (g.dd(), g.dd(), g.any());
        r.call(Op::DdAdd);
        r.result(a.add(b), Recorder::dd);
        r.call(Op::DdSub);
        r.result(a.sub(b), Recorder::dd);
        r.call(Op::DdAddF64);
        r.result(a.add_f64(y), Recorder::dd);
        r.call(Op::DdMulF64);
        r.result(a.mul_f64(y), Recorder::dd);
        r.call(Op::DdMul);
        r.result(a.mul(b), Recorder::dd);
        r.call(Op::DdDiv);
        let divisor = if g.one_in(16) { DD::ZERO } else { b };
        r.result(a.div(divisor), Recorder::dd);
        r.call(Op::DdSqrt);
        r.result(a.sqrt(), Recorder::dd);
        // Nearly cancelling sums.
        r.call(Op::DdAdd);
        let near = (-a).add_f64(a.hi() * pow2(-60)).unwrap_or(-a);
        r.result(a.add(near), Recorder::dd);

        r.call(Op::DdNew);
        let finite = g.any();
        let x = g.pick(&[finite, f64::INFINITY, f64::NAN, f64::MAX]);
        r.result(DD::from_sum(x, g.any()), Recorder::dd);
        r.call(Op::DdNew);
        r.result(DD::from_f64(x), Recorder::dd);
        r.call(Op::DdFromInt);
        r.dd(DD::from_i64(g.int64()));
        r.call(Op::DdFromInt);
        let w = if g.one_in(4) {
            g.pick(&[
                (1i128 << 106) - 1,
                -(1i128 << 106) + 1,
                1i128 << 106,
                i128::MIN,
            ])
        } else {
            g.int128()
        };
        r.option(DD::from_i128(w), Recorder::dd);
        r.call(Op::DdExactProduct);
        r.option(DD::exact_product(g.any(), g.any()), Recorder::dd);

        // Rounding to integers: whole numbers, ties and near-ties, with offsets.
        let n = g.int(-(1 << 52), 1 << 52) as f64;
        let frac = g.pick(&[0.0, 0.5, -0.5, 0.25, 0.75]);
        let tail = g.pick(&[0.0, pow2(-70), -pow2(-70)]);
        let v = DD::from_sum(n + frac, tail).unwrap();
        let offset = if g.coin() {
            i128::from(g.int(-(1 << 61), 1 << 61))
        } else {
            0
        };
        r.call(Op::DdRound);
        r.option(v.round_shifted(offset), |r, x| {
            r.put_i128(x.value());
            r.exact(x.is_exact());
        });
        r.call(Op::DdRound);
        r.option(a.round_shifted(offset), |r, x| {
            r.put_i128(x.value());
            r.exact(x.is_exact());
        });
    }
}

/// A grid transform's translation component: zero, small, at the limit or past it, or anywhere.
fn grid_translation(g: &mut Gen) -> i64 {
    match g.below(6) {
        0 => 0,
        1 => g.int(-1000, 1000),
        2 => g.pick(&[
            TRANSLATION_MAX,
            -TRANSLATION_MAX,
            TRANSLATION_MAX + 1,
            -TRANSLATION_MAX - 1,
        ]),
        _ => g.int(-TRANSLATION_MAX, TRANSLATION_MAX),
    }
}

/// A grid transform from its parts, or `None` if the parts are refused (which is recorded).
fn grid_transform(g: &mut Gen, r: &mut Recorder) -> Option<GridTransform> {
    let mut source = g.pick(&PERMUTATIONS);
    if g.one_in(12) {
        source[2] = source[0];
    }
    let negate = [g.coin(), g.coin(), g.coin()];
    let t = [
        grid_translation(g),
        grid_translation(g),
        grid_translation(g),
    ];
    let made = GridTransform::from_parts(source, negate, t);
    r.call(Op::GridNew);
    r.result(made, |r, t| r.grid(&t));
    made.ok()
}

/// A simple grid transform: a quarter turn, a mirror or a small translation.
fn simple_grid(g: &mut Gen) -> GridTransform {
    let axis = g.pick(&Axis::ALL);
    match g.below(3) {
        0 => GridTransform::quarter_turns(axis, i32::try_from(g.int(-5, 5)).unwrap()),
        1 => GridTransform::mirror(axis),
        _ => GridTransform::translation([g.int(-9, 9), g.int(-9, 9), g.int(-9, 9)]).unwrap(),
    }
}

/// Grid transforms.
fn grid_transforms(g: &mut Gen, r: &mut Recorder) {
    for _ in 0..800 {
        let Some(t) = grid_transform(g, r) else {
            continue;
        };
        r.call(Op::GridDeterminant);
        r.put_i8(t.determinant());
        r.put_bool(t.reverses_orientation());
        r.put_bool(t.is_identity());
        for row in t.matrix() {
            for v in row {
                r.put_i8(v);
            }
        }
        if t.reverses_orientation() {
            r.note("mirror");
        }
        let p = g.point3();
        r.call(Op::GridApply);
        r.result(t.apply(p), Recorder::point);
        let v = V3::between(g.point3(), g.point3()).unwrap();
        r.call(Op::GridApplyVector);
        r.v3(t.apply_vector(v).unwrap());
        let other = if g.coin() {
            simple_grid(g)
        } else {
            grid_transform(g, r).unwrap_or(GridTransform::IDENTITY)
        };
        r.call(Op::GridCompose);
        r.result(t.compose(&other), |r, c| r.grid(&c));
        r.call(Op::GridInverse);
        let inv = t.inverse().unwrap();
        r.grid(&inv);
        r.result(inv.compose(&t), |r, c| r.put_bool(c.is_identity()));
        let b = Box3::from_points([g.point3(), g.point3()]).unwrap();
        r.call(Op::BoxTransformed);
        r.result(b.transformed(&t), Recorder::box3);
    }
}

/// A rational transform of one of the kinds the crate makes, or `None` if refused (recorded).
fn rational_transform(g: &mut Gen, r: &mut Recorder) -> Option<RationalTransform> {
    let axis = g.pick(&Axis::ALL);
    let made = match g.below(6) {
        0 => {
            let (c, s) = g.pick(&PYTHAGOREAN);
            RationalTransform::rotation(axis, Rotation::direction(c, s).unwrap())
        }
        1 => {
            let (n, d) = g.pick(&SCALES);
            RationalTransform::uniform_scale(Scale::new(n, d).unwrap())
        }
        2 => {
            let d = g.pick(&[1, 2, 3, 7, 1024, 1 << 40, (1 << 40) + 1]);
            let t = [0; 3].map(|_| i128::from(g.int(-(1 << 50), 1 << 50)));
            RationalTransform::translation(t, d)
        }
        3 => {
            let m = [[0; 3]; 3].map(|row: [i64; 3]| row.map(|_| g.int(-8, 8)));
            let t = [0; 3].map(|_| i128::from(g.int(-(1 << 44), 1 << 44)));
            RationalTransform::new(m, t, g.int(-12, 12))
        }
        4 => Ok(RationalTransform::from_grid(&simple_grid(g))),
        _ => {
            let big = g.pick(&[1 << 40, (1 << 40) + 1, i64::MAX]);
            RationalTransform::new([[big, 0, 0], [0, 1, 0], [0, 0, 1]], [0; 3], 1)
        }
    };
    r.call(Op::RationalNew);
    r.result(made, |r, t| r.rational(&t));
    made.ok()
}

/// Rational transforms.
fn rationals(g: &mut Gen, r: &mut Recorder) {
    for _ in 0..600 {
        let Some(t) = rational_transform(g, r) else {
            continue;
        };
        let p = g.point3();
        r.call(Op::RationalApply);
        r.result(t.apply(p), Recorder::rounded_point);
        let v = V3::between(g.point3(), g.point3()).unwrap();
        r.call(Op::RationalApplyVector);
        r.result(t.apply_vector(v), |r, x| {
            r.v3(x.value());
            r.exact(x.is_exact());
        });
        if let Some(u) = rational_transform(g, r) {
            r.call(Op::RationalCompose);
            r.result(t.compose(&u), |r, c| r.rational(&c));
        }
        r.call(Op::RationalInverse);
        r.math_result(t.inverse(), |r, i| r.rational(&i));
        r.call(Op::RationalClassify);
        r.classification(t.classify().unwrap());
        r.call(Op::RationalToGrid);
        r.option(t.to_grid(), |r, x| r.grid(&x));
        r.call(Op::RationalToTransform);
        r.rows(&t.to_transform().unwrap());
    }
    // A singular matrix has no inverse.
    let singular = RationalTransform::new([[1, 2, 3], [2, 4, 6], [0, 0, 1]], [0; 3], 1).unwrap();
    r.call(Op::RationalInverse);
    r.math_result(singular.inverse(), |r, i| r.rational(&i));
    r.call(Op::RationalClassify);
    r.classification(singular.classify().unwrap());
}

/// A general transform of one of the kinds the corpus needs, or `None` if refused (recorded).
fn general_transform(g: &mut Gen, r: &mut Recorder) -> Option<Transform> {
    let axis = g.pick(&Axis::ALL);
    let made = match g.below(9) {
        0 => {
            let turns = if g.coin() {
                f64::from(i32::try_from(g.int(-8, 8)).unwrap()) * 0.5
            } else {
                g.double(-12, 3)
            };
            Transform::rotation(axis, turns)
        }
        1 => Transform::scale(g.entry(), g.entry(), g.entry()),
        2 => Transform::uniform_scale(g.pick(&[2.0, 0.5, 1e103, 1e-110, -3.0, 1.0 + f64::EPSILON])),
        3 => Ok(Transform::mirror(axis)),
        4 => Ok(Transform::from_grid(&simple_grid(g))),
        5 => Ok(Transform::translation(
            F3::new(g.entry(), g.entry(), g.entry()).unwrap(),
        )),
        // Entries one ulp from one: the floating-point filter of `apply_grid` cannot decide grid
        // images near the frame's edge, the double-double stage can.
        6 => Transform::from_rows([
            [1.0 + f64::EPSILON, 0.0, 0.0, 0.0],
            [0.0, 1.0 - f64::EPSILON / 2.0, 0.0, 0.5],
            [0.0, 0.0, 1.0, g.pick(&[0.0, 0.5, -0.25])],
        ]),
        // Huge entries that cancel, and subnormal ones: the exact stage.
        7 => {
            let big = g.pick(&[pow2(900), pow2(1000), pow2(500)]);
            Transform::from_rows([
                [big, -big, 0.5, 0.0],
                [f64::from_bits(1), 0.0, 0.0, 2.5],
                [0.0, 0.0, 1.0, g.entry()],
            ])
        }
        _ => {
            let row = |g: &mut Gen| [g.entry(), g.entry(), g.entry(), g.entry()];
            let rows = [row(g), row(g), row(g)];
            if g.one_in(10) {
                // A non-finite entry is refused.
                let mut bad = rows;
                bad[1][2] = f64::INFINITY;
                Transform::from_rows(bad)
            } else {
                Transform::from_rows(rows)
            }
        }
    };
    r.call(Op::TransformNew);
    r.result(made, |r, t| r.rows(&t));
    made.ok()
}

/// A grid point for `apply_grid`: a general one, or one whose first two coordinates are equal
/// (so the cancelling entries cancel).
fn grid_point_for(g: &mut Gen) -> P3 {
    if g.coin() {
        g.point3()
    } else {
        let x = g.coord();
        P3::new(x, x, g.coord()).unwrap()
    }
}

/// General transforms.
fn general_transforms(g: &mut Gen, r: &mut Recorder) {
    for _ in 0..800 {
        let Some(t) = general_transform(g, r) else {
            continue;
        };
        let p = F3::new(g.component(), g.component(), g.component()).unwrap();
        r.call(Op::TransformApplyPoint);
        r.result(t.apply_point(p), Recorder::f3);
        r.call(Op::TransformApplyVector);
        r.result(t.apply_vector(p), Recorder::f3);
        for _ in 0..3 {
            let q = grid_point_for(g);
            r.call(Op::TransformApplyGrid);
            r.result(t.apply_grid(q), Recorder::rounded_point);
        }
        r.call(Op::TransformClassify);
        r.classification(t.classify().unwrap());
        r.call(Op::TransformInverse);
        r.math_result(t.inverse(), |r, i| r.rows(&i));
        if let Some(u) = general_transform(g, r) {
            r.call(Op::TransformCompose);
            r.result(t.compose(&u), |r, c| r.rows(&c));
        }
    }
    // Shears and singular maps.
    let shapes = [
        [
            [1.0, 0.5, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
        [
            [1.0, 2.0, 3.0, 0.0],
            [2.0, 4.0, 6.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
        [[0.0; 4], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]],
        [
            [1.0, 1.0, 0.0, 0.0],
            [1.0, 1.0 + f64::EPSILON, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
    ];
    for rows in shapes {
        let t = Transform::from_rows(rows).unwrap();
        r.call(Op::TransformClassify);
        r.classification(t.classify().unwrap());
        r.call(Op::TransformInverse);
        r.math_result(t.inverse(), |r, i| r.rows(&i));
    }
}

/// A typed transform of any variant.
fn typed_transform(g: &mut Gen, r: &mut Recorder) -> TypedTransform {
    let axis = g.pick(&Axis::ALL);
    let rotation = match g.below(3) {
        0 => Rotation::quarter_turns(i32::try_from(g.int(-4, 4)).unwrap()),
        1 => {
            let (c, s) = g.pick(&PYTHAGOREAN);
            Rotation::direction(c, s).unwrap()
        }
        _ => {
            let (c, s) = g.pick(&IRRATIONAL);
            Rotation::direction(c, s).unwrap()
        }
    };
    let made = TypedTransform::rotation(axis, rotation);
    r.call(Op::TypedRotation);
    r.result(made, |r, t| r.typed(&t));
    made.unwrap_or(TypedTransform::IDENTITY)
}

/// Typed transforms.
fn typed_transforms(g: &mut Gen, r: &mut Recorder) {
    for _ in 0..500 {
        let a = typed_transform(g, r);
        let b = match g.below(3) {
            0 => TypedTransform::from(simple_grid(g)),
            1 => typed_transform(g, r),
            _ => TypedTransform::from(
                RationalTransform::translation([1, -3, 5], g.pick(&[1, 2, 4])).unwrap(),
            ),
        };
        r.call(Op::TypedCompose);
        r.math_result(a.compose(&b), |r, c| r.typed(&c));
        r.call(Op::TypedInverse);
        r.math_result(a.inverse(), |r, c| r.typed(&c));
        r.call(Op::TypedClassify);
        r.classification(a.classify().unwrap());
        r.put_bool(a.is_exact());
        r.rows(&a.to_transform().unwrap());
        let p = g.point3();
        r.call(Op::TypedApply);
        r.result(a.apply(p), Recorder::rounded_point);
        r.call(Op::TypedApply);
        r.result(b.apply(p), Recorder::rounded_point);
        // Simplification back to the exact variant.
        let grid = simple_grid(g);
        r.call(Op::TypedSimplified);
        r.typed(&TypedTransform::Rational(RationalTransform::from_grid(&grid)).simplified());
        r.typed(&TypedTransform::Certified(Transform::from_grid(&grid)).simplified());
    }
}

/// Grid and floating-point boxes.
fn boxes(g: &mut Gen, r: &mut Recorder) {
    for _ in 0..600 {
        let a = Box3::from_points([g.point3(), g.point3()]).unwrap();
        let b = Box3::from_points([g.point3(), g.point3(), g.point3()]).unwrap();
        let p = g.point3();
        r.call(Op::Box3Ops);
        r.box3(a.union(&b));
        r.option(a.intersection(&b), Recorder::box3);
        r.put_bool(a.contains(p));
        r.put_bool(a.contains_box(&b));
        r.put_bool(a.overlaps(&b));
        r.v3(a.extent().unwrap());
        r.call(Op::Box3Ops);
        r.result(Box3::new(g.point3(), g.point3()), Recorder::box3);

        let a2 = Box2::from_points([g.point2(), g.point2()]).unwrap();
        let b2 = Box2::from_points([g.point2(), g.point2()]).unwrap();
        r.call(Op::Box2Ops);
        r.box2(a2.union(&b2));
        r.option(a2.intersection(&b2), Recorder::box2);
        r.put_bool(a2.contains(g.point2()));
        r.put_bool(a2.contains_box(&b2));
        r.put_bool(a2.overlaps(&b2));
        r.v2(a2.extent().unwrap());
        r.call(Op::Box2Ops);
        r.result(Box2::new(g.point2(), g.point2()), Recorder::box2);

        let fa = FBox3::from_box3(&a);
        let lo = F3::new(g.component(), g.component(), g.component()).unwrap();
        let hi = F3::new(g.component(), g.component(), g.component()).unwrap();
        r.call(Op::FBoxNew);
        let made = FBox3::new(lo, hi);
        r.result(made, Recorder::fbox);
        let fb = made.unwrap_or(FBox3::from_point(lo));
        r.call(Op::FBoxOps);
        r.fbox(fa.union(&fb));
        r.option(fa.intersection(&fb), Recorder::fbox);
        r.put_bool(fa.contains(lo));
        r.put_bool(fa.overlaps(&fb));
        if let Some(t) = general_transform(g, r) {
            r.call(Op::FBoxTransformed);
            let moved = fa.transformed(&t);
            r.result(moved, Recorder::fbox);
            if let Ok(m) = moved {
                r.call(Op::FBoxToGrid);
                r.result(m.to_grid_outward(), Recorder::box3);
            }
        }
        r.call(Op::FBoxToGrid);
        r.result(fb.to_grid_outward(), Recorder::box3);
    }
}

/// A frame record's inputs, with origins at the site (central Auckland in NZTM2000), anywhere,
/// at the origin bound or past it, or at zero.
fn frame_record(g: &mut Gen, r: &mut Recorder) -> Option<FrameRecord> {
    let crs = g.pick(&[CrsId::Epsg(2193), CrsId::Local(7)]);
    let step = g.pick(&STEPS);
    let s = i64::try_from(steps_per_metre(step)).unwrap();
    let origin = match g.below(5) {
        0 | 1 => [1_757_000 * s, 5_921_000 * s, 30 * s],
        2 => [
            g.int(-(1 << 50), 1 << 50),
            g.int(-(1 << 50), 1 << 50),
            g.int(-(1 << 40), 1 << 40),
        ],
        3 => [
            g.pick(&[ORIGIN_MAX, -ORIGIN_MAX, ORIGIN_MAX - 1, ORIGIN_MAX + 1]),
            g.int(-1000, 1000),
            0,
        ],
        _ => [0; 3],
    };
    let rotation = match g.below(4) {
        0 => Rotation::IDENTITY,
        1 => Rotation::quarter_turns(i32::try_from(g.int(-4, 4)).unwrap()),
        2 => {
            let (c, sn) = g.pick(&PYTHAGOREAN);
            Rotation::direction(c, sn).unwrap()
        }
        _ => {
            let (c, sn) = g.pick(&IRRATIONAL);
            Rotation::direction(c, sn).unwrap()
        }
    };
    let scale = if g.coin() {
        Scale::ONE
    } else {
        let (n, d) = g.pick(&SCALES);
        Scale::new(n, d).unwrap()
    };
    let made = FrameRecord::new(crs, step, origin, rotation, scale);
    r.call(Op::FrameNew);
    r.result(made, |r, f| r.frame(&f));
    made.ok()
}

/// Map points that stress the conversion into a frame `f`.
fn map_points(g: &mut Gen, f: &FrameRecord) -> Vec<MapPoint> {
    let s_int = i64::try_from(steps_per_metre(f.step())).unwrap();
    let s = s_int as f64;
    let [oe, on, oh] = f.origin();
    let (be, bn, bh) = (oe as f64 / s, on as f64 / s, oh as f64 / s);
    let mut out = Vec::new();
    // Offsets of every size from the origin, past the frame's range at the largest.
    for _ in 0..2 {
        out.push(MapPoint {
            easting: be + g.double(-40, 19),
            northing: bn + g.double(-40, 19),
            height: bh + g.double(-40, 10),
        });
    }
    // Multiples of 2^−12 m: whole and half steps at every grid step (S = 2^11 times an odd
    // number), so exact results and ties.
    let dyadic = |g: &mut Gen, base: f64| {
        let j = (base * 4096.0) as i64 + g.int(-(1 << 24), 1 << 24);
        j as f64 / 4096.0
    };
    out.push(MapPoint {
        easting: dyadic(g, be),
        northing: dyadic(g, bn),
        height: dyadic(g, bh),
    });
    // The image of a grid point, a few ulps away.
    if let Ok(m) = f.to_map(g.point3()) {
        out.push(MapPoint {
            easting: nudge(m.value().easting, g.int(-3, 3)),
            northing: nudge(m.value().northing, g.int(-3, 3)),
            height: m.value().height,
        });
    }
    // On the frame's y axis (x = 0 exactly, even for an irrational direction): with t a multiple
    // of 2^−12 m, u = s·t·S and v = −c·t·S, so c·u + s·v = 0. Only when the origin is whole
    // metres, so that the sums below are exact.
    if oe % s_int == 0 && on % s_int == 0 {
        let rotation = f.rotation();
        let t = g.int(-64, 64) as f64 / 4096.0;
        out.push(MapPoint {
            easting: be + f64::from(rotation.sin()) * t,
            northing: bn - f64::from(rotation.cos()) * t,
            height: bh,
        });
    }
    // Non-finite, far and tiny values.
    out.push(MapPoint {
        easting: g.pick(&[f64::NAN, f64::INFINITY, pow2(63), -pow2(63)]),
        northing: bn,
        height: bh,
    });
    out.push(MapPoint {
        easting: f64::from_bits(1),
        northing: -0.0,
        height: pow2(-1000),
    });
    out
}

/// Frame records and conversions both ways.
fn frames(g: &mut Gen, r: &mut Recorder) {
    for _ in 0..300 {
        let Some(f) = frame_record(g, r) else {
            continue;
        };
        for m in map_points(g, &f) {
            r.call(Op::ToFrame);
            r.result(f.to_frame(m), Recorder::rounded_point);
        }
        // Grid points anywhere, and grid points a whole number of metres from the origin (exact
        // map values in a Map frame).
        let s = i64::try_from(steps_per_metre(f.step())).unwrap();
        // At most 200 m, which every step's range holds (q2 reaches ±275 m).
        let metres = |g: &mut Gen| s * g.int(-200, 200);
        let whole = P3::new(metres(g), metres(g), metres(g)).unwrap();
        for p in [g.point3(), g.point3(), whole] {
            r.call(Op::ToMap);
            r.result(f.to_map(p), |r, m| {
                r.map_point(m.value());
                r.exact(m.is_exact());
            });
        }
    }
    // An origin that nearly cancels the rotated offset: direction (1, 1), O_E = −p and the point
    // (q, −q) with p² − 2q² = ±1 (Pell), so the easting is (q√2 − p)/S, found by exact bisection.
    let (mut p, mut q) = (1i64, 1i64);
    while q + p <= COORD_MAX {
        let (np, nq) = (p + 2 * q, p + q);
        p = np;
        q = nq;
        let f = FrameRecord::new(
            CrsId::Local(1),
            GridStep::Q0,
            [-p, 0, 0],
            Rotation::direction(1, 1).unwrap(),
            Scale::ONE,
        )
        .unwrap();
        r.call(Op::ToMap);
        r.result(f.to_map(P3::new(q, -q, 0).unwrap()), |r, m| {
            r.map_point(m.value());
            r.exact(m.is_exact());
        });
    }
}

/// A lattice frame of a regional network, or `None` if refused (recorded).
fn lattice_frame(
    g: &mut Gen,
    r: &mut Recorder,
    crs: CrsId,
    step: GridStep,
    base: [i64; 3],
) -> Option<FrameRecord> {
    let index = [g.int(-3, 3), g.int(-3, 3)];
    let made = FrameRecord::lattice(crs, step, base, index);
    r.call(Op::FrameNew);
    r.result(made, |r, f| r.frame(&f));
    made.ok()
}

/// Exact shifts between frames, and lattice homes.
fn shifts(g: &mut Gen, r: &mut Recorder) {
    for _ in 0..300 {
        let crs = g.pick(&[CrsId::Epsg(2193), CrsId::Local(3)]);
        let base = [
            g.int(-(1 << 45), 1 << 45),
            g.int(-(1 << 45), 1 << 45),
            g.int(-1000, 1000),
        ];
        let step = g.pick(&STEPS);
        let Some(a) = lattice_frame(g, r, crs, step, base) else {
            continue;
        };
        // Another frame: on the same lattice, at another step, or differing in one property.
        let b = match g.below(8) {
            0..=2 => lattice_frame(g, r, crs, step, base),
            3 => FrameRecord::map(crs, g.pick(&STEPS), a.origin()).ok(),
            4 => FrameRecord::map(CrsId::Local(99), step, a.origin()).ok(),
            5 => Some(a.with_vertical_crs(Some(CrsId::Epsg(7839)))),
            6 => FrameRecord::new(
                crs,
                step,
                a.origin(),
                Rotation::direction(3, 4).unwrap(),
                Scale::ONE,
            )
            .ok(),
            _ => FrameRecord::new(
                crs,
                step,
                a.origin(),
                Rotation::IDENTITY,
                Scale::new(9_996, 10_000).unwrap(),
            )
            .ok(),
        };
        let Some(b) = b else {
            continue;
        };
        r.call(Op::ShiftTo);
        let shift = a.shift_to(&b);
        r.math_result(shift, Recorder::shift);
        if let Ok(sh) = shift {
            for _ in 0..2 {
                r.call(Op::ShiftApply);
                r.result(sh.apply(g.point3()), Recorder::point);
            }
        }
        // Ground frames on a shared lattice: origins that differ by k_n·R·t whole steps, or by
        // one step, which a Pythagorean rotation cannot carry.
        let (c, sn) = g.pick(&PYTHAGOREAN);
        let rotation = Rotation::direction(c, sn).unwrap();
        let (n, d) = g.pick(&SCALES);
        let scale = Scale::new(n, d).unwrap();
        let t = [g.int(-3, 3), g.int(-3, 3)];
        let (ci, si, kn) = (
            i128::from(rotation.cos()),
            i128::from(rotation.sin()),
            i128::from(scale.num()),
        );
        let (t0, t1) = (i128::from(t[0]), i128::from(t[1]));
        // The target's origin, −ΔO, in `i128` (below 2^67), when it fits an `i64`.
        let delta = if g.coin() {
            [kn * (ci * t0 - si * t1), kn * (si * t0 + ci * t1)]
        } else {
            [1, 0]
        };
        let origin = [-delta[0], -delta[1]].map(|v| i64::try_from(v).ok());
        let ga = FrameRecord::new(crs, step, [0, 0, 0], rotation, scale).unwrap();
        if let [Some(oe), Some(on)] = origin
            && let Ok(gb) = FrameRecord::new(crs, step, [oe, on, 5], rotation, scale)
        {
            r.call(Op::ShiftTo);
            r.math_result(ga.shift_to(&gb), Recorder::shift);
        }
    }
    // Homes on a lattice.
    for _ in 0..200 {
        let base = [g.int(-(1 << 45), 1 << 45), g.int(-(1 << 45), 1 << 45), 0];
        let step = g.pick(&STEPS);
        let mut operands = Vec::new();
        for _ in 0..g.int(1, 3) {
            if let Some(f) = lattice_frame(g, r, CrsId::Epsg(2193), step, base) {
                let reach = g.pick(&[CELL_HALF / 2, CELL_HALF, 3 * CELL_HALF, M]);
                let corner = |g: &mut Gen| {
                    P3::new(g.int(-reach, reach), g.int(-reach, reach), g.int(-9, 9)).unwrap()
                };
                let b = Box3::from_points([corner(g), corner(g)]).unwrap();
                r.call(Op::Owns);
                r.put_bool(f.owns(b.min()));
                r.call(Op::HomeOf);
                r.math_result(f.home_frame(&b), |r, (home, sh)| {
                    r.frame(&home);
                    r.shift(sh);
                });
                operands.push((f, b));
            }
        }
        r.call(Op::HomeOf);
        r.math_result(FrameRecord::home_of(&operands), |r, f| r.frame(&f));
    }
    // No operands, and an operand that is not a lattice frame.
    r.call(Op::HomeOf);
    r.math_result(FrameRecord::home_of(&[]), |r, f| r.frame(&f));
    let plain = FrameRecord::map(CrsId::Epsg(2193), GridStep::Q0, [0; 3]).unwrap();
    let b = Box3::from_point(P3::new(0, 0, 0).unwrap());
    r.call(Op::HomeOf);
    r.math_result(FrameRecord::home_of(&[(plain, b)]), |r, f| r.frame(&f));
}

/// The outcomes the corpus is built to reach. If one goes missing, the corpus has stopped testing
/// what it is for.
fn required() -> Vec<(Op, &'static str)> {
    let mut v = vec![
        (Op::VecDot, "Zero"),
        (Op::VecCross, "parallel"),
        (Op::VecTriple, "Zero"),
        (Op::VecTriple, "Positive"),
        (Op::VecTriple, "Negative"),
        (Op::VecAdd, "OutOfRange"),
        (Op::VecOffset, "OutOfRange"),
        (Op::VecNew, "OutOfRange"),
        (Op::FloatAdd, "NotFinite"),
        (Op::FloatDot, "NotFinite"),
        (Op::FloatNormalized, "Degenerate"),
        (Op::FloatNew, "NotFinite"),
        (Op::DdMul, "NotFinite"),
        (Op::DdDiv, "Degenerate"),
        (Op::DdSqrt, "NegativeSqrt"),
        (Op::DdNew, "NotFinite"),
        (Op::DdFromInt, "None"),
        (Op::DdExactProduct, "None"),
        (Op::DdExactProduct, "Some"),
        (Op::DdRound, "exact"),
        (Op::DdRound, "inexact"),
        (Op::DdRound, "None"),
        (Op::GridNew, "Degenerate"),
        (Op::GridNew, "OutOfRange"),
        (Op::GridDeterminant, "mirror"),
        (Op::GridApply, "OutOfRange"),
        (Op::GridCompose, "OutOfRange"),
        (Op::BoxTransformed, "OutOfRange"),
        (Op::RationalNew, "OutOfRange"),
        (Op::RationalNew, "Degenerate"),
        (Op::RationalApply, "exact"),
        (Op::RationalApply, "inexact"),
        (Op::RationalApply, "OutOfRange"),
        (Op::RationalInverse, "Singular"),
        (Op::RationalToGrid, "Some"),
        (Op::RationalToGrid, "None"),
        (Op::TransformNew, "NotFinite"),
        (Op::TransformApplyGrid, "exact"),
        (Op::TransformApplyGrid, "inexact"),
        (Op::TransformApplyGrid, "OutOfRange"),
        (Op::TransformInverse, "Singular"),
        (Op::TransformClassify, "mirror"),
        (Op::TypedRotation, "Exact"),
        (Op::TypedRotation, "Rational"),
        (Op::TypedRotation, "Certified"),
        (Op::TypedSimplified, "Exact"),
        (Op::Box3Ops, "EmptyInterval"),
        (Op::Box3Ops, "None"),
        (Op::Box2Ops, "EmptyInterval"),
        (Op::FBoxNew, "EmptyInterval"),
        (Op::FBoxToGrid, "OutOfRange"),
        (Op::FrameNew, "OutOfRange"),
        (Op::FrameNew, "Map"),
        (Op::FrameNew, "Ground"),
        (Op::ToFrame, "exact"),
        (Op::ToFrame, "inexact"),
        (Op::ToFrame, "OutOfRange"),
        (Op::ToFrame, "NotFinite"),
        (Op::ToMap, "exact"),
        (Op::ToMap, "inexact"),
        (Op::ShiftTo, "Ok"),
        (Op::ShiftTo, "Crs"),
        (Op::ShiftTo, "VerticalCrs"),
        (Op::ShiftTo, "Rotation"),
        (Op::ShiftTo, "Scale"),
        (Op::ShiftTo, "NotOnLattice"),
        (Op::ShiftTo, "WrongDirection"),
        (Op::ShiftApply, "OutOfRange"),
        (Op::HomeOf, "FrameRangeExceeded"),
        (Op::HomeOf, "NotOnLattice"),
        (Op::HomeOf, "Degenerate"),
    ];
    for kind in [
        "Identity",
        "Translation",
        "Rigid",
        "UniformScale",
        "NonUniform",
        "Degenerate",
    ] {
        v.push((Op::TransformClassify, kind));
    }
    for kind in [
        "Identity",
        "Translation",
        "Rigid",
        "UniformScale",
        "NonUniform",
    ] {
        v.push((Op::RationalClassify, kind));
    }
    v
}

/// Runs the whole corpus once.
fn corpus() -> Recorder {
    let mut g = Gen(SEED);
    let mut r = Recorder::new();
    vectors(&mut g, &mut r);
    r.end_section("vectors");
    floats(&mut g, &mut r);
    r.end_section("floats");
    double_double(&mut g, &mut r);
    r.end_section("dd");
    grid_transforms(&mut g, &mut r);
    r.end_section("grid");
    rationals(&mut g, &mut r);
    r.end_section("rational");
    general_transforms(&mut g, &mut r);
    r.end_section("transform");
    typed_transforms(&mut g, &mut r);
    r.end_section("typed");
    boxes(&mut g, &mut r);
    r.end_section("boxes");
    frames(&mut g, &mut r);
    r.end_section("frames");
    shifts(&mut g, &mut r);
    r.end_section("shifts");
    r
}

/// The golden digest. The same constant must hold on every target; a change is a bug or a
/// deliberate kernel edition change (see the module documentation and `GOLDEN`).
#[test]
fn golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let r = corpus();
        (r.total.clone().finish(), r)
    });
    let r = &checked.value;
    let hash = hex(r.total.clone().finish());
    println!("geom-math determinism corpus: {} calls", r.calls);
    for (name, calls, digest) in &r.report {
        println!("  {name:<10} {calls:>6} calls  {}", hex(*digest));
    }
    println!("geom-math golden hash: {hash}");

    // The corpus must still reach every outcome it is built for, make enough calls, and never see
    // a NaN (the crate never makes one).
    let missing: Vec<_> = required()
        .into_iter()
        .filter(|k| !r.seen.contains(k))
        .collect();
    assert!(missing.is_empty(), "unreached outcomes: {missing:?}");
    assert!(r.calls >= MIN_CALLS, "only {} calls", r.calls);
    assert_eq!(r.nans, 0, "a result in the corpus is NaN");
    // No call reports a defect: `BudgetExceeded` is reachable only through a broken bit budget
    // (`error.rs`), never through an input, so the corpus must never see it, whatever the hash.
    let defects: Vec<_> = r
        .seen
        .iter()
        .filter(|(_, class)| *class == "BudgetExceeded")
        .collect();
    assert!(defects.is_empty(), "defects reported: {defects:?}");
    checked.verdict().unwrap_or_else(|e| {
        panic!("{e}. Compare the section digests above with another target's log to find the part of the crate that differs")
    });
}

/// Principle 5 and chapter 21's thread-count test: the corpus gives the same digest on 1, 2, 3
/// and 8 threads running it at once, every run in one process.
#[test]
#[cfg_attr(
    all(target_family = "wasm", not(target_feature = "atomics")),
    ignore = "WebAssembly without the atomics feature (wasm32-wasip1) has no threads: \
              std::thread::scope cannot spawn"
)]
fn golden_hash_is_the_same_on_every_thread_count() {
    let one = hex(corpus().total.finish());
    for threads in [1usize, 2, 3, 8] {
        let hashes: Vec<String> = std::thread::scope(|sc| {
            let handles: Vec<_> = (0..threads)
                .map(|_| sc.spawn(|| hex(corpus().total.finish())))
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert!(
            hashes.iter().all(|h| *h == one),
            "{threads} threads: {hashes:?} against {one} on one"
        );
    }
}
