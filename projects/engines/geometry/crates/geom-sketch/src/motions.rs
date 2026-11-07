// SPDX-License-Identifier: Apache-2.0
//! Free motions, sparse and per entity (piece G224; chapter 15.8, "free motions").
//!
//! What it does: [`MotionFinder`] factors a sketch's Jacobian once, sparsely, and then gives the
//! directions one entity can still move in, or all of them, without ever forming the dense null
//! space. Each direction is a [`crate::diagnose::Motion`], the same canonical echelon form as
//! [`crate::diagnose::free_motions`], which stays as the dense reference the tests hold this to.
//!
//! - **Which variables lead.** The canonical basis puts a 1 at each vector's leading variable and 0
//!   at the others'. The leading variables are the lexicographically first basis of the null
//!   space's rows, which is the complement of the lexicographically last basis of J's columns
//!   (matroid duality, Oxley, *Matroid Theory*, 2nd ed., 2011, section 2.1). So a left-looking
//!   sparse QR of J with its columns taken from the last variable to the first marks a column dead
//!   exactly when it is a combination of the columns after it; the dead columns lead.
//! - **One vector.** For leading variable f, the other leading variables are held at 0 and the rest
//!   solve J_P·x = -J_f, with P the live columns: one least-squares solve against the sparse QR
//!   of J_P, charged by the factors' size, not by the number of variables.
//!
//! Guarantees:
//!
//! - **Same answer as the dense form** to rounding, motion for motion, in the same order.
//! - **Deterministic.** Natural column order for the scan, COLAMD (ties by variable order) for the
//!   solve; plain scalar loops (`geom-linalg`).
//! - **Charged first** to the meter: the factorisations when the finder is made, each solve when
//!   it is asked for.
//! - **No panics, no recursion, no unsafe code.**
//!
//! Method: section 15.8 (`projects/engines/geometry/architecture/15-sketches.md`), with
//! `geom-linalg`'s sparse QR for the factors (piece G215).
//!
//! Cost: one sparse QR of J to find the leading variables and one of J_P, once; then 2·(|V| + |R|)
//! multiply-adds and an O(n) vector for each motion. No recursion.

extern crate alloc;

use alloc::vec::Vec;

use geom_linalg::Work;
use geom_linalg::sparse::Sparse;
use geom_linalg::sparse_qr::{SparseQr, SymbolicQr};
use geom_num::real::Real;
use geom_run::WorkMeter;

use crate::diagnose::Motion;
use crate::equation::Assembly;
use crate::model::{ItemId, Sketch, SketchError, Variable};
use crate::solve::INDEPENDENT;

/// A motion's entries below this are not reported (as the dense form).
const NEGLIGIBLE: f64 = 1e-12;

/// A sketch's Jacobian, factored for free motions.
#[derive(Clone, Debug)]
pub struct MotionFinder {
    variables: Vec<Variable>,
    jacobian: Sparse,
    /// The leading variables, in increasing order.
    leading: Vec<usize>,
    /// The other variables, in increasing order: the columns of `factors`.
    others: Vec<usize>,
    /// The QR of J restricted to `others`; none when there are no other variables.
    factors: Option<SparseQr>,
}

/// Charges the meter what `work` has used of `start`.
fn settle(meter: &mut WorkMeter<'_>, start: u64, work: &Work) -> Result<(), SketchError> {
    meter.charge(start.saturating_sub(work.left()))?;
    Ok(())
}

/// `n - 1 - j`: a column's place when the columns run backwards.
fn mirror(n: usize, j: usize) -> Result<usize, SketchError> {
    n.checked_sub(j)
        .and_then(|k| k.checked_sub(1))
        .ok_or(SketchError::Internal)
}

impl MotionFinder {
    /// Factors the Jacobian of `sketch` at its own values.
    ///
    /// # Errors
    ///
    /// [`SketchError::Stopped`] when the budget runs out; what assembling and factoring refuse.
    pub fn new(sketch: &Sketch, meter: &mut WorkMeter<'_>) -> Result<MotionFinder, SketchError> {
        let mut assembly = Assembly::new(sketch)?;
        assembly.evaluate(sketch)?;
        let variables: Vec<Variable> = assembly.variables().to_vec();
        let jacobian = assembly.jacobian().clone();
        let (rows, n) = (jacobian.rows(), jacobian.cols());
        let tol = Real::new(INDEPENDENT)?;
        let start = u64::MAX;
        let mut work = Work::new(start);
        // The scan: columns from the last variable to the first.
        let mut triplets = Vec::new();
        for j in 0..n {
            let (indices, values) = jacobian.column(j)?;
            let reversed = mirror(n, j)?;
            for (&i, &v) in indices.iter().zip(values) {
                triplets.push((i, reversed, v));
            }
        }
        let mut leading: Vec<usize> = Vec::new();
        if n > 0 {
            let scan = Sparse::from_triplets(rows, n, &triplets)?;
            let symbolic = SymbolicQr::natural(&scan, &mut work)?;
            let qr = SparseQr::factor(&symbolic, &scan, tol, &mut work)?;
            for d in qr.dead_columns() {
                leading.push(mirror(n, d)?);
            }
        }
        leading.sort_unstable();
        let others: Vec<usize> = (0..n)
            .filter(|j| leading.binary_search(j).is_err())
            .collect();
        let factors = if others.is_empty() {
            None
        } else {
            let mut kept = Vec::new();
            for (c, &j) in others.iter().enumerate() {
                let (indices, values) = jacobian.column(j)?;
                for (&i, &v) in indices.iter().zip(values) {
                    kept.push((i, c, v));
                }
            }
            let live = Sparse::from_triplets(rows, others.len(), &kept)?;
            let symbolic = SymbolicQr::new(&live, &mut work)?;
            Some(SparseQr::factor(&symbolic, &live, tol, &mut work)?)
        };
        settle(meter, start, &work)?;
        Ok(MotionFinder {
            variables,
            jacobian,
            leading,
            others,
            factors,
        })
    }

    /// The sketch's degrees of freedom: one for each leading variable.
    #[must_use]
    pub fn dof(&self) -> usize {
        self.leading.len()
    }

    /// The motion whose leading variable is `index`.
    fn motion(&self, index: usize, meter: &mut WorkMeter<'_>) -> Result<Motion, SketchError> {
        let mut solved: Vec<(usize, f64)> = Vec::new();
        if let Some(qr) = &self.factors {
            let (indices, values) = self.jacobian.column(index)?;
            let mut rhs = Vec::new();
            rhs.try_reserve_exact(self.jacobian.rows())
                .map_err(|_| SketchError::OutOfMemory)?;
            rhs.resize(self.jacobian.rows(), 0.0);
            for (&i, &v) in indices.iter().zip(values) {
                *rhs.get_mut(i).ok_or(SketchError::Internal)? = -v;
            }
            let start = u64::MAX;
            let mut work = Work::new(start);
            let x = qr.least_squares(&rhs, &mut work)?;
            settle(meter, start, &work)?;
            for (&j, &value) in self.others.iter().zip(&x) {
                if value.abs() > NEGLIGIBLE {
                    solved.push((j, value));
                }
            }
        }
        // The leading variable's own 1, among the others' entries, in variable order.
        let place = solved.partition_point(|&(j, _)| j < index);
        solved.insert(place, (index, 1.0));
        let mut moves = Vec::new();
        moves
            .try_reserve_exact(solved.len())
            .map_err(|_| SketchError::OutOfMemory)?;
        for (j, x) in solved {
            let var = self.variables.get(j).ok_or(SketchError::Internal)?;
            // The solver sees value / scale: a unit of its variable is `scale` model units.
            moves.push((var.owner, var.slot, x * var.scale));
        }
        Ok(Motion { moves })
    }

    /// The directions entity `id` can still move in: the motions whose leading variable belongs to
    /// it, in variable order.
    ///
    /// # Errors
    ///
    /// [`SketchError::Stopped`] when the budget runs out; [`SketchError::OutOfMemory`].
    pub fn of_entity(
        &self,
        id: ItemId,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<Motion>, SketchError> {
        let mut out = Vec::new();
        for &j in &self.leading {
            if self.variables.get(j).ok_or(SketchError::Internal)?.owner == id {
                out.push(self.motion(j, meter)?);
            }
        }
        Ok(out)
    }

    /// Every direction the sketch can still move in: the same motions as
    /// [`crate::diagnose::free_motions`].
    ///
    /// # Errors
    ///
    /// As [`MotionFinder::of_entity`].
    pub fn all(&self, meter: &mut WorkMeter<'_>) -> Result<Vec<Motion>, SketchError> {
        let mut out = Vec::new();
        for &j in &self.leading {
            out.push(self.motion(j, meter)?);
        }
        Ok(out)
    }
}
