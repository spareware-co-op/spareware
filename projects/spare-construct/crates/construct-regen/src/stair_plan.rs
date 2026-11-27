//! A stair (`stair`, job 6.7, CON-A4-F5-P1) as both builders read it: its filled recipe's
//! material and its pieces, each with the solid it is, in the steps of construct-params' length
//! grid (6,400 to the millimetre), checked for what a builder needs and nothing more.
//!
//! What it reads: the recipe construct-stairs' `filled` writes, which the regenerator builds from
//! ([`crate::stairs`]): `material`, the material every piece is drawn in, and `pieces`, from the
//! bottom, a run `{"id":"run.<k>","treads":<n>,"plan":[[x,y], ...],"loft":[[[x,y,z], ...],
//! [[x,y,z], ...]]}` (its side profile at its right-hand side and the same at its left) and a
//! landing `{"id":"landing.<k>","plan":[[x,y], ...],"extrude":[[x,y,z], ...],"by":[x,y,z]}` (its
//! outline at its underside, swept up by its waist). The stair's base is `z = 0` in its own frame
//! (its base level's elevation plus its offset, the height its built row holds).
//!
//! What it promises: a recipe it reads has at least one piece, the first a run, every ID once and
//! of the form `run.<k>` or `landing.<k>`, every point on the kernel's grid (within 2^40 - 1 steps
//! of the origin), every plan four corners, every loft two sections of one count of at least
//! three corners, every extrusion at least three corners and a vector that is not nothing. A
//! recipe whose rise is not read and laid out yet is `Unfilled`. Where each corner is, and that it
//! is right, is construct-stairs' (its `shape`), and the engine's checker judges every solid it
//! returns.
//!
//! Faces' roles ([`StairPlan::roles`]), so a railing (job 14.2) or a floor's edge can name what it
//! meets: a run's `run.<k>.tread.<i>` and `run.<k>.riser.<i>` for each tread from the bottom (its
//! top riser is the landing's or the floor's), `run.<k>.side.left`, `run.<k>.side.right`,
//! `run.<k>.underside` and `run.<k>.end`; a landing's `landing.<k>.top`, `landing.<k>.underside`
//! and `landing.<k>.edge.<e>` for each edge of its outline from its first corner.
//!
//! Depends on construct-model (the recipe). Nothing here is interim.

use construct_model::geometry::{GeometryError, Recipe};
use serde_json::Value;
use std::collections::BTreeSet;

/// A stair's recipe operation (construct-stairs' `STAIR_OP`).
pub const OP: &str = "stair";
/// The version of a stair's recipe this module reads (construct-stairs' `OP_VERSION`).
pub const OP_VERSION: u32 = 1;
/// The owner of Spare Construct's own operations.
const OWN: &str = "construct";
/// The farthest a point may be from the origin along an axis: the kernel's grid, 2^40 - 1 steps.
const GRID_LIMIT: i64 = 1_099_511_627_775;

fn fault(code: &str, message: impl Into<String>) -> GeometryError {
    GeometryError {
        code: code.to_owned(),
        message: message.into(),
    }
}

/// Whether `recipe` is a stair's, filled or not.
#[must_use]
pub fn is_stair(recipe: &Recipe) -> bool {
    recipe.owner == OWN && recipe.op == OP
}

/// The solid of one piece, in grid steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PieceSolid {
    /// A run: its first section lofted to its second, corner to corner.
    Loft {
        /// The two sections.
        sections: [Vec<[i64; 3]>; 2],
    },
    /// A landing: its outline swept by a vector.
    Extrude {
        /// The outline's corners.
        profile: Vec<[i64; 3]>,
        /// The vector.
        by: [i64; 3],
    },
}

/// One piece, as the filled recipe lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StairPiece {
    /// Its ID: `run.<k>` or `landing.<k>`.
    pub id: String,
    /// A run's count of treads; none for a landing.
    pub treads: Option<usize>,
    /// Its outline in plan, anticlockwise from above.
    pub plan: [[i64; 2]; 4],
    /// Its solid.
    pub solid: PieceSolid,
}

/// A stair read from its filled recipe ([`StairPlan::of`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StairPlan {
    /// The material every piece is drawn in.
    pub material: String,
    /// The pieces, from the bottom.
    pub pieces: Vec<StairPiece>,
}

fn missing(what: &str) -> GeometryError {
    fault("MissingArgument", format!("a stair's {what}"))
}

/// A whole number of grid steps within the grid.
fn coordinate(v: &Value) -> Result<i64, GeometryError> {
    let c = v
        .as_i64()
        .ok_or_else(|| missing("points are whole grid steps"))?;
    if c.unsigned_abs() > GRID_LIMIT.unsigned_abs() {
        return Err(fault(
            "TooBig",
            "a stair's point is beyond the kernel's grid",
        ));
    }
    Ok(c)
}

fn point<const N: usize>(v: &Value) -> Result<[i64; N], GeometryError> {
    let list = v
        .as_array()
        .filter(|l| l.len() == N)
        .ok_or_else(|| missing("points have their coordinates"))?;
    let mut out = [0; N];
    for (slot, c) in out.iter_mut().zip(list) {
        *slot = coordinate(c)?;
    }
    Ok(out)
}

fn points(v: Option<&Value>, least: usize) -> Result<Vec<[i64; 3]>, GeometryError> {
    let list = v
        .and_then(Value::as_array)
        .ok_or_else(|| missing("solid is a list of points"))?;
    if list.len() < least {
        return Err(fault(
            "EmptyShape",
            format!("a stair's polygon has {} corners", list.len()),
        ));
    }
    list.iter().map(point::<3>).collect()
}

fn read_piece(v: &Value) -> Result<StairPiece, GeometryError> {
    let object = v.as_object().ok_or_else(|| missing("piece is an object"))?;
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| missing("piece has an ID"))?
        .to_owned();
    let number = |prefix: &str| {
        id.strip_prefix(prefix)
            .is_some_and(|k| !k.is_empty() && k.bytes().all(|b| b.is_ascii_digit()))
    };
    let plan = object
        .get("plan")
        .and_then(Value::as_array)
        .filter(|p| p.len() == 4)
        .ok_or_else(|| missing("piece's plan is four corners"))?;
    let mut corners = [[0; 2]; 4];
    for (slot, c) in corners.iter_mut().zip(plan) {
        *slot = point::<2>(c)?;
    }
    if number("run.") {
        let treads = object
            .get("treads")
            .and_then(Value::as_u64)
            .and_then(|t| usize::try_from(t).ok())
            .filter(|t| *t > 0)
            .ok_or_else(|| missing("run has a count of treads"))?;
        let sections = object
            .get("loft")
            .and_then(Value::as_array)
            .filter(|s| s.len() == 2)
            .ok_or_else(|| missing("run is a loft of two sections"))?;
        let first = points(sections.first(), 3)?;
        let second = points(sections.get(1), 3)?;
        if first.len() != second.len() {
            return Err(missing("run's sections have one count of corners"));
        }
        Ok(StairPiece {
            id,
            treads: Some(treads),
            plan: corners,
            solid: PieceSolid::Loft {
                sections: [first, second],
            },
        })
    } else if number("landing.") {
        let profile = points(object.get("extrude"), 3)?;
        let by = point::<3>(
            object
                .get("by")
                .ok_or_else(|| missing("landing has a vector"))?,
        )?;
        if by == [0, 0, 0] {
            return Err(fault("EmptyShape", "a stair's landing has no thickness"));
        }
        Ok(StairPiece {
            id,
            treads: None,
            plan: corners,
            solid: PieceSolid::Extrude { profile, by },
        })
    } else {
        Err(missing(&format!("piece {id:?} is a run or a landing")))
    }
}

impl StairPlan {
    /// Reads a stair's filled recipe.
    ///
    /// # Errors
    ///
    /// `NotAStair` for a recipe of another operation or version; `Unfilled` for one whose rise is
    /// not read and laid out yet; `MissingArgument` for arguments not as the module header says;
    /// `EmptyShape` for no piece, a polygon of fewer than three corners or a landing of no
    /// thickness; `TooBig` for a point beyond the grid; `DuplicatePiece` for an ID used twice.
    pub fn of(recipe: &Recipe) -> Result<StairPlan, GeometryError> {
        if !is_stair(recipe) || recipe.op_version != OP_VERSION {
            return Err(fault(
                "NotAStair",
                format!(
                    "a recipe of {}/{} version {} is not a stair's",
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
                    "its rise is read from its levels and its layout worked out before it is built",
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
        let first_is_run = pieces.first().is_some_and(|p| p.treads.is_some());
        if !first_is_run {
            return Err(fault("EmptyShape", "a stair starts with a run"));
        }
        let mut seen = BTreeSet::new();
        for piece in &pieces {
            if !seen.insert(piece.id.as_str()) {
                return Err(fault(
                    "DuplicatePiece",
                    format!("a stair's piece {} is listed twice", piece.id),
                ));
            }
        }
        Ok(StairPlan { material, pieces })
    }

    /// The roles of its faces, piece by piece, as the module header lists them.
    #[must_use]
    pub fn roles(&self) -> Vec<String> {
        let mut roles = Vec::new();
        for piece in &self.pieces {
            let id = &piece.id;
            if let Some(treads) = piece.treads {
                for i in 1..=treads {
                    roles.push(format!("{id}.tread.{i}"));
                    roles.push(format!("{id}.riser.{i}"));
                }
                for face in ["side.left", "side.right", "underside", "end"] {
                    roles.push(format!("{id}.{face}"));
                }
            } else {
                roles.push(format!("{id}.top"));
                roles.push(format!("{id}.underside"));
                for e in 0..4 {
                    roles.push(format!("{id}.edge.{e}"));
                }
            }
        }
        roles
    }

    /// The lowest and highest grid point any piece reaches.
    #[must_use]
    pub fn extent(&self) -> ([i64; 3], [i64; 3]) {
        let mut lo = [i64::MAX; 3];
        let mut hi = [i64::MIN; 3];
        for piece in &self.pieces {
            let corners: Vec<[i64; 3]> = match &piece.solid {
                PieceSolid::Loft { sections } => sections.concat(),
                PieceSolid::Extrude { profile, by } => profile
                    .iter()
                    .flat_map(|p| {
                        [
                            *p,
                            [
                                p[0].saturating_add(by[0]),
                                p[1].saturating_add(by[1]),
                                p[2].saturating_add(by[2]),
                            ],
                        ]
                    })
                    .collect(),
            };
            for p in corners {
                for axis in 0..3 {
                    if let (Some(l), Some(h), Some(&c)) =
                        (lo.get_mut(axis), hi.get_mut(axis), p.get(axis))
                    {
                        *l = (*l).min(c);
                        *h = (*h).max(c);
                    }
                }
            }
        }
        (lo, hi)
    }
}
