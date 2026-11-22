//! The mass of a volume of a material: one exact function.
//!
//! **Rule.** The volume is whole cubic millimetres ([`construct_params::Volume`]) and the density
//! whole grams per cubic metre. One cubic metre is 1,000,000,000 mm3, so
//! `grams = volume_mm3 * density_g_per_m3 / 1_000_000_000`, worked out in 128-bit whole numbers
//! with no float, and rounded **to the nearest whole gram, a half going to the even gram** (the
//! rule the parameters crate and the geometry kernel use). A volume below zero is refused.
//! The largest inputs this crate stores cannot overflow the working type or the result.

use crate::error::MaterialError;
use crate::material::Physical;
use construct_params::Volume;

/// Cubic millimetres in a cubic metre.
const MM3_PER_M3: u128 = 1_000_000_000;

/// The mass in whole grams of `volume` of a material with this physical data, rounded as the
/// module header says.
///
/// # Errors
/// [`MaterialError::NegativeVolume`] for a volume below zero, and [`MaterialError::BadValue`] for
/// a density of zero or above the largest this crate stores (so a hand-built [`Physical`] cannot
/// overflow).
pub fn mass_grams(volume: Volume, physical: &Physical) -> Result<u64, MaterialError> {
    physical.check()?;
    let mm3 = u128::try_from(volume.mm3()).map_err(|_| MaterialError::NegativeVolume)?;
    let density = u128::from(physical.density_g_per_m3.value);
    // At most about 9.2e18 * 1e8 = 9.2e26, far inside u128.
    let product = mm3.checked_mul(density).ok_or(MaterialError::BadValue {
        property: "density",
        value: physical.density_g_per_m3.value,
    })?;
    let whole = product.checked_div(MM3_PER_M3).unwrap_or(0);
    let rest = product.checked_rem(MM3_PER_M3).unwrap_or(0);
    let twice = rest.saturating_mul(2);
    let rounded = if twice > MM3_PER_M3 || (twice == MM3_PER_M3 && whole.checked_rem(2) == Some(1))
    {
        whole.checked_add(1).unwrap_or(whole)
    } else {
        whole
    };
    // At most 9.2e18 * 1e8 / 1e9 + 1, inside u64.
    u64::try_from(rounded).map_err(|_| MaterialError::BadValue {
        property: "density",
        value: physical.density_g_per_m3.value,
    })
}
