//! Frames: the record that ties a model's flat grid to a map projection, exact shifts between
//! frames on a shared lattice, and the regional lattice of frames for long networks.
//!
//! A frame is the kernel's flat coordinate space for one model or one piece of a network (chapters
//! 6.1 and 17.1). Its [`FrameRecord`] holds the grid step, the horizontal coordinate reference
//! system's id, an optional vertical one, the CRS's [`LinearUnit`] (metres per map unit), the
//! coordinate epoch ([`DecimalYear`]) and deformation model ([`ModelRef`]) its coordinates hold at,
//! the origin in grid steps, the [`FrameKind`] (a Map frame, or a Ground frame with a rotation about
//! z and a horizontal scale factor) and, for a frame of a regional network, its [`LatticeCell`].
//! Its meaning, for a point with frame grid coordinates (x, y, z) and map coordinates (E, N, H) in
//! CRS units of u metres each:
//!
//! ```text
//! E·u·S = O_E + k·(cos θ·x − sin θ·y)
//! N·u·S = O_N + k·(sin θ·x + cos θ·y)        S = grid steps per metre (6,400,000 at q0)
//! H·u·S = O_H + z                            heights are never rotated or scaled
//! ```
//!
//! where (`O_E`, `O_N`, `O_H`) is the origin in steps, θ the angle of the rotation's direction (c, s)
//! from the easting axis (cos θ = c/r, sin θ = s/r, r = √(c² + s²)), and k = `k_n/k_d` the scale
//! factor. A Map frame has θ = 0 and k = 1. Converting map coordinates (doubles or exact decimal
//! text) into a frame and back, each rounded once and correctly, is [`crate::convert`]'s job.
//!
//! **The NZTM2000 example.** NZTM2000 (EPSG:2193) is a transverse Mercator projection in metres with
//! false northing 10,000,000 m; central Auckland is near N 5,921,000 m, which is about 2^45 steps
//! at q0, too far from the grid's zero for the 40-bit exact predicates. So a frame's origin sits at
//! the site, as a whole number of steps (up to [`ORIGIN_MAX`] = 2^60), and every model coordinate
//! is a short offset from it. Its coordinates hold at NZGD2000's reference epoch, 2000.0, and
//! observations from other dates reach them through LINZ's deformation model, which the record
//! names by its content (chapter 17.1, Epochs).
//!
//! **Lattices** ([`FrameRecord::shift_to`]). Frames with the same coordinate reference systems,
//! linear unit, epoch, deformation model, rotation and scale whose origins differ by whole steps
//! along the target's axes are on a shared lattice: moving a point between them is multiplying by
//! 25 per rung of refinement (exact, chapter 6.1) and adding an integer offset. Anything else is
//! refused with [`FrameMismatch`]: frames are never mixed without an exact shift.
//!
//! **Regional networks** ([`FrameRecord::lattice`], chapter 17.1). A network wider than one frame
//! uses Map frames with origins every [`LATTICE_SPACING`] = 2^39 steps (85.9 km at q0) in easting and
//! northing from a base origin. Neighbours overlap by three quarters of a frame's width, so a
//! point moves between them by an exact integer shift. Each frame owns the half-open cell within
//! [`CELL_HALF`] = 2^38 steps of its origin ([`FrameRecord::owns`]); geometry belongs to the frame
//! whose cell holds the centre of its box ([`FrameRecord::home_frame`]), an operation runs in the
//! frame whose cell holds the centre of its operands' combined box ([`FrameRecord::home_of`]), and
//! operands that do not fit that frame are refused with [`MathError::FrameRangeExceeded`]. The
//! spacing is an even number of steps, so lattice frames round a map value halfway between two
//! steps to the same place; frames whose origins differ by an odd number of steps may not, since
//! each rounds its own coordinates half to even (chapter 17.1).
//!
//! **The rotation's form, and what stays outside the record.** A Ground frame's rotation is an exact
//! integer direction (c, s), reduced by their greatest common divisor, with |c|, |s| ≤ 2^31:
//! chapter 17.2's second direction form, fine to about 2^−31 rad, for which every conversion is
//! exact or correctly rounded. Chapter 17.1 also names a rotation given as whole angle units and
//! turned into a direction by `sinpi` and `cospi`: that conversion belongs with `AngleUnits`, in
//! piece "Angle units and core COGO", which builds on this record and gives it the direction it
//! stores; the bearing as given stays in the operation's recipe. A frame has one horizontal scale
//! factor: IFC's `IfcMapConversionScaled` with unequal x and y factors, or a height factor other
//! than one, stays in the foreign recipe (chapter 24), because heights are never scaled.
//!
//! Bit budgets (|c|, |s| ≤ 2^31; `k_n`, `k_d` < 2^64; integer r < 2^32; |O| ≤ 2^60; S < 2^32) are
//! derived where each integer is formed. Every integer step is a checked method, never a native
//! operator, so nothing depends on overflow checks being on; a step its budget proves fits can
//! fail only through a defect in this crate, and is then reported as [`NumError::BudgetExceeded`],
//! never answered.
//!
//! Guarantees:
//!
//! - **Canonical records.** Rotations, scales and linear units are stored reduced, epochs and
//!   deformation models exactly, so two records are `==` exactly when they describe the same
//!   frame.
//! - **Frames are never mixed without an exact shift.** [`FrameRecord::shift_to`] and
//!   [`FrameRecord::home_of`] refuse frames whose coordinate reference systems, linear units,
//!   epochs, deformation models, rotations or scales differ with the matching [`FrameMismatch`],
//!   in that order, and frames off a shared lattice with [`FrameMismatch::NotOnLattice`].
//! - **Shifts are exact.** A shift's offset is decided whole before its range is checked, so two
//!   frames off a shared lattice are refused as such however far apart they are; an offset beyond
//!   [`SHIFT_MAX`] is [`NumError::OutOfRange`] with its exact value (saturated beyond `i128`).
//! - **Home frames.** An operation runs in the lattice frame whose cell holds the centre of its
//!   operands' combined box, or is refused with [`MathError::FrameRangeExceeded`].
//!
//! Method: the frame record and lattice rules of chapter 17.1
//! (`projects/engines/geometry/architecture/17-survey-terrain-civil.md`), with EPSG:2193 as given
//! in PROJ's copy of the EPSG data
//! (<https://github.com/OSGeo/PROJ/blob/5fa31c29f0a806568ed7bc76c1a557604db4a804/data/sql/projected_crs.sql#L375>);
//! exact integer shifts, with divisibility decided by long division of a wide integer (D. E. Knuth,
//! *The Art of Computer Programming*, vol. 2, section 4.3.1,
//! <https://www-cs-faculty.stanford.edu/~knuth/taocp.html>).
//! Consulted: PROJ (its copy of the EPSG dataset, whose own terms of use cover the data), MIT,
//!   <https://github.com/OSGeo/PROJ/blob/5fa31c29f0a806568ed7bc76c1a557604db4a804/data/sql/projected_crs.sql#L375>,
//!   for the defining parameters of EPSG:2193, NZTM2000. No code copied.
//!
//! Cost: no allocation, and constant time for every operation except
//! [`FrameRecord::home_of`], which is linear in the number of operands.
//!
//! - Records and accessors: a few comparisons. [`Rotation::direction`] and [`Scale::new`] reduce
//!   32- and 64-bit integers by their greatest common divisor, at most 129 remainder steps.
//! - [`FrameRecord::shift_to`] and [`FrameShift::apply`]: a few `i128` products, one long division
//!   of a 1536-bit integer by a 96-bit one per horizontal axis (1536 shift-and-subtract steps) in a
//!   rational Ground frame, and three range checks.

#![allow(
    clippy::many_single_char_names,
    reason = "variable names follow the formulas of the method each function cites"
)]

use geom_num::NumError;
use geom_num::grid::{COORD_MAX, Coord, GridStep, P3, QUARTER_NM_PER_MM};

use crate::bbox::Box3;
use crate::epoch::{DecimalYear, ModelRef};
use crate::error::{FrameMismatch, MathError, budget};
use crate::exact::{Big, divrem_u128, gcd_u128};
use crate::unit::LinearUnit;

/// The largest magnitude of an origin component, 2^60 steps (about 180 million km at q0 and
/// 288,000 km at q2): every projected coordinate system on Earth, at every step. The bit budgets
/// assume it.
pub const ORIGIN_MAX: i64 = 1 << 60;

/// The largest magnitude of a lattice shift's offset, 2^62 steps, so applying a shift never
/// overflows `i64`.
pub const SHIFT_MAX: i64 = 1 << 62;

/// The spacing of a regional lattice's frame origins, 2^39 steps (85.9 km at q0), in easting and
/// northing (chapter 17.1).
pub const LATTICE_SPACING: i64 = 1 << 39;

/// Half a lattice cell, 2^38 steps: a lattice frame owns the positions x and y with
/// −2^38 ≤ x, y < 2^38 in its own grid.
pub const CELL_HALF: i64 = 1 << 38;

/// One metre in quarter-nanometres: 1000 millimetres, 4·10^9.
const QUARTER_NM_PER_METRE: u64 = 1_000 * QUARTER_NM_PER_MM;

/// [`LATTICE_SPACING`] as an `i128`, a widening.
const SPACING: i128 = LATTICE_SPACING as i128;

/// Twice [`SPACING`], 2^40, evaluated when the crate compiles (where an overflow stops the build).
const TWICE_SPACING: i128 = 2 * SPACING;

/// The grid steps in one metre: 4·10^9 quarter-nanometres divided by the step's size, exactly
/// 6,400,000 (q0), 160,000,000 (q1) or 4,000,000,000 (q2), each below 2^32. Written out, so no
/// division runs; each is checked against the numbers layer's unit arithmetic when the crate
/// compiles.
#[must_use]
pub const fn steps_per_metre(step: GridStep) -> u64 {
    match step {
        GridStep::Q0 => 6_400_000,
        GridStep::Q1 => 160_000_000,
        GridStep::Q2 => 4_000_000_000,
    }
}

// Each step's count is the metre divided by the step exactly, as `GridStep::steps_per` computes
// it, and below 2^32.
const _: () = {
    assert!(QUARTER_NM_PER_METRE == 4_000_000_000);
    assert!(matches!(
        GridStep::Q0.steps_per(QUARTER_NM_PER_METRE),
        Some(6_400_000)
    ));
    assert!(matches!(
        GridStep::Q1.steps_per(QUARTER_NM_PER_METRE),
        Some(160_000_000)
    ));
    assert!(matches!(
        GridStep::Q2.steps_per(QUARTER_NM_PER_METRE),
        Some(4_000_000_000)
    ));
    assert!(steps_per_metre(GridStep::Q0) == 6_400_000);
    assert!(steps_per_metre(GridStep::Q1) == 160_000_000);
    assert!(steps_per_metre(GridStep::Q2) == 4_000_000_000);
    assert!(steps_per_metre(GridStep::Q2) < 1 << 32);
};

/// A coordinate reference system's identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CrsId {
    /// An EPSG code, for example 2193 for NZGD2000 / NZTM2000, or 7839 for NZVD2016 height.
    Epsg(u32),
    /// A local system, numbered by the document that defines it.
    Local(u32),
}

/// A rotation about z, given exactly as an integer direction (c, s): the frame's x axis points
/// along (c, s) in map coordinates, so cos θ = c/r and sin θ = s/r with r = √(c² + s²).
///
/// Stored reduced by the greatest common divisor, so equal rotations have equal fields. Quarter
/// turns are the directions (1, 0), (0, 1), (−1, 0) and (0, −1). c² + s² and, when it is a
/// perfect square, its integer root are computed once, with checked arithmetic, when the rotation
/// is made, so reading them cannot fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rotation {
    cos: i32,
    sin: i32,
    /// c² + s², below 2^63.
    norm2: u64,
    /// √(c² + s²) when it is an integer, below 2^32.
    exact_norm: Option<u32>,
}

/// The quarter turn along the unit direction (c, s): c² + s² = 1, whose root is 1.
const fn unit(cos: i32, sin: i32) -> Rotation {
    Rotation {
        cos,
        sin,
        norm2: 1,
        exact_norm: Some(1),
    }
}

impl Rotation {
    /// No rotation: the direction (1, 0).
    pub const IDENTITY: Rotation = unit(1, 0);

    /// `turns` quarter turns counter-clockwise; any integer, taken modulo 4.
    #[must_use]
    pub const fn quarter_turns(turns: i32) -> Rotation {
        match turns.rem_euclid(4) {
            0 => unit(1, 0),
            1 => unit(0, 1),
            2 => unit(-1, 0),
            _ => unit(0, -1),
        }
    }

    /// The rotation whose x axis points along the integer direction (cos, sin), reduced by their
    /// greatest common divisor.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] for the zero direction (0, 0). [`NumError::BudgetExceeded`] only
    /// if a checked step failed, which the bounds below rule out.
    pub fn direction(cos: i32, sin: i32) -> Result<Rotation, NumError> {
        if cos == 0 && sin == 0 {
            return Err(NumError::Degenerate);
        }
        // 1 ≤ g ≤ 2^31 divides both, so each quotient's magnitude is at most the original's and
        // fits i32 (−2^31 divided by 1 stays −2^31): no step below can fail.
        let g = gcd_u128(
            u128::from(cos.unsigned_abs()),
            u128::from(sin.unsigned_abs()),
        );
        let g = budget(i64::try_from(g).ok())?;
        let reduce = |v: i32| -> Result<i32, NumError> {
            budget(
                i64::from(v)
                    .checked_div(g)
                    .and_then(|q| i32::try_from(q).ok()),
            )
        };
        let (cos, sin) = (reduce(cos)?, reduce(sin)?);
        // |c|, |s| ≤ 2^31, and a reduced direction never has |c| = |s| = 2^31, so
        // c² + s² < 2^63; its root is below 2^31.5, inside u32.
        let (c, s) = (u64::from(cos.unsigned_abs()), u64::from(sin.unsigned_abs()));
        let norm2 = budget(
            c.checked_mul(c)
                .zip(s.checked_mul(s))
                .and_then(|(cc, ss)| cc.checked_add(ss)),
        )?;
        let r = norm2.isqrt();
        let exact_norm = if r.checked_mul(r) == Some(norm2) {
            Some(budget(u32::try_from(r).ok())?)
        } else {
            None
        };
        Ok(Rotation {
            cos,
            sin,
            norm2,
            exact_norm,
        })
    }

    /// The direction's x component c.
    #[must_use]
    pub const fn cos(self) -> i32 {
        self.cos
    }

    /// The direction's y component s.
    #[must_use]
    pub const fn sin(self) -> i32 {
        self.sin
    }

    /// The number of quarter turns (0 to 3) if this is a multiple of a quarter turn.
    #[must_use]
    pub const fn quarter_turn_count(self) -> Option<u8> {
        match (self.cos, self.sin) {
            (1, 0) => Some(0),
            (0, 1) => Some(1),
            (-1, 0) => Some(2),
            (0, -1) => Some(3),
            _ => None,
        }
    }

    /// c² + s², exactly. Below 2^63: a reduced direction never has |c| = |s| = 2^31.
    #[must_use]
    pub const fn norm2(self) -> u64 {
        self.norm2
    }

    /// r = √(c² + s²) when it is an integer (a Pythagorean direction such as (3, 4), or a quarter
    /// turn): then the rotation is rational. Below 2^32.
    #[must_use]
    pub const fn exact_norm(self) -> Option<u32> {
        self.exact_norm
    }
}

/// A horizontal scale factor k = num/den, exact and stored reduced: any positive ratio of 64-bit
/// integers, so a factor given to as many as 19 decimal places is held as given, for example
/// 0.9999876543 = `Scale::new(9_999_876_543, 10_000_000_000)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Scale {
    num: u64,
    den: u64,
}

impl Scale {
    /// The scale factor one.
    pub const ONE: Scale = Scale { num: 1, den: 1 };

    /// The scale factor num/den, reduced.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] if either part is zero. [`NumError::BudgetExceeded`] only if a
    /// checked division failed, which a nonzero common divisor rules out.
    pub fn new(num: u64, den: u64) -> Result<Scale, NumError> {
        if num == 0 || den == 0 {
            return Err(NumError::Degenerate);
        }
        // g ≥ 1 divides both, so each quotient is at most the original and fits u64.
        let g = gcd_u128(u128::from(num), u128::from(den));
        let part = |v: u64| -> Result<u64, NumError> {
            budget(
                u128::from(v)
                    .checked_div(g)
                    .and_then(|q| u64::try_from(q).ok()),
            )
        };
        Ok(Scale {
            num: part(num)?,
            den: part(den)?,
        })
    }

    /// The numerator `k_n`.
    #[must_use]
    pub const fn num(self) -> u64 {
        self.num
    }

    /// The denominator `k_d`.
    #[must_use]
    pub const fn den(self) -> u64 {
        self.den
    }

    /// Whether the factor is exactly one.
    #[must_use]
    pub const fn is_one(self) -> bool {
        self.num == 1 && self.den == 1
    }
}

/// What a frame's grid is, relative to its map projection (chapter 17.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FrameKind {
    /// Map grid coordinates: no rotation and no scale.
    Map,
    /// A site grid: rotated about z and scaled horizontally only (heights are never scaled).
    Ground {
        /// The rotation of the frame's x axis from the easting axis.
        rotation: Rotation,
        /// The horizontal scale factor: map distance over frame distance.
        scale: Scale,
    },
}

/// Where a frame sits in a regional lattice (chapter 17.1): the lattice's base origin in steps and
/// the cell's index; the frame's origin is the base plus 2^39 steps times the index in easting and
/// northing, at the base's height.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LatticeCell {
    base: [i64; 3],
    index: [i64; 2],
}

impl LatticeCell {
    /// The lattice's base origin (easting, northing, height), in steps.
    #[must_use]
    pub const fn base(&self) -> [i64; 3] {
        self.base
    }

    /// The cell's index along easting and northing.
    #[must_use]
    pub const fn index(&self) -> [i64; 2] {
        self.index
    }
}

/// The record tying a frame to the real world (chapter 17.1): grid step, horizontal and vertical
/// coordinate reference systems, linear unit, coordinate epoch, deformation model, origin in
/// steps, kind and lattice cell. Immutable; made only by the constructors, which check the
/// origin's range, and the `with_` methods, which return a changed copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FrameRecord {
    step: GridStep,
    horizontal_crs: CrsId,
    vertical_crs: Option<CrsId>,
    linear_unit: LinearUnit,
    epoch: Option<DecimalYear>,
    model: Option<ModelRef>,
    origin: [i64; 3],
    kind: FrameKind,
    lattice: Option<LatticeCell>,
}

/// Whether two frames describe their map coordinates alike: the same coordinate reference
/// systems, linear unit, epoch and deformation model, checked in that order.
fn same_map(a: &FrameRecord, b: &FrameRecord) -> Result<(), MathError> {
    let mismatch = if a.horizontal_crs != b.horizontal_crs {
        FrameMismatch::Crs
    } else if a.vertical_crs != b.vertical_crs {
        FrameMismatch::VerticalCrs
    } else if a.linear_unit != b.linear_unit {
        FrameMismatch::LinearUnit
    } else if a.epoch != b.epoch {
        FrameMismatch::Epoch
    } else if a.model != b.model {
        FrameMismatch::DeformationModel
    } else {
        return Ok(());
    };
    Err(MathError::FrameMismatch(mismatch))
}

impl FrameRecord {
    /// A frame record: a Map frame when the rotation is the identity and the scale one, otherwise a
    /// Ground frame; map coordinates in metres, and no vertical CRS, epoch, deformation model or
    /// lattice cell (see the `with_` methods and [`FrameRecord::lattice`]).
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if an origin component exceeds [`ORIGIN_MAX`] in magnitude.
    pub fn new(
        crs: CrsId,
        step: GridStep,
        origin: [i64; 3],
        rotation: Rotation,
        scale: Scale,
    ) -> Result<FrameRecord, NumError> {
        for o in origin {
            if o.unsigned_abs() > ORIGIN_MAX.unsigned_abs() {
                return Err(NumError::OutOfRange {
                    value: i128::from(o),
                });
            }
        }
        let kind = if rotation == Rotation::IDENTITY && scale.is_one() {
            FrameKind::Map
        } else {
            FrameKind::Ground { rotation, scale }
        };
        Ok(FrameRecord {
            step,
            horizontal_crs: crs,
            vertical_crs: None,
            linear_unit: LinearUnit::METRE,
            epoch: None,
            model: None,
            origin,
            kind,
            lattice: None,
        })
    }

    /// A Map frame: no rotation, scale one.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if an origin component exceeds [`ORIGIN_MAX`] in magnitude.
    pub fn map(crs: CrsId, step: GridStep, origin: [i64; 3]) -> Result<FrameRecord, NumError> {
        FrameRecord::new(crs, step, origin, Rotation::IDENTITY, Scale::ONE)
    }

    /// The Map frame of one cell of a regional lattice (chapter 17.1): origin
    /// base + 2^39·index in easting and northing, at the base's height.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if an origin component exceeds [`ORIGIN_MAX`] in magnitude.
    pub fn lattice(
        crs: CrsId,
        step: GridStep,
        base: [i64; 3],
        index: [i64; 2],
    ) -> Result<FrameRecord, NumError> {
        let [be, bn, bh] = base;
        let [ie, in_] = index;
        let at = |b: i64, i: i64| -> Result<i64, NumError> {
            // |i|·2^39 saturates far beyond the origin bound, which then refuses it.
            let v = i128::from(b).saturating_add(i128::from(i).saturating_mul(1 << 39));
            i64::try_from(v).map_err(|_| NumError::OutOfRange { value: v })
        };
        let mut frame = FrameRecord::map(crs, step, [at(be, ie)?, at(bn, in_)?, bh])?;
        frame.lattice = Some(LatticeCell { base, index });
        Ok(frame)
    }

    /// The same frame with heights referred to the vertical coordinate reference system
    /// `vertical` (for example `CrsId::Epsg(7839)`, NZVD2016 height), or to none.
    #[must_use]
    pub const fn with_vertical_crs(self, vertical: Option<CrsId>) -> FrameRecord {
        FrameRecord {
            vertical_crs: vertical,
            ..self
        }
    }

    /// The same frame with map coordinates in `unit` (for example
    /// [`LinearUnit::US_SURVEY_FOOT`] for a US state plane system in feet).
    #[must_use]
    pub const fn with_linear_unit(self, unit: LinearUnit) -> FrameRecord {
        FrameRecord {
            linear_unit: unit,
            ..self
        }
    }

    /// The same frame with coordinates holding at the coordinate epoch `epoch`, or at none.
    #[must_use]
    pub const fn with_epoch(self, epoch: Option<DecimalYear>) -> FrameRecord {
        FrameRecord { epoch, ..self }
    }

    /// The same frame with observations carried to its epoch by the deformation model `model`, or
    /// by none.
    #[must_use]
    pub const fn with_deformation_model(self, model: Option<ModelRef>) -> FrameRecord {
        FrameRecord { model, ..self }
    }

    /// The horizontal coordinate reference system.
    #[must_use]
    pub const fn crs(&self) -> CrsId {
        self.horizontal_crs
    }

    /// The vertical coordinate reference system, if the frame names one.
    #[must_use]
    pub const fn vertical_crs(&self) -> Option<CrsId> {
        self.vertical_crs
    }

    /// The grid step.
    #[must_use]
    pub const fn step(&self) -> GridStep {
        self.step
    }

    /// The origin (easting, northing, height) in grid steps.
    #[must_use]
    pub const fn origin(&self) -> [i64; 3] {
        self.origin
    }

    /// Whether this is a Map frame or a Ground frame.
    #[must_use]
    pub const fn kind(&self) -> FrameKind {
        self.kind
    }

    /// The lattice cell, for a frame of a regional lattice.
    #[must_use]
    pub const fn lattice_cell(&self) -> Option<LatticeCell> {
        self.lattice
    }

    /// The rotation about z: the identity for a Map frame.
    #[must_use]
    pub const fn rotation(&self) -> Rotation {
        match self.kind {
            FrameKind::Map => Rotation::IDENTITY,
            FrameKind::Ground { rotation, .. } => rotation,
        }
    }

    /// The horizontal scale factor: one for a Map frame.
    #[must_use]
    pub const fn scale(&self) -> Scale {
        match self.kind {
            FrameKind::Map => Scale::ONE,
            FrameKind::Ground { scale, .. } => scale,
        }
    }

    /// The linear unit of the map coordinates: metres per CRS unit.
    #[must_use]
    pub const fn linear_unit(&self) -> LinearUnit {
        self.linear_unit
    }

    /// The coordinate epoch, if the frame names one.
    #[must_use]
    pub const fn epoch(&self) -> Option<DecimalYear> {
        self.epoch
    }

    /// The deformation model, if the frame names one.
    #[must_use]
    pub const fn deformation_model(&self) -> Option<ModelRef> {
        self.model
    }

    /// Whether this frame owns the horizontal position of `p` (in its own grid): a lattice frame
    /// owns the half-open cell −2^38 ≤ x, y < 2^38 (chapter 17.1), any other frame its whole range.
    #[must_use]
    pub fn owns(&self, p: P3) -> bool {
        if self.lattice.is_none() {
            return true;
        }
        let inside = |c: Coord| (-CELL_HALF..CELL_HALF).contains(&c.get());
        inside(p.x) && inside(p.y)
    }

    /// The frame `b` belongs to, with the exact shift into it (chapter 17.1): for a lattice frame,
    /// the lattice frame whose cell holds the centre of `b` (a box in this frame's grid, see
    /// [`FrameRecord::home_of`]); for any other frame, this frame and the identity shift.
    ///
    /// # Errors
    ///
    /// As [`FrameRecord::home_of`].
    pub fn home_frame(&self, b: &Box3) -> Result<(FrameRecord, FrameShift), MathError> {
        if self.lattice.is_none() {
            return Ok((*self, self.shift_to(self)?));
        }
        let home = FrameRecord::home_of(&[(*self, *b)])?;
        Ok((home, self.shift_to(&home)?))
    }

    /// The lattice frame in which an operation on these operands runs (chapter 17.1): the frame
    /// whose cell holds the centre of the operands' combined box. Each operand is a lattice frame
    /// and a box in that frame's grid.
    ///
    /// Every box is moved into lattice coordinates (steps from the lattice's base origin: a frame
    /// with cell index (i, j) adds (i·2^39, j·2^39), below 2^82 in magnitude, in `i128`), and the
    /// boxes are combined. The cell index i holds the centre c exactly when
    /// −2^38 ≤ c − i·2^39 < 2^38, that is i = ⌊(2c + 2^39)/2^40⌋, computed exactly from
    /// 2c = min + max; likewise j. The combined box must then lie within ±(2^40 − 1) of that cell's
    /// origin on every axis. The home frame has the operands' coordinate reference systems, linear
    /// unit, epoch and deformation model.
    ///
    /// # Errors
    ///
    /// [`MathError::Num`] with [`NumError::Degenerate`] for no operands;
    /// [`MathError::FrameMismatch`] if an operand is not a lattice frame (`NotOnLattice`), or its
    /// map coordinates differ from the first operand's (`Crs`, `VerticalCrs`, `LinearUnit`,
    /// `Epoch`, `DeformationModel`, in that order), or its lattice does: another step or base
    /// origin (`NotOnLattice`); [`MathError::FrameRangeExceeded`] if the combined box does not fit
    /// the frame that owns its centre (the operation must be split at a lattice line);
    /// [`MathError::Num`] with [`NumError::OutOfRange`] if that frame's origin would exceed
    /// [`ORIGIN_MAX`], or with [`NumError::BudgetExceeded`] only if a checked step failed, which
    /// the bounds above rule out.
    pub fn home_of(operands: &[(FrameRecord, Box3)]) -> Result<FrameRecord, MathError> {
        let Some(&(first, _)) = operands.first() else {
            return Err(MathError::Num(NumError::Degenerate));
        };
        let Some(base_cell) = first.lattice else {
            return Err(MathError::FrameMismatch(FrameMismatch::NotOnLattice));
        };
        let mut lo = [i128::MAX; 3];
        let mut hi = [i128::MIN; 3];
        for (frame, b) in operands {
            same_map(frame, &first)?;
            let Some(cell) = frame.lattice else {
                return Err(MathError::FrameMismatch(FrameMismatch::NotOnLattice));
            };
            if frame.step != first.step || cell.base != base_cell.base {
                return Err(MathError::FrameMismatch(FrameMismatch::NotOnLattice));
            }
            // |index|·2^39 < 2^102 and a corner below 2^40: every sum fits i128.
            let [ie, in_] = cell.index.map(i128::from);
            let shift = [
                budget(ie.checked_mul(SPACING))?,
                budget(in_.checked_mul(SPACING))?,
                0,
            ];
            let corners = |p: P3| [p.x.get(), p.y.get(), p.z.get()].map(i128::from);
            let (bmin, bmax) = (corners(b.min()), corners(b.max()));
            for (((l, h), (mn, mx)), s) in lo
                .iter_mut()
                .zip(hi.iter_mut())
                .zip(bmin.into_iter().zip(bmax))
                .zip(shift)
            {
                *l = (*l).min(budget(mn.checked_add(s))?);
                *h = (*h).max(budget(mx.checked_add(s))?);
            }
        }
        let [lx, ly, _] = lo;
        let [hx, hy, _] = hi;
        let cell_of = |l: i128, h: i128| -> Result<i128, NumError> {
            budget(
                l.checked_add(h)
                    .and_then(|sum| sum.checked_add(SPACING))
                    .and_then(|sum| sum.checked_div_euclid(TWICE_SPACING)),
            )
        };
        let (ce, cn) = (cell_of(lx, hx)?, cell_of(ly, hy)?);
        let index = |c: i128| {
            i64::try_from(c).map_err(|_| MathError::Num(NumError::OutOfRange { value: c }))
        };
        let home = FrameRecord::lattice(
            first.horizontal_crs,
            first.step,
            base_cell.base,
            [index(ce)?, index(cn)?],
        )?
        .with_vertical_crs(first.vertical_crs)
        .with_linear_unit(first.linear_unit)
        .with_epoch(first.epoch)
        .with_deformation_model(first.model);
        let max = u128::from(COORD_MAX.unsigned_abs());
        for (v, s) in lo.into_iter().chain(hi).zip([ce, cn, 0, ce, cn, 0]) {
            let local = budget(s.checked_mul(SPACING).and_then(|o| v.checked_sub(o)))?;
            if local.unsigned_abs() > max {
                return Err(MathError::FrameRangeExceeded { value: local });
            }
        }
        Ok(home)
    }

    /// The exact shift from this frame's grid to `target`'s: multiply by 25 per rung of refinement,
    /// then add an integer offset.
    ///
    /// With f = 25^(rungs) and ΔO = `f·O_self` − `O_target` (each component below 2^70), the offset is
    /// `k_d·R⁻¹(ΔO_E`, `ΔO_N)/k_n` horizontally, that is
    /// (`k_d·(c·ΔO_E` + `s·ΔO_N`), `k_d·(−s·ΔO_E` + `c·ΔO_N`)) / (`k_n·r`), and `ΔO_H` vertically (heights are
    /// never rotated or scaled). `c·ΔO_E` + `s·ΔO_N` is below 2^102 in `i128`; its product by
    /// `k_d` < 2^64 is below 2^166, formed exactly as a [`Big`], and divided by `k_n·r` < 2^96 by
    /// long division: a remainder means the frames are not on a shared lattice, whatever the
    /// quotient's size, and only then is the quotient's range checked.
    ///
    /// # Errors
    ///
    /// [`MathError::FrameMismatch`] if the coordinate reference systems, linear units, epochs,
    /// deformation models, rotations or scales differ (checked in that order), or the offset is not
    /// a whole number of steps along the target's axes; [`MathError::Num`] with
    /// [`NumError::WrongDirection`] if `target`'s step is coarser (an exact shift can only refine),
    /// or [`NumError::OutOfRange`] if an offset exceeds [`SHIFT_MAX`] (with the offset, saturated at
    /// the ends of `i128`), or [`NumError::BudgetExceeded`] only if a checked step failed, which
    /// the bounds above rule out.
    pub fn shift_to(&self, target: &FrameRecord) -> Result<FrameShift, MathError> {
        same_map(self, target)?;
        let rotation = self.rotation();
        let scale = self.scale();
        if rotation != target.rotation() {
            return Err(MathError::FrameMismatch(FrameMismatch::Rotation));
        }
        if scale != target.scale() {
            return Err(MathError::FrameMismatch(FrameMismatch::Scale));
        }
        let rungs = target
            .step
            .rung()
            .checked_sub(self.step.rung())
            .ok_or(MathError::Num(NumError::WrongDirection))?;
        let f: i128 = match rungs {
            0 => 1,
            1 => 25,
            _ => 625,
        };
        // |f·O| ≤ 625·2^60 < 2^70: every ΔO fits i128, and so do c·ΔO_E + s·ΔO_N (below 2^102)
        // and k_n·r (below 2^96).
        let delta = |a: i64, b: i64| -> Result<i128, NumError> {
            budget(
                f.checked_mul(i128::from(a))
                    .and_then(|fa| fa.checked_sub(i128::from(b))),
            )
        };
        let ([ae, an, ah], [be, bn, bh]) = (self.origin, target.origin);
        let (de, dn, dh) = (delta(ae, be)?, delta(an, bn)?, delta(ah, bh)?);
        let (c, s) = (i128::from(rotation.cos()), i128::from(rotation.sin()));
        let (ox, oy) = match rotation.exact_norm() {
            Some(r) => {
                let den = budget(u128::from(scale.num()).checked_mul(u128::from(r)))?;
                let kd = Big::from_i128(i128::from(scale.den()));
                let part = |a: i128| -> Result<i128, MathError> {
                    // |a| < 2^102, so a·k_d < 2^166 fits a Big; k_n·r < 2^96 is a valid divisor.
                    let num = budget(Big::from_i128(a).checked_mul(&kd))?;
                    let (q, rem) = budget(divrem_u128(&num, den))?;
                    if rem != 0 {
                        return Err(MathError::FrameMismatch(FrameMismatch::NotOnLattice));
                    }
                    // A quotient beyond i128 is reported saturated, as the numbers layer's
                    // OutOfRange documents.
                    Ok(q.to_i128().unwrap_or(if q.is_negative() {
                        i128::MIN
                    } else {
                        i128::MAX
                    }))
                };
                let combine = |a: i128, u: i128, b: i128, v: i128| -> Result<i128, NumError> {
                    budget(
                        a.checked_mul(u)
                            .zip(b.checked_mul(v))
                            .and_then(|(au, bv)| au.checked_add(bv)),
                    )
                };
                let ns = budget(s.checked_neg())?;
                (
                    part(combine(c, de, s, dn)?)?,
                    part(combine(c, dn, ns, de)?)?,
                )
            }
            // Irrational r: only a zero horizontal offset is a whole number of steps.
            None if de == 0 && dn == 0 => (0, 0),
            None => return Err(MathError::FrameMismatch(FrameMismatch::NotOnLattice)),
        };
        let bounded = |v: i128| -> Result<i64, MathError> {
            if v.unsigned_abs() > SHIFT_MAX.unsigned_abs().into() {
                return Err(MathError::Num(NumError::OutOfRange { value: v }));
            }
            i64::try_from(v).map_err(|_| MathError::Num(NumError::OutOfRange { value: v }))
        };
        Ok(FrameShift {
            from: self.step,
            to: target.step,
            // f is 1, 25 or 625: it fits i64, and a failure would be a defect, reported as one.
            refine: budget(i64::try_from(f).ok())?,
            offset: [bounded(ox)?, bounded(oy)?, bounded(dh)?],
        })
    }
}

/// An exact shift between two frames on a shared lattice: `target = refine · source + offset`,
/// componentwise, in the target's steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FrameShift {
    from: GridStep,
    to: GridStep,
    refine: i64,
    offset: [i64; 3],
}

impl FrameShift {
    /// The source frame's step.
    #[must_use]
    pub const fn source_step(self) -> GridStep {
        self.from
    }

    /// The target frame's step.
    #[must_use]
    pub const fn target_step(self) -> GridStep {
        self.to
    }

    /// The refinement factor: 1, 25 or 625.
    #[must_use]
    pub const fn refine(self) -> i64 {
        self.refine
    }

    /// The offset in target steps.
    #[must_use]
    pub const fn offset(self) -> [i64; 3] {
        self.offset
    }

    /// The point's coordinates in the target frame, exactly.
    ///
    /// Budget: |refine · c| ≤ 625·(2^40 − 1) < 2^50 and |offset| ≤ 2^62, so the sum fits `i64`
    /// before its range check.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if the point lies outside the target frame's ±(2^40 − 1) range.
    /// [`NumError::BudgetExceeded`] only if a checked step failed, which the budget rules out.
    pub fn apply(&self, p: P3) -> Result<P3, NumError> {
        let [ox, oy, oz] = self.offset;
        let shift = |c: Coord, o: i64| -> Result<Coord, NumError> {
            let moved = self
                .refine
                .checked_mul(c.get())
                .and_then(|rc| i128::from(rc).checked_add(i128::from(o)));
            Coord::from_i128(budget(moved)?)
        };
        Ok(P3 {
            x: shift(p.x, ox)?,
            y: shift(p.y, oy)?,
            z: shift(p.z, oz)?,
        })
    }
}
