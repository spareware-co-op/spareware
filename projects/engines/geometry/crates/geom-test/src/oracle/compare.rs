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
//! - **The comparison can be shown to see a difference**: [`perturbed`] moves one value by far more
//!   than any allowance a job states, and the night's `--perturb` run fails unless every moved
//!   answer is caught, so a comparison that has gone blind fails loudly instead of passing
//!   everything.
//!
//! Method: chapter 28.10's comparisons
//! (`projects/engines/geometry/architecture/28-testing.md`), with directed rounding by one unit in
//! the last place on each sum. The sensitivity check follows solidcore's difftest `--perturb`
//! (change one item of each case's output; pass only if every changed case is caught), with its
//! step in relative terms, 1e-6 of the value or of one, far above any bound a job here states.
//! Consulted: solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/b4de2149aadbd737268091c51d8390a7d3cdda70/crates/difftest/README.md>,
//!   for the perturbation check of the comparator itself (the README's "Perturbation" section). No
//!   code copied.
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

/// The step [`perturbed`] takes, relative to the value or to one if it is smaller.
pub const PERTURBATION: f64 = 1e-6;

/// `x` moved by [`PERTURBATION`] times the larger of one and `|x|`; a value that is not finite
/// becomes zero, a value a comparison must still tell from the original.
#[must_use]
pub fn perturbed(x: f64) -> f64 {
    if !x.is_finite() {
        return 0.0;
    }
    let size = if x.abs() > 1.0 { x.abs() } else { 1.0 };
    x + PERTURBATION * size
}
