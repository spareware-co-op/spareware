//! The limits a railing is checked by, as data: a rule set per country's code, each limit with the
//! clause it comes from, beside the stair's rule sets (construct-stairs' `rules/`).
//!
//! A rule set is a JSON text (`rules/<id>.json` beside this crate; the format is in FORMAT.md, "A
//! rule set"), read strictly by [`RuleSet::from_json`]. The rule sets this program holds are built
//! in ([`RuleSet::builtin`]); the first is New Zealand's ([`NZ_F4_AS1`]): the Building Code's
//! clause F4 (Safety from falling) through its Acceptable Solution F4/AS1, third edition, amendment
//! 2, effective 1 January 2017, with the handrail heights of D1/AS1 (6.0.6) beside them. Another
//! country's limits are another file and one line in [`RuleSet::builtin`].
//!
//! What a rule set holds:
//!
//! - **Categories**: where a barrier stands, each a row of F4/AS1's Table 1, with its least height
//!   on a stair's flight (`flight_least`, measured upright from the pitch line, Table 1's Note 1),
//!   where it has one, and on a floor, a landing or anything level (`level_least`), and the openings
//!   rules it may be checked by, its default first. A flight of a category that has no flight
//!   height is held to its level height.
//! - **Openings rules**: the opening a sphere must not pass, by who uses the place. Either a
//!   sphere (`sphere`, anywhere in the barrier; `stair_sphere`, under its foot on a flight: the
//!   opening of riser, tread and the barrier's foot), so an opening as wide as the sphere fails;
//!   or the widest openings allowed (`between_verticals`, between upright members;
//!   `between_rails`, upright between the floor and the barrier's foot), so an opening wider fails.
//! - **Handrail**: the least and most height of a handrail's top above the pitch line.
//!
//! Lengths are written as a person types them (`900 mm`) and read with construct-params' metric
//! display settings (construct-levels' `parse_length`), so a length off the grid is refused.

use crate::error::RailingError;
use construct_levels::parse_length;
use construct_params::{Length, Units};
use serde::Deserialize;

/// The ID of New Zealand's rule set: clause F4 of the Building Code through F4/AS1.
pub const NZ_F4_AS1: &str = "nz-f4-as1";

/// The version of a rule set's text this crate reads.
pub const RULES_VERSION: u32 = 1;

/// A length limit and the clause it comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LengthLimit {
    /// The length.
    pub length: Length,
    /// The clause of the rule set it comes from.
    pub clause: String,
}

/// Where a barrier stands: one row of the code's table of barrier heights.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category {
    /// Its ID within the rule set: `household-stair`.
    pub id: String,
    /// Its name, as the code describes the place.
    pub name: String,
    /// The least height on a stair's flight, above the pitch line, where the code sets one.
    pub flight_least: Option<LengthLimit>,
    /// The least height on a floor, a landing or anything level.
    pub level_least: LengthLimit,
    /// The openings rules it may be checked by, its default first.
    pub openings: Vec<String>,
}

impl Category {
    /// The least height on a flight: its own, or its level height where it has none.
    #[must_use]
    pub fn flight(&self) -> &LengthLimit {
        self.flight_least.as_ref().unwrap_or(&self.level_least)
    }
}

/// How an openings rule limits the openings of a barrier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpeningLimits {
    /// No sphere of these sizes passes: an opening at least as wide as the sphere fails.
    Sphere {
        /// Anywhere over the barrier's height.
        sphere: LengthLimit,
        /// Under the barrier's foot on a flight, between riser, tread and foot.
        stair_sphere: LengthLimit,
    },
    /// The widest openings allowed: an opening wider fails.
    Most {
        /// Between upright members.
        between_verticals: LengthLimit,
        /// Upright, between the floor (a tread, on a flight) and the barrier's foot.
        between_rails: LengthLimit,
    },
}

/// One openings rule, its limits and its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Openings {
    /// Its ID: `children`.
    pub id: String,
    /// Who uses the place, as the code says it.
    pub name: String,
    /// Its limits.
    pub limits: OpeningLimits,
}

/// A rule set: the limits of one country's code, as the module header says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleSet {
    /// Its ID: `nz-f4-as1`.
    pub id: String,
    /// The code it comes from, in words.
    pub name: String,
    /// The edition and amendment of the code it holds.
    pub edition: String,
    /// Where the code is published.
    pub source: String,
    /// Its categories, in the order the text gives them.
    pub categories: Vec<Category>,
    /// Its openings rules.
    pub openings: Vec<Openings>,
    /// The least height of a handrail's top above the pitch line.
    pub handrail_least: LengthLimit,
    /// The most.
    pub handrail_most: LengthLimit,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Text {
    version: u32,
    id: String,
    name: String,
    edition: String,
    source: String,
    categories: Vec<CategoryText>,
    openings: Vec<OpeningsText>,
    handrail: HandrailText,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LengthText {
    length: String,
    clause: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CategoryText {
    id: String,
    name: String,
    #[serde(default)]
    flight_least: Option<LengthText>,
    level_least: LengthText,
    openings: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpeningsText {
    id: String,
    name: String,
    #[serde(default)]
    sphere: Option<LengthText>,
    #[serde(default)]
    stair_sphere: Option<LengthText>,
    #[serde(default)]
    between_verticals: Option<LengthText>,
    #[serde(default)]
    between_rails: Option<LengthText>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HandrailText {
    least: LengthText,
    most: LengthText,
}

fn malformed(why: impl Into<String>) -> RailingError {
    RailingError::Malformed(format!("A railing rule set does not read: {}.", why.into()))
}

impl LengthText {
    fn read(&self, of: &str) -> Result<LengthLimit, RailingError> {
        let length = parse_length(&self.length, &Units::default())
            .map_err(|e| malformed(format!("{of}: {e}")))?;
        if length.steps() <= 0 {
            return Err(malformed(format!("{of} is not more than nothing")));
        }
        if self.clause.trim().is_empty() {
            return Err(malformed(format!("{of} cites no clause")));
        }
        Ok(LengthLimit {
            length,
            clause: self.clause.clone(),
        })
    }
}

impl OpeningsText {
    fn read(&self) -> Result<Openings, RailingError> {
        let of = |what: &str| format!("{}'s {what}", self.id);
        let limits = match (
            &self.sphere,
            &self.stair_sphere,
            &self.between_verticals,
            &self.between_rails,
        ) {
            (Some(sphere), Some(stair), None, None) => OpeningLimits::Sphere {
                sphere: sphere.read(&of("sphere"))?,
                stair_sphere: stair.read(&of("stair sphere"))?,
            },
            (None, None, Some(verticals), Some(rails)) => OpeningLimits::Most {
                between_verticals: verticals.read(&of("most between verticals"))?,
                between_rails: rails.read(&of("most between rails"))?,
            },
            _ => {
                return Err(malformed(format!(
                    "openings rule {:?} gives sphere and stair_sphere, or between_verticals and between_rails",
                    self.id
                )));
            }
        };
        Ok(Openings {
            id: self.id.clone(),
            name: self.name.clone(),
            limits,
        })
    }
}

impl RuleSet {
    /// A rule set from its text, read strictly: every field as FORMAT.md writes it and no other,
    /// every length on the grid and more than nothing, every clause given, at least one category
    /// and one openings rule, no ID twice, every category naming at least one openings rule the
    /// set has.
    ///
    /// # Errors
    /// [`RailingError::Malformed`] naming what does not read.
    pub fn from_json(text: &str) -> Result<Self, RailingError> {
        let t: Text = serde_json::from_str(text).map_err(|e| malformed(e.to_string()))?;
        if t.version != RULES_VERSION {
            return Err(malformed(format!(
                "its version is {}, and this program reads {RULES_VERSION}",
                t.version
            )));
        }
        if t.id.trim().is_empty() || t.categories.is_empty() || t.openings.is_empty() {
            return Err(malformed(
                "it needs an ID, at least one category and one openings rule",
            ));
        }
        let mut openings: Vec<Openings> = Vec::with_capacity(t.openings.len());
        for o in &t.openings {
            if o.id.trim().is_empty() || openings.iter().any(|k| k.id == o.id) {
                return Err(malformed(format!(
                    "openings rule {:?} is blank or given twice",
                    o.id
                )));
            }
            openings.push(o.read()?);
        }
        let mut categories: Vec<Category> = Vec::with_capacity(t.categories.len());
        for c in &t.categories {
            if c.id.trim().is_empty() || categories.iter().any(|k| k.id == c.id) {
                return Err(malformed(format!(
                    "category {:?} is blank or given twice",
                    c.id
                )));
            }
            if c.openings.is_empty()
                || c.openings
                    .iter()
                    .any(|id| !openings.iter().any(|o| o.id == *id))
            {
                return Err(malformed(format!(
                    "category {:?} names no openings rule, or one the set does not have",
                    c.id
                )));
            }
            let of = |what: &str| format!("{}'s {what}", c.id);
            categories.push(Category {
                id: c.id.clone(),
                name: c.name.clone(),
                flight_least: c
                    .flight_least
                    .as_ref()
                    .map(|l| l.read(&of("least height on a flight")))
                    .transpose()?,
                level_least: c.level_least.read(&of("least height"))?,
                openings: c.openings.clone(),
            });
        }
        let handrail_least = t.handrail.least.read("the least handrail height")?;
        let handrail_most = t.handrail.most.read("the most handrail height")?;
        if handrail_most.length.steps() < handrail_least.length.steps() {
            return Err(malformed("its most handrail height is below its least"));
        }
        Ok(Self {
            id: t.id,
            name: t.name,
            edition: t.edition,
            source: t.source,
            categories,
            openings,
            handrail_least,
            handrail_most,
        })
    }

    /// A rule set this program holds, by its ID: [`NZ_F4_AS1`] today.
    ///
    /// # Errors
    /// [`RailingError::UnknownRules`] for any other ID; [`RailingError::Malformed`] if the built-in
    /// text did not read, which its tests rule out.
    pub fn builtin(id: &str) -> Result<Self, RailingError> {
        match id {
            NZ_F4_AS1 => Self::from_json(include_str!("../rules/nz-f4-as1.json")),
            other => Err(RailingError::UnknownRules(other.to_owned())),
        }
    }

    /// One of its categories, by ID.
    ///
    /// # Errors
    /// [`RailingError::UnknownCategory`] naming the categories it has.
    pub fn category(&self, id: &str) -> Result<&Category, RailingError> {
        self.categories
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| RailingError::UnknownCategory {
                rules: self.id.clone(),
                category: id.to_owned(),
                known: self.categories.iter().map(|c| c.id.clone()).collect(),
            })
    }

    /// The openings rule `id`, as `category` takes it.
    ///
    /// # Errors
    /// [`RailingError::UnknownOpenings`] for one the category does not take.
    pub fn openings_for(&self, category: &Category, id: &str) -> Result<&Openings, RailingError> {
        let refused = || RailingError::UnknownOpenings {
            category: category.id.clone(),
            openings: id.to_owned(),
            allowed: category.openings.clone(),
        };
        if !category.openings.iter().any(|o| o == id) {
            return Err(refused());
        }
        self.openings
            .iter()
            .find(|o| o.id == id)
            .ok_or_else(refused)
    }
}
