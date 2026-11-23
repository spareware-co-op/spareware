//! A room (`room`, job 5.4, CON-A4-F7-P1) as both builders read it: its filled recipe's loops and
//! height, in the steps of construct-params' length grid, checked for what a builder needs and
//! nothing more.
//!
//! What it reads: the recipe construct-rooms' `filled` writes, which the regenerator builds from
//! ([`crate::rooms`]): `loops`, the room's outer loop anticlockwise seen from above and then each
//! hole clockwise, each corner `{"at":[x,y],"wall":<the ID of the wall whose face runs from it>}`;
//! and `height`, a construct-params stored length, from the room's base (its level's elevation
//! plus its offset, the height its built row holds) up to its upper limit. The room's volume is
//! the prism on its loops from `z = 0` up to `z = height`.
//!
//! What it promises: a recipe it reads has an outer loop of at least three corners, every corner
//! on the kernel's grid (within 2^40 - 1 steps of the origin), and a height above zero. A room
//! whose loop was not found yet is `Unfilled`. That the loops are simple and nested properly is
//! construct-rooms' search, made when the regenerator fills the recipe, and the engine's checker
//! judges every solid it returns.
//!
//! Depends on construct-model (the recipe) and construct-params (the stored length). Nothing here
//! is interim: every length is exact in construct-params' unit.

use crate::floor_plan::PlanCorner;
use construct_model::geometry::{GeometryError, Recipe};
use construct_params::Value as Stored;
use serde_json::Value;

/// A room's recipe operation (construct-rooms' `ROOM_OP`).
pub const OP: &str = "room";
/// The version of a room's recipe this module reads (construct-rooms' `OP_VERSION`).
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

/// Whether `recipe` is a room's.
#[must_use]
pub fn is_room(recipe: &Recipe) -> bool {
    recipe.owner == OWN && recipe.op == OP
}

/// A room read from its filled recipe ([`RoomPlan::of`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomPlan {
    /// The outer loop, then each hole; each corner's `id` is the wall whose face runs from it.
    pub loops: Vec<Vec<PlanCorner>>,
    /// From its base to its upper limit, in grid steps; above zero.
    pub height: i64,
}

impl RoomPlan {
    /// Reads a room's filled recipe.
    ///
    /// # Errors
    ///
    /// `NotARoom` for a recipe of another operation or version; `Unfilled` for one whose loop is
    /// not found yet; `MissingArgument` for loops or a height not as the module header says;
    /// `EmptyShape` for a loop of fewer than three corners; `NoHeight` for a height at or below
    /// zero; `TooBig` for a corner beyond the grid.
    pub fn of(recipe: &Recipe) -> Result<RoomPlan, GeometryError> {
        if !is_room(recipe) || recipe.op_version != OP_VERSION {
            return Err(fault(
                "NotARoom",
                format!(
                    "a recipe of {}/{} version {} is not a room's",
                    recipe.owner, recipe.op, recipe.op_version
                ),
            ));
        }
        let args = &recipe.args;
        let loops = args.get("loops").and_then(Value::as_array).ok_or_else(|| {
            fault(
                "Unfilled",
                "its loop is found from the walls round it, and was not found before building",
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
                "the room's upper limit is at or below its base",
            ));
        }
        let read = loops
            .iter()
            .enumerate()
            .map(|(at, ring)| read_loop(ring, at))
            .collect::<Result<Vec<_>, _>>()?;
        if read.is_empty() {
            return Err(fault("EmptyShape", "a room has an outer loop"));
        }
        Ok(RoomPlan {
            loops: read,
            height,
        })
    }

    /// The outer loop.
    #[must_use]
    pub fn outer(&self) -> &[PlanCorner] {
        self.loops.first().map_or(&[], Vec::as_slice)
    }

    /// The holes, in order.
    #[must_use]
    pub fn holes(&self) -> &[Vec<PlanCorner>] {
        self.loops.get(1..).unwrap_or_default()
    }

    /// The lowest and highest corners of the outer loop in plan, in grid steps: every hole lies
    /// inside it.
    #[must_use]
    pub fn extent(&self) -> ([i64; 2], [i64; 2]) {
        let mut lo = [i64::MAX; 2];
        let mut hi = [i64::MIN; 2];
        for corner in self.outer() {
            for ((l, h), &c) in lo.iter_mut().zip(hi.iter_mut()).zip(&corner.at) {
                *l = (*l).min(c);
                *h = (*h).max(c);
            }
        }
        (lo, hi)
    }

    /// Whether the room is exactly the box around its outer loop: one loop of four corners, each
    /// edge along x or y, and no hole.
    #[must_use]
    pub fn is_box(&self) -> bool {
        let outer = self.outer();
        if !self.holes().is_empty() || outer.len() != 4 {
            return false;
        }
        outer
            .iter()
            .zip(outer.iter().cycle().skip(1))
            .all(|(a, b)| a.at[0] == b.at[0] || a.at[1] == b.at[1])
    }

    /// The roles of its faces, in order: `bottom`, `top`, and `side.<k>` for each edge of each
    /// loop in loop order, counting from 0.
    #[must_use]
    pub fn roles(&self) -> Vec<String> {
        let mut roles = vec!["bottom".to_owned(), "top".to_owned()];
        let sides = self.loops.iter().map(Vec::len).sum::<usize>();
        roles.extend((0..sides).map(|k| format!("side.{k}")));
        roles
    }
}

/// One loop of the recipe.
fn read_loop(ring: &Value, at: usize) -> Result<Vec<PlanCorner>, GeometryError> {
    let which = if at == 0 {
        "the outer loop".to_owned()
    } else {
        format!("hole {}", at.saturating_sub(1))
    };
    let bad = || {
        fault(
            "MissingArgument",
            format!("{which} must be a list of corners {{\"at\":[x,y],\"wall\"}} in grid steps"),
        )
    };
    let corners = ring.as_array().ok_or_else(bad)?;
    if corners.len() < 3 {
        return Err(fault(
            "EmptyShape",
            format!("{which} has fewer than three corners"),
        ));
    }
    let mut out = Vec::with_capacity(corners.len());
    for corner in corners {
        let object = corner.as_object().ok_or_else(bad)?;
        let wall = object.get("wall").and_then(Value::as_str).ok_or_else(bad)?;
        let place = object.get("at").and_then(Value::as_array).ok_or_else(bad)?;
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
            return Err(fault(
                "TooBig",
                format!("{which}: a corner is beyond the kernel's grid"),
            ));
        }
        out.push(PlanCorner {
            id: wall.to_owned(),
            at: [x, y],
        });
    }
    Ok(out)
}
