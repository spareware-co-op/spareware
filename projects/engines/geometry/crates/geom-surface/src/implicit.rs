//! Implicit surfaces F(x) = 0 seen through one trait: the interval value and gradient of F over a
//! 3D box (chapter 12.1), so planes, quadrics, tori and fields take part in intersection without
//! `geom-intersect` depending on them.
//!
//! What it does: [`ImplicitBound::enclose_box`] gives enclosures of F and of its gradient over a box
//! of points, and whether F is smooth throughout it. `geom-num`'s exact [`Plane`] implements it,
//! F(x) = n·x + d, its first implementor; the analytic kinds implement it in `analytic`, each with
//! the equation of its whole untrimmed shape (G168), and fields do with `geom-field`.
//!
//! Guarantees:
//!
//! - **Enclosures.** For every point x of the box, F(x) lies in `value` and ∇F(x) in `gradient`;
//!   computed in interval arithmetic rounded outward. A plane's coefficients are integers below
//!   2^126, each held exactly (an integer whose double is not exact is widened by its rounding).
//! - **Smoothness stated.** `smooth` is true only where F is continuously differentiable over the
//!   whole box; a kind that cannot tell says false.
//!
//! Method: interval extensions of F and ∇F (R. E. Moore, R. B. Kearfott and M. J. Cloud,
//! *Introduction to Interval Analysis*, SIAM (2009), <https://doi.org/10.1137/1.9780898717716>),
//! the operand form chapter 12.1 gives implicit surfaces.
//!
//! Cost: constant for a plane: nine interval operations.

use geom_curve::CurveError;
use geom_curve::affine::int_interval;
use geom_num::interval::Interval;
use geom_num::plane::Plane;

/// F and its gradient over a box, enclosed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImplicitEnclosure {
    /// Every value of F over the box.
    pub value: Interval,
    /// Every value of ∇F over the box.
    pub gradient: [Interval; 3],
    /// Whether F is continuously differentiable throughout the box.
    pub smooth: bool,
}

/// An implicit surface F(x) = 0, bounded over boxes.
pub trait ImplicitBound {
    /// F and ∇F over the box `region` (one interval per coordinate), enclosed.
    ///
    /// # Errors
    ///
    /// [`CurveError::NotFinite`] if an enclosure leaves the finite doubles.
    fn enclose_box(&self, region: [Interval; 3]) -> Result<ImplicitEnclosure, CurveError>;
}

impl ImplicitBound for Plane {
    fn enclose_box(&self, region: [Interval; 3]) -> Result<ImplicitEnclosure, CurveError> {
        let n = self.n();
        let mut gradient = [Interval::point(0.0)?; 3];
        let mut value = int_interval(self.d())?;
        for ((g, &ni), x) in gradient.iter_mut().zip(&n).zip(&region) {
            *g = int_interval(ni)?;
            value = value.add(g.mul(*x)?)?;
        }
        Ok(ImplicitEnclosure {
            value,
            gradient,
            smooth: true,
        })
    }
}
