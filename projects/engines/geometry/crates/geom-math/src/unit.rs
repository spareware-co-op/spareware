//! Linear units: how many metres one unit of a coordinate reference system is, as an exact ratio.
//!
//! A frame record's map coordinates are in its CRS's linear unit (chapter 17.1's `linear_unit`,
//! metres per CRS unit): metres for NZTM2000, US survey feet for many US state plane systems. The
//! unit is a reduced ratio of two 64-bit integers, so every unit a CRS registry defines by a
//! decimal factor or a ratio of whole numbers is held exactly: the metre 1/1, the international
//! foot 0.3048 = 381/1250, the US survey foot 1200/3937, Clarke's foot 0.3047972654 =
//! 1523986327/5000000000. A unit's ratio lies between 2^−32 and 2^32 metres (0.23 nm to 4.3
//! million km), which every linear unit of a coordinate reference system does; the frame
//! conversions' bit budgets assume it.
//!
//! The table of named units an exchange format uses belongs to the exchange layer (piece
//! "Exchange basics: units, numbers to grid, frames, reports"); this module holds the type and the
//! three units chapter 17.1 names.
//!
//! Guarantees:
//!
//! - **Exact and canonical.** A unit is stored reduced by the greatest common divisor, so equal
//!   ratios have equal fields and `==` is equality of the ratios.
//! - **In range or refused.** [`LinearUnit::new`] and [`LinearUnit::from_decimal`] refuse a zero
//!   part with [`NumError::Degenerate`], and a ratio outside 2^−32 to 2^32, or one whose reduced
//!   parts do not fit 64 bits, with [`NumError::OutsideDomain`]; nothing is rounded.
//!
//! Method: exact rational arithmetic with Euclid's greatest common divisor (D. E. Knuth, *The Art
//! of Computer Programming*, vol. 2, section 4.5.2,
//! <https://www-cs-faculty.stanford.edu/~knuth/taocp.html>); the units as EPSG defines them: the
//! international foot 0.3048 m and Clarke's foot 0.3047972654 m as PROJ's copy of the EPSG data
//! gives them
//! (<https://github.com/OSGeo/PROJ/blob/5fa31c29f0a806568ed7bc76c1a557604db4a804/data/sql/unit_of_measure.sql>),
//! and the US survey foot by its definition, 1200/3937 m (EPSG's 12/39.37), which that copy
//! stores only rounded, as 0.304800609601219241.
//!
//! Consulted: PROJ (its copy of the EPSG dataset, whose own terms of use cover the data), MIT,
//!   <https://github.com/OSGeo/PROJ/blob/5fa31c29f0a806568ed7bc76c1a557604db4a804/data/sql/unit_of_measure.sql>,
//!   for the factors of the US survey foot (EPSG:9003), the international foot (EPSG:9002) and
//!   Clarke's foot (EPSG:9005). No code copied.
//!
//! Cost: constant time; [`LinearUnit::new`] and [`LinearUnit::from_decimal`] reduce by a greatest
//! common divisor (at most 129 remainder steps of 64-bit integers), and
//! [`LinearUnit::from_decimal`] multiplies by at most 38 powers of ten.

use geom_num::NumError;

use crate::decimal::Decimal;
use crate::exact::gcd_u128;

/// Metres per unit of a coordinate reference system, as an exact reduced ratio `num / den` (see
/// the module documentation).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LinearUnit {
    num: u64,
    den: u64,
}

/// 2^32, the bound on a unit's ratio and its inverse.
const RATIO_MAX: u128 = 1 << 32;

impl LinearUnit {
    /// The metre.
    pub const METRE: LinearUnit = LinearUnit { num: 1, den: 1 };

    /// The international foot, 0.3048 m exactly (EPSG:9002).
    pub const INTERNATIONAL_FOOT: LinearUnit = LinearUnit {
        num: 381,
        den: 1250,
    };

    /// The US survey foot, 1200/3937 m exactly (EPSG:9003).
    pub const US_SURVEY_FOOT: LinearUnit = LinearUnit {
        num: 1200,
        den: 3937,
    };

    /// The unit of `num / den` metres, reduced.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] if either part is zero; [`NumError::OutsideDomain`] if the ratio
    /// is below 2^−32 or above 2^32.
    pub fn new(num: u64, den: u64) -> Result<LinearUnit, NumError> {
        LinearUnit::reduced(u128::from(num), u128::from(den))
    }

    /// The unit of `num / den` metres for parts below 2^128: reduced, then checked.
    fn reduced(num: u128, den: u128) -> Result<LinearUnit, NumError> {
        if num == 0 || den == 0 {
            return Err(NumError::Degenerate);
        }
        // g ≥ 1 divides both, so neither quotient fails.
        let g = gcd_u128(num, den);
        let (n, d) = (
            num.checked_div(g).ok_or(NumError::BudgetExceeded)?,
            den.checked_div(g).ok_or(NumError::BudgetExceeded)?,
        );
        // 2^−32 ≤ n/d ≤ 2^32, compared exactly: each product is below 2^160 only if it fits.
        let inside = |a: u128, b: u128| a.checked_mul(RATIO_MAX).is_none_or(|big| big >= b);
        if !inside(n, d) || !inside(d, n) {
            return Err(NumError::OutsideDomain);
        }
        match (u64::try_from(n), u64::try_from(d)) {
            (Ok(num), Ok(den)) => Ok(LinearUnit { num, den }),
            _ => Err(NumError::OutsideDomain),
        }
    }

    /// The unit whose length in metres is the decimal `metres`, exactly: 0.3047972654 gives
    /// Clarke's foot, 1523986327/5000000000 m.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] for zero; [`NumError::OutsideDomain`] for a negative length, a
    /// ratio outside 2^−32 to 2^32, or one whose reduced parts do not fit 64 bits.
    pub fn from_decimal(metres: Decimal) -> Result<LinearUnit, NumError> {
        if metres.is_zero() {
            return Err(NumError::Degenerate);
        }
        let m = u128::try_from(metres.significand()).map_err(|_| NumError::OutsideDomain)?;
        let e = metres.exponent();
        if e >= 0 {
            // m·10^e overflows u128 only above 2^128 > 2^32 metres: out of range.
            let whole = 10u128
                .checked_pow(e.unsigned_abs())
                .and_then(|power| m.checked_mul(power))
                .ok_or(NumError::OutsideDomain)?;
            return LinearUnit::reduced(whole, 1);
        }
        // m / (2^k·5^k) with k = −e ≤ 340. A decimal's m is not a multiple of 10, so it shares
        // factors of 2 or of 5 with the denominator, not both. Shared factors of 5 are cancelled
        // first (m < 2^127, so at most 54 steps), since 2^k alone may fit 64 bits when 10^k
        // overflows 128; shared factors of 2 would leave 5^k ≥ 5^39 > 2^64, which no reduction
        // makes fit. So a denominator that does not fit is one whose reduced form does not fit.
        let (mut m, twos, mut fives) = (m, e.unsigned_abs(), e.unsigned_abs());
        while fives > 0 && m.is_multiple_of(5) {
            m /= 5;
            fives = fives.checked_sub(1).ok_or(NumError::BudgetExceeded)?;
        }
        let den = 2u128
            .checked_pow(twos)
            .zip(5u128.checked_pow(fives))
            .and_then(|(a, b)| a.checked_mul(b))
            .ok_or(NumError::OutsideDomain)?;
        LinearUnit::reduced(m, den)
    }

    /// The numerator: metres per unit times the denominator.
    #[must_use]
    pub const fn num(self) -> u64 {
        self.num
    }

    /// The denominator.
    #[must_use]
    pub const fn den(self) -> u64 {
        self.den
    }

    /// Whether the unit is the metre.
    #[must_use]
    pub const fn is_metre(self) -> bool {
        self.num == 1 && self.den == 1
    }
}
