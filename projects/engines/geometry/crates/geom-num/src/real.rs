//! `Real`: a floating-point number that is never NaN and never infinite.
//!
//! NaN's sign and payload are not deterministic across platforms, even in WebAssembly (Rust RFC 3514),
//! so the kernel never produces one (chapter 21). Floating-point values that leave this crate are
//! `Real`s, checked when made. The ordering is total and deterministic: −0 sorts before +0, the
//! IEEE 754 totalOrder predicate (`f64::total_cmp`).

use core::cmp::Ordering;

use crate::error::NumError;

/// A finite `f64`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Real(f64);

impl Real {
    /// Zero.
    pub const ZERO: Real = Real(0.0);

    /// A `Real`, or `NotFinite` for NaN and infinities.
    pub const fn new(v: f64) -> Result<Real, NumError> {
        if v.is_finite() { Ok(Real(v)) } else { Err(NumError::NotFinite) }
    }

    /// The value.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }

    /// The larger of two values in IEEE totalOrder (so +0 is larger than −0); deterministic, unlike
    /// `f64::max`, which may return either zero.
    #[must_use]
    pub fn total_max(self, other: Real) -> Real {
        if self.0.total_cmp(&other.0) == Ordering::Less { other } else { self }
    }

    /// The smaller of two values in IEEE totalOrder.
    #[must_use]
    pub fn total_min(self, other: Real) -> Real {
        if self.0.total_cmp(&other.0) == Ordering::Greater { other } else { self }
    }
}

impl Eq for Real {}

impl PartialOrd for Real {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Real {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_finite_and_orders_zeros() {
        assert_eq!(Real::new(f64::NAN), Err(NumError::NotFinite));
        assert_eq!(Real::new(f64::INFINITY), Err(NumError::NotFinite));
        let pz = Real::new(0.0).unwrap();
        let nz = Real::new(-0.0).unwrap();
        assert!(nz < pz);
        assert_eq!(pz.total_max(nz).get().to_bits(), 0.0f64.to_bits());
        assert_eq!(nz.total_max(pz).get().to_bits(), 0.0f64.to_bits());
        assert_eq!(pz.total_min(nz).get().to_bits(), (-0.0f64).to_bits());
    }
}
