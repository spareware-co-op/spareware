//! The canonical byte encoding of every benchmark input and result, and its SHA-256 digest.
//!
//! A benchmark's golden digests (chapter 23.13, "Output hash") are SHA-256 hashes of these bytes:
//! one over every input of its batch, one over every result. The encoding is lossless, so two
//! different values never hash alike by construction, only by a SHA-256 collision:
//!
//! - every field that takes part in a value's equality is written, at full width: integers as
//!   little-endian two's complement, doubles as their IEEE 754 bit patterns (so −0 and +0, and
//!   every NaN payload, are distinct), wide integers limb by limb;
//! - each type has a fixed layout, and the variable parts (`Option`, `Result`, enums) start with a
//!   tag byte, so a sequence of values of one type splits back into its values in exactly one way.
//!
//! Guarantees: two values of one type have the same bytes only if they are equal field for field
//! (bit for bit, for doubles); a sequence's bytes split back into its values one way only; and the
//! bytes are the same on every target and in every build profile, since every width and byte
//! order is fixed and nothing depends on memory layout, padding or addresses (the tests check
//! the documented layout).
//!
//! Method: the canonical-encoding rule of chapter 28.13 (hash a fixed, documented byte form of each
//! result, never a lossy summary), with SHA-256 as chapters 21 and 28.13 use it
//! (NIST FIPS 180-4, <https://csrc.nist.gov/pubs/fips/180-4/upd1/final>).
//!
//! Cost: linear in the bytes encoded, from one byte (a sign) to a few hundred (three planes) per
//! value, each fed to SHA-256 once; nothing is allocated.

use core::cmp::Ordering;

use geom_num::crossing::Ssi;
use geom_num::dd::DD;
use geom_num::exact_sum::ExactSum;
use geom_num::grid::{Coord, GridStep, P2, P3, Rounded, Steps};
use geom_num::interval::Interval;
use geom_num::isolate::{Flagged, IsolateError, Isolation, Reason, Zero};
use geom_num::newton::{NewtonError, Precision, Verdict};
use geom_num::plane::{Lpi, Plane, Tpi};
use geom_num::real::Real;
use geom_num::rng::{Philox, RngError, Seed, Stream, StreamId, StreamRange};
use geom_num::wide::Wide;
use geom_num::wide32::Wide32;
use geom_num::{Exactness, NumError, Sign};

use crate::sha256::{Digest, Sha256};

/// A value with a canonical byte encoding (see the module documentation).
pub trait Canonical {
    /// Feeds the value's canonical bytes to `h`.
    fn encode(&self, h: &mut Sha256);
}

/// The SHA-256 of the canonical encodings of `items`, in order.
#[must_use]
pub fn digest<T: Canonical>(items: &[T]) -> Digest {
    let mut h = Sha256::new();
    for item in items {
        item.encode(&mut h);
    }
    h.finish()
}

impl Canonical for bool {
    fn encode(&self, h: &mut Sha256) {
        h.update(&[u8::from(*self)]);
    }
}

impl Canonical for i32 {
    fn encode(&self, h: &mut Sha256) {
        h.update(&self.to_le_bytes());
    }
}

impl Canonical for i64 {
    fn encode(&self, h: &mut Sha256) {
        h.update(&self.to_le_bytes());
    }
}

impl Canonical for u32 {
    fn encode(&self, h: &mut Sha256) {
        h.update(&self.to_le_bytes());
    }
}

impl Canonical for u64 {
    fn encode(&self, h: &mut Sha256) {
        h.update(&self.to_le_bytes());
    }
}

impl Canonical for u128 {
    fn encode(&self, h: &mut Sha256) {
        h.update(&self.to_le_bytes());
    }
}

impl Canonical for i128 {
    fn encode(&self, h: &mut Sha256) {
        h.update(&self.to_le_bytes());
    }
}

impl Canonical for f64 {
    fn encode(&self, h: &mut Sha256) {
        h.update(&self.to_bits().to_le_bytes());
    }
}

impl Canonical for Ordering {
    fn encode(&self, h: &mut Sha256) {
        let tag: u8 = match self {
            Ordering::Less => 0,
            Ordering::Equal => 1,
            Ordering::Greater => 2,
        };
        h.update(&[tag]);
    }
}

impl Canonical for Sign {
    fn encode(&self, h: &mut Sha256) {
        let tag: u8 = match self {
            Sign::Negative => 0,
            Sign::Zero => 1,
            Sign::Positive => 2,
        };
        h.update(&[tag]);
    }
}

impl Canonical for Coord {
    fn encode(&self, h: &mut Sha256) {
        self.get().encode(h);
    }
}

impl Canonical for P2 {
    fn encode(&self, h: &mut Sha256) {
        self.x.encode(h);
        self.y.encode(h);
    }
}

impl Canonical for P3 {
    fn encode(&self, h: &mut Sha256) {
        self.x.encode(h);
        self.y.encode(h);
        self.z.encode(h);
    }
}

impl Canonical for GridStep {
    fn encode(&self, h: &mut Sha256) {
        // The rung is 0, 1 or 2, one per step.
        h.update(&self.rung().to_le_bytes());
    }
}

impl Canonical for Steps {
    fn encode(&self, h: &mut Sha256) {
        self.get().encode(h);
    }
}

impl<const L: usize> Canonical for Wide<L> {
    fn encode(&self, h: &mut Sha256) {
        for limb in self.to_le_limbs() {
            limb.encode(h);
        }
    }
}

/// A `Wide32` (piece G42): its limb width, 32, then its limbs, low first. The width keeps it from
/// ever encoding as the `Wide` of the same bit width, so each benchmark's inputs keep their own
/// digest.
impl<const L: usize> Canonical for Wide32<L> {
    fn encode(&self, h: &mut Sha256) {
        32u32.encode(h);
        for limb in self.to_le_limbs() {
            limb.encode(h);
        }
    }
}

impl Canonical for Plane {
    fn encode(&self, h: &mut Sha256) {
        for n in self.n() {
            n.encode(h);
        }
        self.d().encode(h);
    }
}

impl Canonical for Tpi {
    // A meeting point is exactly its three planes, in order (its equality is theirs).
    fn encode(&self, h: &mut Sha256) {
        for plane in self.planes() {
            plane.encode(h);
        }
    }
}

impl Canonical for Lpi {
    // A line-plane point is exactly its line's two points and its plane, in order.
    fn encode(&self, h: &mut Sha256) {
        for p in self.points() {
            p.encode(h);
        }
        self.plane().encode(h);
    }
}

impl Canonical for Ssi {
    // A crossing is exactly its four points, in order.
    fn encode(&self, h: &mut Sha256) {
        for p in self.points() {
            p.encode(h);
        }
    }
}

impl Canonical for Interval {
    fn encode(&self, h: &mut Sha256) {
        self.lo().encode(h);
        self.hi().encode(h);
    }
}

impl Canonical for Real {
    fn encode(&self, h: &mut Sha256) {
        self.get().encode(h);
    }
}

impl Canonical for Exactness {
    // A tag, then the bound and the grid step it counts (its equality is theirs).
    fn encode(&self, h: &mut Sha256) {
        match self {
            Exactness::Exact => h.update(&[0]),
            Exactness::Certified { bound, step } => {
                h.update(&[1]);
                bound.encode(h);
                step.encode(h);
            }
            Exactness::Approximate { deviation, step } => {
                h.update(&[2]);
                deviation.encode(h);
                step.encode(h);
            }
        }
    }
}

impl Canonical for NumError {
    fn encode(&self, h: &mut Sha256) {
        match self {
            NumError::OutOfRange { value } => {
                h.update(&[0]);
                value.encode(h);
            }
            NumError::Degenerate => h.update(&[1]),
            NumError::NotFinite => h.update(&[2]),
            NumError::WrongDirection => h.update(&[3]),
            NumError::DivisorContainsZero => h.update(&[4]),
            NumError::NegativeSqrt => h.update(&[5]),
            NumError::EmptyInterval => h.update(&[6]),
            NumError::OutsideDomain => h.update(&[7]),
            NumError::BudgetExceeded => h.update(&[8]),
            NumError::ToleranceExceeded => h.update(&[9]),
            NumError::UnprovenMaths => h.update(&[10]),
        }
    }
}

impl Canonical for DD {
    // The leading part, then the trailing part (through its getters): the pair's two fields.
    fn encode(&self, h: &mut Sha256) {
        self.hi().encode(h);
        self.lo().encode(h);
    }
}

impl Canonical for ExactSum {
    // Its fields are private to geom-num, whose only readers of them are the roundings, which lose
    // the value. Its `Debug` text writes the whole value and nothing else (the exact sum's digits
    // after a carry propagation, the special values and the lost flag), so the encoding is that
    // text's length and bytes, and two sums encode alike exactly when they hold the same value.
    fn encode(&self, h: &mut Sha256) {
        let text = format!("{self:?}");
        (text.len() as u64).encode(h);
        h.update(text.as_bytes());
    }
}

impl Canonical for Seed {
    fn encode(&self, h: &mut Sha256) {
        self.0.encode(h);
    }
}

impl Canonical for StreamId {
    fn encode(&self, h: &mut Sha256) {
        self.0.encode(h);
    }
}

impl Canonical for Philox {
    // The seed, then the stream id: the generator's two fields (through its getters).
    fn encode(&self, h: &mut Sha256) {
        self.seed().encode(h);
        self.stream_id().encode(h);
    }
}

impl Canonical for StreamRange {
    // The seed, the first stream id and the last (through its getters).
    fn encode(&self, h: &mut Sha256) {
        self.seed().encode(h);
        self.first().encode(h);
        self.last().encode(h);
    }
}

impl Canonical for Stream {
    // The generator, then the number of words drawn. A stream's other field, the block it holds,
    // is a pure function of those two (`geom_num::rng` keeps it so after every draw), so two
    // streams encode alike exactly when they are equal.
    fn encode(&self, h: &mut Sha256) {
        self.philox().encode(h);
        self.position().encode(h);
    }
}

impl Canonical for RngError {
    // A tag, then the variant's fields. `RngError` is `#[non_exhaustive]`: a variant added after
    // this was written takes tag 255, then the length and bytes of its `Debug` text, which writes
    // every field, so the encoding stays lossless until it gets a tag of its own.
    fn encode(&self, h: &mut Sha256) {
        match self {
            RngError::Exhausted { stream } => {
                h.update(&[0]);
                stream.encode(h);
            }
            RngError::ZeroBound => h.update(&[1]),
            RngError::LowAboveHigh { low, high } => {
                h.update(&[2]);
                low.encode(h);
                high.encode(h);
            }
            RngError::EmptyChunk => h.update(&[3]),
            RngError::TooFewStreams { first, last } => {
                h.update(&[4]);
                first.encode(h);
                last.encode(h);
            }
            RngError::Defect => h.update(&[5]),
            other => {
                let text = format!("{other:?}");
                h.update(&[255]);
                (text.len() as u64).encode(h);
                h.update(text.as_bytes());
            }
        }
    }
}

impl<T: Canonical + Copy> Canonical for Rounded<T> {
    // The value, then whether it is exact (the fields are private to geom-num; these are its
    // getters).
    fn encode(&self, h: &mut Sha256) {
        self.value().encode(h);
        self.is_exact().encode(h);
    }
}

impl<T: Canonical> Canonical for Option<T> {
    fn encode(&self, h: &mut Sha256) {
        match self {
            None => h.update(&[0]),
            Some(v) => {
                h.update(&[1]);
                v.encode(h);
            }
        }
    }
}

impl<T: Canonical> Canonical for Result<T, NumError> {
    fn encode(&self, h: &mut Sha256) {
        match self {
            Ok(v) => {
                h.update(&[0]);
                v.encode(h);
            }
            Err(e) => {
                h.update(&[1]);
                e.encode(h);
            }
        }
    }
}

impl<T: Canonical> Canonical for Result<T, RngError> {
    fn encode(&self, h: &mut Sha256) {
        match self {
            Ok(v) => {
                h.update(&[0]);
                v.encode(h);
            }
            Err(e) => {
                h.update(&[1]);
                e.encode(h);
            }
        }
    }
}

impl<T: Canonical> Canonical for Vec<T> {
    // The length, then each item, so a sequence of vectors splits back one way.
    fn encode(&self, h: &mut Sha256) {
        (self.len() as u64).encode(h);
        for item in self {
            item.encode(h);
        }
    }
}

impl<const N: usize> Canonical for Verdict<N> {
    // A tag, then for a box the enclosure, then the contraction bound.
    fn encode(&self, h: &mut Sha256) {
        match self {
            Verdict::NoZero => h.update(&[0]),
            Verdict::Unique {
                enclosure,
                contraction,
            } => {
                h.update(&[1]);
                enclosure.encode(h);
                contraction.encode(h);
            }
            Verdict::Narrowed {
                enclosure,
                contraction,
            } => {
                h.update(&[2]);
                enclosure.encode(h);
                contraction.encode(h);
            }
        }
    }
}

impl<T: Canonical> Canonical for Result<T, NewtonError> {
    // A tag: 0 and the value; 1 for `Singular`; 2 and the numbers layer's error.
    fn encode(&self, h: &mut Sha256) {
        match self {
            Ok(v) => {
                h.update(&[0]);
                v.encode(h);
            }
            Err(NewtonError::Singular) => h.update(&[1]),
            Err(NewtonError::Num(e)) => {
                h.update(&[2]);
                e.encode(h);
            }
        }
    }
}

impl Canonical for Precision {
    // A tag: 0 for doubles, 1 for double-double balls.
    fn encode(&self, h: &mut Sha256) {
        h.update(&[match self {
            Precision::Double => 0,
            Precision::DoubleDouble => 1,
        }]);
    }
}

impl<const N: usize> Canonical for Isolation<N> {
    // The zeros, each its enclosure, its proof box and its precision; then the flagged regions,
    // each its box and a tag for its reason (0 unproven, 1 singular, 2 not smooth).
    fn encode(&self, h: &mut Sha256) {
        (self.zeros.len() as u64).encode(h);
        for Zero {
            enclosure,
            proof,
            precision,
        } in &self.zeros
        {
            enclosure.encode(h);
            proof.encode(h);
            precision.encode(h);
        }
        (self.flagged.len() as u64).encode(h);
        for Flagged { region, reason } in &self.flagged {
            region.encode(h);
            h.update(&[match reason {
                Reason::Unproven => 0,
                Reason::Singular => 1,
                Reason::NotSmooth => 2,
            }]);
        }
    }
}

impl<T: Canonical, const N: usize> Canonical for Result<T, IsolateError<N>> {
    // A tag: 0 and the value; then one tag per error, with its fields: the numbers layer's error;
    // a budget failure's limits, kind, phase name and use; the box of a cap.
    fn encode(&self, h: &mut Sha256) {
        match self {
            Ok(v) => {
                h.update(&[0]);
                v.encode(h);
            }
            Err(IsolateError::InvalidTarget) => h.update(&[1]),
            Err(IsolateError::Num(e)) => {
                h.update(&[2]);
                e.encode(h);
            }
            Err(IsolateError::Exceeded(e)) => {
                h.update(&[3]);
                e.budget.work.encode(h);
                e.budget.logical_bytes.encode(h);
                h.update(&[u8::from(e.kind == geom_run::BudgetKind::Work)]);
                h.update(e.phase.name().as_bytes());
                e.used.work.encode(h);
                e.used.logical_bytes.encode(h);
            }
            Err(IsolateError::Cancelled) => h.update(&[4]),
            Err(IsolateError::OutOfMemory) => h.update(&[5]),
            Err(IsolateError::CapReached { enclosure }) => {
                h.update(&[6]);
                enclosure.encode(h);
            }
            Err(IsolateError::Imprecise { region }) => {
                h.update(&[7]);
                region.encode(h);
            }
            Err(IsolateError::Unsplittable { region }) => {
                h.update(&[8]);
                region.encode(h);
            }
            Err(IsolateError::Internal { region }) => {
                h.update(&[9]);
                region.encode(h);
            }
        }
    }
}

impl<T: Canonical, const N: usize> Canonical for [T; N] {
    fn encode(&self, h: &mut Sha256) {
        for item in self {
            item.encode(h);
        }
    }
}

impl<A: Canonical, B: Canonical> Canonical for (A, B) {
    fn encode(&self, h: &mut Sha256) {
        self.0.encode(h);
        self.1.encode(h);
    }
}

impl<A: Canonical, B: Canonical, C: Canonical> Canonical for (A, B, C) {
    fn encode(&self, h: &mut Sha256) {
        self.0.encode(h);
        self.1.encode(h);
        self.2.encode(h);
    }
}

impl<A: Canonical, B: Canonical, C: Canonical, D: Canonical> Canonical for (A, B, C, D) {
    fn encode(&self, h: &mut Sha256) {
        self.0.encode(h);
        self.1.encode(h);
        self.2.encode(h);
        self.3.encode(h);
    }
}

// `geom-math`'s types (decision 32.44's count benchmarks for pieces G45, G46 and G48).

impl<T: Canonical + Copy> Canonical for geom_math::Rounded<T> {
    // The value, then whether it is exact (the fields are private to geom-math; these are its
    // getters).
    fn encode(&self, h: &mut Sha256) {
        self.value().encode(h);
        self.is_exact().encode(h);
    }
}

impl Canonical for geom_math::F2 {
    fn encode(&self, h: &mut Sha256) {
        self.to_array().encode(h);
    }
}

impl Canonical for geom_math::MapPoint {
    fn encode(&self, h: &mut Sha256) {
        self.easting.encode(h);
        self.northing.encode(h);
        self.height.encode(h);
    }
}

impl Canonical for geom_math::RationalRotation {
    // The quaternion is the rotation: every other field is derived from it.
    fn encode(&self, h: &mut Sha256) {
        self.quaternion().encode(h);
    }
}

impl<T: Canonical + Copy> Canonical for geom_math::WithinAngle<T> {
    // The value, then the angle it is from the one asked for.
    fn encode(&self, h: &mut Sha256) {
        self.value().encode(h);
        self.angle().encode(h);
    }
}

impl Canonical for String {
    // The length, then the bytes, so a sequence of texts splits back one way.
    fn encode(&self, h: &mut Sha256) {
        (self.len() as u64).encode(h);
        h.update(self.as_bytes());
    }
}

/// The length and bytes of a value's `Debug` text: the encoding of a type whose fields are private
/// to its crate and whose `Debug` writes every one of them (as for `ExactSum`).
fn debug_text<T: core::fmt::Debug>(v: &T, h: &mut Sha256) {
    let text = format!("{v:?}");
    (text.len() as u64).encode(h);
    h.update(text.as_bytes());
}

impl Canonical for geom_math::Decimal {
    fn encode(&self, h: &mut Sha256) {
        debug_text(self, h);
    }
}

impl<T: Canonical> Canonical for Result<T, geom_math::MathError> {
    fn encode(&self, h: &mut Sha256) {
        match self {
            Ok(v) => {
                h.update(&[0]);
                v.encode(h);
            }
            Err(e) => {
                h.update(&[1]);
                debug_text(e, h);
            }
        }
    }
}

impl<T: Canonical> Canonical for Result<T, geom_math::DecimalError> {
    fn encode(&self, h: &mut Sha256) {
        match self {
            Ok(v) => {
                h.update(&[0]);
                v.encode(h);
            }
            Err(e) => {
                h.update(&[1]);
                debug_text(e, h);
            }
        }
    }
}

// `geom-linalg`'s types (decision 32.44's count benchmarks for pieces G49 and G50).

impl Canonical for geom_linalg::Matrix {
    // The shape, then every entry row by row (its getter refuses no index inside the shape).
    fn encode(&self, h: &mut Sha256) {
        (self.rows() as u64).encode(h);
        (self.cols() as u64).encode(h);
        for i in 0..self.rows() {
            for j in 0..self.cols() {
                self.get(i, j).unwrap_or(f64::NAN).encode(h);
            }
        }
    }
}

impl Canonical for geom_linalg::PivotedQr {
    // The column permutation, then R's diagonal in order: what the factorisation exposes.
    fn encode(&self, h: &mut Sha256) {
        let p = self.permutation();
        (p.len() as u64).encode(h);
        for &i in p {
            (i as u64).encode(h);
        }
        let mut k = 0;
        while let Some(d) = self.r_diagonal(k) {
            d.encode(h);
            k += 1;
        }
        (k as u64).encode(h);
    }
}

impl Canonical for geom_linalg::Svd {
    // The singular values, then U and V.
    fn encode(&self, h: &mut Sha256) {
        let s = self.singular_values();
        (s.len() as u64).encode(h);
        for &x in s {
            x.encode(h);
        }
        self.u().encode(h);
        self.v().encode(h);
    }
}

impl<T: Canonical> Canonical for Result<T, geom_linalg::LinalgError> {
    fn encode(&self, h: &mut Sha256) {
        match self {
            Ok(v) => {
                h.update(&[0]);
                v.encode(h);
            }
            Err(e) => {
                h.update(&[1]);
                debug_text(e, h);
            }
        }
    }
}

// `geom-topo`'s types (decision 32.44's count benchmarks for piece G80).

impl Canonical for geom_topo::Builder {
    fn encode(&self, h: &mut Sha256) {
        debug_text(self, h);
    }
}

impl<T: Canonical> Canonical for Result<T, geom_topo::TopoError> {
    fn encode(&self, h: &mut Sha256) {
        match self {
            Ok(v) => {
                h.update(&[0]);
                v.encode(h);
            }
            Err(e) => {
                h.update(&[1]);
                debug_text(e, h);
            }
        }
    }
}

// The attribute step's types (decision 32.44's count benchmarks for piece G90).

impl Canonical for crate::inputs::attributes::Scene {
    fn encode(&self, h: &mut Sha256) {
        debug_text(self, h);
    }
}

impl<T: Canonical> Canonical for Result<T, geom_name::AttrError> {
    fn encode(&self, h: &mut Sha256) {
        match self {
            Ok(v) => {
                h.update(&[0]);
                v.encode(h);
            }
            Err(e) => {
                h.update(&[1]);
                debug_text(e, h);
            }
        }
    }
}

// `geom-bvh`'s types (decision 32.44's count benchmarks for pieces G54 and G55).

impl<const D: usize> Canonical for geom_bvh::Aabb<D> {
    fn encode(&self, h: &mut Sha256) {
        self.lo().encode(h);
        self.hi().encode(h);
    }
}

impl<const D: usize> Canonical for geom_bvh::Entry<D> {
    fn encode(&self, h: &mut Sha256) {
        self.key.encode(h);
        self.bbox.encode(h);
    }
}

impl Canonical for geom_bvh::Point {
    fn encode(&self, h: &mut Sha256) {
        self.key.encode(h);
        self.at.encode(h);
    }
}

impl<T: Canonical> Canonical for Result<T, geom_bvh::BvhError> {
    fn encode(&self, h: &mut Sha256) {
        match self {
            Ok(v) => {
                h.update(&[0]);
                v.encode(h);
            }
            Err(e) => {
                h.update(&[1]);
                debug_text(e, h);
            }
        }
    }
}

// `geom-nurbs`'s types (decision 32.44's count benchmarks for pieces G56, G58 and G59).

impl<const D: usize> Canonical for geom_nurbs::Bounds<D> {
    fn encode(&self, h: &mut Sha256) {
        self.lo.encode(h);
        self.hi.encode(h);
    }
}

impl<const D: usize> Canonical for geom_curve::DerivativeBound<D> {
    fn encode(&self, h: &mut Sha256) {
        match self {
            geom_curve::DerivativeBound::Bounded(b) => {
                h.update(&[0]);
                b.encode(h);
            }
            geom_curve::DerivativeBound::Unbounded => h.update(&[1]),
        }
    }
}

impl Canonical for geom_surface::SecondDerivativeBound {
    fn encode(&self, h: &mut Sha256) {
        match self {
            geom_surface::SecondDerivativeBound::Bounded(p) => {
                h.update(&[0]);
                p.uu.encode(h);
                p.uv.encode(h);
                p.vv.encode(h);
            }
            geom_surface::SecondDerivativeBound::Unbounded => h.update(&[1]),
        }
    }
}

impl<const D: usize> Canonical for geom_nurbs::CurveFit<D> {
    // Its curve's basis and coefficients, its parameters and its stated error, all in its `Debug`
    // text, which prints each double in the shortest form that reads back to the same bits.
    fn encode(&self, h: &mut Sha256) {
        debug_text(self, h);
    }
}

impl<T: Canonical> Canonical for Result<T, geom_nurbs::NurbsError> {
    fn encode(&self, h: &mut Sha256) {
        match self {
            Ok(v) => {
                h.update(&[0]);
                v.encode(h);
            }
            Err(e) => {
                h.update(&[1]);
                debug_text(e, h);
            }
        }
    }
}

// `geom-curve`'s types (decision 32.44's count benchmarks for pieces G60 to G64 and G67).

impl<const D: usize> Canonical for geom_nurbs::Changed<geom_curve::NurbsForm<D>> {
    // The converted curve and whether the conversion changed it, all in its `Debug` text, which
    // prints each double in the shortest form that reads back to the same bits.
    fn encode(&self, h: &mut Sha256) {
        debug_text(self, h);
    }
}

impl<T: Canonical> Canonical for Result<T, geom_curve::CurveError> {
    fn encode(&self, h: &mut Sha256) {
        match self {
            Ok(v) => {
                h.update(&[0]);
                v.encode(h);
            }
            Err(e) => {
                h.update(&[1]);
                debug_text(e, h);
            }
        }
    }
}

// Root finding and quadrature (decision 32.44's count benchmark for piece G51).

impl Canonical for geom_num::brent::Zero {
    // Its fields (the zero's bracket and the evaluations used) are all in its `Debug` text, which
    // prints each double in the shortest form that reads back to the same bits.
    fn encode(&self, h: &mut Sha256) {
        debug_text(self, h);
    }
}

impl<T: Canonical> Canonical for Result<T, geom_num::brent::BrentError> {
    fn encode(&self, h: &mut Sha256) {
        match self {
            Ok(v) => {
                h.update(&[0]);
                v.encode(h);
            }
            Err(e) => {
                h.update(&[1]);
                debug_text(e, h);
            }
        }
    }
}

impl<T: Canonical> Canonical for Result<T, geom_num::quadrature::QuadError> {
    fn encode(&self, h: &mut Sha256) {
        match self {
            Ok(v) => {
                h.update(&[0]);
                v.encode(h);
            }
            Err(e) => {
                h.update(&[1]);
                debug_text(e, h);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geom_num::grid;

    fn bytes<T: Canonical>(v: &T) -> Digest {
        digest(core::slice::from_ref(v))
    }

    #[test]
    fn values_that_differ_anywhere_encode_differently() {
        // The weaknesses of a folded 64-bit summary: bit 63 cancelling, words that alias across
        // fields, errors that alias results. None survives a lossless encoding.
        assert_ne!(bytes(&2.0f64), bytes(&-2.0f64));
        assert_ne!(bytes(&0.0f64), bytes(&-0.0f64));
        let w = |v: i128| Wide::<4>::from_i128(v);
        assert_ne!(bytes(&w(1 << 63)), bytes(&w(1 << 64)));
        let q = |x, y, z| P3::new(x, y, z).unwrap();
        assert_ne!(bytes(&q(65_536, 0, 0)), bytes(&q(0, 1, 0)));
        let ok: Result<Coord, NumError> = Ok(Coord::new(0x0040_0000).unwrap());
        let err: Result<Coord, NumError> = Err(NumError::WrongDirection);
        assert_ne!(bytes(&ok), bytes(&err));
        // One step of Q0 from 25 steps of Q1 (exact) and from 26 (rounded): the same value, and
        // only the flag differs.
        let coarse = |v| grid::coarsen(Coord::new(v).unwrap(), GridStep::Q1, GridStep::Q0).unwrap();
        let (exact, rounded) = (coarse(25), coarse(26));
        assert_eq!(exact.value(), rounded.value());
        assert!(exact.is_exact() && !rounded.is_exact());
        assert_ne!(bytes(&exact), bytes(&rounded));
        assert_ne!(bytes(&Some(0i64)), bytes(&None::<i64>));
        let (q0, q1) = (GridStep::Q0, GridStep::Q1);
        assert_ne!(
            bytes(&Exactness::Certified {
                bound: Steps(3),
                step: q0
            }),
            bytes(&Exactness::Approximate {
                deviation: Steps(3),
                step: q0
            })
        );
        // A bound is a count and its step: 3 steps of Q0 are not 3 steps of Q1.
        assert_ne!(
            bytes(&Exactness::Certified {
                bound: Steps(3),
                step: q0
            }),
            bytes(&Exactness::Certified {
                bound: Steps(3),
                step: q1
            })
        );
        // Every error has its own tag.
        let errors = [
            NumError::OutOfRange { value: 0 },
            NumError::Degenerate,
            NumError::NotFinite,
            NumError::WrongDirection,
            NumError::DivisorContainsZero,
            NumError::NegativeSqrt,
            NumError::EmptyInterval,
            NumError::OutsideDomain,
            NumError::BudgetExceeded,
            NumError::ToleranceExceeded,
            NumError::UnprovenMaths,
        ];
        let tags: std::collections::BTreeSet<Digest> = errors.iter().map(bytes).collect();
        assert_eq!(tags.len(), errors.len());
        // And every error of the random-number module, with its fields.
        let errors = [
            RngError::Exhausted {
                stream: StreamId(0),
            },
            RngError::Exhausted {
                stream: StreamId(1),
            },
            RngError::ZeroBound,
            RngError::LowAboveHigh { low: 2, high: 1 },
            RngError::LowAboveHigh { low: 3, high: 1 },
            RngError::EmptyChunk,
            RngError::TooFewStreams {
                first: StreamId(0),
                last: StreamId(5),
            },
            RngError::Defect,
        ];
        let tags: std::collections::BTreeSet<Digest> = errors.iter().map(bytes).collect();
        assert_eq!(tags.len(), errors.len());
        // A stream is its generator and its position: one word drawn is a different stream.
        let philox = Philox::new(Seed(7), StreamId(9));
        let mut drawn = philox.stream();
        assert!(drawn.next_u32().is_ok());
        assert_ne!(bytes(&philox.stream()), bytes(&drawn));
        assert_ne!(bytes(&philox), bytes(&Philox::new(Seed(9), StreamId(7))));
        let ok: Result<u32, RngError> = Ok(0);
        let err: Result<u32, RngError> = Err(RngError::ZeroBound);
        assert_ne!(bytes(&ok), bytes(&err));
    }

    #[test]
    fn sequences_split_one_way() {
        // [Some(x), None] and [None, Some(x)] have the same bytes in a different order, never the
        // same bytes.
        let a = [Some(5i64), None];
        let b = [None, Some(5i64)];
        assert_ne!(digest(&a), digest(&b));
        // Two flips of the same bit in different results no longer cancel.
        assert_ne!(digest(&[1.5f64, 2.5]), digest(&[-1.5f64, -2.5]));
    }

    #[test]
    fn the_layout_is_the_documented_one() {
        // A coordinate is its i64, little-endian: the encoding is part of the golden digests.
        let mut h = Sha256::new();
        Coord::new(-2).unwrap().encode(&mut h);
        assert_eq!(h.finish(), Sha256::digest(&(-2i64).to_le_bytes()));
        let mut h = Sha256::new();
        Err::<Coord, NumError>(NumError::OutOfRange { value: 7 }).encode(&mut h);
        let mut expected = vec![1u8, 0];
        expected.extend_from_slice(&7i128.to_le_bytes());
        assert_eq!(h.finish(), Sha256::digest(&expected));
    }
}
