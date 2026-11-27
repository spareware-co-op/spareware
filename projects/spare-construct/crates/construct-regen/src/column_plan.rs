//! A column (`column`, job 14.7, CON-A5-F1-P1) as both builders read it: its filled recipe's ring
//! of corners, height and material, in the steps of construct-params' length grid, checked for what
//! a builder needs and nothing more.
//!
//! What it reads: the recipe construct-columns' `finish` writes, which the regenerator builds from
//! ([`crate::columns`]): `ring`, the section's corners about the column's centre, turned,
//! anticlockwise seen from above, each `[x, y]` in grid steps; `height`, a construct-params stored
//! length, from the column's base (its base level's elevation plus its offset, the height its built
//! row holds) up to its top; `material`, a material's ID; and `grids`, the grid lines it stands on
//! (the regenerator's, not the builders'). The column is the prism on its ring from `z = 0` up to
//! `z = height`.
//!
//! What it promises: a recipe it reads has a ring of at least three corners, every corner on the
//! kernel's grid (within 2^40 - 1 steps of the origin), and a height above zero. That the ring is
//! simple is construct-columns' section (a rectangle or a regular polygon, turned), and the
//! engine's checker judges every solid it returns.
//!
//! Depends on construct-model (the recipe) and construct-params (the stored length). Nothing here
//! is interim: every length is exact in construct-params' unit; the round section's polygon is
//! construct-columns' interim.

use construct_model::geometry::{GeometryError, Recipe};
use construct_params::Value as Stored;
use serde_json::Value;

/// A column's recipe operation (construct-columns' `COLUMN_OP`).
pub const OP: &str = "column";
/// The version of a column's recipe this module reads (construct-columns' `OP_VERSION`).
pub const OP_VERSION: u32 = 1;
/// The owner of Spare Construct's own operations.
const OWN: &str = "construct";
/// The farthest a corner may be from the origin along an axis: the kernel's grid, 2^40 - 1 steps.
const GRID_LIMIT: i64 = 1_099_511_627_775;

fn fault(code: &str, message: impl Into<String>) -> GeometryError {
    GeometryError {
        code: code.to_owned(),
        message: message.into(),
    }
}

/// Whether `recipe` is a column's.
#[must_use]
pub fn is_column(recipe: &Recipe) -> bool {
    recipe.owner == OWN && recipe.op == OP
}

/// A column read from its filled recipe ([`ColumnPlan::of`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnPlan {
    /// The ring of corners, anticlockwise from above, in grid steps.
    pub ring: Vec<[i64; 2]>,
    /// From its base to its top, in grid steps; above zero.
    pub height: i64,
    /// What it is made of: a material's ID.
    pub material: String,
}

impl ColumnPlan {
    /// Reads a column's filled recipe.
    ///
    /// # Errors
    ///
    /// `NotAColumn` for a recipe of another operation or version; `Unfilled` for one whose section
    /// or height was not read before building; `MissingArgument` for a ring, a height or a
    /// material not as the module header says; `EmptyShape` for a ring of fewer than three
    /// corners; `NoHeight` for a height at or below zero; `TooBig` for a corner beyond the grid.
    pub fn of(recipe: &Recipe) -> Result<ColumnPlan, GeometryError> {
        if !is_column(recipe) || recipe.op_version != OP_VERSION {
            return Err(fault(
                "NotAColumn",
                format!(
                    "a recipe of {}/{} version {} is not a column's",
                    recipe.owner, recipe.op, recipe.op_version
                ),
            ));
        }
        let args = &recipe.args;
        let ring = args.get("ring").and_then(Value::as_array).ok_or_else(|| {
            fault(
                "Unfilled",
                "its ring is worked out from its type's section, and was not before building",
            )
        })?;
        let height = match args
            .get("height")
            .and_then(Value::as_str)
            .map(Stored::from_stored)
        {
            Some(Some(Stored::Length(length))) => length.steps(),
            _ => {
                return Err(fault(
                    "MissingArgument",
                    "height must be a stored length (length:q0:<steps>)",
                ));
            }
        };
        if height <= 0 {
            return Err(fault(
                "NoHeight",
                "the column's top is at or below its base",
            ));
        }
        let material = args
            .get("material")
            .and_then(Value::as_str)
            .ok_or_else(|| fault("MissingArgument", "material must be a material's ID"))?
            .to_owned();
        let bad = || {
            fault(
                "MissingArgument",
                "the ring must be a list of corners [x,y] in grid steps",
            )
        };
        if ring.len() < 3 {
            return Err(fault(
                "EmptyShape",
                "a column's ring has fewer than three corners",
            ));
        }
        let mut corners = Vec::with_capacity(ring.len());
        for corner in ring {
            let place = corner.as_array().ok_or_else(bad)?;
            let (Some(x), Some(y), 2) = (
                place.first().and_then(Value::as_i64),
                place.get(1).and_then(Value::as_i64),
                place.len(),
            ) else {
                return Err(bad());
            };
            if x.unsigned_abs() > GRID_LIMIT.unsigned_abs()
                || y.unsigned_abs() > GRID_LIMIT.unsigned_abs()
            {
                return Err(fault("TooBig", "a corner is beyond the kernel's grid"));
            }
            corners.push([x, y]);
        }
        Ok(ColumnPlan {
            ring: corners,
            height,
            material,
        })
    }

    /// The lowest and highest corners of the ring in plan, in grid steps.
    #[must_use]
    pub fn extent(&self) -> ([i64; 2], [i64; 2]) {
        let mut lo = [i64::MAX; 2];
        let mut hi = [i64::MIN; 2];
        for corner in &self.ring {
            for ((l, h), &c) in lo.iter_mut().zip(hi.iter_mut()).zip(corner) {
                *l = (*l).min(c);
                *h = (*h).max(c);
            }
        }
        (lo, hi)
    }

    /// Whether the column is exactly the box around its ring: four corners, each edge along x or
    /// y.
    #[must_use]
    pub fn is_box(&self) -> bool {
        if self.ring.len() != 4 {
            return false;
        }
        self.ring
            .iter()
            .zip(self.ring.iter().cycle().skip(1))
            .all(|(a, b)| a[0] == b[0] || a[1] == b[1])
    }

    /// The roles of its faces, in order: `bottom`, `top`, and `side.<k>` for each edge of the ring
    /// in order, counting from 0.
    #[must_use]
    pub fn roles(&self) -> Vec<String> {
        let mut roles = vec!["bottom".to_owned(), "top".to_owned()];
        roles.extend((0..self.ring.len()).map(|k| format!("side.{k}")));
        roles
    }
}
