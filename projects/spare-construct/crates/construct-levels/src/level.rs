//! One level: its permanent ID, name, elevation and storey flag, written as an element record of
//! the levels category and read back. How an elevation is typed and shown.

use crate::error::LevelError;
use construct_categories::LEVELS;
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_params::{Definition, Holder, Kind, Length, Units, Value};
use kit_core::Id;
use std::collections::BTreeMap;

/// The `kind` of every level record, and its recipe's operation.
pub const LEVEL_KIND: &str = "level";
/// The record's category, as every record carries it (construct-types' key).
pub const CATEGORY_KEY: &str = "@category";
/// The level's elevation above the project's zero, in construct-params' stored length text.
pub const ELEVATION_KEY: &str = "@elevation";
/// Whether the level is a building storey, in construct-params' stored yes or no text.
pub const STOREY_KEY: &str = "@storey";

/// Where a level record sits: a datum has no shape, so nowhere.
const NO_PLACEMENT: Placement = Placement { x: 0, y: 0, z: 0 };
/// The ID of the stand-in definition typed lengths are read and shown with. It is never stored.
const LENGTH_TEXT: &str = "elevation";

/// The recipe of a level record: Spare Construct's own operation `level`, version 1, with no
/// arguments. It builds no shape. Its version is the level record's format version.
#[must_use]
pub fn level_recipe() -> Recipe {
    Recipe::construct(LEVEL_KIND, BTreeMap::new())
}

/// A level name as kept: trimmed, and never empty.
pub(crate) fn clean_name(name: &str) -> Result<String, LevelError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(LevelError::EmptyName);
    }
    Ok(name.to_owned())
}

/// A named horizontal datum of a document. Its name is kept unique in the document by
/// [`crate::Levels`], which alone renames, moves or flags a level of a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Level {
    id: Id,
    name: String,
    elevation: Length,
    storey: bool,
}

impl Level {
    /// A level. The name is trimmed.
    ///
    /// # Errors
    /// [`LevelError::EmptyName`] if the name is only spaces.
    pub fn new(id: Id, name: &str, elevation: Length, storey: bool) -> Result<Self, LevelError> {
        Ok(Self {
            id,
            name: clean_name(name)?,
            elevation,
            storey,
        })
    }

    /// Its permanent ID.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// Its name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its height above the project's zero; below it when negative.
    #[must_use]
    pub fn elevation(&self) -> Length {
        self.elevation
    }

    /// Whether it is a building storey, rather than a reference datum.
    #[must_use]
    pub fn is_storey(&self) -> bool {
        self.storey
    }

    pub(crate) fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub(crate) fn set_elevation(&mut self, elevation: Length) {
        self.elevation = elevation;
    }

    pub(crate) fn set_storey(&mut self, storey: bool) {
        self.storey = storey;
    }

    /// The level as an element record of `document` (FORMAT.md beside this crate).
    #[must_use]
    pub fn to_record(&self, document: &Id) -> Element {
        let mut props = BTreeMap::new();
        props.insert(CATEGORY_KEY.to_owned(), LEVELS.0.to_string());
        props.insert(
            ELEVATION_KEY.to_owned(),
            Value::Length(self.elevation).stored(),
        );
        props.insert(STOREY_KEY.to_owned(), Value::YesNo(self.storey).stored());
        Element {
            id: self.id.clone(),
            document: document.clone(),
            kind: LEVEL_KIND.to_owned(),
            name: self.name.clone(),
            level: 0,
            placement: NO_PLACEMENT,
            recipe: level_recipe(),
            props,
            references: Vec::new(),
        }
    }

    /// Reads a level from its element record. Anything [`Level::to_record`] would not write is
    /// refused, never repaired, so one level has one byte string.
    ///
    /// # Errors
    /// [`LevelError::Malformed`], naming the record, for a record that is not a level record as
    /// written.
    pub fn from_record(record: &Element) -> Result<Self, LevelError> {
        let malformed =
            |why: &str| LevelError::Malformed(format!("The level record {} {why}.", record.id));
        if record.kind != LEVEL_KIND || record.recipe != level_recipe() {
            return Err(malformed("is not of kind level, with the level recipe"));
        }
        if record.level != 0 || record.placement != NO_PLACEMENT || !record.references.is_empty() {
            return Err(malformed("has a level, a placement or references"));
        }
        let Some(Value::Length(elevation)) = stored(record, ELEVATION_KEY) else {
            return Err(malformed("holds no elevation in its one spelling"));
        };
        let Some(Value::YesNo(storey)) = stored(record, STOREY_KEY) else {
            return Err(malformed("does not say yes or no to being a storey"));
        };
        let level = Self::new(record.id.clone(), &record.name, elevation, storey)
            .map_err(|_| malformed("has no name"))?;
        if level.to_record(&record.document) != *record {
            return Err(malformed("is not as a level record is written"));
        }
        Ok(level)
    }
}

/// A stored value under `key`, if it is in its one spelling.
fn stored(record: &Element, key: &str) -> Option<Value> {
    let text = record.props.get(key)?;
    Value::from_stored(text).filter(|value| value.stored() == *text)
}

/// The definition typed lengths are read and shown with: a length, of no element.
fn length_text() -> Result<Definition, LevelError> {
    let id = Id::parse(LENGTH_TEXT).map_err(|e| LevelError::Typed(e.to_string()))?;
    Definition::new(id, "Elevation", Kind::Length, Holder::Instance)
        .map_err(|e| LevelError::Typed(e.to_string()))
}

/// An elevation or offset typed by a person: `3.2 m`, `-450`, `12.5 mm`, read in `units`.
///
/// # Errors
/// [`LevelError::Typed`] with construct-params' own words for text that is not a length.
pub fn parse_length(text: &str, units: &Units) -> Result<Length, LevelError> {
    match length_text()?
        .parse(text, units)
        .map_err(|e| LevelError::Typed(e.to_string()))?
    {
        Value::Length(length) => Ok(length),
        _ => Err(LevelError::Typed(format!("{text:?} is not a length"))),
    }
}

/// A length as `units` show it: `3200 mm`. The exact length if the units cannot show it.
#[must_use]
pub fn show(length: Length, units: &Units) -> String {
    length_text()
        .and_then(|def| {
            def.format(&Value::Length(length), units)
                .map_err(|e| LevelError::Typed(e.to_string()))
        })
        .unwrap_or_else(|_| length.to_string())
}
