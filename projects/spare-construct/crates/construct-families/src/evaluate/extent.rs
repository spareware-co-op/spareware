//! The box each of a family's solids lies in at one type's values, worked out without an engine
//! (piece A64, job 6.4): for a builder that draws every shape as a box, so that a family placed in
//! a model can be drawn before any engine is linked.
//!
//! How: the evaluation runs exactly as [`crate::evaluate()`] runs it (the same values, the same
//! sketches solved, the same steps in the same order, the same limits, turned the same way), over
//! an engine of this file's own whose solid is the box around what it was asked: an extrusion is
//! the box of its profile's corners and of those corners moved along its vector; a subtraction is
//! its target's box (what is left lies within it); a union is the box of both. So every box holds
//! its solid, and is that solid's own box when the solid is an extrusion or a union of them. The
//! boxes are exact grid steps: nothing is rounded.

use super::Built;
use super::error::EvalError;
use super::limits::Limits;
use super::turn::Turn;
use super::values::TypeValues;
use crate::model::{Key, Recipe, Visibility};
use construct_categories::CategoryId;
use construct_params::Length;
use engine_geometry::value::engine::{
    Cut, DisplayMesh, Edition, Engine, EngineError, GridPlane, Profile, Section, Volume,
};
use engine_geometry::value::grid::{GridStep, P3};
use kit_core::Id;

/// The box one of a family's solids lies in, in the family's frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Extent {
    /// The step that made the solid.
    pub step: Key,
    /// The subcategory it is drawn with.
    pub subcategory: CategoryId,
    /// Its material.
    pub material: Id,
    /// Which detail levels show it.
    pub visibility: Visibility,
    /// The box's lowest corner: x, y and z.
    pub low: [Length; 3],
    /// The box's highest corner.
    pub high: [Length; 3],
}

/// The box of what this file's engine was asked, in grid steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Reach {
    low: [i64; 3],
    high: [i64; 3],
}

impl Reach {
    fn of(points: impl IntoIterator<Item = [i64; 3]>) -> Option<Reach> {
        let mut reach: Option<Reach> = None;
        for p in points {
            reach = Some(match reach {
                None => Reach { low: p, high: p },
                Some(r) => r.with(Reach { low: p, high: p }),
            });
        }
        reach
    }

    fn with(self, other: Reach) -> Reach {
        let pick = |a: [i64; 3], b: [i64; 3], f: fn(i64, i64) -> i64| {
            [f(a[0], b[0]), f(a[1], b[1]), f(a[2], b[2])]
        };
        Reach {
            low: pick(self.low, other.low, i64::min),
            high: pick(self.high, other.high, i64::max),
        }
    }
}

/// An engine that keeps the box of each solid it is asked for, and nothing else.
struct Boxes;

fn unanswered(operation: &'static str) -> EngineError {
    EngineError::Unsupported {
        operation,
        what: "anything: this engine keeps only the box a solid lies in".to_owned(),
    }
}

impl Engine for Boxes {
    type Solid = Reach;

    fn edition(&self) -> Edition {
        Edition {
            engine: "construct-families.extents".to_owned(),
            version: "1".to_owned(),
            step: GridStep::Q0,
            tolerances: Vec::new(),
            answers: Vec::new(),
        }
    }

    fn extrude(&mut self, profile: &Profile, by: [i64; 3]) -> Result<Reach, EngineError> {
        let steps = |p: &P3| [p.x.get(), p.y.get(), p.z.get()];
        let mut corners = Vec::with_capacity(profile.outer.len().saturating_mul(2));
        for p in &profile.outer {
            let foot = steps(p);
            let mut head = [0_i64; 3];
            for ((slot, along), from) in head.iter_mut().zip(by).zip(foot) {
                *slot = from
                    .checked_add(along)
                    .ok_or_else(|| EngineError::Invalid {
                        operation: "extrude",
                        what: "a corner moved beyond a 64-bit number of steps".to_owned(),
                    })?;
            }
            corners.push(foot);
            corners.push(head);
        }
        Reach::of(corners).ok_or_else(|| EngineError::Invalid {
            operation: "extrude",
            what: "a profile with no corners".to_owned(),
        })
    }

    fn subtract(&mut self, target: &Reach, _tool: &Reach) -> Result<Reach, EngineError> {
        Ok(*target)
    }

    fn subtract_pieces(&mut self, _: &Reach, _: &Reach) -> Result<Vec<Reach>, EngineError> {
        Err(unanswered("subtract_pieces"))
    }

    fn loft(&mut self, _: &[Section]) -> Result<Reach, EngineError> {
        Err(unanswered("loft"))
    }

    fn unite(&mut self, a: &Reach, b: &Reach) -> Result<Reach, EngineError> {
        Ok(a.with(*b))
    }

    fn volume(&mut self, _: &Reach) -> Result<Volume, EngineError> {
        Err(unanswered("volume"))
    }

    fn mesh(&mut self, _: &Reach) -> Result<DisplayMesh, EngineError> {
        Err(unanswered("mesh"))
    }

    fn section(&mut self, _: &Reach, _: &GridPlane) -> Result<Cut, EngineError> {
        Err(unanswered("section"))
    }
}

/// The box each of `recipe`'s solids lies in for the type whose values are `values`, turned by
/// `turn`, in recipe order, within `limits`; no engine is asked.
///
/// # Errors
///
/// As [`crate::evaluate()`], but for the engine's own refusals, which this one never gives for
/// the steps the format has.
pub fn extents(
    recipe: &Recipe,
    values: &TypeValues,
    limits: Limits,
    turn: Turn,
) -> Result<Vec<Extent>, EvalError> {
    let evaluated = super::build::run(
        &mut Boxes,
        recipe,
        values,
        limits,
        super::build::Stand::Turned(turn),
    )?;
    Ok(evaluated
        .solids
        .into_iter()
        .map(|built: Built<Reach>| Extent {
            step: built.step,
            subcategory: built.subcategory,
            material: built.material,
            visibility: built.visibility,
            low: built.solid.low.map(Length::from_steps),
            high: built.solid.high.map(Length::from_steps),
        })
        .collect())
}
