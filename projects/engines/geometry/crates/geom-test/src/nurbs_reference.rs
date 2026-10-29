//! The reference implementation of NURBS curves (chapter 28.9): evaluation, knot insertion,
//! refinement and degree elevation in exact rationals, to check `geom-nurbs` against.
//!
//! What it does: a [`Spline`] holds a degree, a knot vector and control points as exact rationals,
//! in homogeneous form for a rational curve (w·P and then w), converted exactly from the doubles a
//! kernel curve holds: every finite double is a binary fraction, so nothing rounds. Everything else
//! comes from one function, the blossom of a polynomial piece: the symmetric, multi-affine
//! function of p arguments whose value at (u, …, u) is the piece at u. De Boor's algorithm computes
//! it, taking its r-th argument at its r-th level. [`Spline::homogeneous_point`] is the blossom at
//! (u, …, u); [`Spline::point`] divides by the weight. A new knot vector over the same domain, finer
//! or of higher degree, gets its control points from the dual functional property: control point i
//! of a spline of degree q is the blossom of any of its pieces in that point's support, at the q
//! knots that follow knot i. [`Spline::insert`] and [`Spline::refine`] read the original pieces'
//! blossoms that way. [`Spline::elevate`] reads the blossom of each piece seen as a polynomial of
//! degree p + t, which is the mean of the original blossom over every choice of p of its p + t
//! arguments, on the knot vector with each distinct knot repeated t more times.
//!
//! Guarantees:
//!
//! - **Exact.** Every value is an exact rational; nothing rounds, so each result is the exact
//!   result of the operation on the doubles given.
//! - **Independent of the kernel.** Written from the definitions below, without reading
//!   `geom-nurbs`'s code (chapter 28.9: a different author, a different agent session), and by a
//!   different method: blossoms, where the kernel follows The NURBS Book's algorithms as `NLib`
//!   implements them. It shares no code with the kernel and calls none.
//! - **Refused, never guessed.** Malformed input (knots out of order, counts that do not match, a
//!   weight that is not positive, a value that is not finite), a parameter outside the domain, a
//!   knot that cannot be inserted and a curve that cannot be elevated are typed
//!   [`ReferenceError`]s.
//!
//! Method: C. de Boor, "On calculating with B-splines", *Journal of Approximation Theory* 6 (1972),
//! 50–62, <https://doi.org/10.1016/0021-9045(72)90080-9>, for the recurrence; L. Ramshaw,
//! "Blossoms are polar forms", *Computer Aided Geometric Design* 6 (1989), 323–358,
//! <https://doi.org/10.1016/0167-8396(89)90032-0>, for the blossom, the dual functional property
//! and degree elevation as the mean of the blossom over the choices of its arguments; and
//! H.-P. Seidel, "A new multiaffine approach to B-splines", *Computer Aided Geometric Design* 6
//! (1989), 23–32, <https://doi.org/10.1016/0167-8396(89)90003-4>, for de Boor's algorithm as the
//! blossom with one argument per level.
//!
//! Cost: a blossom is O(p²·d) rational operations for dimension d; a point one blossom; insertion
//! and refinement one blossom per new control point; elevation C(p + t, p) blossoms per new control
//! point, which this reference accepts because it runs on small cases only. The rationals grow with
//! the knots' and points' binary expansions; no recursion.

// The lint is for fixed-width arithmetic, which can overflow. Every operator here is on dashu's
// arbitrary-precision rationals, which cannot, and every division is by a value shown nonzero
// beside it (a knot difference across a span longer than one value, a positive weight, a count).
#![allow(
    clippy::arithmetic_side_effects,
    reason = "exact rational arithmetic, which cannot overflow; divisors are shown nonzero"
)]

use dashu_ratio::RBig;

/// Why the reference could not answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferenceError {
    /// The degree is zero.
    Degree,
    /// There are not (number of control points) + degree + 1 knots.
    KnotCount,
    /// The knots decrease somewhere, or a knot is repeated more than degree + 1 times.
    KnotOrder,
    /// The control points do not all have the same dimension, or there are none.
    Points,
    /// A weight is not positive, or there is not one weight per control point.
    Weight,
    /// A knot, coordinate or weight is not finite.
    NotFinite,
    /// The domain [`u_p`, `u_(n+1)`] is a single value.
    EmptyDomain,
    /// A parameter, or a knot to insert, is outside the domain (a knot must be strictly inside).
    Outside,
    /// A knot would be repeated more than the degree allows.
    Multiplicity,
    /// Degree elevation needs a clamped curve.
    NotClamped,
    /// A new control point's support holds no span of the domain, so the curve does not fix it.
    Undetermined,
}

/// A B-spline or NURBS curve with exact rational knots and control points.
#[derive(Clone, Debug, PartialEq)]
pub struct Spline {
    degree: usize,
    knots: Vec<RBig>,
    /// Homogeneous control points: (w·P, w) for a rational curve, P for a nonrational one.
    points: Vec<Vec<RBig>>,
    rational: bool,
}

/// `x` exactly, or `NotFinite`.
fn exact(x: f64) -> Result<RBig, ReferenceError> {
    RBig::try_from(x).map_err(|_| ReferenceError::NotFinite)
}

/// |x|.
fn magnitude(x: &RBig) -> RBig {
    if *x < RBig::ZERO {
        -x.clone()
    } else {
        x.clone()
    }
}

/// The value at `i`, or `Undetermined`: every index below is in range by construction, and a
/// failed read means a shape this reference does not handle, which it refuses.
fn at<T>(items: &[T], i: usize) -> Result<&T, ReferenceError> {
    items.get(i).ok_or(ReferenceError::Undetermined)
}

impl Spline {
    /// The nonrational curve of degree `degree` on `knots` with control points `points`.
    ///
    /// # Errors
    ///
    /// [`ReferenceError::Degree`], [`ReferenceError::Points`], [`ReferenceError::KnotCount`],
    /// [`ReferenceError::NotFinite`], [`ReferenceError::KnotOrder`] or
    /// [`ReferenceError::EmptyDomain`] for malformed input.
    pub fn new(degree: usize, knots: &[f64], points: &[&[f64]]) -> Result<Spline, ReferenceError> {
        let points = points
            .iter()
            .map(|p| p.iter().map(|&x| exact(x)).collect::<Result<Vec<_>, _>>())
            .collect::<Result<Vec<_>, _>>()?;
        Spline::build(degree, knots, points, false)
    }

    /// The rational curve of degree `degree` on `knots` with homogeneous control points
    /// `weighted` (w·P, as a kernel curve stores them) and weights `weights`.
    ///
    /// # Errors
    ///
    /// As [`Spline::new`], and [`ReferenceError::Weight`] unless there is one positive weight per
    /// control point.
    pub fn new_rational(
        degree: usize,
        knots: &[f64],
        weighted: &[&[f64]],
        weights: &[f64],
    ) -> Result<Spline, ReferenceError> {
        if weights.len() != weighted.len() {
            return Err(ReferenceError::Weight);
        }
        let mut points = Vec::new();
        for (p, &w) in weighted.iter().zip(weights) {
            let w = exact(w)?;
            if w <= RBig::ZERO {
                return Err(ReferenceError::Weight);
            }
            let mut h = p.iter().map(|&x| exact(x)).collect::<Result<Vec<_>, _>>()?;
            h.push(w);
            points.push(h);
        }
        Spline::build(degree, knots, points, true)
    }

    fn build(
        degree: usize,
        knots: &[f64],
        points: Vec<Vec<RBig>>,
        rational: bool,
    ) -> Result<Spline, ReferenceError> {
        if degree == 0 {
            return Err(ReferenceError::Degree);
        }
        let dim = points.first().map(Vec::len).ok_or(ReferenceError::Points)?;
        if dim == 0 || points.iter().any(|p| p.len() != dim) {
            return Err(ReferenceError::Points);
        }
        if knots.len() != points.len().saturating_add(degree).saturating_add(1) {
            return Err(ReferenceError::KnotCount);
        }
        let knots = knots
            .iter()
            .map(|&k| exact(k))
            .collect::<Result<Vec<_>, _>>()?;
        let spline = Spline {
            degree,
            knots,
            points,
            rational,
        };
        spline.check_knots()?;
        Ok(spline)
    }

    /// Knots non-decreasing, none repeated more than p + 1 times, and a domain of more than one
    /// value.
    fn check_knots(&self) -> Result<(), ReferenceError> {
        let mut run = 1usize;
        for pair in self.knots.windows(2) {
            let (a, b) = (at(pair, 0)?, at(pair, 1)?);
            if b < a {
                return Err(ReferenceError::KnotOrder);
            }
            run = if a == b { run.saturating_add(1) } else { 1 };
            if run > self.degree.saturating_add(1) {
                return Err(ReferenceError::KnotOrder);
            }
        }
        let (lo, hi) = self.domain()?;
        if lo >= hi {
            return Err(ReferenceError::EmptyDomain);
        }
        Ok(())
    }

    /// The degree p.
    #[must_use]
    pub fn degree(&self) -> usize {
        self.degree
    }

    /// The knots.
    #[must_use]
    pub fn knots(&self) -> &[RBig] {
        &self.knots
    }

    /// The control points in homogeneous form: (w·P, w) for a rational curve, P otherwise.
    #[must_use]
    pub fn homogeneous_points(&self) -> &[Vec<RBig>] {
        &self.points
    }

    /// Whether the curve is rational.
    #[must_use]
    pub fn is_rational(&self) -> bool {
        self.rational
    }

    /// The index of the last control point, n.
    fn last(&self) -> Result<usize, ReferenceError> {
        self.points
            .len()
            .checked_sub(1)
            .ok_or(ReferenceError::Points)
    }

    /// The domain [`u_p`, `u_(n+1)`].
    ///
    /// # Errors
    ///
    /// None for a spline this module built; [`ReferenceError::Undetermined`] otherwise.
    pub fn domain(&self) -> Result<(RBig, RBig), ReferenceError> {
        let end = self.last()?.saturating_add(1);
        Ok((
            at(&self.knots, self.degree)?.clone(),
            at(&self.knots, end)?.clone(),
        ))
    }

    /// Whether each end knot is repeated p + 1 times.
    #[must_use]
    pub fn is_clamped(&self) -> bool {
        let p = self.degree;
        let m = self.knots.len();
        match (self.knots.first(), self.knots.last()) {
            (Some(first), Some(last)) => {
                self.knots
                    .iter()
                    .take(p.saturating_add(1))
                    .all(|k| k == first)
                    && self
                        .knots
                        .iter()
                        .skip(m.saturating_sub(p.saturating_add(1)))
                        .all(|k| k == last)
            }
            _ => false,
        }
    }

    /// The span k of the domain that holds `u`: `u_k` ≤ u < `u_(k+1)`, and at the end of the
    /// domain the last span that is more than one value.
    fn span(&self, u: &RBig) -> Result<usize, ReferenceError> {
        let (lo, hi) = self.domain()?;
        if *u < lo || *u > hi {
            return Err(ReferenceError::Outside);
        }
        let n = self.last()?;
        let mut found = None;
        for k in self.degree..=n {
            let (a, b) = (at(&self.knots, k)?, at(&self.knots, k.saturating_add(1))?);
            if a < b && (*a <= *u && (*u < *b || *b == hi)) {
                found = Some(k);
                if *u < *b {
                    break;
                }
            }
        }
        found.ok_or(ReferenceError::Undetermined)
    }

    /// The blossom of piece k (the span [`u_k`, `u_(k+1)`), control points `P_(k−p)` to `P_k`) at
    /// `args`, which holds p values: de Boor's recurrence with the r-th argument at level r.
    fn blossom(&self, k: usize, args: &[&RBig]) -> Result<Vec<RBig>, ReferenceError> {
        let p = self.degree;
        let first = k.checked_sub(p).ok_or(ReferenceError::Undetermined)?;
        let mut d: Vec<Vec<RBig>> = (first..=k)
            .map(|i| at(&self.points, i).cloned())
            .collect::<Result<_, _>>()?;
        for r in 1..=p {
            let s = *at(args, r.saturating_sub(1))?;
            // Level r: for j from k down to k − p + r, P_j ← (1 − α)·P_(j−1) + α·P_j with
            // α = (s − u_j) / (u_(j+p+1−r) − u_j); the denominator is at least u_(k+1) − u_k > 0.
            for j in (first.saturating_add(r)..=k).rev() {
                let uj = at(&self.knots, j)?;
                let uk = at(
                    &self.knots,
                    j.saturating_add(p).saturating_add(1).saturating_sub(r),
                )?;
                let alpha = (s - uj) / (uk - uj);
                let beta = RBig::ONE - &alpha;
                let lo = j.saturating_sub(first);
                let (prev, cur) = (at(&d, lo.saturating_sub(1))?, at(&d, lo)?);
                let next: Vec<RBig> = prev
                    .iter()
                    .zip(cur)
                    .map(|(a, b)| &beta * a + &alpha * b)
                    .collect();
                *d.get_mut(lo).ok_or(ReferenceError::Undetermined)? = next;
            }
        }
        d.pop().ok_or(ReferenceError::Undetermined)
    }

    /// The homogeneous point at `u`: (w·C(u), w(u)) for a rational curve, C(u) otherwise. At a
    /// knot it is the limit from the right, and at the end of the domain from the left.
    ///
    /// # Errors
    ///
    /// [`ReferenceError::Outside`] if `u` is outside the domain.
    pub fn homogeneous_point(&self, u: &RBig) -> Result<Vec<RBig>, ReferenceError> {
        let k = self.span(u)?;
        let args: Vec<&RBig> = (0..self.degree).map(|_| u).collect();
        self.blossom(k, &args)
    }

    /// The point C(u), as [`Spline::homogeneous_point`] divided by its weight for a rational curve.
    ///
    /// # Errors
    ///
    /// As [`Spline::homogeneous_point`].
    pub fn point(&self, u: &RBig) -> Result<Vec<RBig>, ReferenceError> {
        let mut h = self.homogeneous_point(u)?;
        if self.rational {
            let w = h.pop().ok_or(ReferenceError::Undetermined)?;
            return Ok(h.iter().map(|x| x / &w).collect());
        }
        Ok(h)
    }

    /// The same curve with every homogeneous coordinate replaced by its magnitude: at u its
    /// homogeneous point is Σ `N_j`(u)·|`h_j`| coordinate by coordinate, the sums of magnitudes that
    /// rounding-error bounds are stated in.
    #[must_use]
    pub fn magnitudes(&self) -> Spline {
        Spline {
            degree: self.degree,
            knots: self.knots.clone(),
            points: self
                .points
                .iter()
                .map(|p| p.iter().map(magnitude).collect())
                .collect(),
            rational: self.rational,
        }
    }

    /// The spline of degree `degree` on `knots` that equals this one on the domain, whose control
    /// points the dual functional property fixes: each is the mean of this spline's blossom, on a
    /// piece in the new point's support, over every choice of p of the `degree` knots after it. A
    /// point whose support holds no span of the domain is kept, for insertion only.
    fn respan(&self, degree: usize, knots: Vec<RBig>) -> Result<Spline, ReferenceError> {
        let p = self.degree;
        let count = knots
            .len()
            .checked_sub(degree.saturating_add(1))
            .filter(|c| *c > 0)
            .ok_or(ReferenceError::KnotCount)?;
        let n = count.saturating_sub(1);
        let choices = choose(degree, p);
        let mut points = Vec::new();
        // Whether a point the domain fixes has been met: undetermined points before it belong to
        // the start, after it to the end.
        let mut fixed = false;
        for i in 0..count {
            // A span of the new knots in this point's support and in the domain, more than one value.
            let lo = i.max(degree);
            let hi = i.saturating_add(degree).min(n);
            let mut piece = None;
            for s in lo..=hi {
                let (a, b) = (at(&knots, s)?, at(&knots, s.saturating_add(1))?);
                if a < b {
                    piece = Some(self.span(a)?);
                    break;
                }
            }
            let Some(k) = piece else {
                // The point's basis function vanishes on the whole domain (an unclamped end whose
                // first or last span of the domain is a single value), so the curve does not fix
                // it. Inserting knots inside the domain leaves such a point where it was, as
                // Boehm's insertion does: at the start under the same index, at the end shifted
                // by the number of knots added. A change of degree has no such rule.
                if degree != p {
                    return Err(ReferenceError::Undetermined);
                }
                let added = count.saturating_sub(self.points.len());
                let from = if fixed { i.saturating_sub(added) } else { i };
                points.push(at(&self.points, from)?.clone());
                continue;
            };
            fixed = true;
            let window: Vec<&RBig> = (1..=degree)
                .map(|t| at(&knots, i.saturating_add(t)))
                .collect::<Result<_, _>>()?;
            let mut sum: Option<Vec<RBig>> = None;
            for pick in &choices {
                let args: Vec<&RBig> = pick
                    .iter()
                    .map(|&t| at(&window, t).copied())
                    .collect::<Result<_, _>>()?;
                let value = self.blossom(k, &args)?;
                sum = Some(match sum {
                    None => value,
                    Some(acc) => acc.iter().zip(&value).map(|(a, b)| a + b).collect(),
                });
            }
            let total = RBig::from(dashu_int::UBig::from(choices.len()));
            let mean = sum
                .ok_or(ReferenceError::Undetermined)?
                .iter()
                .map(|x| x / &total)
                .collect();
            points.push(mean);
        }
        Ok(Spline {
            degree,
            knots,
            points,
            rational: self.rational,
        })
    }

    /// How many knots equal `u`.
    fn multiplicity(&self, u: &RBig) -> usize {
        self.knots.iter().filter(|k| *k == u).count()
    }

    /// The same curve with `u` inserted `times` times.
    ///
    /// # Errors
    ///
    /// [`ReferenceError::Outside`] unless `u` is strictly inside the domain;
    /// [`ReferenceError::Multiplicity`] if `u` would be repeated more than p times.
    pub fn insert(&self, u: &RBig, times: usize) -> Result<Spline, ReferenceError> {
        let xs: Vec<RBig> = (0..times).map(|_| u.clone()).collect();
        self.refine(&xs)
    }

    /// The same curve with the knots `xs` inserted, each strictly inside the domain.
    ///
    /// # Errors
    ///
    /// [`ReferenceError::Outside`] unless every value is strictly inside the domain;
    /// [`ReferenceError::Multiplicity`] if a value would be repeated more than p times.
    pub fn refine(&self, xs: &[RBig]) -> Result<Spline, ReferenceError> {
        let (lo, hi) = self.domain()?;
        let mut knots = self.knots.clone();
        for x in xs {
            if *x <= lo || *x >= hi {
                return Err(ReferenceError::Outside);
            }
            knots.push(x.clone());
        }
        knots.sort();
        for x in xs {
            if knots.iter().filter(|k| *k == x).count() > self.degree {
                return Err(ReferenceError::Multiplicity);
            }
        }
        self.respan(self.degree, knots)
    }

    /// The same curve with degree p + `t`: each distinct knot repeated t more times.
    ///
    /// # Errors
    ///
    /// [`ReferenceError::NotClamped`] unless the curve is clamped; [`ReferenceError::Multiplicity`]
    /// if an interior knot is repeated p + 1 times (the curve jumps there).
    pub fn elevate(&self, t: usize) -> Result<Spline, ReferenceError> {
        if !self.is_clamped() {
            return Err(ReferenceError::NotClamped);
        }
        let (lo, hi) = self.domain()?;
        let mut knots = Vec::new();
        let mut distinct: Vec<&RBig> = self.knots.iter().collect();
        distinct.dedup();
        for u in distinct {
            let m = self.multiplicity(u);
            if *u != lo && *u != hi && m > self.degree {
                return Err(ReferenceError::Multiplicity);
            }
            for _ in 0..m.saturating_add(t) {
                knots.push(u.clone());
            }
        }
        self.respan(self.degree.saturating_add(t), knots)
    }
}

/// Every choice of `k` of the indices 0..`n`, each in increasing order, in lexicographic order.
fn choose(n: usize, k: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    let mut pick: Vec<usize> = (0..k).collect();
    if k > n {
        return out;
    }
    loop {
        out.push(pick.clone());
        // The rightmost index that can still move right, moved, and those after it reset.
        let Some(i) = (0..k)
            .rev()
            .find(|&i| pick.get(i).copied() != Some(i.saturating_add(n).saturating_sub(k)))
        else {
            return out;
        };
        let Some(v) = pick.get(i).copied() else {
            return out;
        };
        for (step, slot) in pick.iter_mut().skip(i).enumerate() {
            *slot = v.saturating_add(1).saturating_add(step);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;

    fn r(x: f64) -> RBig {
        RBig::try_from(x).unwrap()
    }

    #[test]
    fn choices_are_every_subset_once() {
        assert_eq!(choose(3, 2), vec![vec![0, 1], vec![0, 2], vec![1, 2]]);
        assert_eq!(choose(4, 4), vec![vec![0, 1, 2, 3]]);
        assert_eq!(choose(5, 2).len(), 10);
        assert_eq!(choose(2, 3), Vec::<Vec<usize>>::new());
    }

    #[test]
    fn a_quadratic_bezier_curve_is_de_casteljau() {
        // B(1/2) of (0, 0), (2, 4), (4, 0) is (2, 2).
        let s = Spline::new(
            2,
            &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            &[&[0.0, 0.0], &[2.0, 4.0], &[4.0, 0.0]],
        )
        .unwrap();
        assert_eq!(s.point(&r(0.5)).unwrap(), vec![r(2.0), r(2.0)]);
        assert_eq!(s.point(&r(1.0)).unwrap(), vec![r(4.0), r(0.0)]);
    }

    #[test]
    fn insertion_and_elevation_keep_the_curve() {
        let s = Spline::new_rational(
            2,
            &[0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0],
            &[&[0.0, 0.0], &[1.0, 2.0], &[3.0, 1.0], &[2.0, 0.0]],
            &[1.0, 2.0, 0.5, 1.0],
        )
        .unwrap();
        let inserted = s.insert(&r(0.25), 2).unwrap();
        let elevated = s.elevate(2).unwrap();
        assert_eq!(elevated.degree(), 4);
        for u in [0.0, 0.125, 0.25, 0.5, 0.75, 1.0] {
            let want = s.point(&r(u)).unwrap();
            assert_eq!(inserted.point(&r(u)).unwrap(), want);
            assert_eq!(elevated.point(&r(u)).unwrap(), want);
        }
    }
}
