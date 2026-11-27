//! The limits a stair is laid out by, as data: a rule set per country's code, each limit with the
//! clause it comes from.
//!
//! A rule set is a JSON text (`rules/<id>.json` beside this crate; the format is in FORMAT.md,
//! "A rule set"), read strictly by [`RuleSet::from_json`]. The rule sets this program holds are
//! built in ([`RuleSet::builtin`]); the first is New Zealand's ([`NZ_D1_AS1`]): the Building Code's
//! clause D1 (Access routes) through its Acceptable Solution D1/AS1, second edition, amendment 6,
//! effective 1 January 2017. Another country's limits are another file and one line in
//! [`RuleSet::builtin`]; no code of the layout changes.
//!
//! What a rule set holds, for every stair: the least pitch, the least riser, the most going, how
//! far apart the risers of one flight may be, the least landing length and the least headroom
//! above the pitch line; and per category of stairway (accessible, common, main private and so
//! on): the most pitch, the most riser, the least going, the most rise between landings and,
//! where the code sets one, the least width. Lengths are written as a person types them (`190
//! mm`) and read with construct-params' metric display settings (construct-levels'
//! `parse_length`), so a length off the grid is refused; pitches are whole degrees.
//!
//! A pitch in degrees is compared as a slope: the slope a roof's pitch of the same angle is
//! worked as (construct-roofs' `Pitch::slope`: rise `round(10^9 tan a)` over a run of 10^9, the
//! one float of that rule, decided in the simple-roof note). A riser `r` on a going `g` is within
//! a pitch limit of slope `p/q` when `r * q` and `g * p` compare so, exactly.

use crate::error::{Named, StairError};
use construct_levels::parse_length;
use construct_params::{Angle, Length, Ratio, Slope, Units};
use construct_roofs::Pitch;
use serde::Deserialize;

/// The ID of New Zealand's rule set: clause D1 of the Building Code through D1/AS1.
pub const NZ_D1_AS1: &str = "nz-d1-as1";

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

impl LengthLimit {
    /// The limit as a refusal names it, saying what it limits.
    #[must_use]
    pub fn named(&self, what: &str) -> Named {
        Named {
            what: what.to_owned(),
            value: self.length,
            clause: self.clause.clone(),
        }
    }
}

/// A pitch limit: whole degrees, the slope they are compared as, and the clause.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PitchLimit {
    /// The angle in whole degrees, as the rule set gives it.
    pub degrees: u32,
    /// The slope it is compared as (the module header says how it is worked).
    pub slope: Slope,
    /// The clause of the rule set it comes from.
    pub clause: String,
}

/// One category of stairway, its limits and its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category {
    /// Its ID within the rule set: `common`.
    pub id: String,
    /// Its name as the code gives it: `Common stairway`.
    pub name: String,
    /// The steepest pitch.
    pub pitch_most: PitchLimit,
    /// The tallest riser.
    pub riser_most: LengthLimit,
    /// The shortest going.
    pub going_least: LengthLimit,
    /// The most a stair may rise between landings.
    pub rise_between_landings_most: LengthLimit,
    /// The narrowest width, where the code sets one.
    pub width_least: Option<LengthLimit>,
}

/// A rule set: the limits of one country's code, as the module header says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleSet {
    /// Its ID: `nz-d1-as1`.
    pub id: String,
    /// The code it comes from, in words.
    pub name: String,
    /// The edition and amendment of the code it holds.
    pub edition: String,
    /// Where the code is published.
    pub source: String,
    /// The least pitch.
    pub pitch_least: PitchLimit,
    /// The lowest riser.
    pub riser_least: LengthLimit,
    /// The longest going.
    pub going_most: LengthLimit,
    /// The most two risers of one flight may differ.
    pub uniform_within: LengthLimit,
    /// The shortest landing.
    pub landing_least: LengthLimit,
    /// The least clear height above the pitch line of a stair or a landing.
    pub headroom_least: LengthLimit,
    /// Its categories of stairway, in the order the text gives them.
    pub categories: Vec<Category>,
}

/// A rule set's text, as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Text {
    version: u32,
    id: String,
    name: String,
    edition: String,
    source: String,
    pitch_least: PitchText,
    riser_least: LengthText,
    going_most: LengthText,
    uniform_within: LengthText,
    landing_least: LengthText,
    headroom_least: LengthText,
    categories: Vec<CategoryText>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LengthText {
    length: String,
    clause: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PitchText {
    degrees: u32,
    clause: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CategoryText {
    id: String,
    name: String,
    pitch_most: PitchText,
    riser_most: LengthText,
    going_least: LengthText,
    rise_between_landings_most: LengthText,
    #[serde(default)]
    width_least: Option<LengthText>,
}

fn malformed(why: impl Into<String>) -> StairError {
    StairError::Malformed(format!("A rule set does not read: {}.", why.into()))
}

fn clause(text: &str, of: &str) -> Result<String, StairError> {
    if text.trim().is_empty() {
        return Err(malformed(format!("{of} cites no clause")));
    }
    Ok(text.to_owned())
}

impl LengthText {
    fn read(&self, of: &str) -> Result<LengthLimit, StairError> {
        let length = parse_length(&self.length, &Units::default())
            .map_err(|e| malformed(format!("{of}: {e}")))?;
        if length.steps() < 0 {
            return Err(malformed(format!("{of} is below nothing")));
        }
        Ok(LengthLimit {
            length,
            clause: clause(&self.clause, of)?,
        })
    }
}

impl PitchText {
    fn read(&self, of: &str) -> Result<PitchLimit, StairError> {
        if !(1..90).contains(&self.degrees) {
            return Err(malformed(format!(
                "{of} is {} degrees, not from 1 to 89",
                self.degrees
            )));
        }
        let slope = Angle::from_degrees(Ratio::from_integer(i128::from(self.degrees)))
            .and_then(|angle| Pitch::Angle(angle).slope())
            .ok_or_else(|| malformed(format!("{of} has no slope")))?;
        Ok(PitchLimit {
            degrees: self.degrees,
            slope,
            clause: clause(&self.clause, of)?,
        })
    }
}

impl RuleSet {
    /// A rule set from its text, read strictly: every field as FORMAT.md writes it and no other,
    /// every length a length on the grid and at least nothing, every pitch whole degrees from 1
    /// to 89, every clause given, at least one category and no two with one ID.
    ///
    /// # Errors
    /// [`StairError::Malformed`] naming what does not read.
    pub fn from_json(text: &str) -> Result<Self, StairError> {
        let t: Text = serde_json::from_str(text).map_err(|e| malformed(e.to_string()))?;
        if t.version != RULES_VERSION {
            return Err(malformed(format!(
                "its version is {}, and this program reads {RULES_VERSION}",
                t.version
            )));
        }
        if t.id.trim().is_empty() || t.categories.is_empty() {
            return Err(malformed("it needs an ID and at least one category"));
        }
        let mut categories: Vec<Category> = Vec::with_capacity(t.categories.len());
        for c in &t.categories {
            if c.id.trim().is_empty() || categories.iter().any(|k| k.id == c.id) {
                return Err(malformed(format!(
                    "category {:?} is blank or given twice",
                    c.id
                )));
            }
            let of = |what: &str| format!("{}'s {what}", c.id);
            categories.push(Category {
                id: c.id.clone(),
                name: c.name.clone(),
                pitch_most: c.pitch_most.read(&of("most pitch"))?,
                riser_most: c.riser_most.read(&of("most riser"))?,
                going_least: c.going_least.read(&of("least going"))?,
                rise_between_landings_most: c
                    .rise_between_landings_most
                    .read(&of("most rise between landings"))?,
                width_least: c
                    .width_least
                    .as_ref()
                    .map(|w| w.read(&of("least width")))
                    .transpose()?,
            });
        }
        Ok(Self {
            id: t.id,
            name: t.name,
            edition: t.edition,
            source: t.source,
            pitch_least: t.pitch_least.read("the least pitch")?,
            riser_least: t.riser_least.read("the least riser")?,
            going_most: t.going_most.read("the most going")?,
            uniform_within: t.uniform_within.read("how far apart risers may be")?,
            landing_least: t.landing_least.read("the least landing")?,
            headroom_least: t.headroom_least.read("the least headroom")?,
            categories,
        })
    }

    /// A rule set this program holds, by its ID: [`NZ_D1_AS1`] today.
    ///
    /// # Errors
    /// [`StairError::UnknownRules`] for any other ID; [`StairError::Malformed`] if the built-in
    /// text did not read, which its tests rule out.
    pub fn builtin(id: &str) -> Result<Self, StairError> {
        match id {
            NZ_D1_AS1 => Self::from_json(include_str!("../rules/nz-d1-as1.json")),
            other => Err(StairError::UnknownRules(other.to_owned())),
        }
    }

    /// One of its categories, by ID.
    ///
    /// # Errors
    /// [`StairError::UnknownCategory`] naming the categories it has.
    pub fn category(&self, id: &str) -> Result<&Category, StairError> {
        self.categories
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| StairError::UnknownCategory {
                rules: self.id.clone(),
                category: id.to_owned(),
                known: self.categories.iter().map(|c| c.id.clone()).collect(),
            })
    }
}
