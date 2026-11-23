//! Placed families (CON-A7-F3-P2, piece A64, job 6.4): an element made from a family recipe, its
//! record, where its recipe comes from, and the opening it cuts in the wall it is hosted in. Both
//! builders read a placed family through this module: the kernel builds its solids through the
//! recipe evaluator (`construct_families::Evaluator`) and cuts its opening from its wall; the
//! stand-in draws each solid as its box (`construct_families::extents`).
//!
//! The kind is [`OP`], `family.instance`: the family format calls each placed copy of a family an
//! instance (a parameter's holder is the type or the instance). Its record (construct-model's
//! FORMAT.md, "A placed family"):
//!
//! - the recipe `construct` / [`OP`] with [`FAMILY_ARG`], the family recipe version by its content
//!   id (`sha256:...`, construct-families' [`ContentId`]), and [`TYPE_ARG`], the key of a type the
//!   recipe names (a built-in type or a type table row, such as `d900x2100`);
//! - hosted as a door is ([`crate::hosting`]): one face reference to a side of its wall, and its
//!   offsets `@host.along` (from the wall's start to the family's insertion point, the middle of
//!   its opening) and `@host.up` (from the wall's base to the family's origin: a door's foot, a
//!   window's floor line, under its sill).
//!
//! Derived and never stored: [`TURN_ARG`], how far the family is turned about its z axis to stand
//! in its wall ([`crate::hosting::seat`] writes it into the recipe a builder is handed), its
//! solids, and its opening.
//!
//! Interim, each here so that replacing it changes this file and no caller:
//!
//! - **Where a family's recipe comes from**: the trait [`FamilySource`], given a content id, the
//!   recipe. [`Library`] answers it from recipes passed in by the caller ([`Library::of`],
//!   [`Library::read`]) or from the library folder's door and window templates baked in at build
//!   time ([`Library::baked`], the source both builders use by default, so the browser and the
//!   server build the same families). Job 7.6 (family load and reload, CON-A7-F2-P3) replaces it:
//!   families stored in the document and read from its records, so a project's own families and
//!   their versions are found there.
//! - **Openings are upright rectangles**: an opening outline of any other shape is refused
//!   (`OpeningNotRectangle`), because a wall's openings are cut as boxes along its axis, by the
//!   same rules as the old box door's (the kernel's `opening`). An arched or angled opening needs
//!   its outline extruded through the wall and the rules asked of a polygon.
//! - **Instance values**: evaluation takes the values a type holds; an instance parameter (a
//!   window's sill height, a door's swing) reads its default until the placing commands (job 7.1)
//!   give instances their own values.

use construct_families::{
    ContentId, EvalError, Key, Limits, Recipe as FamilyRecipe, Turn, TypeValues, opening_within,
};
use construct_model::geometry::{GeometryError, Recipe};
use construct_params::Length;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

/// The operation of a placed family: `construct` / `family.instance`.
pub const OP: &str = "family.instance";
/// The recipe argument naming the family recipe version, by its content id.
pub const FAMILY_ARG: &str = "family";
/// The recipe argument naming the type, by its key in the family recipe.
pub const TYPE_ARG: &str = "type";
/// The argument a builder is handed with how far the family is turned (0 to 3 quarter turns,
/// anticlockwise seen from above). Derived from the host, never stored.
pub const TURN_ARG: &str = "turn";
/// The owner of Spare Construct's own operations.
const OWN: &str = "construct";

/// The library folder's door and window templates, baked in at build time.
const BAKED: [&str; 3] = [
    include_str!("../../../library/doors-windows/single-swing-door.json"),
    include_str!("../../../library/doors-windows/fixed-window.json"),
    include_str!("../../../library/doors-windows/sliding-window.json"),
];

fn fault(code: &str, message: impl Into<String>) -> GeometryError {
    GeometryError {
        code: code.to_owned(),
        message: message.into(),
    }
}

/// Where a placed family's recipe comes from: given a recipe version's content id, the recipe.
/// Interim (this module's header): job 7.6 answers it from the document's own stored families.
pub trait FamilySource: Send + Sync {
    /// The recipe whose content id is `family`, if this source holds it.
    fn recipe(&self, family: &ContentId) -> Option<Arc<FamilyRecipe>>;
}

/// Family recipes held in memory, by content id: those a caller passes in, or the library's
/// templates baked in at build time.
#[derive(Clone, Debug, Default)]
pub struct Library {
    recipes: BTreeMap<ContentId, Arc<FamilyRecipe>>,
}

impl Library {
    /// A library of these recipes, each under its content id.
    pub fn of(recipes: impl IntoIterator<Item = FamilyRecipe>) -> Library {
        Library {
            recipes: recipes
                .into_iter()
                .map(|r| (r.content_id(), Arc::new(r)))
                .collect(),
        }
    }

    /// A library of the recipes written in `texts`, family recipe format JSON.
    ///
    /// # Errors
    ///
    /// The reader's words for the first text that is not a recipe, with its place.
    pub fn read<'a>(texts: impl IntoIterator<Item = &'a str>) -> Result<Library, String> {
        let mut recipes = Vec::new();
        for (n, text) in texts.into_iter().enumerate() {
            let recipe = FamilyRecipe::from_json(text)
                .map_err(|e| format!("recipe {n}: {} at {}", e.reason(), e.at()))?;
            recipes.push(recipe);
        }
        Ok(Library::of(recipes))
    }

    /// The library folder's door and window templates (`library/doors-windows`), baked in at
    /// build time, read once.
    #[must_use]
    pub fn baked() -> Arc<Library> {
        static BAKED_LIBRARY: OnceLock<Arc<Library>> = OnceLock::new();
        Arc::clone(BAKED_LIBRARY.get_or_init(|| {
            // The baked templates are this repository's own and are read by its tests; a
            // template that did not read would leave the library without it, never panic.
            Arc::new(Library::of(
                BAKED
                    .iter()
                    .filter_map(|text| FamilyRecipe::from_json(text).ok()),
            ))
        }))
    }

    /// The content ids it holds, in order.
    #[must_use]
    pub fn ids(&self) -> Vec<ContentId> {
        self.recipes.keys().cloned().collect()
    }

    /// The latest version it holds of the family with the permanent id `id`, if it holds one,
    /// and that version's content id: what a placing command names a family by.
    #[must_use]
    pub fn family(&self, id: &str) -> Option<(ContentId, Arc<FamilyRecipe>)> {
        self.recipes
            .iter()
            .find(|(_, r)| r.id.as_str() == id)
            .map(|(content, r)| (content.clone(), Arc::clone(r)))
    }
}

impl FamilySource for Library {
    fn recipe(&self, family: &ContentId) -> Option<Arc<FamilyRecipe>> {
        self.recipes.get(family).cloned()
    }
}

/// The source both builders use when none is given: the baked library ([`Library::baked`]).
#[must_use]
pub fn baked() -> Arc<dyn FamilySource> {
    Library::baked()
}

/// Whether `recipe` is a placed family's.
#[must_use]
pub fn is_instance(recipe: &Recipe) -> bool {
    recipe.owner == OWN && recipe.op == OP
}

/// The recipe a placed family's record stores: the family recipe version and the type.
#[must_use]
pub fn recipe(family: &ContentId, ty: &Key) -> Recipe {
    let mut args = BTreeMap::new();
    args.insert(
        FAMILY_ARG.to_owned(),
        Value::String(family.as_str().to_owned()),
    );
    args.insert(TYPE_ARG.to_owned(), Value::String(ty.as_str().to_owned()));
    Recipe::construct(OP, args)
}

/// `recipe` with the turn a builder is handed ([`TURN_ARG`]).
#[must_use]
pub fn turned(recipe: &Recipe, turn: Turn) -> Recipe {
    let mut turned = recipe.clone();
    turned
        .args
        .insert(TURN_ARG.to_owned(), Value::from(turn.quarters()));
    turned
}

/// A placed family as its recipe says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instance {
    /// The family recipe version.
    pub family: ContentId,
    /// The type, by its key in the family recipe.
    pub ty: Key,
    /// How far it is turned about its z axis: none unless it is seated in a host.
    pub turn: Turn,
}

impl Instance {
    /// The placed family `recipe` says.
    ///
    /// # Errors
    ///
    /// `NotAFamily` for another operation; `BadInstance` for a family that is not a content id, a
    /// type that is not a key, or a turn that is not 0 to 3.
    pub fn of(recipe: &Recipe) -> Result<Instance, GeometryError> {
        if !is_instance(recipe) {
            return Err(fault(
                "NotAFamily",
                format!("a {} is not a placed family", recipe.op),
            ));
        }
        let text = |arg: &str| recipe.args.get(arg).and_then(Value::as_str);
        let family = text(FAMILY_ARG).and_then(ContentId::parse).ok_or_else(|| {
            fault(
                "BadInstance",
                format!("{FAMILY_ARG} must name a family recipe by its content id (sha256:...)"),
            )
        })?;
        let ty = text(TYPE_ARG).and_then(Key::parse).ok_or_else(|| {
            fault(
                "BadInstance",
                format!("{TYPE_ARG} must be the key of one of the family's types"),
            )
        })?;
        let turn = match recipe.args.get(TURN_ARG) {
            None => Turn::None,
            Some(value) => value
                .as_u64()
                .and_then(|n| u8::try_from(n).ok())
                .and_then(Turn::of_quarters)
                .ok_or_else(|| fault("BadInstance", format!("{TURN_ARG} must be 0, 1, 2 or 3")))?,
        };
        Ok(Instance { family, ty, turn })
    }
}

/// A placed family's recipe found, and its type's values read.
#[derive(Clone, Debug)]
pub struct Resolved {
    /// The family recipe.
    pub recipe: Arc<FamilyRecipe>,
    /// Its type's values.
    pub values: TypeValues,
}

/// The refusal of a family's evaluation, under `code`, naming the family.
fn refused(code: &str, recipe: &FamilyRecipe, error: &EvalError) -> GeometryError {
    fault(code, format!("family {}: {error}", recipe.name))
}

/// `instance`'s recipe from `source`, and its type's values.
///
/// # Errors
///
/// `FamilyMissing` when the source holds no recipe of that content id; `FamilyType` when the
/// recipe names no such type, or the type's values do not read.
pub fn resolve(source: &dyn FamilySource, instance: &Instance) -> Result<Resolved, GeometryError> {
    let recipe = source.recipe(&instance.family).ok_or_else(|| {
        fault(
            "FamilyMissing",
            format!(
                "family {}: no family recipe with this content id is loaded",
                instance.family
            ),
        )
    })?;
    let values =
        TypeValues::of(&recipe, &instance.ty).map_err(|e| refused("FamilyType", &recipe, &e))?;
    Ok(Resolved { recipe, values })
}

/// The refusal of `error` from evaluating `recipe`, as a builder's: `FamilyRefused`, naming the
/// family.
#[must_use]
pub fn evaluation_refused(recipe: &FamilyRecipe, error: &EvalError) -> GeometryError {
    refused("FamilyRefused", recipe, error)
}

/// An opening as an upright rectangle in its family's frame: from `x[0]` to `x[1]` along the
/// wall and from `z[0]` to `z[1]` up, construct-params lengths, the lower first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    /// Along the wall, low then high.
    pub x: [Length; 2],
    /// Up, low then high.
    pub z: [Length; 2],
}

/// The placed family `recipe` and the opening it cuts in its wall, solved at its type's values
/// (construct-families' `opening`), as a rectangle in the family's frame.
///
/// # Errors
///
/// [`Instance::of`]'s and [`resolve`]'s; `OpeningRefused` with the evaluation's words (no host,
/// no opening, an outline that is not a loop, a sketch that does not solve); and
/// `OpeningNotRectangle` for an outline that is not an upright rectangle (interim: this module's
/// header).
pub fn opening_of(
    source: &dyn FamilySource,
    recipe: &Recipe,
) -> Result<(Instance, Span), GeometryError> {
    let instance = Instance::of(recipe)?;
    let resolved = resolve(source, &instance)?;
    let solved = opening_within(&resolved.recipe, &resolved.values, Limits::DEFAULT)
        .map_err(|e| refused("OpeningRefused", &resolved.recipe, &e))?;
    let xs: Vec<Length> = solved.outline.iter().map(|p| p[0]).collect();
    let zs: Vec<Length> = solved.outline.iter().map(|p| p[1]).collect();
    let low_high = |v: &[Length]| Some([*v.iter().min()?, *v.iter().max()?]);
    let (Some(x), Some(z)) = (low_high(&xs), low_high(&zs)) else {
        return Err(not_rectangle(&resolved.recipe));
    };
    // An upright rectangle: four corners, each a corner of the box, each side along an axis.
    let upright = solved.outline.len() == 4
        && solved
            .outline
            .iter()
            .all(|p| x.contains(&p[0]) && z.contains(&p[1]))
        && solved.outline.iter().enumerate().all(|(i, a)| {
            let next = i.checked_add(1).and_then(|n| n.checked_rem(4)).unwrap_or(0);
            solved
                .outline
                .get(next)
                .is_some_and(|b| (a[0] == b[0]) != (a[1] == b[1]))
        })
        && x[0] < x[1]
        && z[0] < z[1];
    if !upright {
        return Err(not_rectangle(&resolved.recipe));
    }
    Ok((instance, Span { x, z }))
}

fn not_rectangle(recipe: &FamilyRecipe) -> GeometryError {
    fault(
        "OpeningNotRectangle",
        format!(
            "family {}: its opening is not an upright rectangle, and only those are cut in a \
             wall yet",
            recipe.name
        ),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "test code")]

    use super::*;

    /// The baked library holds the three templates, each under its content id, and finds each
    /// by its permanent id; a placed family's recipe reads back as written, with no turn until it
    /// is seated; a recipe that does not read says which argument.
    #[test]
    fn a_placed_family_names_its_recipe_and_type() {
        let library = Library::baked();
        assert_eq!(library.ids().len(), 3);
        let (id, door) = library
            .family("library.doors-windows.single-swing-door")
            .unwrap();
        assert_eq!(door.content_id(), id);
        let ty = Key::parse("d900x2100").unwrap();
        let placed = recipe(&id, &ty);
        let read = Instance::of(&placed).unwrap();
        assert_eq!(read.family, id);
        assert_eq!(read.ty, ty);
        assert_eq!(read.turn, Turn::None);
        assert_eq!(
            Instance::of(&turned(&placed, Turn::Half)).unwrap().turn,
            Turn::Half
        );
        let mut bad = placed.clone();
        bad.args.insert(TURN_ARG.to_owned(), Value::from(4));
        assert_eq!(Instance::of(&bad).unwrap_err().code, "BadInstance");
        bad.args
            .insert(FAMILY_ARG.to_owned(), Value::from("single-swing-door"));
        let e = Instance::of(&bad).unwrap_err();
        assert!(e.message.starts_with("family must name"), "{}", e.message);
        let mut wall = placed;
        wall.op = "wall.layered".to_owned();
        assert_eq!(Instance::of(&wall).unwrap_err().code, "NotAFamily");
        assert!(Library::read(["{}"]).unwrap_err().starts_with("recipe 0: "));
    }

    /// The single-swing door's opening at 900 by 2100 is the outside of its frame from the floor;
    /// a family the source does not hold, or a type its recipe does not name, is refused saying
    /// which.
    #[test]
    fn a_door_opens_its_wall_to_the_outside_of_its_frame() {
        let library = Library::baked();
        let (id, _) = library
            .family("library.doors-windows.single-swing-door")
            .unwrap();
        let (_, span) =
            opening_of(&*library, &recipe(&id, &Key::parse("d900x2100").unwrap())).unwrap();
        let mm = |v: i32| Length::from_mm(v);
        assert_eq!(span.x, [mm(-450), mm(450)]);
        assert_eq!(span.z, [mm(0), mm(2100)]);
        let none = Library::default();
        let missing = opening_of(&none, &recipe(&id, &Key::parse("d900x2100").unwrap()));
        assert_eq!(missing.unwrap_err().code, "FamilyMissing");
        let ty = opening_of(&*library, &recipe(&id, &Key::parse("d1x1").unwrap()));
        let ty = ty.unwrap_err();
        assert_eq!(ty.code, "FamilyType");
        assert!(
            ty.message.starts_with("family Single swing door: "),
            "{}",
            ty.message
        );
    }
}
