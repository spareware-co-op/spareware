//! The mass of a volume of a material: one exact function.
//!
//! **Rule.** The volume is whole cubic grid steps of the geometry kernel
//! ([`construct_params::Volume`]; a grid step is 1/6,400 mm, so a cubic millimetre is
//! 262,144,000,000 cubic steps) and the density whole grams per cubic metre. One cubic metre is
//! 262,144,000,000,000,000,000 cubic steps, so
//! `grams = volume_cubic_steps * density_g_per_m3 / 262_144_000_000_000_000_000`, worked out in
//! 128-bit whole numbers with no float, and rounded **to the nearest whole gram, a half going to
//! the even gram** (the rule the parameters crate and the geometry kernel use). This is the same
//! fraction as `volume_mm3 * density / 10^9`, so a volume gives the same mass it gave when volumes
//! were whole cubic millimetres. A volume below zero is refused, and a mass beyond a `u64` of
//! grams is [`MaterialError::MassTooLarge`]; nothing overflows the working type.

use crate::error::MaterialError;
use crate::material::Physical;
use construct_params::Volume;

/// Cubic grid steps in a cubic metre: 10^9 cubic millimetres of 6,400^3 cubic steps each.
const CUBIC_STEPS_PER_M3: u128 = 262_144_000_000_000_000_000;

/// The mass in whole grams of `volume` of a material with this physical data, rounded as the
/// module header says.
///
/// # Errors
/// [`MaterialError::NegativeVolume`] for a volume below zero, [`MaterialError::MassTooLarge`] for
/// a mass beyond a `u64` of grams, and [`MaterialError::BadValue`] for a density of zero or above
/// the largest this crate stores (so a hand-built [`Physical`] cannot overflow).
pub fn mass_grams(volume: Volume, physical: &Physical) -> Result<u64, MaterialError> {
    physical.check()?;
    let steps = u128::try_from(volume.cubic_steps()).map_err(|_| MaterialError::NegativeVolume)?;
    let density = u128::from(physical.density_g_per_m3.value);
    // Whole cubic metres and the rest, so neither product can overflow: the volume is under
    // 2^127, so whole metres are under 6.5e17 and times a density of at most 1e8 under 6.5e25;
    // the rest is under 2.7e20 and times the density under 2.7e28.
    let metres = steps.checked_div(CUBIC_STEPS_PER_M3).unwrap_or(0);
    let left = steps.checked_rem(CUBIC_STEPS_PER_M3).unwrap_or(0);
    let part = left
        .checked_mul(density)
        .ok_or(MaterialError::MassTooLarge)?;
    let whole = metres
        .checked_mul(density)
        .and_then(|grams| grams.checked_add(part.checked_div(CUBIC_STEPS_PER_M3).unwrap_or(0)))
        .ok_or(MaterialError::MassTooLarge)?;
    let rest = part.checked_rem(CUBIC_STEPS_PER_M3).unwrap_or(0);
    let twice = rest.saturating_mul(2);
    let up = twice > CUBIC_STEPS_PER_M3
        || (twice == CUBIC_STEPS_PER_M3 && whole.checked_rem(2) == Some(1));
    let rounded = if up {
        whole.checked_add(1).ok_or(MaterialError::MassTooLarge)?
    } else {
        whole
    };
    u64::try_from(rounded).map_err(|_| MaterialError::MassTooLarge)
}
