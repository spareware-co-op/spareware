//! The arithmetic the knot operations run in twice: doubles for the result, and intervals for a
//! certified bound on that result's rounding.
//!
//! What it does: each knot operation (insertion, refinement, Bezier extraction, removal, degree
//! change) is written once, generic over [`Num`], and run once in `f64` and once in `geom-num`'s
//! [`Interval`]. The two runs make the same operations in the same order on the same data. Every
//! interval operation rounds to nearest and widens by one step, so it contains the exact result of
//! the operation for any operands in its inputs; rounding to nearest is monotone, so it also
//! contains the double the `f64` run computes from operands inside those inputs. By induction over
//! the run, each interval contains both the exact value and the double, and its width bounds their
//! distance. [`Hom`] is a homogeneous control point, w·P with its weight w, combined as `NLib`'s
//! `N_Combine2CPts` combines them.
//!
//! Guarantees:
//!
//! - **Enclosure.** For a computation written once over [`Num`], each value of the interval run
//!   contains the exact value and the value of the `f64` run (the argument above; geom-num's
//!   `interval` module proves the inclusion of each operation).
//! - **The same doubles as a plain computation.** The `f64` implementation is IEEE 754's correctly
//!   rounded operation, nothing else: no fused multiply-add, no reordering.
//! - **Typed failures.** An interval bound that leaves the finite doubles, or a divisor interval
//!   that contains zero, is [`NurbsError::NotFinite`]; the `f64` run is checked for finite results
//!   where the operations collect them.
//!
//! Method: interval arithmetic with outward rounding by one step (R. E. Moore, R. B. Kearfott and
//! M. J. Cloud, *Introduction to Interval Analysis*, SIAM (2009),
//! <https://doi.org/10.1137/1.9780898717716>) as `geom-num` implements it, used as a running error
//! bound for a floating-point computation that makes the same operations (N. J. Higham, *Accuracy
//! and Stability of Numerical Algorithms*, 2nd ed., SIAM (2002), section 3.3,
//! <https://doi.org/10.1137/1.9780898718027>).
//!
//! Cost: constant per operation; nothing allocates.

use geom_num::interval::Interval;

use crate::NurbsError;

/// A number the knot operations compute with: `f64`, or an [`Interval`] that encloses it.
pub(crate) trait Num: Copy {
    /// `x` exactly.
    fn of(x: f64) -> Result<Self, NurbsError>;
    /// `self + o`.
    fn plus(self, o: Self) -> Result<Self, NurbsError>;
    /// `self − o`.
    fn minus(self, o: Self) -> Result<Self, NurbsError>;
    /// `self · o`.
    fn times(self, o: Self) -> Result<Self, NurbsError>;
    /// `self / o`.
    fn over(self, o: Self) -> Result<Self, NurbsError>;
}

impl Num for f64 {
    fn of(x: f64) -> Result<Self, NurbsError> {
        Ok(x)
    }

    fn plus(self, o: Self) -> Result<Self, NurbsError> {
        Ok(self + o)
    }

    fn minus(self, o: Self) -> Result<Self, NurbsError> {
        Ok(self - o)
    }

    fn times(self, o: Self) -> Result<Self, NurbsError> {
        Ok(self * o)
    }

    fn over(self, o: Self) -> Result<Self, NurbsError> {
        Ok(self / o)
    }
}

impl Num for Interval {
    fn of(x: f64) -> Result<Self, NurbsError> {
        Interval::point(x).map_err(|_| NurbsError::NotFinite)
    }

    fn plus(self, o: Self) -> Result<Self, NurbsError> {
        self.add(o).map_err(|_| NurbsError::NotFinite)
    }

    fn minus(self, o: Self) -> Result<Self, NurbsError> {
        self.sub(o).map_err(|_| NurbsError::NotFinite)
    }

    fn times(self, o: Self) -> Result<Self, NurbsError> {
        self.mul(o).map_err(|_| NurbsError::NotFinite)
    }

    fn over(self, o: Self) -> Result<Self, NurbsError> {
        self.div(o).map_err(|_| NurbsError::NotFinite)
    }
}

/// A homogeneous control point: the weighted coordinates w·P and the weight w. A nonrational
/// curve's points carry the weight 1, which the operations update like any coordinate and the
/// results then drop, so it never changes a coordinate.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Hom<N, const D: usize> {
    pub(crate) c: [N; D],
    pub(crate) w: N,
}

impl<N: Num, const D: usize> Hom<N, D> {
    /// The point with weighted coordinates `c` and weight `w`, exactly.
    pub(crate) fn of(c: [f64; D], w: f64) -> Result<Self, NurbsError> {
        let mut out = [N::of(0.0)?; D];
        for (o, x) in out.iter_mut().zip(c) {
            *o = N::of(x)?;
        }
        Ok(Hom {
            c: out,
            w: N::of(w)?,
        })
    }

    /// `self + a·x`, coordinate by coordinate and for the weight, each as `self_d + a·x_d`
    /// (`NLib`'s `N_VectorBlendCPt`).
    pub(crate) fn add_scaled(self, a: N, x: Self) -> Result<Self, NurbsError> {
        let mut coords = self.c;
        for (cd, xd) in coords.iter_mut().zip(x.c) {
            *cd = cd.plus(a.times(xd)?)?;
        }
        Ok(Hom {
            c: coords,
            w: self.w.plus(a.times(x.w)?)?,
        })
    }

    /// `a·x + b·y`, coordinate by coordinate and for the weight, each as `a·x_d + b·y_d`
    /// (`NLib`'s `N_Combine2CPts`).
    pub(crate) fn combine(
        alpha: N,
        first: Self,
        beta: N,
        second: Self,
    ) -> Result<Self, NurbsError> {
        let mut coords = first.c;
        for (cd, (fd, sd)) in coords.iter_mut().zip(first.c.iter().zip(second.c)) {
            *cd = alpha.times(*fd)?.plus(beta.times(sd)?)?;
        }
        Ok(Hom {
            c: coords,
            w: alpha.times(first.w)?.plus(beta.times(second.w)?)?,
        })
    }
}
