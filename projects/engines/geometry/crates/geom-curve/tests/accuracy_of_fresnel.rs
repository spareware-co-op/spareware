//! The Fresnel integrals between the power series and the asymptotic expansion: panels answer there
//! to the tolerance, not the expansion, whose remainder there is far wider.
//!
//! The expansion's bound holds at every argument, so only the width tells which of the two ran. This
//! file sorts first of the crate's tests on purpose: a swapped range sends the arguments past the
//! expansion's start to the panels, whose count grows with the argument, and the tests with huge
//! arguments then outlast the mutation timeout; this one fails in milliseconds before they start.

use geom_curve::transition::fresnel::fresnel;
use geom_num::interval::Interval;

/// Far above a panel sum's error and far below the expansion's remainder anywhere under 5.
const WIDEST: f64 = 1e-12;

#[test]
fn panels_answer_between_the_series_and_the_expansion() {
    for x in [1.25, 1.5, 2.0, 3.0, 4.0, 4.75] {
        let [c, s] = fresnel(Interval::point(x).unwrap()).unwrap();
        for i in [c, s] {
            assert!(i.hi() - i.lo() < WIDEST, "{x}: {i:?}");
        }
    }
}
