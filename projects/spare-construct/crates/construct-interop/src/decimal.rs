//! Lengths for a file, written exactly: grid steps to the decimal text of millimetres.
//!
//! A step is 1/6,400 mm, which is 0.00015625 mm, a decimal that ends: so every length the model
//! holds is an exact decimal of at most eight places, and no float is used to make it. The text is
//! the whole millimetres, a point, and the places that are not zero (`250.`, `12.5`,
//! `0.00015625`), the form ISO 10303-21 gives a real.

use crate::error::IfcError;
use construct_params::Length;

/// Millionths of... the places a step needs: 1/6,400 mm times 10^8 is 15,625.
const PLACES: usize = 8;
/// 10^8 / 6,400, the value of one step in units of the eighth decimal place.
const STEP_IN_PLACES: u64 = 15_625;

/// `steps` grid steps as millimetres, in exact decimal text.
///
/// # Errors
///
/// [`IfcError::TooBig`] only for the one step count whose size does not fit (`i64::MIN`).
pub fn mm(steps: i64) -> Result<String, IfcError> {
    let per = u64::try_from(Length::STEPS_PER_MM).map_err(|_| IfcError::TooBig)?;
    let size = steps.unsigned_abs();
    let whole = size.checked_div(per).ok_or(IfcError::TooBig)?;
    let rest = size.checked_rem(per).ok_or(IfcError::TooBig)?;
    // rest < 6,400, so rest * 15,625 < 10^8.
    let places = rest.checked_mul(STEP_IN_PLACES).ok_or(IfcError::TooBig)?;
    let mut text = String::new();
    if steps < 0 {
        text.push('-');
    }
    text.push_str(&whole.to_string());
    text.push('.');
    if places > 0 {
        let digits = format!("{places:0PLACES$}");
        text.push_str(digits.trim_end_matches('0'));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::mm;

    /// Whole millimetres, halves, a single step and negatives, written exactly.
    #[test]
    fn steps_are_written_as_exact_decimals() {
        assert_eq!(mm(0).unwrap(), "0.");
        assert_eq!(mm(6_400).unwrap(), "1.");
        assert_eq!(mm(16_000_000).unwrap(), "2500.");
        assert_eq!(mm(80_000).unwrap(), "12.5");
        assert_eq!(mm(1).unwrap(), "0.00015625");
        assert_eq!(mm(-1).unwrap(), "-0.00015625");
        assert_eq!(mm(6_401).unwrap(), "1.00015625");
        assert_eq!(mm(-3_200).unwrap(), "-0.5");
        assert_eq!(mm(64).unwrap(), "0.01");
    }

    /// The extremes do not panic.
    #[test]
    fn the_extremes_are_written() {
        assert!(mm(i64::MAX).is_ok());
        assert!(mm(i64::MIN).is_ok());
    }
}
