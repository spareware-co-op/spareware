//! A material and its three separate groups of data.
//!
//! 1. Drawing graphics ([`Drawing`]): how a view draws the material, in projection (`surface`)
//!    and where it cuts through it (`cut`). Each is a [`Graphics`]: a line colour and a line
//!    pattern ([`construct_styles::Colour`], [`construct_styles::PatternId`], as object styles
//!    use them) and a *fill*, a named reference to an area fill pattern.
//!    **The place for fill patterns:** area fill patterns (hatches) are a later job
//!    (CON-A8-F3-P3); this crate stores only the fill's name as a [`RefName`] and never looks it
//!    up, so that job can define the patterns and nothing here changes.
//! 2. Physical data ([`Physical`]): density, and optional thermal conductivity and specific heat,
//!    each a whole number in a stated unit with the source it came from ([`Measure`]).
//! 3. A render look ([`Material::look`]): a name only. Looks are a later job (CON-A16-F1-P3).
//!
//! The three are separate fields so that a change to one never touches another.

use crate::error::MaterialError;
use construct_styles::{Colour, PatternId};
use kit_core::Id;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The longest name or reference name, in characters.
pub const MOST_NAME_CHARS: usize = 100;
/// The largest density stored: 100,000 kg/m3, above any solid.
pub const MOST_DENSITY: u64 = 100_000_000;
/// The largest thermal conductivity stored: 1,000 W/(m K), above any solid.
pub const MOST_CONDUCTIVITY: u64 = 1_000_000;
/// The largest specific heat stored: 100,000 J/(kg K).
pub const MOST_SPECIFIC_HEAT: u64 = 100_000;

/// A material's permanent ID: the kit's checked ID, the one a material parameter value holds.
/// It never changes, whatever is edited, and is never given out twice.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MaterialId(Id);

impl MaterialId {
    /// Checks and wraps an ID text.
    ///
    /// # Errors
    /// [`MaterialError::BadText`] if the text is not a kit ID.
    pub fn parse(text: &str) -> Result<Self, MaterialError> {
        Id::parse(text)
            .map(Self)
            .map_err(|_| MaterialError::BadText { field: "id" })
    }

    /// Wraps a kit ID.
    #[must_use]
    pub fn from_id(id: Id) -> Self {
        Self(id)
    }

    /// The kit ID, for a material parameter value.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.0
    }

    /// The ID as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for MaterialId {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.0.as_str())
    }
}

impl Serialize for MaterialId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0.as_str())
    }
}

impl<'de> Deserialize<'de> for MaterialId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// What a material is made of, as a building sorts it.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    /// Cast or precast concrete, mortar and screed.
    Concrete,
    /// Brick, block and stone.
    Masonry,
    /// Solid timber and timber products.
    Timber,
    /// Steel and iron.
    Steel,
    /// Metals other than steel.
    Metal,
    /// Glass.
    Glass,
    /// Thermal and acoustic insulation.
    Insulation,
    /// Plasterboard and similar boards.
    Plasterboard,
    /// Wet-applied plaster and render.
    Plaster,
    /// Membranes, wraps and damp-proof sheets.
    Membrane,
    /// Paint, tile, carpet and other finishes.
    Finish,
    /// Ground, fill and soil.
    Earth,
    /// An air gap or cavity.
    AirGap,
    /// Anything that is none of these.
    Other,
}

/// A name that points at something defined elsewhere (a fill pattern, a render look). Trimmed,
/// 1 to 100 characters, no control character. This crate never looks one up.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RefName(String);

impl RefName {
    /// Checks a reference name.
    ///
    /// # Errors
    /// [`MaterialError::BadText`] if it is empty, too long or holds a control character.
    pub fn new(text: &str) -> Result<Self, MaterialError> {
        let trimmed = text.trim();
        if trimmed.is_empty() || !text_ok(trimmed) {
            return Err(MaterialError::BadText { field: "reference" });
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for RefName {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for RefName {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        let name = Self::new(&text).map_err(serde::de::Error::custom)?;
        if name.0 != text {
            return Err(serde::de::Error::custom("reference name is not trimmed"));
        }
        Ok(name)
    }
}

/// True if the text is within the length limit and holds no control character.
pub(crate) fn text_ok(text: &str) -> bool {
    text.chars().count() <= MOST_NAME_CHARS && !text.chars().any(char::is_control)
}

/// How a view draws the material in one role (projection or cut).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Graphics {
    /// The line colour.
    pub colour: Colour,
    /// The line pattern, by ID in the project's pattern table.
    pub pattern: PatternId,
    /// The area fill, by name: the place for fill patterns, a later job. `None` draws no fill.
    pub fill: Option<RefName>,
}

/// A material's drawing graphics.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Drawing {
    /// How a view draws the material seen in projection: its face, not cut.
    pub surface: Graphics,
    /// How a view draws the material where the view's cut plane passes through it.
    pub cut: Graphics,
}

/// Where a value came from: a source in the library by key, and where in it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceRef {
    /// The source's key in the library.
    pub source: String,
    /// The table and page, or other place in the source, as it records it.
    pub at: String,
}

/// A physical value: a whole number whose unit the field using it states, and its source.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Measure {
    /// The value, in the unit of the field that holds it.
    pub value: u64,
    /// Where it came from. Every built-in material's values have one; a project's own may not.
    pub source: Option<SourceRef>,
}

/// A material's physical data. Each number is whole, in the unit its field name states, so none
/// is a float.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Physical {
    /// Density in whole grams per cubic metre (1 kg/m3 is 1000). 1 to 100,000,000.
    pub density_g_per_m3: Measure,
    /// Thermal conductivity in whole milliwatts per metre kelvin (1 W/(m K) is 1000), if known.
    /// 1 to 1,000,000.
    pub conductivity_mw_per_m_k: Option<Measure>,
    /// Specific heat capacity in whole joules per kilogram kelvin, if known. 1 to 100,000.
    pub specific_heat_j_per_kg_k: Option<Measure>,
}

impl Physical {
    /// Checks every number is in its range.
    ///
    /// # Errors
    /// [`MaterialError::BadValue`] for the first value out of range.
    pub fn check(&self) -> Result<(), MaterialError> {
        range("density", self.density_g_per_m3.value, MOST_DENSITY)?;
        if let Some(measure) = &self.conductivity_mw_per_m_k {
            range("conductivity", measure.value, MOST_CONDUCTIVITY)?;
        }
        if let Some(measure) = &self.specific_heat_j_per_kg_k {
            range("specific_heat", measure.value, MOST_SPECIFIC_HEAT)?;
        }
        Ok(())
    }

    /// Every source reference this holds, in field order.
    pub(crate) fn source_refs(&self) -> Vec<&SourceRef> {
        [
            Some(&self.density_g_per_m3),
            self.conductivity_mw_per_m_k.as_ref(),
            self.specific_heat_j_per_kg_k.as_ref(),
        ]
        .into_iter()
        .flatten()
        .filter_map(|measure| measure.source.as_ref())
        .collect()
    }

    /// True if every value this holds has a source.
    #[must_use]
    pub fn all_sourced(&self) -> bool {
        [
            Some(&self.density_g_per_m3),
            self.conductivity_mw_per_m_k.as_ref(),
            self.specific_heat_j_per_kg_k.as_ref(),
        ]
        .into_iter()
        .flatten()
        .all(|measure| measure.source.is_some())
    }
}

fn range(property: &'static str, value: u64, most: u64) -> Result<(), MaterialError> {
    if value == 0 || value > most {
        return Err(MaterialError::BadValue { property, value });
    }
    Ok(())
}

/// One material.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Material {
    /// Its permanent ID.
    pub id: MaterialId,
    /// The name people see. Unique in a library, compared without regard to case.
    pub name: String,
    /// What it is made of.
    pub class: Class,
    /// True for a material of the built-in set. Only the default library sets it.
    pub built_in: bool,
    /// Group 1: drawing graphics.
    pub drawing: Drawing,
    /// Group 2: physical data.
    pub physical: Physical,
    /// Group 3: the render look, by name only.
    pub look: Option<RefName>,
}

/// What a caller gives to make a material; the library adds the ID it is given and clears
/// `built_in`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewMaterial {
    /// The name.
    pub name: String,
    /// The class.
    pub class: Class,
    /// Drawing graphics.
    pub drawing: Drawing,
    /// Physical data.
    pub physical: Physical,
    /// Render look, by name.
    pub look: Option<RefName>,
}

/// Trims a material name and checks it.
///
/// # Errors
/// [`MaterialError::EmptyName`] or [`MaterialError::BadText`].
pub(crate) fn clean_name(name: &str) -> Result<String, MaterialError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(MaterialError::EmptyName);
    }
    if !text_ok(trimmed) {
        return Err(MaterialError::BadText { field: "name" });
    }
    Ok(trimmed.to_owned())
}

/// The key a name is compared by: lower case.
pub(crate) fn name_key(name: &str) -> String {
    name.to_lowercase()
}
