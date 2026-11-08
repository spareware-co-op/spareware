//! The checks that follow a commit: every stored root choice decided again, every bounded constraint
//! held to an interval bound, and the fit ceiling.
//!
//! What it does: [`Checker::new`] builds a sketch's equations once. [`Checker::check`] then takes a
//! [`Committed`] sketch (`commit`) and runs chapter 15.10's first two checks, in order. **Root
//! choices:** a side, an outside or a tangency stored on a constraint is decided again with exact
//! predicates on the committed geometry; one that has flipped, or whose point now lies on the
//! boundary where the choice cannot be told, fails the commit with [`SketchError::CommitFlip`],
//! naming the constraint. **Ceiling:** the residual of every constraint the commit did not make
//! exact is enclosed in interval arithmetic on the committed values, by the very text the solver
//! differentiates (`Assembly::enclose`), and must be at most the ceiling, or the commit fails with
//! [`SketchError::ExactnessNotAccepted`], naming the constraint and its bound. What it returns is
//! each constraint's [`Holds`]: `Exact`, `Within` a bound in grid steps, or `Unresolved`.
//!
//! Guarantees:
//!
//! - **A bound is a bound.** A `Within` residual encloses the true residual of the stored geometry:
//!   each input is an interval holding the stored value (a rational point's coordinate is X / W
//!   enclosed outward; a circle through a defining point has the square root of its squared radius
//!   enclosed), and every operation rounds outward. The bound is the larger magnitude of the
//!   interval, so it is never below the exact residual.
//! - **Root choices by exact predicates.** A side is `Rational2::orient` of the line's stored ends
//!   and the point; an outside is `Circle2::side` or `Round::side`; two circles are tangent from
//!   outside when the distance between their centres squared is more than the sum of their radii
//!   squared. No rounding decides one. A choice on geometry the commit did not give a form to (an
//!   ellipse, a spline) is not checked here.
//! - **Nothing changes.** The check reads the sketch and the commit and writes nothing; a failure
//!   leaves both exactly as they were.
//! - **Deterministic.** Constraints are taken in authoring order; the first to fail is the one
//!   reported.
//!
//! Method: chapter 15.10's "Three checks then run" (`projects/engines/geometry/architecture/
//! 15-sketches.md`): root choices decided again on the committed geometry, and a bounded residual
//! "evaluated on the stored geometry in interval arithmetic" against the fit ceiling
//! (`geom_num::ladder::FIT`); interval arithmetic as in R. E. Moore, R. B. Kearfott and M. J.
//! Cloud, *Introduction to Interval Analysis*, SIAM, 2009, doi:10.1137/1.9780898717716, through
//! `geom_num::interval::Interval`, with the residual written once over `geom_num::scalar::Scalar`.
//!
//! Cost: one interval evaluation of every rule, and one exact predicate for every root choice;
//! linear in the sketch, no recursion. A 1,000-entity sketch is checked in about 2 ms
//! (`benches/check.rs`).

extern crate alloc;

use alloc::vec::Vec;

use geom_num::arc::Round;
use geom_num::carrier::Rational2;
use geom_num::interval::Interval;
use geom_num::ladder::FIT;
use geom_num::{NumError, Sign};
use geom_run::WorkMeter;

use crate::commit::{Committed, Mark, Stored, find};
use crate::equation::Assembly;
use crate::model::{
    ConstraintKind, EntityKind, ItemId, Side, Sketch, SketchError, Standing, Variable,
};

/// What the checks found a constraint to hold to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Holds {
    /// Exactly: the commit verified it with exact predicates.
    Exact,
    /// To within this bound, in grid steps: the larger magnitude of its residual's enclosure on the
    /// stored geometry.
    Within {
        /// The bound.
        residual: f64,
    },
    /// Its residual could not be enclosed (a divisor that holds zero, a square root of a negative
    /// range): the stored geometry is too near a singular configuration for a bound.
    Unresolved,
}

/// The result of the checks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Checked {
    /// What every driving constraint holds to, in authoring order.
    pub holds: Vec<(ItemId, Holds)>,
}

impl Checked {
    /// What the constraint `id` holds to.
    #[must_use]
    pub fn get(&self, id: ItemId) -> Option<Holds> {
        find(&self.holds, id).copied()
    }
}

/// A sketch's equations, built once, to check commits of it against.
pub struct Checker {
    assembly: Assembly,
}

fn get<T: Copy>(list: &[T], i: usize) -> Result<T, SketchError> {
    list.get(i).copied().ok_or(SketchError::Internal)
}

fn cost(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// The largest `i128` a `f64` holds exactly: 2^53.
const EXACT: i128 = 9_007_199_254_740_992;

/// An interval holding the integer `v`.
fn whole(v: i128) -> Result<Interval, SketchError> {
    #[allow(
        clippy::cast_precision_loss,
        reason = "past 2^53 the interval is widened by an ulp each way, so it still holds v"
    )]
    let f = v as f64;
    if v.unsigned_abs() <= EXACT.unsigned_abs() {
        Ok(Interval::point(f)?)
    } else {
        Ok(Interval::new(f.next_down(), f.next_up())?)
    }
}

/// An interval holding the rational point's coordinate `x / w`.
fn ratio(x: i128, w: i128) -> Result<Interval, SketchError> {
    Ok(whole(x)?.div(whole(w)?)?)
}

/// An interval holding a stored point's coordinates.
fn coordinates(point: &Stored) -> Result<[Interval; 2], SketchError> {
    let ([x, y], w) = point.rational().coordinates();
    Ok([ratio(x, w)?, ratio(y, w)?])
}

/// The sign of the turn from `a` to `b` to `p`: positive when `p` is to the left of `a` to `b`.
fn turn(a: &Stored, b: &Stored, p: &Stored) -> Result<Sign, SketchError> {
    Ok(Rational2::orient(
        &a.rational(),
        &b.rational(),
        &p.rational(),
    )?)
}

/// What a circle's committed form says of the rational point `p`: negative inside, zero on it,
/// positive outside.
fn circle_side(c: &Committed, circle: ItemId, p: &Rational2) -> Result<Option<Sign>, SketchError> {
    if let Some(k) = find(&c.circles, circle) {
        return Ok(Some(k.side(p)?));
    }
    if let Some(r) = find(&c.rounds, circle) {
        return Ok(Some(r.side(p)?));
    }
    Ok(find(&c.arcs, circle)
        .map(|a| a.round().side(p))
        .transpose()?)
}

/// A circle's committed radius squared and its centre as grid coordinates, if the commit gave it a
/// grid centre: (centre x, centre y, radius squared).
fn square_of(c: &Committed, circle: ItemId) -> Option<(i128, i128, i128)> {
    let squared = |on: [i128; 2], centre: [i128; 2]| {
        let (dx, dy) = (on[0].checked_sub(centre[0])?, on[1].checked_sub(centre[1])?);
        dx.checked_mul(dx)?.checked_add(dy.checked_mul(dy)?)
    };
    let grid = |r: &Round| -> Option<[i128; 2]> {
        let ([x, y], w) = r.centre().coordinates();
        (w == 1).then_some([x, y])
    };
    if let Some(k) = find(&c.circles, circle) {
        let at = k.centre();
        let r = k.radius();
        return Some((
            i128::from(at.x.get()),
            i128::from(at.y.get()),
            r.checked_mul(r)?,
        ));
    }
    let r = find(&c.rounds, circle)?;
    let centre = grid(r)?;
    let on = r.on();
    Some((
        centre[0],
        centre[1],
        squared([i128::from(on.x.get()), i128::from(on.y.get())], centre)?,
    ))
}

/// The stored root choices decided again on the committed geometry: `Err(CommitFlip)` for the first
/// that has flipped or can no longer be told.
fn root_choices(sketch: &Sketch, c: &Committed) -> Result<(), SketchError> {
    let ends = |line: ItemId| -> Result<Option<(&Stored, &Stored)>, SketchError> {
        let [a, b] = sketch.line(line)?;
        Ok(find(&c.points, a).zip(find(&c.points, b)))
    };
    let wants = |side: Side| match side {
        Side::Left => Sign::Positive,
        Side::Right => Sign::Negative,
    };
    for rule in sketch.constraints() {
        if rule.standing != Standing::Driving {
            continue;
        }
        let holds = match rule.kind {
            ConstraintKind::FromLine {
                point, line, side, ..
            } => match (ends(line)?, find(&c.points, point)) {
                (Some((a, b)), Some(p)) => Some(turn(a, b, p)? == wants(side)),
                _ => None,
            },
            ConstraintKind::LinesApart { a, b, side, .. } => {
                let [start, _] = sketch.line(b)?;
                match (ends(a)?, find(&c.points, start)) {
                    (Some((p, q)), Some(r)) => Some(turn(p, q, r)? == wants(side)),
                    _ => None,
                }
            }
            ConstraintKind::TangentLine { line, circle, side } => {
                let centre = sketch.centre(circle)?;
                match (ends(line)?, find(&c.points, centre)) {
                    (Some((p, q)), Some(r)) => Some(turn(p, q, r)? == wants(side)),
                    _ => None,
                }
            }
            ConstraintKind::FromCircle {
                point,
                circle,
                outside,
                ..
            } => match find(&c.points, point) {
                Some(p) => circle_side(c, circle, &p.rational())?.map(|side| {
                    side == if outside {
                        Sign::Positive
                    } else {
                        Sign::Negative
                    }
                }),
                None => None,
            },
            ConstraintKind::TangentCircles { a, b, outside } => {
                match (square_of(c, a), square_of(c, b)) {
                    (Some((ax, ay, ra)), Some((bx, by, rb))) => {
                        let (dx, dy) = (ax.checked_sub(bx), ay.checked_sub(by));
                        let apart = dx.zip(dy).and_then(|(dx, dy)| {
                            dx.checked_mul(dx)?.checked_add(dy.checked_mul(dy)?)
                        });
                        let sum = ra.checked_add(rb);
                        apart.zip(sum).map(
                            |(apart, sum)| {
                                if outside { apart > sum } else { apart < sum }
                            },
                        )
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        if holds == Some(false) {
            return Err(SketchError::CommitFlip(rule.id));
        }
    }
    Ok(())
}

impl Checker {
    /// The equations of `sketch`, with each row's lengths fixed at its solved values.
    ///
    /// # Errors
    ///
    /// What assembling and scaling refuse.
    pub fn new(sketch: &Sketch) -> Result<Checker, SketchError> {
        let mut assembly = Assembly::new(sketch)?;
        let start = assembly.start()?;
        assembly.rescale(&start)?;
        Ok(Checker { assembly })
    }

    /// The committed values of the variables, as intervals holding them.
    fn held(&self, sketch: &Sketch, c: &Committed) -> Result<Vec<Interval>, SketchError> {
        held_of(sketch, c, self.assembly.variables())
    }

    /// Runs the first two of chapter 15.10's checks on a committed sketch.
    ///
    /// # Errors
    ///
    /// [`SketchError::CommitFlip`] for the first stored root choice that has flipped;
    /// [`SketchError::ExactnessNotAccepted`] for the first constraint held to more than `ceiling`
    /// grid steps ([`geom_num::ladder::FIT`] is the ceiling unless the caller accepts more);
    /// [`SketchError::Internal`]; [`SketchError::OutOfMemory`]; [`SketchError::Stopped`] when the
    /// meter stops it.
    pub fn check(
        &self,
        sketch: &Sketch,
        committed: &Committed,
        ceiling: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Checked, SketchError> {
        meter.charge(cost(sketch.constraints().len()).saturating_mul(8))?;
        root_choices(sketch, committed)?;
        let held = self.held(sketch, committed)?;
        let enclosed = self.assembly.enclose(&held)?;
        let mut out = Checked::default();
        out.holds
            .try_reserve_exact(sketch.constraints().len())
            .map_err(|_| SketchError::OutOfMemory)?;
        for rule in sketch.constraints() {
            if rule.standing != Standing::Driving {
                continue;
            }
            let mark = committed.mark(rule.id);
            let holds = if mark == Some(Mark::Exact) {
                Holds::Exact
            } else {
                match find(&enclosed, rule.id) {
                    Some(Some(residuals)) => {
                        let mut bound = 0.0_f64;
                        for r in residuals {
                            let magnitude = if r.lo().abs() >= r.hi().abs() {
                                r.lo().abs()
                            } else {
                                r.hi().abs()
                            };
                            if magnitude > bound {
                                bound = magnitude;
                            }
                        }
                        if bound > ceiling {
                            return Err(SketchError::ExactnessNotAccepted {
                                constraint: rule.id,
                                bound,
                            });
                        }
                        Holds::Within { residual: bound }
                    }
                    _ => Holds::Unresolved,
                }
            };
            out.holds.push((rule.id, holds));
        }
        Ok(out)
    }

    /// The fit ceiling in grid steps: [`geom_num::ladder::FIT`].
    #[must_use]
    pub fn fit() -> f64 {
        #[allow(clippy::cast_precision_loss, reason = "64, a small whole number")]
        let steps = FIT.get() as f64;
        steps
    }
}

/// The committed values of `variables`, as intervals holding them: a stored point's coordinates, a
/// circle's radius, the solved value of anything the commit does not give a form to.
pub(crate) fn held_of(
    sketch: &Sketch,
    c: &Committed,
    variables: &[Variable],
) -> Result<Vec<Interval>, SketchError> {
    let mut held = Vec::new();
    held.try_reserve_exact(variables.len())
        .map_err(|_| SketchError::OutOfMemory)?;
    for v in variables {
        let entity = sketch.entity(v.owner)?;
        let value = match &entity.kind {
            EntityKind::Point { .. } | EntityKind::Reference { .. } => {
                match find(&c.points, v.owner) {
                    Some(p) => get(&coordinates(p)?, usize::from(v.slot))?,
                    None => Interval::point(v.value)?,
                }
            }
            EntityKind::Circle { .. } => {
                match (find(&c.circles, v.owner), find(&c.rounds, v.owner)) {
                    (Some(k), _) => whole(k.radius())?,
                    (None, Some(r)) => radius_of(r)?,
                    _ => Interval::point(v.value)?,
                }
            }
            EntityKind::Arc { .. } => match find(&c.arcs, v.owner) {
                Some(a) => radius_of(&a.round())?,
                None => Interval::point(v.value)?,
            },
            _ => Interval::point(v.value)?,
        };
        held.push(value);
    }
    Ok(held)
}

/// An interval holding the radius of a committed circle through a point: the square root of the
/// squared distance from its centre to the point on it.
fn radius_of(r: &Round) -> Result<Interval, SketchError> {
    let on = r.on();
    let ([cx, cy], w) = r.centre().coordinates();
    let (dx, dy) = (
        whole(i128::from(on.x.get()))?.sub(ratio(cx, w)?)?,
        whole(i128::from(on.y.get()))?.sub(ratio(cy, w)?)?,
    );
    dx.square()?
        .add(dy.square()?)?
        .sqrt()
        .map_err(|e: NumError| SketchError::Numeric(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The meter is charged for what was asked: a count, or all of it when it cannot be told.
    #[test]
    fn the_cost_is_the_count() {
        assert_eq!(cost(0), 0);
        assert_eq!(cost(7), 7);
        assert_eq!(cost(1_000), 1_000);
    }

    /// A whole number a double holds is the point itself; past 2^53 the interval widens by an
    /// ulp each way and still holds it.
    #[test]
    fn a_whole_number_is_held_tightly_where_a_double_holds_it() {
        for v in [0, 3, -3, EXACT, -9_007_199_254_740_992] {
            let i = whole(v).unwrap();
            assert_eq!(i.lo().to_bits(), i.hi().to_bits(), "{v}");
        }
        for v in [
            9_007_199_254_740_993,
            -9_007_199_254_740_993,
            27_021_597_764_222_977,
        ] {
            let i = whole(v).unwrap();
            assert!(i.lo() < i.hi(), "{v}");
            #[allow(
                clippy::cast_precision_loss,
                reason = "the test reads the widened ends"
            )]
            let f = v as f64;
            assert!(i.lo() <= f && f <= i.hi(), "{v}");
        }
    }
}
