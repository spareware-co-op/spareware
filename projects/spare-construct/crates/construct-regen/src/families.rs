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
//! - each value its instance holds for a parameter each instance holds (a window's sill height, a
//!   door's swing) as a recipe argument named [`VALUE_PREFIX`] and the parameter's key, in stored
//!   text (`value.sill_height`: `length:q0:5760000`); one the instance does not set reads its
//!   parameter's default. They are flat strings so that a recipe with them reads no record.
//!
//! - [`FAMILY_ID_ARG`], the family's permanent id, which is also the element id of the family's
//!   record in the document (job 7.6, below). A record written before job 7.6 has none.
//!
//! Derived and never stored: [`TURN_ARG`], how far the family is turned about its z axis to stand
//! in a wall along x or y, or [`crate::seating::SEAT_ARG`], where it stands in a wall at any angle
//! (piece A83; [`crate::hosting::seat`] writes one or the other into the recipe a builder is
//! handed), its solids, and its opening; and [`RECIPE_ARG`], the family's recipe text from the
//! document's record, which the recipe a builder is handed carries ([`filled`]), so a builder needs
//! no library to build a family the document holds.
//!
//! **A family in the document** (CON-A7-F2-P3, piece A73, job 7.6): a record of kind
//! [`RECORD_KIND`] whose element id is the family's permanent id, holding construct-families'
//! [`StoredFamily`] (the recipe in its one written form under [`RECIPE_KEY`], its content id under
//! [`CONTENT_KEY`], where it came from under [`SOURCE_KEY`]; [`record`] writes it, [`read_record`]
//! reads it strictly). It has no shape. A placed family that names one ([`FAMILY_ID_ARG`]) is
//! built from it, its `reads` row naming the record, so the document opens and builds the same on
//! a server without the library. A placed family written before job 7.6 names none: its recipe is
//! found by its content id in the builder's [`FamilySource`] as before (the library baked in), a
//! fallback kept so that old documents read and build unchanged; a reload or a swap of it through
//! the log moves it onto a family in the document.
//!
//! Interim, each here so that replacing it changes this file and no caller:
//!
//! - **Where an old placed family's recipe comes from**: the trait [`FamilySource`], given a
//!   content id, the recipe. [`Library`] answers it from recipes passed in by the caller
//!   ([`Library::of`], [`Library::read`]) or from the library folder's door and window templates
//!   baked in at build time ([`Library::baked`], the source both builders use by default, and
//!   what `family.load` loads by name). The project library job (CON-A7-F9) replaces the baked
//!   templates with a library a project keeps.
//! - **Openings are upright rectangles**: an opening outline of any other shape is refused
//!   (`OpeningNotRectangle`), because a wall's openings are cut as boxes along its axis, by the
//!   same rules as the old box door's (the kernel's `opening`). An arched or angled opening needs
//!   its outline extruded through the wall and the rules asked of a polygon.

use construct_families::{
    ContentId, EvalError, Key, Limits, Recipe as FamilyRecipe, Seat, StoredFamily, Turn, TypeValues,
    opening_within,
};
use construct_model::element::{Element, Placement};
use construct_model::geometry::{GeometryError, Recipe};
use construct_params::Length;
use kit_core::Id;
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
/// The recipe arguments that hold an instance's own values start with this, then the parameter's
/// key: `value.sill_height`.
pub const VALUE_PREFIX: &str = "value.";
/// The recipe argument naming the family in the document: its permanent id, the element id of its
/// record.
pub const FAMILY_ID_ARG: &str = "family_id";
/// The argument a builder is handed with the family's recipe text, read from the document's
/// record. Filled, never stored.
pub const RECIPE_ARG: &str = "recipe";
/// The kind of a family's record in a document, and its recipe's operation.
pub const RECORD_KIND: &str = "family.recipe";
/// The `props` key of a family record's recipe, in its one written form.
pub const RECIPE_KEY: &str = "@family.recipe";
/// The `props` key of a family record's content id.
pub const CONTENT_KEY: &str = "@family.content";
/// The `props` key of a family record's source (construct-families' `Source`, written).
pub const SOURCE_KEY: &str = "@family.source";
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

    /// Every recipe it holds, by content id, in order.
    pub fn recipes(&self) -> impl Iterator<Item = (&ContentId, &Arc<FamilyRecipe>)> {
        self.recipes.iter()
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

/// `recipe` with the value `stored` for the instance parameter `key` (an empty text takes the
/// value away, so the parameter reads its default again).
#[must_use]
pub fn with_value(recipe: &Recipe, key: &Key, stored: &str) -> Recipe {
    let mut changed = recipe.clone();
    let name = format!("{VALUE_PREFIX}{}", key.as_str());
    if stored.is_empty() {
        changed.args.remove(&name);
    } else {
        changed.args.insert(name, Value::String(stored.to_owned()));
    }
    changed
}

/// The family in the document a placed family's recipe names, by its permanent id; `None` for one
/// written before families were held in documents, or a recipe that is not a placed family's.
#[must_use]
pub fn in_document(recipe: &Recipe) -> Option<Id> {
    if !is_instance(recipe) {
        return None;
    }
    recipe
        .args
        .get(FAMILY_ID_ARG)
        .and_then(Value::as_str)
        .and_then(|id| Id::parse(id).ok())
}

/// `recipe` made a placed copy of `family`, a family in the document: that version and that
/// family named, its type and values kept.
#[must_use]
pub fn of_family(recipe: &Recipe, family: &StoredFamily) -> Recipe {
    let mut changed = recipe.clone();
    changed.args.insert(
        FAMILY_ARG.to_owned(),
        Value::String(family.content.as_str().to_owned()),
    );
    changed.args.insert(
        FAMILY_ID_ARG.to_owned(),
        Value::String(family.recipe.id.to_string()),
    );
    changed
}

/// The recipe a builder is handed for a placed family of `family`, the document's record of the
/// family its recipe names: `recipe` with the family's recipe text ([`RECIPE_ARG`]).
///
/// # Errors
/// `FamilyMissing` when the record holds another version than the one `recipe` names.
pub fn filled(recipe: &Recipe, family: &StoredFamily) -> Result<Recipe, GeometryError> {
    let names = recipe.args.get(FAMILY_ARG).and_then(Value::as_str);
    if names != Some(family.content.as_str()) {
        return Err(fault(
            "FamilyMissing",
            format!(
                "family {}: the document holds version {}, not the version this copy names",
                family.recipe.name, family.content
            ),
        ));
    }
    let mut out = recipe.clone();
    out.args
        .insert(RECIPE_ARG.to_owned(), Value::String(family.text()));
    Ok(out)
}

/// The recipe of a family's record: `construct` / [`RECORD_KIND`], no arguments.
fn record_recipe() -> Recipe {
    Recipe::construct(RECORD_KIND, BTreeMap::new())
}

/// A document's record of `family`: kind [`RECORD_KIND`], element id the family's permanent id,
/// at level 0 and placement `{0,0,0}` with no references, and its recipe, content id and source
/// under [`RECIPE_KEY`], [`CONTENT_KEY`] and [`SOURCE_KEY`].
#[must_use]
pub fn record(document: &Id, family: &StoredFamily) -> Element {
    let mut props = BTreeMap::new();
    props.insert(RECIPE_KEY.to_owned(), family.text());
    props.insert(CONTENT_KEY.to_owned(), family.content.as_str().to_owned());
    props.insert(SOURCE_KEY.to_owned(), family.source.write());
    Element {
        id: family.recipe.id.clone(),
        document: document.clone(),
        kind: RECORD_KIND.to_owned(),
        name: family.recipe.name.clone(),
        level: 0,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe: record_recipe(),
        props,
        references: Vec::new(),
    }
}

/// Whether `element` is a family's record.
#[must_use]
pub fn is_record(element: &Element) -> bool {
    element.kind == RECORD_KIND
}

/// The family a document's record holds. Strict: anything [`record`] would not write is refused.
///
/// # Errors
/// Why not, in words, naming the record.
pub fn read_record(element: &Element) -> Result<StoredFamily, String> {
    let named = |why: String| format!("the family record {}: {why}", element.id);
    let text = |key: &str| {
        element
            .props
            .get(key)
            .ok_or_else(|| named(format!("it holds no {key}")))
    };
    let family = StoredFamily::read(text(RECIPE_KEY)?, text(CONTENT_KEY)?, text(SOURCE_KEY)?)
        .map_err(named)?;
    if record(&element.document, &family) != *element {
        return Err(named("it is not as it is written".to_owned()));
    }
    Ok(family)
}

/// `recipe` with another type.
#[must_use]
pub fn with_type(recipe: &Recipe, ty: &Key) -> Recipe {
    let mut changed = recipe.clone();
    changed
        .args
        .insert(TYPE_ARG.to_owned(), Value::String(ty.as_str().to_owned()));
    changed
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
    /// How far it is turned about its z axis: none unless it is seated in a host along x or y.
    pub turn: Turn,
    /// Where it stands in a host at an angle ([`crate::seating::SEAT_ARG`]), in place of a turn.
    pub seat: Option<Seat>,
    /// The values it holds for the parameters each instance holds, in stored text, by key.
    pub values: BTreeMap<Key, String>,
    /// The family in the document it names, by its permanent id ([`FAMILY_ID_ARG`]).
    pub in_document: Option<Id>,
    /// The family's recipe as the document holds it, when the recipe it was read from carries it
    /// ([`RECIPE_ARG`], filled before a build); its content id is [`Instance::family`].
    pub own: Option<Arc<FamilyRecipe>>,
}

impl Instance {
    /// The placed family `recipe` says.
    ///
    /// # Errors
    ///
    /// `NotAFamily` for another operation; `BadInstance` for a family that is not a content id, a
    /// type that is not a key, a turn that is not 0 to 3, or both a turn and a seat; `BadSeat` for
    /// a seat that does not read ([`crate::seating::seat_of`]).
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
        let in_document = match recipe.args.get(FAMILY_ID_ARG) {
            None => None,
            Some(value) => Some(
                value
                    .as_str()
                    .and_then(|id| Id::parse(id).ok())
                    .ok_or_else(|| {
                        fault(
                            "BadInstance",
                            format!("{FAMILY_ID_ARG} must be a family's permanent id"),
                        )
                    })?,
            ),
        };
        let own = match recipe.args.get(RECIPE_ARG) {
            None => None,
            Some(value) => {
                let read = value
                    .as_str()
                    .and_then(|text| FamilyRecipe::from_json(text).ok())
                    .filter(|own| own.content_id() == family)
                    .ok_or_else(|| {
                        fault(
                            "BadInstance",
                            format!(
                                "{RECIPE_ARG} must be the recipe of the version {FAMILY_ARG} names"
                            ),
                        )
                    })?;
                Some(Arc::new(read))
            }
        };
        let seat = crate::seating::seat_of(recipe)?;
        if seat.is_some() && recipe.args.contains_key(TURN_ARG) {
            return Err(fault(
                "BadInstance",
                format!(
                    "a placed family is turned ({TURN_ARG}) or seated ({}), not both",
                    crate::seating::SEAT_ARG
                ),
            ));
        }
        let mut values = BTreeMap::new();
        for (name, value) in &recipe.args {
            let Some(key) = name.strip_prefix(VALUE_PREFIX) else {
                continue;
            };
            let key = Key::parse(key).ok_or_else(|| {
                fault(
                    "BadInstance",
                    format!("{name} must name one of the family's parameters"),
                )
            })?;
            let stored = value.as_str().ok_or_else(|| {
                fault(
                    "BadInstance",
                    format!("{name} must be a value in stored text"),
                )
            })?;
            values.insert(key, stored.to_owned());
        }
        Ok(Instance {
            family,
            ty,
            turn,
            seat,
            values,
            in_document,
            own,
        })
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

/// `instance`'s recipe, and its type's values: the document's, when the recipe it was read from
/// carries it ([`Instance::own`]); else, for a placed family written before families were held
/// in documents, from `source` by its content id.
///
/// # Errors
///
/// `FamilyMissing` when the family is in the document but its recipe was not filled in, or the
/// source holds no recipe of that content id; `FamilyType` when the recipe names no such type,
/// or the type's values do not read.
pub fn resolve(source: &dyn FamilySource, instance: &Instance) -> Result<Resolved, GeometryError> {
    let recipe = if let Some(own) = &instance.own {
        Arc::clone(own)
    } else if let Some(id) = &instance.in_document {
        return Err(fault(
            "FamilyMissing",
            format!("family {id}: its recipe was not read from the document"),
        ));
    } else {
        source.recipe(&instance.family).ok_or_else(|| {
            fault(
                "FamilyMissing",
                format!(
                    "family {}: no family recipe with this content id is loaded",
                    instance.family
                ),
            )
        })?
    };
    let mut values =
        TypeValues::of(&recipe, &instance.ty).map_err(|e| refused("FamilyType", &recipe, &e))?;
    for (key, stored) in &instance.values {
        values.set_instance(key.clone(), stored.clone());
    }
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

    /// An instance's own values ride in the recipe as flat arguments, read back by key; a window's
    /// opening rises with its sill height, a value of another kind or key is refused.
    #[test]
    fn an_instance_holds_its_own_values() {
        let library = Library::baked();
        let (id, _) = library
            .family("library.doors-windows.fixed-window")
            .unwrap();
        let ty = Key::parse("w1200x1200").unwrap();
        let sill = Key::parse("sill_height").unwrap();
        let plain = recipe(&id, &ty);
        let raised = with_value(&plain, &sill, "length:q0:7040000");
        assert_eq!(Instance::of(&plain).unwrap().values.len(), 0);
        let read = Instance::of(&raised).unwrap();
        assert_eq!(
            read.values.get(&sill).map(String::as_str),
            Some("length:q0:7040000")
        );
        let (_, low) = opening_of(&*library, &plain).unwrap();
        let (_, high) = opening_of(&*library, &raised).unwrap();
        assert_eq!(low.z, [Length::from_mm(900), Length::from_mm(2100)]);
        assert_eq!(high.z, [Length::from_mm(1100), Length::from_mm(2300)]);
        assert_eq!(with_value(&raised, &sill, ""), plain);
        let bad = with_value(&plain, &Key::parse("width").unwrap(), "length:q0:6400000");
        assert_eq!(
            opening_of(&*library, &bad).unwrap_err().code,
            "OpeningRefused"
        );
        let mut odd = plain.clone();
        odd.args.insert("value.Sill".to_owned(), Value::from("x"));
        assert_eq!(Instance::of(&odd).unwrap_err().code, "BadInstance");
        odd.args
            .insert("value.sill_height".to_owned(), Value::from(5));
        odd.args.remove("value.Sill");
        assert_eq!(Instance::of(&odd).unwrap_err().code, "BadInstance");
        assert_eq!(
            with_type(&plain, &Key::parse("w600x600").unwrap()).args["type"],
            "w600x600"
        );
    }

    /// A copy of a family the document holds is built from the recipe its record fills in, with
    /// no library; a copy written before families were held in documents reads the library by its
    /// content id; a copy whose recipe was not filled in, or names another version, is refused;
    /// the family's record reads back strictly.
    #[test]
    fn a_copy_of_a_documents_family_needs_no_library() {
        let library = Library::baked();
        let (old_id, window) = library
            .family("library.doors-windows.fixed-window")
            .unwrap();
        let mut own = (*window).clone();
        own.id = Id::parse("office.window").unwrap();
        let stored = StoredFamily::new(own, construct_families::Source::Text);
        let ty = Key::parse("w1200x1200").unwrap();
        let placed = of_family(&recipe(&stored.content, &ty), &stored);
        assert_eq!(
            in_document(&placed),
            Some(Id::parse("office.window").unwrap())
        );
        let none = Library::default();
        assert_eq!(
            opening_of(&none, &placed).unwrap_err().code,
            "FamilyMissing"
        );
        let built = filled(&placed, &stored).unwrap();
        let (_, span) = opening_of(&none, &built).unwrap();
        assert_eq!(span.z, [Length::from_mm(900), Length::from_mm(2100)]);
        let other = StoredFamily::new((*window).clone(), construct_families::Source::Text);
        assert_eq!(filled(&placed, &other).unwrap_err().code, "FamilyMissing");
        let mut tampered = built.clone();
        tampered
            .args
            .insert(RECIPE_ARG.to_owned(), Value::from(window.to_json()));
        assert_eq!(Instance::of(&tampered).unwrap_err().code, "BadInstance");
        // an old copy names no family in the document and is read from the library
        let old = recipe(&old_id, &ty);
        assert_eq!(in_document(&old), None);
        assert!(opening_of(&*library, &old).is_ok());
        // the record
        let document = Id::parse("doc").unwrap();
        let written = record(&document, &stored);
        assert!(is_record(&written));
        assert_eq!(written.id.as_str(), "office.window");
        assert_eq!(read_record(&written).unwrap(), stored);
        let mut renamed = written.clone();
        renamed.name = "Window".to_owned();
        let e = read_record(&renamed).unwrap_err();
        assert!(e.ends_with("it is not as it is written"), "{e}");
        let mut emptied = written;
        emptied.props.remove(CONTENT_KEY);
        let e = read_record(&emptied).unwrap_err();
        assert_eq!(
            e,
            "the family record office.window: it holds no @family.content"
        );
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
