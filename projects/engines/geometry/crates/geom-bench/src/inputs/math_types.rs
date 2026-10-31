//! Inputs for `geom-math`'s hot paths (pieces G45, G46 and G48, decision 32.44): grid points for
//! the typed transforms and the rational rotation, unit directions for the rational-rotation
//! search, grid points for the frame-to-map conversion, and survey coordinates as decimal text.
//!
//! The transforms, the rotation and the frame are fixed per operation and built by the kernels
//! (`crate::kernels::math_types`), the same as `geom-math`'s wall-clock benchmark of G45 builds
//! them, so a batch is the points or texts alone. Class `typical` only: these operations take one
//! path for every input in range, and their refusals are tested in `geom-math` itself.
//!
//! - Points: coordinates within ±2^30 grid steps ([`super::TYPICAL_REACH`]), so every image stays
//!   in the frame.
//! - Directions: both components drawn from [−1, 1) at 2^−52 resolution, never both zero, so
//!   every search succeeds.
//! - Decimal text: a map coordinate to the micrometre, signed, of up to ten digits before the
//!   point, as a survey file writes it.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Ok` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the cases of `geom-math`'s `benches/transform.rs` (the 1M-point transform benchmark of
//! G45, with G46's searches and G48's frames), whose header says they move here once this
//! harness exists.
//! Each is measured as chapter 28.14 sets benchmarks out,
//! `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: a few draws of [`SplitMix64`] per input, once per batch, outside every measured run.

use geom_math::F2;
use geom_num::grid::P3;

use super::{Class, TYPICAL_REACH, batch_of, p3, rng, within};
use crate::rng::SplitMix64;
use geom_math::{CrsId, FrameRecord, Rotation, Scale};
use geom_num::grid::GridStep;

/// Points within ±2^30 steps, for the operation named `op`.
fn points(op: &str, class: Class) -> Vec<P3> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng(op, class);
    batch_of(|_| {
        Some(p3(
            within(&mut g, TYPICAL_REACH),
            within(&mut g, TYPICAL_REACH),
            within(&mut g, TYPICAL_REACH),
        ))
    })
}

/// Points for `TypedTransform::apply` on an exact transform (a quarter turn and a translation).
#[must_use]
pub fn apply_exact(class: Class) -> Vec<P3> {
    points("math_types::apply_exact", class)
}

/// Points for `TypedTransform::apply` on a rational transform (a Pythagorean rotation and a
/// translation by half steps).
#[must_use]
pub fn apply_rational(class: Class) -> Vec<P3> {
    points("math_types::apply_rational", class)
}

/// Points for `TypedTransform::apply` on a certified transform (a rotation by 60 degrees and a
/// translation, rounded once).
#[must_use]
pub fn apply_certified(class: Class) -> Vec<P3> {
    points("math_types::apply_certified", class)
}

/// Points for `RationalRotation::apply` on a rotation whose quaternion's squared norm passes 2^40.
#[must_use]
pub fn rotation_apply(class: Class) -> Vec<P3> {
    points("math_types::rotation_apply", class)
}

/// Points for `FrameRecord::to_map` on a ground frame with an irrational direction and a scale.
#[must_use]
pub fn to_map(class: Class) -> Vec<P3> {
    points("math_types::to_map", class)
}

/// A component in [−1, 1) at 2^−52 resolution.
fn unit(g: &mut SplitMix64) -> f64 {
    #[allow(
        clippy::cast_precision_loss,
        reason = "the value is below 2^53, so the conversion is exact"
    )]
    let v = (g.next_u64() >> 11) as f64;
    v / (1u64 << 52) as f64 - 1.0
}

/// Directions for `RationalRotation::plane_within`, the closest rational rotation within the
/// angular bound.
#[must_use]
pub fn plane_within(class: Class) -> Vec<F2> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("math_types::plane_within", class);
    batch_of(|_| {
        let (x, y) = (unit(&mut g), unit(&mut g));
        if x == 0.0 && y == 0.0 {
            return None;
        }
        F2::new(x, y).ok()
    })
}

/// Decimal text for `Decimal::parse`: a coordinate to the micrometre, signed, as a survey file
/// writes it.
#[must_use]
pub fn decimal_parse(class: Class) -> Vec<String> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("math_types::decimal_parse", class);
    batch_of(|_| {
        let whole = g.range(0, 9_999_999_999);
        let micro = g.range(0, 999_999);
        let sign = if g.sign() < 0 { "-" } else { "" };
        Some(format!("{sign}{whole}.{micro:06}"))
    })
}

/// The ground frame `geom-math`'s benchmark measures: NZTM, the building step, an origin near
/// Auckland, the irrational direction (1000, 7) and the scale 0.99987654.
#[must_use]
pub fn ground_frame() -> Option<FrameRecord> {
    let s = 6_400_000i64;
    let origin = [1_757_000 * s, 5_921_000 * s, 0];
    let rotation = Rotation::direction(1_000, 7).ok()?;
    let scale = Scale::new(99_987_654, 100_000_000).ok()?;
    FrameRecord::new(CrsId::Epsg(2193), GridStep::Q0, origin, rotation, scale).ok()
}
