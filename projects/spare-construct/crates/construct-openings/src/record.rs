//! Openings and shafts as element records (construct-model's, unchanged) and their recipes, and the
//! filled recipes the builders read. FORMAT.md beside this crate is the stored form.
//!
//! - A wall opening: kind [`OPENING_KIND`], recipe [`WALL_OP`] with its width and height as stored
//!   lengths and the plan axis its wall runs along. It is hosted on a side of its wall as a door
//!   is (construct-regen's `hosting`: one face reference and its offsets along and up the face,
//!   its position along the wall and its sill height), so it moves with its wall; its opening goes
//!   through the wall's whole thickness.
//! - A floor opening: kind [`OPENING_KIND`], recipe [`FLOOR_OP`] with its outline in its floor's
//!   own frame (the frame the floor's boundary is drawn in). It is hosted on its floor's top face
//!   by one face reference and no offsets (construct-regen's `hosting`, written there), so it
//!   moves with its floor.
//! - A shaft: kind [`SHAFT_KIND`], recipe [`SHAFT_OP`] with its outline (from its placement) and a
//!   height read from its top tie (`{"tie":"top"}`, filled by construct-regen's `levels` as a
//!   stored length), its base and top tied to levels by construct-levels' ties in its props. It
//!   is hosted on nothing: it cuts every floor it passes through ([`crate::fit::reach`]).
//!
//! Each record's placement is the origin when made (a shaft's place in plan is its outline's),
//! `level` 0, and its props hold its category ([`construct_categories::OPENINGS`]).

use crate::error::{Named, OpeningError};
use crate::outline::Outline;
use construct_categories::OPENINGS;
use construct_levels::{Tie, write_base, write_top};
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_params::{Length, Value as Stored};
use construct_types::CATEGORY_KEY;
use kit_core::Id;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// The element kind of an opening through a wall or a floor.
pub const OPENING_KIND: &str = "opening";
/// The element kind of a shaft.
pub const SHAFT_KIND: &str = "shaft";
/// The recipe operation of an opening through a wall.
pub const WALL_OP: &str = "opening.wall";
/// The recipe operation of an opening through a floor.
pub const FLOOR_OP: &str = "opening.floor";
/// The recipe operation of a shaft.
pub const SHAFT_OP: &str = "shaft";
/// The version of each recipe this crate writes.
pub const OP_VERSION: u32 = 1;
/// The argument holding a wall opening's width, a stored length.
pub const WIDTH_ARG: &str = "width";
/// The argument holding a wall opening's height, or a shaft's (filled), a stored length.
pub const HEIGHT_ARG: &str = "height";
/// The argument naming the plan axis a wall opening's wall runs along: `x` or `y`.
pub const AXIS_ARG: &str = "axis";
/// The argument holding a floor opening's or a shaft's outline ([`Outline::to_value`]).
pub const OUTLINE_ARG: &str = "outline";
/// The owner of Spare Construct's own operations.
const OWN: &str = "construct";

/// Whether `recipe` is of `op`, this crate's version.
fn is_op(recipe: &Recipe, op: &str) -> bool {
    recipe.owner == OWN && recipe.op == op && recipe.op_version == OP_VERSION
}

/// Whether a recipe is a wall opening's.
#[must_use]
pub fn is_wall_opening(recipe: &Recipe) -> bool {
    is_op(recipe, WALL_OP)
}

/// Whether a recipe is a floor opening's.
#[must_use]
pub fn is_floor_opening(recipe: &Recipe) -> bool {
    is_op(recipe, FLOOR_OP)
}

/// Whether a recipe is a shaft's.
#[must_use]
pub fn is_shaft(recipe: &Recipe) -> bool {
    is_op(recipe, SHAFT_OP)
}

/// Whether a recipe is any of the three, of any version: a void, which has no solid of its own.
#[must_use]
pub fn is_void(recipe: &Recipe) -> bool {
    recipe.owner == OWN && [WALL_OP, FLOOR_OP, SHAFT_OP].contains(&recipe.op.as_str())
}

/// Whether a record is a shaft.
#[must_use]
pub fn is_shaft_record(record: &Element) -> bool {
    record.kind == SHAFT_KIND
}

/// What a refusal of an element with `recipe` and permanent ID `key` names it as.
#[must_use]
pub fn named(key: &str, recipe: &Recipe) -> Named {
    if recipe.op == SHAFT_OP {
        Named::shaft(key)
    } else {
        Named::opening(key)
    }
}

fn not_this(element: &Named, recipe: &Recipe) -> OpeningError {
    OpeningError::NotAnOpening {
        element: element.clone(),
        op: format!("{}/{} v{}", recipe.owner, recipe.op, recipe.op_version),
    }
}

/// A stored length argument of `recipe`.
fn length_arg(element: &Named, recipe: &Recipe, arg: &str) -> Result<Length, OpeningError> {
    match recipe
        .args
        .get(arg)
        .and_then(Value::as_str)
        .and_then(Stored::from_stored)
    {
        Some(Stored::Length(length)) => Ok(length),
        _ => Err(OpeningError::Malformed {
            element: element.clone(),
            why: format!("its {arg} is not a stored length"),
        }),
    }
}

fn stored(length: Length) -> Value {
    Value::from(Stored::Length(length).stored())
}

/// A rectangular opening through a wall: its width along the wall and its height, and the plan
/// axis its wall runs along. Where it sits (along the wall, its sill height) is its hosting's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WallOpening {
    /// Its width along its wall.
    pub width: Length,
    /// Its height, from its sill to its head.
    pub height: Length,
    /// Whether its wall runs along y (else along x).
    pub along_y: bool,
}

impl WallOpening {
    /// A wall opening of `width` by `height`, for `element`.
    ///
    /// # Errors
    /// [`OpeningError::NoSize`] for a width or a height of zero or less.
    pub fn new(
        element: &Named,
        width: Length,
        height: Length,
        along_y: bool,
    ) -> Result<Self, OpeningError> {
        for (what, length) in [("width", width), ("height", height)] {
            if length.steps() <= 0 {
                return Err(OpeningError::NoSize {
                    element: element.clone(),
                    what,
                });
            }
        }
        Ok(Self {
            width,
            height,
            along_y,
        })
    }

    /// Its recipe.
    #[must_use]
    pub fn recipe(&self) -> Recipe {
        let mut args = BTreeMap::new();
        args.insert(WIDTH_ARG.to_owned(), stored(self.width));
        args.insert(HEIGHT_ARG.to_owned(), stored(self.height));
        args.insert(
            AXIS_ARG.to_owned(),
            Value::from(if self.along_y { "y" } else { "x" }),
        );
        let mut recipe = Recipe::construct(WALL_OP, args);
        recipe.op_version = OP_VERSION;
        recipe
    }

    /// The wall opening `recipe` makes, read strictly, for the element `key`.
    ///
    /// # Errors
    /// [`OpeningError::NotAnOpening`] for a recipe of another operation or version;
    /// [`OpeningError::Malformed`] for one not of the stored form; [`OpeningError::NoSize`].
    pub fn of(key: &str, recipe: &Recipe) -> Result<Self, OpeningError> {
        let element = Named::opening(key);
        if !is_wall_opening(recipe) {
            return Err(not_this(&element, recipe));
        }
        let along_y = match recipe.args.get(AXIS_ARG).and_then(Value::as_str) {
            Some("x") => false,
            Some("y") => true,
            _ => {
                return Err(OpeningError::Malformed {
                    element,
                    why: "its axis is x or y".to_owned(),
                });
            }
        };
        if recipe.args.len() != 3 {
            return Err(OpeningError::Malformed {
                element,
                why: "its recipe holds its width, height and axis, nothing else".to_owned(),
            });
        }
        let width = length_arg(&element, recipe, WIDTH_ARG)?;
        let height = length_arg(&element, recipe, HEIGHT_ARG)?;
        Self::new(&element, width, height, along_y)
    }
}

/// A new record of kind `kind` with `recipe`, in the openings category.
fn record(id: &Id, document: &Id, name: &str, kind: &str, recipe: Recipe) -> Element {
    let mut props = BTreeMap::new();
    props.insert(CATEGORY_KEY.to_owned(), OPENINGS.0.to_string());
    Element {
        id: id.clone(),
        document: document.clone(),
        kind: kind.to_owned(),
        name: name.to_owned(),
        level: 0,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe,
        props,
        references: Vec::new(),
    }
}

/// A new wall opening's record, not yet hosted: its hosting (its wall's face, along and up) is
/// written by construct-regen's `hosting::write`, as a door's is.
#[must_use]
pub fn wall_opening_record(id: &Id, document: &Id, name: &str, opening: &WallOpening) -> Element {
    record(id, document, name, OPENING_KIND, opening.recipe())
}

/// A floor opening's recipe.
#[must_use]
pub fn floor_opening_recipe(outline: &Outline) -> Recipe {
    let mut args = BTreeMap::new();
    args.insert(OUTLINE_ARG.to_owned(), outline.to_value());
    let mut recipe = Recipe::construct(FLOOR_OP, args);
    recipe.op_version = OP_VERSION;
    recipe
}

/// A new floor opening's record, not yet hosted: its floor's top face is written by
/// construct-regen's `hosting::write_in_plan`.
#[must_use]
pub fn floor_opening_record(id: &Id, document: &Id, name: &str, outline: &Outline) -> Element {
    record(
        id,
        document,
        name,
        OPENING_KIND,
        floor_opening_recipe(outline),
    )
}

/// The argument a shaft's recipe reads its height with: `{"tie":"top"}`, the form
/// construct-regen's `levels` fills (a test there pins the two equal).
#[must_use]
pub fn top_arg() -> Value {
    json!({ "tie": "top" })
}

/// A shaft's recipe: its outline, and its height read from its top tie.
#[must_use]
pub fn shaft_recipe(outline: &Outline) -> Recipe {
    let mut args = BTreeMap::new();
    args.insert(OUTLINE_ARG.to_owned(), outline.to_value());
    args.insert(HEIGHT_ARG.to_owned(), top_arg());
    let mut recipe = Recipe::construct(SHAFT_OP, args);
    recipe.op_version = OP_VERSION;
    recipe
}

/// A new shaft's record: `outline` in plan, its base on `base` and its top on `top`.
#[must_use]
pub fn shaft_record(
    id: &Id,
    document: &Id,
    name: &str,
    outline: &Outline,
    base: &Tie,
    top: &Tie,
) -> Element {
    let mut made = record(id, document, name, SHAFT_KIND, shaft_recipe(outline));
    write_base(&mut made.props, Some(base));
    write_top(&mut made.props, Some(top));
    made
}

/// The outline a floor opening's or a shaft's recipe holds, checked, for the element `key`.
///
/// # Errors
/// [`OpeningError::NotAnOpening`] for a recipe of neither; [`OpeningError::Malformed`] for one
/// with no outline; any refusal of [`Outline::from_value`].
pub fn outline_of(key: &str, recipe: &Recipe) -> Result<Outline, OpeningError> {
    let element = named(key, recipe);
    if !is_floor_opening(recipe) && !is_shaft(recipe) {
        return Err(not_this(&element, recipe));
    }
    let value = recipe
        .args
        .get(OUTLINE_ARG)
        .ok_or_else(|| OpeningError::Malformed {
            element: element.clone(),
            why: "its recipe holds no outline".to_owned(),
        })?;
    Outline::from_value(&element, value)
}

/// A shaft as its filled recipe builds it: its outline, and its height from its base to its top.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shaft {
    /// Its outline in plan, from its placement.
    pub outline: Outline,
    /// From its base up to its top: always above 0.
    pub height: Length,
}

impl Shaft {
    /// The shaft a filled recipe (its height a stored length) makes, for the element `key`.
    ///
    /// # Errors
    /// [`OpeningError::TopBelowBase`] and [`OpeningError::NoHeight`]; any refusal of
    /// [`outline_of`]; [`OpeningError::Malformed`] for a height not filled yet.
    pub fn of(key: &str, recipe: &Recipe) -> Result<Self, OpeningError> {
        let element = Named::shaft(key);
        if !is_shaft(recipe) {
            return Err(not_this(&element, recipe));
        }
        let outline = outline_of(key, recipe)?;
        let height = length_arg(&element, recipe, HEIGHT_ARG)?;
        match height.steps().cmp(&0) {
            std::cmp::Ordering::Less => Err(OpeningError::TopBelowBase { element }),
            std::cmp::Ordering::Equal => Err(OpeningError::NoHeight { element }),
            std::cmp::Ordering::Greater => Ok(Self { outline, height }),
        }
    }
}
