//! The solid steps, run in recipe order through the engine.
//!
//! - `extrude`: the profile less its holes, from the lower of its two heights to the higher, along
//!   the sketch plane's normal (the order the two are written in does not matter; equal heights
//!   are refused). The corners sit at the plane's offset plus the lower height.
//! - `subtract`: its target less its tool, both earlier steps. An extrusion used as a tool is the
//!   recipe's void: it cuts exactly the target its subtract step names, and nothing else.
//! - `unite`: its two earlier steps as one solid.
//!
//! A step a later step uses is that step's input and is not drawn; every other step is one of
//! the family's solids, in recipe order, with its subcategory, material and visibility.
//!
//! Each step is counted against the solid-step limit before it runs, and each extrusion's
//! corners against the result-size limit before the engine is asked (`limits.rs`).

use super::error::{EvalError, Place, Reason};
use super::limits::{Limits, Meter};
use super::sketch::{Drawn, resolve};
use super::values::{TypeValues, Worked, work_out};
use super::{Built, Evaluated};
use crate::check::Sort;
use crate::model::{Key, MaterialRef, Operation, Recipe, Step};
use engine_geometry::value::engine::{Engine, Profile};
use engine_geometry::value::grid::GridStep;
use kit_core::Id;
use std::collections::{BTreeMap, BTreeSet};

/// Evaluates `recipe` for the type whose values are `values`, asking `engine` for every solid,
/// within `limits`.
pub(crate) fn run<E: Engine>(
    engine: &mut E,
    recipe: &Recipe,
    values: &TypeValues,
    limits: Limits,
) -> Result<Evaluated<E::Solid>, EvalError> {
    let step = engine.edition().step;
    if step != GridStep::Q0 {
        return Err(EvalError::new(Place::Engine, Reason::WrongGrid { step }));
    }
    let mut meter = Meter::new(limits);
    let worked = work_out(recipe, values, &mut meter)?;
    let mut sketches: BTreeMap<Key, Drawn> = BTreeMap::new();
    let mut built: Vec<(Key, E::Solid)> = Vec::with_capacity(recipe.steps.len());
    let mut used: BTreeSet<Key> = BTreeSet::new();
    let mut materials: Vec<Id> = Vec::with_capacity(recipe.steps.len());
    for step in &recipe.steps {
        let at = |reason| EvalError::new(Place::Step(step.key.clone()), reason).in_step(&step.key);
        if built.iter().any(|(key, _)| *key == step.key) {
            return Err(at(Reason::Duplicate {
                key: step.key.as_str().to_owned(),
            }));
        }
        meter.solid_step().map_err(at)?;
        let solid = match &step.operation {
            Operation::Extrude {
                sketch,
                profile,
                holes,
                start,
                end,
            } => {
                if !sketches.contains_key(sketch) {
                    let drawn = resolve(recipe, sketch, &worked, &mut meter)
                        .map_err(|e| e.in_step(&step.key))?;
                    sketches.insert(sketch.clone(), drawn);
                }
                let drawn = sketches.get(sketch).ok_or_else(|| {
                    at(Reason::Missing {
                        sort: Sort::Sketch,
                        key: sketch.as_str().to_owned(),
                    })
                })?;
                let heights = (start.as_str(), end.as_str());
                extrude(
                    engine,
                    drawn,
                    &worked,
                    &mut meter,
                    (profile, holes),
                    heights,
                )
                .map_err(at)?
            }
            Operation::Subtract { target, tool } => {
                let target_solid = input(recipe, &built, step, target).map_err(at)?;
                let tool_solid = input(recipe, &built, step, tool).map_err(at)?;
                let solid = engine
                    .subtract(target_solid, tool_solid)
                    .map_err(|e| at(Reason::Engine(e)))?;
                used.extend([target.clone(), tool.clone()]);
                solid
            }
            Operation::Unite { a, b } => {
                let a_solid = input(recipe, &built, step, a).map_err(at)?;
                let b_solid = input(recipe, &built, step, b).map_err(at)?;
                let solid = engine
                    .unite(a_solid, b_solid)
                    .map_err(|e| at(Reason::Engine(e)))?;
                used.extend([a.clone(), b.clone()]);
                solid
            }
        };
        materials.push(material(&worked, step).map_err(at)?);
        built.push((step.key.clone(), solid));
    }
    let solids = recipe
        .steps
        .iter()
        .zip(built)
        .zip(materials)
        .filter(|((step, _), _)| !used.contains(&step.key))
        .map(|((step, (_, solid)), material)| Built {
            step: step.key.clone(),
            solid,
            subcategory: step.subcategory,
            material,
            visibility: step.visibility,
        })
        .collect();
    Ok(Evaluated {
        solids,
        values: worked.all,
        used: meter.used(),
    })
}

/// One extrusion step's solid.
fn extrude<E: Engine>(
    engine: &mut E,
    drawn: &Drawn,
    worked: &Worked,
    meter: &mut Meter,
    (profile, holes): (&[Key], &[Vec<Key>]),
    (start, end): (&str, &str),
) -> Result<E::Solid, Reason> {
    let start = worked.length(start, meter)?;
    let end = worked.length(end, meter)?;
    let (low, high) = if start <= end {
        (start, end)
    } else {
        (end, start)
    };
    if low == high {
        return Err(Reason::NoThickness);
    }
    let depth = high.checked_sub(low).ok_or(Reason::OutOfRange)?;
    let outer = drawn.corners(profile, low)?;
    let holes = holes
        .iter()
        .map(|hole| drawn.corners(hole, low))
        .collect::<Result<Vec<_>, _>>()?;
    meter.corners(
        holes
            .iter()
            .fold(outer.len(), |n, h| n.saturating_add(h.len())),
    )?;
    engine
        .extrude(&Profile { outer, holes }, drawn.along(depth))
        .map_err(Reason::Engine)
}

/// An earlier step's solid, which `step` uses.
fn input<'b, S>(
    recipe: &Recipe,
    built: &'b [(Key, S)],
    step: &Step,
    key: &Key,
) -> Result<&'b S, Reason> {
    if let Some((_, solid)) = built.iter().find(|(k, _)| k == key) {
        return Ok(solid);
    }
    let named = key.as_str().to_owned();
    if *key == step.key || recipe.steps.iter().any(|s| s.key == *key) {
        Err(Reason::NotEarlier { key: named })
    } else {
        Err(Reason::Missing {
            sort: Sort::Step,
            key: named,
        })
    }
}

/// A step's material: fixed, or its material parameter's value for this type.
fn material(worked: &Worked, step: &Step) -> Result<Id, Reason> {
    match &step.material {
        MaterialRef::Fixed(id) => Ok(id.clone()),
        MaterialRef::Parameter(key) => worked.material(key),
    }
}
