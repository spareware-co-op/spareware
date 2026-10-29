//! Banded linear systems solved by Gaussian elimination without pivoting, for the collocation
//! matrices of interpolation.
//!
//! What it does: a [`Band`] holds a square matrix whose row i has its nonzero entries in columns
//! i − `lower` to i + `upper`; [`Band::solve`] solves A·X = B for a block B of `width` columns,
//! eliminating below the diagonal in order and substituting back, all inside the band.
//!
//! Why our own: `geom-linalg` has dense LU with partial pivoting, which costs O(n³) and stores n²
//! entries; an interpolation's matrix has at most p + 1 nonzero entries in a row, and needs no
//! pivoting, so elimination inside the band costs O(n·p²). No Rust library has a deterministic
//! banded solver without SIMD (chapter 15.6 rules out `faer`), and `NLib`'s banded LU is not
//! taken (its numerics are the ones chapter 29.6 keeps out).
//!
//! Guarantees:
//!
//! - **Stable without pivoting, for the matrices it is given.** The B-spline collocation matrix
//!   at increasing parameters is totally positive (C. de Boor, "Total positivity of the spline
//!   collocation matrix", *Indiana University Mathematics Journal* 25(6) (1976), 541–551,
//!   <https://doi.org/10.1512/iumj.1976.25.25043>), and Gaussian elimination without pivoting on
//!   a totally positive matrix is backward stable, its computed factors staying nonnegative (C. de
//!   Boor and A. Pinkus, "Backward error analysis for totally positive linear systems",
//!   *Numerische Mathematik* 27 (1977), 485–490, <https://doi.org/10.1007/BF01399609>). The end
//!   derivative rows of section 9.2.2 come first and last and only fix their control points, so
//!   the rest is a submatrix of such a matrix, which is totally positive too.
//! - **Refused, never guessed.** A pivot that is not positive (the Schoenberg–Whitney condition
//!   fails, or rounding has met a nearly singular matrix) is [`NurbsError::Undetermined`]; a result
//!   that is not finite is [`NurbsError::NotFinite`].
//! - **Deterministic.** Rows eliminated in order, columns in order, no fused multiply-add.
//!
//! Method: Gaussian elimination without pivoting on a totally positive band matrix, as de Boor
//! and Pinkus analyse it (<https://doi.org/10.1007/BF01399609>), in the band storage and with the
//! band triangular solves of G. H. Golub and C. F. Van Loan, *Matrix Computations*, 4th ed., Johns
//! Hopkins (2013), section 4.3; the right-hand sides are carried through the elimination.
//!
//! Cost: n·`lower`·(`upper` + `width`) multiply-adds to eliminate and n·(`upper` + 1)·`width` to
//! substitute, charged to the meter first; one allocation of n·(`lower` + `upper` + 1) doubles.

use alloc::vec::Vec;

use geom_run::WorkMeter;

use crate::NurbsError;
use crate::budget::vec_for;
use crate::index::{add, at, get, mul, sub};

/// A square band matrix, stored row by row: row i's entries from column i − `lower` to
/// i + `upper`, `lower` + `upper` + 1 of them.
#[derive(Clone, Debug)]
pub(crate) struct Band {
    n: usize,
    lower: usize,
    upper: usize,
    entries: Vec<f64>,
}

impl Band {
    /// The n × n zero matrix with these bandwidths.
    pub(crate) fn zeros(
        meter: &mut WorkMeter<'_>,
        n: usize,
        lower: usize,
        upper: usize,
    ) -> Result<Band, NurbsError> {
        let len = mul(n, add(add(lower, upper)?, 1)?)?;
        meter.reserve(
            u64::try_from(len)
                .map_err(|_| NurbsError::Defect)?
                .saturating_mul(8),
        )?;
        let mut entries = vec_for(meter, len)?;
        entries.resize(len, 0.0);
        Ok(Band {
            n,
            lower,
            upper,
            entries,
        })
    }

    /// The slot of entry (i, j), or a defect if it is outside the band.
    fn slot(&self, i: usize, j: usize) -> Result<usize, NurbsError> {
        if i >= self.n || j >= self.n || add(j, self.lower)? < i || j > add(i, self.upper)? {
            return Err(NurbsError::Defect);
        }
        let width = add(add(self.lower, self.upper)?, 1)?;
        add(mul(i, width)?, sub(add(j, self.lower)?, i)?)
    }

    /// Entry (i, j), which must lie in the band.
    fn get(&self, i: usize, j: usize) -> Result<f64, NurbsError> {
        get(&self.entries, self.slot(i, j)?)
    }

    /// Sets entry (i, j), which must lie in the band.
    pub(crate) fn set(&mut self, i: usize, j: usize, v: f64) -> Result<(), NurbsError> {
        let s = self.slot(i, j)?;
        *at(&mut self.entries, s)? = v;
        Ok(())
    }

    /// Solves A·X = B in place: `b` holds B row by row, `width` values a row, and is overwritten
    /// by X.
    pub(crate) fn solve(
        mut self,
        meter: &mut WorkMeter<'_>,
        b: &mut [f64],
        width: usize,
    ) -> Result<(), NurbsError> {
        let n = self.n;
        if b.len() != mul(n, width)? {
            return Err(NurbsError::Defect);
        }
        let wide = |k: usize| u64::try_from(k).map_err(|_| NurbsError::Defect);
        let per_row = add(
            mul(self.lower, add(self.upper, width)?)?,
            mul(add(self.upper, 1)?, width)?,
        )?;
        meter.charge(wide(n)?.saturating_mul(wide(per_row)?))?;
        for k in 0..n {
            let pivot = self.get(k, k)?;
            if !(pivot > 0.0 && pivot.is_finite()) {
                return Err(NurbsError::Undetermined);
            }
            let last_row = (add(k, self.lower)?).min(sub(n, 1)?);
            let last_col = (add(k, self.upper)?).min(sub(n, 1)?);
            for i in add(k, 1)?..=last_row {
                let l = self.get(i, k)? / pivot;
                if l == 0.0 {
                    continue;
                }
                self.set(i, k, 0.0)?;
                for j in add(k, 1)?..=last_col {
                    let v = self.get(i, j)? - l * self.get(k, j)?;
                    self.set(i, j, v)?;
                }
                for c in 0..width {
                    let src = get(b, add(mul(k, width)?, c)?)?;
                    let dst = at(b, add(mul(i, width)?, c)?)?;
                    *dst -= l * src;
                }
            }
        }
        for i in (0..n).rev() {
            let pivot = self.get(i, i)?;
            let last_col = (add(i, self.upper)?).min(sub(n, 1)?);
            for c in 0..width {
                let mut s = get(b, add(mul(i, width)?, c)?)?;
                for j in add(i, 1)?..=last_col {
                    s -= self.get(i, j)? * get(b, add(mul(j, width)?, c)?)?;
                }
                let x = s / pivot;
                if !x.is_finite() {
                    return Err(NurbsError::NotFinite);
                }
                *at(b, add(mul(i, width)?, c)?)? = x;
            }
        }
        Ok(())
    }
}
