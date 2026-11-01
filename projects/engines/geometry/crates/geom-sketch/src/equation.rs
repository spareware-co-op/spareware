//! Equations: each constraint's residual, written once over [`Scalar`], its singular set, and the
//! assembly of a sketch's residuals and Jacobian.
//!
//! What it does: a kind implements [`Residual`]: its residual function, a length that is zero exactly
//! when the constraint holds, and the part of the variables where it has no derivative. Its
//! derivatives come from running that one function on dual numbers (`geom_num::dual`), never from a
//! second text. [`Assembly`] reads a sketch's pattern once: which variables each driving constraint
//! touches, in authoring order, and the Jacobian's stored entries. [`Assembly::evaluate`] then fills
//! the residuals and the Jacobian, in the variables the solver sees: each divided by its power of
//! two, so each column is multiplied by it. It checks the start against each singular set first.
//!
//! Guarantees:
//!
//! - **Derivatives are generated.** A Jacobian entry is the dual number's, and tests hold it to
//!   central differences of the same residual (chapter 15.4).
//! - **Every residual is a length, with a gradient of length one.** At the solution each residual
//!   equals the distance it measures to first order.
//! - **Smooth forms.** A distance d uses (|PQ|² − d²) / 2d, a polynomial in the variables.
//! - **Singular sets are declared.** [`Residual::singular`] is checked before differentiating, and a
//!   start on one is [`SketchError::Degenerate`], naming the constraint. The remedy of section 15.4,
//!   a perturbation derived from keys, is the solver's (G222).
//! - **The pattern is structure alone.** [`Assembly::new`] reads no value, so it is built once for a
//!   sketch's structure and reused for every evaluation.
//! - **Deterministic.** Constraints run in authoring order, and a NaN anywhere is
//!   [`SketchError::Numeric`] from the operation that made it.
//!
//! Method: section 15.4 (`projects/engines/geometry/architecture/15-sketches.md`), on the forward
//! mode of `geom_num::dual`; the point-line distance as a cross product over a length, the standard
//! form, H. S. M. Coxeter, *Introduction to Geometry*, 2nd ed., Wiley (1969), section 1.4.
//!
//! Cost: [`Assembly::new`] sorts the Jacobian's entries once; [`Assembly::evaluate`] is linear in the
//! constraints, with no allocation. No recursion.

extern crate alloc;

use alloc::vec::Vec;

use geom_linalg::Sparse;
use geom_num::NumError;
use geom_num::dual::jacobian;
use geom_num::real::Real;
use geom_num::scalar::Scalar;

use crate::model::{Constraint, ConstraintKind, ItemId, Sketch, SketchError, Standing, Variable};

/// What a variable or two may not be for a residual to have a derivative.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Singular {
    /// A line of no length has no direction.
    ZeroLengthLine,
}

/// A constraint kind's equations: `E` residuals on `N` variables.
pub trait Residual<const N: usize, const E: usize> {
    /// The residuals at `x`, written once over [`Scalar`], so that the one text gives values,
    /// enclosures and derivatives.
    ///
    /// # Errors
    ///
    /// What the arithmetic refuses.
    fn residuals<S: Scalar>(&self, x: [S; N]) -> Result<[S; E], NumError>;

    /// Whether `x` is on the singular set.
    fn singular(&self, _x: &[f64; N]) -> Option<Singular> {
        None
    }
}

/// The two points are one.
struct Coincident;

impl Residual<4, 2> for Coincident {
    fn residuals<S: Scalar>(&self, x: [S; 4]) -> Result<[S; 2], NumError> {
        let [ax, ay, bx, by] = x;
        Ok([ax.sub(bx)?, ay.sub(by)?])
    }
}

/// Two values are one: a horizontal or vertical line's two coordinates.
struct Equal;

impl Residual<2, 1> for Equal {
    fn residuals<S: Scalar>(&self, x: [S; 2]) -> Result<[S; 1], NumError> {
        let [a, b] = x;
        Ok([a.sub(b)?])
    }
}

/// (|PQ|² − d²) / 2d.
struct Distance {
    distance: Real,
}

impl Residual<4, 1> for Distance {
    fn residuals<S: Scalar>(&self, x: [S; 4]) -> Result<[S; 1], NumError> {
        let [ax, ay, bx, by] = x;
        let d = S::constant(self.distance);
        let size = ax.sub(bx)?.square()?.add(ay.sub(by)?.square()?)?;
        let twice = d.add(d)?;
        Ok([size.sub(d.square()?)?.div(twice)?])
    }
}

/// The signed distance from p to the line ab, less the stored side times d.
struct PointLine {
    distance: Real,
    sign: Real,
}

impl Residual<6, 1> for PointLine {
    fn residuals<S: Scalar>(&self, x: [S; 6]) -> Result<[S; 1], NumError> {
        let [px, py, ax, ay, bx, by] = x;
        let (dx, dy) = (bx.sub(ax)?, by.sub(ay)?);
        let cross = dx.mul(py.sub(ay)?)?.sub(dy.mul(px.sub(ax)?)?)?;
        let size = dx.square()?.add(dy.square()?)?.sqrt()?;
        let want = S::constant(self.sign).mul(S::constant(self.distance))?;
        Ok([cross.div(size)?.sub(want)?])
    }

    fn singular(&self, x: &[f64; 6]) -> Option<Singular> {
        let [_, _, ax, ay, bx, by] = *x;
        (ax == bx && ay == by).then_some(Singular::ZeroLengthLine)
    }
}

/// A variable less its fixed value.
struct Fix {
    value: Real,
}

impl Residual<1, 1> for Fix {
    fn residuals<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        let [a] = x;
        Ok([a.sub(S::constant(self.value))?])
    }
}

fn zeros(n: usize) -> Result<Vec<f64>, SketchError> {
    let mut v = Vec::new();
    v.try_reserve_exact(n)
        .map_err(|_| SketchError::OutOfMemory)?;
    v.resize(n, 0.0);
    Ok(v)
}

/// One driving constraint in an assembly: where its variables and its Jacobian entries are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Row {
    id: ItemId,
    /// Its first residual.
    first: usize,
    /// Its variables, at `Assembly::touched[at..]`.
    at: usize,
}

/// A sketch's residuals and Jacobian, on its pattern.
#[derive(Clone, Debug, PartialEq)]
pub struct Assembly {
    variables: Vec<Variable>,
    rows: Vec<Row>,
    touched: Vec<usize>,
    /// The stored place of each constraint's entries, in its equations and variables' order.
    places: Vec<usize>,
    /// The first of each constraint's places.
    place_from: Vec<usize>,
    residuals: Vec<f64>,
    jacobian: Sparse,
}

/// The variable index of `(owner, slot)` among `variables`.
fn index(variables: &[Variable], owner: ItemId, slot: u8) -> Result<usize, SketchError> {
    variables
        .binary_search_by_key(&(owner, slot), |v| (v.owner, v.slot))
        .map_err(|_| SketchError::WrongKind(owner))
}

/// Which variables `constraint` touches, and how many equations it has.
fn touched_by(
    sketch: &Sketch,
    constraint: &Constraint,
    variables: &[Variable],
    out: &mut Vec<usize>,
) -> Result<usize, SketchError> {
    let mut add = |owner: ItemId, slot: u8| -> Result<(), SketchError> {
        out.try_reserve(1).map_err(|_| SketchError::OutOfMemory)?;
        out.push(index(variables, owner, slot)?);
        Ok(())
    };
    Ok(match constraint.kind {
        ConstraintKind::Coincident { a, b } => {
            add(a, 0)?;
            add(a, 1)?;
            add(b, 0)?;
            add(b, 1)?;
            2
        }
        ConstraintKind::Horizontal { line } => {
            let [a, b] = sketch.line(line)?;
            add(a, 1)?;
            add(b, 1)?;
            1
        }
        ConstraintKind::Vertical { line } => {
            let [a, b] = sketch.line(line)?;
            add(a, 0)?;
            add(b, 0)?;
            1
        }
        ConstraintKind::Distance { a, b, .. } => {
            add(a, 0)?;
            add(a, 1)?;
            add(b, 0)?;
            add(b, 1)?;
            1
        }
        ConstraintKind::PointLine { point, line, .. } => {
            let [a, b] = sketch.line(line)?;
            add(point, 0)?;
            add(point, 1)?;
            add(a, 0)?;
            add(a, 1)?;
            add(b, 0)?;
            add(b, 1)?;
            1
        }
        ConstraintKind::Fix { entity, slot, .. } => {
            add(entity, slot)?;
            1
        }
    })
}

impl Assembly {
    /// The pattern of `sketch`'s driving constraints.
    ///
    /// # Errors
    ///
    /// [`SketchError::OutOfMemory`]; [`SketchError::Linear`].
    pub fn new(sketch: &Sketch) -> Result<Assembly, SketchError> {
        let variables = sketch.variables()?;
        let (mut rows, mut touched, mut triplets) = (Vec::new(), Vec::new(), Vec::new());
        let mut place_from = Vec::new();
        let mut equations = 0_usize;
        for constraint in sketch.constraints() {
            if constraint.standing != Standing::Driving {
                continue;
            }
            let at = touched.len();
            let here = touched_by(sketch, constraint, &variables, &mut touched)?;
            place_from
                .try_reserve(1)
                .map_err(|_| SketchError::OutOfMemory)?;
            place_from.push(triplets.len());
            let list = touched.get(at..).ok_or(SketchError::OutOfMemory)?;
            for e in 0..here {
                let row = equations.checked_add(e).ok_or(SketchError::OutOfMemory)?;
                for &column in list {
                    triplets
                        .try_reserve(1)
                        .map_err(|_| SketchError::OutOfMemory)?;
                    triplets.push((row, column, 0.0));
                }
            }
            rows.try_reserve(1).map_err(|_| SketchError::OutOfMemory)?;
            rows.push(Row {
                id: constraint.id,
                first: equations,
                at,
            });
            equations = equations
                .checked_add(here)
                .ok_or(SketchError::OutOfMemory)?;
        }
        let jacobian = Sparse::from_triplets(equations, variables.len(), &triplets)?;
        let mut places = Vec::new();
        places
            .try_reserve_exact(triplets.len())
            .map_err(|_| SketchError::OutOfMemory)?;
        for &(row, column, _) in &triplets {
            places.push(jacobian.place_of(row, column)?);
        }
        Ok(Assembly {
            residuals: zeros(equations)?,
            variables,
            rows,
            touched,
            places,
            place_from,
            jacobian,
        })
    }

    /// The variables, in the Jacobian's column order.
    #[must_use]
    pub fn variables(&self) -> &[Variable] {
        &self.variables
    }

    /// The residuals, in steps, after [`Assembly::evaluate`].
    #[must_use]
    pub fn residuals(&self) -> &[f64] {
        &self.residuals
    }

    /// The Jacobian, by the scaled variables, after [`Assembly::evaluate`].
    #[must_use]
    pub const fn jacobian(&self) -> &Sparse {
        &self.jacobian
    }

    /// Runs one kind: reads its variables, checks its singular set, and stores what it gives.
    fn run<R: Residual<N, E>, const N: usize, const E: usize>(
        &mut self,
        kind: &R,
        place: usize,
        x: &[f64],
    ) -> Result<(), SketchError> {
        let row = *self.rows.get(place).ok_or(SketchError::OutOfMemory)?;
        let list: &[usize; N] = self
            .touched
            .get(row.at..)
            .and_then(|tail| tail.get(..N))
            .and_then(|part| part.try_into().ok())
            .ok_or(SketchError::OutOfMemory)?;
        let mut values = [0.0; N];
        for (value, &i) in values.iter_mut().zip(list) {
            *value = *x.get(i).ok_or(SketchError::OutOfMemory)?;
        }
        if kind.singular(&values).is_some() {
            return Err(SketchError::Degenerate(row.id));
        }
        let mut reals = [Real::ZERO; N];
        for (real, &value) in reals.iter_mut().zip(&values) {
            *real = Real::new(value)?;
        }
        let (result, partials) = jacobian::<Real, _, N, E>(|d| kind.residuals(d), reals)?;
        let from = *self.place_from.get(place).ok_or(SketchError::OutOfMemory)?;
        let mut stored = self
            .places
            .get(from..)
            .ok_or(SketchError::OutOfMemory)?
            .iter();
        for (e, (value, row_partials)) in result.iter().zip(&partials).enumerate() {
            let at = row.first.checked_add(e).ok_or(SketchError::OutOfMemory)?;
            *self.residuals.get_mut(at).ok_or(SketchError::OutOfMemory)? = value.get();
            for (partial, &i) in row_partials.iter().zip(list) {
                let scale = self.variables.get(i).ok_or(SketchError::OutOfMemory)?.scale;
                let spot = *stored.next().ok_or(SketchError::OutOfMemory)?;
                self.jacobian.set_at(spot, partial.get() * scale)?;
            }
        }
        Ok(())
    }

    /// Fills the residuals and the Jacobian at the values of `sketch`, which must have this pattern.
    ///
    /// # Errors
    ///
    /// [`SketchError::Degenerate`] for a start on a singular set, naming the constraint;
    /// [`SketchError::Numeric`] for what the arithmetic refuses.
    pub fn evaluate(&mut self, sketch: &Sketch) -> Result<(), SketchError> {
        let values = sketch.variables()?;
        let mut x = Vec::new();
        x.try_reserve_exact(values.len())
            .map_err(|_| SketchError::OutOfMemory)?;
        x.extend(values.iter().map(|v| v.value));
        let driving = sketch
            .constraints()
            .iter()
            .filter(|c| c.standing == Standing::Driving);
        for (place, constraint) in driving.enumerate() {
            self.one(constraint, place, &x)?;
        }
        Ok(())
    }

    fn one(&mut self, constraint: &Constraint, place: usize, x: &[f64]) -> Result<(), SketchError> {
        match constraint.kind {
            ConstraintKind::Coincident { .. } => self.run(&Coincident, place, x),
            ConstraintKind::Horizontal { .. } | ConstraintKind::Vertical { .. } => {
                self.run(&Equal, place, x)
            }
            ConstraintKind::Distance { distance, .. } => self.run(
                &Distance {
                    distance: Real::new(distance)?,
                },
                place,
                x,
            ),
            ConstraintKind::PointLine { distance, side, .. } => self.run(
                &PointLine {
                    distance: Real::new(distance)?,
                    sign: Real::new(side.sign())?,
                },
                place,
                x,
            ),
            ConstraintKind::Fix { value, .. } => self.run(
                &Fix {
                    value: Real::new(value)?,
                },
                place,
                x,
            ),
        }
    }
}

/// What a reference dimension measures now: the distance between its points, or the point's distance
/// from the line, signed as its stored side. Reported, never solved.
///
/// # Errors
///
/// [`SketchError::NotADimension`] for any other constraint; [`SketchError::Numeric`].
pub fn measure(sketch: &Sketch, constraint: &Constraint) -> Result<f64, SketchError> {
    let at = |id: ItemId| sketch.point(id);
    match constraint.kind {
        ConstraintKind::Distance { a, b, .. } => {
            let (p, q) = (at(a)?, at(b)?);
            let (dx, dy) = (Real::new(p[0] - q[0])?, Real::new(p[1] - q[1])?);
            Ok(dx.square()?.add(dy.square()?)?.sqrt()?.get())
        }
        ConstraintKind::PointLine {
            point, line, side, ..
        } => {
            let [a, b] = sketch.line(line)?;
            let (p, a, b) = (at(point)?, at(a)?, at(b)?);
            let x = [p[0], p[1], a[0], a[1], b[0], b[1]].map(Real::new);
            let [px, py, ax, ay, bx, by] = [x[0]?, x[1]?, x[2]?, x[3]?, x[4]?, x[5]?];
            let rule = PointLine {
                distance: Real::ZERO,
                sign: Real::new(side.sign())?,
            };
            if rule
                .singular(&[p[0], p[1], a[0], a[1], b[0], b[1]])
                .is_some()
            {
                return Err(SketchError::Degenerate(constraint.id));
            }
            let [signed] = rule.residuals([px, py, ax, ay, bx, by])?;
            Ok(signed.get() * side.sign())
        }
        _ => Err(SketchError::NotADimension(constraint.id)),
    }
}
