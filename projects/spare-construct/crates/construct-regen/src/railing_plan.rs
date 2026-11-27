//! A railing (`railing`, job 14.2, CON-A4-F5-P3) as both builders read it: its filled recipe's
//! material and its pieces, each a flat profile swept along a vector, in the steps of
//! construct-params' length grid (6,400 to the millimetre), checked for what a builder needs and
//! nothing more.
//!
//! What it reads: the recipe construct-railings' `filled` writes, which the regenerator builds from
//! ([`crate::railings`]): `material`, the material every piece is drawn in, and `pieces`, each
//! `{"id","part","extrude":[[x,y,z], ...],"by":[x,y,z]}`: a post, a baluster, a piece of top rail
//! or of handrail. The railing's base is `z = 0` in its own frame (its base level's elevation plus
//! its offset, the height its built row holds).
//!
//! What it promises: a recipe it reads has at least one piece, every ID once and of the form
//! `post.`, `rail.`, `handrail.` or `baluster.` and its numbers, every part one of `post`,
//! `top-rail`, `handrail` and `baluster`, every point on the kernel's grid (within 2^40 - 1 steps
//! of the origin), every profile at least three corners and every vector not nothing. A recipe
//! whose lines are not laid out yet is `Unfilled`. Where each corner is, and that it is right, is
//! construct-railings' (its `shape`), and the engine's checker judges every solid it returns.
//!
//! Faces' roles ([`RailingPlan::roles`]): one for each piece, its ID.
//!
//! Depends on construct-model (the recipe). Nothing here is interim.

use construct_model::geometry::{GeometryError, Recipe};
use serde_json::Value;
use std::collections::BTreeSet;

/// A railing's recipe operation (construct-railings' `RAILING_OP`).
pub const OP: &str = "railing";
/// The version of a railing's recipe this module reads (construct-railings' `OP_VERSION`).
pub const OP_VERSION: u32 = 1;
/// The owner of Spare Construct's own operations.
const OWN: &str = "construct";
/// The farthest a point may be from the origin along an axis: the kernel's grid, 2^40 - 1 steps.
const GRID_LIMIT: i64 = 1_099_511_627_775;
/// The parts a piece may be, as construct-railings names them.
const PARTS: [&str; 4] = ["post", "top-rail", "handrail", "baluster"];
/// The prefixes of a piece's ID.
const PREFIXES: [&str; 4] = ["post.", "rail.", "handrail.", "baluster."];

fn fault(code: &str, message: impl Into<String>) -> GeometryError {
    GeometryError {
        code: code.to_owned(),
        message: message.into(),
    }
}

fn missing(what: &str) -> GeometryError {
    fault("MissingArgument", format!("a railing's {what}"))
}

/// Whether `recipe` is a railing's, filled or not.
#[must_use]
pub fn is_railing(recipe: &Recipe) -> bool {
    recipe.owner == OWN && recipe.op == OP
}

/// One piece, as the filled recipe lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RailingPiece {
    /// Its ID.
    pub id: String,
    /// What it is part of: `post`, `top-rail`, `handrail` or `baluster`.
    pub part: String,
    /// Its profile's corners, in grid steps.
    pub profile: Vec<[i64; 3]>,
    /// The sweep, in grid steps.
    pub by: [i64; 3],
}

impl RailingPiece {
    /// Its outline in plan, in grid steps: an upright piece's profile, or the four corners a
    /// rail's section sweeps across in plan (its two outermost corners and the same swept).
    #[must_use]
    pub fn plan(&self) -> Vec<[i64; 2]> {
        if self.by[0] == 0 && self.by[1] == 0 {
            return self.profile.iter().map(|p| [p[0], p[1]]).collect();
        }
        // A rail's section stands in x = const or y = const; its extent along the other axis.
        let along_y = self
            .profile
            .iter()
            .all(|p| Some(p[0]) == self.profile.first().map(|f| f[0]));
        let key = |p: &&[i64; 3]| if along_y { p[1] } else { p[0] };
        let (Some(lo), Some(hi)) = (
            self.profile.iter().min_by_key(key),
            self.profile.iter().max_by_key(key),
        ) else {
            return Vec::new();
        };
        let swept = |p: &[i64; 3]| {
            [
                p[0].saturating_add(self.by[0]),
                p[1].saturating_add(self.by[1]),
            ]
        };
        vec![[lo[0], lo[1]], swept(lo), swept(hi), [hi[0], hi[1]]]
    }
}

/// A railing read from its filled recipe ([`RailingPlan::of`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RailingPlan {
    /// The material every piece is drawn in.
    pub material: String,
    /// The pieces, in order.
    pub pieces: Vec<RailingPiece>,
}

fn coordinate(v: &Value) -> Result<i64, GeometryError> {
    let c = v
        .as_i64()
        .ok_or_else(|| missing("points are whole grid steps"))?;
    if c.unsigned_abs() > GRID_LIMIT.unsigned_abs() {
        return Err(fault(
            "TooBig",
            "a railing's point is beyond the kernel's grid",
        ));
    }
    Ok(c)
}

fn point(v: &Value) -> Result<[i64; 3], GeometryError> {
    let list = v
        .as_array()
        .filter(|l| l.len() == 3)
        .ok_or_else(|| missing("points have three coordinates"))?;
    let mut out = [0; 3];
    for (slot, c) in out.iter_mut().zip(list) {
        *slot = coordinate(c)?;
    }
    Ok(out)
}

fn read_piece(v: &Value) -> Result<RailingPiece, GeometryError> {
    let object = v
        .as_object()
        .filter(|o| o.len() == 4)
        .ok_or_else(|| missing("piece is {\"id\",\"part\",\"extrude\",\"by\"}"))?;
    let text = |key: &str| {
        object
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| missing(&format!("piece has its {key}")))
    };
    let id = text("id")?;
    let numbered = PREFIXES.iter().any(|prefix| {
        id.strip_prefix(prefix).is_some_and(|rest| {
            !rest.is_empty()
                && rest
                    .split('.')
                    .all(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        })
    });
    if !numbered {
        return Err(missing(&format!(
            "piece {id:?} is a post, rail, handrail or baluster"
        )));
    }
    let part = text("part")?;
    if !PARTS.contains(&part.as_str()) {
        return Err(missing(&format!(
            "piece {id}'s part {part:?} is one of its four"
        )));
    }
    let profile = object
        .get("extrude")
        .and_then(Value::as_array)
        .ok_or_else(|| missing("piece's profile is a list of points"))?
        .iter()
        .map(point)
        .collect::<Result<Vec<_>, _>>()?;
    if profile.len() < 3 {
        return Err(fault(
            "EmptyShape",
            format!(
                "a railing's piece {id} has a profile of {} corners",
                profile.len()
            ),
        ));
    }
    let by = point(
        object
            .get("by")
            .ok_or_else(|| missing("piece has a vector"))?,
    )?;
    if by == [0, 0, 0] {
        return Err(fault(
            "EmptyShape",
            format!("a railing's piece {id} has no length"),
        ));
    }
    Ok(RailingPiece {
        id,
        part,
        profile,
        by,
    })
}

impl RailingPlan {
    /// Reads a railing's filled recipe.
    ///
    /// # Errors
    ///
    /// `NotARailing` for a recipe of another operation or version; `Unfilled` for one whose lines
    /// are not laid out yet; `MissingArgument` for arguments not as the module header says;
    /// `EmptyShape` for no piece, a profile of fewer than three corners or a piece of no length;
    /// `TooBig` for a point beyond the grid; `DuplicatePiece` for an ID used twice.
    pub fn of(recipe: &Recipe) -> Result<RailingPlan, GeometryError> {
        if !is_railing(recipe) || recipe.op_version != OP_VERSION {
            return Err(fault(
                "NotARailing",
                format!(
                    "a recipe of {}/{} version {} is not a railing's",
                    recipe.owner, recipe.op, recipe.op_version
                ),
            ));
        }
        let args = &recipe.args;
        let pieces = args
            .get("pieces")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                fault(
                    "Unfilled",
                    "its lines are laid out from its path or its stair before it is built",
                )
            })?;
        let material = args
            .get("material")
            .and_then(Value::as_str)
            .ok_or_else(|| missing("material"))?
            .to_owned();
        if args.len() != 2 {
            return Err(missing("filled recipe holds its material and pieces only"));
        }
        let pieces = pieces
            .iter()
            .map(read_piece)
            .collect::<Result<Vec<_>, _>>()?;
        if pieces.is_empty() {
            return Err(fault("EmptyShape", "a railing with no piece"));
        }
        let mut seen = BTreeSet::new();
        for piece in &pieces {
            if !seen.insert(piece.id.as_str()) {
                return Err(fault(
                    "DuplicatePiece",
                    format!("a railing's piece {} is listed twice", piece.id),
                ));
            }
        }
        Ok(RailingPlan { material, pieces })
    }

    /// The roles of its faces: one for each piece, its ID, in order.
    #[must_use]
    pub fn roles(&self) -> Vec<String> {
        self.pieces.iter().map(|p| p.id.clone()).collect()
    }

    /// The lowest and highest grid point any piece reaches.
    #[must_use]
    pub fn extent(&self) -> ([i64; 3], [i64; 3]) {
        let mut lo = [i64::MAX; 3];
        let mut hi = [i64::MIN; 3];
        for piece in &self.pieces {
            for p in &piece.profile {
                let swept = [
                    p[0].saturating_add(piece.by[0]),
                    p[1].saturating_add(piece.by[1]),
                    p[2].saturating_add(piece.by[2]),
                ];
                for q in [*p, swept] {
                    for ((l, h), c) in lo.iter_mut().zip(hi.iter_mut()).zip(q) {
                        *l = (*l).min(c);
                        *h = (*h).max(c);
                    }
                }
            }
        }
        (lo, hi)
    }
}
