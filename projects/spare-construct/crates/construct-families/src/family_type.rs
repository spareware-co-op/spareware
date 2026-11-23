//! A family's types as construct-types types: a built-in type or a type table row of a checked
//! recipe becomes an `ElementType` in the family's category, with every parameter the family
//! has, the values the type sets, and a system block under the owner tag [`OWNER`] naming the family, the recipe
//! version (its content id) and the type. This follows the way wall types carry their layers in
//! construct-types' system slot (`@system.wall`); construct-types stores the block and never
//! reads inside it.

use crate::check::{Library, Problem, definition, stored_value};
use crate::model::{Key, ParameterValue, Recipe};
use crate::read::ReadError;
use crate::write::ContentId;
use construct_params::ParamError;
use construct_types::{ElementType, Parameters, SystemData, TypeError};
use kit_core::Id;
use serde_json::Value;
use std::fmt;

/// The owner tag of a family type's system block: its record's `props` key is
/// `@system.family`.
pub const OWNER: &str = "family";

/// The version of the block's text this crate writes.
const BLOCK_VERSION: u64 = 1;

/// What a family type names: the family, the recipe version and the built-in type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FamilyRef {
    /// The family's permanent id.
    pub family: Id,
    /// The recipe version the type was made from.
    pub recipe: ContentId,
    /// The built-in type's key in that recipe.
    pub type_key: Key,
}

impl FamilyRef {
    /// The block's text: compact JSON, these members in this order, nothing else:
    /// `{"version":1,"family":..,"recipe":..,"type":..}`.
    #[must_use]
    pub fn to_text(&self) -> String {
        let quoted = |s: &str| Value::String(s.to_owned()).to_string();
        format!(
            "{{\"version\":{BLOCK_VERSION},\"family\":{},\"recipe\":{},\"type\":{}}}",
            quoted(self.family.as_str()),
            quoted(self.recipe.as_str()),
            quoted(self.type_key.as_str()),
        )
    }

    /// Reads a block's text, refusing anything [`FamilyRef::to_text`] would not write.
    ///
    /// # Errors
    /// [`ReadError`] if the text is not JSON, has another member, version or spelling, or an
    /// id, content id or key that is not one.
    pub fn from_text(text: &str) -> Result<Self, ReadError> {
        let bad = |why: &str| ReadError::new("", format!("not a family block: {why}"));
        let value: Value = serde_json::from_str(text).map_err(|_| bad("not JSON"))?;
        let member = |name: &str| value.get(name).and_then(Value::as_str);
        let family = member("family")
            .and_then(|s| Id::parse(s).ok())
            .ok_or_else(|| bad("no family id"))?;
        let recipe = member("recipe")
            .and_then(ContentId::parse)
            .ok_or_else(|| bad("no recipe content id"))?;
        let type_key = member("type")
            .and_then(Key::parse)
            .ok_or_else(|| bad("no type key"))?;
        let found = Self {
            family,
            recipe,
            type_key,
        };
        if found.to_text() != text {
            return Err(bad("not in its one spelling"));
        }
        Ok(found)
    }
}

/// Why a family type could not be made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FamilyError {
    /// The recipe does not pass its checks.
    Refused(Vec<Problem>),
    /// The recipe names no type with this key (built-in or a type table row).
    UnknownType(Key),
    /// construct-params refuses a definition or value (only for a recipe that was not checked).
    Param(ParamError),
    /// construct-types refuses the type.
    Type(TypeError),
}

impl From<TypeError> for FamilyError {
    fn from(error: TypeError) -> Self {
        Self::Type(error)
    }
}

impl From<ParamError> for FamilyError {
    fn from(error: ParamError) -> Self {
        Self::Param(error)
    }
}

impl fmt::Display for FamilyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(problems) => {
                write!(f, "the recipe has {} problems", problems.len())
            }
            Self::UnknownType(key) => write!(f, "the recipe has no type {key}"),
            Self::Param(e) => write!(f, "{e}"),
            Self::Type(e) => write!(f, "{e:?}"),
        }
    }
}

impl std::error::Error for FamilyError {}

/// The recipe's parameters as construct-types parameters: each definition, with its default as
/// the value an element reads when nothing sets it (a worked-out parameter has none). A project
/// adds these to its own set; the ids are the definitions' permanent ids.
///
/// # Errors
/// [`FamilyError::Param`] for a definition or default construct-params refuses, and
/// [`FamilyError::Type`] for two parameters with one id.
pub fn parameters(recipe: &Recipe) -> Result<Parameters, FamilyError> {
    let mut params = Parameters::new();
    for row in &recipe.parameters {
        let def = definition(row)?;
        let default = match &row.value {
            ParameterValue::Default(text) => Some(stored_value(&def, text)?),
            ParameterValue::Formula(_) => None,
        };
        params.add(def, default)?;
    }
    Ok(params)
}

/// The construct-types type a type the recipe names makes (a built-in type or a type table row),
/// with permanent id `id`: in the family's category, with every parameter of the family, the
/// values the type sets, and
/// the `family` system block.
///
/// # Errors
/// [`FamilyError::Refused`] if the recipe does not pass [`Recipe::check`];
/// [`FamilyError::UnknownType`] if it has no such type; [`FamilyError::Type`] if construct-types
/// refuses (for one, `params` lacks a parameter of the family).
pub fn family_type(
    recipe: &Recipe,
    library: &Library<'_>,
    type_key: &Key,
    id: Id,
    params: &Parameters,
) -> Result<ElementType, FamilyError> {
    recipe.check(library).map_err(FamilyError::Refused)?;
    let named = recipe
        .named_type(type_key)
        .ok_or_else(|| FamilyError::UnknownType(type_key.clone()))?;
    let mut ty = ElementType::new(id, named.name(), recipe.category, library.categories)?;
    for row in &recipe.parameters {
        ty.add_parameter(params, &row.definition.id)?;
    }
    for (parameter, value) in named.written() {
        let Some(row) = recipe.parameters.iter().find(|p| p.key == *parameter) else {
            continue;
        };
        let def = definition(row)?;
        ty.set(params, &row.definition.id, stored_value(&def, value)?)?;
    }
    let block = FamilyRef {
        family: recipe.id.clone(),
        recipe: recipe.content_id(),
        type_key: type_key.clone(),
    };
    Ok(ty.with_system(SystemData::new(OWNER, &block.to_text())?))
}

/// What a type's `family` system block names: `None` if it has none (it is not a family type),
/// else the block read.
///
/// # Errors
/// The inner [`ReadError`] if the block's text is not one [`FamilyRef::to_text`] writes.
#[must_use]
pub fn family_of(ty: &ElementType) -> Option<Result<FamilyRef, ReadError>> {
    let block = ty.system()?;
    (block.owner() == OWNER).then(|| FamilyRef::from_text(block.data()))
}
