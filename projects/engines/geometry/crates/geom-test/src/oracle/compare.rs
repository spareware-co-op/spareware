//! Comparing a kernel result with an oracle's (chapter 28.10; piece G113).
//!
//! What it does: [`exact`] compares exact oracles exactly. [`within`] compares a value within an
//! [`Allowance`]: both sides' stated bounds plus the conversion rounding, capped at our bound plus
//! the fit bound. [`Hausdorff`]-style distances come from the same comparison: they are numbers
//! measured by sampling, which can only refute.
//!
//! Guarantees:
//!
//! - **The cap holds.** [`Allowance::total`] never exceeds our bound plus the fit bound, whatever
//!   the oracle states: anything beyond is settled by recomputing the case in high precision, never
//!   accepted because the oracle's own tolerance grew.
//! - **A pass is never lost to rounding, and a difference never hidden by it**: the allowance is
//!   rounded up and the gap rounded down, so [`within`] says "differs" only when the two doubles are
//!   truly further apart than the allowance's real value.
//! - **Not finite is a difference.**
//!
//! Method: chapter 28.10's comparisons
//! (`projects/engines/geometry/architecture/28-testing.md`), with directed rounding by one unit in
//! the last place on each sum.
//!
//! Cost: constant per value.

/// What two results may differ by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Allowance {
    /// Our stated bound.
    pub ours: f64,
    /// The oracle's stated bound.
    pub theirs: f64,
    /// The rounding a change of unit cost.
    pub rounding: f64,
    /// The fit bound, `ε_fit`, in the units compared.
    pub fit: f64,
}

impl Allowance {
    /// `ours + theirs + rounding`, capped at `ours + fit`, each sum rounded up.
    #[must_use]
    pub fn total(&self) -> f64 {
        let stated = (self.ours + self.theirs).next_up();
        let stated = (stated + self.rounding).next_up();
        let cap = (self.ours + self.fit).next_up();
        if stated < cap { stated } else { cap }
    }
}

/// Whether two exact answers agree.
#[must_use]
pub fn exact<T: PartialEq>(ours: &T, theirs: &T) -> bool {
    ours == theirs
}

/// Whether `ours` and `theirs` are within `allowance`.
#[must_use]
pub fn within(ours: f64, theirs: f64, allowance: &Allowance) -> bool {
    if !ours.is_finite() || !theirs.is_finite() {
        return false;
    }
    (ours - theirs).abs().next_down() <= allowance.total()
}
