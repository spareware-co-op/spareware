//! Points of NURBS curves by corner cutting (The NURBS Book, algorithm A5.2): de Boor's algorithm,
//! which inserts the parameter until it is repeated p times and takes the point that leaves.
//!
//! What it does: [`Curve::point_by_corner_cutting`] takes the p + 1 homogeneous control points of
//! the span that holds the parameter and replaces them p times by convex combinations of
//! neighbours, α·Q + (1 − α)·R with α = (u − `u_i`) / (`u_(i+p+1−r)` − `u_i`) at level r; the one
//! point left is C(u) in homogeneous form, divided by its weight for a rational curve. It computes
//! the same point as [`Curve::point`] by a different route, every step a convex combination, which
//! makes it the reference evaluation that knot insertion and subdivision rest on.
//!
//! Guarantees:
//!
//! - **Convex steps.** For a parameter in the span [`u_k`, `u_(k+1)`], every α lies in [0, 1]:
//!   `u_i` ≤ `u_k` ≤ u ≤ `u_(k+1)` ≤ `u_(i+p+1−r)` for every i and r the algorithm visits, and the
//!   rounded differences keep that order, so no step extrapolates.
//! - **Near exact.** Each homogeneous coordinate of the point is within `γ_(10p)` · M of its exact
//!   value, M the largest absolute value of that coordinate over the p + 1 points used, by the
//!   argument of `crate::insert` with one insertion pass per level; a rational point then adds the
//!   division by the weight, which is computed the same way.
//! - **Typed failures.** A parameter outside the domain is [`NurbsError::ParameterOutside`]; a
//!   point that overflows, [`NurbsError::NotFinite`].
//!
//! Method: de Boor's algorithm (C. de Boor, "On calculating with B-splines", *Journal of
//! Approximation Theory* 6 (1972), 50–62, <https://doi.org/10.1016/0021-9045(72)90080-9>), as the
//! corner cutting of Piegl and Tiller's algorithm A5.2 (The NURBS Book, 2nd ed., section 5.2,
//! <https://doi.org/10.1007/978-3-642-59223-2>), written from the recurrence; the span is found by
//! [`crate::basis`], so the domain's ends are evaluated inside it.
//! SPDX-License-Identifier: MPL-2.0
//!
//! Cost: O(p²·D) operations; nothing allocates.

use crate::NurbsError;
use crate::arith::Hom;
use crate::basis::{Limit, WIDTH};
use crate::curve::{Curve, finite};
use crate::index::{add, get, set, sub};

impl<const D: usize> Curve<D> {
    /// The point C(u) by corner cutting (A5.2).
    ///
    /// # Errors
    ///
    /// [`NurbsError::ParameterOutside`] if `u` is not in the domain; [`NurbsError::NotFinite`] if
    /// the point overflows.
    #[allow(
        clippy::many_single_char_names,
        reason = "the names are de Boor's recurrence's: span k, degree p, level r, knot index i"
    )]
    pub fn point_by_corner_cutting(&self, u: f64, limit: Limit) -> Result<[f64; D], NurbsError> {
        let basis = self.basis();
        let p = basis.degree();
        let k = basis.span(u, limit)?;
        let knots = basis.knots();
        let first = sub(k, p)?;
        let zero = Hom::of([0.0; D], 0.0)?;
        let mut rw = [zero; WIDTH];
        for j in 0..=p {
            let i = add(first, j)?;
            let w = self.weights().map_or(Ok(1.0), |ws| get(ws, i))?;
            set(&mut rw, j, Hom::of(get(self.homogeneous(), i)?, w)?)?;
        }
        for r in 1..=p {
            // Level r: rw[j] for j from p down to r combines rw[j−1] and rw[j], knot index
            // i = first + j.
            for j in (r..=p).rev() {
                let i = add(first, j)?;
                let ui = get(knots, i)?;
                let alpha = (u - ui) / (get(knots, sub(add(add(i, p)?, 1)?, r)?)? - ui);
                let next = Hom::combine(alpha, get(&rw, j)?, 1.0 - alpha, get(&rw, sub(j, 1)?)?)?;
                set(&mut rw, j, next)?;
            }
        }
        let h = get(&rw, p)?;
        if self.is_rational() {
            finite(h.c.map(|x| x / h.w))
        } else {
            finite(h.c)
        }
    }
}
