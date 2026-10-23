//! Frames: the record that ties a model's flat grid to a map projection, conversion between map
//! coordinates and the grid, exact shifts between frames on a shared lattice, and the regional
//! lattice of frames for long networks.
//!
//! A frame is the kernel's flat coordinate space for one model or one piece of a network (chapters
//! 6.1 and 17.1). Its [`FrameRecord`] holds the grid step, the horizontal coordinate reference
//! system's id, an optional vertical one, the origin in grid steps, the [`FrameKind`] (a Map frame,
//! or a Ground frame with a rotation about z and a horizontal scale factor) and, for a frame of a
//! regional network, its [`LatticeCell`]. Its meaning, for a point with frame grid coordinates
//! (x, y, z) and map coordinates (E, N, H) in metres:
//!
//! ```text
//! E·S = O_E + k·(cos θ·x − sin θ·y)
//! N·S = O_N + k·(sin θ·x + cos θ·y)        S = grid steps per metre (6,400,000 at q0)
//! H·S = O_H + z                            heights are never rotated or scaled
//! ```
//!
//! where (`O_E`, `O_N`, `O_H`) is the origin in steps, θ the angle of the rotation's direction (c, s)
//! from the easting axis (cos θ = c/r, sin θ = s/r, r = √(c² + s²)), and k = `k_n/k_d` the scale
//! factor. A Map frame has θ = 0 and k = 1.
//!
//! **The NZTM2000 example.** NZTM2000 (EPSG:2193) is a transverse Mercator projection with false
//! northing 10,000,000 m; central Auckland is near N 5,921,000 m, which is about 2^45 steps at q0,
//! too far from the grid's zero for the 40-bit exact predicates. So a frame's origin sits at the
//! site, as a whole number of steps (up to [`ORIGIN_MAX`] = 2^60), and every model coordinate is a
//! short offset from it.
//!
//! **Map to frame** ([`FrameRecord::to_frame`]) rounds once, to the nearest step, ties to even, and
//! reports whether the conversion was exact, for every frame. Coordinates beyond ±(2^40 − 1) steps
//! return [`NumError::OutOfRange`].
//!
//! - *Map frames, and frames rotated by quarter turns with scale one.* E·S is computed exactly as a
//!   double-double by `two_prod` (S is an integer below 2^32), the origin is an integer subtracted
//!   inside the exact rounding of [`DD::round_shifted`], and a quarter turn only swaps and negates.
//!   Nothing rounds before the final rounding, so the result is the exact value correctly rounded,
//!   and `exact` says whether it already was a whole step. Metres given to 0.01 mm convert exactly
//!   when their binary value is exact (one millimetre is 6,400 steps).
//! - *Other rotations or scales (Ground frames).* The value is computed in double-double from the
//!   exact E·S: subtract the origin, rotate by the integer direction, multiply by `k_d` and divide by
//!   `r·k_n` (with r a double-double square root when c² + s² is not a perfect square). The error
//!   derivation on [`FrameRecord::to_frame`] bounds the double-double value within 2^−59 steps of
//!   the exact value for every in-range result; the conversion uses the far looser 2^−40. Every
//!   value within 2^−40 of a whole step or of a midpoint is settled exactly, on the input doubles'
//!   exact values: by integer arithmetic (in 1280-bit integers) when r is an integer, and by the
//!   exact sign of A − T·√(c² + s²), comparing A² with T²·(c² + s²) in 2560-bit integers, when r
//!   is irrational. So the result is always the exact value correctly rounded, ties to even, and
//!   `exact` is exact. When r is irrational no tie can occur, and a coordinate is exact only when
//!   it is zero (then c·u + s·v = 0 exactly).
//!
//! Because each frame rounds its own coordinates to even, a map value exactly halfway between two
//! steps can round to different map positions in two frames whose origins differ by an odd number
//! of steps. Values that land exactly on a step, or not on a midpoint, convert consistently.
//!
//! **Frame to map** ([`FrameRecord::to_map`]) returns the nearest doubles to the exact map
//! coordinates, ties to even, with whether they are exact, for every frame: by exact integer
//! division when the horizontal transform is rational; for an irrational direction by a
//! double-double value certified against its proven error, falling back, when the certificate
//! fails (for example when the origin cancels most of the rotated offset), to a bisection over the
//! doubles decided by exact signs of A − B·√(c² + s²). An irrational map coordinate is never exact
//! and never a tie. At q0 a map-frame round trip frame → map → frame is the identity for every
//! coordinate below 2^29 m, because an ulp of the map value is then smaller than a step.
//!
//! **Lattices** ([`FrameRecord::shift_to`]). Frames with the same coordinate reference systems,
//! rotation and scale whose origins differ by whole steps along the target's axes are on a shared
//! lattice: moving a point between them is multiplying by 25 per rung of refinement (exact,
//! chapter 6.1) and adding an integer offset. Anything else is refused with [`FrameMismatch`]:
//! frames are never mixed without an exact shift.
//!
//! **Regional networks** ([`FrameRecord::lattice`], chapter 17.1). A network wider than one frame
//! uses Map frames with origins every [`LATTICE_SPACING`] = 2^39 steps (85.9 km at q0) in easting
//! and northing from a base origin. Neighbours overlap by three quarters of a frame's width, so a
//! point moves between them by an exact integer shift. Each frame owns the half-open cell within
//! [`CELL_HALF`] = 2^38 steps of its origin ([`FrameRecord::owns`]); geometry belongs to the frame
//! whose cell holds the centre of its box ([`FrameRecord::home_frame`]), an operation runs in the
//! frame whose cell holds the centre of its operands' combined box ([`FrameRecord::home_of`]), and
//! operands that do not fit that frame are refused with [`MathError::FrameRangeExceeded`].
//!
//! **What this record does not hold yet.** Chapter 17.1's record also has a linear unit (metres per
//! CRS unit, such as US survey feet), a coordinate epoch and a deformation model, and stores a
//! Ground frame's rotation as whole angle units turned into sine and cosine by proven `sinpi` and
//! `cospi`. Those belong to pieces not yet built: units and epochs to G48's remaining scope, angle
//! units to G359 (in `geom-num`) and the proven maths functions to G47 (piece G05). Until then: map
//! coordinates are in metres; a record names its datum only through its CRS ids, so it must not be
//! used for observations at different epochs of a dynamic datum, whose frames would compare equal;
//! and a Ground frame's rotation is an exact integer direction, chapter 17.2's second direction
//! form (fine to about 2^−31 rad with 32-bit components), for which every conversion above is
//! exact or correctly rounded.
//!
//! Bit budgets (|c|, |s| ≤ 2^31; `k_n`, `k_d` < 2^32; integer r < 2^32; |O| ≤ 2^60; S < 2^32;
//! |x|, |y| ≤ 2^40; |E|, |N| ≤ 2^62 m) are derived where each integer is formed. Every integer
//! step is a checked method, never a native operator, so nothing depends on overflow checks being
//! on; a step its budget proves fits can fail only through a defect in this crate, and is then
//! reported as [`NumError::BudgetExceeded`], never answered: no exact sign that fails is replaced
//! by a guess, and no conversion returns a value it could not certify.
//!
//! Method: the frame record and conversion rules of chapter 17.1
//! (<https://github.com/spareware-co-op/spareware/blob/piece/G01/projects/engines/geometry/architecture/17-survey-terrain-civil.md>),
//! with EPSG:2193 as given in PROJ's copy of the EPSG data
//! (<https://github.com/OSGeo/PROJ/blob/5fa31c29f0a806568ed7bc76c1a557604db4a804/data/sql/projected_crs.sql#L375>);
//! double-double arithmetic as in [`crate::dd`]; the exact settling as the filter-then-exact
//! pattern of the kernel's predicates (chapter 6.2; J. R. Shewchuk, *Discrete & Computational
//! Geometry* 18 (1997), <https://doi.org/10.1007/PL00009321>), with exact values of doubles read as
//! dyadic rationals (IEEE 754-2019, <https://doi.org/10.1109/IEEESTD.2019.8766229>) and numbers
//! a + b√n compared exactly by their squares (C. K. Yap, *Computational Geometry* 7 (1997),
//! <https://doi.org/10.1016/0925-7721(95)00040-2>).
//!
//! Cost: no allocation, and constant time for every operation except
//! [`FrameRecord::home_of`], which is linear in the number of operands.
//!
//! - Records and accessors: a few comparisons. [`Rotation::direction`] and [`Scale::new`] reduce
//!   32-bit integers by their greatest common divisor, at most 65 remainder steps.
//! - [`FrameRecord::to_frame`]: in a Map frame, or one turned by quarter turns at scale one, three
//!   exact products (`two_prod`) and three exact roundings ([`DD::round_shifted`]). In any other
//!   Ground frame, fewer than twenty double-double operations; then a horizontal coordinate whose
//!   value lies within 2^−40 of a whole step or of a midpoint (rare for measured data, usual for
//!   exact inputs) takes up to three exact signs, each five or six multiplications of integers of
//!   up to 1280 bits (20 limbs) and, for an irrational direction, three more of 2560 bits.
//! - [`FrameRecord::to_map`]: in a Map frame or a frame with a rational rotation, `i128`
//!   arithmetic and three correctly rounded quotients (at most 181 shift-and-subtract steps each).
//!   For an irrational direction, five double-double operations per horizontal coordinate; when
//!   the certificate fails, a bisection of at most 70 halvings and three more exact signs (the
//!   sign of the value and the two ends of its bracket), each sign four multiplications of
//!   1280-bit integers.
//! - [`FrameRecord::shift_to`] and [`FrameShift::apply`]: a few `i128` products and divisions and
//!   three range checks.

#![allow(
    clippy::many_single_char_names,
    reason = "variable names follow the formulas of the method each function cites"
)]

use geom_num::grid::{COORD_MAX, Coord, GridStep, P3, QUARTER_NM_PER_MM};
use geom_num::sign::Sign;
use geom_num::{NumError, math};

use crate::bbox::Box3;
use crate::dd::DD;
use crate::error::{FrameMismatch, MathError, budget};
use crate::exact::{
    Big, const_pow2, dyadic, gcd_u128, pow2_big, rational_to_f64, shl_into, sign_minus_sqrt,
};
use crate::rounded::Rounded;

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

/// The largest map coordinate accepted, 2^62 m: any larger value is out of every frame's range,
/// since |E·S − O| ≥ 2^62·6.4·10^6 − 2^60 > 2^40 steps.
const MAP_MAX: f64 = const_pow2!(62);

/// Double-double values beyond 2^41 steps in magnitude are reported out of range before rounding.
const RANGE_FILTER: f64 = const_pow2!(41);

/// The settling margin, 2^−40 steps: a double-double value farther than this from every whole step
/// and every midpoint rounds the same way as the exact value, because its error is below 2^−59.
const FILTER: f64 = const_pow2!(-40);

/// The smallest magnitude an irrational map coordinate can have, 2^−220 m (see
/// [`FrameRecord::to_map`]): every bisection stays above it.
const IRRATIONAL_MIN: f64 = const_pow2!(-220);

/// The largest magnitude any map coordinate can have, 2^84 m (see [`FrameRecord::to_map`]).
const MAP_RESULT_MAX: f64 = const_pow2!(84);

/// The certificate's relative margin in [`FrameRecord::to_map`], 2^−98 = 256u².
const CERTIFICATE_MARGIN: f64 = const_pow2!(-98);

/// The widening of the bisection's bracket in [`FrameRecord::to_map`], 2^−50.
const BRACKET_WIDENING: f64 = const_pow2!(-50);

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

/// A horizontal scale factor k = num/den, exact and stored reduced: any positive ratio of 32-bit
/// integers, for example 0.99987654 = `Scale::new(99_987_654, 100_000_000)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Scale {
    num: u32,
    den: u32,
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
    pub fn new(num: u32, den: u32) -> Result<Scale, NumError> {
        if num == 0 || den == 0 {
            return Err(NumError::Degenerate);
        }
        // g ≥ 1 divides both, so each quotient is at most the original and fits u32.
        let g = gcd_u128(u128::from(num), u128::from(den));
        let part = |v: u32| -> Result<u32, NumError> {
            budget(
                u128::from(v)
                    .checked_div(g)
                    .and_then(|q| u32::try_from(q).ok()),
            )
        };
        Ok(Scale {
            num: part(num)?,
            den: part(den)?,
        })
    }

    /// The numerator `k_n`.
    #[must_use]
    pub const fn num(self) -> u32 {
        self.num
    }

    /// The denominator `k_d`.
    #[must_use]
    pub const fn den(self) -> u32 {
        self.den
    }

    /// Whether the factor is exactly one.
    #[must_use]
    pub const fn is_one(self) -> bool {
        self.num == 1 && self.den == 1
    }
}

/// Map coordinates in metres: easting, northing and height above the vertical datum.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapPoint {
    /// Easting, in metres.
    pub easting: f64,
    /// Northing, in metres.
    pub northing: f64,
    /// Height, in metres.
    pub height: f64,
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
/// coordinate reference systems, origin in steps, kind and lattice cell. Immutable; made only by
/// the constructors, which check the origin's range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FrameRecord {
    step: GridStep,
    horizontal_crs: CrsId,
    vertical_crs: Option<CrsId>,
    origin: [i64; 3],
    kind: FrameKind,
    lattice: Option<LatticeCell>,
}

/// A map coordinate times the steps per metre, exactly, as a double-double; or the knowledge that
/// it is a nonzero value below 2^−968 in magnitude (`tiny`), where `two_prod` cannot be exact and
/// `dd` is zero.
#[derive(Clone, Copy)]
struct Scaled {
    dd: DD,
    tiny: bool,
}

impl Scaled {
    /// `v · s`, for a map coordinate `v` in metres and `s` steps per metre.
    fn new(v: f64, s: f64) -> Result<Scaled, NumError> {
        if !v.is_finite() {
            return Err(NumError::NotFinite);
        }
        if v > MAP_MAX || v < -MAP_MAX {
            // A saturating conversion: the value only reports how far out of range it was.
            #[allow(
                clippy::cast_possible_truncation,
                reason = "a saturating conversion on purpose: the value only reports how far out of \
                          range the coordinate was"
            )]
            let estimate = (v * s) as i128;
            return Err(NumError::OutOfRange { value: estimate });
        }
        // |v| ≤ 2^62 and 2^22 < s < 2^32, so the product cannot overflow; exact_product refuses
        // only a subnormal v or a product below about 2^−968, which is the tiny case.
        Ok(match DD::exact_product(v, s) {
            Some(dd) => Scaled { dd, tiny: false },
            None => Scaled {
                dd: DD::ZERO,
                tiny: true,
            },
        })
    }
}

/// `±(scaled − origin)` rounded to the nearest step, ties to even, exactly.
///
/// A tiny scaled value is a nonzero number below 2^−968 in magnitude, so ±(tiny − origin) rounds
/// to ∓origin, never a tie, and is not exact.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "negating a double-double flips both parts' signs, exactly"
)]
fn round_axis(v: Scaled, origin: i64, negate: bool) -> Result<Rounded<Coord>, NumError> {
    let (dd, offset) = if negate {
        (-v.dd, budget(i128::from(origin).checked_neg())?)
    } else {
        (v.dd, i128::from(origin))
    };
    // |dd| < 2^95, far inside round_shifted's 2^100, and |offset| ≤ 2^60, so it always answers.
    let r = budget(dd.round_shifted(offset))?;
    Ok(Rounded::new(
        Coord::from_i128(r.value())?,
        r.is_exact() && !v.tiny,
    ))
}

/// The nearest even of two neighbouring integers.
const fn even(a: i128, b: i128) -> i128 {
    if a.rem_euclid(2) == 0 { a } else { b }
}

/// The exact inputs of a Ground-frame conversion, for settling ties and exactness.
///
/// For a coordinate whose exact value is X = `k_d·(α·u` + `β·v)/(k_n·ρ`), with u = E·S − `O_E`,
/// v = N·S − `O_N` and ρ = √(c² + s²), and a candidate half-integer t/2, the sign of X − t/2 is the
/// sign of F = `2·k_d·(α·u` + β·v) − `t·k_n·ρ` (`k_n`, ρ > 0). E = `m_E·2^k_E` and N = `m_N·2^k_N` exactly,
/// so with B = max(0, −`k_E`, −`k_N`), F·2^B = A − T·ρ with the integers
///
/// ```text
/// A = 2·k_d·(α·m_E·S·2^(k_E+B) + β·m_N·S·2^(k_N+B) − (α·O_E + β·O_N)·2^B),   T = t·k_n·2^B
/// ```
///
/// When ρ = r is an integer, A − T·r is formed exactly; otherwise its sign is the sign of
/// A − T·√(c² + s²), decided by comparing A² with T²·(c² + s²).
///
/// Budget (|E|, |N| ≤ 2^62, so |m|·2^k ≤ 2^62; B ≤ 1074; |α|, |β| ≤ 2^31; S, `k_n`, `k_d`, r < 2^32;
/// |O| ≤ 2^60; |t| ≤ 2^42 + 1): α·m·S < 2^116 fits `i128`; each scaled term is at most
/// 2^31·2^32·2^62·2^1074 = 2^1199; their sum, times `2k_d` < 2^33, is below 2^1233; the integer
/// part (|`α·O_E` + `β·O_N`| < 2^92, times `2k_d`) times 2^B is below 2^1200. So |A| < 2^1234,
/// |T| < 2^43·2^32·2^1074 = 2^1149 and |T·r| < 2^1181, inside [`Big`]'s ±2^1279. For the
/// irrational test, A² < 2^2468 and T²·(c² + s²) < 2^2298·2^63 = 2^2361, inside `Wide<40>`'s
/// ±2^2559.
struct ExactInputs {
    me: i64,
    ke: i32,
    mn: i64,
    kn_exp: i32,
    s: i64,
    oe: i64,
    on: i64,
    kd: u32,
    kn: u32,
    r: Option<u32>,
    n2: u64,
    b: u32,
}

impl ExactInputs {
    /// The inputs for finite map coordinates (e, n) in a frame. Every step fits: a finite double
    /// is a dyadic number, an exponent from −1074 to 971 negates within `i32`, and S < 2^32 fits
    /// `i64`; a failure is reported as [`NumError::BudgetExceeded`].
    fn new(frame: &FrameRecord, e: f64, n: f64) -> Result<ExactInputs, NumError> {
        let (me, ke) = budget(dyadic(e))?;
        let (mn, kn_exp) = budget(dyadic(n))?;
        let lift = |m: i64, k: i32| -> Result<i32, NumError> {
            if m == 0 {
                Ok(0)
            } else {
                Ok(budget(k.checked_neg())?.max(0))
            }
        };
        let b = budget(u32::try_from(lift(me, ke)?.max(lift(mn, kn_exp)?)).ok())?;
        let [oe, on, _] = frame.origin;
        let rotation = frame.rotation();
        let scale = frame.scale();
        Ok(ExactInputs {
            me,
            ke,
            mn,
            kn_exp,
            s: budget(i64::try_from(steps_per_metre(frame.step)).ok())?,
            oe,
            on,
            kd: scale.den,
            kn: scale.num,
            r: rotation.exact_norm(),
            n2: rotation.norm2(),
            b,
        })
    }

    /// The integers A and T of the type's documentation, for the direction (α, β) and the
    /// candidate t/2; `None` if a checked operation overflowed, which the budget rules out.
    fn parts(&self, alpha: i64, beta: i64, t: i128) -> Option<(Big, Big)> {
        let big = Big::from_i128;
        let b = i32::try_from(self.b).ok()?;
        let term = |a: i64, m: i64, k: i32| -> Option<Big> {
            if a == 0 || m == 0 {
                return Some(Big::ZERO);
            }
            let coef = i128::from(a)
                .checked_mul(i128::from(m))?
                .checked_mul(i128::from(self.s))?;
            let shift = u32::try_from(k.checked_add(b)?).ok()?;
            big(coef).checked_mul(&pow2_big(shift)?)
        };
        let lin = term(alpha, self.me, self.ke)?.checked_add(&term(beta, self.mn, self.kn_exp)?)?;
        let ints = i128::from(alpha)
            .checked_mul(i128::from(self.oe))?
            .checked_add(i128::from(beta).checked_mul(i128::from(self.on))?)?;
        let pb = pow2_big(self.b)?;
        let inner = lin.checked_sub(&big(ints).checked_mul(&pb)?)?;
        let a = inner.checked_mul(&big(i128::from(self.kd).checked_mul(2)?))?;
        let tk = t.checked_mul(i128::from(self.kn))?;
        Some((a, big(tk).checked_mul(&pb)?))
    }

    /// The exact sign of X − t/2 for the direction (α, β) (see the type's documentation), or
    /// `None` if a checked operation overflowed, which the budget rules out.
    fn sign(&self, alpha: i64, beta: i64, t: i128) -> Option<Sign> {
        let (a, tt) = self.parts(alpha, beta, t)?;
        if let Some(r) = self.r {
            let tr = tt.checked_mul(&Big::from_i128(i128::from(r)))?;
            Some(a.checked_sub(&tr)?.signum())
        } else {
            let (a, tt) = (a.resize::<40>()?, tt.resize::<40>()?);
            sign_minus_sqrt::<40>(&a, &tt, self.n2)
        }
    }
}

/// The candidate from a double-double value: `OutOfRange` beyond 2^41 steps, else the value
/// rounded to nearest, ties to even (exact rounding of the double-double, which always answers
/// within 2^41).
fn candidate(d: DD) -> Result<Rounded<i128>, NumError> {
    if d.hi() > RANGE_FILTER || d.hi() < -RANGE_FILTER {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "a saturating conversion on purpose: the double-double's leading part, an \
                      estimate of the coordinate that is out of range"
        )]
        let estimate = d.hi() as i128;
        return Err(NumError::OutOfRange { value: estimate });
    }
    budget(d.round_shifted(0))
}

/// The exact rounding of a coordinate from its double-double candidate `x` (the double-double
/// value `d` rounded) and the exact sign of X − t/2.
///
/// The double-double value lies within 2^−59 of X (see [`FrameRecord::to_frame`]), so X lies
/// within 1/2 + 2^−59 of x and its rounding is x − 1, x or x + 1. If `d` is farther than
/// [`FILTER`] from x and from both midpoints x ± 1/2, X is neither a whole step nor a midpoint and
/// rounds to x, inexactly. Otherwise exact signs decide: X = x is exact; X > x + 1/2 rounds up,
/// X = x + 1/2 is a tie; and likewise below. A failed exact sign (ruled out by the budget) is
/// reported as [`NumError::BudgetExceeded`], never replaced by the candidate; so are the steps
/// x ± 1 and 2x ± 1, which fit (|x| ≤ 2^41), and the magnitude of the finite fraction.
fn settle(
    cand: Rounded<i128>,
    d: DD,
    sign: impl Fn(i128) -> Option<Sign>,
) -> Result<Rounded<i128>, NumError> {
    let x = cand.value();
    let inexact = |value: i128| Rounded::new(value, false);
    // |x| ≤ 2^41, so x is a double exactly and hi − x is exact (Sterbenz, or x = 0).
    #[allow(
        clippy::cast_precision_loss,
        reason = "|x| ≤ 2^41 < 2^53: exact in f64"
    )]
    let xf = x as f64;
    let frac = math::abs((d.hi() - xf) + d.lo())?.get();
    if frac > FILTER && frac < 0.5 - FILTER {
        return Ok(inexact(x));
    }
    let two_x = budget(x.checked_mul(2))?;
    let (above, below) = (budget(two_x.checked_add(1))?, budget(two_x.checked_sub(1))?);
    let (up, down) = (budget(x.checked_add(1))?, budget(x.checked_sub(1))?);
    if budget(sign(two_x))? == Sign::Zero {
        return Ok(Rounded::new(x, true));
    }
    match budget(sign(above))? {
        Sign::Positive => return Ok(inexact(up)),
        Sign::Zero => return Ok(inexact(even(x, up))),
        Sign::Negative => {}
    }
    Ok(match budget(sign(below))? {
        Sign::Negative => inexact(down),
        Sign::Zero => inexact(even(down, x)),
        Sign::Positive => inexact(x),
    })
}

/// A rounded `i128` coordinate as a range-checked [`Coord`].
fn to_coord(r: Rounded<i128>) -> Result<Rounded<Coord>, NumError> {
    Ok(Rounded::new(Coord::from_i128(r.value())?, r.is_exact()))
}

/// The nearest double to `num / den` as a rounded value; the divisions here always have
/// 0 < den < 2^127, so it always answers, and a refusal is reported as
/// [`NumError::BudgetExceeded`].
fn quotient(num: i128, den: u128) -> Result<Rounded<f64>, NumError> {
    budget(rational_to_f64(num, den))
}

/// One horizontal map coordinate of a frame with an irrational direction, as exact integers:
/// V = (O + `k_n·a/(k_d·√n))/S` metres, with a ≠ 0 the rotated integer offset, n = c² + s² not a
/// square.
struct IrrationalMap {
    origin: i128,
    kn_a: i128,
    kd: i128,
    s: i128,
    n2: u64,
}

impl IrrationalMap {
    /// The exact sign of V − w for w = p·2^f.
    ///
    /// V − w = (O + q − w·S)/S with q = `k_n·a/(k_d·√n)`, so its sign is the sign of
    /// `k_n·a` − (w·S − `O)·k_d·√n` (multiplying by `S·k_d·√n` > 0). For f < 0 both sides are
    /// multiplied by 2^(−f): A = `k_n·a·2^(−f)` and B = (p·S − `O·2^(−f))·k_d`; for f ≥ 0, A = `k_n·a`
    /// and B = (p·S·2^f − `O)·k_d`. The sign of A − B·√n is decided exactly by comparing squares.
    ///
    /// Budget, for the w the conversion uses (zero, the two ends of the bracket and the midpoints
    /// between neighbouring doubles inside it, so 2^−221 ≤ |w| ≤ 2^84 otherwise, with |p| < 2^55
    /// and 2^−276 ≤ 2^f ≤ 2^84): |`k_n·a`| < 2^104 and |p·S| < 2^87 in `i128`; for f < 0,
    /// |A| < 2^380 and |B| < (2^87 + 2^60·2^276)·2^32 < 2^369; for f ≥ 0, |A| < 2^104 and, since
    /// |p·2^f| = |w| ≤ 2^84, |B| < (2^116 + 2^60)·2^32 < 2^149. So A² < 2^760 and
    /// B²·n < 2^738·2^63 = 2^801, inside [`Big`]'s ±2^1279.
    fn sign_minus(&self, p: i128, f: i32) -> Option<Sign> {
        let kd = Big::from_i128(self.kd);
        let ps = p.checked_mul(self.s)?;
        let (a, b) = if f < 0 {
            let k = u32::try_from(f.checked_neg()?).ok()?;
            let a = shl_into::<20>(self.kn_a, k)?;
            let o = shl_into::<20>(self.origin, k)?;
            (a, Big::from_i128(ps).checked_sub(&o)?.checked_mul(&kd)?)
        } else {
            let k = u32::try_from(f).ok()?;
            let w = shl_into::<20>(ps, k)?;
            let b = w
                .checked_sub(&Big::from_i128(self.origin))?
                .checked_mul(&kd)?;
            (Big::from_i128(self.kn_a), b)
        };
        sign_minus_sqrt::<20>(&a, &b, self.n2)
    }

    /// The exact sign of |V| − w for a positive dyadic w = p·2^f, given whether V is negative.
    fn abs_minus(&self, negative: bool, p: i128, f: i32) -> Option<Sign> {
        if negative {
            self.sign_minus(p.checked_neg()?, f).map(Sign::negate)
        } else {
            self.sign_minus(p, f)
        }
    }

    /// The nearest double to V, by bisection over the positive doubles in [lo, hi] (a bracket of
    /// |V|, checked exactly, with the proven bounds as the fallback) decided by exact signs at the
    /// midpoints between neighbouring doubles. Positive doubles are ordered like their bit
    /// patterns, so at most 64 halvings find the double whose rounding interval holds |V|; V is
    /// irrational, so it is never a midpoint. `None` only if a checked step failed, which the
    /// budgets of [`IrrationalMap::sign_minus`] rule out.
    fn nearest(&self, lo: f64, hi: f64) -> Option<f64> {
        let negative = self.sign_minus(0, 0)? == Sign::Negative;
        let holds = |w: f64, want: Sign| -> Option<bool> {
            let (m, e) = dyadic(w)?;
            Some(self.abs_minus(negative, i128::from(m), e)? == want)
        };
        let lo = if holds(lo, Sign::Positive)? {
            lo
        } else {
            IRRATIONAL_MIN
        };
        let hi = if holds(hi, Sign::Negative)? {
            hi
        } else {
            MAP_RESULT_MAX
        };
        let (mut a, mut b) = (lo.to_bits(), hi.to_bits());
        for _ in 0..70 {
            if a >= b {
                break;
            }
            // a < b, both bit patterns of positive finite doubles: every step below fits.
            let mid = a.checked_add(b.checked_sub(a)?.checked_div(2)?)?;
            let next = mid.checked_add(1)?;
            let (m1, e1) = dyadic(f64::from_bits(mid))?;
            let (m2, e2) = dyadic(f64::from_bits(next))?;
            let e0 = e1.min(e2);
            // The midpoint (d₁ + d₂)/2 = p·2^(e0 − 1), |p| < 2^55: each shift is below 128.
            let shift = |m: i64, e: i32| -> Option<i128> {
                i128::from(m).checked_shl(u32::try_from(e.checked_sub(e0)?).ok()?)
            };
            let p = shift(m1, e1)?.checked_add(shift(m2, e2)?)?;
            if self.abs_minus(negative, p, e0.checked_sub(1)?)? == Sign::Positive {
                a = next;
            } else {
                b = mid;
            }
        }
        let magnitude = f64::from_bits(a);
        Some(if negative { -magnitude } else { magnitude })
    }
}

impl FrameRecord {
    /// A frame record: a Map frame when the rotation is the identity and the scale one, otherwise a
    /// Ground frame; no vertical CRS and no lattice cell (see [`FrameRecord::with_vertical_crs`]
    /// and [`FrameRecord::lattice`]).
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

    /// Map coordinates in metres converted to frame grid coordinates, rounded once to the nearest
    /// step, ties to even, with whether no rounding happened (see the module documentation).
    ///
    /// Error bound of the double-double value in a Ground frame, which the settling relies on
    /// (u = 2^−53; `B_a`, `B_m`, `B_d` and `B_s` the proven bounds of `add`, `mul_f64`, `div` and `sqrt` in
    /// [`crate::dd`]). u = E·S − `O_E` and v = N·S − `O_N` are one `sub` each of exact values:
    /// u(1 + `η_u`), v(1 + `η_v`), |η| ≤ `B_a`. Each product by c or s adds a relative `B_m` and their sum
    /// a relative `B_a` of the sum, whose magnitude is at most |c||u| + |s||v|, so the rotated
    /// numerator is within K₁·(|c||u| + |s||v|) of its exact value, with
    /// K₁ = `B_a` + (1 + `B_a)(B_a` + `B_m` + `B_a·B_m`). The product by `k_d`, the divisor `ρ·k_n` (exact when
    /// ρ = r is an integer, else a square root and a product) and the division multiply by a factor
    /// F with |F − 1| ≤ (1 + `B_m)(1` + `B_d)/((1` − `B_s)(1` − `B_m`)) − 1. Since
    /// `k_d(|c||u`| + |`s||v|)/(k_n·ρ`) ≤ `k_d·|(u`, `v)|/k_n` = |(x, y)| by Cauchy–Schwarz ((u, v) is k
    /// times a rotation of (x, y)), the value is within (|F − 1| + K₁(1 + |F − 1|))·|(x, y)| of
    /// the exact coordinate, below 31.2u²·|(x, y)| (evaluated exactly at u = 2^−53). Every
    /// in-range result has |x|, |y| ≤ 2^41, so the error is below 31.2·2^−106·2^41.5 < 2^−59
    /// steps. A tiny input (one whose product with S is below 2^−968, taken as zero) moves the
    /// value by less than 2^−966 steps more, and an underflow anywhere by a few multiples of
    /// 2^−1074 times `k_d` < 2^32 (see the double-double module's *Range*): far below the bound.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if a coordinate is NaN or infinite; [`NumError::OutOfRange`] if a
    /// result lies beyond ±(2^40 − 1) steps. The error's value is the rounded coordinate wherever
    /// the conversion computes one (every coordinate of a map value within 2^62 m, except a Ground
    /// frame's horizontal ones beyond 2^41 steps), and otherwise an estimate: for a map value beyond
    /// 2^62 m, that value times the steps per metre, converted with saturation, before the origin,
    /// rotation and scale; for a Ground frame's horizontal coordinate beyond 2^41 steps, its
    /// double-double value truncated. Those estimates are not the exact value the numbers layer
    /// documents for `OutOfRange`, and a saturated one does not certify the "at least" or "at
    /// most" its message states. [`NumError::BudgetExceeded`] only if a checked step or an exact
    /// sign failed, which the bit budgets rule out.
    pub fn to_frame(self, map: MapPoint) -> Result<Rounded<P3>, NumError> {
        #[allow(
            clippy::cast_precision_loss,
            reason = "the steps per metre are below 2^32 < 2^53: exact in f64"
        )]
        let s = steps_per_metre(self.step) as f64;
        let [oe, on, oh] = self.origin;
        let e = Scaled::new(map.easting, s)?;
        let n = Scaled::new(map.northing, s)?;
        let h = Scaled::new(map.height, s)?;
        let z = round_axis(h, oh, false)?;
        let (x, y) = match (self.rotation().quarter_turn_count(), self.scale().is_one()) {
            (Some(turns), true) => match turns {
                0 => (round_axis(e, oe, false)?, round_axis(n, on, false)?),
                1 => (round_axis(n, on, false)?, round_axis(e, oe, true)?),
                2 => (round_axis(e, oe, true)?, round_axis(n, on, true)?),
                _ => (round_axis(n, on, true)?, round_axis(e, oe, false)?),
            },
            _ => self.ground_to_frame(map, e, n)?,
        };
        Ok(Rounded::new(
            P3 {
                x: x.value(),
                y: y.value(),
                z: z.value(),
            },
            x.is_exact() && y.is_exact() && z.is_exact(),
        ))
    }

    /// The horizontal part of [`FrameRecord::to_frame`] for a rotation other than a quarter turn
    /// or a scale other than one.
    #[allow(
        clippy::single_match_else,
        reason = "the two cases each carry their bound in a comment, side by side"
    )]
    fn ground_to_frame(
        &self,
        map: MapPoint,
        e: Scaled,
        n: Scaled,
    ) -> Result<(Rounded<Coord>, Rounded<Coord>), NumError> {
        let [oe, on, _] = self.origin;
        let rotation = self.rotation();
        let scale = self.scale();
        let (c, s) = (rotation.cos, rotation.sin);
        let (cf, sf) = (f64::from(c), f64::from(s));
        let u = e.dd.sub(DD::from_i64(oe))?;
        let v = n.dd.sub(DD::from_i64(on))?;
        let wx = u.mul_f64(cf)?.add(v.mul_f64(sf)?)?;
        let wy = v.mul_f64(cf)?.sub(u.mul_f64(sf)?)?;
        let kn = f64::from(scale.num);
        let den = match rotation.exact_norm() {
            // r·k_n < 2^64, so the product fits and is a double-double exactly.
            Some(r) => budget(
                i128::from(r)
                    .checked_mul(i128::from(scale.num))
                    .and_then(DD::from_i128),
            )?,
            None => {
                // c² + s² < 2^63 fits i64.
                let n2 = budget(i64::try_from(rotation.norm2()).ok())?;
                DD::from_i64(n2).sqrt()?.mul_f64(kn)?
            }
        };
        let kd = f64::from(scale.den);
        let xd = wx.mul_f64(kd)?.div(den)?;
        let yd = wy.mul_f64(kd)?.div(den)?;
        let x = candidate(xd)?;
        let y = candidate(yd)?;
        let inputs = ExactInputs::new(self, map.easting, map.northing)?;
        // s is an i32 widened to i64, so −s fits.
        let (c, s) = (i64::from(c), i64::from(s));
        let ns = budget(s.checked_neg())?;
        let x = settle(x, xd, |t| inputs.sign(c, s, t))?;
        let y = settle(y, yd, |t| inputs.sign(ns, c, t))?;
        Ok((to_coord(x)?, to_coord(y)?))
    }

    /// Frame grid coordinates converted to map coordinates in metres: the nearest doubles to the
    /// exact values, ties to even, with whether they are exact (see the module documentation).
    ///
    /// Bit budgets of the exact divisions: heights and map frames divide O + z (below 2^61) by S;
    /// a rational Ground frame divides `O_E·k_d·r` + `k_n·(c·x` − s·y) (below 2^124 + 2^104 < 2^125)
    /// by `k_d·r·S` (below 2^96).
    ///
    /// For an irrational direction each horizontal coordinate is V = (O + q)/S with
    /// q = `k_n·a/(k_d·√n)`, a = c·x − s·y (or s·x + c·y) a nonzero integer below 2^72 and
    /// n = c² + s² not a square (a = 0 gives the rational O/S). Bounds: |V| is below
    /// (2^60 + 2^104)/2^22, so below 2^84; and |V| > 2^−220, because V = (A − `B√n)/(k_d·S·√n`)
    /// with A = `k_n·a` and B = −`O·k_d`, A² − B²n is a nonzero integer, so
    /// |A − B√n| = |A² − B²n|/|A + B√n| ≥ 1/(|A| + |B|√n), which is above 2^−124, while
    /// `k_d·S·√n` < 2^95.5. The double-double value (q by a square root, a product and a division,
    /// then one addition and one division) is within 12.3u²·|V| + 13.6u²·|q|/S of V (the bounds
    /// of [`crate::dd`] composed as on [`FrameRecord::to_frame`]). With a margin of
    /// 2^−98 = 256u² times |V| + |q|/S, doubled, the nearest double to the double-double value is
    /// certified when the whole uncertainty interval lies inside its rounding interval. Otherwise
    /// the nearest double is found by bisection with exact signs (`IrrationalMap::nearest`). V is
    /// irrational, so there is never a tie, and the coordinate is never exact.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] only if a double-double step overflows, and
    /// [`NumError::BudgetExceeded`] only if a checked step or an exact sign failed: the budgets
    /// rule out both.
    pub fn to_map(self, p: P3) -> Result<Rounded<MapPoint>, NumError> {
        let s = steps_per_metre(self.step);
        let s128 = u128::from(s);
        let [oe, on, oh] = self.origin.map(i128::from);
        let (x, y, z) = (
            i128::from(p.x.get()),
            i128::from(p.y.get()),
            i128::from(p.z.get()),
        );
        let height = quotient(budget(oh.checked_add(z))?, s128)?;
        let rotation = self.rotation();
        let scale = self.scale();
        let (c, sn) = (i128::from(rotation.cos), i128::from(rotation.sin));
        let (kn, kd) = (i128::from(scale.num), i128::from(scale.den));
        // a·u + b·v exactly, for the budgets below.
        let combine = |a: i128, u: i128, b: i128, v: i128| -> Result<i128, NumError> {
            budget(
                a.checked_mul(u)
                    .zip(b.checked_mul(v))
                    .and_then(|(au, bv)| au.checked_add(bv)),
            )
        };
        // Rotated, unscaled offsets: (c·x − s·y, s·x + c·y), each below 2^72 in magnitude.
        let ns = budget(sn.checked_neg())?;
        let (ae, an) = (combine(c, x, ns, y)?, combine(sn, x, c, y)?);
        let (easting, northing) = match (rotation.quarter_turn_count(), scale.is_one()) {
            // A quarter turn has r = 1, so the offsets are already in steps.
            (Some(_), true) => (
                quotient(budget(oe.checked_add(ae))?, s128)?,
                quotient(budget(on.checked_add(an))?, s128)?,
            ),
            _ => match rotation.exact_norm() {
                Some(r) => {
                    // k_d·r < 2^64 and k_d·r·S < 2^96; O·k_d·r + k_n·a below 2^125.
                    let kdr = budget(kd.checked_mul(i128::from(r)))?;
                    let den = budget(u128::try_from(kdr).ok().and_then(|v| v.checked_mul(s128)))?;
                    (
                        quotient(combine(oe, kdr, kn, ae)?, den)?,
                        quotient(combine(on, kdr, kn, an)?, den)?,
                    )
                }
                None => (
                    self.irrational_to_map(oe, ae, s)?,
                    self.irrational_to_map(on, an, s)?,
                ),
            },
        };
        Ok(Rounded::new(
            MapPoint {
                easting: easting.value(),
                northing: northing.value(),
                height: height.value(),
            },
            easting.is_exact() && northing.is_exact() && height.is_exact(),
        ))
    }

    /// One horizontal map coordinate for an irrational direction: the nearest double to
    /// (O + `k_n·a/(k_d·√n))/S` (see [`FrameRecord::to_map`]).
    fn irrational_to_map(&self, origin: i128, a: i128, s: u64) -> Result<Rounded<f64>, NumError> {
        if a == 0 {
            return quotient(origin, u128::from(s));
        }
        let scale = self.scale();
        let n2 = self.rotation().norm2();
        // |k_n·a| < 2^32·2^72 = 2^104, inside a double-double's 2^106, as are |O| ≤ 2^60 and
        // c² + s² < 2^63; S < 2^32 fits i64 and is a double exactly.
        let kn_a = budget(i128::from(scale.num).checked_mul(a))?;
        let num = budget(DD::from_i128(kn_a))?;
        let n2i = budget(i64::try_from(n2).ok())?;
        let den = DD::from_i64(n2i).sqrt()?.mul_f64(f64::from(scale.den))?;
        let q = num.div(den)?;
        let o = budget(DD::from_i128(origin))?;
        let si = budget(i64::try_from(s).ok())?;
        #[allow(
            clippy::cast_precision_loss,
            reason = "the steps per metre are below 2^32 < 2^53: exact in f64"
        )]
        let sf = s as f64;
        let metres = o.add(q)?.div(DD::from_i64(si))?;
        let (h, l) = (metres.hi(), metres.lo());
        // The parts are finite (a double-double's invariant), so `math::abs` refuses none.
        let (h_size, q_size, l_size) = (
            math::abs(h)?.get(),
            math::abs(q.hi())?.get(),
            math::abs(l)?.get(),
        );
        // The certificate: |V − (h + l)| ≤ err, and h + l ± 2·err inside h's rounding interval.
        let err = (h_size + q_size / sf) * CERTIFICATE_MARGIN;
        let up = h.next_up() - h;
        let down = h - h.next_down();
        if h != 0.0 && l + 2.0 * err < 0.5 * up && l - 2.0 * err > -0.5 * down {
            return Ok(Rounded::new(h, false));
        }
        let exact = IrrationalMap {
            origin,
            kn_a,
            kd: i128::from(scale.den),
            s: i128::from(s),
            n2,
        };
        // A bracket of |V| from the double-double value, checked exactly inside `nearest`.
        let spread = l_size + 2.0 * err;
        let lo = (h_size - spread) * (1.0 - BRACKET_WIDENING);
        let lo = if lo > IRRATIONAL_MIN {
            lo
        } else {
            IRRATIONAL_MIN
        };
        let hi = (h_size + spread) * (1.0 + BRACKET_WIDENING);
        let hi = if hi < MAP_RESULT_MAX {
            hi
        } else {
            MAP_RESULT_MAX
        };
        // `None` would mean an exact sign overflowed its budget, which the bounds rule out: it is
        // reported, never replaced by the double-double's value.
        let value = budget(exact.nearest(lo, hi))?;
        Ok(Rounded::new(value, false))
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
    /// origin on every axis.
    ///
    /// # Errors
    ///
    /// [`MathError::Num`] with [`NumError::Degenerate`] for no operands;
    /// [`MathError::FrameMismatch`] if an operand is not a lattice frame (`NotOnLattice`) or its
    /// lattice differs: another horizontal or vertical CRS (`Crs`, `VerticalCrs`), or another step
    /// or base origin (`NotOnLattice`); [`MathError::FrameRangeExceeded`] if the combined box does
    /// not fit the frame that owns its centre (the operation must be split at a lattice line);
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
            if frame.horizontal_crs != first.horizontal_crs {
                return Err(MathError::FrameMismatch(FrameMismatch::Crs));
            }
            if frame.vertical_crs != first.vertical_crs {
                return Err(MathError::FrameMismatch(FrameMismatch::VerticalCrs));
            }
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
        .with_vertical_crs(first.vertical_crs);
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
    /// never rotated or scaled). `c·ΔO_E` + `s·ΔO_N` is below 2^102; its product by `k_d` is checked,
    /// and overflows only when the offset would exceed 2^62 anyway.
    ///
    /// # Errors
    ///
    /// [`MathError::FrameMismatch`] if the horizontal or vertical coordinate reference systems,
    /// rotations or scales differ (checked in that order), or the offset is not a whole number of
    /// steps along the target's axes; [`MathError::Num`] with [`NumError::WrongDirection`] if
    /// `target`'s step is coarser (an exact shift can only refine), or [`NumError::OutOfRange`] if
    /// an offset exceeds [`SHIFT_MAX`], or [`NumError::BudgetExceeded`] only if a checked step
    /// failed, which the bounds above rule out.
    pub fn shift_to(&self, target: &FrameRecord) -> Result<FrameShift, MathError> {
        if self.horizontal_crs != target.horizontal_crs {
            return Err(MathError::FrameMismatch(FrameMismatch::Crs));
        }
        if self.vertical_crs != target.vertical_crs {
            return Err(MathError::FrameMismatch(FrameMismatch::VerticalCrs));
        }
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
        // and k_n·r (below 2^64).
        let delta = |a: i64, b: i64| -> Result<i128, NumError> {
            budget(
                f.checked_mul(i128::from(a))
                    .and_then(|fa| fa.checked_sub(i128::from(b))),
            )
        };
        let ([ae, an, ah], [be, bn, bh]) = (self.origin, target.origin);
        let (de, dn, dh) = (delta(ae, be)?, delta(an, bn)?, delta(ah, bh)?);
        let (c, s) = (i128::from(rotation.cos), i128::from(rotation.sin));
        let (ox, oy) = match rotation.exact_norm() {
            Some(r) => {
                let den = budget(i128::from(scale.num).checked_mul(i128::from(r)))?;
                let kd = i128::from(scale.den);
                let part = |a: i128| -> Result<i128, MathError> {
                    let num = a.checked_mul(kd).ok_or(NumError::OutOfRange { value: a })?;
                    if budget(num.checked_rem(den))? != 0 {
                        return Err(MathError::FrameMismatch(FrameMismatch::NotOnLattice));
                    }
                    Ok(budget(num.checked_div(den))?)
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

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "tests may use native arithmetic and casts; the rules are for kernel code"
)]
mod tests {
    use super::*;
    use crate::exact::pow2;

    fn dd(hi: f64, lo: f64) -> DD {
        DD::from_sum(hi, lo).unwrap()
    }

    /// The exact sign of X − t/2 for an exact value X = x2/2.
    fn exactly(x2: i128) -> impl Fn(i128) -> Option<Sign> {
        move |t| Some(Sign::of_i128(x2 - t))
    }

    /// The settled rounding of a double-double value with the given exact signs.
    fn run(d: DD, sign: impl Fn(i128) -> Option<Sign>) -> Result<Rounded<i128>, NumError> {
        settle(d.round_shifted(0).unwrap(), d, sign)
    }

    #[allow(
        clippy::unnecessary_wraps,
        reason = "the same Result the functions under test return, for assert_eq"
    )]
    fn inexact(value: i128) -> Result<Rounded<i128>, NumError> {
        Ok(Rounded::new(value, false))
    }

    #[test]
    fn settle_trusts_exact_signs_over_the_double_double() {
        let tiny = pow2(-50).unwrap();
        // The exact value is the midpoint 5.5; the double-double value fell just below it.
        let d = dd(5.5, -tiny);
        assert_eq!(d.round_shifted(0).unwrap().value(), 5);
        assert_eq!(run(d, exactly(11)), inexact(6));
        // The exact midpoint 6.5 approached from above goes to the even neighbour too.
        assert_eq!(run(dd(6.5, tiny), exactly(13)), inexact(6));
        // The exact value is a whole step although the double-double value is not.
        assert_eq!(
            run(dd(5.0, pow2(-45).unwrap()), exactly(10)),
            Ok(Rounded::new(5, true))
        );
        // Just above the midpoint 6.5, with the double-double value just below it: up.
        let above = |t: i128| {
            Some(if t <= 13 {
                Sign::Positive
            } else {
                Sign::Negative
            })
        };
        assert_eq!(run(dd(6.5, -tiny), above), inexact(7));
        // Just below the midpoint 6.5, with the double-double value just above it: down.
        let below = |t: i128| {
            Some(if t >= 13 {
                Sign::Negative
            } else {
                Sign::Positive
            })
        };
        assert_eq!(run(dd(6.5, tiny), below), inexact(6));
        // Far from whole steps and midpoints no exact sign is needed.
        let never = |_: i128| -> Option<Sign> { panic!("no exact sign is needed here") };
        assert_eq!(run(dd(5.3, 0.0), never), inexact(5));
        // A failed exact sign is a defect: reported, never replaced by the candidate.
        assert_eq!(run(dd(5.0, 0.0), |_| None), Err(NumError::BudgetExceeded));
    }

    #[test]
    fn exact_signs_of_a_ground_frame() {
        let rot = Rotation::direction(3, 4).unwrap();
        let f = FrameRecord::new(CrsId::Epsg(2193), GridStep::Q0, [0; 3], rot, Scale::ONE).unwrap();
        // E = 1/4096 m: u = 1562.5 steps, v = 0, so x = 3u/5 = 937.5 and y = −4u/5 = −1250.
        let inputs = ExactInputs::new(&f, 1.0 / 4096.0, 0.0).unwrap();
        let signs = [Some(Sign::Positive), Some(Sign::Zero), Some(Sign::Negative)];
        assert_eq!([1874, 1875, 1876].map(|t| inputs.sign(3, 4, t)), signs);
        assert_eq!([-2501, -2500, -2499].map(|t| inputs.sign(-4, 3, t)), signs);
        // The largest inputs stay inside the 1280-bit budget: E at 2^62 m and N subnormal.
        let big = ExactInputs::new(&f, pow2(62).unwrap(), f64::from_bits(1)).unwrap();
        assert_eq!(big.b, 1074);
        assert_eq!(
            big.sign(-(1i64 << 31), 1 << 31, 1i128 << 42),
            Some(Sign::Negative)
        );
    }

    #[test]
    fn irrational_signs_at_the_largest_inputs() {
        // The largest irrational direction and scale: the inputs stay inside the 2560-bit budget.
        let rot = Rotation::direction(i32::MAX, i32::MIN + 1).unwrap();
        let k = Scale::new(u32::MAX, 1).unwrap();
        let f = FrameRecord::new(CrsId::Epsg(2193), GridStep::Q2, [0; 3], rot, k).unwrap();
        let big = ExactInputs::new(&f, pow2(62).unwrap(), f64::from_bits(1)).unwrap();
        assert!(big.r.is_none());
        let t = (1i128 << 42) + 1;
        assert!(
            big.sign(i64::from(i32::MAX), i64::from(i32::MIN + 1), t)
                .is_some()
        );
        assert!(
            big.sign(i64::from(i32::MIN + 1), i64::from(i32::MAX), -t)
                .is_some()
        );
    }
}
