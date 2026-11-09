//! Where a curve crosses a plane of the plane table, and how far a curve strays from one (11.4
//! step 2, G243a).
//!
//! What it does: [`Against`] is a curve's offset from a plane, n · C(t) + d with the plane's own
//! integers, as a field of one unknown for `geom_intersect::solve`, so [`crossings`] finds every
//! parameter where the curve meets the plane, each proven the only one in its box, or a flagged
//! region where that could not be decided. [`Hull`] holds a curve's NURBS form as enclosures of its
//! control points, with the form's certified deviation, and [`Hull::spread`] encloses the signed
//! distance from a plane of every point of the curve at once: a NURBS curve with positive weights
//! is at each parameter a convex combination of its control points, so n · C + d lies between the
//! least and the greatest of the control points' values. A curve whose spread the contact rule
//! reads as touching lies on the plane; one whose spread is proven of one sign never meets it.
//!
//! Guarantees:
//!
//! - **Every crossing found or flagged.** [`crossings`] is the interval engine of chapter 12.3: a
//!   parameter box is dropped only when its residual excludes zero or Krawczyk's operator proves it
//!   empty, so a crossing no zero or flag reports does not exist, and each zero is a simple root,
//!   the only one in its proof box, where the curve passes from one side of the plane to the other.
//! - **The plane is the table's.** n and d are the plane's integers, each enclosed by the doubles
//!   around it, never a rounded copy of the plane, so the decision is about the carrier itself.
//! - **The spread holds the curve.** Every point of the curve lies within [`Hull::spread`] of the
//!   plane, signed: the control points' values are enclosed outward, divided by an enclosure of
//!   |n|, and widened by the NURBS form's certified deviation. A form with a weight that is not
//!   positive and finite has no such hull, and its spread is the widest interval, never a guess.
//!
//! Method: the residual and its Jacobian are enclosed from the curve kind's own certified bounds
//! (`ParamCurve::bound`, `ParamCurve::derivative_bound`) and handed to
//! `geom_intersect::solve::solve`, chapter 12.3
//! (`projects/engines/geometry/architecture/12-intersections.md`): Krawczyk's operator on a
//! subdivision work list. The hull is the convex-hull property of rational B-splines with positive
//! weights (L. Piegl and W. Tiller, *The NURBS Book*, 2nd edition, Springer (1997), property P4.10,
//! <https://doi.org/10.1007/978-3-642-59223-2>).
//!
//! Cost: [`Hull::of`] is one NURBS conversion of the curve; [`Hull::spread`] is linear in its
//! control points, with no allocation. [`crossings`] costs what the engine's boxes cost, each one
//! bound of the curve, charged to the meter. No recursion.

use alloc::vec::Vec;

use geom_intersect::IntersectError;
use geom_intersect::solve::{Field, Solved, solve};
use geom_num::NumError;
use geom_num::interval::Interval;
use geom_num::plane::Plane;
use geom_run::WorkMeter;
use geom_topo::{CurveKind, DerivativeBound, ParamCurve};

use crate::distance::enclose_i128;

/// A plane's integers, enclosed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Enclosed {
    /// The normal n.
    pub n: [Interval; 3],
    /// The offset d.
    pub d: Interval,
    /// |n|.
    pub norm: Interval,
}

impl Enclosed {
    /// The plane's integers, each enclosed by the doubles around it.
    ///
    /// # Errors
    ///
    /// [`NumError`] if an enclosure cannot be formed.
    pub fn of(plane: &Plane) -> Result<Enclosed, NumError> {
        let [a, b, c] = plane.n();
        let n = [enclose_i128(a)?, enclose_i128(b)?, enclose_i128(c)?];
        Ok(Enclosed {
            n,
            d: enclose_i128(plane.d())?,
            norm: Interval::norm(&n)?,
        })
    }

    /// n · p + d over the enclosure `p`.
    ///
    /// # Errors
    ///
    /// [`NumError`] if an enclosure overflows.
    pub fn value(&self, p: &[Interval; 3]) -> Result<Interval, NumError> {
        let [n0, n1, n2] = self.n;
        let [x, y, z] = *p;
        n0.mul(x)?.add(n1.mul(y)?)?.add(n2.mul(z)?)?.add(self.d)
    }

    /// n · v over the enclosure `v` of a direction.
    fn along(&self, v: &[Interval; 3]) -> Result<Interval, NumError> {
        let [n0, n1, n2] = self.n;
        let [x, y, z] = *v;
        n0.mul(x)?.add(n1.mul(y)?)?.add(n2.mul(z)?)
    }
}

/// The enclosure of a box given by its corners.
fn boxed(lo: [f64; 3], hi: [f64; 3]) -> Result<[Interval; 3], NumError> {
    let [lx, ly, lz] = lo;
    let [hx, hy, hz] = hi;
    Ok([
        Interval::new(lx, hx)?,
        Interval::new(ly, hy)?,
        Interval::new(lz, hz)?,
    ])
}

/// A curve's offset from a plane, n · C(t) + d, as a field of one unknown.
pub struct Against<'a> {
    /// The curve.
    pub curve: &'a CurveKind<3>,
    /// The plane.
    pub plane: Enclosed,
}

impl Field<1> for Against<'_> {
    fn at(&self, x: [f64; 1], meter: &mut WorkMeter<'_>) -> Result<[Interval; 1], IntersectError> {
        let [t] = x;
        #[allow(
            clippy::default_trait_access,
            reason = "the side is geom-nurbs' Limit, which this crate's dependencies do not name; its default is the usual limit from the right"
        )]
        let jet = self.curve.enclose(t, Default::default(), 0, meter)?;
        let p = jet.first().ok_or(IntersectError::Internal)?;
        Ok([self.plane.value(p)?])
    }

    fn over(
        &self,
        x: &[Interval; 1],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[Interval; 1]>, IntersectError> {
        let [t] = x;
        let b = self.curve.bound((t.lo(), t.hi()), meter)?;
        Ok(Some([self.plane.value(&boxed(b.lo, b.hi)?)?]))
    }

    fn slope(
        &self,
        x: &[Interval; 1],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[[Interval; 1]; 1]>, IntersectError> {
        let [t] = x;
        match self.curve.derivative_bound((t.lo(), t.hi()), 1, meter)? {
            DerivativeBound::Bounded(b) => Ok(Some([[self.plane.along(&boxed(b.lo, b.hi)?)?]])),
            DerivativeBound::Unbounded => Ok(None),
        }
    }

    fn reach(&self, x: &[Interval; 1], meter: &mut WorkMeter<'_>) -> Result<f64, IntersectError> {
        let [t] = x;
        let b = self.curve.bound((t.lo(), t.hi()), meter)?;
        let mut widest = 0.0_f64;
        for (lo, hi) in b.lo.iter().zip(&b.hi) {
            let w = hi - lo;
            if w > widest {
                widest = w;
            }
        }
        Ok(widest)
    }
}

/// Every parameter of `curve`'s domain where it meets `plane`: zeros proven alone in their boxes,
/// and flagged regions.
///
/// # Errors
///
/// The curve kind's, an enclosure's, or the meter's stop.
pub fn crossings(
    curve: &CurveKind<3>,
    plane: &Plane,
    meter: &mut WorkMeter<'_>,
) -> Result<Solved<1>, IntersectError> {
    let (a, b) = curve.domain();
    let field = Against {
        curve,
        plane: Enclosed::of(plane)?,
    };
    solve(&field, [Interval::new(a, b)?], |_, _| Ok(true), meter)
}

/// A curve's NURBS form as enclosures of its control points, with the form's certified deviation.
#[derive(Clone, Debug, PartialEq)]
pub struct Hull {
    /// The control points, each enclosed: w·P / w, outward.
    points: Vec<[Interval; 3]>,
    /// The NURBS form's certified distance from the curve.
    deviation: f64,
    /// Whether every weight is positive and finite, so the curve lies in its control points' hull.
    convex: bool,
}

impl Hull {
    /// The hull of `curve`.
    ///
    /// # Errors
    ///
    /// The curve kind's conversion's errors, such as an unbounded line.
    pub fn of(curve: &CurveKind<3>, meter: &mut WorkMeter<'_>) -> Result<Hull, IntersectError> {
        let form = curve.to_nurbs(meter)?;
        let nurbs = &form.value.curve;
        let weights = nurbs.weights();
        let mut convex = form.deviation.is_finite() && form.deviation >= 0.0;
        let mut points = Vec::with_capacity(nurbs.homogeneous().len());
        for (i, h) in nurbs.homogeneous().iter().enumerate() {
            let w = match weights {
                Some(list) => list.get(i).copied().ok_or(IntersectError::Internal)?,
                None => 1.0,
            };
            if !(w > 0.0 && w.is_finite()) {
                convex = false;
                continue;
            }
            let w = Interval::point(w)?;
            let [x, y, z] = *h;
            points.push([
                Interval::point(x)?.div(w)?,
                Interval::point(y)?.div(w)?,
                Interval::point(z)?.div(w)?,
            ]);
        }
        let convex = convex && !points.is_empty();
        Ok(Hull {
            points,
            deviation: form.deviation,
            convex,
        })
    }

    /// An enclosure of the signed distance from `plane` of every point of the curve: negative on
    /// the side the normal points away from.
    ///
    /// # Errors
    ///
    /// [`NumError`] if an enclosure overflows.
    pub fn spread(&self, plane: &Enclosed) -> Result<Interval, NumError> {
        if !self.convex {
            return Interval::new(f64::MIN, f64::MAX);
        }
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for p in &self.points {
            let v = plane.value(p)?;
            if v.lo() < lo {
                lo = v.lo();
            }
            if v.hi() > hi {
                hi = v.hi();
            }
        }
        let signed = Interval::new(lo, hi)?.div(plane.norm)?;
        Interval::new(
            (signed.lo() - self.deviation).next_down(),
            (signed.hi() + self.deviation).next_up(),
        )
    }
}
